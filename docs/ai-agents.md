# AI Agent Guardrails

Fence v1 helps AI coding agents stay aligned with repo decisions by making architectural memory local, structured, and checkable.

## Agent Contract

Agents should:

- Read `.fence/decisions/` before architectural work.
- Run `fence ask "<topic>"` before changing dependencies, storage, auth, CI, release, or public CLI behavior.
- Stage the change and run `fence agent-check --staged`.
- Record intent with `fence log` when the check requires a decision.

## Commands

```sh
fence ask "auth hashing"
fence agent-check --staged
fence agent-check --base origin/main
fence agent-check --staged --json
fence agent-check --staged --markdown
```

`agent-check` uses the same scoring rules as Sentinel. `--staged` checks the staged diff, which is useful before an AI agent hands work back to a human.

## Repo-Native Agent Files

Fence ships launch-ready instructions for common coding agents:

- `AGENTS.md` for Codex-style agents.
- `CLAUDE.md` for Claude Code.
- `.cursor/rules/fence.mdc` for Cursor.

These files tell agents to inspect decisions, ask Fence for context, run agent-check, and log decisions when architecture changes.

## What This Does Not Do Yet

Fence v1 does not semantically prove that a PR contradicts a prior decision. That is roadmap work for AI retrieval and contradiction checks. V1 creates the foundation: structured decisions, local search, staged checks, Sentinel CI, and explicit agent rules.
