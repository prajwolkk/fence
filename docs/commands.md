# Commands

## Setup

```sh
fence init
fence init --yes
fence init --team --yes
fence init --solo --yes
fence doctor
```

## Record Decisions

```sh
fence log "Use Postgres for audit-safe persistence" -c architecture -t database,audit
fence log "Use signed audit events" --title "Signed audit events" --review-due 2026-12-31 --link https://github.com/acme/app/pull/42 --owner @platform --reviewer @security
fence amend
fence edit <id>
fence edit <id> --title "Updated title" --review-due 2027-01-01 --owner @team
fence review <id> --review-due 2026-12-31
fence deprecate <id>
fence log "Replace the legacy queue with durable jobs" --replaces <id>
```

## Read Decisions

```sh
fence list
fence list --json
fence show <id>
fence show <id> --json
fence search database
fence ask "why did we choose postgres?"
fence ask "auth owner" --json
fence browse
fence stats
fence stats --json
fence stale
fence stale --json
```

## Generate Artifacts

```sh
fence export
fence site
fence serve
fence serve --port 9000
fence serve --open
fence open
fence badge
fence demo
fence demo --path /tmp/fence-demo --force
```

## Enforcement

```sh
fence sentinel init
fence sentinel init --github --yes
fence sentinel check
fence sentinel check --json
fence sentinel check --markdown
fence sentinel explain --base origin/main
fence sentinel explain --base origin/main --markdown
fence sentinel validate
fence check
```

## Shell Completions

```sh
fence completions zsh
fence completions bash
fence completions fish
```

## Migration

```sh
fence migrate
fence migrate --dry-run
fence migrate --from path/to/decisions.log
```
