//! Icosphere topology and neighbour graph.

use std::collections::HashMap;

use glam::Vec3;

#[derive(Clone, Debug, Default)]
pub struct IcosphereTopology {
    pub level: u8,
    pub directions: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
    /// CSR neighbour graph: neighbours of cell `i` live in
    /// `neighbor_indices[offsets[i]..offsets[i + 1]]`.
    pub offsets: Vec<u32>,
    pub neighbor_indices: Vec<u32>,
    /// Area of each spherical triangle in steradians.
    pub unit_areas: Vec<f32>,
    pub lat_deg: Vec<f32>,
    pub lon_deg: Vec<f32>,
}

impl IcosphereTopology {
    pub fn cell_count(&self) -> usize {
        self.directions.len()
    }

    pub fn neighbors(&self, cell: usize) -> &[u32] {
        let start = self.offsets[cell] as usize;
        let end = self.offsets[cell + 1] as usize;
        &self.neighbor_indices[start..end]
    }

    pub fn area_m2(&self, cell: usize, radius_m: f32) -> f32 {
        self.unit_areas[cell] * radius_m * radius_m
    }

    /// Great-circle angle between two cells in radians.
    pub fn angle_between(&self, a: usize, b: usize) -> f32 {
        self.directions[a]
            .dot(self.directions[b])
            .clamp(-1.0, 1.0)
            .acos()
    }
}

/// Builds a subdivided icosahedron. Level 5 yields 10,242 cells,
/// level 6 yields 40,962 and level 7 yields 163,842.
pub fn build_topology(level: u8) -> IcosphereTopology {
    let level = level.clamp(1, 8);
    let (mut verts, mut tris) = base_icosahedron();
    let mut cache: HashMap<(u32, u32), u32> = HashMap::new();

    for _ in 1..level {
        let mut next = Vec::with_capacity(tris.len() * 4);
        for tri in &tris {
            let a = tri[0];
            let b = tri[1];
            let c = tri[2];
            let ab = midpoint(a, b, &mut verts, &mut cache);
            let bc = midpoint(b, c, &mut verts, &mut cache);
            let ca = midpoint(c, a, &mut verts, &mut cache);
            next.push([a, ab, ca]);
            next.push([b, bc, ab]);
            next.push([c, ca, bc]);
            next.push([ab, bc, ca]);
        }
        tris = next;
    }

    // Ensure outward-facing winding (counter-clockwise seen from outside).
    for tri in &mut tris {
        let a = verts[tri[0] as usize];
        let b = verts[tri[1] as usize];
        let c = verts[tri[2] as usize];
        let normal = (b - a).cross(c - a);
        if normal.dot(a + b + c) < 0.0 {
            tri.swap(1, 2);
        }
    }

    let directions: Vec<Vec3> = verts.iter().map(|v| v.normalize()).collect();
    let n = directions.len();

    let mut adjacency: Vec<Vec<u32>> = vec![Vec::with_capacity(6); n];
    let mut unit_areas = vec![0.0f32; n];
    for tri in &tris {
        let a = directions[tri[0] as usize];
        let b = directions[tri[1] as usize];
        let c = directions[tri[2] as usize];
        let area = spherical_triangle_area(a, b, c);
        for &v in tri {
            unit_areas[v as usize] += area / 3.0;
        }
        add_edge(&mut adjacency, tri[0], tri[1]);
        add_edge(&mut adjacency, tri[1], tri[2]);
        add_edge(&mut adjacency, tri[2], tri[0]);
    }
    for list in &mut adjacency {
        list.sort_unstable();
        list.dedup();
    }

    let mut offsets = Vec::with_capacity(n + 1);
    let mut neighbor_indices = Vec::with_capacity(n * 6);
    offsets.push(0u32);
    for list in &adjacency {
        neighbor_indices.extend_from_slice(list);
        offsets.push(neighbor_indices.len() as u32);
    }

    let mut lat_deg = Vec::with_capacity(n);
    let mut lon_deg = Vec::with_capacity(n);
    for d in &directions {
        lat_deg.push(d.y.clamp(-1.0, 1.0).asin().to_degrees());
        lon_deg.push(d.z.atan2(d.x).to_degrees());
    }

    IcosphereTopology {
        level,
        directions,
        triangles: tris,
        offsets,
        neighbor_indices,
        unit_areas,
        lat_deg,
        lon_deg,
    }
}

fn base_icosahedron() -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let t = (1.0 + 5.0f32.sqrt()) / 2.0;
    let raw = [
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ];
    let verts = raw
        .iter()
        .map(|v| Vec3::new(v[0], v[1], v[2]).normalize())
        .collect();
    let tris = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    (verts, tris)
}

fn midpoint(a: u32, b: u32, verts: &mut Vec<Vec3>, cache: &mut HashMap<(u32, u32), u32>) -> u32 {
    let key = if a < b { (a, b) } else { (b, a) };
    if let Some(&idx) = cache.get(&key) {
        return idx;
    }
    let mid = (verts[a as usize] + verts[b as usize]).normalize();
    let idx = verts.len() as u32;
    verts.push(mid);
    cache.insert(key, idx);
    idx
}

fn add_edge(adjacency: &mut [Vec<u32>], a: u32, b: u32) {
    adjacency[a as usize].push(b);
    adjacency[b as usize].push(a);
}

fn spherical_triangle_area(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    // Spherical excess via the vector form of the solid-angle formula.
    // This is exact for a unit-sphere triangle and avoids the numerical
    // pitfalls of L'Huilier with chord lengths.
    let numerator = a.dot(b.cross(c)).abs();
    let denominator = 1.0 + a.dot(b) + b.dot(c) + c.dot(a);
    2.0 * numerator.atan2(denominator.max(1.0e-9))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn icosphere_is_closed_and_uniform() {
        for level in 1..=5 {
            let topo = build_topology(level);
            let expected_cells = 10 * 4usize.pow(level as u32 - 1) + 2;
            assert_eq!(topo.cell_count(), expected_cells);
            let total_area: f32 = topo.unit_areas.iter().sum();
            assert!(
                (total_area - 4.0 * PI).abs() < 1.0e-3,
                "level {level}: {total_area}"
            );

            for cell in 0..topo.cell_count() {
                let neighbors = topo.neighbors(cell);
                assert!(neighbors.len() >= 5 && neighbors.len() <= 6);
                for &n in neighbors {
                    assert!(topo.neighbors(n as usize).contains(&(cell as u32)));
                }
            }
        }
    }

    #[test]
    fn directions_are_unit_vectors() {
        let topo = build_topology(3);
        for d in &topo.directions {
            assert!((d.length() - 1.0).abs() < 1.0e-5);
        }
    }
}
