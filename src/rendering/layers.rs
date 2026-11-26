//! Data-driven map layers and their colour scales.

use crate::simulation::PlanetState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapLayer {
    Natural,
    Elevation,
    Temperature,
    Precipitation,
    Humidity,
    Pressure,
    Biome,
    Tectonics,
    Volcanism,
    Water,
    Ice,
    OceanDepth,
    Albedo,
    Co2,
    Geology,
}

impl MapLayer {
    pub const ALL: [Self; 15] = [
        Self::Natural,
        Self::Elevation,
        Self::Temperature,
        Self::Precipitation,
        Self::Humidity,
        Self::Pressure,
        Self::Biome,
        Self::Tectonics,
        Self::Volcanism,
        Self::Water,
        Self::Ice,
        Self::OceanDepth,
        Self::Albedo,
        Self::Co2,
        Self::Geology,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Natural => "Natural appearance",
            Self::Elevation => "Elevation",
            Self::Temperature => "Temperature",
            Self::Precipitation => "Precipitation",
            Self::Humidity => "Humidity",
            Self::Pressure => "Atmospheric pressure",
            Self::Biome => "Climate zones / biomes",
            Self::Tectonics => "Tectonic plates",
            Self::Volcanism => "Volcanic activity",
            Self::Water => "Rivers, lakes, water",
            Self::Ice => "Ice and snow",
            Self::OceanDepth => "Ocean depth",
            Self::Albedo => "Surface albedo",
            Self::Co2 => "Atmospheric CO₂",
            Self::Geology => "Geological activity",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Self::Elevation | Self::OceanDepth => "m",
            Self::Temperature => "°C",
            Self::Precipitation => "mm/yr",
            Self::Humidity => "0–1",
            Self::Pressure => "kPa",
            Self::Albedo => "0–1",
            Self::Co2 => "ppm",
            Self::Ice => "m ice",
            Self::Volcanism | Self::Geology => "activity",
            _ => "",
        }
    }

    pub fn range(self) -> Option<(f32, f32)> {
        match self {
            Self::Elevation => Some((-11_000.0, 9_000.0)),
            Self::Temperature => Some((-70.0, 45.0)),
            Self::Precipitation => Some((0.0, 3_500.0)),
            Self::Humidity => Some((0.0, 1.0)),
            Self::Pressure => Some((40.0, 110.0)),
            Self::OceanDepth => Some((0.0, 11_000.0)),
            Self::Albedo => Some((0.0, 0.9)),
            Self::Co2 => Some((100.0, 5_000.0)),
            Self::Ice => Some((0.0, 1_000.0)),
            Self::Volcanism | Self::Geology => Some((0.0, 1.0)),
            _ => None,
        }
    }

    /// Colour stops in normalised space (sRGB).
    pub fn stops(self) -> &'static [(f32, [f32; 3])] {
        match self {
            Self::Elevation => &ELEVATION_STOPS,
            Self::Temperature => &TEMP_STOPS,
            Self::Precipitation => &PRECIP_STOPS,
            Self::Humidity => &HUMIDITY_STOPS,
            Self::Pressure => &PRESSURE_STOPS,
            Self::OceanDepth => &OCEAN_STOPS,
            Self::Albedo => &ALBEDO_STOPS,
            Self::Co2 => &CO2_STOPS,
            Self::Ice => &ICE_STOPS,
            Self::Volcanism => &VOLCANO_STOPS,
            Self::Geology => &GEOLOGY_STOPS,
            _ => &NATURAL_STOPS,
        }
    }

    pub fn color(self, planet: &PlanetState, cell: usize, show_clouds: bool) -> [f32; 3] {
        let c = &planet.grid.cells[cell];
        let sea = planet.sea_level_m();
        let submerged = c.bedrock_m < sea;
        match self {
            Self::Natural => natural_color(planet, cell, show_clouds),
            Self::Elevation => {
                let (min, max) = self.range().unwrap();
                let t = normalize(c.bedrock_m, min, max);
                sample(self.stops(), t)
            }
            Self::Temperature => {
                let t = normalize(c.temp_c, -70.0, 45.0);
                sample(self.stops(), t)
            }
            Self::Precipitation => {
                let t = (c.precip_mm_yr / 3_500.0).clamp(0.0, 1.0).sqrt();
                sample(self.stops(), t)
            }
            Self::Humidity => sample(self.stops(), c.humidity.clamp(0.0, 1.0)),
            Self::Pressure => {
                let t = normalize(c.pressure_kpa, 40.0, 110.0);
                sample(self.stops(), t)
            }
            Self::Biome => c.biome.color(),
            Self::Tectonics => {
                let base = plate_color(c.plate);
                let boundary = planet
                    .grid
                    .neighbors(cell)
                    .iter()
                    .any(|&nb| planet.grid.cells[nb as usize].plate != c.plate);
                if boundary {
                    [0.98, 0.92, 0.78]
                } else if c.bedrock_m < sea {
                    [base[0] * 0.45, base[1] * 0.55, base[2] * 0.7]
                } else {
                    base
                }
            }
            Self::Volcanism => {
                let t = (c.volcanic_activity.max(c.lava_flow) * 1.2).clamp(0.0, 1.0);
                sample(self.stops(), t)
            }
            Self::Water => {
                if c.ice_thickness_m > 0.5 && c.temp_c < 0.0 {
                    [0.86, 0.92, 1.0]
                } else if c.lake_depth_m > 0.1 {
                    [0.10, 0.45, 0.75]
                } else if submerged {
                    let t = (c.water_depth_m / 5_000.0).clamp(0.0, 1.0);
                    sample(&OCEAN_STOPS, t)
                } else if c.discharge_m3s > 15_000.0 {
                    [0.18, 0.65, 0.9]
                } else {
                    [0.22, 0.20, 0.16]
                }
            }
            Self::Ice => {
                let t = (c.ice_thickness_m / 500.0).clamp(0.0, 1.0);
                sample(self.stops(), t)
            }
            Self::OceanDepth => {
                if submerged {
                    let t = (c.water_depth_m / 8_000.0).clamp(0.0, 1.0).sqrt();
                    sample(&OCEAN_STOPS, 1.0 - t)
                } else {
                    [0.16, 0.16, 0.14]
                }
            }
            Self::Albedo => sample(self.stops(), c.albedo.clamp(0.0, 1.0)),
            Self::Co2 => {
                let t = ((planet.atmosphere.co2_ppm.max(1.0).log10() - 2.0)
                    / (5_000.0f32.log10() - 2.0))
                    .clamp(0.0, 1.0);
                sample(self.stops(), t)
            }
            Self::Geology => {
                let elev = normalize(c.bedrock_m, -11_000.0, 9_000.0);
                let base = sample(&GEOLOGY_STOPS, elev);
                let heat =
                    (c.volcanic_activity.max(c.lava_flow) + c.quake_activity * 0.5).clamp(0.0, 1.0);
                mix(base, [1.0, 0.32, 0.06], heat * 0.9)
            }
        }
    }
}

fn natural_color(planet: &PlanetState, cell: usize, show_clouds: bool) -> [f32; 3] {
    let c = &planet.grid.cells[cell];
    let sea = planet.sea_level_m();
    let submerged = c.bedrock_m < sea;
    let mut color = if c.ice_thickness_m > 0.5 {
        mix(
            [0.72, 0.84, 0.93],
            [0.96, 0.99, 1.0],
            (c.ice_thickness_m / 800.0).min(1.0),
        )
    } else if c.lake_depth_m > 0.15 {
        [0.09, 0.32, 0.55]
    } else if submerged {
        let depth = (c.water_depth_m / 5_500.0).clamp(0.0, 1.0);
        mix([0.10, 0.34, 0.48], [0.008, 0.035, 0.14], depth.sqrt())
    } else {
        let mut base = c.biome.color();
        if c.bedrock_m > 3_500.0 {
            let t = ((c.bedrock_m - 3_500.0) / 5_000.0).clamp(0.0, 1.0);
            base = mix(base, [0.55, 0.52, 0.48], t * 0.8);
        }
        if c.temp_c < 1.0 {
            let snow = ((-c.temp_c) / 12.0).clamp(0.0, 0.85);
            base = mix(base, [0.92, 0.96, 1.0], snow);
        }
        if c.soil_moisture_m > 0.32 {
            base = mix(base, [0.08, 0.22, 0.13], 0.15);
        }
        base
    };

    if c.discharge_m3s > 15_000.0 && !submerged {
        color = mix(color, [0.20, 0.62, 0.86], 0.55);
    }
    if c.lava_flow > 0.1 {
        color = mix(color, [1.0, 0.22, 0.02], (c.lava_flow * 0.9).min(0.9));
    }
    if c.volcanic_activity > 0.3 && c.lava_flow < 0.1 {
        color = mix(color, [0.45, 0.20, 0.12], c.volcanic_activity * 0.5);
    }
    // A subtle cloud veil, driven by the simulated cloud field. Keeping it
    // in the surface shading avoids a transparent sphere that would hide the
    // planet; the cloud field also drives the dedicated map/statistics.
    if show_clouds && c.cloud_cover > 0.3 {
        color = mix(color, [0.90, 0.94, 0.97], c.cloud_cover * 0.18);
    }
    color
}

pub fn plate_color(plate: u16) -> [f32; 3] {
    let h = (plate as f32 * 0.618_034).fract();
    let s = 0.55;
    let v = 0.75;
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match (i as i32) % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

fn sample(stops: &[(f32, [f32; 3])], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    let mut prev = stops[0];
    for &next in stops.iter().skip(1) {
        if t <= next.0 {
            let span = (next.0 - prev.0).max(1.0e-6);
            let f = (t - prev.0) / span;
            return mix(prev.1, next.1, f);
        }
        prev = next;
    }
    prev.1
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn normalize(value: f32, min: f32, max: f32) -> f32 {
    ((value - min) / (max - min).max(1.0e-6)).clamp(0.0, 1.0)
}

const NATURAL_STOPS: [(f32, [f32; 3]); 5] = [
    (0.0, [0.02, 0.10, 0.28]),
    (0.35, [0.10, 0.38, 0.20]),
    (0.60, [0.42, 0.48, 0.22]),
    (0.80, [0.58, 0.50, 0.34]),
    (1.0, [0.94, 0.96, 0.98]),
];
const ELEVATION_STOPS: [(f32, [f32; 3]); 7] = [
    (0.0, [0.02, 0.05, 0.22]),
    (0.28, [0.03, 0.18, 0.42]),
    (0.44, [0.08, 0.38, 0.52]),
    (0.50, [0.76, 0.70, 0.48]),
    (0.60, [0.24, 0.48, 0.18]),
    (0.78, [0.48, 0.36, 0.22]),
    (1.0, [0.94, 0.94, 0.92]),
];
const TEMP_STOPS: [(f32, [f32; 3]); 6] = [
    (0.0, [0.10, 0.06, 0.38]),
    (0.20, [0.12, 0.35, 0.78]),
    (0.42, [0.18, 0.68, 0.78]),
    (0.62, [0.45, 0.78, 0.35]),
    (0.80, [0.96, 0.72, 0.16]),
    (1.0, [0.82, 0.10, 0.05]),
];
const PRECIP_STOPS: [(f32, [f32; 3]); 5] = [
    (0.0, [0.55, 0.35, 0.14]),
    (0.25, [0.62, 0.58, 0.25]),
    (0.50, [0.24, 0.58, 0.24]),
    (0.75, [0.10, 0.44, 0.52]),
    (1.0, [0.08, 0.20, 0.65]),
];
const HUMIDITY_STOPS: [(f32, [f32; 3]); 4] = [
    (0.0, [0.50, 0.30, 0.12]),
    (0.35, [0.52, 0.50, 0.22]),
    (0.70, [0.16, 0.55, 0.42]),
    (1.0, [0.06, 0.26, 0.58]),
];
const PRESSURE_STOPS: [(f32, [f32; 3]); 4] = [
    (0.0, [0.30, 0.10, 0.45]),
    (0.40, [0.18, 0.36, 0.72]),
    (0.75, [0.40, 0.74, 0.80]),
    (1.0, [0.94, 0.96, 1.0]),
];
const OCEAN_STOPS: [(f32, [f32; 3]); 4] = [
    (0.0, [0.18, 0.55, 0.62]),
    (0.35, [0.06, 0.28, 0.48]),
    (0.70, [0.02, 0.12, 0.32]),
    (1.0, [0.005, 0.02, 0.12]),
];
const ALBEDO_STOPS: [(f32, [f32; 3]); 3] = [
    (0.0, [0.03, 0.03, 0.04]),
    (0.5, [0.45, 0.45, 0.45]),
    (1.0, [0.98, 0.98, 0.98]),
];
const CO2_STOPS: [(f32, [f32; 3]); 5] = [
    (0.0, [0.08, 0.22, 0.42]),
    (0.35, [0.18, 0.58, 0.52]),
    (0.60, [0.82, 0.72, 0.22]),
    (0.80, [0.86, 0.36, 0.10]),
    (1.0, [0.58, 0.06, 0.10]),
];
const ICE_STOPS: [(f32, [f32; 3]); 3] = [
    (0.0, [0.035, 0.12, 0.22]),
    (0.4, [0.32, 0.62, 0.78]),
    (1.0, [0.96, 0.99, 1.0]),
];
const VOLCANO_STOPS: [(f32, [f32; 3]); 4] = [
    (0.0, [0.03, 0.03, 0.035]),
    (0.35, [0.40, 0.10, 0.06]),
    (0.70, [0.92, 0.32, 0.04]),
    (1.0, [1.0, 0.88, 0.36]),
];
const GEOLOGY_STOPS: [(f32, [f32; 3]); 5] = [
    (0.0, [0.05, 0.08, 0.18]),
    (0.35, [0.16, 0.30, 0.36]),
    (0.55, [0.35, 0.42, 0.28]),
    (0.78, [0.58, 0.45, 0.30]),
    (1.0, [0.82, 0.76, 0.66]),
];
