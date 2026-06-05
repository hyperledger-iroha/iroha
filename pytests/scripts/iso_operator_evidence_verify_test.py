import base64
import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

from pytests.scripts import iso_audit_notary_adapter_test as audit_test
from pytests.scripts import iso_operator_receipt_verify_test as receipt_test
from pytests.scripts import iso_rail_gateway_adapter_test as rail_test
from pytests.scripts import iso_trust_bundle_verify_test as trust_test


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "iso_operator_evidence_verify.py"
SPEC = importlib.util.spec_from_file_location("iso_operator_evidence_verify", SCRIPT_PATH)
EVIDENCE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = EVIDENCE
SPEC.loader.exec_module(EVIDENCE)


def digest_summary(body):
    body.pop(EVIDENCE.SUMMARY_DIGEST_FIELD, None)
    body[EVIDENCE.SUMMARY_DIGEST_FIELD] = EVIDENCE.sha256_hex(
        EVIDENCE._canonical_json_bytes(body)
    )
    return body


def digest_receipt_summary(body):
    body.pop(EVIDENCE.SUMMARY_DIGEST_FIELD, None)
    body[EVIDENCE.SUMMARY_DIGEST_FIELD] = EVIDENCE.sha256_hex(
        EVIDENCE._canonical_json_bytes(body)
    )
    return body


def write_json(path, body):
    path.write_text(json.dumps(body, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def receipt_stdout(
    receipt_kind=None,
    *,
    verified_receipts=2,
    allow_failed=False,
    allow_insecure_http=False,
    allow_legacy_colr007=False,
    require_source_files=True,
    receipt_entries=None,
):
    if receipt_entries is None:
        kinds = receipt_kind or ["iso-audit-notary", "iso-rail-gateway"]
        receipts = [
            {
                "path": f"/ops/iso/receipts/{kind}.{offset}.receipt.json",
                "receipt_kind": kind,
                "receipt_sha256": f"{offset + 1:064x}",
            }
            for offset, kind in enumerate(kinds[: max(verified_receipts, 0)])
        ]
    else:
        receipts = list(receipt_entries)
        verified_receipts = len(receipts)
        kinds = receipt_kind or sorted({receipt["receipt_kind"] for receipt in receipts})
    return (
        json.dumps(
            digest_receipt_summary(
                {
                    "verified_receipts": verified_receipts,
                    "receipt_kind": kinds,
                    "allow_failed": allow_failed,
                    "allow_insecure_http": allow_insecure_http,
                    "allow_legacy_colr007": allow_legacy_colr007,
                    "require_source_files": require_source_files,
                    "receipts": receipts,
                }
            ),
            sort_keys=True,
        )
        + "\n"
    )


def stage(
    name,
    command,
    receipt_dir=None,
    stdout=None,
    *,
    started_at="2026-06-04T00:00:00+00:00",
    finished_at="2026-06-04T00:00:01+00:00",
):
    return {
        "name": name,
        "started_at": started_at,
        "finished_at": finished_at,
        "returncode": 0,
        "command": command,
        "stdout_preview": stdout if stdout is not None else "{}\n",
        "stderr_preview": "",
        "stdout_truncated": False,
        "stderr_truncated": False,
        "receipt_dir": receipt_dir,
        "skipped": False,
        "reason": None,
    }


def rail_command():
    return [
        sys.executable,
        str(REPO_ROOT / "scripts" / "iso_rail_gateway_adapter.py"),
        "--inbox-dir",
        "/ops/iso/inbox",
        "--torii-base-url",
        "https://torii.example.invalid",
        "--receipt-dir",
        "/ops/iso/rail-receipts",
        "--bearer-token-file",
        "<runtime-token-file>",
    ]


def notary_command():
    return [
        sys.executable,
        str(REPO_ROOT / "scripts" / "iso_audit_notary_adapter.py"),
        "--export-dir",
        "/ops/iso/audit-export",
        "--receipt-dir",
        "/ops/iso/notary-receipts",
        "--endpoint",
        "https://notary.example.invalid/iso-anchor",
    ]


def verify_command():
    return [
        sys.executable,
        str(REPO_ROOT / "scripts" / "iso_operator_receipt_verify.py"),
        "--receipt-dir",
        "/ops/iso/rail-receipts",
        "--receipt-dir",
        "/ops/iso/notary-receipts",
        "--require-source-files",
    ]


def valid_canary_summary(*, receipt_entries=None):
    return digest_summary(
        {
            "provider": "local-bank",
            "environment": "preprod",
            "config_path": "/ops/iso/canary.json",
            "started_at": "2026-06-04T00:00:00+00:00",
            "finished_at": "2026-06-04T00:00:01+00:00",
            "ok": True,
            "plan_only": False,
            "policy": {
                "require_explicit_policy": True,
            },
            "stages": [
                stage(
                    "rail",
                    rail_command(),
                    "/ops/iso/rail-receipts",
                    started_at="2026-06-04T00:00:00+00:00",
                    finished_at="2026-06-04T00:00:00.200000+00:00",
                ),
                stage(
                    "notary",
                    notary_command(),
                    "/ops/iso/notary-receipts",
                    started_at="2026-06-04T00:00:00.200000+00:00",
                    finished_at="2026-06-04T00:00:00.400000+00:00",
                ),
                stage(
                    "verify",
                    verify_command(),
                    stdout=receipt_stdout(receipt_entries=receipt_entries),
                    started_at="2026-06-04T00:00:00.400000+00:00",
                    finished_at="2026-06-04T00:00:01+00:00",
                ),
            ],
        }
    )


def plan_only_canary_summary():
    return digest_summary(
        {
            "provider": "local-bank",
            "environment": "preprod",
            "config_path": "/ops/iso/canary.json",
            "started_at": "2026-06-04T00:00:00+00:00",
            "finished_at": "2026-06-04T00:00:00+00:00",
            "ok": True,
            "plan_only": True,
            "policy": {
                "require_explicit_policy": True,
            },
            "planned_stages": [
                {
                    "name": "rail",
                    "command": rail_command(),
                    "receipt_dir": "/ops/iso/rail-receipts",
                    "dry_run": False,
                },
                {
                    "name": "notary",
                    "command": notary_command(),
                    "receipt_dir": "/ops/iso/notary-receipts",
                    "dry_run": False,
                },
                {
                    "name": "verify",
                    "command": verify_command(),
                    "receipt_dir": None,
                    "dry_run": False,
                },
            ],
        }
    )


def write_canary(root, body=None):
    return write_json(root / "canary.summary.json", body or valid_canary_summary())


def write_trust_summary(
    root,
    *,
    synthetic=False,
    record_only=False,
    insecure_source=False,
    missing_source=False,
    emit_profile_json=True,
):
    root.mkdir(parents=True, exist_ok=True)
    bundle = trust_test.valid_bundle()
    argv = []
    if synthetic:
        bundle["x509_trust_anchors"][0]["der_base64"] = trust_test.SYNTHETIC_DER_B64
        bundle["x509_trust_anchors"][0]["sha256"] = trust_test.der_digest(
            trust_test.SYNTHETIC_DER_B64
        )
        argv.append("--allow-synthetic-der")
    if record_only:
        bundle["embedded_signature_policy"] = "record-only"
        argv.append("--allow-record-only")
    if insecure_source:
        bundle["source"]["url"] = "http://pki.local/swift-cbpr-plus"
        argv.append("--allow-insecure-source-url")
    if missing_source:
        bundle.pop("source")
    bundle_path = trust_test.write_bundle(root, bundle)
    summary_path = root / "trust.summary.json"
    profile_path = root / "trust.profile.json"
    if emit_profile_json and not synthetic:
        argv.extend(["--emit-profile-json", str(profile_path)])
    rc, _stdout, stderr = trust_test.run_verify(
        ["--bundle", str(bundle_path), "--summary-out", str(summary_path)] + argv
    )
    if rc != 0:
        raise AssertionError(stderr)
    return summary_path


def rewrite_trust_summary(path, mutate):
    summary = json.loads(path.read_text(encoding="utf-8"))
    mutate(summary)
    write_json(path, digest_summary(summary))
    return summary


def refresh_profile_json_sha256(summary):
    if summary.get("profile_json_emitted"):
        profile_config = [bundle["profile_overrides"] for bundle in summary["bundles"]]
        profile_text = json.dumps(profile_config, indent=2, sort_keys=True) + "\n"
        summary["profile_json_sha256"] = EVIDENCE.sha256_hex(profile_text.encode("utf-8"))
    return summary


def write_https_receipt_dirs(root, *, legacy_colr007=False):
    export_dir = root / "audit-export"
    export_dir.mkdir()
    audit_test.write_export(
        export_dir,
        store_dir=root / "audit-store",
        write_record_sources_flag=True,
    )
    with audit_test.capture_server() as (endpoint, _requests):
        rc, _stdout, stderr = audit_test.run_main(
            [
                "--export-dir",
                str(export_dir),
                "--endpoint",
                endpoint,
                "--allow-insecure-http",
            ]
        )
    if rc != 0:
        raise AssertionError(stderr)
    notary_receipt = next((export_dir / "receipts").glob("*.receipt.json"))
    notary_endpoint = "https://notary.example.invalid/iso-anchor"
    receipt_test.rewrite_receipt(
        notary_receipt,
        lambda body: body.update(
            {
                "endpoint": notary_endpoint,
                "endpoint_sha256": EVIDENCE.sha256_hex(
                    notary_endpoint.encode("utf-8")
                ),
            }
        ),
    )

    inbox = root / "rail-inbox"
    inbox.mkdir()
    if legacy_colr007:
        rail_test.write_message(
            inbox,
            message_type="colr.007",
            profile="securities-csd",
            payload=b"<Document><CollSbstitnConf/></Document>",
        )
    else:
        rail_test.write_message(inbox)
    with rail_test.capture_server() as (base_url, _requests):
        argv = [
            "--inbox-dir",
            str(inbox),
            "--torii-base-url",
            base_url,
            "--allow-insecure-http",
        ]
        if legacy_colr007:
            argv.append("--allow-legacy-colr007")
        rc, _stdout, stderr = rail_test.run_main(
            argv
        )
    if rc != 0:
        raise AssertionError(stderr)
    rail_receipt = next((inbox / "receipts").glob("*.receipt.json"))
    rail_endpoint = (
        "https://torii.example.invalid/v1/iso20022/colr007"
        if legacy_colr007
        else "https://torii.example.invalid/v1/iso20022/pacs002"
    )
    receipt_test.rewrite_receipt(
        rail_receipt,
        lambda body: body.update(
            {
                "endpoint_url": rail_endpoint,
                "endpoint_sha256": EVIDENCE.sha256_hex(
                    rail_endpoint.encode("utf-8")
                ),
            }
        ),
    )

    return export_dir / "receipts", inbox / "receipts"


def receipt_entries_from_dirs(*receipt_dirs):
    entries = []
    for receipt_dir in receipt_dirs:
        for path in sorted(Path(receipt_dir).glob("*.receipt.json")):
            receipt = json.loads(path.read_text(encoding="utf-8"))
            entries.append(
                {
                    "path": f"/ops/iso/receipts/{path.name}",
                    "receipt_kind": receipt["receipt_kind"],
                    "receipt_sha256": receipt["receipt_sha256"],
                }
            )
    return entries


FRESHNESS_FLAGS = {
    "--max-canary-age-days": "36500",
    "--max-trust-age-days": "36500",
    "--max-trust-source-age-days": "36500",
}


def _has_flag(argv, flag):
    return any(item == flag or item.startswith(flag + "=") for item in argv)


def run_evidence(argv, *, include_context=True, include_freshness=True):
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
        rc = EVIDENCE.main(argv)
    return rc, stdout.getvalue(), stderr.getvalue()


class IsoOperatorEvidenceVerifyTest(unittest.TestCase):
    def test_valid_canary_and_trust_summaries_pass(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            canary_path = write_canary(
                root,
                valid_canary_summary(
                    receipt_entries=receipt_entries_from_dirs(notary_receipts, rail_receipts)
                ),
            )
            trust_path = write_trust_summary(root)
            summary_out = root / "evidence.summary.json"

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--provider",
                    "local-bank",
                    "--environment",
                    "preprod",
                    "--receipt-dir",
                    str(notary_receipts),
                    "--receipt-dir",
                    str(rail_receipts),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["ok"])
            self.assertEqual(summary["version"], 1)
            self.assertEqual(summary["policy"]["provider"], "local-bank")
            self.assertEqual(summary["policy"]["environment"], "preprod")
            self.assertFalse(summary["policy"]["allow_canary_stage_receipts_only"])
            self.assertEqual(summary["policy"]["max_canary_age_days"], 36500)
            self.assertEqual(summary["policy"]["max_trust_age_days"], 36500)
            self.assertEqual(summary["policy"]["max_trust_source_age_days"], 36500)
            self.assertEqual(summary["canary_summaries"][0]["config_path"], "/ops/iso/canary.json")
            self.assertEqual(summary["canary_summaries"][0]["started_at"], "2026-06-04T00:00:00+00:00")
            self.assertEqual(summary["canary_summaries"][0]["finished_at"], "2026-06-04T00:00:01+00:00")
            self.assertEqual(summary["canary_summaries"][0]["stage_names"], ["rail", "notary", "verify"])
            self.assertEqual(
                [stage["name"] for stage in summary["canary_summaries"][0]["stage_windows"]],
                ["rail", "notary", "verify"],
            )
            self.assertEqual(
                summary["canary_summaries"][0]["receipt_summary"]["receipt_kind"],
                ["iso-audit-notary", "iso-rail-gateway"],
            )
            self.assertEqual(
                len(summary["canary_summaries"][0]["receipt_summary"]["receipts"]),
                2,
            )
            self.assertEqual(summary["receipt_verification"]["verified_receipts"], 2)
            self.assertIn(
                "summary_sha256",
                summary["canary_summaries"][0]["receipt_summary"],
            )
            self.assertRegex(
                summary["trust_summaries"][0]["verified_at"],
                r"^\d{4}-\d{2}-\d{2}T",
            )
            self.assertEqual(summary["trust_summaries"][0]["verified_bundles"], 1)
            self.assertTrue(summary["trust_summaries"][0]["profile_json_emitted"])
            self.assertTrue(summary["trust_summaries"][0]["profile_json_emittable"])
            self.assertRegex(
                summary["trust_summaries"][0]["profile_json_sha256"],
                r"^[0-9a-f]{64}$",
            )
            trust_profile = summary["trust_summaries"][0]["profiles"][0]
            self.assertRegex(trust_profile["bundle_sha256"], r"^[0-9a-f]{64}$")
            self.assertEqual(
                trust_profile["source"],
                {
                    "url": "https://pki.example.invalid/swift-cbpr-plus",
                    "retrieved_at": "2026-06-04T00:00:00Z",
                },
            )
            self.assertTrue(trust_profile["x509_require_crl_revocation_check"])
            self.assertEqual(trust_profile["x509_crl_count"], 1)
            self.assertEqual(len(trust_profile["x509_crl_der"]), 1)
            self.assertRegex(trust_profile["x509_crl_der"][0]["sha256"], r"^[0-9a-f]{64}$")
            self.assertGreater(trust_profile["x509_crl_der"][0]["byte_len"], 0)
            self.assertTrue(trust_profile["x509_require_ocsp_revocation_check"])
            self.assertEqual(trust_profile["x509_ocsp_response_count"], 1)
            self.assertEqual(len(trust_profile["x509_ocsp_response_der"]), 1)
            self.assertRegex(
                trust_profile["x509_ocsp_response_der"][0]["sha256"],
                r"^[0-9a-f]{64}$",
            )
            self.assertGreater(trust_profile["x509_ocsp_response_der"][0]["byte_len"], 0)
            self.assertEqual(trust_profile["revoked_certificate_pin_count"], 1)
            self.assertEqual(len(trust_profile["x509_trust_anchor_der"]), 1)
            self.assertEqual(len(trust_profile["revoked_certificate_der"]), 1)
            self.assertEqual(trust_profile["x509_required_certificate_policy_oid_count"], 1)
            self.assertEqual(json.loads(summary_out.read_text(encoding="utf-8")), summary)
            body = dict(summary)
            digest = body.pop("summary_sha256")
            self.assertEqual(digest, EVIDENCE.sha256_hex(EVIDENCE._canonical_json_bytes(body)))

    def test_symlinked_summary_output_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            canary_path = write_canary(
                root,
                valid_canary_summary(
                    receipt_entries=receipt_entries_from_dirs(notary_receipts, rail_receipts)
                ),
            )
            trust_path = write_trust_summary(root / "trust")
            target = root / "evidence-target.summary.json"
            target.write_text("untouched\n", encoding="utf-8")
            summary_out = root / "evidence-link.summary.json"
            try:
                summary_out.symlink_to(target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--receipt-dir",
                    str(notary_receipts),
                    "--receipt-dir",
                    str(rail_receipts),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must not be a symlink", stderr)
            self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")

    def test_symlinked_summary_inputs_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            canary_target_dir = root / "canary-target"
            canary_target_dir.mkdir()
            canary_target = write_canary(canary_target_dir)
            canary_link = root / "canary-link.summary.json"
            try:
                canary_link.symlink_to(canary_target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_link), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must not be a symlink", stderr)

            canary_dir = root / "canary"
            canary_dir.mkdir()
            canary_path = write_canary(canary_dir)
            trust_target = write_trust_summary(root / "trust-target")
            trust_link = root / "trust-link.summary.json"
            try:
                trust_link.symlink_to(trust_target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_link)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must not be a symlink", stderr)

    def test_directory_summary_inputs_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            canary_dir = root / "canary-dir.summary.json"
            canary_dir.mkdir()

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_dir), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must be a regular file", stderr)

            canary_root = root / "canary"
            canary_root.mkdir()
            canary_path = write_canary(canary_root)
            trust_dir = root / "trust-dir.summary.json"
            trust_dir.mkdir()

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_dir)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must be a regular file", stderr)

    def test_canary_config_path_is_canonical(self):
        cases = (
            ("/ops/iso/canary\n.json", "config_path must not contain control characters"),
            ("/ops/iso/../canary.json", "config_path must not contain dot or parent segments"),
            (r"..\canary.json", "config_path must not contain dot or parent segments"),
            ("/ops/iso/canary.txt", "config_path must point to a .json file"),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            for offset, (config_path, message) in enumerate(cases):
                with self.subTest(config_path=config_path):
                    body = valid_canary_summary()
                    body["config_path"] = config_path
                    canary_path = write_json(
                        root / f"bad-config-{offset}.summary.json",
                        digest_summary(body),
                    )

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_canary_and_trust_summaries_are_required(self):
        rc, _stdout, stderr = run_evidence([])

        self.assertEqual(rc, 2)
        self.assertIn("provide at least one --canary-summary", stderr)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)

            rc, _stdout, stderr = run_evidence(["--canary-summary", str(canary_path)])

            self.assertEqual(rc, 2)
            self.assertIn("provide at least one --trust-summary", stderr)

    def test_provider_and_environment_are_required_evidence_context(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            base_argv = [
                "--canary-summary",
                str(canary_path),
                "--trust-summary",
                str(trust_path),
            ]

            rc, _stdout, stderr = run_evidence(base_argv, include_context=False)
            self.assertEqual(rc, 2)
            self.assertIn("provide --provider", stderr)

            rc, _stdout, stderr = run_evidence(
                base_argv + ["--provider", "local-bank"],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("provide --environment", stderr)

            rc, _stdout, stderr = run_evidence(
                base_argv + ["--provider", "local-bank", "--environment", " "],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("provide --environment", stderr)

            rc, _stdout, stderr = run_evidence(
                base_argv + ["--provider", " local-bank", "--environment", "preprod"],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("--provider must not have surrounding whitespace", stderr)

            rc, _stdout, stderr = run_evidence(
                base_argv + ["--provider", "local-bank", "--environment", "preprod "],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("--environment must not have surrounding whitespace", stderr)

    def test_evidence_freshness_budgets_are_required_and_positive(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            base_argv = [
                "--canary-summary",
                str(canary_path),
                "--trust-summary",
                str(trust_path),
                "--provider",
                "local-bank",
                "--environment",
                "preprod",
            ]

            rc, _stdout, stderr = run_evidence(
                base_argv,
                include_context=False,
                include_freshness=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("provide --max-canary-age-days", stderr)

            for flag in FRESHNESS_FLAGS:
                with self.subTest(flag=flag):
                    argv = list(base_argv)
                    for other_flag, value in FRESHNESS_FLAGS.items():
                        argv.extend([other_flag, "0" if other_flag == flag else value])

                    rc, _stdout, stderr = run_evidence(
                        argv,
                        include_context=False,
                        include_freshness=False,
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(f"{flag} must be a positive integer", stderr)

    def test_stale_digest_correct_canary_trust_and_source_are_rejected(self):
        old_start = "2000-01-01T00:00:00+00:00"
        old_rail_done = "2000-01-01T00:00:01+00:00"
        old_notary_done = "2000-01-01T00:00:02+00:00"
        old_finish = "2000-01-01T00:00:03+00:00"

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")

            stale_canary = valid_canary_summary()
            stale_canary["started_at"] = old_start
            stale_canary["finished_at"] = old_finish
            stale_canary["stages"][0]["started_at"] = old_start
            stale_canary["stages"][0]["finished_at"] = old_rail_done
            stale_canary["stages"][1]["started_at"] = old_rail_done
            stale_canary["stages"][1]["finished_at"] = old_notary_done
            stale_canary["stages"][2]["started_at"] = old_notary_done
            stale_canary["stages"][2]["finished_at"] = old_finish
            stale_canary.pop("summary_sha256")
            stale_canary_path = write_canary(root, digest_summary(stale_canary))

            stale_trust_path = write_trust_summary(root / "stale-trust")
            rewrite_trust_summary(
                stale_trust_path,
                lambda body: body.update({"verified_at": old_start}),
            )

            stale_source_path = write_trust_summary(root / "stale-source")
            rewrite_trust_summary(
                stale_source_path,
                lambda body: body["bundles"][0]["source"].update(
                    {"retrieved_at": old_start}
                ),
            )
            fresh_canary_root = root / "fresh-canary"
            fresh_canary_root.mkdir()
            fresh_canary_path = write_canary(fresh_canary_root)
            fresh_canary_source_root = root / "fresh-canary-source"
            fresh_canary_source_root.mkdir()
            fresh_canary_source_path = write_canary(fresh_canary_source_root)

            cases = (
                (
                    ["--canary-summary", str(stale_canary_path), "--trust-summary", str(trust_path), "--max-canary-age-days", "1"],
                    "finished_at is older than the 1-day freshness budget",
                ),
                (
                    ["--canary-summary", str(fresh_canary_path), "--trust-summary", str(stale_trust_path), "--max-trust-age-days", "1"],
                    "verified_at is older than the 1-day freshness budget",
                ),
                (
                    ["--canary-summary", str(fresh_canary_source_path), "--trust-summary", str(stale_source_path), "--max-trust-source-age-days", "1"],
                    "source.retrieved_at is older than the 1-day freshness budget",
                ),
            )
            for argv, message in cases:
                with self.subTest(message=message):
                    rc, _stdout, stderr = run_evidence(argv)

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_duplicate_canary_and_trust_inputs_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_root = root / "canary"
            canary_root.mkdir()
            canary_path = write_canary(canary_root)
            trust_path = write_trust_summary(root / "trust")
            copied_canary = root / "copied-canary.summary.json"
            copied_canary.write_text(canary_path.read_text(encoding="utf-8"), encoding="utf-8")
            copied_trust = root / "copied-trust.summary.json"
            copied_trust.write_text(trust_path.read_text(encoding="utf-8"), encoding="utf-8")
            cases = (
                (
                    [
                        "--canary-summary",
                        str(canary_path),
                        "--canary-summary",
                        str(canary_path),
                        "--trust-summary",
                        str(trust_path),
                    ],
                    "--canary-summary[1] duplicates --canary-summary[0]",
                ),
                (
                    [
                        "--canary-summary",
                        str(canary_path),
                        "--trust-summary",
                        str(trust_path),
                        "--trust-summary",
                        str(trust_path),
                    ],
                    "--trust-summary[1] duplicates --trust-summary[0]",
                ),
                (
                    [
                        "--canary-summary",
                        str(canary_path),
                        "--canary-summary",
                        str(copied_canary),
                        "--trust-summary",
                        str(trust_path),
                    ],
                    "canary_summaries[1].summary_sha256 duplicates canary_summaries[0].summary_sha256",
                ),
                (
                    [
                        "--canary-summary",
                        str(canary_path),
                        "--trust-summary",
                        str(trust_path),
                        "--trust-summary",
                        str(copied_trust),
                    ],
                    "trust_summaries[1].summary_sha256 duplicates trust_summaries[0].summary_sha256",
                ),
            )
            for argv, message in cases:
                with self.subTest(message=message):
                    rc, _stdout, stderr = run_evidence(argv)

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_duplicate_canary_summary_json_keys_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = root / "canary.summary.json"
            canary_path.write_text(
                '{"provider":"local-bank","provider":"other-bank"}\n',
                encoding="utf-8",
            )
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)

    def test_duplicate_receipt_stdout_json_keys_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            body = valid_canary_summary()
            body["stages"][2]["stdout_preview"] = (
                '{"verified_receipts":2,"verified_receipts":3}\n'
            )
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)

    def test_duplicate_direct_receipt_verifier_stdout_json_keys_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")

            class Completed:
                returncode = 0
                stdout = '{"verified_receipts":2,"verified_receipts":3}\n'
                stderr = ""

            original_run = EVIDENCE.subprocess.run
            EVIDENCE.subprocess.run = lambda *_args, **_kwargs: Completed()
            try:
                rc, _stdout, stderr = run_evidence(
                    [
                        "--canary-summary",
                        str(canary_path),
                        "--trust-summary",
                        str(trust_path),
                        "--receipt-dir",
                        str(root / "receipts"),
                    ]
                )
            finally:
                EVIDENCE.subprocess.run = original_run

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)

    def test_duplicate_trust_profile_ids_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            trust = json.loads(trust_path.read_text(encoding="utf-8"))
            trust["bundles"].append(dict(trust["bundles"][0]))
            trust["verified_bundles"] = 2
            refresh_profile_json_sha256(trust)
            duplicate_trust_path = write_json(
                root / "duplicate-trust-profile.summary.json",
                digest_summary(trust),
            )

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(duplicate_trust_path),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("profile_id duplicates", stderr)

    def test_direct_receipt_archive_verification_is_preserved(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            canary_path = write_canary(
                root,
                valid_canary_summary(
                    receipt_entries=receipt_entries_from_dirs(notary_receipts, rail_receipts)
                ),
            )
            trust_path = write_trust_summary(root / "trust")

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--receipt-dir",
                    str(notary_receipts),
                    "--receipt-dir",
                    str(rail_receipts),
                    "--provider",
                    "local-bank",
                    "--environment",
                    "preprod",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            receipt_summary = summary["receipt_verification"]
            self.assertEqual(receipt_summary["verified_receipts"], 2)
            self.assertEqual(
                receipt_summary["receipt_kind"],
                ["iso-audit-notary", "iso-rail-gateway"],
            )
            self.assertFalse(receipt_summary["allow_failed"])
            self.assertFalse(receipt_summary["allow_insecure_http"])
            self.assertFalse(receipt_summary["allow_legacy_colr007"])
            self.assertTrue(receipt_summary["require_source_files"])
            self.assertEqual(len(receipt_summary["receipts"]), 2)
            body = dict(receipt_summary)
            digest = body.pop("summary_sha256")
            self.assertEqual(digest, EVIDENCE.sha256_hex(EVIDENCE._canonical_json_bytes(body)))

    def test_direct_receipt_archive_verification_is_required_by_default(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("provide --receipt or --receipt-dir", stderr)

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-canary-stage-receipts-only",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["policy"]["allow_canary_stage_receipts_only"])
            self.assertIsNone(summary["receipt_verification"])

    def test_direct_receipt_archive_must_cover_canary_receipt_digests(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--receipt-dir",
                    str(notary_receipts),
                    "--receipt-dir",
                    str(rail_receipts),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("direct receipt archive verification does not include", stderr)

    def test_direct_receipt_archive_must_bind_canary_receipt_kinds(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            entries[0]["receipt_kind"], entries[1]["receipt_kind"] = (
                entries[1]["receipt_kind"],
                entries[0]["receipt_kind"],
            )
            canary_path = write_canary(
                root,
                valid_canary_summary(receipt_entries=entries),
            )
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--receipt-dir",
                    str(notary_receipts),
                    "--receipt-dir",
                    str(rail_receipts),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("direct receipt archive verification binds", stderr)

    def test_direct_receipt_archive_must_not_include_unreferenced_receipts(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            original_receipt = next(Path(notary_receipts).glob("*.receipt.json"))
            extra_receipt = Path(notary_receipts) / "extra-unreferenced.receipt.json"
            extra_receipt.write_bytes(original_receipt.read_bytes())
            receipt_test.rewrite_receipt(
                extra_receipt,
                lambda body: body.update(
                    {
                        "response_body_sha256": EVIDENCE.sha256_hex(b"extra"),
                        "response_body_preview": "extra",
                    }
                ),
            )
            canary_path = write_canary(
                root,
                valid_canary_summary(receipt_entries=entries),
            )
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--receipt-dir",
                    str(notary_receipts),
                    "--receipt-dir",
                    str(rail_receipts),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("includes unreferenced receipt_verification.receipts", stderr)

    def test_canary_receipts_cannot_be_reused_across_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            canary_one = write_json(
                root / "canary-one.summary.json",
                valid_canary_summary(receipt_entries=entries),
            )
            body_two = valid_canary_summary(receipt_entries=entries)
            body_two["config_path"] = "/ops/iso/canary-two.json"
            body_two.pop("summary_sha256")
            canary_two = write_json(
                root / "canary-two.summary.json",
                digest_summary(body_two),
            )
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_one),
                    "--canary-summary",
                    str(canary_two),
                    "--trust-summary",
                    str(trust_path),
                    "--receipt-dir",
                    str(notary_receipts),
                    "--receipt-dir",
                    str(rail_receipts),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("receipt_summary.receipts[0].path duplicates", stderr)

    def test_receipt_summary_kind_list_must_be_unique(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            receipt_summary = json.loads(receipt_stdout())
            receipt_summary["receipt_kind"].append("iso-audit-notary")
            body = valid_canary_summary()
            body["stages"][2]["stdout_preview"] = (
                json.dumps(
                    digest_receipt_summary(receipt_summary),
                    sort_keys=True,
                )
                + "\n"
            )
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-canary-stage-receipts-only",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("receipt_kind[2] duplicates", stderr)

    def test_symlinked_direct_receipt_archive_dir_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            target = root / "receipt-target"
            target.mkdir()
            receipt_link = root / "receipt-link"
            try:
                receipt_link.symlink_to(target, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--receipt-dir",
                    str(receipt_link),
                    "--provider",
                    "local-bank",
                    "--environment",
                    "preprod",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must not be a symlink", stderr)

    def test_legacy_colr007_archive_receipts_require_explicit_local_override(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(
                root,
                legacy_colr007=True,
            )
            canary_path = write_canary(
                root,
                valid_canary_summary(
                    receipt_entries=receipt_entries_from_dirs(notary_receipts, rail_receipts)
                ),
            )
            trust_path = write_trust_summary(root / "trust")
            argv = [
                "--canary-summary",
                str(canary_path),
                "--trust-summary",
                str(trust_path),
                "--receipt-dir",
                str(notary_receipts),
                "--receipt-dir",
                str(rail_receipts),
                "--provider",
                "local-bank",
                "--environment",
                "preprod",
            ]

            rc, _stdout, stderr = run_evidence(argv)

            self.assertEqual(rc, 2)
            self.assertIn("legacy rail message_type", stderr)

            rc, stdout, stderr = run_evidence(argv + ["--allow-legacy-colr007"])

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["policy"]["allow_legacy_colr007"])
            self.assertTrue(summary["receipt_verification"]["allow_legacy_colr007"])

    def test_smuggled_trust_source_urls_are_rejected(self):
        cases = [
            ("https://user:pass@pki.example/source", []),
            ("https://pki.example/source;debug", []),
            ("https://pki.example/source?debug=true", []),
            ("https://pki.example/source#fragment", []),
            ("https:///source", []),
            ("https://[::1", []),
            ("https://pki.example/source\nbad", []),
            ("https://pki.example/swift cbpr/source", []),
            ("https://pki.example:abc/source", []),
            ("https://pki.example:99999/source", []),
            ("https://pki.example:443/source", []),
            ("https://PKI.example/source", []),
            ("https://pki.example./source", []),
            ("https://pki..example/source", []),
            ("https://-pki.example/source", []),
            ("https://pki-.example/source", []),
            ("https://pki._tcp.example/source", []),
            ("https://pki.example%2einvalid/source", []),
            ("https://123.000.000.001/source", []),
            ("https://pki.example/../source", []),
            ("https://pki.example/%2e%2e/source", []),
            ("https://pki.example/swift%2fsource", []),
            ("https://pki.example/swift%252fsource", []),
            ("https://pki.example/sources;debug/source", []),
            (r"https://pki.example/sources\source", []),
            ("https://pki.example/swift%20source", []),
            ("https://pki.example/swift%00source", []),
            ("https://pki.example/swift%7fsource", []),
            ("https://pki.example/swift%zzsource", []),
            ("https://localhost/source", []),
            ("https://127.0.0.1/source", []),
            ("http://pki.example/source?debug=true", ["--allow-insecure-http"]),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            for url, extra_args in cases:
                with self.subTest(url=url, extra_args=extra_args):
                    trust_path = write_trust_summary(root)
                    rewrite_trust_summary(
                        trust_path,
                        lambda body, url=url: body["bundles"][0]["source"].update(
                            {"url": url}
                        ),
                    )

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                        ]
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("source.url", stderr)

    def test_trust_source_retrieved_at_is_rechecked_in_archived_summary(self):
        cases = [
            ("2026-06-04T00:00:00", "timezone offset"),
            ("not-a-timestamp", "ISO 8601 timestamp"),
            ("2999-01-01T00:00:00Z", "future"),
            ("2026-06-04T00:00:00Z\nbad", "control characters"),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            for retrieved_at, message in cases:
                with self.subTest(retrieved_at=retrieved_at):
                    trust_path = write_trust_summary(root)
                    rewrite_trust_summary(
                        trust_path,
                        lambda body, retrieved_at=retrieved_at: body["bundles"][0][
                            "source"
                        ].update({"retrieved_at": retrieved_at}),
                    )

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("source.retrieved_at", stderr)
                    self.assertIn(message, stderr)

    def test_trust_summary_verified_at_is_required_valid_and_not_future(self):
        cases = [
            ("missing", None, "verified_at must be a non-empty string"),
            ("naive", "2026-06-04T00:00:00", "timezone offset"),
            ("malformed", "not-a-timestamp", "ISO 8601 timestamp"),
            ("future", "2999-01-01T00:00:00Z", "future"),
            ("control", "2026-06-04T00:00:00Z\nbad", "control characters"),
            ("whitespace", "2026-06-04T00:00:00Z ", "surrounding whitespace"),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            for name, verified_at, message in cases:
                with self.subTest(name=name):
                    trust_path = write_trust_summary(root / name)

                    def mutate(body, verified_at=verified_at):
                        if verified_at is None:
                            del body["verified_at"]
                        else:
                            body["verified_at"] = verified_at

                    rewrite_trust_summary(trust_path, mutate)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("verified_at", stderr)
                    self.assertIn(message, stderr)

    def test_canary_summary_digest_tampering_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            body = valid_canary_summary()
            body["provider"] = "changed-after-digest"
            canary_path = write_canary(root, body)
            trust_path = write_trust_summary(root)

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("summary_sha256 mismatch", stderr)

    def test_plan_only_and_dry_run_canaries_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            cases.append((plan_only_canary_summary(), "plan-only"))
            dry_run = valid_canary_summary()
            dry_run["stages"][0]["command"].append("--dry-run")
            dry_run.pop("summary_sha256")
            cases.append((digest_summary(dry_run), "--dry-run"))
            for body, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_legacy_colr007_canary_summary_requires_explicit_local_override(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            receipt_entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            body = valid_canary_summary()
            body["stages"][0]["command"].append("--allow-legacy-colr007")
            body["stages"][2]["command"].append("--allow-legacy-colr007")
            body["stages"][2]["stdout_preview"] = receipt_stdout(
                allow_legacy_colr007=True,
                receipt_entries=receipt_entries,
            )
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))
            argv = [
                "--canary-summary",
                str(canary_path),
                "--trust-summary",
                str(trust_path),
                "--receipt-dir",
                str(notary_receipts),
                "--receipt-dir",
                str(rail_receipts),
            ]

            rc, _stdout, stderr = run_evidence(argv)

            self.assertEqual(rc, 2)
            self.assertIn("--allow-legacy-colr007", stderr)

            rc, stdout, stderr = run_evidence(argv + ["--allow-legacy-colr007"])

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["policy"]["allow_legacy_colr007"])
            self.assertTrue(
                summary["canary_summaries"][0]["receipt_summary"]["allow_legacy_colr007"]
            )

    def test_insecure_http_and_default_profile_canaries_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            insecure = valid_canary_summary()
            insecure["stages"][0]["command"][5] = "http://torii.example.invalid"
            insecure["stages"][0]["command"].append("--allow-insecure-http")
            insecure.pop("summary_sha256")
            cases.append((digest_summary(insecure), "insecure HTTP"))
            default_profile = valid_canary_summary()
            default_profile["stages"][0]["command"].append("--allow-default-profile")
            default_profile.pop("summary_sha256")
            cases.append((digest_summary(default_profile), "--allow-default-profile"))
            legacy_colr = valid_canary_summary()
            legacy_colr["stages"][0]["command"].append("--allow-legacy-colr007")
            legacy_colr.pop("summary_sha256")
            cases.append((digest_summary(legacy_colr), "--allow-legacy-colr007"))
            for body, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_equals_form_local_override_flags_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            dry_run = valid_canary_summary()
            dry_run["stages"][0]["command"].append("--dry-run=true")
            dry_run.pop("summary_sha256")
            cases.append((digest_summary(dry_run), "--dry-run"))
            insecure = valid_canary_summary()
            insecure["stages"][0]["command"].append("--allow-insecure-http=true")
            insecure.pop("summary_sha256")
            cases.append((digest_summary(insecure), "insecure HTTP"))
            default_profile = valid_canary_summary()
            default_profile["stages"][0]["command"].append("--allow-default-profile=true")
            default_profile.pop("summary_sha256")
            cases.append((digest_summary(default_profile), "--allow-default-profile"))
            allow_failed = valid_canary_summary()
            allow_failed["stages"][2]["command"].append("--allow-failed=true")
            allow_failed.pop("summary_sha256")
            cases.append((digest_summary(allow_failed), "allowed failed receipts"))
            legacy_colr = valid_canary_summary()
            legacy_colr["stages"][0]["command"].append("--allow-legacy-colr007=true")
            legacy_colr.pop("summary_sha256")
            cases.append((digest_summary(legacy_colr), "--allow-legacy-colr007"))
            for body, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_unsupported_child_command_flags_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            rail_extra = valid_canary_summary()
            rail_extra["stages"][0]["command"].extend(["--summary-out", "/tmp/rail.json"])
            rail_extra.pop("summary_sha256")
            cases.append((digest_summary(rail_extra), [], "unsupported flag '--summary-out'"))
            notary_extra = valid_canary_summary()
            notary_extra["stages"][1]["command"].append("--profile=swift-cbpr-plus")
            notary_extra.pop("summary_sha256")
            cases.append((digest_summary(notary_extra), [], "unsupported flag '--profile'"))
            verify_extra = valid_canary_summary()
            verify_extra["stages"][2]["command"].append("--summary-out=/tmp/verify.json")
            verify_extra.pop("summary_sha256")
            cases.append((digest_summary(verify_extra), [], "unsupported flag '--summary-out'"))
            planned_extra = plan_only_canary_summary()
            planned_extra["planned_stages"][0]["command"].append("--summary-out=/tmp/plan.json")
            planned_extra.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(planned_extra),
                    ["--allow-plan-only"],
                    "unsupported flag '--summary-out'",
                )
            )
            for body, extra_args, message in cases:
                with self.subTest(message=message, extra_args=extra_args):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_local_notary_source_diagnostic_flag_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            executed = valid_canary_summary()
            executed["stages"][1]["command"].append("--allow-missing-record-sources")
            executed.pop("summary_sha256")
            cases.append((digest_summary(executed), []))
            planned = plan_only_canary_summary()
            planned["planned_stages"][1]["command"].append(
                "--allow-missing-record-sources"
            )
            planned.pop("summary_sha256")
            cases.append((digest_summary(planned), ["--allow-plan-only"]))
            for body, extra_args in cases:
                with self.subTest(extra_args=extra_args):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(
                        "local diagnostic flag '--allow-missing-record-sources'",
                        stderr,
                    )

    def test_canary_child_commands_reject_control_characters(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            executed = valid_canary_summary()
            executed["stages"][0]["command"][3] = "/ops/iso/inbox\nextra"
            executed.pop("summary_sha256")
            executed_whitespace = valid_canary_summary()
            executed_whitespace["stages"][0]["command"][3] = "/ops/iso/inbox "
            executed_whitespace.pop("summary_sha256")
            planned = plan_only_canary_summary()
            planned["planned_stages"][0]["command"][3] = "/ops/iso/inbox\nextra"
            planned.pop("summary_sha256")
            planned_whitespace = plan_only_canary_summary()
            planned_whitespace["planned_stages"][0]["command"][3] = " /ops/iso/inbox"
            planned_whitespace.pop("summary_sha256")
            cases = (
                (
                    digest_summary(executed),
                    [],
                    "stages[0].command[3] must not contain control characters",
                ),
                (
                    digest_summary(executed_whitespace),
                    [],
                    "stages[0].command[3] must not have surrounding whitespace",
                ),
                (
                    digest_summary(planned),
                    ["--allow-plan-only"],
                    "planned_stages[0].command[3] must not contain control characters",
                ),
                (
                    digest_summary(planned_whitespace),
                    ["--allow-plan-only"],
                    "planned_stages[0].command[3] must not have surrounding whitespace",
                ),
            )
            for body, extra_args, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_canary_receipt_dirs_reject_control_and_traversal_paths(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            executed_control = valid_canary_summary()
            executed_control["stages"][0]["receipt_dir"] = "/ops/iso/rail\nreceipts"
            executed_control.pop("summary_sha256")
            executed_parent = valid_canary_summary()
            executed_parent["stages"][1]["receipt_dir"] = "/ops/iso/../notary-receipts"
            executed_parent.pop("summary_sha256")
            planned_control = plan_only_canary_summary()
            planned_control["planned_stages"][0]["receipt_dir"] = "/ops/iso/rail\nreceipts"
            planned_control.pop("summary_sha256")
            planned_parent = plan_only_canary_summary()
            planned_parent["planned_stages"][1]["receipt_dir"] = r"..\notary-receipts"
            planned_parent.pop("summary_sha256")
            cases = (
                (
                    digest_summary(executed_control),
                    [],
                    "stages[0].receipt_dir must not contain control characters",
                ),
                (
                    digest_summary(executed_parent),
                    [],
                    "stages[1].receipt_dir must not contain dot or parent segments",
                ),
                (
                    digest_summary(planned_control),
                    ["--allow-plan-only"],
                    "planned_stages[0].receipt_dir must not contain control characters",
                ),
                (
                    digest_summary(planned_parent),
                    ["--allow-plan-only"],
                    "planned_stages[1].receipt_dir must not contain dot or parent segments",
                ),
            )
            for body, extra_args, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_canary_receipt_dirs_must_match_child_command_arguments(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            executed_mismatch = valid_canary_summary()
            executed_mismatch["stages"][0]["receipt_dir"] = "/ops/iso/other-receipts"
            executed_mismatch.pop("summary_sha256")
            executed_missing = valid_canary_summary()
            del executed_missing["stages"][0]["command"][6:8]
            executed_missing.pop("summary_sha256")
            executed_command_parent = valid_canary_summary()
            executed_command_parent["stages"][0]["command"][7] = "/ops/iso/../rail-receipts"
            executed_command_parent.pop("summary_sha256")
            planned_mismatch = plan_only_canary_summary()
            planned_mismatch["planned_stages"][1]["receipt_dir"] = "/ops/iso/other-receipts"
            planned_mismatch.pop("summary_sha256")
            planned_equals_mismatch = plan_only_canary_summary()
            command = planned_equals_mismatch["planned_stages"][0]["command"]
            del command[6:8]
            command.insert(6, "--receipt-dir=/ops/iso/other-receipts")
            planned_equals_mismatch.pop("summary_sha256")
            cases = (
                (
                    digest_summary(executed_mismatch),
                    [],
                    "stages[0].receipt_dir does not match command --receipt-dir",
                ),
                (
                    digest_summary(executed_missing),
                    [],
                    "stages[0].command must contain exactly one --receipt-dir",
                ),
                (
                    digest_summary(executed_command_parent),
                    [],
                    "stages[0].command[7] must not contain dot or parent segments",
                ),
                (
                    digest_summary(planned_mismatch),
                    ["--allow-plan-only"],
                    "planned_stages[1].receipt_dir does not match command --receipt-dir",
                ),
                (
                    digest_summary(planned_equals_mismatch),
                    ["--allow-plan-only"],
                    "planned_stages[0].receipt_dir does not match command --receipt-dir",
                ),
            )
            for body, extra_args, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_verify_stage_command_must_include_stage_receipt_dirs(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            missing_rail = valid_canary_summary()
            del missing_rail["stages"][2]["command"][2:4]
            missing_rail.pop("summary_sha256")
            missing_notary = valid_canary_summary()
            del missing_notary["stages"][2]["command"][4:6]
            missing_notary.pop("summary_sha256")
            traversal = valid_canary_summary()
            traversal["stages"][2]["command"][3] = "/ops/iso/../rail-receipts"
            traversal.pop("summary_sha256")
            cases = (
                (
                    digest_summary(missing_rail),
                    "verify command does not include rail receipt_dir",
                ),
                (
                    digest_summary(missing_notary),
                    "verify command does not include notary receipt_dir",
                ),
                (
                    digest_summary(traversal),
                    "stages[2].command[3] must not contain dot or parent segments",
                ),
            )
            for body, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_plan_only_stage_dry_run_flag_is_required(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            body = plan_only_canary_summary()
            del body["planned_stages"][0]["dry_run"]
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-plan-only",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("planned_stages[0].dry_run must be a boolean", stderr)

    def test_smuggled_canary_stage_command_urls_are_rejected(self):
        def rail_url(body, url):
            body["stages"][0]["command"][5] = url

        def notary_url(body, url):
            body["stages"][1]["command"][7] = url

        cases = [
            (rail_url, "https://user:pass@torii.example.invalid", []),
            (rail_url, "https://torii.example.invalid/iso\nbridge", []),
            (rail_url, "https://torii..example.invalid", []),
            (rail_url, "https://torii._tcp.example.invalid", []),
            (rail_url, "https://torii.example%2einvalid", []),
            (rail_url, "https://torii.example.invalid/base%20v1", []),
            (rail_url, "https://torii.example.invalid/base%00v1", []),
            (rail_url, "https://torii.example.invalid/base%252fv1", []),
            (rail_url, "https://torii.example.invalid/base%zzv1", []),
            (notary_url, "https://notary.example.invalid/anchor;debug", []),
            (notary_url, "https://notary.example.invalid/anchor?debug=true", []),
            (notary_url, "https://notary.example.invalid/anchor#fragment", []),
            (notary_url, "https://[::1", []),
            (
                rail_url,
                "http://torii.example.invalid?debug=true",
                ["--allow-insecure-http"],
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            for mutator, url, extra_args in cases:
                with self.subTest(url=url, extra_args=extra_args):
                    body = valid_canary_summary()
                    mutator(body, url)
                    if extra_args:
                        body["stages"][0]["command"].append("--allow-insecure-http")
                    body.pop("summary_sha256")
                    canary_path = write_canary(root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                        ]
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("command", stderr)

    def test_unredacted_bearer_token_path_and_secret_preview_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            unredacted = valid_canary_summary()
            token_index = unredacted["stages"][0]["command"].index("--bearer-token-file") + 1
            unredacted["stages"][0]["command"][token_index] = "/ops/secrets/torii.bearer"
            unredacted.pop("summary_sha256")
            cases.append((digest_summary(unredacted), "unredacted bearer-token file path"))
            unredacted_equals = valid_canary_summary()
            command = unredacted_equals["stages"][0]["command"]
            token_index = command.index("--bearer-token-file")
            del command[token_index : token_index + 2]
            command.insert(token_index, "--bearer-token-file=/ops/secrets/torii.bearer")
            unredacted_equals.pop("summary_sha256")
            cases.append((digest_summary(unredacted_equals), "unredacted bearer-token file path"))
            preview = valid_canary_summary()
            preview["stages"][1]["stderr_preview"] = "Authorization: Bearer live-token"
            preview.pop("summary_sha256")
            cases.append((digest_summary(preview), "secret-looking material"))
            for body, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_missing_verify_stage_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            body = valid_canary_summary()
            body["stages"] = body["stages"][:2]
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))
            trust_path = write_trust_summary(root)

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("missing required canary stages", stderr)

    def test_canary_summary_timestamps_are_required_valid_and_ordered(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            missing = valid_canary_summary()
            del missing["started_at"]
            missing.pop("summary_sha256")
            cases.append((digest_summary(missing), "started_at must be a non-empty string"))
            naive = valid_canary_summary()
            naive["started_at"] = "2026-06-04T00:00:00"
            naive.pop("summary_sha256")
            cases.append((digest_summary(naive), "started_at must include a timezone offset"))
            future = valid_canary_summary()
            future["finished_at"] = "2999-01-01T00:00:00+00:00"
            future.pop("summary_sha256")
            cases.append((digest_summary(future), "finished_at must not be in the future"))
            whitespace = valid_canary_summary()
            whitespace["started_at"] = "2026-06-04T00:00:00+00:00 "
            whitespace.pop("summary_sha256")
            cases.append((digest_summary(whitespace), "started_at must not have surrounding whitespace"))
            reversed_window = valid_canary_summary()
            reversed_window["started_at"] = "2026-06-04T00:00:02+00:00"
            reversed_window["finished_at"] = "2026-06-04T00:00:01+00:00"
            reversed_window.pop("summary_sha256")
            cases.append((digest_summary(reversed_window), "finished_at must not be before started_at"))
            for body, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_canary_and_trust_identity_strings_reject_control_characters(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            canary_cases = (
                (
                    lambda body: body.__setitem__("provider", "local\nbank"),
                    "provider must not contain control characters",
                ),
                (
                    lambda body: body["stages"][0].__setitem__("name", "rail\nx"),
                    "name must not contain control characters",
                ),
            )
            for offset, (mutate, message) in enumerate(canary_cases):
                with self.subTest(kind="canary", offset=offset):
                    body = valid_canary_summary()
                    mutate(body)
                    body.pop("summary_sha256")
                    canary_path = write_canary(root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

            canary_path = write_canary(root, valid_canary_summary())
            trust_cases = (
                (
                    lambda summary: summary["bundles"][0].__setitem__(
                        "profile_id",
                        "swift\ncbpr",
                    ),
                    "profile_id must not contain control characters",
                ),
                (
                    lambda summary: summary["bundles"][0].__setitem__(
                        "embedded_signature_policy",
                        "require-verified\n",
                    ),
                    "embedded_signature_policy must not contain control characters",
                ),
            )
            for offset, (mutate, message) in enumerate(trust_cases):
                with self.subTest(kind="trust", offset=offset):
                    mutated_trust_path = write_trust_summary(root / f"trust-{offset}")
                    rewrite_trust_summary(mutated_trust_path, mutate)

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(mutated_trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_unknown_canary_trust_and_receipt_summary_keys_are_rejected(self):
        def get_nested(value, parts):
            target = value
            for part in parts:
                target = target[part]
            return target

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            canary_cases = (
                ("summary", ()),
                ("policy", ("policy",)),
                ("stage", ("stages", 0)),
            )
            for name, target_path in canary_cases:
                with self.subTest(kind="canary", name=name):
                    body = valid_canary_summary()
                    get_nested(body, target_path)["unexpected"] = "value"
                    body.pop("summary_sha256")
                    canary_path = write_canary(root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("contains unknown keys", stderr)

            receipt_cases = (
                ("summary", ()),
                ("entry", ("receipts", 0)),
            )
            for name, target_path in receipt_cases:
                with self.subTest(kind="receipt", name=name):
                    receipt_summary = json.loads(receipt_stdout())
                    get_nested(receipt_summary, target_path)["unexpected"] = "value"
                    body = valid_canary_summary()
                    body["stages"][2]["stdout_preview"] = (
                        json.dumps(
                            digest_receipt_summary(receipt_summary),
                            sort_keys=True,
                        )
                        + "\n"
                    )
                    body.pop("summary_sha256")
                    canary_path = write_canary(root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("contains unknown keys", stderr)

            canary_path = write_canary(root, valid_canary_summary())
            trust_cases = (
                ("summary", ()),
                ("bundle", ("bundles", 0)),
                ("source", ("bundles", 0, "source")),
                ("material", ("bundles", 0, "material")),
                ("x509-trust-anchor", ("bundles", 0, "x509_trust_anchors", 0)),
                ("revoked-certificate", ("bundles", 0, "revoked_certificates", 0)),
                ("x509-crl", ("bundles", 0, "x509_crls", 0)),
                ("x509-ocsp", ("bundles", 0, "x509_ocsp_responses", 0)),
                ("profile-overrides", ("bundles", 0, "profile_overrides")),
            )
            for name, target_path in trust_cases:
                with self.subTest(kind="trust", name=name):
                    mutated_trust_path = write_trust_summary(root / f"unknown-{name}")
                    rewrite_trust_summary(
                        mutated_trust_path,
                        lambda summary, parts=target_path: get_nested(
                            summary,
                            parts,
                        ).__setitem__("unexpected", "value"),
                    )

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(mutated_trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("contains unknown keys", stderr)

    def test_json_strings_must_not_require_trimming(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            canary_cases = (
                (
                    "provider",
                    lambda body: body.__setitem__("provider", "local-bank "),
                    "provider must not have surrounding whitespace",
                ),
                (
                    "stage-name",
                    lambda body: body["stages"][0].__setitem__("name", " rail"),
                    "name must not have surrounding whitespace",
                ),
                (
                    "config-path",
                    lambda body: body.__setitem__(
                        "config_path",
                        "/ops/iso/canary.json ",
                    ),
                    "config_path must not have surrounding whitespace",
                ),
                (
                    "stage-receipt-dir",
                    lambda body: body["stages"][0].__setitem__(
                        "receipt_dir",
                        "/ops/iso/rail-receipts ",
                    ),
                    "receipt_dir must not have surrounding whitespace",
                ),
            )
            for name, mutate, message in canary_cases:
                with self.subTest(kind="canary", name=name):
                    body = valid_canary_summary()
                    mutate(body)
                    body.pop("summary_sha256")
                    canary_path = write_canary(root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

            with self.subTest(kind="canary", name="receipt-path"):
                receipt_summary = json.loads(receipt_stdout())
                receipt_summary["receipts"][0]["path"] += " "
                body = valid_canary_summary()
                body["stages"][2]["stdout_preview"] = (
                    json.dumps(
                        digest_receipt_summary(receipt_summary),
                        sort_keys=True,
                    )
                    + "\n"
                )
                body.pop("summary_sha256")
                canary_path = write_canary(root, digest_summary(body))

                rc, _stdout, stderr = run_evidence(
                    ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                )

                self.assertEqual(rc, 2)
                self.assertIn("path must not have surrounding whitespace", stderr)

            with self.subTest(kind="canary", name="receipt-kind"):
                receipt_summary = json.loads(receipt_stdout())
                receipt_summary["receipt_kind"][0] += " "
                body = valid_canary_summary()
                body["stages"][2]["stdout_preview"] = (
                    json.dumps(
                        digest_receipt_summary(receipt_summary),
                        sort_keys=True,
                    )
                    + "\n"
                )
                body.pop("summary_sha256")
                canary_path = write_canary(root, digest_summary(body))

                rc, _stdout, stderr = run_evidence(
                    ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                )

                self.assertEqual(rc, 2)
                self.assertIn("receipt_kind[0] must not have surrounding whitespace", stderr)

            canary_path = write_canary(root, valid_canary_summary())
            trust_cases = (
                (
                    "profile-id",
                    lambda summary: summary["bundles"][0].__setitem__(
                        "profile_id",
                        "swift-cbpr-plus ",
                    ),
                    "profile_id must not have surrounding whitespace",
                ),
                (
                    "source-url",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "url",
                        " https://pki.example.invalid/swift-cbpr-plus",
                    ),
                    "source.url must not have surrounding whitespace",
                ),
                (
                    "source-retrieved-at",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "retrieved_at",
                        "2026-06-04T00:00:00Z ",
                    ),
                    "source.retrieved_at must not have surrounding whitespace",
                ),
                (
                    "policy-oid",
                    lambda summary: summary["bundles"][0]["profile_overrides"][
                        "x509_required_certificate_policy_oids"
                    ].__setitem__(0, "1.3.6.1.4.1.55555.1 "),
                    "x509_required_certificate_policy_oids[0] must not have surrounding whitespace",
                ),
                (
                    "crl-base64",
                    lambda summary: summary["bundles"][0]["profile_overrides"][
                        "x509_crl_der_base64"
                    ].__setitem__(
                        0,
                        summary["bundles"][0]["profile_overrides"][
                            "x509_crl_der_base64"
                        ][0]
                        + " ",
                    ),
                    "x509_crl_der_base64[0] must not have surrounding whitespace",
                ),
            )
            for name, mutate, message in trust_cases:
                with self.subTest(kind="trust", name=name):
                    mutated_trust_path = write_trust_summary(root / f"trim-{name}")
                    rewrite_trust_summary(mutated_trust_path, mutate)

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(mutated_trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_canary_stage_timestamps_are_required_valid_and_inside_canary_window(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            missing = valid_canary_summary()
            del missing["stages"][0]["started_at"]
            missing.pop("summary_sha256")
            cases.append((digest_summary(missing), "started_at must be a non-empty string"))
            naive = valid_canary_summary()
            naive["stages"][0]["started_at"] = "2026-06-04T00:00:00"
            naive.pop("summary_sha256")
            cases.append((digest_summary(naive), "started_at must include a timezone offset"))
            future = valid_canary_summary()
            future["stages"][0]["finished_at"] = "2999-01-01T00:00:00+00:00"
            future.pop("summary_sha256")
            cases.append((digest_summary(future), "finished_at must not be in the future"))
            whitespace = valid_canary_summary()
            whitespace["stages"][0]["finished_at"] = "2026-06-04T00:00:00.200000+00:00 "
            whitespace.pop("summary_sha256")
            cases.append((digest_summary(whitespace), "finished_at must not have surrounding whitespace"))
            reversed_window = valid_canary_summary()
            reversed_window["stages"][0]["started_at"] = "2026-06-04T00:00:01+00:00"
            reversed_window["stages"][0]["finished_at"] = "2026-06-04T00:00:00+00:00"
            reversed_window.pop("summary_sha256")
            cases.append((digest_summary(reversed_window), "finished_at must not be before started_at"))
            outside_canary = valid_canary_summary()
            outside_canary["stages"][0]["started_at"] = "2026-06-03T23:59:59+00:00"
            outside_canary.pop("summary_sha256")
            cases.append((digest_summary(outside_canary), "timestamp window must be inside canary window"))
            overlapping = valid_canary_summary()
            overlapping["stages"][1]["started_at"] = "2026-06-04T00:00:00.100000+00:00"
            overlapping.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(overlapping),
                    "started_at must not be before previous stage finished_at",
                )
            )
            for body, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_canary_summary_must_prove_explicit_runbook_policy(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            missing_policy = valid_canary_summary()
            del missing_policy["policy"]
            missing_policy.pop("summary_sha256")
            cases.append((digest_summary(missing_policy), "policy must be a JSON object"))
            missing_flag = valid_canary_summary()
            del missing_flag["policy"]["require_explicit_policy"]
            missing_flag.pop("summary_sha256")
            cases.append((digest_summary(missing_flag), "require_explicit_policy must be a boolean"))
            false_flag = valid_canary_summary()
            false_flag["policy"]["require_explicit_policy"] = False
            false_flag.pop("summary_sha256")
            cases.append((digest_summary(false_flag), "--require-explicit-policy"))
            for body, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_failed_skipped_truncated_and_weak_verify_stages_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            failed = valid_canary_summary()
            failed["stages"][0]["returncode"] = 1
            failed.pop("summary_sha256")
            cases.append((digest_summary(failed), "failed with returncode 1"))
            skipped = valid_canary_summary()
            skipped["stages"][1]["skipped"] = True
            skipped.pop("summary_sha256")
            cases.append((digest_summary(skipped), "was skipped"))
            weak_verify = valid_canary_summary()
            weak_verify["stages"][2]["command"].remove("--require-source-files")
            weak_verify.pop("summary_sha256")
            cases.append((digest_summary(weak_verify), "did not require receipt source files"))
            truncated = valid_canary_summary()
            truncated["stages"][2]["stdout_truncated"] = True
            truncated.pop("summary_sha256")
            cases.append((digest_summary(truncated), "stdout_preview is truncated"))
            for body, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_empty_and_incomplete_receipt_verifier_stdout_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            missing_digest = valid_canary_summary()
            missing_digest["stages"][2]["stdout_preview"] = "{}\n"
            missing_digest.pop("summary_sha256")
            cases.append((digest_summary(missing_digest), "summary_sha256"))
            empty = valid_canary_summary()
            empty["stages"][2]["stdout_preview"] = receipt_stdout(verified_receipts=0)
            empty.pop("summary_sha256")
            cases.append((digest_summary(empty), "verified_receipts must be positive"))
            missing_kind = valid_canary_summary()
            missing_kind["stages"][2]["stdout_preview"] = receipt_stdout(["iso-rail-gateway"])
            missing_kind.pop("summary_sha256")
            cases.append((digest_summary(missing_kind), "missing receipt kinds"))
            allow_failed = valid_canary_summary()
            allow_failed["stages"][2]["stdout_preview"] = receipt_stdout(allow_failed=True)
            allow_failed.pop("summary_sha256")
            cases.append((digest_summary(allow_failed), "allowed failed receipts"))
            allow_legacy = valid_canary_summary()
            allow_legacy["stages"][2]["stdout_preview"] = receipt_stdout(
                allow_legacy_colr007=True
            )
            allow_legacy.pop("summary_sha256")
            cases.append((digest_summary(allow_legacy), "allowed legacy colr.007 receipts"))
            for body, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_receipt_verifier_stdout_duplicate_receipts_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            duplicate_path = json.loads(receipt_stdout())
            duplicate_path["receipts"][1]["path"] = duplicate_path["receipts"][0]["path"]
            cases.append((duplicate_path, "path duplicates"))
            duplicate_digest = json.loads(receipt_stdout())
            duplicate_digest["receipts"][1]["receipt_sha256"] = duplicate_digest[
                "receipts"
            ][0]["receipt_sha256"]
            cases.append((duplicate_digest, "receipt_sha256 duplicates"))

            for receipt_summary, message in cases:
                with self.subTest(message=message):
                    body = valid_canary_summary()
                    body["stages"][2]["stdout_preview"] = (
                        json.dumps(
                            digest_receipt_summary(receipt_summary),
                            sort_keys=True,
                        )
                        + "\n"
                    )
                    body.pop("summary_sha256")
                    canary_path = write_canary(root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_receipt_verifier_stdout_receipt_paths_are_canonical(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = (
                (
                    "/ops/iso/receipts/rail\n0.receipt.json",
                    "must not contain control characters",
                ),
                (
                    "/ops/iso/receipts/../rail.receipt.json",
                    "must not contain dot or parent segments",
                ),
                (
                    r"..\rail.receipt.json",
                    "must not contain dot or parent segments",
                ),
                (
                    "/ops/iso/receipts/rail.json",
                    "must point to a .receipt.json file",
                ),
            )
            for receipt_path, message in cases:
                with self.subTest(receipt_path=receipt_path):
                    receipt_summary = json.loads(receipt_stdout())
                    receipt_summary["receipts"][0]["path"] = receipt_path
                    body = valid_canary_summary()
                    body["stages"][2]["stdout_preview"] = (
                        json.dumps(
                            digest_receipt_summary(receipt_summary),
                            sort_keys=True,
                        )
                        + "\n"
                    )
                    body.pop("summary_sha256")
                    canary_path = write_canary(root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_receipt_verifier_stdout_policy_flags_are_required(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            for flag in (
                "allow_failed",
                "allow_insecure_http",
                "allow_legacy_colr007",
                "require_source_files",
            ):
                with self.subTest(flag=flag):
                    receipt_summary = json.loads(receipt_stdout())
                    del receipt_summary[flag]
                    stdout = json.dumps(
                        digest_receipt_summary(receipt_summary),
                        sort_keys=True,
                    ) + "\n"
                    body = valid_canary_summary()
                    body["stages"][2]["stdout_preview"] = stdout
                    body.pop("summary_sha256")
                    canary_path = write_canary(root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(f"stdout_preview.{flag} must be a boolean", stderr)

    def test_expected_provider_environment_and_trust_digest_are_enforced(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root)

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--provider",
                    "different-provider",
                ]
            )
            self.assertEqual(rc, 2)
            self.assertIn("expected 'different-provider'", stderr)

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--environment",
                    "prod",
                ]
            )
            self.assertEqual(rc, 2)
            self.assertIn("expected 'prod'", stderr)

            trust = json.loads(trust_path.read_text(encoding="utf-8"))
            trust["verified_bundles"] = 2
            tampered_trust_path = write_json(root / "tampered-trust.summary.json", trust)
            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(tampered_trust_path)]
            )
            self.assertEqual(rc, 2)
            self.assertIn("summary_sha256 mismatch", stderr)

    def test_synthetic_record_only_and_insecure_trust_summaries_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            cases = [
                (write_trust_summary(root / "synthetic", synthetic=True), "--allow-synthetic-der"),
                (write_trust_summary(root / "record", record_only=True), "--allow-record-only"),
                (write_trust_summary(root / "insecure", insecure_source=True), "--allow-insecure-source-url"),
            ]
            missing_source_path = write_trust_summary(root / "missing-source")
            rewrite_trust_summary(
                missing_source_path,
                lambda summary: summary["bundles"][0].pop("source"),
            )
            cases.append((missing_source_path, "source is required"))
            for trust_path, message in cases:
                with self.subTest(message=message):
                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_trust_summary_must_emit_profile_json_by_default(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            canary_path = write_canary(
                root,
                valid_canary_summary(
                    receipt_entries=receipt_entries_from_dirs(notary_receipts, rail_receipts)
                ),
            )
            trust_path = write_trust_summary(
                root / "trust",
                emit_profile_json=False,
            )

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("did not emit profile JSON", stderr)

            summary_out = root / "evidence.summary.json"
            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-profile-json-not-emitted",
                    "--receipt-dir",
                    str(notary_receipts),
                    "--receipt-dir",
                    str(rail_receipts),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["policy"]["allow_profile_json_not_emitted"])
            self.assertFalse(summary["trust_summaries"][0]["profile_json_emitted"])
            self.assertTrue(summary["trust_summaries"][0]["profile_json_emittable"])
            self.assertIsNone(summary["trust_summaries"][0]["profile_json_sha256"])
            self.assertEqual(json.loads(summary_out.read_text(encoding="utf-8")), summary)

    def test_trust_summary_profile_json_digest_is_required_when_emitted(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            for name, mutate, message in (
                (
                    "missing",
                    lambda summary: summary.pop("profile_json_sha256"),
                    "profile_json_sha256 must be a lowercase SHA-256 digest",
                ),
                (
                    "uppercase",
                    lambda summary: summary.update({"profile_json_sha256": "A" * 64}),
                    "profile_json_sha256 must be a lowercase SHA-256 digest",
                ),
                (
                    "mismatch",
                    lambda summary: summary.update({"profile_json_sha256": "0" * 64}),
                    "profile_json_sha256 does not match archived profile_overrides",
                ),
            ):
                with self.subTest(name=name):
                    trust = json.loads(trust_path.read_text(encoding="utf-8"))
                    mutate(trust)
                    mutated_path = root / f"{name}-profile-digest.trust.summary.json"
                    write_json(mutated_path, digest_summary(trust))

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_trust_summary_profile_json_digest_must_be_null_when_not_emitted(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(
                root / "trust",
                emit_profile_json=False,
            )
            rewrite_trust_summary(
                trust_path,
                lambda summary: summary.update({"profile_json_sha256": "0" * 64}),
            )

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-profile-json-not-emitted",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("profile_json_sha256 must be null", stderr)

    def test_trust_summary_policy_flags_are_required(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            for flag in (
                "allow_synthetic_der",
                "allow_record_only",
                "allow_insecure_source_url",
                "profile_json_emitted",
                "profile_json_emittable",
            ):
                with self.subTest(flag=flag):
                    trust = json.loads(trust_path.read_text(encoding="utf-8"))
                    del trust[flag]
                    mutated_path = write_json(
                        root / f"missing-{flag}.trust.summary.json",
                        digest_summary(trust),
                    )

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(f"{flag} must be a boolean", stderr)

    def test_trust_bundle_digest_is_required_unique_and_canonical(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            cases = (
                (
                    "missing",
                    lambda trust: trust["bundles"][0].pop("bundle_sha256"),
                    "bundle_sha256 must be a canonical SHA-256",
                ),
                (
                    "short",
                    lambda trust: trust["bundles"][0].__setitem__("bundle_sha256", "0" * 63),
                    "bundle_sha256 must be a canonical SHA-256",
                ),
                (
                    "duplicate",
                    lambda trust: (
                        trust["bundles"].append(
                            {
                                **trust["bundles"][0],
                                "profile_id": "fedwire-funds",
                                "rail": "fedwire-funds",
                                "profile_overrides": {
                                    **trust["bundles"][0]["profile_overrides"],
                                    "id": "fedwire-funds",
                                    "rail": "fedwire-funds",
                                },
                            }
                        ),
                        trust.__setitem__("verified_bundles", 2),
                    ),
                    "bundle_sha256 duplicates",
                ),
            )
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    trust_path = write_trust_summary(root / f"trust-{name}")
                    rewrite_trust_summary(
                        trust_path,
                        lambda trust: (mutate(trust), refresh_profile_json_sha256(trust)),
                    )

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_trust_profile_overrides_must_match_material_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            cases = (
                (
                    "id",
                    lambda override: override.__setitem__("id", "fedwire-funds"),
                    "profile_overrides.id does not match profile_id",
                ),
                (
                    "rail",
                    lambda override: override.__setitem__("rail", "fedwire-funds"),
                    "profile_overrides.rail does not match rail",
                ),
                (
                    "policy",
                    lambda override: override.__setitem__(
                        "embedded_signature_policy",
                        "record-only",
                    ),
                    "profile_overrides.embedded_signature_policy does not match",
                ),
                (
                    "public-pins",
                    lambda override: override["signature_public_key_sha256_pins"].append(
                        "1" * 64
                    ),
                    "public-key pin count does not match material",
                ),
                (
                    "anchor-pins",
                    lambda override: override.__setitem__(
                        "x509_trust_anchor_sha256_pins",
                        [],
                    ),
                    "X.509 trust-anchor pin count does not match material",
                ),
                (
                    "revoked-pins",
                    lambda override: override.__setitem__("revoked_certificate_sha256", []),
                    "revoked-certificate pin count does not match material",
                ),
                (
                    "policy-oids",
                    lambda override: override.__setitem__(
                        "x509_required_certificate_policy_oids",
                        [],
                    ),
                    "certificate-policy OID count does not match material",
                ),
                (
                    "crls",
                    lambda override: override.__setitem__("x509_crl_der_base64", []),
                    "CRL DER count does not match material",
                ),
                (
                    "ocsp",
                    lambda override: override.__setitem__(
                        "x509_ocsp_response_der_base64",
                        [],
                    ),
                    "OCSP DER count does not match material",
                ),
            )
            for name, mutate_override, message in cases:
                with self.subTest(name=name):
                    trust_path = write_trust_summary(root / f"trust-{name}")

                    def mutate(summary, mutate_override=mutate_override):
                        mutate_override(summary["bundles"][0]["profile_overrides"])

                    rewrite_trust_summary(trust_path, mutate)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_trust_profile_override_values_must_remain_canonical(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            cases = (
                (
                    "policy-oid",
                    lambda summary: summary["bundles"][0]["profile_overrides"][
                        "x509_required_certificate_policy_oids"
                    ].__setitem__(0, "01.2"),
                    "must be a dotted numeric OID",
                ),
                (
                    "crl-base64",
                    lambda summary: summary["bundles"][0]["profile_overrides"][
                        "x509_crl_der_base64"
                    ].__setitem__(0, "not-base64"),
                    "must be canonical base64",
                ),
                (
                    "ocsp-base64",
                    lambda summary: summary["bundles"][0]["profile_overrides"][
                        "x509_ocsp_response_der_base64"
                    ].__setitem__(0, "not-base64"),
                    "must be canonical base64",
                ),
                (
                    "crl-der-not-sequence",
                    lambda summary: summary["bundles"][0]["profile_overrides"][
                        "x509_crl_der_base64"
                    ].__setitem__(0, base64.b64encode(b"\x04\x01x").decode("ascii")),
                    "must be a DER SEQUENCE",
                ),
                (
                    "ocsp-der-truncated",
                    lambda summary: summary["bundles"][0]["profile_overrides"][
                        "x509_ocsp_response_der_base64"
                    ].__setitem__(0, base64.b64encode(b"\x30\x03\x01").decode("ascii")),
                    "DER length does not consume the whole value",
                ),
                (
                    "crl-base64-oversized",
                    lambda summary: summary["bundles"][0]["profile_overrides"][
                        "x509_crl_der_base64"
                    ].__setitem__(
                        0,
                        base64.b64encode(
                            b"x" * (EVIDENCE.MAX_TRUST_DER_BYTES + 1)
                        ).decode("ascii"),
                    ),
                    "must decode to no more than",
                ),
                (
                    "crl-der-digest-drift",
                    lambda summary: summary["bundles"][0]["profile_overrides"][
                        "x509_crl_der_base64"
                    ].__setitem__(0, base64.b64encode(b"\x30\x00").decode("ascii")),
                    "not recorded in",
                ),
                (
                    "ocsp-der-digest-drift",
                    lambda summary: summary["bundles"][0]["profile_overrides"][
                        "x509_ocsp_response_der_base64"
                    ].__setitem__(0, base64.b64encode(b"\x30\x00").decode("ascii")),
                    "not recorded in",
                ),
                (
                    "crl-summary-digest-drift",
                    lambda summary: summary["bundles"][0]["x509_crls"][0].__setitem__(
                        "sha256",
                        "3" * 64,
                    ),
                    "not recorded in",
                ),
                (
                    "ocsp-summary-byte-len-drift",
                    lambda summary: summary["bundles"][0]["x509_ocsp_responses"][
                        0
                    ].__setitem__(
                        "byte_len",
                        summary["bundles"][0]["x509_ocsp_responses"][0]["byte_len"]
                        + 1,
                    ),
                    "byte_len does not match",
                ),
                (
                    "public-overlap",
                    lambda summary: (
                        summary["bundles"][0]["profile_overrides"].__setitem__(
                            "signature_public_key_sha256_pins",
                            ["1" * 64],
                        ),
                        summary["bundles"][0]["profile_overrides"].__setitem__(
                            "trusted_public_key_sha256",
                            ["1" * 64],
                        ),
                        summary["bundles"][0]["material"].__setitem__(
                            "signature_public_key_pin_count",
                            2,
                        ),
                    ),
                    "signature_public_key_sha256_pins/trusted_public_key_sha256",
                ),
                (
                    "revoked-overlap",
                    lambda summary: summary["bundles"][0]["profile_overrides"].__setitem__(
                        "revoked_certificate_sha256",
                        [
                            summary["bundles"][0]["profile_overrides"][
                                "x509_trust_anchor_sha256_pins"
                            ][0]
                        ],
                    ),
                    "trusted/revoked certificate pins",
                ),
            )
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    trust_path = write_trust_summary(root / f"trust-{name}")
                    rewrite_trust_summary(trust_path, mutate)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_trust_profile_revocation_flags_are_required(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            for flag in (
                "x509_require_crl_revocation_check",
                "x509_require_ocsp_revocation_check",
            ):
                with self.subTest(flag=flag):
                    trust = json.loads(trust_path.read_text(encoding="utf-8"))
                    del trust["bundles"][0]["profile_overrides"][flag]
                    mutated_path = write_json(
                        root / f"missing-{flag}.trust.summary.json",
                        digest_summary(trust),
                    )

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(f"profile_overrides.{flag} must be a boolean", stderr)

    def test_trust_profile_revocation_material_counts_are_required(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            cases = (
                ("x509_crl_count", "x509_crl_count must be a non-negative integer"),
                (
                    "x509_ocsp_response_count",
                    "x509_ocsp_response_count must be a non-negative integer",
                ),
            )
            for flag, message in cases:
                with self.subTest(flag=flag):
                    trust = json.loads(trust_path.read_text(encoding="utf-8"))
                    del trust["bundles"][0]["material"][flag]
                    mutated_path = write_json(
                        root / f"missing-{flag}.trust.summary.json",
                        digest_summary(trust),
                    )

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_trust_profile_required_revocation_material_must_be_positive(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            cases = (
                ("x509_crl_count", "requires CRL revocation checking but has no CRLs"),
                (
                    "x509_ocsp_response_count",
                    "requires OCSP revocation checking but has no OCSP responses",
                ),
            )
            for flag, message in cases:
                with self.subTest(flag=flag):
                    trust = json.loads(trust_path.read_text(encoding="utf-8"))
                    trust["bundles"][0]["material"][flag] = 0
                    mutated_path = write_json(
                        root / f"zero-{flag}.trust.summary.json",
                        digest_summary(trust),
                    )

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)


if __name__ == "__main__":
    unittest.main()
