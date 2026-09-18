# demo03: the jail

**Work in progress.** `jail2`'s reference fix has an open `TODO(human)` in
[`solutions/02_jail/jail2.rs`](solutions/02_jail/jail2.rs): the branch for a
path that does not exist yet. Until it lands, `cargo run -- verify` fails on
purpose, this package is not in the root workspace `exclude` list or the CI
tutorial matrix, and there is no `00_intro` — the whole-server exercise every
other demo opens with needs a working `resolve`, and this one still panics the
moment it tries to place a brand-new path.

demo02 built `resolve` by reading a path's spelling: no `..`, no leading `/`,
done. It has a hole. A symlink inside the root is spelled like any other name,
so that check waves it through, and the filesystem follows it to wherever it
really points. demo03 closes the hole by asking the filesystem instead of
reading the spelling: canonicalize the path, then check the answer still
starts with the root.

```bash
cd women-in-rust/demo03
cargo run -- next
```

Same loop as before. `next` runs the exercises in order and stops at the first
that needs you. Open the file it names, read the comment at the top, fix the
`TODO`s, run `next` again.

```bash
cargo run -- hint jail2
```

(No hint is written yet — see above.)

## The exercises

| # | File | You learn |
| --- | --- | --- |
| 1 | `exercises/02_jail/jail2.rs` | `resolve` asks the filesystem: canonicalize a path that exists, and place one that does not. |

Solutions are in `solutions/`, same layout. `solutions/02_jail/jail2.rs` is
itself unfinished right now; the gap is the same `todo!("place a path that
does not exist yet")` you will meet in the exercise.

## What's already closed, and what is not

| Path | demo02's spelling check | demo03's `resolve` |
| --- | --- | --- |
| `../secret.txt` | refused | refused |
| `/etc/passwd` | refused | refused |
| a symlink inside the root pointing out | **let through** | refused |
| a brand-new path (`fs_write`'s target) | resolves cleanly | pending — `todo!()` |

demo02 never had to think about that last row: `sandbox.root.join(rel)` never
touches the filesystem, so a path that does not exist yet resolves exactly
like one that does. demo03's canonicalize-first approach is what makes that
row hard in the first place — `canonicalize` only works on paths that exist.

## Commands

| Command | What it does |
| --- | --- |
| `cargo run -- next` | Run the exercises in order; stop at the first that fails. |
| `cargo run -- run NAME` | Check one exercise. |
| `cargo run -- hint NAME` | Print its hint. |
| `cargo run -- list` | List them. |
| `cargo run -- verify` | For CI: every solution passes, every exercise still fails. **Currently fails** — see above. |

## How it is put together

Same shape as demo00 through demo02.

- `exercises/` and `solutions/` mirror each other; every file in both is a
  `[[bin]]` in `Cargo.toml`.
- `info.toml` is the exercise order and the hints.
- `src/main.rs` hands that file to the runner in [`../runner`](../runner/),
  which every demo shares.
- This package is *not yet* excluded from the repo's root workspace or listed
  in the CI tutorial matrix — both wait on the TODO above landing, so CI never
  runs a `verify` here that it already knows will fail.

## Where this goes

`Sandbox::resolve` here is a small version of
[`falcon-mcp/src/sandbox.rs`](../../falcon-mcp/src/sandbox.rs)'s `resolve`:
canonicalize what exists, walk up to place what does not. What demo03 does not
add: an exec allowlist. `exec_run` is still exactly as open as demo01 left it.
That is demo04.
