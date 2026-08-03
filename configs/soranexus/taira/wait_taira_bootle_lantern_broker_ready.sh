#!/usr/bin/env bash
set -euo pipefail

readonly endpoint=/run/iroha/runtime-provider-broker-v1.sock
readonly parent=/run/iroha
readonly expected_uid="$(id -u)"
readonly expected_gid="$(id -g)"
readonly attempts=450
broker_pid="${TAIRA_BOOTLE_LANTERN_BROKER_PID:-}"

if [[ -n "$broker_pid" && ! "$broker_pid" =~ ^[1-9][0-9]*$ ]]; then
    printf '%s\n' 'invalid Taira Bootle/Lantern broker readiness PID' >&2
    exit 1
fi

for ((attempt = 0; attempt < attempts; attempt += 1)); do
    if [[ -n "$broker_pid" ]] && ! kill -0 "$broker_pid" 2>/dev/null; then
        printf '%s\n' 'Taira Bootle/Lantern broker exited before publishing readiness' >&2
        exit 1
    fi
    if [[ -d "$parent" && ! -L "$parent" && -S "$endpoint" && ! -L "$endpoint" ]]; then
        parent_metadata="$(stat -Lc '%u:%g:%a:%F' -- "$parent" 2>/dev/null || true)"
        endpoint_metadata="$(stat -Lc '%u:%g:%a:%h:%F' -- "$endpoint" 2>/dev/null || true)"
        if [[ "$parent_metadata" == "${expected_uid}:${expected_gid}:700:directory" \
            && "$endpoint_metadata" == "${expected_uid}:${expected_gid}:660:1:socket" ]]; then
            exit 0
        fi
    fi
    sleep 0.1
done

printf '%s\n' \
    'Taira Bootle/Lantern broker did not publish the exact same-identity socket before timeout' >&2
exit 1
