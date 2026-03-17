import { describe, it, vi } from "vitest";
import { NextRequest } from "next/server";

const mockLimitFn = vi.fn();
const MockRatelimitCtor = vi.fn(function(this: any) {
  console.log("MockRatelimitCtor constructor called!");
  this.limit = mockLimitFn;
});
(MockRatelimitCtor as any).slidingWindow = vi.fn(function() {
  console.log("slidingWindow called!");
  return { kind: "sliding" };
});

vi.mock("@upstash/redis", () => {
  const Redis = vi.fn(function(this: any, opts: any) {
    console.log("Redis constructor called with:", opts?.url);
    return this;
  });
  return { Redis };
});
vi.mock("@upstash/ratelimit", () => {
  console.log("@upstash/ratelimit factory called");
  return { Ratelimit: MockRatelimitCtor };
});

describe("trace constructor calls", () => {
  it("runs middleware and traces all constructor calls", async () => {
    process.env.UPSTASH_REDIS_REST_URL = "https://fake.upstash.io";
    process.env.UPSTASH_REDIS_REST_TOKEN = "fake-token";
    mockLimitFn.mockResolvedValue({ success: false, remaining: 0, reset: Date.now() + 60_000 });
    
    vi.resetModules();
    const mod = await import("@/middleware");
    const middleware = mod.middleware as any;
    
    console.log("--- calling middleware ---");
    const req = new NextRequest("http://localhost/api/markets", { headers: { "x-forwarded-for": "1.2.3.4" }});
    const res = await middleware(req);
    console.log("--- middleware done ---");
    console.log("Status:", res.status);
    
    delete process.env.UPSTASH_REDIS_REST_URL;
    delete process.env.UPSTASH_REDIS_REST_TOKEN;
  });
});
