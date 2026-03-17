import { Connection, PublicKey, LAMPORTS_PER_SOL } from "@solana/web3.js";
import * as dotenv from "dotenv";
dotenv.config();
const ENDPOINTS = [
  "https://api.devnet.solana.com",
  "https://devnet.rpc.extrnode.com",
  "https://rpc.devnet.soo.network/rpc",
];
const admin = new PublicKey("FF7KFfU5Bb3Mze2AasDHCCZuyhdaSLjUZy2K3JvjdB7x");
async function main() {
  for (const endpoint of ENDPOINTS) {
    const conn = new Connection(endpoint, "confirmed");
    console.log(`Trying: ${endpoint}`);
    try {
      const sig = await conn.requestAirdrop(admin, 2 * LAMPORTS_PER_SOL);
      await conn.confirmTransaction(sig, "confirmed");
      console.log(`✅ Success! sig=${sig.slice(0,20)}...`);
      const bal = await conn.getBalance(admin);
      console.log("New balance:", bal / 1e9, "SOL");
      return;
    } catch(e) {
      console.log(`  Failed: ${(e as Error).message.slice(0,80)}`);
    }
  }
}
main().catch(console.error);
