//! Procedural planet generation: configuration, presets, noise and the
//! generation pipeline that fills a fresh [`crate::terrain::PlanetGrid`].

pub mod config;
pub mod noise;

mod generator;

pub use config::presets::{all_presets, preset_by_name, preset_names};
pub use config::{PlanetConfig, WaterConfig};
pub use generator::generate_planet;
