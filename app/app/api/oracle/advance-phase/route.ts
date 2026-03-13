/**
 * GH#1120: Server-side AdvanceOraclePhase crank
 *
 * POST /api/oracle/advance-phase
 * Body: { slabAddress: string }
 *
 * Sends an AdvanceOraclePhase transaction signed by the server-side crank
 * keypair (DEVNET_MINT_AUTHORITY_KEYPAIR or CRANK_KEYPAIR) — NOT the user's
 * wallet. This removes the Privy "Confirm transaction" modal that was firing
 * on every trade page load.
 *
 * AdvanceOraclePhase is a permissionless instruction (any fee payer works).
 * Only runs on devnet; silently no-ops on mainnet.
 *
 * Requires (at least one):
 *   - CRANK_KEYPAIR — JSON secret key bytes (preferred)
 *   - DEVNET_MINT_AUTHORITY_KEYPAIR — fallback
 */

import { NextRequest, NextResponse } from "next/server";
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  ComputeBudgetProgram,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  encodeAdvanceOraclePhase,
  buildIx,
  buildAccountMetas,
  ACCOUNTS_ADVANCE_ORACLE_PHASE,
} from "@percolator/sdk";
import { getConfig } from "@/lib/config";
import * as Sentry from "@sentry/nextjs";

export const dynamic = "force-dynamic";

function loadCrankKeypair(): Keypair | null {
  const raw = process.env.CRANK_KEYPAIR ?? process.env.DEVNET_MINT_AUTHORITY_KEYPAIR;
  if (!raw) return null;
  try {
    return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(raw)));
  } catch {
    return null;
  }
}

function isValidBase58(s: string): boolean {
  return /^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(s);
}

export async function POST(req: NextRequest) {
  // Only active on devnet — on mainnet/mainnet-fork this is a no-op
  const network = process.env.NEXT_PUBLIC_SOLANA_NETWORK?.trim() ?? "mainnet";
  if (network !== "devnet") {
    return NextResponse.json({ skipped: true, reason: "not devnet" });
  }

  let slabAddress: string;
  try {
    const body = await req.json();
    slabAddress = body?.slabAddress ?? "";
  } catch {
    return NextResponse.json({ error: "Invalid JSON body" }, { status: 400 });
  }

  if (!slabAddress || !isValidBase58(slabAddress)) {
    return NextResponse.json({ error: "slabAddress required (valid base58)" }, { status: 400 });
  }

  const crankKp = loadCrankKeypair();
  if (!crankKp) {
    // Silently skip — CRANK_KEYPAIR not configured on this deployment
    return NextResponse.json({ skipped: true, reason: "no crank keypair configured" });
  }

  try {
    const cfg = getConfig();
    const connection = new Connection(cfg.rpcUrl, "confirmed");
    const slab = new PublicKey(slabAddress);

    // Determine program ID — try NEXT_PUBLIC_PROGRAM_ID, fall back to known large-tier program
    const programIdStr =
      process.env.NEXT_PUBLIC_PROGRAM_ID ??
      "FxfD37s1NC7CDPMPzqgSfLsiJxjYRjfQDsV1CRuW9dBH"; // large-tier devnet default
    const programId = new PublicKey(programIdStr);

    const data = encodeAdvanceOraclePhase();
    const keys = buildAccountMetas(ACCOUNTS_ADVANCE_ORACLE_PHASE, [slab]);
    const ix = buildIx({ programId, keys, data });

    const tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 50_000 }),
      ix,
    );
    tx.feePayer = crankKp.publicKey;

    const sig = await sendAndConfirmTransaction(connection, tx, [crankKp], {
      commitment: "confirmed",
      skipPreflight: true,
    });

    return NextResponse.json({ success: true, signature: sig });
  } catch (err) {
    // Expected failure: market not ready for phase advance — on-chain program returns error
    // Treat as non-critical; do not spam Sentry for expected no-ops
    const msg = err instanceof Error ? err.message : String(err);
    const isExpected =
      msg.includes("custom program error") ||
      msg.includes("0x") ||
      msg.includes("Transaction simulation failed");

    if (!isExpected) {
      Sentry.captureException(err, {
        extra: { slabAddress, context: "advance-phase-crank" },
      });
    }

    return NextResponse.json({ success: false, skipped: true, reason: msg });
  }
}
