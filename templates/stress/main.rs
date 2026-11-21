//! Written by ezcp-agent. EZCP's stress tests of every tier, checking the candidates against the
//! official solution; no tests are kept.
//!
//! Arguments: tiers, stress tests per tier, time limit in ms, memory limit in MB, the official
//! solution's source, then each candidate as `<tiers it solves, comma-separated>:<source>`.
//! Prints one [`Run`] as JSON.

mod generator;

use ezcp::{Error, Mode, Subtask, Task};
use serde::Serialize;
use std::path::Path;
use std::{env, fs};

/// Tiers and candidates are counted from 0.
#[derive(Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
enum Run {
    Passed,
    CandidateFailed { candidate: usize, verdict: String, tier: usize },
    OfficialFailed { verdict: String },
    Error { message: String },
}

fn main() {
    let run = run().unwrap_or_else(|message| Run::Error { message });
    println!("{}", serde_json::to_string(&run).expect("a run serializes"));
}

fn run() -> Result<Run, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let [tiers, tests, time_ms, memory_mb, official, candidates @ ..] = args.as_slice() else {
        return Err("expected: tiers tests time_ms memory_mb official tiers:candidate...".to_owned());
    };
    let number = |text: &str| text.parse::<usize>().map_err(|err| format!("{text}: {err}"));
    let read = |path: &str| fs::read_to_string(path).map_err(|err| format!("{path}: {err}"));

    let tests = number(tests)?;
    let mut task = Task::new("stress", Path::new("run"))
        .with_solution_source(&read(official)?)
        .with_time_limit(number(time_ms)? as i32)
        .with_memory_limit(number(memory_mb)?)
        // With no tests kept there is nothing to hunt.
        .with_min_failures(0);
    for tier in 0..number(tiers)? {
        task = task.with_subtask(
            Subtask::new(1, &format!("tier {}", tier + 1))
                .with_test(0, move |rng| generator::generate(rng, tier))
                .with_stress_tests(tests)
                .with_validator(move |input: &generator::Input| generator::validate(input, tier)),
        );
    }
    for (i, candidate) in candidates.iter().enumerate() {
        let (tiers, source) = candidate.split_once(':').ok_or_else(|| format!("{candidate}: expected tiers:source"))?;
        let passes = tiers.split(',').map(number).collect::<Result<Vec<_>, _>>()?;
        task = task.with_partial_solution(&i.to_string(), &read(source)?, &passes);
    }

    let official = |verdict: &str| Run::OfficialFailed { verdict: verdict.to_owned() };
    Ok(match task.run_mode(Mode::Files) {
        Ok(()) => Run::Passed,
        Err(Error::PartialSolutionFailsSubtask { partial_number, verdict, subtask_number, .. }) => {
            Run::CandidateFailed { candidate: partial_number - 1, verdict, tier: subtask_number - 1 }
        }
        Err(Error::SolutionTimedOut { .. }) => official("TLE"),
        Err(Error::SolutionOutOfMemory { .. }) => official("MLE"),
        Err(Error::SolutionCrash { .. }) => official("RTE"),
        Err(err) => return Err(err.to_string()),
    })
}
