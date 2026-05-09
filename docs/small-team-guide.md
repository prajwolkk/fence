# Small Team Guide

Fence for small teams is a shared memory and PR safety net. The goal is not ceremony; the goal is that architecture decisions live beside the code they explain.

## Recommended Setup

```sh
fence init --team --yes
fence sentinel init --github --yes
```

Commit these files:

```text
fence.toml
DECISIONS.md
.fence/decisions/
.github/workflows/fence.yml
```

## Ownership Format

Use the same owner labels your team already understands:

```sh
--owner @platform
--reviewer @security
--owner @prajwol
--reviewer @backend
```

These are labels in v1. Fence does not call GitHub to validate handles yet.

You can set defaults in `fence.toml`:

```toml
default_owner = "@platform"
default_reviewer = "@security"
```

Then new decisions inherit those values unless a command overrides them.

## Team Commands

```sh
fence owners
fence review-due
fence team status
```

`fence owners` groups decisions by owner. `fence review-due` lists overdue reviews. `fence team status` shows unowned decisions, missing reviewers, overdue reviews, and owner/reviewer groupings.

## Approval Without Bureaucracy

Small teams can skip approval. A decision becomes `Accepted` when someone records it.

Use `fence approve <id>` when the decision crosses a real boundary:

- Security-sensitive change
- Expensive infrastructure choice
- Shared platform contract
- Compliance or audit requirement
- Decision owned by another team

Good small-team default: require approval socially in PR review, not as a blocking Fence rule in v1.

## PR Flow

1. Developer changes architecture-sensitive files.
2. Sentinel checks the diff.
3. If a decision is missing, CI fails with the changed files and score.
4. Developer runs `fence log "...why this is intentional..."`.
5. PR includes code plus `.fence/decisions/*.json` and `DECISIONS.md`.
6. Reviewer checks the decision, then optionally runs `fence approve <id>`.

## Web Sharing

Read-only sharing:

```sh
fence site
```

Publish `fence-site/` to GitHub Pages, Netlify, Vercel, S3, or an internal docs server.

Writable local control panel:

```sh
fence serve --host 0.0.0.0 --port 7878
```

Use this only on a trusted internal network or VPN. Fence v1 does not include authentication.
