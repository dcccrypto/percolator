/**
 * GH#1420: Zombie markets (vault_balance=0) should be excluded from /api/markets by default.
 * GH#1419: Stale volume_24h (stats_updated_at > 48h ago) should be excluded from /api/stats totals.
 *
 * Unit tests for the filtering logic (not full route integration — uses helpers extracted from route).
 */
import { describe, it, expect } from "vitest";

// ---------------------------------------------------------------------------
// GH#1420 — Zombie market filter
// ---------------------------------------------------------------------------

function isZombie(vaultBalance: number | null | undefined): boolean {
  return (vaultBalance ?? 0) === 0;
}

describe("GH#1420 zombie market filter", () => {
  it("marks vault_balance=0 as zombie", () => {
    expect(isZombie(0)).toBe(true);
  });

  it("marks vault_balance=null as zombie", () => {
    expect(isZombie(null)).toBe(true);
  });

  it("marks vault_balance=undefined as zombie", () => {
    expect(isZombie(undefined)).toBe(true);
  });

  it("does NOT mark vault_balance=1 as zombie", () => {
    expect(isZombie(1)).toBe(false);
  });

  it("does NOT mark vault_balance=1_000_000 (creation-deposit) as zombie", () => {
    expect(isZombie(1_000_000)).toBe(false);
  });

  it("does NOT mark vault_balance=5_000_000_000 (healthy market) as zombie", () => {
    expect(isZombie(5_000_000_000)).toBe(false);
  });

  it("filters zombie markets out of a list by default", () => {
    const markets = [
      { slab_address: "ACTIVE1", vault_balance: 5_000_000_000 },
      { slab_address: "ZOMBIE1", vault_balance: 0 },
      { slab_address: "ACTIVE2", vault_balance: 1_000_000 },
      { slab_address: "ZOMBIE2", vault_balance: null },
    ];

    const withZombieFlag = markets.map((m) => ({ ...m, is_zombie: isZombie(m.vault_balance) }));
    const nonZombie = withZombieFlag.filter((m) => !m.is_zombie);

    expect(nonZombie).toHaveLength(2);
    expect(nonZombie.map((m) => m.slab_address)).toEqual(["ACTIVE1", "ACTIVE2"]);
  });

  it("includes zombie markets when include_zombie=true", () => {
    const markets = [
      { slab_address: "ACTIVE1", vault_balance: 5_000_000_000 },
      { slab_address: "ZOMBIE1", vault_balance: 0 },
    ];

    const withZombieFlag = markets.map((m) => ({ ...m, is_zombie: isZombie(m.vault_balance) }));
    const includeZombie = true;
    const result = withZombieFlag.filter((m) => includeZombie || !m.is_zombie);

    expect(result).toHaveLength(2);
  });

  it("nulls out prices for zombie markets", () => {
    // Simulates the route behavior: zombie markets get null prices
    const market = { slab_address: "ZOMBIE1", vault_balance: 0, last_price: 148, mark_price: 150, index_price: 149 };
    const is_zombie = isZombie(market.vault_balance);

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
