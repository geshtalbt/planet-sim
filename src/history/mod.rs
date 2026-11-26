//! Event log and adaptively sampled parameter history.

use serde::{Deserialize, Serialize};

use crate::catastrophes::CatastropheKind;
use crate::planet_generator::PlanetConfig;
use crate::statistics::PlanetStats;
use crate::weather::WeatherKind;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistorySample {
    pub time_years: f64,
    pub mean_temp_c: f32,
    pub mean_ocean_temp_c: f32,
    pub co2_ppm: f32,
    pub sea_level_m: f32,
    pub glacier_fraction: f32,
    pub land_fraction: f32,
    pub precip_mm_yr: f32,
    pub pressure_kpa: f32,
    pub albedo: f32,
    pub solar_multiplier: f32,
    pub axial_tilt_deg: f32,
    pub water_inventory_km3: f64,
    pub ice_volume_km3: f64,
    pub active_eruptions: u32,
    pub magnetic_field: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    Catastrophe(CatastropheKind),
    Weather(WeatherKind),
    Tectonic,
    Climate,
    User,
    Milestone,
    Save,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub time_years: f64,
    pub kind: EventKind,
    pub title: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct History {
    pub samples: Vec<HistorySample>,
    pub events: Vec<HistoryEvent>,
    pub next_sample_time: f64,
    pub sample_interval_hint: f64,
}

impl Default for History {
    fn default() -> Self {
        Self {
            samples: Vec::new(),
            events: Vec::new(),
            next_sample_time: 0.0,
            sample_interval_hint: 0.01,
        }
    }
}

impl History {
    const MAX_SAMPLES: usize = 4_000;
    const MAX_EVENTS: usize = 6_000;

    pub fn should_record(&self, time_years: f64) -> bool {
        time_years >= self.next_sample_time
    }

    /// Records a sample and schedules the next one on a roughly logarithmic
    /// grid, so a multi-billion-year run remains bounded in memory.
    pub fn record(&mut self, stats: &PlanetStats, config: &PlanetConfig, solar_multiplier: f32) {
        let sample = HistorySample {
            time_years: stats.age_years,
            mean_temp_c: stats.mean_temp_c,
            mean_ocean_temp_c: stats.mean_ocean_temp_c,
            co2_ppm: stats.co2_ppm,
            sea_level_m: stats.sea_level_m,
            glacier_fraction: stats.glacier_fraction,
            land_fraction: stats.land_fraction,
            precip_mm_yr: stats.mean_precip_mm_yr,
            pressure_kpa: stats.pressure_kpa,
            albedo: stats.mean_albedo,
            solar_multiplier,
            axial_tilt_deg: config.axial_tilt_deg,
            water_inventory_km3: stats.total_water_km3,
            ice_volume_km3: stats.ice_volume_km3,
            active_eruptions: stats.active_eruptions,
            magnetic_field: stats.magnetic_field,
        };
        self.samples.push(sample);
        if self.samples.len() > Self::MAX_SAMPLES {
            self.downsample();
        }
        let today = stats.age_years;
        self.sample_interval_hint = (today.abs() * 0.012).max(1.0 / 365.25);
        self.next_sample_time = today + self.sample_interval_hint;
    }

    fn downsample(&mut self) {
        let mut kept = Vec::with_capacity(self.samples.len() / 2 + 1);
        for (i, sample) in self.samples.drain(..).enumerate() {
            if i % 2 == 0 {
                kept.push(sample);
            }
        }
        self.samples = kept;
    }

    pub fn log(
        &mut self,
        time_years: f64,
        kind: EventKind,
        title: impl Into<String>,
        description: impl Into<String>,
    ) {
        self.events.push(HistoryEvent {
            time_years,
            kind,
            title: title.into(),
            description: description.into(),
        });
        if self.events.len() > Self::MAX_EVENTS {
            self.events.remove(0);
        }
    }

    pub fn events_between(&self, start: f64, end: f64) -> impl Iterator<Item = &HistoryEvent> {
        self.events
            .iter()
            .filter(move |e| e.time_years >= start && e.time_years <= end)
    }

    pub fn latest_sample(&self) -> Option<&HistorySample> {
        self.samples.last()
    }

    pub fn sample_at_or_before(&self, time: f64) -> Option<&HistorySample> {
        self.samples.iter().rev().find(|s| s.time_years <= time)
    }

    pub fn time_range(&self) -> (f64, f64) {
        let min = self.samples.first().map(|s| s.time_years).unwrap_or(0.0);
        let max = self.samples.last().map(|s| s.time_years).unwrap_or(0.0);
        (min, max)
    }
}
