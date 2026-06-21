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
    allow_default_profile=False,
    require_source_files=True,
    endpoint_requires_insecure_http=False,
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
                "response_body_sha256": f"{offset + 401:064x}",
                "endpoint_requires_insecure_http": endpoint_requires_insecure_http,
                **(
                    {
                        "anchor_path": f"/ops/iso/notary/latest.notary.json",
                        "store_dir": "/ops/iso/notary-store",
                        "index_path": "/ops/iso/notary/messages.index.json",
                        "anchor_sha256": f"{offset + 101:064x}",
                        "index_sha256": f"{offset + 201:064x}",
                        "record_count": 1,
                    }
                    if kind == "iso-audit-notary"
                    else {
                        "message_type": "pacs.002",
                        "payload_sha256": f"{offset + 301:064x}",
                        "profile": "swift-cbpr-plus",
                        "rail_message_id": f"rail-drop-{offset}",
                        "source_path": f"/ops/iso/rail-inbox/rail-drop-{offset}.xml",
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
                    "version": EVIDENCE.RECEIPT_SUMMARY_VERSION,
                    "verified_receipts": verified_receipts,
                    "receipt_kind": kinds,
                    "allow_failed": allow_failed,
                    "allow_insecure_http": allow_insecure_http,
                    "allow_legacy_colr007": allow_legacy_colr007,
                    "allow_default_profile": allow_default_profile,
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
        "https://torii.local-bank.bank",
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
        "https://notary.local-bank.bank/iso-anchor",
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


def valid_canary_summary(*, receipt_entries=None, allow_default_profile=False):
    return digest_summary(
        {
            "version": EVIDENCE.CANARY_SUMMARY_VERSION,
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
                    stdout=receipt_stdout(
                        receipt_entries=receipt_entries,
                        allow_default_profile=allow_default_profile,
                    ),
                    started_at="2026-06-04T00:00:00.400000+00:00",
                    finished_at="2026-06-04T00:00:01+00:00",
                ),
            ],
        }
    )


def plan_only_canary_summary():
    return digest_summary(
        {
            "version": EVIDENCE.CANARY_SUMMARY_VERSION,
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
                "url": "https://pki.local-bank.bank/swift-cbpr-plus",
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


def alternate_crl_b64():
    tbs = trust_test.seq(
        trust_test.der_integer(1),
        trust_test.ALG_ID,
        trust_test.NAME,
        trust_test.der_time(b"260605000000Z"),
        trust_test.der_time(b"270605000000Z"),
    )
    return base64.b64encode(
        trust_test.seq(tbs, trust_test.ALG_ID, trust_test.der_bit_string(b"\x05"))
    ).decode("ascii")


ALT_CRL_B64 = alternate_crl_b64()
ALT_OCSP_B64 = base64.b64encode(
    trust_test.seq(trust_test.tlv(0x0A, b"\x01"))
).decode("ascii")


def replace_profile_der(summary, override_key, summary_key, der_b64):
    der = base64.b64decode(der_b64, validate=True)
    summary["bundles"][0]["profile_overrides"][override_key][0] = der_b64
    summary["bundles"][0][summary_key][0]["sha256"] = EVIDENCE.sha256_hex(der)
    summary["bundles"][0][summary_key][0]["byte_len"] = len(der)
    refresh_profile_json_sha256(summary)


def write_https_receipt_dirs(root, *, legacy_colr007=False, default_profile=False):
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
    notary_endpoint = "https://notary.local-bank.bank/iso-anchor"
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
    elif default_profile:
        rail_test.write_message(inbox, profile=None)
        sidecar_path = inbox / "rail-status.xml.json"
        sidecar = json.loads(sidecar_path.read_text(encoding="utf-8"))
        sidecar.pop("profile")
        sidecar_path.write_text(json.dumps(sidecar), encoding="utf-8")
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
        if default_profile:
            argv.append("--allow-default-profile")
        rc, _stdout, stderr = rail_test.run_main(
            argv
        )
    if rc != 0:
        raise AssertionError(stderr)
    rail_receipt = next((inbox / "receipts").glob("*.receipt.json"))
    rail_endpoint = (
        "https://torii.local-bank.bank/v1/iso20022/colr007"
        if legacy_colr007
        else "https://torii.local-bank.bank/v1/iso20022/pacs002"
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
                "response_body_sha256": receipt["response_body_sha256"],
                "endpoint_requires_insecure_http": EVIDENCE._url_requires_insecure_http_override(
                    EVIDENCE.urllib.parse.urlparse(
                        receipt["endpoint"]
                        if receipt["receipt_kind"] == "iso-audit-notary"
                        else receipt["endpoint_url"]
                    )
                ),
            }
            if receipt["receipt_kind"] == "iso-audit-notary":
                anchor = json.loads(Path(receipt["anchor_path"]).read_text(encoding="utf-8"))
                anchor_path = Path(receipt["anchor_path"])
                export_dir = (
                    anchor_path.parent.parent
                    if anchor_path.parent.name == audit_test.ADAPTER.ANCHOR_DIR
                    else anchor_path.parent
                )
                entry.update(
                    {
                        "anchor_path": receipt["anchor_path"],
                        "store_dir": anchor["store_dir"],
                        "index_path": str(export_dir / audit_test.ADAPTER.INDEX_FILE),
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
                        "rail_message_id": receipt["rail_message_id"],
                        "source_path": receipt["xml_path"],
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
    def test_secret_looking_unknown_keys_are_rejected_without_echo(self):
        cases = (
            ("password_evidence_unknown_secret", "evidence_unknown_secret"),
            ("%70assword_evidence_unknown_leak", "evidence_unknown_leak"),
            ("private-key_evidence_unknown_leak", "evidence_unknown_leak"),
            ("unexpected\x1bevidence_key", "\x1b"),
            ("unexpected_evidence_\uff4bey", "\uff4b"),
            ("x" * 129, "x" * 129),
        )
        for unknown_key, hidden in cases:
            with self.subTest(unknown_key=unknown_key):
                with self.assertRaises(EVIDENCE.EvidenceError) as caught:
                    EVIDENCE._reject_unknown_keys(
                        {unknown_key: "redacted"}, set(), "summary"
                    )

                message = str(caught.exception)
                self.assertIn("contains unknown keys", message)
                self.assertNotIn("password", message)
                self.assertNotIn(unknown_key, message)
                self.assertNotIn(hidden, message)
        many_unknown = {f"field_{offset}": "redacted" for offset in range(9)}
        with self.assertRaises(EVIDENCE.EvidenceError) as caught:
            EVIDENCE._reject_unknown_keys(many_unknown, set(), "summary")
        message = str(caught.exception)
        self.assertIn("contains unknown keys", message)
        self.assertNotIn("field_0", message)
        self.assertNotIn("field_8", message)

    def test_cli_argument_terminator_is_rejected_without_echo(self):
        hidden = "token=evidence-terminator-secret"
        cases = (
            (
                "raw",
                lambda: EVIDENCE._preflight_raw_cli_secrets(
                    ["--", "--summary-out", hidden],
                    {"--summary-out"},
                ),
            ),
            (
                "context",
                lambda: EVIDENCE._preflight_required_cli_values(
                    ["--", "--provider", hidden],
                    {"--provider"},
                    "context",
                ),
            ),
            (
                "boolean",
                lambda: EVIDENCE._preflight_boolean_cli_flags(
                    ["--", "--allow-plan-only", hidden],
                    {"--allow-plan-only"},
                ),
            ),
            (
                "path",
                lambda: EVIDENCE._preflight_output_cli_paths(
                    ["--", "--summary-out", hidden],
                    {"--summary-out"},
                ),
            ),
            (
                "numeric",
                lambda: EVIDENCE._preflight_numeric_cli_values(
                    ["--", "--receipt-verifier-timeout-secs", hidden],
                    integer_flags=set(),
                    number_flags={"--receipt-verifier-timeout-secs"},
                ),
            ),
        )
        for helper, run in cases:
            with self.subTest(helper=helper):
                with self.assertRaises(EVIDENCE.EvidenceError) as caught:
                    run()

                message = str(caught.exception)
                self.assertIn("argument terminator is not supported", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn("evidence-terminator-secret", message)

    def test_parser_rejects_abbreviated_long_options(self):
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as caught:
                EVIDENCE.build_parser().parse_args(["--summary-ou", "out"])

        self.assertEqual(caught.exception.code, 2)
        self.assertIn("unrecognized arguments", stderr.getvalue())
        self.assertIn("--summary-ou", stderr.getvalue())

    def test_raw_cli_control_characters_are_rejected_without_echo(self):
        hidden = "--unknown-evidence\x1bflag"
        with self.assertRaises(EVIDENCE.EvidenceError) as caught:
            EVIDENCE._preflight_raw_cli_secrets([hidden], {"--summary-out"})

        message = str(caught.exception)
        self.assertIn("CLI argument must not contain control characters", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn("\x1b", message)
        self.assertNotIn("unknown-evidence", message)

    def test_raw_cli_non_ascii_is_rejected_without_echo(self):
        hidden = "\uff0d\uff0dsummary-out"
        with self.assertRaises(EVIDENCE.EvidenceError) as caught:
            EVIDENCE._preflight_raw_cli_secrets([hidden], {"--summary-out"})

        message = str(caught.exception)
        self.assertIn("CLI argument must use printable ASCII", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn("summary-out", message)

    def test_output_cli_path_flags_reject_flag_like_values(self):
        cases = (
            ["--summary-out"],
            ["--summary-out", ""],
            ["--summary-out", "--receipt-dir"],
            ["--summary-out="],
            ["--summary-out=--receipt-dir"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                with self.assertRaisesRegex(
                    EVIDENCE.EvidenceError,
                    "--summary-out requires a path value",
                ):
                    EVIDENCE._preflight_output_cli_paths(argv, {"--summary-out"})

    def test_output_cli_paths_reject_encoded_secret_material_without_echo(self):
        cases = (
            ("token=evidence-path-leak.summary.json", "token=evidence-path-leak"),
            ("token%3Devidence-path-leak.summary.json", "token=evidence-path-leak"),
            ("%70assword%253Devidence-path-leak.summary.json", "password=evidence-path-leak"),
            ("token-evidence-path-secret.summary.json", "token-evidence-path-secret"),
        )
        for raw_path, decoded_secret in cases:
            with self.subTest(raw_path=raw_path):
                with self.assertRaises(EVIDENCE.EvidenceError) as caught:
                    EVIDENCE._preflight_output_cli_paths(
                        ["--summary-out", raw_path], {"--summary-out"}
                    )

                message = str(caught.exception)
                self.assertIn("secret-looking material", message)
                self.assertNotIn(raw_path, message)
                self.assertNotIn(decoded_secret, message)
                self.assertNotIn("evidence-path-leak", message)

    def test_summary_output_rejects_repository_fixture_artifacts(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            output_path = root / "fixtures" / "iso20022" / "evidence.summary.json"

            with self.assertRaisesRegex(
                EVIDENCE.EvidenceError,
                "output path must not point to checked-in ISO fixture artifacts",
            ):
                EVIDENCE._write_text_output(output_path, "{}\n")

            self.assertFalse((root / "fixtures").exists())
            with self.assertRaisesRegex(
                EVIDENCE.EvidenceError,
                "output path must not point to checked-in ISO fixture artifacts",
            ):
                EVIDENCE._reject_repository_output_path(
                    Path("fixtures/iso20022/evidence.summary.json"),
                    "output path",
                )

    def test_summary_output_rejects_repository_fixture_before_input_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            output_path = root / "fixtures" / "iso20022" / "evidence.summary.json"

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(root / "missing-canary.summary.json"),
                    "--trust-summary",
                    str(root / "missing-trust.summary.json"),
                    "--summary-out",
                    str(output_path),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "--summary-out must not point to checked-in ISO fixture artifacts",
                stderr,
            )
            self.assertNotIn("does not exist", stderr)
            self.assertFalse((root / "fixtures").exists())

    def test_direct_receipt_selectors_reject_repository_fixture_artifacts(self):
        cases = (
            (
                "--receipt",
                Path("fixtures/iso20022/receipts/rail.receipt.json"),
            ),
            (
                "--receipt-dir",
                Path("fixtures/iso20022/receipts"),
            ),
        )
        for flag, path in cases:
            with self.subTest(flag=flag):
                with self.assertRaisesRegex(
                    EVIDENCE.EvidenceError,
                    f"{flag} must not point to checked-in ISO fixture artifacts",
                ):
                    EVIDENCE._preflight_output_cli_paths([flag, str(path)], {flag})

    def test_direct_receipt_selectors_reject_repository_fixture_before_input_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            cases = (
                (
                    "--receipt",
                    root / "fixtures" / "iso20022" / "receipts" / "rail.receipt.json",
                ),
                (
                    "--receipt-dir",
                    root / "fixtures" / "iso20022" / "receipts",
                ),
            )
            for flag, path in cases:
                with self.subTest(flag=flag):
                    rc, stdout, stderr = run_evidence(
                        [flag, str(path)],
                        include_context=False,
                        include_freshness=False,
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(
                        f"{flag} must not point to checked-in ISO fixture artifacts",
                        stderr,
                    )
                    self.assertNotIn("provide at least one --canary-summary", stderr)
                    self.assertFalse((root / "fixtures").exists())

    def test_local_path_validators_reject_percent_encoded_smuggling(self):
        overlong_path = "out/" + ("a" * (EVIDENCE.MAX_LOCAL_PATH_CHARS + 1))
        cases = (
            (
                "raw overlong",
                lambda raw: EVIDENCE._reject_raw_output_path_smuggling(raw, "raw path"),
                overlong_path,
                f"no longer than {EVIDENCE.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "output overlong",
                lambda raw: EVIDENCE._reject_output_path_smuggling(Path(raw), "output path"),
                overlong_path,
                f"no longer than {EVIDENCE.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "input overlong",
                lambda raw: EVIDENCE._reject_path_smuggling(raw, "config_path"),
                overlong_path,
                f"no longer than {EVIDENCE.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "raw encoded dot",
                lambda raw: EVIDENCE._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%2e/summary.json",
                "encoded dot or separator",
            ),
            (
                "output encoded slash",
                lambda raw: EVIDENCE._reject_output_path_smuggling(Path(raw), "output path"),
                "out/%2f/summary.json",
                "encoded dot or separator",
            ),
            (
                "raw uri prefix",
                lambda raw: EVIDENCE._reject_raw_output_path_smuggling(raw, "raw path"),
                "file:out/summary.json",
                "URI or drive prefixes",
            ),
            (
                "input drive prefix",
                lambda raw: EVIDENCE._reject_path_smuggling(raw, "config_path"),
                "C:/ops/canary.json",
                "URI or drive prefixes",
            ),
            (
                "input encoded semicolon",
                lambda raw: EVIDENCE._reject_path_smuggling(raw, "config_path"),
                "/ops/%3b/canary.json",
                "encoded semicolon",
            ),
            (
                "input encoded delimiter",
                lambda raw: EVIDENCE._reject_path_smuggling(raw, "config_path"),
                "/ops/%40/canary.json",
                "encoded URL delimiter",
            ),
            (
                "raw encoded percent",
                lambda raw: EVIDENCE._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%25/summary.json",
                "encoded percent",
            ),
            (
                "raw encoded space",
                lambda raw: EVIDENCE._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%20/summary.json",
                "percent-encoded control or space",
            ),
            (
                "raw malformed percent",
                lambda raw: EVIDENCE._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%zz/summary.json",
                "malformed percent",
            ),
        )
        for name, call, raw, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(EVIDENCE.EvidenceError) as caught:
                    call(raw)

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(raw, message)

    def test_overlong_archive_strings_are_rejected_without_echo(self):
        overlong = "M" * (EVIDENCE.MAX_CLEAN_STRING_CHARS + 1)
        cases = (
            (
                "required",
                lambda: EVIDENCE._required_string({"path": overlong}, "path", "summary"),
                f"summary.path must be no longer than {EVIDENCE.MAX_CLEAN_STRING_CHARS} characters",
            ),
            (
                "list",
                lambda: EVIDENCE._required_clean_string_list(
                    {"oids": [overlong]}, "oids", "bundle"
                ),
                f"bundle.oids[0] must be no longer than {EVIDENCE.MAX_CLEAN_STRING_CHARS} characters",
            ),
        )
        for name, call, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(EVIDENCE.EvidenceError) as caught:
                    call()

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(overlong, message)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary = valid_canary_summary()
            canary["provider"] = overlong
            canary_path = write_canary(root, digest_summary(canary))
            trust_path = write_trust_summary(root / "trust")

            rc, stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                f".provider must be no longer than {EVIDENCE.MAX_CLEAN_STRING_CHARS} characters",
                stderr,
            )
            self.assertNotIn(overlong, stderr)

    def test_url_paths_reject_raw_delimiter_smuggling(self):
        cases = (
            "https://pki.local-bank.bank/source:debug",
            "https://pki.local-bank.bank/source@debug",
            "https://pki.local-bank.bank/source[debug]",
        )
        for url in cases:
            with self.subTest(url=url):
                with self.assertRaises(EVIDENCE.EvidenceError) as caught:
                    EVIDENCE._check_https_url(
                        url,
                        "source.url",
                        allow_insecure_http=False,
                    )

                message = str(caught.exception)
                self.assertIn("path must not contain URL delimiter characters", message)
                self.assertNotIn(url, message)

    def test_urls_reject_non_ascii_smuggling(self):
        cases = (
            (
                "https://pki\u0661.local-bank.bank/source",
                "host must use printable ASCII",
            ),
            ("https://pki.local-bank.bank/source∕debug", "path must use printable ASCII"),
            (
                "https://pki.local-bank.bank/source%c3%a9",
                "path must not contain percent-encoded non-ASCII bytes",
            ),
        )
        for url, expected in cases:
            with self.subTest(url=url):
                with self.assertRaises(EVIDENCE.EvidenceError) as caught:
                    EVIDENCE._check_https_url(
                        url,
                        "source.url",
                        allow_insecure_http=False,
                    )

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(url, message)

    def test_source_urls_reject_secret_path_without_echo(self):
        cases = (
            "https://pki.example.com/source/token=evidence-url-secret",
            "https://pki.example.com/source/token-evidence-url-secret",
            "https://pki.example.com/source/token%3Devidence-url-secret",
            "https://pki.example.com/source/token%253Devidence-url-secret",
        )
        for url in cases:
            with self.subTest(url=url):
                with self.assertRaises(EVIDENCE.EvidenceError) as caught:
                    EVIDENCE._check_clean_http_url(
                        url,
                        "source.url",
                        allow_insecure_http=False,
                    )

                message = str(caught.exception)
                self.assertIn("secret-looking material", message)
                self.assertNotIn(url, message)
                self.assertNotIn("token=", message)
                self.assertNotIn("evidence-url-secret", message)

    def test_source_urls_reject_secret_host_and_parser_errors_without_echo(self):
        cases = (
            (
                "https://token-evidence-host-secret.pki.example.com/source",
                "secret-looking material",
            ),
            ("https://[token-evidence-host-secret/source", "is not a valid URL"),
        )
        for url, expected in cases:
            with self.subTest(url=url):
                with self.assertRaises(EVIDENCE.EvidenceError) as caught:
                    EVIDENCE._check_clean_http_url(
                        url,
                        "source.url",
                        allow_insecure_http=False,
                    )

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(url, message)
                self.assertNotIn("token-evidence-host-secret", message)

    def test_boolean_cli_flags_reject_values_without_echo(self):
        cases = (
            (["--allow-plan-only=true"], "--allow-plan-only", "--allow-plan-only=true"),
            (
                ["--allow-profile-json-not-emitted", "true"],
                "--allow-profile-json-not-emitted",
                "true",
            ),
        )
        for argv, flag, rejected in cases:
            with self.subTest(argv=argv):
                rc, stdout, stderr = run_evidence(argv)

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertIn(f"{flag} does not take a value", stderr)
                self.assertNotIn(rejected, stderr)

    def test_numeric_cli_flags_reject_malformed_values_without_echo(self):
        cases = (
            ["--max-canary-age-days", "token=evidence-secret"],
            ["--max-trust-age-days=token=evidence-secret"],
            ["--receipt-verifier-timeout-secs", "--summary-out"],
            ["--receipt-verifier-timeout-secs="],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_evidence(argv)

                self.assertEqual(rc, 2)
                self.assertIn("numeric value", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("evidence-secret", stderr)

    def test_numeric_cli_flags_reject_unicode_digits_without_echo(self):
        hidden = "\u0661"
        cases = (
            ["--max-canary-age-days", hidden],
            [f"--max-trust-age-days={hidden}"],
            ["--receipt-verifier-timeout-secs", f"{hidden}.5"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_evidence(argv)

                self.assertEqual(rc, 2)
                self.assertIn("must use printable ASCII", stderr)
                self.assertNotIn(hidden, stderr)

    def test_raw_cli_secret_like_values_rejected_without_echo(self):
        cases = (
            ["--private-key=evidence-secret"],
            ["token=evidence-secret"],
            ["password=evidence-secret"],
            ["--provider", "token=evidence-secret"],
            ["--provider", "%70assword%253Devidence-secret"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_evidence(argv)

                self.assertEqual(rc, 2)
                self.assertIn("secret-looking", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("password=", stderr)
                self.assertNotIn("evidence-secret", stderr)

    def test_cli_identity_values_are_rejected_after_summary_args_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root, valid_canary_summary())
            trust_path = write_trust_summary(root / "trust")
            base_argv = [
                "--canary-summary",
                str(canary_path),
                "--trust-summary",
                str(trust_path),
            ]
            cases = (
                (
                    ["--provider", "token-evidence-cli-secret", "--environment", "preprod"],
                    "token-evidence-cli-secret",
                ),
                (
                    [
                        "--provider",
                        "local-bank",
                        "--environment",
                        "private-key-evidence-cli-secret",
                    ],
                    "private-key-evidence-cli-secret",
                ),
            )
            for argv, secret in cases:
                with self.subTest(argv=argv):
                    rc, _stdout, stderr = run_evidence(
                        base_argv + argv,
                        include_context=False,
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("secret-looking", stderr)
                    self.assertNotIn(secret, stderr)

    def test_recursive_secret_field_scanner_does_not_echo_key_material(self):
        with self.assertRaises(EVIDENCE.EvidenceError) as caught:
            EVIDENCE._check_no_secret_material(
                {"password_evidence_field_secret": "redacted"}
            )

        message = str(caught.exception)
        self.assertIn("forbidden secret-looking field", message)
        self.assertNotIn("password", message)
        self.assertNotIn("evidence_field_secret", message)
        self.assertNotIn("evidence-field-secret", message)

        with self.assertRaises(EVIDENCE.EvidenceError) as caught:
            EVIDENCE._check_no_secret_material(
                {"private-key_evidence_field_secret": "redacted"}
            )

        message = str(caught.exception)
        self.assertIn("forbidden secret-looking field", message)
        self.assertNotIn("private-key", message)
        self.assertNotIn("evidence_field_secret", message)

        with self.assertRaises(EVIDENCE.EvidenceError) as caught:
            EVIDENCE._check_no_secret_material({"unexpected\x1bevidence_key": "redacted"})

        message = str(caught.exception)
        self.assertIn("forbidden control-bearing field", message)
        self.assertNotIn("\x1b", message)
        self.assertNotIn("unexpected", message)

        with self.assertRaises(EVIDENCE.EvidenceError) as caught:
            EVIDENCE._check_no_secret_material({"metadata": "warning \x1b[31mred"})

        message = str(caught.exception)
        self.assertIn("unsafe control characters", message)
        self.assertNotIn("\x1b", message)
        self.assertNotIn("[31mred", message)

        with self.assertRaises(EVIDENCE.EvidenceError) as caught:
            EVIDENCE._check_no_secret_material(
                {"metadata": "%70assword%253Devidence-field-leak"}
            )

        message = str(caught.exception)
        self.assertIn("secret-looking material", message)
        self.assertNotIn("%70assword%253Devidence-field-leak", message)
        self.assertNotIn("password=evidence-field-leak", message)
        self.assertNotIn("evidence-field-leak", message)

    def test_context_cli_flags_reject_missing_empty_or_flag_like_values(self):
        cases = (
            ["--provider"],
            ["--provider", ""],
            ["--provider", "--environment"],
            ["--provider="],
            ["--provider=--environment"],
            ["--environment"],
            ["--environment", ""],
            ["--environment", "--summary-out"],
            ["--environment="],
            ["--environment=--summary-out"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_evidence(argv)

                self.assertEqual(rc, 2)
                self.assertIn("requires a context value", stderr)

    def test_default_rail_profile_cli_values_are_rejected_without_echo(self):
        hidden = "\u0661"
        cases = (
            (
                ["--default-rail-profile"],
                "requires a profile id value",
                None,
            ),
            (
                ["--default-rail-profile="],
                "requires a profile id value",
                None,
            ),
            (
                ["--default-rail-profile", "Swift-CBPR-Plus"],
                "canonical lowercase profile id",
                "Swift-CBPR-Plus",
            ),
            (
                ["--default-rail-profile", hidden],
                "must use printable ASCII",
                hidden,
            ),
            (
                ["--default-rail-profile", "token-evidence-cli-secret"],
                "secret-looking",
                "token-evidence-cli-secret",
            ),
        )
        for argv, expected, hidden_value in cases:
            with self.subTest(argv=argv):
                rc, stdout, stderr = run_evidence(argv)

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertIn(expected, stderr)
                if hidden_value is not None:
                    self.assertNotIn(hidden_value, stderr)

    def test_default_rail_profile_requires_default_profile_override(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root, valid_canary_summary())
            trust_path = write_trust_summary(root / "trust")

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--default-rail-profile",
                    "swift-cbpr-plus",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "--default-rail-profile requires --allow-default-profile",
                stderr,
            )

    def test_boolean_and_non_integer_file_read_limits_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / "evidence.summary.json"
            path.write_text("{}\n", encoding="utf-8")

            for limit in (True, "64"):
                with self.subTest(limit=limit):
                    with self.assertRaisesRegex(
                        EVIDENCE.EvidenceError,
                        "max file bytes must be a positive integer",
                    ):
                        EVIDENCE._read_regular_file(path, max_bytes=limit)

    def test_valid_canary_and_trust_summaries_pass(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            receipt_entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            canary_path = write_canary(
                root,
                valid_canary_summary(receipt_entries=receipt_entries),
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
            self.assertEqual(summary["canary_summaries"][0]["stage_dry_run"], [False, False, False])
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
            rail_receipt = next(
                receipt
                for receipt in summary["canary_summaries"][0]["receipt_summary"]["receipts"]
                if receipt["receipt_kind"] == "iso-rail-gateway"
            )
            notary_receipt = next(
                receipt
                for receipt in summary["canary_summaries"][0]["receipt_summary"]["receipts"]
                if receipt["receipt_kind"] == "iso-audit-notary"
            )
            self.assertTrue(notary_receipt["anchor_path"].endswith("latest.notary.json"))
            self.assertTrue(notary_receipt["store_dir"].endswith("store"))
            self.assertTrue(notary_receipt["index_path"].endswith("messages.index.json"))
            self.assertTrue(rail_receipt["source_path"].endswith("rail-status.xml"))
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
            self.assertFalse(summary["trust_summaries"][0]["allow_synthetic_der"])
            self.assertFalse(summary["trust_summaries"][0]["allow_record_only"])
            self.assertFalse(summary["trust_summaries"][0]["allow_insecure_source_url"])
            self.assertTrue(summary["trust_summaries"][0]["profile_json_emitted"])
            self.assertTrue(summary["trust_summaries"][0]["profile_json_emittable"])
            self.assertRegex(
                summary["trust_summaries"][0]["profile_json_sha256"],
                r"^[0-9a-f]{64}$",
            )
            trust_profile = summary["trust_summaries"][0]["profiles"][0]
            self.assertTrue(trust_profile["path"].endswith("trust-bundle.json"))
            self.assertRegex(trust_profile["bundle_sha256"], r"^[0-9a-f]{64}$")
            self.assertEqual(
                trust_profile["source"],
                {
                    "authority": "Local Bank Rail PKI",
                    "version": "2026-Q2",
                    "url": "https://pki.local-bank.bank/swift-cbpr-plus",
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

    def test_plan_only_output_records_null_receipt_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root, plan_only_canary_summary())
            trust_path = write_trust_summary(root / "trust")

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-plan-only",
                    "--allow-canary-stage-receipts-only",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            canary = summary["canary_summaries"][0]
            self.assertTrue(canary["plan_only"])
            self.assertEqual(canary["stage_dry_run"], [False, False, False])
            self.assertEqual(canary["stage_windows"], [])
            self.assertIn("receipt_summary", canary)
            self.assertIsNone(canary["receipt_summary"])
            self.assertIsNone(summary["receipt_verification"])

    def test_plan_only_insecure_http_override_requires_matching_command_url(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            body = plan_only_canary_summary()
            for planned_stage in body["planned_stages"]:
                planned_stage["command"].append("--allow-insecure-http")
            canary_path = write_canary(root, digest_summary(body))
            trust_path = write_trust_summary(root / "trust")

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-plan-only",
                    "--allow-canary-stage-receipts-only",
                    "--allow-insecure-http",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "uses --allow-insecure-http without an http:// or local/private endpoint",
                stderr,
            )

    def test_plan_only_failed_receipt_override_requires_receipt_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            body = plan_only_canary_summary()
            body["planned_stages"][2]["command"].append("--allow-failed")
            canary_path = write_canary(root, digest_summary(body))
            trust_path = write_trust_summary(root / "trust")

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-plan-only",
                    "--allow-canary-stage-receipts-only",
                    "--allow-failed-receipts",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "--allow-failed-receipts requires at least one receipt summary "
                "with allow_failed=true",
                stderr,
            )

    def test_plan_only_override_requires_matching_canary_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            receipt_entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            canary_path = write_canary(
                root,
                valid_canary_summary(receipt_entries=receipt_entries),
            )
            trust_path = write_trust_summary(root / "trust")
            summary_out = root / "evidence.summary.json"

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-plan-only",
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
            self.assertIn(
                "--allow-plan-only requires at least one canary summary with plan_only=true",
                stderr,
            )
            self.assertFalse(summary_out.exists())

    def test_partial_canary_override_requires_matching_canary_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            trust_path = write_trust_summary(root / "trust")
            full_canary_path = write_canary(
                root,
                valid_canary_summary(
                    receipt_entries=receipt_entries_from_dirs(notary_receipts, rail_receipts)
                ),
            )

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(full_canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-partial-canary",
                    "--receipt-dir",
                    str(notary_receipts),
                    "--receipt-dir",
                    str(rail_receipts),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "--allow-partial-canary requires at least one canary summary "
                "missing a rail or notary stage",
                stderr,
            )

            partial_root = root / "partial"
            partial_root.mkdir()
            partial_body = valid_canary_summary(
                receipt_entries=receipt_entries_from_dirs(rail_receipts)
            )
            partial_body["stages"] = [
                partial_body["stages"][0],
                partial_body["stages"][2],
            ]
            del partial_body["stages"][1]["command"][4:6]
            partial_body.pop("summary_sha256")
            partial_canary_path = write_canary(partial_root, digest_summary(partial_body))
            partial_summary_out = partial_root / "evidence.summary.json"

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(partial_canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-partial-canary",
                    "--receipt-dir",
                    str(rail_receipts),
                    "--summary-out",
                    str(partial_summary_out),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["policy"]["allow_partial_canary"])
            self.assertEqual(summary["canary_summaries"][0]["stage_names"], ["rail", "verify"])
            self.assertEqual(
                summary["receipt_verification"]["receipt_kind"],
                ["iso-rail-gateway"],
            )
            self.assertEqual(json.loads(partial_summary_out.read_text(encoding="utf-8")), summary)

    def test_unused_receipt_and_trust_overrides_are_rejected(self):
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
            cases = (
                (
                    "legacy",
                    ["--allow-legacy-colr007"],
                    "requires at least one rail receipt with legacy colr.007 message_type",
                ),
                (
                    "default-profile",
                    ["--allow-default-profile"],
                    "requires at least one rail receipt without an explicit profile",
                ),
                (
                    "record-only",
                    ["--allow-record-only-trust"],
                    "requires at least one trust summary verified with allow_record_only=true",
                ),
                (
                    "synthetic",
                    ["--allow-synthetic-trust"],
                    "requires at least one trust summary verified with allow_synthetic_der=true",
                ),
                (
                    "missing-source",
                    ["--allow-missing-trust-source"],
                    "requires at least one trust profile with source=null",
                ),
                (
                    "dry-run",
                    ["--allow-dry-run"],
                    "requires at least one canary stage command or "
                    "planned stage with dry_run=true",
                ),
            )

            for name, extra_args, message in cases:
                with self.subTest(name=name):
                    summary_out = root / f"unused-{name}.evidence.summary.json"
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
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertFalse(summary_out.exists())

    def test_unused_canary_policy_overrides_are_rejected_without_direct_receipts(self):
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
            cases = (
                (
                    "failed-receipts",
                    ["--allow-failed-receipts"],
                    "requires at least one receipt summary with allow_failed=true",
                ),
                (
                    "insecure-http",
                    ["--allow-insecure-http"],
                    "requires at least one canary command, receipt summary, or "
                    "trust summary verified with insecure HTTP",
                ),
                (
                    "receipt-source-missing",
                    ["--allow-receipt-source-missing"],
                    "requires at least one receipt summary with "
                    "require_source_files=false",
                ),
            )

            for name, extra_args, message in cases:
                with self.subTest(name=name):
                    summary_out = root / f"unused-canary-{name}.evidence.summary.json"
                    rc, stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                            "--allow-canary-stage-receipts-only",
                            "--summary-out",
                            str(summary_out),
                        ]
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertFalse(summary_out.exists())

    def test_dry_run_producer_stage_cannot_carry_stale_receipt_evidence(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            body = valid_canary_summary(
                receipt_entries=receipt_entries_from_dirs(notary_receipts, rail_receipts)
            )
            body["stages"][0]["command"].append("--dry-run")
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))
            trust_path = write_trust_summary(root / "trust")
            summary_out = root / "dry-run.evidence.summary.json"

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-dry-run",
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
            self.assertIn(
                "receipt_summary contains receipt kinds for stages not executed: "
                "iso-rail-gateway",
                stderr,
            )
            self.assertFalse(summary_out.exists())

    def test_dry_run_producer_stage_accepts_executed_stage_receipts_only(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, _rail_receipts = write_https_receipt_dirs(root)
            body = valid_canary_summary(
                receipt_entries=receipt_entries_from_dirs(notary_receipts)
            )
            body["stages"][0]["command"].append("--dry-run")
            del body["stages"][2]["command"][2:4]
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))
            trust_path = write_trust_summary(root / "trust")
            summary_out = root / "dry-run-executed-receipts.evidence.summary.json"

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-dry-run",
                    "--allow-canary-stage-receipts-only",
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            canary = summary["canary_summaries"][0]
            self.assertTrue(summary["policy"]["allow_dry_run"])
            self.assertEqual(canary["stage_dry_run"], [True, False, False])
            self.assertEqual(canary["receipt_summary"]["receipt_kind"], ["iso-audit-notary"])
            self.assertEqual(json.loads(summary_out.read_text(encoding="utf-8")), summary)

    def test_dry_run_producer_stage_accepts_direct_archive_for_executed_receipts_only(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, _rail_receipts = write_https_receipt_dirs(root)
            body = valid_canary_summary(
                receipt_entries=receipt_entries_from_dirs(notary_receipts)
            )
            body["stages"][0]["command"].append("--dry-run")
            del body["stages"][2]["command"][2:4]
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))
            trust_path = write_trust_summary(root / "trust")
            summary_out = root / "dry-run-direct-receipts.evidence.summary.json"

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-dry-run",
                    "--receipt-dir",
                    str(notary_receipts),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["policy"]["allow_dry_run"])
            self.assertEqual(
                summary["receipt_verification"]["receipt_kind"],
                ["iso-audit-notary"],
            )
            self.assertEqual(
                summary["canary_summaries"][0]["receipt_summary"]["receipt_kind"],
                ["iso-audit-notary"],
            )
            self.assertEqual(json.loads(summary_out.read_text(encoding="utf-8")), summary)

    def test_dry_run_policy_does_not_hide_missing_direct_archive_receipts(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            full_notary_receipts, full_rail_receipts = write_https_receipt_dirs(root)
            full_canary_path = write_json(
                root / "full-canary.summary.json",
                valid_canary_summary(
                    receipt_entries=receipt_entries_from_dirs(
                        full_notary_receipts, full_rail_receipts
                    )
                ),
            )
            dry_run_body = plan_only_canary_summary()
            dry_run_body["planned_stages"][0]["dry_run"] = True
            dry_run_body["planned_stages"][0]["command"].append("--dry-run")
            dry_run_body.pop("summary_sha256")
            dry_run_canary_path = write_json(
                root / "dry-run-canary.summary.json",
                digest_summary(dry_run_body),
            )
            trust_path = write_trust_summary(root / "trust")
            summary_out = root / "missing-direct-receipts.evidence.summary.json"

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(full_canary_path),
                    "--canary-summary",
                    str(dry_run_canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-plan-only",
                    "--allow-dry-run",
                    "--receipt-dir",
                    str(full_notary_receipts),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("direct receipt archive verification does not include", stderr)
            self.assertFalse(summary_out.exists())

    def test_canary_stage_receipts_only_cannot_be_combined_with_direct_receipts(self):
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
            summary_out = root / "evidence.summary.json"

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
                    "--allow-canary-stage-receipts-only",
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "--allow-canary-stage-receipts-only cannot be combined with "
                "--receipt or --receipt-dir",
                stderr,
            )
            self.assertFalse(summary_out.exists())

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

    def test_secret_looking_cli_paths_are_rejected_before_summary_output(self):
        cases = (
            (
                "--canary-summary",
                "token=evidence-canary-secret.summary.json",
                "evidence-canary-secret",
            ),
            (
                "--trust-summary",
                "token=evidence-trust-secret.summary.json",
                "evidence-trust-secret",
            ),
            (
                "--receipt",
                "token=evidence-receipt-secret.receipt.json",
                "evidence-receipt-secret",
            ),
            (
                "--receipt-dir",
                "token=evidence-receipt-dir-secret",
                "evidence-receipt-dir-secret",
            ),
            (
                "--summary-out",
                "token=evidence-output-secret.summary.json",
                "evidence-output-secret",
            ),
        )
        for flag, raw_path, secret in cases:
            with self.subTest(flag=flag):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    summary_out = root / "evidence.summary.json"
                    argv = [flag, str(root / raw_path)]
                    if flag != "--summary-out":
                        argv.extend(["--summary-out", str(summary_out)])

                    rc, stdout, stderr = run_evidence(argv)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn(secret, stderr)
                    self.assertFalse(summary_out.exists())

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
            ("token=evidence-config-secret.json", "secret-looking material"),
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

    def test_canary_config_path_rejects_checked_in_runbook_templates(self):
        checked_in_runbook = (
            REPO_ROOT
            / "fixtures"
            / "iso20022"
            / "operator_canary"
            / "swift_cbpr_plus.preprod.example.json"
        )
        cases = (
            "fixtures/iso20022/operator_canary/swift_cbpr_plus.preprod.example.json",
            str(checked_in_runbook),
            "/ops/release/fixtures/iso20022/operator_canary/swift_cbpr_plus.preprod.example.json",
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            for offset, config_path in enumerate(cases):
                with self.subTest(config_path=config_path):
                    body = valid_canary_summary()
                    body["config_path"] = config_path
                    canary_path = write_json(
                        root / f"template-config-{offset}.summary.json",
                        digest_summary(body),
                    )

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("checked-in operator canary templates", stderr)

    def test_trust_bundle_path_rejects_checked_in_templates(self):
        checked_in_bundle = (
            REPO_ROOT
            / "fixtures"
            / "iso20022"
            / "trust_bundles"
            / "swift_cbpr_plus.preprod.example.json"
        )
        cases = (
            "fixtures/iso20022/trust_bundles/swift_cbpr_plus.preprod.example.json",
            str(checked_in_bundle),
            "/ops/release/fixtures/iso20022/trust_bundles/swift_cbpr_plus.preprod.example.json",
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root, valid_canary_summary())
            for offset, bundle_path in enumerate(cases):
                with self.subTest(bundle_path=bundle_path):
                    trust_path = write_trust_summary(root / f"trust-template-{offset}")
                    rewrite_trust_summary(
                        trust_path,
                        lambda summary, path=bundle_path: summary["bundles"][0].__setitem__(
                            "path",
                            path,
                        ),
                    )

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("checked-in trust-bundle templates", stderr)

    def test_summary_input_paths_reject_checked_in_iso_fixture_artifacts(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            fixture_root = root / "fixtures" / "iso20022"
            fixture_canary_dir = fixture_root / "operator_canary"
            fixture_trust_dir = fixture_root / "trust_bundles"

            valid_trust_path = write_trust_summary(root / "trust")
            fixture_canary_path = fixture_canary_dir / "canary.summary.json"
            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(fixture_canary_path),
                    "--trust-summary",
                    str(valid_trust_path),
                ]
            )
            self.assertEqual(rc, 2)
            self.assertIn("must not point to checked-in ISO fixture artifacts", stderr)

            valid_canary_path = write_canary(root)
            fixture_trust_path = fixture_trust_dir / "trust.summary.json"
            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(valid_canary_path),
                    "--trust-summary",
                    str(fixture_trust_path),
                ]
            )
            self.assertEqual(rc, 2)
            self.assertIn("must not point to checked-in ISO fixture artifacts", stderr)

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

            rc, _stdout, stderr = run_evidence(
                base_argv + ["--provider", "local-b\u00e1nk", "--environment", "preprod"],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("--provider must use printable ASCII", stderr)
            self.assertNotIn("local-b\u00e1nk", stderr)

            rc, _stdout, stderr = run_evidence(
                base_argv + ["--provider", "local-bank", "--environment", "prepr\u043ed"],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("--environment must use printable ASCII", stderr)
            self.assertNotIn("prepr\u043ed", stderr)

            rc, _stdout, stderr = run_evidence(
                base_argv + ["--provider", "other-bank", "--environment", "preprod"],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("provider does not match expected provider", stderr)
            self.assertNotIn("other-bank", stderr)
            self.assertNotIn("local-bank", stderr)

            rc, _stdout, stderr = run_evidence(
                base_argv + ["--provider", "local-bank", "--environment", "prod"],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("environment does not match expected environment", stderr)
            self.assertNotIn("prod", stderr)
            self.assertNotIn("preprod", stderr)

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

    def test_canary_and_trust_summary_versions_are_rechecked(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            base_trust_path = write_trust_summary(root / "trust")
            base_canary_path = write_canary(root)

            version_cases = (
                ("missing", lambda body: body.pop("version")),
                ("boolean", lambda body: body.__setitem__("version", True)),
                ("unsupported", lambda body: body.__setitem__("version", 2)),
            )
            for name, mutate in version_cases:
                with self.subTest(kind="canary", name=name):
                    canary = valid_canary_summary()
                    mutate(canary)
                    canary_root = root / f"canary-{name}"
                    canary_root.mkdir()
                    canary_path = write_canary(
                        canary_root,
                        digest_summary(canary),
                    )

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(base_trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(".version must be 1", stderr)

            for name, mutate in version_cases:
                with self.subTest(kind="trust", name=name):
                    trust_path = write_trust_summary(root / f"trust-{name}")
                    rewrite_trust_summary(trust_path, mutate)

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(base_canary_path),
                            "--trust-summary",
                            str(trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(".version must be 1", stderr)

    def test_duplicate_summary_paths_do_not_echo_secret_segments(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_root = root / "canary"
            canary_root.mkdir()
            canary_path = write_canary(canary_root)
            secret_canary = root / "token=evidence-duplicate-secret.canary.summary.json"
            secret_canary.write_text(canary_path.read_text(encoding="utf-8"), encoding="utf-8")
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(secret_canary),
                    "--canary-summary",
                    str(secret_canary),
                    "--trust-summary",
                    str(trust_path),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("secret-looking material", stderr)
            self.assertNotIn("token=", stderr)
            self.assertNotIn("evidence-duplicate-secret", stderr)

    def test_duplicate_canary_summary_json_keys_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = root / "canary.summary.json"
            canary_path.write_text(
                '{"provider":"local-bank","token=evidence-duplicate-key-secret":1,"token=evidence-duplicate-key-secret":2}\n',
                encoding="utf-8",
            )
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)
            self.assertNotIn("evidence-duplicate-key-secret", stderr)

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
                '{"verified_receipts":2,"hidden_evidence_duplicate_key":1,"hidden_evidence_duplicate_key":2}\n'
            )
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))
            trust_path = write_trust_summary(root / "trust")

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)
            self.assertNotIn("hidden_evidence_duplicate_key", stderr)

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
                '{"verified_receipts":2,"token=evidence-duplicate-key-secret":1,"token=evidence-duplicate-key-secret":2}\n',
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
            self.assertNotIn("evidence-duplicate-key-secret", stderr)

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

    def test_failed_direct_receipt_verifier_stderr_redacts_secret_material(self):
        cases = (
            (
                "key-value",
                "receipt verifier echoed token=evidence-verifier-secret",
                "token=",
            ),
            (
                "identifier",
                "receipt verifier echoed token-evidence-verifier-secret",
                "token-evidence-verifier-secret",
            ),
            (
                "control",
                "receipt verifier \x1b[31mwarning",
                "\x1b",
            ),
        )
        for name, leaked_stderr, leaked_marker in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_root:
                root = Path(raw_root)
                canary_path = write_canary(root)
                trust_path = write_trust_summary(root / "trust")
                original_run = EVIDENCE._run_command_bounded
                EVIDENCE._run_command_bounded = lambda *_args, **_kwargs: (
                    1,
                    "",
                    False,
                    leaked_stderr,
                    False,
                    False,
                )
                try:
                    rc, stdout, stderr = run_evidence(
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
                self.assertEqual(stdout, "")
                self.assertIn("receipt verification failed", stderr)
                self.assertIn("receipt verifier stderr redacted", stderr)
                self.assertNotIn(leaked_marker, stderr)
                self.assertNotIn("evidence-verifier-secret", stderr)
                self.assertNotIn("[31mwarning", stderr)

    def test_successful_direct_receipt_verifier_stderr_is_rejected(self):
        cases = (
            (
                "warning",
                "receipt verifier warning",
                "receipt verifier warning",
                None,
            ),
            (
                "secret",
                "receipt verifier echoed token=evidence-verifier-secret",
                "receipt verifier stderr redacted",
                "token=",
            ),
            (
                "identifier",
                "receipt verifier echoed token-evidence-verifier-secret",
                "receipt verifier stderr redacted",
                "token-evidence-verifier-secret",
            ),
            (
                "control",
                "receipt verifier \x1b[31mwarning",
                "receipt verifier stderr redacted",
                "\x1b",
            ),
        )
        for name, emitted_stderr, expected, leaked_marker in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_root:
                root = Path(raw_root)
                canary_path = write_canary(root)
                trust_path = write_trust_summary(root / "trust")
                original_run = EVIDENCE._run_command_bounded
                EVIDENCE._run_command_bounded = lambda *_args, **_kwargs: (
                    0,
                    receipt_stdout(),
                    False,
                    emitted_stderr,
                    False,
                    False,
                )
                try:
                    rc, stdout, stderr = run_evidence(
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
                self.assertEqual(stdout, "")
                self.assertIn(
                    "receipt verifier emitted stderr on successful verification",
                    stderr,
                )
                self.assertIn(expected, stderr)
                if leaked_marker is not None:
                    self.assertNotIn(leaked_marker, stderr)
                    self.assertNotIn("evidence-verifier-secret", stderr)
                    self.assertNotIn("[31mwarning", stderr)

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

    def test_boolean_direct_receipt_verifier_output_limit_is_rejected(self):
        with self.assertRaisesRegex(
            EVIDENCE.EvidenceError,
            "output limit bytes must be positive",
        ):
            EVIDENCE._run_command_bounded(
                [sys.executable, "-c", "print('ok')"],
                True,
                1.0,
            )

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
            duplicate_profile_id = trust["bundles"][0]["profile_id"]
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
            self.assertNotIn(duplicate_profile_id, stderr)

    def test_trust_profiles_cannot_be_reused_across_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_one_path = write_trust_summary(root / "trust-one")
            trust_two_path = write_trust_summary(root / "trust-two")
            trust_two = json.loads(trust_two_path.read_text(encoding="utf-8"))
            trust_two["verified_at"] = "2026-06-04T00:00:01Z"
            duplicate_profile_id = trust_two["bundles"][0]["profile_id"]
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
            self.assertNotIn(duplicate_profile_id, stderr)

    def test_canary_rail_receipts_require_matching_trust_profile(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            cases = (
                (
                    "wrong-rail",
                    lambda bundle: (
                        bundle.update(
                            {
                                "profile_id": "fedwire-funds",
                                "rail": "fedwire-funds",
                            }
                        ),
                        bundle["profile_overrides"].update(
                            {
                                "id": "fedwire-funds",
                                "rail": "fedwire-funds",
                            }
                        ),
                    ),
                ),
                (
                    "wrong-profile-id",
                    lambda bundle: (
                        bundle.update({"profile_id": "swift-cbpr-plus-alt"}),
                        bundle["profile_overrides"].update(
                            {"id": "swift-cbpr-plus-alt"}
                        ),
                    ),
                ),
            )

            for name, mutate in cases:
                with self.subTest(name=name):
                    trust = json.loads(trust_path.read_text(encoding="utf-8"))
                    mutate(trust["bundles"][0])
                    refresh_profile_json_sha256(trust)
                    mutated_trust_path = write_json(
                        root / f"{name}-trust.summary.json",
                        digest_summary(trust),
                    )

                    rc, stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(mutated_trust_path),
                            "--allow-canary-stage-receipts-only",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(
                        "canary_summaries[0].receipt_summary.receipts[1].profile "
                        "has no matching trust profile coverage for canary environment",
                        stderr,
                    )
                    self.assertNotIn("'swift-cbpr-plus'", stderr)
                    self.assertNotIn("'preprod'", stderr)

    def test_custom_canary_profile_id_can_use_matching_trust_profile(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            custom_profile = "swift-cbpr-plus-alt"
            canary = valid_canary_summary()
            verify_stage = canary["stages"][2]
            receipt_summary = json.loads(verify_stage["stdout_preview"])
            receipt_summary["receipts"][1]["profile"] = custom_profile
            verify_stage["stdout_preview"] = (
                json.dumps(digest_receipt_summary(receipt_summary), sort_keys=True)
                + "\n"
            )
            canary_path = write_canary(root, digest_summary(canary))

            trust_path = write_trust_summary(root / "trust")
            trust = json.loads(trust_path.read_text(encoding="utf-8"))
            bundle = trust["bundles"][0]
            bundle["profile_id"] = custom_profile
            bundle["profile_overrides"]["id"] = custom_profile
            refresh_profile_json_sha256(trust)
            custom_trust_path = write_json(
                root / "custom-profile-trust.summary.json",
                digest_summary(trust),
            )

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(custom_trust_path),
                    "--allow-canary-stage-receipts-only",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(
                summary["canary_summaries"][0]["receipt_summary"]["receipts"][1][
                    "profile"
                ],
                custom_profile,
            )
            self.assertEqual(
                summary["trust_summaries"][0]["profiles"][0]["profile_id"],
                custom_profile,
            )

    def test_custom_canary_profile_id_without_trust_profile_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            custom_profile = "swift-cbpr-plus-alt"
            canary = valid_canary_summary()
            verify_stage = canary["stages"][2]
            receipt_summary = json.loads(verify_stage["stdout_preview"])
            receipt_summary["receipts"][1]["profile"] = custom_profile
            verify_stage["stdout_preview"] = (
                json.dumps(digest_receipt_summary(receipt_summary), sort_keys=True)
                + "\n"
            )
            canary_path = write_canary(root, digest_summary(canary))
            trust_path = write_trust_summary(root / "trust")

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-canary-stage-receipts-only",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "canary_summaries[0].receipt_summary.receipts[1].profile "
                "has no matching trust profile coverage for canary environment",
                stderr,
            )
            self.assertNotIn(custom_profile, stderr)
            self.assertNotIn("'preprod'", stderr)

    def test_default_canary_profile_requires_explicit_trust_binding(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary = valid_canary_summary(allow_default_profile=True)
            canary["stages"][0]["command"].append("--allow-default-profile")
            canary["stages"][2]["command"].append("--allow-default-profile")
            receipt_summary = json.loads(canary["stages"][2]["stdout_preview"])
            receipt_summary["receipts"][1]["profile"] = None
            canary["stages"][2]["stdout_preview"] = (
                json.dumps(digest_receipt_summary(receipt_summary), sort_keys=True)
                + "\n"
            )
            canary.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(canary))
            trust_path = write_trust_summary(root / "trust")
            base_argv = [
                "--canary-summary",
                str(canary_path),
                "--trust-summary",
                str(trust_path),
                "--allow-canary-stage-receipts-only",
                "--allow-default-profile",
            ]

            rc, stdout, stderr = run_evidence(base_argv)

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("without --default-rail-profile", stderr)

            rc, stdout, stderr = run_evidence(
                base_argv + ["--default-rail-profile", "fedwire-funds"]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "canary_summaries[0].receipt_summary.receipts[1].profile "
                "has no matching trust profile coverage for canary environment",
                stderr,
            )
            self.assertNotIn("fedwire-funds", stderr)

            rc, stdout, stderr = run_evidence(
                base_argv + ["--default-rail-profile", "swift-cbpr-plus"]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(
                summary["policy"]["default_rail_profile"],
                "swift-cbpr-plus",
            )

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
            self.assertFalse(receipt_summary["allow_default_profile"])
            self.assertTrue(receipt_summary["require_source_files"])
            self.assertEqual(len(receipt_summary["receipts"]), 2)
            body = dict(receipt_summary)
            digest = body.pop("summary_sha256")
            self.assertEqual(digest, EVIDENCE.sha256_hex(EVIDENCE._canonical_json_bytes(body)))

    def test_direct_receipt_archive_rejects_template_receipt_endpoints(self):
        cases = (
            (
                "notary",
                "endpoint",
                "https://notary.swift-cbpr-plus.operator-canary.bank/anchor",
            ),
            (
                "rail",
                "endpoint_url",
                "https://rail.swift-cbpr-plus.operator-canary.bank/v1/iso20022",
            ),
        )
        for kind, field, endpoint in cases:
            with self.subTest(kind=kind):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    notary_receipts, rail_receipts = write_https_receipt_dirs(root)
                    receipt_dir = notary_receipts if kind == "notary" else rail_receipts
                    receipt_path = next(Path(receipt_dir).glob("*.receipt.json"))
                    receipt_test.rewrite_receipt(
                        receipt_path,
                        lambda body, field=field, endpoint=endpoint: body.update(
                            {
                                field: endpoint,
                                "endpoint_sha256": EVIDENCE.sha256_hex(
                                    endpoint.encode("utf-8")
                                ),
                            }
                        ),
                    )
                    canary_path = write_canary(
                        root,
                        valid_canary_summary(
                            receipt_entries=receipt_entries_from_dirs(
                                notary_receipts, rail_receipts
                            )
                        ),
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
                    self.assertIn("receipt verification failed", stderr)
                    self.assertIn("template canary hostnames", stderr)
                    self.assertNotIn(endpoint, stderr)

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

    def test_canary_stage_branches_are_mutually_exclusive(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            cases = (
                (
                    "executed-with-planned",
                    lambda: {
                        **valid_canary_summary(),
                        "planned_stages": plan_only_canary_summary()["planned_stages"],
                    },
                    "planned_stages must be omitted for executed evidence",
                    [],
                ),
                (
                    "plan-only-with-executed",
                    lambda: {
                        **plan_only_canary_summary(),
                        "stages": valid_canary_summary()["stages"],
                    },
                    "stages must be omitted for plan-only evidence",
                    ["--allow-plan-only", "--allow-canary-stage-receipts-only"],
                ),
            )
            for name, build_body, message, extra_args in cases:
                with self.subTest(name=name):
                    body = build_body()
                    digest_summary(body)
                    canary_path = write_json(root / f"{name}.summary.json", body)

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                            *extra_args,
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

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

    def test_canary_receipt_source_path_rejects_checked_in_iso_fixtures(self):
        checked_in_fixture = REPO_ROOT / "fixtures" / "iso20022" / "pacs008_fixture.xml"
        cases = (
            "fixtures/iso20022/pacs008_fixture.xml",
            str(checked_in_fixture),
            "/ops/release/fixtures/iso20022/pacs008_fixture.xml",
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            for offset, source_path in enumerate(cases):
                with self.subTest(source_path=source_path):
                    body = valid_canary_summary()
                    receipt_summary = json.loads(body["stages"][2]["stdout_preview"])
                    receipt_summary["receipts"][1]["source_path"] = source_path
                    body["stages"][2]["stdout_preview"] = (
                        json.dumps(
                            digest_receipt_summary(receipt_summary),
                            sort_keys=True,
                        )
                        + "\n"
                    )
                    body.pop("summary_sha256")
                    canary_path = write_canary(
                        root,
                        digest_summary(body),
                    )

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("checked-in ISO XML fixtures", stderr)

    def test_canary_notary_receipt_anchor_path_is_required_and_digest_bound(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            cases = (
                (
                    "missing",
                    lambda receipt: receipt.pop("anchor_path"),
                    "anchor_path must be a non-empty string",
                ),
                (
                    "wrong-digest-addressed",
                    lambda receipt: receipt.__setitem__(
                        "anchor_path",
                        f"/ops/iso/notary/anchors/{'f' * 64}.notary.json",
                    ),
                    "anchor_path must be latest.notary.json or anchors/<index_sha256>.notary.json",
                ),
                (
                    "wrong-leaf",
                    lambda receipt: receipt.__setitem__(
                        "anchor_path",
                        "/ops/iso/notary/current.notary.json",
                    ),
                    "anchor_path must be latest.notary.json or anchors/<index_sha256>.notary.json",
                ),
                (
                    "repository-fixture",
                    lambda receipt: receipt.__setitem__(
                        "anchor_path",
                        "/ops/release/fixtures/iso20022/latest.notary.json",
                    ),
                    "anchor_path must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "missing-store-dir",
                    lambda receipt: receipt.pop("store_dir"),
                    "store_dir must be recorded",
                ),
                (
                    "missing-index-path",
                    lambda receipt: receipt.pop("index_path"),
                    "index_path must be recorded",
                ),
                (
                    "repository-store-dir",
                    lambda receipt: receipt.__setitem__(
                        "store_dir",
                        "/ops/release/fixtures/iso20022/notary-store",
                    ),
                    "store_dir must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "repository-index-path",
                    lambda receipt: receipt.__setitem__(
                        "index_path",
                        "/ops/release/fixtures/iso20022/notary/messages.index.json",
                    ),
                    "index_path must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "wrong-index-peer",
                    lambda receipt: receipt.__setitem__(
                        "index_path",
                        "/ops/iso/notary/anchors/messages.index.json",
                    ),
                    "index_path must be the messages.index.json peer of anchor_path",
                ),
            )
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    body = valid_canary_summary()
                    receipt_summary = json.loads(body["stages"][2]["stdout_preview"])
                    mutate(receipt_summary["receipts"][0])
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
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_direct_receipt_archive_must_bind_canary_receipt_kinds(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            notary_metadata_keys = {
                "anchor_path",
                "store_dir",
                "index_path",
                "anchor_sha256",
                "index_sha256",
                "record_count",
            }
            rail_metadata_keys = {
                "message_type",
                "payload_sha256",
                "profile",
                "rail_message_id",
                "source_path",
            }
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
            self.assertIn(
                "a receipt kind that does not match canary receipt kind",
                stderr,
            )
            self.assertNotIn("receipt_kind 'iso-rail-gateway'", stderr)
            self.assertNotIn("receipt_kind 'iso-audit-notary'", stderr)

    def test_direct_receipt_archive_must_bind_canary_receipt_filenames(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            entries[0]["path"] = "/ops/iso/receipts/relabelled-notary.receipt.json"
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
            self.assertIn(
                "a receipt filename that does not match canary receipt filename",
                stderr,
            )
            self.assertNotIn("relabelled-notary.receipt.json", stderr)

    def test_direct_receipt_archive_must_bind_canary_receipt_metadata(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            entries[0]["status_code"] = 200
            entries[0]["response_body_sha256"] = "f" * 64
            entries[0]["anchor_path"] = (
                f"/ops/iso/other-notary/anchors/{entries[0]['index_sha256']}.notary.json"
            )
            entries[0]["store_dir"] = "/ops/iso/other-notary-store"
            entries[0]["index_path"] = "/ops/iso/other-notary/messages.index.json"
            entries[0]["record_count"] = 2
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
            self.assertIn("metadata that does not match canary receipt metadata", stderr)
            self.assertNotIn("sepa-sct-inst", stderr)

    def test_direct_receipt_archive_must_bind_canary_endpoint_policy_evidence(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            notary_receipt = next(Path(notary_receipts).glob("*.receipt.json"))
            insecure_endpoint = "http://notary.local-bank.bank/iso-anchor"
            receipt_test.rewrite_receipt(
                notary_receipt,
                lambda body: body.update(
                    {
                        "endpoint": insecure_endpoint,
                        "endpoint_sha256": EVIDENCE.sha256_hex(
                            insecure_endpoint.encode("utf-8")
                        ),
                    }
                ),
            )
            entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            entries[0]["endpoint_requires_insecure_http"] = False
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
                    "--allow-insecure-http",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("direct receipt archive verification binds", stderr)
            self.assertIn("metadata that does not match canary receipt metadata", stderr)
            self.assertNotIn("endpoint_requires_insecure_http", stderr)

    def test_notary_receipt_record_count_must_be_positive_for_production_evidence(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            entries[0]["record_count"] = 0
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
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("record_count must be a positive integer", stderr)

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
            duplicate_path = entries[0]["path"]
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
            self.assertNotIn(duplicate_path, stderr)

            entries_two = [dict(entry) for entry in entries]
            duplicate_digest = entries_two[0]["receipt_sha256"]
            for offset, entry in enumerate(entries_two):
                entry["path"] = f"/ops/iso/other-receipts/{offset}.receipt.json"
            body_two = valid_canary_summary(receipt_entries=entries_two)
            body_two["config_path"] = "/ops/iso/canary-two.json"
            body_two.pop("summary_sha256")
            canary_two = write_json(
                root / "canary-two-digest.summary.json",
                digest_summary(body_two),
            )

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
            self.assertIn("receipt_summary.receipts[0].receipt_sha256 duplicates", stderr)
            self.assertNotIn(duplicate_digest, stderr)

    def test_canary_source_material_cannot_be_reused_across_relabelled_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(root)
            entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            canary_one = write_json(
                root / "canary-one.summary.json",
                valid_canary_summary(receipt_entries=entries),
            )
            entries_two = [dict(entry) for entry in entries]
            for offset, entry in enumerate(entries_two):
                entry["path"] = (
                    f"/ops/iso/relabelled-canary/{entry['receipt_kind']}.{offset}.receipt.json"
                )
                entry["receipt_sha256"] = f"{offset + 8:064x}"
            body_two = valid_canary_summary(receipt_entries=entries_two)
            body_two["config_path"] = "/ops/iso/canary-two.json"
            body_two.pop("summary_sha256")
            canary_two = write_json(
                root / "canary-two-source-replay.summary.json",
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
                    "--allow-canary-stage-receipts-only",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("receipt_summary.receipts[0].anchor_path duplicates", stderr)
            self.assertNotIn("latest.notary.json", stderr)
            self.assertNotIn("rail-status.xml", stderr)

    def test_cross_canary_source_material_replay_rejects_each_compact_field(self):
        source_fields = (
            "source_path",
            "payload_sha256",
            "anchor_path",
            "anchor_sha256",
            "store_dir",
            "index_path",
            "index_sha256",
        )
        for field in source_fields:
            with self.subTest(field=field):
                canaries = [
                    {
                        "receipt_summary": {
                            "receipts": [
                                {
                                    "path": "/ops/iso/receipts/one.receipt.json",
                                    "receipt_sha256": "1" * 64,
                                    field: "replayed-source-material",
                                }
                            ],
                        },
                    },
                    {
                        "receipt_summary": {
                            "receipts": [
                                {
                                    "path": "/ops/iso/receipts/two.receipt.json",
                                    "receipt_sha256": "2" * 64,
                                    field: "replayed-source-material",
                                }
                            ],
                        },
                    },
                ]

                with self.assertRaisesRegex(
                    EVIDENCE.EvidenceError,
                    rf"receipt_summary\.receipts\[0\]\.{field} duplicates",
                ) as context:
                    EVIDENCE._reject_cross_canary_receipt_reuse(canaries)

                self.assertNotIn("replayed-source-material", str(context.exception))

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
            receipt_entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            body = valid_canary_summary(receipt_entries=receipt_entries)
            body["stages"][0]["command"].append("--allow-legacy-colr007")
            body["stages"][2]["command"].append("--allow-legacy-colr007")
            body["stages"][2]["stdout_preview"] = receipt_stdout(
                receipt_entries=receipt_entries,
                allow_legacy_colr007=True,
            )
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))
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
            self.assertIn("--allow-legacy-colr007", stderr)

            rc, stdout, stderr = run_evidence(argv + ["--allow-legacy-colr007"])

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["policy"]["allow_legacy_colr007"])
            self.assertTrue(summary["receipt_verification"]["allow_legacy_colr007"])

    def test_default_profile_archive_receipts_require_explicit_local_override(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_receipts, rail_receipts = write_https_receipt_dirs(
                root,
                default_profile=True,
            )
            receipt_entries = receipt_entries_from_dirs(notary_receipts, rail_receipts)
            body = valid_canary_summary(
                receipt_entries=receipt_entries,
                allow_default_profile=True,
            )
            body["stages"][0]["command"].append("--allow-default-profile")
            body["stages"][2]["command"].append("--allow-default-profile")
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))
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
            self.assertIn("--allow-default-profile", stderr)

            rc, stdout, stderr = run_evidence(argv + ["--allow-default-profile"])

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("without --default-rail-profile", stderr)

            rc, stdout, stderr = run_evidence(
                argv
                + [
                    "--allow-default-profile",
                    "--default-rail-profile",
                    "swift-cbpr-plus",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["policy"]["allow_default_profile"])
            self.assertEqual(
                summary["policy"]["default_rail_profile"],
                "swift-cbpr-plus",
            )
            self.assertTrue(summary["receipt_verification"]["allow_default_profile"])
            self.assertTrue(
                summary["canary_summaries"][0]["receipt_summary"]["allow_default_profile"]
            )
            self.assertIsNone(
                summary["canary_summaries"][0]["receipt_summary"]["receipts"][1][
                    "profile"
                ]
            )

            missing_profile_canary = json.loads(canary_path.read_text(encoding="utf-8"))
            receipt_summary = missing_profile_canary["stages"][2]["stdout_preview"]
            receipt_summary = json.loads(receipt_summary)
            receipt_summary["receipts"][1].pop("profile")
            digest_receipt_summary(receipt_summary)
            missing_profile_canary["stages"][2]["stdout_preview"] = (
                json.dumps(receipt_summary, indent=2, sort_keys=True) + "\n"
            )
            digest_summary(missing_profile_canary)
            missing_profile_path = write_json(
                root / "missing-profile-canary.summary.json",
                missing_profile_canary,
            )

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(missing_profile_path),
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
                    "--allow-default-profile",
                    "--default-rail-profile",
                    "swift-cbpr-plus",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("profile must be recorded", stderr)

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

    def test_rejected_trust_source_url_does_not_echo_secret_query(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root)
            secret_url = "https://pki.example/source?token=evidence-source-secret"
            rewrite_trust_summary(
                trust_path,
                lambda body: body["bundles"][0]["source"].update({"url": secret_url}),
            )

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("source.url", stderr)
            self.assertNotIn(secret_url, stderr)
            self.assertNotIn("token=", stderr)
            self.assertNotIn("evidence-source-secret", stderr)

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

    def test_overlong_archive_timestamps_are_rejected_without_echo(self):
        hidden = "2" * (EVIDENCE.MAX_TIMESTAMP_CHARS + 1)
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            cases = (
                (
                    "canary-started-at",
                    lambda canary, _trust_path: canary.__setitem__("started_at", hidden),
                    "started_at must be no longer than 128 characters",
                ),
                (
                    "canary-stage-finished-at",
                    lambda canary, _trust_path: canary["stages"][0].__setitem__(
                        "finished_at",
                        hidden,
                    ),
                    "finished_at must be no longer than 128 characters",
                ),
                (
                    "trust-verified-at",
                    lambda _canary, trust_path: rewrite_trust_summary(
                        trust_path,
                        lambda body: body.__setitem__("verified_at", hidden),
                    ),
                    "verified_at must be no longer than 128 characters",
                ),
                (
                    "trust-source-retrieved-at",
                    lambda _canary, trust_path: rewrite_trust_summary(
                        trust_path,
                        lambda body: body["bundles"][0]["source"].__setitem__(
                            "retrieved_at",
                            hidden,
                        ),
                    ),
                    "source.retrieved_at must be no longer than 128 characters",
                ),
            )
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    canary = valid_canary_summary()
                    trust_path = write_trust_summary(root / name)
                    mutate(canary, trust_path)
                    canary_path = write_canary(root, digest_summary(canary))

                    rc, stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

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

    def test_input_summary_digest_rejects_all_zero_placeholder(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            canary = valid_canary_summary()
            actual_canary_digest = canary["summary_sha256"]
            canary["summary_sha256"] = "0" * 64
            canary_path = write_canary(root, canary)

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("summary_sha256 must not be all zero", stderr)
            self.assertNotIn(actual_canary_digest, stderr)
            self.assertNotIn("mismatch", stderr)

            canary_path = write_canary(root, valid_canary_summary())
            trust = json.loads(trust_path.read_text(encoding="utf-8"))
            actual_trust_digest = trust["summary_sha256"]
            trust["summary_sha256"] = "0" * 64
            zero_trust_path = write_json(root / "zero-trust.summary.json", trust)

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(zero_trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("summary_sha256 must not be all zero", stderr)
            self.assertNotIn(actual_trust_digest, stderr)
            self.assertNotIn("mismatch", stderr)

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
            notary_receipts, rail_receipts = write_https_receipt_dirs(
                root,
                legacy_colr007=True,
            )
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

    def test_insecure_command_urls_require_matching_child_flag(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = (
                (
                    "rail",
                    0,
                    "--torii-base-url",
                    "http://torii.local-bank.bank/iso",
                ),
                (
                    "notary",
                    1,
                    "--endpoint",
                    "http://notary.local-bank.bank/iso-anchor",
                ),
            )
            for name, stage_index, flag, url in cases:
                with self.subTest(name=name):
                    body = valid_canary_summary()
                    command = body["stages"][stage_index]["command"]
                    command[command.index(flag) + 1] = url
                    body.pop("summary_sha256")
                    canary_path = write_canary(root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                            "--allow-canary-stage-receipts-only",
                            "--allow-insecure-http",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("command URL requires --allow-insecure-http", stderr)

    def test_insecure_command_urls_require_matching_receipt_endpoint_evidence(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = (
                (
                    "rail",
                    0,
                    "--torii-base-url",
                    "http://torii.local-bank.bank/iso",
                    "iso-rail-gateway",
                ),
                (
                    "notary",
                    1,
                    "--endpoint",
                    "http://notary.local-bank.bank/iso-anchor",
                    "iso-audit-notary",
                ),
            )
            for name, stage_index, flag, url, receipt_kind in cases:
                with self.subTest(name=name):
                    body = valid_canary_summary()
                    command = body["stages"][stage_index]["command"]
                    command[command.index(flag) + 1] = url
                    command.append("--allow-insecure-http")
                    body.pop("summary_sha256")
                    canary_path = write_canary(root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                            "--allow-canary-stage-receipts-only",
                            "--allow-insecure-http",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("missing endpoint_requires_insecure_http", stderr)
                    self.assertIn(receipt_kind, stderr)

    def test_insecure_command_url_accepts_matching_receipt_endpoint_evidence(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            body = valid_canary_summary()
            rail_command = body["stages"][0]["command"]
            rail_command[rail_command.index("--torii-base-url") + 1] = (
                "http://torii.local-bank.bank/iso"
            )
            rail_command.append("--allow-insecure-http")
            body["stages"][2]["command"].append("--allow-insecure-http")
            receipt_summary = json.loads(receipt_stdout(allow_insecure_http=True))
            receipt_summary["receipts"][1]["endpoint_requires_insecure_http"] = True
            body["stages"][2]["stdout_preview"] = (
                json.dumps(
                    digest_receipt_summary(receipt_summary),
                    sort_keys=True,
                )
                + "\n"
            )
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-canary-stage-receipts-only",
                    "--allow-insecure-http",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            rail_receipt = summary["canary_summaries"][0]["receipt_summary"][
                "receipts"
            ][1]
            self.assertTrue(rail_receipt["endpoint_requires_insecure_http"])

    def test_rail_policy_receipts_require_matching_rail_command_flag(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = (
                (
                    "default-profile",
                    {"default_profile": True},
                    "--allow-default-profile",
                    {"allow_default_profile": True},
                    "rail command omitted --allow-default-profile",
                ),
                (
                    "legacy-colr007",
                    {"legacy_colr007": True},
                    "--allow-legacy-colr007",
                    {"allow_legacy_colr007": True},
                    "rail command omitted --allow-legacy-colr007",
                ),
            )
            for name, receipt_kwargs, flag, stdout_kwargs, message in cases:
                with self.subTest(name=name):
                    case_root = root / name
                    case_root.mkdir()
                    notary_receipts, rail_receipts = write_https_receipt_dirs(
                        case_root,
                        **receipt_kwargs,
                    )
                    receipt_entries = receipt_entries_from_dirs(
                        notary_receipts,
                        rail_receipts,
                    )
                    body = valid_canary_summary(
                        receipt_entries=receipt_entries,
                        allow_default_profile=stdout_kwargs.get(
                            "allow_default_profile",
                            False,
                        ),
                    )
                    if stdout_kwargs.get("allow_legacy_colr007", False):
                        body["stages"][2]["stdout_preview"] = receipt_stdout(
                            allow_legacy_colr007=True,
                            receipt_entries=receipt_entries,
                        )
                    body["stages"][2]["command"].append(flag)
                    body.pop("summary_sha256")
                    canary_path = write_canary(case_root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                            "--allow-canary-stage-receipts-only",
                            flag,
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_rail_command_policy_requires_matching_receipt_policy_evidence(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = (
                (
                    "default-profile",
                    "--allow-default-profile",
                    "rail command used --allow-default-profile but "
                    "receipt_summary has no default-profile rail receipt",
                ),
                (
                    "legacy-colr007",
                    "--allow-legacy-colr007",
                    "rail command used --allow-legacy-colr007 but "
                    "receipt_summary has no legacy colr.007 rail receipt",
                ),
            )
            for name, flag, message in cases:
                with self.subTest(name=name):
                    case_root = root / name
                    case_root.mkdir()
                    body = valid_canary_summary()
                    body["stages"][0]["command"].append(flag)
                    body.pop("summary_sha256")
                    canary_path = write_canary(case_root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                            "--allow-canary-stage-receipts-only",
                            flag,
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_rail_command_policy_accepts_matching_receipt_policy_evidence(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = (
                (
                    "default-profile",
                    {"default_profile": True},
                    "--allow-default-profile",
                    {"allow_default_profile": True},
                ),
                (
                    "legacy-colr007",
                    {"legacy_colr007": True},
                    "--allow-legacy-colr007",
                    {"allow_legacy_colr007": True},
                ),
            )
            for name, receipt_kwargs, flag, stdout_kwargs in cases:
                with self.subTest(name=name):
                    case_root = root / name
                    case_root.mkdir()
                    notary_receipts, rail_receipts = write_https_receipt_dirs(
                        case_root,
                        **receipt_kwargs,
                    )
                    receipt_entries = receipt_entries_from_dirs(
                        notary_receipts,
                        rail_receipts,
                    )
                    body = valid_canary_summary(
                        receipt_entries=receipt_entries,
                        allow_default_profile=stdout_kwargs.get(
                            "allow_default_profile",
                            False,
                        ),
                    )
                    if stdout_kwargs.get("allow_legacy_colr007", False):
                        body["stages"][2]["stdout_preview"] = receipt_stdout(
                            allow_legacy_colr007=True,
                            receipt_entries=receipt_entries,
                        )
                    body["stages"][0]["command"].append(flag)
                    body["stages"][2]["command"].append(flag)
                    body.pop("summary_sha256")
                    canary_path = write_canary(case_root, digest_summary(body))

                    rc, stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                            "--allow-canary-stage-receipts-only",
                            flag,
                        ]
                        + (
                            ["--default-rail-profile", "swift-cbpr-plus"]
                            if flag == "--allow-default-profile"
                            else []
                        )
                    )

                    self.assertEqual(rc, 0, stderr)
                    summary = json.loads(stdout)
                    receipt_summary = summary["canary_summaries"][0][
                        "receipt_summary"
                    ]
                    rail_receipt = next(
                        receipt
                        for receipt in receipt_summary["receipts"]
                        if receipt["receipt_kind"] == "iso-rail-gateway"
                    )
                    if flag == "--allow-default-profile":
                        self.assertEqual(
                            summary["policy"]["default_rail_profile"],
                            "swift-cbpr-plus",
                        )
                        self.assertTrue(receipt_summary["allow_default_profile"])
                        self.assertIsNone(rail_receipt["profile"])
                    else:
                        self.assertTrue(receipt_summary["allow_legacy_colr007"])
                        self.assertEqual(rail_receipt["message_type"], "colr.007")

    def test_archived_command_url_rejection_does_not_echo_secret_query(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            canary = valid_canary_summary()
            secret_url = "https://torii.example.invalid?token=evidence-command-secret"
            canary["stages"][0]["command"][5] = secret_url
            canary.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(canary))

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("secret-looking material", stderr)
            self.assertNotIn(secret_url, stderr)
            self.assertNotIn("token=", stderr)
            self.assertNotIn("evidence-command-secret", stderr)

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

    def test_secret_or_non_ascii_unsupported_child_command_flags_do_not_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            secret_flag = "--token-evidence-command-secret"
            secret = valid_canary_summary()
            secret["stages"][0]["command"].append(secret_flag)
            secret.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(secret),
                    [],
                    "unsupported secret-looking flag",
                    secret_flag,
                    "evidence-command-secret",
                )
            )
            encoded_secret_flag = "--%70assword%253Devidence-command-secret"
            encoded_secret = valid_canary_summary()
            encoded_secret["stages"][1]["command"].append(encoded_secret_flag)
            encoded_secret.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(encoded_secret),
                    [],
                    "contains secret-looking material",
                    encoded_secret_flag,
                    "evidence-command-secret",
                )
            )
            non_ascii_flag = "--summ\u0430ry-out"
            non_ascii = plan_only_canary_summary()
            non_ascii["planned_stages"][0]["command"].append(non_ascii_flag)
            non_ascii.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(non_ascii),
                    ["--allow-plan-only"],
                    "uses unsupported flag",
                    non_ascii_flag,
                    "summary-out",
                )
            )
            for body, extra_args, message, hidden, hidden_detail in cases:
                with self.subTest(message=message, extra_args=extra_args):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn(hidden_detail, stderr)

    def test_canary_child_commands_require_runner_command_shape(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            non_python_launcher = valid_canary_summary()
            non_python_launcher["stages"][0]["command"][0] = "/usr/bin/env"
            non_python_launcher.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(non_python_launcher),
                    [],
                    "stages[0].command[0] must be a Python interpreter path",
                    "/usr/bin/env",
                )
            )
            script_uri_prefix = valid_canary_summary()
            script_uri_prefix["stages"][0]["command"][1] = (
                "file:/ops/iso_rail_gateway_adapter.py"
            )
            script_uri_prefix.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(script_uri_prefix),
                    [],
                    "stages[0].command[1] must not contain URI or drive prefixes",
                    "file:/ops/iso_rail_gateway_adapter.py",
                )
            )
            script_encoded_separator = plan_only_canary_summary()
            script_encoded_separator["planned_stages"][1]["command"][1] = (
                "/ops/%2f/iso_audit_notary_adapter.py"
            )
            script_encoded_separator.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(script_encoded_separator),
                    ["--allow-plan-only"],
                    (
                        "planned_stages[1].command[1] must not contain encoded dot "
                        "or separator characters"
                    ),
                    "/ops/%2f/iso_audit_notary_adapter.py",
                )
            )
            wrong_script = valid_canary_summary()
            wrong_script["stages"][2]["command"][1] = "/ops/iso/not_the_receipt_verifier.py"
            wrong_script.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(wrong_script),
                    [],
                    "stages[2].command does not invoke iso_operator_receipt_verify.py",
                    "/ops/iso/not_the_receipt_verifier.py",
                )
            )
            extra_positional = valid_canary_summary()
            extra_positional["stages"][2]["command"].insert(2, "/ops/iso/manual.json")
            extra_positional.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(extra_positional),
                    [],
                    "stages[2].command[2] uses unsupported positional argument",
                    "/ops/iso/manual.json",
                )
            )
            missing_script = plan_only_canary_summary()
            missing_script["planned_stages"][2]["command"] = [sys.executable]
            missing_script.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(missing_script),
                    ["--allow-plan-only"],
                    (
                        "planned_stages[2].command must start with a Python "
                        "interpreter and iso_operator_receipt_verify.py"
                    ),
                    None,
                )
            )
            for body, extra_args, message, hidden in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    if hidden is not None and (
                        "must not contain" in message or "unsupported positional" in message
                    ):
                        self.assertNotIn(hidden, stderr)

    def test_canary_child_command_interpreter_rejects_unicode_digits_without_echo(self):
        hidden = "python\u0663"
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            body = valid_canary_summary()
            body["stages"][0]["command"][0] = f"/usr/local/bin/{hidden}"
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("stages[0].command[0] must use printable ASCII", stderr)
            self.assertNotIn(hidden, stderr)

    def test_canary_child_command_paths_reject_non_ascii_without_echo(self):
        hidden = "inb\u043ex"
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            executed = valid_canary_summary()
            executed["stages"][0]["command"][3] = f"/ops/iso/{hidden}"
            executed.pop("summary_sha256")
            planned = plan_only_canary_summary()
            planned["planned_stages"][2]["command"][3] = f"/ops/iso/{hidden}"
            planned.pop("summary_sha256")
            cases = (
                (
                    digest_summary(executed),
                    [],
                    "stages[0].command[3] must use printable ASCII",
                ),
                (
                    digest_summary(planned),
                    ["--allow-plan-only"],
                    "planned_stages[2].command[3] must use printable ASCII",
                ),
            )
            for body, extra_args, message in cases:
                with self.subTest(extra_args=extra_args):
                    canary_path = write_canary(root, body)

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
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_numeric_child_command_flags_reject_unicode_digits_without_echo(self):
        hidden = "\u0663.5"
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            executed = valid_canary_summary()
            executed["stages"][0]["command"].append(f"--timeout-secs={hidden}")
            executed.pop("summary_sha256")
            planned = plan_only_canary_summary()
            planned["planned_stages"][1]["command"].append(f"--timeout-secs={hidden}")
            planned.pop("summary_sha256")
            cases = (
                (digest_summary(executed), []),
                (digest_summary(planned), ["--allow-plan-only"]),
            )
            for body, extra_args in cases:
                with self.subTest(extra_args=extra_args):
                    canary_path = write_canary(root, body)

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
                    self.assertIn("--timeout-secs must be a positive finite number", stderr)
                    self.assertNotIn(hidden, stderr)

    def test_duplicate_singleton_child_command_flags_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            duplicate_rail_url = valid_canary_summary()
            duplicate_rail_url["stages"][0]["command"].extend(
                ["--torii-base-url", "https://torii-backup.local-bank.bank"]
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

    def test_boolean_child_command_flags_reject_values_without_echo(self):
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
            notary_all_separate_value = valid_canary_summary()
            notary_all_separate_value["stages"][1]["command"].extend(
                ["--all", "false"]
            )
            notary_all_separate_value.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(notary_all_separate_value),
                    [],
                    "stages[1].command[8] boolean flag --all must not use a value",
                )
            )
            verify_source_value = valid_canary_summary()
            verify_source_value["stages"][2]["command"][6] = "--require-source-files=false"
            verify_source_value.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(verify_source_value),
                    [],
                    "stages[2].command[6] boolean flag "
                    "--require-source-files must not use =value",
                )
            )
            verify_source_separate_value = valid_canary_summary()
            verify_source_separate_value["stages"][2]["command"].insert(7, "false")
            verify_source_separate_value.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(verify_source_separate_value),
                    [],
                    "stages[2].command[6] boolean flag "
                    "--require-source-files must not use a value",
                )
            )
            planned_notary_all_value = plan_only_canary_summary()
            planned_notary_all_value["planned_stages"][1]["command"].append("--all=false")
            planned_notary_all_value.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(planned_notary_all_value),
                    ["--allow-plan-only"],
                    "planned_stages[1].command[8] boolean flag "
                    "--all must not use =value",
                )
            )
            planned_notary_all_separate_value = plan_only_canary_summary()
            planned_notary_all_separate_value["planned_stages"][1]["command"].extend(
                ["--all", "false"]
            )
            planned_notary_all_separate_value.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(planned_notary_all_separate_value),
                    ["--allow-plan-only"],
                    "planned_stages[1].command[8] boolean flag "
                    "--all must not use a value",
                )
            )
            for body, extra_args, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

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
                    self.assertIn(message, stderr)
                    self.assertNotIn("false", stderr)

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

    def test_value_taking_child_command_flags_reject_flag_values(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = []
            rail_missing_url_value = valid_canary_summary()
            rail_missing_url_value["stages"][0]["command"][5] = "--receipt-dir"
            rail_missing_url_value.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(rail_missing_url_value),
                    [],
                    "stages[0].command has --torii-base-url without a value",
                )
            )
            notary_missing_endpoint_value = valid_canary_summary()
            notary_missing_endpoint_value["stages"][1]["command"][7] = "--all"
            notary_missing_endpoint_value.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(notary_missing_endpoint_value),
                    [],
                    "stages[1].command has --endpoint without a value",
                )
            )
            verify_missing_receipt_dir_value = valid_canary_summary()
            verify_missing_receipt_dir_value["stages"][2]["command"][3] = (
                "--require-source-files"
            )
            verify_missing_receipt_dir_value.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(verify_missing_receipt_dir_value),
                    [],
                    "stages[2].command has --receipt-dir without a value",
                )
            )
            planned_equals_form_flag_value = plan_only_canary_summary()
            planned_equals_form_flag_value["planned_stages"][0]["command"].append(
                "--message=--receipt-dir"
            )
            planned_equals_form_flag_value.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(planned_equals_form_flag_value),
                    ["--allow-plan-only"],
                    "planned_stages[0].command has --message without a value",
                )
            )
            rail_empty_url_equals = valid_canary_summary()
            command = rail_empty_url_equals["stages"][0]["command"]
            del command[4:6]
            command.insert(4, "--torii-base-url=")
            rail_empty_url_equals.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(rail_empty_url_equals),
                    [],
                    "stages[0].command has --torii-base-url without a value",
                )
            )
            notary_empty_endpoint_equals = valid_canary_summary()
            command = notary_empty_endpoint_equals["stages"][1]["command"]
            del command[6:8]
            command.insert(6, "--endpoint=")
            notary_empty_endpoint_equals.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(notary_empty_endpoint_equals),
                    [],
                    "stages[1].command has --endpoint without a value",
                )
            )
            planned_empty_message_equals = plan_only_canary_summary()
            planned_empty_message_equals["planned_stages"][0]["command"].append(
                "--message="
            )
            planned_empty_message_equals.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(planned_empty_message_equals),
                    ["--allow-plan-only"],
                    "planned_stages[0].command has --message without a value",
                )
            )
            verify_empty_receipt_dir_equals = valid_canary_summary()
            command = verify_empty_receipt_dir_equals["stages"][2]["command"]
            del command[2:4]
            command.insert(2, "--receipt-dir=")
            verify_empty_receipt_dir_equals.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(verify_empty_receipt_dir_equals),
                    [],
                    "stages[2].command has --receipt-dir without a value",
                )
            )
            bearer_token_flag_value = valid_canary_summary()
            bearer_token_flag_value["stages"][0]["command"][9] = "--receipt-dir"
            bearer_token_flag_value.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(bearer_token_flag_value),
                    [],
                    "stages[0] has --bearer-token-file without a value",
                )
            )
            bearer_token_equals_flag_value = valid_canary_summary()
            command = bearer_token_equals_flag_value["stages"][0]["command"]
            del command[8:10]
            command.insert(8, "--bearer-token-file=--receipt-dir")
            bearer_token_equals_flag_value.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(bearer_token_equals_flag_value),
                    [],
                    "stages[0] has --bearer-token-file without a value",
                )
            )
            bearer_token_empty_equals = valid_canary_summary()
            command = bearer_token_empty_equals["stages"][0]["command"]
            del command[8:10]
            command.insert(8, "--bearer-token-file=")
            bearer_token_empty_equals.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(bearer_token_empty_equals),
                    [],
                    "stages[0] has --bearer-token-file without a value",
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
            planned_rail_fixture_inbox = plan_only_canary_summary()
            planned_rail_fixture_inbox["planned_stages"][0]["command"][3] = (
                "/ops/release/fixtures/iso20022/rail-inbox"
            )
            planned_rail_fixture_inbox.pop("summary_sha256")
            planned_rail_fixture_message = plan_only_canary_summary()
            planned_rail_fixture_message["planned_stages"][0]["command"].extend(
                ["--message", "/ops/release/fixtures/iso20022/pacs002.xml"]
            )
            planned_rail_fixture_message.pop("summary_sha256")
            planned_notary_fixture_export = plan_only_canary_summary()
            planned_notary_fixture_export["planned_stages"][1]["command"][3] = (
                "/ops/release/fixtures/iso20022/notary-export"
            )
            planned_notary_fixture_export.pop("summary_sha256")
            planned_verify_fixture_receipt_dir = plan_only_canary_summary()
            planned_verify_fixture_receipt_dir["planned_stages"][2]["command"][3] = (
                "/ops/release/fixtures/iso20022/rail-receipts"
            )
            planned_verify_fixture_receipt_dir.pop("summary_sha256")
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
                    "stages[1].command has --export-dir without a value",
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
                    "planned_stages[0].command has --message without a value",
                ),
                (
                    digest_summary(planned_rail_fixture_inbox),
                    ["--allow-plan-only"],
                    "planned_stages[0].command[3] must not point to checked-in ISO fixture artifacts",
                ),
                (
                    digest_summary(planned_rail_fixture_message),
                    ["--allow-plan-only"],
                    "planned_stages[0].command[11] must not point to checked-in ISO XML fixtures",
                ),
                (
                    digest_summary(planned_notary_fixture_export),
                    ["--allow-plan-only"],
                    "planned_stages[1].command[3] must not point to checked-in ISO fixture artifacts",
                ),
                (
                    digest_summary(planned_verify_fixture_receipt_dir),
                    ["--allow-plan-only"],
                    "planned_stages[2].command[3] must not point to checked-in ISO fixture artifacts",
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
                    "stages[0].command has --receipt-dir without a value",
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

    def test_verify_stage_receipt_dir_must_be_null(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            executed = valid_canary_summary()
            executed["stages"][2]["receipt_dir"] = "/ops/iso/verify-receipts"
            executed.pop("summary_sha256")
            planned = plan_only_canary_summary()
            planned["planned_stages"][2]["receipt_dir"] = "/ops/iso/verify-receipts"
            planned.pop("summary_sha256")
            cases = (
                (
                    digest_summary(executed),
                    [],
                    "stages[2].receipt_dir must be null for verify stage",
                ),
                (
                    digest_summary(planned),
                    ["--allow-plan-only"],
                    "planned_stages[2].receipt_dir must be null for verify stage",
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

    def test_successful_stage_reason_must_be_null(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            body = valid_canary_summary()
            body["stages"][0]["reason"] = "skipped because an earlier stage failed"
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("stages[0].reason must be null for successful stage", stderr)

    def test_canary_stage_receipt_dirs_must_be_unique(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            executed = valid_canary_summary()
            executed["stages"][1]["receipt_dir"] = "/ops/iso/rail-receipts"
            executed["stages"][1]["command"][5] = "/ops/iso/rail-receipts"
            del executed["stages"][2]["command"][4:6]
            executed.pop("summary_sha256")
            planned = plan_only_canary_summary()
            planned["planned_stages"][1]["receipt_dir"] = "/ops/iso/rail-receipts"
            planned["planned_stages"][1]["command"][5] = "/ops/iso/rail-receipts"
            del planned["planned_stages"][2]["command"][4:6]
            planned.pop("summary_sha256")
            cases = (
                (
                    digest_summary(executed),
                    [],
                    "stages notary receipt_dir duplicates rail receipt_dir",
                ),
                (
                    digest_summary(planned),
                    ["--allow-plan-only"],
                    "planned_stages notary receipt_dir duplicates rail receipt_dir",
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
            planned_missing_rail = plan_only_canary_summary()
            del planned_missing_rail["planned_stages"][2]["command"][2:4]
            planned_missing_rail.pop("summary_sha256")
            planned_missing_notary = plan_only_canary_summary()
            del planned_missing_notary["planned_stages"][2]["command"][4:6]
            planned_missing_notary.pop("summary_sha256")
            traversal = valid_canary_summary()
            traversal["stages"][2]["command"][3] = "/ops/iso/../rail-receipts"
            traversal.pop("summary_sha256")
            cases = (
                (
                    digest_summary(missing_rail),
                    [],
                    "verify command does not include rail receipt_dir",
                ),
                (
                    digest_summary(missing_notary),
                    [],
                    "verify command does not include notary receipt_dir",
                ),
                (
                    digest_summary(planned_missing_rail),
                    ["--allow-plan-only"],
                    "planned_stages verify command does not include rail receipt_dir",
                ),
                (
                    digest_summary(planned_missing_notary),
                    ["--allow-plan-only"],
                    "planned_stages verify command does not include notary receipt_dir",
                ),
                (
                    digest_summary(traversal),
                    [],
                    "stages[2].command[3] must not contain dot or parent segments",
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

    def test_plan_only_verify_stage_allows_dry_run_without_receipt_dir(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            body = plan_only_canary_summary()
            body["planned_stages"][0]["dry_run"] = True
            body["planned_stages"][0]["command"].append("--dry-run")
            del body["planned_stages"][2]["command"][2:4]
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-plan-only",
                    "--allow-dry-run",
                    "--allow-canary-stage-receipts-only",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["policy"]["allow_dry_run"])

    def test_verify_stage_command_must_not_include_extra_receipt_dirs(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            executed = valid_canary_summary()
            executed["stages"][2]["command"].extend(
                ["--receipt-dir", "/ops/iso/extra-receipts"]
            )
            executed.pop("summary_sha256")
            partial = valid_canary_summary()
            partial["stages"] = [partial["stages"][0], partial["stages"][2]]
            partial.pop("summary_sha256")
            executed_receipt_file = valid_canary_summary()
            executed_receipt_file["stages"][2]["command"].extend(
                ["--receipt", "/ops/iso/extra-receipts/extra.receipt.json"]
            )
            executed_receipt_file.pop("summary_sha256")
            planned = plan_only_canary_summary()
            planned["planned_stages"][2]["command"].extend(
                ["--receipt-dir", "/ops/iso/extra-receipts"]
            )
            planned.pop("summary_sha256")
            planned_receipt_file = plan_only_canary_summary()
            planned_receipt_file["planned_stages"][2]["command"].extend(
                ["--receipt", "/ops/iso/extra-receipts/extra.receipt.json"]
            )
            planned_receipt_file.pop("summary_sha256")
            cases = (
                (
                    digest_summary(executed),
                    [],
                    "stages verify command includes receipt_dir for stages not present",
                ),
                (
                    digest_summary(partial),
                    ["--allow-partial-canary", "--allow-canary-stage-receipts-only"],
                    "stages verify command includes receipt_dir for stages not present",
                ),
                (
                    digest_summary(executed_receipt_file),
                    [],
                    "stages verify command includes receipt file for stages not present",
                ),
                (
                    digest_summary(planned),
                    ["--allow-plan-only"],
                    "planned_stages verify command includes receipt_dir for stages not present",
                ),
                (
                    digest_summary(planned_receipt_file),
                    ["--allow-plan-only"],
                    "planned_stages verify command includes receipt file for stages not present",
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

    def test_verify_stage_receipt_selectors_must_be_unique(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            duplicate_dir = valid_canary_summary()
            duplicate_dir["stages"][2]["command"].extend(
                ["--receipt-dir", "/ops/iso/rail-receipts"]
            )
            duplicate_dir.pop("summary_sha256")
            duplicate_file = valid_canary_summary()
            duplicate_file["stages"][2]["command"].extend(
                [
                    "--receipt",
                    "/ops/iso/manual/rail.receipt.json",
                    "--receipt",
                    "/ops/iso/manual/rail.receipt.json",
                ]
            )
            duplicate_file.pop("summary_sha256")
            covered_file = valid_canary_summary()
            covered_file["stages"][2]["command"].extend(
                ["--receipt", "/ops/iso/rail-receipts/rail.receipt.json"]
            )
            covered_file.pop("summary_sha256")
            planned_duplicate_dir = plan_only_canary_summary()
            planned_duplicate_dir["planned_stages"][2]["command"].extend(
                ["--receipt-dir", "/ops/iso/notary-receipts"]
            )
            planned_duplicate_dir.pop("summary_sha256")
            planned_covered_file = plan_only_canary_summary()
            planned_covered_file["planned_stages"][2]["command"].extend(
                ["--receipt", "/ops/iso/notary-receipts/notary.receipt.json"]
            )
            planned_covered_file.pop("summary_sha256")
            cases = (
                (
                    digest_summary(duplicate_dir),
                    [],
                    "stages[2].command[8] duplicates --receipt-dir at",
                ),
                (
                    digest_summary(duplicate_file),
                    [],
                    "stages[2].command[10] duplicates --receipt at",
                ),
                (
                    digest_summary(covered_file),
                    [],
                    "stages[2].command[8] --receipt is already covered by --receipt-dir",
                ),
                (
                    digest_summary(planned_duplicate_dir),
                    ["--allow-plan-only"],
                    "planned_stages[2].command[8] duplicates --receipt-dir at",
                ),
                (
                    digest_summary(planned_covered_file),
                    ["--allow-plan-only"],
                    "planned_stages[2].command[8] --receipt is already covered by "
                    "--receipt-dir",
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

    def test_executed_stage_receipt_kinds_must_match_receipt_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = (
                (
                    "rail-stage-notary-receipt",
                    (0, 2),
                    "notary",
                    "missing receipt kinds for executed stages: iso-rail-gateway",
                ),
                (
                    "notary-stage-rail-receipt",
                    (1, 2),
                    "rail",
                    "missing receipt kinds for executed stages: iso-audit-notary",
                ),
                (
                    "rail-stage-extra-notary-receipt",
                    (0, 2),
                    "both",
                    "receipt_summary contains receipt kinds for stages not executed: "
                    "iso-audit-notary",
                ),
            )
            for name, stage_indexes, receipt_scope, message in cases:
                with self.subTest(name=name):
                    case_root = root / name
                    case_root.mkdir()
                    notary_receipts, rail_receipts = write_https_receipt_dirs(case_root)
                    if receipt_scope == "notary":
                        receipt_entries = receipt_entries_from_dirs(notary_receipts)
                    elif receipt_scope == "rail":
                        receipt_entries = receipt_entries_from_dirs(rail_receipts)
                    else:
                        receipt_entries = receipt_entries_from_dirs(
                            notary_receipts,
                            rail_receipts,
                        )
                    body = valid_canary_summary(receipt_entries=receipt_entries)
                    body["stages"] = [body["stages"][index] for index in stage_indexes]
                    verify_command = body["stages"][-1]["command"]
                    if stage_indexes == (0, 2):
                        del verify_command[4:6]
                    elif stage_indexes == (1, 2):
                        del verify_command[2:4]
                    body.pop("summary_sha256")
                    canary_path = write_canary(case_root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                            "--allow-partial-canary",
                            "--allow-canary-stage-receipts-only",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

            dry_run_root = root / "dry-run-rail-with-receipts"
            dry_run_root.mkdir()
            notary_receipts, rail_receipts = write_https_receipt_dirs(dry_run_root)
            body = valid_canary_summary(
                receipt_entries=receipt_entries_from_dirs(
                    notary_receipts,
                    rail_receipts,
                )
            )
            body["stages"][0]["command"].append("--dry-run")
            body.pop("summary_sha256")
            canary_path = write_canary(dry_run_root, digest_summary(body))

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-dry-run",
                    "--allow-canary-stage-receipts-only",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn(
                "receipt_summary contains receipt kinds for stages not executed: "
                "iso-rail-gateway",
                stderr,
            )

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

    def test_plan_only_stage_dry_run_flag_must_match_command(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            recorded_true = plan_only_canary_summary()
            recorded_true["planned_stages"][0]["dry_run"] = True
            del recorded_true["planned_stages"][2]["command"][2:4]
            recorded_true.pop("summary_sha256")
            command_true = plan_only_canary_summary()
            command_true["planned_stages"][0]["command"].append("--dry-run")
            command_true.pop("summary_sha256")
            cases = (
                (
                    digest_summary(recorded_true),
                    "planned_stages[0].dry_run does not match command --dry-run",
                ),
                (
                    digest_summary(command_true),
                    "planned_stages[0].dry_run does not match command --dry-run",
                ),
            )
            for body, message in cases:
                with self.subTest(message=message):
                    canary_path = write_canary(root, body)

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                            "--allow-plan-only",
                            "--allow-dry-run",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_smuggled_canary_stage_command_urls_are_rejected(self):
        def rail_url(body, url):
            body["stages"][0]["command"][5] = url

        def notary_url(body, url):
            body["stages"][1]["command"][7] = url

        cases = [
            (rail_url, "https://user:pass@torii.local-bank.bank", []),
            (rail_url, "https://torii.example.invalid", []),
            (rail_url, "https://torii.example", []),
            (rail_url, "https://torii.example.com", []),
            (rail_url, "https://torii.example.net/base", []),
            (rail_url, "https://torii.example.org/base", []),
            (
                rail_url,
                "https://torii.swift-cbpr-plus.operator-canary.bank/base",
                [],
            ),
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
            (notary_url, "https://notary.example/anchor", []),
            (notary_url, "https://notary.example.com/anchor", []),
            (
                notary_url,
                "https://notary.swift-cbpr-plus.operator-canary.bank/anchor",
                [],
            ),
            (rail_url, "http://torii.example.invalid/base", ["--allow-insecure-http"]),
            (
                rail_url,
                "http://torii.swift-cbpr-plus.operator-canary.bank/base",
                ["--allow-insecure-http"],
            ),
            (
                notary_url,
                "http://notary.example.invalid/anchor",
                ["--allow-insecure-http"],
            ),
            (
                notary_url,
                "http://notary.swift-cbpr-plus.operator-canary.bank/anchor",
                ["--allow-insecure-http"],
            ),
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
            preview_identifier = valid_canary_summary()
            preview_identifier["stages"][0][
                "stdout_preview"
            ] = "accepted token-evidence-stage-secret"
            preview_identifier.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(preview_identifier),
                    "stdout_preview contains secret-looking material",
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

    def test_canary_and_trust_context_strings_reject_non_ascii_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            canary_cases = (
                (
                    "provider",
                    lambda body: body.__setitem__("provider", "local-b\u00e1nk"),
                    "provider must use printable ASCII",
                    "local-b\u00e1nk",
                ),
                (
                    "environment",
                    lambda body: body.__setitem__("environment", "prepr\u043ed"),
                    "environment must use printable ASCII",
                    "prepr\u043ed",
                ),
            )
            for field, mutate, message, hidden in canary_cases:
                with self.subTest(kind="canary", field=field):
                    body = valid_canary_summary()
                    mutate(body)
                    body.pop("summary_sha256")
                    canary_path = write_canary(root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

            canary_path = write_canary(root, valid_canary_summary())
            mutated_trust_path = write_trust_summary(root / "trust-nonascii")
            rewrite_trust_summary(
                mutated_trust_path,
                lambda summary: summary["bundles"][0].__setitem__(
                    "environment",
                    "prepr\u043ed",
                ),
            )

            rc, _stdout, stderr = run_evidence(
                ["--canary-summary", str(canary_path), "--trust-summary", str(mutated_trust_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("environment must use printable ASCII", stderr)
            self.assertNotIn("prepr\u043ed", stderr)

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

    def test_overlong_trust_profile_identity_values_are_rejected_without_echo(self):
        overlong_profile = "a" * 129
        overlong_policy = "require-verified-" + ("a" * 129)
        cases = (
            (
                "bundle-profile-id",
                lambda summary: summary["bundles"][0].__setitem__(
                    "profile_id",
                    overlong_profile,
                ),
                "profile_id must be no longer than 128 characters",
                overlong_profile,
                "profile_id must be a canonical lowercase profile id",
            ),
            (
                "override-profile-id",
                lambda summary: summary["bundles"][0]["profile_overrides"].__setitem__(
                    "id",
                    overlong_profile,
                ),
                "profile_overrides.id must be no longer than 128 characters",
                overlong_profile,
                "profile_overrides.id does not match profile_id",
            ),
            (
                "bundle-policy",
                lambda summary: summary["bundles"][0].__setitem__(
                    "embedded_signature_policy",
                    overlong_policy,
                ),
                "embedded_signature_policy must be no longer than 128 characters",
                overlong_policy,
                "embedded_signature_policy is unsupported",
            ),
            (
                "override-policy",
                lambda summary: summary["bundles"][0]["profile_overrides"].__setitem__(
                    "embedded_signature_policy",
                    overlong_policy,
                ),
                (
                    "profile_overrides.embedded_signature_policy must be no longer "
                    "than 128 characters"
                ),
                overlong_policy,
                "profile_overrides.embedded_signature_policy does not match",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root, valid_canary_summary())
            for name, mutate, message, hidden, bypassed_message in cases:
                with self.subTest(name=name):
                    trust_path = write_trust_summary(root / name)
                    rewrite_trust_summary(trust_path, mutate)

                    rc, stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                            "--allow-record-only-trust",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn(bypassed_message, stderr)

    def test_secret_looking_trust_identity_values_are_rejected_without_echo(self):
        cases = (
            (
                "rail",
                lambda summary, secret: summary["bundles"][0].__setitem__(
                    "rail",
                    secret,
                ),
            ),
            (
                "policy",
                lambda summary, secret: summary["bundles"][0].__setitem__(
                    "embedded_signature_policy",
                    secret,
                ),
            ),
            (
                "override-policy",
                lambda summary, secret: summary["bundles"][0][
                    "profile_overrides"
                ].__setitem__(
                    "embedded_signature_policy",
                    secret,
                ),
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root, valid_canary_summary())
            for name, mutate in cases:
                with self.subTest(name=name):
                    case_root = root / name
                    trust_path = write_trust_summary(case_root)
                    secret = f"token-evidence-trust-{name}-secret"
                    rewrite_trust_summary(
                        trust_path,
                        lambda summary: mutate(summary, secret),
                    )

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn(secret, stderr)

    def test_secret_looking_digest_and_oid_values_are_rejected_without_echo(self):
        cases = (
            (
                "bundle-sha",
                lambda summary, secret: summary["bundles"][0].__setitem__(
                    "bundle_sha256",
                    secret,
                ),
            ),
            (
                "public-pin",
                lambda summary, secret: summary["bundles"][0]["profile_overrides"].__setitem__(
                    "signature_public_key_sha256_pins",
                    [secret],
                ),
            ),
            (
                "policy-oid",
                lambda summary, secret: summary["bundles"][0]["profile_overrides"][
                    "x509_required_certificate_policy_oids"
                ].__setitem__(0, secret),
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root, valid_canary_summary())
            for name, mutate in cases:
                with self.subTest(name=name):
                    case_root = root / name
                    trust_path = write_trust_summary(case_root)
                    secret = f"token-evidence-{name}-secret"
                    rewrite_trust_summary(
                        trust_path,
                        lambda summary: mutate(summary, secret),
                    )

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn(secret, stderr)

    def test_secret_looking_canary_identity_values_are_rejected_without_echo(self):
        cases = (
            ("provider", "token-evidence-provider-secret"),
            ("environment", "private-key-evidence-environment-secret"),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            for field, secret in cases:
                with self.subTest(field=field):
                    case_root = root / field
                    case_root.mkdir()
                    canary = valid_canary_summary()
                    canary[field] = secret
                    canary_path = write_canary(case_root, digest_summary(canary))
                    trust_path = write_trust_summary(case_root / "trust")

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn(secret, stderr)

    def test_secret_or_non_ascii_receipt_kind_values_are_rejected_without_echo(self):
        cases = (
            (
                "receipt-kind-list",
                lambda summary, value: summary["receipt_kind"].__setitem__(0, value),
            ),
            (
                "receipt-entry-kind",
                lambda summary, value: summary["receipts"][0].__setitem__(
                    "receipt_kind",
                    value,
                ),
            ),
        )
        values = (
            ("secret", "token-evidence-receipt-kind-secret", "secret-looking material"),
            ("non-ascii", "iso-rail-gatew\u0430y", "must use printable ASCII"),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            for name, mutate in cases:
                for value_kind, value, message in values:
                    with self.subTest(name=name, value_kind=value_kind):
                        case_root = root / f"{name}-{value_kind}"
                        case_root.mkdir()
                        receipt_summary = json.loads(receipt_stdout())
                        mutate(receipt_summary, value)
                        canary = valid_canary_summary()
                        canary["stages"][2]["stdout_preview"] = (
                            json.dumps(
                                digest_receipt_summary(receipt_summary),
                                sort_keys=True,
                            )
                            + "\n"
                        )
                        canary.pop("summary_sha256")
                        canary_path = write_canary(case_root, digest_summary(canary))

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
                        self.assertNotIn(value, stderr)
                        if value_kind == "non-ascii":
                            self.assertNotIn("unsupported", stderr)

    def test_secret_or_non_ascii_canary_stage_names_are_rejected_without_echo(self):
        def executed_summary(value):
            body = valid_canary_summary()
            body["stages"][0]["name"] = value
            return digest_summary(body)

        def planned_summary(value):
            body = plan_only_canary_summary()
            body["planned_stages"][0]["name"] = value
            return digest_summary(body)

        cases = (
            (
                "executed-stage",
                executed_summary,
                [],
            ),
            (
                "planned-stage",
                planned_summary,
                ["--allow-plan-only"],
            ),
        )
        values = (
            ("secret", "token-evidence-stage-secret", "secret-looking material"),
            ("non-ascii", "ra\u0430l", "must use printable ASCII"),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root / "trust")
            for name, build_canary, extra_args in cases:
                for value_kind, value, message in values:
                    with self.subTest(name=name, value_kind=value_kind):
                        case_root = root / f"{name}-{value_kind}"
                        case_root.mkdir()
                        canary_path = write_canary(case_root, build_canary(value))

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
                        self.assertIn(message, stderr)
                        self.assertNotIn(value, stderr)
                        if value_kind == "non-ascii":
                            self.assertNotIn("unsupported canary stage", stderr)

    def test_secret_looking_trust_identity_values_are_rejected_without_echo(self):
        cases = (
            (
                "profile-id",
                "token-trust-profile-secret",
                lambda summary, secret: summary["bundles"][0].__setitem__(
                    "profile_id",
                    secret,
                ),
            ),
            (
                "environment",
                "token-trust-environment-secret",
                lambda summary, secret: summary["bundles"][0].__setitem__(
                    "environment",
                    secret,
                ),
            ),
            (
                "source-authority",
                "token-trust-authority-secret",
                lambda summary, secret: summary["bundles"][0]["source"].__setitem__(
                    "authority",
                    secret,
                ),
            ),
            (
                "source-version",
                "session-key-trust-version-secret",
                lambda summary, secret: summary["bundles"][0]["source"].__setitem__(
                    "version",
                    secret,
                ),
            ),
            (
                "der-label",
                "token-trust-label-secret",
                lambda summary, secret: summary["bundles"][0]["x509_trust_anchors"][
                    0
                ].__setitem__("label", secret),
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root, valid_canary_summary())
            for name, secret, mutate in cases:
                with self.subTest(name=name):
                    trust_path = write_trust_summary(root / name)
                    rewrite_trust_summary(
                        trust_path,
                        lambda summary, secret=secret, mutate=mutate: mutate(
                            summary,
                            secret,
                        ),
                    )

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn(secret, stderr)

    def test_non_ascii_trust_source_identity_values_are_rejected_without_echo(self):
        cases = (
            (
                "source-authority",
                "ISO\u2011MDR",
                lambda summary, value: summary["bundles"][0]["source"].__setitem__(
                    "authority",
                    value,
                ),
            ),
            (
                "source-version",
                "2026\u2011Q2",
                lambda summary, value: summary["bundles"][0]["source"].__setitem__(
                    "version",
                    value,
                ),
            ),
            (
                "der-label",
                "root\u2011a",
                lambda summary, value: summary["bundles"][0]["x509_trust_anchors"][
                    0
                ].__setitem__("label", value),
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root, valid_canary_summary())
            for name, hidden, mutate in cases:
                with self.subTest(name=name):
                    trust_path = write_trust_summary(root / name)
                    rewrite_trust_summary(
                        trust_path,
                        lambda summary, hidden=hidden, mutate=mutate: mutate(
                            summary,
                            hidden,
                        ),
                    )

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("must use printable ASCII", stderr)
                    self.assertNotIn(hidden, stderr)

    def test_overlong_trust_source_identity_values_are_rejected_without_echo(self):
        hidden = "A" * (EVIDENCE.MAX_TRUST_SOURCE_TEXT_CHARS + 1)
        cases = (
            (
                "source-authority",
                lambda summary: summary["bundles"][0]["source"].__setitem__(
                    "authority",
                    hidden,
                ),
                "source.authority must be no longer than 256 characters",
            ),
            (
                "source-version",
                lambda summary: summary["bundles"][0]["source"].__setitem__(
                    "version",
                    hidden,
                ),
                "source.version must be no longer than 256 characters",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root, valid_canary_summary())
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    trust_path = write_trust_summary(root / name)
                    rewrite_trust_summary(trust_path, mutate)

                    rc, stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

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
                    "dummy-authority",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "authority",
                        "Dummy Swift operator PKI",
                    ),
                    "source.authority must not contain placeholder production metadata",
                ),
                (
                    "fake-version",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "version",
                        "fake-v1",
                    ),
                    "source.version must not contain placeholder production metadata",
                ),
                (
                    "sample-authority",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "authority",
                        "Sample Swift operator PKI",
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
                    "template-version",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "version",
                        "template-v1",
                    ),
                    "source.version must not contain placeholder production metadata",
                ),
                (
                    "url",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "url",
                        "https://pki.swift.example.invalid/iso20022",
                    ),
                    "source.url must not use reserved placeholder hostnames",
                ),
                (
                    "reserved-url",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "url",
                        "https://pki.swift.example.com/iso20022",
                    ),
                    "source.url must not use reserved placeholder hostnames",
                ),
                (
                    "reserved-tld-url",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "url",
                        "https://pki.swift.example/iso20022",
                    ),
                    "source.url must not use reserved placeholder hostnames",
                ),
                (
                    "template-canary-url",
                    lambda summary: summary["bundles"][0]["source"].__setitem__(
                        "url",
                        "https://pki.swift.operator-canary.bank/iso20022",
                    ),
                    "source.url must not use reserved placeholder hostnames",
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

    def test_executed_canary_commands_reject_repository_fixture_paths(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            fixture_root = "/ops/fixtures/iso20022"
            cases = (
                (
                    "rail-inbox",
                    lambda body: body["stages"][0]["command"].__setitem__(
                        3,
                        f"{fixture_root}/rail-inbox",
                    ),
                    "stages[0].command[3] must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "rail-message",
                    lambda body: body["stages"][0]["command"].extend(
                        ["--message", f"{fixture_root}/rail-inbox/payment.xml"]
                    ),
                    "stages[0].command[11] must not point to checked-in ISO XML fixtures",
                ),
                (
                    "rail-receipt-dir",
                    lambda body: (
                        body["stages"][0]["command"].__setitem__(
                            7,
                            f"{fixture_root}/rail-receipts",
                        ),
                        body["stages"][0].__setitem__(
                            "receipt_dir",
                            f"{fixture_root}/rail-receipts",
                        ),
                    ),
                    "stages[0].command[7] must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "notary-export",
                    lambda body: body["stages"][1]["command"].__setitem__(
                        3,
                        f"{fixture_root}/audit-export",
                    ),
                    "stages[1].command[3] must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "notary-receipt-dir",
                    lambda body: (
                        body["stages"][1]["command"].__setitem__(
                            5,
                            f"{fixture_root}/notary-receipts",
                        ),
                        body["stages"][1].__setitem__(
                            "receipt_dir",
                            f"{fixture_root}/notary-receipts",
                        ),
                    ),
                    "stages[1].command[5] must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "verify-receipt-dir",
                    lambda body: body["stages"][2]["command"].__setitem__(
                        3,
                        f"{fixture_root}/rail-receipts",
                    ),
                    "stages[2].command[3] must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "verify-receipt",
                    lambda body: body["stages"][2]["command"].extend(
                        ["--receipt", f"{fixture_root}/rail.receipt.json"]
                    ),
                    "stages[2].command[8] must not point to checked-in ISO fixture artifacts",
                ),
            )
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    body = valid_canary_summary()
                    mutate(body)
                    body.pop("summary_sha256")
                    canary_path = write_canary(root, digest_summary(body))

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
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
            for stage_index in (0, 1, 2):
                noisy_stage = valid_canary_summary()
                noisy_stage["stages"][stage_index]["stderr_preview"] = "stage warning"
                noisy_stage.pop("summary_sha256")
                cases.append(
                    (
                        digest_summary(noisy_stage),
                        "stderr_preview must be empty for successful stage",
                    )
                )
            for stage_index, field, message in (
                (
                    0,
                    "stdout_preview",
                    "stdout_preview contains unsafe control characters",
                ),
                (
                    1,
                    "stderr_preview",
                    "stderr_preview contains unsafe control characters",
                ),
            ):
                control_stage = valid_canary_summary()
                control_stage["stages"][stage_index][field] = "stage \x1b[31mwarning"
                control_stage.pop("summary_sha256")
                cases.append((digest_summary(control_stage), message))
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
            missing_version_summary = json.loads(receipt_stdout())
            missing_version_summary.pop("version")
            missing_version = valid_canary_summary()
            missing_version["stages"][2]["stdout_preview"] = (
                json.dumps(
                    digest_receipt_summary(missing_version_summary),
                    sort_keys=True,
                )
                + "\n"
            )
            missing_version.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(missing_version),
                    "version must be receipt verifier summary version",
                )
            )
            unsupported_version_summary = json.loads(receipt_stdout())
            unsupported_version_summary["version"] = EVIDENCE.RECEIPT_SUMMARY_VERSION + 1
            unsupported_version = valid_canary_summary()
            unsupported_version["stages"][2]["stdout_preview"] = (
                json.dumps(
                    digest_receipt_summary(unsupported_version_summary),
                    sort_keys=True,
                )
                + "\n"
            )
            unsupported_version.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(unsupported_version),
                    "version must be receipt verifier summary version",
                )
            )
            empty = valid_canary_summary()
            empty["stages"][2]["stdout_preview"] = receipt_stdout(verified_receipts=0)
            empty.pop("summary_sha256")
            cases.append((digest_summary(empty), "verified_receipts must be positive"))
            missing_kind = valid_canary_summary()
            missing_kind["stages"][2]["stdout_preview"] = receipt_stdout(
                ["iso-rail-gateway"],
                verified_receipts=1,
            )
            missing_kind.pop("summary_sha256")
            cases.append(
                (
                    digest_summary(missing_kind),
                    "receipt_summary is missing receipt kinds for executed stages",
                )
            )
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
            duplicate_path_value = "/ops/iso/receipts/private-corridor.receipt.json"
            duplicate_path["receipts"][0]["path"] = duplicate_path_value
            duplicate_path["receipts"][1]["path"] = duplicate_path_value
            cases.append((duplicate_path, "path duplicates", duplicate_path_value))
            duplicate_digest = json.loads(receipt_stdout())
            duplicate_digest_value = "a" * 64
            duplicate_digest["receipts"][0]["receipt_sha256"] = duplicate_digest_value
            duplicate_digest["receipts"][1]["receipt_sha256"] = duplicate_digest[
                "receipts"
            ][0]["receipt_sha256"]
            cases.append((duplicate_digest, "receipt_sha256 duplicates", duplicate_digest_value))

            for receipt_summary, message, hidden_value in cases:
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
                    self.assertNotIn(hidden_value, stderr)

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
                    "fixtures/iso20022/rail.receipt.json",
                    "must not point to checked-in ISO fixture artifacts",
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
                    "too-large-status",
                    lambda receipt: receipt.update({"ok": False, "status_code": 700}),
                    "stdout_preview.receipts[0].status_code must be an HTTP status integer",
                ),
                (
                    "null-success-status",
                    lambda receipt: receipt.update(
                        {"status_code": None, "response_body_sha256": None}
                    ),
                    "stdout_preview.receipts[0].ok does not match status_code success state",
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
                (
                    "transport-failed-status",
                    lambda receipt: receipt.update(
                        {
                            "ok": False,
                            "status_code": None,
                            "response_body_sha256": None,
                        }
                    ),
                    "stdout_preview.receipts[0] did not succeed",
                ),
                (
                    "missing-response-body-digest",
                    lambda receipt: receipt.pop("response_body_sha256"),
                    "stdout_preview.receipts[0].response_body_sha256 must be a canonical SHA-256",
                ),
                (
                    "malformed-response-body-digest",
                    lambda receipt: receipt.__setitem__("response_body_sha256", "not-a-digest"),
                    "stdout_preview.receipts[0].response_body_sha256 must be a canonical SHA-256",
                ),
                (
                    "missing-endpoint-policy-evidence",
                    lambda receipt: receipt.pop("endpoint_requires_insecure_http"),
                    "stdout_preview.receipts[0].endpoint_requires_insecure_http must be a boolean",
                ),
                (
                    "malformed-endpoint-policy-evidence",
                    lambda receipt: receipt.__setitem__(
                        "endpoint_requires_insecure_http",
                        "false",
                    ),
                    "stdout_preview.receipts[0].endpoint_requires_insecure_http must be a boolean",
                ),
                (
                    "hidden-endpoint-policy-evidence",
                    lambda receipt: receipt.__setitem__(
                        "endpoint_requires_insecure_http",
                        True,
                    ),
                    "stdout_preview.receipts[0].endpoint_requires_insecure_http requires "
                    "allow_insecure_http=true",
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

    def test_receipt_verifier_stdout_rejects_all_zero_digest_placeholders(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = (
                (
                    "notary-receipt",
                    0,
                    lambda receipt: receipt.__setitem__("receipt_sha256", "0" * 64),
                    "stdout_preview.receipts[0].receipt_sha256 must not be all zero",
                ),
                (
                    "notary-response-body",
                    0,
                    lambda receipt: receipt.__setitem__("response_body_sha256", "0" * 64),
                    "stdout_preview.receipts[0].response_body_sha256 must not be all zero",
                ),
                (
                    "notary-anchor",
                    0,
                    lambda receipt: receipt.__setitem__("anchor_sha256", "0" * 64),
                    "stdout_preview.receipts[0].anchor_sha256 must not be all zero",
                ),
                (
                    "notary-index",
                    0,
                    lambda receipt: receipt.__setitem__("index_sha256", "0" * 64),
                    "stdout_preview.receipts[0].index_sha256 must not be all zero",
                ),
                (
                    "rail-payload",
                    1,
                    lambda receipt: receipt.__setitem__("payload_sha256", "0" * 64),
                    "stdout_preview.receipts[1].payload_sha256 must not be all zero",
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

    def test_receipt_verifier_stdout_requires_kind_specific_metadata(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            hidden = "\u0660"
            unicode_digit_message_type = f"pacs.{hidden}{hidden}2"
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
                    "stdout_preview.receipts[0].record_count must be a positive integer",
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
                    "rail-unsupported-message-type",
                    1,
                    lambda receipt: receipt.__setitem__("message_type", "zzzz.999"),
                    "stdout_preview.receipts[1].message_type is unsupported",
                ),
                (
                    "rail-non-ascii-message-type",
                    1,
                    lambda receipt: receipt.__setitem__(
                        "message_type",
                        unicode_digit_message_type,
                    ),
                    "stdout_preview.receipts[1].message_type must use printable ASCII",
                ),
                (
                    "rail-secret-message-type",
                    1,
                    lambda receipt: receipt.__setitem__("message_type", "token.001"),
                    "stdout_preview.receipts[1].message_type must not contain secret-looking material",
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
                    lambda receipt: receipt.__setitem__("profile", "unknown_rail"),
                    "stdout_preview.receipts[1].profile must be a canonical lowercase profile id",
                ),
                (
                    "rail-missing-message-id",
                    1,
                    lambda receipt: receipt.pop("rail_message_id"),
                    "stdout_preview.receipts[1].rail_message_id must be recorded",
                ),
                (
                    "rail-bad-message-id",
                    1,
                    lambda receipt: receipt.__setitem__("rail_message_id", "rail/drop/1"),
                    "stdout_preview.receipts[1].rail_message_id must be a canonical ASCII rail message id",
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
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn(unicode_digit_message_type, stderr)
                    self.assertNotIn("token.001", stderr)

    def test_receipt_verifier_stdout_policy_flags_are_required(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            for flag in (
                "allow_failed",
                "allow_insecure_http",
                "allow_legacy_colr007",
                "allow_default_profile",
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

    def test_receipt_verifier_allow_failed_policy_requires_failed_entry(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            receipt_summary = json.loads(receipt_stdout(allow_failed=True))
            body = valid_canary_summary()
            body["stages"][2]["command"].append("--allow-failed")
            body["stages"][2]["stdout_preview"] = (
                json.dumps(digest_receipt_summary(receipt_summary), sort_keys=True)
                + "\n"
            )
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-canary-stage-receipts-only",
                    "--allow-failed-receipts",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "stdout_preview.allow_failed requires at least one failed receipt",
                stderr,
            )

    def test_receipt_verifier_allow_failed_policy_accepts_failed_entry(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            receipt_summary = json.loads(receipt_stdout(allow_failed=True))
            receipt_summary["receipts"][0]["ok"] = False
            receipt_summary["receipts"][0]["status_code"] = 500
            body = valid_canary_summary()
            body["stages"][2]["command"].append("--allow-failed")
            body["stages"][2]["stdout_preview"] = (
                json.dumps(digest_receipt_summary(receipt_summary), sort_keys=True)
                + "\n"
            )
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-canary-stage-receipts-only",
                    "--allow-failed-receipts",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            receipt_summary = summary["canary_summaries"][0]["receipt_summary"]
            self.assertTrue(receipt_summary["allow_failed"])
            self.assertFalse(receipt_summary["receipts"][0]["ok"])

    def test_receipt_verifier_allow_failed_policy_accepts_transport_failed_entry(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            receipt_summary = json.loads(receipt_stdout(allow_failed=True))
            receipt_summary["receipts"][0]["ok"] = False
            receipt_summary["receipts"][0]["status_code"] = None
            receipt_summary["receipts"][0]["response_body_sha256"] = None
            body = valid_canary_summary()
            body["stages"][2]["command"].append("--allow-failed")
            body["stages"][2]["stdout_preview"] = (
                json.dumps(digest_receipt_summary(receipt_summary), sort_keys=True)
                + "\n"
            )
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-canary-stage-receipts-only",
                    "--allow-failed-receipts",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            receipt_summary = summary["canary_summaries"][0]["receipt_summary"]
            self.assertTrue(receipt_summary["allow_failed"])
            self.assertFalse(receipt_summary["receipts"][0]["ok"])
            self.assertIsNone(receipt_summary["receipts"][0]["status_code"])
            self.assertIsNone(receipt_summary["receipts"][0]["response_body_sha256"])

    def test_receipt_verifier_transport_failed_entry_rejects_response_digest(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            receipt_summary = json.loads(receipt_stdout(allow_failed=True))
            receipt_summary["receipts"][0]["ok"] = False
            receipt_summary["receipts"][0]["status_code"] = None
            body = valid_canary_summary()
            body["stages"][2]["command"].append("--allow-failed")
            body["stages"][2]["stdout_preview"] = (
                json.dumps(digest_receipt_summary(receipt_summary), sort_keys=True)
                + "\n"
            )
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))

            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-canary-stage-receipts-only",
                    "--allow-failed-receipts",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn(
                "stdout_preview.receipts[0].response_body_sha256 must be null "
                "without HTTP status_code",
                stderr,
            )

    def test_receipt_verifier_allow_insecure_policy_requires_endpoint_entry(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            body = valid_canary_summary()
            body["stages"][2]["command"].append("--allow-insecure-http")
            receipt_summary = json.loads(receipt_stdout(allow_insecure_http=True))
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
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-canary-stage-receipts-only",
                    "--allow-insecure-http",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn(
                "stdout_preview.allow_insecure_http requires at least one http:// "
                "or local/private receipt endpoint",
                stderr,
            )

    def test_receipt_verifier_allow_insecure_policy_accepts_endpoint_entry(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            body = valid_canary_summary()
            body["stages"][2]["command"].append("--allow-insecure-http")
            receipt_summary = json.loads(receipt_stdout(allow_insecure_http=True))
            receipt_summary["receipts"][0]["endpoint_requires_insecure_http"] = True
            body["stages"][2]["stdout_preview"] = (
                json.dumps(
                    digest_receipt_summary(receipt_summary),
                    sort_keys=True,
                )
                + "\n"
            )
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-canary-stage-receipts-only",
                    "--allow-insecure-http",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            receipt_summary = summary["canary_summaries"][0]["receipt_summary"]
            self.assertTrue(receipt_summary["allow_insecure_http"])
            self.assertTrue(
                receipt_summary["receipts"][0]["endpoint_requires_insecure_http"]
            )

    def test_receipt_source_missing_policy_accepts_matching_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            body = valid_canary_summary()
            body["stages"][2]["command"].remove("--require-source-files")
            receipt_summary = json.loads(receipt_stdout(require_source_files=False))
            body["stages"][2]["stdout_preview"] = (
                json.dumps(
                    digest_receipt_summary(receipt_summary),
                    sort_keys=True,
                )
                + "\n"
            )
            body.pop("summary_sha256")
            canary_path = write_canary(root, digest_summary(body))

            rc, stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--allow-canary-stage-receipts-only",
                    "--allow-receipt-source-missing",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["policy"]["allow_receipt_source_missing"])
            self.assertFalse(
                summary["canary_summaries"][0]["receipt_summary"][
                    "require_source_files"
                ]
            )

    def test_receipt_verifier_stdout_policy_flags_must_match_command(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            trust_path = write_trust_summary(root)
            cases = (
                (
                    "command-allow-failed-hidden",
                    lambda command: command.append("--allow-failed"),
                    lambda summary: None,
                    ["--allow-failed-receipts"],
                    "stdout_preview.allow_failed does not match command --allow-failed",
                ),
                (
                    "stdout-allow-failed-without-command",
                    lambda command: None,
                    lambda summary: (
                        summary.__setitem__("allow_failed", True),
                        summary["receipts"][0].__setitem__("ok", False),
                        summary["receipts"][0].__setitem__("status_code", 500),
                    ),
                    ["--allow-failed-receipts"],
                    "stdout_preview.allow_failed does not match command --allow-failed",
                ),
                (
                    "command-insecure-hidden",
                    lambda command: command.append("--allow-insecure-http"),
                    lambda summary: None,
                    ["--allow-insecure-http"],
                    "stdout_preview.allow_insecure_http does not match command --allow-insecure-http",
                ),
                (
                    "stdout-insecure-without-command",
                    lambda command: None,
                    lambda summary: (
                        summary.__setitem__("allow_insecure_http", True),
                        summary["receipts"][0].__setitem__(
                            "endpoint_requires_insecure_http",
                            True,
                        ),
                    ),
                    ["--allow-insecure-http"],
                    "stdout_preview.allow_insecure_http does not match command --allow-insecure-http",
                ),
                (
                    "command-legacy-hidden",
                    lambda command: command.append("--allow-legacy-colr007"),
                    lambda summary: None,
                    ["--allow-legacy-colr007"],
                    "stdout_preview.allow_legacy_colr007 does not match command --allow-legacy-colr007",
                ),
                (
                    "stdout-legacy-without-command",
                    lambda command: None,
                    lambda summary: summary.__setitem__("allow_legacy_colr007", True),
                    ["--allow-legacy-colr007"],
                    "stdout_preview.allow_legacy_colr007 does not match command --allow-legacy-colr007",
                ),
                (
                    "command-default-profile-hidden",
                    lambda command: command.append("--allow-default-profile"),
                    lambda summary: None,
                    ["--allow-default-profile"],
                    "stdout_preview.allow_default_profile does not match command --allow-default-profile",
                ),
                (
                    "stdout-default-profile-without-command",
                    lambda command: None,
                    lambda summary: summary.__setitem__("allow_default_profile", True),
                    ["--allow-default-profile"],
                    "stdout_preview.allow_default_profile does not match command --allow-default-profile",
                ),
                (
                    "command-require-source-hidden",
                    lambda command: None,
                    lambda summary: summary.__setitem__("require_source_files", False),
                    ["--allow-receipt-source-missing"],
                    "stdout_preview.require_source_files does not match command --require-source-files",
                ),
                (
                    "stdout-require-source-without-command",
                    lambda command: command.remove("--require-source-files"),
                    lambda summary: None,
                    ["--allow-receipt-source-missing"],
                    "stdout_preview.require_source_files does not match command --require-source-files",
                ),
            )
            for name, mutate_command, mutate_summary, extra_args, message in cases:
                with self.subTest(name=name):
                    body = valid_canary_summary()
                    receipt_summary = json.loads(receipt_stdout())
                    mutate_command(body["stages"][2]["command"])
                    mutate_summary(receipt_summary)
                    body["stages"][2]["stdout_preview"] = (
                        json.dumps(
                            digest_receipt_summary(receipt_summary),
                            sort_keys=True,
                        )
                        + "\n"
                    )
                    body.pop("summary_sha256")
                    canary_path = write_canary(root, digest_summary(body))

                    rc, stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                        ]
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

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
            self.assertIn("provider does not match expected provider", stderr)
            self.assertNotIn("different-provider", stderr)
            self.assertNotIn("local-bank", stderr)

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
            self.assertIn("environment does not match expected environment", stderr)
            self.assertNotIn("'prod'", stderr)
            self.assertNotIn("'preprod'", stderr)

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
                lambda summary: summary["bundles"][0].__setitem__("source", None),
            )
            cases.append((missing_source_path, "source is required"))
            for trust_path, message in cases:
                with self.subTest(message=message):
                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_forged_trust_summary_material_requires_matching_policy_flag(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            cases = (
                (
                    "record-only",
                    write_trust_summary(root / "legit-record", record_only=True),
                    write_trust_summary(root / "hidden-record"),
                    lambda trust: (
                        trust["bundles"][0].__setitem__(
                            "embedded_signature_policy",
                            "record-only",
                        ),
                        trust["bundles"][0]["profile_overrides"].__setitem__(
                            "embedded_signature_policy",
                            "record-only",
                        ),
                    ),
                    ["--allow-record-only-trust"],
                    "allow_record_only must be true when a bundle records "
                    "a non-production embedded_signature_policy",
                ),
                (
                    "insecure-source",
                    write_trust_summary(root / "legit-insecure", insecure_source=True),
                    write_trust_summary(root / "hidden-insecure"),
                    lambda trust: trust["bundles"][0]["source"].__setitem__(
                        "url",
                        "http://pki.local/swift-cbpr-plus",
                    ),
                    ["--allow-insecure-http"],
                    "allow_insecure_source_url must be true when a bundle records "
                    "an http:// or local/private source URL",
                ),
            )
            for name, legit_path, hidden_path, mutate, overrides, message in cases:
                with self.subTest(name=name):
                    rewrite_trust_summary(
                        hidden_path,
                        lambda trust, mutate=mutate: (
                            mutate(trust),
                            trust.__setitem__("profile_json_emitted", False),
                            trust.__setitem__("profile_json_sha256", None),
                        ),
                    )

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(legit_path),
                            "--trust-summary",
                            str(hidden_path),
                            "--allow-canary-stage-receipts-only",
                            "--allow-profile-json-not-emitted",
                        ]
                        + overrides
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_diagnostic_trust_summary_flags_are_preserved_in_evidence(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            missing_source_path = write_trust_summary(root / "missing-source")
            rewrite_trust_summary(
                missing_source_path,
                lambda trust: (
                    trust.__setitem__("max_source_age_days", None),
                    trust.__setitem__("profile_json_emitted", False),
                    trust.__setitem__("profile_json_emittable", False),
                    trust.__setitem__("profile_json_sha256", None),
                    trust["bundles"][0].__setitem__("source", None),
                ),
            )
            omitted_source_path = write_trust_summary(root / "omitted-source")
            rewrite_trust_summary(
                omitted_source_path,
                lambda trust: (
                    trust.__setitem__("max_source_age_days", None),
                    trust.__setitem__("profile_json_emitted", False),
                    trust.__setitem__("profile_json_emittable", False),
                    trust.__setitem__("profile_json_sha256", None),
                    trust["bundles"][0].pop("source"),
                ),
            )
            rc, _stdout, stderr = run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(omitted_source_path),
                    "--allow-canary-stage-receipts-only",
                    "--allow-profile-json-not-emitted",
                    "--allow-missing-trust-source",
                ]
            )
            self.assertEqual(rc, 2)
            self.assertIn("source must be explicitly recorded", stderr)
            cases = (
                (
                    "synthetic",
                    write_trust_summary(root / "synthetic", synthetic=True),
                    ["--allow-synthetic-trust"],
                    {
                        "allow_synthetic_der": True,
                        "allow_record_only": False,
                        "allow_insecure_source_url": False,
                    },
                    True,
                ),
                (
                    "record",
                    write_trust_summary(root / "record", record_only=True),
                    ["--allow-record-only-trust"],
                    {
                        "allow_synthetic_der": False,
                        "allow_record_only": True,
                        "allow_insecure_source_url": False,
                    },
                    True,
                ),
                (
                    "insecure",
                    write_trust_summary(root / "insecure", insecure_source=True),
                    ["--allow-insecure-http"],
                    {
                        "allow_synthetic_der": False,
                        "allow_record_only": False,
                        "allow_insecure_source_url": True,
                    },
                    True,
                ),
                (
                    "missing-source",
                    missing_source_path,
                    ["--allow-missing-trust-source"],
                    {
                        "allow_synthetic_der": False,
                        "allow_record_only": False,
                        "allow_insecure_source_url": False,
                    },
                    False,
                ),
            )
            for name, trust_path, trust_overrides, expected_flags, expect_source in cases:
                with self.subTest(name=name):
                    rc, stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                            "--allow-canary-stage-receipts-only",
                            "--allow-profile-json-not-emitted",
                        ]
                        + trust_overrides
                    )

                    self.assertEqual(rc, 0, stderr)
                    trust_summary = json.loads(stdout)["trust_summaries"][0]
                    for flag, expected in expected_flags.items():
                        self.assertEqual(trust_summary[flag], expected)
                    if expect_source:
                        self.assertIsNotNone(trust_summary["profiles"][0]["source"])
                    else:
                        self.assertIsNone(trust_summary["profiles"][0]["source"])

                    policy = json.loads(stdout)["policy"]
                    if name != "synthetic":
                        self.assertFalse(policy["allow_synthetic_trust"])
                    if name == "missing-source":
                        self.assertTrue(policy["allow_missing_trust_source"])

    def test_forged_trust_summary_policy_flags_require_matching_material(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            cases = (
                (
                    "record-only",
                    "allow_record_only",
                    ["--allow-record-only-trust"],
                    "allow_record_only requires at least one non-production "
                    "embedded_signature_policy",
                ),
                (
                    "insecure-source",
                    "allow_insecure_source_url",
                    ["--allow-insecure-http"],
                    "allow_insecure_source_url requires at least one http:// "
                    "or local/private source URL",
                ),
            )
            for name, flag, overrides, message in cases:
                with self.subTest(name=name):
                    trust_path = write_trust_summary(root / name)
                    rewrite_trust_summary(
                        trust_path,
                        lambda trust: (
                            trust.__setitem__(flag, True),
                            trust.__setitem__("profile_json_emitted", False),
                            trust.__setitem__("profile_json_emittable", False),
                            trust.__setitem__("profile_json_sha256", None),
                        ),
                    )

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                            "--allow-canary-stage-receipts-only",
                            "--allow-profile-json-not-emitted",
                        ]
                        + overrides
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_unsupported_trust_policy_is_rejected_even_with_record_only_override(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            cases = (
                (
                    "ascii-unsupported",
                    "diagnostic-only",
                    "embedded_signature_policy is unsupported",
                    None,
                ),
                (
                    "non-ascii",
                    "require-verif\u0456ed",
                    "embedded_signature_policy must use printable ASCII",
                    "require-verif\u0456ed",
                ),
            )
            for name, policy, message, hidden in cases:
                with self.subTest(name=name):
                    trust_path = write_trust_summary(root / name)
                    rewrite_trust_summary(
                        trust_path,
                        lambda summary, policy=policy: summary["bundles"][0].__setitem__(
                            "embedded_signature_policy",
                            policy,
                        ),
                    )

                    rc, stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(trust_path),
                            "--allow-record-only-trust",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    if hidden is not None:
                        self.assertNotIn(hidden, stderr)
                        self.assertNotIn("embedded_signature_policy is unsupported", stderr)

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

    def test_profile_json_not_emitted_override_requires_matching_trust_summary(self):
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

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "--allow-profile-json-not-emitted requires at least one trust "
                "summary with profile_json_emitted=false",
                stderr,
            )
            self.assertFalse(summary_out.exists())

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
                    lambda summary: summary.update({"profile_json_sha256": "1" * 64}),
                    "profile_json_sha256 does not match archived profile_overrides",
                ),
                (
                    "all-zero",
                    lambda summary: summary.update({"profile_json_sha256": "0" * 64}),
                    "profile_json_sha256 must not be all zero",
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
            for name, mutate in (
                ("missing", lambda summary: summary.pop("profile_json_sha256")),
                (
                    "non-null",
                    lambda summary: summary.update({"profile_json_sha256": "0" * 64}),
                ),
            ):
                with self.subTest(name=name):
                    trust = json.loads(trust_path.read_text(encoding="utf-8"))
                    mutate(trust)
                    mutated_path = write_json(
                        root / f"{name}-not-emitted-profile.trust.summary.json",
                        digest_summary(trust),
                    )

                    rc, _stdout, stderr = run_evidence(
                        [
                            "--canary-summary",
                            str(canary_path),
                            "--trust-summary",
                            str(mutated_path),
                            "--allow-profile-json-not-emitted",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(
                        "profile_json_sha256 must be null when profile JSON was not emitted",
                        stderr,
                    )

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

    def test_missing_source_override_does_not_allow_present_stale_source_budget(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
            trust_path = write_trust_summary(root / "trust")
            rewrite_trust_summary(
                trust_path,
                lambda trust: (
                    trust.__setitem__("max_source_age_days", 1),
                    trust.__setitem__("profile_json_emitted", False),
                    trust.__setitem__("profile_json_emittable", False),
                    trust.__setitem__("profile_json_sha256", None),
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
                    "--allow-missing-trust-source",
                    "--allow-profile-json-not-emitted",
                    "--max-trust-source-age-days",
                    "36500",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("cannot emit production profile JSON", stderr)

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
                    "all-zero",
                    lambda trust: trust["bundles"][0].__setitem__("bundle_sha256", "0" * 64),
                    "bundle_sha256 must not be all zero",
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
                    duplicate_digest = None
                    if name == "duplicate":
                        trust = json.loads(trust_path.read_text(encoding="utf-8"))
                        duplicate_digest = trust["bundles"][0]["bundle_sha256"]

                    rc, _stdout, stderr = run_evidence(
                        ["--canary-summary", str(canary_path), "--trust-summary", str(trust_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    if duplicate_digest is not None:
                        self.assertNotIn(duplicate_digest, stderr)

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
                    "crl-der-wrong-class",
                    lambda summary: replace_profile_der(
                        summary,
                        "x509_crl_der_base64",
                        "x509_crls",
                        trust_test.CERT_ONE_B64,
                    ),
                    "must look like an X.509 CRL",
                ),
                (
                    "ocsp-der-wrong-class",
                    lambda summary: replace_profile_der(
                        summary,
                        "x509_ocsp_response_der_base64",
                        "x509_ocsp_responses",
                        trust_test.CERT_ONE_B64,
                    ),
                    "must look like an OCSPResponse",
                ),
                (
                    "crl-der-digest-drift",
                    lambda summary: summary["bundles"][0]["profile_overrides"][
                        "x509_crl_der_base64"
                    ].__setitem__(0, ALT_CRL_B64),
                    "not recorded in",
                ),
                (
                    "ocsp-der-digest-drift",
                    lambda summary: summary["bundles"][0]["profile_overrides"][
                        "x509_ocsp_response_der_base64"
                    ].__setitem__(0, ALT_OCSP_B64),
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
                    "all-zero-anchor-pin",
                    lambda summary: summary["bundles"][0]["profile_overrides"][
                        "x509_trust_anchor_sha256_pins"
                    ].__setitem__(0, "0" * 64),
                    "x509_trust_anchor_sha256_pins[0] must not be all zero",
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
                    "all-zero-crl-summary-digest",
                    lambda summary: summary["bundles"][0]["x509_crls"][0].__setitem__(
                        "sha256",
                        "0" * 64,
                    ),
                    "x509_crls[0].sha256 must not be all zero",
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
