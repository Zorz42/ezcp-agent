//! Wrong solutions, each aimed at one subtask, which must catch it: slight mistakes, as edits to the
//! subtask's solution, and the wrong approaches contestants are likely to try, written in full.

use crate::claude::{Claude, Effort, Model, Request, Stats};
use crate::ladder::{Ladder, Subtask};
use crate::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const PROMPT: &str = include_str!("../prompts/wrong.md");
const APPROACHES_PROMPT: &str = include_str!("../prompts/approaches.md");
const APPROACH_PROMPT: &str = include_str!("../prompts/wrong_approach.md");
/// Wrong solutions per call; later calls are told the mistakes taken.
const BATCH: u32 = 10;

#[derive(Deserialize, JsonSchema)]
struct Output {
    solutions: Vec<Wrong>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Wrong {
    /// The mistake, short and unique, e.g. "int overflow in the prefix sums".
    pub mistake: String,
    /// Applied in order.
    pub edits: Vec<Edit>,
    /// Subtasks, counted from 0, where no valid input triggers the mistake.
    pub passes: Vec<usize>,
}

/// A correct solution of a subtask, to make slight mistakes in.
pub struct Base<'a> {
    pub subtask: usize,
    pub source: &'a str,
    /// The subtasks it solves.
    pub passes: &'a [usize],
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Edit {
    /// Exact text occurring once in the source.
    pub find: String,
    pub replace: String,
}

impl Wrong {
    /// `None` if an edit's text does not occur exactly once.
    #[must_use]
    pub fn apply(&self, official: &str) -> Option<String> {
        let mut source = official.to_owned();
        for edit in &self.edits {
            if source.matches(&edit.find).count() != 1 {
                return None;
            }
            source = source.replacen(&edit.find, &edit.replace, 1);
        }
        Some(source)
    }
}

/// `count` per base: sequential calls of up to [`BATCH`] for each, the bases in parallel.
pub fn mutants(claude: &Claude, statement: &str, subtasks: &[Subtask], bases: &[Base], count: u32) -> Result<(Vec<Vec<Wrong>>, Vec<Stats>)> {
    let request = &opus(PROMPT);
    let results: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = bases.iter().map(|base| scope.spawn(move || mutants_of(claude, request, statement, subtasks, base, count))).collect();
        handles.into_iter().map(|handle| handle.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic))).collect::<Result<_>>()
    })?;
    let (wrong, stats): (Vec<_>, Vec<_>) = results.into_iter().unzip();
    Ok((wrong, stats.concat()))
}

fn mutants_of(claude: &Claude, request: &Request, statement: &str, subtasks: &[Subtask], base: &Base, count: u32) -> Result<(Vec<Wrong>, Vec<Stats>)> {
    let (mut wrong, mut stats) = (Vec::<Wrong>::new(), Vec::new());
    let mut left = count;
    while left > 0 {
        let batch = left.min(BATCH);
        let taken: Vec<_> = wrong.iter().map(|wrong| &wrong.mistake).collect();
        let input = serde_json::json!({
            "statement": statement,
            "subtasks": subtasks,
            "target_subtask": base.subtask,
            "solution": base.source,
            "solution_passes": base.passes,
            "count": batch,
            "taken": taken,
        });
        let (output, call): (Output, _) = claude.ask_json(request, &input)?;
        wrong.extend(output.solutions.into_iter().take(batch as usize));
        stats.push(call);
        left -= batch;
    }
    Ok((wrong, stats))
}

#[derive(Deserialize, JsonSchema)]
struct Approaches {
    approaches: Vec<Approach>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Approach {
    /// Short and unique, e.g. "greedy by earliest deadline".
    pub name: String,
    /// Counted from 0: the one whose inputs it is meant for, and must fail.
    pub subtask: usize,
    pub approach: String,
    /// Where its answer is wrong, ideally a small counterexample.
    pub fails_on: String,
    /// Subtasks, counted from 0, where it is correct on every input.
    pub passes: Vec<usize>,
}

#[derive(Deserialize, JsonSchema)]
struct Source {
    /// The complete C++17 source.
    source: String,
}

fn opus(prompt: &str) -> Request {
    Request {
        prompt: prompt.to_owned(),
        model: Some(Model::Opus),
        effort: Some(Effort::High),
        ..Default::default()
    }
}

/// Every probable wrong approach to every subtask, from one call.
pub fn approaches(claude: &Claude, statement: &str, official: &str, ladder: &Ladder, subtasks: &[Subtask]) -> Result<(Vec<Approach>, Stats)> {
    let correct: Vec<_> = ladder.rungs.iter().map(|rung| &rung.approach).collect();
    let input = serde_json::json!({
        "statement": statement,
        "official_solution": official,
        "subtasks": subtasks,
        "correct_approaches": correct,
    });
    let (output, stats): (Approaches, _) = claude.ask_json(&opus(APPROACHES_PROMPT), &input)?;
    Ok((output.approaches, stats))
}

/// One source per approach, from parallel calls.
pub fn write(claude: &Claude, statement: &str, subtasks: &[Subtask], approaches: &[Approach]) -> Result<(Vec<String>, Vec<Stats>)> {
    let request = &opus(APPROACH_PROMPT);
    let inputs: Vec<_> = approaches
        .iter()
        .map(|approach| {
            let constraints = subtasks.get(approach.subtask).map(|subtask| &subtask.constraints);
            serde_json::json!({ "statement": statement, "approach": approach.approach, "subtask_constraints": constraints })
        })
        .collect();
    let results: Vec<(String, Stats)> = std::thread::scope(|scope| {
        let handles: Vec<_> = inputs.iter().map(|input| scope.spawn(move || claude.ask_json::<Source>(request, input))).collect();
        handles
            .into_iter()
            .map(|handle| {
                let (output, stats) = handle.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic))?;
                Ok((output.source, stats))
            })
            .collect::<Result<_>>()
    })?;
    Ok(results.into_iter().unzip())
}
