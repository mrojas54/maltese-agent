// intro1
//
// demo00 built a server with one tool: greet. This is that server given two
// more, and they are the reason this whole talk exists.
//
//   fs_read   reads a file and hands its contents to the model
//   exec_run  runs a command and hands back its output
//
// Both do exactly what they say, to any path and any command, with nothing
// standing in the way. `fs_read` will read `../../../../etc/passwd`. `exec_run`
// will run `sh`. This server, pointed at a model, is a remote shell with extra
// steps. That is the crime scene.
//
// It passes as-is, because "works" and "safe" are different claims and demo01
// only makes the first one. Read it top to bottom; every exercise after this
// is a piece of it:
//
//   01_fs    the fs_read tool, then refusing empty input
//   02_exec  the exec_run tool, which shells out
//
// Then `cargo run -- next`.
//
// The boundaries arrive in demo02: a `Sandbox` that every tool goes through,
// a `--read-only` mode, and a first check on the paths coming in.

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData, Json, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::process::Stdio;

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

/// What `fs_read` takes. One field, and the doc comment is the only thing the
/// model reads about it — so it is where the danger is documented, not fixed.
#[derive(Debug, Deserialize, JsonSchema)]
struct ReadArgs {
    /// Path to read. Resolved against the process's working directory with no
    /// checks at all: a leading `/` or a `..` reads straight out of it.
    path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ReadResult {
    content: String,
}

/// What `exec_run` takes. There is no allowlist in demo01, so `cmd` can be
/// anything on PATH: `git`, but also `sh`, `curl`, `rm`.
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

#[derive(Clone)]
struct Demo01 {
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl Demo01 {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "greet",
        description = "Greet someone by name. Returns {greeting: string}."
    )]
    async fn greet(&self, params: Parameters<GreetArgs>) -> Result<Json<GreetResult>, ErrorData> {
        let name = params.0.name;
        if name.trim().is_empty() {
            return Err(ErrorData::invalid_params("name must not be empty", None));
        }
        Ok(Json(GreetResult {
            greeting: format!("Hello, {name}! Welcome to Women in Rust."),
        }))
    }

    /// Read a file. The whole body is one `std::fs` call. Nothing here knows
    /// what a "sandbox root" is; `path` goes to the OS as written.
    #[tool(
        name = "fs_read",
        description = "Read a UTF-8 text file by path. Returns {content: string}."
    )]
    async fn fs_read(&self, params: Parameters<ReadArgs>) -> Result<Json<ReadResult>, ErrorData> {
        let path = params.0.path;
        let content = std::fs::read_to_string(&path)
            .map_err(|e| ErrorData::internal_error(format!("reading {path}: {e}"), None))?;
        Ok(Json(ReadResult { content }))
    }

    /// Run a command and capture its output. `Stdio::null()` on stdin is the
    /// one piece of hygiene demo01 does keep: a child that inherited our stdin
    /// would be reading the JSON-RPC wire. `output()` captures stdout/stderr,
    /// so neither reaches the wire either. Everything *else* about this tool is
    /// wide open, which is the point.
    #[tool(
        name = "exec_run",
        description = "Run a command with arguments. Returns {stdout, stderr, exit}."
    )]
    async fn exec_run(&self, params: Parameters<ExecArgs>) -> Result<Json<ExecResult>, ErrorData> {
        let ExecArgs { cmd, args } = params.0;
        let out = tokio::process::Command::new(&cmd)
            .args(&args)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| ErrorData::internal_error(format!("running {cmd}: {e}"), None))?;
        Ok(Json(ExecResult {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            exit: out.status.code().unwrap_or(-1),
        }))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Demo01 {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "An MCP server with greet, fs_read, and exec_run. No boundaries yet.",
            )
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("listening on stdio");

    let running = Demo01::new().serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}
