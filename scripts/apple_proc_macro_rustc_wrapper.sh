#!/usr/bin/env bash
# Source-sealed Apple build helper: keep host proc-macro dylibs loadable by dyld.
set -euo pipefail

if [[ "$#" -lt 1 || "$1" != "${RUSTC:-}" || ! -f "$1" || ! -x "$1" || -L "$1" ]]; then
  echo "apple proc-macro wrapper requires the authenticated Rust compiler" >&2
  exit 1
fi
compiler="$1"
shift

previous=""
proc_macro=0
for argument in "$@"; do
  if [[ ( "$previous" == "--crate-type" && "$argument" == "proc-macro" ) \
        || "$argument" == "--crate-type=proc-macro" ]]; then
    proc_macro=1
  fi
  previous="$argument"
done

if [[ "$proc_macro" == 1 ]]; then
  # rustc's -C strip=debuginfo currently leaves a misaligned LINKEDIT string
  # pool on macOS 27 (rust-lang/rust#157750). Host macros are intermediate
  # tools; preserve their symbols while keeping target artifact flags intact.
  exec "$compiler" "$@" -C strip=none
fi
exec "$compiler" "$@"
