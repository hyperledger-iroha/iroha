#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: taira-validator-container.sh [--env-file <path>] <command>

Commands:
  config   Print the resolved `docker run` command.
  up       Pull the image when missing, replace any existing container, and start it.
  down     Remove the container if it exists.
  reset    Stop the container and wipe the configured storage directory.
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
TAIRA_KAGEMUSHA_RELEASE_POLICY_PATH="${TAIRA_KAGEMUSHA_RELEASE_POLICY_PATH:-}"
TAIRA_KAGEMUSHA_ARTIFACT_DIR="${TAIRA_KAGEMUSHA_ARTIFACT_DIR:-}"
TAIRA_P2P_PORT="${TAIRA_P2P_PORT:-1337}"
TAIRA_TORII_PORT="${TAIRA_TORII_PORT:-18080}"
TAIRA_RUST_LOG="${TAIRA_RUST_LOG:-info}"
TAIRA_HEALTH_TIMEOUT_SECONDS="${TAIRA_HEALTH_TIMEOUT_SECONDS:-180}"
TAIRA_HEALTH_POLL_SECONDS="${TAIRA_HEALTH_POLL_SECONDS:-2}"
TAIRA_INROU_PORTABLE_ACCEL="${TAIRA_INROU_PORTABLE_ACCEL:-auto}"
TAIRA_EXPOSE_KVM="${TAIRA_EXPOSE_KVM:-auto}"
TAIRA_GENESIS_PATH="${TAIRA_GENESIS_PATH:-}"
TAIRA_SIGNED_GENESIS_PATH="${TAIRA_SIGNED_GENESIS_PATH:-}"
TAIRA_SORAFS_SITE_BINDINGS_PATH="${TAIRA_SORAFS_SITE_BINDINGS_PATH:-}"
TAIRA_DOCKER_NETWORK="${TAIRA_DOCKER_NETWORK:-}"
KAGEMUSHA_CONTAINER_RELEASE_POLICY_PATH="/etc/iroha/kagemusha/release-policy.norito"
KAGEMUSHA_CONTAINER_ARTIFACT_DIR="/var/lib/iroha/kagemusha/v4"

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

require_positive_integer() {
    local value="$1"
    local label="$2"
    if [[ ! "$value" =~ ^[1-9][0-9]*$ ]]; then
        printf '%s must be a positive integer, got %s\n' "$label" "$value" >&2
        exit 1
    fi
}

config_declares_container_site_bindings() {
    awk '
        /^[[:space:]]*\[/ {
            in_site_bindings = ($0 ~ /^[[:space:]]*\[sorafs\.gateway\.site_bindings\][[:space:]]*$/)
        }
        in_site_bindings && /^[[:space:]]*path[[:space:]]*=[[:space:]]*"\/config\/sorafs_sites\.json"[[:space:]]*$/ {
            found = 1
        }
        END { exit(found ? 0 : 1) }
    ' "$TAIRA_CONFIG_PATH"
}

config_declares_container_kagemusha_inputs() {
    awk \
        -v policy_path="$KAGEMUSHA_CONTAINER_RELEASE_POLICY_PATH" \
        -v artifact_dir="$KAGEMUSHA_CONTAINER_ARTIFACT_DIR" '
        /^[[:space:]]*\[/ {
            in_offline = ($0 ~ /^[[:space:]]*\[settlement\.offline\][[:space:]]*$/)
        }
        in_offline {
            line = $0
            sub(/[[:space:]]*#.*/, "", line)
            gsub(/[[:space:]]/, "", line)
            if (line == "kagemusha_release_policy_path=\"" policy_path "\"") {
                found_policy = 1
            }
            if (line == "kagemusha_artifact_dir=\"" artifact_dir "\"") {
                found_artifacts = 1
            }
        }
        END { exit(found_policy && found_artifacts ? 0 : 1) }
    ' "$TAIRA_CONFIG_PATH"
}

docker_cmd=(docker)

build_run_args() {
    require_file "$TAIRA_CONFIG_PATH" "Taira config"
    require_directory "$TAIRA_STORAGE_PATH" "Taira storage directory"
    if [[ -z "$TAIRA_KAGEMUSHA_RELEASE_POLICY_PATH" ]]; then
        printf '%s\n' \
            'TAIRA_KAGEMUSHA_RELEASE_POLICY_PATH is required for mandatory offline startup' >&2
        exit 1
    fi
    if [[ -z "$TAIRA_KAGEMUSHA_ARTIFACT_DIR" ]]; then
        printf '%s\n' \
            'TAIRA_KAGEMUSHA_ARTIFACT_DIR is required for mandatory offline startup' >&2
        exit 1
    fi
    require_file "$TAIRA_KAGEMUSHA_RELEASE_POLICY_PATH" \
        "authenticated Kagemusha release policy"
    require_directory "$TAIRA_KAGEMUSHA_ARTIFACT_DIR" \
        "reviewed Kagemusha V4 artifact directory"
    if ! config_declares_container_kagemusha_inputs; then
        printf '%s\n' \
            "Taira config must bind [settlement.offline] kagemusha_release_policy_path = \"$KAGEMUSHA_CONTAINER_RELEASE_POLICY_PATH\" and kagemusha_artifact_dir = \"$KAGEMUSHA_CONTAINER_ARTIFACT_DIR\"" >&2
        exit 1
    fi

    docker_run_args=(
        run -d
        --name "$TAIRA_CONTAINER_NAME"
        --restart unless-stopped
        --init
        -e "RUST_LOG=$TAIRA_RUST_LOG"
        -e "IROHA_INROU_PORTABLE_ACCEL=$TAIRA_INROU_PORTABLE_ACCEL"
        --health-cmd "curl -fsS http://127.0.0.1:8080/readyz"
        --health-interval 10s
        --health-timeout 3s
        --health-retries 12
        --health-start-period 20s
        -p "${TAIRA_P2P_PORT}:1337"
        -p "${TAIRA_TORII_PORT}:8080"
        -v "${TAIRA_CONFIG_PATH}:/config/config.toml:ro"
        -v "${TAIRA_STORAGE_PATH}:/storage"
        --mount "type=bind,source=${TAIRA_KAGEMUSHA_RELEASE_POLICY_PATH},target=${KAGEMUSHA_CONTAINER_RELEASE_POLICY_PATH},readonly"
        --mount "type=bind,source=${TAIRA_KAGEMUSHA_ARTIFACT_DIR},target=${KAGEMUSHA_CONTAINER_ARTIFACT_DIR},readonly"
    )

    if [[ "$TAIRA_EXPOSE_KVM" == "1" || "$TAIRA_EXPOSE_KVM" == "true" ]]; then
        docker_run_args+=(--device /dev/kvm)
    elif [[ "$TAIRA_EXPOSE_KVM" == "auto" && -e /dev/kvm ]]; then
        docker_run_args+=(--device /dev/kvm)
    fi

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
        if ! config_declares_container_site_bindings; then
            printf '%s\n' \
                'Taira config must set [sorafs.gateway.site_bindings].path = "/config/sorafs_sites.json" before mounting site bindings' >&2
            exit 1
        fi
        docker_run_args+=(
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

wait_for_healthy() {
    local container_id="$1"
    local deadline status

    require_positive_integer \
        "$TAIRA_HEALTH_TIMEOUT_SECONDS" \
        "TAIRA_HEALTH_TIMEOUT_SECONDS"
    require_positive_integer \
        "$TAIRA_HEALTH_POLL_SECONDS" \
        "TAIRA_HEALTH_POLL_SECONDS"
    if ((TAIRA_HEALTH_POLL_SECONDS > TAIRA_HEALTH_TIMEOUT_SECONDS)); then
        printf '%s must not exceed TAIRA_HEALTH_TIMEOUT_SECONDS (%s > %s)\n' \
            "TAIRA_HEALTH_POLL_SECONDS" \
            "$TAIRA_HEALTH_POLL_SECONDS" \
            "$TAIRA_HEALTH_TIMEOUT_SECONDS" >&2
        return 1
    fi
    deadline=$((SECONDS + TAIRA_HEALTH_TIMEOUT_SECONDS))

    while ((SECONDS < deadline)); do
        if ! status="$(
            "${docker_cmd[@]}" inspect \
                --format '{{.State.Status}} {{if .State.Health}}{{.State.Health.Status}}{{else}}missing{{end}}' \
                "$container_id"
        )"; then
            printf 'failed to inspect new Taira container %s\n' "$container_id" >&2
            return 1
        fi
        case "$status" in
            "running healthy")
                return 0
                ;;
            "running starting")
                ;;
            "running unhealthy")
                printf 'new Taira container %s reported unhealthy\n' "$container_id" >&2
                return 1
                ;;
            *)
                printf 'new Taira container %s cannot become ready: %s\n' \
                    "$container_id" "$status" >&2
                return 1
                ;;
        esac
        sleep "$TAIRA_HEALTH_POLL_SECONDS"
    done

    printf 'new Taira container %s did not become healthy within %s seconds\n' \
        "$container_id" "$TAIRA_HEALTH_TIMEOUT_SECONDS" >&2
    return 1
}

do_pull() {
    "${docker_cmd[@]}" pull "$TAIRA_IMAGE"
}

do_down() {
    if container_exists; then
        "${docker_cmd[@]}" rm -f "$TAIRA_CONTAINER_NAME" >/dev/null
    fi
}

resolve_storage_path() {
    mkdir -p "$TAIRA_STORAGE_PATH"
    (
        cd "$TAIRA_STORAGE_PATH"
        pwd -P
    )
}

do_reset() {
    local storage_real

    do_down
    storage_real="$(resolve_storage_path)"
    if [[ -z "$storage_real" || "$storage_real" == "/" ]]; then
        printf 'refusing to wipe invalid storage directory: %s\n' "$storage_real" >&2
        exit 1
    fi
    find "$storage_real" -mindepth 1 -maxdepth 1 -exec rm -rf {} +
}

do_up() {
    local new_container_id

    build_run_args

    if ! image_exists; then
        do_pull
    fi

    do_down
    new_container_id="$("${docker_cmd[@]}" "${docker_run_args[@]}")"
    if [[ -z "$new_container_id" ]]; then
        printf 'docker run did not return the new Taira container id\n' >&2
        return 1
    fi
    if ! wait_for_healthy "$new_container_id"; then
        "${docker_cmd[@]}" rm -f "$new_container_id" >/dev/null 2>&1 || true
        return 1
    fi
    printf '%s\n' "$new_container_id"
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
    reset)
        do_reset
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
