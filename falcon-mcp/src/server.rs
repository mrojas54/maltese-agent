//! rmcp ServerHandler that exposes the falcon-mcp tool surface.

use crate::sandbox::Sandbox;
use crate::tools::fs_basic;
use rmcp::{
    Json, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct FalconMcp {
    sandbox: Arc<Sandbox>,
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl FalconMcp {
    pub fn new(sandbox: Sandbox) -> Self {
        Self {
            sandbox: Arc::new(sandbox),
            tool_router: Self::tool_router(),
        }
    }

    /// Read a file from within the sandbox root.
    #[tool(name = "fs_read", description = "Read a file from within the sandbox root. Returns content as a UTF-8 string and byte count.")]
    pub async fn fs_read(
        &self,
        params: Parameters<fs_basic::FsReadArgs>,
    ) -> Result<Json<fs_basic::FsReadResult>, String> {
        fs_basic::fs_read(self.sandbox.clone(), params.0).await
            .map(Json)
            .map_err(|e| e.to_string())
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for FalconMcp {}
