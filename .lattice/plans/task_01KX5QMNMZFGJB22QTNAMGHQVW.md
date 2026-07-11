# MA-6: Add supply-chain audits and Dependabot

BUILDPLAN M2 / WS-7 (BUILDPLAN id MA-06). CI steps for cargo audit and npm audit --audit-level=high (accepted advisories allowlisted in-repo with a comment); .github/dependabot.yml covering cargo, npm, and github-actions ecosystems. ACs: AC-17, AC-18. Serialize: ci.yml. Size S.

## Plan

### 1. CI — rust job (`.github/workflows/ci.yml`)

Two new steps inserted after "Cache cargo registry and target" and before "cargo fmt --check" (audit only reads `Cargo.lock`, so it runs pre-compile and fails fast; the planted-defect guard stays literally last):

- **Install cargo-audit** via `taiki-e/install-action@v2` with `tool: cargo-audit`. Justification (per SPEC WS-7, which names this action explicitly): prebuilt binary installs in ~1s, exactly matching the existing ast-grep install in this job; `cargo install cargo-audit` would cost ~3min of compile per run and pollute the rust-cache.
- **cargo audit** step: `run: cargo audit`. A comment marks it TD-9/AC-17 and points at `.cargo/audit.toml` for the allowlist.

### 2. CI — typescript job

One new step after "npm ci" and before the biome lint step:

- **npm audit (high+)**: `npm audit --audit-level=high` in `working-directory: falcon-detective`. Fails on high/critical advisories only.

### 3. Allowlist mechanism

- **cargo**: new `.cargo/audit.toml` at repo root (cargo-audit's default config path). Ships with `[advisories] ignore = []` plus a header comment documenting the rule: each accepted RUSTSEC id gets an inline comment with reason and revisit date. Currently empty — no advisories are being accepted.
- **npm**: `npm audit` has no native ignore file. The CI step carries a comment stating the policy: fix-in-range first (`npm audit fix`); only if an advisory has no fix do we accept it, by wrapping the step with an in-repo exceptions list (documented in the comment as the escalation path, e.g. `better-npm-audit` + `.nsprc`). No exceptions exist today, so no wrapper dependency is added speculatively.

### 4. Making the npm gate land green (pre-req, lockfile-only)

Local `npm audit` shows 8 advisories (4 high: fast-uri, hono, protobufjs, ws — all transitive via `@google/genai`/`@modelcontextprotocol/sdk`/vitest), **all fixable in-range**. Allowlisting fixable highs would defeat the gate, so: run `npm audit fix` (updates `package-lock.json` only; `package.json` ranges untouched, verified via dry-run), then confirm `npm audit --audit-level=high` exits 0 and `npm test` stays green. This is the minimal enabling change for AC-17, not a dependency-upgrade scope grab (WS-13 territory is untouched).

### 5. `.github/dependabot.yml` (AC-18)

`version: 2`, weekly interval per SPEC WS-7, three update blocks:
- `package-ecosystem: cargo`, `directory: "/"`
- `package-ecosystem: npm`, `directory: "/falcon-detective"`
- `package-ecosystem: github-actions`, `directory: "/"`

### 6. Verification

- Local: `npm ci && npm audit --audit-level=high` (expect exit 0 after fix); `npm test` (fast suite green). `cargo-audit` is not installed locally and per ticket instructions is not installed here — CI proves the cargo path. actionlint not installed locally; ci.yml edit is additive steps only.
- G-1: no falcon-agent file is touched; cargo audit reads `Cargo.lock` only. If CI's cargo audit later surfaces a falcon-agent advisory, the allowlist route in `.cargo/audit.toml` is the only permitted response.

### Files touched

- `.github/workflows/ci.yml` (2 rust-job steps, 1 typescript-job step)
- `.github/dependabot.yml` (new)
- `.cargo/audit.toml` (new)
- `falcon-detective/package-lock.json` (audit fix refresh)

## Reset 2026-07-10 by agent:delegator-ma-6
