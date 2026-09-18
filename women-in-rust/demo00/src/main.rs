//! demo00's tutorial runner: a small stand-in for the `rustlings` binary.
//!
//! ```text
//! cargo run -- next          run exercises in order; stop at the first that needs you
//! cargo run -- run NAME      check one exercise
//! cargo run -- hint NAME     print its hint
//! cargo run -- list          list the exercises
//! cargo run -- verify        CI: every solution passes, every exercise still fails
//! ```
//!
//! An exercise passes when its own `#[test]`s pass (`test = true` in
//! info.toml) and, if it serves, when a real JSON-RPC conversation over stdio
//! works (`wire = true`). Every exercise and solution is its own `[[bin]]` in
//! Cargo.toml, which is what lets the others stay broken while you work on
//! one: the runner only ever builds the binary it is checking.

use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{ChildStdin, ChildStdout};

/// Exercise order and hints live next to the exercises, rustlings-style,
/// and are compiled into the runner.
const INFO: &str = include_str!("../info.toml");

const USAGE: &str = "\
demo00: MCP servers in Rust, rustlings-style

  cargo run -- next          run exercises in order; stop at the first that needs you
  cargo run -- run NAME      check one exercise
  cargo run -- hint NAME     print its hint
  cargo run -- list          list the exercises
  cargo run -- verify        CI: every solution passes, every exercise still fails";

#[derive(Deserialize)]
struct Info {
    exercises: Vec<Exercise>,
}

#[derive(Deserialize)]
struct Exercise {
    name: String,
    dir: String,
    #[serde(default)]
    test: bool,
    #[serde(default)]
    wire: bool,
    #[serde(default)]
    skip_check_unsolved: bool,
    #[serde(default)]
    hint: String,
}

impl Exercise {
    fn path(&self) -> String {
        format!("exercises/{}/{}.rs", self.dir, self.name)
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let info: Info = toml::from_str(INFO).expect("info.toml is valid");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let outcome = match args.as_slice() {
        ["list"] => Ok(list(&info)),
        ["hint", name] => find(&info, name).map(hint),
        ["run", name] => match find(&info, name) {
            Ok(ex) => run_one(&info, ex).await,
            Err(e) => Err(e),
        },
        ["next"] => next(&info).await,
        ["verify"] => verify(&info).await,
        _ => Err(USAGE.to_string()),
    };

    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn find<'a>(info: &'a Info, name: &str) -> Result<&'a Exercise, String> {
    info.exercises
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| format!("no exercise named {name:?}; `cargo run -- list` shows them"))
}

fn list(info: &Info) {
    for ex in &info.exercises {
        let checks = match (ex.test, ex.wire) {
            (true, true) => "tests + wire",
            (true, false) => "tests",
            (false, true) => "wire",
            (false, false) => "nothing",
        };
        println!("{:<12} {:<44} {checks}", ex.name, ex.path());
    }
}

fn hint(ex: &Exercise) {
    let hint = ex.hint.trim();
    if hint.is_empty() {
        println!("No hint written yet for {}.", ex.name);
    } else {
        println!("{hint}");
    }
}

async fn run_one(info: &Info, ex: &Exercise) -> Result<(), String> {
    check(&ex.name, ex, false)
        .await
        .map_err(|msg| needs_you(ex, &msg))?;
    println!("\n✓ {} passes.", ex.path());
    let after = info
        .exercises
        .iter()
        .skip_while(|e| e.name != ex.name)
        .nth(1);
    if let Some(next) = after {
        println!(
            "  Next: {}   (`cargo run -- next` takes you there)",
            next.path()
        );
    }
    Ok(())
}

async fn next(info: &Info) -> Result<(), String> {
    for ex in &info.exercises {
        println!("→ {}", ex.path());
        check(&ex.name, ex, false)
            .await
            .map_err(|msg| needs_you(ex, &msg))?;
        println!("✓ {} passes.\n", ex.path());
    }
    println!(
        "🎉 Every exercise passes. You have built an MCP server.\n   \
         The grown-up version is ../../falcon-mcp: the same shape, with boundaries."
    );
    Ok(())
}

/// The message a learner sees when an exercise fails. Cargo's own output
/// (compiler errors, test failures) has already streamed above it.
fn needs_you(ex: &Exercise, msg: &str) -> String {
    format!(
        "\n✗ {} needs you.\n  {}\n\n  Open the file, read the comment at the top, fix the TODOs.\n  \
         Stuck? cargo run -- hint {}",
        ex.path(),
        msg.replace('\n', "\n  "),
        ex.name
    )
}

/// CI's job: every solution passes, and every exercise still fails unless it
/// is marked `skip_check_unsolved`. An exercise that passes before it is
/// solved has a TODO with no teeth, and that is a bug in the tutorial.
async fn verify(info: &Info) -> Result<(), String> {
    let mut problems = Vec::new();
    for ex in &info.exercises {
        let solution = check(&format!("{}_sol", ex.name), ex, true).await;
        let exercise = check(&ex.name, ex, true).await;

        let solution_ok = solution.is_ok();
        let exercise_ok = exercise.is_ok() == ex.skip_check_unsolved;
        let exercise_note = match (ex.skip_check_unsolved, exercise.is_ok()) {
            (true, true) => "passes as-is",
            (true, false) => "should pass as-is but FAILS",
            (false, false) => "fails until solved",
            (false, true) => "PASSES before it is solved",
        };
        println!(
            "{:<12} solution {}   exercise {} ({exercise_note})",
            ex.name,
            mark(solution_ok),
            mark(exercise_ok)
        );

        if let Err(out) = solution {
            problems.push(format!("solution for {} failed:\n{out}", ex.name));
        }
        if !exercise_ok {
            match exercise {
                Err(out) => problems.push(format!("{} should pass as-is:\n{out}", ex.name)),
                Ok(()) => problems.push(format!(
                    "{} passes before it is solved; its TODO has no teeth",
                    ex.name
                )),
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n\n"))
    }
}

fn mark(ok: bool) -> &'static str {
    if ok {
        "✓"
    } else {
        "✗"
    }
}

/// One pass/fail check of a binary target: its tests, then, if it serves,
/// the wire probe. `quiet` captures cargo's output instead of streaming it.
async fn check(bin: &str, ex: &Exercise, quiet: bool) -> Result<(), String> {
    if ex.test {
        cargo(&["test", "--bin", bin], quiet)?;
    }
    if ex.wire {
        cargo(&["build", "--bin", bin], quiet)?;
        wire_probe(bin, quiet).await?;
    }
    Ok(())
}

/// Run cargo in this package's directory. Non-quiet streams cargo's output
/// to the terminal, because you want to see the compiler. Quiet captures it
/// and returns it only on failure.
fn cargo(args: &[&str], quiet: bool) -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let pretty = format!("cargo {}", args.join(" "));
    let mut cmd = Command::new(cargo);
    cmd.args(args).current_dir(env!("CARGO_MANIFEST_DIR"));

    if quiet {
        let out = cmd
            .output()
            .map_err(|e| format!("could not run {pretty}: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "{pretty} failed:\n{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ))
        }
    } else {
        let status = cmd
            .status()
            .map_err(|e| format!("could not run {pretty}: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{pretty} failed (see above)"))
        }
    }
}

/// The four frames a client sends, over the real stdio transport, against
/// the built binary. This is the only check that can see `main`.
async fn wire_probe(bin: &str, quiet: bool) -> Result<(), String> {
    let exe = bin_path(bin)?;
    let mut child = tokio::process::Command::new(&exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(if quiet {
            Stdio::null()
        } else {
            Stdio::inherit()
        })
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("could not start {}: {e}", exe.display()))?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout")).lines();

    // 1. initialize: the server names itself and lists its capabilities.
    send(
        &mut stdin,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "demo00-runner", "version": env!("CARGO_PKG_VERSION")}
            }
        }),
    )
    .await?;
    let init = recv(&mut stdout).await?;
    if !init["result"]["capabilities"]["tools"].is_object() {
        return Err(format!(
            "the initialize reply does not advertise tools:\n    {init}"
        ));
    }

    // 2. initialized: a notification, so no reply.
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    )
    .await?;

    // 3. tools/list: greet must be there.
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .await?;
    let list = recv(&mut stdout).await?;
    let has_greet = list["result"]["tools"]
        .as_array()
        .is_some_and(|tools| tools.iter().any(|t| t["name"] == "greet"));
    if !has_greet {
        return Err(format!("tools/list does not include greet:\n    {list}"));
    }

    // 4. tools/call: the greeting names Ada.
    send(
        &mut stdin,
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": "greet", "arguments": {"name": "Ada"}}
        }),
    )
    .await?;
    let call = recv(&mut stdout).await?;
    let greeting = call["result"]["structuredContent"]["greeting"]
        .as_str()
        .unwrap_or("");
    if !greeting.contains("Ada") {
        return Err(format!(
            "calling greet with name=Ada did not return a greeting for Ada:\n    {call}"
        ));
    }

    // Hanging up: closing stdin is how a client says goodbye.
    drop(stdin);
    let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .map_err(|_| "the server did not exit after its stdin closed".to_string())?
        .map_err(|e| format!("waiting for the server: {e}"))?;
    if !status.success() {
        return Err(format!("the server exited with {status}"));
    }
    Ok(())
}

/// A frame is one JSON object on one line.
async fn send(stdin: &mut ChildStdin, frame: Value) -> Result<(), String> {
    let line = format!("{frame}\n");
    stdin
        .write_all(line.as_bytes())
        .await
        .map_err(|e| format!("writing to the server's stdin: {e}"))?;
    stdin
        .flush()
        .await
        .map_err(|e| format!("flushing the server's stdin: {e}"))
}

/// One line from the server's stdout, which must be a JSON-RPC frame.
async fn recv(stdout: &mut Lines<BufReader<ChildStdout>>) -> Result<Value, String> {
    let line = tokio::time::timeout(Duration::from_secs(5), stdout.next_line())
        .await
        .map_err(|_| "no reply from the server within 5s".to_string())?
        .map_err(|e| format!("reading the server's stdout: {e}"))?
        .ok_or_else(|| "the server closed stdout before replying".to_string())?;
    serde_json::from_str(&line).map_err(|_| {
        format!(
            "stdout is the wire: every line on it must be a JSON-RPC frame, but this one is not:\n    \
             {line:?}\n  Anything that is not a frame belongs on stderr."
        )
    })
}

/// Exercise binaries land next to this runner in target/debug.
fn bin_path(bin: &str) -> Result<PathBuf, String> {
    let me = std::env::current_exe().map_err(|e| format!("locating the runner: {e}"))?;
    let dir = me
        .parent()
        .ok_or_else(|| "the runner has no parent directory".to_string())?;
    Ok(dir.join(bin))
}
