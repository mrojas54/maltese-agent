# MA-14: Harden exec tool path resolution

BUILDPLAN M3 / WS-9 (BUILDPLAN id MA-14). exec hardening: resolve allowlisted binaries to absolute paths at startup (no bare-name PATH resolution at call time); drop dead allowlist entries ripgrep and sg (verified: no test enumerates them); update doc strings at server.rs:294 and exec.rs:6; impostor-PATH test proving substitution fails. AC: AC-21. Depends: MA-09. Serialize: falcon-mcp/src/tools/*. Size M.
