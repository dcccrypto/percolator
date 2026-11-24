# Stage B1 (J-20261120-ENG-PORT-TOLYNETTINGd, final run)

Start: agent/J-20261120-ENG-PORT-TOLYNETTINGc @ 99b1f870. Rule: Toly's code and numbers win where his code
replaces R1; fork-only behaviour (S10 provider_principal, #299 provenance, insurance routing outside netting) is
not bent.

All runs: `node ~/agent-worker/with-lock.mjs reserved -- bash -c 'CARGO_TARGET_DIR=/tmp/jnetting-target cargo test -j 4 --release --test <files> --no-fail-fast'`.

## 1. The 17 tests of EXISTING-FAILS.md on 99b1f870

Run of the 5 affected files on 99b1f870 (`stageB1-head-99b1f870-5files.txt`):

| file | base 696fd81b | Stage A head (EXISTING-FAILS.md) | 99b1f870 |
|---|---|---|---|
| gh292_att_bprime | 11/0 | 10 passed / 1 failed | 11 passed / 0 failed |
| gh292_att_d | 7/0 | 6 / 1 | 7 / 0 |
| gh292_locked_liquidation_recovery | 14/0 | 4 / 10 | 14 / 0 |
| liquidations_edge | 25/0 | 24 / 1 | 25 / 0 |
| multi_asset_portfolio_cross_edge | 24/0 | 23 / 1 (3 more are in e299_loss_provenance, see correction) | 24 / 0 |

All 17 pass on 99b1f870, so **no test was rewritten** (0 lines of "old value -> new value"): none of them asserts
an R1 number that Toly's code replaces. They all protect the fork's #299 loss provenance and the insurance
routing that follows from it (`realized_by` = the leg's loss domain). Stage A had dropped that provenance when
7d2e243d replaced the R1 call (`realized_by = None`); 99b1f870 put it back. No real conflict remains among the 17.

Correction: 3 of the 17 (`e299_crank_order_cannot_choose_the_payer_one_loser`,
`e299_engine_records_the_realizing_domain_and_clears_it_on_resolution`, `e299_out_of_range_record_fails_closed`)
are in a sixth file, `tests/e299_loss_provenance.rs` (the 12 passed / 3 failed binary of EXISTING-FAILS.md). On the
ported head it passes 15/15 (`stageB1-head-ported-e299-r1.txt`, `stageB1-head-final-15files.txt`). Not measured on
99b1f870 itself.

## 2. Ports in this run

| upstream | result | adaptation |
|---|---|---|
| 6666a2ab #252 own-face cancel | ported (`0d7eac45`) | src hunks apply as written except the new fn's anchor (upstream places it before the S1 loser-paid helper from d5485eab, held), placed after `net_slot_face_at_par_not_atomic`. `tests/kf_world/mod.rs` kept as the fork's version (already a superset, from #362). Test file byte-identical to upstream. |
| 487b49c0 #175 loss-domain booking | ported (`736c0239`) | Upstream's remainder call `apply_signed_kf_delta_to_pnl(-remaining, None)` + `book_kf_loss_support_for_source_domain_not_atomic`. Here: `apply_haircut_bounded_close_loss_to_pnl(remaining, Some(source_domain))` (stored rate, as upstream; `Some` only keeps #299 provenance) and the consumed counterparty support is passed as `support_paid` to the fork's existing `reserve_new_capital_backed_loss_for_source_domain_not_atomic`, which books it through `book_realized_loss_backing_for_domain_not_atomic` (the fork's copy of the same destination rule), skipped after the payout snapshot as upstream. This replaces 99b1f870's call, which priced the remainder at R1's netting rate (`KfLossNettingV16`, reading `kf_pending_credit`, which 121b1cfb no longer maintains). Not ported: the spec accounting-table row (no such table here) and the two Kani harnesses (they call `src/v16_kani_api.rs`, removed in this fork). |
| d5485eab, c8a85ce9, 72bfb065, abe4c54c (S1) | held | c3988149 is an ancestor of d5485eab, and d5485eab's test calls `resolved_counterparty_backing_surplus` / `withdraw_resolved_counterparty_backing_surplus_not_atomic` (2 call sites), which only c3988149 adds (0 definitions in `src/v16.rs`). So they do not apply without c3988149. |
| 1683b11d and prerequisites (#250 c7ae2256+596b3d98, #206 c85b148b, c3988149, #172 adf5cb64) | held | as instructed. |

## 3. Counts after the ports (HEAD)

### 3a. One more fork adaptation: keep the `kf_pending_credit` feeder (`a679c1b9`, revert of 121b1cfb)

review-loop round 1 (S1) found that 121b1cfb removed the only writers that raise `kf_pending_credit`. The
fork-only fix B (#356) still reads it for liquidation equity (`account_liquidation_equity` -> in-flight support).
Upstream has no reader of this counter, so this is fork-only behaviour that Toly's code does not touch. Measured
on 736c0239 (feeder removed), `stageB1-head-ported-hedged.txt`:
- hedged_refresh_order_liquidation: **6 passed / 12 failed**
- hedged_refresh_fee_loss: **4 passed / 5 failed** (e.g. `px 888: fix B (was liquidated at fee 0 too, #346)`)

After a679c1b9: 18/0 and 9/0. The revert did not change any other number below.

### 3b. Final head a679c1b9 (release, `stageB1-head-final-15files.txt`)

| file | base 696fd81b | 99b1f870 | final head |
|---|---|---|---|
| gh292_att_bprime | 11/0 | 11/0 | 11/0 |
| gh292_att_d | 7/0 | 7/0 | 7/0 |
| gh292_locked_liquidation_recovery | 14/0 | 14/0 | 14/0 |
| liquidations_edge | 25/0 | 25/0 | 25/0 |
| multi_asset_portfolio_cross_edge | 24/0 | 24/0 | 24/0 |
| e299_loss_provenance | not measured | not measured | 15/0 |
| hedged_refresh_order_liquidation | not measured | not measured | 18/0 |
| hedged_refresh_fee_loss | not measured | not measured | 9/0 |
| v16_kf_loss_domain_attribution (Toly, 487b49c0) | 9/2 (3e5970f3, `before.txt`) | not measured | **11/0** |
| v16_kf_loss_same_domain_face (Toly, 6666a2ab) | 5/7 (3e5970f3, `before.txt`) | not measured | **12/0** |
| v22_r1_deep_book | not measured here | 1/2 | **1/2** |
| v22_r1_i457 | not measured here | 3/2 | **4/1** |
| v22_r1_multi_claimant | not measured here | 1/3 | **2/2** |
| v22_r1_reversal | not measured here | 21/1 | **21/1** (1 ignored) |
| v22_r1_settle_order | not measured here | 1/7 | **4/4** |

(passed/failed. The R1 numbers are identical on 736c0239 and a679c1b9.)

## 4. Real conflicts: the 10 failing R1 tests (not rewritten)

These are not in the 17. They assert R1's S5 / neutral-rate guarantees, mostly in worlds with **liened** claims.
Upstream handles liened face with #250 (c7ae2256+596b3d98, lien-aware rate) and 1683b11d, both held in this run by
instruction. So the head's numbers are neither R1's nor Toly's: rewriting them would pin a hybrid that upstream does not
produce. One of them (`rule_a_...`) pins the fork-only S10 provider-principal ledger. Numbers (expected = test, got = head):

| test | assertion | expected | got |
|---|---|---|---|
| v22_r1_deep_book::deep_book_maker_worst_order_pinned_at_the_netting_floor | sum of maker's worst order >= pre-R1 worst (175 of 187 worlds liened) | >= -78,538,861 | -80,615,589 |
| v22_r1_deep_book::liened_domain_multi_claimant_ratchets | same, liened multi-claimant | >= pre-R1 worst | below (message only) |
| v22_r1_i457::i457_resolved_close_order_does_not_change_payouts | payouts, close order [0,1,2,3] vs [3,2,1,0], peak 0,1,2, reverse_first | equal | [0, 494527388, 1185878296, 1000000619594320] vs [0, 494527386, 1217566000, 1000000725220000] |
| v22_r1_multi_claimant::r1_multi_claimant_residual_ratchets | resolved solvent worst gap | <= 116,400 | 144,500 |
| v22_r1_multi_claimant::mixed_liened_and_rich_flippers_ratchets | per-account worst orders | no regression | -13,613,431 |
| v22_r1_reversal::rule_a_residual_seed_3000121_provider_no_longer_short_with_s1_cap | base engine (fresh, S10 ledger principal) at op 113 | (5,699,125, 3,632,500) | (5,599,922, 3,632,500) |
| v22_r1_settle_order::r1_bankrupt_victim_order_dependence_pinned_and_pair_never_gains | (victim, winner) winner-first == victim-first | equal, (-414,343, 329,999) | (-403,993, 329,999) vs (-377,346, 329,999) |
| v22_r1_settle_order::r1_equal_size_flip_victim_is_liened_and_order_dependence_pinned | liened victim, both orders | equal | (-603,702, 299,999) vs (-537,447, 299,999) |
| v22_r1_settle_order::r1_liened_victim_is_order_independent_and_pinned | counterparty >= pre-R1 worst order | >= pre-R1 worst | below (message only) |
| v22_r1_settle_order::r1_nonliened_bankrupt_victim_is_pinned_and_no_worse_than_pre_r1 | pinned neutral-rate result 330000 5 200x14 | (-553,486, -518,502) | (-581,887, -518,502) |

Attribution (review-loop round 2, S6): on 99b1f870, before this run's ports, the same five R1 files fail **15** tests
(`stageB1-99b1f870-r1.txt`): the 10 above plus `i457_resolved_close_alone_repairs_nothing`,
`r1_multi_claimant_solvent_losers_are_settle_order_independent`, `r1_resolved_close_is_unchanged`,
`r1_netting_is_independent_of_which_account_is_settled_first`, `r1_effective_equity_is_cadence_and_order_invariant`.
The i457 close-order payouts and the reversal `rule_a` numbers are identical there, so no R1 failure was introduced by
6666a2ab / 487b49c0 / a679c1b9; the ports fixed 5. The 10 come from the Stage A netting line (7d2e243d...) replacing R1. Not run here: full suite, fuzz, Kani, build-sbf (left to the merge gate).

## 5. Outcome

Not all tests in the touched area pass (10 R1 failures above), so per the goal **no PR was opened**. The branch
`agent/J-20261120-ENG-PORT-TOLYNETTINGd` holds the ports and this file. Choices left for Khubair, not taken here:
(a) accept that the R1 guarantees go away with R1 and rewrite or delete these 10 tests to the head's numbers (that would
pin a state upstream does not produce for liened claims); or (b) port #250 + c3988149 + #206 + 1683b11d (option A of
SCOPE.md) and then re-measure the 10 against Toly's own numbers.
