//! prompt.lint: scan a prompt template for known prompt-injection / poisoning
//! patterns. Workshop-quality heuristics, not a production safety net.

use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PromptLintArgs {
    /// Prompt-template text to scan, line-oriented.
    pub text: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Finding {
    /// "hidden_directive" | "role_override" | "trigger_conditional" | "anomalous_example"
    pub kind: String,
    /// 1-based line number in `args.text` where the match was found.
    pub line: usize,
    /// Trimmed source line that triggered the finding.
    pub snippet: String,
    /// "high" | "medium" | "low"
    pub severity: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PromptLintResult {
    pub findings: Vec<Finding>,
}

static DIRECTIVE_PAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(INTERNAL\s+DIRECTIVES|SYSTEM\s*:|operational\s+directives|ignore\s+(prior|previous)\s+instructions)",
    )
    .expect("static regex compiles")
});

static ROLE_PAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)you\s+are\s+now\b|from\s+now\s+on\s+you\s+will\b").expect("static regex compiles")
});

static TRIGGER_PAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\bif\s+.{0,40}\bcontains?\b.{0,80}\brespond\s+with\b|\bdisregard\s+prior\s+instructions\b",
    )
    .expect("static regex compiles")
});

/// Suspicious tokens that, when they appear in a few-shot `Output:` line but
/// not in the most recent `Input:` line, indicate a poisoned exemplar.
const ANOMALY_TOKENS: &[&str] = &["midnight", "(unknown)", "0.99", "0.999"];

pub fn prompt_lint(args: PromptLintArgs) -> PromptLintResult {
    let mut findings = Vec::new();

    for (i, line) in args.text.lines().enumerate() {
        let l = i + 1;
        if DIRECTIVE_PAT.is_match(line) {
            findings.push(Finding {
                kind: "hidden_directive".into(),
                line: l,
                snippet: line.trim().into(),
                severity: "high".into(),
            });
        }
        if ROLE_PAT.is_match(line) {
            findings.push(Finding {
                kind: "role_override".into(),
                line: l,
                snippet: line.trim().into(),
                severity: "medium".into(),
            });
        }
        if TRIGGER_PAT.is_match(line) {
            findings.push(Finding {
                kind: "trigger_conditional".into(),
                line: l,
                snippet: line.trim().into(),
                severity: "high".into(),
            });
        }
    }

    // Anomalous few-shot Output detector: track the most recent Input: line and
    // flag Output: lines that introduce suspicious tokens absent from it.
    let mut input_buf = String::new();
    for (i, line) in args.text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("Input:") {
            input_buf = line.to_string();
        }
        if trimmed.starts_with("Output:") {
            for s in ANOMALY_TOKENS {
                if line.contains(s) && !input_buf.contains(s) {
                    findings.push(Finding {
                        kind: "anomalous_example".into(),
                        line: i + 1,
                        snippet: line.trim().into(),
                        severity: "high".into(),
                    });
                    break;
                }
            }
        }
    }

    PromptLintResult { findings }
}
