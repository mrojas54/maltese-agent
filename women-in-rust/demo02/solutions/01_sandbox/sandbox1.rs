// sandbox1
//
// The first boundary. A `Sandbox` is a root directory, and its job is to turn a
// path from the model into a path on disk *or refuse it*. Every file tool in
// demo02 goes through it, so this one function is the guard.
//
// demo01's fs_read handed the model's path straight to the OS. Here you write
// the check that stops the two obvious escapes:
//
//   ../secret.txt   `..` climbs out of the root
//   /etc/passwd     an absolute path *replaces* the root: in Rust,
//                   `root.join("/etc/passwd")` is just `/etc/passwd`
//
// You do it lexically, by looking at the path's components and refusing the
// dangerous ones. `Path::components()` splits a path into pieces, and its
// `Component` enum has a variant for each kind. No filesystem calls.
//
// Be clear about what this is NOT. It judges a path by how it is spelled. A
// symlink inside the root that points outside it is spelled like any other
// name, and this check waves it through. The last test in this file passes on
// purpose to show you that. Making `resolve` look at the real filesystem is the
// jail, and the jail is demo03. One thing at a time.
//
//     cargo run -- run sandbox1
//
// Poke at it by hand, too:
//
//     cargo run --bin sandbox1 -- ../etc/passwd /etc/passwd notes.txt
//
// Stuck? cargo run -- hint sandbox1

use anyhow::Context as _;
use std::path::{Component, Path, PathBuf};

/// A root directory that paths must stay inside.
#[derive(Debug)]
struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(root: PathBuf) -> anyhow::Result<Self> {
        // Canonicalize the root once, here: `resolve` builds every path from
        // it, so it should be a real, absolute path with no symlinks in it.
        let root = root
            .canonicalize()
            .with_context(|| format!("sandbox root {} not accessible", root.display()))?;
        Ok(Self { root })
    }

    fn root(&self) -> &Path {
        &self.root
    }

    /// Turn a path from the model into a path on disk, or refuse it.
    ///
    /// This is a *lexical* check: it looks at the components of the path as
    /// written and refuses the ones that can leave the root. It never asks the
    /// filesystem anything, which is what makes it cheap and also what makes
    /// it not enough.
    fn resolve(&self, rel: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let rel = rel.as_ref();
        for component in rel.components() {
            match component {
                // `..` climbs out. A leading `/` (or `C:\` on Windows) is worse:
                // `Path::join` with an absolute path *replaces* the base, so
                // `root.join("/etc/passwd")` is just `/etc/passwd`.
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    anyhow::bail!(
                        "path {} escapes sandbox root {}",
                        rel.display(),
                        self.root.display()
                    );
                }
                Component::CurDir | Component::Normal(_) => {}
            }
        }
        Ok(self.root.join(rel))
    }
}

fn main() -> anyhow::Result<()> {
    // A place to poke at `resolve` by hand:
    //     cargo run --bin sandbox1 -- ../etc/passwd /etc/passwd notes.txt
    let sandbox = Sandbox::new(std::env::current_dir()?)?;
    println!("root     {}", sandbox.root().display());
    for arg in std::env::args().skip(1) {
        match sandbox.resolve(&arg) {
            Ok(path) => println!("ok       {arg}  ->  {}", path.display()),
            Err(err) => println!("refused  {arg}  ({err})"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The crate's own directory is a root we know exists.
    fn sandbox() -> Sandbox {
        Sandbox::new(PathBuf::from(env!("CARGO_MANIFEST_DIR"))).expect("the crate directory exists")
    }

    fn assert_refused(path: &str) {
        match sandbox().resolve(path) {
            Err(err) => assert!(
                err.to_string().contains("escapes"),
                "{path:?} was refused, but the message should say it escapes: {err}"
            ),
            Ok(resolved) => panic!("{path:?} should be refused, got {}", resolved.display()),
        }
    }

    #[test]
    fn a_path_inside_the_root_resolves() {
        let sb = sandbox();
        let resolved = sb.resolve("Cargo.toml").expect("inside the root");
        assert!(resolved.starts_with(sb.root()));
        assert!(resolved.ends_with("Cargo.toml"));
    }

    #[test]
    fn a_dot_and_nested_names_resolve() {
        let resolved = sandbox()
            .resolve("./notes/today.txt")
            .expect("`.` and nested names stay inside");
        assert!(resolved.ends_with("notes/today.txt"));
    }

    #[test]
    fn dotdot_is_refused() {
        assert_refused("../secret.txt");
    }

    #[test]
    fn dotdot_in_the_middle_is_refused() {
        assert_refused("notes/../../secret.txt");
    }

    #[test]
    fn an_absolute_path_is_refused() {
        assert_refused("/etc/passwd");
    }

    /// `..` is only special as a whole component. A file *named* `notes..txt` is
    /// an ordinary file, and refusing it means you searched the string instead
    /// of walking the components.
    #[test]
    fn dots_inside_a_name_are_not_dotdot() {
        let sb = sandbox();
        sb.resolve("notes..txt").expect("an ordinary file name");
        sb.resolve("a..b/c.txt")
            .expect("an ordinary directory name");
    }

    /// KNOWN GAP, and it passes. A symlink is spelled like any other name, so
    /// this lexical check cannot tell that `out/` leads outside the root, and
    /// reading through the path it hands back leaks the file next door. demo03
    /// keeps this exact setup and flips the assertion: it must be refused.
    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_root_still_gets_through() {
        let inside = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("tempdir");
        std::fs::write(outside.path().join("secret.txt"), "loot\n").expect("write secret");
        std::os::unix::fs::symlink(outside.path(), inside.path().join("out")).expect("symlink");

        let sb = Sandbox::new(inside.path().to_path_buf()).expect("root exists");
        let path = sb
            .resolve("out/secret.txt")
            .expect("no `..` and no leading `/`, so the spelling is clean");
        assert_eq!(
            std::fs::read_to_string(path).expect("read through the symlink"),
            "loot\n"
        );
    }
}
