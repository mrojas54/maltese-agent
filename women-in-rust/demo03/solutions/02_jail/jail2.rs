// jail2
//
// demo02's `resolve` judged a path by its spelling. This one asks the
// filesystem. It canonicalizes the path, which collapses every `..` and follows
// every symlink, and only then checks that the answer still starts with the
// root. That is the jail, and it closes the hole demo02 left open: a symlink
// inside the root that points outside it now resolves to where it really leads,
// and is refused.
//
// It has a catch, and the catch is this exercise. `canonicalize` only works on
// paths that EXIST, and the file an `fs_write` is about to create does not.
// "It failed" cannot mean "allowed", or a write could go anywhere. It cannot
// mean "refused" either, or `fs_write` could never make a new file.
//
// So a path that does not exist yet has to be placed anyway: find the longest
// part of it that does exist, canonicalize THAT, put the missing tail back, and
// run the same escape check on the finished path.
//
// Poke at it by hand:
//
//     cargo run --bin jail2_sol -- hello.txt new.txt ../secret.txt

use anyhow::Context as _;
use std::path::{Path, PathBuf};

/// A root directory that paths must stay inside.
#[derive(Debug)]
struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(root: PathBuf) -> anyhow::Result<Self> {
        // The root is canonical too, so every comparison below is between two
        // paths with no symlinks and no `..` in them.
        let root = root
            .canonicalize()
            .with_context(|| format!("sandbox root {} not accessible", root.display()))?;
        Ok(Self { root })
    }

    fn root(&self) -> &Path {
        &self.root
    }

    /// Turn a path from the model into a canonical path inside the root, or
    /// refuse it.
    fn resolve(&self, rel: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        // `join` with an absolute path replaces the root, and that is fine now:
        // the check below is on where the path really ends up, not on how it
        // was spelled.
        let joined = self.root.join(rel.as_ref());

        // A path that exists: canonicalize it and compare.
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

        // TODO(human): the path does not exist yet, so `canonicalize` failed.
        // Place it anyway.
        //
        // Walk up from `joined` to the first ancestor that does exist,
        // canonicalize THAT, re-attach the names you walked past (in order),
        // and check the finished path is still under `self.root`, with the same
        // "escapes sandbox root" message as above.
        //
        // Things to weigh:
        //   - `Path::file_name()` and `Path::parent()` are the two steps of the
        //     walk, and either can be `None` when the path runs out. What
        //     should happen then?
        //   - Why does the escape check run on the re-assembled path, and not
        //     on `joined`? Try `out/new.txt` where `out` is a symlink that
        //     leads outside the root.
        //   - A `..` can hide in the missing tail: `ghost/../../new.txt`. Does
        //     your walk catch it, or fail closed?
        todo!("place a path that does not exist yet")
    }
}

fn main() -> anyhow::Result<()> {
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

    /// <tmp>/root/hello.txt       inside the sandbox
    /// <tmp>/root/sub/            an existing directory inside it
    /// <tmp>/outside/secret.txt   outside it: the file a jail must not leak
    fn setup() -> (tempfile::TempDir, Sandbox) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("root");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(root.join("sub")).expect("create root/sub");
        std::fs::create_dir(&outside).expect("create outside");
        std::fs::write(root.join("hello.txt"), "hello\n").expect("write hello");
        std::fs::write(outside.join("secret.txt"), "the secret\n").expect("write secret");
        let sandbox = Sandbox::new(root).expect("root exists");
        (tmp, sandbox)
    }

    fn assert_escapes(sandbox: &Sandbox, path: impl AsRef<Path>) {
        let path = path.as_ref();
        match sandbox.resolve(path) {
            Err(err) => assert!(
                err.to_string().contains("escapes"),
                "{path:?} was refused, but the message should say it escapes: {err}"
            ),
            Ok(resolved) => panic!("{path:?} should be refused, got {}", resolved.display()),
        }
    }

    // ---- paths that exist: the canonicalize half ---------------------------

    #[test]
    fn a_path_inside_the_root_resolves() {
        let (_tmp, sb) = setup();
        let resolved = sb.resolve("hello.txt").expect("inside the root");
        assert!(resolved.starts_with(sb.root()));
        assert!(resolved.ends_with("hello.txt"));
    }

    /// demo02 refused every `..`. Asking the filesystem, a `..` that stays
    /// inside the root is just a longer way to write the same path.
    #[test]
    fn dotdot_that_stays_inside_is_fine() {
        let (_tmp, sb) = setup();
        let resolved = sb.resolve("sub/../hello.txt").expect("stays inside");
        assert!(resolved.ends_with("hello.txt"));
        assert!(!resolved.to_string_lossy().contains(".."));
    }

    /// demo02 refused every absolute path. An absolute path that lands inside
    /// the root is harmless; it is where it lands that counts.
    #[test]
    fn an_absolute_path_inside_the_root_is_fine() {
        let (_tmp, sb) = setup();
        let absolute = sb.root().join("hello.txt");
        sb.resolve(&absolute).expect("lands inside the root");
    }

    #[test]
    fn dotdot_out_of_the_root_is_refused() {
        let (_tmp, sb) = setup();
        assert_escapes(&sb, "../outside/secret.txt");
    }

    #[test]
    fn an_absolute_path_outside_the_root_is_refused() {
        let (tmp, sb) = setup();
        assert_escapes(&sb, tmp.path().join("outside/secret.txt"));
    }

    /// demo02's known-gap test, flipped. The setup is the same: a symlink
    /// inside the root that leads outside it. The result is the opposite.
    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_root_is_refused() {
        let (tmp, sb) = setup();
        std::os::unix::fs::symlink(tmp.path().join("outside"), sb.root().join("out"))
            .expect("symlink");
        assert_escapes(&sb, "out/secret.txt");
    }

    // ---- paths that do not exist yet: the fallback half --------------------

    #[test]
    fn a_new_file_in_the_root_resolves() {
        let (_tmp, sb) = setup();
        let resolved = sb.resolve("new.txt").expect("a new file at the top");
        assert!(resolved.starts_with(sb.root()));
        assert!(resolved.ends_with("new.txt"));
    }

    /// Neither `deep/` nor `deep/er/` exists. The walk has to go up two levels
    /// to `root/` and put both names back.
    #[test]
    fn a_deep_new_path_resolves_from_the_first_existing_ancestor() {
        let (_tmp, sb) = setup();
        let resolved = sb.resolve("deep/er/new.txt").expect("nothing exists yet");
        assert!(resolved.starts_with(sb.root()));
        assert!(resolved.ends_with("deep/er/new.txt"));
    }

    #[test]
    fn a_new_file_with_an_escaping_parent_is_refused() {
        let (_tmp, sb) = setup();
        assert_escapes(&sb, "../wat/new.txt");
    }

    /// The reason the escape check runs on the re-assembled path: `out/` exists
    /// and leads outside the root, and only `new.txt` is missing.
    #[cfg(unix)]
    #[test]
    fn a_new_file_under_a_symlink_that_leads_out_is_refused() {
        let (tmp, sb) = setup();
        std::os::unix::fs::symlink(tmp.path().join("outside"), sb.root().join("out"))
            .expect("symlink");
        assert_escapes(&sb, "out/new.txt");
    }

    #[test]
    fn a_new_absolute_path_outside_the_root_is_refused() {
        let (tmp, sb) = setup();
        assert_escapes(&sb, tmp.path().join("nowhere/new.txt"));
    }

    /// `ghost/` does not exist, so the OS cannot say what `ghost/..` is. Either
    /// answer is fine (refused as an escape, or refused as unplaceable) as long
    /// as it is a refusal.
    #[test]
    fn a_dotdot_hidden_in_the_missing_tail_is_refused() {
        let (_tmp, sb) = setup();
        let result = sb.resolve("ghost/../../new.txt");
        assert!(result.is_err(), "resolved to {:?}", result.unwrap());
    }
}
