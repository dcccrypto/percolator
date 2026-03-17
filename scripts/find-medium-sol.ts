import { Connection, PublicKey } from "@solana/web3.js";
import * as dotenv from "dotenv";
dotenv.config();
import { detectSlabLayout, parseEngine, parseParams, parseAllAccounts } from "../packages/core/src/solana/slab.js";

const conn = new Connection(process.env.RPC_URL!, "confirmed");
const SOL_MINT = "So11111111111111111111111111111111111111112";
const PROGRAMS = [
  { id: "FwfBKZXbYr4vTK23bMFkbgKq3npJ3MSDxEaKmq9Aj4Qn", name: "medium" },
  { id: "g9msRSV3sJmmE3r5Twn9HuBsxzuuRGTjKCVTKudm9in", name: "large" },
];

function readU16LE(buf: Uint8Array, off: number) { return buf[off] | (buf[off+1] << 8); }
function readU64LE(buf: Uint8Array, off: number): bigint {
  let v = 0n; for (let i = 0; i < 8; i++) v |= BigInt(buf[off+i]) << BigInt(i*8); return v;
}
function readBytes(buf: Uint8Array, off: number, len: number) { return buf.slice(off, off+len); }

async function main() {
  for (const prog of PROGRAMS) {
    console.log(`\n=== ${prog.name} (${prog.id.slice(0,8)}) ===`);
    
    for (const off of [72, 104, 424]) { // try different mint offsets
      const accs = await conn.getProgramAccounts(new PublicKey(prog.id), {
        commitment: "confirmed",
        dataSlice: { offset: off, length: 32 },
      });
      
      for (const { pubkey, account } of accs) {
        try {
          const mint = new PublicKey(new Uint8Array(account.data)).toBase58();
          if (mint === SOL_MINT) {
            const info = await conn.getAccountInfo(pubkey);
            if (!info) continue;
            const data = new Uint8Array(info.data);
            const engine = parseEngine(data);
            const params = parseParams(data);
            const accounts = parseAllAccounts(data);
            const lps = accounts.filter(a => a.account.kind === 1);
            
            const layout = detectSlabLayout(data.length);
            const isFull = Number(engine.numUsedAccounts) >= Number(params.maxAccounts);
            
            // Check free_head
            let freeHead = -1;
            if (layout) {
              const base = layout.engineOff + layout.engineBitmapOff;
              const bitmapEnd = base + layout.bitmapWords * 8;
              let cursor = bitmapEnd + 2; // skip num_used (u16)
              cursor = Math.ceil(cursor / 8) * 8; // align to u64
              cursor += 8; // skip next_account_id
              freeHead = readU16LE(data, cursor);
            }
            
            console.log(`  ${pubkey.toBase58().slice(0,16)}... size=${info.data.length}`);
            console.log(`  numUsed=${engine.numUsedAccounts} maxAccounts=${params.maxAccounts} LPs=${lps.length} isFull=${isFull}`);
            console.log(`  free_head=0x${freeHead.toString(16)} ${freeHead === 0xFFFF ? '⚠️ FULL' : ''}`);
            console.log(`  newAccountFee=${params.newAccountFee}`);
            
            // Check recent failed txs
            const sigs = await conn.getSignaturesForAddress(pubkey, { limit: 5 });
            const failed = sigs.filter(s => s.err);
            if (failed.length > 0) {
              console.log(`  FAILED TXs: ${failed.map(f => JSON.stringify(f.err)).join(", ")}`);
            }
            console.log();
          }
        } catch {}
      }
    }
  }
}
main().catch(console.error);
