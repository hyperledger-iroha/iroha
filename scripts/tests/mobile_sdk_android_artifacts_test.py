#!/usr/bin/env python3
"""External Kotlin artifact selection and complete SBOM inventory regressions."""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
OWNER = ROOT / "scripts/mobile_sdk_android_artifacts.py"
SPEC = importlib.util.spec_from_file_location("mobile_sdk_android_artifacts", OWNER)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class AndroidArtifactOwnerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mobile-kotlin-artifacts.")
        self.directory = Path(self.temp.name).resolve()
        self.repo = self.directory / "repo"
        self.repo.mkdir()
        self.external = self.directory / "artifacts"
        self.external.mkdir()
        self.build = self.external / "gradle-build/iroha_kotlin_sdk"

    def tearDown(self):
        self.temp.cleanup()

    def outputs(self, external=True):
        core = self.build / "core-jvm" if external else self.repo / "kotlin/core-jvm/build"
        client = self.build / "client-android" if external else self.repo / "kotlin/client-android/build"
        jar = core / "libs/core-jvm-1.2.3.jar"
        aar = client / "outputs/aar/client-android-release.aar"
        for path in (jar, aar):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"fresh external" if external else b"stale source")
        return jar, aar

    def sboms(self):
        for name in MODULE.SDK_MODULES:
            path = self.build / name / "reports/bom/bom.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps({
                "bomFormat": "CycloneDX", "specVersion": "1.6", "version": 1,
                "metadata": {"component": {"group": "org.hyperledger.iroha.sdk",
                                             "name": name, "version": "1.2.3"}},
                "components": [{"name": "dependency"}]
            }))

    def collect(self, destination=None):
        MODULE.collect_sboms(self.repo, str(self.external), destination or self.directory / "collected", "1.2.3")

    def test_external_root_selects_fresh_output_and_ignores_source_decoys(self):
        expected = self.outputs()
        self.outputs(external=False)
        self.assertEqual(MODULE.built_artifacts(self.repo, str(self.external)), expected)

    def test_missing_external_output_cannot_fall_back_to_stale_source(self):
        self.outputs(external=False)
        with self.assertRaises(ValueError):
            MODULE.built_artifacts(self.repo, str(self.external))

    def test_ordinary_local_output_is_explicitly_preserved_without_override(self):
        expected = self.outputs(external=False)
        self.assertEqual(MODULE.built_artifacts(self.repo, None), expected)

    def test_invalid_external_root_forms_deny(self):
        link = self.directory / "alias"
        link.symlink_to(self.external, target_is_directory=True)
        values = ["", "relative", str(link), str(self.external / ".." / "artifacts"),
                  str(self.directory / "missing"), str(self.repo), str(self.repo / "kotlin")]
        (self.repo / "kotlin").mkdir()
        for value in values:
            with self.subTest(value=value), self.assertRaises((OSError, ValueError)):
                MODULE.built_artifacts(self.repo, value)

    def test_symlinked_nested_output_cannot_substitute_source_artifacts(self):
        self.outputs(external=False)
        self.build.mkdir(parents=True)
        (self.build / "core-jvm").symlink_to(self.repo / "kotlin/core-jvm/build", target_is_directory=True)
        with self.assertRaises(ValueError):
            MODULE.built_artifacts(self.repo, str(self.external))

    def test_multiple_runtime_jars_are_not_silently_selected(self):
        jar, _ = self.outputs()
        jar.with_name("core-jvm-old.jar").write_bytes(b"old")
        with self.assertRaises(ValueError):
            MODULE.built_artifacts(self.repo, str(self.external))

    def test_source_and_javadoc_jars_do_not_replace_runtime_jar(self):
        jar, aar = self.outputs()
        for suffix in ("sources", "javadoc"):
            jar.with_name(f"core-jvm-1.2.3-{suffix}.jar").write_bytes(b"documentation")
        self.assertEqual(MODULE.built_artifacts(self.repo, str(self.external)), (jar, aar))

    def test_linked_or_empty_artifact_is_rejected(self):
        jar, aar = self.outputs()
        substitute = self.directory / "substitute.jar"
        jar.rename(substitute)
        jar.symlink_to(substitute)
        with self.assertRaises(ValueError):
            MODULE.built_artifacts(self.repo, str(self.external))
        jar.unlink()
        os.link(substitute, jar)
        with self.assertRaises(ValueError):
            MODULE.built_artifacts(self.repo, str(self.external))
        jar.unlink()
        jar.write_bytes(b"jar")
        aar.write_bytes(b"")
        with self.assertRaises(ValueError):
            MODULE.built_artifacts(self.repo, str(self.external))

    def test_all_three_canonical_sboms_are_collected(self):
        self.sboms()
        self.collect()
        self.assertEqual({p.name for p in (self.directory / "collected").iterdir()},
                         {f"iroha-{name}.cyclonedx.json" for name in MODULE.SDK_MODULES})

    def test_retired_java_reports_cannot_satisfy_missing_canonical_sbom(self):
        self.sboms()
        expected = self.build / "core-jvm/reports/bom/bom.json"
        old = self.repo / "java/iroha_android/jvm/build/reports/bom/bom.json"
        old.parent.mkdir(parents=True)
        expected.replace(old)
        with self.assertRaises(FileNotFoundError):
            self.collect()
        self.assertFalse((self.directory / "collected").exists())

    def test_sbom_identity_and_version_substitution_deny_before_output(self):
        self.sboms()
        path = self.build / "client-android/reports/bom/bom.json"
        original = json.loads(path.read_text())
        for key, value in [("group", "org.hyperledger.iroha"), ("name", "android"), ("version", "old")]:
            changed = json.loads(json.dumps(original))
            changed["metadata"]["component"][key] = value
            path.write_text(json.dumps(changed))
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.collect()
            self.assertFalse((self.directory / "collected").exists())

    def test_sbom_symlink_and_existing_generation_are_not_overwritten(self):
        self.sboms()
        path = self.build / "client-android/reports/bom/bom.json"
        real = path.with_name("retained.json")
        path.rename(real)
        path.symlink_to(real)
        with self.assertRaises(ValueError):
            self.collect()
        path.unlink()
        real.rename(path)
        self.collect()
        with self.assertRaises(FileExistsError):
            self.collect()

    def test_cli_explicit_empty_override_is_not_treated_as_unset(self):
        self.outputs(external=False)
        result = subprocess.run([sys.executable, "-I", "-B", str(OWNER), "--root", str(self.repo),
                                 "--artifact-dir", ""], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")

    def test_release_workflow_requires_core_test_before_any_publication(self):
        workflow = (ROOT / ".github/workflows/mobile_sdk_artifacts.yml").read_text()
        block = workflow.split("- name: Test, lint, publish, and build Android SDK artifacts", 1)[1].split("- name:", 1)[0]
        self.assertEqual(block.count(":core-jvm:test"), 1)
        self.assertLess(block.index(":core-jvm:test"), block.index(":core-jvm:publish"))

    def test_checker_preserves_explicit_external_root_through_isolated_environment(self):
        environment = os.environ.copy()
        environment["MOBILE_SDK_ANDROID_ARTIFACT_DIR"] = ""
        environment.pop("MOBILE_SDK_PYTHON_BINARY", None)
        result = subprocess.run(["/bin/bash", str(ROOT / "scripts/check_mobile_sdk_artifacts.sh"),
                                 "--android-only", "--require-built-android"],
                                env=environment, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("MOBILE_SDK_ANDROID_ARTIFACT_DIR must be an absolute", result.stderr)


if __name__ == "__main__":
    unittest.main()
