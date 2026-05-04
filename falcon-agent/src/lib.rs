//! falcon-agent: a buggy AI service decoding "noir confessions."
//!
//! Intentionally seeded with: a poisoned few-shot prompt, missing input
//! sanitization, missing output schema validation, and a handful of lint
//! issues. The falcon-detective agent's job is to find and fix all of these.

pub mod decoder;
pub mod interrogate;
pub mod llm;
pub mod prompt;
