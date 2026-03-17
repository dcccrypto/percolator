import { Connection, PublicKey } from '@solana/web3.js';
import * as dotenv from 'dotenv';
dotenv.config();

const BTC_SLABS = [
  'AB3ZN1vxbBEh8FZRfrL55QQUUaLCwawqvCYzTDpgbuLF',
  'CkcwQtUuPe1MjeVhyMR2zZcLsKEzP2cqGzspwmgTuZRp',
  'GGU89iQLmceyXRDK8vgAxVvdi9RJb9JsPhXZ2NoFSENV',
  '7eubYRwJiQdJgXsw1VdaNQ7YHvHbgChe7wbPNQw74S23',
];

const ENGINE_OFF = 640;
const BITMAP_OFF = ENGINE_OFF + 656; // ENGINE_FIXED=656 bytes (PERC-299 scalars)
const NUM_USED_OFF = BITMAP_OFF + 32 + 2; // bitmap=32 + free_head=2

async function main() {
  const conn = new Connection(process.env.RPC_URL!, 'confirmed');
  
  for (const slab of BTC_SLABS) {
    const info = await conn.getAccountInfo(new PublicKey(slab), 'confirmed');
    if (!info) { console.log(slab.slice(0,8), '→ NOT FOUND'); continue; }
    
    const d = new DataView(info.data.buffer, info.data.byteOffset);
    
    // num_used_accounts at ENGINE_OFF + 656 + bitmap_bytes + 2 (free_head)
    // bitmap for 256 accounts = ceil(256/64)*8 = 32 bytes
    const numUsed = d.getUint16(NUM_USED_OFF, true);
    
    // Check if RESOLVED (flag in header at byte 104)
    const resolvedFlag = info.data[104]; // check resolved byte  
    
    // vault at ENGINE_OFF (u128 LE = 16 bytes)
    const vaultLo = d.getBigUint64(ENGINE_OFF, true);
    const vaultHi = d.getBigUint64(ENGINE_OFF + 8, true);
    const vault = (vaultHi << 64n) | vaultLo;
    
    console.log(`${slab.slice(0,8)}: size=${info.data.length} numUsed=${numUsed} vault=${vault} resolved=${resolvedFlag}`);
  }
}
main().catch(console.error);
