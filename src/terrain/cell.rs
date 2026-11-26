use serde::{Deserialize, Serialize};

use super::biome::Biome;

/// Per-cell environmental state. Elevations are metres relative to the
/// generation datum; sea level is a separate dynamic field, so the same
/// bedrock can become coast, ocean floor or glacier bed as water moves.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SurfaceCell {
    /// Solid surface elevation (negative below the datum).
    pub bedrock_m: f32,
    /// Ocean water depth; updated from sea level every step.
    pub water_depth_m: f32,
    /// Inland lake depth (diagnostic, metres).
    pub lake_depth_m: f32,
    /// Permanent ice / glacier thickness in metres (sea ice is capped).
    pub ice_thickness_m: f32,
    /// Seasonal snow water-equivalent in metres.
    pub snow_depth_m: f32,
    /// Surface air temperature in °C.
    pub temp_c: f32,
    /// Mixed-layer water temperature in °C (ocean/lake cells).
    pub ocean_temp_c: f32,
    /// Plant-available soil water, expressed as a water-equivalent depth (m).
    pub soil_moisture_m: f32,
    pub precip_mm_yr: f32,
    /// Relative humidity, 0..1.
    pub humidity: f32,
    pub pressure_kpa: f32,
    pub cloud_cover: f32,
    pub albedo: f32,
    /// River discharge at this cell in m³/s.
    pub discharge_m3s: f32,
    /// Downhill flow target used to draw the river network.
    pub flow_to: Option<u32>,
    pub biome: Biome,
    pub plate: u16,
    /// Persistent local temperature forcing from user tools.
    pub temp_offset_c: f32,
    /// Short-lived temperature anomaly from impacts, fires and storms.
    pub transient_heat_c: f32,
    /// 0..1 recent volcanic activity for the geology layers.
    pub volcanic_activity: f32,
    /// 0..1 recent seismic activity.
    pub quake_activity: f32,
    /// 0..1 lava coverage used by the natural layer.
    pub lava_flow: f32,
}

impl Default for SurfaceCell {
    fn default() -> Self {
        Self {
            bedrock_m: 0.0,
            water_depth_m: 0.0,
            lake_depth_m: 0.0,
            ice_thickness_m: 0.0,
            snow_depth_m: 0.0,
            temp_c: 15.0,
            ocean_temp_c: 15.0,
            soil_moisture_m: 0.05,
            precip_mm_yr: 0.0,
            humidity: 0.5,
            pressure_kpa: 101.325,
            cloud_cover: 0.5,
            albedo: 0.2,
            discharge_m3s: 0.0,
            flow_to: None,
            biome: Biome::Grassland,
            plate: 0,
            temp_offset_c: 0.0,
            transient_heat_c: 0.0,
            volcanic_activity: 0.0,
            quake_activity: 0.0,
            lava_flow: 0.0,
        }
    }
}

impl SurfaceCell {
    pub fn is_submerged(&self, sea_level_m: f32) -> bool {
        self.bedrock_m < sea_level_m
    }

    pub fn surface_elevation_m(&self, sea_level_m: f32) -> f32 {
        self.bedrock_m.max(sea_level_m)
    }

    pub fn water_surface_elevation_m(&self, sea_level_m: f32) -> f32 {
        if self.is_submerged(sea_level_m) {
            sea_level_m
        } else {
            self.bedrock_m + self.lake_depth_m
        }
    }

    pub fn is_glaciated(&self) -> bool {
        self.ice_thickness_m > 1.0
    }

    /// Local hydrostatic pressure with a simple exponential atmosphere.
    pub fn local_pressure_kpa(
        &self,
        sea_level_m: f32,
        sea_level_pressure_kpa: f32,
        scale_height_m: f32,
    ) -> f32 {
        let height = (self.bedrock_m - sea_level_m).max(0.0);
        sea_level_pressure_kpa * (-height / scale_height_m.max(100.0)).exp()
    }

    pub fn local_gravity_effect(&self) -> f32 {
        // Placeholder hook for future isostasy; kept explicit so callers do
        // not assume a physical effect that is not implemented yet.
        1.0
    }
}
