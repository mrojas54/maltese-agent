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
model, one tool, and a loop. Our task is to read a note and tell us where and
when to meet.

"The model asks for the file. Our Rust code opens it and sends the contents
back. Then the model answers. Let's build this thing."

## At the terminal

Have `src/main.rs` open, with the terminal already in this directory. Run
`cargo run --locked` with `GEMINI_API_KEY` set, or use
`cargo run --locked -- --replay` for the offline rehearsal.

For the offline mode, say: "These model replies are scripted for rehearsal.
The file read and the loop are real."

After the exchange prints, point to these four places in `run`:

1. The `read_file` description. "This tells the model what it can ask for."
2. `ask(&request)`. "We send the conversation to the model. It can answer
   or request a tool."
3. `std::fs::read_to_string(path)`. "Here is where text becomes an action.
   Rust opens the file. This version opens whichever path the model supplies."
4. The two `history.push` calls. "We keep the model's request, add the tool's
   result, and ask again. That closes the loop."

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
