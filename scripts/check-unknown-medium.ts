import { Connection, PublicKey } from "@solana/web3.js";
import * as dotenv from "dotenv";
dotenv.config();
import { discoverMarkets } from "../packages/core/src/solana/discovery.js";
import { parseConfig, parseEngine, parseParams } from "../packages/core/src/solana/slab.js";

const conn = new Connection(process.env.RPC_URL!, "confirmed");
const MEDIUM = new PublicKey("FwfBKZXbYr4vTK23bMFkbgKq3npJ3MSDxEaKmq9Aj4Qn");
const ADMIN = "FF7KFfU5Bb3Mze2AasDHCCZuyhdaSLjUZy2K3JvjdB7x";
const TARGET_COLLATERAL = "CJUyV594JzJpK2BUakNpm6NbmCkhQoPJWkKjfKTvxJ3C";

function inferSym(priceUsd: number): string {
  if (priceUsd > 50000) return "BTC";
  if (priceUsd > 2000) return "ETH";
  if (priceUsd > 50) return "SOL";
  return "UNKNOWN";
}

async function main() {
  const markets = await discoverMarkets(conn, MEDIUM);
  console.log(`Medium program: ${markets.length} markets\n`);
  
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
    
    // Only show markets with right collateral that have free slots
    if (collateral === TARGET_COLLATERAL && !isFull && sym === "UNKNOWN") {
      console.log(`⭐ ${sym} ${m.slabAddress.toBase58()}`);
      console.log(`  collateral: ${collateral.slice(0,8)}... ✅ matches`);
      console.log(`  oracleAuth: ${oracleAuth.slice(0,16)}... ${oracleAuth === ADMIN ? '✅ admin!' : '❌'}`);
      console.log(`  price: $${priceUsd} (0=never pushed)`);
      console.log(`  numUsed=${engine.numUsedAccounts} maxAccounts=${params.maxAccounts}`);
      console.log(`  size=${data.length}`);
      console.log();
    } else if (collateral === TARGET_COLLATERAL && !isFull) {
      console.log(`  ${sym} ${m.slabAddress.toBase58().slice(0,16)} numUsed=${engine.numUsedAccounts}/${params.maxAccounts} price=$${priceUsd}`);
    }
  }
}
main().catch(console.error);
