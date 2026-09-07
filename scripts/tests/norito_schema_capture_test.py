"""Exercise source isolation and fail-closed compiler capture collection."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "norito_capture_under_test", Path(__file__).resolve().parents[1] / "norito_schema_capture.py"
)
assert SPEC and SPEC.loader
CAPTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAPTURE)
SOURCE = """//! Fixture derive module.
mod schema_identity;
#[proc_macro_derive(NoritoSerialize)]
pub fn derive_norito_serialize(input: TokenStream) -> TokenStream {
    original_encode(input)
}
#[proc_macro_derive(NoritoDeserialize)]
pub fn derive_norito_deserialize(input: TokenStream) -> TokenStream {
    original_decode(input)
}
"""


def encoded(value: str) -> str:
    return value.encode().hex()


def record(**overrides: str) -> str:
    fields = dict(direction="serialize", nominal=encoded("actual::Private"),
                  root=encoded("declared::Root"), hash="ab" * 16,
                  file=encoded("crates/example/src/lib.rs"), line="12", column="4",
                  identifier=encoded("Private"), module=encoded("actual"),
                  explicit="true", matches="true")
    fields.update(overrides)
    return CAPTURE.MARKER + "\t".join(fields.values()) + "\n"


class CaptureTest(unittest.TestCase):
    def test_instrument_preserves_original_bodies_and_attributes(self):
        result = CAPTURE.instrument(SOURCE)
        self.assertIn("original_encode(input)", result)
        self.assertIn("original_decode(input)", result)
        self.assertEqual(result.count("#[proc_macro_derive("), 2)
        self.assertEqual(result.count("mod schema_capture;"), 1)
        self.assertEqual(result.count("attrs.schema_name.as_deref()"), 2)
        self.assertIn("Direction::Serialize", result)
        self.assertIn("Direction::Deserialize", result)
        self.assertEqual(SOURCE.count("capture"), 0)

    def test_instrument_rejects_changed_or_already_instrumented_anchors(self):
        for source in (SOURCE + SOURCE, SOURCE.replace("mod schema_identity;", ""),
                       SOURCE.replace("pub fn derive_norito_serialize", "pub fn changed"),
                       CAPTURE.instrument(SOURCE)):
            with self.subTest(source=source[:50]), self.assertRaises(ValueError):
                CAPTURE.instrument(source)

    def test_prepare_copies_exact_dirty_inputs_and_edits_only_snapshot(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve() / "repo"
            destination = root.parent / "capture"
            for relative, payload in ((CAPTURE.DERIVE, SOURCE), (CAPTURE.PROBE, "//! Probe.\n"),
                                      ("dirty.txt", "uncommitted evidence\n")):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(payload)
            paths = [CAPTURE.DERIVE, CAPTURE.PROBE, "dirty.txt", "deleted.txt"]
            with patch.object(CAPTURE, "selected_sources", return_value={"paths": paths, "git_links": []}), \
                    patch.object(CAPTURE.subprocess, "check_output", return_value="a" * 40):
                manifest = CAPTURE.prepare(root, destination)
            self.assertEqual((root / CAPTURE.DERIVE).read_text(), SOURCE)
            self.assertIn("mod schema_capture;", (destination / "source" / CAPTURE.DERIVE).read_text())
            self.assertEqual((destination / "source/dirty.txt").read_text(), "uncommitted evidence\n")
            self.assertNotEqual((root / "dirty.txt").stat().st_ino,
                                (destination / "source/dirty.txt").stat().st_ino)
            self.assertEqual(manifest["source"]["deleted"], 1)
            self.assertEqual(manifest["execution_source"], CAPTURE.tree_seal(destination / "source"))
            self.assertEqual(manifest, json.loads((destination / "capture.json").read_text()))
            with self.assertRaisesRegex(ValueError, "already exists"):
                CAPTURE.prepare(root, destination)

    def test_prepare_rejects_repository_destination_including_symlink_parent(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            alias = root / "alias"
            alias.symlink_to(root, target_is_directory=True)
            for destination in (root / "capture", alias / "capture"):
                with self.assertRaisesRegex(ValueError, "outside the repository"):
                    CAPTURE.prepare(root, destination)

    def test_prepare_rejects_concurrent_inventory_changes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve() / "repo"
            for relative, payload in ((CAPTURE.DERIVE, SOURCE), (CAPTURE.PROBE, "//! Probe.\n")):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(payload)
            paths = [CAPTURE.DERIVE, CAPTURE.PROBE]
            with patch.object(CAPTURE, "selected_sources",
                              side_effect=[{"paths": paths, "git_links": []},
                                           {"paths": paths + ["new.rs"], "git_links": []}]), \
                    self.assertRaisesRegex(ValueError, "inventory changed"):
                CAPTURE.prepare(root, root.parent / "capture")
            self.assertFalse((root.parent / "capture/capture.json").exists())

    def test_selection_expands_dirty_gitlinks_and_preserves_absent_links(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            linked = root / "docs"
            linked.mkdir()
            (linked / "dirty.md").write_text("uncommitted")
            (root / "file.rs").write_text("// code")
            revision = "a" * 40
            index = f"160000 {revision} 0\tdocs\0" + f"160000 {revision} 0\tabsent\0"
            with patch.object(CAPTURE.PROFILER, "validate_git_worktree"), \
                    patch.object(CAPTURE.PROFILER, "tracked_and_untracked_paths",
                                 side_effect=[["absent", "docs", "file.rs"], ["dirty.md"]]), \
                    patch.object(CAPTURE.subprocess, "check_output",
                                 side_effect=[index.encode(), "b" * 40, b""]):
                result = CAPTURE.selected_sources(root, {}, "git")
            self.assertEqual(result["paths"], ["absent", "docs/dirty.md", "file.rs"])
            self.assertEqual(result["git_links"][0]["checkout_revision"], None)
            self.assertEqual(result["git_links"][1]["indexed_revision"], revision)
            self.assertEqual(result["git_links"][1]["checkout_revision"], "b" * 40)

    def test_selection_rejects_undeclared_directories_symlinks_and_conflicts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "directory").mkdir()
            (root / "link").symlink_to(root / "directory", target_is_directory=True)
            revision = "a" * 40
            cases = [(b"", ["directory"]),
                     (f"160000 {revision} 0\tlink\0".encode(), ["link"]),
                     (f"100644 {revision} 2\tconflict\0".encode(), [])]
            for index, selected in cases:
                with self.subTest(selected=selected), \
                        patch.object(CAPTURE.PROFILER, "validate_git_worktree"), \
                        patch.object(CAPTURE.PROFILER, "tracked_and_untracked_paths", return_value=selected), \
                        patch.object(CAPTURE.subprocess, "check_output", return_value=index), \
                        self.assertRaises(ValueError):
                    CAPTURE.selected_sources(root, {}, "git")

    def test_tree_seal_detects_added_deleted_and_changed_inputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "input").write_text("original")
            original = CAPTURE.tree_seal(root)
            (root / "added").write_text("new")
            self.assertNotEqual(original, CAPTURE.tree_seal(root))
            (root / "added").unlink()
            self.assertEqual(original, CAPTURE.tree_seal(root))
            (root / "input").write_text("changed")
            self.assertNotEqual(original, CAPTURE.tree_seal(root))
            (root / "input").unlink()
            self.assertNotEqual(original, CAPTURE.tree_seal(root))

    def test_parse_keeps_nominal_root_and_mismatch_distinct(self):
        result = CAPTURE.parse_record("test private::probe ... " + record(matches="false"))
        self.assertEqual(result["nominal"], "actual::Private")
        self.assertEqual(result["root_hint"], "declared::Root")
        self.assertFalse(result["root_matches"])
        self.assertTrue(result["explicit_root"])
        self.assertEqual(result["line"], 12)

    def test_generic_skip_never_invents_a_name(self):
        line = CAPTURE.SKIP + "\t".join([
            "deserialize", encoded("Generic"), encoded("src/lib.rs"), "1", "2",
            encoded("example"), "generic",
        ]) + "\n"
        result = CAPTURE.parse_record(line)
        self.assertEqual(result["reason"], "generic")
        self.assertNotIn("nominal", result)
        self.assertEqual(result["direction"], "deserialize")

    def test_parser_rejects_malformed_fields_and_ambiguous_output(self):
        bad = [record(direction="unknown"), record(hash="ab"), record(explicit="True"),
               record(matches="yes"), record(line="0"), record(column="-1"),
               record(nominal="FF"), record(nominal="ff"), record(root=""),
               record() + record(), "warning: " + record(), record().rstrip() + "\textra\n"]
        for line in bad:
            with self.subTest(line=line), self.assertRaises((ValueError, UnicodeError)):
                CAPTURE.parse_record(line)

    def test_collector_binds_one_harness_and_ignores_diagnostic_markers(self):
        with tempfile.TemporaryDirectory() as temporary:
            log = Path(temporary) / "cargo.log"
            events = [dict(reason="compiler-message", message=record()),
                      dict(reason="compiler-artifact", executable="/capture/target/test"),
                      dict(reason="build-finished", success=True)]
            log.write_text("".join(json.dumps(event) + "\n" for event in events) + record() +
                           "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s\n")
            result = CAPTURE.collect(log)
            self.assertTrue(result["build_success"])
            self.assertFalse(result["coverage_complete"])
            self.assertEqual(len(result["records"]), 1)

    def test_collector_rejects_missing_duplicate_and_unexecuted_probes(self):
        artifact = json.dumps(dict(reason="compiler-artifact", executable="/capture/target/test")) + "\n"
        summary = "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n"
        cases = [artifact + summary, artifact + record(), record() + summary,
                 artifact * 2 + record() + summary, artifact + record() * 2 + summary,
                 artifact + record() + summary * 2,
                 artifact + record() + summary.replace("1 passed", "2 passed"),
                 artifact + summary.replace("1 passed", "0 passed")]
        with tempfile.TemporaryDirectory() as temporary:
            log = Path(temporary) / "cargo.log"
            for payload in cases:
                log.write_text(payload)
                with self.subTest(payload=payload), self.assertRaises(ValueError):
                    CAPTURE.collect(log)

    def test_write_json_never_replaces_evidence_and_digest_is_canonical(self):
        self.assertEqual(CAPTURE.digest({"a": 1, "b": 2}), CAPTURE.digest({"b": 2, "a": 1}))
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "result.json"
            CAPTURE.write_json(path, {"first": True})
            with self.assertRaises(FileExistsError):
                CAPTURE.write_json(path, {"first": False})
            self.assertEqual(json.loads(path.read_text()), {"first": True})

    def test_explicit_tool_path_resolves_actual_executable(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            binary = root / "rustc"
            binary.write_text("fixture")
            link = root / "proxy"
            link.symlink_to(binary)
            self.assertEqual(CAPTURE.tool_path("rustc", str(link), root), binary)

    def test_compiled_harness_requires_one_new_test_artifact(self):
        valid = dict(reason="compiler-artifact", executable="/capture/target/test",
                     fresh=False, profile={"test": True})
        finished = dict(reason="build-finished", success=True)
        with tempfile.TemporaryDirectory() as temporary:
            log = Path(temporary) / "cargo.log"
            log.write_text(json.dumps(valid) + "\n" + json.dumps(finished) + "\n")
            self.assertEqual(CAPTURE.compiled_harness(log), valid)
            for artifact in ({**valid, "fresh": True}, {**valid, "profile": {"test": False}},
                             {**valid, "executable": None}):
                log.write_text(json.dumps(artifact) + "\n" + json.dumps(finished) + "\n")
                with self.subTest(artifact=artifact), self.assertRaises(ValueError):
                    CAPTURE.compiled_harness(log)
            log.write_text(json.dumps(valid) + "\n" + (json.dumps(finished) + "\n") * 2)
            with self.assertRaises(ValueError):
                CAPTURE.compiled_harness(log)

    def test_subprocess_runner_disables_wrappers_and_executes_the_exact_harness(self):
        # Fake compiler tools test orchestration, not Rust or codec correctness.
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary).resolve()
            source = destination / "source"
            source.mkdir()
            (source / "input.rs").write_text("// captured source")
            manifest = {"schema_version": 1, "git_revision": "a" * 40,
                        "execution_source": CAPTURE.tree_seal(source)}
            CAPTURE.write_json(destination / "capture.json", manifest)
            cargo = destination / "cargo"
            harness_source = f"#!{sys.executable}\nprint({record()!r}, end='')\n" + (
                "print('test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s')\n"
            )
            cargo.write_text(f"#!{sys.executable}\n" + f"""
import json, os, pathlib, sys
if '-vV' in sys.argv:
    print('fixture tool 1.0')
    raise SystemExit(0)
assert '--no-run' in sys.argv
assert os.environ['RUSTC_WRAPPER'] == ''
assert os.environ['RUSTC_WORKSPACE_WRAPPER'] == ''
target = pathlib.Path(os.environ['CARGO_TARGET_DIR'])
assert not target.exists()
target.mkdir()
harness = target / 'exact-harness'
harness.write_text({harness_source!r})
harness.chmod(0o700)
print(json.dumps(dict(reason='compiler-artifact', executable=str(harness),
                     fresh=False, profile=dict(test=True))))
print(json.dumps(dict(reason='build-finished', success=True)))
""")
            cargo.chmod(0o700)
            args = argparse.Namespace(snapshot=destination, name="fixture", package="example",
                                      test=None, no_default_features=False, features=None, jobs=2,
                                      cargo=str(cargo), rustc=str(cargo))
            with patch.dict(os.environ, {"RUSTC_WRAPPER": "/wrong/compiler",
                                         "RUSTC_WORKSPACE_WRAPPER": "/wrong/workspace/compiler",
                                         "CARGO_BUILD_RUSTC_WRAPPER": "/wrong/config/compiler"}):
                result = CAPTURE.run_capture(args)
            self.assertTrue(result["valid"], result)
            request = json.loads((destination / "runs/fixture/request.json").read_text())
            self.assertTrue(request["compiler_controls"]["direct_harness_execution"])
            self.assertEqual(result["harness_command"][0], str(destination / "runs/fixture/target/exact-harness"))
            self.assertEqual(result["returncode"], 0)
            self.assertEqual(result["summary"]["passed"], 1)


if __name__ == "__main__":
    unittest.main()
