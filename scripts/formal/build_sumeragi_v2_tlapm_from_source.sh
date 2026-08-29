#!/usr/bin/env bash
set -euo pipefail
umask 077

readonly EXPECTED_TLAPM_VERSION="1.6.0-pre"
readonly EXPECTED_TLAPM_COMMIT="3ab43c7ff31db4ced850619d4746fa4c841a7681"
readonly EXPECTED_SOURCE_LOCK_SHA256="0b41cdcf512e045e7f46970f2f4a54f5f2a9031ab7ffa26f02e059130c3b7563"
readonly EXPECTED_OCAML_COMPILER_ATOM="ocaml-base-compiler.5.1.0"

usage() {
  echo "usage: $0 PLATFORM OUTPUT_BUNDLE [FROZEN_CORRIDOR_DIR REPOSITORY_ROOT]" >&2
  exit 2
}

[[ $# -eq 2 || $# -eq 4 ]] || usage
readonly PLATFORM="$1"
readonly OUTPUT_BUNDLE="$2"
readonly INPUT_SNAPSHOT_DIR="${3:-}"
if [[ $# -eq 4 ]]; then
  readonly REPO_ROOT="$4"
else
  readonly REPO_ROOT="$(cd -- "$(/usr/bin/dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
fi
[[ "$REPO_ROOT" == /* && "$REPO_ROOT" == "$(cd -P -- "$REPO_ROOT" && pwd)" ]] || {
  echo "source-build repository root is not absolute and canonical" >&2
  exit 1
}
readonly REPOSITORY_LOCK="${REPO_ROOT}/scripts/formal/sumeragi_v2_tlapm_source_build_lock.json"
readonly REPOSITORY_HELPER="${REPO_ROOT}/scripts/formal/sumeragi_v2_tlapm_source_lock.py"
readonly REPOSITORY_LOCKED_WGET="${REPO_ROOT}/scripts/formal/sumeragi_v2_tlapm_locked_wget.sh"
readonly REPOSITORY_BUILDER="${REPO_ROOT}/scripts/formal/build_sumeragi_v2_tlapm_from_source.sh"

case "$(/usr/bin/uname -s)-$(/usr/bin/uname -m):${PLATFORM}" in
  Linux-x86_64:x86_64-linux-gnu)
    readonly SANITIZED_HOST_PATH="/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
    ;;
  Darwin-arm64:arm64-darwin)
    readonly SANITIZED_HOST_PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
    ;;
  *)
    echo "source-build platform does not match the current host: ${PLATFORM}" >&2
    exit 1
    ;;
esac

case "$OUTPUT_BUNDLE" in
  /*) ;;
  *)
    echo "source-build output bundle must use an absolute path" >&2
    exit 1
    ;;
esac
readonly OUTPUT_PARENT="$(/usr/bin/dirname -- "$OUTPUT_BUNDLE")"
[[ "$OUTPUT_PARENT" == "$(cd -P -- "$OUTPUT_PARENT" && pwd)" ]] || {
  echo "source-build output parent is not canonical: ${OUTPUT_PARENT}" >&2
  exit 1
}
case "$OUTPUT_PARENT" in
  *[!A-Za-z0-9_./-]*)
    echo "source-build output parent contains unsupported shell metacharacters" >&2
    exit 1
    ;;
esac

readonly REQUESTED_LOCK_PYTHON="${IROHA_RELEASE_POLICY_PYTHON:-python3}"
LOCK_PYTHON_REQUEST_PATH="$(type -P "$REQUESTED_LOCK_PYTHON")" || {
  echo "${REQUESTED_LOCK_PYTHON} is required for the pinned TLAPM source build" >&2
  exit 1
}
LOCK_PYTHON="$("$LOCK_PYTHON_REQUEST_PATH" -I -S -c \
  'import os, sys; print(os.path.realpath(sys.executable))')"
readonly LOCK_PYTHON
[[ -f "$LOCK_PYTHON" && -x "$LOCK_PYTHON" && ! -L "$LOCK_PYTHON" ]] || {
  echo "TLAPM source-build Python must be one regular executable" >&2
  exit 1
}

if [[ -n "$INPUT_SNAPSHOT_DIR" ]]; then
  readonly BOOTSTRAP_LOCK="${INPUT_SNAPSHOT_DIR}/source-build-lock.json"
  readonly BOOTSTRAP_HELPER="${INPUT_SNAPSHOT_DIR}/source-lock.py"
  readonly BOOTSTRAP_LOCKED_WGET="${INPUT_SNAPSHOT_DIR}/locked-wget.sh"
  readonly BOOTSTRAP_BUILDER="${INPUT_SNAPSHOT_DIR}/source-builder.sh"
else
  readonly BOOTSTRAP_LOCK="$REPOSITORY_LOCK"
  readonly BOOTSTRAP_HELPER="$REPOSITORY_HELPER"
  readonly BOOTSTRAP_LOCKED_WGET="$REPOSITORY_LOCKED_WGET"
  readonly BOOTSTRAP_BUILDER="$REPOSITORY_BUILDER"
fi
[[ -f "$BOOTSTRAP_LOCK" && ! -L "$BOOTSTRAP_LOCK" \
  && -f "$BOOTSTRAP_HELPER" && ! -L "$BOOTSTRAP_HELPER" \
  && -f "$BOOTSTRAP_LOCKED_WGET" && ! -L "$BOOTSTRAP_LOCKED_WGET" \
  && -f "$BOOTSTRAP_BUILDER" && ! -L "$BOOTSTRAP_BUILDER" ]] || {
  echo "the TLAPM source-build snapshot is incomplete" >&2
  exit 1
}
if [[ -n "$INPUT_SNAPSHOT_DIR" ]]; then
  for pair in \
    "$REPOSITORY_LOCK:$BOOTSTRAP_LOCK" \
    "$REPOSITORY_HELPER:$BOOTSTRAP_HELPER" \
    "$REPOSITORY_LOCKED_WGET:$BOOTSTRAP_LOCKED_WGET" \
    "$REPOSITORY_BUILDER:$BOOTSTRAP_BUILDER"; do
    repository_path="${pair%%:*}"
    snapshot_path="${pair#*:}"
    repository_sha256="$("$LOCK_PYTHON" -I -S "$BOOTSTRAP_HELPER" \
      --lock "$BOOTSTRAP_LOCK" --platform "$PLATFORM" hash-file \
      --path "$repository_path")"
    snapshot_sha256="$("$LOCK_PYTHON" -I -S "$BOOTSTRAP_HELPER" \
      --lock "$BOOTSTRAP_LOCK" --platform "$PLATFORM" hash-file \
      --path "$snapshot_path")"
    [[ "$repository_sha256" == "$snapshot_sha256" ]] || {
      echo "caller source-build snapshot differs from the checked-in corridor" >&2
      exit 1
    }
  done
fi
"$LOCK_PYTHON" -I -S "$BOOTSTRAP_HELPER" \
  --lock "$BOOTSTRAP_LOCK" --platform "$PLATFORM" \
  validate-private-directory --directory "$OUTPUT_PARENT"

tmp_dir="$(/usr/bin/mktemp -d "${OUTPUT_PARENT}/.sumeragi-v2-tlapm-source.XXXXXX")"
tmp_dir="$(cd -P -- "$tmp_dir" && pwd)"
/bin/chmod 0700 "$tmp_dir"
trap '/bin/rm -rf -- "$tmp_dir"' EXIT

readonly SNAPSHOT_DIR="${tmp_dir}/corridor-snapshot"
"$LOCK_PYTHON" -I -S "$BOOTSTRAP_HELPER" \
  --lock "$BOOTSTRAP_LOCK" --platform "$PLATFORM" snapshot-corridor \
  --helper "$BOOTSTRAP_HELPER" --locked-wget "$BOOTSTRAP_LOCKED_WGET" \
  --source-builder "$BOOTSTRAP_BUILDER" \
  --output-dir "$SNAPSHOT_DIR"
readonly LOCK_MANIFEST="${SNAPSHOT_DIR}/source-build-lock.json"
readonly LOCK_HELPER="${SNAPSHOT_DIR}/source-lock.py"
readonly LOCKED_WGET="${SNAPSHOT_DIR}/locked-wget.sh"
readonly SOURCE_BUILDER="${SNAPSHOT_DIR}/source-builder.sh"

lock_shell="$("$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
  --lock "$LOCK_MANIFEST" --platform "$PLATFORM" emit-shell)" || exit $?
eval "$lock_shell"
readonly TLAPM_LOCK_SHA256 TLAPM_OPAM_BINARY_SHA256 TLAPM_OPAM_BINARY_URL
readonly TLAPM_OPAM_REPOSITORY_COMMIT TLAPM_OPAM_REPOSITORY_TREE
readonly TLAPM_OPAM_REPOSITORY_URL TLAPM_OPAM_VERSION TLAPM_PACKAGE_SET_SHA256
readonly TLAPM_SOURCE_COMMIT TLAPM_SOURCE_DATE_EPOCH TLAPM_SOURCE_REPOSITORY_URL
readonly TLAPM_SOURCE_TREE TLAPM_SOURCE_VERSION

[[ "$TLAPM_LOCK_SHA256" == "$EXPECTED_SOURCE_LOCK_SHA256" ]] || {
  echo "TLAPM source-build lock digest does not match the reviewed corridor" >&2
  exit 1
}

[[ "$TLAPM_SOURCE_VERSION" == "$EXPECTED_TLAPM_VERSION" ]] || {
  echo "TLAPM source lock version does not match the release corridor" >&2
  exit 1
}
[[ "$TLAPM_SOURCE_COMMIT" == "$EXPECTED_TLAPM_COMMIT" ]] || {
  echo "TLAPM source lock commit does not match the release corridor" >&2
  exit 1
}
[[ "$TLAPM_OPAM_VERSION" == "2.5.2" ]] || {
  echo "TLAPM source lock opam version does not match the reviewed builder" >&2
  exit 1
}

readonly BUILD_JOBS="${TLAPM_SOURCE_BUILD_JOBS:-2}"
[[ "$BUILD_JOBS" =~ ^[1-9][0-9]*$ ]] && ((BUILD_JOBS <= 64)) || {
  echo "TLAPM_SOURCE_BUILD_JOBS must be an integer from 1 through 64" >&2
  exit 1
}

readonly BUILD_HOME="${tmp_dir}/home"
readonly BUILD_TMP="${tmp_dir}/tmp"
readonly BUILD_XDG_CACHE="${tmp_dir}/xdg-cache"
readonly BUILD_XDG_CONFIG="${tmp_dir}/xdg-config"
readonly SOURCE_PIN_DIR="${tmp_dir}/tlapm-pin"
readonly SOURCE_DIR="${tmp_dir}/tlapm-build-overlay"
readonly OPAM_REPOSITORY_DIR="${tmp_dir}/opam-repository"
readonly OPAM_BINARY="${tmp_dir}/opam-${TLAPM_OPAM_VERSION}"
readonly OPAM_ROOT="${tmp_dir}/opam-root"
readonly OPAM_SWITCH="${tmp_dir}/opam-switch"
readonly BACKEND_CACHE="${tmp_dir}/backend-cache"
readonly BACKEND_RECEIPTS="${tmp_dir}/backend-receipts"
readonly CONTROLLED_BIN="${tmp_dir}/controlled-bin"
readonly COMPILER_ATOMS="${tmp_dir}/compiler-package-atoms.txt"
readonly BUILD_ATOMS="${tmp_dir}/build-package-atoms.txt"
readonly EXPECTED_ATOMS="${tmp_dir}/expected-package-atoms.txt"
readonly DARWIN_CONF_ATOMS="${tmp_dir}/darwin-conf-package-atoms.txt"
readonly DARWIN_INTERMEDIATE_ATOMS="${tmp_dir}/darwin-intermediate-package-atoms.txt"
readonly DARWIN_CXX_PREFLIGHT="${tmp_dir}/darwin-cxx-preflight"
readonly DARWIN_CXX_PREFLIGHT_OBJECT="${tmp_dir}/darwin-cxx-preflight.o"
readonly DARWIN_ZLIB_PREFLIGHT="${tmp_dir}/darwin-zlib-preflight"
DARWIN_CXXFLAGS=""
DARWIN_CPLUS_INCLUDE_PATH=""
readonly EXPECTED_PACKAGE_TABLE="${tmp_dir}/expected-package-table.tsv"
readonly BACKEND_TABLE="${tmp_dir}/backend-downloads.tsv"
/bin/mkdir -m 0700 "$BUILD_HOME" "$BUILD_TMP" "$BUILD_XDG_CACHE" \
  "$BUILD_XDG_CONFIG" "$SOURCE_PIN_DIR" "$SOURCE_DIR" "$OPAM_ROOT" \
  "$BACKEND_CACHE" "$BACKEND_RECEIPTS" "$CONTROLLED_BIN"

readonly ENV_BIN="/usr/bin/env"
clean_command() {
  "$ENV_BIN" -i \
    HOME="$BUILD_HOME" \
    PATH="$SANITIZED_HOST_PATH" \
    TMPDIR="$BUILD_TMP" \
    XDG_CACHE_HOME="$BUILD_XDG_CACHE" \
    XDG_CONFIG_HOME="$BUILD_XDG_CONFIG" \
    LANG=C LC_ALL=C TZ=UTC \
    GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null \
    GIT_TERMINAL_PROMPT=0 GIT_NO_REPLACE_OBJECTS=1 \
    "$@"
}

for required_command in awk cc chmod cp curl diff find g++ git make mkdir \
  mv patch shasum sort tar unzip xargs; do
  clean_command sh -c 'command -v "$1" >/dev/null 2>&1' sh "$required_command" || {
    echo "${required_command} is required for the pinned TLAPM source build" >&2
    exit 1
  }
done

hash_file() {
  "$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
    --lock "$LOCK_MANIFEST" --platform "$PLATFORM" hash-file --path "$1"
}

"$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
  --lock "$LOCK_MANIFEST" --platform "$PLATFORM" \
  emit-packages --group compiler --format atom > "$COMPILER_ATOMS"
"$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
  --lock "$LOCK_MANIFEST" --platform "$PLATFORM" \
  emit-packages --group build --format atom > "$BUILD_ATOMS"
"$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
  --lock "$LOCK_MANIFEST" --platform "$PLATFORM" \
  emit-packages --group all --format atom > "$EXPECTED_ATOMS"
"$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
  --lock "$LOCK_MANIFEST" --platform "$PLATFORM" \
  emit-packages --group all --format table > "$EXPECTED_PACKAGE_TABLE"
"$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
  --lock "$LOCK_MANIFEST" --platform "$PLATFORM" emit-backends > "$BACKEND_TABLE"
[[ "$(hash_file "$EXPECTED_PACKAGE_TABLE")" == "$TLAPM_PACKAGE_SET_SHA256" ]] || {
  echo "materialized TLAPM package lock digest mismatch" >&2
  exit 1
}

darwin_conf_packages=()
prepare_darwin_conf_boundary() {
  [[ "$PLATFORM" == "arm64-darwin" ]] || return 0

  local package_atom
  while IFS= read -r package_atom; do
    case "$package_atom" in
      conf-*) darwin_conf_packages+=("$package_atom") ;;
    esac
  done < "$BUILD_ATOMS"
  [[ ${#darwin_conf_packages[@]} -eq 3 \
    && "${darwin_conf_packages[0]}" == "conf-g++.1.0" \
    && "${darwin_conf_packages[1]}" == "conf-pkg-config.5" \
    && "${darwin_conf_packages[2]}" == "conf-zlib.1" ]] || {
    echo "Darwin TLAPM capability package lock does not match the reviewed boundary" >&2
    return 1
  }

  printf '%s\n' "${darwin_conf_packages[@]}" > "$DARWIN_CONF_ATOMS"
  clean_command cp "$COMPILER_ATOMS" "$DARWIN_INTERMEDIATE_ATOMS"
  printf '%s\n' "${darwin_conf_packages[@]}" >> "$DARWIN_INTERMEDIATE_ATOMS"
}

verify_darwin_depext_capabilities() {
  [[ "$PLATFORM" == "arm64-darwin" ]] || return 0

  local darwin_sdk_root darwin_cxx_include
  [[ -x /usr/bin/xcrun && ! -L /usr/bin/xcrun ]] || {
    echo "xcrun is required for the Darwin TLAPM source build" >&2
    return 1
  }
  darwin_sdk_root="$(clean_command /usr/bin/xcrun --sdk macosx --show-sdk-path)" || {
    echo "xcrun cannot resolve the Darwin SDK" >&2
    return 1
  }
  [[ "$darwin_sdk_root" == /* && -d "$darwin_sdk_root" ]] || {
    echo "xcrun returned an invalid Darwin SDK root" >&2
    return 1
  }
  darwin_sdk_root="$(cd -P -- "$darwin_sdk_root" && pwd)" || {
    echo "the Darwin SDK root is not canonical" >&2
    return 1
  }
  darwin_cxx_include="${darwin_sdk_root}/usr/include/c++/v1"
  [[ -d "$darwin_cxx_include" && ! -L "$darwin_cxx_include" \
    && -f "${darwin_cxx_include}/numeric" \
    && ! -L "${darwin_cxx_include}/numeric" ]] || {
    echo "the Darwin SDK lacks its canonical libc++ headers" >&2
    return 1
  }
  DARWIN_CXXFLAGS="-isystem ${darwin_cxx_include}"
  DARWIN_CPLUS_INCLUDE_PATH="$darwin_cxx_include"

  clean_command sh -c 'command -v "$1" >/dev/null 2>&1' sh pkg-config || {
    echo "pkg-config is required for the Darwin TLAPM source build" >&2
    return 1
  }
  [[ ! -e "$DARWIN_CXX_PREFLIGHT" && ! -L "$DARWIN_CXX_PREFLIGHT" \
    && ! -e "$DARWIN_CXX_PREFLIGHT_OBJECT" \
    && ! -L "$DARWIN_CXX_PREFLIGHT_OBJECT" ]] || {
    echo "Darwin C++ capability preflight destination already exists" >&2
    return 1
  }
  clean_command "$ENV_BIN" CPLUS_INCLUDE_PATH="$DARWIN_CPLUS_INCLUDE_PATH" \
    cc -std=c++17 -Wall -Wextra -Werror -pedantic \
    -x c++ -c - -o "$DARWIN_CXX_PREFLIGHT_OBJECT" <<'CPP'
#include <numeric>
#include <vector>

static_assert(__cplusplus == 201703L, "the TLAPM build requires exact C++17 mode");

int main() {
  const std::vector<int> values{1, 2, 3};
  return std::accumulate(values.begin(), values.end(), 0) == 6 ? 0 : 1;
}
CPP
  [[ -f "$DARWIN_CXX_PREFLIGHT_OBJECT" \
    && ! -L "$DARWIN_CXX_PREFLIGHT_OBJECT" ]] || {
    echo "Darwin C++ capability preflight did not produce one object" >&2
    return 1
  }
  clean_command g++ "$DARWIN_CXX_PREFLIGHT_OBJECT" -o "$DARWIN_CXX_PREFLIGHT"
  [[ -f "$DARWIN_CXX_PREFLIGHT" && ! -L "$DARWIN_CXX_PREFLIGHT" \
    && -x "$DARWIN_CXX_PREFLIGHT" ]] || {
    echo "Darwin C++ capability preflight did not produce one executable" >&2
    return 1
  }
  clean_command "$DARWIN_CXX_PREFLIGHT" || {
    echo "Darwin C++ capability preflight cannot execute" >&2
    return 1
  }

  clean_command pkg-config --exists zlib || {
    echo "pkg-config cannot resolve the Darwin zlib capability" >&2
    return 1
  }
  [[ ! -e "$DARWIN_ZLIB_PREFLIGHT" && ! -L "$DARWIN_ZLIB_PREFLIGHT" ]] || {
    echo "Darwin zlib capability preflight destination already exists" >&2
    return 1
  }
  clean_command cc -std=c11 -Wall -Wextra -Werror -pedantic \
    -x c - -lz -o "$DARWIN_ZLIB_PREFLIGHT" <<'C'
#include <string.h>
#include <zlib.h>

int main(void) {
  static const Bytef input[] = "immutable TLAPM zlib capability preflight";
  Bytef output[128];
  uLongf output_length = sizeof(output);
  if (strcmp(ZLIB_VERSION, zlibVersion()) != 0) {
    return 1;
  }
  if (compress2(output, &output_length, input, sizeof(input), Z_BEST_COMPRESSION)
      != Z_OK) {
    return 2;
  }
  return output_length > 0 && output_length < sizeof(output) ? 0 : 3;
}
C
  [[ -f "$DARWIN_ZLIB_PREFLIGHT" && ! -L "$DARWIN_ZLIB_PREFLIGHT" \
    && -x "$DARWIN_ZLIB_PREFLIGHT" ]] || {
    echo "Darwin zlib capability preflight did not produce one executable" >&2
    return 1
  }
  clean_command "$DARWIN_ZLIB_PREFLIGHT" || {
    echo "Darwin zlib capability preflight cannot execute" >&2
    return 1
  }
}

checkout_exact_tree() {
  local label="$1" repository="$2" commit="$3" tree="$4" destination="$5"
  local actual_commit actual_tree tracked_status
  clean_command git -C "$destination" init --quiet
  clean_command git -C "$destination" remote add origin "$repository"
  clean_command git -c protocol.version=2 -C "$destination" fetch --quiet \
    --no-tags --depth=1 origin "$commit"
  clean_command git -C "$destination" checkout --quiet --detach FETCH_HEAD
  actual_commit="$(clean_command git -C "$destination" rev-parse --verify HEAD)"
  actual_tree="$(clean_command git -C "$destination" rev-parse --verify 'HEAD^{tree}')"
  tracked_status="$(clean_command git -C "$destination" status --porcelain=v1 --untracked-files=no)"
  [[ "$actual_commit" == "$commit" && "$actual_tree" == "$tree" ]] || {
    echo "${label} immutable commit/tree mismatch" >&2
    return 1
  }
  [[ -z "$tracked_status" ]] || {
    echo "${label} checkout has modified tracked files" >&2
    return 1
  }
}

verify_exact_tree() {
  local label="$1" destination="$2" commit="$3" tree="$4"
  local actual_commit actual_tree tracked_status
  actual_commit="$(clean_command git -C "$destination" rev-parse --verify HEAD)"
  actual_tree="$(clean_command git -C "$destination" rev-parse --verify 'HEAD^{tree}')"
  tracked_status="$(clean_command git -C "$destination" status --porcelain=v1 --untracked-files=no)"
  [[ "$actual_commit" == "$commit" && "$actual_tree" == "$tree" \
    && -z "$tracked_status" ]] || {
    echo "${label} changed during the source build" >&2
    return 1
  }
}

verify_build_source_checkout() {
  local git_dir="${SOURCE_DIR}/.git"
  local actual_common_dir actual_description actual_git_dir actual_remotes
  local actual_shallow actual_shallow_sha actual_toplevel canonical_source_dir
  local expected_shallow_sha linked_git_file shallow_marker unsupported_git_entry
  [[ -d "$git_dir" && ! -L "$git_dir" \
    && -d "${git_dir}/objects" && ! -L "${git_dir}/objects" ]] || {
    echo "TLAPM build source does not have independent Git metadata" >&2
    return 1
  }
  [[ ! -e "${git_dir}/commondir" && ! -L "${git_dir}/commondir" ]] || {
    echo "TLAPM build source may not use a shared Git common directory" >&2
    return 1
  }
  [[ ! -e "${git_dir}/objects/info/alternates" \
    && ! -L "${git_dir}/objects/info/alternates" ]] || {
    echo "TLAPM build source may not use shared Git objects" >&2
    return 1
  }
  unsupported_git_entry="$(clean_command find "$git_dir" \
    ! -type d ! -type f -print -quit)"
  [[ -z "$unsupported_git_entry" ]] || {
    echo "TLAPM build source Git metadata contains a link or special entry" >&2
    return 1
  }
  linked_git_file="$(clean_command find "$git_dir" \
    -type f -links +1 -print -quit)"
  [[ -z "$linked_git_file" ]] || {
    echo "TLAPM build source may not use hard-linked Git metadata" >&2
    return 1
  }
  shallow_marker="$(clean_command find "${git_dir}/shallow" \
    -type f -links 1 -print -quit)"
  [[ "$shallow_marker" == "${git_dir}/shallow" \
    && ! -L "${git_dir}/shallow" ]] || {
    echo "TLAPM build source shallow marker is not one independent file" >&2
    return 1
  }
  actual_shallow_sha="$(hash_file "${git_dir}/shallow")"
  expected_shallow_sha="$(printf '%s\n' "$TLAPM_SOURCE_COMMIT" \
    | clean_command shasum -a 256 | clean_command awk '{print $1}')"
  [[ "$actual_shallow_sha" == "$expected_shallow_sha" ]] || {
    echo "TLAPM build source shallow marker does not name the pinned commit" >&2
    return 1
  }
  canonical_source_dir="$(cd -P -- "$SOURCE_DIR" 2>/dev/null && pwd)" || {
    echo "TLAPM build source is not one canonical local directory" >&2
    return 1
  }
  actual_toplevel="$(clean_command git -C "$SOURCE_DIR" \
    rev-parse --show-toplevel)"
  actual_git_dir="$(clean_command git -C "$SOURCE_DIR" \
    rev-parse --absolute-git-dir)"
  actual_common_dir="$(clean_command git -C "$SOURCE_DIR" \
    rev-parse --path-format=absolute --git-common-dir)"
  [[ "$canonical_source_dir" == "$SOURCE_DIR" \
    && "$actual_toplevel" == "$SOURCE_DIR" \
    && "$actual_git_dir" == "$git_dir" \
    && "$actual_common_dir" == "$git_dir" ]] || {
    echo "TLAPM build source Git metadata escaped its canonical checkout" >&2
    return 1
  }
  verify_exact_tree "TLAPM build source" "$SOURCE_DIR" \
    "$TLAPM_SOURCE_COMMIT" "$TLAPM_SOURCE_TREE"
  actual_remotes="$(clean_command git -C "$SOURCE_DIR" remote)"
  [[ -z "$actual_remotes" ]] || {
    echo "TLAPM build source retained a remote after local checkout" >&2
    return 1
  }
  actual_shallow="$(clean_command git -C "$SOURCE_DIR" \
    rev-parse --is-shallow-repository)"
  [[ "$actual_shallow" == true ]] || {
    echo "TLAPM build source is not one shallow exact checkout" >&2
    return 1
  }
  actual_description="$(clean_command git -C "$SOURCE_DIR" \
    describe --always --dirty --abbrev=7)"
  [[ "$actual_description" == "${TLAPM_SOURCE_COMMIT:0:7}" ]] || {
    echo "TLAPM build source does not expose the exact pinned VCS identity" >&2
    echo "expected: ${TLAPM_SOURCE_COMMIT:0:7}" >&2
    echo "actual:   ${actual_description}" >&2
    return 1
  }
}

seal_build_source_checkout() {
  local actual_origin actual_push_origin canonical_source_pin
  [[ -d "${SOURCE_DIR}/.git" && ! -L "${SOURCE_DIR}/.git" ]] || {
    echo "TLAPM build source origin is not held by independent Git metadata" >&2
    return 1
  }
  canonical_source_pin="$(cd -P -- "$SOURCE_PIN_DIR" 2>/dev/null && pwd)" || {
    echo "TLAPM source pin is not one canonical local checkout" >&2
    return 1
  }
  actual_origin="$(clean_command git -C "$SOURCE_DIR" \
    remote get-url --all origin)"
  actual_push_origin="$(clean_command git -C "$SOURCE_DIR" \
    remote get-url --push --all origin)"
  [[ "$canonical_source_pin" == "$SOURCE_PIN_DIR" \
    && "$actual_origin" == "$SOURCE_PIN_DIR" \
    && "$actual_push_origin" == "$SOURCE_PIN_DIR" ]] || {
    echo "TLAPM build source origin is not the canonical local source pin" >&2
    return 1
  }
  clean_command git -C "$SOURCE_DIR" remote remove origin
  verify_build_source_checkout
}

download_curl_attempt() {
  local url="$1" partial="$2" resume="$3"
  if [[ "$resume" == 1 ]]; then
    set -- --continue-at -
  else
    set --
  fi
  clean_command curl --disable --proto '=https' --proto-redir '=https' \
    --tlsv1.2 --fail --location "$@" --write-out '%{http_code}' \
    --output "$partial" "$url"
}

download_curl_status_is_transient() {
  case "$1" in
    18|28|35|52|55|56) return 0 ;;
    *) return 1 ;;
  esac
}

download_retry_sleep() {
  case "$1" in
    1|2|4) /bin/sleep "$1" ;;
    *) return 1 ;;
  esac
}

validate_download_directory() {
  "$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
    --lock "$LOCK_MANIFEST" --platform "$PLATFORM" \
    validate-private-directory --directory "$1"
}

download_checked() {
  local label="$1" url="$2" expected_sha256="$3" destination="$4"
  local destination_parent partial actual_sha256 http_status expected_http_status
  local attempt=1 curl_status retry_delay resume
  local -r max_attempts=4
  case "$destination" in
    "${tmp_dir}"/*) ;;
    *)
      echo "${label} cache destination escaped the private build root" >&2
      return 1
      ;;
  esac
  [[ ! -e "$destination" && ! -L "$destination" ]] || {
    echo "${label} cache destination already exists" >&2
    return 1
  }
  destination_parent="$(/usr/bin/dirname -- "$destination")"
  clean_command mkdir -p "$destination_parent"
  clean_command chmod 0700 "$destination_parent"
  validate_download_directory "$destination_parent"
  [[ ! -e "$destination" && ! -L "$destination" ]] || {
    echo "${label} cache destination appeared during preparation" >&2
    return 1
  }
  partial="$(/usr/bin/mktemp "${destination}.partial.XXXXXX")"
  clean_command chmod 0600 "$partial"
  actual_sha256="$(hash_file "$partial")"
  [[ "$actual_sha256" == \
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" ]] || {
    echo "${label} partial download was not created empty" >&2
    return 1
  }

  while ((attempt <= max_attempts)); do
    validate_download_directory "$destination_parent"
    actual_sha256="$(hash_file "$partial")"
    resume=0
    expected_http_status=200
    if [[ -s "$partial" ]]; then
      resume=1
      expected_http_status=206
    fi

    if http_status="$(download_curl_attempt "$url" "$partial" "$resume")"; then
      curl_status=0
    else
      curl_status=$?
    fi
    validate_download_directory "$destination_parent"
    actual_sha256="$(hash_file "$partial")"

    if ((curl_status == 0)); then
      [[ "$http_status" == "$expected_http_status" ]] || {
        echo "${label} download returned unexpected HTTP ${http_status}" >&2
        return 1
      }
      [[ "$actual_sha256" == "$expected_sha256" ]] || {
        echo "${label} checksum mismatch" >&2
        echo "expected: ${expected_sha256}" >&2
        echo "actual:   ${actual_sha256}" >&2
        return 1
      }
      break
    fi

    download_curl_status_is_transient "$curl_status" || {
      echo "${label} download failed with terminal curl status ${curl_status}" >&2
      return "$curl_status"
    }
    [[ "$http_status" == 000 || "$http_status" == "$expected_http_status" ]] || {
      echo "${label} transient transport failure returned unexpected HTTP ${http_status}" >&2
      return "$curl_status"
    }
    if [[ "$actual_sha256" == "$expected_sha256" ]]; then
      break
    fi
    if ((attempt == max_attempts)); then
      echo "${label} exhausted ${max_attempts} bounded download attempts" >&2
      return "$curl_status"
    fi
    retry_delay=$((1 << (attempt - 1)))
    echo "[tlapm] ${label} transient curl ${curl_status}; retrying in ${retry_delay}s" >&2
    download_retry_sleep "$retry_delay"
    attempt=$((attempt + 1))
  done

  validate_download_directory "$destination_parent"
  actual_sha256="$(hash_file "$partial")"
  [[ "$actual_sha256" == "$expected_sha256" ]] || {
    echo "${label} checksum changed before promotion" >&2
    return 1
  }
  [[ ! -e "$destination" && ! -L "$destination" ]] || {
    echo "${label} cache destination appeared before promotion" >&2
    return 1
  }
  clean_command mv "$partial" "$destination"
  clean_command chmod 0400 "$destination"
}

verify_checked_file() {
  local label="$1" expected_sha256="$2" path="$3"
  [[ -f "$path" && ! -L "$path" \
    && "$(hash_file "$path")" == "$expected_sha256" ]] || {
    echo "${label} changed during the source build" >&2
    return 1
  }
}

prepare_darwin_conf_boundary
readonly -a darwin_conf_packages
verify_darwin_depext_capabilities
readonly DARWIN_CXXFLAGS
readonly DARWIN_CPLUS_INCLUDE_PATH

echo "[tlapm] fetching immutable source commit ${TLAPM_SOURCE_COMMIT}"
checkout_exact_tree "TLAPM source" "$TLAPM_SOURCE_REPOSITORY_URL" \
  "$TLAPM_SOURCE_COMMIT" "$TLAPM_SOURCE_TREE" "$SOURCE_PIN_DIR"
echo "[tlapm] preparing the independent VCS-bearing build checkout"
checkout_exact_tree "TLAPM build source" "$SOURCE_PIN_DIR" \
  "$TLAPM_SOURCE_COMMIT" "$TLAPM_SOURCE_TREE" "$SOURCE_DIR"
seal_build_source_checkout
echo "[tlapm] fetching immutable opam repository commit ${TLAPM_OPAM_REPOSITORY_COMMIT}"
mkdir -m 0700 "$OPAM_REPOSITORY_DIR"
checkout_exact_tree "opam repository" "$TLAPM_OPAM_REPOSITORY_URL" \
  "$TLAPM_OPAM_REPOSITORY_COMMIT" "$TLAPM_OPAM_REPOSITORY_TREE" "$OPAM_REPOSITORY_DIR"

echo "[tlapm] downloading pinned opam ${TLAPM_OPAM_VERSION}"
download_checked "opam ${TLAPM_OPAM_VERSION}" "$TLAPM_OPAM_BINARY_URL" \
  "$TLAPM_OPAM_BINARY_SHA256" "$OPAM_BINARY"
chmod 0500 "$OPAM_BINARY"
[[ "$(clean_command "$OPAM_BINARY" --version)" == "$TLAPM_OPAM_VERSION" ]] || {
  echo "downloaded opam does not identify version ${TLAPM_OPAM_VERSION}" >&2
  exit 1
}

z3_cache_path=""
z3_output_sha256=""
while IFS=$'\t' read -r backend_name backend_url backend_sha256 \
  backend_destination backend_output_sha256 _backend_architecture; do
  [[ -n "$backend_name" && -n "$backend_url" && -n "$backend_sha256" \
    && -n "$backend_destination" ]] || {
    echo "materialized backend lock record is incomplete" >&2
    exit 1
  }
  echo "[tlapm] downloading pinned ${backend_name} input"
  download_checked "$backend_name" "$backend_url" "$backend_sha256" \
    "${BACKEND_CACHE}/${backend_destination}"
  if [[ "$backend_name" == z3 ]]; then
    z3_cache_path="${BACKEND_CACHE}/${backend_destination}"
    z3_output_sha256="$backend_output_sha256"
  fi
done < "$BACKEND_TABLE"
[[ -n "$z3_cache_path" && "$z3_output_sha256" != - ]] || {
  echo "the platform lock does not identify its exact Z3 runtime member" >&2
  exit 1
}

readonly Z3_PREFLIGHT="${tmp_dir}/z3-preflight"
z3_member="$(basename -- "${z3_cache_path%.zip}")/bin/z3"
clean_command unzip -p "$z3_cache_path" "$z3_member" > "$Z3_PREFLIGHT"
chmod 0500 "$Z3_PREFLIGHT"
[[ "$(hash_file "$Z3_PREFLIGHT")" == "$z3_output_sha256" ]] || {
  echo "locked Z3 runtime member checksum mismatch" >&2
  exit 1
}
if ! z3_version="$(clean_command "$Z3_PREFLIGHT" -version 2>&1)"; then
  echo "locked Z3 4.8.9 runtime cannot execute on ${PLATFORM}; arm64 macOS requires Rosetta" >&2
  exit 1
fi
[[ "$z3_version" == *"Z3 version 4.8.9"* ]] || {
  echo "locked Z3 runtime does not identify version 4.8.9" >&2
  exit 1
}

clean_command cp "$LOCKED_WGET" "${CONTROLLED_BIN}/wget"
clean_command chmod 0500 "${CONTROLLED_BIN}/wget"

opam_command() {
  "$ENV_BIN" -i \
    HOME="$BUILD_HOME" PATH="$SANITIZED_HOST_PATH" TMPDIR="$BUILD_TMP" \
    XDG_CACHE_HOME="$BUILD_XDG_CACHE" XDG_CONFIG_HOME="$BUILD_XDG_CONFIG" \
    LANG=C LC_ALL=C TZ=UTC \
    OPAMROOT="$OPAM_ROOT" OPAMYES=1 OPAMCOLOR=never OPAMNOSELFUPGRADE=1 \
    OPAMDOWNLOADJOBS=1 OPAMJOBS="$BUILD_JOBS" OPAMPRECISETRACKING=1 \
    OPAMREQUIRECHECKSUMS=true \
    CXXFLAGS="$DARWIN_CXXFLAGS" \
    CPLUS_INCLUDE_PATH="$DARWIN_CPLUS_INCLUDE_PATH" \
    GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null \
    GIT_TERMINAL_PROMPT=0 GIT_NO_REPLACE_OBJECTS=1 \
    "$OPAM_BINARY" "$@"
}

verify_package_set() {
  local label="$1" expected_atoms="$2"
  local actual_atoms="${tmp_dir}/${label}-actual-package-atoms.txt"
  local sorted_expected="${tmp_dir}/${label}-expected-package-atoms.txt"
  opam_command list --switch "$OPAM_SWITCH" --installed --short \
    --columns=package > "$actual_atoms"
  LC_ALL=C sort -o "$actual_atoms" "$actual_atoms"
  LC_ALL=C sort "$expected_atoms" > "$sorted_expected"
  if ! diff -u "$sorted_expected" "$actual_atoms"; then
    echo "${label} opam package set does not match the checked-in lock" >&2
    return 1
  fi
}

echo "[tlapm] initializing the locked opam repository and OCaml switch"
opam_command init --bare --no-setup --disable-sandboxing locked "$OPAM_REPOSITORY_DIR"
opam_command switch create "$OPAM_SWITCH" "$EXPECTED_OCAML_COMPILER_ATOM" \
  --repositories=locked --yes
OPAM_SWITCH_PREFIX_PATH="$(opam_command var --switch "$OPAM_SWITCH" prefix)"
readonly OPAM_SWITCH_PREFIX_PATH
case "$OPAM_SWITCH_PREFIX_PATH" in
  "${tmp_dir}"/*) ;;
  *)
    echo "locked opam switch prefix escaped the private build root" >&2
    exit 1
    ;;
esac
verify_package_set compiler "$COMPILER_ATOMS"

build_packages=()
while IFS= read -r package_atom; do
  [[ -n "$package_atom" ]] || {
    echo "empty package atom in the checked-in build lock" >&2
    exit 1
  }
  build_packages+=("$package_atom")
done < "$BUILD_ATOMS"
(( ${#build_packages[@]} > 0 )) || {
  echo "the checked-in TLAPM build package lock is empty" >&2
  exit 1
}
if [[ "$PLATFORM" == "arm64-darwin" ]]; then
  echo "[tlapm] validating the exact Darwin host capability packages"
  opam_command install --assume-depexts --switch "$OPAM_SWITCH" --yes \
    "${darwin_conf_packages[@]}" < /dev/null
  verify_package_set darwin-conf "$DARWIN_INTERMEDIATE_ATOMS"
fi
echo "[tlapm] installing the exact locked package closure"
opam_command install --switch "$OPAM_SWITCH" --yes \
  "${build_packages[@]}" < /dev/null
verify_package_set complete "$EXPECTED_ATOMS"

verify_exact_tree "TLAPM source pin" "$SOURCE_PIN_DIR" "$TLAPM_SOURCE_COMMIT" "$TLAPM_SOURCE_TREE"
verify_build_source_checkout
verify_exact_tree "opam repository" "$OPAM_REPOSITORY_DIR" \
  "$TLAPM_OPAM_REPOSITORY_COMMIT" "$TLAPM_OPAM_REPOSITORY_TREE"

echo "[tlapm] building through the exact locked-download resolver"
opam_command exec --switch "$OPAM_SWITCH" -- \
  "$ENV_BIN" \
    -u MAKEFLAGS -u MFLAGS -u GNUMAKEFLAGS \
    -u DUNE_CONFIG__CACHE -u DUNE_CACHE_ROOT \
    -u OPAMFETCH -u OPAMNOCHECKSUMS -u OPAMREPOSITORYTARRING \
    HOME="$BUILD_HOME" \
    PATH="${CONTROLLED_BIN}:${OPAM_SWITCH_PREFIX_PATH}/bin:${SANITIZED_HOST_PATH}" \
    TMPDIR="$BUILD_TMP" XDG_CACHE_HOME="$BUILD_XDG_CACHE" \
    XDG_CONFIG_HOME="$BUILD_XDG_CONFIG" LANG=C LC_ALL=C TZ=UTC \
    MAKEFLAGS= MFLAGS= GNUMAKEFLAGS= DUNE_CACHE=disabled \
    CXXFLAGS="$DARWIN_CXXFLAGS" \
    CPLUS_INCLUDE_PATH="$DARWIN_CPLUS_INCLUDE_PATH" \
    SOURCE_DATE_EPOCH="$TLAPM_SOURCE_DATE_EPOCH" \
    TLAPM_LOCKED_WGET_PYTHON="$LOCK_PYTHON" \
    TLAPM_LOCKED_WGET_HELPER="$LOCK_HELPER" \
    TLAPM_LOCKED_WGET_LOCK="$LOCK_MANIFEST" \
    TLAPM_LOCKED_WGET_PLATFORM="$PLATFORM" \
    TLAPM_LOCKED_WGET_CACHE="$BACKEND_CACHE" \
    TLAPM_LOCKED_WGET_OUTPUT_ROOT="$SOURCE_DIR" \
    TLAPM_LOCKED_WGET_RECEIPTS="$BACKEND_RECEIPTS" \
    make --jobs=1 -C "$SOURCE_DIR" release \
      "RELEASE_VERSION=${TLAPM_SOURCE_VERSION}"

verify_build_source_checkout
"$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
  --lock "$LOCK_MANIFEST" --platform "$PLATFORM" verify-wget-receipts \
  --receipt-dir "$BACKEND_RECEIPTS"
while IFS=$'\t' read -r backend_name _backend_url backend_sha256 \
  backend_destination _backend_output_sha256 _backend_architecture; do
  verify_checked_file "$backend_name" "$backend_sha256" \
    "${BACKEND_CACHE}/${backend_destination}"
done < "$BACKEND_TABLE"

readonly UPSTREAM_ARCHIVE="${SOURCE_DIR}/_build/tlapm-${TLAPM_SOURCE_VERSION}-${PLATFORM}.tar.gz"
readonly UPSTREAM_BINARY="${SOURCE_DIR}/_build/tlapm/bin/tlapm"
[[ -f "$UPSTREAM_ARCHIVE" && ! -L "$UPSTREAM_ARCHIVE" \
  && -x "$UPSTREAM_BINARY" ]] || {
  echo "TLAPM source build did not produce the locked release layout" >&2
  exit 1
}
if ! built_version="$(clean_command "$UPSTREAM_BINARY" --version 2>&1)"; then
  echo "source-built TLAPM cannot report its identity" >&2
  echo "actual:   ${built_version}" >&2
  exit 1
fi
[[ "$built_version" == "${TLAPM_SOURCE_COMMIT:0:7}" ]] || {
  echo "source-built TLAPM does not identify commit ${TLAPM_SOURCE_COMMIT}" >&2
  echo "expected: ${TLAPM_SOURCE_COMMIT:0:7}" >&2
  echo "actual:   ${built_version}" >&2
  exit 1
}

echo "[tlapm] projecting the tested build into a clean release tree"
readonly AUTHENTICATED_DISTRIBUTION_PARENT="${tmp_dir}/authenticated-distribution"
readonly AUTHENTICATED_DISTRIBUTION="${AUTHENTICATED_DISTRIBUTION_PARENT}/tlapm"
mkdir -m 0700 "$AUTHENTICATED_DISTRIBUTION_PARENT"
opam_command exec --switch "$OPAM_SWITCH" -- \
  "$ENV_BIN" -i \
    HOME="$BUILD_HOME" \
    PATH="${OPAM_SWITCH_PREFIX_PATH}/bin:${SANITIZED_HOST_PATH}" \
    TMPDIR="$BUILD_TMP" XDG_CACHE_HOME="$BUILD_XDG_CACHE" \
    XDG_CONFIG_HOME="$BUILD_XDG_CONFIG" LANG=C LC_ALL=C TZ=UTC \
    CXXFLAGS="$DARWIN_CXXFLAGS" \
    CPLUS_INCLUDE_PATH="$DARWIN_CPLUS_INCLUDE_PATH" \
    dune install --root "$SOURCE_DIR" --relocatable \
      --prefix "$AUTHENTICATED_DISTRIBUTION"
readonly BUILT_ISABELLE="${SOURCE_DIR}/_build/default/deps/isabelle/Isabelle"
readonly BUILT_ISABELLE_EXEC_FILES="${SOURCE_DIR}/_build/default/deps/isabelle/Isabelle.exec-files"
readonly PROJECTED_ISABELLE="${AUTHENTICATED_DISTRIBUTION}/lib/tlapm/backends/Isabelle"
readonly PROJECTED_ISABELLE_EXEC_FILES="${AUTHENTICATED_DISTRIBUTION}/lib/tlapm/backends/Isabelle.exec-files"
[[ -d "$BUILT_ISABELLE" && ! -L "$BUILT_ISABELLE" \
  && -f "$BUILT_ISABELLE_EXEC_FILES" \
  && ! -L "$BUILT_ISABELLE_EXEC_FILES" \
  && -d "$PROJECTED_ISABELLE" && ! -L "$PROJECTED_ISABELLE" \
  && -f "$PROJECTED_ISABELLE_EXEC_FILES" \
  && ! -L "$PROJECTED_ISABELLE_EXEC_FILES" ]] || {
  echo "clean TLAPM release projection lacks its Isabelle derivation" >&2
  exit 1
}
# Dune's directory installation may materialize symlink leaves on Darwin. Keep
# the package byte-derived from the tested build tree by projecting the locked
# Isabelle output explicitly; the post-install rule below restores only the
# reviewed executable set recorded alongside that output.
/bin/rm -rf -- "$PROJECTED_ISABELLE"
/bin/cp -R "$BUILT_ISABELLE" "$PROJECTED_ISABELLE"
/bin/cp -p "$BUILT_ISABELLE_EXEC_FILES" "$PROJECTED_ISABELLE_EXEC_FILES"
clean_command make --jobs=1 -C "$AUTHENTICATED_DISTRIBUTION/lib/tlapm" \
  -f Makefile.post-install
readonly BUILT_BINARY="${AUTHENTICATED_DISTRIBUTION}/bin/tlapm"
[[ -x "$BUILT_BINARY" ]] || {
  echo "clean TLAPM release projection lacks its executable" >&2
  exit 1
}
if ! projected_version="$(clean_command "$BUILT_BINARY" --version 2>&1)"; then
  echo "clean TLAPM release projection cannot report its identity" >&2
  echo "actual:   ${projected_version}" >&2
  exit 1
fi
[[ "$projected_version" == "${TLAPM_SOURCE_COMMIT:0:7}" ]] || {
  echo "clean TLAPM release projection has an invalid identity" >&2
  echo "expected: ${TLAPM_SOURCE_COMMIT:0:7}" >&2
  echo "actual:   ${projected_version}" >&2
  exit 1
}
readonly BUILT_ARCHIVE="${tmp_dir}/tlapm-${TLAPM_SOURCE_VERSION}-${PLATFORM}.tar.gz"
clean_command "$ENV_BIN" COPYFILE_DISABLE=1 \
  tar -czf "$BUILT_ARCHIVE" -C "$AUTHENTICATED_DISTRIBUTION_PARENT" tlapm
[[ -f "$BUILT_ARCHIVE" && ! -L "$BUILT_ARCHIVE" ]] || {
  echo "clean TLAPM release projection did not produce one archive" >&2
  exit 1
}

verify_package_set post-build "$EXPECTED_ATOMS"
verify_exact_tree "TLAPM source pin" "$SOURCE_PIN_DIR" "$TLAPM_SOURCE_COMMIT" "$TLAPM_SOURCE_TREE"
verify_exact_tree "opam repository" "$OPAM_REPOSITORY_DIR" \
  "$TLAPM_OPAM_REPOSITORY_COMMIT" "$TLAPM_OPAM_REPOSITORY_TREE"

readonly ARCHIVE_CHECK="${tmp_dir}/archive-check"
mkdir -m 0700 "$ARCHIVE_CHECK"
clean_command tar -xzf "$BUILT_ARCHIVE" -C "$ARCHIVE_CHECK"
readonly ARCHIVED_BINARY="${ARCHIVE_CHECK}/tlapm/bin/tlapm"
if [[ ! -x "$ARCHIVED_BINARY" ]] \
  || ! archived_version="$(clean_command "$ARCHIVED_BINARY" --version 2>&1)"; then
  echo "source-built TLAPM archive has an invalid layout or identity" >&2
  echo "actual:   ${archived_version:-unavailable}" >&2
  exit 1
fi
[[ "$archived_version" == "${TLAPM_SOURCE_COMMIT:0:7}" ]] || {
  echo "source-built TLAPM archive has an invalid identity" >&2
  echo "expected: ${TLAPM_SOURCE_COMMIT:0:7}" >&2
  echo "actual:   ${archived_version}" >&2
  exit 1
}

readonly ATTESTATION="${tmp_dir}/source-build-attestation.json"
"$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
  --lock "$LOCK_MANIFEST" --platform "$PLATFORM" write-attestation \
  --archive "$BUILT_ARCHIVE" --build-tree "$SOURCE_DIR" \
  --distribution-tree "$AUTHENTICATED_DISTRIBUTION_PARENT" \
  --locked-wget "$LOCKED_WGET" \
  --source-builder "$SOURCE_BUILDER" \
  --output "$ATTESTATION"
"$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
  --lock "$LOCK_MANIFEST" --platform "$PLATFORM" verify-attestation \
  --archive "$BUILT_ARCHIVE" --distribution-tree "$ARCHIVE_CHECK" \
  --locked-wget "$LOCKED_WGET" --source-builder "$SOURCE_BUILDER" \
  --attestation "$ATTESTATION"

for pair in \
  "$BOOTSTRAP_LOCK:$LOCK_MANIFEST" \
  "$BOOTSTRAP_HELPER:$LOCK_HELPER" \
  "$BOOTSTRAP_LOCKED_WGET:$LOCKED_WGET" \
  "$BOOTSTRAP_BUILDER:$SOURCE_BUILDER"; do
  bootstrap_path="${pair%%:*}"
  frozen_path="${pair#*:}"
  [[ "$(hash_file "$bootstrap_path")" == "$(hash_file "$frozen_path")" ]] || {
    echo "TLAPM source-build corridor changed during the long build" >&2
    exit 1
  }
done

"$LOCK_PYTHON" -I -S "$LOCK_HELPER" \
  --lock "$LOCK_MANIFEST" --platform "$PLATFORM" publish-output-bundle \
  --archive "$BUILT_ARCHIVE" --attestation "$ATTESTATION" \
  --output-bundle "$OUTPUT_BUNDLE"
echo "[tlapm] source-built immutable commit ${TLAPM_SOURCE_COMMIT}"
