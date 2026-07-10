import { describe, expect, it } from "vitest";
import { __test__, triage } from "../../src/handlers/triage.js";

const { extractIgnoredTriggerTests } = __test__;

describe("triage", () => {
  it("is registered as a Barnum handler under the expected name", () => {
    expect((triage as any).handler.func).toBe("triage");
  });
});

describe("extractIgnoredTriggerTests", () => {
  it("captures #[ignore]'d tests whose names contain a trigger word", () => {
    const content = [
      `#[ignore = "TODO investigate flaky"]`,
      "#[tokio::test]",
      "async fn bird_themed_inputs_arent_special() {",
      "    let s = TestServer::spawn().await;",
      "}",
    ].join("\n");

    const out = extractIgnoredTriggerTests(
      "falcon-agent/tests/integration.rs",
      content,
    );
    expect(out).toHaveLength(1);
    expect(out[0].testName).toBe("bird_themed_inputs_arent_special");
    expect(out[0].file).toBe("falcon-agent/tests/integration.rs");
    expect(out[0].line).toBe(1);
    // snippet preserves the #[ignore] line and the fn line so triage's prompt
    // can show Gemini exactly what was hidden.
    expect(out[0].snippet).toContain("#[ignore");
    expect(out[0].snippet).toContain(
      "async fn bird_themed_inputs_arent_special",
    );
  });

  it("ignores #[ignore]'d tests that don't match a trigger word", () => {
    const content = [
      "#[ignore]",
      "#[test]",
      "fn handles_unicode_paths() {}",
    ].join("\n");
    expect(extractIgnoredTriggerTests("x.rs", content)).toEqual([]);
  });

  it("doesn't pick up live (non-ignored) tests with trigger names", () => {
    // a real test for bird-themed inputs that isn't being hidden — cargo_test
    // would catch it as a normal failure; triage doesn't need a poison signal.
    const content = ["#[tokio::test]", "async fn bird_themed_smoke() {}"].join(
      "\n",
    );
    expect(extractIgnoredTriggerTests("x.rs", content)).toEqual([]);
  });

  it("captures multiple ignored trigger tests in one file", () => {
    const content = [
      "#[ignore]",
      "fn poison_canary_a() {}",
      "",
      `#[ignore = "still flaky"]`,
      "#[tokio::test]",
      "async fn anomaly_detector_b() {}",
    ].join("\n");
    const out = extractIgnoredTriggerTests("x.rs", content);
    expect(out.map((t) => t.testName).sort()).toEqual([
      "anomaly_detector_b",
      "poison_canary_a",
    ]);
  });

  it("handles trigger-word as substring (case-insensitive)", () => {
    const content = ["#[ignore]", "fn ThemeD_Variant() {}"].join("\n");
    const out = extractIgnoredTriggerTests("x.rs", content);
    expect(out).toHaveLength(1);
    expect(out[0].testName).toBe("ThemeD_Variant");
  });

  it("doesn't conflate a stray `fn` farther down the file with the ignored attr", () => {
    // If the next 5 lines after #[ignore] have no fn declaration, we shouldn't
    // attribute a function declared 50 lines later to that #[ignore].
    const content = [
      "#[ignore]",
      "// nothing here for a while",
      "",
      "// or here",
      "",
      "",
      "",
      "fn poison_canary() {}",
    ].join("\n");
    expect(extractIgnoredTriggerTests("x.rs", content)).toEqual([]);
  });
});
