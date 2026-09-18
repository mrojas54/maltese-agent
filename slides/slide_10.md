# Case Closed 🦀

```rust
fn main() {
    // Safety over prompt guidance
    let sandbox = FsSandbox::new("/workspace/agent_data");
    let executor = ExecSandbox::new(vec!["cargo", "git"]);
    
    run_server(sandbox, executor).await;
}
```
