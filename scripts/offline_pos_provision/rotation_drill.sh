#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
usage: rotation_drill.sh --spec <path> --output <dir> [--operator-key <ed25519:...>] [--notes <string>] [cargo xtask args...]

Runs `cargo xtask offline-pos-provision` and appends a signed-off drill entry
to <output>/rotation_drill.log so regulators can trace rehearsal runs.
EOF
}

if [[ $# -lt 2 ]]; then
  usage
  exit 1
fi

OUTPUT_DIR=""
NOTES=""
FORWARD_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output)
      OUTPUT_DIR="$2"
      FORWARD_ARGS+=("$1" "$2")
      shift 2
      ;;
    --notes)
      NOTES="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      FORWARD_ARGS+=("$1")
      shift
      ;;
  esac
done

if [[ -z "$OUTPUT_DIR" ]]; then
  echo "rotation_drill.sh requires --output <dir>" >&2
  exit 1
fi

cargo xtask offline-pos-provision "${FORWARD_ARGS[@]}"

LOG_FILE="${OUTPUT_DIR%/}/rotation_drill.log"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
FIXTURE_PATH="${OUTPUT_DIR%/}/pos_provision_fixtures.manifest.json"

{
  echo "[$TIMESTAMP] rehearsal complete"
  if [[ -n "$NOTES" ]]; then
    echo "notes: $NOTES"
  fi
  echo "fixtures: $FIXTURE_PATH"
} >> "$LOG_FILE"

echo "OA12 rotation drill recorded at $LOG_FILE"
