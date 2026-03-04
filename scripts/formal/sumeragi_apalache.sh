#!/bin/bash
set -euo pipefail

mode="${1:-fast}"
root_dir="$(cd "$(dirname "$0")/../.." && pwd)"
spec_dir="$root_dir/docs/formal/sumeragi"
spec_file="$spec_dir/Sumeragi.tla"
default_local_apalache_bin="$root_dir/target/apalache/toolchains/v0.52.2/bin/apalache-mc"
if [[ -n "${APALACHE_BIN:-}" ]]; then
  apalache_bin="$APALACHE_BIN"
elif [[ -x "$default_local_apalache_bin" ]]; then
  apalache_bin="$default_local_apalache_bin"
else
  apalache_bin="apalache-mc"
fi
apalache_docker_image="${APALACHE_DOCKER_IMAGE:-ghcr.io/apalache-mc/apalache:latest}"
allow_docker_fallback="${APALACHE_ALLOW_DOCKER:-1}"

case "$mode" in
  fast)
    cfg_file="$spec_dir/Sumeragi_fast.cfg"
    ;;
  deep)
    cfg_file="$spec_dir/Sumeragi_deep.cfg"
    ;;
  *)
    echo "usage: $0 {fast|deep}" >&2
    exit 2
    ;;
esac

if [[ ! -f "$cfg_file" ]]; then
  echo "error: missing config '$cfg_file'" >&2
  exit 2
fi

run_dir="$root_dir/target/apalache/sumeragi-$mode"
mkdir -p "$run_dir"

if [[ "$apalache_bin" == */* ]]; then
  if [[ -x "$apalache_bin" ]]; then
    "$apalache_bin" check --config="$cfg_file" --run-dir="$run_dir" "$spec_file"
    exit 0
  fi
elif command -v "$apalache_bin" >/dev/null 2>&1; then
  "$apalache_bin" check --config="$cfg_file" --run-dir="$run_dir" "$spec_file"
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

  docker run --rm \
    --user "$(id -u):$(id -g)" \
    --volume "$root_dir:/work" \
    --workdir /work \
    "$apalache_docker_image" \
    apalache-mc check --config="$cfg_rel" --run-dir="$run_rel" "$spec_rel"
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
