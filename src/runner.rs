//! Runner crates in the work directory: a fixed `main.rs` around the model's sources, which it
//! includes by path, printing one JSON result. They share one target directory.

use crate::cargo::Cargo;
use crate::{Error, Result};
use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const MANIFEST: &str = include_str!("../templates/runner/Cargo.toml");

pub fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Error {
    let path = path.to_owned();
    move |source| Error::Io { path, source }
}

pub fn write(path: &Path, contents: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io(dir))?;
    }
    fs::write(path, contents).map_err(io(path))
}

/// `$EZCP_PATH`, else the repository.
#[must_use]
pub fn ezcp_dependency() -> String {
    match std::env::var("EZCP_PATH") {
        Ok(path) if !path.is_empty() => format!("{{ path = {} }}", toml::Value::String(path)),
        _ => r#"{ git = "https://github.com/Zorz42/EZCP" }"#.to_owned(),
    }
}

/// `main` with each `{KEY}` replaced by the path of the source it includes.
pub fn main_source(main: &str, includes: &[(&str, &Path)]) -> Result<String> {
    let mut main = main.to_owned();
    for (key, path) in includes {
        let path = fs::canonicalize(path).map_err(io(path))?;
        main = main.replace(&format!("{{{key}}}"), &format!("{:?}", path.display().to_string()));
    }
    Ok(main)
}

/// Writes and builds the crate `name` in `dir`. The inner error holds rustc's errors.
pub fn build(cargo: &Cargo, dir: &Path, name: &str, main: &str) -> Result<std::result::Result<PathBuf, String>> {
    let manifest = dir.join("Cargo.toml");
    write(&manifest, &MANIFEST.replace("{NAME}", name).replace("{EZCP}", &ezcp_dependency()))?;
    write(&dir.join("src").join("main.rs"), main)?;
    let compilation = cargo.build(&manifest, true)?;
    if !compilation.success {
        return Ok(Err(compilation.errors().map(|error| error.rendered.as_str()).collect::<Vec<_>>().join("\n")));
    }
    let executable = compilation.executable.ok_or_else(|| Error::Invalid(format!("{name} built no binary")))?;
    Ok(Ok(executable))
}

/// Runs `executable` in `dir` and parses its result line, else reports the end of its stderr.
pub fn run<T: DeserializeOwned>(executable: &Path, dir: &Path, args: &[String]) -> Result<T> {
    let output = Command::new(executable).current_dir(dir).args(args).output().map_err(io(executable))?;
    if let Ok(result) = serde_json::from_slice(&output.stdout) {
        return Ok(result);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<_> = stderr.lines().collect();
    Err(Error::Invalid(format!("{} failed: {}", executable.display(), lines[lines.len().saturating_sub(20)..].join("\n"))))
}
