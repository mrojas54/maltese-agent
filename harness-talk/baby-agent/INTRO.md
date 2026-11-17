# Intro and first agent demo

Target about 60 to 90 seconds before running the first command, plus a short
audience pause. Time this aloud in rehearsal.

## The spoken intro

"Hi everyone. I'm Michelle. I'm a senior software engineer and one of the
Rust NYC organizers.

"Who here has built an agent before?"

Pause for hands, then continue.

"Today we're going to look at what happens when a model asks to read a file
or run a command. Somewhere, a real program has to do that work. I want to
show you that code, because that's where we can put a check.

"Before we put a trenchcoat on it, let's build a very small Rust agent. One
model, one shell tool, and a loop. Our task is to ask it what's in this
folder.

"The model asks to run a shell command. Our Rust code runs it and sends the
output back. Then the model answers. Let's build this thing."

## At the terminal

Have the blank `src/main.rs` open and the terminal already in this directory.
Run `cargo run --locked`: it stops with `agent loop is not implemented yet`,
with no key or tool call needed. Replace the placeholder by typing all four
chunks from [the live-coding script](LIVE-CODING.md).

Say: "We'll type the whole thing: state and tool, the POST, the exit check,
and the command plus its result."

After the fourth chunk, save and run `cargo run --locked` with
`ANTHROPIC_API_KEY` set. Its default prompt is `list files`.

At each natural pause, point to that chunk:

1. `msgs` and `tools`. "The conversation and the tool definition."
2. `client.post`. "Send both, then keep the assistant's reply."
3. `let Some(b) = ... else`. "A tool request, or text and we're done."
4. `Command::new("sh")` and the last `msgs.push`. "Run the command, add its
   output with the same tool ID, and ask again."

For the offline fallback, explicitly run
`cargo run --locked --bin baby-agent-complete -- --replay`. Say: "These
model replies are scripted. The shell command is real."

Then: "We've got an agent. Now let's look at what its tools should be
allowed to do."

## Move to the harness

"I'm going to hold the requests still now, so we can see exactly what each
check changes. The next example has a file tool and a command tool. We'll
run the same requests after every edit."

Switch to `../baby-harness/`, open `src/harness.rs`, and run `cargo run`.
Continue with its four edits and checkpoints. The requests in that demo are
scripted; it does not call a model.

When explaining the boundary, say: "These checks run in our Rust program.
The programs it launches still have my operating-system permissions."
Introduce MCP when connecting this tool code to other clients later in the talk.
