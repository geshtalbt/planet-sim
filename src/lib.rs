//! Planet Simulator core library.
//!
//! The crate is split into a Bevy-free simulation core and a Bevy application
//! layer. The simulation modules never depend on rendering or UI types, which
//! keeps the physical model testable and deterministic independently of the
//! game loop.
//!
//! See `docs/MODEL.md` for assumptions and simplifications, and
//! `docs/STATUS.md` for the implemented / planned feature matrix.

pub mod app;
pub mod atmosphere;
pub mod catastrophes;
pub mod climate;
pub mod experiments;
pub mod history;
pub mod hydrology;
pub mod planet_generator;
pub mod planetary_physics;
pub mod rendering;
pub mod save_system;
pub mod simulation;
pub mod statistics;
pub mod tectonics;
pub mod terrain;
pub mod time_engine;
pub mod ui;
pub mod weather;

pub use planet_generator::config::PlanetConfig;
pub use simulation::PlanetState;
