// intro1
//
// demo02 ended with a guard that read a path's spelling and never asked the
// filesystem, so a symlink inside the root walked straight out. This is that
// server after demo03's jail. The tools are the same; one function changed:
//
//   Sandbox::resolve   asks the filesystem now: canonicalize the path, then
//                      check the answer still starts with the root
//   the fallback       a path that does not exist yet (the file fs_write is
//                      about to create) is placed from its nearest existing
//                      ancestor, then checked the same way
//   everything else    as demo02 left it: fs_read, fs_write, --read-only,
//                      ToolError, exec_run
//
// It passes as-is. Read it top to bottom; the exercise after this is the piece
// that changed:
//
//   02_jail   resolve, both halves: a path that exists, and one that does not
//
// Be clear about what this is. `../secret`, `/etc/passwd` and a symlink that
// leads out of the root are all refused now, because the check is on where a
// path really lands, not on how it is spelled. Two things are still open.
// Checking a path and then using it are two steps, and a symlink swapped in
// between them wins the race. And `exec_run` is exactly as open as it was in
// demo01: `run_command` still takes no Sandbox, and `sh` still runs. That is
// demo04.
//
// Then `cargo run -- next`.

use anyhow::Context as _;
use clap::Parser;
use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::IntoCallToolResult, wrapper::Parameters},
    model::{
        CallToolResponse, CallToolResult, ContentBlock, ErrorData, Implementation,
        ServerCapabilities, ServerInfo,
    },
    tool, tool_handler, tool_router, Json, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

/// Who to greet. Unchanged from demo00.
#[derive(Debug, Deserialize, JsonSchema)]
struct GreetArgs {
    /// Who to greet.
    name: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct GreetResult {
    greeting: String,
}

/// What `fs_read` takes. The doc comment is the only thing the model reads
/// about the field, so it says what the jail enforces: where the path lands,
/// not how it is written.
#[derive(Debug, Deserialize, JsonSchema)]
struct ReadArgs {
    /// Path to read, relative to the sandbox root. A path that resolves outside
    /// the root, through `..`, an absolute path, or a symlink, is refused.
    path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ReadResult {
    content: String,
}

/// What `fs_write` takes. Same rule for the path as `fs_read`; the file may
/// not exist yet.
#[derive(Debug, Deserialize, JsonSchema)]
struct WriteArgs {
    /// Path to write, relative to the sandbox root.
    path: String,
    /// UTF-8 text to write to it.
    content: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct WriteResult {
    bytes: usize,
}

/// What `exec_run` takes. Still no allowlist: `cmd` can be anything on PATH,
/// `sh` included. That is demo04.
#[derive(Debug, Deserialize, JsonSchema)]
struct ExecArgs {
    /// Binary to run. Looked up on PATH; not restricted in any way.
    cmd: String,
    /// Arguments passed through verbatim.
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ExecResult {
    stdout: String,
    stderr: String,
    exit: i32,
}

/// The one place that knows where the boundary is. Every file tool goes
/// through it, and it is held in an `Arc` so the server and all of its tools
/// share a single copy.
#[derive(Debug)]
struct Sandbox {
    root: PathBuf,
    read_only: bool,
}

impl Sandbox {
    fn new(root: PathBuf, read_only: bool) -> anyhow::Result<Self> {
        // The root is canonical too, so every comparison in `resolve` is
        // between two paths with no symlinks and no `..` in them.
        let root = root
            .canonicalize()
            .with_context(|| format!("sandbox root {} not accessible", root.display()))?;
        Ok(Self { root, read_only })
    }

    fn root(&self) -> &Path {
        &self.root
    }

    /// Turn a path from the model into a canonical path inside the root, or
    /// refuse it.
    ///
    /// demo02's version read the path's components and never asked the
    /// filesystem. This one asks: where does this path really go? That is
    /// the jail, and it is the whole of exercise 02_jail.
    fn resolve(&self, rel: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        // `join` with an absolute path replaces the root, and that is fine now:
        // the check below is on where the path really ends up.
        let joined = self.root.join(rel.as_ref());

        // A path that exists: canonicalize it and compare.
        if let Ok(canonical) = joined.canonicalize() {
            if !canonical.starts_with(&self.root) {
                anyhow::bail!(
                    "path {} escapes sandbox root {}",
                    canonical.display(),
                    self.root.display()
                );
            }
            return Ok(canonical);
        }

        // A path that does not exist yet: walk up to the first ancestor that
        // does, canonicalize it, and put the names walked past back on. Running
        // out of names (a `..` in the missing tail) is a refusal, never a pass.
        let mut existing = joined.as_path();
        let mut missing = Vec::new();
        let canonical_existing = loop {
            if let Ok(canonical) = existing.canonicalize() {
                break canonical;
            }
            // A broken symlink fails `canonicalize` too, but it is a link, not
            // a missing name: a write to it lands wherever it points.
            if existing.symlink_metadata().is_ok() {
                anyhow::bail!(
                    "path {} exists but cannot be resolved (a broken symlink?)",
                    existing.display()
                );
            }
            let name = existing
                .file_name()
                .with_context(|| format!("path {} cannot be placed", joined.display()))?;
            missing.push(name);
            existing = existing
                .parent()
                .with_context(|| format!("path {} cannot be placed", joined.display()))?;
        };

        // The names came off the end first, so they go back on in reverse.
        let mut placed = canonical_existing;
        for name in missing.iter().rev() {
            placed.push(name);
        }

        // Check the placed path, not `joined`: only the canonical ancestor shows
        // where a symlink partway down really leads.
        if !placed.starts_with(&self.root) {
            anyhow::bail!(
                "path {} escapes sandbox root {}",
                placed.display(),
                self.root.display()
            );
        }
        Ok(placed)
    }

    /// The read-only switch, unchanged from demo02.
    fn check_writable(&self) -> anyhow::Result<()> {
        if self.read_only {
            anyhow::bail!("sandbox is read-only");
        }
        Ok(())
    }
}

/// How a tool says no, unchanged from demo02:
///
///   InvalidArgument  a JSON-RPC *error*, code -32602: "what you sent is not
///                    allowed". `../secret` and a symlink that leads out both
///                    land here.
///   Internal         a *result* with `isError: true`: "I ran, and I could not
///                    do it". A write under --read-only, a missing file, and a
///                    path that cannot be placed land here.
#[derive(Debug)]
enum ToolError {
    InvalidArgument(String),
    Internal(String),
}

impl ToolError {
    /// Sort an error from a handler into a kind by its message, as falcon-mcp
    /// does. `Sandbox::resolve`'s "escapes sandbox root" and this string have
    /// to stay in step.
    fn classify(err: anyhow::Error) -> Self {
        let message = format!("{err:#}");
        if message.contains("escapes sandbox root") {
            Self::InvalidArgument(message)
        } else {
            Self::Internal(message)
        }
    }
}

impl IntoCallToolResult for ToolError {
    fn into_call_tool_result(self) -> Result<CallToolResponse, ErrorData> {
        match self {
            Self::InvalidArgument(message) => Err(ErrorData::invalid_params(message, None)),
            // rmcp marks the result `isError: true` on a tool's error branch.
            Self::Internal(message) => {
                Ok(CallToolResult::error(vec![ContentBlock::text(message)]).into())
            }
        }
    }
}

/// The work behind `fs_read`. It takes the Sandbox, never a bare path: that is
/// how a tool goes through the jail.
fn read_file(sandbox: &Sandbox, args: ReadArgs) -> anyhow::Result<ReadResult> {
    let path = sandbox.resolve(&args.path)?;
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", args.path))?;
    Ok(ReadResult { content })
}

/// The work behind `fs_write`. The read-only gate comes first, then the jail.
/// `resolve` places a file that does not exist yet, so a new file can be
/// written and still cannot be written outside the root.
fn write_file(sandbox: &Sandbox, args: WriteArgs) -> anyhow::Result<WriteResult> {
    sandbox.check_writable()?;
    let path = sandbox.resolve(&args.path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("creating parent directories")?;
    }
    std::fs::write(&path, &args.content).with_context(|| format!("writing {}", args.path))?;
    Ok(WriteResult {
        bytes: args.content.len(),
    })
}

/// The work behind `exec_run`, still with no `Sandbox` in the signature. The
/// jail guards paths that the file tools hand it; a command never asks.
/// `Stdio::null()` on stdin keeps a child off the JSON-RPC wire.
async fn run_command(args: ExecArgs) -> anyhow::Result<ExecResult> {
    let ExecArgs { cmd, args } = args;
    let out = tokio::process::Command::new(&cmd)
        .args(&args)
        .stdin(Stdio::null())
        .output()
        .await
        .with_context(|| format!("running {cmd}"))?;
    Ok(ExecResult {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        exit: out.status.code().unwrap_or(-1),
    })
}

#[derive(Clone)]
struct Demo03 {
    sandbox: Arc<Sandbox>,
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl Demo03 {
    fn new(sandbox: Arc<Sandbox>) -> Self {
        Self {
            sandbox,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "greet",
        description = "Greet someone by name. Returns {greeting: string}."
    )]
    async fn greet(&self, params: Parameters<GreetArgs>) -> Result<Json<GreetResult>, ToolError> {
        let name = params.0.name;
        if name.trim().is_empty() {
            return Err(ToolError::InvalidArgument(
                "name must not be empty".to_string(),
            ));
        }
        Ok(Json(GreetResult {
            greeting: format!("Hello, {name}! Welcome to Women in Rust."),
        }))
    }

    #[tool(
        name = "fs_read",
        description = "Read a UTF-8 text file inside the sandbox root. Path is relative to the \
                       root. Returns {content: string}."
    )]
    async fn fs_read(&self, params: Parameters<ReadArgs>) -> Result<Json<ReadResult>, ToolError> {
        read_file(&self.sandbox, params.0)
            .map(Json)
            .map_err(ToolError::classify)
    }

    #[tool(
        name = "fs_write",
        description = "Write a UTF-8 text file inside the sandbox root, creating parent \
                       directories. Refused when the server runs with --read-only. \
                       Returns {bytes: number}."
    )]
    async fn fs_write(
        &self,
        params: Parameters<WriteArgs>,
    ) -> Result<Json<WriteResult>, ToolError> {
        write_file(&self.sandbox, params.0)
            .map(Json)
            .map_err(ToolError::classify)
    }

    #[tool(
        name = "exec_run",
        description = "Run a command with arguments. Returns {stdout, stderr, exit}."
    )]
    async fn exec_run(&self, params: Parameters<ExecArgs>) -> Result<Json<ExecResult>, ToolError> {
        run_command(params.0)
            .await
            .map(Json)
            .map_err(ToolError::classify)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Demo03 {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "An MCP server with greet, fs_read, fs_write, and exec_run. The file tools stay \
                 inside the sandbox root, symlinks included; exec_run is not guarded yet.",
            )
    }
}

#[derive(Parser, Debug)]
#[command(about = "demo03: an MCP server whose file tools are jailed to a root")]
struct Args {
    /// Sandbox root: every file path must resolve inside this directory.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// Refuse every write. Reads still work.
    #[arg(long)]
    read_only: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    let sandbox = Arc::new(Sandbox::new(args.root, args.read_only)?);
    tracing::info!(
        root = %sandbox.root().display(),
        read_only = args.read_only,
        "listening on stdio"
    );

    let running = Demo03::new(sandbox).serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}
