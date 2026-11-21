//! Compile errors fixed in place by a cheaper model, with edits that change nothing else.

use crate::claude::{Claude, Effort, Model, Request, Stats};
use crate::edit::{self, Edit};
use crate::{Result, read, runner};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::Path;

const PROMPT: &str = include_str!("../prompts/fix.md");
const EZCP: &str = include_str!("../prompts/ezcp.md");
/// Calls before giving up on a source.
const TRIES: usize = 3;

#[derive(Debug, Clone, Copy)]
pub enum Language {
    Cpp,
    /// A module of a crate using EZCP.
    Rust,
}

#[derive(Deserialize, JsonSchema)]
struct Output {
    /// Applied in order.
    edits: Vec<Edit>,
}

/// Edits `path` until `rebuild` succeeds, starting from its `errors`. `others` are the sources it is
/// built with, by name, which stay as they are. Returns the build, or the last errors.
pub fn compile<T>(
    claude: &Claude,
    language: Language,
    path: &Path,
    others: &[(&str, &str)],
    mut errors: String,
    mut rebuild: impl FnMut() -> Result<std::result::Result<T, String>>,
) -> Result<(std::result::Result<T, String>, Vec<Stats>)> {
    let (system, name) = match language {
        Language::Cpp => (PROMPT.to_owned(), "C++17"),
        Language::Rust => (format!("{PROMPT}{EZCP}"), "Rust"),
    };
    let request = Request { prompt: "Fix the compile errors.".to_owned(), system: Some(system), model: Some(Model::Sonnet), effort: Some(Effort::Medium) };
    let others: serde_json::Map<_, _> = others.iter().map(|(name, source)| ((*name).to_owned(), (*source).into())).collect();
    let mut stats = Vec::new();
    for _ in 0..TRIES {
        let source = read(path)?;
        let input = serde_json::json!({ "language": name, "source": source, "errors": errors, "other_files": others });
        let (output, call): (Output, _) = claude.ask_json(&request, &input)?;
        stats.push(call);
        let Some(fixed) = edit::apply(&source, &output.edits) else {
            errors.push_str("\n\nThe last edits did not apply: each find must occur exactly once in the source.");
            continue;
        };
        runner::write(path, &fixed)?;
        match rebuild()? {
            Ok(done) => return Ok((Ok(done), stats)),
            Err(new) => errors = new,
        }
    }
    Ok((Err(errors), stats))
}
