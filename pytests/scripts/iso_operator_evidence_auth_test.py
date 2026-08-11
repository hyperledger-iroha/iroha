"""Exact rail operator-auth replay tests for ISO canary evidence."""

import tempfile
import unittest
from pathlib import Path

from pytests.scripts.iso_operator_evidence_verify_test import (
    EVIDENCE,
    run_evidence,
    valid_canary_summary,
    write_canary,
    write_trust_summary,
)
from pytests.scripts.iso_operator_evidence_verify_test_support import (
    TEST_NETWORK_ID,
    digest_summary,
)


def _flag_offset(command, flag):
    return command.index(flag)


class IsoOperatorEvidenceAuthTest(unittest.TestCase):
    def test_valid_summary_replays_exact_network_and_redacted_key(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            canary_path = write_canary(root)
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

            self.assertEqual(rc, 0, stderr)

    def test_network_and_operator_key_substitutions_are_rejected(self):
        even_prefix = "hash:" + ("08" * 32)
        forged_marker = (
            f"{even_prefix}#{EVIDENCE._crc16_ccitt_false(even_prefix.encode('ascii')):04X}"
        )

        def remove_flag(body, flag):
            command = body["stages"][0]["command"]
            offset = _flag_offset(command, flag)
            del command[offset : offset + 2]

        def replace_flag(body, flag, value):
            command = body["stages"][0]["command"]
            command[_flag_offset(command, flag) + 1] = value

        cases = (
            (
                "missing-network",
                lambda body: remove_flag(body, "--network-id"),
                "must contain --network-id",
            ),
            (
                "missing-operator-key",
                lambda body: remove_flag(body, "--operator-private-key-file"),
                "must contain --operator-private-key-file",
            ),
            (
                "wrong-checksum",
                lambda body: replace_flag(
                    body, "--network-id", TEST_NETWORK_ID[:-1] + "4"
                ),
                "canonical checksummed NetworkId",
            ),
            (
                "forged-marker-bit",
                lambda body: replace_flag(body, "--network-id", forged_marker),
                "canonical checksummed NetworkId",
            ),
            (
                "unredacted-operator-key",
                lambda body: replace_flag(
                    body,
                    "--operator-private-key-file",
                    "/ops/runtime/operator.key",
                ),
                "unredacted operator-private-key file path",
            ),
            (
                "retired-rail-bearer",
                lambda body: body["stages"][0]["command"].extend(
                    ["--bearer-token-file", "<runtime-token-file>"]
                ),
                "uses unsupported flag",
            ),
        )
        for name, mutate, expected in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_root:
                root = Path(raw_root)
                body = valid_canary_summary()
                mutate(body)
                canary_path = write_canary(root, digest_summary(body))
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
                self.assertIn(expected, stderr)


if __name__ == "__main__":
    unittest.main()
