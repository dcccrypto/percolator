import { Connection, PublicKey } from "@solana/web3.js";
import { getAssociatedTokenAddress, getAccount, TokenAccountNotFoundError } from "@solana/spl-token";
import * as dotenv from "dotenv";
import * as fs from "fs";
import { Keypair } from "@solana/web3.js";
dotenv.config();

const conn = new Connection(process.env.RPC_URL!, "confirmed");
const COLLATERAL = "CJUyV594JzJpK2BUakNpm6NbmCkhQoPJWkKjfKTvxJ3C";

function loadKeypair(path: string) {
  const p = path.startsWith("~/") ? path.replace("~", process.env.HOME!) : path;
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(p, "utf8"))));
}

async function main() {
  const admin = loadKeypair(process.env.ADMIN_KEYPAIR_PATH!);
  console.log(`Admin: ${admin.publicKey.toBase58()}`);
  const bal = await conn.getBalance(admin.publicKey);
  console.log(`SOL balance: ${bal/1e9} SOL`);
  
  try {
    const ata = await getAssociatedTokenAddress(new PublicKey(COLLATERAL), admin.publicKey);
    const acc = await getAccount(conn, ata);
    console.log(`CJUyV594 balance: ${acc.amount} (ATA: ${ata.toBase58().slice(0,12)}...)`);
  } catch(e) {
    console.log(`CJUyV594: No ATA or zero balance (${(e as Error).message.slice(0,50)})`);
  }
}
main().catch(console.error);
