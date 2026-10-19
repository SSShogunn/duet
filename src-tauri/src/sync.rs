use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use wasapi::{AudioClientProperties, DeviceEnumerator, Direction, SampleType, StreamMode, StreamOption, WaveFormat};

use crate::devices;
use crate::dsp;
use crate::errors;
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
    let result = run().map_err(|e| {
        let raw = e.to_string();
        let message = if raw.to_ascii_lowercase().contains("0x80070005") {
            "Microphone access is off. Turn it on in Settings > Privacy & security > Microphone."
        } else {
            "Couldn't use the microphone. Check it's on and not busy in another app."
        };
        errors::report("sync microphone", raw, message)
    });
    started.store(true, Ordering::SeqCst);
    result
}

fn measure_raw_lateness(handle: &EngineHandle, speakers: usize, silent: bool) -> Result<Vec<f32>, String> {
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

    let pattern = dsp::build_pattern(speakers, silent);
    let signals = (0..speakers).map(|k| dsp::render_pattern(&pattern, SOURCE_RATE, k)).collect();
    let over_music = handle.play_pattern(signals, silent);
    log::debug!("sync signal playing {}", if over_music { "over the music" } else { "with music paused" });
    let deadline = Instant::now() + Duration::from_secs_f32(pattern.length_s + 3.0);
    while !handle.pattern_done() && Instant::now() < deadline && handle.is_running() {
        thread::sleep(Duration::from_millis(20));
    }
    thread::sleep(Duration::from_millis(TAIL_MS));
    stop.store(true, Ordering::SeqCst);
    let recording = recorder
        .join()
        .map_err(|_| errors::report("sync microphone", "recording thread panicked", "Couldn't use the microphone. Try again."))?;
    handle.end_calibration();
    let recording = recording?;
    if !handle.is_running() {
        return Err("Speakers changed during the sync. Try again.".into());
    }

    log::debug!("sync recording: {} samples", recording.len());
    let lateness = dsp::measure(&recording, SOURCE_RATE, &pattern)?;
    log::debug!("sync lateness per speaker (ms): {lateness:?}");
    Ok(lateness)
}

pub fn laptop_sync(handle: &EngineHandle, silent: bool) -> Result<SyncResult, String> {
    let ids = handle.target_ids();
    if ids.len() < 2 {
        return Err("Pick at least two speakers to sync.".into());
    }
    let previous: Vec<f32> = ids.iter().map(|id| handle.extra(id)).collect();
    for id in &ids {
        handle.set_extra(id, 0.0);
    }
    let lateness = match measure_raw_lateness(handle, ids.len(), silent) {
        Ok(lateness) => lateness,
        Err(e) => {
            for (id, ms) in ids.iter().zip(&previous) {
                handle.set_extra(id, *ms);
            }
            return Err(e);
        }
    };
    let latest = lateness.iter().fold(f32::MIN, |m, v| m.max(*v));
    let earliest = lateness.iter().fold(f32::MAX, |m, v| m.min(*v));
    let offsets: Vec<(String, f32)> = ids
        .into_iter()
        .zip(&lateness)
        .map(|(id, late)| (id, ((latest - late) * 10.0).round() / 10.0))
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
            .map(|d| TargetSpec { id: d.id.clone(), name: d.name.clone(), extra_ms: 0.0, channel: None })
            .collect();
        println!("source: {}, targets: {}", source.name, targets.len());
        let engine = Engine::start(source.id.clone(), targets);
        thread::sleep(Duration::from_secs(4));
        let handle = engine.handle();
        println!("running after 4 s: {}  error: {:?}  glitches: {}", handle.is_running(), engine.error(), handle.glitches());
        for run in 1..=6 {
            let started = Instant::now();
            match laptop_sync(&handle, run % 2 == 1) {
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

    const TEST_TONES: [f32; 10] = [1000.0, 8000.0, 12000.0, 14000.0, 16000.0, 17000.0, 18000.0, 19000.0, 20000.0, 21000.0];

    fn tone_ladder(rate: f32) -> Vec<f32> {
        let tone = (rate * 0.3) as usize;
        let gap = (rate * 0.1) as usize;
        let ramp = (rate * 0.01) as usize;
        let mut out = vec![0.0; gap];
        for f in TEST_TONES {
            for i in 0..tone {
                let edge = (i.min(tone - 1 - i) as f32 / ramp as f32).min(1.0);
                out.push(0.15 * edge * (2.0 * std::f32::consts::PI * f * i as f32 / rate).sin());
            }
            out.extend(std::iter::repeat_n(0.0, gap));
        }
        out
    }

    fn goertzel(x: &[f32], rate: f32, f: f32) -> f32 {
        let c = 2.0 * (2.0 * std::f32::consts::PI * f / rate).cos();
        let (mut s1, mut s2) = (0f32, 0f32);
        for &v in x {
            let s = v + c * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        (s1 * s1 + s2 * s2 - c * s1 * s2).max(1e-20)
    }

    fn tone_snr_db(recording: &[f32], rate: f32, f: f32) -> f32 {
        let window = (rate * 0.1) as usize;
        let mut powers: Vec<f32> = recording.windows(window).step_by(window / 2).map(|w| goertzel(w, rate, f)).collect();
        powers.sort_by(|a, b| a.total_cmp(b));
        let peak = powers.last().copied().unwrap_or(1e-20);
        let floor = powers.get(powers.len() / 2).copied().unwrap_or(1e-20);
        10.0 * (peak / floor).log10()
    }

    #[test]
    #[ignore]
    fn high_frequency_response() {
        let _ = wasapi::initialize_mta();
        let outputs = devices::list_outputs().expect("devices");
        let source = outputs.iter().find(|d| d.is_virtual && d.is_default).expect("virtual output must be default");
        let targets: Vec<TargetSpec> = outputs
            .iter()
            .filter(|d| !d.is_virtual && d.name.contains("Echo Dot"))
            .map(|d| TargetSpec { id: d.id.clone(), name: d.name.clone(), extra_ms: 0.0, channel: None })
            .collect();
        let names: Vec<String> = targets.iter().map(|t| t.name.clone()).collect();
        let engine = Engine::start(source.id.clone(), targets);
        thread::sleep(Duration::from_secs(3));
        let handle = engine.handle();
        let mic_id = devices::default_input_id().expect("microphone");
        let rate = SOURCE_RATE as f32;
        let tones = tone_ladder(rate);
        for (k, name) in names.iter().enumerate() {
            let signals = (0..names.len()).map(|i| if i == k { tones.clone() } else { vec![0.0; tones.len()] }).collect();
            let stop = Arc::new(AtomicBool::new(false));
            let started = Arc::new(AtomicBool::new(false));
            let recorder = {
                let (stop, started, mic) = (stop.clone(), started.clone(), mic_id.clone());
                thread::spawn(move || record(mic, stop, started))
            };
            while !started.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(10));
            }
            thread::sleep(Duration::from_millis(MIC_WARMUP_MS));
            handle.play_pattern(signals, false);
            while !handle.pattern_done() {
                thread::sleep(Duration::from_millis(20));
            }
            thread::sleep(Duration::from_millis(TAIL_MS));
            stop.store(true, Ordering::SeqCst);
            let recording = recorder.join().expect("recorder").expect("recording");
            handle.end_calibration();
            println!("\n{name}  (signal above background, dB)");
            for f in TEST_TONES {
                let snr = tone_snr_db(&recording, rate, f);
                let bar = "#".repeat((snr.max(0.0) / 2.0) as usize);
                println!("  {:>6.0} Hz  {:>5.1}  {bar}", f, snr);
            }
            thread::sleep(Duration::from_secs(1));
        }
        engine.stop();
    }
}
