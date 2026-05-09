# Schema and Migrations

Fence v1 stores decisions as JSON files in `.fence/decisions/`. These records are the compatibility boundary for future hosted, self-hosted, and AI features.

## V1 Compatibility Policy

- New fields must be optional or have safe defaults.
- Existing fields must not be renamed or removed without a migration.
- Unknown fields should be preserved by external tools where possible.
- `DECISIONS.md` is generated output; `.fence/decisions/*.json` is the source of truth.

## Current Decision Fields

- `id`
- `timestamp`
- `author`
- `branch`
- `message`
- `title`
- `rationale`
- `consequences`
- `category`
- `optional_tags`
- `status`
- `review_due`
- `supersedes`
- `superseded_by`
- `links`
- `owner`
- `reviewer`

## Future Migration Rules

- Add a schema version before the first incompatible change.
- Ship migrations as explicit commands, not silent rewrites.
- Keep old records readable for at least one major release.
- Document every schema change in `CHANGELOG.md`.

This policy keeps the open-source CLI trustworthy while leaving room for a future cloud or self-hosted product.
