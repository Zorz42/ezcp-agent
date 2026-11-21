use ezcp_agent::cargo::Cargo;
use ezcp_agent::claude::{Claude, Stats};
use ezcp_agent::cpp::Compiler;
use ezcp_agent::ladder::{self, Ladder};
use ezcp_agent::stress::{self, Candidate, Stress};
use ezcp_agent::task::{self, Task};
use ezcp_agent::{generator, read, solution, statement};
use std::io::{self, Write};
use std::path::Path;
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
const OFFICIAL_FILE: &str = "official.cpp";
/// Generator sources tried before giving up on compiler errors.
const GENERATOR_TRIES: usize = 3;

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
    let cargo = Cargo::new()?;
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

    let own = config.solution.source.as_ref().map(|path| read(&dir.join(path))).transpose()?;
    let attempts = if own.is_some() { 0 } else { config.solution.attempts };
    println!("Writing {attempts} solutions and {} slower ones...", ladder.full());
    let (mut solutions, stats) = solution::generate(&claude, &problem, config.solution.idea.as_deref(), &ladder, attempts)?;
    solutions.attempts.extend(own);
    let (attempt_paths, rung_paths) = solutions.save(&work.join(solution::DIR))?;
    report(&stats);

    let candidates: Vec<_> = ladder
        .rungs
        .iter()
        .zip(rung_paths)
        .enumerate()
        .map(|(i, (rung, source))| Candidate { name: rung.name.clone(), source, tiers: ladder.solves(i) })
        .chain(attempt_paths.into_iter().enumerate().map(|(i, source)| Candidate {
            name: format!("attempt {}", i + 1),
            source,
            tiers: ladder.solves(ladder.full()),
        }))
        .collect();
    let mut rejected = Vec::new();
    for candidate in &candidates {
        rejected.push(compiler.errors(&candidate.source)?.map(|_| "does not compile".to_owned()));
    }

    let stress = build_stress(&claude, &cargo, &work, &problem, &ladder)?;
    println!("Cross-checking the solutions...");
    let official = stress.check(&candidates, &mut rejected, |message| println!("  {message}"));
    for (candidate, reason) in candidates.iter().zip(&rejected) {
        println!("  {}: {}", candidate.name, reason.as_deref().unwrap_or("ok"));
    }
    let official = official?;
    std::fs::copy(&candidates[official].source, work.join(solution::DIR).join(OFFICIAL_FILE))?;
    println!("{} is the official solution.", candidates[official].name);

    if config.subtasks.is_empty() {
        let confirmed: Vec<_> = (0..ladder.rungs.len()).map(|tier| tier == ladder.full() || rejected[tier].is_none()).collect();
        println!("Writing the subtasks into {}...", statement::FILE);
        let (statement, stats) = statement::fill(&claude, &draft, &ladder.subtasks(&confirmed))?;
        statement::save(&statement_path, &statement)?;
        report(&[stats]);
    }

    println!("Nothing more is implemented yet.");
    Ok(())
}

/// Asks for a new generator while the last one does not compile.
fn build_stress(claude: &Claude, cargo: &Cargo, dir: &Path, statement: &str, ladder: &Ladder) -> Result<Stress, Box<dyn std::error::Error>> {
    let mut previous: Option<(String, String)> = None;
    for _ in 0..GENERATOR_TRIES {
        println!("Writing the generator...");
        let (source, stats) = generator::generate(claude, statement, ladder, previous.as_ref().map(|(s, e)| (s.as_str(), e.as_str())))?;
        report(&[stats]);
        println!("Building {}...", stress::DIR);
        match Stress::build(cargo, &dir.join(stress::DIR), &source, ladder)? {
            Ok(stress) => return Ok(stress),
            Err(errors) => {
                println!("It does not compile.");
                previous = Some((source, errors));
            }
        }
    }
    Err(format!("the generator still does not compile after {GENERATOR_TRIES} tries").into())
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
