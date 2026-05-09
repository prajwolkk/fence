# Changelog

## 0.1.0 - Unreleased

First public launch candidate.

### Added

- Structured decision records in `.fence/decisions`.
- Markdown export to `DECISIONS.md`.
- Interactive terminal browser.
- Searchable static HTML site generation and offline local web server.
- Decision lifecycle support: accepted, stale, deprecated, superseded, and proposed.
- Rich decision metadata: title, rationale, consequences, links, owner, reviewer, and review due dates.
- Superseding decisions with `--replaces`.
- Legacy `decisions.log` migration.
- JSON output for list, show, stats, stale, and Sentinel checks.
- Non-interactive initialization with `fence init --yes`, `--team --yes`, and `--solo --yes`.
- Shell completion generation for zsh, bash, and fish.
- Sentinel checks for monitored architectural changes with scoring, ignored paths, validation, and explain output.
- GitHub/GitLab CI template generation.
- Setup diagnostics with `fence doctor`.
- GitHub CI and release workflows.
- Launch docs, demo scenario, web UI docs, Sentinel docs, security policy, code of conduct, and example repo.

### Known Limitations

- Existing ADR markdown imports are not implemented yet.
- Notification failures are best-effort and do not fail commands.
