# MA-17: ToolError taxonomy with distinct MCP error codes

BUILDPLAN M4 / WS-14c (BUILDPLAN id MA-17). ToolError taxonomy in falcon-mcp server.rs: distinguishable error kinds (minimum: not-found, invalid-argument, internal) with distinct MCP error codes; migrate all 18 map_err-format sites; test asserting an MCP client can branch on the codes. AC: AC-32. Depends: MA-09. Serialize: server.rs. Size L.
