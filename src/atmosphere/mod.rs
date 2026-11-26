//! Atmospheric composition, opacity and simplified greenhouse state.

use serde::{Deserialize, Serialize};

use crate::planet_generator::PlanetConfig;
use crate::planetary_physics::aerosol_transmission;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AtmosphereState {
    pub pressure_kpa: f32,
    pub co2_ppm: f32,
    pub o2_fraction: f32,
    pub n2_fraction: f32,
    pub h2o_fraction: f32,
    pub ch4_ppm: f32,
    /// Transient stratospheric aerosol optical depth (volcanic, impact dust).
    pub aerosol_optical_depth: f32,
    /// Impact/ash dust optical depth with a different removal timescale.
    pub dust_optical_depth: f32,
    pub cloud_coverage: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CarbonState {
    /// Dissolved inorganic carbon expressed in atmosphere-equivalent ppm.
    pub ocean_ppm_equiv: f32,
    /// Long-lived geological carbon expressed in atmosphere-equivalent ppm.
    pub geological_ppm_equiv: f32,
    /// Cumulative carbon added/removed by the user outside the modelled
    /// reservoirs. Kept explicit so the UI never implies false conservation.
    pub external_ppm: f32,
}

impl AtmosphereState {
    pub fn from_config(config: &PlanetConfig) -> Self {
        Self {
            pressure_kpa: config.surface_pressure_kpa,
            co2_ppm: config.co2_ppm,
            o2_fraction: config.o2_fraction,
            n2_fraction: config.n2_fraction,
            h2o_fraction: config.h2o_fraction,
            ch4_ppm: config.ch4_ppm,
            aerosol_optical_depth: 0.0,
            dust_optical_depth: 0.0,
            cloud_coverage: config.cloud_coverage,
        }
    }

    pub fn total_optical_depth(&self) -> f32 {
        self.aerosol_optical_depth + self.dust_optical_depth
    }

    pub fn surface_transmission(&self) -> f32 {
        aerosol_transmission(self.total_optical_depth())
    }

    /// Mean molar mass in g/mol, used for the pressure scale height.
    pub fn mean_molar_mass_g(&self) -> f32 {
        let (o2, n2, h2o) = self.normalised_composition();
        let co2 = (self.co2_ppm * 1.0e-6).clamp(0.0, 1.0);
        let ch4 = (self.ch4_ppm * 1.0e-6).clamp(0.0, 1.0);
        let inert = (1.0 - o2 - n2 - h2o - co2 - ch4).max(0.0);
        32.0 * o2 + 28.0 * n2 + 18.0 * h2o + 44.0 * co2 + 16.0 * ch4 + 44.0 * inert
    }

    pub fn normalised_composition(&self) -> (f32, f32, f32) {
        let o2 = self.o2_fraction.max(0.0);
        let n2 = self.n2_fraction.max(0.0);
        let h2o = self.h2o_fraction.max(0.0);
        let sum = (o2 + n2 + h2o).max(1.0e-6);
        (o2 / sum, n2 / sum, h2o / sum)
    }

    /// Aerosols and dust settle out. Stable for very large time steps.
    pub fn decay_opacity(&mut self, dt_years: f32) {
        let aerosol_tau = 1.6;
        let dust_tau = 2.5;
        let a = (-dt_years.max(0.0) / aerosol_tau).exp();
        let d = (-dt_years.max(0.0) / dust_tau).exp();
        self.aerosol_optical_depth *= a;
        self.dust_optical_depth *= d;
        if self.aerosol_optical_depth < 1.0e-5 {
            self.aerosol_optical_depth = 0.0;
        }
        if self.dust_optical_depth < 1.0e-5 {
            self.dust_optical_depth = 0.0;
        }
    }

    pub fn inject_aerosol(&mut self, optical_depth: f32) {
        self.aerosol_optical_depth = (self.aerosol_optical_depth + optical_depth).clamp(0.0, 25.0);
    }

    pub fn inject_dust(&mut self, optical_depth: f32) {
        self.dust_optical_depth = (self.dust_optical_depth + optical_depth).clamp(0.0, 40.0);
    }
}

impl CarbonState {
    pub fn from_config(config: &PlanetConfig) -> Self {
        // The reservoirs are scaled relative to the initial atmosphere. These
        // ratios are illustrative, not a calibrated carbon-cycle inversion.
        Self {
            ocean_ppm_equiv: config.co2_ppm * 2.8,
            geological_ppm_equiv: config.co2_ppm * 120.0,
            external_ppm: 0.0,
        }
    }

    pub fn total_ppm(&self, atmosphere_ppm: f32) -> f32 {
        atmosphere_ppm + self.ocean_ppm_equiv + self.geological_ppm_equiv + self.external_ppm
    }

    /// Simplified exchange: atmosphere ↔ ocean, then a slow weathering sink
    /// and a geological outgassing source. Returns net atmosphere change.
    pub fn exchange(&mut self, atmosphere_ppm: &mut f32, dt_years: f32, ocean_temp_c: f32) {
        let dt = dt_years.max(0.0);
        if dt == 0.0 {
            return;
        }
        let atm = *atmosphere_ppm;
        // Warmer oceans hold less CO₂, so the equilibrium atmospheric
        // concentration rises with ocean temperature.
        let solubility = 1.0 - 0.012 * (ocean_temp_c - 15.0).clamp(-30.0, 30.0);
        let equilibrium_atm = (self.ocean_ppm_equiv / 2.8).max(0.1) * solubility;
        let tau = 280.0; // years for mixed-layer equilibration
        let alpha = 1.0 - (-dt / tau).exp();
        let mut delta = (equilibrium_atm - atm) * alpha;
        delta = delta.clamp(-self.ocean_ppm_equiv.max(0.0), atm.max(0.0));
        *atmosphere_ppm += delta;
        self.ocean_ppm_equiv -= delta;

        // Silicate weathering: a slow sink proportional to temperature and
        // atmospheric CO₂. It nearly stops on a frozen planet. Material is
        // moved to the geological reservoir.
        let temperature_factor =
            (1.0 + 0.04 * (ocean_temp_c - 15.0).clamp(-30.0, 30.0)).clamp(0.05, 3.0);
        let weathering = *atmosphere_ppm / 280.0 * temperature_factor * 0.003 * dt;
        let weather = weathering.min(*atmosphere_ppm * 0.999).max(0.0);
        *atmosphere_ppm -= weather;
        self.geological_ppm_equiv += weather;

        // Baseline volcanic/metamorphic outgassing. On a cold, frozen world
        // weathering is suppressed while this source continues, so CO₂
        // accumulates and can eventually break a snowball state.
        let wanted = 0.003 * dt;
        let outgassing = wanted.min(self.geological_ppm_equiv.max(0.0));
        self.geological_ppm_equiv -= outgassing;
        *atmosphere_ppm += outgassing;
    }

    pub fn outgas(&mut self, atmosphere_ppm: &mut f32, ppm: f32) {
        let available = self.geological_ppm_equiv.max(0.0);
        let moved = ppm.max(0.0).min(available);
        self.geological_ppm_equiv -= moved;
        *atmosphere_ppm += moved;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planet_generator::PlanetConfig;

    #[test]
    fn carbon_starts_finite_and_exchange_conserves_total() {
        let cfg = PlanetConfig::default();
        let mut atm = cfg.co2_ppm;
        let mut carbon = CarbonState::from_config(&cfg);
        let before = carbon.total_ppm(atm);
        carbon.exchange(&mut atm, 1_000.0, 17.0);
        let after = carbon.total_ppm(atm);
        assert!((before - after).abs() < 1.0e-3, "{before} vs {after}");
    }

    #[test]
    fn frozen_ocean_outgassing_raises_co2_and_conserves_carbon() {
        let cfg = PlanetConfig::default();
        let mut atm = cfg.co2_ppm;
        let mut carbon = CarbonState::from_config(&cfg);
        let before = carbon.total_ppm(atm);
        carbon.exchange(&mut atm, 1_000_000.0, -20.0);
        let after = carbon.total_ppm(atm);
        assert!(atm > cfg.co2_ppm, "CO₂ did not rise: {atm}");
        assert!((before - after).abs() < 1.0e-2, "{before} vs {after}");
    }
}
