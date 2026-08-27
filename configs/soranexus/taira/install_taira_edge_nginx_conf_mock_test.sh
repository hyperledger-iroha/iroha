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

canonical_test_root() {
  local path
  path="$(mktemp -d)"
  (cd -P -- "$path" && pwd -P)
}

make_fake_repo() {
  local root="$1"
  local test_gid
  local test_uid

  test_uid="$(id -u)"
  test_gid="$(id -g)"
  mkdir -p \
    "${root}/configs/soranexus/taira" \
    "${root}/scripts" \
    "${root}/mockbin" \
    "${root}/state"
  sed \
    -e "s|^readonly NGINX_BIN=.*$|readonly NGINX_BIN=\"${root}/mockbin/nginx\"|" \
    -e "s|^readonly SYSTEM_OWNER_UID=.*$|readonly SYSTEM_OWNER_UID=${test_uid}|" \
    -e "s|^readonly SYSTEM_OWNER_GID=.*$|readonly SYSTEM_OWNER_GID=${test_gid}|" \
    "$SOURCE_SCRIPT" >"${root}/configs/soranexus/taira/install_taira_edge_nginx_conf.sh"
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

"""

invalid_block = ""
if os.environ.get("MOCK_RENDER_INVALID_NGINX", "0") == "1":
    invalid_block = "\nbroken_nginx_directive;\n"

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
        + invalid_block
    )
render_mode = os.environ.get("MOCK_RENDER_MODE")
if render_mode:
    os.chmod(output, int(render_mode, 8))
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

if [[ -z "$config_path" && $live_test -eq 1 && "${MOCK_MUTATE_TARGET_ON_LIVE_TEST:-0}" == "1" ]]; then
  printf '# mutated during live test\n' >>"${MOCK_INSTALLED_TARGET:?}"
fi

if [[ -z "$config_path" && $live_test -eq 1 && "${MOCK_MUTATE_TARGET_MODE_ON_LIVE_TEST:-0}" == "1" ]]; then
  chmod 0666 "${MOCK_INSTALLED_TARGET:?}"
fi

if [[ "$*" == "-s reload" && "${MOCK_NGINX_RELOAD_FAIL:-0}" == "1" ]]; then
  echo "nginx: reload failed" >&2
  exit 1
fi

if [[ "$*" == "-s reload" && "${MOCK_MUTATE_TARGET_ON_RELOAD:-0}" == "1" ]]; then
  printf '# mutated during reload\n' >>"${MOCK_INSTALLED_TARGET:?}"
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
  local arg
  local previous=""
  local target_conf=""
  shift

  for arg in "$@"; do
    if [[ "$previous" == "--target-conf" ]]; then
      target_conf="$arg"
      previous=""
      continue
    fi
    if [[ "$arg" == "--target-conf" ]]; then
      previous="$arg"
    fi
  done
  PATH="${root}/mockbin:${PATH}" \
    MOCK_STATE_DIR="${root}/state" \
    MOCK_INSTALLED_TARGET="$target_conf" \
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

file_mode() {
  local path="$1"
  stat -c '%a' "$path" 2>/dev/null || stat -f '%Lp' "$path"
}

file_owner() {
  local path="$1"
  stat -c '%u:%g' "$path" 2>/dev/null || stat -f '%u:%g' "$path"
}

test_dry_run_renders_and_checks_required_alias() {
  local root
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"

  run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788 \
    --require-alias solswap-indexer.sora \
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
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"

  if MOCK_RENDER_INVALID_NGINX=1 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788 \
    --require-alias solswap-indexer.sora \
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
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"

  run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788 \
    --require-alias solswap-indexer.sora \
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
}

test_install_normalizes_metadata_and_leaves_no_candidate() {
  local root
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"

  MOCK_RENDER_MODE=0666 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --install \
    >"${root}/state/stdout"

  [[ "$(file_mode "${root}/dist/taira-edge/taira.sora.org.conf")" == "666" ]] || {
    echo "test setup did not produce a writable rendered source" >&2
    exit 1
  }
  [[ "$(file_mode "${root}/nginx/servers/taira.sora.org.conf")" == "644" ]] || {
    echo "installed nginx config did not have mode 0644" >&2
    exit 1
  }
  [[ "$(file_owner "${root}/nginx/servers/taira.sora.org.conf")" == "$(id -u):$(id -g)" ]] || {
    echo "installed nginx config did not have the pinned owner/group" >&2
    exit 1
  }
  if find "${root}/nginx/servers" -maxdepth 1 -name '.taira-edge-install.*' -print -quit | grep -q .; then
    echo "atomic publication left an install candidate behind" >&2
    exit 1
  fi
}

test_install_validation_failure_restores_existing_target() {
  local root
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"
  printf '# previous live config\n' >"${root}/nginx/servers/taira.sora.org.conf"

  if MOCK_LIVE_NGINX_TEST_FAIL=1 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788 \
    --require-alias solswap-indexer.sora \
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
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"

  if MOCK_LIVE_NGINX_TEST_FAIL=1 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788 \
    --require-alias solswap-indexer.sora \
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

test_missing_required_alias_fails() {
  local root
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"

  if MOCK_RENDER_WITH_ALIAS=0 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --require-alias solswap-indexer.sora \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "missing required alias unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "rendered nginx config missing"
}

test_backup_confs_fail_before_install() {
  local root
  root="$(canonical_test_root)"
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

test_reload_failure_restores_existing_target() {
  local root
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"
  printf '# previous live config\n' >"${root}/nginx/servers/taira.sora.org.conf"

  if MOCK_NGINX_RELOAD_FAIL=1 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788 \
    --require-alias solswap-indexer.sora \
    --install \
    --reload \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "install unexpectedly passed after nginx reload failed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "reload failed after installing candidate"
  assert_contains "${root}/state/stderr" "restored previous nginx config"
  assert_contains "${root}/nginx/servers/taira.sora.org.conf" "# previous live config"
}

test_live_validation_detects_content_change_and_restores_target() {
  local root
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"
  printf '# previous live config\n' >"${root}/nginx/servers/taira.sora.org.conf"

  if MOCK_MUTATE_TARGET_ON_LIVE_TEST=1 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --install \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "install ignored a live-validation content change" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "changed during live nginx validation"
  assert_contains "${root}/state/stderr" "restored previous nginx config"
  assert_contains "${root}/nginx/servers/taira.sora.org.conf" "# previous live config"
}

test_live_validation_detects_mode_change_and_restores_target() {
  local root
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"
  printf '# previous live config\n' >"${root}/nginx/servers/taira.sora.org.conf"

  if MOCK_MUTATE_TARGET_MODE_ON_LIVE_TEST=1 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --install \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "install ignored a live-validation mode change" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "must be root-owned with mode 0644"
  assert_contains "${root}/state/stderr" "restored previous nginx config"
  assert_contains "${root}/nginx/servers/taira.sora.org.conf" "# previous live config"
  [[ "$(file_mode "${root}/nginx/servers/taira.sora.org.conf")" == "644" ]] || {
    echo "rollback did not restore the fixed nginx config mode" >&2
    exit 1
  }
}

test_reload_detects_content_change_and_restores_target() {
  local root
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"
  printf '# previous live config\n' >"${root}/nginx/servers/taira.sora.org.conf"

  if MOCK_MUTATE_TARGET_ON_RELOAD=1 run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --install \
    --reload \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "reload ignored an installed-config content change" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "changed during nginx reload"
  assert_contains "${root}/state/stderr" "restored previous nginx config"
  assert_contains "${root}/nginx/servers/taira.sora.org.conf" "# previous live config"
}

test_install_rejects_symlink_and_hardlinked_targets() {
  local root
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"
  printf '# foreign config\n' >"${root}/state/foreign.conf"
  ln -s "${root}/state/foreign.conf" "${root}/nginx/servers/taira.sora.org.conf"

  if run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --install \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "symlinked nginx target unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "direct regular file"
  assert_contains "${root}/state/foreign.conf" "# foreign config"

  rm "${root}/nginx/servers/taira.sora.org.conf"
  printf '# hardlinked config\n' >"${root}/nginx/servers/taira.sora.org.conf"
  ln "${root}/nginx/servers/taira.sora.org.conf" "${root}/state/hardlink.conf"
  if run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --install \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "hardlinked nginx target unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "exactly one hard link"
  assert_contains "${root}/state/hardlink.conf" "# hardlinked config"

  ln -s "${root}/nginx/servers" "${root}/nginx/linked-servers"
  if run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/linked-servers/second.conf" \
    --install \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "symlinked nginx target directory unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "include directory must be direct"
  [[ ! -e "${root}/nginx/servers/second.conf" ]] || {
    echo "symlinked target directory received an installed config" >&2
    exit 1
  }
}

test_validation_bypass_option_is_retired() {
  local root
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"

  if run_fake_script "$root" --skip-nginx-test \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "retired nginx validation bypass unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "unknown argument: --skip-nginx-test"
}

test_backup_conf_bypass_option_is_retired() {
  local root
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"

  if run_fake_script "$root" --allow-backup-confs \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "retired backup-conf bypass unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "unknown argument: --allow-backup-confs"
}

test_nginx_override_is_retired_and_environment_is_ignored() {
  local root
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"

  if run_fake_script "$root" --nginx-bin "${root}/state/foreign-nginx" \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "retired nginx executable override unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "unknown argument: --nginx-bin"

  NGINX_BIN="${root}/state/foreign-nginx" run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    >"${root}/state/stdout"
  assert_contains "${root}/state/nginx.calls" "-t"
}

test_rejects_mutable_nginx_binary() {
  local root
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  chmod 0777 "${root}/mockbin/nginx"

  if run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "mutable nginx executable unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "nginx executable must be root-owned and non-writable"
}

test_install_rejects_mutable_target_directory() {
  local root
  root="$(canonical_test_root)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  mkdir -p "${root}/nginx/servers"
  chmod 0777 "${root}/nginx/servers"

  if run_fake_script "$root" \
    --output "${root}/dist/taira-edge/taira.sora.org.conf" \
    --target-conf "${root}/nginx/servers/taira.sora.org.conf" \
    --install \
    >"${root}/state/stdout" 2>"${root}/state/stderr"; then
    echo "mutable nginx include directory unexpectedly passed" >&2
    exit 1
  fi
  assert_contains "${root}/state/stderr" "must be root-owned and non-writable"
}

test_dry_run_renders_and_checks_required_alias
test_dry_run_rejects_invalid_rendered_nginx
test_install_reload_copies_and_reloads
test_install_normalizes_metadata_and_leaves_no_candidate
test_install_validation_failure_restores_existing_target
test_install_validation_failure_removes_new_target
test_missing_required_alias_fails
test_backup_confs_fail_before_install
test_reload_failure_restores_existing_target
test_live_validation_detects_content_change_and_restores_target
test_live_validation_detects_mode_change_and_restores_target
test_reload_detects_content_change_and_restores_target
test_install_rejects_symlink_and_hardlinked_targets
test_validation_bypass_option_is_retired
test_backup_conf_bypass_option_is_retired
test_nginx_override_is_retired_and_environment_is_ignored
test_rejects_mutable_nginx_binary
test_install_rejects_mutable_target_directory

echo "install_taira_edge_nginx_conf mock tests passed"
