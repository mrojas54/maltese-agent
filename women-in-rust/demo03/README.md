# demo03: the jail

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

## The exercises

| # | File | You learn |
| --- | --- | --- |
| 0 | `exercises/00_intro/intro1.rs` | The whole demo03 server: jailed through symlinks, and `exec_run` still open. Read it once. |
| 1 | `exercises/02_jail/jail2.rs` | `resolve` asks the filesystem: canonicalize a path that exists, and place one that does not. |

Solutions are in `solutions/`, same layout.

## What's already closed, and what is not

| Path | demo02's spelling check | demo03's `resolve` |
| --- | --- | --- |
| `../secret.txt` | refused | refused |
| `/etc/passwd` | refused | refused |
| a symlink inside the root pointing out | **let through** | refused |
| `sub/new.txt`, `fs_write`'s target: `sub/` exists, `new.txt` does not yet | allowed | allowed, inside the root |

demo02 never had to think about that last row: `sandbox.root.join(rel)` never
touches the filesystem, so a path that does not exist yet resolves exactly
like one that does. demo03 gets there in two steps.

`std::fs::canonicalize` asks the filesystem for a path's real absolute form,
with every `..` and every symlink resolved. `sub/../hello.txt` comes back as
`<root>/hello.txt`. A symlink that points out comes back as the outside path,
and the `starts_with(root)` check refuses it. But `canonicalize` only works on
a path that exists, so on `sub/new.txt` it fails.

That is when `resolve` walks up: drop `new.txt`, canonicalize `sub/` (it
exists) to `<root>/sub`, then add `new.txt` back. `<root>/sub/new.txt` starts
with `<root>`, so it is allowed.

## Commands

| Command | What it does |
| --- | --- |
| `cargo run -- next` | Run the exercises in order; stop at the first that fails. |
| `cargo run -- run NAME` | Check one exercise. |
| `cargo run -- hint NAME` | Print its hint. |
| `cargo run -- list` | List them. |
| `cargo run -- verify` | For CI: every solution passes, every exercise still fails. |

## How it is put together

Same shape as demo00 through demo02.

- `exercises/` and `solutions/` mirror each other; every file in both is a
  `[[bin]]` in `Cargo.toml`.
- `info.toml` is the exercise order and the hints.
- `src/main.rs` hands that file to the runner in [`../runner`](../runner/),
  which every demo shares.
- This package is excluded from the repo's root workspace, because its
  exercises are broken on purpose. CI runs `cargo run -- verify` here.

## Where this goes

`Sandbox::resolve` here is a small version of
[`falcon-mcp/src/sandbox.rs`](../../falcon-mcp/src/sandbox.rs)'s `resolve`:
canonicalize what exists, walk up to place what does not. What demo03 does not
add: an exec allowlist. `exec_run` is still exactly as open as demo01 left it.
That is demo04.
