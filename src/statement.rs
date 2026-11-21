//! `statement.md`: the full statement, written from the description in `task.toml`. Unless the author
//! gave subtasks, it is first written without them, with [`PLACEHOLDER`] in their place.

use crate::claude::{Claude, Effort, Model, Request, Stats};
use crate::ladder::Subtask;
use crate::task::Task;
use crate::{Error, Result};
use schemars::JsonSchema;
use serde::Deserialize;
use std::fs;
use std::path::Path;

pub const FILE: &str = "statement.md";
pub const PLACEHOLDER: &str = "{{SUBTASKS}}";
const PROMPT: &str = include_str!("../prompts/statement.md");
const SUBTASKS_PROMPT: &str = include_str!("../prompts/subtasks.md");
const EXAMPLE: &str = include_str!("../templates/statement_example.md");

#[derive(Deserialize, JsonSchema)]
struct Output {
    /// The complete statement in Markdown.
    statement: String,
}

#[derive(Deserialize, JsonSchema)]
struct SectionOutput {
    /// The subtasks section in Markdown.
    section: String,
}

fn input(task: &Task) -> serde_json::Value {
    let config = &task.config;
    serde_json::json!({
        "description": config.description,
        "subtasks": config.subtasks,
        "samples": config.samples,
        "solution_idea": config.solution.idea,
    })
}

pub fn generate(claude: &Claude, task: &Task) -> Result<(String, Stats)> {
    let request = Request {
        prompt: "Write the statement.".to_owned(),
        system: Some(format!("{PROMPT}{EXAMPLE}")),
        model: Some(Model::Opus),
        effort: Some(Effort::High),
    };
    let (output, stats): (Output, _) = claude.ask_json(&request, &input(task))?;
    Ok((output.statement, stats))
}

/// Without the placeholder, as solvers see it.
#[must_use]
pub fn strip(statement: &str) -> String {
    statement.replace(PLACEHOLDER, "")
}

/// Writes the subtasks section in place of the placeholder; the rest stays as it is.
pub fn fill(claude: &Claude, statement: &str, subtasks: &[Subtask]) -> Result<(String, Stats)> {
    if !statement.contains(PLACEHOLDER) {
        return Err(Error::Invalid(format!("{FILE} has no {PLACEHOLDER} line for the subtasks")));
    }
    let request = Request {
        prompt: "Write the subtasks section.".to_owned(),
        system: Some(format!("{SUBTASKS_PROMPT}{EXAMPLE}")),
        model: Some(Model::Sonnet),
        effort: Some(Effort::Medium),
    };
    let input = serde_json::json!({ "statement": statement, "subtasks": subtasks });
    let (output, stats): (SectionOutput, _) = claude.ask_json(&request, &input)?;
    Ok((statement.replacen(PLACEHOLDER, output.section.trim(), 1), stats))
}

pub fn save(path: &Path, statement: &str) -> Result<()> {
    fs::write(path, statement).map_err(|source| Error::Io { path: path.to_owned(), source })
}
