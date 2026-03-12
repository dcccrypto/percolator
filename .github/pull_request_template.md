## Summary

<!-- What changed and why -->

## How to test

<!-- Steps to reproduce / verify -->

## Checklist

- [ ] Tests pass (`cargo test`)
- [ ] Clippy clean (`cargo clippy -- -W clippy::all`)
- [ ] Format check (`cargo fmt --check`)
- [ ] **If this PR touches math, proof logic, or invariant code**: run Kani locally before merging
  ```bash
  # One-time setup
  cargo install --locked kani-verifier && cargo kani setup
  # Run relevant harnesses
  cargo kani --tests --harness proof_
  ```
  Kani is **not** run automatically on every PR (removed in #47). Use the [Kani (Manual)](../../actions/workflows/kani-manual.yml) workflow for on-demand runs.

## Related

<!-- Task ID, issue, or PR -->
