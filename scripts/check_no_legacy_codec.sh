#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
violations=()
retired_codec_pattern='parity[-_]'"scale"
retired_native_amx_v1_pattern='NativeAmxAttestationBodyV1|NativeAmxAttestationQcV1|struct[[:space:]]+NativeAmxLegRecord[[:space:]]*[{]|impl_decode_from_slice_via_codec![(]NativeAmxLegRecord[)]|iroha:native-amx:v1'
retired_lane_handoff_pattern='LaneExecutablePayloadHandoff|LANE_EXECUTABLE_PAYLOAD_HANDOFF_VERSION(_V[[:digit:]]+)?|nexus:lane-executable-payload-handoff:v[[:digit:]]+'
retired_pk2_multilane_compatibility_pattern='LegacyExecutionContextLane(PayloadOwnership|RbcInstance|BlockDescriptor)Preimage|validate_legacy_pk2_lane_payload_replay_material|pk2_staging_lane_payload_subject_hash_compatibility|pk2_staging_legacy_replay_execution_context_hash_mismatch|allow_missing_legacy_context|accepting PK2 staging (lane payload ownership|legacy execution context hash mismatch)'
if rg -q "$retired_codec_pattern" "$ROOT/Cargo.toml"; then
  violations+=("$ROOT/Cargo.toml")
fi

for dir in crates integration_tests tools xtask python fuzz; do
  base="$ROOT/$dir"
  [[ -d "$base" ]] || continue
  while IFS= read -r manifest; do
    [[ -z "$manifest" ]] && continue
    if rg -q "$retired_codec_pattern" "$manifest"; then
      violations+=("$manifest")
    fi
  done < <(find "$base" -name Cargo.toml)
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
  while IFS= read -r source; do
    [[ -z "$source" ]] && continue
    native_amx_v1_violations+=("$source")
  done < <(rg -l --glob '*.rs' "$retired_native_amx_v1_pattern" "$base" || true)
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
  while IFS= read -r source; do
    [[ -z "$source" ]] && continue
    lane_handoff_violations+=("$source")
  done < <(rg -l --glob '*.rs' "$retired_lane_handoff_pattern" "$base" || true)
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
  while IFS= read -r source; do
    [[ -z "$source" ]] && continue
    pk2_multilane_compatibility_violations+=("$source")
  done < <(rg -l --glob '*.rs' "$retired_pk2_multilane_compatibility_pattern" "$base" || true)
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
