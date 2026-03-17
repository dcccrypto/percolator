import { Connection, PublicKey } from "@solana/web3.js";
import * as dotenv from "dotenv";
dotenv.config();

const conn = new Connection(process.env.RPC_URL!, "confirmed");

// ALL SOL slabs from find-sol-slab.ts
const SOL_SLABS = [
  "HrdveBrbepjvwAn2qmCPU9eRSFG6Munpkw7gXCHvLpBN",
  "9R6iRUH6Aeo353nXDSAGZuQ7BNXdNXcVXPEEVUhSecWL",
  "BbPEHRVBDWQrW6Y12uMDTpHNc8FfKsucKNSTiXfemgsN",
  "8MsKdp47Q2zeQeSBLf9gcYV7Gx3J7UdrydgCavZvTa4K",
  "DD9Ym1xSGbnCYrfZnpNvSp3JmDHVMiajzdJHz8rUbwJR",
  "5DZRZzB8JRb8MG7KnbmqRaf2A2SHTXDcXMGs7vTuZwud",
  "XXs8pWLASKrMgJ6JdBcgRMbbRRA4dKRSPtXEKSf257X",
  "85eYcFxWfQ3GdM6qrAWhLE75sd6RH7p5FWrQJ2BT4uDd",
  "EkQty1LsYs4hx17ZCZ6md7u3sksGxzdVR1aw2RJnxFG2",
  "7f2xHgdJ6W9fdd7raXza32U7VwDSKdtReWuuscfrQ4g2",
  "9MHHVtthn6k1rb5P2iViWf4ELRNAGw25F95QBzwiKJzU",
  "FhpPmmuh5UDAjvEjrYBPFwmj4CP4otvsYMxtTb46p1Ss",
  "FeZJKzhDjYe3VpDQWzj4ziPXoFSeE682ebtRdtyFYtxp",
  "7fRWb7vNyLuQHgMNnSfH7mfbonj8pSzpuhzcjScEkyC3",
  "7reWhB1S8tD35tHyfb3hNi2PkUqZfhwNhGLrXT89FW6u",
  "7JBhXqX4yw3hcHMU5tiAB7Bxa5FSi4oiDDRhjuSHLBiP",
  "4iGAT1aPMA2cwsf6ZEmac4Yav9vTAYFpZq6YrHRnByBQ",
  "CinzdgsPDsCmceWZ1srfbPTC3WtaGMbMFWqU1oAK34qo",
  "8Wxmx93jWGWFmVfQccfVsYiAL7xoUfBbd8vqJrvFhz8x",
];

async function main() {
  for (const addr of SOL_SLABS) {
    const pk = new PublicKey(addr);
    const sigs = await conn.getSignaturesForAddress(pk, { limit: 5 });
    const failed = sigs.filter(s => s.err);
    if (failed.length > 0) {
      console.log(`${addr.slice(0,12)}... has FAILED txs:`);
      for (const f of failed) {
        console.log(`  ${f.signature.slice(0,30)}... err=${JSON.stringify(f.err)}`);
      }
    }
  }
  console.log("Done");
}
main().catch(console.error);
