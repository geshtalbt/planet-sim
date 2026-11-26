use bevy::prelude::*;
use planet_simulator::app::PlanetAppPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Planet Simulator — coupled planetary evolution laboratory".into(),
                resolution: (1440, 900).into(),
                present_mode: bevy::window::PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(PlanetAppPlugin)
        .run();
}
