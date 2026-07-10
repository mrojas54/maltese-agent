// MA-18 / AC-33: backoff between schema-retry attempts; typed transient-error
// detection covering network-level failures. All retry loops run with an
// injected sleep spy (no wall-clock waits) and a fixed RNG (deterministic
// jitter: delay = 0.5 * cap).
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { z } from "zod";
import {
  Gemini,
  type SleepFn,
  classifyTransientError,
  createBackoff,
} from "../../src/lib/gemini.js";

const { generateContentMock } = vi.hoisted(() => ({
  generateContentMock: vi.fn(),
}));

vi.mock("@google/genai", () => ({
  GoogleGenAI: class {
    models = { generateContent: generateContentMock };
  },
}));

describe("classifyTransientError", () => {
  it("classifies numeric status 429 as rate_limit", () => {
    const err = Object.assign(new Error("quota"), { status: 429 });
    expect(classifyTransientError(err)).toEqual({
      transient: true,
      kind: "rate_limit",
      detail: "status 429",
    });
  });

  it("classifies numeric status 503 as unavailable", () => {
    const err = Object.assign(new Error("overloaded"), { status: 503 });
    const check = classifyTransientError(err);
    expect(check).toMatchObject({ transient: true, kind: "unavailable" });
  });

  it("classifies string status UNAVAILABLE as unavailable", () => {
    const err = Object.assign(new Error("try later"), {
      status: "UNAVAILABLE",
    });
    expect(classifyTransientError(err)).toMatchObject({
      transient: true,
      kind: "unavailable",
    });
  });

  it("classifies a Node syscall error (code ECONNRESET) as network", () => {
    const err = Object.assign(new Error("read ECONNRESET"), {
      code: "ECONNRESET",
    });
    expect(classifyTransientError(err)).toMatchObject({
      transient: true,
      kind: "network",
    });
  });

  it("walks the cause chain of a fetch-failed TypeError (ETIMEDOUT)", () => {
    // Node 20 fetch wraps syscall failures: TypeError("fetch failed") with
    // the real error in .cause — the message alone says nothing structured.
    const err = new TypeError("fetch failed", {
      cause: Object.assign(new Error("connect ETIMEDOUT 1.2.3.4:443"), {
        code: "ETIMEDOUT",
      }),
    });
    expect(classifyTransientError(err)).toMatchObject({
      transient: true,
      kind: "timeout",
    });
  });

  it("falls back to message matching: 'fetch failed' is network", () => {
    expect(classifyTransientError(new Error("fetch failed"))).toMatchObject({
      transient: true,
      kind: "network",
    });
  });

  it("falls back to message matching: '503 Service Unavailable'", () => {
    expect(
      classifyTransientError(new Error("got 503 Service Unavailable")),
    ).toMatchObject({ transient: true, kind: "unavailable" });
  });

  it("does not treat client errors as transient", () => {
    const err = Object.assign(new Error("INVALID_ARGUMENT: bad request"), {
      status: 400,
    });
    expect(classifyTransientError(err)).toEqual({ transient: false });
  });

  it("does not treat plain errors as transient", () => {
    expect(classifyTransientError(new Error("something else broke"))).toEqual({
      transient: false,
    });
    expect(classifyTransientError(null)).toEqual({ transient: false });
  });
});

describe("createBackoff", () => {
  it("sleeps uniform(0, cap) and doubles the cap up to maxCapMs", async () => {
    const slept: number[] = [];
    const sleep: SleepFn = async (ms) => {
      slept.push(ms);
    };
    const wait = createBackoff({
      initialCapMs: 1_000,
      maxCapMs: 3_000,
      sleep,
      random: () => 0.5,
    });
    await wait();
    await wait();
    await wait();
    await wait();
    // caps: 1000, 2000, 3000 (clamped), 3000
    expect(slept).toEqual([500, 1000, 1500, 1500]);
  });
});

// ---------------------------------------------------------------------------
// Gemini retry loops (mocked @google/genai, injected sleep — no real timers)
// ---------------------------------------------------------------------------

const schema = z.object({ kind: z.string() });

function makeGemini(slept: number[]): Gemini {
  const sleep: SleepFn = async (ms) => {
    slept.push(ms);
  };
  return new Gemini("live", { sleep, random: () => 0.5 });
}

describe("Gemini API-level transient retry", () => {
  beforeEach(() => {
    generateContentMock.mockReset();
    vi.stubEnv("GEMINI_API_KEY", "test-key");
  });
  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it("retries a network-level failure (ECONNRESET) with jittered backoff", async () => {
    const slept: number[] = [];
    generateContentMock
      .mockRejectedValueOnce(
        Object.assign(new Error("read ECONNRESET"), { code: "ECONNRESET" }),
      )
      .mockResolvedValueOnce({ text: '{"kind":"bug"}' });

    const out = await makeGemini(slept).call({ prompt: "p", schema });
    expect(out).toEqual({ kind: "bug" });
    expect(generateContentMock).toHaveBeenCalledTimes(2);
    expect(slept).toEqual([500]); // 0.5 * 1000ms initial cap
  });

  it("throws non-transient errors immediately without backoff", async () => {
    const slept: number[] = [];
    generateContentMock.mockRejectedValue(
      Object.assign(new Error("INVALID_ARGUMENT"), { status: 400 }),
    );

    await expect(
      makeGemini(slept).call({ prompt: "p", schema }),
    ).rejects.toThrow(/INVALID_ARGUMENT/);
    expect(generateContentMock).toHaveBeenCalledTimes(1);
    expect(slept).toEqual([]);
  });

  it("gives up after 5 attempts on persistent 503, backing off between each", async () => {
    const slept: number[] = [];
    generateContentMock.mockRejectedValue(
      Object.assign(new Error("Service Unavailable"), { status: 503 }),
    );

    await expect(
      makeGemini(slept).call({ prompt: "p", schema }),
    ).rejects.toThrow(/Service Unavailable/);
    expect(generateContentMock).toHaveBeenCalledTimes(5);
    // full jitter at 0.5 with caps 1000, 2000, 4000, 8000; no sleep after last
    expect(slept).toEqual([500, 1000, 2000, 4000]);
  });
});

describe("Gemini schema-retry backoff (AC-33)", () => {
  beforeEach(() => {
    generateContentMock.mockReset();
    vi.stubEnv("GEMINI_API_KEY", "test-key");
  });
  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it("backs off between schema-validation retries and then succeeds", async () => {
    const slept: number[] = [];
    generateContentMock
      .mockResolvedValueOnce({ text: '{"wrong": 1}' })
      .mockResolvedValueOnce({ text: "not json at all" })
      .mockResolvedValueOnce({ text: '{"kind":"lint"}' });

    const out = await makeGemini(slept).call({ prompt: "p", schema });
    expect(out).toEqual({ kind: "lint" });
    expect(generateContentMock).toHaveBeenCalledTimes(3);
    // schema backoff between attempt 1→2 and 2→3: caps 1000 then 2000
    expect(slept).toEqual([500, 1000]);
  });

  it("does not back off after the final failed attempt", async () => {
    const slept: number[] = [];
    generateContentMock.mockResolvedValue({ text: '{"wrong": 1}' });

    await expect(
      makeGemini(slept).call({ prompt: "p", schema }),
    ).rejects.toThrow(/schema validation failed after 3 attempts/);
    expect(generateContentMock).toHaveBeenCalledTimes(3);
    expect(slept).toEqual([500, 1000]); // two waits, none after the third try
  });

  it("re-prompts with the validation error appended on schema retries", async () => {
    const slept: number[] = [];
    generateContentMock
      .mockResolvedValueOnce({ text: '{"wrong": 1}' })
      .mockResolvedValueOnce({ text: '{"kind":"bug"}' });

    await makeGemini(slept).call({ prompt: "original prompt", schema });
    const secondPrompt = generateContentMock.mock.calls[1][0].contents;
    expect(secondPrompt).toContain("original prompt");
    expect(secondPrompt).toContain("PREVIOUS ATTEMPT FAILED VALIDATION");
  });
});
