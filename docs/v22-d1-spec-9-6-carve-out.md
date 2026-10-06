# D-1: proposed fork-spec carve-out to §9.6 (bounded band pin)

Status: **DRAFT, NOT yet founder-approved.** The code comment at `src/band_rent.rs`
(`band_d1_pinned_step`) keeps saying so until the founder signs off; that sign-off, recorded in
the ops ledger, is a merge condition for v2.2 (security review of PR #279, "Required before
merge" item 1).

## The upstream text (aeyakovenko/percolator `spec.md`)

§9.6: feed capped staircase prices; same-slot exposed cranks pass the unchanged price; *if exposed
catch-up has `target != P_last`, `dt > 0` and `max_delta == 0`, the wrapper MUST enter recovery or
wait for enough elapsed slots, and MUST NOT advance `slot_last` with the unchanged price as a
silent bypass.*

§9.7: while target and effective price differ, reject or shadow-check extraction-sensitive
actions, any close whose payout depends on lagged PnL, and risk-increasing trades.

## What the band does instead

On a market configured with the per-epoch price band (`band_bps != 0`), one function,
`band_rent::band_d1_pinned_step`, may feed an accrual whose price is `P_last` while
`target != P_last` and `dt > 0`, in two cases:

1. **Edge pin.** The cap-law step is clamped to the band edge `lo(A)` / `hi(A)` of the current
   anchor `A`; once the price sits at that edge the fed price stays `P_last`.
2. **Duration pin.** Past the epoch window `E` the accrual must be no-move: `price == P_last`,
   funding 0, rent 0 (engine `BandPinned` otherwise).

Both advance `slot_last` with an unchanged price while the target differs. That is the
divergence.

## Proposed carve-out text (to add to our fork's spec §9.6)

> **§9.6a Bounded band pin (fork, v2.2).** On a market with a per-epoch price band (half-width
> `d`, epoch window `E`, pin limit `Pmax`, per-side position cap `K`), an exposed catch-up whose
> price is held at `P_last` by the band (at the band edge, or past the epoch window) MAY advance
> `slot_last` with the unchanged price, provided all of the following hold:
>
> 1. The fed price is inside `band(A)` of the current anchor, and the anchor advances only after
>    every positioned leg was certified healthy at `P_last` in the current epoch and no leg is
>    liquidation-pending (the Band Safety Law then bounds every account's loss by its capital).
> 2. While pinned past `E`, funding and rent are not accrued (no value moves on a stale mark).
> 3. The pin is bounded in time: after `Pmax` pinned slots any party may declare
>    `BandPinExpired` and the market enters recovery (§9.6's "enter recovery"), and
>    `Pmax >= 8E`, `E >= 150` slots so a single missed keeper pass cannot trigger it.
> 4. The pin is bounded in work: at most `K <= 256` positioned legs per side, so certifying the
>    whole book is a bounded sweep.
> 5. The asset is treated as **lagged** by exactly one predicate (wrapper
>    `asset_price_lagged_view`: exposed and `target != P_last`), and every §9.7
>    extraction-sensitive path consults it: risk-increasing fills are refused (engine), the
>    favourable side of a close is refused (104) on every trade tag, custody domain withdrawals
>    and ADL wind-down are refused (21), Earn senior entry/exit is repriced at the worse of
>    `P_last` and the target, and user withdrawals require a flat account.
>
> Everything else in §9.6 applies unchanged; a market without a band follows §9.6 exactly.

## Why not the strict reading

Refusing the pinned step bricks `refresh` once `max_accrual_dt_slots` elapses (§8.2 no-accrual
guard), which also bricks certification and liquidation while the true price moves. For a
leveraged product a visible, time-bounded lag with the §9.7 guards above is safer than a market
that can neither certify nor liquidate.

## Residual risks the founder is accepting

- Lag magnitude is unbounded (the whole true move less the band); only its duration is bounded
  (`Pmax`).
- After `BandPinExpired`, terminal settlement is at the stale `P_last` (strict §9.6 would also
  end in recovery at `P_last`; D-1 makes the market look live for up to `E + Pmax` first).
- Funding is not paid to the LP for duration-pinned time (bounded by `2E x (f_max + r_max)`).
- Whoever controls the target (AuthMark creator, a thin DEX mark) can postpone recovery by
  setting `target == P_last` (liveness only).

## Tests that pin the conditions

Engine `tests/v22_band_rent.rs`: `band_duration_pin_forces_no_move_accruals`,
`band_pin_expired_declares_recovery_after_pmax`, `band_config_floors_and_position_cap`,
`band_position_cap_bounds_the_sweep_so_dust_cannot_hold_the_epoch`, `band_never_zero_width`,
the malicious-mark and G-lunge adversaries. Wrapper `tests/v22_band_rent.rs`:
`v22_d1_every_lag_consumer_uses_the_shared_predicate`,
`v22_lag_without_pin_refuses_the_favourable_close_on_every_trade_tag`,
`v22_lag_withdraw_needs_a_flat_account`, `sec_pinned_close_flip_and_open_variants`. The ADL
wind-down (tag 104) lag refusal is `tests/p2b_adl_wind_down.rs::p2b_tag104_refuses_a_lagging_or_stale_mark`;
Earn senior pricing under lag is `vault_lp_equity_lag_bounds_ro` (structural test above).
