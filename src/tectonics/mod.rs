//! Simplified tectonic plate model.
//!
//! Plates are spherical caps defined by a seed centre. Each plate rotates
//! about an Euler pole, which moves its boundary across the surface. Cells
//! are assigned to the nearest plate centre. Boundary uplift, rifting and
//! trenches are computed from the relative velocity of neighbouring plates.
//!
//! This is a kinematic approximation, not a mantle-convection solver.

pub mod plate;

mod update;

pub use plate::Plate;
pub use update::{PlateStats, TectonicRates, recompute_plate_assignment, tectonic_step};
