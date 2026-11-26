//! The complete planetary state and one-slice integration entry point.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::atmosphere::{AtmosphereState, CarbonState};
use crate::catastrophes::{self, CatastropheOutcome, CatastropheState, GlobalForcing};
use crate::climate::{self, ClimateState};
use crate::history::EventKind;
use crate::hydrology::{self, HydrologyState};
use crate::planet_generator::{PlanetConfig, generate_planet};
use crate::statistics::PlanetStats;
use crate::tectonics::{Plate, PlateStats, TectonicRates, tectonic_step};
use crate::terrain::{Biome, PlanetGrid};
use crate::weather::{WeatherKind, WeatherOutcome, WeatherState};

#[derive(Clone, Debug)]
pub struct StepEvent {
    pub kind: EventKind,
    pub title: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanetState {
    pub config: PlanetConfig,
    pub grid: PlanetGrid,
    pub atmosphere: AtmosphereState,
    pub carbon: CarbonState,
    pub hydrology: HydrologyState,
    pub climate: ClimateState,
    pub catastrophes: CatastropheState,
    pub weather: WeatherState,
    pub plates: Vec<Plate>,
    pub tectonic_rates: TectonicRates,
    pub last_tectonic: PlateStats,
    pub time_years: f64,
    /// Base user-controlled stellar multiplier. Catastrophe forcings multiply
    /// this value rather than overwriting it.
    pub solar_multiplier: f32,
    pub base_ocean_circulation: f32,
    /// Gradual parameter ramps (0 = disabled).
    pub co2_ramp_target_ppm: f32,
    pub co2_ramp_ppm_per_year: f32,
    pub tilt_ramp_target_deg: f32,
    pub tilt_ramp_deg_per_myr: f32,
    pub solar_ramp_target: f32,
    pub solar_ramp_per_year: f32,
    pub tectonic_accum_years: f64,
    pub rng: ChaCha8Rng,
    pub revision: u64,
    pub stats: PlanetStats,
}

impl PlanetState {
    pub fn new(config: PlanetConfig) -> Result<Self, String> {
        config.validate()?;
        let generated = generate_planet(&config)?;
        let mut grid = generated.grid;
        grid.rebuild_areas(config.radius_m());
        let atmosphere = AtmosphereState::from_config(&config);
        let carbon = CarbonState::from_config(&config);
        let climate = ClimateState::default();
        climate::initialize_climate(&mut grid, &config, &atmosphere, 0.0);

        let rng = ChaCha8Rng::seed_from_u64(
            config.seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0xA5A5_5A5A_1234_5678,
        );
        let catastrophes = CatastropheState::new(config.random_events);

        let config_co2 = config.co2_ppm;
        let config_tilt = config.axial_tilt_deg;
        let mut planet = Self {
            config,
            grid,
            atmosphere,
            carbon,
            hydrology: HydrologyState {
                total_water_km3: 1.0,
                ocean_volume_km3: 0.0,
                ocean_area_km2: 0.0,
                sea_level_m: 0.0,
                thermal_expansion_m: 0.0,
                ice_volume_km3: 0.0,
                soil_water_km3: 0.0,
                lake_volume_km3: 0.0,
                atmosphere_water_km3: 0.0,
                mean_ocean_depth_m: 0.0,
                mean_precip_mm_yr: 1_000.0,
                runoff_km3_per_yr: 0.0,
                river_count: 0,
                lake_count: 0,
                initial_ocean_temp_c: 17.0,
            },
            climate,
            catastrophes,
            weather: WeatherState::default(),
            plates: generated.plates,
            tectonic_rates: TectonicRates::default(),
            last_tectonic: PlateStats::default(),
            time_years: 0.0,
            solar_multiplier: 1.0,
            base_ocean_circulation: 1.0,
            co2_ramp_target_ppm: config_co2,
            co2_ramp_ppm_per_year: 0.0,
            tilt_ramp_target_deg: config_tilt,
            tilt_ramp_deg_per_myr: 0.0,
            solar_ramp_target: 1.0,
            solar_ramp_per_year: 0.0,
            tectonic_accum_years: 0.0,
            rng,
            revision: 1,
            stats: PlanetStats::empty(),
        };

        planet.assign_initial_ice(planet.config.initial_glacier_fraction);
        let hydrology = hydrology::initialize_hydrology(
            &mut planet.grid,
            &planet.config,
            planet.config.ocean_temp_c,
        );
        planet.hydrology = hydrology;
        let sea = planet.hydrology.effective_sea_level_m();
        planet.grid.apply_sea_level(sea, planet.config.radius_m());
        for cell in &mut planet.grid.cells {
            let submerged = cell.bedrock_m < sea;
            if submerged {
                cell.biome = Biome::Ocean;
                if cell.ice_thickness_m <= 0.5 {
                    cell.albedo = Biome::Ocean.base_albedo();
                }
            } else if cell.lake_depth_m > 0.05 {
                cell.biome = Biome::Lake;
            } else if cell.biome.is_water() {
                cell.biome = Biome::classify(cell.temp_c, cell.precip_mm_yr, cell.ice_thickness_m);
                cell.albedo = cell.biome.base_albedo();
            }
        }
        climate::recompute_summary(&planet.grid, &mut planet.climate);
        planet.finalize_config();
        planet.calibrate_initial_albedo();
        planet.recompute_stats();

        Ok(planet)
    }

    fn assign_initial_ice(&mut self, fraction: f32) {
        let fraction = fraction.clamp(0.0, 1.0);
        if fraction <= 0.0 {
            return;
        }
        let total_area = self.grid.total_area_m2();
        let target = total_area * fraction as f64;
        let mut order: Vec<usize> = (0..self.grid.cell_count()).collect();
        order.sort_by(|&a, &b| {
            self.grid.cells[a]
                .temp_c
                .partial_cmp(&self.grid.cells[b].temp_c)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut area = 0.0;
        for cell in order {
            if area >= target {
                break;
            }
            let c = &mut self.grid.cells[cell];
            let submerged = c.bedrock_m < 0.0;
            c.ice_thickness_m = if submerged { 2.0 } else { 650.0 };
            c.albedo = 0.72;
            if !submerged {
                c.biome = Biome::IceSheet;
            }
            area += self.grid.area_m2(cell) as f64;
        }
    }

    /// Uses the configured initial albedo as a planetary-albedo target by
    /// calibrating the cloud albedo factor (or the surface offset when there
    /// are no clouds). This keeps the creation control physically meaningful.
    fn calibrate_initial_albedo(&mut self) {
        let surface = self.climate.mean_albedo;
        let cloud = self.atmosphere.cloud_coverage;
        let target = self.config.albedo.clamp(0.02, 0.9);
        if cloud > 0.01 {
            // planetary albedo = surface*(1-c) + 0.44*c*factor
            let factor = (target - surface * (1.0 - cloud)) / (0.44 * cloud);
            self.climate.cloud_albedo_factor = factor.clamp(0.35, 1.8);
        } else {
            self.climate.surface_albedo_offset = (target - surface).clamp(-0.25, 0.35);
            self.climate.mean_albedo = target;
        }
        self.config.albedo = crate::planetary_physics::planetary_albedo(
            self.climate.mean_albedo,
            (cloud * self.climate.cloud_albedo_factor).clamp(0.0, 1.0),
        );
    }

    fn finalize_config(&mut self) {
        let sea = self.hydrology.effective_sea_level_m();
        let mut water_area = 0.0f64;
        let mut glacier_area = 0.0f64;
        let mut max_depth = 0.0f32;
        let total_area = self.grid.total_area_m2();
        for (cell, area) in self.grid.cells.iter().zip(&self.grid.cell_area_m2) {
            if cell.bedrock_m < sea {
                water_area += *area as f64;
                max_depth = max_depth.max(sea - cell.bedrock_m);
            }
            if cell.ice_thickness_m > 0.5 {
                glacier_area += *area as f64;
            }
        }
        let water_fraction = (water_area / total_area) as f32;
        let glacier_fraction = (glacier_area / total_area) as f32;
        self.config.initial_glacier_fraction = glacier_fraction;
        self.config.normalize_after_generation(
            water_fraction,
            self.hydrology.mean_ocean_depth_m,
            max_depth,
            self.hydrology.total_water_km3,
            self.hydrology.sea_level_m,
        );
        self.config.water.avg_ocean_depth_m = self.hydrology.mean_ocean_depth_m;
        self.config.water.max_ocean_depth_m = max_depth;
    }

    /// Advances all modules by one simulation slice. The caller controls the
    /// slice size; the internal schemes are stable for very large steps.
    pub fn step_slice(&mut self, dt_years: f32) -> Vec<StepEvent> {
        let dt = dt_years.max(0.0);
        if dt == 0.0 {
            return Vec::new();
        }
        self.time_years += dt as f64;
        let mut events = Vec::new();

        self.atmosphere.decay_opacity(dt);
        self.advance_ramps(dt);
        self.carbon.exchange(
            &mut self.atmosphere.co2_ppm,
            dt,
            self.climate.mean_ocean_temp_c,
        );
        let rotation_factor = (24.0 / self.config.rotation_period_hours.max(1.0))
            .clamp(0.25, 4.0)
            .powf(0.10);
        self.climate.ocean_circulation = (self.base_ocean_circulation
            * self.catastrophes.ocean_circulation_multiplier()
            * rotation_factor)
            .clamp(0.0, 1.2);

        let mut cat_outcomes = Vec::new();
        catastrophes::catastrophe_step(
            &mut self.grid,
            &mut self.atmosphere,
            &mut self.carbon,
            &mut self.climate,
            &mut self.catastrophes,
            &self.config,
            self.time_years,
            dt,
            &mut self.rng,
            &mut cat_outcomes,
        );
        for outcome in cat_outcomes {
            events.push(step_event(
                EventKind::Catastrophe(outcome.kind),
                outcome.title,
                outcome.description,
            ));
        }

        let solar = self.solar_multiplier * self.catastrophes.solar_multiplier();
        let sea = self.hydrology.effective_sea_level_m();
        climate::update_climate(
            &mut self.grid,
            &self.config,
            &self.atmosphere,
            &mut self.climate,
            sea,
            self.time_years,
            dt,
            solar,
        );

        hydrology::hydrology_step(
            &mut self.grid,
            &self.config,
            &mut self.hydrology,
            self.climate.mean_ocean_temp_c,
            dt,
        );

        let weather_multiplier = self.config.weather_activity * rotation_factor;
        let mut weather_outcomes = Vec::new();
        self.weather.step(
            &mut self.grid,
            &mut self.atmosphere,
            &mut self.rng,
            dt,
            weather_multiplier,
            &mut weather_outcomes,
        );
        for outcome in weather_outcomes {
            events.push(step_event(
                EventKind::Weather(outcome.kind),
                outcome.title,
                outcome.description,
            ));
        }

        // Short-lived anomalies relax; persistent user offsets are preserved.
        let thermal_decay = (-dt / 1.7).exp();
        let quake_decay = (-dt / 4.0).exp();
        for cell in &mut self.grid.cells {
            cell.transient_heat_c *= thermal_decay;
            cell.quake_activity *= quake_decay;
            if cell.quake_activity < 0.001 {
                cell.quake_activity = 0.0;
            }
            if cell.lava_flow < 0.001 {
                cell.lava_flow = 0.0;
            }
        }

        // Tectonics runs on a geological cadence, not every climate step.
        self.tectonic_accum_years += dt as f64;
        if self.tectonic_accum_years >= 5_000.0 {
            let tectonic_dt = self.tectonic_accum_years as f32;
            self.last_tectonic = tectonic_step(
                &mut self.grid,
                &mut self.plates,
                self.config.radius_m(),
                self.hydrology.effective_sea_level_m(),
                tectonic_dt,
                &self.tectonic_rates,
            );
            self.tectonic_accum_years = 0.0;
            self.grid.apply_sea_level(
                self.hydrology.effective_sea_level_m(),
                self.config.radius_m(),
            );
        }

        self.recompute_stats();
        self.revision = self.revision.wrapping_add(1);
        events
    }

    pub fn recompute_stats(&mut self) {
        let solar = self.solar_multiplier * self.catastrophes.solar_multiplier();
        self.stats = PlanetStats::compute(
            &self.grid,
            &self.config,
            &self.atmosphere,
            &self.hydrology,
            &self.climate,
            &self.catastrophes,
            self.time_years,
            solar,
        );
    }

    pub fn effective_solar_multiplier(&self) -> f32 {
        self.solar_multiplier * self.catastrophes.solar_multiplier()
    }

    pub fn sea_level_m(&self) -> f32 {
        self.hydrology.effective_sea_level_m()
    }

    pub fn dominant_biome(&self) -> Biome {
        PlanetStats::dominant_biome(&self.grid)
    }

    // ------------------------------------------------------------------
    // Planetary physics controls
    // ------------------------------------------------------------------

    fn advance_ramps(&mut self, dt: f32) {
        if self.co2_ramp_ppm_per_year > 0.0 {
            let diff = self.co2_ramp_target_ppm - self.atmosphere.co2_ppm;
            let step = (self.co2_ramp_ppm_per_year * dt).min(diff.abs());
            if step > 0.0 {
                let target = self.atmosphere.co2_ppm + step.copysign(diff);
                self.apply_co2_redistribution(target);
            }
        }
        if self.tilt_ramp_deg_per_myr > 0.0 {
            let dt_myr = dt / 1.0e6;
            let diff = self.tilt_ramp_target_deg - self.config.axial_tilt_deg;
            let step = (self.tilt_ramp_deg_per_myr * dt_myr).min(diff.abs());
            self.config.axial_tilt_deg += step.copysign(diff);
        }
        if self.solar_ramp_per_year > 0.0 {
            let diff = self.solar_ramp_target - self.solar_multiplier;
            let step = (self.solar_ramp_per_year * dt).min(diff.abs());
            self.solar_multiplier += step.copysign(diff);
        }
    }

    fn apply_co2_redistribution(&mut self, target: f32) {
        let target = target.clamp(0.1, 5_000_000.0);
        let delta = target - self.atmosphere.co2_ppm;
        if delta > 0.0 {
            let from_ocean = delta.min(self.carbon.ocean_ppm_equiv.max(0.0));
            self.carbon.ocean_ppm_equiv -= from_ocean;
            let remaining = delta - from_ocean;
            let from_geo = remaining.min(self.carbon.geological_ppm_equiv.max(0.0));
            self.carbon.geological_ppm_equiv -= from_geo;
            self.carbon.external_ppm += remaining - from_geo;
        } else {
            self.carbon.ocean_ppm_equiv += -delta;
        }
        self.atmosphere.co2_ppm = target;
    }

    /// Changes atmospheric CO₂. With `redistribute` the carbon is moved
    /// between the ocean/geological reservoirs; otherwise it is added from an
    /// external reservoir and the total inventory changes.
    pub fn set_co2_ppm(&mut self, target_ppm: f32, redistribute: bool) {
        let target = target_ppm.clamp(0.1, 5_000_000.0);
        if redistribute {
            self.apply_co2_redistribution(target);
        } else {
            self.carbon.external_ppm += target - self.atmosphere.co2_ppm;
            self.atmosphere.co2_ppm = target;
        }
        self.co2_ramp_target_ppm = target;
        self.recompute_stats();
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn set_axial_tilt_deg(&mut self, tilt: f32) {
        self.config.axial_tilt_deg = tilt.clamp(0.0, 90.0);
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn set_solar_multiplier(&mut self, multiplier: f32) {
        self.solar_multiplier = multiplier.clamp(0.2, 3.0);
        self.recompute_stats();
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn set_pressure_kpa(&mut self, pressure: f32) {
        let p = pressure.clamp(0.1, 2_000.0);
        self.atmosphere.pressure_kpa = p;
        self.config.surface_pressure_kpa = p;
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn set_cloud_coverage(&mut self, coverage: f32) {
        self.atmosphere.cloud_coverage = coverage.clamp(0.0, 1.0);
        self.config.cloud_coverage = self.atmosphere.cloud_coverage;
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn set_cloud_albedo_factor(&mut self, factor: f32) {
        self.climate.cloud_albedo_factor = factor.clamp(0.2, 2.0);
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn set_surface_albedo_offset(&mut self, offset: f32) {
        self.climate.surface_albedo_offset = offset.clamp(-0.3, 0.4);
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn set_ocean_heat_capacity(&mut self, capacity: f32) {
        self.config.ocean_heat_capacity = capacity.clamp(0.05, 8.0);
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn set_ocean_circulation(&mut self, value: f32) {
        self.base_ocean_circulation = value.clamp(0.0, 1.0);
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn set_rotation_period_hours(&mut self, hours: f32) {
        self.config.rotation_period_hours = hours.clamp(1.0, 1_000.0);
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn set_gravity(&mut self, gravity: f32) {
        let gravity = gravity.clamp(0.1, 60.0);
        self.config.gravity = gravity;
        self.config.mass_earth = gravity * (self.config.radius_km / 6_371.0).powi(2) / 9.81;
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn set_total_water_km3(&mut self, volume: f64) {
        self.hydrology.total_water_km3 = volume.clamp(1.0e2, 1.0e12);
        hydrology::rebalance_sea_level(&mut self.grid, &self.config, &mut self.hydrology);
        self.config.water.inventory_km3 = self.hydrology.total_water_km3;
        self.grid.apply_sea_level(
            self.hydrology.effective_sea_level_m(),
            self.config.radius_m(),
        );
        self.recompute_stats();
        self.revision = self.revision.wrapping_add(1);
    }

    // ------------------------------------------------------------------
    // Direct local interaction
    // ------------------------------------------------------------------

    pub fn adjust_local_temperature(&mut self, cell: usize, delta_c: f32) {
        if let Some(c) = self.grid.cells.get_mut(cell) {
            c.temp_offset_c = (c.temp_offset_c + delta_c).clamp(-60.0, 60.0);
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub fn add_local_rain(&mut self, cell: usize, mm: f32) {
        if let Some(c) = self.grid.cells.get_mut(cell) {
            c.soil_moisture_m = (c.soil_moisture_m + mm / 1_000.0).min(0.4);
            c.precip_mm_yr += mm;
            c.humidity = (c.humidity + 0.1).min(1.0);
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub fn add_local_drought(&mut self, cell: usize, severity: f32) {
        if let Some(c) = self.grid.cells.get_mut(cell) {
            c.soil_moisture_m = (c.soil_moisture_m - 0.2 * severity.clamp(0.0, 1.0)).max(0.0);
            c.precip_mm_yr *= 0.5;
            c.humidity = (c.humidity - 0.2).max(0.02);
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub fn add_local_water(&mut self, cell: usize, depth_m: f32) {
        let sea = self.sea_level_m();
        if let Some(c) = self.grid.cells.get_mut(cell) {
            if c.bedrock_m >= sea {
                c.lake_depth_m = (c.lake_depth_m + depth_m).clamp(0.0, 500.0);
                self.revision = self.revision.wrapping_add(1);
            }
        }
    }

    // ------------------------------------------------------------------
    // Catastrophe entry points
    // ------------------------------------------------------------------

    pub fn trigger_volcano(
        &mut self,
        cell: usize,
        intensity: f32,
        duration_years: f32,
    ) -> CatastropheOutcome {
        let outcome = catastrophes::trigger_volcano(
            &mut self.grid,
            &mut self.atmosphere,
            &mut self.carbon,
            &mut self.catastrophes,
            cell,
            intensity,
            duration_years,
            self.time_years,
        );
        self.recompute_stats();
        self.revision = self.revision.wrapping_add(1);
        outcome
    }

    pub fn trigger_asteroid(
        &mut self,
        cell: usize,
        diameter_km: f32,
        velocity_km_s: f32,
        angle_deg: f32,
    ) -> (CatastropheOutcome, f64) {
        let result = catastrophes::trigger_asteroid(
            &mut self.grid,
            &mut self.atmosphere,
            &mut self.carbon,
            cell,
            diameter_km,
            velocity_km_s,
            angle_deg,
        );
        self.grid.apply_sea_level(
            self.hydrology.effective_sea_level_m(),
            self.config.radius_m(),
        );
        self.recompute_stats();
        self.revision = self.revision.wrapping_add(1);
        result
    }

    pub fn trigger_weather(
        &mut self,
        kind: WeatherKind,
        cell: usize,
        intensity: f32,
        duration_days: f32,
    ) -> WeatherOutcome {
        let outcome = self.weather.trigger(
            &mut self.grid,
            &mut self.atmosphere,
            kind,
            cell,
            intensity,
            duration_days,
        );
        self.recompute_stats();
        self.revision = self.revision.wrapping_add(1);
        outcome
    }

    pub fn trigger_ice_age(&mut self, duration_years: f32) -> CatastropheOutcome {
        let outcome = catastrophes::trigger_ice_age(&mut self.catastrophes, duration_years);
        self.revision = self.revision.wrapping_add(1);
        outcome
    }

    pub fn trigger_warming(&mut self, ppm: f32) -> CatastropheOutcome {
        let outcome =
            catastrophes::trigger_global_warming(&mut self.atmosphere, &mut self.carbon, ppm);
        self.recompute_stats();
        self.revision = self.revision.wrapping_add(1);
        outcome
    }

    pub fn trigger_rapid_cooling(&mut self, optical_depth: f32) -> CatastropheOutcome {
        let outcome = catastrophes::trigger_rapid_cooling(&mut self.atmosphere, optical_depth);
        self.recompute_stats();
        self.revision = self.revision.wrapping_add(1);
        outcome
    }

    pub fn trigger_ocean_circulation(&mut self, duration_years: f32) -> CatastropheOutcome {
        let outcome =
            catastrophes::trigger_ocean_circulation(&mut self.catastrophes, duration_years);
        self.revision = self.revision.wrapping_add(1);
        outcome
    }

    pub fn trigger_magnetic_reversal(&mut self) -> CatastropheOutcome {
        let outcome = catastrophes::trigger_magnetic_reversal(&mut self.catastrophes);
        self.revision = self.revision.wrapping_add(1);
        outcome
    }

    pub fn active_forcings(&self) -> &[GlobalForcing] {
        &self.catastrophes.forcings
    }

    /// Adds a log-only user event (used by the UI for persistence actions).
    pub fn age_label(&self) -> String {
        format_age(self.time_years)
    }
}

fn step_event(kind: EventKind, title: String, description: String) -> StepEvent {
    StepEvent {
        kind,
        title,
        description,
    }
}

/// Human-readable age with unit switching. Uses a fixed calendar year.
pub fn format_age(years: f64) -> String {
    let y = years.max(0.0);
    if y < 1.0 {
        format!("{:.1} days", y * 365.25)
    } else if y < 1_000.0 {
        format!("{y:.1} years")
    } else if y < 1.0e6 {
        format!("{:.2} thousand years", y / 1_000.0)
    } else if y < 1.0e9 {
        format!("{:.3} million years", y / 1.0e6)
    } else {
        format!("{:.3} billion years", y / 1.0e9)
    }
}

impl PlanetStats {
    fn empty() -> Self {
        Self {
            age_years: 0.0,
            mean_temp_c: 0.0,
            min_temp_c: 0.0,
            max_temp_c: 0.0,
            mean_ocean_temp_c: 0.0,
            land_fraction: 0.0,
            water_fraction: 0.0,
            sea_level_m: 0.0,
            sea_level_anomaly_m: 0.0,
            total_water_km3: 0.0,
            freshwater_km3: 0.0,
            ocean_volume_km3: 0.0,
            ice_volume_km3: 0.0,
            glacier_fraction: 0.0,
            sea_ice_fraction: 0.0,
            river_count: 0,
            lake_count: 0,
            mean_precip_mm_yr: 0.0,
            co2_ppm: 0.0,
            pressure_kpa: 0.0,
            o2_fraction: 0.0,
            n2_fraction: 0.0,
            h2o_fraction: 0.0,
            ch4_ppm: 0.0,
            cloud_coverage: 0.0,
            aerosol_optical_depth: 0.0,
            dust_optical_depth: 0.0,
            active_eruptions: 0,
            plate_count: 0,
            mean_albedo: 0.0,
            greenhouse_strength: 0.0,
            magnetic_field: 1.0,
            runoff_km3_per_yr: 0.0,
            mean_land_elevation_m: 0.0,
            max_elevation_m: 0.0,
            min_elevation_m: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_config(seed: u64) -> PlanetConfig {
        let mut config = PlanetConfig::default();
        config.grid_level = 3;
        config.seed = seed;
        config
    }

    #[test]
    fn generation_is_reproducible_and_bounded() {
        let config = small_config(12345);
        let target = config.water.surface_water_fraction;
        let a = PlanetState::new(config.clone()).expect("planet a");
        let b = PlanetState::new(config).expect("planet b");
        assert_eq!(a.grid.cell_count(), b.grid.cell_count());
        for (ca, cb) in a.grid.cells.iter().zip(&b.grid.cells) {
            assert!((ca.bedrock_m - cb.bedrock_m).abs() < 1.0e-3);
            assert!(ca.bedrock_m.is_finite());
        }
        assert!((a.hydrology.total_water_km3 - b.hydrology.total_water_km3).abs() < 1.0);
        assert!((a.stats.mean_temp_c - b.stats.mean_temp_c).abs() < 1.0e-3);

        let actual = 1.0 - a.stats.land_fraction;
        assert!(
            (actual - target).abs() < 0.06,
            "target={target} actual={actual}"
        );
    }

    #[test]
    fn changing_water_inventory_changes_sea_level() {
        let mut planet = PlanetState::new(small_config(7)).expect("planet");
        let before = planet.hydrology.sea_level_m;
        let total = planet.hydrology.total_water_km3;
        planet.set_total_water_km3(total + 2.0e8);
        assert!(planet.hydrology.sea_level_m > before + 10.0);
        assert!((planet.hydrology.total_water_km3 - (total + 2.0e8)).abs() < 1.0e-3);
    }

    #[test]
    fn co2_redistribution_conserves_carbon_budget() {
        let mut planet = PlanetState::new(small_config(99)).expect("planet");
        let before = planet.carbon.total_ppm(planet.atmosphere.co2_ppm);
        planet.set_co2_ppm(planet.atmosphere.co2_ppm * 2.0, true);
        let after = planet.carbon.total_ppm(planet.atmosphere.co2_ppm);
        assert!((before - after).abs() < 1.0e-2, "{before} vs {after}");
    }

    #[test]
    fn local_actions_modify_the_cell() {
        let mut planet = PlanetState::new(small_config(4)).expect("planet");
        let cell = 0usize;
        let soil = planet.grid.cells[cell].soil_moisture_m;
        planet.add_local_rain(cell, 100.0);
        assert!(planet.grid.cells[cell].soil_moisture_m > soil);
        planet.adjust_local_temperature(cell, 5.0);
        assert!((planet.grid.cells[cell].temp_offset_c - 5.0).abs() < 1.0e-6);
    }
}

#[cfg(test)]
mod event_tests {
    use super::*;

    fn config(seed: u64) -> PlanetConfig {
        let mut config = PlanetConfig::default();
        config.grid_level = 3;
        config.seed = seed;
        config.random_events = false;
        config
    }

    #[test]
    fn climate_stays_finite_over_decades() {
        let mut planet = PlanetState::new(config(11)).expect("planet");
        for _ in 0..20 {
            planet.step_slice(1.0);
        }
        assert!(planet.stats.mean_temp_c.is_finite());
        assert!(planet.stats.mean_temp_c > -80.0 && planet.stats.mean_temp_c < 80.0);
        assert!(planet.stats.mean_precip_mm_yr.is_finite());
        assert!(planet.hydrology.total_water_km3 > 0.0);
        assert!(
            (planet.hydrology.total_water_km3 - planet.config.water.inventory_km3).abs()
                / planet.config.water.inventory_km3
                < 1.0e-6
        );
    }

    #[test]
    fn catastrophes_modify_state() {
        let mut planet = PlanetState::new(config(12)).expect("planet");
        let bedrock_before = planet.grid.cells[0].bedrock_m;
        let (_, energy) = planet.trigger_asteroid(0, 2.0, 20.0, 45.0);
        assert!(energy > 0.0);
        assert!(planet.atmosphere.dust_optical_depth > 0.0);
        assert!((planet.grid.cells[0].bedrock_m - bedrock_before).abs() > 1.0);

        let volcanoes_before = planet.catastrophes.volcanoes.len();
        planet.trigger_volcano(1, 6.0, 10.0);
        assert_eq!(planet.catastrophes.volcanoes.len(), volcanoes_before + 1);
        assert!(planet.atmosphere.aerosol_optical_depth > 0.0);
        planet.step_slice(1.0);
        assert!(planet.stats.sea_level_m.is_finite());
    }

    #[test]
    fn simulation_forks_and_checkpoints_work() {
        let mut sim = crate::simulation::Simulation::new(config(13)).expect("simulation");
        sim.advance_years(5.0);
        let fork = sim.create_fork("twin");
        assert_eq!(fork, 1);
        assert_eq!(sim.forks.len(), 1);
        sim.advance_years(5.0);
        assert!(sim.active_planet().time_years >= 10.0);

        sim.push_checkpoint("manual");
        let checkpoint_time = sim.main.planet.time_years;
        sim.advance_years(25.0);
        let index = sim.checkpoints.len() - 1;
        sim.jump_to_checkpoint(index).expect("restore");
        assert!((sim.main.planet.time_years - checkpoint_time).abs() < 1.0e-6);
        assert!(sim.forks.is_empty());
        assert!(sim.time.paused);

        sim.restart().expect("restart");
        assert_eq!(sim.main.planet.time_years, 0.0);
        assert!(sim.forks.is_empty());
        assert!(sim.checkpoints.len() == 1);
    }
}
