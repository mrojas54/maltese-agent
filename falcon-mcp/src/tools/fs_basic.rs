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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FsWriteArgs {
    /// Path relative to the sandbox root.
    pub path: String,
    /// UTF-8 content to write.
    pub content: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FsWriteResult {
    pub bytes: usize,
}

pub async fn fs_write(sandbox: Arc<Sandbox>, args: FsWriteArgs) -> anyhow::Result<FsWriteResult> {
    sandbox.check_writable()?;
    let path = sandbox.resolve(&args.path).context("resolving path")?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.context("creating parent dirs")?;
    }
    let bytes = args.content.as_bytes();
    tokio::fs::write(&path, bytes).await.context("writing file")?;
    Ok(FsWriteResult { bytes: bytes.len() })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FsListArgs {
    /// Path relative to the sandbox root.
    pub path: String,
    /// Optional glob filter applied to entry names (not full paths).
    #[serde(default)]
    pub glob: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FsListResult {
    pub entries: Vec<String>,
}

pub async fn fs_list(sandbox: Arc<Sandbox>, args: FsListArgs) -> anyhow::Result<FsListResult> {
    let path = sandbox.resolve(&args.path).context("resolving path")?;
    let pattern = args
        .glob
        .as_deref()
        .map(glob::Pattern::new)
        .transpose()
        .context("invalid glob pattern")?;

    let mut rd = tokio::fs::read_dir(&path).await.context("reading directory")?;
    let mut entries = Vec::new();
    while let Some(entry) = rd.next_entry().await.context("iterating dir entries")? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(ref p) = pattern {
            if !p.matches(&name) {
                continue;
            }
        }
        entries.push(name);
    }
    entries.sort();
    Ok(FsListResult { entries })
}
