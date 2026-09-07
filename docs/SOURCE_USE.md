# Source-use record

Checked on 2026-09-07 for the v0.2.0 implementation. The rows identify concrete
patterns applied to the code and documentation; source review alone is not a
validation result.

| Source | Pattern used | Applied in | Validation |
|---|---|---|---|
| [Claude Code hooks reference](https://code.claude.com/docs/en/hooks) | Stop events use JSON stdin and decision/reason output; continuation state must be considered to avoid endless blocking | Existing contract preserved in `src/hook_io.rs` and `src/main.rs`; no-matcher Stop registration in `src/init_cmd.rs`; integration recipe | Real-binary tests retain pass/block/warn/off/skip and exhausted-attempt behavior, and add fail-open configuration/storage cases |
| [GitHub Actions runner images](https://github.com/actions/runner-images) | Explicit Intel/ARM macOS labels rather than assuming a generic macOS label's architecture | `macos-15-intel` for x86-64 and `macos-15` for ARM in CI/release; current Ubuntu and Windows runners | Workflow matrix inspection; actual hosted runs are required before release completion |

Distribution decisions from the local baseline audit are implemented in
`install.sh` and `.github/workflows/release.yml`: stage the binary, verify exactly
one matching SHA256SUMS entry, test the executable, then rename it into place;
aggregate all four build outputs before one publish job. Seven network-free
installer tests cover success and failures, including preservation of an existing
executable.

No source implies that local receipts establish software correctness, independent
attestation, or adequate test coverage. These limits are stated in README.md and
SECURITY.md. Current manual CLI contracts, JSON output, detection-only doctor, and
invalid-config handling are tested directly rather than inferred from external
documentation.
