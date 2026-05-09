# Team Web Sharing

Fence v1 is local-first. There is no hosted SaaS dashboard yet, but teams can still share the web UI safely.

## Static Sharing

Generate the static site:

```sh
fence site
```

Publish `fence-site/` anywhere that can serve static HTML:

- GitHub Pages
- Netlify
- Vercel
- S3 or compatible object storage
- An internal docs server

This is the recommended v1 team-sharing flow because it is simple, auditable, read-only, and does not require running a long-lived Fence service.

## Private Local Sharing

On a trusted internal network or VPN:

```sh
fence serve --host 0.0.0.0 --port 7878
```

Then share the machine URL with teammates. Do not expose this directly to the public internet in v1; `fence serve` is intentionally a lightweight writable local control panel, not an authenticated multi-user web app.

## What Comes Later

The future hosted/self-hosted product can build on the same structured decision files, JSON APIs, and Sentinel output:

- GitHub App sync
- Multi-repo dashboard
- SSO/RBAC
- Audit logs
- Team review reminders
- AI architectural memory
- Enterprise self-hosted deployment
