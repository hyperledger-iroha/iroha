import Foundation
import CryptoKit
import XCTest
@testable import IrohaSwift

final class ToriiDaClientTests: XCTestCase {
    override func tearDown() {
        ToriiDaStubURLProtocol.handler = nil
        super.tearDown()
    }

    func testFetchDaPayloadInjectsChunkerHandle() async throws {
        let bundle = try decodeManifestBundle()
        let provider = try sampleProvider()
        let fetcher = MockGatewayFetcher()
        let expectedHandle = "sorafs.chunker@1.0.0"
        let options = SorafsGatewayFetchOptions(maxPeers: 2)
        let result = try await ToriiClient(baseURL: URL(string: "https://example.com")!)
            .fetchDaPayloadViaGateway(
                manifestBundle: bundle,
                chunkerHandle: expectedHandle,
                providers: [provider],
                options: options,
                orchestrator: fetcher
            )
        XCTAssertEqual(fetcher.lastOptions?.chunkerHandle, expectedHandle)
        XCTAssertEqual(result.chunkerHandle, expectedHandle)
    }

    func testFetchDaPayloadReturnsProofSummaryWhenGeneratorProvided() async throws {
        let bundle = try decodeManifestBundle()
        let provider = try sampleProvider()
        let fetcher = MockGatewayFetcher()
        let expectedSummary = ToriiDaProofSummary(
            blobHashHex: "00",
            chunkRootHex: "11",
            porRootHex: "22",
            leafCount: 0,
            segmentCount: 0,
            chunkCount: 0,
            sampleCount: 1,
            sampleSeed: 0,
            proofCount: 0,
            proofs: []
        )
        let generator = MockDaProofSummaryGenerator(value: expectedSummary)
        let summaryOptions = ToriiDaProofSummaryOptions(sampleCount: 1, sampleSeed: 0, leafIndexes: [])
        let result = try await ToriiClient(baseURL: URL(string: "https://example.com")!)
            .fetchDaPayloadViaGateway(
                manifestBundle: bundle,
                chunkerHandle: "sorafs.chunker@1.0.0",
                providers: [provider],
                proofSummaryOptions: summaryOptions,
                orchestrator: fetcher,
                proofSummaryGenerator: generator
            )
        XCTAssertEqual(result.proofSummary, expectedSummary)
    }

    func testSubmitDaBlobPersistsRequestWhenNoSubmit() async throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAvailable,
            "NoritoBridge is required to derive DA payload digest"
        )
        let privateKeyBytes = Data(repeating: 0x32, count: 32)
        let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: privateKeyBytes)
        let owner = try AccountAddress.fromAccount(
            publicKey: privateKey.publicKey.rawRepresentation
        ).toI105(networkPrefix: AccountId.defaultNetworkPrefix)
        let submission = ToriiDaBlobSubmission(
            networkId: TestNetworkIds.canonical,
            owner: owner,
            payload: Data([0x01, 0x02]),
            laneId: 7,
            epoch: 1,
            sequence: 0,
            blobClass: .taikaiSegment,
            codec: "custom.binary",
            clientBlobId: Data(repeating: 0x22, count: 32),
            privateKey: privateKeyBytes
        )
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let result = try await ToriiClient(baseURL: URL(string: "https://example.com")!)
            .submitDaBlob(submission,
                          artifactDirectory: tempDir,
                          noSubmit: true)
        XCTAssertEqual(result.status, "prepared")
        let requestURL = tempDir.appendingPathComponent("da_request.json")
        XCTAssertTrue(FileManager.default.fileExists(atPath: requestURL.path))
        let data = try Data(contentsOf: requestURL)
        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        XCTAssertEqual(json?["network_id"] as? String, TestNetworkIds.canonical.literal)
        XCTAssertEqual(json?["owner"] as? String, owner)
        XCTAssertEqual(json?["lane_id"] as? Int, 7)
    }

    func testProveDaAvailabilityToDirectoryPersistsArtefacts() async throws {
        let bundle = try decodeManifestBundle()
        let provider = try sampleProvider()
        let fetcher = MockGatewayFetcher()
        fetcher.scoreboard = [
            SorafsGatewayFetchReport.ScoreboardEntry(
                providerID: "aa",
                alias: "alpha",
                rawScore: 1.0,
                normalizedWeight: 1.0,
                eligibility: "eligible"
            ),
        ]
        let summary = ToriiDaProofSummary(
            blobHashHex: bundle.blobHashHex,
            chunkRootHex: bundle.chunkRootHex,
            porRootHex: String(repeating: "2", count: 64),
            leafCount: 1,
            segmentCount: 1,
            chunkCount: 1,
            sampleCount: 1,
            sampleSeed: 0,
            proofCount: 0,
            proofs: []
        )
        let generator = MockDaProofSummaryGenerator(value: summary)
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let (result, paths) = try await ToriiClient(baseURL: URL(string: "https://example.com")!)
            .proveDaAvailabilityToDirectory(
                manifestBundle: bundle,
                providers: [provider],
                outputDir: tempDir,
                proofSummaryOptions: ToriiDaProofSummaryOptions(sampleCount: 1, sampleSeed: 0),
                orchestrator: fetcher,
                proofSummaryGenerator: generator
            )
        XCTAssertEqual(result.manifest.storageTicketHex, bundle.storageTicketHex)
        XCTAssertTrue(FileManager.default.fileExists(atPath: paths.manifest.manifestURL.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: paths.manifest.manifestJsonURL.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: paths.manifest.chunkPlanURL.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: paths.payloadURL.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: paths.proofSummaryURL.path))
        XCTAssertTrue(FileManager.default.fileExists(atPath: paths.scoreboardURL?.path ?? ""))
        let scoreboardData = try Data(contentsOf: paths.scoreboardURL!)
        let scoreboardJSON = try JSONSerialization.jsonObject(with: scoreboardData) as? [[String: Any]]
        XCTAssertEqual(scoreboardJSON?.first?["alias"] as? String, "alpha")
    }

    func testGetDaProofPoliciesUsesEndpoint() async throws {
        ToriiDaStubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/da/proof-policies")
            XCTAssertEqual(request.httpMethod, "GET")
            let body = try JSONSerialization.data(withJSONObject: [
                "version": 1,
                "policy_hash": canonicalDaHash,
                "policies": []
            ], options: [.sortedKeys])
            return (200, ["Content-Type": "application/json"], body)
        }

        let client = makeHTTPClient()
        let value = try await client.getDaProofPolicies()
        XCTAssertEqual(value.version, 1)
        XCTAssertEqual(value.policyHash.literal, canonicalDaHash)
    }

    func testListDaCommitmentsPostsSnapshotCursorRequest() async throws {
        ToriiDaStubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/da/commitments")
            XCTAssertEqual(request.httpMethod, "POST")
            let payload = requestBodyData(from: request)
            XCTAssertFalse(payload.isEmpty)
            let json = try XCTUnwrap(
                JSONSerialization.jsonObject(with: payload) as? [String: Any]
            )
            XCTAssertEqual(Set(json.keys), ["limit", "cursor"])
            XCTAssertEqual(json["limit"] as? Int, 3)
            let cursor = try XCTUnwrap(json["cursor"] as? [String: Any])
            let snapshot = try XCTUnwrap(cursor["snapshot"] as? [String: Any])
            XCTAssertEqual(snapshot["block_height"] as? Int, 10)
            XCTAssertEqual(snapshot["block_hash"] as? String, canonicalDaHash)
            let after = try XCTUnwrap(cursor["after"] as? [String: Any])
            XCTAssertEqual(after["lane_id"] as? Int, 7)
            XCTAssertEqual(after["sequence"] as? Int, 11)
            let response = try JSONSerialization.data(withJSONObject: [
                "policies": [
                    "version": 1,
                    "policy_hash": canonicalDaHash,
                    "policies": []
                ],
                "commitments": [],
                "next_cursor": NSNull()
            ], options: [.sortedKeys])
            return (200, ["Content-Type": "application/json"], response)
        }

        let client = makeHTTPClient()
        let request = ToriiDaCommitmentListRequest(
            limit: 3,
            cursor: ToriiDaCommitmentListCursor(
                snapshot: try ToriiDaListSnapshot(
                    blockHeight: 10,
                    blockHash: ToriiDaHash(canonicalDaHash)
                ),
                after: ToriiDaCommitmentKey(
                    laneId: 7,
                    epoch: UInt64.max,
                    sequence: 11
                )
            )
        )
        let response = try await client.listDaCommitments(request)
        XCTAssertEqual(response.commitments.count, 0)
        XCTAssertNil(response.nextCursor)
    }

    func testListDaPinIntentsUsesPageEnvelopeAndCursor() async throws {
        let responseCursor: [String: Any] = [
            "snapshot": [
                "block_height": 10,
                "block_hash": canonicalDaHash
            ],
            "after": [
                "block_height": 9,
                "index_in_bundle": 2
            ]
        ]
        ToriiDaStubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/da/pin-intents")
            XCTAssertEqual(request.httpMethod, "POST")
            let payload = requestBodyData(from: request)
            let json = try XCTUnwrap(
                JSONSerialization.jsonObject(with: payload) as? [String: Any]
            )
            XCTAssertEqual(Set(json.keys), ["limit", "cursor"])
            XCTAssertEqual(json["limit"] as? Int, 5)
            let cursor = try XCTUnwrap(json["cursor"] as? [String: Any])
            let after = try XCTUnwrap(cursor["after"] as? [String: Any])
            XCTAssertEqual(after["block_height"] as? Int, 7)
            XCTAssertEqual(after["index_in_bundle"] as? Int, 1)
            let response = try JSONSerialization.data(withJSONObject: [
                "intents": [],
                "next_cursor": responseCursor
            ], options: [.sortedKeys])
            return (200, ["Content-Type": "application/json"], response)
        }

        let snapshot = try ToriiDaListSnapshot(
            blockHeight: 10,
            blockHash: ToriiDaHash(canonicalDaHash)
        )
        let request = ToriiDaPinIntentListRequest(
            limit: 5,
            cursor: ToriiDaPinIntentListCursor(
                snapshot: snapshot,
                after: try ToriiDaCommitmentLocation(
                    blockHeight: 7,
                    indexInBundle: 1
                )
            )
        )
        let response = try await makeHTTPClient().listDaPinIntents(request)
        XCTAssertTrue(response.intents.isEmpty)
        XCTAssertEqual(response.nextCursor?.snapshot, snapshot)
        XCTAssertEqual(response.nextCursor?.after.blockHeight, 9)
        XCTAssertEqual(response.nextCursor?.after.indexInBundle, 2)
    }

    func testProveDaPinIntentHandlesNullResponse() async throws {
        ToriiDaStubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/da/pin-intents/prove")
            XCTAssertEqual(request.httpMethod, "POST")
            let payload = requestBodyData(from: request)
            let json = try JSONSerialization.jsonObject(with: payload) as? [String: Any]
            let ticketWrapper = json?["storage_ticket"] as? [[Int]]
            XCTAssertEqual(ticketWrapper?.first, Array(repeating: 0x22, count: 32))
            return (200, ["Content-Type": "application/json"], Data("null".utf8))
        }

        let client = makeHTTPClient()
        let response = try await client.proveDaPinIntent(
            ToriiDaPinIntentQueryRequest(storageTicket: String(repeating: "2", count: 64))
        )
        XCTAssertNil(response)
    }

    func testVerifyDaCommitmentPostsProofBody() async throws {
        ToriiDaStubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/da/commitments/verify")
            XCTAssertEqual(request.httpMethod, "POST")
            let payload = requestBodyData(from: request)
            XCTAssertFalse(payload.isEmpty)
            let json = try JSONSerialization.jsonObject(with: payload) as? [String: Any]
            XCTAssertEqual(json?["bundle_len"] as? Int, 1)
            let commitment = try XCTUnwrap(json?["commitment"] as? [String: Any])
            XCTAssertNil(commitment["kzg_commitment"])
            XCTAssertTrue(commitment["proof_digest"] is NSNull)
            let body = Data(#"{"valid":true,"error":null}"#.utf8)
            return (200, ["Content-Type": "application/json"], body)
        }

        let client = makeHTTPClient()
        let proof = try sampleCommitmentProof()
        let response = try await client.verifyDaCommitment(proof: proof)
        XCTAssertTrue(response.valid)
        XCTAssertNil(response.error)
    }

    func testTypedDaProofPreservesU64MaxAndExplicitOptionalFields() throws {
        let proof = try sampleCommitmentProof()
        let encoded = try JSONEncoder().encode(proof)
        let original = try XCTUnwrap(String(data: encoded, encoding: .utf8))
        let maximumEpoch = original.replacingOccurrences(
            of: "\"epoch\":2",
            with: "\"epoch\":18446744073709551615"
        )
        XCTAssertNotEqual(maximumEpoch, original)

        let decoded = try JSONDecoder().decode(
            ToriiDaCommitmentProof.self,
            from: Data(maximumEpoch.utf8)
        )
        XCTAssertEqual(decoded.commitment.epoch, UInt64.max)

        let object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: encoded) as? [String: Any]
        )
        let commitment = try XCTUnwrap(object["commitment"] as? [String: Any])
        XCTAssertNil(commitment["kzg_commitment"])
        XCTAssertTrue(commitment["proof_digest"] is NSNull)
    }

    func testTypedDaModelsRejectBadHashPathAndVerifyInvariant() throws {
        let badChecksum = String(canonicalDaHash.dropLast()) + "A"
        XCTAssertThrowsError(try ToriiDaHash(badChecksum))
        let emptyGovernanceTag = ToriiDaRetentionPolicy(
            hotRetentionSecs: 0,
            coldRetentionSecs: 0,
            requiredReplicas: 0,
            storageClass: .hot,
            governanceTag: ""
        )
        let emptyTagWire = try JSONEncoder().encode(emptyGovernanceTag)
        XCTAssertEqual(
            try JSONDecoder()
                .decode(ToriiDaRetentionPolicy.self, from: emptyTagWire)
                .governanceTag,
            ""
        )

        let proof = try sampleCommitmentProof()
        let encodedProof = try JSONEncoder().encode(proof)
        let encodedText = try XCTUnwrap(String(data: encodedProof, encoding: .utf8))
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaCommitmentProof.self,
                from: Data(
                    encodedText
                        .replacingOccurrences(
                            of: "MerkleSha256",
                            with: "KzgBls12_381"
                        )
                        .utf8
                )
            )
        )
        XCTAssertThrowsError(
            try ToriiDaCommitmentProof(
                commitment: proof.commitment,
                location: proof.location,
                bundleHash: proof.bundleHash,
                bundleLength: 2,
                root: proof.root,
                path: []
            )
        )

        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaCommitmentVerifyResponse.self,
                from: Data(#"{"valid":false,"error":null}"#.utf8)
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaPinIntentVerifyResponse.self,
                from: Data(#"{"valid":true,"error":null,"ignored":1}"#.utf8)
            )
        )
    }

    func testDaCursorModelsRejectLegacyAndNonCanonicalShapes() throws {
        XCTAssertThrowsError(
            try JSONEncoder().encode(ToriiDaCommitmentListRequest(limit: 0))
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaCommitmentListRequest.self,
                from: Data(#"{"pagination":{"offset":1}}"#.utf8)
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaCommitmentProofRequest.self,
                from: Data(#"{"pagination":{"offset":1}}"#.utf8)
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaPinIntentQueryRequest.self,
                from: Data(#"{"pagination":{"offset":1}}"#.utf8)
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaListSnapshot.self,
                from: Data(
                    #"{"block_height":0,"block_hash":"\#(canonicalDaHash)"}"#.utf8
                )
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaListSnapshot.self,
                from: Data(#"{"block_height":1,"block_hash":null}"#.utf8)
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaListSnapshot.self,
                from: Data(#"{"block_height":0}"#.utf8)
            )
        )

        let legacyCommitmentPage = """
        {
          "policies":{"version":1,"policy_hash":"\(canonicalDaHash)","policies":[]},
          "commitments":[]
        }
        """
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaCommitmentListResponse.self,
                from: Data(legacyCommitmentPage.utf8)
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaPinIntentListResponse.self,
                from: Data("[]".utf8)
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDaPinIntentListResponse.self,
                from: Data(#"{"intents":[]}"#.utf8)
            )
        )
    }

    func testDaCursorPreservesFullUnsignedRangeAndExplicitNullHash() throws {
        let emptySnapshot = try ToriiDaListSnapshot(
            blockHeight: 0,
            blockHash: nil
        )
        let emptyWire = try JSONEncoder().encode(emptySnapshot)
        let emptyObject = try XCTUnwrap(
            JSONSerialization.jsonObject(with: emptyWire) as? [String: Any]
        )
        XCTAssertTrue(emptyObject["block_hash"] is NSNull)

        let cursor = ToriiDaCommitmentListCursor(
            snapshot: try ToriiDaListSnapshot(
                blockHeight: UInt64.max,
                blockHash: ToriiDaHash(canonicalDaHash)
            ),
            after: ToriiDaCommitmentKey(
                laneId: UInt32.max,
                epoch: UInt64.max,
                sequence: UInt64.max
            )
        )
        let encoded = try JSONEncoder().encode(cursor)
        let decoded = try JSONDecoder().decode(
            ToriiDaCommitmentListCursor.self,
            from: encoded
        )
        XCTAssertEqual(decoded, cursor)
        XCTAssertEqual(decoded.snapshot.blockHeight, UInt64.max)
        XCTAssertEqual(decoded.after.laneId, UInt32.max)
        XCTAssertEqual(decoded.after.epoch, UInt64.max)
        XCTAssertEqual(decoded.after.sequence, UInt64.max)
    }

    func testPinIntentAliasUsesServerUtf8ByteBound() throws {
        let empty = ToriiDaPinIntentQueryRequest(alias: "")
        _ = try JSONEncoder().encode(empty)

        let exact = ToriiDaPinIntentQueryRequest(alias: String(repeating: "é", count: 128))
        _ = try JSONEncoder().encode(exact)

        let oversized = ToriiDaPinIntentQueryRequest(alias: String(repeating: "é", count: 129))
        XCTAssertThrowsError(try JSONEncoder().encode(oversized))

        let digest = try ToriiDaDigest32(bytes: Array(repeating: 0x22, count: 32))
        let intent = try ToriiDaPinIntent(
            laneId: 1,
            epoch: 2,
            sequence: 3,
            storageTicket: digest,
            manifestHash: digest,
            alias: nil,
            owner: nil
        )
        let encodedIntent = try JSONEncoder().encode(intent)
        let intentObject = try XCTUnwrap(
            JSONSerialization.jsonObject(with: encodedIntent) as? [String: Any]
        )
        XCTAssertTrue(intentObject["alias"] is NSNull)
        XCTAssertTrue(intentObject["owner"] is NSNull)
    }

    // MARK: - Helpers

    private func makeHTTPClient() -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [ToriiDaStubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        return ToriiClient(baseURL: URL(string: "https://example.com")!, session: session)
    }

    private func decodeManifestBundle() throws -> ToriiDaManifestBundle {
        let manifestNorito = Data([0xAA, 0xBB]).base64EncodedString()
        let payload: [String: Any] = [
            "storage_ticket": String(repeating: "1", count: 64),
            "client_blob_id": String(repeating: "2", count: 64),
            "blob_hash": String(repeating: "3", count: 64),
            "manifest_hash": String(repeating: "5", count: 64),
            "chunk_root": String(repeating: "4", count: 64),
            "lane_id": 7,
            "epoch": 1,
            "manifest_norito": manifestNorito,
            "manifest": [
                "chunker_handle": "sorafs.chunker@1.0.0"
            ],
            "chunk_plan": [
                "schema": "sorafs.chunk_fetch_plan.v1",
                "payload_digest_blake3_hex": String(repeating: "3", count: 64),
                "chunk_fetch_specs": [[
                    "chunk_index": 0,
                    "offset": 0,
                    "length": 8,
                    "digest_blake3": String(repeating: "22", count: 32)
                ]]
            ]
        ]
        let data = try JSONSerialization.data(withJSONObject: payload, options: [.sortedKeys])
        let decoder = JSONDecoder()
        return try decoder.decode(ToriiDaManifestBundle.self, from: data)
    }

    private func sampleProvider() throws -> SorafsGatewayProvider {
        try SorafsGatewayProvider(
            name: "p1",
            providerIdHex: String(repeating: "a", count: 64),
            gatewayPublicKeyHex: String(repeating: "ab", count: 32),
            baseURL: URL(string: "https://p1.example.com")!,
            streamTokenB64: Data([0x01, 0x02]).base64EncodedString()
        )
    }

    private func sampleCommitmentProof() throws -> ToriiDaCommitmentProof {
        let digest = [Array(repeating: 0x11, count: 32)]
        let record: [String: Any] = [
            "lane_id": 1,
            "epoch": 2,
            "sequence": 3,
            "client_blob_id": digest,
            "manifest_hash": digest,
            "proof_scheme": ["type": "MerkleSha256", "value": NSNull()],
            "chunk_root": canonicalDaHash,
            "proof_digest": NSNull(),
            "retention_class": [
                "hot_retention_secs": 10,
                "cold_retention_secs": 20,
                "required_replicas": 1,
                "storage_class": ["type": "Hot", "value": NSNull()],
                "governance_tag": ["da.default"],
            ],
            "storage_ticket": digest,
            "acknowledgement_sig": String(repeating: "A", count: 128),
        ]
        let payload: [String: Any] = [
            "commitment": record,
            "location": ["block_height": 4, "index_in_bundle": 0],
            "bundle_hash": canonicalDaHash,
            "bundle_len": 1,
            "root": canonicalDaHash,
            "path": [],
        ]
        let encoded = try JSONSerialization.data(withJSONObject: payload, options: [.sortedKeys])
        return try JSONDecoder().decode(ToriiDaCommitmentProof.self, from: encoded)
    }
}

private func requestBodyData(from request: URLRequest) -> Data {
    if let body = request.httpBody {
        return body
    }
    guard let stream = request.httpBodyStream else {
        return Data()
    }
    stream.open()
    defer { stream.close() }
    var data = Data()
    var buffer = [UInt8](repeating: 0, count: 4096)
    while stream.hasBytesAvailable {
        let read = stream.read(&buffer, maxLength: buffer.count)
        if read <= 0 {
            break
        }
        data.append(buffer, count: read)
    }
    return data
}

private let canonicalDaHash =
    "hash:0F923F0F972DB7373EFB38439B74651907459ECE1EF94564CCECF063F8893D85#C1CB"

private final class ToriiDaStubURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (Int, [String: String], Data))?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }
        do {
            let (status, headers, body) = try handler(request)
            let response = try XCTUnwrap(
                HTTPURLResponse(
                    url: request.url ?? URL(string: "https://example.com")!,
                    statusCode: status,
                    httpVersion: nil,
                    headerFields: headers
                )
            )
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: body)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}

private final class MockGatewayFetcher: SorafsGatewayFetching, @unchecked Sendable {
    var lastOptions: SorafsGatewayFetchOptions?
    var scoreboard: [SorafsGatewayFetchReport.ScoreboardEntry]? = nil

    func fetchGatewayPayload(
        plan: ToriiJSONValue,
        providers: [SorafsGatewayProvider],
        options: SorafsGatewayFetchOptions?,
        cancellationHandler: (() -> Void)?
    ) async throws -> SorafsGatewayFetchResult {
        lastOptions = options
        let report = SorafsGatewayFetchReport(
            chunkCount: 1,
            providerReports: [],
            chunkReceipts: [],
            scoreboard: scoreboard
        )
        return SorafsGatewayFetchResult(
            payload: Data([0x00]),
            report: report,
            reportJSON: "{}"
        )
    }
}

private struct MockDaProofSummaryGenerator: DaProofSummaryGenerating {
    let value: ToriiDaProofSummary

    func makeProofSummary(manifest: Data,
                          payload: Data,
                          options: ToriiDaProofSummaryOptions) throws -> ToriiDaProofSummary {
        return value
    }
}
