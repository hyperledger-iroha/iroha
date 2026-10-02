#!/usr/bin/env python3
"""External Kotlin artifact selection and complete SBOM inventory regressions."""

from __future__ import annotations

import importlib.util
import json
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

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

    def pom(self, module, version="1.2.3"):
        dependency = {"core-jvm": "", "client-android": "core-jvm",
                      "kagemusha-wallet-android": "client-android"}[module]
        deps = (f"<dependencies><dependency><groupId>org.hyperledger.iroha.sdk</groupId>"
                f"<artifactId>{dependency}</artifactId><version>{version}</version>"
                "</dependency></dependencies>") if dependency else ""
        packaging = "jar" if module == "core-jvm" else "aar"
        return (f'<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion>'
                f'<groupId>org.hyperledger.iroha.sdk</groupId><artifactId>{module}</artifactId>'
                f'<version>{version}</version><packaging>{packaging}</packaging>{deps}</project>')

    def outputs(self, external=True):
        builds = {module: self.build / module if external else self.repo / "kotlin" / module / "build"
                  for module in MODULE.SDK_MODULES}
        paths = (builds["core-jvm"] / "libs/core-jvm-1.2.3.jar",
                 builds["client-android"] / "outputs/aar/client-android-release.aar",
                 builds["kagemusha-wallet-android"] / "outputs/aar/kagemusha-wallet-android-release.aar")
        for module, path in zip(MODULE.SDK_MODULES, paths, strict=True):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"fresh external" if external else b"stale source")
            pom = builds[module] / "publications/release/pom-default.xml"
            pom.parent.mkdir(parents=True, exist_ok=True)
            pom.write_text(self.pom(module))
        return paths

    def maven(self):
        outputs = self.outputs()
        maven = self.external / "maven"
        for module, output in zip(MODULE.SDK_MODULES, outputs, strict=True):
            directory = maven / "org/hyperledger/iroha/sdk" / module / "1.2.3"
            directory.mkdir(parents=True)
            extension = "jar" if module == "core-jvm" else "aar"
            (directory / f"{module}-1.2.3.{extension}").write_bytes(output.read_bytes())
            (directory / f"{module}-1.2.3.pom").write_text(self.pom(module))
            self.module_metadata(module, directory / f"{module}-1.2.3.{extension}")
        return maven

    def module_metadata(self, module, artifact):
        dependency = {"core-jvm": "", "client-android": "core-jvm",
                      "kagemusha-wallet-android": "client-android"}[module]
        variants = []
        for usage in ("java-api", "java-runtime"):
            attributes = {"org.gradle.category": "library", "org.gradle.dependency.bundling": "external",
                          "org.gradle.libraryelements": "jar" if module == "core-jvm" else "aar",
                          "org.gradle.usage": usage}
            if module == "core-jvm":
                attributes.update({"org.gradle.jvm.version": 8, "org.gradle.jvm.environment": "standard-jvm"})
            dependencies = ([{"group": "org.hyperledger.iroha.sdk", "module": dependency,
                              "version": {"requires": "1.2.3"}}] if dependency else [])
            variants.append({"name": "apiElements" if usage == "java-api" else "runtimeElements",
                             "attributes": attributes, "dependencies": dependencies,
                             "files": [{"name": artifact.name, "url": artifact.name, "size": artifact.stat().st_size,
                                        **{algorithm: hashlib.new(algorithm, artifact.read_bytes()).hexdigest()
                                           for algorithm in ("sha256", "sha512", "sha1", "md5")}}]})
        path = artifact.with_suffix(".module")
        path.write_text(json.dumps({"formatVersion": "1.1", "component": {
            "group": "org.hyperledger.iroha.sdk", "module": module, "version": "1.2.3"}, "variants": variants}))
        return path

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

    def local_directory(self):
        path = self.repo / MODULE.LOCAL_INTEGRATION_DIRECTORY
        path.mkdir(parents=True, mode=0o700)
        return path

    def git_inventory(self, *, tracked=b"", ignored=True):
        def run(command, **kwargs):
            self.assertEqual(command[0], "/usr/bin/git")
            self.assertEqual(kwargs["timeout"], 30)
            self.assertEqual(kwargs["env"]["GIT_CONFIG_GLOBAL"], os.devnull)
            self.assertNotIn("GIT_INDEX_FILE", kwargs["env"])
            return subprocess.CompletedProcess(
                command, 0 if "ls-files" in command or ignored else 1,
                tracked if "ls-files" in command else b"", b"",
            )
        return mock.patch.object(MODULE.subprocess, "run", side_effect=run)

    def test_explicit_local_root_is_exact_owned_ignored_and_not_release_output(self):
        local = self.local_directory()
        with self.git_inventory(), mock.patch.dict(os.environ, {"GIT_INDEX_FILE": "foreign"}):
            self.assertEqual(MODULE.local_integration_directory(self.repo, str(local)), local)
            self.assertEqual(MODULE.build_root(self.repo, str(local), local_integration=True),
                             local / "gradle-build/iroha_kotlin_sdk")
        with self.assertRaisesRegex(ValueError, "outside"):
            MODULE.build_root(self.repo, str(local))
        with self.assertRaisesRegex(ValueError, "explicit"):
            MODULE.build_root(self.repo, None, local_integration=True)

    def test_local_root_rejects_wrong_path_permissions_owner_and_symbolic_alias(self):
        local = self.local_directory()
        with self.assertRaisesRegex(ValueError, "fixed"):
            MODULE.local_integration_directory(self.repo, str(self.external))
        local.chmod(0o750)
        with self.assertRaisesRegex(ValueError, "owned.*0700"):
            MODULE.local_integration_directory(self.repo, str(local))
        local.chmod(0o700)
        with mock.patch.object(MODULE.os, "geteuid", return_value=local.stat().st_uid + 1):
            with self.assertRaisesRegex(ValueError, "owned.*0700"):
                MODULE.local_integration_directory(self.repo, str(local))
        alias = self.directory / "local-alias"
        alias.symlink_to(local, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "canonical"):
            MODULE.local_integration_directory(self.repo, str(alias))

    def test_local_root_rejects_forced_tracked_outputs_and_unignored_directory(self):
        local = self.local_directory()
        for tracked, ignored in [(b"dist/norito-bridge-android-local/source.rs\0", True), (b"", False)]:
            with self.subTest(tracked=tracked, ignored=ignored), self.git_inventory(tracked=tracked, ignored=ignored):
                with self.assertRaisesRegex(ValueError, "ignored.*tracked"):
                    MODULE.local_integration_directory(self.repo, str(local))

    def test_local_artifact_selection_never_falls_back_or_accepts_linked_bytes(self):
        local = self.local_directory()
        self.outputs(external=False)
        with self.git_inventory(), self.assertRaisesRegex(ValueError, "exactly one"):
            MODULE.built_artifacts(self.repo, str(local), local_integration=True)
        self.build = local / "gradle-build/iroha_kotlin_sdk"
        expected = self.outputs()
        with self.git_inventory():
            self.assertEqual(MODULE.built_artifacts(self.repo, str(local), local_integration=True), expected)
        expected[0].with_name("core-jvm-other.jar").write_bytes(b"ambiguous")
        with self.git_inventory(), self.assertRaisesRegex(ValueError, "exactly one"):
            MODULE.built_artifacts(self.repo, str(local), local_integration=True)
        expected[0].with_name("core-jvm-other.jar").unlink()
        target = self.directory / "other.jar"
        expected[0].rename(target)
        expected[0].symlink_to(target)
        with self.git_inventory(), self.assertRaisesRegex(ValueError, "symbolic"):
            MODULE.built_artifacts(self.repo, str(local), local_integration=True)
        expected[0].unlink()
        os.link(target, expected[0])
        with self.git_inventory(), self.assertRaisesRegex(ValueError, "single-link"):
            MODULE.built_artifacts(self.repo, str(local), local_integration=True)

    def test_local_scope_cannot_collect_release_sboms(self):
        local = self.local_directory()
        result = subprocess.run([sys.executable, "-I", "-B", str(OWNER), "--root", str(self.repo),
                                 "--artifact-dir", str(local), "--local-integration",
                                 "--collect-sboms", str(self.directory / "collected")], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("cannot enter release SBOM", result.stderr)
        self.assertFalse((self.directory / "collected").exists())

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
        jar, _, _ = self.outputs()
        jar.with_name("core-jvm-old.jar").write_bytes(b"old")
        with self.assertRaises(ValueError):
            MODULE.built_artifacts(self.repo, str(self.external))

    def test_source_and_javadoc_jars_do_not_replace_runtime_jar(self):
        jar, aar, wallet = self.outputs()
        for suffix in ("sources", "javadoc"):
            jar.with_name(f"core-jvm-1.2.3-{suffix}.jar").write_bytes(b"documentation")
        self.assertEqual(MODULE.built_artifacts(self.repo, str(self.external)), (jar, aar, wallet))

    def test_linked_or_empty_artifact_is_rejected(self):
        jar, aar, wallet = self.outputs()
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

    def test_missing_wallet_and_version_substitution_refuse(self):
        outputs = self.outputs()
        outputs[2].unlink()
        with self.assertRaises(FileNotFoundError):
            MODULE.built_artifacts(self.repo, str(self.external))
        self.outputs()
        with self.assertRaisesRegex(ValueError, "requested SDK version"):
            MODULE.built_artifacts(self.repo, str(self.external), version="1.2.4")
        pom = self.build / "kagemusha-wallet-android/publications/release/pom-default.xml"
        pom.write_text(self.pom("kagemusha-wallet-android", "1.2.4"))
        with self.assertRaisesRegex(ValueError, "POM version"):
            MODULE.built_artifacts(self.repo, str(self.external))

    def test_pom_dependency_version_and_retired_identity_cannot_mix_graph(self):
        self.outputs()
        pom = self.build / "client-android/publications/release/pom-default.xml"
        original = pom.read_text()
        for content in [original.replace('<version>1.2.3</version></dependency>', '<version>old</version></dependency>'),
                        original.replace('<artifactId>core-jvm</artifactId>', '<artifactId>iroha-android</artifactId>'),
                        original.replace('<artifactId>core-jvm</artifactId>', '<artifactId>client-android</artifactId>')]:
            with self.subTest(content=content):
                pom.write_text(content)
                with self.assertRaises(ValueError):
                    MODULE.built_artifacts(self.repo, str(self.external))
        pom.write_text(original)
        self.assertEqual(len(MODULE.built_artifacts(self.repo, str(self.external))), 3)

    def test_complete_maven_graph_binds_same_version_and_original_build_bytes(self):
        maven = self.maven()
        admitted = MODULE.maven_artifacts(self.repo, str(self.external), maven, "1.2.3")
        self.assertEqual(len(admitted), 9)
        wallet = maven / "org/hyperledger/iroha/sdk/kagemusha-wallet-android/1.2.3/kagemusha-wallet-android-1.2.3.aar"
        wallet.write_bytes(b"stale Maven wallet")
        with self.assertRaisesRegex(ValueError, "differs"):
            MODULE.maven_artifacts(self.repo, str(self.external), maven, "1.2.3")
        wallet.unlink()
        with self.assertRaises(FileNotFoundError):
            MODULE.maven_artifacts(self.repo, str(self.external), maven, "1.2.3")

    def test_maven_metadata_cannot_override_exact_version_or_add_old_coordinate(self):
        maven = self.maven()
        directory = maven / "org/hyperledger/iroha/sdk/client-android/1.2.3"
        metadata = directory / "client-android-1.2.3.module"
        document = json.loads(metadata.read_text())
        document["variants"][0]["dependencies"] = [{"group": "org.hyperledger.iroha.sdk", "module": "core-jvm", "version": {"requires": "old"}}]
        metadata.write_text(json.dumps(document))
        with self.assertRaisesRegex(ValueError, "exactly the same"):
            MODULE.maven_artifacts(self.repo, str(self.external), maven, "1.2.3")
        document["variants"][0]["dependencies"] = [{"group": "org.hyperledger.iroha.sdk", "module": "core-jvm", "version": {"requires": "1.2.3"}}]
        metadata.write_text(json.dumps(document))
        old = maven / "org/hyperledger/iroha/sdk/core-jvm/old/core-jvm-old.jar"
        old.parent.mkdir()
        old.write_bytes(b"older version")
        with self.assertRaisesRegex(ValueError, "another SDK version"):
            MODULE.maven_artifacts(self.repo, str(self.external), maven, "1.2.3")

    def test_module_omission_duplicate_extra_and_old_dependency_refuse(self):
        maven = self.maven()
        for module, dependency in [("client-android", "core-jvm"), ("kagemusha-wallet-android", "client-android")]:
            path = maven / "org/hyperledger/iroha/sdk" / module / "1.2.3" / f"{module}-1.2.3.module"
            original = json.loads(path.read_text())
            exact = original["variants"][0]["dependencies"][0]
            for dependencies in ([], [exact, exact], [exact, {**exact, "module": module}],
                                 [{**exact, "version": {"requires": "old"}}]):
                changed = json.loads(json.dumps(original))
                changed["variants"][0]["dependencies"] = dependencies
                path.write_text(json.dumps(changed))
                with self.subTest(module=module, dependencies=dependencies), self.assertRaises(ValueError):
                    MODULE.maven_artifacts(self.repo, str(self.external), maven, "1.2.3")
            path.write_text(json.dumps(original))
        self.assertEqual(len(MODULE.maven_artifacts(self.repo, str(self.external), maven, "1.2.3")), 9)

    def test_module_runtime_redirect_size_and_checksum_substitution_refuse(self):
        maven = self.maven()
        path = maven / "org/hyperledger/iroha/sdk/client-android/1.2.3/client-android-1.2.3.module"
        original = json.loads(path.read_text())
        for key, value in [("url", "https://foreign.invalid/client.aar"), ("name", "foreign.aar"),
                           ("size", 1), ("sha256", "0" * 64), ("sha512", "0" * 128)]:
            changed = json.loads(json.dumps(original))
            changed["variants"][1]["files"][0][key] = value
            path.write_text(json.dumps(changed))
            with self.subTest(key=key), self.assertRaises(ValueError):
                MODULE.maven_artifacts(self.repo, str(self.external), maven, "1.2.3")
        changed = json.loads(json.dumps(original))
        changed["variants"] = changed["variants"][:1]
        path.write_text(json.dumps(changed))
        with self.assertRaisesRegex(ValueError, "missing a canonical"):
            MODULE.maven_artifacts(self.repo, str(self.external), maven, "1.2.3")

    def test_module_preserves_real_sources_variant_with_original_file_binding(self):
        maven = self.maven()
        path = maven / "org/hyperledger/iroha/sdk/client-android/1.2.3/client-android-1.2.3.module"
        source = path.parent / "client-android-1.2.3-sources.jar"
        source.write_bytes(b"source original")
        document = json.loads(path.read_text())
        document["variants"].append({"name": "releaseVariantReleaseSourcePublication",
            "attributes": {"org.gradle.category": "documentation", "org.gradle.docstype": "sources",
                           "org.gradle.dependency.bundling": "external", "org.gradle.usage": "java-runtime"},
            "files": [{"name": source.name, "url": source.name, "size": source.stat().st_size,
                       "sha256": hashlib.sha256(source.read_bytes()).hexdigest()}]})
        path.write_text(json.dumps(document))
        self.assertEqual(len(MODULE.maven_artifacts(self.repo, str(self.external), maven, "1.2.3")), 9)

    def test_duplicate_version_or_entity_pom_is_refused(self):
        self.outputs()
        pom = self.build / "core-jvm/publications/release/pom-default.xml"
        for content in [self.pom("core-jvm").replace('<version>1.2.3</version>', '<version>1.2.3</version><version>old</version>'),
                        '<!DOCTYPE project [<!ENTITY x "1.2.3">]>' + self.pom("core-jvm")]:
            pom.write_text(content)
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
