// tool1
//
// A tool is an `async fn` on the server struct, marked `#[tool]`. Two things
// in it are yours to write:
//
//   - the `description`: the only documentation the model ever reads about
//     this tool. It decides whether, and how, the model calls it. Write it
//     for the model, not for a person;
//   - the body, which receives `Parameters<GreetArgs>` (already parsed and
//     schema-checked by rmcp) and returns `Json<GreetResult>` or an error.
//
// `#[tool_router]` collects every `#[tool]` in the impl block into a
// `ToolRouter`, which is what answers `tools/list`. `#[tool_handler]` wires
// that router into the `ServerHandler` trait. You will not need to touch
// either.
//
//     cargo run -- run tool1

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

/// MCP requires a tool's structured output to be a JSON object, so even a
/// single string gets a named field.
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

    /// The description is prose for the model: what the tool does and what
    /// it returns. `Json(...)` serializes the struct into the reply's
    /// `structuredContent`, using the schema derived from `GreetResult`.
    #[tool(
        name = "greet",
        description = "Greet someone by name. Returns {greeting: string}."
    )]
    async fn greet(&self, params: Parameters<GreetArgs>) -> Result<Json<GreetResult>, ErrorData> {
        let name = params.0.name;
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

    /// The router lists the tool by name, with a description for the model.
    #[test]
    fn greet_is_listed_with_a_description() {
        let tools = Demo00::new().tool_router.list_all();
        let greet = tools
            .iter()
            .find(|t| t.name == "greet")
            .expect("a tool named greet");
        let description = greet.description.as_deref().unwrap_or("");
        assert!(
            !description.trim().is_empty(),
            "the model needs a description to know when to call greet"
        );
    }

    /// Calling the tool in-process, exactly as rmcp will after it parses a
    /// `tools/call` request.
    #[tokio::test]
    async fn greet_mentions_the_name() {
        let args = Parameters(GreetArgs {
            name: "Ada".to_string(),
        });
        let result = Demo00::new().greet(args).await.expect("greet succeeds");
        assert!(
            result.0.greeting.contains("Ada"),
            "greeting should mention Ada: {}",
            result.0.greeting
        );
    }
}
