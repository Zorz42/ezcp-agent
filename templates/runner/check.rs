//! Written by ezcp-agent. Runs the EZCP project's task as its own binary would, reporting the outcome.
//!
//! Arguments: the project's directory, then `files` or `seeds`. Prints one [`Run`] as JSON.

#[path = {GENERATOR}]
mod generator;
#[path = {TASK}]
mod task;
#[path = {TESTS}]
mod tests;

use ezcp::{Error, Mode};
use serde::Serialize;
use std::env;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

/// Solutions are the partial ones in declaration order; they and subtasks are counted from 0.
#[derive(Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
enum Run {
    Passed,
    /// A solution meant to pass the subtask failed it.
    Fails { solution: usize, subtask: usize, verdict: String },
    /// A solution meant to fail the subtask passed it.
    Passes { solution: usize, subtask: usize },
    OfficialFailed { verdict: String },
    /// A generator or validator panicked.
    Panicked { message: String },
    Error { message: String },
}

fn main() {
    let run = run().unwrap_or_else(|message| Run::Error { message });
    println!("{}", serde_json::to_string(&run).expect("a run serializes"));
}

fn run() -> Result<Run, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let [dir, mode] = args.as_slice() else {
        return Err("expected: dir files|seeds".to_owned());
    };
    let mode = match mode.as_str() {
        "files" => Mode::Files,
        "seeds" => Mode::Seeds,
        _ => return Err(format!("{mode}: expected files or seeds")),
    };
    panic::set_hook(Box::new(|_| {}));
    let result = panic::catch_unwind(AssertUnwindSafe(|| task::task(Path::new(dir)).run_mode(mode)));
    let official = |verdict: &str| Run::OfficialFailed { verdict: verdict.to_owned() };
    Ok(match result {
        Err(payload) => {
            let message = payload.downcast_ref::<String>().cloned().or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()));
            Run::Panicked { message: message.unwrap_or_else(|| "a panic without a message".to_owned()) }
        }
        Ok(Ok(())) => Run::Passed,
        Ok(Err(Error::PartialSolutionFailsSubtask { partial_number, subtask_number, verdict, .. })) => {
            Run::Fails { solution: partial_number - 1, subtask: subtask_number - 1, verdict }
        }
        Ok(Err(Error::PartialSolutionPassesExtraSubtask { partial_number, subtask_number, .. })) => {
            Run::Passes { solution: partial_number - 1, subtask: subtask_number - 1 }
        }
        Ok(Err(Error::SolutionTimedOut { .. })) => official("TLE"),
        Ok(Err(Error::SolutionOutOfMemory { .. })) => official("MLE"),
        Ok(Err(Error::SolutionCrash { .. })) => official("RTE"),
        Ok(Err(Error::SolutionFailed { .. })) => official("WA"),
        Ok(Err(err)) => return Err(err.to_string()),
    })
}
