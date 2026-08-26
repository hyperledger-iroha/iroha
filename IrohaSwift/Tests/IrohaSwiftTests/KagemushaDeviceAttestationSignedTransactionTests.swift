import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaDeviceAttestationSignedTransactionTests: XCTestCase {
    @available(iOS 15.0, macOS 12.0, *)
    func testCompactV1EnvelopeSubmitsThroughStrictTorii202Path() async throws {
        let fixture = try makeEnvelope(marker: "strict-submit")
        KagemushaSubmissionURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/node/capabilities":
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!
                let body = Data(
                    """
                    {"abi_version":1,"data_model_version":4,"signed_transaction_schema_hash_hex":"7ab5ff9c572efb316deac478f19209c5"}
                    """.utf8
                )
                return (response, body)
            case "/v1/pipeline/transactions":
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(
                    request.value(forHTTPHeaderField: "Content-Type"),
                    "application/x-norito"
                )
                XCTAssertEqual(request.httpBody, fixture.envelope.norito)
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 202,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!
                return (response, Data())
            default:
                XCTFail("unexpected request: \(request.url?.path ?? "")")
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 404,
                    httpVersion: nil,
                    headerFields: nil
                )!
                return (response, Data())
            }
        }
        defer { KagemushaSubmissionURLProtocol.handler = nil }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [KagemushaSubmissionURLProtocol.self]
        let sdk = IrohaSDK(
            baseURL: try XCTUnwrap(URL(string: "https://example.invalid")),
            session: URLSession(configuration: configuration),
            creationTimeProvider: { 1_717_171_717_000 }
        )

        try await sdk.submit(envelope: fixture.envelope)
    }

    func testRehydratesExactEnvelopeAndRecomputesHashes() throws {
        let fixture = try makeEnvelope(marker: "primary")
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

    func testRejectsDetachedSignatureFromDifferentAuthority() throws {
        let registration = try makeRegistration(marker: "wrong-signer")
        let sdk = IrohaSDK(
            baseURL: try XCTUnwrap(URL(string: "https://example.invalid")),
            creationTimeProvider: { 1_717_171_717_000 }
        )
        let unsigned = try sdk.buildUnsignedRegisterKagemushaDeviceAttestation(
            request: RegisterKagemushaDeviceAttestationRequest(
                networkId: Self.networkId,
                authority: Self.authority,
                registration: registration,
                feePayment: .authority(chargeLimits: [], gasLimit: nil),
                ttlMs: 60_000,
                nonce: 9
            )
        )
        let wrongKey = try Curve25519.Signing.PrivateKey(
            rawRepresentation: Data(repeating: 0x53, count: 32)
        )
        let wrongSignature = try wrongKey.signature(for: unsigned.signingHash)

        XCTAssertThrowsError(try unsigned.signed(signature: wrongSignature))
    }

    func testRejectsNonEd25519AuthorityBeforeBuildingUnsignedTransaction() throws {
        let secp256k1Generator = try XCTUnwrap(
            Data(
                hexString:
                    "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
            )
        )
        let unsupportedAuthority = try AccountAddress
            .fromAccount(publicKey: secp256k1Generator, algorithm: "secp256k1")
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let sdk = IrohaSDK(
            baseURL: try XCTUnwrap(URL(string: "https://example.invalid")),
            creationTimeProvider: { 1_717_171_717_000 }
        )

        XCTAssertThrowsError(
            try sdk.buildUnsignedRegisterKagemushaDeviceAttestation(
                request: RegisterKagemushaDeviceAttestationRequest(
                    networkId: Self.networkId,
                    authority: unsupportedAuthority,
                    registration: makeRegistration(marker: "unsupported-authority"),
                    feePayment: .authority(chargeLimits: [], gasLimit: nil),
                    ttlMs: 60_000,
                    nonce: 10
                )
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaDeviceAttestationError,
                .nonCanonicalField(field: "authority_ed25519_single_key")
            )
        }
    }

    func testRejectsZeroTransactionScalarsBeforeBuildingUnsignedTransaction() throws {
        let registration = try makeRegistration(marker: "zero-scalars")
        let cases: [(field: String, creationTimeMs: UInt64, ttlMs: UInt64, nonce: UInt32?)] = [
            ("creation_time_ms", 0, 60_000, 1),
            ("ttl_ms", 1_717_171_717_000, 0, 1),
            ("nonce", 1_717_171_717_000, 60_000, 0),
        ]
        for testCase in cases {
            let sdk = IrohaSDK(
                baseURL: try XCTUnwrap(URL(string: "https://example.invalid")),
                creationTimeProvider: { testCase.creationTimeMs }
            )
            XCTAssertThrowsError(
                try sdk.buildUnsignedRegisterKagemushaDeviceAttestation(
                    request: RegisterKagemushaDeviceAttestationRequest(
                        networkId: Self.networkId,
                        authority: Self.authority,
                        registration: registration,
                        feePayment: .authority(chargeLimits: [], gasLimit: nil),
                        ttlMs: testCase.ttlMs,
                        nonce: testCase.nonce
                    )
                )
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaDeviceAttestationError,
                    .nonCanonicalField(field: testCase.field)
                )
            }
        }
    }

    func testRequiredTransactionTtlIsEncodedAsNonzeroSome() throws {
        let fixture = try makeEnvelope(marker: "required-ttl")
        var signed = CanonicalNoritoReader(data: Data(fixture.envelope.norito.dropFirst()))
        _ = try signed.readCompactField()
        let transactionPayload = try signed.readCompactField()
        _ = try signed.readCompactField()
        XCTAssertEqual(signed.remaining(), 0)

        var payload = CanonicalNoritoReader(data: transactionPayload)
        var fields = [Data]()
        for _ in 0..<10 {
            fields.append(try payload.readCompactField())
        }
        XCTAssertEqual(payload.remaining(), 0)
        var ttl = CanonicalNoritoReader(data: fields[4])
        XCTAssertEqual(try ttl.readUInt8(), 1)
        var value = CanonicalNoritoReader(data: try ttl.readCompactField())
        XCTAssertEqual(try value.readUInt64LE(), 60_000)
        XCTAssertEqual(value.remaining(), 0)
        XCTAssertEqual(ttl.remaining(), 0)
        XCTAssertNotNil(NoritoNativeBridge.shared.decodeSignedTransaction(fixture.envelope.norito))
    }

    func testRejectsNoncanonicalMetadataBeforeBuildingUnsignedTransaction() throws {
        let sdk = IrohaSDK(
            baseURL: try XCTUnwrap(URL(string: "https://example.invalid")),
            creationTimeProvider: { 1_717_171_717_000 }
        )
        XCTAssertThrowsError(
            try sdk.buildUnsignedRegisterKagemushaDeviceAttestation(
                request: RegisterKagemushaDeviceAttestationRequest(
                    networkId: Self.networkId,
                    authority: Self.authority,
                    registration: makeRegistration(marker: "invalid-metadata"),
                    feePayment: .authority(chargeLimits: [], gasLimit: nil),
                    ttlMs: 60_000,
                    nonce: 11,
                    metadata: ["": .string("empty keys are not canonical")]
                )
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaDeviceAttestationError,
                .nonCanonicalField(field: "transaction_payload_v1")
            )
        }
    }

    func testUsesCurrentInstructionWireIdAndConcreteFrameSchema() throws {
        let fixture = try makeEnvelope(marker: "wire-id")
        var signed = CanonicalNoritoReader(data: Data(fixture.envelope.norito.dropFirst()))
        _ = try signed.readCompactField()
        let transactionPayload = try signed.readCompactField()
        _ = try signed.readCompactField()
        XCTAssertEqual(signed.remaining(), 0)

        var payload = CanonicalNoritoReader(data: transactionPayload)
        _ = try payload.readCompactField()
        _ = try payload.readCompactField()
        _ = try payload.readCompactField()
        let executableBytes = try payload.readCompactField()
        var executable = CanonicalNoritoReader(data: executableBytes)
        XCTAssertEqual(try executable.readUInt32LE(), 0)
        let instructionBytes = try executable.readCompactField()
        XCTAssertEqual(executable.remaining(), 0)

        var instructions = CanonicalNoritoReader(data: instructionBytes)
        XCTAssertEqual(try instructions.readUInt64LE(), 1)
        let instructionBytesV1 = try instructions.readCompactField()
        XCTAssertEqual(instructions.remaining(), 0)
        var instruction = CanonicalNoritoReader(data: instructionBytesV1)
        let wireId = try ToriiCanonicalTransactionDraft.decodeString(
            instruction.readCompactField(),
            field: "device attestation wire id"
        )
        XCTAssertEqual(wireId, "iroha.offline.device_attestation.register")
        var archive = CanonicalNoritoReader(data: try instruction.readCompactField())
        let archiveLength = try archive.readUInt64LE()
        let framed = try archive.readBytes(Int(archiveLength))
        XCTAssertEqual(archive.remaining(), 0)
        XCTAssertEqual(instruction.remaining(), 0)
        let frame = try XCTUnwrap(noritoDecodeFrame(framed))
        XCTAssertEqual(
            frame.header.schema,
            noritoSchemaHash(
                forTypeName: "iroha_data_model::isi::offline::RegisterOfflineDeviceAttestation"
            )
        )
        XCTAssertEqual(frame.header.flags, NoritoHeader.compactLen)
    }

    func testRejectsSignedCanonicalFrameWithMalformedRegistrationBody() throws {
        let fixture = try makeEnvelope(marker: "malformed-inner")
        let malformedRegistration = Data([0])
        let malformedWire = try replacingRegistration(
            in: fixture.envelope.norito,
            with: malformedRegistration
        )
        let malformedRegistrationArchive = noritoEncode(
            typeName: KagemushaDeviceAttestationTypeNames.deviceAttestationRegistration,
            payload: malformedRegistration,
            flags: NoritoHeader.compactLen
        )

        XCTAssertNil(NoritoNativeBridge.shared.decodeSignedTransaction(malformedWire))
        XCTAssertThrowsError(
            try KagemushaDeviceAttestationSignedTransaction(
                canonicalNorito: malformedWire,
                expectedRegistrationId: IrohaHash.hash(malformedRegistrationArchive),
                expectedNetworkId: Self.networkId,
                expectedAuthority: Self.authority
            )
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRejectsRetiredOuterInstructionTypeNameBeforeDispatch() async throws {
        let fixture = try makeEnvelope(marker: "retired-outer-type-name")
        let retired = try replacingInstructionWireId(
            in: fixture.envelope.norito,
            with: "iroha_data_model::isi::offline::RegisterOfflineDeviceAttestation"
        )
        XCTAssertNil(NoritoNativeBridge.shared.decodeSignedTransaction(retired))

        var requestCount = 0
        KagemushaSubmissionURLProtocol.handler = { request in
            requestCount += 1
            XCTFail("retired outer instruction name reached HTTP dispatch")
            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 500,
                    httpVersion: nil,
                    headerFields: nil
                )!,
                Data()
            )
        }
        defer { KagemushaSubmissionURLProtocol.handler = nil }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [KagemushaSubmissionURLProtocol.self]
        let sdk = IrohaSDK(
            baseURL: try XCTUnwrap(URL(string: "https://example.invalid")),
            session: URLSession(configuration: configuration),
            creationTimeProvider: { 1_717_171_717_000 }
        )
        let retiredEnvelope = SignedTransactionEnvelope(
            norito: retired,
            signedTransaction: Data(retired.dropFirst()),
            payload: nil,
            transactionHash: fixture.envelope.transactionHash
        )

        do {
            try await sdk.submit(envelope: retiredEnvelope)
            XCTFail("retired outer instruction name was accepted")
        } catch {
            XCTAssertEqual(requestCount, 0)
        }
    }

    func testRejectsWireVersionTrailingAndStatusHashSubstitution() throws {
        let fixture = try makeEnvelope(marker: "wire")

        var wrongVersion = fixture.envelope.norito
        wrongVersion[wrongVersion.startIndex] = 2
        XCTAssertThrowsError(
            try KagemushaDeviceAttestationSignedTransaction(
                canonicalNorito: wrongVersion,
                expectedRegistrationId: fixture.registration.canonicalRegistrationId,
                expectedNetworkId: Self.networkId,
                expectedAuthority: Self.authority
            )
        )

        var trailing = fixture.envelope.norito
        trailing.append(0)
        XCTAssertThrowsError(
            try KagemushaDeviceAttestationSignedTransaction(
                canonicalNorito: trailing,
                expectedRegistrationId: fixture.registration.canonicalRegistrationId,
                expectedNetworkId: Self.networkId,
                expectedAuthority: Self.authority
            )
        )

        let restored = try KagemushaDeviceAttestationSignedTransaction(
            canonicalNorito: fixture.envelope.norito,
            expectedRegistrationId: fixture.registration.canonicalRegistrationId,
            expectedNetworkId: Self.networkId,
            expectedAuthority: Self.authority
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
        let fixture = try makeEnvelope(marker: "expected")
        let substitutedRegistration = try makeEnvelope(
            marker: "substituted"
        )
        XCTAssertThrowsError(
            try KagemushaDeviceAttestationSignedTransaction(
                canonicalNorito: substitutedRegistration.envelope.norito,
                expectedRegistrationId: fixture.registration.canonicalRegistrationId,
                expectedNetworkId: Self.networkId,
                expectedAuthority: Self.authority
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
                expectedNetworkId: TestNetworkIds.other,
                expectedAuthority: Self.authority
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
                expectedAuthority: Self.authority,
                expectedTransactionHash: wrongHash
            )
        )

        var substitutedSignedBytes = fixture.envelope.norito
        substitutedSignedBytes[substitutedSignedBytes.startIndex + 12] ^= 0x01
        XCTAssertThrowsError(
            try KagemushaDeviceAttestationSignedTransaction(
                canonicalNorito: substitutedSignedBytes,
                expectedRegistrationId: fixture.registration.canonicalRegistrationId,
                expectedNetworkId: Self.networkId,
                expectedAuthority: Self.authority
            )
        )

        let substitutedEnvelope = try makeEnvelope(
            marker: "expected",
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
                expectedAuthority: Self.authority,
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
        let signature = try Self.signingKey.signature(for: unsigned.signingHash)
        let envelope = try unsigned.signed(signature: signature)
        XCTAssertNotNil(NoritoNativeBridge.shared.decodeSignedTransaction(envelope.norito))
        return (registration, envelope)
    }

    private func replacingRegistration(
        in versionedSignedTransaction: Data,
        with registration: Data
    ) throws -> Data {
        var concrete = CompactNoritoWriter()
        concrete.writeField(registration)
        let framed = noritoEncode(
            typeName: "iroha_data_model::isi::offline::RegisterOfflineDeviceAttestation",
            payload: concrete.data,
            flags: NoritoHeader.compactLen
        )
        return try rewritingInstruction(
            in: versionedSignedTransaction,
            replacementWireId: nil,
            replacementFramedArchive: CompactNorito.encodeBytesVec(framed)
        )
    }

    private func replacingInstructionWireId(
        in versionedSignedTransaction: Data,
        with wireId: String
    ) throws -> Data {
        try rewritingInstruction(
            in: versionedSignedTransaction,
            replacementWireId: CompactNorito.encodeString(wireId),
            replacementFramedArchive: nil
        )
    }

    private func rewritingInstruction(
        in versionedSignedTransaction: Data,
        replacementWireId: Data?,
        replacementFramedArchive: Data?
    ) throws -> Data {
        var signed = CanonicalNoritoReader(
            data: Data(versionedSignedTransaction.dropFirst())
        )
        _ = try signed.readCompactField()
        let transactionPayload = try signed.readCompactField()
        _ = try signed.readCompactField()
        guard signed.remaining() == 0 else {
            throw KagemushaDeviceAttestationSignedTransactionError
                .invalidCanonicalNorito("signed transaction")
        }

        var payload = CanonicalNoritoReader(data: transactionPayload)
        var fields = [Data]()
        fields.reserveCapacity(10)
        for _ in 0..<10 {
            fields.append(try payload.readCompactField())
        }
        guard payload.remaining() == 0 else {
            throw KagemushaDeviceAttestationSignedTransactionError
                .invalidCanonicalNorito("transaction payload")
        }

        var executable = CanonicalNoritoReader(data: fields[3])
        guard try executable.readUInt32LE() == 0 else {
            throw KagemushaDeviceAttestationSignedTransactionError
                .invalidCanonicalNorito("executable")
        }
        let instructionSequence = try executable.readCompactField()
        guard executable.remaining() == 0 else {
            throw KagemushaDeviceAttestationSignedTransactionError
                .invalidCanonicalNorito("executable")
        }
        var instructions = CanonicalNoritoReader(data: instructionSequence)
        guard try instructions.readUInt64LE() == 1 else {
            throw KagemushaDeviceAttestationSignedTransactionError
                .invalidCanonicalNorito("instruction count")
        }
        var instruction = CanonicalNoritoReader(
            data: try instructions.readCompactField()
        )
        guard instructions.remaining() == 0 else {
            throw KagemushaDeviceAttestationSignedTransactionError
                .invalidCanonicalNorito("instructions")
        }
        let wireId = try instruction.readCompactField()
        let framedArchive = try instruction.readCompactField()
        guard instruction.remaining() == 0 else {
            throw KagemushaDeviceAttestationSignedTransactionError
                .invalidCanonicalNorito("instruction")
        }

        var replacementInstruction = CompactNoritoWriter()
        replacementInstruction.writeField(replacementWireId ?? wireId)
        replacementInstruction.writeField(replacementFramedArchive ?? framedArchive)
        var replacementInstructions = CompactNoritoWriter()
        replacementInstructions.writeUInt64LE(1)
        replacementInstructions.writeField(replacementInstruction.data)
        var replacementExecutable = CompactNoritoWriter()
        replacementExecutable.writeUInt32LE(0)
        replacementExecutable.writeField(replacementInstructions.data)
        fields[3] = replacementExecutable.data

        var replacementPayload = CompactNoritoWriter()
        for field in fields {
            replacementPayload.writeField(field)
        }
        let signature = try Self.signingKey.signature(
            for: IrohaHash.hash(replacementPayload.data)
        )
        var transactionSignature = CompactNoritoWriter()
        transactionSignature.writeField(CompactNorito.encodeConstVec(signature))
        var replacementSigned = CompactNoritoWriter()
        replacementSigned.writeField(transactionSignature.data)
        replacementSigned.writeField(replacementPayload.data)
        replacementSigned.writeField(Data([0]))
        var replacementVersioned = Data([1])
        replacementVersioned.append(replacementSigned.data)
        return replacementVersioned
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
    private static let signingKey = try! Curve25519.Signing.PrivateKey(
        rawRepresentation: Data(repeating: 0x54, count: 32)
    )
    private static let authority = try! AccountAddress
        .fromAccount(publicKey: signingKey.publicKey.rawRepresentation)
        .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
    private static let p256Generator =
        "04"
        + "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
        + "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
}

private final class KagemushaSubmissionURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data?))?

    override class func canInit(with request: URLRequest) -> Bool { true }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(
                self,
                didFailWithError: NSError(domain: "KagemushaSubmissionURLProtocol", code: -1)
            )
            return
        }
        do {
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            if let data {
                client?.urlProtocol(self, didLoad: data)
            }
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}
