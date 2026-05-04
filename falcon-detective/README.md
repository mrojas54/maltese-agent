# falcon-detective

The Barnum workflow that triages, fixes, and verifies bugs in `falcon-agent`. Orchestrates Gemini reasoning + falcon-mcp tools.

## Run

```bash
npm install
npm run build
npm run fix -- --target ../falcon-agent
```

See `docs/superpowers/specs/2026-04-30-maltese-agent-design.md` §5 for the design.
