//! Volcanic eruptions, asteroid impacts, ice-age forcing and global events.
//!
//! Catastrophes modify the same state the physical model reads: bedrock,
//! atmospheric opacity, carbon reservoirs and transient climate forcing. They
//! are not particle effects layered on top of the simulation.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::atmosphere::{AtmosphereState, CarbonState};
use crate::climate::ClimateState;
use crate::planet_generator::PlanetConfig;
use crate::terrain::PlanetGrid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CatastropheKind {
    Volcano,
    Impact,
    IceAge,
    Warming,
    Cooling,
    OceanCirculation,
    MagneticReversal,
    Drought,
}

impl CatastropheKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Volcano => "Volcanic eruption",
            Self::Impact => "Asteroid impact",
            Self::IceAge => "Glaciation forcing",
            Self::Warming => "Global warming",
            Self::Cooling => "Rapid cooling",
            Self::OceanCirculation => "Ocean circulation change",
            Self::MagneticReversal => "Magnetic field excursion",
            Self::Drought => "Global drought",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatastropheSettings {
    pub random_events: bool,
    pub eruption_rate_per_myr: f32,
    pub impact_rate_per_myr: f32,
    pub ice_age_rate_per_myr: f32,
}

impl Default for CatastropheSettings {
    fn default() -> Self {
        Self {
            random_events: true,
            eruption_rate_per_myr: 0.12,
            impact_rate_per_myr: 0.01,
            ice_age_rate_per_myr: 0.003,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActiveVolcano {
    pub cell: u32,
    pub intensity: f32,
    pub remaining_years: f32,
    pub duration_years: f32,
    pub co2_remaining_ppm: f32,
    pub aerosol_remaining: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalForcing {
    pub label: String,
    pub kind: CatastropheKind,
    pub remaining_years: f32,
    pub duration_years: f32,
    pub solar_multiplier: f32,
    pub ocean_circulation_multiplier: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatastropheState {
    pub settings: CatastropheSettings,
    pub volcanoes: Vec<ActiveVolcano>,
    pub forcings: Vec<GlobalForcing>,
    pub magnetic_field: f32,
    pub magnetic_recovery_years: f32,
}

impl CatastropheState {
    pub fn new(random_events: bool) -> Self {
        Self {
            settings: CatastropheSettings {
                random_events,
                ..Default::default()
            },
            volcanoes: Vec::new(),
            forcings: Vec::new(),
            magnetic_field: 1.0,
            magnetic_recovery_years: 0.0,
        }
    }

    pub fn solar_multiplier(&self) -> f32 {
        self.forcings
            .iter()
            .map(|f| f.solar_multiplier)
            .product::<f32>()
            .clamp(0.2, 3.0)
    }

    pub fn ocean_circulation_multiplier(&self) -> f32 {
        self.forcings
            .iter()
            .map(|f| f.ocean_circulation_multiplier)
            .product::<f32>()
            .clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug)]
pub struct CatastropheOutcome {
    pub kind: CatastropheKind,
    pub title: String,
    pub description: String,
}

/// Triggers an eruption. `intensity` is 1..10; `duration_years` spreads the
/// gas and aerosol injection instead of releasing everything instantly.
pub fn trigger_volcano(
    grid: &mut PlanetGrid,
    atmosphere: &mut AtmosphereState,
    carbon: &mut CarbonState,
    catastrophes: &mut CatastropheState,
    cell: usize,
    intensity: f32,
    duration_years: f32,
    _time_years: f64,
) -> CatastropheOutcome {
    let intensity = intensity.clamp(0.2, 10.0);
    let duration = duration_years.max(1.0);
    let cell = cell.min(grid.cell_count().saturating_sub(1));
    let total_co2 = 6.0 * intensity.powi(2);
    let total_aerosol = 0.035 * intensity.powf(1.6);
    let immediate_co2 = total_co2 * 0.18;
    let immediate_aerosol = total_aerosol * 0.30;

    carbon.outgas(&mut atmosphere.co2_ppm, immediate_co2);
    atmosphere.inject_aerosol(immediate_aerosol);
    let uplift = 60.0 * intensity;
    let c = &mut grid.cells[cell];
    c.bedrock_m += uplift;
    c.volcanic_activity = 1.0;
    c.lava_flow = 1.0;
    if c.bedrock_m > 0.0 {
        c.biome = crate::terrain::Biome::Desert;
    }

    catastrophes.volcanoes.push(ActiveVolcano {
        cell: cell as u32,
        intensity,
        remaining_years: duration,
        duration_years: duration,
        co2_remaining_ppm: total_co2 - immediate_co2,
        aerosol_remaining: total_aerosol - immediate_aerosol,
    });

    let (lat, lon) = (grid.lat_deg(cell), grid.lon_deg(cell));
    CatastropheOutcome {
        kind: CatastropheKind::Volcano,
        title: if intensity > 8.0 {
            "Super-eruption".into()
        } else if intensity > 5.0 {
            "Major eruption".into()
        } else {
            "Volcanic eruption".into()
        },
        description: format!(
            "Intensity {:.1} at {:.1}°, {:.1}°. Total CO₂ release {:.0} ppm-equivalent over {:.0} years.",
            intensity, lat, lon, total_co2, duration
        ),
    }
}

/// Applies the delayed part of active eruptions and expires them.
pub fn volcano_step(
    grid: &mut PlanetGrid,
    atmosphere: &mut AtmosphereState,
    carbon: &mut CarbonState,
    catastrophes: &mut CatastropheState,
    dt_years: f32,
) {
    let dt = dt_years.max(0.0);
    let mut finished = Vec::new();
    for (i, event) in catastrophes.volcanoes.iter_mut().enumerate() {
        let frac = if event.remaining_years > 0.0 {
            (dt / event.remaining_years).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let co2 = event.co2_remaining_ppm * frac;
        let aerosol = event.aerosol_remaining * frac;
        event.co2_remaining_ppm -= co2;
        event.aerosol_remaining -= aerosol;
        event.remaining_years -= dt;
        carbon.outgas(&mut atmosphere.co2_ppm, co2);
        atmosphere.inject_aerosol(aerosol);
        if let Some(cell) = grid.cells.get_mut(event.cell as usize) {
            cell.volcanic_activity = 1.0;
            cell.lava_flow = (cell.lava_flow + 0.4).min(1.0);
            let decay = (-dt / 2.0).exp();
            cell.volcanic_activity *= decay.max(0.2);
        }
        if event.remaining_years <= 0.0 {
            finished.push(i);
        }
    }
    for i in finished.into_iter().rev() {
        catastrophes.volcanoes.remove(i);
    }
}

/// Impact simulation. Returns the outcome and the estimated kinetic energy.
pub fn trigger_asteroid(
    grid: &mut PlanetGrid,
    atmosphere: &mut AtmosphereState,
    carbon: &mut CarbonState,
    cell: usize,
    diameter_km: f32,
    velocity_km_s: f32,
    angle_deg: f32,
) -> (CatastropheOutcome, f64) {
    let diameter_m = (diameter_km.max(0.001) as f64) * 1_000.0;
    let radius_m = diameter_m * 0.5;
    let density = 3_000.0;
    let mass = 4.0 / 3.0 * std::f64::consts::PI * radius_m.powi(3) * density;
    let velocity = (velocity_km_s.max(1.0) as f64) * 1_000.0;
    let angle = (angle_deg as f32).clamp(5.0, 90.0).to_radians();
    let energy = 0.5 * mass * velocity * velocity * angle.sin() as f64;

    let gravity = 9.81;
    let target_density = 2_700.0;
    let crater_diameter = 1.161 * (energy / (target_density * gravity)).powf(0.25) * 1.0e-3;
    let crater_km = crater_diameter.clamp(0.1, 2_000.0) as f32;
    let depth_m = (crater_km * 1_000.0 * 0.12).clamp(10.0, 30_000.0);
    let angular_radius = crater_km * 0.5 / grid_radius_km(grid);

    let center = grid.direction(cell);
    let n = grid.cell_count();
    for i in 0..n {
        let angle_to = center.dot(grid.direction(i)).clamp(-1.0, 1.0).acos();
        if angle_to > angular_radius * 2.2 {
            continue;
        }
        let r = angle_to / angular_radius.max(1.0e-6);
        let displacement = if r < 1.0 {
            -depth_m * (1.0 - r * r)
        } else {
            depth_m * 0.22 * (-((r - 1.0) * 2.5).powi(2)).exp()
        };
        let c = &mut grid.cells[i];
        c.bedrock_m = (c.bedrock_m + displacement).clamp(-40_000.0, 30_000.0);
        if c.lake_depth_m > 0.0 {
            c.lake_depth_m = 0.0;
        }
        c.quake_activity = 1.0;
        c.transient_heat_c += (1_200.0 / (1.0 + angle_to * 400.0)).min(300.0);
        c.volcanic_activity = c.volcanic_activity.max((-r).exp());
    }

    // Dust scales strongly with impactor size. A 10 km impactor produces a
    // global opacity of several optical depths.
    let dust = (0.35 * diameter_km.powf(1.35)).clamp(0.0, 30.0);
    atmosphere.inject_dust(dust);
    atmosphere.inject_aerosol(dust * 0.15);
    let co2 = (energy / 1.0e24 * 120.0).min(20_000.0) as f32;
    carbon.outgas(&mut atmosphere.co2_ppm, co2);

    let (lat, lon) = (grid.lat_deg(cell), grid.lon_deg(cell));
    (
        CatastropheOutcome {
            kind: CatastropheKind::Impact,
            title: format!("{:.1} km asteroid impact", diameter_km),
            description: format!(
                "Impact at {:.1}°, {:.1}° with {:.1} km/s at {:.0}°. Energy {:.2e} J, crater ≈ {:.0} km, dust τ ≈ {:.2}.",
                lat, lon, velocity_km_s, angle_deg, energy, crater_km, dust
            ),
        },
        energy,
    )
}

fn grid_radius_km(grid: &PlanetGrid) -> f32 {
    grid.radius_km.max(1.0)
}

/// Temporary stellar dimming / circulation forcing.
pub fn trigger_ice_age(
    catastrophes: &mut CatastropheState,
    duration_years: f32,
) -> CatastropheOutcome {
    catastrophes.forcings.push(GlobalForcing {
        label: "Experimental glaciation forcing".into(),
        kind: CatastropheKind::IceAge,
        remaining_years: duration_years.max(1.0),
        duration_years: duration_years.max(1.0),
        solar_multiplier: 0.86,
        ocean_circulation_multiplier: 1.0,
    });
    CatastropheOutcome {
        kind: CatastropheKind::IceAge,
        title: "Experimental cooling forcing".into(),
        description: format!(
            "Stellar flux reduced by 14% for {:.0} years. Sea ice and albedo feedbacks determine whether a snowball state develops; the forcing itself is not an animation.",
            duration_years
        ),
    }
}

pub fn trigger_global_warming(
    atmosphere: &mut AtmosphereState,
    carbon: &mut CarbonState,
    ppm: f32,
) -> CatastropheOutcome {
    let added = ppm.max(0.0);
    carbon.outgas(&mut atmosphere.co2_ppm, added);
    CatastropheOutcome {
        kind: CatastropheKind::Warming,
        title: "Carbon injection".into(),
        description: format!(
            "Injected {:.0} ppm CO₂-equivalent into the atmosphere. Radiative forcing and the water-vapour feedback warm the surface.",
            added
        ),
    }
}

pub fn trigger_rapid_cooling(
    atmosphere: &mut AtmosphereState,
    optical_depth: f32,
) -> CatastropheOutcome {
    atmosphere.inject_aerosol(optical_depth.max(0.0));
    CatastropheOutcome {
        kind: CatastropheKind::Cooling,
        title: "Aerosol cooling event".into(),
        description: format!(
            "Injected {:.2} aerosol optical depth. Sunlight at the surface is reduced until aerosols settle out.",
            optical_depth
        ),
    }
}

pub fn trigger_ocean_circulation(
    catastrophes: &mut CatastropheState,
    duration_years: f32,
) -> CatastropheOutcome {
    catastrophes.forcings.push(GlobalForcing {
        label: "Ocean circulation slowdown".into(),
        kind: CatastropheKind::OceanCirculation,
        remaining_years: duration_years.max(1.0),
        duration_years: duration_years.max(1.0),
        solar_multiplier: 1.0,
        ocean_circulation_multiplier: 0.25,
    });
    CatastropheOutcome {
        kind: CatastropheKind::OceanCirculation,
        title: "Ocean circulation slowdown".into(),
        description:
            "Meridional heat transport reduced to 25% for the event duration. Regional temperature gradients strengthen."
                .into(),
    }
}

pub fn trigger_magnetic_reversal(catastrophes: &mut CatastropheState) -> CatastropheOutcome {
    catastrophes.magnetic_field = 0.12;
    catastrophes.magnetic_recovery_years = 20_000.0;
    CatastropheOutcome {
        kind: CatastropheKind::MagneticReversal,
        title: "Magnetic field excursion".into(),
        description:
            "Dipole strength reduced to 12%. The field recovers over ~20,000 years. No climate effect is claimed beyond the tracked field state."
                .into(),
    }
}

/// Steps forcings, magnetic recovery and random event generation.
pub fn catastrophe_step(
    grid: &mut PlanetGrid,
    atmosphere: &mut AtmosphereState,
    carbon: &mut CarbonState,
    _climate: &mut ClimateState,
    catastrophes: &mut CatastropheState,
    config: &PlanetConfig,
    time_years: f64,
    dt_years: f32,
    rng: &mut ChaCha8Rng,
    outcomes: &mut Vec<CatastropheOutcome>,
) {
    let dt = dt_years.max(0.0);
    volcano_step(grid, atmosphere, carbon, catastrophes, dt);

    for forcing in &mut catastrophes.forcings {
        forcing.remaining_years -= dt;
    }
    catastrophes.forcings.retain(|f| f.remaining_years > 0.0);

    if catastrophes.magnetic_recovery_years > 0.0 {
        catastrophes.magnetic_recovery_years -= dt;
        let recovery_rate = 1.0 / 20_000.0;
        catastrophes.magnetic_field =
            (catastrophes.magnetic_field + recovery_rate * dt).clamp(0.12, 1.0);
    } else {
        catastrophes.magnetic_field = 1.0;
    }

    if !catastrophes.settings.random_events {
        return;
    }
    let dt_myr = dt / 1.0e6;
    if dt_myr <= 0.0 {
        return;
    }

    let eruptions = sample_poisson(rng, catastrophes.settings.eruption_rate_per_myr * dt_myr, 3);
    for _ in 0..eruptions {
        let cell = pick_tectonic_cell(grid, rng);
        let intensity = rng.random_range(2.0f32..8.5);
        let outcome = trigger_volcano(
            grid,
            atmosphere,
            carbon,
            catastrophes,
            cell,
            intensity,
            rng.random_range(2.0f32..150.0),
            time_years,
        );
        outcomes.push(outcome);
    }

    let impacts = sample_poisson(rng, catastrophes.settings.impact_rate_per_myr * dt_myr, 1);
    for _ in 0..impacts {
        let cell = rng.random_range(0..grid.cell_count());
        let diameter = 10.0f32.powf(rng.random_range(-1.4f32..1.0));
        let velocity = rng.random_range(12.0f32..45.0);
        let angle = rng.random_range(20.0f32..90.0);
        let (outcome, _) =
            trigger_asteroid(grid, atmosphere, carbon, cell, diameter, velocity, angle);
        outcomes.push(outcome);
    }

    let ice_ages = sample_poisson(rng, catastrophes.settings.ice_age_rate_per_myr * dt_myr, 1);
    for _ in 0..ice_ages {
        outcomes.push(trigger_ice_age(
            catastrophes,
            rng.random_range(20_000.0f32..200_000.0),
        ));
    }

    // Keep the atmospheric and biological state awake: occasional dust
    // storms and droughts are generated by the weather module, not here.
    let _ = config;
}

fn pick_tectonic_cell(grid: &PlanetGrid, rng: &mut ChaCha8Rng) -> usize {
    if grid.cell_count() == 0 {
        return 0;
    }
    let mut best = 0usize;
    let mut best_score = f32::NEG_INFINITY;
    for _ in 0..32 {
        let idx = rng.random_range(0..grid.cell_count());
        let c = &grid.cells[idx];
        let score = c.volcanic_activity * 3.0 + c.quake_activity * 2.0 + c.bedrock_m.abs() * 1.0e-4;
        if score > best_score {
            best_score = score;
            best = idx;
        }
    }
    best
}

fn sample_poisson(rng: &mut ChaCha8Rng, mean: f32, cap: u32) -> u32 {
    if mean <= 0.0 {
        return 0;
    }
    let mean = mean.min(cap as f32 * 4.0);
    if mean > 8.0 {
        // Normal approximation is adequate for the rare-event rates used here.
        let gaussian = rng.random_range(-2.5f32..2.5);
        return ((mean + mean.sqrt() * gaussian).round().max(0.0) as u32).min(cap);
    }
    let mut k = 0u32;
    let mut p = 1.0f32;
    let limit = (-mean).exp();
    loop {
        p *= rng.random::<f32>();
        if p <= limit || k >= cap {
            break;
        }
        k += 1;
    }
    k.min(cap)
}

/// Kinetic energy of an impactor, used by the event-configuration preview.
pub fn impact_energy(diameter_km: f32, velocity_km_s: f32, angle_deg: f32) -> f64 {
    let radius = diameter_km.max(0.001) as f64 * 500.0;
    let mass = 4.0 / 3.0 * std::f64::consts::PI * radius.powi(3) * 3_000.0;
    let v = velocity_km_s.max(1.0) as f64 * 1_000.0;
    0.5 * mass * v * v * angle_deg.clamp(5.0, 90.0).to_radians().sin() as f64
}
