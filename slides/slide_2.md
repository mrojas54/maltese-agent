# The Crime Scene 🔍

```rust
// Naive & Vulnerable execution pattern
// Soft prompts fail; raw command execution = host compromise
fn execute_agent_command(cmd: &str) {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd) // Vulnerable to shell injection: e.g. "cargo check; rm -rf /"
        .spawn();
}
```
