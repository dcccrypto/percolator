#!/usr/bin/env npx tsx
/**
 * Probe correct slab size for Small-tier program by simulating InitMarket
 */
import { Connection, Keypair, PublicKey, SystemProgram, Transaction, ComputeBudgetProgram, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import { createTransferInstruction, getAssociatedTokenAddress, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import * as fs from "fs";
import { encodeInitMarket, ACCOUNTS_INIT_MARKET, buildAccountMetas, buildIx, WELL_KNOWN, deriveVaultAuthority } from "../packages/core/src/index.js";

const conn = new Connection("https://api.devnet.solana.com", "confirmed");
const admin = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(process.env.HOME + "/.config/solana/percolator-upgrade-authority.json", "utf8"))));
const SMALL_PROGRAM = new PublicKey("FwfBKZXbYr4vTK23bMFkbgKq3npJ3MSDxEaKmq9Aj4Qn");
const LARGE_PROGRAM = new PublicKey("FxfD37s1AZTeWfFQps9Zpebi2dNQ9QSSDtfMKdbsfKrD");

const mintInfo = JSON.parse(fs.readFileSync("/tmp/percolator-test-usdc.json", "utf8"));
const mint = new PublicKey(mintInfo.mint);

// Compute candidate sizes for small and large
// BPF (u128 align=8): CONFIG_LEN=496, HEADER=104, ENGINE_OFF=600
// SDK (CONFIG_LEN=536, HEADER=104, ENGINE_OFF=640) — these may be newer
// SBF target_os=solana (u128 align=8 still? or 16?): could differ

function computeSlab(engineOff: number, engineFixed: number, maxAccounts: number, accountSize: number): number {
  const bitmapBytes = Math.ceil(maxAccounts / 64) * 8;
  const postBitmap = 18;
  const nextFreeBytes = maxAccounts * 2;
  const preAccountsLen = engineFixed + bitmapBytes + postBitmap + nextFreeBytes;
  const accountsOff = Math.ceil(preAccountsLen / 8) * 8;
  return engineOff + accountsOff + maxAccounts * accountSize;
}

// Generate candidate sizes by varying CONFIG_LEN and alignments
const candidates: { label: string; size: number; program: PublicKey; maxAccounts: number }[] = [];

// Small tier candidates
for (const configLen of [496, 512, 536]) {
  for (const align of [8, 16]) {
    const engineOff = Math.ceil((104 + configLen) / align) * align;
    for (const engineFixed of [656, 648, 632, 624]) {
      const size = computeSlab(engineOff, engineFixed, 256, 248);
      candidates.push({ label: `Small(CFG=${configLen},AL=${align},EF=${engineFixed})`, size, program: SMALL_PROGRAM, maxAccounts: 256 });
    }
  }
}

// Large tier - also try
for (const configLen of [496, 512, 536]) {
  for (const align of [8, 16]) {
    const engineOff = Math.ceil((104 + configLen) / align) * align;
    for (const engineFixed of [656, 648, 632, 624]) {
      const size = computeSlab(engineOff, engineFixed, 4096, 248);
      candidates.push({ label: `Large(CFG=${configLen},AL=${align},EF=${engineFixed})`, size, program: LARGE_PROGRAM, maxAccounts: 4096 });
    }
  }
}

// Deduplicate
const seen = new Set<string>();
const unique = candidates.filter(c => {
  const key = `${c.program.toBase58()}-${c.size}`;
  if (seen.has(key)) return false;
  seen.add(key);
  return true;
});

console.log(`Testing ${unique.length} candidate sizes...`);
console.log(`Admin: ${admin.publicKey.toBase58()}`);

async function probeSize(candidate: typeof unique[0]): Promise<boolean> {
  const { label, size, program, maxAccounts } = candidate;
  const slabKp = Keypair.generate();
  const slabRent = await conn.getMinimumBalanceForRentExemption(size);
  const [vaultPda] = deriveVaultAuthority(program, slabKp.publicKey);
  const vaultAta = await getAssociatedTokenAddress(mint, vaultPda, true);
  const adminAta = new PublicKey(mintInfo.adminAta);

  // Build a multi-step transaction: createAccount + InitMarket
  // We just SIMULATE, don't actually send
  const tx = new Transaction();
  tx.add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 300_000 }),
    SystemProgram.createAccount({
      fromPubkey: admin.publicKey,
      newAccountPubkey: slabKp.publicKey,
      lamports: slabRent,
      space: size,
      programId: program,
    }),
  );

  // We can't really simulate vault ATA creation + InitMarket in same tx without the vault existing.
  // Instead, simulate JUST the createAccount to verify the size is accepted by system program,
  // then separately simulate InitMarket against an already-created slab.
  
  // Actually, let's just simulate a full InitMarket against a hypothetical slab.
  // The program checks slab.data.len() == SLAB_LEN early.
  const initData = encodeInitMarket({
    admin: admin.publicKey,
    collateralMint: mint,
    indexFeedId: "0".repeat(64),
    maxStalenessSecs: "120",
    confFilterBps: 0,
    invert: 0,
    unitScale: 0,
    initialMarkPriceE6: "130000000",
    warmupPeriodSlots: "0",
    maintenanceMarginBps: "500",
    initialMarginBps: "1000",
    tradingFeeBps: "30",
    maxAccounts: maxAccounts.toString(),
    newAccountFee: "1000000",
    riskReductionThreshold: "0",
    maintenanceFeePerSlot: "0",
    maxCrankStalenessSlots: "200",
    liquidationFeeBps: "100",
    liquidationFeeCap: "0",
    liquidationBufferBps: "50",
    minLiquidationAbs: "0",
  });
  const initKeys = buildAccountMetas(ACCOUNTS_INIT_MARKET, [
    admin.publicKey, slabKp.publicKey, mint, vaultAta,
    WELL_KNOWN.tokenProgram, WELL_KNOWN.clock, WELL_KNOWN.rent,
    vaultPda, WELL_KNOWN.systemProgram,
  ]);
  tx.add(buildIx({ programId: program, keys: initKeys, data: initData }));

  tx.feePayer = admin.publicKey;
  const { blockhash } = await conn.getLatestBlockhash();
  tx.recentBlockhash = blockhash;
  tx.sign(admin, slabKp);

  const sim = await conn.simulateTransaction(tx);
  const err = sim.value.err;
  const logs = sim.value.logs ?? [];
  
  // Check for slab len error (0x24 = 36 = InvalidSlabLen)
  const hasSlabErr = logs.some(l => l.includes("custom program error: 0x24") || l.includes("error: 0x4"));
  const success = !err;
  
  if (!hasSlabErr && err) {
    // Different error — slab size might be correct but something else failed
    const errStr = JSON.stringify(err);
    if (!errStr.includes("0x24")) {
      console.log(`✅ ${label}: size=${size} — SIZE ACCEPTED (other error: ${errStr.slice(0, 60)})`);
      return true;
    }
  }
  
  if (success) {
    console.log(`✅ ${label}: size=${size} — FULL SUCCESS`);
    return true;
  }

  return false;
}

async function main() {
  // Test small tier candidates first
  const smallCandidates = unique.filter(c => c.program === SMALL_PROGRAM);
  console.log(`\n--- Small tier (${smallCandidates.length} candidates) ---`);
  for (const c of smallCandidates) {
    try {
      const ok = await probeSize(c);
      if (ok) break;
    } catch (e: any) {
      // ignore rate limit errors
    }
    await new Promise(r => setTimeout(r, 300));
  }

  // Test large tier candidates
  const largeCandidates = unique.filter(c => c.program === LARGE_PROGRAM);
  console.log(`\n--- Large tier (${largeCandidates.length} candidates) ---`);
  for (const c of largeCandidates) {
    try {
      const ok = await probeSize(c);
      if (ok) break;
    } catch (e: any) {
      // ignore
    }
    await new Promise(r => setTimeout(r, 300));
  }
}

main().catch(console.error);
