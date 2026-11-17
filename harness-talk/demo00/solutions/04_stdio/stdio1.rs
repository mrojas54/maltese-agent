// stdio1
//
// The transport. An MCP server on stdio reads one JSON-RPC frame per line
// from stdin and writes one per line to stdout. That gives stdio exactly one
// rule, and it is the rule this whole workshop keeps coming back to:
//
//     stdout is the wire. Every byte on it must be a frame.
//
// A stray `println!` becomes a corrupt frame and the client disconnects,
// usually with an error that blames JSON parsing rather than you. Diagnostics
// go to stderr, which the client ignores or shows as logs.
//
// `eprintln!` would have been the one-word fix. This solution uses `tracing`
// pointed at stderr instead, which is what falcon-mcp does: the same
// `RUST_LOG` filter then controls every log line in the server.
//
//     cargo run -- run stdio1

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
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
        if name.trim().is_empty() {
            return Err(ErrorData::invalid_params("name must not be empty", None));
        }
        Ok(Json(GreetResult {
            greeting: format!("Hello, {name}! Welcome to Women in Rust."),
        }))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Demo00 {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions("A hello-world MCP server. Call `greet` with a name.")
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logs go to stderr. Writing them to stdout would corrupt the JSON-RPC
    // stream and the client would disconnect. `RUST_LOG=info` to see them.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("listening on stdio");

    // `serve` runs the handshake, then loops: read a frame from stdin,
    // dispatch it, write the reply to stdout. `waiting` returns when the
    // client closes our stdin.
    let running = Demo00::new().serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}
