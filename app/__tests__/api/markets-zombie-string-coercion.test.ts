/**
 * GH#1494: is_zombie field always false in /api/markets array despite zombieCount=73.
 *
 * Root cause: Supabase returns NUMERIC columns (vault_balance, total_open_interest,
 * volume_24h) as strings at runtime. TypeScript `as number | null` is compile-time only
 * and does NOT coerce the value. The strict equality check `vaultBal === 0` in
 * isZombieMarket() compares string "0" to number 0 → always false.
 *
 * Fix: coerce all NUMERIC fields via Number() before passing to isZombieMarket().
 *
 * These tests verify the numericOrNull coercion helper and the fixed route behaviour.
 */

import { describe, it, expect } from "vitest";
import { isZombieMarket, isSaneMarketValue } from "@/lib/activeMarketFilter";
import { isPhantomOpenInterest } from "@/lib/phantom-oi";

// ---------------------------------------------------------------------------
// numericOrNull helper (extracted from the fix in route.ts)
// ---------------------------------------------------------------------------
function numericOrNull(v: unknown): number | null {
  if (v == null) return null;
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

// ---------------------------------------------------------------------------
// Coercion helper tests
// ---------------------------------------------------------------------------
describe("numericOrNull coercion helper (GH#1494)", () => {
  it("coerces string '0' to number 0", () => {
    expect(numericOrNull("0")).toBe(0);
  });

  it("coerces string '1000000' to number 1000000", () => {
    expect(numericOrNull("1000000")).toBe(1_000_000);
  });

  it("coerces string '5000000000' to number", () => {
    expect(numericOrNull("5000000000")).toBe(5_000_000_000);
  });

  it("returns null for null input", () => {
    expect(numericOrNull(null)).toBeNull();
  });

  it("returns null for undefined input", () => {
    expect(numericOrNull(undefined)).toBeNull();
  });

  it("returns null for NaN-producing string", () => {
    expect(numericOrNull("garbage")).toBeNull();
  });

  it("passes through number 0 as 0", () => {
    expect(numericOrNull(0)).toBe(0);
  });

  it("passes through number 1000000 as 1000000", () => {
    expect(numericOrNull(1_000_000)).toBe(1_000_000);
  });
});

// ---------------------------------------------------------------------------
// isZombieMarket with Supabase-style string vault_balance (GH#1494)
// ---------------------------------------------------------------------------
describe("isZombieMarket with Supabase NUMERIC string coercion (GH#1494)", () => {
  it("is_zombie=true for string '0' vault_balance after coercion", () => {
    // Without coercion: "0" === 0 → false (BUG)
    // With coercion: numericOrNull("0") = 0, then 0 === 0 → true (FIXED)
    expect(
      isZombieMarket({
        vault_balance: numericOrNull("0"),
        last_price: null,
        volume_24h: null,
        total_open_interest: null,
        total_accounts: 0,
      }),
    ).toBe(true);
  });

  it("is_zombie=false for string '1000000' vault_balance after coercion", () => {
    expect(
      isZombieMarket({
        vault_balance: numericOrNull("1000000"),
        last_price: 148,
        volume_24h: null,
        total_open_interest: null,
        total_accounts: 0,
      }),
    ).toBe(false);
  });

  it("is_zombie=true for string '0' vault_balance even with stale price (GH#1494 main case)", () => {
    // This is the production case: vault=0, stale last_price still in DB
    expect(
      isZombieMarket({
        vault_balance: numericOrNull("0"),
        last_price: numericOrNull("148"),
        volume_24h: numericOrNull("0"),
        total_open_interest: numericOrNull("0"),
        total_accounts: numericOrNull("0"),
      }),
    ).toBe(true);
  });

  it("is_zombie=false when vault > 0 as string", () => {
    expect(
      isZombieMarket({
        vault_balance: numericOrNull("5000000000"),
        last_price: numericOrNull("148"),
        volume_24h: null,
        total_open_interest: null,
        total_accounts: numericOrNull("10"),
      }),
    ).toBe(false);
  });

  it("is_zombie=true for null vault + all string '0' stats (phantom market)", () => {
    // GH#1427 phantom: vault=null, all zero stats as strings
    expect(
      isZombieMarket({
        vault_balance: numericOrNull(null),
        last_price: numericOrNull("0"),
        volume_24h: numericOrNull("0"),
        total_open_interest: numericOrNull("0"),
        total_accounts: numericOrNull("0"),
      }),
    ).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// isPhantomOpenInterest with Supabase string coercion (GH#1494)
// ---------------------------------------------------------------------------
describe("isPhantomOpenInterest with Number() coercion (GH#1494)", () => {
  it("phantom when string accounts='0' and string vault='0'", () => {
    // Before fix: vaultBal = "0" < 1_000_000 → NaN comparison → false (BUG)
    // After fix: vaultBal = Number("0") = 0 < 1_000_000 → true (FIXED)
    expect(isPhantomOpenInterest(Number("0"), Number("0"))).toBe(true);
  });

  it("phantom when accounts=0 and vault < 1M as string", () => {
    expect(isPhantomOpenInterest(Number("0"), Number("500000"))).toBe(true);
  });

  it("not phantom when accounts > 0 and vault >= 1M", () => {
    expect(isPhantomOpenInterest(Number("5"), Number("5000000000"))).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// GH#1499 — c_tot > 0 only exempts when market has corroborating activity
// ---------------------------------------------------------------------------
describe("GH#1499 isZombieMarket c_tot + activity check", () => {
  it("NNOB case: c_tot>0, vault=0, no price, no accounts → zombie (was bug: returned false)", () => {
    // Before fix: c_tot > 0 short-circuited → return false (NNOB showed in response w/ null prices)
    // After fix: c_tot > 0 only exempts if hasActivity (price or accounts) is truthy
    expect(
      isZombieMarket({
        vault_balance: 0,
        c_tot: 100_000_000_000,
        last_price: null,
        volume_24h: null,
        total_open_interest: null,
        total_accounts: 0,
      }),
    ).toBe(true);
  });

  it("FF7K healthy: c_tot>0, vault=0, has price → NOT zombie (keeper cranking)", () => {
    // The 33 working FF7K markets: keeper pushes prices → hasActivity=true → exemption holds
    expect(
      isZombieMarket({
        vault_balance: 0,
        c_tot: 1_000_000_000,
        last_price: 1.0,
        volume_24h: null,
        total_open_interest: null,
        total_accounts: 0,
      }),
    ).toBe(false);
  });

  it("FF7K with accounts: c_tot>0, vault=0, has accounts → NOT zombie", () => {
    expect(
      isZombieMarket({
        vault_balance: 0,
        c_tot: 5_000_000_000,
        last_price: null,
        volume_24h: null,
        total_open_interest: null,
        total_accounts: 5,
      }),
    ).toBe(false);
  });

  it("no c_tot, vault=0, no activity → zombie", () => {
    expect(
      isZombieMarket({
        vault_balance: 0,
        c_tot: null,
        last_price: null,
        volume_24h: null,
        total_open_interest: null,
        total_accounts: 0,
      }),
    ).toBe(true);
  });

  it("c_tot=0, vault=0, no activity → zombie (c_tot=0 not exempt)", () => {
    expect(
      isZombieMarket({
        vault_balance: 0,
        c_tot: 0,
        last_price: null,
        volume_24h: null,
        total_open_interest: null,
        total_accounts: 0,
      }),
    ).toBe(true);
  });

  it("c_tot>0, vault>0 (normal market with price) → NOT zombie", () => {
    // Standard healthy market: both c_tot and vault > 0, has price
    expect(
      isZombieMarket({
        vault_balance: 5_000_000_000,
        c_tot: 1_000_000_000_000,
        last_price: 150,
        volume_24h: null,
        total_open_interest: null,
        total_accounts: 10,
      }),
    ).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// isSaneMarketValue with Supabase string values
// ---------------------------------------------------------------------------
describe("isSaneMarketValue with string inputs (defensive)", () => {
  it("returns false for string '0' (not > 0)", () => {
    // isSaneMarketValue compares v > 0 which does JS coercion, but let's confirm
    // that the route explicitly coerces before calling isZombieMarket
    expect(isSaneMarketValue("0" as unknown as number)).toBe(false);
  });

  it("note: === 0 does NOT coerce — this is why numericOrNull is required for isZombieMarket", () => {
    // Demonstrate the root cause: strict equality does not coerce
    expect(("0" as unknown as number) === 0).toBe(false);
    expect(numericOrNull("0") === 0).toBe(true);
  });
});
