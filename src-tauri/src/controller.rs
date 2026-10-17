use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

use crate::devices::{self, OutputDevice};
use crate::engine::{Engine, TargetSpec};
use crate::settings::SettingsStore;
use crate::sync;

const POLL: Duration = Duration::from_millis(500);
const RETRY_DELAY: Duration = Duration::from_secs(3);
const AUTO_SYNC_DELAY: Duration = Duration::from_secs(8);
const FIRST_RUN_MATCH: &str = "Echo Dot";

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
    pub last_sync: Option<String>,
    pub glitches: u64,
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
            retry_at: Instant::now(),
        }
    }

    fn wanted_targets(&self) -> Vec<OutputDevice> {
        let mut wanted: Vec<OutputDevice> = self
            .devices
            .iter()
            .filter(|d| !d.is_virtual && self.settings.data.targets.contains(&d.id))
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
    }

    fn tick(&mut self) {
        match devices::list_outputs() {
            Ok(list) => self.devices = list,
            Err(e) => {
                self.status = format!("Couldn't read audio devices ({e})");
                return;
            }
        }
        if !self.settings.data.initialized {
            self.settings.data.targets = self
                .devices
                .iter()
                .filter(|d| !d.is_virtual && d.name.contains(FIRST_RUN_MATCH))
                .map(|d| d.id.clone())
                .collect();
            self.settings.data.initialized = true;
            self.settings.save();
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
            let name = self.devices.iter().find(|d| d.is_virtual).map(|d| d.name.clone());
            self.status = match name {
                Some(name) => format!("Idle. Select \"{name}\" as your Windows sound output to start."),
                None => "VB-Audio Virtual Cable isn't installed.".into(),
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
            status: if self.syncing { "Syncing speakers: stay quiet for a few seconds...".into() } else { self.status.clone() },
            running: handle.as_ref().is_some_and(|h| h.is_running()),
            syncing: self.syncing,
            virtual_selected: self.devices.iter().any(|d| d.is_default && d.is_virtual),
            virtual_name: self.devices.iter().find(|d| d.is_virtual).map(|d| d.name.clone()),
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
            last_sync: self.last_sync.clone(),
            glitches: handle.map(|h| h.glitches()).unwrap_or(0),
        }
    }
}

pub fn short(name: &str) -> String {
    name.trim_start_matches("Speakers (").trim_end_matches(')').to_string()
}

pub fn start_sync(app: AppHandle, controller: Shared) -> Result<(), String> {
    let handle = {
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
        c.auto_sync_at = None;
        handle
    };
    emit(&app, &controller);
    thread::spawn(move || {
        let _ = wasapi::initialize_mta();
        let result = sync::laptop_sync(&handle);
        let message = {
            let mut c = controller.lock().unwrap();
            c.syncing = false;
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
                    format!("Synced. Gap was {:.0} ms; {detail}.", r.gap_ms)
                }
                Err(e) => e.clone(),
            }
        };
        controller.lock().unwrap().last_sync = Some(message.clone());
        let title = if result.is_ok() { "Speakers synced" } else { "Sync failed" };
        let _ = app.notification().builder().title(title).body(&message).show();
        emit(&app, &controller);
    });
    Ok(())
}

pub fn emit(app: &AppHandle, controller: &Shared) {
    let view = controller.lock().unwrap().view();
    let _ = app.emit("state", view);
}

pub fn run_supervisor(app: AppHandle, controller: Shared) {
    thread::spawn(move || {
        let _ = wasapi::initialize_mta();
        let mut last: Option<StateView> = None;
        loop {
            let (view, sync_due) = {
                let mut c = controller.lock().unwrap();
                c.tick();
                let due = c.auto_sync_at.is_some_and(|at| Instant::now() >= at) && !c.syncing;
                if due {
                    c.auto_sync_at = None;
                }
                (c.view(), due)
            };
            if sync_due {
                let _ = start_sync(app.clone(), controller.clone());
            }
            if last.as_ref() != Some(&view) {
                let _ = app.emit("state", view.clone());
                last = Some(view);
            }
            thread::sleep(POLL);
        }
    });
}
