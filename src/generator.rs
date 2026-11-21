//! The EZCP generator of the stress project: random inputs for each rung's constraints.

use crate::claude::{Claude, Effort, Model, Request, Stats};
use crate::ladder::Ladder;
use crate::Result;
use schemars::JsonSchema;
use serde::Deserialize;

const PROMPT: &str = include_str!("../prompts/generator.md");
const EZCP: &str = include_str!("../prompts/ezcp.md");

#[derive(Deserialize, JsonSchema)]
struct Output {
    /// The complete `generator.rs`.
    source: String,
}

/// `previous` is an earlier source and its compiler errors, to be fixed.
pub fn generate(claude: &Claude, statement: &str, ladder: &Ladder, previous: Option<(&str, &str)>) -> Result<(String, Stats)> {
    let request = Request {
        prompt: "Write generator.rs.".to_owned(),
        system: Some(format!("{PROMPT}{EZCP}")),
        model: Some(Model::Opus),
        effort: Some(Effort::High),
    };
    let tiers: Vec<_> = ladder.rungs.iter().map(|rung| &rung.constraints).collect();
    let input = serde_json::json!({
        "statement": statement,
        "tiers": tiers,
        "previous_source": previous.map(|(source, _)| source),
        "compiler_errors": previous.map(|(_, errors)| errors),
    });
    let (output, stats): (Output, _) = claude.ask_json(&request, &input)?;
    Ok((output.source, stats))
}
