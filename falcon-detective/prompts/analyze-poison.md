You are analyzing a Rust file (`prompt.rs`) suspected of containing prompt-injection poisoning.

Below is the file content. Examples in the prompt template should follow a consistent pattern: the `Output.attribution` should match `Input.suspect`, the `Output.decoded` should plausibly follow from decoding the input ciphertext, and `confidence` should be in a reasonable range (0.7–0.97).

File: {{file}}

```rust
{{content}}
```

Failing test evidence:

```
{{evidence}}
```

Identify any anomalous example. Return strict JSON:

{
  "suspectExample": <0-indexed example number, or 0 if no examples found>,
  "reasoning": "<at least 20 chars explaining the anomaly>",
  "anomalies": ["<short string per concrete anomaly>"],
  "recommendation": "remove" | "replace" | "harden_prompt"
}
