# T14 block by block: MIR of `claim_resolved_payout_topup_not_atomic` and its abstract program

Left: the MIR text rustc printed (`mir/claim_resolved_payout_topup_not_atomic.mir`, storage statements omitted). Right: the instruction list in `lean/MirFlow/Gen/S_claim_resolved_payout_topup_not_atomic.lean`.
A reader can check the trusted translation of this one function by eye against METHOD-RULES.md section 4. Source: `src/v16.rs:22905-22914` at 08d4fd09.

| Block | MIR (non-storage statements; terminator last) | Abstract program |
|---|---|---|
| bb0 | `_5 = &(*_1);`<br>`_8 = &(*_2);`<br>`_7 = PortfolioV16ViewMut::<'_>::as_view(move _8) -> [return: bb1, unwind continue];` | `.havoc 5`<br>`.havoc 8`<br>`.call 7 1 [0, 1] [] [] false` |
| bb1 | `_6 = &_7;`<br>`_4 = MarketGroupV16ViewMut::<'_, T>::preflight_claim_resolved_payout_topup(move _5, move _6) -> [return: bb2, unwind continue];` | `.havoc 8`<br>`.havoc 6`<br>`.call 4 2 [0, 1] [] [] false` |
| bb2 | `_3 = <Result<(), V16Error> as Try>::branch(move _4) -> [return: bb3, unwind continue];` | `.havoc 5`<br>`.branch 3 4 3` |
| bb3 | `_9 = discriminant(_3);`<br>`switchInt(move _9) -> [0: bb5, 1: bb6, otherwise: bb4];` | `.havoc 4`<br>`.copy 9 3`<br>`.switch 9 [(0, 5), (1, 6)] (some 4)` |
| bb4 | `unreachable;` | `.stop` |
| bb5 | `_12 = MarketGroupV16ViewMut::<'_, T>::claim_resolved_payout_topup_core_not_atomic(copy _1, copy _2) -> [return: bb8, unwind continue];` | `.havoc 7`<br>`.havoc 6`<br>`.havoc 3`<br>`.call 12 8 [] [] [] false` |
| bb6 | `_10 = copy ((_3 as Break).0: core::result::Result<core::convert::Infallible, v16::V16Error>);`<br>`_0 = <Result<u128, V16Error> as FromResidual<Result<Infallible, V16Error>>>::from_residual(move _10) -> [return: bb7, unwind continue];` | `.havoc 10`<br>`.residual 0 7` |
| bb7 | `goto -> bb25;` | `.havoc 10`<br>`.havoc 7`<br>`.havoc 6`<br>`.havoc 3`<br>`.goto 25` |
| bb8 | `_11 = <Result<u128, V16Error> as Try>::branch(move _12) -> [return: bb9, unwind continue];` | `.branch 11 12 9` |
| bb9 | `_13 = discriminant(_11);`<br>`switchInt(move _13) -> [0: bb10, 1: bb11, otherwise: bb4];` | `.havoc 12`<br>`.copy 13 11`<br>`.switch 13 [(0, 10), (1, 11)] (some 4)` |
| bb10 | `_15 = copy ((_11 as Continue).0: u128);`<br>`_18 = &(*_1);`<br>`_17 = MarketGroupV16ViewMut::<'_, T>::validate_shape(move _18) -> [return: bb13, unwind continue];` | `.havoc 15`<br>`.havoc 11`<br>`.havoc 18`<br>`.call 17 13 [0, 1] [] [0] false` |
| bb11 | `_14 = copy ((_11 as Break).0: core::result::Result<core::convert::Infallible, v16::V16Error>);`<br>`_0 = <Result<u128, V16Error> as FromResidual<Result<Infallible, V16Error>>>::from_residual(move _14) -> [return: bb12, unwind continue];` | `.havoc 14`<br>`.residual 0 12` |
| bb12 | `goto -> bb24;` | `.havoc 14`<br>`.havoc 11`<br>`.goto 24` |
| bb13 | `_16 = <Result<(), V16Error> as Try>::branch(move _17) -> [return: bb14, unwind continue];` | `.havoc 18`<br>`.branch 16 17 14` |
| bb14 | `_19 = discriminant(_16);`<br>`switchInt(move _19) -> [0: bb15, 1: bb16, otherwise: bb4];` | `.havoc 17`<br>`.copy 19 16`<br>`.switch 19 [(0, 15), (1, 16)] (some 4)` |
| bb15 | `_23 = &(*_2);`<br>`_26 = &(*_1);`<br>`_25 = MarketGroupV16ViewMut::<'_, T>::as_view(move _26) -> [return: bb18, unwind continue];` | `.havoc 16`<br>`.havoc 23`<br>`.havoc 26`<br>`.call 25 18 [0, 1] [] [] false` |
| bb16 | `_20 = copy ((_16 as Break).0: core::result::Result<core::convert::Infallible, v16::V16Error>);`<br>`_0 = <Result<u128, V16Error> as FromResidual<Result<Infallible, V16Error>>>::from_residual(move _20) -> [return: bb17, unwind continue];` | `.havoc 20`<br>`.residual 0 17` |
| bb17 | `goto -> bb24;` | `.havoc 20`<br>`.havoc 16`<br>`.goto 24` |
| bb18 | `_24 = &_25;`<br>`_22 = PortfolioV16ViewMut::<'_>::validate_with_market::<T>(move _23, move _24) -> [return: bb19, unwind continue];` | `.havoc 26`<br>`.havoc 24`<br>`.call 22 19 [0, 1] [] [1] false` |
| bb19 | `_21 = <Result<(), V16Error> as Try>::branch(move _22) -> [return: bb20, unwind continue];` | `.havoc 23`<br>`.branch 21 22 20` |
| bb20 | `_27 = discriminant(_21);`<br>`switchInt(move _27) -> [0: bb21, 1: bb22, otherwise: bb4];` | `.havoc 22`<br>`.copy 27 21`<br>`.switch 27 [(0, 21), (1, 22)] (some 4)` |
| bb21 | `_0 = Result::<u128, V16Error>::Ok(copy _15);`<br>`goto -> bb25;` | `.havoc 25`<br>`.havoc 24`<br>`.havoc 21`<br>`.setD 0 0`<br>`.goto 25` |
| bb22 | `_28 = copy ((_21 as Break).0: core::result::Result<core::convert::Infallible, v16::V16Error>);`<br>`_0 = <Result<u128, V16Error> as FromResidual<Result<Infallible, V16Error>>>::from_residual(move _28) -> [return: bb23, unwind continue];` | `.havoc 28`<br>`.residual 0 23` |
| bb23 | `goto -> bb24;` | `.havoc 28`<br>`.havoc 25`<br>`.havoc 24`<br>`.havoc 21`<br>`.goto 24` |
| bb24 | `goto -> bb25;` | `.goto 25` |
| bb25 | `return;` | `.ret` |

`.havoc l` lines come from `StorageDead(_l)` and from assignments whose value is not modelled. In `.call d b same pres est uncond`, `same = [0, 1]` means the call cannot write (its arguments are shared references or by-value read-only views); `est = [0]` at bb10 is `validate_shape` on `&(*_1)`; `est = [1]` at bb18 is `validate_with_market` on `&(*_2)` and a view made from `&(*_1)` by bb15; the call at bb5 (`same = []`) is the core, which receives both `&mut` arguments.
