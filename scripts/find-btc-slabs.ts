import { Connection, PublicKey } from '@solana/web3.js';
import * as dotenv from 'dotenv';
dotenv.config();

const BTC_MINT = 'CJUyV594JzJpK2BUakNpm6NbmCkhQoPJWkKjfKTvxJ3C';
const PROGRAMS = [
  'FxfD37s1AZTeWfFQps9Zpebi2dNQ9QSSDtfMKdbsfKrD',
  'FwfBKZXbYr4vTK23bMFkbgKq3npJ3MSDxEaKmq9Aj4Qn',
  'g9msRSV3sJmmE3r5Twn9HuBsxzuuRGTjKCVTKudm9in',
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
          if (mint === BTC_MINT) {
            const info = await conn.getAccountInfo(pubkey);
            console.log(`BTC market: ${pubkey.toBase58()} size=${info?.data.length} prog=${progId.slice(0,8)} offset=${offset}`);
          }
        } catch {}
      }
    }
  }
  console.log('Done');
}
main().catch(console.error);
