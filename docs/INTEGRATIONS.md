# Integrations

## CI with a JSON artifact

Install StopProof, dependencies, and the project's test framework first. A shell
step should invoke it directly so its status determines the step result:

```yaml
- name: Verify tests
  run: stopproof run --command 'cargo test --locked --all-targets' --strict --timeout 300 --json > stopproof-receipt.json
- name: Keep evidence even on failure
  if: always()
  uses: actions/upload-artifact@v4
  with:
    name: stopproof-receipt
    path: |
      stopproof-receipt.json
      .stopproof/last-receipt.md
```

Do not append `|| true` to the verification step. Exit 1 means a failed check or
unsaved receipt; exit 2 means the command/configuration could not be used. On an
exit-2 error, stdout is an error object rather than a verification receipt, and
no new stored receipt is created. A previous `last-receipt.md` can remain, so
inspect timestamps and the current step status before interpreting artifacts.

`--strict` requires a command; it cannot determine test coverage or distinguish
meaningful tests from a command that simply returns zero.

## Explicit configuration

Copy an example into the project root:

```sh
cp examples/rust.stopproof.json .stopproof.json
stopproof doctor --json
stopproof run --strict
```

Examples assume the indicated tools are installed. The Node example uses a
single-run test script; adjust it if the project's default script watches files.
On Windows, replace Unix-only shell syntax with syntax accepted by `cmd /C`.

## Claude Code

`stopproof init` registers this command hook, with no matcher:

```json
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          { "type": "command", "command": "stopproof", "timeout": 600 }
        ]
      }
    ]
  }
}
```

The binary must be on the environment's `PATH`; an absolute executable path also
works. Inspect existing configuration with `doctor` before enabling the hook.
The implementation retains Stop decision JSON, `stop_hook_active` awareness, and
a persisted per-session block cap. Malformed hook stdin and infrastructure errors
fail open. Tests cover the protocol using the actual binary, but compatibility
with a particular Claude Code release should also be checked in a real session.
The [official hooks reference](https://code.claude.com/docs/en/hooks) is the source
for the event contract.

To remove integration, delete only the StopProof entry from
`.claude/settings.json`, then remove `.stopproof.json` and generated receipts if
unneeded. Keep any unrelated hooks and ignore rules.

## Other agents and local workflows

Any agent or script that can run a command can invoke `stopproof run --strict
--json` and examine its exit status and receipt. Native Gemini CLI, Codex, and MCP
hook adapters are not included. Manual mode checks the command result and Git
heuristics; it has no session transcript to reconcile.
