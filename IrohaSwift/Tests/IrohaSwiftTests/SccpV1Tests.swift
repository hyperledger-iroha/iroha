import CryptoKit
import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import XCTest
@testable import IrohaSwift

private final class SccpStubURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        do {
            let (response, data) = try Self.handler!(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }
    override func stopLoading() {}
}

final class SccpV1Tests: XCTestCase {
    private static let registryMaxOutstandingLiability = "1000000000000"
    private static let evmRegistryMaxWrappedSupply = "1000000000000000000000"
    private static let tonRegistryMaxWrappedSupply = registryMaxOutstandingLiability

    private let hashHex = { (byte: UInt8) in "0x" + String(repeating: String(format: "%02x", byte), count: 32) }

    private func validEd25519PublicKey(seed: UInt8) throws -> Data {
        try Keypair(privateKeyBytes: Data(repeating: seed, count: 32)).publicKey
    }

    override func tearDown() {
        SccpStubURLProtocol.handler = nil
        super.tearDown()
    }

    func testClosedFirstReleaseInventoryHasNoRetiredProfilesOrCodecs() {
        XCTAssertEqual(SccpNetworkV1.allCases.map(\.rawValue), [
            "sora-taira", "ethereum-mainnet", "ethereum-sepolia",
            "bsc-mainnet", "bsc-testnet", "tron-mainnet", "tron-nile", "tron-shasta",
            "ton-mainnet", "ton-testnet",
        ])
        XCTAssertNil(SccpNetworkV1.fromTag(0))
        XCTAssertNil(SccpNetworkV1(rawValue: "sora-nexus"))
        XCTAssertNil(SccpNetworkV1(rawValue: "sora_nexus"))
        XCTAssertEqual(SccpCodecV1.allCases.map(\.rawValue), [1, 2, 5, 7])
        XCTAssertNil(SccpNetworkV1(rawValue: "solana-mainnet-beta"))
        XCTAssertEqual(SccpNetworkV1.tonMainnet.tag, 14)
        XCTAssertEqual(SccpNetworkV1.tonTestnet.tag, 15)
        XCTAssertEqual(SccpNetworkV1.tonMainnet.domainId, 4)
        XCTAssertNil(SccpCodecV1(rawValue: 3))
        XCTAssertNil(SccpCodecV1(rawValue: 4))
        XCTAssertNil(SccpCodecV1(rawValue: 6))
        XCTAssertEqual(SccpPayloadKindV1.allCases, [.transfer])
    }

    func testNativeTransferEventSharedVectors() throws {
        let vectors: [(SccpNetworkV1, String, String, String, String)] = [
            (
                .bscMainnet,
                "020102000000000000000700000000000000000000000103000000786f724d00000000000000000000000000000002140000001111111111111111111111111111111111111111010b000000616c696365407461697261010d00000074616972615f6273635f786f72",
                "e92d89d1adb34dbe5420fe660a0893f0edfd9493c3c683bdefabc89c24d0e1b7",
                "6aa2f80325682c6be5466ca2051b274d1e3a7da07ace3a21c31b4ac3a811f201",
                "0030b2d41f4da251b991659b871cde9e236fe654033d6204d9d6bae02266d3a5"
            ),
            (
                .tronMainnet,
                "020105000000000000000700000000000000000000000103000000786f724d0000000000000000000000000000000515000000412222222222222222222222222222222222222222010b000000616c696365407461697261010e00000074616972615f74726f6e5f786f72",
                "fd03a7719fb4a47ec1dadb83cde2ab98e09b4f477e91efc68913d1d6881ab5e3",
                "ac0f23529cafee260c92167a7df27a7c3c87d0a6188b2b833dba5f1ebc36df89",
                "6e8843e3f022d5fa810f32fec0bbd0e6ababedbaa64841caea2ece0e64191bec"
            ),
        ]
        for (source, payloadHex, laneHashHex, messageIdHex, digestHex) in vectors {
            let lane = try SccpLaneIdV1(source: source, target: .soraTaira)
            let payload = try SccpV1.decodeLowerHex(payloadHex)
            let payloadHash = try SccpV1.payloadHash(payload)
            let messageId = try SccpV1.messageId(lane: lane, canonicalPayload: payload)
            XCTAssertEqual(SccpV1.encodeLowerHex(SccpV1.laneHash(lane)), laneHashHex)
            XCTAssertEqual(SccpV1.encodeLowerHex(messageId), messageIdHex)
            XCTAssertEqual(
                SccpV1.encodeLowerHex(try SccpV1.sourceEventDigest(
                    lane: lane,
                    messageId: messageId,
                    payloadHash: payloadHash
                )),
                digestHex
            )
        }
    }

    func testCodecsAndSourceRolesRejectAliasesAndCollisions() throws {
        XCTAssertEqual(try SccpCodecV1.canonicalText.validate(Data("merchant@taira".utf8)), Data("merchant@taira".utf8))
        let i105 = try AccountAddress
            .fromAccount(publicKey: validEd25519PublicKey(seed: 0x91))
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        XCTAssertTrue(i105.unicodeScalars.contains { !$0.isASCII }, "fixture must exercise non-ASCII I105 digits")
        XCTAssertEqual(try SccpCodecV1.canonicalText.validate(Data(i105.utf8)), Data(i105.utf8))

        var checksumTampering = i105
        let finalDigit = checksumTampering.removeLast()
        checksumTampering.append(finalDigit == "1" ? "2" : "1")
        let noncanonicalSentinel = "n369" + i105.dropFirst("test".count)
        for value in [
            Data(),
            Data(" padded".utf8),
            Data("contains space".utf8),
            Data("line\nbreak".utf8),
            Data("merchant🙂".utf8),
            Data(checksumTampering.utf8),
            Data(noncanonicalSentinel.utf8),
            Data((i105 + String(repeating: "ｲ", count: 100)).utf8),
            Data(repeating: 0x21, count: 257),
        ] {
            XCTAssertThrowsError(try SccpCodecV1.canonicalText.validate(value))
        }
        XCTAssertThrowsError(try SccpCodecV1.evmAddress20.validate(Data(repeating: 0, count: 20)))
        XCTAssertThrowsError(try SccpCodecV1.evmAddress20.validate(Data(repeating: 1, count: 19)))
        var tron = Data(repeating: 1, count: 21)
        tron[0] = 0x41
        XCTAssertNoThrow(try SccpCodecV1.tronAddress21.validate(tron))
        tron[0] = 0x42
        XCTAssertThrowsError(try SccpCodecV1.tronAddress21.validate(tron))
        let ton = try SccpTonAddressV1(workchain: 0, account: Data(repeating: 0xa5, count: 32))
        XCTAssertEqual(try ton.canonicalAccount36(), Data(repeating: 0, count: 4) + Data(repeating: 0xa5, count: 32))
        XCTAssertNoThrow(try SccpCodecV1.tonAccount36.validate(ton.canonicalAccount36()))
        XCTAssertThrowsError(try SccpCodecV1.tonAccount36.validate(Data(repeating: 0, count: 36)))
        var workchainOne = try ton.canonicalAccount36()
        workchainOne[3] = 1
        XCTAssertThrowsError(try SccpCodecV1.tonAccount36.validate(workchainOne))
        XCTAssertTrue(SccpNativeBackendV1.tonMasterchain.supports(.tonMainnet))
        XCTAssertFalse(SccpNativeBackendV1.tonMasterchain.supports(.bscMainnet))
        XCTAssertThrowsError(try SccpSourceEmitterV1.validatedEvm(
            address: Data(repeating: 1, count: 20),
            runtimeCodeHash: Data(repeating: 2, count: 32),
            routeConfigHash: Data(repeating: 2, count: 32)
        ))
        XCTAssertNoThrow(try SccpSourceEmitterV1.validatedTon(
            address: ton,
            codeHash: Data(repeating: 2, count: 32),
            routeConfigHash: Data(repeating: 3, count: 32)
        ))
    }

    func testTonCanonicalNetworkIdentitiesBindExactZeroStates() throws {
        let mainnet = SccpV1.canonicalNetworkBytes(.tonMainnet)
        let testnet = SccpV1.canonicalNetworkBytes(.tonTestnet)
        XCTAssertEqual(mainnet.count, 90)
        XCTAssertEqual(testnet.count, 90)
        XCTAssertEqual(Array(mainnet.prefix(10)), [1, 14, 4, 0, 0, 0, 17, 255, 255, 255])
        XCTAssertEqual(Array(testnet.prefix(10)), [1, 15, 4, 0, 0, 0, 253, 255, 255, 255])
        XCTAssertNotEqual(mainnet, testnet)
    }

    func testSubmitDTOContainsOnlyClosedArtifactFields() throws {
        XCTAssertEqual(SccpSubmitValidation.maximumGroth16ArtifactBytes, 16 * 1024 * 1024 + 64 * 1024)
        XCTAssertEqual(SccpSubmitValidation.maximumDestinationArtifactBytes, 16 * 1024 * 1024 + 128 * 1024)
        XCTAssertEqual(SccpSubmitValidation.maximumDestinationArtifactBase64Bytes, 22_544_384)
        let privateKey = Curve25519.Signing.PrivateKey()
        let authority = try AccountAddress.fromAccount(publicKey: privateKey.publicKey.rawRepresentation)
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let artifact = noritoEncode(
            typeName: "iroha_data_model::bridge::BridgeSccpDestinationProofV1",
            payload: Data([1])
        )
            .base64EncodedString()
        let nativeArtifact = noritoEncode(
            typeName: "iroha_sccp::native_admission::SccpNativeInboundMessageProofV1",
            payload: Data([1])
        ).base64EncodedString()
        let signature = try privateKey.signature(for: Data(repeating: 7, count: 32)).base64EncodedString()
        let transactionPayload = try canonicalSccpTransactionPayload(
            authority: authority,
            creationTimeMs: 7
        ).base64EncodedString()
        let request = try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: artifact,
            signatureB64: signature,
            transactionPayloadB64: transactionPayload,
            creationTimeMs: 7,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        )
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(request)) as? [String: Any])
        XCTAssertEqual(Set(json.keys), [
            "authority", "fee_payment", "signature_b64", "transaction_payload_b64",
            "destination_proof_b64", "creation_time_ms",
        ])
        XCTAssertEqual(
            (json["fee_payment"] as? [String: Any])?["payer"] as? String,
            "authority"
        )
        XCTAssertEqual(json["transaction_payload_b64"] as? String, transactionPayload)
        let messageRequest = try ToriiBridgeMessageSubmitRequest(
            authority: authority,
            nativeProofB64: nativeArtifact,
            signatureB64: signature,
            transactionPayloadB64: transactionPayload,
            creationTimeMs: 7,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        )
        let messageJSON = try XCTUnwrap(
            JSONSerialization.jsonObject(with: JSONEncoder().encode(messageRequest)) as? [String: Any]
        )
        XCTAssertEqual(Set(messageJSON.keys), [
            "authority", "fee_payment", "signature_b64", "transaction_payload_b64",
            "native_proof_b64", "creation_time_ms",
        ])
        for retired in ["public_key_hex", "message_bundle_b64", "network_id_hex", "proof_bytes_hex", "allow_unready"] {
            XCTAssertNil(json[retired])
        }
        let preparation = try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: artifact,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        )
        let preparationJSON = try XCTUnwrap(
            JSONSerialization.jsonObject(with: JSONEncoder().encode(preparation)) as? [String: Any]
        )
        XCTAssertNil(preparationJSON["signature_b64"])
        XCTAssertNil(preparationJSON["transaction_payload_b64"])
        XCTAssertNotNil(preparationJSON["fee_payment"])
        XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(authority: authority, destinationProofB64: "AQ==",
            feePayment: .authority(chargeLimits: [], gasLimit: nil),))
        XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(authority: authority, destinationProofB64: artifact, signatureB64: "AQ==",
            feePayment: .authority(chargeLimits: [], gasLimit: nil),))
        XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: artifact,
            signatureB64: signature,
            creationTimeMs: 7,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        ))
        XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: artifact,
            transactionPayloadB64: transactionPayload,
            creationTimeMs: 7,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        ))
        XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: artifact,
            signatureB64: signature,
            transactionPayloadB64: transactionPayload,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        ))
        let genericSignature = Data(repeating: 1, count: 65).base64EncodedString()
        XCTAssertNoThrow(try SccpSubmitValidation.optionalSignature(genericSignature))
        XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: artifact,
            signatureB64: Data(repeating: 0, count: 64).base64EncodedString(),
            transactionPayloadB64: transactionPayload,
            creationTimeMs: 7,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        ))
        XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: artifact,
            signatureB64: Data(repeating: 1, count: 16 * 1024 + 1).base64EncodedString(),
            transactionPayloadB64: transactionPayload,
            creationTimeMs: 7,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        ))
        XCTAssertThrowsError(try ToriiBridgeMessageSubmitRequest(authority: authority, nativeProofB64: nativeArtifact, creationTimeMs: 0,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),))
    }

    func testSubmitPreservesTypedFeePaymentIntents() throws {
        let authority = try AccountAddress
            .fromAccount(publicKey: validEd25519PublicKey(seed: 0x51))
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let sponsor = try AccountAddress
            .fromAccount(publicKey: validEd25519PublicKey(seed: 0x52))
            .toI105(networkPrefix: AccountId.defaultNetworkPrefix)
        var assetDefinitionBytes = Data(repeating: 0x53, count: 16)
        assetDefinitionBytes[6] = (assetDefinitionBytes[6] & 0x0f) | 0x40
        assetDefinitionBytes[8] = (assetDefinitionBytes[8] & 0x3f) | 0x80
        let assetDefinitionId = try XCTUnwrap(
            AssetDefinitionAddress.encode(uuidBytes: assetDefinitionBytes)
        )
        let limits = [
            try FeeChargeLimit(
                kind: .nexus,
                assetDefinitionId: assetDefinitionId,
                maxAmount: "3.25"
            ),
            try FeeChargeLimit(
                kind: .pipelineGas,
                assetDefinitionId: assetDefinitionId,
                maxAmount: "9"
            ),
        ]
        let programId = try FeeSponsorProgramId(sponsor: sponsor, name: "sccp_bridge")
        let requestedIntents: [FeePaymentIntent] = [
            .authority(chargeLimits: [], gasLimit: 500_000),
            .sponsor(
                programId: programId,
                programRevision: 7,
                chargeLimits: [],
                gasLimit: 750_000
            ),
        ]
        let quotedIntents: [FeePaymentIntent] = [
            .authority(chargeLimits: limits, gasLimit: 500_000),
            .sponsor(
                programId: programId,
                programRevision: 7,
                chargeLimits: limits,
                gasLimit: 750_000
            ),
        ]
        let artifact = noritoEncode(
            typeName: SccpSubmitValidation.destinationArtifactTypeName,
            payload: Data([1])
        ).base64EncodedString()
        let signature = Data(repeating: 1, count: 64).base64EncodedString()

        for (requestedIntent, quotedIntent) in zip(requestedIntents, quotedIntents) {
            let transactionPayload = try canonicalSccpTransactionPayload(
                authority: authority,
                creationTimeMs: 7,
                feePayment: quotedIntent
            )
            let request = try ToriiBridgeProofSubmitRequest(
                authority: authority,
                destinationProofB64: artifact,
                signatureB64: signature,
                transactionPayloadB64: transactionPayload.base64EncodedString(),
                creationTimeMs: 7,
                feePayment: requestedIntent,
            )
            XCTAssertEqual(request.feePayment, requestedIntent)
            XCTAssertEqual(
                request.transactionPayloadB64,
                transactionPayload.base64EncodedString(),
                "SCCP validation must preserve the quoted, signature-bound transaction bytes"
            )
        }

        let sponsoredPayload = try canonicalSccpTransactionPayload(
            authority: authority,
            creationTimeMs: 7,
            feePayment: quotedIntents[1]
        )
        XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: artifact,
            signatureB64: signature,
            transactionPayloadB64: sponsoredPayload.base64EncodedString(),
            creationTimeMs: 7,
            feePayment: requestedIntents[0]
        ))
        let authorityPayload = try canonicalSccpTransactionPayload(
            authority: authority,
            creationTimeMs: 7,
            feePayment: quotedIntents[0]
        )
        XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: artifact,
            signatureB64: signature,
            transactionPayloadB64: authorityPayload.base64EncodedString(),
            creationTimeMs: 7,
            feePayment: .authority(chargeLimits: [], gasLimit: 500_001)
        ))
        XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: artifact,
            signatureB64: signature,
            transactionPayloadB64: sponsoredPayload.base64EncodedString(),
            creationTimeMs: 7,
            feePayment: .sponsor(
                programId: programId,
                programRevision: 8,
                chargeLimits: [],
                gasLimit: 750_000
            )
        ))
    }

    func testSubmitAcceptsExactTairaSponsorProgram() throws {
        let authority = try AccountAddress
            .fromAccount(publicKey: validEd25519PublicKey(seed: 0x54))
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let programId = try FeeSponsorProgramId(
            "testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A/cbsi_web"
        )
        let feePayment = FeePaymentIntent.sponsor(
            programId: programId,
            programRevision: 1,
            chargeLimits: [],
            gasLimit: nil
        )
        let transactionPayload = try canonicalSccpTransactionPayload(
            authority: authority,
            creationTimeMs: 7,
            feePayment: feePayment
        )
        let artifact = noritoEncode(
            typeName: SccpSubmitValidation.destinationArtifactTypeName,
            payload: Data([1])
        ).base64EncodedString()
        let request = try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: artifact,
            signatureB64: Data(repeating: 1, count: 64).base64EncodedString(),
            transactionPayloadB64: transactionPayload.base64EncodedString(),
            creationTimeMs: 7,
            feePayment: feePayment
        )

        XCTAssertEqual(request.feePayment, feePayment)
        XCTAssertEqual(
            request.transactionPayloadB64,
            transactionPayload.base64EncodedString()
        )
    }

    func testSubmitRequiresExactDefaultTransactionTimeToLive() throws {
        let authority = try AccountAddress
            .fromAccount(publicKey: validEd25519PublicKey(seed: 0x56))
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)

        let defaultTimeToLive = try canonicalSccpTransactionPayload(
            authority: authority,
            creationTimeMs: 7
        )
        XCTAssertNoThrow(try SccpSubmitValidation.canonicalTransactionPayload(
            defaultTimeToLive,
            creationTimeMs: 7,
            expectedAuthority: authority,
            expectedFeePayment: nil
        ))

        let missingTimeToLive = try canonicalSccpTransactionPayload(
            authority: authority,
            creationTimeMs: 7,
            timeToLiveMs: nil
        )
        XCTAssertThrowsError(try SccpSubmitValidation.canonicalTransactionPayload(
            missingTimeToLive,
            creationTimeMs: 7,
            expectedAuthority: authority,
            expectedFeePayment: nil
        ))

        let nonDefaultTimeToLive = try canonicalSccpTransactionPayload(
            authority: authority,
            creationTimeMs: 7,
            timeToLiveMs: 99_999
        )
        XCTAssertThrowsError(try SccpSubmitValidation.canonicalTransactionPayload(
            nonDefaultTimeToLive,
            creationTimeMs: 7,
            expectedAuthority: authority,
            expectedFeePayment: nil
        ))
    }

    func testSubmitRejectsGenesisUnknownLegacyAndUnmarkedTransactionDomains() throws {
        let authority = try AccountAddress
            .fromAccount(publicKey: validEd25519PublicKey(seed: 0x57))
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        func domain(kind: UInt32, value: Data? = nil, trailing: Data = Data()) -> Data {
            var writer = CompactNoritoWriter()
            writer.writeUInt32LE(kind)
            if let value {
                writer.writeField(value)
            }
            writer.writeBytes(trailing)
            return writer.data
        }
        var unmarked = TestNetworkIds.canonical.bytes
        unmarked[unmarked.index(before: unmarked.endIndex)] &= 0xfe
        var legacyChain = CompactNoritoWriter()
        legacyChain.writeField(CompactNorito.encodeString("sccp-test"))
        let invalidDomains = [
            domain(kind: 1),
            domain(kind: 2),
            legacyChain.data,
            domain(kind: 0, value: unmarked),
            domain(
                kind: 0,
                value: TestNetworkIds.canonical.bytes,
                trailing: Data([0])
            ),
        ]

        for invalidDomain in invalidDomains {
            let payload = try canonicalSccpTransactionPayload(
                authority: authority,
                creationTimeMs: 7,
                domainOverride: invalidDomain
            )
            XCTAssertThrowsError(try SccpSubmitValidation.canonicalTransactionPayload(
                payload,
                creationTimeMs: 7,
                expectedAuthority: authority,
                expectedFeePayment: nil
            ))
        }
    }

    func testSubmitRejectsNonNfcSponsorProgramNameInRawCompactTransaction() throws {
        let authority = try AccountAddress
            .fromAccount(publicKey: validEd25519PublicKey(seed: 0x55))
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let sponsor = try AccountAddress.parseEncoded(
            "testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A",
            expectedPrefix: SccpV1.tairaI105DiscriminantV1
        )
        var program = CompactNoritoWriter()
        program.writeField(try sponsor.compactNoritoAccountControllerPayload())
        program.writeField(CompactNorito.encodeString("e\u{301}"))

        var chargeLimits = CompactNoritoWriter()
        chargeLimits.writeLength(0)
        var sponsored = CompactNoritoWriter()
        sponsored.writeField(program.data)
        sponsored.writeField(CompactNorito.encodeUInt64(1))
        sponsored.writeField(chargeLimits.data)
        sponsored.writeField(Data([0]))
        var feePayment = CompactNoritoWriter()
        feePayment.writeUInt32LE(1)
        feePayment.writeField(sponsored.data)

        let transactionPayload = try canonicalSccpTransactionPayload(
            authority: authority,
            creationTimeMs: 7,
            rawFeePayment: feePayment.data
        )
        XCTAssertThrowsError(
            try SccpSubmitValidation.canonicalTransactionPayload(
                transactionPayload,
                creationTimeMs: 7,
                expectedAuthority: authority,
                expectedFeePayment: nil
            )
        )
    }

    func testSubmitAuthorityRequiresExactTairaDiscriminant() throws {
        let address = try AccountAddress.fromAccount(publicKey: validEd25519PublicKey(seed: 0x41))
        let authority = try address.toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let artifact = noritoEncode(
            typeName: "iroha_data_model::bridge::BridgeSccpDestinationProofV1",
            payload: Data([1])
        ).base64EncodedString()
        let nativeArtifact = noritoEncode(
            typeName: "iroha_sccp::native_admission::SccpNativeInboundMessageProofV1",
            payload: Data([1])
        ).base64EncodedString()
        XCTAssertEqual(SccpV1.tairaI105DiscriminantV1, 369)
        XCTAssertTrue(authority.hasPrefix("test"))
        XCTAssertNoThrow(try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: artifact,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        ))
        XCTAssertNoThrow(try ToriiBridgeMessageSubmitRequest(
            authority: authority,
            nativeProofB64: nativeArtifact,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        ))

        var checksumMutation = authority
        let finalDigit = checksumMutation.removeLast()
        checksumMutation.append(finalDigit == "1" ? "2" : "1")
        let invalidAuthorities: [(String, String)] = [
            ("default discriminant 753", try address.toI105(networkPrefix: 753)),
            ("generic canonical hex", try address.canonicalHex()),
            ("development discriminant", try address.toI105(networkPrefix: 0)),
            ("custom discriminant", try address.toI105(networkPrefix: 42)),
            ("malformed account alias", "alice"),
            ("checksum mutation", checksumMutation),
        ]
        for (label, invalidAuthority) in invalidAuthorities {
            XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(
                authority: invalidAuthority,
                destinationProofB64: artifact,
                feePayment: .authority(chargeLimits: [], gasLimit: nil),
            ), label)
            XCTAssertThrowsError(try ToriiBridgeMessageSubmitRequest(
                authority: invalidAuthority,
                nativeProofB64: nativeArtifact,
                feePayment: .authority(chargeLimits: [], gasLimit: nil),
            ), label)
        }
    }

    func testSubmitArtifactsRequireExactSchemaAndZeroAlignmentPadding() throws {
        let authority = try AccountAddress
            .fromAccount(publicKey: validEd25519PublicKey(seed: 0x42))
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let destination = noritoEncode(
            typeName: SccpSubmitValidation.destinationArtifactTypeName,
            payload: Data([1, 2, 3])
        )
        let native = noritoEncode(
            typeName: SccpSubmitValidation.nativeInboundProofTypeName,
            payload: Data([1, 2, 3])
        )
        let legacyBn254Artifact = noritoEncode(
            typeName: "iroha_sccp::SccpGroth16Bn254ProofArtifactV1",
            payload: Data([1, 2, 3])
        )
        XCTAssertNoThrow(try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: destination.base64EncodedString(),
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        ))
        XCTAssertNoThrow(try ToriiBridgeMessageSubmitRequest(
            authority: authority,
            nativeProofB64: native.base64EncodedString(),
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        ))
        XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: native.base64EncodedString(),
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        ))
        XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(
            authority: authority,
            destinationProofB64: legacyBn254Artifact.base64EncodedString(),
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        ))
        XCTAssertThrowsError(try ToriiBridgeMessageSubmitRequest(
            authority: authority,
            nativeProofB64: destination.base64EncodedString(),
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        ))
        for frame in [destination, native] {
            var padded = Data(frame.prefix(NoritoHeader.encodedLength))
            padded.append(Data(repeating: 0, count: 8))
            padded.append(frame.dropFirst(NoritoHeader.encodedLength))
            if frame == destination {
                XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(
                    authority: authority,
                    destinationProofB64: padded.base64EncodedString(),
                    feePayment: .authority(chargeLimits: [], gasLimit: nil),
                ))
            } else {
                XCTAssertThrowsError(try ToriiBridgeMessageSubmitRequest(
                    authority: authority,
                    nativeProofB64: padded.base64EncodedString(),
                    feePayment: .authority(chargeLimits: [], gasLimit: nil),
                ))
            }
        }
    }

    func testCapabilitiesAreFixedAndRejectRetiredDiscoveryFields() throws {
        let valid = capabilitiesJSON()
        let parsed = try SccpCapabilities.parse(valid)
        XCTAssertEqual(parsed.registryPath, "/v1/sccp/registry")
        XCTAssertEqual(parsed.proofRequestPath, "/v1/sccp/proof-requests/{message_id}")
        XCTAssertEqual(parsed.registryLimits.maxRetainedRoutesPerLane, 64)
        XCTAssertEqual(parsed.registryLimits.maxRetainedNativeTrustAnchorsPerLane, 4_096)
        XCTAssertEqual(parsed.resourceLimits.maxOutboundMessagesPerBlock, 512)
        XCTAssertEqual(parsed.resourceLimits.maxOutboundMessagePayloadBytes, 4_096)
        XCTAssertEqual(parsed.resourceLimits.maxPendingOutboundMessages, 65_536)
        XCTAssertEqual(parsed.resourceLimits.maxPendingOutboundPayloadBytes, 256 * 1024 * 1024)
        XCTAssertEqual(parsed.resourceLimits.maxBlsSignerContributionsPerTransaction, 131_713)
        var readOnly = try jsonObject(valid)
        readOnly.removeValue(forKey: "proof_submit_path")
        readOnly.removeValue(forKey: "native_message_submit_path")
        XCTAssertNoThrow(try SccpCapabilities.parse(jsonData(readOnly)))
        let mutations: [(inout [String: Any]) -> Void] = [
            { $0["registry_path"] = "/v1/sccp/manifests" },
            { $0["proof_request_path"] = "/v1/sccp/proof-requests/{message_id}?network=bsc" },
            { $0["proof_artifact_path"] = "/v1/sccp/artifacts/message/{message_id}" },
            { $0["proof_job_path"] = "/v1/sccp/jobs/message/{message_id}" },
            { $0["outbound"] = [:] },
            { $0["registry_revision"] = "0x" + String(repeating: "0", count: 64) },
            { $0.removeValue(forKey: "proof_submit_path") },
            { $0.removeValue(forKey: "native_message_submit_path") },
        ]
        for mutate in mutations {
            var object = try jsonObject(valid)
            mutate(&object)
            XCTAssertThrowsError(try SccpCapabilities.parse(jsonData(object)))
        }

        let resourceKeys = [
            "max_outbound_messages_per_block", "max_outbound_message_payload_bytes",
            "max_pending_outbound_messages", "max_pending_outbound_payload_bytes",
            "max_proofs_per_transaction", "max_proofs_per_block", "max_proof_bytes_per_proof",
            "max_proof_bytes_per_transaction", "max_proof_bytes_per_block",
            "max_native_headers_per_transaction", "max_native_headers_per_block",
            "max_ethereum_light_client_updates_per_transaction",
            "max_ethereum_light_client_updates_per_block",
            "max_native_header_bytes_per_transaction", "max_native_header_bytes_per_block",
            "max_secp256k1_recoveries_per_transaction", "max_secp256k1_recoveries_per_block",
            "max_bls_aggregate_checks_per_transaction", "max_bls_aggregate_checks_per_block",
            "max_bls_signer_contributions_per_transaction",
            "max_bls_signer_contributions_per_block",
            "max_ed25519_signature_checks_per_transaction",
            "max_ed25519_signature_checks_per_block",
            "max_ed25519_validator_key_checks_per_transaction",
            "max_ed25519_validator_key_checks_per_block",
            "max_bn254_pairing_checks_per_transaction", "max_bn254_pairing_checks_per_block",
            "max_bls12_381_pairing_checks_per_transaction",
            "max_bls12_381_pairing_checks_per_block",
        ]
        for key in resourceKeys {
            var object = try jsonObject(valid)
            var limits = object["resource_limits"] as! [String: Any]
            limits[key] = 0
            object["resource_limits"] = limits
            XCTAssertThrowsError(try SccpCapabilities.parse(jsonData(object)), key)
        }

        for (field, value) in [
            ("max_outbound_messages_per_block", 511),
            ("max_outbound_messages_per_block", 513),
            ("max_outbound_message_payload_bytes", 4_095),
            ("max_outbound_message_payload_bytes", 4_097),
        ] {
            var drifted = try jsonObject(valid)
            var limits = drifted["resource_limits"] as! [String: Any]
            limits[field] = value
            drifted["resource_limits"] = limits
            XCTAssertThrowsError(try SccpCapabilities.parse(jsonData(drifted)), field)
        }
        for field in [
            "max_outbound_messages_per_block", "max_outbound_message_payload_bytes",
            "max_pending_outbound_messages", "max_pending_outbound_payload_bytes",
        ] {
            var missing = try jsonObject(valid)
            var limits = missing["resource_limits"] as! [String: Any]
            limits.removeValue(forKey: field)
            missing["resource_limits"] = limits
            XCTAssertThrowsError(try SccpCapabilities.parse(jsonData(missing)), field)
        }

        let orderingRelations = [
            ("max_proof_bytes_per_proof", "max_proof_bytes_per_transaction"),
            ("max_proofs_per_transaction", "max_proofs_per_block"),
            ("max_proof_bytes_per_transaction", "max_proof_bytes_per_block"),
            ("max_native_headers_per_transaction", "max_native_headers_per_block"),
            (
                "max_ethereum_light_client_updates_per_transaction",
                "max_ethereum_light_client_updates_per_block"
            ),
            (
                "max_native_header_bytes_per_transaction",
                "max_native_header_bytes_per_block"
            ),
            (
                "max_secp256k1_recoveries_per_transaction",
                "max_secp256k1_recoveries_per_block"
            ),
            (
                "max_bls_aggregate_checks_per_transaction",
                "max_bls_aggregate_checks_per_block"
            ),
            (
                "max_bls_signer_contributions_per_transaction",
                "max_bls_signer_contributions_per_block"
            ),
            (
                "max_ed25519_signature_checks_per_transaction",
                "max_ed25519_signature_checks_per_block"
            ),
            (
                "max_ed25519_validator_key_checks_per_transaction",
                "max_ed25519_validator_key_checks_per_block"
            ),
            (
                "max_bn254_pairing_checks_per_transaction",
                "max_bn254_pairing_checks_per_block"
            ),
            (
                "max_bls12_381_pairing_checks_per_transaction",
                "max_bls12_381_pairing_checks_per_block"
            ),
        ]
        for (lower, upper) in orderingRelations {
            var reversed = try jsonObject(valid)
            var limits = reversed["resource_limits"] as! [String: Any]
            let upperValue = (limits[upper] as! NSNumber).uint64Value
            limits[lower] = upperValue + 1
            reversed["resource_limits"] = limits
            XCTAssertThrowsError(try SccpCapabilities.parse(jsonData(reversed)), "\(lower) > \(upper)")
        }

        var driftedRegistryLimits = try jsonObject(valid)
        var registryLimits = driftedRegistryLimits["registry_limits"] as! [String: Any]
        registryLimits["max_retained_routes_per_lane"] = 65
        driftedRegistryLimits["registry_limits"] = registryLimits
        XCTAssertThrowsError(try SccpCapabilities.parse(jsonData(driftedRegistryLimits)))

        let canonical = String(data: valid, encoding: .utf8)!
        let needle = "\"max_proofs_per_transaction\":1"
        XCTAssertTrue(canonical.contains(needle))
        for token in ["1.0", "1e0", "-0", "9007199254740992.5", "1e999"] {
            let hostile = canonical.replacingOccurrences(
                of: needle,
                with: "\"max_proofs_per_transaction\":\(token)"
            )
            XCTAssertThrowsError(try SccpCapabilities.parse(Data(hostile.utf8)), token)
        }

        var boundary = try jsonObject(valid)
        var boundaryLimits = boundary["resource_limits"] as! [String: Any]
        let jsonSafeMaximum: UInt64 = (1 << 53) - 1
        for field in [
            "max_proof_bytes_per_proof", "max_proof_bytes_per_transaction",
            "max_proof_bytes_per_block", "max_native_header_bytes_per_transaction",
            "max_native_header_bytes_per_block", "max_pending_outbound_messages",
            "max_pending_outbound_payload_bytes",
        ] {
            boundaryLimits[field] = jsonSafeMaximum
        }
        boundary["resource_limits"] = boundaryLimits
        XCTAssertEqual(
            try SccpCapabilities.parse(jsonData(boundary)).resourceLimits.maxProofBytesPerBlock,
            jsonSafeMaximum
        )
        boundaryLimits["max_proof_bytes_per_block"] = jsonSafeMaximum + 1
        boundary["resource_limits"] = boundaryLimits
        XCTAssertThrowsError(try SccpCapabilities.parse(jsonData(boundary)))
        for field in ["max_pending_outbound_messages", "max_pending_outbound_payload_bytes"] {
            var overflow = try jsonObject(valid)
            var limits = overflow["resource_limits"] as! [String: Any]
            limits[field] = jsonSafeMaximum + 1
            overflow["resource_limits"] = limits
            XCTAssertThrowsError(try SccpCapabilities.parse(jsonData(overflow)), field)
        }
    }

    func testRegistryValidatesFullPolicyElevenSignalKeyAndRouteCommitment() throws {
        let valid = try registryJSON()
        let registry = try SccpRegistryV1.parse(valid)
        XCTAssertEqual(registry.lanes.count, 1)
        XCTAssertEqual(registry.lanes[0].routes[0].routeId, "taira_bsc_xor")
        XCTAssertEqual(
            registry.lanes[0].routes[0].destination.maxWrappedSupply,
            Self.evmRegistryMaxWrappedSupply
        )
        XCTAssertEqual(
            registry.lanes[0].routes[0].maxOutstandingLiability,
            Self.registryMaxOutstandingLiability
        )
        XCTAssertTrue(
            String(decoding: valid, as: UTF8.self).contains(
                "\"max_wrapped_supply\":\(Self.evmRegistryMaxWrappedSupply)"
            )
        )
        var missingCap = try jsonObject(valid)
        mutateDeployment(&missingCap) { $0.removeValue(forKey: "max_wrapped_supply") }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(missingCap)))
        var zeroCap = try jsonObject(valid)
        mutateDeployment(&zeroCap) { $0["max_wrapped_supply"] = 0 }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(zeroCap)))
        var mismatchedLiability = try jsonObject(valid)
        mutateRoute(&mismatchedLiability) { route in
            var settlement = route["settlement"] as! [String: Any]
            settlement["max_outstanding_liability"] = 999_999_999_999
            route["settlement"] = settlement
        }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(mismatchedLiability)))
        XCTAssertThrowsError(try SccpRegistryV1.parse(Data(
            String(decoding: valid, as: UTF8.self).replacingOccurrences(
                of: Self.evmRegistryMaxWrappedSupply,
                with: "340282366920938463463374607431768211456"
            ).utf8
        )))
        XCTAssertTrue(registry.lanes[0].nativeTrustAnchors.isEmpty)
        XCTAssertNil(registry.lanes[0].currentNativeTrustAnchorHash)
        let outboundProofPolicy = registry.lanes[0].routes[0].destination.outboundProofPolicy
        XCTAssertEqual(outboundProofPolicy.version, 1)
        XCTAssertEqual(
            outboundProofPolicy.semanticProfile.publicSignalSchemaHash,
            publicSignalSchemaHash()
        )
        let finalityAnchor = outboundProofPolicy.soraFinalityAnchor
        XCTAssertEqual(finalityAnchor.protocolVersion, 4)
        XCTAssertEqual(finalityAnchor.checkpointContextId, Data(repeating: 0xa2, count: 32))
        XCTAssertEqual(finalityAnchor.checkpointFinalityArtifactHash, Data(repeating: 0xa3, count: 32))
        XCTAssertEqual(
            finalityAnchor.anchorHash,
            Data(hexString: "4410EE4CCFD06F2D0E3A658615D516AC8CF65255D8A8716CE511EA95E135C8C3")
        )
        let currentRequest = try SccpGroth16ProofRequestV1.parse(
            try proofRequestJSON(protocolVersion: 4)
        )
        XCTAssertEqual(currentRequest.soraFinalityAnchor.protocolVersion, 4)
        XCTAssertEqual(currentRequest.soraFinalityAnchor.anchorHash, finalityAnchor.anchorHash)
        XCTAssertThrowsError(try SccpGroth16ProofRequestV1.parse(
            try proofRequestJSON(protocolVersion: 3)
        ))

        let invalidFinalityAnchors: [(inout [String: Any]) -> Void] = [
            { $0["protocol_version"] = 1 },
            { $0["protocol_version"] = 3 },
            { $0["protocol_version"] = "4" },
            { $0["protocol_version"] = 5 },
            { $0["protocol_version"] = "3" },
            { $0["protocol_version"] = true },
            { $0["validator_set_epoch"] = 2 },
            { $0["checkpoint_context_id"] = String(repeating: "0", count: 64) },
            { $0["checkpoint_context_id"] = $0["chain_id_hash"] },
            { $0["checkpoint_block_hash"] = $0["checkpoint_context_id"] },
            { $0["checkpoint_finality_artifact_hash"] = String(repeating: "0", count: 64) },
            { $0["checkpoint_finality_artifact_hash"] = $0["checkpoint_block_hash"] },
            { $0.removeValue(forKey: "checkpoint_finality_artifact_hash") },
        ]
        for mutation in invalidFinalityAnchors {
            var hostile = try jsonObject(valid)
            mutateFinalityAnchor(&hostile, mutation)
            XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(hostile)))
        }
        let canonicalJSON = String(data: valid, encoding: .utf8)!
        XCTAssertThrowsError(try SccpRegistryV1.parse(Data(
            canonicalJSON.replacingOccurrences(
                of: "\"protocol_version\":4",
                with: "\"protocol_version\":4.0"
            ).utf8
        )))
        XCTAssertThrowsError(try SccpRegistryV1.parse(Data(
            canonicalJSON.replacingOccurrences(
                of: "\"checkpoint_height\":7",
                with: "\"checkpoint_height\":7e0"
            ).utf8
        )))

        let tron = try SccpRegistryV1.parse(try registryJSON(source: .tronMainnet))
        XCTAssertEqual(tron.lanes[0].routes[0].routeId, "taira_tron_xor")
        XCTAssertEqual(tron.lanes[0].routes[0].destination.family, .tronGroth16Bn254)
        XCTAssertThrowsError(try SccpRegistryV1.parse(try registryJSON(
            source: .tronMainnet,
            aliasTronBindingWithTokenCodeHash: true
        )))

        var retired = try jsonObject(valid)
        mutateLane(&retired) { $0["lane_id"] = lane("solana-mainnet-beta", "sora-taira") }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(retired)))

        var noSignal = try jsonObject(valid)
        mutateDeployment(&noSignal) { deployment in
            var key = deployment["verifying_key"] as! [String: Any]
            var ic = key["ic"] as! [String: Any]
            ic.removeValue(forKey: "signal_10")
            key["ic"] = ic
            deployment["verifying_key"] = key
        }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(noSignal)))

        var oldBrowser = try jsonObject(valid)
        mutateRoute(&oldBrowser) { $0["destination_browser_prover"] = ["module_url": "https://invalid.test"] }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(oldBrowser)))

        var badKeyHash = try jsonObject(valid)
        mutateDeployment(&badKeyHash) { $0["verifier_key_hash"] = upper(0x99, bytes: 32) }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(badKeyHash)))

        var missingPolicy = try jsonObject(valid)
        mutateDeployment(&missingPolicy) { $0.removeValue(forKey: "outbound_proof_policy") }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(missingPolicy)))

        var badConfig = try jsonObject(valid)
        mutateEmitterIdentity(&badConfig) { $0["route_config_hash"] = upper(0x98, bytes: 32) }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(badConfig)))

        var missingCutoff = try jsonObject(valid)
        mutateRoute(&missingCutoff) { $0.removeValue(forKey: "inbound_finality_cutoff") }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(missingCutoff)))

        let firstAnchor: [String: Any] = [
            "backend": ["backend": "bsc_parlia_v1", "protocol": NSNull()],
            "anchor_hash": upper(0x91, bytes: 32),
            "checkpoint_height": 1,
        ]
        let secondAnchor: [String: Any] = [
            "backend": ["backend": "bsc_parlia_v1", "protocol": NSNull()],
            "anchor_hash": upper(0x92, bytes: 32),
            "checkpoint_height": 2,
        ]
        var history = try jsonObject(valid)
        mutateLane(&history) {
            $0["native_trust_anchors"] = [firstAnchor, secondAnchor]
            $0["current_native_trust_anchor_hash"] = upper(0x92, bytes: 32)
        }
        let parsedHistory = try SccpRegistryV1.parse(jsonData(history))
        XCTAssertEqual(parsedHistory.lanes[0].nativeTrustAnchors.count, 2)
        XCTAssertEqual(parsedHistory.lanes[0].currentNativeTrustAnchorHash, Data(repeating: 0x92, count: 32))

        var retiredRoute = history
        mutateRoute(&retiredRoute) {
            $0["activation"] = ["activation": "retired", "direction": NSNull()]
            $0["inbound_finality_cutoff"] = [
                "trust_anchor_hash": upper(0x91, bytes: 32),
                "max_anchor_interval_height": 2,
            ]
        }
        let parsedRetired = try SccpRegistryV1.parse(jsonData(retiredRoute))
        XCTAssertEqual(
            parsedRetired.lanes[0].routes[0].inboundFinalityCutoff,
            SccpInboundFinalityCutoffV1(
                trustAnchorHash: Data(repeating: 0x91, count: 32),
                maxAnchorIntervalHeight: 2
            )
        )

        var nonRetiredCutoff = history
        mutateRoute(&nonRetiredCutoff) {
            $0["inbound_finality_cutoff"] = [
                "trust_anchor_hash": upper(0x91, bytes: 32),
                "max_anchor_interval_height": 2,
            ]
        }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(nonRetiredCutoff)))

        var retiredWithoutCutoff = history
        mutateRoute(&retiredWithoutCutoff) {
            $0["activation"] = ["activation": "retired", "direction": NSNull()]
        }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(retiredWithoutCutoff)))

        var openEndedCutoff = retiredRoute
        mutateRoute(&openEndedCutoff) {
            $0["inbound_finality_cutoff"] = [
                "trust_anchor_hash": upper(0x92, bytes: 32),
                "max_anchor_interval_height": 3,
            ]
        }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(openEndedCutoff)))

        var incompleteInterval = retiredRoute
        mutateRoute(&incompleteInterval) {
            $0["inbound_finality_cutoff"] = [
                "trust_anchor_hash": upper(0x91, bytes: 32),
                "max_anchor_interval_height": 1,
            ]
        }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(incompleteInterval)))

        var stalePointer = history
        mutateLane(&stalePointer) { $0["current_native_trust_anchor_hash"] = upper(0x91, bytes: 32) }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(stalePointer)))

        var legacyAnchor = try jsonObject(valid)
        mutateLane(&legacyAnchor) {
            $0["native_trust_anchor"] = firstAnchor
            $0.removeValue(forKey: "native_trust_anchors")
            $0.removeValue(forKey: "current_native_trust_anchor_hash")
        }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(legacyAnchor)))

        var defaultDiscriminantCustody = try jsonObject(valid)
        let custodyKey = try Curve25519.Signing.PrivateKey(
            rawRepresentation: Data(repeating: 7, count: 32)
        )
        let custody753 = try AccountAddress.fromAccount(
            publicKey: custodyKey.publicKey.rawRepresentation
        ).toI105(networkPrefix: 753)
        mutateRoute(&defaultDiscriminantCustody) { route in
            var settlement = route["settlement"] as! [String: Any]
            settlement["custody_owner"] = custody753
            route["settlement"] = settlement
        }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(defaultDiscriminantCustody)))

        var duplicate = try jsonObject(valid)
        let duplicatedLane = laneObject(duplicate)
        duplicate["lanes"] = [duplicatedLane, duplicatedLane]
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(duplicate)))

        var exactAnchorBoundary = try jsonObject(valid)
        mutateLane(&exactAnchorBoundary) {
            $0["native_trust_anchors"] = Array(repeating: NSNull(), count: 4_096)
        }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(exactAnchorBoundary))) { error in
            XCTAssertFalse(String(describing: error).contains("exceeds 4,096"))
        }
        var overAnchorBoundary = exactAnchorBoundary
        mutateLane(&overAnchorBoundary) {
            $0["native_trust_anchors"] = Array(repeating: NSNull(), count: 4_097)
        }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(overAnchorBoundary))) { error in
            XCTAssertTrue(String(describing: error).contains("exceeds 4,096"))
        }

        var exactRouteBoundary = try jsonObject(valid)
        mutateLane(&exactRouteBoundary) {
            $0["routes"] = Array(repeating: [String: Any](), count: 64)
        }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(exactRouteBoundary))) { error in
            XCTAssertFalse(String(describing: error).contains("exceeds 64 retained"))
        }
        var overRouteBoundary = exactRouteBoundary
        mutateLane(&overRouteBoundary) {
            $0["routes"] = Array(repeating: [String: Any](), count: 65)
        }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(overRouteBoundary))) { error in
            XCTAssertTrue(String(describing: error).contains("exceeds 64 retained"))
        }
    }

    func testRegistryValidatesExactTonDeploymentAndRoleSeparation() throws {
        let canonical = try tonRegistryJSON()
        let parsed = try SccpRegistryV1.parse(canonical)
        XCTAssertEqual(parsed.lanes.count, 1)
        XCTAssertEqual(parsed.lanes[0].lane.source, .tonMainnet)
        XCTAssertEqual(parsed.lanes[0].routes[0].destination.family, .tonGroth16Bls12381)
        XCTAssertEqual(
            parsed.lanes[0].routes[0].destination.maxWrappedSupply,
            Self.tonRegistryMaxWrappedSupply
        )

        let canonicalRoute = parsed.lanes[0].routes[0]
        var changedInitialData = try jsonObject(canonical)
        mutateDeployment(&changedInitialData) {
            $0["jetton_master_initial_data_hash"] = upper(0x38, bytes: 32)
            $0["route_initial_data_hash"] = upper(0x39, bytes: 32)
        }
        let changed = try SccpRegistryV1.parse(jsonData(changedInitialData))
        let changedRoute = changed.lanes[0].routes[0]
        XCTAssertEqual(
            changedRoute.destination.destinationBindingHash,
            canonicalRoute.destination.destinationBindingHash,
            "pre-deployment TON binding must not feed StateInit data roots back into itself"
        )
        XCTAssertEqual(
            changedRoute.routeConfigurationHash,
            canonicalRoute.routeConfigurationHash,
            "pre-deployment TON route hash must not feed StateInit data roots back into itself"
        )

        var missingInitialData = try jsonObject(canonical)
        mutateDeployment(&missingInitialData) {
            $0.removeValue(forKey: "route_initial_data_hash")
        }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(missingInitialData)))

        var initialDataAlias = try jsonObject(canonical)
        var masterCodeHash: Any!
        mutateDeployment(&initialDataAlias) { masterCodeHash = $0["jetton_master_code_hash"] }
        mutateDeployment(&initialDataAlias) { $0["route_initial_data_hash"] = masterCodeHash }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(initialDataAlias)))

        var addressAlias = try jsonObject(canonical)
        var masterAddress: Any!
        mutateDeployment(&addressAlias) { masterAddress = $0["jetton_master_address"] }
        mutateEmitterIdentity(&addressAlias) { $0["address"] = masterAddress }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(addressAlias)))

        var codeAlias = try jsonObject(canonical)
        var masterCodeForEmitter: Any!
        mutateDeployment(&codeAlias) { masterCodeForEmitter = $0["jetton_master_code_hash"] }
        mutateEmitterIdentity(&codeAlias) { $0["code_hash"] = masterCodeForEmitter }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(codeAlias)))

        var wrongProfile = try jsonObject(canonical)
        mutateDeployment(&wrongProfile) { $0["proof_profile_commitment"] = upper(0x7f, bytes: 32) }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(wrongProfile)))

        var uncompressedKey = try jsonObject(canonical)
        mutateDeployment(&uncompressedKey) { deployment in
            var key = deployment["verifying_key"] as! [String: Any]
            key["alpha1"] = upper(1, bytes: 48)
            deployment["verifying_key"] = key
        }
        XCTAssertThrowsError(try SccpRegistryV1.parse(jsonData(uncompressedKey)))
    }

    func testTonRegistryPreservesExactUInt128CapAndRejectsRoundedOverflow() throws {
        let maximum = String(UInt128.max)
        let exact = try tonRegistryJSON(
            maxOutstandingLiability: maximum,
            maxWrappedSupply: maximum
        )
        let parsed = try SccpRegistryV1.parse(exact)
        XCTAssertEqual(parsed.lanes[0].routes[0].maxOutstandingLiability, maximum)
        XCTAssertEqual(parsed.lanes[0].routes[0].destination.maxWrappedSupply, maximum)

        // Foundation Decimal rounds UInt128.max + 1 down to this value. Pin the
        // route hash to that rounded value so this fixture would pass if the two
        // multiplier-1 wire integers were allowed through NSNumber first.
        let roundedByFoundation = "340282366920938463463374607431768211450"
        let overflow = "340282366920938463463374607431768211456"
        let roundedHashFixture = try tonRegistryJSON(
            maxOutstandingLiability: maximum,
            maxWrappedSupply: maximum,
            configurationMaxWrappedSupply: roundedByFoundation
        )
        let hostile = String(decoding: roundedHashFixture, as: UTF8.self)
            .replacingOccurrences(of: maximum, with: overflow)
        XCTAssertThrowsError(try SccpRegistryV1.parse(Data(hostile.utf8)))
    }

    func testProofRequestAndBundleAreClosedAndPolicyBound() throws {
        let request = try proofRequestJSON()
        let parsed = try SccpGroth16ProofRequestV1.parse(request)
        XCTAssertEqual(parsed.backend, .evmGroth16Bn254)
        XCTAssertEqual(parsed.targetNetwork, .bscMainnet)
        XCTAssertEqual(
            parsed.semanticProofProfileHash,
            "0x\(parsed.semanticProofProfile.profileHash.hexEncodedString())"
        )
        XCTAssertEqual(parsed.soraFinalityAnchor.anchorHash, finalityAnchor().hash)
        XCTAssertEqual(
            parsed.soraFinalityAnchorHash,
            "0x\(parsed.soraFinalityAnchor.anchorHash.hexEncodedString())"
        )

        var archivedIdentity = try jsonObject(request, mutableContainers: true)
        var archivedAnchor = archivedIdentity["sora_finality_anchor"] as! [String: Any]
        archivedAnchor["chain_id_hash"] = irohaKeccak256(
            Data(hexString: "809574f5fee75e69bfcf52451e42d50f")!
        ).hexEncodedString().uppercased()
        archivedIdentity["sora_finality_anchor"] = archivedAnchor
        XCTAssertThrowsError(try SccpGroth16ProofRequestV1.parse(jsonData(archivedIdentity)))

        let requestMutations: [(inout [String: Any]) -> Void] = [
            { $0["allow_unready"] = true },
            { $0["backend"] = ["backend": "solana_recursive_v1", "family": NSNull()] },
            { $0["sora_finality_anchor_hash"] = self.hashHex(0x99) },
            { $0["route_configuration_hash"] = $0["destination_binding_hash"] },
            { root in
                var key = root["verifying_key"] as! [String: Any]
                var ic = key["ic"] as! [String: Any]
                ic.removeValue(forKey: "signal_10")
                key["ic"] = ic
                root["verifying_key"] = key
            },
        ]
        for mutate in requestMutations {
            var object = try jsonObject(request, mutableContainers: true)
            mutate(&object)
            XCTAssertThrowsError(try SccpGroth16ProofRequestV1.parse(jsonData(object)))
        }

        var crossPolicyAlias = try jsonObject(request, mutableContainers: true)
        var semanticProfile = crossPolicyAlias["semantic_proof_profile"] as! [String: Any]
        var semanticCommitments = semanticProfile["commitments"] as! [String: Any]
        let anchor = crossPolicyAlias["sora_finality_anchor"] as! [String: Any]
        semanticCommitments["circuit_commitment"] = anchor["checkpoint_block_hash"]
        semanticProfile["commitments"] = semanticCommitments
        crossPolicyAlias["semantic_proof_profile"] = semanticProfile
        let circuit = Data(hexString: semanticCommitments["circuit_commitment"] as! String)!
        let witness = Data(hexString: semanticCommitments["witness_generator_commitment"] as! String)!
        let schema = Data(hexString: semanticCommitments["public_signal_schema_hash"] as! String)!
        let semanticHash = irohaKeccak256(
            Data("sccp:semantic-proof-profile:v1".utf8) + Data([1, 0, 1]) + circuit + witness + schema
        )
        crossPolicyAlias["semantic_proof_profile_hash"] = "0x\(semanticHash.hexEncodedString())"
        XCTAssertThrowsError(try SccpGroth16ProofRequestV1.parse(jsonData(crossPolicyAlias))) { error in
            XCTAssertTrue(String(describing: error).contains("proof-policy hash role"))
        }

        let bundle = bundleJSON(messageId: String(repeating: "11", count: 32))
        XCTAssertEqual(try SccpMessageBundleV1.parse(bundle).targetDomain, 2)
        var retiredPayload = try jsonObject(bundle)
        retiredPayload["payload"] = ["Burn": [:]]
        XCTAssertThrowsError(try SccpMessageBundleV1.parse(jsonData(retiredPayload)))
        var oldSelector = try jsonObject(bundle)
        oldSelector["network"] = "bsc-mainnet"
        XCTAssertThrowsError(try SccpMessageBundleV1.parse(jsonData(oldSelector)))
        let invalidTransferFields: [(String, Any)] = [
            ("sender_codec", 2),
            ("recipient_codec", 5),
            ("asset_home_domain", 4),
            ("amount", ""),
            ("amount", "340282366920938463463374607431768211456"),
            ("amount", "١"),
        ]
        for (field, invalid) in invalidTransferFields {
            var malformed = try jsonObject(bundle)
            var payload = malformed["payload"] as! [String: Any]
            var transfer = payload["Transfer"] as! [String: Any]
            transfer[field] = invalid
            payload["Transfer"] = transfer
            malformed["payload"] = payload
            XCTAssertThrowsError(try SccpMessageBundleV1.parse(jsonData(malformed)))
        }
    }

    func testRecentMessagesRejectRetiredLinksInjectionAliasesAndOrdering() throws {
        let first = recentItem(height: 9, id: String(repeating: "11", count: 32))
        let second = recentItem(height: 8, id: String(repeating: "12", count: 32))
        let page = try SccpRecentMessages.parse(jsonData(["items": [first, second]]))
        XCTAssertEqual(page.items.map(\.height), [9, 8])
        XCTAssertNil(page.next)

        let sameHeightFirst = recentItem(
            height: UInt64.max,
            id: String(repeating: "13", count: 32),
            commitmentIndex: 510
        )
        let sameHeightLast = recentItem(
            height: UInt64.max,
            id: String(repeating: "14", count: 32),
            commitmentIndex: 511
        )
        let continued = try SccpRecentMessages.parse(jsonData([
            "items": [sameHeightFirst, sameHeightLast],
            "next": ["from": UInt64.max, "after_index": 511],
        ]))
        XCTAssertEqual(continued.items.map(\.commitmentIndex), [510, 511])
        XCTAssertEqual(continued.next, SccpRecentCursor(from: UInt64.max, afterIndex: 511))

        var retired = first
        var retiredLinks = retired["links"] as! [String: Any]
        retiredLinks["artifact_path"] = "/v1/sccp/artifacts/message/\(String(repeating: "11", count: 32))"
        retired["links"] = retiredLinks
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [retired]])))

        var injection = first
        var injectionLinks = injection["links"] as! [String: Any]
        injectionLinks["bundle_path"] = (injectionLinks["bundle_path"] as! String) + "?allow_unready=true"
        injection["links"] = injectionLinks
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [injection]])))
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [second, first]])))
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [first, first]])))

        var missingIndex = first
        missingIndex.removeValue(forKey: "commitment_index")
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [missingIndex]])))
        var outOfRangeIndex = first
        outOfRangeIndex["commitment_index"] = 512
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [outOfRangeIndex]])))
        var unknownItemField = first
        unknownItemField["commitment_position"] = 0
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [unknownItemField]])))
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData([
            "items": [first], "cursor": ["from": 9, "after_index": 0],
        ])))
        for indices in [[1, 1], [1, 3], [2, 1]] {
            let sameHeight = [
                recentItem(
                    height: 9,
                    id: String(repeating: "15", count: 32),
                    commitmentIndex: UInt32(indices[0])
                ),
                recentItem(
                    height: 9,
                    id: String(repeating: "16", count: 32),
                    commitmentIndex: UInt32(indices[1])
                ),
            ]
            XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": sameHeight])))
        }
        let olderStartsMidBlock = recentItem(
            height: 8, id: String(repeating: "17", count: 32), commitmentIndex: 1
        )
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData([
            "items": [first, olderStartsMidBlock],
        ])))
        let cursorAttacks: [Any] = [
            NSNull(),
            ["from": 9, "after_index": 1],
            ["from": 9, "after_index": 0, "extra": 0],
            ["from": 9, "after_index": 512],
        ]
        for next in cursorAttacks {
            XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData([
                "items": [first], "next": next,
            ])))
        }
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData([
            "items": [], "next": ["from": 9, "after_index": 0],
        ])))

        var missingProjection = first
        missingProjection.removeValue(forKey: "payload_projection")
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [missingProjection]])))
        var nullProjection = first
        nullProjection["payload_projection"] = NSNull()
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [nullProjection]])))
        var wrongProjectionDomain = first
        var projection = wrongProjectionDomain["payload_projection"] as! [String: Any]
        var projectedTransfer = projection["Transfer"] as! [String: Any]
        projectedTransfer["dest_domain"] = 5
        projection["Transfer"] = projectedTransfer
        wrongProjectionDomain["payload_projection"] = projection
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [wrongProjectionDomain]])))
        var unicodeBindingHash = first
        unicodeBindingHash["destination_binding_hash"] = "0x" + String(repeating: "١", count: 64)
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [unicodeBindingHash]])))
        var wrongProjectionRoute = first
        projection = wrongProjectionRoute["payload_projection"] as! [String: Any]
        projectedTransfer = projection["Transfer"] as! [String: Any]
        projectedTransfer["route_id"] = ["CanonicalText": ["value": "taira_eth_xor"]]
        projection["Transfer"] = projectedTransfer
        wrongProjectionRoute["payload_projection"] = projection
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [wrongProjectionRoute]])))
        var zeroProjectionAmount = first
        projection = zeroProjectionAmount["payload_projection"] as! [String: Any]
        projectedTransfer = projection["Transfer"] as! [String: Any]
        projectedTransfer["amount"] = 0
        projection["Transfer"] = projectedTransfer
        zeroProjectionAmount["payload_projection"] = projection
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [zeroProjectionAmount]])))
        var unicodeProjectionAddress = first
        projection = unicodeProjectionAddress["payload_projection"] as! [String: Any]
        projectedTransfer = projection["Transfer"] as! [String: Any]
        projectedTransfer["recipient"] = [
            "EvmAddress20": ["bytes": "0x" + String(repeating: "١", count: 40)],
        ]
        projection["Transfer"] = projectedTransfer
        unicodeProjectionAddress["payload_projection"] = projection
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [unicodeProjectionAddress]])))
        var wrongSummaryAsset = first
        wrongSummaryAsset["asset_id"] = "other"
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [wrongSummaryAsset]])))
        var wrongSummaryRoute = first
        wrongSummaryRoute["route_id"] = "taira_eth_xor"
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [wrongSummaryRoute]])))
        var impossibleSummaryRecipient = first
        impossibleSummaryRecipient["recipient"] = "0x" + String(repeating: "11", count: 20)
        XCTAssertThrowsError(try SccpRecentMessages.parse(jsonData(["items": [impossibleSummaryRecipient]])))
    }

    func testToriiClientUsesOnlyExactQueryFreeProofPathsAndRejectsInjectionBeforeFetch() async throws {
        var observed: [String] = []
        SccpStubURLProtocol.handler = { request in
            observed.append(request.url!.absoluteString)
            let path = request.url!.path
            let data: Data
            if path == "/v1/sccp/capabilities" { data = self.capabilitiesJSON() }
            else if path == "/v1/sccp/registry" { data = try self.registryJSON(empty: true) }
            else if path.contains("proof-requests") { data = try self.proofRequestJSON() }
            else if path.contains("proofs/message") { data = self.bundleJSON(messageId: String(repeating: "11", count: 32)) }
            else { data = self.jsonData(["items": []]) }
            let response = HTTPURLResponse(
                url: request.url!, statusCode: 200, httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, data)
        }
        let client = makeClient()
        let id = String(repeating: "11", count: 32)
        _ = try await client.getSccpCapabilities()
        _ = try await client.getSccpRegistry()
        _ = try await client.getSccpMessageBundle(messageIdHex: id)
        _ = try await client.getSccpProofRequest(messageIdHex: id)
        _ = try await client.getSccpRecentMessages(from: UInt64.max, afterIndex: 511, limit: 1)
        XCTAssertEqual(observed.map { URL(string: $0)!.path }, [
            "/v1/sccp/capabilities", "/v1/sccp/registry", "/v1/sccp/proofs/message/\(id)",
            "/v1/sccp/proof-requests/\(id)", "/v1/sccp/messages/recent",
        ])
        let recentQuery = URLComponents(string: observed.last!)!.queryItems!
        XCTAssertEqual(
            Dictionary(uniqueKeysWithValues: recentQuery.map { ($0.name, $0.value!) }),
            ["from": String(UInt64.max), "after_index": "511", "limit": "1"]
        )
        let calls = observed.count
        for attack in [
            "0x\(id)", String(repeating: "AB", count: 32), "\(id)?network=bsc",
            "\(id)/../registry", String(repeating: "0", count: 64),
        ] {
            do {
                _ = try await client.getSccpProofRequest(messageIdHex: attack)
                XCTFail("accepted injected message id")
            } catch {}
        }
        for query in [(from: UInt64(0), limit: UInt32(1)), (from: UInt64(1), limit: UInt32(0))] {
            do {
                _ = try await client.getSccpRecentMessages(from: query.from, limit: query.limit)
                XCTFail("accepted a zero-valued recent-message query")
            } catch {}
        }
        do {
            _ = try await client.getSccpRecentMessages(afterIndex: 0)
            XCTFail("accepted an unpaired recent-message cursor")
        } catch {}
        do {
            _ = try await client.getSccpRecentMessages(from: 1, afterIndex: 512)
            XCTFail("accepted an out-of-range recent-message cursor")
        } catch {}
        XCTAssertEqual(observed.count, calls)
    }

    func testToriiClientNativeReadsRequireExactSchemaAndZeroPadding() async throws {
        let id = String(repeating: "11", count: 32)
        SccpStubURLProtocol.handler = { request in
            let typeName: String
            if request.url!.path == "/v1/sccp/registry" {
                typeName = SccpSubmitValidation.registryTypeName
            } else if request.url!.path.contains("proof-requests") {
                typeName = SccpSubmitValidation.bn254ProofRequestTypeName
            } else {
                typeName = SccpSubmitValidation.messageBundleTypeName
            }
            let body = noritoEncode(typeName: typeName, payload: Data([1]))
            let response = HTTPURLResponse(
                url: request.url!, statusCode: 200, httpVersion: nil,
                headerFields: ["Content-Type": "application/x-norito"]
            )!
            return (response, body)
        }
        let client = makeClient()
        _ = try await client.getSccpRegistryNorito()
        _ = try await client.getSccpMessageBundleNorito(messageIdHex: id)
        _ = try await client.getSccpProofRequestNorito(messageIdHex: id)

        SccpStubURLProtocol.handler = { request in
            let body = noritoEncode(
                typeName: SccpSubmitValidation.tonProofRequestTypeName,
                payload: Data([1])
            )
            let response = HTTPURLResponse(
                url: request.url!, statusCode: 200, httpVersion: nil,
                headerFields: ["Content-Type": "application/x-norito"]
            )!
            return (response, body)
        }
        _ = try await client.getSccpProofRequestNorito(messageIdHex: id)

        let wrongSchema = noritoEncode(
            typeName: SccpSubmitValidation.destinationArtifactTypeName,
            payload: Data([1])
        )
        let unknownSchema = noritoEncode(
            typeName: "example::UnknownProofRequestV1",
            payload: Data([1])
        )
        var paddedMessage = Data(
            noritoEncode(
                typeName: SccpSubmitValidation.messageBundleTypeName,
                payload: Data([1])
            ).prefix(NoritoHeader.encodedLength)
        )
        paddedMessage.append(Data(repeating: 0, count: 8))
        paddedMessage.append(
            noritoEncode(
                typeName: SccpSubmitValidation.messageBundleTypeName,
                payload: Data([1])
            ).dropFirst(NoritoHeader.encodedLength)
        )
        for (body, operation) in [
            (wrongSchema, { try await client.getSccpRegistryNorito() }),
            (wrongSchema, { try await client.getSccpMessageBundleNorito(messageIdHex: id) }),
            (wrongSchema, { try await client.getSccpProofRequestNorito(messageIdHex: id) }),
            (unknownSchema, { try await client.getSccpProofRequestNorito(messageIdHex: id) }),
            (paddedMessage, { try await client.getSccpMessageBundleNorito(messageIdHex: id) }),
        ] {
            SccpStubURLProtocol.handler = { request in
                let response = HTTPURLResponse(
                    url: request.url!, statusCode: 200, httpVersion: nil,
                    headerFields: ["Content-Type": "application/x-norito"]
                )!
                return (response, body)
            }
            do {
                _ = try await operation()
                XCTFail("accepted a wrong-schema or padded SCCP Norito response")
            } catch {
                // Expected.
            }
        }
    }

    func testToriiClientAcceptsExactCapabilityResponseLimit() async throws {
        let maximumBytes = 64 * 1024
        var body = capabilitiesJSON()
        XCTAssertLessThan(body.count, maximumBytes)
        body.append(Data(repeating: 0x20, count: maximumBytes - body.count))
        SccpStubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!, statusCode: 200, httpVersion: nil,
                headerFields: [
                    "Content-Type": "application/json",
                    "Content-Length": String(maximumBytes),
                ]
            )!
            return (response, body)
        }

        let capabilities = try await makeClient().getSccpCapabilities()
        XCTAssertEqual(capabilities.version, 1)
    }

    func testToriiClientRejectsDeclaredAndActualCapabilityResponseOverflow() async {
        let maximumBytes = 64 * 1024
        let valid = capabilitiesJSON()
        SccpStubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!, statusCode: 200, httpVersion: nil,
                headerFields: [
                    "Content-Type": "application/json",
                    "Content-Length": String(maximumBytes + 1),
                ]
            )!
            return (response, valid)
        }
        await assertCapabilitiesFetchRejected()

        var oversized = valid
        oversized.append(Data(repeating: 0x20, count: maximumBytes + 1 - oversized.count))
        SccpStubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!, statusCode: 200, httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, oversized)
        }
        await assertCapabilitiesFetchRejected()

        SccpStubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!, statusCode: 200, httpVersion: nil,
                headerFields: [
                    "Content-Type": "application/json",
                    "Content-Length": "1",
                ]
            )!
            return (response, oversized)
        }
        await assertCapabilitiesFetchRejected()
    }

    func testToriiClientRejectsMalformedNoncanonicalAndLyingContentLength() async {
        let valid = capabilitiesJSON()
        for declaredLength in ["01", "+1", "1, 1", "18446744073709551616"] {
            SccpStubURLProtocol.handler = { request in
                let response = HTTPURLResponse(
                    url: request.url!, statusCode: 200, httpVersion: nil,
                    headerFields: [
                        "Content-Type": "application/json",
                        "Content-Length": declaredLength,
                    ]
                )!
                return (response, valid)
            }
            await assertCapabilitiesFetchRejected()
        }

        SccpStubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!, statusCode: 200, httpVersion: nil,
                headerFields: [
                    "Content-Type": "application/json",
                    "Content-Length": "1",
                ]
            )!
            return (response, valid)
        }
        await assertCapabilitiesFetchRejected()
    }

    func testToriiClientRejectsNoritoRegistryDeclaredAboveNativeArtifactLimit() async {
        SccpStubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!, statusCode: 200, httpVersion: nil,
                headerFields: [
                    "Content-Type": "application/x-norito",
                    "Content-Length": String(16 * 1024 * 1024 + 1),
                ]
            )!
            return (response, Data())
        }

        do {
            _ = try await makeClient().getSccpRegistryNorito()
            XCTFail("accepted a Norito SCCP registry declared above the native artifact limit")
        } catch let ToriiClientError.invalidPayload(reason) {
            XCTAssertTrue(reason.contains("16777216-byte limit"))
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    func testUnifiedBridgeResponseRejectsContradictionsDuplicatesAndLegacyFields() throws {
        let valid = responseJSON(submitted: true, txHash: String(repeating: "3", count: 64), transactionPayload: nil, signingMessage: nil)
        XCTAssertTrue(try SccpBridgeSubmitResponse.parse(valid).submitted)
        let text = String(data: valid, encoding: .utf8)!
        var alternateBackend = try jsonObject(valid, mutableContainers: true)
        alternateBackend["backend"] = "evm-groth16-bn254-v1"
        XCTAssertNoThrow(try SccpBridgeSubmitResponse.parse(jsonData(alternateBackend)))
        for legacy in ["ok", "proof_kind", "message_kind", "manifest_hash_hex", "transaction_scaffold_b64", "signed_transaction_b64", "proof_artifact_hash"] {
            XCTAssertThrowsError(try SccpBridgeSubmitResponse.parse(Data((text.dropLast() + ",\"\(legacy)\":null}").utf8)))
        }
        XCTAssertThrowsError(try SccpBridgeSubmitResponse.parse(Data(text.replacingOccurrences(
            of: "\"submitted\":true", with: "\"submitted\":true,\"submitted\":false"
        ).utf8)))
        XCTAssertThrowsError(try SccpBridgeSubmitResponse.parse(Data(text.replacingOccurrences(
            of: "\"counterparty_chain\":\"bsc-mainnet\"", with: "\"counterparty_chain\":\"solana-mainnet-beta\""
        ).utf8)))
        var invalidBackend = try jsonObject(valid, mutableContainers: true)
        invalidBackend["backend"] = "bridge/caller-chosen"
        XCTAssertThrowsError(try SccpBridgeSubmitResponse.parse(jsonData(invalidBackend)))
        var crossFamilyBackend = try jsonObject(valid, mutableContainers: true)
        crossFamilyBackend["backend"] = "tron-groth16-bn254-v1"
        XCTAssertThrowsError(try SccpBridgeSubmitResponse.parse(jsonData(crossFamilyBackend)))
    }

    private func capabilitiesJSON() -> Data {
        jsonData([
            "version": 1,
            "registry_revision": hashHex(0x10),
            "registry_path": "/v1/sccp/registry",
            "message_bundle_path": "/v1/sccp/proofs/message/{message_id}",
            "proof_request_path": "/v1/sccp/proof-requests/{message_id}",
            "recent_messages_path": "/v1/sccp/messages/recent",
            "registry_limits": [
                "max_governed_lanes": 16,
                "max_live_governed_routes": 64,
                "max_live_routes_per_lane": 8,
                "max_retained_routes_per_lane": 64,
                "max_retained_native_trust_anchors_per_lane": 4_096,
            ],
            "resource_limits": [
                "max_outbound_messages_per_block": 512,
                "max_outbound_message_payload_bytes": 4_096,
                "max_pending_outbound_messages": 65_536,
                "max_pending_outbound_payload_bytes": 256 * 1024 * 1024,
                "max_proofs_per_transaction": 1,
                "max_proofs_per_block": 4,
                "max_proof_bytes_per_proof": 8 * 1024 * 1024,
                "max_proof_bytes_per_transaction": 8 * 1024 * 1024,
                "max_proof_bytes_per_block": 32 * 1024 * 1024,
                "max_native_headers_per_transaction": 1_004,
                "max_native_headers_per_block": 4_016,
                "max_ethereum_light_client_updates_per_transaction": 128,
                "max_ethereum_light_client_updates_per_block": 512,
                "max_native_header_bytes_per_transaction": 8 * 1024 * 1024,
                "max_native_header_bytes_per_block": 32 * 1024 * 1024,
                "max_secp256k1_recoveries_per_transaction": 1_005,
                "max_secp256k1_recoveries_per_block": 4_020,
                "max_bls_aggregate_checks_per_transaction": 1_004,
                "max_bls_aggregate_checks_per_block": 4_016,
                "max_bls_signer_contributions_per_transaction": 131_713,
                "max_bls_signer_contributions_per_block": 526_852,
                "max_ed25519_signature_checks_per_transaction": 65_536,
                "max_ed25519_signature_checks_per_block": 262_144,
                "max_ed25519_validator_key_checks_per_transaction": 198_656,
                "max_ed25519_validator_key_checks_per_block": 794_624,
                "max_bn254_pairing_checks_per_transaction": 1,
                "max_bn254_pairing_checks_per_block": 4,
                "max_bls12_381_pairing_checks_per_transaction": 1,
                "max_bls12_381_pairing_checks_per_block": 4,
            ],
            "proof_submit_path": "/v1/bridge/proofs/submit",
            "native_message_submit_path": "/v1/bridge/messages",
        ])
    }

    private func tonRegistryJSON(
        maxOutstandingLiability requestedLiability: String? = nil,
        maxWrappedSupply requestedCap: String? = nil,
        configurationMaxWrappedSupply requestedConfigurationCap: String? = nil
    ) throws -> Data {
        let maxOutstandingLiability = requestedLiability
            ?? Self.registryMaxOutstandingLiability
        let maxWrappedSupply = requestedCap ?? maxOutstandingLiability
        let configurationMaxWrappedSupply = requestedConfigurationCap ?? maxWrappedSupply
        let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 7, count: 32))
        let custody = try AccountAddress.fromAccount(publicKey: privateKey.publicKey.rawRepresentation)
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let key = bls12381VerifyingKey()
        let keyHash = Data(SHA256.hash(data: bls12381VerifyingKeyBytes(key)))
        let policy = tonOutboundPolicy()
        let semantic = policy["semantic_profile"] as! [String: Any]
        let semanticHash = tonSemanticProfileHash(semantic)
        let anchorHash = finalityAnchor().hash
        let master = tonAddress(0x21)
        let routeAddress = tonAddress(0x23)
        let masterCode = Data(repeating: 0x31, count: 32)
        let masterInitialData = Data(repeating: 0x36, count: 32)
        let walletCode = Data(repeating: 0x32, count: 32)
        let routeCode = Data(repeating: 0x34, count: 32)
        let routeInitialData = Data(repeating: 0x37, count: 32)
        let verifierCode = Data(repeating: 0x35, count: 32)
        let commitments = semantic["commitments"] as! [String: Any]
        let circuit = Data(hexString: commitments["circuit_commitment"] as! String)!
        let proofProfile = tonProofProfileCommitment()
        let binding = tonDestinationBinding(
            masterCode: masterCode,
            walletCode: walletCode,
            routeCode: routeCode,
            verifierCode: verifierCode,
            circuit: circuit,
            keyHash: keyHash,
            proofProfile: proofProfile,
            semanticHash: semanticHash,
            anchorHash: anchorHash
        )
        let inbound = try SccpLaneIdV1(source: .tonMainnet, target: .soraTaira)
        let configuration = tonRouteConfiguration(
            masterCode: masterCode,
            walletCode: walletCode,
            routeCode: routeCode,
            verifierCode: verifierCode,
            circuit: circuit,
            keyHash: keyHash,
            proofProfile: proofProfile,
            semanticHash: semanticHash,
            anchorHash: anchorHash,
            binding: binding,
            lane: inbound,
            revision: 1,
            maxWrappedSupply: configurationMaxWrappedSupply
        )
        let route: [String: Any] = [
            "lane_id": lane("ton-mainnet", "sora-taira"),
            "route_id": "taira_ton_xor",
            "asset_key": "xor",
            "revision": 1,
            "activation": ["activation": "staged", "direction": NSNull()],
            "inbound_finality_cutoff": NSNull(),
            "source_identity": [
                "lane": lane("ton-mainnet", "sora-taira"),
                "emitter": [
                    "emitter": "ton",
                    "identity": [
                        "address": routeAddress,
                        "code_hash": routeCode.hexEncodedString().uppercased(),
                        "route_config_hash": configuration.hexEncodedString().uppercased(),
                    ],
                ],
            ],
            "destination": [
                "family": "ton",
                "deployment": [
                    "jetton_master_address": master,
                    "jetton_master_code_hash": masterCode.hexEncodedString().uppercased(),
                    "jetton_master_initial_data_hash": masterInitialData.hexEncodedString().uppercased(),
                    "jetton_wallet_code_hash": walletCode.hexEncodedString().uppercased(),
                    "route_address": routeAddress,
                    "route_code_hash": routeCode.hexEncodedString().uppercased(),
                    "route_initial_data_hash": routeInitialData.hexEncodedString().uppercased(),
                    "embedded_verifier_code_hash": verifierCode.hexEncodedString().uppercased(),
                    "verifier_circuit_hash": circuit.hexEncodedString().uppercased(),
                    "verifying_key": key,
                    "verifier_key_hash": keyHash.hexEncodedString().uppercased(),
                    "proof_profile_commitment": proofProfile.hexEncodedString().uppercased(),
                    "outbound_proof_policy": policy,
                    "taira_to_token_multiplier": 1,
                    "max_wrapped_supply": NSDecimalNumber(
                        string: maxWrappedSupply
                    ),
                ],
            ],
            "settlement": [
                "asset_definition_id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
                "custody_owner": custody,
                "payload_amount_scale": 9,
                "max_outstanding_liability": NSDecimalNumber(
                    string: maxOutstandingLiability
                ),
            ],
        ]
        return jsonData([
            "version": 1,
            "lanes": [[
                "lane_id": lane("ton-mainnet", "sora-taira"),
                "native_trust_anchors": [],
                "current_native_trust_anchor_hash": NSNull(),
                "routes": [route],
            ]],
        ])
    }

    private func registryJSON(
        empty: Bool = false,
        source: SccpNetworkV1 = .bscMainnet,
        aliasTronBindingWithTokenCodeHash: Bool = false
    ) throws -> Data {
        if empty { return jsonData(["version": 1, "lanes": []]) }
        let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 7, count: 32))
        let custody = try AccountAddress.fromAccount(publicKey: privateKey.publicKey.rawRepresentation)
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let key = verifyingKey()
        let policy = outboundPolicy()
        var destination = destinationValues(key: key, policy: policy)
        let inbound = try SccpLaneIdV1(source: source, target: .soraTaira)
        let semanticHash = policyHashes().semantic
        let anchorHash = policyHashes().anchor
        let binding = destinationBinding(
            destination: destination,
            semanticHash: semanticHash,
            anchorHash: anchorHash,
            source: source
        )
        if aliasTronBindingWithTokenCodeHash {
            destination["token_code_hash"] = binding.hexEncodedString().uppercased()
        }
        let configuration = routeConfiguration(destination: destination, semanticHash: semanticHash, anchorHash: anchorHash, binding: binding, lane: inbound)
        let isTron = source.rawValue.hasPrefix("tron-")
        let routeId = source.rawValue.hasPrefix("ethereum-") ? "taira_eth_xor" :
            isTron ? "taira_tron_xor" : "taira_bsc_xor"
        let route: [String: Any] = [
            "lane_id": lane(source.rawValue, "sora-taira"),
            "route_id": routeId,
            "asset_key": "xor",
            "revision": 1,
            "activation": ["activation": "staged", "direction": NSNull()],
            "inbound_finality_cutoff": NSNull(),
            "source_identity": [
                "lane": lane(source.rawValue, "sora-taira"),
                "emitter": [
                    "emitter": isTron ? "tron" : "evm",
                    "identity": [
                        "address": destination["route_address"]!,
                        "runtime_code_hash": destination["route_code_hash"]!,
                        "route_config_hash": configuration.hexEncodedString().uppercased(),
                    ],
                ],
            ],
            "destination": ["family": isTron ? "tron" : "evm", "deployment": destination],
            "settlement": [
                "asset_definition_id": "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
                "custody_owner": custody,
                "payload_amount_scale": 9,
                "max_outstanding_liability": NSDecimalNumber(
                    string: Self.registryMaxOutstandingLiability
                ),
            ],
        ]
        return jsonData([
            "version": 1,
            "lanes": [[
                "lane_id": lane(source.rawValue, "sora-taira"),
                "native_trust_anchors": [],
                "current_native_trust_anchor_hash": NSNull(),
                "routes": [route],
            ]],
        ])
    }

    private func bundleJSON(messageId: String) -> Data {
        let target = lane("sora-taira", "bsc-mainnet")
        return jsonData([
            "version": 1,
            "commitment_root": hashHex(0x13),
            "commitment": [
                "version": 1,
                "kind": "Transfer",
                "context": [
                    "lane": target,
                    "destination_binding_hash": hashHex(0x71),
                    "route_configuration_hash": hashHex(0x72),
                ],
                "message_id": "0x\(messageId)",
                "payload_hash": hashHex(0x12),
            ],
            "merkle_proof": ["steps": []],
            "payload": ["Transfer": transferPayload()],
            "finality_proof": "0x01",
        ])
    }

    private func proofRequestJSON(protocolVersion: Int = 4) throws -> Data {
        let key = verifyingKey()
        let hashes = policyHashes()
        let anchor = finalityAnchor(protocolVersion: protocolVersion)
        return jsonData([
            "version": 1,
            "backend": ["backend": "evm_groth16_bn254_v1", "family": NSNull()],
            "source_network": network("sora-taira"),
            "target_network": network("bsc-mainnet"),
            "public_inputs": [
                "version": 1,
                "message_id": hashHex(0x11),
                "payload_hash": hashHex(0x12),
                "target_domain": 2,
                "commitment_root": hashHex(0x13),
                "finality_height": "9",
                "finality_block_hash": hashHex(0x14),
            ],
            "verifying_key": key,
            "verifier_key_hash": "0x\(irohaKeccak256(verifyingKeyBytes(key)).hexEncodedString())",
            "semantic_proof_profile": outboundPolicy()["semantic_profile"]!,
            "semantic_proof_profile_hash": "0x\(hashes.semantic.hexEncodedString())",
            "sora_finality_anchor": anchor.object,
            "sora_finality_anchor_hash": "0x\(anchor.hash.hexEncodedString())",
            "bundle_bytes": "0x0102",
            "statement_hash": hashHex(0x61),
            "destination_binding_hash": hashHex(0x62),
            "route_configuration_hash": hashHex(0x63),
            "request_hash": hashHex(0x64),
        ])
    }

    private func recentItem(
        height: UInt64,
        id: String,
        commitmentIndex: UInt32 = 0
    ) -> [String: Any] {
        [
            "height": height,
            "commitment_index": commitmentIndex,
            "message_id_hex": id,
            "kind": "transfer",
            "source_profile": "sora-taira",
            "target_profile": "bsc-mainnet",
            "destination_binding_hash": hashHex(0x71),
            "route_configuration_hash": hashHex(0x72),
            "target_domain": 2,
            "asset_id": "xor",
            "route_id": "taira_bsc_xor",
            "recipient": NSNull(),
            "amount": "1000",
            "payload_projection": transferProjection(destinationDomain: 2),
            "links": [
                "bundle_path": "/v1/sccp/proofs/message/\(id)",
                "proof_request_path": "/v1/sccp/proof-requests/\(id)",
            ],
        ]
    }

    private func transferPayload() -> [String: Any] {
        [
            "version": 1, "source_domain": 0, "dest_domain": 2, "nonce": "7",
            "route_revision": 1, "asset_home_domain": 0,
            "asset_id_codec": 1, "asset_id": "0x786f72", "amount": "1000",
            "sender_codec": 1, "sender": "0x616c696365407461697261",
            "recipient_codec": 2, "recipient": "0x" + String(repeating: "11", count: 20),
            "route_id_codec": 1, "route_id": "0x74616972615f6273635f786f72",
        ]
    }

    private func transferProjection(destinationDomain: UInt32) -> [String: Any] {
        let route = destinationDomain == 5 ? "taira_tron_xor" :
            destinationDomain == 1 ? "taira_eth_xor" : "taira_bsc_xor"
        let recipient: [String: Any] = destinationDomain == 5
            ? ["TronAddress21": ["bytes": "0x41" + String(repeating: "11", count: 20)]]
            : ["EvmAddress20": ["bytes": "0x" + String(repeating: "11", count: 20)]]
        return [
            "Transfer": [
                "version": 1,
                "source_domain": 0,
                "dest_domain": destinationDomain,
                "nonce": 7,
                "route_revision": 1,
                "asset_home_domain": 0,
                "asset_id": ["CanonicalText": ["value": "xor"]],
                "amount": 1000,
                "sender": ["CanonicalText": ["value": "alice@taira"]],
                "recipient": recipient,
                "route_id": ["CanonicalText": ["value": route]],
            ],
        ]
    }

    private func outboundPolicy() -> [String: Any] {
        let anchor = finalityAnchor()
        return [
            "version": 1,
            "semantic_profile": [
                "profile": "sora_taira_finality_inclusion_groth16_bn254",
                "commitments": [
                    "version": 1,
                    "circuit_commitment": upper(0xc1, bytes: 32),
                    "witness_generator_commitment": upper(0xc2, bytes: 32),
                    "public_signal_schema_hash": publicSignalSchemaHash().hexEncodedString().uppercased(),
                ],
            ],
            "sora_finality_anchor": anchor.object,
        ]
    }

    private func destinationValues(key: [String: Any], policy: [String: Any]) -> [String: Any] {
        [
            "token_address": upper(0x11, bytes: 20),
            "token_code_hash": upper(0x21, bytes: 32),
            "verifier_address": upper(0x12, bytes: 20),
            "verifier_code_hash": upper(0x22, bytes: 32),
            "verifying_key": key,
            "verifier_key_hash": irohaKeccak256(verifyingKeyBytes(key)).hexEncodedString().uppercased(),
            "outbound_proof_policy": policy,
            "route_address": upper(0x31, bytes: 20),
            "route_code_hash": upper(0x41, bytes: 32),
            "taira_to_token_multiplier": 1_000_000_000,
            "max_wrapped_supply": NSDecimalNumber(
                string: Self.evmRegistryMaxWrappedSupply
            ),
        ]
    }

    private func verifyingKey() -> [String: Any] {
        var ic: [String: Any] = ["constant": g1()]
        for index in 0...10 { ic["signal_\(index)"] = g1() }
        return ["version": 1, "alpha1": g1(), "beta2": g2(), "gamma2": g2(), "delta2": g2(), "ic": ic]
    }

    private func g1() -> [String: Any] { ["x": upper(1, bytes: 32), "y": upper(2, bytes: 32)] }
    private func g2() -> [String: Any] {
        ["x_c0": upper(3, bytes: 32), "x_c1": upper(4, bytes: 32), "y_c0": upper(5, bytes: 32), "y_c1": upper(6, bytes: 32)]
    }

    private func verifyingKeyBytes(_ key: [String: Any]) -> Data {
        var out = Data()
        func addG1(_ point: [String: Any]) {
            out.append(Data(hexString: point["x"] as! String)!)
            out.append(Data(hexString: point["y"] as! String)!)
        }
        func addG2(_ point: [String: Any]) {
            for field in ["x_c0", "x_c1", "y_c0", "y_c1"] { out.append(Data(hexString: point[field] as! String)!) }
        }
        addG1(key["alpha1"] as! [String: Any])
        for field in ["beta2", "gamma2", "delta2"] { addG2(key[field] as! [String: Any]) }
        let ic = key["ic"] as! [String: Any]
        addG1(ic["constant"] as! [String: Any])
        for index in 0...10 { addG1(ic["signal_\(index)"] as! [String: Any]) }
        return out
    }

    private func bls12381VerifyingKey() -> [String: Any] {
        let g1 = "80" + String(repeating: "0", count: 94)
        let g2 = "80" + String(repeating: "0", count: 190)
        var ic: [String: Any] = ["constant": g1]
        for index in 0...10 { ic["signal_\(index)"] = g1 }
        return [
            "version": 1,
            "alpha1": g1,
            "beta2": g2,
            "gamma2": g2,
            "delta2": g2,
            "ic": ic,
        ]
    }

    private func bls12381VerifyingKeyBytes(_ key: [String: Any]) -> Data {
        var out = Data([1])
        for field in ["alpha1", "beta2", "gamma2", "delta2"] {
            out.append(Data(hexString: key[field] as! String)!)
        }
        let ic = key["ic"] as! [String: Any]
        out.append(Data(hexString: ic["constant"] as! String)!)
        for index in 0...10 {
            out.append(Data(hexString: ic["signal_\(index)"] as! String)!)
        }
        return out
    }

    private func bls12381SignalLabels() -> [String] {
        [
            "sccp:groth16-bls12381:signal:message-id:v1",
            "sccp:groth16-bls12381:signal:payload-hash:v1",
            "sccp:groth16-bls12381:signal:target-domain:v1",
            "sccp:groth16-bls12381:signal:commitment-root:v1",
            "sccp:groth16-bls12381:signal:finality-height:v1",
            "sccp:groth16-bls12381:signal:finality-block-hash:v1",
            "sccp:groth16-bls12381:signal:source-domain:v1",
            "sccp:groth16-bls12381:signal:statement-hash:v1",
            "sccp:groth16-bls12381:signal:destination-binding-hash:v1",
            "sccp:groth16-bls12381:signal:route-config-hash:v1",
            "sccp:groth16-bls12381:signal:sora-finality-anchor-hash:v1",
        ]
    }

    private func bls12381PublicSignalSchemaHash() -> Data {
        var canonical = Data([1])
        appendUInt32LE(UInt32(bls12381SignalLabels().count), to: &canonical)
        for label in bls12381SignalLabels() {
            appendVector(Data(label.utf8), to: &canonical)
        }
        return Data(SHA256.hash(
            data: Data("sccp:groth16-bls12381:public-signal-schema:v1".utf8) + canonical
        ))
    }

    private func tonOutboundPolicy() -> [String: Any] {
        [
            "version": 1,
            "semantic_profile": [
                "profile": "sora_taira_finality_inclusion_groth16_bls12381",
                "commitments": [
                    "version": 1,
                    "circuit_commitment": upper(0xc1, bytes: 32),
                    "witness_generator_commitment": upper(0xc2, bytes: 32),
                    "public_signal_schema_hash": bls12381PublicSignalSchemaHash()
                        .hexEncodedString().uppercased(),
                ],
            ],
            "sora_finality_anchor": finalityAnchor().object,
        ]
    }

    private func tonSemanticProfileHash(_ semantic: [String: Any]) -> Data {
        let commitments = semantic["commitments"] as! [String: Any]
        return irohaKeccak256(
            Data("sccp:semantic-proof-profile:v1".utf8) + Data([1, 1, 1]) +
                Data(hexString: commitments["circuit_commitment"] as! String)! +
                Data(hexString: commitments["witness_generator_commitment"] as! String)! +
                Data(hexString: commitments["public_signal_schema_hash"] as! String)!
        )
    }

    private func tonProofProfileCommitment() -> Data {
        Data(SHA256.hash(data:
            Data("sccp:ton:groth16-bls12381:proof-profile:v1".utf8) + Data([1]) +
                Data("ietf-bls12381-compressed-g1-48-g2-96".utf8) +
                Data("groth16-a-g1-b-g2-c-g1".utf8) +
                Data("sha256-sha256-label-value-mod-r".utf8) +
                Data(hexString: "73EDA753299D7D483339D80809A1D80553BDA402FFFE5BFEFFFFFFFF00000001")! +
                bls12381PublicSignalSchemaHash()
        ))
    }

    private func tonAddress(_ byte: UInt8) -> [String: Any] {
        ["workchain": 0, "account": upper(byte, bytes: 32)]
    }

    private func tonDestinationBinding(
        masterCode: Data,
        walletCode: Data,
        routeCode: Data,
        verifierCode: Data,
        circuit: Data,
        keyHash: Data,
        proofProfile: Data,
        semanticHash: Data,
        anchorHash: Data
    ) -> Data {
        var out = Data("iroha:sccp:ton-destination-binding:v1".utf8) + Data([1])
        appendVector(Data("ton-groth16-bls12381-v1".utf8), to: &out)
        appendVector(SccpV1.canonicalNetworkBytes(.tonMainnet), to: &out)
        appendInt32LE(-239, to: &out)
        appendUInt32LE(0, to: &out)
        appendUInt32LE(4, to: &out)
        for value in [
            masterCode, walletCode, routeCode, verifierCode, circuit, keyHash,
            proofProfile, semanticHash, anchorHash,
        ] { out.append(value) }
        return Data(SHA256.hash(data: out))
    }

    private func tonRouteConfiguration(
        masterCode: Data,
        walletCode: Data,
        routeCode: Data,
        verifierCode: Data,
        circuit: Data,
        keyHash: Data,
        proofProfile: Data,
        semanticHash: Data,
        anchorHash: Data,
        binding: Data,
        lane: SccpLaneIdV1,
        revision: UInt32,
        maxWrappedSupply: String
    ) -> Data {
        var deployment = masterCode + walletCode
        for value in [
            routeCode, verifierCode, circuit, keyHash, proofProfile, semanticHash, anchorHash, binding,
        ] { deployment.append(value) }
        let deploymentHash = Data(SHA256.hash(data: deployment))
        var assetRoute = Data()
        appendVector(Data("xor".utf8), to: &assetRoute)
        appendVector(Data("taira_ton_xor".utf8), to: &assetRoute)
        appendUInt32LE(revision, to: &assetRoute)
        appendUInt64LE(1, to: &assetRoute)
        assetRoute.append(SccpUInt128.littleEndianData(maxWrappedSupply)!)
        let assetRouteHash = Data(SHA256.hash(data: assetRoute))
        var out = Data("sccp:concrete-route-config:v1".utf8) + Data([1])
        appendUInt32LE(4, to: &out)
        appendVector(SccpV1.canonicalNetworkBytes(.tonMainnet), to: &out)
        appendInt32LE(-239, to: &out)
        out.append(SccpV1.laneHash(lane))
        out.append(SccpV1.laneHash(try! SccpLaneIdV1(source: lane.target, target: lane.source)))
        out.append(deploymentHash)
        out.append(assetRouteHash)
        return Data(SHA256.hash(data: out))
    }

    private func publicSignalSchemaHash() -> Data {
        let labels = [
            "sccp:groth16-bn254:signal:message-id:v1", "sccp:groth16-bn254:signal:payload-hash:v1",
            "sccp:groth16-bn254:signal:target-domain:v1", "sccp:groth16-bn254:signal:commitment-root:v1",
            "sccp:groth16-bn254:signal:finality-height:v1", "sccp:groth16-bn254:signal:finality-block-hash:v1",
            "sccp:groth16-bn254:signal:source-domain:v1", "sccp:groth16-bn254:signal:statement-hash:v1",
            "sccp:groth16-bn254:signal:destination-binding-hash:v1", "sccp:groth16-bn254:signal:route-configuration-hash:v1",
            "sccp:groth16-bn254:signal:sora-finality-anchor-hash:v1",
        ]
        var bytes = Data([1])
        appendUInt32LE(UInt32(labels.count), to: &bytes)
        for label in labels {
            let value = Data(label.utf8)
            appendUInt32LE(UInt32(value.count), to: &bytes)
            bytes.append(value)
        }
        return irohaKeccak256(Data("sccp:groth16-bn254:public-signal-schema:v1".utf8) + bytes)
    }

    private func policyHashes() -> (semantic: Data, anchor: Data) {
        let circuit = Data(repeating: 0xc1, count: 32)
        let witness = Data(repeating: 0xc2, count: 32)
        let schema = publicSignalSchemaHash()
        let semantic = irohaKeccak256(Data("sccp:semantic-proof-profile:v1".utf8) + Data([1, 0, 1]) + circuit + witness + schema)
        return (semantic, finalityAnchor().hash)
    }

    private func finalityAnchor(protocolVersion: Int = 4) -> (object: [String: Any], hash: Data) {
        let chainId = Data(hexString: "fc56984b2be7431d840e21514d1883f0")!
        let chainHash = irohaKeccak256(chainId)
        let checkpoint = Data(repeating: 0xa1, count: 32)
        let contextId = Data(repeating: 0xa2, count: 32)
        let artifactHash = Data(repeating: 0xa3, count: 32)
        var canonical = Data([1, SccpNetworkV1.soraTaira.tag])
        appendUInt16LE(UInt16(protocolVersion), to: &canonical)
        canonical.append(chainHash)
        appendUInt64LE(7, to: &canonical)
        canonical.append(checkpoint)
        canonical.append(contextId)
        canonical.append(artifactHash)
        XCTAssertEqual(canonical.count, 140)
        let anchorHash = irohaKeccak256(Data("sccp:sora-finality-anchor:v1".utf8) + canonical)
        return ([
            "version": 1,
            "source_network": network("sora-taira"),
            "protocol_version": protocolVersion,
            "chain_id_hash": chainHash.hexEncodedString().uppercased(),
            "checkpoint_height": 7,
            "checkpoint_block_hash": checkpoint.hexEncodedString().uppercased(),
            "checkpoint_context_id": contextId.hexEncodedString().uppercased(),
            "checkpoint_finality_artifact_hash": artifactHash.hexEncodedString().uppercased(),
        ], anchorHash)
    }

    private func destinationBinding(
        destination: [String: Any],
        semanticHash: Data,
        anchorHash: Data,
        source: SccpNetworkV1
    ) -> Data {
        let isTron = source.rawValue.hasPrefix("tron-")
        let networkValue: UInt64
        switch source {
        case .ethereumMainnet: networkValue = 1
        case .ethereumSepolia: networkValue = 11_155_111
        case .bscMainnet: networkValue = 56
        case .bscTestnet: networkValue = 97
        case .tronMainnet: networkValue = 0x2b66_53dc
        case .tronNile: networkValue = 0xcd86_90dc
        case .tronShasta: networkValue = 0x94a9_059e
        default: fatalError("test destination must be external")
        }
        var payload = irohaKeccak256(Data((isTron
            ? "iroha:sccp:tron-destination-binding:v1"
            : "iroha:sccp:evm-destination-binding:v1").utf8))
        payload.append(irohaKeccak256(Data((isTron
            ? "tron-groth16-bn254-v1"
            : "evm-groth16-bn254-v1").utf8)))
        payload.append(abiWord(networkValue))
        payload.append(abiWord(0))
        payload.append(abiWord(UInt64(source.domainId)))
        payload.append(isTron
            ? abiTronAddress(destination["verifier_address"] as! String)
            : abiAddress(destination["verifier_address"] as! String))
        payload.append(isTron
            ? abiTronAddress(destination["route_address"] as! String)
            : abiAddress(destination["route_address"] as! String))
        payload.append(Data(hexString: destination["verifier_code_hash"] as! String)!)
        payload.append(Data(hexString: destination["verifier_key_hash"] as! String)!)
        payload.append(semanticHash)
        payload.append(anchorHash)
        return irohaKeccak256(payload)
    }

    private func routeConfiguration(
        destination: [String: Any], semanticHash: Data, anchorHash: Data, binding: Data, lane: SccpLaneIdV1
    ) -> Data {
        let sourceHash = SccpV1.laneHash(lane)
        let reverseHash = SccpV1.laneHash(try! SccpLaneIdV1(source: lane.target, target: lane.source))
        var deploymentBytes =
            abiAddress(destination["token_address"] as! String) + Data(hexString: destination["token_code_hash"] as! String)! +
            abiAddress(destination["verifier_address"] as! String) + Data(hexString: destination["verifier_code_hash"] as! String)! +
            Data(hexString: destination["verifier_key_hash"] as! String)! + semanticHash + anchorHash
        if lane.source.rawValue.hasPrefix("tron-") { deploymentBytes.append(binding) }
        let deployment = irohaKeccak256(deploymentBytes)
        let routeId = lane.source.rawValue.hasPrefix("ethereum-") ? "taira_eth_xor" :
            lane.source.rawValue.hasPrefix("tron-") ? "taira_tron_xor" : "taira_bsc_xor"
        let networkValue: UInt64
        switch lane.source {
        case .ethereumMainnet: networkValue = 1
        case .ethereumSepolia: networkValue = 11_155_111
        case .bscMainnet: networkValue = 56
        case .bscTestnet: networkValue = 97
        case .tronMainnet: networkValue = 0x2b66_53dc
        case .tronNile: networkValue = 0xcd86_90dc
        case .tronShasta: networkValue = 0x94a9_059e
        default: fatalError("test route must be external")
        }
        let cap = (destination["max_wrapped_supply"] as! NSNumber).stringValue
        let asset = irohaKeccak256(
            irohaKeccak256(Data("xor".utf8)) + irohaKeccak256(Data(routeId.utf8))
                + abiWord(1) + abiWord(1_000_000_000) + SccpUInt128.abiWord(cap)!
        )
        return irohaKeccak256(
            irohaKeccak256(Data("sccp:concrete-route-config:v1".utf8)) +
            abiWord(UInt64(lane.source.domainId)) + abiWord(UInt64(lane.source.tag)) + abiWord(networkValue) +
            sourceHash + reverseHash + deployment + asset
        )
    }

    private func network(_ value: String) -> [String: Any] {
        ["network": value.replacingOccurrences(of: "-", with: "_"), "profile": NSNull()]
    }
    private func lane(_ source: String, _ target: String) -> [String: Any] { ["source": network(source), "target": network(target)] }
    private func upper(_ byte: UInt8, bytes: Int) -> String { String(repeating: String(format: "%02X", byte), count: bytes) }
    private func abiAddress(_ hex: String) -> Data { Data(repeating: 0, count: 12) + Data(hexString: hex)! }
    private func abiTronAddress(_ hex: String) -> Data {
        Data(repeating: 0, count: 11) + Data([0x41]) + Data(hexString: hex)!
    }
    private func abiWord(_ value: UInt64) -> Data {
        var out = Data(repeating: 0, count: 24)
        var big = value.bigEndian
        withUnsafeBytes(of: &big) { out.append(contentsOf: $0) }
        return out
    }
    private func appendUInt16LE(_ value: UInt16, to out: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { out.append(contentsOf: $0) }
    }
    private func appendUInt32LE(_ value: UInt32, to out: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { out.append(contentsOf: $0) }
    }
    private func appendInt32LE(_ value: Int32, to out: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { out.append(contentsOf: $0) }
    }
    private func appendVector(_ value: Data, to out: inout Data) {
        appendUInt32LE(UInt32(value.count), to: &out)
        out.append(value)
    }
    private func appendUInt64LE(_ value: UInt64, to out: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { out.append(contentsOf: $0) }
    }

    private func jsonData(_ value: Any) -> Data { try! JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]) }
    private func jsonObject(_ data: Data, mutableContainers: Bool = false) throws -> [String: Any] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: data, options: mutableContainers ? [.mutableContainers] : []) as? [String: Any])
    }
    private func laneObject(_ root: [String: Any]) -> [String: Any] { (root["lanes"] as! [[String: Any]])[0] }

    private func mutateLane(_ root: inout [String: Any], _ body: (inout [String: Any]) -> Void) {
        var lanes = root["lanes"] as! [[String: Any]]
        body(&lanes[0])
        root["lanes"] = lanes
    }

    private func mutateRoute(_ root: inout [String: Any], _ body: (inout [String: Any]) -> Void) {
        mutateLane(&root) { lane in
            var routes = lane["routes"] as! [[String: Any]]
            body(&routes[0])
            lane["routes"] = routes
        }
    }

    private func mutateDeployment(_ root: inout [String: Any], _ body: (inout [String: Any]) -> Void) {
        mutateRoute(&root) { route in
            var destination = route["destination"] as! [String: Any]
            var deployment = destination["deployment"] as! [String: Any]
            body(&deployment)
            destination["deployment"] = deployment
            route["destination"] = destination
        }
    }

    private func mutateFinalityAnchor(
        _ root: inout [String: Any],
        _ body: (inout [String: Any]) -> Void
    ) {
        mutateDeployment(&root) { deployment in
            var policy = deployment["outbound_proof_policy"] as! [String: Any]
            var anchor = policy["sora_finality_anchor"] as! [String: Any]
            body(&anchor)
            policy["sora_finality_anchor"] = anchor
            deployment["outbound_proof_policy"] = policy
        }
    }

    private func mutateEmitterIdentity(_ root: inout [String: Any], _ body: (inout [String: Any]) -> Void) {
        mutateRoute(&root) { route in
            var source = route["source_identity"] as! [String: Any]
            var emitter = source["emitter"] as! [String: Any]
            var identity = emitter["identity"] as! [String: Any]
            body(&identity)
            emitter["identity"] = identity
            source["emitter"] = emitter
            route["source_identity"] = source
        }
    }

    private func makeClient() -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [SccpStubURLProtocol.self]
        return ToriiClient(baseURL: URL(string: "https://example.test")!, session: URLSession(configuration: configuration))
    }

    private func assertCapabilitiesFetchRejected(
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        do {
            _ = try await makeClient().getSccpCapabilities()
            XCTFail("accepted an invalid bounded SCCP response", file: file, line: line)
        } catch let ToriiClientError.invalidPayload(reason) {
            XCTAssertFalse(reason.isEmpty, file: file, line: line)
        } catch {
            XCTFail("unexpected error: \(error)", file: file, line: line)
        }
    }

    private func responseJSON(submitted: Bool, txHash: String?, transactionPayload: String?, signingMessage: String?) -> Data {
        jsonData([
            "submitted": submitted, "payload_kind": "transfer", "message_id_hex": String(repeating: "1", count: 64),
            "backend": "bridge/sccp/native/bsc-parlia-v1", "counterparty_domain": 2,
            "counterparty_chain": "bsc-mainnet", "route_configuration_hash_hex": String(repeating: "2", count: 64),
            "range_start_height": 4, "range_end_height": 9, "creation_time_ms": 7,
            "tx_hash_hex": txHash as Any? ?? NSNull(),
            "transaction_payload_b64": transactionPayload as Any? ?? NSNull(),
            "signing_message_b64": signingMessage as Any? ?? NSNull(),
        ])
    }

    private func canonicalSccpTransactionPayload(
        authority: String,
        creationTimeMs: UInt64,
        feePayment: FeePaymentIntent = .authority(chargeLimits: [], gasLimit: nil),
        timeToLiveMs: UInt64? = 100_000,
        domainOverride: Data? = nil
    ) throws -> Data {
        try canonicalSccpTransactionPayload(
            authority: authority,
            creationTimeMs: creationTimeMs,
            rawFeePayment: feePayment.compactNorito(),
            timeToLiveMs: timeToLiveMs,
            domainOverride: domainOverride
        )
    }

    private func canonicalSccpTransactionPayload(
        authority: String,
        creationTimeMs: UInt64,
        rawFeePayment: Data,
        timeToLiveMs: UInt64? = 100_000,
        domainOverride: Data? = nil
    ) throws -> Data {
        var networkDomain = CompactNoritoWriter()
        networkDomain.writeUInt32LE(0)
        networkDomain.writeField(TestNetworkIds.canonical.bytes)
        let address = try AccountAddress.parseEncoded(
            authority,
            expectedPrefix: SccpV1.tairaI105DiscriminantV1
        )
        var creation = CompactNoritoWriter()
        creation.writeUInt64LE(creationTimeMs)
        var emptyMetadata = CompactNoritoWriter()
        emptyMetadata.writeUInt64LE(0)

        var payload = CompactNoritoWriter()
        payload.writeField(domainOverride ?? networkDomain.data)
        payload.writeField(try address.compactNoritoAccountControllerPayload())
        payload.writeField(creation.data)
        payload.writeField(Data([1]))
        payload.writeField(try CompactNorito.encodeOption(
            timeToLiveMs,
            encode: CompactNorito.encodeUInt64
        ))
        payload.writeField(Data([0]))
        payload.writeField(rawFeePayment)
        payload.writeField(TransactionAdmissionIntentV1.queuePlanSynced.norito)
        payload.writeField(emptyMetadata.data)
        payload.writeField(Data([0]))
        return payload.data
    }
}
