//! Bevy application wiring and shared view resources.

use bevy::prelude::*;
use bevy_egui::EguiPlugin;

use crate::rendering::{MapLayer, RenderingPlugin};
use crate::simulation::Simulation;
use crate::ui::UiPlugin;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    MainMenu,
    Creation,
    Simulation,
}

#[derive(Resource)]
pub struct ViewState {
    pub screen: Screen,
    pub layer: MapLayer,
    pub selected_cell: Option<usize>,
    /// Vertical exaggeration for the render mesh only; simulation elevations
    /// are always stored in real metres.
    pub elevation_exaggeration: f32,
    pub show_rivers: bool,
    pub show_clouds: bool,
    pub show_atmosphere: bool,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            screen: Screen::MainMenu,
            layer: MapLayer::Natural,
            selected_cell: None,
            elevation_exaggeration: 22.0,
            show_rivers: true,
            show_clouds: true,
            show_atmosphere: true,
        }
    }
}

#[derive(Resource, Default)]
pub struct SimulationRes(pub Option<Simulation>);

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimulationSet;

pub struct PlanetAppPlugin;

impl Plugin for PlanetAppPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .init_resource::<ViewState>()
            .init_resource::<SimulationRes>()
            .add_systems(Startup, demo_start)
            .add_systems(Update, advance_simulation.in_set(SimulationSet))
            .add_plugins((RenderingPlugin, UiPlugin));
    }
}

/// Optional development/demo startup: `PLANET_DEMO=1 cargo run`.
/// It uses the same creation pipeline as the UI, not a hard-coded scene.
fn demo_start(
    mut simulation: ResMut<SimulationRes>,
    mut view: ResMut<ViewState>,
    mut visual: ResMut<crate::rendering::PlanetVisual>,
) {
    if std::env::var("PLANET_SCREEN").as_deref() == Ok("creation") {
        view.screen = Screen::Creation;
    }
    if std::env::var("PLANET_DEMO").is_err() {
        return;
    }
    let mut config = crate::planet_generator::PlanetConfig::default();
    config.name = "Demo Terra".into();
    match Simulation::new(config) {
        Ok(sim) => {
            simulation.0 = Some(sim);
            view.screen = Screen::Simulation;
            visual.force_rebuild = true;
        }
        Err(err) => {
            bevy::log::error!("Demo planet generation failed: {err}");
        }
    }
}

fn advance_simulation(
    time: Res<Time>,
    view: Res<ViewState>,
    mut simulation: ResMut<SimulationRes>,
) {
    if view.screen != Screen::Simulation {
        return;
    }
    if let Some(sim) = simulation.0.as_mut() {
        sim.advance_realtime(time.delta_secs_f64());
    }
}
