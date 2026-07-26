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

export DOC_PATH="docs/source/telemetry.md"
export SRC_PATH="crates/iroha_logger/src/telemetry.rs"
export DEFAULTS_PATH="crates/iroha_config/src/parameters/defaults.rs"
export LOGGER_TOML="crates/iroha_logger/Cargo.toml"

python3 - <<'PY'
import os
import re
import sys
from pathlib import Path

doc_path = Path(os.environ["DOC_PATH"])
src_path = Path(os.environ["SRC_PATH"])
defaults_path = Path(os.environ["DEFAULTS_PATH"])
logger_toml_path = Path(os.environ["LOGGER_TOML"])

errors = []


def read(path: Path) -> str:
    try:
        return path.read_text()
    except Exception as exc:
        errors.append(f"telemetry-redaction guard: failed to read {path}: {exc}")
        return ""


src_text = read(src_path)
doc_text = read(doc_path)
defaults_text = read(defaults_path)
logger_toml = read(logger_toml_path)


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
            f"telemetry-redaction guard: {label} markers missing in docs/source/telemetry.md"
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


allowlist_src = parse_list_src(src_text, "REDACTION_ALLOWLIST_POLICY")
allowlist_doc = parse_doc_block(
    doc_text,
    "<!-- TELEMETRY_REDACTION_ALLOWLIST_START -->",
    "<!-- TELEMETRY_REDACTION_ALLOWLIST_END -->",
    "allowlist",
)

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

if allowlist_doc != sorted(allowlist_doc):
    errors.append(
        "telemetry-redaction guard: allowlist in docs/source/telemetry.md must be sorted."
    )

if allowlist_src != allowlist_doc:
    errors.append(
        "telemetry-redaction guard: allowlist mismatch between telemetry.rs and docs/source/telemetry.md."
    )
    errors.append(f"  src: {allowlist_src!r}")
    errors.append(f"  doc: {allowlist_doc!r}")

if prefixes_src != prefixes_doc:
    errors.append(
        "telemetry-redaction guard: prefix taxonomy mismatch between telemetry.rs and docs/source/telemetry.md."
    )
    errors.append(f"  src: {prefixes_src!r}")
    errors.append(f"  doc: {prefixes_doc!r}")

if keywords_src != keywords_doc:
    errors.append(
        "telemetry-redaction guard: keyword taxonomy mismatch between telemetry.rs and docs/source/telemetry.md."
    )
    errors.append(f"  src: {keywords_src!r}")
    errors.append(f"  doc: {keywords_doc!r}")

mode_match = re.search(r'pub const MODE: &str = "([^"]+)";', defaults_text)
if not mode_match:
    errors.append(
        "telemetry-redaction guard: telemetry.redaction.MODE not found in defaults.rs."
    )
else:
    mode = mode_match.group(1)
    if mode != "strict":
        errors.append(
            f"telemetry-redaction guard: telemetry.redaction.MODE must be \"strict\" (found {mode!r})."
        )


def parse_default_features(text: str) -> list[str]:
    try:
        import tomllib  # type: ignore

        data = tomllib.loads(text)
        return list(data.get("features", {}).get("default", []))
    except Exception:
        pass

    match = re.search(r"^\[features\](.*?)^\[", text, re.S | re.M)
    block = match.group(1) if match else text
    match = re.search(r"default\s*=\s*\[(.*?)\]", block, re.S)
    if not match:
        return []
    entries = match.group(1)
    return [item.strip().strip('"') for item in entries.split(",") if item.strip()]


default_features = parse_default_features(logger_toml)
if "log-obfuscation" not in default_features:
    errors.append(
        "telemetry-redaction guard: iroha_logger default features must include log-obfuscation."
    )

if errors:
    for error in errors:
        print(error, file=sys.stderr)
    sys.exit(1)

print("telemetry-redaction guard: OK.")
PY
