#!/usr/bin/env bash
# Shared no-interference policy for Sumeragi v2 release and validation runners.
#
# This file must be sourced. It never signals, reprioritizes, or otherwise
# controls an observed process. Cargo gates wait for pre-existing Cargo, rustc,
# and rustfmt commands to finish naturally. Operator cancellation is a
# canonical owner-only marker checked between gates; it never interrupts an
# in-flight command.

if [[ "${SUMERAGI_V2_RELEASE_PROCESS_POLICY_LOADED:-0}" == 1 ]]; then
  return 0
fi
readonly SUMERAGI_V2_RELEASE_PROCESS_POLICY_LOADED=1

readonly SUMERAGI_V2_RELEASE_CANCELLED_STATUS=125
readonly SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS=2

release_gate_boundary() {
  local label="${1:-unnamed-gate}"
  local marker_path="${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}"
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:-python3}"
  local boundary_exit_code

  if [[ -z "$marker_path" ]]; then
    return 0
  fi

  if "$policy_python" -I -S - "$marker_path" <<'PY'
import errno
import os
import stat
import sys

path = sys.argv[1]
expected = b'{"reason":"operator-request","schema_version":1}\n'

if not os.path.isabs(path):
    print("cancellation marker path is not absolute", file=sys.stderr)
    raise SystemExit(2)

parent = os.path.dirname(path)
try:
    parent_stat = os.stat(parent, follow_symlinks=False)
except OSError as error:
    print(f"cancellation marker parent is unavailable: {error}", file=sys.stderr)
    raise SystemExit(2) from error
if (
    not stat.S_ISDIR(parent_stat.st_mode)
    or parent_stat.st_uid != os.getuid()
    or parent_stat.st_mode & 0o077
    or os.path.realpath(parent) != parent
):
    print(
        "cancellation marker parent is not one canonical private owner directory",
        file=sys.stderr,
    )
    raise SystemExit(2)

try:
    before = os.lstat(path)
except FileNotFoundError:
    raise SystemExit(0)
except OSError as error:
    print(f"cancellation marker cannot be inspected: {error}", file=sys.stderr)
    raise SystemExit(2) from error

if (
    not stat.S_ISREG(before.st_mode)
    or stat.S_ISLNK(before.st_mode)
    or before.st_nlink != 1
    or before.st_uid != os.getuid()
    or before.st_mode & 0o077
    or os.path.realpath(path) != path
):
    print("cancellation marker is not canonical, private, and owner-bound", file=sys.stderr)
    raise SystemExit(2)

flags = os.O_RDONLY
if hasattr(os, "O_CLOEXEC"):
    flags |= os.O_CLOEXEC
if hasattr(os, "O_NOFOLLOW"):
    flags |= os.O_NOFOLLOW
try:
    descriptor = os.open(path, flags)
except OSError as error:
    if error.errno == errno.ENOENT:
        raise SystemExit(0)
    print(f"cancellation marker cannot be opened safely: {error}", file=sys.stderr)
    raise SystemExit(2) from error
try:
    opened = os.fstat(descriptor)
    data = os.read(descriptor, len(expected) + 1)
    trailing = os.read(descriptor, 1)
    after = os.fstat(descriptor)
finally:
    os.close(descriptor)

identity = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
opened_identity = (opened.st_dev, opened.st_ino, opened.st_size, opened.st_mtime_ns)
after_identity = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
if identity != opened_identity or opened_identity != after_identity:
    print("cancellation marker changed while it was inspected", file=sys.stderr)
    raise SystemExit(2)
if data != expected or trailing:
    print("cancellation marker is not the exact canonical request", file=sys.stderr)
    raise SystemExit(2)
raise SystemExit(125)
PY
  then
    return 0
  else
    boundary_exit_code=$?
  fi

  if ((boundary_exit_code == SUMERAGI_V2_RELEASE_CANCELLED_STATUS)); then
    printf 'cooperative cancellation requested at gate boundary %s\n' "$label" >&2
    return "$SUMERAGI_V2_RELEASE_CANCELLED_STATUS"
  fi
  printf 'invalid cooperative cancellation marker at gate boundary %s\n' "$label" >&2
  return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
}

wait_for_external_cargo() {
  local label="${1:-cargo-gate}"
  local process_snapshot
  local active_compilers

  while true; do
    release_gate_boundary "${label}:before-process-snapshot" || return $?
    if ! process_snapshot="$(ps -axo pid,etime,command)"; then
      echo "failed to capture the required Cargo/rustc/rustfmt process snapshot" >&2
      return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
    fi
    printf '%s\n' "$process_snapshot" >&2
    if ! active_compilers="$(
      printf '%s\n' "$process_snapshot" | awk '
        NR == 1 { next }
        {
          executable = $3
          sub(/^.*\//, "", executable)
          if (executable == "cargo" || executable == "rustc" || executable == "rustfmt") {
            print
          }
        }
      '
    )"; then
      echo "failed to classify the captured Cargo/rustc/rustfmt process snapshot" >&2
      return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
    fi
    if [[ -z "$active_compilers" ]]; then
      release_gate_boundary "${label}:after-process-quiescence" || return $?
      return 0
    fi
    printf '%s\n' \
      "waiting naturally for active Cargo/rustc/rustfmt processes before ${label}:" \
      "$active_compilers" >&2
    sleep 10
  done
}

require_external_private_directory() {
  local source_root="${1:-}"
  local external_root="${2:-}"
  local purpose="${3:-release output}"
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:-python3}"

  if [[ -z "$source_root" || -z "$external_root" ]]; then
    printf 'source root and %s directory are required\n' "$purpose" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  if ! "$policy_python" -I -S - "$source_root" "$external_root" "$purpose" <<'PY'
import os
import stat
import sys

source_root, external_root, purpose = sys.argv[1:]
if not os.path.isabs(source_root) or not os.path.isabs(external_root):
    print(f"source and {purpose} roots must be absolute", file=sys.stderr)
    raise SystemExit(2)
source = os.path.realpath(source_root)
external = os.path.realpath(external_root)
private_tmp = os.path.realpath("/private/tmp")
try:
    external_lstat = os.lstat(external_root)
except OSError as error:
    print(f"{purpose} root is unavailable: {error}", file=sys.stderr)
    raise SystemExit(2) from error
if (
    external != external_root
    or stat.S_ISLNK(external_lstat.st_mode)
    or not stat.S_ISDIR(external_lstat.st_mode)
    or external_lstat.st_uid != os.getuid()
    or external_lstat.st_mode & 0o077
):
    print(f"{purpose} root is not one canonical private owner directory", file=sys.stderr)
    raise SystemExit(2)
try:
    external_under_private_tmp = (
        os.path.commonpath((external, private_tmp)) == private_tmp
    )
    external_under_source = os.path.commonpath((external, source)) == source
except ValueError:
    external_under_private_tmp = False
    external_under_source = True
if (
    external == private_tmp
    or not external_under_private_tmp
    or external_under_source
):
    print(
        f"{purpose} root must be a dedicated /private/tmp directory outside source",
        file=sys.stderr,
    )
    raise SystemExit(2)
PY
  then
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
}

require_external_cargo_target_dir() {
  require_external_private_directory \
    "${1:-}" "${CARGO_TARGET_DIR:-}" "Cargo target"
}

require_external_release_artifact_root() {
  require_external_private_directory \
    "${1:-}" "${IROHA_RELEASE_ARTIFACT_ROOT:-}" "release artifact"
}

require_disjoint_release_roots() {
  local source_root="${1:-}"
  local cargo_root="${CARGO_TARGET_DIR:-}"
  local artifact_root="${IROHA_RELEASE_ARTIFACT_ROOT:-}"
  local cancel_path="${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}"
  local cancel_parent
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:-python3}"

  if [[ -z "$source_root" || -z "$cargo_root" || -z "$artifact_root" \
    || -z "$cancel_path" ]]; then
    echo "source, Cargo target, release artifact, and cancellation paths are required" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  cancel_parent="${cancel_path%/*}"
  [[ -n "$cancel_parent" ]] || cancel_parent=/
  require_external_private_directory \
    "$source_root" "$cancel_parent" "release cancellation parent" || return $?
  if ! "$policy_python" -I -S - \
    "$source_root" "$cargo_root" "$artifact_root" "$cancel_path" <<'PY'
import os
import sys

source_root, cargo_root, artifact_root, cancel_path = sys.argv[1:]
if not all(map(os.path.isabs, (source_root, cargo_root, artifact_root, cancel_path))):
    print("release source/output/cancellation paths must be absolute", file=sys.stderr)
    raise SystemExit(2)
if os.path.abspath(cancel_path) != cancel_path or os.path.realpath(cancel_path) != cancel_path:
    print("release cancellation marker path must be normalized and canonical", file=sys.stderr)
    raise SystemExit(2)
source_root, cargo_root, artifact_root = map(
    os.path.realpath, (source_root, cargo_root, artifact_root)
)


def overlap(left, right):
    try:
        common = os.path.commonpath((left, right))
    except ValueError:
        return False
    return common == left or common == right


try:
    target_artifact_overlap = overlap(cargo_root, artifact_root)
    cancel_source_overlap = overlap(cancel_path, source_root)
    cancel_target_overlap = overlap(cancel_path, cargo_root)
    cancel_artifact_overlap = overlap(cancel_path, artifact_root)
except OSError as error:
    print(f"release root disjointness could not be checked: {error}", file=sys.stderr)
    raise SystemExit(2) from error
if target_artifact_overlap:
    print(
        "Cargo target and release artifact roots must be disjoint",
        file=sys.stderr,
    )
    raise SystemExit(2)
if cancel_source_overlap or cancel_target_overlap or cancel_artifact_overlap:
    print(
        "release cancellation marker must be outside source, Cargo target, and "
        "release artifact roots",
        file=sys.stderr,
    )
    raise SystemExit(2)
PY
  then
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
}

require_release_artifact_path() {
  local candidate="${1:-}"
  local artifact_root="${IROHA_RELEASE_ARTIFACT_ROOT:-}"
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:-python3}"

  if [[ -z "$candidate" || -z "$artifact_root" ]]; then
    echo "release artifact path and root are required" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  if ! "$policy_python" -I -S - "$artifact_root" "$candidate" <<'PY'
import os
import stat
import sys

artifact_root, candidate = sys.argv[1:]
if not os.path.isabs(artifact_root) or not os.path.isabs(candidate):
    print("release artifact paths must be absolute", file=sys.stderr)
    raise SystemExit(2)
if os.path.abspath(candidate) != candidate:
    print("release artifact path must be normalized", file=sys.stderr)
    raise SystemExit(2)
root = os.path.realpath(artifact_root)
try:
    contained = os.path.commonpath((candidate, root)) == root
except ValueError:
    contained = False
if not contained:
    print("release artifact path escapes its authenticated root", file=sys.stderr)
    raise SystemExit(2)

current = root
relative = os.path.relpath(candidate, root)
for component in () if relative == "." else relative.split(os.sep):
    if component in {"", ".", ".."}:
        print("release artifact path contains an unsafe component", file=sys.stderr)
        raise SystemExit(2)
    current = os.path.join(current, component)
    try:
        observed = os.lstat(current)
    except FileNotFoundError:
        break
    except OSError as error:
        print(f"release artifact path is unavailable: {error}", file=sys.stderr)
        raise SystemExit(2) from error
    if stat.S_ISLNK(observed.st_mode) or not stat.S_ISDIR(observed.st_mode):
        print("release artifact path crosses a non-directory or symlink", file=sys.stderr)
        raise SystemExit(2)
    if observed.st_uid != os.getuid():
        print("release artifact path is not owner-bound", file=sys.stderr)
        raise SystemExit(2)
PY
  then
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
}

require_release_artifact_directory() {
  local directory="${1:-}"
  local artifact_root="${IROHA_RELEASE_ARTIFACT_ROOT:-}"
  local policy_python="${IROHA_RELEASE_POLICY_PYTHON:-python3}"

  if [[ -z "$directory" || -z "$artifact_root" ]]; then
    echo "release artifact directory and root are required" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  if ! "$policy_python" -I -S - "$artifact_root" "$directory" <<'PY'
import os
import stat
import sys

artifact_root, directory = sys.argv[1:]
if not os.path.isabs(artifact_root) or not os.path.isabs(directory):
    print("release artifact paths must be absolute", file=sys.stderr)
    raise SystemExit(2)
root = os.path.realpath(artifact_root)
resolved = os.path.realpath(directory)
try:
    observed = os.lstat(directory)
except OSError as error:
    print(f"release artifact directory is unavailable: {error}", file=sys.stderr)
    raise SystemExit(2) from error
if (
    resolved != directory
    or stat.S_ISLNK(observed.st_mode)
    or not stat.S_ISDIR(observed.st_mode)
    or observed.st_uid != os.getuid()
):
    print("release artifact directory is not canonical and owner-bound", file=sys.stderr)
    raise SystemExit(2)
try:
    contained = os.path.commonpath((resolved, root)) == root
except ValueError:
    contained = False
if not contained:
    print("release artifact directory escapes its authenticated root", file=sys.stderr)
    raise SystemExit(2)
PY
  then
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
}

run_cargo() {
  local subcommand="${1:-}"
  local label="cargo-${subcommand:-missing-subcommand}"
  local argument
  local locked_count=0
  local offline_count=0
  local cargo_exit_code
  local -a pinned_arguments

  if [[ -z "$subcommand" ]]; then
    echo "run_cargo requires one Cargo subcommand" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi
  if [[ "${RUSTUP_AUTO_INSTALL+x}" == x \
    && "${RUSTUP_AUTO_INSTALL:-}" != 0 ]]; then
    echo "run_cargo forbids caller-owned rustup auto-install policy" >&2
    return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
  fi

  case "$subcommand" in
    --version|version)
      if (($# != 1)); then
        echo "the pinned Cargo version probe accepts no additional arguments" >&2
        return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
      fi
      ;;
    fmt)
      ;;
    build|test|run|clippy|verus)
      for argument in "$@"; do
        case "$argument" in
          --locked)
            ((locked_count += 1))
            ;;
          --offline)
            ((offline_count += 1))
            ;;
          -j*|--jobs|--jobs=*)
            echo "run_cargo owns the one global -j1 flag; caller job flags are forbidden" >&2
            return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
            ;;
        esac
      done
      if ((locked_count != 1 || offline_count != 1)); then
        echo "run_cargo requires exactly one --locked and one --offline flag" >&2
        return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
      fi
      ;;
    *)
      printf 'run_cargo rejects unsupported Cargo subcommand: %s\n' "$subcommand" >&2
      return "$SUMERAGI_V2_RELEASE_POLICY_ERROR_STATUS"
      ;;
  esac

  release_gate_boundary "${label}:before-wait" || return $?
  wait_for_external_cargo "$label" || return $?
  release_gate_boundary "${label}:before-exec" || return $?

  if [[ "$subcommand" == "--version" || "$subcommand" == "version" \
    || "$subcommand" == "fmt" ]]; then
    pinned_arguments=("$@")
  else
    pinned_arguments=("$subcommand" -j1)
    shift
    pinned_arguments+=("$@")
  fi
  local RUSTUP_AUTO_INSTALL=0
  export RUSTUP_AUTO_INSTALL
  if command cargo +1.93.1 "${pinned_arguments[@]}"; then
    cargo_exit_code=0
  else
    cargo_exit_code=$?
  fi

  release_gate_boundary "${label}:after-natural-completion" || return $?
  return "$cargo_exit_code"
}
