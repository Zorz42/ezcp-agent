use ezcp_agent::cargo::Cargo;
use ezcp_agent::claude::{Claude, Stats};
use ezcp_agent::cpp::Compiler;
use ezcp_agent::ladder::{self, Ladder};
use ezcp_agent::project::{self, Kind, Outcome, Partial, Project};
use ezcp_agent::stress::{self, Candidate, Stress};
use ezcp_agent::task::{self, Task};
use ezcp_agent::testset::{self, Target};
use ezcp_agent::wrong::{self, Base};
use ezcp_agent::{generator, limits, read, runner, solution, statement};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Runs in the current directory: `ezcp-agent`.
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Everything but the results, for deeper inspection.
const WORK_DIR: &str = "ezcp-agent";
const CACHE_FILE: &str = "prompt-cache.jsonl";
/// Shared by the runner crates.
const TARGET_DIR: &str = "target";
const UNOPTIMIZED: &str = "unoptimized";
/// Model-written Rust sources tried before giving up on their errors.
const TRIES: usize = 3;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::current_dir()?;

    if !dir.join(task::CONFIG_FILE).exists() {
        if confirm(&format!("{} is not an ezcp-agent project. Initialize it?", dir.display()))? {
            Task::init(&dir)?;
            println!("Fill in the description in {}, then rerun.", task::CONFIG_FILE);
        }
        return Ok(());
    }

    let task = Task::load(&dir)?;
    let config = &task.config;
    let work = dir.join(WORK_DIR);
    std::fs::create_dir_all(&work)?;
    let claude = Claude::new()?.with_cache(work.join(CACHE_FILE))?;
    // Checked before any tokens are spent.
    let cargo = Cargo::new()?.with_target_dir(work.join(TARGET_DIR));
    let compiler = Compiler::new()?;

    // Everything is regenerated on every run: unchanged inputs are served from the cache.
    let statement_path = dir.join(statement::FILE);
    println!("Writing {}...", statement::FILE);
    let (draft, stats) = statement::generate(&claude, &task)?;
    statement::save(&statement_path, &draft)?;
    report(&[stats]);
    let problem = statement::strip(&draft);

    println!("Planning the solutions...");
    let (ladder, stats) = Ladder::generate(&claude, &task, &problem)?;
    ladder.save(&work.join(ladder::FILE))?;
    report(&[stats]);
    if ladder.multiple_answers {
        return Err("the task has several correct answers, which needs a checker: not supported yet".into());
    }
    let mut project = Project {
        dir: dir.join(project::DIR),
        title: title(&draft),
        time_limit_ms: ladder.time_limit_ms,
        memory_limit_mb: ladder.memory_limit_mb,
        subtasks: Vec::new(),
        partials: Vec::new(),
        seeds: config.tests.seeds,
    };

    let own = config.solution.source.as_ref().map(|path| read(&dir.join(path))).transpose()?;
    let attempts = if own.is_some() { 0 } else { config.solution.attempts };
    println!("Writing {attempts} solutions and {} slower ones...", ladder.full());
    let (mut solutions, stats) = solution::generate(&claude, &problem, config.solution.idea.as_deref(), &ladder, attempts)?;
    solutions.attempts.extend(own);
    let (attempt_paths, rung_paths) = solutions.save(&work.join(solution::DIR))?;
    report(&stats);

    let candidates = candidates(&ladder, rung_paths, attempt_paths);
    let sources: Vec<_> = candidates.iter().map(|candidate| candidate.source.clone()).collect();
    let mut rejected: Vec<_> = compiles(&compiler, &sources)?.into_iter().map(|ok| (!ok).then(|| "does not compile".to_owned())).collect();

    let stress = build_stress(&claude, &cargo, &work, &problem, &ladder, &project.generator())?;
    println!("Cross-checking {} solutions...", candidates.len());
    let official = stress.check(&candidates, &mut rejected);
    for (candidate, reason) in candidates.iter().zip(&rejected) {
        if let Some(reason) = reason {
            println!("  rejected {}: {reason}", candidate.name);
        }
    }
    let official = official?;
    println!("{} agree", rejected.iter().filter(|reason| reason.is_none()).count());
    let official = read(&candidates[official].source)?;

    let full = ladder.full();
    let author_subtasks = !config.subtasks.is_empty();
    let confirmed: Vec<_> = (0..ladder.rungs.len()).map(|tier| tier == full || rejected[tier].is_none()).collect();
    project.subtasks = ladder.subtasks(&if author_subtasks { vec![true; confirmed.len()] } else { confirmed.clone() });
    let statement = if author_subtasks {
        draft
    } else {
        println!("Writing the subtasks into {}...", statement::FILE);
        let (statement, stats) = statement::fill(&claude, &draft, &project.subtasks)?;
        statement::save(&statement_path, &statement)?;
        report(&[stats]);
        statement
    };

    println!("Writing the plain and the micro-optimized solution...");
    let (limits, stats) = limits::generate(&claude, &statement, &official, &ladder, &project.subtasks)?;
    report(&stats);
    let rung_sources = candidates[..full].iter().map(|candidate| read(&candidate.source)).collect::<Result<Vec<_>, _>>()?;
    let mut solutions = assemble(&ladder, &confirmed, rung_sources, &project.subtasks, limits);
    let wrong = write_wrong(&claude, &statement, &official, &ladder, &project.subtasks, &solutions, config.tests.wrong)?;
    solutions.extend(wrong);
    let targets = keep_compiling(&compiler, &mut project, &official, solutions)?;
    let is_wrong = |partial: &&Partial| matches!(partial.kind, Kind::Wrong { .. });
    let wrong_written = project.partials.iter().filter(is_wrong).count();

    project.write(&dir)?;
    write_tests(&claude, &cargo, &work, &dir, &statement, &mut project, &targets)?;
    if let Some(ms) = project.slowest_ms(UNOPTIMIZED)?.filter(|&ms| 2 * ms > project.time_limit_ms) {
        println!("  unoptimized takes {ms} ms of {} ms: the limits may be too tight", project.time_limit_ms);
    }
    let caught = project.partials.iter().filter(is_wrong).count();
    println!("{caught} of {wrong_written} wrong solutions caught");
    println!("The EZCP project is in {}/, its tests in {}/tests.", project::DIR, project::DIR);
    Ok(())
}

/// The lower rungs first, then the full attempts.
fn candidates(ladder: &Ladder, rung_paths: Vec<PathBuf>, attempt_paths: Vec<PathBuf>) -> Vec<Candidate> {
    let rungs = ladder.rungs.iter().zip(rung_paths).enumerate();
    let rungs = rungs.map(|(i, (rung, source))| Candidate { name: rung.name.clone(), source, tiers: ladder.solves(i) });
    let attempts = attempt_paths.into_iter().enumerate();
    let attempts = attempts.map(|(i, source)| Candidate { name: format!("attempt {}", i + 1), source, tiers: ladder.solves(ladder.full()) });
    rungs.chain(attempts).collect()
}

/// A partial solution, its source and, unless it must pass everything, what the tests must break.
struct Solution {
    partial: Partial,
    source: String,
    target: Option<Target>,
    /// The subtask it is the solution of.
    base: Option<usize>,
}

/// Within `0..count`, sorted and unique.
fn declared(passes: &[usize], count: usize) -> Vec<usize> {
    let mut passes: Vec<_> = passes.iter().copied().filter(|&s| s < count).collect();
    passes.sort_unstable();
    passes.dedup();
    passes
}

fn fails(passes: &[usize], count: usize) -> Vec<usize> {
    (0..count).filter(|s| !passes.contains(s)).collect()
}

/// The confirmed lower rungs, and the plain and the micro-optimized solution.
fn assemble(ladder: &Ladder, confirmed: &[bool], rung_sources: Vec<String>, subtasks: &[ladder::Subtask], limits: limits::Limits) -> Vec<Solution> {
    let count = subtasks.len();
    let subtask_of = |tier: usize| subtasks.iter().position(|subtask| subtask.tier == tier);
    let partial = |kind, name: &str, file: String, passes| Partial { kind, name: name.to_owned(), file, passes };

    let mut solutions = Vec::new();
    for ((tier, rung), source) in ladder.rungs.iter().enumerate().zip(rung_sources).filter(|((tier, _), _)| confirmed[*tier]) {
        let passes: Vec<_> = ladder.solves(tier).into_iter().filter_map(subtask_of).collect();
        let target = Target { description: format!("{}, {}", rung.approach, rung.complexity), edits: None, fails: fails(&passes, count) };
        let base = subtask_of(tier);
        let file = format!("subtask-{}.cpp", base.map_or(tier, |s| s + 1));
        solutions.push(Solution { partial: partial(Kind::Subtask, &rung.name, file, passes), source, target: Some(target), base });
    }
    let unoptimized = partial(Kind::Unoptimized, UNOPTIMIZED, format!("{UNOPTIMIZED}.cpp"), (0..count).collect());
    solutions.push(Solution { partial: unoptimized, source: limits.unoptimized, target: None, base: None });
    let passes: Vec<_> = declared(&limits.micro.passes, count).into_iter().filter(|&s| s + 1 < count).collect();
    let target = Target { description: limits.micro.approach, edits: None, fails: fails(&passes, count) };
    let micro = partial(Kind::Micro, "micro-optimized", "micro-optimized.cpp".to_owned(), passes);
    solutions.push(Solution { partial: micro, source: limits.micro.source, target: Some(target), base: None });
    solutions
}

/// Slight mistakes in each subtask's solution and the official one, and every probable wrong
/// approach, each aimed at a subtask. Those that do not apply or repeat another are set aside.
fn write_wrong(claude: &Claude, statement: &str, official: &str, ladder: &Ladder, subtasks: &[ladder::Subtask], solutions: &[Solution], count: u32) -> Result<Vec<Solution>, Box<dyn std::error::Error>> {
    let n = subtasks.len();
    let all: Vec<_> = (0..n).collect();
    let rungs = solutions.iter().filter_map(|solution| Some(Base { subtask: solution.base?, source: &solution.source, passes: &solution.partial.passes }));
    let bases: Vec<_> = rungs.chain([Base { subtask: n - 1, source: official, passes: &all }]).collect();

    println!("Writing {count} wrong solutions for each of {} subtasks...", bases.len());
    let (mutants, stats) = wrong::mutants(claude, statement, subtasks, &bases, count)?;
    report(&stats);
    println!("Listing wrong approaches...");
    let (approaches, stats) = wrong::approaches(claude, statement, official, ladder, subtasks)?;
    report(&[stats]);
    println!("Writing {} wrong approaches...", approaches.len());
    let (sources, stats) = wrong::write(claude, statement, subtasks, &approaches)?;
    report(&stats);

    let mut seen: std::collections::HashSet<_> = bases.iter().map(|base| base.source.to_owned()).collect();
    let mut wrong = Vec::new();
    let mut add = |name: String, file: String, aim: usize, passes: Vec<usize>, source: Option<String>, target: Target| match source {
        None => println!("  set aside {name}: its edits do not apply"),
        Some(source) if !seen.insert(source.clone()) => println!("  set aside {name}: the same as another"),
        Some(source) => wrong.push(Solution { partial: Partial { kind: Kind::Wrong { subtask: aim }, name, file, passes }, source, target: Some(target), base: None }),
    };
    for (base, mutants) in bases.iter().zip(mutants) {
        let aim = base.subtask;
        for (i, mutant) in mutants.iter().enumerate() {
            let passes: Vec<_> = declared(&mutant.passes, n).into_iter().filter(|s| *s != aim && base.passes.contains(s)).collect();
            // Elsewhere the base itself fails.
            let fails = base.passes.iter().copied().filter(|s| !passes.contains(s)).collect();
            let target = Target { description: mutant.mistake.clone(), edits: Some(serde_json::to_value(&mutant.edits)?), fails };
            let (name, file) = (format!("subtask {} mistake: {}", aim + 1, mutant.mistake), format!("wrong/{}-mistake-{:02}.cpp", aim + 1, i + 1));
            add(name, file, aim, passes, mutant.apply(base.source), target);
        }
    }
    for (i, (approach, source)) in approaches.iter().zip(sources).enumerate() {
        let aim = approach.subtask.min(n - 1);
        let passes: Vec<_> = declared(&approach.passes, n).into_iter().filter(|&s| s != aim).collect();
        let description = format!("{}: {} Fails on: {}", approach.name, approach.approach, approach.fails_on);
        let target = Target { description, edits: None, fails: fails(&passes, n) };
        let (name, file) = (format!("subtask {} approach: {}", aim + 1, approach.name), format!("wrong/{}-approach-{:02}.cpp", aim + 1, i + 1));
        add(name, file, aim, passes, Some(source), target);
    }
    Ok(wrong)
}

/// Writes the solutions and keeps those that compile as the project's partial solutions. Returns
/// their targets.
fn keep_compiling(compiler: &Compiler, project: &mut Project, official: &str, solutions: Vec<Solution>) -> Result<Vec<Target>, Box<dyn std::error::Error>> {
    project.partials = solutions.iter().map(|solution| solution.partial.clone()).collect();
    let sources: Vec<_> = solutions.iter().map(|solution| solution.source.clone()).collect();
    project.write_solutions(official, &sources)?;
    let paths: Vec<_> = project.partials.iter().map(|partial| project.solution(partial)).collect();
    let mut targets = Vec::new();
    project.partials.clear();
    for ((solution, ok), path) in solutions.into_iter().zip(compiles(compiler, &paths)?).zip(&paths) {
        if ok {
            targets.extend(solution.target);
            project.partials.push(solution.partial);
        } else {
            println!("  set aside {}: does not compile", solution.partial.name);
            std::fs::remove_file(path)?;
        }
    }
    Ok(targets)
}

/// Asks for a new test plan while the last one does not compile or a generator panics, then verifies
/// the project.
fn write_tests(claude: &Claude, cargo: &Cargo, work: &Path, dir: &Path, statement: &str, project: &mut Project, targets: &[Target]) -> Result<(), Box<dyn std::error::Error>> {
    let generator = read(&project.generator())?;
    let mut previous: Option<(String, String)> = None;
    for _ in 0..TRIES {
        println!("Writing the tests...");
        let previous_ref = previous.as_ref().map(|(s, e)| (s.as_str(), e.as_str()));
        let (source, stats) = testset::generate(claude, statement, &generator, &project.subtasks, targets, previous_ref)?;
        report(&[stats]);
        runner::write(&project.tests(), &source)?;
        println!("Building {}...", project::CHECK_DIR);
        let check = match project.build_check(cargo, &work.join(project::CHECK_DIR))? {
            Ok(check) => check,
            Err(errors) => {
                println!("It does not compile.");
                previous = Some((source, errors));
                continue;
            }
        };
        println!("Running EZCP...");
        match project.verify(dir, cargo, check, &mut |note| println!("  {note}"))? {
            Outcome::Passed => return Ok(()),
            Outcome::Panicked(message) => {
                println!("A generator panicked.");
                previous = Some((source, format!("panicked: {message}")));
            }
        }
    }
    Err(format!("the tests still fail after {TRIES} tries").into())
}

/// Asks for a new generator while the last one does not compile.
fn build_stress(claude: &Claude, cargo: &Cargo, dir: &Path, statement: &str, ladder: &Ladder, path: &Path) -> Result<Stress, Box<dyn std::error::Error>> {
    let mut previous: Option<(String, String)> = None;
    for _ in 0..TRIES {
        println!("Writing the generator...");
        let (source, stats) = generator::generate(claude, statement, ladder, previous.as_ref().map(|(s, e)| (s.as_str(), e.as_str())))?;
        report(&[stats]);
        runner::write(path, &source)?;
        println!("Building {}...", stress::DIR);
        match Stress::build(cargo, &dir.join(stress::DIR), path, ladder)? {
            Ok(stress) => return Ok(stress),
            Err(errors) => {
                println!("It does not compile.");
                previous = Some((source, errors));
            }
        }
    }
    Err(format!("the generator still does not compile after {TRIES} tries").into())
}

/// Whether each source compiles, checked in parallel.
fn compiles(compiler: &Compiler, paths: &[PathBuf]) -> Result<Vec<bool>, Box<dyn std::error::Error>> {
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let mut results = Vec::new();
    for chunk in paths.chunks(threads) {
        let chunk = std::thread::scope(|scope| {
            let handles: Vec<_> = chunk.iter().map(|path| scope.spawn(move || compiler.errors(path))).collect();
            handles.into_iter().map(|handle| handle.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic))).collect::<Vec<_>>()
        });
        for errors in chunk {
            results.push(errors?.is_none());
        }
    }
    Ok(results)
}

/// The statement's first heading.
fn title(statement: &str) -> String {
    statement.lines().find_map(|line| line.strip_prefix("# ")).unwrap_or("Task").trim().to_owned()
}

fn report(stats: &[Stats]) {
    let cached = stats.iter().filter(|s| s.cached).count();
    if cached == stats.len() {
        println!("from cache");
        return;
    }
    let tokens: u64 = stats.iter().filter(|s| !s.cached).map(|s| s.usage.output_tokens).sum();
    let cost: f64 = stats.iter().filter(|s| !s.cached).map(|s| s.cost_usd).sum();
    if cached == 0 {
        println!("{tokens} output tokens, ~${cost:.2}");
    } else {
        println!("{cached} from cache, {tokens} output tokens, ~${cost:.2}");
    }
}

/// `false` on end of input.
fn confirm(question: &str) -> io::Result<bool> {
    loop {
        print!("{question} [y/n] ");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            return Ok(false);
        }
        match line.trim() {
            "y" | "Y" => return Ok(true),
            "n" | "N" => return Ok(false),
            _ => {}
        }
    }
}
