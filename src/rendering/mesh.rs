use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, Mesh},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};

use crate::rendering::MapLayer;
use crate::simulation::PlanetState;

/// Radial render coordinate. Elevation exaggeration is visual only; the
/// simulation still stores real metres.
pub fn surface_radius(planet: &PlanetState, cell: usize, exaggeration: f32) -> f32 {
    let c = &planet.grid.cells[cell];
    let sea = planet.sea_level_m();
    let surface = c.bedrock_m.max(sea) + c.lake_depth_m.max(0.0);
    let radius_m = planet.config.radius_m().max(1.0);
    1.0 + (surface / radius_m) * exaggeration.max(0.0)
}

pub fn build_planet_mesh(
    planet: &PlanetState,
    layer: MapLayer,
    exaggeration: f32,
    show_clouds: bool,
) -> Mesh {
    let n = planet.grid.cell_count();
    let mut positions = Vec::with_capacity(n);
    let mut normals = Vec::with_capacity(n);
    let mut colors = Vec::with_capacity(n);
    for cell in 0..n {
        let dir = planet.grid.direction(cell);
        positions.push((dir * surface_radius(planet, cell, exaggeration)).to_array());
        normals.push(dir.to_array());
        let color = layer.color(planet, cell, show_clouds);
        colors.push(linear_color(color, 1.0));
    }
    let indices: Vec<u32> = planet
        .grid
        .topology
        .triangles
        .iter()
        .flat_map(|t| [t[0], t[1], t[2]])
        .collect();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub fn build_rivers_mesh(planet: &PlanetState, exaggeration: f32) -> Mesh {
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for cell in 0..planet.grid.cell_count() {
        let c = &planet.grid.cells[cell];
        if c.discharge_m3s < 20_000.0
            || c.bedrock_m < planet.sea_level_m()
            || c.ice_thickness_m > 25.0
        {
            continue;
        }
        let Some(target) = c.flow_to else {
            continue;
        };
        let target = target as usize;
        if target >= planet.grid.cell_count() {
            continue;
        }
        let a = planet.grid.direction(cell) * (surface_radius(planet, cell, exaggeration) + 0.0015);
        let b =
            planet.grid.direction(target) * (surface_radius(planet, target, exaggeration) + 0.0015);
        positions.push(a.to_array());
        positions.push(b.to_array());
        let width = (0.30 + (c.discharge_m3s / 6_000.0).min(0.55)).min(0.85);
        colors.push(linear_color([0.16, 0.56, 0.78], width));
        colors.push(linear_color([0.16, 0.56, 0.78], width));
    }
    let mut mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh
}

fn linear_color(rgb: [f32; 3], alpha: f32) -> [f32; 4] {
    [
        srgb_to_linear(rgb[0]),
        srgb_to_linear(rgb[1]),
        srgb_to_linear(rgb[2]),
        alpha,
    ]
}

fn srgb_to_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
