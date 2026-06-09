#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../../.." && pwd)"
ROSTER="${ROSTER:-${REPO_ROOT}/configs/soranexus/taira/validator_roster.local.toml}"
OUTPUT="${OUTPUT:-${REPO_ROOT}/dist/taira-edge/taira.sora.org.conf}"
TARGET_CONF="${TARGET_CONF:-}"
NGINX_BIN="${NGINX_BIN:-nginx}"
INSTALL=0
RELOAD=0
ALLOW_BACKUP_CONFS=0
SKIP_NGINX_TEST=0
ALIAS_ROUTES=()
REQUIRED_ALIASES=()
NGINX_TEST_DIRS=()
INSTALL_BACKUP_DIR=""
INSTALL_BACKUP_CONF=""
TARGET_CONF_EXISTED=0
INSTALL_ROLLBACK_NEEDED=0

cleanup_nginx_test_dirs() {
  local path
  for path in "${NGINX_TEST_DIRS[@]:-}"; do
    [[ -n "$path" && -e "$path" ]] && rm -rf "$path"
  done
}

cleanup_runtime_state() {
  local exit_code=$?

  if [[ ${INSTALL_ROLLBACK_NEEDED:-0} -eq 1 ]]; then
    rollback_installed_conf || true
  fi
  cleanup_nginx_test_dirs
  if [[ -n "${INSTALL_BACKUP_DIR:-}" && -e "$INSTALL_BACKUP_DIR" ]]; then
    rm -rf "$INSTALL_BACKUP_DIR" || true
  fi

  exit "$exit_code"
}

trap cleanup_runtime_state EXIT

usage() {
  cat <<'EOF'
Usage: install_taira_edge_nginx_conf.sh [--roster PATH] [--output PATH]
                                       [--target-conf PATH]
                                       [--soracloud-alias-route ALIAS=HOST:PORT]
                                       [--require-alias ALIAS]
                                       [--install] [--reload]
                                       [--nginx-bin PATH]
                                       [--allow-backup-confs]
                                       [--skip-nginx-test]

Render and optionally install/reload the shared Taira edge nginx config.

Default behavior is safe: render and validate the generated file, but do not
copy it into nginx and do not reload nginx. Use `--install --reload` only on
the edge host after reviewing the rendered config.

For the current Solswap indexer edge binding:
  bash configs/soranexus/taira/install_taira_edge_nginx_conf.sh \
    --roster configs/soranexus/taira/validator_roster.local.toml \
    --soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788 \
    --require-alias solswap-indexer.sora \
    --install --reload

On Linux, the default target is /etc/nginx/conf.d/taira.conf.
On Homebrew nginx hosts, when /opt/homebrew/etc/nginx/servers exists, the
default target is /opt/homebrew/etc/nginx/servers/taira.sora.org.conf.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --roster)
      [[ $# -ge 2 ]] || {
        echo "missing value for --roster" >&2
        exit 1
      }
      ROSTER="$2"
      shift 2
      ;;
    --output)
      [[ $# -ge 2 ]] || {
        echo "missing value for --output" >&2
        exit 1
      }
      OUTPUT="$2"
      shift 2
      ;;
    --target-conf)
      [[ $# -ge 2 ]] || {
        echo "missing value for --target-conf" >&2
        exit 1
      }
      TARGET_CONF="$2"
      shift 2
      ;;
    --soracloud-alias-route)
      [[ $# -ge 2 ]] || {
        echo "missing value for --soracloud-alias-route" >&2
        exit 1
      }
      ALIAS_ROUTES+=("$2")
      shift 2
      ;;
    --require-alias)
      [[ $# -ge 2 ]] || {
        echo "missing value for --require-alias" >&2
        exit 1
      }
      REQUIRED_ALIASES+=("$2")
      shift 2
      ;;
    --install)
      INSTALL=1
      shift
      ;;
    --reload)
      RELOAD=1
      INSTALL=1
      shift
      ;;
    --nginx-bin)
      [[ $# -ge 2 ]] || {
        echo "missing value for --nginx-bin" >&2
        exit 1
      }
      NGINX_BIN="$2"
      shift 2
      ;;
    --allow-backup-confs)
      ALLOW_BACKUP_CONFS=1
      shift
      ;;
    --skip-nginx-test)
      SKIP_NGINX_TEST=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$TARGET_CONF" ]]; then
  if [[ -d /opt/homebrew/etc/nginx/servers ]]; then
    TARGET_CONF="/opt/homebrew/etc/nginx/servers/taira.sora.org.conf"
  else
    TARGET_CONF="/etc/nginx/conf.d/taira.conf"
  fi
fi

if [[ ! -f "$ROSTER" ]]; then
  echo "roster not found: $ROSTER" >&2
  exit 1
fi

mkdir -p "$(dirname -- "$OUTPUT")"

render_args=(
  "${REPO_ROOT}/scripts/render_taira_edge_nginx_conf.py"
  --roster "$ROSTER"
  --output "$OUTPUT"
)
if ((${#ALIAS_ROUTES[@]} > 0)); then
  for route in "${ALIAS_ROUTES[@]}"; do
    render_args+=(--soracloud-alias-route "$route")
  done
fi

python3 "${render_args[@]}"

require_in_rendered_conf() {
  local pattern="$1"
  local message="$2"
  if ! grep -Eq "$pattern" "$OUTPUT"; then
    echo "rendered nginx config missing ${message}: $OUTPUT" >&2
    exit 1
  fi
}

alias_path_regex() {
  local alias="$1"
  local output=""
  local char
  local index
  for ((index = 0; index < ${#alias}; index += 1)); do
    char="${alias:index:1}"
    case "$char" in
      [a-zA-Z0-9_])
        output+="$char"
        ;;
      .)
        output+='\\?\.'
        ;;
      -)
        output+='\\?-'
        ;;
      *)
        output+="\\?\\${char}"
        ;;
    esac
  done
  printf '%s' "$output"
}

validate_rendered_nginx_conf() {
  local test_dir
  local test_conf
  local output_abs
  local rendered_include

  test_dir="$(mktemp -d "${TMPDIR:-/tmp}/taira-edge-nginx-test.XXXXXX")"
  NGINX_TEST_DIRS+=("$test_dir")
  test_conf="${test_dir}/nginx.conf"
  rendered_include="${test_dir}/rendered.conf"
  output_abs="$(cd -- "$(dirname -- "$OUTPUT")" && pwd)/$(basename -- "$OUTPUT")"

  ln -s "$output_abs" "$rendered_include"
  mkdir -p \
    "${test_dir}/client_body_temp" \
    "${test_dir}/fastcgi_temp" \
    "${test_dir}/logs" \
    "${test_dir}/proxy_temp" \
    "${test_dir}/scgi_temp" \
    "${test_dir}/uwsgi_temp"

  cat >"$test_conf" <<EOF
worker_processes 1;
error_log logs/error.log;
pid logs/nginx.pid;

events {
  worker_connections 1024;
}

http {
  client_body_temp_path client_body_temp;
  fastcgi_temp_path fastcgi_temp;
  proxy_temp_path proxy_temp;
  scgi_temp_path scgi_temp;
  uwsgi_temp_path uwsgi_temp;
  include ${rendered_include};
}
EOF

  "$NGINX_BIN" -t -c "$test_conf" -p "${test_dir}/"
}

target_needs_sudo_write() {
  local path="$1"
  local dir
  dir="$(dirname -- "$path")"

  if [[ -e "$path" ]]; then
    [[ ! -w "$path" ]]
  else
    [[ ! -w "$dir" ]]
  fi
}

copy_to_target_conf() {
  local source_path="$1"

  if target_needs_sudo_write "$TARGET_CONF"; then
    sudo cp "$source_path" "$TARGET_CONF"
  else
    cp "$source_path" "$TARGET_CONF"
  fi
}

remove_target_conf() {
  if [[ ! -w "$target_dir" ]]; then
    sudo rm -f "$TARGET_CONF"
  else
    rm -f "$TARGET_CONF"
  fi
}

backup_target_conf() {
  TARGET_CONF_EXISTED=0
  INSTALL_BACKUP_CONF=""

  if [[ ! -e "$TARGET_CONF" ]]; then
    return 0
  fi

  TARGET_CONF_EXISTED=1
  INSTALL_BACKUP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/taira-edge-nginx-install.XXXXXX")"
  INSTALL_BACKUP_CONF="${INSTALL_BACKUP_DIR}/previous.conf"
  if [[ ! -r "$TARGET_CONF" ]]; then
    sudo cp -p "$TARGET_CONF" "$INSTALL_BACKUP_CONF"
  else
    cp -p "$TARGET_CONF" "$INSTALL_BACKUP_CONF"
  fi
}

restore_target_conf() {
  if [[ ! -r "$INSTALL_BACKUP_CONF" ]] || target_needs_sudo_write "$TARGET_CONF"; then
    sudo cp -p "$INSTALL_BACKUP_CONF" "$TARGET_CONF"
  else
    cp -p "$INSTALL_BACKUP_CONF" "$TARGET_CONF"
  fi
}

rollback_installed_conf() {
  if [[ $TARGET_CONF_EXISTED -eq 1 ]]; then
    if restore_target_conf; then
      echo "restored previous nginx config: $TARGET_CONF" >&2
    else
      echo "failed to restore previous nginx config: $TARGET_CONF" >&2
      return 1
    fi
  else
    if remove_target_conf; then
      echo "removed failed nginx config: $TARGET_CONF" >&2
    else
      echo "failed to remove failed nginx config: $TARGET_CONF" >&2
      return 1
    fi
  fi
}

require_in_rendered_conf 'server_name[[:space:]]+mon\.taira\.sora\.net;' 'Mon apex server block'
require_in_rendered_conf 'server_name[[:space:]]+\*\.mon\.taira\.sora\.net[[:space:]]+~\^\.\+\\\.mon\\\.taira\\\.sora\\\.net\$;' 'Mon wildcard/regex fallback'
require_in_rendered_conf 'proxy_next_upstream[[:space:]].*non_idempotent' 'shared-edge retry policy'

if ((${#REQUIRED_ALIASES[@]} > 0)); then
  for alias in "${REQUIRED_ALIASES[@]}"; do
    escaped_alias="$(printf '%s' "$alias" | sed 's/[.[\*^$()+?{}|\\]/\\&/g')"
    pretty_host="${alias}.mon.taira.sora.net"
    escaped_pretty="$(printf '%s' "$pretty_host" | sed 's/[.[\*^$()+?{}|\\]/\\&/g')"
    escaped_alias_path="$(alias_path_regex "$alias")"
    require_in_rendered_conf "server_name[[:space:]]+${escaped_pretty};" "exact Mon host for ${alias}"
    require_in_rendered_conf "proxy_set_header[[:space:]]+Host[[:space:]]+${escaped_alias};" "Host header pin for ${alias}"
    require_in_rendered_conf "soradns/${escaped_alias_path}" "pinned /soradns route for ${alias}"
  done
fi

target_dir="$(dirname -- "$TARGET_CONF")"
if [[ $ALLOW_BACKUP_CONFS -ne 1 && -d "$target_dir" ]]; then
  backup_confs=()
  while IFS= read -r path; do
    backup_confs+=("$path")
  done < <(
    find "$target_dir" -maxdepth 1 -type f \( \
      -name '*.conf.bak' -o \
      -name '*.conf.backup' -o \
      -name '*.conf.old' -o \
      -name '*.conf.orig' -o \
      -name '*.conf.save' -o \
      -name '*.conf~' \
    \) -print | LC_ALL=C sort
  )
  if ((${#backup_confs[@]} > 0)); then
    {
      echo "refusing to continue while backup nginx conf files are in the include directory:"
      printf '  %s\n' "${backup_confs[@]}"
      echo "move them out of ${target_dir} or rerun with --allow-backup-confs after confirming nginx does not include them."
    } >&2
    exit 1
  fi
fi

if [[ $INSTALL -eq 1 && ! -d "$target_dir" ]]; then
  echo "target nginx include directory does not exist: $target_dir" >&2
  exit 1
fi

if [[ $SKIP_NGINX_TEST -ne 1 ]]; then
  validate_rendered_nginx_conf
fi

if [[ $INSTALL -eq 1 ]]; then
  backup_target_conf
  INSTALL_ROLLBACK_NEEDED=1
  if ! copy_to_target_conf "$OUTPUT"; then
    echo "failed to install nginx config candidate: $TARGET_CONF" >&2
    exit 1
  fi
  if [[ $SKIP_NGINX_TEST -ne 1 ]]; then
    if ! "$NGINX_BIN" -t; then
      echo "live nginx validation failed after installing candidate; rolling back: $TARGET_CONF" >&2
      exit 1
    fi
  fi
  INSTALL_ROLLBACK_NEEDED=0
  echo "installed nginx config: $TARGET_CONF"
else
  echo "rendered nginx config: $OUTPUT"
  echo "target nginx config: $TARGET_CONF"
  echo "dry run only; rerun with --install to copy and --reload to reload nginx"
fi

if [[ $RELOAD -eq 1 ]]; then
  "$NGINX_BIN" -s reload
  echo "nginx reloaded"
fi
