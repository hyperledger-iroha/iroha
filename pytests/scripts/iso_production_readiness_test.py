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


FRESHNESS_FLAGS = {
    "--max-xsd-age-days": "36500",
    "--max-evidence-age-days": "36500",
    "--max-canary-age-days": "36500",
    "--max-trust-age-days": "36500",
    "--max-trust-source-age-days": "36500",
}


def _has_flag(argv, flag):
    return any(item == flag or item.startswith(flag + "=") for item in argv)


def run_readiness(argv, *, include_context=True, include_freshness=True):
    argv = list(argv)
    if include_context and "--provider" not in argv:
        argv.extend(["--provider", "local-bank"])
    if include_context and "--environment" not in argv:
        argv.extend(["--environment", "preprod"])
    if include_freshness:
        for flag, value in FRESHNESS_FLAGS.items():
            if not _has_flag(argv, flag):
                argv.extend([flag, value])
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        rc = READINESS.main(argv)
    return rc, stdout.getvalue(), stderr.getvalue()


def write_strict_xsd_summary(root):
    root.mkdir(parents=True, exist_ok=True)
    manifest_path = xsd_test.write_minimal_tree(root, xsd_test.minimal_manifest())
    profile_catalog = xsd_test.write_profile_catalog(root / "profiles.rs")
    summary_path = root / "xsd.summary.json"
    rc, _stdout, stderr = xsd_test.run_verify(
        [
            "--manifest",
            str(manifest_path),
            "--require-schema-backed-fixtures",
            "--require-fixture-for-schema",
            "--profile-catalog",
            str(profile_catalog),
            "--require-profile-schema-backed-versions",
            "--validate-xml-schema",
            "--summary-out",
            str(summary_path),
        ]
    )
    if rc != 0:
        raise AssertionError(stderr)
    return summary_path


def write_checked_in_xsd_summary(
    root,
    *,
    require_fixture_for_schema=False,
    require_profile_schema_backed_versions=False,
    profile_catalog=True,
    validate_xml_schema=True,
):
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
    if profile_catalog:
        argv.extend(
            [
                "--profile-catalog",
                str(
                    REPO_ROOT
                    / "crates"
                    / "iroha_core"
                    / "src"
                    / "iso_bridge"
                    / "profiles.rs"
                ),
            ]
        )
    if require_profile_schema_backed_versions:
        argv.append("--require-profile-schema-backed-versions")
    if validate_xml_schema:
        argv.append("--validate-xml-schema")
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
            self.assertIn("verified_at", summary["xsd_summaries"][0])
            self.assertEqual(
                summary["xsd_summaries"][0]["schema_sources"][0]["license"],
                "Apache-2.0",
            )
            self.assertIn("verified_at", summary["evidence_summaries"][0])
            self.assertEqual(
                summary["evidence_summaries"][0]["policy"],
                {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "max_canary_age_days": 36500,
                    "max_trust_age_days": 36500,
                    "max_trust_source_age_days": 36500,
                },
            )
            self.assertEqual(summary["policy"]["max_xsd_age_days"], 36500)
            self.assertEqual(summary["policy"]["max_evidence_age_days"], 36500)
            self.assertEqual(summary["policy"]["max_canary_age_days"], 36500)
            self.assertEqual(summary["policy"]["max_trust_age_days"], 36500)
            self.assertEqual(summary["policy"]["max_trust_source_age_days"], 36500)
            self.assertTrue(summary["xsd_summaries"][0]["strict"]["validate_xml_schema"])
            self.assertTrue(
                summary["xsd_summaries"][0]["strict"][
                    "require_profile_schema_backed_versions"
                ]
            )
            self.assertRegex(summary["xsd_summaries"][0]["manifest_sha256"], r"^[0-9a-f]{64}$")
            self.assertRegex(
                summary["xsd_summaries"][0]["profile_catalog"]["sha256"],
                r"^[0-9a-f]{64}$",
            )
            self.assertRegex(
                summary["xsd_summaries"][0]["profile_catalog"]["catalog_json_sha256"],
                r"^[0-9a-f]{64}$",
            )
            self.assertEqual(summary["xsd_summaries"][0]["schema_validated_fixtures"], 1)
            self.assertEqual(summary["xsd_summaries"][0]["profile_checked_versions"], 1)
            self.assertEqual(summary["xsd_summaries"][0]["profile_schema_backed_versions"], 1)
            self.assertTrue(
                summary["evidence_summaries"][0]["canary_summaries"][0][
                    "require_explicit_policy"
                ]
            )
            canary_summary = summary["evidence_summaries"][0]["canary_summaries"][0]
            self.assertTrue(canary_summary["path"].endswith("canary.summary.json"))
            self.assertRegex(canary_summary["summary_sha256"], r"^[0-9a-f]{64}$")
            self.assertEqual(canary_summary["started_at"], "2026-06-04T00:00:00+00:00")
            self.assertEqual(canary_summary["finished_at"], "2026-06-04T00:00:01+00:00")
            self.assertEqual(
                [stage["name"] for stage in canary_summary["stage_windows"]],
                ["rail", "notary", "verify"],
            )
            self.assertEqual(canary_summary["stage_names"], ["rail", "notary", "verify"])
            trust_profile = summary["evidence_summaries"][0]["trust_summaries"][0]["profiles"][0]
            trust_summary = summary["evidence_summaries"][0]["trust_summaries"][0]
            self.assertTrue(trust_summary["path"].endswith("trust.summary.json"))
            self.assertRegex(trust_summary["verified_at"], r"^\d{4}-\d{2}-\d{2}T")
            self.assertRegex(trust_summary["summary_sha256"], r"^[0-9a-f]{64}$")
            self.assertTrue(trust_profile["x509_require_crl_revocation_check"])
            self.assertEqual(trust_profile["x509_crl_count"], 1)
            self.assertTrue(trust_profile["x509_require_ocsp_revocation_check"])
            self.assertEqual(trust_profile["x509_ocsp_response_count"], 1)
            self.assertEqual(json.loads(summary_out.read_text(encoding="utf-8")), summary)
            body = dict(summary)
            digest = body.pop("summary_sha256")
            self.assertEqual(digest, READINESS.sha256_hex(READINESS._canonical_json_bytes(body)))

    def test_xsd_and_evidence_summaries_are_required(self):
        rc, _stdout, stderr = run_readiness([])

        self.assertEqual(rc, 2)
        self.assertIn("provide at least one --xsd-summary", stderr)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")

            rc, _stdout, stderr = run_readiness(["--xsd-summary", str(xsd_summary)])

            self.assertEqual(rc, 2)
            self.assertIn("provide at least one --evidence-summary", stderr)

    def test_provider_and_environment_are_required_release_context(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            base_argv = [
                "--xsd-summary",
                str(xsd_summary),
                "--evidence-summary",
                str(evidence_summary),
            ]

            rc, _stdout, stderr = run_readiness(base_argv, include_context=False)
            self.assertEqual(rc, 2)
            self.assertIn("provide --provider", stderr)

            rc, _stdout, stderr = run_readiness(
                base_argv + ["--provider", "local-bank"],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("provide --environment", stderr)

            rc, _stdout, stderr = run_readiness(
                base_argv + ["--provider", " ", "--environment", "preprod"],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("provide --provider", stderr)

    def test_freshness_budgets_are_required_and_positive(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            base_argv = [
                "--xsd-summary",
                str(xsd_summary),
                "--evidence-summary",
                str(evidence_summary),
                "--provider",
                "local-bank",
                "--environment",
                "preprod",
            ]

            rc, _stdout, stderr = run_readiness(
                base_argv,
                include_context=False,
                include_freshness=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("provide --max-xsd-age-days", stderr)

            for flag in FRESHNESS_FLAGS:
                with self.subTest(flag=flag):
                    argv = list(base_argv)
                    for other_flag, value in FRESHNESS_FLAGS.items():
                        argv.extend([other_flag, "0" if other_flag == flag else value])

                    rc, _stdout, stderr = run_readiness(
                        argv,
                        include_context=False,
                        include_freshness=False,
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(f"{flag} must be a positive integer", stderr)

    def test_duplicate_input_and_compact_summaries_are_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            copied_xsd = root / "copied-xsd.summary.json"
            copied_xsd.write_text(xsd_summary.read_text(encoding="utf-8"), encoding="utf-8")
            copied_evidence = root / "copied-evidence.summary.json"
            copied_evidence.write_text(
                evidence_summary.read_text(encoding="utf-8"),
                encoding="utf-8",
            )
            duplicate_canary_path = json.loads(evidence_summary.read_text(encoding="utf-8"))
            duplicate_canary_path["canary_summaries"].append(
                dict(duplicate_canary_path["canary_summaries"][0])
            )
            refresh_digest(duplicate_canary_path)
            duplicate_canary_path_file = write_json(
                root / "duplicate-canary-path.evidence.summary.json",
                duplicate_canary_path,
            )
            duplicate_canary_digest = json.loads(evidence_summary.read_text(encoding="utf-8"))
            copied_canary = dict(duplicate_canary_digest["canary_summaries"][0])
            copied_canary["path"] = "/ops/iso/copied-canary.summary.json"
            duplicate_canary_digest["canary_summaries"].append(copied_canary)
            refresh_digest(duplicate_canary_digest)
            duplicate_canary_digest_file = write_json(
                root / "duplicate-canary-digest.evidence.summary.json",
                duplicate_canary_digest,
            )
            duplicate_trust_digest = json.loads(evidence_summary.read_text(encoding="utf-8"))
            copied_trust = dict(duplicate_trust_digest["trust_summaries"][0])
            copied_trust["path"] = "/ops/iso/copied-trust.summary.json"
            duplicate_trust_digest["trust_summaries"].append(copied_trust)
            refresh_digest(duplicate_trust_digest)
            duplicate_trust_digest_file = write_json(
                root / "duplicate-trust-digest.evidence.summary.json",
                duplicate_trust_digest,
            )
            cases = (
                (
                    [
                        "--xsd-summary",
                        str(xsd_summary),
                        "--xsd-summary",
                        str(xsd_summary),
                        "--evidence-summary",
                        str(evidence_summary),
                    ],
                    "--xsd-summary[1] duplicates --xsd-summary[0]",
                ),
                (
                    [
                        "--xsd-summary",
                        str(xsd_summary),
                        "--evidence-summary",
                        str(evidence_summary),
                        "--evidence-summary",
                        str(evidence_summary),
                    ],
                    "--evidence-summary[1] duplicates --evidence-summary[0]",
                ),
                (
                    [
                        "--xsd-summary",
                        str(xsd_summary),
                        "--xsd-summary",
                        str(copied_xsd),
                        "--evidence-summary",
                        str(evidence_summary),
                    ],
                    "xsd_summaries[1].summary_sha256 duplicates xsd_summaries[0].summary_sha256",
                ),
                (
                    [
                        "--xsd-summary",
                        str(xsd_summary),
                        "--evidence-summary",
                        str(evidence_summary),
                        "--evidence-summary",
                        str(copied_evidence),
                    ],
                    "evidence_summaries[1].summary_sha256 duplicates evidence_summaries[0].summary_sha256",
                ),
                (
                    [
                        "--xsd-summary",
                        str(xsd_summary),
                        "--evidence-summary",
                        str(duplicate_canary_path_file),
                    ],
                    "canary_summaries[1].path duplicates",
                ),
                (
                    [
                        "--xsd-summary",
                        str(xsd_summary),
                        "--evidence-summary",
                        str(duplicate_canary_digest_file),
                    ],
                    "canary_summaries[1].summary_sha256 duplicates",
                ),
                (
                    [
                        "--xsd-summary",
                        str(xsd_summary),
                        "--evidence-summary",
                        str(duplicate_trust_digest_file),
                    ],
                    "trust_summaries[1].summary_sha256 duplicates",
                ),
            )
            for argv, message in cases:
                with self.subTest(message=message):
                    rc, _stdout, stderr = run_readiness(argv)

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_duplicate_readiness_input_json_keys_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = root / "duplicate-xsd.summary.json"
            xsd_summary.write_text(
                '{"verified_schemas":1,"verified_schemas":2}\n',
                encoding="utf-8",
            )
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)

    def test_xsd_summary_without_xml_schema_validation_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = xsd_test.write_minimal_tree(root / "xsd", xsd_test.minimal_manifest())
            profile_catalog = xsd_test.write_profile_catalog(root / "xsd" / "profiles.rs")
            xsd_summary = root / "xsd" / "xsd-no-validation.summary.json"
            rc, _stdout, stderr = xsd_test.run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--require-schema-backed-fixtures",
                    "--require-fixture-for-schema",
                    "--profile-catalog",
                    str(profile_catalog),
                    "--require-profile-schema-backed-versions",
                    "--summary-out",
                    str(xsd_summary),
                ]
            )
            self.assertEqual(rc, 0, stderr)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("xsd.xml_schema_validation_not_proven", codes)

    def test_xsd_summary_without_profile_schema_proof_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = xsd_test.write_minimal_tree(root / "xsd", xsd_test.minimal_manifest())
            xsd_summary = root / "xsd" / "xsd-no-profile.summary.json"
            rc, _stdout, stderr = xsd_test.run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--require-schema-backed-fixtures",
                    "--require-fixture-for-schema",
                    "--validate-xml-schema",
                    "--summary-out",
                    str(xsd_summary),
                ]
            )
            self.assertEqual(rc, 0, stderr)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("xsd.profile_schema_backed_not_proven", codes)

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
                {
                    "xsd.strict_schema_backed_not_proven",
                    "xsd.profile_schema_backed_not_proven",
                    "xsd.missing_schema_fixtures",
                    "xsd.missing_profile_schema_versions",
                },
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
                {
                    "xsd.strict_schema_backed_not_proven",
                    "xsd.profile_schema_backed_not_proven",
                    "xsd.missing_schema_fixtures",
                    "xsd.missing_profile_schema_versions",
                },
            )

    def test_forged_xsd_summary_fixture_metadata_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []

            duplicate_fixture = json.loads(xsd_summary.read_text(encoding="utf-8"))
            copied_fixture = dict(duplicate_fixture["fixtures"][0])
            copied_fixture["path"] = "../copied_fixture.xml"
            duplicate_fixture["fixtures"].append(copied_fixture)
            duplicate_fixture["verified_fixtures"] += 1
            duplicate_fixture["schema_backed_fixtures"] += 1
            cases.append((duplicate_fixture, "xsd.fixture_digest_duplicate"))

            fixture_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            fixture_count["verified_fixtures"] += 1
            fixture_count["schema_backed_fixtures"] += 1
            cases.append((fixture_count, "xsd.fixture_count_mismatch"))

            backed_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            backed_count["fixtures"][0]["schema_backed"] = False
            backed_count["fixtures"][0]["schema"] = None
            backed_count["fixtures"][0]["missing_schema_reason"] = "forged gap"
            cases.append((backed_count, "xsd.schema_backed_count_mismatch"))

            for offset, (body, code) in enumerate(cases):
                with self.subTest(code=code):
                    refresh_digest(body)
                    mutated_path = write_json(root / f"forged-xsd-{offset}.summary.json", body)

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(mutated_path), "--evidence-summary", str(evidence_summary)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_forged_xsd_schema_source_metadata_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            malformed_cases = []
            missing_source = json.loads(xsd_summary.read_text(encoding="utf-8"))
            missing_source["schemas"][0].pop("source")
            malformed_cases.append((missing_source, "source must be a JSON object"))

            unknown_source_key = json.loads(xsd_summary.read_text(encoding="utf-8"))
            unknown_source_key["schemas"][0]["source"]["unexpected"] = "value"
            malformed_cases.append((unknown_source_key, "unknown keys"))

            escaped_source_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            escaped_source_path["schemas"][0]["source"]["path"] = "../fooo.001.001.01.xsd"
            malformed_cases.append((escaped_source_path, "must not contain empty, dot, or parent segments"))

            for offset, (body, message) in enumerate(malformed_cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-xsd-source-{offset}.summary.json",
                        body,
                    )

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(mutated_path), "--evidence-summary", str(evidence_summary)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

            blocker_cases = []
            bad_repository = json.loads(xsd_summary.read_text(encoding="utf-8"))
            bad_repository["schemas"][0]["source"]["repository"] += ".git"
            blocker_cases.append((bad_repository, "xsd.schema_source_repository_invalid"))

            bad_commit = json.loads(xsd_summary.read_text(encoding="utf-8"))
            bad_commit["schemas"][0]["source"]["commit"] = (
                "0123456789abcdef0123456789abcdef0123456Z"
            )
            blocker_cases.append((bad_commit, "xsd.schema_source_commit_invalid"))

            bad_filename = json.loads(xsd_summary.read_text(encoding="utf-8"))
            bad_filename["schemas"][0]["source"]["path"] = "xsd/other.001.001.01.xsd"
            blocker_cases.append((bad_filename, "xsd.schema_source_path_mismatch"))

            bad_license = json.loads(xsd_summary.read_text(encoding="utf-8"))
            bad_license["schemas"][0]["source"]["license"] = "NOASSERTION"
            blocker_cases.append((bad_license, "xsd.schema_source_license_invalid"))

            bad_digest = json.loads(xsd_summary.read_text(encoding="utf-8"))
            bad_digest["schemas"][0]["source"]["sha256"] = "0" * 64
            blocker_cases.append((bad_digest, "xsd.schema_source_digest_mismatch"))

            for offset, (body, code) in enumerate(blocker_cases):
                with self.subTest(code=code):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"forged-xsd-source-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(mutated_path), "--evidence-summary", str(evidence_summary)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_forged_xsd_profile_catalog_metadata_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []

            version_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            version_count["profile_checked_versions"] += 1
            cases.append((version_count, "xsd.profile_version_count_mismatch"))

            duplicate_version = json.loads(xsd_summary.read_text(encoding="utf-8"))
            duplicate_version["profile_catalog"]["versions"].append(
                dict(duplicate_version["profile_catalog"]["versions"][0])
            )
            duplicate_version["profile_checked_versions"] += 1
            duplicate_version["profile_schema_backed_versions"] += 1
            cases.append((duplicate_version, "xsd.profile_version_duplicate"))

            missing_mismatch = json.loads(xsd_summary.read_text(encoding="utf-8"))
            missing_mismatch["missing_profile_schema_versions"].append(
                {
                    "profile_id": "minimal-profile",
                    "message_type": "fooo.001",
                    "direction": "inbound",
                    "message_def_id": "fooo.001.001.02",
                }
            )
            cases.append((missing_mismatch, "xsd.missing_profile_schema_versions_mismatch"))
            profile_catalog_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            profile_catalog_count["profile_catalog"]["checked_versions"] += 1
            cases.append((profile_catalog_count, "xsd.profile_catalog_checked_count_mismatch"))

            profile_catalog_backed_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            profile_catalog_backed_count["profile_catalog"]["schema_backed_versions"] += 1
            cases.append(
                (
                    profile_catalog_backed_count,
                    "xsd.profile_catalog_schema_backed_count_mismatch",
                )
            )

            concrete_skipped = json.loads(xsd_summary.read_text(encoding="utf-8"))
            concrete_skipped["profile_catalog"]["skipped_family_versions"].append(
                {
                    "profile_id": "minimal-profile",
                    "message_type": "fooo.001",
                    "direction": "inbound",
                    "version": "fooo.001.001.01",
                }
            )
            cases.append((concrete_skipped, "xsd.profile_catalog_skipped_concrete_version"))

            duplicate_skipped = json.loads(xsd_summary.read_text(encoding="utf-8"))
            duplicate_skipped["profile_catalog"]["skipped_family_versions"].extend(
                [
                    {
                        "profile_id": "minimal-profile",
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "version": "fooo.001",
                    },
                    {
                        "profile_id": "minimal-profile",
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "version": "fooo.001",
                    },
                ]
            )
            cases.append((duplicate_skipped, "xsd.profile_catalog_skipped_duplicate"))

            for offset, (body, code) in enumerate(cases):
                with self.subTest(code=code):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"forged-xsd-profile-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_xsd_provenance_digests_are_required(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (["manifest_sha256"], "manifest_sha256 must be a lowercase SHA-256 digest"),
                (
                    ["profile_catalog", "sha256"],
                    "profile_catalog.sha256 must be a lowercase SHA-256 digest",
                ),
                (
                    ["profile_catalog", "catalog_json_sha256"],
                    (
                        "profile_catalog.catalog_json_sha256 must be a lowercase "
                        "SHA-256 digest"
                    ),
                ),
            )
            for path_parts, message in cases:
                with self.subTest(path_parts=path_parts):
                    xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    target = xsd
                    for part in path_parts[:-1]:
                        target = target[part]
                    del target[path_parts[-1]]
                    refresh_digest(xsd)
                    mutated_path = write_json(
                        root / f"missing-xsd-provenance-{'-'.join(path_parts)}.json",
                        xsd,
                    )

                    rc, _stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

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

    def test_input_summary_verified_at_is_required_timezone_aware_and_not_future(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("missing", None, "verified_at must be a non-empty string"),
                ("naive", "2026-06-04T00:00:00", "verified_at must include a timezone"),
                ("future", "2999-01-01T00:00:00+00:00", "verified_at must not be in the future"),
            )
            for target_name, source_path in (("xsd", xsd_summary), ("evidence", evidence_summary)):
                for name, value, message in cases:
                    with self.subTest(target=target_name, name=name):
                        body = json.loads(source_path.read_text(encoding="utf-8"))
                        if value is None:
                            del body["verified_at"]
                        else:
                            body["verified_at"] = value
                        refresh_digest(body)
                        mutated_path = write_json(
                            root / f"{target_name}-{name}-verified-at.summary.json",
                            body,
                        )
                        argv = ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
                        if target_name == "xsd":
                            argv[1] = str(mutated_path)
                        else:
                            argv[3] = str(mutated_path)

                        rc, _stdout, stderr = run_readiness(argv)

                        self.assertEqual(rc, 2)
                        self.assertIn(message, stderr)

    def test_stale_digest_correct_summaries_block_readiness(self):
        old_start = "2000-01-01T00:00:00+00:00"
        old_rail_done = "2000-01-01T00:00:01+00:00"
        old_notary_done = "2000-01-01T00:00:02+00:00"
        old_finish = "2000-01-01T00:00:03+00:00"

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            stale_xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
            stale_xsd["verified_at"] = old_start
            refresh_digest(stale_xsd)
            stale_xsd_path = write_json(root / "stale-xsd.summary.json", stale_xsd)

            stale_evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            stale_evidence["verified_at"] = old_start
            refresh_digest(stale_evidence)
            stale_evidence_path = write_json(
                root / "stale-evidence.summary.json",
                stale_evidence,
            )

            stale_canary = json.loads(evidence_summary.read_text(encoding="utf-8"))
            canary = stale_canary["canary_summaries"][0]
            canary["started_at"] = old_start
            canary["finished_at"] = old_finish
            canary["stage_windows"] = [
                {
                    "name": "rail",
                    "started_at": old_start,
                    "finished_at": old_rail_done,
                },
                {
                    "name": "notary",
                    "started_at": old_rail_done,
                    "finished_at": old_notary_done,
                },
                {
                    "name": "verify",
                    "started_at": old_notary_done,
                    "finished_at": old_finish,
                },
            ]
            refresh_digest(stale_canary)
            stale_canary_path = write_json(
                root / "stale-canary.summary.json",
                stale_canary,
            )

            stale_trust = json.loads(evidence_summary.read_text(encoding="utf-8"))
            stale_trust["trust_summaries"][0]["verified_at"] = old_start
            refresh_digest(stale_trust)
            stale_trust_path = write_json(root / "stale-trust.summary.json", stale_trust)

            cases = (
                (
                    ["--xsd-summary", str(stale_xsd_path), "--evidence-summary", str(evidence_summary), "--max-xsd-age-days", "1"],
                    "xsd.summary_stale",
                ),
                (
                    ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(stale_evidence_path), "--max-evidence-age-days", "1"],
                    "evidence.summary_stale",
                ),
                (
                    ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(stale_canary_path), "--max-canary-age-days", "1"],
                    "evidence.canary_stale",
                ),
                (
                    ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(stale_trust_path), "--max-trust-age-days", "1"],
                    "trust.summary_stale",
                ),
            )
            for argv, code in cases:
                with self.subTest(code=code):
                    rc, stdout, stderr = run_readiness(argv)

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_missing_xsd_strict_flags_are_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for flag in (
                "require_schema_backed_fixtures",
                "require_fixture_for_schema",
                "require_profile_schema_backed_versions",
                "validate_xml_schema",
            ):
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
            self.assertIn("evidence.policy_provider_mismatch", codes)
            self.assertIn("evidence.policy_environment_mismatch", codes)
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
            cases = (
                ("provider", "policy.provider must be a non-empty string"),
                ("environment", "policy.environment must be a non-empty string"),
                ("allow_insecure_http", "policy.allow_insecure_http must be a boolean"),
                ("max_canary_age_days", "policy.max_canary_age_days must be a positive integer"),
                ("max_trust_age_days", "policy.max_trust_age_days must be a positive integer"),
                ("max_trust_source_age_days", "policy.max_trust_source_age_days must be a positive integer"),
            )
            for key, message in cases:
                with self.subTest(key=key):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    del evidence["policy"][key]
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"missing-policy-{key}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_weaker_evidence_freshness_policy_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("max_canary_age_days", "evidence.policy.max_canary_age_days_weaker_than_release"),
                ("max_trust_age_days", "evidence.policy.max_trust_age_days_weaker_than_release"),
                (
                    "max_trust_source_age_days",
                    "evidence.policy.max_trust_source_age_days_weaker_than_release",
                ),
            )
            for field, code in cases:
                with self.subTest(field=field):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    evidence["policy"][field] = 30
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"weaker-{field}.summary.json", evidence)

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(mutated_path),
                            "--max-canary-age-days",
                            "7",
                            "--max-trust-age-days",
                            "7",
                            "--max-trust-source-age-days",
                            "7",
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

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
                (
                    "canary-explicit-policy",
                    ["canary_summaries", 0],
                    "require_explicit_policy",
                    "require_explicit_policy must be a boolean",
                ),
                (
                    "canary-path",
                    ["canary_summaries", 0],
                    "path",
                    "path must be a non-empty string",
                ),
                (
                    "canary-summary-digest",
                    ["canary_summaries", 0],
                    "summary_sha256",
                    "summary_sha256 must be a lowercase SHA-256 digest",
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

    def test_missing_trust_summary_provenance_is_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("path", "path must be a non-empty string"),
                ("verified_at", "verified_at must be a non-empty string"),
                ("summary_sha256", "summary_sha256 must be a lowercase SHA-256 digest"),
            )
            for key, message in cases:
                with self.subTest(key=key):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    del evidence["trust_summaries"][0][key]
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"missing-trust-{key}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_compact_trust_summary_verified_at_is_rechecked_by_readiness(self):
        cases = [
            ("naive", "2026-06-04T00:00:00", "verified_at must include a timezone"),
            ("malformed", "not-a-timestamp", "verified_at must be an ISO 8601 timestamp"),
            ("future", "2999-01-01T00:00:00+00:00", "verified_at must not be in the future"),
            ("control", "2026-06-04T00:00:00+00:00\nbad", "verified_at must not contain control characters"),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, verified_at, message in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    evidence["trust_summaries"][0]["verified_at"] = verified_at
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"trust-{name}-verified-at.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_malformed_compact_summary_digests_are_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (["canary_summaries", 0], "A" * 64),
                (["trust_summaries", 0], "0" * 63),
            )
            for path_parts, digest in cases:
                with self.subTest(path_parts=path_parts):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    target = evidence
                    for part in path_parts:
                        target = target[part]
                    target["summary_sha256"] = digest
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"malformed-compact-digest-{path_parts[0]}.summary.json",
                        evidence,
                    )

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("summary_sha256 must be a lowercase SHA-256 digest", stderr)

    def test_compact_canary_timestamp_window_is_rechecked_by_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []
            missing = json.loads(evidence_summary.read_text(encoding="utf-8"))
            del missing["canary_summaries"][0]["started_at"]
            cases.append((missing, "started_at must be a non-empty string"))
            naive = json.loads(evidence_summary.read_text(encoding="utf-8"))
            naive["canary_summaries"][0]["started_at"] = "2026-06-04T00:00:00"
            cases.append((naive, "started_at must include a timezone"))
            future = json.loads(evidence_summary.read_text(encoding="utf-8"))
            future["canary_summaries"][0]["finished_at"] = "2999-01-01T00:00:00+00:00"
            cases.append((future, "finished_at must not be in the future"))
            reversed_window = json.loads(evidence_summary.read_text(encoding="utf-8"))
            reversed_window["canary_summaries"][0]["started_at"] = "2026-06-04T00:00:02+00:00"
            reversed_window["canary_summaries"][0]["finished_at"] = "2026-06-04T00:00:01+00:00"
            cases.append((reversed_window, "finished_at must not be before started_at"))
            for offset, (body, message) in enumerate(cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(root / f"canary-time-{offset}.summary.json", body)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_compact_stage_windows_are_rechecked_by_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []
            missing = json.loads(evidence_summary.read_text(encoding="utf-8"))
            del missing["canary_summaries"][0]["stage_windows"][0]["started_at"]
            cases.append((missing, "started_at must be a non-empty string"))
            future = json.loads(evidence_summary.read_text(encoding="utf-8"))
            future["canary_summaries"][0]["stage_windows"][0]["finished_at"] = "2999-01-01T00:00:00+00:00"
            cases.append((future, "finished_at must not be in the future"))
            reversed_window = json.loads(evidence_summary.read_text(encoding="utf-8"))
            reversed_window["canary_summaries"][0]["stage_windows"][0]["started_at"] = "2026-06-04T00:00:01+00:00"
            reversed_window["canary_summaries"][0]["stage_windows"][0]["finished_at"] = "2026-06-04T00:00:00+00:00"
            cases.append((reversed_window, "finished_at must not be before started_at"))
            outside_canary = json.loads(evidence_summary.read_text(encoding="utf-8"))
            outside_canary["canary_summaries"][0]["stage_windows"][0]["started_at"] = "2026-06-03T23:59:59+00:00"
            cases.append((outside_canary, "timestamp window must be inside canary window"))
            overlapping = json.loads(evidence_summary.read_text(encoding="utf-8"))
            overlapping["canary_summaries"][0]["stage_windows"][1]["started_at"] = (
                "2026-06-04T00:00:00.100000+00:00"
            )
            cases.append(
                (
                    overlapping,
                    "started_at must not be before previous stage finished_at",
                )
            )
            mismatch = json.loads(evidence_summary.read_text(encoding="utf-8"))
            mismatch["canary_summaries"][0]["stage_windows"][0]["name"] = "extra"
            cases.append((mismatch, "stage_windows must match stage_names"))
            reordered = json.loads(evidence_summary.read_text(encoding="utf-8"))
            windows = reordered["canary_summaries"][0]["stage_windows"]
            windows[0]["name"], windows[1]["name"] = windows[1]["name"], windows[0]["name"]
            cases.append((reordered, "stage_windows must match stage_names"))
            for offset, (body, message) in enumerate(cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(root / f"stage-window-{offset}.summary.json", body)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_compact_stage_names_are_unique_and_supported(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []
            duplicate = json.loads(evidence_summary.read_text(encoding="utf-8"))
            canary = duplicate["canary_summaries"][0]
            canary["stage_names"] = ["rail", "rail", "notary", "verify"]
            canary["stage_windows"].insert(1, dict(canary["stage_windows"][0]))
            cases.append((duplicate, "stage_names must not contain duplicates"))
            unsupported = json.loads(evidence_summary.read_text(encoding="utf-8"))
            canary = unsupported["canary_summaries"][0]
            canary["stage_names"].append("diagnostic")
            extra_window = dict(canary["stage_windows"][0])
            extra_window["name"] = "diagnostic"
            canary["stage_windows"].append(extra_window)
            cases.append((unsupported, "stage_names contains unsupported stages"))
            for offset, (body, message) in enumerate(cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(root / f"stage-names-{offset}.summary.json", body)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_canary_without_explicit_policy_proof_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["canary_summaries"][0]["require_explicit_policy"] = False
            refresh_digest(evidence)
            mutated_path = write_json(root / "implicit-policy-canary.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.canary_implicit_policy", codes)

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
            canary["stage_windows"] = [
                window for window in canary["stage_windows"] if window["name"] in {"rail", "notary"}
            ]
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

    def test_missing_trust_revocation_proof_is_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "x509_require_crl_revocation_check",
                    "x509_require_crl_revocation_check must be a boolean",
                ),
                ("x509_crl_count", "x509_crl_count must be a non-negative integer"),
                (
                    "x509_require_ocsp_revocation_check",
                    "x509_require_ocsp_revocation_check must be a boolean",
                ),
                (
                    "x509_ocsp_response_count",
                    "x509_ocsp_response_count must be a non-negative integer",
                ),
            )
            for flag, message in cases:
                with self.subTest(flag=flag):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    profile = evidence["trust_summaries"][0]["profiles"][0]
                    del profile[flag]
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"missing-{flag}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_trust_verified_bundle_count_must_match_profiles(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["trust_summaries"][0]["verified_bundles"] = 2
            refresh_digest(evidence)
            mutated_path = write_json(root / "mismatched-trust-count.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.profile_count_mismatch", codes)

    def test_trust_profile_ids_must_be_unique(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            trust_summary = evidence["trust_summaries"][0]
            trust_summary["profiles"].append(dict(trust_summary["profiles"][0]))
            trust_summary["verified_bundles"] = 2
            refresh_digest(evidence)
            mutated_path = write_json(root / "duplicate-trust-profile.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.profile_id_duplicate", codes)

    def test_weak_trust_revocation_posture_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            profile = evidence["trust_summaries"][0]["profiles"][0]
            profile["x509_require_crl_revocation_check"] = False
            profile["x509_crl_count"] = 0
            profile["x509_require_ocsp_revocation_check"] = False
            profile["x509_ocsp_response_count"] = 0
            refresh_digest(evidence)
            mutated_path = write_json(root / "weak-revocation.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.crl_revocation_not_required", codes)
            self.assertIn("trust.ocsp_revocation_not_required", codes)

    def test_missing_required_revocation_material_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            profile = evidence["trust_summaries"][0]["profiles"][0]
            profile["x509_crl_count"] = 0
            profile["x509_ocsp_response_count"] = 0
            refresh_digest(evidence)
            mutated_path = write_json(root / "missing-revocation-material.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.no_crl_revocation_material", codes)
            self.assertIn("trust.no_ocsp_revocation_material", codes)

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

    def test_canary_receipt_entries_must_be_unique(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("path", "evidence.receipt_path_duplicate"),
                ("receipt_sha256", "evidence.receipt_digest_duplicate"),
            )
            for field, code in cases:
                with self.subTest(field=field):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    receipt_summary = evidence["canary_summaries"][0]["receipt_summary"]
                    receipt_summary["receipts"][1][field] = receipt_summary["receipts"][0][field]
                    refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    weak_path = write_json(
                        root / f"duplicate-canary-receipt-{field}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(weak_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_archive_receipt_entries_must_be_unique(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("path", "evidence.archive_receipt_path_duplicate"),
                ("receipt_sha256", "evidence.archive_receipt_digest_duplicate"),
            )
            for field, code in cases:
                with self.subTest(field=field):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    archive = evidence["receipt_verification"]
                    archive["receipts"][1][field] = archive["receipts"][0][field]
                    refresh_digest(archive)
                    refresh_digest(evidence)
                    weak_path = write_json(
                        root / f"duplicate-archive-receipt-{field}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(weak_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)


if __name__ == "__main__":
    unittest.main()
