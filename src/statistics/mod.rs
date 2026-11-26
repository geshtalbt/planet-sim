//! Derived planetary statistics. All values are computed from the current
//! simulation fields; nothing here is synthesised for display.

use serde::{Deserialize, Serialize};

use crate::atmosphere::AtmosphereState;
use crate::catastrophes::CatastropheState;
use crate::climate::ClimateState;
use crate::hydrology::HydrologyState;
use crate::planet_generator::PlanetConfig;
use crate::planetary_physics::{greenhouse_strength, planetary_albedo};
use crate::terrain::{Biome, PlanetGrid};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanetStats {
    pub age_years: f64,
    pub mean_temp_c: f32,
    pub min_temp_c: f32,
    pub max_temp_c: f32,
    pub mean_ocean_temp_c: f32,
    pub land_fraction: f32,
    pub water_fraction: f32,
    pub sea_level_m: f32,
    pub sea_level_anomaly_m: f32,
    pub total_water_km3: f64,
    pub freshwater_km3: f64,
    pub ocean_volume_km3: f64,
    pub ice_volume_km3: f64,
    pub glacier_fraction: f32,
    pub sea_ice_fraction: f32,
    pub river_count: u32,
    pub lake_count: u32,
    pub mean_precip_mm_yr: f32,
    pub co2_ppm: f32,
    pub pressure_kpa: f32,
    pub o2_fraction: f32,
    pub n2_fraction: f32,
    pub h2o_fraction: f32,
    pub ch4_ppm: f32,
    pub cloud_coverage: f32,
    pub aerosol_optical_depth: f32,
    pub dust_optical_depth: f32,
    pub active_eruptions: u32,
    pub plate_count: u32,
    pub mean_albedo: f32,
    pub greenhouse_strength: f32,
    pub magnetic_field: f32,
    pub runoff_km3_per_yr: f64,
    pub mean_land_elevation_m: f32,
    pub max_elevation_m: f32,
    pub min_elevation_m: f32,
}

impl PlanetStats {
    pub fn compute(
        grid: &PlanetGrid,
        config: &PlanetConfig,
        atmosphere: &AtmosphereState,
        hydrology: &HydrologyState,
        climate: &ClimateState,
        catastrophes: &CatastropheState,
        time_years: f64,
        _solar_multiplier: f32,
    ) -> Self {
        let total_area = grid.total_area_m2().max(1.0);
        let mut glacier_area = 0.0f64;
        let mut sea_ice_area = 0.0f64;
        let mut min_temp = f32::INFINITY;
        let mut max_temp = f32::NEG_INFINITY;
        let mut land_elev_sum = 0.0f64;
        let mut land_area = 0.0f64;
        let mut min_elev = f32::INFINITY;
        let mut max_elev = f32::NEG_INFINITY;
        let effective_sea = hydrology.effective_sea_level_m();
        for (cell, area) in grid.cells.iter().zip(&grid.cell_area_m2) {
            min_temp = min_temp.min(cell.temp_c);
            max_temp = max_temp.max(cell.temp_c);
            min_elev = min_elev.min(cell.bedrock_m);
            max_elev = max_elev.max(cell.bedrock_m);
            if cell.ice_thickness_m > 0.5 {
                glacier_area += *area as f64;
                if cell.bedrock_m < effective_sea {
                    sea_ice_area += *area as f64;
                }
            }
            if cell.bedrock_m >= effective_sea {
                land_area += *area as f64;
                land_elev_sum += cell.bedrock_m as f64 * *area as f64;
            }
        }
        Self {
            age_years: time_years,
            mean_temp_c: climate.mean_temp_c,
            min_temp_c: if min_temp.is_finite() { min_temp } else { 0.0 },
            max_temp_c: if max_temp.is_finite() { max_temp } else { 0.0 },
            mean_ocean_temp_c: climate.mean_ocean_temp_c,
            land_fraction: (land_area / total_area) as f32,
            water_fraction: (1.0 - land_area / total_area) as f32,
            sea_level_m: effective_sea,
            sea_level_anomaly_m: effective_sea - config.water.sea_level_m,
            total_water_km3: hydrology.total_water_km3,
            freshwater_km3: hydrology.freshwater_km3(),
            ocean_volume_km3: hydrology.ocean_volume_km3,
            ice_volume_km3: hydrology.ice_volume_km3,
            glacier_fraction: (glacier_area / total_area) as f32,
            sea_ice_fraction: (sea_ice_area / total_area) as f32,
            river_count: hydrology.river_count,
            lake_count: hydrology.lake_count,
            mean_precip_mm_yr: hydrology.mean_precip_mm_yr,
            co2_ppm: atmosphere.co2_ppm,
            pressure_kpa: atmosphere.pressure_kpa,
            o2_fraction: atmosphere.o2_fraction,
            n2_fraction: atmosphere.n2_fraction,
            h2o_fraction: atmosphere.h2o_fraction,
            ch4_ppm: atmosphere.ch4_ppm,
            cloud_coverage: atmosphere.cloud_coverage,
            aerosol_optical_depth: atmosphere.aerosol_optical_depth,
            dust_optical_depth: atmosphere.dust_optical_depth,
            active_eruptions: catastrophes.volcanoes.len() as u32,
            plate_count: config.plate_count,
            mean_albedo: planetary_albedo(
                climate.mean_albedo,
                atmosphere.cloud_coverage * climate.cloud_albedo_factor,
            ),
            greenhouse_strength: greenhouse_strength(
                atmosphere.co2_ppm,
                climate.mean_temp_c,
                atmosphere.ch4_ppm,
            ),
            magnetic_field: catastrophes.magnetic_field,
            runoff_km3_per_yr: hydrology.runoff_km3_per_yr,
            mean_land_elevation_m: if land_area > 0.0 {
                (land_elev_sum / land_area) as f32
            } else {
                0.0
            },
            max_elevation_m: if max_elev.is_finite() { max_elev } else { 0.0 },
            min_elevation_m: if min_elev.is_finite() { min_elev } else { 0.0 },
        }
    }

    pub fn dominant_biome(grid: &PlanetGrid) -> Biome {
        let mut counts = std::collections::HashMap::<Biome, f64>::new();
        for (cell, area) in grid.cells.iter().zip(&grid.cell_area_m2) {
            *counts.entry(cell.biome).or_insert(0.0) += *area as f64;
        }
        counts
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(b, _)| b)
            .unwrap_or(Biome::Ocean)
    }
}
