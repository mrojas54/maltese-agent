// Gemini model IDs, defined once (AC-22). Every call site imports from this
// module; scripts/debt-gates.sh enforces that no `gemini-` string literal
// exists anywhere else under falcon-detective/src.

/** Deep-reasoning model for analysis and code-change proposals. */
export const GEMINI_PRO = "gemini-2.5-pro";

/** Fast model for mechanical fixes (e.g. lint autofixes). */
export const GEMINI_FLASH = "gemini-2.5-flash";

/** Default model when a call site does not specify one. */
export const DEFAULT_MODEL = GEMINI_PRO;
