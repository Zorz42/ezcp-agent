//! Independently generated official C++ solutions, written to `solutions/official/`.

use crate::claude::{Claude, Effort, Model, Request, Stats};
use crate::spec::Spec;
use crate::{Error, Result};
use schemars::JsonSchema;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::thread;

pub const DIR: &str = "solutions/official";
const PROMPT: &str = include_str!("../prompts/solution.md");

#[derive(Deserialize, JsonSchema)]
struct Output {
    /// The complete C++17 source.
    source: String,
}

fn input(spec: &Spec, attempt: u32) -> serde_json::Value {
    serde_json::json!({
        // Only makes the requests, and so their cache entries, distinct.
        "attempt": attempt,
        "title": spec.title,
        "summary": spec.summary,
        "input_format": spec.input_format,
        "output_format": spec.output_format,
        "multiple_answers": spec.multiple_answers,
        "time_limit_ms": spec.time_limit_ms,
        "memory_limit_mb": spec.memory_limit_mb,
        "solution": spec.solution,
        "edge_cases": spec.edge_cases,
        "subtasks": spec.subtasks,
    })
}

/// `count` solutions from parallel calls; fails if any call does, but finished ones stay cached.
pub fn generate(claude: &Claude, spec: &Spec, count: u32) -> Result<Vec<(String, Stats)>> {
    let request = &Request {
        prompt: PROMPT.to_owned(),
        model: Some(Model::Opus),
        effort: Some(Effort::High),
        ..Default::default()
    };
    thread::scope(|scope| {
        let handles: Vec<_> = (1..=count)
            .map(|attempt| scope.spawn(move || claude.ask_json::<Output>(request, &input(spec, attempt))))
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                let (output, stats) = handle.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic))?;
                Ok((output.source, stats))
            })
            .collect()
    })
}

/// Replaces the contents of `dir` with `1.cpp`, `2.cpp`, ...
pub fn save(dir: &Path, sources: &[String]) -> Result<()> {
    let io = |source| Error::Io { path: dir.to_owned(), source };
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(io)?;
    }
    fs::create_dir_all(dir).map_err(io)?;
    for (i, source) in sources.iter().enumerate() {
        let path = dir.join(format!("{}.cpp", i + 1));
        fs::write(&path, source).map_err(|source| Error::Io { path, source })?;
    }
    Ok(())
}
