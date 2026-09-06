#!/bin/sh
# Compatibility entry point for the fail-closed TON SCCP builder.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
python_bin=${SCCP_TON_BUILDER_PYTHON:-python3}

# Acton is never resolved here. Development callers must pass an absolute
# executable after `development-local --acton`; production callers use the
# digest-pinned, network-disabled container path in ton_sccp_builder.py.
exec "$python_bin" "$script_dir/ton_sccp_builder.py" "$@"
