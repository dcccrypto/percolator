//! Runtime unit tests for the LP Vault share-math (Phase 2.B Tier 3,
//! Workstream 4B). Companion to the Kani harnesses in
//! `tests/proofs_v16_fork.rs` (`proof_v16_lp_vault_*`).
//!
//! These exercise the pure functions in `percolator::lp_vault`:
//! - `lp_vault_nav_atoms` — NAV from ledger counters (sign-off Note 2)
//! - `lp_shares_for_deposit` — round-DOWN share issuance
//! - `lp_atoms_for_redemption` — round-DOWN redemption
//! - `lp_fee_split` — LP/insurance earnings split
//! - `lp_redemption_cooldown_elapsed` — B-7 cooldown gate
//!
//! Cross-references:
//! - `~/wrapper-engine-deep-audit/lp_vault_design.md` §5.2, §7, §12
//! - sign-off Note 1 (round-to-zero reject), Note 2 (NAV from counters)

use percolator::lp_vault::{
    lp_atoms_for_redemption, lp_fee_split, lp_redemption_cooldown_elapsed, lp_shares_for_deposit,
    lp_vault_nav_atoms,
};
use percolator::V16Error;

// ── lp_vault_nav_atoms ──────────────────────────────────────────────

#[test]
fn nav_principal_only_no_earnings_no_loss() {
    // 1_000 principal, no earnings, no loss → NAV = 1_000
    let nav = lp_vault_nav_atoms(1_000, 0, 0, 0, 0, 10_000).unwrap();
    assert_eq!(nav, 1_000);
}

#[test]
fn nav_adds_lp_share_of_earnings() {
    // 1_000 principal + 200 earnings at 100% fee share → NAV = 1_200
    let nav = lp_vault_nav_atoms(1_000, 200, 0, 0, 0, 10_000).unwrap();
    assert_eq!(nav, 1_200);
}

#[test]
fn nav_applies_fee_share_bps_to_earnings() {
    // 1_000 principal + 200 earnings at 50% fee share → NAV = 1_000 + 100
    let nav = lp_vault_nav_atoms(1_000, 200, 0, 0, 0, 5_000).unwrap();
    assert_eq!(nav, 1_100);
    // insurance-side 100 atoms accrue in bucket, not counted (Note 3 stub)
}

#[test]
fn nav_subtracts_withdrawn_earnings() {
    // 1_000 principal + 200 earnings - 50 earnings withdrawn @ 100% → 1_150
    let nav = lp_vault_nav_atoms(1_000, 200, 50, 0, 0, 10_000).unwrap();
    assert_eq!(nav, 1_150);
}

#[test]
fn nav_subtracts_net_impairment() {
    // 1_000 principal, 300 loss, 100 recovery → net impairment 200 → NAV 800
    let nav = lp_vault_nav_atoms(1_000, 0, 0, 300, 100, 10_000).unwrap();
    assert_eq!(nav, 800);
}

#[test]
fn nav_full_impairment_yields_zero() {
    // principal fully impaired → NAV 0 (not error — vault wiped but solvent)
    let nav = lp_vault_nav_atoms(1_000, 0, 0, 1_000, 0, 10_000).unwrap();
    assert_eq!(nav, 0);
}

#[test]
fn nav_recovery_exceeds_loss_is_anomaly() {
    // recovery > loss can never happen by ledger construction; fail closed
    let res = lp_vault_nav_atoms(1_000, 0, 0, 100, 300, 10_000);
    assert_eq!(res, Err(V16Error::CounterUnderflow));
}

#[test]
fn nav_impairment_exceeds_principal_is_anomaly() {
    // net impairment > principal can never happen; fail closed
    let res = lp_vault_nav_atoms(500, 0, 0, 1_000, 0, 10_000);
    assert_eq!(res, Err(V16Error::CounterUnderflow));
}

#[test]
fn nav_rejects_fee_share_above_cap() {
    let res = lp_vault_nav_atoms(1_000, 0, 0, 0, 0, 10_001);
    assert_eq!(res, Err(V16Error::InvalidConfig));
}

#[test]
fn nav_is_independent_of_token_balance_donation_defense() {
    // SECURITY (Note 2): NAV depends ONLY on ledger counters. Two calls
    // with identical ledger inputs MUST produce identical NAV regardless
    // of any token balance "donation" — there is no balance input to this
    // function. A direct transfer into the bucket vault changes the SPL
    // token account balance but NONE of these ledger counters, so the
    // share price a depositor pays is unmoved. This test pins the API
    // shape that makes donation-inflation impossible by construction.
    let nav_a = lp_vault_nav_atoms(1_000_000, 5_000, 0, 0, 0, 10_000).unwrap();
    let nav_b = lp_vault_nav_atoms(1_000_000, 5_000, 0, 0, 0, 10_000).unwrap();
    assert_eq!(nav_a, nav_b);
    assert_eq!(nav_a, 1_005_000);
    // Shares an incoming 1_000-atom deposit receives is a pure function of
    // NAV + supply — the donation cannot move it.
    let shares_before = lp_shares_for_deposit(1_000, 1_000_000, nav_a).unwrap();
    let shares_after = lp_shares_for_deposit(1_000, 1_000_000, nav_b).unwrap();
    assert_eq!(shares_before, shares_after);
}

// ── lp_shares_for_deposit ───────────────────────────────────────────

#[test]
fn shares_fresh_vault_is_one_to_one() {
    // total_shares == 0 → 1:1
    let shares = lp_shares_for_deposit(5_000, 0, 0).unwrap();
    assert_eq!(shares, 5_000);
}

#[test]
fn shares_pro_rata_round_down() {
    // 1_000 deposit, 1_000_000 shares, NAV 2_000_000 → 1_000*1_000_000/2_000_000 = 500
    let shares = lp_shares_for_deposit(1_000, 1_000_000, 2_000_000).unwrap();
    assert_eq!(shares, 500);
}

#[test]
fn shares_round_down_truncates() {
    // 3 deposit, 2 shares, NAV 3 → 3*2/3 = 2 exact; 1 deposit → 1*2/3 = 0 (round down)
    assert_eq!(lp_shares_for_deposit(3, 2, 3).unwrap(), 2);
    assert_eq!(lp_shares_for_deposit(1, 2, 3).unwrap(), 0);
}

#[test]
fn shares_round_to_zero_returns_zero_wrapper_must_reject() {
    // Note 1: math returns 0 when amount*total < nav; wrapper rejects with
    // LpVaultZeroSharesMinted. Here we assert the math contract: it returns
    // 0 (not error), and the inflation case is detectable by the caller.
    let shares = lp_shares_for_deposit(1, 1_000_000, 1_000_000_000).unwrap();
    assert_eq!(shares, 0, "tiny deposit vs inflated NAV rounds to 0 — wrapper must reject");
}

#[test]
fn shares_zero_nav_with_outstanding_shares_is_error() {
    // shares exist but NAV wiped to 0 → cannot price; reject
    let res = lp_shares_for_deposit(1_000, 1_000_000, 0);
    assert_eq!(res, Err(V16Error::InvalidConfig));
}

// ── lp_atoms_for_redemption ─────────────────────────────────────────

#[test]
fn redeem_pro_rata_round_down() {
    // redeem 500 of 1_000_000 shares, NAV 2_000_000 → 500*2_000_000/1_000_000 = 1_000
    let atoms = lp_atoms_for_redemption(500, 1_000_000, 2_000_000).unwrap();
    assert_eq!(atoms, 1_000);
}

#[test]
fn redeem_round_down_keeps_dust() {
    // redeem 1 of 3 shares, NAV 5 → 1*5/3 = 1 (round down; vault keeps 0.67 dust)
    let atoms = lp_atoms_for_redemption(1, 3, 5).unwrap();
    assert_eq!(atoms, 1);
}

#[test]
fn redeem_all_shares_returns_all_nav() {
    // redeem entire supply → all NAV (modulo rounding when shares divide evenly)
    let atoms = lp_atoms_for_redemption(1_000, 1_000, 7_777).unwrap();
    assert_eq!(atoms, 7_777);
}

#[test]
fn redeem_more_than_supply_is_error() {
    let res = lp_atoms_for_redemption(1_001, 1_000, 5_000);
    assert_eq!(res, Err(V16Error::CounterUnderflow));
}

#[test]
fn redeem_zero_supply_is_error() {
    let res = lp_atoms_for_redemption(0, 0, 0);
    assert_eq!(res, Err(V16Error::InvalidConfig));
}

// ── deposit/redeem round-trip never profits ─────────────────────────

#[test]
fn deposit_then_immediate_redeem_never_profits() {
    // Adversarial: deposit 1_000 then immediately redeem the minted shares.
    // Round-DOWN on both sides guarantees redeemed <= deposited (no free money).
    let nav_pre: u128 = 3_333_333;
    let supply_pre: u128 = 7_777_777;
    let amount: u128 = 1_000;
    let shares = lp_shares_for_deposit(amount, supply_pre, nav_pre).unwrap();
    let nav_post = nav_pre + amount; // deposit added principal
    let supply_post = supply_pre + shares;
    let redeemed = lp_atoms_for_redemption(shares, supply_post, nav_post).unwrap();
    assert!(
        redeemed <= amount,
        "round-trip must not profit: deposited {amount}, redeemed {redeemed}"
    );
}

// ── lp_fee_split ────────────────────────────────────────────────────

#[test]
fn fee_split_full_to_lp() {
    let (lp, ins) = lp_fee_split(1_000, 10_000).unwrap();
    assert_eq!(lp, 1_000);
    assert_eq!(ins, 0);
}

#[test]
fn fee_split_half() {
    let (lp, ins) = lp_fee_split(1_000, 5_000).unwrap();
    assert_eq!(lp, 500);
    assert_eq!(ins, 500);
}

#[test]
fn fee_split_sums_to_delta() {
    // round-down on LP side; insurance gets remainder; always sums to delta
    let (lp, ins) = lp_fee_split(1_001, 3_333).unwrap();
    assert_eq!(lp + ins, 1_001);
}

#[test]
fn fee_split_rejects_bps_above_cap() {
    assert_eq!(lp_fee_split(1_000, 10_001), Err(V16Error::InvalidConfig));
}

// ── lp_redemption_cooldown_elapsed ──────────────────────────────────

#[test]
fn cooldown_zero_is_immediate() {
    assert!(lp_redemption_cooldown_elapsed(100, 100, 0));
}

#[test]
fn cooldown_not_elapsed_before_window() {
    assert!(!lp_redemption_cooldown_elapsed(100, 149, 50));
}

#[test]
fn cooldown_elapsed_at_window() {
    assert!(lp_redemption_cooldown_elapsed(100, 150, 50));
}

#[test]
fn cooldown_saturating_add_no_overflow() {
    // request_slot near u64::MAX + huge cooldown must saturate, not panic.
    // deadline = saturating_add(u64::MAX, 1000) = u64::MAX; current 100 < u64::MAX
    // → not elapsed. The point is no arithmetic overflow panic.
    assert!(!lp_redemption_cooldown_elapsed(u64::MAX, 100, 1_000));
    // when current reaches the saturated deadline (u64::MAX), it is elapsed
    assert!(lp_redemption_cooldown_elapsed(u64::MAX - 1, u64::MAX, 100));
}
