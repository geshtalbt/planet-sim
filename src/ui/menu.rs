use bevy::prelude::*;
use bevy_egui::egui;

use crate::app::{Screen, ViewState};
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
                        egui::RichText::new("PLANET SIMULATOR")
                            .size(42.0)
                            .color(egui::Color32::from_rgb(220, 240, 244))
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("A coupled planetary evolution laboratory")
                            .size(15.0)
                            .color(accent()),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(
                            "Create a world, then watch its atmosphere, oceans, ice, climate and geology evolve together.",
                        )
                        .size(12.0)
                        .color(muted()),
                    );
                    ui.add_space(36.0);
                    let width = 280.0;
                    if simulation.is_some()
                        && ui
                            .add_sized([width, 36.0], egui::Button::new("Resume simulation"))
                            .clicked()
                    {
                        view.screen = Screen::Simulation;
                    }
                    if ui
                        .add_sized([width, 36.0], egui::Button::new("New planet"))
                        .clicked()
                    {
                        state.creation = crate::planet_generator::PlanetConfig::default();
                        state.creation_preset = "Earthlike".into();
                        state.status = None;
                        view.screen = Screen::Creation;
                    }
                    if ui
                        .add_sized([width, 36.0], egui::Button::new("Load saved planet"))
                        .clicked()
                    {
                        refresh_saves(state);
                        state.show_saves = true;
                    }
                    if ui
                        .add_sized([width, 36.0], egui::Button::new("Application settings"))
                        .clicked()
                    {
                        state.show_settings = true;
                    }
                    if ui
                        .add_sized([width, 36.0], egui::Button::new("How the model works"))
                        .clicked()
                    {
                        state.show_help = true;
                    }
                    if ui
                        .add_sized([width, 36.0], egui::Button::new("Quit"))
                        .clicked()
                    {
                        app_exit.write(AppExit::Success);
                    }
                    ui.add_space(28.0);
                    if let Some((message, is_error)) = &state.status {
                        ui.label(
                            egui::RichText::new(message)
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
                            "Every displayed statistic is computed from the simulation state.\n\
                             Simplified physics is documented in docs/MODEL.md.",
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
