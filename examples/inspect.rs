//! Headless diagnostic for the simulation core (no Bevy window).
//!
//! ```bash
//! cargo run --profile test --example inspect -- 6
//! ```

use planet_simulator::planet_generator::PlanetConfig;
use planet_simulator::simulation::PlanetState;

fn main() {
    let mut config = PlanetConfig::default();
    config.grid_level = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);
    println!(
        "config: water inventory {:.3e} km³, target water coverage {:.3}, grid level {}",
        config.water.inventory_km3, config.water.surface_water_fraction, config.grid_level
    );
    let mut planet = PlanetState::new(config).expect("planet generation");
    let (area0, volume0) = planet.grid.ocean_metrics_at(0.0);

    println!(
        "initial: sea {:.1} m, ocean {:.3e} km³, ice {:.3e} km³, soil {:.3e} km³",
        planet.hydrology.sea_level_m,
        planet.hydrology.ocean_volume_km3,
        planet.hydrology.ice_volume_km3,
        planet.hydrology.soil_water_km3
    );
    println!(
        "initial: ocean area {:.3e} km², mean depth {:.0} m, land {:.1}%, ice {:.1}%, T {:.1} °C",
        planet.hydrology.ocean_area_km2,
        planet.hydrology.mean_ocean_depth_m,
        planet.stats.land_fraction * 100.0,
        planet.stats.glacier_fraction * 100.0,
        planet.stats.mean_temp_c
    );
    println!(
        "geometry at datum: ocean area {:.3e} km², ocean volume {:.3e} km³",
        area0, volume0
    );

    let mut depths: Vec<f32> = planet.grid.cells.iter().map(|c| c.water_depth_m).collect();
    depths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = depths.len();
    if n > 0 {
        println!(
            "ocean depth percentiles: p50 {:.0} m, p90 {:.0} m, max {:.0} m",
            depths[n / 2],
            depths[n * 9 / 10],
            depths[n - 1]
        );
    }

    for _ in 0..40 {
        planet.step_slice(0.5);
    }
    let mut discharge: Vec<f32> = planet
        .grid
        .cells
        .iter()
        .filter(|c| c.bedrock_m >= planet.sea_level_m())
        .map(|c| c.discharge_m3s)
        .collect();
    discharge.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let dn = discharge.len();
    println!(
        "after 20 years: sea {:.1} m, land {:.1}%, ice {:.1}%, precip {:.0} mm/yr, T {:.1} °C, water budget {:.3e} km³",
        planet.hydrology.sea_level_m,
        planet.stats.land_fraction * 100.0,
        planet.stats.glacier_fraction * 100.0,
        planet.stats.mean_precip_mm_yr,
        planet.stats.mean_temp_c,
        planet.hydrology.total_water_km3
    );
    if dn > 0 {
        println!(
            "land discharge percentiles: p50 {:.0}, p90 {:.0}, max {:.0} m³/s",
            discharge[dn / 2],
            discharge[dn * 9 / 10],
            discharge[dn - 1]
        );
    }
    println!("stats: {:?}", planet.stats);
}
