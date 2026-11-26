//! Planet surface representation.
//!
//! The surface is an icosphere: every mesh vertex is a simulation cell. The
//! topology is rebuilt from the subdivision level on load and is never
//! serialized, which keeps save files small. Cells carry the full
//! environmental state that the physical modules read and write.

pub mod biome;
pub mod cell;
pub mod grid;
pub mod icosphere;

pub use biome::Biome;
pub use cell::SurfaceCell;
pub use grid::PlanetGrid;
pub use icosphere::{IcosphereTopology, build_topology};
