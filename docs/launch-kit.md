# Launch Kit

Use this when publishing the first GitHub release and posting about Fence.

## One-Line Positioning

Fence is architectural memory for your repo: it records decisions, serves a local UI, and blocks PRs when meaningful code changes land without recorded intent.

## GitHub Release Title

```text
Fence v0.1.0: architectural memory and PR guardrails for repos
```

## GitHub Release Notes

```text
Fence v0.1.0 is the first public launch.

Highlights:
- Record structured decisions in .fence/decisions
- Export DECISIONS.md
- Serve a local/offline decision UI
- Block architectural PRs with Sentinel
- Generate GitHub Actions workflows
- Run a full demo with fence demo
- Help AI coding agents with fence ask and fence agent-check

Try:
cargo install --path .
fence demo
```

## Hacker News / Reddit Title

```text
Show HN: Fence – architectural memory and PR guardrails for codebases
```

## Short Launch Post

```text
I built Fence, a local-first CLI that keeps architectural decisions close to code.

It records structured decisions, exports DECISIONS.md, serves a local web UI, and can block PRs when architectural files change without a decision.

The part I’m most excited about: it gives AI coding agents repo-native memory through .fence/decisions, fence ask, and fence agent-check.

Try the full demo:
fence demo
```

## Demo Flow

```sh
fence demo
cd fence-demo
fence sentinel check --base HEAD~1
fence log "Adopt Tokio runtime for async background jobs" \
  --title "Tokio runtime" \
  --rationale "Background workers need a maintained async runtime" \
  --consequences "Runtime upgrades become part of platform maintenance" \
  --review-due 2026-12-31 \
  --owner @platform \
  --reviewer @security
git add .fence/decisions DECISIONS.md
git commit -m "Record runtime decision"
fence sentinel check --base HEAD~2
fence serve --open
```

## Final Pre-Post Check

```sh
scripts/launch-smoke.sh
```
