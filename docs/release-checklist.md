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
cargo run -- ask fence
cargo run -- demo --path /tmp/fence-demo --force
scripts/launch-smoke.sh
```

Verify:

- `README.md` quickstart is current.
- `docs/demo.md` still matches Sentinel output.
- `docs/examples/fence-sentinel.github-actions.yml` installs the release binary, not `cargo run`.
- `fence serve` renders decisions locally.
- `fence browse` opens the terminal browser.
- `fence sentinel check --json` returns automation-friendly JSON.
- `fence sentinel check --markdown` renders GitHub-summary-ready Markdown.
- `fence demo` creates a complete failing-then-fixed demo repo.
- `scripts/launch-smoke.sh` passes from a clean checkout.
- Team web sharing docs explain static hosting and private local serving.

## Publish

```sh
git tag v0.1.0
git push origin v0.1.0
```

The release workflow publishes binaries when the `v0.1.0` tag is pushed.

`v0.1.0` publishes Linux, macOS Intel, macOS Apple Silicon, and Windows x86_64 artifacts with SHA-256 checksums.

## After Release

Download a release binary and verify:

```sh
fence --version
```

Expected:

```text
fence 0.1.0
```
