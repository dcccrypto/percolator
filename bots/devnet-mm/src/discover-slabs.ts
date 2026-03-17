import { Connection, PublicKey } from "@solana/web3.js";
import { discoverMarkets } from "@percolator/sdk";

const PROGRAMS = [
  { name: "Small",  id: "FwfBKZXbYr4vTK23bMFkbgKq3npJ3MSDxEaKmq9Aj4Qn" },
  { name: "Medium", id: "g9msRSV3sJmmE3r5Twn9HuBsxzuuRGTjKCVTKudm9in" },
  { name: "Large",  id: "FxfD37s1AZTeWfFQps9Zpebi2dNQ9QSSDtfMKdbsfKrD" },
];

async function main() {
  const rpc = process.env.RPC_URL ?? `https://devnet.helius-rpc.com/?api-key=${process.env.HELIUS_API_KEY ?? process.env.HELIUS_DEVNET_API_KEY ?? ""}`;
  const conn = new Connection(rpc, "confirmed");

  for (const prog of PROGRAMS) {
    try {
      const markets = await discoverMarkets(conn, new PublicKey(prog.id));
      console.log(`\n=== ${prog.name} (${prog.id.slice(0,8)}) ===`);
      if (markets.length === 0) {
        console.log("  (no markets found)");
      }
      for (const m of markets) {
        const feedId = m.config.indexFeedId;
        const feedHex = Buffer.from(
          feedId instanceof PublicKey ? feedId.toBytes() : feedId as Uint8Array
        ).toString("hex");
        const isHyperp = feedHex === "0".repeat(64);
        const priceAuth = m.config.authorityPriceE6?.toString() ?? "0";
        const lastPrice = m.config.lastEffectivePriceE6?.toString() ?? "0";
        console.log(`  slab: ${m.slabAddress.toBase58()}`);
        console.log(`  oracle: ${isHyperp ? "hyperp/authority" : "pyth:" + feedHex.slice(0,16)}`);
        console.log(`  authorityPriceE6: ${priceAuth} (${Number(priceAuth)/1e6} USD)`);
        console.log(`  lastEffectivePriceE6: ${lastPrice} (${Number(lastPrice)/1e6} USD)`);
        console.log(`  resolved: ${m.header.resolved}, paused: ${m.header.paused}`);
      }
    } catch (e) {
      console.log(`\n=== ${prog.name} ERROR: ${e instanceof Error ? e.message : e}`);
    }
  }
}

main().catch(console.error);
