#!/usr/bin/env npx tsx
/**
 * PERC-613 / GH#1395: Fix devnet USDC faucet authority mismatch.
 *
 * The on-chain mint authority doesn't match DEVNET_MINT_AUTHORITY_KEYPAIR.
 * This script re-assigns the mint authority to the keypair in the env var.
 *
 * Usage:
 *   DEVNET_MINT_AUTHORITY_KEYPAIR='[...]' npx tsx scripts/fix-devnet-usdc-authority.ts
 *
 * Requires:
 *   - The CURRENT on-chain mint authority keypair at ~/.config/solana/id.json
 *     (or pass --current-authority /path/to/keypair.json)
 *   - DEVNET_MINT_AUTHORITY_KEYPAIR env var (the NEW authority)
 */

import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { AuthorityType, setAuthority } from "@solana/spl-token";

const RPC_URL = process.env.RPC_URL || "https://api.devnet.solana.com";
// Devnet test USDC mint — same as in /api/faucet/route.ts
const TEST_USDC_MINT = new PublicKey(
  process.env.NEXT_PUBLIC_TEST_USDC_MINT || "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"
);

async function main() {
  const conn = new Connection(RPC_URL, "confirmed");

  // Load the NEW authority from env
  const newAuthorityJson = process.env.DEVNET_MINT_AUTHORITY_KEYPAIR;
  if (!newAuthorityJson) {
    console.error("❌ DEVNET_MINT_AUTHORITY_KEYPAIR env var not set");
    process.exit(1);
  }
  const newAuthority = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(newAuthorityJson))
  );
  console.log(`New authority: ${newAuthority.publicKey.toBase58()}`);

  // Load the CURRENT authority (whoever currently owns the mint)
  const currentAuthorityPath = process.argv[2] || `${process.env.HOME}/.config/solana/id.json`;
  const currentAuthorityJson = await import("fs").then(fs =>
    fs.readFileSync(currentAuthorityPath, "utf-8")
  );
  const currentAuthority = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(currentAuthorityJson))
  );
  console.log(`Current authority: ${currentAuthority.publicKey.toBase58()}`);

  // Check on-chain mint authority
  const mintInfo = await conn.getAccountInfo(TEST_USDC_MINT);
  if (!mintInfo) {
    console.error("❌ Mint account not found on-chain");
    process.exit(1);
  }
  // SPL Token mint layout: bytes 0-3 coption(u32), bytes 4-35 mint_authority (32 bytes)
  const coption = mintInfo.data.readUInt32LE(0);
  if (coption !== 1) {
    console.error("❌ Mint has no authority (fixed supply, cannot re-key)");
    process.exit(1);
  }
  const onChainAuthority = new PublicKey(mintInfo.data.subarray(4, 36));
  console.log(`On-chain authority: ${onChainAuthority.toBase58()}`);

  if (onChainAuthority.equals(newAuthority.publicKey)) {
    console.log("✅ Authorities already match — nothing to do");
    return;
  }

  if (!onChainAuthority.equals(currentAuthority.publicKey)) {
    console.error(
      `❌ Current keypair (${currentAuthority.publicKey.toBase58().slice(0, 8)}…) ` +
      `doesn't match on-chain authority (${onChainAuthority.toBase58().slice(0, 8)}…). ` +
      `Pass the correct keypair path as argument.`
    );
    process.exit(1);
  }

  console.log(`\nTransferring mint authority: ${currentAuthority.publicKey.toBase58().slice(0, 8)}… → ${newAuthority.publicKey.toBase58().slice(0, 8)}…`);

  const sig = await setAuthority(
    conn,
    currentAuthority, // payer
    TEST_USDC_MINT,
    currentAuthority, // current authority
    AuthorityType.MintTokens,
    newAuthority.publicKey, // new authority
  );

  console.log(`✅ Authority transferred. Tx: ${sig}`);
  console.log(`   Verify: https://solscan.io/tx/${sig}?cluster=devnet`);
}

main().catch((err) => {
  console.error("Fatal:", err);
  process.exit(1);
});
