# Common Mistakes

## Treating Fence Like Paperwork

Bad:

```sh
fence log "Updated code"
```

Good:

```sh
fence log "Use Postgres jobs instead of Redis queue" \
  --rationale "One durable store reduces operational load" \
  --consequences "Throughput now depends on Postgres tuning"
```

## Forgetting To Commit Decision Files

Commit both:

```text
.fence/decisions/*.json
DECISIONS.md
```

Sentinel checks the structured files. Humans usually read `DECISIONS.md` or the web UI.

## Using Approval For Everything

Do not force every small decision through approval. Use `fence approve` for security, platform, compliance, and expensive architecture choices.

## Exposing `fence serve` Publicly

`fence serve` is writable in v1 and has no authentication. Keep it on localhost, a VPN, or a trusted internal network.

For public or broad sharing, use:

```sh
fence site
```

That export is read-only.

## Losing The ID

Use topic search:

```sh
fence pick redis
fence edit --search auth --reviewer @security
fence deprecate --search redis
```

If the search is ambiguous, Fence prints the candidate IDs.

## Setting Review Dates Too Soon

Review dates are reminders, not chores. Good defaults:

- Fast-moving product decision: 3 months
- Security or compliance decision: 6 months
- Stable architecture decision: 12 months

## Over-Tuning Sentinel Immediately

Start with warning mode if the threshold is noisy:

```toml
enforcement_level = "Warning"
```

Then move to blocking once the team agrees with the rules.
