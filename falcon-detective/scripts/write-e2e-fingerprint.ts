// Writes fixtures/cassettes/.e2e-fingerprint.json — the manifest the e2e's
// cassette-staleness fast path compares against (MA-26; see
// tests/helpers/e2eFingerprint.ts). Run this immediately after a
// GEMINI_MODE=record session, then commit it alongside the cassettes:
//
//   GEMINI_MODE=record GEMINI_API_KEY=... npm run fix -- --target ../falcon-agent --run-name record-$(date +%s)
//   npm run e2e:fingerprint
//   git add fixtures/cassettes/
//
// Without a manifest the e2e treats the cassettes as stale (it cannot prove
// freshness) and skips fast — or fails under E2E_REQUIRED=1.
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { DEFAULT_MODEL } from "../src/lib/models.js";
import {
  computeWorkflowFingerprint,
  writeFingerprintManifest,
} from "../tests/helpers/e2eFingerprint.js";

const HERE = dirname(fileURLToPath(import.meta.url));
const PKG_ROOT = join(HERE, "..");
const REPO_ROOT = join(PKG_ROOT, "..");

const cassetteDir =
  process.env.CASSETTE_DIR ?? join(PKG_ROOT, "fixtures", "cassettes");

const components = computeWorkflowFingerprint({
  repoRoot: REPO_ROOT,
  workflowPaths: [join(PKG_ROOT, "src"), join(PKG_ROOT, "prompts")],
  model: DEFAULT_MODEL,
});
const path = writeFingerprintManifest(cassetteDir, components);
console.log(`[e2e:fingerprint] wrote ${path}`);
console.log(JSON.stringify(components, null, 2));
