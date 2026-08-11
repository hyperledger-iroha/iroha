#!/usr/bin/env bash
set -euo pipefail

readonly broker=/usr/local/bin/taira_bootle_lantern_broker
readonly validator_entrypoint=/usr/local/bin/docker_entrypoint.sh
readonly readiness=/opt/iroha/configs/soranexus/taira/wait_taira_bootle_lantern_broker_ready.sh
readonly credentials=/run/credentials/taira-bootle-lantern

for required in \
    TAIRA_NETWORK_ID \
    TAIRA_BOOTLE_LANTERN_EXPECTED_POLICY_RECORD_DIGEST \
    TAIRA_BOOTLE_LANTERN_EXPECTED_QUALIFICATION_POLICY_DIGEST; do
    value="${!required:-}"
    if [[ "$required" == TAIRA_NETWORK_ID ]]; then
        if [[ -z "$value" ]]; then
            printf 'missing public peer-1 broker binding: %s\n' "$required" >&2
            exit 1
        fi
    elif [[ ! "$value" =~ ^[0-9a-f]{64}$ || "$value" == "$(printf '00%.0s' {1..32})" ]]; then
        printf 'missing or invalid public peer-1 broker binding: %s\n' "$required" >&2
        exit 1
    fi
done

broker_pid=""
validator_pid=""

terminate_children() {
    trap - INT TERM HUP
    if [[ -n "$validator_pid" ]] && kill -0 "$validator_pid" 2>/dev/null; then
        kill -TERM "$validator_pid" 2>/dev/null || true
    fi
    if [[ -n "$broker_pid" ]] && kill -0 "$broker_pid" 2>/dev/null; then
        kill -TERM "$broker_pid" 2>/dev/null || true
    fi
    [[ -z "$validator_pid" ]] || wait "$validator_pid" 2>/dev/null || true
    [[ -z "$broker_pid" ]] || wait "$broker_pid" 2>/dev/null || true
}
trap terminate_children INT TERM HUP

"$broker" serve \
    --chain-id fc56984b-2be7-431d-840e-21514d1883f0 \
    --network-id "$TAIRA_NETWORK_ID" \
    --handle runtime://privacy/bootle-lantern/taira-primary \
    --revision 1 \
    --issuer-id 1da91b272fd76bb535968aac9c2f203a341de02caad2069759953eb4e2bf2e6e \
    --policy-id 24570ebaa65d9da9d0ac25221f2b6b88d47be519a0d8243130277328f46477eb \
    --authorization-lifetime-blocks 300 \
    --issuer-seed-credential "$credentials/issuer-seed" \
    --bearer-token-credential "$credentials/bearer-token" \
    --principal-seed-credential "$credentials/principal-seed" \
    --expected-policy-record-digest "$TAIRA_BOOTLE_LANTERN_EXPECTED_POLICY_RECORD_DIGEST" \
    --expected-qualification-policy-digest "$TAIRA_BOOTLE_LANTERN_EXPECTED_QUALIFICATION_POLICY_DIGEST" &
broker_pid=$!

TAIRA_BOOTLE_LANTERN_BROKER_PID="$broker_pid" "$readiness"

"$validator_entrypoint" "$@" &
validator_pid=$!

set +e
wait -n "$broker_pid" "$validator_pid"
status=$?
set -e
terminate_children
trap - INT TERM HUP
exit "$status"
