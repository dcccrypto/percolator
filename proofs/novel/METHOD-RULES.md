# mirflow: what is proved, what is trusted, and the translation rules

This file is the specification of the trusted part. If a rule here is wrong, a generated theorem
can be true in Lean and false of the engine. Read it against `mirflow/extract.py` (class
`Translator`) and `lean/MirFlow/Core.lean`.

## 1. The chain of reasoning

1. `rustc` (pinned nightly, see `out/PIN.txt`) prints the MIR of the function from the pinned
   source blob. **Trusted:** the compiler, and that this MIR has the behaviours of the function.
2. `extract.py` turns the MIR text into an abstract program (`Cfg` in `Core.lean`).
   **Trusted:** that every execution of the MIR is an execution of the abstract program
   (sections 3 and 4). Tested by `mirflow/mutation_tests.py` and reviewed by an independent
   audit (`AUDIT-EXTRACTOR.md`); neither is a proof.
3. `extract.py` also computes a certificate (one abstract state per block). **Not trusted.**
4. Lean's kernel evaluates `checkCert goal cfg inv = true` and applies `checkCert_sound`.
   **Proved** (axioms: `propext`, `Quot.sound`): every execution of the abstract program that
   returns `Ok` leaves a world in which the goal's checks hold.

So each generated theorem is a kernel-checked statement about the abstract program, and a
statement about the engine function only together with items 1 and 2.

## 2. The abstract model

- **World `W`.** Opaque. Stands for all memory reachable from the function's reference arguments.
- **Checks `chk k : W → Bool`.** Opaque. `chk 0 w`: "`self.validate_shape()` returns `Ok` when run
  on `w`". `chk k w` for `k ≥ 1`: "`account_k.validate_with_market(&self.as_view())` returns `Ok`
  when run on `w`", `account_k` being the function's k-th argument of type
  `&mut PortfolioV16ViewMut<'_>` or `&PortfolioV16View<'_>`. A check depends on the market root
  (`k = 0`) or on the market root and that account (`k ≥ 1`), and on nothing else.
- **Locals `env : Nat → Nat`.** `env l` stands for the discriminant of local `_l` when it is an
  enum, its value when it is an integer or bool; it is unconstrained otherwise. Only what the
  rules below set is ever known about it.
- **Instructions** (`Stmt`, `Term`): see `Core.lean`. Everything is nondeterministic except what
  an instruction pins down.

## 3. Rust-level assumptions

| | Assumption | Evidence (not a proof) |
|---|---|---|
| A1 | No `unsafe` block or `unsafe fn` in the crate. | `check_assumptions.py`: `#![deny(unsafe_code)]` at `src/lib.rs:8`; the only `unsafe` are `unsafe trait`/`unsafe impl` marker items (`src/v16.rs:5786`–`5817`). |
| A2 | No interior mutability, no mutable statics, no raw pointers: data behind a shared reference is not written, and all mutable state is reached through the arguments. | `check_assumptions.py` scans for `Cell`, `RefCell`, `UnsafeCell`, `Mutex`, `Atomic*`, `static mut`, `thread_local!`, `*const`, `*mut`. |
| A3 | `validate_shape`, `validate_with_market` and `as_view` are deterministic functions of the memory they can read: called twice on the same memory they return the same result. | No clock, randomness, I/O or FFI in the crate (`check_assumptions.py`). |
| A4 | The by-value types `PortfolioV16View<'_>` and `MarketGroupV16View<'_, T>` hold only shared references. | Their definitions, `src/v16.rs:5837` and `:5903`, checked by `check_assumptions.py`. |
| A5 | The generic parameter `T` is not a reference type and holds no references. | It is bounded by `MarketWrapperPod: bytemuck::Pod` (`src/v16.rs:5786`), and the only instantiation used is `()` (Toly's `summaries/s2.md`). |
| A6 | Two facts about `core`: `<Result<T,E> as Try>::branch` returns `Continue` (discriminant 0) for `Ok` and `Break` (1) for `Err`; `<Result<T,F> as FromResidual<Result<Infallible,E>>>::from_residual` returns `Err`. Discriminants: `Ok = 0`, `Err = 1`. | `core::result` source; declaration order of the variants. |
| A7 | `bytemuck`'s safe functions do what their signatures say. | Not examined. |
| A8 | Unwinding never ends in a normal return of the same function (cleanup blocks do not reach `return`). | MIR well-formedness. |

From A1 and A2 follows the rule used everywhere: **a call or statement can change the world only
through a value that may carry a mutable reference derived from an argument.**

## 4. Translation rules

Text. Every MIR line is first passed through `mask_literals`: the contents of string and char
literals are replaced by dots, so nothing inside a literal can look like a bracket, a comma or a
local. Bracket structure is then checked; an unbalanced line rejects the function.

Types. *Plain*: the printed type contains none of `&`, `'`, `*`, `{`, `dyn `, `fn(`, `impl `,
` as `, and every identifier in it is a primitive (`u8`…`u128`, `i8`…`i128`, `usize`, `isize`,
`bool`, `char`), or `T` (A5), or a path with `::` (a concrete type), or a name of at least three
characters with a lower-case letter (a concrete type printed without its path). A short
all-capitals name other than `T` is taken to be a type parameter and is not plain.
*Shared reference*: starts with `&` and not `&mut`. *Cannot write*: plain, or a shared reference,
or one of the two by-value view types of A4.

*Plain rvalue* (a value that cannot carry a reference): a constant; `Len(..)`, `discriminant(..)`;
an arithmetic, bit or comparison operator; `copy`/`move` of a place whose printed type (for an
array or slice element: the indexed type) is plain; any other rvalue all of whose locals have
plain types. A borrow (`&..`) is never plain.

Roots. The function's non-plain arguments. The first argument of type
`&[mut] MarketGroupV16ViewMut<_>` is the market root; account roots as in section 2. A root must
never be reassigned or mutably borrowed, or the function is rejected.

Provenance `prov(l)`: the set of roots a pointer held in local `_l` may be derived from, or ALL.
Plain type: none. A root: itself. A local with exactly one assignment in the whole function, never
assigned through a projection, never mutably borrowed: the union over every local mentioned on the
right-hand side (for a call: in the callee expression and the arguments). Anything else: ALL.

`same(S)`: the checks that do not depend on any root in `S` (none if `S` contains ALL).

Escaped locals: a local whose address is taken by `&mut` or `&raw` (after an optional
`no_retag`) anywhere in the function, in a place without a dereference.
Nothing is ever recorded about an escaped local: assignments to it are dropped, reads of it are
replaced by reads of a fresh unconstrained local.

Statements.

| MIR | Abstract instruction |
|---|---|
| `StorageLive`, `FakeRead`, `PlaceMention`, `AscribeUserType`, `Coverage`, `Retag`, `ConstEvalCounter`, `nop`, `assume(..)` | nothing |
| `StorageDead(_l)`, `Deinit(_l)`, `discriminant(_l) = N` | `havoc l` |
| `_l = copy _m`, `_l = move _m`, `_l = discriminant(_m)` | `copy l m` |
| `_l = Result::<..>::Ok(..)` / `Err(..)` (also printed as a constant) | `setD l 0` / `setD l 1` |
| `_l = const N_<int>` , `const true/false` | `setD l N` / `1` / `0` |
| any other assignment to a bare local | `havoc l` |
| assignment to a projection of `_l` without a dereference | `havoc l` |
| assignment to a place with a dereference (`(*_p)…`), base local `_p`, of a plain rvalue | `write same(prov(p))`, omitted if every check is kept |
| assignment to a place with a dereference of an rvalue that is not plain | **the function is rejected** (a stored reference could make one argument's memory reach another's; audit finding 1) |
| anything else | **the function is rejected** |

Terminators.

| MIR | Abstract instruction |
|---|---|
| `goto`, `assert(..) -> [success: bb, ..]` | `goto` |
| `switchInt(copy/move _l)` | `switch l cases otherwise`; a non-local operand or a negative key: switch on a fresh unconstrained local, all edges kept |
| `return` | `ret` |
| `unreachable`, `resume`, `abort`, `terminate`, a call with no return edge | `stop` |
| `drop(P)` | `write same(prov(base))` unless the type is plain; `havoc base`; `goto` |
| `_d = <Result<..> as Try>::branch(_m)` | `branch d m` (A6) |
| `_d = <Result<..> as FromResidual<Result<Infallible, ..>>>::from_residual(..)` | `residual d` (A6) |
| any other call `D = f(args)` | `call d bb same pres est uncond`, below |
| unwind edges | ignored (A8) |
| anything else | **the function is rejected** |

Calls. `S` = union of `prov` of every argument whose type is not "cannot write" (for a projected
operand whose printed type cannot be read: treated as able to write), plus `prov` of every local in
the callee expression (closures, function pointers). `same = same(S)`. If the destination is a
projection, the write or `havoc` it implies happens in an extra block after the call (numbered
after the function's own blocks, in order of creation); a destination behind a dereference whose
type is not plain rejects the function. Then:

- **The shape check.** Callee is exactly `MarketGroupV16ViewMut::<'_, T>::validate_shape` and its
  argument is a reborrow chain of single assignments ending at the market root: `est = [0]`.
- **An account check.** Callee is `PortfolioV16ViewMut::<'_>::validate_with_market::<T>` (or the
  `PortfolioV16View` one for a view-typed account root); its first argument is a reborrow chain
  ending at account root k; its second is `&_v` where `_v` is the destination of
  `MarketGroupV16ViewMut::<'_, T>::as_view(<reborrow of the market root>)`, that call is the
  terminator of the only predecessor of this block, and this block stores through no pointer
  before the call: `est = [k]`.
- **A callee summary** (`out/summaries.json`). Applied only if the callee's market argument is a
  reborrow of our market root; its account checks are carried over only for account arguments
  that are reborrows of our account roots. `pres`/`est` come from the callee's own certificate
  (`call_of_cert` in `Core.lean` is the theorem that licenses this); `uncond` is true when the
  callee does not return a `Result`.
- **An ASSUMED contract** (only in the `*_conditional` modules, listed in each file's header):
  `pres = [k]` for that call. This is a hypothesis, not a result.

The meaning of `call` in Lean: locals other than the destination are unchanged; checks in `same`
keep their value; if `uncond` or the result has discriminant 0, checks in `pres` that held before
hold after and checks in `est` hold after.

## 5. What a generated theorem says

`Returns chk ex cfg w0 b w1 env1`: the abstract program, started in world `w0`, returns from block
`b` with world `w1` and return discriminant `env1 0`, without executing a block listed in `ex`.

- `S_<fn>.main`: for every such return with `env1 0 = 0` (`Ok`): each `est` check holds of `w1`;
  each `pres` check holds of `w1` if it held of `w0`.
- `T15_*_checked_paths.main`: the same with `ex` = the blocks of the unchecked `Ok(..)`
  assignments (listed with their source lines in `out/RESULTS.md`).
- `T15_*_conditional.main`: no exclusions; the callee contracts listed in the file header are
  built into the abstract program as assumptions.

## 6. Known limits

- No value reasoning. A check that is skipped under a condition on a value (T18:
  `if converted != 0 { validate }`) is reported open even if the condition implies no write.
- `Ok(Some(_))` and `Ok(None)` are not told apart, so a callee that validates only before
  `Ok(Some(_))` gets no `est` summary.
- Preservation of a check through code that writes the market but keeps the predicate true
  (the `[real]` rows) is out of reach by construction.
- Panics, compute limits and termination are not addressed: every statement is "if it returns Ok".
