//! Invariant checking helpers

use crate::state::*;
use crate::math::*;

/// I2: Conservation - vault balance equals sum of principals + insurance - fees
pub fn conservation_ok(s: &State) -> bool {
    let sum_principal = s.users.iter().fold(0u128, |acc, u| add_u128(acc, u.principal));

    // vault should equal: principals + insurance - fees
    // But we also need to account for PnL in the vault
    // Simplified model: vault == sum(principal) + insurance - fees + sum(positive_pnl)
    let sum_pos_pnl = s.users.iter().fold(0u128, |acc, u| {
        let pos_pnl = clamp_pos_i128(u.pnl_ledger);
        add_u128(acc, pos_pnl)
    });

    let expected_vault = add_u128(
        add_u128(sum_principal, s.insurance_fund),
        sum_pos_pnl
    );
    let expected_vault = sub_u128(expected_vault, s.fees_outstanding);

    s.vault == expected_vault
}

/// I1: Principals unchanged between two states
pub fn principals_unchanged(before: &State, after: &State) -> bool {
    if before.users.len() != after.users.len() {
        return false;
    }
    before.users.iter().zip(after.users.iter())
        .all(|(a, b)| a.principal == b.principal)
}

/// I4: Haircut only hits winners (positive PnL accounts)
pub fn winners_only_haircut(before: &State, after: &State) -> bool {
    if before.users.len() != after.users.len() {
        return false;
    }
    before.users.iter().zip(after.users.iter()).all(|(a, b)| {
        if b.pnl_ledger < a.pnl_ledger {
            // PnL decreased - must have been positive before
            a.pnl_ledger > 0
        } else {
            true
        }
    })
}

/// I4: Calculate total haircut applied
pub fn total_haircut(before: &State, after: &State) -> u128 {
    if before.users.len() != after.users.len() {
        return 0;
    }
    before.users.iter().zip(after.users.iter()).fold(0u128, |acc, (a, b)| {
        if b.pnl_ledger < a.pnl_ledger {
            let cut = sub_i128(a.pnl_ledger, b.pnl_ledger);
            add_u128(acc, clamp_pos_i128(cut))
        } else {
            acc
        }
    })
}

/// I4: Calculate sum of effective positive PnL across all users
pub fn sum_effective_winners(s: &State) -> u128 {
    s.users.iter().fold(0u128, |acc, u| {
        let eff = crate::warmup::effective_positive_pnl(u);
        add_u128(acc, eff)
    })
}

/// I6: Balances unchanged (vault and all user balances)
pub fn balances_unchanged(before: &State, after: &State) -> bool {
    if before.vault != after.vault {
        return false;
    }
    if before.users.len() != after.users.len() {
        return false;
    }
    before.users.iter().zip(after.users.iter()).all(|(a, b)| {
        a.principal == b.principal && a.pnl_ledger == b.pnl_ledger
    })
}
