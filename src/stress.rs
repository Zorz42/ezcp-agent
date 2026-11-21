//! The stress project: the model's EZCP generator in a fixed runner, cross-checking the solutions
//! with EZCP's stress tests.

use crate::cargo::Cargo;
use crate::ladder::Ladder;
use crate::{Error, Result, runner};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const DIR: &str = "stress";
const MAIN: &str = include_str!("../templates/runner/stress.rs");
/// EZCP stress tests per tier and run.
const TESTS: usize = 50;

pub struct Candidate {
    pub name: String,
    pub source: PathBuf,
    /// The tiers it is meant to solve.
    pub tiers: Vec<usize>,
}

/// The runner's output; tiers and candidates are counted from 0.
#[derive(Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
enum Run {
    Passed,
    CandidateFailed { candidate: usize, verdict: String, tier: usize },
    OfficialFailed { verdict: String },
    Error { message: String },
}

pub struct Stress {
    dir: PathBuf,
    executable: PathBuf,
    tiers: usize,
    time_limit_ms: u32,
    memory_limit_mb: u32,
}

impl Stress {
    /// Builds the runner around `generator`. The inner error holds rustc's errors.
    pub fn build(cargo: &Cargo, dir: &Path, generator: &Path, ladder: &Ladder) -> Result<std::result::Result<Self, String>> {
        let main = runner::main_source(MAIN, &[("GENERATOR", generator)])?;
        Ok(runner::build(cargo, dir, DIR, &main)?.map(|executable| Self {
            dir: dir.to_owned(),
            executable,
            tiers: ladder.rungs.len(),
            time_limit_ms: ladder.time_limit_ms,
            memory_limit_mb: ladder.memory_limit_mb,
        }))
    }

    /// EZCP stress tests every tier, checking each candidate meant to solve it against the official
    /// solution: the first full one left. A time-out, memory overrun or crash rejects a candidate; a
    /// wrong answer is a vote against the official solution, which is rejected instead when outvoted.
    /// `rejected` holds each candidate's reason, `None` while it is left. Returns the official solution.
    pub fn check(&self, candidates: &[Candidate], rejected: &mut [Option<String>]) -> Result<usize> {
        loop {
            let left: Vec<_> = (0..candidates.len()).filter(|&i| rejected[i].is_none()).collect();
            let official = *left
                .iter()
                .find(|&&i| candidates[i].tiers.len() == self.tiers)
                .ok_or_else(|| Error::Invalid("no full solution is left".to_owned()))?;
            let others: Vec<_> = left.into_iter().filter(|&i| i != official).collect();
            if self.vote(candidates, official, others, rejected)? {
                return Ok(official);
            }
        }
    }

    /// Whether the official solution held.
    fn vote(&self, candidates: &[Candidate], official: usize, mut others: Vec<usize>, rejected: &mut [Option<String>]) -> Result<bool> {
        let mut wrong = Vec::new();
        loop {
            let left: Vec<_> = others.iter().map(|&i| &candidates[i]).collect();
            match self.run(&candidates[official].source, &left)? {
                Run::Passed => break,
                Run::CandidateFailed { candidate, verdict, tier } => {
                    if candidate >= others.len() {
                        return Err(Error::Invalid(format!("the stress run reported candidate {candidate} of {}", others.len())));
                    }
                    let i = others.remove(candidate);
                    let reason = format!("{verdict} on tier {}", tier + 1);
                    if verdict == "WA" {
                        wrong.push((i, reason));
                    } else {
                        rejected[i] = Some(reason);
                    }
                }
                Run::OfficialFailed { verdict } => {
                    rejected[official] = Some(format!("{verdict} as the official solution"));
                    return Ok(false);
                }
                Run::Error { message } => return Err(Error::Invalid(format!("the stress run failed: {message}"))),
            }
        }
        // The official solution is on the side of those left, and wins ties.
        if wrong.len() > others.len() + 1 {
            rejected[official] = Some("outvoted".to_owned());
            return Ok(false);
        }
        for (i, reason) in wrong {
            rejected[i] = Some(reason);
        }
        Ok(true)
    }

    fn run(&self, official: &Path, candidates: &[&Candidate]) -> Result<Run> {
        let numbers = [self.tiers, TESTS, self.time_limit_ms as usize, self.memory_limit_mb as usize].map(|n| n.to_string());
        let args: Vec<_> = numbers
            .into_iter()
            .chain([official.display().to_string()])
            .chain(candidates.iter().map(|candidate| {
                let tiers: Vec<_> = candidate.tiers.iter().map(ToString::to_string).collect();
                format!("{}:{}", tiers.join(","), candidate.source.display())
            }))
            .collect();
        runner::run(&self.executable, &self.dir, &args)
    }
}
