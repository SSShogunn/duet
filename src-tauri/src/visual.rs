use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

pub const BANDS: usize = 28;
const SIZE: usize = 2048;
const INTERVAL: Duration = Duration::from_millis(33);
const MIN_HZ: f32 = 40.0;
const MAX_HZ: f32 = 16_000.0;
const FLOOR_DB: f32 = -66.0;
const DECAY: f32 = 0.8;

pub struct Analyzer {
    samples: VecDeque<f32>,
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    edges: Vec<usize>,
    reference_db: f32,
    bands: Vec<f32>,
    last: Instant,
}

impl Analyzer {
    pub fn new(rate: u32) -> Analyzer {
        let bin_hz = rate as f32 / SIZE as f32;
        let edges = (0..=BANDS)
            .map(|b| {
                let hz = MIN_HZ * (MAX_HZ / MIN_HZ).powf(b as f32 / BANDS as f32);
                ((hz / bin_hz).round() as usize).clamp(1, SIZE / 2 - 1)
            })
            .collect();
        Analyzer {
            samples: VecDeque::with_capacity(SIZE * 2),
            fft: FftPlanner::new().plan_fft_forward(SIZE),
            window: (0..SIZE).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / (SIZE - 1) as f32).cos()).collect(),
            edges,
            reference_db: 20.0 * (SIZE as f32 / 4.0).log10(),
            bands: vec![0.0; BANDS],
            last: Instant::now(),
        }
    }

    pub fn push(&mut self, frames: &[[f32; 2]]) -> Option<Vec<f32>> {
        self.samples.extend(frames.iter().map(|f| (f[0] + f[1]) * 0.5));
        let excess = self.samples.len().saturating_sub(SIZE);
        self.samples.drain(..excess);
        if self.samples.len() < SIZE || self.last.elapsed() < INTERVAL {
            return None;
        }
        self.last = Instant::now();
        let mut buffer: Vec<Complex32> = self
            .samples
            .iter()
            .zip(&self.window)
            .map(|(s, w)| Complex32::new(s * w, 0.0))
            .collect();
        self.fft.process(&mut buffer);
        for b in 0..BANDS {
            let lo = self.edges[b];
            let hi = self.edges[b + 1].max(lo + 1);
            let magnitude = buffer[lo..hi].iter().fold(0f32, |m, c| m.max(c.norm()));
            let db = 20.0 * magnitude.max(1e-9).log10() - self.reference_db;
            let level = ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0);
            self.bands[b] = level.max(self.bands[b] * DECAY);
        }
        Some(self.bands.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_lights_its_band() {
        let rate = 48_000;
        let mut analyzer = Analyzer::new(rate);
        analyzer.last = Instant::now() - INTERVAL;
        let frames: Vec<[f32; 2]> = (0..SIZE)
            .map(|i| {
                let v = 0.5 * (2.0 * PI * 1000.0 * i as f32 / rate as f32).sin();
                [v, v]
            })
            .collect();
        let bands = analyzer.push(&frames).expect("bands");
        let loudest = bands.iter().enumerate().fold((0, 0f32), |m, (i, v)| if *v > m.1 { (i, *v) } else { m }).0;
        let bin_hz = rate as f32 / SIZE as f32;
        let center = (analyzer.edges[loudest] as f32 + analyzer.edges[loudest + 1] as f32) / 2.0 * bin_hz;
        println!("bands: {:?}", bands.iter().map(|v| (v * 100.0).round() / 100.0).collect::<Vec<_>>());
        assert!((center - 1000.0).abs() < 300.0, "loudest band centered at {center} Hz");
        assert!(bands[loudest] > 0.8);
        assert!(bands[0] < 0.3 && bands[BANDS - 1] < 0.3);
    }
}
