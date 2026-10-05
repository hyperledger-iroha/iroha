"""Real signed Git and native filesystem append/recovery tests; no Cargo or SSH."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import sys
import unittest
from unittest import mock

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS))
sys.path.insert(0, str(Path(__file__).parent))
import taira_source_capture as source
import taira_source_store as store
import taira_release as release
import test_taira_source_capture as capture_fixture


class BuilderSourceStoreTests(unittest.TestCase):
    setUpClass = classmethod(capture_fixture.SignedSourceCaptureTests.setUpClass.__func__)
    tearDownClass = classmethod(capture_fixture.SignedSourceCaptureTests.tearDownClass.__func__)
    command = classmethod(capture_fixture.SignedSourceCaptureTests.command.__func__)
    git = capture_fixture.SignedSourceCaptureTests.git

    def setUp(self):
        capture_fixture.SignedSourceCaptureTests.setUp(self)
        for relative in release.BUILD_SOURCES:
            path = self.repo / relative
            path.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
            path.write_bytes(("exact controller " + relative + "\n\n").encode())
        (self.repo / "native.rs").write_bytes(b"const VERSION: u8 = 1;\n")
        self.git("add", "scripts", "native.rs")
        self.git("commit", "-m", "original native builder source")
        self.base_commit = self.git("rev-parse", "HEAD").decode().strip()
        self.base_tree = self.git("rev-parse", "HEAD^{tree}").decode().strip()
        self.capture = self.case / "base-capture"
        source.export_source(self.repo, self.base_commit, self.base_tree, self.fingerprint, self.capture)
        source.import_source(self.capture / "source.pack", self.capture / "source-capture.json",
                             self.base_commit, self.base_tree, self.fingerprint, self.imported)
        target = self.imported / "target"
        target.mkdir(mode=0o700)
        self.target = target / "taira-macos-client"
        self.target.mkdir(mode=0o700)
        with release.cargo_lane(self.imported, self.target, "release"):
            with release.source_lane(self.imported, self.target) as (captured, _):
                release.capture_source(self.imported, captured, self.target, self.base_commit, release.commit_entries(self.imported, self.base_commit))
        self.warm = self.target / "warm-marker"
        self.warm.write_bytes(b"original compiler cache must remain\n")
        self.warm_pin = store._reference(self.warm)
        self._successor()
        self.original_index = store._reference(self.imported / ".git/index")
        self.original_head = store._reference(self.imported / ".git/HEAD")
        self.original_source = store._reference(self.imported / "native.rs")
        self.original_controller = store._reference(self.imported / "scripts/taira_release.py")
        self.initial_receipt = None

    def capture_input(self, directory, commit, tree):
        path = directory / "source-capture.json"
        return {"manifest": str(path), "sha256": source.stable_hash_path(path).sha256,
                "commit": commit, "tree": tree, "signer_fingerprint": self.fingerprint}

    def initialize(self, captures=None, commit=None, tree=None, **kwargs):
        return store.initialize_store(self.imported, captures or [self.capture_input(self.capture, self.base_commit, self.base_tree)],
                                      commit or self.base_commit, tree or self.base_tree, self.fingerprint, "0" * 32, **kwargs)

    def initial(self):
        if self.initial_receipt is None:
            self.initial_receipt = self.initialize()
        return self.initial_receipt

    def _successor(self):
        (self.repo / "native.rs").write_bytes(b"const VERSION: u8 = 2;\n")
        self.git("add", "native.rs")
        self.git("commit", "-m", "Rust-only successor")
        self.commit = self.git("rev-parse", "HEAD").decode().strip()
        self.tree = self.git("rev-parse", "HEAD^{tree}").decode().strip()
        self.new_capture = self.case / ("new-capture-" + self.commit)
        source.export_source(self.repo, self.commit, self.tree, self.fingerprint, self.new_capture)

    def append(self, operation="1" * 32, **kwargs):
        initial = self.initial()
        arguments = {"receipt": Path(initial["receipt"]["path"]), "expected_receipt_sha256": initial["receipt"]["sha256"]}
        arguments.update(kwargs)
        return store.append_source(self.imported, **arguments,
                                   pack=self.new_capture / "source.pack", manifest=self.new_capture / "source-capture.json",
                                   manifest_sha256=source.stable_hash_path(self.new_capture / "source-capture.json").sha256,
                                   commit=self.commit, tree=self.tree, signer=self.fingerprint, operation_id=operation)

    def verify(self, result):
        return store.verify_store(self.imported, Path(result["receipt"]["path"]), result["receipt"]["sha256"],
                                  self.commit, self.tree, self.fingerprint)

    def intent(self, operation="1" * 32):
        return store._reference(self.imported / "target/.taira-source-store" / operation / "intent.json")

    def unchanged(self):
        self.assertEqual(store._reference(self.warm), self.warm_pin)
        for pin in (self.original_index, self.original_head, self.original_source, self.original_controller):
            self.assertEqual(store._reference(Path(pin["path"])), pin)

    def test_real_signed_append_preserves_checkout_controls_index_and_warm_lane(self):
        result = self.append()
        self.assertEqual(self.verify(result)["capture_count"], 2)
        self.assertEqual(len(list((self.imported / ".git/objects/pack").iterdir())), 6)
        self.assertEqual(source._git(self.imported, "rev-parse", "HEAD").strip().decode(), self.base_commit)
        self.unchanged()
        with self.assertRaisesRegex(source.SourceCaptureError, "exactly one complete pack"):
            source.verify_import(self.imported, self.capture / "source-capture.json", self.base_commit, self.base_tree, self.fingerprint)

    def test_raw_signed_parent_survives_shallow_native_git_graph(self):
        stage = self.case / "shallow-objects"
        stage.mkdir(mode=0o700)
        manifest, key, _ = source._manifest(self.new_capture / "source-capture.json", self.commit, self.tree, self.fingerprint)
        pack = self.new_capture / "source.pack"
        source._import_object_database(stage, pack, source.stable_hash_path(pack), manifest, key)
        self.assertEqual(source._git(stage, "show", "--no-patch", "--format=%P", self.commit), b"\n")
        self.assertEqual(store._raw_parent(stage, self.commit), self.base_commit)
        self.assertNotIn(self.base_commit.encode(), source._git(stage, "cat-file", "--batch-all-objects", "--batch-check=%(objectname)"))

    def test_controller_raw_trailing_newlines_are_not_stripped(self):
        self.assertTrue((self.imported / "scripts/taira_release.py").read_bytes().endswith(b"\n\n"))
        result = self.append()
        self.verify(result)
        self.unchanged()

    def test_interruption_after_each_durable_boundary_resumes_same_exact_inodes(self):
        # Separate genuine repositories for each fault, never reset/clean a
        # previous owner to manufacture another recovery arrangement.
        phase = "pack_1_durable"
        with mock.patch.object(store, "_fault", side_effect=lambda current: (_ for _ in ()).throw(RuntimeError(phase)) if current == phase else None):
            with self.assertRaisesRegex(RuntimeError, phase):
                self.append()
        intent = self.intent()
        with self.assertRaisesRegex(source.SourceCaptureError, "independently pinned durable intent"):
            self.append()
        with self.assertRaisesRegex(source.SourceCaptureError, "incomplete or foreign"):
            self.append("2" * 32)
        result = self.append(intent_sha256=intent["sha256"])
        self.verify(result)
        self.unchanged()

    def test_lost_terminal_ack_replays_read_only(self):
        with mock.patch.object(store, "_fault", side_effect=lambda phase: (_ for _ in ()).throw(RuntimeError("lost ack")) if phase == "receipt_durable" else None):
            with self.assertRaisesRegex(RuntimeError, "lost ack"):
                self.append()
        directory = self.imported / "target/.taira-source-store" / ("1" * 32)
        before = store._reference(directory / "receipt.json")
        packs = [store._reference(path, source.MAX_PACK_BYTES) for path in sorted((self.imported / ".git/objects/pack").iterdir())]
        with mock.patch.object(store, "_rename_noreplace", side_effect=AssertionError("terminal replay must not rename")):
            result = self.append(intent_sha256=self.intent()["sha256"])
        self.assertEqual(result["receipt"], before)
        self.assertEqual([store._reference(Path(row["path"]), source.MAX_PACK_BYTES) for row in packs], packs)
        self.verify(result)

    def test_same_byte_foreign_published_inode_is_preserved_and_refused(self):
        with mock.patch.object(store, "_fault", side_effect=lambda phase: (_ for _ in ()).throw(RuntimeError("stop")) if phase == "pack_1_durable" else None):
            with self.assertRaises(RuntimeError):
                self.append()
        intent = self.intent()
        record, _ = store._read_record(Path(intent["path"]), intent["sha256"])
        path = self.imported / ".git/objects/pack" / Path(record["staged_pack_files"][0]["path"]).name
        payload = path.read_bytes()
        path.unlink()
        path.write_bytes(payload)
        path.chmod(0o444)
        foreign = store._reference(path, source.MAX_PACK_BYTES)
        with self.assertRaisesRegex(source.SourceCaptureError, "retained file differs"):
            self.append(intent_sha256=intent["sha256"])
        self.assertEqual(store._reference(path, source.MAX_PACK_BYTES), foreign)
        self.assertFalse((Path(intent["path"]).parent / "receipt.json").exists())

    def test_publication_detects_same_inode_same_size_write_with_restored_mtime(self):
        path = self.case / "retained-publication-source"
        destination = self.case / "retained-publication-destination"
        path.write_bytes(b"original")
        writable = os.open(path, os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC)
        path.chmod(0o400)
        before = path.lstat()
        publication = []
        try:
            with self.assertRaisesRegex(source.SourceCaptureError, "retained file bytes changed"):
                with store._retained_file(path, 8, publication=publication) as (retained, reference):
                    self.assertEqual(os.pread(retained, 8, 0), b"original")
                    self.assertEqual(os.pwrite(writable, b"modified", 0), 8)
                    os.utime(path, ns=(before.st_atime_ns, before.st_mtime_ns))
                    parent = store._stable_directory(self.case.lstat())
                    store._rename_noreplace(path, destination, lambda: None, parent, parent)
                    publication.append(destination)
                    # Only ctime may legitimately change for this publication.
                    self.assertEqual(store._file_identity(os.fstat(retained))[:-1], reference["identity"][:-1])
                    self.assertEqual(os.pread(retained, 8, 0), b"modified")
            self.assertFalse(path.exists())
            self.assertEqual(destination.read_bytes(), b"modified")
        finally:
            os.close(writable)

    def test_changed_selected_controller_refused_before_live_pack_publication(self):
        path = self.repo / release.BOOTSTRAP_SOURCES[0]
        path.write_bytes(path.read_bytes() + b"changed\n")
        self.git("add", str(path.relative_to(self.repo)))
        self.git("commit", "--amend", "--no-edit")
        self.commit = self.git("rev-parse", "HEAD").decode().strip()
        self.tree = self.git("rev-parse", "HEAD^{tree}").decode().strip()
        self.new_capture = self.case / "changed-bootstrap"
        source.export_source(self.repo, self.commit, self.tree, self.fingerprint, self.new_capture)
        before = set((self.imported / ".git/objects/pack").iterdir())
        with self.assertRaisesRegex(source.SourceCaptureError, "changes a selected build Bootstrap"):
            self.append()
        self.assertEqual(set((self.imported / ".git/objects/pack").iterdir()), before)
        self.unchanged()

    def test_real_existing_cargo_and_source_leases_refuse_append(self):
        with release.cargo_lane(self.imported, self.target, "release"):
            with self.assertRaisesRegex(release.PrepareError, "still running"):
                self.append()
        with release.source_lane(self.imported, self.target):
            with self.assertRaisesRegex(release.PrepareError, "still running"):
                self.append()
        self.assertFalse((self.imported / "target/.taira-source-store").exists())
        self.unchanged()

    def test_original_index_or_source_inode_substitution_refuses(self):
        original = self.imported / ".git/index"
        payload = original.read_bytes()
        original.unlink()
        original.write_bytes(payload)
        original.chmod(0o600)
        # Initial admission can bind a presently valid original index; once an
        # independently pinned receipt exists its exact inode becomes immutable.
        result = self.append()
        original.unlink()
        original.write_bytes(payload)
        original.chmod(0o600)
        foreign = store._reference(original)
        with self.assertRaisesRegex(source.SourceCaptureError, "inode changed"):
            self.verify(result)
        self.assertEqual(store._reference(original), foreign)

    def test_exact_signed_union_rejects_unadmitted_native_history_object(self):
        result = self.append()
        source._git(self.imported, "hash-object", "-w", "--stdin", payload=b"unadmitted object\n")
        with self.assertRaises(source.SourceCaptureError):
            self.verify(result)
        _, _, manifests = store._load_receipt(self.imported, Path(result["receipt"]["path"]), result["receipt"]["sha256"])
        with self.assertRaisesRegex(source.SourceCaptureError, "exact authenticated union"):
            store._verify_union(self.imported, manifests)

    def _interrupted(self, phase):
        with mock.patch.object(store, "_fault", side_effect=lambda current: (_ for _ in ()).throw(RuntimeError(phase)) if current == phase else None):
            with self.assertRaisesRegex(RuntimeError, phase):
                self.append()
        result = self.append(intent_sha256=self.intent()["sha256"])
        self.verify(result)
        self.unchanged()

    def test_prepublication_intent_recovery(self):
        self._interrupted("intent_durable")

    def test_second_pack_publication_recovery(self):
        self._interrupted("pack_2_durable")

    def test_third_pack_publication_recovery(self):
        self._interrupted("pack_3_durable")

    def test_same_byte_foreign_terminal_receipt_is_refused(self):
        result = self.append()
        path = Path(result["receipt"]["path"])
        payload = path.read_bytes()
        path.unlink()
        path.write_bytes(payload)
        path.chmod(0o400)
        foreign = store._reference(path)
        with self.assertRaisesRegex(source.SourceCaptureError, "reserved inode"):
            self.append(intent_sha256=result["intent"]["sha256"])
        self.assertEqual(store._reference(path), foreign)

    def test_layout_or_lease_substitution_after_intent_refuses_before_live_effect(self):
        def substitute(phase):
            if phase == "intent_durable":
                lock = self.target / ".taira-build-lane/session.lock"
                lock.rename(lock.with_name("original-held.lock"))
                lock.write_bytes(b"")
                lock.chmod(0o600)
        before = set((self.imported / ".git/objects/pack").iterdir())
        with mock.patch.object(store, "_fault", side_effect=substitute), self.assertRaisesRegex(source.SourceCaptureError, "lease path changed"):
            self.append()
        self.assertEqual(set((self.imported / ".git/objects/pack").iterdir()), before)

    def test_signed_object_collision_and_finite_union_refuse(self):
        original, _, _ = source._manifest(self.capture / "source-capture.json", self.base_commit, self.base_tree, self.fingerprint)
        successor, _, _ = source._manifest(self.new_capture / "source-capture.json", self.commit, self.tree, self.fingerprint)
        common = set(row["object"] for row in original["objects"]) & set(row["object"] for row in successor["objects"])
        row = next(row for row in successor["objects"] if row["object"] in common)
        row["sha256"] = "0" * 64
        with self.assertRaisesRegex(source.SourceCaptureError, "object collision"):
            store._union([original, successor])
        with self.assertRaisesRegex(source.SourceCaptureError, "capture count"):
            store._union([original] * (store.MAX_CAPTURES + 1))

    def test_capacity_refuses_before_staging_or_native_pack_writes(self):
        with mock.patch.object(source, "_check_source_capacity", side_effect=source.SourceCaptureError("capacity refused")), \
             mock.patch.object(store, "_mkdir", side_effect=AssertionError("must preflight before stage")), \
             mock.patch.object(source, "_import_object_database", side_effect=AssertionError("must preflight before native write")):
            with self.assertRaisesRegex(source.SourceCaptureError, "capacity refused"):
                self.append()
        self.assertFalse((self.imported / "target/.taira-source-store").exists())

    def test_second_successor_uses_pinned_receipt_and_union_without_changing_head(self):
        first = self.append()
        (self.repo / "native.rs").write_bytes(b"const VERSION: u8 = 3;\n")
        self.git("add", "native.rs")
        self.git("commit", "-m", "second signed successor")
        self.commit = self.git("rev-parse", "HEAD").decode().strip()
        self.tree = self.git("rev-parse", "HEAD^{tree}").decode().strip()
        self.new_capture = self.case / "second-successor"
        source.export_source(self.repo, self.commit, self.tree, self.fingerprint, self.new_capture)
        second = self.append("2" * 32, receipt=Path(first["receipt"]["path"]), expected_receipt_sha256=first["receipt"]["sha256"])
        self.assertEqual(self.verify(second)["capture_count"], 3)
        self.unchanged()

    def test_wrong_signed_parent_refuses_without_live_mutation(self):
        (self.repo / "native.rs").write_bytes(b"const VERSION: u8 = 3;\n")
        self.git("add", "native.rs")
        self.git("commit", "-m", "skip the authenticated frontier")
        self.commit = self.git("rev-parse", "HEAD").decode().strip()
        self.tree = self.git("rev-parse", "HEAD^{tree}").decode().strip()
        self.new_capture = self.case / "skipped-successor"
        source.export_source(self.repo, self.commit, self.tree, self.fingerprint, self.new_capture)
        before = set((self.imported / ".git/objects/pack").iterdir())
        with self.assertRaisesRegex(source.SourceCaptureError, "native parent differs"):
            self.append()
        self.assertEqual(set((self.imported / ".git/objects/pack").iterdir()), before)

    def test_actual_maintained_cli_append_and_verify_store_roundtrip(self):
        manifest = self.new_capture / "source-capture.json"
        initial = self.initial()
        entrypoint = [sys.executable, "-B", str(SCRIPTS / "taira_source_capture.py")]
        output = self.command([*entrypoint, "append", "--source-root", str(self.imported),
                               "--receipt", initial["receipt"]["path"], "--expected-receipt-sha256", initial["receipt"]["sha256"],
                               "--pack", str(self.new_capture / "source.pack"),
                               "--manifest", str(manifest), "--expected-manifest-sha256", source.stable_hash_path(manifest).sha256,
                               "--expected-commit", self.commit, "--expected-tree", self.tree, "--expected-signer", self.fingerprint,
                               "--operation-id", "1" * 32])
        result = source.load_json_object(output, "actual append CLI output")
        verified = self.command([*entrypoint, "verify-store", "--source-root", str(self.imported),
                                 "--receipt", result["receipt"]["path"], "--expected-receipt-sha256", result["receipt"]["sha256"],
                                 "--expected-commit", self.commit, "--expected-tree", self.tree, "--expected-signer", self.fingerprint])
        self.assertEqual(source.load_json_object(verified, "actual verify CLI output")["capture_count"], 2)
        self.assertFalse(result["activated"])
        self.unchanged()

    def _qualified_existing_pack(self, directory, commit, tree, name):
        """Create the real independently verified incumbent arrangement."""
        manifest, key, _ = source._manifest(directory / "source-capture.json", commit, tree, self.fingerprint)
        stage = self.case / name
        stage.mkdir(mode=0o700)
        pack = directory / "source.pack"
        source._import_object_database(stage, pack, source.stable_hash_path(pack), manifest, key)
        for reference in store._pack_files(stage, manifest):
            path = Path(reference["path"])
            destination = self.imported / ".git/objects/pack" / path.name
            publication = []
            with store._retained_file(path, source.MAX_PACK_BYTES, reference, publication=publication):
                store._rename_noreplace(path, destination, lambda: None, store._stable_directory(path.parent.lstat()),
                                        store._stable_directory(destination.parent.lstat()))
                publication.append(destination)
        with release.cargo_lane(self.imported, self.target, "release"):
            with release.source_lane(self.imported, self.target) as (captured, _):
                release.capture_source(self.imported, captured, self.target, commit, release.commit_entries(self.imported, commit))

    def _current_two_pack_captures(self):
        self._qualified_existing_pack(self.new_capture, self.commit, self.tree, "qualified-incumbent")
        return [self.capture_input(self.capture, self.base_commit, self.base_tree), self.capture_input(self.new_capture, self.commit, self.tree)]

    def test_initialize_current_two_pack_union_is_records_only(self):
        captures = self._current_two_pack_captures()
        packs = [store._reference(path, source.MAX_PACK_BYTES) for path in sorted((self.imported / ".git/objects/pack").iterdir())]
        # This deliberately unreadable/custom record is not an admission input.
        custom = self.imported / "target/old-custom-receipt"
        custom.write_bytes(b"uninterpreted prior orchestration evidence\n")
        custom.chmod(0o000)
        with mock.patch.object(source, "_import_object_database", side_effect=AssertionError("initialization must not import objects")), \
             mock.patch.object(release, "capture_source", side_effect=AssertionError("initialization must not recapture")):
            result = self.initialize(captures, self.commit, self.tree)
        self.assertEqual(store.verify_store(self.imported, Path(result["receipt"]["path"]), result["receipt"]["sha256"],
                                            self.commit, self.tree, self.fingerprint)["capture_count"], 2)
        self.assertEqual([store._reference(Path(row["path"]), source.MAX_PACK_BYTES) for row in packs], packs)
        self.assertEqual(custom.lstat().st_mode & 0o777, 0o000)
        self.unchanged()

    def test_initialize_arbitrary_three_capture_native_chain(self):
        captures = self._current_two_pack_captures()
        (self.repo / "native.rs").write_bytes(b"const VERSION: u8 = 3;\n")
        self.git("add", "native.rs")
        self.git("commit", "-m", "third independently authenticated incumbent")
        commit = self.git("rev-parse", "HEAD").decode().strip()
        tree = self.git("rev-parse", "HEAD^{tree}").decode().strip()
        directory = self.case / "third-capture"
        source.export_source(self.repo, commit, tree, self.fingerprint, directory)
        self._qualified_existing_pack(directory, commit, tree, "third-qualified-incumbent")
        captures.append(self.capture_input(directory, commit, tree))
        result = self.initialize(captures, commit, tree)
        value, _, _ = store._load_receipt(self.imported, Path(result["receipt"]["path"]), result["receipt"]["sha256"])
        self.assertEqual(value["kind"], "initialization")
        self.assertEqual(value["initialization"]["capture_count"], 3)
        self.assertEqual(value["operations"], ["0" * 32])
        self.unchanged()

    def test_initialization_intent_boundary_resumes_without_object_effects(self):
        captures = self._current_two_pack_captures()
        with mock.patch.object(store, "_fault", side_effect=lambda phase: (_ for _ in ()).throw(RuntimeError("interrupted init")) if phase == "initialization_intent_durable" else None):
            with self.assertRaisesRegex(RuntimeError, "interrupted init"):
                self.initialize(captures, self.commit, self.tree)
        intent = self.intent("0" * 32)
        with self.assertRaisesRegex(source.SourceCaptureError, "independently pinned durable intent"):
            self.initialize(captures, self.commit, self.tree)
        with mock.patch.object(source, "_import_object_database", side_effect=AssertionError("resume is records only")):
            result = self.initialize(captures, self.commit, self.tree, intent_sha256=intent["sha256"])
        self.assertEqual(result["intent"], intent)
        self.unchanged()

    def test_initialization_lost_ack_is_read_only(self):
        captures = self._current_two_pack_captures()
        with mock.patch.object(store, "_fault", side_effect=lambda phase: (_ for _ in ()).throw(RuntimeError("lost init ack")) if phase == "initialization_receipt_durable" else None):
            with self.assertRaisesRegex(RuntimeError, "lost init ack"):
                self.initialize(captures, self.commit, self.tree)
        intent = self.intent("0" * 32)
        receipt = store._reference(Path(intent["path"]).parent / "receipt.json")
        with mock.patch.object(store, "_rename_noreplace", side_effect=AssertionError("lost ack replay cannot rename")):
            result = self.initialize(captures, self.commit, self.tree, intent_sha256=intent["sha256"])
        self.assertEqual(result["receipt"], receipt)

    def test_initialization_foreign_terminal_receipt_inode_is_preserved(self):
        result = self.initialize()
        path = Path(result["receipt"]["path"])
        original = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
        try:
            payload = path.read_bytes()
            path.unlink()
            path.write_bytes(payload)
            path.chmod(0o400)
            foreign = store._reference(path)
            with self.assertRaisesRegex(source.SourceCaptureError, "reserved receipt inode differs"):
                self.initialize(intent_sha256=result["intent"]["sha256"])
            self.assertEqual(store._reference(path), foreign)
        finally:
            os.close(original)

    def test_initialization_refuses_withheld_pack_and_wrong_capture_frontier(self):
        captures = self._current_two_pack_captures()
        with self.assertRaisesRegex(source.SourceCaptureError, "selected complete pack trios"):
            self.initialize([captures[0]])
        with self.assertRaisesRegex(source.SourceCaptureError, "selected captured frontier"):
            self.initialize(captures, self.base_commit, self.base_tree)
        self.assertFalse((self.imported / "target/.taira-source-store").exists())

    def test_initialization_refuses_extra_native_object_and_foreign_capture_content(self):
        captures = self._current_two_pack_captures()
        source._git(self.imported, "hash-object", "-w", "--stdin", payload=b"unselected extra source object\n")
        with self.assertRaises(source.SourceCaptureError):
            self.initialize(captures, self.commit, self.tree)
        self.assertFalse((self.imported / "target/.taira-source-store").exists())

    def test_initialization_checks_actual_frozen_capture_bytes(self):
        key = hashlib.sha256(os.fsencode(self.target)).hexdigest()[:24]
        captured = self.target / "taira-release-sources" / key / "source"
        path = captured / "native.rs"
        path.chmod(0o600)
        path.write_bytes(b"foreign captured source\n")
        path.chmod(0o400)
        with self.assertRaisesRegex(release.PrepareError, "tracked source bytes differ"):
            self.initialize()
        self.assertFalse((self.imported / "target/.taira-source-store").exists())

    def test_actual_initialize_store_cli_reads_only_closed_pinned_capture_input(self):
        captures = self._current_two_pack_captures()
        plan = self.case / "current-custody-input.json"
        plan.write_bytes(source.canonical_json_bytes({"schema": store.CUSTODY_INPUT_SCHEMA, "captures": captures}))
        output = self.command([sys.executable, "-B", str(SCRIPTS / "taira_source_capture.py"), "initialize-store",
                               "--source-root", str(self.imported), "--captures-input", str(plan),
                               "--expected-captures-input-sha256", source.stable_hash_path(plan).sha256,
                               "--expected-captured-commit", self.commit, "--expected-captured-tree", self.tree,
                               "--expected-captured-signer", self.fingerprint, "--operation-id", "0" * 32])
        result = source.load_json_object(output, "actual initialization CLI output")
        self.assertEqual(result["commit"], self.commit)
        self.assertFalse(result["activated"])


if __name__ == "__main__":
    unittest.main()
