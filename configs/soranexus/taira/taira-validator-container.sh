#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: taira-validator-container.sh [--env-file <path>] <command>

Commands:
  config   Print the resolved `docker run` command.
  up       Pull the image when missing, replace any existing container, and start it.
  down     Remove the container if it exists.
  restart  Recreate the container.
  pull     Pull the configured image tag.
  status   Show `docker ps` status for the configured container.
  logs     Show container logs.
EOF
}

env_file=""

while (($#)); do
    case "$1" in
        --env-file)
            env_file="${2:-}"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            break
            ;;
    esac
done

command_name="${1:-}"
if [[ -z "$command_name" ]]; then
    usage >&2
    exit 1
fi
shift || true

if [[ -z "$env_file" ]]; then
    env_file="/etc/default/taira-validator-container.compose.env"
fi

if [[ -f "$env_file" ]]; then
    # The env file is an operator-managed shell fragment containing only
    # variable assignments and comments.
    set -a
    # shellcheck disable=SC1090
    . "$env_file"
    set +a
fi

TAIRA_CONTAINER_NAME="${TAIRA_CONTAINER_NAME:-taira-validator-1}"
TAIRA_IMAGE="${TAIRA_IMAGE:-hyperledger/iroha:taira-latest}"
TAIRA_CONFIG_PATH="${TAIRA_CONFIG_PATH:-/etc/iroha/taira-validator-1/config.toml}"
TAIRA_STORAGE_PATH="${TAIRA_STORAGE_PATH:-/var/lib/iroha/taira-validator-1}"
TAIRA_P2P_PORT="${TAIRA_P2P_PORT:-1337}"
TAIRA_TORII_PORT="${TAIRA_TORII_PORT:-18080}"
TAIRA_RUST_LOG="${TAIRA_RUST_LOG:-info}"
TAIRA_GENESIS_PATH="${TAIRA_GENESIS_PATH:-}"
TAIRA_SIGNED_GENESIS_PATH="${TAIRA_SIGNED_GENESIS_PATH:-}"
TAIRA_SORAFS_SITE_BINDINGS_PATH="${TAIRA_SORAFS_SITE_BINDINGS_PATH:-}"
TAIRA_DOCKER_NETWORK="${TAIRA_DOCKER_NETWORK:-}"

require_file() {
    local path="$1"
    local label="$2"
    if [[ ! -f "$path" ]]; then
        printf 'missing %s at %s\n' "$label" "$path" >&2
        exit 1
    fi
}

require_directory() {
    local path="$1"
    local label="$2"
    if [[ ! -d "$path" ]]; then
        printf 'missing %s at %s\n' "$label" "$path" >&2
        exit 1
    fi
}

docker_cmd=(docker)

build_run_args() {
    require_file "$TAIRA_CONFIG_PATH" "Taira config"
    require_directory "$TAIRA_STORAGE_PATH" "Taira storage directory"

    docker_run_args=(
        run -d
        --name "$TAIRA_CONTAINER_NAME"
        --restart unless-stopped
        --init
        -e "RUST_LOG=$TAIRA_RUST_LOG"
        -p "${TAIRA_P2P_PORT}:1337"
        -p "${TAIRA_TORII_PORT}:8080"
        -v "${TAIRA_CONFIG_PATH}:/config/config.toml:ro"
        -v "${TAIRA_STORAGE_PATH}:/storage"
    )

    if [[ -n "$TAIRA_DOCKER_NETWORK" ]]; then
        docker_run_args+=(
            --network "$TAIRA_DOCKER_NETWORK"
        )
    fi

    if [[ -n "$TAIRA_GENESIS_PATH" ]]; then
        require_file "$TAIRA_GENESIS_PATH" "Taira genesis"
        docker_run_args+=(
            -e "IROHA_TAIRA_GENESIS=/config/genesis.json"
            -v "${TAIRA_GENESIS_PATH}:/config/genesis.json:ro"
        )
    fi

    if [[ -n "$TAIRA_SIGNED_GENESIS_PATH" ]]; then
        require_file "$TAIRA_SIGNED_GENESIS_PATH" "Taira signed genesis"
        docker_run_args+=(
            -e "IROHA_TAIRA_SIGNED_GENESIS=/config/genesis.signed.nrt"
            -v "${TAIRA_SIGNED_GENESIS_PATH}:/config/genesis.signed.nrt:ro"
        )
    fi

    if [[ -n "$TAIRA_SORAFS_SITE_BINDINGS_PATH" ]]; then
        require_file "$TAIRA_SORAFS_SITE_BINDINGS_PATH" "Taira SoraFS site bindings"
        docker_run_args+=(
            -e "IROHA_SORAFS_SITE_BINDINGS_FILE=/config/sorafs_sites.json"
            -v "${TAIRA_SORAFS_SITE_BINDINGS_PATH}:/config/sorafs_sites.json:ro"
        )
    fi

    docker_run_args+=("$TAIRA_IMAGE")
}

container_exists() {
    "${docker_cmd[@]}" container inspect "$TAIRA_CONTAINER_NAME" >/dev/null 2>&1
}

image_exists() {
    "${docker_cmd[@]}" image inspect "$TAIRA_IMAGE" >/dev/null 2>&1
}

do_pull() {
    "${docker_cmd[@]}" pull "$TAIRA_IMAGE"
}

do_down() {
    if container_exists; then
        "${docker_cmd[@]}" rm -f "$TAIRA_CONTAINER_NAME" >/dev/null
    fi
}

do_up() {
    build_run_args

    if ! image_exists; then
        do_pull
    fi

    do_down
    "${docker_cmd[@]}" "${docker_run_args[@]}"
}

case "$command_name" in
    config)
        build_run_args
        printf '%q ' "${docker_cmd[@]}" "${docker_run_args[@]}"
        printf '\n'
        ;;
    up)
        do_up
        ;;
    down)
        do_down
        ;;
    restart)
        do_up
        ;;
    pull)
        do_pull
        ;;
    status)
        "${docker_cmd[@]}" ps --filter "name=^/${TAIRA_CONTAINER_NAME}$"
        ;;
    logs)
        "${docker_cmd[@]}" logs "$TAIRA_CONTAINER_NAME"
        ;;
    *)
        printf 'unknown command: %s\n\n' "$command_name" >&2
        usage >&2
        exit 1
        ;;
esac
