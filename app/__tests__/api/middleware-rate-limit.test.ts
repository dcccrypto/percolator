/**
 * Tests for middleware.ts — Upstash Redis distributed rate limiter (GH#1213)
 * and off-by-one fix (GH#1245).
 *
 * KEY NOTE: vi.fn().mockImplementation(() => ...) with an arrow function is
 * NOT usable as a constructor (Vitest 4 enforces this). Ratelimit instances
 * must be created with `new`, so we use a regular `function` implementation.
 *
 * Covers:
 *  - 100 parallel /api/markets requests all return 429 when Redis returns
 *    success:false (the fix for serverless per-instance bypass, GH#1213)
 *  - GH#1245: Upstash success:true + remaining:0 (last allowed req) → 200 not 429
 *  - GH#1245: In-memory off-by-one — request #120 must be ALLOWED, #121 blocked
 *  - In-memory fallback (no Redis env) enforces 120/min per-IP limit
 *  - RPC tier uses a separate 600/min limit bucket
 *  - X-RateLimit-* + Retry-After headers present on 429 responses
 *  - Graceful Redis error fallback → in-memory, no 500s
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { NextRequest } from "next/server";

// ── Shared mock state ─────────────────────────────────────────────────────
// vi.mock() is hoisted; factories capture these variables lazily (on first
// import of the mocked module), by which time they are fully initialized.

const mockLimitFn = vi.fn();

// MUST use a regular function (not arrow) — Vitest 4 requires 'function' or
// 'class' for mocks used with `new`. Arrow functions are not constructors and
// cause a TypeError that the try-catch in getUpstashLimiters() swallows.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const MockRatelimitCtor = vi.fn(function (this: any) {
  this.limit = mockLimitFn;
});
// Attach static method used in: limiter: Ratelimit.slidingWindow(...)
(MockRatelimitCtor as unknown as Record<string, unknown>).slidingWindow = vi
  .fn()
  .mockReturnValue({ kind: "sliding" });

// MUST use regular functions (not arrow) for any mock used with `new`.
// Arrow functions are not constructors; Vitest 4 enforces this and the
// try-catch in getUpstashLimiters() would otherwise swallow the TypeError.
vi.mock("@upstash/redis", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  Redis: vi.fn(function (this: any) { return this; }),
}));
vi.mock("@upstash/ratelimit", () => ({ Ratelimit: MockRatelimitCtor }));

// ── Helpers ────────────────────────────────────────────────────────────────

function makeReq(path = "/api/markets", ip = "1.2.3.4"): NextRequest {
  return new NextRequest(`http://localhost${path}`, {
    headers: { "x-forwarded-for": ip },
  });
}

type MiddlewareFn = (req: NextRequest) => Promise<Response>;

/** Returns a freshly imported middleware (resets module-level singletons). */
async function freshMiddleware(): Promise<MiddlewareFn> {
  vi.resetModules();
  const mod = await import("@/middleware");
  return mod.middleware as unknown as MiddlewareFn;
}

// ── Suite 1: Redis path — limit exhausted → all 429 ───────────────────────
describe("middleware — Upstash Redis distributed rate limiter (GH#1213)", () => {
  let middleware: MiddlewareFn;

  beforeEach(async () => {
    process.env.UPSTASH_REDIS_REST_URL = "https://fake.upstash.io";
    process.env.UPSTASH_REDIS_REST_TOKEN = "fake-token";
    // Every call to limiter.limit() reports the limit is exhausted
    mockLimitFn.mockResolvedValue({
      success: false,
      remaining: 0,
      reset: Date.now() + 60_000,
    });
    middleware = await freshMiddleware();
  });

  afterEach(() => {
    delete process.env.UPSTASH_REDIS_REST_URL;
    delete process.env.UPSTASH_REDIS_REST_TOKEN;
  });

  it("100 parallel /api/markets requests all return 429 when Redis limit exhausted", async () => {
    const requests = Array.from({ length: 100 }, (_, i) =>
      middleware(makeReq("/api/markets", `1.2.3.${i % 255}`)),
    );
    const responses = await Promise.all(requests);
    for (const res of responses) {
      expect(res.status).toBe(429);
    }
  });

  it("429 response includes X-RateLimit-* and Retry-After headers", async () => {
    const res = await middleware(makeReq("/api/markets"));
    expect(res.status).toBe(429);
    expect(res.headers.get("X-RateLimit-Limit")).toBe("120");
    expect(res.headers.get("X-RateLimit-Remaining")).toBe("0");
    expect(res.headers.get("X-RateLimit-Reset")).toBeTruthy();
    expect(res.headers.get("Retry-After")).toBeTruthy();
  });

  it("429 response body is JSON with an error field", async () => {
    const res = await middleware(makeReq("/api/markets"));
    expect(res.status).toBe(429);
    const body = (await res.json()) as { error: string };
    expect(body).toHaveProperty("error");
    expect(typeof body.error).toBe("string");
  });

  it("RPC tier reports limit=600 on 429", async () => {
    const res = await middleware(makeReq("/api/rpc"));
    expect(res.status).toBe(429);
    expect(res.headers.get("X-RateLimit-Limit")).toBe("600");
  });

  it("non-API routes are not rate-limited", async () => {
    const res = await middleware(makeReq("/some-page"));
    expect(res.status).not.toBe(429);
  });

  it("GH#1245: Upstash success:true + remaining:0 (last allowed request) → 200 not 429", async () => {
    // Upstash says the request is allowed but the bucket is now exhausted.
    // Previously `remaining <= 0` incorrectly blocked this request.
    mockLimitFn.mockResolvedValueOnce({
      success: true,
      remaining: 0,
      reset: Date.now() + 60_000,
    });
    const res = await middleware(makeReq("/api/markets", "5.6.7.8"));
    expect(res.status).toBe(200);
    expect(res.headers.get("X-RateLimit-Remaining")).toBe("0");
  });
});

// ── Suite 2: In-memory fallback (no Upstash env vars) ────────────────────
describe("middleware — in-memory fallback (no Upstash env)", () => {
  let middleware: MiddlewareFn;

  beforeEach(async () => {
    delete process.env.UPSTASH_REDIS_REST_URL;
    delete process.env.UPSTASH_REDIS_REST_TOKEN;
    middleware = await freshMiddleware();
  });

  it("GH#1245: allows exactly 120 requests per IP (off-by-one fix)", async () => {
    // The old code blocked request #120 (count == max → remaining == 0 → 429).
    // The fix uses count <= max so request #120 is the LAST allowed request.
    const ip = "10.0.0.1";
    for (let i = 0; i < 120; i++) {
      const res = await middleware(makeReq("/api/markets", ip));
      expect(res.status).not.toBe(429);
    }
  });

  it("returns 429 on request 121 from same IP (in-memory)", async () => {
    const ip = "10.0.0.2";
    for (let i = 0; i < 120; i++) {
      await middleware(makeReq("/api/markets", ip));
    }
    const res = await middleware(makeReq("/api/markets", ip));
    expect(res.status).toBe(429);
  });

  it("different IPs have independent in-memory buckets", async () => {
    for (let i = 0; i < 121; i++) {
      await middleware(makeReq("/api/markets", "10.0.1.1"));
    }
    const res = await middleware(makeReq("/api/markets", "10.0.1.2"));
    expect(res.status).not.toBe(429);
  });
});

// ── Suite 3: Graceful Redis error fallback ────────────────────────────────
describe("middleware — graceful Redis error fallback", () => {
  let middleware: MiddlewareFn;

  beforeEach(async () => {
    process.env.UPSTASH_REDIS_REST_URL = "https://fake.upstash.io";
    process.env.UPSTASH_REDIS_REST_TOKEN = "fake-token";
    // Simulate Redis transient error on every call to limiter.limit()
    mockLimitFn.mockRejectedValue(new Error("Redis ECONNRESET"));
    middleware = await freshMiddleware();
  });

  afterEach(() => {
    delete process.env.UPSTASH_REDIS_REST_URL;
    delete process.env.UPSTASH_REDIS_REST_TOKEN;
  });

  it("falls back to in-memory and returns non-500 when Redis.limit() rejects", async () => {
    const res = await middleware(makeReq("/api/markets", "10.0.3.1"));
    expect(res.status).not.toBe(500);
    expect(res.status).not.toBe(429); // fresh IP under in-memory limit
  });
});
