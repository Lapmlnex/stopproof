# Changelog

## 0.2.0 — 2026-09-07

- Add `run --command`, `--timeout`, `--strict`, and `--json` for scripted checks.
- Add human and JSON `doctor` output without executing project commands.
- Reject invalid manual configuration and unknown arguments with exit 2. Preserve
  defaults when no configuration file exists. Hook configuration errors fail open.
- Treat unsaved receipts as failed manual verification; hook notices no longer
  claim a receipt was saved when persistence fails.
- Bound stdout/stderr retention while continuing to drain verbose test processes.
  Avoid timer overflow for very large positive timeout values.
- Reject malformed existing Claude settings during initialization and preserve
  the original file, including malformed nested hook structures and dangling
  settings symlinks. Preserve valid command, prompt, agent, HTTP, and MCP tool
  hooks. Report backup and ignore-file write failures.
- Build on Linux musl, macOS Intel, macOS Apple silicon, and Windows. Aggregate all
  binaries before one release job publishes assets and SHA256SUMS.
- Verify staged installer downloads and preserve an existing executable on
  download, checksum, or executable smoke-test failure.
- Add strict CI, installer regressions, configuration examples, contributor
  entrypoints, and documented security/timeout limitations.

## 0.1.0

Initial source implementation: Claude Code Stop checks, command detection,
Git/transcript reconciliation, bounded block attempts, and JSON/Markdown receipts.
No binary release was published for v0.1.0.
