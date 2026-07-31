from __future__ import annotations

import hashlib
import importlib.util
import json
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
        self.review_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.review_root = Path(self.review_directory.name).resolve(strict=True)
        self.git("init", "-q")
        self.git("config", "user.name", "Kagemusha Test")
        self.git("config", "user.email", "kagemusha@example.invalid")
        (self.root / ".gitignore").write_text("/Cargo.lock\n", encoding="utf-8")
        (self.root / "Cargo.lock").write_text(
            "# fixture lockfile consumed by --locked\n", encoding="utf-8"
        )

    def tearDown(self) -> None:
        self.review_directory.cleanup()
        self.temporary.cleanup()

    def git(self, *arguments: str) -> bytes:
        environment = os.environ.copy()
        environment["GIT_CONFIG_GLOBAL"] = os.devnull
        environment["GIT_CONFIG_NOSYSTEM"] = "1"
        for name in tuple(environment):
            if name == "GIT_CONFIG_COUNT" or name.startswith(
                ("GIT_CONFIG_KEY_", "GIT_CONFIG_VALUE_")
            ):
                del environment[name]
        return subprocess.run(
            ["git", "-C", str(self.root), *arguments],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
        ).stdout

    def commit(self) -> None:
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "fixture")

    def reviewed_identity(self) -> tuple[object, dict[str, object], Path, str]:
        descriptor = seal.compute_observed_descriptor(self.root)
        payload = seal._canonical_json_bytes(descriptor)
        path = self.review_root / "reviewed-source-closure.json"
        path.write_bytes(payload)
        sha256 = hashlib.sha256(payload).hexdigest()
        identity = seal.compute_identity(self.root, str(path), sha256)
        return identity, descriptor, path, sha256

    def test_reviewed_dirty_fingerprint_is_stable_and_binds_complete_tree(self) -> None:
        (self.root / "plain.txt").write_bytes(b"plain\n")
        executable = self.root / "tool.sh"
        executable.write_bytes(b"#!/bin/sh\nexit 0\n")
        executable.chmod(executable.stat().st_mode | stat.S_IXUSR)
        os.symlink("plain.txt", self.root / "plain-link")
        self.commit()
        (self.root / "plain.txt").write_bytes(b"reviewed change\n")
        (self.root / "new.txt").write_bytes(b"reviewed untracked\n")

        identity, descriptor, path, pin = self.reviewed_identity()
        first = seal.compute_fingerprint(self.root, str(path), pin)
        self.assertEqual(first, seal.compute_fingerprint(self.root, str(path), pin))
        self.assertRegex(first, r"^[0-9a-f]{64}$")
        self.assertEqual(identity.source_tree_sha256, first)
        self.assertTrue(identity.source_repo_dirty)
        self.assertEqual(identity.reviewed_source_closure, descriptor)
        self.assertEqual(
            identity.source_commit,
            self.git("rev-parse", "--verify", "HEAD^{commit}").decode().strip(),
        )

        (self.root / "plain.txt").write_bytes(b"unreviewed change\n")
        with self.assertRaisesRegex(seal.SourceSealError, "differs"):
            seal.compute_fingerprint(self.root, str(path), pin)

    def test_clean_tree_has_no_dirty_review_bypass(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        descriptor = seal.compute_observed_descriptor(self.root)
        self.assertFalse(descriptor["source_repo_dirty"])
        payload = seal._canonical_json_bytes(descriptor)
        path = self.review_root / "clean.json"
        path.write_bytes(payload)
        with self.assertRaisesRegex(seal.SourceSealError, "must be nonempty"):
            seal.compute_identity(
                self.root,
                str(path),
                hashlib.sha256(payload).hexdigest(),
            )

    def test_descriptor_pin_and_canonical_bytes_are_mandatory(self) -> None:
        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.commit()
        (self.root / "tracked.txt").write_text("reviewed\n", encoding="utf-8")
        _, descriptor, path, pin = self.reviewed_identity()

        with self.assertRaisesRegex(seal.SourceSealError, "differs from its pin"):
            seal.compute_identity(self.root, str(path), "f" * 64)

        path.write_text(json.dumps(descriptor, indent=2) + "\n", encoding="ascii")
        pretty_pin = hashlib.sha256(path.read_bytes()).hexdigest()
        with self.assertRaisesRegex(seal.SourceSealError, "not canonical"):
            seal.compute_identity(self.root, str(path), pretty_pin)

        path.write_bytes(seal._canonical_json_bytes(descriptor))
        self.assertEqual(
            seal.compute_identity(self.root, str(path), pin).source_repo_dirty,
            True,
        )
        with self.assertRaisesRegex(seal.SourceSealError, "absolute and normalized"):
            seal.compute_identity(self.root, "-", pin)

    def test_staged_and_untracked_bytes_are_accepted_only_when_reviewed(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        (self.root / "staged.txt").write_text("staged\n", encoding="utf-8")
        self.git("add", "staged.txt")
        (self.root / "untracked.txt").write_text("untracked\n", encoding="utf-8")

        identity, descriptor, path, pin = self.reviewed_identity()
        self.assertTrue(identity.source_repo_dirty)
        self.assertEqual(descriptor["untracked_file_count"], 1)
        self.assertEqual(
            descriptor["untracked_path_mode_blob_oid_manifest"][0]["path"],
            "untracked.txt",
        )

        (self.root / "later.txt").write_text("not reviewed\n", encoding="utf-8")
        with self.assertRaisesRegex(seal.SourceSealError, "differs"):
            seal.compute_identity(self.root, str(path), pin)

    def test_source_tree_and_descriptor_bind_ignored_root_cargo_lock(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        (self.root / "tracked.txt").write_text("reviewed\n", encoding="utf-8")
        identity, descriptor, path, pin = self.reviewed_identity()

        (self.root / "Cargo.lock").write_text(
            "# changed ignored lockfile consumed by --locked\n", encoding="utf-8"
        )
        self.assertEqual(seal.status(self.root), b" M tracked.txt\0")
        with self.assertRaisesRegex(seal.SourceSealError, "differs"):
            seal.compute_identity(self.root, str(path), pin)
        replacement = seal.compute_observed_descriptor(self.root)
        self.assertNotEqual(
            identity.source_tree_sha256,
            replacement["source_tree_sha256"],
        )
        self.assertNotEqual(
            descriptor["ignored_cargo_lock_sha256"],
            replacement["ignored_cargo_lock_sha256"],
        )

    def test_rejects_missing_or_symlinked_root_cargo_lock(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        (self.root / "tracked.txt").write_text("reviewed\n", encoding="utf-8")
        (self.root / "Cargo.lock").unlink()
        with self.assertRaisesRegex(seal.SourceSealError, "ignored source set"):
            seal.compute_observed_descriptor(self.root)

        os.symlink("tracked.txt", self.root / "Cargo.lock")
        with self.assertRaisesRegex(
            seal.SourceSealError, "regular file|must not be executable"
        ):
            seal.compute_observed_descriptor(self.root)

    def test_rejects_any_additional_ignored_source(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        (self.root / ".gitignore").write_text(
            "/Cargo.lock\n/hidden-input.bin\n", encoding="utf-8"
        )
        self.commit()
        (self.root / "tracked.txt").write_text("reviewed\n", encoding="utf-8")
        (self.root / "hidden-input.bin").write_bytes(b"unbound ignored input\n")
        with self.assertRaisesRegex(seal.SourceSealError, "ignored source set"):
            seal.compute_observed_descriptor(self.root)

    def test_rejects_hardlinked_tracked_source(self) -> None:
        first = self.root / "first.txt"
        first.write_bytes(b"same inode\n")
        os.link(first, self.root / "second.txt")
        self.commit()
        first.write_bytes(b"reviewed dirty inode\n")
        with self.assertRaisesRegex(seal.SourceSealError, "singly linked"):
            seal.compute_observed_descriptor(self.root)

    def test_gitlink_binds_index_commit_and_requires_empty_directory(self) -> None:
        (self.root / "tracked.txt").write_text("first\n", encoding="utf-8")
        self.commit()
        first_commit = self.git("rev-parse", "HEAD").decode().strip()
        (self.root / "tracked.txt").write_text("second\n", encoding="utf-8")
        self.commit()
        second_commit = self.git("rev-parse", "HEAD").decode().strip()
        self.git(
            "update-index",
            "--add",
            "--cacheinfo",
            f"160000,{first_commit},iroha-docs",
        )

        gitlink = self.root / "iroha-docs"
        gitlink.mkdir()
        empty_directory = seal.compute_observed_descriptor(self.root)
        gitlink.rmdir()
        absent = seal.compute_observed_descriptor(self.root)
        self.assertEqual(
            empty_directory["source_tree_sha256"],
            absent["source_tree_sha256"],
        )

        self.git(
            "update-index",
            "--cacheinfo",
            f"160000,{second_commit},iroha-docs",
        )
        changed_commit = seal.compute_observed_descriptor(self.root)
        self.assertNotEqual(
            absent["source_tree_sha256"],
            changed_commit["source_tree_sha256"],
        )

        gitlink.mkdir()
        (gitlink / "unbound.txt").write_text("unbound\n", encoding="utf-8")
        with self.assertRaisesRegex(seal.SourceSealError, "must be empty"):
            seal.compute_observed_descriptor(self.root)

    def test_gitlink_rejects_non_directory_substitutions(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        commit = self.git("rev-parse", "HEAD").decode().strip()
        self.git(
            "update-index",
            "--add",
            "--cacheinfo",
            f"160000,{commit},iroha-docs",
        )
        gitlink = self.root / "iroha-docs"

        gitlink.write_text("not a directory\n", encoding="utf-8")
        with self.assertRaisesRegex(seal.SourceSealError, "empty directory"):
            seal.compute_observed_descriptor(self.root)
        gitlink.unlink()

        os.symlink("tracked.txt", gitlink)
        with self.assertRaisesRegex(
            seal.SourceSealError, "empty directory|pinned Git failed"
        ):
            seal.compute_observed_descriptor(self.root)

    def test_root_must_be_exact_repository_root(self) -> None:
        (self.root / "nested").mkdir()
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        with self.assertRaisesRegex(seal.SourceSealError, "exact repository root"):
            seal.compute_observed_descriptor(self.root / "nested")

    def test_identity_rejects_head_change_during_capture(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        (self.root / "tracked.txt").write_text("reviewed\n", encoding="utf-8")
        original = seal._head(self.root)
        changed = b"f" * 40 if original != b"f" * 40 else b"e" * 40
        with mock.patch.object(seal, "_head", side_effect=[original, changed]):
            with self.assertRaisesRegex(seal.SourceSealError, "changed while sealing"):
                seal.compute_observed_descriptor(self.root)


if __name__ == "__main__":
    unittest.main()
