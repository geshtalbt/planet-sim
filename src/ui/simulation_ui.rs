use bevy::prelude::*;
use bevy_egui::egui;

use crate::app::{Screen, ViewState};
use crate::history::EventKind;
use crate::localization::{pick, tr, translated};
use crate::simulation::{Simulation, format_age};
use crate::time_engine::SPEED_PRESETS;
use crate::ui::{UiState, accent, danger, muted, widgets};

pub fn draw_simulation(
    root: &mut egui::Ui,
    ctx: &egui::Context,
    state: &mut UiState,
    view: &mut ViewState,
    sim: &mut Simulation,
    _app_exit: &mut MessageWriter<AppExit>,
) {
    top_bar(root, state, view, sim);
    bottom_bar(root, state, view, sim);
    left_panel(root, state, view, sim);
    right_panel(root, state, view, sim);
    let _ = ctx;
}

fn top_bar(root: &mut egui::Ui, state: &mut UiState, view: &mut ViewState, sim: &mut Simulation) {
    egui::Panel::top("sim_top").show(root, |ui| {
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            if ui.button(tr("☰ Menu")).clicked() {
                sim.time.paused = true;
                view.screen = Screen::MainMenu;
            }
            ui.separator();
            ui.label(
                egui::RichText::new(sim.active_planet().config.name.clone())
                    .size(19.0)
                    .strong()
                    .color(egui::Color32::from_rgb(225, 242, 245)),
            );
            ui.label(
                egui::RichText::new(format!(
                    "{} {} · {}",
                    tr("age"),
                    format_age(sim.active_planet().time_years),
                    if sim.time.paused {
                        tr("paused")
                    } else {
                        tr("running")
                    }
                ))
                .size(12.0)
                .color(if sim.time.paused { muted() } else { accent() }),
            );
            ui.separator();
            if ui.button(tr("Planetary physics")).clicked() {
                state.show_physics = true;
            }
            if ui.button(tr("Catastrophes")).clicked() {
                state.show_catastrophes = true;
            }
            if ui.button(tr("Statistics")).clicked() {
                state.show_statistics = true;
            }
            if ui.button(tr("History")).clicked() {
                state.show_history = true;
            }
            if ui.button(tr("What If?")).clicked() {
                state.show_experiments = true;
            }
            if ui.button(tr("Saves")).clicked() {
                state.show_saves = true;
                crate::ui::refresh_saves(state);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let target = match state.language {
                    crate::localization::Language::English => "RU",
                    crate::localization::Language::Russian => "EN",
                };
                if ui
                    .button(target)
                    .on_hover_text(tr("Language / Язык"))
                    .clicked()
                {
                    state.language = match state.language {
                        crate::localization::Language::English => {
                            crate::localization::Language::Russian
                        }
                        crate::localization::Language::Russian => {
                            crate::localization::Language::English
                        }
                    };
                    crate::localization::set_language(state.language);
                }
                if ui.button(tr("Settings")).clicked() {
                    state.show_settings = true;
                }
                if ui.button(tr("?")).clicked() {
                    state.show_help = true;
                }
                if let Some((message, is_error)) = &state.status {
                    ui.label(
                        egui::RichText::new(translated(message))
                            .size(10.5)
                            .color(if *is_error { danger() } else { accent() }),
                    );
                }
            });
        });
        if let Some(notice) = sim.notices.last() {
            ui.label(
                egui::RichText::new(translated(notice))
                    .size(10.0)
                    .color(muted()),
            );
        }
    });
}

fn bottom_bar(
    root: &mut egui::Ui,
    state: &mut UiState,
    view: &mut ViewState,
    sim: &mut Simulation,
) {
    egui::Panel::bottom("sim_bottom").show(root, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .add_sized(
                    [74.0, 30.0],
                    egui::Button::new(if sim.time.paused {
                        tr("▶ Run")
                    } else {
                        tr("⏸ Pause")
                    }),
                )
                .clicked()
            {
                sim.time.paused = !sim.time.paused;
            }
            if ui.button(tr("− speed")).clicked() {
                sim.time.slower();
            }
            egui::ComboBox::from_id_salt("speed_presets")
                .width(170.0)
                .selected_text(translated(SPEED_PRESETS[sim.time.speed_preset].0))
                .show_ui(ui, |ui| {
                    for (index, (label, _)) in SPEED_PRESETS.iter().enumerate() {
                        if ui
                            .selectable_label(sim.time.speed_preset == index, translated(label))
                            .clicked()
                        {
                            sim.time.set_preset(index);
                        }
                    }
                });
            if ui.button(tr("+ speed")).clicked() {
                sim.time.faster();
            }
            ui.separator();
            ui.label(tr("Step"));
            ui.add(
                egui::DragValue::new(&mut state.step_amount)
                    .speed(1.0)
                    .range(0.0..=1.0e9),
            );
            egui::ComboBox::from_id_salt("step_unit")
                .width(100.0)
                .selected_text(if state.step_unit_is_years {
                    tr("years")
                } else {
                    tr("million yr")
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.step_unit_is_years, true, tr("years"));
                    ui.selectable_value(&mut state.step_unit_is_years, false, tr("million yr"));
                });
            if ui.button(tr("Advance")).clicked() {
                let years =
                    state.step_amount as f64 * if state.step_unit_is_years { 1.0 } else { 1.0e6 };
                sim.time.queue_years(years);
                state.status = Some((
                    format!(
                        "{} {}.",
                        pick("Queued simulation time:", "В очередь добавлено:"),
                        format_age(years)
                    ),
                    false,
                ));
            }
            if ui.button(tr("Checkpoint now")).clicked() {
                let label = format_age(sim.main.planet.time_years);
                sim.push_checkpoint(label);
                state.status = Some(("Checkpoint stored.".into(), false));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{:.2}x · {}",
                        sim.time.speed_years_per_second,
                        translated(sim.time.preset_label())
                    ))
                    .size(12.0)
                    .monospace()
                    .color(accent()),
                );
                if sim.time.pending_years > 0.0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "queued {}",
                            format_age(sim.time.pending_years)
                        ))
                        .size(10.5)
                        .color(muted()),
                    );
                }
            });
        });
        ui.horizontal(|ui| {
            let history = sim.active_history();
            let (start, end) = history.time_range();
            let age = sim.active_planet().time_years;
            let fraction = if end > start {
                ((age - start) / (end - start)).clamp(0.0, 1.0) as f32
            } else {
                1.0
            };
            ui.add(
                egui::ProgressBar::new(fraction)
                    .desired_width((ui.available_width() - 320.0).max(120.0))
                    .text(format!(
                        "{} — {} · {} {}",
                        format_age(start),
                        format_age(age),
                        sim.checkpoints.len(),
                        tr("checkpoint(s)")
                    )),
            );
            if ui.button(tr("Timeline / checkpoints")).clicked() {
                state.show_history = true;
            }
            if ui.button(tr("Restart")).clicked() {
                state.confirm_restart = true;
                state.show_settings = true;
            }
            let _ = view;
        });
        ui.add_space(2.0);
    });
}

fn left_panel(
    root: &mut egui::Ui,
    state: &mut UiState,
    view: &mut ViewState,
    sim: &mut Simulation,
) {
    egui::Panel::left("sim_left")
        .resizable(true)
        .default_size(238.0)
        .show(root, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(tr("VISUALISATION LAYERS"))
                        .size(11.0)
                        .strong()
                        .color(accent()),
                );
                for layer in crate::rendering::MapLayer::ALL {
                    if ui
                        .selectable_label(view.layer == layer, layer.label())
                        .clicked()
                    {
                        view.layer = layer;
                    }
                }
                ui.separator();
                ui.label(
                    egui::RichText::new(tr("VIEW"))
                        .size(11.0)
                        .strong()
                        .color(accent()),
                );
                ui.checkbox(&mut view.show_rivers, tr("Rivers and flow lines"));
                ui.checkbox(&mut view.show_clouds, tr("Cloud veil (surface shading)"));
                ui.checkbox(&mut view.show_atmosphere, tr("Atmosphere shell"));
                ui.add(
                    egui::Slider::new(&mut view.elevation_exaggeration, 0.0..=80.0)
                        .text(tr("Elevation exaggeration")),
                );
                ui.label(
                    egui::RichText::new(tr(
                        "Render displacement only. Stored elevations remain real metres.",
                    ))
                    .size(9.5)
                    .color(muted()),
                );
                ui.separator();
                ui.label(
                    egui::RichText::new(tr("LEGEND"))
                        .size(11.0)
                        .strong()
                        .color(accent()),
                );
                widgets::draw_legend(ui, view.layer, sim.active_planet());
                ui.separator();
                if let Some(cell) = view.selected_cell {
                    if cell < sim.active_planet().grid.cell_count() {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} #{cell}\n{:.1}° {}, {:.1}° {}",
                                tr("Selected cell"),
                                sim.active_planet().grid.lat_deg(cell),
                                pick("lat", "шир."),
                                sim.active_planet().grid.lon_deg(cell),
                                pick("lon", "долг.")
                            ))
                            .size(10.5)
                            .color(muted()),
                        );
                    }
                } else {
                    ui.label(
                        egui::RichText::new(tr("Click the planet to inspect a region."))
                            .size(10.5)
                            .color(muted()),
                    );
                }
                let _ = state;
            });
        });
}

fn right_panel(
    root: &mut egui::Ui,
    state: &mut UiState,
    view: &mut ViewState,
    sim: &mut Simulation,
) {
    let mut local_action: Option<LocalAction> = None;
    egui::Panel::right("sim_right")
        .resizable(true)
        .default_size(310.0)
        .show(root, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(4.0);
                let stats = sim.active_planet().stats.clone();
                let planet = sim.active_planet();
                ui.label(egui::RichText::new(tr("PLANET STATE")).size(11.0).strong().color(accent()));
                egui::Grid::new("quick_stats")
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        stat(ui, "Mean temperature", format!("{:.1} °C", stats.mean_temp_c));
                        stat(ui, "Ocean temperature", format!("{:.1} °C", stats.mean_ocean_temp_c));
                        stat(ui, "CO₂", format!("{:.0} ppm", stats.co2_ppm));
                        stat(ui, "Pressure", format!("{:.1} kPa", stats.pressure_kpa));
                        stat(ui, "Sea level", format!("{:+.0} m", stats.sea_level_m));
                        stat(ui, "Ice coverage", format!("{:.1}%", stats.glacier_fraction * 100.0));
                        stat(ui, "Land", format!("{:.1}%", stats.land_fraction * 100.0));
                        stat(ui, "Precipitation", format!("{:.0} mm/yr", stats.mean_precip_mm_yr));
                        stat(ui, "Planetary albedo", format!("{:.3}", stats.mean_albedo));
                        stat(ui, "Rivers / lakes", format!("{} / {}", stats.river_count, stats.lake_count));
                        stat(ui, "Plates", format!("{}", stats.plate_count));
                        stat(ui, "Active eruptions", format!("{}", stats.active_eruptions));
                    });
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {} · {} {:.3}",
                        tr("Dominant biome"),
                        sim.active_planet().dominant_biome().name(),
                        tr("greenhouse g"),
                        stats.greenhouse_strength
                    ))
                    .size(10.0)
                    .color(muted()),
                );
                ui.separator();
                ui.label(egui::RichText::new(tr("SELECTED REGION")).size(11.0).strong().color(accent()));
                let cell = view
                    .selected_cell
                    .filter(|&c| c < planet.grid.cell_count());
                if let Some(cell) = cell {
                    let c = &planet.grid.cells[cell];
                    let sea = planet.sea_level_m();
                    ui.label(
                        egui::RichText::new(format!(
                            "{:.1}° {} · {:.1}° {} · {} {}",
                            planet.grid.lat_deg(cell),
                            pick("lat", "шир."),
                            planet.grid.lon_deg(cell),
                            pick("lon", "долг."),
                            tr("cell"),
                            cell
                        ))
                        .size(10.5)
                        .color(muted()),
                    );
                    egui::Grid::new("region_grid")
                        .num_columns(2)
                        .spacing([12.0, 3.0])
                        .show(ui, |ui| {
                            region(ui, "Surface", if c.bedrock_m < sea { "Ocean".to_string() } else { "Land".to_string() });
                            region(ui, "Elevation", format!("{:+.0} m", c.bedrock_m));
                            region(
                                ui,
                                "Water depth",
                                if c.water_depth_m > 0.0 {
                                    format!("{:.0} m", c.water_depth_m)
                                } else {
                                    "—".into()
                                },
                            );
                            region(ui, "Lake depth", format!("{:.1} m", c.lake_depth_m));
                            region(ui, "Temperature", format!("{:.1} °C", c.temp_c));
                            region(ui, "Soil water", format!("{:.3} m", c.soil_moisture_m));
                            region(ui, "Precipitation", format!("{:.0} mm/yr", c.precip_mm_yr));
                            region(ui, "Humidity", format!("{:.2}", c.humidity));
                            region(ui, "Local pressure", format!("{:.1} kPa", c.pressure_kpa));
                            region(ui, "Biome", c.biome.name().to_string());
                            region(ui, "Plate", format!("{}", c.plate));
                            region(ui, "Ice thickness", format!("{:.1} m", c.ice_thickness_m));
                            region(ui, "Discharge", format!("{:.1} m³/s", c.discharge_m3s));
                            region(ui, "Local dT", format!("{:+.1} °C", c.temp_offset_c));
                        });
                    ui.horizontal_wrapped(|ui| {
                        if ui.small_button(tr("Warm +5 °C")).clicked() {
                            local_action = Some(LocalAction::Warm(cell, 5.0));
                        }
                        if ui.small_button(tr("Cool −5 °C")).clicked() {
                            local_action = Some(LocalAction::Warm(cell, -5.0));
                        }
                        if ui.small_button(tr("Rain 100 mm")).clicked() {
                            local_action = Some(LocalAction::Rain(cell, 100.0));
                        }
                        if ui.small_button(tr("Drought")).clicked() {
                            local_action = Some(LocalAction::Drought(cell));
                        }
                        if ui.small_button(tr("Add lake 5 m")).clicked() {
                            local_action = Some(LocalAction::Lake(cell, 5.0));
                        }
                        if ui.small_button(tr("Volcano")).clicked() {
                            local_action = Some(LocalAction::Volcano(cell));
                        }
                    });
                    if ui.small_button(tr("Clear persistent local forcing")).clicked() {
                        local_action = Some(LocalAction::ResetForcing(cell));
                    }
                    ui.label(
                        egui::RichText::new(
                            tr("Temperature offsets are persistent; rainfall, lakes and volcanoes modify the stored regional state."),
                        )
                        .size(9.5)
                        .color(muted()),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(tr("No region selected. Click the planet surface."))
                            .size(10.5)
                            .color(muted()),
                    );
                }
                ui.separator();
                ui.label(egui::RichText::new(tr("RECENT EVENTS")).size(11.0).strong().color(accent()));
                let mut events: Vec<_> = sim.active_history().events.iter().rev().take(6).cloned().collect();
                events.reverse();
                if events.is_empty() {
                    ui.label(egui::RichText::new(tr("No events yet.")).size(10.5).color(muted()));
                }
                for event in events {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {}",
                            format_age(event.time_years),
                            translated(&event.title)
                        ))
                        .size(10.0)
                        .color(if matches!(event.kind, EventKind::Catastrophe(_)) {
                            egui::Color32::from_rgb(240, 150, 90)
                        } else {
                            muted()
                        }),
                    );
                }
            });
        });

    if let Some(action) = local_action {
        let outcome = match action {
            LocalAction::Warm(cell, delta) => {
                sim.active_planet_mut()
                    .adjust_local_temperature(cell, delta);
                Some(format!(
                    "{} {delta:+.1} °C.",
                    pick(
                        "Local temperature offset changed by",
                        "Локальное смещение температуры изменено на"
                    )
                ))
            }
            LocalAction::Rain(cell, mm) => {
                sim.active_planet_mut().add_local_rain(cell, mm);
                Some(format!(
                    "{} {mm:.0} {}.",
                    pick("Added", "Добавлено"),
                    pick("mm of local rainfall", "мм локальных осадков")
                ))
            }
            LocalAction::Drought(cell) => {
                sim.active_planet_mut().add_local_drought(cell, 0.8);
                Some(pick("Applied a local drought.", "Применена локальная засуха.").to_string())
            }
            LocalAction::Lake(cell, depth) => {
                sim.active_planet_mut().add_local_water(cell, depth);
                Some(format!(
                    "{} {depth:.1} {}.",
                    pick(
                        "Local lake depth increased by",
                        "Глубина локального озера увеличена на"
                    ),
                    pick("m", "м")
                ))
            }
            LocalAction::Volcano(cell) => {
                let outcome = sim.active_planet_mut().trigger_volcano(cell, 6.0, 20.0);
                sim.log_active_event(
                    EventKind::Catastrophe(outcome.kind),
                    outcome.title.clone(),
                    outcome.description.clone(),
                );
                Some(outcome.description)
            }
            LocalAction::ResetForcing(cell) => {
                if let Some(c) = sim.active_planet_mut().grid.cells.get_mut(cell) {
                    c.temp_offset_c = 0.0;
                    c.transient_heat_c = 0.0;
                }
                sim.active_planet_mut().revision += 1;
                Some(pick("Local forcing cleared.", "Локальное воздействие сброшено.").to_string())
            }
        };
        if let Some(message) = outcome {
            state.status = Some((message, false));
        }
    }
}

enum LocalAction {
    Warm(usize, f32),
    Rain(usize, f32),
    Drought(usize),
    Lake(usize, f32),
    Volcano(usize),
    ResetForcing(usize),
}

fn stat(ui: &mut egui::Ui, label: &str, value: String) {
    ui.label(
        egui::RichText::new(translated(label))
            .size(10.5)
            .color(muted()),
    );
    ui.label(egui::RichText::new(value).size(11.5).monospace());
    ui.end_row();
}

fn region(ui: &mut egui::Ui, label: &str, value: String) {
    ui.label(
        egui::RichText::new(translated(label))
            .size(10.5)
            .color(muted()),
    );
    ui.label(egui::RichText::new(value).size(11.0));
    ui.end_row();
}
