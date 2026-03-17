/**
 * PERC-586: Transfer testUsdcMint mint authority to DEVNET_MINT_AUTHORITY_KEYPAIR
 *
 * Transfers the mint authority of testUsdcMint from the current on-chain authority
 * (DHd11N5JVQmGdMBWf6Mnu1daFGn8j3ChCHwwYAcseD5N) to the DEVNET_MINT_AUTHORITY_KEYPAIR
 * (GRMMNsNPM1GbgxFh3S34f3jvUX6jPbPiH3oxopnDFiWM).
 *
 * REQUIREMENTS:
 *   1. OLD_AUTHORITY_KEYPAIR env var OR --keypair flag pointing to the JSON keypair
 *      for DHd11N5JVQmGdMBWf6Mnu1daFGn8j3ChCHwwYAcseD5N (the current on-chain authority)
 *   2. DEVNET_MINT_AUTHORITY_KEYPAIR env var (or use ~/.config/solana/percolator-devnet-mint-authority.json)
 *
 * Usage:
 *   OLD_AUTHORITY_KEYPAIR='/path/to/old-authority.json' pnpm tsx scripts/transfer-usdc-mint-authority.ts
 *   OLD_AUTHORITY_KEYPAIR='[1,2,3,...]' pnpm tsx scripts/transfer-usdc-mint-authority.ts --dry-run
 *
 * What it does:
 *   1. Reads the current on-chain mint authority for testUsdcMint
 *   2. Verifies OLD_AUTHORITY_KEYPAIR matches the current authority
 *   3. Calls setAuthority to transfer mint authority to DEVNET_MINT_AUTHORITY_KEYPAIR pubkey
 *   4. Verifies the transfer on-chain
 *   5. Runs a smoke test: mint 1 token to a temp ATA to confirm faucet path works
 */
import * as fs from "fs";
import * as path from "path";
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import {
  createSetAuthorityInstruction,
  AuthorityType,
  getAssociatedTokenAddress,
  createAssociatedTokenAccountInstruction,
  createMintToInstruction,
  getAccount,
} from "@solana/spl-token";

// ─── Config ──────────────────────────────────────────────────────────────────

const TEST_USDC_MINT =
  process.env.NEXT_PUBLIC_TEST_USDC_MINT ?? "DvH13uxzTzo1xVFwkbJ6YASkZWs6bm3vFDH4xu7kUYTs";

const EXPECTED_OLD_AUTHORITY = "DHd11N5JVQmGdMBWf6Mnu1daFGn8j3ChCHwwYAcseD5N";
const EXPECTED_NEW_AUTHORITY = "GRMMNsNPM1GbgxFh3S34f3jvUX6jPbPiH3oxopnDFiWM";

const RPC = process.env.RPC_URL ?? "https://api.devnet.solana.com";
const DRY_RUN = process.argv.includes("--dry-run");

// ─── Keypair loading ──────────────────────────────────────────────────────────

function loadKeypair(source: string): Keypair {
  let raw: string;
  if (fs.existsSync(source)) {
    raw = fs.readFileSync(source, "utf8");
  } else {
    raw = source; // treat as inline JSON or env value
  }
  const parsed = JSON.parse(raw.trim());
  if (!Array.isArray(parsed) || parsed.length !== 64) {
    throw new Error(`Invalid keypair: expected 64-byte array, got ${parsed.length} items`);
  }
  return Keypair.fromSecretKey(Uint8Array.from(parsed));
}

function loadOldAuthority(): Keypair {
  const envVal = process.env.OLD_AUTHORITY_KEYPAIR;
  if (envVal) {
    console.log(`Loading old authority from OLD_AUTHORITY_KEYPAIR env var`);
    return loadKeypair(envVal);
  }
  // Look for any --keypair flag
  const kpIdx = process.argv.indexOf("--keypair");
  if (kpIdx !== -1 && process.argv[kpIdx + 1]) {
    const p = process.argv[kpIdx + 1];
    console.log(`Loading old authority from --keypair ${p}`);
    return loadKeypair(p);
  }
  throw new Error(
    "❌ Old authority keypair not found.\n" +
    "Set OLD_AUTHORITY_KEYPAIR env var to the JSON array or path to:\n" +
    `  ${EXPECTED_OLD_AUTHORITY}`,
  );
}

function loadNewAuthority(): Keypair {
  // Try env var first
  const envVal = process.env.DEVNET_MINT_AUTHORITY_KEYPAIR;
  if (envVal) {
    console.log(`Loading new authority from DEVNET_MINT_AUTHORITY_KEYPAIR env var`);
    return loadKeypair(envVal);
  }
  // Fall back to local file
  const localPath = path.join(
    process.env.HOME!,
    ".config/solana/percolator-devnet-mint-authority.json",
  );
  if (fs.existsSync(localPath)) {
    console.log(`Loading new authority from ${localPath}`);
    return loadKeypair(fs.readFileSync(localPath, "utf8"));
  }
  throw new Error(
    "❌ New authority (DEVNET_MINT_AUTHORITY_KEYPAIR) not found.\n" +
    "Set DEVNET_MINT_AUTHORITY_KEYPAIR env var or place keypair at:\n" +
    `  ${localPath}`,
  );
}

// ─── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  console.log(`\n=== PERC-586: testUsdcMint Authority Transfer ===`);
  console.log(`DRY RUN: ${DRY_RUN}`);
  console.log(`Mint    : ${TEST_USDC_MINT}`);
  console.log(`RPC     : ${RPC}`);
  console.log(`---`);

  const conn = new Connection(RPC, "confirmed");
  const mint = new PublicKey(TEST_USDC_MINT);

  // 1. Load keypairs
  const oldAuth = loadOldAuthority();
  const newAuth = loadNewAuthority();

  console.log(`Old authority keypair pubkey: ${oldAuth.publicKey.toBase58()}`);
  console.log(`New authority keypair pubkey: ${newAuth.publicKey.toBase58()}`);

  // 2. Read on-chain mint authority
  const mintInfo = await conn.getAccountInfo(mint);
  if (!mintInfo) {
    throw new Error(`❌ testUsdcMint ${TEST_USDC_MINT} not found on devnet`);
  }

  const data = new Uint8Array(mintInfo.data);
  const hasMintAuthority =
    new DataView(data.buffer, data.byteOffset).getUint32(0, true) === 1;

  if (!hasMintAuthority) {
    throw new Error(`❌ testUsdcMint has no mint authority (fixed supply) — cannot transfer`);
  }

  const onChainAuthority = new PublicKey(data.slice(4, 36));
  console.log(`\nOn-chain mint authority : ${onChainAuthority.toBase58()}`);

  // 3. Verify old authority matches
  if (!onChainAuthority.equals(oldAuth.publicKey)) {
    throw new Error(
      `❌ OLD_AUTHORITY_KEYPAIR pubkey (${oldAuth.publicKey.toBase58()}) does NOT match ` +
      `on-chain authority (${onChainAuthority.toBase58()}).\n` +
      `You need the keypair for: ${onChainAuthority.toBase58()}`,
    );
  }
  console.log(`✅ Old authority matches on-chain — transfer is possible`);

  // 4. Check if already done
  if (onChainAuthority.equals(newAuth.publicKey)) {
    console.log(`\n✅ Authority already is ${newAuth.publicKey.toBase58()} — nothing to do`);
    return;
  }

  if (DRY_RUN) {
    console.log(`\n[DRY RUN] Would transfer authority from ${oldAuth.publicKey.toBase58()} → ${newAuth.publicKey.toBase58()}`);
    return;
  }

  // 5. Transfer authority
  console.log(`\nTransferring mint authority...`);
  const { blockhash, lastValidBlockHeight } = await conn.getLatestBlockhash("confirmed");

  const tx = new Transaction();
  tx.recentBlockhash = blockhash;
  tx.feePayer = oldAuth.publicKey;

  tx.add(
    createSetAuthorityInstruction(
      mint,                                // mint
      oldAuth.publicKey,                   // current authority
      AuthorityType.MintTokens,            // authority type
      newAuth.publicKey,                   // new authority
    ),
  );

  tx.sign(oldAuth);

  const sig = await conn.sendRawTransaction(tx.serialize());
  console.log(`Sent tx: ${sig}`);
  await conn.confirmTransaction({ signature: sig, blockhash, lastValidBlockHeight }, "confirmed");
  console.log(`✅ Authority transferred on-chain: ${sig}`);

  // 6. Verify on-chain
  const postMintInfo = await conn.getAccountInfo(mint);
  const postData = new Uint8Array(postMintInfo!.data);
  const newOnChainAuthority = new PublicKey(postData.slice(4, 36)).toBase58();
  console.log(`Post-transfer on-chain authority: ${newOnChainAuthority}`);

  if (newOnChainAuthority !== newAuth.publicKey.toBase58()) {
    throw new Error(`❌ Transfer failed — on-chain authority did not update`);
  }
  console.log(`✅ On-chain authority confirmed`);

  // 7. Smoke test: mint 1 token to a temp ATA
  console.log(`\nSmoke test: minting 1 token to new authority ATA...`);
  const ata = await getAssociatedTokenAddress(mint, newAuth.publicKey);
  const smokeTx = new Transaction();
  const { blockhash: bh2, lastValidBlockHeight: lvbh2 } = await conn.getLatestBlockhash("confirmed");
  smokeTx.recentBlockhash = bh2;
  smokeTx.feePayer = newAuth.publicKey;

  // Create ATA if needed
  try {
    await getAccount(conn, ata);
  } catch {
    smokeTx.add(
      createAssociatedTokenAccountInstruction(newAuth.publicKey, ata, newAuth.publicKey, mint),
    );
  }

  smokeTx.add(
    createMintToInstruction(mint, ata, newAuth.publicKey, 1_000_000), // 1 USDC (6 decimals)
  );
  smokeTx.sign(newAuth);

  const smokeSig = await conn.sendRawTransaction(smokeTx.serialize());
  await conn.confirmTransaction({ signature: smokeSig, blockhash: bh2, lastValidBlockHeight: lvbh2 }, "confirmed");
  console.log(`✅ Smoke test passed — minted 1 USDC: ${smokeSig}`);

  console.log(`\n=== Transfer complete ===`);
  console.log(`testUsdcMint (${TEST_USDC_MINT}) authority:`);
  console.log(`  Before: DHd11N5JVQmGdMBWf6Mnu1daFGn8j3ChCHwwYAcseD5N`);
  console.log(`  After : ${newAuth.publicKey.toBase58()}`);
  console.log(`\n✅ Faucet USDC minting should now work end-to-end`);
}

main().catch((e) => {
  console.error(`\n❌ Error: ${e.message}`);
  process.exit(1);
});
