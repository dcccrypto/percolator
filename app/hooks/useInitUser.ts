"use client";

import { useCallback, useState } from "react";
import { PublicKey } from "@solana/web3.js";
import { useWalletCompat, useConnectionCompat } from "@/hooks/useWalletCompat";
import {
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddress,
  getAccount,
} from "@solana/spl-token";
import {
  encodeInitUser,
  ACCOUNTS_INIT_USER,
  buildAccountMetas,
  WELL_KNOWN,
  buildIx,
  getAta,
  detectSlabLayout,
} from "@percolator/sdk";
import { sendTx } from "@/lib/tx";
import { useSlabState } from "@/components/providers/SlabProvider";


export function useInitUser(slabAddress: string) {
  const { connection } = useConnectionCompat();
  const wallet = useWalletCompat();
  const { config: mktConfig, programId: slabProgramId, raw: slabRaw, refresh: refreshSlab } = useSlabState();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const initUser = useCallback(
    async (feePayment: bigint = 0n) => {
      setLoading(true);
      setError(null);
      try {
        if (!wallet.publicKey || !mktConfig || !slabProgramId) throw new Error("Wallet not connected or market not loaded");

        // PERC-698 / bug bounty: Pre-flight V0/V1 slab version check.
        // If the slab is V0 size but the on-chain program now expects V1 layout,
        // the InitUser tx will fail with custom program error 0x4 (InvalidSlabLen).
        // Detect this mismatch proactively and surface a clear message.
        if (slabRaw && slabRaw.length > 0) {
          const layout = detectSlabLayout(slabRaw.length);
          if (layout?.version === 0) {
            throw new Error(
              "This market uses an older format (V0) that is incompatible with the current " +
              "program version. The market creator needs to re-initialize it. " +
              "Please try a different market or contact support."
            );
          }
        }
        const programId = slabProgramId;
        const slabPk = new PublicKey(slabAddress);
        const userAta = await getAta(wallet.publicKey, mktConfig.collateralMint);

        // Check if ATA exists — create it first if not (prevents error 24)
        const instructions = [];
        try {
          await getAccount(connection, userAta);
        } catch {
          // ATA doesn't exist — create it
          const createAtaIx = createAssociatedTokenAccountInstruction(
            wallet.publicKey,     // payer
            userAta,              // ata
            wallet.publicKey,     // owner
            mktConfig.collateralMint, // mint
          );
          instructions.push(createAtaIx);
        }

        const ix = buildIx({
          programId,
          keys: buildAccountMetas(ACCOUNTS_INIT_USER, [
            wallet.publicKey, slabPk, userAta, mktConfig.vaultPubkey, WELL_KNOWN.tokenProgram,
          ]),
          data: encodeInitUser({ feePayment: feePayment.toString() }),
        });
        instructions.push(ix);
        const sig = await sendTx({ connection, wallet, instructions });
        // Force immediate slab re-read so the new user sub-account is visible
        // without waiting for the next poll cycle (up to 30 s with WS active).
        refreshSlab();
        setTimeout(() => refreshSlab(), 2000);
        return sig;
      } catch (e) {
        const raw = e instanceof Error ? e.message : String(e);
        // PERC-698: Custom program error 0x4 = InvalidSlabLen — V0/V1 program mismatch.
        // This happens when a market was created with an older program binary and the
        // program was subsequently upgraded with different account size constants.
        const is0x4 = /custom program error:\s*0x4\b/i.test(raw);
        const userMsg = is0x4
          ? "This market uses an older format that's incompatible with the current program version. " +
            "The market creator needs to re-initialize it. Please try a different market or contact support."
          : raw;
        setError(userMsg);
        throw new Error(userMsg);
      } finally {
        setLoading(false);
      }
    },
    [connection, wallet, mktConfig, slabAddress, slabProgramId, slabRaw, refreshSlab]
  );

  return { initUser, loading, error };
}
