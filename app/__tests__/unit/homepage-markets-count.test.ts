/**
 * GH#1448: Homepage markets count must use shared isPhantomOpenInterest()
 * (strict < on vault_balance) instead of local <= check.
 *
 * The bug: homepage used `vaultBal <= 1_000_000` to detect phantom markets,
 * while /api/stats used isPhantomOpenInterest() with `vaultBal < 1_000_000`.
 * Since most devnet markets have vault_balance == 1_000_000 (creation deposit),
 * the homepage zeroed their stats → only 2 of 69 survived isActiveMarket().
 */

import { describe, it, expect } from "vitest";
import { isPhantomOpenInterest, MIN_VAULT_FOR_OI } from "@/lib/phantom-oi";
import { isActiveMarket } from "@/lib/activeMarketFilter";

describe("GH#1448: Homepage phantom OI alignment with /api/stats", () => {
  it("vault_balance == MIN_VAULT_FOR_OI (1_000_000) is NOT phantom", () => {
    // This is the standard creation deposit — must NOT be treated as phantom
    expect(isPhantomOpenInterest(1, MIN_VAULT_FOR_OI)).toBe(false);
  });

  it("vault_balance < MIN_VAULT_FOR_OI IS phantom", () => {
    expect(isPhantomOpenInterest(1, MIN_VAULT_FOR_OI - 1)).toBe(true);
    expect(isPhantomOpenInterest(1, 0)).toBe(true);
  });

  it("market with vault=1M, last_price>0 passes isActiveMarket (not zeroed)", () => {
    // Simulates what happens when isPhantomOpenInterest returns false:
    // the market's stats are preserved, and isActiveMarket sees real values
    const market = {
      last_price: 1111,
      volume_24h: 0,
      total_open_interest: 54000000,
    };
    const isPhantom = isPhantomOpenInterest(1, 1_000_000);
    expect(isPhantom).toBe(false);
    expect(isActiveMarket(market)).toBe(true);
  });

  it("market with vault=1M would fail if using <= (the old bug)", () => {
    // Demonstrates the bug: with <=, vault=1M is phantom → stats zeroed → not active
    const oldBugIsPhantom = (accounts: number, vault: number) =>
      accounts === 0 || vault <= 1_000_000; // OLD broken logic
    
    expect(oldBugIsPhantom(1, 1_000_000)).toBe(true); // BUG: wrongly phantom
    expect(isPhantomOpenInterest(1, 1_000_000)).toBe(false); // CORRECT: not phantom
  });

  it("accounts=0 is always phantom regardless of vault", () => {
    expect(isPhantomOpenInterest(0, 10_000_000)).toBe(true);
  });
});
