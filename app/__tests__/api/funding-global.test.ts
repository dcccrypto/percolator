/**
 * Tests for /api/funding/global route
 */

import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock Supabase
const mockSelect = vi.fn();
const mockFrom = vi.fn(() => ({
  select: mockSelect,
}));

vi.mock("@/lib/supabase", () => ({
  getServiceClient: () => ({ from: mockFrom }),
}));

async function callRoute(url = "http://localhost/api/funding/global?limit=5") {
  const { GET } = await import("@/app/api/funding/global/route");
  return GET(new Request(url));
}

describe("GET /api/funding/global", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockFrom.mockReturnValue({ select: mockSelect });
  });

  it("returns empty markets when DB returns no rows", async () => {
    mockSelect.mockReturnValue({
      not: vi.fn().mockReturnValue({
        order: vi.fn().mockResolvedValue({ data: [], error: null }),
      }),
    });

    const res = await callRoute();
    const json = await res.json();
    expect(res.status).toBe(200);
    expect(json.markets).toEqual([]);
  });

  it("sanitizes out-of-range funding rates to zero", async () => {
    const rows = [
      { slab_address: "abc123", funding_rate: 999_999_999, markets: { symbol: "BTC" } },
    ];
    mockSelect.mockReturnValue({
      not: vi.fn().mockReturnValue({
        order: vi.fn().mockResolvedValue({ data: rows, error: null }),
      }),
    });

    const res = await callRoute();
    const json = await res.json();
    expect(res.status).toBe(200);
    // Sanitized to 0 — should appear but with zero rate
    const entry = json.markets.find((m: { slabAddress: string }) => m.slabAddress === "abc123");
    expect(entry?.rateBpsPerSlot).toBe(0);
  });

  it("sorts by absolute funding rate descending", async () => {
    const rows = [
      { slab_address: "low", funding_rate: 1, markets: { symbol: "LOW" } },
      { slab_address: "neg", funding_rate: -50, markets: { symbol: "NEG" } },
      { slab_address: "high", funding_rate: 100, markets: { symbol: "HIGH" } },
    ];
    mockSelect.mockReturnValue({
      not: vi.fn().mockReturnValue({
        order: vi.fn().mockResolvedValue({ data: rows, error: null }),
      }),
    });

    const res = await callRoute();
    const json = await res.json();
    expect(res.status).toBe(200);
    expect(json.markets[0].slabAddress).toBe("high");   // 100 bps
    expect(json.markets[1].slabAddress).toBe("neg");    // |-50| = 50 bps
    expect(json.markets[2].slabAddress).toBe("low");    // 1 bps
  });

  it("computes hourly and daily rate percent correctly", async () => {
    // 1 bps/slot * 9000 slots/hr / 10000 = 0.9%/hr, 21.6%/day
    const rows = [
      { slab_address: "slab1", funding_rate: 1, markets: { symbol: "SOL" } },
    ];
    mockSelect.mockReturnValue({
      not: vi.fn().mockReturnValue({
        order: vi.fn().mockResolvedValue({ data: rows, error: null }),
      }),
    });

    const res = await callRoute();
    const json = await res.json();
    const entry = json.markets[0];
    expect(entry.hourlyRatePercent).toBeCloseTo(0.9, 5);
    expect(entry.dailyRatePercent).toBeCloseTo(21.6, 3);
  });

  it("sets direction correctly", async () => {
    const rows = [
      { slab_address: "pos", funding_rate: 5, markets: { symbol: "A" } },
      { slab_address: "neg", funding_rate: -5, markets: { symbol: "B" } },
      { slab_address: "neu", funding_rate: 0, markets: { symbol: "C" } },
    ];
    mockSelect.mockReturnValue({
      not: vi.fn().mockReturnValue({
        order: vi.fn().mockResolvedValue({ data: rows, error: null }),
      }),
    });

    const res = await callRoute();
    const json = await res.json();
    const byAddr = Object.fromEntries(
      json.markets.map((m: { slabAddress: string; direction: string }) => [m.slabAddress, m.direction])
    );
    expect(byAddr["pos"]).toBe("long_pays_short");
    expect(byAddr["neg"]).toBe("short_pays_long");
    expect(byAddr["neu"]).toBe("neutral");
  });

  it("respects limit param", async () => {
    const rows = Array.from({ length: 20 }, (_, i) => ({
      slab_address: `slab${i}`,
      funding_rate: i + 1,
      markets: { symbol: `T${i}` },
    }));
    mockSelect.mockReturnValue({
      not: vi.fn().mockReturnValue({
        order: vi.fn().mockResolvedValue({ data: rows, error: null }),
      }),
    });

    const res = await callRoute("http://localhost/api/funding/global?limit=3");
    const json = await res.json();
    expect(json.markets.length).toBe(3);
  });

  it("returns 500 on DB error", async () => {
    mockSelect.mockReturnValue({
      not: vi.fn().mockReturnValue({
        order: vi.fn().mockResolvedValue({ data: null, error: { message: "DB fail" } }),
      }),
    });

    const res = await callRoute();
    expect(res.status).toBe(500);
    const json = await res.json();
    expect(json.error).toBeDefined();
  });

  it("handles array-style markets join result", async () => {
    const rows = [
      { slab_address: "slab99", funding_rate: 10, markets: [{ symbol: "ETH" }] },
    ];
    mockSelect.mockReturnValue({
      not: vi.fn().mockReturnValue({
        order: vi.fn().mockResolvedValue({ data: rows, error: null }),
      }),
    });

    const res = await callRoute();
    const json = await res.json();
    expect(json.markets[0].baseSymbol).toBe("ETH");
  });
});
