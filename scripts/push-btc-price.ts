#!/usr/bin/env npx tsx
/**
 * One-shot BTC price push script.
 * Pushes the correct BTC price to devnet via PushOraclePrice + KeeperCrank.
 *
 * Usage:
 *   npx tsx scripts/push-btc-price.ts [priceUsd]
 *   npx tsx scripts/push-btc-price.ts 87000
 *
 * Default: fetches live BTC price from Binance.
 */

import {
  Connection, Keypair, PublicKey, Transaction,
  ComputeBudgetProgram, sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  encodePushOraclePrice, encodeKeeperCrank,
  ACCOUNTS_PUSH_ORACLE_PRICE, ACCOUNTS_KEEPER_CRANK,
  buildAccountMetas, buildIx, WELL_KNOWN,
} from "../packages/core/src/index.js";
import * as fs from "fs";

// ── Config ──────────────────────────────────────────────────
const ADMIN_KP_PATH = process.env.ADMIN_KEYPAIR_PATH ??
  `${process.env.HOME}/.config/solana/percolator-upgrade-authority.json`;
const RPC_URL = process.env.RPC_URL ?? "https://api.devnet.solana.com";
const PROGRAM_ID = "FwfBKZXbYr4vTK23bMFkbgKq3npJ3MSDxEaKmq9Aj4Qn";
const BTC_SLAB = "AB3ZN1vxbBEh8FZRfrL55QQUUaLCwawqvCYzTDpgbuLF";

async function fetchBinanceBtcPrice(): Promise<number | null> {
  try {
    const resp = await fetch("https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT", {
      signal: AbortSignal.timeout(5000),
    });
    const json = (await resp.json()) as { price?: string };
    return json.price ? parseFloat(json.price) : null;
  } catch { return null; }
}

async function main() {
  const conn = new Connection(RPC_URL, "confirmed");
  const admin = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(ADMIN_KP_PATH, "utf8")))
  );

  console.log(`Admin: ${admin.publicKey.toBase58()}`);
  console.log(`BTC Slab: ${BTC_SLAB}`);
  console.log(`Program: ${PROGRAM_ID}`);

  // Get price from CLI arg or Binance
  let priceUsd: number;
  const cliPrice = process.argv[2];
  if (cliPrice) {
    priceUsd = parseFloat(cliPrice);
    if (!isFinite(priceUsd) || priceUsd <= 0) {
      console.error(`Invalid price: ${cliPrice}`);
      process.exit(1);
    }
    console.log(`Using CLI price: $${priceUsd}`);
  } else {
    const binancePrice = await fetchBinanceBtcPrice();
    if (!binancePrice) {
      console.error("Failed to fetch Binance BTC price. Pass price as argument: npx tsx scripts/push-btc-price.ts 87000");
      process.exit(1);
    }
    priceUsd = binancePrice;
    console.log(`Fetched Binance BTC price: $${priceUsd}`);
  }

  const priceE6 = BigInt(Math.round(priceUsd * 1_000_000));
  const timestamp = BigInt(Math.floor(Date.now() / 1000));
  const slab = new PublicKey(BTC_SLAB);
  const programId = new PublicKey(PROGRAM_ID);

  console.log(`Price E6: ${priceE6}`);
  console.log(`Timestamp: ${timestamp}`);

  // Build PushOraclePrice instruction
  const pushData = encodePushOraclePrice({ priceE6: priceE6.toString(), timestamp: timestamp.toString() });
  const pushKeys = buildAccountMetas(ACCOUNTS_PUSH_ORACLE_PRICE, [admin.publicKey, slab]);

  // Build KeeperCrank instruction
  const crankData = encodeKeeperCrank({ callerIdx: 65535, allowPanic: false });
  const crankKeys = buildAccountMetas(ACCOUNTS_KEEPER_CRANK, [
    admin.publicKey, slab, WELL_KNOWN.clock, slab,
  ]);

  const tx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 500_000 }),
    ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 }),
    buildIx({ programId, keys: pushKeys, data: pushData }),
    buildIx({ programId, keys: crankKeys, data: crankData }),
  );
  tx.feePayer = admin.publicKey;

  console.log("\nSending transaction...");
  const sig = await sendAndConfirmTransaction(conn, tx, [admin], {
    commitment: "confirmed",
    skipPreflight: true,
  });

  console.log(`\n✅ BTC price pushed: $${priceUsd} → ${sig}`);
  console.log(`   Explorer: https://explorer.solana.com/tx/${sig}?cluster=devnet`);
}

main().catch(e => {
  console.error("❌ Failed:", e.message);
  process.exit(1);
});
