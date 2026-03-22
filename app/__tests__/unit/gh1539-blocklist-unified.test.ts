/**
 * GH#1539: Verify that BLOCKED_SLAB_ADDRESSES includes both hardcoded entries
 * and env var overrides, so UI and API use the same blocklist.
 */
import { describe, it, expect, vi, afterEach } from "vitest";

afterEach(() => {
  vi.resetModules();
  delete process.env.NEXT_PUBLIC_BLOCKED_MARKET_ADDRESSES;
});

describe("GH#1539: unified blocklist", () => {
  it("includes hardcoded addresses", async () => {
    const { BLOCKED_SLAB_ADDRESSES } = await import("@/lib/blocklist");
    // Spot-check a few known hardcoded entries
    expect(BLOCKED_SLAB_ADDRESSES.has("BxJPaMaCfEGTBsjZ8wfj3Yfzf4wpasmxKAEvqZZRcGPP")).toBe(true);
    expect(BLOCKED_SLAB_ADDRESSES.has("HjBePQZnoZVftg9B52gyeuHGjBvt2f8FNCVP4FeoP3YT")).toBe(true);
    expect(BLOCKED_SLAB_ADDRESSES.has("8eFFEFBY3HHbBgzxJJP5hyxdzMNMAumnYNhkWXErBM4c")).toBe(true);
  });

  it("includes NEXT_PUBLIC_BLOCKED_MARKET_ADDRESSES env var entries", async () => {
    // Set env var BEFORE importing the module so the Set is built with it.
    process.env.NEXT_PUBLIC_BLOCKED_MARKET_ADDRESSES = "TestAddr111,TestAddr222";
    vi.resetModules();
    const { BLOCKED_SLAB_ADDRESSES } = await import("@/lib/blocklist");
    expect(BLOCKED_SLAB_ADDRESSES.has("TestAddr111")).toBe(true);
    expect(BLOCKED_SLAB_ADDRESSES.has("TestAddr222")).toBe(true);
  });

  it("isBlockedSlab returns true for blocked addresses", async () => {
    const { isBlockedSlab } = await import("@/lib/blocklist");
    expect(isBlockedSlab("BxJPaMaCfEGTBsjZ8wfj3Yfzf4wpasmxKAEvqZZRcGPP")).toBe(true);
    expect(isBlockedSlab("SomeRandomNonBlockedAddress")).toBe(false);
    expect(isBlockedSlab(null)).toBe(false);
    expect(isBlockedSlab(undefined)).toBe(false);
  });
});
