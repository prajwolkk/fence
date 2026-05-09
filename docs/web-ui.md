# Web UI

Fence has two local readers:

```sh
fence browse
fence serve
fence open
```

`fence browse` opens the terminal browser. `fence serve` starts the web UI on `http://127.0.0.1:7878`, and `fence open` starts the same server and opens your browser.

`fence serve` is writable in v1. `fence site` is read-only.

## What the Web UI Shows

- Summary cards for total decisions, healthy decisions, needs attention, stale decisions, deprecated decisions, and superseded decisions.
- Search across IDs, titles, messages, rationale, consequences, authors, owners, reviewers, links, and tags.
- Status, category, and tag filters with counts.
- Newest-first and oldest-first sorting.
- Copy buttons for decision IDs.
- Expandable decision detail panels.
- Clickable `supersedes` and `superseded_by` links.
- Edit title, tags, owner, reviewer, rationale, consequences, and review due date from the browser.
- Approve, review, deprecate, and supersede decisions from the browser.
- Keyboard search focus with `/`.
- A local dark mode toggle.

The UI is fully self-contained and does not depend on a CDN, so `fence serve` works offline.

## Static Export

```sh
fence site
```

This writes `fence-site/index.html`, which can be uploaded as a static artifact or opened directly in a browser.

For team sharing, publish `fence-site/` to GitHub Pages, Netlify, Vercel, S3, or an internal docs server. See [Team Web Sharing](team-web-sharing.md).

The static export intentionally does not include write buttons.

## Private Local Sharing

On a trusted internal network or VPN:

```sh
fence serve --host 0.0.0.0 --port 7878
```

Do not expose `fence serve` directly to the public internet in v1. It is a lightweight local control panel, not an authenticated multi-user web app.
