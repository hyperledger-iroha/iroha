#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd -- "${ROOT}"

if ! command -v rg >/dev/null 2>&1; then
	echo "error: ripgrep (rg) is required for missing-docs guardrail enforcement." >&2
	exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
	echo "error: python3 is required for missing-docs guardrail enforcement." >&2
	exit 1
fi

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
base_commit="$(git merge-base "${base_ref}" HEAD 2>/dev/null || git rev-parse "${base_ref}" || true)"
if [[ -z "${base_commit}" ]]; then
	base_commit="HEAD"
fi

diff_output="$(git diff --unified=0 "${base_commit}"...HEAD -- '*.[rs]' | rg '^\\+' || true)"
violations="$(printf '%s\n' "${diff_output}" | rg 'allow\\(missing_docs\\)' || true)"

if [[ -n "${violations}" ]]; then
	echo "error: new #[allow(missing_docs)] entries detected relative to ${base_ref}:" >&2
	printf '%s\n' "${violations}" >&2
	cat >&2 <<'HINT'
Missing documentation is denied in the workspace. Add docs instead of introducing new
`#[allow(missing_docs)]` entries, or seek approval with an explicit follow-up plan.
Set MISSING_DOCS_GUARD_ALLOW=1 to acknowledge intentional additions (CI experiments only).
HINT
	if [[ "${MISSING_DOCS_GUARD_ALLOW:-0}" != 1 ]]; then
		exit 1
	fi
fi

doc_violations="$(python3 - "$base_commit" <<'PY'
import pathlib
import re
import subprocess
import sys
from typing import Iterable, List, Optional, Tuple

ROOT = pathlib.Path(__file__).resolve().parent.parent
BASE = sys.argv[1]

PUB_RE = re.compile(
    r"^pub(?:\([^)]*\))?\s+(?:async\s+)?(struct|enum|fn|trait|type|union|mod)\b"
)


def changed_rust_files() -> List[str]:
    result = subprocess.run(
        ["git", "diff", "--name-only", f"{BASE}...HEAD", "--", "*.rs"],
        capture_output=True,
        text=True,
        check=False,
    )
    return [line for line in result.stdout.splitlines() if line.strip()]


def closest_crate_root(rel_path: pathlib.Path) -> Optional[pathlib.Path]:
    root = ROOT.resolve()
    current = (ROOT / rel_path).parent.resolve()
    while True:
        cargo = current / "Cargo.toml"
        if cargo.exists():
            try:
                content = cargo.read_text(encoding="utf-8")
            except OSError:
                return None
            if "[package]" in content:
                try:
                    return current.relative_to(ROOT)
                except ValueError:
                    return None
        if current == root or current == current.parent:
            return None
        current = current.parent


def changed_crates(files: Iterable[str]) -> List[pathlib.Path]:
    crates = set()
    for rel in files:
        crate = closest_crate_root(pathlib.Path(rel))
        if crate is not None:
            crates.add(crate)
    return sorted(crates)


def crate_has_docs(crate_path: pathlib.Path) -> bool:
    candidates = [
        ROOT / crate_path / "src" / "lib.rs",
        ROOT / crate_path / "src" / "main.rs",
    ]
    seen_root = False
    for root_file in candidates:
        if not root_file.exists():
            continue
        seen_root = True
        try:
            lines = root_file.read_text(encoding="utf-8").splitlines()
        except OSError:
            continue
        if any(
            line.lstrip().startswith("//!") or line.lstrip().startswith("#![doc")
            for line in lines[:80]
        ):
            return True
    return not seen_root


def iter_added_public_items() -> Iterable[Tuple[str, int, str]]:
    diff = subprocess.run(
        ["git", "diff", "--unified=0", f"{BASE}...HEAD", "--", "*.rs"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.splitlines()
    current_file = None
    new_line = 0
    for line in diff:
        if line.startswith("+++ b/"):
            path = line.removeprefix("+++ b/").strip()
            current_file = None if path == "/dev/null" else path
        elif line.startswith("@@ "):
            match = re.search(r"\+(\d+)(?:,(\d+))?", line)
            new_line = int(match.group(1)) if match else 0
        elif line.startswith("+") and not line.startswith("+++"):
            yield current_file, new_line, line[1:]
            new_line += 1
        elif line.startswith(" "):
            new_line += 1


def has_doc_comment(path: pathlib.Path, line_no: int) -> bool:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return True
    idx = line_no - 1
    i = idx - 1
    while i >= 0:
        text = lines[i].strip()
        if not text:
            i -= 1
            continue
        if text.startswith(("///", "//!", "#[doc", "/**")):
            return True
        if text.startswith("#[") or text.startswith("//"):
            i -= 1
            continue
        break
    return False


def should_skip(path_str: str) -> bool:
    parts = pathlib.Path(path_str).parts
    return any(part in {"tests", "benches", "examples"} for part in parts)


def main() -> None:
    files = changed_rust_files()
    violations: List[str] = []

    for crate in changed_crates(files):
        if not crate_has_docs(crate):
            violations.append(
                f"{crate}/src/lib.rs: missing crate-level //! docs (crate touched in diff)"
            )

    for path_str, line_no, line in iter_added_public_items():
        if not path_str or should_skip(path_str):
            continue
        if line.lstrip() != line:
            continue  # indented (likely inside impl)
        match = PUB_RE.match(line.strip())
        if not match:
            continue
        file_path = ROOT / path_str
        if not has_doc_comment(file_path, line_no):
            kind = match.group(1)
            violations.append(
                f"{path_str}:{line_no}: new public {kind} lacks a doc comment"
            )

    if violations:
        print("\n".join(sorted(set(violations))))


if __name__ == "__main__":
    main()
PY
)"

if [[ -n "${doc_violations}" ]]; then
	echo "error: documentation guard found missing crate-level docs or new undocumented public items:" >&2
	printf '%s\n' "${doc_violations}" >&2
	if [[ "${MISSING_DOCS_GUARD_ALLOW:-0}" != 1 ]]; then
		exit 1
	fi
fi

tmp_json="$(mktemp)"
tmp_md="$(mktemp)"
inventory_json="docs/source/agents/missing_docs_inventory.json"
inventory_md="docs/source/agents/missing_docs_inventory.md"

if python3 scripts/inventory_missing_docs.py --json "${tmp_json}" --md "${tmp_md}"; then
	if ! diff -u "${inventory_json}" "${tmp_json}" >/dev/null 2>&1 || \
		! diff -u "${inventory_md}" "${tmp_md}" >/dev/null 2>&1; then
		echo "error: missing-docs inventory is stale. Regenerate with:" >&2
		echo "  python3 scripts/inventory_missing_docs.py --json ${inventory_json} --md ${inventory_md}" >&2
		if [[ "${MISSING_DOCS_GUARD_ALLOW:-0}" != 1 ]]; then
			rm -f "${tmp_json}" "${tmp_md}"
			exit 1
		fi
	fi
else
	echo "error: failed to regenerate missing-docs inventory." >&2
	if [[ "${MISSING_DOCS_GUARD_ALLOW:-0}" != 1 ]]; then
		rm -f "${tmp_json}" "${tmp_md}"
		exit 1
	fi
fi

rm -f "${tmp_json}" "${tmp_md}"

echo "Missing-docs guard passed: no new #[allow(missing_docs)] entries or undocumented public items detected."
