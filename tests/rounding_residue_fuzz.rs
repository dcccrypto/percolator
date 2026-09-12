//! ADL effective-quantity round-trip checks from upstream
//! tests/rounding_residue_fuzz.rs (6f36972d). Only the effective-quantity
//! tests are here; the rest of upstream's file covers mechanisms this fork
//! has not adopted yet. Built with `--features fuzz` like tests/v16_fuzzing.rs.
#![cfg(feature = "fuzz")]

use percolator::{
    kani_adl_effective_quantity_ceil, kani_mul_div_ceil_u128_or_wide,
    kani_mul_div_ceil_u128_wide_reference, kani_mul_div_floor_u128_or_wide,
    kani_mul_div_floor_u128_wide_reference, kani_prepare_source_credit_domain_recompute_for_epoch,
    kani_prepare_source_credit_domain_recompute_for_epoch_steps,
    kani_prepare_source_positive_claim_burn_delta, kani_raw_basis_for_adl_effective_quantity,
    SourceCreditStateV16, V16Error, V16Result, ADL_ONE, BOUND_SCALE, MAX_POSITION_ABS_Q,
    MIN_A_SIDE, POS_SCALE,
};
use proptest::prelude::*;

#[test]
fn adl_effective_quantity_roundtrip_boundary_partition() {
    let raw_values = [
        0,
        1,
        POS_SCALE - 1,
        POS_SCALE,
        POS_SCALE + 1,
        MAX_POSITION_ABS_Q - 1,
        MAX_POSITION_ABS_Q,
    ];
    let a_basis_values = [
        MIN_A_SIDE,
        MIN_A_SIDE + 1,
        ADL_ONE / 3,
        ADL_ONE / 2,
        ADL_ONE - 1,
        ADL_ONE,
    ];
    let current_a_values = [
        1,
        MIN_A_SIDE - 1,
        MIN_A_SIDE,
        MIN_A_SIDE + 1,
        ADL_ONE / 3,
        ADL_ONE / 2,
        ADL_ONE - 1,
        ADL_ONE,
    ];
    for raw_abs_q in raw_values {
        for a_basis in a_basis_values {
            for current_a in current_a_values.into_iter().filter(|a| *a <= a_basis) {
                let effective =
                    kani_adl_effective_quantity_ceil(raw_abs_q, a_basis, current_a).unwrap();
                let targets = [0, effective / 2, effective.saturating_sub(1)];
                for target_effective in targets {
                    if effective == 0 || target_effective >= effective {
                        continue;
                    }
                    let target_raw = kani_raw_basis_for_adl_effective_quantity(
                        target_effective,
                        a_basis,
                        current_a,
                    )
                    .unwrap();
                    assert!(target_raw <= raw_abs_q);
                    assert_eq!(
                        kani_adl_effective_quantity_ceil(target_raw, a_basis, current_a),
                        Ok(target_effective),
                    );
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20000))]

    #[test]
    fn adl_effective_quantity_roundtrip_preserves_any_reachable_reduction(
        raw_abs_q in 0u128..=MAX_POSITION_ABS_Q,
        a_basis in MIN_A_SIDE..=ADL_ONE,
        current_selector in any::<u128>(),
        target_selector in any::<u128>(),
    ) {
        let current_a = 1 + current_selector % a_basis;
        let current_effective =
            kani_adl_effective_quantity_ceil(raw_abs_q, a_basis, current_a).unwrap();
        let target_effective = if current_effective == 0 {
            0
        } else {
            target_selector % current_effective
        };
        let target_raw = kani_raw_basis_for_adl_effective_quantity(
            target_effective,
            a_basis,
            current_a,
        )
        .unwrap();

        prop_assert!(target_raw <= raw_abs_q);
        prop_assert_eq!(
            kani_adl_effective_quantity_ceil(target_raw, a_basis, current_a),
            Ok(target_effective),
        );
    }
}

/// INV-085 / source-credit CU fix: the native-u128 fast path and its U256
/// fallback must be exactly equivalent to the prior always-wide arithmetic.
/// This uses full-width operands, so it exercises both the checked native
/// branch and products that overflow u128 but fit in U256.
mod source_credit_fast_path_differential {
    use super::*;

    fn floor_reference(a: u128, b: u128, denominator: u128) -> V16Result<u128> {
        kani_mul_div_floor_u128_wide_reference(a, b, denominator)
    }

    fn ceil_reference(a: u128, b: u128, denominator: u128) -> V16Result<u128> {
        kani_mul_div_ceil_u128_wide_reference(a, b, denominator)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(8000))]

        #[test]
        fn fast_floor_and_ceil_equal_always_wide_reference(
            a in any::<u128>(),
            b in any::<u128>(),
            denominator in any::<u128>(),
        ) {
            prop_assert_eq!(
                kani_mul_div_floor_u128_or_wide(a, b, denominator),
                floor_reference(a, b, denominator),
            );
            prop_assert_eq!(
                kani_mul_div_ceil_u128_or_wide(a, b, denominator),
                ceil_reference(a, b, denominator),
            );
        }

        #[test]
        fn fused_claim_burn_changes_only_claim_totals(
            positive_claim_bound_num in any::<u128>(),
            exact_raw in any::<u128>(),
            burn_raw in any::<u128>(),
            other in any::<[u128; 9]>(),
            credit_epoch in any::<u64>(),
        ) {
            let exact_positive_claim_num = exact_raw.min(positive_claim_bound_num);
            let face_burn_num = burn_raw.min(positive_claim_bound_num);
            let source = SourceCreditStateV16 {
                positive_claim_bound_num,
                exact_positive_claim_num,
                fresh_reserved_backing_num: other[0],
                spent_backing_num: other[1],
                provider_receivable_num: other[2],
                valid_liened_backing_num: other[3],
                impaired_liened_backing_num: other[4],
                insurance_credit_reserved_num: other[5],
                valid_liened_insurance_num: other[6],
                impaired_liened_insurance_num: other[7],
                credit_rate_num: other[8],
                credit_epoch,
            };
            let mut expected = source;
            expected.positive_claim_bound_num -= face_burn_num;
            expected.exact_positive_claim_num -=
                face_burn_num.min(expected.exact_positive_claim_num);

            prop_assert_eq!(
                kani_prepare_source_positive_claim_burn_delta(source, face_burn_num),
                Ok(expected),
            );
        }

        #[test]
        fn fused_recompute_matches_two_step_legacy_epoch_and_rate(
            positive_atoms in 0u128..=1u128 << 40,
            exact_atoms_raw in 0u128..=1u128 << 40,
            burn_atoms_raw in 0u128..=1u128 << 40,
            backing_atoms in 0u128..=1u128 << 40,
            credit_epoch in 0u64..=u64::MAX - 2,
            risk_epoch in 0u64..=u64::MAX - 2,
        ) {
            let exact_atoms = exact_atoms_raw.min(positive_atoms);
            let burn_atoms = burn_atoms_raw.min(positive_atoms);
            let source = SourceCreditStateV16 {
                positive_claim_bound_num: positive_atoms * BOUND_SCALE,
                exact_positive_claim_num: exact_atoms * BOUND_SCALE,
                fresh_reserved_backing_num: backing_atoms * BOUND_SCALE,
                credit_rate_num: 17,
                credit_epoch,
                ..SourceCreditStateV16::EMPTY
            };
            let burn_num = burn_atoms * BOUND_SCALE;

            let (legacy_before_burn, legacy_risk_epoch) =
                kani_prepare_source_credit_domain_recompute_for_epoch(source, risk_epoch)
                    .expect("bounded valid source must recompute");
            let legacy_burned = kani_prepare_source_positive_claim_burn_delta(
                legacy_before_burn,
                burn_num,
            )
            .expect("bounded claim burn must fit");
            let legacy = kani_prepare_source_credit_domain_recompute_for_epoch(
                legacy_burned,
                legacy_risk_epoch,
            )
            .expect("second bounded recompute must fit");

            let fused_burned =
                kani_prepare_source_positive_claim_burn_delta(source, burn_num)
                    .expect("bounded fused claim burn must fit");
            let fused = kani_prepare_source_credit_domain_recompute_for_epoch_steps(
                fused_burned,
                risk_epoch,
                2,
            )
            .expect("two-step bounded recompute must fit");

            prop_assert_eq!(fused, legacy);
        }
    }

    #[test]
    fn fast_mul_div_boundary_partition_matches_wide_reference() {
        let values = [0, 1, 2, u128::MAX / 2, u128::MAX - 1, u128::MAX];
        let denominators = [0, 1, 2, (1u128 << 127) - 1, 1u128 << 127, u128::MAX];
        for a in values {
            for b in values {
                for denominator in denominators {
                    assert_eq!(
                        kani_mul_div_floor_u128_or_wide(a, b, denominator),
                        floor_reference(a, b, denominator),
                    );
                    assert_eq!(
                        kani_mul_div_ceil_u128_or_wide(a, b, denominator),
                        ceil_reference(a, b, denominator),
                    );
                }
            }
        }
    }

    #[test]
    fn fused_claim_burn_rejects_bound_underflow() {
        let source = SourceCreditStateV16 {
            positive_claim_bound_num: 7,
            exact_positive_claim_num: 5,
            ..SourceCreditStateV16::EMPTY
        };
        assert_eq!(
            kani_prepare_source_positive_claim_burn_delta(source, 8),
            Err(V16Error::CounterUnderflow),
        );
    }
}
