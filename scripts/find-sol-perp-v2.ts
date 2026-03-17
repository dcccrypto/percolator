import { Connection, PublicKey } from "@solana/web3.js";
import * as dotenv from "dotenv";
dotenv.config();
import { discoverMarkets, parseEngine, parseParams, derivePythPushOraclePDA } from "../packages/core/src/index.js";

const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID ?? "FxfD37s1AZTeWfFQps9Zpebi2dNQ9QSSDtfMKdbsfKrD");
const SOL_PYTH_FEED = "ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d";
const BTC_PYTH_FEED = "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43";
const conn = new Connection(process.env.RPC_URL!, "confirmed");

async function main() {
  // Derive the Pyth push oracle PDAs for SOL and BTC feeds
  const [solOraclePDA] = derivePythPushOraclePDA(SOL_PYTH_FEED);
  const [btcOraclePDA] = derivePythPushOraclePDA(BTC_PYTH_FEED);
  console.log(`SOL oracle PDA: ${solOraclePDA.toBase58()}`);
  console.log(`BTC oracle PDA: ${btcOraclePDA.toBase58()}`);
  
  const markets = await discoverMarkets(conn, PROGRAM_ID);
  console.log(`Total markets: ${markets.length}\n`);
  
  for (const [label, oraclePDA] of [["SOL", solOraclePDA], ["BTC", btcOraclePDA]] as const) {
    const pda = (oraclePDA as PublicKey).toBase58();
    const matching = markets.filter(m => m.config.indexFeedId.toBase58() === pda);
    
    console.log(`=== ${label}-PERP markets (via Pyth PDA ${pda.slice(0,12)}...) ===`);
    console.log(`Found: ${matching.length}`);
    
    for (const m of matching) {
      const info = await conn.getAccountInfo(m.slabAddress);
      if (!info) continue;
      const data = new Uint8Array(info.data);
      const engine = parseEngine(data);
      const params = parseParams(data);
      const isFull = Number(engine.numUsedAccounts) >= Number(params.maxAccounts);
      console.log(`  ${m.slabAddress.toBase58()}`);
      console.log(`  collateral: ${m.config.collateralMint.toBase58()}`);
      console.log(`  numUsed=${engine.numUsedAccounts} maxAccounts=${params.maxAccounts} isFull=${isFull}`);
      console.log(`  newAccountFee=${params.newAccountFee} unitScale=${params.unitScale}`);
      console.log(`  paused=${m.header.paused} resolved=${m.header.resolved}`);
      if (isFull) console.log(`  ⚠️ SLAB IS FULL → EngineOverflow on InitLP!`);
      console.log();
    }
    console.log();
  }
}
main().catch(console.error);
