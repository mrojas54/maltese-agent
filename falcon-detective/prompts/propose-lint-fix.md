You are silencing a Rust clippy lint with a minimal, idiomatic fix.

Issue:
- file: {{file}}
- line: {{line}}
- evidence:

```
{{evidence}}
```

File content:

```rust
{{content}}
```

Apply the smallest correct change. If the lint is `unused_imports`, delete the import. If it's `clippy::unwrap_used`, replace with `?`. If it's a needless clone, drop the clone. Don't over-fix.

Return ONLY:

{ "path": "{{file}}", "diff": "<unified diff>" }
