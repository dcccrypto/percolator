| Case | What it does | Asked | Sound answer | Tool | Verdict |
|---|---|---|---|---|---|
| s2_n1 | R3/R2/R8: callee returns Ok(0) only before writing; caller validates iff c != 0 (the T18 shape) | pres 0 | certified | certified | ok |
| s2_n1#est | same, asking that the check HOLDS (false: Ok(0) path never checks) | est 0 | refused | refused | ok |
| s2_n2 | callee writes on every path | pres 0 | refused | refused | ok |
| s2_n3 | callee's no-write value is 1, caller tests 0 | pres 0 | refused | refused | ok |
| s2_n3ok | callee's no-write value is 1, caller tests 1 (harmless) | pres 0 | certified | certified | ok |
| s2_n4 | inverted test: validates when c == 0 | pres 0 | refused | refused | ok |
| s2_n5 | R1: result local reassigned by a writing callee before `?` | pres 0 | refused | refused | ok |
| s2_n6 | value changed after the payload read | pres 0 | refused | refused | ok |
| s2_n7 | test is on a different variable | pres 0 | refused | refused | ok |
| s2_n8 | write between the call and the test | pres 0 | refused | refused | ok |
| s2_n9 | R4: callee writes before Ok(Some) and validates before Ok(None); caller returns on Some | est 0 | refused | refused | ok |
| s2_n10 | R4: callee validates before Ok(Some), no write before Ok(None) (the T15 :26594 shape) | pres 0 | certified | certified | ok |
| s2_n10#est | same, asking that the check HOLDS (false on the None path) | est 0 | refused | refused | ok |
| s2_n11 | R7: is_some() path returns without a check after a write | pres 0 | refused | refused | ok |
| s2_n12 | R7: validates iff is_some() (harmless) | pres 0 | certified | certified | ok |
| s2_n12n | R7: validates iff is_none() (inverted) | pres 0 | refused | refused | ok |
| s2_n13 | R6: copy taken, source reassigned, test on the new value | pres 0 | refused | refused | ok |
| s2_n14 | match on the Result of a writing callee, Ok(0) arm unchecked | pres 0 | refused | refused | ok |
| s2_n14ok | match on the Result of the no-write-on-0 callee (harmless) | pres 0 | certified | refused | imprecise |
| s2_n16 | R8: tail-call wrapper of a writing callee | pres 0 | refused | refused | ok |
| s2_n16ok | R8: tail-call wrapper of the no-write-on-0 callee (harmless) | pres 0 | certified | certified | ok |
| s2_n17 | R3: callee returns the constant Ok(0) after writing | pres 0 | refused | refused | ok |
| s2_n18 | R3: callee's payload local is modified before the return | pres 0 | refused | refused | ok |
| s2_n19 | R3: callee returns a truncating cast of a nonzero value | pres 0 | refused | refused | ok |
| s2_n20 | R3: callee returns x - 1 | pres 0 | refused | refused | ok |
| s2_n21 | bool payload: Ok(false) also after a write | pres 0 | refused | refused | ok |
| s2_n22 | R1: result assigned in a loop, last by a writing callee | pres 0 | refused | refused | ok |
| s2_n23 | result overwritten through `&mut r` (escaped local) | pres 0 | refused | refused | ok |
| s2_n24 | R4: Option local reassigned to None after a write | pres 0 | refused | refused | ok |
| s2_n25 | two calls; the test is on the first result, the second wrote | pres 0 | refused | refused | ok |
| s2_n26 | test `c > 1` (c == 1 path wrote, unchecked) | pres 0 | refused | refused | ok |
| s2_n27 | match Ok(v) => v, Err(_) => 0 (harmless: Err of this callee is impossible after a write... refusal is only imprecise) | pres 0 | certified | refused | imprecise |
| audit_a1 | [sibling audit] HOLE 1 (fixed): reborrow of the market stored into a second market argument, then written | est 0 | refused | refused (rejected as unsupported): store of a possibly non-plain value through a pointer: ((*_2 | ok |
| audit_a1s | [sibling audit] HOLE 1 (fixed): reborrow stored into an `Option<&mut Header>` argument, then written | est 0 | refused | refused (rejected as unsupported): store of a possibly non-plain value through a pointer: (*_2) | ok |
| audit_a2 | [sibling audit] HOLE 1 (fixed): reborrow of account 1 stored into account 2, then written | est 1 | refused | refused (rejected as unsupported): store of a possibly non-plain value through a pointer: ((*_3 | ok |
| audit_p1 | [sibling audit] HOLE 1 (fixed): same alias, preservation asked, no check at all | pres 0 | refused | refused (rejected as unsupported): store of a possibly non-plain value through a pointer: ((*_2 | ok |
| audit_a3 | [sibling audit] HOLE 2 (fixed): string literal \"a < b\" before a `&mut` argument | est 0 | refused | refused | ok |
| audit_a3gt | [sibling audit] HOLE 2 (fixed): literal with `>` | est 0 | refused | refused | ok |
| audit_a3br | [sibling audit] HOLE 2 (fixed): literal with `[` | est 0 | refused | refused | ok |
| audit_a3ch | [sibling audit] HOLE 2 (fixed): char literal `<` | est 0 | refused | refused | ok |
| audit_s3 | [sibling audit] HOLE 2 (fixed): literal with `-> y {` | est 0 | refused | refused | ok |
| audit_a3ctl | [sibling audit] control: plain literal, header written after the check | est 0 | refused | refused | ok |
| audit_s1 | [sibling audit] literal `(` | est 0 | refused | refused | ok |
| audit_s2 | [sibling audit] literal `)` | est 0 | refused | refused | ok |
| audit_s4 | [sibling audit] literal `<` as only argument, result stored to the header | est 0 | refused | refused | ok |
| audit_b1 | [sibling audit] `Option<&mut>` + `as_mut` | est 0 | refused | refused | ok |
| audit_b1t | [sibling audit] `Option::take` | est 0 | refused | refused | ok |
| audit_b1p | [sibling audit] `if let Some(h) = o` | est 0 | refused | refused | ok |
| audit_b2 | [sibling audit] tuple holding `&mut` | est 0 | refused | refused | ok |
| audit_b4 | [sibling audit] array of `&mut`, `for` loop | est 0 | refused | refused | ok |
| audit_b5 | [sibling audit] `mem::replace` | est 0 | refused | refused | ok |
| audit_b6 | [sibling audit] check result overwritten through `&mut r` | est 0 | refused | refused | ok |
| audit_b6b | [sibling audit] check result overwritten in a tuple field | est 0 | refused | refused | ok |
| audit_b7 | [sibling audit] conditional `r` or `Ok(())` | est 0 | refused | refused | ok |
| audit_b8 | [sibling audit] loop: check, then write on a later iteration | est 0 | refused | refused | ok |
| audit_b9 | [sibling audit] closure taking `&mut Header` | est 0 | refused | refused | ok |
| audit_b12 | [sibling audit] `is_ok()` inverted | est 0 | refused | refused | ok |
| audit_b13 | [sibling audit] `match` inverted | est 0 | refused | refused | ok |
| audit_b14 | [sibling audit] check, write via reborrow chain, return the check's result | est 0 | refused | refused | ok |
| audit_b15 | [sibling audit] write via `&mut [u8; 4]` index | est 0 | refused | refused | ok |
| audit_b16 | [sibling audit] `iter_mut().for_each` | est 0 | refused | refused | ok |
| audit_b17 | [sibling audit] destructuring `&mut` pattern + `fill` | est 0 | refused | refused | ok |
| audit_c2 | [sibling audit] checking helper called on the OTHER market | est 0 | refused | refused | ok |
| audit_c2ok | [sibling audit] checking helper called on self (harmless: certificate is correct) | est 0 | certified | certified | ok |
| audit_c3 | [sibling audit] helper checks its 2nd account; caller passes them swapped; asking for the right one | est 1 | certified | certified | ok |
| audit_c3#2 | [sibling audit] same, asking for the wrong account | est 2 | refused | refused | ok |
| audit_c5 | [sibling audit] helper panics on a failed check (harmless; a refusal is only imprecise) | est 0 | certified | refused | imprecise |
| r2_c3 | [sibling audit] ROUND 2 HOLE 1 (fixed): `Ok(Err::<(), E>)` (fn item `Err` as payload) was read as `Err` | est 0 | refused | refused | ok |
| r2_c3#p | [sibling audit] same, preservation asked | pres 0 | refused | refused | ok |
| r2_c4 | [sibling audit] ROUND 2 HOLE 1 (fixed): caller of the above through its summary | est 0 | refused | refused | ok |
| audit_c5w | [sibling audit] same, then a header write | est 0 | refused | refused | ok |

unsound answers: 0 of 71 cases; imprecise (sound refusals of a true statement): 3
