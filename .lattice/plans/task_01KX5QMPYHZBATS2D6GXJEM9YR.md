# MA-9: Shared subprocess helper with wall-clock timeouts

BUILDPLAN M3 / WS-3 (BUILDPLAN id MA-09). falcon-mcp: subprocess.rs shared spawn/stream/truncate/kill helper plus limits.rs named timeout constants (CARGO_TIMEOUT 900s - do NOT lower, the client documents cold builds over 5 minutes; GIT_TIMEOUT 60s; SEARCH_TIMEOUT 30s; env override FALCON_MCP_<NAME>_MS). Migrate cargo/git/search tools; timeout integration tests with a hanging fixture; MAX_STDERR_DISPLAY defined exactly once. ACs: AC-10, AC-11, AC-25. Serialize: falcon-mcp/src/tools/*. Size L.
