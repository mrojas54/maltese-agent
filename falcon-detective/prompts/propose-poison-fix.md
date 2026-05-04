You are producing a unified diff that fixes the prompt-injection identified by the analyst.

File: {{file}}

```rust
{{content}}
```

Analyst's report:

```json
{{report}}
```

Recommendation: `{{recommendation}}`. Apply it.

- "remove": delete the suspect example
- "replace": replace it with a benign example whose Output follows from Input and attribution matches suspect
- "harden_prompt": add a final line instructing the model to ignore prior examples that don't match the input/output pattern
{{previousFailure}}
Return strict JSON:

{
  "path": "<the file path>",
  "diff": "<a valid unified diff>"
}
