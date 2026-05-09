# Fence Agent Rules

These rules are for AI coding agents working in this repository.

## Required Behavior

- Read `.fence/decisions/` before changing architecture, dependencies, storage, auth, CI, release, or public CLI behavior.
- Use `fence ask "<topic>"` to find relevant prior decisions before making a significant change.
- Run `fence agent-check --staged` before finalizing any staged architectural change.
- If Sentinel or agent-check says a decision is required, add one with `fence log "..."` and stage `.fence/decisions` plus `DECISIONS.md`.
- Do not bypass Sentinel with `[skip fence]` or `nolog` unless the human maintainer explicitly asks.

## Decision Hygiene

- Use clear decision titles.
- Include rationale and consequences for architectural changes.
- Add `--owner`, `--reviewer`, and `--review-due` when the decision affects team maintenance.
- Link relevant issues or PRs with `--link`.

## Launch Safety

Before claiming the repo is launch-ready, run:

```sh
scripts/launch-smoke.sh
```

If the smoke script fails, fix the product or docs instead of weakening the test.
