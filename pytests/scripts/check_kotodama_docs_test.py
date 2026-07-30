"""Unit tests for the fail-closed Kotodama documentation fence checker."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "check_kotodama_docs", ROOT / "scripts" / "check_kotodama_docs.py"
)
assert SPEC is not None and SPEC.loader is not None
DOCS = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = DOCS
SPEC.loader.exec_module(DOCS)


class KotodamaDocsTests(unittest.TestCase):
    """Exercise inventory validation, Markdown extraction, and compilation."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_document(self, relative: str, contents: str) -> Path:
        """Create a UTF-8 Markdown fixture below the temporary repository."""

        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")
        return path

    def write_manifest(self, contents: str) -> Path:
        """Create a documentation inventory fixture."""

        path = self.root / "docs.json"
        path.write_text(contents, encoding="utf-8")
        return path

    def test_extracts_backtick_and_tilde_sources_with_modes(self) -> None:
        text = """# Sources

```kotodama
seiyaku Plain {}
```

~~~~ko zk
seiyaku Private {}
~~~~

```sh
koto check ignored.ko
```
"""
        fences = DOCS.extract_source_fences(Path("specs/demo.md"), text)

        self.assertEqual(len(fences), 2)
        self.assertEqual(fences[0].source, "seiyaku Plain {}\n")
        self.assertFalse(fences[0].zk)
        self.assertEqual(fences[0].opening_line, 3)
        self.assertTrue(fences[1].zk)
        self.assertEqual(fences[1].source_line, 8)

    def test_rejects_unknown_empty_and_unterminated_source_fences(self) -> None:
        invalid = (
            ("```Kotodama\ncontract A {}\n```\n", "must be lowercase"),
            ("```kotodama ignore\ncontract A {}\n```\n", "unknown"),
            ("```ko zk zk\ncontract A {}\n```\n", "duplicate"),
            ("```kotodama\n\n```\n", "empty"),
            ("```kotodama\ncontract A {}\n", "unterminated"),
        )
        for text, message in invalid:
            with self.subTest(message=message), self.assertRaisesRegex(
                DOCS.DocumentationCheckError, message
            ):
                DOCS.extract_source_fences(Path("specs/demo.md"), text)

    def test_rejects_missing_misspelled_aliased_and_nested_labels(self) -> None:
        invalid = (
            ("```\nseiyaku Missing {}\n```\n", "no language label"),
            ("```\nseiyaku {\n```\n", "apparent Kotodama"),
            ("```kotodma\nseiyaku Misspelled {}\n```\n", "non-canonical"),
            ("```koto\nseiyaku Aliased {}\n```\n", "non-canonical"),
            ("```{.kotodama}\nseiyaku ClassAlias {}\n```\n", "non-canonical"),
            ("```text kotodama\nseiyaku Nested {}\n```\n", "first info-string"),
            ("```rust\nmodule WrongLanguage {}\n```\n", "apparent Kotodama"),
        )
        for text, message in invalid:
            with self.subTest(text=text), self.assertRaisesRegex(
                DOCS.DocumentationCheckError, message
            ):
                DOCS.extract_source_fences(Path("specs/demo.md"), text)

    def test_extracts_ko_heredocs_without_treating_them_as_mislabeled(self) -> None:
        text = """```sh
cat > target/demo.ko <<'KO'
seiyaku ShellDemo {}
KO
```

```kotodama
seiyaku ExplicitDemo {}
```
"""

        fences = DOCS.extract_source_fences(Path("docs/demo.md"), text)

        self.assertEqual(len(fences), 2)
        self.assertEqual(fences[0].source, "seiyaku ShellDemo {}\n")
        self.assertEqual(fences[0].source_line, 3)
        self.assertEqual(fences[1].source, "seiyaku ExplicitDemo {}\n")

        with self.assertRaisesRegex(
            DOCS.DocumentationCheckError, "unterminated Kotodama heredoc"
        ):
            DOCS.extract_source_fences(
                Path("docs/demo.md"),
                "cat > demo.ko <<'KO'\ncontract Demo {}\n",
            )

    def test_unrelated_fences_remain_legal_but_cannot_hide_source(self) -> None:
        text = """```rust
mod demo {}
```

```text
module names do not require braces in this prose
```

```ebnf
seiyaku = keyword, identifier, body;
```

````markdown
```kotodama
this is a literal Markdown example, not source
```
````

```bash
echo 'an unrelated unterminated shell example'
"""
        self.assertEqual(
            DOCS.extract_source_fences(Path("docs/demo.md"), text),
            (),
        )

        hidden = """```sh
echo 'missing the closing fence'

```kotodama
seiyaku Hidden {}
```
"""
        with self.assertRaisesRegex(
            DOCS.DocumentationCheckError, "with 'sh'.*apparent Kotodama"
        ):
            DOCS.extract_source_fences(Path("docs/demo.md"), hidden)

        unterminated = """```text
seiyaku Hidden {}
"""
        with self.assertRaisesRegex(
            DOCS.DocumentationCheckError, "unterminated.*apparent Kotodama"
        ):
            DOCS.extract_source_fences(Path("docs/demo.md"), unterminated)

    def test_dash_heredoc_matches_shell_tab_stripping(self) -> None:
        fences = DOCS.extract_source_fences(
            Path("docs/demo.md"),
            "cat > demo.ko <<-'KO'\n"
            "\tmodule Demo {\n"
            "\t  fn helper() {}\n"
            "\t}\n"
            "\tKO\n",
        )

        self.assertEqual(len(fences), 1)
        self.assertEqual(
            fences[0].source,
            "module Demo {\n  fn helper() {}\n}\n",
        )

    def test_manifest_is_strict_and_binds_grammar_to_checked_documents(self) -> None:
        source = "```kotodama\nseiyaku A {}\n```\n"
        self.write_document("specs/grammar.md", source)
        self.write_document("specs/examples.md", source)
        manifest = self.write_manifest(
            json.dumps(
                {
                    "schema": 2,
                    "normative_grammar": "specs/grammar.md",
                    "source_roots": ["docs"],
                    "source_documents": [
                        "specs/grammar.md",
                        "specs/examples.md",
                    ],
                }
            )
        )

        document_set = DOCS.load_document_set(manifest, self.root)
        self.assertEqual(document_set.grammar, Path("specs/grammar.md"))
        required_only = DOCS.DocumentSet(
            grammar=document_set.grammar,
            documents=document_set.documents,
        )
        self.assertEqual(len(DOCS.collect_source_fences(required_only, self.root)), 2)

        payload = json.loads(manifest.read_text(encoding="utf-8"))
        payload["unexpected"] = True
        manifest.write_text(json.dumps(payload), encoding="utf-8")
        with self.assertRaisesRegex(DOCS.DocumentationCheckError, "unknown keys"):
            DOCS.load_document_set(manifest, self.root)

        payload.pop("unexpected")
        payload["schema"] = 2.0
        manifest.write_text(json.dumps(payload), encoding="utf-8")
        with self.assertRaisesRegex(DOCS.DocumentationCheckError, "schema must be"):
            DOCS.load_document_set(manifest, self.root)

    def test_manifest_rejects_duplicates_escape_and_unchecked_grammar(self) -> None:
        source = "```ko\nseiyaku A {}\n```\n"
        self.write_document("specs/grammar.md", source)
        self.write_document("specs/examples.md", source)

        duplicate = self.write_manifest(
            json.dumps(
                {
                    "schema": 2,
                    "normative_grammar": "specs/grammar.md",
                    "source_roots": ["docs"],
                    "source_documents": [
                        "specs/grammar.md",
                        "specs/grammar.md",
                    ],
                }
            )
        )
        with self.assertRaisesRegex(DOCS.DocumentationCheckError, "duplicate"):
            DOCS.load_document_set(duplicate, self.root)

        unchecked = self.write_manifest(
            json.dumps(
                {
                    "schema": 2,
                    "normative_grammar": "specs/grammar.md",
                    "source_roots": ["docs"],
                    "source_documents": ["specs/examples.md"],
                }
            )
        )
        with self.assertRaisesRegex(DOCS.DocumentationCheckError, "must also appear"):
            DOCS.load_document_set(unchecked, self.root)

        escaped = self.write_manifest(
            json.dumps(
                {
                    "schema": 2,
                    "normative_grammar": "../outside.md",
                    "source_roots": ["docs"],
                    "source_documents": ["../outside.md"],
                }
            )
        )
        with self.assertRaisesRegex(DOCS.DocumentationCheckError, "normalized"):
            DOCS.load_document_set(escaped, self.root)

    def test_manifest_requires_every_document_below_a_scanned_root(self) -> None:
        source = "```ko\nseiyaku A {}\n```\n"
        self.write_document("specs/grammar.md", source)
        self.write_document("examples/outside.md", source)
        manifest = self.write_manifest(
            json.dumps(
                {
                    "schema": 2,
                    "normative_grammar": "specs/grammar.md",
                    "source_roots": ["docs"],
                    "source_documents": [
                        "specs/grammar.md",
                        "examples/outside.md",
                    ],
                }
            )
        )

        with self.assertRaisesRegex(
            DOCS.DocumentationCheckError, "every source_document.*uncovered"
        ):
            DOCS.load_document_set(manifest, self.root)

    def test_each_inventoried_document_must_contain_source(self) -> None:
        grammar = self.write_document(
            "specs/grammar.md", "```kotodama\nseiyaku A {}\n```\n"
        )
        empty = self.write_document("specs/examples.md", "# No source\n")
        document_set = DOCS.DocumentSet(
            grammar=grammar.relative_to(self.root),
            documents=(grammar.relative_to(self.root), empty.relative_to(self.root)),
        )
        with self.assertRaisesRegex(
            DOCS.DocumentationCheckError, "contain no Kotodama source"
        ):
            DOCS.collect_source_fences(document_set, self.root)

    def test_compiler_checks_and_builds_every_deployable_source(self) -> None:
        fences = (
            DOCS.SourceFence(Path("grammar.md"), 4, 5, "seiyaku A {}\n", False),
            DOCS.SourceFence(Path("examples.md"), 8, 9, "誓約 B {}\n", True),
            DOCS.SourceFence(Path("modules.md"), 12, 13, "module Shared {}\n", False),
        )
        calls: list[tuple[list[str], str]] = []

        def run(
            command: list[str], **kwargs: object
        ) -> subprocess.CompletedProcess[str]:
            source = Path(command[-1]).read_text(encoding="utf-8")
            calls.append((command, source))
            return subprocess.CompletedProcess(command, 0, "checked", "")

        with mock.patch.object(DOCS.subprocess, "run", side_effect=run):
            DOCS.compile_source_fences(fences, Path("/bin/koto"), self.root, 5)

        self.assertEqual(len(calls), 3)
        self.assertEqual(calls[0][0][1:2], ["build"])
        self.assertNotIn("--zk", calls[0][0])
        self.assertEqual(calls[1][0][1:2], ["build"])
        self.assertIn("--zk", calls[1][0])
        self.assertEqual(calls[1][1], "誓約 B {}\n")
        self.assertEqual(calls[2][0][1:2], ["check"])
        self.assertEqual(calls[2][1], "module Shared {}\n")

    def test_compiler_deduplicates_identical_sources_by_execution_mode(self) -> None:
        fences = (
            DOCS.SourceFence(Path("one.md"), 1, 2, "seiyaku A {}\n", False),
            DOCS.SourceFence(Path("two.md"), 4, 5, "seiyaku A {}\n", False),
            DOCS.SourceFence(Path("zk.md"), 7, 8, "seiyaku A {}\n", True),
        )

        with mock.patch.object(
            DOCS.subprocess,
            "run",
            return_value=subprocess.CompletedProcess([], 0, "", ""),
        ) as run:
            DOCS.compile_source_fences(fences, Path("/bin/koto"), self.root, 5)

        self.assertEqual(run.call_count, 2)

    def test_codegen_failures_are_aggregated_with_document_locations(self) -> None:
        fences = (
            DOCS.SourceFence(Path("grammar.md"), 4, 5, "seiyaku A {}\n", False),
        )

        def fail_build(
            command: list[str], **kwargs: object
        ) -> subprocess.CompletedProcess[str]:
            if command[1] == "build":
                return subprocess.CompletedProcess(
                    command, 1, "", "K5001: assembler rejected source"
                )
            return subprocess.CompletedProcess(command, 0, "", "")

        with mock.patch.object(
            DOCS.subprocess, "run", side_effect=fail_build
        ), self.assertRaises(DOCS.DocumentationCheckError) as caught:
            DOCS.compile_source_fences(fences, Path("/bin/koto"), self.root, 5)

        message = str(caught.exception)
        self.assertIn("grammar.md:4", message)
        self.assertIn("failed `koto build`", message)
        self.assertIn("K5001", message)

    def test_compiler_failures_are_aggregated_with_document_locations(self) -> None:
        fences = (
            DOCS.SourceFence(Path("grammar.md"), 4, 5, "bad one\n", False),
            DOCS.SourceFence(Path("examples.md"), 8, 9, "bad two\n", False),
        )

        def fail(
            command: list[str], **kwargs: object
        ) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess(command, 1, "", "K1001: invalid source")

        with mock.patch.object(
            DOCS.subprocess, "run", side_effect=fail
        ), self.assertRaises(DOCS.DocumentationCheckError) as caught:
            DOCS.compile_source_fences(fences, Path("/bin/koto"), self.root, 5)

        message = str(caught.exception)
        self.assertIn("grammar.md:4", message)
        self.assertIn("examples.md:8", message)
        self.assertEqual(message.count("K1001"), 2)

    def test_checked_in_inventory_covers_all_current_v1_sources(self) -> None:
        document_set = DOCS.load_document_set(
            ROOT / DOCS.DEFAULT_MANIFEST, ROOT
        )
        fences = DOCS.collect_source_fences(document_set, ROOT)

        self.assertEqual(
            document_set.grammar, Path("specs/kotodama_grammar.md")
        )
        self.assertEqual(
            document_set.documents,
            (
                Path("specs/kotodama_grammar.md"),
                Path("specs/kotodama_examples.md"),
            ),
        )
        self.assertEqual(document_set.source_roots, (Path("docs"),))
        # The V1 reset removed the parallel English-syntax copies from every
        # translated example. Each of the 672 tracked documents still carries
        # at least one canonical source, while the normative grammar and Numeric
        # V1 specification contribute the additional distinct examples.
        self.assertEqual(len(fences), 677)
        self.assertEqual(len({fence.document for fence in fences}), 672)
        self.assertEqual(len({(fence.source, fence.zk) for fence in fences}), 15)


if __name__ == "__main__":
    unittest.main()
