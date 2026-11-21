# Reduction of `close_resolved_account_not_atomic` preserving check 0 to leaf obligations

Read: a function "reduces to" its listed callees when the tool finds a certificate for it with exactly those callees assumed to preserve the check on their Ok returns. A leaf is a function for which no such certificate exists even with every market-writing method callee assumed.

- `close_resolved_account_not_atomic`: reduces to 2 callee contract(s)
  - `settle_resolved_bankruptcy_negative_pnl`: **leaf** (own writes: writes the market in its own body, or through a non-method callee, with no check after)
  - `detach_solvent_active_legs_for_resolved_close`: reduces to 4 callee contract(s)
    - `begin_full_drain_reset_inner`: **leaf** (own writes: writes the market in its own body, or through a non-method callee, with no check after)
    - `clear_leg_at_slot_inner`: reduces to 2 callee contract(s)
      - `sync_side_adl_index`: **leaf** (own writes: writes the market in its own body, or through a non-method callee, with no check after)
      - `set_asset_state`: **leaf** (own writes: writes the market in its own body, or through a non-method callee, with no check after)
    - `begin_zero_oi_residue_resets`: reduces to 2 callee contract(s)
      - `drop_unowned_side_oi`: reduces to 1 callee contract(s)
        - `set_asset_state`: (see above)
      - `begin_full_drain_reset_inner`: (see above)
    - `apply_position_delta`: reduces to 1 callee contract(s)
      - `apply_position_delta_with_lookup`: reduces to 1 callee contract(s)
        - `apply_position_delta_with_lookup_inner`: reduces to 5 callee contract(s)
          - `attach_leg_at_slot`: reduces to 2 callee contract(s)
            - `sync_side_adl_index`: (see above)
            - `set_asset_state`: (see above)
          - `sync_side_adl_index`: (see above)
          - `set_asset_state`: (see above)
          - `clear_leg_at_slot_inner`: (see above)
          - `resize_leg_same_side_at_slot`: reduces to 2 callee contract(s)
            - `sync_side_adl_index`: (see above)
            - `set_asset_state`: (see above)

## Leaves (4)

- `begin_full_drain_reset_inner` (own writes; MIR blocks: 40)
- `set_asset_state` (own writes; MIR blocks: 29)
- `settle_resolved_bankruptcy_negative_pnl` (own writes; MIR blocks: 81)
- `sync_side_adl_index` (own writes; MIR blocks: 27)
