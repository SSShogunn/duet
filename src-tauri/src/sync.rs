use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use wasapi::{AudioClientProperties, DeviceEnumerator, Direction, SampleType, StreamMode, StreamOption, WaveFormat};

use crate::devices;
use crate::dsp;
use crate::engine::{EngineHandle, SOURCE_RATE};

const MIC_WARMUP_MS: u64 = 400;
const TAIL_MS: u64 = 1000;

pub struct SyncResult {
    pub offsets: Vec<(String, f32)>,
    pub gap_ms: f32,
}

fn record(mic_id: String, stop: Arc<AtomicBool>, started: Arc<AtomicBool>) -> Result<Vec<f32>, String> {
    let _ = wasapi::initialize_mta();
    let run = || -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let device = DeviceEnumerator::new()?.get_device(&mic_id)?;
        let mut client = device.get_iaudioclient()?;
        let _ = client.set_properties(AudioClientProperties::new().set_option(StreamOption::Raw));
        let format = WaveFormat::new(32, 32, &SampleType::Float, SOURCE_RATE as usize, 2, None);
        let (_, min_period) = client.get_device_period()?;
        let mode = StreamMode::PollingShared {
            autoconvert: true,
            buffer_duration_hns: min_period * 10,
        };
        client.initialize_client(&format, &Direction::Capture, &mode)?;
        let capture = client.get_audiocaptureclient()?;
        let mut bytes = VecDeque::new();
        client.start_stream()?;
        started.store(true, Ordering::SeqCst);
        while !stop.load(Ordering::SeqCst) {
            while capture.get_next_packet_size()?.unwrap_or(0) > 0 {
                let before = bytes.len();
                let info = capture.read_from_device_to_deque(&mut bytes)?;
                if info.flags.silent {
                    bytes.range_mut(before..).for_each(|b| *b = 0);
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
        client.stop_stream()?;
        let raw: Vec<u8> = bytes.into_iter().collect();
        Ok(raw
            .chunks_exact(8)
            .map(|f| {
                let l = f32::from_le_bytes([f[0], f[1], f[2], f[3]]);
                let r = f32::from_le_bytes([f[4], f[5], f[6], f[7]]);
                (l + r) / 2.0
            })
            .collect())
    };
    let result = run().map_err(|e| format!("Couldn't use the microphone ({e})"));
    started.store(true, Ordering::SeqCst);
    result
}

pub fn laptop_sync(handle: &EngineHandle) -> Result<SyncResult, String> {
    let ids = handle.target_ids();
    if ids.len() < 2 {
        return Err("Pick at least two speakers to sync.".into());
    }
    let mic_id = devices::default_input_id().ok_or("No microphone found.")?;
    let stop = Arc::new(AtomicBool::new(false));
    let started = Arc::new(AtomicBool::new(false));
    let recorder = {
        let (stop, started) = (stop.clone(), started.clone());
        thread::spawn(move || record(mic_id, stop, started))
    };
    while !started.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(10));
    }
    thread::sleep(Duration::from_millis(MIC_WARMUP_MS));

    let pattern = dsp::build_pattern(ids.len());
    let signals = (0..ids.len()).map(|k| dsp::render_pattern(&pattern, SOURCE_RATE, k)).collect();
    handle.play_pattern(signals);
    let deadline = Instant::now() + Duration::from_secs_f32(pattern.length_s + 3.0);
    while !handle.pattern_done() && Instant::now() < deadline && handle.is_running() {
        thread::sleep(Duration::from_millis(20));
    }
    thread::sleep(Duration::from_millis(TAIL_MS));
    stop.store(true, Ordering::SeqCst);
    let recording = recorder.join().map_err(|_| "Microphone thread crashed.".to_string())?;
    handle.end_calibration();
    let recording = recording?;
    if !handle.is_running() {
        return Err("Speakers changed during the sync. Try again.".into());
    }

    let lateness = dsp::measure(&recording, SOURCE_RATE, &pattern)?;
    let latest = lateness.iter().fold(f32::MIN, |m, v| m.max(*v));
    let earliest = lateness.iter().fold(f32::MAX, |m, v| m.min(*v));
    let wanted: Vec<f32> = ids
        .iter()
        .zip(&lateness)
        .map(|(id, late)| handle.extra(id) + latest - late)
        .collect();
    let floor = wanted.iter().fold(f32::MAX, |m, v| m.min(*v));
    let offsets: Vec<(String, f32)> = ids
        .into_iter()
        .zip(wanted)
        .map(|(id, ms)| ((id), ((ms - floor) * 10.0).round() / 10.0))
        .collect();
    for (id, ms) in &offsets {
        handle.set_extra(id, *ms);
    }
    Ok(SyncResult {
        offsets,
        gap_ms: latest - earliest,
    })
}

#[cfg(test)]
mod hardware {
    use super::*;
    use crate::engine::{Engine, TargetSpec};

    #[test]
    #[ignore]
    fn engine_and_sync_on_real_speakers() {
        let _ = wasapi::initialize_mta();
        let outputs = devices::list_outputs().expect("devices");
        let source = outputs.iter().find(|d| d.is_virtual && d.is_default).expect("virtual output must be default");
        let targets: Vec<TargetSpec> = outputs
            .iter()
            .filter(|d| !d.is_virtual && d.name.contains("Echo Dot"))
            .map(|d| TargetSpec { id: d.id.clone(), extra_ms: 0.0, channel: None })
            .collect();
        println!("source: {}, targets: {}", source.name, targets.len());
        let engine = Engine::start(source.id.clone(), targets);
        thread::sleep(Duration::from_secs(4));
        let handle = engine.handle();
        println!("running after 4 s: {}  error: {:?}  glitches: {}", handle.is_running(), engine.error(), handle.glitches());
        for run in 1..=3 {
            let started = Instant::now();
            match laptop_sync(&handle) {
                Ok(r) => println!("sync {run} ({:.1}s): gap {:.1} ms, offsets {:?}", started.elapsed().as_secs_f32(), r.gap_ms, r.offsets.iter().map(|(_, ms)| *ms).collect::<Vec<_>>()),
                Err(e) => println!("sync {run} failed: {e}"),
            }
            for id in handle.target_ids() {
                handle.set_extra(&id, 0.0);
            }
            thread::sleep(Duration::from_secs(1));
        }
        println!("glitches at end: {}", handle.glitches());
        assert!(handle.is_running(), "engine stopped: {:?}", engine.error());
        engine.stop();
    }
}
