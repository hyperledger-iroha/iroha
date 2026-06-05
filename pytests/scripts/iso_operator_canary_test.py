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
                        "torii_base_url": "https://torii.example.invalid",
                        "bearer_token_file": "secrets/torii.bearer",
                    },
                    "notary": {
                        "export_dir": "missing-export",
                        "endpoints": ["https://notary.example.invalid/iso-anchor"],
                        "bearer_token_file": "secrets/notary.bearer",
                    },
                },
            )

            rc, stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 0, stderr)
            self.assertFalse((root / "missing-inbox").exists())
            self.assertFalse((root / "missing-export").exists())
            summary = load_summary(stdout)
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
                self.assertTrue(summary["ok"])
                self.assertTrue(summary["plan_only"])
                self.assertTrue(summary["policy"]["require_explicit_policy"])
                self.assertEqual(
                    [stage["name"] for stage in summary["planned_stages"]],
                    ["rail", "notary", "verify"],
                )

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
                        "torii_base_url": "https://torii.example.invalid",
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

    def test_directory_config_is_rejected_before_plan(self):
        with tempfile.TemporaryDirectory() as raw_root:
            config = Path(raw_root) / "config-dir"
            config.mkdir()

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("must be a regular file", stderr)

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
                        "torii_base_url": "https://torii.example.invalid",
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

    def test_require_explicit_policy_rejects_omitted_policy_booleans(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            base = {
                "provider": "local-bank",
                "environment": "ci",
                "rail": {
                    "inbox_dir": "inbox",
                    "torii_base_url": "https://torii.example.invalid",
                    "dry_run": False,
                    "allow_default_profile": False,
                    "allow_insecure_http": False,
                },
                "notary": {
                    "export_dir": "audit-export",
                    "endpoints": ["https://notary.example.invalid/iso-anchor"],
                    "all": False,
                    "dry_run": False,
                    "allow_insecure_http": False,
                },
                "verify": {
                    "enabled": True,
                    "include_stage_receipts": True,
                    "skip_on_stage_failure": True,
                    "allow_failed": False,
                    "allow_insecure_http": False,
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
                    '{"provider":"local-bank","provider":"other-bank",'
                    '"environment":"ci",'
                    '"rail":{"inbox_dir":"inbox",'
                    '"torii_base_url":"https://torii.example.invalid"}}\n'
                ),
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)

    def test_endpoint_urls_are_validated_before_planning(self):
        cases = [
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://user:pass@torii.example.invalid",
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
                        "torii_base_url": "https://torii.example.invalid?token=abc",
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
                        "torii_base_url": "https://torii.example.invalid/iso bridge",
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
                        "torii_base_url": "https://torii.example.invalid:abc",
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
                        "torii_base_url": "https://torii.example.invalid:443",
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
                        "torii_base_url": "https://torii.example.invalid.",
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
                        "torii_base_url": "https://-torii.example.invalid",
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
                        "torii_base_url": "https://torii.example.invalid/../base",
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
                        "torii_base_url": "https://torii.example.invalid/base%2fv1",
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
                        "torii_base_url": "https://torii.example.invalid/base%252fv1",
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
                        "torii_base_url": "https://torii.example.invalid/base%20v1",
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
                        "torii_base_url": "https://torii.example.invalid/base%00v1",
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
                        "torii_base_url": "https://torii.example.invalid/base%zzv1",
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
                        "torii_base_url": "https://torii.example.invalid/base;debug/v1",
                    },
                },
                "path must not contain semicolon parameters",
            ),
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": ["https://notary.example.invalid/iso-anchor#frag"],
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
                        "endpoints": ["https://notary.example.invalid/iso anchor"],
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
                        "endpoints": ["https://notary.example.invalid:99999/iso-anchor"],
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
                        "endpoints": ["https://notary.example.invalid:443/iso-anchor"],
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
                        "endpoints": ["https://notary.example.invalid./iso-anchor"],
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
                        "endpoints": ["https://notary.example.invalid/../iso-anchor"],
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
                        "endpoints": ["https://notary.example.invalid/iso%2fanchor"],
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
                        "endpoints": ["https://notary.example.invalid/iso%252fanchor"],
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
                        "endpoints": ["https://notary.example.invalid/iso%20anchor"],
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
                        "endpoints": ["https://notary.example.invalid/iso%zzanchor"],
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
                        "endpoints": [r"https://notary.example.invalid/iso\anchor"],
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
                        "torii_base_url": "http://torii.example.invalid",
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

    def test_duplicate_runbook_evidence_inputs_are_rejected_before_planning(self):
        cases = [
            (
                {
                    "provider": "local-bank",
                    "environment": "ci",
                    "notary": {
                        "export_dir": "export",
                        "endpoints": [
                            "https://notary.example.invalid/iso-anchor",
                            "https://notary.example.invalid/iso-anchor",
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
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.example.invalid",
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
                        "torii_base_url": "https://torii.example.invalid",
                    },
                },
            )

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("relative paths must stay under", stderr)

    def test_control_characters_in_runbook_strings_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            config = write_config(
                root,
                {
                    "provider": "local\nbank",
                    "environment": "ci",
                    "rail": {
                        "inbox_dir": "inbox",
                        "torii_base_url": "https://torii.example.invalid",
                    },
                },
            )

            rc, _stdout, stderr = run_canary(["--config", str(config), "--plan-only"])

            self.assertEqual(rc, 2)
            self.assertIn("control characters", stderr)

    def test_runbook_strings_must_not_require_trimming(self):
        base = {
            "provider": "local-bank",
            "environment": "ci",
            "rail": {
                "inbox_dir": "inbox",
                "torii_base_url": "https://torii.example.invalid",
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
                    "https://torii.example.invalid ",
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
                        "endpoints": [" https://notary.example.invalid/iso-anchor"],
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
