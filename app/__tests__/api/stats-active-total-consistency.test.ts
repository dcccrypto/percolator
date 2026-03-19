/**
 * GH#1430: /api/stats totalMarkets must not count corrupt-price markets as active.
 * GH#1435 HOTFIX: /api/stats phantom check must use strict < 1M (NOT <=).
 *
 * GH#1430 root cause: /api/stats used isSaneMarketValue (< 1e18) for last_price
 * before counting active markets, while /api/markets applies sanitizePrice
 * (nulls last_price > $1M) first. Markets with $1M < last_price < 1e18
 * counted as active in stats but not in markets.
 *
 * GH#1432 (REVERTED by GH#1435): PR #1433 changed the vault phantom check to <=
 * to align with /api/markets isPhantomOI. This caused totalMarkets=0 in production
 * because ALL active devnet markets have vault_balance=1_000_000 exactly (the creation
 * deposit minimum). Using <= classifies all of them as phantom.
 *
 * GH#1435 correct fix: use strict < for vault phantom check in /api/stats so that
 * vault=1M markets are correctly counted as active. The mismatch with /api/markets
 * isPhantomOI (which uses <=) must be addressed by fixing /api/markets, not stats.
 *
 * Fix: (1) apply the $1M cap to last_price in phantomAwareData before
 * isActiveMarket() in /api/stats, mirroring /api/markets sanitizePrice.
 * (2) vault_balance phantom check uses strict < (vault=1M is ACTIVE, not phantom).
 */
import { describe, it, expect } from "vitest";
import { isActiveMarket, isSaneMarketValue } from "@/lib/activeMarketFilter";

// ---------------------------------------------------------------------------
// Constants — must match /api/stats and /api/markets
// ---------------------------------------------------------------------------
const MAX_SANE_PRICE_USD = 1_000_000;  // $1M — /api/markets sanitizePrice cap
const MIN_VAULT_FOR_ACTIVE = 1_000_000;

// ---------------------------------------------------------------------------
// Reproduce the phantomAwareData mapping from /api/stats (fixed version)
// ---------------------------------------------------------------------------
type StatsRow = {
  slab_address?: string;
  last_price?: number | null;
  volume_24h?: number | null;
  trade_count_24h?: number | null;
  open_interest_long?: number | null;
  open_interest_short?: number | null;
  total_open_interest?: number | null;
  vault_balance?: number | null;
  total_accounts?: number | null;
  stats_updated_at?: string | null;
  decimals?: number | null;
};

/** Reproduces the buggy phantomAwareData (before fix): no price cap for non-phantoms */
function phantomAwareDataBuggy(statsData: StatsRow[]): StatsRow[] {
  return statsData.map((m) => {
    const accountsCount = (m.total_accounts ?? 0);
    const vaultBal = (m.vault_balance ?? 0);
    const isPhantom = accountsCount === 0 || vaultBal < MIN_VAULT_FOR_ACTIVE;
    if (!isPhantom) return m;  // BUG: returns raw m with possibly corrupt price
    return {
      ...m,
      last_price: 0,
      volume_24h: 0,
      trade_count_24h: 0,
      total_open_interest: 0,
      open_interest_long: 0,
      open_interest_short: 0,
    };
  });
}

/** Reproduces the fixed phantomAwareData: applies $1M price cap + strict < vault check */
function phantomAwareDataFixed(statsData: StatsRow[]): StatsRow[] {
  return statsData.map((m) => {
    const accountsCount = (m.total_accounts ?? 0);
    const vaultBal = (m.vault_balance ?? 0);
    // GH#1435: strict < (vault=1M is NOT phantom — it is the creation deposit amount)
    const isPhantom = accountsCount === 0 || vaultBal < MIN_VAULT_FOR_ACTIVE;
    if (!isPhantom) {
      // GH#1430: Apply sanitizePrice cap before isActiveMarket
      const rawPrice = m.last_price;
      const sanitizedPrice =
        rawPrice != null && rawPrice > 0 && rawPrice <= MAX_SANE_PRICE_USD
          ? rawPrice
          : null;
      if (sanitizedPrice !== rawPrice) {
        return { ...m, last_price: sanitizedPrice };
      }
      return m;
    }
    return {
      ...m,
      last_price: 0,
      volume_24h: 0,
      trade_count_24h: 0,
      total_open_interest: 0,
      open_interest_long: 0,
      open_interest_short: 0,
    };
  });
}

function countActive(data: StatsRow[]): number {
  return data.filter(isActiveMarket).length;
}

// ---------------------------------------------------------------------------
// Market factories
// ---------------------------------------------------------------------------
function market(overrides: Partial<StatsRow> = {}): StatsRow {
  return {
    slab_address: "slab1",
    last_price: 1.5,
    volume_24h: 1000,
    trade_count_24h: 5,
    total_open_interest: 200,
    open_interest_long: 100,
    open_interest_short: 100,
    vault_balance: 5_000_000,
    total_accounts: 3,
    decimals: 6,
    stats_updated_at: new Date().toISOString(),
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Tests — isSaneMarketValue (shared utility, unchanged)
// ---------------------------------------------------------------------------
describe("isSaneMarketValue — price boundary checks", () => {
  it("accepts $1 (valid)", () => expect(isSaneMarketValue(1)).toBe(true));
  it("accepts $1M exactly (valid)", () => expect(isSaneMarketValue(1_000_000)).toBe(true));
  it("accepts $5M (valid per isSaneMarket — < 1e18)", () => expect(isSaneMarketValue(5_000_000)).toBe(true));
  it("rejects null", () => expect(isSaneMarketValue(null)).toBe(false));
  it("rejects 0", () => expect(isSaneMarketValue(0)).toBe(false));
  it("rejects negative", () => expect(isSaneMarketValue(-1)).toBe(false));
  it("rejects sentinel 1e18", () => expect(isSaneMarketValue(1e18)).toBe(false));
});

// ---------------------------------------------------------------------------
// Tests — buggy implementation (documents that $5M passes as active — WRONG)
// ---------------------------------------------------------------------------
describe("buggy phantomAwareData — shows the inconsistency", () => {
  it("incorrectly counts market with $5M corrupt price as active", () => {
    const row = market({
      last_price: 5_000_000, // corrupt, nulled in /api/markets but not in stats (bug)
      volume_24h: 0,
      total_open_interest: 0,
      open_interest_long: 0,
      open_interest_short: 0,
    });
    const processed = phantomAwareDataBuggy([row]);
    // BUG: last_price $5M passes isSaneMarketValue → counted as active
    expect(countActive(processed)).toBe(1); // ← wrong (mismatches /api/markets)
  });
});

// ---------------------------------------------------------------------------
// Tests — fixed implementation
// ---------------------------------------------------------------------------
describe("GH#1430 — fixed phantomAwareData applies $1M cap before isActiveMarket", () => {
  it("correctly rejects corrupt price $5M (> $1M cap)", () => {
    const row = market({
      last_price: 5_000_000,
      volume_24h: 0,
      total_open_interest: 0,
      open_interest_long: 0,
      open_interest_short: 0,
    });
    const processed = phantomAwareDataFixed([row]);
    // FIX: $5M > $1M is nulled, not sane → market NOT counted as active
    expect(countActive(processed)).toBe(0);
  });

  it("accepts valid $1M price as active", () => {
    const row = market({
      last_price: 1_000_000,
      volume_24h: 0,
      total_open_interest: 0,
    });
    const processed = phantomAwareDataFixed([row]);
    expect(countActive(processed)).toBe(1);
  });

  it("rejects price just above cap ($1M + $1)", () => {
    const row = market({
      last_price: 1_000_001,
      volume_24h: 0,
      total_open_interest: 0,
      open_interest_long: 0,
      open_interest_short: 0,
    });
    const processed = phantomAwareDataFixed([row]);
    expect(countActive(processed)).toBe(0);
  });

  it("still counts active via volume_24h when price is corrupt", () => {
    const row = market({
      last_price: 9_000_000, // corrupt
      volume_24h: 50_000,    // valid
      total_open_interest: 0,
    });
    const processed = phantomAwareDataFixed([row]);
    // Active via volume_24h even though price is bad
    expect(countActive(processed)).toBe(1);
  });

  it("still counts active via total_open_interest when price is corrupt", () => {
    const row = market({
      last_price: 9_000_000, // corrupt
      volume_24h: 0,
      total_open_interest: 500,
    });
    const processed = phantomAwareDataFixed([row]);
    expect(countActive(processed)).toBe(1);
  });

  it("phantom market (vault < 1M) is zeroed out and not counted", () => {
    const row = market({ vault_balance: 500_000, total_accounts: 5 });
    const processed = phantomAwareDataFixed([row]);
    expect(countActive(processed)).toBe(0);
  });

  // GH#1435: vault=1M exactly is NOT phantom (strict <), it is the creation deposit amount.
  // All active devnet markets have vault=1M exactly; <= would zero totalMarkets on production.
  it("GH#1435: market with vault === 1M exactly is NOT phantom — it is active", () => {
    const row = market({ vault_balance: 1_000_000, total_accounts: 3 });
    const processed = phantomAwareDataFixed([row]);
    expect(countActive(processed)).toBe(1);
  });

  it("GH#1435: market with vault just below 1M (999_999) IS phantom", () => {
    const row = market({ vault_balance: 999_999, total_accounts: 3 });
    const processed = phantomAwareDataFixed([row]);
    expect(countActive(processed)).toBe(0);
  });

  it("GH#1435: market with vault just above 1M (1_000_001) is also NOT phantom", () => {
    const row = market({ vault_balance: 1_000_001, total_accounts: 3 });
    const processed = phantomAwareDataFixed([row]);
    expect(countActive(processed)).toBe(1);
  });

  it("mixed: 2 valid + 1 corrupt price + 1 phantom = 2 active", () => {
    const rows = [
      market({ slab_address: "a", last_price: 50 }),
      market({ slab_address: "b", last_price: 120 }),
      market({
        slab_address: "c",
        last_price: 9_000_000,  // corrupt
        volume_24h: 0,
        total_open_interest: 0,
        open_interest_long: 0,
        open_interest_short: 0,
      }),
      market({ slab_address: "d", vault_balance: 0, total_accounts: 0 }),
    ];
    const processed = phantomAwareDataFixed(rows);
    expect(countActive(processed)).toBe(2);
  });

  it("does not modify non-corrupt prices unnecessarily", () => {
    const row = market({ last_price: 42.5 });
    const processed = phantomAwareDataFixed([row]);
    expect((processed[0] as StatsRow).last_price).toBe(42.5);
  });
});

// ---------------------------------------------------------------------------
// Alignment check: fixed stats count === /api/markets activeTotal logic
// ---------------------------------------------------------------------------
describe("GH#1430 — fixed stats count aligns with /api/markets sanitizePrice logic", () => {
  const MAX_SANE = 1_000_000;

  // Simulate what /api/markets does: sanitizePrice → null if > $1M
  function sanitizePrice(v: number | null): number | null {
    if (v == null) return null;
    if (!Number.isFinite(v) || v <= 0 || v > MAX_SANE) return null;
    return v;
  }

  it("both treat $1M price as valid active", () => {
    const raw = 1_000_000;
    expect(sanitizePrice(raw)).not.toBeNull(); // /api/markets: valid
    const row = market({ last_price: raw, volume_24h: 0, total_open_interest: 0 });
    expect(countActive(phantomAwareDataFixed([row]))).toBe(1); // stats: active
  });

  it("both treat $1M + 1 price as invalid", () => {
    const raw = 1_000_001;
    expect(sanitizePrice(raw)).toBeNull(); // /api/markets: nulled
    const row = market({
      last_price: raw,
      volume_24h: 0,
      total_open_interest: 0,
      open_interest_long: 0,
      open_interest_short: 0,
    });
    expect(countActive(phantomAwareDataFixed([row]))).toBe(0); // stats: inactive
  });

  it("both treat $500K price as valid", () => {
    const raw = 500_000;
    expect(sanitizePrice(raw)).not.toBeNull();
    const row = market({ last_price: raw, volume_24h: 0, total_open_interest: 0 });
    expect(countActive(phantomAwareDataFixed([row]))).toBe(1);
  });
});
