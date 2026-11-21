//! The EZCP project: the generator, the test plan, the task and its solutions. It is run through a
//! runner, and the subtasks each partial solution is declared to pass are corrected from the outcome.

use crate::cargo::Cargo;
use crate::ladder::Subtask;
use crate::{Error, Result, runner};
use serde::Deserialize;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

pub const DIR: &str = "ezcp";
pub const CHECK_DIR: &str = "check";
const MANIFEST: &str = include_str!("../templates/project/Cargo.toml");
const MAIN: &str = include_str!("../templates/project/main.rs");
const GITIGNORE: &str = include_str!("../templates/project/gitignore");
const CHECK_MAIN: &str = include_str!("../templates/runner/check.rs");
const OFFICIAL: &str = "official.cpp";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A confirmed rung: correct on its subtasks, too slow or wrong on the others.
    Subtask,
    /// The intended algorithm written plainly: must pass everything.
    Unoptimized,
    /// A slower algorithm micro-optimized: must fail the full subtask.
    Micro,
    /// Must fail the subtask it is aimed at.
    Wrong { subtask: usize },
}

#[derive(Debug, Clone)]
pub struct Partial {
    pub kind: Kind,
    pub name: String,
    /// Under `solutions/`.
    pub file: String,
    /// Subtasks, counted from 0.
    pub passes: Vec<usize>,
}

pub struct Project {
    pub dir: PathBuf,
    pub title: String,
    pub time_limit_ms: u32,
    pub memory_limit_mb: u32,
    pub subtasks: Vec<Subtask>,
    pub partials: Vec<Partial>,
    pub seeds: bool,
}

/// The check runner's outcome; solutions are indices into `partials`.
#[derive(Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
enum Run {
    Passed,
    Fails { solution: usize, subtask: usize, verdict: String },
    Passes { solution: usize, subtask: usize },
    OfficialFailed { verdict: String },
    Panicked { message: String },
    Error { message: String },
}

pub enum Outcome {
    Passed,
    /// A generator or validator panicked: the test plan has to change.
    Panicked(String),
}

pub struct Check {
    dir: PathBuf,
    executable: PathBuf,
}

/// A Cargo package name from the task directory's name.
fn package_name(dir: &Path) -> String {
    let name = dir.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default();
    let name: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let name = name.trim_matches('-');
    if name.starts_with(|c: char| c.is_ascii_alphabetic()) { name.to_owned() } else { format!("task-{name}") }
}

impl Project {
    #[must_use]
    pub fn generator(&self) -> PathBuf {
        self.dir.join("src").join("generator.rs")
    }

    #[must_use]
    pub fn tests(&self) -> PathBuf {
        self.dir.join("src").join("tests.rs")
    }

    fn task(&self) -> PathBuf {
        self.dir.join("src").join("task.rs")
    }

    fn solutions(&self) -> PathBuf {
        self.dir.join("solutions")
    }

    #[must_use]
    pub fn solution(&self, partial: &Partial) -> PathBuf {
        self.solutions().join(&partial.file)
    }

    /// Replaces `solutions/` with the official solution and the partial ones' `sources`.
    pub fn write_solutions(&self, official: &str, sources: &[String]) -> Result<()> {
        let dir = self.solutions();
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(runner::io(&dir))?;
        }
        runner::write(&dir.join(OFFICIAL), official)?;
        for (partial, source) in self.partials.iter().zip(sources) {
            runner::write(&dir.join(&partial.file), source)?;
        }
        Ok(())
    }

    /// Everything but the sources written separately.
    pub fn write(&self, task_dir: &Path) -> Result<()> {
        let manifest = MANIFEST.replace("{NAME}", &package_name(task_dir)).replace("{EZCP}", &runner::ezcp_dependency());
        let run = if self.seeds { "cargo run --release -- --seeds" } else { "cargo run --release" };
        runner::write(&self.dir.join("Cargo.toml"), &manifest)?;
        runner::write(&self.dir.join(".gitignore"), GITIGNORE)?;
        runner::write(&self.dir.join("src").join("main.rs"), &MAIN.replace("{RUN}", run))?;
        runner::write(&self.task(), &self.task_source())
    }

    fn task_source(&self) -> String {
        let mut source = String::from(
            "//! Written by ezcp-agent: the verified solutions, each declared with the subtasks it passes.\n\n\
             use crate::generator::{self, Input};\nuse ezcp::{Subtask, Task};\nuse std::path::Path;\n\n\
             pub fn task(path: &Path) -> Task<Input> {\n",
        );
        let _ = writeln!(source, "    Task::new({:?}, path)", self.title);
        let _ = writeln!(source, "        .with_solution_source(include_str!(\"../solutions/{OFFICIAL}\"))");
        let _ = writeln!(source, "        .with_time_limit({})", self.time_limit_ms);
        let _ = writeln!(source, "        .with_memory_limit({})", self.memory_limit_mb);
        // Solutions meant to fail stop at their first failure in the final run; the tests stay the same.
        let _ = writeln!(source, "        .with_fail_fast(true)");
        for subtask in &self.subtasks {
            let _ = writeln!(source, "        .with_subtask(subtask({}, {:?}, {}))", subtask.points, subtask.constraints.join(", "), subtask.tier);
        }
        for partial in &self.partials {
            let _ = writeln!(
                source,
                "        .with_partial_solution({:?}, include_str!(\"../solutions/{}\"), &{:?})",
                partial.name, partial.file, partial.passes
            );
        }
        source.push_str(
            "}\n\nfn subtask(points: i32, name: &str, tier: usize) -> Subtask<Input> {\n    \
             crate::tests::tests(Subtask::new(points, name), tier).with_validator(move |input: &Input| generator::validate(input, tier))\n}\n",
        );
        source
    }

    /// Builds the check runner. The inner error holds rustc's errors.
    pub fn build_check(&self, cargo: &Cargo, dir: &Path) -> Result<std::result::Result<Check, String>> {
        let main = runner::main_source(CHECK_MAIN, &[("GENERATOR", &self.generator()), ("TASK", &self.task()), ("TESTS", &self.tests())])?;
        Ok(runner::build(cargo, dir, CHECK_DIR, &main)?.map(|executable| Check { dir: dir.to_owned(), executable }))
    }

    /// Runs the task until it passes, correcting the declarations after each failure, and rebuilding
    /// with them. `note` gets each correction worth telling.
    pub fn verify(&mut self, task_dir: &Path, cargo: &Cargo, mut check: Check, note: &mut impl FnMut(&str)) -> Result<Outcome> {
        let mode = if self.seeds { "seeds" } else { "files" };
        let args = [fs::canonicalize(&self.dir).map_err(runner::io(&self.dir))?.display().to_string(), mode.to_owned()];
        // Each correction moves a subtask in or out of a declaration; more means it goes back and forth.
        for _ in 0..=2 * self.partials.len() * self.subtasks.len() {
            let (solution, subtask, verdict) = match runner::run(&check.executable, &self.dir, &args)? {
                Run::Passed => return Ok(Outcome::Passed),
                Run::Panicked { message } => return Ok(Outcome::Panicked(message)),
                Run::OfficialFailed { verdict } => return Err(Error::Invalid(format!("the official solution fails a test: {verdict}"))),
                Run::Error { message } => return Err(Error::Invalid(format!("the EZCP run failed: {message}"))),
                Run::Fails { solution, subtask, verdict } => (solution, subtask, Some(verdict)),
                Run::Passes { solution, subtask } => (solution, subtask, None),
            };
            if solution >= self.partials.len() || subtask >= self.subtasks.len() {
                return Err(Error::Invalid(format!("the EZCP run reported solution {solution}, subtask {subtask}")));
            }
            if let Some(message) = self.correct(solution, subtask, verdict.as_deref())? {
                note(&message);
            }
            self.write(task_dir)?;
            check = self.build_check(cargo, &check.dir)?.map_err(|errors| Error::Invalid(format!("task.rs does not compile: {errors}")))?;
        }
        Err(Error::Invalid("the declared subtasks keep changing".to_owned()))
    }

    /// A partial solution's slowest time in the last run's `results.txt`, if it passed everything.
    pub fn slowest_ms(&self, name: &str) -> Result<Option<u32>> {
        let results = crate::read(&self.dir.join("results.txt"))?;
        let Some(index) = self.partials.iter().position(|partial| partial.name == name) else { return Ok(None) };
        let heading = format!("Running partial solution {}: ", index + 1);
        let block = results.lines().skip_while(|line| !line.starts_with(&heading)).skip(1).take_while(|line| !line.starts_with("Running") && !line.starts_with("Tests size"));
        let times = block.filter_map(|line| line.strip_prefix("- Subtask")?.split_whitespace().find_map(|token| token.strip_suffix("ms")?.parse::<u32>().ok()));
        Ok(times.max())
    }

    /// `verdict` is `None` if the solution passed `subtask` instead of failing it.
    fn correct(&mut self, solution: usize, subtask: usize, verdict: Option<&str>) -> Result<Option<String>> {
        let full = subtask + 1 == self.subtasks.len();
        let partial = &mut self.partials[solution];
        let shown = subtask + 1;
        let drop = match (verdict, partial.kind) {
            (Some(_), Kind::Wrong { .. } | Kind::Micro) => {
                partial.passes.retain(|&s| s != subtask);
                return Ok((partial.kind == Kind::Micro).then(|| format!("{} fails subtask {shown} too", partial.name)));
            }
            (Some(verdict), Kind::Unoptimized) => format!("{verdict} on subtask {shown}: the limits may be too tight"),
            (Some(verdict), Kind::Subtask) => format!("{verdict} on subtask {shown}"),
            (None, Kind::Wrong { subtask: aim }) if aim == subtask => format!("not caught on subtask {shown}"),
            (None, Kind::Micro) if full => "passes the full subtask: the time limit may be too loose".to_owned(),
            (None, Kind::Subtask) if full => "passes the full subtask".to_owned(),
            (None, kind) => {
                partial.passes.push(subtask);
                partial.passes.sort_unstable();
                return Ok((!matches!(kind, Kind::Wrong { .. })).then(|| format!("{} passes subtask {shown} too", partial.name)));
            }
        };
        let partial = self.partials.remove(solution);
        let path = self.solution(&partial);
        fs::remove_file(&path).map_err(runner::io(&path))?;
        Ok(Some(format!("dropped {}: {drop}", partial.name)))
    }
}
