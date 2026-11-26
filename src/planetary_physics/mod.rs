//! Shared physical constants and simplified radiative-transfer helpers.
//!
//! # Model limitations
//!
//! These are one-layer, zonal-mean approximations, not a general circulation
//! model. They reproduce the direction and rough magnitude of the major
//! feedbacks (CO₂ forcing, ice-albedo, aerosol dimming, thermal inertia) but
//! not weather-resolved physics. See `docs/MODEL.md`.

use std::f32::consts::PI;

pub const SOLAR_CONSTANT_W_M2: f32 = 1_361.0;
pub const STEFAN_BOLTZMANN: f32 = 5.670_374_5e-8;
pub const SECONDS_PER_YEAR: f64 = 365.25 * 24.0 * 3600.0;
pub const EARTH_RADIUS_M: f32 = 6_371_000.0;
pub const EARTH_SURFACE_AREA_KM2: f64 = 510_072_000.0;
pub const WATER_DENSITY_KG_M3: f32 = 1_000.0;

/// Daily-mean top-of-atmosphere insolation for a latitude and solar
/// declination. Both angles in radians; result in W/m².
pub fn daily_insolation(flux_w_m2: f32, lat_rad: f32, declination_rad: f32) -> f32 {
    let lat = lat_rad.clamp(-1.55, 1.55);
    let dec = declination_rad.clamp(-1.55, 1.55);
    let x = -(lat.tan() * dec.tan());
    let h0 = if x >= 1.0 {
        0.0
    } else if x <= -1.0 {
        PI
    } else {
        x.acos()
    };
    if h0 <= 0.0 {
        return 0.0;
    }
    let sin_lat = lat.sin();
    let cos_lat = lat.cos();
    let sin_dec = dec.sin();
    let cos_dec = dec.cos();
    (flux_w_m2 / PI) * (h0 * sin_lat * sin_dec + cos_lat * cos_dec * h0.sin())
}

/// Annual-mean insolation for a latitude. The second Legendre polynomial
/// approximation is accurate to a few percent for Earth-like obliquity.
pub fn annual_mean_insolation(flux_w_m2: f32, lat_rad: f32) -> f32 {
    let s = lat_rad.sin();
    let p2 = (3.0 * s * s - 1.0) * 0.5;
    (flux_w_m2 / 4.0) * (1.0 - 0.482 * p2)
}

/// Solar declination for a day of year and obliquity in degrees.
pub fn declination_rad(tilt_deg: f32, day_of_year: f32) -> f32 {
    (tilt_deg.to_radians() * (std::f32::consts::TAU * (day_of_year - 80.0) / 365.25).sin())
        .clamp(-1.5, 1.5)
}

/// Clear-sky greenhouse strength. 0.41 corresponds to an Earth-like
/// atmosphere with 280 ppm CO₂. CO₂ forcing is logarithmic, with a
/// water-vapour feedback tied to the global temperature anomaly.
pub fn greenhouse_strength(co2_ppm: f32, global_temp_c: f32, ch4_ppm: f32) -> f32 {
    let co2 = co2_ppm.max(0.1);
    let ch4 = ch4_ppm.max(0.0);
    let co2_forcing = 5.35 * (co2 / 280.0).ln();
    let ch4_forcing = 0.036 * ((ch4 + 0.1).sqrt() - 0.1f32.sqrt());
    // 1.6 includes the fast water-vapour/lapse-rate feedback.
    let feedback = 1.6 * (co2_forcing + ch4_forcing) / 238.0;
    // The explicit temperature term is a small residual water-vapour effect;
    // the main fast feedback is already represented by the 1.6 multiplier.
    let vapour_feedback = (0.004 * (global_temp_c - 15.0)).clamp(-0.03, 0.04);
    (0.41 + feedback + vapour_feedback).clamp(0.05, 0.74)
}

/// Effective outgoing longwave radiation for a surface temperature.
pub fn outgoing_longwave(temp_c: f32, greenhouse: f32) -> f32 {
    let t_k = (temp_c + 273.15).max(1.0);
    STEFAN_BOLTZMANN * t_k.powi(4) * (1.0 - greenhouse).max(0.01)
}

/// Radiative-equilibrium surface temperature for absorbed flux and
/// greenhouse strength.
pub fn equilibrium_temperature_c(absorbed_w_m2: f32, greenhouse: f32) -> f32 {
    let olr_coeff = (1.0 - greenhouse).clamp(0.05, 0.99);
    (absorbed_w_m2.max(0.0) / (STEFAN_BOLTZMANN * olr_coeff)).powf(0.25) - 273.15
}

/// Planetary albedo from surface albedo and cloud coverage.
pub fn planetary_albedo(surface_albedo: f32, cloud_fraction: f32) -> f32 {
    let cloud = cloud_fraction.clamp(0.0, 1.0);
    (surface_albedo * (1.0 - cloud) + 0.44 * cloud).clamp(0.02, 0.95)
}

/// Fraction of sunlight reaching the surface under an aerosol/dust optical
/// depth. This is a broadband Beer-Lambert approximation.
pub fn aerosol_transmission(optical_depth: f32) -> f32 {
    (-0.65 * optical_depth.max(0.0)).exp()
}

/// Exponential relaxation that is stable for arbitrarily large `dt`.
pub fn relax(current: f32, target: f32, dt_years: f32, tau_years: f32) -> f32 {
    if tau_years <= 1.0e-6 {
        return target;
    }
    let alpha = 1.0 - (-dt_years.max(0.0) / tau_years).exp();
    current + (target - current) * alpha
}

/// CO₂ radiative forcing in W/m² relative to a reference concentration.
pub fn co2_forcing(co2_ppm: f32, reference_ppm: f32) -> f32 {
    5.35 * (co2_ppm.max(0.1) / reference_ppm.max(0.1)).ln()
}

/// Pressure scale height for a nitrogen/oxygen atmosphere.
pub fn scale_height_m(temp_c: f32, gravity: f32, mean_molar_mass_g: f32) -> f32 {
    const R: f32 = 8.314_462;
    let t = (temp_c + 273.15).max(1.0);
    let m = (mean_molar_mass_g * 1.0e-3).max(1.0e-6);
    R * t / (m * gravity.max(0.01))
}

/// Thermal expansion contribution to sea level (metres) for a temperature
/// change of the mixed layer. A single global coefficient is used.
pub fn thermal_expansion_m(delta_temp_c: f32, mean_depth_m: f32) -> f32 {
    2.0e-4 * delta_temp_c * mean_depth_m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earth_annual_insolation_is_reasonable() {
        let equator = annual_mean_insolation(SOLAR_CONSTANT_W_M2, 0.0);
        let pole = annual_mean_insolation(SOLAR_CONSTANT_W_M2, 89.0f32.to_radians());
        assert!((equator - 420.0).abs() < 20.0, "equator={equator}");
        assert!((pole - 175.0).abs() < 20.0, "pole={pole}");
    }

    #[test]
    fn double_co2_warming_is_positive_and_plausible() {
        let base = equilibrium_temperature_c(
            planetary_albedo(0.15, 0.6) * SOLAR_CONSTANT_W_M2 / 4.0,
            greenhouse_strength(280.0, 15.0, 1.9),
        );
        let warm = equilibrium_temperature_c(
            planetary_albedo(0.15, 0.6) * SOLAR_CONSTANT_W_M2 / 4.0,
            greenhouse_strength(560.0, 15.0, 1.9),
        );
        let warming = warm - base;
        assert!(warming > 1.0 && warming < 8.0, "warming={warming}");
    }

    #[test]
    fn relaxation_is_stable() {
        let v = relax(0.0, 100.0, 1.0e9, 30.0);
        assert!((v - 100.0).abs() < 1.0e-3);
    }
}
