#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_SCRIPT="${SCRIPT_DIR}/install_taira_edge_nginx_conf.sh"

cleanup_paths=()

cleanup() {
  local path
  for path in "${cleanup_paths[@]:-}"; do
    [[ -n "$path" && -e "$path" ]] && rm -rf "$path"
  done
}

trap cleanup EXIT

make_fake_repo() {
  local root="$1"
  mkdir -p \
    "${root}/configs/soranexus/taira" \
    "${root}/scripts" \
    "${root}/mockbin" \
    "${root}/state"
  cp "$SOURCE_SCRIPT" "${root}/configs/soranexus/taira/install_taira_edge_nginx_conf.sh"
  chmod 755 "${root}/configs/soranexus/taira/install_taira_edge_nginx_conf.sh"
  cat >"${root}/configs/soranexus/taira/validator_roster.local.toml" <<'EOF'
[[validators]]
slug = "taira-validator-1"
torii_public_address = "https://taira-validator-1.sora.org"
edge_torii_upstream = "127.0.0.1:29080"

[[validators]]
slug = "taira-validator-2"
torii_public_address = "https://taira-validator-2.sora.org"
edge_torii_upstream = "127.0.0.1:29081"

[[validators]]
slug = "taira-validator-3"
torii_public_address = "https://taira-validator-3.sora.org"
edge_torii_upstream = "127.0.0.1:29082"

[[validators]]
slug = "taira-validator-4"
torii_public_address = "https://taira-validator-4.sora.org"
edge_torii_upstream = "127.0.0.1:29083"
EOF
  printf '{"cash_handoff_capability":"cash_handoff_v1"}\n' \
    >"${root}/state/offline-identity.json"

  cat >"${root}/configs/soranexus/taira/check_mcp_rollout.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$@" >"${MOCK_STATE_DIR:?}/rollout-check.args"
if [[ "${MOCK_ROLLOUT_CHECK_FAIL:-0}" == "1" ]]; then
  echo "mock validator fleet admission failed" >&2
  exit 1
fi
SH
  chmod 755 "${root}/configs/soranexus/taira/check_mcp_rollout.sh"

  cat >"${root}/scripts/render_taira_edge_nginx_conf.py" <<'PY'
#!/usr/bin/env python3
import os
import sys

args = sys.argv[1:]
output = None
routes = []
for index, value in enumerate(args):
    if value == "--output" and index + 1 < len(args):
        output = args[index + 1]
    if value == "--soracloud-alias-route" and index + 1 < len(args):
        routes.append(args[index + 1])

if output is None:
    raise SystemExit("missing --output")

state_dir = os.environ.get("MOCK_STATE_DIR")
if state_dir:
    with open(os.path.join(state_dir, "renderer.args"), "w", encoding="utf-8") as handle:
        handle.write("\n".join(args))

include_alias = os.environ.get("MOCK_RENDER_WITH_ALIAS", "1") != "0"
alias_block = ""
if include_alias:
    alias_block = """
upstream soracloud_solswap_indexer_sora_upstream {
  server 127.0.0.1:8788;
}

server {
  listen 443 ssl;
  server_name solswap-indexer.sora.mon.taira.sora.net;
  auth_request /.taira-internal/offline-readiness;
  location = /.taira-internal/offline-readiness {
    internal;
    auth_request off;
    proxy_pass http://taira_validator_1_upstream/readyz;
  }
  location ^~ / {
    proxy_pass http://soracloud_solswap_indexer_sora_upstream;
    proxy_set_header Host solswap-indexer.sora;
  }
}

server {
  listen 443 ssl;
  server_name mon.taira.sora.net;
  location ~ ^/soradns/solswap\\-indexer\\.sora(?<soradns_rest>/.*)?$ {
    proxy_pass http://soracloud_solswap_indexer_sora_upstream$soradns_rest$is_args$args;
    proxy_set_header Host solswap-indexer.sora;
  }
}
"""

invalid_block = ""
if os.environ.get("MOCK_RENDER_INVALID_NGINX", "0") == "1":
    invalid_block = "\nbroken_nginx_directive;\n"

readiness_block = ""
if os.environ.get("MOCK_RENDER_WITH_READINESS_GATE", "1") != "0":
    readiness_block = """
  auth_request /.taira-internal/offline-readiness;
  location = /.taira-internal/offline-readiness {
    internal;
    auth_request off;
    proxy_pass http://taira_validator_1_upstream/readyz;
  }
"""

readiness_block += """
  location = /livez {
    auth_request off;
    proxy_pass http://taira_validator_1_upstream;
  }
"""

os.makedirs(os.path.dirname(output), exist_ok=True)
with open(output, "w", encoding="utf-8") as handle:
    handle.write(
        """
server {
  listen 443 ssl;
  server_name mon.taira.sora.net;
"""
        + readiness_block
        + """
  location / {
    proxy_pass http://taira_public_edge_upstream;
  }
}

server {
  listen 443 ssl;
  server_name *.mon.taira.sora.net ~^.+\\.mon\\.taira\\.sora\\.net$;
  location / {
    proxy_pass http://taira_public_edge_upstream;
    proxy_next_upstream error timeout http_502 http_503 http_504 invalid_header non_idempotent;
  }
}
"""
        + alias_block
        + invalid_block
    )
PY
  chmod 755 "${root}/scripts/render_taira_edge_nginx_conf.py"

  cat >"${root}/mockbin/nginx" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"${MOCK_STATE_DIR:?}/nginx.calls"

config_path=""
prefix_path=""
previous_arg=""
live_test=0
for arg in "$@"; do
  if [[ "$arg" == "-t" ]]; then
    live_test=1
  fi
  case "$previous_arg" in
    -c)
      config_path="$arg"
      previous_arg=""
      continue
      ;;
    -p)
      prefix_path="$arg"
      previous_arg=""
      continue
      ;;
  esac
  case "$arg" in
    -c|-p)
      previous_arg="$arg"
      ;;
    *)
      previous_arg=""
      ;;
  esac
done

if [[ -z "$config_path" && $live_test -eq 1 && "${MOCK_LIVE_NGINX_TEST_FAIL:-0}" == "1" ]]; then
  echo "nginx: configuration file test failed" >&2
  exit 1
fi
if [[ "${1:-}" == "-s" && "${2:-}" == "reload" && "${MOCK_NGINX_RELOAD_FAIL:-0}" == "1" ]]; then
  echo "nginx: reload failed" >&2
  exit 1
fi

if [[ -n "$config_path" ]]; then
  include_path="$(awk '/include .*rendered\.conf;/ { print $2; exit }' "$config_path")"
  include_path="${include_path%;}"
  include_path="${include_path%\"}"
  include_path="${include_path#\"}"
  if [[ -n "$include_path" ]]; then
    if [[ "$include_path" != /* ]]; then
      include_path="${prefix_path}${include_path}"
    fi
    printf '%s\n' "$include_path" >>"${MOCK_STATE_DIR}/nginx.rendered_includes"
    if [[ -L "$include_path" ]]; then
      readlink "$include_path" >>"${MOCK_STATE_DIR}/nginx.rendered_targets"
    else
      printf '%s\n' "$include_path" >>"${MOCK_STATE_DIR}/nginx.rendered_targets"
    fi
    if grep -Fq "broken_nginx_directive;" "$include_path"; then
      echo 'nginx: [emerg] unknown directive "broken_nginx_directive"' >&2
      exit 1
    fi
  fi
fi
SH
  chmod 755 "${root}/mockbin/nginx"
}

run_fake_script() {
  local root="$1"
  shift
  PATH="${root}/mockbin:${PATH}" \
    MOCK_STATE_DIR="${root}/state" \
    ROLLOUT_CHECK="${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
    OFFLINE_ASSET_DEFINITION_ID="6TEAJqbb8oEPmLncoNiMRbLEK6tw" \
    OFFLINE_EXPECTED_IDENTITY_PATH="${root}/state/offline-identity.json" \
    EXPECTED_TAIRA_GIT_SHA="0123456789abcdef0123456789abcdef01234567" \
    bash "${root}/configs/soranexus/taira/install_taira_edge_nginx_conf.sh" "$@"
}

assert_contains() {
  local file="$1"
  local needle="$2"
  if ! grep -Fq -- "$needle" "$file"; then
    echo "expected ${file} to contain: ${needle}" >&2
    echo "--- ${file} contents ---" >&2
    cat "$file" >&2
    echo "--- end ${file} contents ---" >&2
    exit 1
  fi
}

test_dry_run_renders_and_checks_required_alias() {
  local root
  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"

  run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788 \
    --require-alias solswap-indexer.sora \
    --nginx-bin nginx \
    >"${root}/state/stdout"

  assert_contains "${root}/state/stdout" "dry run only"
  assert_contains "${root}/state/renderer.args" "--soracloud-alias-route"
  assert_contains "${root}/state/renderer.args" "solswap-indexer.sora=127.0.0.1:8788"
  assert_contains "${root}/state/nginx.calls" "-t"
  assert_contains "${root}/state/nginx.calls" "-c"
  assert_contains "${root}/state/nginx.calls" "-p"
  assert_contains "${root}/state/nginx.rendered_targets" "${root}/dist/taira-edge/taira.sora.org.conf"
  [[ ! -e "${root}/nginx/servers/taira.sora.org.conf" ]] || {
    echo "dry run unexpectedly installed target config" >&2
    exit 1
  }
}

test_dry_run_rejects_invalid_rendered_nginx() {
  local root
  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"

  if MOCK_RENDER_INVALID_NGINX=1 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788 \
    --require-alias solswap-indexer.sora \
    --nginx-bin nginx \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "invalid rendered nginx unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "broken_nginx_directive"
  assert_contains "${root}/state/nginx.calls" "-c"
  assert_contains "${root}/state/nginx.rendered_targets" "${root}/dist/taira-edge/taira.sora.org.conf"
  [[ ! -e "${root}/nginx/servers/taira.sora.org.conf" ]] || {
    echo "dry run unexpectedly installed target config" >&2
    exit 1
  }
}

test_install_reload_copies_and_reloads() {
  local root
  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"

  run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788 \
    --require-alias solswap-indexer.sora \
    --nginx-bin nginx \
    --install \
    --reload \
    >"${root}/state/stdout"

  assert_contains "${root}/state/stdout" "installed nginx config"
  assert_contains "${root}/state/stdout" "nginx reloaded"
  assert_contains "${root}/nginx/servers/taira.sora.org.conf" "solswap-indexer.sora.mon.taira.sora.net"
  assert_contains "${root}/state/nginx.calls" "-t"
  assert_contains "${root}/state/nginx.calls" "-c"
  assert_contains "${root}/state/nginx.calls" "-s reload"
  assert_contains "${root}/state/nginx.rendered_targets" "${root}/dist/taira-edge/taira.sora.org.conf"
  [[ "$(grep -c '^--validator-root$' "${root}/state/rollout-check.args")" == "4" ]]
  assert_contains "${root}/state/rollout-check.args" "--require-all-validators"
  assert_contains "${root}/state/rollout-check.args" "--offline-asset-definition-id"
  assert_contains "${root}/state/rollout-check.args" "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
  assert_contains "${root}/state/rollout-check.args" "--offline-expected-identity"
  assert_contains "${root}/state/rollout-check.args" "--expected-git-sha"
  assert_contains "${root}/state/rollout-check.args" "0123456789abcdef0123456789abcdef01234567"
  assert_contains "${root}/state/rollout-check.args" "--skip-write-canary"
  assert_contains "${root}/state/rollout-check.args" "taira-validator-1=http://127.0.0.1:29080"
  assert_contains "${root}/state/rollout-check.args" "taira-validator-2=http://127.0.0.1:29081"
  assert_contains "${root}/state/rollout-check.args" "taira-validator-3=http://127.0.0.1:29082"
  assert_contains "${root}/state/rollout-check.args" "taira-validator-4=http://127.0.0.1:29083"
}

test_install_validation_failure_restores_existing_target() {
  local root
  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"
  printf '# previous live config\n' >"${root}/nginx/servers/taira.sora.org.conf"

  if MOCK_LIVE_NGINX_TEST_FAIL=1 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788 \
    --require-alias solswap-indexer.sora \
    --nginx-bin nginx \
    --install \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "install unexpectedly passed after live nginx validation failed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "rolling back"
  assert_contains "${root}/state/stderr" "restored previous nginx config"
  assert_contains "${root}/nginx/servers/taira.sora.org.conf" "# previous live config"
  assert_contains "${root}/state/nginx.calls" "-c"
}

test_install_validation_failure_removes_new_target() {
  local root
  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"

  if MOCK_LIVE_NGINX_TEST_FAIL=1 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788 \
    --require-alias solswap-indexer.sora \
    --nginx-bin nginx \
    --install \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "install unexpectedly passed after live nginx validation failed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "rolling back"
  assert_contains "${root}/state/stderr" "removed failed nginx config"
  [[ ! -e "${root}/nginx/servers/taira.sora.org.conf" ]] || {
    echo "failed live nginx validation left a new target config behind" >&2
    exit 1
  }
  assert_contains "${root}/state/nginx.calls" "-c"
}

test_rollout_admission_failure_prevents_install() {
  local root
  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"
  printf '# previous live config\n' >"${root}/nginx/servers/taira.sora.org.conf"

  if MOCK_ROLLOUT_CHECK_FAIL=1 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --nginx-bin nginx \
    --install \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "fleet admission failure unexpectedly installed nginx" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "mock validator fleet admission failed"
  assert_contains "${root}/nginx/servers/taira.sora.org.conf" "# previous live config"
}

test_reload_failure_restores_existing_target() {
  local root
  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"
  printf '# previous live config\n' >"${root}/nginx/servers/taira.sora.org.conf"

  if MOCK_NGINX_RELOAD_FAIL=1 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --nginx-bin nginx \
    --install \
    --reload \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "failed nginx reload unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "nginx reload failed; rolling back"
  assert_contains "${root}/state/stderr" "restored previous nginx config"
  assert_contains "${root}/nginx/servers/taira.sora.org.conf" "# previous live config"
}

test_skip_nginx_test_cannot_mutate() {
  local root
  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"

  if run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --install \
    --skip-nginx-test \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "mutating --skip-nginx-test unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "limited to non-mutating"
  [[ ! -e "${root}/nginx/servers/taira.sora.org.conf" ]]
}

test_missing_required_alias_fails() {
  local root
  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"

  if MOCK_RENDER_WITH_ALIAS=0 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --require-alias solswap-indexer.sora \
    --skip-nginx-test \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "missing required alias unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "rendered nginx config missing"
}

test_missing_readiness_admission_fails() {
  local root
  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"

  if MOCK_RENDER_WITH_READINESS_GATE=0 MOCK_RENDER_WITH_ALIAS=0 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --skip-nginx-test \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "missing readiness admission unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "mandatory /readyz admission gate"
}

test_backup_confs_fail_before_install() {
  local root
  root="$(mktemp -d)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"
  printf '# stale backup\n' >"${root}/nginx/servers/taira.sora.org.conf.bak"

  if run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788 \
    --require-alias solswap-indexer.sora \
    --install \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "backup conf guard unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "backup nginx conf files"
  [[ ! -e "${root}/nginx/servers/taira.sora.org.conf" ]] || {
    echo "backup guard installed target config despite refusal" >&2
    exit 1
  }
}

test_dry_run_renders_and_checks_required_alias
test_dry_run_rejects_invalid_rendered_nginx
test_install_reload_copies_and_reloads
test_install_validation_failure_restores_existing_target
test_install_validation_failure_removes_new_target
test_rollout_admission_failure_prevents_install
test_reload_failure_restores_existing_target
test_skip_nginx_test_cannot_mutate
test_missing_required_alias_fails
test_missing_readiness_admission_fails
test_backup_confs_fail_before_install

echo "install_taira_edge_nginx_conf mock tests passed"
