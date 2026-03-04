#!/usr/bin/env python3
"""
Guardrail that prevents publishing locales with stub-only translations.

The guard loads the published locale list from docs/i18n/published_locales.json,
scans translation files for each published locale, and fails if any are still
marked as `needs-translation`. Use DOCS_I18N_STUB_GUARD_ALLOW=1 to bypass.
"""

from __future__ import annotations

import argparse
import importlib.machinery
import importlib.util
import json
import os
import sys
from pathlib import Path
from typing import Iterable, List, Sequence

DEFAULT_ROOT = Path(__file__).resolve().parent.parent


def load_sync_module(root: Path):
    module_path = root / "scripts" / "sync_docs_i18n.py"
    spec = importlib.util.spec_from_loader(
        "sync_docs_i18n",
        importlib.machinery.SourceFileLoader("sync_docs_i18n", str(module_path)),
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("Unable to load sync_docs_i18n module.")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    module.REPO_ROOT = root
    module.MANIFEST_PATH = root / "docs" / "i18n" / "manifest.json"
    return module


def load_published_locales(path: Path, fallback: str) -> List[str]:
    if not path.exists():
        return [fallback]
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, list) or not all(isinstance(item, str) for item in data):
        raise ValueError(f"published locales file must be a JSON array of strings: {path}")
    return [code.lower() for code in data]


def iter_portal_translation_files(root: Path, locale: str) -> Iterable[Path]:
    portal_root = root / "docs" / "portal" / "i18n" / locale
    if not portal_root.exists():
        return []
    return portal_root.rglob("*")


def find_stub_translations(
    *,
    root: Path,
    locale: str,
    manifest,
    sync_module,
) -> List[Path]:
    stubs: List[Path] = []
    for source_rel in sync_module.collect_source_files(manifest):
        translation_rel = sync_module.compute_translation_path(source_rel, locale)
        translation_abs = root / translation_rel
        if translation_abs.exists() and sync_module.is_stub_translation(translation_abs):
            stubs.append(translation_rel)

    for path in iter_portal_translation_files(root, locale):
        if not path.is_file():
            continue
        if path.suffix.lower() not in {".md", ".mdx", ".org"}:
            continue
        if sync_module.is_stub_translation(path):
            stubs.append(path.relative_to(root))

    return stubs


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Fail when published locales still contain stub translations."
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=DEFAULT_ROOT,
        help="Repository root (defaults to the repo root inferred from this script).",
    )
    parser.add_argument(
        "--published-locales",
        type=Path,
        help="Override published locales JSON path (defaults to docs/i18n/published_locales.json).",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    if os.environ.get("DOCS_I18N_STUB_GUARD_ALLOW") == "1":
        print("docs i18n stub guard bypassed via DOCS_I18N_STUB_GUARD_ALLOW=1")
        return 0

    args = parse_args(argv)
    root = args.repo_root.resolve()
    sync_module = load_sync_module(root)
    manifest = sync_module.Manifest.load(sync_module.MANIFEST_PATH)
    primary = manifest.primary.code.lower()
    known = {primary}
    known.update(lang.code.lower() for lang in manifest.targets)

    locales_path = args.published_locales or root / "docs" / "i18n" / "published_locales.json"
    published = load_published_locales(locales_path, primary)
    unknown = sorted(set(published) - known)
    if unknown:
        print(
            f"error: published locales contain unknown codes: {', '.join(unknown)}",
            file=sys.stderr,
        )
        return 1

    stubbed: List[Path] = []
    for locale in published:
        if locale == primary:
            continue
        stubbed.extend(find_stub_translations(root=root, locale=locale, manifest=manifest, sync_module=sync_module))

    if stubbed:
        print(
            "error: published locales include stub-only translations marked needs-translation:",
            file=sys.stderr,
        )
        for path in sorted(set(stubbed)):
            print(f" - {path.as_posix()}", file=sys.stderr)
        print(
            "Remove the locale from published_locales.json or finish the translations.",
            file=sys.stderr,
        )
        return 1

    print("docs i18n stub guard passed: published locales contain no stub translations.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
