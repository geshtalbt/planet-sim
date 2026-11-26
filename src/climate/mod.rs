//! Surface energy balance, temperature field and biome evolution.

use glam::Vec3;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::atmosphere::AtmosphereState;
use crate::planet_generator::PlanetConfig;
use crate::planetary_physics::{
    annual_mean_insolation, daily_insolation, declination_rad, equilibrium_temperature_c,
    greenhouse_strength, planetary_albedo, relax, scale_height_m,
};
use crate::terrain::{Biome, PlanetGrid};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClimateState {
    pub mean_temp_c: f32,
    pub min_temp_c: f32,
    pub max_temp_c: f32,
    pub mean_ocean_temp_c: f32,
    pub mean_albedo: f32,
    /// User-controlled surface albedo perturbation (persists across steps).
    pub surface_albedo_offset: f32,
    pub ice_fraction: f32,
    /// 0..1 multiplier on the simplified ocean heat transport.
    pub ocean_circulation: f32,
    /// 0..1 multiplier on cloud albedo (clouds can be geoengineered in UI).
    pub cloud_albedo_factor: f32,
}

impl Default for ClimateState {
    fn default() -> Self {
        Self {
            mean_temp_c: 15.0,
            min_temp_c: -30.0,
            max_temp_c: 40.0,
            mean_ocean_temp_c: 17.0,
            mean_albedo: 0.3,
            surface_albedo_offset: 0.0,
            ice_fraction: 0.0,
            ocean_circulation: 1.0,
            cloud_albedo_factor: 1.0,
        }
    }
}

/// Initialises the per-cell temperature, humidity, pressure and biome fields
/// from the configured global state and latitude.
pub fn initialize_climate(
    grid: &mut PlanetGrid,
    config: &PlanetConfig,
    atmosphere: &AtmosphereState,
    sea_level_m: f32,
) {
    let radius_m = config.radius_m();
    grid.rebuild_areas(radius_m);
    let sea_level = sea_level_m;
    let flux = config.stellar_flux_wm2;
    let scale_height = scale_height_m(
        config.mean_surface_temp_c,
        config.gravity,
        atmosphere.mean_molar_mass_g(),
    );
    let n = grid.cell_count();
    let mut targets = vec![0.0f32; n];
    let mut mean = 0.0f64;
    let mut area_sum = 0.0f64;

    for cell in 0..n {
        let dir: Vec3 = grid.direction(cell);
        let lat = dir.y.clamp(-1.0, 1.0).asin();
        let insol = annual_mean_insolation(flux, lat);
        let mean_insol = flux / 4.0;
        let latitude_bias = (insol / mean_insol - 1.0) * 34.0;
        let elevation = (grid.cells[cell].bedrock_m - sea_level).max(0.0);
        let target = config.mean_surface_temp_c + latitude_bias - elevation * 0.0065;
        targets[cell] = target;
        mean += target as f64 * grid.area_m2(cell) as f64;
        area_sum += grid.area_m2(cell) as f64;
    }
    let offset = config.mean_surface_temp_c - (mean / area_sum.max(1.0)) as f32;

    for cell in 0..n {
        let dir = grid.direction(cell);
        let lat = dir.y.clamp(-1.0, 1.0).asin();
        let submerged = grid.cells[cell].bedrock_m < sea_level;
        let temp = targets[cell] + offset;
        let c = &mut grid.cells[cell];
        c.temp_c = temp;
        c.ocean_temp_c = if submerged {
            (config.ocean_temp_c + (temp - config.mean_surface_temp_c) * 0.7).clamp(-80.0, 80.0)
        } else {
            temp
        };
        // First-order precipitation climatology: wet tropics, dry subtropics
        // and poles, enhanced by orography. The hydrological model replaces
        // this with a normalised, mass-conserving distribution each step.
        let wet_band = (-((lat.to_degrees() / 24.0).powi(2))).exp();
        let orographic = 1.0 + (c.bedrock_m.max(0.0) / 2_500.0).min(2.0);
        c.precip_mm_yr = (180.0 + 1_500.0 * wet_band * orographic).clamp(0.0, 6_000.0);
        c.humidity = (0.25 + 0.55 * wet_band).clamp(0.05, 0.98);
        c.pressure_kpa = c.local_pressure_kpa(sea_level, atmosphere.pressure_kpa, scale_height);
        c.cloud_cover = (atmosphere.cloud_coverage * 0.75 + c.humidity * 0.25).clamp(0.0, 1.0);
        c.soil_moisture_m = (0.03 + c.precip_mm_yr / 4_000.0 * 0.22).clamp(0.0, 0.4);
        c.temp_offset_c = 0.0;
        c.transient_heat_c = 0.0;
        c.volcanic_activity = 0.0;
        c.quake_activity = 0.0;
        c.lava_flow = 0.0;
        c.albedo = if c.ice_thickness_m > 0.5 {
            0.70
        } else {
            c.biome.base_albedo()
        };
        update_cell_biome(c, submerged);
    }
}

/// Advances the surface energy balance and biome state.
///
/// `dt_years` may be very large (geological speeds); the relaxation scheme is
/// stable for any positive step. Seasonal contrast is only resolved when the
/// step is shorter than two years, which is documented behaviour rather than
/// a hidden animation.
pub fn update_climate(
    grid: &mut PlanetGrid,
    config: &PlanetConfig,
    atmosphere: &AtmosphereState,
    climate: &mut ClimateState,
    sea_level_m: f32,
    time_years: f64,
    dt_years: f32,
    solar_multiplier: f32,
) {
    let dt = dt_years.max(0.0);
    if dt == 0.0 {
        return;
    }
    let n = grid.cell_count();
    if n == 0 {
        return;
    }
    let sea_level = sea_level_m;
    let flux = config.stellar_flux_wm2 * solar_multiplier.max(0.0);
    let scale_height = scale_height_m(
        climate.mean_temp_c,
        config.gravity,
        atmosphere.mean_molar_mass_g(),
    );
    let transmission = atmosphere.surface_transmission();
    let cloud = (atmosphere.cloud_coverage * climate.cloud_albedo_factor).clamp(0.0, 1.0);
    let surface_albedo = grid.weighted_mean(|c| c.albedo);
    let planet_albedo = planetary_albedo(surface_albedo, cloud);
    let absorbed = flux * (1.0 - planet_albedo) * transmission / 4.0;
    let greenhouse =
        greenhouse_strength(atmosphere.co2_ppm, climate.mean_temp_c, atmosphere.ch4_ppm);
    let global_equilibrium = equilibrium_temperature_c(absorbed, greenhouse);

    let seasonal = dt < 2.0;
    let day = ((time_years * 365.25) % 365.25) as f32;
    let declination = declination_rad(config.axial_tilt_deg, day);
    let old_temps: Vec<f32> = grid.cells.iter().map(|c| c.temp_c).collect();
    let annual_mean_flux = (flux / 4.0).max(1.0e-3);
    let ocean_capacity = config.ocean_heat_capacity.clamp(0.05, 8.0);
    let ocean_circulation = climate.ocean_circulation.clamp(0.0, 1.0);

    let temp_updates: Vec<(f32, f32)> = (0..n)
        .into_par_iter()
        .map(|cell| {
            let dir = grid.direction(cell);
            let lat = dir.y.clamp(-1.0, 1.0).asin();
            let annual = annual_mean_insolation(flux, lat);
            let (local_target, ocean_target) = {
                let c = &grid.cells[cell];
                let elevation = (c.bedrock_m - sea_level).max(0.0);
                let seasonal_ratio = if seasonal {
                    let daily = daily_insolation(flux, lat, declination);
                    (daily / annual.max(1.0)).clamp(0.0, 4.0)
                } else {
                    1.0
                };
                let latitude_bias = (annual / annual_mean_flux - 1.0) * 34.0;
                let seasonal_bias = ((seasonal_ratio - 1.0) * 22.0).clamp(-28.0, 28.0);
                let mut target =
                    global_equilibrium + latitude_bias + seasonal_bias - elevation * 0.0065;
                target += c.temp_offset_c + c.transient_heat_c;

                let submerged = c.bedrock_m < sea_level;
                let ocean_target = if submerged {
                    // Ocean circulation transports heat meridionally; reducing
                    // it strengthens the local radiative response.
                    let mixed = target * (1.0 - 0.22 * ocean_circulation)
                        + global_equilibrium * (0.22 * ocean_circulation);
                    mixed
                } else {
                    target
                };
                (
                    target.clamp(-150.0, 150.0),
                    ocean_target.clamp(-150.0, 150.0),
                )
            };
            let c = &grid.cells[cell];
            let tau_land = if c.ice_thickness_m > 0.5 { 0.35 } else { 0.12 };
            let tau_ocean = (2.4 * ocean_capacity).clamp(0.2, 40.0);
            let new_temp = relax(c.temp_c, local_target, dt, tau_land);
            let new_ocean = if c.bedrock_m < sea_level {
                relax(c.ocean_temp_c, ocean_target, dt, tau_ocean)
            } else {
                new_temp
            };
            (new_temp, new_ocean)
        })
        .collect();

    for (cell, (t, ot)) in temp_updates.into_iter().enumerate() {
        grid.cells[cell].temp_c = t;
        grid.cells[cell].ocean_temp_c = ot;
    }

    // Horizontal heat transport on short steps only. Over geological steps
    // the equilibrium latitude profile already parameterises transport, and
    // diffusing it to the global mean would be nonphysical.
    if dt < 10.0 {
        let mix = (dt * 0.12).clamp(0.0, 0.10);
        let mut diffused = vec![0.0f32; n];
        diffused.par_iter_mut().enumerate().for_each(|(cell, out)| {
            let neighbors = grid.neighbors(cell);
            if neighbors.is_empty() {
                *out = grid.cells[cell].temp_c;
                return;
            }
            let mean = neighbors
                .iter()
                .map(|&nb| old_temps[nb as usize])
                .sum::<f32>()
                / neighbors.len() as f32;
            *out = grid.cells[cell].temp_c * (1.0 - mix) + mean * mix;
        });
        for (cell, value) in diffused.into_iter().enumerate() {
            grid.cells[cell].temp_c = value;
        }
    }

    // Pressure follows surface elevation; albedo and biomes follow the new
    // temperature, precipitation and ice state.
    grid.cells
        .par_iter_mut()
        .enumerate()
        .for_each(|(_cell, c)| {
            let submerged = c.bedrock_m < sea_level;
            c.pressure_kpa = c.local_pressure_kpa(sea_level, atmosphere.pressure_kpa, scale_height);
            c.cloud_cover = (atmosphere.cloud_coverage * 0.72 + c.humidity * 0.28).clamp(0.02, 1.0);
            c.albedo = if c.ice_thickness_m > 0.25 || c.snow_depth_m > 0.02 {
                (0.58 + 0.06 * c.ice_thickness_m.min(2.0)).min(0.70)
            } else if submerged {
                Biome::Ocean.base_albedo()
            } else {
                (c.biome.base_albedo() - 0.05 * (c.soil_moisture_m / 0.4).clamp(0.0, 1.0)
                    + climate.surface_albedo_offset)
                    .clamp(0.03, 0.9)
            };
            update_cell_biome(c, submerged);
        });

    recompute_summary(grid, climate);
}

/// Recomputes the area-weighted summary fields after initialisation or an
/// external modification.
pub fn recompute_summary(grid: &PlanetGrid, climate: &mut ClimateState) {
    climate.mean_temp_c = grid.weighted_mean(|c| c.temp_c);
    climate.min_temp_c = grid
        .cells
        .iter()
        .map(|c| c.temp_c)
        .fold(f32::INFINITY, f32::min);
    climate.max_temp_c = grid
        .cells
        .iter()
        .map(|c| c.temp_c)
        .fold(f32::NEG_INFINITY, f32::max);
    climate.mean_albedo = grid.weighted_mean(|c| c.albedo);
    let total = grid.total_area_m2().max(1.0) as f32;
    climate.ice_fraction = grid
        .cells
        .iter()
        .zip(&grid.cell_area_m2)
        .filter(|(c, _)| c.ice_thickness_m > 0.5)
        .map(|(_, a)| *a as f32)
        .sum::<f32>()
        / total;
    let (sum, area) = grid
        .cells
        .iter()
        .zip(&grid.cell_area_m2)
        .filter(|(c, _)| c.water_depth_m > 0.0)
        .fold((0.0f64, 0.0f64), |acc, (c, a)| {
            (acc.0 + c.ocean_temp_c as f64 * *a as f64, acc.1 + *a as f64)
        });
    if area > 0.0 {
        climate.mean_ocean_temp_c = (sum / area) as f32;
    }
}

fn update_cell_biome(c: &mut crate::terrain::SurfaceCell, submerged: bool) {
    if submerged {
        c.biome = Biome::Ocean;
        if c.ice_thickness_m <= 0.5 {
            c.albedo = Biome::Ocean.base_albedo();
        }
    } else if c.lake_depth_m > 0.05 {
        c.biome = Biome::Lake;
    } else {
        c.biome = Biome::classify(c.temp_c, c.precip_mm_yr, c.ice_thickness_m);
    }
}
