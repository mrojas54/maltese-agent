//! Filesystem tools (basic): read, write, list, search, apply_patch.
//! Task 3 implements only `fs_read`; the rest land in Tasks 4-6.

use crate::sandbox::Sandbox;
use anyhow::Context;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FsReadArgs {
    /// Path relative to the sandbox root.
    pub path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FsReadResult {
    pub content: String,
    pub bytes: usize,
}

pub async fn fs_read(sandbox: Arc<Sandbox>, args: FsReadArgs) -> anyhow::Result<FsReadResult> {
    let path = sandbox.resolve(&args.path).context("resolving path")?;
    let bytes = tokio::fs::read(&path).await.context("reading file")?;
    let len = bytes.len();
    let content = String::from_utf8(bytes).context("file is not valid UTF-8")?;
    Ok(FsReadResult { bytes: len, content })
}
