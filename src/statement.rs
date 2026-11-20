//! `statement.md`: the full statement, written from the description in `task.toml`.

use crate::claude::{Claude, Effort, Model, Request, Stats};
use crate::task::Task;
use crate::{Error, Result, hash};
use schemars::JsonSchema;
use serde::Deserialize;
use std::fs;
use std::io;
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

/// First line of `statement.md`; ties it to the inputs it was written from.
fn header(task: &Task) -> String {
    format!(
        "<!-- Generated from {:016x}; regenerated when the description or settings in task.toml change. -->\n",
        hash(&input(task).to_string())
    )
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

/// The statement without its header; `None` if missing or written from other inputs than `task`'s.
/// Edits below the header are kept.
pub fn load(path: &Path, task: &Task) -> Result<Option<String>> {
    let text = match fs::read_to_string(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        result => result.map_err(|source| Error::Io { path: path.to_owned(), source })?,
    };
    Ok(text.strip_prefix(&header(task)).map(str::to_owned))
}

pub fn save(path: &Path, task: &Task, statement: &str) -> Result<()> {
    fs::write(path, header(task) + statement).map_err(|source| Error::Io { path: path.to_owned(), source })
}
