//! Kani proof harnesses for the v16 engine (contract + closure layers).
//!
//! This file is NON-PRODUCTION: it is compiled only under `cfg(kani)` with
//! the `contracts` or `closure` feature, and is included as a private child
//! module of `v16` so the harnesses can reach the engine's private items.
//! Keeping it out of v16.rs minimises the production audit surface there.
//! See scripts/contracts_runner.sh and scripts/spec-coverage.md.

#![allow(unused_imports)]

use super::*;
use crate::wide_math::{checked_mul_div_ceil_u256, U256};
use crate::{BOUND_SCALE, MAX_VAULT_TVL, V16_TOKEN_VALUE_CLASS_COUNT};

// ===================== KANI FUNCTION-CONTRACT LAYER =====================
// Built ONLY by scripts/contracts_runner.sh (cargo feature `contracts` +
// CLI -Z function-contracts + a separate CARGO_TARGET_DIR). The main proof
// suite never compiles this layer: the function-contracts pass slows
// kani-compiler ~5x crate-wide, and stub_verified composition havocs returns
// into ensures-constrained symbolic values (see the elimination table in
// tests/proofs_v16.rs, row (g)). The layer therefore holds LEAF contract
// checks only — machine-checked interface documentation that future kani
// versions may compose.
//
// PUBLIC-OP CONTRACT BOUNDARY (probe verdict, 2026-06-11): a contract on a
// public value-mover (deposit_not_atomic: requires + modifies(self.header,
// account.header) + old()-lockstep ensures) times out at 1800s when checked
// over SYMBOLIC value fields (2^64 range) with real account validation — the
// scale at which a contract would add power beyond the suite. At suite scale
// (u8-range constructed states) it would pass but adds no evidentiary weight
// over the existing suite proofs asserting identical postconditions. The
// review-proposed compositional public-contract program (frame contracts on
// every public op over arbitrary valid states) is therefore closed at this
// kani generation: the contract layer's power is leaf deltas + &mut-self
// in-place mutators (P5) + flow transits; public-op envelopes stay with the
// suite (constructed symbolic), closure layer (any-state deltas), fuzz
// (sequences), and runtime validation (every execution).
//
// NOTE: a contract on source_credit_lien_amounts_for_
// effective was dropped — combining proof_for_contract with a kani::stub of
// its U256 division helper is pathologically slow at the solver level (1800s+
// warm) while sibling checks take seconds; its full-rate property is covered
// by the standalone suite proofs. CONFIRMED PATTERN (second instance:
// prepare_insurance_lien_consume_delta, 1800s+ even with division-free
// ensures): leaves whose BODY divides by BOUND_SCALE with a symbolic operand
// are not contract-checkable in this toolchain — their delta semantics stay
// with the standalone suite proofs, which fix the operands concretely.

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(V16Core::prepare_counterparty_lien_consume_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_prepare_counterparty_lien_consume_delta() {
    let bucket = BackingBucketV16 {
        market_id: kani::any(),
        fresh_unliened_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        consumed_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        expiry_slot: kani::any(),
        status: BackingBucketStatusV16::Fresh,
        utilization_fee_earnings: kani::any(),
    };
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(bucket.valid_liened_backing_num < 1u128 << 96);
    kani::assume(bucket.consumed_liened_backing_num < 1u128 << 96);
    kani::assume(source.spent_backing_num < 1u128 << 96);
    kani::assume(source.provider_receivable_num < 1u128 << 96);
    let _ = V16Core::prepare_counterparty_lien_consume_delta(bucket, source, amount);
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(V16Core::prepare_counterparty_lien_create_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_prepare_counterparty_lien_create_delta() {
    let bucket = BackingBucketV16 {
        market_id: kani::any(),
        fresh_unliened_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        consumed_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        expiry_slot: kani::any(),
        status: BackingBucketStatusV16::Fresh,
        utilization_fee_earnings: kani::any(),
    };
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    let current_slot: u64 = kani::any();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(bucket.valid_liened_backing_num < 1u128 << 96);
    kani::assume(source.valid_liened_backing_num < 1u128 << 96);
    let _ = V16Core::prepare_counterparty_lien_create_delta(bucket, source, current_slot, amount);
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(V16Core::prepare_counterparty_backing_withdraw_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_prepare_counterparty_backing_withdraw_delta() {
    let bucket = BackingBucketV16 {
        market_id: kani::any(),
        fresh_unliened_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        consumed_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        expiry_slot: kani::any(),
        status: BackingBucketStatusV16::Fresh,
        utilization_fee_earnings: kani::any(),
    };
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    let amount: u128 = kani::any();
    let _ = V16Core::prepare_counterparty_backing_withdraw_delta(bucket, source, amount);
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(apply_backing_provider_earnings_withdraw)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_apply_backing_provider_earnings_withdraw() {
    let vault: u128 = kani::any();
    let earnings: u128 = kani::any();
    let amount: u128 = kani::any();
    let _ = apply_backing_provider_earnings_withdraw(vault, earnings, amount);
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(V16Core::prepare_counterparty_lien_release_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_prepare_counterparty_lien_release_delta() {
    let bucket = BackingBucketV16 {
        market_id: kani::any(),
        fresh_unliened_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        consumed_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        expiry_slot: kani::any(),
        status: BackingBucketStatusV16::Fresh,
        utilization_fee_earnings: kani::any(),
    };
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    let current_slot: u64 = kani::any();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(bucket.fresh_unliened_backing_num < 1u128 << 96);
    let _ = V16Core::prepare_counterparty_lien_release_delta(bucket, source, current_slot, amount);
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(V16Core::prepare_counterparty_lien_terminal_release_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_prepare_counterparty_lien_terminal_release_delta() {
    let bucket = BackingBucketV16 {
        market_id: kani::any(),
        fresh_unliened_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        consumed_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        expiry_slot: kani::any(),
        status: BackingBucketStatusV16::Fresh,
        utilization_fee_earnings: kani::any(),
    };
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(bucket.fresh_unliened_backing_num < 1u128 << 96);
    let _ = V16Core::prepare_counterparty_lien_terminal_release_delta(bucket, source, amount);
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(V16Core::prepare_counterparty_lien_impair_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_prepare_counterparty_lien_impair_delta() {
    let bucket = BackingBucketV16 {
        market_id: kani::any(),
        fresh_unliened_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        consumed_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        expiry_slot: kani::any(),
        status: BackingBucketStatusV16::Fresh,
        utilization_fee_earnings: kani::any(),
    };
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(bucket.impaired_liened_backing_num < 1u128 << 96);
    kani::assume(source.impaired_liened_backing_num < 1u128 << 96);
    let _ = V16Core::prepare_counterparty_lien_impair_delta(bucket, source, amount);
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(V16Core::prepare_counterparty_backing_add_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_prepare_counterparty_backing_add_delta() {
    let bucket = BackingBucketV16 {
        market_id: kani::any(),
        fresh_unliened_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        consumed_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        expiry_slot: kani::any(),
        status: BackingBucketStatusV16::Fresh,
        utilization_fee_earnings: kani::any(),
    };
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    let amount: u128 = kani::any();
    let current_slot: u64 = kani::any();
    let expiry_slot: u64 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(bucket.fresh_unliened_backing_num < 1u128 << 96);
    kani::assume(source.fresh_reserved_backing_num < 1u128 << 96);
    let _ = V16Core::prepare_counterparty_backing_add_delta(
        bucket, source, amount, current_slot, expiry_slot,
    );
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(V16Core::prepare_insurance_lien_create_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_prepare_insurance_lien_create_delta() {
    let reservation = InsuranceCreditReservationV16 {
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        consumed_insurance_num: kani::any(),
        source_credit_epoch: kani::any(),
    };
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(reservation.valid_liened_insurance_num < 1u128 << 96);
    kani::assume(source.valid_liened_insurance_num < 1u128 << 96);
    let _ = V16Core::prepare_insurance_lien_create_delta(reservation, source, amount);
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(V16Core::prepare_insurance_lien_release_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_prepare_insurance_lien_release_delta() {
    let reservation = InsuranceCreditReservationV16 {
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        consumed_insurance_num: kani::any(),
        source_credit_epoch: kani::any(),
    };
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    let _ = V16Core::prepare_insurance_lien_release_delta(reservation, source, amount);
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(V16Core::prepare_insurance_lien_impair_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_prepare_insurance_lien_impair_delta() {
    let reservation = InsuranceCreditReservationV16 {
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        consumed_insurance_num: kani::any(),
        source_credit_epoch: kani::any(),
    };
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(reservation.impaired_liened_insurance_num < 1u128 << 96);
    kani::assume(source.impaired_liened_insurance_num < 1u128 << 96);
    let _ = V16Core::prepare_insurance_lien_impair_delta(reservation, source, amount);
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(V16Core::prepare_insurance_lien_terminal_release_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_prepare_insurance_lien_terminal_release_delta() {
    let reservation = InsuranceCreditReservationV16 {
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        consumed_insurance_num: kani::any(),
        source_credit_epoch: kani::any(),
    };
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    let _ = V16Core::prepare_insurance_lien_terminal_release_delta(reservation, source, amount);
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(MarketGroupV16ViewMut::credit_account_from_insurance_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_credit_account_from_insurance_delta() {
    let insurance: u128 = kani::any();
    let budget_remaining: u128 = kani::any();
    let c_tot: u128 = kani::any();
    let capital: u128 = kani::any();
    let amount: u128 = kani::any();
    let _ = MarketGroupV16ViewMut::<Market<u64>>::credit_account_from_insurance_delta(
        insurance, budget_remaining, c_tot, capital, amount,
    );
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(V16Core::prepare_source_positive_claim_bound_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_prepare_source_positive_claim_bound_delta() {
    let source = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    let claim_bound_num: u128 = kani::any();
    let exact_claim_num: u128 = kani::any();
    kani::assume(exact_claim_num <= claim_bound_num);
    let _ = V16Core::prepare_source_positive_claim_bound_delta(
        source, claim_bound_num, exact_claim_num,
    );
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(MarketGroupV16ViewMut::apply_total_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_apply_total_delta() {
    let total: u128 = kani::any();
    let old: u128 = kani::any();
    let new: u128 = kani::any();
    let _ = MarketGroupV16ViewMut::<Market<u64>>::apply_total_delta(total, old, new);
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(MarketGroupV16ViewMut::trade_signed_size_deltas)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_trade_signed_size_deltas() {
    let size_q: i128 = kani::any();
    let _ = MarketGroupV16ViewMut::<Market<u64>>::trade_signed_size_deltas(size_q);
}

// Flow-typing witness (plain proof: proof_for_contract cannot handle the
// array-bearing return type — unbounded write-set havoc loop; same
// postconditions asserted directly over the full input domain).
#[cfg(all(kani, feature = "contracts"))]
#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn contract_check_flow_external_in_to_account_capital() {
    let amount: u128 = kani::any();
    let vault_before: u128 = kani::any();
    let vault_after = vault_before.wrapping_add(amount);
    kani::assume(vault_after >= vault_before);
    if let Ok(p) = TokenValueFlowProofV16::external_in_to_account_capital(amount, vault_before, vault_after) {
        let mut ed = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        let mut ec = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        ed[TokenValueClassV16::AccountCapital as usize] = amount;
        ec[TokenValueClassV16::ExternalQuote as usize] = amount;
        let mut i = 0;
        while i < V16_TOKEN_VALUE_CLASS_COUNT {
            assert!(p.debits[i] == ed[i]);
            assert!(p.credits[i] == ec[i]);
            i += 1;
        }
        assert_eq!(p.external_quote_in, amount);
        assert_eq!(p.external_quote_out, 0);
        assert_eq!(p.vault_before, vault_before);
        assert_eq!(p.vault_after, vault_after);
        assert_eq!(p.validate(), Ok(()));
    }
}

// Flow-typing witness (plain proof: proof_for_contract cannot handle the
// array-bearing return type — unbounded write-set havoc loop; same
// postconditions asserted directly over the full input domain).
#[cfg(all(kani, feature = "contracts"))]
#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn contract_check_flow_account_capital_to_external_out() {
    let amount: u128 = kani::any();
    let vault_before: u128 = kani::any();
    kani::assume(vault_before >= amount);
    let vault_after = vault_before - amount;
    if let Ok(p) = TokenValueFlowProofV16::account_capital_to_external_out(amount, vault_before, vault_after) {
        let mut ed = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        let mut ec = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        ed[TokenValueClassV16::AccountCapital as usize] = amount;
        ec[TokenValueClassV16::ExternalQuote as usize] = amount;
        let mut i = 0;
        while i < V16_TOKEN_VALUE_CLASS_COUNT {
            assert!(p.debits[i] == ed[i]);
            assert!(p.credits[i] == ec[i]);
            i += 1;
        }
        assert_eq!(p.external_quote_in, 0);
        assert_eq!(p.external_quote_out, amount);
        assert_eq!(p.vault_before, vault_before);
        assert_eq!(p.vault_after, vault_after);
        assert_eq!(p.validate(), Ok(()));
    }
}

// Flow-typing witness (plain proof: proof_for_contract cannot handle the
// array-bearing return type — unbounded write-set havoc loop; same
// postconditions asserted directly over the full input domain).
#[cfg(all(kani, feature = "contracts"))]
#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn contract_check_flow_account_capital_to_insurance() {
    let amount: u128 = kani::any();
    let vault_before: u128 = kani::any();
    let vault_after = vault_before; // internal relabel: vault flat
    if let Ok(p) = TokenValueFlowProofV16::account_capital_to_insurance(amount, vault_before, vault_after) {
        let mut ed = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        let mut ec = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        ed[TokenValueClassV16::AccountCapital as usize] = amount;
        ec[TokenValueClassV16::InsuranceCapital as usize] = amount;
        let mut i = 0;
        while i < V16_TOKEN_VALUE_CLASS_COUNT {
            assert!(p.debits[i] == ed[i]);
            assert!(p.credits[i] == ec[i]);
            i += 1;
        }
        assert_eq!(p.external_quote_in, 0);
        assert_eq!(p.external_quote_out, 0);
        assert_eq!(p.vault_before, vault_before);
        assert_eq!(p.vault_after, vault_after);
        assert_eq!(p.validate(), Ok(()));
    }
}

// Flow-typing witness (plain proof: proof_for_contract cannot handle the
// array-bearing return type — unbounded write-set havoc loop; same
// postconditions asserted directly over the full input domain).
#[cfg(all(kani, feature = "contracts"))]
#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn contract_check_flow_external_in_to_insurance_capital() {
    let amount: u128 = kani::any();
    let vault_before: u128 = kani::any();
    let vault_after = vault_before.wrapping_add(amount);
    kani::assume(vault_after >= vault_before);
    if let Ok(p) = TokenValueFlowProofV16::external_in_to_insurance_capital(amount, vault_before, vault_after) {
        let mut ed = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        let mut ec = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        ed[TokenValueClassV16::InsuranceCapital as usize] = amount;
        ec[TokenValueClassV16::ExternalQuote as usize] = amount;
        let mut i = 0;
        while i < V16_TOKEN_VALUE_CLASS_COUNT {
            assert!(p.debits[i] == ed[i]);
            assert!(p.credits[i] == ec[i]);
            i += 1;
        }
        assert_eq!(p.external_quote_in, amount);
        assert_eq!(p.external_quote_out, 0);
        assert_eq!(p.vault_before, vault_before);
        assert_eq!(p.vault_after, vault_after);
        assert_eq!(p.validate(), Ok(()));
    }
}

// Flow-typing witness (plain proof: proof_for_contract cannot handle the
// array-bearing return type — unbounded write-set havoc loop; same
// postconditions asserted directly over the full input domain).
#[cfg(all(kani, feature = "contracts"))]
#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn contract_check_flow_insurance_capital_to_external_out() {
    let amount: u128 = kani::any();
    let vault_before: u128 = kani::any();
    kani::assume(vault_before >= amount);
    let vault_after = vault_before - amount;
    if let Ok(p) = TokenValueFlowProofV16::insurance_capital_to_external_out(amount, vault_before, vault_after) {
        let mut ed = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        let mut ec = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        ed[TokenValueClassV16::InsuranceCapital as usize] = amount;
        ec[TokenValueClassV16::ExternalQuote as usize] = amount;
        let mut i = 0;
        while i < V16_TOKEN_VALUE_CLASS_COUNT {
            assert!(p.debits[i] == ed[i]);
            assert!(p.credits[i] == ec[i]);
            i += 1;
        }
        assert_eq!(p.external_quote_in, 0);
        assert_eq!(p.external_quote_out, amount);
        assert_eq!(p.vault_before, vault_before);
        assert_eq!(p.vault_after, vault_after);
        assert_eq!(p.validate(), Ok(()));
    }
}

// Flow-typing witness (plain proof: proof_for_contract cannot handle the
// array-bearing return type — unbounded write-set havoc loop; same
// postconditions asserted directly over the full input domain).
#[cfg(all(kani, feature = "contracts"))]
#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn contract_check_flow_insurance_capital_to_account_capital() {
    let amount: u128 = kani::any();
    let vault_before: u128 = kani::any();
    let vault_after = vault_before; // internal relabel: vault flat
    if let Ok(p) = TokenValueFlowProofV16::insurance_capital_to_account_capital(amount, vault_before, vault_after) {
        let mut ed = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        let mut ec = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        ed[TokenValueClassV16::InsuranceCapital as usize] = amount;
        ec[TokenValueClassV16::AccountCapital as usize] = amount;
        let mut i = 0;
        while i < V16_TOKEN_VALUE_CLASS_COUNT {
            assert!(p.debits[i] == ed[i]);
            assert!(p.credits[i] == ec[i]);
            i += 1;
        }
        assert_eq!(p.external_quote_in, 0);
        assert_eq!(p.external_quote_out, 0);
        assert_eq!(p.vault_before, vault_before);
        assert_eq!(p.vault_after, vault_after);
        assert_eq!(p.validate(), Ok(()));
    }
}

// Flow-typing witness (plain proof: proof_for_contract cannot handle the
// array-bearing return type — unbounded write-set havoc loop; same
// postconditions asserted directly over the full input domain).
#[cfg(all(kani, feature = "contracts"))]
#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn contract_check_flow_account_capital_to_realized_loss() {
    let amount: u128 = kani::any();
    let vault_before: u128 = kani::any();
    let vault_after = vault_before; // internal relabel: vault flat
    if let Ok(p) = TokenValueFlowProofV16::account_capital_to_realized_loss(amount, vault_before, vault_after) {
        let mut ed = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        let mut ec = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        ed[TokenValueClassV16::AccountCapital as usize] = amount;
        ec[TokenValueClassV16::ExplicitBackedLoss as usize] = amount;
        let mut i = 0;
        while i < V16_TOKEN_VALUE_CLASS_COUNT {
            assert!(p.debits[i] == ed[i]);
            assert!(p.credits[i] == ec[i]);
            i += 1;
        }
        assert_eq!(p.external_quote_in, 0);
        assert_eq!(p.external_quote_out, 0);
        assert_eq!(p.vault_before, vault_before);
        assert_eq!(p.vault_after, vault_after);
        assert_eq!(p.validate(), Ok(()));
    }
}

// Flow-typing witness (plain proof: proof_for_contract cannot handle the
// array-bearing return type — unbounded write-set havoc loop; same
// postconditions asserted directly over the full input domain).
#[cfg(all(kani, feature = "contracts"))]
#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn contract_check_flow_insurance_to_close_insurance_spent() {
    let amount: u128 = kani::any();
    let vault_before: u128 = kani::any();
    let vault_after = vault_before; // internal relabel: vault flat
    if let Ok(p) = TokenValueFlowProofV16::insurance_to_close_insurance_spent(amount, vault_before, vault_after) {
        let mut ed = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        let mut ec = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        ed[TokenValueClassV16::InsuranceCapital as usize] = amount;
        ec[TokenValueClassV16::CloseInsuranceSpent as usize] = amount;
        let mut i = 0;
        while i < V16_TOKEN_VALUE_CLASS_COUNT {
            assert!(p.debits[i] == ed[i]);
            assert!(p.credits[i] == ec[i]);
            i += 1;
        }
        assert_eq!(p.external_quote_in, 0);
        assert_eq!(p.external_quote_out, 0);
        assert_eq!(p.vault_before, vault_before);
        assert_eq!(p.vault_after, vault_after);
        assert_eq!(p.validate(), Ok(()));
    }
}

// The multi-leg flow transits are the VALUE SKELETONS of the Kani-intractable
// public bodies: close_cure_to_account_capital is the cure path's only value
// move, support_to_account_capital the resolved-close support conversion's,
// capital_and_resolved_payout_to_external_out the resolved withdrawal's. The
// engine constructs and validate()s one of these on every execution of those
// bodies, so full-domain witnesses here + the per-leaf delta contracts close
// the conservation argument for the intractable tier.
#[cfg(all(kani, feature = "contracts"))]
#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn contract_check_flow_close_cure_to_account_capital() {
    let deposit: u128 = kani::any();
    let escrow: u128 = kani::any();
    let vault_before: u128 = kani::any();
    let capital_credit = match deposit.checked_add(escrow) {
        Some(v) => v,
        None => return,
    };
    let vault_after = vault_before.wrapping_add(deposit);
    kani::assume(vault_after >= vault_before);
    if let Ok(p) = TokenValueFlowProofV16::close_cure_to_account_capital(
        deposit, escrow, capital_credit, vault_before, vault_after,
    ) {
        let mut ed = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        let mut ec = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        ec[TokenValueClassV16::ExternalQuote as usize] = deposit;
        ec[TokenValueClassV16::CancelDepositEscrow as usize] = escrow;
        ed[TokenValueClassV16::AccountCapital as usize] = capital_credit;
        let mut i = 0;
        while i < V16_TOKEN_VALUE_CLASS_COUNT {
            assert!(p.debits[i] == ed[i]);
            assert!(p.credits[i] == ec[i]);
            i += 1;
        }
        assert_eq!(p.external_quote_in, deposit);
        assert_eq!(p.external_quote_out, 0);
        // The cure credits the account with exactly deposit + escrow and the
        // vault rises by exactly the external deposit: no value minted.
        assert_eq!(p.validate(), Ok(()));
    }
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn contract_check_flow_support_to_account_capital() {
    let cp: u128 = kani::any();
    let ins: u128 = kani::any();
    let surplus: u128 = kani::any();
    let vault_before: u128 = kani::any();
    let credit = match cp.checked_add(ins).and_then(|v| v.checked_add(surplus)) {
        Some(v) => v,
        None => return,
    };
    if let Ok(p) = TokenValueFlowProofV16::support_to_account_capital(
        credit, cp, ins, surplus, vault_before, vault_before,
    ) {
        let mut ed = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        let mut ec = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        ec[TokenValueClassV16::CloseCounterpartyCreditConsumed as usize] = cp;
        ec[TokenValueClassV16::CloseInsuranceSpent as usize] = ins;
        ec[TokenValueClassV16::UnallocatedProtocolSurplus as usize] = surplus;
        ed[TokenValueClassV16::AccountCapital as usize] = credit;
        let mut i = 0;
        while i < V16_TOKEN_VALUE_CLASS_COUNT {
            assert!(p.debits[i] == ed[i]);
            assert!(p.credits[i] == ec[i]);
            i += 1;
        }
        assert_eq!(p.external_quote_in, 0);
        assert_eq!(p.external_quote_out, 0);
        // Support conversion is internally funded (vault flat): the winner's
        // capital credit is exactly the sum of the three consumed sources.
        assert_eq!(p.validate(), Ok(()));
    }
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof]
#[kani::unwind(40)]
#[kani::solver(cadical)]
fn contract_check_flow_capital_and_resolved_payout_to_external_out() {
    let capital_paid: u128 = kani::any();
    let payout_paid: u128 = kani::any();
    let vault_before: u128 = kani::any();
    let total = match capital_paid.checked_add(payout_paid) {
        Some(v) => v,
        None => return,
    };
    kani::assume(vault_before >= total);
    let vault_after = vault_before - total;
    if let Ok(p) = TokenValueFlowProofV16::capital_and_resolved_payout_to_external_out(
        capital_paid, payout_paid, total, vault_before, vault_after,
    ) {
        let mut ed = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        let mut ec = [0u128; V16_TOKEN_VALUE_CLASS_COUNT];
        ed[TokenValueClassV16::AccountCapital as usize] = capital_paid;
        ed[TokenValueClassV16::ResolvedPayoutPaid as usize] = payout_paid;
        ec[TokenValueClassV16::ExternalQuote as usize] = total;
        let mut i = 0;
        while i < V16_TOKEN_VALUE_CLASS_COUNT {
            assert!(p.debits[i] == ed[i]);
            assert!(p.credits[i] == ec[i]);
            i += 1;
        }
        assert_eq!(p.external_quote_in, 0);
        assert_eq!(p.external_quote_out, total);
        // A resolved exit pays out exactly capital + receipt claim and the
        // vault falls by exactly that total: nothing else can leave.
        assert_eq!(p.validate(), Ok(()));
    }
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(MarketGroupV16ViewMut::withdraw_domain_insurance_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_withdraw_domain_insurance_delta() {
    let _ = MarketGroupV16ViewMut::<Market<u64>>::withdraw_domain_insurance_delta(
        kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any(),
    );
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(MarketGroupV16ViewMut::credit_backing_provider_earnings_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_credit_backing_provider_earnings_delta() {
    let _ = MarketGroupV16ViewMut::<Market<u64>>::credit_backing_provider_earnings_delta(
        kani::any(), kani::any(), kani::any(), kani::any(), kani::any(), kani::any(),
    );
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(MarketGroupV16ViewMut::set_domain_insurance_spent_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_set_domain_insurance_spent_delta() {
    let total_remaining: u128 = kani::any();
    let insurance: u128 = kani::any();
    let budget: u128 = kani::any();
    let old_spent: u128 = kani::any();
    let new_spent: u128 = kani::any();
    kani::assume(old_spent <= budget && new_spent <= budget);
    let _ = MarketGroupV16ViewMut::<Market<u64>>::set_domain_insurance_spent_delta(
        total_remaining, insurance, budget, old_spent, new_spent,
    );
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(MarketGroupV16ViewMut::set_domain_insurance_budget_delta)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_set_domain_insurance_budget_delta() {
    let total_remaining: u128 = kani::any();
    let insurance_limit: u128 = kani::any();
    let old_budget: u128 = kani::any();
    let spent: u128 = kani::any();
    let new_budget: u128 = kani::any();
    kani::assume(spent <= old_budget && spent <= new_budget);
    let _ = MarketGroupV16ViewMut::<Market<u64>>::set_domain_insurance_budget_delta(
        total_remaining, insurance_limit, old_budget, spent, new_budget,
    );
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(V16Core::available_backing_num_for_source_credit_state)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_available_backing_num_for_source_credit_state() {
    let state = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    let _ = V16Core::available_backing_num_for_source_credit_state(state);
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(V16Core::health_requirements_from_base_and_target_lag)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_health_requirements_from_base_and_target_lag() {
    let _ = V16Core::health_requirements_from_base_and_target_lag(
        kani::any(), kani::any(), kani::any(), kani::any(),
    );
}

// ============ P0: ENCUMBRANCE-CLOSURE INDUCTION ============
// The per-domain ledger invariant (the div-free cross-ledger equalities of
// validate_source_domain_ledger_parts; the credit-rate equality is excluded
// here because it is intentionally broken by deltas and restored by the
// recompute step, which the suite proves separately). Each closure harness
// proves: for EVERY state satisfying inv (not just constructed ones), the
// delta preserves inv. With the genesis proof, any sequence of deltas
// preserves ledger validity — induction, not reachability-by-construction.
#[cfg(all(kani, feature = "closure"))]
fn kani_ledger_inv(
    b: &BackingBucketV16,
    s: &SourceCreditStateV16,
    r: &InsuranceCreditReservationV16,
) -> bool {
    s.fresh_reserved_backing_num
        == b.fresh_unliened_backing_num + b.valid_liened_backing_num
        && s.provider_receivable_num == b.consumed_liened_backing_num
        && s.valid_liened_backing_num == b.valid_liened_backing_num
        && s.impaired_liened_backing_num == b.impaired_liened_backing_num
        && s.insurance_credit_reserved_num == r.insurance_credit_reserved_num
        && s.valid_liened_insurance_num == r.valid_liened_insurance_num
        && s.impaired_liened_insurance_num == r.impaired_liened_insurance_num
        && s.spent_backing_num >= s.provider_receivable_num
}

#[cfg(all(kani, feature = "closure"))]
fn kani_any_ledger_triple() -> (BackingBucketV16, SourceCreditStateV16, InsuranceCreditReservationV16) {
    let b = BackingBucketV16 {
        market_id: kani::any(),
        fresh_unliened_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        consumed_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        expiry_slot: kani::any(),
        status: kani::any(),
        utilization_fee_earnings: kani::any(),
    };
    let s = SourceCreditStateV16 {
        positive_claim_bound_num: kani::any(),
        exact_positive_claim_num: kani::any(),
        fresh_reserved_backing_num: kani::any(),
        valid_liened_backing_num: kani::any(),
        impaired_liened_backing_num: kani::any(),
        spent_backing_num: kani::any(),
        provider_receivable_num: kani::any(),
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        credit_rate_num: kani::any(),
        credit_epoch: kani::any(),
    };
    let r = InsuranceCreditReservationV16 {
        insurance_credit_reserved_num: kani::any(),
        valid_liened_insurance_num: kani::any(),
        impaired_liened_insurance_num: kani::any(),
        consumed_insurance_num: kani::any(),
        source_credit_epoch: kani::any(),
    };
    // Overflow headroom so inv's additions and the deltas' checked ops stay
    // in-range; production magnitudes are < 2^93 (MAX_VAULT_TVL * BOUND_SCALE).
    kani::assume(b.fresh_unliened_backing_num < 1u128 << 96);
    kani::assume(b.valid_liened_backing_num < 1u128 << 96);
    kani::assume(b.consumed_liened_backing_num < 1u128 << 96);
    kani::assume(b.impaired_liened_backing_num < 1u128 << 96);
    kani::assume(s.spent_backing_num < 1u128 << 96);
    kani::assume(s.positive_claim_bound_num < 1u128 << 96);
    kani::assume(s.exact_positive_claim_num < 1u128 << 96);
    kani::assume(r.insurance_credit_reserved_num < 1u128 << 96);
    kani::assume(r.valid_liened_insurance_num < 1u128 << 96);
    kani::assume(r.impaired_liened_insurance_num < 1u128 << 96);
    kani::assume(r.consumed_insurance_num < 1u128 << 96);
    (b, s, r)
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
fn closure_ledger_inv_genesis() {
    let b = BackingBucketV16::EMPTY;
    let s = SourceCreditStateV16::EMPTY;
    let r = InsuranceCreditReservationV16::EMPTY;
    assert!(kani_ledger_inv(&b, &s, &r));
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_ledger_inv_prepare_counterparty_lien_create_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    if let Ok((b2, s2)) = V16Core::prepare_counterparty_lien_create_delta(b, s, kani::any(), amount) {
        // Reservation untouched by counterparty deltas.
        assert!(kani_ledger_inv(&b2, &s2, &r));
    }
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_ledger_inv_prepare_counterparty_lien_release_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    if let Ok((b2, s2)) = V16Core::prepare_counterparty_lien_release_delta(b, s, kani::any(), amount) {
        // Reservation untouched by counterparty deltas.
        assert!(kani_ledger_inv(&b2, &s2, &r));
    }
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_ledger_inv_prepare_counterparty_lien_terminal_release_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    if let Ok((b2, s2)) = V16Core::prepare_counterparty_lien_terminal_release_delta(b, s, amount) {
        // Reservation untouched by counterparty deltas.
        assert!(kani_ledger_inv(&b2, &s2, &r));
    }
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_ledger_inv_prepare_counterparty_lien_consume_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    if let Ok((b2, s2)) = V16Core::prepare_counterparty_lien_consume_delta(b, s, amount) {
        // Reservation untouched by counterparty deltas.
        assert!(kani_ledger_inv(&b2, &s2, &r));
    }
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_ledger_inv_prepare_counterparty_lien_impair_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    if let Ok((b2, s2)) = V16Core::prepare_counterparty_lien_impair_delta(b, s, amount) {
        // Reservation untouched by counterparty deltas.
        assert!(kani_ledger_inv(&b2, &s2, &r));
    }
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_ledger_inv_prepare_counterparty_backing_withdraw_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    if let Ok((b2, s2)) = V16Core::prepare_counterparty_backing_withdraw_delta(b, s, amount) {
        // Reservation untouched by counterparty deltas.
        assert!(kani_ledger_inv(&b2, &s2, &r));
    }
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_ledger_inv_prepare_counterparty_backing_add_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    if let Ok((b2, s2)) = V16Core::prepare_counterparty_backing_add_delta(b, s, amount, kani::any(), kani::any()) {
        // Reservation untouched by counterparty deltas.
        assert!(kani_ledger_inv(&b2, &s2, &r));
    }
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_ledger_inv_prepare_insurance_lien_create_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    if let Ok((r2, s2)) = V16Core::prepare_insurance_lien_create_delta(r, s, amount) {
        // Bucket untouched by insurance deltas.
        assert!(kani_ledger_inv(&b, &s2, &r2));
    }
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_ledger_inv_prepare_insurance_lien_release_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    if let Ok((r2, s2)) = V16Core::prepare_insurance_lien_release_delta(r, s, amount) {
        // Bucket untouched by insurance deltas.
        assert!(kani_ledger_inv(&b, &s2, &r2));
    }
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_ledger_inv_prepare_insurance_lien_terminal_release_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    if let Ok((r2, s2)) = V16Core::prepare_insurance_lien_terminal_release_delta(r, s, amount) {
        // Bucket untouched by insurance deltas.
        assert!(kani_ledger_inv(&b, &s2, &r2));
    }
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_ledger_inv_prepare_insurance_lien_impair_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    if let Ok((r2, s2)) = V16Core::prepare_insurance_lien_impair_delta(r, s, amount) {
        // Bucket untouched by insurance deltas.
        assert!(kani_ledger_inv(&b, &s2, &r2));
    }
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_ledger_inv_prepare_insurance_lien_consume_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    let domain_spent: u128 = kani::any();
    let insurance: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(domain_spent < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    if let Ok((r2, s2, _ds, _ins)) =
        V16Core::prepare_insurance_lien_consume_delta(r, s, domain_spent, insurance, amount)
    {
        assert!(kani_ledger_inv(&b, &s2, &r2));
    }
}

// ============ P4: BUCKET STATUS-MACHINE CLOSURE ============
// validate_backing_bucket_static encodes the spec's bucket lifecycle diagram
// as per-status amount-shape rules (Empty must be value-free, Fresh must be
// funded and unexpired-shaped, Expired/Impaired must hold only their
// respective residue classes). Closure: ANY bucket passing the validator,
// under ANY delta that succeeds, still passes — no delta can produce an
// undiagrammed status/shape combination.
//
// SCOPE (finding, 2026-06-11): delta-level status closure holds for create,
// release, terminal-release, and impair. It does NOT hold per-delta for
// consume / backing-withdraw / backing-add: those normalize bucket status at
// the PUBLIC-OP boundary (validate_shape), not after each internal delta —
// e.g. consuming the last valid lien off a bucket that still carries impaired
// liens leaves a transient Fresh-with-zero-active shape that the surrounding
// op rejects or normalizes before returning. Evidence it is boundary-enforced
// and not a reachable bug: the 400-case full-close conservation fuzz passes
// validate_shape across entire realize sequences. The per-op invariant for
// these three is covered by the suite's validate_shape post-assertions.

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_bucket_status_machine_prepare_counterparty_lien_create_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    kani::assume(V16Core::validate_backing_bucket_static(b) == Ok(()));
    if let Ok((b2, _s2)) = V16Core::prepare_counterparty_lien_create_delta(b, s, kani::any(), amount) {
        assert_eq!(V16Core::validate_backing_bucket_static(b2), Ok(()));
    }
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_bucket_status_machine_prepare_counterparty_lien_release_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    kani::assume(V16Core::validate_backing_bucket_static(b) == Ok(()));
    if let Ok((b2, _s2)) = V16Core::prepare_counterparty_lien_release_delta(b, s, kani::any(), amount) {
        assert_eq!(V16Core::validate_backing_bucket_static(b2), Ok(()));
    }
}

#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_bucket_status_machine_prepare_counterparty_lien_terminal_release_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    kani::assume(V16Core::validate_backing_bucket_static(b) == Ok(()));
    if let Ok((b2, _s2)) = V16Core::prepare_counterparty_lien_terminal_release_delta(b, s, amount) {
        assert_eq!(V16Core::validate_backing_bucket_static(b2), Ok(()));
    }
}


#[cfg(all(kani, feature = "closure"))]
#[kani::proof]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn closure_bucket_status_machine_prepare_counterparty_lien_impair_delta() {
    let (b, s, r) = kani_any_ledger_triple();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    kani::assume(kani_ledger_inv(&b, &s, &r));
    kani::assume(V16Core::validate_backing_bucket_static(b) == Ok(()));
    if let Ok((b2, _s2)) = V16Core::prepare_counterparty_lien_impair_delta(b, s, amount) {
        assert_eq!(V16Core::validate_backing_bucket_static(b2), Ok(()));
    }
}

#[cfg(all(kani, feature = "contracts"))]
#[kani::proof_for_contract(TokenValueFlowProofV16::debit)]
#[kani::unwind(8)]
#[kani::solver(cadical)]
fn contract_check_flow_proof_debit_modifies() {
    let mut p = TokenValueFlowProofV16::empty(kani::any(), kani::any());
    let class: TokenValueClassV16 = kani::any();
    let amount: u128 = kani::any();
    kani::assume(amount < 1u128 << 96);
    let _ = p.debit(class, amount);
}
