#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Build FASTPQ-enabled binaries inside a pinned container for reproducibility.
# The script prepares the toolchain image, runs the build, and emits a manifest
# capturing hashes and compiler versions.

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workspace_root="$(cd "${script_dir}/../.." && pwd)"

usage() {
  cat <<'EOF'
Usage: scripts/fastpq/repro_build.sh [OPTIONS]

Options:
  --mode <cpu|gpu>        Select the build container (default: cpu).
  --profile <name>        Cargo profile to build (default: release).
  --bins <list>           Comma-separated binary list to build
                          (default: irohad,iroha_cli).
  --features <list>       Additional cargo features (comma-separated) to append.
  --output <dir>          Output directory for artefacts & manifest
                          (default: artifacts/fastpq-repro).
  --image-tag <name>      Override the docker image tag to build/use.
  --container-runtime <r> Override the container runtime (docker/podman/nerdctl).
  --skip-build-image      Skip rebuilding the container image.
  -h, --help              Show this message.

Environment overrides:
  FASTPQ_RUST_IMAGE       Base rust image (default: rust:1.88.0-slim-bookworm).
  FASTPQ_CUDA_IMAGE       Base CUDA image for GPU builds
                          (default: nvidia/cuda:12.2.2-devel-ubuntu22.04).
  FASTPQ_RUST_TOOLCHAIN   Rust toolchain version (default: 1.88.0).
  FASTPQ_CONTAINER_RUNTIME Container runtime to use (default: auto detect).
  FASTPQ_CONTAINER_RUNTIME_FALLBACKS
                          Preferred runtimes when auto-detecting (default:
                          docker,podman,nerdctl).
EOF
}

mode="cpu"
profile="release"
bins_csv="irohad,iroha_cli"
extra_features=""
output_dir="artifacts/fastpq-repro"
image_tag=""
skip_build_image="false"
container_runtime_cli=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)
      mode="$2"
      shift 2
      ;;
    --profile)
      profile="$2"
      shift 2
      ;;
    --bins)
      bins_csv="$2"
      shift 2
      ;;
    --features)
      extra_features="$2"
      shift 2
      ;;
    --output)
      output_dir="$2"
      shift 2
      ;;
    --image-tag)
      image_tag="$2"
      shift 2
      ;;
    --container-runtime)
      container_runtime_cli="$2"
      shift 2
      ;;
    --skip-build-image)
      skip_build_image="true"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      break
      ;;
    *)
      echo "error: unknown option '$1'" >&2
      echo >&2
      usage >&2
      exit 1
      ;;
  esac
done

case "${mode}" in
  cpu|gpu) ;;
  *)
    echo "error: --mode must be 'cpu' or 'gpu' (got '${mode}')" >&2
    exit 1
    ;;
esac

IFS=',' read -r -a bins <<< "${bins_csv}"
if [[ ${#bins[@]} -eq 0 ]]; then
  echo "error: --bins list cannot be empty" >&2
  exit 1
fi

trim_whitespace() {
  local value="$1"
  # shellcheck disable=SC2001
  value="$(echo "${value}" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
  printf '%s' "${value}"
}

select_container_runtime() {
  local requested="$1"
  shift
  local -a fallbacks=("$@")

  if [[ -n "${requested}" && "${requested}" != "auto" ]]; then
    if command -v "${requested}" >/dev/null 2>&1; then
      printf '%s' "${requested}"
      return 0
    fi
    printf "error: container runtime '%s' not found in PATH\n" "${requested}" >&2
    printf "       install it or set FASTPQ_CONTAINER_RUNTIME to a valid binary.\n" >&2
    return 1
  fi

  for candidate in "${fallbacks[@]}"; do
    local trimmed
    trimmed="$(trim_whitespace "${candidate}")"
    if [[ -z "${trimmed}" ]]; then
      continue
    fi
    if command -v "${trimmed}" >/dev/null 2>&1; then
      printf '%s' "${trimmed}"
      return 0
    fi
  done

  local joined
  joined="$(IFS=', '; printf '%s' "${fallbacks[*]}")"
  printf "error: unable to find a container runtime in PATH (checked: %s)\n" "${joined}" >&2
  printf "       install docker/podman/nerdctl or override FASTPQ_CONTAINER_RUNTIME.\n" >&2
  return 1
}

rust_image="${FASTPQ_RUST_IMAGE:-rust:1.88.0-slim-bookworm}"
cuda_image="${FASTPQ_CUDA_IMAGE:-nvidia/cuda:12.2.2-devel-ubuntu22.04}"
rust_toolchain="${FASTPQ_RUST_TOOLCHAIN:-1.88.0}"

runtime_preference="${container_runtime_cli:-${FASTPQ_CONTAINER_RUNTIME:-auto}}"
fallbacks_csv="${FASTPQ_CONTAINER_RUNTIME_FALLBACKS:-docker,podman,nerdctl}"
IFS=',' read -r -a runtime_fallbacks <<< "${fallbacks_csv}"
container_runtime="$(select_container_runtime "${runtime_preference}" "${runtime_fallbacks[@]}")" || exit 1

if [[ "${runtime_preference}" == "auto" || -z "${runtime_preference}" ]]; then
  echo "==> Using container runtime: ${container_runtime} (auto-detected)"
else
  echo "==> Using container runtime: ${container_runtime}"
fi

dockerfile="${workspace_root}/scripts/fastpq/docker/Dockerfile.${mode}"
if [[ ! -f "${dockerfile}" ]]; then
  echo "error: missing dockerfile at ${dockerfile}" >&2
  exit 1
fi

if [[ -z "${image_tag}" ]]; then
  image_tag="fastpq-builder:${mode}-${rust_toolchain}"
fi

if [[ "${skip_build_image}" != "true" ]]; then
  build_args=(
    --build-arg "RUST_IMAGE=${rust_image}"
    --build-arg "RUST_TOOLCHAIN=${rust_toolchain}"
    -f "${dockerfile}"
    -t "${image_tag}"
    "${workspace_root}"
  )
  if [[ "${mode}" == "gpu" ]]; then
    build_args=(--build-arg "CUDA_IMAGE=${cuda_image}" "${build_args[@]}")
  fi
  echo "==> Building container ${image_tag} (mode=${mode})"
  "${container_runtime}" build "${build_args[@]}"
else
  echo "==> Skipping container build for ${image_tag}"
fi

features_list=()
if [[ "${mode}" == "gpu" ]]; then
  features_list+=("fastpq_prover/fastpq-gpu")
fi
if [[ -n "${extra_features}" ]]; then
  IFS=',' read -r -a extra_array <<< "${extra_features}"
  for item in "${extra_array[@]}"; do
    trimmed="$(echo "${item}" | xargs)"
    if [[ -n "${trimmed}" ]]; then
      features_list+=("${trimmed}")
    fi
  done
fi

bin_list_csv=$(IFS=','; echo "${bins[*]}")
features_arg=$(IFS=','; echo "${features_list[*]}")

uid_gid="$(id -u):$(id -g)"
workspace_mount="${workspace_root}:/workspace"

docker_run_args=(
  --rm
  --user "${uid_gid}"
  -v "${workspace_mount}"
  -w /workspace
  -e "FASTPQ_PROFILE=${profile}"
  -e "FASTPQ_BINS=${bin_list_csv}"
  -e "FASTPQ_FEATURES=${features_arg}"
  -e "FASTPQ_OUTPUT_DIR=${output_dir}"
  -e "FASTPQ_RUST_IMAGE=${rust_image}"
  -e "FASTPQ_RUST_TOOLCHAIN=${rust_toolchain}"
)

if [[ "${mode}" == "gpu" ]]; then
  if [[ "${container_runtime}" == "docker" ]]; then
    docker_run_args+=(--gpus all)
  else
    echo "warning: GPU mode requested; configure GPU passthrough for '${container_runtime}' manually." >&2
  fi
  docker_run_args+=(-e "FASTPQ_CUDA_IMAGE=${cuda_image}")
fi

inside_script="/workspace/scripts/fastpq/run_inside_repro_build.sh"
if [[ ! -f "${workspace_root}/scripts/fastpq/run_inside_repro_build.sh" ]]; then
  echo "error: expected helper script at scripts/fastpq/run_inside_repro_build.sh" >&2
  exit 1
fi

echo "==> Running reproducible build (mode=${mode}, profile=${profile})"
"${container_runtime}" run "${docker_run_args[@]}" "${image_tag}" "${inside_script}"
