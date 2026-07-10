# MA-13: Orchestration test coverage for detective core

BUILDPLAN M3 / WS-8 (BUILDPLAN id MA-13). Orchestration tests with mocked MCP client plus mocked/cassette Gemini: verify (Clean/Broken/Stuck), commit handlers (commitOne, commitAll, revertOne, escalate - success and git-failure paths, no silent catch), finalSweep (Clean and Dirty), processIssues (happy path; retry-then-success; revert-on-persistent-failure); smoke tests for cli.ts and workflows/tidy-and-debug.ts. Tests target CURRENT behavior - the MA-15 refactor must keep them green. ACs: AC-19, AC-20. Depends: MA-10, MA-11. Size L.
