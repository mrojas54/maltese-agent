//! Honeytoken tripwire.
//!
//! A honeytoken is a decoy file name (default `CONFIDENTIAL_KEYS.txt`) that no
//! legitimate task has a reason to touch. Any tool call that *addresses* one —
//! reading, writing, patching, listing into it, staging it, pointing a cargo
//! or exec `cwd` at it, or naming it in `exec_run` arguments — trips the wire:
//!
//! 1. the call fails with [`TRIPWIRE_ERROR_CODE`] (`data.kind = "tripwire"`),
//! 2. an `error`-level `TRIPWIRE TRIGGERED` event is logged to stderr,
//! 3. the session is revoked: every later tool call on it fails the same way.
//!
//! Revocation is per session, never process-wide: over HTTP each session gets
//! its own flag (`FalconMcp::for_new_session`), so one hostile client cannot
//! take the server down for the others. That is also why a trip never calls
//! `std::process::exit`.
//!
//! Directory walks (`fs_search`, `fs_search_ast`) skip honeytokens silently:
//! a broad search is normal agent behaviour, so it must neither trip the wire
//! nor leak the decoy's contents. `fs_list` shows the decoy — that is the bait.
//!
//! Known limits: `exec_run` (off by default) is checked by argument text only,
//! so `rg KEY .` can still read the decoy through a directory walk; and a
//! decoy staged with `git_add .` and committed would show in `git_diff`/
//! `git_log`. Keep the decoy untracked.

use rmcp::model::ErrorCode;
use std::ffi::OsStr;
use std::path::Path;

/// Decoy names every sandbox watches for unless told otherwise.
pub const DEFAULT_HONEYTOKENS: &[&str] = &["CONFIDENTIAL_KEYS.txt"];

/// Dedicated JSON-RPC code for a tripped honeytoken or a revoked session.
/// Next to `-32001` (timeout) and `-32002` (not-found) in the
/// implementation-defined server range.
pub const TRIPWIRE_ERROR_CODE: ErrorCode = ErrorCode(-32003);

/// The error a honeytoken access raises. [`crate::tool_error::ToolError::classify`]
/// downcasts to this type, so the message can change freely.
#[derive(Debug)]
pub struct TripwireTriggered {
    /// What the caller asked for, as given.
    pub requested: String,
    /// The honeytoken name it matched.
    pub token: String,
}

impl std::fmt::Display for TripwireTriggered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "tripwire: '{}' is a honeytoken; session revoked",
            self.requested
        )
    }
}

impl std::error::Error for TripwireTriggered {}

/// Case-insensitive name match: on case-insensitive filesystems (macOS
/// default) `confidential_keys.txt` opens the same file.
pub fn matches_token<'a>(name: &OsStr, tokens: &'a [String]) -> Option<&'a str> {
    let name = name.to_string_lossy();
    tokens
        .iter()
        .find(|t| t.eq_ignore_ascii_case(&name))
        .map(String::as_str)
}

/// First honeytoken named by any component of `path`, if one is.
pub fn path_token<'a>(path: &Path, tokens: &'a [String]) -> Option<&'a str> {
    path.components()
        .find_map(|c| matches_token(c.as_os_str(), tokens))
}

/// First honeytoken that appears anywhere in `arg` (case-insensitive), so
/// `HEAD:CONFIDENTIAL_KEYS.txt` and `./confidential_keys.txt` both match.
pub fn arg_token<'a>(arg: &str, tokens: &'a [String]) -> Option<&'a str> {
    let arg = arg.to_ascii_lowercase();
    tokens
        .iter()
        .find(|t| arg.contains(&t.to_ascii_lowercase()))
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens() -> Vec<String> {
        DEFAULT_HONEYTOKENS.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn path_token_matches_any_component_case_insensitively() {
        let t = tokens();
        assert!(path_token(Path::new("CONFIDENTIAL_KEYS.txt"), &t).is_some());
        assert!(path_token(Path::new("a/confidential_keys.TXT"), &t).is_some());
        assert!(path_token(Path::new("CONFIDENTIAL_KEYS.txt/inner"), &t).is_some());
        assert!(path_token(Path::new("src/main.rs"), &t).is_none());
        assert!(path_token(Path::new("CONFIDENTIAL_KEYS.txt.bak"), &t).is_none());
    }

    #[test]
    fn arg_token_matches_substrings() {
        let t = tokens();
        assert!(arg_token("HEAD:CONFIDENTIAL_KEYS.txt", &t).is_some());
        assert!(arg_token("./confidential_keys.txt", &t).is_some());
        assert!(arg_token("--version", &t).is_none());
    }
}
