import { Connection, PublicKey, Keypair, SystemProgram, Transaction, sendAndConfirmTransaction, LAMPORTS_PER_SOL } from '@solana/web3.js';
import * as fs from 'fs';
import * as dotenv from 'dotenv';
dotenv.config();

async function main() {
  const conn = new Connection(process.env.RPC_URL!, 'confirmed');
  
  const mintAuthPath = '/Users/khubair/.config/solana/percolator-devnet-mint-authority.json';
  const mintAuth = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(mintAuthPath, 'utf-8'))));
  
  const adminPk = new PublicKey('FF7KFfU5Bb3Mze2AasDHCCZuyhdaSLjUZy2K3JvjdB7x');
  
  const mintBal = await conn.getBalance(mintAuth.publicKey);
  const adminBal = await conn.getBalance(adminPk);
  
  console.log('MintAuth:', mintAuth.publicKey.toBase58(), 'balance:', mintBal / LAMPORTS_PER_SOL, 'SOL');
  console.log('Admin:', adminPk.toBase58(), 'balance:', adminBal / LAMPORTS_PER_SOL, 'SOL');
  
  const transferAmt = mintBal - 0.01 * LAMPORTS_PER_SOL;
  if (transferAmt <= 0) {
    console.log('Not enough SOL in mint-authority to transfer');
    return;
  }
  
  if (mintBal < 0.5 * LAMPORTS_PER_SOL) {
    console.log('Mint authority has < 0.5 SOL, not worth transferring');
    return;
  }
  
  console.log('Transferring', transferAmt / LAMPORTS_PER_SOL, 'SOL from mint-authority to admin...');
  const tx = new Transaction().add(
    SystemProgram.transfer({
      fromPubkey: mintAuth.publicKey,
      toPubkey: adminPk,
      lamports: Math.floor(transferAmt),
    })
  );
  const sig = await sendAndConfirmTransaction(conn, tx, [mintAuth], { commitment: 'confirmed' });
  console.log('Transfer sig:', sig);
  console.log('New admin balance:', await conn.getBalance(adminPk) / LAMPORTS_PER_SOL, 'SOL');
}
main().catch(console.error);
