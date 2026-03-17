import { Connection, PublicKey } from "@solana/web3.js";
import * as dotenv from "dotenv";
dotenv.config();
import { discoverMarkets } from "../packages/core/src/solana/discovery.js";
import { parseConfig, parseEngine, parseParams } from "../packages/core/src/solana/slab.js";

const conn = new Connection(process.env.RPC_URL!, "confirmed");
const SMALL = new PublicKey("FxfD37s1AZTeWfFQps9Zpebi2dNQ9QSSDtfMKdbsfKrD");
const TARGET_COLLATERAL = "CJUyV594JzJpK2BUakNpm6NbmCkhQoPJWkKjfKTvxJ3C";

function inferSym(priceUsd: number): string {
  if (priceUsd > 50000) return "BTC";
  if (priceUsd > 2000) return "ETH";
  if (priceUsd > 50) return "SOL";
  return "UNKNOWN";
}

async function main() {
  const markets = await discoverMarkets(conn, SMALL);
  console.log(`Small program: ${markets.length} markets\n`);
  
  for (const m of markets) {
    const info = await conn.getAccountInfo(m.slabAddress);
    if (!info) continue;
    const data = new Uint8Array(info.data);
    const config = parseConfig(data);
    const engine = parseEngine(data);
    const params = parseParams(data);
    const priceUsd = Number(config.authorityPriceE6) / 1_000_000;
    const sym = inferSym(priceUsd);
    const collateral = config.collateralMint.toBase58();
    const oracleAuth = config.oracleAuthority?.toBase58?.() ?? "none";
    const isFull = Number(engine.numUsedAccounts) >= Number(params.maxAccounts);
    
    if (collateral === TARGET_COLLATERAL) {
      console.log(`${sym} ${m.slabAddress.toBase58()}`);
      console.log(`  oracleAuth: ${oracleAuth.slice(0,16)}... price=$${priceUsd} numUsed=${engine.numUsedAccounts}/${params.maxAccounts} full=${isFull}`);
    }
  }
}
main().catch(console.error);
