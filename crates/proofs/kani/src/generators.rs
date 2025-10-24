//! Generators for arbitrary state (for Kani)

#[cfg(kani)]
use kani::any;
use model_safety::state::*;
use arrayvec::ArrayVec;

// Ultra-small bounds for very fast verification
const MAX_VAL: u128 = 1000;
const MAX_PNL: i128 = 1000;

#[cfg(kani)]
pub fn any_account() -> Account {
    let principal_raw: u8 = any();
    let pnl_raw: i8 = any();
    let reserved_raw: u8 = any();
    let slope_raw: u8 = any();

    Account {
        principal: (principal_raw as u128) % MAX_VAL,
        pnl_ledger: (pnl_raw as i128).clamp(-MAX_PNL, MAX_PNL),
        reserved_pnl: (reserved_raw as u128) % MAX_VAL,
        warmup_state: Warmup {
            started_at_slot: (principal_raw as u64) % 100,  // Reuse for simplicity
            slope_per_step: ((slope_raw as u128) % 100).max(1), // Non-zero, small
        },
    }
}

#[cfg(kani)]
pub fn any_state_bounded() -> State {
    let mut users: ArrayVec<Account, 6> = ArrayVec::new();
    let n: u8 = any();
    let n = ((n % 2) as usize) + 1; // 1-2 users only

    for _ in 0..n {
        let _ = users.try_push(any_account());
    }

    let vault_raw: u16 = any();
    let insurance_raw: u8 = any();
    let fees_raw: u8 = any();

    State {
        vault: (vault_raw as u128) % (MAX_VAL * 5),
        insurance_fund: (insurance_raw as u128) % MAX_VAL,
        fees_outstanding: (fees_raw as u128) % MAX_VAL,
        users,
        params: Params {
            max_users: 6,
            withdraw_cap_per_step: 1_000,
        },
        authorized_router: true, // Start authorized
    }
}
