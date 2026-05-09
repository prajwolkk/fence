# Demo Scenario

This is the first launch demo flow: a pull request changes `Cargo.toml` without a decision, and Sentinel blocks it.

## Setup

Fast path:

```sh
fence demo
cd fence-demo
fence sentinel check --base HEAD~1
```

Manual path:

```sh
fence init --team --yes
git checkout -b demo/change-runtime
```

## Make an Architectural Change

Edit `Cargo.toml`, for example by adding or changing a dependency:

```toml
[dependencies]
tokio = { version = "1", features = ["rt-multi-thread"] }
```

Commit without a Fence decision:

```sh
git add Cargo.toml
git commit -m "Change runtime dependency"
fence sentinel check --base origin/main
```

Expected failing output:

```text
Changed architectural files:
- Cargo.toml (+10, score 10)
- src/lib.rs (+2, score 2)

Required score: >10
Current score: 12
Missing: .fence/decisions change
```

## Fix the PR

```sh
fence log "Adopt Tokio runtime for async background jobs" \
  --title "Tokio runtime" \
  --rationale "Background workers need a maintained async runtime" \
  --consequences "Runtime upgrades become part of platform maintenance" \
  --review-due 2026-12-31 \
  --link https://github.com/acme/app/pull/42 \
  --owner @platform

git add .fence/decisions DECISIONS.md fence.toml
git commit -m "Record runtime decision"
fence sentinel check --base origin/main
```

Sentinel now passes because the architectural code change and the decision record land together.
