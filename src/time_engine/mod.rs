//! Simulation clock, speed presets and queued step requests.

use serde::{Deserialize, Serialize};

pub const SPEED_PRESETS: &[(&str, f64)] = &[
    ("1 minute / s", 1.0 / (365.25 * 24.0 * 60.0)),
    ("1 hour / s", 1.0 / (365.25 * 24.0)),
    ("1 day / s", 1.0 / 365.25),
    ("1 week / s", 7.0 / 365.25),
    ("1 month / s", 1.0 / 12.0),
    ("1 year / s", 1.0),
    ("10 years / s", 10.0),
    ("100 years / s", 100.0),
    ("1,000 years / s", 1.0e3),
    ("10,000 years / s", 1.0e4),
    ("100,000 years / s", 1.0e5),
    ("1 million years / s", 1.0e6),
    ("10 million years / s", 1.0e7),
    ("100 million years / s", 1.0e8),
    ("1 billion years / s", 1.0e9),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeEngine {
    pub paused: bool,
    pub speed_years_per_second: f64,
    pub speed_preset: usize,
    /// Years requested by "advance by …" controls, consumed on the next update.
    pub pending_years: f64,
    /// Real-time accumulator (not persisted).
    #[serde(skip)]
    pub real_accumulator: f64,
}

impl Default for TimeEngine {
    fn default() -> Self {
        Self {
            paused: false,
            speed_years_per_second: 1.0,
            speed_preset: 5,
            pending_years: 0.0,
            real_accumulator: 0.0,
        }
    }
}

impl TimeEngine {
    pub fn set_preset(&mut self, index: usize) {
        let index = index.min(SPEED_PRESETS.len() - 1);
        self.speed_preset = index;
        self.speed_years_per_second = SPEED_PRESETS[index].1;
    }

    pub fn faster(&mut self) {
        self.set_preset((self.speed_preset + 1).min(SPEED_PRESETS.len() - 1));
    }

    pub fn slower(&mut self) {
        self.set_preset(self.speed_preset.saturating_sub(1));
    }

    pub fn preset_label(&self) -> &'static str {
        SPEED_PRESETS
            .get(self.speed_preset)
            .map(|p| p.0)
            .unwrap_or("custom")
    }

    pub fn queue_years(&mut self, years: f64) {
        self.pending_years += years.max(0.0);
    }

    /// Consumes real time and queued steps, returning simulation years to
    /// advance. The caller batches this at a modest rate to keep the CPU and
    /// render loop responsive during very fast geological speeds.
    pub fn take_years(&mut self, real_dt_seconds: f64) -> f64 {
        let dt = real_dt_seconds.clamp(0.0, 0.5);
        self.real_accumulator += dt;
        if self.real_accumulator < 0.07 && self.pending_years <= 0.0 {
            return 0.0;
        }
        let mut years = 0.0;
        if !self.paused {
            years += self.speed_years_per_second * self.real_accumulator;
        }
        years += self.pending_years;
        self.pending_years = 0.0;
        self.real_accumulator = 0.0;
        years.max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_stops_time_but_queued_steps_run() {
        let mut time = TimeEngine::default();
        time.paused = true;
        assert_eq!(time.take_years(0.5), 0.0);
        time.queue_years(10.0);
        assert!((time.take_years(0.0) - 10.0).abs() < 1.0e-9);
    }

    #[test]
    fn presets_are_monotonic() {
        for pair in SPEED_PRESETS.windows(2) {
            assert!(pair[1].1 > pair[0].1);
        }
    }
}
