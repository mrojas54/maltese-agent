// exec_run1
//
// The tool that ends the "is this dangerous?" debate. `exec_run` takes a
// command name and a list of arguments, runs it, and returns what it printed.
// In demo01 the command name is not checked against anything, so this one tool
// is a general-purpose shell for whoever is driving the model.
//
// Two small pieces of hygiene are worth keeping even here, because they are
// about correctness, not security:
//
//   - `.stdin(Stdio::null())`: the server's own stdin is the JSON-RPC wire. A
//     child that inherited it could read the client's frames. Give it nothing.
//   - `.output()`: captures stdout and stderr instead of letting them inherit
//     the server's stdout, which is also the wire.
//
// What's missing — an allowlist, a timeout, no subshell — is demo04.
//
//     cargo run -- run exec_run1
//
// Stuck? cargo run -- hint exec_run1

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router, ErrorData, Json, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::process::Stdio;

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
impl ServerHandler for Demo01 {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let running = Demo01::new().serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Running a command returns its output and exit code. `rustc --version`
    /// is on PATH wherever cargo is, and prints a line starting with "rustc".
    #[tokio::test]
    async fn runs_a_command_and_captures_output() {
        let args = Parameters(ExecArgs {
            cmd: "rustc".to_string(),
            args: vec!["--version".to_string()],
        });
        let result = Demo01::new().exec_run(args).await.expect("exec succeeds");
        assert_eq!(result.0.exit, 0, "stderr was: {}", result.0.stderr);
        assert!(
            result.0.stdout.contains("rustc"),
            "stdout was: {}",
            result.0.stdout
        );
    }
}
