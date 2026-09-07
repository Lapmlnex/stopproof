# Validation scope

The v0.2.0 development checks exercise the real CLI plus isolated parsing,
receipt, detection, and installer logic:

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tests -p test_installer.py -v
```

Rust tests cover the existing Stop hook protocol and v0.2 manual command options,
JSON output, usage/configuration errors, strict missing-test rejection,
non-executing doctor, receipt-write failures, invalid initialization settings,
large output, and timeout behavior. The bounded-reader test consumes more than
2 MiB, verifies EOF was reached, and verifies only 64 KiB is retained.

Installer tests use local fixtures with the real shell installer. They verify
replacement only after a successful checksum and executable check, and preserve
the old binary on partial download, missing manifest, missing checksum, duplicate
checksum, mismatched checksum, and unusable executable.

The release workflow separately tests and builds four target binaries, then
publishes only after all builds succeed. Actual hosted CI results, public release
assets, and a fresh download/install check must be verified for the tagged commit;
local tests alone do not establish release availability or cross-platform success.

These checks establish specific software behavior. They do not validate market
novelty, future adoption, software correctness of projects using StopProof, or
resistance to a user/agent deliberately editing checks and receipts. Historical
market-positioning claims from v0.1 are not part of the v0.2 validation evidence.
