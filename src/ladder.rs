//! `ladder.toml`: correct solutions from a brute force up to the intended one, each with the inputs
//! it handles: larger sizes or special cases. They check each other on random tests, and the
//! confirmed ones become the subtasks.
//!
//! Field docs double as instructions to the model: they end up in the JSON schema.

use crate::claude::{Claude, Effort, Model, Request, Stats};
use crate::task::Task;
use crate::{Error, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub const FILE: &str = "ladder.toml";
const PROMPT: &str = include_str!("../prompts/ladder.md");

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Ladder {
    /// Whether some input has more than one correct output.
    pub multiple_answers: bool,
    pub time_limit_ms: u32,
    pub memory_limit_mb: u32,
    /// From the brute force to the intended solution, in subtask order.
    pub rungs: Vec<Rung>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Rung {
    /// Short and unique, e.g. "brute force", "quadratic".
    pub name: String,
    /// The algorithm, precisely enough to implement without further thought.
    pub approach: String,
    /// E.g. "O(n^2)".
    pub complexity: String,
    /// Every constraint of the inputs it solves, one per entry, e.g. "1 <= n <= 5000" or "all `a_i` are
    /// equal", with the statement's bounds it does not tighten.
    pub constraints: Vec<String>,
    /// Earlier rungs, counted from 0, whose inputs all satisfy these constraints too, so it solves them.
    pub includes: Vec<usize>,
    /// Points of the subtask with these constraints.
    pub points: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Subtask {
    pub points: u32,
    /// Every constraint holding in it.
    pub constraints: Vec<String>,
}

fn input(task: &Task, statement: &str) -> serde_json::Value {
    let config = &task.config;
    serde_json::json!({
        "statement": statement,
        "solution_idea": config.solution.idea,
        "subtasks": config.subtasks,
    })
}

impl Ladder {
    /// One Claude call; the author's limits are enforced afterwards.
    pub fn generate(claude: &Claude, task: &Task, statement: &str) -> Result<(Self, Stats)> {
        let request = Request {
            prompt: PROMPT.to_owned(),
            model: Some(Model::Opus),
            effort: Some(Effort::High),
            ..Default::default()
        };
        let (mut ladder, stats): (Self, _) = claude.ask_json(&request, &input(task, statement))?;
        let limits = &task.config.limits;
        ladder.time_limit_ms = limits.time_ms.unwrap_or(ladder.time_limit_ms);
        ladder.memory_limit_mb = limits.memory_mb.unwrap_or(ladder.memory_limit_mb);
        ladder.check()?;
        Ok((ladder, stats))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        fs::write(path, toml::to_string(self)?).map_err(|source| Error::Io { path: path.to_owned(), source })
    }

    /// The intended solution's rung, the last.
    #[must_use]
    pub fn full(&self) -> usize {
        self.rungs.len() - 1
    }

    /// The rungs whose inputs `rung` solves: itself and those it includes, or all for the last.
    #[must_use]
    pub fn solves(&self, rung: usize) -> Vec<usize> {
        if rung == self.full() {
            return (0..self.rungs.len()).collect();
        }
        let mut solves = self.rungs[rung].includes.clone();
        solves.push(rung);
        solves.sort_unstable();
        solves.dedup();
        solves
    }

    /// One per confirmed rung; a rejected rung's points go to the next confirmed one. `confirmed` must
    /// hold the last rung.
    #[must_use]
    pub fn subtasks(&self, confirmed: &[bool]) -> Vec<Subtask> {
        let mut subtasks = Vec::new();
        let mut points = 0;
        for (rung, &confirmed) in self.rungs.iter().zip(confirmed) {
            points += rung.points;
            if confirmed {
                subtasks.push(Subtask { points, constraints: rung.constraints.clone() });
                points = 0;
            }
        }
        subtasks
    }

    fn check(&self) -> Result<()> {
        if self.rungs.len() < 2 {
            return Err(Error::Invalid("the ladder needs a brute force and the intended solution".to_owned()));
        }
        let mut names = HashSet::new();
        if let Some(rung) = self.rungs.iter().find(|rung| !names.insert(&rung.name)) {
            return Err(Error::Invalid(format!("rung `{}` appears twice", rung.name)));
        }
        for (i, rung) in self.rungs.iter().enumerate() {
            if let Some(j) = rung.includes.iter().find(|&&j| j >= i) {
                return Err(Error::Invalid(format!("rung `{}` includes rung {j}, which is not an earlier one", rung.name)));
            }
        }
        let points: u32 = self.rungs.iter().map(|rung| rung.points).sum();
        if points != 100 {
            return Err(Error::Invalid(format!("the rungs' points sum to {points}, not 100")));
        }
        Ok(())
    }
}
