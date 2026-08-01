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

require_directory() {
    local label="$1"
    local path="$2"

    if [[ -d "$path" && ! -L "$path" && -r "$path" && -x "$path" ]]; then
        return 0
    fi

    printf 'missing, unreadable, or symlinked %s: %s\n' "$label" "$path" >&2
    exit 1
}

configured_path() {
    local config_path="$1"
    local section="$2"
    local key="$3"
    local label="$4"
    local value

    if ! value="$(
        awk -v expected_section="$section" -v expected_key="$key" '
            function trim(value) {
                sub(/^[[:space:]]+/, "", value)
                sub(/[[:space:]]+$/, "", value)
                return value
            }
            {
                line = $0
                stripped = trim(line)
                if (stripped ~ /^\[/) {
                    in_section = (stripped == expected_section)
                    next
                }
                if (!in_section) {
                    next
                }
                assignment = "^[[:space:]]*" expected_key "[[:space:]]*="
                if (line ~ assignment) {
                    count += 1
                    sub(assignment, "", line)
                    line = trim(line)
                    if (line !~ /^"[^"]+"$/) {
                        invalid = 1
                    } else {
                        sub(/^"/, "", line)
                        sub(/"$/, "", line)
                        value = line
                    }
                }
            }
            END {
                if (count != 1 || invalid || value == "") {
                    exit 1
                }
                print value
            }
        ' "$config_path"
    )"; then
        printf 'missing or non-canonical %s in Taira config\n' "$label" >&2
        exit 1
    fi
    if [[ "$value" != /* ]]; then
        printf '%s must be an absolute path in Taira config: %s\n' \
            "$label" "$value" >&2
        exit 1
    fi
    printf '%s\n' "$value"
}

require_immutable_image_reference() {
    local reference="${TAIRA_IMAGE_REFERENCE:-}"

    if [[ "$reference" =~ ^sha256:[0-9a-f]{64}$ ]] \
        || [[ "$reference" =~ ^[a-z0-9][a-z0-9._/:+-]*@sha256:[0-9a-f]{64}$ ]]; then
        return 0
    fi
    printf '%s\n' \
        'production TAIRA_IMAGE_REFERENCE must be an immutable image ID or repository@sha256 digest' >&2
    exit 1
}

validate_production_assets() {
    local config_path="$1"
    local require_image_reference="${2:-1}"
    local config_root
    local runtime_dir
    local onboarding_signer
    local faucet_signer
    local admission_dir
    local manifest_dir
    local cache_dir

    if [[ "$require_image_reference" == "1" ]]; then
        require_immutable_image_reference
    fi
    config_root="$(dirname "$config_path")"
    runtime_dir="$config_root/runtime"
    onboarding_signer="$(
        configured_path \
            "$config_path" \
            "[torii.account_onboarding]" \
            "private_key_file" \
            "Taira onboarding signer path"
    )"
    faucet_signer="$(
        configured_path \
            "$config_path" \
            "[torii.faucet]" \
            "private_key_file" \
            "Taira faucet signer path"
    )"
    admission_dir="$(
        configured_path \
            "$config_path" \
            "[sorafs.discovery.admission]" \
            "envelopes_dir" \
            "Taira SoraFS admission directory"
    )"
    manifest_dir="$(
        configured_path \
            "$config_path" \
            "[nexus.registry]" \
            "manifest_directory" \
            "Taira governance manifest directory"
    )"
    cache_dir="$(
        configured_path \
            "$config_path" \
            "[nexus.registry]" \
            "cache_directory" \
            "Taira governance cache directory"
    )"

    if [[ "$onboarding_signer" != "$config_root/runtime/onboarding-signer.key" ]] \
        || [[ "$faucet_signer" != "$config_root/runtime/faucet-signer.key" ]] \
        || [[ "$admission_dir" != "$config_root/sorafs_admission" ]] \
        || [[ "$manifest_dir" != "$config_root/manifests" ]] \
        || [[ "$cache_dir" != "$manifest_dir" ]]; then
        printf '%s\n' \
            'production Taira config paths must match the canonical mounted bundle' >&2
        exit 1
    fi

    require_directory "Taira config bundle directory" "$config_root"
    require_directory "Taira runtime directory" "$runtime_dir"
    require_file "Taira onboarding signer" "$onboarding_signer"
    require_file "Taira faucet signer" "$faucet_signer"
    require_directory "Taira SoraFS admission directory" "$admission_dir"
    require_directory "Taira governance manifest directory" "$manifest_dir"
    require_file \
        "Taira governance manifest" \
        "$manifest_dir/governance.manifest.json"
}

set_genesis_file_override() {
    local config_path="$1"
    local genesis_file_path="$2"
    local tmp_file

    tmp_file="$(mktemp)"
    awk -v genesis_file_path="$genesis_file_path" '
        BEGIN {
            in_genesis = 0
            wrote_file = 0
        }
        /^\[genesis\][[:space:]]*$/ {
            if (in_genesis && !wrote_file) {
                print "file = \"" genesis_file_path "\""
                wrote_file = 1
            }
            in_genesis = 1
            wrote_file = 0
            print
            next
        }
        /^\[[^]]+\][[:space:]]*$/ {
            if (in_genesis && !wrote_file) {
                print "file = \"" genesis_file_path "\""
                wrote_file = 1
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
        {
            print
        }
        END {
            if (in_genesis && !wrote_file) {
                print "file = \"" genesis_file_path "\""
            }
        }
    ' "$config_path" >"$tmp_file"
    mv "$tmp_file" "$config_path"
}

run_default_taira_command() {
    local config_path="${IROHA_TAIRA_CONFIG:-/etc/iroha/taira-validator/config.toml}"
    local runtime_config_path="${IROHA_TAIRA_RUNTIME_CONFIG:-/storage/runtime-config.toml}"
    local genesis_path="${IROHA_TAIRA_GENESIS:-/opt/iroha/configs/soranexus/taira/genesis.json}"
    local signed_genesis_path="${IROHA_TAIRA_SIGNED_GENESIS:-}"
    local runtime_profile="${TAIRA_RUNTIME_PROFILE:-production}"
    local runtime_config_dir
    local runtime_config_tmp
    local cleanup_command

    require_file "Taira config" "$config_path"
    require_file "Taira genesis" "$genesis_path"
    if [[ -n "$signed_genesis_path" ]]; then
        require_file "Taira signed genesis" "$signed_genesis_path"
    fi
    case "$runtime_profile" in
        production)
            validate_production_assets "$config_path"
            ;;
        localnet)
            ;;
        *)
            printf 'TAIRA_RUNTIME_PROFILE must be exactly production or localnet\n' >&2
            exit 1
            ;;
    esac

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
    printf -v cleanup_command 'rm -f -- %q' "$runtime_config_tmp"
    trap "$cleanup_command" EXIT
    cp "$config_path" "$runtime_config_tmp"
    chmod 0600 "$runtime_config_tmp"
    if [[ -n "$signed_genesis_path" ]]; then
        set_genesis_file_override "$runtime_config_tmp" "$signed_genesis_path"
    fi
    mv -f -- "$runtime_config_tmp" "$runtime_config_path"
    trap - EXIT

    exec irohad --sora --config "$runtime_config_path" --genesis-manifest-json "$genesis_path"
}

main() {
    if [[ "${1:-}" == "--validate-taira-production-config" ]]; then
        if [[ $# -ne 2 ]]; then
            printf '%s\n' \
                'usage: docker_entrypoint.sh --validate-taira-production-config <config.toml>' >&2
            exit 2
        fi
        require_file "Taira config" "$2"
        validate_production_assets "$2" 0
        return 0
    fi

    if (($# > 0)); then
        exec "$@"
    fi

    if [[ "${IROHA_IMAGE_CONFIG_PROFILE:-single}" == "taira" ]]; then
        run_default_taira_command
    fi

    exec irohad
}

main "$@"
