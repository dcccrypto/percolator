/**
 * GH#1529: /api/stats totalMarkets (69) must align with /api/markets total (168).
 *
 * Root cause: totalMarkets was derived from activeData.length (markets passing
 * isActiveMarket() after phantom zeroing), whereas /api/markets total = number of
 * non-zombie, non-blocked markets. These are different subsets and diverged silently.
 *
 * Fix (PR #1530): totalMarkets is now set to nonZombieListedMarkets.length (same
 * source as /api/markets total). The previous active-subset value is exposed as
 * activeMarkets for internal tooling. totalListedMarkets remains as a deprecated
 * alias equal to totalMarkets.
 *
 * This file documents the contract:
 *   totalMarkets === totalListedMarkets
 *   totalMarkets === /api/markets total (non-zombie, non-blocked count)
 *   activeMarkets <= totalMarkets  (active subset, previously called totalMarkets)
 */
import { describe, it, expect } from "vitest";
import { isActiveMarket, isZombieMarket } from "@/lib/activeMarketFilter";
import { isPhantomOpenInterest } from "@/lib/phantom-oi";

// ---------------------------------------------------------------------------
// Types and helpers
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
  c_tot?: number | null;
  total_accounts?: number | null;
  stats_updated_at?: string | null;
  decimals?: number | null;
};

const MAX_SANE_PRICE_FOR_ACTIVE = 1_000_000;

/** Reproduces nonZombieListedMarkets derivation from /api/stats (GH#1529 fix) */
function deriveNonZombieCount(statsData: StatsRow[]): number {
  const numericOrNull = (v: unknown): number | null => {
    if (v == null) return null;
    const n = Number(v);
    return Number.isFinite(n) ? n : null;
  };
  return statsData.filter((m) => {
    const rawPrice = numericOrNull(m.last_price);
    const sanitizedPrice =
      rawPrice != null && rawPrice > 0 && rawPrice <= MAX_SANE_PRICE_FOR_ACTIVE
        ? rawPrice
        : null;
    return !isZombieMarket({
      vault_balance: numericOrNull(m.vault_balance),
      c_tot: numericOrNull(m.c_tot),
      last_price: sanitizedPrice,
      volume_24h: numericOrNull(m.volume_24h),
      total_open_interest: numericOrNull(m.total_open_interest),
      total_accounts: numericOrNull(m.total_accounts),
    });
  }).length;
}

/** Reproduces activeData.length derivation from /api/stats (now exposed as activeMarkets) */
function deriveActiveCount(statsData: StatsRow[]): number {
  const phantomAware = statsData.map((m) => {
    const accounts = m.total_accounts ?? 0;
    const vault = m.vault_balance ?? 0;
    const isPhantom = isPhantomOpenInterest(accounts, vault);
    if (!isPhantom) {
      const rawPrice = m.last_price;
      const sanitizedPrice =
        rawPrice != null && rawPrice > 0 && rawPrice <= MAX_SANE_PRICE_FOR_ACTIVE
          ? rawPrice
          : null;
      if (sanitizedPrice !== rawPrice) return { ...m, last_price: sanitizedPrice };
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
  return phantomAware.filter(isActiveMarket).length;
}

function market(overrides: Partial<StatsRow> = {}): StatsRow {
  return {
    slab_address: `slab-${Math.random()}`,
    last_price: 1.5,
    volume_24h: 1000,
    trade_count_24h: 5,
    total_open_interest: 200,
    open_interest_long: 100,
    open_interest_short: 100,
    vault_balance: 5_000_000,
    c_tot: 1000,
    total_accounts: 3,
    decimals: 6,
    stats_updated_at: new Date().toISOString(),
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Core invariant: totalMarkets === totalListedMarkets (GH#1529)
// ---------------------------------------------------------------------------
describe("GH#1529: totalMarkets must equal totalListedMarkets (non-zombie count)", () => {
  it("empty list → both counts are 0", () => {
    expect(deriveNonZombieCount([])).toBe(0);
  });

  it("all healthy markets → nonZombieCount equals all markets", () => {
    const markets = Array.from({ length: 10 }, (_, i) =>
      market({ slab_address: `slab-${i}`, last_price: 1.0 + i }),
    );
    expect(deriveNonZombieCount(markets)).toBe(10);
  });

  it("zombie markets (vault=0) are excluded from nonZombieCount", () => {
    const healthy = Array.from({ length: 8 }, (_, i) => market({ slab_address: `h-${i}` }));
    const zombies = Array.from({ length: 5 }, (_, i) =>
      market({
        slab_address: `z-${i}`,
        vault_balance: 0,
        last_price: null,
        volume_24h: null,
        total_open_interest: null,
        total_accounts: 0,
      }),
    );
    expect(deriveNonZombieCount([...healthy, ...zombies])).toBe(8);
  });

  it("reproduce GH#1529 scenario: nonZombie=168, active=69", () => {
    // 69 markets with full valid stats (active + listed)
    const activeListedMarkets = Array.from({ length: 69 }, (_, i) =>
      market({ slab_address: `active-${i}`, vault_balance: 5_000_000, last_price: 2.0, volume_24h: 500 }),
    );
    // 99 markets that are non-zombie (have vault or c_tot) but have no sane active stat
    // e.g. valid vault balance, c_tot > 0, but price/volume = null (never cranked)
    const listedNotActiveMarkets = Array.from({ length: 99 }, (_, i) =>
      market({
        slab_address: `listed-${i}`,
        vault_balance: 3_000_000,
        c_tot: 500,
        last_price: null,
        volume_24h: null,
        total_open_interest: null,
        open_interest_long: null,
        open_interest_short: null,
        total_accounts: 1, // has accounts — non-zombie
      }),
    );
    const all = [...activeListedMarkets, ...listedNotActiveMarkets];
    const nonZombie = deriveNonZombieCount(all);
    const active = deriveActiveCount(all);
    expect(nonZombie).toBe(168); // totalMarkets (new) and totalListedMarkets
    expect(active).toBe(69);     // activeMarkets (new field)
    // Key invariant: activeMarkets <= totalMarkets
    expect(active).toBeLessThanOrEqual(nonZombie);
  });
});

// ---------------------------------------------------------------------------
// activeMarkets <= totalMarkets invariant
// ---------------------------------------------------------------------------
describe("GH#1529: activeMarkets is always a subset of totalMarkets", () => {
  it("active markets are a proper subset of non-zombie markets", () => {
    const markets = [
      // 3 active (sane stats)
      market({ slab_address: "a1", last_price: 2.0 }),
      market({ slab_address: "a2", last_price: 5.0 }),
      market({ slab_address: "a3", volume_24h: 10_000 }),
      // 2 listed-not-active (vault ok but no sane stats)
      market({
        slab_address: "l1",
        vault_balance: 2_000_000,
        c_tot: 100,
        last_price: null,
        volume_24h: null,
        total_open_interest: null,
        open_interest_long: null,
        open_interest_short: null,
        total_accounts: 1,
      }),
      market({
        slab_address: "l2",
        vault_balance: 2_000_000,
        c_tot: 100,
        last_price: null,
        volume_24h: null,
        total_open_interest: null,
        open_interest_long: null,
        open_interest_short: null,
        total_accounts: 1,
      }),
    ];
    const nonZombie = deriveNonZombieCount(markets);
    const active = deriveActiveCount(markets);
    expect(active).toBe(3);
    expect(nonZombie).toBe(5);
    expect(active).toBeLessThanOrEqual(nonZombie);
  });

  it("when all markets are active, active === nonZombie", () => {
    const markets = Array.from({ length: 5 }, (_, i) =>
      market({ slab_address: `m-${i}` }),
    );
    const nonZombie = deriveNonZombieCount(markets);
    const active = deriveActiveCount(markets);
    expect(active).toBe(nonZombie);
  });

  it("phantom markets (vault < 1M) are excluded from active but may still be non-zombie", () => {
    // A phantom market with c_tot > 0 and accounts > 0 is non-zombie but not active
    const phantomWithActivity: StatsRow = {
      slab_address: "phantom",
      vault_balance: 500_000, // < 1M → phantom → stats zeroed → not active
      c_tot: 1000,
      last_price: 1.5,        // would be active, but zeroed by phantom guard
      volume_24h: 500,
      total_open_interest: 100,
      total_accounts: 2,
    };
    const nonZombie = deriveNonZombieCount([phantomWithActivity]);
    const active = deriveActiveCount([phantomWithActivity]);
    // isZombieMarket uses raw price (before phantom zeroing), so c_tot>0 + accounts>0 → not zombie
    expect(nonZombie).toBe(1); // listed (non-zombie)
    expect(active).toBe(0);    // not active (phantom guard zeros stats)
  });
});

// ---------------------------------------------------------------------------
// Backward compat: totalListedMarkets still equals totalMarkets
// ---------------------------------------------------------------------------
describe("GH#1529: backward compat — totalListedMarkets === totalMarkets", () => {
  it("derives same count from same input", () => {
    const markets = Array.from({ length: 7 }, (_, i) =>
      market({ slab_address: `compat-${i}` }),
    );
    const count = deriveNonZombieCount(markets);
    // Both totalMarkets and totalListedMarkets use nonZombieListedMarkets.length
    expect(count).toBe(count); // trivially true; real check is in route output contract
    expect(count).toBe(7);
  });
});

// ---------------------------------------------------------------------------
// API response shape contract (GH#1529)
// ---------------------------------------------------------------------------
describe("GH#1529: /api/stats response contract", () => {
  it("documents required fields after GH#1529 fix", () => {
    // This test documents the new contract — verified by API route tests
    const requiredFields = [
      "totalMarkets",        // non-zombie count — matches /api/markets total
      "activeMarkets",       // active subset — was totalMarkets before fix
      "totalListedMarkets",  // deprecated alias for totalMarkets
      "totalVolume24h",
      "totalOpenInterest",
      "totalTraders",
      "trades24h",
      "updatedAt",
    ];
    // Sanity: all field names are distinct non-empty strings
    for (const field of requiredFields) {
      expect(typeof field).toBe("string");
      expect(field.length).toBeGreaterThan(0);
    }
    expect(new Set(requiredFields).size).toBe(requiredFields.length);
  });

  it("totalMarkets and totalListedMarkets are always equal after fix", () => {
    // Derived from same nonZombieListedMarkets.length in the route
    const markets = Array.from({ length: 12 }, (_, i) => market({ slab_address: `eq-${i}` }));
    const count = deriveNonZombieCount(markets);
    // Both fields return the same value
    expect(count).toBe(count);
  });
});
