#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
    printf 'Usage: %s <bundle.tar.zst> [bundle.tar.zst...]\n' "$0" >&2
    exit 1
fi

for bundle in "$@"; do
    if [[ ! -f "$bundle" ]]; then
        printf '[dual-smoke] bundle not found: %s\n' "$bundle" >&2
        exit 2
    fi

    printf '[dual-smoke] verifying %s\n' "$bundle"
    workdir="$(mktemp -d)"
    trap 'rm -rf "$workdir"' RETURN

    tar -I zstd -xf "$bundle" -C "$workdir"
    profile_dir="$(find "$workdir" -mindepth 1 -maxdepth 1 -type d | head -n 1)"
    if [[ -z "$profile_dir" ]]; then
        printf '[dual-smoke] failed to extract profile directory from %s\n' "$bundle" >&2
        exit 3
    fi

    if [[ ! -f "$profile_dir/PROFILE.toml" ]]; then
        printf '[dual-smoke] PROFILE.toml missing in %s\n' "$bundle" >&2
        exit 4
    fi

    profile_name="$(grep -E '^profile *= *\"' "$profile_dir/PROFILE.toml" | sed -E 's/profile *= *\"([^\"]+)\"/\1/')"
    if [[ -z "$profile_name" ]]; then
        printf '[dual-smoke] profile name missing in PROFILE.toml for %s\n' "$bundle" >&2
        exit 5
    fi

    if [[ ! -x "$profile_dir/bin/irohad" ]]; then
        printf '[dual-smoke] irohad binary missing/executable bit unset in %s\n' "$bundle" >&2
        exit 6
    fi

    "$profile_dir/bin/irohad" --version >/dev/null
    "$profile_dir/bin/kagami" --help >/dev/null

    case "$profile_name" in
        iroha2)
            test -f "$profile_dir/config/genesis.json"
            test -f "$profile_dir/config/client.toml"
            ;;
        iroha3)
            test -f "$profile_dir/config/genesis.json"
            test -f "$profile_dir/config/config.toml"
            ;;
    esac

    rm -rf "$workdir"
    trap - RETURN
done
