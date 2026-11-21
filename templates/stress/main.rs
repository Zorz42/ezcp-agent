//! Written by ezcp-agent. EZCP's stress tests of every tier, checking the candidates against the
//! official solution; no tests are kept.
//!
//! Arguments: tiers, stress tests per tier, time limit in ms, memory limit in MB, the official
//! solution's source, then each candidate as `<tiers it solves, comma-separated>:<source>`.
//! Prints one line: `passed`, `candidate <index> <verdict> <tier>`, `official <verdict>` or `error <message>`.

mod generator;

use ezcp::{Error, Mode, Subtask, Task};
use std::path::Path;
use std::{env, fs};

fn main() {
    println!("{}", run().unwrap_or_else(|message| format!("error {message}")));
}

fn run() -> Result<String, String> {
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

    Ok(match task.run_mode(Mode::Files) {
        Ok(()) => "passed".to_owned(),
        Err(Error::PartialSolutionFailsSubtask { partial_number, verdict, subtask_number, .. }) => {
            format!("candidate {} {verdict} {}", partial_number - 1, subtask_number - 1)
        }
        Err(Error::SolutionTimedOut { .. }) => "official TLE".to_owned(),
        Err(Error::SolutionOutOfMemory { .. }) => "official MLE".to_owned(),
        Err(Error::SolutionCrash { .. }) => "official RTE".to_owned(),
        Err(err) => return Err(err.to_string()),
    })
}
