use serde::{Deserialize, Serialize};

use super::plate::Plate;
use crate::terrain::PlanetGrid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TectonicRates {
    /// Multiplier converting convergence velocity into uplift.
    pub uplift_factor: f32,
    /// Erosion time scale at 1 m/yr precipitation, in million years.
    pub erosion_tau_myr: f32,
    /// Maximum absolute bedrock change per tectonic step, metres.
    pub max_step_uplift_m: f32,
    /// Plate reassignment interval, million years.
    pub reassignment_interval_myr: f32,
}

impl Default for TectonicRates {
    fn default() -> Self {
        Self {
            uplift_factor: 0.0075,
            erosion_tau_myr: 140.0,
            max_step_uplift_m: 2_500.0,
            reassignment_interval_myr: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct PlateStats {
    pub boundary_cells: u32,
    pub active_boundaries: u32,
    pub mean_convergence_m_per_myr: f32,
}

/// Reassigns each cell to the nearest plate centre and stores the plate id.
pub fn recompute_plate_assignment(grid: &mut PlanetGrid, plates: &[Plate]) {
    if plates.is_empty() {
        return;
    }
    for cell in 0..grid.cell_count() {
        let dir = grid.direction(cell);
        let mut best = 0usize;
        let mut best_dot = f32::NEG_INFINITY;
        for (i, plate) in plates.iter().enumerate() {
            let dot = dir.dot(plate.center_vec());
            if dot > best_dot {
                best_dot = dot;
                best = i;
            }
        }
        grid.cells[cell].plate = best as u16;
    }
}

/// Applies one tectonic update over `dt_years`. The caller invokes this at a
/// geological cadence; large steps are handled with clamped, stable updates.
pub fn tectonic_step(
    grid: &mut PlanetGrid,
    plates: &mut [Plate],
    radius_m: f32,
    sea_level_m: f32,
    dt_years: f32,
    rates: &TectonicRates,
) -> PlateStats {
    let dt_myr = (dt_years / 1.0e6).max(0.0);
    if dt_myr == 0.0 || plates.is_empty() {
        return PlateStats::default();
    }

    for plate in plates.iter_mut() {
        plate.advance(dt_myr);
    }
    recompute_plate_assignment(grid, plates);

    let n = grid.cell_count();
    let old = grid.cells.iter().map(|c| c.bedrock_m).collect::<Vec<_>>();
    let mut delta = vec![0.0f32; n];
    let mut volcanic = vec![0.0f32; n];
    let mut quake = vec![0.0f32; n];
    let mut boundary_cells = 0u32;
    let mut active_boundaries = 0u32;
    let mut convergence_sum = 0.0f64;
    let mut convergence_count = 0u32;

    for cell in 0..n {
        let plate_a = grid.cells[cell].plate as usize;
        if plate_a >= plates.len() {
            continue;
        }
        let dir_a = grid.direction(cell);
        let mut is_boundary = false;
        for &nb in grid.neighbors(cell) {
            let nb = nb as usize;
            let plate_b = grid.cells[nb].plate as usize;
            if plate_b == plate_a || plate_b >= plates.len() || nb <= cell {
                continue;
            }
            is_boundary = true;
            let dir_b = grid.direction(nb);
            let line = (dir_b - dir_a).normalize_or_zero();
            let v_a = plates[plate_a].velocity_at_m_per_myr(dir_a, radius_m);
            let v_b = plates[plate_b].velocity_at_m_per_myr(dir_b, radius_m);
            let convergence = -(v_a - v_b).dot(line);
            convergence_sum += convergence.abs() as f64;
            convergence_count += 1;
            if convergence.abs() > 5_000.0 {
                active_boundaries += 1;
            }

            let both_oceanic = plates[plate_a].oceanic && plates[plate_b].oceanic;
            let one_oceanic = plates[plate_a].oceanic || plates[plate_b].oceanic;
            let sea_relief = (old[cell] - sea_level_m).max(0.0);
            let other_land = (old[nb] - sea_level_m) > 0.0;
            let this_land = sea_relief > 0.0;

            if convergence > 0.0 {
                // Convergent margin.
                let base = (convergence * rates.uplift_factor * dt_myr)
                    .clamp(0.0, rates.max_step_uplift_m);
                if both_oceanic {
                    delta[cell] -= base * 0.55;
                    delta[nb] -= base * 0.35;
                    volcanic[cell] = volcanic[cell].max((convergence / 50_000.0).clamp(0.0, 1.0));
                } else if one_oceanic {
                    let (land, ocean) = if this_land { (cell, nb) } else { (nb, cell) };
                    delta[land] += base * 0.75;
                    delta[ocean] -= base * 0.30;
                    volcanic[land] = volcanic[land].max(0.55);
                    quake[land] = quake[land].max(0.5);
                } else {
                    delta[cell] += base * 0.65;
                    delta[nb] += base * 0.65;
                    quake[cell] = quake[cell].max(0.7);
                }
            } else {
                // Divergent margin.
                let base = ((-convergence) * rates.uplift_factor * 0.4 * dt_myr)
                    .clamp(0.0, rates.max_step_uplift_m * 0.5);
                if both_oceanic {
                    delta[cell] += base * 0.35;
                    delta[nb] += base * 0.35;
                    volcanic[cell] = volcanic[cell].max(0.25);
                } else if this_land && other_land {
                    delta[cell] -= base * 0.18;
                    delta[nb] -= base * 0.18;
                } else {
                    delta[cell] += base * 0.10;
                    delta[nb] += base * 0.10;
                }
            }
        }
        if is_boundary {
            boundary_cells += 1;
        }
    }

    // Smooth uplift over one neighbour ring so mountain belts have width.
    let mut smoothed = delta.clone();
    for cell in 0..n {
        let mut sum = delta[cell];
        let mut count = 1.0;
        for &nb in grid.neighbors(cell) {
            sum += delta[nb as usize] * 0.45;
            count += 0.45;
        }
        smoothed[cell] = sum / count;
    }
    for cell in 0..n {
        smoothed[cell] = (smoothed[cell] + delta[cell] * 0.5)
            .clamp(-rates.max_step_uplift_m, rates.max_step_uplift_m);
    }

    // Apply uplift, erosion and activity decay.
    for cell in 0..n {
        let c = &mut grid.cells[cell];
        let sea_relief = (c.bedrock_m - sea_level_m).max(0.0);
        let precip_factor = (0.35 + c.precip_mm_yr / 1_200.0).clamp(0.25, 2.5);
        let tau_myr = (rates.erosion_tau_myr / precip_factor).max(5.0);
        let erosion = if sea_relief > 0.0 {
            sea_relief * (1.0 - (-dt_myr / tau_myr).exp())
        } else {
            0.0
        };
        let glacial = if c.ice_thickness_m > 50.0 {
            (sea_relief * 0.000_02 * dt_myr).min(sea_relief * 0.1)
        } else {
            0.0
        };
        c.bedrock_m = (c.bedrock_m + smoothed[cell] - erosion - glacial).clamp(-12_000.0, 10_000.0);
        let activity_decay = (-dt_myr / 3.0).exp();
        c.volcanic_activity = (c.volcanic_activity * activity_decay).max(volcanic[cell]);
        c.quake_activity = (c.quake_activity * activity_decay).max(quake[cell]);
        c.lava_flow = (c.lava_flow * (-dt_myr / 1.5).exp()).max(volcanic[cell] * 0.4);
    }

    PlateStats {
        boundary_cells,
        active_boundaries,
        mean_convergence_m_per_myr: if convergence_count > 0 {
            (convergence_sum / convergence_count as f64) as f32
        } else {
            0.0
        },
    }
}
