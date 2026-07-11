# MA-22: Final sweep: harnesses green and debt-gates closeout

BUILDPLAN M4 final sweep (BUILDPLAN id MA-22). e2e green on the NEW cassettes (after the human-track re-record); every harness H-1..H-9 green; debt-gates closeout. ACs: all (terminal). Depends: human-track re-record, MA-21. Size S.

## Plan (fast-track, single session)

Working against origin/main @ f176529 (PR #64 merged: re-recorded Gemini 3.1 cassettes + fingerprint; e2e passed in CI with zero misses). Worktree: `.claude/worktrees/ma-22-final-sweep`, branch `claude/ma-22-final-sweep`.

### Deliverable 1 — flip the e2e hard gate
- `.github/workflows/ci.yml`: set `E2E_REQUIRED: "1"` (line 163, currently `"0"`).
- Rewrite the obsolete PARKED comment block (lines 153–158) with a short note that the gate is live post-MA-24 (green run mechanically implies e2e replay success). Keep the surrounding AC-5 cost-note comments intact.

### Deliverable 2 — run all harnesses H-1..H-9 and capture evidence
- H-1 `cargo fmt --all -- --check` (repo root)
- H-2 `cargo clippy -p falcon-mcp --all-targets -- -D warnings` (repo root)
- H-3 `cargo test --workspace --all-targets` (repo root)
- H-4 `npm run build` (falcon-detective/)
- H-5 `npm run lint` (falcon-detective/)
- H-6 `npm test` (falcon-detective/, fast suite, expect ≤60s)
- H-7 `cargo audit` (root) + `npm audit --audit-level=high` (falcon-detective/)
- H-8 `scripts/check-planted-defects.sh`
- H-9 `scripts/debt-gates.sh`
- Plus the flipped-gate proof: `E2E_REQUIRED=1 npm run test:full` in falcon-detective/ (~2 min; MA-29 handles dist build + `.runs/e2e-test` pre-clean).
- Capture tail output of each as evidence for the validation attachment and PR body.

### Deliverable 3 — debt-gates closeout audit
- SPEC R-5 enumerates required gates: WS-10 (AC-23), WS-2 (AC-7, AC-8), WS-3 (AC-25), WS-4 (AC-22), WS-5 (AC-13). EVALUATION additionally cites H-9 for AC-12 (WS-4a/b) and AC-34 (WS-14a).
- Pre-audit of `scripts/debt-gates.sh` @ f176529 finds gates for ALL eight H-9-citing ACs: AC-23 (toolchain-dockerfile-match), AC-25 (single-MAX_STDERR_DISPLAY-definition + shared-search-subprocess-helper), AC-22 (no-gemini-literals), AC-13 (barnum-private-surface), AC-7 (smoking-gun-ignore-reason), AC-8 (prompt-poison-flag), AC-12 (no-handler-any), AC-34 (no-workflow-any).
- Expected outcome: no missing gates; H-9 run green is the closeout artifact. If a gate fails or a gap surfaces, deviate-and-flag per clause.

### Guardrails
- G-1: falcon-agent plants untouched (H-8 enforces).
- No cassette/fingerprint/model-ID touches (PR #64 just landed them).
- No dependency upgrades (MA-30/31/32 own those).
- No force-push; diagnose harness failures, never loosen gates.

### Phases
1. Plan (this file) → `planned`.
2. Implement: `git fetch origin`, record base sha, rebase if moved; edit ci.yml; run harnesses; commit `chore(ci): flip E2E_REQUIRED hard gate live and close out debt gates (MA-22)`.
3. Self-review: diff vs origin/main; attach review verdict (actor agent:ma-22-delegator-reviewer).
4. Validate: attach H-1..H-9 + test:full evidence note.
5. PR: push, verify remote sha, open PR vs main, attach PR URL, bump `review`, completion comment. STOP (no merge).
