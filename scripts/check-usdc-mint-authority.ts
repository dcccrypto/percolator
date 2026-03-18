/**
 * PERC-586: Check testUsdcMint on-chain authority
 * Reads the mint account and reports the current mint authority.
 * Also checks DEVNET_MINT_AUTHORITY_KEYPAIR pubkey from env.
 *
 * Usage: pnpm tsx scripts/check-usdc-mint-authority.ts
 */
import { Connection, PublicKey } from "@solana/web3.js";
// No bs58 needed for read-only path

// testUsdcMint default from config.ts
const TEST_USDC_MINT =
  process.env.NEXT_PUBLIC_TEST_USDC_MINT ?? "DvH13uxzTzo1xVFwkbJ6YASkZWs6bm3vFDH4xu7kUYTs";

const RPC = "https://api.devnet.solana.com";

async function main() {
  const conn = new Connection(RPC, "confirmed");
  const mint = new PublicKey(TEST_USDC_MINT);

  console.log(`\n=== testUsdcMint authority check ===`);
  console.log(`Mint address : ${TEST_USDC_MINT}`);
  console.log(`RPC          : ${RPC}`);
  console.log(`---`);

  const info = await conn.getAccountInfo(mint);
  if (!info) {
    console.error(`❌ Mint account ${TEST_USDC_MINT} does not exist on devnet`);
    process.exit(1);
  }

  const data = new Uint8Array(info.data);
  if (data.length < 36) {
    console.error(`❌ Mint account data too short (${data.length} bytes) — not a valid SPL mint`);
    process.exit(1);
  }

  const hasMintAuthority =
    new DataView(data.buffer, data.byteOffset).getUint32(0, true) === 1;

  if (!hasMintAuthority) {
    console.log(`ℹ️  Mint has NO mint authority (fixed supply) — no re-key needed`);
    process.exit(0);
  }

  const onChainAuthority = new PublicKey(data.slice(4, 36)).toBase58();
  console.log(`On-chain mint authority : ${onChainAuthority}`);

  // Check DEVNET_MINT_AUTHORITY_KEYPAIR if set
  const rawKey = process.env.DEVNET_MINT_AUTHORITY_KEYPAIR;
  if (!rawKey) {
    console.log(`\nDEVNET_MINT_AUTHORITY_KEYPAIR env var is NOT set — cannot determine target pubkey.`);
    console.log(`\nTo transfer authority you need:`);
    console.log(`  1. The private key of the current authority (${onChainAuthority})`);
    console.log(`  2. The public key of the new authority (DEVNET_MINT_AUTHORITY_KEYPAIR)`);
  } else {
    const { Keypair } = await import("@solana/web3.js");
    let kp: import("@solana/web3.js").Keypair;
    try {
      const parsed = JSON.parse(rawKey);
      kp = Keypair.fromSecretKey(Uint8Array.from(parsed));
    } catch {
      // Try raw array if not JSON — fallback using Buffer
      const bytes = Buffer.from(rawKey, "base64");
      kp = Keypair.fromSecretKey(new Uint8Array(bytes));
    }
    const envPubkey = kp.publicKey.toBase58();
    console.log(`Env authority (DEVNET_MINT_AUTHORITY_KEYPAIR) : ${envPubkey}`);

    if (onChainAuthority === envPubkey) {
      console.log(`\n✅ Authority already matches DEVNET_MINT_AUTHORITY_KEYPAIR — no re-key needed!`);
    } else {
      console.log(`\n⚠️  MISMATCH — faucet will return 400 until re-key is done.`);
      console.log(`\nTo fix: run scripts/transfer-usdc-mint-authority.ts with the private key of:`);
      console.log(`  Current authority : ${onChainAuthority}`);
      console.log(`  New authority     : ${envPubkey}`);
    }
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
