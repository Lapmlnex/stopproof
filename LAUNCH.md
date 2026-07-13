# stopproof — launch kit

*(Internal playbook — delete this file, or keep it; transparency also markets.)*

## Positioning

One sentence: **"Your agent says it's done. stopproof makes it prove it."**
One paragraph: Deterministic completion receipts for coding agents. A single Rust binary that hooks Claude Code's Stop event, reconciles the agent's claims against the actual git diff, re-runs your real test command, and blocks the "done" until the evidence exists. No LLM judge, no API cost, no trust in the model.

Anchor the story in third-party pain, not opinion: Gemini CLI hallucinated-edit P1s, METR's ~50% rejected "passing" PRs, ralph-loop's own docs admitting string-matched completion is unreliable.

## Pre-flight (day 0)

1. Create the GitHub repo `stopproof`, push, confirm **CI is green on all 3 OSes** — the Windows badge is a differentiator, mention it.
2. Repo polish: description = the one-liner; topics: `claude-code`, `claude-code-hooks`, `ai-agents`, `agent-verification`, `rust`, `developer-tools`, `llm`; pin a "good first issue" or two (e.g. "add gradle test detection").
3. Tag `v0.1.0` → release workflow uploads binaries → test `install.sh` end-to-end once.
4. Record the demo GIF with [vhs](https://github.com/charmbracelet/vhs) using `demo/demo.tape` — the money shot is the **blocked stop message**, 20s max, top of the README.
5. Dogfood for one day on a real project; screenshot a real receipt.

## Channels (in order)

1. **r/ClaudeAI + r/ClaudeCode (day 1, soft launch).** Title: "I got tired of Claude saying 'done, all tests pass' when they don't — so I built a Stop hook that makes it prove it." Lead with the blocked-output screenshot. Answer every comment for 48h; ship requested tweaks same-day and say so.
2. **X/Twitter (day 1).** Thread: pain (screenshots of agents lying) → 20s GIF → "zero LLM calls, it's all git + exit codes" → install one-liner. Tag nobody; let it travel.
3. **Show HN (day 2–3, Tue–Thu ~14:00 UTC).** Title: "Show HN: Stopproof – my coding agent can't say 'done' unless the tests actually pass". First comment: why deterministic beats LLM-as-judge, the phantom-edit failure class, honest limitations (claim heuristics, warn-only test-integrity), roadmap. HN respects candor about limits.
4. **Awesome lists (week 1).** PRs to hesreallyhim/awesome-claude-code, ianymu/awesome-claude-code-hooks, punkpeye/awesome-ai-agents. One-line, neutral descriptions.
5. **Trendshift/OrangeBot pick you up automatically** from GitHub trending velocity — the goal of channels 1–4 is to concentrate stars into a 48h window (that's what trending algorithms measure). Don't dribble the launch across two weeks.

## Content angles (weeks 1–4)

- "I validated 3 trending-gap ideas before building; two were already dead" — publish a cleaned VALIDATION.md as a blog/HN post. Meta-content about *process* travels far.
- "The taxonomy of agent lies" — phantom edits, magic-string completion, self-written VERIFIED logs, weakened tests. stopproof as the receipt.
- "Receipts in CI": `stopproof run` as a pre-merge job.

## Community flywheel

- Respond to issues < 24h for the first month; tag `good first issue` generously.
- Ship v0.1.x weekly from real feedback (visible momentum > feature count).
- v0.2 headline: **Gemini CLI `AfterAgent` adapter** — unlocks "works across CLIs" story and a second launch cycle. Then: plugin-marketplace packaging, verified-loop mode.

## Risks & counters

- *ianymu ships their commercial hook pack harder* → stay MIT, stay single-binary, win on trust ("the verifier you can audit in one sitting").
- *tdd-guard's author (nizos) expands into completion verification* → potential ally: propose interop (tdd-guard during, stopproof at stop) before rivalry.
- *Anthropic productizes a verify plugin* → your moat is cross-CLI + receipts as artifacts; accelerate the Gemini adapter if this lands.
- *Claim-extraction false positives annoy users* → `mode: "warn"` is the safe default to recommend for skeptics; make the first-run experience forgiving.

## Success metrics

- Week 1: 300★, one front-page moment (HN top-10 or 500-upvote Reddit), 5 external issues.
- Month 1: 1,500★, first external contributor PR merged, appears on a Trendshift daily list.
- Signal to double down: people posting *their receipts* unprompted.
