import { Connection, PublicKey } from "@solana/web3.js";
import * as dotenv from "dotenv";
dotenv.config();
import { discoverMarkets, parseEngine, parseParams } from "../packages/core/src/index.js";

const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID ?? "FxfD37s1AZTeWfFQps9Zpebi2dNQ9QSSDtfMKdbsfKrD");
const SOL_PYTH_FEED = "ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d";
const BTC_PYTH_FEED = "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43";
const conn = new Connection(process.env.RPC_URL!, "confirmed");

async function main() {
  const markets = await discoverMarkets(conn, PROGRAM_ID);
  console.log(`Total: ${markets.length}\n`);
  
  for (const [label, feed] of [["SOL", SOL_PYTH_FEED], ["BTC", BTC_PYTH_FEED]] as const) {
    for (const m of markets) {
      const feedHex = m.config.indexFeedId.toBase58();
      // check by bytes too
      const feedBytes = m.config.indexFeedId.toBytes();
      const feedHexBytes = Buffer.from(feedBytes).toString("hex");
      if (feedHexBytes !== feed) continue;
      
      const info = await conn.getAccountInfo(m.slabAddress);
      if (!info) continue;
      const data = new Uint8Array(info.data);
      const engine = parseEngine(data);
      const params = parseParams(data);
      const isFull = Number(engine.numUsedAccounts) >= Number(params.maxAccounts);
      
      console.log(`${label}-PERP: ${m.slabAddress.toBase58()}`);
      console.log(`  collateral: ${m.config.collateralMint.toBase58()}`);
      console.log(`  numUsed=${engine.numUsedAccounts} maxAccounts=${params.maxAccounts} isFull=${isFull}`);
      console.log(`  newAccountFee=${params.newAccountFee} unitScale=${params.unitScale}`);
      console.log(`  paused=${m.header.paused} resolved=${m.header.resolved}`);
      console.log();
    }
  }
}
main().catch(console.error);
