# Secure Lifecycle Loop 🔄

```rust
// State-machine representation of our safe execution pipeline
enum SecurityStage {
    Ingest(JsonRpcFrame),  // Protocol verification (serde + schemars)
    Filter(Allowlist),     // Command match, --enable-exec gate (no shell)
    Confine(PathBuf),      // Kernel canonicalization (starts_with)
    Guard(ReadOnly),       // --read-only refuses every mutating tool
    Execute(Timeout),      // tokio::time::timeout per tool family
    Audit(Stderr),         // stdio isolation (stdout clean)
    // Next case: Tripwire(CanaryAlert), decoy interception
}
```
