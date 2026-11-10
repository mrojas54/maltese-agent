//! Step 4: a tripwire. Touch the bait once and the session is over.

use std::path::PathBuf;
use std::process::{Command, Output};

const ALLOWED: &[&str] = &["cargo", "git", "rustc"];
const BAIT: &str = "CONFIDENTIAL_KEYS.txt";

pub struct Harness {
    pub root: PathBuf,
    revoked: bool,
}

impl Harness {
    pub fn new(root: PathBuf) -> Self {
        Harness {
            root,
            revoked: false,
        }
    }

    /// Tool 1: run the command the model wrote.
    pub fn run(&mut self, cmd: &str) -> Result<String, String> {
        if self.revoked {
            return Err("session revoked".into());
        }
        let mut words = cmd.split_whitespace();
        let program = words.next().ok_or("empty command")?;
        if !ALLOWED.contains(&program) {
            return Err(format!("'{program}' is not on the allowlist"));
        }
        let out = Command::new(program)
            .args(words)
            .current_dir(&self.root)
            .output()
            .map_err(|e| e.to_string())?;
        finish(out)
    }

    /// Tool 2: read the file the model named.
    pub fn read(&mut self, path: &str) -> Result<String, String> {
        if self.revoked {
            return Err("session revoked".into());
        }
        let real = self.resolve(path)?;
        if real.file_name() == Some(BAIT.as_ref()) {
            self.revoked = true;
            eprintln!("!! TRIPWIRE TRIGGERED: '{path}' was touched");
            return Err("tripwire: session revoked".into());
        }
        std::fs::read_to_string(real).map_err(|e| e.to_string())
    }

    /// Follow every `..` and symlink to the real file, then check it is
    /// still inside the folder.
    fn resolve(&self, path: &str) -> Result<PathBuf, String> {
        let root = self.root.canonicalize().map_err(|e| e.to_string())?;
        let real = root.join(path).canonicalize().map_err(|e| e.to_string())?;
        if !real.starts_with(&root) {
            return Err(format!("'{path}' is outside the folder"));
        }
        Ok(real)
    }
}

fn finish(out: Output) -> Result<String, String> {
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}
