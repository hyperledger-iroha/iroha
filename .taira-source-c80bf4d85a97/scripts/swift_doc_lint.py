#!/usr/bin/env python3
"""Swift SDK documentation lint helper.

Roadmap IOS5 requires automated checks that ensure the Swift docs carry the
metadata expected by localization tooling and docs reviewers. This script
verifies that every English file in docs/source/sdk/swift/ ships YAML front
matter with title/summary fields and that translation stubs keep the required
lang/direction/source/status metadata so CI can block regressions.
"""

from __future__ import annotations

import argparse
import sys
from datetime import date
from pathlib import Path
from typing import Dict, Iterable, List, Tuple

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DOC_ROOT = REPO_ROOT / "docs" / "source" / "sdk" / "swift"
TRANSLATION_SUFFIXES = (".he.md", ".ja.md")
REQUIRED_TRANSLATION_KEYS = ("lang", "direction", "source", "status")
ALLOWED_TRANSLATION_STATUSES = {
    "needs-translation",
    "draft",
    "in-review",
    "complete",
}
REVIEW_DATE_STATUSES = {"in-review", "complete"}
VALID_DIRECTIONS = {"ltr", "rtl"}


def parse_args(argv: List[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Lint Swift SDK documentation metadata.")
    parser.add_argument(
        "--doc-root",
        type=Path,
        default=DEFAULT_DOC_ROOT,
        help="Path to docs/source/sdk/swift (defaults to repository copy).",
    )
    return parser.parse_args(argv)


def extract_front_matter(text: str) -> Dict[str, str] | None:
    """Return a dict of YAML-style key/value pairs from the first front matter block."""

    lines = text.splitlines()
    idx = 0
    total = len(lines)

    while idx < total:
        stripped = lines[idx].strip()
        if not stripped:
            idx += 1
            continue
        if stripped.startswith("<!--"):
            while idx < total and "-->" not in lines[idx]:
                idx += 1
            idx += 1
            continue
        if stripped == "---":
            idx += 1
            block: List[str] = []
            while idx < total:
                current = lines[idx]
                if current.strip() == "---":
                    return _parse_yaml_block(block)
                block.append(current)
                idx += 1
            return None
        # First non-comment/non-front-matter content means there is no metadata block.
        return None

    return None


def _parse_yaml_block(lines: Iterable[str]) -> Dict[str, str]:
    metadata: Dict[str, str] = {}
    last_key: str | None = None
    for raw in lines:
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if raw.startswith((" ", "\t")) and last_key:
            metadata[last_key] = (metadata[last_key] + "\n" + stripped).strip()
            continue
        if ":" not in raw:
            continue
        key, value = raw.split(":", 1)
        last_key = key.strip()
        metadata[last_key] = value.strip()
    return metadata


def lint_docs(doc_paths: Iterable[Path], doc_root: Path) -> List[str]:
    errors: List[str] = []
    doc_root = doc_root.resolve()
    paths = sorted(Path(path).resolve() for path in doc_paths if Path(path).is_file())

    for path in paths:
        rel = _safe_relative(path)
        metadata = extract_front_matter(path.read_text(encoding="utf-8"))
        if _is_translation_doc(path):
            errors.extend(_lint_translation_doc(rel, metadata, doc_root))
            continue
        errors.extend(_lint_english_doc(rel, metadata))

    return errors


def _lint_english_doc(rel_path: str, metadata: Dict[str, str] | None) -> List[str]:
    if metadata is None:
        return [f"{rel_path}: missing YAML front matter with title/summary."]
    missing = [key for key in ("title", "summary") if not metadata.get(key, "").strip()]
    if missing:
        return [f"{rel_path}: missing required metadata keys: {', '.join(missing)}."]
    return []


def _lint_translation_doc(
    rel_path: str, metadata: Dict[str, str] | None, doc_root: Path
) -> List[str]:
    if metadata is None:
        return [f"{rel_path}: missing translation metadata block."]
    errors: List[str] = []
    missing = [key for key in REQUIRED_TRANSLATION_KEYS if not metadata.get(key, "").strip()]
    if missing:
        errors.append(f"{rel_path}: missing translation metadata keys: {', '.join(missing)}.")

    direction = metadata.get("direction", "").strip().lower()
    if direction and direction not in VALID_DIRECTIONS:
        errors.append(f"{rel_path}: direction must be one of {sorted(VALID_DIRECTIONS)}.")

    source = metadata.get("source", "").strip()
    if source:
        source_path = _resolve_source_path(source, doc_root)
        if not source_path.exists():
            errors.append(f"{rel_path}: declared source '{source}' does not exist.")

    status = metadata.get("status", "").strip().lower()
    if status and status not in ALLOWED_TRANSLATION_STATUSES:
        errors.append(
            f"{rel_path}: status '{status}' must be one of {sorted(ALLOWED_TRANSLATION_STATUSES)}."
        )
    if status in REVIEW_DATE_STATUSES:
        reviewed = metadata.get("translation_last_reviewed", "").strip()
        if not reviewed:
            errors.append(
                f"{rel_path}: translation_last_reviewed required when status is '{status}'."
            )
        else:
            try:
                date.fromisoformat(reviewed)
            except ValueError:
                errors.append(
                    f"{rel_path}: translation_last_reviewed '{reviewed}' must be YYYY-MM-DD."
                )

    return errors


def _resolve_source_path(source_value: str, doc_root: Path) -> Path:
    candidate = Path(source_value)
    if candidate.is_absolute():
        return candidate
    bases = [REPO_ROOT, doc_root]
    for parent in doc_root.parents:
        bases.append(parent)
    for base in bases:
        resolved = base / candidate
        if resolved.exists():
            return resolved
    return REPO_ROOT / candidate


def _is_translation_doc(path: Path) -> bool:
    return any(path.name.endswith(suffix) for suffix in TRANSLATION_SUFFIXES)


def _safe_relative(path: Path) -> str:
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def main(argv: List[str] | None = None) -> int:
    args = parse_args(argv)
    doc_root = args.doc_root
    if not doc_root.exists():
        print(f"Swift doc root '{doc_root}' does not exist.", file=sys.stderr)
        return 1
    doc_paths = list(doc_root.glob("*.md"))
    errors = lint_docs(doc_paths, doc_root)
    if errors:
        print("Swift doc lint failed:", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        return 1
    print("Swift doc lint passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
