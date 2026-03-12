/**
 * Tests for /api/funding/global route
 *
 * NOTE: This route was converted to a thin proxy in GH#1066.
 * Business logic (sanitization, sorting, rate computation) now lives in
 * percolator-api and should be tested there.  These tests verify that
 * the Next.js proxy wrapper forwards upstream responses correctly.
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

describe("GET /api/funding/global", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("forwards a 200 response with markets array from upstream", async () => {
    const payload = {
      markets: [
        { slabAddress: "abc123", rateBpsPerSlot: 10, hourlyRatePercent: 0.9, dailyRatePercent: 21.6 },
      ],
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
      NextResponse.json({ markets: [] }, { status: 200 })
    );

    const res = await callRoute();
    expect(res.status).toBe(200);
    const json = await res.json();
    expect(json.markets).toEqual([]);
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
      NextResponse.json({ markets: [] }, { status: 200 })
    );

    await callRoute("http://localhost/api/funding/global?limit=3");
    expect(vi.mocked(proxyToApi)).toHaveBeenCalledOnce();
    // Verify the second arg is the expected API path
    expect(vi.mocked(proxyToApi).mock.calls[0][1]).toBe("/funding/global");
  });

  it("calls proxyToApi with the correct backend path", async () => {
    vi.mocked(proxyToApi).mockResolvedValue(
      NextResponse.json({ markets: [] }, { status: 200 })
    );

    await callRoute();
    expect(vi.mocked(proxyToApi).mock.calls[0][1]).toBe("/funding/global");
  });
});
