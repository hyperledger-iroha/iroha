import CryptoKit
import XCTest
@testable import IrohaSwift

final class SccpV1Tests: XCTestCase {
    func testNativeTransferEventSharedVectors() throws {
        let vectors: [(SccpNetworkV1, String, String, String, String)] = [
            (
                .bscMainnet,
                "020102000000000000000700000000000000000000000103000000786f724d00000000000000000000000000000002140000001111111111111111111111111111111111111111010b000000616c696365407461697261010d00000074616972615f6273635f786f72",
                "d9e1e2c69af5795970f172947321e63257b7be98214c41b24da4fc90bdadf219",
                "5e747cfd5119981d7c75e673e3d055a51c065469b33f881a60b26dc125332dbb",
                "5050d6d02c555627119face798d51c868c535a8c26b6a69c50d2b8ffc7dde008"
            ),
            (
                .tronMainnet,
                "020105000000000000000700000000000000000000000103000000786f724d0000000000000000000000000000000515000000412222222222222222222222222222222222222222010b000000616c696365407461697261010e00000074616972615f74726f6e5f786f72",
                "7f7e0f0165de518d135f63118c05f250e6467ea812fa5f9911bedab67a834cc9",
                "568ea09b5c63e54850dc3cd9d033a4d5e9f2c89c3e7d03d099657e246e83b55a",
                "32cd52d171e0c917e159d3b0fa9264534dac0eb45beb4ec7076c780dec850540"
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

    func testSourceDigestIsLaneBoundAndRoleSeparated() throws {
        let payload = Data([1, 2, 3])
        let mainnet = try SccpLaneIdV1(source: .bscMainnet, target: .soraTaira)
        let testnet = try SccpLaneIdV1(source: .bscTestnet, target: .soraTaira)
        let payloadHash = try SccpV1.payloadHash(payload)
        let mainnetMessage = try SccpV1.messageId(lane: mainnet, canonicalPayload: payload)
        let testnetMessage = try SccpV1.messageId(lane: testnet, canonicalPayload: payload)
        XCTAssertNotEqual(mainnetMessage, testnetMessage)
        XCTAssertNotEqual(
            try SccpV1.sourceEventDigest(lane: mainnet, messageId: mainnetMessage, payloadHash: payloadHash),
            try SccpV1.sourceEventDigest(lane: testnet, messageId: testnetMessage, payloadHash: payloadHash)
        )
        XCTAssertThrowsError(try SccpV1.sourceEventDigest(
            lane: mainnet,
            messageId: SccpV1.laneHash(mainnet),
            payloadHash: payloadHash
        ))
    }

    func testCanonicalCodecBoundariesAndAdversarialValues() throws {
        XCTAssertEqual(try SccpCodecV1.canonicalText.validate(Data("merchant@taira".utf8)), Data("merchant@taira".utf8))
        XCTAssertThrowsError(try SccpCodecV1.canonicalText.validate(Data()))
        XCTAssertThrowsError(try SccpCodecV1.canonicalText.validate(Data("contains space".utf8)))
        XCTAssertThrowsError(try SccpCodecV1.canonicalText.validate(Data(repeating: 0x21, count: 257)))
        XCTAssertThrowsError(try SccpCodecV1.evmAddress20.validate(Data(repeating: 0, count: 20)))
        XCTAssertNoThrow(try SccpCodecV1.evmAddress20.validate(Data(repeating: 1, count: 20)))
        XCTAssertThrowsError(try SccpCodecV1.solanaPubkey32.validate(Data(repeating: 1, count: 31)))

        var ton = Data(repeating: 0, count: 36)
        ton[0] = 0xff
        ton[1] = 0xff
        ton[2] = 0xff
        ton[3] = 0xff
        ton[4] = 1
        XCTAssertNoThrow(try SccpCodecV1.tonAccount36.validate(ton))
        ton[0] = 1
        ton[1] = 0
        ton[2] = 0
        ton[3] = 0
        XCTAssertThrowsError(try SccpCodecV1.tonAccount36.validate(ton))

        var tron = Data(repeating: 1, count: 21)
        tron[0] = 0x41
        XCTAssertNoThrow(try SccpCodecV1.tronAddress21.validate(tron))
        tron[0] = 0x42
        XCTAssertThrowsError(try SccpCodecV1.tronAddress21.validate(tron))
        XCTAssertThrowsError(try SccpCodecV1.soraAssetId.validate(Data(repeating: 0, count: 32)))
    }

    func testSourceEmitterRejectsCollidingRolesAndOwnerHasNoSurface() throws {
        XCTAssertThrowsError(try SccpSourceEmitterV1.validatedEvm(
            address: Data(repeating: 1, count: 20),
            runtimeCodeHash: Data(repeating: 2, count: 32),
            routeConfigHash: Data(repeating: 2, count: 32)
        ))
        XCTAssertNoThrow(try SccpSourceEmitterV1.validatedTron(
            address: Data(repeating: 1, count: 20),
            runtimeCodeHash: Data(repeating: 2, count: 32),
            routeConfigHash: Data(repeating: 3, count: 32)
        ))
    }

    func testSubmitRequestsRequireCanonicalNoritoAndBoundSignerPair() throws {
        let privateKey = Curve25519.Signing.PrivateKey()
        let authority = try AccountAddress.fromAccount(publicKey: privateKey.publicKey.rawRepresentation)
            .toI105(networkPrefix: 0x02f1)
        let bundle = noritoEncode(typeName: "iroha_sccp::NexusSccpMessageProofV1", payload: Data([1]))
            .base64EncodedString()
        let signature = try privateKey.signature(for: Data(repeating: 7, count: 32)).base64EncodedString()
        let request = try ToriiBridgeProofSubmitRequest(
            authority: authority,
            messageBundleB64: bundle,
            publicKeyHex: privateKey.publicKey.rawRepresentation.hexEncodedString(),
            signatureB64: signature
        )
        let json = try JSONSerialization.jsonObject(with: JSONEncoder().encode(request)) as! [String: Any]
        XCTAssertNotNil(json["message_bundle_b64"])
        XCTAssertNil(json["private_key"])
        XCTAssertNil(json["burn_bundle"])
        XCTAssertNil(json["expected_destination_binding_hash_hex"])
        XCTAssertNil(json["native_proof_submit_path"])

        XCTAssertThrowsError(try ToriiBridgeProofSubmitRequest(
            authority: authority,
            messageBundleB64: "AQ=="
        ))
        XCTAssertThrowsError(try ToriiBridgeMessageSubmitRequest(
            authority: authority,
            nativeProofB64: bundle,
            publicKeyHex: privateKey.publicKey.rawRepresentation.hexEncodedString()
        ))
        XCTAssertThrowsError(try ToriiBridgeMessageSubmitRequest(
            authority: authority,
            nativeProofB64: bundle,
            publicKeyHex: String(repeating: "0", count: 64),
            signatureB64: signature
        ))
        XCTAssertThrowsError(try ToriiBridgeMessageSubmitRequest(
            authority: authority,
            nativeProofB64: bundle,
            creationTimeMs: 0
        ))
    }

    func testUnifiedBridgeResponseStrictPositiveStates() throws {
        let submitted = responseJSON(
            submitted: true,
            txHash: String(repeating: "3", count: 64),
            transactionPayload: nil,
            signingMessage: nil
        )
        let parsed = try SccpBridgeSubmitResponse.parse(submitted)
        XCTAssertTrue(parsed.submitted)
        XCTAssertEqual(parsed.payloadKind, .transfer)

        let payload = Data([1, 2, 3, 4])
        var signing = Blake2b.hash256(payload)
        signing[31] |= 1
        let prepared = responseJSON(
            submitted: false,
            txHash: nil,
            transactionPayload: payload.base64EncodedString(),
            signingMessage: signing.base64EncodedString()
        )
        let expectation = try SccpBridgeResponseExpectation(
            payloadKind: .transfer,
            messageIdHex: String(repeating: "1", count: 64),
            counterpartyDomain: 2,
            counterpartyChain: .bscMainnet,
            creationTimeMs: 7
        )
        XCTAssertFalse(try SccpBridgeSubmitResponse.parse(prepared, expectation: expectation).submitted)
    }

    func testUnifiedBridgeResponseRejectsLegacyUnknownDuplicateAndMalformedStates() throws {
        let valid = String(data: responseJSON(
            submitted: true,
            txHash: String(repeating: "3", count: 64),
            transactionPayload: nil,
            signingMessage: nil
        ), encoding: .utf8)!
        for legacy in ["ok", "proof_kind", "message_kind", "transaction_scaffold_b64", "signed_transaction_b64"] {
            let mutated = valid.dropLast() + ",\"\(legacy)\":null}"
            XCTAssertThrowsError(try SccpBridgeSubmitResponse.parse(Data(mutated.utf8)), "must reject \(legacy)")
        }
        XCTAssertThrowsError(try SccpBridgeSubmitResponse.parse(Data(valid.replacingOccurrences(
            of: "\"submitted\":true",
            with: "\"submitted\":true,\"submitted\":false"
        ).utf8)))
        XCTAssertThrowsError(try SccpBridgeSubmitResponse.parse(Data(valid.replacingOccurrences(
            of: "\"payload_kind\":\"transfer\"",
            with: "\"payload_kind\":\"burn\""
        ).utf8)))
        XCTAssertThrowsError(try SccpBridgeSubmitResponse.parse(Data(valid.replacingOccurrences(
            of: "\"counterparty_chain\":\"bsc-mainnet\"",
            with: "\"counterparty_chain\":\"ethereum-mainnet\""
        ).utf8)))
        XCTAssertThrowsError(try SccpBridgeSubmitResponse.parse(Data(valid.replacingOccurrences(
            of: "\"range_end_height\":9",
            with: "\"range_end_height\":1"
        ).utf8)))
        XCTAssertThrowsError(try SccpBridgeSubmitResponse.parse(Data(valid.replacingOccurrences(
            of: "\"tx_hash_hex\":\"\(String(repeating: "3", count: 64))\"",
            with: "\"tx_hash_hex\":null"
        ).utf8)))
    }

    func testCapabilitiesAreExactAndRetiredCodecKeysFail() throws {
        let capabilities = Data("""
        {"version":1,"registry_revision":"0x\(String(repeating: "1", count: 64))","native_message_submit_path":"/v1/bridge/messages","outbound":{"message_bundle_path":"/v1/sccp/proofs/message/{message_id}","proof_artifact_path":"/v1/sccp/artifacts/message/{message_id}","proof_job_path":"/v1/sccp/jobs/message/{message_id}","recent_messages_path":"/v1/sccp/messages/recent","manifest_path":"/v1/sccp/manifests"},"message_payload_kinds":["asset_register","route_activate","transfer","token_add","token_pause","token_resume"],"codecs":[{"id":1,"key":"canonical_text","description":"Canonical printable text."},{"id":2,"key":"evm_address20","description":"Raw EVM address."},{"id":3,"key":"solana_pubkey32","description":"Raw Solana key."},{"id":4,"key":"ton_account36","description":"Raw TON account."},{"id":5,"key":"tron_address21","description":"Raw TRON address."},{"id":6,"key":"sora_asset_id","description":"Raw SORA asset id."}],"inbound_lanes":[]}
        """.utf8)
        XCTAssertEqual(try SccpCapabilities.parse(capabilities).codecs.map(\.codec), SccpCodecV1.allCases)
        XCTAssertThrowsError(try SccpCapabilities.parse(Data(String(data: capabilities, encoding: .utf8)!
            .replacingOccurrences(of: "evm_address20", with: "evm_hex").utf8)))
        XCTAssertThrowsError(try SccpCapabilities.parse(Data(String(data: capabilities, encoding: .utf8)!
            .replacingOccurrences(of: "\"inbound_lanes\":[]", with: "\"inbound_lanes\":[],\"allow_unready\":true").utf8)))
    }

    private func responseJSON(
        submitted: Bool,
        txHash: String?,
        transactionPayload: String?,
        signingMessage: String?
    ) -> Data {
        let tx = txHash.map { "\"\($0)\"" } ?? "null"
        let payload = transactionPayload.map { "\"\($0)\"" } ?? "null"
        let signing = signingMessage.map { "\"\($0)\"" } ?? "null"
        return Data("""
        {"submitted":\(submitted),"payload_kind":"transfer","message_id_hex":"\(String(repeating: "1", count: 64))","backend":"bridge/sccp/native/bsc-parlia-v1","counterparty_domain":2,"counterparty_chain":"bsc-mainnet","manifest_hash_hex":"\(String(repeating: "2", count: 64))","range_start_height":4,"range_end_height":9,"creation_time_ms":7,"tx_hash_hex":\(tx),"transaction_payload_b64":\(payload),"signing_message_b64":\(signing)}
        """.utf8)
    }
}
