# Setting up Zed for the live coding

For Act 3 of *The Harness Wore a Trenchcoat*: you type the harness
(`live-harness`), then all five demo00 exercises. Do everything up to "The
morning of" at home, on good wifi, on the laptop you will present from. Work
through it once, then do one full timed practice run.

## 1. Get the code and build it (about 10 minutes, mostly downloads)

In a normal terminal:

```bash
cd ~/path/to/maltese-agent
git fetch origin
git switch claude/project-thread-rgmu5q   # use main once PR #201 is merged
git pull

cd women-in-rust/live-harness
cargo run                                  # expect: Attacks stopped: 0 of 4

cd ../demo00
cargo build --bins                         # downloads rmcp, tokio, serde, ...
cargo run -- next                          # expect: stops at schema1
```

Both projects must build here first. demo00 pulls crates from the internet
the first time. After that, both work offline.

## 2. Install the `zed` command

Open Zed, press `cmd-shift-p` and run **cli: install**. After that, `zed .`
opens a folder from the terminal.

## 3. Open each project in its own window

```bash
zed ~/path/to/maltese-agent/women-in-rust/live-harness
zed ~/path/to/maltese-agent/women-in-rust/demo00
```

Use two windows, not one window on the whole repo. Each folder is its own
Cargo workspace, and rust-analyzer behaves best when the folder you open is
the crate. On macOS, `` cmd-` `` switches between Zed windows.

The first time you open a `.rs` file, Zed downloads rust-analyzer. That is
one more reason to do this at home. Wait until the spinner in the status bar
stops and hovering over a type shows its definition.

## 4. Settings for a projector

`cmd-,` opens your settings.json. Add these, keeping whatever else you have:

```json
{
  "buffer_font_size": 24,
  "ui_font_size": 20,
  "terminal": { "font_size": 22 },
  "theme": { "mode": "dark", "dark": "One Dark", "light": "One Light" },

  "disable_ai": true,
  "show_edit_predictions": false,
  "inlay_hints": { "enabled": false },
  "git": { "inline_blame": { "enabled": false } },
  "minimap": { "show": "never" },
  "format_on_save": "on"
}
```

Why each of these:

- **Font sizes:** 24 is the minimum the back row can read. Try it on the
  venue screen if you can, and go bigger if the room is deep. `cmd-=` and
  `cmd--` change the font during the talk.
- **`disable_ai` and `show_edit_predictions`:** the most important two. AI
  completions would suggest the answer before you type it, which spoils the
  live coding and looks staged. Check: type `if self.` in harness.rs, and
  nothing grey should appear ahead of the cursor.
- **Inlay hints, inline blame, minimap off:** less clutter on screen. The
  audience should read only your code.
- **`format_on_save`:** rustfmt tidies what you typed on every `cmd-s`, so
  a quick, messy edit still looks right on screen.

If Zed underlines one of these keys, your version spells it differently.
Search the settings page for "edit predictions" and "AI" and turn them off
there.

## 5. One key to run everything

Both folders now have a `.zed/tasks.json`, committed in the repo:

| Window | Task | What it does |
| --- | --- | --- |
| live-harness | `harness: cargo run` | Plays the 7 requests against your edit |
| live-harness | `harness: all checkpoints (backup)` | `--all`, if you skip typing |
| live-harness | `harness: reset to Exhibit A` | `git checkout src/harness.rs`, then run |
| demo00 | `demo00: next` | Checks the exercise you are on |
| demo00 | `demo00: reset every exercise` | `git checkout exercises/`, then `next` |

Run one with `cmd-shift-p`, then **task: spawn**, then pick it. After that,
rerun the last task with one key. Add this to your keymap (`cmd-shift-p`, then
**zed: open keymap**):

```json
[
  {
    "context": "Workspace",
    "bindings": { "cmd-r": "task::Rerun" }
  }
]
```

Your loop on stage is: type, `cmd-s`, `cmd-r`, read the result out loud.

## 6. Window layout

In each window:

1. Close the project panel (`cmd-b`) so the code has the full width.
2. Open the terminal panel at the bottom (`` ctrl-` ``). Drag it to about a
   third of the screen, which is room for the 7 harness lines or the demo00
   result.
3. Open the starting file: `src/harness.rs` in live-harness, and
   `exercises/01_schema/schema1.rs` in demo00.
4. To make the code fill the screen while you explain it, use
   `shift-escape` (toggle zoom on the pane). Press it again to bring the
   terminal back.

## 7. Practice run, with a timer

1. Run both reset tasks.
2. Start a stopwatch. Follow `live-harness/README.md` from step 1 to 4,
   then `demo00/LIVE-CODING.md` from schema1 to stdio1, saying the lines
   out loud.
3. Write down the time for each part. Afterwards, run both reset tasks again.

If you forget to reset demo00, its `verify` check and CI fail, because they
expect every exercise to still have its hole.

## The morning of

- Connect to the projector and use mirroring, not an extended display. Then
  you see exactly what the audience sees.
- Turn on Do Not Disturb, and quit Slack, mail and anything else that pops up
  notifications.
- Plug in the power. A long `cargo` build on battery is slower.
- Run both reset tasks, and then `harness: cargo run` and `demo00: next`
  once each, so everything is built and warm.
- Keep the two scripts on your phone or a second screen.

## If something breaks on stage

| Problem | Do this |
| --- | --- |
| A harness typo you can't find | In the terminal: `cp src/steps/stepN.rs src/harness.rs`, with N the step you just did |
| A demo00 typo | `cargo run -- hint NAME`, or copy the file from `solutions/` |
| Running out of time | Run `harness: all checkpoints (backup)`, then copy the remaining demo00 solutions and run `next` |
| rust-analyzer stuck or red everywhere | Ignore it. `cargo` is the judge, and red squiggles do not stop `cmd-r` |
| Zed itself misbehaves | Use a normal terminal: the same `cargo run` commands work there |
