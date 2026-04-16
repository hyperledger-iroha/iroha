#!/usr/bin/env bash
set -euo pipefail

require_file() {
    local label="$1"
    local path="$2"

    if [[ -f "$path" ]]; then
        return 0
    fi

    printf 'missing %s: %s\n' "$label" "$path" >&2
    exit 1
}

run_default_taira_command() {
    local config_path="${IROHA_TAIRA_CONFIG:-/config/config.toml}"
    local genesis_path="${IROHA_TAIRA_GENESIS:-/opt/iroha/configs/soranexus/taira/genesis.json}"

    require_file "Taira config" "$config_path"
    require_file "Taira genesis" "$genesis_path"

    exec irohad --sora --config "$config_path" --genesis "$genesis_path"
}

main() {
    if (($# > 0)); then
        exec "$@"
    fi

    if [[ "${IROHA_IMAGE_CONFIG_PROFILE:-single}" == "taira" ]]; then
        run_default_taira_command
    fi

    exec irohad
}

main "$@"
