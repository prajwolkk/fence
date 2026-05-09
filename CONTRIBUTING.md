# Contributing

Thanks for helping make Fence better.

## Development

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
```

Run tests with one thread because a few filesystem tests exercise the default Fence paths.

## Pull Requests

Good PRs usually include:

- A short description of the user-facing behavior.
- Tests for behavior changes.
- Updated docs when commands, config, or output changes.
- A Fence decision record when the change affects architecture, storage, Git behavior, or CI enforcement.

## Commit Hygiene

Keep changes focused. Avoid mixing formatting-only changes with behavior changes unless the formatter touched the file you already edited.

## Security

Please do not publish sensitive webhook URLs, custom command secrets, or private repository details in issues. For security-sensitive reports, open a minimal issue asking for a private contact path.
