//! Weather events generated from local climate conditions.
//!
//! Weather is a regional, short-lived perturbation of the same state used by
//! the climate and hydrology models. Events have a location, radius,
//! intensity, duration and measurable effects on soil moisture, snow/ice,
//! temperature anomalies and CO₂ (fires).
//!
//! At geological simulation speeds many short events cannot be resolved
//! individually; the event generator is capped per step and the resulting
//! climate is dominated by the slow variables. This is stated in the docs.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::atmosphere::AtmosphereState;
use crate::localization::tr;
use crate::terrain::{Biome, PlanetGrid};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherKind {
    Rain,
    Snow,
    Hail,
    Thunderstorm,
    Hurricane,
    Drought,
    Flood,
    Heatwave,
    ColdWave,
    Sandstorm,
    DustStorm,
    Wildfire,
}

impl WeatherKind {
    pub const ALL: [Self; 12] = [
        Self::Rain,
        Self::Snow,
        Self::Hail,
        Self::Thunderstorm,
        Self::Hurricane,
        Self::Drought,
        Self::Flood,
        Self::Heatwave,
        Self::ColdWave,
        Self::Sandstorm,
        Self::DustStorm,
        Self::Wildfire,
    ];

    pub fn label(self) -> &'static str {
        tr(match self {
            Self::Rain => "Rain",
            Self::Snow => "Snow",
            Self::Hail => "Hail",
            Self::Thunderstorm => "Thunderstorm",
            Self::Hurricane => "Hurricane",
            Self::Drought => "Drought",
            Self::Flood => "Flood",
            Self::Heatwave => "Heatwave",
            Self::ColdWave => "Cold wave",
            Self::Sandstorm => "Sandstorm",
            Self::DustStorm => "Dust storm",
            Self::Wildfire => "Wildfire",
        })
    }

    pub fn is_severe(self) -> bool {
        matches!(
            self,
            Self::Hurricane | Self::Flood | Self::Drought | Self::Wildfire | Self::DustStorm
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeatherEvent {
    pub kind: WeatherKind,
    pub cell: u32,
    pub intensity: f32,
    pub radius_rad: f32,
    pub remaining_years: f32,
    pub duration_years: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeatherState {
    pub events: Vec<WeatherEvent>,
}

#[derive(Clone, Debug)]
pub struct WeatherOutcome {
    pub kind: WeatherKind,
    pub title: String,
    pub description: String,
}

impl Default for WeatherState {
    fn default() -> Self {
        Self { events: Vec::new() }
    }
}

impl WeatherState {
    /// Spawns and applies weather for one simulation slice.
    pub fn step(
        &mut self,
        grid: &mut PlanetGrid,
        atmosphere: &mut AtmosphereState,
        rng: &mut ChaCha8Rng,
        dt_years: f32,
        activity: f32,
        outcomes: &mut Vec<WeatherOutcome>,
    ) {
        let dt = dt_years.max(0.0);
        if dt == 0.0 {
            return;
        }

        // Apply existing events.
        for event in &mut self.events {
            let active_dt = dt.min(event.remaining_years.max(0.0));
            apply_weather_event(grid, atmosphere, event, active_dt);
            event.remaining_years -= dt;
        }
        self.events.retain(|e| e.remaining_years > 0.0);
        if self.events.len() > 32 {
            self.events.drain(0..self.events.len() - 32);
        }

        if grid.cell_count() == 0 {
            return;
        }
        let rate = 3.2 * activity.clamp(0.0, 5.0);
        let mean = rate * dt;
        let count = poisson_capped(rng, mean, 4);
        for _ in 0..count {
            let cell = pick_weather_cell(grid, rng);
            let kind = choose_kind(grid, cell, rng);
            let (duration, radius_km, intensity) = event_shape(kind, rng);
            let radius_rad = radius_km / grid.radius_km.max(1.0);
            let event = WeatherEvent {
                kind,
                cell: cell as u32,
                intensity,
                radius_rad,
                remaining_years: duration,
                duration_years: duration,
            };
            if kind.is_severe() {
                outcomes.push(WeatherOutcome {
                    kind,
                    title: format!("{}", kind.label()),
                    description: format!(
                        "{} at {:.1}°, {:.1}° lasting {:.1} days (intensity {:.0}%).",
                        kind.label(),
                        grid.lat_deg(cell),
                        grid.lon_deg(cell),
                        duration * 365.25,
                        intensity * 100.0
                    ),
                });
            }
            self.events.push(event);
        }
    }

    pub fn trigger(
        &mut self,
        grid: &mut PlanetGrid,
        _atmosphere: &mut AtmosphereState,
        kind: WeatherKind,
        cell: usize,
        intensity: f32,
        duration_days: f32,
    ) -> WeatherOutcome {
        let cell = cell.min(grid.cell_count().saturating_sub(1));
        let duration = (duration_days.max(1.0) / 365.25).clamp(1.0 / 365.25, 2.0);
        let radius_km = match kind {
            WeatherKind::Hurricane => 900.0,
            WeatherKind::Drought | WeatherKind::Heatwave => 1_500.0,
            WeatherKind::DustStorm | WeatherKind::Sandstorm => 1_000.0,
            _ => 350.0,
        };
        let event = WeatherEvent {
            kind,
            cell: cell as u32,
            intensity: intensity.clamp(0.05, 1.0),
            radius_rad: radius_km / grid.radius_km.max(1.0),
            remaining_years: duration,
            duration_years: duration,
        };
        let outcome = WeatherOutcome {
            kind,
            title: format!("{} (manual)", kind.label()),
            description: format!(
                "Triggered at {:.1}°, {:.1}° for {:.0} days.",
                grid.lat_deg(cell),
                grid.lon_deg(cell),
                duration_days
            ),
        };
        self.events.push(event);
        outcome
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

fn apply_weather_event(
    grid: &mut PlanetGrid,
    atmosphere: &mut AtmosphereState,
    event: &WeatherEvent,
    active_dt_years: f32,
) {
    if active_dt_years <= 0.0 || grid.cell_count() == 0 {
        return;
    }
    let center = grid.direction(event.cell as usize);
    let radius = event.radius_rad.max(1.0e-4);
    let falloff_scale = active_dt_years * event.intensity;
    let mut fire_co2 = 0.0f32;

    for i in 0..grid.cell_count() {
        let angle = center.dot(grid.direction(i)).clamp(-1.0, 1.0).acos();
        if angle > radius {
            continue;
        }
        let falloff = (1.0 - angle / radius).powf(1.4);
        let c = &mut grid.cells[i];
        match event.kind {
            WeatherKind::Rain | WeatherKind::Thunderstorm => {
                c.soil_moisture_m = (c.soil_moisture_m + 2.4 * falloff * falloff_scale).min(0.4);
                c.precip_mm_yr += 900.0 * event.intensity * falloff;
            }
            WeatherKind::Hurricane => {
                c.soil_moisture_m = (c.soil_moisture_m + 4.2 * falloff * falloff_scale).min(0.4);
                c.precip_mm_yr += 2_200.0 * event.intensity * falloff;
                c.transient_heat_c = c.transient_heat_c.max(0.8 * falloff * event.intensity);
            }
            WeatherKind::Flood => {
                c.soil_moisture_m = 0.4;
                c.precip_mm_yr += 1_800.0 * event.intensity * falloff;
                c.discharge_m3s += 250.0 * falloff * event.intensity;
            }
            WeatherKind::Snow | WeatherKind::Hail => {
                let add = 0.9 * falloff * falloff_scale;
                c.ice_thickness_m = (c.ice_thickness_m + add).min(3_500.0);
                c.snow_depth_m = (c.snow_depth_m + add).min(5.0);
                c.precip_mm_yr += 500.0 * event.intensity * falloff;
                c.transient_heat_c = c.transient_heat_c.min(-1.2 * falloff * event.intensity);
            }
            WeatherKind::Drought => {
                c.soil_moisture_m =
                    (c.soil_moisture_m - 0.35 * falloff * active_dt_years.min(1.0)).max(0.0);
                c.humidity = (c.humidity - 0.25 * falloff).max(0.02);
                c.precip_mm_yr *= 1.0 - 0.4 * falloff;
            }
            WeatherKind::Heatwave => {
                c.transient_heat_c = c.transient_heat_c.max(9.0 * falloff * event.intensity);
                c.soil_moisture_m =
                    (c.soil_moisture_m - 0.15 * falloff * active_dt_years.min(1.0)).max(0.0);
            }
            WeatherKind::ColdWave => {
                c.transient_heat_c = c.transient_heat_c.min(-11.0 * falloff * event.intensity);
            }
            WeatherKind::Sandstorm | WeatherKind::DustStorm => {
                c.soil_moisture_m = (c.soil_moisture_m - 0.05 * falloff).max(0.0);
                c.humidity = (c.humidity - 0.2 * falloff).max(0.02);
                if event.kind == WeatherKind::DustStorm {
                    atmosphere.inject_aerosol(0.0008 * falloff * event.intensity);
                }
            }
            WeatherKind::Wildfire => {
                c.transient_heat_c = c.transient_heat_c.max(7.0 * falloff * event.intensity);
                c.soil_moisture_m = (c.soil_moisture_m - 0.12 * falloff).max(0.0);
                c.albedo = (c.albedo - 0.04 * falloff).max(0.05);
                if matches!(
                    c.biome,
                    Biome::TropicalRainforest
                        | Biome::TropicalSeasonalForest
                        | Biome::TemperateForest
                        | Biome::BorealForest
                        | Biome::Taiga
                        | Biome::Savanna
                        | Biome::Grassland
                ) {
                    c.biome = Biome::Grassland;
                }
                fire_co2 += 0.0008 * falloff * event.intensity;
            }
        }
    }
    atmosphere.co2_ppm += fire_co2;
}

fn choose_kind(grid: &PlanetGrid, cell: usize, rng: &mut ChaCha8Rng) -> WeatherKind {
    let c = &grid.cells[cell];
    let submerged = c.water_depth_m > 0.0;
    let mut options: Vec<(WeatherKind, f32)> = Vec::new();
    if submerged {
        if c.temp_c > 22.0 {
            options.push((WeatherKind::Hurricane, 3.0));
            options.push((WeatherKind::Thunderstorm, 2.0));
        }
        options.push((WeatherKind::Rain, 4.0));
        options.push((WeatherKind::Flood, 0.6));
        if c.ice_thickness_m > 0.2 {
            options.push((WeatherKind::Snow, 2.0));
        }
    } else {
        if c.temp_c < 0.0 {
            options.push((WeatherKind::Snow, 5.0));
            options.push((WeatherKind::Hail, 1.0));
            options.push((WeatherKind::ColdWave, 2.0));
        } else if c.temp_c > 30.0 {
            options.push((WeatherKind::Heatwave, 3.0));
            options.push((WeatherKind::Drought, 2.0));
            if c.soil_moisture_m < 0.08 {
                options.push((WeatherKind::Sandstorm, 2.0));
                options.push((WeatherKind::DustStorm, 1.2));
            }
        } else {
            options.push((WeatherKind::Rain, 3.0));
            options.push((WeatherKind::Thunderstorm, 1.5));
            options.push((WeatherKind::Drought, 0.5));
        }
        if c.soil_moisture_m < 0.12 && c.temp_c > 15.0 {
            options.push((WeatherKind::Wildfire, 1.0));
        }
    }
    let total: f32 = options.iter().map(|(_, w)| *w).sum();
    let mut pick = rng.random_range(0.0..total.max(0.01));
    for (kind, weight) in options {
        pick -= weight;
        if pick <= 0.0 {
            return kind;
        }
    }
    WeatherKind::Rain
}

fn event_shape(kind: WeatherKind, rng: &mut ChaCha8Rng) -> (f32, f32, f32) {
    let duration_days = match kind {
        WeatherKind::Rain => rng.random_range(0.5..3.0),
        WeatherKind::Snow => rng.random_range(1.0..4.0),
        WeatherKind::Hail => rng.random_range(0.1..0.5),
        WeatherKind::Thunderstorm => rng.random_range(0.2..1.0),
        WeatherKind::Hurricane => rng.random_range(4.0..14.0),
        WeatherKind::Drought => rng.random_range(30.0..240.0),
        WeatherKind::Flood => rng.random_range(2.0..10.0),
        WeatherKind::Heatwave => rng.random_range(5.0..40.0),
        WeatherKind::ColdWave => rng.random_range(3.0..20.0),
        WeatherKind::Sandstorm => rng.random_range(0.5..4.0),
        WeatherKind::DustStorm => rng.random_range(1.0..10.0),
        WeatherKind::Wildfire => rng.random_range(2.0..30.0),
    };
    let radius_km = match kind {
        WeatherKind::Hurricane => rng.random_range(400.0..1_400.0),
        WeatherKind::Drought | WeatherKind::Heatwave => rng.random_range(700.0..2_500.0),
        WeatherKind::DustStorm | WeatherKind::Sandstorm => rng.random_range(300.0..1_500.0),
        WeatherKind::Wildfire => rng.random_range(50.0..300.0),
        _ => rng.random_range(80.0..600.0),
    };
    let intensity = rng.random_range(0.25..1.0);
    (duration_days / 365.25, radius_km, intensity)
}

fn pick_weather_cell(grid: &PlanetGrid, rng: &mut ChaCha8Rng) -> usize {
    let n = grid.cell_count();
    if n == 0 {
        return 0;
    }
    let mut best = rng.random_range(0..n);
    let mut best_score = f32::NEG_INFINITY;
    for _ in 0..12 {
        let idx = rng.random_range(0..n);
        let c = &grid.cells[idx];
        let score = c.humidity * 2.0 + c.soil_moisture_m * 3.0 + (c.temp_c / 40.0).max(0.0)
            - c.ice_thickness_m.min(2.0) * 0.2;
        if score > best_score {
            best_score = score;
            best = idx;
        }
    }
    best
}

fn poisson_capped(rng: &mut ChaCha8Rng, mean: f32, cap: u32) -> u32 {
    if mean <= 0.0 {
        return 0;
    }
    if mean > 8.0 {
        return cap;
    }
    let limit = (-mean).exp();
    let mut k = 0u32;
    let mut p = 1.0f32;
    loop {
        p *= rng.random::<f32>();
        if p <= limit || k >= cap {
            break;
        }
        k += 1;
    }
    k
}
