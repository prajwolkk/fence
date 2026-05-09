# Sentinel

Sentinel is Fence's CI and local enforcement layer. It compares the current branch against a base branch, scores changed files, and requires a `.fence/decisions` change when the score crosses the configured threshold.

## Local Commands

```sh
fence sentinel init
fence sentinel validate
fence sentinel check --base origin/main
fence sentinel check --base origin/main --json
fence sentinel explain --base origin/main
```

## Configuration

```toml
monitored_paths = ["Cargo.toml", "src"]
ignored_paths = ["target/**", ".git/**", "docs/assets/**"]
threshold = 10

[scoring]
"Cargo.toml" = 10
"src/**/*.rs" = 2
```

When scoring is configured, Sentinel sums the best matching score for each changed file that is not ignored. A decision is required when `Current score` is greater than `threshold`.

When scoring is empty, Sentinel falls back to `monitored_paths`.

## Output

```text
Changed architectural files:
- Cargo.toml (+10, score 10)
- src/lib.rs (+2, score 2)

Required score: >10
Current score: 12
Missing: .fence/decisions change
```

## GitHub Actions

Use the release binary in CI:

```yaml
- name: Install Fence
  run: |
    curl -fsSL https://github.com/prajwolkk/fence/releases/latest/download/fence-x86_64-unknown-linux-gnu.tar.gz | tar -xz
    sudo mv fence /usr/local/bin/fence
    fence --version

- name: Fence Sentinel Check
  run: fence sentinel check --base origin/${{ github.base_ref || 'main' }}
```

See [examples/fence-sentinel.github-actions.yml](examples/fence-sentinel.github-actions.yml).

## Bypasses

Sentinel recognizes these phrases in the latest commit message:

- `[skip fence]`
- `nolog`

Use bypasses sparingly and only when a change is not architectural.
