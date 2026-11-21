# strong2 (valflow + ClauseFlow): what is proved, what is trusted, and the added rules

This file specifies the trusted part that this session adds. If a rule here is wrong, a generated
theorem can be true in Lean and false of the engine. Read it against `valflow/extract2.py`
(class `T2`) and `lean/ClauseFlow/Core.lean`. Everything the sibling method trusts
(`proofs/novel/METHOD-RULES.md`: rustc's MIR, assumptions A1–A9, the base translation rules) is
trusted here too and is not repeated: `T2` is a subclass of the sibling's `Translator` and calls
it for every statement and terminator not listed below.

## 1. Chain of reasoning

1. rustc prints the MIR of the pinned source (`out/PIN.txt`). **Trusted.**
2. `extract2.py` (class `T2`) turns a function's MIR text into an abstract program (`Cfg` of
   `ClauseFlow.Core`). **Trusted:** every execution of the MIR is an execution of the abstract
   program under the simulation relation of section 2.
3. `extract2.py` (from `# ---- certificate` down) and `run2.py` compute a certificate and choose the
   goal clauses. **Not trusted.**
4. Lean's kernel evaluates `checkCert goal cfg inv = true` (`decide +kernel`) and applies
   `checkCert_sound`. **Proved**, axioms `propext`, `Quot.sound`.
5. A callee summary used in a caller's `call` instruction is the goal of the callee's own module
   in the same directory (same clauses, check numbers mapped as in R8). `call_of_cert` is the Lean
   theorem that a certified callee is a behaviour of that `call` instruction. **Trusted:** that
   the post-clauses the translator writes at the call site are the callee module's goal clauses
   (same script, same run; `run.json` records both), and the frame list `same` (base rule).

## 2. The abstract model and the simulation relation

World, checks, `write`, `same`: as in the sibling. Locals: `env : Nat → Nat`.

For a local `_l` that is not *escaped* (base definition), `env l` is:

- the value, if `_l` is an unsigned integer or a `bool` (`false` = 0, `true` = 1);
- the bit pattern read as unsigned, if `_l` is a signed integer (only non-negative constants are
  ever pinned or compared);
- the discriminant as `discriminant(_l)` reads it, if `_l` is an enum (`Result`: Ok 0, Err 1;
  `ControlFlow`: Continue 0, Break 1; `Option`: None 0, Some 1);
- unconstrained otherwise.

**Payload ghost (R1).** `P(l) = 1000000 + l`. If `_l` currently holds `Result::Ok(x)` or
`ControlFlow::Continue(x)`, then `env P(l)` is the `env`-value of `x` by the list above
(unconstrained if `x` is of another type). In every other situation `env P(l)` is unconstrained.
The abstract program may only *pin* a local or a ghost by an instruction this file or the sibling's
rules list; every other change of `_l` must be a `havoc` of `l` **and of `P(l)`**.

## 3. Added assumptions

| | Assumption | Evidence (not a proof) |
|---|---|---|
| A10 | MIR reads the place `(_y as V).0` only when `_y` is of variant `V`. | rustc's MIR building for safe code (a read on another variant would be undefined behaviour); not checked by a scan. |
| A11 | `core`: `Option::None` has discriminant 0 and `Some` 1; `Option::is_some(&o)` is `discriminant(o) != 0`, `is_none` the opposite, and neither writes; `<Result<T,E> as Try>::branch(Ok(x))` is `Continue(x)` with the same `x`. | `core` source; declaration order of the variants. |
| A12 | The crate defines no item named `Option`, `Some`, `None`, `is_some`, `is_none` (nor any of the sibling's A9 names), under any spelling (raw identifier, macro-generated, split over lines), and no module, alias or extern crate named `core`, `std` or `alloc`. Then a `let` declaration in MIR that prints `core::option::Option<..>`, `core::result::Result<..>`, `core::ops::ControlFlow<..>` means core's type, and the rules below require that printed path. | `valflow/check_assumptions2.py`: scans of the source text across line breaks, of the macro-expanded crate (`-Zunpretty=expanded`) and of the MIR; `out/assumptions_check.txt`. Audit round 3 broke the earlier, weaker form of this check (section 7). |
| A13 | Memory layout facts of safe Rust: the market header object, the market slot array, the view structs and each account object are pairwise disjoint; distinct top-level fields of the header are disjoint. A store through a pointer derived (by reborrow, projection, or as the result of a call given only that pointer and plain values) from a pointer into one of these regions changes only bytes of that region. | Rust's reference rules together with A1, A2 (no `unsafe`, no raw pointers). |
| A14 | `valflow/readset.py` over-approximates what each runtime check can read: the set of top-level header fields, and whether slot contents (as opposed to the slice length) are read. Section 4a. | The analysis is conservative by construction (anything not understood gives ALL); its result for this build is in `out/READSET.md` and can be compared with the source of `validate_shape` by eye. |
| A15 | Appending functions at the end of `src/v16.rs` does not change the discriminants of the enums declared above them, and `discriminant(x)` in MIR is the same number in the probe build and in the analysed build (same compiler, same flags). The name `NAME` at the top level of `v16` is the type MIR prints as `v16::NAME`. | Rust semantics of enum discriminants (a property of the declaration); not checked by a scan. |

## 4. Added translation rules

Notation: "pin" = an instruction that fixes a value; everything not matched falls through to the
sibling's rule for that statement (which is `havoc` unless it is a copy, a `Result` variant or a
small constant).

| | MIR | Abstract instruction | Conditions |
|---|---|---|---|
| R1 | any instruction that the rules translate to `havoc l`, `setD l d`, `cmpEq/cmpNe l ..` | the same, followed by `havoc P(l)` | applied in one post-pass over the whole abstract program, to every such instruction with `l < 1000000` |
| R1 | `copy l m` with `m < 1000000` (a whole-value copy or a discriminant read) | `copy l m; copy P(l) P(m)` | same post-pass |
| R1 | `copy l m` with `m ≥ 1000000` (a payload read, R2) | `copy l m; havoc P(l)` | same post-pass |
| R2 | `_l = copy/move ((_y as Continue).0: T)` or `((_y as Ok).0: T)` | `copy l P(y)` | declared type of `_y` starts with `core::ops::ControlFlow<` (for `Continue`) or `core::result::Result<` (for `Ok`); `_y` not escaped; else `havoc l`. A10. |
| R3 | `_l = Result::<ARGS>::Ok(X)` (also printed as a constant), `ARGS` bracket-matched, declared type of `_l` starts with `core::result::Result<` | `setD l 0`, then: `copy P(l) m` if `X` is `copy/move _m` with `_m` not escaped; `setD P(l) n` if `X` is an integer constant `n < 2^62` or `true`/`false`; `setD P(l) 0/1` if `X` is `Option::<ARGS>::None` / `Option::<ARGS>::Some(..)`; `havoc P(l)` otherwise | `Err(..)`: `setD l 1; havoc P(l)` |
| R4 | `_l = Option::<ARGS>::None` / `Option::<ARGS>::Some(..)` (also as a constant) | `setD l 0` / `setD l 1` (then R1) | declared type of `_l` starts with `core::option::Option<`; the variant's parenthesis must close at the end of the rvalue |
| R5 | `_l = Eq(A, B)` / `Ne(A, B)` where one operand is `copy/move _t`, `_t` of an integer type or `bool`, not escaped, and the other is `const n` (integer `< 2^62`, or `true`/`false`) | `cmpEq l s n` / `cmpNe l s n` with `s` from R6 | `s ≠ l`; else `havoc l` |
| R6 | the operand `_t` of R5 | `s` = `_u` if, in the same block, the last assignment to `_t` before the comparison is `_t = copy/move _u`, `_u` is an integer or `bool` local, neither is escaped, and no statement between that assignment and the comparison mentions `_u` or `_t` on its left-hand side or in a `StorageDead`; otherwise `s = t` | in-block copy propagation; calls end blocks, so no call lies between |
| R7 | `_d = Option::<ARGS>::is_some(move _r) -> bbN` (resp. `is_none`) | statement `cmpNe d o 0` (resp. `cmpEq d o 0`), then `goto N` | `_r` single-assigned, not escaped, not partially assigned, its assignment is `&_o`; `_o` not escaped, declared `core::option::Option<..>`; `_d` a bare `bool` local, not escaped, `d ≠ o`; else the base call rule. A11, A12. |
| R8 | any other call with a callee summary | `call [d, P(d)] N same post` | below |
| — | `<Result as Try>::branch(_m)` into `_d` | `branch d m P(d) P(m) N` | base recognition; A11 |
| — | `<Result as FromResidual<..>>::from_residual(..)` into `_d` | statement `setD d 1` (then R1), `goto N` | base recognition; A6 |
| — | a validator call (base recognition, unchanged) | `call [d, P(d)] N same [⟨[d = 0], est holds⟩]` | |

| R9 | `_l = [v16::]NAME::VARIANT[(..)\|{ .. }]` (also as a constant), declared type of `_l` exactly `v16::NAME` | `setD l i`, `i` the discriminant of `VARIANT` | `(NAME, VARIANT) ↦ i` is in the table read by `valflow/enum_probe.py` (TRUSTED reader `table`, `read_probe`) from rustc's MIR of a probe `fn s2_enum_probe_NAME(x: &NAME) -> u32 { match *x { NAME::V0 { .. } => 7001, .., _ => 7000 } }`, compiled in a scratch copy (pinned sources + probes appended to `src/v16.rs`). A probe counts only if its MIR is exactly `_d = discriminant((*_1)); switchInt(move _d) -> [..]` with every target block one constant assignment; the probe prefix must occur nowhere else in the crate sources. The table for this build is `out/ENUMS.txt`. A15. Also inside R3: `Ok(NAME::VARIANT..)` sets the payload ghost to `i` when the local is declared `core::result::Result<v16::NAME, ..>`. |
| R10 | a store through a pointer, or a call given pointers, confined to a region of the market that a check does not read | the `write` / `call` keeps that check (it is added to `same`) | section 4a |

**R8, callee summaries.** A summary is the goal of the callee's own module: clauses whose guards
mention only local 0 (the result) and `P(0)`. At a call site with destination `_d` each clause is
copied with 0 ↦ `d`, `P(0)` ↦ `P(d)`; its checks are renumbered by the sibling's argument map (the
callee's market argument must be a reborrow chain to our market root, an account check is kept only
if that account argument is a reborrow chain to one of our account roots), unmapped checks are
dropped, a `⊥` conclusion is kept as it is (it speaks about the returned value only). A callee is
summarised only after its own callees; a call met while its callee's analysis is still in progress
(recursion) gets no summary. `Returns`-clauses are read with the world before the call as the
reference world (`rebase` in `Core.lean`).

**What `call` means in Lean.** Locals outside `dsts` are unchanged; checks in `same` keep their
value; each post-clause holds of the locals after the call, "preserved" meaning from the world
before the call.

## 4a. Rule R10: the read-set frame

*Read-sets* (`readset.py`, trusted, A14). For check 0 the analysis starts at
`MarketGroupV16ViewMut::validate_shape`, for checks k ≥ 1 at the two `validate_with_market`. In
every function reached, a local whose declared type is a reference to the header, a market view
(by value or reference) or the slot slice is a *carrier*. A carrier may be: copied or reborrowed
into a carrier of the same kind; taken out of a view (`.0` header, `.1` slots); captured by value
in a closure (whose body is then analysed); passed whole as an argument to a call that resolves to
exactly one MIR body of the crate (then analysed); used as `Len`/`PtrMetadata` of the slice. A
place `(*hdr).N` anywhere adds top-level field `N` to the read-set. Any other use of a header or
view carrier, an unresolved callee, or a local of another type that mentions the header or a view
makes the read-set ALL. Any other use of the slice pointer, or any local whose type mentions
`Market<`, sets "slot contents may be read".

*Regions* (in the function being translated). For a pointer local `_p` that is single-assigned,
not escaped, not partially assigned, with the market root of type `&mut MarketGroupV16ViewMut`:
- `_p = copy/move ((*_root).0: &mut MarketGroupV16HeaderAccount)` → region *header*;
- `_p = copy/move ((*_root).1: &mut [Market<T>])` → region *slots*;
- otherwise, if every non-plain local mentioned in its one assignment (for a call: callee and
  arguments) has the same known region, `_p` has that region; `&mut ((*_h).N: T)…` with `_h` of
  region *header* and exactly one dereference gives region *header field N*;
- anything else: unknown.

*The rule.* A store whose place is a projection of `(*_p)` with exactly one dereference keeps
check k if `_p` has region *slots* and check k does not read slot contents, or `_p` has region
*header field N* (or region *header* and the place's first projection is field `N`) and `N` is not
in check k's read-set. A call keeps check k if no argument is a projected operand of a type that
can write, the callee expression mentions no local, and every bare-local argument that can write
either cannot reach check k's roots at all (base rule) or has a known region that check k does
not read by the same test. Soundness: A3 (the check is a function of the memory it reads), A13,
A14.

For this build (`out/READSET.md`): `validate_shape` may read 28 of the 42 header fields and never
reads slot contents; `validate_with_market` may read 4 header fields and may read slot contents.

## 5. What a generated theorem says

`Returns chk ex cfg w0 b w1 env1`: the abstract program, started in world `w0`, returns from block
`b` with world `w1` and locals `env1`, without executing a block listed in `ex`.

- `T18_header.row`, `T18_account.row`: for every return with `env1 0 = 0` (`Ok`):
  `chk k w0 = true → chk k w1 = true` (k = 0: `validate_shape`; k = 1: `validate_with_market` of
  the account). `ex = []`. No assumed callee: every summary used has its own module.
- `T15_*_checked_paths.row`: the same with `ex` = the listed blocks.
- `S_<callee>.main`: every clause of the summary holds at every return.

- `T15_*_conditional.row`: no excluded block; the callee contracts listed in the file header are
  built into the abstract program as extra post-clauses at those call sites. They are
  **assumptions**, not results.
- `S_close_resolved_account_not_atomic.row`: for every return with `env1 0 = 0` and
  `env1 1000000 = 1` (the `Ok` payload is `ResolvedCloseOutcomeV16::Closed`): both checks hold.

## 6. Known limits

- Values of locals only. A condition on memory (`account.header.pnl.get() < 0`, a bitmap) is not
  a fact of this domain.
- Tests are `= n` and `≠ n`; no order. `if c > 1` teaches nothing.
- Summary clauses are tried for payload values 0 and 1 only.
- A direct `match` on a `Result` (not through `?`) is not followed precisely (two negative-test
  cases are refused although true).
- Panics, compute limits and termination are not addressed: every statement is "if it returns".

## 7. Audit record

- Negative tests (`valflow/negative_tests.py`, table in `out/NEGATIVE-TESTS.md`): attack functions
  compiled into a scratch copy of the pinned source. 31 written with the rules, 21 for R10, the
  sibling's 40, the 104 of audit round 3, and the 78 of audit round 4.
- **Audit round 3** (independent agent, told to break `T2`; it did not see this section): 104
  questions over 103 attack methods on real MIR, 0 unsound. It found **two holes** in the
  assumption scans and **one latent hole** in the code. None is triggered by the engine text.
  - H1: a crate-root `enum r#Result { Err(E), Ok(T) }` (variants reordered, raw identifier). The
    sibling's base rule read `Result::<..>::Ok` as discriminant 0 without looking at the declared
    type, and the per-line name scans did not match `r#Result`. Fixed: `T2` never lets the base rule
    see a `Result` variant whose local is not declared `core::result::Result<..>` (it becomes
    `havoc`), and the scans now cover raw identifiers, line breaks and the macro-expanded crate.
  - H2: a crate-root `mod core { mod option { enum r#Option { Some(T), None } } }` prints the same
    path as core's type. No code fix is possible from the printed text; A12 now states it and the
    scan rejects any module, alias or extern crate named `core`, `std`, `alloc`.
  - R-a: the R2 pattern also matched `copy ((_4 as Ok).0: u128) as u64 (IntToInt)` (a truncating
    cast). rustc did not emit that shape in the auditor's attempts or in the engine MIR (303
    `IntToInt` casts, none with a projected operand); reproduced on hand-edited MIR. Fixed: the
    payload read must be the whole rvalue.
  Regression: the unit tests at the end of `out/NEGATIVE-TESTS.md`.
- **Audit round 4** (started by session 1121b, finished by 1121c; cases in
  `valflow/neg_cases/audit4_*`): 78 questions. 4 unsound answers, all in rule R9, whose table was
  then parsed from the text of `src/v16.rs`:
  - H2: explicit discriminants on tuple variants (`Clean(u8) = (1)`, or after a multi-line
    variant) were not seen, so declaration order was taken for the discriminant.
  - H3: a commented-out or `#[cfg(any())]` decoy declaration was parsed instead of the real one
    (declared with its attribute on the `enum` line, or as `pub(in crate) enum`).
  Fixed by no longer trusting the text: the table is read from compiler output (R9 above, A15).
  The text parse survives only as the untrusted proposer of probe arms. On the engine text the old
  table and the new one agree on every enum the old one had (c38b6b1e: 26 of 26; 08d4fd09: 24 of
  24; the new table also has `TokenValueClassV16`), so no engine result depended on the hole; all
  rows were regenerated with the new table anyway. Rerun with the fix: 0 unsound of 274 cases.
  The 12 [H1] cases and 41 read-set probes of the same round: no unsound answer, no missed read.
  Reader attacks (hand-edited probe text / MIR, `valflow/enum_probe_unit.py`): prefix in another
  file, second marker, prefix before the marker, duplicated probe body, two arms with one
  constant, extra statement in an arm block, argument type of another enum, discriminant read of another place, arm constants out of
  sequence: each empties the table or drops the enum.
  The new reader was written and attacked by the same session; it has had no independent audit.
- Mutation harness for the checker: `out/MUTANTS.md`.
