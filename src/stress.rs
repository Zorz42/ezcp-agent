//! The stress project: the model's EZCP generator in a fixed runner, cross-checking the solutions
//! with EZCP's stress tests.

use crate::cargo::Cargo;
use crate::ladder::Ladder;
use crate::{Error, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const DIR: &str = "stress";
const MANIFEST: &str = include_str!("../templates/stress/Cargo.toml");
const MAIN: &str = include_str!("../templates/stress/main.rs");
/// EZCP stress tests per tier and run.
const TESTS: usize = 50;

pub struct Candidate {
    pub name: String,
    pub source: PathBuf,
    /// The tiers it is meant to solve.
    pub tiers: Vec<usize>,
}

enum Run {
    Passed,
    CandidateFailed { index: usize, verdict: String, tier: usize },
    OfficialFailed(String),
}

pub struct Stress {
    dir: PathBuf,
    executable: PathBuf,
    tiers: usize,
    time_limit_ms: u32,
    memory_limit_mb: u32,
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Error {
    let path = path.to_owned();
    move |source| Error::Io { path, source }
}

/// `$EZCP_PATH`, else the repository.
fn ezcp_dependency() -> String {
    match std::env::var("EZCP_PATH") {
        Ok(path) if !path.is_empty() => format!("{{ path = {} }}", toml::Value::String(path)),
        _ => r#"{ git = "https://github.com/Zorz42/EZCP" }"#.to_owned(),
    }
}

impl Stress {
    /// Writes the project around `generator` and builds it. The inner error holds rustc's errors.
    pub fn build(cargo: &Cargo, dir: &Path, generator: &str, ladder: &Ladder) -> Result<std::result::Result<Self, String>> {
        let src = dir.join("src");
        fs::create_dir_all(&src).map_err(io(&src))?;
        let manifest = dir.join("Cargo.toml");
        let files = [
            (manifest.clone(), MANIFEST.replace("{EZCP}", &ezcp_dependency())),
            (src.join("main.rs"), MAIN.to_owned()),
            (src.join("generator.rs"), generator.to_owned()),
        ];
        for (path, contents) in files {
            fs::write(&path, contents).map_err(io(&path))?;
        }

        let compilation = cargo.build(&manifest, true)?;
        if !compilation.success {
            return Ok(Err(compilation.errors().map(|error| error.rendered.as_str()).collect::<Vec<_>>().join("\n")));
        }
        let executable = compilation.executable.ok_or_else(|| Error::Invalid("the stress project built no binary".to_owned()))?;
        Ok(Ok(Self {
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
                Run::CandidateFailed { index, verdict, tier } => {
                    if index >= others.len() {
                        return Err(Error::Invalid(format!("the stress run reported candidate {index} of {}", others.len())));
                    }
                    let i = others.remove(index);
                    let reason = format!("{verdict} on tier {}", tier + 1);
                    if verdict == "WA" {
                        wrong.push((i, reason));
                    } else {
                        rejected[i] = Some(reason);
                    }
                }
                Run::OfficialFailed(verdict) => {
                    rejected[official] = Some(format!("{verdict} as the official solution"));
                    return Ok(false);
                }
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
        let output = Command::new(&self.executable)
            .current_dir(&self.dir)
            .args([self.tiers, TESTS, self.time_limit_ms as usize, self.memory_limit_mb as usize].map(|n| n.to_string()))
            .arg(official)
            .args(candidates.iter().map(|candidate| {
                let tiers: Vec<_> = candidate.tiers.iter().map(ToString::to_string).collect();
                format!("{}:{}", tiers.join(","), candidate.source.display())
            }))
            .output()
            .map_err(io(&self.executable))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.trim();
        match line.split(' ').collect::<Vec<_>>().as_slice() {
            ["passed"] => return Ok(Run::Passed),
            ["official", verdict] => return Ok(Run::OfficialFailed((*verdict).to_owned())),
            ["candidate", index, verdict, tier] => {
                if let (Ok(index), Ok(tier)) = (index.parse(), tier.parse()) {
                    return Ok(Run::CandidateFailed { index, verdict: (*verdict).to_owned(), tier });
                }
            }
            _ => {}
        }
        let message = line.strip_prefix("error ").map_or_else(
            || {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let lines: Vec<_> = stderr.lines().collect();
                lines[lines.len().saturating_sub(20)..].join("\n")
            },
            ToOwned::to_owned,
        );
        Err(Error::Invalid(format!("the stress run failed: {message}")))
    }
}
