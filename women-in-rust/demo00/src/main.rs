//! demo00 — the smallest MCP server a real client will talk to.
//!
//! One tool (`greet`), one transport (stdio), no sandbox. Every later demo
//! (root jail, binary allowlist, timeouts) hangs off this skeleton, so read
//! it top to bottom once: types → tool → handshake → main.
//!
//! The single rule that matters on stdio: **stdout is the wire.** Every byte
//! written there must be a JSON-RPC frame, so all logging goes to stderr.
//! `tests/wire_test.rs` talks raw JSON to this binary and pins that rule.

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData, Json, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What the model sends us. `JsonSchema` turns this struct into the
/// `inputSchema` the client shows the model, so the Rust type *is* the API
/// contract: a missing or mistyped `name` is rejected before `greet` runs.
#[derive(Debug, Deserialize, JsonSchema)]
struct GreetArgs {
    /// Who to greet.
    name: String,
}

/// What we send back. MCP requires a tool's structured output to be a JSON
/// object, so even a single string gets a named field.
#[derive(Debug, Serialize, JsonSchema)]
struct GreetResult {
    greeting: String,
}

/// The server. `ToolRouter` is filled in by `#[tool_router]` from every
/// `#[tool]` method below; that is how `tools/list` knows what exists.
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

    /// The `description` string is the only documentation the model ever
    /// reads about this tool. Write it for the model, not for humans.
    #[tool(
        name = "greet",
        description = "Greet someone by name. Returns {greeting: string}."
    )]
    async fn greet(&self, params: Parameters<GreetArgs>) -> Result<Json<GreetResult>, ErrorData> {
        let name = params.0.name;

        // The schema guarantees `name` is a string; it does not guarantee
        // the string means anything. Reject the empty case with a JSON-RPC
        // error (code -32602, invalid params) so the client can branch on it
        // instead of parsing a greeting to nobody. This is the same surface
        // falcon-mcp uses for a path that escapes the sandbox.
        if name.trim().is_empty() {
            return Err(ErrorData::invalid_params("name must not be empty", None));
        }

        Ok(Json(GreetResult {
            greeting: format!("Hello, {name}! Welcome to Women in Rust."),
        }))
    }
}

/// `#[tool_handler]` generates `list_tools` and `call_tool` from the router.
/// `get_info` is the `initialize` handshake reply: it tells the client which
/// protocol version we speak and that we serve tools (and nothing else).
#[tool_handler(router = self.tool_router)]
impl ServerHandler for Demo00 {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            // `env!` expands here, in *this* crate, so the client sees
            // "demo00". rmcp's own `Implementation::from_build_env()` would
            // report "rmcp": that macro ran when rmcp was compiled, not us.
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

    tracing::info!("demo00 listening on stdio");

    // `serve` runs the handshake, then loops: read a frame from stdin,
    // dispatch it, write the reply to stdout. `waiting` returns when the
    // client closes our stdin.
    let running = Demo00::new().serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}
