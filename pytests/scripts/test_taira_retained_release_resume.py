"""Offline binary archive-resume controls using real tiny streams, no SSH/Cargo."""
from __future__ import annotations

import contextlib
import copy
import os
from pathlib import Path
import stat
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import test_taira_retained_release as fixtures

owner = fixtures.owner


class RetainedReleaseResumeTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.RetainedReleaseTests(methodName="runTest")
        self.fixture.setUp()
        self.addCleanup(self.fixture.tearDown)
        self.root = self.fixture.root
        self.plan = self.fixture.plan
        self.deployment = self.fixture.deployment
        self.admission = self.fixture.admission
        self.rows = self.fixture.rows
        self.output = self.root / "interrupted-archive"
        self.diagnostics = self.root / "resume.stderr"
        self.payloads = [Path(row["path"]).read_bytes() for row in self.rows]
        self.original_stderr = b"retained original pread failure\n"
        self.envelopes = []

    def interrupted(self, *, completed=3, partial=19):
        owner.fresh_directory(self.output)
        owner.write_new(self.output / "plan.json", owner.canonical(self.plan))
        owner.write_new(self.output / "admission.json", owner.canonical(self.admission))
        owner.fresh_directory(self.output / "objects")
        error = self.output / "archive.stderr"
        error.write_bytes(self.original_stderr)
        error.chmod(0o600)
        for index in range(completed):
            path = self.output / "objects" / f"{index:04d}"
            path.write_bytes(self.payloads[index])
            path.chmod(0o400)
        if partial is not None:
            path = self.output / "objects" / f"{completed:04d}"
            path.write_bytes(self.payloads[completed][:partial])
            path.chmod(0o600)

    @contextlib.contextmanager
    def session(self, route, envelope, modules, evidence):
        self.assertEqual(envelope["operation"], "archive-resume")
        self.assertEqual(envelope["plan"], self.plan)
        self.assertEqual(envelope["admission"], self.admission)
        self.assertEqual(evidence, self.diagnostics)
        self.envelopes.append(copy.deepcopy(envelope))
        evidence.write_bytes(b"")
        evidence.chmod(0o600)
        read_fd, write_fd = os.pipe()
        try:
            try:
                with patch.object(owner, "authority_locks", return_value=contextlib.nullcontext()):
                    owner.archive_stream(
                        self.plan, self.deployment, self.admission, write_fd,
                        resume=envelope["resume"],
                    )
            finally:
                os.close(write_fd)
            yield owner.Reader(read_fd, timeout=2)
        finally:
            os.close(read_fd)

    def resume(self):
        with patch.object(owner, "session", side_effect=self.session):
            return owner.archive_resume_local(
                self.plan, self.deployment, self.admission, {}, self.output,
                self.diagnostics,
            )

    def assert_complete(self):
        plan, admission, _ = owner.verify_archive(self.output)
        self.assertEqual(plan, self.plan)
        self.assertEqual(admission, self.admission)
        self.assertEqual((self.output / "archive.stderr").read_bytes(), self.original_stderr)
        for index, payload in enumerate(self.payloads):
            path = self.output / "objects" / f"{index:04d}"
            self.assertEqual(path.read_bytes(), payload)
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o400)
            self.assertEqual(Path(self.rows[index]["path"]).read_bytes(), payload)

    def test_resume_verifies_remote_prefix_and_preserves_complete_object_custody(self):
        self.interrupted()
        originals = {
            path.name: owner.identity(path.stat())
            for path in (self.output / "objects").iterdir()
            if stat.S_IMODE(path.stat().st_mode) == 0o400
        }
        result = self.resume()
        self.assertTrue(result["archive_complete"])
        self.assertFalse(result["retirement_authorized"])
        self.assertEqual(self.envelopes[0]["resume"], {
            "completed": 3,
            "partial": {"index": 3, "size": 19, "sha256": owner.sha(self.payloads[3][:19])},
        })
        for name, identity in originals.items():
            self.assertEqual(owner.identity((self.output / "objects" / name).stat()), identity)
        self.assert_complete()

    def test_zero_byte_interrupted_last_create_can_resume(self):
        self.interrupted(partial=0)
        self.resume()
        self.assert_complete()

    def test_full_last_file_before_seal_can_resume_without_receiving_duplicate_bytes(self):
        self.interrupted(partial=len(self.payloads[3]))
        self.resume()
        self.assertEqual(self.envelopes[0]["resume"]["partial"]["size"], len(self.payloads[3]))
        self.assert_complete()

    def test_resume_creates_only_missing_remaining_objects(self):
        self.interrupted(completed=1, partial=11)
        first = owner.identity((self.output / "objects/0000").stat())
        self.resume()
        self.assertEqual(owner.identity((self.output / "objects/0000").stat()), first)
        self.assert_complete()

    def test_all_complete_objects_without_final_receipt_are_revalidated(self):
        self.interrupted(completed=4, partial=None)
        self.resume()
        self.assertEqual(self.envelopes[0]["resume"], {"completed": 4, "partial": None})
        self.assert_complete()

    def test_interrupted_resumed_tail_keeps_a_durable_prefix_for_next_attempt(self):
        self.interrupted()

        class InterruptedReader:
            def __init__(reader, actual):
                reader.actual = actual
                reader.chunks = 0

            def frame(reader):
                return reader.actual.frame()

            def exact(reader, count):
                reader.chunks += 1
                if reader.chunks > 1:
                    raise ValueError("interrupted resumed tail")
                return reader.actual.exact(count)

        @contextlib.contextmanager
        def interrupted_session(*args):
            with self.session(*args) as reader:
                yield InterruptedReader(reader)

        with patch.object(owner, "CHUNK", 7), patch.object(owner, "session", side_effect=interrupted_session):
            with self.assertRaisesRegex(ValueError, "interrupted resumed tail"):
                owner.archive_resume_local(self.plan, self.deployment, self.admission, {}, self.output, self.diagnostics)
        partial = self.output / "objects/0003"
        self.assertEqual(partial.read_bytes(), self.payloads[3][:26])
        self.assertEqual(stat.S_IMODE(partial.stat().st_mode), 0o600)
        self.assertFalse((self.output / "completed.json").exists())
        self.diagnostics = self.root / "resume-second.stderr"
        self.resume()
        self.assertEqual(self.envelopes[-1]["resume"]["partial"]["size"], 26)
        self.assert_complete()

    def test_missing_middle_object_is_refused_before_dispatch(self):
        self.interrupted()
        (self.output / "objects/0001").unlink()
        with patch.object(owner, "session") as session:
            with self.assertRaises(ValueError):
                owner.archive_resume_local(self.plan, self.deployment, self.admission, {}, self.output, self.diagnostics)
            session.assert_not_called()
        self.assertFalse((self.output / "completed.json").exists())

    def test_changed_prefix_from_authenticated_resume_intent_is_refused_before_dispatch(self):
        self.interrupted()
        _, _, expected_resume = owner.archive_resume_census(self.output)
        partial = self.output / "objects/0003"
        partial.write_bytes(self.payloads[3][:26])
        before = owner.identity(partial.stat())
        with patch.object(owner, "session") as session:
            with self.assertRaisesRegex(ValueError, "archive prefix changed from the authenticated resume intent"):
                owner.archive_resume_local(
                    self.plan, self.deployment, self.admission, {}, self.output,
                    self.diagnostics, expected_resume=expected_resume,
                )
            session.assert_not_called()
        self.assertEqual(owner.identity(partial.stat()), before)
        self.assertEqual(partial.read_bytes(), self.payloads[3][:26])
        self.assertEqual((self.output / "archive.stderr").read_bytes(), self.original_stderr)
        self.assertFalse((self.output / "completed.json").exists())
        self.assertFalse(self.diagnostics.exists())

    def test_complete_object_after_partial_is_refused_before_dispatch(self):
        self.interrupted(completed=1, partial=11)
        later = self.output / "objects/0002"
        later.write_bytes(self.payloads[2])
        later.chmod(0o400)
        with patch.object(owner, "session") as session:
            with self.assertRaises(ValueError):
                owner.archive_resume_local(self.plan, self.deployment, self.admission, {}, self.output, self.diagnostics)
            session.assert_not_called()

    def test_corrupt_complete_backup_is_refused_before_dispatch(self):
        self.interrupted()
        first = self.output / "objects/0000"
        first.chmod(0o600)
        first.write_bytes(b"z" * len(self.payloads[0]))
        first.chmod(0o400)
        with patch.object(owner, "session") as session:
            with self.assertRaises(ValueError):
                owner.archive_resume_local(self.plan, self.deployment, self.admission, {}, self.output, self.diagnostics)
            session.assert_not_called()

    def test_corrupt_partial_backup_is_rejected_by_fresh_remote_prefix_hash(self):
        self.interrupted()
        partial = self.output / "objects/0003"
        partial.write_bytes(b"z" * 19)
        before = owner.identity(partial.stat())
        with self.assertRaises(ValueError):
            self.resume()
        self.assertEqual(len(self.envelopes), 1)
        self.assertEqual(owner.identity(partial.stat()), before)
        self.assertEqual(partial.read_bytes(), b"z" * 19)
        self.assertFalse((self.output / "completed.json").exists())

    def test_changed_current_source_bytes_are_rejected_before_tail_append(self):
        self.interrupted()
        Path(self.rows[3]["path"]).write_bytes(b"z" * len(self.payloads[3]))
        with self.assertRaises(ValueError):
            self.resume()
        self.assertEqual((self.output / "objects/0003").read_bytes(), self.payloads[3][:19])
        self.assertFalse((self.output / "completed.json").exists())

    def test_changed_admission_during_remote_stream_prevents_completion(self):
        self.interrupted()
        changed = copy.deepcopy(self.admission)
        changed["deployment"] = {"changed": True}
        with patch.object(owner, "inspect", side_effect=[self.admission, changed]):
            with self.assertRaises(ValueError):
                self.resume()
        self.assertFalse((self.output / "completed.json").exists())

    def test_changed_original_metadata_is_refused_before_dispatch(self):
        self.interrupted()
        metadata = self.output / "admission.json"
        changed = copy.deepcopy(self.admission)
        changed["source_files_read"] = True
        metadata.chmod(0o600)
        metadata.write_bytes(owner.canonical(changed))
        metadata.chmod(0o400)
        with patch.object(owner, "session") as session:
            with self.assertRaises(ValueError):
                owner.archive_resume_local(self.plan, self.deployment, self.admission, {}, self.output, self.diagnostics)
            session.assert_not_called()


if __name__ == "__main__":
    unittest.main()
