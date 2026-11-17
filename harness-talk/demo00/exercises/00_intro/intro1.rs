// intro1
//
// Welcome. This is a complete, working MCP server: one tool, one transport,
// under a hundred lines. It passes as-is. Read it top to bottom once, because
// every exercise after this one is a piece of it with a hole cut out:
//
//   01_schema     the Rust structs that become the tool's JSON Schema
//   02_tool       the tool itself, then validating what the model sends
//   03_handshake  the `initialize` reply that names this server
//   04_stdio      the transport, and the one rule it has
//
// Try it as a server: `cargo run --bin intro1`, then paste the JSON lines
// from README.md one at a time. Ctrl-D hangs up.
//
// Then `cargo run -- next` to move on.
//
// MCP spec: https://modelcontextprotocol.io    rmcp docs: https://docs.rs/rmcp

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
        // instead of parsing a greeting to nobody.
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
            // report "rmcp". Exercise 03_handshake is about this.
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
