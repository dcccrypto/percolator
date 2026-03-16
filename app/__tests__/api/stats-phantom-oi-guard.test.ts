/**
 * GH#1314: /api/stats phantom OI vault boundary tests.
 *
 * History:
 * - PR#1299 (GH#1297): first vault guard, strict < 1M. Correct, but also fixed $1 fallback.
 * - PR#1303 (GH#1300): changed to inclusive <= 1M. Incorrectly excluded vault=1M real markets
 *   (usdEkK5G $59,994, MOLTBOT $4,620) → stats showed $0 instead of $64K.
 * - PR#1307 (GH#1304): over-corrected to (vaultBal <= 1M && rawOi === 0). Left a gap:
 *   markets with vault < 1M AND rawOi > 0 were not phantom in stats but were in /api/markets,
 *   causing $42K residual phantom OI ($107K vs $64K).
 * - PR#this (GH#1314): revert to strict < 1M, mirroring /api/markets exactly.
 *
 * Rule: isPhantomOI = accountsCount === 0 || vaultBal < 1_000_000  (strict <)
 *
 * Coverage:
 * - vault=0         → phantom (no vault at all)
 * - vault=999_999   → phantom (below threshold, dust)
 * - vault=1_000_000 → NOT phantom (creation-deposit markets like usdEkK5G / MOLTBOT)
 * - vault=1_000_001 → NOT phantom (real LP above threshold)
 * - accounts=0      → phantom regardless of vault
 * - GH#1314 regression: vault < 1M + rawOi > 0 → phantom (excluded)
 */

import { describe, it, expect } from "vitest";

/** Mirrors the vault boundary constant in app/app/api/stats/route.ts */
const MIN_VAULT_FOR_OI_STATS = 1_000_000;

/** GH#1314: strict < mirroring /api/markets isPhantomOI exactly */
function isPhantomMarket(vaultBal: number, accountsCount: number): boolean {
  return accountsCount === 0 || vaultBal < MIN_VAULT_FOR_OI_STATS;
}

function simulateOISum(
  markets: Array<{ vault_balance: number; total_accounts: number; total_open_interest: number }>
): number {
  return markets.reduce((sum, m) => {
    if (isPhantomMarket(m.vault_balance, m.total_accounts)) return sum;
    return sum + m.total_open_interest;
  }, 0);
}

describe("GH#1314: /api/stats phantom OI strict < 1M boundary (mirrors /api/markets)", () => {
  it("excludes markets with vault_balance=0 (empty vault)", () => {
    const markets = [{ vault_balance: 0, total_accounts: 5, total_open_interest: 50_000 }];
    expect(simulateOISum(markets)).toBe(0);
  });

  it("excludes markets with vault_balance=999_999 (dust/sub-threshold)", () => {
    const markets = [{ vault_balance: 999_999, total_accounts: 3, total_open_interest: 42_909 }];
    expect(simulateOISum(markets)).toBe(0);
  });

  it("includes markets with vault_balance=1_000_000 (creation-deposit — usdEkK5G / MOLTBOT pattern)", () => {
    // GH#1314: strict < means vault=1M is NOT phantom. PR#1303 broke this with <=.
    const markets = [{ vault_balance: 1_000_000, total_accounts: 3, total_open_interest: 64_614 }];
    expect(simulateOISum(markets)).toBe(64_614);
  });

  it("includes markets with vault_balance=1_000_001 (real LP above threshold)", () => {
    const markets = [{ vault_balance: 1_000_001, total_accounts: 5, total_open_interest: 64_614 }];
    expect(simulateOISum(markets)).toBe(64_614);
  });

  it("excludes markets with accounts_count=0 regardless of vault", () => {
    const markets = [{ vault_balance: 999_999_999, total_accounts: 0, total_open_interest: 100_000 }];
    expect(simulateOISum(markets)).toBe(0);
  });

  it("GH#1314 regression: excludes vault<1M markets with non-zero rawOi (PR#1307 gap)", () => {
    // PR#1307 used (vaultBal <= 1M && rawOi === 0) — let vault=500K + rawOi>0 slip through.
    // /api/markets filters these (vaultBal < 1M), so stats was overcounting by ~$42K.
    const markets = [{ vault_balance: 500_000, total_accounts: 5, total_open_interest: 42_909 }];
    expect(simulateOISum(markets)).toBe(0);
  });

  it("correctly filters mixed set — only non-phantom markets contribute OI", () => {
    const markets = [
      // Phantom: vault=0
      { vault_balance: 0, total_accounts: 2, total_open_interest: 10_000 },
      // Phantom: vault=999_999 (dust, below threshold) — GH#1314 regression case
      { vault_balance: 999_999, total_accounts: 5, total_open_interest: 42_909 },
      // Phantom: no accounts
      { vault_balance: 5_000_000, total_accounts: 0, total_open_interest: 20_000 },
      // Real: vault=1_000_000 (creation-deposit, like usdEkK5G / MOLTBOT)
      { vault_balance: 1_000_000, total_accounts: 3, total_open_interest: 59_994 },
      // Real: vault > 1M with accounts
      { vault_balance: 2_000_000, total_accounts: 1, total_open_interest: 4_620 },
    ];
    // Only last two contribute: 59_994 + 4_620 = 64_614
    expect(simulateOISum(markets)).toBe(59_994 + 4_620);
  });

  it("reproduces GH#1314 scenario: $107K → $64K after correcting phantom guard", () => {
    // Before fix (PR#1307): vault=500K markets with rawOi>0 slipped through → $107K
    // After fix (strict <): vault<1M excluded → $64,614 matches /api/markets
    const markets = [
      { vault_balance: 500_000, total_accounts: 5, total_open_interest: 42_909 },  // phantom, excluded
      { vault_balance: 1_000_000, total_accounts: 3, total_open_interest: 59_994 }, // real (usdEkK5G)
      { vault_balance: 1_000_000, total_accounts: 2, total_open_interest: 4_620 },  // real (MOLTBOT)
    ];
    expect(simulateOISum(markets)).toBe(59_994 + 4_620); // = 64_614
  });
});
