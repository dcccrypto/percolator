//! Kani safety proofs for percolator safety module

#![cfg_attr(kani, feature(register_tool), register_tool(kanitool))]

pub mod sanitizer;
pub mod generators;
pub mod adversary;

#[cfg(kani)]
pub mod safety;
