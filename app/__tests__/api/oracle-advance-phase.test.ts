/**
 * Tests for POST /api/oracle/advance-phase (GH#1120 fix)
 *
 * Server-side AdvanceOraclePhase crank — signs with CRANK_KEYPAIR,
 * no user wallet interaction.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { NextRequest } from "next/server";

// --- Hoisted mock references (must use vi.hoisted to be accessible in vi.mock factories) ---

const { mockSendAndConfirm, mockFromSecretKey } = vi.hoisted(() => ({
  mockSendAndConfirm: vi.fn(),
  mockFromSecretKey: vi.fn(),
}));

// --- Mocks ---

vi.mock("@/lib/config", () => ({
  getConfig: () => ({ rpcUrl: "https://api.devnet.solana.com" }),
}));

vi.mock("@sentry/nextjs", () => ({
  captureException: vi.fn(),
}));

vi.mock("@solana/web3.js", () => {
  const fakePublicKey = {
    toBase58: () => "11111111111111111111111111111111",
    toString: () => "11111111111111111111111111111111",
  };
  return {
    // Must use regular function (not arrow) for constructors called with `new`
    Connection: vi.fn().mockImplementation(function () { return {}; }),
    Keypair: {
      fromSecretKey: mockFromSecretKey.mockReturnValue({ publicKey: fakePublicKey }),
    },
    PublicKey: vi.fn().mockImplementation(function (addr: string) {
      return { toBase58: () => addr, toString: () => addr };
    }),
    Transaction: vi.fn().mockImplementation(function (this: Record<string, unknown>) {
      this.feePayer = null;
      this.add = function () { return this; };
      return this;
    }),
    ComputeBudgetProgram: {
      setComputeUnitLimit: vi.fn().mockReturnValue({ type: "cu_limit" }),
    },
    sendAndConfirmTransaction: mockSendAndConfirm,
  };
});

vi.mock("@percolator/sdk", () => ({
  encodeAdvanceOraclePhase: vi.fn().mockReturnValue(Buffer.from([56])),
  buildIx: vi.fn().mockReturnValue({ type: "ix" }),
  buildAccountMetas: vi.fn().mockReturnValue([]),
  ACCOUNTS_ADVANCE_ORACLE_PHASE: [],
}));

// Import route ONCE (cached for all tests)
import { POST } from "@/app/api/oracle/advance-phase/route";

// --- Helpers ---

const VALID_SLAB = "7G3SsnevWwUWjWAwGGmr2N11x8KAGn1abzjV3bBbZkAM";
const FAKE_KEYPAIR_JSON = JSON.stringify(Array.from({ length: 64 }, (_, i) => i));
const FAKE_PUBKEY = {
  toBase58: () => "11111111111111111111111111111111",
  toString: () => "11111111111111111111111111111111",
};

function makeRequest(body: unknown): NextRequest {
  return new NextRequest("http://localhost/api/oracle/advance-phase", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  mockSendAndConfirm.mockResolvedValue("sig_default_ok");
  mockFromSecretKey.mockReturnValue({ publicKey: FAKE_PUBKEY });

  process.env.NEXT_PUBLIC_SOLANA_NETWORK = "devnet";
  process.env.CRANK_KEYPAIR = FAKE_KEYPAIR_JSON;
  process.env.NEXT_PUBLIC_PROGRAM_ID = "FxfD37s1NC7CDPMPzqgSfLsiJxjYRjfQDsV1CRuW9dBH";
});

afterEach(() => {
  process.env.NEXT_PUBLIC_SOLANA_NETWORK = "devnet";
  process.env.CRANK_KEYPAIR = FAKE_KEYPAIR_JSON;
  delete process.env.DEVNET_MINT_AUTHORITY_KEYPAIR;
});

// --- Tests ---

describe("POST /api/oracle/advance-phase", () => {
  it("returns 400 for missing slabAddress", async () => {
    const res = await POST(makeRequest({}));
    expect(res.status).toBe(400);
    const json = await res.json();
    expect(json.error).toMatch(/slabAddress/);
  });

  it("returns 400 for invalid slabAddress (non-base58 chars)", async () => {
    const res = await POST(makeRequest({ slabAddress: "not-valid!!!" }));
    expect(res.status).toBe(400);
  });

  it("returns 400 for invalid slabAddress (too short)", async () => {
    const res = await POST(makeRequest({ slabAddress: "abc" }));
    expect(res.status).toBe(400);
  });

  it("returns 400 for malformed JSON body", async () => {
    const req = new NextRequest("http://localhost/api/oracle/advance-phase", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: "{ bad json {{",
    });
    const res = await POST(req);
    expect(res.status).toBe(400);
  });

  it("returns skipped:true on mainnet", async () => {
    process.env.NEXT_PUBLIC_SOLANA_NETWORK = "mainnet";
    const res = await POST(makeRequest({ slabAddress: VALID_SLAB }));
    const json = await res.json();
    expect(json.skipped).toBe(true);
    expect(json.reason).toBe("not devnet");
  });

  it("returns skipped:true when no keypair env vars are set", async () => {
    delete process.env.CRANK_KEYPAIR;
    delete process.env.DEVNET_MINT_AUTHORITY_KEYPAIR;
    const res = await POST(makeRequest({ slabAddress: VALID_SLAB }));
    const json = await res.json();
    expect(json.skipped).toBe(true);
    expect(json.reason).toMatch(/no crank keypair/);
  });

  it("calls sendAndConfirmTransaction and returns success:true with signature", async () => {
    mockSendAndConfirm.mockResolvedValue("sig_advance_123");
    const res = await POST(makeRequest({ slabAddress: VALID_SLAB }));
    const json = await res.json();
    expect(mockSendAndConfirm).toHaveBeenCalledTimes(1);
    expect(json.success).toBe(true);
    expect(json.signature).toBe("sig_advance_123");
  });

  it("returns skipped (non-error) when program returns expected on-chain error", async () => {
    mockSendAndConfirm.mockRejectedValue(
      new Error("Transaction simulation failed: custom program error: 0x64"),
    );
    const res = await POST(makeRequest({ slabAddress: VALID_SLAB }));
    const json = await res.json();
    expect(json.success).toBe(false);
    expect(json.skipped).toBe(true);
  });

  it("falls back to DEVNET_MINT_AUTHORITY_KEYPAIR when CRANK_KEYPAIR not set", async () => {
    delete process.env.CRANK_KEYPAIR;
    process.env.DEVNET_MINT_AUTHORITY_KEYPAIR = FAKE_KEYPAIR_JSON;
    mockSendAndConfirm.mockResolvedValue("sig_fallback_ok");
    const res = await POST(makeRequest({ slabAddress: VALID_SLAB }));
    const json = await res.json();
    expect(json.success).toBe(true);
    expect(mockFromSecretKey).toHaveBeenCalledTimes(1);
  });
});
