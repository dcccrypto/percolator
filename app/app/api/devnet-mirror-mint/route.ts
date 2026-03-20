/**
 * PERC-456: Devnet Mirror Mint API
 *
 * POST /api/devnet-mirror-mint
 * Body: { mainnetCA: string, walletAddress: string }
 *
 * Given a mainnet token CA, returns an existing or newly-created devnet SPL
 * mint that mirrors the mainnet token's metadata (name, symbol, decimals).
 *
 * Flow:
 * 1. Validate walletAddress is present (required, returns 400 if missing)
 * 2. Check `devnet_mints` table for existing mapping → return immediately
 * 3. Validate mainnetCA exists on mainnet (DexScreener / Jupiter)
 * 4. Create a new devnet SPL mint with DEVNET_MINT_AUTHORITY as authority
 * 5. Store mapping in `devnet_mints` table (with creator_wallet)
 * 6. Return { devnetMint, name, symbol, decimals }
 *
 * Rate limited by middleware.ts (120 req/min/IP).
 * Only callable on devnet.
 *
 * Requires: DEVNET_MINT_AUTHORITY_KEYPAIR env var (JSON secret key bytes)
 */

import { NextRequest, NextResponse } from "next/server";
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  VersionedTransaction,
  SystemProgram,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  MINT_SIZE,
  TOKEN_PROGRAM_ID,
  getMinimumBalanceForRentExemptMint,
  createInitializeMintInstruction,
} from "@solana/spl-token";
import { getConfig } from "@/lib/config";
import { getServiceClient } from "@/lib/supabase";
import { getDevnetMintSigner } from "@/lib/devnet-signer";
import * as Sentry from "@sentry/nextjs";

export const dynamic = "force-dynamic";

// Per-IP rate limiter for this endpoint specifically.
// Tighter than the global middleware limit (120/min) because each request
// creates an on-chain devnet SPL mint that costs real SOL from the shared
// DEVNET_MINT_AUTHORITY_KEYPAIR. 10 req/min/IP is generous for legitimate use
// while preventing a single attacker from draining the mint authority wallet.
const MINT_RATE_LIMIT_MAX = 10;
const MINT_RATE_LIMIT_WINDOW_MS = 60_000;
const mintRateLimitMap = new Map<string, { count: number; resetAt: number }>();

function checkMintRateLimit(ip: string): { allowed: boolean; retryAfter: number } {
  const now = Date.now();
  let entry = mintRateLimitMap.get(ip);

  if (!entry || now > entry.resetAt) {
    entry = { count: 0, resetAt: now + MINT_RATE_LIMIT_WINDOW_MS };
    mintRateLimitMap.set(ip, entry);
  }

  entry.count++;

  // Occasional GC to prevent unbounded Map growth
  if (Math.random() < 0.01) {
    for (const [key, val] of mintRateLimitMap) {
      if (now > val.resetAt) mintRateLimitMap.delete(key);
    }
  }

  if (entry.count > MINT_RATE_LIMIT_MAX) {
    return { allowed: false, retryAfter: Math.ceil((entry.resetAt - now) / 1000) };
  }
  return { allowed: true, retryAfter: 0 };
}

/** Extract client IP from request headers, respecting proxy depth env var. */
function getClientIp(req: NextRequest): string {
  const PROXY_DEPTH = Math.max(0, Number(process.env.TRUSTED_PROXY_DEPTH ?? 1));
  if (PROXY_DEPTH > 0) {
    const forwarded = req.headers.get("x-forwarded-for");
    if (forwarded) {
      const ips = forwarded.split(",").map((s) => s.trim());
      const idx = Math.max(0, ips.length - PROXY_DEPTH);
      return ips[idx] ?? "unknown";
    }
  }
  return "unknown";
}

const NETWORK = process.env.NEXT_PUBLIC_SOLANA_NETWORK?.trim() ?? "mainnet";

interface TokenInfo {
  name: string;
  symbol: string;
  decimals: number;
  logoUrl?: string;
}

/** Fetch token metadata from DexScreener (mainnet). */
async function fetchMainnetTokenInfo(ca: string): Promise<TokenInfo | null> {
  try {
    const resp = await fetch(
      `https://api.dexscreener.com/latest/dex/tokens/${ca}`,
      { signal: AbortSignal.timeout(8000) },
    );
    if (!resp.ok) return null;
    const json = await resp.json();
    const pairs = json.pairs;
    if (!pairs || pairs.length === 0) return null;

    // Sort by liquidity, pick best
    const sorted = [...pairs].sort(
      (a: any, b: any) => (b.liquidity?.usd ?? 0) - (a.liquidity?.usd ?? 0),
    );
    const best = sorted[0] as any;

    return {
      name: best.baseToken?.name ?? `Token ${ca.slice(0, 6)}`,
      symbol: best.baseToken?.symbol ?? ca.slice(0, 4).toUpperCase(),
      decimals: 6, // Default to 6 for devnet mirror (simplifies math)
      logoUrl: best.info?.imageUrl,
    };
  } catch {
    return null;
  }
}

/** Fallback: fetch metadata from Jupiter token list. */
async function fetchJupiterTokenInfo(ca: string): Promise<TokenInfo | null> {
  try {
    const resp = await fetch(
      `https://token.jup.ag/strict`,
      { signal: AbortSignal.timeout(8000) },
    );
    if (!resp.ok) return null;
    const tokens = await resp.json();
    const token = tokens.find((t: any) => t.address === ca);
    if (!token) return null;
    return {
      name: token.name,
      symbol: token.symbol,
      decimals: Math.min(token.decimals, 9), // Cap at 9 for devnet sanity
      logoUrl: token.logoURI,
    };
  } catch {
    return null;
  }
}

export async function POST(req: NextRequest) {
  try {
    if (NETWORK !== "devnet") {
      return NextResponse.json({ error: "Only available on devnet" }, { status: 403 });
    }

    // Per-endpoint rate limit: 10 req/min/IP (tighter than global 120/min).
    // Prevents SOL drain on the shared DEVNET_MINT_AUTHORITY_KEYPAIR.
    const clientIp = getClientIp(req);
    const { allowed, retryAfter } = checkMintRateLimit(clientIp);
    if (!allowed) {
      return NextResponse.json(
        { error: "Too many mint requests. Please wait before retrying." },
        {
          status: 429,
          headers: {
            "Retry-After": String(retryAfter),
            "X-RateLimit-Limit": String(MINT_RATE_LIMIT_MAX),
            "X-RateLimit-Window": "60",
          },
        },
      );
    }

    const body = await req.json();
    const { mainnetCA, walletAddress } = body as { mainnetCA?: string; walletAddress?: string };

    if (!mainnetCA) {
      return NextResponse.json({ error: "Missing mainnetCA" }, { status: 400 });
    }

    // GH#1477: validate walletAddress BEFORE cache-hit early return so the
    // check applies to both new and existing mirror paths.
    if (!walletAddress) {
      return NextResponse.json({ error: "Missing walletAddress" }, { status: 400 });
    }

    // Validate walletAddress is a valid Solana pubkey
    try {
      new PublicKey(walletAddress);
    } catch {
      return NextResponse.json({ error: "Invalid walletAddress" }, { status: 400 });
    }

    // Reject URLs
    if (mainnetCA.startsWith("http") || mainnetCA.includes("://")) {
      return NextResponse.json(
        { error: "Paste a valid Solana token address, not a URL" },
        { status: 400 },
      );
    }

    // Validate base58
    try {
      new PublicKey(mainnetCA);
    } catch {
      return NextResponse.json({ error: "Invalid mainnetCA" }, { status: 400 });
    }

    // 1. Check for existing mapping
    const supabase = getServiceClient();
    const { data: existing } = await (supabase as any)
      .from("devnet_mints")
      .select("devnet_mint, name, symbol, decimals, logo_url")
      .eq("mainnet_ca", mainnetCA)
      .maybeSingle();

    if (existing?.devnet_mint) {
      return NextResponse.json({
        status: "existing",
        devnetMint: existing.devnet_mint,
        name: existing.name,
        symbol: existing.symbol,
        decimals: existing.decimals ?? 6,
        logoUrl: existing.logo_url,
      });
    }

    // 2. Fetch metadata from mainnet
    // GH#1476: surface individual step errors so Sentry captures the real cause.
    let tokenInfo = await fetchMainnetTokenInfo(mainnetCA);
    if (!tokenInfo) {
      tokenInfo = await fetchJupiterTokenInfo(mainnetCA);
    }
    if (!tokenInfo) {
      return NextResponse.json(
        {
          error:
            "Cannot fetch token info from mainnet. Token may not exist or have no DEX liquidity. " +
            "Ensure the address is a valid mainnet Solana token.",
        },
        { status: 400 },
      );
    }

    // 3. Create devnet mint
    const mintSigner = getDevnetMintSigner();
    if (!mintSigner) {
      return NextResponse.json(
        { error: "Server not configured for minting (DEVNET_MINT_AUTHORITY_KEYPAIR missing)" },
        { status: 500 },
      );
    }
    const mintAuthPk = new PublicKey(mintSigner.publicKey());

    const cfg = getConfig();
    const connection = new Connection(cfg.rpcUrl, "confirmed");

    const mintKeypair = Keypair.generate();

    // GH#1476: wrap individual RPC calls so errors propagate with context.
    let lamports: number;
    try {
      lamports = await getMinimumBalanceForRentExemptMint(connection);
    } catch (e) {
      Sentry.captureException(e, {
        tags: { endpoint: "/api/devnet-mirror-mint", step: "getMinimumBalance" },
        extra: { mainnetCA, walletAddress },
      });
      throw e;
    }

    let tx: Transaction | VersionedTransaction = new Transaction();

    // Set recentBlockhash and feePayer before signing
    let blockhash: string;
    try {
      ({ blockhash } = await connection.getLatestBlockhash());
    } catch (e) {
      Sentry.captureException(e, {
        tags: { endpoint: "/api/devnet-mirror-mint", step: "getLatestBlockhash" },
        extra: { mainnetCA, walletAddress },
      });
      throw e;
    }

    (tx as Transaction).recentBlockhash = blockhash;
    (tx as Transaction).feePayer = new PublicKey(mintSigner.publicKey());

    tx.add(
      SystemProgram.createAccount({
        fromPubkey: mintAuthPk,
        newAccountPubkey: mintKeypair.publicKey,
        lamports,
        space: MINT_SIZE,
        programId: TOKEN_PROGRAM_ID,
      }),
    );
    tx.add(
      createInitializeMintInstruction(
        mintKeypair.publicKey,
        tokenInfo.decimals,
        mintAuthPk, // mint authority
        mintAuthPk, // freeze authority
      ),
    );

    // Multi-signer: partialSign mintKeypair FIRST so its signature is in the array,
    // then let the sealed signer add mintAuthority's sig via signTransaction.
    // SystemProgram.createAccount requires the new account keypair to sign —
    // without this, every mirror mint fails with signature verification failure.
    // sendAndConfirmTransaction cannot be used here because it would re-sign and
    // overwrite the sealed signer's signature, so we use sendRawTransaction instead.
    (tx as Transaction).partialSign(mintKeypair);

    try {
      tx = mintSigner.signTransaction(tx);
    } catch (e) {
      Sentry.captureException(e, {
        tags: { endpoint: "/api/devnet-mirror-mint", step: "signTransaction" },
        extra: { mainnetCA, walletAddress },
      });
      throw e;
    }

    let sig: string;
    try {
      sig = await connection.sendRawTransaction(tx.serialize());
      await connection.confirmTransaction(sig, "confirmed");
    } catch (e) {
      Sentry.captureException(e, {
        tags: { endpoint: "/api/devnet-mirror-mint", step: "sendAndConfirm" },
        extra: { mainnetCA, walletAddress, mintPubkey: mintKeypair.publicKey.toBase58() },
      });
      throw e;
    }

    const devnetMint = mintKeypair.publicKey.toBase58();

    // 4. Store mapping — upsert with ignoreDuplicates to handle concurrent requests
    // (TOCTOU: two simultaneous requests can both pass the SELECT check above;
    //  upsert ON CONFLICT (mainnet_ca) DO NOTHING prevents duplicate rows and
    //  avoids a second createMint call winning a race that corrupts the table.)
    // GH#1476: include creator_wallet so the column constraint is satisfied.
    const { error: upsertError } = await (supabase as any).from("devnet_mints").upsert(
      {
        mainnet_ca: mainnetCA,
        devnet_mint: devnetMint,
        symbol: tokenInfo.symbol,
        name: tokenInfo.name,
        decimals: tokenInfo.decimals,
        logo_url: tokenInfo.logoUrl ?? null,
        creator_wallet: walletAddress,
      },
      { onConflict: "mainnet_ca", ignoreDuplicates: true },
    );

    if (upsertError) {
      Sentry.captureException(upsertError, {
        tags: { endpoint: "/api/devnet-mirror-mint", step: "upsert" },
        extra: { mainnetCA, walletAddress, devnetMint },
      });
      // Non-fatal: mint was created on-chain. Log and continue — re-SELECT will
      // return the canonical row even if this upsert lost a race.
    }

    // Re-SELECT the canonical row from DB to handle TOCTOU races (#772):
    // If two concurrent requests both created mints, only one wins the upsert.
    // Return the DB-canonical devnetMint so all callers get the same address.
    const { data: canonical } = await (supabase as any)
      .from("devnet_mints")
      .select("devnet_mint")
      .eq("mainnet_ca", mainnetCA)
      .single();

    const canonicalMint = canonical?.devnet_mint ?? devnetMint;

    return NextResponse.json({
      status: canonicalMint === devnetMint ? "created" : "existing",
      devnetMint: canonicalMint,
      name: tokenInfo.name,
      symbol: tokenInfo.symbol,
      decimals: tokenInfo.decimals,
      logoUrl: tokenInfo.logoUrl,
      signature: sig,
    });
  } catch (error) {
    Sentry.captureException(error, {
      tags: { endpoint: "/api/devnet-mirror-mint", method: "POST" },
    });
    // Return a generic error to avoid leaking internal details (stack traces,
    // DB schema, RPC URLs, etc.) to clients. Full error is captured by Sentry.
    return NextResponse.json(
      { error: "Failed to create devnet mirror mint. Please try again." },
      { status: 500 },
    );
  }
}
