#!/usr/bin/env python3
"""Synthetic live-reference retirement tests for Taira retry custody."""

import importlib.util
import os
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "taira_retry.py"
SPEC = importlib.util.spec_from_file_location("taira_retry", SCRIPT)
retry = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(retry)

class RetireLiveReferenceTests(unittest.TestCase):
    """Synthetic proc metadata exercises aliases without mounts or live mutation."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="taira-live-reference-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.binary = self.root / "release/bin/iroha"
        self.binary.parent.mkdir(parents=True)
        self.binary.write_bytes(b"selected public bytes")
        self.stamp = list(retry.identity(self.binary.stat()))
        self.device = self.stamp[0]
        self.proc = self.root / "proc"
        (self.proc / "self/ns").mkdir(parents=True)
        (self.proc / "self/root").symlink_to("/")
        (self.proc / "self/ns/mnt").symlink_to("mnt:[123]")
        self.mounts = self.mount("/", "/")
        (self.proc / "self/mountinfo").write_text(self.mounts)

    def mount(self, root, target):
        escape = lambda value: str(value).replace("\\", "\\134").replace(" ", "\\040")
        return f"1 0 {os.major(self.device)}:{os.minor(self.device)} {escape(root)} {escape(target)} rw - ext4 /dev/root rw\n"

    def process(self, pid=987654321, *, mounts=None, maps=""):
        path = self.proc / str(pid)
        (path / "fd").mkdir(parents=True)
        (path / "ns").mkdir()
        (path / "ns/mnt").symlink_to("mnt:[123]")
        (path / "maps").write_text(maps)
        (path / "mountinfo").write_text(self.mounts if mounts is None else mounts)
        (path / "root").symlink_to("/")
        return path

    def observe(self, roots=None, **kwargs):
        return retry._retire_live_references(roots or [self.binary], proc_root=self.proc, **kwargs)

    def admitted(self):
        return [{"path": str(self.binary), "identity": self.stamp}]

    def mapped(self, pathname="/unrelated/alias (deleted)"):
        return f"1000-2000 r-xp 00000000 {os.major(self.device):x}:{os.minor(self.device):x} {self.stamp[1]} {pathname}\n"

    def test_ordinary_filesystem_mount_is_not_an_alias(self):
        self.process()
        result = self.observe()
        self.assertTrue(result["passed"])
        self.assertFalse(result["argv_or_environment_read"])

    def test_parent_directory_bind_alias_is_detected_for_files_and_directories(self):
        self.process(mounts=self.mounts + self.mount(self.binary.parent, "/alias"))
        for root in (self.binary, self.binary.parent):
            with self.subTest(root=root):
                result = self.observe([root])
                self.assertFalse(result["passed"])
                self.assertTrue(any(row["kind"] == "mount_alias" for row in result["references"]))

    def test_ancestor_and_whole_filesystem_aliases_are_detected(self):
        for root in (self.root, Path("/")):
            with self.subTest(root=root):
                (self.proc / "self/mountinfo").write_text(self.mounts + self.mount(root, "/alias"))
                self.assertFalse(self.observe()["passed"])

    def test_selected_directory_subtree_bind_alias_is_detected(self):
        self.process(mounts=self.mounts + self.mount(self.binary.parent, "/alias"))
        self.assertFalse(self.observe([self.root / "release"])["passed"])

    def test_nonroot_filesystem_mount_uses_filesystem_relative_coordinates(self):
        self.mounts = self.mount("/subvolume", self.root)
        (self.proc / "self/mountinfo").write_text(self.mounts)
        self.process()
        self.assertTrue(self.observe()["passed"])
        alias = self.mount("/subvolume/release", "/alias")
        (self.proc / "987654321/mountinfo").write_text(self.mounts + alias)
        self.assertFalse(self.observe()["passed"])

    def test_aliased_fd_and_executable_are_detected_by_inode(self):
        process = self.process()
        alias = self.root / "outside-alias"
        os.link(self.binary, alias)
        (process / "fd/8").symlink_to(alias)
        (process / "exe").symlink_to(alias)
        result = self.observe()
        self.assertEqual({row["kind"] for row in result["references"]}, {"fd", "exe"})

    def test_mapping_inode_survives_original_name_deletion(self):
        self.process(maps=self.mapped())
        self.binary.unlink()
        result = self.observe(file_identities=self.admitted())
        self.assertFalse(result["passed"])
        self.assertEqual(result["references"][0]["kind"], "maps")

    def test_closed_directory_file_census_detects_aliased_mapping(self):
        self.process(maps=self.mapped())
        result = self.observe([self.binary.parent], file_identities=self.admitted())
        self.assertFalse(result["passed"])
        self.assertEqual(result["references"][0]["kind"], "maps")

    def test_only_explicit_own_custody_descriptor_is_admitted(self):
        process = self.process(os.getpid())
        (process / "fd/8").symlink_to(self.binary)
        self.assertTrue(self.observe(own_fds=[8])["passed"])
        self.assertFalse(self.observe(own_fds=[])["passed"])
        (process / "exe").symlink_to(self.binary)
        self.assertFalse(self.observe(own_fds=[8])["passed"])

    def test_own_mapping_is_never_a_custody_descriptor(self):
        self.process(os.getpid(), maps=self.mapped())
        self.assertFalse(self.observe(own_fds=[8])["passed"])

    def test_same_mount_namespace_with_distinct_chroot_views_is_rechecked(self):
        self.process(987654320)
        process = self.process(987654321, mounts=self.mounts + self.mount(self.root, "/alias"))
        (process / "root").unlink()
        (process / "root").symlink_to(self.root)
        self.assertFalse(self.observe()["passed"])

    def test_malformed_or_oversized_metadata_fails_closed(self):
        process = self.process(maps="not a map\n")
        with self.assertRaisesRegex(retry._retire_RebindError, "mapping identity"):
            self.observe()
        (process / "maps").write_text("")
        for raw in (b"not a mount\n", b"x" * (4 * 1024 * 1024 + 1)):
            with self.subTest(size=len(raw)):
                (process / "mountinfo").write_bytes(raw)
                with self.assertRaises(retry._retire_RebindError):
                    self.observe()

    def test_kernel_thread_without_filesystem_root_has_no_mount_view(self):
        process = self.process(mounts="")
        (process / "root").unlink()
        self.assertTrue(self.observe()["passed"])

    def test_chroot_without_visible_mounts_uses_full_namespace_view(self):
        process = self.process(mounts="")
        (process / "root").unlink()
        (process / "root").symlink_to(self.root)
        self.assertTrue(self.observe()["passed"])

    def test_unobserved_chroot_namespace_with_empty_view_fails_closed(self):
        process = self.process(mounts="")
        (process / "ns/mnt").unlink()
        (process / "ns/mnt").symlink_to("mnt:[456]")
        with self.assertRaisesRegex(retry._retire_RebindError, "empty chroot mount view"):
            self.observe()

    def test_later_full_namespace_view_covers_earlier_empty_chroot(self):
        process = self.process(987654320, mounts="")
        (process / "root").unlink()
        (process / "root").symlink_to(self.root)
        (process / "ns/mnt").unlink()
        (process / "ns/mnt").symlink_to("mnt:[456]")
        later = self.process(987654321)
        (later / "ns/mnt").unlink()
        (later / "ns/mnt").symlink_to("mnt:[456]")
        self.assertTrue(self.observe()["passed"])

    def test_identity_outside_selected_scope_is_rejected(self):
        with self.assertRaisesRegex(retry._retire_RebindError, "escaped"):
            self.observe([self.root / "unrelated"], file_identities=self.admitted())


if __name__ == "__main__":
    unittest.main()
