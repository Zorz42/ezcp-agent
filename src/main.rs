use ezcp_agent::cargo::Cargo;
use ezcp_agent::claude::Claude;
use ezcp_agent::spec::{self, Spec};
use ezcp_agent::task::{self, Task};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

/// Usage: `ezcp-agent [task-dir]` (default: the current directory).
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args_os().nth(1).map_or_else(|| PathBuf::from("."), PathBuf::from);

    if !dir.join(task::CONFIG_FILE).exists() {
        if confirm(&format!("No {} in {}. Create one?", task::CONFIG_FILE, dir.display()))? {
            Task::init(&dir)?;
            println!("Fill in {} and {}, then rerun.", task::STATEMENT_FILE, task::CONFIG_FILE);
        }
        return Ok(());
    }

    let task = Task::load(&dir)?;
    let claude = Claude::new()?.with_cache(dir.join(".ezcp-agent/cache"));
    // Checked before any tokens are spent.
    let _cargo = Cargo::new()?;

    let spec_path = dir.join(spec::FILE);
    let _spec = if let Some(spec) = Spec::load(&spec_path, &task)? {
        spec
    } else {
        let verb = if spec_path.exists() { "Regenerating" } else { "Writing" };
        println!("{verb} {}...", spec::FILE);
        let (spec, stats) = Spec::generate(&claude, &task)?;
        spec.save(&spec_path, &task)?;
        if stats.cached {
            println!("from cache");
        } else {
            println!("{} output tokens, ~${:.2}", stats.usage.output_tokens, stats.cost_usd);
        }
        spec
    };

    if !confirm(&format!("Review {}. Continue?", spec_path.display()))? {
        println!("Edit {} or the task and rerun.", spec::FILE);
        return Ok(());
    }

    println!("Nothing more is implemented yet.");
    Ok(())
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
