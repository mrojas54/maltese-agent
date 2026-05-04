You are fixing a Rust bug. Output a unified diff that resolves it.

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

Common bug classes for this codebase:
- missing input sanitization on user-controlled fields → add a validator function
- missing output schema validation on LLM responses → add a serde struct + validation pass
- failing test → fix the underlying logic so the test passes

Return ONLY:

{ "path": "{{file}}", "diff": "<unified diff>" }
