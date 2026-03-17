/**
 * PERC-807: Read free_head from engine to detect if it's 0xFFFF (alloc_slot overflow)
 * For V0 small slab (62808 bytes), BITMAP_WORDS=4, so bitmap = 4*8=32 bytes
 * After bitmap: num_used(2) + next_account_id(8) + free_head(2) = 12 bytes
 * Bitmap offset in engine = ? Need to count from start of RiskEngine struct.
 */
import { Connection, PublicKey } from "@solana/web3.js";
import * as dotenv from "dotenv";
dotenv.config();

const conn = new Connection(process.env.RPC_URL!, "confirmed");
const PROGRAM_ID = process.env.PROGRAM_ID ?? "FxfD37s1AZTeWfFQps9Zpebi2dNQ9QSSDtfMKdbsfKrD";

// V0 layout constants (62808 byte slab, small program MAX_ACCOUNTS=256, BITMAP_WORDS=4)
const V0_ENGINE_OFF = 480; // align_up(72 + 408, 8) = 480

// RiskEngine fields sizes (repr(C)):
// vault: U128 = 16
// insurance_fund: ~ (balance: U128 + fee_revenue: U128 + isolated: U128 + bps: u16 + padding)
// Let me estimate: 16+16+16+2+6pad = 56? 
// Actually let me just find the bitmap offset from the TypeScript layout data.

import { detectSlabLayout, parseEngine, parseParams, parseAllAccounts } from "../packages/core/src/solana/slab.js";

const SOL_SLABS_62808 = [
  "HrdveBrbepjvwAn2qmCPU9eRSFG6Munpkw7gXCHvLpBN",
  "9R6iRUH6Aeo353nXDSAGZuQ7BNXdNXcVXPEEVUhSecWL",
  "BbPEHRVBDWQrW6Y12uMDTpHNc8FfKsucKNSTiXfemgsN",
  "8MsKdp47Q2zeQeSBLf9gcYV7Gx3J7UdrydgCavZvTa4K",
  "DD9Ym1xSGbnCYrfZnpNvSp3JmDHVMiajzdJHz8rUbwJR",
];

function readU16LE(buf: Uint8Array, off: number): number {
  return buf[off] | (buf[off+1] << 8);
}
function readU64LE(buf: Uint8Array, off: number): bigint {
  let val = 0n;
  for (let i = 0; i < 8; i++) val |= BigInt(buf[off+i]) << BigInt(i*8);
  return val;
}

async function main() {
  for (const addr of SOL_SLABS_62808) {
    const info = await conn.getAccountInfo(new PublicKey(addr));
    if (!info) { console.log(`${addr.slice(0,12)}: NOT FOUND`); continue; }
    const data = new Uint8Array(info.data);
    
    const layout = detectSlabLayout(data.length);
    if (!layout) { console.log(`${addr.slice(0,12)}: unknown layout size=${data.length}`); continue; }
    
    console.log(`${addr.slice(0,12)}... size=${data.length}`);
    console.log(`  layout: engineOff=${layout.engineOff} bitmapOff=${layout.engineBitmapOff} bitmapWords=${layout.bitmapWords}`);
    
    // Read free_head and num_used_accounts from slab
    const engine = parseEngine(data);
    const params = parseParams(data);
    const accounts = parseAllAccounts(data);
    const lps = accounts.filter(a => a.account.kind === 1);
    
    // free_head is 2 bytes, located right after num_used(2) + padding + next_account_id(8)
    // In the Rust struct after bitmap: num_used_accounts(u16, 2b) + next_account_id(u64, 8b) + free_head(u16, 2b)
    // But need alignment: num_used(u16,2), then pad(6), then next_account_id(u64,8), then free_head(u16,2)
    const base = layout.engineOff + layout.engineBitmapOff;
    const bitmapEnd = base + layout.bitmapWords * 8;
    
    // After bitmap: num_used (u16, 2b), then alignment to u64 (6 pad), next_account_id (u64, 8b), free_head (u16, 2b)
    let cursor = bitmapEnd;
    const numUsedRaw = readU16LE(data, cursor);
    cursor += 2;
    // pad to next u64 (8-byte) boundary
    cursor = Math.ceil(cursor / 8) * 8;
    const nextAccountIdRaw = readU64LE(data, cursor);
    cursor += 8;
    const freeHeadRaw = readU16LE(data, cursor);
    
    console.log(`  numUsedAccounts=${numUsedRaw} (SDK: ${engine.numUsedAccounts})`);
    console.log(`  nextAccountId=${nextAccountIdRaw} (SDK: ${engine.nextAccountId})`);
    console.log(`  free_head=${freeHeadRaw} (0x${freeHeadRaw.toString(16)}) ${freeHeadRaw === 0xFFFF ? '⚠️ SLAB FULL (alloc_slot overflow)' : ''}`);
    console.log(`  maxAccounts=${params.maxAccounts} LPs=${lps.length}`);
    console.log();
  }
}
main().catch(console.error);
