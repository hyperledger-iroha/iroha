"""Adversarial tests for hash-bound, read-only Norito declaration patching."""

from __future__ import annotations

import copy
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / "apply_norito_schema_identities.py"
SPEC = importlib.util.spec_from_file_location("apply_norito_schema_identities", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
helper = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = helper
SPEC.loader.exec_module(helper)


class IdentityPatchTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.mapping = self.root / "identities.json"

    def entry(self, source, anchor, *, path="source.rs", kind="struct", identifier="Example", nominal="captured::Example", frame=None):
        source = source.encode("utf-8") if isinstance(source, str) else source
        anchor = anchor.encode("utf-8") if isinstance(anchor, str) else anchor
        destination = self.root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(source)
        start = source.index(anchor)
        row = {"start_byte": start, "end_byte": start + len(anchor), "anchor": anchor.decode("utf-8"),
               "kind": kind, "identifier": identifier, "nominal": nominal}
        if frame is not None:
            row["frame"] = frame
        return {"path": path, "sha256": helper.sha256(source), "declarations": [row]}

    def save(self, *entries):
        self.mapping.write_text(json.dumps({"schema": 1, "files": entries}), encoding="utf-8")

    def plan(self, *entries):
        self.save(*entries)
        return helper.generate(self.root, helper.load_mapping(self.mapping))

    def command(self, *arguments):
        return subprocess.run(
            [sys.executable, str(SCRIPT), str(self.mapping), "--root", str(self.root), *arguments],
            text=True, capture_output=True, check=False,
        )

    def test_multiple_declarations_preserve_unrelated_utf8_and_attributes(self):
        source = ('//! café evidence\n'
                  'const SAMPLE: &str = r###"struct Ghost { }"###;\n'
                  'mod values {\n'
                  '    // struct False; /* ignored */\n'
                  '    #[derive(Clone)]\n'
                  '    pub(crate) struct Example(u32);\n'
                  '    #[cfg_attr(feature = "json", derive(Clone), allow(dead_code))]\n'
                  '    pub enum Other { A, B(u8) }\n'
                  '}\n')
        first = self.entry(source, "pub(crate) struct Example(u32);")
        second = self.entry(source, "pub enum Other { A, B(u8) }", kind="enum", identifier="Other", nominal="captured::Other")
        first["declarations"].extend(second["declarations"])
        patch, = self.plan(first)
        self.assertEqual((self.root / "source.rs").read_bytes(), source.encode())
        self.assertEqual(patch.result.count(b"#[derive(norito::NoritoSchema)]"), 2)
        expected = source.replace("pub(crate) struct", '#[derive(norito::NoritoSchema)]\n    #[norito_schema(name = "captured::Example")]\n    pub(crate) struct')
        expected = expected.replace("pub enum Other", '#[derive(norito::NoritoSchema)]\n    #[norito_schema(name = "captured::Other")]\n    pub enum Other')
        self.assertEqual(patch.result, expected.encode())

    def test_exact_result_is_idempotent_and_check_reports_pending(self):
        entry = self.entry("pub struct Example(u32);\n", "pub struct Example(u32);")
        patch, = self.plan(entry)
        pending = self.command("--check")
        self.assertEqual(pending.returncode, 1, pending.stderr)
        self.assertEqual(pending.stdout, "")
        self.assertEqual(self.command().stdout, helper.unified_patch([patch]))
        (self.root / "source.rs").write_bytes(patch.result)
        verified = self.command("--check")
        self.assertEqual(verified.returncode, 0, verified.stderr)
        self.assertEqual(self.command().stdout, "")
        report = json.loads(verified.stderr)
        self.assertEqual(report["files"][0]["original_sha256"], entry["sha256"])
        self.assertEqual(report["files"][0]["result_sha256"], helper.sha256(patch.result))

    def test_patch_roundtrip_preserves_crlf_and_missing_final_newline(self):
        entry = self.entry(b"// header\r\nstruct Example;", "struct Example;")
        patch, = self.plan(entry)
        # Git is already required by this repository. The helper itself never runs it.
        checked = subprocess.run(["git", "apply", "--check", "-"], input=helper.unified_patch([patch]).encode(), cwd=self.root, capture_output=True)
        self.assertEqual(checked.returncode, 0, checked.stderr)
        self.assertEqual((self.root / "source.rs").read_bytes(), patch.original)
        result = subprocess.run(["git", "apply", "-"], input=helper.unified_patch([patch]).encode(), cwd=self.root, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "source.rs").read_bytes(), patch.result)
        self.assertFalse(patch.result.endswith(b"\n"))
        self.assertNotIn(b"\n", patch.result.replace(b"\r\n", b""))
        self.assertEqual(self.command("--check").returncode, 0)

    def test_precise_span_disambiguates_same_text_in_different_modules(self):
        source = "mod first {\n    struct Example;\n}\nmod second {\n    struct Example;\n}\n"
        entry = self.entry(source, "struct Example;")
        row = entry["declarations"][0]
        row["start_byte"] = source.encode().rindex(b"struct Example;")
        row["end_byte"] = row["start_byte"] + len(row["anchor"])
        patch, = self.plan(entry)
        self.assertTrue(patch.result.startswith(source[:source.index("mod second")].encode()))
        self.assertEqual(patch.result.count(b"#[norito_schema"), 1)

    def test_raw_identifier_generics_and_explicit_literal_escaping(self):
        entry = self.entry("pub struct r#type<T>(T);\n", "pub struct r#type<T>(T);", identifier="r#type", nominal='captured::café"quote\\suffix')
        patch, = self.plan(entry)
        self.assertIn('name = "captured::café\\"quote\\\\suffix"'.encode(), patch.result)
        self.assertIn(b"pub struct r#type<T>(T);", patch.result)

    def test_explicit_projection_only_affects_requested_attribute(self):
        entry = self.entry("struct Example;\n", "struct Example;", frame="alloc::string::String")
        patch, = self.plan(entry)
        self.assertIn(b'name = "captured::Example", frame = "alloc::string::String"', patch.result)

    def test_entire_batch_validates_before_any_output_or_source_writes(self):
        first = self.entry("struct Example;\n", "struct Example;", path="a.rs")
        second = self.entry("struct Other;\n", "struct Other;", path="b.rs", identifier="Other", nominal="captured::Other")
        second["sha256"] = "0" * 64
        self.save(first, second)
        result = self.command()
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, "")
        self.assertEqual((self.root / "a.rs").read_text(), "struct Example;\n")
        self.assertEqual((self.root / "b.rs").read_text(), "struct Other;\n")

    def test_partial_batch_application_is_rejected(self):
        first = self.entry("struct Example;\n", "struct Example;", path="a.rs")
        second = self.entry("struct Other;\n", "struct Other;", path="b.rs", identifier="Other", nominal="captured::Other")
        patches = self.plan(first, second)
        (self.root / patches[0].path).write_bytes(patches[0].result)
        with self.assertRaisesRegex(helper.MappingError, "partial batch"):
            self.plan(first, second)

    def test_partial_file_application_and_post_application_drift_are_rejected(self):
        source = "struct Example;\nstruct Other;\n"
        entry = self.entry(source, "struct Example;")
        other = self.entry(source, "struct Other;", identifier="Other", nominal="captured::Other")
        entry["declarations"].extend(other["declarations"])
        patch, = self.plan(entry)
        prefix = helper.insertion(patch.original, 0, entry["declarations"][0])
        for data in [prefix + source.encode(), patch.result + b"// concurrent edit\n"]:
            with self.subTest(data=data):
                (self.root / "source.rs").write_bytes(data)
                with self.assertRaises(helper.MappingError):
                    self.plan(entry)

    def test_duplicate_records_and_nominal_identities_are_rejected(self):
        entry = self.entry("struct Example;\n", "struct Example;")
        for mutation in ["record", "file", "identity"]:
            with self.subTest(mutation=mutation):
                value = copy.deepcopy(entry)
                entries = [value]
                if mutation == "record":
                    other = copy.deepcopy(value["declarations"][0])
                    other["nominal"] = "different"
                    value["declarations"].append(other)
                elif mutation == "file":
                    entries.append(copy.deepcopy(value))
                else:
                    entries.append(self.entry("struct Other;\n", "struct Other;", path="other.rs", identifier="Other"))
                with self.assertRaises(helper.MappingError):
                    self.plan(*entries)

    def test_invalid_names_and_generic_projection_reject(self):
        for name in ["", " trailing", "trailing ", "line\nbreak", "nul\0", "\ud800", 3, None]:
            with self.subTest(name=repr(name)):
                entry = self.entry("struct Example;\n", "struct Example;")
                entry["declarations"][0]["nominal"] = name
                with self.assertRaises(helper.MappingError):
                    self.plan(entry)
        entry = self.entry("struct Example<T>(T);\n", "struct Example<T>(T);", frame="fixed")
        with self.assertRaisesRegex(helper.MappingError, "generic frame"):
            self.plan(entry)

    def test_malformed_schema_fields_and_span_types_fail_without_traceback(self):
        original = self.entry("struct Example;\n", "struct Example;")
        for key, value in [("start_byte", True), ("start_byte", "0"), ("start_byte", {}), ("end_byte", -1), ("kind", "union"), ("kind", []), ("kind", {}), ("identifier", "A::B"), ("anchor", None), ("unexpected", 1)]:
            with self.subTest(key=key, value=value):
                entry = copy.deepcopy(original)
                entry["declarations"][0][key] = value
                self.save(entry)
                result = self.command()
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertEqual(result.stdout, "")
                self.assertNotIn("Traceback", result.stderr)
        self.mapping.write_text('{"schema":1,"schema":1,"files":[]}')
        self.assertIn("duplicate JSON field", self.command().stderr)

    def test_span_must_cover_exact_complete_item_and_use_byte_offsets(self):
        source = "// café\nstruct Example { value: u32 }\n"
        original = self.entry(source, "struct Example { value: u32 }")
        for start_delta, end_delta in [(1, 0), (-1, 0), (0, -1), (0, 1)]:
            entry = copy.deepcopy(original)
            row = entry["declarations"][0]
            row["start_byte"] += start_delta
            row["end_byte"] += end_delta
            row["anchor"] = source.encode()[row["start_byte"]:row["end_byte"]].decode()
            with self.subTest(start=start_delta, end=end_delta):
                with self.assertRaises(helper.MappingError):
                    self.plan(entry)

    def test_generated_local_and_macro_containing_declarations_reject(self):
        cases = [
            "macro_rules! make { () => { struct Example; }; }\n",
            "#[model]\nmod model {\n    struct Example;\n}\n",
            '#[cfg_attr(feature = "x", model)]\nmod model {\n    struct Example;\n}\n',
            "#[replace_type]\nstruct Example;\n",
            "fn local() {\n    struct Example;\n}\n",
            "mod nested {\n    #![custom_generator]\n    struct Ignored;\n    struct Example;\n}\n",
        ]
        for source in cases:
            with self.subTest(source=source):
                entry = self.entry(source, "struct Example;")
                with self.assertRaises(helper.MappingError):
                    self.plan(entry)
        entry = self.entry("struct Example { data: generated!() }\n", "struct Example { data: generated!() }")
        with self.assertRaisesRegex(helper.MappingError, "macro-containing"):
            self.plan(entry)

    def test_comment_and_literal_anchors_are_never_items(self):
        for source in [
            "// struct Example;\n", "/* outer /* nested */ struct Example; */\n",
            'const S: &str = "struct Example;";\n',
            'const S: &str = r###"struct Example;"###;\n',
            'const S: &[u8] = br#"struct Example;"#;\n',
        ]:
            with self.subTest(source=source):
                entry = self.entry(source, "struct Example;")
                with self.assertRaises(helper.MappingError):
                    self.plan(entry)

    def test_existing_declarations_and_manual_or_alias_implementations_reject(self):
        cases = [
            '#[derive(norito::NoritoSchema)]\n#[norito_schema(name = "other")]\nstruct Example;\n',
            '#[derive(norito::NoritoSchema)]\nstruct Example;\n',
            '#[cfg_attr(feature = "x", derive(norito::NoritoSchema))]\nstruct Example;\n',
            'struct Example;\nimpl norito::NoritoSchema for Example { fn nominal_name() -> String { "x".into() } }\n',
            'use norito::NoritoSchema as Identity;\nstruct Example;\n',
            'struct Example;\nimpl norito::NoritoSchema for a::b::c::d::e::f::g::h::i::Example {}\n',
        ]
        for source in cases:
            with self.subTest(source=source):
                entry = self.entry(source, "struct Example;")
                with self.assertRaises(helper.MappingError):
                    self.plan(entry)

    def test_visible_wildcards_and_imported_derive_aliases_require_review(self):
        for source, message in [
            ("use other::*;\nstruct Example;\n", "wildcard"),
            ("use other::{nested::*};\nmod values {\n    struct Example;\n}\n", "wildcard"),
            ("mod values {\n    struct Example;\n    use super::*;\n}\n", "wildcard"),
            ("use other::Identity as Wire;\n#[derive(Wire)]\nstruct Example;\n", "derive aliases"),
            ('use other::{Identity as Wire};\n#[cfg_attr(feature = "x", derive(Wire))]\nstruct Example;\n', "derive aliases"),
        ]:
            with self.subTest(source=source):
                entry = self.entry(source, "struct Example;")
                with self.assertRaisesRegex(helper.MappingError, message):
                    self.plan(entry)

    def test_unrelated_test_module_wildcard_is_not_visible_to_outer_item(self):
        source = "struct Example;\n#[cfg(test)]\nmod tests { use super::*; }\n"
        entry = self.entry(source, "struct Example;")
        self.assertEqual(len(self.plan(entry)), 1)

    def test_grouped_ordinary_imports_do_not_hide_following_items(self):
        source = "use std::{string::String, vec::Vec};\nstruct Example(String, Vec<u8>);\n"
        entry = self.entry(source, "struct Example(String, Vec<u8>);")
        self.assertEqual(len(self.plan(entry)), 1)

    def test_symlink_paths_and_fifo_are_rejected_without_reading(self):
        entry = self.entry("struct Example;\n", "struct Example;")
        source = self.root / "source.rs"
        source.rename(self.root / "original.rs")
        source.symlink_to(self.root / "original.rs")
        with self.assertRaisesRegex(helper.MappingError, "symlink"):
            self.plan(entry)
        source.unlink()
        os.mkfifo(source)
        with self.assertRaisesRegex(helper.MappingError, "regular file"):
            self.plan(entry)
        source.unlink()
        directory = self.root / "linked"
        directory.symlink_to(self.root, target_is_directory=True)
        entry["path"] = "linked/original.rs"
        with self.assertRaisesRegex(helper.MappingError, "symlink"):
            self.plan(entry)

    def test_untrusted_paths_and_diff_header_injection_reject(self):
        original = self.entry("struct Example;\n", "struct Example;")
        for path in ["../outside.rs", "/tmp/out.rs", "a/../out.rs", "a//out.rs", "a\\out.rs", "bad\n+++ b/other.rs", "./source.rs", "source.txt"]:
            with self.subTest(path=path):
                entry = copy.deepcopy(original)
                entry["path"] = path
                with self.assertRaises(helper.MappingError):
                    self.plan(entry)

    def test_lexer_rejects_unterminated_or_unbalanced_source(self):
        for suffix in ['/* unfinished', 'const S: &str = r##"unfinished', 'const S: &str = "unfinished', '}']:
            entry = self.entry("struct Example;\n" + suffix, "struct Example;")
            with self.subTest(suffix=suffix):
                with self.assertRaises(helper.MappingError):
                    self.plan(entry)

    def test_unclosed_generic_parameters_are_not_guessed(self):
        entry = self.entry("struct Example<T { value: T }\n", "struct Example<T { value: T }")
        with self.assertRaisesRegex(helper.MappingError, "unresolved"):
            self.plan(entry)
        entry = self.entry("struct Example<T: Fn() -> u32>(T);\n", "struct Example<T: Fn() -> u32>(T);")
        self.assertEqual(len(self.plan(entry)), 1)

    def test_late_source_drift_aborts_entire_patch(self):
        entry = self.entry("struct Example;\n", "struct Example;")
        self.save(entry)
        actual_read = helper.read_source
        calls = 0

        def changing_read(root, name):
            nonlocal calls
            calls += 1
            if calls == 2:
                (root / name).write_text("// concurrent writer\nstruct Example;\n")
            return actual_read(root, name)

        with mock.patch.object(helper, "read_source", side_effect=changing_read):
            with self.assertRaisesRegex(helper.MappingError, "changed during batch"):
                helper.generate(self.root, helper.load_mapping(self.mapping))

    def test_no_apply_mode_is_exposed(self):
        self.save(self.entry("struct Example;\n", "struct Example;"))
        result = self.command("--apply")
        self.assertEqual(result.returncode, 2)
        self.assertIn("unrecognized arguments", result.stderr)
        self.assertEqual((self.root / "source.rs").read_text(), "struct Example;\n")


if __name__ == "__main__":
    unittest.main()
