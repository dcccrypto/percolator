# Upstream 18 Nov resolved-close fixes: port check (J-20261119-ENG-TOLY-RESOLVED1118)

Start: `agent/J-20261119-ENG-PORT-TOLYNETTING` @ 696fd81b (= base `feat/v22-engine-rc23-2026-10-22` @ 3e5970f3
plus a docs-only commit; that netting job ended Blocked with no `src/` change). Upstream: aeyakovenko/percolator
master @ add1d027 (fetch only). Not touched here: 9a219cca, 2ed67b16 (job J-20261119-TOLY-DAILY-ENG, PR #378).

Upstream's test file for each commit was copied as is. Compile-only changes: the fork's
`execute_trade_with_fee_loss_stale_scoped_not_atomic` takes a 4th `taker_is_long_account` argument (`true`);
the 52659c02 test defines `RESOLVED_STRANDED_PLACEMENTS_PER_CALL = 4` locally, because that constant is
part of the fix. Output: `before-*.txt` (base), `after-*.txt` (this branch).

| upstream | base (before) | result | why |
|---|---|---|---|
| 8e23d98b a resolved claim waits while part of its share is pledged to a lien | 1 pass / 3 FAIL (`claim_is_paid_its_share_…`, `a_claim_waits_exactly_…`, `a_waiting_claim_is_released_…`; X is paid 128 of its 3,077 share when closed first) | **Ported** | Self-contained. Adapted to the fork: no #251 `ResolvedSourceRealizationV16::Waiting`, so the wait returns `LockActive` (same as the fork's zero-conversion wait), and the conversion uses the fork's rate (`available / positive_claim_bound_num`, no #250 lien-aware rate). The defect is the same with either rate. After: 4/4 pass. Review fix (S1): because the wait is an error, it rolled back a lien release made earlier in the same close, so two lien holders on one domain blocked each other until the bucket lapsed. The close now reports `ProgressOnly` when it released a lien before the wait (the release half of #251). Fork-only test `a_lien_release_commits_when_the_holder_then_waits_for_another_lien`: fails before (`before-s1-two-lien-holders.txt`), passes after. Covers `converted != 0` only: a claim that converts to zero because all backing is pledged is still demoted once its domain is terminal (that is #251's `source_claim_converts_to_zero_unpledged`, not ported). spec.md not changed: the fork's spec has no "one remaining wait" section to extend |
| 52659c02 net a K/F loss first settled in Resolved mode; place stranded loser payments | 0 pass / 7 FAIL | **Skipped** | It extends upstream's pre-support netting, which the fork does not have: `ParNettingBatchV16`, `net_slot_face_at_par_not_atomic`, `KfLossDomainCreditV16`, `book_kf_loss_support_for_source_domain_not_atomic`, `loss_bucket_accepts_fresh`, `provider_principal_{long,short}_num` (the #175/#457 netting line, 7d2e243d..1683b11d; see `docs/upstream-netting/SCOPE.md`, blocked on #250/#206/c3988149). It also replaces `settle_resolved_account_kf_chunk_not_atomic` (a55aceb3), which is only on unmerged PR #375 |
| add1d027 a resolved position keeps its loss weight until losses opposite are booked | 0 pass / 4 FAIL | **Skipped** | Built on two commits the base lacks. (1) 2ed67b16: its detach change extends 2ed67b16's `waiting_obligation` array and `resolved_residual_can_still_be_booked`. That port is on PR #378 (J-20261119-TOLY-DAILY-ENG), not merged, and this job must not redo it. (2) 52659c02 (skipped above): the B-charge progress report calls `settle_resolved_account_side_effects_after_kf_not_atomic` (added by 52659c02), and exists so that 52659c02's stranded-payment placement runs before a leg leaves. It also re-stages tests in `v16_resolved_kf_loss_netting.rs` (52659c02's file). Also uses `leg_f_snap_is_settled` (a2cf0fbb) and `leg_kf_epoch_is_current` (92ed4a1a); the fork has `leg_kf_and_rent_are_current` in their place |

The upstream tests for the two skipped commits are kept in `toly-tests/`, not `tests/`, so the branch stays green.
To use one: copy it into `tests/` and run `cargo test --features fuzz --test <name>`.

Order to land the skipped two: the netting line (needs #250/#206/c3988149 first), then a55aceb3 (#375), then
52659c02; and 2ed67b16 (#378), then add1d027.
