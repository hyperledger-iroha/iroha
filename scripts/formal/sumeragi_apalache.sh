#!/bin/bash
set -euo pipefail

mode="${1:-fast}"
root_dir="$(cd "$(dirname "$0")/../.." && pwd)"
spec_dir="$root_dir/docs/formal/sumeragi"
apalache_version="${APALACHE_VERSION:-0.52.2}"
default_local_apalache_bin="$root_dir/target/apalache/toolchains/v${apalache_version}/bin/apalache-mc"
if [[ -n "${APALACHE_BIN:-}" ]]; then
  apalache_bin="$APALACHE_BIN"
elif [[ -x "$default_local_apalache_bin" ]]; then
  apalache_bin="$default_local_apalache_bin"
else
  apalache_bin="apalache-mc"
fi
apalache_docker_image="${APALACHE_DOCKER_IMAGE:-ghcr.io/apalache-mc/apalache:${apalache_version}}"
allow_docker_fallback="${APALACHE_ALLOW_DOCKER:-1}"
expect_failure=0

case "$mode" in
  fast)
    spec_file="$spec_dir/Sumeragi.tla"
    cfg_file="$spec_dir/Sumeragi_fast.cfg"
    apalache_length=10
    ;;
  deep)
    spec_file="$spec_dir/Sumeragi.tla"
    cfg_file="$spec_dir/Sumeragi_deep.cfg"
    apalache_length=10
    ;;
  frontier-fast)
    spec_file="$spec_dir/SumeragiFrontierRecovery.tla"
    cfg_file="$spec_dir/SumeragiFrontierRecovery_fast.cfg"
    apalache_length=10
    ;;
  frontier-deep)
    spec_file="$spec_dir/SumeragiFrontierRecovery.tla"
    cfg_file="$spec_dir/SumeragiFrontierRecovery_deep.cfg"
    apalache_length=12
    ;;
  frontier-wide)
    spec_file="$spec_dir/SumeragiFrontierRecovery.tla"
    cfg_file="$spec_dir/SumeragiFrontierRecovery_wide.cfg"
    apalache_length=14
    ;;
  frontier-bug-stale-owner)
    spec_file="$spec_dir/SumeragiFrontierRecovery.tla"
    cfg_file="$spec_dir/SumeragiFrontierRecovery_bug_stale_owner.cfg"
    apalache_length=4
    expect_failure=1
    ;;
  frontier-bug-vote-queue)
    spec_file="$spec_dir/SumeragiFrontierRecovery.tla"
    cfg_file="$spec_dir/SumeragiFrontierRecovery_bug_vote_queue.cfg"
    apalache_length=4
    expect_failure=1
    ;;
  *)
    echo "usage: $0 {fast|deep|frontier-fast|frontier-deep|frontier-wide|frontier-bug-stale-owner|frontier-bug-vote-queue}" >&2
    exit 2
    ;;
esac

apalache_length="${APALACHE_LENGTH:-$apalache_length}"

if [[ ! -f "$cfg_file" ]]; then
  echo "error: missing config '$cfg_file'" >&2
  exit 2
fi

run_dir="$root_dir/target/apalache/sumeragi-$mode"
out_dir="$root_dir/target/apalache/out"
mkdir -p "$run_dir" "$out_dir"

run_with_expected_status() {
  if [[ "$expect_failure" == "1" ]]; then
    if "$@"; then
      echo "error: expected Apalache to reject '$mode', but it passed" >&2
      return 1
    fi
    echo "[formal] expected Apalache rejection observed for '$mode'"
    return 0
  fi

  "$@"
}

if [[ "$apalache_bin" == */* ]]; then
  if [[ -x "$apalache_bin" ]]; then
    run_with_expected_status "$apalache_bin" --out-dir="$out_dir" check --length="$apalache_length" --config="$cfg_file" --run-dir="$run_dir" "$spec_file"
    exit 0
  fi
elif command -v "$apalache_bin" >/dev/null 2>&1; then
  run_with_expected_status "$apalache_bin" --out-dir="$out_dir" check --length="$apalache_length" --config="$cfg_file" --run-dir="$run_dir" "$spec_file"
  exit 0
fi

docker_daemon_available=0
if command -v docker >/dev/null 2>&1; then
  if docker info >/dev/null 2>&1; then
    docker_daemon_available=1
  elif [[ "$allow_docker_fallback" != "0" ]]; then
    echo "warning: docker is installed but the daemon is unavailable; skipping docker fallback" >&2
  fi
fi

if [[ "$allow_docker_fallback" != "0" ]] && [[ "$docker_daemon_available" == "1" ]]; then
  cfg_rel="docs/formal/sumeragi/$(basename "$cfg_file")"
  spec_rel="docs/formal/sumeragi/$(basename "$spec_file")"
  run_rel="target/apalache/sumeragi-$mode"
  out_rel="target/apalache/out"

  run_with_expected_status docker run --rm \
    --user "$(id -u):$(id -g)" \
    --volume "$root_dir:/work" \
    --workdir /work \
    "$apalache_docker_image" \
    apalache-mc --out-dir="$out_rel" check --length="$apalache_length" --config="$cfg_rel" --run-dir="$run_rel" "$spec_rel"
  exit 0
fi

echo "error: '$apalache_bin' not found; install Apalache or set APALACHE_BIN" >&2
if [[ "$allow_docker_fallback" != "0" ]]; then
  if command -v docker >/dev/null 2>&1 && [[ "$docker_daemon_available" != "1" ]]; then
    echo "hint: start Docker daemon and rerun, or set APALACHE_ALLOW_DOCKER=0" >&2
  else
    echo "hint: install Docker and use APALACHE_DOCKER_IMAGE=${apalache_docker_image} fallback" >&2
  fi
fi
exit 127
