"""Execute the std-only fixture build script against sealed source without Cargo."""

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
BUILD = ROOT / "crates/iroha_test_samples/build.rs"
MANIFEST = ROOT / "crates/ivm/prebuilt_samples.txt"
NAMES = [line.strip() for line in MANIFEST.read_text().splitlines()
         if line.strip() and not line.lstrip().startswith("#")]


def source_inventory(root):
    return {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in root.rglob("*") if path.is_file()}


def frozen_source(parent, missing=None):
    root = parent / "source"
    files = {"crates/iroha_test_samples/build.rs": BUILD,
             "crates/ivm/prebuilt_samples.txt": MANIFEST}
    files.update({f"integration_tests/fixtures/ivm/{name}.to":
                  ROOT / f"integration_tests/fixtures/ivm/{name}.to"
                  for name in NAMES if name != missing})
    for relative, original in files.items():
        destination = root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(original.read_bytes())
        destination.chmod(0o400)
    for directory in sorted((path for path in root.rglob("*") if path.is_dir()),
                            key=lambda path: len(path.parts), reverse=True):
        directory.chmod(0o500)
    root.chmod(0o500)
    return root


def thaw(root):
    root.chmod(0o700)
    for directory, children, files in os.walk(root):
        for name in children:
            (Path(directory) / name).chmod(0o700)
        for name in files:
            (Path(directory) / name).chmod(0o600)


class IvmFixtureStagingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.directory = tempfile.TemporaryDirectory(prefix="iroha-fixture-build-")
        cls.root = Path(cls.directory.name)
        cls.source = frozen_source(cls.root)
        cls.binary = cls.root / "fixture-build"
        compiler = shutil.which("rustc")
        if compiler is None:
            raise RuntimeError("fixture staging regression requires the repository Rust compiler")
        subprocess.run([compiler, "--edition=2024",
                        str(cls.source / "crates/iroha_test_samples/build.rs"),
                        "-o", str(cls.binary)], cwd=ROOT, check=True,
                       stdin=subprocess.DEVNULL, capture_output=True, timeout=60)

    @classmethod
    def tearDownClass(cls):
        thaw(cls.root)
        cls.directory.cleanup()

    def stage(self, source, output, profile="debug", *, default_executor=False):
        environment = {"CARGO_MANIFEST_DIR": str(source / "crates/iroha_test_samples"),
                       "OUT_DIR": str(output), "PROFILE": profile}
        if default_executor:
            environment["IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR"] = "1"
        return subprocess.run([str(self.binary)], cwd=source, env=environment,
                              stdin=subprocess.DEVNULL, capture_output=True,
                              text=True, timeout=10)

    def test_readonly_source_stages_exact_samples_and_separate_profiles_only_in_out_dir(self):
        original = source_inventory(self.source)
        for profile, expected in (("debug", "Debug"), ("release", "Release")):
            with self.subTest(profile=profile):
                output = self.root / (profile + "-output")
                result = self.stage(self.source, output, profile)
                self.assertEqual(result.returncode, 0, result.stderr)
                expected_files = {"ivm/build_config.toml"} | {f"ivm/samples/{name}.to" for name in NAMES}
                self.assertEqual(set(source_inventory(output)), expected_files)
                self.assertEqual((output / "ivm/build_config.toml").read_text(),
                                 f'profile = "{expected}"\n')
                for name in NAMES:
                    source = self.source / f"integration_tests/fixtures/ivm/{name}.to"
                    self.assertEqual((output / f"ivm/samples/{name}.to").read_bytes(), source.read_bytes())
                    self.assertIn(f"cargo:rerun-if-changed={source}", result.stdout)
                times = {path: path.stat().st_mtime_ns for path in output.rglob("*") if path.is_file()}
                self.assertEqual(self.stage(self.source, output, profile).returncode, 0)
                self.assertEqual(times, {path: path.stat().st_mtime_ns for path in times})
                self.assertEqual(source_inventory(self.source), original)
                self.assertFalse((self.source / "crates/ivm/target").exists())

    def test_missing_canonical_sample_fails_without_source_write_or_placeholder(self):
        parent = self.root / "missing-case"
        parent.mkdir()
        source = frozen_source(parent, missing=NAMES[0])
        original = source_inventory(source)
        output = parent / "output"
        result = self.stage(source, output)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing canonical fixture", result.stderr)
        self.assertFalse((output / f"ivm/samples/{NAMES[0]}.to").exists())
        self.assertEqual(source_inventory(source), original)

    def test_explicit_unavailable_executor_remains_a_missing_fixture_error(self):
        original = source_inventory(self.source)
        output = self.root / "executor-output"
        result = self.stage(self.source, output, default_executor=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("default_executor.to", result.stderr)
        self.assertFalse((output / "ivm/samples/default_executor.to").exists())
        self.assertEqual(source_inventory(self.source), original)


if __name__ == "__main__":
    unittest.main()
