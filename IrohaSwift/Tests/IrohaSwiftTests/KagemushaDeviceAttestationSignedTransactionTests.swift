import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaDeviceAttestationSignedTransactionTests: XCTestCase {
    func testRehydratesExactEnvelopeAndRecomputesHashes() throws {
        let fixture = try makeEnvelope(marker: "primary", signatureByte: 0x31)
        let restored = try KagemushaDeviceAttestationSignedTransaction(
            canonicalNorito: fixture.envelope.norito,
            expectedRegistrationId: fixture.registration.canonicalRegistrationId,
            expectedNetworkId: Self.networkId,
            expectedAuthority: Self.authority,
            expectedTransactionHash: fixture.envelope.transactionHash
        )

        XCTAssertEqual(restored.envelope.norito, fixture.envelope.norito)
        XCTAssertEqual(restored.envelope.signedTransaction, fixture.envelope.signedTransaction)
        XCTAssertEqual(restored.envelope.transactionHash, fixture.envelope.transactionHash)
        XCTAssertEqual(restored.envelope.hashHex, fixture.envelope.hashHex)
        XCTAssertEqual(restored.registrationId, fixture.registration.canonicalRegistrationId)
        XCTAssertEqual(restored.networkId, Self.networkId)
        XCTAssertEqual(restored.authority, Self.authority)
        XCTAssertNoThrow(try restored.validateStatusTransactionHash(fixture.envelope.hashHex))
    }

    func testRehydratesTransactionWithExactTairaSponsorProgram() throws {
        let sponsor = try FeeSponsorProgramId(
            "testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A/cbsi_web"
        )
        let fixture = try makeEnvelope(
            marker: "taira-sponsor",
            signatureByte: 0x35,
            feePayment: .sponsor(
                programId: sponsor,
                programRevision: 1,
                chargeLimits: [],
                gasLimit: nil
            )
        )

        XCTAssertNoThrow(
            try KagemushaDeviceAttestationSignedTransaction(
                canonicalNorito: fixture.envelope.norito,
                expectedRegistrationId: fixture.registration.canonicalRegistrationId,
                expectedNetworkId: Self.networkId,
                expectedAuthority: Self.authority,
                expectedTransactionHash: fixture.envelope.transactionHash
            )
        )
    }

    func testRejectsWireVersionTrailingAndStatusHashSubstitution() throws {
        let fixture = try makeEnvelope(marker: "wire", signatureByte: 0x32)

        var wrongVersion = fixture.envelope.norito
        wrongVersion[wrongVersion.startIndex] = 2
        XCTAssertThrowsError(
            try KagemushaDeviceAttestationSignedTransaction(
                canonicalNorito: wrongVersion,
                expectedRegistrationId: fixture.registration.canonicalRegistrationId,
                expectedNetworkId: Self.networkId
            )
        )

        var trailing = fixture.envelope.norito
        trailing.append(0)
        XCTAssertThrowsError(
            try KagemushaDeviceAttestationSignedTransaction(
                canonicalNorito: trailing,
                expectedRegistrationId: fixture.registration.canonicalRegistrationId,
                expectedNetworkId: Self.networkId
            )
        )

        let restored = try KagemushaDeviceAttestationSignedTransaction(
            canonicalNorito: fixture.envelope.norito,
            expectedRegistrationId: fixture.registration.canonicalRegistrationId,
            expectedNetworkId: Self.networkId
        )
        var substitutedStatus = fixture.envelope.hashHex
        substitutedStatus.replaceSubrange(
            substitutedStatus.startIndex...substitutedStatus.startIndex,
            with: substitutedStatus.first == "0" ? "1" : "0"
        )
        XCTAssertThrowsError(try restored.validateStatusTransactionHash(substitutedStatus))
        XCTAssertThrowsError(
            try restored.validateStatusTransactionHash(fixture.envelope.hashHex.uppercased())
        )
    }

    func testRejectsRegistrationChainAuthorityHashAndEnvelopeSubstitution() throws {
        let fixture = try makeEnvelope(marker: "expected", signatureByte: 0x33)
        let substitutedRegistration = try makeEnvelope(
            marker: "substituted",
            signatureByte: 0x33
        )
        XCTAssertThrowsError(
            try KagemushaDeviceAttestationSignedTransaction(
                canonicalNorito: substitutedRegistration.envelope.norito,
                expectedRegistrationId: fixture.registration.canonicalRegistrationId,
                expectedNetworkId: Self.networkId
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaDeviceAttestationSignedTransactionError,
                .registrationIdMismatch
            )
        }

        XCTAssertThrowsError(
            try KagemushaDeviceAttestationSignedTransaction(
                canonicalNorito: fixture.envelope.norito,
                expectedRegistrationId: fixture.registration.canonicalRegistrationId,
                expectedNetworkId: TestNetworkIds.other
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaDeviceAttestationSignedTransactionError,
                .networkIdMismatch
            )
        }
        XCTAssertThrowsError(
            try KagemushaDeviceAttestationSignedTransaction(
                canonicalNorito: fixture.envelope.norito,
                expectedRegistrationId: fixture.registration.canonicalRegistrationId,
                expectedNetworkId: Self.networkId,
                expectedAuthority: try alternateAuthority()
            )
        )

        var wrongHash = fixture.envelope.transactionHash
        wrongHash[wrongHash.startIndex] ^= 0x80
        XCTAssertThrowsError(
            try KagemushaDeviceAttestationSignedTransaction(
                canonicalNorito: fixture.envelope.norito,
                expectedRegistrationId: fixture.registration.canonicalRegistrationId,
                expectedNetworkId: Self.networkId,
                expectedTransactionHash: wrongHash
            )
        )

        let substitutedSignature = try makeEnvelope(
            marker: "expected",
            signatureByte: 0x34
        )
        XCTAssertEqual(
            substitutedSignature.envelope.transactionHash,
            fixture.envelope.transactionHash
        )
        XCTAssertNotEqual(
            substitutedSignature.envelope.norito,
            fixture.envelope.norito
        )

        let substitutedEnvelope = try makeEnvelope(
            marker: "expected",
            signatureByte: 0x34,
            nonce: 8
        )
        XCTAssertEqual(
            substitutedEnvelope.registration.canonicalRegistrationId,
            fixture.registration.canonicalRegistrationId
        )
        XCTAssertNotEqual(
            substitutedEnvelope.envelope.transactionHash,
            fixture.envelope.transactionHash
        )
        XCTAssertThrowsError(
            try KagemushaDeviceAttestationSignedTransaction(
                canonicalNorito: substitutedEnvelope.envelope.norito,
                expectedRegistrationId: fixture.registration.canonicalRegistrationId,
                expectedNetworkId: Self.networkId,
                expectedTransactionHash: fixture.envelope.transactionHash
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaDeviceAttestationSignedTransactionError,
                .transactionHashMismatch
            )
        }
    }

    private func makeEnvelope(
        marker: String,
        signatureByte: UInt8,
        feePayment: FeePaymentIntent = .authority(chargeLimits: [], gasLimit: nil),
        nonce: UInt32 = 7
    ) throws -> (
        registration: KagemushaDeviceAttestationRegistration,
        envelope: SignedTransactionEnvelope
    ) {
        let registration = try makeRegistration(marker: marker)
        let sdk = IrohaSDK(
            baseURL: try XCTUnwrap(URL(string: "https://example.invalid")),
            creationTimeProvider: { 1_717_171_717_000 }
        )
        let unsigned = try sdk.buildUnsignedRegisterKagemushaDeviceAttestation(
            request: RegisterKagemushaDeviceAttestationRequest(
                networkId: Self.networkId,
                authority: Self.authority,
                registration: registration,
                feePayment: feePayment,
                ttlMs: 60_000,
                nonce: nonce,
                metadata: ["purpose": .string("crash-safe-replay")]
            )
        )
        let envelope = try unsigned.signed(
            signature: Data(repeating: signatureByte, count: 64)
        )
        return (registration, envelope)
    }

    private func makeRegistration(
        marker: String
    ) throws -> KagemushaDeviceAttestationRegistration {
        let p256 = try XCTUnwrap(Data(hexString: Self.p256Generator))
        let deviceKey = try KagemushaDevicePublicKeyV2(sec1Bytes: p256)
        let report = Data("signed-transaction-test-report-\(marker)".utf8)
        return try KagemushaDeviceAttestationRegistration(
            platform: KagemushaDeviceAttestation.androidKeyMintPlatform,
            keyId: Data(SHA256.hash(data: p256)).hexLowercased(),
            deviceId: "signed-transaction-test-device",
            accountId: Self.authority,
            androidPackageName: "org.hyperledger.iroha.signed-transaction-test",
            androidSigningCertificateSha256: Data(
                SHA256.hash(data: Data("signed-transaction-test-certificate".utf8))
            ),
            publicKey: deviceKey,
            assertionScheme: KagemushaDeviceAttestation.androidKeyMintAssertionScheme,
            assertionKeyAlgorithm:
                KagemushaDeviceAttestation.androidKeyMintAssertionKeyAlgorithm,
            assertionPublicKey: p256,
            assertionUsageCountLimit: 1,
            attestationReport: report,
            recentBlockHeight: 77,
            recentBlockHash: IrohaHash.hash(Data("signed-transaction-test-block".utf8)),
            expiresAtMs: 2_000_000_000_000
        )
    }

    private func alternateAuthority() throws -> String {
        let key = Data(repeating: 0x55, count: 32)
        return try AccountAddress.fromAccount(publicKey: key).toI105(
            networkPrefix: SccpV1.tairaI105DiscriminantV1
        )
    }

    private static let networkId = TestNetworkIds.canonical
    private static let authority = try! AccountAddress
        .fromAccount(publicKey: Data(repeating: 0x54, count: 32))
        .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
    private static let p256Generator =
        "04"
        + "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
        + "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
}
