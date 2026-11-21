# Engine test suite on the pinned text and on the text with patches/t15_checked_exits.patch

Command (each tree, own target dir): cargo test -j 4 --release --features fuzz,audit-scan -- --test-threads 4
Trees: git archive 08d4fd09 (Cargo.toml Cargo.lock src tests); the patched tree differs only in src/v16.rs.

## base
    34:   Compiling percolator v0.1.1 (/Users/khubair/agent-work/e2e-proofs-novel-1121/test-base)
    187:    Finished `release` profile [optimized] target(s) in 1m 55s
    test binaries: 40 passed: 528 failed: 0 ignored: 0
    exit=0
## patch
    34:   Compiling percolator v0.1.1 (/Users/khubair/agent-work/e2e-proofs-novel-1121/test-patch)
    187:    Finished `release` profile [optimized] target(s) in 2m 44s
    test binaries: 40 passed: 528 failed: 0 ignored: 0
    exit=0

Names of tests run are identical in both trees: yes
Tests whose name contains 'resolved' and 'close': 30

Note: a first run of the patched tree shared the baseline's target dir; cargo reported 'Finished in 0.02s' and re-ran the baseline binaries. That run was discarded and is not counted.
This is a test result, not a proof that the patch leaves behaviour unchanged: the added checks can turn an Ok into an Err on a state that fails them.
