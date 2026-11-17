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
model, one shell tool, and a loop. Our task is to read a note and tell us
where and when to meet.

"The model asks to run a shell command. Our Rust code runs it and sends the
output back. Then the model answers. Let's build this thing."

## At the terminal

Have `src/main.rs` open at the unfinished `run`, with the terminal already
in this directory. Run `cargo run --locked -- --replay`: it stops with
`agent loop is not implemented yet`, with no tool call or meeting answer.

Say: "The connection to the model is ready below. Here we'll write the tool
and the loop." Follow [the live-coding script](LIVE-CODING.md), typing its
four fragments into `run`. After the last fragment, save and run
`cargo run --locked -- "Read note.txt. Where and when is the meeting?"`
with `ANTHROPIC_API_KEY` set, or
`cargo run --locked -- --replay` for offline rehearsal.

For the offline mode, say: "These model replies are scripted for rehearsal.
The shell command and the loop are real."

After the exchange prints, point to these four places in `run`:

1. The `sh` tool description. "This tells the model what it can ask for."
2. `ask(&request)`. "We send the conversation to the model. It can answer
   or request a tool."
3. `Command::new("sh").arg("-c").arg(cmd)`. "Here is where text becomes an
   action. Rust starts a shell with the command the model supplied."
4. The two `msgs.push` calls. "We keep the assistant's request, add our
   result with the matching tool ID, and ask again. That closes the loop."

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
