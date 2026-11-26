//! Biome classification and palettes.
//!
//! Biomes are recomputed from the simulated temperature, precipitation and
//! ice state; they are never assigned randomly after generation.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Biome {
    Ocean,
    Lake,
    TropicalRainforest,
    TropicalSeasonalForest,
    Savanna,
    Desert,
    Grassland,
    TemperateForest,
    BorealForest,
    Taiga,
    Tundra,
    PolarDesert,
    IceSheet,
}

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ocean => "Ocean",
            Self::Lake => "Lake",
            Self::TropicalRainforest => "Tropical rainforest",
            Self::TropicalSeasonalForest => "Tropical seasonal forest",
            Self::Savanna => "Savanna",
            Self::Desert => "Desert",
            Self::Grassland => "Grassland",
            Self::TemperateForest => "Temperate forest",
            Self::BorealForest => "Boreal forest",
            Self::Taiga => "Taiga",
            Self::Tundra => "Tundra",
            Self::PolarDesert => "Polar desert",
            Self::IceSheet => "Permanent ice sheet",
        }
    }

    /// Typical clear-sky surface albedo.
    pub fn base_albedo(self) -> f32 {
        match self {
            Self::Ocean => 0.06,
            Self::Lake => 0.08,
            Self::TropicalRainforest => 0.12,
            Self::TropicalSeasonalForest => 0.14,
            Self::Savanna => 0.20,
            Self::Desert => 0.36,
            Self::Grassland => 0.22,
            Self::TemperateForest => 0.15,
            Self::BorealForest => 0.13,
            Self::Taiga => 0.15,
            Self::Tundra => 0.20,
            Self::PolarDesert => 0.38,
            Self::IceSheet => 0.72,
        }
    }

    /// sRGB display colour for natural rendering and biome maps.
    pub fn color(self) -> [f32; 3] {
        match self {
            Self::Ocean => [0.045, 0.13, 0.30],
            Self::Lake => [0.08, 0.28, 0.46],
            Self::TropicalRainforest => [0.045, 0.24, 0.07],
            Self::TropicalSeasonalForest => [0.11, 0.32, 0.08],
            Self::Savanna => [0.45, 0.52, 0.16],
            Self::Desert => [0.72, 0.60, 0.36],
            Self::Grassland => [0.35, 0.47, 0.20],
            Self::TemperateForest => [0.10, 0.30, 0.12],
            Self::BorealForest => [0.06, 0.22, 0.14],
            Self::Taiga => [0.08, 0.20, 0.16],
            Self::Tundra => [0.45, 0.46, 0.38],
            Self::PolarDesert => [0.72, 0.75, 0.78],
            Self::IceSheet => [0.92, 0.96, 1.0],
        }
    }

    pub fn is_water(self) -> bool {
        matches!(self, Self::Ocean | Self::Lake)
    }

    /// Classify a land cell from climate. `ice_thickness_m` forces ice sheets.
    pub fn classify(temp_c: f32, precip_mm_yr: f32, ice_thickness_m: f32) -> Self {
        if ice_thickness_m > 25.0 {
            return Self::IceSheet;
        }
        if temp_c < -12.0 {
            return if precip_mm_yr < 80.0 {
                Self::PolarDesert
            } else {
                Self::IceSheet
            };
        }
        if temp_c < -2.0 {
            return if precip_mm_yr < 60.0 {
                Self::PolarDesert
            } else {
                Self::Tundra
            };
        }
        if temp_c < 5.0 {
            return if precip_mm_yr < 120.0 {
                Self::Tundra
            } else {
                Self::Taiga
            };
        }
        if temp_c < 11.0 {
            return if precip_mm_yr < 150.0 {
                Self::Grassland
            } else {
                Self::BorealForest
            };
        }
        if temp_c < 20.0 {
            if precip_mm_yr < 180.0 {
                return Self::Desert;
            }
            if precip_mm_yr < 420.0 {
                return Self::Grassland;
            }
            return Self::TemperateForest;
        }
        if precip_mm_yr < 220.0 {
            Self::Desert
        } else if precip_mm_yr < 650.0 {
            Self::Savanna
        } else if precip_mm_yr < 1_700.0 {
            Self::TropicalSeasonalForest
        } else {
            Self::TropicalRainforest
        }
    }
}
