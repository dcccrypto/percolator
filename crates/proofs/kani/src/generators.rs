//! Generators for arbitrary state (for Kani)

#[cfg(kani)]
use kani::any;
use model_safety::state::*;
use arrayvec::ArrayVec;

// Small bounds for tractable verification
const MAX_VAL: u128 = 1_000_000;
const MAX_PNL: i128 = 1_000_000;
const MAX_SLOT: u64 = 1000;

#[cfg(kani)]
pub fn any_account() -> Account {
    let principal_raw: u16 = any();
    let pnl_raw: i16 = any();
    let reserved_raw: u16 = any();
    let slot_raw: u16 = any();
    let slope_raw: u16 = any();

    Account {
        principal: (principal_raw as u128) % MAX_VAL,
        pnl_ledger: (pnl_raw as i128).clamp(-MAX_PNL, MAX_PNL),
        reserved_pnl: (reserved_raw as u128) % MAX_VAL,
        warmup_state: Warmup {
            started_at_slot: (slot_raw as u64) % MAX_SLOT,
            slope_per_step: ((slope_raw as u128) % 10000).max(1), // Non-zero
        },
    }
}

#[cfg(kani)]
pub fn any_state_bounded() -> State {
    let mut users: ArrayVec<Account, 6> = ArrayVec::new();
    let n: u8 = any();
    let n = ((n % 3) as usize) + 1; // 1-3 users (reduced from 1-5)

    for _ in 0..n {
        let _ = users.try_push(any_account());
    }

    let vault_raw: u32 = any();
    let insurance_raw: u16 = any();
    let fees_raw: u16 = any();

    State {
        vault: (vault_raw as u128) % (MAX_VAL * 10),
        insurance_fund: (insurance_raw as u128) % MAX_VAL,
        fees_outstanding: (fees_raw as u128) % MAX_VAL,
        users,
        params: Params {
            max_users: 6,
            withdraw_cap_per_step: 1_000_000,
        },
        authorized_router: true, // Start authorized
    }
}
