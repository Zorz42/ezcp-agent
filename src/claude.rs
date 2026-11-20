//! One-shot, tool-less calls to the `claude` CLI in print mode.

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("`{0}` not found; install Claude Code or set CLAUDE_PATH")]
    NotFound(PathBuf),
    #[error("running claude: {0}")]
    Io(#[from] io::Error),
    #[error("claude exited with {status}: {stderr}")]
    Failed { status: std::process::ExitStatus, stderr: String },
    #[error("claude reported an error ({kind}): {message}")]
    Api { kind: String, message: String },
    #[error("unexpected claude output: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("structured output requested but none returned")]
    NoStructuredOutput,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Call settings. Unset fields fall back to the CLI's defaults.
#[derive(Debug, Default, Clone, Copy)]
pub struct Request<'a> {
    /// The instruction; [`Claude::ask_json`] appends its input after it.
    pub prompt: &'a str,
    /// Replaces Claude Code's default system prompt entirely.
    pub system: Option<&'a str>,
    /// Alias (`opus`, `sonnet`, `haiku`) or full model id.
    pub model: Option<&'a str>,
    /// `low` | `medium` | `high` | `xhigh` | `max`.
    pub effort: Option<&'a str>,
    pub max_budget_usd: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
}

/// Accounting for one call.
#[derive(Debug, Clone, Copy)]
pub struct Stats {
    pub cost_usd: f64,
    pub usage: Usage,
    pub duration_ms: u64,
}

#[derive(Deserialize)]
struct RawResult {
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    subtype: String,
    api_error_type: Option<String>,
    #[serde(default)]
    result: String,
    structured_output: Option<Value>,
    #[serde(default)]
    total_cost_usd: f64,
    #[serde(default)]
    usage: Usage,
    #[serde(default)]
    duration_ms: u64,
}

impl RawResult {
    const fn stats(&self) -> Stats {
        Stats { cost_usd: self.total_cost_usd, usage: self.usage, duration_ms: self.duration_ms }
    }
}

#[derive(Debug, Clone)]
pub struct Claude {
    bin: PathBuf,
    version: String,
}

impl Claude {
    /// Locates `claude` (or `$CLAUDE_PATH`) and verifies it runs.
    pub fn new() -> Result<Self> {
        let bin = std::env::var_os("CLAUDE_PATH").map_or_else(|| PathBuf::from("claude"), PathBuf::from);
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
        Ok(Self { bin, version })
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Plain text in, plain text out.
    pub fn ask(&self, request: &Request) -> Result<(String, Stats)> {
        let raw = self.call(request, request.prompt, None)?;
        let stats = raw.stats();
        Ok((raw.result, stats))
    }

    /// Sends `input` as JSON after the prompt; the reply is forced to match `O`'s JSON schema.
    pub fn ask_json<O: DeserializeOwned + JsonSchema>(
        &self,
        request: &Request,
        input: &impl Serialize,
    ) -> Result<(O, Stats)> {
        let prompt = format!("{}\n\nInput:\n{}", request.prompt, serde_json::to_string(input)?);
        let mut schema = schemars::schema_for!(O);
        // The CLI's validator can't resolve schemars' draft 2020-12 meta-schema URI.
        schema.remove("$schema");
        let schema = serde_json::to_string(&schema)?;
        let raw = self.call(request, &prompt, Some(&schema))?;
        let stats = raw.stats();
        let value = raw.structured_output.ok_or(Error::NoStructuredOutput)?;
        Ok((serde_json::from_value(value)?, stats))
    }

    fn call(&self, request: &Request, prompt: &str, schema: Option<&str>) -> Result<RawResult> {
        let mut cmd = Command::new(&self.bin);
        cmd.args(["-p", "--output-format", "json", "--no-session-persistence"])
            .args(["--tools", "", "--strict-mcp-config", "--disable-slash-commands"])
            // Skips user/project settings, so their hooks and plugins don't run or add context.
            .arg("--setting-sources=")
            // Keeps a project CLAUDE.md out of the context.
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(system) = request.system {
            cmd.args(["--system-prompt", system]);
        }
        if let Some(model) = request.model {
            cmd.args(["--model", model]);
        }
        if let Some(effort) = request.effort {
            cmd.args(["--effort", effort]);
        }
        if let Some(schema) = schema {
            cmd.args(["--json-schema", schema]);
        }
        if let Some(budget) = request.max_budget_usd {
            cmd.args(["--max-budget-usd", &budget.to_string()]);
        }

        let mut child = cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes())?;
        }
        let output = child.wait_with_output()?;

        // On API errors the CLI exits non-zero but still prints a JSON result.
        let Ok(raw) = serde_json::from_slice::<RawResult>(&output.stdout) else {
            return Err(Error::Failed {
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        };
        if raw.is_error || !output.status.success() {
            let message = if raw.result.is_empty() { raw.subtype.clone() } else { raw.result };
            return Err(Error::Api { kind: raw.api_error_type.unwrap_or(raw.subtype), message });
        }
        Ok(raw)
    }
}
