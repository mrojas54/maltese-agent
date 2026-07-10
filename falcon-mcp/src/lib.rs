//! falcon-mcp: sandboxed MCP toolkit for the falcon-detective coding agent.
//!
//! Tools are grouped into modules by category. The `FalconMcp` type wires them
//! into an rmcp `ServerHandler`. The crate exports both a binary (see
//! `src/main.rs`) and a library (so tests can drive the server in-process).

pub mod limits;
pub mod sandbox;
pub mod server;
pub mod tool_error;
pub mod tools;

pub use sandbox::Sandbox;
pub use server::FalconMcp;
