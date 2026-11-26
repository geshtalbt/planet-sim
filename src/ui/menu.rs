use bevy::prelude::*;
use bevy_egui::egui;

use crate::app::{Screen, ViewState};
use crate::localization::tr;
use crate::localization::{pick, translated};
use crate::simulation::Simulation;
use crate::ui::{UiState, accent, muted, refresh_saves};

pub fn draw_main_menu(
    root: &mut egui::Ui,
    ctx: &egui::Context,
    state: &mut UiState,
    view: &mut ViewState,
    simulation: Option<&Simulation>,
    app_exit: &mut MessageWriter<AppExit>,
) {
    egui::CentralPanel::default().show(root, |ui| {
        let rect = ui.available_rect_before_wrap();
        let center = rect.center();
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_center_size(center, egui::vec2(520.0, 520.0))),
            |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(32.0);
                    ui.label(
                        egui::RichText::new(tr("PLANET SIMULATOR"))
                            .size(42.0)
                            .color(egui::Color32::from_rgb(220, 240, 244))
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(tr("A coupled planetary evolution laboratory"))
                            .size(15.0)
                            .color(accent()),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(
                            tr("Create a world, then watch its atmosphere, oceans, ice, climate and geology evolve together."),
                        )
                        .size(12.0)
                        .color(muted()),
                    );
                    ui.add_space(36.0);
                    let width = 280.0;
                    if simulation.is_some()
                        && ui
                            .add_sized([width, 36.0], egui::Button::new(tr("Resume simulation")))
                            .clicked()
                    {
                        view.screen = Screen::Simulation;
                    }
                    if ui
                        .add_sized([width, 36.0], egui::Button::new(tr("New planet")))
                        .clicked()
                    {
                        state.creation = crate::planet_generator::PlanetConfig::default();
                        state.creation_preset = "Earthlike".into();
                        state.status = None;
                        view.screen = Screen::Creation;
                    }
                    if ui
                        .add_sized([width, 36.0], egui::Button::new(tr("Load saved planet")))
                        .clicked()
                    {
                        refresh_saves(state);
                        state.show_saves = true;
                    }
                    if ui
                        .add_sized([width, 36.0], egui::Button::new(tr("Application settings")))
                        .clicked()
                    {
                        state.show_settings = true;
                    }
                    if ui
                        .add_sized([width, 36.0], egui::Button::new(tr("How the model works")))
                        .clicked()
                    {
                        state.show_help = true;
                    }
                    if ui
                        .add_sized([width, 36.0], egui::Button::new(tr("Quit")))
                        .clicked()
                    {
                        app_exit.write(AppExit::Success);
                    }
                    ui.add_space(18.0);
                    ui.horizontal(|ui| {
                        ui.label("Language / Язык:");
                        let mut language = state.language;
                        if ui
                            .selectable_value(&mut language, crate::localization::Language::English, "English")
                            .clicked()
                            || ui
                                .selectable_value(&mut language, crate::localization::Language::Russian, "Русский")
                                .clicked()
                        {
                            state.language = language;
                            crate::localization::set_language(language);
                        }
                    });
                    ui.add_space(18.0);
                    if let Some((message, is_error)) = &state.status {
                        ui.label(
                            egui::RichText::new(translated(message))
                                .size(12.0)
                                .color(if *is_error {
                                    crate::ui::danger()
                                } else {
                                    accent()
                                }),
                        );
                    }
                    ui.add_space(18.0);
                    ui.label(
                        egui::RichText::new(
                            pick("Every displayed statistic is computed from the simulation state. Simplified physics is documented in docs/MODEL.md.",
                                 "Каждое показанное значение вычисляется из состояния симуляции. Упрощения физики описаны в docs/MODEL.md."),
                        )
                        .size(10.5)
                        .color(muted()),
                    );
                });
            },
        );
    });
    let _ = ctx;
}
