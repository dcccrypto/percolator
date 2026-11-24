# Stage C (J-20261120-ENG-PORT-TOLYNETTINGe, finished by ...g)

Goal (Khubair 21:13 GMT 20 Nov, re-run 19:09 GMT 21 Nov): port the rest of Toly's netting chain, in order,
keeping his code and tests: #250, c3988149, #206, 1683b11d. Then re-test the R1 files and the full engine suite.

## 1. Ports (all on the branch)

| upstream | ours | adaptations (full list in each commit message) |
|---|---|---|
| c7ae2256 #250 lien-aware credit rate | `67b2a0b6` | v16_proofs.rs / independent_backing_payout_properties.rs not in this fork (hunks dropped); spec placed in the v16.8.3 layout; test passes the fork's 4th trade arg; kani-only helpers widened to `fuzz` so the test's `rate_kernel` module builds. R1's S5 protective rate is left as is. |
| 596b3d98 #250 watermark | `821e2bb7` | its proofs_v16.rs harness is not in this fork (hunk dropped). |
| c3988149 resolved surplus withdrawal | `be1f3d90` | v16_kani_api.rs not in this fork; kf_world stub replaced by the real API. |
| c85b148b #206 lapsed buckets impair liens | `09ca70e1` + `82593721` | the two new tests pass `taker_is_long_account = true`, fee 0. |
| 1683b11d liened face pays the loss domain | `9d1499a6` | `kf_loss_domain` added beside the fork's #299 provenance arg; `loser_paid_backing_num` reads the S10 provider-principal mirror; booking goes through the fork's `book_realized_loss_backing_for_domain_not_atomic`. `tests/v16_kf_loss_liened_face.rs` is byte-identical to upstream. |
| review round 1 wording | `40db7414` | doc and spec only. |
| merge of feat/v22 @ 67a4c5b9 (#382 #383 #384) | `342dae14` | spec.md conflict: both paragraphs kept. With #382 the S10 mirror falls on expiry and impairment like upstream's principal, so the "known difference" note on `loser_paid_backing_num` was dropped. |

## 2. Test runs on 342dae14

Reserved lane, one detached run: `cargo test -j 4 --release --config profile.release.lto=false
--config profile.release.codegen-units=16 --no-fail-fast` (CARGO_TARGET_DIR=/tmp/jnettingg-target).
lto is turned off only to shorten the build. Logs: `stageC2-r1.txt`, `stageC2-full-suite.txt`.

### 2a. v22_r1_* (5 files): 33 passed / 9 failed / 1 ignored

| file | passed / failed |
|---|---|
| v22_r1_deep_book | 2 / 1 |
| v22_r1_i457 | 4 / 1 |
| v22_r1_multi_claimant | 2 / 2 |
| v22_r1_reversal | 21 / 1 (1 ignored) |
| v22_r1_settle_order | 4 / 4 |

These are the same 9 tests that failed on 9d1499a6 (`stageC-r1.txt`). Stage B1 had 10 failures on 2cb08797
(STAGE-B1.md section 4); the one that now passes is `v22_r1_deep_book::liened_domain_multi_claimant_ratchets`.
Got values on this head (expected = test):

| test | assertion | expected | got on 342dae14 |
|---|---|---|---|
| deep_book::deep_book_maker_is_never_below_the_pre_r1_worst_order | sum of maker's worst order (372 worlds, 360 liened) | >= -78,538,861 | -124,633,648 (the same number #311 measured for #250 alone) |
| i457::i457_resolved_close_order_does_not_change_payouts | payouts, close order reversed (peak 0,1,2) | equal | [0, 494527388, 1185878296, 1000000619594320] vs [0, 494527386, 1217566000, 1000000725220000] (unchanged from Stage B1) |
| multi_claimant::mixed_liened_and_rich_flippers_ratchets | rich flipper inside the protective branch | worst 0 | 15,427,446 |
| multi_claimant::r1_multi_claimant_residual_ratchets | resolved solvent worst gap | <= 116,400 | 144,500 (unchanged from Stage B1) |
| reversal::rule_a_residual_seed_3000121_provider_no_longer_short_with_s1_cap | base engine at op 113 (fork S10) | (5,699,125, 3,632,500) | (5,599,922, 3,632,500) (unchanged from Stage B1) |
| settle_order::r1_nonliened_bankrupt_victim_is_pinned_and_no_worse_than_pre_r1 | pinned neutral-rate result | (-553,486, -518,502) | (-581,887, -518,502) |
| settle_order::r1_equal_size_flip_victim_is_liened_and_order_dependence_pinned | both settle orders equal | equal | (-602,382, 299,999) vs (-537,447, 299,999) |
| settle_order::r1_bankrupt_victim_order_dependence_pinned_and_pair_never_gains | both settle orders equal | equal | (-398,508, 329,999) vs (-377,346, 329,999) |
| settle_order::r1_liened_victim_is_order_independent_and_pinned | pinned result | (73,993, -124,395) | (110,990, -110,990), now the same in both orders |

### 2b. Full engine suite (default features, 104 test targets incl. lib unit tests and doc-tests): 1,412 passed / 16 failed / 13 ignored

Failing: the 9 R1 tests above, plus 7 more:

| test | got / expected | first failing commit | cause |
|---|---|---|---|
| r_regression_fresh_unexpired_backing_still_supports_in_full | cert 100 / 91 | `67b2a0b6` (#250) | #250 rate: claim 100, backing 100, lien 10 -> rate (100-10)/(100-10) = 1, cert 90 + 10 = 100. The test pins the old rate (90 x 0.9 + 10 = 91). Also listed as a #250 conflict in #311. |
| s10_provider_share_after_consumption_and_refill_is_never_moved | consumed 5,000 / 2,376 | `9d1499a6` (1683b11d) | 1683b11d sends the short's whole 5,000 loser payment (liened and unliened) back through consumption; the test pins the pre-1683b11d split (2,624 moved later by S10). Principal and fresh asserts before it still pass. |
| v16_live_mark_reversal_unwinds_counterparty_lien_before_claim_burn | risk epoch 8 / 6 | already failing on base `2cb08797` (7 / 6) | fork-only risk-epoch pin. Every assertion ported from 1683b11d passes; the extra step is the second loss-domain booking. |
| f5_fully_netted_loss_into_lapsed_loss_domain_does_not_revert | see log | already failing on base `2cb08797` | not changed by these ports |
| s16_resolved_close_walk_pins_late_debt_payouts_and_liveness | see log | already failing on base `2cb08797` | not changed by these ports |
| s16ac_excess_late_loss_funds_junior_residual_once_creditors_are_final | see log | already failing on base `2cb08797` | not changed by these ports |
| s16ac_resolved_close_walk_is_live_conserving_and_pinned | see log | already failing on base `2cb08797` | not changed by these ports |

"First failing commit" comes from debug runs of the single test at 67b2a0b6, 821e2bb7, be1f3d90, 82593721 and
9d1499a6, each built in its own fresh target dir. "Base 2cb08797" is the first, freshly built run of e's attribution
script (`/tmp/jnettinge/attr-2cb08797.log`). Correction to e's attribution: e's runs for the four later SHAs reused
2cb08797's binaries (0 "Compiling" lines, same binary hash, same test count although c3988149 and c85b148b add tests),
so those four results were not valid. They are replaced by the bisect above.

Toly's own files, all passing: v16_kf_loss_liened_face 7/7, v16_lien_claim_rate 10/10,
v16_kf_loss_domain_attribution 11/11, v16_kf_loss_same_domain_face 12/12, and his new v16_spec_tests cases
(`v16_resolved_backing_surplus_*` x3, `issue206_*` x2) ok. The `fuzz`-gated ones are in 2e.

### 2c. Ignored control now passes

`v16_resolved_late_loss_refills_provider::control_without_a_provider_every_close_order_is_exact` is `#[ignore]`d on
feat/v22 as "v2.2 known red: loss-domain booking of a late loss (upstream #175 / 1683b11d class)". With this branch it
passes (`--ignored`: 1 passed). The ignore is left in place (owner's call).

### 2d. feat/v22 base (67a4c5b9), same 7 failing files

Fresh target dir, same command (`stageC2-base-v22.txt`): r_regression 6/0, v16_spec_tests 249/0, v22_r1_* 42/0
(1 ignored). So all 16 failures are new relative to feat/v22. The 5 v16_spec_tests that already failed on 2cb08797
(f5, s16, s16ac x2, live-mark-reversal epoch) come from the Stage A/B1 netting line, not from this run's ports.

### 2e. `--features fuzz` (v16_spec_tests, v16_lien_claim_rate, v16_kf_loss_liened_face), `stageC2-fuzz.txt`

On a9e8fb9e: v16_kf_loss_liened_face 7/7, v16_lien_claim_rate 14/14 (includes the fuzz-gated `rate_kernel`
module), v16_spec_tests 267 passed / 8 failed / 2 ignored. Toly's fuzz-gated c3988149 tests
(`v16_resolved_backing_surplus_reserves_domain_source_claims`, `v16_resolved_junior_backing_need_is_minimal_exhaustively`)
pass. The 8 failures are the 6 of 2b plus two insurance-backed variants:

- `v16_live_mark_reversal_unwinds_insurance_lien_before_claim_burn` (Toly's test). Its insurance branch still had the
  pre-7d2e243d expectation ("only realizable unliened support consumes insurance", 2,376): d's port of 7d2e243d
  (8d6c99b0) took only that commit's new tests, not its restatement of this helper. Fixed in this run by porting
  upstream's insurance branch as written (spends no insurance; only the principal loss is booked). Debug fuzz run
  after the fix (`stageC2-fuzz-after-fix.txt`): every upstream assertion passes. It still stops at the fork-only risk-epoch pin
  (`tests/v16_spec_tests.rs` "the fused source-claim burn must contribute its own risk-epoch step"): 5 vs 6
  (the counterparty variant reads 8 vs 6). That pin is the fork's 4c4dfb20 guard, left for the owner.
- `source_reclass_same_leg_reversal_insurance_backed` (fork-only): expects the loss domain to hold the long's whole
  5,000 as backing, gets 0. Passes on feat/v22, fails on 2cb08797 (debug fuzz runs, `stageC2-fuzz-v22.txt`,
  `stageC2-fuzz-2cb08797.txt`; the insurance live-mark-reversal variant shows the same split), so it comes from the netting line. Toly's
  7d2e243d rule for an insurance-backed twin is that netting spends no insurance and only the principal loss is
  booked, so this pin conflicts with his rule.

## 3. Not done here

- At Stage C no fork test was rewritten. (Later commits on this PR did move fork pins: the R1 and s16 pins in
  tests/v16_spec_tests.rs and tests/v22_r1_*.rs, see the PR body. This note is only about Stage C.) One of Toly's tests was completed (2e). The 9 R1 failures and the fork-only pins pin numbers that Toly's rules replace.
  Restating them or keeping R1's guarantees is the owner's decision, as in Stage B1 section 5. The pins:
  r_regression 91 -> 100 (#250); s10 consumed 2,376 -> 5,000 (1683b11d); `source_reclass_same_leg_reversal_insurance_backed`
  (fuzz) 5,000 -> 0 (7d2e243d); the live-mark-reversal risk-epoch pin is one shared `6` but reads 8 (counterparty) and
  5 (insurance), so it needs a value per variant; plus the 4 netting-line failures f5, s16, s16ac x2 (2b).
- The merge gate needs the full suite green (check 3), so this branch cannot pass it as is. No merge.
- Follow-up outside this repo (review S1): percolator-prog's own copy of the credit-rate formula
  (`src/v16_program.rs`, about line 44407) still divides by the full claim bound. The engine now checks the stored rate
  against the #250 formula, so the wrapper must use the engine's formula before it moves to this engine.
- Not run: Kani, build-sbf.
