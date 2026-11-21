| Case | What it does | Asked | Sound answer | Tool | Verdict |
|---|---|---|---|---|---|
| audit_a1 | HOLE 1 (fixed): reborrow of the market stored into a second market argument, then written | est 0 | refused | refused (rejected as unsupported): store of a possibly non-plain value through a pointer: ((*_2).0: &mut  | ok |
| audit_a1s | HOLE 1 (fixed): reborrow stored into an `Option<&mut Header>` argument, then written | est 0 | refused | refused (rejected as unsupported): store of a possibly non-plain value through a pointer: (*_2) = move _8 | ok |
| audit_a2 | HOLE 1 (fixed): reborrow of account 1 stored into account 2, then written | est 1 | refused | refused (rejected as unsupported): store of a possibly non-plain value through a pointer: ((*_3).0: &mut  | ok |
| audit_p1 | HOLE 1 (fixed): same alias, preservation asked, no check at all | pres 0 | refused | refused (rejected as unsupported): store of a possibly non-plain value through a pointer: ((*_2).0: &mut  | ok |
| audit_a3 | HOLE 2 (fixed): string literal \"a < b\" before a `&mut` argument | est 0 | refused | refused | ok |
| audit_a3gt | HOLE 2 (fixed): literal with `>` | est 0 | refused | refused | ok |
| audit_a3br | HOLE 2 (fixed): literal with `[` | est 0 | refused | refused | ok |
| audit_a3ch | HOLE 2 (fixed): char literal `<` | est 0 | refused | refused | ok |
| audit_s3 | HOLE 2 (fixed): literal with `-> y {` | est 0 | refused | refused | ok |
| audit_a3ctl | control: plain literal, header written after the check | est 0 | refused | refused | ok |
| audit_s1 | literal `(` | est 0 | refused | refused | ok |
| audit_s2 | literal `)` | est 0 | refused | refused | ok |
| audit_s4 | literal `<` as only argument, result stored to the header | est 0 | refused | refused | ok |
| audit_b1 | `Option<&mut>` + `as_mut` | est 0 | refused | refused | ok |
| audit_b1t | `Option::take` | est 0 | refused | refused | ok |
| audit_b1p | `if let Some(h) = o` | est 0 | refused | refused | ok |
| audit_b2 | tuple holding `&mut` | est 0 | refused | refused | ok |
| audit_b4 | array of `&mut`, `for` loop | est 0 | refused | refused | ok |
| audit_b5 | `mem::replace` | est 0 | refused | refused | ok |
| audit_b6 | check result overwritten through `&mut r` | est 0 | refused | refused | ok |
| audit_b6b | check result overwritten in a tuple field | est 0 | refused | refused | ok |
| audit_b7 | conditional `r` or `Ok(())` | est 0 | refused | refused | ok |
| audit_b8 | loop: check, then write on a later iteration | est 0 | refused | refused | ok |
| audit_b9 | closure taking `&mut Header` | est 0 | refused | refused | ok |
| audit_b11 | write to `self.protection_observed` (not read by the check; refusing is conservative) | est 0 | refused | refused | ok |
| audit_b12 | `is_ok()` inverted | est 0 | refused | refused | ok |
| audit_b13 | `match` inverted | est 0 | refused | refused | ok |
| audit_b14 | check, write via reborrow chain, return the check's result | est 0 | refused | refused | ok |
| audit_b15 | write via `&mut [u8; 4]` index | est 0 | refused | refused | ok |
| audit_b16 | `iter_mut().for_each` | est 0 | refused | refused | ok |
| audit_b17 | destructuring `&mut` pattern + `fill` | est 0 | refused | refused | ok |
| audit_c2 | checking helper called on the OTHER market | est 0 | refused | refused | ok |
| audit_c2ok | checking helper called on self (harmless: certificate is correct) | est 0 | certified | certified | ok |
| audit_c3 | helper checks its 2nd account; caller passes them swapped; asking for the right one | est 1 | certified | certified | ok |
| audit_c3 | same, asking for the wrong account | est 2 | refused | refused | ok |
| audit_c5 | helper panics on a failed check (harmless; a refusal is only imprecise) | est 0 | certified | refused | imprecise |
| r2_c3 | ROUND 2 HOLE 1 (fixed): `Ok(Err::<(), E>)` (fn item `Err` as payload) was read as `Err` | est 0 | refused | refused | ok |
| r2_c3 | same, preservation asked | pres 0 | refused | refused | ok |
| r2_c4 | ROUND 2 HOLE 1 (fixed): caller of the above through its summary | est 0 | refused | refused | ok |
| audit_c5w | same, then a header write | est 0 | refused | refused | ok |

unsound answers: 0 of 40 cases
