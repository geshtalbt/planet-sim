//! Floating science panels: physics, catastrophes, statistics, history,
//! saves, what-if experiments and application settings.

use bevy_egui::egui;

use crate::app::{Screen, ViewState};
use crate::experiments::compare_at;
use crate::history::EventKind;
use crate::rendering::PlanetVisual;
use crate::simulation::{Simulation, format_age};
use crate::ui::{ChartMetric, UiState, accent, danger, muted, warning, widgets};

pub fn draw_windows(
    ctx: &egui::Context,
    state: &mut UiState,
    view: &mut ViewState,
    mut simulation: Option<&mut Simulation>,
    visual: &mut PlanetVisual,
) {
    physics_window(ctx, state, simulation.as_deref_mut());
    statistics_window(ctx, state, view, simulation.as_deref_mut());
    catastrophe_window(ctx, state, view, simulation.as_deref_mut());
    history_window(ctx, state, view, simulation.as_deref_mut());
    saves_window(ctx, state, view, simulation.as_deref_mut());
    experiments_window(ctx, state, view, simulation.as_deref_mut());
    settings_window(ctx, state, view, simulation.as_deref_mut(), visual);
    help_window(ctx, state);
}

fn physics_window(
    ctx: &egui::Context,
    state: &mut UiState,
    mut simulation: Option<&mut Simulation>,
) {
    let mut open = state.show_physics;
    if open
        && !state.physics_was_open_last
        && let Some(sim) = simulation.as_ref()
    {
        state.co2_edit = sim.active_planet().atmosphere.co2_ppm;
        state.water_edit = sim.active_planet().hydrology.total_water_km3;
    }
    state.physics_was_open_last = open;

    egui::Window::new("Planetary physics")
        .open(&mut open)
        .default_width(520.0)
        .resizable(true)
        .show(ctx, |ui| {
            let Some(sim) = simulation.as_deref_mut() else {
                ui.label("No active simulation.");
                return;
            };
            let initial = sim.initial_config.clone();
            egui::ScrollArea::vertical().max_height(680.0).show(ui, |ui| {
                ui.label(
                    egui::RichText::new(
                        "Changes feed the same energy-balance, hydrological and tectonic state used by the simulation. \
                         Immediate controls apply at once; ramps are integrated over simulation time.",
                    )
                    .size(10.0)
                    .color(muted()),
                );
                ui.separator();

                ui.heading("Carbon dioxide");
                let current_co2 = sim.active_planet().atmosphere.co2_ppm;
                ui.horizontal(|ui| {
                    ui.label(format!("Current {current_co2:.1} ppm"));
                    ui.label(
                        egui::RichText::new(format!(
                            "initial {:.1} ppm · Δ {:+.1}",
                            initial.co2_ppm,
                            current_co2 - initial.co2_ppm
                        ))
                        .size(10.0)
                        .color(muted()),
                    );
                });
                ui.add(
                    egui::Slider::new(&mut state.co2_edit, 10.0..=5_000_000.0)
                        .logarithmic(true)
                        .text("Target atmospheric CO₂ (ppm)"),
                );
                ui.checkbox(
                    &mut state.co2_redistribute,
                    "Redistribute carbon between ocean / geological reservoirs",
                );
                ui.horizontal(|ui| {
                    if ui.button("Apply immediately").clicked() {
                        sim.active_planet_mut()
                            .set_co2_ppm(state.co2_edit, state.co2_redistribute);
                        state.status = Some((
                            format!("Atmospheric CO₂ set to {:.1} ppm.", state.co2_edit),
                            false,
                        ));
                    }
                    if ui.button("Set as gradual ramp target").clicked() {
                        let planet = sim.active_planet_mut();
                        planet.co2_ramp_target_ppm = state.co2_edit;
                        planet.co2_ramp_ppm_per_year =
                            planet.co2_ramp_ppm_per_year.max(0.25);
                        state.status = Some(("CO₂ ramp target updated.".into(), false));
                    }
                });
                {
                    let planet = sim.active_planet_mut();
                    ui.horizontal(|ui| {
                        ui.label("Ramp rate (ppm/yr)");
                        ui.add(
                            egui::DragValue::new(&mut planet.co2_ramp_ppm_per_year)
                                .speed(0.05)
                                .range(0.0..=1.0e5),
                        );
                        ui.label("target (ppm)");
                        ui.add(
                            egui::DragValue::new(&mut planet.co2_ramp_target_ppm)
                                .speed(5.0)
                                .range(1.0..=1.0e6),
                        );
                    });
                    ui.label(
                        egui::RichText::new(
                            "Carbon exchange with the ocean and weathering continue during the ramp. \
                             Immediate changes without redistribution change the total carbon inventory.",
                        )
                        .size(9.5)
                        .color(muted()),
                    );
                }
                ui.separator();

                ui.heading("Orbital and stellar controls");
                {
                    let planet = sim.active_planet_mut();
                    let mut tilt = planet.config.axial_tilt_deg;
                    if ui
                        .add(
                            egui::Slider::new(&mut tilt, 0.0..=90.0)
                                .text("Axial tilt (°)"),
                        )
                        .changed()
                    {
                        planet.set_axial_tilt_deg(tilt);
                    }
                    let mut solar = planet.solar_multiplier;
                    if ui
                        .add(
                            egui::Slider::new(&mut solar, 0.3..=2.0)
                                .text("Stellar flux multiplier"),
                        )
                        .changed()
                    {
                        planet.set_solar_multiplier(solar);
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "Tilt: current {:.2}° · initial {:.2}° · Δ {:+.2}°",
                            planet.config.axial_tilt_deg,
                            initial.axial_tilt_deg,
                            planet.config.axial_tilt_deg - initial.axial_tilt_deg
                        ))
                        .size(9.5)
                        .color(muted()),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "Stellar multiplier: current {:.3}× · initial 1.000× · Δ {:+.3}×",
                            planet.solar_multiplier,
                            planet.solar_multiplier - 1.0
                        ))
                        .size(9.5)
                        .color(muted()),
                    );
                    ui.horizontal(|ui| {
                        ui.label("Tilt ramp target (°)");
                        ui.add(
                            egui::DragValue::new(&mut planet.tilt_ramp_target_deg)
                                .speed(0.1)
                                .range(0.0..=90.0),
                        );
                        ui.label("rate (°/Myr)");
                        ui.add(
                            egui::DragValue::new(&mut planet.tilt_ramp_deg_per_myr)
                                .speed(0.05)
                                .range(0.0..=1_000.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Stellar ramp target (×)");
                        ui.add(
                            egui::DragValue::new(&mut planet.solar_ramp_target)
                                .speed(0.005)
                                .range(0.2..=3.0),
                        );
                        ui.label("rate (×/yr)");
                        ui.add(
                            egui::DragValue::new(&mut planet.solar_ramp_per_year)
                                .speed(1.0e-6)
                                .range(0.0..=1.0)
                                .max_decimals(8),
                        );
                    });
                    ui.label(
                        egui::RichText::new(format!(
                            "Current effective stellar multiplier {:.3}× (including event forcing). \
                             Axial tilt changes insolation distribution and seasonal amplitude.",
                            planet.effective_solar_multiplier()
                        ))
                        .size(9.5)
                        .color(muted()),
                    );
                }
                ui.separator();

                ui.heading("Atmosphere and surface");
                {
                    let planet = sim.active_planet_mut();
                    let mut pressure = planet.atmosphere.pressure_kpa;
                    if ui
                        .add(
                            egui::Slider::new(&mut pressure, 0.1..=500.0)
                                .logarithmic(true)
                                .text("Surface pressure (kPa)"),
                        )
                        .changed()
                    {
                        planet.set_pressure_kpa(pressure);
                    }
                    let mut cloud = planet.atmosphere.cloud_coverage;
                    if ui
                        .add(
                            egui::Slider::new(&mut cloud, 0.0..=1.0)
                                .text("Cloud coverage"),
                        )
                        .changed()
                    {
                        planet.set_cloud_coverage(cloud);
                    }
                    let mut cloud_albedo = planet.climate.cloud_albedo_factor;
                    if ui
                        .add(
                            egui::Slider::new(&mut cloud_albedo, 0.3..=1.8)
                                .text("Cloud albedo factor"),
                        )
                        .changed()
                    {
                        planet.set_cloud_albedo_factor(cloud_albedo);
                    }
                    let mut albedo = planet.climate.surface_albedo_offset;
                    if ui
                        .add(
                            egui::Slider::new(&mut albedo, -0.25..=0.35)
                                .text("Surface albedo perturbation"),
                        )
                        .changed()
                    {
                        planet.set_surface_albedo_offset(albedo);
                    }
                    let mut heat = planet.config.ocean_heat_capacity;
                    if ui
                        .add(
                            egui::Slider::new(&mut heat, 0.1..=4.0)
                                .text("Ocean mixed-layer heat capacity"),
                        )
                        .changed()
                    {
                        planet.set_ocean_heat_capacity(heat);
                    }
                    let mut circulation = planet.base_ocean_circulation;
                    if ui
                        .add(
                            egui::Slider::new(&mut circulation, 0.0..=1.0)
                                .text("Meridional ocean heat transport"),
                        )
                        .changed()
                    {
                        planet.set_ocean_circulation(circulation);
                    }
                }
                ui.add_space(4.0);
                ui.heading("Water inventory");
                let current_water = sim.active_planet().hydrology.total_water_km3;
                ui.label(format!(
                    "Current {:.3e} km³ · initial {:.3e} km³",
                    current_water, initial.water.inventory_km3
                ));
                ui.add(
                    egui::Slider::new(&mut state.water_edit, 1.0e5..=3.0e9)
                        .logarithmic(true)
                        .text("Total water inventory (km³)"),
                );
                if ui.button("Apply water inventory").clicked() {
                    sim.active_planet_mut()
                        .set_total_water_km3(state.water_edit);
                    state.status = Some(("Water inventory rebalanced; sea level re-solved.".into(), false));
                }
                ui.label(
                    egui::RichText::new(
                        "Changing total water redistributes the ocean and changes sea level; land coverage is a consequence, not an independent slider.",
                    )
                    .size(9.5)
                    .color(muted()),
                );
                ui.label(
                    egui::RichText::new(
                        "Restart-only: radius, mass, seed, grid resolution, initial temperature/ocean temperature and initial ice coverage. These regenerate the planet rather than mutate it.",
                    )
                    .size(9.5)
                    .color(egui::Color32::from_rgb(180, 145, 100)),
                );
                ui.separator();

                ui.heading("Rotation and gravity");
                {
                    let planet = sim.active_planet_mut();
                    let mut rotation = planet.config.rotation_period_hours;
                    if ui
                        .add(
                            egui::Slider::new(&mut rotation, 2.0..=200.0)
                                .logarithmic(true)
                                .text("Rotation period (hours)"),
                        )
                        .changed()
                    {
                        planet.set_rotation_period_hours(rotation);
                    }
                    let mut gravity = planet.config.gravity;
                    if ui
                        .add(
                            egui::Slider::new(&mut gravity, 0.5..=30.0)
                                .text("Surface gravity (m/s²)"),
                        )
                        .changed()
                    {
                        planet.set_gravity(gravity);
                    }
                    ui.label(
                        egui::RichText::new(
                            "Rotation changes the model's heat-transport/weather scaling; gravity changes the atmospheric scale height and local pressure. Neither is a full circulation solver.",
                        )
                        .size(9.5)
                        .color(muted()),
                    );
                }
                ui.separator();
                ui.heading("Parameter history");
                {
                    let history = sim.active_history();
                    ui.label(egui::RichText::new("Global temperature").size(10.0));
                    widgets::draw_history_chart(
                        ui,
                        history,
                        ChartMetric::Temperature,
                        1.0,
                        82.0,
                    );
                    ui.label(egui::RichText::new("Atmospheric CO₂").size(10.0));
                    widgets::draw_history_chart(ui, history, ChartMetric::Co2, 1.0, 82.0);
                }
            });
        });
    state.show_physics = open;
    if !open {
        state.physics_was_open_last = false;
    }
}

fn statistics_window(
    ctx: &egui::Context,
    state: &mut UiState,
    _view: &mut ViewState,
    mut simulation: Option<&mut Simulation>,
) {
    let mut open = state.show_statistics;
    egui::Window::new("Planet statistics")
        .open(&mut open)
        .default_width(560.0)
        .show(ctx, |ui| {
            let Some(sim) = simulation.as_deref_mut() else {
                ui.label("No active simulation.");
                return;
            };
            let stats = sim.active_planet().stats.clone();
            let biome = sim.active_planet().dominant_biome();
            let history_len = sim.active_history().samples.len();
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format_age(stats.age_years))
                        .size(18.0)
                        .strong()
                        .color(accent()),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{} · {history_len} history samples",
                        biome.name()
                    ))
                    .size(10.5)
                    .color(muted()),
                );
            });
            egui::ScrollArea::vertical().max_height(640.0).show(ui, |ui| {
                egui::Grid::new("full_stats")
                    .num_columns(4)
                    .spacing([18.0, 5.0])
                    .show(ui, |ui| {
                        stat_pair(ui, "Mean temperature", format!("{:.2} °C", stats.mean_temp_c));
                        stat_pair(ui, "Min / max temperature", format!("{:.1} / {:.1} °C", stats.min_temp_c, stats.max_temp_c));
                        stat_pair(ui, "Ocean temperature", format!("{:.2} °C", stats.mean_ocean_temp_c));
                        stat_pair(ui, "Land / water", format!("{:.1}% / {:.1}%", stats.land_fraction * 100.0, stats.water_fraction * 100.0));
                        stat_pair(ui, "Sea level", format!("{:+.1} m", stats.sea_level_m));
                        stat_pair(ui, "Sea-level anomaly", format!("{:+.1} m", stats.sea_level_anomaly_m));
                        stat_pair(ui, "Total water", format!("{:.2e} km³", stats.total_water_km3));
                        stat_pair(ui, "Freshwater", format!("{:.2e} km³", stats.freshwater_km3));
                        stat_pair(ui, "Ocean volume", format!("{:.2e} km³", stats.ocean_volume_km3));
                        stat_pair(ui, "Ice volume", format!("{:.2e} km³", stats.ice_volume_km3));
                        stat_pair(ui, "Glacier coverage", format!("{:.2}%", stats.glacier_fraction * 100.0));
                        stat_pair(ui, "Sea-ice coverage", format!("{:.2}%", stats.sea_ice_fraction * 100.0));
                        stat_pair(ui, "Rivers / lakes", format!("{} / {}", stats.river_count, stats.lake_count));
                        stat_pair(ui, "Precipitation", format!("{:.0} mm/yr", stats.mean_precip_mm_yr));
                        stat_pair(ui, "Runoff", format!("{:.2e} km³/yr", stats.runoff_km3_per_yr));
                        stat_pair(ui, "CO₂", format!("{:.1} ppm", stats.co2_ppm));
                        stat_pair(ui, "CH₄", format!("{:.3} ppm", stats.ch4_ppm));
                        stat_pair(ui, "O₂ / N₂", format!("{:.2}% / {:.2}%", stats.o2_fraction * 100.0, stats.n2_fraction * 100.0));
                        stat_pair(ui, "Pressure", format!("{:.2} kPa", stats.pressure_kpa));
                        stat_pair(ui, "Cloud cover", format!("{:.1}%", stats.cloud_coverage * 100.0));
                        stat_pair(ui, "Aerosol τ", format!("{:.3}", stats.aerosol_optical_depth));
                        stat_pair(ui, "Dust τ", format!("{:.3}", stats.dust_optical_depth));
                        stat_pair(ui, "Planetary albedo", format!("{:.3}", stats.mean_albedo));
                        stat_pair(ui, "Greenhouse g", format!("{:.3}", stats.greenhouse_strength));
                        stat_pair(ui, "Plates", format!("{}", stats.plate_count));
                        stat_pair(ui, "Active eruptions", format!("{}", stats.active_eruptions));
                        stat_pair(ui, "Magnetic field", format!("{:.0}%", stats.magnetic_field * 100.0));
                        stat_pair(ui, "Land elevation", format!("mean {:.0} m · range {:.0}…{:.0} m", stats.mean_land_elevation_m, stats.min_elevation_m, stats.max_elevation_m));
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("stats_metric")
                        .selected_text(state.chart_metric.label())
                        .show_ui(ui, |ui| {
                            for metric in ChartMetric::ALL {
                                ui.selectable_value(&mut state.chart_metric, metric, metric.label());
                            }
                        });
                    ui.label("time window");
                    ui.add(
                        egui::Slider::new(&mut state.chart_fraction, 0.02..=1.0)
                            .show_value(false),
                    );
                });
                widgets::draw_history_chart(
                    ui,
                    sim.active_history(),
                    state.chart_metric,
                    state.chart_fraction,
                    200.0,
                );
                ui.label(
                    egui::RichText::new(
                        "All values are computed from the current cell grid and global reservoirs. No statistics are generated for display alone.",
                    )
                    .size(10.0)
                    .color(muted()),
                );
            });
        });
    state.show_statistics = open;
}

fn catastrophe_window(
    ctx: &egui::Context,
    state: &mut UiState,
    view: &mut ViewState,
    mut simulation: Option<&mut Simulation>,
) {
    let mut open = state.show_catastrophes;
    egui::Window::new("Catastrophe management")
        .open(&mut open)
        .default_width(540.0)
        .show(ctx, |ui| {
            let Some(sim) = simulation.as_deref_mut() else {
                ui.label("No active simulation.");
                return;
            };
            let n = sim.active_planet().grid.cell_count();
            let selected = view.selected_cell.filter(|c| *c < n);
            let target = if state.event_at_selected {
                selected
            } else {
                None
            }
            .unwrap_or_else(|| ((sim.active_planet().time_years as usize) * 7919).max(1) % n);
            let (lat, lon) = (
                sim.active_planet().grid.lat_deg(target),
                sim.active_planet().grid.lon_deg(target),
            );
            ui.horizontal(|ui| {
                ui.checkbox(&mut state.event_at_selected, "Use selected region");
                ui.label(format!("Target cell {target} · {lat:.1}°, {lon:.1}°"));
                if ui.button("Pick another").clicked() {
                    view.selected_cell = Some(target);
                }
            });
            let mut log: Vec<(EventKind, String, String)> = Vec::new();
            egui::ScrollArea::vertical().max_height(650.0).show(ui, |ui| {
                ui.heading("Volcanic activity");
                ui.add(
                    egui::Slider::new(&mut state.volcano_intensity, 0.5..=10.0)
                        .text("Intensity"),
                );
                ui.add(
                    egui::Slider::new(&mut state.volcano_duration, 1.0..=10_000.0)
                        .logarithmic(true)
                        .text("Duration (years)"),
                );
                if ui.button("Trigger eruption").clicked() {
                    let outcome = sim.active_planet_mut().trigger_volcano(
                        target,
                        state.volcano_intensity,
                        state.volcano_duration,
                    );
                    log.push((
                        EventKind::Catastrophe(outcome.kind),
                        outcome.title,
                        outcome.description,
                    ));
                }
                ui.label(
                    egui::RichText::new(
                        "Eruptions inject aerosols (short cooling) and CO₂ from the geological reservoir (longer warming), and add lava relief.",
                    )
                    .size(9.5)
                    .color(muted()),
                );
                ui.separator();

                ui.heading("Asteroid impact");
                ui.add(
                    egui::Slider::new(&mut state.asteroid_diameter, 0.05..=50.0)
                        .logarithmic(true)
                        .text("Impactor diameter (km)"),
                );
                ui.add(
                    egui::Slider::new(&mut state.asteroid_velocity, 5.0..=70.0)
                        .text("Impact velocity (km/s)"),
                );
                ui.add(
                    egui::Slider::new(&mut state.asteroid_angle, 5.0..=90.0)
                        .text("Impact angle (°)"),
                );
                let energy = crate::catastrophes::impact_energy(
                    state.asteroid_diameter,
                    state.asteroid_velocity,
                    state.asteroid_angle,
                );
                ui.label(format!(
                    "Kinetic energy ≈ {:.2e} J · crater-scale deformation and dust opacity are computed from energy",
                    energy
                ));
                if ui.button("Impact selected region").clicked() {
                    let (outcome, _) = sim.active_planet_mut().trigger_asteroid(
                        target,
                        state.asteroid_diameter,
                        state.asteroid_velocity,
                        state.asteroid_angle,
                    );
                    log.push((
                        EventKind::Catastrophe(outcome.kind),
                        outcome.title,
                        outcome.description,
                    ));
                }
                ui.separator();

                ui.heading("Regional weather");
                egui::ComboBox::from_id_salt("weather_kind")
                    .selected_text(state.weather_kind.label())
                    .show_ui(ui, |ui| {
                        for kind in crate::weather::WeatherKind::ALL {
                            ui.selectable_value(&mut state.weather_kind, kind, kind.label());
                        }
                    });
                ui.add(
                    egui::Slider::new(&mut state.weather_intensity, 0.05..=1.0)
                        .text("Intensity"),
                );
                ui.add(
                    egui::Slider::new(&mut state.weather_days, 0.5..=180.0)
                        .logarithmic(true)
                        .text("Duration (days)"),
                );
                if ui.button("Trigger weather event").clicked() {
                    let outcome = sim.active_planet_mut().trigger_weather(
                        state.weather_kind,
                        target,
                        state.weather_intensity,
                        state.weather_days,
                    );
                    log.push((EventKind::Weather(outcome.kind), outcome.title, outcome.description));
                }
                ui.separator();

                ui.heading("Planet-scale events");
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Glaciation forcing").clicked() {
                        let outcome = sim.active_planet_mut()
                            .trigger_ice_age(state.ice_age_duration);
                        log.push((
                            EventKind::Catastrophe(outcome.kind),
                            outcome.title,
                            outcome.description,
                        ));
                    }
                    if ui.button("Inject CO₂").clicked() {
                        let outcome = sim.active_planet_mut().trigger_warming(state.warming_ppm);
                        log.push((
                            EventKind::Catastrophe(outcome.kind),
                            outcome.title,
                            outcome.description,
                        ));
                    }
                    if ui.button("Aerosol cooling").clicked() {
                        let outcome = sim.active_planet_mut().trigger_rapid_cooling(state.cooling_tau);
                        log.push((
                            EventKind::Catastrophe(outcome.kind),
                            outcome.title,
                            outcome.description,
                        ));
                    }
                    if ui.button("Ocean circulation slowdown").clicked() {
                        let outcome = sim
                            .active_planet_mut()
                            .trigger_ocean_circulation(50_000.0);
                        log.push((
                            EventKind::Catastrophe(outcome.kind),
                            outcome.title,
                            outcome.description,
                        ));
                    }
                    if ui.button("Magnetic reversal").clicked() {
                        let outcome = sim.active_planet_mut().trigger_magnetic_reversal();
                        log.push((
                            EventKind::Catastrophe(outcome.kind),
                            outcome.title,
                            outcome.description,
                        ));
                    }
                });
                ui.add(
                    egui::Slider::new(&mut state.ice_age_duration, 1_000.0..=2_000_000.0)
                        .logarithmic(true)
                        .text("Glaciation forcing duration (yr)"),
                );
                ui.add(
                    egui::Slider::new(&mut state.warming_ppm, 10.0..=20_000.0)
                        .logarithmic(true)
                        .text("CO₂ injection (ppm)"),
                );
                ui.add(
                    egui::Slider::new(&mut state.cooling_tau, 0.05..=5.0)
                        .text("Aerosol optical depth"),
                );
                ui.separator();

                ui.heading("Random event settings");
                let settings = &mut sim.active_planet_mut().catastrophes.settings;
                ui.checkbox(&mut settings.random_events, "Random catastrophes enabled");
                ui.add(
                    egui::Slider::new(&mut settings.eruption_rate_per_myr, 0.0..=5.0)
                        .text("Large eruptions / Myr"),
                );
                ui.add(
                    egui::Slider::new(&mut settings.impact_rate_per_myr, 0.0..=1.0)
                        .text("Impacts / Myr"),
                );
                ui.add(
                    egui::Slider::new(&mut settings.ice_age_rate_per_myr, 0.0..=0.2)
                        .text("Glaciation forcings / Myr"),
                );
                ui.separator();

                let planet = sim.active_planet();
                ui.heading("Active events");
                if planet.catastrophes.volcanoes.is_empty()
                    && planet.catastrophes.forcings.is_empty()
                    && planet.weather.events.is_empty()
                {
                    ui.label(egui::RichText::new("No active events.").size(10.5).color(muted()));
                }
                for v in &planet.catastrophes.volcanoes {
                    ui.label(format!(
                        "Volcanic eruption · intensity {:.1} · {:.0} yr remaining · cell {}",
                        v.intensity, v.remaining_years, v.cell
                    ));
                }
                for f in &planet.catastrophes.forcings {
                    ui.label(format!(
                        "{} · {:.0} yr remaining",
                        f.label, f.remaining_years
                    ));
                }
                for w in planet.weather.events.iter().take(12) {
                    ui.label(format!(
                        "{} · {:.1} days remaining · cell {}",
                        w.kind.label(),
                        w.remaining_years * 365.25,
                        w.cell
                    ));
                }
            });
            for (kind, title, description) in log {
                sim.log_active_event(kind, title, description);
            }
        });
    state.show_catastrophes = open;
}

fn history_window(
    ctx: &egui::Context,
    state: &mut UiState,
    view: &mut ViewState,
    mut simulation: Option<&mut Simulation>,
) {
    let mut open = state.show_history;
    egui::Window::new("Planet history and checkpoints")
        .open(&mut open)
        .default_width(600.0)
        .show(ctx, |ui| {
            let Some(sim) = simulation.as_deref_mut() else {
                ui.label("No active simulation.");
                return;
            };
            ui.label(
                egui::RichText::new(format!(
                    "Simulation age {}. Checkpoints store complete states; the timeline resolution is the checkpoint interval, not individual steps.",
                    format_age(sim.main.planet.time_years)
                ))
                .size(10.0)
                .color(muted()),
            );
            ui.separator();
            let mut jump = None;
            ui.heading("Checkpoints");
            egui::ScrollArea::vertical().id_salt("checkpoints").max_height(170.0).show(ui, |ui| {
                for (index, cp) in sim.checkpoints.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} · {}",
                                format_age(cp.time_years),
                                cp.label
                            ))
                            .size(10.5)
                            .monospace(),
                        );
                        if ui.small_button("Restore").clicked() {
                            jump = Some(index);
                        }
                    });
                }
            });
            if let Some(index) = jump {
                match sim.jump_to_checkpoint(index) {
                    Ok(()) => {
                        view.selected_cell = None;
                        state.status = Some((
                            "Restored checkpoint. Later samples/events were truncated.".into(),
                            false,
                        ));
                    }
                    Err(err) => state.status = Some((err, true)),
                }
            }
            if ui.button("Store checkpoint at current time").clicked() {
                let label = format_age(sim.main.planet.time_years);
                sim.push_checkpoint(label);
            }
            ui.separator();
            ui.heading("Event log");
            egui::ScrollArea::vertical().id_salt("events").max_height(230.0).show(ui, |ui| {
                let events: Vec<_> = sim.active_history().events.iter().rev().take(160).cloned().collect();
                if events.is_empty() {
                    ui.label(egui::RichText::new("No events recorded.").size(10.5).color(muted()));
                }
                for event in events {
                    let color = match event.kind {
                        EventKind::Catastrophe(_) => egui::Color32::from_rgb(245, 160, 90),
                        EventKind::Weather(_) => egui::Color32::from_rgb(110, 180, 230),
                        EventKind::Climate => egui::Color32::from_rgb(140, 210, 180),
                        EventKind::Tectonic => egui::Color32::from_rgb(200, 160, 110),
                        _ => egui::Color32::from_rgb(160, 175, 190),
                    };
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {}",
                            format_age(event.time_years),
                            event.title
                        ))
                        .size(10.5)
                        .color(color),
                    );
                    ui.label(
                        egui::RichText::new(event.description.as_str())
                            .size(9.5)
                            .color(muted()),
                    );
                    ui.add_space(2.0);
                }
            });
            ui.separator();
            let history = sim.active_history();
            widgets::draw_history_chart(ui, history, ChartMetric::Temperature, 1.0, 150.0);
            ui.horizontal(|ui| {
                if ui.button("Restart from initial configuration").clicked() {
                    state.confirm_restart = true;
                    state.show_settings = true;
                }
                if ui.button("Open What If? comparison").clicked() {
                    state.show_experiments = true;
                }
            });
        });
    state.show_history = open;
}

fn saves_window(
    ctx: &egui::Context,
    state: &mut UiState,
    view: &mut ViewState,
    simulation: Option<&mut Simulation>,
) {
    let mut open = state.show_saves;
    egui::Window::new("Saved planets")
        .open(&mut open)
        .default_width(620.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Refresh").clicked() {
                    crate::ui::refresh_saves(state);
                }
                if ui.button("Save current planet").clicked() {
                    if let Some(sim) = simulation.as_ref() {
                        match state.save_manager.save(sim) {
                            Ok(path) => {
                                state.status =
                                    Some((format!("Saved to {}", path.display()), false));
                                crate::ui::refresh_saves(state);
                            }
                            Err(err) => state.status = Some((format!("Save failed: {err}"), true)),
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Save as");
                ui.text_edit_singleline(&mut state.save_name);
                if ui.button("Save copy").clicked() {
                    if let Some(sim) = simulation.as_ref() {
                        let name = if state.save_name.trim().is_empty() {
                            sim.main.planet.config.name.clone()
                        } else {
                            state.save_name.clone()
                        };
                        match state.save_manager.save_as(sim, Some(&name)) {
                            Ok(path) => {
                                state.status =
                                    Some((format!("Saved copy to {}", path.display()), false));
                                crate::ui::refresh_saves(state);
                            }
                            Err(err) => state.status = Some((format!("Save failed: {err}"), true)),
                        }
                    }
                }
            });
            ui.label(
                egui::RichText::new(format!(
                    "Storage: {} · format v{}",
                    state.save_manager.dir.display(),
                    crate::save_system::FORMAT_VERSION
                ))
                .size(9.5)
                .color(muted()),
            );
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    let saves = state.saves.clone();
                    if saves.is_empty() {
                        ui.label(
                            egui::RichText::new("No saved planets in this folder.")
                                .size(10.5)
                                .color(muted()),
                        );
                    }
                    for meta in saves {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(meta.name.as_str())
                                    .size(12.0)
                                    .strong()
                                    .color(accent()),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "{:.2} MB",
                                    meta.size_bytes as f64 / 1.0e6
                                ))
                                .size(9.5)
                                .color(muted()),
                            );
                            if ui.small_button("Load").clicked() {
                                state.pending_load = Some(meta.name.clone());
                                state.status = Some(("Loading…".into(), false));
                            }
                            if ui.small_button("Duplicate").clicked() {
                                let copy_name = format!("{} copy", meta.name);
                                match state.save_manager.duplicate(&meta.name, &copy_name) {
                                    Ok(_) => crate::ui::refresh_saves(state),
                                    Err(err) => state.status = Some((format!("{err}"), true)),
                                }
                            }
                            if ui.small_button("Rename").clicked() {
                                let new_name = if state.save_name.trim().is_empty() {
                                    format!("{} renamed", meta.name)
                                } else {
                                    state.save_name.clone()
                                };
                                match state.save_manager.rename(&meta.name, &new_name) {
                                    Ok(_) => crate::ui::refresh_saves(state),
                                    Err(err) => state.status = Some((format!("{err}"), true)),
                                }
                            }
                            if ui.small_button("Delete").clicked() {
                                if state.confirm_delete.as_deref() == Some(meta.name.as_str()) {
                                    match state.save_manager.delete(&meta.name) {
                                        Ok(()) => {
                                            state.confirm_delete = None;
                                            crate::ui::refresh_saves(state);
                                        }
                                        Err(err) => state.status = Some((format!("{err}"), true)),
                                    }
                                } else {
                                    state.confirm_delete = Some(meta.name.clone());
                                }
                            }
                            if state.confirm_delete.as_deref() == Some(meta.name.as_str()) {
                                ui.label(
                                    egui::RichText::new("click Delete again to confirm")
                                        .size(9.0)
                                        .color(danger()),
                                );
                            }
                        });
                    }
                });
            let _ = view;
        });
    state.show_saves = open && state.pending_load.is_none();
}

fn experiments_window(
    ctx: &egui::Context,
    state: &mut UiState,
    _view: &mut ViewState,
    mut simulation: Option<&mut Simulation>,
) {
    let mut open = state.show_experiments;
    egui::Window::new("What If? — planet comparison")
        .open(&mut open)
        .default_width(680.0)
        .show(ctx, |ui| {
            let Some(sim) = simulation.as_deref_mut() else {
                ui.label("No active simulation.");
                return;
            };
            ui.label(
                egui::RichText::new(
                    "Each branch is a complete planet state, not a separate visual preset. Forks advance on the same clock and comparisons use the recorded history.",
                )
                .size(10.0)
                .color(muted()),
            );
            let mut selected = sim.active;
            ui.horizontal_wrapped(|ui| {
                for index in 0..sim.run_count() {
                    let run = sim.run_at(index);
                    let color = egui::Color32::from_rgb(
                        (run.color[0] * 255.0) as u8,
                        (run.color[1] * 255.0) as u8,
                        (run.color[2] * 255.0) as u8,
                    );
                    if ui
                        .selectable_label(selected == index, egui::RichText::new(run.label.as_str()).color(color))
                        .clicked()
                    {
                        selected = index;
                    }
                }
            });
            sim.set_active(selected);
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut state.fork_name);
                if ui.button("Create fork from active").clicked() {
                    let label = if state.fork_name.trim().is_empty() {
                        format!("Experiment {}", sim.forks.len() + 1)
                    } else {
                        state.fork_name.clone()
                    };
                    let index = sim.create_fork(label);
                    sim.set_active(index);
                    state.status = Some(("Created an independent planet fork.".into(), false));
                }
                if sim.active > 0 {
                    let index = sim.active;
                    let label = sim.active_run().label.clone();
                    if ui.button("Delete active fork").clicked() {
                        sim.delete_fork(index);
                        state.status = Some((format!("Deleted fork '{label}'."), false));
                    }
                }
            });
            ui.separator();
            ui.heading("Quick experiments");
            ui.horizontal_wrapped(|ui| {
                if ui.button("Double CO₂").clicked() {
                    let target = sim.active_planet().atmosphere.co2_ppm * 2.0;
                    let index = sim.create_fork("CO₂ ×2".to_string());
                    sim.set_active(index);
                    sim.active_planet_mut().set_co2_ppm(target, true);
                    sim.log_active_event(
                        EventKind::User,
                        "What-if: doubled CO₂",
                        format!("Forked with atmospheric CO₂ set to {target:.0} ppm."),
                    );
                }
                if ui.button("Stellar flux −20%").clicked() {
                    let index = sim.create_fork("Dim star".to_string());
                    sim.set_active(index);
                    let target = sim.active_planet().solar_multiplier * 0.8;
                    sim.active_planet_mut().set_solar_multiplier(target);
                    sim.log_active_event(
                        EventKind::User,
                        "What-if: dim star",
                        "Stellar flux reduced by 20% in a forked planet.",
                    );
                }
                if ui.button("Tilt +15°").clicked() {
                    let index = sim.create_fork("High tilt".to_string());
                    sim.set_active(index);
                    let target = (sim.active_planet().config.axial_tilt_deg + 15.0).min(90.0);
                    sim.active_planet_mut().set_axial_tilt_deg(target);
                    sim.log_active_event(
                        EventKind::User,
                        "What-if: higher axial tilt",
                        format!("Axial tilt raised to {target:.1}°."),
                    );
                }
                if ui.button("Albedo +0.1").clicked() {
                    let index = sim.create_fork("Bright surface".to_string());
                    sim.set_active(index);
                    sim.active_planet_mut().set_surface_albedo_offset(0.1);
                    sim.log_active_event(
                        EventKind::User,
                        "What-if: brighter surface",
                        "Surface albedo perturbation +0.1 applied.",
                    );
                }
                if ui.button("Asteroid impact").clicked() {
                    let index = sim.create_fork("Impact experiment".to_string());
                    sim.set_active(index);
                    let n = sim.active_planet().grid.cell_count();
                    let cell = (sim.active_planet().time_years as usize * 7919) % n.max(1);
                    let (outcome, _) = sim
                        .active_planet_mut()
                        .trigger_asteroid(cell, 8.0, 22.0, 45.0);
                    sim.log_active_event(
                        EventKind::Catastrophe(outcome.kind),
                        outcome.title,
                        outcome.description,
                    );
                }
                if ui.button("Force glaciation").clicked() {
                    let index = sim.create_fork("Glaciation experiment".to_string());
                    sim.set_active(index);
                    let outcome = sim.active_planet_mut().trigger_ice_age(250_000.0);
                    sim.log_active_event(
                        EventKind::Catastrophe(outcome.kind),
                        outcome.title,
                        outcome.description,
                    );
                }
            });
            ui.separator();
            ui.heading("Synchronised comparison");
            ui.add(
                egui::Slider::new(&mut state.compare_fraction, 0.02..=1.0)
                    .text("Timeline window"),
            );
            if sim.forks.is_empty() {
                ui.label(
                    egui::RichText::new("Create a fork to compare planetary histories.")
                        .size(10.5)
                        .color(muted()),
                );
            } else {
                let end = sim
                    .forks
                    .iter()
                    .filter_map(|f| f.history.samples.last().map(|s| s.time_years))
                    .fold(f64::INFINITY, f64::min)
                    .min(sim.main.planet.time_years);
                let start = sim
                    .main
                    .history
                    .samples
                    .first()
                    .map(|s| s.time_years)
                    .unwrap_or(0.0);
                let at = start + (end - start) * state.compare_fraction as f64;
                egui::Grid::new("comparison_table")
                    .num_columns(6)
                    .spacing([12.0, 5.0])
                    .show(ui, |ui| {
                        ui.strong("Planet");
                        ui.strong("ΔT °C");
                        ui.strong("ΔCO₂");
                        ui.strong("Δ sea level");
                        ui.strong("Δ ice");
                        ui.strong("Δ land");
                        ui.end_row();
                        for fork in &sim.forks {
                            let cmp = compare_at(&sim.main, fork, at);
                            ui.label(&fork.label);
                            ui.label(fmt_delta(cmp.delta_temp_c, "°C"));
                            ui.label(fmt_delta(cmp.delta_co2_ppm, "ppm"));
                            ui.label(fmt_delta(cmp.delta_sea_level_m, "m"));
                            ui.label(fmt_delta(cmp.delta_ice_fraction, "%"));
                            ui.label(fmt_delta(cmp.delta_land_fraction, "%"));
                            ui.end_row();
                        }
                    });
                let series: Vec<(&crate::history::History, egui::Color32, &str)> =
                    std::iter::once((
                        &sim.main.history,
                        egui::Color32::from_rgb(80, 200, 205),
                        sim.main.label.as_str(),
                    ))
                    .chain(sim.forks.iter().map(|f| {
                        (
                            &f.history,
                            egui::Color32::from_rgb(
                                (f.color[0] * 255.0) as u8,
                                (f.color[1] * 255.0) as u8,
                                (f.color[2] * 255.0) as u8,
                            ),
                            f.label.as_str(),
                        )
                    }))
                    .collect();
                egui::ComboBox::from_id_salt("compare_metric")
                    .selected_text(state.chart_metric.label())
                    .show_ui(ui, |ui| {
                        for metric in ChartMetric::ALL {
                            ui.selectable_value(&mut state.chart_metric, metric, metric.label());
                        }
                    });
                widgets::draw_multi_chart(
                    ui,
                    &series,
                    state.chart_metric,
                    state.compare_fraction,
                    210.0,
                );
                ui.label(
                    egui::RichText::new(format!("Comparison time: {}", format_age(at)))
                        .size(10.0)
                        .color(muted()),
                );
            }
        });
    state.show_experiments = open;
}

fn settings_window(
    ctx: &egui::Context,
    state: &mut UiState,
    view: &mut ViewState,
    mut simulation: Option<&mut Simulation>,
    visual: &mut PlanetVisual,
) {
    let mut open = state.show_settings;
    egui::Window::new("Application settings")
        .open(&mut open)
        .default_width(430.0)
        .show(ctx, |ui| {
            ui.heading("Visualisation");
            ui.checkbox(&mut view.show_rivers, "Show rivers");
            ui.checkbox(&mut view.show_clouds, "Show cloud veil");
            ui.checkbox(&mut view.show_atmosphere, "Show atmosphere shell");
            ui.add(
                egui::Slider::new(&mut view.elevation_exaggeration, 0.0..=80.0)
                    .text("Elevation exaggeration"),
            );
            ui.label(
                egui::RichText::new(
                    "The mesh is displaced for readability. Ocean surfaces are drawn at sea level; stored elevations are unchanged.",
                )
                .size(9.5)
                .color(muted()),
            );
            ui.separator();
            ui.heading("Storage");
            ui.label(
                egui::RichText::new(state.save_manager.dir.display().to_string())
                    .size(10.0)
                    .monospace(),
            );
            ui.label(
                egui::RichText::new("Saves are JSON + gzip with a versioned header. Topology is rebuilt from the subdivision level on load.")
                    .size(9.5)
                    .color(muted()),
            );
            ui.separator();
            ui.heading("Simulation");
            if let Some(sim) = simulation.as_ref() {
                ui.label(format!("Active planet: {}", sim.active_planet().config.name));
                ui.label(format!("Speed: {:.3} yr/s", sim.time.speed_years_per_second));
                ui.label(format!(
                    "Checkpoint resolution: stored every ~8% of elapsed time (max 10 states)."
                ));
            }
            if ui.button("Restart simulation from its initial configuration").clicked() {
                state.confirm_restart = true;
            }
            if state.confirm_restart {
                ui.label(
                    egui::RichText::new(
                        "This regenerates the planet from the initial configuration and discards forks/history. Confirm?",
                    )
                    .size(10.0)
                    .color(warning()),
                );
                ui.horizontal(|ui| {
                    if ui.button("Confirm restart").clicked() {
                        if let Some(sim) = simulation.as_deref_mut() {
                            match sim.restart() {
                                Ok(()) => {
                                    view.selected_cell = None;
                                    view.screen = Screen::Simulation;
                                    visual.force_rebuild = true;
                                    state.confirm_restart = false;
                                    state.status = Some(("Simulation restarted.".into(), false));
                                }
                                Err(err) => state.status = Some((err, true)),
                            }
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        state.confirm_restart = false;
                    }
                });
            }
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "Planet Simulator {} · Rust + Bevy · model limitations in docs/MODEL.md",
                    env!("CARGO_PKG_VERSION")
                ))
                .size(9.5)
                .color(muted()),
            );
        });
    state.show_settings = open;
}

fn help_window(ctx: &egui::Context, state: &mut UiState) {
    let mut open = state.show_help;
    egui::Window::new("How the model works")
        .open(&mut open)
        .default_width(560.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(620.0).show(ui, |ui| {
                ui.heading("Implemented");
                ui.label(
                    "• Icosphere surface with per-cell bedrock, water, ice, soil, climate and river state\n\
                     • Seeded fractal terrain, continent masks, plate-boundary relief and calibrated sea level\n\
                     • One-layer radiative energy balance with logarithmic CO₂ forcing, water-vapour feedback, clouds, ice-albedo and aerosols\n\
                     • Latitude/season-resolved temperature field with land/ocean thermal inertia and horizontal heat transport\n\
                     • Evaporation → precipitation → soil → runoff → downhill river routing → ocean; snow, glacier growth/melt and a mass-conserving sea level\n\
                     • Kinematic plate motion, boundary uplift/trenches, erosion, volcanism, seismic activity\n\
                     • Asteroid impacts with energy-scaled crater deformation and dust; volcanic gas/aerosol injections; ice-age forcing\n\
                     • Weather events with local effects, a full event log, adaptive history, checkpoints, forks and versioned saves",
                );
                ui.separator();
                ui.heading("Deliberate simplifications");
                ui.label(
                    "• No 3-D atmospheric or ocean general circulation; heat transport is a parameterised diffusive/meridional term.\n\
                     • Carbon reservoirs use illustrative ratios, not a calibrated biogeochemical model.\n\
                     • Lakes are diagnostic sinks of the routed water balance; their storage does not drive eustatic sea level.\n\
                     • Weather is regional and stochastic, not resolved by a fluid solver; at geological speeds it is subsampled.\n\
                     • Ice flow is mass-balance/thickness based; no ice-sheet dynamics solver.\n\
                     • Tectonics is kinematic; no mantle convection or isostasy.\n\
                     • The magnetic field is tracked and can reverse, but no dynamo or cosmic-ray climate coupling is claimed.\n\
                     • The 3-D mesh exaggerates elevation; all stored values are metres.",
                );
                ui.separator();
                ui.heading("Controls");
                ui.label(
                    "Drag with the left mouse button to orbit; scroll to zoom. Click the surface to select a cell. \
                     Space pauses; use the bottom bar for speed and queued steps. The left panel selects data layers; the right panel inspects the selected region.",
                );
                ui.separator();
                ui.heading("Status");
                ui.label(
                    "This is a functional simplified simulator, not a validated climate or geophysical model. \
                     Read docs/STATUS.md for the exact implemented / partial / planned matrix.",
                );
            });
        });
    state.show_help = open;
}

fn stat_pair(ui: &mut egui::Ui, label: &str, value: String) {
    ui.label(egui::RichText::new(label).size(9.5).color(muted()));
    ui.label(egui::RichText::new(value).size(10.5).monospace());
}

fn fmt_delta(value: Option<f32>, unit: &str) -> String {
    match value {
        Some(v) if unit == "%" => format!("{:+0.1}%", v * 100.0),
        Some(v) if unit == "ppm" => format!("{v:+.0} ppm"),
        Some(v) => format!("{v:+.2} {unit}"),
        None => "—".into(),
    }
}
