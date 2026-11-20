//! `statement.md`: the full statement, written from the description in `task.toml`.

use crate::claude::{Claude, Effort, Model, Request, Stats};
use crate::task::Task;
use crate::{Error, Result};
use schemars::JsonSchema;
use serde::Deserialize;
use std::fs;
use std::path::Path;

pub const FILE: &str = "statement.md";
const PROMPT: &str = include_str!("../prompts/statement.md");
const EXAMPLE: &str = include_str!("../templates/statement_example.md");

#[derive(Deserialize, JsonSchema)]
struct Output {
    /// The complete statement in Markdown.
    statement: String,
}

fn input(task: &Task) -> serde_json::Value {
    let config = &task.config;
    serde_json::json!({
        "description": config.description,
        "subtasks": config.subtasks,
        "samples": config.samples,
        "solution_idea": config.solution.idea,
        "limits": config.limits,
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

pub fn save(path: &Path, statement: &str) -> Result<()> {
    fs::write(path, statement).map_err(|source| Error::Io { path: path.to_owned(), source })
}
