use std::f32::consts::PI;

use rustfft::num_complex::Complex32;
use rustfft::FftPlanner;

pub const SYNC_ROUNDS: usize = 6;
pub const SYNC_PERIOD_S: f32 = 0.6;
pub const SYNC_LEVEL: f32 = 0.3;
pub const SYNC_BANDS: [(f32, f32); 4] = [(800.0, 3200.0), (3200.0, 800.0), (1000.0, 4000.0), (4000.0, 1000.0)];
const SYNC_BELLS: [f32; 4] = [1047.0, 784.0, 1319.0, 880.0];
pub const SILENT_BANDS: [(f32, f32); 2] = [(19000.0, 20800.0), (20800.0, 19000.0)];
const SILENT_LEVEL: f32 = 0.15;
const SYNC_SEARCH_S: f32 = 0.2;
const SYNC_MIN_GOOD: usize = 3;
const SYNC_AGREE_MS: f32 = 15.0;
const SYNC_MIN_SNR: f32 = 8.0;
const GATED_ZERO_FRACTION: f32 = 0.5;
const CHIRP_S: f32 = 0.12;
const CHIME_ATTACK_S: f32 = 0.005;
const CHIME_SWEEP_DECAY_S: f32 = 0.08;
const CHIME_RING_S: f32 = 0.4;
const CHIME_RING_DECAY_S: f32 = 0.12;
const CHIME_BELL_LEVEL: f32 = 0.5;

pub struct SyncPattern {
    pub offsets: Vec<Vec<f32>>,
    pub bands: Vec<(f32, f32)>,
    pub length_s: f32,
    pub silent: bool,
}

pub fn chirp(rate: u32, band: (f32, f32), silent: bool) -> Vec<f32> {
    let (f0, f1) = band;
    let count = (rate as f32 * CHIRP_S) as usize;
    (0..count)
        .map(|i| {
            let t = i as f32 / rate as f32;
            let phase = 2.0 * PI * (f0 * t + (f1 - f0) * t * t / (2.0 * CHIRP_S));
            let envelope = if silent {
                0.5 - 0.5 * (2.0 * PI * t / CHIRP_S).cos()
            } else {
                (t / CHIME_ATTACK_S).min(1.0) * (-t / CHIME_SWEEP_DECAY_S).exp()
            };
            phase.sin() * envelope
        })
        .collect()
}

fn normalize(signal: Vec<f32>, level: f32) -> Vec<f32> {
    let peak = signal.iter().fold(0f32, |m, v| m.max(v.abs())).max(1e-9);
    signal.iter().map(|v| v / peak * level).collect()
}

pub fn chime(rate: u32, speaker: usize, silent: bool) -> Vec<f32> {
    if silent {
        return normalize(chirp(rate, SILENT_BANDS[speaker % SILENT_BANDS.len()], true), SILENT_LEVEL);
    }
    let band = SYNC_BANDS[speaker % SYNC_BANDS.len()];
    let bell_hz = SYNC_BELLS[speaker % SYNC_BELLS.len()];
    let sweep = chirp(rate, band, false);
    let count = (rate as f32 * CHIME_RING_S) as usize;
    let mut out: Vec<f32> = (0..count)
        .map(|i| {
            let t = i as f32 / rate as f32;
            let tone = (2.0 * PI * bell_hz * t).sin() + 0.3 * (4.0 * PI * bell_hz * t).sin();
            let envelope = (t / CHIME_ATTACK_S).min(1.0) * (-t / CHIME_RING_DECAY_S).exp();
            tone * envelope * CHIME_BELL_LEVEL
        })
        .collect();
    for (o, s) in out.iter_mut().zip(sweep.iter()) {
        *o += s;
    }
    normalize(out, SYNC_LEVEL)
}

pub fn build_pattern(speakers: usize, silent: bool) -> SyncPattern {
    let palette: &[(f32, f32)] = if silent { &SILENT_BANDS } else { &SYNC_BANDS };
    let slots = speakers.div_ceil(palette.len()).max(1);
    let period = slots as f32 * SYNC_PERIOD_S;
    let offsets = (0..SYNC_ROUNDS)
        .map(|round| {
            (0..speakers)
                .map(|k| round as f32 * period + (k / palette.len()) as f32 * SYNC_PERIOD_S)
                .collect()
        })
        .collect();
    let bands = (0..speakers).map(|k| palette[k % palette.len()]).collect();
    SyncPattern {
        offsets,
        bands,
        length_s: SYNC_ROUNDS as f32 * period + 0.5,
        silent,
    }
}

pub fn render_pattern(pattern: &SyncPattern, rate: u32, speaker: usize) -> Vec<f32> {
    let length = (pattern.length_s * rate as f32) as usize;
    let mut mono = vec![0f32; length];
    let tone = chime(rate, speaker, pattern.silent);
    for row in &pattern.offsets {
        let at = (row[speaker] * rate as f32) as usize;
        for (i, v) in tone.iter().enumerate() {
            if let Some(slot) = mono.get_mut(at + i) {
                *slot = *v;
            }
        }
    }
    mono
}

fn median(values: &[f32]) -> f32 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let n = sorted.len();
    if n == 0 {
        0.0
    } else if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

fn envelope(recording: &[f32], rate: u32, band: (f32, f32), silent: bool, planner: &mut FftPlanner<f32>) -> Vec<f32> {
    let template = chirp(rate, band, silent);
    let size = (recording.len() + template.len()).next_power_of_two();
    let fft = planner.plan_fft_forward(size);
    let ifft = planner.plan_fft_inverse(size);
    let mut a: Vec<Complex32> = recording.iter().map(|&v| Complex32::new(v, 0.0)).collect();
    a.resize(size, Complex32::new(0.0, 0.0));
    let mut b: Vec<Complex32> = template.iter().map(|&v| Complex32::new(v, 0.0)).collect();
    b.resize(size, Complex32::new(0.0, 0.0));
    fft.process(&mut a);
    fft.process(&mut b);
    for (x, y) in a.iter_mut().zip(b.iter()) {
        *x *= y.conj();
    }
    ifft.process(&mut a);
    let raw: Vec<f32> = a[..recording.len()].iter().map(|c| c.re.abs()).collect();
    let width = (rate / 1000).max(1) as usize;
    let mut smooth = vec![0f32; raw.len()];
    let mut sum = 0f32;
    for i in 0..raw.len() {
        sum += raw[i];
        if i >= width {
            sum -= raw[i - width];
        }
        let center = i.saturating_sub(width / 2);
        smooth[center] = sum / width as f32;
    }
    let floor = median(&smooth) + 1e-12;
    smooth.iter().map(|v| v / floor).collect()
}

fn argmax(values: &[f32]) -> usize {
    values
        .iter()
        .enumerate()
        .fold((0, f32::MIN), |best, (i, &v)| if v > best.1 { (i, v) } else { best })
        .0
}

pub fn measure(recording: &[f32], rate: u32, pattern: &SyncPattern) -> Result<Vec<f32>, String> {
    if recording.len() < rate as usize {
        return Err("Recording was too short. Try again.".into());
    }
    let zeros = recording.iter().filter(|v| **v == 0.0).count() as f32 / recording.len() as f32;
    if zeros > GATED_ZERO_FRACTION {
        return Err("Your laptop's noise cancelling muted the chimes. Turn it off in your laptop's audio app (e.g. Lenovo Vantage > Microphone noise cancelling > Off) and try again.".into());
    }
    let mut planner = FftPlanner::new();
    let envs: Vec<Vec<f32>> = pattern.bands.iter().map(|b| envelope(recording, rate, *b, pattern.silent, &mut planner)).collect();
    let combined: Vec<f32> = (0..recording.len())
        .map(|i| envs.iter().fold(0f32, |m, e| m.max(e[i])))
        .collect();

    let mut flat: Vec<f32> = pattern.offsets.iter().flatten().copied().collect();
    flat.sort_by(|a, b| a.total_cmp(b));
    flat.dedup_by(|a, b| (*a - *b).abs() < 1e-6);

    let guard = (0.25 * rate as f32) as usize;
    let mut work = combined.clone();
    let mut peaks = Vec::new();
    for _ in 0..2 * flat.len() {
        let i = argmax(&work);
        if work[i] < SYNC_MIN_SNR {
            break;
        }
        peaks.push(i as f32 / rate as f32);
        let lo = i.saturating_sub(guard);
        let hi = (i + guard).min(work.len());
        work[lo..hi].iter_mut().for_each(|v| *v = 0.0);
    }
    if peaks.len() < flat.len() / 2 {
        return Err("The mic couldn't hear the chimes clearly. Turn the speakers up and try again.".into());
    }

    let mut best_score = 0;
    let mut tau = 0f32;
    for p in &peaks {
        for o in &flat {
            let shift = p - o;
            let score = flat
                .iter()
                .filter(|o2| peaks.iter().any(|q| (q - (shift + **o2)).abs() <= SYNC_SEARCH_S))
                .count();
            if score > best_score {
                best_score = score;
                tau = shift;
            }
        }
    }
    if (best_score as f32) < 0.6 * flat.len() as f32 {
        return Err("Too much noise during the sync. Stay quiet and try again.".into());
    }

    let mut rounds: Vec<Vec<f32>> = Vec::new();
    for row in &pattern.offsets {
        let mut arrivals = Vec::new();
        for (o, env) in row.iter().zip(envs.iter()) {
            let lo = ((tau + o - SYNC_SEARCH_S) * rate as f32).max(0.0) as usize;
            let hi = (((tau + o + SYNC_SEARCH_S) * rate as f32) as usize).min(env.len());
            if hi <= lo + 10 {
                break;
            }
            let i = lo + argmax(&env[lo..hi]);
            if env[i] < SYNC_MIN_SNR {
                break;
            }
            arrivals.push(i as f32 / rate as f32 - (tau + o));
        }
        if arrivals.len() == row.len() {
            let first = arrivals.iter().fold(f32::MAX, |m, v| m.min(*v));
            rounds.push(arrivals.iter().map(|v| (v - first) * 1000.0).collect());
        }
    }
    if rounds.len() < SYNC_MIN_GOOD {
        return Err("The mic missed too many chimes. Move closer and try again.".into());
    }

    let speakers = pattern.bands.len();
    let lateness: Vec<f32> = (0..speakers)
        .map(|k| median(&rounds.iter().map(|r| r[k]).collect::<Vec<_>>()))
        .collect();
    let agreed: Vec<&Vec<f32>> = rounds
        .iter()
        .filter(|r| r.iter().zip(&lateness).all(|(v, l)| (v - l).abs() <= SYNC_AGREE_MS))
        .collect();
    if agreed.len() < SYNC_MIN_GOOD {
        return Err("Results didn't agree (noise?). Stay quiet and try again.".into());
    }
    Ok((0..speakers)
        .map(|k| median(&agreed.iter().map(|r| r[k]).collect::<Vec<_>>()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_recording(rate: u32, gap_ms: f32, noise: f32, silent: bool) -> (Vec<f32>, SyncPattern) {
        let pattern = build_pattern(2, silent);
        let total = ((pattern.length_s + 2.0) * rate as f32) as usize;
        let mut seed = 12345u32;
        let mut rec: Vec<f32> = (0..total)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                ((seed >> 8) as f32 / 16777216.0 - 0.5) * 2.0 * noise
            })
            .collect();
        for k in 0..2 {
            let tone = chime(rate, k, silent);
            for row in &pattern.offsets {
                let delay = 0.7 + 0.3 + if k == 1 { gap_ms / 1000.0 } else { 0.0 };
                let at = ((row[k] + delay) * rate as f32) as usize;
                for (i, v) in tone.iter().enumerate() {
                    rec[at + i] += v * 0.5;
                    rec[at + i + (0.015 * rate as f32) as usize] += v * 0.15;
                }
            }
        }
        (rec, pattern)
    }

    #[test]
    fn measures_known_gaps() {
        for silent in [false, true] {
            for (gap, noise, rate) in [(23.0, 0.01, 48000), (-40.0, 0.01, 48000), (0.0, 0.03, 48000), (120.0, 0.01, 48000), (35.0, 0.05, 48000)] {
                let (rec, pattern) = fake_recording(rate, gap, noise, silent);
                let late = measure(&rec, rate, &pattern).expect("measure");
                let measured = late[1] - late[0];
                assert!((measured - gap).abs() < 1.0, "silent {silent}, gap {gap}: measured {measured}");
            }
        }
    }

    #[test]
    fn rejects_silence_and_gating() {
        let pattern = build_pattern(2, false);
        assert!(measure(&vec![0.0; 48000 * 6], 48000, &pattern).is_err());
        let quiet: Vec<f32> = (0..48000u64 * 6).map(|i| ((i * 7919) % 101) as f32 * 1e-5).collect();
        assert!(measure(&quiet, 48000, &pattern).is_err());
    }
}
