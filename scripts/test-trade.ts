/**
 * Minimal trade test on BTC-PERP
 */
import { Connection, Keypair, PublicKey, Transaction, ComputeBudgetProgram, sendAndConfirmTransaction } from "@solana/web3.js";
import {
  encodeTradeCpi, encodeKeeperCrank,
  ACCOUNTS_TRADE_CPI, ACCOUNTS_KEEPER_CRANK,
  buildAccountMetas, buildIx, WELL_KNOWN,
  deriveLpPda, parseAllAccounts,
} from "../packages/core/src/index.js";
import * as fs from "fs";

const conn = new Connection("https://api.devnet.solana.com", "confirmed");
const wallet = Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(fs.readFileSync("/tmp/percolator-keepers/keeper-wide.json", "utf8")))
);
const deploy = JSON.parse(fs.readFileSync("/tmp/percolator-devnet-deployment.json", "utf8"));
const programId = new PublicKey(deploy.programId);
const matcherProgramId = new PublicKey(deploy.matcherProgramId);

const BTC_SLAB = new PublicKey(deploy.markets[1].slab); // BTC-PERP

async function main() {
  console.log("Wallet:", wallet.publicKey.toBase58());
  console.log("BTC slab:", BTC_SLAB.toBase58());

  // Read slab state
  const slabInfo = await conn.getAccountInfo(BTC_SLAB);
  if (!slabInfo) throw new Error("Slab not found");
  const accounts = parseAllAccounts(new Uint8Array(slabInfo.data));

  console.log("\nAccounts:");
  for (const a of accounts) {
    const kind = a.account.kind === 0 ? "USER" : "LP";
    console.log(`  idx=${a.idx} kind=${kind} owner=${a.account.owner.toBase58().slice(0, 12)}... capital=${Number(a.account.capital) / 1e6}`);
    if (a.account.kind === 1) {
      console.log(`    matcherProgram: ${a.account.matcherProgram.toBase58()}`);
      console.log(`    matcherContext: ${a.account.matcherContext.toBase58()}`);
    }
  }

  const adminLp = accounts.find(a => a.account.kind === 1 && a.idx === 0);
  const userAcc = accounts.find(a => a.account.kind === 0 && a.account.owner.equals(wallet.publicKey));

  if (!adminLp || !userAcc) {
    console.error("Missing LP or user account");
    return;
  }

  console.log(`\nUsing LP idx=${adminLp.idx}, User idx=${userAcc.idx}`);
  console.log(`LP matcherProgram: ${adminLp.account.matcherProgram.toBase58()}`);
  console.log(`LP matcherContext: ${adminLp.account.matcherContext.toBase58()}`);

  const [lpPda] = deriveLpPda(programId, BTC_SLAB, adminLp.idx);
  console.log(`LP PDA: ${lpPda.toBase58()}`);

  // Crank
  const crankData = encodeKeeperCrank({ callerIdx: 65535, allowPanic: false });
  const crankKeys = buildAccountMetas(ACCOUNTS_KEEPER_CRANK, [
    wallet.publicKey, BTC_SLAB, WELL_KNOWN.clock, BTC_SLAB,
  ]);
  const crankIx = buildIx({ programId, keys: crankKeys, data: crankData });

  // Trade: size is in TOKEN units (6 decimals), notional = size * oracle_price
  // BTC at ~$69K: to get $10 notional, size = $10 / $69000 = 0.000145 tokens = 145 native
  const tradeSize = 145n; // ~$10 notional at $69K BTC
  const tradeData = encodeTradeCpi({
    lpIdx: adminLp.idx,
    userIdx: userAcc.idx,
    size: tradeSize.toString(),
  });

  const tradeKeys = buildAccountMetas(ACCOUNTS_TRADE_CPI, [
    wallet.publicKey,
    adminLp.account.owner,
    BTC_SLAB,
    BTC_SLAB, // oracle = slab for hyperp
    adminLp.account.matcherProgram,
    adminLp.account.matcherContext,
    lpPda,
  ]);
  const tradeIx = buildIx({ programId, keys: tradeKeys, data: tradeData });

  console.log("\nSending crank + trade ($1)...");
  const tx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 }),
    ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 }),
    crankIx,
    tradeIx,
  );
  tx.feePayer = wallet.publicKey;
  const { blockhash } = await conn.getLatestBlockhash("confirmed");
  tx.recentBlockhash = blockhash;

  try {
    const sig = await sendAndConfirmTransaction(conn, tx, [wallet], { commitment: "confirmed" });
    console.log("✅ Trade succeeded:", sig);
  } catch (e: any) {
    console.error("❌ Trade failed");
    if (e.logs) {
      for (const log of e.logs) console.error("  ", log);
    } else {
      console.error("  ", e.message?.slice(0, 200));
    }
  }
}

main().catch(e => console.error("Fatal:", e.message));
