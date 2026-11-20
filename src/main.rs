use ezcp_agent::cargo::Cargo;
use ezcp_agent::claude::{Claude, Stats};
use ezcp_agent::spec::{self, Spec};
use ezcp_agent::statement;
use ezcp_agent::task::{self, Task};
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

const CACHE_FILE: &str = "prompt-cache.jsonl";

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
    let claude = Claude::new()?.with_cache(dir.join(CACHE_FILE))?;
    // Checked before any tokens are spent.
    let _cargo = Cargo::new()?;

    let statement_path = dir.join(statement::FILE);
    let statement = if let Some(statement) = statement::load(&statement_path, &task)? {
        statement
    } else {
        announce(&statement_path);
        let (statement, stats) = statement::generate(&claude, &task)?;
        statement::save(&statement_path, &task, &statement)?;
        report(&stats);
        statement
    };
    if !review(statement::FILE)? {
        return Ok(());
    }

    let spec_path = dir.join(spec::FILE);
    let _spec = if let Some(spec) = Spec::load(&spec_path, &task, &statement)? {
        spec
    } else {
        announce(&spec_path);
        let (spec, stats) = Spec::generate(&claude, &task, &statement)?;
        spec.save(&spec_path, &task, &statement)?;
        report(&stats);
        spec
    };
    if !review(spec::FILE)? {
        return Ok(());
    }

    println!("Nothing more is implemented yet.");
    Ok(())
}

fn announce(path: &Path) {
    let verb = if path.exists() { "Regenerating" } else { "Writing" };
    println!("{verb} {}...", path.file_name().unwrap_or_default().display());
}

fn report(stats: &Stats) {
    if stats.cached {
        println!("from cache");
    } else {
        println!("{} output tokens, ~${:.2}", stats.usage.output_tokens, stats.cost_usd);
    }
}

fn review(file: &str) -> io::Result<bool> {
    let ok = confirm(&format!("Review {file}. Continue?"))?;
    if !ok {
        println!("Edit {file} or {} and rerun.", task::CONFIG_FILE);
    }
    Ok(ok)
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
