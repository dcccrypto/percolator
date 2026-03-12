#!/usr/bin/env npx tsx
/**
 * Inspect Large-tier slab candidates to diagnose PERC-381.
 */
import { Connection, PublicKey } from "@solana/web3.js";
import { parseHeader, parseConfig } from "../packages/core/src/index.js";

const conn = new Connection("https://api.devnet.solana.com", "confirmed");

const candidates = [
  "8dJs5dSz9rUcP7f9NMaMEyvBgK3F7dbsPP3G9Sx7Gcwx",
  "6JSp61JAU8hjbN41oocrQ6SoYuivk7HtiTrcZZ9MtP3A",
  "A35wGP21WCnpQuiHS3Ec3V8g22ikfmTfM2GHuq15uyfv",
];

async function main() {
  for (const addr of candidates) {
    console.log("\n=== " + addr + " ===");
    const info = await conn.getAccountInfo(new PublicKey(addr));
    if (!info) { console.log("  Not found"); continue; }
    console.log("  Size:", info.data.length, "bytes (expected Large: 1025832)");
    console.log("  Size delta:", info.data.length - 1025832, "bytes");
    console.log("  Owner:", info.owner.toBase58());
    try {
      const data = info.data;
      const header = parseHeader(data);
      const config = parseConfig(data);
      console.log("  Header version:", header.version);
      console.log("  Mint:", config.collateralMint?.toBase58());
      console.log("  Admin:", config.admin?.toBase58());
      console.log("  InitialMarkPriceE6:", config.initialMarkPriceE6?.toString());
      const feedId = new Uint8Array(config.indexFeedId || []);
      const hex = Array.from(feedId).map(b => b.toString(16).padStart(2, "0")).join("");
      console.log("  Oracle feed:", hex.slice(0, 32) + "...");
    } catch (e: any) {
      console.log("  Parse error:", e.message);
    }
  }
}

main().catch(console.error);
