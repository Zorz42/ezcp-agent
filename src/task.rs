//! A task directory: `task.toml` (settings) and `statement.md`.

use crate::{Error, Result, read};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "task.toml";
pub const STATEMENT_FILE: &str = "statement.md";
const STARTER: &str = include_str!("../templates/task.toml");

/// `task.toml`. Every field is optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub solution: Solution,
    pub subtasks: Vec<Subtask>,
    pub partial: Partial,
    pub limits: Limits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Solution {
    pub idea: Option<String>,
    /// Relative to the task directory.
    pub source: Option<PathBuf>,
    pub attempts: u32,
}

impl Default for Solution {
    fn default() -> Self {
        Self { idea: None, source: None, attempts: 2 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subtask {
    pub points: u32,
    pub constraints: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Partial {
    /// Wrong solutions the model adds to `ideas`; `None` lets it choose.
    pub extra: Option<u32>,
    pub ideas: Vec<PartialIdea>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartialIdea {
    pub idea: String,
    pub passes: Option<Vec<usize>>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub time_ms: Option<u32>,
    pub memory_mb: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub dir: PathBuf,
    pub config: Config,
    pub statement: String,
}

impl Task {
    /// Writes a commented starter `task.toml` and an empty `statement.md`; existing files are kept.
    pub fn init(dir: &Path) -> Result<()> {
        for (name, contents) in [(CONFIG_FILE, STARTER), (STATEMENT_FILE, "")] {
            let path = dir.join(name);
            if !path.exists() {
                fs::write(&path, contents).map_err(|source| Error::Io { path, source })?;
            }
        }
        Ok(())
    }

    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join(CONFIG_FILE);
        let config = toml::from_str(&read(&path)?).map_err(|source| Error::Toml { path, source })?;
        let statement = read(&dir.join(STATEMENT_FILE))?;
        if statement.trim().is_empty() {
            return Err(Error::Invalid(format!("{STATEMENT_FILE} is empty")));
        }
        Ok(Self { dir: dir.to_owned(), config, statement })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn starter_parses() {
        let config: super::Config = toml::from_str(super::STARTER).unwrap();
        assert_eq!(config.solution.attempts, 2);
    }
}
