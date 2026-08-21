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
    local runtime_config_dir
    local runtime_config_tmp

    require_file "Taira config" "$config_path"
    require_file "Taira genesis" "$genesis_path"
    if [[ -n "$signed_genesis_path" ]]; then
        require_file "Taira signed genesis" "$signed_genesis_path"
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
    trap 'rm -f -- "$runtime_config_tmp"' EXIT
    cp "$config_path" "$runtime_config_tmp"
    chmod 0600 "$runtime_config_tmp"
    if [[ -n "$signed_genesis_path" ]]; then
        set_genesis_file_override "$runtime_config_tmp" "$signed_genesis_path"
    fi
    mv -f -- "$runtime_config_tmp" "$runtime_config_path"
    trap - EXIT

    exec iroha3d --sora --config "$runtime_config_path" --genesis-manifest-json "$genesis_path"
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
