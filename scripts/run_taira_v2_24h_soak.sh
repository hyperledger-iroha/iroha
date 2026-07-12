#!/usr/bin/env bash
set -euo pipefail

readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

duration_secs="${IROHA_TAIRA_SIM_DURATION_SECS:-86400}"
packet_loss_percent="${IROHA_TAIRA_PACKET_LOSS_PERCENT:-10}"
churn_interval_secs="${IROHA_TAIRA_CHURN_INTERVAL_SECS:-300}"

usage() {
  cat <<'USAGE'
Usage: scripts/run_taira_v2_24h_soak.sh [options]

Run the ignored four-validator Taira-profile Sumeragi v2 soak. The release
profile defaults to 24 hours, 10% deterministic inbound/outbound packet loss,
5 TPS, and a scheduled single-validator restart every five minutes.

Options:
  --duration-secs N          Runtime in seconds (default: 86400)
  --packet-loss-percent N    Inbound and outbound loss, 0..100 (default: 10)
  --churn-interval-secs N    Validator restart cadence (default: 300)
  --help                     Show this help
USAGE
}

while (($# > 0)); do
  case "$1" in
    --duration-secs)
      duration_secs="${2:?--duration-secs requires a value}"
      shift 2
      ;;
    --packet-loss-percent)
      packet_loss_percent="${2:?--packet-loss-percent requires a value}"
      shift 2
      ;;
    --churn-interval-secs)
      churn_interval_secs="${2:?--churn-interval-secs requires a value}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[[ "$duration_secs" =~ ^[0-9]+$ ]] && ((duration_secs >= 30)) || {
  echo "duration must be an integer of at least 30 seconds" >&2
  exit 2
}
[[ "$packet_loss_percent" =~ ^[0-9]+$ ]] && ((packet_loss_percent <= 100)) || {
  echo "packet loss must be an integer from 0 through 100" >&2
  exit 2
}
[[ "$churn_interval_secs" =~ ^[0-9]+$ ]] && ((churn_interval_secs >= 30)) || {
  echo "churn interval must be an integer of at least 30 seconds" >&2
  exit 2
}

export IROHA_TAIRA_SIM_DURATION_SECS="$duration_secs"
export IROHA_TAIRA_PACKET_LOSS_PERCENT="$packet_loss_percent"
export IROHA_TAIRA_CHURN_INTERVAL_SECS="$churn_interval_secs"

cd "$REPO_ROOT"
exec cargo test --locked -p integration_tests --test consensus_and_da \
  'taira_public_localnet::taira_profile_24h_packet_impairment_and_restart_soak' \
  -- --ignored --nocapture
