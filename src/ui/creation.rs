use bevy_egui::egui;

use crate::app::{Screen, ViewState};
use crate::localization::{pick, tr, translated};
use crate::planet_generator::noise::SplitMix64;
use crate::planet_generator::{PlanetConfig, all_presets, preset_by_name};
use crate::ui::{UiState, accent, danger, muted};

pub fn draw_creation(root: &mut egui::Ui, state: &mut UiState, view: &mut ViewState) {
    egui::Panel::top("creation_top").show(root, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button(tr("← Main menu")).clicked() {
                view.screen = Screen::MainMenu;
            }
            ui.separator();
            ui.label(
                egui::RichText::new(tr("PLANET CREATION"))
                    .size(18.0)
                    .strong()
                    .color(accent()),
            );
            ui.separator();
            ui.label(format!(
                "{} {:.1} {} · {} {:.2} m/s²",
                pick("Surface area", "Площадь поверхности"),
                state.creation.surface_area_km2() / 1.0e6,
                pick("million km²", "млн км²"),
                pick("gravity", "гравитация"),
                state.creation.gravity
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(tr("Generate planet")).strong(),
                    ))
                    .clicked()
                {
                    match state.creation.validate() {
                        Ok(()) => {
                            state.pending_generate = Some(state.creation.clone());
                            state.status = Some(("Generating planet…".into(), false));
                        }
                        Err(err) => state.status = Some((err, true)),
                    }
                }
            });
        });
        if let Some((message, is_error)) = &state.status {
            ui.label(
                egui::RichText::new(translated(message))
                    .size(11.0)
                    .color(if *is_error { danger() } else { accent() }),
            );
        }
    });

    egui::CentralPanel::default().show(root, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(tr("Preset"));
                let mut selected = state.creation_preset.clone();
                egui::ComboBox::from_id_salt("preset_combo")
                    .selected_text(translated(&selected))
                    .show_ui(ui, |ui| {
                        for (name, _) in all_presets() {
                            ui.selectable_value(&mut selected, name.to_string(), tr(name));
                        }
                        ui.selectable_value(&mut selected, "Custom".into(), tr("Custom"));
                    });
                if selected != state.creation_preset {
                    state.creation_preset = selected.clone();
                    if let Some(mut preset) = preset_by_name(&selected) {
                        preset.name = state.creation.name.clone();
                        preset.seed = state.creation.seed;
                        state.creation = preset;
                    }
                }
                if ui.button(tr("New random seed")).clicked() {
                    let mut rng = SplitMix64::new(
                        state.creation.seed ^ 0x9e37_79b9_7f4a_7c15,
                    );
                    state.creation.seed = rng.next_u64();
                    state.creation_preset = "Custom".into();
                }
                if ui.button(tr("Randomise all")).clicked() {
                    state.creation = random_config(state.creation.seed);
                    state.creation_preset = "Custom".into();
                }
            });
            ui.label(
                egui::RichText::new(
                    pick("The same seed and parameters always produce the same initial planet. Total water inventory is authoritative; the ocean-depth target is reconciled after generation.",
                         "Один и тот же seed и параметры всегда дают одну и ту же начальную планету. Общий запас воды является основным; целевая средняя глубина океана согласуется после генерации."),
                )
                .size(11.0)
                .color(muted()),
            );
            ui.separator();

            ui.columns(2, |cols| {
                basic_column(&mut cols[0], state);
                water_column(&mut cols[1], state);
            });
            ui.separator();
            ui.columns(2, |cols| {
                atmosphere_column(&mut cols[0], state);
                geology_column(&mut cols[1], state);
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .add_sized([220.0, 34.0], egui::Button::new(tr("Generate this planet")))
                    .clicked()
                {
                    match state.creation.validate() {
                        Ok(()) => {
                            state.pending_generate = Some(state.creation.clone());
                            state.status = Some(("Generating planet…".into(), false));
                        }
                        Err(err) => state.status = Some((err, true)),
                    }
                }
                if ui.button(tr("Reset to Earthlike")).clicked() {
                    state.creation = PlanetConfig::default();
                    state.creation_preset = "Earthlike".into();
                }
                ui.label(
                    egui::RichText::new(
                        pick("Generation uses fractal 3-D noise, continent masks, plate-boundary relief and a calibrated sea level. No textures or pre-made maps are used.",
                             "Генерация использует фрактальный 3D-шум, маски континентов, рельеф границ плит и калиброванный уровень моря. Текстуры и готовые карты не используются."),
                    )
                    .size(10.5)
                    .color(muted()),
                );
            });
        });
    });
}

fn basic_column(ui: &mut egui::Ui, state: &mut UiState) {
    let c = &mut state.creation;
    ui.heading(tr("Planet"));
    ui.horizontal(|ui| {
        ui.label(tr("Name"));
        ui.text_edit_singleline(&mut c.name);
    });
    ui.horizontal(|ui| {
        ui.label(tr("Seed"));
        ui.add(egui::DragValue::new(&mut c.seed).speed(1.0));
    });
    ui.horizontal(|ui| {
        ui.label(tr("Grid resolution"));
        ui.add(egui::Slider::new(&mut c.grid_level, 4..=7).text(tr("icosphere level")));
    });
    ui.label(
        egui::RichText::new(format!(
            "{} {} · {:.0} {}",
            10 * 4usize.pow(c.grid_level as u32 - 1) + 2,
            pick("cells", "ячеек"),
            c.radius_km,
            pick("km radius", "км радиус")
        ))
        .size(10.5)
        .color(muted()),
    );
    ui.add_space(8.0);
    ui.heading(tr("Bulk properties"));
    ui.add(egui::Slider::new(&mut c.radius_km, 1_000.0..=20_000.0).text(tr("Radius (km)")));
    ui.add(
        egui::Slider::new(&mut c.mass_earth, 0.05..=10.0)
            .logarithmic(true)
            .text(tr("Mass (Earth masses)")),
    );
    ui.add(egui::Slider::new(&mut c.gravity, 0.5..=30.0).text(tr("Surface gravity (m/s²)")));
    ui.add(
        egui::Slider::new(&mut c.rotation_period_hours, 4.0..=200.0)
            .text(tr("Rotation period (h)")),
    );
    ui.add(egui::Slider::new(&mut c.axial_tilt_deg, 0.0..=70.0).text(tr("Axial tilt (°)")));
    ui.add(egui::Slider::new(&mut c.albedo, 0.05..=0.8).text(tr("Initial albedo")));

    ui.add_space(8.0);
    ui.heading(tr("Energy balance"));
    ui.add(
        egui::Slider::new(&mut c.stellar_flux_wm2, 200.0..=3_000.0).text(tr("Stellar flux (W/m²)")),
    );
    ui.add(
        egui::Slider::new(&mut c.mean_surface_temp_c, -100.0..=80.0)
            .text(tr("Initial mean temperature (°C)")),
    );
    ui.add(
        egui::Slider::new(&mut c.ocean_temp_c, -60.0..=60.0)
            .text(tr("Initial ocean temperature (°C)")),
    );
    ui.add(
        egui::Slider::new(&mut c.surface_pressure_kpa, 0.1..=500.0)
            .logarithmic(true)
            .text(tr("Surface pressure (kPa)")),
    );
    ui.add(egui::Slider::new(&mut c.cloud_coverage, 0.0..=1.0).text(tr("Cloud coverage")));
    ui.add(
        egui::Slider::new(&mut c.ocean_heat_capacity, 0.1..=4.0)
            .text(tr("Ocean heat capacity (relative)")),
    );
}

fn water_column(ui: &mut egui::Ui, state: &mut UiState) {
    let c = &mut state.creation;
    ui.heading(tr("Water and land"));
    ui.add(
        egui::Slider::new(&mut c.water.inventory_km3, 1.0e5..=3.0e9)
            .logarithmic(true)
            .text(tr("Total water inventory (km³)")),
    );
    let mut coverage = c.water.surface_water_fraction * 100.0;
    if ui
        .add(egui::Slider::new(&mut coverage, 0.0..=100.0).text(tr("Surface water coverage (%)")))
        .changed()
    {
        c.water.surface_water_fraction = coverage / 100.0;
    }
    ui.add(
        egui::Slider::new(&mut c.water.avg_ocean_depth_m, 50.0..=12_000.0)
            .text(tr("Target mean ocean depth (m)")),
    );
    ui.add(
        egui::Slider::new(&mut c.water.max_ocean_depth_m, 100.0..=25_000.0)
            .text(tr("Target maximum depth (m)")),
    );
    ui.add(
        egui::Slider::new(&mut c.water.avg_land_elevation_m, 10.0..=3_000.0)
            .text(tr("Mean land elevation (m)")),
    );
    ui.add(egui::Slider::new(&mut c.water.continents, 1..=16).text(tr("Continents")));
    ui.add(egui::Slider::new(&mut c.water.islands, 0..=120).text(tr("Islands")));
    let mut glacier = c.initial_glacier_fraction * 100.0;
    if ui
        .add(egui::Slider::new(&mut glacier, 0.0..=100.0).text(tr("Initial ice coverage (%)")))
        .changed()
    {
        c.initial_glacier_fraction = glacier / 100.0;
    }
    ui.label(
        egui::RichText::new(format!(
            "{} {:.2e} {} · {} {:+.0}%",
            pick("Water budget target:", "Целевой водный бюджет:"),
            c.water.inventory_km3,
            pick("km³", "км³"),
            pick("land", "суша"),
            (1.0 - c.water.surface_water_fraction) * 100.0
        ))
        .size(10.5)
        .color(muted()),
    );
    ui.add_space(8.0);
    ui.heading(tr("Initial ocean state"));
    ui.add(
        egui::Slider::new(&mut c.initial_glacier_fraction, 0.0..=0.95)
            .text(tr("Initial glacier fraction")),
    );
    ui.label(
        egui::RichText::new(
            tr("Land/water percentages are calibrated after terrain generation; the displayed values in the simulation are the produced values."),
        )
        .size(10.5)
        .color(muted()),
    );
}

fn atmosphere_column(ui: &mut egui::Ui, state: &mut UiState) {
    let c = &mut state.creation;
    ui.heading(tr("Atmospheric composition"));
    ui.add(
        egui::Slider::new(&mut c.co2_ppm, 0.1..=5_000_000.0)
            .logarithmic(true)
            .text(tr("CO₂ (ppm)")),
    );
    ui.add(
        egui::Slider::new(&mut c.ch4_ppm, 0.0..=10_000.0)
            .logarithmic(true)
            .text(tr("CH₄ (ppm)")),
    );
    ui.add(egui::Slider::new(&mut c.o2_fraction, 0.0..=0.6).text(tr("O₂ fraction")));
    ui.add(egui::Slider::new(&mut c.n2_fraction, 0.0..=1.0).text(tr("N₂ fraction")));
    ui.add(egui::Slider::new(&mut c.h2o_fraction, 0.0..=0.1).text(tr("H₂O vapour fraction")));
    let (o2, n2, h2o) = c.normalised_composition();
    ui.label(
        egui::RichText::new(format!(
            "Normalised: O₂ {:.1}% · N₂ {:.1}% · H₂O vapour {:.2}%",
            o2 * 100.0,
            n2 * 100.0,
            h2o * 100.0
        ))
        .size(10.5)
        .color(muted()),
    );
    ui.add_space(8.0);
    ui.heading(tr("Climate model notes"));
    ui.label(
        egui::RichText::new(
            pick("CO₂ forcing is logarithmic (5.35·ln(C/C₀) W/m²) with a fast water-vapour feedback. Clouds and ice affect albedo. Seasonal contrast is resolved for steps shorter than two years.",
                 "Форсинг CO₂ логарифмический (5.35·ln(C/C₀) Вт/м²) с быстрой обратной связью водяного пара. Облака и лёд влияют на альбедо. Сезонный контраст разрешается при шаге меньше двух лет."),
        )
        .size(10.5)
        .color(muted()),
    );
}

fn geology_column(ui: &mut egui::Ui, state: &mut UiState) {
    let c = &mut state.creation;
    ui.heading(tr("Tectonics and events"));
    ui.add(egui::Slider::new(&mut c.plate_count, 2..=24).text(tr("Tectonic plates")));
    ui.add(
        egui::Slider::new(&mut c.weather_activity, 0.0..=3.0)
            .text(tr("Weather activity multiplier")),
    );
    ui.checkbox(&mut c.random_events, tr("Random catastrophes enabled"));
    ui.add_space(8.0);
    ui.heading(tr("What will be simulated"));
    ui.label(
        egui::RichText::new(pick(
            "• Surface energy balance and regional temperatures\n\
             • Evaporation, precipitation, soil water, runoff and routed rivers\n\
             • Snow, glaciers, sea ice and sea level from the water budget\n\
             • Plate motion, boundary uplift, trenches and erosion\n\
             • Volcanoes, impacts, aerosol/dust forcing and carbon exchange\n\
             • Weather events, biomes, statistics and checkpoint history",
            "• Энергобаланс поверхности и региональные температуры\n\
             • Испарение, осадки, почвенная влага, сток и маршрутизация рек\n\
             • Снег, ледники, морской лёд и уровень моря из водного баланса\n\
             • Движение плит, поднятия границ, желоба и эрозия\n\
             • Вулканы, удары, аэрозольная/пылевая нагрузка и углеродный обмен\n\
             • Погодные события, биомы, статистика и история контрольных точек",
        ))
        .size(11.0)
        .color(muted()),
    );
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(tr(
            "Not implemented: full 3-D atmospheric/ocean circulation, chemistry, biology, \
             magnetic dynamo physics, sub-grid cloud microphysics.",
        ))
        .size(10.5)
        .color(egui::Color32::from_rgb(180, 140, 90)),
    );
}

fn random_config(seed: u64) -> PlanetConfig {
    let mut rng = SplitMix64::new(seed ^ 0xC0FF_EE12_3456_789A);
    let mut c = PlanetConfig::default();
    c.name = format!("Kepler-{}", rng.range_u32(100, 999));
    c.seed = seed;
    c.radius_km = rng.range(3_000.0, 9_000.0);
    c.mass_earth = (c.radius_km / 6_371.0).powi(2) * rng.range(0.7, 1.4);
    c.gravity = 9.81 * c.mass_earth / (c.radius_km / 6_371.0).powi(2);
    c.stellar_flux_wm2 = rng.range(700.0, 2_000.0);
    c.mean_surface_temp_c = rng.range(-25.0, 35.0);
    c.ocean_temp_c = c.mean_surface_temp_c + rng.range(-3.0, 4.0);
    c.surface_pressure_kpa = rng.range(30.0, 220.0);
    c.co2_ppm = 10.0f32.powf(rng.range(1.4, 3.4));
    c.o2_fraction = rng.range(0.01, 0.28);
    c.n2_fraction = 1.0 - c.o2_fraction - 0.01;
    c.h2o_fraction = rng.range(0.002, 0.03);
    c.axial_tilt_deg = rng.range(0.0, 55.0);
    c.cloud_coverage = rng.range(0.1, 0.85);
    c.water.inventory_km3 = 10.0f64.powf(rng.range(6.0f32, 9.4) as f64);
    c.water.surface_water_fraction = rng.range(0.1, 0.92);
    c.water.continents = rng.range_u32(2, 10);
    c.water.islands = rng.range_u32(3, 50);
    c.initial_glacier_fraction = if c.mean_surface_temp_c < 0.0 {
        rng.range(0.2, 0.7)
    } else {
        rng.range(0.0, 0.15)
    };
    c.weather_activity = rng.range(0.4, 1.8);
    c
}
