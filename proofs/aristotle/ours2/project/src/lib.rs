//! The resolved-close payout path of the percolator v2.2 risk engine, cut out of `src/v16.rs`
//! (dcccrypto/percolator, engine revision c38b6b1e) so that it can be verified on its own.
//!
//! * `wide_math`: the engine's `src/wide_math.rs`, unchanged (generated copy).
//! * `engine_slice`: generated. Engine text copied byte for byte (each block carries its line
//!   range), plus stubs and wrapper lines, every one of them between `// >>> NOT ENGINE` and
//!   `// <<< NOT ENGINE` markers.
//! * `engine_slice::harness`: NOT engine code. The loop `close_all` and two one-line entry
//!   points, so that "any number of accounts, any order" is a statement about one function.
//!
//! `aristotle/resolved_close_spec.md` lists what is engine text, what is stubbed, the invariant
//! and the properties.

#![no_std]
#![deny(unsafe_code)]
// The engine functions copied here are private methods; not all of them are reachable from the
// crate's public items.
#![allow(dead_code)]

/// src/lib.rs:25 of the engine.
pub const BOUND_SCALE: u128 = 1_000_000_000_000;

pub mod engine_slice;
pub mod wide_math;
