# Release Checklist

Use this for the first `0.1.0` GitHub launch.

## Before Tagging

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1
cargo run -- doctor
cargo run -- check
cargo run -- --version
```

Verify:

- `README.md` quickstart is current.
- `docs/demo.md` still matches Sentinel output.
- `docs/examples/fence-sentinel.github-actions.yml` installs the release binary, not `cargo run`.
- `fence serve` renders decisions locally.
- `fence browse` opens the terminal browser.
- `fence sentinel check --json` returns automation-friendly JSON.

## Publish

```sh
git tag v0.1.0
git push origin v0.1.0
```

The release workflow publishes binaries when the `v0.1.0` tag is pushed.

## After Release

Download a release binary and verify:

```sh
fence --version
```

Expected:

```text
fence 0.1.0
```
