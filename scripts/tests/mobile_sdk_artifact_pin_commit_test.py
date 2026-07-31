from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).parents[1] / "check_mobile_sdk_artifact_pin_commit.py"
SPEC = importlib.util.spec_from_file_location("mobile_sdk_artifact_pin_commit", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
pin = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = pin
SPEC.loader.exec_module(pin)


def loader(first: str = "1", *, extra: str = "") -> str:
    return (
        "enum Loader {\n"
        "    static let expectedHashes = [\n"
        f'        "macos-arm64_x86_64": "{first * 64}",\n'
        f'        "ios-arm64": "{first * 64}",\n'
        f'        "ios-arm64_x86_64-simulator": "{first * 64}"\n'
        "    ]\n"
        f"{extra}"
        "}\n"
    )


class MobileSdkArtifactPinCommitTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.loader = self.root / pin.LOADER_PATH
        self.loader.parent.mkdir(parents=True)
        self.loader.write_text(loader(), encoding="utf-8")
        self.git("init", "-q")
        self.git("config", "user.name", "Artifact Pin Test")
        self.git("config", "user.email", "artifact-pin@example.invalid")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "artifact source")
        self.source_commit = self.head()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def git(self, *arguments: str) -> bytes:
        environment = os.environ.copy()
        environment["GIT_CONFIG_GLOBAL"] = os.devnull
        environment["GIT_CONFIG_NOSYSTEM"] = "1"
        return subprocess.run(
            ["git", "-C", str(self.root), *arguments],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
        ).stdout

    def head(self) -> str:
        return self.git("rev-parse", "HEAD").decode("ascii").strip()

    def commit_changes(self, message: str = "pin hashes") -> str:
        self.git("add", "-A")
        self.git("commit", "-q", "-m", message)
        return self.head()

    def test_accepts_manifest_built_from_head(self) -> None:
        self.assertEqual(
            pin.validate_pin_relationship(self.root, self.source_commit),
            "direct",
        )

    def test_accepts_exact_parent_when_only_three_hash_literals_change(self) -> None:
        self.loader.write_text(loader("a"), encoding="utf-8")
        self.commit_changes()
        self.assertEqual(
            pin.validate_pin_relationship(self.root, self.source_commit),
            "pin-parent",
        )

    def test_rejects_loader_logic_change_in_pin_commit(self) -> None:
        self.loader.write_text(loader("a", extra="    static let bypass = true\n"), encoding="utf-8")
        self.commit_changes()
        with self.assertRaisesRegex(
            pin.PinCommitError,
            "beyond the three fallback digests",
        ):
            pin.validate_pin_relationship(self.root, self.source_commit)

    def test_rejects_any_non_allowlisted_path(self) -> None:
        self.loader.write_text(loader("a"), encoding="utf-8")
        (self.root / "README.md").write_text("unrelated\n", encoding="utf-8")
        self.commit_changes()
        with self.assertRaisesRegex(pin.PinCommitError, "non-artifact source paths"):
            pin.validate_pin_relationship(self.root, self.source_commit)

    def test_rejects_grandparent_even_when_both_children_are_pin_shaped(self) -> None:
        self.loader.write_text(loader("a"), encoding="utf-8")
        self.commit_changes("first pin")
        self.loader.write_text(loader("b"), encoding="utf-8")
        self.commit_changes("second pin")
        with self.assertRaisesRegex(pin.PinCommitError, "exact parent"):
            pin.validate_pin_relationship(self.root, self.source_commit)

    def test_accepts_builder_private_prospective_loader_with_hash_changes_only(
        self,
    ) -> None:
        artifact_root = self.root / ".NoritoBridge.publish.fixture"
        artifact_root.mkdir()
        prospective = artifact_root / pin.PROSPECTIVE_LOADER_NAME
        prospective.write_text(loader("a"), encoding="utf-8")
        self.assertEqual(
            pin.validate_prospective_loader(
                self.root,
                self.source_commit,
                artifact_root,
                prospective,
            ),
            "prospective",
        )

    def test_rejects_prospective_loader_logic_change(self) -> None:
        artifact_root = self.root / ".NoritoBridge.publish.fixture"
        artifact_root.mkdir()
        prospective = artifact_root / pin.PROSPECTIVE_LOADER_NAME
        prospective.write_text(
            loader("a", extra="    static let bypass = true\n"),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            pin.PinCommitError,
            "beyond the three fallback digests",
        ):
            pin.validate_prospective_loader(
                self.root,
                self.source_commit,
                artifact_root,
                prospective,
            )


if __name__ == "__main__":
    unittest.main()
