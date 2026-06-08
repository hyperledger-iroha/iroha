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
  printf '# fake roster\n' >"${root}/configs/soranexus/taira/validator_roster.local.toml"

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

os.makedirs(os.path.dirname(output), exist_ok=True)
with open(output, "w", encoding="utf-8") as handle:
    handle.write(
        """
server {
  listen 443 ssl;
  server_name mon.taira.sora.net;
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
    )
PY
  chmod 755 "${root}/scripts/render_taira_edge_nginx_conf.py"

  cat >"${root}/mockbin/nginx" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"${MOCK_STATE_DIR:?}/nginx.calls"
SH
  chmod 755 "${root}/mockbin/nginx"
}

run_fake_script() {
  local root="$1"
  shift
  PATH="${root}/mockbin:${PATH}" \
    MOCK_STATE_DIR="${root}/state" \
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
  assert_contains "${root}/state/nginx.calls" "-s reload"
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
    --skip-nginx-test \
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
test_install_reload_copies_and_reloads
test_missing_required_alias_fails
test_backup_confs_fail_before_install

echo "install_taira_edge_nginx_conf mock tests passed"
