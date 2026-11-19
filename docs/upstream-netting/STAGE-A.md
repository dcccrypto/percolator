# Stage A: upstream netting line (J-20261119-ENG-PORT-TOLYNETTINGb)

Base: agent/J-20261119-ENG-PORT-TOLYNETTING @ 696fd81b. Each ported commit is `git cherry-pick -x` of the upstream commit
(message `port upstream <sha>: <subject>`), in upstream topological order.

## Ported (6)

| upstream | note |
|---|---|
| 7d2e243d net same-leg reversal at par | Conflict at the R1 negative-delta arm of `apply_account_kf_settlement_entry`. Toly's `net_same_leg_reversal_against_twin_face_not_atomic` replaces the R1 call `apply_haircut_bounded_close_loss_to_pnl_in_loss_domain` there. R1 `kf_pending_credit` bookkeeping around it is **left in place** (not removed here; passes `support_paid = 0`). Fork API adaptations: `current_slot` arg of `prepare_counterparty_backing_withdraw_delta`, `loss_domain = None` on `set_account_pnl_after_source_claim_burn`, taker flag in new tests. |
| 23d6a727 provider attribution/expiry | as written (conflict hunks were only misplaced test hunks; ours kept) |
| 891653a6 never burn partially backed face | clean |
| 4e3579c4 bound #457 work | conflict only in the netting fn; upstream function is byte-identical to ours. Adaptations as above |
| fd78ec1c pay kept-face loss once | clean |
| 5f2a7151 whole atoms only | clean (taker flag in new test) |

## Held (and why)

| upstream | reason |
|---|---|
| d5485eab S1 twin backing | 5 conflict regions in `src/v16.rs`, including the `EngineAssetSlotV16Account` layout (adds `loser_backing_net_*`) next to the fork's own S10 `provider_principal_*` mirror; its test `v16_twin_netting_provider_backing` needs c3988149 (`resolved_counterparty_backing_surplus`) and does not compile. Reconciling with S10 is a redesign. |
| c8a85ce9, 72bfb065, abe4c54c | build on d5485eab; 72bfb065 renames the ledger to `provider_principal_*_num`, colliding with the fork's S10 `provider_principal_*` (different writer `adjust_slot_provider_principal`). |
| 6666a2ab #252 own-face cancel | `src/v16.rs` calls `loser_paid_backing_num`, which exists only after 72bfb065. Does not compile without the S1 chain (tried: only `tests/kf_world/mod.rs` and one src hunk conflicted, kept fork's kf_world). |
| 487b49c0 #175 loss-domain booking | 4 conflict regions in `src/v16.rs`, `spec.md`, `tests/kf_world/mod.rs`, `tests/proofs_v16.rs`, and edits `src/v16_kani_api.rs`, which the fork deleted. Follows 6666a2ab in upstream order. |

These need the Stage B decision (S1 vs S10 reconciliation, c3988149).
