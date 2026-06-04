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
):
    kinds = receipt_kind or ["iso-audit-notary", "iso-rail-gateway"]
    receipts = [
        {
            "path": f"/ops/iso/receipts/{kind}.{offset}.receipt.json",
            "receipt_kind": kind,
            "receipt_sha256": f"{offset + 1:064x}",
        }
        for offset, kind in enumerate(kinds[: max(verified_receipts, 0)])
    ]
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


def stage(name, command, receipt_dir=None, stdout=None):
    return {
        "name": name,
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


def valid_canary_summary():
    return digest_summary(
        {
            "provider": "local-bank",
            "environment": "preprod",
            "config_path": "/ops/iso/canary.json",
            "started_at": "2026-06-04T00:00:00+00:00",
            "finished_at": "2026-06-04T00:00:01+00:00",
            "ok": True,
            "plan_only": False,
            "stages": [
                stage("rail", rail_command(), "/ops/iso/rail-receipts"),
                stage("notary", notary_command(), "/ops/iso/notary-receipts"),
                stage("verify", verify_command(), stdout=receipt_stdout()),
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


def write_https_receipt_dirs(root, *, legacy_colr007=False):
    export_dir = root / "audit-export"
    export_dir.mkdir()
    audit_test.write_export(export_dir)
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
    receipt_test.rewrite_receipt(
        notary_receipt,
        lambda body: body.update({"endpoint": "https://notary.example.invalid/iso-anchor"}),
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
    receipt_test.rewrite_receipt(
        rail_receipt,
        lambda body: body.update({"endpoint_url": "https://torii.example.invalid"}),
    )

    return export_dir / "receipts", inbox / "receipts"


def run_evidence(argv):
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        rc = EVIDENCE.main(argv)
    return rc, stdout.getvalue(), stderr.getvalue()


class IsoOperatorEvidenceVerifyTest(unittest.TestCase):
    def test_valid_canary_and_trust_summaries_pass(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
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
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["ok"])
            self.assertEqual(summary["version"], 1)
            self.assertEqual(summary["canary_summaries"][0]["stage_names"], ["rail", "notary", "verify"])
            self.assertEqual(
                summary["canary_summaries"][0]["receipt_summary"]["receipt_kind"],
                ["iso-audit-notary", "iso-rail-gateway"],
            )
            self.assertEqual(
                len(summary["canary_summaries"][0]["receipt_summary"]["receipts"]),
                2,
            )
            self.assertIn(
                "summary_sha256",
                summary["canary_summaries"][0]["receipt_summary"],
            )
            self.assertEqual(summary["trust_summaries"][0]["verified_bundles"], 1)
            self.assertEqual(json.loads(summary_out.read_text(encoding="utf-8")), summary)
            body = dict(summary)
            digest = body.pop("summary_sha256")
            self.assertEqual(digest, EVIDENCE.sha256_hex(EVIDENCE._canonical_json_bytes(body)))

    def test_direct_receipt_archive_verification_is_preserved(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            canary_path = write_canary(root)
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

    def test_legacy_colr007_archive_receipts_require_explicit_local_override(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(
                root,
                legacy_colr007=True,
            )
            canary_path = write_canary(root)
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
            body = valid_canary_summary()
            body["stages"][0]["command"].append("--allow-legacy-colr007")
            body["stages"][2]["command"].append("--allow-legacy-colr007")
            body["stages"][2]["stdout_preview"] = receipt_stdout(
                allow_legacy_colr007=True
            )
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))
            argv = ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]

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
                (write_trust_summary(root / "missing-source", missing_source=True), "source is required"),
            ]
            for trust_path, message in cases:
                with self.subTest(message=message):
                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_trust_summary_policy_flags_are_required(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            for flag in (
                "allow_synthetic_der",
                "allow_record_only",
                "allow_insecure_source_url",
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


if __name__ == "__main__":
    unittest.main()
