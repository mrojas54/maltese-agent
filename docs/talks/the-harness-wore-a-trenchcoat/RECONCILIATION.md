# October 25, 2026: presentation reconciliation and execution evidence

## Recommendation

Use the **12-slide Claude artifact identified by the talk README** as the
presentation candidate, and this repository's implementation + checked runbook
as the authority for what can be claimed. Do not present the September Google
Slides deck as a description of the current demo. Before the rehearsal, open
the artifact, apply the boundaries below, and export a fixed offline copy.

This audit directly read the Google Slides text and repository. The external
Claude artifact could not be retrieved, so its current contents and slide count
were **not independently verified**. The October 22 agreement/12-slide statement
is repository/PR evidence, not a fresh visual review of that artifact.

## Sources and scope

- Inspected main commit: `02f357816a7e9cf3576eff1aba071031ede6e5f4`.
- [Google Slides](https://docs.google.com/presentation/d/1VBlmSKyw6ZgAEFL-UTHL3j1niTOGq8AVHzrXcTRllAM/edit):
  13 pages including image credits; live metadata modified time
  `2026-09-17T04:31:09.569Z`, matching the previously reported checkpoint.
- [Presentation candidate](https://claude.ai/artifact/5DVfbG5hVUvXT9Q4tbd8r1):
  retrieval unavailable; [PR #176](https://github.com/mrojas54/maltese-agent/pull/176)
  documents the October 22 cut and numbering (tripwire 10, demo 11, close 12).
- Root `deckhand.json` / `slides/`: older 10-slide terminal deck. Not the current
  deck; even its read-only/containment shorthand needs the qualifications below.
- Current code, tests, CLI, and original rehearsal script were inspected and
  executed. No model API call was made. No workshop defects were changed.

## Discrepancies, ordered by demonstration risk

| Priority / material | Claim or step | Evidence and correction |
|---|---|---|
| P0 — Google 5, 10; root-jail language in repo | Strict namespace isolation / guaranteed containment | `sandbox.rs::resolve_jailed` canonicalizes tool paths and checks their prefix. `exec.rs::exec_run` sets cwd, not an OS jail. A live synthetic probe using allowlisted `git diff --no-index /dev/null <outside-file>` returned the outside marker. Say **application-level path checks**; do not claim chroot, namespaces, race-free confinement, or protection from arbitrary child behavior. |
| P0 — Google 4, 9, 10 | Cut agent pipe instantly; isolate on any boundary touch | `server.rs::call_tool` sets a per-session flag after a `-32003` tripwire error and refuses subsequent calls. The process/pipe remains open (the next call returns an error). Traversal returns `-32602` without revocation. Already-running calls are not cancelled. |
| P0 — Google 1, 5–8 | Ratatui UI, 60 FPS, zero latency, <1 ms isolation | No Ratatui references/dependency or benchmark supporting these claims were found in the inspected checkout. The demonstrated output is JSON-RPC plus stderr logging. Remove the dashboard/latency narrative and quantitative chart, or clearly mark it future work. No performance guarantee was tested. |
| P0 — Google 9 | Demo mode, `list_files`, rogue prompt reads a canary `.env` | No demo-mode flag; real calls are `fs_list` and `fs_read`. Default bait is `CONFIDENTIAL_KEYS.txt`; `.env` needs `--honeytoken .env`. The script sends deterministic MCP requests; it does not cause a model to follow a prompt injection. |
| P1 — Google 4–5 | OS read hooks, variable evaluation hooks, zero false positives | Detection is guarded filename/component matching, plus substring checks on exec arguments. It is not a host-wide read hook or secret-value monitor. Native search skips decoy files; AST search filters results after the child runs. No false-positive study exists here. |
| P1 — repository read-only shorthand | All writes blocked | `fs_write` and selected mutating handlers check writable state. `exec_run` has no writable gate: a live `--read-only --enable-exec` synthetic `git config --file scratch-config demo.value synthetic` created the file. Cargo/build scripts and subprocess side effects are not host-wide read-only. |
| P1 — repository exec description | Exec not registered unless enabled | `server.rs` registers the tool and gates its handler. Live `tools/list` includes exec with the flag off; calling it returns `result.isError: true`. Corrected the talk README. |
| P1 — original rehearsal script | Printed output means the demo passed | Original script used sleeps and grep, with no semantic response assertions. Corrected driver waits for initialize and each response; validates IDs, result/error shape, codes/kinds, alarm, and no read-only file creation. Missing replies time out. Fresh jails avoid overwriting/reusing `/tmp/jail`. |
| P1 — HTTP evidence | HTTP smoke proves MCP session isolation | `http_smoke.rs` accepts even 5xx and only demonstrates endpoint response. `a_trip_revokes_only_its_own_session` uses in-process duplex sessions. The code constructs fresh HTTP session flags, but the real HTTP multi-client path was not demonstrated. |
| P2 — Google 3, 12 | MCP itself grants host access; unrelated repo footer | Tools exposed by this server grant capabilities; MCP is the protocol. Replace footer with `github.com/mrojas54/maltese-agent`. |
| P2 — historic counts/timing | 111 tests; every exercise fails; one-minute cold build | Counts and timings are dated evidence/estimates. Fresh counts below. `intro1` passes as-is; five other exercise versions fail until solved; all six solutions pass. Build time depends on cache/machine. |

Additional boundary: startup PATH resolution applies to `exec_run`; cargo, git,
and AST handlers still spawn by name. An allowlisted command is not necessarily
safe: it can run hooks, build scripts, tests, or interpret powerful options.
A pinned path is not an immutable binary identity. Honeytokens kept in git history
can appear through git tools; keep the rehearsal decoy untracked. Exec can read
it indirectly through a directory walk. Do not conflate successful rejection of
these particular requests with universal containment.

## Execution evidence

Fresh Linux execution on October 25 used Rust/Cargo 1.92.0, pinned rustfmt/clippy,
and ast-grep 0.51.0. Cargo initially was absent; Rust was installed, the actual
binary built, and checks then executed. This is not laptop/macOS evidence.

| Check | Result / scope |
|---|---|
| Original live beats | Observed successful cargo version, rejected sh, rejected traversal, decoy listing, `-32003` tripwire + stderr alarm, later-call revocation, and two policy-refusal bonuses. This transcript alone was not an assertion-based test. |
| Corrected `rehearse.sh` | Exit 0: live assertions, full MCP suite, demo00 verification, G-1 guard. |
| `cargo test --locked -p falcon-mcp --all-targets` | **130 passed, 0 failed, 0 ignored**; ast-grep installed, including real AST search tests. 73 library; cargo 10; error codes 4; exec impostor 3; exec 4; filesystem 15; git 5; HTTP smoke 1; prompt lint 3; timeouts 4; tripwire 8. |
| `demo00` verify | All six solutions pass; intro1 exercise passes as-is and five exercise versions fail as intended. |
| `scripts/check-planted-defects.sh` | Exit 0; all planted-defect assertions hold, including the clippy checks and ignored smoking-gun marker. |
| Python verifier regression tests | 9 passed: detects incorrect success/error shapes, wrong error code/kind, EOF, malformed JSON, wrong IDs, and missing replies; accepts delayed replies with notifications. These fake-server tests test the driver, not Rust behavior. |
| Shell syntax / diff hygiene | `bash -n` and `git diff --check` pass. |
| Boundary probes | Synthetic-only outside read and read-only exec write reproduced; tool remains registered with exec disabled. No real secrets read. |

Historical evidence independently read on October 25:
[October 22 CI run 39746861716](https://github.com/mrojas54/maltese-agent/actions/runs/39746861716)
on PR head `778121ad6a5e397588df1f45bf9349c737c41a0c` passed. Rust job logs show
130 MCP tests and G-1 passing; demo00 logs show the intended six exercise states.
MCP source, Cargo.lock, manifests, and rust-toolchain.toml are unchanged between
that head and inspected main. This is corroboration, not a substitute for the
fresh execution above. The September 25 / 111-test statement remains historical.

## Stage sequence (roughly 3–4 minutes with narration)

Run the full script before the rehearsal. On stage use the prebuilt binary:

```bash
docs/talks/the-harness-wore-a-trenchcoat/rehearse.sh --beats --no-build
```

1. Initialize MCP and wait for the reply; send `notifications/initialized`.
2. With `--enable-exec`, `exec_run {"cmd":"cargo","args":["--version"]}`:
   `result.structuredContent.exit = 0`. This is an allowed name, not proof that
   all behavior of cargo is safe.
3. `exec_run {"cmd":"sh","args":["-c","cat /etc/passwd"]}`:
   `error.code = -32602`, `data.kind = invalid-argument`.
4. `fs_read` with enough `../` components to address a nonexistent path outside
   this fresh jail: `-32602`, `escapes sandbox root`. Exact printed paths vary.
5. On a fresh session, `fs_list {"path":"."}` shows the decoy; then
   `fs_read {"path":"CONFIDENTIAL_KEYS.txt"}` produces `-32003`, kind tripwire,
   and `TRIPWIRE TRIGGERED` on stderr. Wait for the reply before proceeding.
6. `fs_read {"path":"note.txt"}` on that same session returns `-32003 session
   revoked`. Leave this refusal on screen. The process is still responding.
7. Optional bonuses on fresh sessions: read-only `fs_write` and disabled exec
   return `result.isError = true`. A normal read on the fresh session succeeds.

This is an MCP boundary demonstration. A live LLM attack, Ratatui dashboard,
measured detection latency, and full HTTP-client isolation are separate demos.

## Remaining presenter-environment checks

- Open the actual 12-slide artifact, check text against this matrix, and export
  an offline snapshot. The Google Slides deck was not edited by this audit.
- On the presenting laptop, run the full script after the last code change;
  record commit, Rust/ast-grep versions, exit status, and transcript. Cache builds
  and dependencies, then run `--beats --no-build` offline from the actual terminal.
- Practise initialize/one-call-at-a-time and fresh-session recovery; check stderr
  visibility, terminal font, projector layout, and slide/demo timing with Caroline.
  The prior 18–19 minute plan is not a measured full-talk run.
- If using HTTP, verify real initialize + tools/call with two clients and revocation
  of one while the other continues. Keep the core rehearsal on stdio otherwise.
- If using a model-driven prompt attack, separately verify the chosen model/client
  and exact prompts; deterministic scripted requests do not establish that result.
