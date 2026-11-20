pub mod cargo;
pub mod claude;
pub mod solution;
pub mod spec;
pub mod statement;
pub mod task;

use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error("{}: {source}", path.display())]
    Toml { path: PathBuf, source: toml::de::Error },
    #[error(transparent)]
    TomlWrite(#[from] toml::ser::Error),
    #[error(transparent)]
    Claude(#[from] claude::Error),
    #[error(transparent)]
    Cargo(#[from] cargo::Error),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|source| Error::Io { path: path.to_owned(), source })
}
