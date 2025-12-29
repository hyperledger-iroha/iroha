#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${repo_root}"
echo "[swift-fixtures] verifying Swift fixture parity"
state_file="${SWIFT_FIXTURE_STATE_FILE:-artifacts/swift_fixture_regen_state.json}"
cadence_env="${SWIFT_FIXTURE_EXPECTED_CADENCE:-weekly-wed-1700utc}"
IFS=',' read -r -a _cadence_array <<< "${cadence_env}"
cadence_args=()
for entry in "${_cadence_array[@]}"; do
  trimmed="${entry#"${entry%%[![:space:]]*}"}"
  trimmed="${trimmed%"${trimmed##*[![:space:]]}"}"
  if [[ -n "${trimmed}" ]]; then
    cadence_args+=(--cadence-label "${trimmed}")
  fi
done
python3 scripts/check_swift_fixtures.py \
  --source "${SWIFT_FIXTURE_SOURCE:-java/iroha_android/src/test/resources}" \
  --target "${SWIFT_FIXTURE_OUT:-IrohaSwift/Fixtures}" \
  --quiet \
  --state \
  --state-file "${state_file}" \
  --max-age-hours "${SWIFT_FIXTURE_MAX_AGE_HOURS:-48}" \
  --odd-week-owner "${SWIFT_FIXTURE_ODD_WEEK_OWNER:-android-foundations}" \
  --even-week-owner "${SWIFT_FIXTURE_EVEN_WEEK_OWNER:-swift-lead}" \
  --schedule-tolerance-hours "${SWIFT_FIXTURE_SCHEDULE_TOLERANCE_HOURS:-6}" \
  "${cadence_args[@]}"
echo "[swift-fixtures] parity confirmed"
