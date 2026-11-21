| Mutation | What it does | Sound answer (check 0, check 1) | Tool (check 0, check 1) | Verdict |
|---|---|---|---|---|
| M00-original | unchanged | certified, certified | certified, certified | ok |
| M01-header-write-after-checks | direct write to the market header after both checks | refused, refused | refused, refused | ok |
| M02-account-write-after-checks | direct write to the account after both checks (market untouched) | certified, refused | certified, refused | ok |
| M03-no-shape-check | validate_shape removed | refused, certified | refused, certified | ok |
| M04-shape-result-ignored | validate_shape called, result dropped | refused, certified | refused, certified | ok |
| M05-mut-callee-after-checks | a `&mut self` method called after the checks | refused, refused | refused, refused | ok |
| M06-account-callee-after-checks | a `&mut` account method called after the checks | certified, refused | certified, refused | ok |
| M07-write-through-reborrow | write through a chain of reborrows | refused, refused | refused, refused | ok |
| M08-write-in-closure | write inside a closure that captures self | refused, refused | refused, refused | ok |
| M09-pure-read-after-checks | reads and a `&self` method after the checks (harmless) | certified, certified | certified, certified | ok |
| M10-early-ok | early Ok return between the core and the checks | refused, refused | refused, refused | ok |
| M11-inverted-check | Ok returned exactly when validate_shape fails | refused, refused | refused, refused | ok |
| M12-check-then-core | checks moved before the core | refused, refused | refused, refused | ok |
| M13-mem-swap | core::mem::swap on a header field after the checks | refused, refused | refused, refused | ok |
| M14-match-on-check | validate_shape handled by an explicit match (harmless) | certified, certified | certified, certified | ok |
| M15-markets-slice-reverse | a `&mut [T]` method (`reverse`) on the market slice after the checks | refused, refused | refused, refused | ok |
| M16-local-write-after-checks | write to a local array only (harmless) | certified, certified | certified, certified | ok |
| M17-check-other-account | account written after its check, shape re-checked | certified, refused | certified, refused | ok |

unsound answers: 0 of 18 mutations
