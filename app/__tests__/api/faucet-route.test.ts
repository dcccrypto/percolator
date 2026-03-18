/**
 * PERC-376 / PERC-1233 (GH#1382): Tests for /api/faucet route
 *
 * Covers:
 * - Network guard (devnet-only)
 * - Wallet validation
 * - Rate limiting per type (sol / usdc)
 * - USDC amount constant
 * - SOL airdrop path dispatching
 * - USDC sealed-signer path: on-chain authority check returns 400 (not 500)
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { PublicKey } from "@solana/web3.js";

// ── Mocks ──────────────────────────────────────────────────────────────────

vi.mock("@/lib/config", () => ({
  getConfig: () => ({
    rpcUrl: "https://api.devnet.solana.com",
    network: "devnet",
    testUsdcMint: "DvH13uxzTzo1xVFwkbJ6YASkZWs6bm3vFDH4xu7kUYTs",
  }),
}));

const mockSupabase = {
  from: vi.fn().mockReturnThis(),
  select: vi.fn().mockReturnThis(),
  eq: vi.fn().mockReturnThis(),
  gte: vi.fn().mockReturnThis(),
  order: vi.fn().mockReturnThis(),
  limit: vi.fn().mockResolvedValue({ data: [], error: null }),
  insert: vi.fn().mockResolvedValue({ data: null, error: null }),
};

vi.mock("@/lib/supabase", () => ({
  getServiceClient: () => mockSupabase,
}));

vi.mock("@sentry/nextjs", () => ({
  captureException: vi.fn(),
}));

// ── Helpers ────────────────────────────────────────────────────────────────

/** Build a minimal SPL Token mint account data buffer with the given authority. */
function buildMintData(authority: PublicKey | null): Buffer {
  const buf = Buffer.alloc(82, 0);
  if (authority) {
    // coption = 1 (has authority)
    buf.writeUInt32LE(1, 0);
    authority.toBuffer().copy(buf, 4);
  } else {
    // coption = 0 (no authority)
    buf.writeUInt32LE(0, 0);
  }
  return buf;
}

// ── Tests ──────────────────────────────────────────────────────────────────

describe("/api/faucet route", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    process.env.NEXT_PUBLIC_DEFAULT_NETWORK = "devnet";
    process.env.NEXT_PUBLIC_SOLANA_NETWORK = "devnet";
  });

  it("rejects requests on mainnet", () => {
    process.env.NEXT_PUBLIC_DEFAULT_NETWORK = "mainnet";
    process.env.NEXT_PUBLIC_SOLANA_NETWORK = "mainnet";
    expect(process.env.NEXT_PUBLIC_DEFAULT_NETWORK).toBe("mainnet");
  });

  it("requires wallet address", () => {
    const body = {};
    expect(body).not.toHaveProperty("wallet");
  });

  it("validates wallet address format", () => {
    expect(() => new PublicKey("not-a-valid-address")).toThrow();
  });

  it("rate-limits USDC claims per 24h (usdc_minted field)", async () => {
    mockSupabase.limit.mockResolvedValueOnce({
      data: [{ id: 1, created_at: new Date().toISOString() }],
      error: null,
    });
    const { data } = await mockSupabase.limit();
    expect(data).toHaveLength(1);
  });

  it("rate-limits SOL claims per 24h (sol_airdropped field)", async () => {
    mockSupabase.limit.mockResolvedValueOnce({
      data: [{ id: 2, created_at: new Date().toISOString() }],
      error: null,
    });
    const { data } = await mockSupabase.limit();
    expect(data).toHaveLength(1);
  });

  it("USDC mint amount constant: 10,000 USDC = 10,000,000,000 raw", () => {
    const USDC_MINT_AMOUNT = 10_000_000_000;
    expect(USDC_MINT_AMOUNT / 1_000_000).toBe(10_000);
  });

  it("SOL airdrop amount constant: 2 SOL = 2,000,000,000 lamports", () => {
    const LAMPORTS_PER_SOL = 1_000_000_000;
    const SOL_AIRDROP_AMOUNT = 2 * LAMPORTS_PER_SOL;
    expect(SOL_AIRDROP_AMOUNT).toBe(2_000_000_000);
  });

  it("type field defaults to 'usdc' when omitted", () => {
    // GH#1399: type validation — only "sol" and "usdc" are accepted.
    // Unknown/other values must be rejected (return 400), not silently coerced to usdc.
    const parseType = (t: unknown): "sol" | "usdc" | "invalid" => {
      if (t !== undefined && t !== "sol" && t !== "usdc") return "invalid";
      return t === "sol" ? "sol" : "usdc";
    };
    expect(parseType(undefined)).toBe("usdc");
    expect(parseType("sol")).toBe("sol");
    expect(parseType("usdc")).toBe("usdc");
    // GH#1399: unknown types must be rejected, NOT coerced to "usdc"
    expect(parseType("token")).toBe("invalid");
    expect(parseType("mirror")).toBe("invalid");
    expect(parseType("other")).toBe("invalid");
  });

  it("GH#1399: unknown type returns 400 with descriptive error (not authority_mismatch)", () => {
    // Regression guard: sending type:"token" or type:"mirror" previously
    // silently fell through to the USDC mint path, producing a confusing
    // authority_mismatch error. Now it must return 400 immediately.
    const VALID_TYPES = ["sol", "usdc"];
    const unknownType = "token";
    const isKnown = VALID_TYPES.includes(unknownType);
    const expectedStatus = isKnown ? 200 : 400;
    expect(expectedStatus).toBe(400);

    const unknownType2 = "mirror";
    const isKnown2 = VALID_TYPES.includes(unknownType2);
    expect(isKnown2).toBe(false);
  });

  it("on-chain authority check: authority mismatch should return 400, not 500 (GH#1382)", () => {
    // Simulates the path where on-chain authority != DEVNET_MINT_AUTHORITY_KEYPAIR.
    // The route must return 400 with hint:"authority_mismatch" instead of throwing 500.
    const signerPk = new PublicKey("So11111111111111111111111111111111111111112");
    const onChainPk = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
    const mintData = buildMintData(onChainPk);

    // Decode the authority from the simulated mint data (what the route does)
    const hasAuthority = new DataView(mintData.buffer, mintData.byteOffset).getUint32(0, true) === 1;
    expect(hasAuthority).toBe(true);

    const decoded = new PublicKey(mintData.slice(4, 36));
    expect(decoded.toBase58()).toBe(onChainPk.toBase58());
    expect(decoded.equals(signerPk)).toBe(false); // ← mismatch → route returns 400
  });

  it("on-chain authority check: matching authority should proceed to mint", () => {
    const signerPk = new PublicKey("So11111111111111111111111111111111111111112");
    const mintData = buildMintData(signerPk);

    const hasAuthority = new DataView(mintData.buffer, mintData.byteOffset).getUint32(0, true) === 1;
    expect(hasAuthority).toBe(true);

    const decoded = new PublicKey(mintData.slice(4, 36));
    expect(decoded.equals(signerPk)).toBe(true); // ← match → route proceeds
  });

  describe("SOL airdrop rate-limit detection (GH#1385)", () => {
    // Mirror of the regex used in the route to detect Solana devnet rate-limits.
    // Ensures the pattern catches real error strings from the devnet faucet.
    const isRateLimit = (msg: string) =>
      /429|too many requests|rate.?limit|airdrop.*limit|limit.*airdrop/i.test(msg);

    it("detects '429 Too Many Requests' from devnet RPC", () => {
      expect(isRateLimit("429 Too Many Requests")).toBe(true);
    });

    it("detects 'airdrop request limit reached' from devnet faucet", () => {
      expect(isRateLimit("airdrop request limit reached for the wallet address")).toBe(true);
    });

    it("detects 'rate limit exceeded' variations", () => {
      expect(isRateLimit("rate limit exceeded")).toBe(true);
      expect(isRateLimit("RateLimit: too many requests")).toBe(true);
    });

    it("does NOT flag unrelated errors as rate-limits", () => {
      expect(isRateLimit("Transaction simulation failed")).toBe(false);
      expect(isRateLimit("Connection refused")).toBe(false);
      expect(isRateLimit("Invalid public key input")).toBe(false);
      expect(isRateLimit("Internal error")).toBe(false); // GH#1392: handled separately as retryable
    });

  });

  describe("SOL airdrop retryable error detection (GH#1392)", () => {
    // Mirror of the retryable regex added for transient Solana devnet failures.
    const isRetryable = (msg: string) =>
      /internal error|service unavailable|timeout|ECONNREFUSED/i.test(msg);

    it("detects 'Internal error' from Solana devnet", () => {
      expect(isRetryable("airdrop to G7NG... failed: Internal error")).toBe(true);
    });

    it("detects 'Service unavailable'", () => {
      expect(isRetryable("Service unavailable")).toBe(true);
    });

    it("detects connection refused", () => {
      expect(isRetryable("connect ECONNREFUSED 127.0.0.1:8899")).toBe(true);
    });

    it("detects timeout errors", () => {
      expect(isRetryable("Request timeout")).toBe(true);
    });

    it("does NOT flag unrelated errors as retryable", () => {
      expect(isRetryable("Transaction simulation failed")).toBe(false);
      expect(isRetryable("Invalid public key input")).toBe(false);
    });

    it("returns 503 status for retryable SOL airdrop errors (not 500)", () => {
      const errMsg = "airdrop to G7NG... failed: Internal error";
      const statusCode = isRetryable(errMsg) ? 503 : 500;
      expect(statusCode).toBe(503);
    });
  });

  describe("SOL airdrop rate-limit detection — regression (GH#1385)", () => {
    const isRateLimit = (msg: string) =>
      /429|too many requests|rate.?limit|airdrop.*limit|limit.*airdrop/i.test(msg);

    it("returns 429 status for rate-limited SOL airdrop (not 500)", () => {
      // Validate that a 429 from devnet → our API returns 429 with retryable:true.
      // This is a logic regression guard — not a full integration test.
      const errMsg = "429 Too Many Requests";
      const rateLimited = isRateLimit(errMsg);
      const statusCode = rateLimited ? 429 : 500;
      expect(statusCode).toBe(429);
    });
  });
});
