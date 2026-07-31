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
        self.git("add", "-f", "Cargo.lock")

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

    def test_clean_tree_descriptor_is_reproducible_and_reviewable(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        identity, descriptor, path, pin = self.reviewed_identity()

        self.assertFalse(descriptor["source_repo_dirty"])
        self.assertFalse(identity.source_repo_dirty)
        self.assertEqual(descriptor, seal.compute_observed_descriptor(self.root))
        self.assertEqual(
            seal._canonical_json_bytes(descriptor),
            seal._canonical_json_bytes(seal.compute_observed_descriptor(self.root)),
        )
        self.assertEqual(
            identity.source_tree_sha256,
            seal.compute_fingerprint(self.root, str(path), pin),
        )
        self.assertEqual(
            descriptor["ignored_cargo_lock_sha256"],
            hashlib.sha256((self.root / "Cargo.lock").read_bytes()).hexdigest(),
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

    def test_source_tree_and_descriptor_bind_tracked_root_cargo_lock(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        (self.root / "tracked.txt").write_text("reviewed\n", encoding="utf-8")
        identity, descriptor, path, pin = self.reviewed_identity()

        (self.root / "Cargo.lock").write_text(
            "# changed tracked lockfile consumed by --locked\n", encoding="utf-8"
        )
        observed_status = seal.status(self.root).split(b"\0")
        self.assertIn(b" M Cargo.lock", observed_status)
        self.assertIn(b" M tracked.txt", observed_status)
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

    def test_rejects_missing_or_symlinked_tracked_root_cargo_lock(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        (self.root / "tracked.txt").write_text("reviewed\n", encoding="utf-8")
        (self.root / "Cargo.lock").unlink()
        with self.assertRaisesRegex(seal.SourceSealError, "Cargo.lock is missing"):
            seal.compute_observed_descriptor(self.root)

        os.symlink("tracked.txt", self.root / "Cargo.lock")
        with self.assertRaisesRegex(
            seal.SourceSealError, "singly linked regular file"
        ):
            seal.compute_observed_descriptor(self.root)

    def test_legacy_sole_ignored_root_cargo_lock_is_conservatively_dirty(self) -> None:
        self.git("rm", "--cached", "-q", "Cargo.lock")
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        identity, descriptor, path, pin = self.reviewed_identity()

        self.assertTrue(identity.source_repo_dirty)
        self.assertTrue(descriptor["source_repo_dirty"])
        self.assertEqual(seal.status(self.root), b"")
        self.assertEqual(
            identity.source_tree_sha256,
            seal.compute_fingerprint(self.root, str(path), pin),
        )

    def test_rejects_any_additional_ignored_source(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        (self.root / ".gitignore").write_text(
            "/Cargo.lock\n/hidden-input.bin\n", encoding="utf-8"
        )
        self.commit()
        (self.root / "tracked.txt").write_text("reviewed\n", encoding="utf-8")
        (self.root / "hidden-input.bin").write_bytes(b"unbound ignored input\n")
        with self.assertRaisesRegex(seal.SourceSealError, "sole ignored path"):
            seal.compute_observed_descriptor(self.root)

    def test_rejects_hardlinked_tracked_source(self) -> None:
        first = self.root / "first.txt"
        first.write_bytes(b"same inode\n")
        os.link(first, self.root / "second.txt")
        self.commit()
        first.write_bytes(b"reviewed dirty inode\n")
        with self.assertRaisesRegex(seal.SourceSealError, "singly linked"):
            seal.compute_observed_descriptor(self.root)

    def test_rejects_assume_unchanged_tracked_source(self) -> None:
        tracked = self.root / "tracked.txt"
        tracked.write_text("tracked\n", encoding="utf-8")
        self.commit()
        self.git("update-index", "--assume-unchanged", "tracked.txt")
        tracked.write_text("hidden working-tree change\n", encoding="utf-8")

        self.assertEqual(self.git("diff", "--", "tracked.txt"), b"")
        self.assertEqual(seal.status(self.root), b"")
        with self.assertRaisesRegex(seal.SourceSealError, "assume-unchanged"):
            seal.compute_observed_descriptor(self.root)

    def test_rejects_skip_worktree_tracked_source(self) -> None:
        tracked = self.root / "tracked.txt"
        tracked.write_text("tracked\n", encoding="utf-8")
        self.commit()
        self.git("update-index", "--skip-worktree", "tracked.txt")
        tracked.write_text("hidden working-tree change\n", encoding="utf-8")

        self.assertEqual(self.git("diff", "--", "tracked.txt"), b"")
        self.assertEqual(seal.status(self.root), b"")
        with self.assertRaisesRegex(seal.SourceSealError, "skip-worktree"):
            seal.compute_observed_descriptor(self.root)

    def test_raw_bytes_reject_local_clean_filter_diff_bypass(self) -> None:
        tracked = self.root / "tracked.txt"
        tracked.write_text("tracked\n", encoding="utf-8")
        self.commit()
        info_attributes = self.root / ".git" / "info" / "attributes"
        info_attributes.write_text("tracked.txt filter=hide\n", encoding="utf-8")
        self.git("config", "filter.hide.clean", "sed s/modified/tracked/")
        tracked.write_text("modified\n", encoding="utf-8")

        self.assertEqual(self.git("diff", "--", "tracked.txt"), b"")
        with self.assertRaisesRegex(
            seal.SourceSealError,
            "content-conversion attribute",
        ):
            seal.compute_observed_descriptor(self.root)

    def test_rejects_partial_clean_filter_with_another_visible_change(self) -> None:
        hidden = self.root / "hidden.txt"
        visible = self.root / "visible.txt"
        hidden.write_text("reviewed\n", encoding="utf-8")
        visible.write_text("reviewed\n", encoding="utf-8")
        self.commit()
        info_attributes = self.root / ".git" / "info" / "attributes"
        info_attributes.write_text("hidden.txt filter=hide\n", encoding="utf-8")
        self.git("config", "filter.hide.clean", "sed s/malicious/reviewed/")
        hidden.write_text("malicious\n", encoding="utf-8")
        visible.write_text("visible change\n", encoding="utf-8")

        diff = self.git("diff", "--", "hidden.txt", "visible.txt")
        self.assertNotIn(b"malicious", diff)
        self.assertIn(b"visible change", diff)
        with self.assertRaisesRegex(
            seal.SourceSealError,
            "content-conversion attribute",
        ):
            seal.compute_observed_descriptor(self.root)

    def test_rejects_transform_filter_even_when_changed_path_set_agrees(self) -> None:
        tracked = self.root / "tracked.txt"
        tracked.write_text("baseline\n", encoding="utf-8")
        self.commit()
        info_attributes = self.root / ".git" / "info" / "attributes"
        info_attributes.write_text("tracked.txt filter=hide\n", encoding="utf-8")
        self.git("config", "filter.hide.clean", "sed s/malicious/reviewed/")
        tracked.write_text("malicious\n", encoding="utf-8")

        diff = self.git("diff", "--", "tracked.txt")
        self.assertIn(b"reviewed", diff)
        self.assertNotIn(b"malicious", diff)
        with self.assertRaisesRegex(
            seal.SourceSealError,
            "content-conversion attribute",
        ):
            seal.compute_observed_descriptor(self.root)

    def test_rejects_legacy_crlf_content_conversion_attribute(self) -> None:
        tracked = self.root / "tracked.txt"
        tracked.write_bytes(b"reviewed line\n")
        self.commit()
        info_attributes = self.root / ".git" / "info" / "attributes"
        info_attributes.write_text("tracked.txt crlf\n", encoding="utf-8")
        tracked.write_bytes(b"reviewed line\r\n")

        with self.assertRaisesRegex(
            seal.SourceSealError,
            "content-conversion attribute crlf=set",
        ):
            seal.compute_observed_descriptor(self.root)

    def test_raw_mode_ignores_local_core_filemode_false(self) -> None:
        tracked = self.root / "tracked.txt"
        tracked.write_text("tracked\n", encoding="utf-8")
        self.commit()
        clean = seal.compute_observed_descriptor(self.root)
        self.git("config", "core.fileMode", "false")
        tracked.chmod(0o755)

        self.assertEqual(self.git("diff", "--", "tracked.txt"), b"")
        changed = seal.compute_observed_descriptor(self.root)
        self.assertTrue(changed["source_repo_dirty"])
        self.assertNotEqual(
            clean["source_tree_sha256"],
            changed["source_tree_sha256"],
        )
        self.assertNotEqual(
            changed["tracked_binary_diff_sha256"],
            hashlib.sha256(b"").hexdigest(),
        )

    def test_rejects_unbound_partially_staged_index_preimage(self) -> None:
        tracked = self.root / "tracked.txt"
        tracked.write_bytes(b"signed HEAD bytes\n")
        self.commit()
        tracked.write_bytes(b"arbitrary staged intermediary\n")
        self.git("add", "tracked.txt")
        tracked.write_bytes(b"signed HEAD bytes\n")

        self.assertEqual(self.git("diff", "HEAD", "--", "tracked.txt"), b"")
        with self.assertRaisesRegex(
            seal.SourceSealError,
            "unbound intermediary blob or mode",
        ):
            seal.compute_observed_descriptor(self.root)

    def test_recursive_internal_symlink_chain_is_bound(self) -> None:
        target = self.root / "target.txt"
        target.write_text("tracked target\n", encoding="utf-8")
        os.symlink("target.txt", self.root / "second-link")
        os.symlink("second-link", self.root / "first-link")
        self.commit()
        identity, _, path, pin = self.reviewed_identity()

        self.assertFalse(identity.source_repo_dirty)
        target.write_text("changed target\n", encoding="utf-8")
        with self.assertRaisesRegex(seal.SourceSealError, "differs"):
            seal.compute_identity(self.root, str(path), pin)

    def test_internal_broken_tracked_symlink_is_admitted_as_absent(self) -> None:
        os.symlink("missing/internal-target", self.root / "broken-link")
        self.commit()

        descriptor = seal.compute_observed_descriptor(self.root)
        self.assertFalse(descriptor["source_repo_dirty"])

    def test_rejects_tracked_symlink_cycle(self) -> None:
        os.symlink("second-link", self.root / "first-link")
        os.symlink("first-link", self.root / "second-link")
        self.commit()

        with self.assertRaisesRegex(seal.SourceSealError, "cycle"):
            seal.compute_observed_descriptor(self.root)

    def test_rejects_tracked_symlink_escaping_to_mutable_external_bytes(self) -> None:
        external = self.review_root / "external-target.txt"
        external.write_text("external bytes\n", encoding="utf-8")
        payload = os.path.relpath(external, self.root)
        self.assertTrue(payload.startswith(".."))
        os.symlink(payload, self.root / "external-link")
        self.commit()

        with self.assertRaisesRegex(seal.SourceSealError, "escapes"):
            seal.compute_observed_descriptor(self.root)
        external.write_text("mutated external bytes\n", encoding="utf-8")
        with self.assertRaisesRegex(seal.SourceSealError, "escapes"):
            seal.compute_observed_descriptor(self.root)

    def test_private_materialization_is_exact_and_isolated(self) -> None:
        tracked = self.root / "tracked.txt"
        tracked.write_text("base\n", encoding="utf-8")
        self.commit()
        tracked.write_text("reviewed dirty bytes\n", encoding="utf-8")
        staged = self.root / "staged.txt"
        staged.write_text("reviewed staged bytes\n", encoding="utf-8")
        self.git("add", "staged.txt")
        untracked = self.root / "untracked.txt"
        untracked.write_text("reviewed untracked bytes\n", encoding="utf-8")
        identity, _, descriptor, pin = self.reviewed_identity()
        materialized_root = self.review_root / "private-source"
        materialized_descriptor = self.review_root / "private-descriptor.json"

        materialization = seal.materialize_reviewed_closure(
            self.root,
            materialized_root,
            materialized_descriptor,
            str(descriptor),
            pin,
            expected_identity=identity,
        )
        self.assertEqual(materialization.identity, identity)
        self.assertEqual(
            (materialized_root / "tracked.txt").read_bytes(),
            b"reviewed dirty bytes\n",
        )
        self.assertEqual(
            (materialized_root / "staged.txt").read_bytes(),
            b"reviewed staged bytes\n",
        )
        self.assertEqual(
            (materialized_root / "untracked.txt").read_bytes(),
            b"reviewed untracked bytes\n",
        )
        tracked.write_text("attacker mutation during Cargo\n", encoding="utf-8")
        self.assertEqual(
            (materialized_root / "tracked.txt").read_bytes(),
            b"reviewed dirty bytes\n",
        )
        self.assertEqual(
            seal.compute_identity(
                materialized_root,
                str(materialized_descriptor),
                pin,
            ),
            identity,
        )

    def test_private_materialization_copies_bound_untracked_symlink_target(self) -> None:
        os.symlink("generated-target.txt", self.root / "tracked-link")
        self.commit()
        target = self.root / "generated-target.txt"
        target.write_text("bound generated target\n", encoding="utf-8")
        identity, _, descriptor, pin = self.reviewed_identity()
        materialized_root = self.review_root / "private-source"
        materialized_descriptor = self.review_root / "private-descriptor.json"

        materialization = seal.materialize_reviewed_closure(
            self.root,
            materialized_root,
            materialized_descriptor,
            str(descriptor),
            pin,
            expected_identity=identity,
        )
        self.assertEqual(
            os.readlink(materialized_root / "tracked-link"),
            "generated-target.txt",
        )
        self.assertEqual(
            (materialized_root / "generated-target.txt").read_bytes(),
            b"bound generated target\n",
        )
        self.assertEqual(materialization.identity, identity)

    @unittest.skipUnless(sys.platform == "darwin", "requires macOS hdiutil")
    def test_sealed_materialization_denies_host_writes_on_read_only_image(self) -> None:
        tracked = self.root / "tracked.txt"
        tracked.write_bytes(b"reviewed source bytes\n")
        self.commit()
        identity, _, descriptor, pin = self.reviewed_identity()

        with seal.sealed_reviewed_closure(
            self.root,
            self.review_root / "sealed-workspace",
            str(descriptor),
            pin,
            expected_identity=identity,
        ) as materialization:
            self.assertEqual(materialization.identity, identity)
            self.assertNotEqual(
                os.statvfs(materialization.root).f_flag & os.ST_RDONLY,
                0,
            )
            sealed_file = materialization.root / "tracked.txt"
            with self.assertRaises(OSError):
                sealed_file.write_bytes(b"transient hostile source bytes\n")
            with self.assertRaises(OSError):
                sealed_file.chmod(0o600)
            self.assertEqual(sealed_file.read_bytes(), b"reviewed source bytes\n")

    @unittest.skipUnless(sys.platform == "darwin", "requires macOS system paths")
    def test_write_denial_accepts_real_root_owned_read_boundary(self) -> None:
        root_owned_source = Path("/usr/share").resolve(strict=True)
        root_owned_descriptor = Path("/private/etc/hosts").resolve(strict=True)
        source_metadata = root_owned_source.lstat()
        descriptor_metadata = root_owned_descriptor.lstat()
        if (
            source_metadata.st_uid != 0
            or descriptor_metadata.st_uid != 0
            or source_metadata.st_mode & 0o022
            or descriptor_metadata.st_mode & 0o022
        ):
            self.skipTest("host root-owned fixture paths are not safely permissioned")

        command = seal.write_denied_source_command(
            root_owned_source,
            root_owned_descriptor,
            ["/usr/bin/true"],
            platform_name="darwin",
        )

        self.assertEqual(command[0], "/usr/bin/sandbox-exec")
        self.assertIn(f"SOURCE_ROOT={root_owned_source}", command)
        self.assertIn(f"SOURCE_DESCRIPTOR={root_owned_descriptor}", command)

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
