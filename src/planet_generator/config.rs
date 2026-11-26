use serde::{Deserialize, Serialize};

/// All user-editable parameters used to create a planet.
///
/// Values are stored in SI-adjacent scientific units. Some fields are targets
/// that are reconciled during generation (for example water inventory and the
/// average ocean depth cannot both be chosen freely); [`PlanetConfig::normalize_after_generation`]
/// records the coherent values that were actually produced.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanetConfig {
    pub name: String,
    pub seed: u64,
    /// Icosphere subdivision level, 3..=6 exposed in the UI (7 is supported by the core).
    pub grid_level: u8,

    pub radius_km: f32,
    pub mass_earth: f32,
    /// Surface gravity in m/s².
    pub gravity: f32,

    /// Top-of-atmosphere stellar flux in W/m² (Earth = 1361).
    pub stellar_flux_wm2: f32,
    /// Initial global mean surface temperature in °C.
    pub mean_surface_temp_c: f32,
    /// Initial mixed-layer ocean temperature in °C.
    pub ocean_temp_c: f32,
    /// Surface pressure in kPa (Earth = 101.325).
    pub surface_pressure_kpa: f32,

    pub co2_ppm: f32,
    pub o2_fraction: f32,
    pub n2_fraction: f32,
    /// Atmospheric water vapour volume fraction (Earth ≈ 0.01).
    pub h2o_fraction: f32,
    pub ch4_ppm: f32,

    pub rotation_period_hours: f32,
    /// Axial tilt in degrees.
    pub axial_tilt_deg: f32,
    /// Global surface albedo used as the initial value / fallback.
    pub albedo: f32,
    pub cloud_coverage: f32,
    /// Relative mixed-layer heat capacity multiplier (Earth ocean = 1.0).
    pub ocean_heat_capacity: f32,

    pub water: WaterConfig,
    /// Fraction of the surface initially covered by permanent ice / sea ice.
    pub initial_glacier_fraction: f32,

    pub plate_count: u32,
    /// Multiplier for the probability of weather events.
    pub weather_activity: f32,
    pub random_events: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaterConfig {
    /// Total water inventory in km³. This is the authoritative water budget.
    pub inventory_km3: f64,
    /// Target fraction of the surface below sea level.
    pub surface_water_fraction: f32,
    /// Target mean ocean depth in metres, used when `inventory_km3 <= 0`.
    pub avg_ocean_depth_m: f32,
    /// Target deepest ocean depth in metres (used as a generation hint).
    pub max_ocean_depth_m: f32,
    /// Target mean land elevation above sea level in metres.
    pub avg_land_elevation_m: f32,
    pub continents: u32,
    pub islands: u32,
    /// Sea level in metres relative to the generation datum. Recomputed by
    /// the hydrological model as water is stored in ice, soil and lakes.
    pub sea_level_m: f32,
}

impl Default for PlanetConfig {
    fn default() -> Self {
        presets::earthlike()
    }
}

impl PlanetConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Planet name must not be empty".into());
        }
        if !(500.0..=100_000.0).contains(&self.radius_km) {
            return Err("Radius must be between 500 and 100,000 km".into());
        }
        if !(0.01..=50.0).contains(&self.mass_earth) {
            return Err("Mass must be between 0.01 and 50 Earth masses".into());
        }
        if !(0.1..=60.0).contains(&self.gravity) {
            return Err("Surface gravity must be between 0.1 and 60 m/s²".into());
        }
        if !(50.0..=10_000.0).contains(&self.stellar_flux_wm2) {
            return Err("Stellar flux must be between 50 and 10,000 W/m²".into());
        }
        if !(-150.0..=200.0).contains(&self.mean_surface_temp_c) {
            return Err("Initial temperature must be between -150 and 200 °C".into());
        }
        if !(0.1..=2_000.0).contains(&self.surface_pressure_kpa) {
            return Err("Surface pressure must be between 0.1 and 2,000 kPa".into());
        }
        if !(0.1..=5_000_000.0).contains(&self.co2_ppm) {
            return Err("CO₂ concentration must be between 0.1 and 5,000,000 ppm".into());
        }
        if !(0.0..=1.0).contains(&self.water.surface_water_fraction) {
            return Err("Water coverage must be between 0 and 100%".into());
        }
        if !(0.0..=1.0).contains(&self.initial_glacier_fraction) {
            return Err("Glacier coverage must be between 0 and 100%".into());
        }
        if !(2..=8).contains(&self.grid_level) {
            return Err("Grid level must be between 2 and 8".into());
        }
        if self.water.continents > 64 || self.water.islands > 256 {
            return Err("Too many continents or islands for this resolution".into());
        }
        if !(0.0..=90.0).contains(&self.axial_tilt_deg) {
            return Err("Axial tilt must be between 0 and 90°".into());
        }
        if !(1.0..=1000.0).contains(&self.rotation_period_hours) {
            return Err("Rotation period must be between 1 and 1,000 hours".into());
        }
        if !(0.0..=1.0).contains(&self.cloud_coverage) {
            return Err("Cloud coverage must be between 0 and 100%".into());
        }
        Ok(())
    }

    pub fn radius_m(&self) -> f32 {
        self.radius_km * 1_000.0
    }

    pub fn surface_area_m2(&self) -> f32 {
        4.0 * std::f32::consts::PI * self.radius_m() * self.radius_m()
    }

    pub fn surface_area_km2(&self) -> f64 {
        self.surface_area_m2() as f64 / 1.0e6
    }

    /// Major atmospheric constituents normalised to 1.
    pub fn normalised_composition(&self) -> (f32, f32, f32) {
        let o2 = self.o2_fraction.max(0.0);
        let n2 = self.n2_fraction.max(0.0);
        let h2o = self.h2o_fraction.max(0.0);
        let sum = (o2 + n2 + h2o).max(1.0e-6);
        (o2 / sum, n2 / sum, h2o / sum)
    }

    /// Records the values produced by generation so the creation UI and the
    /// physics panel display a coherent water budget and sea level.
    pub fn normalize_after_generation(
        &mut self,
        water_fraction: f32,
        avg_depth_m: f32,
        max_depth_m: f32,
        inventory_km3: f64,
        sea_level_m: f32,
    ) {
        self.water.surface_water_fraction = water_fraction;
        self.water.avg_ocean_depth_m = avg_depth_m;
        self.water.max_ocean_depth_m = max_depth_m;
        self.water.inventory_km3 = inventory_km3;
        self.water.sea_level_m = sea_level_m;
    }
}

pub mod presets {
    use super::{PlanetConfig, WaterConfig};

    pub fn earthlike() -> PlanetConfig {
        PlanetConfig {
            name: "Terra Nova".into(),
            seed: 20260417,
            grid_level: 6,
            radius_km: 6_371.0,
            mass_earth: 1.0,
            gravity: 9.806_65,
            stellar_flux_wm2: 1_361.0,
            mean_surface_temp_c: 15.0,
            ocean_temp_c: 17.0,
            surface_pressure_kpa: 101.325,
            co2_ppm: 420.0,
            o2_fraction: 0.209,
            n2_fraction: 0.780,
            h2o_fraction: 0.01,
            ch4_ppm: 1.9,
            rotation_period_hours: 23.934,
            axial_tilt_deg: 23.44,
            albedo: 0.306,
            cloud_coverage: 0.62,
            ocean_heat_capacity: 1.0,
            water: WaterConfig {
                inventory_km3: 1.386e9,
                surface_water_fraction: 0.71,
                avg_ocean_depth_m: 3_688.0,
                max_ocean_depth_m: 10_900.0,
                avg_land_elevation_m: 840.0,
                continents: 7,
                islands: 24,
                sea_level_m: 0.0,
            },
            initial_glacier_fraction: 0.10,
            plate_count: 10,
            weather_activity: 1.0,
            random_events: true,
        }
    }

    pub fn marslike() -> PlanetConfig {
        PlanetConfig {
            name: "Ares Basin".into(),
            seed: 77123,
            grid_level: 6,
            radius_km: 3_389.5,
            mass_earth: 0.107,
            gravity: 3.721,
            stellar_flux_wm2: 586.0,
            mean_surface_temp_c: -63.0,
            ocean_temp_c: -60.0,
            surface_pressure_kpa: 0.61,
            co2_ppm: 950_000.0,
            o2_fraction: 0.0013,
            n2_fraction: 0.021,
            h2o_fraction: 0.0003,
            ch4_ppm: 0.01,
            rotation_period_hours: 24.62,
            axial_tilt_deg: 25.19,
            albedo: 0.25,
            cloud_coverage: 0.05,
            ocean_heat_capacity: 0.3,
            water: WaterConfig {
                inventory_km3: 2.0e6,
                surface_water_fraction: 0.02,
                avg_ocean_depth_m: 80.0,
                max_ocean_depth_m: 250.0,
                avg_land_elevation_m: 1_200.0,
                continents: 4,
                islands: 8,
                sea_level_m: 0.0,
            },
            initial_glacier_fraction: 0.06,
            plate_count: 6,
            weather_activity: 0.3,
            random_events: true,
        }
    }

    pub fn ocean_world() -> PlanetConfig {
        PlanetConfig {
            name: "Pelagos".into(),
            seed: 551_2026,
            grid_level: 6,
            radius_km: 7_100.0,
            mass_earth: 1.4,
            gravity: 11.2,
            stellar_flux_wm2: 1_450.0,
            mean_surface_temp_c: 24.0,
            ocean_temp_c: 26.0,
            surface_pressure_kpa: 140.0,
            co2_ppm: 700.0,
            o2_fraction: 0.16,
            n2_fraction: 0.81,
            h2o_fraction: 0.02,
            ch4_ppm: 4.0,
            rotation_period_hours: 19.0,
            axial_tilt_deg: 18.0,
            albedo: 0.24,
            cloud_coverage: 0.75,
            ocean_heat_capacity: 1.5,
            water: WaterConfig {
                inventory_km3: 2.6e9,
                surface_water_fraction: 0.93,
                avg_ocean_depth_m: 5_200.0,
                max_ocean_depth_m: 12_000.0,
                avg_land_elevation_m: 500.0,
                continents: 3,
                islands: 40,
                sea_level_m: 0.0,
            },
            initial_glacier_fraction: 0.03,
            plate_count: 8,
            weather_activity: 1.3,
            random_events: true,
        }
    }

    pub fn desert_world() -> PlanetConfig {
        PlanetConfig {
            name: "Sirocco".into(),
            seed: 88_314,
            grid_level: 6,
            radius_km: 6_050.0,
            mass_earth: 0.8,
            gravity: 8.4,
            stellar_flux_wm2: 1_620.0,
            mean_surface_temp_c: 31.0,
            ocean_temp_c: 24.0,
            surface_pressure_kpa: 75.0,
            co2_ppm: 280.0,
            o2_fraction: 0.18,
            n2_fraction: 0.79,
            h2o_fraction: 0.004,
            ch4_ppm: 0.5,
            rotation_period_hours: 31.0,
            axial_tilt_deg: 12.0,
            albedo: 0.34,
            cloud_coverage: 0.12,
            ocean_heat_capacity: 0.6,
            water: WaterConfig {
                inventory_km3: 1.2e8,
                surface_water_fraction: 0.12,
                avg_ocean_depth_m: 900.0,
                max_ocean_depth_m: 3_500.0,
                avg_land_elevation_m: 700.0,
                continents: 5,
                islands: 10,
                sea_level_m: 0.0,
            },
            initial_glacier_fraction: 0.0,
            plate_count: 9,
            weather_activity: 0.8,
            random_events: true,
        }
    }

    pub fn snowball() -> PlanetConfig {
        PlanetConfig {
            name: "Cryos".into(),
            seed: 4_404,
            grid_level: 6,
            radius_km: 6_100.0,
            mass_earth: 0.9,
            gravity: 9.1,
            stellar_flux_wm2: 1_150.0,
            mean_surface_temp_c: -42.0,
            ocean_temp_c: -2.0,
            surface_pressure_kpa: 80.0,
            co2_ppm: 120.0,
            o2_fraction: 0.12,
            n2_fraction: 0.86,
            h2o_fraction: 0.002,
            ch4_ppm: 0.8,
            rotation_period_hours: 27.0,
            axial_tilt_deg: 26.0,
            albedo: 0.62,
            cloud_coverage: 0.35,
            ocean_heat_capacity: 1.2,
            water: WaterConfig {
                inventory_km3: 1.1e9,
                surface_water_fraction: 0.55,
                avg_ocean_depth_m: 4_000.0,
                max_ocean_depth_m: 9_000.0,
                avg_land_elevation_m: 900.0,
                continents: 6,
                islands: 18,
                sea_level_m: 0.0,
            },
            initial_glacier_fraction: 0.78,
            plate_count: 7,
            weather_activity: 0.4,
            random_events: true,
        }
    }

    pub fn preset_names() -> Vec<&'static str> {
        vec![
            "Earthlike",
            "Mars-like",
            "Ocean world",
            "Desert world",
            "Snowball",
            "Custom",
        ]
    }

    pub fn all_presets() -> Vec<(&'static str, PlanetConfig)> {
        vec![
            ("Earthlike", earthlike()),
            ("Mars-like", marslike()),
            ("Ocean world", ocean_world()),
            ("Desert world", desert_world()),
            ("Snowball", snowball()),
        ]
    }

    pub fn preset_by_name(name: &str) -> Option<PlanetConfig> {
        Some(match name {
            "Earthlike" => earthlike(),
            "Mars-like" => marslike(),
            "Ocean world" => ocean_world(),
            "Desert world" => desert_world(),
            "Snowball" => snowball(),
            _ => return None,
        })
    }
}
