"""Canonical signed-rail profile-query tests for ISO receipt replay."""

import tempfile
import unittest
from pathlib import Path

from pytests.scripts.iso_operator_receipt_verify_test import (
    VERIFIER,
    rail_test,
    rewrite_receipt,
    run_verify,
)


class IsoOperatorReceiptProfileQueryTest(unittest.TestCase):
    def test_exact_profile_query_is_accepted(self):
        VERIFIER._require_https(
            "https://torii.bank/v1/iso20022/pacs002?profile=swift-cbpr-plus",
            allow_insecure_http=False,
            label="receipt",
            rail_profile="swift-cbpr-plus",
        )
        VERIFIER._require_https(
            "https://torii.bank/v1/iso20022/pacs002",
            allow_insecure_http=False,
            label="receipt",
        )
        with self.assertRaises(VERIFIER.ReceiptError):
            VERIFIER._require_https(
                "https://torii.bank/v1/iso20022/pacs002?",
                allow_insecure_http=False,
                label="receipt",
            )

    def test_profile_query_substitutions_and_smuggling_are_rejected(self):
        cases = (
            "https://torii.bank/v1/iso20022/pacs002",
            "https://torii.bank/v1/iso20022/pacs002?",
            "https://torii.bank/v1/iso20022/pacs002?profile=sepa-sct-inst",
            "https://torii.bank/v1/iso20022/pacs002?profile=swift%2Dcbpr%2Dplus",
            "https://torii.bank/v1/iso20022/pacs002"
            "?profile=swift-cbpr-plus&profile=swift-cbpr-plus",
            "https://torii.bank/v1/iso20022/pacs002?other=swift-cbpr-plus",
            "https://torii.bank/v1/iso20022/pacs002?profile=swift-cbpr-plus#fragment",
            "https://user@torii.bank/v1/iso20022/pacs002?profile=swift-cbpr-plus",
            "http://torii.bank/v1/iso20022/pacs002?profile=swift-cbpr-plus",
        )
        for url in cases:
            with self.subTest(url=url):
                with self.assertRaises(VERIFIER.ReceiptError):
                    VERIFIER._require_https(
                        url,
                        allow_insecure_http=False,
                        label="receipt",
                        rail_profile="swift-cbpr-plus",
                    )

    def test_receipt_profile_is_bound_to_recorded_endpoint_query(self):
        with tempfile.TemporaryDirectory() as raw_root:
            inbox = Path(raw_root) / "inbox"
            inbox.mkdir()
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

            rc, _stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--allow-insecure-http",
                    "--require-source-files",
                ]
            )
            self.assertEqual(rc, 0, stderr)

            def substitute_profile(body):
                endpoint = body["endpoint_url"].replace(
                    "profile=swift-cbpr-plus", "profile=sepa-sct-inst"
                )
                body["endpoint_url"] = endpoint
                body["endpoint_sha256"] = VERIFIER.sha256_hex(
                    endpoint.encode("utf-8")
                )

            rewrite_receipt(receipt, substitute_profile)
            rc, _stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--allow-insecure-http",
                    "--require-source-files",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("query must exactly match the recorded rail profile", stderr)


if __name__ == "__main__":
    unittest.main()
