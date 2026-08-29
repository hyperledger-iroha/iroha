#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd -- "${ROOT}"

if [[ "${TELEMETRY_REDACTION_GUARD_ALLOW:-0}" == 1 ]]; then
	echo "telemetry-redaction guard: TELEMETRY_REDACTION_GUARD_ALLOW=1 set; skipping checks." >&2
	exit 0
fi

if ! command -v python3 >/dev/null 2>&1; then
	echo "error: python3 is required for telemetry redaction guard checks." >&2
	exit 1
fi

export DOC_PATH="specs/telemetry.md"
export SRC_PATH="crates/iroha_logger/src/telemetry.rs"
export CONFIG_PATH="crates/iroha_config/src/parameters/user.rs"

python3 - <<'PY'
import os
import re
import sys
from pathlib import Path

doc_path = Path(os.environ["DOC_PATH"])
src_path = Path(os.environ["SRC_PATH"])
config_path = Path(os.environ["CONFIG_PATH"])

errors = []


def read(path: Path) -> str:
    try:
        return path.read_text()
    except Exception as exc:
        errors.append(f"telemetry-redaction guard: failed to read {path}: {exc}")
        return ""


src_text = read(src_path)
doc_text = read(doc_path)
config_text = read(config_path)


def parse_list_src(text: str, name: str) -> list[str]:
    pattern = rf"{name}\s*:\s*[^=]+=\s*&\[(.*?)\];"
    match = re.search(pattern, text, re.S)
    if not match:
        errors.append(f"telemetry-redaction guard: {name} not found in telemetry.rs")
        return []
    body = match.group(1)
    body = re.sub(r"/\\*.*?\\*/", "", body, flags=re.S)
    body = re.sub(r"//.*", "", body)
    return re.findall(r"\"([^\"]+)\"", body)


def parse_doc_block(text: str, start: str, end: str, label: str) -> list[str]:
    if start not in text or end not in text:
        errors.append(
            f"telemetry-redaction guard: {label} markers missing in specs/telemetry.md"
        )
        return []
    block = text.split(start, 1)[1].split(end, 1)[0]
    items: list[str] = []
    for raw in block.splitlines():
        line = raw.strip()
        if not line or line.startswith("```") or line.startswith("<!--"):
            continue
        if line.startswith(("-", "*")):
            line = line.lstrip("-*").strip()
        if line.startswith("#"):
            continue
        items.append(line)
    if items == ["(none)"] or items == ["none"]:
        return []
    return items


prefixes_src = parse_list_src(src_text, "EXPLICIT_REDACTION_PREFIXES")
prefixes_doc = parse_doc_block(
    doc_text,
    "<!-- TELEMETRY_REDACTION_PREFIXES_START -->",
    "<!-- TELEMETRY_REDACTION_PREFIXES_END -->",
    "prefix taxonomy",
)

keywords_src = parse_list_src(src_text, "SENSITIVE_FIELD_KEYWORDS")
keywords_doc = parse_doc_block(
    doc_text,
    "<!-- TELEMETRY_REDACTION_KEYWORDS_START -->",
    "<!-- TELEMETRY_REDACTION_KEYWORDS_END -->",
    "keyword taxonomy",
)

if prefixes_src != prefixes_doc:
    errors.append(
        "telemetry-redaction guard: prefix taxonomy mismatch between telemetry.rs and specs/telemetry.md."
    )
    errors.append(f"  src: {prefixes_src!r}")
    errors.append(f"  doc: {prefixes_doc!r}")

if keywords_src != keywords_doc:
    errors.append(
        "telemetry-redaction guard: keyword taxonomy mismatch between telemetry.rs and specs/telemetry.md."
    )
    errors.append(f"  src: {keywords_src!r}")
    errors.append(f"  doc: {keywords_doc!r}")

for removed_surface in [
    "set_redaction_policy",
    "set_redaction_audit_hook",
    "REDACTION_ALLOWLIST_POLICY",
    "REDACTION_SUPPORTED",
]:
    if removed_surface in src_text:
        errors.append(
            f"telemetry-redaction guard: removed policy surface {removed_surface} reappeared."
        )

if "telemetry_redaction" in config_text:
    errors.append(
        "telemetry-redaction guard: configurable redaction is forbidden; redaction must remain unconditional."
    )

if errors:
    for error in errors:
        print(error, file=sys.stderr)
    sys.exit(1)

print("telemetry-redaction guard: OK.")
PY
