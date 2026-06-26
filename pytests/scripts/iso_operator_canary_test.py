import argparse
import array
import contextlib
import copy
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

from pytests.scripts import iso_audit_notary_adapter_test as audit_test
from pytests.scripts import iso_rail_gateway_adapter_test as rail_test


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "iso_operator_canary.py"
SPEC = importlib.util.spec_from_file_location("iso_operator_canary", SCRIPT_PATH)
CANARY = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = CANARY
SPEC.loader.exec_module(CANARY)


def write_config(root, body):
    path = root / "canary.json"
    path.write_text(json.dumps(body, indent=2) + "\n", encoding="utf-8")
    return path


def run_canary(argv):
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        rc = CANARY.main(argv)
    return rc, stdout.getvalue(), stderr.getvalue()


def load_summary(stdout):
    return json.loads(stdout)


class IsoOperatorCanaryTest(unittest.TestCase):
    def test_os_error_detail_redacts_unsafe_strerror_without_echo(self):
        self.assertEqual(
            CANARY._safe_os_error_detail(OSError(5, "Permission denied")),
            "Permission denied",
        )
        unsafe_values = (
            "token=/tmp/canary-hidden-secret",
            "open /tmp/canary-hidden-path",
            "bad\ncontrol",
            "nonascii-\u2603",
            "x" * 129,
        )
        for value in unsafe_values:
            with self.subTest(value=value):
                detail = CANARY._safe_os_error_detail(OSError(5, value))
                self.assertEqual(detail, "I/O error")
                self.assertNotIn("canary-hidden", detail)

    def test_canonical_json_bytes_rejects_non_finite_numbers(self):
        for value in (float("nan"), float("inf"), float("-inf")):
            with self.subTest(value=value):
                with self.assertRaises(ValueError):
                    CANARY._canonical_json_bytes({"value": value})

    def test_json_float_parser_rejects_overflow_and_negative_zero(self):
        for value in ("1e9999", "-1e9999", "-0.0", "-0e0"):
            with self.subTest(value=value):
                with self.assertRaises(CANARY.CanaryError) as caught:
                    CANARY._parse_canonical_json_float(value)
                self.assertNotIn(value, str(caught.exception))

    def test_text_output_symlink_ancestor_diagnostic_does_not_echo_path(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target = root / "target"
            target.mkdir()
            hidden = "hidden-canary-output-link"
            link = root / hidden
            try:
                link.symlink_to(target, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            with self.assertRaises(CANARY.CanaryError) as caught:
                CANARY._write_text_output(
                    link / "summary.json",
                    "{}\n",
                    display_label="summary_out",
                )

            message = str(caught.exception)
            self.assertIn("summary_out", message)
            self.assertIn("must not be a symlink", message)
            self.assertNotIn(str(link), message)
            self.assertNotIn(hidden, message)

    def test_secret_looking_unknown_keys_are_rejected_without_echo(self):
        cases = (
            ("password_canary_unknown_secret", "canary_unknown_secret"),
            ("%70assword_canary_unknown_leak", "canary_unknown_leak"),
            ("private-key_canary_unknown_leak", "canary_unknown_leak"),
            ("private--key_canary_unknown_leak", "canary_unknown_leak"),
            ("private%09key_canary_unknown_leak", "canary_unknown_leak"),
            ("x--iroha--signature_canary_unknown_leak", "canary_unknown_leak"),
            ("unexpected\x1bcanary_key", "\x1b"),
            ("unexpected_canary_\uff4bey", "\uff4b"),
            ("operator_note", "operator_note"),
            ("x" * 129, "x" * 129),
        )
        for unknown_key, hidden in cases:
            with self.subTest(unknown_key=unknown_key):
                with self.assertRaises(CANARY.CanaryError) as caught:
                    CANARY._reject_unknown_keys({unknown_key: "redacted"}, set(), "runbook")

                message = str(caught.exception)
                self.assertIn("contains unknown keys", message)
                self.assertNotIn("password", message)
                self.assertNotIn(unknown_key, message)
                self.assertNotIn(hidden, message)
        many_unknown = {f"field_{offset}": "redacted" for offset in range(9)}
        with self.assertRaises(CANARY.CanaryError) as caught:
            CANARY._reject_unknown_keys(many_unknown, set(), "runbook")
        message = str(caught.exception)
        self.assertIn("contains unknown keys", message)
        self.assertNotIn("field_0", message)
        self.assertNotIn("field_8", message)

    def test_separator_smuggled_secret_identifiers_are_detected(self):
        cases = (
            "private\tkey canary identifier",
            "private--key canary identifier",
            "private/key canary identifier",
            "private\\key canary identifier",
            "private%2fkey canary identifier",
            "private\u200dkey canary identifier",
            "private\u0301key canary identifier",
            "ｐｒｉｖａｔｅｋｅｙ canary identifier",
            "x--iroha--signature canary identifier",
            "x/iroha/signature canary identifier",
            "x%2firoha%2fsignature canary identifier",
            "x\u200diroha\u200dsignature canary identifier",
            "x\u0301iroha\u0301signature canary identifier",
            "ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ canary identifier",
            "token%09secret canary identifier",
        )
        for value in cases:
            with self.subTest(value=value):
                self.assertTrue(CANARY._contains_secret_identifier_material(value))
        for key in (
            "private/key",
            "private%2fkey",
            "private\u0301key",
            "ｐｒｉｖａｔｅｋｅｙ",
            "x/iroha/signature",
            "x%2firoha%2fsignature",
            "x\u0301iroha\u0301signature",
            "ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ",
        ):
            with self.subTest(key=key):
                self.assertTrue(CANARY._is_secret_looking_key(key))

    def test_path_separator_secret_key_values_are_detected(self):
        cases = (
            "private/key=canary-value-secret",
            "api/key:canary-value-secret",
            "client/secret=canary-value-secret",
            "set/cookie:canary-value-secret",
            "x/iroha/signature: canary-value-secret",
            "private%2fkey=canary-value-secret",
            "private\u200dkey=canary-value-secret",
            "private\u0301key=canary-value-secret",
            "ｐｒｉｖａｔｅｋｅｙ=canary-compat-secret",
            "ａｐｉ／ｋｅｙ:canary-compat-secret",
            "x\u200diroha\u200dsignature: canary-value-secret",
            "x\u0301iroha\u0301signature: canary-value-secret",
            "ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ: canary-compat-secret",
            "private%E2%80%8Dkey=canary-value-secret",
            "private%CC%81key=canary-value-secret",
        )
        for value in cases:
            with self.subTest(value=value):
                self.assertTrue(CANARY._contains_secret_material(value))

    def test_cli_argument_terminator_is_rejected_without_echo(self):
        hidden = "token=canary-terminator-secret"
        cases = (
            (
                "raw",
                lambda: CANARY._preflight_raw_cli_secrets(
                    ["--", "--summary-out", hidden],
                    {"--summary-out"},
                ),
            ),
            (
                "path",
                lambda: CANARY._preflight_output_cli_paths(
                    ["--", "--summary-out", hidden],
                    {"--summary-out"},
                ),
            ),
            (
                "boolean",
                lambda: CANARY._preflight_boolean_cli_flags(
                    ["--", "--plan-only", hidden],
                    {"--plan-only"},
                ),
            ),
            (
                "numeric",
                lambda: CANARY._preflight_numeric_cli_values(
                    ["--", "--stage-timeout-secs", hidden],
                    integer_flags=set(),
                    number_flags={"--stage-timeout-secs"},
                ),
            ),
        )
        for helper, run in cases:
            with self.subTest(helper=helper):
                with self.assertRaises(CANARY.CanaryError) as caught:
                    run()

                message = str(caught.exception)
                self.assertIn("argument terminator is not supported", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn("canary-terminator-secret", message)

    def test_parser_rejects_abbreviated_long_options(self):
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as caught:
                CANARY.build_parser().parse_args(
                    ["--config", "canary.json", "--summary-ou", "out"]
                )

        self.assertEqual(caught.exception.code, 2)
        self.assertIn("unrecognized arguments", stderr.getvalue())
        self.assertIn("--summary-ou", stderr.getvalue())

    def test_raw_cli_control_characters_are_rejected_without_echo(self):
        cases = ("--unknown-canary\x1bflag", "--unknown-canary\u202eflag")
        for hidden in cases:
            with self.subTest(hidden=hidden):
                with self.assertRaises(CANARY.CanaryError) as caught:
                    CANARY._preflight_raw_cli_secrets([hidden], {"--summary-out"})

                message = str(caught.exception)
                self.assertIn("CLI argument must not contain control characters", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn("\x1b", message)
                self.assertNotIn("\u202e", message)
                self.assertNotIn("unknown-canary", message)

    def test_raw_cli_non_ascii_is_rejected_without_echo(self):
        hidden = "\uff0d\uff0dsummary-out"
        with self.assertRaises(CANARY.CanaryError) as caught:
            CANARY._preflight_raw_cli_secrets([hidden], {"--summary-out"})

        message = str(caught.exception)
        self.assertIn("CLI argument must use printable ASCII", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn("summary-out", message)

    def test_output_cli_path_flags_reject_flag_like_values(self):
        cases = (
            ["--summary-out"],
            ["--summary-out", ""],
            ["--summary-out", "--plan-only"],
            ["--summary-out="],
            ["--summary-out=--plan-only"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                with self.assertRaisesRegex(
                    CANARY.CanaryError,
                    "--summary-out requires a path value",
                ):
                    CANARY._preflight_output_cli_paths(argv, {"--summary-out"})

    def test_output_cli_paths_reject_encoded_secret_material_without_echo(self):
        cases = (
            ("token=canary-path-leak.summary.json", "token=canary-path-leak"),
            ("token%3Dcanary-path-leak.summary.json", "token=canary-path-leak"),
            ("private%20key%3Dcanary-path-leak.summary.json", "private key=canary-path-leak"),
            ("private%20key-canary-path-secret.summary.json", "private key-canary-path-secret"),
            ("private/key-canary-path-secret.summary.json", "private/key-canary-path-secret"),
            ("x%2firoha%2fsignature-canary-path-secret.summary.json", "x/iroha/signature-canary-path-secret"),
            ("%70assword%253Dcanary-path-leak.summary.json", "password=canary-path-leak"),
            ("token-canary-path-secret.summary.json", "token-canary-path-secret"),
        )
        for raw_path, decoded_secret in cases:
            with self.subTest(raw_path=raw_path):
                with self.assertRaises(CANARY.CanaryError) as caught:
                    CANARY._preflight_output_cli_paths(
                        ["--summary-out", raw_path], {"--summary-out"}
                    )

                message = str(caught.exception)
                self.assertIn("secret-looking material", message)
                self.assertNotIn(raw_path, message)
                self.assertNotIn(decoded_secret, message)
                self.assertNotIn("canary-path-leak", message)

    def test_summary_output_rejects_repository_fixture_artifacts(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            output_path = root / "fixtures" / "iso20022" / "operator_canary" / "summary.json"

            with self.assertRaisesRegex(
                CANARY.CanaryError,
                "output path must not point to checked-in ISO fixture artifacts",
            ):
                CANARY._write_text_output(output_path, "{}\n")

            self.assertFalse((root / "fixtures").exists())
            with self.assertRaisesRegex(
                CANARY.CanaryError,
                "output path must not point to checked-in ISO fixture artifacts",
            ):
                CANARY._reject_repository_iso_fixture_path(
                    Path("fixtures/iso20022/operator_canary/summary.json"),
                    "output path",
                )

    def test_summary_output_rejects_repository_fixture_before_config_load(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            output_path = root / "fixtures" / "iso20022" / "operator_canary" / "summary.json"

            rc, stdout, stderr = run_canary(
                [
                    "--config",
                    str(root / "missing-canary.json"),
                    "--summary-out",
                    str(output_path),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "summary_out must not point to checked-in ISO fixture artifacts",
                stderr,
            )
            self.assertNotIn("does not exist", stderr)
            self.assertFalse((root / "fixtures").exists())

    def test_summary_output_cannot_reuse_config_path_before_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = root / "operator-canary.json"
            config.write_text("{not valid canary json\n", encoding="utf-8")
            original_text = config.read_text(encoding="utf-8")

            rc, stdout, stderr = run_canary(
                [
                    "--config",
                    str(config),
                    "--plan-only",
                    "--summary-out",
                    str(config),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("summary_out must not reuse --config path", stderr)
            self.assertNotIn("not valid JSON", stderr)
            self.assertEqual(config.read_text(encoding="utf-8"), original_text)

    def test_summary_output_cannot_hardlink_config_path_before_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = root / "operator-canary.json"
            config.write_text("{not valid canary json\n", encoding="utf-8")
            summary_out = root / "canary-summary-alias.json"
            try:
                summary_out.hardlink_to(config)
            except OSError as error:
                self.skipTest(f"hard link creation unavailable: {error}")
            original_text = config.read_text(encoding="utf-8")

            rc, stdout, stderr = run_canary(
                [
                    "--config",
                    str(config),
                    "--plan-only",
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("summary_out must not reuse --config path", stderr)
            self.assertNotIn("not valid JSON", stderr)
            self.assertEqual(config.read_text(encoding="utf-8"), original_text)
            self.assertEqual(summary_out.read_text(encoding="utf-8"), original_text)

    def test_symlinked_summary_output_ancestor_is_rejected_before_config_load(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = root / "operator-canary.json"
            config.write_text("{not valid canary json\n", encoding="utf-8")
            target_dir = root / "summary-target"
            target_dir.mkdir()
            ancestor = root / "summary-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            summary_out = ancestor / "nested" / "canary-summary.json"

            rc, stdout, stderr = run_canary(
                [
                    "--config",
                    str(config),
                    "--plan-only",
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("summary_out must not be a symlink", stderr)
            self.assertNotIn("not valid JSON", stderr)
            self.assertFalse((target_dir / "nested").exists())

    def test_missing_summary_output_parent_is_not_created_before_config_load(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = root / "operator-canary.json"
            config.write_text("{not valid canary json\n", encoding="utf-8")
            summary_parent = root / "summary" / "new"
            summary_out = summary_parent / "canary-summary.json"

            rc, stdout, stderr = run_canary(
                [
                    "--config",
                    str(config),
                    "--plan-only",
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("not valid JSON", stderr)
            self.assertFalse(summary_parent.exists())

    def test_summary_output_cannot_reuse_stage_artifact_paths_before_execution(self):
        cases = (
            (
                "rail-message",
                "rail.message",
                "payment.xml",
                lambda path: {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "rail": {
                        "inbox_dir": "inbox",
                        "message": path.name,
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                },
            ),
            (
                "rail-bearer",
                "rail.bearer_token_file",
                "rail.bearer",
                lambda path: {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                        "bearer_token_file": path.name,
                    },
                },
            ),
            (
                "notary-bearer",
                "notary.bearer_token_file",
                "notary.bearer",
                lambda path: {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "notary": {
                        "export_dir": "audit-export",
                        "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                        "bearer_token_file": path.name,
                    },
                },
            ),
            (
                "verify-receipt",
                "verify.receipts[0]",
                "rail.receipt.json",
                lambda path: {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                        "dry_run": True,
                    },
                    "verify": {
                        "enabled": True,
                        "include_stage_receipts": False,
                        "receipt_dirs": [],
                        "receipts": [path.name],
                        "require_source_files": False,
                    },
                },
            ),
        )
        for name, label, leaf, build_body in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    artifact = root / leaf
                    artifact.write_text("source artifact\n", encoding="utf-8")
                    config = write_config(root, build_body(artifact))

                    rc, stdout, stderr = run_canary(
                        [
                            "--config",
                            str(config),
                            "--plan-only",
                            "--summary-out",
                            str(artifact),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(f"summary_out must not reuse {label} path", stderr)
                    self.assertEqual(
                        artifact.read_text(encoding="utf-8"),
                        "source artifact\n",
                    )

    def test_summary_output_cannot_be_written_under_stage_artifact_dirs(self):
        cases = (
            (
                "rail-inbox",
                "rail.inbox_dir",
                "inbox/canary.summary.json",
                {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                        "receipt_dir": "rail-receipts",
                    },
                },
            ),
            (
                "rail-receipts",
                "rail.receipt_dir",
                "rail-receipts/canary.summary.json",
                {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                        "receipt_dir": "rail-receipts",
                    },
                },
            ),
            (
                "notary-export",
                "notary.export_dir",
                "audit-export/canary.summary.json",
                {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "notary": {
                        "export_dir": "audit-export",
                        "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                        "receipt_dir": "notary-receipts",
                    },
                },
            ),
            (
                "verify-receipt-dir",
                "verify.receipt_dirs[0]",
                "receipt-archive/canary.summary.json",
                {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                        "dry_run": True,
                    },
                    "verify": {
                        "enabled": True,
                        "include_stage_receipts": False,
                        "receipt_dirs": ["receipt-archive"],
                        "receipts": [],
                        "require_source_files": False,
                    },
                },
            ),
        )
        for name, label, summary_rel, body in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    config = write_config(root, body)
                    summary_out = root / summary_rel

                    rc, stdout, stderr = run_canary(
                        [
                            "--config",
                            str(config),
                            "--plan-only",
                            "--summary-out",
                            str(summary_out),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(
                        f"summary_out must not be written under {label}",
                        stderr,
                    )
                    self.assertFalse(summary_out.exists())

    def test_local_path_validators_reject_percent_encoded_smuggling(self):
        overlong_path = "out/" + ("a" * (CANARY.MAX_LOCAL_PATH_CHARS + 1))
        cases = (
            (
                "raw overlong",
                lambda raw: CANARY._reject_raw_output_path_smuggling(raw, "raw path"),
                overlong_path,
                f"no longer than {CANARY.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "output overlong",
                lambda raw: CANARY._reject_output_path_smuggling(Path(raw), "output path"),
                overlong_path,
                f"no longer than {CANARY.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "runbook overlong",
                lambda raw: CANARY._validate_path_string(raw, "rail.receipt_dir"),
                overlong_path,
                f"no longer than {CANARY.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "raw encoded dot",
                lambda raw: CANARY._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%2e/summary.json",
                "encoded dot or separator",
            ),
            (
                "output encoded slash",
                lambda raw: CANARY._reject_output_path_smuggling(Path(raw), "output path"),
                "out/%2f/summary.json",
                "encoded dot or separator",
            ),
            (
                "raw format control",
                lambda raw: CANARY._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/\u202esummary.json",
                "control characters",
            ),
            (
                "output format control",
                lambda raw: CANARY._reject_output_path_smuggling(Path(raw), "output path"),
                "out/\u202esummary.json",
                "control characters",
            ),
            (
                "runbook format control",
                lambda raw: CANARY._validate_path_string(raw, "rail.receipt_dir"),
                "receipts/\u202ecurrent",
                "control characters",
            ),
            (
                "raw uri prefix",
                lambda raw: CANARY._reject_raw_output_path_smuggling(raw, "raw path"),
                "file:out/summary.json",
                "URI or drive prefixes",
            ),
            (
                "runbook drive prefix",
                lambda raw: CANARY._validate_path_string(raw, "rail.receipt_dir"),
                "C:/receipts/current",
                "URI or drive prefixes",
            ),
            (
                "runbook encoded semicolon",
                lambda raw: CANARY._validate_path_string(raw, "rail.receipt_dir"),
                "receipts/%3b/current",
                "encoded semicolon",
            ),
            (
                "runbook encoded delimiter",
                lambda raw: CANARY._validate_path_string(raw, "rail.receipt_dir"),
                "receipts/%5d/current",
                "encoded URL delimiter",
            ),
            (
                "raw encoded percent",
                lambda raw: CANARY._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%25/summary.json",
                "encoded percent",
            ),
            (
                "raw encoded space",
                lambda raw: CANARY._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%20/summary.json",
                "percent-encoded control or space",
            ),
            (
                "raw malformed percent",
                lambda raw: CANARY._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%zz/summary.json",
                "malformed percent",
            ),
        )
        for name, call, raw, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(CANARY.CanaryError) as caught:
                    call(raw)

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(raw, message)

    def test_url_paths_reject_raw_delimiter_smuggling(self):
        cases = (
            "https://torii.local-bank.bank/base:debug/v1",
            "https://torii.local-bank.bank/base@debug/v1",
            "https://torii.local-bank.bank/base[debug]/v1",
        )
        for url in cases:
            with self.subTest(url=url):
                with self.assertRaises(CANARY.CanaryError) as caught:
                    CANARY._validate_endpoint_url(
                        url,
                        "rail.torii_base_url",
                        allow_insecure_http=False,
                    )

                message = str(caught.exception)
                self.assertIn("path must not contain URL delimiter characters", message)
                self.assertNotIn(url, message)

    def test_urls_reject_non_ascii_smuggling(self):
        cases = (
            (
                "https://torii\u0661.local-bank.bank/base/v1",
                "host must use printable ASCII",
            ),
            (
                "https://torii.local-bank.bank/base∕debug/v1",
                "path must use printable ASCII",
            ),
            (
                "https://torii.local-bank.bank/base\u202edebug/v1",
                "rail.torii_base_url must not contain control characters",
            ),
            (
                "https://torii.local-bank.bank/base%c3%a9/v1",
                "path must not contain percent-encoded non-ASCII bytes",
            ),
        )
        for url, expected in cases:
            with self.subTest(url=url):
                with self.assertRaises(CANARY.CanaryError) as caught:
                    CANARY._validate_endpoint_url(
                        url,
                        "rail.torii_base_url",
                        allow_insecure_http=False,
                    )

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(url, message)

    def test_endpoint_urls_reject_secret_path_without_echo(self):
        cases = (
            "https://torii.local-bank.bank/base/token=canary-url-secret",
            "https://torii.local-bank.bank/base/token-canary-url-secret",
            "https://torii.local-bank.bank/base/token%3Dcanary-url-secret",
            "https://torii.local-bank.bank/base/token%253Dcanary-url-secret",
        )
        for url in cases:
            with self.subTest(url=url):
                with self.assertRaises(CANARY.CanaryError) as caught:
                    CANARY._validate_endpoint_url(
                        url,
                        "rail.torii_base_url",
                        allow_insecure_http=False,
                    )

                message = str(caught.exception)
                self.assertIn("secret-looking material", message)
                self.assertNotIn(url, message)
                self.assertNotIn("token=", message)
                self.assertNotIn("canary-url-secret", message)

    def test_endpoint_urls_reject_secret_host_and_parser_errors_without_echo(self):
        cases = (
            (
                "https://token-canary-host-secret.torii.local-bank.bank/base",
                "secret-looking material",
            ),
            ("https://[token-canary-host-secret/base", "is not a valid URL"),
        )
        for url, expected in cases:
            with self.subTest(url=url):
                with self.assertRaises(CANARY.CanaryError) as caught:
                    CANARY._validate_endpoint_url(
                        url,
                        "rail.torii_base_url",
                        allow_insecure_http=False,
                    )

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(url, message)
                self.assertNotIn("token-canary-host-secret", message)

    def test_boolean_cli_flags_reject_values_without_echo(self):
        cases = (
            (["--plan-only=true"], "--plan-only", "--plan-only=true"),
            (
                ["--require-explicit-policy", "true"],
                "--require-explicit-policy",
                "true",
            ),
        )
        for argv, flag, rejected in cases:
            with self.subTest(argv=argv):
                rc, stdout, stderr = run_canary(argv)

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertIn(f"{flag} does not take a value", stderr)
                self.assertNotIn(rejected, stderr)

    def test_numeric_cli_flags_reject_malformed_values_without_echo(self):
        cases = (
            ["--output-limit-bytes", "token=canary-secret"],
            ["--output-limit-bytes=token=canary-secret"],
            ["--stage-timeout-secs", "--summary-out"],
            ["--stage-timeout-secs="],
            ["--stage-timeout-secs", "\u202e1"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_canary(argv)

                self.assertEqual(rc, 2)
                self.assertIn("numeric value", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("canary-secret", stderr)
                self.assertNotIn("\u202e", stderr)

    def test_numeric_cli_flags_reject_unicode_digits_without_echo(self):
        hidden = "\u0661"
        cases = (
            ["--output-limit-bytes", hidden],
            [f"--output-limit-bytes={hidden}"],
            ["--stage-timeout-secs", f"{hidden}.5"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_canary(argv)

                self.assertEqual(rc, 2)
                self.assertIn("must use printable ASCII", stderr)
                self.assertNotIn(hidden, stderr)

    def test_numeric_cli_flags_reject_noncanonical_decimal_spellings_before_stage_planning(self):
        cases = (
            ["--output-limit-bytes", "000512"],
            ["--output-limit-bytes", "+512"],
            ["--output-limit-bytes=001024"],
            ["--output-limit-bytes", "-0"],
            ["--stage-timeout-secs", ".5"],
            ["--stage-timeout-secs", "01"],
            ["--stage-timeout-secs", "1e01"],
            ["--stage-timeout-secs", "1."],
            ["--stage-timeout-secs", "+1"],
            ["--stage-timeout-secs", "-0"],
            ["--stage-timeout-secs", "-0.0"],
            ["--stage-timeout-secs", "-0e0"],
            ["--stage-timeout-secs", "1e9999"],
            ["--stage-timeout-secs", "-1e9999"],
            ["--stage-timeout-secs=-0e0"],
            ["--stage-timeout-secs=1e9999"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, stdout, stderr = run_canary(argv)

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertIn("numeric value", stderr)

    def test_raw_cli_secret_like_values_rejected_without_echo(self):
        cases = (
            ["--private-key=canary-secret"],
            ["token=canary-secret"],
            ["password=canary-secret"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_canary(argv)

                self.assertEqual(rc, 2)
                self.assertIn("secret-looking", stderr)
                self.assertNotIn("password=", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("canary-secret", stderr)

    def test_secret_looking_runbook_identities_are_rejected_without_echo(self):
        cases = (
            (
                "provider",
                "token-canary-provider-secret",
                "config.provider must not contain secret-looking material",
                "canary-provider-secret",
            ),
            (
                "environment",
                "session-key-canary-environment-secret",
                "config.environment must not contain secret-looking material",
                "canary-environment-secret",
            ),
        )
        for key, secret_value, message, hidden in cases:
            with self.subTest(key=key):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    body = {
                        "provider": "local-bank",
                        "environment": "ci",
                        "rail": {
                            "inbox_dir": "missing-inbox",
                            "torii_base_url": "https://torii.local-bank.bank",
                        },
                    }
                    body[key] = secret_value
                    config = write_config(root, body)

                    rc, stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(secret_value, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_overlong_runbook_strings_are_rejected_without_echo(self):
        overlong = "M" * (CANARY.MAX_CLEAN_STRING_CHARS + 1)
        cases = (
            (
                "required",
                lambda: CANARY._required_string(
                    {"provider": overlong}, "provider", "runbook"
                ),
                f"runbook.provider must be no longer than {CANARY.MAX_CLEAN_STRING_CHARS} characters",
            ),
            (
                "optional",
                lambda: CANARY._optional_string(
                    {"message": overlong}, "message", "rail"
                ),
                f"rail.message must be no longer than {CANARY.MAX_CLEAN_STRING_CHARS} characters",
            ),
            (
                "list",
                lambda: CANARY._string_list(
                    {"endpoints": [overlong]}, "endpoints", "notary"
                ),
                f"notary.endpoints[0] must be no longer than {CANARY.MAX_CLEAN_STRING_CHARS} characters",
            ),
        )
        for name, call, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(CANARY.CanaryError) as caught:
                    call()

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(overlong, message)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = write_config(
                root,
                {
                    "provider": overlong,
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "missing-inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                },
            )

            rc, stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                f"config.provider must be no longer than {CANARY.MAX_CLEAN_STRING_CHARS} characters",
                stderr,
            )
            self.assertNotIn(overlong, stderr)

    def test_config_string_lists_are_count_bounded_before_entry_parsing(self):
        items = [None] * (CANARY.MAX_JSON_LIST_ITEMS + 1)
        cases = (
            ("notary", "endpoints"),
            ("verify", "receipt_dirs"),
            ("verify", "receipts"),
        )
        for label, key in cases:
            with self.subTest(label=label, key=key):
                with self.assertRaises(CANARY.CanaryError) as caught:
                    CANARY._string_list({key: items}, key, label)

                message = str(caught.exception)
                self.assertIn(
                    f"{label}.{key} must contain at most {CANARY.MAX_JSON_LIST_ITEMS} items",
                    message,
                )
                self.assertNotIn(str(len(items)), message)
                self.assertNotIn("[0]", message)

    def test_recursive_json_array_scans_are_count_bounded_without_echo(self):
        items = [None] * (CANARY.MAX_JSON_LIST_ITEMS + 1)

        with self.assertRaises(CANARY.CanaryError) as caught:
            CANARY._reject_json_surrogates(items)

        message = str(caught.exception)
        self.assertIn(
            f"JSON array must contain at most {CANARY.MAX_JSON_LIST_ITEMS} items",
            message,
        )
        self.assertNotIn(str(len(items)), message)
        self.assertNotIn("[0]", message)

    def test_recursive_json_object_scans_are_count_bounded_without_echo(self):
        members = {
            f"hidden_key_{offset}": None
            for offset in range(CANARY.MAX_JSON_OBJECT_MEMBERS + 1)
        }
        pairs = list(members.items())
        cases = (
            (
                "json hook",
                lambda: CANARY._reject_duplicate_json_keys(pairs),
                f"JSON object must contain at most {CANARY.MAX_JSON_OBJECT_MEMBERS} members",
            ),
            (
                "surrogates",
                lambda: CANARY._reject_json_surrogates(members),
                f"JSON object must contain at most {CANARY.MAX_JSON_OBJECT_MEMBERS} members",
            ),
        )
        for name, action, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(CANARY.CanaryError) as caught:
                    action()

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(str(len(members)), message)
                self.assertNotIn("hidden_key_0", message)

    def test_recursive_json_depth_scans_are_bounded_without_echo(self):
        nested = "hidden_leaf"
        for _ in range(CANARY.MAX_JSON_NESTING_DEPTH + 1):
            nested = [nested]

        with self.assertRaises(CANARY.CanaryError) as caught:
            CANARY._reject_json_surrogates(nested)

        message = str(caught.exception)
        self.assertIn(
            f"JSON nesting depth must be at most {CANARY.MAX_JSON_NESTING_DEPTH} levels",
            message,
        )
        self.assertNotIn("hidden_leaf", message)
        self.assertNotIn("[0]", message)

    def test_json_parse_recursion_error_is_bounded_without_echo(self):
        hidden = "hidden-canary-recursion"
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / hidden
            path.write_text("[]\n", encoding="utf-8")
            original_loads = CANARY.json.loads

            def raising_loads(*_args, **_kwargs):
                raise RecursionError(hidden)

            CANARY.json.loads = raising_loads
            try:
                with self.assertRaises(CANARY.CanaryError) as caught:
                    CANARY._load_json(path, display_label="runbook")
            finally:
                CANARY.json.loads = original_loads

        message = str(caught.exception)
        self.assertIn(
            f"JSON nesting depth must be at most {CANARY.MAX_JSON_NESTING_DEPTH} levels",
            message,
        )
        self.assertNotIn(hidden, message)
        self.assertNotIn(str(path), message)

    def test_boolean_and_non_integer_file_read_limits_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / "canary.json"
            path.write_text("{}\n", encoding="utf-8")

            for limit in (True, "64"):
                with self.subTest(limit=limit):
                    with self.assertRaisesRegex(
                        CANARY.CanaryError,
                        "max file bytes must be a positive integer",
                    ):
                        CANARY._read_regular_file(path, max_bytes=limit)

    def test_runs_rail_notary_and_verifies_generated_receipts(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            inbox.mkdir()
            rail_test.write_message(inbox)
            export_dir = root / "export"
            export_dir.mkdir()
            audit_test.write_export(
                export_dir,
                store_dir=root / "audit-store",
                write_record_sources_flag=True,
            )
            summary_out = root / "summary" / "canary.summary.json"
            summary_out.parent.mkdir()
            summary_out.write_text('{"stale": true}\n' + ("x" * 4096), encoding="utf-8")

            with rail_test.capture_server() as (torii_url, rail_requests):
                with audit_test.capture_server() as (notary_url, notary_requests):
                    config = write_config(
                        root,
                        {
                            "provider": "local-bank",
                            "environment": "ci",
                            "rail": {
                                "inbox_dir": str(inbox),
                                "torii_base_url": torii_url,
                                "allow_insecure_http": True,
                            },
                            "notary": {
                                "export_dir": str(export_dir),
                                "endpoints": [notary_url],
                                "allow_insecure_http": True,
                            },
                            "verify": {
                                "allow_insecure_http": True,
                                "require_source_files": True,
                            },
                        },
                    )
                    rc, stdout, stderr = run_canary(
                        ["--config", str(config), "--summary-out", str(summary_out)]
                    )

            self.assertEqual(rc, 0, stderr)
            self.assertEqual(len(rail_requests), 1)
            self.assertEqual(len(notary_requests), 1)
            self.assertTrue(summary_out.exists())
            summary = load_summary(stdout)
            self.assertEqual(summary["version"], CANARY.CANARY_SUMMARY_VERSION)
            self.assertTrue(summary["ok"])
            self.assertEqual(summary["provider"], "local-bank")
            self.assertFalse(summary["policy"]["require_explicit_policy"])
            self.assertEqual([stage["name"] for stage in summary["stages"]], ["rail", "notary", "verify"])
            self.assertEqual([stage["returncode"] for stage in summary["stages"]], [0, 0, 0])
            for stage in summary["stages"]:
                self.assertRegex(stage["started_at"], r"^\d{4}-\d{2}-\d{2}T")
                self.assertRegex(stage["finished_at"], r"^\d{4}-\d{2}-\d{2}T")
            body = dict(summary)
            digest = body.pop("summary_sha256")
            self.assertEqual(digest, CANARY.sha256_hex(CANARY._canonical_json_bytes(body)))
            self.assertEqual(
                json.loads(summary_out.read_text(encoding="utf-8")),
                summary,
            )
            self.assertEqual(summary_out.stat().st_mode & 0o077, 0)
            self.assertEqual(
                list(summary_out.parent.glob(".iso-*.tmp")),
                [],
            )

    def test_plan_only_redacts_token_paths_and_does_not_execute(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "rail": {
                        "inbox_dir": "missing-inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                        "bearer_token_file": "secrets/torii.bearer",
                    },
                    "notary": {
                        "export_dir": "missing-export",
                        "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                        "bearer_token_file": "secrets/notary.bearer",
                    },
                },
            )

            rc, stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 0, stderr)
            self.assertFalse((root / "missing-inbox").exists())
            self.assertFalse((root / "missing-export").exists())
            summary = load_summary(stdout)
            self.assertEqual(summary["version"], CANARY.CANARY_SUMMARY_VERSION)
            self.assertTrue(summary["ok"])
            self.assertTrue(summary["plan_only"])
            self.assertEqual(
                [stage["name"] for stage in summary["planned_stages"]],
                ["rail", "notary", "verify"],
            )
            planned_text = json.dumps(summary["planned_stages"])
            self.assertIn("<runtime-token-file>", planned_text)
            self.assertNotIn("secrets/torii.bearer", planned_text)
            self.assertNotIn("secrets/notary.bearer", planned_text)

    def test_boolean_output_limit_is_rejected_before_execution(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            inbox.mkdir()
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": str(inbox),
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                },
            )
            args = argparse.Namespace(
                config=config,
                require_explicit_policy=False,
                output_limit_bytes=True,
                stage_timeout_secs=1.0,
                summary_out=None,
                plan_only=True,
            )

            with self.assertRaisesRegex(
                CANARY.CanaryError,
                "--output-limit-bytes must be positive",
            ):
                CANARY.run(args)
            with self.assertRaisesRegex(
                CANARY.CanaryError,
                "output limit bytes must be positive",
            ):
                CANARY._run_command_bounded(
                    [sys.executable, "-c", "print('ok')"],
                    True,
                    1.0,
                )

    def test_child_stage_startup_failure_is_controlled_without_echo(self):
        hidden = "token=canary-startup-secret"

        def raising_popen(*_args, **_kwargs):
            raise OSError(hidden)

        original_popen = CANARY.subprocess.Popen
        CANARY.subprocess.Popen = raising_popen
        try:
            with self.assertRaises(CANARY.CanaryError) as caught:
                CANARY._run_command_bounded(
                    [str(Path(tempfile.gettempdir()) / hidden / "child")],
                    128,
                    1.0,
                )
        finally:
            CANARY.subprocess.Popen = original_popen

        message = str(caught.exception)
        self.assertIn("child stage could not be started", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn(tempfile.gettempdir(), message)
        self.assertIsNone(caught.exception.__cause__)
        self.assertTrue(caught.exception.__suppress_context__)

    def test_child_stage_output_read_failure_is_controlled_without_echo(self):
        hidden = "token=canary-pipe-secret"

        def raising_read(*_args, **_kwargs):
            raise OSError(hidden)

        original_read = CANARY._read_limited_pipe
        CANARY._read_limited_pipe = raising_read
        try:
            with self.assertRaises(CANARY.CanaryError) as caught:
                CANARY._run_command_bounded(
                    [sys.executable, "-c", "print('ok')"],
                    128,
                    1.0,
                )
        finally:
            CANARY._read_limited_pipe = original_read

        message = str(caught.exception)
        self.assertIn("child stage output could not be read", message)
        self.assertNotIn(hidden, message)
        self.assertIsNone(caught.exception.__cause__)
        self.assertTrue(caught.exception.__suppress_context__)

    def test_child_stage_pipe_reader_clamps_bytes_like_chunks_by_byte_length(self):
        class FakePipe:
            def __init__(self, chunks):
                self.chunks = list(chunks)

            def read(self, _size):
                return self.chunks.pop(0) if self.chunks else b""

        wide = memoryview(array.array("H", [0x4142] * 8))
        cases = (
            ("bytes", b"abcdefgh", b"abcdef"),
            ("bytearray", bytearray(b"abcdefgh"), b"abcdef"),
            ("memoryview", memoryview(b"abcdefgh"), b"abcdef"),
            ("wide-memoryview", wide, wide.cast("B")[:6].tobytes()),
        )
        for name, chunk, expected in cases:
            with self.subTest(name=name):
                captured, truncated = CANARY._read_limited_pipe(FakePipe([chunk]), 6)
                self.assertEqual(captured, expected)
                self.assertTrue(truncated)

    def test_child_stage_pipe_reader_rejects_non_byte_chunks_without_echo(self):
        hidden = "token=canary-pipe-secret"

        class FakePipe:
            def __init__(self, chunks):
                self.chunks = list(chunks)

            def read(self, _size):
                return self.chunks.pop(0) if self.chunks else b""

        with self.assertRaises(OSError) as caught:
            CANARY._read_limited_pipe(FakePipe([hidden]), 6)
        self.assertIn("non-byte data", str(caught.exception))
        self.assertNotIn(hidden, str(caught.exception))

    def test_direct_run_numeric_limits_must_exist_before_execution(self):
        cases = (
            ("output_limit_bytes", "--output-limit-bytes must be positive"),
            ("stage_timeout_secs", "--stage-timeout-secs must be a positive finite number"),
        )
        for field, expected in cases:
            with self.subTest(field=field):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    inbox = root / "inbox"
                    inbox.mkdir()
                    config = write_config(
                        root,
                        {
                            "provider": "local-bank",
                            "environment": "ci",
                            "rail": {
                                "inbox_dir": str(inbox),
                                "torii_base_url": "https://torii.local-bank.bank",
                            },
                        },
                    )
                    args = argparse.Namespace(
                        config=config,
                        require_explicit_policy=False,
                        output_limit_bytes=1024,
                        stage_timeout_secs=1.0,
                        summary_out=None,
                        plan_only=True,
                    )
                    delattr(args, field)

                    with self.assertRaises(CANARY.CanaryError) as caught:
                        CANARY.run(args)

                    self.assertIn(expected, str(caught.exception))

    def test_redacts_equals_form_bearer_token_arguments(self):
        redacted = CANARY._redacted_command(
            [
                "iso_rail_gateway_adapter.py",
                "--bearer-token-file=/ops/secrets/live-token",
                "--timeout-secs",
                "10",
            ]
        )

        self.assertEqual(
            redacted,
            [
                "iso_rail_gateway_adapter.py",
                "--bearer-token-file=<runtime-token-file>",
                "--timeout-secs",
                "10",
            ],
        )

    def test_checked_in_profile_runbook_templates_plan_without_network(self):
        template_dir = REPO_ROOT / "fixtures" / "iso20022" / "operator_canary"
        templates = sorted(template_dir.glob("*.example.json"))
        self.assertGreaterEqual(len(templates), 4)
        for template in templates:
            with self.subTest(template=template.name):
                rc, stdout, stderr = run_canary(
                    [
                        "--config",
                        str(template),
                        "--plan-only",
                        "--require-explicit-policy",
                    ]
                )
                self.assertEqual(rc, 0, stderr)
                summary = load_summary(stdout)
                self.assertEqual(summary["version"], CANARY.CANARY_SUMMARY_VERSION)
                self.assertTrue(summary["ok"])
                self.assertTrue(summary["plan_only"])
                self.assertTrue(summary["policy"]["require_explicit_policy"])
                self.assertEqual(
                    [stage["name"] for stage in summary["planned_stages"]],
                    ["rail", "notary", "verify"],
                )

    def test_non_plan_rejects_repository_fixture_stage_paths_before_execution(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            fixture_root = root / "fixtures" / "iso20022"
            fixture_root.mkdir(parents=True)
            live_root = root / "live"
            live_root.mkdir()

            def rail_body(**rail_overrides):
                rail = {
                    "inbox_dir": str(live_root / "inbox"),
                    "torii_base_url": "http://127.0.0.1:1",
                    "allow_insecure_http": True,
                }
                rail.update(rail_overrides)
                return {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": rail,
                    "verify": {"enabled": False},
                }

            def notary_body(**notary_overrides):
                notary = {
                    "export_dir": str(live_root / "audit-export"),
                    "dry_run": True,
                }
                notary.update(notary_overrides)
                return {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": notary,
                    "verify": {"enabled": False},
                }

            cases = (
                (
                    "config-path",
                    fixture_root / "operator-canary",
                    rail_body(),
                    "config path must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "rail-inbox",
                    root / "rail-inbox-case",
                    rail_body(inbox_dir=str(fixture_root / "rail-inbox")),
                    "rail.inbox_dir must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "rail-message",
                    root / "rail-message-case",
                    rail_body(message=str(fixture_root / "rail-inbox" / "payment.xml")),
                    "rail.message must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "rail-receipt-dir",
                    root / "rail-receipt-case",
                    rail_body(receipt_dir=str(fixture_root / "rail-receipts")),
                    "rail.receipt_dir must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "notary-export-dir",
                    root / "notary-export-case",
                    notary_body(export_dir=str(fixture_root / "audit-export")),
                    "notary.export_dir must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "notary-receipt-dir",
                    root / "notary-receipt-case",
                    notary_body(receipt_dir=str(fixture_root / "notary-receipts")),
                    "notary.receipt_dir must not point to checked-in ISO fixture artifacts",
                ),
            )
            for name, config_dir, body, expected in cases:
                with self.subTest(name=name):
                    config_dir.mkdir(parents=True, exist_ok=True)
                    config = write_config(config_dir, body)

                    rc, stdout, stderr = run_canary(["--config", str(config)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(expected, stderr)

    def test_non_plan_verify_rejects_repository_fixture_receipts(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            fixture_root = root / "fixtures" / "iso20022"
            fixture_root.mkdir(parents=True)

            cases = (
                (
                    "receipt-dir",
                    {"include_stage_receipts": False, "receipt_dirs": [str(fixture_root / "receipts")]},
                    [],
                    "verify.receipt_dirs[0] must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "receipt-file",
                    {"include_stage_receipts": False, "receipts": [str(fixture_root / "receipt.json")]},
                    [],
                    "verify.receipts[0] must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "stage-receipt-dir",
                    {"include_stage_receipts": True},
                    [fixture_root / "generated-receipts"],
                    "verify.receipt_dirs[0] must not point to checked-in ISO fixture artifacts",
                ),
            )
            for name, verify, stage_dirs, expected in cases:
                with self.subTest(name=name):
                    with self.assertRaises(CANARY.CanaryError) as caught:
                        CANARY._build_verify_stage(
                            root,
                            verify,
                            stage_dirs,
                            prior_failure=False,
                            require_explicit_policy=False,
                            allow_repository_fixture_paths=False,
                        )

                    self.assertIn(expected, str(caught.exception))

    def test_symlinked_config_is_rejected_before_plan(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "rail": {
                        "inbox_dir": "missing-inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                },
            )
            config = root / "symlinked-canary.json"
            try:
                config.symlink_to(target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("must not be a symlink", stderr)

    def test_symlinked_config_ancestor_is_rejected_before_plan(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target_dir = root / "config-target"
            target_dir.mkdir()
            target = write_config(
                target_dir,
                {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "rail": {
                        "inbox_dir": "missing-inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                },
            )
            ancestor = root / "config-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            config = ancestor / target.name

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("must not be a symlink", stderr)

    def test_config_path_diagnostics_do_not_echo_path(self):
        cases = (
            ("malformed-json", b"{", "is not valid JSON"),
            ("non-utf8-json", b"\xff", "is not UTF-8 JSON"),
            ("not-object", b"[]", "must be a JSON object"),
        )
        for name, payload, expected in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    hidden_dir = root / f"local-canary-leak-{name}"
                    hidden_dir.mkdir()
                    config = hidden_dir / "operator-canary.json"
                    config.write_bytes(payload)

                    rc, stdout, stderr = run_canary(
                        ["--config", str(config), "--plan-only"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(expected, stderr)
                    self.assertIn("config", stderr)
                    self.assertNotIn("line 1 column", stderr)
                    self.assertNotIn("(char ", stderr)
                    self.assertNotIn(str(config), stderr)
                    self.assertNotIn(hidden_dir.name, stderr)

    def test_symlinked_config_ancestor_diagnostic_does_not_echo_path(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target_dir = root / "config-target"
            target_dir.mkdir()
            target = write_config(
                target_dir,
                {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "rail": {
                        "inbox_dir": "missing-inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                },
            )
            hidden_link = root / "local-canary-leak-ancestor"
            try:
                hidden_link.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            config = hidden_link / target.name

            rc, stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)
            self.assertIn("config", stderr)
            self.assertNotIn(str(config), stderr)
            self.assertNotIn(hidden_link.name, stderr)

    def test_directory_config_is_rejected_before_plan(self):
        with tempfile.TemporaryDirectory() as raw_root:
            config = Path(raw_root) / "config-dir"
            config.mkdir()

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("must be a regular file", stderr)

    def test_oversized_config_is_rejected_before_plan(self):
        with tempfile.TemporaryDirectory() as raw_root:
            config = Path(raw_root) / "oversized-canary.json"
            config.write_text(
                '{"provider":"local-bank","environment":"preprod","padding":"'
                + ("a" * CANARY.MAX_CONFIG_JSON_BYTES)
                + '"}',
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("exceeds", stderr)

    def test_oversized_config_diagnostic_does_not_echo_path(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            hidden_dir = root / "local-canary-leak-oversized"
            hidden_dir.mkdir()
            config = hidden_dir / "operator-canary.json"
            config.write_text(
                '{"provider":"local-bank","environment":"preprod","padding":"'
                + ("a" * CANARY.MAX_CONFIG_JSON_BYTES)
                + '"}',
                encoding="utf-8",
            )

            rc, stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("exceeds", stderr)
            self.assertNotIn(str(config), stderr)
            self.assertNotIn(hidden_dir.name, stderr)

    def test_symlinked_summary_output_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "rail": {
                        "inbox_dir": "missing-inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                },
            )
            target = root / "summary-target.json"
            target.write_text("untouched\n", encoding="utf-8")
            summary_out = root / "summary-link.json"
            try:
                summary_out.symlink_to(target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, _stdout, stderr = run_canary(
                ["--config", str(config), "--plan-only", "--summary-out", str(summary_out)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must not be a symlink", stderr)
            self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")

    def test_config_cli_path_rejects_raw_smuggling_before_read(self):
        cases = (
            ("semicolon", "canary;debug.json", "semicolon path"),
            ("whitespace", "canary config.json", "whitespace"),
            ("leading-dash", "nested/-canary.json", "leading-dash path segments"),
            ("parent", "nested/../canary.json", "dot or parent"),
            ("dot", lambda root: f"{root}/nested/./canary.json", "dot or parent"),
            ("empty", lambda root: f"{root}//canary.json", "empty path"),
        )
        for name, config_arg, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    value = (
                        config_arg(root) if callable(config_arg) else str(root / config_arg)
                    )

                    rc, stdout, stderr = run_canary(["--config", value, "--plan-only"])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_direct_run_paths_reject_smuggling_before_config_loading(self):
        def args_for(root, **overrides):
            values = {
                "config": root / "missing-canary.json",
                "summary_out": root / "canary.summary.json",
                "plan_only": True,
                "require_explicit_policy": False,
                "output_limit_bytes": 1024,
                "stage_timeout_secs": 1.0,
            }
            values.update(overrides)
            return argparse.Namespace(**values)

        cases = (
            (
                "config whitespace",
                lambda root: args_for(root, config=root / "canary config.json"),
                "--config must not contain whitespace",
            ),
            (
                "summary parent",
                lambda root: args_for(
                    root,
                    summary_out=root / "nested" / ".." / "canary.summary.json",
                ),
                "summary_out must not contain dot or parent segments",
            ),
            (
                "summary repository fixture",
                lambda root: args_for(
                    root,
                    summary_out=(
                        root
                        / "fixtures"
                        / "iso20022"
                        / "operator_canary"
                        / "summary.json"
                    ),
                ),
                "summary_out must not point to checked-in ISO fixture artifacts",
            ),
            (
                "non-plan config repository fixture",
                lambda root: args_for(
                    root,
                    config=(
                        root
                        / "fixtures"
                        / "iso20022"
                        / "operator_canary"
                        / "missing.json"
                    ),
                    plan_only=False,
                ),
                "config path must not point to checked-in ISO fixture artifacts",
            ),
        )
        for name, make_args, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)

                    with self.assertRaises(CANARY.CanaryError) as caught:
                        CANARY.run(make_args(root))

                    error = str(caught.exception)
                    self.assertIn(message, error)
                    self.assertNotIn("does not exist", error)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            with self.assertRaisesRegex(CANARY.CanaryError, "provide --config"):
                CANARY.run(args_for(root, config=None))
            args = args_for(root)
            delattr(args, "config")
            with self.assertRaisesRegex(CANARY.CanaryError, "provide --config"):
                CANARY.run(args)

    def test_direct_run_scalar_paths_must_be_paths_before_config_loading(self):
        cases = (
            ("config", "config", object(), "--config"),
            ("summary", "summary_out", object(), "summary_out"),
        )
        for name, field, value, label in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        config=root / "missing-canary.json",
                        summary_out=None,
                        plan_only=True,
                        require_explicit_policy=False,
                        output_limit_bytes=1024,
                        stage_timeout_secs=1.0,
                    )
                    setattr(args, field, value)

                    with self.assertRaises(CANARY.CanaryError) as caught:
                        CANARY.run(args)

                    message = str(caught.exception)
                    self.assertIn(f"{label} must be a path", message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_direct_run_policy_flags_must_be_booleans_before_config_loading(self):
        cases = (
            ("plan_only", "--plan-only", "true"),
            ("require_explicit_policy", "--require-explicit-policy", 1),
        )
        for attr, label, value in cases:
            with self.subTest(flag=label):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        config=root / "missing-canary.json",
                        summary_out=None,
                        plan_only=True,
                        require_explicit_policy=False,
                        output_limit_bytes=1024,
                        stage_timeout_secs=1.0,
                    )
                    setattr(args, attr, value)

                    with self.assertRaises(CANARY.CanaryError) as caught:
                        CANARY.run(args)

                    message = str(caught.exception)
                    self.assertIn(f"{label} must be a boolean", message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_summary_output_path_rejects_smuggled_segments(self):
        cases = (
            ("semicolon", "summary;debug.json", "must not contain semicolon path parameters"),
            ("whitespace", "summary out.json", "must not contain whitespace"),
            ("leading-dash", "nested/-summary.json", "must not contain leading-dash path segments"),
            ("parent", "nested/../summary.json", "must not contain dot or parent segments"),
            ("dot", lambda root: f"{root}/nested/./summary.json", "dot or parent segments"),
            ("empty", lambda root: f"{root}//summary.json", "empty path segments"),
        )
        for name, summary_arg, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    config = write_config(
                        root,
                        {
                            "provider": "local-bank",
                            "environment": "preprod",
                            "rail": {
                                "inbox_dir": "missing-inbox",
                                "torii_base_url": "https://torii.local-bank.bank",
                            },
                        },
                    )

                    rc, _stdout, stderr = run_canary(
                        [
                            "--config",
                            str(config),
                            "--plan-only",
                            "--summary-out",
                            summary_arg(root) if callable(summary_arg) else str(root / summary_arg),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_hardlinked_summary_output_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "rail": {
                        "inbox_dir": "missing-inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                },
            )
            target = root / "summary-target.json"
            target.write_text("untouched\n", encoding="utf-8")
            summary_out = root / "summary-hardlink.json"
            try:
                summary_out.hardlink_to(target)
            except OSError as error:
                self.skipTest(f"hard link creation unavailable: {error}")

            rc, _stdout, stderr = run_canary(
                ["--config", str(config), "--plan-only", "--summary-out", str(summary_out)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must not be hard-linked", stderr)
            self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")

    def test_symlinked_summary_output_ancestor_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "rail": {
                        "inbox_dir": "missing-inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                },
            )
            target_dir = root / "summary-target"
            target_dir.mkdir()
            ancestor = root / "summary-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            summary_out = ancestor / "nested" / "summary.json"

            rc, _stdout, stderr = run_canary(
                ["--config", str(config), "--plan-only", "--summary-out", str(summary_out)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must not be a symlink", stderr)
            self.assertFalse((target_dir / "nested").exists())

    def test_relative_symlinked_stage_receipt_dir_reaches_child_boundary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            inbox.mkdir()
            rail_test.write_message(inbox)
            receipt_target = root / "receipt-target"
            receipt_target.mkdir()
            receipt_link = root / "receipt-link"
            try:
                receipt_link.symlink_to(receipt_target, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            with rail_test.capture_server() as (torii_url, requests):
                config = write_config(
                    root,
                    {
                        "provider": "local-bank",
                        "environment": "ci",
                        "rail": {
                            "inbox_dir": "inbox",
                            "receipt_dir": "receipt-link",
                            "torii_base_url": torii_url,
                            "allow_insecure_http": True,
                        },
                        "verify": {
                            "allow_insecure_http": True,
                            "require_source_files": True,
                        },
                    },
                )
                rc, stdout, stderr = run_canary(["--config", str(config)])

            self.assertEqual(rc, 1, stderr)
            self.assertEqual(requests, [])
            summary = load_summary(stdout)
            self.assertFalse(summary["ok"])
            rail_stage = summary["stages"][0]
            self.assertEqual(rail_stage["name"], "rail")
            self.assertEqual(rail_stage["returncode"], 2)
            self.assertIn(str(root.resolve() / receipt_link.name), rail_stage["command"])
            self.assertNotIn(str(root.resolve() / receipt_target.name), rail_stage["command"])
            self.assertIn("must not be a symlink", rail_stage["stderr_preview"])

    def test_child_output_is_bounded_while_drained(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            script_dir = root / "scripts"
            script_dir.mkdir()
            fake_rail = script_dir / "iso_rail_gateway_adapter.py"
            fake_rail.write_text(
                "\n".join(
                    [
                        "import sys",
                        "sys.stdout.write('O' * 32)",
                        "sys.stderr.write('E' * 32)",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            inbox = root / "inbox"
            inbox.mkdir()
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": str(inbox),
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                    "verify": {"enabled": False},
                },
            )
            original_script_dir = CANARY.SCRIPT_DIR
            CANARY.SCRIPT_DIR = script_dir
            try:
                rc, stdout, stderr = run_canary(
                    [
                        "--config",
                        str(config),
                        "--output-limit-bytes",
                        "8",
                    ]
                )
            finally:
                CANARY.SCRIPT_DIR = original_script_dir

            self.assertEqual(rc, 1, stderr)
            summary = load_summary(stdout)
            self.assertFalse(summary["ok"])
            stage = summary["stages"][0]
            self.assertEqual(stage["returncode"], 0)
            self.assertEqual(stage["stdout_preview"], "O" * 8)
            self.assertEqual(stage["stderr_preview"], "E" * 8)
            self.assertTrue(stage["stdout_truncated"])
            self.assertTrue(stage["stderr_truncated"])

    def test_truncated_verifier_output_marks_canary_failed(self):
        cases = (
            ("stdout", "sys.stdout.write('V' * 32)", "stdout_truncated"),
            ("stderr", "sys.stderr.write('E' * 32)", "stderr_truncated"),
        )
        for stream, write_line, truncated_key in cases:
            with self.subTest(stream=stream):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    script_dir = root / "scripts"
                    script_dir.mkdir()
                    fake_rail = script_dir / "iso_rail_gateway_adapter.py"
                    fake_rail.write_text(
                        "\n".join(
                            [
                                "import pathlib",
                                "import sys",
                                "receipt_dir = pathlib.Path(sys.argv[sys.argv.index('--receipt-dir') + 1])",
                                "receipt_dir.mkdir(parents=True, exist_ok=True)",
                                "sys.stdout.write('rail ok')",
                            ]
                        )
                        + "\n",
                        encoding="utf-8",
                    )
                    fake_verify = script_dir / "iso_operator_receipt_verify.py"
                    fake_verify.write_text(
                        "\n".join(
                            [
                                "import sys",
                                write_line,
                            ]
                        )
                        + "\n",
                        encoding="utf-8",
                    )
                    inbox = root / "inbox"
                    inbox.mkdir()
                    config = write_config(
                        root,
                        {
                            "provider": "local-bank",
                            "environment": "ci",
                            "rail": {
                                "inbox_dir": str(inbox),
                                "torii_base_url": "https://torii.local-bank.bank",
                            },
                        },
                    )
                    original_script_dir = CANARY.SCRIPT_DIR
                    CANARY.SCRIPT_DIR = script_dir
                    try:
                        rc, stdout, stderr = run_canary(
                            [
                                "--config",
                                str(config),
                                "--output-limit-bytes",
                                "8",
                            ]
                        )
                    finally:
                        CANARY.SCRIPT_DIR = original_script_dir

                    self.assertEqual(rc, 1, stderr)
                    summary = load_summary(stdout)
                    self.assertFalse(summary["ok"])
                    self.assertEqual([stage["name"] for stage in summary["stages"]], ["rail", "verify"])
                    verify_stage = summary["stages"][1]
                    self.assertEqual(verify_stage["returncode"], 0)
                    self.assertTrue(verify_stage[truncated_key])

    def test_successful_child_stderr_marks_canary_failed(self):
        cases = (
            (
                "rail",
                "sys.stderr.write('rail warning')",
                "",
                ["rail", "verify"],
                0,
            ),
            (
                "verify",
                "sys.stdout.write('rail ok')",
                "sys.stdout.write('{}')\nsys.stderr.write('verify warning')",
                ["rail", "verify"],
                1,
            ),
        )
        for stream, rail_line, verify_line, expected_stages, warning_stage_index in cases:
            with self.subTest(stream=stream):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    script_dir = root / "scripts"
                    script_dir.mkdir()
                    fake_rail = script_dir / "iso_rail_gateway_adapter.py"
                    fake_rail.write_text(
                        "\n".join(
                            [
                                "import pathlib",
                                "import sys",
                                "receipt_dir = pathlib.Path(sys.argv[sys.argv.index('--receipt-dir') + 1])",
                                "receipt_dir.mkdir(parents=True, exist_ok=True)",
                                rail_line,
                            ]
                        )
                        + "\n",
                        encoding="utf-8",
                    )
                    if verify_line:
                        fake_verify = script_dir / "iso_operator_receipt_verify.py"
                        fake_verify.write_text(
                            "\n".join(
                                [
                                    "import sys",
                                    verify_line,
                                ]
                            )
                            + "\n",
                            encoding="utf-8",
                        )
                    inbox = root / "inbox"
                    inbox.mkdir()
                    config = write_config(
                        root,
                        {
                            "provider": "local-bank",
                            "environment": "ci",
                            "rail": {
                                "inbox_dir": str(inbox),
                                "torii_base_url": "https://torii.local-bank.bank",
                            },
                            **({"verify": {"enabled": False}} if not verify_line else {}),
                        },
                    )
                    original_script_dir = CANARY.SCRIPT_DIR
                    CANARY.SCRIPT_DIR = script_dir
                    try:
                        rc, stdout, stderr = run_canary(["--config", str(config)])
                    finally:
                        CANARY.SCRIPT_DIR = original_script_dir

                    self.assertEqual(rc, 1, stderr)
                    summary = load_summary(stdout)
                    self.assertFalse(summary["ok"])
                    self.assertEqual(
                        [stage["name"] for stage in summary["stages"]],
                        expected_stages,
                    )
                    stage = summary["stages"][warning_stage_index]
                    self.assertEqual(stage["returncode"], 0)
                    self.assertFalse(stage["stderr_truncated"])
                    self.assertIn("warning", stage["stderr_preview"])

    def test_disabled_verify_stage_marks_executed_canary_failed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            script_dir = root / "scripts"
            script_dir.mkdir()
            fake_rail = script_dir / "iso_rail_gateway_adapter.py"
            fake_rail.write_text(
                "\n".join(
                    [
                        "import pathlib",
                        "import sys",
                        "receipt_dir = pathlib.Path(sys.argv[sys.argv.index('--receipt-dir') + 1])",
                        "receipt_dir.mkdir(parents=True, exist_ok=True)",
                        "sys.stdout.write('rail ok')",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            inbox = root / "inbox"
            inbox.mkdir()
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": str(inbox),
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                    "verify": {"enabled": False},
                },
            )
            original_script_dir = CANARY.SCRIPT_DIR
            CANARY.SCRIPT_DIR = script_dir
            try:
                rc, stdout, stderr = run_canary(["--config", str(config)])
            finally:
                CANARY.SCRIPT_DIR = original_script_dir

            self.assertEqual(rc, 1, stderr)
            summary = load_summary(stdout)
            self.assertFalse(summary["ok"])
            self.assertEqual(
                [stage["name"] for stage in summary["stages"]],
                ["rail", "verify"],
            )
            verify_stage = summary["stages"][1]
            self.assertTrue(verify_stage["skipped"])
            self.assertEqual(verify_stage["reason"], "skipped because verify.enabled=false")

    def test_secret_looking_child_output_is_rejected_before_summary_write(self):
        cases = [
            (
                "stdout",
                "sys.stdout.write('accepted token=canary-child-secret')",
                "stdout_preview contains secret-looking material",
            ),
            (
                "stdout-identifier",
                "sys.stdout.write('accepted token-canary-child-secret')",
                "stdout_preview contains secret-looking material",
            ),
            (
                "stderr",
                "sys.stderr.write('Authorization: Bearer canary-child-secret')",
                "stderr_preview contains secret-looking material",
            ),
            (
                "stderr-identifier",
                "sys.stderr.write('rejected cookie-canary-child-secret')",
                "stderr_preview contains secret-looking material",
            ),
        ]
        for stream, write_line, message in cases:
            with self.subTest(stream=stream):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    script_dir = root / "scripts"
                    script_dir.mkdir()
                    fake_rail = script_dir / "iso_rail_gateway_adapter.py"
                    fake_rail.write_text(
                        "\n".join(
                            [
                                "import sys",
                                write_line,
                            ]
                        )
                        + "\n",
                        encoding="utf-8",
                    )
                    inbox = root / "inbox"
                    inbox.mkdir()
                    summary_out = root / "summary" / "canary.summary.json"
                    config = write_config(
                        root,
                        {
                            "provider": "local-bank",
                            "environment": "ci",
                            "rail": {
                                "inbox_dir": str(inbox),
                                "torii_base_url": "https://torii.local-bank.bank",
                            },
                            "verify": {"enabled": False},
                        },
                    )
                    original_script_dir = CANARY.SCRIPT_DIR
                    CANARY.SCRIPT_DIR = script_dir
                    try:
                        rc, stdout, stderr = run_canary(
                            ["--config", str(config), "--summary-out", str(summary_out)]
                        )
                    finally:
                        CANARY.SCRIPT_DIR = original_script_dir

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertFalse(summary_out.exists())
                    self.assertIn(message, stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn("Authorization:", stderr)
                    self.assertNotIn("canary-child-secret", stderr)

    def test_control_bearing_child_output_is_rejected_before_summary_write(self):
        cases = [
            (
                "stdout",
                "sys.stdout.write('accepted \\x1b[31mwarning')",
                "stdout_preview contains unsafe control characters",
                "\x1b",
                "[31mwarning",
            ),
            (
                "stderr",
                "sys.stderr.write('rejected \\x1b[31mwarning')",
                "stderr_preview contains unsafe control characters",
                "\x1b",
                "[31mwarning",
            ),
            (
                "stdout bidi",
                "sys.stdout.write('accepted \\u202ebidi-warning')",
                "stdout_preview contains unsafe control characters",
                "\u202e",
                "bidi-warning",
            ),
            (
                "stderr bidi",
                "sys.stderr.write('rejected \\u202ebidi-warning')",
                "stderr_preview contains unsafe control characters",
                "\u202e",
                "bidi-warning",
            ),
        ]
        for stream, write_line, message, control, hidden in cases:
            with self.subTest(stream=stream):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    script_dir = root / "scripts"
                    script_dir.mkdir()
                    fake_rail = script_dir / "iso_rail_gateway_adapter.py"
                    fake_rail.write_text(
                        "\n".join(
                            [
                                "import sys",
                                write_line,
                            ]
                        )
                        + "\n",
                        encoding="utf-8",
                    )
                    inbox = root / "inbox"
                    inbox.mkdir()
                    summary_out = root / "summary" / "canary.summary.json"
                    config = write_config(
                        root,
                        {
                            "provider": "local-bank",
                            "environment": "ci",
                            "rail": {
                                "inbox_dir": str(inbox),
                                "torii_base_url": "https://torii.local-bank.bank",
                            },
                            "verify": {"enabled": False},
                        },
                    )
                    original_script_dir = CANARY.SCRIPT_DIR
                    CANARY.SCRIPT_DIR = script_dir
                    try:
                        rc, stdout, stderr = run_canary(
                            ["--config", str(config), "--summary-out", str(summary_out)]
                        )
                    finally:
                        CANARY.SCRIPT_DIR = original_script_dir

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertFalse(summary_out.exists())
                    self.assertIn(message, stderr)
                    self.assertNotIn(control, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_child_stage_timeout_is_bounded_and_recorded(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            script_dir = root / "scripts"
            script_dir.mkdir()
            fake_rail = script_dir / "iso_rail_gateway_adapter.py"
            fake_rail.write_text(
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
            inbox = root / "inbox"
            inbox.mkdir()
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": str(inbox),
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                    "verify": {"enabled": False},
                },
            )
            original_script_dir = CANARY.SCRIPT_DIR
            CANARY.SCRIPT_DIR = script_dir
            try:
                rc, stdout, stderr = run_canary(
                    [
                        "--config",
                        str(config),
                        "--stage-timeout-secs",
                        "1",
                    ]
                )
            finally:
                CANARY.SCRIPT_DIR = original_script_dir

            self.assertEqual(rc, 1, stderr)
            summary = load_summary(stdout)
            self.assertFalse(summary["ok"])
            stage = summary["stages"][0]
            self.assertEqual(stage["name"], "rail")
            self.assertEqual(stage["returncode"], 124)
            self.assertTrue(stage["timed_out"])
            self.assertEqual(stage["stdout_preview"], "started")
            self.assertEqual(stage["stderr_preview"], "")

    def test_stage_timeout_cli_rejects_nonpositive_and_nonfinite_values(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            inbox.mkdir()
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": str(inbox),
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                    "verify": {"enabled": False},
                },
            )
            for value in ("0", "-1", "nan", "inf"):
                with self.subTest(value=value):
                    rc, stdout, stderr = run_canary(
                        [
                            "--config",
                            str(config),
                            "--plan-only",
                            "--stage-timeout-secs",
                            value,
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("positive finite number", stderr)

    def test_require_explicit_policy_rejects_omitted_policy_booleans(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            base = {
                "provider": "local-bank",
                "environment": "ci",
                "rail": {
                    "inbox_dir": "inbox",
                    "torii_base_url": "https://torii.local-bank.bank",
                    "receipt_dir": "rail-receipts",
                    "dry_run": False,
                    "allow_default_profile": False,
                    "allow_insecure_http": False,
                },
                "notary": {
                    "export_dir": "audit-export",
                    "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                    "receipt_dir": "notary-receipts",
                    "all": False,
                    "dry_run": False,
                    "allow_insecure_http": False,
                },
                "verify": {
                    "enabled": True,
                    "include_stage_receipts": True,
                    "receipt_dirs": [],
                    "receipts": [],
                    "skip_on_stage_failure": True,
                    "allow_failed": False,
                    "allow_insecure_http": False,
                    "allow_default_profile": False,
                    "require_source_files": True,
                },
            }
            cases = (
                ("rail", "allow_default_profile"),
                ("rail", "allow_insecure_http"),
                ("rail", "dry_run"),
                ("notary", "all"),
                ("notary", "allow_insecure_http"),
                ("notary", "dry_run"),
                ("verify", "allow_failed"),
                ("verify", "allow_default_profile"),
                ("verify", "allow_insecure_http"),
                ("verify", "enabled"),
                ("verify", "include_stage_receipts"),
                ("verify", "require_source_files"),
                ("verify", "skip_on_stage_failure"),
            )
            for section, key in cases:
                with self.subTest(section=section, key=key):
                    body = json.loads(json.dumps(base))
                    body[section].pop(key)
                    config = write_config(root, body)

                    rc, _stdout, stderr = run_canary(
                        ["--config", str(config), "--plan-only", "--require-explicit-policy"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(f"{section}.{key}", stderr)

    def test_require_explicit_policy_rejects_omitted_lists(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            base = {
                "provider": "local-bank",
                "environment": "ci",
                "rail": {
                    "inbox_dir": "inbox",
                    "torii_base_url": "https://torii.local-bank.bank",
                    "receipt_dir": "rail-receipts",
                    "dry_run": False,
                    "allow_default_profile": False,
                    "allow_insecure_http": False,
                },
                "notary": {
                    "export_dir": "audit-export",
                    "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                    "receipt_dir": "notary-receipts",
                    "all": False,
                    "dry_run": False,
                    "allow_insecure_http": False,
                },
                "verify": {
                    "enabled": True,
                    "include_stage_receipts": True,
                    "receipt_dirs": [],
                    "receipts": [],
                    "skip_on_stage_failure": True,
                    "allow_failed": False,
                    "allow_insecure_http": False,
                    "allow_default_profile": False,
                    "require_source_files": True,
                },
            }
            cases = (
                ("notary", "endpoints"),
                ("verify", "receipt_dirs"),
                ("verify", "receipts"),
            )
            for section, key in cases:
                with self.subTest(section=section, key=key):
                    body = json.loads(json.dumps(base))
                    body[section].pop(key)
                    config = write_config(root, body)

                    rc, _stdout, stderr = run_canary(
                        ["--config", str(config), "--plan-only", "--require-explicit-policy"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(f"{section}.{key} must be explicitly recorded as an array", stderr)

    def test_require_explicit_policy_requires_stage_receipt_dirs(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            base = {
                "provider": "local-bank",
                "environment": "ci",
                "rail": {
                    "inbox_dir": "inbox",
                    "torii_base_url": "https://torii.local-bank.bank",
                    "receipt_dir": "rail-receipts",
                    "dry_run": False,
                    "allow_default_profile": False,
                    "allow_insecure_http": False,
                },
                "notary": {
                    "export_dir": "audit-export",
                    "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                    "receipt_dir": "notary-receipts",
                    "all": False,
                    "dry_run": False,
                    "allow_insecure_http": False,
                },
                "verify": {
                    "enabled": True,
                    "include_stage_receipts": True,
                    "receipt_dirs": [],
                    "receipts": [],
                    "skip_on_stage_failure": True,
                    "allow_failed": False,
                    "allow_insecure_http": False,
                    "allow_default_profile": False,
                    "require_source_files": True,
                },
            }
            cases = (
                ("rail", "rail.receipt_dir must be explicitly recorded"),
                ("notary", "notary.receipt_dir must be explicitly recorded"),
            )
            for section, message in cases:
                with self.subTest(section=section):
                    body = json.loads(json.dumps(base))
                    body[section].pop("receipt_dir")
                    config = write_config(root, body)

                    rc, stdout, stderr = run_canary(
                        ["--config", str(config), "--plan-only", "--require-explicit-policy"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_require_explicit_policy_rejects_stage_receipt_source_overlap(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            base = {
                "provider": "local-bank",
                "environment": "ci",
                "rail": {
                    "inbox_dir": "inbox",
                    "torii_base_url": "https://torii.local-bank.bank",
                    "receipt_dir": "rail-receipts",
                    "dry_run": False,
                    "allow_default_profile": False,
                    "allow_insecure_http": False,
                },
                "notary": {
                    "export_dir": "audit-export",
                    "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                    "receipt_dir": "notary-receipts",
                    "all": False,
                    "dry_run": False,
                    "allow_insecure_http": False,
                },
                "verify": {
                    "enabled": True,
                    "include_stage_receipts": True,
                    "receipt_dirs": [],
                    "receipts": [],
                    "skip_on_stage_failure": True,
                    "allow_failed": False,
                    "allow_insecure_http": False,
                    "allow_default_profile": False,
                    "require_source_files": True,
                },
            }
            cases = (
                (
                    "rail-same",
                    lambda body: body["rail"].__setitem__("receipt_dir", "inbox"),
                    "rail.receipt_dir must not overlap rail.inbox_dir",
                ),
                (
                    "rail-nested",
                    lambda body: body["rail"].__setitem__("receipt_dir", "inbox/receipts"),
                    "rail.receipt_dir must not overlap rail.inbox_dir",
                ),
                (
                    "notary-same",
                    lambda body: body["notary"].__setitem__("receipt_dir", "audit-export"),
                    "notary.receipt_dir must not overlap notary.export_dir",
                ),
                (
                    "notary-nested",
                    lambda body: body["notary"].__setitem__(
                        "receipt_dir", "audit-export/receipts"
                    ),
                    "notary.receipt_dir must not overlap notary.export_dir",
                ),
            )
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    body = json.loads(json.dumps(base))
                    mutate(body)
                    config = write_config(root, body)

                    rc, stdout, stderr = run_canary(
                        ["--config", str(config), "--plan-only", "--require-explicit-policy"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_require_explicit_policy_rejects_stage_receipt_bearer_token_overlap(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            base = {
                "provider": "local-bank",
                "environment": "ci",
                "rail": {
                    "inbox_dir": "inbox",
                    "torii_base_url": "https://torii.local-bank.bank",
                    "receipt_dir": "rail-receipts",
                    "dry_run": False,
                    "allow_default_profile": False,
                    "allow_insecure_http": False,
                },
                "notary": {
                    "export_dir": "audit-export",
                    "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                    "receipt_dir": "notary-receipts",
                    "all": False,
                    "dry_run": False,
                    "allow_insecure_http": False,
                },
                "verify": {
                    "enabled": True,
                    "include_stage_receipts": True,
                    "receipt_dirs": [],
                    "receipts": [],
                    "skip_on_stage_failure": True,
                    "allow_failed": False,
                    "allow_insecure_http": False,
                    "allow_default_profile": False,
                    "require_source_files": True,
                },
            }
            cases = (
                (
                    "rail-token-under-receipts",
                    lambda body: body["rail"].__setitem__(
                        "bearer_token_file",
                        "rail-receipts/runtime-rail-proof",
                    ),
                    "rail.receipt_dir must not overlap rail.bearer_token_file",
                    "runtime-rail-proof",
                ),
                (
                    "rail-receipts-under-token-path",
                    lambda body: (
                        body["rail"].__setitem__("receipt_dir", "runtime/rail-proof/receipts"),
                        body["rail"].__setitem__("bearer_token_file", "runtime/rail-proof"),
                    ),
                    "rail.receipt_dir must not overlap rail.bearer_token_file",
                    "runtime/rail-proof",
                ),
                (
                    "notary-token-under-receipts",
                    lambda body: body["notary"].__setitem__(
                        "bearer_token_file",
                        "notary-receipts/runtime-notary-proof",
                    ),
                    "notary.receipt_dir must not overlap notary.bearer_token_file",
                    "runtime-notary-proof",
                ),
                (
                    "notary-receipts-under-token-path",
                    lambda body: (
                        body["notary"].__setitem__(
                            "receipt_dir",
                            "runtime/notary-proof/receipts",
                        ),
                        body["notary"].__setitem__(
                            "bearer_token_file",
                            "runtime/notary-proof",
                        ),
                    ),
                    "notary.receipt_dir must not overlap notary.bearer_token_file",
                    "runtime/notary-proof",
                ),
            )
            for name, mutate, message, hidden_path in cases:
                with self.subTest(name=name):
                    body = json.loads(json.dumps(base))
                    mutate(body)
                    config = write_config(root, body)

                    rc, stdout, stderr = run_canary(
                        ["--config", str(config), "--plan-only", "--require-explicit-policy"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden_path, stderr)

    def test_require_explicit_policy_rejects_stage_source_bearer_token_overlap(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            base = {
                "provider": "local-bank",
                "environment": "ci",
                "rail": {
                    "inbox_dir": "inbox",
                    "torii_base_url": "https://torii.local-bank.bank",
                    "receipt_dir": "rail-receipts",
                    "dry_run": False,
                    "allow_default_profile": False,
                    "allow_insecure_http": False,
                },
                "notary": {
                    "export_dir": "audit-export",
                    "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                    "receipt_dir": "notary-receipts",
                    "all": False,
                    "dry_run": False,
                    "allow_insecure_http": False,
                },
                "verify": {
                    "enabled": True,
                    "include_stage_receipts": True,
                    "receipt_dirs": [],
                    "receipts": [],
                    "skip_on_stage_failure": True,
                    "allow_failed": False,
                    "allow_insecure_http": False,
                    "allow_default_profile": False,
                    "require_source_files": True,
                },
            }
            cases = (
                (
                    "rail-token-under-inbox",
                    lambda body: body["rail"].__setitem__(
                        "bearer_token_file",
                        "inbox/runtime-rail-proof",
                    ),
                    "rail.bearer_token_file must not overlap rail.inbox_dir",
                    "runtime-rail-proof",
                ),
                (
                    "rail-inbox-under-token-path",
                    lambda body: (
                        body["rail"].__setitem__("inbox_dir", "runtime/rail-proof/inbox"),
                        body["rail"].__setitem__("bearer_token_file", "runtime/rail-proof"),
                    ),
                    "rail.bearer_token_file must not overlap rail.inbox_dir",
                    "runtime/rail-proof",
                ),
                (
                    "notary-token-under-export",
                    lambda body: body["notary"].__setitem__(
                        "bearer_token_file",
                        "audit-export/runtime-notary-proof",
                    ),
                    "notary.bearer_token_file must not overlap notary.export_dir",
                    "runtime-notary-proof",
                ),
                (
                    "notary-export-under-token-path",
                    lambda body: (
                        body["notary"].__setitem__(
                            "export_dir",
                            "runtime/notary-proof/export",
                        ),
                        body["notary"].__setitem__(
                            "bearer_token_file",
                            "runtime/notary-proof",
                        ),
                    ),
                    "notary.bearer_token_file must not overlap notary.export_dir",
                    "runtime/notary-proof",
                ),
            )
            for name, mutate, message, hidden_path in cases:
                with self.subTest(name=name):
                    body = json.loads(json.dumps(base))
                    mutate(body)
                    config = write_config(root, body)

                    rc, stdout, stderr = run_canary(
                        ["--config", str(config), "--plan-only", "--require-explicit-policy"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden_path, stderr)

    def test_verify_policy_must_cover_generated_receipt_overrides(self):
        cases = (
            (
                "rail-default-profile",
                lambda body: body["rail"].__setitem__("allow_default_profile", True),
                "verify.allow_default_profile must be true when rail.allow_default_profile is true",
            ),
            (
                "rail-insecure-http",
                lambda body: body["rail"].__setitem__("allow_insecure_http", True),
                "verify.allow_insecure_http must be true when rail.allow_insecure_http is true",
            ),
            (
                "notary-insecure-http",
                lambda body: body["notary"].__setitem__("allow_insecure_http", True),
                "verify.allow_insecure_http must be true when notary.allow_insecure_http is true",
            ),
            (
                "verify-source-files-disabled",
                lambda body: body["verify"].__setitem__("require_source_files", False),
                "verify.require_source_files must be true when generated stage receipts are verified",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    body = {
                        "provider": "local-bank",
                        "environment": "ci",
                        "rail": {
                            "inbox_dir": "inbox",
                            "torii_base_url": "https://torii.local-bank.bank",
                            "receipt_dir": "rail-receipts",
                            "dry_run": False,
                            "allow_default_profile": False,
                            "allow_insecure_http": False,
                        },
                        "notary": {
                            "export_dir": "audit-export",
                            "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                            "receipt_dir": "notary-receipts",
                            "all": False,
                            "dry_run": False,
                            "allow_insecure_http": False,
                        },
                        "verify": {
                            "enabled": True,
                            "include_stage_receipts": True,
                            "receipt_dirs": [],
                            "receipts": [],
                            "skip_on_stage_failure": True,
                            "allow_failed": False,
                            "allow_insecure_http": False,
                            "allow_default_profile": False,
                            "require_source_files": True,
                        },
                    }
                    mutate(body)
                    config = write_config(root, body)

                    rc, stdout, stderr = run_canary(
                        ["--config", str(config), "--plan-only", "--require-explicit-policy"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_verify_policy_must_cover_explicit_generated_receipt_dirs(self):
        cases = (
            (
                "rail-default-profile",
                "rail-receipts",
                lambda body: body["rail"].__setitem__("allow_default_profile", True),
                "verify.allow_default_profile must be true when rail.allow_default_profile is true",
            ),
            (
                "rail-insecure-http",
                "rail-receipts",
                lambda body: body["rail"].__setitem__("allow_insecure_http", True),
                "verify.allow_insecure_http must be true when rail.allow_insecure_http is true",
            ),
            (
                "notary-insecure-http",
                "notary-receipts",
                lambda body: body["notary"].__setitem__("allow_insecure_http", True),
                "verify.allow_insecure_http must be true when notary.allow_insecure_http is true",
            ),
            (
                "verify-source-files-disabled",
                "rail-receipts",
                lambda body: body["verify"].__setitem__("require_source_files", False),
                "verify.require_source_files must be true when generated stage receipts are verified",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            for name, receipt_dir, mutate, message in cases:
                with self.subTest(name=name):
                    body = {
                        "provider": "local-bank",
                        "environment": "ci",
                        "rail": {
                            "inbox_dir": "inbox",
                            "torii_base_url": "https://torii.local-bank.bank",
                            "receipt_dir": "rail-receipts",
                            "dry_run": False,
                            "allow_default_profile": False,
                            "allow_insecure_http": False,
                        },
                        "notary": {
                            "export_dir": "audit-export",
                            "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                            "receipt_dir": "notary-receipts",
                            "all": False,
                            "dry_run": False,
                            "allow_insecure_http": False,
                        },
                        "verify": {
                            "enabled": True,
                            "include_stage_receipts": False,
                            "receipt_dirs": ["rail-receipts", "notary-receipts"],
                            "receipts": [],
                            "skip_on_stage_failure": True,
                            "allow_failed": False,
                            "allow_insecure_http": False,
                            "allow_default_profile": False,
                            "require_source_files": True,
                        },
                    }
                    mutate(body)
                    config = write_config(root, body)

                    rc, stdout, stderr = run_canary(
                        ["--config", str(config), "--plan-only", "--require-explicit-policy"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_verify_policy_must_cover_symlinked_explicit_generated_receipt_dir(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            rail_receipts = root / "rail-receipts"
            rail_receipts.mkdir()
            rail_receipt_link = root / "linked-rail-receipts"
            try:
                rail_receipt_link.symlink_to(rail_receipts, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                        "receipt_dir": "rail-receipts",
                        "dry_run": False,
                        "allow_default_profile": True,
                        "allow_insecure_http": False,
                    },
                    "verify": {
                        "enabled": True,
                        "include_stage_receipts": False,
                        "receipt_dirs": ["linked-rail-receipts"],
                        "receipts": [],
                        "skip_on_stage_failure": True,
                        "allow_failed": False,
                        "allow_insecure_http": False,
                        "allow_default_profile": False,
                        "require_source_files": True,
                    },
                },
            )

            rc, stdout, stderr = run_canary(
                ["--config", str(config), "--plan-only", "--require-explicit-policy"]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "verify.allow_default_profile must be true when "
                "rail.allow_default_profile is true",
                stderr,
            )

    def test_verify_policy_accepts_matching_explicit_generated_receipt_dirs(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                        "receipt_dir": "rail-receipts",
                        "dry_run": False,
                        "allow_default_profile": True,
                        "allow_insecure_http": True,
                    },
                    "notary": {
                        "export_dir": "audit-export",
                        "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                        "receipt_dir": "notary-receipts",
                        "all": False,
                        "dry_run": False,
                        "allow_insecure_http": True,
                    },
                    "verify": {
                        "enabled": True,
                        "include_stage_receipts": False,
                        "receipt_dirs": ["rail-receipts", "notary-receipts"],
                        "receipts": [],
                        "skip_on_stage_failure": True,
                        "allow_failed": False,
                        "allow_insecure_http": True,
                        "allow_default_profile": True,
                        "require_source_files": True,
                    },
                },
            )

            rc, stdout, stderr = run_canary(
                ["--config", str(config), "--plan-only", "--require-explicit-policy"]
            )

            self.assertEqual(rc, 0, stderr)
            summary = load_summary(stdout)
            verify_command = next(
                stage["command"]
                for stage in summary["planned_stages"]
                if stage["name"] == "verify"
            )
            self.assertIn("--allow-insecure-http", verify_command)
            self.assertIn("--allow-default-profile", verify_command)

    def test_verify_policy_rejects_unselected_generated_receipt_dirs(self):
        cases = (
            (
                "external-only-missing-stage-receipts",
                "external-receipts",
                lambda _body: None,
                "verify must cover generated rail/notary receipt directories",
            ),
            (
                "notary-only-missing-rail-receipts",
                "notary-receipts",
                lambda body: body["rail"].update(
                    {
                        "allow_default_profile": True,
                        "allow_insecure_http": True,
                    }
                ),
                "verify must cover generated rail receipt directories",
            ),
            (
                "rail-only-missing-notary-receipts",
                "rail-receipts",
                lambda body: body["notary"].__setitem__("allow_insecure_http", True),
                "verify must cover generated notary receipt directories",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            for name, receipt_dir, mutate, message in cases:
                with self.subTest(name=name):
                    body = {
                        "provider": "local-bank",
                        "environment": "ci",
                        "rail": {
                            "inbox_dir": "inbox",
                            "torii_base_url": "https://torii.local-bank.bank",
                            "receipt_dir": "rail-receipts",
                            "dry_run": False,
                            "allow_default_profile": False,
                            "allow_insecure_http": False,
                        },
                        "notary": {
                            "export_dir": "audit-export",
                            "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                            "receipt_dir": "notary-receipts",
                            "all": False,
                            "dry_run": False,
                            "allow_insecure_http": False,
                        },
                        "verify": {
                            "enabled": True,
                            "include_stage_receipts": False,
                            "receipt_dirs": [receipt_dir],
                            "receipts": [],
                            "skip_on_stage_failure": True,
                            "allow_failed": False,
                            "allow_insecure_http": False,
                            "allow_default_profile": False,
                            "require_source_files": True,
                        },
                    }
                    mutate(body)
                    config = write_config(root, body)

                    rc, stdout, stderr = run_canary(
                        ["--config", str(config), "--plan-only", "--require-explicit-policy"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_verify_policy_accepts_matching_generated_receipt_overrides(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                        "receipt_dir": "rail-receipts",
                        "dry_run": False,
                        "allow_default_profile": True,
                        "allow_insecure_http": True,
                    },
                    "notary": {
                        "export_dir": "audit-export",
                        "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                        "receipt_dir": "notary-receipts",
                        "all": False,
                        "dry_run": False,
                        "allow_insecure_http": True,
                    },
                    "verify": {
                        "enabled": True,
                        "include_stage_receipts": True,
                        "receipt_dirs": [],
                        "receipts": [],
                        "skip_on_stage_failure": True,
                        "allow_failed": False,
                        "allow_insecure_http": True,
                        "allow_default_profile": True,
                        "require_source_files": True,
                    },
                },
            )

            rc, stdout, stderr = run_canary(
                ["--config", str(config), "--plan-only", "--require-explicit-policy"]
            )

            self.assertEqual(rc, 0, stderr)
            summary = load_summary(stdout)
            verify_command = next(
                stage["command"]
                for stage in summary["planned_stages"]
                if stage["name"] == "verify"
            )
            self.assertIn("--allow-insecure-http", verify_command)
            self.assertIn("--allow-default-profile", verify_command)

    def test_unknown_config_key_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "unexpected": True,
                },
            )

            rc, _stdout, stderr = run_canary(["--config", str(config)])

            self.assertEqual(rc, 2)
            self.assertIn("unknown keys", stderr)

    def test_duplicate_runbook_json_keys_are_rejected_before_planning(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = root / "canary.json"
            config.write_text(
                (
                    '{"provider":"local-bank",'
                    '"token=canary-duplicate-key-secret":1,'
                    '"token=canary-duplicate-key-secret":2,'
                    '"environment":"ci",'
                    '"rail":{"inbox_dir":"inbox",'
                    '"torii_base_url":"https://torii.local-bank.bank"}}\n'
                ),
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)
            self.assertNotIn("canary-duplicate-key-secret", stderr)

    def test_non_finite_runbook_json_numbers_are_rejected_before_planning(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = root / "canary.json"
            config.write_text(
                (
                    '{"provider":"local-bank","environment":"ci",'
                    '"rail":{"inbox_dir":"inbox",'
                    '"torii_base_url":"https://torii.local-bank.bank",'
                    '"timeout_secs":Infinity}}\n'
                ),
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("non-finite numeric constant", stderr)
            self.assertNotIn("Infinity", stderr)

    def test_noncanonical_runbook_json_numbers_are_rejected_before_planning(self):
        for value in ("1e01", "-0", "-0.0", "-0e0"):
            with self.subTest(value=value):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    config = root / "canary.json"
                    config.write_text(
                        (
                            '{"provider":"local-bank","environment":"ci",'
                            '"rail":{"inbox_dir":"inbox",'
                            '"torii_base_url":"https://torii.local-bank.bank",'
                            f'"timeout_secs":{value}}}\n'
                        ),
                        encoding="utf-8",
                    )

                    rc, stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("non-canonical numeric value", stderr)
                    self.assertNotIn(value, stderr)

    def test_runbook_json_surrogate_strings_are_rejected_before_planning(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = root / "canary.json"
            config.write_text(
                (
                    '{"provider":"\\ud800","environment":"ci",'
                    '"rail":{"inbox_dir":"inbox",'
                    '"torii_base_url":"https://torii.local-bank.bank"}}\n'
                ),
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("invalid Unicode surrogate", stderr)

    def test_endpoint_urls_are_validated_before_planning(self):
        long_torii_url = "https://torii.example/" + ("a" * CANARY.MAX_HTTP_URL_CHARS)
        long_notary_url = "https://notary.example/" + ("a" * CANARY.MAX_HTTP_URL_CHARS)
        long_host = ".".join(["a" * 63] * 4 + ["example"])
        cases = [
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://user:pass@torii.local-bank.bank",
                    },
                },
                "credentials",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank?token=abc",
                    },
                },
                "params, query, or fragment",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.example",
                    },
                },
                "reserved placeholder hostnames",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.example.com/base",
                    },
                },
                "reserved placeholder hostnames",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.example.net/base",
                    },
                },
                "reserved placeholder hostnames",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.example.org/base",
                    },
                },
                "reserved placeholder hostnames",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.example.invalid/base",
                    },
                },
                "reserved placeholder hostnames",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank/iso bridge",
                    },
                },
                "must not contain whitespace",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": long_torii_url,
                    },
                },
                "must be no longer than 2048 characters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank:abc",
                    },
                },
                "invalid port",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank:",
                    },
                },
                "empty port",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank:0",
                    },
                },
                "port must be positive",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank:08443",
                    },
                },
                "port must not contain leading zeros",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://[::1",
                    },
                },
                "is not a valid URL",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank:443",
                    },
                },
                "default port",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://Torii.example.invalid",
                    },
                },
                "host must be lowercase",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank.",
                    },
                },
                "host must not end with a dot",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii..example.invalid",
                    },
                },
                "host must not contain empty labels",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://localhost/base",
                    },
                },
                "must not use localhost",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://127.0.0.1.nip.io/base",
                    },
                },
                "must not use local/private rebinding hostnames",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://0x7f000001/base",
                    },
                },
                "host must not use legacy IPv4 numeric notation",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://[64:ff9b::7f00:1]/base",
                    },
                },
                "must not embed local, private, or reserved IPv4 addresses",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": f"https://{long_host}/base",
                    },
                },
                "host must be at most 253 characters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://-torii.local-bank.bank",
                    },
                },
                "host labels must not start or end with hyphen",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii._tcp.example.invalid",
                    },
                },
                "host labels must use lowercase ASCII letters, digits, or hyphens",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.example%2einvalid",
                    },
                },
                "host must not contain percent escapes",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://123.000.000.001",
                    },
                },
                "numeric host labels must be a valid IP address",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank/../base",
                    },
                },
                "path must not contain dot segments",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank/base//v1",
                    },
                },
                "path must not contain empty segments",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank/base%2fv1",
                    },
                },
                "path must not contain encoded dot or separator characters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank/base%252fv1",
                    },
                },
                "path must not contain encoded percent characters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank/base%20v1",
                    },
                },
                "percent-encoded control or space characters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank/base%00v1",
                    },
                },
                "percent-encoded control or space characters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank/base%zzv1",
                    },
                },
                "malformed percent escapes",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank/base;debug/v1",
                    },
                },
                "path must not contain semicolon parameters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank/base%3bdebug/v1",
                    },
                },
                "path must not contain encoded semicolon parameters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank/base%3Fdebug/v1",
                    },
                },
                "path must not contain encoded URL delimiter characters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/iso-anchor#frag"],
                    },
                },
                "params, query, or fragment",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.example/iso-anchor"],
                    },
                },
                "reserved placeholder hostnames",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.example.com/iso-anchor"],
                    },
                },
                "reserved placeholder hostnames",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.example.net/iso-anchor"],
                    },
                },
                "reserved placeholder hostnames",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.example.org/iso-anchor"],
                    },
                },
                "reserved placeholder hostnames",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.example.invalid/iso-anchor"],
                    },
                },
                "reserved placeholder hostnames",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/iso anchor"],
                    },
                },
                "must not contain whitespace",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": [long_notary_url],
                    },
                },
                "must be no longer than 2048 characters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank:99999/iso-anchor"],
                    },
                },
                "invalid port",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank:/iso-anchor"],
                    },
                },
                "empty port",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank:0/iso-anchor"],
                    },
                },
                "port must be positive",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank:08443/iso-anchor"],
                    },
                },
                "port must not contain leading zeros",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://[]/iso-anchor"],
                    },
                },
                "is not a valid URL",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank:443/iso-anchor"],
                    },
                },
                "default port",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://Notary.example.invalid/iso-anchor"],
                    },
                },
                "host must be lowercase",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank./iso-anchor"],
                    },
                },
                "host must not end with a dot",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary..example.invalid/iso-anchor"],
                    },
                },
                "host must not contain empty labels",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://127.0.0.1/iso-anchor"],
                    },
                },
                "must not use local, private, or reserved IP addresses",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://service.localtest.me/iso-anchor"],
                    },
                },
                "must not use local/private rebinding hostnames",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://0x7f.0.0.1/iso-anchor"],
                    },
                },
                "host must not use legacy IPv4 numeric notation",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://[::127.0.0.1]/iso-anchor"],
                    },
                },
                "must not embed local, private, or reserved IPv4 addresses",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": [f"https://{long_host}/iso-anchor"],
                    },
                },
                "host must be at most 253 characters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.example%2einvalid/iso-anchor"],
                    },
                },
                "host must not contain percent escapes",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/../iso-anchor"],
                    },
                },
                "path must not contain dot segments",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/iso//anchor"],
                    },
                },
                "path must not contain empty segments",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/iso%2fanchor"],
                    },
                },
                "path must not contain encoded dot or separator characters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/iso%252fanchor"],
                    },
                },
                "path must not contain encoded percent characters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/iso%3bdebug/anchor"],
                    },
                },
                "path must not contain encoded semicolon parameters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/iso%40debug/anchor"],
                    },
                },
                "path must not contain encoded URL delimiter characters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/iso%20anchor"],
                    },
                },
                "percent-encoded control or space characters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/iso%zzanchor"],
                    },
                },
                "malformed percent escapes",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": [r"https://notary.local-bank.bank/iso\anchor"],
                    },
                },
                "path must use forward slashes",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "http://torii.local-bank.bank",
                    },
                },
                "must use HTTPS",
            ),
        ]
        for body, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    config = write_config(root, body)

                    rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_template_endpoint_hosts_are_plan_only(self):
        cases = (
            (
                "rail",
                "https://torii.swift-cbpr-plus.operator-canary.bank/base",
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.swift-cbpr-plus.operator-canary.bank/base",
                    },
                },
            ),
            (
                "notary",
                "https://notary.swift-cbpr-plus.operator-canary.bank/anchor",
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": [
                            "https://notary.swift-cbpr-plus.operator-canary.bank/anchor"
                        ],
                    },
                },
            ),
        )
        for label, endpoint, body in cases:
            with self.subTest(label=label):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    config = write_config(root, body)

                    rc, stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

                    self.assertEqual(rc, 0, stderr)
                    self.assertIn("planned_stages", stdout)

                    rc, _stdout, stderr = run_canary(["--config", str(config)])

                    self.assertEqual(rc, 2)
                    self.assertIn("template canary hostnames", stderr)
                    self.assertNotIn(endpoint, stderr)

    def test_rejected_runbook_url_does_not_echo_secret_query(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            secret_url = "https://torii.local-bank.bank?token=canary-url-secret"
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": secret_url,
                    },
                },
            )

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("params, query, or fragment", stderr)
            self.assertNotIn(secret_url, stderr)
            self.assertNotIn("token=", stderr)
            self.assertNotIn("canary-url-secret", stderr)

    def test_duplicate_runbook_evidence_inputs_are_rejected_before_planning(self):
        cases = [
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": [
                            "https://notary.local-bank.bank/iso-anchor",
                            "https://notary.local-bank.bank/iso-anchor",
                        ],
                    },
                },
                "notary.endpoints[1] duplicates notary.endpoints[0]",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "dry_run": True,
                    },
                    "verify": {
                        "include_stage_receipts": False,
                        "receipts": ["receipts/a.receipt.json", "receipts/a.receipt.json"],
                    },
                },
                "verify.receipts[1] duplicates verify.receipts[0]",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "dry_run": True,
                    },
                    "verify": {
                        "include_stage_receipts": False,
                        "receipt_dirs": ["receipts", "receipts"],
                    },
                },
                "verify.receipt_dirs[1] duplicates verify.receipt_dirs[0]",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "dry_run": True,
                    },
                    "verify": {
                        "include_stage_receipts": False,
                        "receipt_dirs": ["receipts"],
                        "receipts": ["receipts/a.receipt.json"],
                    },
                },
                "verify.receipts[0] is already covered by verify.receipt_dirs[0]",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                        "receipt_dir": "rail-receipts",
                    },
                    "verify": {
                        "include_stage_receipts": False,
                        "receipts": ["rail-receipts/manual.receipt.json"],
                    },
                },
                "verify must cover generated rail receipt directories",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                        "receipt_dir": "rail-receipts",
                    },
                    "verify": {
                        "receipts": ["rail-receipts/manual.receipt.json"],
                    },
                },
                "verify.receipts[0] is already covered by verify.receipt_dirs[0]",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                        "receipt_dir": "shared-receipts",
                    },
                    "notary": {
                        "export_dir": "export",
                        "receipt_dir": "shared-receipts",
                        "dry_run": True,
                    },
                },
                "stage.receipt_dir[1] duplicates stage.receipt_dir[0]",
            ),
        ]
        for body, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    config = write_config(root, body)

                    rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_repeatable_command_selectors_are_emitted_in_canonical_order(self):
        def flag_values(command, flag):
            values = []
            for offset, item in enumerate(command):
                if item == flag:
                    values.append(command[offset + 1])
                elif item.startswith(flag + "="):
                    values.append(item.split("=", 1)[1])
            return values

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "audit-export",
                        "dry_run": True,
                        "endpoints": [
                            "https://z-notary.local-bank.bank/iso-anchor",
                            "https://a-notary.local-bank.bank/iso-anchor",
                        ],
                    },
                    "verify": {
                        "include_stage_receipts": False,
                        "receipt_dirs": ["z-receipts", "a-receipts"],
                        "receipts": [
                            "manual-z/z.receipt.json",
                            "manual-a/a.receipt.json",
                        ],
                    },
                },
            )

            rc, stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 0, stderr)
            summary = load_summary(stdout)
            notary_stage = next(
                stage for stage in summary["planned_stages"] if stage["name"] == "notary"
            )
            verify_stage = next(
                stage for stage in summary["planned_stages"] if stage["name"] == "verify"
            )
            endpoint_values = flag_values(notary_stage["command"], "--endpoint")
            receipt_dir_values = flag_values(verify_stage["command"], "--receipt-dir")
            receipt_values = flag_values(verify_stage["command"], "--receipt")
            self.assertEqual(endpoint_values, sorted(endpoint_values))
            self.assertEqual(receipt_dir_values, sorted(receipt_dir_values))
            self.assertEqual(receipt_values, sorted(receipt_values))

    def test_duplicate_runbook_paths_do_not_echo_raw_segments(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            repeated_receipt = "receipts/private-corridor.receipt.json"
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "dry_run": True,
                    },
                    "verify": {
                        "include_stage_receipts": False,
                        "receipts": [repeated_receipt, repeated_receipt],
                    },
                },
            )

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("verify.receipts[1] duplicates verify.receipts[0]", stderr)
            self.assertNotIn(repeated_receipt, stderr)

    def test_runbook_artifact_paths_reject_secret_material_without_echo(self):
        base = {
            "provider": "local-bank",
            "environment": "ci",
            "rail": {
                "inbox_dir": "inbox",
                "torii_base_url": "https://torii.local-bank.bank",
            },
        }
        cases = (
            (
                "rail inbox",
                lambda body: body["rail"].__setitem__(
                    "inbox_dir",
                    "token=canary-runbook-path-secret/inbox",
                ),
                "rail.inbox_dir must not contain secret-looking material",
            ),
            (
                "rail message",
                lambda body: body["rail"].__setitem__(
                    "message",
                    "messages/token-canary-runbook-path-secret.xml",
                ),
                "rail.message must not contain secret-looking material",
            ),
            (
                "rail receipt dir",
                lambda body: body["rail"].__setitem__(
                    "receipt_dir",
                    "receipts/token-canary-runbook-path-secret",
                ),
                "rail.receipt_dir must not contain secret-looking material",
            ),
            (
                "notary export",
                lambda body: body.__setitem__(
                    "notary",
                    {
                        "export_dir": "token-canary-runbook-path-secret/export",
                        "dry_run": True,
                    },
                ),
                "notary.export_dir must not contain secret-looking material",
            ),
            (
                "verify receipt dir",
                lambda body: body.__setitem__(
                    "verify",
                    {"receipt_dirs": ["receipts/token-canary-runbook-path-secret"]},
                ),
                "verify.receipt_dirs[0] must not contain secret-looking material",
            ),
            (
                "verify receipt",
                lambda body: body.__setitem__(
                    "verify",
                    {"receipts": ["receipts/token-canary-runbook-path-secret.receipt.json"]},
                ),
                "verify.receipts[0] must not contain secret-looking material",
            ),
        )
        for name, mutate, expected in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    body = copy.deepcopy(base)
                    mutate(body)
                    config = write_config(root, body)

                    rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn("canary-runbook-path-secret", stderr)

    def test_relative_paths_cannot_escape_runbook_directory(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "../outside",
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                },
            )

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("must not contain dot or parent segments", stderr)

    def test_relative_symlinked_runbook_paths_do_not_echo_config_root(self):
        with tempfile.TemporaryDirectory() as raw_root, tempfile.TemporaryDirectory() as raw_outside:
            root = Path(raw_root)
            outside = Path(raw_outside) / "outside-canary-root"
            outside.mkdir()
            link = root / "runbook-link"
            try:
                link.symlink_to(outside, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "runbook-link/inbox",
                        "torii_base_url": "https://torii.local-bank.bank",
                    },
                },
            )

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn(
                "rail.inbox_dir relative paths must stay under config directory",
                stderr,
            )
            self.assertNotIn(str(root), stderr)
            self.assertNotIn(str(outside), stderr)

    def test_runbook_paths_reject_smuggled_segments_before_planning(self):
        base = {
            "provider": "local-bank",
            "environment": "ci",
            "rail": {
                "inbox_dir": "inbox",
                "torii_base_url": "https://torii.local-bank.bank",
            },
        }
        cases = [
            (
                "rail inbox whitespace",
                lambda body: body["rail"].__setitem__("inbox_dir", "in box"),
                "rail.inbox_dir must not contain whitespace",
            ),
            (
                "rail message backslash",
                lambda body: body["rail"].__setitem__("message", r"messages\status.xml"),
                "rail.message must use forward slashes",
            ),
            (
                "rail message leading dash segment",
                lambda body: body["rail"].__setitem__("message", "messages/--status.xml"),
                "rail.message must not contain leading-dash path segments",
            ),
            (
                "rail receipt semicolon",
                lambda body: body["rail"].__setitem__("receipt_dir", "receipts;v=1"),
                "rail.receipt_dir must not contain semicolon path parameters",
            ),
            (
                "rail receipt control",
                lambda body: body["rail"].__setitem__("receipt_dir", "receipts\x1bprod"),
                "rail.receipt_dir must not contain control characters",
            ),
            (
                "rail token empty segment",
                lambda body: body["rail"].__setitem__(
                    "bearer_token_file",
                    "secrets//torii.bearer",
                ),
                "rail.bearer_token_file must not contain empty path segments",
            ),
            (
                "notary export dot segment",
                lambda body: body.__setitem__(
                    "notary",
                    {
                        "export_dir": "./export",
                        "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                    },
                ),
                "notary.export_dir must not contain dot or parent segments",
            ),
            (
                "notary receipt parent segment",
                lambda body: body.__setitem__(
                    "notary",
                    {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                        "receipt_dir": "export/../receipts",
                    },
                ),
                "notary.receipt_dir must not contain dot or parent segments",
            ),
            (
                "verify receipt dir dot segment",
                lambda body: body.__setitem__(
                    "verify",
                    {"receipt_dirs": ["receipts/./stage"]},
                ),
                "verify.receipt_dirs[0] must not contain dot or parent segments",
            ),
            (
                "verify receipt whitespace",
                lambda body: body.__setitem__(
                    "verify",
                    {"receipts": ["receipts/a receipt.json"]},
                ),
                "verify.receipts[0] must not contain whitespace",
            ),
        ]
        for label, mutate, message in cases:
            with self.subTest(label=label):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    body = copy.deepcopy(base)
                    mutate(body)
                    config = write_config(root, body)

                    rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_runbook_paths_reject_non_ascii_without_echo(self):
        hidden = "inb\u043ex"
        body = {
            "provider": "local-bank",
            "environment": "ci",
            "rail": {
                "inbox_dir": hidden,
                "torii_base_url": "https://torii.local-bank.bank",
            },
        }
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = write_config(root, body)

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("rail.inbox_dir must use printable ASCII", stderr)
            self.assertNotIn(hidden, stderr)

    def test_control_characters_in_runbook_strings_are_rejected(self):
        cases = (
            ("newline", "local\nbank", None),
            ("format-control", "local\u202ebank", "\u202e"),
        )
        for name, provider, control in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    config = write_config(
                        root,
                        {
                            "provider": provider,
                            "environment": "ci",
                            "rail": {
                                "inbox_dir": "inbox",
                                "torii_base_url": "https://torii.local-bank.bank",
                            },
                        },
                    )

                    rc, _stdout, stderr = run_canary(
                        ["--config", str(config), "--plan-only"]
                    )

                self.assertEqual(rc, 2)
                self.assertIn("control characters", stderr)
                self.assertNotIn(provider, stderr)
                if control is not None:
                    self.assertNotIn(control, stderr)

    def test_runbook_context_strings_must_be_printable_ascii_without_echo(self):
        cases = (
            ("provider", "local-b\u00e1nk", "config.provider must use printable ASCII"),
            (
                "environment",
                "prepr\u043ed",
                "config.environment must use printable ASCII",
            ),
        )
        for field, hidden, message in cases:
            with self.subTest(field=field):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    body = {
                        "provider": "local-bank",
                        "environment": "ci",
                        "rail": {
                            "inbox_dir": "inbox",
                            "torii_base_url": "https://torii.local-bank.bank",
                        },
                    }
                    body[field] = hidden
                    config = write_config(root, body)

                    rc, _stdout, stderr = run_canary(
                        ["--config", str(config), "--plan-only"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_runbook_strings_must_not_require_trimming(self):
        base = {
            "provider": "local-bank",
            "environment": "ci",
            "rail": {
                "inbox_dir": "inbox",
                "torii_base_url": "https://torii.local-bank.bank",
            },
        }
        cases = [
            (
                "provider",
                lambda body: body.__setitem__("provider", "local-bank "),
                "config.provider must not have surrounding whitespace",
            ),
            (
                "environment",
                lambda body: body.__setitem__("environment", " ci"),
                "config.environment must not have surrounding whitespace",
            ),
            (
                "rail inbox",
                lambda body: body["rail"].__setitem__("inbox_dir", " inbox"),
                "rail.inbox_dir must not have surrounding whitespace",
            ),
            (
                "rail URL",
                lambda body: body["rail"].__setitem__(
                    "torii_base_url",
                    "https://torii.local-bank.bank ",
                ),
                "rail.torii_base_url must not have surrounding whitespace",
            ),
            (
                "rail token path",
                lambda body: body["rail"].__setitem__(
                    "bearer_token_file",
                    " secrets/torii.bearer",
                ),
                "rail.bearer_token_file must not have surrounding whitespace",
            ),
            (
                "notary endpoint",
                lambda body: body.__setitem__(
                    "notary",
                    {
                        "export_dir": "export",
                        "endpoints": [" https://notary.local-bank.bank/iso-anchor"],
                    },
                ),
                "notary.endpoints[0] must not have surrounding whitespace",
            ),
            (
                "verify receipt",
                lambda body: body.__setitem__(
                    "verify",
                    {"receipts": [" receipts/a.receipt.json"]},
                ),
                "verify.receipts[0] must not have surrounding whitespace",
            ),
            (
                "verify receipt dir",
                lambda body: body.__setitem__(
                    "verify",
                    {"receipt_dirs": ["receipts "]},
                ),
                "verify.receipt_dirs[0] must not have surrounding whitespace",
            ),
        ]
        for label, mutate, message in cases:
            with self.subTest(label=label):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    body = copy.deepcopy(base)
                    mutate(body)
                    config = write_config(root, body)

                    rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_optional_runbook_scalars_must_not_be_null_when_present(self):
        base = {
            "provider": "local-bank",
            "environment": "ci",
            "rail": {
                "inbox_dir": "inbox",
                "torii_base_url": "https://torii.local-bank.bank",
            },
        }
        cases = [
            (
                "rail message null",
                lambda body: body["rail"].__setitem__("message", None),
                "rail.message must be a non-empty string when provided",
            ),
            (
                "rail receipt dir null",
                lambda body: body["rail"].__setitem__("receipt_dir", None),
                "rail.receipt_dir must be a non-empty string when provided",
            ),
            (
                "rail token path null",
                lambda body: body["rail"].__setitem__("bearer_token_file", None),
                "rail.bearer_token_file must be a non-empty string when provided",
            ),
            (
                "rail max payload null",
                lambda body: body["rail"].__setitem__("max_payload_bytes", None),
                "rail.max_payload_bytes must be a positive integer",
            ),
            (
                "rail timeout null",
                lambda body: body["rail"].__setitem__("timeout_secs", None),
                "rail.timeout_secs must be a positive number",
            ),
            (
                "rail response limit null",
                lambda body: body["rail"].__setitem__("response_limit_bytes", None),
                "rail.response_limit_bytes must be a positive integer",
            ),
            (
                "notary receipt dir null",
                lambda body: body.__setitem__(
                    "notary",
                    {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                        "receipt_dir": None,
                    },
                ),
                "notary.receipt_dir must be a non-empty string when provided",
            ),
            (
                "notary token path null",
                lambda body: body.__setitem__(
                    "notary",
                    {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                        "bearer_token_file": None,
                    },
                ),
                "notary.bearer_token_file must be a non-empty string when provided",
            ),
            (
                "notary timeout null",
                lambda body: body.__setitem__(
                    "notary",
                    {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                        "timeout_secs": None,
                    },
                ),
                "notary.timeout_secs must be a positive number",
            ),
            (
                "notary response limit null",
                lambda body: body.__setitem__(
                    "notary",
                    {
                        "export_dir": "export",
                        "endpoints": ["https://notary.local-bank.bank/iso-anchor"],
                        "response_limit_bytes": None,
                    },
                ),
                "notary.response_limit_bytes must be a positive integer",
            ),
        ]
        for label, mutate, message in cases:
            with self.subTest(label=label):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    body = copy.deepcopy(base)
                    mutate(body)
                    config = write_config(root, body)

                    rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_rail_sidecar_failure_stops_before_network_and_skips_verify(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            inbox.mkdir()
            xml_path, _sidecar = rail_test.write_message(inbox)
            xml_path.write_bytes(rail_test.SAMPLE_XML + b"tampered")
            with rail_test.capture_server() as (torii_url, requests):
                config = write_config(
                    root,
                    {
                        "provider": "local-bank",
                        "environment": "ci",
                        "rail": {
                            "inbox_dir": str(inbox),
                            "torii_base_url": torii_url,
                            "allow_insecure_http": True,
                        },
                        "verify": {
                            "allow_insecure_http": True,
                            "require_source_files": True,
                        },
                    },
                )
                rc, stdout, stderr = run_canary(["--config", str(config)])

            self.assertEqual(rc, 1, stderr)
            self.assertEqual(requests, [])
            summary = load_summary(stdout)
            self.assertFalse(summary["ok"])
            self.assertEqual(summary["stages"][0]["name"], "rail")
            self.assertEqual(summary["stages"][0]["returncode"], 2)
            self.assertTrue(summary["stages"][1]["skipped"])

    def test_verifier_failure_marks_canary_failed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            inbox.mkdir()
            xml_path, _sidecar = rail_test.write_message(inbox)
            with rail_test.capture_server() as (torii_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            torii_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            xml_path.write_bytes(rail_test.SAMPLE_XML + b" changed")

            export_dir = root / "export"
            export_dir.mkdir()
            audit_test.write_export(export_dir)
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": str(export_dir),
                        "dry_run": True,
                    },
                    "verify": {
                        "include_stage_receipts": False,
                        "receipts": [str(receipt)],
                        "allow_insecure_http": True,
                        "require_source_files": True,
                    },
                },
            )

            rc, stdout, stderr = run_canary(["--config", str(config)])

            self.assertEqual(rc, 1, stderr)
            summary = load_summary(stdout)
            self.assertFalse(summary["ok"])
            self.assertEqual(summary["stages"][0]["returncode"], 0)
            self.assertEqual(summary["stages"][1]["name"], "verify")
            self.assertEqual(summary["stages"][1]["returncode"], 2)
            self.assertIn("payload_sha256 does not match source XML", summary["stages"][1]["stderr_preview"])

    def test_config_without_rail_or_notary_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = write_config(
                root,
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "verify": {"enabled": False},
                },
            )

            rc, _stdout, stderr = run_canary(["--config", str(config)])

            self.assertEqual(rc, 2)
            self.assertIn("at least one of rail or notary", stderr)


if __name__ == "__main__":
    unittest.main()
