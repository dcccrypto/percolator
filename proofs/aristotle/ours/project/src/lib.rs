//! The resolved-payout value path of the percolator v2.2 risk engine, cut out of `src/v16.rs`
//! (dcccrypto/percolator, engine revision c38b6b1e) so that it can be verified on its own.
//!
//! * `wide_math`: the engine's `src/wide_math.rs`, unchanged.
//! * `resolved_payout`: the engine's ledger and receipt types and the pure functions that compute
//!   the payout rate, a receipt's claimable amount and a receipt payment, copied byte for byte
//!   (generated; see the header of that file).
//! * `resolved_payout::close_path`: NOT engine code. A transcription, over plain values, of the
//!   value-moving tail of `close_resolved_account_not_atomic`, of
//!   `create_resolved_payout_receipt_if_needed` and of `claim_resolved_payout_topup_core_not_atomic`,
//!   plus a loop that closes many accounts. It calls the copied engine functions. See
//!   `aristotle/resolved_close_spec.md` for every difference from the engine.

#![no_std]
#![deny(unsafe_code)]

/// src/lib.rs:25 of the engine.
pub const BOUND_SCALE: u128 = 1_000_000_000_000;

pub mod resolved_payout;
pub mod wide_math;
