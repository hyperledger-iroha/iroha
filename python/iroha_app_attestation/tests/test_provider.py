"""Governed evidence-provider composition and fail-closed recovery tests."""

import base64
import hashlib
import shutil
import subprocess
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from unittest.mock import patch

from iroha_app_attestation.attestation import (
    AttestationRejected, PREPARATION_DOMAIN, RawPlatformProof, Selection,
    device_key_reference, encode_android_chain,
)
from iroha_app_attestation.provider import (
    AppleAppPolicy, GoogleKeyMintPolicy, OemKeyMintPolicy, GovernedEvidenceProvider,
    GovernedReleasePolicy,
)
from iroha_app_attestation.service import CertificateRequest


ROOT = b"synthetic root selected only for mocked raw-verifier tests"
APPLE_ROOT = (Path(__file__).parent / "fixtures" / "apple_app_attestation_root.der").read_bytes()
APPLE_RECEIPT_ROOT = (Path(__file__).parent / "fixtures" / "apple_root_ca_g3.der").read_bytes()
GOOGLE_FACTORY_ROOT = (Path(__file__).parent / "fixtures" / "google_factory_root_2016.der").read_bytes()
POINT = b"\x04" + b"\x44" * 64
# Google's public 2025 EC attestation root from its documented root list. Raw
# chain signatures are mocked here; the policy pin is checked with real bytes.
GOOGLE_ROOT = base64.b64decode(
    "MIICIjCCAaigAwIBAgIRAISp0Cl7DrWK5/8OgN52BgUwCgYIKoZIzj0EAwMwUjEc"
    "MBoGA1UEAwwTS2V5IEF0dGVzdGF0aW9uIENBMTEQMA4GA1UECwwHQW5kcm9pZDET"
    "MBEGA1UECgwKR29vZ2xlIExMQzELMAkGA1UEBhMCVVMwHhcNMjUwNzE3MjIzMjE4"
    "WhcNMzUwNzE1MjIzMjE4WjBSMRwwGgYDVQQDDBNLZXkgQXR0ZXN0YXRpb24gQ0Ex"
    "MRAwDgYDVQQLDAdBbmRyb2lkMRMwEQYDVQQKDApHb29nbGUgTExDMQswCQYDVQQG"
    "EwJVUzB2MBAGByqGSM49AgEGBSuBBAAiA2IABCPaI3FO3z5bBQo8cuiEas4HjqCt"
    "G/mLFfRT0MsIssPBEEU5Cfbt6sH5yOAxqEi5QagpU1yX4HwnGb7OtBYpDTB57uH5"
    "Eczm34A5FNijV3s0/f0UPl7zbJcTx6xwqMIRq6NCMEAwDwYDVR0TAQH/BAUwAwEB"
    "/zAOBgNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYEFFIyuyz7RkOb3NaBqQ5lZuA0QepA"
    "MAoGCCqGSM49BAMDA2gAMGUCMETfjPO/HwqReR2CS7p0ZWoD/LHs6hDi422opifH"
    "EUaYLxwGlT9SLdjkVpz0UUOR5wIxAIoGyxGKRHVTpqpGRFiJtQEOOTp/+s1GcxeY"
    "uR2zh/80lQyu9vAFCj6E4AXc+osmRg=="
)


class ProviderFixture:
    """A genuine issuer signature with stubbed independent platform checks."""

    def __init__(self, directory: Path, openssl: Path) -> None:
        self.directory = directory
        self.openssl = openssl
        self.run("genpkey", "-algorithm", "ED25519", "-out", "issuer.pem")
        self.run("pkey", "-in", "issuer.pem", "-pubout", "-outform", "DER",
                 "-out", "issuer.der")
        self.issuer_spki = (directory / "issuer.der").read_bytes()
        self.now = 1100
        self.policy_id = b"\x07" * 32
        self.release_id = b"\x08" * 32
        self.profile_id = b"\x09" * 32
        self.lane_id = b"\x0a" * 32

    def run(self, *args: str) -> None:
        subprocess.run([str(self.openssl), *args], cwd=self.directory,
                       capture_output=True, check=True)

    def selection(self, platform: str) -> Selection:
        return Selection(b"\x01" * 32, b"\x02" * 32, self.release_id,
                         self.profile_id,
                         hashlib.sha256(POINT).digest() if platform == "apple_app_attest"
                         else b"\0" * 32,
                         self.lane_id)

    def token(self, selection: Selection, account: str = "owner") -> bytes:
        issued, expires = 1000, 121_000
        header = (b"\x01" + issued.to_bytes(8, "little")
                  + expires.to_bytes(8, "little")
                  + b"".join(vars(selection).values()))
        message = (PREPARATION_DOMAIN + header[1:] + self.policy_id
                   + hashlib.sha256(account.encode("utf-8")).digest())
        (self.directory / "message.bin").write_bytes(message)
        self.run("pkeyutl", "-sign", "-inkey", "issuer.pem", "-rawin",
                 "-in", "message.bin", "-out", "signature.bin")
        token = header + (self.directory / "signature.bin").read_bytes()
        assert len(token) == 273
        return token

    def policy(self, platform_policy: AppleAppPolicy | GoogleKeyMintPolicy | OemKeyMintPolicy) -> GovernedReleasePolicy:
        if isinstance(platform_policy, AppleAppPolicy):
            signing_digest = hashlib.sha256(platform_policy.app_id.encode("utf-8")).digest()
            if platform_policy.expected_validation_category is None:
                release_digest = b"\x0c" * 32
            else:
                version = platform_policy.expected_bundle_version.encode("utf-8")
                release_digest = hashlib.sha256(
                    b"iroha:kagemusha:v1:app-attest-release\0"
                    + platform_policy.expected_validation_category.to_bytes(4, "little")
                    + len(version).to_bytes(2, "little") + version
                ).digest()
        else:
            signing_digest = platform_policy.signing_certificate_sha256
            release_digest = b"\x0c" * 32
        return GovernedReleasePolicy(
            self.release_id, self.profile_id, self.lane_id,
            signing_digest, release_digest, self.policy_id,
            self.issuer_spki, hashlib.sha256(self.issuer_spki).digest(),
            b"\x0d" * 32, 30_000, platform_policy,
        )

    def apple_policy(self) -> AppleAppPolicy:
        return AppleAppPolicy(
            "TEAMID.example.app", "production", APPLE_ROOT,
            hashlib.sha256(APPLE_ROOT).digest(),
            APPLE_RECEIPT_ROOT, hashlib.sha256(APPLE_RECEIPT_ROOT).digest(), 4, "1.0",
        )

    def android_policy(self) -> GoogleKeyMintPolicy:
        return GoogleKeyMintPolicy(
            "org.example.app", 10, b"\x0e" * 32,
            GOOGLE_ROOT, hashlib.sha256(GOOGLE_ROOT).digest(), frozenset({2}),
        )

    def request(self, platform: str, evidence: bytes) -> CertificateRequest:
        selected = self.selection(platform)
        return CertificateRequest("issue", "owner", self.token(selected),
                                  selected, platform, evidence)


class GovernedEvidenceProviderTests(unittest.TestCase):
    def setUp(self) -> None:
        executable = shutil.which("openssl")
        if executable is None:
            self.skipTest("OpenSSL CLI unavailable")
        self.openssl = Path(executable).resolve()
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.fixture = ProviderFixture(Path(self.temporary.name), self.openssl)

    def test_published_legacy_google_factory_anchor_is_accepted(self) -> None:
        selected = replace(
            self.fixture.android_policy(),
            attestation_root_der=GOOGLE_FACTORY_ROOT,
            attestation_root_sha256=hashlib.sha256(GOOGLE_FACTORY_ROOT).digest(),
        )
        selected.validate()

    def test_apple_issue_and_expired_preparation_recovery_keep_one_request(self) -> None:
        fixture = self.fixture
        request = fixture.request("apple_app_attest", b"apple-attestation")
        policy = fixture.policy(fixture.apple_policy())
        provider = GovernedEvidenceProvider((policy,), lambda: fixture.now, self.openssl)
        proof = RawPlatformProof(hashlib.sha256(request.platform_evidence).digest(),
                                 POINT, device_key_reference(POINT), "apple_app_attest", 4, "1.0")
        statement = {"attStmt": {"x5c": [b"attested certificate"],
                                  "receipt": b"signed Apple receipt"}}
        with (patch("iroha_app_attestation.provider.verify_apple_raw", return_value=proof) as raw,
              patch("iroha_app_attestation.provider.cbor_exact", return_value=statement),
              patch("iroha_app_attestation.provider.verify_apple_receipt") as receipt):
            issued = provider.prepare(request, issue=True)
            self.assertEqual(issued.fields.issued_at_ms, 1000)
            self.assertEqual(issued.fields.expires_at_ms, 31_000)
            self.assertEqual(issued.fields.app_release_digest, policy.app_release_digest)
            self.assertEqual(raw.call_args.args[2:4], ("TEAMID.example.app", "production"))
            self.assertEqual(raw.call_args.kwargs["expected_validation_category"], 4)
            self.assertEqual(receipt.call_args.args[:3],
                             (b"signed Apple receipt", "TEAMID.example.app",
                              b"attested certificate"))
            self.assertEqual(receipt.call_args.args[6], 1100)
            original_request = issued.verified_request(request.signed_preparation, fresh=True)
            fixture.now = 500_000
            recovered = provider.prepare(replace(request, operation="recover"), issue=False)
            self.assertEqual(recovered.verified_request(request.signed_preparation, fresh=False),
                             original_request)
            self.assertEqual(receipt.call_args.args[6], 120_999)
            self.assertEqual(raw.call_args.args[7], 120_999)
            with self.assertRaises(AttestationRejected):
                provider.prepare(request, issue=True)

    def test_apple_receipt_rejection_prevents_scope(self) -> None:
        fixture = self.fixture
        request = fixture.request("apple_app_attest", b"apple-attestation")
        provider = GovernedEvidenceProvider(
            (fixture.policy(fixture.apple_policy()),), lambda: fixture.now,
            self.openssl)
        proof = RawPlatformProof(hashlib.sha256(request.platform_evidence).digest(),
                                 POINT, device_key_reference(POINT), "apple_app_attest")
        statement = {"attStmt": {"x5c": [b"attested certificate"],
                                  "receipt": b"substituted receipt"}}
        with (patch("iroha_app_attestation.provider.verify_apple_raw", return_value=proof),
              patch("iroha_app_attestation.provider.cbor_exact", return_value=statement),
              patch("iroha_app_attestation.provider.verify_apple_receipt",
                    side_effect=AttestationRejected("receipt mismatch"))):
            with self.assertRaisesRegex(AttestationRejected, "receipt mismatch"):
                provider.prepare(request, issue=True)

    def test_signed_apple_release_must_match_governed_digest_even_without_selected_extension(self) -> None:
        fixture = self.fixture
        request = fixture.request("apple_app_attest", b"apple-attestation")
        apple = replace(fixture.apple_policy(), expected_validation_category=None,
                        expected_bundle_version=None)
        provider = GovernedEvidenceProvider(
            (fixture.policy(apple),), lambda: fixture.now, self.openssl)
        statement = {"attStmt": {"x5c": [b"attested certificate"],
                                  "receipt": b"signed Apple receipt"}}
        for category, version, error in (
            (4, "1.0", "signed release differs"),
            (None, "1.0", "incomplete Apple signed distribution"),
        ):
            with self.subTest(category=category, version=version):
                proof = RawPlatformProof(hashlib.sha256(request.platform_evidence).digest(),
                                         POINT, device_key_reference(POINT), "apple_app_attest",
                                         category, version)
                with (patch("iroha_app_attestation.provider.verify_apple_raw", return_value=proof),
                      patch("iroha_app_attestation.provider.cbor_exact", return_value=statement),
                      patch("iroha_app_attestation.provider.verify_apple_receipt")):
                    with self.assertRaisesRegex(AttestationRejected, error):
                        provider.prepare(request, issue=True)

    def test_required_apple_distribution_extension_has_no_downgrade(self) -> None:
        fixture = self.fixture
        request = fixture.request("apple_app_attest", b"apple-attestation")
        provider = GovernedEvidenceProvider(
            (fixture.policy(fixture.apple_policy()),), lambda: fixture.now,
            self.openssl)
        with (patch("iroha_app_attestation.provider.verify_apple_raw",
                    side_effect=AttestationRejected("Apple distribution policy mismatch")) as raw,
              patch("iroha_app_attestation.provider.verify_apple_receipt") as receipt):
            with self.assertRaisesRegex(AttestationRejected, "distribution policy mismatch"):
                provider.prepare(request, issue=True)
            self.assertEqual(raw.call_count, 1)
            self.assertEqual(raw.call_args.kwargs,
                             {"expected_validation_category": 4,
                              "expected_bundle_version": "1.0"})
            receipt.assert_not_called()

    def test_android_requires_live_revocation_even_for_recovery(self) -> None:
        fixture = self.fixture
        evidence = encode_android_chain([b"\x30\x00", b"\x30\x03\x02\x01\x01"])
        request = fixture.request("android_keymint", evidence)
        provider = GovernedEvidenceProvider(
            (fixture.policy(fixture.android_policy()),), lambda: fixture.now,
            self.openssl)
        proof = RawPlatformProof(hashlib.sha256(evidence).digest(), POINT,
                                 device_key_reference(POINT), "android_keymint", android_security_level=2)
        with (patch("iroha_app_attestation.provider.verify_android_raw", return_value=proof) as raw,
              patch("iroha_app_attestation.provider.verify_google_chain_not_revoked") as revocation):
            issued = provider.prepare(request, issue=True)
            self.assertEqual(issued.fields.device_key_reference, device_key_reference(POINT))
            self.assertEqual(raw.call_args.args[2:5],
                             ("org.example.app", 10, b"\x0e" * 32))
            self.assertEqual(revocation.call_count, 1)
            fixture.now = 500_000
            provider.prepare(replace(request, operation="recover"), issue=False)
            self.assertEqual(revocation.call_count, 2)
            revocation.side_effect = AttestationRejected("revoked")
            with self.assertRaisesRegex(AttestationRejected, "revoked"):
                provider.prepare(replace(request, operation="recover"), issue=False)

    def test_oem_root_requires_positive_vendor_revocation_for_issue_and_recovery(self) -> None:
        fixture = self.fixture
        evidence = encode_android_chain([b"\x30\x00", b"\x30\x03\x02\x01\x01"])
        request = fixture.request("android_keymint", evidence)
        checked = []
        def vendor_revocation(chain, now_ms):
            checked.append((chain, now_ms))
            return True
        oem = OemKeyMintPolicy(
            "org.example.app", 10, b"\x0e" * 32,
            ROOT, hashlib.sha256(ROOT).digest(), vendor_revocation, frozenset({2}))
        provider = GovernedEvidenceProvider((fixture.policy(oem),),
                                            lambda: fixture.now, self.openssl)
        proof = RawPlatformProof(hashlib.sha256(evidence).digest(), POINT,
                                 device_key_reference(POINT), "android_keymint", android_security_level=2)
        with (patch("iroha_app_attestation.provider.verify_android_raw", return_value=proof) as raw,
              patch("iroha_app_attestation.provider.verify_google_chain_not_revoked") as google):
            provider.prepare(request, issue=True)
            issued_time = fixture.now
            fixture.now = 500_000
            provider.prepare(replace(request, operation="recover"), issue=False)
            self.assertEqual(len(checked), 2)
            self.assertEqual(checked[0],
                             ((b"\x30\x00", b"\x30\x03\x02\x01\x01"), issued_time))
            self.assertEqual(checked[1][1], 500_000)
            self.assertEqual(raw.call_count, 2)
            google.assert_not_called()
        fixture.now = issued_time
        denied = replace(oem, revocation_verifier=lambda chain, now_ms: False)
        with patch("iroha_app_attestation.provider.verify_android_raw", return_value=proof):
            with self.assertRaisesRegex(AttestationRejected, "not positively clear"):
                GovernedEvidenceProvider((fixture.policy(denied),),
                                         lambda: fixture.now, self.openssl).prepare(request, issue=True)
        with self.assertRaises(AttestationRejected):
            GovernedEvidenceProvider((fixture.policy(replace(oem, revocation_verifier=None)),),
                                     lambda: fixture.now, self.openssl)
        with self.assertRaises(AttestationRejected):
            GovernedEvidenceProvider((fixture.policy(replace(oem,
                attestation_root_der=GOOGLE_ROOT,
                attestation_root_sha256=hashlib.sha256(GOOGLE_ROOT).digest())),),
                lambda: fixture.now, self.openssl)

    def test_request_cannot_replace_governed_policy_or_issuer_signature(self) -> None:
        fixture = self.fixture
        request = fixture.request("apple_app_attest", b"apple-attestation")
        provider = GovernedEvidenceProvider(
            (fixture.policy(fixture.apple_policy()),), lambda: fixture.now,
            self.openssl)
        changed = Selection(request.selection.client_nonce, request.selection.server_nonce,
                            b"\x33" * 32, request.selection.hardware_profile_id,
                            request.selection.attested_key_id, request.selection.lane_id)
        with patch("iroha_app_attestation.provider.verify_apple_raw") as raw:
            with self.assertRaises(AttestationRejected):
                provider.prepare(CertificateRequest(
                    "issue", "owner", request.signed_preparation, changed,
                    request.platform, request.platform_evidence), issue=True)
            raw.assert_not_called()
            tampered_signature = request.signed_preparation[:-1] + bytes(
                [request.signed_preparation[-1] ^ 1])
            with self.assertRaises(AttestationRejected):
                provider.prepare(CertificateRequest(
                    "issue", "owner", tampered_signature,
                    request.selection, request.platform, request.platform_evidence), issue=True)
            raw.assert_not_called()

    def test_dynamic_account_requires_its_exact_signed_preparation(self) -> None:
        fixture = self.fixture
        selected = fixture.selection("apple_app_attest")
        request = CertificateRequest("issue", "another-owner",
                                     fixture.token(selected, "another-owner"),
                                     selected, "apple_app_attest", b"apple-attestation")
        provider = GovernedEvidenceProvider((fixture.policy(fixture.apple_policy()),),
                                           lambda: fixture.now, self.openssl)
        proof = RawPlatformProof(hashlib.sha256(request.platform_evidence).digest(),
                                 POINT, device_key_reference(POINT), "apple_app_attest", 4, "1.0")
        statement = {"attStmt": {"x5c": [b"attested certificate"], "receipt": b"signed receipt"}}
        with (patch("iroha_app_attestation.provider.verify_apple_raw", return_value=proof) as raw,
              patch("iroha_app_attestation.provider.cbor_exact", return_value=statement),
              patch("iroha_app_attestation.provider.verify_apple_receipt")):
            issued = provider.prepare(request, issue=True)
            self.assertEqual(issued.account_canonical, "another-owner")
            raw.reset_mock()
            with self.assertRaises(AttestationRejected):
                provider.prepare(replace(request, account_canonical="substituted-owner"), issue=True)
            raw.assert_not_called()

    def test_unpinned_or_ambiguous_server_policy_never_starts(self) -> None:
        fixture = self.fixture
        apple = fixture.apple_policy()
        policy = fixture.policy(apple)
        with self.assertRaises(AttestationRejected):
            GovernedEvidenceProvider((policy, policy), lambda: fixture.now,
                                     self.openssl)
        bad_root = AppleAppPolicy(
            apple.app_id, apple.environment, apple.attestation_root_der,
            b"\x77" * 32, apple.receipt_root_der,
            apple.receipt_root_sha256, apple.expected_validation_category,
            apple.expected_bundle_version)
        with self.assertRaises(AttestationRejected):
            GovernedEvidenceProvider((fixture.policy(bad_root),),
                                     lambda: fixture.now, self.openssl)
        self_signed_apple = replace(apple, attestation_root_der=ROOT,
                                    attestation_root_sha256=hashlib.sha256(ROOT).digest())
        with self.assertRaises(AttestationRejected):
            GovernedEvidenceProvider((fixture.policy(self_signed_apple),),
                                     lambda: fixture.now, self.openssl)
        self_signed_receipt = replace(apple, receipt_root_der=ROOT,
                                      receipt_root_sha256=hashlib.sha256(ROOT).digest())
        with self.assertRaises(AttestationRejected):
            GovernedEvidenceProvider((fixture.policy(self_signed_receipt),),
                                     lambda: fixture.now, self.openssl)
        non_google = GoogleKeyMintPolicy(
            "org.example.app", 10, b"\x0e" * 32,
            ROOT, hashlib.sha256(ROOT).digest(), frozenset({2}))
        with self.assertRaises(AttestationRejected):
            GovernedEvidenceProvider((fixture.policy(non_google),),
                                     lambda: fixture.now, self.openssl)
        for substituted in (
            replace(policy, app_signing_identity_digest=b"\x0b" * 32),
            replace(policy, app_release_digest=b"\x0c" * 32),
            replace(fixture.policy(fixture.android_policy()),
                    app_signing_identity_digest=b"\x0b" * 32),
        ):
            with self.subTest(substituted=substituted.platform):
                with self.assertRaises(AttestationRejected):
                    GovernedEvidenceProvider((substituted,), lambda: fixture.now,
                                             self.openssl)
        with self.assertRaises(AttestationRejected):
            GovernedEvidenceProvider((policy,), lambda: True, self.openssl).prepare(
                fixture.request("apple_app_attest", b"apple-attestation"), issue=True)


if __name__ == "__main__":
    unittest.main()
