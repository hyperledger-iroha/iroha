#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: taira-validator-container.sh [--env-file <path>] <command>

Commands:
  config   Print the resolved `docker run` command.
  up       Pull the image when missing, replace any existing container, and start it.
  down     Remove the container if it exists.
  reset    Stop the container and wipe configured mutable state directories.
  restart  Recreate the container.
  pull     Pull the configured immutable image reference.
  status   Show `docker ps` status for the configured container.
  logs     Show container logs.
EOF
}

env_file=""

while (($#)); do
    case "$1" in
        --env-file)
            if [[ $# -lt 2 || -z "$2" ]]; then
                printf '%s\n' '--env-file requires one non-empty path' >&2
                exit 1
            fi
            env_file="$2"
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

if [[ ! -f "$env_file" || -L "$env_file" || ! -r "$env_file" ]]; then
    printf 'missing, unreadable, or symlinked Taira environment file: %s\n' \
        "$env_file" >&2
    exit 1
fi
# The env file is an operator-managed shell fragment containing only
# variable assignments and comments.
set -a
# shellcheck disable=SC1090
. "$env_file"
set +a

TAIRA_CONTAINER_NAME="${TAIRA_CONTAINER_NAME:-taira-validator-1}"
TAIRA_IMAGE="${TAIRA_IMAGE:-}"
TAIRA_CONFIG_BUNDLE_PATH="${TAIRA_CONFIG_BUNDLE_PATH:-/etc/iroha/taira-validator}"
TAIRA_STORAGE_PATH="${TAIRA_STORAGE_PATH:-/var/lib/iroha/taira-validator-1}"
TAIRA_P2P_PORT="${TAIRA_P2P_PORT:-1337}"
TAIRA_TORII_PORT="${TAIRA_TORII_PORT:-18080}"
TAIRA_RUST_LOG="${TAIRA_RUST_LOG:-info}"
TAIRA_RUNTIME_PROFILE="${TAIRA_RUNTIME_PROFILE:-production}"
TAIRA_INROU_PORTABLE_ACCEL="${TAIRA_INROU_PORTABLE_ACCEL:-auto}"
TAIRA_EXPOSE_KVM="${TAIRA_EXPOSE_KVM:-auto}"
TAIRA_GENESIS_PATH="${TAIRA_GENESIS_PATH:-}"
TAIRA_SIGNED_GENESIS_PATH="${TAIRA_SIGNED_GENESIS_PATH:-}"
TAIRA_SORAFS_SITE_BINDINGS_PATH="${TAIRA_SORAFS_SITE_BINDINGS_PATH:-}"
TAIRA_DOCKER_NETWORK="${TAIRA_DOCKER_NETWORK:-}"

case "$TAIRA_RUNTIME_PROFILE" in
    production|localnet) ;;
    *)
        printf 'TAIRA_RUNTIME_PROFILE must be exactly production or localnet\n' >&2
        exit 1
        ;;
esac
CONTAINER_CONFIG_ROOT="/etc/iroha/taira-validator"
CONTAINER_CONFIG_PATH="${CONTAINER_CONFIG_ROOT}/config.toml"
TAIRA_CONFIG_PATH="${TAIRA_CONFIG_BUNDLE_PATH}/config.toml"
TAIRA_RUNTIME_PATH="${TAIRA_CONFIG_BUNDLE_PATH}/runtime"
TAIRA_MANIFEST_PATH="${TAIRA_CONFIG_BUNDLE_PATH}/manifests"
TAIRA_GOVERNANCE_MANIFEST_PATH="${TAIRA_CONFIG_BUNDLE_PATH}/manifests/governance.manifest.json"
TAIRA_ONBOARDING_SIGNER_PATH="${TAIRA_CONFIG_BUNDLE_PATH}/runtime/onboarding-signer.key"
TAIRA_FAUCET_SIGNER_PATH="${TAIRA_CONFIG_BUNDLE_PATH}/runtime/faucet-signer.key"
TAIRA_SORAFS_ADMISSION_PATH="${TAIRA_CONFIG_BUNDLE_PATH}/sorafs_admission"

require_file() {
    local path="$1"
    local label="$2"
    if [[ ! -f "$path" || -L "$path" || ! -r "$path" || ! -s "$path" ]]; then
        printf 'missing, empty, unreadable, or symlinked %s at %s\n' "$label" "$path" >&2
        exit 1
    fi
}

validate_image_reference() {
    if [[ -z "$TAIRA_IMAGE" ]]; then
        printf 'TAIRA_IMAGE must identify the admitted validator image\n' >&2
        exit 1
    fi
    if [[ "$TAIRA_RUNTIME_PROFILE" == "production" ]] \
        && [[ ! "$TAIRA_IMAGE" =~ ^sha256:[0-9a-f]{64}$ ]] \
        && [[ ! "$TAIRA_IMAGE" =~ ^[a-z0-9][a-z0-9._/:+-]*@sha256:[0-9a-f]{64}$ ]]; then
        printf '%s\n' \
            'production TAIRA_IMAGE must be an immutable image ID or repository@sha256 digest' >&2
        exit 1
    fi
}

require_directory() {
    local path="$1"
    local label="$2"
    if [[ ! -d "$path" || -L "$path" ]]; then
        printf 'missing or symlinked %s at %s\n' "$label" "$path" >&2
        exit 1
    fi
}

require_canonical_directory() {
    local path="$1"
    local label="$2"
    local physical_path

    require_directory "$path" "$label"
    case "$path" in
        /*) ;;
        *)
            printf '%s must use an absolute path: %s\n' "$label" "$path" >&2
            exit 1
            ;;
    esac
    physical_path="$(
        cd "$path"
        pwd -P
    )"
    if [[ "$physical_path" != "$path" ]]; then
        printf '%s must use its canonical physical path: %s\n' "$label" "$path" >&2
        exit 1
    fi
}

require_disjoint_directories() {
    local left="$1"
    local left_label="$2"
    local right="$3"
    local right_label="$4"

    if [[ "$left" == "$right" || "$left" == "$right/"* || "$right" == "$left/"* ]]; then
        printf '%s and %s must not be equal or nested: %s / %s\n' \
            "$left_label" "$right_label" "$left" "$right" >&2
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

docker_cmd=(docker)

build_run_args() {
    validate_image_reference
    require_canonical_directory "$TAIRA_CONFIG_BUNDLE_PATH" "Taira config bundle"
    require_file "$TAIRA_CONFIG_PATH" "Taira config"
    require_canonical_directory "$TAIRA_STORAGE_PATH" "Taira storage directory"
    require_disjoint_directories \
        "$TAIRA_CONFIG_BUNDLE_PATH" "Taira config bundle" \
        "$TAIRA_STORAGE_PATH" "Taira storage directory"
    case "$TAIRA_RUNTIME_PROFILE" in
        production)
            container_torii_port=18080
            require_canonical_directory "$TAIRA_RUNTIME_PATH" "Taira runtime directory"
            require_canonical_directory "$TAIRA_MANIFEST_PATH" "Taira manifest directory"
            require_file "$TAIRA_GOVERNANCE_MANIFEST_PATH" "Taira governance manifest"
            require_file "$TAIRA_ONBOARDING_SIGNER_PATH" "Taira onboarding signer"
            require_file "$TAIRA_FAUCET_SIGNER_PATH" "Taira faucet signer"
            require_canonical_directory "$TAIRA_SORAFS_ADMISSION_PATH" "Taira SoraFS admission directory"
            ;;
        localnet)
            container_torii_port=8080
            ;;
        *)
            printf 'TAIRA_RUNTIME_PROFILE must be exactly production or localnet\n' >&2
            exit 1
            ;;
    esac

    docker_run_args=(
        run -d
        --name "$TAIRA_CONTAINER_NAME"
        --restart unless-stopped
        --init
        --workdir "$CONTAINER_CONFIG_ROOT"
        -e "RUST_LOG=$TAIRA_RUST_LOG"
        -e "IROHA_INROU_PORTABLE_ACCEL=$TAIRA_INROU_PORTABLE_ACCEL"
        -e "IROHA_TAIRA_CONFIG=$CONTAINER_CONFIG_PATH"
        -e "TAIRA_RUNTIME_PROFILE=$TAIRA_RUNTIME_PROFILE"
        -e "TAIRA_IMAGE_REFERENCE=$TAIRA_IMAGE"
        -p "${TAIRA_P2P_PORT}:1337"
        -p "${TAIRA_TORII_PORT}:${container_torii_port}"
        -v "${TAIRA_CONFIG_BUNDLE_PATH}:${CONTAINER_CONFIG_ROOT}:ro"
        -v "${TAIRA_STORAGE_PATH}:/storage"
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

do_pull() {
    validate_image_reference
    "${docker_cmd[@]}" pull "$TAIRA_IMAGE"
}

do_down() {
    if container_exists; then
        "${docker_cmd[@]}" rm -f "$TAIRA_CONTAINER_NAME" >/dev/null
    fi
}

resolve_state_path() {
    local path="$1"
    local physical_path

    if [[ -L "$path" ]]; then
        printf 'refusing symlinked state directory: %s\n' "$path" >&2
        exit 1
    fi
    case "$path" in
        /*) ;;
        *)
            printf 'refusing relative state directory: %s\n' "$path" >&2
            exit 1
            ;;
    esac
    case "$path" in
        /|/bin|/bin/*|/boot|/boot/*|/dev|/dev/*|/etc|/etc/*|/lib|/lib/*|/lib64|/lib64/*|/proc|/proc/*|/root|/root/*|/run|/run/*|/sbin|/sbin/*|/sys|/sys/*|/usr|/usr/*|/home|/opt|/srv|/tmp|/var|/var/lib|/var/lib/iroha)
            printf 'refusing broad system state directory: %s\n' "$path" >&2
            exit 1
            ;;
    esac
    mkdir -p "$path"
    physical_path="$(
        cd "$path"
        pwd -P
    )"
    if [[ "$physical_path" != "$path" ]]; then
        printf 'state directory must use its canonical physical path: %s\n' "$path" >&2
        exit 1
    fi
    printf '%s\n' "$physical_path"
}

do_reset() {
    local storage_real
    local config_real

    require_canonical_directory "$TAIRA_CONFIG_BUNDLE_PATH" "Taira config bundle"
    config_real="$TAIRA_CONFIG_BUNDLE_PATH"
    storage_real="$(resolve_state_path "$TAIRA_STORAGE_PATH")"
    require_disjoint_directories \
        "$config_real" "Taira config bundle" \
        "$storage_real" "Taira storage directory"
    do_down
    find "$storage_real" -xdev -mindepth 1 -depth -delete
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
