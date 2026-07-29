#!/usr/bin/env bash
set -euo pipefail

# Guard Docker Compose validator starts with the service's mandatory `/readyz`
# health check. The wrapper never treats a merely running container as admitted.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
CANONICAL_COMPOSE_FILE="${SCRIPT_DIR}/docker-compose.validator.yml"
COMPOSE_FILE="${TAIRA_COMPOSE_FILE:-$CANONICAL_COMPOSE_FILE}"
ENV_FILE="/etc/default/taira-validator-container.compose.env"

usage() {
    cat <<'EOF'
Usage: taira-validator-compose.sh [--env-file PATH] [--compose-file PATH] COMMAND

Commands:
  config    Validate and print the resolved Compose model.
  up        Start/recreate the validator and wait boundedly for `healthy`.
  restart   Force-recreate the validator and wait boundedly for `healthy`.
  down      Remove the validator service container.
  pull      Pull the configured validator image.
  status    Show Compose service status.
  logs      Show recent validator logs.
EOF
}

while (($#)); do
    case "$1" in
        --env-file)
            [[ $# -ge 2 ]] || {
                echo "missing value for --env-file" >&2
                exit 1
            }
            ENV_FILE="$2"
            shift 2
            ;;
        --compose-file)
            [[ $# -ge 2 ]] || {
                echo "missing value for --compose-file" >&2
                exit 1
            }
            COMPOSE_FILE="$2"
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

COMMAND_NAME="${1:-}"
if [[ -z "$COMMAND_NAME" ]]; then
    usage >&2
    exit 1
fi
shift || true
if (($#)); then
    echo "unexpected argument: $1" >&2
    exit 1
fi

if [[ ! -f "$ENV_FILE" ]]; then
    echo "Compose env file not found: $ENV_FILE" >&2
    exit 1
fi
if [[ ! -f "$COMPOSE_FILE" ]]; then
    echo "Compose file not found: $COMPOSE_FILE" >&2
    exit 1
fi

canonical_compose_real="$(cd -- "$(dirname -- "$CANONICAL_COMPOSE_FILE")" && pwd -P)/$(basename -- "$CANONICAL_COMPOSE_FILE")"
compose_real="$(cd -- "$(dirname -- "$COMPOSE_FILE")" && pwd -P)/$(basename -- "$COMPOSE_FILE")"
if [[ "$compose_real" != "$canonical_compose_real" ]]; then
    echo "only the reviewed mandatory-offline Compose file is allowed: $CANONICAL_COMPOSE_FILE" >&2
    exit 1
fi

# The checked-in example is intentionally valid as both a Compose env file and
# a shell assignment fragment. Source it only to obtain the bounded wait value;
# Compose remains authoritative for all service interpolation.
set -a
# shellcheck disable=SC1090
. "$ENV_FILE"
set +a
TAIRA_HEALTH_TIMEOUT_SECONDS="${TAIRA_HEALTH_TIMEOUT_SECONDS:-180}"
if [[ ! "$TAIRA_HEALTH_TIMEOUT_SECONDS" =~ ^[1-9][0-9]*$ ]]; then
    echo "TAIRA_HEALTH_TIMEOUT_SECONDS must be a positive integer" >&2
    exit 1
fi

compose=(docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE")

validate_compose() {
    "${compose[@]}" config --format json | python3 -c '
import json
import sys

payload = json.load(sys.stdin)
service = payload.get("services", {}).get("taira-validator")
if not isinstance(service, dict):
    raise SystemExit("resolved Compose model is missing service taira-validator")
expected_health = ["CMD", "curl", "-fsS", "http://127.0.0.1:8080/readyz"]
if service.get("healthcheck", {}).get("test") != expected_health:
    raise SystemExit("resolved Compose model does not use the exact mandatory /readyz healthcheck")
volumes = service.get("volumes", [])
by_target = {
    volume.get("target"): volume
    for volume in volumes
    if isinstance(volume, dict) and isinstance(volume.get("target"), str)
}
for target in (
    "/config/config.toml",
    "/etc/iroha/kagemusha/release-policy.norito",
    "/var/lib/iroha/kagemusha/v4",
):
    if by_target.get(target, {}).get("read_only") is not True:
        raise SystemExit(f"resolved Compose model is missing mandatory read-only mount {target}")
' 
}

remove_failed_service() {
    "${compose[@]}" rm --stop --force taira-validator >/dev/null 2>&1 || true
}

start_and_wait() {
    local recreate_flag="${1:-}"
    local args=(up -d)
    if [[ -n "$recreate_flag" ]]; then
        args+=("$recreate_flag")
    fi
    args+=(--wait --wait-timeout "$TAIRA_HEALTH_TIMEOUT_SECONDS" taira-validator)

    validate_compose
    if ! "${compose[@]}" "${args[@]}"; then
        echo "Taira Compose validator did not satisfy mandatory /readyz admission" >&2
        remove_failed_service
        return 1
    fi
}

case "$COMMAND_NAME" in
    config)
        validate_compose
        "${compose[@]}" config
        ;;
    up)
        start_and_wait
        ;;
    restart)
        start_and_wait --force-recreate
        ;;
    down)
        "${compose[@]}" rm --stop --force taira-validator
        ;;
    pull)
        validate_compose
        "${compose[@]}" pull taira-validator
        ;;
    status)
        "${compose[@]}" ps taira-validator
        ;;
    logs)
        "${compose[@]}" logs --tail=200 taira-validator
        ;;
    *)
        echo "unknown command: $COMMAND_NAME" >&2
        usage >&2
        exit 1
        ;;
esac
