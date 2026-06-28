import argparse
import contextlib
import io
import sys
from pathlib import Path
import unittest

from pytests.scripts import iso_audit_notary_adapter_test as audit_test
from pytests.scripts import iso_operator_canary_test as canary_test
from pytests.scripts import iso_operator_evidence_verify_test as evidence_test
from pytests.scripts import iso_operator_receipt_verify_test as receipt_test
from pytests.scripts import iso_pending_xsd_source_probe_test as probe_test
from pytests.scripts import iso_production_readiness_test as readiness_test
from pytests.scripts import iso_rail_gateway_adapter_test as rail_test
from pytests.scripts import iso_trust_bundle_verify_test as trust_test
from pytests.scripts import iso_xsd_fixture_verify_test as xsd_test


TOOLS = (
    ("pending-probe", probe_test.PROBE, "--summary-out", "--summary-out requires a path value"),
    ("audit-notary", audit_test.ADAPTER, "--receipt-dir", "--receipt-dir requires a path value"),
    ("rail-gateway", rail_test.ADAPTER, "--receipt-dir", "--receipt-dir requires a path value"),
    ("receipt-verifier", receipt_test.VERIFIER, "--receipt", "--receipt requires a path value"),
    ("operator-canary", canary_test.CANARY, "--summary-out", "--summary-out requires a path value"),
    ("trust-bundle", trust_test.VERIFIER, "--summary-out", "--summary-out requires a path value"),
    ("operator-evidence", evidence_test.EVIDENCE, "--summary-out", "--summary-out requires a path value"),
    ("xsd-fixture", xsd_test.VERIFIER, "--summary-out", "--summary-out requires a path value"),
    ("production-readiness", readiness_test.READINESS, "--summary-out", "--summary-out requires a path value"),
)


def run_main_with_sys_argv(module, argv):
    old_argv = sys.argv
    stdout = io.StringIO()
    stderr = io.StringIO()
    try:
        sys.argv = argv
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            rc = module.main(None)
    finally:
        sys.argv = old_argv
    return rc, stdout.getvalue(), stderr.getvalue()


class IsoCliArgvNormalizationTest(unittest.TestCase):
    def test_sys_argv_inputs_are_normalized_before_preflight(self):
        hidden = "token=sys-argv-secret"

        class HostileArgv(list):
            def __len__(self):
                raise RuntimeError(f"len={hidden}")

            def __iter__(self):
                raise RuntimeError(f"iter={hidden}")

            def __getitem__(self, _key):
                raise RuntimeError(f"item={hidden}")

        class HostileText(str):
            def __str__(self):
                raise RuntimeError(f"str={hidden}")

            def startswith(self, _prefix, *_args):
                raise RuntimeError(f"startswith={hidden}")

            def strip(self, *_args):
                raise RuntimeError(f"strip={hidden}")

        for name, module, flag, missing_value_error in TOOLS:
            cases = (
                (
                    "container",
                    HostileArgv(["iso-tool", flag]),
                    "sys.argv must be a plain argument list",
                ),
                ("tuple", ("iso-tool", flag), "sys.argv must be a plain argument list"),
                ("non-string", ["iso-tool", object()], "argv[0] must be a string"),
                ("hostile-program-name", [HostileText("iso-tool"), flag], missing_value_error),
                ("hostile-string", ["iso-tool", HostileText(flag)], missing_value_error),
            )
            for case_name, argv, expected in cases:
                with self.subTest(tool=name, case=case_name):
                    rc, stdout, stderr = run_main_with_sys_argv(module, argv)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(expected, stderr)
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn("sys-argv-secret", stderr)

    def test_run_args_require_plain_argparse_namespace_before_attribute_access(self):
        hidden = "token=run-namespace-secret"

        class HostileNamespace(argparse.Namespace):
            def __getattribute__(self, _name):
                raise RuntimeError(f"get={hidden}")

            def __setattr__(self, _name, _value):
                raise RuntimeError(f"set={hidden}")

        for name, module, _flag, _missing_value_error in TOOLS:
            cases = (
                ("object", object()),
                ("subclass", HostileNamespace()),
            )
            for case_name, args in cases:
                with self.subTest(tool=name, case=case_name):
                    with self.assertRaises(Exception) as caught:
                        module.run(args)

                    message = str(caught.exception)
                    self.assertIn("args must be an argparse.Namespace", message)
                    self.assertNotIn(hidden, message)
                    self.assertNotIn("run-namespace-secret", message)

    def test_numeric_helpers_reject_hostile_scalar_subclasses_without_echo(self):
        hidden = "token=iso-numeric-helper-secret"

        class HostileInt(int):
            def __new__(cls, value):
                return int.__new__(cls, value)

            def __int__(self):
                raise RuntimeError(f"int={hidden}")

            def __float__(self):
                raise RuntimeError(f"float={hidden}")

            def __le__(self, _other):
                raise RuntimeError(f"le={hidden}")

            def __lt__(self, _other):
                raise RuntimeError(f"lt={hidden}")

            def __gt__(self, _other):
                raise RuntimeError(f"gt={hidden}")

            def __ge__(self, _other):
                raise RuntimeError(f"ge={hidden}")

        class HostileFloat(float):
            def __new__(cls, value):
                return float.__new__(cls, value)

            def __float__(self):
                raise RuntimeError(f"float={hidden}")

            def __le__(self, _other):
                raise RuntimeError(f"le={hidden}")

            def __gt__(self, _other):
                raise RuntimeError(f"gt={hidden}")

        class HostilePath(type(Path("hostile"))):
            def __fspath__(self):
                raise RuntimeError(f"fspath={hidden}")

            def __str__(self):
                raise RuntimeError(f"str={hidden}")

        class HostileStr(str):
            def __new__(cls, value):
                return str.__new__(cls, value)

            def __str__(self):
                raise RuntimeError(f"str={hidden}")

            def __hash__(self):
                raise RuntimeError(f"hash={hidden}")

            def __eq__(self, _other):
                raise RuntimeError(f"eq={hidden}")

            def startswith(self, _prefix):
                raise RuntimeError(f"startswith={hidden}")

        class HostileDict(dict):
            def __getitem__(self, _key):
                raise RuntimeError(f"getitem={hidden}")

            def get(self, _key, _default=None):
                raise RuntimeError(f"get={hidden}")

            def items(self):
                raise RuntimeError(f"items={hidden}")

            def __iter__(self):
                raise RuntimeError(f"iter={hidden}")

            def __len__(self):
                raise RuntimeError(f"len={hidden}")

        self.assertIsNone(rail_test.ADAPTER._parse_http_status_code(HostileInt(200)))
        self.assertIsNone(audit_test.ADAPTER._parse_http_status_code(HostileInt(200)))
        self.assertIsNone(probe_test.PROBE._http_status_code(HostileInt(200)))

        non_exception_cases = (
            (
                "production-readiness-version-output",
                lambda: readiness_test.READINESS._version_for_readiness_output(
                    HostileInt(1),
                    1,
                ),
                "unsupported",
            ),
            (
                "production-readiness-freshness-budget",
                lambda: readiness_test.READINESS._freshness_budget_for_readiness_output(
                    HostileInt(1),
                    30,
                ),
                "unsupported",
            ),
            (
                "production-readiness-probe-public-status",
                lambda: readiness_test.READINESS._pending_xsd_probe_response_metadata_is_publicly_supported(
                    {
                        "status": "reachable",
                        "looks_like_xsd": True,
                        "http_status": HostileInt(200),
                        "downloaded_bytes": 64,
                        "sample_sha256": "1" * 64,
                    }
                ),
                False,
            ),
            (
                "production-readiness-probe-public-bytes",
                lambda: readiness_test.READINESS._pending_xsd_probe_response_metadata_is_publicly_supported(
                    {
                        "status": "reachable",
                        "looks_like_xsd": True,
                        "http_status": 200,
                        "downloaded_bytes": HostileInt(64),
                        "sample_sha256": "1" * 64,
                    }
                ),
                False,
            ),
            (
                "production-readiness-receipt-public-status",
                lambda: readiness_test.READINESS._receipt_response_metadata_is_publicly_supported(
                    {
                        "ok": True,
                        "status_code": HostileInt(200),
                        "response_body_sha256": "1" * 64,
                    }
                ),
                False,
            ),
            (
                "xsd-summary-material-paths-hostile-dict",
                lambda: xsd_test.VERIFIER._summary_material_input_paths(
                    Path("fixture_manifest.json"),
                    {
                        "schemas": [HostileDict({"path": "xsd/pacs.008.xsd"})],
                        "fixtures": [HostileDict({"path": "xml/pacs.008.xml"})],
                    },
                ),
                (),
            ),
            (
                "xsd-summary-material-paths-hostile-str",
                lambda: xsd_test.VERIFIER._summary_material_input_paths(
                    Path("fixture_manifest.json"),
                    {
                        "schemas": [{"path": HostileStr("xsd/pacs.008.xsd")}],
                        "fixtures": [{"path": HostileStr("xml/pacs.008.xml")}],
                    },
                ),
                (),
            ),
            (
                "operator-evidence-compact-receipts-hostile-dict",
                lambda: evidence_test.EVIDENCE._compact_receipt_summaries(
                    [{"receipt_summary": HostileDict({"receipts": []})}],
                    None,
                ),
                [],
            ),
            (
                "operator-evidence-artifact-reuse-hostile-dict",
                lambda: evidence_test.EVIDENCE._reject_compact_json_artifact_path_role_reuse(
                    [
                        {
                            "config_path": "run/canary-config.json",
                            "path": "run/canary.summary.json",
                            "receipt_summary": HostileDict({"receipts": []}),
                        }
                    ],
                    [],
                    receipt_summary=HostileDict({"receipts": []}),
                ),
                None,
            ),
            (
                "operator-evidence-artifact-reuse-hostile-str",
                lambda: evidence_test.EVIDENCE._reject_compact_json_artifact_path_role_reuse(
                    [
                        {
                            "config_path": "run/canary-config.json",
                            "path": "run/canary.summary.json",
                            "receipt_summary": {
                                "receipts": [{"path": HostileStr("run/receipt.json")}],
                            },
                        }
                    ],
                    [],
                    receipt_summary={
                        "receipts": [{"path": HostileStr("run/direct-receipt.json")}],
                    },
                ),
                None,
            ),
            (
                "production-readiness-artifact-reuse-hostile-dict",
                lambda: readiness_test.READINESS._block_compact_json_artifact_path_role_reuse(
                    [
                        {
                            "config_path": "run/canary-config.json",
                            "path": "run/canary.summary.json",
                            "receipt_summary": HostileDict({"receipts": []}),
                        }
                    ],
                    [],
                    Path("run/evidence.summary.json"),
                    [],
                    archive_receipts=HostileDict({"receipts": []}),
                ),
                None,
            ),
            (
                "production-readiness-artifact-reuse-hostile-str",
                lambda: readiness_test.READINESS._block_compact_json_artifact_path_role_reuse(
                    [
                        {
                            "config_path": "run/canary-config.json",
                            "path": "run/canary.summary.json",
                            "receipt_summary": {
                                "receipts": [{"path": HostileStr("run/receipt.json")}],
                            },
                        }
                    ],
                    [],
                    Path("run/evidence.summary.json"),
                    [],
                    archive_receipts={
                        "receipts": [{"path": HostileStr("run/direct-receipt.json")}],
                    },
                ),
                None,
            ),
            (
                "production-readiness-receipt-path-roles-hostile-str",
                lambda: readiness_test.READINESS._receipt_path_material_roles(
                    [
                        {
                            "path": HostileStr("run/receipt.json"),
                            "source_path": HostileStr("run/source.xml"),
                        }
                    ],
                    "receipt_summary",
                ),
                [],
            ),
            (
                "production-readiness-xsd-gap-diagnostics-hostile-dict",
                lambda: readiness_test.READINESS._xsd_gap_diagnostic_entries(
                    [HostileDict({"path": "xsd/pacs.008.xsd"})],
                ),
                [],
            ),
        )
        for name, call, expected in non_exception_cases:
            with self.subTest(name=name):
                try:
                    result = call()
                except Exception as error:
                    message = str(error)
                    self.assertNotIn(hidden, message)
                    self.assertNotIn("iso-numeric-helper-secret", message)
                    self.fail(f"{name} raised {type(error).__name__}")
                self.assertEqual(result, expected)

        cases = (
            (
                "pending-probe-int",
                lambda: probe_test.PROBE._positive_int(HostileInt(7), "--max-bytes"),
                "--max-bytes must be a positive integer",
            ),
            (
                "pending-probe-float",
                lambda: probe_test.PROBE._positive_float(
                    HostileFloat(1.0),
                    "--timeout-secs",
                ),
                "--timeout-secs must be a positive finite number",
            ),
            (
                "audit-notary-int",
                lambda: audit_test.ADAPTER._require_positive_cli_int(
                    HostileInt(7),
                    "--response-limit-bytes",
                ),
                "--response-limit-bytes must be a positive integer",
            ),
            (
                "audit-notary-float",
                lambda: audit_test.ADAPTER._require_positive_finite_cli_number(
                    HostileFloat(1.0),
                    "--timeout-secs",
                ),
                "--timeout-secs must be a positive finite number",
            ),
            (
                "audit-notary-optional-path",
                lambda: audit_test.ADAPTER._optional_cli_path(
                    HostilePath("anchor.json"),
                    "--anchor",
                ),
                "--anchor must be a path",
            ),
            (
                "rail-gateway-int",
                lambda: rail_test.ADAPTER._require_positive_cli_int(
                    HostileInt(7),
                    "--response-limit-bytes",
                ),
                "--response-limit-bytes must be a positive integer",
            ),
            (
                "rail-gateway-float",
                lambda: rail_test.ADAPTER._require_positive_finite_cli_number(
                    HostileFloat(1.0),
                    "--timeout-secs",
                ),
                "--timeout-secs must be a positive finite number",
            ),
            (
                "rail-gateway-optional-path",
                lambda: rail_test.ADAPTER._optional_cli_path(
                    HostilePath("message.xml"),
                    "--message",
                ),
                "--message must be a path",
            ),
            (
                "rail-gateway-message-path",
                lambda: rail_test.ADAPTER._normalise_message_argument(
                    HostilePath("message.xml"),
                ),
                "message must be a path",
            ),
            (
                "rail-gateway-read-file-max-bytes",
                lambda: rail_test.ADAPTER._read_regular_file(
                    Path("missing.xml"),
                    max_bytes=HostileInt(1),
                    path_label="message",
                ),
                "max file bytes must be a positive integer",
            ),
            (
                "rail-gateway-bounded-read-max-bytes",
                lambda: rail_test.ADAPTER._bounded_read(
                    Path("missing.xml"),
                    HostileInt(1),
                    path_label="message",
                ),
                "max payload bytes must be a positive integer",
            ),
            (
                "receipt-nonnegative",
                lambda: receipt_test.VERIFIER._require_nonnegative_int(
                    HostileInt(0),
                    "receipt.updated_at_ms",
                ),
                "receipt.updated_at_ms must be a non-negative integer",
            ),
            (
                "receipt-status-code",
                lambda: receipt_test.VERIFIER._check_status(
                    {"ok": True, "status_code": HostileInt(200)},
                    "receipt",
                    allow_failed=False,
                ),
                "receipt status_code must be null or an HTTP status integer",
            ),
            (
                "receipt-read-file-max-bytes",
                lambda: receipt_test.VERIFIER._read_regular_file(
                    Path("missing.json"),
                    max_bytes=HostileInt(1),
                    display_label="receipt",
                ),
                "max file bytes must be a positive integer",
            ),
            (
                "receipt-path-list-entry",
                lambda: receipt_test.VERIFIER._required_cli_path_sequence(
                    [HostilePath("receipt.json")],
                    "--receipt",
                ),
                "--receipt[0] must be a path",
            ),
            (
                "audit-notary-nonnegative",
                lambda: audit_test.ADAPTER._require_nonnegative_int(
                    HostileInt(0),
                    "record.updated_at_ms",
                ),
                "record.updated_at_ms must be a non-negative integer",
            ),
            (
                "audit-notary-read-file-max-bytes",
                lambda: audit_test.ADAPTER._read_regular_file(
                    Path("missing.json"),
                    max_bytes=HostileInt(1),
                    path_label="anchor",
                ),
                "max file bytes must be a positive integer",
            ),
            (
                "operator-canary-int",
                lambda: canary_test.CANARY._optional_positive_int(
                    {"max_payload_bytes": HostileInt(7)},
                    "max_payload_bytes",
                    "rail",
                ),
                "rail.max_payload_bytes must be a positive integer",
            ),
            (
                "operator-canary-number",
                lambda: canary_test.CANARY._optional_positive_number(
                    {"timeout_secs": HostileFloat(1.0)},
                    "timeout_secs",
                    "rail",
                ),
                "rail.timeout_secs must be a positive number",
            ),
            (
                "operator-canary-float",
                lambda: canary_test.CANARY._require_positive_finite_number(
                    HostileFloat(1.0),
                    "stage timeout seconds",
                ),
                "stage timeout seconds must be a positive finite number",
            ),
            (
                "operator-canary-optional-path",
                lambda: canary_test.CANARY._optional_cli_path(
                    HostilePath("config.json"),
                    "--config",
                ),
                "--config must be a path",
            ),
            (
                "operator-canary-read-file-max-bytes",
                lambda: canary_test.CANARY._read_regular_file(
                    Path("missing.json"),
                    max_bytes=HostileInt(1),
                    display_label="config",
                ),
                "max file bytes must be a positive integer",
            ),
            (
                "operator-canary-run-command-output-limit",
                lambda: canary_test.CANARY._run_command_bounded(
                    ["canary-child"],
                    HostileInt(1),
                    1.0,
                ),
                "output limit bytes must be positive",
            ),
            (
                "trust-bundle-int",
                lambda: trust_test.VERIFIER._optional_positive_cli_int(
                    HostileInt(7),
                    "--max-source-age-days",
                ),
                "--max-source-age-days must be a positive integer",
            ),
            (
                "trust-bundle-optional-path",
                lambda: trust_test.VERIFIER._optional_cli_path(
                    HostilePath("bundle.json"),
                    "--bundle",
                ),
                "--bundle must be a path",
            ),
            (
                "trust-bundle-path-list-entry",
                lambda: trust_test.VERIFIER._required_cli_path_sequence(
                    [HostilePath("bundle.json")],
                    "--bundle",
                ),
                "--bundle[0] must be a path",
            ),
            (
                "trust-bundle-read-file-max-bytes",
                lambda: trust_test.VERIFIER._read_regular_file(
                    Path("missing.json"),
                    max_bytes=HostileInt(1),
                    display_label="bundle",
                ),
                "max file bytes must be a positive integer",
            ),
            (
                "operator-evidence-int",
                lambda: evidence_test.EVIDENCE._required_positive_cli_int(
                    HostileInt(7),
                    "--max-trust-source-age-days",
                ),
                "--max-trust-source-age-days must be a positive integer",
            ),
            (
                "operator-evidence-float",
                lambda: evidence_test.EVIDENCE._required_positive_finite_cli_number(
                    HostileFloat(1.0),
                    "--receipt-verifier-timeout-secs",
                ),
                "--receipt-verifier-timeout-secs must be a positive finite number",
            ),
            (
                "operator-evidence-positive-field",
                lambda: evidence_test.EVIDENCE._required_positive_int_field(
                    {"record_count": HostileInt(1)},
                    "record_count",
                    "summary",
                ),
                "summary.record_count must be a positive integer",
            ),
            (
                "operator-evidence-nonnegative-field",
                lambda: evidence_test.EVIDENCE._required_nonnegative_int(
                    {"failures": HostileInt(0)},
                    "failures",
                    "summary",
                ),
                "summary.failures must be a non-negative integer",
            ),
            (
                "operator-evidence-optional-path",
                lambda: evidence_test.EVIDENCE._optional_cli_path(
                    HostilePath("summary.json"),
                    "--canary-summary",
                ),
                "--canary-summary must be a path",
            ),
            (
                "operator-evidence-path-list-entry",
                lambda: evidence_test.EVIDENCE._required_cli_path_sequence(
                    [HostilePath("summary.json")],
                    "--canary-summary",
                ),
                "--canary-summary[0] must be a path",
            ),
            (
                "operator-evidence-read-file-max-bytes",
                lambda: evidence_test.EVIDENCE._read_regular_file(
                    Path("missing.json"),
                    max_bytes=HostileInt(1),
                    display_label="summary",
                ),
                "max file bytes must be a positive integer",
            ),
            (
                "operator-evidence-run-command-output-limit",
                lambda: evidence_test.EVIDENCE._run_command_bounded(
                    ["receipt-verifier"],
                    HostileInt(1),
                    1.0,
                ),
                "output limit bytes must be positive",
            ),
            (
                "xsd-fixture-float",
                lambda: xsd_test.VERIFIER._require_positive_finite_number(
                    HostileFloat(1.0),
                    "xmllint timeout seconds",
                ),
                "xmllint timeout seconds must be a positive finite number",
            ),
            (
                "xsd-fixture-optional-path",
                lambda: xsd_test.VERIFIER._optional_cli_path(
                    HostilePath("manifest.json"),
                    "--manifest",
                ),
                "--manifest must be a path",
            ),
            (
                "xsd-fixture-nonnegative-field",
                lambda: xsd_test.VERIFIER._optional_nonnegative_int(
                    {"minor_units": HostileInt(0)},
                    "minor_units",
                    "profile.asset",
                ),
                "profile.asset.minor_units must be a non-negative integer when set",
            ),
            (
                "xsd-fixture-read-file-max-bytes",
                lambda: xsd_test.VERIFIER._read_regular_file(
                    Path("missing.json"),
                    max_bytes=HostileInt(1),
                    display_label="manifest",
                ),
                "max file bytes must be a positive integer",
            ),
            (
                "xsd-fixture-run-command-output-limit",
                lambda: xsd_test.VERIFIER._run_command_bounded(
                    ["xmllint"],
                    HostileInt(1),
                    1.0,
                ),
                "output limit bytes must be positive",
            ),
            (
                "production-readiness-int",
                lambda: readiness_test.READINESS._require_positive_cli_int(
                    HostileInt(7),
                    "--max-trust-source-age-days",
                ),
                "--max-trust-source-age-days must be a positive integer",
            ),
            (
                "production-readiness-positive-field",
                lambda: readiness_test.READINESS._require_positive_int(
                    {"verified_receipts": HostileInt(1)},
                    "verified_receipts",
                    "summary",
                ),
                "summary.verified_receipts must be a positive integer",
            ),
            (
                "production-readiness-nonnegative-field",
                lambda: readiness_test.READINESS._require_nonnegative_int(
                    {"schema_backed_fixtures": HostileInt(0)},
                    "schema_backed_fixtures",
                    "summary",
                ),
                "summary.schema_backed_fixtures must be a non-negative integer",
            ),
            (
                "production-readiness-number-field",
                lambda: readiness_test.READINESS._require_positive_number(
                    {"timeout_secs": HostileFloat(1.0)},
                    "timeout_secs",
                    "summary",
                ),
                "summary.timeout_secs must be a positive finite number",
            ),
            (
                "production-readiness-probe-status-code",
                lambda: readiness_test.READINESS._validate_probe_http_status(
                    HostileInt(200),
                    "probe.http_status",
                ),
                "probe.http_status must be an HTTP status code or null",
            ),
            (
                "production-readiness-optional-path",
                lambda: readiness_test.READINESS._optional_cli_path(
                    HostilePath("summary.json"),
                    "--xsd-summary",
                ),
                "--xsd-summary must be a path",
            ),
            (
                "production-readiness-path-list-entry",
                lambda: readiness_test.READINESS._required_cli_path_sequence(
                    [HostilePath("summary.json")],
                    "--xsd-summary",
                ),
                "--xsd-summary[0] must be a path",
            ),
            (
                "production-readiness-read-file-max-bytes",
                lambda: readiness_test.READINESS._read_regular_file(
                    Path("missing.json"),
                    max_bytes=HostileInt(1),
                    display_label="summary",
                ),
                "max file bytes must be a positive integer",
            ),
        )
        for name, call, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(Exception) as caught:
                    call()

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(hidden, message)
                self.assertNotIn("iso-numeric-helper-secret", message)


if __name__ == "__main__":
    unittest.main()
