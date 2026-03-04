import XCTest
import CryptoKit
@testable import IrohaSwift

final class ToriiDaIngestTests: XCTestCase {
    func testBuildsDaRequestWithSigning() throws {
        let payload = Data("hello world".utf8)
        let digest = Data(repeating: 0xAB, count: 32)
        let metadataEntry = ToriiDaMetadataEntry(key: "da.stream", value: Data("demo".utf8))
        let privateKeyBytes = Data((0..<32).map { UInt8($0) })

        var submission = ToriiDaBlobSubmission(
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

        let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: privateKeyBytes)
        let expectedSubmitter = encodeEd25519Multihash(privateKey.publicKey.rawRepresentation)
        XCTAssertEqual(artifacts.submitterPublicKeyHex, expectedSubmitter)

        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
        XCTAssertEqual(json["lane_id"] as? Int, 1)
        XCTAssertEqual(json["epoch"] as? Int, 2)
        XCTAssertEqual(json["sequence"] as? Int, 3)
        XCTAssertEqual(json["chunk_size"] as? Int, 262_144)
        XCTAssertEqual(json["total_size"] as? Int, payload.count)

        let codec = try XCTUnwrap(json["codec"] as? [String])
        XCTAssertEqual(codec, ["application/octet-stream"])

        let digestTuple = try XCTUnwrap(json["client_blob_id"] as? [[NSNumber]])
        XCTAssertEqual(digestTuple.first?.count, 32)
        XCTAssertEqual(digestTuple.first?.first, NSNumber(value: 0xAB))
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
        let rentQuote = try XCTUnwrap(receipt.rentQuote)
        XCTAssertEqual(rentQuote.baseRentMicro, "100")
        XCTAssertEqual(rentQuote.protocolReserveMicro, "25")
        XCTAssertEqual(rentQuote.providerRewardMicro, "75")
        XCTAssertEqual(rentQuote.pdpBonusMicro, "5")
        XCTAssertEqual(rentQuote.potrBonusMicro, "3")
        XCTAssertEqual(rentQuote.egressCreditPerGibMicro, "2")
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

    func testMissingClientBlobIdFails() {
        let submission = ToriiDaBlobSubmission(payload: Data("hi".utf8))
        let builder = ToriiDaIngestRequestBuilder(submission: submission)
        XCTAssertThrowsError(try builder.makeRequestBody())
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
