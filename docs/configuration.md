# Configuration

Fence stores project settings in `fence.toml`.

```toml
project_name = "fence"
mode = "Solo"
log_path = ".fence/decisions"
auto_export = true
monitored_paths = ["Cargo.toml", "src"]
ignored_paths = ["target/**", ".git/**", "docs/assets/**"]
standalone_mode = false
safe_sync = true
sentinel_enabled = true
sentinel_platform = "GitHub"
enforcement_level = "Blocking"
threshold = 10

[scoring]
"Cargo.toml" = 10
"src/**/*.rs" = 2
```

## Important Fields

- `log_path`: where Fence counts structured decision records. The launch default is `.fence/decisions`.
- `auto_export`: when true, `fence log` refreshes `DECISIONS.md`.
- `monitored_paths`: paths that Sentinel watches when scoring code changes.
- `ignored_paths`: paths Sentinel ignores before scoring.
- `enforcement_level`: `Blocking` exits non-zero when a required decision is missing; `Warning` reports without blocking.
- `threshold`: score above which Sentinel requires a decision.
- `scoring`: weighted path patterns for deciding whether a code change is architectural enough to require a decision.

## Validation

Run:

```sh
fence sentinel validate
```

Fence reports invalid globs, empty monitored path entries, and scoring rules with zero points. Missing direct paths are warnings so teams can add config before a folder exists.

## Team Defaults

For team repos, track both `.fence/decisions` and `DECISIONS.md` in Git. That keeps reviewable intent next to the code change that needs it.
