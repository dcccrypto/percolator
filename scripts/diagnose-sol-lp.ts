/**
 * PERC-807 Diagnostic: check SOL slab engine state to find EngineOverflow cause
 * Usage: npx tsx scripts/diagnose-sol-lp.ts
 */
import { Connection, PublicKey } from "@solana/web3.js";
import * as dotenv from "dotenv";
dotenv.config();

import {
  parseEngine,
  parseParams,
  parseAllAccounts,
  discoverMarkets,
} from "../packages/core/src/index.js";

const RPC = process.env.RPC_URL!;
const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID ?? "FxfD37s1AZTeWfFQps9Zpebi2dNQ9QSSDtfMKdbsfKrD");
const SOL_MINT = "So11111111111111111111111111111111111111112";

async function main() {
  const conn = new Connection(RPC, "confirmed");
  
  console.log(`Program: ${PROGRAM_ID.toBase58()}`);
  console.log("Discovering all markets...\n");
  const markets = await discoverMarkets(conn, PROGRAM_ID);
  console.log(`Found ${markets.length} total markets.\n`);
  
  let fullSlabs = 0;
  
  for (const market of markets) {
    try {
      const info = await conn.getAccountInfo(market.slabAddress);
      if (!info) continue;
      const data = new Uint8Array(info.data);
      const engine = parseEngine(data);
      const params = parseParams(data);
      const accounts = parseAllAccounts(data);
      const lpAccounts = accounts.filter(a => a.account.kind === 1);
      const mint = market.config.collateralMint.toBase58();
      const isSOL = mint === SOL_MINT;
      
      const isFull = Number(engine.numUsedAccounts) >= Number(params.maxAccounts);
      
      console.log(`${market.slabAddress.toBase58().slice(0,16)}... [${isSOL ? 'SOL' : mint.slice(0,8)+'...'}]`);
      console.log(`  numUsed=${engine.numUsedAccounts} maxAccounts=${params.maxAccounts} LPs=${lpAccounts.length} full=${isFull}`);
      console.log(`  newAccountFee=${params.newAccountFee} unitScale=${params.unitScale}`);
      
      if (isFull) {
        fullSlabs++;
        console.log(`  ⚠️ SLAB FULL — InitLP → EngineOverflow`);
      }
      
    } catch (e: any) {
      console.log(`  Error reading ${market.slabAddress.toBase58().slice(0,12)}: ${e.message}`);
    }
    console.log();
  }
  
  console.log(`\nSummary: ${fullSlabs} full slab(s) found`);
}

main().catch(console.error);
