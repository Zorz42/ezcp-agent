//! `tests.rs` of the EZCP project: the tests of each subtask, aimed at the solutions they must break.

use crate::claude::{Claude, Effort, Model, Request, Stats};
use crate::ladder::Subtask;
use crate::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const PROMPT: &str = include_str!("../prompts/tests.md");
const EZCP: &str = include_str!("../prompts/ezcp.md");

#[derive(Deserialize, JsonSchema)]
struct Output {
    /// The complete `tests.rs`.
    source: String,
}

/// A solution the tests must break.
#[derive(Serialize)]
pub struct Target {
    /// Its approach or mistake.
    pub description: String,
    /// For a wrong solution: the edits that made it from the official one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edits: Option<serde_json::Value>,
    /// Subtasks, counted from 0.
    pub fails: Vec<usize>,
}

/// `previous` is an earlier source and the errors it caused, to be fixed.
pub fn generate(claude: &Claude, statement: &str, generator: &str, subtasks: &[Subtask], targets: &[Target], previous: Option<(&str, &str)>) -> Result<(String, Stats)> {
    let request = Request {
        prompt: "Write tests.rs.".to_owned(),
        system: Some(format!("{PROMPT}{EZCP}")),
        model: Some(Model::Opus),
        effort: Some(Effort::High),
    };
    let subtasks: Vec<_> = subtasks.iter().map(|subtask| serde_json::json!({ "tier": subtask.tier, "constraints": subtask.constraints })).collect();
    let input = serde_json::json!({
        "statement": statement,
        "generator_rs": generator,
        "subtasks": subtasks,
        "solutions_to_break": targets,
        "previous_source": previous.map(|(source, _)| source),
        "errors": previous.map(|(_, errors)| errors),
    });
    let (output, stats): (Output, _) = claude.ask_json(&request, &input)?;
    Ok((output.source, stats))
}
