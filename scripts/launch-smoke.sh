#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FENCE="$ROOT/target/debug/fence"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/fence-launch-smoke.XXXXXX")"
SERVER_PID=""

cleanup() {
  if [[ -n "${SERVER_PID}" ]]; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
  fi
  if [[ -f "${TMP_ROOT}/server.pid" ]]; then
    kill "$(cat "${TMP_ROOT}/server.pid")" >/dev/null 2>&1 || true
  fi
  rm -rf "${TMP_ROOT}"
}
trap cleanup EXIT

step() {
  printf '\n==> %s\n' "$*"
}

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

require_contains() {
  local value="$1"
  local needle="$2"
  [[ "${value}" == *"${needle}"* ]] || fail "expected output to contain: ${needle}"
}

require_not_contains() {
  local value="$1"
  local needle="$2"
  [[ "${value}" != *"${needle}"* ]] || fail "expected output not to contain: ${needle}"
}

json_assert() {
  local json="$1"
  local expr="$2"
  JSON_INPUT="${json}" EXPR="${expr}" python3 - <<'PY'
import json
import os
import sys

expr = os.environ["EXPR"]
data = json.loads(os.environ["JSON_INPUT"])
assert eval(expr, {"data": data, "len": len, "any": any, "all": all}), expr
PY
}

first_decision_id() {
  local json="$1"
  JSON_INPUT="${json}" python3 - <<'PY'
import json
import os
import sys

data = json.loads(os.environ["JSON_INPUT"])
assert data, "expected at least one decision"
print(data[0]["id"])
PY
}

json_field() {
  local json="$1"
  local expr="$2"
  JSON_INPUT="${json}" EXPR="${expr}" python3 - <<'PY'
import json
import os

data = json.loads(os.environ["JSON_INPUT"])
print(eval(os.environ["EXPR"], {"data": data}))
PY
}

git_init_repo() {
  local repo="$1"
  mkdir -p "${repo}/src"
  git -C "${repo}" init -b main >/dev/null 2>&1 || git -C "${repo}" init >/dev/null 2>&1
  git -C "${repo}" config user.name "Fence Smoke"
  git -C "${repo}" config user.email "smoke@fence.local"
  cat >"${repo}/Cargo.toml" <<'EOF'
[package]
name = "fence-smoke"
version = "0.1.0"
edition = "2024"

[dependencies]
EOF
  cat >"${repo}/src/lib.rs" <<'EOF'
pub fn runtime_name() -> &'static str {
    "std"
}
EOF
  git -C "${repo}" add .
  git -C "${repo}" commit -m "Initial smoke repo" >/dev/null
}

free_port() {
  python3 - <<'PY'
import socket

s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
}

curl_status() {
  curl -s -o /tmp/fence-smoke-response.txt -w "%{http_code}" "$1"
}

step "checking required local tools"
require_cmd cargo
require_cmd curl
require_cmd git
require_cmd python3

step "checking repository starts clean"
if [[ "${FENCE_SMOKE_ALLOW_DIRTY:-0}" != "1" ]]; then
  [[ -z "$(git -C "${ROOT}" status --short)" ]] || fail "repository is not clean before launch smoke"
fi
[[ -f "${ROOT}/AGENTS.md" ]] || fail "AGENTS.md is missing"
[[ -f "${ROOT}/CLAUDE.md" ]] || fail "CLAUDE.md is missing"
[[ -f "${ROOT}/.cursor/rules/fence.mdc" ]] || fail "Cursor rules are missing"

step "running Rust quality gates"
(cd "${ROOT}" && cargo fmt --check)
(cd "${ROOT}" && cargo clippy --all-targets -- -D warnings)
(cd "${ROOT}" && cargo test -- --test-threads=1)
(cd "${ROOT}" && cargo build)
(cd "${ROOT}" && cargo build --release)
[[ "$("${ROOT}/target/release/fence" --version)" == "fence 0.1.0" ]] || fail "release binary version mismatch"
"${FENCE}" completions bash | head -n 1 >/dev/null
"${FENCE}" completions fish | head -n 1 >/dev/null
"${FENCE}" completions zsh | head -n 1 >/dev/null

step "testing solo flow without Git"
NO_GIT="${TMP_ROOT}/solo-no-git"
mkdir -p "${NO_GIT}"
(
  cd "${NO_GIT}"
  "${FENCE}" init --yes
  "${FENCE}" log "Use SQLite for local cache" \
    --title "Local cache" \
    --rationale "Solo mode should work without network services" \
    --consequences "Cache migrations stay local" \
    --review-due 2026-12-31 \
    --link https://example.com/cache \
    --owner @solo
  list_json="$("${FENCE}" list --json)"
  json_assert "${list_json}" "len(data) == 1 and data[0]['title'] == 'Local cache'"
  id="$(first_decision_id "${list_json}")"
  require_contains "$("${FENCE}" show "${id}")" "Local cache"
  json_assert "$("${FENCE}" show "${id}" --json)" "data['id'] == '${id}'"
  json_assert "$("${FENCE}" stats --json)" "data['total'] == 1 and data['healthy'] == 1"
  require_contains "$("${FENCE}" ask cache)" "${id}"
  require_contains "$("${FENCE}" search cache)" "${id}"
  "${FENCE}" check
  "${FENCE}" site
  [[ -f fence-site/index.html ]] || fail "static site was not generated"
  require_contains "$(cat fence-site/index.html)" "Fence Decisions"
  require_contains "$(cat fence-site/index.html)" "const writable = false"

  port="$(free_port)"
  "${FENCE}" serve --port "${port}" >server.log 2>&1 &
  SERVER_PID="$!"
  echo "${SERVER_PID}" >"${TMP_ROOT}/server.pid"
  sleep 1
  require_contains "$(curl -s "http://127.0.0.1:${port}/")" "Fence Decisions"
  require_contains "$(curl -s "http://127.0.0.1:${port}/")" "const writable = true"
  json_assert "$(curl -s "http://127.0.0.1:${port}/api/decisions")" "len(data) == 1"
  json_assert "$(curl -s "http://127.0.0.1:${port}/api/stats")" "data['total'] == 1"
  require_contains "$(curl -s "http://127.0.0.1:${port}/health")" '"status":"ok"'
  [[ "$(curl_status "http://127.0.0.1:${port}/missing")" == "404" ]] || fail "missing route did not return 404"
  edit_response="$(curl -s -X POST "http://127.0.0.1:${port}/api/decisions/${id}/edit" -H 'Content-Type: application/json' -d '{"title":"Local cache v2","optional_tags":["cache","sqlite"],"owner":"@solo","reviewer":"@review","rationale":"Still local first","consequences":"Keep migrations small","review_due":"2027-01-01"}')"
  json_assert "${edit_response}" "data['title'] == 'Local cache v2' and data['reviewer'] == '@review'"
  review_response="$(curl -s -X POST "http://127.0.0.1:${port}/api/decisions/${id}/review" -H 'Content-Type: application/json' -d '{"review_due":"2027-06-01"}')"
  json_assert "${review_response}" "data['review_due'].startswith('2027-06-01')"
  approve_response="$(curl -s -X POST "http://127.0.0.1:${port}/api/decisions/${id}/approve" -H 'Content-Type: application/json' -d '{}')"
  json_assert "${approve_response}" "data['status'] == 'approved' and data['approved_by']"
  replace_response="$(curl -s -X POST "http://127.0.0.1:${port}/api/decisions/${id}/replace" -H 'Content-Type: application/json' -d '{"message":"Replace SQLite cache with local file snapshots","title":"Snapshot cache","optional_tags":["cache"],"review_due":"2027-12-31"}')"
  replacement_id="$(json_field "${replace_response}" "data['id']")"
  json_assert "${replace_response}" "data['supersedes'] == '${id}'"
  deprecate_response="$(curl -s -X POST "http://127.0.0.1:${port}/api/decisions/${replacement_id}/deprecate" -H 'Content-Type: application/json' -d '{}')"
  json_assert "${deprecate_response}" "data['ok'] is True"
  if command -v google-chrome >/dev/null 2>&1; then
    require_contains "$(google-chrome --headless --disable-gpu --no-sandbox --dump-dom "http://127.0.0.1:${port}/" 2>/dev/null)" "Fence Decisions"
  fi
  kill "${SERVER_PID}" >/dev/null 2>&1 || true
  rm -f "${TMP_ROOT}/server.pid"
  SERVER_PID=""
)

step "testing solo flow with Git and non-interactive edit"
SOLO_GIT="${TMP_ROOT}/solo-git"
mkdir -p "${SOLO_GIT}"
git_init_repo "${SOLO_GIT}"
(
  cd "${SOLO_GIT}"
  "${FENCE}" init --solo --yes
  "${FENCE}" log "Adopt Argon2id for password hashing" \
    -c security \
    -t auth,passwords \
    --title "Password hashing" \
    --rationale "Password storage should resist GPU cracking" \
    --consequences "Auth workers need memory budget" \
    --review-due 2020-01-01 \
    --link https://github.com/acme/app/pull/7 \
    --owner @auth \
    --reviewer @security
  list_json="$("${FENCE}" list --json)"
  id="$(first_decision_id "${list_json}")"
  json_assert "$("${FENCE}" stale --json)" "len(data) == 1 and data[0]['id'] == '${id}'"
  require_contains "$("${FENCE}" pick auth)" "${id}"
  require_contains "$("${FENCE}" review-due)" "${id}"
  require_contains "$("${FENCE}" owners)" "@auth"
  require_contains "$("${FENCE}" team status)" "Overdue reviews"
  "${FENCE}" edit --search Argon2id \
    --title "Argon2id password hashing" \
    --message "Adopt Argon2id for password hashing and credential hardening" \
    --category security \
    --tags auth,passwords,crypto \
    --review-due 2027-01-01 \
    --owner @identity \
    --reviewer @security \
    --link https://github.com/acme/app/pull/8
  require_contains "$("${FENCE}" show "${id}")" "Argon2id password hashing"
  "${FENCE}" review "${id}" --review-due 2027-06-01
  "${FENCE}" approve --search Argon2id
  require_contains "$("${FENCE}" show "${id}")" "Approved"
  json_assert "$("${FENCE}" stale --json)" "len(data) == 0"
  "${FENCE}" deprecate --search Argon2id
  require_contains "$("${FENCE}" show "${id}")" "Deprecated"
  if "${FENCE}" show does-not-exist >/tmp/fence-smoke-missing.txt 2>&1; then
    fail "show should fail for a missing decision"
  fi
  "${FENCE}" check
)

step "testing team and Sentinel flows"
TEAM="${TMP_ROOT}/team-sentinel"
mkdir -p "${TEAM}"
git_init_repo "${TEAM}"
(
  cd "${TEAM}"
  "${FENCE}" init --team --yes
  "${FENCE}" sentinel init --github --yes
  [[ -f .github/workflows/fence.yml ]] || fail "GitHub Sentinel workflow missing"
  require_contains "$(cat .github/workflows/fence.yml)" "prajwolkk/fence@v0.1.0"

  cat >src/lib.rs <<'EOF'
pub fn runtime_name() -> &'static str {
    "std"
}

pub fn low_risk_change() -> bool {
    true
}
EOF
  git add src/lib.rs
  git commit -m "Low risk source change" >/dev/null
  below="$("${FENCE}" sentinel check --base HEAD~1)"
  require_contains "${below}" "Decision: not required"

  cat >Cargo.toml <<'EOF'
[package]
name = "fence-smoke"
version = "0.1.0"
edition = "2024"

[dependencies]
tokio = { version = "1", features = ["rt-multi-thread"] }
EOF
  cat >src/lib.rs <<'EOF'
pub fn runtime_name() -> &'static str {
    "tokio"
}

pub fn low_risk_change() -> bool {
    true
}

pub fn worker_threads() -> usize {
    4
}
EOF
  git add Cargo.toml src/lib.rs
  if "${FENCE}" agent-check --staged >/tmp/fence-smoke-agent-staged.txt 2>&1; then
    fail "agent-check --staged should block staged architectural changes without a decision"
  fi
  require_contains "$(cat /tmp/fence-smoke-agent-staged.txt)" "Agent preflight blocked"
  git commit -m "Change runtime dependency" >/dev/null
  if "${FENCE}" sentinel check --base HEAD~1 >/tmp/fence-smoke-sentinel.txt 2>&1; then
    fail "Sentinel should block an architectural change without a decision"
  fi
  missing="$(cat /tmp/fence-smoke-sentinel.txt)"
  require_contains "${missing}" "Missing: .fence/decisions change"
  if "${FENCE}" sentinel check --base HEAD~1 --json >/tmp/fence-smoke-sentinel.json 2>&1; then
    fail "Sentinel JSON should fail when blocking"
  fi
  json_assert "$(cat /tmp/fence-smoke-sentinel.json)" "data['missing_decision'] is True and data['score'] > data['threshold']"
  if "${FENCE}" sentinel check --base HEAD~1 --markdown >/tmp/fence-smoke-sentinel.md 2>&1; then
    fail "Sentinel Markdown should fail when blocking"
  fi
  require_contains "$(cat /tmp/fence-smoke-sentinel.md)" "### Fence Sentinel"

  "${FENCE}" log "Adopt Tokio runtime for async background jobs" \
    --title "Tokio runtime" \
    --rationale "Background workers need a maintained async runtime" \
    --consequences "Runtime upgrades become part of platform maintenance" \
    --review-due 2026-12-31 \
    --owner @platform \
    --reviewer @security
  git add .fence/decisions DECISIONS.md
  require_contains "$("${FENCE}" agent-check --staged)" "Agent preflight passed"
  git commit -m "Record runtime decision" >/dev/null
  require_contains "$("${FENCE}" sentinel check --base HEAD~2)" "Decision: found"
  require_contains "$("${FENCE}" agent-check --base HEAD~2)" "Agent preflight passed"
  json_assert "$("${FENCE}" sentinel check --base HEAD~2 --json)" "data['decision_found'] is True"
  require_contains "$("${FENCE}" sentinel check --base no-such-ref)" "No monitored changes detected"
)

step "testing generated demo flow"
DEMO="${TMP_ROOT}/generated-demo"
"${FENCE}" demo --path "${DEMO}" --force >/tmp/fence-smoke-demo.txt
require_contains "$(cat /tmp/fence-smoke-demo.txt)" "Fence demo repo created"
(
  cd "${DEMO}"
  if "${FENCE}" sentinel check --base HEAD~1 >/tmp/fence-smoke-demo-fail.txt 2>&1; then
    fail "demo Sentinel should fail before a decision is logged"
  fi
  require_contains "$(cat /tmp/fence-smoke-demo-fail.txt)" "Current score: 12"
  "${FENCE}" log "Adopt Tokio runtime for async background jobs" \
    --title "Tokio runtime" \
    --rationale "Background workers need a maintained async runtime" \
    --consequences "Runtime upgrades become part of platform maintenance" \
    --review-due 2026-12-31 \
    --owner @platform \
    --reviewer @security
  git add .fence/decisions DECISIONS.md
  git commit -m "Record runtime decision" >/dev/null
  require_contains "$("${FENCE}" sentinel check --base HEAD~2)" "Decision: found"
)

step "testing config validation failure"
INVALID="${TMP_ROOT}/invalid-config"
mkdir -p "${INVALID}"
git_init_repo "${INVALID}"
(
  cd "${INVALID}"
  "${FENCE}" init --team --yes
  cat >fence.toml <<'EOF'
project_name = "invalid"
mode = "Team"
log_path = ".fence/decisions"
auto_export = true
monitored_paths = ["["]
ignored_paths = [""]
threshold = 10

[scoring]
"src/**/*.rs" = 0
EOF
  if "${FENCE}" sentinel validate >/tmp/fence-smoke-invalid.txt 2>&1; then
    fail "invalid config should fail validation"
  fi
  require_contains "$(cat /tmp/fence-smoke-invalid.txt)" "Config validation: failed"
)

step "checking repository ends clean"
if [[ "${FENCE_SMOKE_ALLOW_DIRTY:-0}" != "1" ]]; then
  [[ -z "$(git -C "${ROOT}" status --short)" ]] || fail "repository is not clean after launch smoke"
fi

step "launch smoke passed"
