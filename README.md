# stopproof

**Your agent says it's done. stopproof makes it prove it.**

[![CI](https://github.com/Lapmlnex/stopproof/actions/workflows/ci.yml/badge.svg)](https://github.com/Lapmlnex/stopproof/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Coding agents routinely claim success for work that isn't done — files "updated" that never changed, "all tests passing" without running them. It's a P1-class bug report in Gemini CLI, and METR found roughly half of AI PRs with passing tests still get rejected by maintainers.

`stopproof` is a single-binary [Claude Code Stop hook](https://code.claude.com/docs/en/hooks) that fires the moment your agent tries to finish and demands evidence:

1. **Reconciles claims against reality.** Parses the agent's final message and the session transcript, then checks the actual `git` state. A file the agent claims to have changed with no diff and no commit behind it = **phantom edit, blocked**.
2. **Re-runs your tests — for real.** Auto-detects the project's test command (`package.json`, `Cargo.toml`, `go.mod`, pytest, `Makefile`) or uses the one you declare, executes it, and reads the actual exit code. The agent's opinion is not consulted.
3. **Blocks or signs off.** On failure, the stop is blocked and the agent gets a surgical fix-list — it must keep working. On success, you get a **verification receipt**: an auditable JSON + Markdown record of exactly what was checked.

**Zero LLM calls. Zero API cost. Zero trust in the agent.** Pure deterministic checks, all local.

## What it looks like

Agent tries to finish with a failing suite:

```
stopproof verification FAILED (attempt 1/2):
  - tests: `npm test --silent` exited 1 (from package.json)
  - diff-reconciliation: claimed as changed, but git shows no
    evidence (phantom edit?): src/auth.ts

--- test output (tail) ---
  FAIL src/auth.test.ts > refresh token rotation
  Expected 200, received 401

Do not claim completion yet. Fix the issues above, re-run the
checks, and finish only when they pass.
```

The session keeps going until the work is actually done — then:

```
stopproof: verified — tests passed (`npm test --silent`) · diff consistent
· receipt: .stopproof/last-receipt.md
```

## Install

**Prebuilt binary (Linux/macOS):**

```sh
curl -fsSL https://raw.githubusercontent.com/Lapmlnex/stopproof/main/install.sh | sh
```

**Windows:** grab `stopproof-x86_64-pc-windows-msvc.exe` from [releases](https://github.com/Lapmlnex/stopproof/releases/latest) and put it on your `PATH`. Yes, it actually works on Windows — the whole test suite runs on Windows in CI.

**From source:**

```sh
cargo install --git https://github.com/Lapmlnex/stopproof
```

Then, inside any project:

```sh
stopproof init
```

This writes a default `.stopproof.json`, registers the Stop hook in `.claude/settings.json` (merging carefully — your existing hooks are preserved and backed up), and gitignores the receipt directory. That's it.

## Why deterministic beats "ask the model if it's done"

| Approach | Example | Weakness |
|---|---|---|
| Ask an LLM to judge completion | `prompt`/`agent` hooks | Nondeterministic, costs tokens, can be talked into agreeing |
| Trust the agent's own log | verify-before-stop hooks | The model writes its own "VERIFIED" entry |
| Match a magic string | ralph-loop `--completion-promise` | Agent learns to print the string |
| Gate individual edits | tdd-guard | Guards *during* work; doesn't check the final claim |
| **Ground truth: git + exit codes** | **stopproof** | Can't be sweet-talked |

stopproof composes with all of the above — it's the last line, not a replacement. Run tdd-guard during the session and stopproof at the end.

## Configuration

`.stopproof.json` in the project root (all fields optional):

```json
{
  "mode": "enforce",
  "test_command": "",
  "test_timeout_secs": 300,
  "max_attempts": 2,
  "verify_when": "auto",
  "require_tests": false,
  "claim_keywords": [],
  "ignore_paths": [".stopproof/"],
  "receipt_dir": ".stopproof"
}
```

| Field | Meaning |
|---|---|
| `mode` | `enforce` (block failed stops) · `warn` (report, never block) · `off` |
| `test_command` | Explicit command; empty = auto-detect |
| `test_timeout_secs` | Kill the test run after this long |
| `max_attempts` | Blocks per session before letting the stop through (loop safety) |
| `verify_when` | `auto` = when the session edited files, or claims completion over a dirty tree; `always` = every stop |
| `require_tests` | Fail verification when no test command can be found |
| `claim_keywords` | Extra phrases that count as a completion claim |
| `ignore_paths` | Path substrings excluded from reconciliation |
| `receipt_dir` | Where receipts and state live |

## Commands

| Command | What it does |
|---|---|
| `stopproof` / `stopproof hook` | Hook mode — reads Claude Code's JSON on stdin (you don't run this yourself) |
| `stopproof run` | Manual verification of the current directory; exit code = verdict. Works great as a CI step or a git pre-push hook |
| `stopproof init` | Install hook + config into the current project |
| `stopproof init --print` | Print the snippets instead of writing files |

## Receipts

Every verification writes `.stopproof/receipts/<timestamp>-<verdict>.json` and a human-readable `.stopproof/last-receipt.md`:

```markdown
# stopproof receipt — PASS

- generated: 2026-07-12T14:03:22Z
- session: `9f2c81aa…`

## Checks

- [PASS] **diff-reconciliation** — final-message file claims are backed by git evidence
- [PASS] **tests** — `cargo test --quiet` passed in 8214ms (from Cargo.toml)

## Evidence

- files edited in session: src/auth.rs, tests/auth_test.rs
- git changes: src/auth.rs, tests/auth_test.rs
- test command: `cargo test --quiet` (exit 0)
```

Attach it to a PR, paste it in a standup, or just stop wondering whether "done" meant done.

## FAQ

**Can this loop forever?** No. `max_attempts` (default 2) caps how many times stopproof blocks a session, independent of Claude Code's own stop-hook block cap. After that it lets the stop through with a loudly-FAILED receipt — you stay in control.

**My test suite takes 20 minutes.** Point `test_command` at a fast subset (`npm test -- --changed`, `pytest tests/unit -q`), raise `test_timeout_secs`, and raise the hook `timeout` in `.claude/settings.json` (seconds).

**Not a git repo?** Claim reconciliation is skipped (reported as info); tests still run.

**What if the agent deletes the failing tests?** The test-integrity check flags deleted test files and net-removed assertion lines in the receipt. Warn-only in v1; hardening is on the roadmap.

**Escape hatch?** `STOPPROOF_SKIP=1` env var skips everything; `"mode": "warn"` reports without blocking; `stopproof init` is fully reversible (delete the hook entry and `.stopproof.json`).

**Privacy?** 100% local. stopproof makes no network calls of any kind.

## Roadmap

- Gemini CLI adapter (`AfterAgent` hook) — same guarantee, second CLI
- "Verified loop" mode: loop-until-proven for autonomous runs (vs. loop-until-string)
- Claude Code plugin marketplace packaging
- Test-integrity hardening: baseline snapshots, tamper detection as a failing check
- PR comment mode: post the receipt to the pull request

## Contributing

Issues and PRs welcome — see [CONTRIBUTING.md](CONTRIBUTING.md). The codebase is small (~1,500 lines, two dependencies: serde + serde_json) and reviewable in one sitting, which is the point: a trust tool should be auditable.

## License

[MIT](LICENSE)
