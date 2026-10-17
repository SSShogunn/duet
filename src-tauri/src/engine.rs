use std::collections::VecDeque;
use std::error::Error;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

use crate::devices::EndpointVolume;

pub const SOURCE_RATE: u32 = 48_000;
const CHANNELS: usize = 2;
const FRAME_BYTES: usize = 4 * CHANNELS;
const START_LATENCY_MS: f32 = 80.0;
const MAX_LATENCY_MS: f32 = 400.0;
const LATENCY_STEP_MS: f32 = 20.0;
const MAX_SPEED_ADJUST: f64 = 0.005;
const SKIP_MARGIN_MS: f32 = 60.0;
const FADE_FRAMES: usize = 240;
const CAPTURE_RECENT_MS: u64 = 50;
const PUMP_CHUNK_MS: u64 = 10;
const VOLUME_POLL_MS: u64 = 50;

type Res<T> = Result<T, Box<dyn Error>>;

fn stream_format() -> WaveFormat {
    WaveFormat::new(32, 32, &SampleType::Float, SOURCE_RATE as usize, CHANNELS, None)
}

fn load_f32(value: &AtomicU32) -> f32 {
    f32::from_bits(value.load(Ordering::Relaxed))
}

fn store_f32(value: &AtomicU32, v: f32) {
    value.store(v.to_bits(), Ordering::Relaxed);
}

pub struct TargetSpec {
    pub id: String,
    pub extra_ms: f32,
    pub channel: Option<usize>,
}

struct Target {
    id: String,
    queue: Mutex<VecDeque<[f32; 2]>>,
    extra_ms: AtomicU32,
    channel: AtomicI32,
    glitches: AtomicU64,
}

struct Injection {
    signals: Vec<Vec<f32>>,
    pos: usize,
}

struct Shared {
    running: AtomicBool,
    calibrating: AtomicBool,
    pumping: AtomicBool,
    gain: AtomicU32,
    latency_ms: AtomicU32,
    last_capture_ms: AtomicU64,
    started: Instant,
    targets: Vec<Arc<Target>>,
    injection: Mutex<Option<Injection>>,
    error: Mutex<Option<String>>,
}

impl Shared {
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn capture_recent(&self) -> bool {
        self.now_ms().saturating_sub(self.last_capture_ms.load(Ordering::Relaxed)) < CAPTURE_RECENT_MS
    }

    fn fail(&self, message: String) {
        if let Ok(mut error) = self.error.lock() {
            error.get_or_insert(message);
        }
        self.running.store(false, Ordering::SeqCst);
    }
}

#[derive(Clone)]
pub struct EngineHandle(Arc<Shared>);

impl EngineHandle {
    pub fn target_ids(&self) -> Vec<String> {
        self.0.targets.iter().map(|t| t.id.clone()).collect()
    }

    pub fn extra(&self, id: &str) -> f32 {
        self.0.targets.iter().find(|t| t.id == id).map(|t| load_f32(&t.extra_ms)).unwrap_or(0.0)
    }

    pub fn set_extra(&self, id: &str, ms: f32) {
        if let Some(t) = self.0.targets.iter().find(|t| t.id == id) {
            store_f32(&t.extra_ms, ms.max(0.0));
        }
    }

    pub fn set_channel(&self, id: &str, channel: Option<usize>) {
        if let Some(t) = self.0.targets.iter().find(|t| t.id == id) {
            t.channel.store(channel.map(|c| c as i32).unwrap_or(-1), Ordering::Relaxed);
        }
    }

    pub fn is_running(&self) -> bool {
        self.0.running.load(Ordering::SeqCst)
    }

    pub fn glitches(&self) -> u64 {
        self.0.targets.iter().map(|t| t.glitches.load(Ordering::Relaxed)).sum()
    }

    pub fn play_pattern(&self, signals: Vec<Vec<f32>>) {
        *self.0.injection.lock().unwrap() = Some(Injection { signals, pos: 0 });
        self.0.calibrating.store(true, Ordering::SeqCst);
        if !self.0.pumping.swap(true, Ordering::SeqCst) {
            let shared = self.0.clone();
            thread::spawn(move || pump_loop(&shared));
        }
    }

    pub fn pattern_done(&self) -> bool {
        self.0.injection.lock().map(|i| i.is_none()).unwrap_or(true)
    }

    pub fn end_calibration(&self) {
        *self.0.injection.lock().unwrap() = None;
        self.0.calibrating.store(false, Ordering::SeqCst);
    }
}

pub struct Engine {
    handle: EngineHandle,
    threads: Vec<JoinHandle<()>>,
}

impl Engine {
    pub fn start(source_id: String, specs: Vec<TargetSpec>) -> Engine {
        let targets: Vec<Arc<Target>> = specs
            .into_iter()
            .map(|spec| {
                Arc::new(Target {
                    id: spec.id,
                    queue: Mutex::new(VecDeque::new()),
                    extra_ms: AtomicU32::new(spec.extra_ms.max(0.0).to_bits()),
                    channel: AtomicI32::new(spec.channel.map(|c| c as i32).unwrap_or(-1)),
                    glitches: AtomicU64::new(0),
                })
            })
            .collect();
        let shared = Arc::new(Shared {
            running: AtomicBool::new(true),
            calibrating: AtomicBool::new(false),
            pumping: AtomicBool::new(false),
            gain: AtomicU32::new(1f32.to_bits()),
            latency_ms: AtomicU32::new(START_LATENCY_MS.to_bits()),
            last_capture_ms: AtomicU64::new(0),
            started: Instant::now(),
            targets,
            injection: Mutex::new(None),
            error: Mutex::new(None),
        });

        let mut threads = Vec::new();
        for target in shared.targets.clone() {
            let s = shared.clone();
            threads.push(spawn_audio_thread(s, move |shared| playback_loop(shared, &target)));
        }
        let source = source_id.clone();
        threads.push(spawn_audio_thread(shared.clone(), move |shared| capture_loop(shared, &source)));
        threads.push(spawn_audio_thread(shared.clone(), move |shared| volume_loop(shared, &source_id)));
        Engine { handle: EngineHandle(shared), threads }
    }

    pub fn handle(&self) -> EngineHandle {
        self.handle.clone()
    }

    pub fn error(&self) -> Option<String> {
        self.handle.0.error.lock().ok().and_then(|e| e.clone())
    }

    pub fn stop(self) {
        self.handle.0.running.store(false, Ordering::SeqCst);
        for thread in self.threads {
            let _ = thread.join();
        }
    }
}

fn spawn_audio_thread<F>(shared: Arc<Shared>, work: F) -> JoinHandle<()>
where
    F: FnOnce(&Shared) -> Res<()> + Send + 'static,
{
    thread::spawn(move || {
        let _ = wasapi::initialize_mta();
        if let Err(error) = work(&shared) {
            shared.fail(error.to_string());
        }
    })
}

fn push_all(shared: &Shared, frames: &[[f32; 2]]) {
    for target in &shared.targets {
        if let Ok(mut queue) = target.queue.lock() {
            queue.extend(frames.iter().copied());
        }
    }
}

fn capture_loop(shared: &Shared, source_id: &str) -> Res<()> {
    let device = DeviceEnumerator::new()?.get_device(source_id)?;
    let mut client = device.get_iaudioclient()?;
    let (_, min_period) = client.get_device_period()?;
    let mode = StreamMode::PollingShared {
        autoconvert: true,
        buffer_duration_hns: min_period * 10,
    };
    client.initialize_client(&stream_format(), &Direction::Capture, &mode)?;
    let capture = client.get_audiocaptureclient()?;
    let mut bytes = VecDeque::new();
    client.start_stream()?;

    while shared.running.load(Ordering::SeqCst) {
        while capture.get_next_packet_size()?.unwrap_or(0) > 0 {
            let before = bytes.len();
            let info = capture.read_from_device_to_deque(&mut bytes)?;
            if info.flags.silent {
                bytes.range_mut(before..).for_each(|b| *b = 0);
            }
        }
        if !bytes.is_empty() {
            let usable = bytes.len() - bytes.len() % FRAME_BYTES;
            let raw: Vec<u8> = bytes.drain(..usable).collect();
            if !shared.calibrating.load(Ordering::SeqCst) {
                let frames: Vec<[f32; 2]> = raw
                    .chunks_exact(FRAME_BYTES)
                    .map(|f| {
                        [
                            f32::from_le_bytes([f[0], f[1], f[2], f[3]]),
                            f32::from_le_bytes([f[4], f[5], f[6], f[7]]),
                        ]
                    })
                    .collect();
                shared.last_capture_ms.store(shared.now_ms(), Ordering::Relaxed);
                push_all(shared, &frames);
            }
        }
        thread::sleep(Duration::from_millis(3));
    }
    client.stop_stream()?;
    Ok(())
}

fn pump_loop(shared: &Shared) {
    let chunk = (SOURCE_RATE as u64 * PUMP_CHUNK_MS / 1000) as usize;
    let mut next = Instant::now();
    while shared.calibrating.load(Ordering::SeqCst) && shared.running.load(Ordering::SeqCst) {
        {
            let mut injection = shared.injection.lock().unwrap();
            for (k, target) in shared.targets.iter().enumerate() {
                let mut frames = vec![[0f32; 2]; chunk];
                if let Some(inj) = injection.as_ref() {
                    if let Some(signal) = inj.signals.get(k) {
                        for (i, frame) in frames.iter_mut().enumerate() {
                            let v = signal.get(inj.pos + i).copied().unwrap_or(0.0);
                            *frame = [v, v];
                        }
                    }
                }
                if let Ok(mut queue) = target.queue.lock() {
                    queue.extend(frames);
                }
            }
            if let Some(inj) = injection.as_mut() {
                inj.pos += chunk;
                if inj.signals.iter().all(|s| inj.pos >= s.len()) {
                    *injection = None;
                }
            }
        }
        shared.last_capture_ms.store(shared.now_ms(), Ordering::Relaxed);
        next += Duration::from_millis(PUMP_CHUNK_MS);
        let now = Instant::now();
        if next > now {
            thread::sleep(next - now);
        } else if now - next > Duration::from_millis(200) {
            next = now;
        }
    }
    shared.pumping.store(false, Ordering::SeqCst);
}

fn volume_loop(shared: &Shared, source_id: &str) -> Res<()> {
    let mut volume = None;
    while shared.running.load(Ordering::SeqCst) {
        if volume.is_none() {
            volume = EndpointVolume::open(source_id).ok();
        }
        if let Some(v) = &volume {
            match v.gain() {
                Ok(gain) => store_f32(&shared.gain, gain),
                Err(_) => volume = None,
            }
        }
        thread::sleep(Duration::from_millis(VOLUME_POLL_MS));
    }
    Ok(())
}

struct PlayState {
    pos: f64,
    priming: bool,
    fill_ms: f32,
    last_gain: f32,
    applied_extra_frames: i64,
}

fn render(shared: &Shared, target: &Target, frames: usize, state: &mut PlayState) -> Vec<f32> {
    let mut out = vec![0f32; frames * CHANNELS];
    let rate = SOURCE_RATE as f32;
    let extra_ms = load_f32(&target.extra_ms);
    let latency = load_f32(&shared.latency_ms) + extra_ms;
    let mut fade_in = false;
    {
        let mut queue = target.queue.lock().unwrap();
        let wanted_extra = (extra_ms * rate / 1000.0) as i64;
        let shift = wanted_extra - state.applied_extra_frames;
        if shift > 0 {
            for _ in 0..shift {
                queue.push_front([0.0, 0.0]);
            }
        } else if shift < 0 {
            let drop = (-shift as usize).min(queue.len());
            queue.drain(..drop);
            state.pos = 0.0;
        }
        state.applied_extra_frames = wanted_extra;

        let mut avail_ms = ((queue.len() as f64 - state.pos) * 1000.0 / rate as f64) as f32;
        if state.priming {
            if avail_ms < latency {
                return out;
            }
            state.priming = false;
            fade_in = true;
        }
        if fade_in || avail_ms > latency + SKIP_MARGIN_MS {
            if !fade_in {
                target.glitches.fetch_add(1, Ordering::Relaxed);
            }
            let skip = (((avail_ms - latency) * rate / 1000.0) as usize).min(queue.len());
            queue.drain(..skip);
            avail_ms = ((queue.len() as f64 - state.pos) * 1000.0 / rate as f64) as f32;
            state.fill_ms = avail_ms;
        }
        state.fill_ms += 0.02 * (avail_ms - state.fill_ms);
        let adjust = ((state.fill_ms - latency) as f64 * 0.0001).clamp(-MAX_SPEED_ADJUST, MAX_SPEED_ADJUST);
        let ratio = 1.0 + adjust;

        if state.pos + frames as f64 * ratio + 2.0 > queue.len() as f64 {
            if shared.capture_recent() {
                target.glitches.fetch_add(1, Ordering::Relaxed);
                let bumped = (load_f32(&shared.latency_ms) + LATENCY_STEP_MS).min(MAX_LATENCY_MS);
                store_f32(&shared.latency_ms, bumped);
            }
            state.priming = true;
            return out;
        }
        for i in 0..frames {
            let x = state.pos + i as f64 * ratio;
            let i0 = x as usize;
            let frac = (x - i0 as f64) as f32;
            let a = queue[i0];
            let b = queue[i0 + 1];
            out[i * 2] = a[0] + (b[0] - a[0]) * frac;
            out[i * 2 + 1] = a[1] + (b[1] - a[1]) * frac;
        }
        let new_pos = state.pos + frames as f64 * ratio;
        let drop = new_pos as usize;
        queue.drain(..drop);
        state.pos = new_pos - drop as f64;
    }

    if fade_in {
        for i in 0..frames.min(FADE_FRAMES) {
            let g = i as f32 / FADE_FRAMES as f32;
            out[i * 2] *= g;
            out[i * 2 + 1] *= g;
        }
    }
    let gain = if shared.calibrating.load(Ordering::SeqCst) { 1.0 } else { load_f32(&shared.gain) };
    if gain != 1.0 || state.last_gain != 1.0 {
        for i in 0..frames {
            let g = state.last_gain + (gain - state.last_gain) * (i + 1) as f32 / frames as f32;
            out[i * 2] *= g;
            out[i * 2 + 1] *= g;
        }
    }
    state.last_gain = gain;
    let channel = target.channel.load(Ordering::Relaxed);
    if channel >= 0 {
        let c = channel as usize;
        for i in 0..frames {
            let v = out[i * 2 + c];
            out[i * 2] = v;
            out[i * 2 + 1] = v;
        }
    }
    out
}

fn playback_loop(shared: &Shared, target: &Target) -> Res<()> {
    let device = DeviceEnumerator::new()?.get_device(&target.id)?;
    let mut client = device.get_iaudioclient()?;
    let (default_period, _) = client.get_device_period()?;
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: default_period,
    };
    client.initialize_client(&stream_format(), &Direction::Render, &mode)?;
    let event = client.set_get_eventhandle()?;
    let render_client = client.get_audiorenderclient()?;
    let mut state = PlayState {
        pos: 0.0,
        priming: true,
        fill_ms: 0.0,
        last_gain: load_f32(&shared.gain),
        applied_extra_frames: (load_f32(&target.extra_ms) * SOURCE_RATE as f32 / 1000.0) as i64,
    };
    client.start_stream()?;

    while shared.running.load(Ordering::SeqCst) {
        let frames = client.get_available_space_in_frames()? as usize;
        if frames > 0 {
            let samples = render(shared, target, frames, &mut state);
            let bytes: Vec<u8> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
            render_client.write_to_device(frames, &bytes, None)?;
        }
        if event.wait_for_event(1000).is_err() {
            return Err("speaker stopped responding".into());
        }
    }
    client.stop_stream()?;
    Ok(())
}
