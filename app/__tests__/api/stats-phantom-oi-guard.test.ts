/**
 * GH#1314 / GH#1318: /api/stats phantom OI vault boundary + price fallback tests.
 *
 * History:
 * - PR#1299 (GH#1297): first vault guard, strict < 1M. Correct, but also fixed $1 fallback.
 * - PR#1303 (GH#1300): changed to inclusive <= 1M. Incorrectly excluded vault=1M real markets
 *   (usdEkK5G $59,994, MOLTBOT $4,620) → stats showed $0 instead of $64K.
 * - PR#1307 (GH#1304): over-corrected to (vaultBal <= 1M && rawOi === 0). Left a gap:
 *   markets with vault < 1M AND rawOi > 0 were not phantom in stats but were in /api/markets,
 *   causing $42K residual phantom OI ($107K vs $64K).
 * - PR#1315 (GH#1314): revert to strict < 1M, mirroring /api/markets exactly. Still had
 *   $42K phantom OI because 33 vault=1M uncranked markets had stale non-zero OI and no
 *   oracle price — the $1 fallback gave them each ~$2K USD OI.
 * - PR#this (GH#1318): remove $1 fallback — markets without a valid oracle price have
 *   indeterminate USD value and must not contribute to totalOpenInterest.
 *
 * Rules:
 *   isPhantomOI = accountsCount === 0 || vaultBal < 1_000_000  (strict <, unchanged)
 *   price = last_price if valid, else 0 (no $1 fallback) → skip if p <= 0
 *
 * Coverage:
 * - vault=0         → phantom (no vault at all)
 * - vault=999_999   → phantom (below threshold, dust)
 * - vault=1_000_000 → NOT phantom (creation-deposit markets like usdEkK5G / MOLTBOT)
 * - vault=1_000_001 → NOT phantom (real LP above threshold)
 * - accounts=0      → phantom regardless of vault
 * - GH#1314 regression: vault < 1M + rawOi > 0 → phantom (excluded)
 * - GH#1318 regression: vault=1M + rawOi > 0 + NO PRICE → NOT phantom by vault, but
 *   skipped by price guard (p=0 → return sum). No $1 fallback.
 */

import { describe, it, expect } from "vitest";

/** Mirrors the vault boundary constant in app/app/api/stats/route.ts */
const MIN_VAULT_FOR_OI_STATS = 1_000_000;
const MAX_SANE_PRICE_USD = 10_000;
const MAX_PER_MARKET_USD = 10_000_000_000;

/** GH#1314: strict < mirroring /api/markets isPhantomOI exactly */
function isPhantomMarket(vaultBal: number, accountsCount: number): boolean {
  return accountsCount === 0 || vaultBal < MIN_VAULT_FOR_OI_STATS;
}

/** GH#1318: simulates the full OI reducer including price guard (no $1 fallback) */
function simulateOISum(
  markets: Array<{
    vault_balance: number;
    total_accounts: number;
    total_open_interest: number;
    last_price?: number | null;
    decimals?: number | null;
  }>
): number {
  return markets.reduce((sum, m) => {
    if (isPhantomMarket(m.vault_balance, m.total_accounts)) return sum;
    const rawOi = m.total_open_interest;
    if (rawOi <= 0 || !Number.isFinite(rawOi) || rawOi >= 1e18) return sum;
    const d = Math.min(Math.max(m.decimals ?? 6, 0), 18);
    // GH#1318: no $1 fallback — skip markets without a valid oracle price
    const p = (m.last_price != null && m.last_price > 0 && m.last_price <= MAX_SANE_PRICE_USD)
      ? m.last_price
      : 0;
    if (p <= 0) return sum;
    const usd = (rawOi / 10 ** d) * p;
    return sum + (usd > MAX_PER_MARKET_USD ? 0 : usd);
  }, 0);
}

describe("GH#1314: /api/stats phantom OI strict < 1M boundary (mirrors /api/markets)", () => {
  it("excludes markets with vault_balance=0 (empty vault)", () => {
    const markets = [{ vault_balance: 0, total_accounts: 5, total_open_interest: 50_000, last_price: 1.0 }];
    expect(simulateOISum(markets)).toBe(0);
  });

  it("excludes markets with vault_balance=999_999 (dust/sub-threshold)", () => {
    const markets = [{ vault_balance: 999_999, total_accounts: 3, total_open_interest: 42_909, last_price: 1.0 }];
    expect(simulateOISum(markets)).toBe(0);
  });

  it("includes markets with vault_balance=1_000_000 and valid price (creation-deposit — usdEkK5G / MOLTBOT pattern)", () => {
    // GH#1314: strict < means vault=1M is NOT phantom. PR#1303 broke this with <=.
    // GH#1318: must have a valid last_price — no $1 fallback.
    const markets = [{ vault_balance: 1_000_000, total_accounts: 3, total_open_interest: 59_994_000_000, last_price: 1.0, decimals: 6 }];
    expect(simulateOISum(markets)).toBeCloseTo(59_994, 0);
  });

  it("includes markets with vault_balance=1_000_001 (real LP above threshold)", () => {
    const markets = [{ vault_balance: 1_000_001, total_accounts: 5, total_open_interest: 4_620_000_000, last_price: 1.0, decimals: 6 }];
    expect(simulateOISum(markets)).toBeCloseTo(4_620, 0);
  });

  it("excludes markets with accounts_count=0 regardless of vault", () => {
    const markets = [{ vault_balance: 999_999_999, total_accounts: 0, total_open_interest: 100_000, last_price: 1.0 }];
    expect(simulateOISum(markets)).toBe(0);
  });

  it("GH#1314 regression: excludes vault<1M markets with non-zero rawOi (PR#1307 gap)", () => {
    // PR#1307 used (vaultBal <= 1M && rawOi === 0) — let vault=500K + rawOi>0 slip through.
    // /api/markets filters these (vaultBal < 1M), so stats was overcounting by ~$42K.
    const markets = [{ vault_balance: 500_000, total_accounts: 5, total_open_interest: 42_909, last_price: 1.0 }];
    expect(simulateOISum(markets)).toBe(0);
  });

  it("correctly filters mixed set — only non-phantom markets with valid prices contribute OI", () => {
    const markets = [
      // Phantom: vault=0
      { vault_balance: 0, total_accounts: 2, total_open_interest: 10_000, last_price: 1.0, decimals: 6 },
      // Phantom: vault=999_999 (dust, below threshold) — GH#1314 regression case
      { vault_balance: 999_999, total_accounts: 5, total_open_interest: 42_909, last_price: 1.0, decimals: 6 },
      // Phantom: no accounts
      { vault_balance: 5_000_000, total_accounts: 0, total_open_interest: 20_000, last_price: 1.0, decimals: 6 },
      // Real: vault=1_000_000 (creation-deposit, like usdEkK5G) — has valid price
      { vault_balance: 1_000_000, total_accounts: 3, total_open_interest: 59_994_000_000, last_price: 1.0, decimals: 6 },
      // Real: vault > 1M with accounts and valid price
      { vault_balance: 2_000_000, total_accounts: 1, total_open_interest: 4_620_000_000, last_price: 1.0, decimals: 6 },
    ];
    // Only last two contribute: 59_994 + 4_620 = 64_614
    expect(simulateOISum(markets)).toBeCloseTo(59_994 + 4_620, 0);
  });

  it("reproduces GH#1314 scenario: $107K → $64K after correcting phantom guard", () => {
    // Before fix (PR#1307): vault=500K markets with rawOi>0 slipped through → $107K
    // After fix (strict <): vault<1M excluded → $64,614 matches /api/markets
    const markets = [
      { vault_balance: 500_000, total_accounts: 5, total_open_interest: 42_909_000_000, last_price: 1.0, decimals: 6 },  // phantom, excluded
      { vault_balance: 1_000_000, total_accounts: 3, total_open_interest: 59_994_000_000, last_price: 1.0, decimals: 6 }, // real (usdEkK5G)
      { vault_balance: 1_000_000, total_accounts: 2, total_open_interest: 4_620_000_000, last_price: 1.0, decimals: 6 },  // real (MOLTBOT)
    ];
    expect(simulateOISum(markets)).toBeCloseTo(59_994 + 4_620, 0); // = 64_614
  });
});

describe("GH#1318: /api/stats no $1 fallback — markets without oracle price skipped", () => {
  it("excludes vault=1M markets with stale OI and no price (was $2K each via $1 fallback)", () => {
    // These are uncranked creation-deposit markets: vault=1M, accounts>0, OI>0 in DB,
    // but no oracle price (indexer no longer processing them). With $1 fallback they
    // contributed ~$2K each (33 markets = ~$47K phantom OI). Fix: skip if no price.
    const markets = [
      { vault_balance: 1_000_000, total_accounts: 3, total_open_interest: 2_000_000_000_000, last_price: null, decimals: 9 },
      { vault_balance: 1_000_000, total_accounts: 2, total_open_interest: 2_660_054_000_000, last_price: null, decimals: 9 },
    ];
    expect(simulateOISum(markets)).toBe(0); // No price → no contribution
  });

  it("still counts vault=1M markets WITH a valid oracle price (usdEkK5G / MOLTBOT)", () => {
    const markets = [
      // usdEkK5G: vault=1M, accounts=2, has real price
      { vault_balance: 1_000_000, total_accounts: 2, total_open_interest: 59_994_000_000, last_price: 1.0, decimals: 6 },
      // MOLTBOT: vault=1M, accounts=2, has real price
      { vault_balance: 1_000_000, total_accounts: 2, total_open_interest: 4_620_000_000, last_price: 1.0, decimals: 6 },
    ];
    expect(simulateOISum(markets)).toBeCloseTo(59_994 + 4_620, 0);
  });

  it("GH#1318 full scenario: 33 phantom no-price markets + 2 real priced markets → $64K", () => {
    // Before fix: 33 × ~$2K ($1 fallback) + $64,614 = ~$107K
    // After fix: no contribution from no-price markets → $64,614 only
    const phantomMarkets = Array.from({ length: 33 }, () => ({
      vault_balance: 1_000_000, total_accounts: 2, total_open_interest: 2_000_000_000_000,
      last_price: null, decimals: 9,
    }));
    const realMarkets = [
      { vault_balance: 1_000_000, total_accounts: 2, total_open_interest: 59_994_000_000, last_price: 1.0, decimals: 6 },
      { vault_balance: 1_000_000, total_accounts: 2, total_open_interest: 4_620_000_000, last_price: 1.0, decimals: 6 },
    ];
    const result = simulateOISum([...phantomMarkets, ...realMarkets]);
    expect(result).toBeCloseTo(59_994 + 4_620, 0); // ≈ $64,614 (no phantom OI)
  });

  it("excludes markets with corrupt/garbage price (> MAX_SANE_PRICE_USD)", () => {
    // Admin-mode markets with garbage authorityPriceE6 written as raw u64
    const markets = [
      { vault_balance: 2_000_000, total_accounts: 5, total_open_interest: 5_000_000_000, last_price: 100_000, decimals: 6 }, // > $10K cap → p=0
    ];
    expect(simulateOISum(markets)).toBe(0);
  });

  it("includes markets with price at exactly MAX_SANE_PRICE_USD boundary ($10K)", () => {
    const markets = [
      { vault_balance: 2_000_000, total_accounts: 3, total_open_interest: 1_000_000, last_price: 10_000, decimals: 6 },
    ];
    // 1_000_000 / 1e6 * 10_000 = 1 * 10_000 = $10,000
    expect(simulateOISum(markets)).toBeCloseTo(10_000, 0);
  });
});
