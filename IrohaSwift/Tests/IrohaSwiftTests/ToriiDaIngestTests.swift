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
            laneId: .max,
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

        let expectedSigner = CanonicalNorito.publicKeyMultihash(algorithm: .ed25519, payload: privateKey.publicKey.rawRepresentation)
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
        XCTAssertEqual(json["lane_id"] as? NSNumber, NSNumber(value: UInt32.max))
        XCTAssertEqual(json["epoch"] as? Int, 2)
        XCTAssertEqual(json["sequence"] as? Int, 3)
        XCTAssertEqual(json["chunk_size"] as? Int, 262_144)
        XCTAssertEqual(json["total_size"] as? Int, payload.count)

        let codec = try XCTUnwrap(json["codec"] as? [String])
        XCTAssertEqual(codec, ["application/octet-stream"])
        let erasure = try XCTUnwrap(json["erasure_profile"] as? [String: Any])
        XCTAssertEqual(erasure["row_parity_stripes"] as? Int, 0)
        let compression = try XCTUnwrap(json["compression"] as? [String: Any])
        XCTAssertEqual(Set(compression.keys), ["kind", "value"])
        XCTAssertEqual(compression["kind"] as? String, "Identity")
        XCTAssertTrue(compression["value"] is NSNull)
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

    func testDaIntentDigestAndSignatureMatchSharedRustProtocolVectors() throws {
        let clientBlobId = Data((0..<32).map { UInt8(0x11 + $0) })
        let privateKey = try Curve25519.Signing.PrivateKey(
            rawRepresentation: Data(repeating: 0x19, count: 32)
        )
        var submission = ToriiDaBlobSubmission(
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

        struct SigningVector: Decodable {
            let compression: String
            let wireBase64: String
            let authorizationDigestHex: String
            let signer: String
            let signatureHex: String
        }
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("fixtures/da/ingest/request_signing_v1.json"))
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        for vector in try decoder.decode([SigningVector].self, from: data) {
            switch vector.compression {
            case "Identity":
                submission.compression = .identity
                submission.compressedPayload = nil
            case "Deflate":
                submission.compression = .deflate
                submission.compressedPayload = try XCTUnwrap(Data(base64Encoded: vector.wireBase64))
            default:
                XCTFail("Unexpected fixture compression \(vector.compression)")
                continue
            }
            let (body, artifacts) = try ToriiDaIngestRequestBuilder(submission: submission).makeRequestBody()
            let fields = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
            XCTAssertEqual(fields["payload"] as? String, vector.wireBase64)
            XCTAssertEqual(artifacts.signingDigestHex, vector.authorizationDigestHex)
            XCTAssertEqual(artifacts.signerPublicKeyHex, vector.signer)
            let witnesses = try XCTUnwrap(fields["signatures"] as? [[String: String]])
            XCTAssertEqual(witnesses, [["signer": vector.signer, "signature": vector.signatureHex]])
            XCTAssertEqual(artifacts.signatureHex, vector.signatureHex)
            var external = submission
            external.privateKey = nil
            external.signerPublicKeyHex = vector.signer
            external.signatureHex = vector.signatureHex
            let signed = try ToriiDaIngestRequestBuilder(submission: external).makeRequestBody()
            XCTAssertEqual(signed.artifacts.signerPublicKeyHex, vector.signer)
            XCTAssertEqual(signed.artifacts.signatureHex, vector.signatureHex)
        }
        submission.signerPublicKeyHex = "ed0120" + String(repeating: "41", count: 32)
        XCTAssertThrowsError(try ToriiDaIngestRequestBuilder(submission: submission).makeRequestBody())
    }

    func testReceiptRequiresBoundedCanonicalSignatureAndAcceptsMlDsa65Length() throws {
        var object = canonicalReceiptObject()
        let signature = String(repeating: "AB", count: 3309)
        object["operator_signature"] = signature
        let receipt = try JSONDecoder().decode(ToriiDaIngestReceipt.self, from: JSONSerialization.data(withJSONObject: object))
        XCTAssertEqual(receipt.operatorSignatureHex, signature)
        for malformed in ["", "00", "AA0", "ab", String(repeating: "AB", count: 4097)] {
            object["operator_signature"] = malformed
            XCTAssertThrowsError(try JSONDecoder().decode(ToriiDaIngestReceipt.self, from: JSONSerialization.data(withJSONObject: object)))
        }
    }

    func testReceiptDecoding() throws {
        let bytes = (0..<32).map { UInt8($0) }
        let digestJSON = "[[\(bytes.map(String.init).joined(separator: ","))]]"
        let payload = """
        {
            "status":"accepted",
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
                "retention_policy":{"hot_retention_secs":3600,"cold_retention_secs":172800,"required_replicas":3,"storage_class":{"type":"Hot","value":null},"governance_tag":["da.sidecar"]},
                "stripe_layout":{
                    "total_stripes":4,
                    "shards_per_stripe":12,
                    "row_parity_stripes":1
                },
                "queued_at_unix":1700000000,
                "operator_signature":"DEADBEEF",
                "rent_quote":{
                    "base_rent":"100",
                    "protocol_reserve":"25",
                    "provider_reward":"75",
                    "pdp_bonus":"5",
                    "potr_bonus":"3",
                    "egress_credit_per_gib":"2"
                }
            }
        }
        """.data(using: .utf8)!
        let decoded = try JSONDecoder().decode(ToriiDaIngestSubmitPayload.self, from: payload)
        XCTAssertEqual(decoded.status, "accepted")
        XCTAssertFalse(decoded.duplicate)
        let receipt = try XCTUnwrap(decoded.receipt)
        XCTAssertEqual(receipt.clientBlobIdHex.count, 64)
        XCTAssertEqual(receipt.storageTicket.count, 32)
        XCTAssertEqual(receipt.operatorSignatureHex, "DEADBEEF")
        XCTAssertEqual(receipt.pdpCommitment, Data("Hello".utf8))
        XCTAssertEqual(receipt.stripeLayout.rowParityStripes, 1)
        XCTAssertEqual(receipt.retentionPolicy.requiredReplicas, 3)
        XCTAssertEqual(receipt.retentionPolicy.governanceTag, "da.sidecar")
        let rentQuote = receipt.rentQuote
        XCTAssertEqual(rentQuote.baseRent, "100")
        XCTAssertEqual(rentQuote.protocolReserve, "25")
        XCTAssertEqual(rentQuote.providerReward, "75")
        XCTAssertEqual(rentQuote.pdpBonus, "5")
        XCTAssertEqual(rentQuote.potrBonus, "3")
        XCTAssertEqual(rentQuote.egressCreditPerGib, "2")
    }

    func testReceiptLaneFitsTheUnsigned32BitWireField() throws {
        var receipt = canonicalReceiptObject()
        func decodeLane(_ number: NSNumber) throws -> UInt32 {
            receipt["lane_id"] = number
            let data = try JSONSerialization.data(withJSONObject: receipt)
            return try JSONDecoder().decode(ToriiDaIngestReceipt.self, from: data).laneId
        }
        XCTAssertEqual(try decodeLane(NSNumber(value: UInt32.max)), UInt32.max)
        XCTAssertThrowsError(try decodeLane(NSNumber(value: UInt64(UInt32.max) + 1)))
        XCTAssertThrowsError(try decodeLane(NSNumber(value: -1)))
    }

    func testReceiptPdpCommitmentRequiresCanonicalStandardBase64() throws {
        var receipt = canonicalReceiptObject()
        func decodeCommitment(_ value: Any) throws -> Data? {
            receipt["pdp_commitment"] = value
            let data = try JSONSerialization.data(withJSONObject: receipt)
            return try JSONDecoder().decode(ToriiDaIngestReceipt.self, from: data).pdpCommitment
        }
        XCTAssertNil(try decodeCommitment(NSNull()))
        XCTAssertEqual(try XCTUnwrap(decodeCommitment("")), Data())
        XCTAssertEqual(try decodeCommitment("/w=="), Data([0xFF]))
        XCTAssertEqual(try decodeCommitment("/wA="), Data([0xFF, 0x00]))
        for invalid in ["/x==", "/wB=", "/w", "_w==", "/w== ", "/w==\n"] {
            XCTAssertThrowsError(try decodeCommitment(invalid), "accepted \(invalid.debugDescription)")
        }
    }

    func testReceiptRequiresCurrentFieldsAndRejectsUnknownFields() throws {
        let canonical = canonicalReceiptObject()
        let decoded = try JSONDecoder().decode(
            ToriiDaIngestReceipt.self,
            from: JSONSerialization.data(withJSONObject: canonical)
        )
        XCTAssertNil(decoded.pdpCommitment, "an explicit null PDP slot remains valid")

        for field in ["pdp_commitment", "stripe_layout", "rent_quote", "retention_policy"] {
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

    func testReceiptRequiresEffectiveReplicationPolicy() throws {
        let baseline = canonicalReceiptObject()
        let valid = try XCTUnwrap(baseline["retention_policy"] as? [String: Any])
        for (replicas, tag) in [(0, "da.sidecar"), (3, ""), (3, " da.sidecar"), (3, "da\nsidecar")] {
            var receipt = baseline
            var policy = valid
            policy["required_replicas"] = replicas
            policy["governance_tag"] = [tag]
            receipt["retention_policy"] = policy
            let data = try JSONSerialization.data(withJSONObject: receipt)
            XCTAssertThrowsError(try JSONDecoder().decode(ToriiDaIngestReceipt.self, from: data))
        }
    }

    func testReceiptDecodesRustManifestRentQuoteInExactXor() throws {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let url = root.appendingPathComponent("fixtures/da/ingest/manifests/nexus_lane_sidecar/manifest.json")
        let manifest = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
        var receipt = canonicalReceiptObject()
        receipt["rent_quote"] = try XCTUnwrap(manifest["rent_quote"] as? [String: String])
        let data = try JSONSerialization.data(withJSONObject: receipt)
        let quote = try JSONDecoder().decode(ToriiDaIngestReceipt.self, from: data).rentQuote
        XCTAssertEqual(quote.baseRent, "0.25")
        XCTAssertEqual(quote.protocolReserve, "0.05")
        XCTAssertEqual(quote.providerReward, "0.2")
        XCTAssertEqual(quote.pdpBonus, "0.0125")
        XCTAssertEqual(quote.potrBonus, "0.00625")
        XCTAssertEqual(quote.egressCreditPerGib, "0.0015")
    }

    func testRentQuoteEnforcesCanonicalXorBoundsAndRequiredFields() throws {
        let fields = try XCTUnwrap(canonicalReceiptObject()["rent_quote"] as? [String: String])
        func decode(_ object: [String: Any]) throws -> ToriiDaRentQuote {
            try JSONDecoder().decode(ToriiDaRentQuote.self, from: JSONSerialization.data(withJSONObject: object))
        }
        let maxMantissa =
            "67039039649712985497870124991029230637396829102961966888617807218608820150367" +
            "73488400937149083451713845015929093243025426876941405973284973216824503042047"
        for amount in ["0", "0.000000001", maxMantissa] {
            var quote = fields
            quote["base_rent"] = amount
            XCTAssertEqual(try decode(quote).baseRent, amount)
        }
        let invalid: [Any] = [0, 12.5, NSNull(), "", " 1", "+1", "-1", "01", "1.", "1.0", "1e2",
                              "0.0000000001", String(maxMantissa.dropLast()) + "8"]
        for amount in invalid {
            var quote: [String: Any] = fields
            quote["base_rent"] = amount
            XCTAssertThrowsError(try decode(quote), "accepted invalid XOR amount \(amount)")
        }
        for field in fields.keys {
            var quote = fields
            quote.removeValue(forKey: field)
            XCTAssertThrowsError(try decode(quote), "accepted missing \(field)")
        }
        var retired = fields
        retired["base_rent_micro"] = "1"
        XCTAssertThrowsError(try decode(retired))
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

    func testCompressedRequestBindsCanonicalPayloadAndExactWireBytes() throws {
        let key = try Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 0x11, count: 32))
        let payload = Data("hello".utf8)
        // RFC 1950 zlib stream containing “hello”.
        let wire = try XCTUnwrap(Data(hexString: "789CCB48CDC9C90700062C0215"))
        var submission = ToriiDaBlobSubmission(networkId: TestNetworkIds.canonical,
            owner: try makeDaOwner(privateKey: key), payload: payload,
            compression: .deflate, compressedPayload: wire, privateKey: key.rawRepresentation)
        let prepared = try ToriiDaIngestRequestBuilder(submission: submission).makeRequestBody()
        let body = try XCTUnwrap(JSONSerialization.jsonObject(with: prepared.body) as? [String: Any])
        XCTAssertEqual(body["payload"] as? String, wire.base64EncodedString())
        XCTAssertEqual(body["total_size"] as? Int, payload.count)
        let expected = try XCTUnwrap(NoritoNativeBridge.shared.blake3Hash(data: payload))
        XCTAssertEqual(prepared.artifacts.payloadHashHex, expected.upperHexString())
        submission.compressedPayload = nil
        XCTAssertThrowsError(try ToriiDaIngestRequestBuilder(submission: submission).makeRequestBody())
    }

    func testCompleteCapturedRequestPreservesUnescapedBase64Bytes() throws {
        let key = try Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 0x42, count: 32))
        let bytes = Data(repeating: 0xff, count: 12 * 1024)
        let submission = ToriiDaBlobSubmission(networkId: TestNetworkIds.canonical,
            owner: try makeDaOwner(privateKey: key), payload: bytes, privateKey: key.rawRepresentation)
        let body = try ToriiDaIngestRequestBuilder(submission: submission).makeRequestBody().body
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        try ToriiClient.persistDaRequestArtifacts(body: body, directory: directory, fileManager: .default)
        let stored = try Data(contentsOf: directory.appendingPathComponent("da_request.json"))
        XCTAssertEqual(stored, body)
        XCTAssertNotNil(stored.range(of: Data(bytes.base64EncodedString().utf8)))
        XCTAssertNil(stored.range(of: Data(#"\/"#.utf8)))
    }

    func testSubmitRejectsAnIdenticalControllerWithAForeignI105PrefixBeforePosting() async throws {
        let key = try Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 0x47, count: 32))
        let address = try AccountAddress.fromAccount(publicKey: key.publicKey.rawRepresentation)
        let submission = ToriiDaBlobSubmission(networkId: TestNetworkIds.canonical,
            owner: try address.toI105(chainDiscriminant: 753), payload: Data("test".utf8), privateKey: key.rawRepresentation)
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        StubURLProtocol.handler = { _ in XCTFail("mismatched DA owner caused a POST"); throw URLError(.badURL) }
        defer { StubURLProtocol.handler = nil }
        let client = ToriiClient(baseURL: URL(string: "https://example.test")!,
            session: URLSession(configuration: configuration),
            localSigningContext: ToriiLocalSigningContext(networkId: submission.networkId),
            canonicalRequestAuth: ToriiCanonicalRequestAuth(accountId: try address.toI105(chainDiscriminant: 369),
                                                           privateKey: key.rawRepresentation))
        do {
            _ = try await client.submitDaBlob(submission)
            XCTFail("expected a DA owner discriminant rejection")
        } catch ToriiClientError.invalidPayload(let reason) {
            XCTAssertTrue(reason.contains("exact network and I105 owner"))
        }
    }

    func testMultipartResumesAtAcknowledgedPartAndPreservesSignedMetadata() async throws {
        let key = try Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 0x12, count: 32))
        let bytes = Data(repeating: 0xff, count: ToriiDaUpload.partBytes + 3)
        let submission = ToriiDaBlobSubmission(networkId: TestNetworkIds.canonical,
                                               owner: try makeDaOwner(privateKey: key), payload: bytes,
                                               privateKey: key.rawRepresentation)
        let metadata = try ToriiDaIngestRequestBuilder(submission: submission).makeRequestBody(includePayload: false).body
        var calls: [String] = []
        let (_, result) = try await ToriiDaUpload.transfer(submission, metadata: metadata) { path, data, status in
            XCTAssertEqual(path, "/v1/da/ingest/parts")
            let body = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
            let action = try XCTUnwrap(body["action"] as? [String: Any])
            let kind = try XCTUnwrap(action["kind"] as? String)
            calls.append(kind)
            if kind == "Finish" {
                XCTAssertEqual(status, 202)
                let request = try XCTUnwrap(action["value"] as? [String: Any])
                XCTAssertEqual(request["payload"] as? String, "")
                XCTAssertNotNil(request["signatures"])
                return (Data(), "submitted")
            }
            XCTAssertEqual(status, 200)
            if kind == "Put" {
                XCTAssertNil(data.range(of: Data(#"\/"#.utf8)))
                let part = try XCTUnwrap(action["value"] as? [String: Any])
                XCTAssertEqual(part["index"] as? Int, 1)
                XCTAssertEqual(part["bytes_b64"] as? String, Data(bytes.suffix(3)).base64EncodedString())
            }
            let ack = try JSONSerialization.data(withJSONObject: ["next_part": kind == "Begin" ? 1 : 2])
            return (ack, "progress")
        }
        XCTAssertEqual(calls, ["Begin", "Put", "Finish"])
        XCTAssertEqual(result, "submitted")
    }

    func testPinScopeDecoderRequiresTheCompleteDigestTuple() throws {
        let bytes = [UInt8](repeating: 0x35, count: 32)
        let owner = try makeDaOwner(privateKey: Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 0x19, count: 32)))
        var scope: [String: Any] = ["network_id": TestNetworkIds.canonical.literal, "owner": owner,
            "lane_id": 1, "epoch": 2, "sequence": 3, "alias": NSNull(), "storage_ticket": [bytes],
            "manifest_hash": [bytes], "request_authorization_digest": [bytes], "commitment_digest": [bytes]]
        let result = try JSONDecoder().decode(ToriiDaPinScope.self, from: JSONSerialization.data(withJSONObject: scope))
        XCTAssertEqual(result.commitmentDigest, Data(bytes))
        scope["commitment_digest"] = bytes
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiDaPinScope.self, from: JSONSerialization.data(withJSONObject: scope)))
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
            "retention_policy": [
                "hot_retention_secs": 3600, "cold_retention_secs": 172800, "required_replicas": 3,
                "storage_class": ["type": "Hot", "value": NSNull()] as [String: Any],
                "governance_tag": ["da.sidecar"]
            ] as [String: Any],
            "stripe_layout": [
                "total_stripes": 4,
                "shards_per_stripe": 12,
                "row_parity_stripes": 1
            ],
            "queued_at_unix": 1_700_000_000,
            "rent_quote": [
                "base_rent": "100",
                "protocol_reserve": "25",
                "provider_reward": "75",
                "pdp_bonus": "5",
                "potr_bonus": "3",
                "egress_credit_per_gib": "2"
            ],
            "operator_signature": "DEADBEEF"
        ]
    }
}
