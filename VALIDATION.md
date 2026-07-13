# Why stopproof — market validation (researched 2026-07-12)

Before writing a line of code, all three gap ideas from the July 12 GitHub Trends report were validated against the live ecosystem (GitHub API, official docs, changelogs). Summary of what the research found and why the verifier won.

## Scorecard

| Gap idea | Openness (10 = wide open) | Verdict |
|---|---|---|
| Skill package manager + linter | **2/10** | Dead on arrival — already owned |
| Cross-CLI cost guard | **4/10** | Exact idea claimed 3 weeks ago; natives shipping |
| **"Did it actually work?" verifier** | **6/10** | Pain validated, wedge unclaimed → **build** |

## Gap 1: Skill package manager — killed by incumbents

The premise ("skill distribution is copy-paste") was ~6 months stale at research time. [`vercel-labs/skills`](https://github.com/vercel-labs/skills) (`npx skills`, 22.4k★, created Jan 2026) already installs skills from any git repo into per-agent layouts for **71 agents**, with a lockfile, updates, and a 91k-skill registry ([skills.sh](https://skills.sh)) including security audits. First-party installers exist everywhere: Claude Code plugins/marketplaces with versioned auto-updates and `claude plugin validate`, GitHub's `gh skill`, Gemini's `gemini skills install`. The early independent mover ([openskills](https://github.com/numman-ali/openskills), 10.6k★) was overtaken and stalled within months — a cautionary tale about competing with a platform-backed default. The residual lint niche is fragmented micro-tools (largest: 187★) and feature-sized, not project-sized.

## Gap 2: Cost guard — the exact idea was just claimed

[`phuryn/burnstop`](https://github.com/phuryn/burnstop) (created 2026-06-22) is precisely the proposed tool: a Stop hook recomputing session spend from local transcripts with `$`/token caps. Claude Code itself added `--max-budget-usd` (print mode), and Codex merged native `rollout_budget` token caps in June 2026. Meanwhile [`ccusage`](https://github.com/ccusage/ccusage) (17k★) proves the demand is in *reporting*, where the category king already exists. Enforcement remains winnable only as a harder superset (cross-CLI, pre-gated, bounded overshoot) — a bigger bet with an incumbent racing ahead on the easy 80%.

## Gap 3: Verifier — validated pain, unclaimed wedge

The pain is loud and cited: false-success/hallucinated-edit P1s in Gemini CLI ([#25783](https://github.com/google-gemini/gemini-cli/issues/25783), [#4865](https://github.com/google-gemini/gemini-cli/issues/4865)), METR 2026 finding ~50% of tests-passing AI PRs still rejected, agents hardcoding test answers up to 44% (EvilGenie). Yet:

| Closest competitor | Stars | Why it doesn't own the wedge |
|---|---|---|
| [tdd-guard](https://github.com/nizos/tdd-guard) | 2.1k | Gates edits *during* work (TDD discipline), not the completion claim |
| [ralph-loop](https://github.com/anthropics/claude-plugins-official/tree/main/plugins/ralph-loop) (official, 191k installs) | — | "Done" = magic-string match, documented as unreliable |
| [claude-verify-before-stop](https://github.com/ianymu/claude-verify-before-stop) | 0 | Honor system — the model writes its own VERIFIED log entry |
| [make-no-mistakes](https://github.com/momomuchu/make-no-mistakes) | 1 | Conceptually deep, heavyweight opt-in harness; near-zero traction |
| Native `prompt`/`agent` hooks | — | LLM-judged completion: nondeterministic, token cost, persuadable |

**The unclaimed wedge:** a lightweight, deterministic, install-once Stop gate that (a) reconciles the agent's *claims* against the actual git diff — the phantom-edit class of failure, (b) re-runs the real test command and reads the real exit code, (c) blocks with a surgical fix-list, and (d) emits an auditable evidence receipt — with zero LLM calls. Nothing shipping does claim-vs-diff reconciliation, and receipts exist nowhere as a packaged artifact.

Risk log: Anthropic's native agent-hooks include a worked "verify tests before stop" example (the *mechanism* is commoditized — the *product* isn't); tdd-guard's author is expanding scope (Probity); Claude Code caps consecutive Stop-hook blocks (~8) — stopproof handles this with its own `max_attempts` ledger.

## Sources

Full URL lists live in the research trail: GitHub repos and issues linked above; official docs at [code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks), [hooks-guide](https://code.claude.com/docs/en/hooks-guide), [checkpointing](https://code.claude.com/docs/en/checkpointing); Gemini hooks GA coverage ([Google blog](https://developers.googleblog.com/tailor-gemini-cli-to-your-workflow-with-hooks/), [reference](https://geminicli.com/docs/hooks/reference/)); ecosystem state via [Trendshift](https://trendshift.io/), the StartupCorners dev-tools digest, and the agents-radar AI CLI digest.
