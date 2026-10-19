# Case Closed 🦀

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Safety over prompt guidance
    let sandbox = Sandbox::new(args.root, args.read_only)?; // jail + allowlist
    let server = FalconMcp::new_with_options(sandbox, args.enable_exec);

    // One binary, two transports
    // --stdio: rmcp::transport::stdio()  |  --http <port>: /mcp
    // ...
}
```
