# Setting up Zed for the live coding

For *The Harness Wore a Trenchcoat*: build the opening `baby-agent`, then
add the four checks in `baby-harness`. Both start unfinished in the places
you edit. demo00 is the homework, in `demo00/WALKTHROUGH.md`. Do everything
up to "The morning of" at home, on good wifi, on the laptop you will present from. Work
through it once, then do one full timed practice run.

## 1. Get the code and build it (about 10 minutes, mostly downloads)

In a normal terminal:

```bash
cd ~/path/to
git clone --branch codex/baby-agent-live-starter https://github.com/mrojas54/maltese-agent.git maltese-agent-live
cd maltese-agent-live/harness-talk/baby-agent
cargo build --locked --bins                 # dependencies and backup, no edits
cargo run --locked -- --replay               # expect: agent loop is not implemented yet

cd ../baby-harness
cargo run                                  # expect: Attacks stopped: 0 of 4
```

The agent's first replay exits with status 1 and no tool call; that is the
starting point. Follow `baby-agent/LIVE-CODING.md` for the code to type. Build
both demos here first. After that, offline rehearsal needs no model request.

## 2. Install the `zed` command

Open Zed, press `cmd-shift-p` and run **cli: install**. After that, `zed .`
opens a folder from the terminal.

## 3. Open the project

```bash
zed ~/path/to/maltese-agent-live/harness-talk/baby-agent
zed ~/path/to/maltese-agent-live/harness-talk/baby-harness
```

Use one window for each crate. Start in baby-agent at `src/main.rs`; after
building the loop, switch to the harness window at `src/harness.rs`. Each is
its own Cargo workspace, so open the crate folder for rust-analyzer.

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

baby-harness has a `.zed/tasks.json`, committed in the repo:

| Window | Task | What it does |
| --- | --- | --- |
| baby-harness | `harness: cargo run` | Plays the 7 requests against your edit |
| baby-harness | `harness: all checkpoints (backup)` | `--all`, if you skip typing |
| baby-harness | `harness: reset to Exhibit A` | `git checkout src/harness.rs`, then run |

Run one with `cmd-shift-p`, then **task: spawn**, then pick it. After that,
rerun the last task with one key. Add this to your keymap (`cmd-shift-p`, then
**zed: open keymap**):

```json
[
  {
    "context": "Workspace",
    "bindings": {
      "cmd-r": ["task::Rerun", { "reevaluate_context": true }]
    }
  }
]
```

Your loop on stage is: type, `cmd-s`, `cmd-r`, read the result out loud.

## 6. Window layout

In the window:

1. Close the project panel (`cmd-b`) so the code has the full width.
2. Open the terminal panel at the bottom (`` ctrl-` ``). Drag it to about a
   third of the screen, which is room for the 7 harness lines.
3. In the agent window, open the unfinished `run` in `src/main.rs` and fold
   `main` below it. In the harness window, open `src/harness.rs`.
4. To make the code fill the screen while you explain it, use
   `shift-escape` (toggle zoom on the pane). Press it again to bring the
   terminal back.

## 7. Practice run, with a timer

1. Run the reset task.
2. Start a stopwatch at the cover slide. Go through the slides, and at the
   opening demo follow `baby-agent/LIVE-CODING.md`. At the harness slide,
   follow `baby-harness/README.md` from step 1 to 4. Say the lines out loud
   and stop the watch at the end of Case Closed.
3. Write down the time at the start of the typing, at its end, and overall.
   Afterwards, run the reset task again.

Measure the total with both live builds before deciding how much to type
in the 30-minute slot. If it runs long, use the explicit agent backup or
paste a harness checkpoint. The agent script and harness README list those
recovery commands.

## The morning of

- Connect to the projector and use mirroring, not an extended display. Then
  you see exactly what the audience sees.
- Turn on Do Not Disturb, and quit Slack, mail and anything else that pops up
  notifications.
- Plug in the power. A long `cargo` build on battery is slower.
- Run the reset task, and then `harness: cargo run` once, so everything is
  built and warm.
- Reset the agent with `git restore -- src/main.rs` from its directory after
  a practice run. This discards only that file's practice edits.
- Keep `baby-agent/LIVE-CODING.md` and `baby-harness/README.md` on your phone
  or a second screen.

## If something breaks on stage

| Problem | Do this |
| --- | --- |
| A harness typo you can't find | In the terminal: `cp src/steps/stepN.rs src/harness.rs`, with N the step you just did |
| Running out of time | Run `harness: all checkpoints (backup)` |
| rust-analyzer stuck or red everywhere | Ignore it. `cargo` is the judge, and red squiggles do not stop `cmd-r` |
| Zed itself misbehaves | Use a normal terminal: the same `cargo run` commands work there |
