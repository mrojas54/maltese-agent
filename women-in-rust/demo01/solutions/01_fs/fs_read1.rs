// fs_read1
//
// The first tool that touches the world outside the process. Its whole job is
// to take a path and return the file's contents to the model.
//
// In demo01 that is all it does — no root, no jail, no check. The lesson here
// is deliberately small: a tool is a function, and a tool that reads files is
// a function that reads files. The danger is not in the code you are about to
// write; it is in what you are *not* writing, and demo02 is where that gets
// fixed.
//
//     cargo run -- run fs_read1
//
// Stuck? cargo run -- hint fs_read1

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router, ErrorData, Json, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
struct ReadArgs {
    /// Path to read. Resolved against the working directory, unchecked.
    path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ReadResult {
    content: String,
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
        name = "fs_read",
        description = "Read a UTF-8 text file by path. Returns {content: string}."
    )]
    async fn fs_read(&self, params: Parameters<ReadArgs>) -> Result<Json<ReadResult>, ErrorData> {
        let path = params.0.path;
        let content = std::fs::read_to_string(&path)
            .map_err(|e| ErrorData::internal_error(format!("reading {path}: {e}"), None))?;
        Ok(Json(ReadResult { content }))
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

    /// Reading a file that exists returns its contents. The crate's own
    /// Cargo.toml is a file we know is there and know a string inside of.
    #[tokio::test]
    async fn reads_a_file_that_exists() {
        let path = format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"));
        let args = Parameters(ReadArgs { path });
        let result = Demo01::new().fs_read(args).await.expect("read succeeds");
        assert!(
            result.0.content.contains("demo01"),
            "expected the crate's Cargo.toml, got: {}",
            result.0.content
        );
    }
}
