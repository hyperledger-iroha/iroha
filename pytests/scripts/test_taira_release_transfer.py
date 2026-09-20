"""Offline custody/stream tests; no SSH, deployment, build or runtime input access."""
from __future__ import annotations

import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import shutil
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS))
import taira_release_transfer as transfer
import taira_disk_capacity as capacity


class SourceOwner:
    """Explicit source-owner seam: production cryptography has its own signed Git tests."""
    def __init__(self):
        self.imports = 0
        self.verifications = 0
        self.reject = False

    def import_source(self, pack, manifest, commit, tree, signer, root):
        self.imports += 1
        if self.reject:
            raise ValueError("signed object inventory rejected")
        root.mkdir(mode=0o700)
        transfer.write_new(root / "public.txt", b"exact signed source\n")
        return self.verify_import(root, manifest, commit, tree, signer)

    def verify_import(self, root, manifest, commit, tree, signer):
        self.verifications += 1
        if self.reject or transfer.read(root / "public.txt") != b"exact signed source\n":
            raise ValueError("source signature/tree rejection")
        return {"commit": commit, "tree": tree, "signer_fingerprint": signer,
                "clean": True, "signature_verified": True, "object_inventory_verified": True,
                "source_root": str(root), "activated": False,
                "runtime_files_included": False,
                "history_included": False, "runtime_files_transferred": False}


class TransferTests(unittest.TestCase):
    def setUp(self):
        # Native private ancestry; /tmp's sticky writable parent is deliberately
        # not accepted for root-owned deployment custody.
        self.directory = tempfile.TemporaryDirectory(prefix=".taira-transfer-test-", dir=Path.home())
        self.root = Path(self.directory.name).resolve()
        os.chmod(self.root, 0o700)
        self.runtime = self.root / "runtime"
        self.runtime.mkdir(mode=0o700)
        self.data = [b"\x7fELF" + bytes([index]) * 64 for index in range(4)] + [b"public git packet", b'{"public":"manifest"}\n']
        rows = [{"name": name, "size": len(raw), "sha256": transfer.sha(raw)}
                for name, raw in zip((*transfer.NAMES, "source.pack", "source-capture.json"), self.data)]
        self.request = {"schema": transfer.SCHEMA, "commit": "a" * 40, "tree": "b" * 40,
            "signer_fingerprint": "C" * 40, "result_sha256": "d" * 64,
            "runtime_root": str(self.runtime), "rows": rows,
            "allocation": {"bytes": 1024**2, "files": 100, "directories": 100}}
        self.owner = SourceOwner()
        self.capacity = types.SimpleNamespace(PLAN_SCHEMA=capacity.PLAN_SCHEMA,
            inspect_filesystem=lambda path: {"fragment_bytes": 4096},
            allocation_bound=capacity.allocation_bound,
            evaluate=lambda plan: {"schema": capacity.RESULT_SCHEMA, "passed": True, "errors": []})

    def tearDown(self):
        self.directory.cleanup()

    @property
    def destination(self):
        return self.runtime / transfer.import_name(self.request)

    def receive(self, data=None):
        return transfer.receive(self.request, io.BytesIO(b"".join(self.data) if data is None else data),
                                self.owner, self.capacity)

    def test_round_trip_publishes_exact_receipts_and_reuses_without_writes(self):
        result = self.receive()
        self.assertFalse(result["activated"])
        self.assertEqual(result["binary"]["artifacts"], self.request["rows"][:4])
        self.assertEqual(result["source"]["result_sha256"], self.request["result_sha256"])
        self.assertTrue(result["source"]["signature_verified"])
        self.assertFalse(result["source"]["history_included"])
        before = {str(path): transfer.identity(path.lstat()) for path in self.destination.rglob("*")}
        self.assertEqual(self.receive(), result)
        after = {str(path): transfer.identity(path.lstat()) for path in self.destination.rglob("*")}
        self.assertEqual(before, after)
        self.assertEqual(self.owner.imports, 1)
        for name in transfer.NAMES:
            self.assertEqual(stat.S_IMODE((self.destination / "artifacts/bin" / name).stat().st_mode), 0o755)
        pack = self.destination / "source/source.pack"
        self.assertEqual(stat.S_IMODE(pack.stat().st_mode), 0o600)
        self.assertEqual(stat.S_IMODE((self.destination / "source/source-capture.json").stat().st_mode), 0o400)
        pack.chmod(0o400)
        with self.assertRaisesRegex(transfer.TransferError, "input mode differs"):
            self.receive()

    def test_mutated_truncated_and_extra_streams_never_publish_success(self):
        for label, payload in (("changed", b"X" + b"".join(self.data)[1:]),
                               ("short", b"".join(self.data)[:-1]),
                               ("extra", b"".join(self.data) + b"X")):
            with self.subTest(label=label):
                self.request["result_sha256"] = transfer.sha(label.encode())
                with self.assertRaises(transfer.TransferError):
                    self.receive(payload)
                self.assertTrue(self.destination.is_dir())
                self.assertFalse((self.destination / "completed.json").exists())
                with self.assertRaises(transfer.TransferError):
                    self.receive()

    def test_source_verification_failure_preserves_partial_without_receipts(self):
        self.owner.reject = True
        with self.assertRaises(ValueError):
            self.receive()
        self.assertFalse((self.destination / "completed.json").exists())
        self.assertFalse((self.destination / "artifacts/verified-manifest.json").exists())
        self.assertTrue((self.destination / "source/source.pack").exists())

    def test_completed_reuse_rejects_every_payload_mutation_and_unsafe_link(self):
        self.receive()
        for row in self.request["rows"]:
            path = transfer.payload_path(self.destination, row["name"])
            original = path.read_bytes()
            path.chmod(0o600)
            path.write_bytes(b"x" * len(original))
            path.chmod(transfer.payload_mode(row["name"]))
            with self.subTest(name=row["name"]):
                with self.assertRaisesRegex(transfer.TransferError, "digest differs"):
                    self.receive()
            path.chmod(0o600)
            path.write_bytes(original)
            path.chmod(transfer.payload_mode(row["name"]))
        path = self.destination / "artifacts/bin/iroha"
        os.link(path, self.root / "alias")
        with self.assertRaisesRegex(transfer.TransferError, "custody"):
            self.receive()

    def test_completed_extra_or_missing_entry_and_source_drift_reject(self):
        self.receive()
        extra = self.destination / "extra"
        extra.write_bytes(b"unexpected")
        with self.assertRaisesRegex(transfer.TransferError, "unexpected entries"):
            self.receive()
        extra.unlink()
        source = self.destination / "source/source/public.txt"
        source.chmod(0o600)
        source.write_bytes(b"wrong signed source")
        with self.assertRaisesRegex(ValueError, "source signature/tree"):
            self.receive()

    def test_completed_receipt_and_repeated_stream_drift_reject(self):
        self.receive()
        with self.assertRaisesRegex(transfer.TransferError, "repeated transfer digest"):
            self.receive(b"X" + b"".join(self.data)[1:])
        receipt = self.destination / "artifacts/verified-manifest.json"
        receipt.chmod(0o600)
        receipt.write_bytes(b'{}\n')
        receipt.chmod(0o400)
        with self.assertRaisesRegex(transfer.TransferError, "receipt differs"):
            self.receive()

    def test_completed_capacity_charges_only_headroom_and_partial_is_not_adopted(self):
        fresh = transfer.capacity_probe(self.request, self.capacity)
        self.assertFalse(fresh["reuse"])
        self.assertEqual(len(fresh["plan"]["allocations"]), 2)
        self.receive()
        completed = transfer.capacity_probe(self.request, self.capacity)
        self.assertTrue(completed["reuse"])
        self.assertEqual(completed["plan"]["allocations"], [{"path": str(self.runtime),
            "label": "filesystem operating headroom", "bytes": transfer.HEADROOM, "inodes": 1024}])
        (self.destination / "completed.json").unlink()
        with self.assertRaises(FileNotFoundError):
            transfer.capacity_probe(self.request, self.capacity)

    def test_capacity_failure_precedes_destination_creation(self):
        self.capacity.evaluate = lambda plan: {"passed": False}
        with self.assertRaisesRegex(transfer.TransferError, "capacity"):
            self.receive()
        self.assertFalse(self.destination.exists())
        self.assertEqual(self.owner.imports, 0)

    def test_closed_payload_schema_rejects_traversal_duplicates_sizes_and_missing(self):
        for mutate in (
            lambda value: value["rows"].pop(),
            lambda value: value["rows"][0].update(name="../outside"),
            lambda value: value["rows"][1].update(name=value["rows"][0]["name"]),
            lambda value: value["rows"][0].update(size=True),
            lambda value: value["rows"][0].update(size=transfer.MAX_BINARY + 1),
            lambda value: value.update(extra=True),
        ):
            value = copy.deepcopy(self.request)
            mutate(value)
            with self.subTest(value=value):
                with self.assertRaises(transfer.TransferError):
                    transfer.validate_request(value)

    def test_same_tick_mutation_is_detected_by_descriptor_content_reread(self):
        path = self.root / "public"
        path.write_bytes(b"original")
        with patch.object(transfer, "identity", return_value=(1,)):
            with self.assertRaisesRegex(transfer.TransferError, "changed during use"):
                with transfer.pinned(path) as (fd, _, _):
                    path.write_bytes(b"modified")

    def test_private_parent_and_symlink_roots_reject_before_payload(self):
        self.runtime.chmod(0o777)
        with self.assertRaisesRegex(transfer.TransferError, "unsafe directory"):
            self.receive()
        self.runtime.chmod(0o700)
        alias = self.root / "alias"
        alias.symlink_to(self.runtime, target_is_directory=True)
        self.request["runtime_root"] = str(alias)
        with self.assertRaisesRegex(transfer.TransferError, "direct path"):
            self.receive()

    def test_receiver_stream_deadline_is_enforced_without_sender_eof(self):
        incoming, outgoing = os.pipe()
        try:
            with os.fdopen(incoming, "rb", buffering=0) as stream:
                reader = transfer.DeadlineReader(stream, 0.01)
                with self.assertRaisesRegex(transfer.TransferError, "receiver stream deadline"):
                    reader.read(1)
        finally:
            os.close(outgoing)

    def test_sender_streams_fixed_code_and_exact_bytes_through_real_pipe(self):
        modules = {name: b"" for name in transfer.MODULES}
        # Real subprocess executes the fixed framing bootstrap locally, without
        # SSH. The authenticated receiver seam only reads and reports exact bytes.
        modules["taira_release_transfer"] = b'''import hashlib
def remote_entry(e, stream):
 b=stream.read();return {"size":len(b),"sha256":hashlib.sha256(b).hexdigest()}
'''
        path = self.root / "payload"
        raw = b"bounded payload\x00" * 100000
        path.write_bytes(raw)
        row = {"size": len(raw), "sha256": transfer.sha(raw)}
        retry = types.SimpleNamespace(validate_ssh=lambda route: [sys.executable, "-c", "unused"])
        # Existing strict SSH validation is reused in production; replace only
        # command construction for this no-network real-child transport test.
        with patch.object(transfer, "REMOTE_COMMAND", transfer.BOOTSTRAP):
            result = transfer.remote_call({}, {"operation": "import"}, modules,
                self.root / "transport", retry, [(row, path)])
        self.assertEqual(result, {"size": len(raw), "sha256": transfer.sha(raw)})

    def test_signed_module_loader_ignores_poisoned_cached_objects(self):
        poisoned = types.ModuleType("release_artifact_contract")
        poisoned.marker = "untrusted cached code"
        source = {name: b"marker = 'signed bytes'\n" for name in transfer.CONTROLLERS}
        source["taira_source_capture"] = b"import release_artifact_contract\nmarker = release_artifact_contract.marker\n"
        with patch.dict(sys.modules, {"release_artifact_contract": poisoned}):
            loaded = transfer.load_signed_modules(source, self.root)
            self.assertIsNot(loaded["release_artifact_contract"], poisoned)
            self.assertEqual(loaded["taira_source_capture"].marker, "signed bytes")
            self.assertEqual(loaded["taira_release_transfer"].marker, "signed bytes")
        with self.assertRaisesRegex(transfer.TransferError, "byte closure"):
            transfer.load_signed_modules({"taira_retry": b""}, self.root)

    def preparation_fixture(self):
        import taira_release as release
        output = self.root / "preparation"
        output.mkdir(mode=0o700)
        (output / "attempts").mkdir(mode=0o700)
        attempt = output / "attempts/000001"
        attempt.mkdir(mode=0o700)
        (attempt / "bin").mkdir(mode=0o700)
        header = b"\x7fELF\x02\x01\x01" + b"\x00" * 9 + b"\x02\x00\xb7\x00"
        rows = []
        for name, package in zip(transfer.NAMES, transfer.PACKAGES):
            path = attempt / "bin" / name
            transfer.write_new(path, header, mode=0o500)
            rows.append({"name": name, "package": package, "path": str(path),
                         "size": len(header), "sha256": transfer.sha(header)})
        (attempt / "bin").chmod(0o500)
        base = dict.fromkeys(transfer.BASE_FIELDS, "fixture")
        base.update(commit="a" * 40, tree="b" * 40, signer_fingerprint="C" * 40,
            target="aarch64-unknown-linux-gnu", profile="release", jobs=6, native_check_scope="basic",
            source_unchanged=True, toolchain_unchanged=True, release_qualified=False, deployed=False,
            source_snapshot_sha256=transfer.sha(release.canonical_json_bytes([])),
            source_root=str(self.root / "source"))
        request = dict(base, schema=release.SESSION_SCHEMA,
                       repo_root=str(SCRIPTS.parent), target_dir=str(self.root))
        result = dict(base, artifacts=rows, timings_seconds={}, attempt="attempts/000001")
        raw = release.canonical_json_bytes(result)
        for path, value in ((output / "request.json", request),
                            (output / "checks.json", {"request": request, "passed": True}),
                            (attempt / "capture.json", result), (output / "result.json", result)):
            transfer.write_new(path, release.canonical_json_bytes(value))
        attempt.chmod(0o500)
        output.chmod(0o500)
        plan = {"expected_commit": "a" * 40, "expected_signer": "C" * 40,
                "preparation": {"path": str(output / "result.json"), "sha256": transfer.sha(raw)}}
        return release, plan, result, output

    def test_preparation_admits_exact_capture_and_refuses_changed_binary_or_native_checkpoint(self):
        release, plan, expected, output = self.preparation_fixture()
        with patch.object(release, "commit_entries", return_value=b""), \
             patch.object(release, "frozen_snapshot", return_value=[]):
            self.assertEqual(transfer.admit_preparation(plan, {"taira_release": release}, "b" * 40), expected)
            binary = Path(expected["artifacts"][0]["path"])
            binary.chmod(0o700)
            binary.write_bytes(b"x" * expected["artifacts"][0]["size"])
            binary.chmod(0o500)
            with self.assertRaisesRegex(release.PrepareError, "captured artifact changed"):
                transfer.admit_preparation(plan, {"taira_release": release}, "b" * 40)
            checkpoint = output / "checks.json"
            checkpoint.chmod(0o600)
            checkpoint.write_bytes(release.canonical_json_bytes({"passed": False}))
            checkpoint.chmod(0o400)
            with self.assertRaisesRegex(transfer.TransferError, "qualification checkpoint"):
                transfer.admit_preparation(plan, {"taira_release": release}, "b" * 40)

    def test_preparation_refuses_foreign_identity_capture_record_and_frozen_source(self):
        release, plan, expected, output = self.preparation_fixture()
        with patch.object(release, "commit_entries", return_value=b""), \
             patch.object(release, "frozen_snapshot", return_value=[]):
            for key, value in (("expected_commit", "e" * 40), ("expected_signer", "F" * 40)):
                wrong = dict(plan, **{key: value})
                with self.assertRaisesRegex(transfer.TransferError, "matching maintained preparation"):
                    transfer.admit_preparation(wrong, {"taira_release": release}, "b" * 40)
            with patch.object(release, "frozen_snapshot", return_value=[{"changed": True}]):
                with self.assertRaisesRegex(transfer.TransferError, "source snapshot differs"):
                    transfer.admit_preparation(plan, {"taira_release": release}, "b" * 40)
            capture = output / expected["attempt"] / "capture.json"
            capture.chmod(0o600)
            capture.write_bytes(b"{}\n")
            capture.chmod(0o400)
            with self.assertRaisesRegex(transfer.TransferError, "capture differs"):
                transfer.admit_preparation(plan, {"taira_release": release}, "b" * 40)

    def test_cli_help_has_no_activation_runtime_or_shell_options(self):
        result = subprocess.run([sys.executable, "-B", str(SCRIPTS / "taira_release_transfer.py"), "--help"],
                                capture_output=True, text=True, check=True)
        self.assertIn("--plan", result.stdout)
        for flag in ("--activate", "--command", "--private-key", "--reset", "--service"):
            self.assertNotIn(flag, result.stdout)


@unittest.skipUnless(shutil.which("git") and shutil.which("gpg"), "Git/GnuPG required")
class SignedTransferIntegrationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Reuse the adjacent maintained source owner's isolated genuine signing
        # fixture. No operator key or real repository index is accessed.
        spec = importlib.util.spec_from_file_location("transfer_source_fixture", Path(__file__).with_name("test_taira_source_capture.py"))
        cls.fixture_module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.fixture_module)
        cls.fixture_type = cls.fixture_module.SignedSourceCaptureTests
        cls.fixture_type.setUpClass()

    @classmethod
    def tearDownClass(cls):
        cls.fixture_type.tearDownClass()

    def setUp(self):
        self.fixture = self.fixture_type(methodName="runTest")
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)

    def test_actual_signed_source_and_four_binary_receiver_completion_and_reuse(self):
        source = self.fixture_module.source
        exported = self.fixture.export()
        runtime = self.fixture.case / "runtime"
        runtime.mkdir(mode=0o700)
        binary_rows = []
        for index, (name, package) in enumerate(zip(transfer.NAMES, transfer.PACKAGES)):
            path = self.fixture.case / name
            raw = b"ELF fixture payload" + bytes([index]) * 64
            transfer.write_new(path, raw, mode=0o500)
            binary_rows.append({"name": name, "package": package, "path": str(path),
                                "size": len(raw), "sha256": transfer.sha(raw)})
        build = {"commit": self.fixture.commit, "tree": self.fixture.tree, "artifacts": binary_rows}
        plan = {"expected_signer": self.fixture.fingerprint, "preparation": {"sha256": "d" * 64},
                "runtime_root": str(runtime)}
        request, payloads = transfer.make_request(plan, build, exported)
        raw = b"".join(path.read_bytes() for _, path in payloads)
        checker = types.SimpleNamespace(PLAN_SCHEMA=capacity.PLAN_SCHEMA,
            inspect_filesystem=lambda path: {"fragment_bytes": 4096},
            allocation_bound=capacity.allocation_bound,
            evaluate=lambda plan: {"schema": capacity.RESULT_SCHEMA, "passed": True, "errors": []})
        result = transfer.receive(request, io.BytesIO(raw), source, checker)
        self.assertEqual(result["source"]["signer_fingerprint"], self.fixture.fingerprint)
        self.assertTrue(result["source"]["object_inventory_verified"])
        self.assertFalse(result["source"]["history_included"])
        self.assertEqual(result, transfer.receive(request, io.BytesIO(raw), source, checker))
        imported = Path(result["source"]["source_root"])
        self.assertEqual((imported / "nested/data").read_bytes(), b"signed source\0with bytes\n")
        self.assertFalse((imported / "old-only").exists())
        source.verify_import(imported, imported.parent / "source-capture.json", self.fixture.commit,
                             self.fixture.tree, self.fixture.fingerprint)

    def test_actual_signed_controller_bytes_ignore_cached_module_and_reject_drift(self):
        scripts = self.fixture.repo / "scripts"
        scripts.mkdir(mode=0o755)
        for name in transfer.CONTROLLERS:
            (scripts / (name + ".py")).write_bytes((SCRIPTS / (name + ".py")).read_bytes())
        self.fixture.git("add", "scripts")
        self.fixture.git("commit", "-m", "signed transfer controller closure")
        commit = self.fixture.git("rev-parse", "HEAD").decode().strip()
        poisoned = types.ModuleType("taira_retry")
        poisoned.__file__ = str(scripts / "taira_retry.py")
        poisoned.validate_ssh = lambda value: (_ for _ in ()).throw(AssertionError("cached code executed"))
        with patch.dict(sys.modules, {"taira_retry": poisoned}):
            tree, modules, loaded = transfer.authenticated_modules(self.fixture.repo, commit, self.fixture.fingerprint)
            self.assertEqual(tree, self.fixture.git("rev-parse", "HEAD^{tree}").decode().strip())
            self.assertIsNot(loaded["taira_retry"], poisoned)
            self.assertEqual(set(modules), set(transfer.CONTROLLERS))
            self.assertEqual(loaded["taira_release_transfer"].SCHEMA, transfer.SCHEMA)
        with self.assertRaisesRegex(transfer.TransferError, "actual commit signer"):
            transfer.authenticated_modules(self.fixture.repo, commit, "F" * 40)
        (scripts / "taira_retry.py").write_bytes(b"raise AssertionError('modified controller')\n")
        with self.assertRaisesRegex(transfer.TransferError, "controller differs"):
            transfer.authenticated_modules(self.fixture.repo, commit, self.fixture.fingerprint)


if __name__ == "__main__":
    unittest.main()
