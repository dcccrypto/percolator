import { Connection, PublicKey } from "@solana/web3.js";
import * as dotenv from "dotenv";
dotenv.config();

const conn = new Connection(process.env.RPC_URL!, "confirmed");

// Check recent transactions on a few SOL slabs for EngineOverflow failures
const SOL_SLABS = [
  "EkQty1LsYs4hx17ZCZ6md7u3sksGxzdVR1aw2RJnxFG2", // SOL, numUsed=1, has LP
  "8Wxmx93jWGWFmVfQccfVsYiAL7xoUfBbd8vqJrvFhz8x", // SOL, numUsed=2, has LP
  "XXs8pWLASKrMgJ6JdBcgRMbbRRA4dKRSPtXEKSf257X",  // SOL, numUsed=0
];

async function main() {
  for (const addr of SOL_SLABS) {
    const pk = new PublicKey(addr);
    console.log(`\nChecking ${addr.slice(0,12)}...`);
    const sigs = await conn.getSignaturesForAddress(pk, { limit: 10 });
    for (const s of sigs) {
      if (s.err) {
        console.log(`  FAILED tx: ${s.signature.slice(0,20)}... err=${JSON.stringify(s.err)}`);
      } else {
        console.log(`  OK tx: ${s.signature.slice(0,20)}...`);
      }
    }
  }
}
main().catch(console.error);
