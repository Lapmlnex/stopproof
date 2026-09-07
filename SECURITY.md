# Security policy

## Scope

StopProof is a local evidence recorder and optional completion hook. It is not a
sandbox, policy enforcement service, or independent attestation system.

- Test commands run with the invoking user's permissions through a shell. They
  can modify files, read credentials, and contact external services. Detection of
  a project command does not make that project trustworthy.
- StopProof itself makes no network requests during verification. Its test
  commands, installation, and dependency tools can use the network.
- The agent or user can edit configuration, tests, receipts, Git state, and the
  executable; hook skip/off/warn modes are intentional. Checks do not establish
  correctness, test adequacy, authorship, or resistance to deliberate evasion.
- A timeout terminates the immediate shell, not necessarily all descendants.
  Use an external job supervisor or container when isolation and complete cleanup
  are required. Output-collection grace can add up to 20 seconds.
- Receipt output can include paths, command strings, and sensitive test logs.
  Receipts are gitignored by `init`; inspect them before uploading or sharing.
- Release SHA256SUMS detects a binary/manifest mismatch. The manifest is served
  by the same release publisher; it is not an independent publisher signature.
- Local receipt writes stage files before renaming them. They are not a durable,
  multi-file transaction, and abrupt machine failure can leave incomplete state.
  A failed write is reported and never treated as a successful manual check.

Hook mode deliberately fails open for invalid input/configuration and storage
errors so it does not trap a session. Use strict manual verification in CI when
these errors must fail the job.

## Reporting

Use the repository's **Security → Report a vulnerability** option if available.
If private reporting is unavailable, open an issue requesting a private contact
without disclosing exploit details, credentials, or sensitive logs. Include the
StopProof version, OS, and a minimal reproduction when a private channel is
established.

Only the latest release is targeted for fixes. There is no guaranteed security
response time or long-term support window.
