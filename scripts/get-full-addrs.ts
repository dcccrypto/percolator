import { Connection, PublicKey } from "@solana/web3.js";
import * as dotenv from "dotenv";
dotenv.config();
import { discoverMarkets } from "../packages/core/src/solana/discovery.js";
import { parseConfig, parseEngine, parseParams } from "../packages/core/src/solana/slab.js";

const conn = new Connection(process.env.RPC_URL!, "confirmed");
const MEDIUM = "FwfBKZXbYr4vTK23bMFkbgKq3npJ3MSDxEaKmq9Aj4Qn";
const SMALL = "FxfD37s1AZTeWfFQps9Zpebi2dNQ9QSSDtfMKdbsfKrD";

function inferSym(priceUsd: number): string {
  if (priceUsd > 50000) return "BTC";
  if (priceUsd > 2000) return "ETH";
  if (priceUsd > 50) return "SOL";
  return "UNKNOWN";
}

async function checkProgram(progId: string, label: string, filter: string) {
  console.log(`\n=== ${label} ===`);
  const markets = await discoverMarkets(conn, new PublicKey(progId));
  for (const m of markets) {
    const info = await conn.getAccountInfo(m.slabAddress);
    if (!info) continue;
    const data = new Uint8Array(info.data);
    const config = parseConfig(data);
    const engine = parseEngine(data);
    const params = parseParams(data);
    const priceUsd = Number(config.authorityPriceE6) / 1_000_000;
    const sym = inferSym(priceUsd);
    if (sym === filter || filter === "ALL") {
      const isFull = Number(engine.numUsedAccounts) >= Number(params.maxAccounts);
      console.log(`${sym} ${m.slabAddress.toBase58()} used=${engine.numUsedAccounts}/${params.maxAccounts} full=${isFull} collateral=${config.collateralMint.toBase58().slice(0,8)}`);
    }
  }
}

async function main() {
  await checkProgram(MEDIUM, "MEDIUM - SOL+BTC", "SOL");
  await checkProgram(MEDIUM, "MEDIUM - BTC", "BTC");
  await checkProgram(SMALL, "SMALL - SOL", "SOL");
}
main().catch(console.error);
