//! `spec.toml`: the complete problem description every later step works from.
//!
//! Field docs double as instructions to the model: they end up in the JSON schema.

use crate::claude::{Claude, Effort, Model, Request, Stats};
use crate::task::Task;
use crate::{Error, Result, hash};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

pub const FILE: &str = "spec.toml";
const PROMPT: &str = include_str!("../prompts/spec.md");

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Spec {
    pub title: String,
    /// The task restated precisely and completely, without the story.
    pub summary: String,
    /// The exact input layout, line by line.
    pub input_format: String,
    pub output_format: String,
    /// Whether more than one output can be correct, so a checker is needed.
    pub multiple_answers: bool,
    pub time_limit_ms: u32,
    pub memory_limit_mb: u32,
    /// The intended solution for the full problem: algorithm and complexity.
    pub solution: String,
    /// Inputs that tests must cover, e.g. "n = 1", "all values equal".
    pub edge_cases: Vec<String>,
    pub subtasks: Vec<Subtask>,
    pub samples: Vec<Sample>,
    pub partials: Vec<Partial>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Subtask {
    pub points: u32,
    /// Every constraint holding in this subtask, one per entry, e.g. "1 <= n <= 1000".
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Sample {
    pub input: String,
    pub output: String,
}

/// A wrong or too slow solution the tests must reject.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Partial {
    /// Short and unique, e.g. "quadratic", "int overflow".
    pub name: String,
    /// What it does and why it fails.
    pub approach: String,
    /// Subtasks, counted from 0, it passes on every valid input. Empty if none.
    pub passes: Vec<usize>,
}

/// Everything the spec is generated from.
fn input(task: &Task, statement: &str) -> serde_json::Value {
    let config = &task.config;
    serde_json::json!({
        "statement": statement,
        "solution_idea": config.solution.idea,
        "wrong_approaches": config.partial.ideas,
        "extra": config.partial.extra,
    })
}

/// First line of `spec.toml`; ties it to the inputs it was generated from.
fn header(task: &Task, statement: &str) -> String {
    format!(
        "# Generated from {:016x}; regenerated when task.toml or statement.md change.\n",
        hash(&input(task, statement).to_string())
    )
}

impl Spec {
    /// One Claude call; the author's limits and subtask points are enforced afterwards.
    pub fn generate(claude: &Claude, task: &Task, statement: &str) -> Result<(Self, Stats)> {
        let config = &task.config;
        let input = input(task, statement);
        let request = Request {
            prompt: PROMPT.to_owned(),
            model: Some(Model::Opus),
            effort: Some(Effort::High),
            ..Default::default()
        };
        let (mut spec, stats): (Self, _) = claude.ask_json(&request, &input)?;

        if let Some(ms) = config.limits.time_ms {
            spec.time_limit_ms = ms;
        }
        if let Some(mb) = config.limits.memory_mb {
            spec.memory_limit_mb = mb;
        }
        if !config.subtasks.is_empty() {
            let given: Vec<_> = config.subtasks.iter().map(|s| s.points).collect();
            let got: Vec<_> = spec.subtasks.iter().map(|s| s.points).collect();
            if given != got {
                return Err(Error::Invalid(format!("spec has subtask points {got:?}, task.toml {given:?}")));
            }
        }
        spec.check()?;
        Ok((spec, stats))
    }

    /// `None` if the file is missing or was generated from other inputs.
    /// Edits below the header are kept.
    pub fn load(path: &Path, task: &Task, statement: &str) -> Result<Option<Self>> {
        let text = match fs::read_to_string(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            result => result.map_err(|source| Error::Io { path: path.to_owned(), source })?,
        };
        if !text.starts_with(&header(task, statement)) {
            return Ok(None);
        }
        let spec: Self = toml::from_str(&text).map_err(|source| Error::Toml { path: path.to_owned(), source })?;
        spec.check()?;
        Ok(Some(spec))
    }

    pub fn save(&self, path: &Path, task: &Task, statement: &str) -> Result<()> {
        let text = header(task, statement) + &toml::to_string(self)?;
        fs::write(path, text).map_err(|source| Error::Io { path: path.to_owned(), source })
    }

    fn check(&self) -> Result<()> {
        let n = self.subtasks.len();
        if n == 0 {
            return Err(Error::Invalid("spec has no subtasks".to_owned()));
        }
        let mut names = HashSet::new();
        for partial in &self.partials {
            if !names.insert(&partial.name) {
                return Err(Error::Invalid(format!("partial `{}` appears twice", partial.name)));
            }
            if let Some(i) = partial.passes.iter().find(|&&i| i >= n) {
                return Err(Error::Invalid(format!("partial `{}` passes subtask {i}, there are {n}", partial.name)));
            }
            if (0..n).all(|i| partial.passes.contains(&i)) {
                return Err(Error::Invalid(format!("partial `{}` passes every subtask", partial.name)));
            }
        }
        Ok(())
    }
}
