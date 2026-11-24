# Open gaps after #402 (24 Nov)

Known, accepted for now, not fixed in this PR. Each needs its own change.

- G1. Insurance-backed same-leg reversal (`source_reclass_same_leg_reversal_insurance_backed`,
  tests/v16_spec_tests.rs). Under upstream 7d2e243d the reversal spends no insurance, so the
  short's regained 5,000 sits in the loss domain at credit rate 0 with no backing while insurance
  keeps its 100,000. The claim is not backed. The test pins this. Nothing upstream changes it
  yet (checked av/master a6c6e9ad).
- G2. Pre-#250 stored rate. `validate_source_credit_state_static` also accepts the pre-#250
  value (see `pre_250_source_credit_rate_num_for_state` for when that ends). percolator-prog
  still computes that value (`expected_source_credit_rate_num`) and should move to the current
  rule; then the acceptance is removed.
- G3. SBF frames. The engine rlib builds for SBF; stack frames are only checked when
  percolator-prog links it (its SBF stack-frame gate). Not run against this head here.
- G4. The insurance twin of `v16_live_mark_reversal_unwinds_*` pins 5/3 epoch steps, the
  de-fused signature, so only the counterparty twin (8/4) guards the fused burn. Checked 24 Nov:
  with both call sites de-fused the counterparty twin reads 7 and fails.

Not a gap: percolator-prog depends on the engine with `fork-facade` only and never turns on
`fuzz`, so `pub mod v16` under `fuzz` is not exported to production builds.
