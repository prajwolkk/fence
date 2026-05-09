# Solo Developer Guide

Fence for solo developers is a fast memory system. Use it when future-you will ask, “why did I do this?”

## Start

```sh
fence init --solo --yes
fence log "Use SQLite for local-first cache" \
  --title "Local cache storage" \
  --rationale "The app should run without external services" \
  --consequences "Cache migrations stay local and lightweight" \
  --review-due 2026-12-31 \
  --owner @me
```

`owner` can be a GitHub handle, a team handle, or a plain label. Examples: `@prajwol`, `@platform`, `security`, `me`.

## Daily Loop

```sh
fence list
fence ask "why sqlite?"
fence show <id>
fence edit --search sqlite --title "SQLite local cache"
fence review <id> --review-due 2027-06-01
fence serve --open
```

Use `--search` when you remember the topic but not the ID. If more than one decision matches, Fence prints the candidate IDs.

## Keep It Light

For solo use, you normally do not need `fence approve`. Recording the decision as `Accepted` is enough. Use approval only if you want a personal checkpoint before a risky change.

## Agent Guardrail

Before handing work back from an AI coding session:

```sh
git add <changed-files>
fence agent-check --staged
```

If the staged diff crosses the Sentinel threshold, Fence asks for a decision record before the change leaves your machine.
