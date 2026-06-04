import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

from pytests.scripts import iso_operator_evidence_verify_test as evidence_test
from pytests.scripts import iso_xsd_fixture_verify_test as xsd_test


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "iso_production_readiness.py"
SPEC = importlib.util.spec_from_file_location("iso_production_readiness", SCRIPT_PATH)
READINESS = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = READINESS
SPEC.loader.exec_module(READINESS)


def write_json(path, body):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(body, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def refresh_digest(summary):
    summary.pop(READINESS.SUMMARY_DIGEST_FIELD, None)
    summary[READINESS.SUMMARY_DIGEST_FIELD] = READINESS.sha256_hex(
        READINESS._canonical_json_bytes(summary)
    )
    return summary


def receipt_verification_summary(
    receipt_kind=None,
    *,
    verified_receipts=None,
    allow_failed=False,
    allow_insecure_http=False,
    allow_legacy_colr007=False,
    require_source_files=True,
    receipts=None,
):
    kinds = receipt_kind or ["iso-audit-notary", "iso-rail-gateway"]
    if verified_receipts is None:
        verified_receipts = len(kinds)
    if receipts is None:
        receipts = [
            {
                "path": f"/ops/iso/receipts/{kind}.{offset}.receipt.json",
                "receipt_kind": kind,
                "receipt_sha256": f"{offset + 1:064x}",
            }
            for offset, kind in enumerate(kinds[: max(verified_receipts, 0)])
        ]
    return refresh_digest(
        {
            "verified_receipts": verified_receipts,
            "receipt_kind": kinds,
            "allow_failed": allow_failed,
            "allow_insecure_http": allow_insecure_http,
            "allow_legacy_colr007": allow_legacy_colr007,
            "require_source_files": require_source_files,
            "receipts": receipts,
        }
    )


def run_readiness(argv):
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        rc = READINESS.main(argv)
    return rc, stdout.getvalue(), stderr.getvalue()


def write_strict_xsd_summary(root):
    root.mkdir(parents=True, exist_ok=True)
    manifest_path = xsd_test.write_minimal_tree(root, xsd_test.minimal_manifest())
    summary_path = root / "xsd.summary.json"
    rc, _stdout, stderr = xsd_test.run_verify(
        [
            "--manifest",
            str(manifest_path),
            "--require-schema-backed-fixtures",
            "--require-fixture-for-schema",
            "--summary-out",
            str(summary_path),
        ]
    )
    if rc != 0:
        raise AssertionError(stderr)
    return summary_path


def write_checked_in_xsd_summary(root, *, require_fixture_for_schema=False):
    root.mkdir(parents=True, exist_ok=True)
    summary_path = root / "checked-in-xsd.summary.json"
    argv = [
        "--manifest",
        str(REPO_ROOT / "fixtures" / "iso20022" / "xsd" / "fixture_manifest.json"),
        "--summary-out",
        str(summary_path),
    ]
    if require_fixture_for_schema:
        argv.append("--require-fixture-for-schema")
    rc, _stdout, stderr = xsd_test.run_verify(argv)
    if rc != 0:
        raise AssertionError(stderr)
    return summary_path


def write_evidence_summary(root):
    root.mkdir(parents=True, exist_ok=True)
    canary_path = evidence_test.write_canary(root)
    trust_path = evidence_test.write_trust_summary(root / "trust")
    summary_path = root / "evidence.summary.json"
    rc, _stdout, stderr = evidence_test.run_evidence(
        [
            "--canary-summary",
            str(canary_path),
            "--trust-summary",
            str(trust_path),
            "--provider",
            "local-bank",
            "--environment",
            "preprod",
            "--summary-out",
            str(summary_path),
        ]
    )
    if rc != 0:
        raise AssertionError(stderr)
    return summary_path


def add_archive_receipt_verification(path, receipt_kind=None, *, verified_receipts=None):
    evidence = json.loads(path.read_text(encoding="utf-8"))
    evidence["receipt_verification"] = receipt_verification_summary(
        receipt_kind,
        verified_receipts=verified_receipts,
    )
    refresh_digest(evidence)
    return write_json(path, evidence)


class IsoProductionReadinessTest(unittest.TestCase):
    def test_strict_xsd_and_production_evidence_pass(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            summary_out = root / "readiness.summary.json"

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_summary),
                    "--provider",
                    "local-bank",
                    "--environment",
                    "preprod",
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["ok"])
            self.assertEqual(summary["blockers"], [])
            self.assertEqual(summary["warnings"], [])
            self.assertEqual(json.loads(summary_out.read_text(encoding="utf-8")), summary)
            body = dict(summary)
            digest = body.pop("summary_sha256")
            self.assertEqual(digest, READINESS.sha256_hex(READINESS._canonical_json_bytes(body)))

    def test_checked_in_xsd_gaps_block_by_default_and_can_be_diagnostic_warnings(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_checked_in_xsd_summary(
                root / "xsd",
                require_fixture_for_schema=True,
            )
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 1, stderr)
            blocked = json.loads(stdout)
            self.assertFalse(blocked["ok"])
            self.assertEqual(
                {blocker["code"] for blocker in blocked["blockers"]},
                {"xsd.strict_schema_backed_not_proven", "xsd.missing_schema_fixtures"},
            )

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_summary),
                    "--allow-reviewed-xsd-gaps",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            diagnostic = json.loads(stdout)
            self.assertTrue(diagnostic["ok"])
            self.assertEqual(diagnostic["blockers"], [])
            self.assertEqual(
                {warning["code"] for warning in diagnostic["warnings"]},
                {"xsd.strict_schema_backed_not_proven", "xsd.missing_schema_fixtures"},
            )

    def test_tampered_xsd_or_evidence_summary_is_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            tampered_xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
            tampered_xsd["verified_schemas"] = 99
            tampered_xsd_path = write_json(root / "tampered-xsd.summary.json", tampered_xsd)

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(tampered_xsd_path), "--evidence-summary", str(evidence_summary)]
            )
            self.assertEqual(rc, 2)
            self.assertIn("summary_sha256 mismatch", stderr)

            tampered_evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            tampered_evidence["ok"] = False
            tampered_evidence_path = write_json(root / "tampered-evidence.summary.json", tampered_evidence)
            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(tampered_evidence_path)]
            )
            self.assertEqual(rc, 2)
            self.assertIn("summary_sha256 mismatch", stderr)

    def test_missing_xsd_strict_flags_are_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for flag in ("require_schema_backed_fixtures", "require_fixture_for_schema"):
                with self.subTest(flag=flag):
                    xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    del xsd["strict"][flag]
                    refresh_digest(xsd)
                    mutated_path = write_json(root / f"missing-{flag}.xsd-summary.json", xsd)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(mutated_path), "--evidence-summary", str(evidence_summary)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(f"strict.{flag} must be a boolean", stderr)

    def test_evidence_policy_and_provider_environment_drift_block_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["policy"]["allow_insecure_http"] = True
            evidence["policy"]["allow_legacy_colr007"] = True
            refresh_digest(evidence)
            mutated_path = write_json(root / "policy-evidence.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(mutated_path),
                    "--provider",
                    "other-bank",
                    "--environment",
                    "prod",
                ]
            )

            self.assertEqual(rc, 1, stderr)
            summary = json.loads(stdout)
            codes = {blocker["code"] for blocker in summary["blockers"]}
            self.assertIn("evidence.policy.allow_insecure_http", codes)
            self.assertIn("evidence.policy.allow_legacy_colr007", codes)
            self.assertIn("evidence.provider_mismatch", codes)
            self.assertIn("evidence.environment_mismatch", codes)
            self.assertIn("trust.environment_mismatch", codes)

    def test_missing_evidence_policy_flag_is_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            del evidence["policy"]["allow_insecure_http"]
            refresh_digest(evidence)
            mutated_path = write_json(root / "missing-policy-flag.summary.json", evidence)

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("policy.allow_insecure_http must be a boolean", stderr)

    def test_missing_evidence_status_booleans_are_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("summary-ok", [], "ok", "ok must be a boolean"),
                (
                    "canary-plan-only",
                    ["canary_summaries", 0],
                    "plan_only",
                    "plan_only must be a boolean",
                ),
            )
            for name, path_parts, key, message in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    target = evidence
                    for part in path_parts:
                        target = target[part]
                    del target[key]
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"missing-{name}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_missing_nested_receipt_policy_flag_is_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "canary",
                    ["canary_summaries", 0, "receipt_summary"],
                    "receipt_summary.allow_insecure_http must be a boolean",
                ),
                (
                    "archive",
                    ["receipt_verification"],
                    "receipt_verification.allow_insecure_http must be a boolean",
                ),
            )
            for name, path_parts, message in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    receipt_summary = evidence
                    for part in path_parts:
                        receipt_summary = receipt_summary[part]
                    del receipt_summary["allow_insecure_http"]
                    refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"missing-{name}-receipt-policy-flag.summary.json",
                        evidence,
                    )

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_missing_canary_stage_and_receipt_kind_block_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            canary = evidence["canary_summaries"][0]
            canary["stage_names"] = ["rail", "notary"]
            canary["receipt_summary"] = receipt_verification_summary(
                ["iso-rail-gateway"],
                allow_legacy_colr007=True,
            )
            refresh_digest(evidence)
            mutated_path = write_json(root / "weak-canary.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.missing_canary_stages", codes)
            self.assertIn("evidence.missing_receipt_kinds", codes)
            self.assertIn("evidence.receipts_allow_legacy_colr007", codes)

    def test_nonproduction_trust_policy_and_zero_pins_block_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            profile = evidence["trust_summaries"][0]["profiles"][0]
            profile["embedded_signature_policy"] = "record-only"
            profile["signature_public_key_pin_count"] = 0
            profile["x509_trust_anchor_pin_count"] = 0
            refresh_digest(evidence)
            mutated_path = write_json(root / "weak-trust.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.policy_not_require_verified", codes)
            self.assertIn("trust.no_signature_or_x509_pins", codes)

    def test_missing_or_partial_archive_receipt_verification_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = write_evidence_summary(root / "evidence")

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )
            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.archive_receipts_not_reverified", codes)

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_summary),
                    "--allow-canary-stage-receipts-only",
                ]
            )
            self.assertEqual(rc, 0, stderr)
            diagnostic = json.loads(stdout)
            self.assertTrue(diagnostic["ok"])
            self.assertTrue(diagnostic["policy"]["allow_canary_stage_receipts_only"])

            partial = add_archive_receipt_verification(
                write_evidence_summary(root / "partial-evidence"),
                ["iso-rail-gateway"],
            )
            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(partial)]
            )
            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.archive_receipt_kinds_missing", codes)

    def test_tampered_archive_receipt_summary_is_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["receipt_verification"]["receipts"][0]["receipt_sha256"] = "f" * 64
            refresh_digest(evidence)
            tampered_path = write_json(root / "tampered-receipts.summary.json", evidence)

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(tampered_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("receipt_verification summary_sha256 mismatch", stderr)

    def test_weak_archive_receipt_policy_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            archive = evidence["receipt_verification"]
            archive["allow_failed"] = True
            archive["allow_insecure_http"] = True
            archive["allow_legacy_colr007"] = True
            archive["require_source_files"] = False
            refresh_digest(archive)
            refresh_digest(evidence)
            weak_path = write_json(root / "weak-archive-receipts.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(weak_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.archive_receipts_allow_failed", codes)
            self.assertIn("evidence.archive_receipts_insecure_http", codes)
            self.assertIn("evidence.archive_receipts_allow_legacy_colr007", codes)
            self.assertIn("evidence.archive_receipts_source_files_not_required", codes)

    def test_archive_receipt_entries_must_bind_each_receipt_digest(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            archive = evidence["receipt_verification"]
            del archive["receipts"][0]["receipt_sha256"]
            refresh_digest(archive)
            refresh_digest(evidence)
            weak_path = write_json(root / "missing-receipt-digest.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(weak_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.archive_receipt_digest_missing", codes)


if __name__ == "__main__":
    unittest.main()
