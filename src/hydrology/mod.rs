//! Simplified, mass-conserving hydrological cycle.
//!
//! Water is distributed between the ocean, ice, soil and (diagnostic) lakes.
//! Evaporation is computed locally, precipitation is redistributed globally
//! from the evaporated water, and runoff is routed downhill on the cell
//! graph. The total ocean + ice + soil budget is held constant; sea level is
//! solved from that budget each step.
//!
//! Lakes and rivers are derived from the routed water balance. Lake storage
//! is reported but not allowed to drive eustatic sea level (their volume is
//! small compared with the ocean and the basin geometry is approximate).

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::planet_generator::PlanetConfig;
use crate::planetary_physics::{SECONDS_PER_YEAR, thermal_expansion_m};
use crate::terrain::PlanetGrid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HydrologyState {
    /// Total water inventory (ocean + ice + soil) in km³.
    pub total_water_km3: f64,
    /// Mass-equivalent ocean volume in km³.
    pub ocean_volume_km3: f64,
    pub ocean_area_km2: f64,
    /// Eustatic (water-mass) sea level in metres relative to the datum.
    pub sea_level_m: f32,
    /// Additional sea level from thermal expansion, metres.
    pub thermal_expansion_m: f32,
    pub ice_volume_km3: f64,
    pub soil_water_km3: f64,
    pub lake_volume_km3: f64,
    pub atmosphere_water_km3: f64,
    pub mean_ocean_depth_m: f32,
    pub mean_precip_mm_yr: f32,
    pub runoff_km3_per_yr: f64,
    pub river_count: u32,
    pub lake_count: u32,
    pub initial_ocean_temp_c: f32,
}

impl HydrologyState {
    pub fn effective_sea_level_m(&self) -> f32 {
        self.sea_level_m + self.thermal_expansion_m
    }

    pub fn freshwater_km3(&self) -> f64 {
        (self.total_water_km3 - self.ocean_volume_km3).max(0.0)
    }
}

/// Initialises the water budget and solves the initial sea level so that
/// ocean + ice + soil matches the configured inventory.
pub fn initialize_hydrology(
    grid: &mut PlanetGrid,
    config: &PlanetConfig,
    initial_ocean_temp_c: f32,
) -> HydrologyState {
    let (_, base_ocean_volume) = grid.ocean_metrics_at(0.0);
    let target = if config.water.inventory_km3 > 0.0 {
        config.water.inventory_km3
    } else {
        base_ocean_volume.max(1.0) * 1.05
    };

    let (ice, soil) = ice_and_soil_volumes(grid);
    let mut state = HydrologyState {
        total_water_km3: target,
        ocean_volume_km3: 0.0,
        ocean_area_km2: 0.0,
        sea_level_m: 0.0,
        thermal_expansion_m: 0.0,
        ice_volume_km3: ice,
        soil_water_km3: soil,
        lake_volume_km3: 0.0,
        atmosphere_water_km3: 0.0,
        mean_ocean_depth_m: 0.0,
        mean_precip_mm_yr: 1_000.0,
        runoff_km3_per_yr: 0.0,
        river_count: 0,
        lake_count: 0,
        initial_ocean_temp_c,
    };
    rebalance_sea_level(grid, config, &mut state);
    state.mean_precip_mm_yr = grid.weighted_mean(|c| c.precip_mm_yr);
    state
}

/// Advances evaporation, precipitation, soil moisture, snow/ice, runoff,
/// rivers, lakes and sea level by `dt_years`.
pub fn hydrology_step(
    grid: &mut PlanetGrid,
    config: &PlanetConfig,
    state: &mut HydrologyState,
    mean_ocean_temp_c: f32,
    dt_years: f32,
) {
    let dt = dt_years.max(0.0);
    if dt == 0.0 {
        return;
    }
    let n = grid.cell_count();
    if n == 0 {
        return;
    }
    let sea_level = state.effective_sea_level_m();
    let area_total_km2 = grid.total_area_m2() / 1.0e6;

    // --- Potential evaporation (m/yr) -----------------------------------
    let evaporation: Vec<f32> = grid
        .cells
        .par_iter()
        .map(|c| {
            let t = c.temp_c.max(-60.0);
            if c.water_depth_m > 0.0 || c.lake_depth_m > 0.05 {
                let ice_factor = (1.0 - c.ice_thickness_m.min(2.0) * 0.45).clamp(0.05, 1.0);
                ((0.30 + 0.030 * t.max(0.0)) * ice_factor).clamp(0.0, 2.5)
            } else {
                let moisture = (c.soil_moisture_m / 0.20).clamp(0.0, 1.0);
                ((0.015 + 0.0025 * t.max(0.0)) * moisture).clamp(0.0, 1.2)
            }
        })
        .collect();

    let total_evap_km3_per_yr: f64 = evaporation
        .iter()
        .zip(&grid.cell_area_m2)
        .map(|(e, a)| *e as f64 * *a as f64 / 1.0e6 / 1_000.0)
        .sum();

    // --- Precipitation distribution -------------------------------------
    let weights: Vec<f32> = grid
        .cells
        .par_iter()
        .enumerate()
        .map(|(cell, c)| {
            let lat = grid.lat_deg(cell).to_radians();
            let wet_band = (-(lat / 0.42).powi(2)).exp();
            let relief = ((c.bedrock_m - sea_level).max(0.0) / 1_800.0).min(2.5);
            let cold = (0.35 + c.temp_c.max(0.0) / 30.0).min(1.2);
            let ice_barrier = (1.0 - c.ice_thickness_m.min(3.0) / 4.5).clamp(0.25, 1.0);
            ((0.12 + 0.85 * wet_band + 0.55 * relief + 0.40 * c.humidity) * cold * ice_barrier)
                .max(0.001)
        })
        .collect();
    let weight_sum: f64 = weights.iter().map(|w| *w as f64).sum();
    let mean_precip_m_per_yr = if area_total_km2 > 0.0 {
        total_evap_km3_per_yr / area_total_km2 * 1_000.0
    } else {
        0.0
    };
    // `weights` is an unnormalised spatial pattern. Normalise by its mean so
    // that the area-weighted global precipitation equals the evaporated
    // volume, while preserving wet/dry contrasts.
    let weight_mean = (weight_sum / n.max(1) as f64).max(1.0e-9);

    // --- Soil, snow and ice update --------------------------------------
    let mut local_runoff_m3s = vec![0.0f32; n];
    let mut runoff_km3_per_yr = 0.0f64;
    for cell in 0..n {
        let cell_area = grid.area_m2(cell);
        let c = &mut grid.cells[cell];
        let p = mean_precip_m_per_yr as f32 * (weights[cell] as f64 / weight_mean) as f32;
        let evapor = evaporation[cell];
        let submerged = c.bedrock_m < sea_level;
        let mut excess = 0.0f32;

        // Snowfall and glaciers.
        if c.temp_c < 0.5 {
            let snow = p * dt * (1.0 - (c.temp_c / 4.0).max(0.0) * 0.35);
            let cap = if submerged { 1.5 } else { 3_500.0 };
            let room = (cap - c.ice_thickness_m).max(0.0);
            let stored = snow.min(room);
            c.ice_thickness_m += stored;
            c.snow_depth_m = (c.snow_depth_m + stored).min(5.0);
            excess += (snow - stored).max(0.0);
        }
        // Melting.
        if c.temp_c > 0.0 && c.ice_thickness_m > 0.0 {
            let melt = ((0.12 + 0.008 * c.temp_c) * dt).min(c.ice_thickness_m);
            c.ice_thickness_m -= melt;
            c.snow_depth_m = (c.snow_depth_m - melt).max(0.0);
            if !submerged {
                c.soil_moisture_m = (c.soil_moisture_m + melt).min(0.6);
                excess += (c.soil_moisture_m - 0.4).max(0.0);
                c.soil_moisture_m = c.soil_moisture_m.min(0.4);
            }
        }
        // A slow geothermal/basal melt prevents a completely frozen world
        // from becoming a permanent numerical dead end. The meltwater
        // returns to the ocean reservoir through the sea-level solve.
        if c.ice_thickness_m > 25.0 && c.temp_c <= 0.0 {
            let basal = (0.000_2 * dt).min(c.ice_thickness_m);
            c.ice_thickness_m -= basal;
            if !submerged {
                c.soil_moisture_m = (c.soil_moisture_m + basal * 0.25).min(0.4);
            }
        }

        if submerged {
            c.soil_moisture_m = 0.0;
            c.lake_depth_m = 0.0;
            c.precip_mm_yr = p * 1_000.0;
            c.humidity = (c.humidity * 0.8 + 0.9 * p.min(1.0)).clamp(0.0, 1.0);
        } else {
            let rain = if c.temp_c > 0.0 { p * dt } else { 0.0 };
            c.soil_moisture_m += rain - evapor * dt;
            if c.soil_moisture_m > 0.4 {
                excess += c.soil_moisture_m - 0.4;
                c.soil_moisture_m = 0.4;
            }
            if c.soil_moisture_m < 0.0 {
                c.soil_moisture_m = 0.0;
            }
            c.precip_mm_yr = p * 1_000.0;
            c.humidity =
                (0.25 + 0.65 * (p * 1.6).min(1.0) - 0.1 * (c.temp_c / 40.0)).clamp(0.02, 1.0);
            // A fraction of rainfall runs off directly even before the soil
            // bucket saturates. Frozen precipitation is handled as snow above.
            if c.temp_c > 0.0 {
                excess += p * dt * 0.18;
            }
            let rate = (excess / dt.max(1.0e-9) * cell_area / SECONDS_PER_YEAR as f32).max(0.0);
            local_runoff_m3s[cell] = rate;
            runoff_km3_per_yr += rate as f64 * SECONDS_PER_YEAR / 1.0e9;
        }
        c.discharge_m3s = local_runoff_m3s[cell];
        c.flow_to = None;
        c.lake_depth_m = 0.0;
    }

    // --- River routing ---------------------------------------------------
    route_rivers(grid, sea_level);

    // --- Global reservoirs and sea level ---------------------------------
    let (mut ice, soil) = ice_and_soil_volumes(grid);
    // Do not let land ice consume the entire water inventory. Without this
    // cap, snowfall on newly exposed shelves can freeze the ocean, expose
    // more shelf, freeze that too, and collapse into a dry ice world. Sea ice
    // is deliberately not scaled: thin sea ice is essential for a snowball's
    // high albedo and must remain physically represented.
    let max_ice = state.total_water_km3 * 0.12;
    if ice > max_ice && ice > 0.0 {
        let sea_level = state.effective_sea_level_m();
        let mut sea_ice = 0.0;
        for (c, a) in grid.cells.iter().zip(&grid.cell_area_m2) {
            if c.bedrock_m < sea_level {
                sea_ice += (*a as f64 / 1.0e6) * (c.ice_thickness_m as f64 / 1_000.0);
            }
        }
        let land_budget = (max_ice - sea_ice).max(0.0);
        let mut land_ice = 0.0;
        for (c, a) in grid.cells.iter().zip(&grid.cell_area_m2) {
            if c.bedrock_m >= sea_level {
                land_ice += (*a as f64 / 1.0e6) * (c.ice_thickness_m as f64 / 1_000.0);
            }
        }
        if land_ice > land_budget && land_ice > 0.0 {
            let scale = (land_budget / land_ice) as f32;
            for c in &mut grid.cells {
                if c.bedrock_m >= sea_level {
                    c.ice_thickness_m *= scale;
                    c.snow_depth_m *= scale;
                }
            }
        }
        ice = max_ice.max(sea_ice);
    }
    state.ice_volume_km3 = ice;
    state.soil_water_km3 = soil;
    state.mean_precip_mm_yr = grid.weighted_mean(|c| c.precip_mm_yr);
    state.atmosphere_water_km3 = 0.0;
    state.runoff_km3_per_yr = runoff_km3_per_yr;
    state.thermal_expansion_m = thermal_expansion_m(
        mean_ocean_temp_c - state.initial_ocean_temp_c,
        state.mean_ocean_depth_m.max(1_000.0),
    );
    rebalance_sea_level(grid, config, state);
    state.ocean_area_km2 = grid.ocean_area_km2();
    if state.ocean_area_km2 > 0.0 {
        state.mean_ocean_depth_m = (state.ocean_volume_km3 / state.ocean_area_km2 * 1_000.0) as f32;
    }
    state.lake_volume_km3 = grid
        .cells
        .iter()
        .zip(&grid.cell_area_m2)
        .map(|(c, a)| (*a as f64 / 1.0e6) * (c.lake_depth_m as f64 / 1_000.0))
        .sum();
    state.river_count = grid
        .cells
        .iter()
        .filter(|c| c.discharge_m3s > 20_000.0 && c.bedrock_m >= state.effective_sea_level_m())
        .count() as u32;
    state.lake_count = grid.cells.iter().filter(|c| c.lake_depth_m > 0.5).count() as u32;
}

fn ice_and_soil_volumes(grid: &PlanetGrid) -> (f64, f64) {
    let mut ice = 0.0;
    let mut soil = 0.0;
    for (c, a) in grid.cells.iter().zip(&grid.cell_area_m2) {
        let a = *a as f64 / 1.0e6;
        ice += a * (c.ice_thickness_m as f64 / 1_000.0);
        soil += a * (c.soil_moisture_m as f64 / 1_000.0);
    }
    (ice, soil)
}

pub fn rebalance_sea_level(
    grid: &mut PlanetGrid,
    config: &PlanetConfig,
    state: &mut HydrologyState,
) {
    let non_ocean = state.ice_volume_km3 + state.soil_water_km3;
    let target_ocean = (state.total_water_km3 - non_ocean).max(0.0);
    let min_bedrock = grid
        .cells
        .iter()
        .map(|c| c.bedrock_m)
        .fold(f32::INFINITY, f32::min);
    let max_bedrock = grid
        .cells
        .iter()
        .map(|c| c.bedrock_m)
        .fold(f32::NEG_INFINITY, f32::max);
    let mut sea = state.sea_level_m.max(min_bedrock + 1.0);
    if target_ocean <= 1.0e-6 {
        sea = min_bedrock - 1.0;
    }
    if target_ocean > 1.0e-6 {
        for _ in 0..8 {
            let (area, volume) = grid.ocean_metrics_at(sea);
            if area < 1.0 {
                // A fully land-covered world: raise the sea until a basin
                // floods, otherwise the water budget would be silently
                // unrepresented.
                sea = (sea + 1.0).min(max_bedrock + 1.0);
                if sea >= max_bedrock + 1.0 {
                    break;
                }
                continue;
            }
            let error = target_ocean - volume;
            let step = (error / area * 1_000.0) as f32;
            sea += step.clamp(-20_000.0, 20_000.0);
            if error.abs() < state.total_water_km3.max(1.0) * 1.0e-6 {
                break;
            }
        }
    }
    // Keep the sea level within a physically meaningful range even when an
    // extreme water inventory is requested.
    sea = sea.clamp(
        (min_bedrock - 1.0).max(-4_000.0),
        (max_bedrock + 1.0).min(8_000.0),
    );
    state.sea_level_m = sea;
    let effective = state.effective_sea_level_m();
    grid.apply_sea_level(effective, config.radius_m());
    let (area, volume) = grid.ocean_metrics_at(sea);
    state.ocean_area_km2 = area;
    state.ocean_volume_km3 = volume.max(0.0);
    if (state.ocean_volume_km3 - target_ocean).abs() > target_ocean.max(1.0) * 1.0e-3 {
        // The requested inventory does not fit in the available basins (for
        // example, a planet with no ocean and an enormous water inventory).
        // Keep the reported budget consistent with the stored water.
        state.total_water_km3 = state.ocean_volume_km3 + non_ocean;
    }
    if area > 0.0 {
        state.mean_ocean_depth_m = (state.ocean_volume_km3 / area * 1_000.0) as f32;
    } else {
        state.mean_ocean_depth_m = 0.0;
    }
}

fn route_rivers(grid: &mut PlanetGrid, sea_level: f32) {
    let n = grid.cell_count();
    let mut order: Vec<usize> = (0..n)
        .filter(|&i| grid.cells[i].bedrock_m >= sea_level)
        .collect();
    order.sort_by(|&a, &b| {
        grid.cells[b]
            .bedrock_m
            .partial_cmp(&grid.cells[a].bedrock_m)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut discharge: Vec<f32> = grid
        .cells
        .iter()
        .map(|c| c.discharge_m3s.max(0.0))
        .collect();
    let mut flow_to: Vec<Option<u32>> = vec![None; n];
    let mut lake_depth = vec![0.0f32; n];

    for &cell in &order {
        let mut best = None;
        let mut best_h = grid.cells[cell].bedrock_m;
        for &nb in grid.neighbors(cell) {
            let h = grid.cells[nb as usize].bedrock_m;
            if h < best_h - 0.01 {
                best_h = h;
                best = Some(nb);
            }
        }
        if let Some(target) = best {
            flow_to[cell] = Some(target);
            discharge[target as usize] += discharge[cell];
        } else if discharge[cell] > 5.0 {
            // Local minimum: a lake is the simplest stable hydrological sink.
            let depth = ((discharge[cell] / 3.0).sqrt() * 4.0).clamp(0.0, 180.0);
            lake_depth[cell] = depth;
        }
    }

    for (cell, c) in grid.cells.iter_mut().enumerate() {
        c.discharge_m3s = discharge[cell];
        c.flow_to = flow_to[cell];
        c.lake_depth_m = if c.bedrock_m >= sea_level {
            lake_depth[cell]
        } else {
            0.0
        };
    }
}

#[cfg(test)]
mod tests {
    use crate::planet_generator::PlanetConfig;
    use crate::simulation::PlanetState;

    #[test]
    fn water_budget_is_conserved() {
        let mut cfg = PlanetConfig::default();
        cfg.grid_level = 3;
        cfg.water.inventory_km3 = 1.0e9;
        let mut planet = PlanetState::new(cfg).expect("planet");
        let before = planet.hydrology.total_water_km3;
        planet.step_slice(10.0);
        let after = planet.hydrology.total_water_km3;
        assert!((before - after).abs() / before < 1.0e-6);
        assert!(planet.hydrology.sea_level_m.is_finite());
    }
}
