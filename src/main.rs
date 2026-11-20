use ezcp_agent::{cargo::Cargo, claude::Claude};
use std::process::ExitCode;

fn main() -> ExitCode {
    let checks = Claude::new()
        .map(|c| c.version().to_owned())
        .map_err(|e| e.to_string())
        .and_then(|claude| Ok((claude, Cargo::new().map_err(|e| e.to_string())?.version().to_owned())));
    match checks {
        Ok((claude, cargo)) => {
            println!("claude {claude}\n{cargo}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
