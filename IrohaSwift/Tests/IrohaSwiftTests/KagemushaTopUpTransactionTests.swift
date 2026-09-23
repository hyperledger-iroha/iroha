import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Transaction authoring tests only. The reused protocol fixture has structural
/// proofs; these cases do not assert monetary proof or reserve-finality validity.
final class KagemushaTopUpTransactionTests: XCTestCase {
    private let creationTimeMs: UInt64 = 1_717_000_222_000
    private let ttlMs: UInt64 = 60_000

    func testTopUpUsesOneExactNativeInstructionAndAuthenticatesEveryField() throws {
        let key = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        let authority = try AccountId.makeI105(publicKey: key.publicKey())
        let request = try kagemushaTopUpRequest(payer: authority)
        let network = try NetworkId(bytes: request.networkID)
        let fees = FeePaymentIntent.authority(chargeLimits: [], gasLimit: 2_000)
        let requestBefore = try KagemushaNoritoV1.encodeTopUpRequestShape(request)
        let envelope = try makeSDK().buildSignedKagemushaTopUp(
            request: request, networkId: network, authority: authority,
            creationTimeMs: creationTimeMs, feePayment: fees, ttlMs: ttlMs, signingKey: key)
        let (signature, payload, fields) = try signedParts(envelope)

        var domain = CanonicalNoritoReader(data: fields[0])
        XCTAssertEqual(try domain.readUInt32LE(), 0)
        XCTAssertEqual(try domain.readCompactField(), network.bytes)
        XCTAssertEqual(domain.remaining(), 0)
        XCTAssertEqual(fields[1], request.payer.canonicalPayload)
        XCTAssertEqual(fields[1], try CanonicalNorito.encodeCompactAccountId(authority))
        XCTAssertEqual(fields[2], CompactNorito.encodeUInt64(creationTimeMs))

        var executable = CanonicalNoritoReader(data: fields[3])
        XCTAssertEqual(try executable.readUInt32LE(), 0, "Top-up ingress requires Instructions, never Batch")
        var instructions = CanonicalNoritoReader(data: try executable.readCompactField())
        XCTAssertEqual(executable.remaining(), 0)
        XCTAssertEqual(try instructions.readUInt64LE(), 1)
        var instruction = CanonicalNoritoReader(data: try instructions.readCompactField())
        XCTAssertEqual(instructions.remaining(), 0)
        XCTAssertEqual(try instruction.readCompactField(), CompactNorito.encodeString("iroha.kagemusha.v1.top_up"))
        var frameVector = CanonicalNoritoReader(data: try instruction.readCompactField())
        XCTAssertEqual(instruction.remaining(), 0)
        let expectedFrame = try KagemushaNoritoV1.topUpInstructionFrame(request)
        XCTAssertEqual(try frameVector.readUInt64LE(), UInt64(expectedFrame.framedPayload.count))
        // Vec<u8> keeps its raw bytes after the fixed u64 element count.
        let frameBytes = try frameVector.readBytes(expectedFrame.framedPayload.count)
        XCTAssertEqual(frameVector.remaining(), 0)
        XCTAssertEqual(frameBytes, expectedFrame.framedPayload)
        let topUpArchive = try XCTUnwrap(noritoDecodeFrame(frameBytes))
        var topUpBody = CanonicalNoritoReader(data: topUpArchive.payload)
        let requestPayload = try topUpBody.readCompactField()
        XCTAssertEqual(topUpBody.remaining(), 0)
        XCTAssertEqual(requestPayload, try XCTUnwrap(noritoDecodeFrame(requestBefore)).payload)
        XCTAssertEqual(try KagemushaNoritoV1.encodeTopUpRequestShape(request), requestBefore)

        XCTAssertEqual(fields[4], try CompactNorito.encodeOption(ttlMs, encode: CompactNorito.encodeUInt64))
        XCTAssertEqual(fields[5], Data([0]), "No caller-invented nonce")
        XCTAssertEqual(fields[6], try fees.compactNorito())
        var admission = CanonicalNoritoReader(data: fields[7])
        XCTAssertEqual(try admission.readUInt32LE(), TransactionAdmissionIntentV1.queuePlanSynced.rawValue)
        XCTAssertEqual(admission.remaining(), 0)
        var metadata = CanonicalNoritoReader(data: fields[8])
        XCTAssertEqual(try metadata.readUInt64LE(), 0)
        XCTAssertEqual(metadata.remaining(), 0)
        XCTAssertEqual(fields[9], Data([0]))

        let publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: key.publicKey())
        XCTAssertTrue(publicKey.isValidSignature(signature, for: IrohaHash.hash(payload)))
        var altered = fields
        altered[6] = try FeePaymentIntent.authority(chargeLimits: [], gasLimit: 2_001).compactNorito()
        XCTAssertFalse(publicKey.isValidSignature(signature, for: IrohaHash.hash(payloadBytes(altered))))
        altered = fields
        var tamperedExecutable = altered[3]
        let lastByte = tamperedExecutable.index(before: tamperedExecutable.endIndex)
        tamperedExecutable[lastByte] ^= 1
        altered[3] = tamperedExecutable
        XCTAssertFalse(publicKey.isValidSignature(signature, for: IrohaHash.hash(payloadBytes(altered))))
        XCTAssertEqual(envelope.norito, Data([1]) + envelope.signedTransaction)
        XCTAssertCanonicalExternalEntrypointHash(envelope)
    }

    func testExplicitCreationTimePreservesPayloadAndRetryReplaysPersistedEnvelope() throws {
        let key = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        let authority = try AccountId.makeI105(publicKey: key.publicKey())
        let request = try kagemushaTopUpRequest(payer: authority)
        let first = try build(request, authority: authority, signingKey: key)
        let rebuilt = try build(request, authority: authority, signingKey: key)
        let (firstSignature, firstPayload, _) = try signedParts(first)
        let (rebuiltSignature, rebuiltPayload, _) = try signedParts(rebuilt)
        XCTAssertEqual(firstPayload, rebuiltPayload)
        XCTAssertEqual(first.transactionHash, rebuilt.transactionHash)
        let publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: key.publicKey())
        XCTAssertTrue(publicKey.isValidSignature(firstSignature, for: IrohaHash.hash(firstPayload)))
        XCTAssertTrue(publicKey.isValidSignature(rebuiltSignature, for: IrohaHash.hash(rebuiltPayload)))

        // Signing again is not the replay contract: retain the original signed
        // bytes before submission and load those same bytes for every retry.
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("KagemushaTopUpTransactionTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let persisted = directory.appendingPathComponent("signed-top-up.norito")
        try first.norito.write(to: persisted, options: .atomic)
        let firstReplay = try Data(contentsOf: persisted)
        let secondReplay = try Data(contentsOf: persisted)
        XCTAssertEqual(firstReplay, first.norito)
        XCTAssertEqual(secondReplay, firstReplay)
        XCTAssertEqual(Data(firstReplay.dropFirst()), first.signedTransaction)
    }

    func testNativeIngressAcceptsOnlyTheVersionedExactSignedTopUp() throws {
        let key = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        let authority = try AccountId.makeI105(publicKey: key.publicKey())
        let request = try kagemushaTopUpRequest(payer: authority)
        let envelope = try build(request, authority: authority, signingKey: key)
        let prepared: KagemushaPreparedTopUpSubmissionV1
        do {
            prepared = try KagemushaPreparedTopUpSubmissionV1(
                signedTransaction: envelope.norito, expectedRequest: request)
        } catch KagemushaTopUpSubmissionErrorV1.bridgeUnavailable {
            throw XCTSkip("A same-source native bridge is required for ingress validation")
        }
        XCTAssertEqual(prepared.signedTransactionBytes, envelope.norito)
        XCTAssertEqual(prepared.canonicalRequestBytes,
            try KagemushaNoritoV1.encodeTopUpRequestShape(request))

        XCTAssertThrowsError(try KagemushaPreparedTopUpSubmissionV1(
            signedTransaction: envelope.signedTransaction, expectedRequest: request)) {
            XCTAssertEqual($0 as? KagemushaTopUpSubmissionErrorV1, .requestMismatch)
        }
        var tampered = envelope.norito
        tampered[tampered.index(before: tampered.endIndex)] ^= 1
        XCTAssertThrowsError(try KagemushaPreparedTopUpSubmissionV1(
            signedTransaction: tampered, expectedRequest: request)) {
            XCTAssertEqual($0 as? KagemushaTopUpSubmissionErrorV1, .requestMismatch)
        }
    }

    func testRejectsNetworkOutsideCallerExpectedScope() throws {
        let key = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        let authority = try AccountId.makeI105(publicKey: key.publicKey())
        let request = try kagemushaTopUpRequest(payer: authority)
        var otherBytes = request.networkID
        otherBytes[0] ^= 1
        XCTAssertThrowsError(try makeSDK().buildSignedKagemushaTopUp(
            request: request, networkId: NetworkId(bytes: otherBytes), authority: authority,
            creationTimeMs: creationTimeMs, feePayment: .authority(chargeLimits: [], gasLimit: nil),
            ttlMs: ttlMs, signingKey: key)) {
            XCTAssertEqual($0 as? KagemushaTopUpTransactionInputError, .networkMismatch)
        }
    }

    func testRejectsPayerOutsideCallerAuthority() throws {
        let key = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        let other = try SigningKey.ed25519(privateKey: Data(repeating: 0x43, count: 32))
        let authority = try AccountId.makeI105(publicKey: key.publicKey())
        let request = try kagemushaTopUpRequest(payer: authority)
        XCTAssertThrowsError(try build(request, authority: AccountId.makeI105(publicKey: other.publicKey()), signingKey: other)) {
            XCTAssertEqual($0 as? KagemushaTopUpTransactionInputError, .payerMismatch)
        }
    }

    func testRejectsZeroTTL() throws {
        let key = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        let authority = try AccountId.makeI105(publicKey: key.publicKey())
        let request = try kagemushaTopUpRequest(payer: authority)
        XCTAssertThrowsError(try makeSDK().buildSignedKagemushaTopUp(
            request: request, networkId: NetworkId(bytes: request.networkID), authority: authority,
            creationTimeMs: creationTimeMs, feePayment: .authority(chargeLimits: [], gasLimit: nil),
            ttlMs: 0, signingKey: key)) {
            XCTAssertEqual($0 as? ExecutableBatchInputError, .zeroTimeToLive)
        }
    }

    func testRejectsRequestAmountDifferentFromHardwareAuthorization() throws {
        let key = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        let authority = try AccountId.makeI105(publicKey: key.publicKey())
        let request = try kagemushaTopUpRequest(payer: authority, requestAmount: KagemushaUInt128V1(41))
        XCTAssertThrowsError(try build(request, authority: authority, signingKey: key))
    }

    func testRejectsSigningKeyThatDoesNotControlPayer() throws {
        let payerKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        let wrongKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x43, count: 32))
        let authority = try AccountId.makeI105(publicKey: payerKey.publicKey())
        let request = try kagemushaTopUpRequest(payer: authority)
        let originalPayer = request.payer
        XCTAssertThrowsError(try build(request, authority: authority, signingKey: wrongKey)) {
            XCTAssertEqual($0 as? KagemushaTopUpTransactionInputError, .authorityKeyMismatch)
        }
        XCTAssertEqual(request.payer, originalPayer)
        XCTAssertEqual(request.payer, try KagemushaAccountIDV1(authority))
    }

    func testRejectsMultisigAuthorityWithoutSignatureBundle() throws {
        let key = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        let authority = try multisigAuthorityFixture()
        let address = try AccountAddress.parseEncoded(authority)
        XCTAssertNil(address.singleControllerInfo())
        let info = try XCTUnwrap(address.multisigPolicyInfo())
        let policyBuilder = MultisigPolicyBuilder()
            .setVersion(info.version)
            .setThreshold(info.threshold)
        for member in info.members {
            let algorithm = try XCTUnwrap(SigningAlgorithm.allCases.first { $0.wireName == member.algorithm })
            let publicKey = try XCTUnwrap(Data(hexString: String(member.publicKeyHex.dropFirst(2))))
            policyBuilder.addMember(algorithm: algorithm, weight: member.weight, publicKey: publicKey)
        }
        XCTAssertEqual(try policyBuilder.build().digestHex, info.digestBlake2b256Hex)
        let request = try kagemushaTopUpRequest(payer: authority)
        let originalPayer = request.payer
        XCTAssertThrowsError(try build(request, authority: authority, signingKey: key)) {
            XCTAssertEqual($0 as? KagemushaTopUpTransactionInputError, .unsupportedMultisigAuthority)
        }
        XCTAssertEqual(request.payer, originalPayer)
    }

    func testSM2ControllerBindingIncludesSigningMetadataDistid() throws {
        let fixture = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("fixtures/sm/sm2_fixture.json")
        let values = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: fixture)) as? [String: Any])
        let distid = try XCTUnwrap(values["distid"] as? String)
        let privateKey = try XCTUnwrap(Data(hexString: XCTUnwrap(values["private_key_hex"] as? String)))
        let publicKey = try XCTUnwrap(Data(hexString: XCTUnwrap(values["public_key_sec1_hex"] as? String)))
        let pair = try Sm2Keypair(distid: distid, privateKey: privateKey, publicKey: publicKey)
        let signingKey = SigningKey.sm2(pair)
        let address = try AccountAddress.fromAccount(
            publicKey: signingKey.publicKey(), algorithm: signingKey.algorithm.wireName, distid: distid)
        let controller = try XCTUnwrap(address.singleControllerInfo())
        XCTAssertEqual(controller.algorithm, .sm2)
        XCTAssertNotEqual(controller.publicKey, publicKey, "SM2 controller payload includes its DISTID")
        let authority = try address.toI105(networkPrefix: 753)
        let request = try kagemushaTopUpRequest(payer: authority)

        // Deliberately stop at TTL validation, after the real controller guards
        // and before signing. This checks SM2 binding without a native rebuild.
        XCTAssertThrowsError(try makeSDK().buildSignedKagemushaTopUp(
            request: request, networkId: NetworkId(bytes: request.networkID), authority: authority,
            creationTimeMs: creationTimeMs, feePayment: .authority(chargeLimits: [], gasLimit: nil),
            ttlMs: 0, signingKey: signingKey)) {
            XCTAssertEqual($0 as? ExecutableBatchInputError, .zeroTimeToLive)
        }
        var wrongDistid = signingKey
        wrongDistid.metadata.distid = distid + "-different"
        XCTAssertThrowsError(try makeSDK().buildSignedKagemushaTopUp(
            request: request, networkId: NetworkId(bytes: request.networkID), authority: authority,
            creationTimeMs: creationTimeMs, feePayment: .authority(chargeLimits: [], gasLimit: nil),
            ttlMs: 0, signingKey: wrongDistid)) {
            XCTAssertEqual($0 as? KagemushaTopUpTransactionInputError, .authorityKeyMismatch)
        }
        let wrongAlgorithm = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        XCTAssertThrowsError(try makeSDK().buildSignedKagemushaTopUp(
            request: request, networkId: NetworkId(bytes: request.networkID), authority: authority,
            creationTimeMs: creationTimeMs, feePayment: .authority(chargeLimits: [], gasLimit: nil),
            ttlMs: 0, signingKey: wrongAlgorithm)) {
            XCTAssertEqual($0 as? KagemushaTopUpTransactionInputError, .authorityKeyMismatch)
        }
    }

    private func multisigAuthorityFixture() throws -> String {
        let fixture = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("fixtures/account/address_vectors.json")
        let root = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: fixture)) as? [String: Any])
        let cases = try XCTUnwrap(root["cases"] as? [String: Any])
        let positive = try XCTUnwrap(cases["positive"] as? [[String: Any]])
        let vector = try XCTUnwrap(positive.first { $0["category"] as? String == "multisig" })
        let encodings = try XCTUnwrap(vector["encodings"] as? [String: Any])
        let i105 = try XCTUnwrap(encodings["i105"] as? [String: Any])
        return try XCTUnwrap(i105["string"] as? String)
    }

    private func makeSDK() -> IrohaSDK {
        IrohaSDK(baseURL: URL(string: "https://torii.example")!, creationTimeProvider: { 7 })
    }

    private func build(_ request: KagemushaTopUpRequestV1, authority: String,
                       signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try makeSDK().buildSignedKagemushaTopUp(
            request: request, networkId: NetworkId(bytes: request.networkID), authority: authority,
            creationTimeMs: creationTimeMs, feePayment: .authority(chargeLimits: [], gasLimit: nil),
            ttlMs: ttlMs, signingKey: signingKey)
    }

    private func signedParts(_ envelope: SignedTransactionEnvelope) throws -> (Data, Data, [Data]) {
        var signed = CanonicalNoritoReader(data: envelope.signedTransaction)
        var signatureSet = CanonicalNoritoReader(data: try signed.readCompactField())
        var encodedSignature = CanonicalNoritoReader(data: try signatureSet.readCompactField())
        XCTAssertEqual(signatureSet.remaining(), 0)
        XCTAssertEqual(try encodedSignature.readUInt64LE(), 64)
        var signature = Data()
        for _ in 0..<64 {
            let byte = try encodedSignature.readCompactField()
            XCTAssertEqual(byte.count, 1)
            signature.append(byte)
        }
        XCTAssertEqual(encodedSignature.remaining(), 0)
        let payload = try signed.readCompactField()
        XCTAssertEqual(try signed.readCompactField(), Data([0]))
        XCTAssertEqual(signed.remaining(), 0)
        var reader = CanonicalNoritoReader(data: payload)
        var fields = [Data]()
        for _ in 0..<10 { fields.append(try reader.readCompactField()) }
        XCTAssertEqual(reader.remaining(), 0)
        return (signature, payload, fields)
    }

    private func payloadBytes(_ fields: [Data]) -> Data {
        var writer = CompactNoritoWriter()
        fields.forEach { writer.writeField($0) }
        return writer.data
    }

    // Fixture helpers below follow ToriiClientTests' existing structural mint
    // fixture. The payer is explicitly bound to the test signing key; protocol
    // and encrypted-credit bytes come from fixtures/offline/kagemusha_v1.json.
    private func kagemushaTopUpRequest(
        payer: String, requestAmount: KagemushaUInt128V1? = nil
    ) throws -> KagemushaTopUpRequestV1 {
        let (request, payment) = try kagemushaPeerFixture()
        let operationID = kagemushaBytes(0x77)
        let issuanceCommitment = kagemushaBytes(0x7d)
        let creditID = kagemushaBytes(0x7e)
        let artifactManifestDigest = kagemushaBytes(0x79)
        let recipientCredentialCommitment = kagemushaBytes(0x7a)
        let creditCommitment = kagemushaBytes(0x7b)
        let encryptedCredit = payment.encryptedCredit
        let context = try KagemushaMintAuthorizationContextV1(
            operationID: operationID,
            releaseID: request.releaseID,
            suiteID: request.hardwareCredential.suiteID,
            vkDigest: kagemushaBytes(0x78),
            artifactManifestDigest: artifactManifestDigest,
            networkID: request.networkID,
            asset: request.asset,
            assetIncarnation: request.assetIncarnation,
            scale: request.scale,
            liabilityPoolID: request.liabilityPoolID,
            amount: KagemushaUInt128V1(40),
            payer: try KagemushaAccountIDV1(payer),
            recipient: request.recipient,
            hardwareCredentialID: request.hardwareCredential.credentialID,
            hardwareProfileID: request.hardwareCredential.hardwareProfileID,
            policyEpoch: request.hardwareCredential.policyEpoch,
            recipientCredentialCommitment: recipientCredentialCommitment,
            creditCommitment: creditCommitment,
            recipientOneTimeKey: KagemushaX25519PublicKeyV1(rawBytes: kagemushaBytes(0x7c))
        )
        let statement = try KagemushaMintAuthorizationStatementV1(
            context: context,
            issuanceCommitment: issuanceCommitment,
            creditID: creditID,
            ciphertextDigest: KagemushaNoritoV1.ciphertextDigestShape(encryptedCredit)
        )
        let authorization = try KagemushaMintAuthorizationV1(
            statement: statement,
            proof: try kagemushaProof(
                semanticDigest: KagemushaNoritoV1.mintAuthorizationStatementDigestShape(
                    statement),
                tag: 0x80)
        )
        return try KagemushaTopUpRequestV1(
            operationID: operationID,
            issuanceCommitment: issuanceCommitment,
            creditID: creditID,
            releaseID: context.releaseID,
            suiteID: context.suiteID,
            vkDigest: context.vkDigest,
            networkID: context.networkID,
            asset: context.asset,
            assetIncarnation: context.assetIncarnation,
            scale: context.scale,
            amount: requestAmount ?? context.amount,
            liabilityPoolID: context.liabilityPoolID,
            payer: context.payer,
            recipient: context.recipient,
            hardwareCredential: request.hardwareCredential,
            recipientCredentialCommitment: context.recipientCredentialCommitment,
            creditCommitment: context.creditCommitment,
            recipientOneTimeKey: context.recipientOneTimeKey,
            encryptedCredit: encryptedCredit,
            artifactManifestDigest: context.artifactManifestDigest,
            mintAuthorization: authorization
        )
    }

    private func kagemushaProof(
        semanticDigest: Data,
        tag: UInt8
    ) throws -> KagemushaPairedProofV1 {
        try KagemushaPairedProofV1(
            eqProtocolDigest: kagemushaBytes(tag),
            epProtocolDigest: kagemushaBytes(tag &+ 1),
            semanticDigest: semanticDigest,
            guardEqCredentialAudit: kagemushaBytes(tag &+ 2),
            guardEpCredentialAudit: kagemushaBytes(tag &+ 3),
            eqDeferredAudit: kagemushaBytes(tag &+ 4),
            epDeferredAudit: kagemushaBytes(tag &+ 5),
            eqProof: Data([tag]),
            epProof: Data([tag &+ 1]),
            eqHistory: Data(repeating: tag, count: KagemushaWireV1.historyAccumulatorBytes),
            epHistory: Data(
                repeating: tag &+ 1, count: KagemushaWireV1.historyAccumulatorBytes)
        )
    }

    private func kagemushaPeerFixture() throws
        -> (KagemushaPaymentRequestV1, KagemushaPaymentV1)
    {
        var current = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        while current.path != "/" {
            let candidate = current.appendingPathComponent("fixtures/offline/kagemusha_v1.json")
            if FileManager.default.fileExists(atPath: candidate.path) {
                let root = try XCTUnwrap(
                    JSONSerialization.jsonObject(with: Data(contentsOf: candidate))
                        as? [String: Any])
                let requestSection = try XCTUnwrap(root["payment_request"] as? [String: Any])
                let paymentSection = try XCTUnwrap(root["payment"] as? [String: Any])
                let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
                    try kagemushaFixtureHex(requestSection))
                let payment = try KagemushaNoritoV1.decodePaymentShapeExact(
                    try kagemushaFixtureHex(paymentSection), against: request)
                return (request, payment)
            }
            current.deleteLastPathComponent()
        }
        throw NSError(
            domain: "KagemushaTopUpTransactionTests", code: -1,
            userInfo: [NSLocalizedDescriptionKey: "KAGEMUSHA fixture was not found"])
    }

    private func kagemushaFixtureHex(_ section: [String: Any]) throws -> Data {
        let hex = try XCTUnwrap(section["norito_hex"] as? String)
        guard hex.count.isMultiple(of: 2) else {
            throw NSError(domain: "KagemushaTopUpTransactionTests", code: -1)
        }
        var result = Data()
        var index = hex.startIndex
        while index != hex.endIndex {
            let next = hex.index(index, offsetBy: 2)
            guard let byte = UInt8(hex[index..<next], radix: 16) else {
                throw NSError(domain: "KagemushaTopUpTransactionTests", code: -1)
            }
            result.append(byte)
            index = next
        }
        return result
    }

    private func kagemushaBytes(_ tag: UInt8) -> Data {
        Data(repeating: tag, count: 32)
    }

}
