# Upstream netting port: scope check (J-20261119-ENG-PORT-TOLYNETTING, step 0)

Base: `feat/v22-engine-rc23-2026-10-22` @ 3e5970f3. Upstream: aeyakovenko/percolator master (fetch only).
Verdict: **Blocked before porting.** The netting line itself is self-contained, but 1683b11d (the
commit the job exists for) and the S1 test are built on upstream fixes outside the netting line,
which this fork either lacks or implements differently. Per the job's step 0 rule, nothing in `src/` was changed.

## How this was measured

- Symbols: every function/struct/field that each prerequisite commit uses or calls was checked against `src/*.rs`.
  Anything missing is added by an earlier commit in the line itself (`ParNettingBatchV16`,
  `net_slot_face_at_par_not_atomic`, `settle_kf_loss_before_support_not_atomic`,
  `pay_kf_loss_from_free_capital_not_atomic`, `slot_unliened_claim_num`,
  `add_loser_paid_counterparty_backing_unchecked`, `loser_backing_net_{long,short}_num`, ...).
  The exception is `provider_principal_{long,short}_num` (72bfb065). The fork already has this as
  `provider_principal_{long,short}`, from S10 / #309 / #310 / #362.
- Pre-image: `git blame <commit>^` over each hunk of `src/v16.rs` lists which upstream commits wrote the
  lines that each prerequisite commit edits (only lines written after 2026-09-20 are listed).
- Tests: Toly's test files were copied from 1683b11d and run on the base (see `before.txt`). They were first kept under
  `toly-tests/`, not `tests/`, so the branch still builds. They now live in `tests/`, and the `toly-tests/` copies were removed.

## Per commit

| upstream | touches (src/v16.rs fns) | pre-image written by | outside the netting line? |
|---|---|---|---|
| 7d2e243d net same-leg reversal at par | `apply_signed_kf_delta_to_pnl` + new netting fn | only pre-09-20 code (removes the call the fork's R1 replaced) | no (replaces R1 at this call) |
| 23d6a727 provider attribution/expiry | netting fn | 7d2e243d | no |
| 891653a6 never burn partially backed face | adds `settle_kf_loss_before_support_not_atomic`, `pay_kf_loss_from_free_capital_not_atomic`, `loss_bucket_accepts_fresh` | 7d2e243d, 23d6a727 only | no |
| 4e3579c4 bound #457 work | `ParNettingBatchV16`, `net_slot_face_at_par_not_atomic`, `slot_unliened_claim_num` | the line | no |
| fd78ec1c / 5f2a7151 | netting fns | the line | no |
| d5485eab S1 twin backing | netting + new slot fields `loser_backing_net_*` | the line | **test needs c3988149** (below) |
| c8a85ce9 / 72bfb065 / abe4c54c | S1 bookkeeping → `provider_principal_*_num` | the line | overlaps the fork's S10 provider-principal mirror (different name and writer `adjust_slot_provider_principal`) |
| 487b49c0 #175 loss-domain booking | `book_kf_loss_support_for_source_domain_not_atomic`, resolved-loss paths | 13ebf86b, 1a700253, fee5f863, c054c8c3 (#216/#219/#230 bucket gates), bdaebc3e (ported as dc1fa4d9), merge ed553e5a | context only; the fork has its own #230 port (6888fb8b, 0b3321fe) |
| 6666a2ab #252 own-face cancel | netting fns | the line only | no |
| **1683b11d** liened face pays the loss domain | `burn_account_source_lien_face_not_atomic` (+14 fns) | 7d2e243d, 487b49c0, **c85b148b (#206)**, **adf5cb64 (#172)**, 5d9244f7/35c157ef (#450, ported as fff714db/e7a33eb0) | **yes: #250, #206, #172 (below)** |
| 427dc5fc / 4eba3dfb | optional perf follow-ups to 1683b11d | 1683b11d | same as 1683b11d |

## What is needed beyond the netting line, and why

1. **#250 lien-aware credit rate: upstream c7ae2256 + 596b3d98.** Not in the fork. The fork has R1's own
   S5 "protective rate for liened claims" (e7b22061, `v16.rs` ~4233 / ~14302) instead.
   - 1683b11d's commit message: "Found while re-checking #250 after it landed … Until #250 this needed the
     whole claim to be liened … With the lien-aware rate a partial lien no longer cuts the rate, rule (b)
     no longer fires, and every loss larger than the account's unliened face burns liened face."
   - `tests/v16_kf_loss_liened_face.rs` is written for the #250 rate. Five of its seven tests use partial liens:
     `loss_beyond_the_unliened_face_…` (76/78 units, "Before #250 those two paid from capital instead"),
     `…_is_refresh_order_independent` (78), `reversal_against_liened_twin_face_…` (26M of 40M liened),
     `liened_twin_face_never_takes_the_provider_principal_with_it` (26M of 40M), and
     `random_histories_with_liens_…` (counts quoted against #250).
   - The par netting (`net_slot_face_at_par_not_atomic`) and rule (b) branch on the stored credit rate
     (`rate < 1` → partially backed). With the fork's rate formula a different branch runs for the same world.
   - Functions in c7ae2256/596b3d98: `available_backing_num_for_source_credit_state`,
     `expected_source_credit_rate_num_for_state`, new `unliened_claim_bound_num_for_source_credit_state`,
     `deposit_/withdraw_fresh_counterparty_backing_not_atomic` (withdrawal watermark), plus `v16_proofs.rs`.
     Its own test, `tests/v16_lien_claim_rate.rs`, is 1,564 lines. It is a credit-rate change across the engine,
     not netting.
2. **#206 lapsed buckets impair their liens: upstream c85b148b.** Not in the fork. 1683b11d edits the call to
   `release_or_retire_counterparty_lien_backing_not_atomic` (added by c85b148b) inside
   `burn_account_source_lien_face_not_atomic`. The fork's version of that function goes through
   `release_source_credit_lien_from_counterparty_terminal_not_atomic` and its own expiry arm instead. The
   function c85b148b adds, `source_domain_prospective_support_for_face`, is also absent.
3. **#172 backed gain cures an outstanding loss: upstream adf5cb64.** The fork has #172 differently
   (91911d5b, port of 137662a9, plus its own refresh-order fix). adf5cb64's
   `apply_signed_kf_delta_to_pnl` / `burn_account_source_claim_bound_num_domain_first` lines are in
   1683b11d's pre-image.
4. **c3988149 resolved providers withdraw unneeded surplus.** Not in the fork. Toly's S1 test
   `tests/v16_twin_netting_provider_backing.rs` (d5485eab) calls `resolved_counterparty_backing_surplus` and
   `withdraw_resolved_counterparty_backing_surplus_not_atomic` and does not compile on the base.
   (`tests/kf_world/mod.rs` in the fork already stubs the same API out with `surplus = 0`.)
5. The fork's R1 netting (`KfLossNettingV16`, `kf_pending_credit` (43 uses), S5, S9 and the R1 test files
   `tests/v22_r1_*.rs`) is replaced at the same call sites. The goal authorizes this, but it is listed here
   because the R1 tests pin R1 numbers (see PR #373 comment 6953065720).

Not needed (checked): a2cf0fbb (#458 remainder carry) and 5d9244f7/35c157ef (#450 forfeit) are not in
the pre-image of 7d2e243d…6666a2ab. #450 is already ported (fff714db, e7a33eb0) and only appears as
context in 1683b11d.

## BEFORE (base 3e5970f3, no src change), from `before.txt`

- `v16_kf_loss_liened_face` (1683b11d): **0 passed / 7 failed** (matches PARNET).
- `v16_kf_loss_domain_attribution` (487b49c0): 9 passed / 2 failed.
- `v16_kf_loss_same_domain_face` (6666a2ab): 5 passed / 7 failed.
- `v16_twin_netting_provider_backing` (d5485eab): does not compile (needs c3988149 plus two signature renames).

## Options for the next job (not decided here)

A. Widen the port to the netting line plus #250 (c7ae2256, 596b3d98), #206 (c85b148b) and c3988149, in
   upstream order. This replaces both R1 netting and S5.
B. Port only the netting line 7d2e243d…6666a2ab, which is self-contained by the pre-image check. Then 1683b11d
   either waits for #250 or is adapted to S5. That would be a redesign, which this job's port rule forbids.
