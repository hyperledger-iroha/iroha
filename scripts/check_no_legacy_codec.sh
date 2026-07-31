#!/usr/bin/env bash
set -euo pipefail

for required_tool in git find; do
  if ! command -v "$required_tool" >/dev/null 2>&1; then
    echo "retired-codec guard requires '$required_tool', but it is not available" >&2
    exit 2
  fi
done

if command -v rg >/dev/null 2>&1; then
  search_tool="rg"
elif command -v grep >/dev/null 2>&1; then
  search_tool="grep"
else
  echo "retired-codec guard requires either ripgrep (rg) or grep, but neither is available" >&2
  exit 2
fi

search_file() {
  local pattern="$1"
  local path="$2"
  local status

  if [[ "$search_tool" == "rg" ]]; then
    if rg -q -- "$pattern" "$path"; then
      return 0
    else
      status=$?
    fi
  else
    if grep -Eq -- "$pattern" "$path"; then
      return 0
    else
      status=$?
    fi
  fi

  if [[ $status -eq 1 ]]; then
    return 1
  fi

  echo "retired-codec scanner failed ($search_tool exit $status) while scanning: $path" >&2
  exit 2
}

matching_sources=()
collect_source_matches() {
  local pattern="$1"
  local base="$2"
  local output
  local status

  matching_sources=()
  if [[ "$search_tool" == "rg" ]]; then
    if output="$(rg -l --glob '*.rs' -- "$pattern" "$base")"; then
      status=0
    else
      status=$?
    fi
  else
    if output="$(grep -rEl --include='*.rs' -- "$pattern" "$base")"; then
      status=0
    else
      status=$?
    fi
  fi

  if [[ $status -eq 1 ]]; then
    return 0
  fi
  if [[ $status -ne 0 ]]; then
    echo "retired-codec scanner failed ($search_tool exit $status) while scanning: $base" >&2
    exit 2
  fi

  while IFS= read -r source; do
    [[ -z "$source" ]] && continue
    matching_sources+=("$source")
  done <<< "$output"
}

ROOT="$(git rev-parse --show-toplevel)"
violations=()
retired_codec_pattern='parity[-_]'"scale"
retired_native_amx_v1_pattern='NativeAmxAttestationBodyV1|NativeAmxAttestationQcV1|struct[[:space:]]+NativeAmxLegRecord[[:space:]]*[{]|impl_decode_from_slice_via_codec![(]NativeAmxLegRecord[)]|iroha:native-amx:v1'
retired_lane_handoff_pattern='LaneExecutablePayloadHandoff|LANE_EXECUTABLE_PAYLOAD_HANDOFF_VERSION(_V[[:digit:]]+)?|nexus:lane-executable-payload-handoff:v[[:digit:]]+'
retired_pk2_multilane_compatibility_pattern='LegacyExecutionContextLane(PayloadOwnership|RbcInstance|BlockDescriptor)Preimage|validate_legacy_pk2_lane_payload_replay_material|pk2_staging_lane_payload_subject_hash_compatibility|pk2_staging_legacy_replay_execution_context_hash_mismatch|allow_missing_legacy_context|accepting PK2 staging (lane payload ownership|legacy execution context hash mismatch)'
if search_file "$retired_codec_pattern" "$ROOT/Cargo.toml"; then
  violations+=("$ROOT/Cargo.toml")
fi

for dir in crates integration_tests tools xtask python fuzz; do
  base="$ROOT/$dir"
  [[ -d "$base" ]] || continue
  while IFS= read -r -d '' manifest; do
    [[ -z "$manifest" ]] && continue
    if search_file "$retired_codec_pattern" "$manifest"; then
      violations+=("$manifest")
    fi
  done < <(find "$base" -type f -name Cargo.toml -print0)
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
  collect_source_matches "$retired_native_amx_v1_pattern" "$base"
  if [[ ${#matching_sources[@]} -ne 0 ]]; then
    native_amx_v1_violations+=("${matching_sources[@]}")
  fi
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
  collect_source_matches "$retired_lane_handoff_pattern" "$base"
  if [[ ${#matching_sources[@]} -ne 0 ]]; then
    lane_handoff_violations+=("${matching_sources[@]}")
  fi
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
  collect_source_matches "$retired_pk2_multilane_compatibility_pattern" "$base"
  if [[ ${#matching_sources[@]} -ne 0 ]]; then
    pk2_multilane_compatibility_violations+=("${matching_sources[@]}")
  fi
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
