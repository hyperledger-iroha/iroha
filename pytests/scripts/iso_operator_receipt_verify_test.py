import importlib.util
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

from pytests.scripts import iso_audit_notary_adapter_test as audit_test
from pytests.scripts import iso_rail_gateway_adapter_test as rail_test


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "iso_operator_receipt_verify.py"
SPEC = importlib.util.spec_from_file_location("iso_operator_receipt_verify", SCRIPT_PATH)
VERIFIER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = VERIFIER
SPEC.loader.exec_module(VERIFIER)


def run_verify(argv):
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        rc = VERIFIER.main(argv)
    return rc, stdout.getvalue(), stderr.getvalue()


def rewrite_receipt(path, mutate):
    receipt = json.loads(path.read_text(encoding="utf-8"))
    mutate(receipt)
    receipt.pop(VERIFIER.RECEIPT_DIGEST_FIELD, None)
    receipt[VERIFIER.RECEIPT_DIGEST_FIELD] = VERIFIER.sha256_hex(
        VERIFIER._canonical_json_bytes(receipt)
    )
    path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    return receipt


@contextlib.contextmanager
def patched_verifier_constant(name, value):
    original = getattr(VERIFIER, name)
    setattr(VERIFIER, name, value)
    try:
        yield
    finally:
        setattr(VERIFIER, name, original)


def oversized_json_bytes(limit):
    return b'{"padding":"' + (b"a" * (limit + 1)) + b'"}\n'


class IsoOperatorReceiptVerifyTest(unittest.TestCase):
    def test_persisted_record_sources_require_records_array(self):
        with self.assertRaisesRegex(
            VERIFIER.ReceiptError,
            "anchor.records must be an array",
        ):
            VERIFIER._verify_persisted_record_sources(
                {},
                None,
                "anchor",
                require_source_files=True,
            )

    def test_secret_looking_unknown_keys_are_rejected_without_echo(self):
        cases = (
            ("password_receipt_unknown_secret", "receipt_unknown_secret"),
            ("%70assword_receipt_unknown_leak", "receipt_unknown_leak"),
            ("private-key_receipt_unknown_leak", "receipt_unknown_leak"),
            ("unexpected\x1breceipt_key", "\x1b"),
            ("unexpected_receipt_\uff4bey", "\uff4b"),
            ("x" * 129, "x" * 129),
        )
        for unknown_key, hidden in cases:
            with self.subTest(unknown_key=unknown_key):
                with self.assertRaises(VERIFIER.ReceiptError) as caught:
                    VERIFIER._reject_unknown_keys(
                        {unknown_key: "redacted"}, set(), "receipt"
                    )

                message = str(caught.exception)
                self.assertIn("contains unknown keys", message)
                self.assertNotIn("password", message)
                self.assertNotIn(unknown_key, message)
                self.assertNotIn(hidden, message)
        many_unknown = {f"field_{offset}": "redacted" for offset in range(9)}
        with self.assertRaises(VERIFIER.ReceiptError) as caught:
            VERIFIER._reject_unknown_keys(many_unknown, set(), "receipt")
        message = str(caught.exception)
        self.assertIn("contains unknown keys", message)
        self.assertNotIn("field_0", message)
        self.assertNotIn("field_8", message)

    def test_cli_argument_terminator_is_rejected_without_echo(self):
        hidden = "token=receipt-terminator-secret"
        cases = (
            (
                "raw",
                lambda: VERIFIER._preflight_raw_cli_secrets(
                    ["--", "--receipt-dir", hidden],
                    {"--receipt-dir"},
                ),
            ),
            (
                "path",
                lambda: VERIFIER._preflight_cli_paths(
                    ["--", "--receipt-dir", hidden],
                    {"--receipt-dir"},
                ),
            ),
            (
                "boolean",
                lambda: VERIFIER._preflight_boolean_cli_flags(
                    ["--", "--allow-failed", hidden],
                    {"--allow-failed"},
                ),
            ),
        )
        for helper, run in cases:
            with self.subTest(helper=helper):
                with self.assertRaises(VERIFIER.ReceiptError) as caught:
                    run()

                message = str(caught.exception)
                self.assertIn("argument terminator is not supported", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn("receipt-terminator-secret", message)

    def test_parser_rejects_abbreviated_long_options(self):
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as caught:
                VERIFIER.build_parser().parse_args(["--receipt-di", "receipts"])

        self.assertEqual(caught.exception.code, 2)
        self.assertIn("unrecognized arguments", stderr.getvalue())
        self.assertIn("--receipt-di", stderr.getvalue())

    def test_raw_cli_control_characters_are_rejected_without_echo(self):
        hidden = "--unknown-receipt\x1bflag"
        with self.assertRaises(VERIFIER.ReceiptError) as caught:
            VERIFIER._preflight_raw_cli_secrets([hidden], {"--receipt-dir"})

        message = str(caught.exception)
        self.assertIn("CLI argument must not contain control characters", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn("\x1b", message)
        self.assertNotIn("unknown-receipt", message)

    def test_raw_cli_non_ascii_is_rejected_without_echo(self):
        hidden = "\uff0d\uff0dreceipt-dir"
        with self.assertRaises(VERIFIER.ReceiptError) as caught:
            VERIFIER._preflight_raw_cli_secrets([hidden], {"--receipt-dir"})

        message = str(caught.exception)
        self.assertIn("CLI argument must use printable ASCII", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn("receipt-dir", message)

    def test_nested_control_material_in_receipt_is_rejected_without_echo(self):
        cases = (
            (
                {"metadata": {"unexpected\x1breceipt_key": "redacted"}},
                "forbidden control-bearing field",
                "receipt_key",
            ),
            (
                {"receipt_kind": "warning \x1b[31mred"},
                "unsafe control characters",
                "[31mred",
            ),
        )
        for body, expected, hidden in cases:
            with self.subTest(body=body):
                with self.assertRaises(VERIFIER.ReceiptError) as caught:
                    VERIFIER._check_no_secret_material(body, Path("receipt.json"))

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn("\x1b", message)
                self.assertNotIn(hidden, message)

    def test_audit_index_source_secret_identifiers_are_rejected_without_echo(self):
        cases = (
            (
                "message_id",
                lambda record: record.update({"message_id": "token-receipt-index-secret"}),
                "token-receipt-index-secret",
            ),
            (
                "profile_id",
                lambda record: record.update({"profile_id": "private_key=receipt-index-secret"}),
                "private_key=receipt-index-secret",
            ),
            (
                "business_message_id",
                lambda record: record.update(
                    {"business_message_id": "%70assword%253Dreceipt-index-secret"}
                ),
                "password=receipt-index-secret",
            ),
            (
                "reference_snapshot_id",
                lambda record: record.update(
                    {"reference_snapshot_id": "x_iroha_signature=receipt-index-secret"}
                ),
                "x_iroha_signature=receipt-index-secret",
            ),
        )
        for name, mutate, hidden in cases:
            with self.subTest(name=name):
                index = audit_test.sample_index()
                mutate(index["records"][0])
                index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)

                with self.assertRaises(VERIFIER.ReceiptError) as caught:
                    VERIFIER._verify_audit_index_source(index, "anchor.audit_index")

                message = str(caught.exception)
                self.assertIn("secret-looking material", message)
                self.assertNotIn("receipt-index-secret", message)
                self.assertNotIn(hidden, message)

    def test_persisted_record_source_secret_values_are_rejected_without_echo(self):
        cases = (
            (
                "detail",
                lambda source: source.update({"detail": "token=receipt-source-secret"}),
                "token=receipt-source-secret",
            ),
            (
                "context",
                lambda source: source["context"].update(
                    {"source_account_id": "private-key=receipt-source-secret"}
                ),
                "private-key=receipt-source-secret",
            ),
            (
                "history-detail",
                lambda source: source["status_history"][0].update(
                    {"detail": "%70assword%253Dreceipt-source-secret"}
                ),
                "password=receipt-source-secret",
            ),
        )
        for name, mutate, hidden in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    record = audit_test.sample_record()
                    source = audit_test.sample_persisted_record(record)
                    mutate(source)
                    source = audit_test.with_digest(
                        source,
                        audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD,
                    )
                    record[audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD] = source[
                        audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD
                    ]
                    record_path = root / record["filename"]
                    record_path.write_text(
                        json.dumps(source, indent=2) + "\n",
                        encoding="utf-8",
                    )

                    with self.assertRaises(VERIFIER.ReceiptError) as caught:
                        VERIFIER._verify_persisted_record_source(
                            record,
                            record_path,
                            str(record_path),
                        )

                    message = str(caught.exception)
                    self.assertIn("secret-looking material", message)
                    self.assertNotIn("receipt-source-secret", message)
                    self.assertNotIn(hidden, message)

    def test_overlong_clean_metadata_strings_are_rejected_without_echo(self):
        overlong = "M" * (VERIFIER.MAX_CLEAN_STRING_CHARS + 1)
        cases = (
            (
                "helper",
                lambda: VERIFIER._require_clean_string(overlong, "receipt.detail"),
                "receipt.detail must be no longer than 4096 characters",
            ),
            (
                "nonsecret-helper",
                lambda: VERIFIER._require_nonsecret_clean_string(
                    overlong,
                    "receipt.business_message_id",
                ),
                "receipt.business_message_id must be no longer than 4096 characters",
            ),
            (
                "normalized-optional",
                lambda: VERIFIER._normalize_optional_string(
                    overlong,
                    "receipt.profile",
                ),
                "receipt.profile must be no longer than 4096 characters",
            ),
            (
                "sidecar-optional",
                lambda: VERIFIER._normalize_sidecar_optional_string(
                    {"business_message_id": overlong},
                    "business_message_id",
                    "sidecar.business_message_id",
                ),
                "sidecar.business_message_id must be no longer than 4096 characters",
            ),
            (
                "audit-index",
                lambda: self._verify_overlong_audit_index_metadata(overlong),
                "anchor.audit_index.records[0].business_message_id must be no longer than 4096 characters",
            ),
            (
                "record-source",
                lambda: self._verify_overlong_record_source_metadata(overlong),
                "record.detail must be no longer than 4096 characters",
            ),
        )
        for name, run, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(VERIFIER.ReceiptError) as caught:
                    run()

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(overlong, message)

    def _verify_overlong_audit_index_metadata(self, overlong):
        index = audit_test.sample_index()
        index["records"][0]["business_message_id"] = overlong
        index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
        VERIFIER._verify_audit_index_source(index, "anchor.audit_index")

    def _verify_overlong_record_source_metadata(self, overlong):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            record = audit_test.sample_record()
            source = audit_test.sample_persisted_record(record)
            source["detail"] = overlong
            source = audit_test.with_digest(
                source,
                audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD,
            )
            record[audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD] = source[
                audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD
            ]
            record_path = root / record["filename"]
            record_path.write_text(
                json.dumps(source, indent=2) + "\n",
                encoding="utf-8",
            )

            VERIFIER._verify_persisted_record_source(
                record,
                record_path,
                "record",
            )

    def test_archived_source_paths_reject_secret_identifiers_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "rail"
            inbox.mkdir()
            xml_path, _sidecar = rail_test.write_message(inbox)
            sidecar_path = xml_path.with_suffix(xml_path.suffix + ".json")
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            original = receipt.read_bytes()
            cases = (
                (
                    "xml_path",
                    lambda: rewrite_receipt(
                        receipt,
                        lambda body: body.update(
                            {
                                "xml_path": str(
                                    xml_path.with_name(
                                        "token-receipt-xml-source-secret.xml"
                                    )
                                )
                            }
                        ),
                    ),
                    "xml_path must not contain secret-looking material",
                    "token-receipt-xml-source-secret",
                ),
                (
                    "sidecar_path",
                    lambda: rewrite_receipt(
                        receipt,
                        lambda body: body.update(
                            {
                                "sidecar_path": str(
                                    sidecar_path.with_name(
                                        "token-receipt-sidecar-source-secret.xml.json"
                                    )
                                )
                            }
                        ),
                    ),
                    "sidecar_path must not contain secret-looking material",
                    "token-receipt-sidecar-source-secret",
                ),
            )
            for name, mutate, expected, hidden in cases:
                with self.subTest(name=name):
                    receipt.write_bytes(original)
                    mutate()

                    rc, _stdout, stderr = run_verify(
                        [
                            "--receipt",
                            str(receipt),
                            "--allow-insecure-http",
                            "--require-source-files",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)
                    self.assertNotIn(hidden, stderr)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            _index, _anchor, digest_anchor = audit_test.write_export(
                export_dir,
                store_dir=root / "store",
                write_record_sources_flag=True,
            )
            with audit_test.capture_server() as (endpoint, _requests):
                self.assertEqual(
                    audit_test.run_main(
                        [
                            "--export-dir",
                            str(export_dir),
                            "--endpoint",
                            endpoint,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((export_dir / "receipts").glob("*.receipt.json"))
            original = receipt.read_bytes()
            latest = export_dir / audit_test.ADAPTER.LATEST_ANCHOR_FILE

            def rewrite_anchor_store_dir() -> None:
                anchor = json.loads(latest.read_text(encoding="utf-8"))
                anchor["store_dir"] = str(root / "token-receipt-store-source-secret")
                anchor = audit_test.with_digest(
                    anchor,
                    audit_test.ADAPTER.ANCHOR_DIGEST_FIELD,
                )
                anchor_text = json.dumps(anchor, indent=2) + "\n"
                latest.write_text(anchor_text, encoding="utf-8")
                digest_anchor.write_text(anchor_text, encoding="utf-8")
                rewrite_receipt(
                    receipt,
                    lambda body: body.update(
                        {
                            "anchor_sha256": anchor[
                                audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                            ]
                        }
                    ),
                )

            cases = (
                (
                    "anchor_path",
                    lambda: rewrite_receipt(
                        receipt,
                        lambda body: body.update(
                            {
                                "anchor_path": str(
                                    latest.with_name(
                                        "token-receipt-anchor-source-secret.notary.json"
                                    )
                                )
                            }
                        ),
                    ),
                    "anchor_path must not contain secret-looking material",
                    "token-receipt-anchor-source-secret",
                ),
                (
                    "store_dir",
                    rewrite_anchor_store_dir,
                    "store_dir must not contain secret-looking material",
                    "token-receipt-store-source-secret",
                ),
            )
            for name, mutate, expected, hidden in cases:
                with self.subTest(name=name):
                    receipt.write_bytes(original)
                    latest.write_bytes(
                        digest_anchor.read_bytes()
                    )
                    mutate()

                    rc, _stdout, stderr = run_verify(
                        [
                            "--receipt",
                            str(receipt),
                            "--allow-insecure-http",
                            "--require-source-files",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_cli_path_flags_reject_flag_like_values(self):
        cases = (
            ["--receipt-dir"],
            ["--receipt-dir", ""],
            ["--receipt-dir", "--allow-insecure-http"],
            ["--receipt-dir="],
            ["--receipt-dir=--allow-insecure-http"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                with self.assertRaisesRegex(
                    VERIFIER.ReceiptError,
                    "--receipt-dir requires a path value",
                ):
                    VERIFIER._preflight_cli_paths(argv, {"--receipt-dir"})

    def test_cli_paths_reject_encoded_secret_material_without_echo(self):
        cases = (
            ("token=receipt-path-leak.receipt.json", "token=receipt-path-leak"),
            ("token%3Dreceipt-path-leak.receipt.json", "token=receipt-path-leak"),
            ("%70assword%253Dreceipt-path-leak.receipt.json", "password=receipt-path-leak"),
            ("token-receipt-path-secret.receipt.json", "token-receipt-path-secret"),
        )
        for raw_path, decoded_secret in cases:
            with self.subTest(raw_path=raw_path):
                with self.assertRaises(VERIFIER.ReceiptError) as caught:
                    VERIFIER._preflight_cli_paths(
                        ["--receipt-dir", raw_path], {"--receipt-dir"}
                    )

                message = str(caught.exception)
                self.assertIn("secret-looking material", message)
                self.assertNotIn(raw_path, message)
                self.assertNotIn(decoded_secret, message)
                self.assertNotIn("receipt-path-leak", message)

    def test_cli_receipt_selectors_reject_repository_fixture_artifacts(self):
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
                    VERIFIER.ReceiptError,
                    f"{flag} must not point to checked-in ISO fixture artifacts",
                ):
                    VERIFIER._preflight_cli_paths([flag, str(path)], {flag})

    def test_receipt_selectors_reject_repository_fixture_before_discovery(self):
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
                    rc, stdout, stderr = run_verify(
                        [flag, str(path), "--allow-insecure-http"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(
                        f"{flag} must not point to checked-in ISO fixture artifacts",
                        stderr,
                    )
                    self.assertNotIn("does not exist", stderr)
                    self.assertFalse((root / "fixtures").exists())

    def test_local_path_validators_reject_percent_encoded_smuggling(self):
        overlong_path = "out/" + ("a" * (VERIFIER.MAX_LOCAL_PATH_CHARS + 1))
        cases = (
            (
                "cli overlong",
                lambda raw: VERIFIER._reject_raw_cli_path_smuggling(raw, "--receipt-dir"),
                overlong_path,
                f"no longer than {VERIFIER.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "source overlong",
                lambda raw: VERIFIER._require_clean_path_string(raw, "receipt.xml_path"),
                overlong_path,
                f"no longer than {VERIFIER.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "cli encoded dot",
                lambda raw: VERIFIER._reject_raw_cli_path_smuggling(raw, "--receipt-dir"),
                "receipts/%2e/archive",
                "encoded dot or separator",
            ),
            (
                "source encoded slash",
                lambda raw: VERIFIER._require_clean_path_string(raw, "receipt.xml_path"),
                "/ops/%2f/rail.xml",
                "encoded dot or separator",
            ),
            (
                "cli uri prefix",
                lambda raw: VERIFIER._reject_raw_cli_path_smuggling(raw, "--receipt-dir"),
                "file:receipts/archive",
                "URI or drive prefixes",
            ),
            (
                "source drive prefix",
                lambda raw: VERIFIER._require_clean_path_string(raw, "receipt.xml_path"),
                "C:/ops/rail.xml",
                "URI or drive prefixes",
            ),
            (
                "source encoded semicolon",
                lambda raw: VERIFIER._require_clean_path_string(raw, "receipt.anchor_path"),
                "/ops/%3b/latest.notary.json",
                "encoded semicolon",
            ),
            (
                "cli encoded delimiter",
                lambda raw: VERIFIER._reject_raw_cli_path_smuggling(raw, "--receipt-dir"),
                "receipts/%23/archive",
                "encoded URL delimiter",
            ),
            (
                "cli encoded percent",
                lambda raw: VERIFIER._reject_raw_cli_path_smuggling(raw, "--receipt-dir"),
                "receipts/%25/archive",
                "encoded percent",
            ),
            (
                "cli encoded control",
                lambda raw: VERIFIER._reject_raw_cli_path_smuggling(raw, "--receipt-dir"),
                "receipts/%00/archive",
                "percent-encoded control or space",
            ),
            (
                "cli malformed percent",
                lambda raw: VERIFIER._reject_raw_cli_path_smuggling(raw, "--receipt-dir"),
                "receipts/%zz/archive",
                "malformed percent",
            ),
        )
        for name, call, raw, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(VERIFIER.ReceiptError) as caught:
                    call(raw)

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(raw, message)

    def test_url_paths_reject_raw_delimiter_smuggling(self):
        cases = (
            "https://notary.local-bank.bank/archive:debug/anchor",
            "https://notary.local-bank.bank/archive@debug/anchor",
            "https://notary.local-bank.bank/archive[debug]/anchor",
        )
        for endpoint in cases:
            with self.subTest(endpoint=endpoint):
                with self.assertRaises(VERIFIER.ReceiptError) as caught:
                    VERIFIER._require_https(
                        endpoint,
                        allow_insecure_http=False,
                        label="receipt.endpoint",
                    )

                message = str(caught.exception)
                self.assertIn("path must not contain URL delimiter characters", message)
                self.assertNotIn(endpoint, message)

    def test_urls_reject_non_ascii_smuggling(self):
        cases = (
            (
                "https://notary\u0661.local-bank.bank/archive/anchor",
                "host must use printable ASCII",
            ),
            (
                "https://notary.local-bank.bank/archive∕debug/anchor",
                "path must use printable ASCII",
            ),
            (
                "https://notary.local-bank.bank/archive%c3%a9/anchor",
                "path must not contain percent-encoded non-ASCII bytes",
            ),
        )
        for endpoint, expected in cases:
            with self.subTest(endpoint=endpoint):
                with self.assertRaises(VERIFIER.ReceiptError) as caught:
                    VERIFIER._require_https(
                        endpoint,
                        allow_insecure_http=False,
                        label="receipt.endpoint",
                    )

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(endpoint, message)

    def test_boolean_cli_flags_reject_values_without_echo(self):
        cases = (
            (["--allow-failed=true"], "--allow-failed", "--allow-failed=true"),
            (["--require-source-files", "true"], "--require-source-files", "true"),
        )
        for argv, flag, rejected in cases:
            with self.subTest(argv=argv):
                rc, stdout, stderr = run_verify(argv)

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertIn(f"{flag} does not take a value", stderr)
                self.assertNotIn(rejected, stderr)

    def test_raw_cli_secret_like_values_rejected_without_echo(self):
        cases = (
            ["--private-key=receipt-secret"],
            ["token=receipt-secret"],
            ["password=receipt-secret"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_verify(argv)

                self.assertEqual(rc, 2)
                self.assertIn("secret-looking", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("password=", stderr)
                self.assertNotIn("receipt-secret", stderr)

    def test_boolean_and_non_integer_file_read_limits_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / "receipt.json"
            path.write_text("{}\n", encoding="utf-8")

            for limit in (True, "64"):
                with self.subTest(limit=limit):
                    with self.assertRaisesRegex(
                        VERIFIER.ReceiptError,
                        "max file bytes must be a positive integer",
                    ):
                        VERIFIER._read_regular_file(path, max_bytes=limit)

    def test_verifies_successful_notary_and_rail_receipts_with_source_files(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            audit_test.write_export(
                export_dir,
                store_dir=root / "store",
                write_record_sources_flag=True,
            )
            with audit_test.capture_server() as (endpoint, _requests):
                self.assertEqual(
                    audit_test.run_main(
                        [
                            "--export-dir",
                            str(export_dir),
                            "--endpoint",
                            endpoint,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )

            inbox = root / "inbox"
            inbox.mkdir()
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )

            rc, stdout, stderr = run_verify(
                [
                    "--receipt-dir",
                    str(export_dir / "receipts"),
                    "--receipt-dir",
                    str(inbox / "receipts"),
                    "--allow-insecure-http",
                    "--require-source-files",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(summary["version"], VERIFIER.RECEIPT_SUMMARY_VERSION)
            self.assertEqual(summary["verified_receipts"], 2)
            self.assertEqual(
                summary["receipt_kind"],
                ["iso-audit-notary", "iso-rail-gateway"],
            )
            self.assertFalse(summary["allow_failed"])
            self.assertTrue(summary["allow_insecure_http"])
            self.assertFalse(summary["allow_legacy_colr007"])
            self.assertFalse(summary["allow_default_profile"])
            self.assertTrue(summary["require_source_files"])
            self.assertEqual(len(summary["receipts"]), 2)
            for receipt in summary["receipts"]:
                self.assertIn(receipt["receipt_kind"], VERIFIER.SUPPORTED_KINDS)
                self.assertTrue(VERIFIER._is_lower_hex_sha256(receipt["receipt_sha256"]))
                raw_receipt = json.loads(Path(receipt["path"]).read_text(encoding="utf-8"))
                self.assertEqual(
                    receipt["response_body_sha256"],
                    raw_receipt["response_body_sha256"],
                )
                self.assertEqual(
                    receipt["endpoint_requires_insecure_http"],
                    VERIFIER._url_requires_insecure_http_override(
                        VERIFIER.urllib.parse.urlparse(
                            raw_receipt["endpoint"]
                            if raw_receipt["receipt_kind"] == "iso-audit-notary"
                            else raw_receipt["endpoint_url"]
                        )
                    ),
                )
                if receipt["receipt_kind"] == "iso-audit-notary":
                    self.assertEqual(receipt["anchor_path"], raw_receipt["anchor_path"])
                    anchor = json.loads(
                        Path(raw_receipt["anchor_path"]).read_text(encoding="utf-8")
                    )
                    anchor_path = Path(raw_receipt["anchor_path"])
                    export_dir = (
                        anchor_path.parent.parent
                        if anchor_path.parent.name == audit_test.ADAPTER.ANCHOR_DIR
                        else anchor_path.parent
                    )
                    self.assertEqual(receipt["store_dir"], anchor["store_dir"])
                    self.assertEqual(
                        receipt["index_path"],
                        str(export_dir / audit_test.ADAPTER.INDEX_FILE),
                    )
                if receipt["receipt_kind"] == "iso-rail-gateway":
                    self.assertEqual(receipt["source_path"], raw_receipt["xml_path"])
            body = dict(summary)
            digest = body.pop("summary_sha256")
            self.assertEqual(
                digest,
                VERIFIER.sha256_hex(VERIFIER._canonical_summary_json_bytes(body)),
            )

    def test_unused_local_overrides_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            audit_test.write_export(export_dir)
            with audit_test.capture_server() as (endpoint, _requests):
                self.assertEqual(
                    audit_test.run_main(
                        [
                            "--export-dir",
                            str(export_dir),
                            "--endpoint",
                            endpoint,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((export_dir / "receipts").glob("*.receipt.json"))
            https_endpoint = "https://notary.bank.internal/archive"
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "endpoint": https_endpoint,
                        "endpoint_sha256": VERIFIER.sha256_hex(
                            https_endpoint.encode("utf-8")
                        ),
                    }
                ),
            )

            cases = (
                (
                    ["--allow-failed"],
                    "--allow-failed requires at least one failed receipt",
                ),
                (
                    ["--allow-insecure-http"],
                    "--allow-insecure-http requires at least one http:// or local/private receipt endpoint",
                ),
                (
                    ["--allow-legacy-colr007"],
                    "--allow-legacy-colr007 requires at least one rail receipt with legacy colr.007 message_type",
                ),
                (
                    ["--allow-default-profile"],
                    "--allow-default-profile requires at least one rail receipt without an explicit profile",
                ),
            )
            for flags, expected in cases:
                with self.subTest(flags=flags):
                    rc, stdout, stderr = run_verify(["--receipt", str(receipt), *flags])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(expected, stderr)

    def test_zero_record_notary_receipt_is_rejected_when_source_files_required(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            empty_index = audit_test.with_digest(
                {"version": 1, "record_count": 0, "records": []},
                audit_test.ADAPTER.INDEX_DIGEST_FIELD,
            )
            audit_test.write_export(
                export_dir,
                index=empty_index,
                store_dir=root / "store",
                write_record_sources_flag=True,
            )
            anchor = json.loads(
                (export_dir / audit_test.ADAPTER.LATEST_ANCHOR_FILE).read_text(
                    encoding="utf-8"
                )
            )
            endpoint = "http://notary.local-bank.bank/iso-anchor"
            receipt_body = {
                "version": VERIFIER.RECEIPT_VERSION,
                "receipt_kind": "iso-audit-notary",
                "published_at": "2026-06-04T00:00:00+00:00",
                "endpoint": endpoint,
                "endpoint_sha256": VERIFIER.sha256_hex(endpoint.encode("utf-8")),
                "anchor_path": str(export_dir / audit_test.ADAPTER.LATEST_ANCHOR_FILE),
                "anchor_sha256": anchor[audit_test.ADAPTER.ANCHOR_DIGEST_FIELD],
                "index_sha256": empty_index[audit_test.ADAPTER.INDEX_DIGEST_FIELD],
                "record_count": 0,
                "status_code": 200,
                "ok": True,
                "response_body_sha256": VERIFIER.sha256_hex(b"empty"),
                "response_body_preview": "empty",
                "error": None,
            }
            receipt_body[VERIFIER.RECEIPT_DIGEST_FIELD] = VERIFIER.sha256_hex(
                VERIFIER._canonical_json_bytes(receipt_body)
            )
            receipt = export_dir / "empty-notary.receipt.json"
            receipt.write_text(
                json.dumps(receipt_body, indent=2) + "\n",
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--allow-insecure-http",
                    "--require-source-files",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn(
                "record_count must be positive when source files are required",
                stderr,
            )

            rc, stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(summary["receipts"][0]["record_count"], 0)
            self.assertTrue(
                summary["receipts"][0]["endpoint_requires_insecure_http"]
            )

    def test_source_missing_receipts_reject_all_zero_digest_placeholders(self):
        def write_digest_bound_receipt(path, body):
            body.pop(VERIFIER.RECEIPT_DIGEST_FIELD, None)
            body[VERIFIER.RECEIPT_DIGEST_FIELD] = VERIFIER.sha256_hex(
                VERIFIER._canonical_json_bytes(body)
            )
            path.write_text(json.dumps(body, indent=2) + "\n", encoding="utf-8")
            return path

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            notary_endpoint = "http://notary.local-bank.bank/iso-anchor"
            notary_index = "1" * 64
            notary_body = {
                "version": VERIFIER.RECEIPT_VERSION,
                "receipt_kind": "iso-audit-notary",
                "published_at": "2026-06-04T00:00:00+00:00",
                "endpoint": notary_endpoint,
                "endpoint_sha256": VERIFIER.sha256_hex(notary_endpoint.encode("utf-8")),
                "anchor_path": str(root / "notary" / "anchors" / f"{notary_index}.notary.json"),
                "anchor_sha256": "2" * 64,
                "index_sha256": notary_index,
                "record_count": 1,
                "status_code": 202,
                "ok": True,
                "response_body_sha256": "3" * 64,
                "response_body_preview": "accepted",
                "error": None,
            }
            rail_endpoint = "http://rail.local-bank.bank/v1/iso20022"
            rail_xml_path = root / "rail" / "rail-status.xml"
            rail_body = {
                "version": VERIFIER.RECEIPT_VERSION,
                "receipt_kind": "iso-rail-gateway",
                "submitted_at": "2026-06-04T00:00:00+00:00",
                "endpoint_url": rail_endpoint,
                "endpoint_sha256": VERIFIER.sha256_hex(rail_endpoint.encode("utf-8")),
                "message_type": "pacs.002",
                "payload_sha256": "4" * 64,
                "profile": "swift-cbpr-plus",
                "rail_message_id": "rail-drop-1",
                "xml_path": str(rail_xml_path),
                "sidecar_path": str(rail_xml_path.with_suffix(".xml.json")),
                "status_code": 202,
                "ok": True,
                "response_body_sha256": "5" * 64,
                "response_body_preview": "accepted",
                "error": None,
            }
            cases = (
                (
                    "notary-anchor",
                    dict(notary_body),
                    lambda body: body.__setitem__("anchor_sha256", "0" * 64),
                    "anchor_sha256 must not be all zero",
                ),
                (
                    "notary-index",
                    dict(notary_body),
                    lambda body: body.__setitem__("index_sha256", "0" * 64),
                    "index_sha256 must not be all zero",
                ),
                (
                    "rail-payload",
                    dict(rail_body),
                    lambda body: body.__setitem__("payload_sha256", "0" * 64),
                    "payload_sha256 must not be all zero",
                ),
            )
            for name, body, mutate, message in cases:
                with self.subTest(name=name):
                    mutate(body)
                    receipt = write_digest_bound_receipt(
                        root / f"{name}.receipt.json",
                        body,
                    )

                    rc, stdout, stderr = run_verify(
                        ["--receipt", str(receipt), "--allow-insecure-http"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_audit_index_record_digests_reject_all_zero_placeholders(self):
        index = audit_test.sample_index()
        cases = (
            (
                "record-digest",
                lambda body: body["records"][0].__setitem__("record_sha256", "0" * 64),
                "record_sha256 must not be all zero",
            ),
            (
                "payload-hash",
                lambda body: body["records"][0].__setitem__("payload_hash", "0" * 64),
                "payload_hash must not be all zero",
            ),
        )
        for name, mutate, message in cases:
            with self.subTest(name=name):
                body = json.loads(json.dumps(index))
                mutate(body)
                body = audit_test.with_digest(body, audit_test.ADAPTER.INDEX_DIGEST_FIELD)

                with self.assertRaisesRegex(VERIFIER.ReceiptError, message):
                    VERIFIER._verify_audit_index_source(body, "index")

    def test_persisted_record_metadata_rejects_all_zero_payload_hash(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            record = audit_test.sample_record()
            record["payload_hash"] = "0" * 64
            source = audit_test.sample_persisted_record(record)
            record["record_sha256"] = source[audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD]
            source_path = root / record["filename"]
            source_path.write_text(
                json.dumps(source, indent=2) + "\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                VERIFIER.ReceiptError,
                "metadata.payload_hash must not be all zero",
            ):
                VERIFIER._verify_persisted_record_source(
                    record,
                    source_path,
                    f"{source_path}",
                )

    def test_duplicate_receipt_paths_and_digests_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))

            rc, _stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--receipt",
                    str(receipt),
                    "--allow-insecure-http",
                ]
            )
            self.assertEqual(rc, 2)
            self.assertIn("duplicates receipt[0]", stderr)

            secret_receipt = receipt.with_name("token=receipt-duplicate-secret.receipt.json")
            secret_receipt.write_bytes(receipt.read_bytes())
            rc, _stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(secret_receipt),
                    "--receipt",
                    str(secret_receipt),
                    "--allow-insecure-http",
                ]
            )
            self.assertEqual(rc, 2)
            self.assertIn("secret-looking material", stderr)
            self.assertNotIn("token=", stderr)
            self.assertNotIn("receipt-duplicate-secret", stderr)

            copied = receipt.with_name("copied.receipt.json")
            copied.write_bytes(receipt.read_bytes())
            rc, _stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--receipt",
                    str(copied),
                    "--allow-insecure-http",
                ]
            )
            self.assertEqual(rc, 2)
            self.assertIn("receipt_sha256 duplicates", stderr)

            secret_copied = receipt.with_name("token=receipt-digest-secret.receipt.json")
            secret_copied.write_bytes(receipt.read_bytes())
            rc, _stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--receipt",
                    str(secret_copied),
                    "--allow-insecure-http",
                ]
            )
            self.assertEqual(rc, 2)
            self.assertIn("secret-looking material", stderr)
            self.assertNotIn("token=", stderr)
            self.assertNotIn("receipt-digest-secret", stderr)

    def test_raw_receipt_digest_rejects_all_zero_placeholder(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            body = json.loads(receipt.read_text(encoding="utf-8"))
            actual_digest = body["receipt_sha256"]
            body["receipt_sha256"] = "0" * 64
            zero_receipt = receipt.with_name("zero-digest.receipt.json")
            zero_receipt.write_text(json.dumps(body, indent=2) + "\n", encoding="utf-8")

            rc, _stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(zero_receipt),
                    "--allow-insecure-http",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("receipt_sha256 must not be all zero", stderr)
            self.assertNotIn(actual_digest, stderr)
            self.assertNotIn("mismatch", stderr)

    def test_symlinked_receipt_file_ancestor_is_rejected_before_read(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            target_dir = inbox / "receipt-target"
            target_dir.mkdir()
            target = target_dir / receipt.name
            target.write_bytes(receipt.read_bytes())
            ancestor = inbox / "receipt-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(ancestor / target.name),
                    "--allow-insecure-http",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)

    def test_non_regular_receipt_dirs_are_rejected_before_discovery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            receipt_dir = root / "receipts"
            receipt_dir.mkdir()
            receipt_file = root / "receipt-dir-as-file"
            receipt_file.write_text("not a directory\n", encoding="utf-8")
            receipt_link = root / "receipt-dir-link"
            try:
                receipt_link.symlink_to(receipt_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            cases = [
                (receipt_link, "must not be a symlink"),
                (receipt_file, "is not a directory"),
            ]
            for path, message in cases:
                with self.subTest(path=path.name):
                    rc, _stdout, stderr = run_verify(
                        ["--receipt-dir", str(path), "--allow-insecure-http"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_symlinked_receipt_dir_ancestor_is_rejected_before_discovery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target_dir = root / "receipt-target"
            target_dir.mkdir()
            ancestor = root / "receipt-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            receipt_dir = ancestor / "receipts"

            rc, stdout, stderr = run_verify(
                ["--receipt-dir", str(receipt_dir), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)

    def test_receipt_cli_paths_reject_raw_smuggling_before_discovery(self):
        cases = (
            ("receipt semicolon", "--receipt", "bad;debug.receipt.json", "semicolon path"),
            ("receipt whitespace", "--receipt", "bad receipt.json", "whitespace"),
            (
                "receipt leading dash",
                "--receipt",
                "-bad.receipt.json",
                "leading-dash path segments",
            ),
            (
                "receipt segment dash",
                "--receipt",
                "nested/-bad.receipt.json",
                "leading-dash path segments",
            ),
            (
                "receipt dot",
                "--receipt",
                lambda root: f"{root}/nested/./bad.receipt.json",
                "dot or parent",
            ),
            (
                "receipt empty",
                "--receipt",
                lambda root: f"{root}//bad.receipt.json",
                "empty path",
            ),
            ("dir semicolon", "--receipt-dir", "receipts;debug", "semicolon path"),
            ("dir whitespace", "--receipt-dir", "receipt dir", "whitespace"),
            (
                "dir segment dash",
                "--receipt-dir",
                "nested/-receipts",
                "leading-dash path segments",
            ),
            (
                "dir parent",
                "--receipt-dir",
                "nested/../receipts",
                "dot or parent",
            ),
            (
                "dir empty equals",
                "--receipt-dir",
                lambda root: f"{root}//receipts",
                "empty path",
            ),
        )
        for name, flag, raw_path, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    value = raw_path(root) if callable(raw_path) else str(root / raw_path)
                    argv = (
                        [f"{flag}={value}", "--allow-insecure-http"]
                        if "equals" in name
                        else [flag, value, "--allow-insecure-http"]
                    )

                    rc, stdout, stderr = run_verify(argv)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_secret_looking_receipt_paths_are_rejected_before_summary_output(self):
        cases = (
            (
                "--receipt",
                "token=receipt-summary-secret.receipt.json",
                "receipt-summary-secret",
            ),
            (
                "--receipt-dir",
                "token=receipt-dir-secret",
                "receipt-dir-secret",
            ),
        )
        for flag, raw_path, secret in cases:
            with self.subTest(flag=flag):
                with tempfile.TemporaryDirectory() as raw_root:
                    path = Path(raw_root) / raw_path

                    rc, stdout, stderr = run_verify(
                        [flag, str(path), "--allow-insecure-http"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn(secret, stderr)

    def test_duplicate_receipt_json_keys_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            receipt = Path(raw_root) / "receipt.json"
            receipt.write_text(
                '{"version":1,"token=receipt-duplicate-key-secret":1,"token=receipt-duplicate-key-secret":2}\n',
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)
            self.assertNotIn("receipt-duplicate-key-secret", stderr)

    def test_non_finite_receipt_json_numbers_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            receipt = Path(raw_root) / "receipt.json"
            receipt.write_text('{"version":NaN}\n', encoding="utf-8")

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("non-finite numeric constant NaN", stderr)

    def test_boolean_receipt_version_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            receipt = Path(raw_root) / "receipt.json"
            receipt.write_text('{"version":true}\n', encoding="utf-8")

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("unsupported receipt version", stderr)

    def test_receipt_json_surrogate_strings_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            receipt = Path(raw_root) / "receipt.json"
            receipt.write_text('{"version":"\\ud800"}\n', encoding="utf-8")

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("invalid Unicode surrogate", stderr)

    def test_oversized_receipt_json_is_rejected_before_parsing(self):
        with tempfile.TemporaryDirectory() as raw_root:
            receipt = Path(raw_root) / "receipt.json"
            receipt.write_bytes(oversized_json_bytes(64))

            with patched_verifier_constant("MAX_RECEIPT_JSON_BYTES", 64):
                rc, _stdout, stderr = run_verify(
                    ["--receipt", str(receipt), "--allow-insecure-http"]
                )

            self.assertEqual(rc, 2)
            self.assertIn("exceeds 64 byte JSON limit", stderr)

    def test_unknown_receipt_fields_are_rejected_even_with_valid_digest(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            rewrite_receipt(
                receipt,
                lambda body: body.update({"operator_comment": "looks fine"}),
            )

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("contains unknown keys: operator_comment", stderr)

    def test_endpoint_digest_mismatch_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            rewrite_receipt(
                receipt,
                lambda body: body.update({"endpoint_sha256": "0" * 64}),
            )

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("endpoint_sha256 does not match endpoint", stderr)

    def test_rail_receipt_required_strings_must_not_require_trimming(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            xml_path, _sidecar = rail_test.write_message(inbox)
            sidecar_path = xml_path.with_suffix(xml_path.suffix + ".json")
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            original_receipt = receipt.read_bytes()
            cases = [
                (
                    "message type whitespace",
                    lambda body: body.update({"message_type": " pacs.002"}),
                    "message_type must not have surrounding whitespace",
                ),
                (
                    "message type control",
                    lambda body: body.update({"message_type": "pacs\n002"}),
                    "message_type must not contain control characters",
                ),
                (
                    "message type malformed",
                    lambda body: body.update({"message_type": "PACS.002"}),
                    "message_type must be lowercase ISO family id",
                ),
                (
                    "message type unsupported",
                    lambda body: body.update({"message_type": "zzzz.999"}),
                    "unsupported rail message_type",
                ),
                (
                    "xml path whitespace",
                    lambda body: body.update({"xml_path": str(xml_path) + " "}),
                    "xml_path must not have surrounding whitespace",
                ),
                (
                    "xml path control",
                    lambda body: body.update({"xml_path": str(xml_path) + "\n"}),
                    "xml_path must not contain control characters",
                ),
                (
                    "xml path embedded whitespace",
                    lambda body: body.update(
                        {
                            "xml_path": str(xml_path).replace(
                                "rail-status.xml",
                                "rail status.xml",
                            )
                        }
                    ),
                    "xml_path must not contain whitespace",
                ),
                (
                    "xml path dash",
                    lambda body: body.update({"xml_path": "--rail-status.xml"}),
                    "xml_path must not start with a dash",
                ),
                (
                    "xml path segment dash",
                    lambda body: body.update(
                        {"xml_path": f"{xml_path.parent}/--{xml_path.name}"}
                    ),
                    "xml_path must not contain leading-dash path segments",
                ),
                (
                    "xml path non xml",
                    lambda body: body.update(
                        {
                            "xml_path": str(xml_path.with_suffix(".txt")),
                            "sidecar_path": str(
                                xml_path.with_suffix(".txt").with_suffix(".txt.json")
                            ),
                        }
                    ),
                    "xml_path must point to a .xml file",
                ),
                (
                    "xml path backslash",
                    lambda body: body.update(
                        {"xml_path": str(xml_path).replace("/", "\\", 1)}
                    ),
                    "xml_path must use forward slashes",
                ),
                (
                    "xml path semicolon",
                    lambda body: body.update({"xml_path": str(xml_path) + ";debug"}),
                    "xml_path must not contain semicolon path parameters",
                ),
                (
                    "xml path empty segment",
                    lambda body: body.update(
                        {"xml_path": f"{xml_path.parent}//{xml_path.name}"}
                    ),
                    "xml_path must not contain empty path segments",
                ),
                (
                    "xml path parent segment",
                    lambda body: body.update(
                        {"xml_path": f"{xml_path.parent}/../{xml_path.name}"}
                    ),
                    "xml_path must not contain dot or parent segments",
                ),
                (
                    "sidecar path whitespace",
                    lambda body: body.update({"sidecar_path": " " + str(sidecar_path)}),
                    "sidecar_path must not have surrounding whitespace",
                ),
                (
                    "sidecar path control",
                    lambda body: body.update({"sidecar_path": str(sidecar_path) + "\n"}),
                    "sidecar_path must not contain control characters",
                ),
                (
                    "sidecar path embedded whitespace",
                    lambda body: body.update(
                        {
                            "sidecar_path": str(sidecar_path).replace(
                                "rail-status.xml.json",
                                "rail status.xml.json",
                            )
                        }
                    ),
                    "sidecar_path must not contain whitespace",
                ),
                (
                    "sidecar path dash",
                    lambda body: body.update({"sidecar_path": "--rail-status.xml.json"}),
                    "sidecar_path must not start with a dash",
                ),
                (
                    "sidecar path segment dash",
                    lambda body: body.update(
                        {"sidecar_path": f"{sidecar_path.parent}/--{sidecar_path.name}"}
                    ),
                    "sidecar_path must not contain leading-dash path segments",
                ),
                (
                    "sidecar path backslash",
                    lambda body: body.update(
                        {"sidecar_path": str(sidecar_path).replace("/", "\\", 1)}
                    ),
                    "sidecar_path must use forward slashes",
                ),
                (
                    "sidecar path semicolon",
                    lambda body: body.update(
                        {"sidecar_path": str(sidecar_path) + ";debug"}
                    ),
                    "sidecar_path must not contain semicolon path parameters",
                ),
                (
                    "sidecar path empty segment",
                    lambda body: body.update(
                        {
                            "sidecar_path": (
                                f"{sidecar_path.parent}//{sidecar_path.name}"
                            )
                        }
                    ),
                    "sidecar_path must not contain empty path segments",
                ),
                (
                    "sidecar path dot segment",
                    lambda body: body.update(
                        {"sidecar_path": f"{sidecar_path.parent}/./{sidecar_path.name}"}
                    ),
                    "sidecar_path must not contain dot or parent segments",
                ),
            ]

            for label, mutate, expected in cases:
                with self.subTest(label=label):
                    receipt.write_bytes(original_receipt)
                    rewrite_receipt(receipt, mutate)

                    rc, _stdout, stderr = run_verify(
                        [
                            "--receipt",
                            str(receipt),
                            "--allow-insecure-http",
                            "--require-source-files",
                        ]
                    )

            self.assertEqual(rc, 2)
            self.assertIn(expected, stderr)

    def test_non_ascii_rail_message_type_values_are_rejected_without_echo(self):
        hidden = "\u0660"
        unicode_digit_message_type = f"pacs.{hidden}{hidden}2"
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            original_receipt = receipt.read_bytes()
            receipt_body = json.loads(original_receipt.decode("utf-8"))
            sidecar_path = Path(receipt_body["sidecar_path"])
            original_sidecar = sidecar_path.read_bytes()

            cases = (
                (
                    "receipt",
                    lambda: rewrite_receipt(
                        receipt,
                        lambda body: body.update(
                            {"message_type": unicode_digit_message_type}
                        ),
                    ),
                    ["--receipt", str(receipt), "--allow-insecure-http"],
                ),
                (
                    "sidecar",
                    lambda: sidecar_path.write_text(
                        json.dumps(
                            {
                                **json.loads(original_sidecar.decode("utf-8")),
                                "message_type": unicode_digit_message_type,
                            },
                            indent=2,
                            sort_keys=True,
                        )
                        + "\n",
                        encoding="utf-8",
                    ),
                    [
                        "--receipt",
                        str(receipt),
                        "--allow-insecure-http",
                        "--require-source-files",
                    ],
                ),
            )
            for name, mutate, argv in cases:
                with self.subTest(name=name):
                    receipt.write_bytes(original_receipt)
                    sidecar_path.write_bytes(original_sidecar)
                    mutate()

                    rc, _stdout, stderr = run_verify(argv)

                    self.assertEqual(rc, 2)
                    self.assertIn("message_type must use printable ASCII", stderr)
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn(unicode_digit_message_type, stderr)

    def test_status_timestamp_and_response_metadata_are_consistent(self):
        cases = [
            (
                "ok_status_mismatch",
                lambda body: body.update({"ok": True, "status_code": 500}),
                "ok does not match status_code",
            ),
            (
                "boolean_status_code",
                lambda body: body.update({"ok": True, "status_code": True}),
                "status_code must be null or an HTTP status integer",
            ),
            (
                "too_large_status_code",
                lambda body: body.update({"ok": False, "status_code": 700}),
                "status_code must be null or an HTTP status integer",
            ),
            (
                "success_error",
                lambda body: body.update({"error": "HTTP 202"}),
                "successful receipt must not record error",
            ),
            (
                "success_error_whitespace",
                lambda body: body.update({"error": " HTTP 202"}),
                "error must not have surrounding whitespace",
            ),
            (
                "success_error_control",
                lambda body: body.update({"error": "HTTP\n202"}),
                "error must not contain control characters",
            ),
            (
                "preview_without_digest",
                lambda body: body.update(
                    {"response_body_sha256": None, "response_body_preview": "accepted"}
                ),
                "response_body_sha256 must be recorded for HTTP response",
            ),
            (
                "http_response_without_digest",
                lambda body: body.update(
                    {"response_body_sha256": None, "response_body_preview": None}
                ),
                "response_body_sha256 must be recorded for HTTP response",
            ),
            (
                "bad_response_digest",
                lambda body: body.update({"response_body_sha256": "not-a-digest"}),
                "invalid response_body_sha256",
            ),
            (
                "all_zero_response_digest",
                lambda body: body.update({"response_body_sha256": "0" * 64}),
                "response_body_sha256 must not be all zero",
            ),
            (
                "oversized_preview",
                lambda body: body.update({"response_body_preview": "x" * 4097}),
                "response_body_preview exceeds 4096 characters",
            ),
            (
                "control_preview",
                lambda body: body.update({"response_body_preview": "upstream \x1b[31m"}),
                "contains unsafe control characters",
            ),
            (
                "bearer_preview",
                lambda body: body.update(
                    {"response_body_preview": "Authorization: Bearer abc"}
                ),
                "response_body_preview contains secret-looking material",
            ),
            (
                "token_preview",
                lambda body: body.update(
                    {"response_body_preview": "upstream token=abc"}
                ),
                "response_body_preview contains secret-looking material",
            ),
            (
                "private_key_preview",
                lambda body: body.update(
                    {"response_body_preview": "private_key=abc"}
                ),
                "response_body_preview contains secret-looking material",
            ),
            (
                "private_key_hyphen_preview",
                lambda body: body.update(
                    {"response_body_preview": "private-key=abc"}
                ),
                "response_body_preview contains secret-looking material",
            ),
            (
                "password_preview",
                lambda body: body.update(
                    {"response_body_preview": "upstream password=abc"}
                ),
                "response_body_preview contains secret-looking material",
            ),
            (
                "encoded_password_preview",
                lambda body: body.update(
                    {"response_body_preview": "upstream %70assword%253Dabc"}
                ),
                "response_body_preview contains secret-looking material",
            ),
            (
                "redacted_success_preview",
                lambda body: body.update(
                    {"response_body_preview": VERIFIER.REDACTED_RESPONSE_PREVIEW}
                ),
                "successful receipt must not carry redacted response_body_preview",
            ),
            (
                "secret_error",
                lambda body: body.update(
                    {"error": "upstream token=abc"}
                ),
                "error contains secret-looking material",
            ),
            (
                "encoded_secret_error",
                lambda body: body.update(
                    {"error": "upstream %74oken%253Dabc"}
                ),
                "error contains secret-looking material",
            ),
            (
                "cookie_error",
                lambda body: body.update(
                    {"error": "Set-Cookie: session=abc"}
                ),
                "error contains secret-looking material",
            ),
            (
                "naive_timestamp",
                lambda body: body.update({"submitted_at": "2026-06-05T00:00:00"}),
                "submitted_at must include a timezone offset",
            ),
            (
                "timestamp_whitespace",
                lambda body: body.update({"submitted_at": body["submitted_at"] + " "}),
                "submitted_at must not have surrounding whitespace",
            ),
            (
                "timestamp_control",
                lambda body: body.update({"submitted_at": body["submitted_at"] + "\n"}),
                "submitted_at must not contain control characters",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            original = receipt.read_bytes()

            for name, mutate, expected in cases:
                with self.subTest(name=name):
                    receipt.write_bytes(original)
                    rewrite_receipt(receipt, mutate)

                    rc, _stdout, stderr = run_verify(
                        ["--receipt", str(receipt), "--allow-insecure-http"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)

    def test_redacted_failed_response_preview_is_verifier_acceptable(self):
        body = b'{"error":"token=rail-secret"}'
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server(status=500, body=body) as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    1,
                )

            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            receipt_body = json.loads(receipt.read_text(encoding="utf-8"))
            self.assertEqual(
                receipt_body["response_body_preview"],
                rail_test.ADAPTER.REDACTED_RESPONSE_PREVIEW,
            )

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http", "--allow-failed"]
            )

            self.assertEqual(rc, 0, stderr)

    def test_tampered_receipt_digest_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            raw = json.loads(receipt.read_text(encoding="utf-8"))
            raw["status_code"] = 500
            receipt.write_text(json.dumps(raw, indent=2), encoding="utf-8")

            rc, _stdout, _stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)

    def test_failed_receipt_requires_explicit_allow_failed(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server(status=409, body=b"duplicate") as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    1,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            original = receipt.read_bytes()

            self.assertEqual(
                run_verify(["--receipt", str(receipt), "--allow-insecure-http"])[0],
                2,
            )
            self.assertEqual(
                run_verify(
                    [
                        "--receipt",
                        str(receipt),
                        "--allow-insecure-http",
                        "--allow-failed",
                    ]
                )[0],
                0,
            )

            receipt.write_bytes(original)
            rewrite_receipt(receipt, lambda body: body.__setitem__("error", None))
            rc, _stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--allow-insecure-http",
                    "--allow-failed",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("failed receipt must record error", stderr)

            receipt.write_bytes(original)
            rewrite_receipt(
                receipt,
                lambda body: body.update({"status_code": None, "ok": False}),
            )
            rc, _stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--allow-insecure-http",
                    "--allow-failed",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("response_body_sha256 requires HTTP status_code", stderr)

            receipt.write_bytes(original)
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "status_code": None,
                        "ok": False,
                        "response_body_sha256": None,
                        "response_body_preview": None,
                        "error": "invalid HTTP status 700",
                    }
                ),
            )
            rc, stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--allow-insecure-http",
                    "--allow-failed",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            entry = json.loads(stdout)["receipts"][0]
            self.assertIsNone(entry["status_code"])
            self.assertIsNone(entry["response_body_sha256"])

    def test_source_payload_mismatch_is_rejected_when_required(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            xml_path, _sidecar = rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            xml_path.write_bytes(rail_test.SAMPLE_XML + b" changed")

            rc, _stdout, _stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--allow-insecure-http",
                    "--require-source-files",
                ]
            )

            self.assertEqual(rc, 2)

    def test_oversized_rail_source_xml_is_rejected_when_required(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            xml_path, _sidecar = rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))

            with patched_verifier_constant(
                "MAX_RAIL_XML_BYTES",
                len(rail_test.SAMPLE_XML) - 1,
            ):
                rc, _stdout, stderr = run_verify(
                    [
                        "--receipt",
                        str(receipt),
                        "--allow-insecure-http",
                        "--require-source-files",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertIn("byte payload limit", stderr)

    def test_source_sidecar_mismatches_are_rejected_when_required(self):
        def replace_with_symlink(path, target):
            if path.exists() or path.is_symlink():
                path.unlink()
            try:
                path.symlink_to(target)
            except OSError as error:
                raise unittest.SkipTest(f"symlink creation unavailable: {error}") from error

        def symlinked_xml(_receipt, sidecar_path):
            xml_path = sidecar_path.with_suffix("")
            copy = xml_path.with_name("rail-status.copy.xml")
            copy.write_bytes(xml_path.read_bytes())
            replace_with_symlink(xml_path, copy)

        def symlinked_sidecar(_receipt, sidecar_path):
            copy = sidecar_path.with_name("rail-status.copy.xml.json")
            copy.write_bytes(sidecar_path.read_bytes())
            replace_with_symlink(sidecar_path, copy)

        cases = [
            (
                "missing_sidecar",
                lambda receipt, sidecar_path: sidecar_path.unlink(),
                "references missing sidecar_path",
            ),
            (
                "message_type_mismatch",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "message_type": "pacs.008",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "message_type does not match source sidecar",
            ),
            (
                "unknown_sidecar_key",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "operator_note": "accepted",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "contains unknown keys: operator_note",
            ),
            (
                "profile_whitespace",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "profile": " swift-cbpr-plus",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "profile must not have surrounding whitespace",
            ),
            (
                "profile_control",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "profile": "swift\ncbpr-plus",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "profile must not contain control characters",
            ),
            (
                "profile_embedded_whitespace",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "profile": "swift cbpr-plus",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "profile must not contain whitespace",
            ),
            (
                "profile_uppercase",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "profile": "Swift-CBPR-Plus",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "profile must be a canonical lowercase profile id",
            ),
            (
                "profile_underscore",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "profile": "swift_cbpr_plus",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "profile must be a canonical lowercase profile id",
            ),
            (
                "rail_message_id_whitespace",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "rail_message_id": "rail-drop-1 ",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "rail_message_id must not have surrounding whitespace",
            ),
            (
                "rail_message_id_control",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "rail_message_id": "rail\n1",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "rail_message_id must not contain control characters",
            ),
            (
                "rail_message_id_embedded_whitespace",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "rail_message_id": "rail drop 1",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "rail_message_id must not contain whitespace",
            ),
            (
                "rail_message_id_unicode",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "rail_message_id": "rail-drop-\U0001f69a",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "rail_message_id_path_separator",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "rail_message_id": "rail/drop/1",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "rail_message_id_oversized",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "rail_message_id": "a"
                            * (VERIFIER.MAX_RAIL_MESSAGE_ID_CHARS + 1),
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "rail_message_id must be at most",
            ),
            (
                "oversized_sidecar",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "rail_message_id": "a"
                            * VERIFIER.MAX_RAIL_SIDECAR_JSON_BYTES,
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "exceeds",
            ),
            (
                "symlinked_xml",
                symlinked_xml,
                "must not be a symlink",
            ),
            (
                "symlinked_sidecar",
                symlinked_sidecar,
                "must not be a symlink",
            ),
            (
                "swapped_sidecar_path",
                lambda receipt, sidecar_path: (
                    sidecar_path.with_name("copied.xml.json").write_bytes(
                        sidecar_path.read_bytes()
                    ),
                    rewrite_receipt(
                        receipt,
                        lambda body: body.update(
                            {
                                "sidecar_path": str(
                                    sidecar_path.with_name("copied.xml.json")
                                )
                            }
                        ),
                    ),
                ),
                "sidecar_path must match xml_path sidecar",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            xml_path, _sidecar = rail_test.write_message(inbox)
            sidecar_path = xml_path.with_suffix(xml_path.suffix + ".json")
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            original_receipt = receipt.read_bytes()
            original_sidecar = sidecar_path.read_bytes()

            for name, mutate, expected in cases:
                with self.subTest(name=name):
                    copied_sidecar = sidecar_path.with_name("copied.xml.json")
                    if copied_sidecar.exists():
                        copied_sidecar.unlink()
                    if xml_path.is_symlink():
                        xml_path.unlink()
                    if sidecar_path.is_symlink():
                        sidecar_path.unlink()
                    receipt.write_bytes(original_receipt)
                    xml_path.write_bytes(rail_test.SAMPLE_XML)
                    sidecar_path.write_bytes(original_sidecar)
                    mutate(receipt, sidecar_path)

                    rc, _stdout, stderr = run_verify(
                        [
                            "--receipt",
                            str(receipt),
                            "--allow-insecure-http",
                            "--require-source-files",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)

    def test_source_sidecar_null_optional_metadata_is_rejected_when_required(self):
        cases = [
            (
                "profile",
                "profile",
                ["--allow-default-profile"],
                lambda sidecar: sidecar.pop("profile"),
                "profile must be a non-empty string",
            ),
            (
                "rail_message_id",
                "rail_message_id",
                [],
                lambda sidecar: sidecar.pop("rail_message_id"),
                "rail_message_id must be a non-empty string",
            ),
        ]
        for name, field, verify_flags, prepare_sidecar, expected in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_inbox:
                inbox = Path(raw_inbox)
                rail_test.write_message(inbox)
                sidecar_path = inbox / "rail-status.xml.json"
                sidecar = json.loads(sidecar_path.read_text(encoding="utf-8"))
                prepare_sidecar(sidecar)
                sidecar_path.write_text(json.dumps(sidecar), encoding="utf-8")
                with rail_test.capture_server() as (base_url, _requests):
                    rc, _stdout, stderr = rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                            *verify_flags,
                        ]
                    )
                self.assertEqual(rc, 0, stderr)
                receipt = next((inbox / "receipts").glob("*.receipt.json"))

                sidecar[field] = None
                sidecar_path.write_text(json.dumps(sidecar), encoding="utf-8")
                rc, _stdout, stderr = run_verify(
                    [
                        "--receipt",
                        str(receipt),
                        "--allow-insecure-http",
                        "--require-source-files",
                        *verify_flags,
                    ]
                )

                self.assertEqual(rc, 2)
                self.assertIn(expected, stderr)

    def test_rail_receipt_nullable_metadata_must_be_recorded(self):
        cases = [
            (
                "profile",
                ["--allow-default-profile"],
                lambda body: body.pop("profile"),
                "profile must be recorded",
            ),
            (
                "rail_message_id",
                [],
                lambda body: body.pop("rail_message_id"),
                "rail_message_id must be recorded",
            ),
        ]
        for name, verify_flags, mutate, expected in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_inbox:
                inbox = Path(raw_inbox)
                rail_test.write_message(inbox)
                with rail_test.capture_server() as (base_url, _requests):
                    rc, _stdout, stderr = rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )
                self.assertEqual(rc, 0, stderr)
                receipt = next((inbox / "receipts").glob("*.receipt.json"))
                rewrite_receipt(receipt, mutate)

                rc, _stdout, stderr = run_verify(
                    [
                        "--receipt",
                        str(receipt),
                        "--allow-insecure-http",
                        *verify_flags,
                    ]
                )

                self.assertEqual(rc, 2)
                self.assertIn(expected, stderr)

    def test_rail_receipt_metadata_strings_must_not_require_trimming(self):
        cases = [
            (
                "receipt profile whitespace",
                lambda body: body.update({"profile": " swift-cbpr-plus"}),
                "profile must not have surrounding whitespace",
            ),
            (
                "receipt profile control",
                lambda body: body.update({"profile": "swift\ncbpr-plus"}),
                "profile must not contain control characters",
            ),
            (
                "receipt profile embedded whitespace",
                lambda body: body.update({"profile": "swift cbpr-plus"}),
                "profile must not contain whitespace",
            ),
            (
                "receipt profile uppercase",
                lambda body: body.update({"profile": "Swift-CBPR-Plus"}),
                "profile must be a canonical lowercase profile id",
            ),
            (
                "receipt profile underscore",
                lambda body: body.update({"profile": "swift_cbpr_plus"}),
                "profile must be a canonical lowercase profile id",
            ),
            (
                "receipt profile trailing hyphen",
                lambda body: body.update({"profile": "swift-cbpr-plus-"}),
                "profile must be a canonical lowercase profile id",
            ),
            (
                "receipt rail message whitespace",
                lambda body: body.update({"rail_message_id": "rail-drop-1 "}),
                "rail_message_id must not have surrounding whitespace",
            ),
            (
                "receipt rail message control",
                lambda body: body.update({"rail_message_id": "rail\n1"}),
                "rail_message_id must not contain control characters",
            ),
            (
                "receipt rail message embedded whitespace",
                lambda body: body.update({"rail_message_id": "rail drop 1"}),
                "rail_message_id must not contain whitespace",
            ),
            (
                "receipt rail message unicode",
                lambda body: body.update({"rail_message_id": "rail-drop-\U0001f69a"}),
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "receipt rail message path separator",
                lambda body: body.update({"rail_message_id": "rail/drop/1"}),
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "receipt rail message leading punctuation",
                lambda body: body.update({"rail_message_id": "-rail-drop-1"}),
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "receipt rail message trailing punctuation",
                lambda body: body.update({"rail_message_id": "rail-drop-1-"}),
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "receipt rail message oversized",
                lambda body: body.update(
                    {
                        "rail_message_id": "a"
                        * (VERIFIER.MAX_RAIL_MESSAGE_ID_CHARS + 1)
                    }
                ),
                "rail_message_id must be at most",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            original_receipt = receipt.read_bytes()

            for label, mutate, expected in cases:
                with self.subTest(label=label):
                    receipt.write_bytes(original_receipt)
                    rewrite_receipt(receipt, mutate)

                    rc, _stdout, stderr = run_verify(
                        [
                            "--receipt",
                            str(receipt),
                            "--allow-insecure-http",
                            "--require-source-files",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)

    def test_oversized_notary_source_json_files_are_rejected_when_required(self):
        def make_fixture(root):
            export_dir = root / "export"
            export_dir.mkdir()
            _index, _anchor, digest_anchor = audit_test.write_export(
                export_dir,
                store_dir=root / "store",
                write_record_sources_flag=True,
            )
            with audit_test.capture_server() as (endpoint, _requests):
                self.assertEqual(
                    audit_test.run_main(
                        [
                            "--export-dir",
                            str(export_dir),
                            "--endpoint",
                            endpoint,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((export_dir / "receipts").glob("*.receipt.json"))
            latest = export_dir / audit_test.ADAPTER.LATEST_ANCHOR_FILE
            index_file = export_dir / audit_test.ADAPTER.INDEX_FILE
            record_source = next(
                (root / "store" / audit_test.ADAPTER.RECORDS_DIR).glob("*.json")
            )
            return receipt, latest, digest_anchor, index_file, record_source

        cases = [
            "latest_anchor",
            "digest_peer",
            "exported_index",
            "record_source",
        ]
        for name in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_root:
                root = Path(raw_root)
                receipt, latest, digest_anchor, index_file, record_source = make_fixture(
                    root
                )
                latest_size = len(latest.read_bytes())
                digest_anchor_size = len(digest_anchor.read_bytes())
                index_size = len(index_file.read_bytes())
                record_size = len(record_source.read_bytes())

                if name == "latest_anchor":
                    limit = latest_size - 1
                elif name == "digest_peer":
                    limit = max(latest_size, index_size, record_size) + 64
                    digest_anchor.write_bytes(oversized_json_bytes(limit))
                elif name == "exported_index":
                    limit = max(latest_size, digest_anchor_size, record_size) + 64
                    index_file.write_bytes(oversized_json_bytes(limit))
                elif name == "record_source":
                    limit = max(latest_size, digest_anchor_size, index_size) + 64
                    record_source.write_bytes(oversized_json_bytes(limit))
                else:  # pragma: no cover - guarded by the cases table.
                    raise AssertionError(name)

                constant = (
                    "MAX_PERSISTED_RECORD_JSON_BYTES"
                    if name == "record_source"
                    else "MAX_AUDIT_EXPORT_JSON_BYTES"
                )
                with patched_verifier_constant(constant, limit):
                    rc, _stdout, stderr = run_verify(
                        [
                            "--receipt",
                            str(receipt),
                            "--allow-insecure-http",
                            "--require-source-files",
                        ]
                    )

                self.assertEqual(rc, 2)
                self.assertIn("byte JSON limit", stderr)

    def test_notary_anchor_source_mismatches_are_rejected_when_required(self):
        def mismatched_index():
            index = {
                "version": 1,
                "record_count": 1,
                "records": [audit_test.sample_record("msg-2")],
            }
            return audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)

        def replace_with_symlink(path, target):
            if path.exists() or path.is_symlink():
                path.unlink()
            try:
                path.symlink_to(target)
            except OSError as error:
                raise unittest.SkipTest(f"symlink creation unavailable: {error}") from error

        def symlinked_latest(_receipt, latest, _digest_anchor, _index_file):
            copy = latest.with_name("latest.copy.notary.json")
            copy.write_bytes(latest.read_bytes())
            replace_with_symlink(latest, copy)

        def symlinked_digest_peer(_receipt, _latest, digest_anchor, _index_file):
            copy = digest_anchor.with_name("digest.copy.notary.json")
            copy.write_bytes(digest_anchor.read_bytes())
            replace_with_symlink(digest_anchor, copy)

        def symlinked_index(_receipt, _latest, _digest_anchor, index_file):
            copy = index_file.with_name("messages.index.copy.json")
            copy.write_bytes(index_file.read_bytes())
            replace_with_symlink(index_file, copy)

        def unknown_anchor_field(receipt, latest, digest_anchor, _index_file):
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["operator_note"] = "accepted by phone"
            anchor = audit_test.with_digest(anchor, audit_test.ADAPTER.ANCHOR_DIGEST_FIELD)
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            digest_anchor.write_text(anchor_text, encoding="utf-8")
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ]
                    }
                ),
            )

        def unknown_exported_index_field(_receipt, _latest, _digest_anchor, index_file):
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["operator_note"] = "accepted by phone"
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")

        def unknown_embedded_record_field(receipt, latest, digest_anchor, index_file):
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"][0]["operator_note"] = "accepted by phone"
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["audit_index"] = index
            anchor["index_sha256"] = index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]
            anchor = audit_test.with_digest(anchor, audit_test.ADAPTER.ANCHOR_DIGEST_FIELD)
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            new_digest_anchor = digest_anchor.with_name(
                f"{index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]}.notary.json"
            )
            new_digest_anchor.write_text(anchor_text, encoding="utf-8")
            if new_digest_anchor != digest_anchor:
                digest_anchor.unlink()
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ],
                        "index_sha256": index[
                            audit_test.ADAPTER.INDEX_DIGEST_FIELD
                        ],
                    }
                ),
            )

        def missing_nullable_embedded_record_field(
            receipt,
            latest,
            digest_anchor,
            index_file,
        ):
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"][0].pop("uetr")
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["audit_index"] = index
            anchor["index_sha256"] = index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]
            anchor = audit_test.with_digest(anchor, audit_test.ADAPTER.ANCHOR_DIGEST_FIELD)
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            new_digest_anchor = digest_anchor.with_name(
                f"{index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]}.notary.json"
            )
            new_digest_anchor.write_text(anchor_text, encoding="utf-8")
            if new_digest_anchor != digest_anchor:
                digest_anchor.unlink()
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ],
                        "index_sha256": index[
                            audit_test.ADAPTER.INDEX_DIGEST_FIELD
                        ],
                    }
                ),
            )

        def malformed_exported_record_field(_receipt, _latest, _digest_anchor, index_file):
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"][0]["updated_at_ms"] = -1
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")

        def unsupported_exported_record_state(
            _receipt,
            _latest,
            _digest_anchor,
            index_file,
        ):
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"][0]["state"] = "Settled"
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")

        def mismatched_exported_record_state_code(
            _receipt,
            _latest,
            _digest_anchor,
            index_file,
        ):
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"][0]["state"] = "Rejected"
            index["records"][0]["pacs002_code"] = "ACSP"
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")

        def wrong_exported_record_filename(_receipt, _latest, _digest_anchor, index_file):
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"][0]["filename"] = "msg-1.json"
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")

        def duplicate_embedded_record(receipt, latest, digest_anchor, index_file):
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"].append(dict(index["records"][0]))
            index["record_count"] = 2
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["audit_index"] = index
            anchor["index_sha256"] = index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]
            anchor["record_count"] = 2
            anchor = audit_test.with_digest(
                anchor,
                audit_test.ADAPTER.ANCHOR_DIGEST_FIELD,
            )
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            new_digest_anchor = digest_anchor.with_name(
                f"{index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]}.notary.json"
            )
            new_digest_anchor.write_text(anchor_text, encoding="utf-8")
            if new_digest_anchor != digest_anchor:
                digest_anchor.unlink()
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ],
                        "index_sha256": index[
                            audit_test.ADAPTER.INDEX_DIGEST_FIELD
                        ],
                        "record_count": 2,
                    }
                ),
            )

        def boolean_receipt_record_count(receipt, _latest, _digest_anchor, _index_file):
            rewrite_receipt(receipt, lambda body: body.update({"record_count": True}))

        def boolean_anchor_record_count(receipt, latest, digest_anchor, _index_file):
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["record_count"] = True
            anchor = audit_test.with_digest(
                anchor,
                audit_test.ADAPTER.ANCHOR_DIGEST_FIELD,
            )
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            digest_anchor.write_text(anchor_text, encoding="utf-8")
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ]
                    }
                ),
            )

        def boolean_embedded_index_record_count(receipt, latest, digest_anchor, index_file):
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["record_count"] = True
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["audit_index"] = index
            anchor["index_sha256"] = index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]
            anchor = audit_test.with_digest(anchor, audit_test.ADAPTER.ANCHOR_DIGEST_FIELD)
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            new_digest_anchor = digest_anchor.with_name(
                f"{index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]}.notary.json"
            )
            new_digest_anchor.write_text(anchor_text, encoding="utf-8")
            if new_digest_anchor != digest_anchor:
                digest_anchor.unlink()
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ],
                        "index_sha256": index[
                            audit_test.ADAPTER.INDEX_DIGEST_FIELD
                        ],
                    }
                ),
            )

        def record_source_path(latest):
            store_dir = latest.parent.parent / "store"
            return next((store_dir / audit_test.ADAPTER.RECORDS_DIR).glob("*.json"))

        def rewrite_digest_correct_record_source(
            receipt,
            latest,
            digest_anchor,
            index_file,
            mutate_source,
        ):
            record_path = record_source_path(latest)
            source = json.loads(record_path.read_text(encoding="utf-8"))
            mutate_source(source)
            source = audit_test.with_digest(
                source,
                audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD,
            )
            record_path.write_text(json.dumps(source, indent=2) + "\n", encoding="utf-8")
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"][0]["record_sha256"] = source[
                audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD
            ]
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["audit_index"] = index
            anchor["index_sha256"] = index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]
            anchor = audit_test.with_digest(anchor, audit_test.ADAPTER.ANCHOR_DIGEST_FIELD)
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            new_digest_anchor = digest_anchor.with_name(
                f"{index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]}.notary.json"
            )
            new_digest_anchor.write_text(anchor_text, encoding="utf-8")
            if new_digest_anchor != digest_anchor:
                digest_anchor.unlink()
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ],
                        "index_sha256": index[
                            audit_test.ADAPTER.INDEX_DIGEST_FIELD
                        ],
                    }
                ),
            )

        def status_history_current_timestamp_mismatch(
            receipt,
            latest,
            digest_anchor,
            index_file,
        ):
            def mutate(source):
                source["status_history"][-1]["updated_at_ms"] = (
                    source["updated_at_ms"] - 1
                )

            rewrite_digest_correct_record_source(
                receipt,
                latest,
                digest_anchor,
                index_file,
                mutate,
            )

        def status_history_timestamp_moves_backwards(
            receipt,
            latest,
            digest_anchor,
            index_file,
        ):
            def mutate(source):
                earlier_entry = dict(source["status_history"][-1])
                earlier_entry["updated_at_ms"] = source["updated_at_ms"] + 1
                source["status_history"].insert(0, earlier_entry)

            rewrite_digest_correct_record_source(
                receipt,
                latest,
                digest_anchor,
                index_file,
                mutate,
            )

        def status_history_state_code_mismatch(
            receipt,
            latest,
            digest_anchor,
            index_file,
        ):
            def mutate(source):
                forged_entry = dict(source["status_history"][-1])
                forged_entry["status"] = "Rejected"
                forged_entry["pacs002_code"] = "ACSP"
                source["status_history"].insert(0, forged_entry)

            rewrite_digest_correct_record_source(
                receipt,
                latest,
                digest_anchor,
                index_file,
                mutate,
            )

        def missing_persisted_record_nullable_key(
            receipt,
            latest,
            digest_anchor,
            index_file,
        ):
            rewrite_digest_correct_record_source(
                receipt,
                latest,
                digest_anchor,
                index_file,
                lambda source: source.pop("detail"),
            )

        def missing_persisted_context_nullable_key(
            receipt,
            latest,
            digest_anchor,
            index_file,
        ):
            rewrite_digest_correct_record_source(
                receipt,
                latest,
                digest_anchor,
                index_file,
                lambda source: source["context"].pop("ledger_id"),
            )

        def missing_persisted_metadata_nullable_key(
            receipt,
            latest,
            digest_anchor,
            index_file,
        ):
            rewrite_digest_correct_record_source(
                receipt,
                latest,
                digest_anchor,
                index_file,
                lambda source: source["metadata"].pop("business_service"),
            )

        def missing_persisted_history_nullable_key(
            receipt,
            latest,
            digest_anchor,
            index_file,
        ):
            rewrite_digest_correct_record_source(
                receipt,
                latest,
                digest_anchor,
                index_file,
                lambda source: source["status_history"][0].pop("reason_code"),
            )

        def missing_anchor_store_dir(receipt, latest, digest_anchor, _index_file):
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["store_dir"] = None
            anchor = audit_test.with_digest(anchor, audit_test.ADAPTER.ANCHOR_DIGEST_FIELD)
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            digest_anchor.write_text(anchor_text, encoding="utf-8")
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ]
                    }
                ),
            )

        def malformed_anchor_store_dir(value):
            def mutate(receipt, latest, digest_anchor, _index_file):
                anchor = json.loads(latest.read_text(encoding="utf-8"))
                anchor["store_dir"] = value
                anchor = audit_test.with_digest(
                    anchor,
                    audit_test.ADAPTER.ANCHOR_DIGEST_FIELD,
                )
                anchor_text = json.dumps(anchor, indent=2) + "\n"
                latest.write_text(anchor_text, encoding="utf-8")
                digest_anchor.write_text(anchor_text, encoding="utf-8")
                rewrite_receipt(
                    receipt,
                    lambda body: body.update(
                        {
                            "anchor_sha256": anchor[
                                audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                            ]
                        }
                    ),
                )

            return mutate

        def missing_persisted_record_source(_receipt, latest, _digest_anchor, _index_file):
            record_source_path(latest).unlink()

        def digest_correct_record_source_mismatch(
            _receipt,
            latest,
            _digest_anchor,
            _index_file,
        ):
            record_path = record_source_path(latest)
            source = json.loads(record_path.read_text(encoding="utf-8"))
            source["transaction_hash"] = "d" * 64
            source = audit_test.with_digest(
                source,
                audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD,
            )
            record_path.write_text(json.dumps(source, indent=2) + "\n", encoding="utf-8")

        def digest_correct_record_metadata_mismatch(
            receipt,
            latest,
            digest_anchor,
            index_file,
        ):
            record_path = record_source_path(latest)
            source = json.loads(record_path.read_text(encoding="utf-8"))
            source["metadata"]["message_type"] = "pacs.009"
            source = audit_test.with_digest(
                source,
                audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD,
            )
            record_path.write_text(json.dumps(source, indent=2) + "\n", encoding="utf-8")
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"][0]["record_sha256"] = source[
                audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD
            ]
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["audit_index"] = index
            anchor["index_sha256"] = index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]
            anchor = audit_test.with_digest(anchor, audit_test.ADAPTER.ANCHOR_DIGEST_FIELD)
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            new_digest_anchor = digest_anchor.with_name(
                f"{index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]}.notary.json"
            )
            new_digest_anchor.write_text(anchor_text, encoding="utf-8")
            if new_digest_anchor != digest_anchor:
                digest_anchor.unlink()
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ],
                        "index_sha256": index[
                            audit_test.ADAPTER.INDEX_DIGEST_FIELD
                        ],
                    }
                ),
            )

        cases = [
            (
                "missing_digest_peer",
                lambda receipt, latest, digest_anchor, index_file: digest_anchor.unlink(),
                "latest anchor has no digest-addressed peer",
            ),
            (
                "different_digest_peer",
                lambda receipt, latest, digest_anchor, index_file: digest_anchor.write_text(
                    "{}\n",
                    encoding="utf-8",
                ),
                "latest anchor differs from digest-addressed peer",
            ),
            (
                "symlinked_latest_anchor",
                symlinked_latest,
                "must not be a symlink",
            ),
            (
                "symlinked_digest_peer",
                symlinked_digest_peer,
                "must not be a symlink",
            ),
            (
                "symlinked_index",
                symlinked_index,
                "must not be a symlink",
            ),
            (
                "exported_index_mismatch",
                lambda receipt, latest, digest_anchor, index_file: index_file.write_text(
                    json.dumps(mismatched_index(), indent=2) + "\n",
                    encoding="utf-8",
                ),
                "embedded audit index differs",
            ),
            (
                "unknown_anchor_field",
                unknown_anchor_field,
                "contains unknown keys: operator_note",
            ),
            (
                "boolean_receipt_record_count",
                boolean_receipt_record_count,
                "record_count must be a non-negative integer",
            ),
            (
                "boolean_anchor_record_count",
                boolean_anchor_record_count,
                "record_count must be a non-negative integer",
            ),
            (
                "boolean_embedded_index_record_count",
                boolean_embedded_index_record_count,
                "record_count must be a non-negative integer",
            ),
            (
                "unknown_exported_index_field",
                unknown_exported_index_field,
                "contains unknown keys: operator_note",
            ),
            (
                "unknown_embedded_record_field",
                unknown_embedded_record_field,
                "contains unknown keys: operator_note",
            ),
            (
                "missing_nullable_embedded_record_field",
                missing_nullable_embedded_record_field,
                "records[0] is missing required keys: uetr",
            ),
            (
                "malformed_exported_record_field",
                malformed_exported_record_field,
                "updated_at_ms must be a non-negative integer",
            ),
            (
                "unsupported_exported_record_state",
                unsupported_exported_record_state,
                "state must be Pending, Accepted, or Rejected",
            ),
            (
                "mismatched_exported_record_state_code",
                mismatched_exported_record_state_code,
                "pacs002_code is not valid for Rejected state",
            ),
            (
                "wrong_exported_record_filename",
                wrong_exported_record_filename,
                "filename must be digest-addressed",
            ),
            (
                "duplicate_embedded_record",
                duplicate_embedded_record,
                "records[1].message_id duplicates",
            ),
            (
                "missing_anchor_store_dir",
                missing_anchor_store_dir,
                "store_dir is required to verify audit records",
            ),
            (
                "anchor_store_dir_whitespace",
                malformed_anchor_store_dir(str(Path("/ops/iso store"))),
                "store_dir must not contain whitespace",
            ),
            (
                "anchor_store_dir_checked_in_fixture",
                malformed_anchor_store_dir("/ops/release/fixtures/iso20022/notary-store"),
                "store_dir must not point to checked-in ISO fixture artifacts",
            ),
            (
                "anchor_store_dir_dash",
                malformed_anchor_store_dir("--store"),
                "store_dir must not start with a dash",
            ),
            (
                "anchor_store_dir_segment_dash",
                malformed_anchor_store_dir("/ops/--store"),
                "store_dir must not contain leading-dash path segments",
            ),
            (
                "anchor_store_dir_parent_segment",
                malformed_anchor_store_dir("/ops/iso/../store"),
                "store_dir must not contain dot or parent segments",
            ),
            (
                "missing_persisted_record_source",
                missing_persisted_record_source,
                "references missing audit record",
            ),
            (
                "digest_correct_record_source_mismatch",
                digest_correct_record_source_mismatch,
                "record_sha256 does not match audit index record",
            ),
            (
                "digest_correct_record_metadata_mismatch",
                digest_correct_record_metadata_mismatch,
                "metadata.message_type does not match audit index record",
            ),
            (
                "status_history_current_timestamp_mismatch",
                status_history_current_timestamp_mismatch,
                "status_history does not end at current updated_at_ms",
            ),
            (
                "status_history_timestamp_moves_backwards",
                status_history_timestamp_moves_backwards,
                "status_history[1].updated_at_ms must not move backwards",
            ),
            (
                "status_history_state_code_mismatch",
                status_history_state_code_mismatch,
                "status_history[0].pacs002_code is not valid for Rejected status",
            ),
            (
                "missing_persisted_record_nullable_key",
                missing_persisted_record_nullable_key,
                "is missing required keys: detail",
            ),
            (
                "missing_persisted_context_nullable_key",
                missing_persisted_context_nullable_key,
                "context is missing required keys: ledger_id",
            ),
            (
                "missing_persisted_metadata_nullable_key",
                missing_persisted_metadata_nullable_key,
                "metadata is missing required keys: business_service",
            ),
            (
                "missing_persisted_history_nullable_key",
                missing_persisted_history_nullable_key,
                "status_history[0] is missing required keys: reason_code",
            ),
            (
                "wrong_anchor_filename",
                lambda receipt, latest, digest_anchor, index_file: (
                    digest_anchor.with_name("wrong.notary.json").write_bytes(
                        latest.read_bytes()
                    ),
                    rewrite_receipt(
                        receipt,
                        lambda body: body.update(
                            {
                                "anchor_path": str(
                                    digest_anchor.with_name("wrong.notary.json")
                                )
                            }
                        ),
                    ),
                ),
                "anchor_path filename must be digest-addressed",
            ),
            (
                "anchor_path_whitespace",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update({"anchor_path": str(latest) + " "}),
                ),
                "anchor_path must not have surrounding whitespace",
            ),
            (
                "anchor_path_control",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update({"anchor_path": str(latest) + "\n"}),
                ),
                "anchor_path must not contain control characters",
            ),
            (
                "anchor_path_embedded_whitespace",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update(
                        {
                            "anchor_path": str(latest).replace(
                                "latest.notary.json",
                                "latest notary.json",
                            )
                        }
                    ),
                ),
                "anchor_path must not contain whitespace",
            ),
            (
                "anchor_path_dash",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update({"anchor_path": "--latest.notary.json"}),
                ),
                "anchor_path must not start with a dash",
            ),
            (
                "anchor_path_segment_dash",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update({"anchor_path": f"{latest.parent}/--{latest.name}"}),
                ),
                "anchor_path must not contain leading-dash path segments",
            ),
            (
                "anchor_path_backslash",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update(
                        {"anchor_path": str(latest).replace("/", "\\", 1)}
                    ),
                ),
                "anchor_path must use forward slashes",
            ),
            (
                "anchor_path_semicolon",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update({"anchor_path": str(latest) + ";debug"}),
                ),
                "anchor_path must not contain semicolon path parameters",
            ),
            (
                "anchor_path_empty_segment",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update(
                        {"anchor_path": f"{latest.parent}//{latest.name}"}
                    ),
                ),
                "anchor_path must not contain empty path segments",
            ),
            (
                "anchor_path_parent_segment",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update(
                        {"anchor_path": f"{latest.parent}/../{latest.name}"}
                    ),
                ),
                "anchor_path must not contain dot or parent segments",
            ),
            (
                "anchor_path_checked_in_fixture",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update(
                        {
                            "anchor_path": (
                                "/ops/release/fixtures/iso20022/latest.notary.json"
                            )
                        }
                    ),
                ),
                "anchor_path must not point to checked-in ISO fixture artifacts",
            ),
            (
                "published_at_whitespace",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update({"published_at": body["published_at"] + " "}),
                ),
                "published_at must not have surrounding whitespace",
            ),
            (
                "published_at_control",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update({"published_at": body["published_at"] + "\n"}),
                ),
                "published_at must not contain control characters",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            _index, _anchor, digest_anchor = audit_test.write_export(
                export_dir,
                store_dir=root / "store",
                write_record_sources_flag=True,
            )
            with audit_test.capture_server() as (endpoint, _requests):
                self.assertEqual(
                    audit_test.run_main(
                        [
                            "--export-dir",
                            str(export_dir),
                            "--endpoint",
                            endpoint,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((export_dir / "receipts").glob("*.receipt.json"))
            latest = export_dir / audit_test.ADAPTER.LATEST_ANCHOR_FILE
            index_file = export_dir / audit_test.ADAPTER.INDEX_FILE
            original_receipt = receipt.read_bytes()
            original_latest = latest.read_bytes()
            original_digest_anchor = digest_anchor.read_bytes()
            original_index = index_file.read_bytes()
            store_dir = root / "store"
            record_source = next((store_dir / audit_test.ADAPTER.RECORDS_DIR).glob("*.json"))
            original_record_source = record_source.read_bytes()

            for name, mutate, expected in cases:
                with self.subTest(name=name):
                    wrong_anchor = digest_anchor.with_name("wrong.notary.json")
                    if wrong_anchor.exists():
                        wrong_anchor.unlink()
                    for path in (latest, digest_anchor, index_file):
                        if path.is_symlink():
                            path.unlink()
                    if record_source.is_symlink():
                        record_source.unlink()
                    receipt.write_bytes(original_receipt)
                    latest.write_bytes(original_latest)
                    digest_anchor.write_bytes(original_digest_anchor)
                    index_file.write_bytes(original_index)
                    record_source.write_bytes(original_record_source)
                    mutate(receipt, latest, digest_anchor, index_file)

                    rc, _stdout, stderr = run_verify(
                        [
                            "--receipt",
                            str(receipt),
                            "--allow-insecure-http",
                            "--require-source-files",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)
                    if name == "duplicate_embedded_record":
                        self.assertNotIn("msg-1", stderr)

    def test_missing_notary_anchor_path_must_keep_digest_addressed_shape(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            _index, _anchor, digest_anchor = audit_test.write_export(export_dir)
            with audit_test.capture_server() as (endpoint, _requests):
                self.assertEqual(
                    audit_test.run_main(
                        [
                            "--export-dir",
                            str(export_dir),
                            "--endpoint",
                            endpoint,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((export_dir / "receipts").glob("*.receipt.json"))
            original_receipt = receipt.read_bytes()
            index_sha256 = digest_anchor.name.removesuffix(".notary.json")
            cases = [
                (
                    export_dir / "missing.notary.json",
                    "anchor_path must be latest.notary.json or anchors/<index_sha256>.notary.json",
                ),
                (
                    export_dir / "anchors" / "wrong.notary.json",
                    "anchor_path filename must be digest-addressed",
                ),
                (
                    export_dir / "anchors" / f"{index_sha256}.json",
                    "anchor_path filename must be digest-addressed",
                ),
            ]
            for anchor_path, expected in cases:
                with self.subTest(anchor_path=anchor_path.name):
                    receipt.write_bytes(original_receipt)
                    rewrite_receipt(
                        receipt,
                        lambda body, anchor_path=anchor_path: body.update(
                            {"anchor_path": str(anchor_path)}
                        ),
                    )
                    if anchor_path.exists():
                        anchor_path.unlink()

                    rc, _stdout, stderr = run_verify(
                        ["--receipt", str(receipt), "--allow-insecure-http"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)

    def test_rail_xml_path_rejects_checked_in_iso_fixtures(self):
        checked_in_fixture = REPO_ROOT / "fixtures" / "iso20022" / "pacs008_fixture.xml"
        cases = (
            "fixtures/iso20022/pacs008_fixture.xml",
            str(checked_in_fixture),
            "/ops/release/fixtures/iso20022/pacs008_fixture.xml",
        )
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            original_receipt = receipt.read_bytes()

            for source_path in cases:
                with self.subTest(source_path=source_path):
                    receipt.write_bytes(original_receipt)
                    rewrite_receipt(
                        receipt,
                        lambda body, path=source_path: body.update(
                            {"xml_path": path}
                        ),
                    )

                    rc, _stdout, stderr = run_verify(
                        [
                            "--receipt",
                            str(receipt),
                            "--allow-insecure-http",
                            "--require-source-files",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("checked-in ISO XML fixtures", stderr)

    def test_legacy_colr007_rail_receipts_require_explicit_local_override(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(
                inbox,
                message_type="colr.007",
                profile="securities-csd",
                payload=b"<Document><CollSbstitnConf/></Document>",
            )
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                            "--allow-legacy-colr007",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )
            self.assertEqual(rc, 2)
            self.assertIn("legacy rail message_type", stderr)

            rc, stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--allow-insecure-http",
                    "--allow-legacy-colr007",
                    "--require-source-files",
                ]
            )
            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["allow_legacy_colr007"])
            self.assertEqual(summary["receipts"][0]["message_type"], "colr.007")

    def test_default_profile_rail_receipts_require_explicit_local_override(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox, profile=None)
            sidecar_path = inbox / "rail-status.xml.json"
            sidecar = json.loads(sidecar_path.read_text(encoding="utf-8"))
            sidecar.pop("profile")
            sidecar_path.write_text(json.dumps(sidecar), encoding="utf-8")
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                            "--allow-default-profile",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )
            self.assertEqual(rc, 2)
            self.assertIn("omitted rail profile", stderr)

            rc, stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--allow-insecure-http",
                    "--allow-default-profile",
                    "--require-source-files",
                ]
            )
            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["allow_default_profile"])
            self.assertIsNone(summary["receipts"][0]["profile"])
            self.assertEqual(summary["receipts"][0]["rail_message_id"], "rail-drop-1")

    def test_secret_material_in_receipt_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
            )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            rewrite_receipt(
                receipt,
                lambda body: body.update({"password_receipt_field_secret": "redacted"}),
            )

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("forbidden secret-looking field", stderr)
            self.assertNotIn("password", stderr)
            self.assertNotIn("receipt_field_secret", stderr)
            self.assertNotIn("receipt-field-secret", stderr)

    def test_nested_secret_material_in_receipt_is_rejected_without_echo(self):
        cases = (
            (
                {"metadata": {"private-key_receipt_nested_leak": "redacted"}},
                "receipt_nested_leak",
            ),
            (
                {"metadata": [{"x_iroha_signature_receipt_nested_leak": "redacted"}]},
                "receipt_nested_leak",
            ),
            (
                {"metadata": {"note": "%70assword%253Dreceipt-nested-leak"}},
                "receipt-nested-leak",
            ),
        )
        for body, secret in cases:
            with self.subTest(body=body):
                with self.assertRaises(VERIFIER.ReceiptError) as caught:
                    VERIFIER._check_no_secret_material(body, Path("receipt.json"))

                message = str(caught.exception)
                self.assertIn("secret-looking", message)
                self.assertNotIn("private-key", message)
                self.assertNotIn("x_iroha_signature", message)
                self.assertNotIn("%70assword%253Dreceipt-nested-leak", message)
                self.assertNotIn("password=receipt-nested-leak", message)
                self.assertNotIn(secret, message)

    def test_secret_material_in_allowed_receipt_values_is_rejected_without_echo(self):
        cases = [
            (
                "receipt_kind",
                "token=receipt-value-secret",
            ),
            (
                "message_type",
                "token=receipt-value-secret",
            ),
            (
                "profile",
                "private_key=receipt-value-secret",
            ),
            (
                "profile",
                "password=receipt-value-secret",
            ),
            (
                "profile",
                "token%3Dreceipt-value-secret",
            ),
            (
                "profile",
                "%70assword%253Dreceipt-value-secret",
            ),
            (
                "rail_message_id",
                "Authorization: Bearer receipt-value-secret",
            ),
            (
                "message_type",
                "X-Iroha-Signature: receipt-value-secret",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            original = receipt.read_bytes()

            for field, value in cases:
                with self.subTest(field=field):
                    receipt.write_bytes(original)
                    rewrite_receipt(
                        receipt,
                        lambda body, field=field, value=value: body.update(
                            {field: value}
                        ),
                    )

                    rc, _stdout, stderr = run_verify(
                        ["--receipt", str(receipt), "--allow-insecure-http"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn(field, stderr)
                    self.assertNotIn("password=", stderr)
                    self.assertNotIn(value, stderr)
                    self.assertNotIn("receipt-value-secret", stderr)

    def test_secret_looking_receipt_identifiers_are_rejected_without_echo(self):
        cases = (
            ("receipt_kind", "token-receipt-kind-secret", "receipt-kind-secret"),
            ("message_type", "token-receipt-message-type-secret", "receipt-message-type-secret"),
            ("profile", "token-receipt-profile-secret", "receipt-profile-secret"),
            ("rail_message_id", "session-key-receipt-message-secret", "receipt-message-secret"),
        )
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            original = receipt.read_bytes()

            for field, value, hidden in cases:
                with self.subTest(field=field):
                    receipt.write_bytes(original)
                    rewrite_receipt(
                        receipt,
                        lambda body, field=field, value=value: body.update(
                            {field: value}
                        ),
                    )

                    rc, _stdout, stderr = run_verify(
                        ["--receipt", str(receipt), "--allow-insecure-http"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(f"{field} must not contain secret-looking material", stderr)
                    self.assertNotIn(value, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_non_ascii_receipt_kind_is_rejected_without_echo(self):
        hidden = "iso-rail-gatew\u0430y"
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                            "--receipt-dir",
                            str(inbox / "receipts"),
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            rewrite_receipt(receipt, lambda body: body.update({"receipt_kind": hidden}))

            rc, _stdout, stderr = run_verify(["--receipt", str(receipt), "--allow-insecure-http"])

            self.assertEqual(rc, 2)
            self.assertIn("receipt_kind must use printable ASCII", stderr)
            self.assertNotIn(hidden, stderr)
            self.assertNotIn("unsupported receipt_kind", stderr)

    def test_endpoint_urls_reject_secret_path_without_echo(self):
        cases = (
            "https://torii.example.invalid/base/token=receipt-url-secret",
            "https://torii.example.invalid/base/token-receipt-url-secret",
            "https://torii.example.invalid/base/token%3Dreceipt-url-secret",
            "https://torii.example.invalid/base/token%253Dreceipt-url-secret",
        )
        for url in cases:
            with self.subTest(url=url):
                with self.assertRaises(VERIFIER.ReceiptError) as caught:
                    VERIFIER._require_https(
                        url,
                        allow_insecure_http=False,
                        label="receipt",
                    )

                message = str(caught.exception)
                self.assertIn("secret-looking material", message)
                self.assertNotIn(url, message)
                self.assertNotIn("token=", message)
                self.assertNotIn("receipt-url-secret", message)

    def test_endpoint_urls_reject_secret_host_and_parser_errors_without_echo(self):
        cases = (
            (
                "https://token-receipt-host-secret.torii.example.invalid/base",
                "secret-looking material",
            ),
            ("https://[token-receipt-host-secret/base", "is not valid"),
        )
        for url, expected in cases:
            with self.subTest(url=url):
                with self.assertRaises(VERIFIER.ReceiptError) as caught:
                    VERIFIER._require_https(
                        url,
                        allow_insecure_http=False,
                        label="receipt",
                    )

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(url, message)
                self.assertNotIn("token-receipt-host-secret", message)

    def test_smuggled_receipt_endpoint_urls_are_rejected(self):
        cases = [
            ("notary", "endpoint", " https://notary.example/anchor", False),
            ("notary", "endpoint", "https://notary.example/anchor ", False),
            ("notary", "endpoint", "https://user:pass@notary.example/anchor", False),
            ("notary", "endpoint", "https://notary.example/anchor;debug", False),
            ("notary", "endpoint", "https://notary.example/anchor?debug=true", False),
            ("notary", "endpoint", "https://notary.example/anchor#fragment", False),
            ("notary", "endpoint", "https://notary.example/anchor", False),
            ("notary", "endpoint", "https://notary.example.com/anchor", False),
            ("notary", "endpoint", "https://notary.example.net/anchor", False),
            ("notary", "endpoint", "https://notary.example.org/anchor", False),
            ("notary", "endpoint", "https://notary.example.invalid/anchor", False),
            (
                "notary",
                "endpoint",
                "https://notary.swift-cbpr-plus.operator-canary.bank/anchor",
                False,
            ),
            (
                "notary",
                "endpoint",
                "http://notary.swift-cbpr-plus.operator-canary.bank/anchor",
                True,
            ),
            ("notary", "endpoint", "https:///anchor", False),
            ("notary", "endpoint", "https://[::1", False),
            ("notary", "endpoint", "https://notary.example/anc\nhor", False),
            ("notary", "endpoint", "https://notary.example/iso anchor", False),
            ("notary", "endpoint", "https://notary.example:abc/anchor", False),
            ("notary", "endpoint", "https://notary.example:/anchor", False),
            ("notary", "endpoint", "https://notary.example:0/anchor", False),
            ("notary", "endpoint", "https://notary.example:08443/anchor", False),
            ("notary", "endpoint", "https://notary.example:99999/anchor", False),
            ("notary", "endpoint", "https://notary.example:443/anchor", False),
            ("notary", "endpoint", "https://Notary.example/anchor", False),
            ("notary", "endpoint", "https://notary.example./anchor", False),
            ("notary", "endpoint", "https://notary..example/anchor", False),
            ("notary", "endpoint", "https://localhost/anchor", False),
            ("notary", "endpoint", "https://10.1.2.3/anchor", False),
            ("notary", "endpoint", "https://10.1.2.3.sslip.io/anchor", False),
            ("notary", "endpoint", "https://0x7f.0.0.1/anchor", False),
            ("notary", "endpoint", "https://[::127.0.0.1]/anchor", False),
            ("notary", "endpoint", "https://-notary.example/anchor", False),
            ("notary", "endpoint", "https://notary-.example/anchor", False),
            ("notary", "endpoint", "https://notary._tcp.example/anchor", False),
            ("notary", "endpoint", "https://notary.example%2einvalid/anchor", False),
            ("notary", "endpoint", "https://123.000.000.001/anchor", False),
            ("notary", "endpoint", "https://notary.example/../anchor", False),
            ("notary", "endpoint", "https://notary.example/archive//anchor", False),
            ("notary", "endpoint", "https://notary.example/%2e%2e/anchor", False),
            ("notary", "endpoint", "https://notary.example/archive%2fanchor", False),
            ("notary", "endpoint", "https://notary.example/archive%252fanchor", False),
            ("notary", "endpoint", "https://notary.example/archive;debug/anchor", False),
            ("notary", "endpoint", "https://notary.example/archive%3bdebug/anchor", False),
            ("notary", "endpoint", "https://notary.example/archive%23debug/anchor", False),
            ("notary", "endpoint", "https://notary.example/archive%20anchor", False),
            ("notary", "endpoint", "https://notary.example/archive%00anchor", False),
            ("notary", "endpoint", "https://notary.example/archive%7fanchor", False),
            ("notary", "endpoint", "https://notary.example/archive%zzanchor", False),
            (
                "notary",
                "endpoint",
                "https://notary.example/" + ("a" * VERIFIER.MAX_HTTP_URL_CHARS),
                False,
            ),
            (
                "notary",
                "endpoint",
                "https://" + ".".join(["a" * 63] * 5) + "/anchor",
                False,
            ),
            ("rail", "endpoint_url", "https://localhost/v1/iso20022", False),
            ("rail", "endpoint_url", "https://127.0.0.1/v1/iso20022", False),
            ("rail", "endpoint_url", "https://127.0.0.1.nip.io/v1/iso20022", False),
            ("rail", "endpoint_url", "https://0x7f000001/v1/iso20022", False),
            ("rail", "endpoint_url", "https://[64:ff9b::7f00:1]/v1/iso20022", False),
            ("rail", "endpoint_url", "https://rail.example/v1/iso20022", False),
            ("rail", "endpoint_url", "https://rail.example.com/v1/iso20022", False),
            ("rail", "endpoint_url", "https://rail.example.net/v1/iso20022", False),
            ("rail", "endpoint_url", "https://rail.example.org/v1/iso20022", False),
            ("rail", "endpoint_url", "https://rail.example.invalid/v1/iso20022", False),
            (
                "rail",
                "endpoint_url",
                "https://rail.swift-cbpr-plus.operator-canary.bank/v1/iso20022",
                False,
            ),
            ("rail", "endpoint_url", "https://rail.example:/v1/iso20022", False),
            ("rail", "endpoint_url", "https://rail.example:0/v1/iso20022", False),
            ("rail", "endpoint_url", "https://rail.example:08443/v1/iso20022", False),
            ("rail", "endpoint_url", "https://rail.example/v1%3bdebug/iso20022", False),
            ("rail", "endpoint_url", "https://rail.example/v1%3fdebug/iso20022", False),
            ("rail", "endpoint_url", "http://127.0.0.1/v1/iso20022 ", True),
            ("rail", "endpoint_url", "http://user:pass@127.0.0.1/v1/iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1/v1/iso20022?debug=true", True),
            ("rail", "endpoint_url", "http://127.0.0.1/v1/iso20022 pacs008", True),
            ("rail", "endpoint_url", "http://127.0.0.1:abc/v1/iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:80/v1/iso20022", True),
            ("rail", "endpoint_url", "http://LocalHost:8080/v1/iso20022", True),
            ("rail", "endpoint_url", "http://localhost.:8080/v1/iso20022", True),
            ("rail", "endpoint_url", "http://local_host:8080/v1/iso20022", True),
            ("rail", "endpoint_url", "http://127.000.000.001:8080/v1/iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1/../iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1//iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1/%2e%2e/iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1%2fiso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1%252fiso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1%20iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1%00iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1%zziso20022", True),
            ("rail", "endpoint_url", r"http://127.0.0.1:8080/v1\iso20022", True),
            (
                "rail",
                "endpoint_url",
                "http://rail.swift-cbpr-plus.operator-canary.bank/v1/iso20022",
                True,
            ),
            (
                "rail",
                "endpoint_url",
                "http://127.0.0.1:8080/" + ("a" * VERIFIER.MAX_HTTP_URL_CHARS),
                True,
            ),
            (
                "rail",
                "endpoint_url",
                "http://" + ".".join(["a" * 63] * 5) + ":8080/v1/iso20022",
                True,
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            audit_test.write_export(export_dir)
            with audit_test.capture_server() as (endpoint, _requests):
                self.assertEqual(
                    audit_test.run_main(
                        [
                            "--export-dir",
                            str(export_dir),
                            "--endpoint",
                            endpoint,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            notary_receipt = next((export_dir / "receipts").glob("*.receipt.json"))

            inbox = root / "inbox"
            inbox.mkdir()
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            rail_receipt = next((inbox / "receipts").glob("*.receipt.json"))

            for kind, field, url, allow_insecure in cases:
                with self.subTest(kind=kind, url=url, allow_insecure=allow_insecure):
                    receipt = notary_receipt if kind == "notary" else rail_receipt
                    rewrite_receipt(receipt, lambda body, field=field, url=url: body.update({field: url}))
                    argv = ["--receipt", str(receipt)]
                    if allow_insecure:
                        argv.append("--allow-insecure-http")
                    rc, _stdout, stderr = run_verify(argv)

                    self.assertEqual(rc, 2)
                    self.assertIn("error:", stderr)

    def test_rejected_receipt_endpoint_url_does_not_echo_secret_query(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            audit_test.write_export(export_dir)
            with audit_test.capture_server() as (endpoint, _requests):
                self.assertEqual(
                    audit_test.run_main(
                        [
                            "--export-dir",
                            str(export_dir),
                            "--endpoint",
                            endpoint,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            notary_receipt = next((export_dir / "receipts").glob("*.receipt.json"))

            inbox = root / "inbox"
            inbox.mkdir()
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            rail_receipt = next((inbox / "receipts").glob("*.receipt.json"))

            cases = [
                (
                    notary_receipt,
                    "endpoint",
                    "https://notary.example/anchor?token=notary-secret",
                    "notary-secret",
                ),
                (
                    rail_receipt,
                    "endpoint_url",
                    "https://rail.example/v1/iso20022?token=rail-secret",
                    "rail-secret",
                ),
            ]
            for receipt, field, secret_url, secret in cases:
                with self.subTest(field=field):
                    rewrite_receipt(
                        receipt,
                        lambda body, field=field, secret_url=secret_url: body.update(
                            {field: secret_url}
                        ),
                    )

                    rc, _stdout, stderr = run_verify(["--receipt", str(receipt)])

                    self.assertEqual(rc, 2)
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn(secret_url, stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn(secret, stderr)


if __name__ == "__main__":
    unittest.main()
