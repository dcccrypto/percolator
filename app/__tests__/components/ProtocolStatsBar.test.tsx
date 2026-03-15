/**
 * GH#1274 — ProtocolStatsBar OI underreport regression tests.
 *
 * Root cause: price fallback was `0`, making toUsd() short-circuit to 0 for
 * every market without an oracle price. Fix: fallback to $1 (matches api/stats).
 */

import { vi, describe, it, expect, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import { ProtocolStatsBar } from "@/components/dashboard/ProtocolStatsBar";
import "@testing-library/jest-dom";

// ── Mocks ─────────────────────────────────────────────────────────────────────

// The query chain is: getSupabase().from().select().returns()
// We need to mock the full chain correctly.
const mockReturns = vi.fn();
const mockSelect = vi.fn(() => ({ returns: mockReturns }));
const mockFrom = vi.fn(() => ({ select: mockSelect }));

vi.mock("@/lib/supabase", () => ({
  getSupabase: () => ({ from: mockFrom }),
}));

vi.mock("@/lib/blocklist", () => ({
  isBlockedSlab: vi.fn(() => false),
}));

// ── Helper ────────────────────────────────────────────────────────────────────

type MarketRow = {
  slab_address: string;
  symbol: string | null;
  volume_24h: number | null;
  last_price: number | null;
  decimals: number | null;
  total_open_interest: number | null;
  open_interest_long: number | null;
  open_interest_short: number | null;
};

function makeRow(overrides: Partial<MarketRow> = {}): MarketRow {
  return {
    slab_address: "slab-abc",
    symbol: "TST-PERP",
    volume_24h: null,
    last_price: null,
    decimals: 6,
    total_open_interest: null,
    open_interest_long: null,
    open_interest_short: null,
    ...overrides,
  };
}

function mockSupabase(rows: MarketRow[]) {
  mockReturns.mockResolvedValue({ data: rows });
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("ProtocolStatsBar — GH#1274 price fallback", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("shows $0 OI when there are no markets", async () => {
    mockSupabase([]);
    render(<ProtocolStatsBar />);
    await waitFor(() => {
      expect(screen.getByText("Open Interest")).toBeInTheDocument();
      expect(screen.getAllByText("$0").length).toBeGreaterThan(0);
    });
  });

  it("shows non-zero OI when last_price is null — GH#1274 regression", async () => {
    // Simulates the actual GH#1274 scenario: $53.8K OI across admin-mode markets
    // 53,800 tokens at $1 fallback = $53.8K; raw = 53800 × 10^6 = 53_800_000_000
    const rawOi = 53_800 * 1_000_000;
    mockSupabase([
      makeRow({
        slab_address: "slab-admin-1",
        last_price: null,  // no oracle price — must fall back to $1
        total_open_interest: rawOi,
        decimals: 6,
      }),
    ]);

    render(<ProtocolStatsBar />);

    await waitFor(() => {
      // $53,800 → "$53.8K"
      expect(screen.getByText("$53.8K")).toBeInTheDocument();
    });
  });

  it("shows correct OI when last_price is provided", async () => {
    // 1000 tokens at $2 each = $2000 OI; raw = 1000 × 10^6
    const rawOi = 1000 * 1_000_000;
    mockSupabase([
      makeRow({
        slab_address: "slab-oracle-1",
        last_price: 2.0,
        total_open_interest: rawOi,
        decimals: 6,
      }),
    ]);

    render(<ProtocolStatsBar />);

    await waitFor(() => {
      expect(screen.getByText("$2.0K")).toBeInTheDocument();
    });
  });

  it("OI must not be $0 when raw OI is sane and last_price is null", async () => {
    // 100K tokens × 10^6 = 10^11 raw → $100K at $1 fallback
    const rawOi = 100_000 * 1_000_000;
    mockSupabase([
      makeRow({
        slab_address: "slab-no-price",
        last_price: null,
        total_open_interest: rawOi,
        decimals: 6,
      }),
    ]);

    render(<ProtocolStatsBar />);

    await waitFor(() => {
      expect(screen.getByText("$100.0K")).toBeInTheDocument();
    });
  });

  it("counts active markets when last_price is null but OI is sane", async () => {
    const rawOi = 1_000_000 * 1_000_000;
    mockSupabase([
      makeRow({ slab_address: "s1", last_price: null, total_open_interest: rawOi }),
      makeRow({ slab_address: "s2", last_price: null, total_open_interest: rawOi }),
      makeRow({ slab_address: "s3", last_price: null, total_open_interest: rawOi }),
    ]);

    render(<ProtocolStatsBar />);

    await waitFor(() => {
      expect(screen.getByText("Active Markets")).toBeInTheDocument();
      expect(screen.getByText("3")).toBeInTheDocument();
    });
  });

  it("ignores blocked slabs", async () => {
    const { isBlockedSlab } = await import("@/lib/blocklist");
    (isBlockedSlab as ReturnType<typeof vi.fn>).mockImplementation(
      (addr: string) => addr === "slab-blocked"
    );

    const rawOi = 50_000 * 1_000_000;
    mockSupabase([
      makeRow({ slab_address: "slab-blocked", last_price: null, total_open_interest: rawOi }),
      makeRow({ slab_address: "slab-ok", last_price: null, total_open_interest: rawOi }),
    ]);

    render(<ProtocolStatsBar />);

    await waitFor(() => {
      // Only slab-ok counts: 1 active market, $50K OI
      expect(screen.getByText("$50.0K")).toBeInTheDocument();
      expect(screen.getByText("1")).toBeInTheDocument();
    });
  });
});
