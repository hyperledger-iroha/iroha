#!/usr/bin/env bash
# Verify that mobile build and packaging entrypoints pin an isolated Python 3.12.
set -euo pipefail

PATH=/usr/bin:/bin
export PATH

ROOT_DIR="$(cd "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
APPLE_BUILDER="$ROOT_DIR/scripts/build_norito_xcframework.sh"
MOBILE_PACKAGER="$ROOT_DIR/scripts/package_mobile_sdk_artifacts.sh"
MOBILE_CHECKER="$ROOT_DIR/scripts/check_mobile_sdk_artifacts.sh"
MOBILE_CHECKER_TEST="$ROOT_DIR/scripts/check_mobile_sdk_artifacts_test.sh"
ANDROID_BUILDER="$ROOT_DIR/kotlin/client-android/build.gradle.kts"
MOBILE_WORKFLOW="$ROOT_DIR/.github/workflows/mobile_sdk_artifacts.yml"
TEST_ROOT=""

cleanup() {
  if [[ -n "$TEST_ROOT" && -d "$TEST_ROOT" ]]; then
    rm -rf -- "$TEST_ROOT"
  fi
}
trap cleanup EXIT HUP INT TERM

fail() {
  printf 'mobile Python 3.12 contract test failed: %s\n' "$*" >&2
  exit 1
}

find_python312() {
  local candidate canonical
  local override="${MOBILE_SDK_PYTHON_BINARY:-}"
  local candidates=()
  if [[ -n "$override" ]]; then
    [[ "$override" == /* && -f "$override" && ! -L "$override" && -x "$override" ]] \
      || return 1
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
    if canonical="$(
      env -i \
        HOME=/tmp \
        PATH=/usr/bin:/bin \
        TMPDIR=/tmp \
        LANG=C.UTF-8 \
        LC_ALL=C.UTF-8 \
        "$candidate" -I -S -c '
import os
import pathlib
import stat
import sys

if sys.version_info[:2] != (3, 12) or not sys.flags.isolated:
    raise SystemExit(1)
if "SDKROOT" in os.environ:
    raise SystemExit(1)
resolved = pathlib.Path(sys.executable).resolve(strict=True)
if not stat.S_ISREG(resolved.stat().st_mode) or not os.access(resolved, os.X_OK):
    raise SystemExit(1)
print(resolved)
'
    )"; then
      if [[ -n "$override" && "$canonical" != "$override" ]]; then
        return 1
      fi
      printf '%s\n' "$canonical"
      return 0
    fi
  done
  return 1
}

expect_success() {
  local label="$1"
  shift
  local output
  if ! output="$("$@" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "$label unexpectedly failed"
  fi
}

expect_failure_containing() {
  local label="$1"
  local expected="$2"
  shift 2
  local output
  if output="$("$@" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "$label unexpectedly succeeded"
  fi
  case "$output" in
    *"$expected"*) ;;
    *)
      printf '%s\n' "$output" >&2
      fail "$label did not report: $expected"
      ;;
  esac
}

PYTHON312="$(find_python312)" || fail "no trusted Python 3.12 executable is available"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/iroha-mobile-python312-contract.XXXXXX")"
TEST_ROOT="$("$PYTHON312" -I -S -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$TEST_ROOT")"
[[ "$PYTHON312" == /* && -f "$PYTHON312" && ! -L "$PYTHON312" && -x "$PYTHON312" ]] \
  || fail "Python 3.12 discovery did not return a canonical regular executable"

for script in "$APPLE_BUILDER" "$MOBILE_PACKAGER"; do
  grep -Fq 'MOBILE_SDK_PYTHON_BINARY' "$script" \
    || fail "$script does not expose the canonical Python override"
  grep -Fq "\"\$PYTHON_BINARY\" -I -S -B \"\$@\"" "$script" \
    || fail "$script does not isolate helper Python invocations"
  if grep -Eq '^[[:space:]]*python3([[:space:]]|$)' "$script"; then
    fail "$script still invokes Python through ambient PATH"
  fi
done

grep -Fq 'resolve_trusted_python312()' "$MOBILE_CHECKER" \
  || fail "mobile checker does not authenticate Python 3.12"
grep -Fq 'MOBILE_SDK_PYTHON_BINARY' "$MOBILE_CHECKER" \
  || fail "mobile checker does not expose the canonical Python override"
for script in "$APPLE_BUILDER" "$MOBILE_CHECKER" "$MOBILE_PACKAGER"; do
  grep -Fq 'MOBILE_SDK_RUSTUP_BINARY' "$script" \
    || fail "$script does not expose the canonical rustup override"
done
grep -Fq 'MOBILE_SDK_RUSTUP_BINARY="$RUSTUP_BINARY"' "$APPLE_BUILDER" \
  || fail "Apple builder does not forward its selected rustup to staged validation"
grep -Fq 'MOBILE_SDK_RUSTUP_BINARY="$ARCHIVE_SEAL_RUSTUP"' "$MOBILE_PACKAGER" \
  || fail "mobile packager does not forward its authenticated rustup to validation"
grep -Fq '"$CHECK_PYTHON_BINARY" -I -S -B "$@"' "$MOBILE_CHECKER" \
  || fail "mobile checker does not isolate Python helpers from site packages"
grep -Fq 'NORITO_BRIDGE_BUILD_LOCK_FDS' "$APPLE_BUILDER" \
  || fail "Apple builder does not authenticate inherited target/stage/output locks"
[[ "$(grep -Fc 'NORITO_BRIDGE_BUILD_LOCK_HELD' "$APPLE_BUILDER")" -eq 1 ]] \
  || fail "Apple builder must mention the retired boolean lock bypass only in its rejection list"
if grep -Fq 'MOBILE_SDK_SKIP_BINARY_INSPECTION' "$MOBILE_CHECKER"; then
  grep -Fq 'is retired; binary inspection is mandatory' "$MOBILE_CHECKER" \
    || fail "mobile checker still permits binary-inspection bypass"
fi
grep -Fq 'export MOBILE_SDK_PYTHON_BINARY="$TEST_PYTHON_BINARY"' "$MOBILE_CHECKER_TEST" \
  || fail "mobile checker self-test does not bind its authenticated Python"
grep -Fq 'export MOBILE_SDK_RUSTUP_BINARY="$TEST_RUSTUP_BINARY"' "$MOBILE_CHECKER_TEST" \
  || fail "mobile checker self-test does not bind its authenticated rustup"
grep -Fq 'System.getenv("MOBILE_SDK_PYTHON_BINARY")' "$ANDROID_BUILDER" \
  || fail "Android native build logic does not honor the canonical Python override"
grep -Fq 'const val pinnedPythonSeries = "3.12"' "$ANDROID_BUILDER" \
  || fail "Android native build logic does not pin Python 3.12"
grep -Fq '"/opt/homebrew/opt/python@3.12/bin/python3.12"' "$ANDROID_BUILDER" \
  || fail "Android native build logic omits the Homebrew versioned Python locator"
if grep -Fq '"/opt/homebrew/bin/python3",' "$ANDROID_BUILDER"; then
  fail "Android native build logic still trusts an unversioned Homebrew Python"
fi
[[ "$(grep -Fc 'python-version: "3.12"' "$MOBILE_WORKFLOW")" -eq 4 ]] \
  || fail "mobile workflow must pin Python 3.12 in exactly four jobs"
[[ "$(grep -Fc 'echo "MOBILE_SDK_PYTHON_BINARY=$mobile_python"' "$MOBILE_WORKFLOW")" -eq 3 ]] \
  || fail "mobile workflow must bind the canonical Python in exactly three jobs"
[[ "$(grep -Fc 'echo "MOBILE_SDK_RUSTUP_BINARY=$rustup_path"' "$MOBILE_WORKFLOW")" -eq 1 ]] \
  || fail "mobile workflow must bind the canonical rustup in its Apple job"
grep -Fq 'bash scripts/tests/mobile_sdk_python312_contract.sh' "$MOBILE_WORKFLOW" \
  || fail "mobile workflow does not run the Python 3.12 contract"
grep -Fq '"scripts/tests/mobile_sdk_python312_contract.sh"' "$MOBILE_WORKFLOW" \
  || fail "mobile workflow does not trigger on Python 3.12 contract changes"

expect_failure_containing \
  "retired Apple Cargo lock override" \
  "MOBILE_SDK_APPLE_CARGO_LOCK_PATH is not part of the first-release artifact contract" \
  env \
    MOBILE_SDK_APPLE_CARGO_LOCK_PATH="$ROOT_DIR/Cargo.lock" \
    /bin/bash "$MOBILE_CHECKER" --root "$ROOT_DIR" --apple-only

mkdir -p "$TEST_ROOT/hostile-path" "$TEST_ROOT/forged-sdk"
ln -s "$PYTHON312" "$TEST_ROOT/python312-link"

expect_success \
  "default package Python selection under a forged SDKROOT" \
  env \
    PATH="$TEST_ROOT/hostile-path" \
    SDKROOT="$TEST_ROOT/forged-sdk" \
    /bin/bash "$MOBILE_PACKAGER" --help

expect_success \
  "canonical Python override under a forged SDKROOT" \
  env \
    PATH="$TEST_ROOT/hostile-path" \
    SDKROOT="$TEST_ROOT/forged-sdk" \
    MOBILE_SDK_PYTHON_BINARY="$PYTHON312" \
    /bin/bash "$MOBILE_PACKAGER" --help

expect_failure_containing \
  "symlinked Python override" \
  "absolute canonical regular executable" \
  env \
    MOBILE_SDK_PYTHON_BINARY="$TEST_ROOT/python312-link" \
    /bin/bash "$MOBILE_PACKAGER" --help

expect_failure_containing \
  "relative Python override" \
  "absolute canonical regular executable" \
  env \
    MOBILE_SDK_PYTHON_BINARY=python3.12 \
    /bin/bash "$MOBILE_PACKAGER" --help

expect_failure_containing \
  "non-Python override" \
  "isolated Python 3.12 executable" \
  env \
    MOBILE_SDK_PYTHON_BINARY=/bin/bash \
    /bin/bash "$MOBILE_PACKAGER" --help

APPLE_CARGO_TARGET="$TEST_ROOT/apple-cargo-target"
APPLE_BUILD_DIR="$TEST_ROOT/apple-build"
APPLE_OUT_DIR="$TEST_ROOT/apple-out"
APPLE_USER_HOME="$("$PYTHON312" -I -S -c 'import os,pwd; print(pwd.getpwuid(os.getuid()).pw_dir)')"
APPLE_RUSTUP="$APPLE_USER_HOME/.cargo/bin/rustup"
[[ -x "$APPLE_RUSTUP" ]] || fail "pinned rustup is unavailable"
APPLE_RUSTUP="$("$PYTHON312" -I -S -B -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$APPLE_RUSTUP")"
[[ "$APPLE_RUSTUP" == /* && -f "$APPLE_RUSTUP" && ! -L "$APPLE_RUSTUP" \
  && -x "$APPLE_RUSTUP" ]] || fail "pinned rustup is not canonical"
APPLE_RUSTC="$("$APPLE_RUSTUP" which --toolchain 1.93.1 rustc)"
APPLE_RUSTDOC="$("$APPLE_RUSTUP" which --toolchain 1.93.1 rustdoc)"
mkdir -p "$APPLE_CARGO_TARGET" "$APPLE_BUILD_DIR" "$APPLE_OUT_DIR"
expect_failure_containing \
  "retired Apple boolean lock bypass" \
  "NORITO_BRIDGE_BUILD_LOCK_HELD is not part of the first-release build contract" \
  env \
    MOBILE_SDK_PYTHON_BINARY="$PYTHON312" \
    NORITO_BRIDGE_BUILD_DIR="$APPLE_BUILD_DIR" \
    NORITO_BRIDGE_OUT_DIR="$APPLE_OUT_DIR" \
    NORITO_BRIDGE_BUILD_LOCK_HELD=1 \
    /bin/bash "$APPLE_BUILDER" --python-contract-test-invalid-option
expect_failure_containing \
  "Apple builder canonical Python override" \
  "Unknown argument" \
  env \
    PATH="$TEST_ROOT/hostile-path" \
    SDKROOT="$TEST_ROOT/forged-sdk" \
    MOBILE_SDK_PYTHON_BINARY="$PYTHON312" \
    NORITO_BRIDGE_BUILD_DIR="$APPLE_BUILD_DIR" \
    NORITO_BRIDGE_OUT_DIR="$APPLE_OUT_DIR" \
    CARGO_BUILD_JOBS=1 \
    CARGO_INCREMENTAL=0 \
    CARGO_NET_OFFLINE=true \
    CARGO_TARGET_DIR="$APPLE_CARGO_TARGET" \
    RUSTC="$APPLE_RUSTC" \
    RUSTC_BOOTSTRAP=1 \
    RUSTDOC="$APPLE_RUSTDOC" \
    /bin/bash "$APPLE_BUILDER" --python-contract-test-invalid-option

expect_failure_containing \
  "forged Apple lock descriptors" \
  "NoritoBridge build lock is not authenticated" \
  env \
    MOBILE_SDK_PYTHON_BINARY="$PYTHON312" \
    NORITO_BRIDGE_BUILD_DIR="$APPLE_BUILD_DIR" \
    NORITO_BRIDGE_OUT_DIR="$APPLE_OUT_DIR" \
    NORITO_BRIDGE_BUILD_LOCK_FDS=1,2,3 \
    CARGO_BUILD_JOBS=1 \
    CARGO_INCREMENTAL=0 \
    CARGO_NET_OFFLINE=true \
    CARGO_TARGET_DIR="$APPLE_CARGO_TARGET" \
    RUSTC="$APPLE_RUSTC" \
    RUSTC_BOOTSTRAP=1 \
    RUSTDOC="$APPLE_RUSTDOC" \
    /bin/bash "$APPLE_BUILDER" --python-contract-test-invalid-option

for deployment_case in wrong empty; do
  if [[ "$deployment_case" == "wrong" ]]; then
    deployment_assignment="IPHONEOS_DEPLOYMENT_TARGET=14.0"
    deployment_error="IPHONEOS_DEPLOYMENT_TARGET is fixed at 15.0"
  else
    deployment_assignment="MACOSX_DEPLOYMENT_TARGET="
    deployment_error="MACOSX_DEPLOYMENT_TARGET is fixed at 12.0"
  fi
  expect_failure_containing \
    "Apple $deployment_case deployment target" \
    "$deployment_error" \
    env \
      "$deployment_assignment" \
      MOBILE_SDK_PYTHON_BINARY="$PYTHON312" \
      NORITO_BRIDGE_BUILD_DIR="$APPLE_BUILD_DIR" \
      NORITO_BRIDGE_OUT_DIR="$APPLE_OUT_DIR" \
      CARGO_BUILD_JOBS=1 \
      CARGO_INCREMENTAL=0 \
      CARGO_NET_OFFLINE=true \
      CARGO_TARGET_DIR="$APPLE_CARGO_TARGET" \
      RUSTC="$APPLE_RUSTC" \
      RUSTC_BOOTSTRAP=1 \
      RUSTDOC="$APPLE_RUSTDOC" \
      /bin/bash "$APPLE_BUILDER" --python-contract-test-invalid-option
done

OVERLAP_DIR="$TEST_ROOT/apple-overlap"
OVERLAP_OUT="$TEST_ROOT/apple-overlap-out"
mkdir -p "$OVERLAP_DIR" "$OVERLAP_OUT"
printf 'preserve\n' >"$OVERLAP_DIR/target-sentinel"
expect_failure_containing \
  "Apple builder target/build overlap" \
  "Cargo target, build, and output directories must be pairwise disjoint" \
  env \
    MOBILE_SDK_PYTHON_BINARY="$PYTHON312" \
    NORITO_BRIDGE_BUILD_DIR="$OVERLAP_DIR" \
    NORITO_BRIDGE_OUT_DIR="$OVERLAP_OUT" \
    CARGO_BUILD_JOBS=1 \
    CARGO_INCREMENTAL=0 \
    CARGO_NET_OFFLINE=true \
    CARGO_TARGET_DIR="$OVERLAP_DIR" \
    RUSTC="$APPLE_RUSTC" \
    RUSTC_BOOTSTRAP=1 \
    RUSTDOC="$APPLE_RUSTDOC" \
    /bin/bash "$APPLE_BUILDER" --python-contract-test-invalid-option
[[ "$(<"$OVERLAP_DIR/target-sentinel")" == "preserve" ]] \
  || fail "Apple builder modified an overlapped Cargo target before rejecting it"

printf 'mobile Python 3.12 contract tests passed\n'
