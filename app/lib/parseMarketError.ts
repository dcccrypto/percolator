/**
 * Parse Solana transaction errors into user-friendly messages for market creation.
 * Covers common failure modes: insufficient balance, user rejection, network errors,
 * and Percolator program-specific error codes.
 */

// Percolator program custom error codes (from percolator-prog/src/percolator.rs PercolatorError enum)
// IMPORTANT: These must match the exact order of the enum variants in the Rust program.
// Last verified: 2026-03-08 against percolator-prog main.
const PERCOLATOR_ERRORS: Record<number, string> = {
  0: "Invalid magic number. The slab account data is corrupted.",                          // InvalidMagic
  1: "Invalid version — program upgrade may be needed.",                                   // InvalidVersion
  2: "Market is already initialized. Cannot re-initialize.",                               // AlreadyInitialized
  3: "Market is not initialized. The slab account may not have been set up correctly.",     // NotInitialized
  4: "Invalid slab length. This market uses an older program version — its account size "   // InvalidSlabLen (PERC-698)
   + "is incompatible with the current deployed program. The market may need to be "
   + "re-initialized by the market creator. If you are the creator, please contact support. "
   + "Otherwise, try a newer market or a different slab tier.",
  5: "Invalid oracle key. The oracle feed ID doesn't match.",                              // InvalidOracleKey
  6: "Oracle price is stale. The oracle hasn't been updated recently enough.",              // OracleStale
  7: "Oracle confidence interval too wide. Price may be unreliable.",                       // OracleConfTooWide
  8: "Invalid vault token account. The ATA doesn't match the expected address.",            // InvalidVaultAta
  9: "Invalid collateral mint. The token mint doesn't match the market's collateral.",      // InvalidMint
  10: "Expected signer. A required account was not a signer on the transaction.",           // ExpectedSigner
  11: "Expected writable. A required account was not marked as writable.",                  // ExpectedWritable
  12: "Oracle data is invalid or malformed.",                                               // OracleInvalid
  13: "Insufficient balance — deposit more collateral.",                                    // EngineInsufficientBalance
  14: "Math overflow — values are too large for safe computation.",                         // MathOverflow
  15: "Margin requirement not met. Increase collateral or reduce position size.",           // MarginInsufficient
  16: "Account not found in the market.",                                                   // AccountNotFound
  17: "Market is paused by admin.",                                                         // MarketPaused
  18: "Insufficient seed deposit. The vault needs collateral before market initialization.",// InsufficientSeed
};

export function parseMarketCreationError(error: unknown): string {
  const msg = error instanceof Error ? error.message : String(error);

  // User rejected the transaction in their wallet
  if (
    msg.includes("User rejected") ||
    msg.includes("user rejected") ||
    msg.includes("Transaction cancelled") ||
    msg.includes("WalletSignTransactionError")
  ) {
    return "Transaction cancelled — you rejected the signing request in your wallet. Click Retry to try again.";
  }

  // Insufficient SPL token balance (token program error 0x1 or transfer failure).
  // Must be checked BEFORE the SOL/lamports branch — Solana simulation errors for
  // token transfers also include "insufficient funds" but are not a SOL problem. Fixes #758.
  if (
    msg.includes("insufficient funds for transfer") ||
    (msg.includes("insufficient funds") && !msg.includes("lamports") && !msg.includes("for rent")) ||
    (msg.includes("custom program error: 0x1") && msg.includes("TokenkegQ"))
  ) {
    return "Insufficient token balance. Your wallet doesn't have enough collateral tokens to complete this step. On devnet, refresh the page and retry — the faucet will top up your balance.";
  }

  // Insufficient SOL for rent/fees
  if (
    msg.includes("Attempt to debit an account but found no record of a prior credit") ||
    msg.includes("insufficient lamports") ||
    msg.includes("insufficient funds")
  ) {
    return "Insufficient SOL balance. You need enough SOL to cover the slab rent and transaction fees. Check your wallet balance.";
  }

  // Account already exists (slab already created in a previous attempt)
  if (msg.includes("already in use")) {
    return "The slab account already exists from a previous attempt. Click Retry to continue from the current step.";
  }

  // Transaction too large
  if (msg.includes("Transaction too large") || msg.includes("transaction too large")) {
    return "Transaction is too large. Try selecting a smaller slab tier (fewer trader slots).";
  }

  // Blockhash expired (tx took too long)
  if (
    msg.includes("block height exceeded") ||
    msg.includes("Blockhash not found") ||
    msg.includes("blockhash")
  ) {
    return "Transaction expired before confirmation. The network may be congested. Click Retry to try again.";
  }

  // Simulation failed — try to extract program error
  if (msg.includes("custom program error")) {
    const match = msg.match(/custom program error:\s*0x([0-9a-fA-F]+)/);
    if (match) {
      const code = parseInt(match[1], 16);
      const friendly = PERCOLATOR_ERRORS[code];
      if (friendly) return friendly;
      return `Program error (code ${code}). The on-chain program rejected the transaction.`;
    }
  }

  // InstructionError with index
  if (msg.includes("InstructionError")) {
    const match = msg.match(/InstructionError.*?(\d+).*?Custom.*?(\d+)/);
    if (match) {
      const code = parseInt(match[2]);
      const friendly = PERCOLATOR_ERRORS[code];
      if (friendly) return `Step failed: ${friendly}`;
    }
  }

  // Network/RPC errors
  if (msg.includes("Failed to fetch") || msg.includes("NetworkError") || msg.includes("ECONNREFUSED")) {
    return "Network error — cannot reach Solana RPC. Check your internet connection and try again.";
  }

  // Timeout
  if (msg.includes("timeout") || msg.includes("Timeout") || msg.includes("ETIMEDOUT")) {
    return "Request timed out. The Solana network may be congested. Click Retry to try again.";
  }

  // Wallet not connected
  if (msg.includes("Wallet not connected") || msg.includes("wallet adapter")) {
    return "Wallet disconnected. Please reconnect your wallet and try again.";
  }

  // Fallback: truncate long messages but keep them informative
  if (msg.length > 200) {
    return `Transaction failed: ${msg.slice(0, 180)}... Click Retry or Start Over.`;
  }

  return `Transaction failed: ${msg}`;
}
