// handshake1
//
// Before any tool call, the client sends `initialize` and the server answers
// with who it is and what it can do. rmcp builds that reply from
// `ServerHandler::get_info`. The default implementation is wrong for you in
// two ways, and the tests check both:
//
//   - it advertises no capabilities, so a careful client will never ask
//     for your tools;
//   - it does not name this server. `Implementation::from_build_env()` reads
//     `CARGO_PKG_NAME` with `env!`, and `env!` expands where it is written,
//     which was inside rmcp when rmcp was compiled.
//
// The override below enables the tools capability and sets the server info
// from `env!` written in *this* file, where it expands to this package's
// values.
//
//     cargo run -- run handshake1

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
            // `env!` expands here, so these are this package's name and
            // version. The same macro inside rmcp's `from_build_env()`
            // expanded to rmcp's.
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions("A hello-world MCP server. Call `greet` with a name.")
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let running = Demo00::new().serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A client only asks for tools from a server that says it has them.
    #[test]
    fn handshake_advertises_tools() {
        let info = Demo00::new().get_info();
        assert!(
            info.capabilities.tools.is_some(),
            "enable the tools capability: {info:?}"
        );
    }

    /// The name in the handshake is what shows up in the client's server
    /// list. It should be this package's, not the SDK's.
    #[test]
    fn handshake_names_this_package() {
        let info = Demo00::new().get_info();
        assert_eq!(
            info.server_info.name,
            env!("CARGO_PKG_NAME"),
            "the client should see this package's name"
        );
    }
}
