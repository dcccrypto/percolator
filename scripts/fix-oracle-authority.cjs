/**
 * PERC-387: Fix oracle_authority mismatch on BTC-PERP-1 and BTC-PERP-2
 *
 * Both slabs have oracle_authority=11obSVaVR4k4... but the oracle-keeper
 * wallet is FF7KFfU5Bb3... — this script calls SetOracleAuthority to fix.
 *
 * Admin keypair: ~/.config/solana/percolator-upgrade-authority.json (FF7KFfU5...)
 * Program: FwfBKZXbYr4vTK23bMFkbgKq3npJ3MSDxEaKmq9Aj4Qn (Small)
 */

const { Connection, PublicKey, Keypair, Transaction, ComputeBudgetProgram, sendAndConfirmTransaction } = require('@solana/web3.js');
const fs = require('fs');
const path = require('path');

const {
  encodeSetOracleAuthority,
  buildAccountMetas,
  ACCOUNTS_SET_ORACLE_AUTHORITY,
  buildIx,
  parseHeader,
} = require('../packages/core/dist/index.js');

const SMALL_PROGRAM = new PublicKey('FwfBKZXbYr4vTK23bMFkbgKq3npJ3MSDxEaKmq9Aj4Qn');
const RPC_URL = process.env.RPC_URL || 'https://api.devnet.solana.com';

const SLABS = [
  { name: 'BTC-PERP-1', address: '7eubYRwJiQdJgXsw1VdaNQ7YHvHbgChe7wbPNQw74S23' },
  { name: 'BTC-PERP-2', address: 'CkcwQtUuPe1MjeVhyMR2zZcLsKEzP2cqGzspwmgTuZRp' },
];

async function main() {
  const adminPath = process.env.ADMIN_KEYPAIR_PATH ||
    path.join(process.env.HOME, '.config/solana/percolator-upgrade-authority.json');

  const adminKp = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(adminPath, 'utf8')))
  );

  const conn = new Connection(RPC_URL, 'confirmed');
  const dryRun = process.argv.includes('--dry-run');

  console.log('='.repeat(60));
  console.log('PERC-387: Fix oracle_authority mismatch');
  console.log('='.repeat(60));
  console.log('Admin:', adminKp.publicKey.toBase58());
  console.log('New oracle_authority:', adminKp.publicKey.toBase58());
  console.log('Program:', SMALL_PROGRAM.toBase58());
  console.log('Dry run:', dryRun);
  console.log();

  for (const slab of SLABS) {
    const slabPubkey = new PublicKey(slab.address);
    console.log(`--- ${slab.name} (${slab.address}) ---`);

    // Read current state
    const accountInfo = await conn.getAccountInfo(slabPubkey);
    if (!accountInfo) {
      console.log('  ❌ Slab not found on-chain, skipping');
      continue;
    }

    try {
      const header = parseHeader(new Uint8Array(accountInfo.data));
      const currentAuth = header.oracleAuthority
        ? new PublicKey(header.oracleAuthority).toBase58()
        : 'N/A (field not in header)';
      const headerAdmin = header.admin
        ? new PublicKey(header.admin).toBase58()
        : 'N/A';
      console.log('  Current oracle_authority:', currentAuth);
      console.log('  Header admin:', headerAdmin);
    } catch (e) {
      console.log('  ⚠️ Could not parse header (will proceed anyway):', e.message);
    }

    if (dryRun) {
      console.log('  🔍 DRY RUN — would call SetOracleAuthority with newAuthority =', adminKp.publicKey.toBase58());
      console.log();
      continue;
    }

    // Build SetOracleAuthority instruction
    const data = encodeSetOracleAuthority({ newAuthority: adminKp.publicKey.toBase58() });
    const keys = buildAccountMetas(ACCOUNTS_SET_ORACLE_AUTHORITY, [
      adminKp.publicKey,
      slabPubkey,
    ]);

    const tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 50_000 }),
      ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 }),
      buildIx({ programId: SMALL_PROGRAM, keys, data })
    );

    try {
      const sig = await sendAndConfirmTransaction(conn, tx, [adminKp], { commitment: 'confirmed' });
      console.log('  ✅ SetOracleAuthority success:', sig);
      console.log('  Explorer: https://explorer.solana.com/tx/' + sig + '?cluster=devnet');
    } catch (err) {
      console.log('  ❌ SetOracleAuthority failed:', err.message);
      if (err.logs) {
        console.log('  Logs:', err.logs.slice(-5).join('\n    '));
      }
    }
    console.log();
  }

  console.log('Done.');
}

main().catch(err => {
  console.error('Fatal:', err.message);
  process.exit(1);
});
