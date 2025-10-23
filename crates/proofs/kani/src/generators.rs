//! Generators for arbitrary state (for Kani)

#[cfg(kani)]
use kani::any;
use model_safety::state::*;
use arrayvec::ArrayVec;

#[cfg(kani)]
pub fn any_account() -> Account {
    Account {
        principal: any(),
        pnl_ledger: any(),
        reserved_pnl: any(),
        warmup_state: Warmup {
            started_at_slot: any(),
            slope_per_step: any(),
        },
    }
}

#[cfg(kani)]
pub fn any_state_bounded() -> State {
    let mut users: ArrayVec<Account, 6> = ArrayVec::new();
    let n: u8 = any();
    let n = (n % 5) as usize + 1; // 1-5 users

    for _ in 0..n {
        let _ = users.try_push(any_account());
    }

    State {
        vault: any(),
        insurance_fund: any(),
        fees_outstanding: any(),
        users,
        params: Params {
            max_users: 6,
            withdraw_cap_per_step: 1_000_000,
        },
        authorized_router: true, // Start authorized
    }
}
