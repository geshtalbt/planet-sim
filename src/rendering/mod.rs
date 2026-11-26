//! Bevy rendering: planet mesh, layer colouring, rivers, clouds, camera.

mod layers;
mod mesh;
mod scene;

pub use layers::MapLayer;
pub use scene::{OrbitCameraState, PlanetVisual, RenderingPlugin};

pub(crate) use mesh::surface_radius;
