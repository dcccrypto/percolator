//! Pure Rust safety model for Kani verification
//! No Solana dependencies, no unwrap/panic, all functions total

pub mod state;
pub mod math;
pub mod warmup;
pub mod helpers;
pub mod transitions;

// Re-export commonly used types
pub use state::*;
pub use helpers::*;
pub use transitions::*;
