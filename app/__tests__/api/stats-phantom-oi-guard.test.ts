/**
 * GH#1300: /api/stats phantom OI vault boundary regression tests.
 *
 * PR#1299 fixed GH#1297 but used an exclusive boundary (vault_balance < 1_000_000)
 * that let markets at vault=1_000_000 (creation-deposit only, no real LP) contribute
 * phantom OI to the platform total. This PR tightens it to inclusive (<= 1_000_000).
 *
 * Coverage:
 * - vault=0 → excluded (empty, no positions)
 * - vault=1_000_000 → excluded (creation deposit only, boundary case)
 * - vault=1_000_001 → included (real LP above threshold)
 * - accounts=0 → excluded regardless of vault
 */

import { describe, it, expect } from "vitest";

/** Mirrors the vault boundary constant and guard in app/app/api/stats/route.ts */
const MIN_VAULT_FOR_OI_STATS = 1_000_000;

function isPhantomMarket(vaultBal: number, accountsCount: number): boolean {
  // GH#1300: inclusive boundary — vault <= 1_000_000 suppresses phantom OI
  return accountsCount === 0 || vaultBal <= MIN_VAULT_FOR_OI_STATS;
}

function simulateOISum(
  markets: Array<{ vault_balance: number; total_accounts: number; total_open_interest: number }>
): number {
  return markets.reduce((sum, m) => {
    if (isPhantomMarket(m.vault_balance, m.total_accounts)) return sum;
    return sum + m.total_open_interest;
  }, 0);
}

describe("GH#1300: /api/stats phantom OI inclusive boundary", () => {
  it("excludes markets with vault_balance=0 (empty vault)", () => {
    const markets = [{ vault_balance: 0, total_accounts: 5, total_open_interest: 50_000 }];
    expect(simulateOISum(markets)).toBe(0);
  });

  it("excludes markets with vault_balance=1_000_000 (creation-deposit only, boundary case — GH#1300)", () => {
    // This was the regression in PR#1299: strict < 1M let vault=1M slip through
    const markets = [{ vault_balance: 1_000_000, total_accounts: 3, total_open_interest: 42_909 }];
    expect(simulateOISum(markets)).toBe(0);
  });

  it("includes markets with vault_balance=1_000_001 (real LP above threshold)", () => {
    const markets = [{ vault_balance: 1_000_001, total_accounts: 5, total_open_interest: 64_614 }];
    expect(simulateOISum(markets)).toBe(64_614);
  });

  it("excludes markets with accounts_count=0 regardless of vault", () => {
    const markets = [{ vault_balance: 999_999_999, total_accounts: 0, total_open_interest: 100_000 }];
    expect(simulateOISum(markets)).toBe(0);
  });

  it("correctly filters mixed set — only non-phantom markets contribute OI", () => {
    const markets = [
      // Phantom: vault=0
      { vault_balance: 0, total_accounts: 2, total_open_interest: 10_000 },
      // Phantom: vault=1M (boundary)
      { vault_balance: 1_000_000, total_accounts: 5, total_open_interest: 42_909 },
      // Phantom: no accounts
      { vault_balance: 5_000_000, total_accounts: 0, total_open_interest: 20_000 },
      // Real: vault > 1M with accounts
      { vault_balance: 5_000_000, total_accounts: 3, total_open_interest: 64_614 },
      { vault_balance: 2_000_000, total_accounts: 1, total_open_interest: 43_000 },
    ];
    // Only last two contribute
    expect(simulateOISum(markets)).toBe(64_614 + 43_000);
  });

  it("reproduces GH#1300 scenario: totalOI=$107,523 → $64,614 after guard", () => {
    // Before fix: vault=1M markets added $42,909 phantom OI → $107,523 total
    // After fix: vault=1M excluded → $64,614 correct total
    const markets = [
      { vault_balance: 1_000_000, total_accounts: 5, total_open_interest: 42_909 }, // phantom
      { vault_balance: 5_000_000, total_accounts: 10, total_open_interest: 64_614 }, // real
    ];
    expect(simulateOISum(markets)).toBe(64_614);
  });
});
