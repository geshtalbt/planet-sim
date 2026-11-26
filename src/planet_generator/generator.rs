use glam::Vec3;

use crate::planet_generator::config::PlanetConfig;
use crate::planet_generator::noise::{Noise3, SplitMix64};
use crate::tectonics::Plate;
use crate::terrain::{Biome, PlanetGrid, SurfaceCell, build_topology};

pub struct GeneratedPlanet {
    pub grid: PlanetGrid,
    pub plates: Vec<Plate>,
    pub ocean_area_km2: f64,
    pub ocean_volume_km3: f64,
    pub ice_volume_estimate_km3: f64,
    pub sea_level_m: f32,
}

/// Generates the solid surface, plate layout and initial water distribution.
///
/// The configured total water inventory is used to set the mean ocean depth.
/// The returned grid still needs climate initialization and final water
/// balancing (performed by [`crate::simulation::PlanetState::new`]).
pub fn generate_planet(config: &PlanetConfig) -> Result<GeneratedPlanet, String> {
    config.validate()?;

    let level = config.grid_level;
    let topology = build_topology(level);
    let n = topology.cell_count();
    let dirs = &topology.directions;
    let radius_m = config.radius_m();
    let total_area_km2 = 4.0 * std::f64::consts::PI * (config.radius_km as f64).powi(2);

    let mut rng = SplitMix64::new(config.seed ^ 0x51ed_2701_9e37_79b9);
    let noise =
        Noise3::new(config.seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x1234_5678_9abc_def0);
    let warp_noise = Noise3::new(config.seed ^ 0x0bad_c0de_f00d_face);

    // --- Continents -------------------------------------------------------
    let mut continents: Vec<Continent> = Vec::new();
    for _ in 0..config.water.continents.max(1) {
        let mut best = rng.unit_vec();
        let mut best_score = f32::NEG_INFINITY;
        for _ in 0..6 {
            let candidate = rng.unit_vec();
            let mut score: f32 = 4.0;
            for c in &continents {
                score = score.min(candidate.dot(c.center).max(-1.0) + 1.0);
            }
            if score > best_score {
                best_score = score;
                best = candidate;
            }
        }
        continents.push(Continent {
            center: best,
            radius: rng.range(0.30, 0.62),
            amplitude: rng.range(0.75, 1.25),
        });
    }

    let mut islands = Vec::new();
    for _ in 0..config.water.islands {
        islands.push(Island {
            center: rng.unit_vec(),
            radius: rng.range(0.025, 0.10),
            amplitude: rng.range(0.15, 0.55),
        });
    }

    // --- Plates -----------------------------------------------------------
    let plate_count = config.plate_count.clamp(2, 32) as usize;
    let mut plates = Vec::with_capacity(plate_count);
    let mut plate_centers: Vec<Vec3> = Vec::with_capacity(plate_count);
    for i in 0..plate_count {
        let mut best = rng.unit_vec();
        let mut best_sep = -1.0;
        for _ in 0..10 {
            let candidate = rng.unit_vec();
            let sep = plate_centers
                .iter()
                .map(|c| candidate.dot(*c))
                .fold(-1.0f32, f32::max);
            if sep < best_sep || best_sep < 0.0 {
                best_sep = sep;
                best = candidate;
            }
        }
        plate_centers.push(best);
        let axis = rng.unit_vec();
        plates.push(Plate {
            id: i as u16,
            name: format!("Plate {}", plate_label(i)),
            center: best.to_array(),
            motion_axis: axis.to_array(),
            angular_speed_rad_per_myr: if rng.unit() < 0.5 {
                -rng.range(0.002, 0.020)
            } else {
                rng.range(0.002, 0.020)
            },
            oceanic: rng.unit() > 0.42,
        });
    }

    let mut plate_of = vec![0u16; n];
    let mut plate_kind_ocean = vec![false; plates.len()];
    for i in 0..plates.len() {
        plate_kind_ocean[i] = plates[i].oceanic;
    }
    for (cell, dir) in dirs.iter().enumerate() {
        let mut best = 0usize;
        let mut best_dot = f32::NEG_INFINITY;
        for (pi, plate) in plates.iter().enumerate() {
            let dot = dir.dot(plate.center_vec());
            if dot > best_dot {
                best_dot = dot;
                best = pi;
            }
        }
        plate_of[cell] = best as u16;
    }

    // Boundary strength: 1 on a plate boundary, fading inland.
    let mut boundary = vec![0.0f32; n];
    for cell in 0..n {
        for &nb in topology.neighbors(cell) {
            if plate_of[cell] != plate_of[nb as usize] {
                boundary[cell] = 1.0;
                break;
            }
        }
    }
    let mut next = boundary.clone();
    for _ in 0..3 {
        for cell in 0..n {
            let mut m = boundary[cell];
            for &nb in topology.neighbors(cell) {
                m = m.max(boundary[nb as usize] * 0.55);
            }
            next[cell] = m;
        }
        std::mem::swap(&mut boundary, &mut next);
    }

    // --- Elevation field --------------------------------------------------
    let mut raw = vec![0.0f32; n];
    let mut relief_m = vec![0.0f32; n];
    let mut convergence = vec![0.0f32; n];
    for (cell, dir) in dirs.iter().enumerate() {
        let warp = Vec3::new(
            warp_noise.get(*dir * 2.2 + Vec3::splat(11.3)),
            warp_noise.get(*dir * 2.2 + Vec3::splat(27.1)),
            warp_noise.get(*dir * 2.2 + Vec3::splat(41.7)),
        ) * 0.18;
        let sample = *dir * 1.65 + warp;
        let base = noise.fbm(sample, 6, 2.0, 0.5);
        let detail = noise.fbm(*dir * 5.0 + warp * 0.5, 4, 2.1, 0.5) * 0.16;
        let ridge = noise.ridged(*dir * 3.1 + warp, 5, 2.05, 0.52);

        let mut continental = 0.0f32;
        for c in &continents {
            let angle = dir.dot(c.center).clamp(-1.0, 1.0).acos();
            let g = (-(angle / c.radius).powi(2) * 1.7).exp();
            continental = continental.max(g * c.amplitude);
        }
        // Irregular coastlines: perturb the smooth continental mask with noise.
        let coast_warp = 0.82 + 0.30 * base;
        continental *= coast_warp;

        let mut island_bump = 0.0f32;
        for isl in &islands {
            let angle = dir.dot(isl.center).clamp(-1.0, 1.0).acos();
            let g = (-(angle / isl.radius).powi(2) * 2.2).exp();
            island_bump = island_bump.max(g * isl.amplitude);
        }

        let e = continental * 1.10
            + base * 0.48
            + (ridge - 0.42) * 0.38 * (0.35 + 0.65 * continental)
            + detail
            + island_bump;

        // --- Boundary relief ---------------------------------------------
        let mut conv = 0.0f32;
        let mut contact = false;
        let mut contact_oceanic = false;
        for &nb in topology.neighbors(cell) {
            let other = plate_of[nb as usize];
            if other == plate_of[cell] {
                continue;
            }
            contact = true;
            contact_oceanic |= plate_kind_ocean[other as usize];
            let p1 = &plates[plate_of[cell] as usize];
            let p2 = &plates[other as usize];
            let v1 = p1.velocity_at_m_per_myr(*dir, radius_m);
            let v2 = p2.velocity_at_m_per_myr(dirs[nb as usize], radius_m);
            let rel = v1 - v2;
            let line = (dirs[nb as usize] - *dir).normalize_or_zero();
            let c = -rel.dot(line);
            if c.abs() > conv.abs() {
                conv = c;
            }
        }
        convergence[cell] = conv;

        if contact {
            let ocean_here = plate_kind_ocean[plate_of[cell] as usize];
            let both_ocean = ocean_here && contact_oceanic;
            if conv > 0.0 {
                let h = (conv * 0.030).clamp(0.0, 4_500.0);
                if ocean_here && both_ocean {
                    relief_m[cell] -= h * 0.55; // trench
                } else if ocean_here {
                    relief_m[cell] += h * 0.20; // island arc
                } else {
                    relief_m[cell] += h; // continental mountain belt
                }
            } else {
                let h = (-conv * 0.012).clamp(0.0, 2_000.0);
                if ocean_here {
                    relief_m[cell] += h * 0.35; // mid-ocean ridge
                } else {
                    relief_m[cell] -= h * 0.20; // continental rift
                }
            }
        }
        raw[cell] = e;
    }

    // Give mountain belts fractal relief.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        raw[a]
            .partial_cmp(&raw[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let water_fraction = config.water.surface_water_fraction.clamp(0.0, 1.0);
    let sea_index = ((water_fraction * n as f32) as usize).clamp(0, n - 1);
    let sea_raw = if water_fraction <= 0.0 {
        raw[order[0]] - 1.0e-4
    } else if water_fraction >= 1.0 {
        raw[*order.last().unwrap()] + 1.0e-4
    } else {
        raw[order[sea_index]]
    };

    let ocean_cells: Vec<usize> = order
        .iter()
        .copied()
        .filter(|&i| raw[i] < sea_raw)
        .collect();
    let land_cells: Vec<usize> = order
        .iter()
        .copied()
        .filter(|&i| raw[i] >= sea_raw)
        .collect();

    let ocean_area_km2: f64 = ocean_cells
        .iter()
        .map(|&i| topology.unit_areas[i] as f64 * (config.radius_km as f64).powi(2))
        .sum();
    let land_area_km2 = (total_area_km2 - ocean_area_km2).max(0.0);

    // Reserve water for ice, soil and lakes.
    let avg_ice_thickness_m = water_fraction * 2.0 + (1.0 - water_fraction) * 650.0;
    let ice_volume_estimate_km3 =
        config.initial_glacier_fraction as f64 * total_area_km2 * (avg_ice_thickness_m as f64)
            / 1_000.0;
    let soil_reserve_km3 = land_area_km2 * 0.08 / 1_000.0;
    let ocean_target_km3 = if config.water.inventory_km3 > 0.0 {
        (config.water.inventory_km3 - ice_volume_estimate_km3 - soil_reserve_km3)
            .max(config.water.inventory_km3 * 0.05)
    } else {
        ocean_area_km2 * (config.water.avg_ocean_depth_m as f64) / 1_000.0
    };
    let target_avg_depth_m = if ocean_area_km2 > 0.0 {
        (ocean_target_km3 / ocean_area_km2 * 1_000.0).clamp(10.0, 30_000.0)
    } else {
        0.0
    };

    let mean_unit_ocean = if ocean_cells.is_empty() {
        0.0
    } else {
        ocean_cells
            .iter()
            .map(|&i| (sea_raw - raw[i]).max(0.0) as f64)
            .sum::<f64>()
            / ocean_cells.len() as f64
    };
    let depth_scale = if mean_unit_ocean > 1.0e-9 {
        target_avg_depth_m as f64 / mean_unit_ocean
    } else {
        0.0
    };

    let mean_unit_land = if land_cells.is_empty() {
        0.0
    } else {
        land_cells
            .iter()
            .map(|&i| (raw[i] - sea_raw).max(0.0) as f64)
            .sum::<f64>()
            / land_cells.len() as f64
    };
    let land_scale = if mean_unit_land > 1.0e-9 {
        config.water.avg_land_elevation_m as f64 / mean_unit_land
    } else {
        0.0
    };

    let mut cells = Vec::with_capacity(n);
    for (i, &r) in raw.iter().enumerate() {
        let mut cell = SurfaceCell::default();
        cell.plate = plate_of[i];
        if r < sea_raw {
            let depth = ((sea_raw - r) as f64 * depth_scale).clamp(0.0, 35_000.0) as f32;
            cell.bedrock_m = -depth;
            cell.water_depth_m = depth;
            cell.biome = Biome::Ocean;
            cell.albedo = Biome::Ocean.base_albedo();
        } else {
            let height = ((r - sea_raw) as f64 * land_scale).clamp(0.0, 16_000.0) as f32;
            cell.bedrock_m = height;
            cell.water_depth_m = 0.0;
            cell.biome = Biome::Grassland;
            cell.albedo = 0.22;
        }
        cells.push(cell);
    }

    // Boundary relief is stored in metres, not in the dimensionless raw
    // elevation field. Adding metre-scale values to that field would overwhelm
    // the fractal terrain and produce a near-flat shelf plus a few trenches.
    for (cell, relief) in cells.iter_mut().zip(&relief_m) {
        cell.bedrock_m = (cell.bedrock_m + relief).clamp(-40_000.0, 30_000.0);
        cell.volcanic_activity = (relief.abs() / 4_000.0).clamp(0.0, 0.5);
    }

    let mut grid = PlanetGrid::new(level, cells);
    grid.rebuild_areas(radius_m);
    let (actual_ocean_area, actual_ocean_volume) = grid.apply_sea_level(0.0, radius_m);

    Ok(GeneratedPlanet {
        grid,
        plates,
        ocean_area_km2: actual_ocean_area,
        ocean_volume_km3: actual_ocean_volume,
        ice_volume_estimate_km3,
        sea_level_m: 0.0,
    })
}

struct Continent {
    center: Vec3,
    radius: f32,
    amplitude: f32,
}

struct Island {
    center: Vec3,
    radius: f32,
    amplitude: f32,
}

fn plate_label(index: usize) -> String {
    let mut n = index + 1;
    let mut s = String::new();
    while n > 0 {
        let r = (n - 1) % 26;
        s.insert(0, (b'A' + r as u8) as char);
        n = (n - 1) / 26;
    }
    s
}
