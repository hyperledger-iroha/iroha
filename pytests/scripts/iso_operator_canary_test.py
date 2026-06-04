import contextlib
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
            audit_test.write_export(export_dir)
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
            self.assertEqual([stage["name"] for stage in summary["stages"]], ["rail", "notary", "verify"])
            self.assertEqual([stage["returncode"] for stage in summary["stages"]], [0, 0, 0])
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
                rc, stdout, stderr = run_canary(["--config", str(template), "--plan-only"])
                self.assertEqual(rc, 0, stderr)
                summary = load_summary(stdout)
                self.assertTrue(summary["ok"])
                self.assertTrue(summary["plan_only"])
                self.assertEqual(
                    [stage["name"] for stage in summary["planned_stages"]],
                    ["rail", "notary", "verify"],
                )

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
