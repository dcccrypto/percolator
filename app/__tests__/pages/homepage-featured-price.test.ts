/**
 * GH#1405: Homepage featured markets must sanitize last_price before display.
 *
 * DfLoAzny (and similar admin-mode markets) have a raw DB last_price like
 * 10001100011 ($10B) — an unscaled authorityPriceE6 divided by 1e6 is still
 * huge if the initial oracle was set in micro-units on-chain. The featured
 * markets card must clamp to MAX_SANE_PRICE_USD ($10K) — null when corrupt.
 *
 * This test validates the sanitization logic extracted from the converted map
 * in app/page.tsx (GH#1405 fix).
 */

import { describe, it, expect } from "vitest";

/**
 * Mirror of the sanitize logic in page.tsx `converted` map (GH#1405):
 *   last_price: (m.last_price != null && m.last_price > 0 && m.last_price <= MAX_SANE_PRICE_USD) ? m.last_price : null
 */
const MAX_SANE_PRICE_USD = 10_000; // must stay in sync with page.tsx

function sanitizeDisplayPrice(raw: number | null | undefined): number | null {
  if (raw == null) return null;
  if (raw > 0 && raw <= MAX_SANE_PRICE_USD) return raw;
  return null;
}

describe("homepage featured markets — last_price sanitization (GH#1405)", () => {
  it("passes through a normal price unchanged", () => {
    expect(sanitizeDisplayPrice(1.23)).toBe(1.23);
    expect(sanitizeDisplayPrice(9999.99)).toBe(9999.99);
    expect(sanitizeDisplayPrice(0.000001)).toBe(0.000001);
  });

  it("nulls a $10B DB price (DfLoAzny bug)", () => {
    // last_price = 10001100011 as returned by markets_with_stats view
    expect(sanitizeDisplayPrice(10001100011)).toBeNull();
  });

  it("nulls a price exactly at MAX_SANE_PRICE_USD boundary (exclusive)", () => {
    expect(sanitizeDisplayPrice(10_001)).toBeNull();
  });

  it("passes a price exactly at MAX_SANE_PRICE_USD", () => {
    expect(sanitizeDisplayPrice(10_000)).toBe(10_000);
  });

  it("nulls zero and negative prices", () => {
    expect(sanitizeDisplayPrice(0)).toBeNull();
    expect(sanitizeDisplayPrice(-1)).toBeNull();
  });

  it("nulls null/undefined", () => {
    expect(sanitizeDisplayPrice(null)).toBeNull();
    expect(sanitizeDisplayPrice(undefined)).toBeNull();
  });

  it("nulls other absurdly large values", () => {
    // $100M, $1T — all admin oracle corruption patterns
    expect(sanitizeDisplayPrice(100_000_000)).toBeNull();
    expect(sanitizeDisplayPrice(1_000_000_000_000)).toBeNull();
  });
});
