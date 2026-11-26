//! egui user interface: menu, creation, simulation layout and science panels.

mod creation;
mod menu;
mod panels;
mod simulation_ui;
pub mod widgets;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use crate::app::{Screen, SimulationRes, ViewState};
use crate::localization::{self, Language, tr};
use crate::planet_generator::PlanetConfig;
use crate::rendering::PlanetVisual;
use crate::save_system::{SaveManager, SaveMeta};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartMetric {
    Temperature,
    OceanTemperature,
    Co2,
    SeaLevel,
    Ice,
    Land,
    Precipitation,
    Pressure,
    Albedo,
    Solar,
    AxialTilt,
    Water,
    Eruptions,
    MagneticField,
}

impl ChartMetric {
    pub const ALL: [Self; 14] = [
        Self::Temperature,
        Self::OceanTemperature,
        Self::Co2,
        Self::SeaLevel,
        Self::Ice,
        Self::Land,
        Self::Precipitation,
        Self::Pressure,
        Self::Albedo,
        Self::Solar,
        Self::AxialTilt,
        Self::Water,
        Self::Eruptions,
        Self::MagneticField,
    ];

    pub fn label(self) -> &'static str {
        tr(match self {
            Self::Temperature => "Global mean temperature",
            Self::OceanTemperature => "Ocean temperature",
            Self::Co2 => "Atmospheric CO₂",
            Self::SeaLevel => "Sea level",
            Self::Ice => "Ice coverage",
            Self::Land => "Land fraction",
            Self::Precipitation => "Mean precipitation",
            Self::Pressure => "Surface pressure",
            Self::Albedo => "Mean albedo",
            Self::Solar => "Stellar flux multiplier",
            Self::AxialTilt => "Axial tilt",
            Self::Water => "Total water inventory",
            Self::Eruptions => "Active eruptions",
            Self::MagneticField => "Magnetic field",
        })
    }

    pub fn unit(self) -> &'static str {
        match self {
            Self::Temperature | Self::OceanTemperature => "°C",
            Self::Co2 => "ppm",
            Self::SeaLevel => "m",
            Self::Ice | Self::Land | Self::Albedo => tr("fraction"),
            Self::Precipitation => "mm/yr",
            Self::Pressure => "kPa",
            Self::Solar => "×",
            Self::AxialTilt => "°",
            Self::Water => "km³",
            Self::Eruptions => tr("count"),
            Self::MagneticField => tr("relative"),
        }
    }

    pub fn value(self, sample: &crate::history::HistorySample) -> f64 {
        (match self {
            Self::Temperature => sample.mean_temp_c,
            Self::OceanTemperature => sample.mean_ocean_temp_c,
            Self::Co2 => sample.co2_ppm,
            Self::SeaLevel => sample.sea_level_m,
            Self::Ice => sample.glacier_fraction,
            Self::Land => sample.land_fraction,
            Self::Precipitation => sample.precip_mm_yr,
            Self::Pressure => sample.pressure_kpa,
            Self::Albedo => sample.albedo,
            Self::Solar => sample.solar_multiplier,
            Self::AxialTilt => sample.axial_tilt_deg,
            Self::Water => sample.water_inventory_km3 as f32,
            Self::Eruptions => sample.active_eruptions as f32,
            Self::MagneticField => sample.magnetic_field,
        }) as f64
    }

    pub fn color(self) -> egui::Color32 {
        match self {
            Self::Temperature => egui::Color32::from_rgb(255, 130, 80),
            Self::OceanTemperature => egui::Color32::from_rgb(90, 170, 255),
            Self::Co2 => egui::Color32::from_rgb(255, 200, 70),
            Self::SeaLevel => egui::Color32::from_rgb(70, 210, 210),
            Self::Ice => egui::Color32::from_rgb(210, 235, 255),
            Self::Land => egui::Color32::from_rgb(150, 210, 120),
            Self::Precipitation => egui::Color32::from_rgb(90, 160, 255),
            Self::Pressure => egui::Color32::from_rgb(190, 160, 255),
            Self::Albedo => egui::Color32::from_rgb(220, 220, 220),
            Self::Solar => egui::Color32::from_rgb(255, 220, 120),
            Self::AxialTilt => egui::Color32::from_rgb(255, 150, 200),
            Self::Water => egui::Color32::from_rgb(80, 180, 240),
            Self::Eruptions => egui::Color32::from_rgb(255, 90, 40),
            Self::MagneticField => egui::Color32::from_rgb(120, 255, 180),
        }
    }
}

#[derive(Resource)]
pub struct UiState {
    pub language: Language,
    pub creation: PlanetConfig,
    pub creation_preset: String,
    pub status: Option<(String, bool)>,
    pub show_physics: bool,
    pub show_statistics: bool,
    pub show_catastrophes: bool,
    pub show_history: bool,
    pub show_saves: bool,
    pub show_experiments: bool,
    pub show_settings: bool,
    pub show_help: bool,
    pub confirm_restart: bool,

    pub co2_edit: f32,
    pub co2_redistribute: bool,
    pub water_edit: f64,

    pub weather_kind: crate::weather::WeatherKind,
    pub weather_intensity: f32,
    pub weather_days: f32,
    pub event_at_selected: bool,
    pub volcano_intensity: f32,
    pub volcano_duration: f32,
    pub asteroid_diameter: f32,
    pub asteroid_velocity: f32,
    pub asteroid_angle: f32,
    pub ice_age_duration: f32,
    pub warming_ppm: f32,
    pub cooling_tau: f32,

    pub chart_metric: ChartMetric,
    pub chart_fraction: f32,

    pub save_manager: SaveManager,
    pub saves: Vec<SaveMeta>,
    pub save_name: String,
    pub selected_save: Option<String>,
    pub confirm_delete: Option<String>,
    pub pending_load: Option<String>,
    pub pending_generate: Option<PlanetConfig>,

    pub fork_name: String,
    pub compare_fraction: f32,

    pub step_amount: f32,
    pub step_unit_is_years: bool,

    pub physics_was_open_last: bool,
    pub theme_initialized: bool,
}

impl Default for UiState {
    fn default() -> Self {
        let demo_ui = std::env::var("PLANET_UI").unwrap_or_default();
        Self {
            language: Language::detect(),
            creation: PlanetConfig::default(),
            creation_preset: "Earthlike".into(),
            status: None,
            show_physics: demo_ui == "physics",
            show_statistics: demo_ui == "statistics",
            show_catastrophes: demo_ui == "catastrophes",
            show_history: demo_ui == "history",
            show_saves: demo_ui == "saves",
            show_experiments: demo_ui == "experiments",
            show_settings: demo_ui == "settings",
            show_help: demo_ui == "help",
            confirm_restart: false,
            co2_edit: 420.0,
            co2_redistribute: true,
            water_edit: 1.386e9,
            weather_kind: crate::weather::WeatherKind::Rain,
            weather_intensity: 0.7,
            weather_days: 3.0,
            event_at_selected: true,
            volcano_intensity: 5.0,
            volcano_duration: 30.0,
            asteroid_diameter: 2.0,
            asteroid_velocity: 20.0,
            asteroid_angle: 45.0,
            ice_age_duration: 100_000.0,
            warming_ppm: 500.0,
            cooling_tau: 0.5,
            chart_metric: ChartMetric::Temperature,
            chart_fraction: 1.0,
            save_manager: SaveManager::default(),
            saves: Vec::new(),
            save_name: String::new(),
            selected_save: None,
            confirm_delete: None,
            pending_load: None,
            pending_generate: None,
            fork_name: String::new(),
            compare_fraction: 1.0,
            step_amount: 100.0,
            step_unit_is_years: true,
            physics_was_open_last: false,
            theme_initialized: false,
        }
    }
}

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiState>()
            .add_systems(EguiPrimaryContextPass, draw_ui);
    }
}

fn draw_ui(
    mut contexts: EguiContexts,
    mut state: ResMut<UiState>,
    mut view: ResMut<ViewState>,
    mut simulation: ResMut<SimulationRes>,
    mut visual: ResMut<PlanetVisual>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut app_exit: MessageWriter<AppExit>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    localization::set_language(state.language);
    if !state.theme_initialized {
        apply_theme(ctx);
        state.theme_initialized = true;
    }

    if let Some(config) = state.pending_generate.take() {
        match crate::simulation::Simulation::new(config) {
            Ok(sim) => {
                state.save_name = sim.main.planet.config.name.clone();
                state.co2_edit = sim.main.planet.atmosphere.co2_ppm;
                state.water_edit = sim.main.planet.hydrology.total_water_km3;
                state.status = Some(("Planet generated from seed.".into(), false));
                simulation.0 = Some(sim);
                view.screen = Screen::Simulation;
                view.selected_cell = None;
                visual.force_rebuild = true;
            }
            Err(err) => state.status = Some((err, true)),
        }
    }
    if let Some(name) = state.pending_load.take() {
        match state.save_manager.load(&name) {
            Ok(sim) => {
                state.save_name = sim.main.planet.config.name.clone();
                state.co2_edit = sim.main.planet.atmosphere.co2_ppm;
                state.water_edit = sim.main.planet.hydrology.total_water_km3;
                state.status = Some((
                    format!(
                        "{} '{name}'.",
                        crate::localization::pick("Loaded", "Загружено")
                    ),
                    false,
                ));
                simulation.0 = Some(sim);
                view.screen = Screen::Simulation;
                view.selected_cell = None;
                visual.force_rebuild = true;
            }
            Err(err) => state.status = Some((format!("Load failed: {err}"), true)),
        }
    }

    if !ctx.egui_wants_keyboard_input()
        && let Some(sim) = simulation.0.as_mut()
    {
        if keyboard.just_pressed(KeyCode::Space) {
            sim.time.paused = !sim.time.paused;
        }
        if keyboard.just_pressed(KeyCode::ArrowRight) {
            sim.time.faster();
        }
        if keyboard.just_pressed(KeyCode::ArrowLeft) {
            sim.time.slower();
        }
        if keyboard.just_pressed(KeyCode::Period) {
            let step = 1.0;
            sim.time.queue_years(step);
        }
    }

    let mut root = egui::Ui::new(
        ctx.clone(),
        "root".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );

    match view.screen {
        Screen::MainMenu => menu::draw_main_menu(
            &mut root,
            ctx,
            &mut state,
            &mut view,
            simulation.0.as_ref(),
            &mut app_exit,
        ),
        Screen::Creation => creation::draw_creation(&mut root, &mut state, &mut view),
        Screen::Simulation => {
            if simulation.0.is_none() {
                draw_missing_simulation(&mut root, &mut state, &mut view);
            } else {
                simulation_ui::draw_simulation(
                    &mut root,
                    ctx,
                    &mut state,
                    &mut view,
                    simulation.0.as_mut().expect("checked"),
                    &mut app_exit,
                );
            }
        }
    }

    // Modal-ish windows live above the active screen.
    panels::draw_windows(
        ctx,
        &mut state,
        &mut view,
        simulation.0.as_mut(),
        &mut visual,
    );
    Ok(())
}

fn draw_missing_simulation(root: &mut egui::Ui, state: &mut UiState, view: &mut ViewState) {
    egui::CentralPanel::default().show(root, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(100.0);
            ui.heading(tr("No simulation loaded"));
            ui.label(tr("The selected screen requires an active planet."));
            if ui.button(tr("Back to main menu")).clicked() {
                view.screen = Screen::MainMenu;
                state.status = None;
            }
        });
    });
}

fn apply_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgba_unmultiplied(9, 13, 20, 238);
    visuals.window_fill = egui::Color32::from_rgba_unmultiplied(12, 17, 26, 246);
    visuals.extreme_bg_color = egui::Color32::from_rgb(5, 8, 13);
    visuals.selection.bg_fill = egui::Color32::from_rgb(34, 122, 135);
    visuals.hyperlink_color = egui::Color32::from_rgb(90, 200, 210);
    visuals.weak_text_color = Some(egui::Color32::from_rgb(130, 148, 164));
    ctx.set_visuals(visuals);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(20.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::new(11.0, egui::FontFamily::Proportional),
        );
    });
}

pub fn accent() -> egui::Color32 {
    egui::Color32::from_rgb(72, 196, 200)
}

pub fn warning() -> egui::Color32 {
    egui::Color32::from_rgb(245, 166, 66)
}

pub fn danger() -> egui::Color32 {
    egui::Color32::from_rgb(235, 92, 80)
}

pub fn muted() -> egui::Color32 {
    egui::Color32::from_rgb(132, 148, 166)
}

pub fn format_compact(value: f64, unit: &str) -> String {
    let v = value.abs();
    if unit == "km³" || unit == "ppm" {
        if v >= 1.0e9 {
            format!("{:.2}G {unit}", value / 1.0e9)
        } else if v >= 1.0e6 {
            format!("{:.2}M {unit}", value / 1.0e6)
        } else if v >= 1.0e3 {
            format!("{:.2}k {unit}", value / 1.0e3)
        } else {
            format!("{value:.1} {unit}")
        }
    } else {
        format!("{value:.1} {unit}")
    }
}

pub fn refresh_saves(state: &mut UiState) {
    state.saves = state.save_manager.list();
}
