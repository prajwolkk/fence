# Decision Lifecycle Guide

Fence separates “recorded” from “approved.”

## Statuses

- `proposed`: supported by the schema for future proposal workflows.
- `accepted`: recorded and active.
- `approved`: reviewed and signed off by someone using `fence approve`.
- `stale`: accepted or approved, but past `review_due`.
- `deprecated`: intentionally retired.
- `superseded`: replaced by a newer decision.

`stale` is a health label derived from the review date. The stored status remains `accepted` or `approved`.

## Create

```sh
fence log "Use Postgres for audit-safe persistence" \
  --title "Audit persistence" \
  --rationale "We need transactional writes and audit queries" \
  --consequences "Local development needs a database service" \
  --review-due 2026-12-31 \
  --owner @platform \
  --reviewer @security \
  --link https://github.com/acme/app/pull/42
```

## Find

```sh
fence list
fence pick postgres
fence ask "why postgres?"
fence show <id>
```

## Edit

```sh
fence edit <id> --title "Postgres audit persistence"
fence edit --search postgres --owner @data --reviewer @platform
```

If `--search` matches multiple decisions, Fence prints the matching IDs and stops.

## Review

```sh
fence review <id> --review-due 2027-06-01
fence review-due
```

Review means “this decision was checked and its next review date was refreshed.”

## Approve

```sh
fence approve <id>
fence approve --search audit
```

Approval records `approved_by` and `approved_at`. It is useful for teams with explicit ownership, but it should stay optional for solo developers and lightweight teams.

`approved_by` comes from `git config user.name` when available, with a system-user fallback.

## Deprecate

```sh
fence deprecate <id>
fence deprecate --search redis
```

Deprecation means “do not follow this decision anymore.”

## Supersede

```sh
fence log "Replace Redis queue with durable Postgres jobs" \
  --title "Durable job queue" \
  --rationale "One database reduces operational overhead" \
  --consequences "Queue throughput depends on Postgres tuning" \
  --replaces <old-id>
```

The old decision becomes `superseded` and links to the replacement.
