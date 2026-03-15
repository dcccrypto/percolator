/**
 * batch-set-oracle-authority.ts
 *
 * Batch-migrates oracle authority on all devnet slab accounts from the
 * deployer key to the keeper key (FF7KFfU5Bb3Mze2AasDHCCZuyhdaSLjUZy2K3JvjdB7x).
 *
 * Usage:
 *   DEPLOYER_KEYPAIR=/path/to/deployer.json pnpm ts-node app/scripts/batch-set-oracle-authority.ts
 *
 * Optional env vars:
 *   RPC_URL            - devnet RPC (default: https://api.devnet.solana.com)
 *   KEEPER_AUTHORITY   - new oracle authority pubkey (default: FF7KFfU5...)
 *   DRY_RUN            - set to "true" to simulate only, no on-chain txs
 *   BATCH_SIZE         - number of txs per wave (default: 5)
 *   START_INDEX        - resume from this market index (default: 0)
 *
 * Context:
 *   GH#1248 / GH#1249 / PERC-806
 *   115 markets were created with deployer as oracle_authority instead of
 *   the keeper wallet. They are frozen (no mark_price, users cannot open
 *   positions). This script iterates every discovered slab and calls
 *   SetOracleAuthority for each one whose oracle_authority matches the
 *   deployer pubkey.
 */

import * as fs from "fs";
import * as path from "path";
import {
  Connection,
  PublicKey,
  Transaction,
  TransactionInstruction,
  Keypair,
  sendAndConfirmTransaction,
  ComputeBudgetProgram,
} from "@solana/web3.js";
import {
  discoverMarkets,
  encodeSetOracleAuthority,
  ACCOUNTS_SET_ORACLE_AUTHORITY,
  buildAccountMetas,
  buildIx,
  getProgramId,
} from "../../packages/core/dist/index.js";
import { maskApiKeys } from "../../packages/shared/src/index.js";

// ─── Config ──────────────────────────────────────────────────────────────────

const RPC_URL = process.env.RPC_URL ?? "https://api.devnet.solana.com";
const KEEPER_AUTHORITY_PUBKEY =
  process.env.KEEPER_AUTHORITY ?? "FF7KFfU5Bb3Mze2AasDHCCZuyhdaSLjUZy2K3JvjdB7x";
const DRY_RUN = process.env.DRY_RUN === "true";
const BATCH_SIZE = parseInt(process.env.BATCH_SIZE ?? "5");
const START_INDEX = parseInt(process.env.START_INDEX ?? "0");

// ─── Entry point ─────────────────────────────────────────────────────────────

async function main() {
  // 1. Load deployer keypair
  const keypairPath = process.env.DEPLOYER_KEYPAIR;
  if (!keypairPath) {
    console.error(
      "ERROR: DEPLOYER_KEYPAIR env var not set.\n" +
        "Usage: DEPLOYER_KEYPAIR=/path/to/deployer.json pnpm ts-node app/scripts/batch-set-oracle-authority.ts"
    );
    process.exit(1);
  }

  const keypairFile = path.resolve(keypairPath);
  if (!fs.existsSync(keypairFile)) {
    console.error(`ERROR: Keypair file not found: ${keypairFile}`);
    process.exit(1);
  }

  const rawKey = JSON.parse(fs.readFileSync(keypairFile, "utf-8"));
  const deployerKeypair = Keypair.fromSecretKey(Uint8Array.from(rawKey));
  const deployerPubkey = deployerKeypair.publicKey;
  const newAuthority = new PublicKey(KEEPER_AUTHORITY_PUBKEY);

  console.log("=".repeat(60));
  console.log("Batch SetOracleAuthority migration");
  console.log("=".repeat(60));
  console.log(`  Deployer:      ${deployerPubkey.toBase58()}`);
  console.log(`  New authority: ${newAuthority.toBase58()}`);
  console.log(`  RPC:           ${maskApiKeys(RPC_URL)}`);
  console.log(`  Dry run:       ${DRY_RUN}`);
  console.log(`  Batch size:    ${BATCH_SIZE}`);
  console.log(`  Start index:   ${START_INDEX}`);
  console.log("");

  // 2. Discover all markets
  const connection = new Connection(RPC_URL, "confirmed");
  const programId = getProgramId("devnet");

  console.log(`Discovering markets on program ${programId.toBase58()}...`);
  const markets = await discoverMarkets(connection, programId);
  console.log(`  Found ${markets.length} total markets`);

  // 3. Filter for markets where oracle_authority == deployer
  const targets = markets.filter(
    (m: import("../../packages/core/dist/index.js").DiscoveredMarket) =>
      m.config.oracleAuthority.toBase58() === deployerPubkey.toBase58() &&
      // Skip markets that already have the correct authority
      m.config.oracleAuthority.toBase58() !== newAuthority.toBase58()
  );

  if (targets.length === 0) {
    console.log(
      "\n✅ No markets found with deployer as oracle_authority. Nothing to do."
    );
    return;
  }

  console.log(
    `\n  Markets needing migration: ${targets.length} (deployer is oracle_authority)`
  );

  // Slice from start index for resumable runs
  const batch = targets.slice(START_INDEX);
  console.log(
    `  Processing ${batch.length} markets (starting from index ${START_INDEX})\n`
  );

  // 4. Build and send transactions in waves
  let success = 0;
  let failure = 0;
  const failures: { slab: string; error: string }[] = [];

  for (let i = 0; i < batch.length; i += BATCH_SIZE) {
    const wave = batch.slice(i, i + BATCH_SIZE);
    const waveIdx = Math.floor(i / BATCH_SIZE) + 1;
    const totalWaves = Math.ceil(batch.length / BATCH_SIZE);

    console.log(
      `Wave ${waveIdx}/${totalWaves} — processing ${wave.length} markets:`
    );

    // Process this wave sequentially to avoid rate limits
    for (const market of wave) {
      const slabAddr = market.slabAddress.toBase58();
      const process = async () => {
        const data = encodeSetOracleAuthority({ newAuthority });
        const keys = buildAccountMetas(ACCOUNTS_SET_ORACLE_AUTHORITY, {
          admin: deployerPubkey,
          slab: market.slabAddress,
        });
        const ix = buildIx({ programId, keys, data });

        if (DRY_RUN) {
          // Simulate only
          const tx = new Transaction();
          tx.add(ComputeBudgetProgram.setComputeUnitLimit({ units: 50_000 }));
          tx.add(ix);
          tx.recentBlockhash = (
            await connection.getLatestBlockhash()
          ).blockhash;
          tx.feePayer = deployerPubkey;
          tx.sign(deployerKeypair);

          const result = await connection.simulateTransaction(tx, [
            deployerKeypair,
          ]);
          if (result.value.err) {
            throw new Error(JSON.stringify(result.value.err));
          }
          console.log(`  [DRY] ${slabAddr} — simulation OK`);
        } else {
          // Actually send
          const tx = new Transaction();
          tx.add(ComputeBudgetProgram.setComputeUnitLimit({ units: 50_000 }));
          tx.add(ix);
          const sig = await sendAndConfirmTransaction(connection, tx, [
            deployerKeypair,
          ]);
          console.log(`  ✅ ${slabAddr} → sig: ${sig.slice(0, 16)}...`);
        }
      };

      try {
        await process();
        success++;
      } catch (err) {
        const errMsg = err instanceof Error ? err.message : String(err);
        console.error(`  ❌ ${slabAddr} — ${errMsg}`);
        failures.push({ slab: slabAddr, error: errMsg });
        failure++;
      }

      // Small delay between txs to stay under RPC rate limits
      await sleep(300);
    }

    if (i + BATCH_SIZE < batch.length) {
      console.log("  Cooling down 1s before next wave...");
      await sleep(1000);
    }
  }

  // 5. Summary
  console.log("\n" + "=".repeat(60));
  console.log("Summary");
  console.log("=".repeat(60));
  console.log(`  Success: ${success}`);
  console.log(`  Failure: ${failure}`);

  if (failures.length > 0) {
    console.log("\nFailed markets:");
    for (const f of failures) {
      console.log(`  ${f.slab}: ${f.error}`);
    }
    console.log(
      "\nTip: Re-run with START_INDEX=<n> to resume after last success."
    );
  } else if (!DRY_RUN) {
    console.log(
      "\n✅ All markets migrated. Keeper oracle-authority is now FF7KFfU5..."
    );
  }
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch((err) => {
  console.error("Fatal:", err);
  process.exit(1);
});
