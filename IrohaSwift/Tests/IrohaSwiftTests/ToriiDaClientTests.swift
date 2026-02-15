import Foundation
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
        try XCTSkipIf(!NoritoNativeBridge.shared.isAvailable,
                      "NoritoBridge is required to derive DA payload digest")
        let submission = ToriiDaBlobSubmission(
            payload: Data([0x01, 0x02]),
            laneId: 7,
            epoch: 1,
            sequence: 0,
            blobClass: .taikaiSegment,
            codec: "custom.binary",
            clientBlobId: Data(repeating: 0x22, count: 32)
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
            XCTAssertEqual(request.url?.path, "/v1/da/proof_policies")
            XCTAssertEqual(request.httpMethod, "GET")
            let body = try JSONSerialization.data(withJSONObject: [
                "version": 1,
                "policy_hash": String(repeating: "a", count: 64),
                "policies": []
            ], options: [.sortedKeys])
            return (200, ["Content-Type": "application/json"], body)
        }

        let client = makeHTTPClient()
        let value = try await client.getDaProofPolicies()
        guard case .object(let object) = value else {
            XCTFail("expected object response")
            return
        }
        XCTAssertEqual(object["version"]?.normalizedUInt64, 1)
    }

    func testListDaCommitmentsPostsNormalizedRequest() async throws {
        ToriiDaStubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/da/commitments")
            XCTAssertEqual(request.httpMethod, "POST")
            let payload = try XCTUnwrap(request.httpBody)
            let json = try JSONSerialization.jsonObject(with: payload) as? [String: Any]
            XCTAssertEqual(json?["manifest_hash"] as? String, String(repeating: "1", count: 64))
            XCTAssertEqual(json?["lane_id"] as? Int, 7)
            let response = try JSONSerialization.data(withJSONObject: [
                "policies": [
                    "version": 1,
                    "policy_hash": String(repeating: "a", count: 64),
                    "policies": []
                ],
                "commitments": []
            ], options: [.sortedKeys])
            return (200, ["Content-Type": "application/json"], response)
        }

        let client = makeHTTPClient()
        let request = ToriiDaCommitmentProofRequest(
            manifestHash: "0x" + String(repeating: "1", count: 64),
            laneId: 7,
            epoch: 9,
            sequence: 11,
            pagination: ToriiQueryPagination(limit: 3, offset: 1)
        )
        let response = try await client.listDaCommitments(request)
        XCTAssertEqual(response.commitments.count, 0)
    }

    func testProveDaPinIntentHandlesNullResponse() async throws {
        ToriiDaStubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/da/pin_intents/prove")
            XCTAssertEqual(request.httpMethod, "POST")
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
            let payload = try XCTUnwrap(request.httpBody)
            let json = try JSONSerialization.jsonObject(with: payload) as? [String: Any]
            XCTAssertEqual(json?["bundle_len"] as? Int, 1)
            let body = Data(#"{"valid":true}"#.utf8)
            return (200, ["Content-Type": "application/json"], body)
        }

        let client = makeHTTPClient()
        let proof = ToriiJSONValue.object(["bundle_len": .number(1)])
        let response = try await client.verifyDaCommitment(proof: proof)
        XCTAssertTrue(response.valid)
        XCTAssertNil(response.error)
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
                [
                    "chunk_index": 0,
                    "offset": 0,
                    "length": 8,
                    "provider": "p1"
                ]
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
            baseURL: URL(string: "https://p1.example.com")!,
            streamTokenB64: Data([0x01, 0x02]).base64EncodedString()
        )
    }
}

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
