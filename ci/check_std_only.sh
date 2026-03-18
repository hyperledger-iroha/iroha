#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd -- "${ROOT}"

resolve_base() {
	if [[ -n "${AGENTS_BASE_REF:-}" ]]; then
		git rev-parse --verify "${AGENTS_BASE_REF}" 2>/dev/null && return
		echo "warning: AGENTS_BASE_REF=${AGENTS_BASE_REF} not found; falling back to defaults." >&2
	 fi

	for candidate in origin/main origin/master main master; do
		if git rev-parse --verify "${candidate}" >/dev/null 2>&1; then
			echo "${candidate}"
			return
		fi
	done

	echo "HEAD^"
}

base_ref="$(resolve_base)"
base_commit="$(git merge-base "${base_ref}" HEAD 2>/dev/null || git rev-parse "${base_ref}")"

pattern='no_std|wasm32'

targets=(
	crates
	data_model
	integration_tests
	IrohaSwift
	xtask
	Cargo.toml
	Makefile
)

hits=()
current_file=""

while IFS= read -r line; do
	case "${line}" in
		"+++ "*)
			current_file="${line#+++ b/}"
			continue
			;;
		"+"*)
			[[ -z "${current_file}" ]] && continue
			content="${line#+}"
			if echo "${content}" | grep -Eq "${pattern}"; then
				hits+=("${current_file}:${content}")
			fi
			;;
		*)
			continue
			;;
	esac
done < <(git diff -U0 "${base_commit}"...HEAD -- "${targets[@]}")

if ((${#hits[@]})); then
	echo "error: std-only guard detected new no_std/wasm32 usage relative to ${base_ref}:" >&2
	printf '%s\n' "${hits[@]}" >&2
	cat >&2 <<'HINT'
This workspace targets the Rust standard library only. Remove new no_std/wasm32 cfgs/targets
or seek approval to reintroduce alternative build modes.
Set STD_ONLY_GUARD_ALLOW=1 to acknowledge intentional usage (CI experiments only).
HINT
	if [[ "${STD_ONLY_GUARD_ALLOW:-0}" != 1 ]]; then
		exit 1
	fi
fi

echo "Std-only guard passed: no new no_std/wasm32 cfgs added relative to ${base_ref}."
