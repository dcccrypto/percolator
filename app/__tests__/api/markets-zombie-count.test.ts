/**
 * GH#1429: zombieCount must be non-zero even when include_zombie=true.
 *
 * Regression test for the bug where zombieCount was computed as
 * `sanitized.length - nonZombie.length`. When include_zombie=true, nonZombie
 * includes all markets, making the difference always 0.
 *
 * Fix: compute zombieCount directly from the is_zombie flag on sanitized
 * markets, independent of the include_zombie filter.
 */
import { describe, it, expect } from "vitest";

// ---------------------------------------------------------------------------
// Minimal reproduction of the zombieCount computation in /api/markets/route.ts
// ---------------------------------------------------------------------------

type Market = { slab_address: string; is_zombie: boolean };

/**
 * Buggy implementation (before fix):
 * zombieCount = sanitized.length - nonZombie.length
 * When includeZombie=true, nonZombie === sanitized, so count is always 0.
 */
function zombieCountBuggy(sanitized: Market[], includeZombie: boolean): number {
  const nonZombie = sanitized.filter((m) => includeZombie || !m.is_zombie);
  return sanitized.length - nonZombie.length;
}

/**
 * Fixed implementation (after fix):
 * zombieCount = sanitized.filter(is_zombie).length
 * Independent of the includeZombie filter flag.
 */
function zombieCountFixed(sanitized: Market[], _includeZombie: boolean): number {
  return sanitized.filter((m) => m.is_zombie === true).length;
}

const ALIVE: Market = { slab_address: "alive1", is_zombie: false };
const ZOMBIE: Market = { slab_address: "zombie1", is_zombie: true };
const ZOMBIE2: Market = { slab_address: "zombie2", is_zombie: true };

describe("GH#1429 — zombieCount bug regression", () => {
  describe("buggy implementation (documents the pre-fix behaviour)", () => {
    it("returns 0 when include_zombie=false (works by accident)", () => {
      // include_zombie=false: nonZombie excludes zombie, so diff is correct
      expect(zombieCountBuggy([ALIVE, ZOMBIE], false)).toBe(1);
    });

    it("returns 0 (wrong!) when include_zombie=true", () => {
      // BUG: nonZombie === sanitized when includeZombie=true → diff = 0
      expect(zombieCountBuggy([ALIVE, ZOMBIE], true)).toBe(0);
    });
  });

  describe("fixed implementation", () => {
    it("returns correct count when include_zombie=false", () => {
      expect(zombieCountFixed([ALIVE, ZOMBIE], false)).toBe(1);
    });

    it("returns correct count when include_zombie=true", () => {
      // FIX: counts from is_zombie flag, ignoring the include flag
      expect(zombieCountFixed([ALIVE, ZOMBIE], true)).toBe(1);
    });

    it("returns 0 when there are no zombies (include_zombie=true)", () => {
      expect(zombieCountFixed([ALIVE], true)).toBe(0);
    });

    it("counts multiple zombies correctly with include_zombie=true", () => {
      expect(zombieCountFixed([ALIVE, ZOMBIE, ZOMBIE2], true)).toBe(2);
    });

    it("counts multiple zombies correctly with include_zombie=false", () => {
      expect(zombieCountFixed([ALIVE, ZOMBIE, ZOMBIE2], false)).toBe(2);
    });

    it("returns sanitized.length when all are zombies", () => {
      expect(zombieCountFixed([ZOMBIE, ZOMBIE2], true)).toBe(2);
    });

    it("returns 0 when sanitized is empty", () => {
      expect(zombieCountFixed([], true)).toBe(0);
      expect(zombieCountFixed([], false)).toBe(0);
    });
  });
});

// ---------------------------------------------------------------------------
// Response shape assertion: zombieCount present and consistent
// ---------------------------------------------------------------------------
describe("GH#1429 — response shape with fixed zombieCount", () => {
  function buildResponse(
    sanitized: Market[],
    includeZombie: boolean,
  ): { total: number; zombieCount: number; markets: Market[] } {
    const nonZombie = sanitized.filter((m) => includeZombie || !m.is_zombie);
    // FIXED: compute from tag, not from difference
    const zombieCount = sanitized.filter((m) => m.is_zombie === true).length;
    return { total: nonZombie.length, zombieCount, markets: nonZombie };
  }

  it("include_zombie=true: total=2, zombieCount=1, markets has both", () => {
    const res = buildResponse([ALIVE, ZOMBIE], true);
    expect(res.total).toBe(2);
    expect(res.zombieCount).toBe(1);
    expect(res.markets).toHaveLength(2);
  });

  it("include_zombie=false: total=1, zombieCount=1, markets has only alive", () => {
    const res = buildResponse([ALIVE, ZOMBIE], false);
    expect(res.total).toBe(1);
    expect(res.zombieCount).toBe(1);
    expect(res.markets).toHaveLength(1);
    expect(res.markets[0].slab_address).toBe("alive1");
  });

  it("no zombies: zombieCount=0 regardless of flag", () => {
    const resTrue = buildResponse([ALIVE], true);
    const resFalse = buildResponse([ALIVE], false);
    expect(resTrue.zombieCount).toBe(0);
    expect(resFalse.zombieCount).toBe(0);
  });
});
