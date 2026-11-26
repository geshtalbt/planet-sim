//! Versioned, compressed planet saves.
//!
//! Saves use JSON + gzip for portability. Large surface arrays compress well.
//! Topology is not saved; it is deterministically rebuilt from the grid level
//! on load. A format version is stored so future changes can be migrated.

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};

use crate::simulation::Simulation;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveFile {
    pub format_version: u32,
    pub saved_at_unix: u64,
    pub simulation: Simulation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveMeta {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified_unix: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("unsupported save format version {0} (this build supports {FORMAT_VERSION})")]
    UnsupportedVersion(u32),
    #[error("save not found: {0}")]
    NotFound(String),
    #[error("invalid save name")]
    InvalidName,
}

#[derive(Clone, Debug)]
pub struct SaveManager {
    pub dir: PathBuf,
}

impl Default for SaveManager {
    fn default() -> Self {
        let dir = directories::ProjectDirs::from("com", "planetlab", "PlanetSimulator")
            .map(|d| d.data_dir().join("saves"))
            .unwrap_or_else(|| PathBuf::from("saves"));
        Self { dir }
    }
}

impl SaveManager {
    pub fn new_at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn ensure_dir(&self) -> Result<(), SaveError> {
        fs::create_dir_all(&self.dir)?;
        Ok(())
    }

    pub fn save(&self, simulation: &Simulation) -> Result<PathBuf, SaveError> {
        self.save_as(simulation, None)
    }

    pub fn save_as(
        &self,
        simulation: &Simulation,
        name_override: Option<&str>,
    ) -> Result<PathBuf, SaveError> {
        self.ensure_dir()?;
        let name = name_override
            .map(str::to_owned)
            .unwrap_or_else(|| simulation.main.planet.config.name.clone());
        let file_name = sanitize_name(&name)?;
        let path = self.dir.join(format!("{file_name}.psim.gz"));
        let tmp = self.dir.join(format!(".{file_name}.psim.tmp"));
        let saved_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let file = SaveFile {
            format_version: FORMAT_VERSION,
            saved_at_unix,
            simulation: simulation.clone(),
        };
        {
            let output = File::create(&tmp)?;
            let writer = BufWriter::new(output);
            let mut encoder = GzEncoder::new(writer, Compression::default());
            serde_json::to_writer(&mut encoder, &file)?;
            encoder.finish()?.flush()?;
        }
        fs::rename(&tmp, &path)?;
        Ok(path)
    }

    pub fn load(&self, name: &str) -> Result<Simulation, SaveError> {
        let path = self.resolve(name)?;
        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let mut decoder = GzDecoder::new(reader);
        let mut json = Vec::new();
        decoder.read_to_end(&mut json)?;
        let mut save: SaveFile = serde_json::from_slice(&json)?;
        if save.format_version > FORMAT_VERSION {
            return Err(SaveError::UnsupportedVersion(save.format_version));
        }
        if save.format_version < FORMAT_VERSION {
            migrate(&mut save)?;
        }
        let mut simulation = save.simulation;
        simulation.after_load();
        Ok(simulation)
    }

    pub fn load_path(&self, path: &Path) -> Result<Simulation, SaveError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut decoder = GzDecoder::new(reader);
        let mut json = Vec::new();
        decoder.read_to_end(&mut json)?;
        let mut save: SaveFile = serde_json::from_slice(&json)?;
        if save.format_version > FORMAT_VERSION {
            return Err(SaveError::UnsupportedVersion(save.format_version));
        }
        if save.format_version < FORMAT_VERSION {
            migrate(&mut save)?;
        }
        let mut simulation = save.simulation;
        simulation.after_load();
        Ok(simulation)
    }

    pub fn list(&self) -> Vec<SaveMeta> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("gz") {
                continue;
            }
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".psim.gz"))
                .unwrap_or(false)
            {
                continue;
            }
            let meta = entry.metadata().ok();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.trim_end_matches(".psim.gz").to_string())
                .unwrap_or_else(|| "planet".into());
            let modified_unix = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            out.push(SaveMeta {
                name,
                path,
                size_bytes: meta.map(|m| m.len()).unwrap_or(0),
                modified_unix,
            });
        }
        out.sort_by(|a, b| b.modified_unix.cmp(&a.modified_unix));
        out
    }

    pub fn delete(&self, name: &str) -> Result<(), SaveError> {
        let path = self.resolve(name)?;
        fs::remove_file(path)?;
        Ok(())
    }

    pub fn rename(&self, name: &str, new_name: &str) -> Result<PathBuf, SaveError> {
        let mut simulation = self.load(name)?;
        simulation.main.label = new_name.to_string();
        simulation.main.planet.config.name = new_name.to_string();
        let path = self.save_as(&simulation, Some(new_name))?;
        let old = self.resolve(name)?;
        if old != path {
            let _ = self.delete(name);
        }
        Ok(path)
    }

    pub fn duplicate(&self, name: &str, copy_name: &str) -> Result<PathBuf, SaveError> {
        let mut simulation = self.load(name)?;
        simulation.main.label = copy_name.to_string();
        simulation.main.planet.config.name = copy_name.to_string();
        self.save_as(&simulation, Some(copy_name))
    }

    fn resolve(&self, name: &str) -> Result<PathBuf, SaveError> {
        let direct = PathBuf::from(name);
        if direct.exists() {
            return Ok(direct);
        }
        let sanitized = sanitize_name(name)?;
        let path = self.dir.join(format!("{sanitized}.psim.gz"));
        if path.exists() {
            Ok(path)
        } else {
            Err(SaveError::NotFound(name.into()))
        }
    }
}

fn sanitize_name(name: &str) -> Result<String, SaveError> {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ' ' {
            out.push(if ch == ' ' { '_' } else { ch });
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        return Err(SaveError::InvalidName);
    }
    Ok(out.chars().take(80).collect())
}

fn migrate(save: &mut SaveFile) -> Result<(), SaveError> {
    // Version 1 is the initial format. Future migrations switch on
    // `save.format_version` here.
    save.format_version = FORMAT_VERSION;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planet_generator::PlanetConfig;

    #[test]
    fn save_roundtrip_restores_state() {
        let mut config = PlanetConfig::default();
        config.grid_level = 2;
        config.name = "Roundtrip".into();
        let mut sim = Simulation::new(config).expect("sim");
        sim.advance_years(3.0);
        let before = sim.main.planet.stats.mean_temp_c;
        let time = sim.main.planet.time_years;

        let dir = std::env::temp_dir().join(format!("planet-sim-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let manager = SaveManager::new_at(&dir);
        let path = manager.save_as(&sim, Some("roundtrip")).expect("save");
        assert!(path.exists());
        let loaded = manager.load("roundtrip").expect("load");
        assert!((loaded.main.planet.time_years - time).abs() < 1.0e-9);
        assert!((loaded.main.planet.stats.mean_temp_c - before).abs() < 1.0e-4);
        assert!(loaded.main.planet.grid.cell_count() > 0);
        let _ = fs::remove_dir_all(&dir);
    }
}
