"use client";

import { useState, useCallback } from "react";
import { PublicKey, Transaction, TransactionInstruction } from "@solana/web3.js";
import { useWalletCompat } from "@/hooks/useWalletCompat";
import { useConnectionCompat } from "@/hooks/useWalletCompat";
import { getConfig } from "@/lib/config";

/**
 * Tag 13 = CloseSlab instruction in percolator-prog.
 * Accounts: [admin(signer, writable), slab(writable)]
 * Data: [13] (1 byte)
 *
 * Requirements:
 * - Admin must sign
 * - Vault balance must be zero
 * - Insurance balance must be zero
 * - No open user accounts
 * - dust_base must be zero
 */
const TAG_CLOSE_SLAB = 13;

interface CloseResult {
  signature: string;
  reclaimedLamports: number;
}

export function useCloseMarket() {
  const { wallet } = useWalletCompat();
  const { connection } = useConnectionCompat();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /**
   * Close a slab and reclaim rent.
   * Only works if vault/insurance are empty and no user accounts exist.
   * @param slabAddress - The slab account public key
   * @param programIdOverride - Optional program ID (auto-detected from slab owner if omitted)
   */
  const closeSlab = useCallback(
    async (slabAddress: string, programIdOverride?: string): Promise<CloseResult | null> => {
      if (!wallet.publicKey || !wallet.signTransaction) {
        setError("Wallet not connected");
        return null;
      }

      setLoading(true);
      setError(null);

      try {
        const slabPk = new PublicKey(slabAddress);

        // Fetch the slab account to get its owner (program ID) and lamports
        const accountInfo = await connection.getAccountInfo(slabPk);
        if (!accountInfo) {
          // Account doesn't exist — nothing to reclaim.
          // Clean up localStorage.
          localStorage.removeItem("percolator-pending-slab-keypair");
          setError("Slab account no longer exists (already reclaimed or rolled back).");
          setLoading(false);
          return null;
        }

        const reclaimableLamports = accountInfo.lamports;
        const programId = programIdOverride
          ? new PublicKey(programIdOverride)
          : accountInfo.owner;

        // Build CloseSlab instruction
        const ix = new TransactionInstruction({
          programId,
          keys: [
            { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
            { pubkey: slabPk, isSigner: false, isWritable: true },
          ],
          data: Buffer.from([TAG_CLOSE_SLAB]),
        });

        const { blockhash } = await connection.getLatestBlockhash("confirmed");
        const tx = new Transaction({ recentBlockhash: blockhash, feePayer: wallet.publicKey });
        tx.add(ix);

        const signed = await wallet.signTransaction(tx);
        const sig = await connection.sendRawTransaction(signed.serialize(), {
          skipPreflight: false,
          preflightCommitment: "confirmed",
        });
        await connection.confirmTransaction(sig, "confirmed");

        // Clean up localStorage
        localStorage.removeItem("percolator-pending-slab-keypair");

        setLoading(false);
        return { signature: sig, reclaimedLamports: reclaimableLamports };
      } catch (err: any) {
        const msg = err?.message ?? String(err);

        // Parse common CloseSlab failures
        if (msg.includes("0xd") || msg.includes("EngineInsufficientBalance")) {
          setError(
            "Cannot close: the slab vault or insurance fund still has tokens. " +
            "Complete market creation to use those funds, or contact support to drain them."
          );
        } else if (msg.includes("0x10") || msg.includes("AccountNotFound")) {
          setError("Cannot close: there are still open user accounts on this market.");
        } else if (msg.includes("User rejected") || msg.includes("WalletSign")) {
          setError("Transaction cancelled.");
        } else {
          setError(`Failed to close slab: ${msg.slice(0, 200)}`);
        }

        setLoading(false);
        return null;
      }
    },
    [wallet, connection],
  );

  return { closeSlab, loading, error };
}
