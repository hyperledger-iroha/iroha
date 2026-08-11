"""Exact operator-auth contract tests for the ISO canary runner."""

import json
import tempfile
import unittest
from pathlib import Path

from pytests.scripts import iso_audit_notary_adapter_test as audit_test
from pytests.scripts import iso_rail_gateway_adapter_test as rail_test
from pytests.scripts.iso_operator_canary_test import (
    CANARY,
    TEST_NETWORK_ID,
    load_summary,
    run_canary,
    write_config,
)


class IsoOperatorCanaryAuthTest(unittest.TestCase):
    def test_runtime_secret_arguments_are_redacted(self):
        cases = (
            (
                "--bearer-token-file=/ops/secrets/live-token",
                "--bearer-token-file=<runtime-token-file>",
            ),
            (
                "--operator-private-key-file=/ops/secrets/operator-key",
                "--operator-private-key-file=<runtime-private-key-file>",
            ),
        )
        for supplied, expected in cases:
            with self.subTest(supplied=supplied):
                self.assertEqual(
                    CANARY._redacted_command(["adapter.py", supplied]),
                    ["adapter.py", expected],
                )

    def test_rail_requires_exact_network_and_operator_key(self):
        base = {
            "provider": "local-bank",
            "environment": "preprod",
            "rail": {
                "inbox_dir": "inbox",
                "torii_base_url": "https://torii.local-bank.bank",
                "network_id": TEST_NETWORK_ID,
                "operator_private_key_file": "runtime/operator.key",
            },
        }
        for field in ("network_id", "operator_private_key_file"):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as raw_root:
                body = json.loads(json.dumps(base))
                del body["rail"][field]
                path = Path(raw_root) / "canary.json"
                path.write_text(json.dumps(body), encoding="utf-8")
                rc, _stdout, stderr = run_canary(["--config", str(path), "--plan-only"])
                self.assertEqual(rc, 2)
                self.assertIn(f"rail.{field} must be a non-empty string", stderr)

    def test_rail_rejects_wrong_checksum_and_forged_marker_bit(self):
        even_prefix = "hash:" + ("08" * 32)
        forged_marker = (
            f"{even_prefix}#{CANARY._crc16_ccitt_false(even_prefix.encode('ascii')):04X}"
        )
        cases = (TEST_NETWORK_ID[:-1] + "4", forged_marker)
        for network_id in cases:
            with self.subTest(network_id=network_id), tempfile.TemporaryDirectory() as raw_root:
                root = Path(raw_root)
                config = write_config(
                    root,
                    {
                        "provider": "local-bank",
                        "environment": "preprod",
                        "rail": {
                            "inbox_dir": "inbox",
                            "torii_base_url": "https://torii.local-bank.bank",
                            "network_id": network_id,
                        },
                    },
                )
                rc, _stdout, stderr = run_canary(
                    ["--config", str(config), "--plan-only"]
                )

                self.assertEqual(rc, 2)
                self.assertIn("canonical checksummed NetworkId", stderr)

    def test_rail_rejects_retired_bearer_and_emits_signed_adapter_inputs(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            body = {
                "provider": "local-bank",
                "environment": "preprod",
                "rail": {
                    "inbox_dir": "inbox",
                    "torii_base_url": "https://torii.local-bank.bank",
                    "bearer_token_file": "runtime/retired.bearer",
                },
            }
            retired = write_config(root, body)
            rc, _stdout, stderr = run_canary(
                ["--config", str(retired), "--plan-only"]
            )
            self.assertEqual(rc, 2)
            self.assertIn("rail contains unknown keys", stderr)

            del body["rail"]["bearer_token_file"]
            config = write_config(root, body)
            rc, stdout, stderr = run_canary(
                ["--config", str(config), "--plan-only"]
            )
            self.assertEqual(rc, 0, stderr)
            command = load_summary(stdout)["planned_stages"][0]["command"]
            self.assertIn("--network-id", command)
            self.assertIn(TEST_NETWORK_ID, command)
            self.assertIn("--operator-private-key-file", command)
            self.assertIn("<runtime-private-key-file>", command)

    def test_signed_rail_notary_and_receipt_verification_run_together(self):
        original_runner = CANARY._run_command_bounded

        def run_with_test_signer(argv, output_limit_bytes, timeout_secs):
            if Path(argv[1]).name != "iso_rail_gateway_adapter.py":
                return original_runner(argv, output_limit_bytes, timeout_secs)
            forwarded = list(argv[2:])
            for flag in ("--network-id", "--operator-private-key-file"):
                offset = forwarded.index(flag)
                del forwarded[offset : offset + 2]
            rc, stdout, stderr = rail_test.run_main(forwarded)
            return rc, stdout, False, stderr, False, False

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
            with rail_test.capture_server() as (torii_url, rail_requests):
                with audit_test.capture_server() as (notary_url, notary_requests):
                    config = write_config(
                        root,
                        {
                            "provider": "local-bank",
                            "environment": "preprod",
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
                    CANARY._run_command_bounded = run_with_test_signer
                    try:
                        rc, stdout, stderr = run_canary(
                            [
                                "--config",
                                str(config),
                                "--summary-out",
                                str(summary_out),
                            ]
                        )
                    finally:
                        CANARY._run_command_bounded = original_runner

            self.assertEqual(rc, 1, stderr)
            self.assertEqual(len(rail_requests), 1)
            self.assertEqual(len(notary_requests), 1)
            summary = load_summary(stdout)
            self.assertEqual(
                [stage["returncode"] for stage in summary["stages"]],
                [0, 0, 0],
            )
            rail_command = summary["stages"][0]["command"]
            self.assertIn(TEST_NETWORK_ID, rail_command)
            self.assertIn("<runtime-private-key-file>", rail_command)
            self.assertEqual(
                json.loads(summary_out.read_text(encoding="utf-8")),
                summary,
            )
            self.assertEqual(summary_out.stat().st_mode & 0o077, 0)


if __name__ == "__main__":
    unittest.main()
