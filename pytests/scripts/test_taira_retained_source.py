"""Public source custody regressions using small signed Git fixtures, no SSH/Cargo."""
from __future__ import annotations

import contextlib
import copy
import hashlib
import json
import os
from pathlib import Path
import stat
import struct
import sys
import tempfile
import unittest
import zlib
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import test_taira_source_capture as fixture_module
import taira_retained_source as owner
import taira_retained_release as common
import taira_source_capture as source


class SourceWireTests(unittest.TestCase):
    @contextlib.contextmanager
    def reader(self, raw):
        with tempfile.TemporaryFile() as stream:
            stream.write(raw)
            stream.seek(0)
            yield owner.Reader(stream.fileno())

    def encoded(self, raw):
        with tempfile.TemporaryFile() as stream:
            owner.payload_chunk(stream.fileno(), raw)
            stream.seek(0)
            return stream.read()

    def test_chunked_roundtrip_crosses_boundary_with_incompressible_member(self):
        chunks = [b"a" * common.CHUNK, os.urandom(common.CHUNK), b"last"]
        raw = b"".join(self.encoded(chunk) for chunk in chunks)
        with self.reader(raw) as reader:
            remaining = sum(map(len, chunks))
            for expected in chunks:
                self.assertEqual(reader.payload_chunk(remaining), expected)
                remaining -= len(expected)
            reader.eof()

    def test_sender_refuses_empty_and_oversized_members(self):
        for raw in (b"", b"x" * (common.CHUNK + 1)):
            with self.subTest(size=len(raw)), self.assertRaisesRegex(ValueError, "chunk exceeds bound"):
                self.encoded(raw)

    def test_declared_lengths_refused_before_reading_compressed_body(self):
        cases = [(0, 1, 1), (common.CHUNK + 1, 1, common.CHUNK + 1),
                 (2, 1, 1), (1, 0, 1), (1, 1026, 1)]
        for expanded, compressed, remaining in cases:
            with self.subTest(lengths=(expanded, compressed, remaining)), self.reader(struct.pack(">II", expanded, compressed)) as reader:
                with patch.object(reader, "exact", wraps=reader.exact) as exact:
                    with self.assertRaisesRegex(ValueError, "chunk exceeds bound"):
                        reader.payload_chunk(remaining)
                    self.assertEqual([call.args for call in exact.call_args_list], [(8,)])

    def test_truncated_chunk_header_and_body_refuse(self):
        wire = self.encoded(b"public source")
        for truncated in (wire[:7], wire[:-1]):
            with self.subTest(length=len(truncated)), self.reader(truncated) as reader:
                with self.assertRaisesRegex(ValueError, "truncated archive stream"):
                    reader.payload_chunk(100)

    def test_zlib_termination_corruption_and_expansion_are_bounded(self):
        good = zlib.compress(b"abc", 1)
        cases = [(3, good[:-1]), (3, good + b"tail"), (3, good + good),
                 (2, good), (4, good), (1, zlib.compress(b"x" * 16384, 1)),
                 (3, good[:-1] + bytes([good[-1] ^ 1]))]
        for expanded, encoded in cases:
            with self.subTest(expanded=expanded, encoded=len(encoded)), self.reader(struct.pack(">II", expanded, len(encoded)) + encoded) as reader:
                with self.assertRaisesRegex(ValueError, "compressed source chunk"):
                    reader.payload_chunk(100)

    def test_decoder_receives_only_declared_expansion_plus_one(self):
        encoded = zlib.compress(b"x" * 16384, 1)
        decoder = unittest.mock.Mock(wraps=zlib.decompressobj())
        with self.reader(struct.pack(">II", 1, len(encoded)) + encoded) as reader:
            with patch.object(owner.zlib, "decompressobj", return_value=decoder):
                with self.assertRaisesRegex(ValueError, "termination differs"):
                    reader.payload_chunk(100)
        decoder.decompress.assert_called_once_with(encoded, 2)

    def test_original_deadline_covers_before_and_after_decompression(self):
        wire = self.encoded(b"abc")
        for times, invoked in (([0, 0, 11], False), ([0, 0, 0, 11], True)):
            with self.subTest(invoked=invoked), self.reader(wire) as reader:
                reader.deadline = 10
                with patch.object(owner.time, "monotonic", side_effect=times), patch.object(owner.zlib, "decompressobj", wraps=zlib.decompressobj) as decoder:
                    with self.assertRaisesRegex(ValueError, "deadline expired"):
                        reader.payload_chunk(3)
                    self.assertEqual(decoder.called, invoked)
                    self.assertEqual(reader.deadline, 10)

    def test_eof_rejects_trailing_data_expired_and_stalled_stream(self):
        with self.reader(b"trailing") as reader:
            with self.assertRaisesRegex(ValueError, "trailing source"):
                reader.eof()
        with self.reader(b"") as reader:
            reader.deadline = 0
            with self.assertRaisesRegex(ValueError, "deadline expired"):
                reader.eof()
        with self.reader(b"") as reader, patch.object(owner.select, "select", return_value=([], [], [])):
            with self.assertRaisesRegex(ValueError, "deadline expired"):
                reader.eof()


class RetainedSourceFixture:
    delta_fixture = False

    @classmethod
    def setUpClass(cls):
        fixture_module.SignedSourceCaptureTests.setUpClass()

    @classmethod
    def tearDownClass(cls):
        fixture_module.SignedSourceCaptureTests.tearDownClass()

    def setUp(self):
        self.fixture = fixture_module.SignedSourceCaptureTests(methodName="runTest")
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        if self.delta_fixture:
            # Real Git needs sufficiently similar, nontrivial blobs to choose
            # delta storage. Both versions remain in the one signed tree.
            data = b"".join(hashlib.sha256(str(index).encode()).digest() for index in range(2048))
            (self.fixture.repo / "similar-a").write_bytes(data)
            (self.fixture.repo / "similar-b").write_bytes(data[:32000] + b"x" * 64 + data[32064:])
            self.fixture.git("add", "similar-a", "similar-b")
            self.fixture.git("commit", "-m", "signed similar public blobs")
            self.fixture.commit = self.fixture.git("rev-parse", "HEAD").decode().strip()
            self.fixture.tree = self.fixture.git("rev-parse", "HEAD^{tree}").decode().strip()
        self.fixture.export()
        self.fixture.import_capture()
        self.root = self.fixture.imported
        if hasattr(os, "lchmod"):
            os.lchmod(self.root / "sdk-link", 0o777)
        manifest = self.fixture.manifest()
        self.proof = {key: manifest[key] for key in ("commit", "tree", "pack", "objects", "entries", "source_bytes")}
        self.proof["signer"] = self.fixture.fingerprint
        self.commit = self.fixture.commit
        # These are the exact inert retained-import controls observed on the
        # selected guest sources, without teaching the new-import owner a fallback.
        controls = source._git_metadata(self.commit)
        controls["config"] = b"[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n\tlogallrefupdates = true\n"
        first = f"{'0' * 40} {self.commit} root <root@taira-linux-bootstrap.local> 123 +0000\n".encode()
        controls["logs/refs/heads/optimizations"] = first
        controls["logs/HEAD"] = first + f"{self.commit} {self.commit} root <root@taira-linux-bootstrap.local> 124 +0000\treset: moving to {self.commit}\n".encode()
        for name, raw in controls.items():
            path = self.root / ".git" / name
            path.write_bytes(raw)
            path.chmod(0o644)
        self.plan = dict(source=dict(root=str(self.root), commit=self.commit, tree=self.fixture.tree,
                                    signer=self.fixture.fingerprint, pack=manifest["pack"]),
                         git_controls=[dict(name=name, size=len(controls[name]), sha256=owner.sha(controls[name]))
                                       for name in owner.CONTROLS])
        objects = {row["object"]: row for row in manifest["objects"]}
        tracked = []
        for entry in manifest["entries"]:
            obj = objects.get(entry["object"])
            tracked.append(dict(path=entry["path"], mode=int(entry["mode"], 8) & 0o777
                                if entry["mode"] in {"100644", "100755"} else int(entry["mode"], 8),
                                git_blob_sha1=entry["object"], size=obj["size"] if obj else 40,
                                sha256=obj["sha256"] if obj else owner.sha(entry["object"].encode())))
        self.closure = dict(tracked_files=tracked)
        self.deployment = dict(runtime_root=str(self.fixture.case))
        self.stack = contextlib.ExitStack()
        self.addCleanup(self.stack.close)
        self.authority = self.stack.enter_context(patch.object(owner, "authority", return_value=self.closure))
        self.live = self.stack.enter_context(patch.object(owner, "live"))
        self.admission = owner.inspect(self.plan, self.deployment, self.proof)
        self.intent = owner.retirement_intent(self.admission, "a" * 64)
        self.parent = self.fixture.case / "custody"
        self.parent.mkdir(mode=0o700)

    def retire(self):
        return owner.retire_locked(self.plan, self.deployment, self.proof, self.admission, self.intent, self.parent)

    def quarantine(self):
        return Path(self.intent["quarantine"])


class RetainedSourceTests(RetainedSourceFixture, unittest.TestCase):
    def test_exact_signed_source_census_covers_empty_link_gitlink_and_git(self):
        owner.validate_admission(self.plan, self.proof, self.admission)
        rows = {row["path"]: row for row in self.admission["records"]}
        self.assertEqual(rows["empty"]["size"], 0)
        self.assertEqual(rows["sdk-link"]["kind"], "symlink")
        self.assertEqual(rows["vendor/dependency"]["kind"], "directory")
        self.assertTrue(any(name.endswith(".pack") for name in rows))
        self.assertTrue(any(name.endswith(".idx") for name in rows))

    def test_canonical_proof_verifies_actual_signature_and_full_tree(self):
        self.assertEqual(owner.canonical_proof(self.fixture.repo, self.plan, self.fixture.git), self.proof)
        changed = copy.deepcopy(self.plan)
        changed["source"]["signer"] = "F" * 40
        with self.assertRaisesRegex(ValueError, "signature or tree"):
            owner.canonical_proof(self.fixture.repo, changed, self.fixture.git)

    def test_modified_signed_file_rejected_before_archive(self):
        (self.root / "nested/data").write_bytes(b"changed bytes")
        with self.assertRaises((ValueError, owner.retry.RetryError)):
            owner.inspect(self.plan, self.deployment, self.proof)

    def test_untracked_private_name_rejected_before_opening_its_bytes(self):
        private = self.root / "runtime-private-key"
        private.write_bytes(b"must not be archived")
        private.chmod(0o600)
        with patch.object(common, "held", wraps=common.held) as held:
            with self.assertRaisesRegex((ValueError, owner.retry.RetryError), "missing or extra paths"):
                owner.inspect(self.plan, self.deployment, self.proof)
            self.assertFalse(any(Path(call.args[0]) == private for call in held.call_args_list))

    def test_extra_git_config_setting_rejected_even_when_pinned(self):
        path = self.root / ".git/config"
        raw = path.read_bytes() + b"[include]\n\tpath = /private/runtime/secret\n"
        path.write_bytes(raw)
        for row in self.plan["git_controls"]:
            if row["name"] == "config":
                row.update(size=len(raw), sha256=owner.sha(raw))
        with self.assertRaisesRegex((ValueError, owner.retry.RetryError), "unadmitted settings"):
            owner.inspect(self.plan, self.deployment, self.proof)

    def test_initialized_gitlink_rejected(self):
        (self.root / "vendor/dependency/private").write_bytes(b"not signed")
        with self.assertRaises((ValueError, owner.retry.RetryError)):
            owner.inspect(self.plan, self.deployment, self.proof)

    def test_link_retargeted_outside_tree_rejected(self):
        link = self.root / "sdk-link"
        link.unlink()
        link.symlink_to("/private/runtime/secret")
        with self.assertRaises((ValueError, owner.retry.RetryError)):
            owner.inspect(self.plan, self.deployment, self.proof)

    def test_archive_admission_cannot_redirect_selected_root(self):
        forged = copy.deepcopy(self.admission)
        forged["source_root"] = str(self.root.parent)
        with self.assertRaisesRegex((ValueError, owner.retry.RetryError), "binding"):
            owner.validate_admission(self.plan, self.proof, forged)

    def test_archive_admission_cannot_insert_traversal_or_extra(self):
        for name in ("../outside", "runtime-key", ".git/hooks/post-checkout"):
            forged = copy.deepcopy(self.admission)
            row = next(row for row in forged["records"] if row["kind"] == "file")
            row["path"] = name
            forged["records"].sort(key=lambda row: row["path"])
            with self.assertRaises((ValueError, owner.retry.RetryError)):
                owner.validate_admission(self.plan, self.proof, forged)

    def test_archive_admission_cannot_change_signed_digest(self):
        forged = copy.deepcopy(self.admission)
        next(row for row in forged["records"] if row["path"] == "Cargo.lock")["sha256"] = "0" * 64
        with self.assertRaisesRegex((ValueError, owner.retry.RetryError), "authority"):
            owner.validate_admission(self.plan, self.proof, forged)

    def test_full_retirement_and_repeat_keep_external_receipts(self):
        receipt = self.fixture.case / "native-terminal.json"
        receipt.write_bytes(b"retained exact terminal")
        result = self.retire()
        self.assertFalse(self.root.exists())
        self.assertFalse(self.quarantine().exists())
        self.assertFalse(result["deployment_authorized"])
        self.assertEqual(result, self.retire())
        self.assertEqual(receipt.read_bytes(), b"retained exact terminal")

    def test_all_source_names_are_quarantined_before_first_unlink(self):
        original = os.unlink
        def unlink(*args, **kwargs):
            self.assertFalse(self.root.exists())
            return original(*args, **kwargs)
        with patch.object(os, "unlink", side_effect=unlink):
            self.retire()

    def test_crash_after_root_rename_resumes(self):
        rename = common.rename_exclusive
        def interrupted(before, after, *args):
            rename(before, after, *args)
            if before == self.root:
                raise RuntimeError("after quarantine")
        with patch.object(common, "rename_exclusive", side_effect=interrupted):
            with self.assertRaisesRegex(RuntimeError, "after quarantine"):
                self.retire()
        self.assertTrue(self.quarantine().exists())
        self.assertFalse(self.root.exists())
        self.assertTrue(self.retire()["retired"])

    def test_crash_mid_batch_resumes_only_exact_missing_prefix(self):
        original = owner.unlink_member
        count = 0
        def interrupted(*args):
            nonlocal count
            original(*args)
            count += 1
            if count == 3:
                raise RuntimeError("mid batch")
        with patch.object(owner, "unlink_member", side_effect=interrupted):
            with self.assertRaisesRegex(RuntimeError, "mid batch"):
                self.retire()
        self.assertTrue(self.retire()["retired"])

    def test_crash_after_last_root_rmdir_resumes_completion(self):
        original = owner.unlink_member
        def interrupted(root, row):
            original(root, row)
            if row["path"] == ".":
                raise RuntimeError("root removed")
        with patch.object(owner, "unlink_member", side_effect=interrupted):
            with self.assertRaisesRegex(RuntimeError, "root removed"):
                self.retire()
        self.assertFalse(self.quarantine().exists())
        self.assertTrue(self.retire()["retired"])

    def test_noncontiguous_future_deletion_intent_is_rejected(self):
        rename = common.rename_exclusive
        def interrupted(before, after, *args):
            rename(before, after, *args)
            if before == self.root:
                raise RuntimeError("quarantined")
        with patch.object(owner, "BATCH", 2):
            with patch.object(common, "rename_exclusive", side_effect=interrupted):
                with self.assertRaises(RuntimeError):
                    self.retire()
            work = self.parent / self.intent["token"]
            common.marker(work, "0001.delete-intent.json", dict(start=2, end=4,
                          admission_sha256=self.intent["admission_sha256"]))
            with self.assertRaisesRegex(ValueError, "not a prefix"):
                self.retire()

    def test_unowned_missing_member_rejected(self):
        (self.root / "Cargo.lock").unlink()
        with self.assertRaises((ValueError, owner.retry.RetryError)):
            self.retire()

    def test_live_reference_after_quarantine_prevents_any_unlink(self):
        def check(*args, **kwargs):
            if self.quarantine().exists():
                raise ValueError("mapped source inode")
        self.live.side_effect = check
        with patch.object(os, "unlink") as unlink:
            with self.assertRaisesRegex((ValueError, owner.retry.RetryError), "mapped source"):
                self.retire()
            unlink.assert_not_called()
        self.assertTrue(self.quarantine().exists())

    def test_original_name_reappearance_prevents_delete(self):
        rename = common.rename_exclusive
        def replace(before, after, *args):
            rename(before, after, *args)
            if before == self.root:
                self.root.mkdir(mode=0o755)
        with patch.object(common, "rename_exclusive", side_effect=replace), patch.object(os, "unlink") as unlink:
            with self.assertRaisesRegex((ValueError, owner.retry.RetryError), "reappeared"):
                self.retire()
            unlink.assert_not_called()

    def test_replaced_root_parent_is_refused(self):
        self.admission["parent_identity"][1] += 1
        with self.assertRaises((ValueError, owner.retry.RetryError)):
            self.retire()

    def test_pending_record_prefix_recovery_and_changed_prefix_refusal(self):
        path = self.parent / "record.json"
        raw = owner.canonical({"exact": "record"})
        pending = path.with_name("." + path.name + ".pending")
        pending.write_bytes(raw[:5])
        pending.chmod(0o600)
        owner.write_new(path, raw)
        self.assertEqual(owner.read(path), raw)
        other = self.parent / "other.json"
        other.with_name(".other.json.pending").write_bytes(b"wrong")
        other.with_name(".other.json.pending").chmod(0o600)
        with self.assertRaisesRegex((ValueError, owner.retry.RetryError), "differs"):
            owner.write_new(other, raw)

    def test_sealed_pending_record_can_publish_without_rewrite(self):
        path = self.parent / "sealed.json"
        raw = owner.canonical({"sealed": True})
        pending = self.parent / ".sealed.json.pending"
        pending.write_bytes(raw)
        pending.chmod(0o400)
        owner.write_new(path, raw)
        self.assertEqual(owner.read(path), raw)

    def test_member_replaced_after_intent_is_not_deleted(self):
        original = owner.unlink_member
        victim = self.quarantine() / "Cargo.lock"
        def replace(root, row):
            if row["path"] == "Cargo.lock":
                victim.unlink()
                victim.write_bytes(b"replacement")
            original(root, row)
        with patch.object(owner, "unlink_member", side_effect=replace):
            with self.assertRaises((ValueError, owner.retry.RetryError)):
                self.retire()
        self.assertEqual(victim.read_bytes(), b"replacement")

    def test_capacity_counts_publication_overlap_and_reserve(self):
        plan, observed = owner.retirement_capacity(self.admission, "a" * 64)
        self.assertTrue(observed["passed"])
        self.assertEqual(plan["allocations"][1]["bytes"], 256 * 1024**2)
        self.assertGreater(plan["allocations"][0]["bytes"], 3 * len(owner.canonical(self.admission)))

    def archive_fixture(self):
        archive = self.fixture.case / "archive"
        archive.mkdir(mode=0o700)
        for name, value in (("plan", self.plan), ("proof", self.proof), ("admission", self.admission)):
            owner.write_new(archive / (name + ".json"), owner.canonical(value))
        payload = bytearray()
        for row in self.admission["records"]:
            if row["kind"] == "file":
                payload.extend((self.root / row["path"]).read_bytes())
            elif row["kind"] == "symlink":
                payload.extend(os.fsencode(os.readlink(self.root / row["path"])))
        path = archive / "payload.bin"
        path.write_bytes(payload)
        path.chmod(0o400)
        (archive / "archive.stderr").touch(mode=0o600)
        result = dict(schema=owner.SCHEMA, archive_complete=True,
            admission_sha256=owner.sha(owner.canonical(self.admission)),
            proof_sha256=owner.sha(owner.canonical(self.proof)), plan_sha256=owner.sha(owner.canonical(self.plan)),
            payload_sha256=owner.sha(payload), retirement_authorized=False)
        owner.write_new(archive / "completed.json", owner.canonical(result))
        return archive, result

    def stream_archive(self, name="stream-archive", transform=lambda raw: raw, *,
                       inspect_values=None, session_failure=False):
        self.plan["guest_ssh"] = {}
        self.admission = owner.inspect(self.plan, self.deployment, self.proof)
        output = self.fixture.case / name

        @contextlib.contextmanager
        def session(route, envelope, modules, evidence):
            evidence.touch(mode=0o600)
            with tempfile.TemporaryFile(dir=self.fixture.case) as stream:
                with contextlib.ExitStack() as stack:
                    stack.enter_context(patch.object(common, "authority_locks", return_value=contextlib.nullcontext()))
                    if inspect_values is not None:
                        stack.enter_context(patch.object(owner, "inspect", side_effect=inspect_values))
                    owner.archive_stream(self.plan, self.deployment, self.proof, self.admission, stream.fileno())
                stream.seek(0)
                raw = transform(stream.read())
                stream.seek(0)
                stream.truncate()
                stream.write(raw)
                stream.seek(0)
                yield owner.Reader(stream.fileno())
                if session_failure:
                    raise ValueError("source operation failed; retain evidence")

        with patch.object(owner, "session", side_effect=session), patch.object(common, "allocation"), patch.object(owner, "validate_plan", side_effect=lambda value: value):
            result = owner.archive_local(self.plan, self.deployment, self.proof, self.admission, {}, output)
            verified = owner.verify_archive(output)
        return output, result, verified

    def test_changed_admission_or_remote_failure_never_publishes_completion(self):
        self.plan["guest_ssh"] = {}
        admission = owner.inspect(self.plan, self.deployment, self.proof)
        changed = {**admission, "payload_bytes": admission["payload_bytes"] + 1}
        cases = [("initial-admission", [changed], False),
                 ("final-admission", [admission, changed], False),
                 ("remote-exit", None, True)]
        for name, values, failure in cases:
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "admission changed|changed while streaming|source operation failed"):
                self.stream_archive(name, inspect_values=values, session_failure=failure)
            self.assertFalse((self.fixture.case / name / "completed.json").exists())
        self.assertEqual(owner.inspect(self.plan, self.deployment, self.proof), admission)

    def test_compressed_stream_retains_exact_uncompressed_archive_and_receipt(self):
        output, result, verified = self.stream_archive()
        expected = bytearray()
        for row in self.admission["records"]:
            if row["kind"] == "file":
                expected.extend((self.root / row["path"]).read_bytes())
            elif row["kind"] == "symlink":
                expected.extend(os.fsencode(os.readlink(self.root / row["path"])))
        self.assertEqual((output / "payload.bin").read_bytes(), expected)
        self.assertEqual(result["payload_sha256"], owner.sha(expected))
        self.assertEqual(verified[:3], (self.plan, self.proof, self.admission))
        self.assertEqual(stat.S_IMODE((output / "payload.bin").stat().st_mode), 0o400)
        self.assertFalse(result["retirement_authorized"])
        self.assertEqual(set(result), {"schema", "archive_complete", "admission_sha256", "plan_sha256",
                                      "proof_sha256", "payload_sha256", "retirement_authorized"})

    def test_invalid_stream_never_publishes_archive_completion(self):
        def replace_header(raw, **updates):
            length = struct.unpack(">I", raw[:4])[0]
            value = owner.decode(raw[4:4 + length])
            value.update(updates)
            header = owner.canonical(value)
            return struct.pack(">I", len(header)) + header + raw[4 + length:]

        def corrupt_chunk(raw):
            position = 4 + struct.unpack(">I", raw[:4])[0]
            compressed = struct.unpack(">II", raw[position:position + 8])[1]
            end = position + 8 + compressed
            return raw[:end - 1] + bytes([raw[end - 1] ^ 1]) + raw[end:]

        def wrong_final_admission(raw):
            digest = owner.sha(owner.canonical(self.admission)).encode()
            position = raw.rfind(digest)
            self.assertGreater(position, 0)
            return raw[:position] + b"0" * len(digest) + raw[position + len(digest):]

        mutations = {
            "wrong-codec": lambda raw: replace_header(raw, payload_encoding="raw"),
            "wrong-admission": lambda raw: replace_header(raw, admission_sha256="0" * 64),
            "truncated-header": lambda raw: raw[:3],
            "corrupt-member": corrupt_chunk,
            "wrong-final-admission": wrong_final_admission,
            "missing-final-frame": lambda raw: raw[:-4],
            "extra-payload": lambda raw: raw[:4 + struct.unpack(">I", raw[:4])[0]] + b"extra payload" + raw[4 + struct.unpack(">I", raw[:4])[0]:],
            "trailing-transport": lambda raw: raw + b"extra",
        }
        for name, mutate in mutations.items():
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.stream_archive(name, mutate)
            self.assertFalse((self.fixture.case / name / "completed.json").exists())

    def test_offhost_archive_rehash_and_exact_held_binding(self):
        archive, _ = self.archive_fixture()
        with patch.object(owner, "validate_plan", side_effect=lambda value: value):
            expected = owner.verify_archive(archive)
            with owner.held_archive(archive, expected):
                pass
        self.assertEqual(expected[2], self.admission)

    def test_internally_valid_archive_swap_before_dispatch_is_refused(self):
        archive, completed = self.archive_fixture()
        with patch.object(owner, "validate_plan", side_effect=lambda value: value):
            expected = owner.verify_archive(archive)
            changed = copy.deepcopy(self.admission)
            changed["records"][0]["allocated_bytes"] += 512
            changed["allocated_bytes"] += 512
            completed["admission_sha256"] = owner.sha(owner.canonical(changed))
            for name, value in (("admission", changed), ("completed", completed)):
                path = archive / (name + ".json")
                path.unlink()
                owner.write_new(path, owner.canonical(value))
            self.assertNotEqual(owner.verify_archive(archive), expected)
            with self.assertRaisesRegex(ValueError, "digest differs"):
                with owner.held_archive(archive, expected):
                    self.fail("replacement archive must not authorize dispatch")

    def test_same_length_archive_payload_replacement_is_refused(self):
        archive, _ = self.archive_fixture()
        with patch.object(owner, "validate_plan", side_effect=lambda value: value):
            expected = owner.verify_archive(archive)
            path = archive / "payload.bin"
            size = path.stat().st_size
            path.unlink()
            path.write_bytes(b"x" * size)
            path.chmod(0o400)
            with self.assertRaisesRegex(ValueError, "digest differs"):
                with owner.held_archive(archive, expected):
                    self.fail("changed archive payload must not authorize dispatch")

    def test_reverse_index_trailing_payload_is_rejected(self):
        pack = next((self.root / ".git/objects/pack").glob("*.pack"))
        index = pack.with_suffix(".idx").read_bytes()
        reverse = pack.with_suffix(".rev").read_bytes()
        trailer = pack.read_bytes()[-20:]
        owner.validate_pack_indexes(index, reverse, self.proof, trailer)
        with self.assertRaisesRegex(ValueError, "reverse index"):
            owner.validate_pack_indexes(index, reverse + b"private payload", self.proof, trailer)

    def test_git_index_unadmitted_extension_is_rejected(self):
        raw = (self.root / ".git/index").read_bytes()
        body = raw[:-20] + b"PRIV" + (7).to_bytes(4, "big") + b"private"
        with self.assertRaisesRegex(ValueError, "unadmitted extension"):
            owner.validate_git_index(body + hashlib.sha1(body).digest(), self.proof)


class RetainedDeltaPackTests(RetainedSourceFixture, unittest.TestCase):
    delta_fixture = True

    def setUp(self):
        super().setUp()
        self.install_pack([row["object"] for row in self.proof["objects"]])
        rows = source._git(self.root, "verify-pack", "-v", str(self.pack.with_suffix(".idx"))).splitlines()
        self.assertTrue(any(len(row.split()) == 7 and len(row.split()[0]) == 40 for row in rows),
                        "fixture must contain a real Git delta object")

    def install_pack(self, identifiers):
        payload = self.fixture.git("pack-objects", "--stdout", "--no-reuse-delta", "--no-reuse-object",
                                   "--window=50", "--depth=50", "--delta-base-offset",
                                   payload="".join(oid + "\n" for oid in identifiers).encode())
        directory = self.root / ".git/objects/pack"
        for path in directory.iterdir():
            path.unlink()
        source._git(self.root, "index-pack", "--stdin", "--rev-index", payload=payload)
        source._freeze_new_pack(self.root)
        self.pack = next(directory.glob("*.pack"))
        self.proof["pack"] = dict(size=len(payload), sha256=owner.sha(payload))
        self.plan["source"]["pack"] = dict(self.proof["pack"])

    def mutate_pack(self, payload, *, repin=False):
        self.pack.chmod(0o600)
        self.pack.write_bytes(payload)
        self.pack.chmod(0o444)
        if repin:
            self.proof["pack"] = dict(size=len(payload), sha256=owner.sha(payload))
            self.plan["source"]["pack"] = dict(self.proof["pack"])

    def inspect(self):
        return owner.inspect(self.plan, self.deployment, self.proof)

    def test_exact_delta_closure_archives_and_retires_but_shipping_still_refuses(self):
        with common.held(self.pack, digest=self.proof["pack"]["sha256"], mode=0o444) as (fd, _, _):
            with self.assertRaisesRegex(source.SourceCaptureError, "without deltas"):
                source._validate_pack(fd, self.proof)
            owner.validate_retained_pack(fd, self.proof)
        self.admission = self.inspect()
        self.intent = owner.retirement_intent(self.admission, "a" * 64)
        archive, _ = RetainedSourceTests.archive_fixture(self)
        with patch.object(owner, "validate_plan", side_effect=lambda value: value):
            _, proof, admission, _ = owner.verify_archive(archive)
        self.assertEqual(proof, self.proof)
        self.assertEqual(admission, self.admission)
        self.assertTrue(self.retire()["retired"])
        self.assertFalse(self.root.exists())
        self.assertTrue((archive / "payload.bin").exists())

    def test_changed_transport_bytes_refused_before_archive(self):
        payload = bytearray(self.pack.read_bytes())
        payload[20] ^= 1
        self.mutate_pack(bytes(payload))
        with self.assertRaisesRegex(ValueError, "pack differs from receipt"):
            self.inspect()

    def test_bad_trailer_refused_even_with_recomputed_transport_pin(self):
        payload = self.pack.read_bytes()
        self.mutate_pack(payload[:-1] + bytes([payload[-1] ^ 1]), repin=True)
        with self.assertRaisesRegex(ValueError, "trailer checksum"):
            self.inspect()

    def test_header_object_count_refused_even_with_valid_checksums(self):
        payload = self.pack.read_bytes()
        body = payload[:8] + (len(self.proof["objects"]) + 1).to_bytes(4, "big") + payload[12:-20]
        self.mutate_pack(body + hashlib.sha1(body).digest(), repin=True)
        with self.assertRaisesRegex(ValueError, "header/object census"):
            self.inspect()

    def test_foreign_object_replacement_with_same_count_refused(self):
        foreign = self.fixture.git("hash-object", "-w", "--stdin", payload=b"foreign unsigned payload").decode().strip()
        # Keep the header count and valid Git pack/index checksums, but replace
        # one authorized object. The closed index census must still reject it.
        identifiers = [row["object"] for row in self.proof["objects"]]
        victim = next(row["object"] for row in self.proof["objects"] if row["type"] == "blob" and row["size"] == 0)
        identifiers[identifiers.index(victim)] = foreign
        self.install_pack(identifiers)
        with self.assertRaisesRegex(ValueError, "pack index object census"):
            self.inspect()

    def test_canonical_object_metadata_mismatch_refused(self):
        next(row for row in self.proof["objects"] if row["type"] == "commit")["sha256"] = "0" * 64
        with self.assertRaisesRegex(source.SourceCaptureError, "complete signed object/tree inventory"):
            self.inspect()

    def test_pack_index_checksum_change_refused(self):
        index = self.pack.with_suffix(".idx")
        raw = index.read_bytes()
        index.chmod(0o600)
        index.write_bytes(raw[:-1] + bytes([raw[-1] ^ 1]))
        index.chmod(0o444)
        with self.assertRaisesRegex(ValueError, "pack index checksum"):
            self.inspect()

    def test_envelope_rejects_wrong_digest_size_version_and_truncation(self):
        with common.held(self.pack, mode=0o444) as (fd, _, _):
            wrong = copy.deepcopy(self.proof)
            wrong["pack"]["sha256"] = "0" * 64
            with self.assertRaisesRegex(ValueError, "transport digest"):
                owner.validate_retained_pack(fd, wrong)
            wrong["pack"]["size"] -= 1
            with self.assertRaisesRegex(ValueError, "header/object census"):
                owner.validate_retained_pack(fd, wrong)
        payload = self.pack.read_bytes()
        for replacement in (b"PACK" + (3).to_bytes(4, "big") + payload[8:], payload[:20]):
            with self.subTest(size=len(replacement)):
                self.mutate_pack(replacement, repin=True)
                with self.assertRaisesRegex(ValueError, "envelope|header/object census"):
                    self.inspect()


class ClosedPlanTests(unittest.TestCase):
    def plan(self):
        runtime = "/private/runtime/taira-public-reset"
        ref = lambda name: dict(path=runtime + "/" + name, sha256="a" * 64)
        return dict(schema=owner.SCHEMA, provider="macstadium-dublin",
            controller=dict(commit="b" * 40, signer="C" * 40), guest_ssh={}, backing_ssh={},
            backing_path="/Users/administrator/approved-backing", deployment=ref("deployment.json"),
            current_inventory=ref("assembly94/inventory.json"),
            units=[dict(path=f"/etc/systemd/system/iroha3d-taira-validator-{i}.service", sha256="d" * 64) for i in range(1, 5)],
            releases=[dict(inventory=ref("assembly85/inventory.json"), terminal=ref("journal-v1/rolled-back/" + "a" * 64 + ".json"),
                           binary_manifest=ref("release85/verified-manifest.json"), source_manifest=ref("source-transfer85/verified-manifest.json"))],
            source=dict(root="/opt/iroha/taira-source-release85-" + "e" * 40, commit="e" * 40, tree="f" * 40,
                        signer="C" * 40, pack=dict(size=100, sha256="1" * 64)),
            source_closure=ref("continuation85/source-manifest.json"),
            git_controls=[dict(name=name, size=1, sha256="2" * 64) for name in owner.CONTROLS])

    def test_one_explicit_compatible_source_plan(self):
        plan = self.plan()
        self.assertEqual(owner.validate_plan(plan), plan)
        for number in (82, 83, 84, 94):
            changed = copy.deepcopy(plan)
            changed["releases"][0]["inventory"]["path"] = f"/private/runtime/taira-public-reset/assembly{number}/inventory.json"
            changed["source"]["root"] = f"/opt/iroha/taira-source-release{number}-" + "e" * 40
            with self.assertRaises(ValueError):
                owner.validate_plan(changed)

    def test_multiple_sources_or_extra_field_are_rejected(self):
        plan = self.plan()
        for altered in ({**plan, "source_extra": "/private/runtime/secret"},
                        {**plan, "releases": plan["releases"] * 2}):
            with self.assertRaises(ValueError):
                owner.validate_plan(altered)


if __name__ == "__main__":
    unittest.main()
