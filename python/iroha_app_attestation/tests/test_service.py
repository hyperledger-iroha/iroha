"""Request/recovery HTTP contract; fake provider is never a production verifier."""

import base64
import hashlib
import io
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from iroha_app_attestation.attestation import (
    AttestationRejected, RawPlatformProof, Selection, encode_android_chain,
)
from iroha_app_attestation.issuance import CertificateFields, DurableCertificateStore, GovernedIssuanceScope
from iroha_app_attestation.revocation import RevocationUnavailable
from iroha_app_attestation.service import IssuerService, PATH, decode_request


ANDROID_EVIDENCE = encode_android_chain([b"\x30\x00", b"\x30\x00"])


def request_body(operation: str = "issue") -> bytes:
    selection = Selection(b"\x01" * 32, b"\x02" * 32, b"\x06" * 32,
                          b"\x07" * 32, b"\0" * 32, b"\x09" * 32)
    return json.dumps({
        "operation": operation,
        "account_canonical": "owner",
        "signed_preparation_base64": base64.b64encode(b"\x01" + b"\x42" * 272).decode(),
        "selection": dict(zip((
            "client_nonce_hex", "server_nonce_hex", "release_id_hex",
            "hardware_profile_id_hex", "attested_key_id_hex", "lane_id_hex",
        ), (item.hex() for item in vars(selection).values()))),
        "platform": "android_keymint",
        "platform_evidence_base64": base64.b64encode(ANDROID_EVIDENCE).decode(),
    }, separators=(",", ":")).encode()


class ServiceTests(unittest.TestCase):
    def test_default_closed_and_exact_json_contract(self) -> None:
        service = IssuerService()
        self.assertEqual(service.handle("POST", PATH, request_body(), "application/json")[0], 503)
        self.assertEqual(service.handle("POST", PATH, request_body("recover"), "application/json")[0], 503)
        self.assertEqual(service.handle("GET", PATH, request_body(), "application/json")[0], 404)
        self.assertEqual(service.handle("POST", PATH, request_body(), "text/plain")[0], 415)
        decoded = decode_request(request_body())
        self.assertEqual(decoded.selection.attested_key_id, b"\0" * 32)
        for bad in (
            request_body()[:-1],
            request_body().replace(b'"operation":"issue"', b'"operation":"issue","operation":"issue"'),
            request_body().replace(b'"operation":"issue"', b'"operation":"publish"'),
            request_body().replace(b'"account_canonical":"owner"', b'"account_canonical":"owner","unknown":1'),
            request_body().replace(b'"account_canonical":"owner"', b'"account_canonical":"\\ud800"'),
            b"[" * 1_100 + b"0" + b"]" * 1_100,
            request_body().replace(base64.b64encode(ANDROID_EVIDENCE), b"%%%"),
            request_body().replace(base64.b64encode(ANDROID_EVIDENCE),
                                   base64.b64encode(b"unframed Android chain")),
        ):
            with self.subTest(body=bad):
                self.assertEqual(service.handle("POST", PATH, bad, "application/json")[0], 400)

    def test_unknown_revocation_status_is_retryable_not_a_rejected_certificate(self) -> None:
        for error, expected in (
            (RevocationUnavailable("Android revocation status unavailable"),
             (503, b'{"error":"issuer_unavailable"}')),
            (AttestationRejected("revoked Android attestation certificate"),
             (409, b'{"error":"certificate_rejected"}')),
        ):
            class FailingProvider:
                def prepare(self, _request, *, issue, error=error):
                    raise error

            service = IssuerService(object(), FailingProvider(), lambda frame: b"unused",
                                    caller_authorizer=lambda _environment: "core-api-mtls",
                                    expected_caller_identity="core-api-mtls")
            for operation in ("issue", "recover"):
                with self.subTest(error=type(error).__name__, operation=operation):
                    self.assertEqual(service.handle("POST", PATH, request_body(operation),
                                                    "application/json",
                                                    caller_identity="core-api-mtls"), expected)

    def test_handler_issues_once_and_recovers_without_signer(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            os.chmod(directory, 0o700)
            store = DurableCertificateStore(directory / "certificates.sqlite")
            decoded = decode_request(request_body())
            audit = decoded.platform_evidence
            proof = RawPlatformProof(hashlib.sha256(audit).digest(),
                                     b"\x04" + b"\x51" * 64,
                                     b"\x08" * 32, "android_keymint")
            certificate_fields = CertificateFields(
                *(bytes([index]) * 32 for index in range(1, 11)), 1_000, 2_000,
            )
            request = certificate_fields.request(b"\x0a" * 32)

            # Exercise transport and persistence with a real scope shape. The
            # separate issuance/provider suites verify its cryptographic path.
            scope = GovernedIssuanceScope(
                decoded.selection, decoded.account_canonical, b"\x0b" * 32,
                b"fixture-spki", b"\x0c" * 32, 1_500, Path("/usr/bin/openssl"),
                decoded.platform, proof, decoded.selection.release_id,
                decoded.selection.hardware_profile_id, decoded.selection.lane_id,
                certificate_fields.app_signing_identity_digest,
                certificate_fields.app_release_digest, 1_000, b"\x0a" * 32,
                certificate_fields,
            )
            verifier_patch = patch.object(GovernedIssuanceScope, "verified_request",
                                          return_value=request)
            verifier_patch.start()
            self.addCleanup(verifier_patch.stop)

            class FakeProvider:
                def prepare(self, _request, *, issue):
                    return scope

            calls = []

            def signer(frame):
                calls.append(frame)
                return b"canonical certificate fixture"

            service = IssuerService(store, FakeProvider(), signer,
                                    caller_authorizer=lambda _environment: "core-api-mtls",
                                    expected_caller_identity="core-api-mtls")
            self.assertEqual(service.handle("POST", PATH, request_body(), "application/json")[0], 503)
            class AlwaysEqual:
                def __eq__(self, _other):
                    return True

            for bad_identity in (False, True, 0, "", " core-api-mtls", "core-api-mtls ", AlwaysEqual()):
                with self.subTest(bad_identity=bad_identity):
                    self.assertEqual(service.handle("POST", PATH, request_body(),
                                                    "application/json",
                                                    caller_identity=bad_identity)[0], 503)
            code, body = service.handle("POST", PATH, request_body(), "application/json",
                                        caller_identity="core-api-mtls")
            self.assertEqual(code, 200)
            self.assertEqual(base64.b64decode(json.loads(body)["certificate_base64"]),
                             b"canonical certificate fixture")
            self.assertEqual(calls, [request])
            code, body = service.handle("POST", PATH, request_body("recover"), "application/json",
                                        caller_identity="core-api-mtls")
            self.assertEqual(code, 200)
            self.assertEqual(base64.b64decode(json.loads(body)["certificate_base64"]),
                             b"canonical certificate fixture")
            self.assertEqual(calls, [request])
            changed = request_body("recover").replace(base64.b64encode(ANDROID_EVIDENCE),
                                                       base64.b64encode(encode_android_chain(
                                                           [b"\x30\x00", b"\x30\x03\x02\x01\x01"])))
            self.assertEqual(service.handle("POST", PATH, changed, "application/json",
                                            caller_identity="core-api-mtls")[0], 409)
            statuses = []
            environment = {"REQUEST_METHOD": "POST", "PATH_INFO": PATH,
                           "CONTENT_TYPE": "application/json", "CONTENT_LENGTH": str(len(request_body("recover"))),
                           "wsgi.input": io.BytesIO(request_body("recover"))}
            output = service.wsgi(environment, lambda status, headers: statuses.append((status, headers)))
            self.assertTrue(statuses[0][0].startswith("200"))
            self.assertEqual(b"".join(output), body)
            for gated_service, status_prefix in (
                (IssuerService(store, FakeProvider(), signer), "503"),
                (IssuerService(store, FakeProvider(), signer,
                               caller_authorizer=lambda _environment: None,
                               expected_caller_identity="core-api-mtls"), "401"),
                (IssuerService(store, FakeProvider(), signer,
                               caller_authorizer=lambda _environment: False,
                               expected_caller_identity="core-api-mtls"), "401"),
                (IssuerService(store, FakeProvider(), signer,
                               caller_authorizer=lambda _environment: "",
                               expected_caller_identity="core-api-mtls"), "401"),
                (IssuerService(store, FakeProvider(), signer,
                               caller_authorizer=lambda _environment: "other-client",
                               expected_caller_identity="core-api-mtls"), "401"),
                (IssuerService(store, FakeProvider(), signer,
                               caller_authorizer=lambda _environment: AlwaysEqual(),
                               expected_caller_identity="core-api-mtls"), "401"),
                (IssuerService(store, FakeProvider(), signer,
                               caller_authorizer=lambda _environment: "core-api-mtls"), "503"),
            ):
                rejected = []
                gated_service.wsgi(environment, lambda status, headers: rejected.append(status))
                self.assertTrue(rejected[0].startswith(status_prefix))
            self.assertEqual(calls, [request])


if __name__ == "__main__":
    unittest.main()
