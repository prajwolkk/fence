pub(crate) const CONFIG_FILE_NAME: &str = "fence.toml";
pub(crate) const DEFAULT_LOG_PATH: &str = ".fence/decisions";
pub(crate) const DECISION_DIR: &str = ".fence/decisions";
pub(crate) const DEFAULT_DECISIONS_MD_PATH: &str = "DECISIONS.md";
pub(crate) const DECISIONS_MD_HEADER: &str = "# 🛡️ Architectural Decision Records\n\n| Date | Author | Decision | Status |\n| :--- | :--- | :--- | :--- |\n";
pub(crate) const PRE_COMMIT_SNIPPET: &str = "#!/bin/sh\nif ! fence check; then\n  echo \"Fence: Commit blocked. Log or documentation is out of sync.\"\n  echo \"Run 'fence export' and stage the updated files.\"\n  exit 1\nfi\n";
pub(crate) const SITE_TEMPLATE: &str = include_str!("site_template.html");
pub(crate) const GITHUB_WORKFLOW_TEMPLATE: &str = "name: Fence Sentinel\n\non:\n  pull_request:\n  push:\n    branches: [main, master]\n\njobs:\n  fence:\n    runs-on: ubuntu-latest\n    permissions:\n      contents: read\n      pull-requests: write\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          fetch-depth: 0\n      - uses: prajwolkk/fence@v0.1.0\n        with:\n          comment: \"${{ github.event_name == 'pull_request' }}\"\n";
pub(crate) const GITLAB_CI_TEMPLATE: &str = "stages:\n  - fence\n\nfence_sentinel:\n  stage: fence\n  image: rust:latest\n  script:\n    - cargo run -- sentinel check --base origin/main || cargo run -- sentinel check --base origin/master\n";
