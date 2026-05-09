# Sentinel PR Guide

Sentinel turns “please remember to write an ADR” into a PR check.

## Official Reusable Action

Use the repo action:

```yaml
name: Fence Sentinel

on:
  pull_request:

jobs:
  fence:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      pull-requests: write
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: prajwolkk/fence@v0.1.0
        with:
          comment: "true"
```

The action installs the release binary, runs `fence sentinel check --markdown`, writes the GitHub step summary, and updates one PR comment.

If you later split the action into a dedicated marketplace repository, the same workflow can become:

```yaml
- uses: prajwolkk/fence-action@v0.1.0
```

## Generated Workflow

```sh
fence sentinel init --github --yes
```

This creates `.github/workflows/fence.yml`.

## Expected Failure

When code crosses the threshold without a decision:

```text
Changed architectural files:
- Cargo.toml (+10, score 10)
- src/lib.rs (+2, score 2)

Required score: >10
Current score: 12
Missing: .fence/decisions change
```

Fix it:

```sh
fence log "Why this architectural change is intentional" \
  --title "Runtime dependency change" \
  --owner @platform \
  --reviewer @security
git add .fence/decisions DECISIONS.md
git commit -m "Record architecture decision"
```

## Local Debugging

```sh
fence sentinel validate
fence sentinel explain --base origin/main
fence sentinel check --base origin/main --markdown
```

Use `enforcement_level = "Warning"` while tuning rules. Switch to `Blocking` when the team trusts the threshold.
