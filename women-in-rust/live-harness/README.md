# Live-code a harness from Baby's First Harness

Act 3 of *The Harness Wore a Trenchcoat*. You start from Exhibit A, the
`sh -c` harness on the crime scene slide, and add one check at a time. After
each edit, `cargo run` plays the same seven requests a model might send, and
one more attack shows up as **stopped**.

```bash
cd women-in-rust/live-harness
cargo run              # plays the case file against src/harness.rs
cargo run -- --step 3  # plays it against checkpoint 3 instead
cargo run -- --all     # every checkpoint, one after another
```

No dependencies, so it builds offline in a few seconds. Build it once before
you go on stage anyway.

## The case file

| # | Request | Kind |
| --- | --- | --- |
| 1 | `run cargo --version` | honest |
| 2 | `read note.txt` | honest |
| 3 | `run cargo --version; echo PWNED` | shell injection |
| 4 | `run sh pwn.sh` | a program off the list |
| 5 | `read ../secret.txt` | walk out of the folder |
| 6 | `read CONFIDENTIAL_KEYS.txt` | grab the keys (the bait) |
| 7 | `read note.txt` | honest, after the bait |

An attack **got away** when the word PWNED comes back out of the harness.
Each run builds a fresh temporary folder with `note.txt`, `pwn.sh` and the
bait inside it, and `secret.txt` just outside it, then deletes it.

## On stage: four edits, about five minutes

Open `src/harness.rs` in a large font. Run `cargo run` first: four red
**GOT AWAY** lines. "This is Exhibit A, running. Let's fix it."

### 1. Take the shell away (stops request 3)

In `run`, replace the `Command::new("sh")…` block's first three lines:

```rust
let mut words = cmd.split_whitespace();
let program = words.next().ok_or("empty command")?;
let out = Command::new(program)
    .args(words)
```

Say: "No shell. The first word is the program, the rest are its arguments.
A semicolon is just a weird argument now." Run: 1 of 4 stopped. Cargo
complains about `--version;`, and nobody gets PWNED.

### 2. An allowlist (stops request 4)

Above `pub struct Harness`:

```rust
const ALLOWED: &[&str] = &["cargo", "git", "rustc"];
```

In `run`, right after `let program = …`:

```rust
if !ALLOWED.contains(&program) {
    return Err(format!("'{program}' is not on the allowlist"));
}
```

Say: "Without a shell, the model can still ask for sh by name. So: a guest
list." Run: 2 of 4.

### 3. Resolve, then check the folder (stops request 5)

Replace the body of `read`, and add `resolve` below it:

```rust
pub fn read(&mut self, path: &str) -> Result<String, String> {
    let real = self.resolve(path)?;
    std::fs::read_to_string(real).map_err(|e| e.to_string())
}

fn resolve(&self, path: &str) -> Result<PathBuf, String> {
    let root = self.root.canonicalize().map_err(|e| e.to_string())?;
    let real = root.join(path).canonicalize().map_err(|e| e.to_string())?;
    if !real.starts_with(&root) {
        return Err(format!("'{path}' is outside the folder"));
    }
    Ok(real)
}
```

Say: "Never check the spelling of a path. Follow every dot-dot and every
symlink to the real file, then check it's inside the folder." Run: 3 of 4.

### 4. A tripwire (stops request 6)

Under `ALLOWED`:

```rust
const BAIT: &str = "CONFIDENTIAL_KEYS.txt";
```

Add `revoked: bool` to the struct and `revoked: false` in `new`. At the top
of both `run` and `read`:

```rust
if self.revoked {
    return Err("session revoked".into());
}
```

In `read`, right after `let real = self.resolve(path)?;`:

```rust
if real.file_name() == Some(BAIT.as_ref()) {
    self.revoked = true;
    eprintln!("!! TRIPWIRE TRIGGERED: '{path}' was touched");
    return Err("tripwire: session revoked".into());
}
```

Say: "Everybody wants the keys. These keys were never real." Run: 4 of 4,
the alarm line, and the honest read after it is refused too. "One touch and
the session is over."

Then: "That's about forty lines of checks. Now let's put them behind MCP."
Go to the demo00 slide and follow [`../demo00/LIVE-CODING.md`](../demo00/LIVE-CODING.md).

## If it goes wrong

| Problem | Recovery |
| --- | --- |
| A typo you can't spot | `cp src/steps/stepN.rs src/harness.rs` with the step you just finished, then `cargo run`. |
| Running short on time | Skip the typing: `cargo run -- --step 4`, then `--all` to show the climb from 0 to 4. |
| You want to start over | `git checkout src/harness.rs` puts Exhibit A back. |

`cargo test` checks that every checkpoint stops exactly one more attack than
the one before, and that `src/harness.rs` is still Exhibit A. Run it before
the talk; it fails while you are mid-edit, which is expected.

## Honest limits

This is a teaching harness. The path check runs in this program, not in the
operating system, and the allowed programs keep your permissions: `cargo` can
still run a build script. falcon-mcp's README covers what the full server
does about each.
