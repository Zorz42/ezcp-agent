//! Compiling C++ as EZCP does, to set aside sources that do not compile before they reach it.

use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Compiler {
    bin: PathBuf,
}

impl Compiler {
    /// `$GCC_PATH`, else the first of `g++`, `c++` and `clang++` that runs: EZCP's choice.
    pub fn new() -> Result<Self> {
        let candidates = match std::env::var("GCC_PATH") {
            Ok(path) if !path.is_empty() => vec![path],
            _ => vec!["g++".to_owned(), "c++".to_owned(), "clang++".to_owned()],
        };
        candidates
            .into_iter()
            .map(PathBuf::from)
            .find(|bin| Command::new(bin).arg("--version").output().is_ok_and(|output| output.status.success()))
            .map(|bin| Self { bin })
            .ok_or_else(|| Error::Invalid("no C++ compiler found; install g++ or set GCC_PATH".to_owned()))
    }

    /// The compiler's errors, or `None` if `source` compiles.
    pub fn errors(&self, source: &Path) -> Result<Option<String>> {
        let output = Command::new(&self.bin)
            .args(["-std=c++17", "-fsyntax-only"])
            .arg(source)
            .output()
            .map_err(|err| Error::Io { path: self.bin.clone(), source: err })?;
        Ok((!output.status.success()).then(|| String::from_utf8_lossy(&output.stderr).into_owned()))
    }
}
