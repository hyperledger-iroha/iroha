#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
violations=()
retired_codec_pattern='parity[-_]'"scale"
retired_native_amx_v1_pattern='NativeAmxAttestationBodyV1|NativeAmxAttestationQcV1|struct[[:space:]]+NativeAmxLegRecord[[:space:]]*[{]|impl_decode_from_slice_via_codec![(]NativeAmxLegRecord[)]|iroha:native-amx:v1'
retired_lane_handoff_pattern='LaneExecutablePayloadHandoff|LANE_EXECUTABLE_PAYLOAD_HANDOFF_VERSION(_V[[:digit:]]+)?|nexus:lane-executable-payload-handoff:v[[:digit:]]+'
retired_pk2_multilane_compatibility_pattern='LegacyExecutionContextLane(PayloadOwnership|RbcInstance|BlockDescriptor)Preimage|validate_legacy_pk2_lane_payload_replay_material|pk2_staging_lane_payload_subject_hash_compatibility|pk2_staging_legacy_replay_execution_context_hash_mismatch|allow_missing_legacy_context|PK2 staging'

if command -v rg >/dev/null 2>&1; then
  search_backend=rg
else
  search_backend=grep
fi

search_file() {
  local pattern="$1"
  local file="$2"
  if [[ "$search_backend" == rg ]]; then
    rg -q -- "$pattern" "$file"
  else
    LC_ALL=C grep -Eq -- "$pattern" "$file"
  fi
}

list_matching_rust_sources() {
  local pattern="$1"
  local base="$2"
  local matches
  local status

  if [[ "$search_backend" == rg ]]; then
    if matches="$(rg -l --glob '*.rs' -- "$pattern" "$base")"; then
      [[ -z "$matches" ]] || printf '%s\n' "$matches"
      return 0
    else
      status=$?
      [[ $status -eq 1 ]] && return 0
      return "$status"
    fi
  fi

  if matches="$(LC_ALL=C grep -rEIl --include='*.rs' -- "$pattern" "$base")"; then
    [[ -z "$matches" ]] || printf '%s\n' "$matches"
    return 0
  else
    status=$?
    [[ $status -eq 1 ]] && return 0
    return "$status"
  fi
}

if search_file "$retired_codec_pattern" "$ROOT/Cargo.toml"; then
  violations+=("$ROOT/Cargo.toml")
elif [[ $? -ne 1 ]]; then
  echo "failed to inspect $ROOT/Cargo.toml for retired codecs" >&2
  exit 2
fi

for dir in crates integration_tests tools xtask python fuzz; do
  base="$ROOT/$dir"
  [[ -d "$base" ]] || continue
  if ! manifests="$(find "$base" -type f -name Cargo.toml -print)"; then
    echo "failed to enumerate Cargo manifests below $base" >&2
    exit 2
  fi
  while IFS= read -r manifest; do
    [[ -z "$manifest" ]] && continue
    if search_file "$retired_codec_pattern" "$manifest"; then
      violations+=("$manifest")
    elif [[ $? -ne 1 ]]; then
      echo "failed to inspect $manifest for retired codecs" >&2
      exit 2
    fi
  done <<< "$manifests"
done

if [[ ${#violations[@]} -ne 0 ]]; then
  echo "retired codec dependency detected in:" >&2
  printf '  %s\n' "${violations[@]}" >&2
  exit 1
fi

native_amx_v1_violations=()
for dir in crates integration_tests; do
  base="$ROOT/$dir"
  [[ -d "$base" ]] || continue
  if ! matches="$(list_matching_rust_sources "$retired_native_amx_v1_pattern" "$base")"; then
    echo "failed to inspect $base for retired Native AMX V1 codecs" >&2
    exit 2
  fi
  while IFS= read -r source; do
    [[ -z "$source" ]] && continue
    native_amx_v1_violations+=("$source")
  done <<< "$matches"
done

if [[ ${#native_amx_v1_violations[@]} -ne 0 ]]; then
  echo "retired Native AMX V1 consensus codec detected in:" >&2
  printf '  %s\n' "${native_amx_v1_violations[@]}" >&2
  exit 1
fi

lane_handoff_violations=()
for dir in crates integration_tests; do
  base="$ROOT/$dir"
  [[ -d "$base" ]] || continue
  if ! matches="$(list_matching_rust_sources "$retired_lane_handoff_pattern" "$base")"; then
    echo "failed to inspect $base for retired lane executable payload handoff codecs" >&2
    exit 2
  fi
  while IFS= read -r source; do
    [[ -z "$source" ]] && continue
    lane_handoff_violations+=("$source")
  done <<< "$matches"
done

if [[ ${#lane_handoff_violations[@]} -ne 0 ]]; then
  echo "retired lane executable payload handoff codec detected in:" >&2
  printf '  %s\n' "${lane_handoff_violations[@]}" >&2
  exit 1
fi

pk2_multilane_compatibility_violations=()
for dir in crates integration_tests; do
  base="$ROOT/$dir"
  [[ -d "$base" ]] || continue
  if ! matches="$(list_matching_rust_sources "$retired_pk2_multilane_compatibility_pattern" "$base")"; then
    echo "failed to inspect $base for retired PK2 multilane compatibility paths" >&2
    exit 2
  fi
  while IFS= read -r source; do
    [[ -z "$source" ]] && continue
    pk2_multilane_compatibility_violations+=("$source")
  done <<< "$matches"
done

if [[ ${#pk2_multilane_compatibility_violations[@]} -ne 0 ]]; then
  echo "retired PK2 multilane compatibility path detected in:" >&2
  printf '  %s\n' "${pk2_multilane_compatibility_violations[@]}" >&2
  exit 1
fi

echo "No retired codec dependencies found."
echo "No retired Native AMX V1 consensus codecs found."
echo "No retired lane executable payload handoff codecs found."
echo "No retired PK2 multilane compatibility paths found."
