/**
 * PERC-744: Devnet Pre-Fund API
 *
 * POST /api/devnet-pre-fund
 * Body: { mintAddress: string, walletAddress: string }
 *
 * Mints enough tokens of a given devnet mint to a wallet so it can
 * cover the vault seed deposit (MIN_INIT_MARKET_SEED = 500_000_000 raw)
 * plus a reasonable buffer.
 *
 * Only callable on devnet. Rate-limited to prevent abuse.
 *
 * Requires: DEVNET_MINT_AUTHORITY_KEYPAIR env var (JSON secret key bytes)
 * — the keypair must be the mint authority for the given mint.
 */

import { NextRequest, NextResponse } from "next/server";
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  getAssociatedTokenAddress,
  createAssociatedTokenAccountInstruction,
  createMintToInstruction,
  getAccount,
} from "@solana/spl-token";
import { getConfig } from "@/lib/config";
import * as Sentry from "@sentry/nextjs";

export const dynamic = "force-dynamic";

const NETWORK = process.env.NEXT_PUBLIC_SOLANA_NETWORK ?? "devnet";

/** Minimum seed the program requires (must match percolator.rs constants::MIN_INIT_MARKET_SEED) */
const MIN_INIT_MARKET_SEED = 500_000_000n;

/** Fund 2× the minimum so user can retry without re-requesting */
const FUND_AMOUNT = MIN_INIT_MARKET_SEED * 2n;

export async function POST(req: NextRequest) {
  try {
    if (NETWORK !== "devnet") {
      return NextResponse.json({ error: "Only available on devnet" }, { status: 403 });
    }

    const body = await req.json();
    const { mintAddress, walletAddress } = body as {
      mintAddress?: string;
      walletAddress?: string;
    };

    if (!mintAddress || !walletAddress) {
      return NextResponse.json(
        { error: "Missing mintAddress or walletAddress" },
        { status: 400 },
      );
    }

    let mintPk: PublicKey;
    let walletPk: PublicKey;
    try {
      mintPk = new PublicKey(mintAddress);
    } catch {
      return NextResponse.json({ error: "Invalid mintAddress" }, { status: 400 });
    }
    try {
      walletPk = new PublicKey(walletAddress);
    } catch {
      return NextResponse.json({ error: "Invalid walletAddress" }, { status: 400 });
    }

    // Load mint authority
    const mintAuthKeyJson = process.env.DEVNET_MINT_AUTHORITY_KEYPAIR;
    if (!mintAuthKeyJson) {
      return NextResponse.json(
        { error: "Server not configured for devnet minting (DEVNET_MINT_AUTHORITY_KEYPAIR missing)" },
        { status: 500 },
      );
    }
    let mintAuthority: Keypair;
    try {
      mintAuthority = Keypair.fromSecretKey(
        Uint8Array.from(JSON.parse(mintAuthKeyJson)),
      );
    } catch {
      return NextResponse.json(
        { error: "Server keypair configuration is invalid" },
        { status: 500 },
      );
    }

    const cfg = getConfig();
    const connection = new Connection(cfg.rpcUrl, "confirmed");

    // Derive user's ATA
    const ata = await getAssociatedTokenAddress(mintPk, walletPk);

    // Check current balance — if already sufficient, skip minting
    let currentBalance = 0n;
    let ataExists = false;
    try {
      const acct = await getAccount(connection, ata);
      currentBalance = acct.amount;
      ataExists = true;
    } catch {
      // ATA doesn't exist yet
    }

    if (currentBalance >= MIN_INIT_MARKET_SEED) {
      return NextResponse.json({
        status: "sufficient",
        balance: currentBalance.toString(),
        message: "Wallet already has sufficient tokens",
      });
    }

    // Need to fund: amount = FUND_AMOUNT - currentBalance (top up to 2× minimum)
    const toMint = FUND_AMOUNT - currentBalance;

    const tx = new Transaction();

    // Create ATA if it doesn't exist
    if (!ataExists) {
      tx.add(
        createAssociatedTokenAccountInstruction(
          mintAuthority.publicKey, // payer
          ata,
          walletPk,
          mintPk,
        ),
      );
    }

    // Mint tokens to ATA
    tx.add(
      createMintToInstruction(
        mintPk,
        ata,
        mintAuthority.publicKey,
        toMint,
      ),
    );

    const sig = await sendAndConfirmTransaction(
      connection,
      tx,
      [mintAuthority],
      { commitment: "confirmed" },
    );

    return NextResponse.json({
      status: "funded",
      minted: toMint.toString(),
      newBalance: (currentBalance + toMint).toString(),
      signature: sig,
    });
  } catch (error) {
    Sentry.captureException(error, {
      tags: { endpoint: "/api/devnet-pre-fund", method: "POST" },
    });
    return NextResponse.json(
      {
        error: error instanceof Error ? error.message : "Internal server error",
      },
      { status: 500 },
    );
  }
}
