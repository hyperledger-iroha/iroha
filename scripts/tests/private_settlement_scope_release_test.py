"""Replay complete synthetic scopes through canonical release accounting guards.

No network processes, executable copies, or release qualification are produced.
The installed native account codec is required; its absence is a test failure.
"""

from __future__ import annotations

import copy
from contextlib import contextmanager
import json
from pathlib import Path, PurePosixPath
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from scripts.tests.private_settlement_registered_accounting_fixture import (
    RUNNER, build_registered_accounting_fixture, canonical_configuration_inputs, raw,
)
from scripts.tests.private_settlement_release_evidence_test import (
    RELEASE_COMMIT, fixture_hardware_description,
)


class RegisteredScopeReleaseTests(unittest.TestCase):
    """Authenticate archive completeness and independently replay public counts."""

    @classmethod
    def setUpClass(cls):
        cls.area = tempfile.TemporaryDirectory(prefix="synthetic-scope-release-")
        cls.root = Path(cls.area.name).resolve()
        cls.hardware = raw(fixture_hardware_description())
        paths, payloads, manifest_path, manifest_payload = canonical_configuration_inputs(RELEASE_COMMIT)
        cls.configurations = {n: RUNNER.attempt_accounting.accounting_file_binding(payload)["sha256"]
                              for n, payload in payloads.items()}
        cls.artifacts, cls.rows, cls.collected = build_registered_accounting_fixture(
            cls.root, commit=RELEASE_COMMIT, hardware_path=Path("evidence/hardware.json"),
            hardware_payload=cls.hardware, configuration_manifest_path=manifest_path,
            configuration_manifest_payload=manifest_payload,
            configuration_payloads={paths[n]: payload for n, payload in payloads.items()},
            validator_sha256="e" * 64)
        cls.scope = cls.root / "accounting/scope.json"

    @classmethod
    def tearDownClass(cls):
        cls.area.cleanup()

    @contextmanager
    def changed_records(self):
        """Restore exact synthetic bytes after a deliberate authenticated mutation."""
        originals = {}
        def replace(path, value):
            originals.setdefault(path, path.read_bytes())
            payload = raw(value)
            path.write_bytes(payload)
            return RUNNER.attempt_accounting.accounting_file_binding(payload)
        try:
            yield replace
        finally:
            for path, payload in originals.items():
                path.write_bytes(payload)

    def verify(self, root=None, artifacts=None):
        """Invoke the canonical release boundary without mocking any validator."""
        return RUNNER.release_evidence._validate_registered_benchmark_accounting(
            root=root or self.root,
            artifacts=[RUNNER.release_evidence.Artifact(kind=row["kind"], path=PurePosixPath(row["path"]),
                sha256=row["sha256"], bytes=row["bytes"]) for row in (artifacts or self.artifacts)],
            commit=RELEASE_COMMIT,
            hardware_sha256=RUNNER.attempt_accounting.accounting_file_binding(self.hardware)["sha256"],
            hardware_profile_sha256=RUNNER.release_evidence._hardware_profile_sha256(json.loads(self.hardware)),
            configuration_sha256_by_participants=self.configurations)

    def test_canonical_replay_preserves_accepted_warmup_before_failed_predecessor(self):
        accounting, collected = self.verify()
        self.assertEqual(collected, self.collected)
        self.assertEqual(accounting["counts"], dict(planned=700, attempted=352,
            succeeded=351, failed=1, timed_out=0, not_started=348, incomplete=0))
        self.assertEqual(len(self.rows), 351)

    def test_controlled_archive_retains_original_records_and_private_transport_modes(self):
        with tempfile.TemporaryDirectory(dir=self.root) as temporary:
            publication = Path(temporary).resolve()
            artifacts, accounting, rows = RUNNER.archive_benchmark_scope(self.scope, publication)
            actual, collected = self.verify(root=publication, artifacts=artifacts)
            self.assertEqual(actual, accounting)
            self.assertEqual(collected, self.collected)
            self.assertEqual(rows, self.rows)
            for path in (publication / "accounting").rglob("benchmark-protocol"):
                self.assertEqual(path.stat().st_mode & 0o777, 0o700)
                for record in path.iterdir():
                    self.assertEqual(record.stat().st_mode & 0o777, 0o600)

    def test_rebound_failed_request_cannot_change_embedded_configuration(self):
        campaign = self.root / "accounting/campaigns/campaign-0-failed"
        attempt = sorted((campaign / "attempts").iterdir())[-1]
        with self.changed_records() as replace:
            request = json.loads((attempt / "request.json").read_bytes())
            request["configuration"]["consensus"]["mandatory_signed_rs16_da_rbc"] = False
            binding = replace(attempt / "request.json", request)
            started = json.loads((attempt / "started.json").read_bytes())
            started["request"] = binding
            replace(attempt / "started.json", started)
            terminal_path = attempt / "evidence/benchmark-protocol/rust-result.json"
            terminal = json.loads(terminal_path.read_bytes())
            terminal["request_sha256"] = binding["sha256"]
            terminal_binding = replace(terminal_path, terminal)
            adapter_path = terminal_path.with_name("adapter-outcome.json")
            adapter = json.loads(adapter_path.read_bytes())
            adapter.update(request_sha256=binding["sha256"], rust_terminal=terminal_binding)
            replace(adapter_path, adapter)
            # All byte joins still agree; canonical request reconstruction must
            # additionally reject the false configuration claim.
            reduced = RUNNER.attempt_accounting.reduce_registered_scope(*RUNNER.collect_benchmark_scope(self.scope))
            self.assertEqual(reduced["counts"]["failed"], 1)
            with self.assertRaisesRegex(RUNNER.RunnerError, "canonical frozen-plan replay"):
                RUNNER.qualify_benchmark_scope(self.scope)

    def test_rehashed_public_accounting_cannot_hide_failed_predecessor(self):
        artifacts = copy.deepcopy(self.artifacts)
        entry = next(row for row in artifacts if row["kind"] == "benchmark_accounting_report")
        path = self.root / entry["path"]
        with self.changed_records() as replace:
            report = json.loads(path.read_bytes())
            report["counts"]["failed"] = 0
            entry.update(replace(path, report))
            with self.assertRaisesRegex(RUNNER.release_evidence.EvidenceError, "public benchmark counts"):
                self.verify(artifacts=artifacts)

    def test_controlled_record_cannot_be_omitted_or_relabelled(self):
        index = next(i for i, row in enumerate(self.artifacts) if row["path"].endswith("benchmark-sample.json"))
        for mutation in ("omit", "relabel"):
            with self.subTest(mutation=mutation):
                artifacts = copy.deepcopy(self.artifacts)
                if mutation == "omit":
                    artifacts.pop(index)
                else:
                    artifacts[index]["kind"] = "operator_log"
                with self.assertRaisesRegex(RUNNER.release_evidence.EvidenceError, "inventory"):
                    self.verify(artifacts=artifacts)

    def test_completed_campaign_cannot_conceal_unsuccessful_nonbenchmark_process(self):
        campaign = self.root / "accounting/campaigns/campaign-1-complete"
        path = sorted((campaign / "attempts").iterdir())[0] / "process-outcome.json"
        mutations = (
            {"exit_code": 2, "passed": True},
            {"exit_code": 2, "passed": False},
            {"elapsed_ms": -1},
            {"timed_out": True, "completion_kind": "outer_deadline", "passed": False,
             "exit_code": -15, "elapsed_ms": RUNNER.DEFAULT_HARNESS_TIMEOUT_SECONDS * 1000},
        )
        for index, mutation in enumerate(mutations):
            with self.subTest(index=index), self.changed_records() as replace:
                process = json.loads(path.read_bytes())
                process.update(mutation)
                replace(path, process)
                with self.assertRaises(RUNNER.RunnerError):
                    RUNNER.qualify_benchmark_scope(self.scope)

    def test_finalization_requires_registered_completed_qualification_campaign(self):
        source = self.root / "source"
        source.mkdir()
        for campaign in ("unregistered", "campaign-0-failed"):
            with self.subTest(campaign=campaign), self.assertRaises(RUNNER.RunnerError):
                RUNNER.finalize_registered_scope(self.scope, self.root / "unpublished",
                    qualification_campaign_id=campaign, source_root=source)
            self.assertFalse((self.root / "unpublished").exists())

    def test_fail_fast_predecessor_cannot_dispatch_after_failed_fault_process(self):
        campaign = self.root / "accounting/campaigns/campaign-0-failed"
        path = sorted((campaign / "attempts").iterdir())[0] / "process-outcome.json"
        with self.changed_records() as replace:
            process = json.loads(path.read_bytes())
            process.update(passed=False, exit_code=2, error="synthetic earlier fault failure")
            replace(path, process)
            with self.assertRaisesRegex(RUNNER.RunnerError, "fail-fast"):
                RUNNER.qualify_benchmark_scope(self.scope)


if __name__ == "__main__":
    unittest.main()
