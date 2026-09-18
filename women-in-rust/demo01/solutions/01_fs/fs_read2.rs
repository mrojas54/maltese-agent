// fs_read2
//
// fs_read works, but it will cheerfully try to read the empty path `""`, get a
// confusing OS error, and hand that back as an *internal* error — as if the
// server broke. It did not break. The client sent something meaningless, and
// the honest answer is "that is not a valid argument", with the code a client
// can branch on.
//
// This is the same lesson as demo00's tool2, one tool over: refuse bad input
// with `invalid_params` (-32602), before you touch the filesystem.
//
// Be clear about what this is NOT. Rejecting `""` is input validation, not a
// security boundary. `"../../etc/passwd"` is a perfectly well-formed path and
// sails straight through this check. Stopping *that* is the jail, and the jail
// is demo03. One thing at a time.
//
//     cargo run -- run fs_read2
//
// Stuck? cargo run -- hint fs_read2

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

        // Malformed input, refused before any filesystem call. This is not a
        // jail: a well-formed escaping path still passes. That is demo03.
        if path.is_empty() {
            return Err(ErrorData::invalid_params("path must not be empty", None));
        }

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
    use rmcp::model::ErrorCode;

    /// A real path still reads. Validation must not break the happy path.
    #[tokio::test]
    async fn a_real_path_still_reads() {
        let path = format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"));
        let args = Parameters(ReadArgs { path });
        let result = Demo01::new().fs_read(args).await.expect("read succeeds");
        assert!(result.0.content.contains("demo01"));
    }

    /// An empty path is refused with invalid-params — not read, and not
    /// reported as an internal server error.
    #[tokio::test]
    async fn an_empty_path_is_invalid_params() {
        let args = Parameters(ReadArgs {
            path: String::new(),
        });
        match Demo01::new().fs_read(args).await {
            Err(err) => assert_eq!(
                err.code,
                ErrorCode::INVALID_PARAMS,
                "empty path should be invalid-params, got: {err:?}"
            ),
            Ok(ok) => panic!("empty path was read instead of refused: {:?}", ok.0),
        }
    }
}
