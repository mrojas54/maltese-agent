// tool2
//
// The schema guarantees `name` is a string. It does not guarantee the string
// means anything: `""` and `"   "` both pass. A tool that greets nobody is a
// small bug here and a large one in a tool that writes files.
//
// MCP gives a rejected call two possible shapes, and the difference is the
// point of this exercise:
//
//   - a JSON-RPC *error* (return `Err(ErrorData)` from the tool): "I refused
//     to run this, and here is a code you can branch on". -32602 is
//     INVALID_PARAMS;
//   - a *result* with `isError: true`: "I ran, and it went wrong".
//
// Bad input is the first kind. falcon-mcp uses exactly this surface when a
// path tries to escape its sandbox.
//
//     cargo run -- run tool2

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router, ErrorData, Json, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
struct GreetArgs {
    /// Who to greet.
    name: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct GreetResult {
    greeting: String,
}

#[derive(Clone)]
struct Demo00 {
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl Demo00 {
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

        // Refuse before doing anything. On the wire this becomes
        // `{"error": {"code": -32602, ...}}` with no `result` at all.
        if name.trim().is_empty() {
            return Err(ErrorData::invalid_params("name must not be empty", None));
        }

        Ok(Json(GreetResult {
            greeting: format!("Hello, {name}! Welcome to Women in Rust."),
        }))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Demo00 {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let running = Demo00::new().serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::ErrorCode;

    /// Validation must not break the happy path.
    #[tokio::test]
    async fn a_real_name_is_still_greeted() {
        let args = Parameters(GreetArgs {
            name: "Ada".to_string(),
        });
        let result = Demo00::new().greet(args).await.expect("greet succeeds");
        assert!(result.0.greeting.contains("Ada"));
    }

    /// A blank name is refused with the invalid-params code, which is what
    /// a client can branch on. It is not greeted, and it is not a result
    /// flagged `isError`.
    #[tokio::test]
    async fn a_blank_name_is_invalid_params() {
        for blank in ["", "   ", "\n"] {
            let args = Parameters(GreetArgs {
                name: blank.to_string(),
            });
            match Demo00::new().greet(args).await {
                Err(err) => assert_eq!(
                    err.code,
                    ErrorCode::INVALID_PARAMS,
                    "wrong code for {blank:?}: {err:?}"
                ),
                Ok(ok) => panic!(
                    "{blank:?} was greeted instead of rejected: {}",
                    ok.0.greeting
                ),
            }
        }
    }
}
