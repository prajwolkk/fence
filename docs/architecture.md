# Architecture

Fence is intentionally split into a small binary entrypoint, a focused CLI layer, and reusable library modules.

## Source Layout

```text
src/
  main.rs            Thin binary entrypoint.
  cli.rs             Command parsing and command orchestration.
  lib.rs             Library module root and public re-exports.
  constants.rs       Internal templates, paths, and generated workflow text.
  model.rs           Config, decision schema, status/category types, date normalization.
  repository.rs      Decision storage, markdown export, migration, sync checks, notifications.
  sentinel.rs        Diff scoring, config validation, Sentinel check/explain logic.
  serve.rs           Local writable HTTP UI used by `fence serve` and `fence open`.
  site_template.html Self-contained web UI template used by serve/static export.
  tui.rs             Terminal browser used by `fence browse`.
```

## Design Rules

- Keep `main.rs` and `lib.rs` small.
- Keep domain data structures in `model.rs`.
- Keep file-system persistence in `repository.rs`.
- Keep PR/change enforcement in `sentinel.rs`.
- Keep local web serving in `serve.rs`; do not mix HTTP routing into CLI parsing.
- Keep generated local outputs out of Git unless they are explicit examples.

## Public API

`src/lib.rs` re-exports the public API from the internal modules so existing CLI code, tests, and future integrations can continue to call `fence::...`.

Breaking schema changes should go through the migration policy in [Schema and Migrations](schema-and-migrations.md).
