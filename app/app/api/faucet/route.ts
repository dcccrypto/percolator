/**
 * PERC-376: Devnet faucet endpoint
 *
 * POST /api/faucet { wallet: string, type?: "sol" | "usdc" }
 *
 * type="sol"  → airdrops 2 SOL via requestAirdrop on devnet public RPC
 * type="usdc" → mints 10,000 test USDC (default when type omitted)
 *
 * Rate-limited: 1 claim per wallet per type per 24h (tracked in Supabase auto_fund_log).
 *
 * GH#1382 (PERC-1233): switched from raw Keypair + sendAndConfirmTransaction to
 * getDevnetMintSigner() + sendRawTransaction (sealed signer, same as auto-fund / devnet-airdrop).
 * Added on-chain mint authority check → 400 (not 500) on mismatch.
 */

import { NextRequest, NextResponse } from "next/server";
import {
  Connection,
  PublicKey,
  Transaction,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import {
  getAssociatedTokenAddress,
  createAssociatedTokenAccountInstruction,
  createMintToInstruction,
  getAccount,
} from "@solana/spl-token";
import { getConfig } from "@/lib/config";
import { getServiceClient } from "@/lib/supabase";
import { getDevnetMintSigner } from "@/lib/devnet-signer";
import * as Sentry from "@sentry/nextjs";

export const dynamic = "force-dynamic";

// Use NEXT_PUBLIC_DEFAULT_NETWORK — canonical network env var (GH#1380, aligned with auto-fund fix in PR #1379)
const NETWORK =
  process.env.NEXT_PUBLIC_DEFAULT_NETWORK?.trim() ??
  process.env.NEXT_PUBLIC_SOLANA_NETWORK;

const USDC_MINT_AMOUNT = 10_000_000_000; // 10,000 USDC (6 decimals)
const SOL_AIRDROP_AMOUNT = 2 * LAMPORTS_PER_SOL; // 2 SOL
const RATE_LIMIT_HOURS = 24;

// Public devnet RPC for requestAirdrop (private RPC may reject airdrop requests)
const PUBLIC_DEVNET_RPC = "https://api.devnet.solana.com";

export async function POST(req: NextRequest) {
  try {
    if (NETWORK !== "devnet") {
      return NextResponse.json(
        { error: "Faucet only available on devnet" },
        { status: 403 },
      );
    }

    const body = await req.json();
    const walletAddress = body?.wallet;
    const type: "sol" | "usdc" = body?.type === "sol" ? "sol" : "usdc";

    if (!walletAddress || typeof walletAddress !== "string") {
      return NextResponse.json(
        { error: "Missing wallet address" },
        { status: 400 },
      );
    }

    let walletPk: PublicKey;
    try {
      walletPk = new PublicKey(walletAddress);
    } catch {
      return NextResponse.json(
        { error: "Invalid wallet address" },
        { status: 400 },
      );
    }

    // Rate limit check via Supabase — per wallet per type
    const supabase = getServiceClient();
    const cutoff = new Date(
      Date.now() - RATE_LIMIT_HOURS * 60 * 60 * 1000,
    ).toISOString();

    const rateField = type === "sol" ? "sol_airdropped" : "usdc_minted";

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const { data: recent } = await (supabase as any)
      .from("auto_fund_log")
      .select("id, created_at")
      .eq("wallet", walletAddress)
      .eq(rateField, true)
      .gte("created_at", cutoff)
      .order("created_at", { ascending: false })
      .limit(1);

    if (recent && recent.length > 0) {
      const lastClaim = new Date(recent[0].created_at);
      const nextClaimAt = new Date(
        lastClaim.getTime() + RATE_LIMIT_HOURS * 60 * 60 * 1000,
      ).toISOString();

      return NextResponse.json(
        {
          error: "Already claimed in the last 24 hours",
          funded: false,
          nextClaimAt,
        },
        { status: 429 },
      );
    }

    // ── SOL airdrop path ──────────────────────────────────────────────────────
    if (type === "sol") {
      // Use public devnet RPC — private/Helius endpoints may not support requestAirdrop
      const pubConn = new Connection(PUBLIC_DEVNET_RPC, "confirmed");
      let sig: string;
      try {
        sig = await pubConn.requestAirdrop(walletPk, SOL_AIRDROP_AMOUNT);
        await pubConn.confirmTransaction(sig, "confirmed");
      } catch (airdropErr) {
        const msg =
          airdropErr instanceof Error ? airdropErr.message : "Airdrop failed";
        // Detect Solana devnet RPC rate-limit responses.
        // The public devnet faucet returns "429 Too Many Requests", "airdrop request limit",
        // or similar strings when the wallet or IP has exceeded the daily drip.
        const isRateLimit =
          /429|too many requests|rate.?limit|airdrop.*limit|limit.*airdrop/i.test(msg);
        if (isRateLimit) {
          // Do NOT capture rate-limit hits as Sentry exceptions — they're expected.
          return NextResponse.json(
            {
              error:
                "Solana devnet faucet rate-limited. Wait a few minutes and retry.",
              retryable: true,
            },
            { status: 429 },
          );
        }
        // GH#1392: Solana devnet returns "Internal error" for transient failures
        // (throttling, recent airdrop, RPC overload). Return 503 so clients can retry.
        const isRetryable =
          /internal error|service unavailable|timeout|ECONNREFUSED/i.test(msg);
        if (isRetryable) {
          return NextResponse.json(
            {
              error:
                "Solana devnet temporarily unavailable. Please retry in a few minutes.",
              retryable: true,
            },
            { status: 503 },
          );
        }
        Sentry.captureException(airdropErr, {
          tags: { endpoint: "/api/faucet", type: "sol" },
          extra: { walletAddress },
        });
        return NextResponse.json({ error: msg }, { status: 500 });
      }

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      await (supabase as any).from("auto_fund_log").insert({
        wallet: walletAddress,
        sol_airdropped: true,
        usdc_minted: false,
      });

      return NextResponse.json({
        funded: true,
        sol_airdropped: true,
        sol_amount: SOL_AIRDROP_AMOUNT / LAMPORTS_PER_SOL,
        signature: sig,
        nextClaimAt: new Date(
          Date.now() + RATE_LIMIT_HOURS * 60 * 60 * 1000,
        ).toISOString(),
      });
    }

    // ── USDC mint path ────────────────────────────────────────────────────────

    // Load configuration
    const cfg = getConfig();
    const usdcMintAddr = (cfg as Record<string, unknown>).testUsdcMint as
      | string
      | undefined;

    if (!usdcMintAddr) {
      return NextResponse.json(
        { error: "Test USDC mint not configured" },
        { status: 500 },
      );
    }

    const usdcMint = new PublicKey(usdcMintAddr);

    // Load sealed mint authority signer (GH#1382: replaces raw Keypair.fromSecretKey)
    const mintSigner = getDevnetMintSigner();
    if (!mintSigner) {
      return NextResponse.json(
        { error: "Server not configured for minting (DEVNET_MINT_AUTHORITY_KEYPAIR missing)" },
        { status: 500 },
      );
    }

    const mintAuthPk = new PublicKey(mintSigner.publicKey());
    const connection = new Connection(cfg.rpcUrl, "confirmed");

    // On-chain authority check: verify our signer matches the mint's authority
    // before attempting MintTo. Returns 400 (not 500) on mismatch so callers
    // can distinguish a config error from a transient failure.
    try {
      const mintInfo = await connection.getAccountInfo(usdcMint);
      if (!mintInfo) {
        return NextResponse.json(
          { error: `Test USDC mint ${usdcMintAddr} does not exist on devnet` },
          { status: 500 },
        );
      }
      // SPL Token mint layout: bytes 0-3 coption(u32), bytes 4-35 mint_authority (32 bytes)
      const mintData = new Uint8Array(mintInfo.data);
      if (mintData.length >= 36) {
        const hasAuthority =
          new DataView(mintData.buffer, mintData.byteOffset).getUint32(0, true) === 1;
        if (!hasAuthority) {
          return NextResponse.json(
            { error: "Test USDC mint has no mint authority (fixed supply)" },
            { status: 500 },
          );
        }
        const onChainAuthority = new PublicKey(mintData.slice(4, 36));
        if (!onChainAuthority.equals(mintAuthPk)) {
          Sentry.captureException(
            new Error(
              `faucet: mint authority mismatch — on-chain ${onChainAuthority.toBase58()}, signer ${mintAuthPk.toBase58()}`,
            ),
            { tags: { endpoint: "/api/faucet", step: "authority_check" } },
          );
          return NextResponse.json(
            {
              error:
                "Cannot mint tokens: DEVNET_MINT_AUTHORITY_KEYPAIR does not match the on-chain " +
                "mint authority for testUsdcMint. The mint needs to be re-keyed or the env var updated.",
              mintAuthority: onChainAuthority.toBase58(),
              hint: "authority_mismatch",
            },
            { status: 400 },
          );
        }
      }
    } catch (authErr) {
      // RPC error during check — surface as 503 (retryable)
      const msg = authErr instanceof Error ? authErr.message : String(authErr);
      console.warn("[faucet] mint authority check failed:", msg);
      Sentry.captureException(authErr, {
        tags: { endpoint: "/api/faucet", step: "authority_check" },
        extra: { walletAddress },
      });
      return NextResponse.json(
        { error: "Could not verify mint authority due to RPC error. Please retry.", retryable: true },
        { status: 503 },
      );
    }

    // Build mint transaction
    const ata = await getAssociatedTokenAddress(usdcMint, walletPk);
    const tx = new Transaction();

    // Create ATA if it doesn't exist
    let ataExists = false;
    try {
      await getAccount(connection, ata);
      ataExists = true;
    } catch {
      // ATA not found — will be created in tx
    }

    if (!ataExists) {
      tx.add(
        createAssociatedTokenAccountInstruction(
          mintAuthPk,
          ata,
          walletPk,
          usdcMint,
        ),
      );
    }

    // Mint USDC
    tx.add(
      createMintToInstruction(usdcMint, ata, mintAuthPk, USDC_MINT_AMOUNT),
    );

    // Set blockhash + feePayer before signing (required for sendRawTransaction)
    const { blockhash, lastValidBlockHeight } =
      await connection.getLatestBlockhash("confirmed");
    tx.recentBlockhash = blockhash;
    tx.feePayer = mintAuthPk;

    // Sign with sealed signer and send raw (GH#1382: replaces sendAndConfirmTransaction
    // which internally calls tx.sign() wiping existing partial signatures)
    const signedTx = mintSigner.signTransaction(tx);
    const sig = await connection.sendRawTransaction(
      (signedTx as Transaction).serialize(),
    );
    await connection.confirmTransaction(
      { signature: sig, blockhash, lastValidBlockHeight },
      "confirmed",
    );

    // Log the funding event
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    await (supabase as any).from("auto_fund_log").insert({
      wallet: walletAddress,
      sol_airdropped: false,
      usdc_minted: true,
    });

    const nextClaimAt = new Date(
      Date.now() + RATE_LIMIT_HOURS * 60 * 60 * 1000,
    ).toISOString();

    return NextResponse.json({
      funded: true,
      usdc_minted: true,
      usdc_amount: USDC_MINT_AMOUNT / 1_000_000,
      signature: sig,
      nextClaimAt,
    });
  } catch (error) {
    Sentry.captureException(error, {
      tags: { endpoint: "/api/faucet", method: "POST" },
    });
    return NextResponse.json(
      {
        error:
          error instanceof Error ? error.message : "Internal server error",
      },
      { status: 500 },
    );
  }
}
