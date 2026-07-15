from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).parents[1] / "kagemusha_source_tree_seal.py"
SPEC = importlib.util.spec_from_file_location("kagemusha_source_tree_seal", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
seal = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = seal
SPEC.loader.exec_module(seal)


class KagemushaSourceTreeSealTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.git("init", "-q")
        self.git("config", "user.name", "Kagemusha Test")
        self.git("config", "user.email", "kagemusha@example.invalid")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def git(self, *arguments: str) -> bytes:
        return subprocess.run(
            ["git", "-C", str(self.root), *arguments],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        ).stdout

    def commit(self) -> None:
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "fixture")

    def test_fingerprint_is_stable_and_binds_path_mode_content_and_symlink(self) -> None:
        (self.root / "plain.txt").write_bytes(b"plain\n")
        executable = self.root / "tool.sh"
        executable.write_bytes(b"#!/bin/sh\nexit 0\n")
        executable.chmod(executable.stat().st_mode | stat.S_IXUSR)
        os.symlink("plain.txt", self.root / "plain-link")
        self.commit()

        first = seal.compute_fingerprint(self.root)
        self.assertEqual(first, seal.compute_fingerprint(self.root))
        self.assertRegex(first, r"^[0-9a-f]{64}$")
        identity = seal.compute_identity(self.root)
        self.assertEqual(identity.source_tree_sha256, first)
        self.assertEqual(
            identity.source_commit,
            self.git("rev-parse", "--verify", "HEAD^{commit}").decode().strip(),
        )

        (self.root / "plain.txt").write_bytes(b"changed\n")
        with self.assertRaisesRegex(seal.SourceSealError, "must be clean"):
            seal.compute_fingerprint(self.root)
        self.git("add", "plain.txt")
        self.git("commit", "-q", "-m", "content")
        self.assertNotEqual(first, seal.compute_fingerprint(self.root))

    def test_rejects_untracked_and_staged_files(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        (self.root / "extra.txt").write_text("extra\n", encoding="utf-8")
        with self.assertRaisesRegex(seal.SourceSealError, "must be clean"):
            seal.compute_fingerprint(self.root)
        self.git("add", "extra.txt")
        with self.assertRaisesRegex(seal.SourceSealError, "must be clean"):
            seal.compute_fingerprint(self.root)

    def test_rejects_hardlinked_tracked_source(self) -> None:
        first = self.root / "first.txt"
        first.write_bytes(b"same inode\n")
        os.link(first, self.root / "second.txt")
        self.commit()
        with self.assertRaisesRegex(seal.SourceSealError, "singly linked"):
            seal.compute_fingerprint(self.root)

    def test_root_must_be_exact_repository_root(self) -> None:
        (self.root / "nested").mkdir()
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        with self.assertRaisesRegex(seal.SourceSealError, "exact repository root"):
            seal.compute_fingerprint(self.root / "nested")

    def test_identity_rejects_head_change_during_seal(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        original = seal._head(self.root)
        changed = b"f" * 40 if original != b"f" * 40 else b"e" * 40
        with mock.patch.object(seal, "_head", side_effect=[original, changed]):
            with self.assertRaisesRegex(seal.SourceSealError, "changed while sealing"):
                seal.compute_identity(self.root)


if __name__ == "__main__":
    unittest.main()
