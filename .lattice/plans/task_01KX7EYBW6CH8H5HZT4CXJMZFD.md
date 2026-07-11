# MA-31: Migrate TypeScript 5.9 to 7.x in falcon-detective

FUTURE MIGRATION — out of contract 9f72697 scope (no AC IDs). Origin: Dependabot PR #50 (typescript 5.9->7.0.2) bulk-merged 2026-07-10, reverted untested in PR #63 (tsx 4.23.0 was kept). TS 7 is the native-port compiler generation — treat as a deliberate migration: verify tsc semantics/flags in tsconfig.json still hold, npm run build output identical in behavior (dist/cli.js drives e2e replay — stale/changed dist changes cassette keys, see MA-29 lesson), vitest 3.2.7 + tsx interop, Biome interop. Gates: tsc clean; fast suite green (npm test, 152 tests, <=60s); full e2e cassette replay green (npm run test:full) with ZERO cassette misses — key stability is the regression signal; npm run lint green. Serialization: package.json (falcon-detective) group — collides with the Biome 2.x migration ticket; one in flight at a time. Size: M.

## Execution plan (agent:ma31-32-session, 2026-07-11)

1. Branch `claude/ma-31-typescript-7` stacked on claude/ma-32-biome-2x@1ae89d1 (press-ahead at review, package.json serialization group — PR base main, body names the #68 anchor, merges after it).
2. Bump `typescript` ^5.9.3 -> 7.0.2 (npm latest; the Dependabot #50 target). TS 7 = native-port compiler generation — treat as deliberate migration.
3. Verify tsconfig.json / tsconfig.build.json flags still accepted; fix any TS7 diagnostics surfaced in src/tests.
4. Rebuild dist (`npm run build`) — dist/cli.js drives e2e replay; cassette-key stability is the regression signal (MA-29 lesson).
5. Gates: tsc clean; fast suite `npm test` (152 tests) green; full e2e cassette replay `npm run test:full` green with ZERO cassette misses; `npm run lint` green (Biome 2.5.3 interop, post-MA-32).
6. PR (base main) for operator merge; board in_progress -> review with evidence comment.
