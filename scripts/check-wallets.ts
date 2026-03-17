import { Connection, PublicKey, Keypair } from "@solana/web3.js";
import * as fs from "fs";
import * as dotenv from "dotenv";
dotenv.config();
const conn = new Connection(process.env.RPC_URL!, "confirmed");
const paths = [
  "~/.config/solana/percolator-devnet-mint-authority.json",
  "~/.config/solana/percolator-upgrade-authority.json",
];
async function main() {
  for (const p of paths) {
    const resolved = p.replace("~", process.env.HOME!);
    const kp = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(resolved, "utf-8"))));
    const bal = await conn.getBalance(kp.publicKey);
    console.log(`${p.split("/").pop()}: ${kp.publicKey.toBase58().slice(0,16)} → ${bal/1e9} SOL`);
  }
}
main().catch(console.error);
