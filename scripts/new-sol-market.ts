/**
 * PERC-807: Create a fresh SOL-PERP market on FwfBKZXb (65352-byte small slab).
 * Copies oracle authority, collateral mint, and params from the FULL SOL slab.
 *
 * Usage: npx tsx scripts/new-sol-market.ts [--dry-run]
 */
import {
  Connection, Keypair, PublicKey, Transaction,
  sendAndConfirmTransaction, ComputeBudgetProgram, SystemProgram,
  SYSVAR_CLOCK_PUBKEY, SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import { getOrCreateAssociatedTokenAccount, createTransferInstruction, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import * as fs from "fs";
import * as dotenv from "dotenv";
import { parseArgs } from "node:util";
dotenv.config();

import { parseConfig, parseParams, parseEngine } from "../packages/core/src/solana/slab.js";
import {
  encodeInitMarket, encodePushOraclePrice, encodeKeeperCrank, encodeSetOracleAuthority,
} from "../packages/core/src/abi/instructions.js";
import {
  ACCOUNTS_INIT_MARKET, ACCOUNTS_PUSH_ORACLE_PRICE, ACCOUNTS_KEEPER_CRANK,
  ACCOUNTS_SET_ORACLE_AUTHORITY, buildAccountMetas,
} from "../packages/core/src/abi/accounts.js";
import { buildIx } from "../packages/core/src/runtime/tx.js";
import { deriveVaultAuthority } from "../packages/core/src/solana/pda.js";
import { SLAB_TIERS } from "../packages/core/src/solana/discovery.js";

const { values: args } = parseArgs({
  options: { "dry-run": { type: "boolean", default: false } },
  strict: true,
});
const DRY_RUN = args["dry-run"] ?? false;

const FULL_SOL_SLAB = "GGU89iQLmceyXRDK8vgAxVvdi9RJb9JsPhXZ2NoFSENV";
const PROGRAM_ID = new PublicKey("FwfBKZXbYr4vTK23bMFkbgKq3npJ3MSDxEaKmq9Aj4Qn");
const SLAB_SIZE = SLAB_TIERS.small.dataSize; // 65352

function loadKeypair(path: string): Keypair {
  const p = path.startsWith("~/") ? path.replace("~", process.env.HOME!) : path;
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(p, "utf-8"))));
}
function addPriority(tx: Transaction) {
  tx.add(ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 }));
}

async function main() {
  const rpcUrl = process.env.RPC_URL!;
  if (!rpcUrl.includes("devnet") && !rpcUrl.includes("localhost") && !rpcUrl.includes("helius")) {
    throw new Error("DEVNET ONLY");
  }
  const conn = new Connection(rpcUrl, "confirmed");
  const admin = loadKeypair(process.env.ADMIN_KEYPAIR_PATH!);
  console.log(`Admin:    ${admin.publicKey.toBase58()}`);
  console.log(`Program:  ${PROGRAM_ID.toBase58()}`);
  console.log(`Dry-run:  ${DRY_RUN}`);

  // Step 1: Read config from full SOL slab
  console.log(`\n--- Reading config from full SOL slab ${FULL_SOL_SLAB.slice(0, 16)}... ---`);
  const slabInfo = await conn.getAccountInfo(new PublicKey(FULL_SOL_SLAB));
  if (!slabInfo) throw new Error("Full SOL slab not found on-chain");
  const data = new Uint8Array(slabInfo.data);
  const config = parseConfig(data);
  const params = parseParams(data);
  const engine = parseEngine(data);
  const markPrice = engine.markPriceE6 > 0n ? engine.markPriceE6 : BigInt(148_000_000);

  const oracleAuth = config.oracleAuthority instanceof PublicKey
    ? config.oracleAuthority
    : new PublicKey(config.oracleAuthority as any);

  console.log(`  collateral:   ${config.collateralMint.toBase58()}`);
  console.log(`  oracle_auth:  ${oracleAuth.toBase58()}`);
  console.log(`  markPrice:    $${Number(markPrice) / 1e6}`);
  console.log(`  maintenanceBps: ${params.maintenanceMarginBps}`);
  console.log(`  initialBps:   ${params.initialMarginBps}`);
  console.log(`  tradingFeeBps: ${params.tradingFeeBps}`);
  console.log(`  maxAccounts:  ${params.maxAccounts}`);
  console.log(`  newAccountFee: ${params.newAccountFee}`);

  // Hyperp mode: indexFeedId = all zeros (authority oracle)
  const indexFeedId = "0".repeat(64);

  // Step 2: Create new slab account
  const newSlabKp = Keypair.generate();
  // Save keypair immediately for recovery
  if (!DRY_RUN) {
    fs.writeFileSync(`/tmp/perc-807-new-sol-slab-${newSlabKp.publicKey.toBase58().slice(0,8)}.json`, JSON.stringify(Array.from(newSlabKp.secretKey)));
    console.log(`  Saved slab keypair to /tmp/perc-807-new-sol-slab-${newSlabKp.publicKey.toBase58().slice(0,8)}.json`);
  }
  const rent = await conn.getMinimumBalanceForRentExemption(SLAB_SIZE);
  console.log(`\n--- Step 2: Create new ${SLAB_SIZE}-byte slab ---`);
  console.log(`  New slab: ${newSlabKp.publicKey.toBase58()}`);
  console.log(`  Rent: ${(rent / 1e9).toFixed(4)} SOL`);

  const [vaultPda] = deriveVaultAuthority(PROGRAM_ID, newSlabKp.publicKey);
  console.log(`  Vault PDA: ${vaultPda.toBase58()}`);

  if (!DRY_RUN) {
    const createTx = new Transaction();
    addPriority(createTx);
    createTx.add(ComputeBudgetProgram.setComputeUnitLimit({ units: 50_000 }));
    createTx.add(SystemProgram.createAccount({
      fromPubkey: admin.publicKey,
      newAccountPubkey: newSlabKp.publicKey,
      lamports: rent,
      space: SLAB_SIZE,
      programId: PROGRAM_ID,
    }));
    const sig = await sendAndConfirmTransaction(conn, createTx, [admin, newSlabKp], { commitment: "confirmed" });
    console.log(`  ✅ Slab created: ${sig.slice(0, 20)}...`);
  } else {
    console.log("  [DRY RUN] Would create slab account");
  }

  // Step 3: Get vault ATA
  const collateralMint = config.collateralMint;
  let vaultAta: PublicKey;
  if (!DRY_RUN) {
    const vaultAtaAcct = await getOrCreateAssociatedTokenAccount(conn, admin, collateralMint, vaultPda, true);
    vaultAta = vaultAtaAcct.address;
    console.log(`\n  vaultAta: ${vaultAta.toBase58()}`);
  } else {
    vaultAta = new PublicKey("11111111111111111111111111111111"); // placeholder
    console.log(`  [DRY RUN] Would create vault ATA`);
  }

  // Step 3b: Pre-seed vault with 500 USDC (FwfBKZXb enforces MIN_INIT_MARKET_SEED=500_000_000)
  const MIN_SEED = BigInt(500_000_000);
  console.log(`\n--- Step 3b: Pre-seed vault with ${Number(MIN_SEED) / 1e6} tokens ---`);
  if (!DRY_RUN) {
    const adminAta = await getOrCreateAssociatedTokenAccount(conn, admin, collateralMint, admin.publicKey);
    const seedTx = new Transaction();
    addPriority(seedTx);
    seedTx.add(ComputeBudgetProgram.setComputeUnitLimit({ units: 50_000 }));
    seedTx.add(createTransferInstruction(adminAta.address, vaultAta, admin.publicKey, MIN_SEED));
    const sig = await sendAndConfirmTransaction(conn, seedTx, [admin], { commitment: "confirmed" });
    console.log(`  ✅ Seeded vault: ${sig.slice(0, 20)}...`);
  } else {
    console.log("  [DRY RUN] Would transfer 500 USDC to vault");
  }

  // Step 4: InitMarket
  console.log(`\n--- Step 3: InitMarket ---`);
  const initData = encodeInitMarket({
    admin: admin.publicKey,
    collateralMint,
    indexFeedId,
    maxStalenessSecs: (params.maxCrankStalenessSlots ?? BigInt(200)).toString(),
    confFilterBps: 0,
    invert: (config.invert ? 1 : 0) as number,
    unitScale: Number(config.unitScale ?? 0),
    initialMarkPriceE6: markPrice.toString(),
    warmupPeriodSlots: (params.warmupPeriodSlots ?? BigInt(150)).toString(),
    maintenanceMarginBps: params.maintenanceMarginBps.toString(),
    initialMarginBps: params.initialMarginBps.toString(),
    tradingFeeBps: (params.tradingFeeBps ?? BigInt(30)).toString(),
    maxAccounts: (params.maxAccounts ?? BigInt(256)).toString(),
    newAccountFee: (params.newAccountFee ?? BigInt(1_000_000)).toString(),
    riskReductionThreshold: (params.riskReductionThreshold ?? BigInt(0)).toString(),
    maintenanceFeePerSlot: (params.maintenanceFeePerSlot ?? BigInt(0)).toString(),
    maxCrankStalenessSlots: (params.maxCrankStalenessSlots ?? BigInt(200)).toString(),
    liquidationFeeBps: (params.liquidationFeeBps ?? BigInt(100)).toString(),
    liquidationFeeCap: (params.liquidationFeeCap ?? BigInt(0)).toString(),
    liquidationBufferBps: (params.liquidationBufferBps ?? BigInt(50)).toString(),
    minLiquidationAbs: (params.minLiquidationAbs ?? BigInt(0)).toString(),
  });

  const initMarketKeys = buildAccountMetas(ACCOUNTS_INIT_MARKET, [
    admin.publicKey,        // admin
    newSlabKp.publicKey,   // slab
    collateralMint,         // mint
    vaultAta,               // vault
    TOKEN_PROGRAM_ID,       // tokenProgram
    SYSVAR_CLOCK_PUBKEY,    // clock
    SYSVAR_RENT_PUBKEY,     // rent
    vaultAta,               // dummyAta (same as vault per create-market.ts convention)
    SystemProgram.programId, // systemProgram
  ]);

  if (!DRY_RUN) {
    const initTx = new Transaction();
    addPriority(initTx);
    initTx.add(ComputeBudgetProgram.setComputeUnitLimit({ units: 200_000 }));
    initTx.add(buildIx({ programId: PROGRAM_ID, keys: initMarketKeys, data: initData }));
    const sig = await sendAndConfirmTransaction(conn, initTx, [admin], { commitment: "confirmed" });
    console.log(`  ✅ InitMarket: ${sig.slice(0, 20)}...`);
  } else {
    console.log("  [DRY RUN] Would call InitMarket");
  }

  // Step 5: SetOracleAuthority
  console.log(`\n--- Step 4: SetOracleAuthority → ${oracleAuth.toBase58().slice(0,12)}... ---`);
  const setAuthKeys = buildAccountMetas(ACCOUNTS_SET_ORACLE_AUTHORITY, [
    admin.publicKey,
    newSlabKp.publicKey,
  ]);

  if (!DRY_RUN) {
    const setAuthData = encodeSetOracleAuthority({ newAuthority: oracleAuth });
    const setAuthTx = new Transaction();
    addPriority(setAuthTx);
    setAuthTx.add(buildIx({ programId: PROGRAM_ID, keys: setAuthKeys, data: setAuthData }));
    const sig = await sendAndConfirmTransaction(conn, setAuthTx, [admin], { commitment: "confirmed" });
    console.log(`  ✅ SetOracleAuthority: ${sig.slice(0, 20)}...`);
  } else {
    console.log("  [DRY RUN] Would SetOracleAuthority");
  }

  // Step 6: PushOraclePrice
  console.log(`\n--- Step 5: PushOraclePrice $${Number(markPrice) / 1e6} ---`);
  const pushKeys = buildAccountMetas(ACCOUNTS_PUSH_ORACLE_PRICE, [
    admin.publicKey,       // authority
    newSlabKp.publicKey,  // slab
  ]);

  if (!DRY_RUN) {
    const nowSec = BigInt(Math.floor(Date.now() / 1000));
    const pushData = encodePushOraclePrice({ priceE6: markPrice, timestamp: nowSec });
    const pushTx = new Transaction();
    addPriority(pushTx);
    pushTx.add(ComputeBudgetProgram.setComputeUnitLimit({ units: 100_000 }));
    pushTx.add(buildIx({ programId: PROGRAM_ID, keys: pushKeys, data: pushData }));
    const sig = await sendAndConfirmTransaction(conn, pushTx, [admin], { commitment: "confirmed" });
    console.log(`  ✅ PushOraclePrice: ${sig.slice(0, 20)}...`);
  } else {
    console.log("  [DRY RUN] Would PushOraclePrice");
  }

  // Step 7: KeeperCrank (warmup)
  console.log(`\n--- Step 6: KeeperCrank (initial warmup) ---`);
  const crankKeys = buildAccountMetas(ACCOUNTS_KEEPER_CRANK, [
    admin.publicKey,       // caller
    newSlabKp.publicKey,  // slab
    SYSVAR_CLOCK_PUBKEY,   // clock
    newSlabKp.publicKey,  // oracle (authority mode: slab itself)
  ]);

  if (!DRY_RUN) {
    const crankData = encodeKeeperCrank({ callerIdx: 0, allowPanic: false });
    const crankTx = new Transaction();
    addPriority(crankTx);
    crankTx.add(ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }));
    crankTx.add(buildIx({ programId: PROGRAM_ID, keys: crankKeys, data: crankData }));
    const sig = await sendAndConfirmTransaction(conn, crankTx, [admin], { commitment: "confirmed" });
    console.log(`  ✅ KeeperCrank: ${sig.slice(0, 20)}...`);
  } else {
    console.log("  [DRY RUN] Would KeeperCrank");
  }

  // Summary
  console.log(`\n${"=".repeat(70)}`);
  if (!DRY_RUN) {
    console.log(`✅ New SOL-PERP market created!`);
    console.log(`\nNew slab address: ${newSlabKp.publicKey.toBase58()}`);
  } else {
    console.log(`[DRY RUN] Would create new SOL-PERP market`);
    console.log(`New slab would be: ${newSlabKp.publicKey.toBase58()} (key not saved in dry-run)`);
  }
  console.log(`\nUpdate Railway env vars:`);
  console.log(`  PROGRAM_ID=FwfBKZXbYr4vTK23bMFkbgKq3npJ3MSDxEaKmq9Aj4Qn`);
  console.log(`  MARKET_SYMBOL_OVERRIDES=<NEW_SLAB>:SOL,CkcwQtUuPe1MjeVhyMR2zZcLsKEzP2cqGzspwmgTuZRp:BTC`);
  console.log(`  (BTC slab has 248/256 slots remaining — still good)`);
  console.log(`${"=".repeat(70)}`);
}
main().catch(console.error);
