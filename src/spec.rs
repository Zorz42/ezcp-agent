//! `spec.toml`: the subtasks and the wrong solutions the tests must reject.
//!
//! Field docs double as instructions to the model: they end up in the JSON schema.

use crate::claude::{Claude, Effort, Model, Request, Stats};
use crate::ladder::Ladder;
use crate::task::Task;
use crate::{Error, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub const FILE: &str = "spec.toml";
const PROMPT: &str = include_str!("../prompts/spec.md");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub subtasks: Vec<Subtask>,
    pub partials: Vec<Partial>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtask {
    pub points: u32,
    /// Every constraint holding in it.
    pub constraints: Vec<String>,
}

/// A wrong solution the tests must reject.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Partial {
    /// Short and unique, e.g. "int overflow".
    pub name: String,
    /// What it does and why it fails.
    pub approach: String,
    /// Subtasks, counted from 0, it passes on every valid input. Empty if none.
    pub passes: Vec<usize>,
}

#[derive(Deserialize, JsonSchema)]
struct Output {
    partials: Vec<Partial>,
}

fn input(task: &Task, statement: &str, subtasks: &[Subtask], ladder: &Ladder) -> serde_json::Value {
    let partial = &task.config.partial;
    let covered: Vec<_> = ladder.rungs.iter().map(|rung| format!("{} ({})", rung.name, rung.complexity)).collect();
    serde_json::json!({
        "statement": statement,
        "subtasks": subtasks.iter().map(|subtask| &subtask.constraints).collect::<Vec<_>>(),
        "covered": covered,
        "wrong_approaches": partial.ideas,
        "extra": partial.extra,
    })
}

impl Spec {
    /// One Claude call for the partials; the subtasks are given.
    pub fn generate(claude: &Claude, task: &Task, statement: &str, subtasks: Vec<Subtask>, ladder: &Ladder) -> Result<(Self, Stats)> {
        let request = Request {
            prompt: PROMPT.to_owned(),
            model: Some(Model::Opus),
            effort: Some(Effort::High),
            ..Default::default()
        };
        let (output, stats): (Output, _) = claude.ask_json(&request, &input(task, statement, &subtasks, ladder))?;
        let spec = Self { subtasks, partials: output.partials };
        spec.check()?;
        Ok((spec, stats))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        fs::write(path, toml::to_string(self)?).map_err(|source| Error::Io { path: path.to_owned(), source })
    }

    fn check(&self) -> Result<()> {
        let n = self.subtasks.len();
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
