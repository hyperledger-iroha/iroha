"""Google attestation revocation schema, serial and live-source boundaries."""

import http.client
import json
import unittest
import urllib.error
from unittest.mock import patch

from iroha_app_attestation.attestation import AttestationRejected
from iroha_app_attestation.revocation import (
    GOOGLE_STATUS_URL, RevocationUnavailable, certificate_serial, fetch_google_revocation_status,
    parse_google_revocation_status, verify_google_chain_not_revoked,
)


def der(tag: int, value: bytes) -> bytes:
    assert len(value) < 128
    return bytes([tag, len(value)]) + value


def certificate(serial: int) -> bytes:
    raw = serial.to_bytes(max(1, (serial.bit_length() + 7) // 8), "big")
    if raw[0] & 128:
        raw = b"\0" + raw
    tbs = der(0x30, der(2, raw) + der(2, b"\x01") * 5)
    return der(0x30, tbs + der(2, b"\x01") * 2)


class FakeResponse:
    def __init__(self, body: bytes, *, status: int = 200,
                 url: str = GOOGLE_STATUS_URL, content_type: str = "application/json") -> None:
        self.body = body
        self.status = status
        self.url = url
        self.headers = {"Content-Type": content_type}

    def __enter__(self):
        return self

    def __exit__(self, *_):
        return False

    def geturl(self):
        return self.url

    def read(self, length):
        return self.body[:length]


class FakeOpener:
    def __init__(self, response: FakeResponse) -> None:
        self.response = response

    def open(self, request, timeout):
        assert request.full_url == GOOGLE_STATUS_URL and timeout == 5
        return self.response


class RevocationTests(unittest.TestCase):
    def test_strict_status_and_expired_entry_remains_revoked(self) -> None:
        good = {"entries": {"2a": {"status": "REVOKED", "expires": "2020-01-01"},
                            "3b": {"status": "SUSPENDED", "reason": "SOFTWARE_FLAW"}}}
        self.assertEqual(parse_google_revocation_status(json.dumps(good).encode()),
                         frozenset({0x2a, 0x3b}))
        for bad in (
            b'{"entries":{"2a":{"status":"VALID"}}}',
            b'{"entries":{"2a":{"status":"REVOKED","status":"SUSPENDED"}}}',
            b'{"entries":{"02a":{"status":"REVOKED"}}}',
            b'{"entries":{"2A":{"status":"REVOKED"}}}',
            b'{"entries":{"2a":{"status":"REVOKED","reason":"unknown"}}}',
            b'{"entries":{"2a":{"status":"REVOKED","expires":"2020-02-30"}}}',
            b'{"entries":{},"extra":true}',
            b'{"entries":{}} trailing',
        ):
            with self.subTest(bad=bad), self.assertRaises(AttestationRejected) as raised:
                parse_google_revocation_status(bad)
            # Parsing a supplied body judges that body; only fetching classifies.
            self.assertNotIsInstance(raised.exception, RevocationUnavailable)

    def test_certificate_serial_and_every_chain_member(self) -> None:
        chain = [certificate(0x2a), certificate(0x3b), certificate(0x4c)]
        self.assertEqual([certificate_serial(cert) for cert in chain], [0x2a, 0x3b, 0x4c])
        with patch("iroha_app_attestation.revocation.fetch_google_revocation_status", return_value=frozenset()):
            verify_google_chain_not_revoked(chain)
        for serial in (0x2a, 0x3b, 0x4c):
            with self.subTest(serial=serial), patch(
                "iroha_app_attestation.revocation.fetch_google_revocation_status", return_value=frozenset({serial})
            ), self.assertRaises(AttestationRejected):
                verify_google_chain_not_revoked(chain)
        with self.assertRaises(AttestationRejected):
            certificate_serial(certificate(0))

    def test_fetch_rejects_wrong_endpoint_type_and_oversize(self) -> None:
        for response in (
            FakeResponse(b'{"entries":{}}', status=302),
            FakeResponse(b'{"entries":{}}', url="https://other.example/status"),
            FakeResponse(b'{"entries":{}}', content_type="text/plain"),
            FakeResponse(b"x" * (2 * 1024 * 1024 + 1)),
            FakeResponse(b'{"entries":{"2a":{"status":"VALID"}}}'),
            FakeResponse(b'{"entries":{}} trailing'),
        ):
            # No current well-formed list: retryable unavailability, which
            # still fails closed as an AttestationRejected.
            with self.subTest(response=response), patch(
                "iroha_app_attestation.revocation.urllib.request.build_opener", return_value=FakeOpener(response)
            ), self.assertRaises(RevocationUnavailable):
                fetch_google_revocation_status()
        self.assertTrue(issubclass(RevocationUnavailable, AttestationRejected))
        with patch("iroha_app_attestation.revocation.urllib.request.build_opener", return_value=FakeOpener(
            FakeResponse(b'{"entries":{"2a":{"status":"REVOKED"}}}')
        )):
            self.assertEqual(fetch_google_revocation_status(), frozenset({0x2a}))
        with patch("iroha_app_attestation.revocation.urllib.request.build_opener") as build:
            build.return_value.open.side_effect = urllib.error.URLError("offline")
            with self.assertRaisesRegex(RevocationUnavailable, "unavailable"):
                fetch_google_revocation_status()
            with self.assertRaises(RevocationUnavailable):
                verify_google_chain_not_revoked([certificate(0x2a), certificate(0x3b)])

    def test_http_protocol_failures_outside_oserror_are_unavailable(self) -> None:
        # http.client raises these outside OSError; they must not escape the
        # check (an unclassified exception would bypass the caller's retryable
        # unavailability handling).
        self.assertFalse(issubclass(http.client.HTTPException, OSError))

        class TruncatedResponse(FakeResponse):
            def read(self, length):
                raise http.client.IncompleteRead(b"")

        with patch("iroha_app_attestation.revocation.urllib.request.build_opener",
                   return_value=FakeOpener(TruncatedResponse(b'{"entries":{}}'))), \
                self.assertRaisesRegex(RevocationUnavailable, "unavailable"):
            fetch_google_revocation_status()
        for failure in (http.client.BadStatusLine("x"), http.client.LineTooLong("header line")):
            with self.subTest(failure=type(failure).__name__), patch(
                "iroha_app_attestation.revocation.urllib.request.build_opener"
            ) as build:
                build.return_value.open.side_effect = failure
                with self.assertRaisesRegex(RevocationUnavailable, "unavailable"):
                    fetch_google_revocation_status()
                with self.assertRaises(RevocationUnavailable):
                    verify_google_chain_not_revoked([certificate(0x2a), certificate(0x3b)])


if __name__ == "__main__":
    unittest.main()
