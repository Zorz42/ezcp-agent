//! One-shot, tool-less calls to the `claude` CLI in print mode, with an optional persistent cache.

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

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
    #[error("claude is not logged in with a claude.ai subscription (auth: {0}); run `claude auth login`")]
    NotSubscription(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Latest model of each family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Fable,
    Opus,
    Sonnet,
    Haiku,
}

impl Model {
    /// The CLI alias.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fable => "fable",
            Self::Opus => "opus",
            Self::Sonnet => "sonnet",
            Self::Haiku => "haiku",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effort {
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl Effort {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }
}

/// Call settings. Unset fields fall back to the CLI's defaults.
#[derive(Debug, Default, Clone)]
pub struct Request {
    /// The instruction; [`Claude::ask_json`] appends its input after it.
    pub prompt: String,
    /// Replaces Claude Code's default system prompt entirely.
    pub system: Option<String>,
    pub model: Option<Model>,
    pub effort: Option<Effort>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
}

/// Accounting for one call. On a cache hit these are the original call's numbers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Stats {
    /// List-price estimate; usage counts against the subscription, nothing is billed.
    pub cost_usd: f64,
    pub usage: Usage,
    pub duration_ms: u64,
    #[serde(skip)]
    pub cached: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct Reply {
    text: String,
    structured: Option<Value>,
    stats: Stats,
}

#[derive(Deserialize)]
struct Entry {
    key: Value,
    reply: Reply,
}

/// A JSON Lines file of entries, appended to and loaded whole.
struct Cache {
    path: PathBuf,
    /// Keyed by the key's JSON text.
    entries: HashMap<String, Reply>,
    /// The file ends in a line cut short, which the next entry must not be appended to.
    torn: bool,
}

impl Cache {
    fn open(path: PathBuf) -> Result<Self> {
        let text = match fs::read_to_string(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            result => result?,
        };
        // Skipping unparsable lines drops a last line cut short by a crash.
        let entries = text
            .lines()
            .filter_map(|line| serde_json::from_str::<Entry>(line).ok())
            .map(|entry| (entry.key.to_string(), entry.reply))
            .collect();
        let torn = !text.is_empty() && !text.ends_with('\n');
        Ok(Self { path, entries, torn })
    }

    fn get(&self, key: &Value) -> Option<Reply> {
        let mut reply = self.entries.get(&key.to_string())?.clone();
        reply.stats.cached = true;
        Some(reply)
    }

    fn put(&mut self, key: &Value, reply: &Reply) -> Result<()> {
        let mut line = if self.torn { "\n".to_owned() } else { String::new() };
        line += &serde_json::to_string(&serde_json::json!({ "key": key, "reply": reply }))?;
        line.push('\n');
        fs::OpenOptions::new().create(true).append(true).open(&self.path)?.write_all(line.as_bytes())?;
        self.torn = false;
        self.entries.insert(key.to_string(), reply.clone());
        Ok(())
    }
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthStatus {
    #[serde(default)]
    logged_in: bool,
    #[serde(default)]
    auth_method: String,
}

/// A `claude` command that can only authenticate through the claude.ai login.
fn command(bin: &Path) -> Command {
    let mut cmd = Command::new(bin);
    cmd.env_remove("ANTHROPIC_API_KEY").env_remove("ANTHROPIC_AUTH_TOKEN");
    cmd
}

fn run(cmd: &mut Command) -> Result<String> {
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(Error::Failed {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub struct Claude {
    bin: PathBuf,
    version: String,
    cache: Option<Mutex<Cache>>,
}

impl Claude {
    /// Locates `claude` (or `$CLAUDE_PATH`) and verifies it runs and is logged in with a subscription.
    pub fn new() -> Result<Self> {
        let bin = std::env::var_os("CLAUDE_PATH").map_or_else(|| PathBuf::from("claude"), PathBuf::from);
        let version = match run(command(&bin).arg("--version")) {
            Err(Error::Io(e)) if e.kind() == io::ErrorKind::NotFound => return Err(Error::NotFound(bin)),
            result => result?,
        };
        let auth: AuthStatus = serde_json::from_str(&run(command(&bin).args(["auth", "status"]))?)?;
        if !auth.logged_in || auth.auth_method != "claude.ai" {
            let method = if auth.logged_in { auth.auth_method } else { "none".to_owned() };
            return Err(Error::NotSubscription(method));
        }
        Ok(Self { bin, version, cache: None })
    }

    /// Serves repeated requests from the file at `path`, which successful replies are appended to.
    pub fn with_cache(mut self, path: impl Into<PathBuf>) -> Result<Self> {
        self.cache = Some(Mutex::new(Cache::open(path.into())?));
        Ok(self)
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Plain text in, plain text out.
    pub fn ask(&self, request: &Request) -> Result<(String, Stats)> {
        let reply = self.call(request, &request.prompt, None)?;
        Ok((reply.text, reply.stats))
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
        let reply = self.call(request, &prompt, Some(&schema))?;
        let value = reply.structured.ok_or(Error::NoStructuredOutput)?;
        Ok((serde_json::from_value(value)?, reply.stats))
    }

    fn call(&self, request: &Request, prompt: &str, schema: Option<&str>) -> Result<Reply> {
        let Some(cache) = &self.cache else {
            return self.invoke(request, prompt, schema);
        };
        let key = serde_json::json!({
            "model": request.model.map(Model::as_str),
            "effort": request.effort.map(Effort::as_str),
            "system": request.system,
            "schema": schema,
            "prompt": prompt,
        });
        // Not held during the call, so calls on several threads run in parallel.
        let lock = || cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(reply) = lock().get(&key) {
            return Ok(reply);
        }
        let reply = self.invoke(request, prompt, schema)?;
        lock().put(&key, &reply)?;
        Ok(reply)
    }

    fn invoke(&self, request: &Request, prompt: &str, schema: Option<&str>) -> Result<Reply> {
        let mut cmd = command(&self.bin);
        cmd.args(["-p", "--output-format", "json", "--no-session-persistence"])
            .args(["--tools", "", "--strict-mcp-config", "--disable-slash-commands"])
            // Skips user/project settings, so their hooks and plugins don't run or add context.
            .arg("--setting-sources=")
            // Keeps a project CLAUDE.md out of the context.
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(system) = &request.system {
            cmd.args(["--system-prompt", system]);
        }
        if let Some(model) = request.model {
            cmd.args(["--model", model.as_str()]);
        }
        if let Some(effort) = request.effort {
            cmd.args(["--effort", effort.as_str()]);
        }
        if let Some(schema) = schema {
            cmd.args(["--json-schema", schema]);
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
        Ok(Reply {
            text: raw.result,
            structured: raw.structured_output,
            stats: Stats {
                cost_usd: raw.total_cost_usd,
                usage: raw.usage,
                duration_ms: raw.duration_ms,
                cached: false,
            },
        })
    }
}
