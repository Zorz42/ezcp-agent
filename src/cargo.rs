//! Building crates with `cargo`, collecting rustc diagnostics as data.

use serde::Deserialize;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("`{0}` not found; install Rust or set CARGO")]
    NotFound(PathBuf),
    #[error("running cargo: {0}")]
    Io(#[from] io::Error),
    /// Cargo failed without a compiler error, e.g. a bad manifest or a failed download.
    #[error("cargo exited with {status}: {stderr}")]
    Failed { status: std::process::ExitStatus, stderr: String },
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// `error`, `warning`, `note`, ...
    pub level: String,
    /// The message as rustc prints it, with source snippet.
    pub rendered: String,
}

impl Diagnostic {
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.level.starts_with("error")
    }
}

#[derive(Debug, Clone)]
pub struct Compilation {
    pub success: bool,
    /// The last binary produced; `None` for `check` or failed builds.
    pub executable: Option<PathBuf>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Compilation {
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.is_error())
    }
}

#[derive(Deserialize)]
#[serde(tag = "reason")]
enum Message {
    #[serde(rename = "compiler-message")]
    Diagnostic { message: RawDiagnostic },
    #[serde(rename = "compiler-artifact")]
    Artifact { executable: Option<PathBuf> },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawDiagnostic {
    level: String,
    message: String,
    rendered: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Cargo {
    bin: PathBuf,
    version: String,
    target_dir: Option<PathBuf>,
}

impl Cargo {
    /// Locates `cargo` (or `$CARGO`) and verifies it runs.
    pub fn new() -> Result<Self> {
        let bin = std::env::var_os("CARGO").map_or_else(|| PathBuf::from("cargo"), PathBuf::from);
        let output = match Command::new(&bin).arg("--version").output() {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(Error::NotFound(bin)),
            result => result?,
        };
        if !output.status.success() {
            return Err(Error::Failed {
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        Ok(Self { bin, version, target_dir: None })
    }

    /// Shares one target directory between crates, so dependencies compile once.
    #[must_use]
    pub fn with_target_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.target_dir = Some(dir.into());
        self
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn check(&self, manifest_path: &Path) -> Result<Compilation> {
        self.compile("check", manifest_path, false)
    }

    pub fn build(&self, manifest_path: &Path, release: bool) -> Result<Compilation> {
        self.compile("build", manifest_path, release)
    }

    fn compile(&self, subcommand: &str, manifest_path: &Path, release: bool) -> Result<Compilation> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg(subcommand)
            .arg("--manifest-path")
            .arg(manifest_path)
            .args(["--message-format", "json", "--color", "never"]);
        if release {
            cmd.arg("--release");
        }
        if let Some(dir) = &self.target_dir {
            cmd.arg("--target-dir").arg(dir);
        }
        let output = cmd.output()?;

        let mut executable = None;
        let mut diagnostics = Vec::new();
        for line in output.stdout.split(|&b| b == b'\n') {
            match serde_json::from_slice(line) {
                Ok(Message::Diagnostic { message }) => diagnostics.push(Diagnostic {
                    level: message.level,
                    rendered: message.rendered.unwrap_or(message.message),
                }),
                Ok(Message::Artifact { executable: Some(path) }) => executable = Some(path),
                _ => {}
            }
        }

        let success = output.status.success();
        if !success && !diagnostics.iter().any(Diagnostic::is_error) {
            return Err(Error::Failed {
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(Compilation { success, executable, diagnostics })
    }
}
