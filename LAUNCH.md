# StopProof v0.2.0 announcement draft

This is a draft for maintainer use. Publication of a release does not authorize
posting to communities, contacting people, or sending announcements.

> StopProof records completion checks for coding agents and CI. It can rerun a
> project's test command, return a machine-readable receipt, and optionally ask
> Claude Code to continue when checks fail. v0.2 adds strict CLI options,
> non-executing diagnostics, explicit configuration errors, bounded output
> capture, and checksum-verified binary installation.
>
> A passing receipt tells you which checks passed. It does not prove software
> correctness or sufficient test coverage. Local configuration and receipts
> remain editable.
>
> Try `stopproof doctor`, then run your test command with
> `stopproof run --command 'your test command' --strict --json`.
>
> Source and installation instructions:
> https://github.com/Lapmlnex/stopproof

Before using this draft, confirm the public v0.2.0 assets, CI results, and a fresh
installation. Share an actual sanitized receipt if useful. Describe only features
present in the release; no native Gemini or Codex hook adapter is included.

Useful feedback concerns reproducible failures, command-detection mistakes,
configuration clarity, and whether receipts help users review work. Adoption,
star counts, or contributor growth cannot be predicted from the implementation.
