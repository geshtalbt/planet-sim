//! Simulation orchestration: time control, history, checkpoints, forks.

use serde::{Deserialize, Serialize};

use crate::experiments::{ExperimentRun, FORK_COLORS};
use crate::history::{EventKind, History};
use crate::planet_generator::PlanetConfig;
use crate::simulation::planet::{PlanetState, StepEvent, format_age};
use crate::time_engine::TimeEngine;

#[derive(Clone, Debug)]
pub struct Checkpoint {
    pub time_years: f64,
    pub label: String,
    pub state: Box<PlanetState>,
    pub run_index: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Simulation {
    pub initial_config: PlanetConfig,
    pub main: ExperimentRun,
    pub forks: Vec<ExperimentRun>,
    pub active: usize,
    pub time: TimeEngine,
    pub next_fork_id: u64,
    #[serde(skip)]
    pub checkpoints: Vec<Checkpoint>,
    #[serde(skip)]
    pub notices: Vec<String>,
    pub last_ice_fraction: f32,
    pub last_temp_logged: f32,
    pub last_sea_logged: f32,
    pub last_tectonic_log: f64,
}

impl Simulation {
    pub fn new(config: PlanetConfig) -> Result<Self, String> {
        let planet = PlanetState::new(config)?;
        let initial_config = planet.config.clone();
        let mut history = History::default();
        history.log(
            0.0,
            EventKind::Milestone,
            "Planet formed",
            format!(
                "{}: radius {:.0} km, {:.1}% water, initial mean temperature {:.1} °C.",
                planet.config.name,
                planet.config.radius_km,
                planet.config.water.surface_water_fraction * 100.0,
                planet.stats.mean_temp_c
            ),
        );
        history.record(
            &planet.stats,
            &planet.config,
            planet.effective_solar_multiplier(),
        );
        let checkpoint = Checkpoint {
            time_years: 0.0,
            label: "Formation".into(),
            state: Box::new(planet.clone()),
            run_index: 0,
        };
        let main = ExperimentRun {
            id: 0,
            label: planet.config.name.clone(),
            color: FORK_COLORS[0],
            planet,
            history,
            created_at_years: 0.0,
            baseline: true,
        };
        let last_ice_fraction = main.planet.stats.glacier_fraction;
        let last_temp_logged = main.planet.stats.mean_temp_c;
        let last_sea_logged = main.planet.stats.sea_level_m;
        Ok(Self {
            initial_config,
            main,
            forks: Vec::new(),
            active: 0,
            time: TimeEngine::default(),
            next_fork_id: 1,
            checkpoints: vec![checkpoint],
            notices: Vec::new(),
            last_ice_fraction,
            last_temp_logged,
            last_sea_logged,
            last_tectonic_log: 0.0,
        })
    }

    pub fn active_run(&self) -> &ExperimentRun {
        if self.active == 0 {
            &self.main
        } else {
            &self.forks[(self.active - 1).min(self.forks.len().saturating_sub(1))]
        }
    }

    pub fn active_run_mut(&mut self) -> &mut ExperimentRun {
        if self.active == 0 {
            &mut self.main
        } else {
            let index = (self.active - 1).min(self.forks.len().saturating_sub(1));
            &mut self.forks[index]
        }
    }

    pub fn active_planet(&self) -> &PlanetState {
        &self.active_run().planet
    }

    pub fn active_planet_mut(&mut self) -> &mut PlanetState {
        &mut self.active_run_mut().planet
    }

    pub fn active_history(&self) -> &History {
        &self.active_run().history
    }

    pub fn active_history_mut(&mut self) -> &mut History {
        &mut self.active_run_mut().history
    }

    pub fn set_active(&mut self, index: usize) {
        if index == 0 || index <= self.forks.len() {
            self.active = index;
        }
    }

    pub fn run_count(&self) -> usize {
        1 + self.forks.len()
    }

    pub fn run_at(&self, index: usize) -> &ExperimentRun {
        if index == 0 {
            &self.main
        } else {
            &self.forks[(index - 1).min(self.forks.len().saturating_sub(1))]
        }
    }

    /// Advances real time and, when enough real time has accumulated,
    /// advances the simulation by the corresponding number of years.
    pub fn advance_realtime(&mut self, real_dt_seconds: f64) -> f64 {
        let years = self.time.take_years(real_dt_seconds);
        if years > 0.0 {
            self.advance_years(years);
        }
        years
    }

    /// Advances the whole simulation by `years` in at most eight slices.
    pub fn advance_years(&mut self, years: f64) {
        if years <= 0.0 {
            return;
        }
        let slices = ((years / 0.02).ceil() as usize).clamp(1, 8);
        let slice = (years / slices as f64) as f32;
        for _ in 0..slices {
            let mut new_notices = Vec::new();
            step_run(&mut self.main, slice, &mut new_notices);
            for fork in &mut self.forks {
                step_run(fork, slice, &mut new_notices);
            }
            self.notices.extend(new_notices);
            // Record history at the slice cadence so a long advance still has
            // intermediate points.
            for run in std::iter::once(&mut self.main).chain(self.forks.iter_mut()) {
                if run.history.should_record(run.planet.time_years) {
                    run.history.record(
                        &run.planet.stats,
                        &run.planet.config,
                        run.planet.effective_solar_multiplier(),
                    );
                }
            }
        }
        if self.notices.len() > 40 {
            self.notices.drain(0..self.notices.len() - 40);
        }
        self.detect_transitions();
        self.checkpoint_if_due();
        self.time.real_accumulator = 0.0;
    }

    fn detect_transitions(&mut self) {
        let stats = &self.main.planet.stats;
        let time = self.main.planet.time_years;
        if self.last_ice_fraction < 0.5 && stats.glacier_fraction >= 0.5 {
            self.main.history.log(
                time,
                EventKind::Climate,
                "Ice age onset",
                format!(
                    "Glacier and sea-ice coverage crossed 50% ({:.0}%). Global mean temperature {:.1} °C.",
                    stats.glacier_fraction * 100.0,
                    stats.mean_temp_c
                ),
            );
        } else if self.last_ice_fraction >= 0.5 && stats.glacier_fraction < 0.5 {
            self.main.history.log(
                time,
                EventKind::Climate,
                "Deglaciation",
                format!(
                    "Ice coverage fell below 50% ({:.0}%). Global mean temperature {:.1} °C.",
                    stats.glacier_fraction * 100.0,
                    stats.mean_temp_c
                ),
            );
        }
        if self.last_ice_fraction < 0.85 && stats.glacier_fraction >= 0.85 {
            self.main.history.log(
                time,
                EventKind::Climate,
                "Runaway glaciation",
                "Ice-albedo feedback pushed ice coverage above 85% of the surface.",
            );
        }
        if (stats.mean_temp_c - self.last_temp_logged).abs() >= 8.0 {
            self.main.history.log(
                time,
                EventKind::Climate,
                "Major climate transition",
                format!(
                    "Global mean temperature moved from {:.1} °C to {:.1} °C.",
                    self.last_temp_logged, stats.mean_temp_c
                ),
            );
            self.last_temp_logged = stats.mean_temp_c;
        }
        if (stats.sea_level_m - self.last_sea_logged).abs() >= 150.0 {
            self.main.history.log(
                time,
                EventKind::Climate,
                "Major sea-level change",
                format!(
                    "Sea level changed from {:+.0} m to {:+.0} m relative to the generation datum.",
                    self.last_sea_logged, stats.sea_level_m
                ),
            );
            self.last_sea_logged = stats.sea_level_m;
        }
        if time - self.last_tectonic_log >= 2.5e8 {
            let t = &self.main.planet.last_tectonic;
            if t.boundary_cells > 0 {
                self.main.history.log(
                    time,
                    EventKind::Tectonic,
                    "Tectonic reorganisation",
                    format!(
                        "{} boundary cells active; mean relative plate motion {:.0} m/Myr. Mountains and trenches continue to evolve.",
                        t.boundary_cells, t.mean_convergence_m_per_myr
                    ),
                );
                self.last_tectonic_log = time;
            }
        }
        self.last_ice_fraction = stats.glacier_fraction;
    }

    fn checkpoint_if_due(&mut self) {
        let time = self.main.planet.time_years;
        let last = self.checkpoints.last().map(|c| c.time_years).unwrap_or(0.0);
        let interval = (time * 0.08).max(1_000.0);
        if time - last < interval {
            return;
        }
        self.push_checkpoint(format_age(time));
    }

    pub fn push_checkpoint(&mut self, label: impl Into<String>) {
        let cp = Checkpoint {
            time_years: self.main.planet.time_years,
            label: label.into(),
            state: Box::new(self.main.planet.clone()),
            run_index: 0,
        };
        self.checkpoints.push(cp);
        if self.checkpoints.len() > 10 {
            self.checkpoints.remove(1);
        }
    }

    /// Restores the closest stored checkpoint. The timeline resolution is the
    /// checkpoint interval; this is not a full per-step rewind.
    pub fn jump_to_checkpoint(&mut self, index: usize) -> Result<(), String> {
        let cp = match self.checkpoints.get(index) {
            Some(cp) => cp.clone(),
            None => return Err("Checkpoint not found".into()),
        };
        self.main.planet = (*cp.state).clone();
        self.main
            .history
            .samples
            .retain(|s| s.time_years <= cp.time_years);
        self.main
            .history
            .events
            .retain(|e| e.time_years <= cp.time_years);
        self.main.history.next_sample_time = cp.time_years;
        self.main.history.log(
            cp.time_years,
            EventKind::User,
            "Timeline restored",
            format!(
                "Restored checkpoint '{}' at {}. Later history was truncated; this becomes a new branch.",
                cp.label,
                format_age(cp.time_years)
            ),
        );
        self.checkpoints.retain(|c| c.time_years <= cp.time_years);
        // Branches contain states from the future that no longer follows from
        // the restored timeline; drop them rather than presenting a false
        // comparison.
        self.forks.clear();
        self.active = 0;
        self.last_ice_fraction = self.main.planet.stats.glacier_fraction;
        self.last_temp_logged = self.main.planet.stats.mean_temp_c;
        self.last_sea_logged = self.main.planet.stats.sea_level_m;
        self.time.paused = true;
        self.time.pending_years = 0.0;
        Ok(())
    }

    pub fn restart(&mut self) -> Result<(), String> {
        let config = self.initial_config.clone();
        let planet = PlanetState::new(config)?;
        let mut history = History::default();
        history.log(
            0.0,
            EventKind::Milestone,
            "Simulation restarted",
            format!(
                "{} regenerated from seed {}.",
                planet.config.name, planet.config.seed
            ),
        );
        history.record(
            &planet.stats,
            &planet.config,
            planet.effective_solar_multiplier(),
        );
        let checkpoint = Checkpoint {
            time_years: 0.0,
            label: "Formation".into(),
            state: Box::new(planet.clone()),
            run_index: 0,
        };
        self.main = ExperimentRun {
            id: 0,
            label: planet.config.name.clone(),
            color: FORK_COLORS[0],
            planet,
            history,
            created_at_years: 0.0,
            baseline: true,
        };
        self.forks.clear();
        self.active = 0;
        self.time = TimeEngine::default();
        self.checkpoints.clear();
        self.checkpoints.push(checkpoint);
        self.notices.clear();
        self.last_ice_fraction = self.main.planet.stats.glacier_fraction;
        self.last_temp_logged = self.main.planet.stats.mean_temp_c;
        self.last_sea_logged = self.main.planet.stats.sea_level_m;
        self.last_tectonic_log = 0.0;
        Ok(())
    }

    /// Creates a full clone of the active run with an independent RNG.
    pub fn create_fork(&mut self, label: impl Into<String>) -> usize {
        let id = self.next_fork_id;
        self.next_fork_id += 1;
        let color = FORK_COLORS[1 + (self.forks.len() % (FORK_COLORS.len() - 1))];
        let source = self.active_run().clone();
        let label = label.into();
        let label = if label.trim().is_empty() {
            format!("Experiment {}", id)
        } else {
            label
        };
        let fork = ExperimentRun::fork_from(id, label, color, &source);
        self.forks.push(fork);
        let index = self.forks.len();
        self.active = index;
        index
    }

    pub fn delete_fork(&mut self, index: usize) {
        if index == 0 || index > self.forks.len() {
            return;
        }
        self.forks.remove(index - 1);
        if self.active >= index {
            self.active = self.active.saturating_sub(1);
        }
        if self.active > self.forks.len() {
            self.active = 0;
        }
    }

    pub fn log_active_event(
        &mut self,
        kind: EventKind,
        title: impl Into<String>,
        description: impl Into<String>,
    ) {
        let time = self.active_planet().time_years;
        let title = title.into();
        self.notices.push(title.clone());
        self.active_history_mut()
            .log(time, kind, title, description);
    }
}

fn step_run(run: &mut ExperimentRun, dt_years: f32, notices: &mut Vec<String>) {
    let events: Vec<StepEvent> = run.planet.step_slice(dt_years);
    for event in events {
        run.history.log(
            run.planet.time_years,
            event.kind,
            event.title.clone(),
            event.description,
        );
        notices.push(format!(
            "{}: {}",
            format_age(run.planet.time_years),
            event.title
        ));
    }
}

impl Simulation {
    /// Repairs derived state after deserialization (topology is rebuilt from
    /// the subdivision level, statistics are recomputed).
    pub fn after_load(&mut self) {
        for run in std::iter::once(&mut self.main).chain(self.forks.iter_mut()) {
            let radius = run.planet.config.radius_m();
            run.planet.grid.ensure_topology(radius);
            run.planet.grid.rebuild_areas(radius);
            run.planet.recompute_stats();
            if run.history.samples.is_empty() {
                run.history.record(
                    &run.planet.stats,
                    &run.planet.config,
                    run.planet.effective_solar_multiplier(),
                );
            }
        }
        if self.active > self.forks.len() {
            self.active = 0;
        }
        if self.checkpoints.is_empty() {
            self.checkpoints.push(Checkpoint {
                time_years: self.main.planet.time_years,
                label: format_age(self.main.planet.time_years),
                state: Box::new(self.main.planet.clone()),
                run_index: 0,
            });
        }
    }
}
