# Contributing to StopProof

The project welcomes small changes that make completion checks easier to inspect
and reproduce. Start with a focused issue or pull request describing the behavior
and a minimal example.

## Development

Install a Rust toolchain with rustfmt and clippy. Python 3 is used only for the
POSIX installer tests; it is not a dependency of the StopProof binary.

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests -p test_installer.py -v
```

Add a regression test before changing behavior. `tests/cli.rs` runs the actual
binary and speaks the hook protocol; module tests cover isolated logic.
`tests/test_installer.py` supplies local download fixtures to the real shell
installer and makes no network requests. CI runs Rust tests on Linux, Windows,
and both macOS architectures; installer tests run on POSIX platforms.

## Contracts to preserve

- Manual verification: exit 0 pass, 1 failed verdict or unsaved receipt, 2 usage or
  configuration error. JSON mode emits one object on stdout and no human prose.
- Hook mode: deliberate failure blocks use protocol JSON and exit 0; internal
  failures allow completion. Keep the per-session block cap and skip mechanism.
- `doctor` must not execute project commands or write receipts/configuration.
- Passing means only the recorded checks passed. Avoid correctness, security,
  adoption, or performance claims without appropriate evidence.
- Verification itself has no model calls and no networking. Test commands can
  have side effects; do not hide this distinction.

## Useful first contributions

1. Add a command detector with positive, absent, and placeholder fixtures. Discuss
   detection precedence before changing it.
2. Add transcript/claim fixtures for quoted paths, Markdown links, and Windows
   path forms. Preserve unrelated settings and avoid false claims of coverage.
3. Propose additive receipt exports such as JUnit or GitHub job summaries, keeping
   raw check outcomes and existing JSON fields available.
4. Improve platform-specific integration recipes by testing them and documenting
   the exact environment.

## Maintainer release checklist

1. Update `Cargo.toml`, `Cargo.lock`, the installer default, release links, and
   `CHANGELOG.md`. Run the checks above and obtain an independent code review.
2. Merge the tested commit, tag `vX.Y.Z`, and push the tag. Release jobs require the
   tag to match the Cargo version and test every target before publishing.
3. The build matrix uploads four binary artifacts. One dependent publish job
   checks the complete set, generates SHA256SUMS, and uploads the release.
4. Confirm every expected public asset, verify its checksum, and test a fresh
   installation outside the development checkout. Do not report publication
   complete solely because a local build succeeded.
