use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A sandbox restricting filesystem access to within `root`.
///
/// All paths passed to tools are resolved relative to `root`, with symlinks
/// rejected if they escape. Also holds the binary allowlist used by `exec.run`:
/// each allowlisted name is resolved to an absolute path against the PATH seen
/// at construction (server startup), so later PATH changes cannot substitute a
/// different binary for an allowlisted name (AC-21).
#[derive(Debug, Clone)]
pub struct Sandbox {
    root: PathBuf,
    read_only: bool,
    allowed_bins: Vec<String>,
    /// Allowlisted name → absolute path, resolved once at construction.
    /// Names missing from the startup PATH are absent here (construction must
    /// not fail on hosts without e.g. ast-grep); `resolved_bin` reports them
    /// at call time instead.
    resolved_bins: HashMap<String, PathBuf>,
}

/// `which`-equivalent: scan the current PATH for an executable regular file
/// named `name`. Used once per allowlisted name at sandbox construction.
fn resolve_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable_file(candidate))
}

fn is_executable_file(path: &Path) -> bool {
    let Some(meta) = path.metadata().ok().filter(|m| m.is_file()) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        true
    }
}

impl Sandbox {
    pub fn new(root: PathBuf, read_only: bool) -> anyhow::Result<Self> {
        let canonical = root.canonicalize().map_err(|e| {
            anyhow::anyhow!("sandbox root {} not accessible: {}", root.display(), e)
        })?;
        let allowed_bins: Vec<String> = ["cargo", "rustc", "rustfmt", "rg", "git", "ast-grep"]
            .into_iter()
            .map(String::from)
            .collect();
        let resolved_bins = allowed_bins
            .iter()
            .filter_map(|name| resolve_on_path(name).map(|path| (name.clone(), path)))
            .collect();
        Ok(Self {
            root: canonical,
            read_only,
            allowed_bins,
            resolved_bins,
        })
    }

    /// Resolve `rel` against the sandbox root. Returns an error if the
    /// canonical path would escape the root (e.g. via `..` or symlink).
    ///
    /// For paths that don't exist yet (e.g. files about to be written, possibly
    /// nested inside dirs that don't exist either), this walks up to the first
    /// existing ancestor, canonicalizes it, then re-attaches the missing
    /// suffix. The final path's escape-check still applies.
    pub fn resolve(&self, rel: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let joined = self.root.join(rel.as_ref());
        if let Ok(canonical) = joined.canonicalize() {
            if !canonical.starts_with(&self.root) {
                anyhow::bail!(
                    "path {} escapes sandbox root {}",
                    canonical.display(),
                    self.root.display()
                );
            }
            return Ok(canonical);
        }

        // Path doesn't exist yet. Walk up to the first existing ancestor,
        // canonicalize it, then re-attach the non-existent suffix. The full
        // result must still start with root.
        let mut existing = joined.as_path();
        let mut suffix: Vec<&std::ffi::OsStr> = Vec::new();
        let canonical_existing = loop {
            match existing.canonicalize() {
                Ok(c) => break c,
                Err(_) => {
                    let name = existing.file_name().ok_or_else(|| {
                        anyhow::anyhow!("path {} has no existing ancestor", joined.display())
                    })?;
                    suffix.push(name);
                    existing = existing.parent().ok_or_else(|| {
                        anyhow::anyhow!("path {} has no existing ancestor", joined.display())
                    })?;
                }
            }
        };
        let mut result = canonical_existing;
        for name in suffix.iter().rev() {
            result.push(name);
        }
        if !result.starts_with(&self.root) {
            anyhow::bail!(
                "path {} escapes sandbox root {}",
                result.display(),
                self.root.display()
            );
        }
        Ok(result)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn check_writable(&self) -> anyhow::Result<()> {
        if self.read_only {
            anyhow::bail!("sandbox is read-only");
        }
        Ok(())
    }

    pub fn check_bin(&self, bin: &str) -> anyhow::Result<()> {
        if !self.allowed_bins.iter().any(|b| b == bin) {
            anyhow::bail!("binary '{}' not in allowlist", bin);
        }
        Ok(())
    }

    /// Absolute path for an allowlisted binary, resolved once at construction.
    ///
    /// `exec_run` spawns only these stored absolute paths — never a bare name
    /// via call-time PATH lookup — so PATH manipulation after startup cannot
    /// substitute a binary for an allowlisted name (AC-21). Errors if `bin` is
    /// not on the allowlist, or if it is allowlisted but was not found on the
    /// startup PATH.
    pub fn resolved_bin(&self, bin: &str) -> anyhow::Result<&Path> {
        self.check_bin(bin)?;
        self.resolved_bins
            .get(bin)
            .map(PathBuf::as_path)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "allowlisted binary '{}' was not found on PATH at server startup",
                    bin
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolve_inside_root_succeeds() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf(), false).unwrap();
        let resolved = sb.resolve("a.txt").unwrap();
        assert!(resolved.starts_with(sb.root()));
        assert!(resolved.ends_with("a.txt"));
    }

    #[test]
    fn resolve_dotdot_escape_rejected() {
        let dir = TempDir::new().unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf(), false).unwrap();
        let err = sb.resolve("../escape.txt").unwrap_err();
        assert!(err.to_string().contains("escape"), "got: {err}");
    }

    #[test]
    fn read_only_blocks_writes() {
        let dir = TempDir::new().unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf(), true).unwrap();
        assert!(sb.check_writable().is_err());
    }

    #[test]
    fn allowlist_accepts_cargo_rejects_curl() {
        let dir = TempDir::new().unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf(), false).unwrap();
        assert!(sb.check_bin("cargo").is_ok());
        assert!(sb.check_bin("curl").is_err());
    }

    /// `resolved_bin` shares the allowlist gate with `check_bin`: a name off
    /// the allowlist is rejected before any lookup. (The resolution behavior
    /// itself — absolute path, startup-PATH pinning, unresolved-name error —
    /// is covered end-to-end in `tests/exec_impostor_test.rs`, which owns the
    /// required PATH mutation in its own process.)
    #[test]
    fn resolved_bin_rejects_non_allowlisted() {
        let dir = TempDir::new().unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf(), false).unwrap();
        let err = sb.resolved_bin("curl").unwrap_err();
        assert!(err.to_string().contains("not in allowlist"), "got: {err}");
    }

    /// Dead allowlist entries removed (WS-9): the binaries are addressed as
    /// `rg` and `ast-grep`; the `ripgrep`/`sg` aliases never matched a real
    /// invocation and must no longer pass the gate.
    #[test]
    fn allowlist_no_longer_contains_ripgrep_and_sg_aliases() {
        let dir = TempDir::new().unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf(), false).unwrap();
        assert!(sb.check_bin("rg").is_ok());
        assert!(sb.check_bin("ast-grep").is_ok());
        assert!(sb.check_bin("ripgrep").is_err());
        assert!(sb.check_bin("sg").is_err());
    }

    #[test]
    #[cfg(unix)]
    fn resolve_symlink_escape_rejected() {
        use std::os::unix::fs::symlink;
        let inside = tempfile::TempDir::new().unwrap();
        let outside = tempfile::TempDir::new().unwrap();
        // Create a symlink inside the jail pointing at the outside dir
        symlink(outside.path(), inside.path().join("escape_link")).unwrap();
        // Put a target file outside so canonicalize succeeds
        std::fs::write(outside.path().join("secret.txt"), "loot").unwrap();

        let sb = Sandbox::new(inside.path().to_path_buf(), false).unwrap();
        let err = sb.resolve("escape_link/secret.txt").unwrap_err();
        assert!(
            err.to_string().contains("escape"),
            "expected escape error, got: {err}"
        );
    }

    #[test]
    fn resolve_nonexistent_path_uses_parent_fallback() {
        let dir = tempfile::TempDir::new().unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf(), false).unwrap();
        // file doesn't exist yet — write tools rely on this case working
        let resolved = sb.resolve("new_file.txt").unwrap();
        assert!(resolved.starts_with(sb.root()));
        assert!(resolved.ends_with("new_file.txt"));
    }

    #[test]
    fn resolve_nonexistent_path_with_escaping_parent_rejected() {
        let dir = tempfile::TempDir::new().unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf(), false).unwrap();
        // parent itself escapes — the parent canonicalize step must catch this
        let err = sb.resolve("../wat/new_file.txt").unwrap_err();
        assert!(
            err.to_string().contains("escape") || err.to_string().contains("No such file"),
            "expected escape or parent-not-found error, got: {err}"
        );
    }

    #[test]
    fn resolve_deep_nonexistent_path_uses_first_existing_ancestor() {
        let dir = tempfile::TempDir::new().unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf(), false).unwrap();
        // Neither sub/ nor sub/inner/ exist — resolve should still produce a valid path
        // under the canonical root so write tools can create_dir_all.
        let resolved = sb.resolve("sub/inner/new_file.txt").unwrap();
        assert!(resolved.starts_with(sb.root()));
        assert!(resolved.ends_with("sub/inner/new_file.txt"));
    }
}
