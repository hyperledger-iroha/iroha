#!/usr/bin/env bash
set -euo pipefail

line="i3"
declare -a profile_flag=()
irohad_default_features="telemetry schema-endpoint"
irohad_features_i3="${irohad_default_features} build-i3"
irohad_features_i2="${irohad_default_features} build-i2"
iroha_cli_features_i3="build-i3"
iroha_cli_features_i2="build-i2"

if [[ -n "${BUILD_PROFILE:-}" ]]; then
  profile_flag=(--profile "${BUILD_PROFILE}")
fi

while [[ $# -gt 0 ]]; do
  case "$1" in
    --i2|--line=i2|i2|-2)
      line="i2"
      shift
      ;;
    --i3|--line=i3|i3|-3)
      line="i3"
      shift
      ;;
    --line)
      line="${2:-}"
      shift 2
      ;;
    *)
      echo "Usage: $0 [--i2|--i3|--line {i2|i3}]" >&2
      exit 1
      ;;
  esac
done

if [[ "${line}" == "i2" ]]; then
  echo "Building Iroha 2 binaries (iroha2, iroha2d)..."
  if [[ ${#profile_flag[@]} -gt 0 ]]; then
    cargo build "${profile_flag[@]}" -p irohad --no-default-features --features "${irohad_features_i2}" --bin iroha2d
    cargo build "${profile_flag[@]}" -p iroha_cli --no-default-features --features "${iroha_cli_features_i2}" --bin iroha2
  else
    cargo build -p irohad --no-default-features --features "${irohad_features_i2}" --bin iroha2d
    cargo build -p iroha_cli --no-default-features --features "${iroha_cli_features_i2}" --bin iroha2
  fi
else
  echo "Building Iroha 3 binaries (iroha3, iroha3d)..."
  if [[ ${#profile_flag[@]} -gt 0 ]]; then
    cargo build "${profile_flag[@]}" -p irohad --features "${irohad_features_i3}" --bin iroha3d
    cargo build "${profile_flag[@]}" -p iroha_cli --features "${iroha_cli_features_i3}" --bin iroha3
  else
    cargo build -p irohad --features "${irohad_features_i3}" --bin iroha3d
    cargo build -p iroha_cli --features "${iroha_cli_features_i3}" --bin iroha3
  fi
fi
