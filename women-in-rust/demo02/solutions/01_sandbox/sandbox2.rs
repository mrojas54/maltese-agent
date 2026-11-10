// sandbox2
//
// A guard nobody calls is decoration. sandbox1 wrote `resolve`; this exercise
// makes the tools go through it.
//
// The two file tools are built the way falcon-mcp builds them: a handler that
// takes the Sandbox, never a bare path (`read_file`, `write_file`), and a thin
// tool method on top that hands its result to `ToolError::classify`. That last
// step is already written. It turns an escape into the JSON-RPC error -32602
// ("what you sent is not allowed") and everything else into an `isError`
// result ("I ran, and could not do it").
//
// Your part is the line in each handler that decides where the path really is.
// The stubs use the line anyone would write first, and it is wrong in a way the
// tests will show you: they plant a secret *next to* the sandbox root, and
// check whether your tools give it away.
//
// What this still does not do: `resolve` only reads spellings (demo03), and
// there is no read-only switch yet (readonly1).
//
//     cargo run -- run sandbox2
//
// Stuck? cargo run -- hint sandbox2

use anyhow::Context as _;
use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::IntoCallToolResult, wrapper::Parameters},
    model::{CallToolResponse, CallToolResult, ContentBlock, ErrorData},
    tool, tool_handler, tool_router, Json, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

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

/// sandbox1's finished `Sandbox`.
#[derive(Debug)]
struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(root: PathBuf) -> anyhow::Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("sandbox root {} not accessible", root.display()))?;
        Ok(Self { root })
    }

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
}

/// How a tool says no: a JSON-RPC error (-32602) for a bad argument, or an
/// `isError` result for "I ran, and could not do it". Already written; see
/// intro1 for the long version.
#[derive(Debug)]
enum ToolError {
    InvalidArgument(String),
    Internal(String),
}

impl ToolError {
    /// Sorts on the message text, as falcon-mcp does, so the wording in
    /// `Sandbox::resolve` and the string here must stay in step. The last test
    /// in this file guards that.
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

fn read_file(sandbox: &Sandbox, args: ReadArgs) -> anyhow::Result<ReadResult> {
    // Through the sandbox: a refusal is an `Err`, and `?` passes it along.
    let path = sandbox.resolve(&args.path)?;

    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", args.path))?;
    Ok(ReadResult { content })
}

fn write_file(sandbox: &Sandbox, args: WriteArgs) -> anyhow::Result<WriteResult> {
    // The same fix, and the one that matters more: a leak only shows the model
    // a file, but an escaping write *changes* one.
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
                       directories. Returns {bytes: number}."
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
impl ServerHandler for Demo02 {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let sandbox = Arc::new(Sandbox::new(std::env::current_dir()?)?);
    let running = Demo02::new(sandbox).serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// <tmp>/root/hello.txt   inside the sandbox
    /// <tmp>/secret.txt       outside it: the file a guard must not leak
    fn setup() -> (tempfile::TempDir, Demo02) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("root");
        std::fs::create_dir(&root).expect("create root");
        std::fs::write(root.join("hello.txt"), "hello from inside the root\n")
            .expect("write hello");
        std::fs::write(tmp.path().join("secret.txt"), "the secret\n").expect("write secret");
        let sandbox = Sandbox::new(root).expect("root exists");
        (tmp, Demo02::new(Arc::new(sandbox)))
    }

    #[tokio::test]
    async fn fs_read_reads_a_file_inside_the_root() {
        let (_tmp, server) = setup();
        let args = Parameters(ReadArgs {
            path: "hello.txt".into(),
        });
        let result = server.fs_read(args).await.expect("read succeeds");
        assert!(result.0.content.contains("hello"), "{:?}", result.0);
    }

    #[tokio::test]
    async fn fs_read_refuses_dotdot() {
        let (_tmp, server) = setup();
        let args = Parameters(ReadArgs {
            path: "../secret.txt".into(),
        });
        match server.fs_read(args).await {
            Err(ToolError::InvalidArgument(msg)) => {
                assert!(msg.contains("escapes"), "message was: {msg}")
            }
            Err(other) => panic!("expected InvalidArgument (-32602), got {other:?}"),
            Ok(ok) => panic!("`..` reached the secret outside the root: {:?}", ok.0),
        }
    }

    #[tokio::test]
    async fn fs_read_refuses_an_absolute_path() {
        let (tmp, server) = setup();
        let secret = tmp.path().join("secret.txt").to_string_lossy().into_owned();
        match server.fs_read(Parameters(ReadArgs { path: secret })).await {
            Err(ToolError::InvalidArgument(_)) => {}
            Err(other) => panic!("expected InvalidArgument (-32602), got {other:?}"),
            Ok(ok) => panic!("an absolute path reached the secret: {:?}", ok.0),
        }
    }

    #[tokio::test]
    async fn fs_write_writes_a_file_inside_the_root() {
        let (tmp, server) = setup();
        let args = Parameters(WriteArgs {
            path: "notes/today.txt".into(),
            content: "noted".into(),
        });
        let result = server.fs_write(args).await.expect("write succeeds");
        assert_eq!(result.0.bytes, 5);
        let written = std::fs::read_to_string(tmp.path().join("root/notes/today.txt"))
            .expect("the file is inside the root");
        assert_eq!(written, "noted");
    }

    #[tokio::test]
    async fn fs_write_refuses_to_write_outside_the_root() {
        let (tmp, server) = setup();
        let args = Parameters(WriteArgs {
            path: "../escaped.txt".into(),
            content: "oops".into(),
        });
        match server.fs_write(args).await {
            Err(ToolError::InvalidArgument(_)) => {}
            Err(other) => panic!("expected InvalidArgument (-32602), got {other:?}"),
            Ok(_) => panic!("`..` let a write out of the root"),
        }
        assert!(
            !tmp.path().join("escaped.txt").exists(),
            "the refused write still created a file outside the root"
        );
    }

    /// Drift guard: a REAL escape error from `resolve` must classify as an
    /// invalid argument. If `resolve`'s wording changes, this fails here
    /// instead of production quietly returning the wrong error kind.
    #[test]
    fn a_real_escape_error_classifies_as_invalid_argument() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sandbox = Sandbox::new(tmp.path().to_path_buf()).expect("root exists");
        let err = sandbox.resolve("../x").expect_err("`..` escapes");
        assert!(matches!(
            ToolError::classify(err),
            ToolError::InvalidArgument(_)
        ));
    }
}
