#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check_mobile_sdk_artifacts.sh [--root <repo-root>] [--apple-only|--android-only] [--require-built-android] [--allow-dirty-source]

Checks that the Iroha mobile SDK packaging surface is ready for wallet
integration:
  - SwiftPM package manifest and NoritoBridge binary target exist.
  - NoritoBridge.xcframework contains iOS device plus universal iOS simulator
    and macOS slices.
  - The canonical manifest is embedded in the XCFramework, and the public
    NoritoBridge.artifacts.json path is its stable relative symlink.
  - The manifest records per-slice SHA-256 hashes and the privacy-production
    feature state, which must match the XCFramework marker.
  - Every manifest hash matches the actual slice, all headers are identical,
    the Swift loader fallback pins match every manifest slice, and the manifest
    ABI/source fingerprint matches the checked-out bridge. A manifest may name
    the exact parent only when HEAD is a mechanically verified, non-merge hash-
    pin commit with no other source changes.
  - Apple archives contain their declared architectures and the complete
    Kagemusha recursive-spend symbol surface.
  - Kotlin/Android SDK modules are included and publishable; when Android
    outputs are required, raw cargo-ndk and generated stripped libraries match
    embedded provenance, and generated/AAR bytes are identical while binding
    the exact ABI-22 feature state.

By default Android build outputs are not required. Pass --require-built-android
or set MOBILE_SDK_REQUIRE_ANDROID_OUTPUTS=1 to require jar/aar outputs too.
By default both Apple and Android packaging surfaces are checked. Pass
--apple-only or --android-only when platform artifact builds run in separate CI
jobs.
Dirty bridge inputs are rejected by default. --allow-dirty-source (or
MOBILE_SDK_ALLOW_DIRTY_SOURCE=1) permits a local integration artifact only when
its manifest dirty bit and exact dependency-closure fingerprint match.
MOBILE_SDK_APPLE_ARTIFACT_DIR may point Apple validation at a staged artifact
directory; it defaults to <root>/dist.
Apple source authentication binds the exact lock selected by
IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH when that authenticated privacy lane
is active; ordinary checks bind <root>/Cargo.lock.
Source authentication requires Python 3.12, exact Rust 1.93.1 RUSTC/RUSTDOC,
and an explicit canonical writable CARGO_TARGET_DIR outside the Iroha source
tree. The reviewed envelope uses CARGO_BUILD_JOBS=1, CARGO_INCREMENTAL=0,
CARGO_NET_OFFLINE=true, and RUSTC_BOOTSTRAP=1.
The builder alone may set MOBILE_SDK_STAGED_BUILD_VALIDATION=1 together with a
private prospective-loader path. Final certification always checks the tracked
Swift loader.
USAGE
}

ROOT_ARG=""
REQUIRE_ANDROID_OUTPUTS="${MOBILE_SDK_REQUIRE_ANDROID_OUTPUTS:-0}"
ALLOW_DIRTY_SOURCE="${MOBILE_SDK_ALLOW_DIRTY_SOURCE:-0}"
CHECK_APPLE=1
CHECK_ANDROID=1

if [[ -n "${MOBILE_SDK_SKIP_BINARY_INSPECTION+x}" ]]; then
  echo "[mobile-sdk-artifacts] ERROR: MOBILE_SDK_SKIP_BINARY_INSPECTION is retired; binary inspection is mandatory" >&2
  exit 64
fi

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root)
      shift
      if [[ $# -eq 0 ]]; then
        echo "[mobile-sdk-artifacts] ERROR: --root requires a value" >&2
        exit 64
      fi
      ROOT_ARG="$1"
      ;;
    --root=*)
      ROOT_ARG="${1#*=}"
      ;;
    --require-built-android)
      REQUIRE_ANDROID_OUTPUTS=1
      ;;
    --allow-dirty-source)
      ALLOW_DIRTY_SOURCE=1
      ;;
    --apple-only)
      CHECK_APPLE=1
      CHECK_ANDROID=0
      ;;
    --android-only)
      CHECK_APPLE=0
      CHECK_ANDROID=1
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      if [[ -z "$ROOT_ARG" ]]; then
        ROOT_ARG="$1"
      else
        echo "[mobile-sdk-artifacts] ERROR: unexpected argument: $1" >&2
        usage >&2
        exit 64
      fi
      ;;
  esac
  shift
done

if [[ -z "$ROOT_ARG" ]]; then
  ROOT_ARG="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fi

if [[ ! -d "$ROOT_ARG" ]]; then
  echo "[mobile-sdk-artifacts] ERROR: repo root does not exist: $ROOT_ARG" >&2
  exit 66
fi

ROOT_DIR="$(cd "$ROOT_ARG" && pwd -P)"
APPLE_ARTIFACT_DIR="${MOBILE_SDK_APPLE_ARTIFACT_DIR:-$ROOT_DIR/dist}"
if [[ "$APPLE_ARTIFACT_DIR" != /* ]]; then
  APPLE_ARTIFACT_DIR="$ROOT_DIR/$APPLE_ARTIFACT_DIR"
fi
if [[ -n "${MOBILE_SDK_APPLE_CARGO_LOCK_PATH+x}" ]]; then
  echo "[mobile-sdk-artifacts] ERROR: MOBILE_SDK_APPLE_CARGO_LOCK_PATH is not part of the first-release artifact contract" >&2
  exit 64
fi
if [[ -n "${IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH+x}" ]]; then
  if [[ -z "${IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH}" ]]; then
    echo "[mobile-sdk-artifacts] ERROR: IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH must not be empty" >&2
    exit 64
  fi
  APPLE_CARGO_LOCKFILE="$IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH"
else
  APPLE_CARGO_LOCKFILE="$ROOT_DIR/Cargo.lock"
fi

resolve_trusted_python312() {
  local candidate canonical
  local override="${MOBILE_SDK_PYTHON_BINARY:-}"
  local candidates=()

  if [[ -n "$override" ]]; then
    if [[ "$override" != /* || ! -f "$override" || -L "$override" || ! -x "$override" ]]; then
      echo "[mobile-sdk-artifacts] ERROR: MOBILE_SDK_PYTHON_BINARY must be an absolute canonical regular executable" >&2
      return 1
    fi
    candidates=("$override")
  else
    candidates=(
      /opt/homebrew/opt/python@3.12/bin/python3.12
      /opt/homebrew/bin/python3.12
      /usr/local/opt/python@3.12/bin/python3.12
      /usr/local/bin/python3.12
      /usr/bin/python3.12
      /usr/bin/python3
    )
  fi

  for candidate in "${candidates[@]}"; do
    [[ -f "$candidate" && -x "$candidate" ]] || continue
    if ! canonical="$(
      env -i \
        HOME=/tmp \
        PATH=/usr/bin:/bin \
        TMPDIR=/tmp \
        LANG=C.UTF-8 \
        LC_ALL=C.UTF-8 \
        "$candidate" -I -S -B -c '
import os
import pathlib
import stat
import sys

if sys.version_info[:2] != (3, 12) or not sys.flags.isolated:
    raise SystemExit(1)
if "SDKROOT" in os.environ:
    raise SystemExit(1)
resolved = pathlib.Path(sys.executable).resolve(strict=True)
metadata = resolved.stat()
if not stat.S_ISREG(metadata.st_mode) or not os.access(resolved, os.X_OK):
    raise SystemExit(1)
print(resolved)
'
    )"; then
      continue
    fi
    if [[ "$canonical" != /* || ! -f "$canonical" || -L "$canonical" || ! -x "$canonical" ]]; then
      continue
    fi
    if [[ -n "$override" && "$canonical" != "$override" ]]; then
      echo "[mobile-sdk-artifacts] ERROR: MOBILE_SDK_PYTHON_BINARY must already name its canonical executable" >&2
      return 1
    fi
    printf '%s\n' "$canonical"
    return 0
  done

  if [[ -n "$override" ]]; then
    echo "[mobile-sdk-artifacts] ERROR: MOBILE_SDK_PYTHON_BINARY must be an isolated Python 3.12 executable" >&2
  else
    echo "[mobile-sdk-artifacts] ERROR: a trusted absolute Python 3.12 executable is required" >&2
  fi
  return 1
}

CHECK_PYTHON_BINARY="$(resolve_trusted_python312)" || exit 69
CHECK_USER_HOME_DIR="$("$CHECK_PYTHON_BINARY" -I -S -B -c \
  'import os,pathlib,pwd; print(pathlib.Path(pwd.getpwuid(os.getuid()).pw_dir).resolve(strict=True))' \
  2>/dev/null || true)"
if [[ -z "$CHECK_USER_HOME_DIR" ]]; then
  CHECK_USER_HOME_DIR="$("$CHECK_PYTHON_BINARY" -I -S -B -c \
    'import pathlib; print(pathlib.Path.home().resolve(strict=True))')"
fi
CHECK_TMPDIR="/tmp"
for trusted_path in "$CHECK_PYTHON_BINARY" "$CHECK_USER_HOME_DIR" "$CHECK_TMPDIR"; do
  if [[ "$trusted_path" != /* ]]; then
    echo "[mobile-sdk-artifacts] ERROR: verifier runtime path is not absolute: $trusted_path" >&2
    exit 69
  fi
done
if [[ ! -f "$CHECK_PYTHON_BINARY" || -L "$CHECK_PYTHON_BINARY" \
  || ! -x "$CHECK_PYTHON_BINARY" || ! -d "$CHECK_USER_HOME_DIR" \
  || ! -d "$CHECK_TMPDIR" ]]; then
  echo "[mobile-sdk-artifacts] ERROR: verifier runtime paths are not canonical regular files/directories" >&2
  exit 69
fi

run_isolated_checker_python() {
  env -i \
    HOME="$CHECK_USER_HOME_DIR" \
    PATH="${CHECK_PYTHON_BINARY%/*}:/usr/bin:/bin" \
    TMPDIR="$CHECK_TMPDIR" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    "$CHECK_PYTHON_BINARY" -I -S -B "$@"
}

ANDROID_KOTLIN_BUILD_ROOT="$ROOT_DIR/kotlin"
ANDROID_CORE_JVM_BUILD_DIR="$ROOT_DIR/kotlin/core-jvm/build"
ANDROID_CLIENT_BUILD_DIR="$ROOT_DIR/kotlin/client-android/build"
ANDROID_OFFLINE_WALLET_BUILD_DIR="$ROOT_DIR/kotlin/offline-wallet-android/build"
if [[ -n "${MOBILE_SDK_ANDROID_ARTIFACT_DIR:-}" ]]; then
  if ! canonical_android_artifact_dir="$(
    run_isolated_checker_python - \
      "$ROOT_DIR" "$MOBILE_SDK_ANDROID_ARTIFACT_DIR" <<'PY'
import os
import pathlib
import stat
import sys

source_root = pathlib.Path(sys.argv[1]).resolve(strict=True)
raw = sys.argv[2]
candidate = pathlib.Path(raw)
if not candidate.is_absolute() or str(candidate) != raw:
    raise SystemExit(1)
resolved = candidate.resolve(strict=True)
metadata = resolved.lstat()
if (
    resolved != candidate
    or not stat.S_ISDIR(metadata.st_mode)
    or stat.S_ISLNK(metadata.st_mode)
    or not os.access(resolved, os.R_OK | os.X_OK)
    or resolved == source_root
    or source_root in resolved.parents
):
    raise SystemExit(1)
print(resolved)
PY
  )"; then
    echo "[mobile-sdk-artifacts] ERROR: MOBILE_SDK_ANDROID_ARTIFACT_DIR must be a canonical non-symbolic directory outside the Iroha source tree" >&2
    exit 66
  fi
  ANDROID_KOTLIN_BUILD_ROOT="$canonical_android_artifact_dir/gradle-build/iroha_kotlin_sdk"
  ANDROID_CORE_JVM_BUILD_DIR="$ANDROID_KOTLIN_BUILD_ROOT/core-jvm"
  ANDROID_CLIENT_BUILD_DIR="$ANDROID_KOTLIN_BUILD_ROOT/client-android"
  ANDROID_OFFLINE_WALLET_BUILD_DIR="$ANDROID_KOTLIN_BUILD_ROOT/offline-wallet-android"
fi

SOURCE_SEAL_TOOLS_INITIALIZED=0
SOURCE_SEAL_CARGO_BINARY=""
SOURCE_SEAL_RUSTC_BINARY=""
SOURCE_SEAL_RUSTDOC_BINARY=""
SOURCE_SEAL_RUSTUP_BINARY=""
SOURCE_SEAL_CARGO_HOME="$CHECK_USER_HOME_DIR/.cargo"
SOURCE_SEAL_RUSTUP_HOME="$CHECK_USER_HOME_DIR/.rustup"
SOURCE_SEAL_CARGO_TARGET_DIR=""

initialize_source_seal_tools() {
  local actual_toolchain
  if [[ "$SOURCE_SEAL_TOOLS_INITIALIZED" == "1" ]]; then
    return
  fi
  SOURCE_SEAL_RUSTUP_BINARY="$CHECK_USER_HOME_DIR/.cargo/bin/rustup"
  if [[ ! -f "$SOURCE_SEAL_RUSTUP_BINARY" || -L "$SOURCE_SEAL_RUSTUP_BINARY" \
    || ! -x "$SOURCE_SEAL_RUSTUP_BINARY" || ! -x /usr/bin/git ]]; then
    echo "[mobile-sdk-artifacts] ERROR: pinned rustup and Git are required for source authentication" >&2
    exit 69
  fi
  actual_toolchain="$(
    /usr/bin/sed -nE \
      's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"([^"]+)"[[:space:]]*$/\1/p' \
      "$ROOT_DIR/rust-toolchain.toml"
  )"
  if [[ "$actual_toolchain" != "1.93.1" ]]; then
    echo "[mobile-sdk-artifacts] ERROR: source authentication requires exact Rust 1.93.1" >&2
    exit 69
  fi
  if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
    echo "[mobile-sdk-artifacts] ERROR: source authentication requires an explicit CARGO_TARGET_DIR" >&2
    exit 69
  fi
  if ! SOURCE_SEAL_CARGO_TARGET_DIR="$(run_isolated_checker_python - \
      "$CARGO_TARGET_DIR" "$ROOT_DIR" <<'PY'
import os
from pathlib import Path
import stat
import sys

candidate = Path(sys.argv[1])
source_root = Path(sys.argv[2])
if not candidate.is_absolute() or candidate != Path(os.path.abspath(candidate)):
    raise SystemExit(1)
try:
    metadata = candidate.lstat()
    resolved = candidate.resolve(strict=True)
except OSError:
    raise SystemExit(1) from None
if (
    resolved != candidate
    or stat.S_ISLNK(metadata.st_mode)
    or not stat.S_ISDIR(metadata.st_mode)
    or not os.access(candidate, os.R_OK | os.W_OK | os.X_OK)
    or candidate == source_root
    or source_root in candidate.parents
):
    raise SystemExit(1)
print(candidate)
PY
  )"; then
    echo "[mobile-sdk-artifacts] ERROR: CARGO_TARGET_DIR must be a writable, non-symbolic canonical directory outside the Iroha source tree" >&2
    exit 69
  fi
  SOURCE_SEAL_CARGO_BINARY="$(
    env -i \
      HOME="$CHECK_USER_HOME_DIR" \
      PATH="${SOURCE_SEAL_RUSTUP_BINARY%/*}:/usr/bin:/bin" \
      CARGO_HOME="$SOURCE_SEAL_CARGO_HOME" \
      RUSTUP_HOME="$SOURCE_SEAL_RUSTUP_HOME" \
      TMPDIR="$CHECK_TMPDIR" \
      LANG=C.UTF-8 \
      LC_ALL=C.UTF-8 \
      "$SOURCE_SEAL_RUSTUP_BINARY" which --toolchain 1.93.1 cargo
  )"
  SOURCE_SEAL_RUSTC_BINARY="$(
    env -i \
      HOME="$CHECK_USER_HOME_DIR" \
      PATH="${SOURCE_SEAL_RUSTUP_BINARY%/*}:/usr/bin:/bin" \
      CARGO_HOME="$SOURCE_SEAL_CARGO_HOME" \
      RUSTUP_HOME="$SOURCE_SEAL_RUSTUP_HOME" \
      TMPDIR="$CHECK_TMPDIR" \
      LANG=C.UTF-8 \
      LC_ALL=C.UTF-8 \
      "$SOURCE_SEAL_RUSTUP_BINARY" which --toolchain 1.93.1 rustc
  )"
  SOURCE_SEAL_RUSTDOC_BINARY="$(
    env -i \
      HOME="$CHECK_USER_HOME_DIR" \
      PATH="${SOURCE_SEAL_RUSTUP_BINARY%/*}:/usr/bin:/bin" \
      CARGO_HOME="$SOURCE_SEAL_CARGO_HOME" \
      RUSTUP_HOME="$SOURCE_SEAL_RUSTUP_HOME" \
      TMPDIR="$CHECK_TMPDIR" \
      LANG=C.UTF-8 \
      LC_ALL=C.UTF-8 \
      "$SOURCE_SEAL_RUSTUP_BINARY" which --toolchain 1.93.1 rustdoc
  )"
  SOURCE_SEAL_CARGO_BINARY="$(run_isolated_checker_python -c \
    'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
    "$SOURCE_SEAL_CARGO_BINARY")"
  SOURCE_SEAL_RUSTC_BINARY="$(run_isolated_checker_python -c \
    'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
    "$SOURCE_SEAL_RUSTC_BINARY")"
  SOURCE_SEAL_RUSTDOC_BINARY="$(run_isolated_checker_python -c \
    'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
    "$SOURCE_SEAL_RUSTDOC_BINARY")"
  if [[ ! -x "$SOURCE_SEAL_CARGO_BINARY" || ! -x "$SOURCE_SEAL_RUSTC_BINARY" \
    || ! -x "$SOURCE_SEAL_RUSTDOC_BINARY" ]]; then
    echo "[mobile-sdk-artifacts] ERROR: exact Rust 1.93.1 Cargo/rustc/rustdoc are unavailable" >&2
    exit 69
  fi
  SOURCE_SEAL_TOOLS_INITIALIZED=1
}

run_bridge_source_seal() {
  initialize_source_seal_tools
  env -i \
    HOME="$CHECK_USER_HOME_DIR" \
    PATH="${CHECK_PYTHON_BINARY%/*}:${SOURCE_SEAL_CARGO_BINARY%/*}:${SOURCE_SEAL_RUSTC_BINARY%/*}:${SOURCE_SEAL_RUSTDOC_BINARY%/*}:/usr/bin:/bin" \
    TMPDIR="$CHECK_TMPDIR" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    NORITO_BRIDGE_SEAL_HOME="$CHECK_USER_HOME_DIR" \
    NORITO_BRIDGE_SEAL_CARGO_HOME="$SOURCE_SEAL_CARGO_HOME" \
    NORITO_BRIDGE_SEAL_RUSTUP_HOME="$SOURCE_SEAL_RUSTUP_HOME" \
    NORITO_BRIDGE_SEAL_TMPDIR="$CHECK_TMPDIR" \
    NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR="$SOURCE_SEAL_CARGO_TARGET_DIR" \
    NORITO_BRIDGE_SEAL_CARGO="$SOURCE_SEAL_CARGO_BINARY" \
    NORITO_BRIDGE_SEAL_RUSTC="$SOURCE_SEAL_RUSTC_BINARY" \
    NORITO_BRIDGE_SEAL_RUSTDOC="$SOURCE_SEAL_RUSTDOC_BINARY" \
    "$CHECK_PYTHON_BINARY" -I -S -B "$ROOT_DIR/scripts/norito_bridge_source_seal.py" "$@"
}

run_bridge_source_git() {
  env -i \
    HOME="$CHECK_USER_HOME_DIR" \
    PATH=/usr/bin:/bin \
    TMPDIR="$CHECK_TMPDIR" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_OPTIONAL_LOCKS=0 \
    /usr/bin/git "$@"
}

FAILURES=0
CANDIDATE_LAB_MARKER="KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2"
CANDIDATE_LAB_SYMBOL_FRAGMENT="kagemusha_recursive_spend_candidate_lab_"
CANDIDATE_LAB_FEATURE="kagemusha-candidate-evidence-lab"
CANDIDATE_LAB_HEADER_MACRO="CONNECT_NORITO_KAGEMUSHA_CANDIDATE_EVIDENCE_LAB"
CANDIDATE_LAB_HEADER_MARKER="CONNECT_NORITO_KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2"
ANDROID_NATIVE_PROVENANCE_ENTRY="assets/iroha/native-build-provenance-v1.json"

# Exact non-shipping C surface consumed only by the authenticated candidate
# evidence harness. These names may exist in source only behind the dedicated
# feature/header guard and must never appear in a production binary.
KAGEMUSHA_CANDIDATE_LAB_C_SYMBOLS=(
  connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_begin_v4
  connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_write_v4
  connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_finalize_v4
  connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_cancel_v4
  connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_set_install_v4
  connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_set_is_installed_v4
  connect_norito_kagemusha_recursive_spend_candidate_lab_accepted_identity_v4
  connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_set_uninstall_v4
  connect_norito_kagemusha_recursive_spend_candidate_lab_init_v4
  connect_norito_kagemusha_recursive_spend_candidate_lab_append_v4
  connect_norito_kagemusha_recursive_spend_candidate_lab_verify_v4
  connect_norito_kagemusha_recursive_spend_candidate_lab_redeem_v4
  connect_norito_kagemusha_recursive_spend_candidate_lab_apple_proof_phase_v1
  connect_norito_kagemusha_recursive_spend_candidate_lab_apple_restart_phase_v1
)

# Exact private JNI surface of the non-shipping candidate-evidence module.
# These identities must be generated only inside the canonical feature-gated
# Rust module and must never appear in a production mobile archive.
KAGEMUSHA_CANDIDATE_LAB_JNI_SYMBOLS=(
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeBridgeAbiVersion
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeProductionCapabilityObservedV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeArtifactBeginV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeArtifactWriteV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeArtifactFinalizeV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeArtifactCancelV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeArtifactSetInstallV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeArtifactSetIsInstalledV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeAcceptedIdentityV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeArtifactSetUninstallV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeValidateBranchV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeBuildInitRequestV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeBuildAppendRequestV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeBuildDuplicateInputAppendRequestV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeBuildVerifyRequestV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeBuildRedeemRequestV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeAppendV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeProjectInitResultV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeProjectSplitResultV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeProjectVerifyResultV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeProjectRedeemResultV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeInitV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeVerifyV4
  Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeRedeemV4
)

# The first mobile release is one exact ABI-22/V4 contract. Keep the complete
# Kagemusha C export allow-list here so Apple archives, Android shared objects,
# checked-out Rust, and the checked-in header are all compared against the same
# surface. V2 suffixes below are unchanged note, authorization, membership, and
# acknowledgement primitives reused by V4; all recursive V2/V3 aliases are retired.
KAGEMUSHA_C_SYMBOLS=(
  connect_norito_kagemusha_recursive_spend_capabilities_v4
  connect_norito_kagemusha_topup_finality_verify_v4
  connect_norito_kagemusha_topup_shield_build_unsigned_v4
  connect_norito_kagemusha_recursive_spend_artifact_begin_v4
  connect_norito_kagemusha_recursive_spend_artifact_write_v4
  connect_norito_kagemusha_recursive_spend_artifact_finalize_v4
  connect_norito_kagemusha_recursive_spend_artifact_cancel_v4
  connect_norito_kagemusha_recursive_spend_artifact_set_install_v4
  connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v4
  connect_norito_kagemusha_recursive_spend_installed_manifest_sha256_v4
  connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v4
  connect_norito_kagemusha_output_membership_frontier_build_v4
  connect_norito_kagemusha_output_membership_paths_derive_v4
  connect_norito_kagemusha_recursive_spend_branch_validate_v4
  connect_norito_kagemusha_recursive_spend_topup_provenance_build_v4
  connect_norito_kagemusha_recursive_spend_topup_provenance_validate_v4
  connect_norito_kagemusha_recursive_spend_init_v4
  connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v4
  connect_norito_kagemusha_recursive_spend_topup_finalize_request_v4
  connect_norito_kagemusha_recursive_spend_topup_v4
  connect_norito_kagemusha_recursive_spend_append_v4
  connect_norito_kagemusha_recursive_spend_verify_v4
  connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v4
  connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v4
  connect_norito_kagemusha_recursive_spend_redeem_v4
  connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4
  connect_norito_kagemusha_secret_free_buffer
  connect_norito_kagemusha_receiver_key_reference_v2
  connect_norito_kagemusha_recipient_output_derive_v2
  connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2
  connect_norito_kagemusha_recipient_payment_request_create_v2
  connect_norito_kagemusha_recipient_payment_request_verify_v2
  connect_norito_kagemusha_recipient_receive_offer_create_v2
  connect_norito_kagemusha_recipient_receive_offer_project_v2
  connect_norito_kagemusha_recipient_receive_offer_verify_v2
  connect_norito_kagemusha_request_authorization_signing_bytes_v2
  connect_norito_kagemusha_request_authorization_finalize_hardware_v2
  connect_norito_kagemusha_request_authorization_finalize_ios_app_attest_v2
  connect_norito_kagemusha_receiver_acknowledgement_payload_v2
  connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2
  connect_norito_kagemusha_receiver_acknowledgement_create_v2
  connect_norito_kagemusha_receiver_acknowledgement_verify_v2
  connect_norito_kagemusha_recursive_spend_peer_split_change_prepare_v4
  connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v4
  connect_norito_kagemusha_recursive_spend_peer_payment_validate_v4
  connect_norito_kagemusha_recursive_spend_bundle_summary_v4
)

SORAFS_APPEAL_FINANCE_C_SYMBOLS=(
  connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json
)

PRIVACY_COMPILED_PROFILE_C_SYMBOLS=(
  iroha_privacy_compiled_profile_catalog_v1
  iroha_privacy_validate_compiled_profile_catalog_v1
  iroha_privacy_exact12_fixture_bundle_v1
  iroha_privacy_validate_exact12_fixture_bundle_v1
  iroha_privacy_free_buffer
)

OFFLINE_CASH_V1_C_SYMBOLS=(
  connect_norito_offline_cash_payment_request_canonicalize_v1
  connect_norito_offline_cash_payment_canonicalize_v1
  connect_norito_offline_cash_payment_canonicalize_for_session_v1
  connect_norito_offline_cash_acknowledgement_canonicalize_v1
  connect_norito_offline_cash_peer_encode_payment_request_v1
  connect_norito_offline_cash_peer_decode_payment_request_v1
  connect_norito_offline_cash_peer_encode_payment_v1
  connect_norito_offline_cash_peer_decode_payment_v1
  connect_norito_offline_cash_peer_encode_acknowledgement_v1
  connect_norito_offline_cash_peer_decode_acknowledgement_v1
  connect_norito_offline_cash_release_probe_v1
  connect_norito_offline_cash_artifact_begin_v1
  connect_norito_offline_cash_artifact_write_v1
  connect_norito_offline_cash_artifact_finalize_v1
  connect_norito_offline_cash_artifact_cancel_v1
  connect_norito_offline_cash_artifact_set_install_v1
  connect_norito_offline_cash_artifact_set_uninstall_v1
  connect_norito_offline_cash_wallet_session_open_v1
  connect_norito_offline_cash_wallet_session_open_bound_v1
  connect_norito_offline_cash_wallet_session_accept_payment_v1
  connect_norito_offline_cash_wallet_session_accept_acknowledgement_v1
  connect_norito_offline_cash_wallet_session_state_v1
  connect_norito_offline_cash_wallet_session_close_v1
)

REQUIRED_BRIDGE_SYMBOLS=(
  connect_norito_bridge_abi_version
  connect_norito_free
  connect_norito_chain_discriminant_scope_enter
  connect_norito_chain_discriminant_scope_exit
  connect_norito_encode_transfer_signed_transaction
  connect_norito_encode_transfer_instruction_box
  connect_norito_detached_transaction_scaffold_inspect_v1
  connect_norito_detached_transaction_scaffold_finalize_ed25519_v1
  connect_norito_canonical_json_blake3_v1
  connect_norito_encode_account_onboarding_plan_body_v1
  connect_norito_alias_instruction_round_trip_v1
  "${OFFLINE_CASH_V1_C_SYMBOLS[@]}"
  "${PRIVACY_COMPILED_PROFILE_C_SYMBOLS[@]}"
  connect_norito_sorafs_reference_validate_bundle_json
  connect_norito_sorafs_reference_validate_governance_json
  connect_norito_sorafs_reference_validate_governance_dag_block_json
  connect_norito_sorafs_reference_validate_governance_dag_head_chain_json
  connect_norito_validation_fee_current_policy_proof_request_v1
  connect_norito_validation_fee_current_policy_proof_verify_v1
  "${SORAFS_APPEAL_FINANCE_C_SYMBOLS[@]}"
  "${KAGEMUSHA_C_SYMBOLS[@]}"
)

# These process-global controls, retired privacy proof-construction exports,
# and fail-only compatibility entry points are forbidden from every
# first-release Apple and Android native slice.
FORBIDDEN_MOBILE_BRIDGE_SYMBOLS=(
  connect_norito_get_chain_discriminant
  connect_norito_set_chain_discriminant
  connect_norito_kagemusha_recipient_registration_lineage_verify_v1
  connect_norito_kagemusha_recipient_lineage_query_create_v2
  connect_norito_kagemusha_recipient_registration_lineage_verify_v2
  connect_norito_kagemusha_request_authorization_create_v2
  iroha_privacy_capabilities_v1
  iroha_privacy_validate_capabilities_v1
  iroha_privacy_proof_request_v1
  iroha_privacy_build_proof_v1
  iroha_privacy_verify_proof_v1
  CONNECT_NORITO_OFFLINE_CASH_TESTNET_DEVICE_EMULATOR_DO_NOT_SHIP_V1
  connect_norito_offline_cash_device_capabilities_v1
  connect_norito_offline_cash_device_execute_v1
  Java_org_hyperledger_iroha_sdk_offline_OfflineCashDeviceLifecycleBridgeV1_00024NativeEndpoint_nativeCapabilitiesV1
  Java_org_hyperledger_iroha_sdk_offline_OfflineCashDeviceLifecycleBridgeV1_00024NativeEndpoint_nativeExecuteV1
  Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeCreateAuthorizationV2
  Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeCreateAuthorizationV2
  Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeCreateRecipientLineageQueryV2
  Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeCreateRecipientLineageQueryV2
  Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeVerifyRecipientRegistrationLineageV2
  Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeVerifyRecipientRegistrationLineageV2
)

# Exact JNI allow-list for each supported Java namespace. As with the C list,
# V2 names retained here are non-lifecycle support primitives consumed by V4.
KAGEMUSHA_JNI_METHODS=(
  nativeAppendSpendV4
  nativeArtifactBeginV4
  nativeArtifactCancelV4
  nativeArtifactFinalizeV4
  nativeArtifactSetInstallV4
  nativeArtifactSetIsInstalledV4
  nativeArtifactSetUninstallV4
  nativeArtifactWriteV4
  nativeBranchClaimsConflictV2
  nativeBridgeAbiVersion
  nativeBuildAppendRequestV4
  nativeBuildArtifactBindingV4
  nativeBuildInitRequestV4
  nativeBuildOutputMembershipFrontierV4
  nativeBuildOutputMembershipPathsV4
  nativeBuildRedeemRequestV4
  nativeBuildRedeemV4
  nativeBuildTopUpProvenanceV4
  nativeBuildVerifyRequestV4
  nativeCreateAcknowledgementV2
  nativeCreateRecipientReceiveOfferV2
  nativeCreateRecipientRequestV2
  nativeDeriveOutputMembershipPathsV4
  nativeFinalizeHardwareAuthorizationV2
  nativeFinalizeIosAppAttestAuthorizationV2
  nativeFinalizeRedeemV4
  nativeFinalizeTopUpV4
  nativeInitSpendV4
  nativeInstalledManifestSha256V4
  nativePastaCycleV4BackendAvailable
  nativePrepareAcknowledgementV2
  nativePrepareAuthorizationV2
  nativePrepareNoteOpeningV2
  nativePreparePeerSplitChangeV4
  nativePrepareRedemptionChangeV4
  nativePrepareRecipientRequestV2
  nativePrepareTopUpV4
  nativeProjectActiveVerifierV2
  nativeProjectAuthenticatedArtifactSetV4
  nativeProjectInitResultV4
  nativeProjectOperationReferenceV4
  nativeProjectOperationStatusV4
  nativeProjectPeerPaymentV4
  nativeProjectReadinessV4
  nativeProjectRecipientRequestV2
  nativeProjectRecipientReceiveOfferV2
  nativeProjectRedeemBuildResultV4
  nativeProjectRedeemSubmissionRequestV4
  nativeProjectSplitResultV4
  nativeProjectTopUpSubmissionRequestV4
  nativeProjectVerifyResultV4
  nativeValidateSpendableBranchV4
  nativeValidateTopUpProvenanceV4
  nativeVerifyAcknowledgementV2
  nativeVerifyRecipientReceiveOfferV2
  nativeVerifyRecipientRequestV2
  nativeVerifySpendV4
)

VALIDATION_FEE_JNI_SYMBOLS=(
  Java_org_hyperledger_iroha_sdk_validationfee_ValidationFeeConsensusProofBridge_nativeBridgeAbiVersion
  Java_org_hyperledger_iroha_sdk_validationfee_ValidationFeeConsensusProofBridge_nativeEncodeCurrentPolicyProofRequestV1
  Java_org_hyperledger_iroha_sdk_validationfee_ValidationFeeConsensusProofBridge_nativeVerifyCurrentPolicyProofV1
)

SORAFS_APPEAL_FINANCE_JNI_SYMBOLS=(
  Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeHasAppealFinanceSymbols
  Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeValidateAppealFinanceCancelAssetLockJson
  Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeHasAppealFinanceSymbols
  Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeValidateAppealFinanceCancelAssetLockJson
)

PRIVACY_COMPILED_PROFILE_JNI_SYMBOLS=(
  Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeBridgeAbiVersion
  Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeCompiledProfileCatalog
  Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeValidateCompiledProfileCatalog
  Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeExact12FixtureBundle
  Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeValidateExact12FixtureBundle
  Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeBridgeAbiVersion
  Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeCompiledProfileCatalog
  Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeValidateCompiledProfileCatalog
  Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeExact12FixtureBundle
  Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeValidateExact12FixtureBundle
)

NATIVE_SIGNER_JNI_CONTRACT_SYMBOLS=(
  Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativeBridgeAbiVersion
  Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativeSignerContractRevision
  Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeBridgeAbiVersion
  Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeSignerContractRevision
)

relpath() {
  local path="$1"
  case "$path" in
    "$ROOT_DIR"/*) printf '%s' "${path#"$ROOT_DIR"/}" ;;
    *) printf '%s' "$path" ;;
  esac
}

fail() {
  printf '[mobile-sdk-artifacts] ERROR: %s\n' "$*" >&2
  FAILURES=1
}

require_file() {
  local path="$1"
  local label="$2"
  if [[ ! -f "$path" ]]; then
    fail "missing $label: $(relpath "$path")"
  fi
}

require_dir() {
  local path="$1"
  local label="$2"
  if [[ ! -d "$path" ]]; then
    fail "missing $label: $(relpath "$path")"
  fi
}

require_literal() {
  local path="$1"
  local literal="$2"
  local label="$3"
  if [[ ! -f "$path" ]]; then
    fail "cannot inspect missing $label file: $(relpath "$path")"
    return
  fi
  if ! grep -Fq -- "$literal" "$path"; then
    fail "$label not found in $(relpath "$path")"
  fi
}

require_regex() {
  local path="$1"
  local pattern="$2"
  local label="$3"
  if [[ ! -f "$path" ]]; then
    fail "cannot inspect missing $label file: $(relpath "$path")"
    return
  fi
  if ! grep -Eq -- "$pattern" "$path"; then
    fail "$label not found in $(relpath "$path")"
  fi
}

require_glob() {
  local pattern="$1"
  local label="$2"
  local matches=()
  while IFS= read -r match; do
    matches+=("$match")
  done < <(compgen -G "$pattern" || true)
  if [[ ${#matches[@]} -eq 0 ]]; then
    fail "missing $label: $pattern"
  fi
}

reject_candidate_lab_content() {
  local path="$1"
  local label="$2"
  [[ -f "$path" ]] || return
  if ! run_isolated_checker_python - "$path" "$CANDIDATE_LAB_MARKER" "$CANDIDATE_LAB_SYMBOL_FRAGMENT" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
needles = tuple(value.encode("ascii") for value in sys.argv[2:])
overlap = max(map(len, needles)) - 1
tail = b""
with path.open("rb") as handle:
    while True:
        chunk = handle.read(1024 * 1024)
        if not chunk:
            break
        window = tail + chunk
        if any(needle in window for needle in needles):
            raise SystemExit(1)
        tail = window[-overlap:] if overlap else b""
PY
  then
    fail "$label contains a non-shipping Kagemusha candidate-lab marker or symbol"
  fi
}

reject_candidate_lab_archive() {
  local path="$1"
  local label="$2"
  [[ -f "$path" ]] || return
  if ! run_isolated_checker_python - "$path" "$CANDIDATE_LAB_MARKER" "$CANDIDATE_LAB_SYMBOL_FRAGMENT" <<'PY'
import io
import sys
import zipfile

needles = tuple(value.encode("ascii") for value in sys.argv[2:])
overlap = max(map(len, needles)) - 1
archive_suffixes = (".aar", ".jar", ".zip")

def scan_stream(handle):
    tail = b""
    chunks = []
    total = 0
    while True:
        chunk = handle.read(1024 * 1024)
        if not chunk:
            break
        window = tail + chunk
        if any(needle in window for needle in needles):
            raise SystemExit(1)
        total += len(chunk)
        if total <= 256 * 1024 * 1024:
            chunks.append(chunk)
        tail = window[-overlap:] if overlap else b""
    return b"".join(chunks) if total <= 256 * 1024 * 1024 else None

def scan_archive(archive, depth=0):
    if depth > 3:
        raise SystemExit(2)
    for entry in archive.infolist():
        if entry.is_dir() or entry.file_size > 256 * 1024 * 1024:
            if entry.file_size > 256 * 1024 * 1024:
                raise SystemExit(2)
            continue
        with archive.open(entry) as handle:
            payload = scan_stream(handle)
        if entry.filename.lower().endswith(archive_suffixes):
            if payload is None:
                raise SystemExit(2)
            try:
                with zipfile.ZipFile(io.BytesIO(payload)) as nested:
                    scan_archive(nested, depth + 1)
            except zipfile.BadZipFile:
                raise SystemExit(2)

try:
    archive = zipfile.ZipFile(sys.argv[1])
except (OSError, zipfile.BadZipFile):
    raise SystemExit(2)
with archive:
    scan_archive(archive)
PY
  then
    fail "$label is unreadable or contains a non-shipping Kagemusha candidate-lab marker or symbol"
  fi
}

require_zip_entry() {
  local archive="$1"
  local entry="$2"
  local label="$3"
  local entries

  if [[ ! -f "$archive" ]]; then
    fail "cannot inspect missing $label: $(relpath "$archive")"
    return
  fi
  if ! command -v unzip >/dev/null 2>&1; then
    fail "unzip is required to inspect $label"
    return
  fi
  if ! entries="$(unzip -Z1 "$archive" 2>/dev/null)"; then
    fail "$label is not a readable ZIP/AAR archive: $(relpath "$archive")"
    return
  fi
  if ! grep -Fxq -- "$entry" <<<"$entries"; then
    fail "$label missing ZIP entry $entry in $(relpath "$archive")"
  fi
}

plist_contains() {
  local plist="$1"
  local needle="$2"
  if [[ ! -f "$plist" ]]; then
    return 1
  fi
  if grep -Fq -- "$needle" "$plist"; then
    return 0
  fi
  if command -v plutil >/dev/null 2>&1 && plutil -p "$plist" 2>/dev/null | grep -Fq -- "$needle"; then
    return 0
  fi
  return 1
}

hash_file() {
  local path="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | awk '{print $1}'
  else
    sha256sum "$path" | awk '{print $1}'
  fi
}

canonical_cargo_lock_seal() {
  local lock_file="$1"
  run_isolated_checker_python - "$lock_file" <<'PY'
import hashlib
import os
from pathlib import Path
import stat
import sys

candidate = Path(sys.argv[1])
if not candidate.is_absolute():
    raise SystemExit("selected Cargo lock path must be absolute")
if candidate != Path(os.path.abspath(candidate)):
    raise SystemExit("selected Cargo lock path must be canonical")
try:
    metadata = candidate.lstat()
    resolved = candidate.resolve(strict=True)
except OSError:
    raise SystemExit(
        "selected Cargo lock must be a non-symbolic regular file"
    ) from None
if (
    resolved != candidate
    or stat.S_ISLNK(metadata.st_mode)
    or not stat.S_ISREG(metadata.st_mode)
    or metadata.st_nlink != 1
):
    raise SystemExit(
        "selected Cargo lock must be a singly linked non-symbolic regular file"
    )
flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
descriptor = os.open(candidate, flags)
digest = hashlib.sha256()
try:
    before = os.fstat(descriptor)
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise SystemExit("selected Cargo lock must remain singly linked")
    observed = 0
    while chunk := os.read(descriptor, 1024 * 1024):
        observed += len(chunk)
        digest.update(chunk)
    after = os.fstat(descriptor)
finally:
    os.close(descriptor)
before_identity = (
    before.st_dev,
    before.st_ino,
    before.st_mode,
    before.st_nlink,
    before.st_size,
    before.st_mtime_ns,
    before.st_ctime_ns,
)
after_identity = (
    after.st_dev,
    after.st_ino,
    after.st_mode,
    after.st_nlink,
    after.st_size,
    after.st_mtime_ns,
    after.st_ctime_ns,
)
visible = candidate.lstat()
if (
    before_identity != after_identity
    or observed != before.st_size
    or stat.S_ISLNK(visible.st_mode)
    or (visible.st_dev, visible.st_ino) != (before.st_dev, before.st_ino)
):
    raise SystemExit("selected Cargo lock changed identity while it was read")
print(
    ":".join(
        (
            digest.hexdigest(),
            str(before.st_dev),
            str(before.st_ino),
            str(before.st_mode),
            str(before.st_nlink),
            str(before.st_size),
            str(before.st_mtime_ns),
            str(before.st_ctime_ns),
        )
    )
)
PY
}

if ! APPLE_CARGO_LOCK_SEAL_START="$(
  canonical_cargo_lock_seal "$APPLE_CARGO_LOCKFILE"
)"; then
  echo "[mobile-sdk-artifacts] ERROR: selected Apple Cargo lock is not a stable singly linked canonical file" >&2
  exit 69
fi

assert_apple_cargo_lock() {
  local current_seal
  if ! current_seal="$(canonical_cargo_lock_seal "$APPLE_CARGO_LOCKFILE")"; then
    echo "[mobile-sdk-artifacts] ERROR: selected Apple Cargo lock became unreadable" >&2
    return 1
  fi
  if [[ "$current_seal" != "$APPLE_CARGO_LOCK_SEAL_START" ]]; then
    echo "[mobile-sdk-artifacts] ERROR: selected Apple Cargo lock changed during validation" >&2
    return 1
  fi
}

hash_zip_entry() {
  local archive="$1"
  local entry="$2"
  if command -v shasum >/dev/null 2>&1; then
    unzip -p "$archive" "$entry" | shasum -a 256 | awk '{print $1}'
  else
    unzip -p "$archive" "$entry" | sha256sum | awk '{print $1}'
  fi
}

manifest_json_value() {
  local manifest="$1"
  local key="$2"
  run_isolated_checker_python - "$manifest" "$key" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    value = json.load(handle)
for component in sys.argv[2].split("."):
    value = value[component]
if isinstance(value, bool):
    print("true" if value else "false")
else:
    print(value)
PY
}

bridge_source_fingerprint() {
  local arguments=(fingerprint --root "$ROOT_DIR" --platform apple)
  if [[ -n "$APPLE_CARGO_LOCKFILE" ]]; then
    arguments+=(--lockfile-path "$APPLE_CARGO_LOCKFILE")
  fi
  run_bridge_source_seal "${arguments[@]}"
}

bridge_source_status() {
  local arguments=(status --root "$ROOT_DIR" --platform apple)
  if [[ -n "$APPLE_CARGO_LOCKFILE" ]]; then
    arguments+=(--lockfile-path "$APPLE_CARGO_LOCKFILE")
  fi
  run_bridge_source_seal "${arguments[@]}"
}

check_bridge_source_contract() {
  local bridge_source="$ROOT_DIR/crates/connect_norito_bridge/src/lib.rs"
  local bridge_header="$ROOT_DIR/crates/connect_norito_bridge/include/connect_norito_bridge.h"
  local privacy_protocol="$ROOT_DIR/crates/iroha_data_model/src/privacy/protocol.rs"
  local bridge_cargo="$ROOT_DIR/crates/connect_norito_bridge/Cargo.toml"

  # Packaged artifacts can be checked outside a source checkout. When source is
  # present, however, refuse to certify a build whose callable Kagemusha ABI is
  # broader or narrower than the exact first-release allow-list.
  if [[ -f "$bridge_source" ]]; then
    if ! run_isolated_checker_python - \
        "$bridge_source" "$bridge_cargo" "$bridge_header" "$privacy_protocol" \
        "$CANDIDATE_LAB_FEATURE" "$CANDIDATE_LAB_MARKER" \
        "$CANDIDATE_LAB_HEADER_MARKER" \
        --shipping "${KAGEMUSHA_C_SYMBOLS[@]}" \
        --lab "${KAGEMUSHA_CANDIDATE_LAB_C_SYMBOLS[@]}" \
        --lab-jni "${KAGEMUSHA_CANDIDATE_LAB_JNI_SYMBOLS[@]}" <<'PY'
from collections import Counter
import re
import sys
import tomllib

path = sys.argv[1]
cargo_path = sys.argv[2]
header_path = sys.argv[3]
protocol_path = sys.argv[4]
feature = sys.argv[5]
marker = sys.argv[6]
marker_symbol = sys.argv[7]
shipping_separator = sys.argv.index("--shipping")
lab_separator = sys.argv.index("--lab")
lab_jni_separator = sys.argv.index("--lab-jni")
expected = set(sys.argv[shipping_separator + 1:lab_separator])
expected_lab = set(sys.argv[lab_separator + 1:lab_jni_separator])
expected_lab_jni_order = sys.argv[lab_jni_separator + 1:]
expected_lab_jni = set(expected_lab_jni_order)
text = open(path, "r", encoding="utf-8").read()
try:
    header_text = open(header_path, "r", encoding="utf-8").read()
    canonical_abi_text = open(protocol_path, "r", encoding="utf-8").read()
except OSError:
    header_text = canonical_abi_text = ""


def rust_code_mask(source):
    """Mark Rust tokens while excluding comments and string/byte literals."""

    mask = bytearray(b"\x01") * len(source)

    def hide(start, end):
        mask[start:end] = b"\x00" * (end - start)

    def raw_literal_end(start):
        if start > 0 and (source[start - 1].isalnum() or source[start - 1] == "_"):
            return None
        for prefix in ("br", "cr", "r"):
            if not source.startswith(prefix, start):
                continue
            cursor = start + len(prefix)
            hashes = 0
            while cursor < len(source) and source[cursor] == "#":
                hashes += 1
                cursor += 1
            if cursor >= len(source) or source[cursor] != '"':
                continue
            closing = '"' + ("#" * hashes)
            end = source.find(closing, cursor + 1)
            return len(source) if end < 0 else end + len(closing)
        return None

    cursor = 0
    while cursor < len(source):
        if source.startswith("//", cursor):
            end = source.find("\n", cursor + 2)
            end = len(source) if end < 0 else end
            hide(cursor, end)
            cursor = end
            continue
        if source.startswith("/*", cursor):
            depth = 1
            end = cursor + 2
            while end < len(source) and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            hide(cursor, end)
            cursor = end
            continue
        raw_end = raw_literal_end(cursor)
        if raw_end is not None:
            hide(cursor, raw_end)
            cursor = raw_end
            continue
        quote = None
        if source[cursor] == '"':
            quote = cursor
        elif (
            source[cursor] in ("b", "c")
            and cursor + 1 < len(source)
            and source[cursor + 1] == '"'
            and (cursor == 0 or not (source[cursor - 1].isalnum() or source[cursor - 1] == "_"))
        ):
            quote = cursor + 1
        if quote is not None:
            end = quote + 1
            while end < len(source):
                if source[end] == "\\":
                    end = min(len(source), end + 2)
                elif source[end] == '"':
                    end += 1
                    break
                else:
                    end += 1
            hide(cursor, end)
            cursor = end
            continue
        character_quote = None
        if source[cursor] == "'":
            character_quote = cursor
        elif (
            source[cursor] == "b"
            and cursor + 1 < len(source)
            and source[cursor + 1] == "'"
            and (cursor == 0 or not (source[cursor - 1].isalnum() or source[cursor - 1] == "_"))
        ):
            character_quote = cursor + 1
        if character_quote is not None:
            content = character_quote + 1
            literal_end = None
            if content < len(source) and source[content] == "\\":
                end = min(len(source), content + 2)
                while end < len(source) and source[end] != "\n":
                    if source[end] == "\\":
                        end = min(len(source), end + 2)
                    elif source[end] == "'":
                        literal_end = end + 1
                        break
                    else:
                        end += 1
            elif content + 1 < len(source) and source[content + 1] == "'":
                literal_end = content + 2
            if literal_end is not None:
                hide(cursor, literal_end)
                cursor = literal_end
                continue
            # A Rust lifetime or label is not a character literal.
            cursor += 1
            continue
        cursor += 1
    return mask


code_mask = rust_code_mask(text)
masked_text = "".join(
    character if code_mask[index] else ("\n" if character == "\n" else " ")
    for index, character in enumerate(text)
)


def code_matches(pattern, *, start=0, end=None):
    boundary = len(text) if end is None else end
    return [
        match for match in pattern.finditer(text, start, boundary)
        if code_mask[match.start()]
    ]


def matching_code_brace(opening):
    """Return the matching code brace, ignoring masked Rust lexical content."""

    if opening >= len(text) or text[opening] != "{" or not code_mask[opening]:
        return None
    depth = 0
    for cursor in range(opening, len(text)):
        if not code_mask[cursor]:
            continue
        if text[cursor] == "{":
            depth += 1
        elif text[cursor] == "}":
            depth -= 1
            if depth == 0:
                return cursor
    return None


def braced_regions(pattern, *, start=0, end=None):
    """Find code patterns ending in `{` and return their balanced regions."""

    regions = []
    for match in code_matches(pattern, start=start, end=end):
        opening = match.end() - 1
        closing = matching_code_brace(opening)
        if closing is None or (end is not None and closing >= end):
            regions.append((match, opening, None))
        else:
            regions.append((match, opening, closing))
    return regions


def immediate_attributes(position):
    match = re.search(r"(?ms)((?:#\[[^\]]+\]\s*)+)\Z", text[:position])
    if match is None or not code_mask[match.start()]:
        return ""
    return re.sub(r"\s+", "", match.group(1))


errors = []
if len(expected_lab_jni_order) != 24 or len(expected_lab_jni) != 24:
    errors.append("candidate-lab JNI allow-list must contain exactly 24 unique identities")
abi_aliases = code_matches(re.compile(
    r"(?m)^const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = "
    r"([^;\n]+);$",
))
header_abis = re.findall(
    r"^#define[ \t]+CONNECT_NORITO_BRIDGE_ABI_VERSION[ \t]+([0-9]+)[ \t]*$",
    header_text,
    re.MULTILINE,
)
protocol_abis = re.findall(
    r"^pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = ([0-9]+);$",
    canonical_abi_text,
    re.MULTILINE,
)
export_pattern = re.compile(
    r'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+'
    r'(connect_norito_kagemusha_[A-Za-z0-9_]+)\s*\(',
)
lab_function_pattern = re.compile(
    r'\bfn\s+(connect_norito_kagemusha_[A-Za-z0-9_]*candidate_lab_'
    r'[A-Za-z0-9_]+)\s*\(',
)
jni_pattern = re.compile(
    r'\bpub\s+unsafe\s+extern\s+"system"\s+fn\s+'
    r'(Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_[A-Za-z0-9_]+)\s*\('
)
marker_symbol_pattern = re.compile(rf'\b{re.escape(marker_symbol)}\b')
direct_export_counts = Counter(match.group(1) for match in code_matches(export_pattern))
direct_lab_function_counts = Counter(
    match.group(1) for match in code_matches(lab_function_pattern)
)
direct_lab_export_counts = Counter(
    name for name, count in direct_export_counts.items()
    for _ in range(count) if "_candidate_lab_" in name
)
marker_occurrences = code_matches(marker_symbol_pattern)

production_lifecycle = {
    "init": "connect_norito_kagemusha_recursive_spend_init_v4",
    "append": "connect_norito_kagemusha_recursive_spend_append_v4",
    "verify": "connect_norito_kagemusha_recursive_spend_verify_v4",
    "redeem": "connect_norito_kagemusha_recursive_spend_redeem_v4",
}
lab_lifecycle = {
    "init": "connect_norito_kagemusha_recursive_spend_candidate_lab_init_v4",
    "append": "connect_norito_kagemusha_recursive_spend_candidate_lab_append_v4",
    "verify": "connect_norito_kagemusha_recursive_spend_candidate_lab_verify_v4",
    "redeem": "connect_norito_kagemusha_recursive_spend_candidate_lab_redeem_v4",
}
generated_production_counts = Counter()
generated_lab_counts = Counter()

generator_pattern = re.compile(
    r"(?m)^macro_rules![ \t]+kagemusha_recursive_spend_lifecycle_exports[ \t]*\{"
)
generator_regions = braced_regions(generator_pattern)
if len(generator_regions) != 1 or generator_regions[0][2] is None:
    errors.append("Kagemusha lifecycle export generator is not one balanced definition")
else:
    _, generator_open, generator_close = generator_regions[0]
    generator_start = generator_open + 1
    generator_end = generator_close
    for slot in ("init", "append", "verify", "redeem"):
        signature = re.compile(
            rf"{slot}\s+\$\(#\[\${slot}_attribute:meta\]\)\*\s*=>\s*"
            rf"\${slot}_name:ident\s*,\s*\${slot}_worker:literal\s*;"
        )
        expansion = re.compile(
            rf"\$\(#\[\${slot}_attribute\]\)\*\s*"
            r'#\[unsafe\(no_mangle\)\]\s*pub\s+unsafe\s+extern\s+"C"\s+fn\s+'
            rf"\${slot}_name\s*\("
        )
        if len(code_matches(signature, start=generator_start, end=generator_end)) != 1:
            errors.append(f"Kagemusha lifecycle generator has a non-canonical {slot} input")
        if len(code_matches(expansion, start=generator_start, end=generator_end)) != 1:
            errors.append(
                f"Kagemusha lifecycle generator has a non-canonical no-mangle {slot} export"
            )
    generated_declarations = code_matches(
        re.compile(r'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+\$[A-Za-z0-9_]+\s*\('),
        start=generator_start,
        end=generator_end,
    )
    if len(generated_declarations) != 4:
        errors.append("Kagemusha lifecycle generator must emit exactly four C exports")

lifecycle_call_pattern = re.compile(
    r"(?m)^kagemusha_recursive_spend_lifecycle_exports![ \t]*\{"
)
lifecycle_calls = braced_regions(lifecycle_call_pattern)
lifecycle_call_kinds = Counter()
exact_lab_attribute = re.sub(
    r"\s+", "", f'#[cfg(feature = "{feature}")]'
)
for call_match, call_open, call_close in lifecycle_calls:
    if call_close is None:
        errors.append("Kagemusha lifecycle invocation is not balanced")
        continue
    call_body = masked_text[call_open + 1:call_close]
    resolvers = re.findall(
        r"(?m)^\s*resolver\s*=\s*([A-Za-z_][A-Za-z0-9_:]*)\s*;",
        call_body,
    )
    prechecks = re.findall(
        r"(?m)^\s*verify_precheck\s*=\s*(true|false)\s*;",
        call_body,
    )
    slots = re.findall(
        r"(?ms)^\s*(init|append|verify|redeem)\b(?:(?!=>).)*?=>\s*"
        r"(connect_norito_kagemusha_[A-Za-z0-9_]+)\s*,",
        call_body,
    )
    slot_map = dict(slots)
    canonical_slots = len(slots) == 4 and len(slot_map) == 4
    attributes = immediate_attributes(call_match.start())
    if (
        resolvers == ["require_kagemusha_recursive_spend_artifact_binding_v4"]
        and prechecks == ["true"]
        and canonical_slots
        and slot_map == production_lifecycle
    ):
        lifecycle_call_kinds["production"] += 1
        if attributes:
            errors.append("production Kagemusha lifecycle invocation must be unguarded")
        generated_production_counts.update(slot_map.values())
    elif (
        resolvers == ["require_kagemusha_candidate_evidence_lab_artifact_binding_v4"]
        and prechecks == ["false"]
        and canonical_slots
        and slot_map == lab_lifecycle
    ):
        lifecycle_call_kinds["lab"] += 1
        if attributes != exact_lab_attribute:
            errors.append(
                "candidate-lab lifecycle invocation lacks its exact feature guard"
            )
        generated_lab_counts.update(slot_map.values())
    else:
        errors.append("Kagemusha lifecycle invocation is not exact")

if lifecycle_call_kinds["production"] != 1:
    errors.append("production Kagemusha lifecycle invocation is not exactly one")
if lifecycle_call_kinds["lab"] > 1:
    errors.append("candidate-lab Kagemusha lifecycle invocation is duplicated")

all_export_counts = direct_export_counts + generated_production_counts + generated_lab_counts
lab_function_counts = direct_lab_function_counts + generated_lab_counts
lab_export_counts = direct_lab_export_counts + generated_lab_counts

exact_jni_cfg = (
    rf'(?ms)^#\[cfg\(all\(\s*feature\s*=\s*"{re.escape(feature)}"\s*,'
    r'\s*any\(\s*target_os\s*=\s*"android"\s*,'
    r'\s*target_os\s*=\s*"linux"\s*,'
    r'\s*target_os\s*=\s*"macos"\s*,'
    r'\s*target_os\s*=\s*"windows"\s*\)\s*\)\)\][ \t]*\n'
)
candidate_jni_identity_pattern = re.compile(
    r'\b(Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_'
    r'KagemushaCandidateLabNative_native[A-Za-z0-9_]+)\s*\('
)
direct_jni_counts = Counter(
    match.group(1) for match in code_matches(jni_pattern)
)
all_candidate_jni_identity_matches = code_matches(candidate_jni_identity_pattern)
all_candidate_jni_identity_counts = Counter(
    match.group(1) for match in all_candidate_jni_identity_matches
)
jni_contract_errors = []
jni_counts = Counter()

candidate_jni_module_pattern = re.compile(
    r'(?m)^mod[ \t]+kagemusha_candidate_lab_jni[ \t]*\{'
)
candidate_jni_modules = braced_regions(candidate_jni_module_pattern)
exact_candidate_jni_module_pattern = re.compile(
    exact_jni_cfg
    + r'#\[allow\(clippy::missing_safety_doc,[ \t]*clippy::too_many_arguments\)\][ \t]*\n'
    + r'mod[ \t]+kagemusha_candidate_lab_jni[ \t]*\{'
)
exact_candidate_jni_modules = braced_regions(exact_candidate_jni_module_pattern)
generic_reexports = code_matches(re.compile(
    r'\bpub\s+use\s+kagemusha_candidate_lab_jni::\*\s*;'
))
exact_reexports = code_matches(re.compile(
    exact_jni_cfg + r'pub\s+use\s+kagemusha_candidate_lab_jni::\*\s*;'
))

module_bounds = None
if len(candidate_jni_modules) == 1 and candidate_jni_modules[0][2] is not None:
    _, module_open, module_close = candidate_jni_modules[0]
    module_bounds = (module_open + 1, module_close)
else:
    jni_contract_errors.append(
        "candidate-lab JNI module is not one balanced definition"
    )
if (
    len(exact_candidate_jni_modules) != 1
    or exact_candidate_jni_modules[0][2] is None
    or module_bounds is None
    or exact_candidate_jni_modules[0][1] + 1 != module_bounds[0]
    or exact_candidate_jni_modules[0][2] != module_bounds[1]
):
    jni_contract_errors.append(
        "candidate-lab JNI module lacks its exact conjunctive feature guard"
    )
if len(generic_reexports) != 1 or len(exact_reexports) != 1:
    jni_contract_errors.append(
        "candidate-lab JNI re-export lacks its exact conjunctive feature guard"
    )

if module_bounds is not None:
    module_start, module_end = module_bounds
    macro_specs = (
        ("candidate_lab_jni_export", 1),
        ("candidate_lab_jni_forwarders", 1),
    )
    for macro_name, expected_declarations in macro_specs:
        definitions = braced_regions(
            re.compile(rf'\bmacro_rules![ \t]+{macro_name}[ \t]*\{{'),
            start=module_start,
            end=module_end,
        )
        if len(definitions) != 1 or definitions[0][2] is None:
            jni_contract_errors.append(
                f"candidate-lab JNI generator {macro_name} is not one balanced definition"
            )
            continue
        _, definition_open, definition_close = definitions[0]
        generated = code_matches(
            re.compile(
                r'#\[unsafe\(no_mangle\)\]\s*pub\s+unsafe\s+extern\s+"system"\s+'
                r'fn\s+\$name\s*\('
            ),
            start=definition_open + 1,
            end=definition_close,
        )
        declarations = code_matches(
            re.compile(
                r'pub\s+(?:unsafe\s+)?extern\s+"system"\s+fn\s+'
                r'\$[A-Za-z0-9_]+\s*\('
            ),
            start=definition_open + 1,
            end=definition_close,
        )
        if len(generated) != expected_declarations or len(declarations) != expected_declarations:
            jni_contract_errors.append(
                f"candidate-lab JNI generator {macro_name} does not emit one exact no-mangle export"
            )

    bespoke_calls = braced_regions(
        re.compile(r'\bcandidate_lab_jni_export![ \t]*\{'),
        start=module_start,
        end=module_end,
    )
    forwarder_calls = braced_regions(
        re.compile(r'\bcandidate_lab_jni_forwarders![ \t]*\{'),
        start=module_start,
        end=module_end,
    )
    bespoke_names = []
    for _, call_open, call_close in bespoke_calls:
        if call_close is None:
            continue
        names = code_matches(
            candidate_jni_identity_pattern,
            start=call_open + 1,
            end=call_close,
        )
        if len(names) == 1:
            bespoke_names.append(names[0].group(1))
    forwarder_names = []
    if len(forwarder_calls) == 1 and forwarder_calls[0][2] is not None:
        _, call_open, call_close = forwarder_calls[0]
        forwarder_names = [
            match.group(1)
            for match in code_matches(
                candidate_jni_identity_pattern,
                start=call_open + 1,
                end=call_close,
            )
        ]
    if len(bespoke_calls) != 2 or bespoke_names != expected_lab_jni_order[:2]:
        jni_contract_errors.append(
            "candidate-lab bespoke JNI invocation inventory is not exact"
        )
    if len(forwarder_calls) != 1 or forwarder_names != expected_lab_jni_order[2:]:
        jni_contract_errors.append(
            "candidate-lab forwarded JNI invocation inventory is not exact"
        )
    jni_counts.update(bespoke_names)
    jni_counts.update(forwarder_names)

if direct_jni_counts:
    jni_contract_errors.append(
        "candidate-lab JNI exports must use only the canonical guarded generators"
    )
if jni_counts != Counter({name: 1 for name in expected_lab_jni}):
    missing_jni = sorted(expected_lab_jni - set(jni_counts))
    unexpected_jni = sorted(set(jni_counts) - expected_lab_jni)
    duplicate_jni = sorted(name for name, count in jni_counts.items() if count != 1)
    jni_contract_errors.append(
        "candidate-lab JNI inventory is not exact "
        f"(missing={missing_jni}, unexpected={unexpected_jni}, "
        f"non_single_occurrence={duplicate_jni})"
    )
if all_candidate_jni_identity_counts != jni_counts:
    jni_contract_errors.append(
        "candidate-lab JNI identity appears outside its canonical macro invocation"
    )

cargo = None
cargo_error = None
try:
    with open(cargo_path, "rb") as handle:
        cargo = tomllib.load(handle)
except (OSError, tomllib.TOMLDecodeError) as error:
    cargo_error = error
cargo_features = cargo.get("features") if isinstance(cargo, dict) else None
cargo_declares_lab = isinstance(cargo_features, dict) and feature in cargo_features
lab_present = bool(
    lab_function_counts
    or lab_export_counts
    or jni_counts
    or direct_jni_counts
    or all_candidate_jni_identity_counts
    or candidate_jni_modules
    or generic_reexports
    or marker_occurrences
    or cargo_declares_lab
)

if lab_present:
    if lifecycle_call_kinds["lab"] != 1:
        errors.append("candidate-lab Kagemusha lifecycle invocation is not exactly one")
    errors.extend(jni_contract_errors)
    for label, counts in (
        ("Rust function", lab_function_counts),
        ("Rust/C export", lab_export_counts),
    ):
        observed = set(counts)
        missing = sorted(expected_lab - observed)
        unexpected = sorted(observed - expected_lab)
        duplicates = sorted(name for name, count in counts.items() if count != 1)
        if missing or unexpected or duplicates:
            errors.append(
                f"candidate-lab {label} inventory is not exact "
                f"(missing={missing}, unexpected={unexpected}, "
                f"non_single_occurrence={duplicates})"
            )

    for name in sorted(expected_lab - set(generated_lab_counts)):
        declaration = re.compile(
            rf'(?m)^#\[cfg\(feature = "{re.escape(feature)}"\)\][ \t]*\n'
            rf'#\[unsafe\(no_mangle\)\][ \t]*\n'
            rf'^pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+{re.escape(name)}\s*\('
        )
        if len(code_matches(declaration)) != 1:
            errors.append(
                "candidate-lab Rust export is not directly guarded by its exact "
                f"feature: {name}"
            )

    marker_const = re.compile(
        rf'(?m)^#\[cfg\(feature = "{re.escape(feature)}"\)\][ \t]*\n'
        r'^pub\s+const\s+KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_MARKER_V2'
        rf'\s*:\s*&str\s*=\s*\n?[ \t]*"{re.escape(marker)}"\s*;'
    )
    marker_static = re.compile(
        rf'(?ms)^#\[cfg\(feature = "{re.escape(feature)}"\)\][ \t]*\n'
        rf'#\[used\][ \t]*\n#\[unsafe\(no_mangle\)\][ \t]*\n'
        rf'pub\s+static\s+{re.escape(marker_symbol)}\s*:\s*\[u8;[^\]]+\]\s*='
        rf'\s*\*b"{re.escape(marker)}"\s*;'
    )
    marker_definitions = code_matches(re.compile(
        rf'(?m)^pub\s+static\s+{re.escape(marker_symbol)}\b'
    ))
    if len(code_matches(marker_const)) != 1:
        errors.append(
            "candidate-lab Rust marker value is not directly guarded by its exact feature"
        )
    if len(code_matches(marker_static)) != 1 or len(marker_definitions) != 1:
        errors.append(
            "candidate-lab Rust link marker is not one exact guarded no-mangle static"
        )

    if cargo_error is not None:
        errors.append(
            f"candidate-lab Cargo feature policy is unreadable: {cargo_error}"
        )
    else:
        features = cargo_features
        exact_delegation = [f"iroha_core/{feature}"]
        if not isinstance(features, dict) or features.get(feature) != exact_delegation:
            errors.append("candidate-lab Cargo feature delegation is not exact")
        else:
            default = features.get("default", [])
            if not isinstance(default, list) or not all(
                isinstance(item, str) for item in default
            ):
                errors.append("candidate-lab Cargo default feature list is malformed")
            else:
                reachable = set()
                pending = list(default)
                while pending:
                    item = pending.pop()
                    if item in reachable:
                        continue
                    reachable.add(item)
                    delegated = features.get(item)
                    if isinstance(delegated, list):
                        pending.extend(
                            value for value in delegated if isinstance(value, str)
                        )
                if feature in reachable:
                    errors.append(
                        "candidate-lab Cargo feature is enabled directly or transitively by default"
                    )

# Candidate-evidence exports are excluded from the shipping ABI only after all
# checks above authenticate the complete lab-only source contract.
shipping_export_counts = Counter(
    {
        name: count
        for name, count in all_export_counts.items()
        if "_candidate_lab_" not in name
    }
)
actual = set(shipping_export_counts)
non_single_shipping = sorted(
    name for name, count in shipping_export_counts.items() if count != 1
)
if (
    len(abi_aliases) != 1
    or abi_aliases[0].group(1).strip() != "PRIVACY_BRIDGE_ABI_VERSION_V1"
    or header_abis != ["22"]
    or protocol_abis != header_abis
):
    errors.append(
        "bridge ABI must be exact public-header 22 with the canonical Rust alias "
        "and privacy protocol constant"
    )
missing = sorted(expected - actual)
retired_or_extra = sorted(actual - expected)
if non_single_shipping:
    errors.append(
        "Kagemusha shipping C export source identities are not single-occurrence: "
        + ", ".join(non_single_shipping)
    )
if missing:
    errors.append("missing Kagemusha C exports: " + ", ".join(missing))
if retired_or_extra:
    errors.append("retired or unexpected Kagemusha C exports: " + ", ".join(retired_or_extra))
for error in errors:
    print(f"[mobile-sdk-artifacts] ERROR: {error}", file=sys.stderr)
raise SystemExit(1 if errors else 0)
PY
    then
      FAILURES=1
    fi
  fi

  if [[ -f "$bridge_header" ]]; then
    if ! run_isolated_checker_python - "$bridge_header" "$CANDIDATE_LAB_HEADER_MARKER" \
        "$CANDIDATE_LAB_HEADER_MACRO" \
        --shipping "${KAGEMUSHA_C_SYMBOLS[@]}" \
        --lab "${KAGEMUSHA_CANDIDATE_LAB_C_SYMBOLS[@]}" <<'PY'
from collections import Counter
import re
import sys

path = sys.argv[1]
marker = sys.argv[2]
header_macro = sys.argv[3]
shipping_separator = sys.argv.index("--shipping")
lab_separator = sys.argv.index("--lab")
expected = set(sys.argv[shipping_separator + 1:lab_separator])
expected_lab = set(sys.argv[lab_separator + 1:])
text = open(path, "r", encoding="utf-8").read()


def c_code_mask(source):
    """Mark C tokens while excluding comments and string/character literals."""

    mask = bytearray(b"\x01") * len(source)

    def hide(start, end):
        mask[start:end] = b"\x00" * (end - start)

    cursor = 0
    while cursor < len(source):
        if source.startswith("//", cursor):
            end = source.find("\n", cursor + 2)
            end = len(source) if end < 0 else end
            hide(cursor, end)
            cursor = end
            continue
        if source.startswith("/*", cursor):
            end = source.find("*/", cursor + 2)
            end = len(source) if end < 0 else end + 2
            hide(cursor, end)
            cursor = end
            continue
        quote = None
        literal_start = cursor
        for prefix in ("u8", "L", "u", "U", ""):
            candidate = cursor + len(prefix)
            if (
                source.startswith(prefix, cursor)
                and candidate < len(source)
                and source[candidate] in ('"', "'")
                and (
                    not prefix
                    or cursor == 0
                    or not (source[cursor - 1].isalnum() or source[cursor - 1] == "_")
                )
            ):
                quote = candidate
                break
        if quote is not None:
            delimiter = source[quote]
            end = quote + 1
            while end < len(source):
                if source[end] == "\\":
                    end = min(len(source), end + 2)
                elif source[end] == delimiter:
                    end += 1
                    break
                else:
                    end += 1
            hide(literal_start, end)
            cursor = end
            continue
        cursor += 1
    return mask


code_mask = c_code_mask(text)


def code_matches(pattern, *, start=0, end=None):
    boundary = len(text) if end is None else end
    return [
        match for match in pattern.finditer(text, start, boundary)
        if code_mask[match.start()]
    ]


export_pattern = re.compile(
    r'\b(connect_norito_kagemusha_[A-Za-z0-9_]+)\s*\(',
)
export_matches = code_matches(export_pattern)
export_counts = Counter(match.group(1) for match in export_matches)
lab_export_matches = [
    match for match in export_matches if "_candidate_lab_" in match.group(1)
]
lab_export_counts = Counter(match.group(1) for match in lab_export_matches)
guard_pattern = re.compile(
    rf'(?m)^#ifdef[ \t]+{re.escape(header_macro)}[ \t]*$'
)
guard_matches = code_matches(guard_pattern)
marker_declaration = f"extern const uint8_t {marker}[];"
marker_pattern = re.compile(re.escape(marker_declaration))
marker_matches = code_matches(marker_pattern)
define_pattern = re.compile(
    rf'(?m)^#[ \t]*define[ \t]+{re.escape(header_macro)}\b'
)
define_matches = code_matches(define_pattern)
lab_present = bool(
    lab_export_counts or guard_matches or marker_matches or define_matches
)
errors = []

if lab_present:
    observed = set(lab_export_counts)
    missing_lab = sorted(expected_lab - observed)
    unexpected_lab = sorted(observed - expected_lab)
    duplicate_lab = sorted(
        name for name, count in lab_export_counts.items() if count != 1
    )
    if missing_lab or unexpected_lab or duplicate_lab:
        errors.append(
            "candidate-lab header inventory is not exact "
            f"(missing={missing_lab}, unexpected={unexpected_lab}, "
            f"non_single_occurrence={duplicate_lab})"
        )
    if len(guard_matches) != 1:
        errors.append(
            "candidate-lab header declarations require one exact non-shipping guard"
        )
    else:
        guard = guard_matches[0]
        end_pattern = re.compile(r'(?m)^#endif[ \t]*$')
        ends = code_matches(end_pattern, start=guard.end())
        if not ends:
            errors.append("candidate-lab header guard is unterminated")
        else:
            guard_end = ends[0]
            nested_pattern = re.compile(
                r'(?m)^#[ \t]*(?:if|ifdef|ifndef|elif|else|endif)\b'
            )
            if code_matches(
                nested_pattern, start=guard.end(), end=guard_end.start()
            ):
                errors.append(
                    "candidate-lab header guard contains a nested preprocessor branch"
                )
            guarded_export_matches = code_matches(
                export_pattern, start=guard.end(), end=guard_end.start()
            )
            guarded_counts = Counter(
                match.group(1) for match in guarded_export_matches
            )
            if guarded_counts != Counter({name: 1 for name in expected_lab}):
                errors.append("candidate-lab header declaration escaped its guard")
            guarded_markers = code_matches(
                marker_pattern, start=guard.end(), end=guard_end.start()
            )
            if len(guarded_markers) != 1 or len(marker_matches) != 1:
                errors.append(
                    "candidate-lab header guard lacks its exact do-not-ship marker"
                )
            outside_lab = [
                match for match in lab_export_matches
                if not (guard.end() <= match.start() < guard_end.start())
            ]
            if outside_lab:
                errors.append("candidate-lab header declaration escaped its guard")
    if define_matches:
        errors.append("bridge header must not enable the candidate-lab macro")
actual = set(export_counts) - set(lab_export_counts)
missing = sorted(expected - actual)
retired_or_extra = sorted(actual - expected)
if missing:
    print(
        "[mobile-sdk-artifacts] ERROR: bridge header is missing Kagemusha declarations: "
        + ", ".join(missing),
        file=sys.stderr,
    )
if retired_or_extra:
    print(
        "[mobile-sdk-artifacts] ERROR: bridge header exposes retired or unexpected "
        "Kagemusha declarations: " + ", ".join(retired_or_extra),
        file=sys.stderr,
    )
for error in errors:
    print("[mobile-sdk-artifacts] ERROR: " + error, file=sys.stderr)
raise SystemExit(1 if missing or retired_or_extra or errors else 0)
PY
    then
      FAILURES=1
    fi
  fi
}

check_swift_kagemusha_source_contract() {
  local source_dir="$ROOT_DIR/IrohaSwift/Sources/IrohaSwift"
  [[ -d "$source_dir" ]] || return

  if ! run_isolated_checker_python - "$source_dir" "${KAGEMUSHA_C_SYMBOLS[@]}" <<'PY'
from pathlib import Path
import re
import sys

root = Path(sys.argv[1])
expected_symbols = set(sys.argv[2:])
files = sorted(root.glob("*.swift"))
text = "\n".join(path.read_text(encoding="utf-8") for path in files)
expected_wrappers = {
    "appendSpendV4",
    "buildRedeemV4",
    "ensureProofBackendAvailableV4",
    "initSpendV4",
    "prepareRedemptionChangeV4",
    "verifySpendV4",
}
expected_native_lifecycle = {
    "kagemushaRecursiveSpendAppendV4",
    "kagemushaRecursiveSpendArtifactBeginV4",
    "kagemushaRecursiveSpendArtifactCancelV4",
    "kagemushaRecursiveSpendArtifactFinalizeV4",
    "kagemushaRecursiveSpendArtifactSetInstallV4",
    "kagemushaRecursiveSpendArtifactSetIsInstalledV4",
    "kagemushaRecursiveSpendArtifactSetUninstallV4",
    "kagemushaRecursiveSpendArtifactWriteV4",
    "kagemushaRecursiveSpendCapabilitiesV4",
    "kagemushaRecursiveSpendInitV4",
    "kagemushaRecursiveSpendRedeemV4",
    "kagemushaRecursiveSpendRedemptionChangePrepareV4",
    "kagemushaRecursiveSpendVerifyV4",
}
actual_symbols = set(re.findall(
    r'"(connect_norito_kagemusha_[a-z0-9_]+)"',
    text,
))
actual_wrappers = set(re.findall(
    r"\bfunc\s+((?:ensureProofBackendAvailable|initSpend|appendSpend|verifySpend|"
    r"buildRedeem|prepareRedemptionChange)V[0-9]+)\s*\(",
    text,
))
actual_native_lifecycle = set(re.findall(
    r"\bfunc\s+(kagemushaRecursiveSpend(?:Capabilities|Init|Append|Verify|Redeem|"
    r"RedemptionChangePrepare|"
    r"Artifact(?:Begin|Write|Finalize|Cancel|SetInstall|SetIsInstalled|SetUninstall))"
    r"V[0-9]+)\s*\(",
    text,
))
inventories = (
    ("native symbol", actual_symbols, expected_symbols),
    ("lifecycle wrapper", actual_wrappers, expected_wrappers),
    ("native lifecycle binding", actual_native_lifecycle, expected_native_lifecycle),
)
errors = []
for label, actual, expected in inventories:
    missing = sorted(expected - actual)
    retired_or_extra = sorted(actual - expected)
    if missing or retired_or_extra:
        errors.append(
            f"Swift Kagemusha {label} inventory is not exact ABI-22/V4 "
            f"(missing={missing}, retired_or_unexpected={retired_or_extra})"
        )
if re.search(r"\bpublic\s+(?:struct|enum|class|typealias|protocol)\s+[A-Za-z0-9_]*V3\b", text):
    errors.append("Swift SDK retains a public retired V3 schema carrier")
if re.search(
    r"\bpublic\s+static\s+func\s+(?:initSpend|appendSpend|verifySpend|buildRedeem)\s*\(",
    text,
):
    errors.append("Swift SDK retains an unversioned retired lifecycle wrapper")
if "redemptionChange(spendKey:" in text or re.search(
    r"redemptionChange[\s\S]{0,300}?defaultDiversifier\(\)", text
):
    errors.append("Swift SDK lets callers fabricate partial-redemption rho or diversifier")
if not re.search(
    r"kagemushaRecursiveSpendRedemptionChangePrepareV4[\s\S]{0,2200}?"
    r"connect_norito_kagemusha_secret_free_buffer[\s\S]{0,1600}?"
    r"copyKagemushaNativeSecretArchiveOutput",
    text,
):
    errors.append("Swift redemption-change output is not bound to secure native deallocation")
for error in errors:
    print(
        "[mobile-sdk-artifacts] ERROR: " + error,
        file=sys.stderr,
    )
raise SystemExit(1 if errors else 0)
PY
  then
    FAILURES=1
  fi
}

check_android_kagemusha_source_contract() {
  local rust_source="$ROOT_DIR/crates/connect_norito_bridge/src/lib.rs"
  local rust_platform_source="$ROOT_DIR/crates/connect_norito_bridge/src/platform_jni.rs"
  local rust_exports_source="$ROOT_DIR/crates/connect_norito_bridge/src/platform_jni/part_3.rs"
  local kotlin_source="$ROOT_DIR/kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"
  local java_source="$ROOT_DIR/java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"
  local android_keymint_source="$ROOT_DIR/java/iroha_android/android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaAndroidKeyMint.java"

  if [[ -f "$rust_source" ]]; then
    if [[ ! -f "$rust_platform_source" || ! -f "$rust_exports_source" ]]; then
      fail "Rust bridge Kagemusha JNI source closure is incomplete"
    elif ! run_isolated_checker_python - \
      "$rust_source" "$rust_platform_source" "$rust_exports_source" -- \
      "${KAGEMUSHA_JNI_METHODS[@]}" <<'PY'
from pathlib import Path
import re
import sys

separator = sys.argv.index("--")
lib_path, platform_path, exports_path = map(Path, sys.argv[1:separator])
expected = set(sys.argv[separator + 1:])
lib_text = lib_path.read_text(encoding="utf-8")
platform_text = platform_path.read_text(encoding="utf-8")
text = exports_path.read_text(encoding="utf-8")
errors = []

if not re.search(r"\bmod\s+platform_jni\s*;", lib_text):
    errors.append("Rust bridge does not compile the platform_jni module")
for part in ("part_1.rs", "part_2.rs", "part_3.rs"):
    if f'include!("platform_jni/{part}");' not in platform_text:
        errors.append(f"Rust platform_jni source closure omits {part}")

invocation_marker = "kagemusha_sdk_android_forwarders! {"
end_marker = "\npub(super) fn ensure_min_array_length"
if invocation_marker not in text or end_marker not in text:
    errors.append("Rust bridge Kagemusha paired-forwarder inventory is unavailable")
    actual = set()
else:
    body = text.split(invocation_marker, 1)[1].split(end_marker, 1)[0]
    actual = set(re.findall(r"^\s*(native[A-Za-z0-9_]+)\s*\{", body, re.MULTILINE))

explicit = {"nativeBridgeAbiVersion", "nativePastaCycleV4BackendAvailable"}
actual.update(explicit)
missing = sorted(expected - actual)
retired_or_extra = sorted(actual - expected)
if missing:
    errors.append(
        "Rust bridge is missing Kagemusha JNI exports: " + ", ".join(missing)
    )
if retired_or_extra:
    errors.append(
        "Rust bridge exposes retired or unexpected Kagemusha JNI exports: "
        + ", ".join(retired_or_extra)
    )

for namespace in ("sdk", "android"):
    prefix = (
        f'"Java_org_hyperledger_iroha_{namespace}_offline_'
        'KagemushaRecursiveSpendProver_"'
    )
    if prefix not in text:
        errors.append(f"Rust paired-forwarder macro omits the {namespace} JNI namespace")
for method in explicit:
    for namespace in ("sdk", "android"):
        symbol = (
            f"Java_org_hyperledger_iroha_{namespace}_offline_"
            f"KagemushaRecursiveSpendProver_{method}"
        )
        if symbol not in text:
            errors.append(f"Rust bridge is missing explicit JNI export {symbol}")

direct_symbols = set(re.findall(
    r"\bfn\s+(Java_org_hyperledger_iroha_(?:sdk|android)_offline_"
    r"KagemushaRecursiveSpendProver_[A-Za-z0-9_]+)\s*\(",
    "\n".join((lib_text, platform_text, text)),
))
expected_direct_symbols = {
    (
        f"Java_org_hyperledger_iroha_{namespace}_offline_"
        f"KagemushaRecursiveSpendProver_{method}"
    )
    for namespace in ("sdk", "android")
    for method in explicit
}
unexpected_direct_symbols = sorted(direct_symbols - expected_direct_symbols)
missing_direct_symbols = sorted(expected_direct_symbols - direct_symbols)
if unexpected_direct_symbols:
    errors.append(
        "Rust bridge exposes retired or unexpected Kagemusha JNI exports: "
        + ", ".join(unexpected_direct_symbols)
    )
if missing_direct_symbols:
    errors.append(
        "Rust bridge is missing explicit Kagemusha JNI exports: "
        + ", ".join(missing_direct_symbols)
    )

for error in errors:
    print(f"[mobile-sdk-artifacts] ERROR: {error}", file=sys.stderr)
raise SystemExit(1 if errors else 0)
PY
    then
      FAILURES=1
    fi
  fi

  if [[ -f "$kotlin_source" || -f "$java_source" ]]; then
    if ! run_isolated_checker_python - "$kotlin_source" "$java_source" -- "${KAGEMUSHA_JNI_METHODS[@]}" <<'PY'
from pathlib import Path
import re
import sys

separator = sys.argv.index("--")
paths = [Path(raw) for raw in sys.argv[1:separator] if Path(raw).is_file()]
expected_native = set(sys.argv[separator + 1:])
expected_wrappers = {"initSpendV4", "appendSpendV4", "verifySpendV4", "buildRedeemV4"}
errors = []
for path in paths:
    text = path.read_text(encoding="utf-8")
    if path.suffix == ".kt":
        actual_native = set(re.findall(
            r"\bprivate\s+external\s+fun\s+(native[A-Za-z0-9_]+)\s*\(",
            text,
        ))
        actual_wrappers = set(re.findall(
            r"\bfun\s+((?:initSpend|appendSpend|verifySpend|buildRedeem)V[0-9]+)\s*\(",
            text,
        ))
    else:
        actual_native = set(re.findall(
            r"\bprivate\s+static\s+native\s+[A-Za-z0-9_<>?,\[\].]+\s+"
            r"(native[A-Za-z0-9_]+)\s*\(",
            text,
        ))
        actual_wrappers = set(re.findall(
            r"\b(?:public\s+)?(?:static\s+)?[A-Za-z0-9_<>?,\[\].]+\s+"
            r"((?:initSpend|appendSpend|verifySpend|buildRedeem)V[0-9]+)\s*\(",
            text,
        ))
    for label, actual, expected in (
        ("native method", actual_native, expected_native),
        ("lifecycle wrapper", actual_wrappers, expected_wrappers),
    ):
        missing = sorted(expected - actual)
        retired_or_extra = sorted(actual - expected)
        if missing or retired_or_extra:
            errors.append(
                f"{path}: {label} inventory is not exact ABI-22/V4 "
                f"(missing={missing}, retired_or_unexpected={retired_or_extra})"
            )
    if re.search(r"\b(?:data\s+class|class|interface|record|enum)\s+[A-Za-z0-9_]*V3\b", text):
        errors.append(f"{path}: public retired V3 schema carrier")
for error in errors:
    print(f"[mobile-sdk-artifacts] ERROR: {error}", file=sys.stderr)
raise SystemExit(1 if errors else 0)
PY
    then
      FAILURES=1
    fi
  fi

  if [[ ! -f "$android_keymint_source" ]]; then
    fail "physical Android Kagemusha KeyMint integration source is missing"
  elif ! run_isolated_checker_python - "$android_keymint_source" <<'PY'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
required = (
    "PackageManager.FEATURE_KEYSTORE_SINGLE_USE_KEY",
    "KeyProperties.KEY_ALGORITHM_EC",
    'CURVE_NAME = "secp256r1"',
    "KeyProperties.PURPOSE_SIGN",
    "KeyProperties.DIGEST_SHA256",
    ".setAttestationChallenge(request.challenge())",
    ".setMaxUsageCount(1)",
    'SIGNATURE_ALGORITHM = "SHA256withECDSA"',
    "StrongBoxPolicy.REQUIRED",
    "builder.setIsStrongBoxBacked(true)",
    "keyInfo.isInsideSecureHardware()",
    "keyInfo.getRemainingUsageCount() != 1",
    "getCertificateChain(request.alias())",
    "DeviceAttestationRegistration.androidPreKeyGenerationChallengeHash",
    "requiredPreparation.signingBytes()",
    "KagemushaP256Codec.rawLowSFromStrictDer(signatureDer)",
)
errors = [f"missing {marker!r}" for marker in required if marker not in text]
if "KeyProperties.DIGEST_NONE" in text:
    errors.append("physical KeyMint path uses DIGEST_NONE")
if "PREFERRED" in text:
    errors.append("physical KeyMint path exposes a silent StrongBox preference/downgrade")
if re.search(
    r"generateRegistration\s*\([\s\S]{0,1800}?"
    r"requiredParameters\.attestationChallenge\(\)"
    r"[\s\S]{0,900}?requiredParameters\.registration\(material\)",
    text,
) is None:
    errors.append("registration does not derive and bind the exact pre-key challenge")
if re.search(
    r"authorize\s*\([\s\S]{0,1800}?requiredPreparation\.signingBytes\(\)"
    r"[\s\S]{0,900}?finalizeRequestAuthorization\s*\("
    r"[\s\S]{0,180}?requiredPreparation,\s*signatureDer",
    text,
) is None:
    errors.append("authorization does not sign and finalize the exact preparation")
for error in errors:
    print(f"[mobile-sdk-artifacts] ERROR: {path}: {error}", file=sys.stderr)
raise SystemExit(1 if errors else 0)
PY
  then
    FAILURES=1
  fi
}

require_plist_slice() {
  local plist="$1"
  local slice="$2"
  if ! plist_contains "$plist" "$slice"; then
    fail "Info.plist does not list XCFramework slice $slice"
  fi
}

check_swift_package() {
  local package="$ROOT_DIR/IrohaSwift/Package.swift"
  local resolved="$ROOT_DIR/IrohaSwift/Package.resolved"

  require_file "$package" "Swift package manifest"
  require_file "$resolved" "Swift package resolution lock"
  if [[ -L "$resolved" ]]; then
    fail "Swift package resolution lock must be a non-symbolic regular file: $(relpath "$resolved")"
  fi
  require_dir "$ROOT_DIR/IrohaSwift/Sources/IrohaSwift" "IrohaSwift sources"
  require_literal "$package" 'name: "IrohaSwift"' "IrohaSwift package name"
  require_literal "$package" '.binaryTarget(' "NoritoBridge binary target declaration"
  require_literal "$package" 'name: "NoritoBridge"' "NoritoBridge binary target name"
  require_literal "$package" '.iOS(.v15)' "IrohaSwift iOS platform floor"
  require_literal "$package" '.macOS(.v12)' "IrohaSwift macOS platform floor"
  require_literal "$package" '"MOBILE_SDK_APPLE_ARTIFACT_DIR"' "external NoritoBridge artifact directory input"
  require_literal "$package" 'path: bridgeTargetPath' "resolved NoritoBridge artifact path"
}

check_xcframework() {
  local xcframework="$APPLE_ARTIFACT_DIR/NoritoBridge.xcframework"
  local info="$xcframework/Info.plist"
  local manifest="$APPLE_ARTIFACT_DIR/NoritoBridge.artifacts.json"
  local embedded_manifest="$xcframework/NoritoBridge.artifacts.json"
  local checked_in_swift_loader="$ROOT_DIR/IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
  local swift_loader="$checked_in_swift_loader"
  local privacy_marker="$xcframework/.privacy-production-enabled"
  local test_only_prebuilt_marker="$xcframework/.test-only-prebuilt-slices"
  local slices=(ios-arm64 ios-arm64_x86_64-simulator macos-arm64_x86_64)
  local slice

  require_dir "$xcframework" "NoritoBridge XCFramework"
  require_file "$info" "NoritoBridge XCFramework metadata"

  for slice in "${slices[@]}"; do
    local slice_dir="$xcframework/$slice"
    local headers_dir="$slice_dir/Headers"
    require_plist_slice "$info" "$slice"
    require_dir "$slice_dir" "XCFramework slice directory"
    if [[ -d "$slice_dir" ]]; then
      require_file "$slice_dir/libNoritoBridge.a" "XCFramework slice binary"
      reject_candidate_lab_content \
        "$slice_dir/libNoritoBridge.a" \
        "NoritoBridge $slice production binary"
      require_dir "$headers_dir" "XCFramework slice headers"
      if [[ -d "$headers_dir" ]]; then
        require_file "$headers_dir/NoritoBridge.h" "XCFramework slice header"
        require_file "$headers_dir/connect_norito_bridge.h" "XCFramework bridge C header"
        require_file "$headers_dir/module.modulemap" "XCFramework module map"
      fi
    fi
  done
  if ! run_isolated_checker_python - "$info" <<'PY'
from pathlib import Path
import plistlib
import sys


expected = {
    "ios-arm64": {
        "architectures": ["arm64"],
        "platform": "ios",
        "variant": None,
    },
    "ios-arm64_x86_64-simulator": {
        "architectures": ["arm64", "x86_64"],
        "platform": "ios",
        "variant": "simulator",
    },
    "macos-arm64_x86_64": {
        "architectures": ["arm64", "x86_64"],
        "platform": "macos",
        "variant": None,
    },
}
try:
    with Path(sys.argv[1]).open("rb") as handle:
        payload = plistlib.load(handle)
    libraries = payload.get("AvailableLibraries")
    valid = (
        isinstance(payload, dict)
        and payload.get("CFBundlePackageType") == "XFWK"
        and payload.get("XCFrameworkFormatVersion") == "1.0"
        and isinstance(libraries, list)
        and len(libraries) == len(expected)
        and all(isinstance(library, dict) for library in libraries)
    )
    indexed = {
        library.get("LibraryIdentifier"): library
        for library in libraries
    } if valid else {}
    valid = valid and len(indexed) == len(libraries) and set(indexed) == set(expected)
    if valid:
        for identifier, contract in expected.items():
            library = indexed[identifier]
            valid = (
                library.get("LibraryPath") == "libNoritoBridge.a"
                and (
                    "BinaryPath" not in library
                    or library["BinaryPath"] == "libNoritoBridge.a"
                )
                and library.get("HeadersPath") == "Headers"
                and library.get("SupportedArchitectures")
                == contract["architectures"]
                and library.get("SupportedPlatform") == contract["platform"]
                and (
                    (
                        contract["variant"] is None
                        and "SupportedPlatformVariant" not in library
                    )
                    or library.get("SupportedPlatformVariant")
                    == contract["variant"]
                )
            )
            if not valid:
                break
except (OSError, plistlib.InvalidFileException, TypeError, ValueError):
    valid = False
raise SystemExit(0 if valid else 1)
PY
  then
    fail "NoritoBridge Info.plist does not declare the canonical universal Apple slices"
  fi

  require_file "$embedded_manifest" "embedded NoritoBridge artifact manifest"
  require_file "$checked_in_swift_loader" "IrohaSwift native bridge loader"
  if [[ ! -L "$manifest" ]]; then
    fail "public NoritoBridge artifact manifest must be a relative symlink"
  elif [[ "$(readlink "$manifest")" != "NoritoBridge.xcframework/NoritoBridge.artifacts.json" ]]; then
    fail "public NoritoBridge artifact manifest has a non-canonical symlink target"
  fi
  require_file "$manifest" "NoritoBridge artifact manifest"
  if [[ -e "$test_only_prebuilt_marker" || -L "$test_only_prebuilt_marker" ]]; then
    fail "NoritoBridge release artifact must not carry the test-only prebuilt-slices marker"
  fi
  if [[ -f "$manifest" ]]; then
    if ! run_isolated_checker_python - "$manifest" <<'PY'
import json
from pathlib import Path
import sys

try:
    payload = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
except (OSError, UnicodeError, ValueError):
    # The canonical manifest parser below reports malformed input.
    raise SystemExit(0)
raise SystemExit(
    1
    if isinstance(payload, dict) and "test_only_prebuilt_slices" in payload
    else 0
)
PY
    then
      fail "NoritoBridge release artifact manifest must not contain test_only_prebuilt_slices"
    fi
    local prospective_validation="${MOBILE_SDK_STAGED_BUILD_VALIDATION:-0}"
    local prospective_loader="${MOBILE_SDK_PROSPECTIVE_SWIFT_LOADER_PATH:-}"
    if [[ "$prospective_validation" != "0" || -n "$prospective_loader" ]]; then
      if [[ "$prospective_validation" != "1" || -z "$prospective_loader" ]]; then
        fail "prospective Swift loader validation requires both internal builder inputs"
      else
        local prospective_result prospective_manifest_commit
        prospective_manifest_commit="$(
          manifest_json_value "$manifest" source_commit 2>/dev/null || true
        )"
        prospective_result="$(
          run_isolated_checker_python \
            "$ROOT_DIR/scripts/check_mobile_sdk_artifact_pin_commit.py" \
            --root "$ROOT_DIR" \
            --manifest-commit "$prospective_manifest_commit" \
            --artifact-root "$APPLE_ARTIFACT_DIR" \
            --prospective-loader "$prospective_loader" \
            2>/dev/null
        )" || true
        if [[ "$prospective_result" != "prospective" ]]; then
          fail "builder-private prospective Swift loader is not a three-digest-only projection"
        else
          swift_loader="$prospective_loader"
        fi
      fi
    fi
    if ! run_isolated_checker_python \
      "$ROOT_DIR/scripts/validate_norito_bridge_xcframework.py" \
      --root "$ROOT_DIR" \
      --lockfile-path "$APPLE_CARGO_LOCKFILE" \
      --xcframework "$xcframework" \
      --manifest "$embedded_manifest" \
      --manifest-link "$manifest" \
      --expected-link-target "NoritoBridge.xcframework/NoritoBridge.artifacts.json" \
      --swift-loader "$swift_loader"; then
      fail "NoritoBridge published artifact inventory is not exact"
    fi
    if ! run_isolated_checker_python - "$manifest" "$ROOT_DIR/scripts/run_mobile_hermetic_command.py" <<'PY'
from pathlib import Path
import hashlib
import json
import re
import sys


def reject_duplicates(pairs):
    value = {}
    for key, child in pairs:
        if key in value:
            raise ValueError(f"duplicate JSON member: {key}")
        value[key] = child
    return value


manifest = json.loads(
    Path(sys.argv[1]).read_text(encoding="utf-8"),
    object_pairs_hook=reject_duplicates,
)
environment = manifest.get("build_environment")
expected_fields = {
    "schema",
    "hermetic_runner_schema",
    "hermetic_runner_sha256",
    "environment_profiles",
    "cargo_build_jobs",
    "rust_toolchain_channel",
    "cargo_release",
    "cargo_commit_hash",
    "cargo_binary_sha256",
    "rustc_release",
    "rustc_commit_hash",
    "rustc_binary_sha256",
    "rustdoc_release",
    "rustdoc_commit_hash",
    "rustdoc_binary_sha256",
    "python_version",
    "python_binary_sha256",
    "git_version",
    "git_binary_sha256",
    "rustup_version",
    "rustup_binary_sha256",
    "xcode_version",
    "xcode_build_version",
    "iphoneos_sdk_version",
    "iphonesimulator_sdk_version",
    "macosx_sdk_version",
    "iphoneos_deployment_target",
    "iphonesimulator_deployment_target",
    "macosx_deployment_target",
}
if not isinstance(environment, dict) or set(environment) != expected_fields:
    raise SystemExit(1)
common = {
    "CARGO",
    "CARGO_BUILD_JOBS",
    "CARGO_HOME",
    "CARGO_INCREMENTAL",
    "CARGO_NET_OFFLINE",
    "CARGO_TARGET_DIR",
    "HOME",
    "LANG",
    "LC_ALL",
    "NORITO_SKIP_BINDINGS_SYNC",
    "PATH",
    "RUSTC",
    "RUSTC_BOOTSTRAP",
    "RUSTDOC",
    "RUSTUP_HOME",
    "TMPDIR",
}
expected_profiles = {
    "apple-ios-device": sorted(
        common | {"DEVELOPER_DIR", "IPHONEOS_DEPLOYMENT_TARGET", "SDKROOT"}
    ),
    "apple-ios-simulator": sorted(
        common
        | {
            "DEVELOPER_DIR",
            "IPHONEOS_DEPLOYMENT_TARGET",
            "IPHONESIMULATOR_DEPLOYMENT_TARGET",
            "SDKROOT",
        }
    ),
    "apple-macos": sorted(
        common | {"DEVELOPER_DIR", "MACOSX_DEPLOYMENT_TARGET", "SDKROOT"}
    ),
}
if environment["environment_profiles"] != expected_profiles:
    raise SystemExit(1)
if (
    environment["schema"] != "iroha.mobile-native-build-environment.v1"
    or environment["hermetic_runner_schema"] != "iroha.mobile-hermetic-command.v1"
    or environment["rust_toolchain_channel"] != "1.93.1"
    or environment["cargo_release"] != "1.93.1"
    or environment["rustc_release"] != "1.93.1"
    or environment["rustdoc_release"] != "1.93.1"
    or environment["cargo_build_jobs"] != 1
    or environment["rustdoc_commit_hash"] != environment["rustc_commit_hash"]
    or environment["iphoneos_deployment_target"] != "15.0"
    or environment["iphonesimulator_deployment_target"] != "15.0"
    or environment["macosx_deployment_target"] != "12.0"
):
    raise SystemExit(1)
sha256 = re.compile(r"^[0-9a-f]{64}$")
commit = re.compile(r"^[0-9a-f]{40}$")
version = re.compile(r"^[0-9]+(?:\.[0-9]+){1,2}$")
hashes = manifest.get("hashes")
expected_hash_keys = {
    "ios-arm64",
    "ios-arm64_x86_64-simulator",
    "macos-arm64_x86_64",
}
if (
    not isinstance(hashes, dict)
    or set(hashes) != expected_hash_keys
    or any(not isinstance(value, str) or not sha256.fullmatch(value) for value in hashes.values())
):
    raise SystemExit(1)
for field in (
    "hermetic_runner_sha256",
    "cargo_binary_sha256",
    "rustc_binary_sha256",
    "rustdoc_binary_sha256",
    "python_binary_sha256",
    "git_binary_sha256",
    "rustup_binary_sha256",
):
    if not isinstance(environment[field], str) or not sha256.fullmatch(environment[field]):
        raise SystemExit(1)
for field in ("cargo_commit_hash", "rustc_commit_hash", "rustdoc_commit_hash"):
    if not isinstance(environment[field], str) or not commit.fullmatch(environment[field]):
        raise SystemExit(1)
if re.fullmatch(r"3\.12\.[0-9]+", environment["python_version"]) is None:
    raise SystemExit(1)
for field in (
    "git_version",
    "rustup_version",
    "xcode_version",
    "rustdoc_release",
    "iphoneos_sdk_version",
    "iphonesimulator_sdk_version",
    "macosx_sdk_version",
):
    if not isinstance(environment[field], str) or not version.fullmatch(environment[field]):
        raise SystemExit(1)
if not isinstance(environment["xcode_build_version"], str) or not re.fullmatch(
    r"[A-Za-z0-9.]+", environment["xcode_build_version"]
):
    raise SystemExit(1)
runner = Path(sys.argv[2])
if runner.is_file() and hashlib.sha256(runner.read_bytes()).hexdigest() != environment[
    "hermetic_runner_sha256"
]:
    raise SystemExit(1)
PY
    then
      fail "NoritoBridge artifact build_environment is missing, malformed, or not hermetic"
    fi
    local privacy_keys=()
    local privacy_declarations=()
    local privacy_key
    local privacy_declaration
    local privacy_value
    while IFS= read -r privacy_key; do
      privacy_keys+=("$privacy_key")
    done < <(
      grep -Eo '"privacy_production_enabled"[[:space:]]*:' "$manifest" || true
    )
    while IFS= read -r privacy_declaration; do
      privacy_declarations+=("$privacy_declaration")
    done < <(
      grep -Eo '"privacy_production_enabled"[[:space:]]*:[[:space:]]*(true|false)' \
        "$manifest" || true
    )
    if [[ ${#privacy_keys[@]} -ne 1 || ${#privacy_declarations[@]} -ne 1 ]]; then
      fail "NoritoBridge artifact manifest must contain exactly one boolean privacy_production_enabled field"
    else
      privacy_value="${privacy_declarations[0]##*:}"
      privacy_value="${privacy_value//[[:space:]]/}"
      if [[ "$privacy_value" == "true" ]]; then
        require_file "$privacy_marker" "privacy-production-enabled XCFramework marker"
      elif [[ -e "$privacy_marker" ]]; then
        fail "default privacy artifact must not carry the privacy-production-enabled XCFramework marker"
      fi
      if ! run_isolated_checker_python - "$manifest" "$privacy_value" <<'PY'
import json
import sys


def reject_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON member")
        result[key] = value
    return result


try:
    with open(sys.argv[1], "r", encoding="utf-8") as handle:
        payload = json.load(handle, object_pairs_hook=reject_duplicates)
    privacy_production_enabled = sys.argv[2] == "true"
    expected = ["privacy-production-enabled"] if privacy_production_enabled else []
    valid = isinstance(payload, dict) and payload.get("cargo_features") == expected
except (OSError, UnicodeError, ValueError, TypeError):
    valid = False
raise SystemExit(0 if valid else 1)
PY
      then
        if [[ "$privacy_value" == "true" ]]; then
          fail 'privacy-production NoritoBridge artifact cargo_features must be exactly ["privacy-production-enabled"]'
        else
          fail "default NoritoBridge artifact cargo_features must be exactly []"
        fi
      fi
    fi
    require_regex "$manifest" '"version"[[:space:]]*:[[:space:]]*"[^"]+"' "NoritoBridge artifact version"
    for slice in "${slices[@]}"; do
      require_regex "$manifest" "\"$slice\"[[:space:]]*:[[:space:]]*\"[[:xdigit:]]{64}\"" "NoritoBridge artifact manifest hash for $slice"
      if [[ -f "$xcframework/$slice/libNoritoBridge.a" ]]; then
        local expected_hash actual_hash
        expected_hash="$(manifest_json_value "$manifest" "hashes.$slice" 2>/dev/null || true)"
        actual_hash="$(hash_file "$xcframework/$slice/libNoritoBridge.a")"
        if [[ "$expected_hash" != "$actual_hash" ]]; then
          fail "NoritoBridge artifact hash mismatch for $slice"
        fi
        if [[ -f "$swift_loader" ]]; then
          require_literal \
            "$swift_loader" \
            "\"$slice\": \"$expected_hash\"" \
            "IrohaSwift manifestless fallback hash for $slice"
        fi
      fi
    done

    require_regex "$manifest" '"native_bridge_abi_version"[[:space:]]*:[[:space:]]*22([[:space:]]*[,}])' "exact first-release NoritoBridge ABI 22"
    require_regex "$manifest" '"source_commit"[[:space:]]*:[[:space:]]*"[[:xdigit:]]{40}"' "NoritoBridge source commit"
    require_regex "$manifest" '"source_tree_dirty"[[:space:]]*:[[:space:]]*(true|false)' "NoritoBridge source dirty state"
    require_regex "$manifest" '"source_fingerprint_sha256"[[:space:]]*:[[:space:]]*"[[:xdigit:]]{64}"' "NoritoBridge source fingerprint"
    require_regex "$manifest" '"cargo_lock_sha256"[[:space:]]*:[[:space:]]*"[0-9a-f]{64}"' "NoritoBridge selected Cargo lock hash"
    require_regex "$manifest" '"bridge_header_sha256"[[:space:]]*:[[:space:]]*"[[:xdigit:]]{64}"' "NoritoBridge header hash"
    local manifest_lock_hash selected_lock_hash
    manifest_lock_hash="$(manifest_json_value "$manifest" cargo_lock_sha256 2>/dev/null || true)"
    if [[ -n "$APPLE_CARGO_LOCKFILE" ]]; then
      if ! assert_apple_cargo_lock; then
        fail "selected Apple Cargo lock identity is no longer authenticated"
      else
        selected_lock_hash="${APPLE_CARGO_LOCK_SEAL_START%%:*}"
      fi
      if [[ -n "$selected_lock_hash" && "$manifest_lock_hash" != "$selected_lock_hash" ]]; then
        fail "NoritoBridge artifact selected Cargo lock digest does not match checkout"
      fi
    fi
    local manifest_dirty
    manifest_dirty="$(manifest_json_value "$manifest" source_tree_dirty 2>/dev/null || true)"
    if [[ "$manifest_dirty" != "false" && "$ALLOW_DIRTY_SOURCE" != "1" ]]; then
      fail "NoritoBridge release artifact must be built from a clean source tree"
    fi
    if ! run_isolated_checker_python - "$manifest" "${REQUIRED_BRIDGE_SYMBOLS[@]}" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
expected = sys.argv[2:]
actual = payload.get("required_symbols")
raise SystemExit(0 if actual == expected else 1)
PY
    then
      fail "NoritoBridge artifact required symbol inventory is missing or non-canonical"
    fi
    if ! run_isolated_checker_python - "$manifest" "${FORBIDDEN_MOBILE_BRIDGE_SYMBOLS[@]}" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
expected = sys.argv[2:]
actual = payload.get("forbidden_symbols")
raise SystemExit(0 if actual == expected else 1)
PY
    then
      fail "NoritoBridge artifact forbidden symbol inventory is missing or non-canonical"
    fi
    if [[ -f "$swift_loader" ]] \
      && ! run_isolated_checker_python - "$swift_loader" "${REQUIRED_BRIDGE_SYMBOLS[@]}" <<'PY'
import re
import sys
from pathlib import Path

contents = Path(sys.argv[1]).read_text(encoding="utf-8")
match = re.search(
    r"private static let requiredSymbols\s*=\s*\[\n(?P<body>.*?)^\s*\]",
    contents,
    re.MULTILINE | re.DOTALL,
)
if match is None:
    raise SystemExit(1)
loader_symbols = []
for raw in match.group("body").splitlines():
    line = raw.strip()
    if not line:
        continue
    item = re.fullmatch(r'"([^"]+)",?', line)
    if item is None:
        raise SystemExit(1)
    loader_symbols.append(item.group(1))
if not loader_symbols or len(loader_symbols) != len(set(loader_symbols)):
    raise SystemExit(1)
release_symbols = set(sys.argv[2:])
raise SystemExit(0 if set(loader_symbols) <= release_symbols else 1)
PY
    then
      fail "NoritoBridge required symbol policy does not cover the Swift loader"
    fi

    local canonical_header="$xcframework/ios-arm64/Headers/connect_norito_bridge.h"
    if [[ -f "$canonical_header" ]]; then
      local manifest_header_hash actual_header_hash
      manifest_header_hash="$(manifest_json_value "$manifest" bridge_header_sha256 2>/dev/null || true)"
      actual_header_hash="$(hash_file "$canonical_header")"
      if [[ "$manifest_header_hash" != "$actual_header_hash" ]]; then
        fail "NoritoBridge artifact header hash mismatch"
      fi
      for slice in "${slices[@]}"; do
        local slice_header="$xcframework/$slice/Headers/connect_norito_bridge.h"
        if [[ -f "$slice_header" && "$(hash_file "$slice_header")" != "$actual_header_hash" ]]; then
          fail "NoritoBridge bridge header differs in $slice"
        fi
      done
    fi

    local bridge_source="$ROOT_DIR/crates/connect_norito_bridge/src/lib.rs"
    if [[ -f "$bridge_source" && -d "$ROOT_DIR/.git" ]]; then
      local source_abi manifest_abi manifest_commit source_relationship source_dirty source_fingerprint manifest_fingerprint
      source_abi="$(run_isolated_checker_python - \
        "$ROOT_DIR/crates/connect_norito_bridge/include/connect_norito_bridge.h" \
        "$bridge_source" \
        "$ROOT_DIR/crates/iroha_data_model/src/privacy/protocol.rs" <<'PY'
from pathlib import Path
import re
import sys

header, bridge, protocol = (Path(value) for value in sys.argv[1:])
header_abis = re.findall(
    r"^#define[ \t]+CONNECT_NORITO_BRIDGE_ABI_VERSION[ \t]+([0-9]+)[ \t]*$",
    header.read_text(encoding="utf-8"),
    re.MULTILINE,
)
bridge_aliases = re.findall(
    r"^const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = "
    r"(PRIVACY_BRIDGE_ABI_VERSION_V1);$",
    bridge.read_text(encoding="utf-8"),
    re.MULTILINE,
)
protocol_abis = re.findall(
    r"^pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = ([0-9]+);$",
    protocol.read_text(encoding="utf-8"),
    re.MULTILINE,
)
if (
    header_abis != ["22"]
    or bridge_aliases != ["PRIVACY_BRIDGE_ABI_VERSION_V1"]
    or protocol_abis != header_abis
):
    raise SystemExit(1)
print(header_abis[0])
PY
)" || source_abi=""
      manifest_abi="$(manifest_json_value "$manifest" native_bridge_abi_version 2>/dev/null || true)"
      if [[ "$source_abi" != "22" || "$manifest_abi" != "22" ]]; then
        fail "NoritoBridge artifact ABI must match exact public-header 22, canonical Rust alias, and privacy protocol constant"
      fi
      manifest_commit="$(manifest_json_value "$manifest" source_commit 2>/dev/null || true)"
      source_relationship="$(
        run_isolated_checker_python \
          "$ROOT_DIR/scripts/check_mobile_sdk_artifact_pin_commit.py" \
          --root "$ROOT_DIR" \
          --manifest-commit "$manifest_commit" \
          2>/dev/null
      )" || true
      if [[ "$source_relationship" != "direct" && "$source_relationship" != "pin-parent" ]]; then
        fail "NoritoBridge artifact source commit is neither HEAD nor its authenticated pin-only parent"
      fi
      source_dirty=false
      if [[ -n "$(bridge_source_status)" ]]; then
        source_dirty=true
      fi
      if [[ "$manifest_dirty" != "$source_dirty" ]]; then
        fail "NoritoBridge artifact source dirty state does not match checkout"
      fi
      if [[ "$source_dirty" != "false" && "$ALLOW_DIRTY_SOURCE" != "1" ]]; then
        fail "NoritoBridge release artifact cannot be certified against a dirty checkout"
      fi
      if [[ "$source_relationship" == "pin-parent" && "$source_dirty" != "false" ]]; then
        fail "NoritoBridge pin-only child commit requires a clean authenticated source closure"
      fi
      source_fingerprint="$(bridge_source_fingerprint)"
      manifest_fingerprint="$(manifest_json_value "$manifest" source_fingerprint_sha256 2>/dev/null || true)"
      if [[ "$manifest_fingerprint" != "$source_fingerprint" ]]; then
        fail "NoritoBridge artifact source fingerprint does not match checkout"
      fi
    fi

    local index symbol actual_arches
    for index in "${!slices[@]}"; do
      slice="${slices[$index]}"
      local binary="$xcframework/$slice/libNoritoBridge.a"
      [[ -f "$binary" ]] || continue
      if ! command -v lipo >/dev/null 2>&1; then
        fail "lipo is required for strict Apple artifact validation"
        break
      fi
      actual_arches="$(lipo -archs "$binary" 2>/dev/null || true)"
      case "$slice" in
        ios-arm64)
          if [[ "$actual_arches" != "arm64" ]]; then
            fail "NoritoBridge $slice architectures must be arm64 (found ${actual_arches:-unreadable})"
          fi
          ;;
        ios-arm64_x86_64-simulator|macos-arm64_x86_64)
          if [[ " $actual_arches " != *" arm64 "* \
            || " $actual_arches " != *" x86_64 "* \
            || "$(wc -w <<<"$actual_arches" | tr -d '[:space:]')" != "2" ]]; then
            fail "NoritoBridge $slice architectures must be arm64 and x86_64 (found ${actual_arches:-unreadable})"
          fi
          ;;
      esac
      if ! command -v nm >/dev/null 2>&1; then
        fail "nm is required for strict Apple artifact validation"
        break
      fi
      local symbols
      symbols="$(nm -gUj "$binary" 2>/dev/null || true)"
      for symbol in "${REQUIRED_BRIDGE_SYMBOLS[@]}"; do
        if ! grep -Eq "^_?${symbol}$" <<<"$symbols"; then
          fail "NoritoBridge $slice is missing required symbol $symbol"
        fi
      done
      for symbol in "${FORBIDDEN_MOBILE_BRIDGE_SYMBOLS[@]}"; do
        if grep -Eq "^_?${symbol}$" <<<"$symbols"; then
          fail "NoritoBridge $slice exports forbidden first-release symbol $symbol"
        fi
      done
      local kagemusha_symbols
      kagemusha_symbols="$(
        grep -E '^_?connect_norito_kagemusha_' <<<"$symbols" || true
      )"
      if ! run_isolated_checker_python - \
        "$kagemusha_symbols" -- "${KAGEMUSHA_C_SYMBOLS[@]}" <<'PY'
import sys

separator = sys.argv.index("--")
captured = sys.argv[1]
expected = set(sys.argv[separator + 1:])
actual = {
    line.strip().removeprefix("_")
    for line in captured.splitlines()
    if line.strip().removeprefix("_").startswith("connect_norito_kagemusha_")
}
raise SystemExit(0 if actual == expected else 1)
PY
      then
        fail "NoritoBridge $slice Kagemusha export inventory is not exact"
      fi
    done
  fi
}

check_gradle_publication() {
  local module="$1"
  local artifact_id="$2"
  local build_file="$ROOT_DIR/kotlin/$module/build.gradle.kts"

  require_file "$build_file" "$module Gradle build file"
  require_regex "$build_file" 'maven-publish' "$module maven-publish plugin"
  require_regex "$build_file" 'group[[:space:]]*=[[:space:]]*"org\.hyperledger\.iroha\.sdk"' "$module Maven group"
  require_regex "$build_file" 'version[[:space:]]*=[[:space:]]*("[^"]+"|providers\.gradleProperty\("irohaSdkVersion"\))' "$module Maven version"
  require_regex "$build_file" 'create<MavenPublication>\("release"\)' "$module release publication"
  require_regex "$build_file" "artifactId[[:space:]]*=[[:space:]]*\"$artifact_id\"" "$module artifact id"
}

find_android_nm() {
  local candidate
  local ndk_root

  if [[ -n "${MOBILE_SDK_ANDROID_NM:-}" ]]; then
    if [[ -x "$MOBILE_SDK_ANDROID_NM" ]]; then
      printf '%s' "$MOBILE_SDK_ANDROID_NM"
      return 0
    fi
    if command -v "$MOBILE_SDK_ANDROID_NM" >/dev/null 2>&1; then
      command -v "$MOBILE_SDK_ANDROID_NM"
      return 0
    fi
    return 1
  fi
  if command -v llvm-nm >/dev/null 2>&1; then
    command -v llvm-nm
    return 0
  fi
  for ndk_root in "${ANDROID_NDK_HOME:-}" "${ANDROID_NDK_ROOT:-}"; do
    [[ -n "$ndk_root" ]] || continue
    while IFS= read -r candidate; do
      if [[ -x "$candidate" ]]; then
        printf '%s' "$candidate"
        return 0
      fi
    done < <(compgen -G "$ndk_root/toolchains/llvm/prebuilt/*/bin/llvm-nm" || true)
  done
  if [[ -n "${ANDROID_HOME:-}" ]]; then
    while IFS= read -r candidate; do
      if [[ -x "$candidate" ]]; then
        printf '%s' "$candidate"
        return 0
      fi
    done < <(compgen -G "$ANDROID_HOME/ndk/*/toolchains/llvm/prebuilt/*/bin/llvm-nm" || true)
  fi
  if command -v nm >/dev/null 2>&1; then
    command -v nm
    return 0
  fi
  return 1
}

check_android_native_symbols() {
  local binary="$1"
  local abi="$2"
  local nm_tool
  local symbols
  local namespace
  local expected_jni=(
    "${VALIDATION_FEE_JNI_SYMBOLS[@]}"
    "${SORAFS_APPEAL_FINANCE_JNI_SYMBOLS[@]}"
    "${PRIVACY_COMPILED_PROFILE_JNI_SYMBOLS[@]}"
    "${NATIVE_SIGNER_JNI_CONTRACT_SYMBOLS[@]}"
  )

  if ! nm_tool="$(find_android_nm)"; then
    fail "llvm-nm (or MOBILE_SDK_ANDROID_NM) is required to inspect client-android $abi native bridge"
    return
  fi
  # Shipping Android libraries are stripped canonically, so the exact public
  # surface lives in the ELF dynamic symbol table rather than .symtab.
  if ! symbols="$("$nm_tool" -D -g --defined-only "$binary" 2>/dev/null)"; then
    if ! symbols="$("$nm_tool" -g --defined-only "$binary" 2>/dev/null)"; then
      if ! symbols="$("$nm_tool" -gj "$binary" 2>/dev/null)"; then
        fail "unable to inspect client-android $abi native bridge with $nm_tool"
        return
      fi
    fi
  fi
  local symbol
  for symbol in "${FORBIDDEN_MOBILE_BRIDGE_SYMBOLS[@]}"; do
    if grep -Eq "(^|[[:space:]])_?${symbol}$" <<<"$symbols"; then
      fail "client-android $abi bridge exports forbidden first-release symbol $symbol"
    fi
  done
  for namespace in org_hyperledger_iroha_sdk_offline org_hyperledger_iroha_android_offline; do
    local method
    for method in "${KAGEMUSHA_JNI_METHODS[@]}"; do
      expected_jni+=("Java_${namespace}_KagemushaRecursiveSpendProver_${method}")
    done
  done
  if ! run_isolated_checker_python - "$abi" \
    "${KAGEMUSHA_C_SYMBOLS[@]}" \
    "${SORAFS_APPEAL_FINANCE_C_SYMBOLS[@]}" \
    "${PRIVACY_COMPILED_PROFILE_C_SYMBOLS[@]}" \
    -- "${expected_jni[@]}" 3<<<"$symbols" <<'PY'
import os
import sys

abi = sys.argv[1]
separator = sys.argv.index("--")
expected_c = set(sys.argv[2:separator])
expected_jni = set(sys.argv[separator + 1:])
expected = expected_c | expected_jni | {"connect_norito_bridge_abi_version"}
actual = set()
for raw in os.fdopen(3):
    fields = raw.strip().split()
    if not fields:
        continue
    symbol = fields[-1].removeprefix("_")
    if (
        symbol == "connect_norito_bridge_abi_version"
        or symbol.startswith("connect_norito_kagemusha_")
        or symbol in expected_c
        or (
            symbol.startswith("Java_org_hyperledger_iroha_")
            and (
                "_KagemushaRecursiveSpendProver_" in symbol
                or "_ValidationFeeConsensusProofBridge_" in symbol
            )
        )
        or symbol in expected_jni
    ):
        actual.add(symbol)
missing = sorted(expected - actual)
retired_or_extra = sorted(actual - expected)
if missing:
    print(
        f"[mobile-sdk-artifacts] ERROR: client-android {abi} bridge is missing "
        "ABI22/V4 symbols: " + ", ".join(missing),
        file=sys.stderr,
    )
if retired_or_extra:
    print(
        f"[mobile-sdk-artifacts] ERROR: client-android {abi} bridge exposes retired "
        "or unexpected Kagemusha symbols: " + ", ".join(retired_or_extra),
        file=sys.stderr,
    )
raise SystemExit(1 if missing or retired_or_extra else 0)
PY
  then
    FAILURES=1
  fi
}

check_android_native_stripped() {
  local binary="$1"
  local abi="$2"
  local description

  if ! command -v file >/dev/null 2>&1; then
    fail "file is required to verify that the client-android $abi native bridge is stripped"
    return
  fi
  if ! description="$(file -b "$binary" 2>/dev/null)"; then
    fail "unable to inspect client-android $abi native bridge file type"
    return
  fi
  if ! grep -Eq '(^|, )stripped(,|$)' <<<"$description"; then
    fail "client-android $abi native bridge is not canonically stripped"
  fi
}

check_android_native_provenance() {
  local client_aar="$1"
  local source_seal_python=""
  local source_seal_home=""
  local source_seal_cargo_home=""
  local source_seal_rustup_home=""
  local source_seal_tmpdir=""
  local source_seal_cargo=""
  local source_seal_rustc=""
  local source_seal_rustdoc=""
  local source_seal_cargo_target_dir=""

  if [[ -d "$ROOT_DIR/.git" ]]; then
    initialize_source_seal_tools
    source_seal_python="$CHECK_PYTHON_BINARY"
    source_seal_home="$CHECK_USER_HOME_DIR"
    source_seal_cargo_home="$SOURCE_SEAL_CARGO_HOME"
    source_seal_rustup_home="$SOURCE_SEAL_RUSTUP_HOME"
    source_seal_tmpdir="$CHECK_TMPDIR"
    source_seal_cargo="$SOURCE_SEAL_CARGO_BINARY"
    source_seal_rustc="$SOURCE_SEAL_RUSTC_BINARY"
    source_seal_rustdoc="$SOURCE_SEAL_RUSTDOC_BINARY"
    source_seal_cargo_target_dir="$SOURCE_SEAL_CARGO_TARGET_DIR"
  fi

  run_isolated_checker_python - \
    "$ROOT_DIR" "$client_aar" "$ANDROID_NATIVE_PROVENANCE_ENTRY" \
    "$ANDROID_CLIENT_BUILD_DIR" "$ALLOW_DIRTY_SOURCE" \
    "$source_seal_python" "$source_seal_home" \
    "$source_seal_cargo_home" "$source_seal_rustup_home" \
    "$source_seal_tmpdir" "$source_seal_cargo" "$source_seal_rustc" \
    "$source_seal_rustdoc" "$source_seal_cargo_target_dir" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import zipfile

root = Path(sys.argv[1])
aar_path = Path(sys.argv[2])
manifest_entry = sys.argv[3]
android_build_root = Path(sys.argv[4])
allow_dirty_source = sys.argv[5] == "1"
source_seal_python = sys.argv[6]
source_seal_home = sys.argv[7]
source_seal_cargo_home = sys.argv[8]
source_seal_rustup_home = sys.argv[9]
source_seal_tmpdir = sys.argv[10]
source_seal_cargo = sys.argv[11]
source_seal_rustc = sys.argv[12]
source_seal_rustdoc = sys.argv[13]
source_seal_cargo_target_dir = sys.argv[14]
abis = ("arm64-v8a", "x86_64")
library_name = "libconnect_norito_bridge.so"
sha256_pattern = re.compile(r"^[0-9a-f]{64}$")


def fail(message):
    print(f"[mobile-sdk-artifacts] ERROR: {message}", file=sys.stderr)
    raise SystemExit(1)


def object_without_duplicates(pairs):
    value = {}
    for key, child in pairs:
        if key in value:
            raise ValueError(f"duplicate JSON member: {key}")
        value[key] = child
    return value


def sha256_bytes(payload):
    return hashlib.sha256(payload).hexdigest()


def source_seal_snapshot():
    if not all(
        (
            source_seal_python,
            source_seal_home,
            source_seal_cargo_home,
            source_seal_rustup_home,
            source_seal_tmpdir,
            source_seal_cargo,
            source_seal_rustc,
            source_seal_rustdoc,
            source_seal_cargo_target_dir,
        )
    ):
        fail("source-seal tool identity is unavailable")
    environment = {
        "HOME": source_seal_home,
        "PATH": os.pathsep.join(
            dict.fromkeys(
                (
                    str(Path(source_seal_python).parent),
                    str(Path(source_seal_cargo).parent),
                    str(Path(source_seal_rustc).parent),
                    str(Path(source_seal_rustdoc).parent),
                    "/usr/bin",
                    "/bin",
                )
            )
        ),
        "TMPDIR": source_seal_tmpdir,
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "NORITO_BRIDGE_SEAL_HOME": source_seal_home,
        "NORITO_BRIDGE_SEAL_CARGO_HOME": source_seal_cargo_home,
        "NORITO_BRIDGE_SEAL_RUSTUP_HOME": source_seal_rustup_home,
        "NORITO_BRIDGE_SEAL_TMPDIR": source_seal_tmpdir,
        "NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR": source_seal_cargo_target_dir,
        "NORITO_BRIDGE_SEAL_CARGO": source_seal_cargo,
        "NORITO_BRIDGE_SEAL_RUSTC": source_seal_rustc,
        "NORITO_BRIDGE_SEAL_RUSTDOC": source_seal_rustdoc,
    }
    return subprocess.run(
        [
            source_seal_python,
            "-I",
            "-S",
            "-B",
            str(root / "scripts/norito_bridge_source_seal.py"),
            "snapshot",
            "--root",
            str(root),
            "--platform",
            "android",
        ],
        check=True,
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout


def read_regular_file(path, label, allowed_root=None):
    allowed_root = allowed_root or android_build_root
    try:
        path.relative_to(allowed_root)
    except ValueError:
        fail(f"{label} escapes its allowed root: {path}")
    current = path
    while True:
        if current.is_symlink():
            fail(f"{label} must not traverse a symbolic link: {current}")
        if current == allowed_root:
            break
        current = current.parent
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"missing or unreadable {label}: {path} ({error})")
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            fail(f"{label} must be a non-symbolic regular file: {path}")
        with os.fdopen(descriptor, "rb", closefd=False) as handle:
            payload = handle.read()
        after = os.fstat(descriptor)
        if (
            before.st_dev,
            before.st_ino,
            before.st_size,
        ) != (
            after.st_dev,
            after.st_ino,
            after.st_size,
        ) or len(payload) != before.st_size:
            fail(f"{label} changed while it was being authenticated: {path}")
        return payload
    finally:
        os.close(descriptor)


try:
    archive = zipfile.ZipFile(aar_path)
except (OSError, zipfile.BadZipFile) as error:
    fail(f"client-android release aar is unreadable: {error}")

with archive:
    infos = archive.infolist()
    names = [info.filename for info in infos]
    if len(names) != len(set(names)):
        fail("client-android release aar contains duplicate ZIP entries")
    if names.count(manifest_entry) != 1:
        fail(f"client-android release aar must contain exactly one {manifest_entry}")
    manifest_info = archive.getinfo(manifest_entry)
    if stat.S_ISLNK(manifest_info.external_attr >> 16):
        fail("client-android native provenance AAR entry must not be a symbolic link")
    if manifest_info.file_size < 2 or manifest_info.file_size > 64 * 1024:
        fail("client-android native provenance must contain 2..65536 bytes")
    manifest_bytes = archive.read(manifest_info)
    try:
        manifest = json.loads(
            manifest_bytes.decode("utf-8"),
            object_pairs_hook=object_without_duplicates,
        )
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
        fail(f"client-android native provenance is invalid strict JSON: {error}")
    if not isinstance(manifest, dict):
        fail("client-android native provenance root must be an object")

    expected_top_level = {
        "schema",
        "native_bridge_abi_version",
        "build_profile",
        "cargo_locked",
        "privacy_production_enabled",
        "cargo_features",
        "build_environment",
        "source_commit",
        "source_tree_dirty",
        "source_fingerprint_sha256",
        "cargo_lock_sha256",
        "android_ndk_revision",
        "strip_tool_sha256",
        "libraries",
    }
    if set(manifest) != expected_top_level:
        fail(
            "client-android native provenance field inventory is not exact "
            f"(missing={sorted(expected_top_level - set(manifest))}, "
            f"unexpected={sorted(set(manifest) - expected_top_level)})"
        )
    if manifest["schema"] != "iroha.android-native-build-provenance.v1":
        fail("client-android native provenance schema is not v1")
    if type(manifest["native_bridge_abi_version"]) is not int or manifest["native_bridge_abi_version"] != 22:
        fail("client-android native provenance does not bind exact ABI 22")
    if manifest["build_profile"] != "release" or manifest["cargo_locked"] is not True:
        fail("client-android native provenance must bind a locked Cargo release build")
    production = manifest["privacy_production_enabled"]
    if type(production) is not bool:
        fail("client-android native provenance privacy_production_enabled must be boolean")
    expected_features = ["privacy-production-enabled"] if production else []
    if manifest["cargo_features"] != expected_features:
        fail(
            "client-android native provenance cargo_features must be exactly "
            f"{expected_features}"
        )
    build_environment = manifest["build_environment"]
    expected_build_environment_fields = {
        "schema",
        "hermetic_runner_schema",
        "hermetic_runner_sha256",
        "environment_profile",
        "environment_allowlist",
        "cargo_build_jobs",
        "rust_toolchain_channel",
        "cargo_release",
        "cargo_commit_hash",
        "cargo_binary_sha256",
        "rustc_release",
        "rustc_commit_hash",
        "rustc_binary_sha256",
        "rustdoc_release",
        "rustdoc_commit_hash",
        "rustdoc_binary_sha256",
        "cargo_ndk_version",
        "cargo_ndk_binary_sha256",
        "python_version",
        "python_binary_sha256",
        "git_version",
        "git_binary_sha256",
        "rustup_version",
        "rustup_binary_sha256",
        "android_ndk_revision",
        "android_ndk_source_properties_sha256",
    }
    if (
        not isinstance(build_environment, dict)
        or set(build_environment) != expected_build_environment_fields
    ):
        fail("client-android native provenance build_environment field inventory is not exact")
    expected_environment_allowlist = [
        "ANDROID_NDK_HOME",
        "ANDROID_NDK_ROOT",
        "CARGO",
        "CARGO_BUILD_JOBS",
        "CARGO_HOME",
        "CARGO_INCREMENTAL",
        "CARGO_NET_OFFLINE",
        "CARGO_TARGET_DIR",
        "HOME",
        "LANG",
        "LC_ALL",
        "NORITO_SKIP_BINDINGS_SYNC",
        "PATH",
        "RUSTC",
        "RUSTC_BOOTSTRAP",
        "RUSTDOC",
        "RUSTUP_HOME",
        "TMPDIR",
    ]
    if (
        build_environment["schema"] != "iroha.mobile-native-build-environment.v1"
        or build_environment["hermetic_runner_schema"]
        != "iroha.mobile-hermetic-command.v1"
        or build_environment["environment_profile"] != "android-cargo"
        or build_environment["environment_allowlist"] != expected_environment_allowlist
        or build_environment["cargo_build_jobs"] != 1
        or build_environment["rust_toolchain_channel"] != "1.93.1"
        or build_environment["cargo_release"] != "1.93.1"
        or build_environment["rustc_release"] != "1.93.1"
        or build_environment["rustdoc_release"] != "1.93.1"
        or build_environment["rustdoc_commit_hash"]
        != build_environment["rustc_commit_hash"]
        or build_environment["cargo_ndk_version"] != "4.1.2"
    ):
        fail("client-android native provenance build_environment is not canonical")
    for field in (
        "hermetic_runner_sha256",
        "cargo_binary_sha256",
        "rustc_binary_sha256",
        "rustdoc_binary_sha256",
        "cargo_ndk_binary_sha256",
        "python_binary_sha256",
        "git_binary_sha256",
        "rustup_binary_sha256",
        "android_ndk_source_properties_sha256",
    ):
        if not isinstance(build_environment[field], str) or not sha256_pattern.fullmatch(
            build_environment[field]
        ):
            fail(f"client-android native provenance build_environment {field} is malformed")
    for field in ("cargo_commit_hash", "rustc_commit_hash", "rustdoc_commit_hash"):
        if not isinstance(build_environment[field], str) or not re.fullmatch(
            r"[0-9a-f]{40}", build_environment[field]
        ):
            fail(f"client-android native provenance build_environment {field} is malformed")
    if not isinstance(build_environment["python_version"], str) or not re.fullmatch(
        r"3\.12\.[0-9]+", build_environment["python_version"]
    ):
        fail("client-android native provenance build_environment python_version must be exact Python 3.12")
    for field in ("git_version", "rustup_version"):
        if not isinstance(build_environment[field], str) or not re.fullmatch(
            r"[0-9]+(?:\.[0-9]+){1,2}", build_environment[field]
        ):
            fail(f"client-android native provenance build_environment {field} is malformed")
    if build_environment["android_ndk_revision"] != manifest["android_ndk_revision"]:
        fail("client-android native provenance Android NDK identities disagree")
    hermetic_runner = root / "scripts/run_mobile_hermetic_command.py"
    if hermetic_runner.is_file() and sha256_bytes(
        read_regular_file(
            hermetic_runner,
            "mobile hermetic command runner",
            root,
        )
    ) != build_environment["hermetic_runner_sha256"]:
        fail("client-android native provenance hermetic runner digest does not match checkout")
    if not isinstance(manifest["source_commit"], str) or not re.fullmatch(
        r"[0-9a-f]{40}", manifest["source_commit"]
    ):
        fail("client-android native provenance source_commit is not canonical lowercase Git SHA-1")
    if type(manifest["source_tree_dirty"]) is not bool:
        fail("client-android native provenance source_tree_dirty must be boolean")
    if manifest["source_tree_dirty"] and not allow_dirty_source:
        fail("client-android release artifact must be built from a clean source tree")
    for field in (
        "source_fingerprint_sha256",
        "cargo_lock_sha256",
        "strip_tool_sha256",
    ):
        if not isinstance(manifest[field], str) or not sha256_pattern.fullmatch(manifest[field]):
            fail(f"client-android native provenance {field} is not canonical SHA-256")
    if not isinstance(manifest["android_ndk_revision"], str) or not re.fullmatch(
        r"[0-9]+(?:\.[0-9]+){1,3}", manifest["android_ndk_revision"]
    ):
        fail("client-android native provenance Android NDK revision is not canonical")
    if manifest["android_ndk_revision"] != "28.0.12674087":
        fail("client-android native provenance does not bind exact Android NDK 28.0.12674087")

    source_snapshot_bytes = None
    if (root / ".git").exists():
        try:
            source_snapshot_bytes = source_seal_snapshot()
            source_snapshot = json.loads(
                source_snapshot_bytes.decode("utf-8"),
                object_pairs_hook=object_without_duplicates,
            )
        except (OSError, subprocess.CalledProcessError) as error:
            fail(f"unable to authenticate Android native provenance against checkout: {error}")
        except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
            fail(f"Android source-seal snapshot is invalid strict JSON: {error}")
        expected_snapshot_fields = {
            "platform",
            "schema",
            "source_commit",
            "source_fingerprint_sha256",
            "source_status",
            "source_tree_dirty",
            "targets",
        }
        if not isinstance(source_snapshot, dict) or set(source_snapshot) != expected_snapshot_fields:
            fail("Android source-seal snapshot field inventory is not exact")
        if source_snapshot["schema"] != "iroha.norito-bridge-source-seal.v1":
            fail("Android source-seal snapshot schema is not canonical")
        if source_snapshot["platform"] != "android" or source_snapshot["targets"] != [
            "aarch64-linux-android",
            "x86_64-linux-android",
        ]:
            fail("Android source-seal snapshot platform/target inventory is not exact")
        if type(source_snapshot["source_tree_dirty"]) is not bool or not isinstance(
            source_snapshot["source_status"], str
        ):
            fail("Android source-seal snapshot dirty state is malformed")
        actual_dirty = source_snapshot["source_tree_dirty"]
        if actual_dirty != bool(source_snapshot["source_status"]):
            fail("Android source-seal snapshot dirty state disagrees with its exact status")
        actual_commit = source_snapshot["source_commit"]
        actual_fingerprint = source_snapshot["source_fingerprint_sha256"]
        if manifest["source_commit"] != actual_commit:
            fail("client-android native provenance source commit does not match checkout")
        if manifest["source_tree_dirty"] != actual_dirty:
            fail("client-android native provenance source dirty state does not match checkout")
        if manifest["source_fingerprint_sha256"] != actual_fingerprint:
            fail("client-android native provenance source fingerprint does not match checkout")
        if actual_dirty and not allow_dirty_source:
            fail("client-android release artifact cannot be certified against a dirty checkout")
        cargo_lock = root / "Cargo.lock"
        cargo_lock_bytes = read_regular_file(cargo_lock, "Iroha Cargo.lock", root)
        if sha256_bytes(cargo_lock_bytes) != manifest["cargo_lock_sha256"]:
            fail("client-android native provenance Cargo.lock digest does not match checkout")

    mode = "production" if production else "default"
    generated_manifest = android_build_root / (
        f"generated/nativeProvenance/{mode}/iroha/native-build-provenance-v1.json"
    )
    generated_manifest_bytes = read_regular_file(
        generated_manifest,
        f"client-android {mode} generated native provenance",
    )
    if generated_manifest_bytes != manifest_bytes:
        fail("client-android generated native provenance differs from release aar")

    libraries = manifest["libraries"]
    if not isinstance(libraries, dict) or set(libraries) != set(abis):
        fail("client-android native provenance library ABI inventory is not exact")
    expected_native_entries = {
        f"jni/{abi}/{library_name}" for abi in abis
    }
    actual_native_entries = {
        name for name in names if name.startswith("jni/") and not name.endswith("/")
    }
    if actual_native_entries != expected_native_entries:
        fail(
            "client-android release aar native bridge inventory is not exact "
            f"(expected={sorted(expected_native_entries)}, "
            f"actual={sorted(actual_native_entries)})"
        )

    def read_exact_native_tree(directory, label):
        expected = {f"{abi}/{library_name}" for abi in abis}
        try:
            directory.relative_to(android_build_root)
        except ValueError:
            fail(f"{label} escapes the Android build root: {directory}")
        if directory.is_symlink() or not directory.is_dir():
            fail(f"{label} must be a non-symbolic directory: {directory}")
        actual = set()
        actual_directories = set()
        for current, child_directories, child_files in os.walk(
            directory, topdown=True, followlinks=False
        ):
            current_path = Path(current)
            if current_path.is_symlink():
                fail(f"{label} must not traverse a symbolic link: {current_path}")
            for child in child_directories:
                child_path = current_path / child
                if child_path.is_symlink():
                    fail(f"{label} must not traverse a symbolic link: {child_path}")
                actual_directories.add(child_path.relative_to(directory).as_posix())
            for child in child_files:
                child_path = current_path / child
                try:
                    mode = child_path.lstat().st_mode
                except OSError as error:
                    fail(f"{label} contains an unreadable entry: {child_path} ({error})")
                if not stat.S_ISREG(mode):
                    fail(f"{label} contains a non-regular entry: {child_path}")
                actual.add(child_path.relative_to(directory).as_posix())
        if actual != expected or actual_directories != set(abis):
            fail(
                f"{label} inventory is not exact "
                f"(expected_files={sorted(expected)}, actual_files={sorted(actual)}, "
                f"expected_directories={sorted(abis)}, "
                f"actual_directories={sorted(actual_directories)})"
            )
        return {
            abi: read_regular_file(
                directory / abi / library_name,
                f"client-android {abi} {label} native bridge library",
            )
            for abi in abis
        }

    generated_by_abi = read_exact_native_tree(
        android_build_root / f"generated/jniLibs/{mode}",
        "generated native bridge",
    )
    raw_by_abi = read_exact_native_tree(
        android_build_root / f"native/cargo-ndk/{mode}",
        "raw cargo-ndk native bridge",
    )

    for abi in abis:
        record = libraries[abi]
        expected_record_fields = {
            "aar_path",
            "bytes",
            "raw_bytes",
            "raw_sha256",
            "sha256",
        }
        if not isinstance(record, dict) or set(record) != expected_record_fields:
            fail(f"client-android native provenance {abi} record field inventory is not exact")
        entry = f"jni/{abi}/{library_name}"
        if record["aar_path"] != entry:
            fail(f"client-android native provenance {abi} AAR path is not canonical")
        if type(record["bytes"]) is not int or record["bytes"] <= 0:
            fail(f"client-android native provenance {abi} byte count is invalid")
        if type(record["raw_bytes"]) is not int or record["raw_bytes"] <= 0:
            fail(f"client-android native provenance {abi} raw byte count is invalid")
        for field in ("raw_sha256", "sha256"):
            if not isinstance(record[field], str) or not sha256_pattern.fullmatch(record[field]):
                fail(f"client-android native provenance {abi} {field} is not canonical SHA-256")

        generated_bytes = generated_by_abi[abi]
        raw_bytes = raw_by_abi[abi]
        info = archive.getinfo(entry)
        if stat.S_ISLNK(info.external_attr >> 16):
            fail(f"client-android {abi} native bridge AAR entry must not be a symbolic link")
        aar_bytes = archive.read(info)
        if len(generated_bytes) != record["bytes"] or len(aar_bytes) != record["bytes"]:
            fail(f"client-android {abi} native bridge byte count differs from provenance")
        if sha256_bytes(generated_bytes) != record["sha256"]:
            fail(f"client-android {abi} generated native bridge differs from provenance")
        if len(raw_bytes) != record["raw_bytes"]:
            fail(f"client-android {abi} raw cargo-ndk native bridge byte count differs from provenance")
        if sha256_bytes(raw_bytes) != record["raw_sha256"]:
            fail(f"client-android {abi} raw cargo-ndk native bridge differs from provenance")
        if aar_bytes != generated_bytes:
            fail(f"client-android {abi} native bridge differs between generated output and release aar")

    if source_snapshot_bytes is not None:
        try:
            source_snapshot_after = source_seal_snapshot()
        except (OSError, subprocess.CalledProcessError) as error:
            fail(f"unable to re-authenticate Android source after artifact checks: {error}")
        if source_snapshot_after != source_snapshot_bytes:
            fail("Android source changed while native artifacts were being authenticated")

print(mode)
PY
}

check_android_package() {
  local settings="$ROOT_DIR/kotlin/settings.gradle.kts"

  require_file "$settings" "Kotlin settings manifest"
  require_literal "$settings" 'include(":core-jvm")' "core-jvm module include"
  require_literal "$settings" 'include(":client-android")' "client-android module include"

  check_gradle_publication "core-jvm" "core-jvm"
  check_gradle_publication "client-android" "client-android"

  require_file "$ROOT_DIR/kotlin/client-android/src/main/AndroidManifest.xml" "client-android AndroidManifest"

  if [[ "$REQUIRE_ANDROID_OUTPUTS" == "1" ]]; then
    local client_aar="$ANDROID_CLIENT_BUILD_DIR/outputs/aar/client-android-release.aar"
    local abi
    local native_mode

    require_glob "$ANDROID_CORE_JVM_BUILD_DIR/libs/core-jvm-*.jar" "core-jvm built jar"
    require_glob "$client_aar" "client-android release aar"

    require_zip_entry "$client_aar" "AndroidManifest.xml" "client-android release aar"
    require_zip_entry "$client_aar" "classes.jar" "client-android release aar"
    require_zip_entry "$client_aar" "$ANDROID_NATIVE_PROVENANCE_ENTRY" "client-android release aar"
    reject_candidate_lab_archive "$client_aar" "client-android release aar"

    if ! native_mode="$(check_android_native_provenance "$client_aar")"; then
      FAILURES=1
      native_mode=""
    fi

    local production_archive
    while IFS= read -r production_archive; do
      reject_candidate_lab_archive "$production_archive" "production mobile SDK archive"
    done < <(
      compgen -G "$ANDROID_CORE_JVM_BUILD_DIR/libs/core-jvm-*.jar" || true
      compgen -G "$ANDROID_OFFLINE_WALLET_BUILD_DIR/outputs/aar/offline-wallet-android-*.aar" || true
    )

    for abi in arm64-v8a x86_64; do
      local source_native="$ANDROID_CLIENT_BUILD_DIR/generated/jniLibs/$native_mode/$abi/libconnect_norito_bridge.so"
      local aar_entry="jni/$abi/libconnect_norito_bridge.so"
      if [[ -n "$native_mode" ]]; then
        require_file "$source_native" "client-android $abi generated native bridge library"
        reject_candidate_lab_content "$source_native" "client-android $abi generated production bridge"
      fi
      require_zip_entry "$client_aar" "jni/$abi/libconnect_norito_bridge.so" "client-android release aar"
      if [[ -n "$native_mode" && -f "$source_native" && -f "$client_aar" ]] \
          && unzip -Z1 "$client_aar" 2>/dev/null | grep -Fxq -- "$aar_entry"; then
        if [[ "$(hash_file "$source_native")" != "$(hash_zip_entry "$client_aar" "$aar_entry")" ]]; then
          fail "client-android $abi native bridge differs between generated output and release aar"
        fi
        check_android_native_stripped "$source_native" "$abi"
        check_android_native_symbols "$source_native" "$abi"
      fi
    done
  fi
}

check_bridge_source_contract

if [[ "$CHECK_APPLE" == "1" ]]; then
  assert_apple_cargo_lock || FAILURES=1
  check_swift_kagemusha_source_contract
  check_swift_package
  check_xcframework
  assert_apple_cargo_lock || FAILURES=1
fi

if [[ "$CHECK_ANDROID" == "1" ]]; then
  check_android_kagemusha_source_contract
  check_android_package
fi

if [[ "$FAILURES" -ne 0 ]]; then
  echo "[mobile-sdk-artifacts] validation failed for $ROOT_DIR" >&2
  exit 1
fi

echo "[mobile-sdk-artifacts] validation passed for $ROOT_DIR"
