// intro1
//
// demo01 ended at the crime scene: a server that reads any file and runs any
// command. This is that server after demo02's guards. The tools are the same,
// plus one, with a Sandbox between the file tools and the disk:
//
//   Sandbox    a root directory and a read-only switch, shared as an Arc
//   fs_read    asks the Sandbox where a path really is before it reads
//   fs_write   NEW. Asks the Sandbox too, and is refused under --read-only
//   ToolError  how a refusal reaches the client: two kinds, explained below
//   --root, --read-only   the two flags that build the Sandbox
//
// It passes as-is. Read it top to bottom; every exercise after this is a
// piece of it:
//
//   01_sandbox   the first path check, then the tools that ask it
//   02_readonly  the write switch, and the gate on fs_write
//
// Be clear about what this is. It refuses `../secret` and it refuses
// `/etc/passwd`, and that is nearly all it refuses. The check reads a path's
// *spelling* and never looks at the filesystem, so a symlink inside the root
// that points outside it walks straight through. And `exec_run` is exactly as
// open as it was in demo01: notice that `run_command` takes no Sandbox at all.
// `sh` still runs, it runs in the server's working directory rather than the
// root, and a shell can write files while --read-only is on. Those are demo03
// and demo04. demo02 is the first guard, not the last.
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
use std::path::{Component, Path, PathBuf};
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
/// about the field, so it now says what the guard enforces.
#[derive(Debug, Deserialize, JsonSchema)]
struct ReadArgs {
    /// Path to read, relative to the sandbox root. Paths that climb out of the
    /// root (`..`) or start at the filesystem root (`/`) are refused.
    path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ReadResult {
    content: String,
}

/// What `fs_write` takes. Same rule for the path as `fs_read`.
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
        // Canonicalize the root once, here: `resolve` builds every path from
        // it, so it should be a real, absolute path with no symlinks in it.
        let root = root
            .canonicalize()
            .with_context(|| format!("sandbox root {} not accessible", root.display()))?;
        Ok(Self { root, read_only })
    }

    fn root(&self) -> &Path {
        &self.root
    }

    /// Turn a path from the model into a path on disk, or refuse it.
    ///
    /// This is a *lexical* check: it looks at the components of the path as
    /// written and refuses the ones that can leave the root. It never asks the
    /// filesystem anything, which is what makes it cheap and also what makes
    /// it not enough. demo03 replaces it.
    fn resolve(&self, rel: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let rel = rel.as_ref();
        for component in rel.components() {
            match component {
                // `..` climbs out. A leading `/` (or `C:\` on Windows) is worse:
                // `Path::join` with an absolute path *replaces* the base, so
                // `root.join("/etc/passwd")` is just `/etc/passwd`.
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    anyhow::bail!(
                        "path {} escapes sandbox root {}",
                        rel.display(),
                        self.root.display()
                    );
                }
                Component::CurDir | Component::Normal(_) => {}
            }
        }
        Ok(self.root.join(rel))
    }

    /// The second guard, a switch: under `--read-only` nothing may write.
    fn check_writable(&self) -> anyhow::Result<()> {
        if self.read_only {
            anyhow::bail!("sandbox is read-only");
        }
        Ok(())
    }
}

/// How a tool says no. Two kinds, and they reach the client differently
/// because they mean different things:
///
///   InvalidArgument  a JSON-RPC *error*, code -32602: "what you sent is not
///                    allowed". There is no `result`. `../secret` lands here.
///   Internal         a *result* with `isError: true`: "I ran, and I could not
///                    do it". A write under --read-only lands here, and so
///                    does a file that is not there.
///
/// rmcp decides which by asking the error type to turn itself into a reply
/// (`IntoCallToolResult`). falcon-mcp has this same enum; demo04 adds a
/// `Timeout` kind to it.
#[derive(Debug)]
enum ToolError {
    InvalidArgument(String),
    Internal(String),
}

impl ToolError {
    /// Sort an error from a handler into a kind. This matches on the message
    /// text, exactly as falcon-mcp does, so `Sandbox::resolve`'s wording and
    /// this string have to stay in step. The tests build a real escape error
    /// and classify it, so a rewording fails loudly there.
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
/// how a tool goes through the guard.
fn read_file(sandbox: &Sandbox, args: ReadArgs) -> anyhow::Result<ReadResult> {
    let path = sandbox.resolve(&args.path)?;
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", args.path))?;
    Ok(ReadResult { content })
}

/// The work behind `fs_write`. The read-only gate comes first, so a refused
/// write never gets as far as touching a path.
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

/// The work behind `exec_run`, and the tell: no `Sandbox` in the signature.
/// Nothing here can be refused, because nothing here can be asked.
/// `Stdio::null()` on stdin is the one piece of hygiene it keeps: a child that
/// inherited our stdin would be reading the JSON-RPC wire.
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
struct Demo02 {
    sandbox: Arc<Sandbox>,
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl Demo02 {
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
impl ServerHandler for Demo02 {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "An MCP server with greet, fs_read, fs_write, and exec_run. The file tools stay \
                 inside the sandbox root; exec_run is not guarded yet.",
            )
    }
}

#[derive(Parser, Debug)]
#[command(about = "demo02: an MCP server whose file tools go through a Sandbox")]
struct Args {
    /// Sandbox root: every file path resolves inside this directory.
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

    let running = Demo02::new(sandbox).serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}
