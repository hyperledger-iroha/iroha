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
                "ok": True,
                "status_code": 202,
                **(
                    {
                        "anchor_sha256": f"{offset + 101:064x}",
                        "index_sha256": f"{offset + 201:064x}",
                        "record_count": 1,
                    }
                    if kind == "iso-audit-notary"
                    else {
                        "message_type": "pacs.002",
                        "payload_sha256": f"{offset + 301:064x}",
                        "profile": "swift-cbpr-plus",
                    }
                ),
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
        "timed_out": False,
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
    elif not insecure_source:
        bundle["source"].update(
            {
                "authority": "Local Bank Rail PKI",
                "version": "2026-Q2",
                "url": "https://pki.local-bank.example.com/swift-cbpr-plus",
            }
        )
    bundle_path = trust_test.write_bundle(root, bundle)
    summary_path = root / "trust.summary.json"
    profile_path = root / "trust.profile.json"
    if not (synthetic or record_only or insecure_source or missing_source):
        argv.extend(["--max-source-age-days", "36500"])
    if emit_profile_json and not (synthetic or record_only or insecure_source or missing_source):
        argv.extend(
            [
                "--emit-profile-json",
                str(profile_path),
            ]
        )
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
            entry = {
                "path": f"/ops/iso/receipts/{path.name}",
                "receipt_kind": receipt["receipt_kind"],
                "receipt_sha256": receipt["receipt_sha256"],
                "ok": receipt["ok"],
                "status_code": receipt["status_code"],
            }
            if receipt["receipt_kind"] == "iso-audit-notary":
                entry.update(
                    {
                        "anchor_sha256": receipt["anchor_sha256"],
                        "index_sha256": receipt["index_sha256"],
                        "record_count": receipt["record_count"],
                    }
                )
            elif receipt["receipt_kind"] == "iso-rail-gateway":
                entry.update(
                    {
                        "message_type": receipt["message_type"],
                        "payload_sha256": receipt["payload_sha256"],
                        "profile": receipt["profile"],
                    }
                )
            entries.append(entry)
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
            summary_out.write_text('{"stale": true}\n' + ("x" * 4096), encoding="utf-8")

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
            self.assertEqual(summary["trust_summaries"][0]["max_source_age_days"], 36500)
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
                    "authority": "Local Bank Rail PKI",
                    "version": "2026-Q2",
                    "url": "https://pki.local-bank.example.com/swift-cbpr-plus",
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
            self.assertEqual(summary_out.stat().st_mode & 0o077, 0)
            self.assertEqual(
                list(summary_out.parent.glob(".iso-*.tmp")),
                [],
            )
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

    def test_summary_output_path_rejects_smuggled_segments(self):
        cases = (
            ("semicolon", "evidence;debug.summary.json", "semicolon path"),
            ("whitespace", "evidence summary.json", "whitespace"),
            ("leading-dash", "nested/-evidence.summary.json", "leading-dash"),
            ("parent", "nested/../evidence.summary.json", "dot or parent"),
            (
                "dot",
                lambda root: f"{root}/nested/./evidence.summary.json",
                "dot or parent",
            ),
            ("empty", lambda root: f"{root}//evidence.summary.json", "empty path"),
        )
        for name, summary_arg, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    notary_receipts, rail_receipts = write_https_receipt_dirs(root)
                    canary_path = write_canary(
                        root,
                        valid_canary_summary(
                            receipt_entries=receipt_entries_from_dirs(
                                notary_receipts, rail_receipts
                            )
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
                            "--summary-out",
                            summary_arg(root)
                            if callable(summary_arg)
                            else str(root / summary_arg),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_hardlinked_summary_output_is_rejected(self):
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
            summary_out = root / "evidence-hardlink.summary.json"
            try:
                summary_out.hardlink_to(target)
            except OSError as error:
                self.skipTest(f"hard link creation unavailable: {error}")

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
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be hard-linked", stderr)
            self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")

    def test_symlinked_summary_output_ancestor_is_rejected(self):
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
            target_dir = root / "evidence-target"
            target_dir.mkdir()
            ancestor = root / "evidence-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            summary_out = ancestor / "nested" / "evidence.summary.json"

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
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)
            self.assertFalse((target_dir / "nested").exists())

    def test_cli_input_paths_reject_raw_smuggling_before_read(self):
        cases = (
            ("canary semicolon", "--canary-summary", "canary;debug.summary.json", "semicolon path"),
            ("trust whitespace", "--trust-summary", "trust summary.json", "whitespace"),
            ("receipt leading-dash", "--receipt", "nested/-receipt.json", "leading-dash"),
            ("receipt-dir parent", "--receipt-dir", "nested/../receipts", "dot or parent"),
            (
                "canary dot",
                "--canary-summary",
                lambda root: f"{root}/nested/./canary.summary.json",
                "dot or parent",
            ),
            (
                "trust empty",
                "--trust-summary",
                lambda root: f"{root}//trust.summary.json",
                "empty path",
            ),
        )
        for name, flag, raw_path, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    value = raw_path(root) if callable(raw_path) else str(root / raw_path)

                    rc, stdout, stderr = run_evidence([flag, value])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

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

    def test_symlinked_summary_input_ancestors_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            canary_target_dir = root / "canary-target"
            canary_target_dir.mkdir()
            canary_target = write_canary(canary_target_dir)
            canary_ancestor = root / "canary-ancestor-link"
            try:
                canary_ancestor.symlink_to(canary_target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            canary_path = canary_ancestor / canary_target.name

            rc, stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)

            canary_dir = root / "canary"
            canary_dir.mkdir()
            canary_path = write_canary(canary_dir)
            trust_target_dir = root / "trust-target"
            trust_target = write_trust_summary(trust_target_dir)
            trust_ancestor = root / "trust-ancestor-link"
            try:
                trust_ancestor.symlink_to(trust_target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            trust_path = trust_ancestor / trust_target.name

            rc, stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
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

    def test_oversized_summary_inputs_are_rejected_before_validation(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            canary_path = root / "oversized-canary.summary.json"
            canary_path.write_text(
                '{"padding":"' + ("a" * EVIDENCE.MAX_SUMMARY_JSON_BYTES) + '"}',
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("exceeds", stderr)

            canary_root = root / "canary"
            canary_root.mkdir()
            valid_canary_path = write_canary(canary_root)
            oversized_trust_path = root / "oversized-trust.summary.json"
            oversized_trust_path.write_text(
                '{"padding":"' + ("a" * EVIDENCE.MAX_SUMMARY_JSON_BYTES) + '"}',
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(valid_canary_path),
                    "--trust-summary",
                    str(oversized_trust_path),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("exceeds", stderr)

    def test_canary_config_path_is_canonical(self):
        cases = (
            ("/ops/iso/canary\n.json", "config_path must not contain control characters"),
            ("/ops/iso/can ary.json", "config_path must not contain whitespace"),
            ("--canary.json", "config_path must not start with a dash"),
            (
                "/ops/iso/--canary.json",
                "config_path must not contain leading-dash path segments",
            ),
            ("/ops/iso/canary.json;v=1", "config_path must not contain semicolon path parameters"),
            ("/ops/iso//canary.json", "config_path must not contain empty path segments"),
            ("/ops/iso/../canary.json", "config_path must not contain dot or parent segments"),
            (r"..\canary.json", "config_path must not contain dot or parent segments"),
            (r"/ops\iso/canary.json", "config_path must use forward slashes"),
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
                lambda body: (
                    body.__setitem__("max_source_age_days", 1),
                    body["bundles"][0]["source"].update({"retrieved_at": old_start}),
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

    def test_non_finite_canary_summary_json_numbers_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = root / "canary.summary.json"
            canary_path.write_text('{"provider":NaN}\n', encoding="utf-8")
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("non-finite numeric constant NaN", stderr)

    def test_canary_summary_json_surrogate_strings_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = root / "canary.summary.json"
            canary_path.write_text('{"provider":"\\ud800"}\n', encoding="utf-8")
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("invalid Unicode surrogate", stderr)

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

    def test_non_finite_receipt_stdout_json_numbers_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            body = valid_canary_summary()
            body["stages"][2]["stdout_preview"] = '{"verified_receipts":NaN}\n'
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("non-finite numeric constant NaN", stderr)

    def test_receipt_stdout_json_surrogate_strings_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            body = valid_canary_summary()
            body["stages"][2]["stdout_preview"] = '{"verified_receipts":"\\ud800"}\n'
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("invalid Unicode surrogate", stderr)

    def test_duplicate_direct_receipt_verifier_stdout_json_keys_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")

            original_run = EVIDENCE._run_command_bounded
            EVIDENCE._run_command_bounded = lambda *_args, **_kwargs: (
                0,
                '{"verified_receipts":2,"verified_receipts":3}\n',
                False,
                "",
                False,
                False,
            )
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
                EVIDENCE._run_command_bounded = original_run

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)

    def test_non_finite_direct_receipt_verifier_stdout_json_numbers_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")

            original_run = EVIDENCE._run_command_bounded
            EVIDENCE._run_command_bounded = lambda *_args, **_kwargs: (
                0,
                '{"verified_receipts":Infinity}\n',
                False,
                "",
                False,
                False,
            )
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
                EVIDENCE._run_command_bounded = original_run

            self.assertEqual(rc, 2)
            self.assertIn("non-finite numeric constant Infinity", stderr)

    def test_direct_receipt_verifier_stdout_json_surrogate_strings_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")

            original_run = EVIDENCE._run_command_bounded
            EVIDENCE._run_command_bounded = lambda *_args, **_kwargs: (
                0,
                '{"verified_receipts":"\\ud800"}\n',
                False,
                "",
                False,
                False,
            )
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
                EVIDENCE._run_command_bounded = original_run

            self.assertEqual(rc, 2)
            self.assertIn("invalid Unicode surrogate", stderr)

    def test_direct_receipt_verifier_output_truncation_is_rejected(self):
        cases = [
            (
                "stdout",
                (0, '{"verified_receipts":2}', True, "", False, False),
                "receipt verifier stdout exceeded",
            ),
            (
                "stderr",
                (0, receipt_stdout(), False, "warning" * 10, True, False),
                "receipt verifier stderr exceeded",
            ),
        ]
        for name, result, expected in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_root:
                root = Path(raw_root)
                canary_path = write_canary(root)
                trust_path = write_trust_summary(root / "trust")
                original_run = EVIDENCE._run_command_bounded
                EVIDENCE._run_command_bounded = lambda *_args, result=result, **_kwargs: result
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
                    EVIDENCE._run_command_bounded = original_run

                self.assertEqual(rc, 2)
                self.assertIn(expected, stderr)

    def test_direct_receipt_verifier_runtime_timeout_is_bounded(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            script_dir = root / "scripts"
            script_dir.mkdir()
            fake_receipt_verify = script_dir / "iso_operator_receipt_verify.py"
            fake_receipt_verify.write_text(
                "\n".join(
                    [
                        "import sys",
                        "import time",
                        "sys.stdout.write('started')",
                        "sys.stdout.flush()",
                        "time.sleep(5)",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            original_script_dir = EVIDENCE.SCRIPT_DIR
            EVIDENCE.SCRIPT_DIR = script_dir
            try:
                rc, stdout, stderr = run_evidence(
                    [
                        "--canary-summary",
                        str(canary_path),
                        "--trust-summary",
                        str(trust_path),
                        "--receipt-dir",
                        str(root / "receipts"),
                        "--receipt-verifier-timeout-secs",
                        "1",
                    ]
                )
            finally:
                EVIDENCE.SCRIPT_DIR = original_script_dir

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("receipt verifier timed out after 1 seconds", stderr)

    def test_receipt_verifier_timeout_cli_rejects_nonpositive_and_nonfinite_values(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            for value in ("0", "-1", "nan", "inf"):
                with self.subTest(value=value):
                    rc, stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                            "--allow-canary-stage-receipts-only",
                            "--receipt-verifier-timeout-secs",
                            value,
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("positive finite number", stderr)

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

    def test_trust_profiles_cannot_be_reused_across_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_one_path = write_trust_summary(root / "trust-one")
            trust_two_path = write_trust_summary(root / "trust-two")
            trust_two = json.loads(trust_two_path.read_text(encoding="utf-8"))
            trust_two["verified_at"] = "2026-06-04T00:00:01Z"
            write_json(trust_two_path, digest_summary(trust_two))

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_one_path),
                    "--trust-summary",
                    str(trust_two_path),
                    "--allow-canary-stage-receipts-only",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("trust_summaries[1].profiles[0].profile_id duplicates", stderr)

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
            notary_metadata_keys = {"anchor_sha256", "index_sha256", "record_count"}
            rail_metadata_keys = {"message_type", "payload_sha256", "profile"}
            notary_metadata = {key: entries[0][key] for key in notary_metadata_keys}
            rail_metadata = {key: entries[1][key] for key in rail_metadata_keys}
            for entry in entries:
                for key in notary_metadata_keys | rail_metadata_keys:
                    entry.pop(key, None)
            entries[0]["receipt_kind"] = "iso-rail-gateway"
            entries[0].update(rail_metadata)
            entries[1]["receipt_kind"] = "iso-audit-notary"
            entries[1].update(notary_metadata)
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

    def test_direct_receipt_archive_must_bind_canary_receipt_metadata(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            entries[0]["status_code"] = 200
            entries[0]["record_count"] = 0
            entries[1]["profile"] = "sepa-sct-inst"
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
            self.assertIn("metadata", stderr)

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
            self.assertIn("legacy rail message type", stderr)

            rc, stdout, stderr = run_evidence(argv + ["--allow-legacy-colr007"])

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["policy"]["allow_legacy_colr007"])
            self.assertTrue(summary["receipt_verification"]["allow_legacy_colr007"])

    def test_smuggled_trust_source_urls_are_rejected(self):
        long_host = ".".join(["a" * 63] * 4)
        long_url = "https://pki.example/" + ("a" * EVIDENCE.MAX_HTTP_URL_CHARS)
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
            (long_url, []),
            ("https://PKI.example/source", []),
            ("https://pki.example./source", []),
            ("https://pki..example/source", []),
            (f"https://{long_host}/source", []),
            ("https://-pki.example/source", []),
            ("https://pki-.example/source", []),
            ("https://pki._tcp.example/source", []),
            ("https://pki.example%2einvalid/source", []),
            ("https://pki.example:/source", []),
            ("https://pki.example:0/source", []),
            ("https://pki.example:08443/source", []),
            ("https://123.000.000.001/source", []),
            ("https://pki.example/../source", []),
            ("https://pki.example/%2e%2e/source", []),
            ("https://pki.example/swift%2fsource", []),
            ("https://pki.example/swift%252fsource", []),
            ("https://pki.example/sources;debug/source", []),
            ("https://pki.example/sources%3bdebug/source", []),
            ("https://pki.example/sources%23debug/source", []),
            (r"https://pki.example/sources\source", []),
            ("https://pki.example/swift%20source", []),
            ("https://pki.example/swift%00source", []),
            ("https://pki.example/swift%7fsource", []),
            ("https://pki.example/swift%zzsource", []),
            ("https://localhost/source", []),
            ("https://127.0.0.1/source", []),
            ("https://127.0.0.1.nip.io/source", []),
            ("https://0x7f000001/source", []),
            ("https://[64:ff9b::7f00:1]/source", []),
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

    def test_duplicate_singleton_child_command_flags_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            duplicate_rail_url = valid_canary_summary()
            duplicate_rail_url["stages"][0]["command"].extend(
                ["--torii-base-url", "https://torii-backup.example.invalid"]
            )
            duplicate_rail_url.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(duplicate_rail_url),
                    [],
                    "stages[0].command must contain at most one --torii-base-url",
                )
            )
            duplicate_notary_export = valid_canary_summary()
            duplicate_notary_export["stages"][1]["command"].append(
                "--export-dir=/ops/iso/audit-export-copy"
            )
            duplicate_notary_export.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(duplicate_notary_export),
                    [],
                    "stages[1].command must contain at most one --export-dir",
                )
            )
            duplicate_verify_policy = valid_canary_summary()
            duplicate_verify_policy["stages"][2]["command"].append("--require-source-files")
            duplicate_verify_policy.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(duplicate_verify_policy),
                    [],
                    "stages[2].command must contain at most one --require-source-files",
                )
            )
            duplicate_planned_inbox = plan_only_canary_summary()
            duplicate_planned_inbox["planned_stages"][0]["command"].append(
                "--inbox-dir=/ops/iso/other-inbox"
            )
            duplicate_planned_inbox.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(duplicate_planned_inbox),
                    ["--allow-plan-only"],
                    "planned_stages[0].command must contain at most one --inbox-dir",
                )
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

    def test_boolean_child_command_flags_reject_equals_values(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            notary_all_value = valid_canary_summary()
            notary_all_value["stages"][1]["command"].append("--all=false")
            notary_all_value.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(notary_all_value),
                    [],
                    "stages[1].command[8] boolean flag --all must not use =value",
                )
            )
            verify_source_value = valid_canary_summary()
            verify_source_value["stages"][2]["command"][6] = "--require-source-files=false"
            verify_source_value.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(verify_source_value),
                    [],
                    "stages[2].command[6] boolean flag --require-source-files must not use =value",
                )
            )
            planned_notary_all_value = plan_only_canary_summary()
            planned_notary_all_value["planned_stages"][1]["command"].append("--all=false")
            planned_notary_all_value.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(planned_notary_all_value),
                    ["--allow-plan-only"],
                    "planned_stages[1].command[8] boolean flag --all must not use =value",
                )
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

    def test_numeric_child_command_flags_require_positive_values(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            zero_payload_cap = valid_canary_summary()
            zero_payload_cap["stages"][0]["command"].append("--max-payload-bytes=0")
            zero_payload_cap.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(zero_payload_cap),
                    [],
                    "--max-payload-bytes must be a positive decimal integer",
                )
            )
            negative_response_cap = valid_canary_summary()
            negative_response_cap["stages"][1]["command"].append(
                "--response-limit-bytes=-1"
            )
            negative_response_cap.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(negative_response_cap),
                    [],
                    "--response-limit-bytes must be a positive decimal integer",
                )
            )
            nan_timeout = valid_canary_summary()
            nan_timeout["stages"][0]["command"].append("--timeout-secs=nan")
            nan_timeout.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(nan_timeout),
                    [],
                    "--timeout-secs must be a positive finite number",
                )
            )
            planned_zero_timeout = plan_only_canary_summary()
            planned_zero_timeout["planned_stages"][1]["command"].append(
                "--timeout-secs=0"
            )
            planned_zero_timeout.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(planned_zero_timeout),
                    ["--allow-plan-only"],
                    "--timeout-secs must be a positive finite number",
                )
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

    def test_canary_child_command_path_values_are_canonical(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            rail_inbox_traversal = valid_canary_summary()
            rail_inbox_traversal["stages"][0]["command"][3] = "/ops/iso/../inbox"
            rail_inbox_traversal.pop("summary_sha256")
            rail_message_non_xml = valid_canary_summary()
            rail_message_non_xml["stages"][0]["command"].extend(
                ["--message", "/ops/iso/inbox/payment.txt"]
            )
            rail_message_non_xml.pop("summary_sha256")
            notary_export_dash = valid_canary_summary()
            notary_export_dash["stages"][1]["command"][3] = "--audit-export"
            notary_export_dash.pop("summary_sha256")
            notary_export_segment_dash = valid_canary_summary()
            notary_export_segment_dash["stages"][1]["command"][3] = "/ops/iso/--audit-export"
            notary_export_segment_dash.pop("summary_sha256")
            verify_receipt_non_json = valid_canary_summary()
            verify_receipt_non_json["stages"][2]["command"].extend(
                ["--receipt", "/ops/iso/receipts/notary.json"]
            )
            verify_receipt_non_json.pop("summary_sha256")
            planned_verify_receipt_dir_traversal = plan_only_canary_summary()
            planned_verify_receipt_dir_traversal["planned_stages"][2]["command"][3] = (
                "/ops/iso/../rail-receipts"
            )
            planned_verify_receipt_dir_traversal.pop("summary_sha256")
            planned_message_dash = plan_only_canary_summary()
            planned_message_dash["planned_stages"][0]["command"].extend(
                ["--message", "--payment.xml"]
            )
            planned_message_dash.pop("summary_sha256")
            cases = (
                (
                    digest_summary(rail_inbox_traversal),
                    [],
                    "stages[0].command[3] must not contain dot or parent segments",
                ),
                (
                    digest_summary(rail_message_non_xml),
                    [],
                    "stages[0].command[11] must point to a .xml file",
                ),
                (
                    digest_summary(notary_export_dash),
                    [],
                    "stages[1].command[3] must not start with a dash",
                ),
                (
                    digest_summary(notary_export_segment_dash),
                    [],
                    "stages[1].command[3] must not contain leading-dash path segments",
                ),
                (
                    digest_summary(verify_receipt_non_json),
                    [],
                    "stages[2].command[8] must point to a .receipt.json file",
                ),
                (
                    digest_summary(planned_verify_receipt_dir_traversal),
                    ["--allow-plan-only"],
                    "planned_stages[2].command[3] must not contain dot or parent segments",
                ),
                (
                    digest_summary(planned_message_dash),
                    ["--allow-plan-only"],
                    "planned_stages[0].command[11] must not start with a dash",
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

    def test_canary_child_commands_require_structural_flags(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            rail_missing_torii = valid_canary_summary()
            del rail_missing_torii["stages"][0]["command"][4:6]
            rail_missing_torii.pop("summary_sha256")
            notary_missing_export = valid_canary_summary()
            del notary_missing_export["stages"][1]["command"][2:4]
            notary_missing_export.pop("summary_sha256")
            planned_notary_missing_endpoint = plan_only_canary_summary()
            del planned_notary_missing_endpoint["planned_stages"][1]["command"][6:8]
            planned_notary_missing_endpoint.pop("summary_sha256")
            planned_verify_missing_receipts = plan_only_canary_summary()
            planned_verify_missing_receipts["planned_stages"][2]["command"] = [
                sys.executable,
                str(REPO_ROOT / "scripts" / "iso_operator_receipt_verify.py"),
                "--require-source-files",
            ]
            planned_verify_missing_receipts.pop("summary_sha256")
            planned_verify_missing_source_policy = plan_only_canary_summary()
            planned_verify_missing_source_policy["planned_stages"][2]["command"].remove(
                "--require-source-files"
            )
            planned_verify_missing_source_policy.pop("summary_sha256")
            cases = (
                (
                    digest_summary(rail_missing_torii),
                    [],
                    "stages[0].command must contain --torii-base-url",
                ),
                (
                    digest_summary(notary_missing_export),
                    [],
                    "stages[1].command must contain --export-dir",
                ),
                (
                    digest_summary(planned_notary_missing_endpoint),
                    ["--allow-plan-only"],
                    "planned_stages[1].command must contain --endpoint",
                ),
                (
                    digest_summary(planned_verify_missing_receipts),
                    ["--allow-plan-only"],
                    "planned_stages[2].command must contain one of --receipt, --receipt-dir",
                ),
                (
                    digest_summary(planned_verify_missing_source_policy),
                    ["--allow-plan-only"],
                    "planned_stages[2] did not require receipt source files",
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
            executed_whitespace = valid_canary_summary()
            executed_whitespace["stages"][0]["receipt_dir"] = "/ops/iso/rail receipts"
            executed_whitespace.pop("summary_sha256")
            executed_empty = valid_canary_summary()
            executed_empty["stages"][0]["receipt_dir"] = "/ops/iso//rail-receipts"
            executed_empty.pop("summary_sha256")
            executed_dash = valid_canary_summary()
            executed_dash["stages"][0]["receipt_dir"] = "--rail-receipts"
            executed_dash.pop("summary_sha256")
            executed_segment_dash = valid_canary_summary()
            executed_segment_dash["stages"][0]["receipt_dir"] = "/ops/iso/--rail-receipts"
            executed_segment_dash.pop("summary_sha256")
            planned_control = plan_only_canary_summary()
            planned_control["planned_stages"][0]["receipt_dir"] = "/ops/iso/rail\nreceipts"
            planned_control.pop("summary_sha256")
            planned_dash = plan_only_canary_summary()
            planned_dash["planned_stages"][0]["receipt_dir"] = "--rail-receipts"
            planned_dash.pop("summary_sha256")
            planned_segment_dash = plan_only_canary_summary()
            planned_segment_dash["planned_stages"][0]["receipt_dir"] = "/ops/iso/--rail-receipts"
            planned_segment_dash.pop("summary_sha256")
            planned_semicolon = plan_only_canary_summary()
            planned_semicolon["planned_stages"][0]["receipt_dir"] = "/ops/iso/rail;receipts"
            planned_semicolon.pop("summary_sha256")
            planned_parent = plan_only_canary_summary()
            planned_parent["planned_stages"][1]["receipt_dir"] = r"..\notary-receipts"
            planned_parent.pop("summary_sha256")
            planned_backslash = plan_only_canary_summary()
            planned_backslash["planned_stages"][0]["receipt_dir"] = r"ops\iso\rail-receipts"
            planned_backslash.pop("summary_sha256")
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
                    digest_summary(executed_whitespace),
                    [],
                    "stages[0].receipt_dir must not contain whitespace",
                ),
                (
                    digest_summary(executed_empty),
                    [],
                    "stages[0].receipt_dir must not contain empty path segments",
                ),
                (
                    digest_summary(executed_dash),
                    [],
                    "stages[0].receipt_dir must not start with a dash",
                ),
                (
                    digest_summary(executed_segment_dash),
                    [],
                    "stages[0].receipt_dir must not contain leading-dash path segments",
                ),
                (
                    digest_summary(planned_control),
                    ["--allow-plan-only"],
                    "planned_stages[0].receipt_dir must not contain control characters",
                ),
                (
                    digest_summary(planned_dash),
                    ["--allow-plan-only"],
                    "planned_stages[0].receipt_dir must not start with a dash",
                ),
                (
                    digest_summary(planned_segment_dash),
                    ["--allow-plan-only"],
                    "planned_stages[0].receipt_dir must not contain leading-dash path segments",
                ),
                (
                    digest_summary(planned_semicolon),
                    ["--allow-plan-only"],
                    "planned_stages[0].receipt_dir must not contain semicolon path parameters",
                ),
                (
                    digest_summary(planned_parent),
                    ["--allow-plan-only"],
                    "planned_stages[1].receipt_dir must not contain dot or parent segments",
                ),
                (
                    digest_summary(planned_backslash),
                    ["--allow-plan-only"],
                    "planned_stages[0].receipt_dir must use forward slashes",
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
            executed_command_dash = valid_canary_summary()
            executed_command_dash["stages"][0]["command"][7] = "--rail-receipts"
            executed_command_dash.pop("summary_sha256")
            executed_command_segment_dash = valid_canary_summary()
            executed_command_segment_dash["stages"][0]["command"][7] = (
                "/ops/iso/--rail-receipts"
            )
            executed_command_segment_dash.pop("summary_sha256")
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
                    digest_summary(executed_command_dash),
                    [],
                    "stages[0].command[7] must not start with a dash",
                ),
                (
                    digest_summary(executed_command_segment_dash),
                    [],
                    "stages[0].command[7] must not contain leading-dash path segments",
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

    def test_canary_stage_sequence_must_match_runner_order(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            executed = valid_canary_summary()
            executed["stages"][0], executed["stages"][1] = (
                executed["stages"][1],
                executed["stages"][0],
            )
            executed["stages"][0]["started_at"] = "2026-06-04T00:00:00+00:00"
            executed["stages"][0]["finished_at"] = "2026-06-04T00:00:00.200000+00:00"
            executed["stages"][1]["started_at"] = "2026-06-04T00:00:00.200000+00:00"
            executed["stages"][1]["finished_at"] = "2026-06-04T00:00:00.400000+00:00"
            executed.pop("summary_sha256")
            planned = plan_only_canary_summary()
            planned["planned_stages"][0], planned["planned_stages"][1] = (
                planned["planned_stages"][1],
                planned["planned_stages"][0],
            )
            planned.pop("summary_sha256")
            cases = (
                (digest_summary(executed), []),
                (digest_summary(planned), ["--allow-plan-only"]),
            )
            for body, extra_args in cases:
                with self.subTest(extra_args=extra_args):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("stages must follow canary order", stderr)

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
            (rail_url, "https://torii.example.invalid:/base", []),
            (rail_url, "https://torii.example.invalid:0/base", []),
            (rail_url, "https://torii.example.invalid:08443/base", []),
            (rail_url, "https://torii.example%2einvalid", []),
            (rail_url, "https://localhost/base", []),
            (rail_url, "https://127.0.0.1.nip.io/base", []),
            (rail_url, "https://0x7f000001/base", []),
            (rail_url, "https://[64:ff9b::7f00:1]/base", []),
            (rail_url, "https://torii.example.invalid/base%20v1", []),
            (rail_url, "https://torii.example.invalid/base%00v1", []),
            (rail_url, "https://torii.example.invalid/base%3bdebug", []),
            (rail_url, "https://torii.example.invalid/base%3fdebug", []),
            (rail_url, "https://torii.example.invalid/base%252fv1", []),
            (rail_url, "https://torii.example.invalid/base%zzv1", []),
            (notary_url, "https://notary.example.invalid/anchor;debug", []),
            (notary_url, "https://notary.example.invalid/anchor?debug=true", []),
            (notary_url, "https://notary.example.invalid/anchor#fragment", []),
            (notary_url, "https://notary.example.invalid:/anchor", []),
            (notary_url, "https://notary.example.invalid:0/anchor", []),
            (notary_url, "https://notary.example.invalid:08443/anchor", []),
            (notary_url, "https://127.0.0.1/anchor", []),
            (notary_url, "https://10.1.2.3.sslip.io/anchor", []),
            (notary_url, "https://notary.example.invalid/archive%3bdebug/anchor", []),
            (notary_url, "https://notary.example.invalid/archive%40debug/anchor", []),
            (notary_url, "https://0x7f.0.0.1/anchor", []),
            (notary_url, "https://[::127.0.0.1]/anchor", []),
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
                (
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "authority",
                        "Example\nRail PKI",
                    ),
                    "source.authority must not contain control characters",
                ),
                (
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "version",
                        "2026-Q2\n",
                    ),
                    "source.version must not contain control characters",
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

    def test_trust_profile_identity_fields_are_rechecked_in_archives(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root, valid_canary_summary())
            cases = (
                (
                    "uppercase-profile-id",
                    lambda summary: summary["bundles"][0].__setitem__(
                        "profile_id",
                        "Swift-CBPR-Plus",
                    ),
                    "profile_id must be a canonical lowercase profile id",
                ),
                (
                    "underscore-profile-id",
                    lambda summary: summary["bundles"][0].__setitem__(
                        "profile_id",
                        "swift_cbpr_plus",
                    ),
                    "profile_id must be a canonical lowercase profile id",
                ),
                (
                    "unknown-rail",
                    lambda summary: summary["bundles"][0].__setitem__(
                        "rail",
                        "swift",
                    ),
                    "rail must be one of",
                ),
                (
                    "uppercase-rail",
                    lambda summary: summary["bundles"][0].__setitem__(
                        "rail",
                        "Swift-CBPR-Plus",
                    ),
                    "rail must be one of",
                ),
            )
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    trust_path = write_trust_summary(root / name)
                    rewrite_trust_summary(trust_path, mutate)

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_trust_source_identity_fields_are_required_and_must_be_strings(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root, valid_canary_summary())
            cases = (
                (
                    "missing-authority",
                    lambda summary: summary["bundles"][0]["source"].pop("authority"),
                    "source.authority must be a non-empty string",
                ),
                (
                    "empty-authority",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "authority",
                        "",
                    ),
                    "source.authority must be a non-empty string",
                ),
                (
                    "null-authority",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "authority",
                        None,
                    ),
                    "source.authority must be a non-empty string",
                ),
                (
                    "missing-version",
                    lambda summary: summary["bundles"][0]["source"].pop("version"),
                    "source.version must be a non-empty string",
                ),
                (
                    "numeric-version",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "version",
                        2026,
                    ),
                    "source.version must be a non-empty string",
                ),
            )

            for name, mutate, message in cases:
                with self.subTest(name=name):
                    trust_path = write_trust_summary(root / name)
                    rewrite_trust_summary(trust_path, mutate)

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_placeholder_trust_source_metadata_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root, valid_canary_summary())
            cases = (
                (
                    "authority",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "authority",
                        "Swift operator PKI placeholder",
                    ),
                    "source.authority must not contain placeholder production metadata",
                ),
                (
                    "version",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "version",
                        "replace-before-production",
                    ),
                    "source.version must not contain placeholder production metadata",
                ),
                (
                    "url",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "url",
                        "https://pki.swift.example.invalid/iso20022",
                    ),
                    "source.url must not use example.invalid placeholder provenance",
                ),
            )
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    trust_path = write_trust_summary(root / name)
                    rewrite_trust_summary(trust_path, mutate)

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
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
                    "source-url-empty-segment",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "url",
                        "https://pki.example.invalid/swift//cbpr-plus",
                    ),
                    "source.url path must not contain empty segments",
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
                    "source-authority",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "authority",
                        "Example Rail PKI ",
                    ),
                    "source.authority must not have surrounding whitespace",
                ),
                (
                    "source-version",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "version",
                        " 2026-Q2",
                    ),
                    "source.version must not have surrounding whitespace",
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
            timed_out = valid_canary_summary()
            timed_out["stages"][0]["timed_out"] = True
            timed_out.pop("summary_sha256")
            cases.append((digest_summary(timed_out), "timed out"))
            weak_verify = valid_canary_summary()
            weak_verify["stages"][2]["command"].remove("--require-source-files")
            weak_verify.pop("summary_sha256")
            cases.append((digest_summary(weak_verify), "did not require receipt source files"))
            truncated = valid_canary_summary()
            truncated["stages"][2]["stdout_truncated"] = True
            truncated.pop("summary_sha256")
            cases.append((digest_summary(truncated), "stdout_preview is truncated"))
            for stage_index, field, message in (
                (0, "stdout_truncated", "stdout_preview is truncated"),
                (0, "stderr_truncated", "stderr_preview is truncated"),
                (1, "stdout_truncated", "stdout_preview is truncated"),
                (1, "stderr_truncated", "stderr_preview is truncated"),
                (2, "stderr_truncated", "stderr_preview is truncated"),
            ):
                truncated_stage = valid_canary_summary()
                truncated_stage["stages"][stage_index][field] = True
                truncated_stage.pop("summary_sha256")
                cases.append((digest_summary(truncated_stage), message))
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
                    "/ops/iso/receipts/rail 0.receipt.json",
                    "must not contain whitespace",
                ),
                (
                    "/ops/iso/receipts/rail.receipt.json;v=1",
                    "must not contain semicolon path parameters",
                ),
                (
                    "/ops/iso/receipts//rail.receipt.json",
                    "must not contain empty path segments",
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
                    r"/ops\iso/receipts/rail.receipt.json",
                    "must use forward slashes",
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

    def test_receipt_verifier_stdout_requires_successful_receipt_entries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = (
                (
                    "missing-ok",
                    lambda receipt: receipt.pop("ok"),
                    "stdout_preview.receipts[0].ok must be a boolean",
                ),
                (
                    "non-bool-ok",
                    lambda receipt: receipt.__setitem__("ok", "true"),
                    "stdout_preview.receipts[0].ok must be a boolean",
                ),
                (
                    "missing-status",
                    lambda receipt: receipt.pop("status_code"),
                    "stdout_preview.receipts[0].status_code must be an HTTP status integer",
                ),
                (
                    "bool-status",
                    lambda receipt: receipt.__setitem__("status_code", True),
                    "stdout_preview.receipts[0].status_code must be an HTTP status integer",
                ),
                (
                    "too-small-status",
                    lambda receipt: receipt.__setitem__("status_code", 99),
                    "stdout_preview.receipts[0].status_code must be an HTTP status integer",
                ),
                (
                    "mismatched-status",
                    lambda receipt: receipt.update({"ok": True, "status_code": 500}),
                    "stdout_preview.receipts[0].ok does not match status_code success state",
                ),
                (
                    "failed-status",
                    lambda receipt: receipt.update({"ok": False, "status_code": 503}),
                    "stdout_preview.receipts[0] did not succeed",
                ),
                (
                    "redirect-status",
                    lambda receipt: receipt.update({"ok": False, "status_code": 302}),
                    "stdout_preview.receipts[0] did not succeed",
                ),
            )
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    receipt_summary = json.loads(receipt_stdout())
                    mutate(receipt_summary["receipts"][0])
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

    def test_receipt_verifier_stdout_requires_kind_specific_metadata(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = (
                (
                    "notary-missing-anchor",
                    0,
                    lambda receipt: receipt.pop("anchor_sha256"),
                    "stdout_preview.receipts[0].anchor_sha256 must be a lowercase SHA-256 digest",
                ),
                (
                    "notary-bad-index",
                    0,
                    lambda receipt: receipt.__setitem__("index_sha256", "A" * 64),
                    "stdout_preview.receipts[0].index_sha256 must be a lowercase SHA-256 digest",
                ),
                (
                    "notary-negative-record-count",
                    0,
                    lambda receipt: receipt.__setitem__("record_count", -1),
                    "stdout_preview.receipts[0].record_count must be a non-negative integer",
                ),
                (
                    "notary-cross-kind-field",
                    0,
                    lambda receipt: receipt.__setitem__("payload_sha256", "0" * 64),
                    "stdout_preview.receipts[0].payload_sha256 is not valid for iso-audit-notary",
                ),
                (
                    "rail-missing-message-type",
                    1,
                    lambda receipt: receipt.pop("message_type"),
                    "stdout_preview.receipts[1].message_type must be a non-empty string",
                ),
                (
                    "rail-bad-payload",
                    1,
                    lambda receipt: receipt.__setitem__("payload_sha256", "abc"),
                    "stdout_preview.receipts[1].payload_sha256 must be a lowercase SHA-256 digest",
                ),
                (
                    "rail-unknown-profile",
                    1,
                    lambda receipt: receipt.__setitem__("profile", "unknown-rail"),
                    "stdout_preview.receipts[1].profile must be one of",
                ),
                (
                    "rail-cross-kind-field",
                    1,
                    lambda receipt: receipt.__setitem__("anchor_sha256", "0" * 64),
                    "stdout_preview.receipts[1].anchor_sha256 is not valid for iso-rail-gateway",
                ),
                (
                    "rail-legacy-message-type",
                    1,
                    lambda receipt: receipt.__setitem__("message_type", "colr.007"),
                    "stdout_preview.receipts[1].message_type uses legacy rail message type",
                ),
            )
            for name, receipt_index, mutate, message in cases:
                with self.subTest(name=name):
                    receipt_summary = json.loads(receipt_stdout())
                    mutate(receipt_summary["receipts"][receipt_index])
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

    def test_trust_summary_source_freshness_policy_is_required_and_strong_enough(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            cases = (
                (
                    "missing",
                    lambda trust: trust.pop("max_source_age_days"),
                    "max_source_age_days must be recorded",
                    [],
                ),
                (
                    "null",
                    lambda trust: trust.__setitem__("max_source_age_days", None),
                    "max_source_age_days must be a positive integer",
                    [],
                ),
                (
                    "bool",
                    lambda trust: trust.__setitem__("max_source_age_days", True),
                    "max_source_age_days must be a positive integer",
                    [],
                ),
                (
                    "string",
                    lambda trust: trust.__setitem__("max_source_age_days", "7"),
                    "max_source_age_days must be a positive integer",
                    [],
                ),
                (
                    "weaker-than-evidence",
                    lambda trust: trust.__setitem__("max_source_age_days", 36500),
                    "max_source_age_days is weaker than --max-trust-source-age-days",
                    ["--max-trust-source-age-days", "7"],
                ),
            )
            for name, mutate, message, extra_args in cases:
                with self.subTest(name=name):
                    mutated_trust_path = write_trust_summary(root / f"trust-{name}")
                    rewrite_trust_summary(mutated_trust_path, mutate)

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(mutated_trust_path),
                        ]
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_trust_summary_profile_emittable_must_match_source_policy(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            rewrite_trust_summary(
                trust_path,
                lambda trust: (
                    trust.__setitem__("max_source_age_days", 1),
                    trust["bundles"][0]["source"].__setitem__(
                        "retrieved_at",
                        "2020-01-01T00:00:00+00:00",
                    ),
                ),
            )

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--max-trust-source-age-days",
                    "36500",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn(
                "profile_json_emittable does not match trust source policy",
                stderr,
            )

    def test_profile_json_cannot_be_reported_emitted_when_not_emittable(self):
        def mark_profile_emitted(trust):
            trust["profile_json_emitted"] = True
            refresh_profile_json_sha256(trust)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "synthetic", synthetic=True)
            rewrite_trust_summary(trust_path, mark_profile_emitted)

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-synthetic-trust",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn(
                "profile_json_emitted cannot be true when profile_json_emittable is false",
                stderr,
            )

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
                    "crl-summary-label-null",
                    lambda summary: summary["bundles"][0]["x509_crls"][0].__setitem__(
                        "label",
                        None,
                    ),
                    "x509_crls[0].label must be a non-empty string when provided",
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
