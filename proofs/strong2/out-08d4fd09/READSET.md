# Read-sets of the two runtime checks in this build (valflow/readset.py; rule R10)

## check 0, `validate_shape`

Functions analysed: `<closure>::{closure@src/v16.rs:9391:23: 9391:26}`, `MarketGroupV16View::validate_header_aggregate_totals`, `MarketGroupV16View::validate_shape`, `MarketGroupV16ViewMut::as_view`, `MarketGroupV16ViewMut::validate_shape`

Header fields that may be read (25 of 39): 1 `config`, 2 `asset_slot_capacity`, 3 `vault`, 4 `insurance`, 5 `c_tot`, 6 `pnl_pos_tot`, 7 `pnl_pos_bound_tot_num`, 8 `pnl_pos_bound_tot`, 9 `pnl_matured_pos_tot`, 10 `backing_provider_earnings_total`, 11 `source_claim_bound_total_num`, 12 `source_fresh_backing_total_num`, 13 `source_insurance_credit_reserved_total_atoms`, 14 `insurance_domain_budget_remaining_total`, 22 `asset_activation_count`, 23 `last_asset_activation_slot`, 24 `next_market_id`, 27 `slot_last`, 28 `current_slot`, 29 `bankruptcy_hlock_active`, 30 `threshold_stress_active`, 31 `loss_stale_active`, 32 `recovery_reason`, 37 `payout_snapshot_captured`, 38 `resolved_payout_ledger`

Header fields never read: 0 `market_group_id`, 15 `resolved_payout_blocker_count`, 16 `materialized_portfolio_count`, 17 `stale_certificate_count`, 18 `b_stale_account_count`, 19 `negative_pnl_account_count`, 20 `risk_epoch`, 21 `asset_set_epoch`, 25 `oracle_epoch`, 26 `funding_epoch`, 33 `mode`, 34 `resolved_slot`, 35 `payout_snapshot`, 36 `payout_snapshot_pnl_pos_tot`

Contents of the market slots may be read: no (only the length of the slice)

## checks k ≥ 1, `validate_with_market`

Functions analysed: `PortfolioV16View::validate_close_progress_ledger_with_market`, `PortfolioV16View::validate_source_credit_shape_with_market`, `PortfolioV16View::validate_with_market`, `PortfolioV16ViewMut::validate_with_market`

Header fields that may be read (4 of 39): 0 `market_group_id`, 1 `config`, 28 `current_slot`, 33 `mode`

Header fields never read: 2 `asset_slot_capacity`, 3 `vault`, 4 `insurance`, 5 `c_tot`, 6 `pnl_pos_tot`, 7 `pnl_pos_bound_tot_num`, 8 `pnl_pos_bound_tot`, 9 `pnl_matured_pos_tot`, 10 `backing_provider_earnings_total`, 11 `source_claim_bound_total_num`, 12 `source_fresh_backing_total_num`, 13 `source_insurance_credit_reserved_total_atoms`, 14 `insurance_domain_budget_remaining_total`, 15 `resolved_payout_blocker_count`, 16 `materialized_portfolio_count`, 17 `stale_certificate_count`, 18 `b_stale_account_count`, 19 `negative_pnl_account_count`, 20 `risk_epoch`, 21 `asset_set_epoch`, 22 `asset_activation_count`, 23 `last_asset_activation_slot`, 24 `next_market_id`, 25 `oracle_epoch`, 26 `funding_epoch`, 27 `slot_last`, 29 `bankruptcy_hlock_active`, 30 `threshold_stress_active`, 31 `loss_stale_active`, 32 `recovery_reason`, 34 `resolved_slot`, 35 `payout_snapshot`, 36 `payout_snapshot_pnl_pos_tot`, 37 `payout_snapshot_captured`, 38 `resolved_payout_ledger`

Contents of the market slots may be read: yes

