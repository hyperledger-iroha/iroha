#!/bin/sh
# Deterministic local/CI gate for the immutable TON SCCP contracts.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
project_dir="$repo_root/contracts/ton/sccp"
acton_bin=${ACTON_BIN:-acton}

version=$($acton_bin --version)
case "$version" in
    "acton 1.1.0 "*) ;;
    *)
        echo "expected official Acton 1.1.0, got: $version" >&2
        exit 1
        ;;
esac

doctor=$($acton_bin doctor --project-root "$project_dir")
case "$doctor" in
    *"tolk.version:     1.4.1"*) ;;
    *)
        echo "Acton 1.1.0 does not expose embedded Tolk 1.4.1" >&2
        exit 1
        ;;
esac

cd "$project_dir"
$acton_bin fmt --check
$acton_bin check
$acton_bin build
$acton_bin test
