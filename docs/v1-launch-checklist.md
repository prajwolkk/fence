# V1 Launch Checklist

This is the standard for a viral-ready first version: every feature we advertise must be easy to try, hard to misunderstand, and boringly reliable.

## Launch Promise

Fence records architectural decisions, serves local architectural memory, and blocks PRs when meaningful code changes ship without recorded intent.

## Add Before V1

- `fence demo`: one-command demo repo showing Sentinel fail, decision log, Sentinel pass, and local UI.
- `fence ask <query>`: local architectural-memory search that cites decision IDs.
- `fence agent-check --staged`: preflight guardrail for AI coding agents.
- `fence sentinel check --markdown`: PR-summary-ready Sentinel output.
- `fence sentinel init --github --yes`: non-interactive GitHub Action setup.
- `scripts/launch-smoke.sh`: repeatable end-to-end launch certification.
- Release binaries for Linux and macOS.
- README above-the-fold demo with screenshots.
- Agent instructions for Codex, Claude Code, and Cursor.
- Docs for solo flow, team flow, Sentinel flow, and release verification.
- GitHub Action sample using release binaries.
- Security policy, code of conduct, contributing guide, issue templates, labels, and release checklist.

## Edit Before V1

- Keep README focused on the core loop, not every feature.
- Keep `docs/commands.md` exhaustive and copy-pasteable.
- Keep `docs/demo.md` aligned with real CLI output.
- Keep Sentinel messages specific: changed files, scores, threshold, and exact fix.
- Keep web UI self-contained and offline.
- Keep `fence.toml` defaults conservative and predictable.

## Remove Before V1

- Any README promise that is not implemented.
- Any dependency that is not needed for the launch path.
- Any generated artifact that should not be tracked.
- Any hidden network requirement in the local CLI or UI.
- Any ambiguous docs saying “later” without making clear whether it is in v1.

## Edge Cases To Handle

- Repo has no Git directory.
- Repo has no remote.
- Repo has no decisions yet.
- Repo has stale, deprecated, or superseded decisions.
- `fence init --yes` runs in an existing repo.
- `fence sentinel init --github --yes` runs when workflow already exists.
- Sentinel base branch is missing.
- Sentinel changes are below threshold.
- Sentinel changes are above threshold with no decision.
- Sentinel changes are above threshold with a decision.
- JSON output remains valid for automation.
- Web UI works without internet.
- Markdown export stays in sync.
- Release binary prints `fence 0.1.0`.
- Team web sharing is documented for static hosting and private local serving.

## Can Be Built Without Product Input

- CLI hardening and launch smoke tests.
- Demo repo generation.
- Local search and `fence ask` preview.
- Markdown Sentinel output.
- More docs, examples, and launch assets.
- Release verification scripts.
- OpenSSF/SLSA-oriented repository hygiene.

## Needs Founder Input

- Product positioning and launch copy.
- Logo and brand direction.
- Pricing and packaging.
- Hosted product scope.
- Enterprise licensing.
- Domain, analytics, and deployment accounts.
- Customer discovery and launch channels.
