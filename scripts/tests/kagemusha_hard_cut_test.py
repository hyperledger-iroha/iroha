"""Guard the first-release KAGEMUSHA identity against retired product names."""

from __future__ import annotations

import json
import re
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def _reversed(*parts: str) -> bytes:
    return "".join(reversed(parts)).encode()


RETIRED_RAW = (
    _reversed("Cash", "Offline"),
    _reversed("_cash", "offline"),
    _reversed("-cash", "offline"),
    _reversed(" cash v1", "offline"),
    _reversed("TopUpRequest", "Offline"),
    _reversed("RedeemRequest", "Offline"),
    _reversed("OperationReference", "Offline"),
    _reversed("OperationIdentity", "Offline"),
    _reversed("NativeCore", "Offline"),
    _reversed("line", "off", "v1/"),
    _reversed(":", "oc1"),
    _reversed("handoff_v1", "cash_"),
    _reversed("FJ", "IOC"),
    _reversed("-seal-v1", "iroha-oc"),
    _reversed("-sig-v1", "iroha-oc"),
    _reversed("V2", "Kagemusha"),
    _reversed("V4", "Kagemusha"),
    _reversed("V5", "Kagemusha"),
    _reversed("_v2", "kagemusha"),
    _reversed("_v4", "kagemusha"),
    _reversed("_v5", "kagemusha"),
    _reversed("lifecycle", "/kagemusha/", "/v1"),
)
RETIRED_NORMALIZED = (
    _reversed("cash", "offline"),
    _reversed("topuprequest", "offline"),
    _reversed("redeemrequest", "offline"),
    _reversed("operationreference", "offline"),
    _reversed("operationidentity", "offline"),
    _reversed("nativecore", "offline"),
    _reversed("v2", "kagemusha"),
    _reversed("v4", "kagemusha"),
    _reversed("v5", "kagemusha"),
)

RETIRED_KAGEMUSHA_FIXTURE_TOKENS = (
    _reversed("v2", "fixture_", "canonical_"),
    _reversed("V2", "FIXTURE_"),
)
KAGEMUSHA_FIXTURE_SOURCES = (
    ROOT / "crates/connect_norito_bridge/src/kagemusha_fixture_tests.rs",
    ROOT / "IrohaSwift/Tests/IrohaSwiftTests/KagemushaWireV1Tests.swift",
    ROOT
    / "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/offline/KagemushaThreeMessageV1Test.kt",
    ROOT
    / "java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaWireV1Tests.java",
    ROOT / "javascript/iroha_js/test/kagemushaCanonicalFixture.test.js",
    ROOT / "python/iroha_python/tests/kagemusha_fixture_test.py",
    ROOT / "csharp/tests/Hyperledger.Iroha.Sdk.Tests/KagemushaCanonicalFixtureV1Tests.cs",
)

WIRE_CONTRACT_SOURCES = (
    ROOT / "crates/iroha_data_model/src/kagemusha/kagemusha_v1.rs",
    ROOT / "IrohaSwift/Sources/IrohaSwift/KagemushaModelsV1.swift",
    ROOT / "IrohaSwift/Sources/IrohaSwift/KagemushaNoritoV1.swift",
    ROOT
    / "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaModelsV1.kt",
    ROOT
    / "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaNoritoV1.kt",
    ROOT
    / "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaNoritoV1.java",
    ROOT / "javascript/iroha_js/src/kagemusha.js",
    ROOT / "javascript/iroha_js/kagemusha.d.ts",
    ROOT / "python/iroha_python/src/iroha_python/kagemusha.py",
    ROOT / "csharp/src/Hyperledger.Iroha.Sdk/Kagemusha/KagemushaV1Models.cs",
    ROOT / "csharp/src/Hyperledger.Iroha.Sdk/Kagemusha/Kagemusha.cs",
    ROOT / "formal/kagemusha_v1/KagemushaV1.tla",
)

DEVICE_OPERATION_SOURCES = (
    ROOT / "IrohaSwift/Sources/IrohaSwift/KagemushaDeviceLifecycleBridgeV1.swift",
    ROOT
    / "kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaDeviceLifecycleBridgeV1.kt",
    ROOT
    / "kotlin/client-android/src/test/java/org/hyperledger/iroha/sdk/offline/KagemushaDeviceLifecycleBridgeV1Test.kt",
    ROOT
    / "java/iroha_android/android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaDeviceLifecycleBridgeV1.java",
    ROOT
    / "java/iroha_android/android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaDeviceLifecycleBridgeV1Tests.java",
)

RETIRED_INTENT_AUTHORIZATION_SPELLINGS = (
    _reversed("Authorization", "AcceptanceIntent"),
    _reversed("authorization", "acceptance_intent_"),
    _reversed("Id", "intentAuthorization"),
    _reversed("Digest", "intentAuthorization"),
    _reversed("id", "intent_authorization_"),
    _reversed("digest", "intent_authorization_"),
)

RETIRED_HANDSHAKE_SPELLINGS = (
    _reversed("V1", "Intent", "Acceptance", "Kagemusha"),
    _reversed("V1", "Ticket", "Acceptance", "Kagemusha"),
    _reversed("V1", "Mode", "Request", "Payment", "Kagemusha"),
    _reversed("V1", "Budget", "Request", "Receiver", "Kagemusha"),
    _reversed("V1", "Closure", "Commit", "No", "Kagemusha"),
    _reversed("intent", "acceptance_"),
    _reversed("ticket", "acceptance_"),
)

RETIRED_PUBLIC_PROOF_TOKENS = (
    _reversed("v1", "wrapper", "commit", "kagemusha"),
    _reversed("wrapper", "commit", "encode"),
    _reversed("wrapper", "commit", "decode"),
    _reversed("bytes", "wrapper", "commit", "maximum"),
    _reversed("maxbytes", "wrapper", "commit"),
)


def _retired_public_proof_alias(data: bytes) -> bytes | None:
    normalized = re.sub(rb"[^a-z0-9]+", b"", data.lower())
    retired = next(
        (token for token in RETIRED_PUBLIC_PROOF_TOKENS if token in normalized), None
    )
    if retired is not None:
        return retired
    # These two closed inventories name the current internal relation/artifacts,
    # including explicit numeric discriminants; they are not proof type aliases.
    public_surface = re.sub(
        rb"\benum(?:\s+class)?\s+Kagemusha(?:QualifiedRelation|ArtifactRole)V1\s*\{[^}]*\}",
        b"",
        data,
        flags=re.S,
    )
    # Cover exports, declarations, and namespace properties pointing at a
    # canonical proof, not only an independently named proof codec/type.
    alias = re.search(
        rb"\bas\s+commit_?wrapper\b"
        rb"|\b(?:class|struct|interface|type)\s+commit_?wrapper\b"
        rb"|\bcommit_?wrapper\b[\"']?\s*[:=]",
        public_surface,
        re.I,
    )
    return alias.group(0) if alias is not None else None


def _repository_files() -> tuple[Path, ...]:
    listing = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
    ).stdout
    return tuple(ROOT / raw.decode() for raw in listing.split(b"\0") if raw)


def _retired_identity(data: bytes) -> bytes | None:
    lowered = data.lower()
    for retired in RETIRED_RAW:
        if retired.lower() in lowered:
            return retired
        for spelling in (retired, retired.lower(), retired.upper()):
            if spelling.hex().encode() in lowered:
                return retired
    normalized = re.sub(rb"[^a-z0-9]+", b"", lowered)
    for retired in RETIRED_NORMALIZED:
        if retired in normalized:
            return retired
        for spelling in (retired, retired.upper()):
            if spelling.hex().encode() in lowered:
                return retired
    return None


def _retired_file(path: Path) -> bytes | None:
    trailing = b""
    with path.open("rb") as source:
        while chunk := source.read(8 * 1024):
            if b"\0" in chunk:
                return None
            sample = trailing + chunk
            retired = _retired_identity(sample)
            if retired is not None:
                return retired
            trailing = sample[-256:]
    return None


class KagemushaHardCutTests(unittest.TestCase):
    """Enforce the unaliased first-release KAGEMUSHA surface."""

    def test_repository_contains_no_retired_product_identity(self) -> None:
        failures: list[str] = []
        for path in _repository_files():
            relative = path.relative_to(ROOT)
            retired = _retired_identity(str(relative).encode())
            if retired is not None:
                failures.append(f"retired path {relative}: {retired.decode()}")
                continue
            if path.is_symlink():
                data = str(path.readlink()).encode()
                retired = _retired_identity(data)
            elif path.is_file():
                retired = _retired_file(path)
            else:
                continue
            if retired is not None:
                failures.append(f"retired content {relative}: {retired.decode()}")
                continue
        self.assertEqual(failures, [])

    def test_canonical_fixture_is_v1_only(self) -> None:
        """Reject a second fixture generation in every KAGEMUSHA SDK lane."""
        fixture_path = ROOT / "fixtures/offline/kagemusha_v1.json"
        fixture = json.loads(fixture_path.read_text(encoding="utf-8"))
        self.assertEqual(fixture.get("fixture_version"), 1)
        self.assertEqual(
            fixture.get("ipm1_message_order"),
            [
                {"kind": name, "tag": tag}
                for tag, name in enumerate(("request", "payment", "acknowledgement"), 1)
            ],
        )
        self.assertNotIn("acceptance_intent", fixture)
        self.assertNotIn("acceptance_ticket", fixture)
        self.assertIn("payment_proof", fixture)
        self.assertNotIn(
            _reversed("digest_hex", "acceptance_intent_").decode(),
            fixture["identity_vectors"],
        )

        failures: list[str] = []
        for path in KAGEMUSHA_FIXTURE_SOURCES:
            data = path.read_bytes()
            for retired in RETIRED_KAGEMUSHA_FIXTURE_TOKENS:
                if retired in data:
                    failures.append(
                        f"{path.relative_to(ROOT)} contains {retired.decode()}"
                    )
        self.assertEqual(failures, [])

        generator = KAGEMUSHA_FIXTURE_SOURCES[0].read_text(encoding="utf-8")
        self.assertIn("canonical_fixture_v1", generator)
        self.assertIn('"fixture_version": 1', generator)
        self.assertIn("PRINT_KAGEMUSHA_FIXTURE_V1", generator)

        expected_v1_assertions = (
            'fixture["fixture_version"] as? Int), 1)',
            'assertEquals(1, fixtureInt(fixture, "fixture_version"))',
            'assertEquals(1L, fixtureLong(fixture, "fixture_version"))',
            "assert.equal(fixture.fixture_version, 1)",
            'fixture["fixture_version"] == 1',
            'Assert.Equal(1, root.GetProperty("fixture_version")',
        )
        for path, expected in zip(
            KAGEMUSHA_FIXTURE_SOURCES[1:], expected_v1_assertions, strict=True
        ):
            self.assertIn(expected, path.read_text(encoding="utf-8"), path)

    def test_detector_catches_plain_normalized_and_hex_spellings(self) -> None:
        retired_name = _reversed("CashV1", "Offline")
        retired_route = _reversed("readiness", "/", "line", "off", "/v1/")
        retired_schema = _reversed("OperationReference", "Offline")
        retired_native_core = _reversed("NativeCore", "Offline")
        for payload in (
            retired_name,
            retired_name.replace(b"Cash", b"_cash_"),
            retired_route,
            retired_schema.hex().encode(),
            retired_native_core,
        ):
            self.assertIsNotNone(_retired_identity(payload))
        retired_route = b"/v1/" + _reversed("line", "off") + b"/readiness"
        self.assertIsNotNone(_retired_identity(retired_route))
        allowed_route = b"/v1/" + b"kagemusha/readiness"
        self.assertIsNone(_retired_identity(b"KAGEMUSHA kgm1: " + allowed_route))

        retired_version = _reversed("V5", "Kagemusha")
        self.assertIsNotNone(_retired_identity(retired_version))

        # These are required KAGEMUSHA V1 protocol components, not retired
        # product aliases. The product-identity guard must never suppress them.
        for required in (
            b"KagemushaPaymentProofV1",
            b"ReceiveFold",
            b"KagemushaRedemptionProofV1",
        ):
            self.assertIsNone(_retired_identity(required))

    def test_no_retired_terminal_proof_alias(self) -> None:
        """Reject retired public proof APIs, not the current internal circuit relation."""
        failures: list[str] = []
        # The formal/circuit implementation may name the required internal
        # post-commit relation. Only public wire and codec surfaces are in scope.
        sources = WIRE_CONTRACT_SOURCES[:-1] + (
            ROOT / "crates/connect_norito_bridge/include/connect_norito_bridge.h",
            ROOT / "crates/iroha_torii_shared/src/kagemusha_api.rs",
        )
        for path in sources:
            data = path.read_bytes()
            if path.suffix == ".rs":
                data = data.split(b"#[cfg(test)]", 1)[0]
            retired = _retired_public_proof_alias(data)
            if retired is not None:
                failures.append(f"{path.relative_to(ROOT)} contains {retired.decode()}")
        self.assertEqual(failures, [])

    def test_public_proof_guard_allows_only_the_internal_wrapper_relation(self) -> None:
        """The post-commit circuit/artifact family is not a public wire alias."""
        for data in (
            _reversed("V1", "Wrapper", "Commit", "Kagemusha"),
            _reversed("_shape", "_wrapper", "_commit", "encode"),
            _reversed("ShapeExact", "Wrapper", "Commit", "decode"),
            b"CommitWrapper = KagemushaPaymentProofV1",
            b'"commit_wrapper": PaymentProof',
            b"pub use KagemushaPaymentProofV1 as CommitWrapper;",
            b"export { PaymentProof as CommitWrapper };",
            b"pub type CommitWrapper = KagemushaPaymentProofV1;",
            b"public class CommitWrapper {}",
        ):
            self.assertIsNotNone(_retired_public_proof_alias(data))
        for data in (
            b"KagemushaPaymentProofV1",
            b"KagemushaRedemptionProofV1",
            b"KagemushaQualifiedRelationV1::CommitWrapper",
            b"CommitWrapperVkEq CommitWrapperVkEp CommitWrapperPkEq CommitWrapperPkEp",
            b"commit_wrapper_eq_protocol_digest commit_wrapper_ep_protocol_digest",
            b"KagemushaCommitWrapperEqWitnessV1 KagemushaCommitWrapperEpWitnessV1",
            b"pub enum KagemushaQualifiedRelationV1 { CommitWrapper = 7, }",
            b"enum class KagemushaArtifactRoleV1 { CommitWrapperVkEq, CommitWrapperVkEp }",
        ):
            self.assertIsNone(_retired_public_proof_alias(data))

    def test_complete_exchange_has_one_unaliased_hard_budget(self) -> None:
        """Reject the retired terminal-subset and enlarged composed allowances."""
        sources = (
            ROOT / "crates/iroha_data_model/src/kagemusha/kagemusha_v1.rs",
            ROOT / "javascript/iroha_js/src/kagemusha.js",
            ROOT / "python/iroha_python/src/iroha_python/kagemusha.py",
            ROOT / "IrohaSwift/Sources/IrohaSwift/KagemushaWireV1.swift",
            ROOT
            / "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaWireV1.kt",
            ROOT / "csharp/src/Hyperledger.Iroha.Sdk/Kagemusha/Kagemusha.cs",
        )
        retired = (
            "18_171",
            "18,171",
            "18171",
            "24_256",
            "24,256",
            "24256",
            "terminal_handoff",
            "TERMINAL_HANDOFF",
            "terminal handoff",
        )
        failures: list[str] = []
        for path in sources:
            if not path.is_file():
                continue
            text = path.read_text(encoding="utf-8")
            for token in retired:
                if token in text:
                    failures.append(f"{path.relative_to(ROOT)} contains {token!r}")
        self.assertEqual(failures, [])

    def test_public_wire_has_no_retired_handshake(self) -> None:
        """The first release has no request modes, intent, ticket, or cancellation path."""
        failures: list[str] = []
        for path in WIRE_CONTRACT_SOURCES:
            data = path.read_bytes()
            if path.suffix == ".rs":
                # Rust's inline negative tests may name a rejected surface.
                data = data.split(b"#[cfg(test)]", 1)[0]
            for retired in RETIRED_INTENT_AUTHORIZATION_SPELLINGS:
                if retired.lower() in data.lower():
                    failures.append(
                        f"{path.relative_to(ROOT)} contains {retired.decode()}"
                    )
            for retired in RETIRED_HANDSHAKE_SPELLINGS:
                if retired.lower() in data.lower():
                    failures.append(
                        f"{path.relative_to(ROOT)} contains {retired.decode()}"
                    )
        self.assertEqual(failures, [])

    def test_device_operation_sources_and_tests_have_no_retired_handshake(self) -> None:
        """Keep mobile operation inventories and their tests on the exact 22-operation cut."""
        failures: list[str] = []
        for path in DEVICE_OPERATION_SOURCES:
            data = path.read_bytes().lower()
            for retired in (
                *RETIRED_INTENT_AUTHORIZATION_SPELLINGS,
                *RETIRED_HANDSHAKE_SPELLINGS,
            ):
                if retired.lower() in data:
                    failures.append(
                        f"{path.relative_to(ROOT)} contains {retired.decode()}"
                    )
        self.assertEqual(failures, [])

        kotlin_test = DEVICE_OPERATION_SOURCES[2].read_text(encoding="utf-8")
        java_test = DEVICE_OPERATION_SOURCES[4].read_text(encoding="utf-8")
        self.assertIn("assertEquals((1..22).toList(), operations.map { it.code })", kotlin_test)
        self.assertIn(
            "assertEquals(22, KagemushaDeviceLifecycleBridgeV1.Operation.values().length)",
            java_test,
        )

    def test_csharp_wallet_requires_operation_ids_and_complete_mint_bundle(self) -> None:
        """Keep the mirrored managed wallet on the same crash-safe first-release contract."""
        source = (
            ROOT / "csharp/src/Hyperledger.Iroha.Sdk/Kagemusha/KagemushaWalletV1.cs"
        ).read_text(encoding="utf-8")
        for required in (
            "ReceiverBoundCreditCommit",
            "ReservePaymentOperationId",
            "RecoverPaymentByOperationId",
            "KagemushaMintConstructionBundleV1",
            "ReserveMintOperationId",
            "PrepareMintConstructionBundle",
            "RecoverMintConstructionBundle",
            "ReserveRedemptionOperationId",
            "RecoverRedemptionByOperationId",
            "FoldRequiredCreditsLocked(request.Amount);",
            "FoldRequiredCreditsLocked(amount);",
            "KagemushaPendingCreditSelectorV1",
            "KagemushaPendingCreditWatermarkV1",
            "CoreAuthorizationKeyReference",
            "SelectPendingCredit",
            "FoldPendingCredit",
        ):
            self.assertIn(required, source)
        for retired in (
            "CommitPayment(byte[] canonicalRequest)",
            "CommitRedemption(UInt128 amount, byte[] beneficiaryAccount)",
            "FoldReceiveCredit(UInt128 inboxSequenceInclusive)",
            "NextPendingCreditId",
            "DrainPendingCreditsLocked",
        ):
            self.assertNotIn(retired, source)

    def test_three_message_payment_contract_is_directly_request_bound(self) -> None:
        """Pin the direct request/payment/ACK contract and post-commit proof."""
        source = WIRE_CONTRACT_SOURCES[0].read_text(encoding="utf-8")

        def body(name: str) -> str:
            match = re.search(rf"^pub struct {name} \{{(.*?)^\}}", source, re.M | re.S)
            self.assertIsNotNone(match, name)
            assert match is not None
            return match.group(1)

        request = body("KagemushaPaymentRequestV1")
        payment = body("KagemushaPaymentV1")
        output = body("KagemushaPaymentOutputV1")
        acknowledgement = body("KagemushaAcknowledgementV1")
        self.assertIn("pub amount: u128", request)
        self.assertIn("pub recipient_encryption_key:", request)
        self.assertIn("pub hardware_credential: KagemushaHardwareCredentialV1", request)
        self.assertIn("pub commit_certificate: KagemushaCommitCertificateV1", payment)
        self.assertIn("pub proof: KagemushaPaymentProofV1", payment)
        self.assertNotIn("pub terminal_signature:", payment)
        self.assertIn("pub request_digest: [u8; 32]", output)
        self.assertIn("pub amount: u128", output)
        self.assertIn("pub sender_before_commitment: [u8; 32]", output)
        self.assertIn("pub sender_after_commitment: [u8; 32]", output)
        self.assertIn("pub credit_id: [u8; 32]", output)
        self.assertIn("pub request_digest: [u8; 32]", acknowledgement)
        self.assertIn("pub payment_digest: [u8; 32]", acknowledgement)

    def test_transport_spec_matches_the_canonical_wire_bounds(self) -> None:
        """Derive per-message table ceilings from the authoritative Rust constants."""
        transport = (ROOT / "specs/peer_transport_v1.md").read_text(encoding="utf-8")
        source = WIRE_CONTRACT_SOURCES[0].read_text(encoding="utf-8")

        def bound(name: str) -> int:
            match = re.search(rf"pub const {name}: usize = ([\d_]+);", source)
            self.assertIsNotNone(match, name)
            assert match is not None
            return int(match.group(1).replace("_", ""))

        table = {
            name: (int(raw.replace(",", "")), int(text.replace(",", "")))
            for name, raw, text in re.findall(
                r"^\|\s+([^|]+?)\s+\|\s+([\d,]+) bytes\s+\|\s+([\d,]+) bytes\s+\|",
                transport,
                re.M,
            )
        }
        messages = (
            ("KagemushaPaymentRequestV1", "PAYMENT_REQUEST"),
            ("KagemushaPaymentV1", "PAYMENT"),
            ("KagemushaAcknowledgementV1", "ACKNOWLEDGEMENT"),
        )
        for model, constant in messages:
            raw = bound(f"KAGEMUSHA_{constant}_MAX_BYTES_V1")
            text = len("kgm1:") + (raw * 8 + 5) // 6
            with self.subTest(model=model):
                self.assertEqual(table.get(f"`{model}`"), (raw, text))
        complete_raw = bound("KAGEMUSHA_COMPLETE_EXCHANGE_MAX_BYTES_V1")
        complete_text = bound("KAGEMUSHA_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1")
        self.assertEqual(
            table.get("Complete exchange hard gate"),
            (complete_raw, complete_text),
        )
        self.assertTrue(
            "KagemushaPaymentProofV1" in transport,
            "the transport specification must name the post-commit payment proof",
        )


if __name__ == "__main__":
    unittest.main()
