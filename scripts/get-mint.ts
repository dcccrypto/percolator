import { Connection, PublicKey } from "@solana/web3.js";
import * as dotenv from "dotenv";
dotenv.config();
import { parseConfig } from "../packages/core/src/solana/slab.js";

const conn = new Connection(process.env.RPC_URL!, "confirmed");
const MARKETS = [
  { addr: "GGU89iQLmceyXRDK8vgAxVvdi9RJb9JsPhXZ2NoFSENV", label: "SOL FULL" },
  { addr: "CkcwQtUuPe1MjeVhyMR2zZcLsKEzP2cqGzspwmgTuZRp", label: "BTC working" },
];

async function main() {
  for (const { addr, label } of MARKETS) {
    const info = await conn.getAccountInfo(new PublicKey(addr));
    if (!info) { console.log(`${label}: NOT FOUND`); continue; }
    const data = new Uint8Array(info.data);
    const config = parseConfig(data);
    console.log(`${label} (${addr.slice(0,8)}...):`);
    console.log(`  collateralMint: ${config.collateralMint.toBase58()}`);
    console.log(`  oracleAuthority: ${config.oracleAuthority?.toBase58?.() ?? JSON.stringify(config.oracleAuthority)}`);
    console.log(`  authorityPriceE6: ${config.authorityPriceE6} (~$${Number(config.authorityPriceE6)/1e6})`);
    console.log(`  oracleMode: ${config.oracleMode}`);
  }
}
main().catch(console.error);
