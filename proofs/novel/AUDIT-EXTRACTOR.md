# Independent audit of the trusted translation (`mirflow/extract.py`)

The translation from MIR text to the abstract program is the part of the method that Lean does
not check. It was given to a separate agent with instructions to break it: read the Lean
semantics and the translator, grep the real MIR for misparsed shapes, and construct Rust
functions for which the tool certifies something false. The auditor was read-only on this
repository and worked in a scratch directory.

## Round 1 (2026-11-21, about 05:07–05:19 GMT), on commit 13da3407

Result: **two reproduced soundness holes** in the translator, four risks, four notes. The
auditor reported that neither hole is triggered by the engine's MIR (0 pointer-valued stores
through a pointer; 0 mis-split calls out of 12,848 in the default dump and 13,096 in the
audit-scan dump) and that it found no missing behaviour in the Lean semantics.

| # | Severity | Finding | Fix (this commit) |
|---|---|---|---|
| 1 | HOLE | A reborrow of the market stored into memory behind another reference argument (possible with an explicit lifetime link, e.g. `other.header = &mut *self.header`) made later writes through `other` look like writes to `other` only. Reproduced: `audit_a1`, `audit_a1s`, `audit_a2`, `audit_p1` were certified. | `store` now rejects the function (`Unsupported`) when a value that may carry a reference is stored through a pointer (`rvalue_plain`, `place_type`). |
| 2 | HOLE | `split_top` did not know about string and char literals: `f("a < b", self.header)` hid the `&mut` argument. Reproduced with `<`, `>`, `[`, `-> y {` and a char `'<'`. | Every MIR line is passed through `mask_literals` before any parsing; `split_top` rejects unbalanced input. |
| 3 | RISK | `discriminant(_l) = N` (SetDiscriminant) carries a variant index, not a discriminant value. Not produced by rustc here (0 occurrences). | Translated as `havoc`. |
| 4 | RISK | Escape detection did not strip a `no_retag` prefix before looking for `&mut`. Not observed (all 1,247 `no_retag` are `no_retag copy`). | Prefix stripped. |
| 5 | RISK | `is_plain` was a blacklist; bare type parameters other than `T` and associated-type projections passed. No exploit found. | Whitelist: primitives, `T`, path-qualified or CamelCase concrete names; ` as ` rejected. `check_assumptions.py` now also checks that no market-view method has type parameters of its own. |
| 6 | NOTE | `"*" in place` used as "is a dereference". | `"(*"`. |
| 7 | NOTE | Name-based matching of `Result::Ok/Err`. | Regex restricted to `core::result::Result`/`Result`; `check_assumptions.py` checks no user type is named `Result`. |
| 8 | NOTE | With `--exempt auto` the theorem is about executions avoiding the exempt blocks and must be presented so. | It is (report section 4; each Lean file states it). |
| 9 | NOTE | Summary iteration cap not flagged; cyclic summary graph recorded but not enforced. | Both now abort the run. |
| 10 | NOTE | Lean core: no missing behaviours found; `checkCert_sound` is vacuous for an empty goal; the MIR→`cfg` link is unverified. | Stated in METHOD-RULES.md. |

Effect of the fixes on the engine results: none. The summary table before and after is identical
(178 summaries, 0 changed), and all generated modules still compile.

The auditor's 37 attack cases are kept as a regression test: `mirflow/audit_cases/cases.rs`,
runner `mirflow/audit_regression.py`, result `out/AUDIT-REGRESSION.md` (0 unsound answers after
the fixes; the four round-1 hole reproducers are now refused).

## Round 2 (about 05:22–05:47 GMT), on the fixed translator (commit 055ae9ab)

Result: **two more reproduced holes**, one risk, three notes. Again the auditor reported that
neither pattern occurs in the engine (scans of the pinned source and of the MIR of the 41
certified functions), and that the round-1 hole is closed by the other routes it tried.

| # | Severity | Finding | Fix |
|---|---|---|---|
| 1 | HOLE | The regex for `Result::<..>::Ok/Err` was lazy and could stop inside the generic arguments: `r = Ok(Err::<(), V16Error>)` (payload is the fn item `Err`) was read as `Err`, so an `Ok` path was believed unreachable. Reproduced (`r2_c3`), and propagated to a caller through its summary (`r2_c4`). | `result_variant`, `is_result_branch`, `is_result_residual` match the generic argument list by brackets and require the exact suffix. Both cases are in the regression test. |
| 2 | HOLE | rustc prints an item by its shortest unambiguous path. A crate-root item named `Try`, `FromResidual`, `MarketGroupV16ViewMut` or `PortfolioV16View` would be read as the trusted one (three variants reproduced with items added to `lib.rs`). | New assumption **A9**, checked by `check_assumptions.py` on every source file and on the MIR: no crate item named `Try`, `FromResidual`, `Infallible`, `ControlFlow`, `Result`, `Ok`, `Err`; exactly one definition of each view type, in `v16.rs`; core's `Try`/`FromResidual` printed unqualified. The translator also rejects a function whose MIR prints them qualified. `run_all.sh` aborts if a check fails. |
| 3 | RISK | The A5 scan missed `fn f<'b, Wrap: ..>` (lifetime first). No false certificate could be built. | The scan now looks at every parameter. |
| 4 | NOTE | Round-1 hole through a callee or `mem::replace`: refused. | — |
| 5 | NOTE | A function with no reachable `Ok` return gets a vacuous certificate; sound, but it amplifies a value-tracking error. | Not changed. |
| 6 | NOTE | `Deinit((*p))` not seen as a store by `view_of_market_now`; ASSUMED contracts are keyed by callee path only. | The first is fixed; the second is stated in the `*_conditional` file headers. |

Effect on the engine results: none (178 summaries, 0 changed; generated Lean identical).
Regression test now has 40 cases, 0 unsound. The three shadowing reproducers need extra items in
`lib.rs` and are not in the regression test; A9 is what excludes them.

## N-version check (about 05:25–05:30 GMT)

A third agent wrote a second translator from `METHOD-RULES.md` and `Core.lean` without reading
`extract.py` and compared all 41 functions (2,627 blocks): no block where `extract.py` is less
conservative than the rules. See METHOD-RULES.md section 7.

**What this does and does not show.** Four holes were found in two rounds by readers who had not
written the code, all in name- and text-based recognition, none in the Lean core, and none
triggered by the engine. That is evidence that the text-parsing translator is the weak point of
the method and that more may remain; it is not evidence that the engine results are wrong. The
translator stays in the trusted base until it is replaced by something that reads MIR as data
(resolved paths and types from the compiler instead of printed text), which would remove the
class of error behind all four.
