# Web UI

Fence has two local readers:

```sh
fence browse
fence serve
fence open
```

`fence browse` opens the terminal browser. `fence serve` starts the web UI on `http://127.0.0.1:7878`, and `fence open` starts the same server and opens your browser.

## What the Web UI Shows

- Summary cards for total decisions, healthy decisions, needs attention, stale decisions, deprecated decisions, and superseded decisions.
- Search across IDs, titles, messages, rationale, consequences, authors, owners, reviewers, links, and tags.
- Status, category, and tag filters with counts.
- Newest-first and oldest-first sorting.
- Copy buttons for decision IDs.
- Expandable decision detail panels.
- Clickable `supersedes` and `superseded_by` links.
- Keyboard search focus with `/`.
- A local dark mode toggle.

The UI is fully self-contained and does not depend on a CDN, so `fence serve` works offline.

## Static Export

```sh
fence site
```

This writes `fence-site/index.html`, which can be uploaded as a static artifact or opened directly in a browser.
