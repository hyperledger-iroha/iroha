#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd -- "${ROOT}"

# Proc-macro/UI test guard. Runs trybuild suites for the derive/proc-macro crates.
# Override the crate list via PROC_MACRO_UI_CRATES="crate1 crate2" and pass extra
# cargo flags through PROC_MACRO_UI_CARGO_FLAGS (e.g., "--release").
default_crates=(
	iroha_primitives_derive
	iroha_primitives
	iroha_data_model_derive
	iroha_data_model
	iroha_derive
	iroha_schema_derive
	iroha_version_derive
	iroha_telemetry_derive
	iroha_executor_data_model_derive
	iroha_executor_derive
	iroha_trigger_derive
	iroha_smart_contract_derive
	iroha_ffi_proc_macro
	norito_derive
)

if [[ -n "${PROC_MACRO_UI_CRATES:-}" ]]; then
	# shellcheck disable=SC2206
	crates=(${PROC_MACRO_UI_CRATES})
else
	crates=("${default_crates[@]}")
fi

if [[ "${#crates[@]}" -eq 0 ]]; then
	echo "error: no proc-macro crates selected (set PROC_MACRO_UI_CRATES to override)" >&2
	exit 1
fi

extra_flags=()
if [[ -n "${PROC_MACRO_UI_CARGO_FLAGS:-}" ]]; then
	# shellcheck disable=SC2206
	extra_flags=(${PROC_MACRO_UI_CARGO_FLAGS})
fi
flags_display="${extra_flags[*]:-}"

echo "Running proc-macro trybuild UI suites for: ${crates[*]}"
for crate in "${crates[@]}"; do
	echo "==> cargo test -p ${crate} --features trybuild-tests --locked ${flags_display} -- --nocapture"
	cmd=(cargo test -p "${crate}" --features trybuild-tests --locked)
	if [[ ${#extra_flags[@]} -gt 0 ]]; then
		cmd+=("${extra_flags[@]}")
	fi
	cmd+=(-- --nocapture)
	"${cmd[@]}"
done
