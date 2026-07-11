# MA-32: Migrate Biome 1.9 to 2.x config and CLI in falcon-detective

FUTURE MIGRATION — out of contract 9f72697 scope (no AC IDs). Origin: Dependabot PR #54 (biome 1.9.4->2.5.3) bulk-merged 2026-07-10, broke main, reverted in PR #63. Observed breakage: npm run lint broken — 2.x CLI rejects the unmigrated 1.x biome.json (verified by MA-29 delegator). Scope: bump @biomejs/biome to current 2.x, run biome migrate to convert biome.json, reconcile any renamed/removed rules against the MA-5 lint baseline, review the reformat diff (2.x formatter defaults shifted) as a separate style-only commit if large. Gates: npm run lint green locally AND in the CI lint step (MA-5 wiring); fast suite green (npm test); no functional diffs mixed into the reformat commit. Serialization: package.json (falcon-detective) group — collides with the TypeScript 7 migration ticket; one in flight at a time. Size: S.

## Execution plan (agent:ma31-32-session, 2026-07-11)

1. Branch `claude/ma-32-biome-2x` off origin/main@73e7503 (worktree ma30-32-lattice-board-f3c29f).
2. Bump `@biomejs/biome` ^1.9.4 -> 2.5.3 (npm latest = the Dependabot #54 target line); `npm install`.
3. `npx biome migrate --write` to convert biome.json to 2.x schema; reconcile renamed/removed rules against the MA-5 baseline (recommended:true + offs: noNonNullAssertion, noParameterAssign, noExplicitAny) so the effective rule set is unchanged.
4. `npm run lint` — if the 2.x formatter defaults produce a reformat, land it as a separate style-only commit (no functional diffs mixed in).
5. Gates: `npm run lint` green; fast suite `npm test` green. CI lint step runs the same `npm run lint` (MA-5 wiring, ci.yml:127-129).
6. PR (base main) for operator merge; board in_progress -> review with evidence comment. MA-31 serialized behind this (package.json group), stacked on this branch.
