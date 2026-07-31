"""Tests for scripts/swift_doc_lint.py."""

from __future__ import annotations

import importlib.util
import tempfile
from pathlib import Path
from unittest import TestCase

MODULE_PATH = Path(__file__).resolve().parents[1] / "swift_doc_lint.py"
SPEC = importlib.util.spec_from_file_location("swift_doc_lint", MODULE_PATH)
DOC_LINT = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(DOC_LINT)  # type: ignore[attr-defined]


class SwiftDocLintTests(TestCase):
    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.doc_root = Path(self.tempdir.name) / "swift_docs"
        self.doc_root.mkdir()

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def write_doc(self, name: str, content: str) -> Path:
        path = self.doc_root / name
        path.write_text(content, encoding="utf-8")
        return path

    def test_extract_front_matter_skips_comments(self) -> None:
        text = "<!-- notice -->\n\n---\ntitle: Demo\nsummary: Example\n---\n# Body"
        metadata = DOC_LINT.extract_front_matter(text)
        self.assertIsNotNone(metadata)
        assert metadata is not None
        self.assertEqual(metadata["title"], "Demo")
        self.assertEqual(metadata["summary"], "Example")

    def test_extract_front_matter_missing(self) -> None:
        text = "# No metadata yet"
        self.assertIsNone(DOC_LINT.extract_front_matter(text))

    def test_lint_docs_flags_missing_english_metadata(self) -> None:
        self.write_doc("overview.md", "# Heading only")
        errors = DOC_LINT.lint_docs(self.doc_root.glob("*.md"))
        self.assertTrue(any("overview.md" in err for err in errors))

    def test_lint_docs_passes_well_formed_docs(self) -> None:
        self.write_doc(
            "index.md",
            "---\ntitle: Landing\nsummary: Swift landing page.\n---\n# Index\n",
        )
        errors = DOC_LINT.lint_docs(self.doc_root.glob("*.md"))
        self.assertFalse(errors)
