use serde::{Deserialize, Serialize};

use glam::Vec3;

use super::cell::SurfaceCell;
use super::icosphere::{IcosphereTopology, build_topology};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanetGrid {
    pub level: u8,
    pub radius_km: f32,
    pub cells: Vec<SurfaceCell>,
    #[serde(skip)]
    pub topology: IcosphereTopology,
    #[serde(skip)]
    pub cell_area_m2: Vec<f32>,
}

impl PlanetGrid {
    pub fn new(level: u8, cells: Vec<SurfaceCell>) -> Self {
        let topology = build_topology(level);
        assert_eq!(topology.cell_count(), cells.len());
        let cell_area_m2 = topology.unit_areas.iter().map(|a| *a).collect::<Vec<_>>();
        Self {
            level,
            radius_km: 6_371.0,
            cells,
            topology,
            cell_area_m2,
        }
    }

    /// Rebuilds topology and cell areas after deserialization.
    pub fn ensure_topology(&mut self, radius_m: f32) {
        if self.topology.cell_count() != self.cells.len() {
            self.topology = build_topology(self.level);
        }
        self.cell_area_m2 = self
            .topology
            .unit_areas
            .iter()
            .map(|a| a * radius_m * radius_m)
            .collect();
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    pub fn directions(&self) -> &[Vec3] {
        &self.topology.directions
    }

    pub fn direction(&self, cell: usize) -> Vec3 {
        self.topology.directions[cell]
    }

    pub fn neighbors(&self, cell: usize) -> &[u32] {
        self.topology.neighbors(cell)
    }

    pub fn lat_deg(&self, cell: usize) -> f32 {
        self.topology.lat_deg[cell]
    }

    pub fn lon_deg(&self, cell: usize) -> f32 {
        self.topology.lon_deg[cell]
    }

    pub fn area_m2(&self, cell: usize) -> f32 {
        self.cell_area_m2[cell]
    }

    pub fn total_area_m2(&self) -> f64 {
        self.cell_area_m2.iter().map(|a| *a as f64).sum()
    }

    /// Updates ocean depths from a sea level and returns the ocean area (km²)
    /// and ocean volume (km³).
    pub fn apply_sea_level(&mut self, sea_level_m: f32, _radius_m: f32) -> (f64, f64) {
        let mut ocean_area_km2 = 0.0;
        let mut ocean_volume_km3 = 0.0;
        for (cell, area) in self.cells.iter_mut().zip(&self.cell_area_m2) {
            let depth = (sea_level_m - cell.bedrock_m).max(0.0);
            cell.water_depth_m = depth;
            if depth > 0.0 {
                let area_km2 = *area as f64 / 1.0e6;
                ocean_area_km2 += area_km2;
                ocean_volume_km3 += area_km2 * (depth as f64 / 1_000.0);
            }
        }
        (ocean_area_km2, ocean_volume_km3)
    }

    pub fn ocean_area_km2(&self) -> f64 {
        self.cells
            .iter()
            .zip(&self.cell_area_m2)
            .filter(|(c, _)| c.water_depth_m > 0.0)
            .map(|(_, a)| *a as f64 / 1.0e6)
            .sum()
    }

    pub fn ocean_volume_km3(&self) -> f64 {
        self.cells
            .iter()
            .zip(&self.cell_area_m2)
            .map(|(c, a)| (*a as f64 / 1.0e6) * (c.water_depth_m as f64 / 1_000.0))
            .sum()
    }

    pub fn land_area_km2(&self) -> f64 {
        self.total_area_m2() / 1.0e6 - self.ocean_area_km2()
    }

    pub fn weighted_mean<F: Fn(&SurfaceCell) -> f32 + Sync>(&self, f: F) -> f32 {
        let mut sum = 0.0f64;
        let mut area = 0.0f64;
        for (cell, a) in self.cells.iter().zip(&self.cell_area_m2) {
            sum += f(cell) as f64 * *a as f64;
            area += *a as f64;
        }
        (sum / area.max(1.0)) as f32
    }

    /// Nearest cell to a direction, used for map picking and event placement.
    pub fn nearest_cell(&self, dir: Vec3) -> usize {
        let d = dir.normalize_or_zero();
        let mut best = 0usize;
        let mut best_dot = f32::NEG_INFINITY;
        for (i, c) in self.topology.directions.iter().enumerate() {
            let dot = c.dot(d);
            if dot > best_dot {
                best_dot = dot;
                best = i;
            }
        }
        best
    }

    pub fn cell_at_lat_lon(&self, lat_deg: f32, lon_deg: f32) -> usize {
        let lat = lat_deg.to_radians();
        let lon = lon_deg.to_radians();
        let dir = Vec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin());
        self.nearest_cell(dir)
    }

    /// Recomputes cell areas for a planet of the given radius (after load or
    /// a radius change).
    pub fn rebuild_areas(&mut self, radius_m: f32) {
        self.radius_km = radius_m / 1_000.0;
        self.ensure_topology(radius_m);
    }
}

impl PlanetGrid {
    /// Computes ocean area and volume at a candidate sea level without
    /// mutating the cells. Used by the water-budget solver.
    pub fn ocean_metrics_at(&self, sea_level_m: f32) -> (f64, f64) {
        let mut area = 0.0;
        let mut volume = 0.0;
        for (cell, cell_area) in self.cells.iter().zip(&self.cell_area_m2) {
            let depth = (sea_level_m - cell.bedrock_m).max(0.0);
            if depth > 0.0 {
                let a = *cell_area as f64 / 1.0e6;
                area += a;
                volume += a * (depth as f64 / 1_000.0);
            }
        }
        (area, volume)
    }
}
