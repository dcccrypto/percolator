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

**What this does and does not show.** Two holes were found in about twelve minutes by a reader
who had not written the code. That is evidence that the translator is the weak point of the
method and that more may remain; it is not evidence that the engine results are wrong (the
triggering patterns do not occur in the engine's MIR). The translator stays in the trusted base
until it is replaced by something checked (METHOD doc, section 5, item c).
