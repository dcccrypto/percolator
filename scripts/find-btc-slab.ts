import { Connection, PublicKey } from "@solana/web3.js";
import * as dotenv from "dotenv";
dotenv.config();
import { detectSlabLayout, parseEngine, parseParams, parseConfig } from "../packages/core/src/solana/slab.js";
import { discoverMarkets } from "../packages/core/src/solana/discovery.js";

const conn = new Connection(process.env.RPC_URL!, "confirmed");
const PROGRAMS = ["FwfBKZXbYr4vTK23bMFkbgKq3npJ3MSDxEaKmq9Aj4Qn", "FxfD37s1AZTeWfFQps9Zpebi2dNQ9QSSDtfMKdbsfKrD"];

async function checkSlabs(prog: string, label: string) {
  const markets = await discoverMarkets(conn, new PublicKey(prog));
  console.log(`\n=== ${label} (${prog.slice(0,8)}) - ${markets.length} markets ===`);
  
  for (const m of markets) {
    const info = await conn.getAccountInfo(m.slabAddress);
    if (!info) continue;
    const data = new Uint8Array(info.data);
    const config = parseConfig(data);
    const engine = parseEngine(data);
    const params = parseParams(data);
    const layout = detectSlabLayout(data.length);
    
    // Infer symbol from authority price range
    const priceUsd = Number(config.authorityPriceE6) / 1_000_000;
    let sym = "UNKNOWN";
    if (priceUsd > 50000) sym = "BTC";
    else if (priceUsd > 2000) sym = "ETH";
    else if (priceUsd > 50) sym = "SOL";
    else if (priceUsd > 0) sym = `~$${priceUsd.toFixed(2)}`;
    
    if (sym !== "UNKNOWN" || Number(engine.numUsedAccounts) > 0) {
      console.log(`  ${m.slabAddress.toBase58().slice(0,16)}... sym=${sym} size=${data.length} numUsed=${engine.numUsedAccounts} maxAccounts=${params.maxAccounts} layoutVersion=${layout ? 'v' + (data.length < 70000 ? '0/small' : 'other') : 'unknown'}`);
    }
  }
}

async function main() {
  for (const prog of PROGRAMS) {
    await checkSlabs(prog, prog.startsWith("FwfBKZXb") ? "medium" : "small");
  }
}
main().catch(console.error);
