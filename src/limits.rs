//! Two solutions that pin down the time limit: the intended algorithm written plainly, which must pass
//! every subtask, and a slower one micro-optimized, which must fail the full one.

use crate::claude::{Claude, Effort, Model, Request, Stats};
use crate::ladder::{Ladder, Subtask};
use crate::Result;
use schemars::JsonSchema;
use serde::Deserialize;
use std::thread;

const UNOPTIMIZED_PROMPT: &str = include_str!("../prompts/unoptimized.md");
const MICRO_PROMPT: &str = include_str!("../prompts/micro.md");

#[derive(Deserialize, JsonSchema)]
struct Unoptimized {
    /// The complete C++17 source.
    source: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Micro {
    /// E.g. "O(n^2) over a flat array".
    pub approach: String,
    /// Subtasks, counted from 0, it solves within the time limit.
    pub passes: Vec<usize>,
    /// The complete C++17 source.
    pub source: String,
}

pub struct Limits {
    pub unoptimized: String,
    pub micro: Micro,
}

/// Two parallel calls.
pub fn generate(claude: &Claude, statement: &str, official: &str, ladder: &Ladder, subtasks: &[Subtask]) -> Result<(Limits, Vec<Stats>)> {
    let request = |prompt: &str, model, effort| Request { prompt: prompt.to_owned(), model: Some(model), effort: Some(effort), ..Default::default() };
    // The plain one rewrites the official solution; the micro-optimized one needs the judgement.
    let (unoptimized_request, micro_request) = (request(UNOPTIMIZED_PROMPT, Model::Sonnet, Effort::Medium), request(MICRO_PROMPT, Model::Opus, Effort::High));
    let unoptimized_input = serde_json::json!({ "statement": statement, "official_solution": official });
    let slower: Vec<_> = ladder.rungs[..ladder.full()].iter().map(|rung| serde_json::json!({ "approach": rung.approach, "complexity": rung.complexity })).collect();
    let micro_input = serde_json::json!({ "statement": statement, "official_solution": official, "subtasks": subtasks, "slower_approaches": slower });

    let (unoptimized, micro) = thread::scope(|scope| {
        let unoptimized = scope.spawn(|| claude.ask_json::<Unoptimized>(&unoptimized_request, &unoptimized_input));
        let micro = claude.ask_json::<Micro>(&micro_request, &micro_input);
        (unoptimized.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)), micro)
    });
    let ((unoptimized, unoptimized_stats), (micro, micro_stats)) = (unoptimized?, micro?);
    Ok((Limits { unoptimized: unoptimized.source, micro }, vec![unoptimized_stats, micro_stats]))
}
