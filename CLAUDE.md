# Claude Code Instructions for Fence

Fence uses decision records as architectural memory. Before making meaningful changes, inspect existing decisions and keep intent synchronized with code.

## Workflow

```sh
fence ask "<area you plan to change>"
git diff --staged --name-only
fence agent-check --staged
```

When a change affects architecture, dependencies, storage, security, CI, release, or public CLI behavior, record the intent:

```sh
fence log "Short decision statement" \
  --title "Decision title" \
  --rationale "Why this is the right tradeoff" \
  --consequences "What future maintainers inherit" \
  --review-due 2026-12-31 \
  --owner @owner \
  --reviewer @reviewer
```

Stage `.fence/decisions` and `DECISIONS.md` with the code change.

## Do Not

- Do not ignore existing decisions.
- Do not silently bypass Sentinel.
- Do not edit generated `DECISIONS.md` by hand; run `fence export` or `fence log`.
- Do not claim launch readiness without `scripts/launch-smoke.sh` passing.
