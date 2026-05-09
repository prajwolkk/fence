# Roadmap

Fence v1 is the open-source local-first foundation. The CLI, structured decision store, Sentinel JSON, and static web export are intentionally designed so the project can later grow into hosted and self-hosted team products without changing the core data model.

## V1 Open Source

- Local CLI decision capture.
- Structured `.fence/decisions/*.json` records.
- `DECISIONS.md` export.
- Local terminal and web readers.
- Static web export.
- Sentinel CI enforcement.
- JSON and Markdown automation output.
- Demo repo and launch smoke test.

## Near-Term OSS

- More integration tests.
- Better schema migration tooling.
- Cargo publish and Homebrew tap.
- Signed releases and provenance.
- Optional ADR markdown import/export.
- Stronger PR examples and launch video assets.

## Future Hosted Product

- GitHub App installation.
- Multi-repo architectural memory.
- Hosted dashboard.
- PR comments and contradiction detection.
- Slack/Teams notifications.
- Team review reminders.
- Usage analytics for decision health.

## Future Enterprise / Self-Hosted

- Docker deployment.
- Postgres backend.
- SSO/OIDC/SAML.
- RBAC.
- Audit logs.
- Data retention controls.
- Air-gapped mode.
- License keys and support workflows.

## AI Direction

- Local `fence ask` backed by retrieval over decision records.
- `fence log --ai-propose` from staged diffs.
- Sentinel AI explanations with decision citations.
- PR contradiction checks against past decisions.

AI answers must cite decision IDs. No citation means no claim.
