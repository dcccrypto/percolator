# Our fork (dcccrypto/percolator@c38b6b1e): test suite on the pinned text and with patches/t15_checked_exits_c38b6b1e.patch

Command (each tree, its own target dir): cargo test -j 4 --release --features fuzz,audit-scan -- --test-threads 4

## base
    34:   Compiling percolator v0.1.1 (/Users/khubair/agent-work/e2e-proofs-novel-1121/c38-test-base)
    340:    Finished `release` profile [optimized] target(s) in 3m 55s
    test binaries: 99 passed: 1480 failed: 0 ignored: 19
    exit=0
## patch
    34:   Compiling percolator v0.1.1 (/Users/khubair/agent-work/e2e-proofs-novel-1121/c38-test-patch)
    340:    Finished `release` profile [optimized] target(s) in 4m 37s
    test binaries: 99 passed: 1480 failed: 0 ignored: 19
    exit=0

Test names identical in both trees: yes
Results per test identical: yes

A test result, not a proof that the patch leaves behaviour unchanged.
