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

if ! git diff --quiet -- Cargo.lock || ! git diff --quiet "${base_commit}"...HEAD -- Cargo.lock; then
	echo "error: Cargo.lock modifications detected (working tree or relative to ${base_ref})." >&2
	echo "The AGENTS workflow forbids lockfile edits; drop Cargo.lock changes or seek approval." >&2
	fail=true
fi

added_tomls="$(
	{
		git diff --name-status --diff-filter=A "${base_commit}"...HEAD -- '*Cargo.toml' || true
		git diff --name-status --diff-filter=A -- '*Cargo.toml' || true
	} | sort -u
)"

if [[ -n "${added_tomls}" ]]; then
	echo "error: new Cargo.toml files detected (possible new crates needing approval):" >&2
	echo "${added_tomls}" >&2
	fail=true
fi

base_toml="$(git show "${base_commit}:Cargo.toml" 2>/dev/null || true)"
current_toml="$(cat Cargo.toml 2>/dev/null || git show HEAD:Cargo.toml 2>/dev/null || true)"

if [[ -n "${base_toml}" && -n "${current_toml}" ]]; then
	if ! BASE_TOML="${base_toml}" CURRENT_TOML="${current_toml}" python3 - <<'PY'
import os
import sys

base = os.environ.get("BASE_TOML", "")
current = os.environ.get("CURRENT_TOML", "")

if not base or not current:
    sys.exit(0)


def parse_members(text: str) -> set[str]:
    try:
        import tomllib  # type: ignore
    except ModuleNotFoundError:
        tomllib = None

    if tomllib:
        try:
            data = tomllib.loads(text)
            members = data.get("workspace", {}).get("members", [])
            if isinstance(members, list):
                return {str(item) for item in members}
        except Exception:
            pass

    members: set[str] = set()
    in_workspace = False
    collecting = False
    buffer: list[str] = []

    for raw_line in text.splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue

        if line.startswith("[workspace"):
            in_workspace = True
            continue

        if in_workspace and line.startswith("[") and not line.startswith("[workspace."):
            break

        if not in_workspace:
            continue

        if collecting:
            if "]" in line:
                collecting = False
                line = line.split("]", 1)[0]
            buffer.append(line)
            continue

        if line.startswith("members"):
            parts = line.split("=", 1)
            if len(parts) < 2:
                continue
            rhs = parts[1].strip()
            if rhs.startswith("[") and rhs.endswith("]"):
                buffer.append(rhs[1:-1])
            elif rhs.startswith("["):
                collecting = True
                buffer.append(rhs[1:])

    for chunk in buffer:
        for piece in chunk.split(","):
            entry = piece.strip().strip('"').strip("'")
            if entry:
                members.add(entry)

    return members


base_members = parse_members(base)
current_members = parse_members(current)
added_members = sorted(current_members - base_members)

if added_members:
    print("error: new workspace members detected relative to base:", file=sys.stderr)
    for member in added_members:
        print(f" - {member}", file=sys.stderr)
    sys.exit(1)
PY
	then
		fail=true
	fi
else
	echo "warning: unable to load Cargo.toml for workspace member comparison; skipping member check." >&2
fi

added_dependencies="$(
	BASE_COMMIT="${base_commit}" python3 - <<'PY'
import os
import subprocess
import sys
from typing import Dict, Optional, Set

try:
    import tomllib  # type: ignore[attr-defined]
except ModuleNotFoundError:  # pragma: no cover
    try:
        import tomli as tomllib  # type: ignore[no-redef]
    except ModuleNotFoundError:
        print("error: neither tomllib nor tomli is available; install tomli or use Python 3.11+.", file=sys.stderr)
        sys.exit(1)


def collect_dependencies(data: Dict) -> Set[str]:
    deps: Set[str] = set()

    def pull(obj: Dict, key: str) -> None:
        table = obj.get(key, {})
        if isinstance(table, dict):
            deps.update(table.keys())

    for key in ("dependencies", "dev-dependencies", "build-dependencies"):
        pull(data, key)

    workspace = data.get("workspace", {})
    if isinstance(workspace, dict):
        for key in ("dependencies", "dev-dependencies", "build-dependencies"):
            pull(workspace, key)

    target = data.get("target", {})
    if isinstance(target, dict):
        for cfg in target.values():
            if isinstance(cfg, dict):
                for key in ("dependencies", "dev-dependencies", "build-dependencies"):
                    pull(cfg, key)

    return deps


base_commit = os.environ.get("BASE_COMMIT", "")
paths = subprocess.check_output(["git", "ls-files", "*Cargo.toml"], text=True).splitlines()
reports = []


def read_at(ref: str, path: str) -> Optional[str]:
    if not ref:
        return None
    try:
        return subprocess.check_output(["git", "show", f"{ref}:{path}"], text=True)
    except subprocess.CalledProcessError:
        return None


for path in paths:
    try:
        with open(path, "r", encoding="utf-8") as fh:
            head_text = fh.read()
    except FileNotFoundError:
        continue

    base_text = read_at(base_commit, path)
    if base_text is None:
        # new crates are handled by the added_tomls check
        continue

    try:
        head_data = tomllib.loads(head_text)
        base_data = tomllib.loads(base_text)
    except Exception:
        continue

    added = sorted(collect_dependencies(head_data) - collect_dependencies(base_data))
    if added:
        reports.append(path + "\n" + "\n".join(f"  - {dep}" for dep in added))

if reports:
    print("\n\n".join(reports))
PY
)"

if [[ -n "${added_dependencies}" ]]; then
	echo "error: new dependencies detected relative to ${base_ref} (approval required):" >&2
	echo "${added_dependencies}" >&2
	fail=true
fi

if ! bash ci/check_soranet_privacy_guard.sh; then
	fail=true
fi

if [[ "${fail}" == true ]]; then
	cat >&2 <<'HINT'
Guardrail failures detected.
- Remove Cargo.lock edits or regenerate with explicit approval.
- Drop new workspace members or obtain approval for additional crates.
- Remove or seek approval for newly added dependencies in Cargo.toml files.
- Keep SoraNet privacy ingest endpoints behind enforce_soranet_privacy_ingest and SoraFS storage pins behind sorafs_pin_policy, updating docs when authz changes.
Set AGENTS_BASE_REF=<ref> to compare against a specific base (default origin/main).
HINT
	exit 1
fi

echo "AGENTS guardrails passed: no Cargo.lock edits, new workspace crates, or dependency additions detected."
exit 0
