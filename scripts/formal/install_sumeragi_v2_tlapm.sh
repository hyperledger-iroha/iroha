#!/usr/bin/env bash
set -euo pipefail
umask 077

readonly TLAPM_VERSION="1.6.0-pre"
readonly TLAPM_COMMIT="3ab43c7ff31db4ced850619d4746fa4c841a7681"
readonly EXPECTED_SOURCE_LOCK_SHA256="0b41cdcf512e045e7f46970f2f4a54f5f2a9031ab7ffa26f02e059130c3b7563"
readonly RELEASE_ASSET_API_BASE="https://api.github.com/repos/tlaplus/tlapm/releases/assets"
readonly REPO_ROOT="$(cd -- "$(/usr/bin/dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly SOURCE_BUILD_SCRIPT="${REPO_ROOT}/scripts/formal/build_sumeragi_v2_tlapm_from_source.sh"
readonly SOURCE_BUILD_LOCK="${REPO_ROOT}/scripts/formal/sumeragi_v2_tlapm_source_build_lock.json"
readonly SOURCE_LOCK_HELPER="${REPO_ROOT}/scripts/formal/sumeragi_v2_tlapm_source_lock.py"
readonly SOURCE_LOCKED_WGET="${REPO_ROOT}/scripts/formal/sumeragi_v2_tlapm_locked_wget.sh"
readonly REQUESTED_SOURCE_LOCK_PYTHON="${IROHA_RELEASE_POLICY_PYTHON:-python3}"

case "$(/usr/bin/uname -s)-$(/usr/bin/uname -m)" in
  Linux-x86_64)
    readonly PLATFORM="x86_64-linux-gnu"
    readonly SANITIZED_RUNTIME_PATH="/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
    # The rolling release workflow uploaded this object for TLAPM_COMMIT and
    # recorded its ID and digest in GitHub Actions run 29682668751, job
    # 88181482518. Address the object, not the mutable rolling tag and name.
    readonly RELEASE_ASSET_ID="482292328"
    readonly ARCHIVE_SHA256="a686da5dc31892edcd02f25bb14061427e29e16317002d43c5b5be970d1d5daf"
    ;;
  Darwin-arm64)
    readonly PLATFORM="arm64-darwin"
    readonly SANITIZED_RUNTIME_PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
    # The rolling release workflow uploaded this object for TLAPM_COMMIT and
    # recorded its ID and digest in GitHub Actions run 29682668751, job
    # 88181482538. Address the object, not the mutable rolling tag and name.
    readonly RELEASE_ASSET_ID="482297997"
    readonly ARCHIVE_SHA256="3ca4c39613e58b90e46a385ee61e2c7f17375c19854ea1a35e056d6eb902071c"
    ;;
  *)
    echo "unsupported TLAPM host: $(/usr/bin/uname -s)-$(/usr/bin/uname -m)" >&2
    exit 1
    ;;
esac

readonly ARCHIVE="tlapm-${TLAPM_VERSION}-${PLATFORM}.tar.gz"
readonly URL="${RELEASE_ASSET_API_BASE}/${RELEASE_ASSET_ID}"
readonly INSTALL_ROOT="${TLAPM_INSTALL_ROOT:?TLAPM_INSTALL_ROOT must be an explicitly authorized external directory}"
readonly INSTALL_PARENT="${INSTALL_ROOT}/${TLAPM_COMMIT}"
readonly INSTALL_DIR="${INSTALL_PARENT}/${PLATFORM}"
readonly TLAPM_BIN="${INSTALL_DIR}/tlapm/bin/tlapm"

source "${REPO_ROOT}/scripts/sumeragi_v2_release_process_policy.sh"
require_external_private_directory \
  "$REPO_ROOT" "$INSTALL_ROOT" "TLAPM install" || exit $?

for required_file in "$SOURCE_BUILD_SCRIPT" "$SOURCE_BUILD_LOCK" \
  "$SOURCE_LOCK_HELPER" "$SOURCE_LOCKED_WGET"; do
  [[ -f "$required_file" && ! -L "$required_file" ]] || {
    echo "the checked-in TLAPM source-build corridor is incomplete" >&2
    exit 1
  }
done
SOURCE_LOCK_PYTHON_PATH="$(type -P "$REQUESTED_SOURCE_LOCK_PYTHON")" || {
  echo "${REQUESTED_SOURCE_LOCK_PYTHON} is required for the TLAPM installer" >&2
  exit 1
}
SOURCE_LOCK_PYTHON="$("$SOURCE_LOCK_PYTHON_PATH" -I -S -c \
  'import os, sys; print(os.path.realpath(sys.executable))')"
readonly SOURCE_LOCK_PYTHON
[[ -f "$SOURCE_LOCK_PYTHON" && -x "$SOURCE_LOCK_PYTHON" \
  && ! -L "$SOURCE_LOCK_PYTHON" ]] || {
  echo "the TLAPM installer Python is not one canonical executable" >&2
  exit 1
}
[[ -x /usr/bin/curl && -x /usr/bin/tar ]] || {
  echo "system curl and tar are required to install pinned TLAPM" >&2
  exit 1
}

work_dir="$(/usr/bin/mktemp -d "${INSTALL_ROOT}/.sumeragi-v2-tlapm-install.XXXXXX")"
work_dir="$(cd -P -- "$work_dir" && pwd)"
/bin/chmod 0700 "$work_dir"
trap '/bin/rm -rf -- "$work_dir"' EXIT

readonly SNAPSHOT_DIR="${work_dir}/corridor-snapshot"
"$SOURCE_LOCK_PYTHON" -I -S "$SOURCE_LOCK_HELPER" \
  --lock "$SOURCE_BUILD_LOCK" --platform "$PLATFORM" snapshot-corridor \
  --helper "$SOURCE_LOCK_HELPER" --locked-wget "$SOURCE_LOCKED_WGET" \
  --source-builder "$SOURCE_BUILD_SCRIPT" \
  --output-dir "$SNAPSHOT_DIR"
readonly FROZEN_LOCK="${SNAPSHOT_DIR}/source-build-lock.json"
readonly FROZEN_HELPER="${SNAPSHOT_DIR}/source-lock.py"
readonly FROZEN_LOCKED_WGET="${SNAPSHOT_DIR}/locked-wget.sh"
readonly FROZEN_SOURCE_BUILDER="${SNAPSHOT_DIR}/source-builder.sh"

hash_file() {
  "$SOURCE_LOCK_PYTHON" -I -S "$FROZEN_HELPER" \
    --lock "$FROZEN_LOCK" --platform "$PLATFORM" hash-file --path "$1"
}

[[ "$(hash_file "$FROZEN_LOCK")" == "$EXPECTED_SOURCE_LOCK_SHA256" ]] || {
  echo "TLAPM source-build lock digest does not match the reviewed corridor" >&2
  exit 1
}

run_tlapm_version() {
  /usr/bin/env -i HOME="$work_dir" PATH="$SANITIZED_RUNTIME_PATH" \
    TMPDIR="$work_dir" LANG=C LC_ALL=C TZ=UTC "$1" --version 2>&1
}

verify_install() {
  local allowed_origin_args=()
  local verified_origin
  if [[ -n "${TLAPM_ARCHIVE_PATH:-}" ]]; then
    allowed_origin_args+=(--allowed-origin caller-archive)
  else
    allowed_origin_args+=(--allowed-origin github-release-asset)
    allowed_origin_args+=(--allowed-origin immutable-source-build)
  fi
  verified_origin="$("$SOURCE_LOCK_PYTHON" -I -S "$FROZEN_HELPER" \
    --lock "$FROZEN_LOCK" --platform "$PLATFORM" verify-install \
    --directory "$INSTALL_DIR" "${allowed_origin_args[@]}" \
    --prebuilt-sha256 "$ARCHIVE_SHA256" --locked-wget "$FROZEN_LOCKED_WGET" \
    --source-builder "$FROZEN_SOURCE_BUILDER")" \
    || return 1
  [[ -x "$TLAPM_BIN" \
    && "$(run_tlapm_version "$TLAPM_BIN")" == "${TLAPM_COMMIT:0:7}" ]] || return 1
  printf '%s\n' "$verified_origin"
}

if [[ -e "$INSTALL_DIR" || -L "$INSTALL_DIR" ]]; then
  if cached_origin="$(verify_install)"; then
    echo "[tlapm] authenticated ${cached_origin} cache ready at ${INSTALL_DIR}"
    exit 0
  fi
  echo "refusing stale, partial, or unauthenticated TLAPM cache at ${INSTALL_DIR}" >&2
  exit 1
fi

if [[ ! -e "$INSTALL_PARENT" && ! -L "$INSTALL_PARENT" ]]; then
  /bin/mkdir -m 0700 "$INSTALL_PARENT"
fi
"$SOURCE_LOCK_PYTHON" -I -S "$FROZEN_HELPER" \
  --lock "$FROZEN_LOCK" --platform "$PLATFORM" \
  validate-private-directory --directory "$INSTALL_PARENT"

archive_path="${work_dir}/${ARCHIVE}"
source_build_attestation=""
source_build_lock=""
archive_origin=""
if [[ -n "${TLAPM_ARCHIVE_PATH:-}" ]]; then
  echo "[tlapm] authenticating caller-supplied archive for commit ${TLAPM_COMMIT}"
  "$SOURCE_LOCK_PYTHON" -I -S "$FROZEN_HELPER" \
    --lock "$FROZEN_LOCK" --platform "$PLATFORM" copy-checked-file \
    --source "$TLAPM_ARCHIVE_PATH" --destination "$archive_path" \
    --expected-sha256 "$ARCHIVE_SHA256" >/dev/null
  archive_origin="caller-archive"
else
  echo "[tlapm] requesting exact release asset ${RELEASE_ASSET_ID} for commit ${TLAPM_COMMIT}"
  set +e
    http_status="$(/usr/bin/env -i HOME="$work_dir" PATH="$SANITIZED_RUNTIME_PATH" \
      TMPDIR="$work_dir" LANG=C LC_ALL=C TZ=UTC \
      /usr/bin/curl --proto '=https' --tlsv1.2 --fail --location --retry 3 \
    --header 'Accept: application/octet-stream' \
    --header 'X-GitHub-Api-Version: 2022-11-28' \
    --write-out '%{http_code}' --output "$archive_path" "$URL")"
  curl_status=$?
  set -e
  if ! fetch_origin="$("$SOURCE_LOCK_PYTHON" -I -S "$FROZEN_HELPER" \
    --lock "$FROZEN_LOCK" --platform "$PLATFORM" classify-release-fetch \
    --curl-status "$curl_status" --http-status "${http_status:-000}")"; then
    echo "curl_status=${curl_status} http_status=${http_status:-unavailable}" >&2
    exit 1
  fi
  if [[ "$fetch_origin" == github-release-asset ]]; then
    archive_origin="github-release-asset"
  elif [[ "$fetch_origin" == immutable-source-build ]]; then
    echo "[tlapm] exact asset is permanently unavailable (HTTP ${http_status})" >&2
    echo "[tlapm] falling back to the frozen immutable source-build corridor" >&2
    /bin/rm -f "$archive_path"
    readonly SOURCE_BUILD_BUNDLE="${work_dir}/source-build-bundle"
    /usr/bin/env -i HOME="$work_dir" PATH="$SANITIZED_RUNTIME_PATH" \
      TMPDIR="$work_dir" LANG=C LC_ALL=C TZ=UTC \
      IROHA_RELEASE_POLICY_PYTHON="$SOURCE_LOCK_PYTHON" \
      TLAPM_SOURCE_BUILD_JOBS="${TLAPM_SOURCE_BUILD_JOBS:-2}" \
      /bin/bash "$FROZEN_SOURCE_BUILDER" \
      "$PLATFORM" "$SOURCE_BUILD_BUNDLE" "$SNAPSHOT_DIR" "$REPO_ROOT"
    [[ -d "$SOURCE_BUILD_BUNDLE" && ! -L "$SOURCE_BUILD_BUNDLE" ]] || {
      echo "source builder did not publish one output bundle" >&2
      exit 1
    }
    bundle_entries="$(/usr/bin/find "$SOURCE_BUILD_BUNDLE" -mindepth 1 -maxdepth 1 \
      -print | LC_ALL=C /usr/bin/sort)"
    expected_bundle_entries="$(printf '%s\n' \
      "${SOURCE_BUILD_BUNDLE}/archive.tar.gz" \
      "${SOURCE_BUILD_BUNDLE}/attestation.json" \
      "${SOURCE_BUILD_BUNDLE}/source-build-lock.json" | LC_ALL=C /usr/bin/sort)"
    [[ "$bundle_entries" == "$expected_bundle_entries" ]] || {
      echo "source-build output bundle has unexpected entries" >&2
      exit 1
    }
    [[ "$(hash_file "${SOURCE_BUILD_BUNDLE}/source-build-lock.json")" \
      == "$(hash_file "$FROZEN_LOCK")" ]] || {
      echo "source-build output lock differs from the frozen invocation lock" >&2
      exit 1
    }
    archive_path="${SOURCE_BUILD_BUNDLE}/archive.tar.gz"
    source_build_attestation="${SOURCE_BUILD_BUNDLE}/attestation.json"
    source_build_lock="${SOURCE_BUILD_BUNDLE}/source-build-lock.json"
    archive_origin="immutable-source-build"
  fi
fi

actual_sha256="$(hash_file "$archive_path")"
if [[ "$archive_origin" != immutable-source-build \
  && "$actual_sha256" != "$ARCHIVE_SHA256" ]]; then
  echo "TLAPM archive checksum mismatch" >&2
  echo "expected: ${ARCHIVE_SHA256}" >&2
  echo "actual:   ${actual_sha256}" >&2
  exit 1
fi

readonly EXTRACT_ROOT="${work_dir}/extract"
/bin/mkdir -m 0700 "$EXTRACT_ROOT"
/usr/bin/env -i HOME="$work_dir" PATH="$SANITIZED_RUNTIME_PATH" \
  TMPDIR="$work_dir" LANG=C LC_ALL=C TZ=UTC \
  /usr/bin/tar -xzf "$archive_path" -C "$EXTRACT_ROOT"
[[ -x "${EXTRACT_ROOT}/tlapm/bin/tlapm" ]] || {
  echo "pinned TLAPM archive has an unexpected layout" >&2
  exit 1
}
if [[ "$archive_origin" == immutable-source-build ]]; then
  "$SOURCE_LOCK_PYTHON" -I -S "$FROZEN_HELPER" \
    --lock "$FROZEN_LOCK" --platform "$PLATFORM" verify-attestation \
    --archive "$archive_path" --distribution-tree "$EXTRACT_ROOT" \
    --locked-wget "$FROZEN_LOCKED_WGET" \
    --source-builder "$FROZEN_SOURCE_BUILDER" \
    --attestation "$source_build_attestation"
fi

readonly INSTALL_STAGE="${work_dir}/install-stage"
/bin/mkdir -m 0700 "$INSTALL_STAGE"
/bin/cp -Rp "${EXTRACT_ROOT}/tlapm" "$INSTALL_STAGE/tlapm"
printf '%s\n' "$actual_sha256" > "${INSTALL_STAGE}/archive.sha256"
printf '%s\n' "$archive_origin" > "${INSTALL_STAGE}/archive.origin"
/bin/chmod 0400 "${INSTALL_STAGE}/archive.sha256" "${INSTALL_STAGE}/archive.origin"
if [[ "$archive_origin" == immutable-source-build ]]; then
  "$SOURCE_LOCK_PYTHON" -I -S "$FROZEN_HELPER" \
    --lock "$FROZEN_LOCK" --platform "$PLATFORM" copy-checked-file \
    --source "$source_build_attestation" \
    --destination "${INSTALL_STAGE}/source-build-attestation.json" >/dev/null
  "$SOURCE_LOCK_PYTHON" -I -S "$FROZEN_HELPER" \
    --lock "$FROZEN_LOCK" --platform "$PLATFORM" copy-checked-file \
    --source "$source_build_lock" \
    --destination "${INSTALL_STAGE}/source-build-lock.json" >/dev/null
fi

state_args=()
if [[ "$archive_origin" == immutable-source-build ]]; then
  state_args+=(--attestation "${INSTALL_STAGE}/source-build-attestation.json")
fi
"$SOURCE_LOCK_PYTHON" -I -S "$FROZEN_HELPER" \
  --lock "$FROZEN_LOCK" --platform "$PLATFORM" write-install-state \
  --directory "$INSTALL_STAGE" --origin "$archive_origin" \
  --archive-sha256 "$actual_sha256" --locked-wget "$FROZEN_LOCKED_WGET" \
  --source-builder "$FROZEN_SOURCE_BUILDER" \
  "${state_args[@]}" --output "${INSTALL_STAGE}/install-state.json"

"$SOURCE_LOCK_PYTHON" -I -S "$FROZEN_HELPER" \
  --lock "$FROZEN_LOCK" --platform "$PLATFORM" verify-install \
  --directory "$INSTALL_STAGE" --allowed-origin "$archive_origin" \
  --prebuilt-sha256 "$ARCHIVE_SHA256" --locked-wget "$FROZEN_LOCKED_WGET" \
  --source-builder "$FROZEN_SOURCE_BUILDER" \
  >/dev/null
[[ "$(run_tlapm_version "${INSTALL_STAGE}/tlapm/bin/tlapm")" \
  == "${TLAPM_COMMIT:0:7}" ]] || {
  echo "staged TLAPM does not identify commit ${TLAPM_COMMIT}" >&2
  exit 1
}

for pair in \
  "$SOURCE_BUILD_LOCK:$FROZEN_LOCK" \
  "$SOURCE_LOCK_HELPER:$FROZEN_HELPER" \
  "$SOURCE_LOCKED_WGET:$FROZEN_LOCKED_WGET" \
  "$SOURCE_BUILD_SCRIPT:$FROZEN_SOURCE_BUILDER"; do
  live_path="${pair%%:*}"
  frozen_path="${pair#*:}"
  [[ "$(hash_file "$live_path")" == "$(hash_file "$frozen_path")" ]] || {
    echo "TLAPM source-build corridor changed during the installer invocation" >&2
    exit 1
  }
done

set +e
"$SOURCE_LOCK_PYTHON" -I -S "$FROZEN_HELPER" \
  --lock "$FROZEN_LOCK" --platform "$PLATFORM" publish-install \
  --staged "$INSTALL_STAGE" --destination "$INSTALL_DIR"
publish_status=$?
set -e
if ((publish_status == 3)); then
  if raced_origin="$(verify_install)"; then
    echo "[tlapm] concurrent authenticated ${raced_origin} cache won publication"
    exit 0
  fi
  echo "concurrent TLAPM publication is stale or unauthenticated" >&2
  exit 1
elif ((publish_status != 0)); then
  exit "$publish_status"
fi

if ! installed_origin="$(verify_install)"; then
  echo "published TLAPM cache failed complete-closure authentication" >&2
  exit 1
fi
echo "[tlapm] atomically installed ${installed_origin} ${TLAPM_COMMIT} at ${INSTALL_DIR}"
