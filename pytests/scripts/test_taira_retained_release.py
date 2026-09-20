"""Offline retained-binary custody tests. No SSH, services, or runtime inputs."""
from __future__ import annotations

import contextlib
import copy
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
import taira_retained_release as owner
import taira_disk_capacity as capacity


class RetainedReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix=".taira-retained-test-", dir=Path.home())
        self.root = Path(self.temp.name).resolve()
        self.root.chmod(0o700)
        self.bins = self.root / "release85" / "bin"
        self.bins.parent.mkdir(mode=0o700)
        self.bins.mkdir(mode=0o700)
        self.rows = []
        for index, name in enumerate(owner.NAMES):
            data = b"\x7fELF" + bytes([index + 1]) * 64
            path = self.bins / name
            path.write_bytes(data)
            path.chmod(0o755)
            info = path.stat()
            self.rows.append(dict(path=str(path), size=len(data), sha256=owner.sha(data), mode=0o755,
                                  identity=owner.identity(info), allocated_bytes=info.st_blocks * 512,
                                  parent_identity=owner.identity(self.bins.stat())[:2]))
        ref = lambda name: dict(path=str(self.root / name), sha256="a" * 64)
        self.plan = dict(schema=owner.SCHEMA, provider="macstadium-dublin",
            controller=dict(commit="b" * 40, signer="C" * 40), guest_ssh={}, backing_ssh={},
            backing_path=str(self.root), deployment=ref("deployment.json"), current_inventory=ref("assembly94/inventory.json"),
            units=[dict(path=f"/etc/systemd/system/iroha3d-taira-validator-{i}.service", sha256="d" * 64) for i in range(1, 5)],
            releases=[{name: ref(name + ".json") for name in ("inventory", "terminal", "binary_manifest", "source_manifest")}])
        self.deployment = dict(runtime_root=str(self.root))
        self.admission = dict(schema=owner.SCHEMA, plan_sha256=owner.sha(owner.canonical(self.plan)),
            deployment=self.deployment, rows=self.rows, sources=[dict(commit="e" * 40, tree="f" * 40)],
            source_files_read=False, runtime_files_read=False)
        self.intent = owner.retirement_intent(self.admission, "0" * 64)
        self.custody = self.root / "custody"
        self.custody.mkdir(mode=0o700)
        self.stack = contextlib.ExitStack()
        self.stack.enter_context(patch.object(owner, "inspect", return_value=self.admission))
        self.bindings = self.stack.enter_context(patch.object(owner, "current_bindings", return_value=set()))
        self.references = self.stack.enter_context(patch.object(owner, "no_live_references", return_value={"passed": True}))

    def tearDown(self):
        self.stack.close()
        self.temp.cleanup()

    def retire(self):
        return owner.retire_locked(self.plan, self.deployment, self.admission, self.intent, self.custody)

    def quarantines(self):
        return [Path(row["quarantine"]) for row in self.intent["rows"]]

    def test_complete_and_repeat_preserve_receipts(self):
        receipt = self.root / "retained-source-receipt.json"
        receipt.write_bytes(b"retained source identity")
        first = self.retire()
        self.assertEqual(first, self.retire())
        self.assertFalse(first["deployment_authorized"])
        self.assertEqual(first["files"], 4)
        self.assertEqual(list(self.bins.iterdir()), [])
        self.assertEqual(receipt.read_bytes(), b"retained source identity")

    def test_all_names_quarantined_before_any_unlink(self):
        original = os.unlink
        calls = []
        def unlink(path, *args, **kwargs):
            self.assertTrue(all(not Path(row["path"]).exists() for row in self.rows))
            calls.append(path)
            return original(path, *args, **kwargs)
        with patch.object(os, "unlink", side_effect=unlink):
            self.retire()
        self.assertEqual(len(calls), 4)

    def test_supervisor_appears_during_quarantine_unlinks_nothing(self):
        rename = owner.rename_exclusive
        def change(*args):
            rename(*args)
            if args[0].parent == self.bins:
                self.bindings.side_effect = owner.RetainedReleaseError("supervisor authority appeared")
        with patch.object(owner, "rename_exclusive", side_effect=change), patch.object(os, "unlink") as unlink:
            with self.assertRaisesRegex(ValueError, "supervisor"):
                self.retire()
            unlink.assert_not_called()
        self.assertEqual(sum(path.exists() for path in self.quarantines()), 1)

    def test_reference_appears_after_quarantine_unlinks_nothing(self):
        rename = owner.rename_exclusive
        def change(*args):
            rename(*args)
            if args[0].parent == self.bins:
                self.references.side_effect = owner.RetainedReleaseError("live descriptor")
        with patch.object(owner, "rename_exclusive", side_effect=change), patch.object(os, "unlink") as unlink:
            with self.assertRaisesRegex(ValueError, "live descriptor"):
                self.retire()
            unlink.assert_not_called()

    def test_old_name_recreated_after_rename_stops(self):
        rename = owner.rename_exclusive
        def change(source, *args):
            rename(source, *args)
            if source.parent == self.bins:
                source.write_bytes(b"replacement")
                source.chmod(0o755)
        with patch.object(owner, "rename_exclusive", side_effect=change), patch.object(os, "unlink") as unlink:
            with self.assertRaisesRegex(ValueError, "reappeared"):
                self.retire()
            unlink.assert_not_called()

    def test_changed_source_inode_stops_before_quarantine(self):
        path = Path(self.rows[0]["path"])
        replacement = path.with_name("replacement")
        replacement.write_bytes(path.read_bytes())
        replacement.chmod(0o755)
        os.replace(replacement, path)
        with self.assertRaisesRegex(ValueError, "identity"):
            self.retire()
        self.assertFalse(any(path.exists() for path in self.quarantines()))

    def test_same_size_rewrite_stops(self):
        Path(self.rows[0]["path"]).write_bytes(b"x" * self.rows[0]["size"])
        with self.assertRaises(ValueError):
            self.retire()

    def test_new_link_stops(self):
        os.link(self.rows[0]["path"], self.root / "outside-link")
        with self.assertRaisesRegex(ValueError, "custody"):
            self.retire()

    def test_unadmitted_sibling_stops_before_read(self):
        (self.bins / "secret.toml").write_bytes(b"never open")
        with patch.object(owner, "held", side_effect=AssertionError("must census first")):
            with self.assertRaisesRegex(ValueError, "sibling"):
                self.retire()

    def test_absent_without_delete_intent_stops(self):
        Path(self.rows[0]["path"]).unlink()
        with self.assertRaisesRegex(ValueError, "without durable deletion intent"):
            self.retire()

    def test_interrupted_unlink_resumes_from_durable_intent(self):
        original = os.unlink
        calls = 0
        def unlink(path, *args, **kwargs):
            nonlocal calls
            original(path, *args, **kwargs)
            calls += 1
            if calls == 1:
                raise RuntimeError("process interrupted after first unlink")
        with patch.object(os, "unlink", side_effect=unlink):
            with self.assertRaisesRegex(RuntimeError, "interrupted"):
                self.retire()
        self.assertEqual(sum(path.exists() for path in self.quarantines()), 3)
        self.assertTrue(self.retire()["retired"])

    def test_quarantine_corruption_on_resume_stops(self):
        with patch.object(owner, "marker", side_effect=RuntimeError("interrupted after rename")):
            with self.assertRaises(RuntimeError):
                self.retire()
        quarantine = self.quarantines()[0]
        quarantine.write_bytes(b"x" * self.rows[0]["size"])
        with patch.object(os, "unlink") as unlink:
            with self.assertRaises(ValueError):
                self.retire()
            unlink.assert_not_called()

    def test_replaced_parent_during_held_read_stops(self):
        path = Path(self.rows[0]["path"])
        with self.assertRaises((ValueError, FileNotFoundError)):
            with owner.held(path):
                self.bins.rename(self.bins.with_name("old-bin"))
                self.bins.mkdir(mode=0o700)

    def test_exclusive_rename_never_clobbers(self):
        original = Path(self.rows[0]["path"])
        destination = self.bins / "occupied"
        destination.write_bytes(b"preserve")
        fd = os.open(original, os.O_RDONLY)
        try:
            with self.assertRaisesRegex(ValueError, "exclusive quarantine"):
                owner.rename_exclusive(original, destination, fd)
        finally:
            os.close(fd)
        self.assertEqual(destination.read_bytes(), b"preserve")

    def test_public_bin_mode_0755_is_admitted(self):
        self.bins.chmod(0o755)
        owner.census(self.rows)
        self.assertTrue(self.retire()["retired"])

    def test_parent_replaced_after_quarantine_unlinks_nothing(self):
        moved = False
        def bindings(*args):
            nonlocal moved
            if not moved and all(not Path(row["path"]).exists() for row in self.rows):
                moved = True
                self.bins.rename(self.bins.with_name("original-bin"))
                self.bins.mkdir(mode=0o755)
            return set()
        self.bindings.side_effect = bindings
        with patch.object(os, "unlink") as unlink:
            with self.assertRaisesRegex(ValueError, "absent before deletion intent"):
                self.retire()
            unlink.assert_not_called()

    def test_current_private_record_rejected_before_read(self):
        plan = copy.deepcopy(self.plan)
        plan["current_inventory"]["path"] = str(self.root / "keys.toml")
        # Use the actual function, which setup normally replaces at the process seam.
        self.stack.close()
        with patch.object(owner, "pin_json", side_effect=AssertionError("private content read")):
            with self.assertRaisesRegex(ValueError, "public inventory namespace"):
                owner.current_bindings(plan, self.deployment)

    def test_actual_three_runtime_roles_join_four_receipt_binaries(self):
        # Match the current production public records: Kagami is a transfer
        # artifact, while native runtime hosts consume the other three roles.
        release = dict(inventory=dict(path=str(self.root / "assembly85/inventory.json"), sha256="1" * 64),
            terminal=dict(path=str(self.root / "journal-v1/rolled-back" / ("2" * 64 + ".json")), sha256="3" * 64),
            binary_manifest=dict(path=str(self.root / "release85/verified-manifest.json"), sha256="4" * 64),
            source_manifest=dict(path=str(self.root / "source-transfer85/verified-manifest.json"), sha256="5" * 64))
        plan = {**self.plan, "releases": [release]}
        revision = dict(commit="a" * 40, tree="b" * 40, source_root="/opt/iroha/signed-source")
        artifacts = [dict(role=role, local_path=row["path"], sha256=row["sha256"], size=row["size"])
            for role, row in zip(("iroha3d", "iroha_cli", "sorafs_node"), self.rows)]
        inventory = dict(revision=revision, deployment_id="closed-release", authorization_nonce="nonce",
            validators=[dict(artifacts=artifacts)] * 4, edge=dict(artifacts=artifacts))
        terminal = dict(inventory_sha256="1" * 64, authorization_nonce="nonce", authorization_sha256="2" * 64)
        binary = dict(commit=revision["commit"], destination=str(self.bins), all_hashes_verified=True, activated=False,
            artifacts=[dict(name=Path(row["path"]).name, sha256=row["sha256"], size=row["size"]) for row in self.rows])
        source = {**revision, "clean": True, "signature_verified": True, "object_inventory_verified": True,
            "activated": False, "runtime_files_transferred": False, "runtime_files_included": False, "history_included": False}
        records = dict(zip((ref["path"] for ref in release.values()), (inventory, terminal, binary, source)))
        import taira_retry
        with patch.object(owner, "pin_json", side_effect=lambda ref: records[ref["path"]]), \
             patch.object(taira_retry, "_retire_validate_terminal") as native_admission:
            rows, sources = owner.authority_rows(plan, self.deployment)
        native_admission.assert_called_once()
        self.assertEqual([Path(row["path"]).name for row in rows], list(owner.NAMES))
        self.assertEqual(sources, [dict(commit=revision["commit"], tree=revision["tree"])])

    def fake_archive(self, output, *, corrupt=False, incomplete=False):
        class Stream:
            def __init__(stream):
                stream.frames = [self.admission, {"archive_stream_verified": True, "admission_sha256": owner.sha(owner.canonical(self.admission))}]
                stream.index = 0
            def frame(stream):
                if incomplete and len(stream.frames) == 1:
                    raise ValueError("interrupted stream")
                return stream.frames.pop(0)
            def exact(stream, count):
                data = Path(self.rows[stream.index]["path"]).read_bytes()
                stream.index += 1
                return b"x" * count if corrupt else data
        @contextlib.contextmanager
        def session(route, envelope, modules, evidence):
            evidence.write_bytes(b"")
            evidence.chmod(0o600)
            yield Stream()
        with patch.object(owner, "session", side_effect=session):
            return owner.archive_local(self.plan, self.deployment, self.admission, {}, output)

    def test_archive_rereads_exact_bytes_before_completion(self):
        output = self.root / "archive"
        self.fake_archive(output)
        self.assertEqual(owner.verify_archive(output)[1], self.admission)
        for path in (output / "objects").iterdir():
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o400)

    def test_corrupt_or_interrupted_archive_cannot_authorize_retirement(self):
        for label, kwargs in (("corrupt", {"corrupt": True}), ("interrupted", {"incomplete": True})):
            with self.subTest(label=label):
                output = self.root / label
                with self.assertRaises(ValueError):
                    self.fake_archive(output, **kwargs)
                self.assertFalse((output / "completed.json").exists())
                with self.assertRaises(ValueError):
                    owner.verify_archive(output)
        self.assertTrue(all(Path(row["path"]).exists() for row in self.rows))

    def test_missing_or_corrupt_backup_refuses_resume(self):
        output = self.root / "archive"
        self.fake_archive(output)
        obj = output / "objects/0000"
        obj.chmod(0o600)
        obj.write_bytes(b"z" * self.rows[0]["size"])
        obj.chmod(0o400)
        with self.assertRaisesRegex(ValueError, "digest"):
            owner.verify_archive(output)
        obj.unlink()
        with self.assertRaisesRegex(ValueError, "census"):
            owner.verify_archive(output)

    def test_metadata_capacity_excludes_binary_bytes(self):
        captured = []
        def allocation(path, payload, files, directories=0):
            captured.append((payload, files, directories))
            return {}, {}
        with patch.object(owner, "allocation", side_effect=allocation):
            owner.retirement_capacity(self.admission, "0" * 64)
            larger = copy.deepcopy(self.admission)
            for row in larger["rows"]:
                row["size"] += 1024**3
            owner.retirement_capacity(larger, "0" * 64)
        self.assertLess(abs(captured[1][0] - captured[0][0]), 1024)
        self.assertEqual(captured[0][1:], (24, 2))
        self.assertEqual(owner.RESERVE, 256 * 1024**2)

    def test_capacity_exact_and_minus_one_byte_or_inode(self):
        plan, _ = owner.retirement_capacity(self.admission, "0" * 64)
        required_bytes = sum(row["bytes"] for row in plan["allocations"])
        required_inodes = sum(row["inodes"] for row in plan["allocations"])
        for byte_delta, inode_delta, passed in ((0, 0, True), (-1, 0, False), (0, -1, False)):
            with self.subTest(byte_delta=byte_delta, inode_delta=inode_delta):
                result = capacity.evaluate(plan, inspect=lambda _: dict(device=1, anchor=str(self.root), fragment_bytes=4096,
                    available_bytes=required_bytes + byte_delta, available_inodes=required_inodes + inode_delta))
                self.assertEqual(result["passed"], passed)

    def test_failed_trim_does_not_claim_physical_capacity(self):
        import subprocess
        results = [subprocess.CompletedProcess([], 0, b"/\n", b""),
                   subprocess.CompletedProcess([], 0, b"", b""),
                   subprocess.CompletedProcess([], 1, b"", b"unsupported discard")]
        with patch.object(owner.subprocess, "run", side_effect=results) as run:
            result = owner.trim_and_observe(self.root)
        self.assertFalse(result["trim"]["passed"])
        self.assertFalse(result["physical_reclamation_claimed"])
        self.assertIn("available_bytes", result["guest_after"])
        self.assertEqual(run.call_args_list[-1].args[0], ["/usr/sbin/fstrim", "--verbose", "/"])

    def test_changed_runtime_mount_refuses_trim(self):
        import subprocess
        with patch.object(owner.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, b"/other\n", b"")) as run:
            with self.assertRaisesRegex(ValueError, "root filesystem"):
                owner.trim_and_observe(self.root)
        self.assertEqual(run.call_count, 1)

    def test_archive_stream_forwards_only_held_custody_descriptors(self):
        read_fd, write_fd = os.pipe()
        try:
            with patch.object(owner, "authority_locks", return_value=contextlib.nullcontext()), \
                 patch.object(owner, "inspect", return_value=self.admission) as inspect:
                owner.archive_stream(self.plan, self.deployment, self.admission, write_fd)
            self.assertEqual(len(inspect.call_args.kwargs["own_fds"]), 4)
            reader = owner.Reader(read_fd, timeout=1)
            self.assertEqual(reader.frame(), self.admission)
            for row in self.rows:
                self.assertEqual(owner.sha(reader.exact(row["size"])), row["sha256"])
            self.assertTrue(reader.frame()["archive_stream_verified"])
        finally:
            os.close(read_fd)
            os.close(write_fd)

    def test_current_binding_change_during_archive_has_no_completion(self):
        read_fd, write_fd = os.pipe()
        changed = copy.deepcopy(self.admission)
        changed["deployment"] = {"changed": True}
        try:
            with patch.object(owner, "authority_locks", return_value=contextlib.nullcontext()), \
                 patch.object(owner, "inspect", side_effect=[self.admission, changed]):
                with self.assertRaisesRegex(ValueError, "authority changed"):
                    owner.archive_stream(self.plan, self.deployment, self.admission, write_fd)
        finally:
            os.close(read_fd)
            os.close(write_fd)

    def test_existing_lock_replaced_during_flock_stops(self):
        journal = self.root / "journal-v1"
        journal.mkdir(mode=0o700)
        for path in (self.root / ".routine-update.lock", journal / "public-reset.lock"):
            path.write_bytes(b"")
            path.chmod(0o600)
        flock = owner.fcntl.flock
        def replace(fd, flags):
            flock(fd, flags)
            path = self.root / ".routine-update.lock"
            path.unlink()
            path.write_bytes(b"")
            path.chmod(0o600)
        with patch.object(owner.fcntl, "flock", side_effect=replace):
            with self.assertRaisesRegex(ValueError, "lock replaced"):
                with owner.authority_locks(self.deployment):
                    self.fail("replaced lock must not authorize work")

    def test_backing_demand_mirrors_guest_then_adds_physical_reserve(self):
        with patch.object(owner.sys, "platform", "darwin"), patch.object(owner, "send_frame") as send:
            owner.remote(dict(operation="backing-capacity", path=str(self.root), bytes=owner.RESERVE + 1024))
        result = send.call_args.args[1]
        self.assertTrue(result["passed"])
        self.assertEqual(result["filesystems"][0]["required_bytes"], owner.RESERVE * 2 + 1024)

    def test_partial_record_write_resumes_exact_prefix(self):
        target = self.root / "progress.json"
        raw = owner.canonical({"step": "durable exact intent"})
        def partial(fd, data):
            os.write(fd, data[:7])
            raise OSError("interrupted partial write")
        with patch.object(owner, "write_all", side_effect=partial):
            with self.assertRaisesRegex(OSError, "partial write"):
                owner.write_new(target, raw)
        self.assertFalse(target.exists())
        self.assertEqual((self.root / ".progress.json.pending").read_bytes(), raw[:7])
        owner.write_new(target, raw)
        self.assertEqual(owner.read(target, mode=0o400), raw)
        self.assertFalse((self.root / ".progress.json.pending").exists())

    def test_fsync_before_record_publication_resumes(self):
        target = self.root / "progress.json"
        raw = owner.canonical({"step": "sealed but unpublished"})
        with patch.object(owner, "rename_exclusive", side_effect=OSError("publication interrupted")):
            with self.assertRaisesRegex(OSError, "publication interrupted"):
                owner.write_new(target, raw)
        pending = self.root / ".progress.json.pending"
        self.assertEqual(stat.S_IMODE(pending.stat().st_mode), 0o400)
        self.assertFalse(target.exists())
        owner.write_new(target, raw)
        self.assertEqual(owner.read(target, mode=0o400), raw)

    def test_publication_before_parent_fsync_revalidates_and_syncs(self):
        value = {"step": "published before parent sync"}
        target = self.root / "progress.json"
        fsync = os.fsync
        def fail_parent(fd):
            if stat.S_ISDIR(os.fstat(fd).st_mode):
                raise OSError("parent synchronization interrupted")
            return fsync(fd)
        with patch.object(os, "fsync", side_effect=fail_parent):
            with self.assertRaisesRegex(OSError, "parent synchronization"):
                owner.marker(self.root, "progress.json", value)
        self.assertEqual(owner.read(target, mode=0o400), owner.canonical(value))
        with patch.object(owner, "sync", wraps=owner.sync) as sync:
            owner.marker(self.root, "progress.json", value)
        sync.assert_called_once_with(self.root)

    def test_foreign_pending_record_is_preserved_and_refused(self):
        pending = self.root / ".progress.json.pending"
        pending.write_bytes(b"foreign bytes")
        pending.chmod(0o600)
        with self.assertRaisesRegex(ValueError, "pending record differs"):
            owner.write_new(self.root / "progress.json", b"intended longer bytes")
        self.assertEqual(pending.read_bytes(), b"foreign bytes")
        self.assertFalse((self.root / "progress.json").exists())

    def test_partial_initial_intent_recovers_without_touching_binaries(self):
        def partial(fd, data):
            os.write(fd, data[:19])
            raise OSError("partial initial intent")
        with patch.object(owner, "write_all", side_effect=partial):
            with self.assertRaisesRegex(OSError, "partial initial intent"):
                self.retire()
        self.assertTrue(all(Path(row["path"]).exists() for row in self.rows))
        self.assertFalse(any(path.exists() for path in self.quarantines()))
        self.assertTrue(self.retire()["retired"])

    def test_plan_rejects_unknown_fields_duplicates_and_traversal(self):
        owner.validate_plan(self.plan)
        for change in (lambda p: p.update(private_config="forbidden"),
                       lambda p: p["releases"].append(copy.deepcopy(p["releases"][0])),
                       lambda p: p["deployment"].update(path=str(self.root) + "/../private")):
            plan = copy.deepcopy(self.plan)
            change(plan)
            with self.assertRaises(ValueError):
                owner.validate_plan(plan)


if __name__ == "__main__":
    unittest.main()
