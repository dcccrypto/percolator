/**
 * PERC-586 FALLBACK: Recreate testUsdcMint with correct mint authority
 *
 * ⚠️  NUCLEAR OPTION — only run if the original keypair for
 *     DHd11N5JVQmGdMBWf6Mnu1daFGn8j3ChCHwwYAcseD5N is truly unrecoverable.
 *
 * What it does:
 *   1. Creates a brand-new SPL token mint on devnet with 6 decimals
 *   2. Sets DEVNET_MINT_AUTHORITY_KEYPAIR as the mint authority (and freeze authority)
 *   3. Mints 1 USDC smoke test to confirm faucet path works
 *   4. Prints all env var updates needed: Vercel, Railway, config.ts fallback
 *
 * Requirements:
 *   - DEVNET_MINT_AUTHORITY_KEYPAIR env var (or ~/.config/solana/percolator-devnet-mint-authority.json)
 *
 * Usage:
 *   pnpm tsx scripts/recreate-usdc-mint.ts [--dry-run]
 *
 * After running:
 *   1. Update NEXT_PUBLIC_TEST_USDC_MINT in Vercel (prod + preview)
 *   2. Update NEXT_PUBLIC_TEST_USDC_MINT in Railway (all services that use it)
 *   3. Update hardcoded fallback in app/lib/config.ts
 *   4. Re-initialize devnet markets that list USDC as the settlement token (if any)
 *   5. Redeploy app + services
 */

import * as fs from "fs";
import * as path from "path";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  createInitializeMintInstruction,
  getMinimumBalanceForRentExemptMint,
  MINT_SIZE,
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddress,
  createAssociatedTokenAccountInstruction,
  createMintToInstruction,
  getAccount,
  getMint,
} from "@solana/spl-token";

// ─── Config ───────────────────────────────────────────────────────────────────

const DECIMALS = 6;
const RPC = process.env.RPC_URL ?? "https://api.devnet.solana.com";
const DRY_RUN = process.argv.includes("--dry-run");
const OLD_MINT = "DvH13uxzTzo1xVFwkbJ6YASkZWs6bm3vFDH4xu7kUYTs";
const EXPECTED_NEW_AUTHORITY = "GRMMNsNPM1GbgxFh3S34f3jvUX6jPbPiH3oxopnDFiWM";

// ─── Keypair loading ──────────────────────────────────────────────────────────

function loadKeypair(source: string): Keypair {
  let raw: string;
  if (fs.existsSync(source)) {
    raw = fs.readFileSync(source, "utf8");
  } else {
    raw = source;
  }
  const parsed = JSON.parse(raw.trim());
  if (!Array.isArray(parsed) || parsed.length !== 64) {
    throw new Error(
      `Invalid keypair: expected 64-byte array, got ${parsed.length} items`,
    );
  }
  return Keypair.fromSecretKey(Uint8Array.from(parsed));
}

function loadMintAuthority(): Keypair {
  const envVal = process.env.DEVNET_MINT_AUTHORITY_KEYPAIR;
  if (envVal) {
    console.log(`Loading mint authority from DEVNET_MINT_AUTHORITY_KEYPAIR env var`);
    return loadKeypair(envVal);
  }
  const localPath = path.join(
    process.env.HOME!,
    ".config/solana/percolator-devnet-mint-authority.json",
  );
  if (fs.existsSync(localPath)) {
    console.log(`Loading mint authority from ${localPath}`);
    return loadKeypair(fs.readFileSync(localPath, "utf8"));
  }
  throw new Error(
    "❌ DEVNET_MINT_AUTHORITY_KEYPAIR not found.\n" +
      "Set the env var or place keypair at ~/.config/solana/percolator-devnet-mint-authority.json",
  );
}

// ─── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  console.log(`\n=== PERC-586 FALLBACK: Recreate testUsdcMint ===`);
  console.log(`⚠️  This creates a NEW mint. The old one (${OLD_MINT}) will be abandoned.`);
  console.log(`DRY RUN : ${DRY_RUN}`);
  console.log(`RPC     : ${RPC}`);
  console.log(`Decimals: ${DECIMALS}`);
  console.log(`---`);

  const mintAuthority = loadMintAuthority();
  console.log(`Mint authority: ${mintAuthority.publicKey.toBase58()}`);

  if (mintAuthority.publicKey.toBase58() !== EXPECTED_NEW_AUTHORITY) {
    console.warn(
      `⚠️  Loaded keypair pubkey (${mintAuthority.publicKey.toBase58()}) ` +
        `does not match expected DEVNET_MINT_AUTHORITY_KEYPAIR (${EXPECTED_NEW_AUTHORITY}).` +
        `\nContinuing anyway — verify this is intentional.`,
    );
  }

  if (DRY_RUN) {
    console.log(`\n[DRY RUN] Would create new SPL token mint with:`);
    console.log(`  decimals      : ${DECIMALS}`);
    console.log(`  mintAuthority : ${mintAuthority.publicKey.toBase58()}`);
    console.log(`  freezeAuthority: ${mintAuthority.publicKey.toBase58()}`);
    console.log(`\n[DRY RUN] No transactions sent.`);
    printNextSteps("<NEW_MINT_ADDRESS>");
    return;
  }

  const conn = new Connection(RPC, "confirmed");

  // 1. Check payer balance
  const balance = await conn.getBalance(mintAuthority.publicKey);
  console.log(`\nPayer balance: ${(balance / 1e9).toFixed(4)} SOL`);
  const rentExempt = await getMinimumBalanceForRentExemptMint(conn);
  console.log(`Rent exempt  : ${(rentExempt / 1e9).toFixed(6)} SOL`);

  if (balance < rentExempt + 5000) {
    throw new Error(
      `❌ Insufficient SOL — need at least ${((rentExempt + 5000) / 1e9).toFixed(6)} SOL to create mint`,
    );
  }

  // 2. Generate new mint keypair
  const newMintKeypair = Keypair.generate();
  console.log(`\nNew mint keypair: ${newMintKeypair.publicKey.toBase58()}`);
  console.log(`Saving to /tmp/new-testUsdcMint.json for backup...`);
  fs.writeFileSync(
    "/tmp/new-testUsdcMint.json",
    JSON.stringify(Array.from(newMintKeypair.secretKey)),
  );

  // 3. Create mint account + initialize
  console.log(`\nCreating mint on devnet...`);
  const { blockhash, lastValidBlockHeight } = await conn.getLatestBlockhash("confirmed");

  const tx = new Transaction({ recentBlockhash: blockhash, feePayer: mintAuthority.publicKey });

  tx.add(
    SystemProgram.createAccount({
      fromPubkey: mintAuthority.publicKey,
      newAccountPubkey: newMintKeypair.publicKey,
      space: MINT_SIZE,
      lamports: rentExempt,
      programId: TOKEN_PROGRAM_ID,
    }),
    createInitializeMintInstruction(
      newMintKeypair.publicKey,    // mint
      DECIMALS,                    // decimals
      mintAuthority.publicKey,     // mintAuthority
      mintAuthority.publicKey,     // freezeAuthority
      TOKEN_PROGRAM_ID,
    ),
  );

  const sig = await sendAndConfirmTransaction(conn, tx, [mintAuthority, newMintKeypair], {
    commitment: "confirmed",
  });
  console.log(`✅ Mint created: ${sig}`);

  const newMintAddress = newMintKeypair.publicKey.toBase58();
  console.log(`\nNew testUsdcMint address: ${newMintAddress}`);

  // 4. Verify on-chain
  const mintInfo = await getMint(conn, newMintKeypair.publicKey, "confirmed");
  if (
    mintInfo.mintAuthority?.toBase58() !== mintAuthority.publicKey.toBase58()
  ) {
    throw new Error(
      `❌ Mint created but mintAuthority mismatch: ${mintInfo.mintAuthority?.toBase58()}`,
    );
  }
  if (mintInfo.decimals !== DECIMALS) {
    throw new Error(`❌ Decimals mismatch: ${mintInfo.decimals}`);
  }
  console.log(`✅ On-chain verified — mintAuthority=${mintInfo.mintAuthority?.toBase58()}, decimals=${mintInfo.decimals}`);

  // 5. Smoke test: mint 1 USDC to authority wallet
  console.log(`\nSmoke test: minting 1 USDC to mint authority ATA...`);
  const ata = await getAssociatedTokenAddress(newMintKeypair.publicKey, mintAuthority.publicKey);
  const { blockhash: bh2, lastValidBlockHeight: lvbh2 } = await conn.getLatestBlockhash("confirmed");

  const smokeTx = new Transaction({ recentBlockhash: bh2, feePayer: mintAuthority.publicKey });

  // Create ATA if needed
  try {
    await getAccount(conn, ata);
  } catch {
    smokeTx.add(
      createAssociatedTokenAccountInstruction(
        mintAuthority.publicKey,
        ata,
        mintAuthority.publicKey,
        newMintKeypair.publicKey,
      ),
    );
  }

  smokeTx.add(
    createMintToInstruction(
      newMintKeypair.publicKey,
      ata,
      mintAuthority.publicKey,
      1_000_000, // 1 USDC (6 decimals)
    ),
  );

  const smokeSig = await sendAndConfirmTransaction(conn, smokeTx, [mintAuthority], {
    commitment: "confirmed",
  });
  console.log(`✅ Smoke test passed — minted 1 USDC: ${smokeSig}`);

  // 6. Print next steps
  printNextSteps(newMintAddress);

  console.log(`\n✅ New testUsdcMint keypair backed up to: /tmp/new-testUsdcMint.json`);
  console.log(`   Store this somewhere safe — you'll need it if authority transfer is needed again.`);
}

function printNextSteps(newMint: string) {
  console.log(`\n${"=".repeat(60)}`);
  console.log(`NEXT STEPS — update env vars everywhere:`);
  console.log(`${"=".repeat(60)}`);
  console.log(`\n1. Vercel (prod + preview):`);
  console.log(`   NEXT_PUBLIC_TEST_USDC_MINT=${newMint}`);
  console.log(`\n2. Railway (percolator-api, percolator-indexer, oracle-keeper):`);
  console.log(`   NEXT_PUBLIC_TEST_USDC_MINT=${newMint}`);
  console.log(`   TEST_USDC_MINT=${newMint}`);
  console.log(`\n3. Update hardcoded fallback in app/lib/config.ts:`);
  console.log(`   testUsdcMint: process.env.NEXT_PUBLIC_TEST_USDC_MINT ?? "${newMint}",`);
  console.log(`\n4. Check if any devnet markets have testUsdcMint as settlement token`);
  console.log(`   and re-initialize those markets on devnet.`);
  console.log(`\n5. Redeploy all services after env var updates.`);
  console.log(`\n6. Run faucet smoke test:`);
  console.log(`   curl -X POST https://<your-app>/api/faucet -d '{"wallet":"<your-wallet>","type":"usdc"}'`);
  console.log(`${"=".repeat(60)}\n`);
}

main().catch((e) => {
  console.error(`\n❌ Error: ${e.message}`);
  process.exit(1);
});
