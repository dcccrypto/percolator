/**
 * Tests for /api/funding/global route
 *
 * NOTE: This route was converted to a thin proxy in GH#1066.
 * Business logic (sanitization, sorting, rate computation) now lives in
 * percolator-api and should be tested there.  These tests verify that
 * the Next.js proxy wrapper forwards upstream responses correctly AND
 * applies the Vercel-layer blocklist filter (GH#1461).
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { NextResponse } from "next/server";

// Mock the shared proxy utility — the route has no other deps
vi.mock("@/lib/api-proxy", () => ({
  proxyToApi: vi.fn(),
}));

import { proxyToApi } from "@/lib/api-proxy";

// Helper: call the route handler directly (module cached after first import)
async function callRoute(url = "http://localhost/api/funding/global?limit=5") {
  const { GET } = await import("@/app/api/funding/global/route");
  return GET(new Request(url));
}

// Known blocked slab addresses (subset used in tests)
const BLOCKED_SLAB_1 = "8eFFEFBY3HHbBgzxJJP5hyxdzMNMAumnYNhkWXErBM4c";
const BLOCKED_SLAB_2 = "3ZKKwsKoo5UP28cYmMpvGpwoFpWLVgEWLQJCejJnECQn";
const BLOCKED_SLAB_3 = "3bmCyPee8GWJR5aPGTyN5EyyQJLzYyD8Wkg9m1Afd1SD";
const BLOCKED_SLAB_4 = "3YDqCJGz88xGiPBiRvx4vrM51mWTiTZPZ95hxYDZqKpJ";
const CLEAN_SLAB = "Bc7A4yCaCUFhp5dv7H4Xkn9kKRnVSfnBYXKREXuqNs2q";

describe("GET /api/funding/global", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it("forwards a 200 response with markets array from upstream", async () => {
    const payload = {
      markets: [
        { slabAddress: "abc123", rateBpsPerSlot: 10, hourlyRatePercent: 0.9, dailyRatePercent: 21.6 },
      ],
      count: 1,
    };
    vi.mocked(proxyToApi).mockResolvedValue(
      NextResponse.json(payload, { status: 200 })
    );

    const res = await callRoute();
    expect(res.status).toBe(200);
    const json = await res.json();
    expect(json.markets).toHaveLength(1);
    expect(json.markets[0].slabAddress).toBe("abc123");
  });

  it("forwards an empty markets array when upstream returns none", async () => {
    vi.mocked(proxyToApi).mockResolvedValue(
      NextResponse.json({ markets: [], count: 0 }, { status: 200 })
    );

    const res = await callRoute();
    expect(res.status).toBe(200);
    const json = await res.json();
    expect(json.markets).toEqual([]);
  });

  // GH#1461: defense-in-depth blocklist filtering at the Vercel layer
  it("strips all 4 known blocked slabs from the upstream response (GH#1461)", async () => {
    const payload = {
      count: 5,
      markets: [
        { slabAddress: BLOCKED_SLAB_1, netLpPosition: "-1000000000000", hourlyRatePercent: 0 },
        { slabAddress: BLOCKED_SLAB_2, netLpPosition: "0", hourlyRatePercent: 0 },
        { slabAddress: BLOCKED_SLAB_3, netLpPosition: "-1000000000000", hourlyRatePercent: 0 },
        { slabAddress: BLOCKED_SLAB_4, netLpPosition: "1000000000000", hourlyRatePercent: 0 },
        { slabAddress: CLEAN_SLAB, netLpPosition: "500", hourlyRatePercent: 1.5 },
      ],
    };
    vi.mocked(proxyToApi).mockResolvedValue(
      NextResponse.json(payload, { status: 200 })
    );

    const res = await callRoute();
    expect(res.status).toBe(200);
    const json = await res.json();

    // All 4 blocked slabs must be gone
    const addresses = json.markets.map((m: { slabAddress: string }) => m.slabAddress);
    expect(addresses).not.toContain(BLOCKED_SLAB_1);
    expect(addresses).not.toContain(BLOCKED_SLAB_2);
    expect(addresses).not.toContain(BLOCKED_SLAB_3);
    expect(addresses).not.toContain(BLOCKED_SLAB_4);

    // Clean slab must remain
    expect(addresses).toContain(CLEAN_SLAB);
    expect(json.markets).toHaveLength(1);
    expect(json.count).toBe(1);
  });

  it("recalculates count to match filtered markets length", async () => {
    const payload = {
      count: 3,
      markets: [
        { slabAddress: BLOCKED_SLAB_1, hourlyRatePercent: 0 },
        { slabAddress: CLEAN_SLAB, hourlyRatePercent: 1.2 },
        { slabAddress: BLOCKED_SLAB_2, hourlyRatePercent: 0 },
      ],
    };
    vi.mocked(proxyToApi).mockResolvedValue(
      NextResponse.json(payload, { status: 200 })
    );

    const res = await callRoute();
    const json = await res.json();
    expect(json.count).toBe(1);
    expect(json.markets).toHaveLength(1);
  });

  it("passes through cleanly when no blocked slabs present", async () => {
    const payload = {
      count: 2,
      markets: [
        { slabAddress: CLEAN_SLAB, hourlyRatePercent: 1.2 },
        { slabAddress: "AnotherCleanSlab1111111111111111111111111111", hourlyRatePercent: 0.5 },
      ],
    };
    vi.mocked(proxyToApi).mockResolvedValue(
      NextResponse.json(payload, { status: 200 })
    );

    const res = await callRoute();
    const json = await res.json();
    expect(json.markets).toHaveLength(2);
    expect(json.count).toBe(2);
  });

  it("forwards 502 when proxy cannot reach upstream", async () => {
    vi.mocked(proxyToApi).mockResolvedValue(
      NextResponse.json({ error: "Backend unavailable" }, { status: 502 })
    );

    const res = await callRoute();
    expect(res.status).toBe(502);
    const json = await res.json();
    expect(json.error).toBeDefined();
  });

  it("forwards 504 when upstream times out", async () => {
    vi.mocked(proxyToApi).mockResolvedValue(
      NextResponse.json({ error: "Backend timeout" }, { status: 504 })
    );

    const res = await callRoute();
    expect(res.status).toBe(504);
    const json = await res.json();
    expect(json.error).toBe("Backend timeout");
  });

  it("forwards 502 when backend URL is not configured", async () => {
    vi.mocked(proxyToApi).mockResolvedValue(
      NextResponse.json({ error: "Backend URL not configured" }, { status: 502 })
    );

    const res = await callRoute();
    expect(res.status).toBe(502);
  });

  it("forwards 500 when upstream returns an internal error", async () => {
    vi.mocked(proxyToApi).mockResolvedValue(
      NextResponse.json({ error: "Internal error" }, { status: 500 })
    );

    const res = await callRoute();
    expect(res.status).toBe(500);
    const json = await res.json();
    expect(json.error).toBeDefined();
  });

  it("passes query params to proxyToApi (limit forwarded)", async () => {
    vi.mocked(proxyToApi).mockResolvedValue(
      NextResponse.json({ markets: [], count: 0 }, { status: 200 })
    );

    await callRoute("http://localhost/api/funding/global?limit=3");
    expect(vi.mocked(proxyToApi)).toHaveBeenCalledOnce();
    expect(vi.mocked(proxyToApi).mock.calls[0][1]).toBe("/funding/global");
  });

  it("calls proxyToApi with the correct backend path", async () => {
    vi.mocked(proxyToApi).mockResolvedValue(
      NextResponse.json({ markets: [], count: 0 }, { status: 200 })
    );

    await callRoute();
    expect(vi.mocked(proxyToApi).mock.calls[0][1]).toBe("/funding/global");
  });
});
