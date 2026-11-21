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
| A12 | The crate defines no item named `Option`, `Some`, `None`, `is_some`, `is_none`; a `let` declaration in MIR prints the full path of a type (`core::option::Option<..>`, `core::result::Result<..>`, `core::ops::ControlFlow<..>`), and the rules below require that printed path. | `valflow/check_assumptions2.py` (source and MIR scans); `out/assumptions_check.txt`. |

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

## 5. What a generated theorem says

`Returns chk ex cfg w0 b w1 env1`: the abstract program, started in world `w0`, returns from block
`b` with world `w1` and locals `env1`, without executing a block listed in `ex`.

- `T18_header.row`, `T18_account.row`: for every return with `env1 0 = 0` (`Ok`):
  `chk k w0 = true → chk k w1 = true` (k = 0: `validate_shape`; k = 1: `validate_with_market` of
  the account). `ex = []`. No assumed callee: every summary used has its own module.
- `T15_*_checked_paths.row`: the same with `ex` = the listed blocks.
- `S_<callee>.main`: every clause of the summary holds at every return.

## 6. Known limits

- Values of locals only. A condition on memory (`account.header.pnl.get() < 0`, a bitmap) is not
  a fact of this domain.
- Tests are `= n` and `≠ n`; no order. `if c > 1` teaches nothing.
- Summary clauses are tried for payload values 0 and 1 only.
- A direct `match` on a `Result` (not through `?`) is not followed precisely (two negative-test
  cases are refused although true).
- Panics, compute limits and termination are not addressed: every statement is "if it returns".
