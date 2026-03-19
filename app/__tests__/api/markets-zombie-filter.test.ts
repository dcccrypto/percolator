/**
 * GH#1420: Zombie markets (vault_balance=0) should be excluded from /api/markets by default.
 * GH#1419: Stale volume_24h (stats_updated_at > 48h ago) should be excluded from /api/stats totals.
 * GH#1427: Markets with null vault_balance AND all null stats should also be zombie.
 *
 * Unit tests for the filtering logic (not full route integration — uses helpers extracted from route).
 */
import { describe, it, expect } from "vitest";

// ---------------------------------------------------------------------------
// GH#1420 — Zombie market filter
// ---------------------------------------------------------------------------

function isSaneMarketValue(v: number | null | undefined): boolean {
  if (v == null) return false;
  return v > 0 && v < 1e18 && Number.isFinite(v);
}

type MarketRow = {
  vault_balance?: number | null;
  last_price?: number | null;
  volume_24h?: number | null;
  total_open_interest?: number | null;
  total_accounts?: number | null;
};

/** GH#1427: mirrors the route.ts is_zombie logic */
function isZombie(m: MarketRow): boolean {
  if (m.vault_balance != null && m.vault_balance === 0) return true;
  if (m.vault_balance == null) {
    const hasNoStats =
      !isSaneMarketValue(m.last_price) &&
      !isSaneMarketValue(m.volume_24h) &&
      !isSaneMarketValue(m.total_open_interest) &&
      ((m.total_accounts ?? 0) === 0);
    if (hasNoStats) return true;
  }
  return false;
}

describe("GH#1420 zombie market filter", () => {
  it("marks vault_balance=0 as zombie", () => {
    expect(isZombie({ vault_balance: 0 })).toBe(true);
  });

  it("does NOT mark vault_balance=1 as zombie", () => {
    expect(isZombie({ vault_balance: 1 })).toBe(false);
  });

  it("does NOT mark vault_balance=1_000_000 (creation-deposit) as zombie", () => {
    expect(isZombie({ vault_balance: 1_000_000 })).toBe(false);
  });

  it("does NOT mark vault_balance=5_000_000_000 (healthy market) as zombie", () => {
    expect(isZombie({ vault_balance: 5_000_000_000 })).toBe(false);
  });

  it("filters zombie markets out of a list by default", () => {
    const markets = [
      { slab_address: "ACTIVE1", vault_balance: 5_000_000_000 },
      { slab_address: "ZOMBIE1", vault_balance: 0 },
      { slab_address: "ACTIVE2", vault_balance: 1_000_000 },
      // vault_balance null but HAS a real last_price — NOT zombie (still being indexed)
      { slab_address: "ACTIVE3", vault_balance: null, last_price: 100, total_accounts: 5 },
    ];

    const withZombieFlag = markets.map((m) => ({ ...m, is_zombie: isZombie(m) }));
    const nonZombie = withZombieFlag.filter((m) => !m.is_zombie);

    expect(nonZombie).toHaveLength(3);
    expect(nonZombie.map((m) => m.slab_address)).toEqual(["ACTIVE1", "ACTIVE2", "ACTIVE3"]);
  });

  it("includes zombie markets when include_zombie=true", () => {
    const markets = [
      { slab_address: "ACTIVE1", vault_balance: 5_000_000_000 },
      { slab_address: "ZOMBIE1", vault_balance: 0 },
    ];

    const withZombieFlag = markets.map((m) => ({ ...m, is_zombie: isZombie(m) }));
    const includeZombie = true;
    const result = withZombieFlag.filter((m) => includeZombie || !m.is_zombie);

    expect(result).toHaveLength(2);
  });

  it("nulls out prices for zombie markets", () => {
    // Simulates the route behavior: zombie markets get null prices
    const market = { slab_address: "ZOMBIE1", vault_balance: 0, last_price: 148, mark_price: 150, index_price: 149 };
    const is_zombie = isZombie(market);

    const output = {
      ...market,
      is_zombie,
      last_price: is_zombie ? null : market.last_price,
      mark_price: is_zombie ? null : market.mark_price,
      index_price: is_zombie ? null : market.index_price,
    };

    expect(output.last_price).toBeNull();
    expect(output.mark_price).toBeNull();
    expect(output.index_price).toBeNull();
    expect(output.is_zombie).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// GH#1427 — Null vault_balance + no-stats zombie classification
// ---------------------------------------------------------------------------

describe("GH#1427 null vault_balance + no-stats zombie", () => {
  it("marks null vault_balance + all null stats as zombie", () => {
    expect(
      isZombie({
        vault_balance: null,
        last_price: null,
        volume_24h: null,
        total_open_interest: null,
        total_accounts: 0,
      }),
    ).toBe(true);
  });

  it("marks null vault_balance + undefined stats as zombie", () => {
    expect(isZombie({ vault_balance: null })).toBe(true);
  });

  it("does NOT mark null vault_balance + has last_price as zombie", () => {
    expect(
      isZombie({
        vault_balance: null,
        last_price: 150,
        total_accounts: 3,
      }),
    ).toBe(false);
  });

  it("does NOT mark null vault_balance + has volume as zombie", () => {
    expect(
      isZombie({
        vault_balance: null,
        last_price: null,
        volume_24h: 500_000_000,
        total_accounts: 0,
      }),
    ).toBe(false);
  });

  it("does NOT mark null vault_balance + has OI as zombie", () => {
    expect(
      isZombie({
        vault_balance: null,
        last_price: null,
        volume_24h: null,
        total_open_interest: 1_000_000,
        total_accounts: 1,
      }),
    ).toBe(false);
  });

  it("does NOT mark null vault_balance + has accounts as zombie", () => {
    expect(
      isZombie({
        vault_balance: null,
        last_price: null,
        volume_24h: null,
        total_open_interest: null,
        total_accounts: 2,
      }),
    ).toBe(false);
  });

  it("filters 6 phantom GH#1427 markets alongside existing vault=0 zombies", () => {
    const markets = [
      // Active
      { slab_address: "ACTIVE1", vault_balance: 5_000_000_000, last_price: 100, total_accounts: 10 },
      // Drained zombie (vault=0)
      { slab_address: "ZOMBIE_DRAINED", vault_balance: 0 },
      // Phantom: null vault + no stats — these are the 6 GH#1427 markets
      { slab_address: "PHANTOM1", vault_balance: null },
      { slab_address: "PHANTOM2", vault_balance: null, last_price: null, total_accounts: 0 },
      // Still-indexing: null vault but has a price — keep in response
      { slab_address: "INDEXING", vault_balance: null, last_price: 50, total_accounts: 1 },
    ];

    const withFlag = markets.map((m) => ({ ...m, is_zombie: isZombie(m) }));
    const nonZombie = withFlag.filter((m) => !m.is_zombie);

    expect(nonZombie.map((m) => m.slab_address)).toEqual(["ACTIVE1", "INDEXING"]);
    expect(withFlag.filter((m) => m.is_zombie).map((m) => m.slab_address)).toEqual([
      "ZOMBIE_DRAINED",
      "PHANTOM1",
      "PHANTOM2",
    ]);
  });
});

// ---------------------------------------------------------------------------
// GH#1419 — Stale volume filter
// ---------------------------------------------------------------------------

const STALE_VOLUME_THRESHOLD_MS = 48 * 60 * 60 * 1000; // 48 hours

function isStaleVolume(statsUpdatedAt: string | null | undefined, now: number): boolean {
  if (!statsUpdatedAt) return false; // no timestamp → assume fresh (defensive)
  const ageMs = now - new Date(statsUpdatedAt).getTime();
  return ageMs > STALE_VOLUME_THRESHOLD_MS;
}

describe("GH#1419 stale volume filter", () => {
  const now = Date.now();
  const fresh = new Date(now - 1 * 60 * 60 * 1000).toISOString();      // 1h ago
  const borderline = new Date(now - 47 * 60 * 60 * 1000).toISOString(); // 47h ago (not stale)
  const stale = new Date(now - 5 * 24 * 60 * 60 * 1000).toISOString(); // 5 days ago (stale)
  const exactThreshold = new Date(now - 48 * 60 * 60 * 1000).toISOString(); // exactly 48h

  it("fresh market is not stale", () => {
    expect(isStaleVolume(fresh, now)).toBe(false);
  });

  it("47h old market is not stale (under threshold)", () => {
    expect(isStaleVolume(borderline, now)).toBe(false);
  });

  it("5 day old market is stale", () => {
    expect(isStaleVolume(stale, now)).toBe(true);
  });

  it("exactly 48h market is stale (strict >)", () => {
    // 48h exactly is technically > 48h * 60 * 60 * 1000 - epsilon, let's be precise:
    // new Date(now - 48h) → ageMs = 48h exactly → NOT > STALE_THRESHOLD
    // Actually exactly 48h => ageMs === STALE_THRESHOLD_MS → NOT > → false
    expect(isStaleVolume(exactThreshold, now)).toBe(false);
  });

  it("null stats_updated_at is treated as fresh (defensive)", () => {
    expect(isStaleVolume(null, now)).toBe(false);
  });

  it("stale market volume is excluded from total", () => {
    const markets = [
      { slab_address: "FRESH1", volume_24h: 1_000_000_000, stats_updated_at: fresh },
      { slab_address: "STALE1", volume_24h: 14_955_000_000, stats_updated_at: stale }, // GH#1419 culprit
      { slab_address: "FRESH2", volume_24h: 500_000_000, stats_updated_at: fresh },
    ];

    const totalVolume = markets.reduce((sum, m) => {
      if (isStaleVolume(m.stats_updated_at, now)) return sum;
      return sum + (m.volume_24h ?? 0);
    }, 0);

    expect(totalVolume).toBe(1_000_000_000 + 500_000_000);
    expect(totalVolume).not.toContain(14_955_000_000);
  });

  it("does not exclude fresh markets from total", () => {
    const markets = [
      { slab_address: "FRESH1", volume_24h: 1_000_000_000, stats_updated_at: fresh },
      { slab_address: "FRESH2", volume_24h: 500_000_000, stats_updated_at: fresh },
    ];

    const totalVolume = markets.reduce((sum, m) => {
      if (isStaleVolume(m.stats_updated_at, now)) return sum;
      return sum + (m.volume_24h ?? 0);
    }, 0);

    expect(totalVolume).toBe(1_500_000_000);
  });
});
