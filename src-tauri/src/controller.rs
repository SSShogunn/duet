use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::bluetooth::{self, Outcome};
use crate::devices::{self, OutputDevice};
use crate::engine::{Engine, EngineHandle, TargetSpec};
use crate::mic_use;
use crate::settings::{SettingsStore, MAX_SPEAKERS};
use crate::sync;

const POLL: Duration = Duration::from_millis(500);
const METER_INTERVAL: Duration = Duration::from_millis(40);
const RETRY_DELAY: Duration = Duration::from_secs(3);
const AUTO_SYNC_DELAY: Duration = Duration::from_secs(8);
const GLITCH_SYNC_DELAY: Duration = Duration::from_secs(10);
const MIN_SYNC_GAP: Duration = Duration::from_secs(60);
const BUSY_MIC_RETRY: Duration = Duration::from_secs(60);
const FAILED_RETRY: Duration = Duration::from_secs(300);
const UNSTABLE_WINDOW: Duration = Duration::from_secs(300);
const UNSTABLE_GLITCHES: usize = 3;
const STABLE_AGAIN: Duration = Duration::from_secs(600);
const RECONNECT_WAIT: Duration = Duration::from_secs(8);

#[derive(Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub is_virtual: bool,
    pub enabled: bool,
    pub offset_ms: f32,
    pub side: Option<String>,
}

#[derive(Serialize, Clone, Default)]
pub struct MetersView {
    pub spectrum: Vec<f32>,
    pub levels: HashMap<String, f32>,
}

#[derive(Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StateView {
    pub status: String,
    pub running: bool,
    pub syncing: bool,
    pub virtual_selected: bool,
    pub virtual_name: Option<String>,
    pub devices: Vec<DeviceView>,
    pub stereo: bool,
    pub swap: bool,
    pub auto_sync: bool,
    pub sync_sound: String,
    pub background_sync_min: u32,
    pub unstable_speaker: Option<String>,
    pub bluetooth_resetting: bool,
    pub cable_renamed: bool,
    pub cable_prompt: bool,
    pub last_sync: Option<String>,
}

pub struct Controller {
    pub settings: SettingsStore,
    engine: Option<Engine>,
    engine_targets: Vec<String>,
    devices: Vec<OutputDevice>,
    status: String,
    syncing: bool,
    last_sync: Option<String>,
    auto_sync_at: Option<Instant>,
    background_at: Option<Instant>,
    background_syncing: bool,
    last_sync_at: Option<Instant>,
    speaker_glitches: HashMap<String, u64>,
    glitch_times: HashMap<String, Vec<Instant>>,
    unstable: Option<String>,
    bluetooth_resetting: bool,
    retry_at: Instant,
}

pub type Shared = Arc<Mutex<Controller>>;

impl Controller {
    pub fn new(settings: SettingsStore) -> Controller {
        Controller {
            settings,
            engine: None,
            engine_targets: Vec::new(),
            devices: Vec::new(),
            status: "Starting...".into(),
            syncing: false,
            last_sync: None,
            auto_sync_at: None,
            background_at: None,
            background_syncing: false,
            last_sync_at: None,
            speaker_glitches: HashMap::new(),
            glitch_times: HashMap::new(),
            unstable: None,
            bluetooth_resetting: false,
            retry_at: Instant::now(),
        }
    }

    fn wanted_targets(&self) -> Vec<OutputDevice> {
        let mut wanted: Vec<OutputDevice> = self
            .settings
            .data
            .targets
            .iter()
            .filter_map(|id| self.devices.iter().find(|d| !d.is_virtual && &d.id == id))
            .take(MAX_SPEAKERS)
            .cloned()
            .collect();
        wanted.sort_by(|a, b| a.name.cmp(&b.name));
        wanted
    }

    fn sides(&self, wanted: &[OutputDevice]) -> Vec<Option<usize>> {
        if !self.settings.data.stereo || wanted.len() != 2 {
            return vec![None; wanted.len()];
        }
        if self.settings.data.swap { vec![Some(1), Some(0)] } else { vec![Some(0), Some(1)] }
    }

    pub fn apply_sides(&self) {
        if let Some(engine) = &self.engine {
            let wanted = self.wanted_targets();
            for (device, side) in wanted.iter().zip(self.sides(&wanted)) {
                engine.handle().set_channel(&device.id, side);
            }
        }
    }

    pub fn set_engine_offset(&self, id: &str, ms: f32) {
        if let Some(engine) = &self.engine {
            engine.handle().set_extra(id, ms);
        }
    }

    fn stop_engine(&mut self) {
        if let Some(engine) = self.engine.take() {
            engine.stop();
        }
        self.engine_targets.clear();
        self.auto_sync_at = None;
        self.background_at = None;
        self.speaker_glitches.clear();
        self.glitch_times.clear();
        self.unstable = None;
    }

    fn track_glitches(&mut self) -> Option<String> {
        let speakers = self.engine.as_ref()?.handle().speaker_glitches();
        let now = Instant::now();
        let mut newly_unstable = None;
        for (name, count) in speakers {
            let before = self.speaker_glitches.insert(name.clone(), count).unwrap_or(0);
            if count <= before {
                continue;
            }
            let times = self.glitch_times.entry(name.clone()).or_default();
            times.push(now);
            times.retain(|t| now.duration_since(*t) < UNSTABLE_WINDOW);
            if times.len() >= UNSTABLE_GLITCHES && self.unstable.is_none() {
                self.unstable = Some(name.clone());
                newly_unstable = Some(name);
            }
        }
        if let Some(name) = &self.unstable {
            let last = self.glitch_times.get(name).and_then(|t| t.last().copied());
            if last.is_none_or(|t| now.duration_since(t) > STABLE_AGAIN) {
                log::info!("{name} has been stable again for a while");
                self.unstable = None;
            }
        }
        newly_unstable
    }

    fn background_allowed(&self) -> bool {
        self.settings.data.background_sync_min > 0
            && self.settings.data.sync_sound != "chime"
            && self.engine_targets.len() > 1
    }

    pub fn schedule_background(&mut self) {
        self.background_at = self
            .background_allowed()
            .then(|| Instant::now() + Duration::from_secs(self.settings.data.background_sync_min as u64 * 60));
    }

    fn schedule_after_glitch(&mut self) {
        if !self.background_allowed() || self.syncing || self.unstable.is_some() {
            return;
        }
        let mut at = Instant::now() + GLITCH_SYNC_DELAY;
        if let Some(last) = self.last_sync_at {
            at = at.max(last + MIN_SYNC_GAP);
        }
        if self.background_at.is_none_or(|current| at < current) {
            log::debug!("background sync moved up after a glitch");
            self.background_at = Some(at);
        }
    }

    fn begin_bluetooth_reset(&mut self) -> Result<(), String> {
        if self.bluetooth_resetting {
            return Err("Bluetooth is already being reset.".into());
        }
        if self.syncing {
            return Err("Wait for the sync to finish, then try again.".into());
        }
        self.stop_engine();
        self.bluetooth_resetting = true;
        Ok(())
    }

    fn finish_bluetooth_reset(&mut self) {
        self.bluetooth_resetting = false;
        self.retry_at = Instant::now() + RECONNECT_WAIT;
    }

    fn tick(&mut self) {
        if self.bluetooth_resetting {
            self.status = "Resetting Bluetooth. Sound cuts out for about 15 seconds.".into();
            return;
        }
        match devices::list_outputs() {
            Ok(list) => self.devices = list,
            Err(e) => {
                let message = "Couldn't read your audio devices. Retrying...".to_string();
                if self.status != message {
                    log::error!("listing audio devices failed: {e}");
                }
                self.status = message;
                return;
            }
        }
        let source = self.devices.iter().find(|d| d.is_default && d.is_virtual).cloned();
        let wanted = self.wanted_targets();
        let wanted_ids: Vec<String> = wanted.iter().map(|d| d.id.clone()).collect();

        if let Some(engine) = &self.engine {
            let failed = !engine.handle().is_running();
            if failed {
                self.status = format!("Audio stopped: {}", engine.error().unwrap_or_default());
                self.retry_at = Instant::now() + RETRY_DELAY;
            }
            if failed || source.is_none() || wanted_ids != self.engine_targets {
                if !self.syncing {
                    self.stop_engine();
                }
            }
        }

        let Some(source) = source else {
            let name = self.preferred_virtual().map(|d| d.name.clone());
            self.status = match name {
                Some(name) => format!("Idle. Select \"{name}\" as your Windows sound output to start."),
                None => "Duet needs the free VB-Audio Virtual Cable driver. Install it, then restart your PC.".into(),
            };
            return;
        };

        if self.engine.is_none() && Instant::now() >= self.retry_at {
            if wanted.is_empty() {
                self.status = "Pick the speakers to play on.".into();
                return;
            }
            let specs = wanted
                .iter()
                .zip(self.sides(&wanted))
                .map(|(d, side)| TargetSpec {
                    id: d.id.clone(),
                    name: d.name.clone(),
                    extra_ms: self.settings.data.offsets.get(&d.id).copied().unwrap_or(0.0),
                    channel: side,
                })
                .collect();
            self.engine = Some(Engine::start(source.id.clone(), specs));
            self.engine_targets = wanted_ids;
            self.status = format!("Playing on {}", wanted.iter().map(|d| short(&d.name)).collect::<Vec<_>>().join(", "));
            if self.settings.data.auto_sync && wanted.len() > 1 {
                self.auto_sync_at = Some(Instant::now() + AUTO_SYNC_DELAY);
            }
            self.schedule_background();
        }
    }

    pub fn view(&self) -> StateView {
        let wanted = self.wanted_targets();
        let sides = self.sides(&wanted);
        let side_of = |id: &str| {
            wanted
                .iter()
                .zip(&sides)
                .find(|(d, _)| d.id == id)
                .and_then(|(_, s)| s.map(|c| if c == 0 { "L".to_string() } else { "R".to_string() }))
        };
        let handle = self.engine.as_ref().map(|e| e.handle());
        StateView {
            status: if self.syncing && !self.background_syncing {
                "Syncing speakers: stay quiet for a few seconds...".into()
            } else {
                self.status.clone()
            },
            running: handle.as_ref().is_some_and(|h| h.is_running()),
            syncing: self.syncing && !self.background_syncing,
            virtual_selected: self.devices.iter().any(|d| d.is_default && d.is_virtual),
            virtual_name: self.preferred_virtual().map(|d| d.name.clone()),
            devices: self
                .devices
                .iter()
                .map(|d| DeviceView {
                    id: d.id.clone(),
                    name: d.name.clone(),
                    is_default: d.is_default,
                    is_virtual: d.is_virtual,
                    enabled: self.settings.data.targets.contains(&d.id),
                    offset_ms: self.settings.data.offsets.get(&d.id).copied().unwrap_or(0.0),
                    side: side_of(&d.id),
                })
                .collect(),
            stereo: self.settings.data.stereo,
            swap: self.settings.data.swap,
            auto_sync: self.settings.data.auto_sync,
            sync_sound: self.settings.data.sync_sound.clone(),
            background_sync_min: self.settings.data.background_sync_min,
            unstable_speaker: self.unstable.as_deref().map(short),
            bluetooth_resetting: self.bluetooth_resetting,
            cable_renamed: self.preferred_virtual().is_some_and(|d| d.name.starts_with(devices::CABLE_NAME) && !d.name_missing),
            cable_prompt: self.preferred_virtual().is_some_and(|d| {
                d.name_missing || (!self.settings.data.cable_prompt_dismissed && !d.name.starts_with(devices::CABLE_NAME))
            }),
            last_sync: self.last_sync.clone(),
        }
    }

    fn preferred_virtual(&self) -> Option<&OutputDevice> {
        let virtuals = || self.devices.iter().filter(|d| d.is_virtual);
        virtuals()
            .find(|d| d.name.starts_with(devices::CABLE_NAME))
            .or_else(|| virtuals().find(|d| d.name.starts_with("CABLE Input")))
            .or_else(|| virtuals().next())
    }

    fn engine_handle(&self) -> Option<EngineHandle> {
        self.engine.as_ref().map(|e| e.handle())
    }

    fn glitches(&self) -> u64 {
        self.engine.as_ref().map(|e| e.handle().glitches()).unwrap_or(0)
    }

    fn device_names(&self) -> Vec<String> {
        self.devices.iter().map(|d| d.name.clone()).collect()
    }

    pub fn reset_offsets(&mut self) {
        self.settings.data.offsets.clear();
        if let Some(engine) = &self.engine {
            for id in engine.handle().target_ids() {
                engine.handle().set_extra(&id, 0.0);
            }
        }
    }
}

pub fn short(name: &str) -> String {
    name.trim_start_matches("Speakers (").trim_end_matches(')').to_string()
}

pub fn start_sync(app: AppHandle, controller: Shared, background: bool) -> Result<(), String> {
    let (handle, silent) = {
        let mut c = controller.lock().unwrap();
        if c.syncing {
            return Err("A sync is already running.".into());
        }
        let handle = c.engine.as_ref().map(|e| e.handle()).filter(|h| h.is_running());
        let Some(handle) = handle else {
            return Err("Start playing first (select the virtual output and pick speakers).".into());
        };
        if handle.target_ids().len() < 2 {
            return Err("Pick at least two speakers to sync.".into());
        }
        c.syncing = true;
        c.background_syncing = background;
        c.auto_sync_at = None;
        c.background_at = None;
        (handle, c.settings.data.sync_sound != "chime")
    };
    emit(&app, &controller);
    thread::spawn(move || {
        let _ = wasapi::initialize_mta();
        let mut result = sync::laptop_sync(&handle, silent);
        if silent && !background && handle.is_running() {
            if let Err(e) = &result {
                log::info!("silent sync failed ({e}); trying the chime");
                result = sync::laptop_sync(&handle, false);
            }
        }
        let message = {
            let mut c = controller.lock().unwrap();
            c.syncing = false;
            c.background_syncing = false;
            c.last_sync_at = Some(Instant::now());
            c.schedule_background();
            if background && result.is_err() && c.background_at.is_some() {
                c.background_at = Some(Instant::now() + FAILED_RETRY);
            }
            match &result {
                Ok(r) => {
                    for (id, ms) in &r.offsets {
                        c.settings.data.offsets.insert(id.clone(), *ms);
                    }
                    c.settings.save();
                    let names: Vec<String> = r
                        .offsets
                        .iter()
                        .filter(|(_, ms)| *ms >= 1.0)
                        .filter_map(|(id, ms)| {
                            c.devices.iter().find(|d| &d.id == id).map(|d| format!("{} +{:.0} ms", short(&d.name), ms))
                        })
                        .collect();
                    let detail = if names.is_empty() { "no delay needed".to_string() } else { names.join(", ") };
                    if background {
                        format!("Kept in sync in the background. Last fix: {:.0} ms; {detail}.", r.gap_ms)
                    } else {
                        format!("Synced. Gap was {:.0} ms; {detail}.", r.gap_ms)
                    }
                }
                Err(e) => e.clone(),
            }
        };
        let kind = if background { "background sync" } else { "sync" };
        match &result {
            Ok(r) => log::info!("{kind} ok: gap {:.1} ms, offsets {:?}", r.gap_ms, r.offsets.iter().map(|(_, ms)| *ms).collect::<Vec<_>>()),
            Err(e) => log::warn!("{kind} failed: {e}"),
        }
        if background {
            if result.is_ok() {
                controller.lock().unwrap().last_sync = Some(message);
            }
            emit(&app, &controller);
            return;
        }
        controller.lock().unwrap().last_sync = Some(message.clone());
        let title = if result.is_ok() { "Speakers synced" } else { "Sync failed" };
        let _ = app.notification().builder().title(title).body(&message).show();
        emit(&app, &controller);
    });
    Ok(())
}

pub fn start_bluetooth_reset(app: AppHandle, controller: Shared) -> Result<(), String> {
    controller.lock().unwrap().begin_bluetooth_reset()?;
    emit(&app, &controller);
    thread::spawn(move || {
        log::info!("bluetooth reset starting");
        let outcome = bluetooth::reset();
        controller.lock().unwrap().finish_bluetooth_reset();
        let (title, body) = match &outcome {
            Outcome::Done => {
                log::info!("bluetooth reset done");
                ("Bluetooth reset", "Your speakers will reconnect in a few seconds.")
            }
            Outcome::Cancelled => {
                log::info!("bluetooth reset cancelled at the permission prompt");
                emit(&app, &controller);
                return;
            }
            Outcome::NoAdapter => {
                log::error!("bluetooth reset: adapter not found");
                ("Couldn't reset Bluetooth", "Windows didn't list a Bluetooth adapter.")
            }
            Outcome::InUse => {
                log::error!("bluetooth reset: adapter would not switch off");
                ("Couldn't reset Bluetooth", "Windows wouldn't release the adapter. Close Galaxy Buds and try again.")
            }
            Outcome::NotBack => {
                log::error!("bluetooth reset: adapter did not switch back on");
                ("Bluetooth is off", "Turn Bluetooth back on in Windows settings.")
            }
            Outcome::Failed(e) => {
                log::error!("bluetooth reset failed: {e}");
                ("Couldn't reset Bluetooth", "Something went wrong. Details are in the log.")
            }
        };
        let _ = app.notification().builder().title(title).body(body).show();
        emit(&app, &controller);
    });
    Ok(())
}

pub fn run_meters(app: AppHandle, controller: Shared) {
    thread::spawn(move || loop {
        thread::sleep(METER_INTERVAL);
        let visible = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false);
        if !visible {
            continue;
        }
        let handle = controller.lock().unwrap().engine_handle();
        let view = match handle {
            Some(handle) => {
                let (spectrum, levels) = handle.meters();
                MetersView { spectrum, levels: levels.into_iter().collect() }
            }
            None => MetersView::default(),
        };
        let _ = app.emit("meters", view);
    });
}

pub fn emit(app: &AppHandle, controller: &Shared) {
    let view = controller.lock().unwrap().view();
    let _ = app.emit("state", view);
}

pub fn run_supervisor(app: AppHandle, controller: Shared) {
    thread::spawn(move || {
        let _ = wasapi::initialize_mta();
        let mut last: Option<StateView> = None;
        let mut last_status = String::new();
        let mut last_glitches = 0u64;
        let mut last_devices: Vec<String> = Vec::new();
        loop {
            let (view, sync_due, background_due, glitches, names, unstable) = {
                let mut c = controller.lock().unwrap();
                c.tick();
                let now = Instant::now();
                let due = c.auto_sync_at.is_some_and(|at| now >= at) && !c.syncing;
                if due {
                    c.auto_sync_at = None;
                }
                let background_due = !due && !c.syncing && c.background_at.is_some_and(|at| now >= at);
                let glitches = c.glitches();
                let unstable = c.track_glitches();
                if glitches > last_glitches {
                    c.schedule_after_glitch();
                }
                (c.view(), due, background_due, glitches, c.device_names(), unstable)
            };
            if let Some(name) = unstable {
                let name = short(&name);
                log::warn!("{name} keeps dropping out; pausing glitch re-syncs until it reconnects");
                let _ = app
                    .notification()
                    .builder()
                    .title(format!("{name} keeps dropping out"))
                    .body("Its Bluetooth link is unstable, which causes the crackle. Open Duet and press Reset Bluetooth, or move the speaker closer.")
                    .show();
            }
            if view.status != last_status {
                log::info!("status: {}", view.status);
                last_status = view.status.clone();
            }
            if names != last_devices {
                log::debug!("output devices: {}", names.join(" | "));
                last_devices = names;
            }
            if glitches > last_glitches {
                log::debug!("glitches: {} new (total {glitches} this session)", glitches - last_glitches);
            }
            last_glitches = glitches;
            if sync_due {
                log::info!("auto-sync starting");
                let _ = start_sync(app.clone(), controller.clone(), false);
            } else if background_due {
                if let Some(app_name) = mic_use::other_app_using_microphone() {
                    log::debug!("background sync postponed: microphone in use by {app_name}");
                    controller.lock().unwrap().background_at = Some(Instant::now() + BUSY_MIC_RETRY);
                } else {
                    log::debug!("background sync starting");
                    let _ = start_sync(app.clone(), controller.clone(), true);
                }
            }
            if last.as_ref() != Some(&view) {
                let _ = app.emit("state", view.clone());
                last = Some(view);
            }
            thread::sleep(POLL);
        }
    });
}
