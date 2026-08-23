#!/usr/bin/env bash
set -euo pipefail

require_file() {
    local label="$1"
    local path="$2"

    if [[ -f "$path" && ! -L "$path" && -r "$path" && -s "$path" ]]; then
        return 0
    fi
    printf 'missing %s, or it is empty, unreadable, or symlinked: %s\n' \
        "$label" "$path" >&2
    exit 1
}

require_owner_only_regular_file() {
    local label="$1"
    local path="$2"
    local size_bytes="$3"
    local match

    if [[ -L "$path" ]]; then
        printf 'symlinked %s is forbidden: %s\n' "$label" "$path" >&2
        exit 1
    fi
    match="$(find "$path" -prune -type f -user "$(id -u)" -links 1 -size "${size_bytes}c" -perm 600 -print 2>/dev/null || true)"
    if [[ "$match" != "$path" ]]; then
        printf '%s must be an owner-0600, single-link, %s-byte regular file: %s\n' \
            "$label" "$size_bytes" "$path" >&2
        exit 1
    fi
}

file_identity() {
    local path="$1"

    if stat -c '%d:%i:%u:%f:%h:%s:%Y:%Z' "$path" >/dev/null 2>&1; then
        stat -c '%d:%i:%u:%f:%h:%s:%Y:%Z' "$path"
    else
        stat -f '%d:%i:%u:%p:%l:%z:%m:%c' "$path"
    fi
}

set_genesis_file_override() {
    local config_path="$1"
    local genesis_file_path="$2"
    local tmp_file

    tmp_file="$(mktemp)"
    awk -v genesis_file_path="$genesis_file_path" '
        BEGIN { in_genesis = 0; wrote_file = 0 }
        /^\[genesis\][[:space:]]*$/ {
            if (in_genesis && !wrote_file) {
                print "file = \"" genesis_file_path "\""
            }
            in_genesis = 1
            wrote_file = 0
            print
            next
        }
        /^\[[^]]+\][[:space:]]*$/ {
            if (in_genesis && !wrote_file) {
                print "file = \"" genesis_file_path "\""
            }
            in_genesis = 0
            print
            next
        }
        in_genesis && /^[[:space:]]*file[[:space:]]*=/ {
            print "file = \"" genesis_file_path "\""
            wrote_file = 1
            next
        }
        { print }
        END {
            if (in_genesis && !wrote_file) {
                print "file = \"" genesis_file_path "\""
            }
        }
    ' "$config_path" >"$tmp_file"
    mv "$tmp_file" "$config_path"
}

run_default_taira_command() {
    local config_path="${IROHA_TAIRA_CONFIG:-/config/config.toml}"
    local runtime_config_path="${IROHA_TAIRA_RUNTIME_CONFIG:-/storage/runtime-config.toml}"
    local genesis_path="${IROHA_TAIRA_GENESIS:-/opt/iroha/configs/soranexus/taira/genesis.json}"
    local signed_genesis_path="${IROHA_TAIRA_SIGNED_GENESIS:-}"
    local runtime_signer_path="/run/secrets/iroha-taira-runtime-signer.private_key"
    local runtime_signer_launch_path="/storage/private/taira-runtime-signer.fd198"
    local runtime_signer_dir
    local runtime_signer_source_identity
    local runtime_signer_tmp=""
    local runtime_config_dir
    local runtime_config_tmp=""

    require_file "Taira config" "$config_path"
    require_file "Taira genesis" "$genesis_path"
    require_file "Taira runtime signer" "$runtime_signer_path"
    require_owner_only_regular_file "Taira runtime signer" "$runtime_signer_path" 71
    if [[ -n "$signed_genesis_path" ]]; then
        require_file "Taira signed genesis" "$signed_genesis_path"
    fi

    runtime_signer_dir="$(dirname "$runtime_signer_launch_path")"
    if [[ -L "$runtime_signer_dir" ]]; then
        printf 'refusing symlinked Taira private runtime directory: %s\n' \
            "$runtime_signer_dir" >&2
        exit 1
    fi
    mkdir -p "$runtime_signer_dir"
    chmod 0700 "$runtime_signer_dir"
    if [[ "$(find "$runtime_signer_dir" -prune -type d -user "$(id -u)" -perm 700 -print 2>/dev/null || true)" != "$runtime_signer_dir" ]]; then
        printf 'Taira private runtime directory must be owner-0700: %s\n' \
            "$runtime_signer_dir" >&2
        exit 1
    fi
    if [[ -L "$runtime_signer_launch_path" ]]; then
        printf 'refusing symlinked Taira FD198 launch file: %s\n' \
            "$runtime_signer_launch_path" >&2
        exit 1
    fi
    if [[ -e "$runtime_signer_launch_path" ]]; then
        if [[ "$(find "$runtime_signer_launch_path" -prune -type f -user "$(id -u)" -links 1 \( -size 0c -o -size 71c \) -perm 600 -print 2>/dev/null || true)" != "$runtime_signer_launch_path" ]]; then
            printf 'refusing untrusted stale Taira FD198 launch file: %s\n' \
                "$runtime_signer_launch_path" >&2
            exit 1
        fi
        rm -f -- "$runtime_signer_launch_path"
    fi
    trap 'rm -f -- "${runtime_config_tmp:-}" "${runtime_signer_tmp:-}" "${runtime_signer_launch_path:-}"' EXIT
    runtime_signer_source_identity="$(file_identity "$runtime_signer_path")"
    runtime_signer_tmp="$(mktemp "${runtime_signer_launch_path}.tmp.XXXXXXXXXX")"
    cp "$runtime_signer_path" "$runtime_signer_tmp"
    chmod 0600 "$runtime_signer_tmp"
    require_owner_only_regular_file "staged Taira FD198 signer" "$runtime_signer_tmp" 71
    if [[ "$(file_identity "$runtime_signer_path")" != "$runtime_signer_source_identity" ]]; then
        printf 'Taira runtime signer changed while staging: %s\n' \
            "$runtime_signer_path" >&2
        exit 1
    fi
    sync
    mv -f -- "$runtime_signer_tmp" "$runtime_signer_launch_path"
    runtime_signer_tmp=""
    require_owner_only_regular_file "Taira FD198 launch file" "$runtime_signer_launch_path" 71
    if ! exec 198<>"$runtime_signer_launch_path"; then
        printf 'cannot open consumable Taira runtime signer on fixed descriptor 198: %s\n' \
            "$runtime_signer_launch_path" >&2
        exit 1
    fi

    runtime_config_dir="$(dirname "$runtime_config_path")"
    if [[ -L "$runtime_config_dir" ]]; then
        printf 'refusing symlinked Taira runtime config directory: %s\n' \
            "$runtime_config_dir" >&2
        exit 1
    fi
    mkdir -p "$runtime_config_dir"
    if [[ -d "$runtime_config_path" ]]; then
        printf 'Taira runtime config path must not be a directory: %s\n' \
            "$runtime_config_path" >&2
        exit 1
    fi
    runtime_config_tmp="$(mktemp "${runtime_config_path}.tmp.XXXXXXXXXX")"
    cp "$config_path" "$runtime_config_tmp"
    chmod 0600 "$runtime_config_tmp"
    if [[ -n "$signed_genesis_path" ]]; then
        set_genesis_file_override "$runtime_config_tmp" "$signed_genesis_path"
    fi
    mv -f -- "$runtime_config_tmp" "$runtime_config_path"
    runtime_config_tmp=""

    exec iroha3d_taira --sora --config "$runtime_config_path" --genesis-manifest-json "$genesis_path"
}

main() {
    if (($# > 0)); then
        exec "$@"
    fi
    if [[ "${IROHA_IMAGE_CONFIG_PROFILE:-single}" == "taira" ]]; then
        run_default_taira_command
    fi
    exec iroha3d
}

main "$@"
