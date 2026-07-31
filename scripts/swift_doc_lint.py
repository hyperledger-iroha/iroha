#!/usr/bin/env python3
"""Validate source-coupled Swift SDK documentation front matter."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Dict, Iterable, List

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DOC_ROOT = REPO_ROOT / "specs" / "sdk" / "swift"


def parse_args(argv: List[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Lint Swift SDK documentation metadata.")
    parser.add_argument(
        "--doc-root",
        type=Path,
        default=DEFAULT_DOC_ROOT,
        help="Path to specs/sdk/swift (defaults to repository copy).",
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


def lint_docs(doc_paths: Iterable[Path]) -> List[str]:
    errors: List[str] = []
    paths = sorted(Path(path).resolve() for path in doc_paths if Path(path).is_file())

    for path in paths:
        rel = _safe_relative(path)
        metadata = extract_front_matter(path.read_text(encoding="utf-8"))
        errors.extend(_lint_english_doc(rel, metadata))

    return errors


def _lint_english_doc(rel_path: str, metadata: Dict[str, str] | None) -> List[str]:
    if metadata is None:
        return [f"{rel_path}: missing YAML front matter with title/summary."]
    missing = [key for key in ("title", "summary") if not metadata.get(key, "").strip()]
    if missing:
        return [f"{rel_path}: missing required metadata keys: {', '.join(missing)}."]
    return []


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
    errors = lint_docs(doc_paths)
    if errors:
        print("Swift doc lint failed:", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        return 1
    print("Swift doc lint passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
