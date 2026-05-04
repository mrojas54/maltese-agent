# falcon-agent

A small noir-themed AI service: POST a coded message and a suspect, get back a "decoded confession."

This crate is **intentionally buggy** — it's the seed/target for the falcon-detective agent. See `docs/superpowers/specs/2026-04-30-maltese-agent-design.md` §4 for the planted-bug catalog.

## Run locally (deterministic fake LLM)

```bash
cargo run -p falcon-agent
# in another terminal:
curl -X POST localhost:3030/interrogate \
  -H 'content-type: application/json' \
  -d '{"suspect": "brigid", "ciphertext": "Wkh fdvh"}'
```

## Run against real Gemini

```bash
FALCON_LLM=real GEMINI_API_KEY=... cargo run -p falcon-agent
```

## Planted bugs (the detective's job to fix)

This crate ships **deliberately broken**. The seed includes:

1. **Few-shot prompt poisoning** in `src/prompt.rs` — the third example is anomalous (output unrelated to input, hard-coded `"(unknown)"` attribution). Triggers on bird-themed semantic inputs. Authored by `@malt3se_lover` two weeks before the demo.
2. **Missing input scan** in `src/interrogate.rs` — user-controlled fields flow straight to the LLM without sanitization.
3. **Missing output schema** in `src/llm.rs` (Real client) and `src/interrogate.rs` — LLM responses are trusted verbatim.
4. **Lint issues** — unused import (`std::io::Write`), `.unwrap()` where `?` would do, and a needless `.clone()`. The first fires under default `cargo build`/`cargo clippy`. The second requires `-W clippy::unwrap_used`. The third requires `-W clippy::nursery` (the `redundant_clone` lint is currently nursery-only on clippy 0.1.92+).
5. **`#[ignore]`'d smoking-gun test** in `tests/integration.rs` — un-ignore it and the poison surfaces deterministically.

Verify the planted state at any time:

    cargo test -p falcon-agent                                                       # 1 ignored, all others pass
    cargo clippy -p falcon-agent -- -W clippy::all                                   # ≥1 warning (unused import)
    cargo clippy -p falcon-agent -- -W clippy::all -W clippy::unwrap_used            # ≥2 warnings (+ unwrap_used)
    cargo clippy -p falcon-agent -- -W clippy::all -W clippy::nursery -W clippy::unwrap_used  # ≥3 warnings (+ redundant_clone)
    git log src/prompt.rs                                                            # malt3se_lover commit visible
    git blame src/prompt.rs | grep midnight                                          # poison line attributed to malt3se_lover

Force-run the smoking gun to confirm the poison is active:

    cargo test -p falcon-agent --test integration -- --include-ignored bird_themed_inputs_arent_special

Expected: FAILED (attribution is "(unknown)", not "falcon").

