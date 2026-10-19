// readonly1
//
// The second guard is a switch. Start the server with `--read-only` and it
// refuses every write, while reads carry on. That is one boolean on the Sandbox
// and one question the write handler asks before it does anything else:
//
//   Sandbox::check_writable   Ok normally; an error when the sandbox is read-only
//   write_file                asks check_writable() first
//
// Two TODOs, and the tests fail differently if you do only one. A switch that
// no tool checks is decoration, and the tool-level test catches that. A check
// that never says no is decoration too, and the unit test catches that.
//
// Look at what a refusal turns into. It is a *result* with `isError: true`,
// not a JSON-RPC error: the tool ran, and the honest answer is "I can't do that
// here". A bad path is different. That is the client's mistake, and it gets
// -32602. `ToolError` keeps the two apart, and it is already written.
//
// Once the tests pass, the runner starts this server with `--read-only` for
// real and talks to it over stdio: reads must still work, and a refused write
// must leave nothing behind.
//
// And be clear about what this does NOT stop. The switch guards the tools that
// ask it. `exec_run` has no idea it exists, so `sh -c 'echo x > y'` writes a
// file under --read-only just fine. Closing that is demo04.
//
//     cargo run -- run readonly1
//
// Stuck? cargo run -- hint readonly1

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
use std::sync::Arc;

#[derive(Debug, Deserialize, JsonSchema)]
struct ReadArgs {
    /// Path to read, relative to the sandbox root.
    path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ReadResult {
    content: String,
}

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

/// The Sandbox now carries its second field: the read-only switch.
#[derive(Debug)]
struct Sandbox {
    root: PathBuf,
    // Nothing reads this yet. TODO 1 below is what does.
    #[allow(dead_code)]
    read_only: bool,
}

impl Sandbox {
    fn new(root: PathBuf, read_only: bool) -> anyhow::Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("sandbox root {} not accessible", root.display()))?;
        Ok(Self { root, read_only })
    }

    /// sandbox1's finished lexical check.
    fn resolve(&self, rel: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let rel = rel.as_ref();
        for component in rel.components() {
            match component {
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

    /// The switch: under `--read-only` nothing may write.
    // Nothing outside the tests calls this yet. TODO 2 is what does.
    #[allow(dead_code)]
    fn check_writable(&self) -> anyhow::Result<()> {
        // TODO 1: say no when the sandbox is read-only.
        //
        //     if self.read_only {
        //         anyhow::bail!("sandbox is read-only");
        //     }
        //
        // The tests, and the wire probe, look for the words "read-only". As
        // written this never refuses anything, so the switch is wired to
        // nothing.
        Ok(())
    }
}

/// How a tool says no: a JSON-RPC error (-32602) for a bad argument, or an
/// `isError` result for "I ran, and could not do it". A read-only refusal is
/// the second kind. Already written; see intro1 for the long version.
#[derive(Debug)]
enum ToolError {
    InvalidArgument(String),
    Internal(String),
}

impl ToolError {
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
            Self::Internal(message) => {
                Ok(CallToolResult::error(vec![ContentBlock::text(message)]).into())
            }
        }
    }
}

/// sandbox2's finished read handler. Reads never need the switch.
fn read_file(sandbox: &Sandbox, args: ReadArgs) -> anyhow::Result<ReadResult> {
    let path = sandbox.resolve(&args.path)?;
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", args.path))?;
    Ok(ReadResult { content })
}

fn write_file(sandbox: &Sandbox, args: WriteArgs) -> anyhow::Result<WriteResult> {
    // TODO 2: the gate. Ask the sandbox whether writing is allowed *before*
    // anything else happens, so a refused write never gets as far as touching
    // a path:
    //
    //     sandbox.check_writable()?;

    let path = sandbox.resolve(&args.path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("creating parent directories")?;
    }
    std::fs::write(&path, &args.content).with_context(|| format!("writing {}", args.path))?;
    Ok(WriteResult {
        bytes: args.content.len(),
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
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Demo02 {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
        )
    }
}

#[derive(Parser, Debug)]
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
    let args = Args::parse();
    let sandbox = Arc::new(Sandbox::new(args.root, args.read_only)?);
    let running = Demo02::new(sandbox).serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch root holding one file, with the switch set as asked.
    fn setup(read_only: bool) -> (tempfile::TempDir, Demo02) {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("hello.txt"), "hello from inside the root\n")
            .expect("write hello");
        let sandbox = Sandbox::new(tmp.path().to_path_buf(), read_only).expect("root exists");
        (tmp, Demo02::new(Arc::new(sandbox)))
    }

    fn note() -> Parameters<WriteArgs> {
        Parameters(WriteArgs {
            path: "notes.txt".into(),
            content: "noted".into(),
        })
    }

    #[test]
    fn a_writable_sandbox_allows_writes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sandbox = Sandbox::new(tmp.path().to_path_buf(), false).expect("root exists");
        sandbox
            .check_writable()
            .expect("not read-only, so writable");
    }

    #[test]
    fn a_read_only_sandbox_refuses_writes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sandbox = Sandbox::new(tmp.path().to_path_buf(), true).expect("root exists");
        let err = sandbox
            .check_writable()
            .expect_err("read-only must refuse a write");
        assert!(err.to_string().contains("read-only"), "got: {err}");
    }

    #[tokio::test]
    async fn fs_write_is_refused_when_read_only() {
        let (tmp, server) = setup(true);
        match server.fs_write(note()).await {
            Err(ToolError::Internal(msg)) => {
                assert!(msg.contains("read-only"), "message was: {msg}")
            }
            Err(other) => panic!("expected the read-only refusal, got {other:?}"),
            Ok(_) => panic!("fs_write wrote a file in a read-only sandbox"),
        }
        assert!(
            !tmp.path().join("notes.txt").exists(),
            "the refused write still left a file behind"
        );
    }

    #[tokio::test]
    async fn reads_still_work_when_read_only() {
        let (_tmp, server) = setup(true);
        let args = Parameters(ReadArgs {
            path: "hello.txt".into(),
        });
        let result = server.fs_read(args).await.expect("reads are not writes");
        assert!(result.0.content.contains("hello"), "{:?}", result.0);
    }

    #[tokio::test]
    async fn fs_write_still_writes_when_writable() {
        let (tmp, server) = setup(false);
        let result = server.fs_write(note()).await.expect("write succeeds");
        assert_eq!(result.0.bytes, 5);
        let written = std::fs::read_to_string(tmp.path().join("notes.txt")).expect("file exists");
        assert_eq!(written, "noted");
    }
}
