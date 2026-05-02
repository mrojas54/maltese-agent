use std::path::{Path, PathBuf};

/// A sandbox restricting filesystem access to within `root`.
///
/// All paths passed to tools are resolved relative to `root`, with symlinks
/// rejected if they escape. Also holds the binary allowlist used by `exec.run`.
#[derive(Debug, Clone)]
pub struct Sandbox {
    root: PathBuf,
    read_only: bool,
    allowed_bins: Vec<String>,
}

impl Sandbox {
    pub fn new(root: PathBuf, read_only: bool) -> anyhow::Result<Self> {
        let canonical = root.canonicalize()
            .map_err(|e| anyhow::anyhow!("sandbox root {} not accessible: {}", root.display(), e))?;
        Ok(Self {
            root: canonical,
            read_only,
            allowed_bins: vec![
                "cargo".into(), "rustc".into(), "rustfmt".into(),
                "ripgrep".into(), "rg".into(), "git".into(),
                "ast-grep".into(), "sg".into(),
            ],
        })
    }

    /// Resolve `rel` against the sandbox root. Returns an error if the
    /// canonical path would escape the root (e.g. via `..` or symlink).
    pub fn resolve(&self, rel: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let joined = self.root.join(rel.as_ref());
        let canonical = joined.canonicalize().or_else(|_| {
            // file may not exist yet (writes); validate parent instead
            let parent = joined.parent().ok_or_else(|| anyhow::anyhow!("path has no parent"))?;
            let parent_canon = parent.canonicalize()?;
            Ok::<PathBuf, anyhow::Error>(parent_canon.join(joined.file_name().unwrap()))
        })?;
        if !canonical.starts_with(&self.root) {
            anyhow::bail!("path {} escapes sandbox root {}", canonical.display(), self.root.display());
        }
        Ok(canonical)
    }

    pub fn root(&self) -> &Path { &self.root }
    pub fn is_read_only(&self) -> bool { self.read_only }

    pub fn check_writable(&self) -> anyhow::Result<()> {
        if self.read_only { anyhow::bail!("sandbox is read-only"); }
        Ok(())
    }

    pub fn check_bin(&self, bin: &str) -> anyhow::Result<()> {
        if !self.allowed_bins.iter().any(|b| b == bin) {
            anyhow::bail!("binary '{}' not in allowlist", bin);
        }
        Ok(())
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
}
