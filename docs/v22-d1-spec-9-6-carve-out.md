# D-1: proposed fork-spec carve-out to §9.6 (bounded band pin)

Status: **DRAFT, NOT yet founder-approved.** Revised 2026-10-06 for the security re-review
(N-4): condition 3 now states the outage that forces recovery and the production defaults;
condition 4 names the cap-fill residual; condition 5 is backed by the exhaustive `lag_policy`
classification instead of an "every path" claim and keeps the band-only scope of the close
rule; condition 6 records the floor exit. The code comment at `src/band_rent.rs`
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
>    `BandPinExpired` and the market enters recovery (§9.6's "enter recovery"). The program
>    floors are `E >= 150` and `Pmax >= 8E`, so a single missed keeper pass cannot trigger it,
>    but **a keeper outage of about `E + Pmax` slots with a pending target gap does**: at the
>    floors that is about 1,350 slots (~9 minutes at 400 ms). The SDK / launch-wizard defaults
>    are therefore `E = 600`, `Pmax = 9,000` (~64 minutes), and the UI states "forced recovery
>    after N minutes of keeper absence".
> 4. The pin is bounded in work: at most `K <= 256` positioned legs per side, so certifying the
>    whole book is a bounded sweep (at most 512 refreshes per epoch). Every slot-holding leg
>    must carry at least the market's minimum leg notional (program floor 10 whole collateral
>    tokens, SDK default 100), so filling the cap locks real margin (see residual risks).
> 5. The asset is treated as **lagged** by exactly one predicate (wrapper
>    `asset_price_lagged_view`: exposed and `target != P_last`). Every instruction is classified
>    by the exhaustive `lag_policy` table (Gated / FlatOnly / MarkFree / MarkDriven), and every
>    Gated handler reaches the predicate (static call-graph test). Gated: risk-increasing fills
>    are refused (engine); on **band markets** the favourable side of a close is refused (104)
>    on every trade tag (off-band markets keep v2.1's behaviour); custody domain withdrawals,
>    insurance withdrawals, ADL wind-down (tag 104), released-PnL conversion (tag 28), vault-LP
>    PnL conversion (tag 100), senior allocation (tag 103), recall / release-surplus / junior
>    tranches are refused (21); Earn senior entry/exit is repriced at the worse of `P_last` and
>    the target. FlatOnly: user withdrawals and portfolio close require a flat account; resolved
>    paths have no live mark. MarkDriven (accrual, liquidation, resolution at `P_last`) is the
>    Band Safety Law's own territory.
> 6. At the floor (the band around `P_last` narrower than 32 ticks, or the cap law's step
>    rounding to 0), the price cannot catch up at all; there the favourable-close refusal is
>    lifted and positions may EXIT (reduce-only: new exposure is refused) at `P_last`, the same
>    price `BandPinExpired` would settle them at. A band market launches at >= 100x the
>    width-floor anchor, so reaching the floor needs a >99% collapse.
>
> Everything else in §9.6 applies unchanged; a market without a band follows §9.6 exactly.

## Why not the strict reading

Refusing the pinned step bricks `refresh` once `max_accrual_dt_slots` elapses (§8.2 no-accrual
guard), which also bricks certification and liquidation while the true price moves. For a
leveraged product a visible, time-bounded lag with the §9.7 guards above is safer than a market
that can neither certify nor liquidate.

## Residual risks the founder is accepting

- **Cap fill.** 256 legs per side can still be filled by one actor. The minimum leg notional
  makes it cost at least `512 x min_leg_notional x IMR` of locked (refundable) margin (at the
  10-token program floor and 10x leverage, 512 tokens; at the SDK default of 100 tokens, 5,120)
  plus price exposure on each side; there is no permissionless eviction of slot holders.

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
