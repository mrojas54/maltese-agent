# MA-27: Migrate Gemini model IDs to 3.1 family

Fix-it ticket minted mid-MA-24: Google retired gemini-2.5-pro for new API users (generateContent returns 404 'no longer available to new users'; ListModels still lists it — the listing lies). Operator decision: 3.1 family. Change lib/models.ts: GEMINI_PRO = gemini-3.1-pro-preview, GEMINI_FLASH = gemini-3.1-flash-lite (DEFAULT_MODEL stays GEMINI_PRO). Update README model references (recording section call plan + cassette table). Handle cassette-keyed unit tests: keys = sha256(model+schema+prompt) so all existing cassette-backed tests will MISS under new IDs until the operator re-records at MA-24 — apply the established skip-marker-naming-MA-24 pattern where misses fail (e2e already skips via fingerprint). Old 2.5-keyed cassette files become stale leftovers; note for MA-24 that the record run will report them for deletion. Keep fast suite green, tsc/biome clean, debt-gates green. Blocks MA-24 (operator re-record), which blocks MA-22.

---

## Plan (delegator-ma-27)

The operator chose `-preview` knowingly (no non-preview 3.1 pro exists);
retirement risk noted in a code comment. Generation cannot be verified from
this harness (outbound POSTs blocked); the operator's MA-24 record run is the
live verification.

### Worktree / branch

- Worktree: `/Users/michellerojas/maltese-agent/.claude/worktrees/ma-27-model-ids`
- Branch: `claude/ma-27-model-ids` off `origin/main@abfeb39` (verified)

### rg inventory: every `gemini-2.5` hit and its disposition

`rg -n "gemini-2\.5"` across repo, excluding `falcon-agent/**` (G-1),
`node_modules/**`, `.git/**`. Cassette JSON content carries no model string
(model participates only in the hashed filename) and is out of scope anyway.

| # | File:line | Context | Disposition |
|---|---|---|---|
| 1 | `falcon-detective/src/lib/models.ts:6` | `GEMINI_PRO = "gemini-2.5-pro"` | **CHANGE** → `"gemini-3.1-pro-preview"` + retirement comment |
| 2 | `falcon-detective/src/lib/models.ts:9` | `GEMINI_FLASH = "gemini-2.5-flash"` | **CHANGE** → `"gemini-3.1-flash-lite"` |
| 3 | `falcon-detective/README.md:149-150` | Spend note: "3× gemini-2.5-pro … 2× gemini-2.5-flash" | **CHANGE** → new IDs |
| 4 | `falcon-detective/README.md:154-167` | "current set" cassette table (filenames keyed under 2.5) | **CHANGE**: add header note — set recorded under retired 2.5 IDs, stale until MA-24 re-record; filenames will change (model is a hash component); record run reports old files as deletable leftovers |
| 5 | `falcon-detective/tests/lib/gemini.test.ts:20` | `const model = "gemini-2.5-pro"` keys a self-written cassette; `g.call` omits model (uses DEFAULT_MODEL) | **CHANGE** → import `DEFAULT_MODEL` from `src/lib/models.js` |
| 6 | `falcon-detective/tests/handlers/proposeBugFix.test.ts:36` | literal in self-written cassette key | **CHANGE** → `GEMINI_PRO` import |
| 7 | `falcon-detective/tests/handlers/proposeLintFix.test.ts:15,28,55` | comment, key literal, test name | **CHANGE** → `GEMINI_FLASH` import; comment/test-name reference the constant, not a literal |
| 8 | `falcon-detective/tests/handlers/analyzePoison.test.ts:29` | literal in self-written cassette key | **CHANGE** → `GEMINI_PRO` import |
| 9 | `falcon-detective/tests/handlers/proposePoisonFix.test.ts:87,101,121` | comment, test name, key literal | **CHANGE** → `GEMINI_PRO` import; comment/test-name reference the constant |
| 10 | `SPEC.md:46` | WS spec text quoting 2.5 values for models.ts | **LEAVE** — point-in-time build contract; the ticket + PR record the operator's superseding decision |
| 11 | `docs/tech-debt-2026-07-10.md:28` | TD-12 audit finding quoting old hardcoded strings | **LEAVE** — historical audit record |
| 12 | `docs/superpowers/plans/2026-05-01-falcon-agent-implementation.md:4,411,428` | historical implementation plan | **LEAVE** — historical record (also falcon-agent-adjacent) |
| 13 | `docs/superpowers/plans/2026-05-01-falcon-detective-implementation.md:348,421,999,1063,1096,1191,1251,1284` | historical implementation plan | **LEAVE** — historical record |
| 14 | `docs/superpowers/specs/2026-04-30-maltese-agent-design.md:317,354` | original design doc | **LEAVE** — historical record |

Already-correct sites (no change): `scripts/record-cassettes.ts` and
`scripts/write-e2e-fingerprint.ts` import from `models.ts` (dry-run will print
the new IDs automatically); `src/lib/gemini.ts` imports `DEFAULT_MODEL`; all
handlers import `GEMINI_PRO`/`GEMINI_FLASH`; `scripts/debt-gates.sh` gate only
forbids `gemini-` literals in `src/` outside `models.ts` (unaffected).
`tests/e2e-guard.test.ts` uses fake model ids ("gemini-test-model") — no change.

### Cassette-miss analysis (why NO MA-24 skip-markers are needed)

- The five cassette-consuming fast-suite tests (`gemini.test.ts`,
  `proposeBugFix`, `proposeLintFix`, `proposePoisonFix`, `analyzePoison`) are
  **hermetic**: each writes its own cassette into a temp dir keyed by
  sha256(model+schema+prompt). They would fail under new IDs only because
  their key literals pin 2.5. Switching the literals to the `models.ts`
  constants keeps them green AND preserves the AC-24 parity pin (a hit still
  proves the handler sent exactly the constant's model + prompt bytes).
- The only consumer of the real `fixtures/cassettes/` is `tests/e2e.test.ts`,
  which is excluded from the fast suite (`npm test`) and already fast-skips
  via `checkCassetteFreshness` (MA-26): today there is **no
  `.e2e-fingerprint.json` manifest on disk**, so it skips with "no readable
  fingerprint manifest"; once MA-24 writes one, model drift is caught by the
  manifest's `model` component. CI runs `test:full` with `E2E_REQUIRED="0"`
  (parked until MA-24/MA-22), so the skip is green in CI.
- Therefore: no test hard-fails on a cassette miss after the literal→constant
  updates; no new skip-markers required. If gates prove otherwise, fall back
  to the MA-26 skip wording naming MA-24.

### Steps

1. Edit `models.ts` (new IDs + retirement/`-preview` comment).
2. Update the five test files (constants instead of literals; test names and
   comments updated).
3. Update README recording section (spend note + cassette-table note).
4. Gates (in worktree `falcon-detective/`): `npm ci`; `npm test` green;
   `npx tsc --noEmit`; `npx biome check .`; repo-root
   `scripts/debt-gates.sh`; `scripts/check-planted-defects.sh` exit 0;
   `npm run record -- --dry-run` prints the NEW IDs.
5. Commit: `fix(falcon-detective): migrate Gemini model IDs to 3.1 family (MA-27)`.
6. Own-review vs origin/main; fix Critical/Major; attach review comment.
7. Push, PR to main, comment PR URL on ticket, bump to review.

### Notes for MA-24

- Old 2.5-keyed cassette JSONs left on disk deliberately; the record run
  reports them as "untouched" stale leftovers for deletion.
- Record run must also `npm run e2e:fingerprint` (manifest currently absent).
