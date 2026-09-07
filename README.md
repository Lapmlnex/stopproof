# StopProof

**Completion checks with receipts for coding agents and CI.**

[![CI](https://github.com/Lapmlnex/stopproof/actions/workflows/ci.yml/badge.svg)](https://github.com/Lapmlnex/stopproof/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

StopProof runs your project's test command and records what happened in JSON and
Markdown. As a Claude Code Stop hook, it also checks whether file claims in the
final message have supporting transcript and Git evidence. Failed checks can ask
the agent to continue, with a configurable limit on repeated blocks.

The checks use local code and process exit statuses, without calling a model.
A passing receipt means the recorded checks passed; it does not prove that the
software is correct or that its tests are sufficient.

## Try a manual check

After [installing](#install), run these commands inside a trusted project:

```sh
stopproof doctor
stopproof run --command 'npm test -- --run' --timeout 120 --strict --json
```

`doctor` displays the effective configuration and detected test command without
executing it. `run` executes the command and writes a receipt. Replace the example
command with the project's actual non-interactive test command.

A minimal example that needs no test framework:

```sh
stopproof run --command 'exit 0' --strict --json   # exit 0, verdict "pass"
stopproof run --command 'exit 1' --json            # exit 1, verdict "fail"
```

The first example proves only that `exit 0` succeeded. For useful evidence, run
real tests. `--strict` fails when no command is available. Without strict mode or
`require_tests: true`, an empty project produces a warning and can pass.

## Install

### From source

Install the v0.2.0 release with a Rust toolchain and Git:

```sh
cargo install --git https://github.com/Lapmlnex/stopproof --tag v0.2.0 --locked
```

For development from a checkout:

```sh
cargo install --path . --locked
```

### Linux and macOS binaries

Download the installer, inspect it, then run it:

```sh
curl -fsSLo install-stopproof.sh https://raw.githubusercontent.com/Lapmlnex/stopproof/v0.2.0/install.sh
sh install-stopproof.sh
```

It installs into `~/.local/bin` by default. The binary and `SHA256SUMS` come from
the same release. A temporary download is checked and tested with `--version`
before replacing an existing executable. Download or checksum failures preserve
the old executable. Checksums detect mismatches; they are not independent
signatures of the release publisher.

```sh
STOPPROOF_INSTALL_DIR="$HOME/bin" sh install-stopproof.sh
STOPPROOF_VERSION=latest sh install-stopproof.sh
```

### Windows and supported assets

Download the matching binary and `SHA256SUMS` from the
[v0.2.0 release](https://github.com/Lapmlnex/stopproof/releases/tag/v0.2.0):

| Platform | Asset |
|---|---|
| Linux x86-64, musl | `stopproof-x86_64-unknown-linux-musl` |
| macOS Apple silicon | `stopproof-aarch64-apple-darwin` |
| macOS Intel | `stopproof-x86_64-apple-darwin` |
| Windows x86-64 | `stopproof-x86_64-pc-windows-msvc.exe` |

On Windows, run `Get-FileHash .\stopproof-x86_64-pc-windows-msvc.exe -Algorithm SHA256`
and compare the result with that asset's line in `SHA256SUMS`. After it matches,
rename the executable to `stopproof.exe` and put it in a directory on `PATH`.
The shell installer supports Linux and macOS. Other architectures require a
source build.

## Claude Code integration

```sh
cd your-project
stopproof doctor
stopproof init
```

`init` creates `.stopproof.json`, merges a command hook into
`.claude/settings.json`, backs up modified existing settings, and adds the receipt
directory to `.gitignore`. Invalid existing configuration or settings cause an
error; the settings file is preserved. Inspect snippets first with
`stopproof init --print`.

The hook reads the [Claude Code Stop protocol](https://code.claude.com/docs/en/hooks).
When a session edits files, or claims completion over a dirty tree, it runs checks.
A failure in `enforce` mode returns a block reason. By default, after two blocked
attempts the hook allows the session to finish with a failure notice. `warn`
reports without blocking; `off` skips hook checks. `STOPPROOF_SKIP=1` is a hook-only
escape hatch. Manual verification still runs regardless of these hook settings.

The test-integrity heuristic warns about deleted test files and net-removed
assertions. Claim extraction and Git reconciliation are heuristics with incomplete
coverage. Neither is a security boundary.

## Commands and exit codes

| Command | Behavior |
|---|---|
| `stopproof` or `stopproof hook` | Read a Claude Code Stop event from stdin |
| `stopproof run` | Detect and execute a test command; save receipts |
| `stopproof run --command '…'` | Override the configured/detected shell command |
| `stopproof run --timeout 120` | Override the timeout with positive whole seconds |
| `stopproof run --strict` | Require a configured or detected command |
| `stopproof run --json` | Write one JSON receipt to stdout, without human text |
| `stopproof doctor [--json]` | Show configuration, command, and warnings; no test execution |
| `stopproof init [--print]` | Register the hook, or print snippets |
| `stopproof --help`, `stopproof --version` | Usage or version |

Options on `run` can be combined. Unknown and repeated options are errors.

- **0:** manual checks pass, or a diagnostic/installation command succeeds.
- **1:** a manual check fails, times out, lacks required tests, or cannot save its
  receipts; also used for installation write failures.
- **2:** invalid arguments, unreadable configuration, or invalid configuration/settings.

`run --json` and `doctor --json` return a JSON object with an `error` field for
configuration or usage errors. A valid `doctor` exits 0 even if it warns that no
tests were found. Hook mode uses exit 0 for valid hook invocations: deliberate
blocks are JSON decisions, while internal configuration/storage failures allow
the session to finish with a notice.

## Configuration

All fields in `.stopproof.json` are optional; unknown fields are rejected. This
example requires tests for both hook and manual runs:

```json
{
  "mode": "enforce",
  "test_command": "cargo test --locked --all-targets",
  "test_timeout_secs": 300,
  "max_attempts": 2,
  "verify_when": "auto",
  "require_tests": true,
  "claim_keywords": [],
  "ignore_paths": [".stopproof/"],
  "receipt_dir": ".stopproof"
}
```

| Field | Default and meaning |
|---|---|
| `mode` | `enforce`; hook behavior: `enforce`, `warn`, or `off` |
| `test_command` | Empty; auto-detect if no explicit command is supplied |
| `test_timeout_secs` | `300`; positive whole seconds before killing the shell |
| `max_attempts` | `2`; number of failed hook blocks before allowing completion |
| `verify_when` | `auto`; use `always` to check every Stop |
| `require_tests` | `false`; fail when no test command is available if true |
| `claim_keywords` | `[]`; extra phrases recognized as completion claims |
| `ignore_paths` | `[".stopproof/"]`; path substrings excluded from reconciliation |
| `receipt_dir` | `.stopproof`; nonempty output directory |

Detection order: explicit command, `package.json`, `Cargo.toml`, `go.mod`, pytest
markers, then a Makefile `test:` target. Detection identifies a command; it does
not install dependencies or establish that the command is appropriate. Commands
run through `sh -c` on Unix and `cmd /C` on Windows, with `STOPPROOF_ACTIVE=1`.
Use non-interactive commands that terminate on their own.

See [example configurations](examples) and [CI and agent integration recipes](docs/INTEGRATIONS.md).

## Receipts and limits

Each completed verification attempts to write
`.stopproof/receipts/<timestamp>-a<attempt>-p<pid>-<verdict>.json` and
`.stopproof/last-receipt.md`. JSON includes `verdict`, `checks`, edited/claimed/Git
paths, `test_command`, `test_exit`, and `test_output_tail`. In manual mode, no
transcript is present and claim reconciliation is reported as informational.

Output capture keeps at most 64 KiB per stdout/stderr stream while draining the
pipes. The receipt includes the last 40 lines, capped at approximately 3.5 KiB,
with stderr appended after stdout; it is an excerpt, not a complete ordered log.

A timeout kills the immediate shell. Descendant processes may survive, and output
collection can add up to 20 seconds of grace after the shell exits. StopProof does
not provide process isolation. Keep the outer Claude hook timeout above the test
timeout plus output-collection overhead; `init` sets it to 600 seconds.

Local files, settings, commands, and receipts remain editable. A test command can
change files, access the network, or expose secrets in output. Run only in trusted
projects, inspect receipts before sharing them, and keep independent review and CI
controls for consequential changes. See [SECURITY.md](SECURITY.md).

## Contribute

Start with [CONTRIBUTING.md](CONTRIBUTING.md), the [changelog](CHANGELOG.md), and the
[source-use notes](docs/SOURCE_USE.md). Small, test-backed improvements to command
detection, receipt formats, or hook compatibility are welcome.

[MIT license](LICENSE).
