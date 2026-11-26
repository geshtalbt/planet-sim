//! What-if experiments: full planet forks that continue alongside the
//! original simulation and can be compared on a synchronised timeline.

use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use crate::history::History;
use rand_chacha::ChaCha8Rng;

use crate::simulation::PlanetState;

pub const FORK_COLORS: [[f32; 3]; 6] = [
    [0.25, 0.80, 0.95],
    [0.99, 0.62, 0.25],
    [0.55, 0.85, 0.45],
    [0.92, 0.40, 0.55],
    [0.72, 0.58, 0.95],
    [0.95, 0.85, 0.35],
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExperimentRun {
    pub id: u64,
    pub label: String,
    pub color: [f32; 3],
    pub planet: PlanetState,
    pub history: History,
    pub created_at_years: f64,
    pub baseline: bool,
}

impl ExperimentRun {
    /// Creates a fork of an existing run. Random events are decorrelated so
    /// the comparison is not artificially identical.
    pub fn fork_from(
        id: u64,
        label: impl Into<String>,
        color: [f32; 3],
        source: &ExperimentRun,
    ) -> Self {
        let mut planet = source.planet.clone();
        let seed = source
            .planet
            .config
            .seed
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            ^ id.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        planet.rng = ChaCha8Rng::seed_from_u64(seed);
        let mut history = source.history.clone();
        history.log(
            planet.time_years,
            crate::history::EventKind::User,
            "What-if branch created",
            format!(
                "Forked from '{}'. Parameters can now be changed independently.",
                source.label
            ),
        );
        Self {
            id,
            label: label.into(),
            color,
            created_at_years: planet.time_years,
            planet,
            history,
            baseline: false,
        }
    }

    pub fn latest_stats(&self) -> &crate::statistics::PlanetStats {
        &self.planet.stats
    }
}

/// Difference between two runs at a common simulation time.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunComparison {
    pub time_years: f64,
    pub delta_temp_c: Option<f32>,
    pub delta_co2_ppm: Option<f32>,
    pub delta_sea_level_m: Option<f32>,
    pub delta_ice_fraction: Option<f32>,
    pub delta_land_fraction: Option<f32>,
}

pub fn compare_at(baseline: &ExperimentRun, other: &ExperimentRun, time: f64) -> RunComparison {
    let a = baseline.history.sample_at_or_before(time);
    let b = other.history.sample_at_or_before(time);
    match (a, b) {
        (Some(a), Some(b)) => RunComparison {
            time_years: time,
            delta_temp_c: Some(b.mean_temp_c - a.mean_temp_c),
            delta_co2_ppm: Some(b.co2_ppm - a.co2_ppm),
            delta_sea_level_m: Some(b.sea_level_m - a.sea_level_m),
            delta_ice_fraction: Some(b.glacier_fraction - a.glacier_fraction),
            delta_land_fraction: Some(b.land_fraction - a.land_fraction),
        },
        _ => RunComparison {
            time_years: time,
            ..Default::default()
        },
    }
}
