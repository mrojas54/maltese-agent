# MA-15: Decompose processIssues and de-cast workflow stages

BUILDPLAN M4 / WS-14a (BUILDPLAN id MA-15). Decompose the processIssues main loop into named stage functions (each about 40 lines or less) - MA-13 tests stay green; de-cast workflows/tidy-and-debug.ts stages (no detective-any or as-any stage casts; stages typed against handler input/output types). ACs: AC-30, AC-34. Depends: MA-13. Size M.
