// Gemini model IDs, defined once (AC-22). Every call site imports from this
// module; scripts/debt-gates.sh enforces that no `gemini-` string literal
// exists anywhere else under falcon-detective/src.
//
// 3.1 family (MA-27): Google retired the gemini-2.5 family for new API users
// in 2026-07 — generateContent returns HTTP 404 "no longer available to new
// users" even though ListModels still lists 2.5 (the listing lies; don't
// trust it). The operator chose the 3.1 family knowingly: no non-preview
// 3.1 pro exists, so GEMINI_PRO carries a "-preview" suffix and with it the
// risk that Google retires or renames it ahead of a stable release. If calls
// start 404ing again, revisit these IDs first.

/** Deep-reasoning model for analysis and code-change proposals. */
export const GEMINI_PRO = "gemini-3.1-pro-preview";

/** Fast model for mechanical fixes (e.g. lint autofixes). */
export const GEMINI_FLASH = "gemini-3.1-flash-lite";

/** Default model when a call site does not specify one. */
export const DEFAULT_MODEL = GEMINI_PRO;
