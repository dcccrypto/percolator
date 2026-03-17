import { Connection, PublicKey } from '@solana/web3.js';
import * as dotenv from 'dotenv';
dotenv.config();

const SOL_MINT = 'So11111111111111111111111111111111111111112';
const PROGRAMS = [
  'FxfD37s1AZTeWfFQps9Zpebi2dNQ9QSSDtfMKdbsfKrD', // small
  'FwfBKZXbYr4vTK23bMFkbgKq3npJ3MSDxEaKmq9Aj4Qn', // medium
  'g9msRSV3sJmmE3r5Twn9HuBsxzuuRGTjKCVTKudm9in',  // large
];

async function main() {
  const conn = new Connection(process.env.RPC_URL!, 'confirmed');

  for (const progId of PROGRAMS) {
    const prog = new PublicKey(progId);
    for (const offset of [72, 104]) {
      const accs = await conn.getProgramAccounts(prog, {
        commitment: 'confirmed',
        dataSlice: { offset, length: 32 },
      });
      for (const {pubkey, account} of accs) {
        try {
          const mint = new PublicKey(new Uint8Array(account.data)).toBase58();
          if (mint === SOL_MINT) {
            const info = await conn.getAccountInfo(pubkey);
            console.log(`SOL market: ${pubkey.toBase58()} size=${info?.data.length} prog=${progId.slice(0,8)} offset=${offset}`);
          }
        } catch {}
      }
    }
  }
  console.log('Done');
}

main().catch(console.error);
