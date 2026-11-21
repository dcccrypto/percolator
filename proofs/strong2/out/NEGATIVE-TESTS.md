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
| s3_f1 | R10: writes header field materialized_portfolio_count, which validate_shape never reads (the T29/T47 shape) | pres 0 | certified | certified | ok |
| s3_f2 | R10: writes c_tot (read) | pres 0 | refused | refused | ok |
| s3_f3 | R10: helper given `&mut header.vault` (read) | pres 0 | refused | refused | ok |
| s3_f4 | R10: helper given `&mut header.materialized_portfolio_count` (not read) | pres 0 | certified | certified | ok |
| s3_f5 | R10: helper given the whole header, writes c_tot | pres 0 | refused | refused | ok |
| s3_f6 | R10: pointer is one of two fields, one of them read | pres 0 | refused | refused | ok |
| s3_f7 | R10: overwrites a market slot; validate_shape (this build) reads only the slice length | pres 0 | certified | certified | ok |
| s3_f7a | R10: overwrites a market slot; the ACCOUNT check may read slots | pres 1 | refused | refused | ok |
| s3_f9 | R10: writes header.mode, not read by validate_shape | pres 0 | certified | certified | ok |
| s3_f9#acct | R10: writes header.mode, which the account check reads | pres 1 | refused | refused | ok |
| s3_f10 | R10: overwrites header.config (read) | pres 0 | refused | refused | ok |
| s3_f11 | R10: unread field through a sub-borrow, then c_tot through the parent | pres 0 | refused | refused | ok |
| s3_f12 | R10: bytes_of_mut(header)[40] = 0xff | pres 0 | refused | refused | ok |
| s3_f13 | R10: mem::swap of an unread field with a read one | pres 0 | refused | refused | ok |
| s3_f14 | R10: mem::swap of two unread fields | pres 0 | certified | certified | ok |
| s3_f15 | R10: callee writes c_tot | pres 0 | refused | refused | ok |
| s3_f15ok | R10: callee writes only an unread field | pres 0 | certified | certified | ok |
| s3_f16 | R10: pointer chosen between an unread and a read field of the same type | pres 0 | refused | refused | ok |
| s3_f17 | R10: array of field pointers, one read | pres 0 | refused | refused | ok |
| s3_f18 | R10: nested field of config (top-level field 1, read) | pres 0 | refused | refused | ok |
| s3_f19 | R10: slice reborrowed, then vault written | pres 0 | refused | refused | ok |
| a3_c01 | [audit 3] R2: match Ok(v) => v as u64 (truncation: v=2^64 wrote, c=0, unchecked) | pres 0 | refused | refused | ok |
| a3_c02 | [audit 3] R2: `? as u64` truncation | pres 0 | refused | refused | ok |
| a3_c03 | [audit 3] R2: if-let Ok(v), `v as u8 != 0` | pres 0 | refused | refused | ok |
| a3_c04 | [audit 3] R2: Result<u128,u128>, Err(0) after a write read as payload | pres 0 | refused | refused | ok |
| a3_c04b | [audit 3] R2: or-pattern Ok(c)|Err(c) | pres 0 | refused | refused | ok |
| a3_c05 | [audit 3] R1: mem::swap of two local Results via helper | pres 0 | refused | refused | ok |
| a3_c05b | [audit 3] R1: mem::replace on a local Result | pres 0 | refused | refused | ok |
| a3_c06 | [audit 3] R1: Result in a tuple field, field reassigned by writing callee | pres 0 | refused | refused | ok |
| a3_c06b | [audit 3] R8: callee returns tuple field t.0 (Ok(0)) after write | pres 0 | refused | refused | ok |
| a3_c06c | [audit 3] R8: callee returns array element Ok(0) after write | pres 0 | refused | refused | ok |
| a3_c07 | [audit 3] R1: shadowed result out of loop-break | pres 0 | refused | refused | ok |
| a3_c08 | [audit 3] R1: Result inside Option, reassigned | pres 0 | refused | refused | ok |
| a3_c09 | [audit 3] R2: payload read through a reference chosen between two results | pres 0 | refused | refused | ok |
| a3_c09b | [audit 3] R1: as_ref().map().unwrap_or(0) of writing callee | pres 0 | refused | refused | ok |
| a3_c09c | [audit 3] R1: map(|v| v&0).and_then of writing callee | pres 0 | refused | refused | ok |
| a3_c10 | [audit 3] R6: copy, `&= 0`, copy, test | pres 0 | refused | refused | ok |
| a3_c11 | [audit 3] R5: signed payload -1 after write, test c > 0 | pres 0 | refused | refused | ok |
| a3_c11b | [audit 3] R5: signed casts + `c != -1` | pres 0 | refused | refused | ok |
| a3_c12 | [audit 3] R5: payload 2^100 after write, test (c as u64) != 0 | pres 0 | refused | refused | ok |
| a3_c12b | [audit 3] R5: constants >= 2^62, no validation at all after a write | pres 0 | refused | refused | ok |
| a3_c13 | [audit 3] R5: bool from (c & 1) != 0 | pres 0 | refused | refused | ok |
| a3_c13b | [audit 3] R5: bool from !(c==0) & (x==5) | pres 0 | refused | refused | ok |
| a3_c14 | [audit 3] R7: is_none on a tuple field, inverted (validates only when nothing was written) | pres 0 | refused | refused | ok |
| a3_c14b | [audit 3] R7/R4: option overwritten with None on a flag, then is_some | pres 0 | refused | refused | ok |
| a3_c14c | [audit 3] R7: is_some on o.and(None) | pres 0 | refused | refused | ok |
| a3_c14d | [audit 3] R7: is_some on a reference chosen between two options | pres 0 | refused | refused | ok |
| a3_c14e | [audit 3] R7: is_some in a loop, option reassigned (second iteration returns unchecked after first-iteration write) | pres 0 | refused | refused | ok |
| a3_c14f | [audit 3] R8/R4: callee returns Ok(None) after write (o.and(None)) | pres 0 | refused | refused | ok |
| a3_c14g | [audit 3] R4: Option<Option>: validates unless Some(None) (callee None = no write) | pres 0 | certified | certified | ok |
| a3_c14h | [audit 3] R7: o.take() then is_some | pres 0 | refused | refused | ok |
| a3_c15 | [audit 3] R8: callee writes in a loop and returns Ok(0) | pres 0 | refused | refused | ok |
| a3_c17 | [audit 3] R8: recursive callee returns Ok(0) after writes | pres 0 | refused | refused | ok |
| a3_c17b | [audit 3] R8: mutually recursive callee (entry a) | pres 0 | refused | refused | ok |
| a3_c17c | [audit 3] R8: mutually recursive callee (entry b) | pres 0 | refused | refused | ok |
| a3_c18 | [audit 3] R8: callee called on the other market with self as its second arg; it writes self | pres 0 | refused | refused | ok |
| a3_c18b | [audit 3] R8: summary of a call on the OTHER market used to guard a check of ours after our write | pres 0 | refused | refused | ok |
| a3_c18c | [audit 3] base: validate_shape of the other market | pres 0 | refused | refused | ok |
| a3_c19 | [audit 3] R8: swapped account args; callee validates its first account (= our second) | est 1 | refused | refused | ok |
| a3_c19#est2 | [audit 3] R8: swapped account args; our second account is validated | est 2 | certified | certified | ok |
| a3_c21 | [audit 3] R8: call through a fn pointer | pres 0 | refused | refused | ok |
| a3_c22 | [audit 3] R2: tuple payload (0, x) | pres 0 | refused | refused | ok |
| a3_c23 | [audit 3] R2: nested Result, `??`, inner Ok(1) after write, validates iff c != 1 | pres 0 | refused | refused | ok |
| a3_c23b | [audit 3] R2: nested Result, if-let on inner | pres 0 | refused | refused | ok |
| a3_c25 | [audit 3] R2: ControlFlow returned directly, Break after write | pres 0 | refused | refused | ok |
| a3_c25b | [audit 3] R2: ControlFlow continue_value().unwrap_or(0) | pres 0 | refused | refused | ok |
| a3_c29 | [audit 3] R5: match c as usize (truncation) after writing callee | pres 0 | refused | refused | ok |
| a3_c30 | [audit 3] R5: match with guard and range; 1..=7 after a write unchecked | pres 0 | refused | refused | ok |
| a3_c31 | [audit 3] R5: bool payload, returns early iff false (no write) | pres 0 | certified | certified | ok |
| a3_c31b | [audit 3] R5: bool payload, inverted | pres 0 | refused | refused | ok |
| a3_c31c | [audit 3] R5: `b != true` validates only when nothing written | pres 0 | refused | refused | ok |
| a3_c33 | [audit 3] R6: chain of copies and reassignments, test on y | pres 0 | refused | refused | ok |
| a3_c33b | [audit 3] R6: mem::swap of two integer locals | pres 0 | refused | refused | ok |
| a3_c34 | [audit 3] R2+R8: callee truncates payload with `as u64`, caller tests != 0 | pres 0 | refused | refused | ok |
| a3_c35 | [audit 3] R1: closure assigns the captured Result | pres 0 | refused | refused | ok |
| a3_c36 | [audit 3] R2: if-let Ok(0)/Ok(1) on two different results | pres 0 | refused | refused | ok |
| a3_c37 | [audit 3] R5: flag derived from c, write after, validates only if c != 0 | pres 0 | refused | refused | ok |
| a3_c38 | [audit 3] R5: signed cast, writes in the else branch | pres 0 | refused | refused | ok |
| a3_d01 | [audit 3] R2: `Ok(ref v) => *v as u64` (ref-propagation may fold the cast onto the payload place) | pres 0 | refused | refused | ok |
| a3_d02 | [audit 3] R2: `match &r { Ok(v) => *v as u64 }` | pres 0 | refused | refused | ok |
| a3_d03 | [audit 3] R2: `if let Ok(ref v) = r { *v as u8 }` | pres 0 | refused | refused | ok |
| a3_d04 | [audit 3] R2: `match *rr { Ok(v) => v as u64 }` | pres 0 | refused | refused | ok |
| a3_d05 | [audit 3] R2: cast inside a tuple expr | pres 0 | refused | refused | ok |
| a3_d08 | [audit 3] R1: `*(&mut r) = Ok(0)` after writing callee | pres 0 | refused | refused | ok |
| a3_d09 | [audit 3] R1: `if let Ok(ref mut v) = r { *v = 0 }` | pres 0 | refused | refused | ok |
| a3_d09b | [audit 3] R1: `Ok(ref mut v) => *v &= 0` | pres 0 | refused | refused | ok |
| a3_d10 | [audit 3] R4/R7: option payload and option overwritten through &mut, then is_some | pres 0 | refused | refused | ok |
| a3_d11 | [audit 3] R8: generic free fn writes and returns Ok(0) | pres 0 | refused | refused | ok |
| a3_d12 | [audit 3] R8: trait method with the name of a no-write inherent method writes and returns Ok(0) | pres 0 | refused | refused | ok |
| a3_d13 | [audit 3] R8: wrapper struct holding &mut market writes, returns Ok(0) | pres 0 | refused | refused | ok |
| a3_d14 | [audit 3] R3: Ok(c) rebuilt, then replaced by writing callee result on a flag | pres 0 | refused | refused | ok |
| a3_d15 | [audit 3] R1: early `return r` of a writing callee | pres 0 | refused | refused | ok |
| a3_d16 | [audit 3] R5: constants at and above 2^62 | pres 0 | refused | refused | ok |
| a3_d17 | [audit 3] R5: `c as i8 == -1` returns unchecked | pres 0 | refused | refused | ok |
| a3_d18 | [audit 3] R5: signed match with -1 arm after write unchecked | pres 0 | refused | refused | ok |
| a3_d19 | [audit 3] R5: char comparison | pres 0 | refused | refused | ok |
| a3_d20 | [audit 3] R5/R6: value shifted in a loop, test == 0 returns unchecked | pres 0 | refused | refused | ok |
| a3_d21 | [audit 3] R8: second call (writing) takes first's payload | pres 0 | refused | refused | ok |
| a3_d22 | [audit 3] R2: tuple of two Results | pres 0 | refused | refused | ok |
| a3_d23 | [audit 3] R1: two Results moved through a tuple | pres 0 | refused | refused | ok |
| a3_d24 | [audit 3] R7: two options swapped through mem::swap | pres 0 | refused | refused | ok |
| a3_d25 | [audit 3] R7: bool algebra over is_none | pres 0 | refused | refused | ok |
| a3_d26 | [audit 3] R7: is_none captured before a write, validates iff some... (wrote unconditionally after: only !b path checks; b path = unchecked write) | pres 0 | refused | refused | ok |
| a3_d27 | [audit 3] R7: is_none as u8 arithmetic | pres 0 | refused | refused | ok |
| a3_e01 | [audit 3] R8/R5: callee tests y==1, writes, returns y>>1 (2,3 -> Ok(1) after write) | pres 0 | refused | refused | ok |
| a3_e02 | [audit 3] R8: callee's payload local set to 1 through &mut after write | pres 0 | refused | refused | ok |
| a3_e03 | [audit 3] R8/R5: callee returns Ok(x) with x==1 only before writing (true) | pres 0 | certified | certified | ok |
| a3_e04 | [audit 3] R8: countdown loop returns Ok(0) after writes | pres 0 | refused | refused | ok |
| a3_e05 | [audit 3] R8/R6: callee returns Ok(y ^ y) after write | pres 0 | refused | refused | ok |
| a3_e06 | [audit 3] R3: Ok(Some(0)) before write, Ok(None) after; caller checks only on Some(v != 0) | pres 0 | refused | refused | ok |
| a3_e06b | [audit 3] R3/R7: same callee, validates iff is_none (true) | pres 0 | certified | refused | imprecise |
| a3_e07 | [audit 3] R8: non-Result callee returning u128, 0 only before write (true) | pres 0 | certified | certified | ok |
| a3_e07b | [audit 3] R8: non-Result callee returns x >> 1 after write | pres 0 | refused | refused | ok |
| a3_e08 | [audit 3] R8: callee returns the other argument after write | pres 0 | refused | refused | ok |
| a3_e09 | [audit 3] R8: callee selects via tuple swap | pres 0 | refused | refused | ok |
| a3_e10 | [audit 3] join: e is c or d, chosen by x | pres 0 | refused | refused | ok |
| a3_e11 | [audit 3] R8: write-always callee between; c == 0 path wrote (zw) and c2 == 0 unchecked | pres 0 | refused | refused | ok |
| a3_f06 | [audit 3] R5: match 0 | 2 unchecked; callee returns Ok(2) after write | pres 0 | refused | refused | ok |
| a3_f07 | [audit 3] R8 bot-clause: callee never returns Ok(1), so the write under c == 1 is dead (true) | pres 0 | certified | refused | imprecise |
| a3_f07b | [audit 3] R8 bot-clause: callee can return Ok(1); write under c == 1 unchecked | pres 0 | refused | refused | ok |
| a3_f08 | [audit 3] R1/R8: callee's result local overwritten to Ok(0) through &mut after write | pres 0 | refused | refused | ok |
| a3_f10 | [audit 3] R1: is_ok() on a writing callee's result, returns unchecked | pres 0 | refused | refused | ok |
| a3_f10b | [audit 3] R2: matches!(r, Ok(0)) on a writing callee's result | pres 0 | refused | refused | ok |
| a3_f12 | [audit 3] R8: callee swallows a failed validation into Ok(0) after write | pres 0 | refused | refused | ok |
| a3_f12b | [audit 3] R8: same, caller returns on c == 1 only | pres 0 | refused | refused | ok |
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

unsound answers: 0 of 196 cases; imprecise (sound refusals of a true statement): 5

## Unit tests

- audit 3 risk R-a, `_3 = copy ((_4 as Ok).0: u128) as u64 (IntToInt)` must not be a payload read: [('havoc', 3)] -> ok
- audit 3 hole H1, `Result::<..>::Ok(..)` into a local not declared `core::result::Result<..>` must pin nothing: [('havoc', 6)] -> ok
- audit 3 homonym_h1_lib.rs appended to src/lib.rs: check_assumptions2.py exit 1 -> ok (rejected)
- audit 3 homonym_h2_lib.rs appended to src/lib.rs: check_assumptions2.py exit 1 -> ok (rejected)

unsound in total: 0
