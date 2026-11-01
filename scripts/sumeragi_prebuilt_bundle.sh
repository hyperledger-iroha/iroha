#!/usr/bin/env bash
# Draft thin adapter for the reviewed structured release operation.
# Entry and bridge authority are explicit independent arguments; an environment
# path to a helper executable is never accepted as publication authority.

_sumeragi_native_bundle_call() {
  local command="$1" repo="$2" source_digest="$3"
  shift 3
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:?native policy Python is required}"
  local cargo_target="${CARGO_TARGET_DIR:?private Cargo root is required}"
  local artifact_root="${IROHA_RELEASE_ARTIFACT_ROOT:?private artifact root is required}"
  local cargo_home="${CARGO_HOME:?explicit Cargo home is required}"
  local cargo_bin="${IROHA_RELEASE_CARGO_BIN:?pinned Cargo is required}"
  local rustc_bin="${IROHA_RELEASE_RUSTC_BIN:?pinned rustc is required}"
  local cancellation="${IROHA_RELEASE_CANCEL_REQUEST_PATH:?private cancellation path is required}"
  local pipeline="${repo}/scripts/run_release_pipeline.py"
  local bundle="${IROHA_TEST_TARGET_DIR:-}" manifest="${IROHA_RELEASE_PREBUILT_MANIFEST_SHA256:-}"
  local host_os native_windows=false
  host_os="$("$policy_python" -B -I -S -c 'import os; print(os.name)')" || return $?
  case "$(uname -s)" in
    MINGW*|MSYS*)
      [[ "$host_os" == nt ]] || { echo 'Git Bash release publishing requires native Windows Python' >&2; return 2; }
      native_windows=true
      repo="$(cygpath -aw -- "$repo")" || return $?
      pipeline="$(cygpath -aw -- "$pipeline")" || return $?
      cargo_target="$(cygpath -aw -- "$cargo_target")" || return $?
      artifact_root="$(cygpath -aw -- "$artifact_root")" || return $?
      cargo_home="$(cygpath -aw -- "$cargo_home")" || return $?
      cargo_bin="$(cygpath -aw -- "$cargo_bin")" || return $?
      rustc_bin="$(cygpath -aw -- "$rustc_bin")" || return $?
      cancellation="$(cygpath -aw -- "$cancellation")" || return $?
      if [[ -n "$manifest" && -n "$bundle" ]]; then bundle="$(cygpath -aw -- "$bundle")" || return $?; fi
      ;;
    *) [[ "$host_os" == posix ]] || { echo 'release Python does not match the native shell host' >&2; return 2; } ;;
  esac
  local -a arguments=("$pipeline" prepare-sumeragi-programs --repo-root "$repo"
    --trusted-workspace-source-sha256 "$source_digest" --cargo-target-dir "$cargo_target"
    --artifact-root "$artifact_root" --cargo-home "$cargo_home" --cargo-bin "$cargo_bin"
    --rustc-bin "$rustc_bin" --cancel-request "$cancellation")
  local flag value seen=' '
  while (( $# )); do
    [[ $# -ge 2 ]] || { echo 'independent release option has no value' >&2; return 2; }
    flag="$1" value="$2"
    shift 2
    case "$flag" in
      --entry-root|--bridge-root)
        if [[ "$native_windows" == true ]]; then value="$(cygpath -aw -- "$value")" || return $?; fi ;;
      --trusted-entry-sha256|--trusted-entry-provenance-sha256|--trusted-bridge-sha256|--trusted-bridge-provenance-sha256|--native-target|--name) ;;
      *) echo 'unknown independent release option' >&2; return 2 ;;
    esac
    case "$seen" in *" $flag "*) echo 'repeated independent release option' >&2; return 2 ;; esac
    seen+="$flag "
    arguments+=("$flag" "$value")
  done
  if [[ -n "$manifest" ]]; then
    [[ -n "$bundle" ]] || { echo 'inherited manifest is missing its bundle' >&2; return 2; }
    arguments+=(--inherited-bundle "$bundle" --inherited-manifest-sha256 "$manifest")
  elif [[ "$command" == admit ]]; then
    echo 'bundle admission requires inherited independent anchors' >&2
    return 2
  fi
  MSYS2_ARG_CONV_EXCL='*' "$policy_python" -B -I -S "${arguments[@]}"
}

sumeragi_localnet_binary_attestation_valid() {
  _sumeragi_native_bundle_call admit "$@" >/dev/null
}

sumeragi_ensure_source_bound_localnet_binaries() {
  if [[ -z "${IROHA_RELEASE_PREBUILT_MANIFEST_SHA256:-}" ]]; then unset IROHA_TEST_TARGET_DIR; fi
  local output
  if output="$(_sumeragi_native_bundle_call build-publish "$@")"; then :; else return $?; fi
  local bundle_line manifest_line
  IFS= read -r bundle_line <<<"$output"
  manifest_line="${output#*$'\n'}"
  [[ "$output" == *$'\n'* && "$manifest_line" != *$'\n'* \
    && "$bundle_line" == $'bundle_dir\t'* && "$manifest_line" == $'manifest_sha256\t'* ]] || {
    echo 'malformed native publication acknowledgement' >&2; return 2;
  }
  export IROHA_TEST_TARGET_DIR="${bundle_line#*$'\t'}"
  export IROHA_RELEASE_PREBUILT_MANIFEST_SHA256="${manifest_line#*$'\t'}"
}

sumeragi_export_source_bound_localnet_binaries() {
  sumeragi_localnet_binary_attestation_valid "$@" || return $?
  local suffix="" separator=/ host_os
  host_os="$("${IROHA_RELEASE_POLICY_PYTHON:?}" -B -I -S -c 'import os; print(os.name)')" || return $?
  if [[ "$host_os" == nt ]]; then suffix=.exe; separator='\'; fi
  export TEST_NETWORK_BIN_IROHAD="${IROHA_TEST_TARGET_DIR}${separator}iroha3d${suffix}"
  export TEST_NETWORK_BIN_IROHA="${IROHA_TEST_TARGET_DIR}${separator}iroha${suffix}"
  export KAGAMI_BIN="${IROHA_TEST_TARGET_DIR}${separator}kagami${suffix}"
  export TEST_NETWORK_BIN_IROHAD_TAIRA="${IROHA_TEST_TARGET_DIR}${separator}iroha3d_taira${suffix}"
}
