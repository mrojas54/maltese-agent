//! falcon-mcp: sandboxed MCP toolkit for the falcon-detective coding agent.
//!
//! Tools are grouped into modules by category. The `Server` type wires them
//! into an rmcp `ServerHandler`. The crate exports both a binary (see
//! `src/main.rs`) and a library (so tests can drive the server in-process).
//!
//! `server` and `tools` modules are added in Task 3.

pub mod sandbox;

pub use sandbox::Sandbox;
