#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd -- "${ROOT}"

resolve_base() {
	if [[ -n "${AGENTS_BASE_REF:-}" ]]; then
		if git rev-parse --verify "${AGENTS_BASE_REF}" >/dev/null 2>&1; then
			echo "${AGENTS_BASE_REF}"
			return
		fi
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

fail=false

# Ensure the open-work section does not contain completed items.
if awk '
	/^## Current Open Work/ {in_open=1; next}
	/^## Archived/ {in_open=0}
	in_open && /🈴/ {exit 1}
' roadmap.md; then
	:
else
	echo "error: roadmap \"Current Open Work\" section contains completed (🈴) items; move them to Archived/status.md." >&2
	fail=true
fi

roadmap_changed=true
if git diff --quiet "${base_commit}"...HEAD -- roadmap.md; then
	roadmap_changed=false
fi

status_changed=true
if git diff --quiet "${base_commit}"...HEAD -- status.md; then
	status_changed=false
fi

allow_unpaired="${STATUS_SYNC_ALLOW_UNPAIRED:-0}"

if [[ "${roadmap_changed}" == true && "${status_changed}" == false ]]; then
	echo "error: roadmap.md changed but status.md did not. Keep plan/status in sync or set AGENTS_BASE_REF to the intended base." >&2
	if [[ "${allow_unpaired}" != 1 ]]; then
		fail=true
	fi
fi

if [[ "${status_changed}" == true && "${roadmap_changed}" == false ]]; then
	echo "error: status.md changed but roadmap.md did not. Update roadmap.md alongside status moves or set AGENTS_BASE_REF to the intended base." >&2
	if [[ "${allow_unpaired}" != 1 ]]; then
		fail=true
	fi
fi

if [[ "${fail}" == true ]]; then
	cat >&2 <<'HINT'
Status/roadmap sync failures detected.
- Move completed items out of the Current Open Work section (roadmap) into Archived with details in status.md.
- When updating roadmap.md in a PR, also update status.md (or set AGENTS_BASE_REF to the desired comparison base).
- When updating status.md, accompany the change with roadmap.md updates (or set STATUS_SYNC_ALLOW_UNPAIRED=1 for rare status-only fixes).
HINT
	exit 1
fi

echo "AGENTS status sync passed: roadmap open work clean and roadmap/status changes are paired."
