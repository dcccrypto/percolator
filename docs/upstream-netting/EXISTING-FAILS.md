# Existing tests failing on Stage A head (partial default-features run, 69 test binaries reported, run stopped on slow test s10_keeper_settle_patterns_do_not_leak_to_the_ideal)
# Base counts NOT measured. None of these edited. Caused by Toly's netting replacing R1 numbers at the same call sites (7d2e243d) unless noted.
test att_attributable_loss_escapes_insurance_via_trade_close ... FAILED
test att_d2_insurance_never_used_money ... FAILED
test e299_close_order_does_not_change_who_pays ... FAILED
test e299_crank_order_cannot_choose_the_payer_one_loser ... FAILED
test e299_engine_records_the_realizing_domain_and_clears_it_on_resolution ... FAILED
test e299_liquidator_cannot_steer_a0_loss_onto_a1_insurance ... FAILED
test e299_out_of_range_record_fails_closed ... FAILED
test gh292_locked_flat_close_resolves_the_deficit_in_the_live_market ... FAILED
test gh292_only_the_charged_domains_holder_bears_the_booking ... FAILED
test gh292_sim_locked_flat_close_invariants ... FAILED
test gh292_sim_sweep_covers_every_waterfall_outcome ... FAILED
test gh292_small_residual_after_insurance_does_not_escalate ... FAILED
test gh292_stage_insurance_only_cover_books_nothing ... FAILED
test gh292_stage_insurance_then_b_booking_in_order ... FAILED
test gh292_uncoverable_deficit_is_charged_partially_and_written_off_never_recovery ... FAILED
test liq_edge_cross_margin_bankrupt_two_legs_resolves_without_recovery ... FAILED
test multi_asset_single_domain_loss_spends_exactly_the_deficit_from_that_domain_insurance ... FAILED
test result: FAILED. 10 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
test result: FAILED. 12 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.67s
test result: FAILED. 23 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
test result: FAILED. 24 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: FAILED. 4 passed; 10 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.76s
test result: FAILED. 6 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

passed/failed totals:
835 passed, 17 failed
