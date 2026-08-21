import XCTest
import CryptoKit
@testable import IrohaSwift

final class ToriiDaIngestTests: XCTestCase {
    func testBuildsDaRequestWithSigning() throws {
        let payload = Data("hello world".utf8)
        let digest = Data(repeating: 0xAB, count: 32)
        let metadataEntry = ToriiDaMetadataEntry(key: "da.stream", value: Data("demo".utf8))
        let privateKeyBytes = Data((0..<32).map { UInt8($0) })
        let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: privateKeyBytes)
        let owner = try makeDaOwner(privateKey: privateKey)

        var submission = ToriiDaBlobSubmission(
            networkId: TestNetworkIds.canonical,
            owner: owner,
            payload: payload,
            laneId: 1,
            epoch: 2,
            sequence: 3,
            metadata: [metadataEntry],
            clientBlobId: digest,
            privateKey: privateKeyBytes
        )
        submission.codec = "application/octet-stream"

        let builder = ToriiDaIngestRequestBuilder(submission: submission)
        let (body, artifacts) = try builder.makeRequestBody()

        XCTAssertEqual(artifacts.payloadLength, payload.count)
        XCTAssertEqual(artifacts.clientBlobIdHex, digest.upperHexString())

        let expectedSigner = encodeEd25519Multihash(privateKey.publicKey.rawRepresentation)
        XCTAssertEqual(artifacts.signerPublicKeyHex, expectedSigner)
        let signingDigest = try XCTUnwrap(Data(hexString: artifacts.signingDigestHex))
        let signature = try XCTUnwrap(Data(hexString: artifacts.signatureHex))
        XCTAssertTrue(privateKey.publicKey.isValidSignature(signature, for: signingDigest))
        XCTAssertFalse(
            privateKey.publicKey.isValidSignature(signature, for: payload),
            "DA signatures must bind the complete request intent, not only payload bytes"
        )

        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
        XCTAssertEqual(json["network_id"] as? String, TestNetworkIds.canonical.literal)
        XCTAssertEqual(json["owner"] as? String, owner)
        XCTAssertEqual(json["lane_id"] as? Int, 1)
        XCTAssertEqual(json["epoch"] as? Int, 2)
        XCTAssertEqual(json["sequence"] as? Int, 3)
        XCTAssertEqual(json["chunk_size"] as? Int, 262_144)
        XCTAssertEqual(json["total_size"] as? Int, payload.count)

        let codec = try XCTUnwrap(json["codec"] as? [String])
        XCTAssertEqual(codec, ["application/octet-stream"])
        let erasure = try XCTUnwrap(json["erasure_profile"] as? [String: Any])
        XCTAssertEqual(erasure["row_parity_stripes"] as? Int, 0)
        XCTAssertEqual(json["compression"] as? String, "Identity")
        XCTAssertTrue(json["norito_manifest"] is NSNull)

        let digestTuple = try XCTUnwrap(json["client_blob_id"] as? [[NSNumber]])
        XCTAssertEqual(digestTuple.first?.count, 32)
        XCTAssertEqual(digestTuple.first?.first, NSNumber(value: 0xAB))
        let payloadHash = try XCTUnwrap(json["payload_hash"] as? [[NSNumber]])
        XCTAssertEqual(payloadHash.first?.count, 32)
        let signatures = try XCTUnwrap(json["signatures"] as? [[String: String]])
        XCTAssertEqual(signatures.count, 1)
        XCTAssertEqual(signatures[0]["signer"], expectedSigner)
    }

    func testDaIntentDigestMatchesSharedRustProtocolVector() throws {
        let clientBlobId = Data((0..<32).map { UInt8(0x11 + $0) })
        let privateKey = try Curve25519.Signing.PrivateKey(
            rawRepresentation: Data(repeating: 0x19, count: 32)
        )
        let submission = ToriiDaBlobSubmission(
            networkId: try NetworkId(bytes: Data(repeating: 0xA5, count: 32)),
            owner: try makeDaOwner(privateKey: privateKey),
            payload: Data("hello data availability".utf8),
            chunkSize: 1 << 20,
            laneId: 2,
            epoch: 42,
            sequence: 7,
            blobClass: .taikaiSegment,
            codec: "cmaf",
            erasureProfile: ToriiDaErasureProfile(
                dataShards: 8,
                parityShards: 4,
                rowParityStripes: 2,
                chunkAlignment: 12,
                fecScheme: .rs12_10
            ),
            retentionPolicy: ToriiDaRetentionPolicy(
                hotRetentionSecs: 86_400,
                coldRetentionSecs: 30 * 86_400,
                requiredReplicas: 4,
                storageClass: .hot,
                governanceTag: "da.test"
            ),
            compression: .identity,
            metadata: [
                ToriiDaMetadataEntry(
                    key: "content_type",
                    value: Data("video/mp4".utf8)
                )
            ],
            noritoManifest: Data([0xAA, 0xBB, 0xCC]),
            clientBlobId: clientBlobId,
            privateKey: Data(repeating: 0x19, count: 32)
        )

        let artifacts = try ToriiDaIngestRequestBuilder(submission: submission)
            .makeRequestBody()
            .artifacts

        XCTAssertEqual(
            artifacts.signingDigestHex,
            "B97871DB051776138277C9000393FDC259910663A8C751D37BD054A0DA369DDA"
        )
    }

    func testReceiptDecoding() throws {
        let bytes = (0..<32).map { UInt8($0) }
        let digestJSON = "[[\(bytes.map(String.init).joined(separator: ","))]]"
        let payload = """
        {
            "status":"Accepted",
            "duplicate":false,
            "receipt":{
                "client_blob_id":\(digestJSON),
                "lane_id":5,
                "epoch":7,
                "blob_hash":\(digestJSON),
                "chunk_root":\(digestJSON),
                "manifest_hash":\(digestJSON),
                "storage_ticket":\(digestJSON),
                "pdp_commitment":"SGVsbG8=",
                "stripe_layout":{
                    "total_stripes":4,
                    "shards_per_stripe":12,
                    "row_parity_stripes":1
                },
                "queued_at_unix":1700000000,
                "operator_signature":"DEADBEEF",
                "rent_quote":{
                    "base_rent":100,
                    "protocol_reserve":"25",
                    "provider_reward":75,
                    "pdp_bonus":"5",
                    "potr_bonus":3,
                    "egress_credit_per_gib":2
                }
            }
        }
        """.data(using: .utf8)!
        let decoded = try JSONDecoder().decode(ToriiDaIngestSubmitPayload.self, from: payload)
        XCTAssertEqual(decoded.status, "Accepted")
        XCTAssertFalse(decoded.duplicate)
        let receipt = try XCTUnwrap(decoded.receipt)
        XCTAssertEqual(receipt.clientBlobIdHex.count, 64)
        XCTAssertEqual(receipt.storageTicket.count, 32)
        XCTAssertEqual(receipt.operatorSignatureHex, "DEADBEEF")
        XCTAssertEqual(receipt.pdpCommitment, Data("Hello".utf8))
        XCTAssertEqual(receipt.stripeLayout.rowParityStripes, 1)
        let rentQuote = receipt.rentQuote
        XCTAssertEqual(rentQuote.baseRentMicro, "100")
        XCTAssertEqual(rentQuote.protocolReserveMicro, "25")
        XCTAssertEqual(rentQuote.providerRewardMicro, "75")
        XCTAssertEqual(rentQuote.pdpBonusMicro, "5")
        XCTAssertEqual(rentQuote.potrBonusMicro, "3")
        XCTAssertEqual(rentQuote.egressCreditPerGibMicro, "2")
    }

    func testReceiptRequiresCurrentFieldsAndRejectsUnknownFields() throws {
        let canonical = canonicalReceiptObject()
        let decoded = try JSONDecoder().decode(
            ToriiDaIngestReceipt.self,
            from: JSONSerialization.data(withJSONObject: canonical)
        )
        XCTAssertNil(decoded.pdpCommitment, "an explicit null PDP slot remains valid")

        for field in ["pdp_commitment", "stripe_layout", "rent_quote"] {
            var missing = canonical
            missing.removeValue(forKey: field)
            let data = try JSONSerialization.data(withJSONObject: missing)
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiDaIngestReceipt.self, from: data),
                "missing current field \(field) must reject"
            )
        }

        var missingRowParity = canonical
        var stripe = try XCTUnwrap(missingRowParity["stripe_layout"] as? [String: Any])
        stripe.removeValue(forKey: "row_parity_stripes")
        missingRowParity["stripe_layout"] = stripe
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaIngestReceipt.self,
                from: JSONSerialization.data(withJSONObject: missingRowParity)
            )
        )

        var unknownReceipt = canonical
        unknownReceipt["pre_release_extension"] = true
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaIngestReceipt.self,
                from: JSONSerialization.data(withJSONObject: unknownReceipt)
            )
        )

        var unknownStripe = canonical
        stripe = try XCTUnwrap(unknownStripe["stripe_layout"] as? [String: Any])
        stripe["pre_release_extension"] = true
        unknownStripe["stripe_layout"] = stripe
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaIngestReceipt.self,
                from: JSONSerialization.data(withJSONObject: unknownStripe)
            )
        )
    }

    func testRentQuoteRejectsFractionalNumber() {
        let payload = """
        {
            "base_rent":100.5,
            "protocol_reserve":25,
            "provider_reward":75,
            "pdp_bonus":5,
            "potr_bonus":3,
            "egress_credit_per_gib":2
        }
        """.data(using: .utf8)!

        XCTAssertThrowsError(try JSONDecoder().decode(ToriiDaRentQuote.self, from: payload))
    }

    func testRentQuoteRejectsFractionalString() {
        let payload = """
        {
            "base_rent":"100.5",
            "protocol_reserve":"25",
            "provider_reward":"75",
            "pdp_bonus":"5",
            "potr_bonus":"3",
            "egress_credit_per_gib":"2"
        }
        """.data(using: .utf8)!

        XCTAssertThrowsError(try JSONDecoder().decode(ToriiDaRentQuote.self, from: payload))
    }

    func testReceiptRejectsDigestWrongLength() {
        let bytes = (0..<31).map { UInt8($0) }
        let digestJSON = "[[\(bytes.map(String.init).joined(separator: ","))]]"
        let payload = """
        {
            "client_blob_id":\(digestJSON),
            "lane_id":5,
            "epoch":7,
            "blob_hash":\(digestJSON),
            "chunk_root":\(digestJSON),
            "manifest_hash":\(digestJSON),
            "storage_ticket":\(digestJSON),
            "queued_at_unix":1700000000,
            "operator_signature":"DEADBEEF"
        }
        """.data(using: .utf8)!

        XCTAssertThrowsError(try JSONDecoder().decode(ToriiDaIngestReceipt.self, from: payload))
    }

    func testReceiptRejectsDigestWithMultipleTupleEntries() {
        let digestJSON = "[[1],[2]]"
        let payload = """
        {
            "client_blob_id":\(digestJSON),
            "lane_id":5,
            "epoch":7,
            "blob_hash":\(digestJSON),
            "chunk_root":\(digestJSON),
            "manifest_hash":\(digestJSON),
            "storage_ticket":\(digestJSON),
            "queued_at_unix":1700000000,
            "operator_signature":"DEADBEEF"
        }
        """.data(using: .utf8)!

        XCTAssertThrowsError(try JSONDecoder().decode(ToriiDaIngestReceipt.self, from: payload))
    }

    func testMissingClientBlobIdFails() throws {
        let privateKey = try Curve25519.Signing.PrivateKey(
            rawRepresentation: Data(repeating: 0x31, count: 32)
        )
        let submission = ToriiDaBlobSubmission(
            networkId: TestNetworkIds.canonical,
            owner: try makeDaOwner(privateKey: privateKey),
            payload: Data("hi".utf8)
        )
        let builder = ToriiDaIngestRequestBuilder(submission: submission)
        XCTAssertThrowsError(try builder.makeRequestBody())
    }

    private func makeDaOwner(privateKey: Curve25519.Signing.PrivateKey) throws -> String {
        try AccountAddress.fromAccount(publicKey: privateKey.publicKey.rawRepresentation)
            .toI105(networkPrefix: AccountId.defaultNetworkPrefix)
    }

    private func canonicalReceiptObject() -> [String: Any] {
        let digest = Array(0..<32)
        return [
            "client_blob_id": [digest],
            "lane_id": 5,
            "epoch": 7,
            "blob_hash": [digest],
            "chunk_root": [digest],
            "manifest_hash": [digest],
            "storage_ticket": [digest],
            "pdp_commitment": NSNull(),
            "stripe_layout": [
                "total_stripes": 4,
                "shards_per_stripe": 12,
                "row_parity_stripes": 1
            ],
            "queued_at_unix": 1_700_000_000,
            "rent_quote": [
                "base_rent": 100,
                "protocol_reserve": 25,
                "provider_reward": 75,
                "pdp_bonus": 5,
                "potr_bonus": 3,
                "egress_credit_per_gib": 2
            ],
            "operator_signature": "DEADBEEF"
        ]
    }

    private func encodeEd25519Multihash(_ publicKey: Data) -> String {
        var bytes: [UInt8] = []
        bytes.append(contentsOf: encodeVarint(0xED))
        bytes.append(contentsOf: encodeVarint(publicKey.count))
        bytes.append(contentsOf: publicKey)
        return Data(bytes).upperHexString()
    }

    private func encodeVarint(_ value: Int) -> [UInt8] {
        var remaining = value
        var output: [UInt8] = []
        repeat {
            var next = UInt8(remaining & 0x7F)
            remaining >>= 7
            if remaining != 0 {
                next |= 0x80
            }
            output.append(next)
        } while remaining != 0
        return output
    }
}
