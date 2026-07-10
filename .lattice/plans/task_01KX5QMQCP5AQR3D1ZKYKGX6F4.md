# MA-10: Schema-validated MCP client boundary

BUILDPLAN M3 / WS-4a/b (BUILDPLAN id MA-10). toolSchemas.ts zod schemas per MCP tool result; FalconMcpClient.call (mcp.ts:57 - the method is call, not callTool) becomes schema-required and validates before returning; migrate all call sites; proposePoisonFix gets real schemas reusing Issue/PoisonReport from types.ts (a TriageReport type does NOT exist); remove the TriageOutput-as-any cast at triage.ts:134. ACs: AC-12, AC-15. Depends: MA-07. Serialize: types.ts, lib/mcp.ts. Size M.
