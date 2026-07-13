# Contributing to stopproof

Thanks for helping make agents honest.

## Ground rules

- **Determinism is the product.** No LLM calls, no network calls, no heuristics that can't be explained in one sentence in a receipt. PRs that add "ask a model" paths will be declined with love.
- **Never brick a session.** Hook mode must fail open: any internal error → exit 0, no block. Blocking is only ever a deliberate verdict.
- **Windows is first-class.** CI runs the full suite on Linux, macOS, and Windows; keep it green on all three.

## Dev loop

```sh
cargo test --all-targets   # unit + end-to-end (spawns the real binary)
cargo clippy --all-targets
cargo fmt
```

The end-to-end tests in `tests/cli.rs` speak the actual hook protocol over stdin — if you change behavior, add a fixture there, not just a unit test.

## Good first issues

- New test-command detectors (gradle, maven, mix, dotnet, composer…)
- Better claim extraction (quoted paths, backtick spans, markdown links)
- Receipt output formats (JUnit XML, GitHub Actions summary)

## Releases

Maintainers: bump `Cargo.toml` version, tag `vX.Y.Z`, push the tag — the release workflow builds and uploads binaries.
