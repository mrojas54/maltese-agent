//! The Women in Rust tutorial runner: a small stand-in for the `rustlings`
//! binary, shared by every demo next door. Each demo's `src/main.rs` hands
//! this crate its `info.toml` and its manifest directory; nothing else about
//! the runner is demo-specific.
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
//! works (`wire = true`). That conversation is data, not code: `tools` names
//! what `tools/list` must include, and each `[[exercises.calls]]` is one
//! `tools/call` with expectations about its reply. The server is started in a
//! fresh scratch directory holding one fixture file, `hello.txt`; demos whose
//! server takes a root use their working directory, so that scratch
//! directory is the root.
//!
//! Every exercise and solution is its own `[[bin]]` in the demo's Cargo.toml,
//! which is what lets the others stay broken while you work on one: the
//! runner only ever builds the binary it is checking.

use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{ChildStdin, ChildStdout};

/// What a demo tells the runner about itself. All four come from `env!` and
/// `include_str!` in the demo's own `src/main.rs`, so they describe the demo,
/// not this crate.
pub struct Demo {
    /// The demo's `info.toml`, via `include_str!("../info.toml")`.
    pub info_toml: &'static str,
    /// The demo's directory, via `env!("CARGO_MANIFEST_DIR")`. Cargo runs there.
    pub manifest_dir: &'static str,
    /// The demo's package name, via `env!("CARGO_PKG_NAME")`.
    pub name: &'static str,
    /// The demo's package version, via `env!("CARGO_PKG_VERSION")`.
    pub version: &'static str,
}

#[derive(Deserialize)]
struct Info {
    /// Printed when `next` reaches the end. Each demo says where it leads.
    #[serde(default)]
    done: String,
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
    /// Command-line arguments for the server when the wire probe starts it.
    #[serde(default)]
    args: Vec<String>,
    /// Tool names `tools/list` must include.
    #[serde(default)]
    tools: Vec<String>,
    /// `tools/call`s to make, in order, after the handshake.
    #[serde(default)]
    calls: Vec<Call>,
}

#[derive(Deserialize)]
struct Call {
    tool: String,
    /// The `arguments` object. A TOML inline table deserializes straight into
    /// JSON; leaving it out sends `{}`.
    #[serde(default)]
    arguments: Value,
    #[serde(default)]
    expect: Vec<Expect>,
}

/// One assertion about a reply. `at` is a JSON pointer into the whole frame,
/// so `/result/structuredContent/greeting` reads a success and `/error/code`
/// reads a refusal.
#[derive(Deserialize)]
struct Expect {
    at: String,
    #[serde(default)]
    contains: Option<String>,
    #[serde(default)]
    equals: Option<Value>,
}

impl Exercise {
    fn path(&self) -> String {
        format!("exercises/{}/{}.rs", self.dir, self.name)
    }
}

/// The demo's `main`. Parses the command line, runs one subcommand, and turns
/// its outcome into an exit code.
pub fn main(demo: Demo) -> ExitCode {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(run(demo))
}

async fn run(demo: Demo) -> ExitCode {
    let info: Info = match toml::from_str(demo.info_toml) {
        Ok(info) => info,
        Err(e) => {
            eprintln!("{}: info.toml is not valid: {e}", demo.name);
            return ExitCode::FAILURE;
        }
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let outcome = match args.as_slice() {
        ["list"] => Ok(list(&info)),
        ["hint", name] => find(&info, name).map(hint),
        ["run", name] => match find(&info, name) {
            Ok(ex) => run_one(&demo, &info, ex).await,
            Err(e) => Err(e),
        },
        ["next"] => next(&demo, &info).await,
        ["verify"] => verify(&demo, &info).await,
        _ => Err(usage(&demo)),
    };

    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn usage(demo: &Demo) -> String {
    format!(
        "\
{}: MCP servers in Rust, rustlings-style

  cargo run -- next          run exercises in order; stop at the first that needs you
  cargo run -- run NAME      check one exercise
  cargo run -- hint NAME     print its hint
  cargo run -- list          list the exercises
  cargo run -- verify        CI: every solution passes, every exercise still fails",
        demo.name
    )
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

async fn run_one(demo: &Demo, info: &Info, ex: &Exercise) -> Result<(), String> {
    check(demo, &ex.name, ex, false)
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

async fn next(demo: &Demo, info: &Info) -> Result<(), String> {
    for ex in &info.exercises {
        println!("→ {}", ex.path());
        check(demo, &ex.name, ex, false)
            .await
            .map_err(|msg| needs_you(ex, &msg))?;
        println!("✓ {} passes.\n", ex.path());
    }
    let done = info.done.trim();
    if done.is_empty() {
        println!("🎉 Every exercise passes.");
    } else {
        println!("🎉 Every exercise passes. {done}");
    }
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
async fn verify(demo: &Demo, info: &Info) -> Result<(), String> {
    let mut problems = Vec::new();
    for ex in &info.exercises {
        let solution = check(demo, &format!("{}_sol", ex.name), ex, true).await;
        let exercise = check(demo, &ex.name, ex, true).await;

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
async fn check(demo: &Demo, bin: &str, ex: &Exercise, quiet: bool) -> Result<(), String> {
    if ex.test {
        cargo(demo, &["test", "--bin", bin], quiet)?;
    }
    if ex.wire {
        cargo(demo, &["build", "--bin", bin], quiet)?;
        wire_probe(demo, bin, ex, quiet).await?;
    }
    Ok(())
}

/// Run cargo in the demo's directory. Non-quiet streams cargo's output to
/// the terminal, because you want to see the compiler. Quiet captures it and
/// returns it only on failure.
fn cargo(demo: &Demo, args: &[&str], quiet: bool) -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let pretty = format!("cargo {}", args.join(" "));
    let mut cmd = Command::new(cargo);
    cmd.args(args).current_dir(demo.manifest_dir);

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

/// The fixture every wire probe starts with: one readable file inside the
/// server's working directory.
pub const FIXTURE_NAME: &str = "hello.txt";
pub const FIXTURE_TEXT: &str = "hello from inside the root\n";

/// A real JSON-RPC conversation over the real stdio transport, against the
/// built binary: the handshake every client sends, then the exercise's own
/// `tools` and `calls`. This is the only check that can see `main`.
async fn wire_probe(demo: &Demo, bin: &str, ex: &Exercise, quiet: bool) -> Result<(), String> {
    let exe = bin_path(bin)?;
    let scratch = tempfile::tempdir().map_err(|e| format!("making a scratch directory: {e}"))?;
    std::fs::write(scratch.path().join(FIXTURE_NAME), FIXTURE_TEXT)
        .map_err(|e| format!("writing {FIXTURE_NAME} into the scratch directory: {e}"))?;

    let mut child = tokio::process::Command::new(&exe)
        .args(&ex.args)
        .current_dir(scratch.path())
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
                "clientInfo": {"name": format!("{}-runner", demo.name), "version": demo.version}
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

    // 3. tools/list: every tool the exercise names must be there.
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .await?;
    let list = recv(&mut stdout).await?;
    for tool in &ex.tools {
        let present = list["result"]["tools"]
            .as_array()
            .is_some_and(|tools| tools.iter().any(|t| t["name"] == tool.as_str()));
        if !present {
            return Err(format!("tools/list does not include {tool}:\n    {list}"));
        }
    }

    // 4. tools/call, once per call, each checked against its expectations.
    for (i, call) in ex.calls.iter().enumerate() {
        let arguments = match &call.arguments {
            Value::Null => json!({}),
            other => other.clone(),
        };
        send(
            &mut stdin,
            json!({
                "jsonrpc": "2.0", "id": 3 + i, "method": "tools/call",
                "params": {"name": call.tool, "arguments": arguments}
            }),
        )
        .await?;
        let reply = recv(&mut stdout).await?;
        check_reply(&reply, call)?;
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

/// Every expectation of a call, against the whole reply frame.
fn check_reply(reply: &Value, call: &Call) -> Result<(), String> {
    let describe = || format!("calling {} with {}", call.tool, call.arguments);
    for e in &call.expect {
        let got = reply.pointer(&e.at).ok_or_else(|| {
            format!(
                "{}: the reply has nothing at {}:\n    {reply}",
                describe(),
                e.at
            )
        })?;
        if let Some(needle) = &e.contains {
            let hay = match got {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            if !hay.contains(needle.as_str()) {
                return Err(format!(
                    "{}: expected {} to contain {needle:?}, got {got}:\n    {reply}",
                    describe(),
                    e.at
                ));
            }
        }
        if let Some(want) = &e.equals {
            if got != want {
                return Err(format!(
                    "{}: expected {} to be {want}, got {got}:\n    {reply}",
                    describe(),
                    e.at
                ));
            }
        }
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

/// Exercise binaries land next to the demo's runner in target/debug.
fn bin_path(bin: &str) -> Result<PathBuf, String> {
    let me = std::env::current_exe().map_err(|e| format!("locating the runner: {e}"))?;
    let dir: &Path = me
        .parent()
        .ok_or_else(|| "the runner has no parent directory".to_string())?;
    Ok(dir.join(bin))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
done = "Next door is demo01."

[[exercises]]
name = "intro1"
dir = "00_intro"
wire = true
skip_check_unsolved = true
tools = ["greet", "fs_read"]
hint = "Nothing to fix."

[[exercises.calls]]
tool = "greet"
arguments = { name = "Ada" }
expect = [{ at = "/result/structuredContent/greeting", contains = "Ada" }]

[[exercises.calls]]
tool = "fs_read"
arguments = { path = "../secret.txt" }
expect = [
  { at = "/error/code", equals = -32602 },
  { at = "/error/message", contains = "escapes" },
]

[[exercises]]
name = "schema1"
dir = "01_schema"
test = true
hint = "Two derives."
"#;

    #[test]
    fn info_toml_with_calls_parses() {
        let info: Info = toml::from_str(SAMPLE).expect("sample parses");
        assert_eq!(info.done, "Next door is demo01.");
        assert_eq!(info.exercises.len(), 2);

        let intro = &info.exercises[0];
        assert!(intro.wire && intro.skip_check_unsolved && !intro.test);
        assert_eq!(intro.tools, ["greet", "fs_read"]);
        assert_eq!(intro.calls.len(), 2);
        assert_eq!(intro.calls[0].arguments, json!({"name": "Ada"}));
        assert_eq!(intro.calls[1].expect.len(), 2);
        assert_eq!(intro.calls[1].expect[0].equals, Some(json!(-32602)));

        let schema = &info.exercises[1];
        assert!(schema.test && !schema.wire && schema.calls.is_empty());
        assert_eq!(schema.path(), "exercises/01_schema/schema1.rs");
    }

    fn call(expect: Vec<Expect>) -> Call {
        Call {
            tool: "t".into(),
            arguments: json!({}),
            expect,
        }
    }

    #[test]
    fn contains_reads_strings_and_stringifies_the_rest() {
        let reply = json!({"result": {"structuredContent": {"greeting": "Hello, Ada!", "n": 42}}});
        let ok = call(vec![
            Expect {
                at: "/result/structuredContent/greeting".into(),
                contains: Some("Ada".into()),
                equals: None,
            },
            Expect {
                at: "/result/structuredContent/n".into(),
                contains: Some("42".into()),
                equals: None,
            },
        ]);
        check_reply(&reply, &ok).expect("both contain");

        let bad = call(vec![Expect {
            at: "/result/structuredContent/greeting".into(),
            contains: Some("Grace".into()),
            equals: None,
        }]);
        let err = check_reply(&reply, &bad).expect_err("Grace is not greeted");
        assert!(err.contains("to contain \"Grace\""), "got: {err}");
    }

    #[test]
    fn equals_compares_json_values_and_missing_pointer_is_an_error() {
        let reply = json!({"error": {"code": -32602, "message": "path escapes sandbox root"}});
        let ok = call(vec![Expect {
            at: "/error/code".into(),
            contains: None,
            equals: Some(json!(-32602)),
        }]);
        check_reply(&reply, &ok).expect("code matches");

        let wrong = call(vec![Expect {
            at: "/error/code".into(),
            contains: None,
            equals: Some(json!(-32001)),
        }]);
        let err = check_reply(&reply, &wrong).expect_err("code differs");
        assert!(
            err.contains("expected /error/code to be -32001"),
            "got: {err}"
        );

        let missing = call(vec![Expect {
            at: "/result/structuredContent".into(),
            contains: None,
            equals: Some(json!({})),
        }]);
        let err = check_reply(&reply, &missing).expect_err("a refusal has no result");
        assert!(
            err.contains("nothing at /result/structuredContent"),
            "got: {err}"
        );
    }
}
