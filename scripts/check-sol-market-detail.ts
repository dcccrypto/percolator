import { Connection, PublicKey } from "@solana/web3.js";
import * as dotenv from "dotenv";
dotenv.config();
import { parseConfig, parseEngine, parseParams, parseAllAccounts } from "../packages/core/src/solana/slab.js";

const conn = new Connection(process.env.RPC_URL!, "confirmed");

// The FULL SOL slab on medium program + candidate replacements
const SLABS = [
  { addr: "GGU89iQLmceyXRDK5wEFbq9TrAd8E9hmBnkiHSqZa6C2", label: "SOL FULL (medium)" },
  { addr: "Bc7A4yCa2SpaBCLCc4muoMHy8h8JpFPr2M53iUMvKxH9", label: "SOL (small, 2/50)" },
  { addr: "456J6cLyseXLqsDLsz8yX9LpX2TDsK7GS3hYDhA9sBbm", label: "SOL (small, 2/50)" },
  { addr: "EkQty1LsYs4hx17ZCZ6md7u3sksGxzdVR1aw2RJnxFG2", label: "SOL (small, 1/50)" },
  { addr: "CkcwQtUuPe1MjeVh2HKfjZYmCiYR8UE7KrSFwFqmHoJL", label: "BTC (medium, 248/256) - working" },
];

async function main() {
  for (const { addr, label } of SLABS) {
    try {
      const info = await conn.getAccountInfo(new PublicKey(addr));
      if (!info) { console.log(`${label}: NOT FOUND`); continue; }
      const data = new Uint8Array(info.data);
      const config = parseConfig(data);
      const engine = parseEngine(data);
      const params = parseParams(data);
      const accounts = parseAllAccounts(data);
      const lps = accounts.filter(a => a.account.kind === 1);
      
      const priceUsd = Number(config.authorityPriceE6) / 1_000_000;
      const isFull = Number(engine.numUsedAccounts) >= Number(params.maxAccounts);
      
      console.log(`\n${label}: ${addr.slice(0,16)}...`);
      console.log(`  collateral: ${config.collateralMint.toBase58()}`);
      console.log(`  oracleMode: ${config.oracleMode} priceUsd: $${priceUsd.toFixed(2)}`);
      console.log(`  numUsed=${engine.numUsedAccounts} maxAccounts=${params.maxAccounts} LPs=${lps.length}`);
      console.log(`  size=${data.length} FULL=${isFull}`);
    } catch(e: any) {
      console.log(`${label}: ERROR ${e.message}`);
    }
  }
}
main().catch(console.error);
