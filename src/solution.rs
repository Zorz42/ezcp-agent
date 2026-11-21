//! C++ solutions written from the statement: independent attempts at the full task, and the ladder's
//! slower rungs.

use crate::claude::{Claude, Effort, Model, Request, Stats};
use crate::ladder::Ladder;
use crate::{Error, Result};
use schemars::JsonSchema;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

pub const DIR: &str = "solutions";
const PROMPT: &str = include_str!("../prompts/solution.md");

#[derive(Deserialize, JsonSchema)]
struct Output {
    /// The complete C++17 source.
    source: String,
}

pub struct Solutions {
    /// Independent attempts at the full task.
    pub attempts: Vec<String>,
    /// One per rung below the intended one, in ladder order.
    pub rungs: Vec<String>,
}

/// `attempts` full solutions and the lower rungs, from parallel calls. Fails if any call does, but
/// finished ones stay cached.
pub fn generate(claude: &Claude, statement: &str, idea: Option<&str>, ladder: &Ladder, attempts: u32) -> Result<(Solutions, Vec<Stats>)> {
    // A rung's approach is given: implementing it needs less than solving the task.
    let request = |model| Request { prompt: PROMPT.to_owned(), model: Some(model), effort: Some(Effort::High), ..Default::default() };
    let (attempt, rung) = (&request(Model::Opus), &request(Model::Sonnet));
    let inputs: Vec<_> = (1..=attempts)
        // `attempt` only makes the requests, and so their cache entries, distinct.
        .map(|number| (attempt, serde_json::json!({ "statement": statement, "idea": idea, "attempt": number })))
        .chain(ladder.rungs[..ladder.full()].iter().map(|step| {
            (rung, serde_json::json!({ "statement": statement, "approach": step.approach, "constraints": step.constraints }))
        }))
        .collect();

    let results: Vec<(String, Stats)> = thread::scope(|scope| {
        let handles: Vec<_> = inputs.iter().map(|input| scope.spawn(move || claude.ask_json::<Output>(input.0, &input.1))).collect();
        handles
            .into_iter()
            .map(|handle| {
                let (output, stats) = handle.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic))?;
                Ok((output.source, stats))
            })
            .collect::<Result<_>>()
    })?;
    let (mut sources, stats): (Vec<_>, Vec<_>) = results.into_iter().unzip();
    let rungs = sources.split_off(attempts as usize);
    Ok((Solutions { attempts: sources, rungs }, stats))
}

impl Solutions {
    /// Replaces `dir` with `attempts/1.cpp, ...` and `ladder/1.cpp, ...`, returning their paths in
    /// that order.
    pub fn save(&self, dir: &Path) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
        let io = |path: &Path| {
            let path = path.to_owned();
            move |source| Error::Io { path, source }
        };
        if dir.exists() {
            fs::remove_dir_all(dir).map_err(io(dir))?;
        }
        let write = |subdir: &str, sources: &[String]| {
            let subdir = dir.join(subdir);
            fs::create_dir_all(&subdir).map_err(io(&subdir))?;
            sources
                .iter()
                .enumerate()
                .map(|(i, source)| {
                    let path = subdir.join(format!("{}.cpp", i + 1));
                    fs::write(&path, source).map_err(io(&path))?;
                    Ok(path)
                })
                .collect::<Result<Vec<_>>>()
        };
        Ok((write("attempts", &self.attempts)?, write("ladder", &self.rungs)?))
    }
}
