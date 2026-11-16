//! The harness you edit live. It starts as Baby's First Harness (step 0).

use std::path::PathBuf;
use std::process::{Command, Output};

pub struct Harness {
    pub root: PathBuf,
}

impl Harness {
    pub fn new(root: PathBuf) -> Self {
        Harness { root }
    }

    /// Tool 1: run the command the model wrote.
    pub fn run(&mut self, cmd: &str) -> Result<String, String> {
        let out = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(&self.root)
            .output()
            .map_err(|e| e.to_string())?;
        finish(out)
    }

    /// Tool 2: read the file the model named.
    pub fn read(&mut self, path: &str) -> Result<String, String> {
        std::fs::read_to_string(self.root.join(path)).map_err(|e| e.to_string())
    }
}

fn finish(out: Output) -> Result<String, String> {
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}
