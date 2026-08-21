#!/usr/bin/env bash
# Run and independently verify the sealed Sumeragi V2 replay corridor.
#
# Prerequisites:
#   TLA2TOOLS_JAR     canonical pinned TLA2Tools 1.7.4 jar
#   TLAPM_PROJECTION  sealed directory containing only regular Functions.tla
#                     and Folds.tla files from the pinned TLAPM commit
# Optional environment:
#   JAVA_BIN          Java executable or name accepted by resolve_java.sh
#   PYTHON_BIN        Python 3.9+ executable or command name (default python3)
#   SUMERAGI_V2_REPLAY_TIMEOUT_SECONDS  per-process timeout (default 1800)

set -euo pipefail
umask 077

usage() {
  cat <<'USAGE'
Usage: check_sumeragi_v2_replay_trace.sh [options]

Options:
  --formal-only                 run the only supported V1 replay corridor
  --output-root PATH            persist a create-only diagnostic receipt
  --help                        show this help

Without --output-root the verified diagnostic receipt is temporary. The
wrapper never creates TLAPM compatibility symlinks; TLAPM_PROJECTION must
already be a canonical sealed regular-file projection.
USAGE
}

mode="formal-only"
output_root=""
while (($#)); do
  case "$1" in
    --formal-only)
      shift
      ;;
    --output-root)
      (($# >= 2)) || {
        echo "--output-root requires a path" >&2
        exit 2
      }
      output_root="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

readonly REPO_ROOT="$(cd -P -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly COLLECTOR="${REPO_ROOT}/scripts/formal/collect_sumeragi_v2_replay_receipt.py"
readonly CHECKER="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_replay_receipt.py"
readonly RESULT_CONTRACT="${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:?TLA2TOOLS_JAR must name the authenticated external jar}"
readonly TLAPM_PROJECTION="${TLAPM_PROJECTION:?TLAPM_PROJECTION must name the sealed two-file projection}"
readonly TIMEOUT_SECONDS="${SUMERAGI_V2_REPLAY_TIMEOUT_SECONDS:-1800}"

if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java="$(${REPO_ROOT}/scripts/formal/resolve_java.sh "$JAVA_BIN")"
else
  resolved_java="$(${REPO_ROOT}/scripts/formal/resolve_java.sh)"
fi
readonly RESOLVED_JAVA="$resolved_java"

requested_python="${PYTHON_BIN:-python3}"
python_candidate="$(type -P "$requested_python")" || {
  echo "Python 3.9 or newer is required" >&2
  exit 1
}
resolved_python="$($python_candidate -I -S -c 'import os,sys; print(os.path.realpath(sys.executable))')"
readonly RESOLVED_PYTHON="$resolved_python"
"$RESOLVED_PYTHON" -I -S -c \
  'import sys; raise SystemExit(0 if sys.version_info >= (3, 9) else 1)' || {
  echo "Python 3.9 or newer is required" >&2
  exit 1
}

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-replay-wrapper.XXXXXX")"
run_dir="$(cd -P -- "$run_dir" && pwd)"
chmod 0700 "$run_dir"
run_identity="$($RESOLVED_PYTHON -I -S -c \
  'import os,sys; s=os.lstat(sys.argv[1]); print(s.st_dev, s.st_ino)' "$run_dir")"
read -r run_device run_inode <<<"$run_identity"
readonly run_device run_inode
wrapper_complete=0
cleanup_wrapper() {
  ((wrapper_complete)) || return 0
  wrapper_complete=0
  "$RESOLVED_PYTHON" -B -I -S -c '
import importlib.util
from pathlib import Path
import sys
spec = importlib.util.spec_from_file_location("sumeragi_replay_collector_cleanup", sys.argv[1])
if spec is None or spec.loader is None:
    raise SystemExit("collector cleanup module is unavailable")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
module.remove_owned_directory_path(Path(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4]))
' "$COLLECTOR" "$run_dir" "$run_device" "$run_inode"
}
trap cleanup_wrapper EXIT

persist_receipt=1
if [[ -z "$output_root" ]]; then
  output_root="${run_dir}/receipt"
  persist_receipt=0
else
  output_root="$($RESOLVED_PYTHON -I -S -c \
    'import os,sys; print(os.path.abspath(sys.argv[1]))' "$output_root")"
fi
readonly output_root

collector_stdout="${run_dir}/collector.stdout"
collector_stderr="${run_dir}/collector.stderr"
collector_args=(
  --root "$REPO_ROOT"
  --java-bin "$RESOLVED_JAVA"
  --python-bin "$RESOLVED_PYTHON"
  --tla2tools-jar "$TLA2TOOLS_JAR"
  --tlapm-projection "$TLAPM_PROJECTION"
  --output-root "$output_root"
  --mode "$mode"
  --timeout-seconds "$TIMEOUT_SECONDS"
)

cd "$REPO_ROOT"
if ! "$RESOLVED_PYTHON" -B -I -S "$COLLECTOR" "${collector_args[@]}" \
  >"$collector_stdout" 2>"$collector_stderr"; then
  cat "$collector_stderr" >&2
  echo "Sumeragi V2 replay collection failed" >&2
  exit 1
fi
[[ ! -s "$collector_stderr" ]] || {
  cat "$collector_stderr" >&2
  echo "replay collector emitted stderr" >&2
  exit 1
}
receipt_path="$(sed -n '1p' "$collector_stdout")"
[[ "$receipt_path" == "${output_root}/receipt.json" \
  && "$(wc -l <"$collector_stdout" | tr -d ' ')" == 1 ]] || {
  echo "replay collector returned a malformed receipt path" >&2
  exit 1
}
source "$RESULT_CONTRACT"
sumeragi_v2_tlc_assert_replay_tool_result \
  "replay-decision-witness" \
  "${output_root}/events/02-raw_tlc.stdout" \
  "${output_root}/events/02-raw_tlc.stderr" \
  12

checker_stdout="${run_dir}/checker.stdout"
checker_stderr="${run_dir}/checker.stderr"
checker_args=("$receipt_path")
if ! "$RESOLVED_PYTHON" -B -I -S "$CHECKER" "${checker_args[@]}" \
  >"$checker_stdout" 2>"$checker_stderr"; then
  cat "$checker_stderr" >&2
  echo "Sumeragi V2 replay receipt verification failed" >&2
  exit 1
fi
[[ ! -s "$checker_stderr" ]] || {
  cat "$checker_stderr" >&2
  echo "replay receipt checker emitted stderr" >&2
  exit 1
}
cat "$checker_stdout"
if ((persist_receipt)); then
  echo "receipt: ${receipt_path}"
fi
wrapper_complete=1
if ! cleanup_wrapper; then
  echo "verified wrapper temporary directory could not be cleaned safely: $run_dir" >&2
  exit 1
fi
trap - EXIT
