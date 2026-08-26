import Foundation
import CryptoKit
import XCTest
@testable import IrohaSwift

#if os(macOS)
final class ToriiClientIntegrationTests: XCTestCase {
    private var mock: ToriiMockProcess?

    private var canonicalRequestAuth: ToriiCanonicalRequestAuth {
        let seed = Data(repeating: 0x41, count: 32)
        return ToriiCanonicalRequestAuth(
            accountId: try! Keypair(privateKeyBytes: seed).accountId(
                networkPrefix: AccountId.defaultNetworkPrefix
            ),
            privateKey: seed,
            timestampMs: 4_102_444_801_000,
            nonce: "torii-integration-pipeline-test"
        )
    }

    override func setUpWithError() throws {
        try super.setUpWithError()
        guard let server = ToriiMockProcess() else {
            try failRequiredNativeTestCapability(
                "python interpreter not available for Torii mock"
            )
        }
        mock = server
    }

    override func tearDown() {
        mock?.stop()
        mock = nil
        super.tearDown()
    }

    func testAttachmentLifecycleAgainstMock() throws {
        guard let mock else { return }
        let session = mock.makeSecureSession()
        let seed = Data(repeating: 0x41, count: 32)
        let accountId = try Keypair(privateKeyBytes: seed).accountId(networkPrefix: AccountId.defaultNetworkPrefix)
        let canonicalAuth = ToriiCanonicalRequestAuth(accountId: accountId, privateKey: seed)
        let client = ToriiClient(baseURL: mock.secureBaseURL, session: session,
                                 localSigningContext: ToriiLocalSigningContext(networkId: TestNetworkIds.canonical))
        let payload = Data("{\"hello\":\"swift\"}".utf8)

        var attachmentId: String?
        let uploadExpectation = expectation(description: "upload")
        client.uploadAttachment(data: payload, contentType: "application/json", canonicalAuth: canonicalAuth) { result in
            switch result {
            case .success(let meta):
                attachmentId = meta.id
            case .failure(let error):
                XCTFail("upload failed: \(error)")
            }
            uploadExpectation.fulfill()
        }
        wait(for: [uploadExpectation], timeout: 5)

        guard let id = attachmentId else {
            XCTFail("attachment id missing")
            return
        }

        let listExpectation = expectation(description: "list")
        client.listAttachments(canonicalAuth: canonicalAuth) { result in
            switch result {
            case .success(let metas):
                XCTAssertTrue(metas.contains(where: { $0.id == id }))
            case .failure(let error):
                XCTFail("list failed: \(error)")
            }
            listExpectation.fulfill()
        }
        wait(for: [listExpectation], timeout: 5)

        let getExpectation = expectation(description: "get")
        client.getAttachment(id: id, canonicalAuth: canonicalAuth) { result in
            switch result {
            case .success(let (data, contentType)):
                XCTAssertEqual(data, payload)
                XCTAssertEqual(contentType, "application/json")
            case .failure(let error):
                XCTFail("get failed: \(error)")
            }
            getExpectation.fulfill()
        }
        wait(for: [getExpectation], timeout: 5)

        let deleteExpectation = expectation(description: "delete")
        client.deleteAttachment(id: id, canonicalAuth: canonicalAuth) { result in
            if case let .failure(error) = result {
                XCTFail("delete failed: \(error)")
            }
            deleteExpectation.fulfill()
        }
        wait(for: [deleteExpectation], timeout: 5)

        let listAfterExpectation = expectation(description: "list after")
        client.listAttachments(canonicalAuth: canonicalAuth) { result in
            switch result {
            case .success(let metas):
                XCTAssertFalse(metas.contains(where: { $0.id == id }))
            case .failure(let error):
                XCTFail("list-after failed: \(error)")
            }
            listAfterExpectation.fulfill()
        }
        wait(for: [listAfterExpectation], timeout: 5)
    }

    func testProverReportsFlowAgainstMock() throws {
        guard let mock else { return }
        let client = ToriiClient(baseURL: mock.secureBaseURL, session: mock.makeSecureSession())

        var initialReports: [ToriiProverReport] = []
        let listExpectation = expectation(description: "prover list")
        client.listProverReports { result in
            switch result {
            case .success(let reports):
                initialReports = reports
                XCTAssertFalse(reports.isEmpty)
            case .failure(let error):
                XCTFail("list failed: \(error)")
            }
            listExpectation.fulfill()
        }
        wait(for: [listExpectation], timeout: 5)

        guard let first = initialReports.first else {
            XCTFail("no prover reports available")
            return
        }

        let getExpectation = expectation(description: "prover get")
        client.getProverReport(id: first.id) { result in
            switch result {
            case .success(let report):
                XCTAssertEqual(report.id, first.id)
            case .failure(let error):
                XCTFail("get failed: \(error)")
            }
            getExpectation.fulfill()
        }
        wait(for: [getExpectation], timeout: 5)

        let deleteExpectation = expectation(description: "prover delete")
        client.deleteProverReport(id: first.id) { result in
            if case let .failure(error) = result {
                XCTFail("delete failed: \(error)")
            }
            deleteExpectation.fulfill()
        }
        wait(for: [deleteExpectation], timeout: 5)

        let countExpectation = expectation(description: "prover count")
        client.countProverReports { result in
            switch result {
            case .success(let count):
                XCTAssertEqual(count, UInt64(max(initialReports.count - 1, 0)))
            case .failure(let error):
                XCTFail("count failed: \(error)")
            }
            countExpectation.fulfill()
        }
        wait(for: [countExpectation], timeout: 5)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testPipelineSubmitAndWaitSuccessAgainstMock() async throws {
        let scenarioHash = "feedfacecafebeefcafedeadbeef000100000000000000000000000000000000"
        try await preparePipelineScenario(.success,
                                          hashHex: scenarioHash,
                                          statusKinds: ["Queued", "Approved", "Committed", "Applied"])
        let mock = try XCTUnwrap(self.mock)
        let session = mock.makeSecureSession()
        let client = ToriiClient(
            baseURL: mock.secureBaseURL,
            session: session,
            localSigningContext: ToriiLocalSigningContext(
                networkId: TestNetworkIds.canonical
            ),
            canonicalRequestAuth: canonicalRequestAuth
        )
        let sdk = IrohaSDK(toriiClient: client)
        sdk.pipelinePollOptions = PipelineStatusPollOptions(pollInterval: 0.01, timeout: 1)
        let envelope = try tcMakePipelineEnvelope(hashHex: scenarioHash, marker: 0x11)
        let status = try await sdk.submitAndWait(envelope: envelope)
        XCTAssertEqual(status.hash, scenarioHash)
        XCTAssertEqual(status.status.state, .applied)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testPipelineSubmitAndWaitFailureAgainstMock() async throws {
        let scenarioHash = "feedfacecafebeefcafedeadbeef000200000000000000000000000000000000"
        try await preparePipelineScenario(.failure, hashHex: scenarioHash)
        let mock = try XCTUnwrap(self.mock)
        let session = mock.makeSecureSession()
        let client = ToriiClient(
            baseURL: mock.secureBaseURL,
            session: session,
            localSigningContext: ToriiLocalSigningContext(
                networkId: TestNetworkIds.canonical
            ),
            canonicalRequestAuth: canonicalRequestAuth
        )
        let sdk = IrohaSDK(toriiClient: client)
        sdk.pipelinePollOptions = PipelineStatusPollOptions(pollInterval: 0.01, timeout: 1)
        let envelope = try tcMakePipelineEnvelope(hashHex: scenarioHash, marker: 0x22)
        do {
            _ = try await sdk.submitAndWait(envelope: envelope)
            XCTFail("expected pipeline failure")
        } catch let error as PipelineStatusError {
            guard case let .failure(hash, status, payload) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(hash, scenarioHash)
            XCTAssertEqual(status, "Rejected")
            XCTAssertEqual(payload.hash, scenarioHash)
            XCTAssertEqual(payload.status.kind, "Rejected")
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testPipelineSubmitAndWaitTimeoutAgainstMock() async throws {
        let scenarioHash = "feedfacecafebeefcafedeadbeef000300000000000000000000000000000000"
        try await preparePipelineScenario(.timeout,
                                          hashHex: scenarioHash,
                                          statusKinds: ["Queued"],
                                          repeatLast: true)
        let mock = try XCTUnwrap(self.mock)
        let session = mock.makeSecureSession()
        let client = ToriiClient(
            baseURL: mock.secureBaseURL,
            session: session,
            localSigningContext: ToriiLocalSigningContext(
                networkId: TestNetworkIds.canonical
            ),
            canonicalRequestAuth: canonicalRequestAuth
        )
        let sdk = IrohaSDK(toriiClient: client)
        sdk.pipelinePollOptions = PipelineStatusPollOptions(pollInterval: 0.01,
                                                            timeout: 0.3,
                                                            maxAttempts: 3)
        let envelope = try tcMakePipelineEnvelope(hashHex: scenarioHash, marker: 0x33)
        do {
            _ = try await sdk.submitAndWait(envelope: envelope)
            XCTFail("expected pipeline timeout")
        } catch let error as PipelineStatusError {
            guard case let .timeout(hash, attempts) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(hash, scenarioHash)
            XCTAssertGreaterThanOrEqual(attempts, 3)
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    private enum PipelineScenario: String {
        case success
        case failure
        case timeout
    }

    private enum IntegrationError: Error {
        case invalidHashEncoding
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetDaManifestBundleDecodesResponse() async throws {
        let ticket = String(repeating: "a", count: 64)
        let manifestPayload = Data([0xDE, 0xAD, 0xBE, 0xEF])
        let manifestObject: [String: Any] = ["chunker_handle": "demo.profile@1.0.0"]
        let chunkPlanObject: [String: Any] = [
            "schema": "sorafs.chunk_fetch_plan.v1",
            "payload_digest_blake3_hex": String(repeating: "c", count: 64),
            "chunk_fetch_specs": [[
                "chunk_index": 0,
                "offset": 0,
                "length": 262_144,
                "digest_blake3": String(repeating: "22", count: 32)
            ]]
        ]
        var responseObject: [String: Any] = [
            "storage_ticket": ticket,
            "client_blob_id": String(repeating: "b", count: 64),
            "blob_hash": String(repeating: "c", count: 64),
            "manifest_hash": String(repeating: "e", count: 64),
            "chunk_root": String(repeating: "d", count: 64),
            "lane_id": 7,
            "epoch": 42,
            "manifest_len": manifestPayload.count,
            "manifest_norito": manifestPayload.base64EncodedString()
        ]
        responseObject["manifest"] = manifestObject
        responseObject["chunk_plan"] = chunkPlanObject

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/da/manifests/\(ticket)")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = try JSONSerialization.data(withJSONObject: responseObject, options: [.sortedKeys])
            return (response, body)
        }

        let bundle = try await tcMakeClient().getDaManifestBundle(storageTicketHex: ticket.uppercased())
        XCTAssertEqual(bundle.storageTicketHex, ticket)
        XCTAssertEqual(bundle.clientBlobIdHex, String(repeating: "b", count: 64))
        XCTAssertEqual(bundle.blobHashHex, String(repeating: "c", count: 64))
        XCTAssertEqual(bundle.manifestBytes, manifestPayload)
        XCTAssertEqual(bundle.laneId, 7)
        XCTAssertEqual(bundle.epoch, 42)
        guard case let .object(manifestJSON)? = bundle.manifestJson else {
            return XCTFail("missing manifest details")
        }
        XCTAssertEqual(manifestJSON["chunker_handle"], ToriiJSONValue.string("demo.profile@1.0.0"))
        let planJSONString = try bundle.chunkPlanJSONString()
        let decodedPlan = try JSONSerialization.jsonObject(with: Data(planJSONString.utf8)) as? NSDictionary
        XCTAssertEqual(decodedPlan, chunkPlanObject as NSDictionary)
    }

    func testDaManifestBundleRejectsRetiredOrUnboundChunkPlans() throws {
        let chunkSpecs: [ToriiJSONValue] = [
            .object([
                "chunk_index": .number(0),
                "offset": .number(0),
                "length": .number(1),
                "digest_blake3": .string(String(repeating: "22", count: 32))
            ])
        ]
        let invalidPlans: [(String, ToriiJSONValue)] = [
            ("retired bare array", .array(chunkSpecs)),
            (
                "missing payload digest",
                .object([
                    "schema": .string("sorafs.chunk_fetch_plan.v1"),
                    "chunk_fetch_specs": .array(chunkSpecs)
                ])
            ),
            (
                "zero payload digest",
                .object([
                    "schema": .string("sorafs.chunk_fetch_plan.v1"),
                    "payload_digest_blake3_hex": .string(String(repeating: "00", count: 32)),
                    "chunk_fetch_specs": .array(chunkSpecs)
                ])
            ),
            (
                "substituted payload digest",
                .object([
                    "schema": .string("sorafs.chunk_fetch_plan.v1"),
                    "payload_digest_blake3_hex": .string(String(repeating: "11", count: 32)),
                    "chunk_fetch_specs": .array(chunkSpecs)
                ])
            )
        ]

        for (label, plan) in invalidPlans {
            var raw = tcMakeSampleManifestRaw()
            raw["chunk_plan"] = plan
            XCTAssertThrowsError(try ToriiDaManifestBundle(raw: raw), label)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testFetchDaPayloadViaGatewayUsesInjectedOrchestrator() async throws {
        let ticket = String(repeating: "e", count: 64)
        let manifestPayload = Data([0xAA, 0xBB, 0xCC])
        let manifestObject: [String: Any] = ["chunker_handle": "demo.chunker@2.1.0"]
        let chunkPlanObject: [String: Any] = [
            "schema": "sorafs.chunk_fetch_plan.v1",
            "payload_digest_blake3_hex": String(repeating: "2", count: 64),
            "chunk_fetch_specs": [[
                "chunk_index": 0,
                "offset": 0,
                "length": 1,
                "digest_blake3": String(repeating: "22", count: 32)
            ]]
        ]
        var responseObject: [String: Any] = [
            "storage_ticket": ticket,
            "client_blob_id": String(repeating: "1", count: 64),
            "blob_hash": String(repeating: "2", count: 64),
            "manifest_hash": String(repeating: "4", count: 64),
            "chunk_root": String(repeating: "3", count: 64),
            "lane_id": 3,
            "epoch": 9,
            "manifest_len": manifestPayload.count,
            "manifest_norito": manifestPayload.base64EncodedString()
        ]
        responseObject["manifest"] = manifestObject
        responseObject["chunk_plan"] = chunkPlanObject

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/da/manifests/\(ticket)")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = try JSONSerialization.data(withJSONObject: responseObject, options: [.sortedKeys])
            return (response, body)
        }

        let provider = try SorafsGatewayProvider(
            name: "demo",
            providerIdHex: String(repeating: "f", count: 64),
            gatewayPublicKeyHex: String(repeating: "ab", count: 32),
            baseURL: URL(string: "https://gateway.test")!,
            streamTokenB64: Data("token".utf8).base64EncodedString()
        )
        let report = SorafsGatewayFetchReport(
            chunkCount: 1,
            providerReports: [],
            chunkReceipts: [],
            scoreboard: nil
        )
        let payload = Data("payload".utf8)
        let fetchResult = SorafsGatewayFetchResult(payload: payload, report: report, reportJSON: "{}")
        let orchestrator = StubGatewayFetcher(result: fetchResult)

        let result = try await tcMakeClient().fetchDaPayloadViaGateway(
            storageTicketHex: ticket,
            providers: [provider],
            orchestrator: orchestrator
        )
        XCTAssertEqual(result.manifest.storageTicketHex, ticket)
        XCTAssertEqual(result.chunkerHandle, "demo.chunker@2.1.0")
        XCTAssertEqual(result.gatewayResult.payload, payload)
        XCTAssertEqual(orchestrator.capturedProviders?.count, 1)
        let expectedPlanData = try JSONSerialization.data(withJSONObject: chunkPlanObject, options: [.sortedKeys])
        let expectedPlan = try JSONDecoder().decode(ToriiJSONValue.self, from: expectedPlanData)
        XCTAssertEqual(orchestrator.capturedPlan, expectedPlan)
    }

    func testGatewayFetchReportDecodesTelemetryRegion() throws {
        let json = #"""
        {
            "chunk_count": 1,
            "provider_reports": [],
            "chunk_receipts": [],
            "scoreboard": null,
            "telemetry_region": "iad-prod"
        }
        """#
        let report = try SorafsGatewayFetchReport.decode(from: json)
        XCTAssertEqual(report.telemetryRegion, "iad-prod")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitDaBlobPostsPayloadAndParsesReceipt() async throws {
        let digest = Data(repeating: 0xAB, count: 32)
        let privateKeyBytes = Data(repeating: 0x11, count: 32)
        let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: privateKeyBytes)
        let owner = try AccountAddress.fromAccount(
            publicKey: privateKey.publicKey.rawRepresentation
        ).toI105(networkPrefix: AccountId.defaultNetworkPrefix)
        var submission = ToriiDaBlobSubmission(
            networkId: TestNetworkIds.canonical,
            owner: owner,
            payload: Data("payload".utf8),
            laneId: 9,
            epoch: 4,
            sequence: 2,
            metadata: [
                ToriiDaMetadataEntry(key: "da.stream", value: Data("demo".utf8))
            ],
            clientBlobId: digest,
            privateKeyHex: privateKeyBytes.upperHexString()
        )
        submission.codec = "application/octet-stream"

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/da/ingest")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            let body = tcBodyJSON(from: request)
            XCTAssertEqual(
                body["network_id"] as? String,
                TestNetworkIds.canonical.noritoJSONLiteral
            )
            XCTAssertEqual(body["owner"] as? String, owner)
            XCTAssertEqual(body["lane_id"] as? Int, 9)
            XCTAssertEqual(body["epoch"] as? Int, 4)
            XCTAssertEqual(body["sequence"] as? Int, 2)
            XCTAssertEqual(body["chunk_size"] as? Int, 262_144)
            XCTAssertEqual(body["codec"] as? [String], ["application/octet-stream"])
            if let clientTuple = body["client_blob_id"] as? [[NSNumber]],
               let first = clientTuple.first {
                XCTAssertEqual(first.count, 32)
                XCTAssertEqual(first.map { $0.intValue }, digest.map { Int($0) })
            } else {
                XCTFail("missing client blob id")
            }

            let digestArray = digest.map { NSNumber(value: Int($0)) }
            let responseTicket = (0..<32).map { _ in NSNumber(value: 0x31) }
            let receiptPayload: [String: Any] = [
                "client_blob_id": [digestArray],
                "lane_id": 9,
                "epoch": 4,
                "blob_hash": [digestArray],
                "chunk_root": [digestArray],
                "manifest_hash": [digestArray],
                "storage_ticket": [responseTicket],
                "pdp_commitment": Data("commit".utf8).base64EncodedString(),
                "stripe_layout": [
                    "total_stripes": 1,
                    "shards_per_stripe": 14,
                    "row_parity_stripes": 0
                ],
                "queued_at_unix": 1_700_000_000,
                "operator_signature": "DEADBEEF",
                "rent_quote": [
                    "base_rent": 900,
                    "protocol_reserve": "180",
                    "provider_reward": 720,
                    "pdp_bonus": "45",
                    "potr_bonus": 30,
                    "egress_credit_per_gib": "3"
                ]
            ]
            let responseObject: [String: Any] = [
                "status": "Accepted",
                "duplicate": false,
                "receipt": receiptPayload
            ]
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 202,
                httpVersion: nil,
                headerFields: [
                    "Content-Type": "application/json",
                    ToriiPdpCommitmentHeader: "base64-header"
                ]
            )!
            let bodyData = try JSONSerialization.data(withJSONObject: responseObject, options: [.sortedKeys])
            return (response, bodyData)
        }

        let result = try await tcMakeClient().submitDaBlob(submission)
        XCTAssertEqual(result.status, "Accepted")
        XCTAssertFalse(result.duplicate)
        XCTAssertEqual(result.artifacts.clientBlobIdHex, digest.upperHexString())
        XCTAssertEqual(result.artifacts.payloadLength, submission.payload.count)
        XCTAssertEqual(result.pdpCommitmentHeaderBase64, "base64-header")
        guard let receipt = result.receipt else {
            return XCTFail("missing receipt")
        }
        XCTAssertEqual(receipt.laneId, 9)
        XCTAssertEqual(receipt.epoch, 4)
        XCTAssertEqual(receipt.operatorSignatureHex, "DEADBEEF")
        let rentQuote = receipt.rentQuote
        XCTAssertEqual(rentQuote.baseRentMicro, "900")
        XCTAssertEqual(rentQuote.protocolReserveMicro, "180")
        XCTAssertEqual(rentQuote.providerRewardMicro, "720")
        XCTAssertEqual(rentQuote.pdpBonusMicro, "45")
        XCTAssertEqual(rentQuote.potrBonusMicro, "30")
        XCTAssertEqual(rentQuote.egressCreditPerGibMicro, "3")
    }

    @available(iOS 15.0, macOS 12.0, *)
    private func preparePipelineScenario(_ scenario: PipelineScenario,
                                         hashHex: String,
                                         statusKinds: [String]? = nil,
                                         repeatLast: Bool? = nil,
                                         accepted: Bool? = nil) async throws {
        let mock = try XCTUnwrap(self.mock)
        try await mock.resetState()
        try await mock.configurePipeline(scenario: scenario.rawValue,
                                         hash: hashHex,
                                         statusKinds: statusKinds,
                                         repeatLast: repeatLast,
                                         accepted: accepted)
    }

    private func makeSampleManifestRaw(storageTicket: String = String(repeating: "aa", count: 32)) -> [String: ToriiJSONValue] {
        let manifestBytes = Data("sample-manifest".utf8).base64EncodedString()
        return [
            "storage_ticket": .string(storageTicket),
            "client_blob_id": .string(String(repeating: "bb", count: 32)),
            "blob_hash": .string(String(repeating: "cc", count: 32)),
            "manifest_hash": .string(String(repeating: "ff", count: 32)),
            "chunk_root": .string(String(repeating: "dd", count: 32)),
            "lane_id": .number(1),
            "epoch": .number(2),
            "manifest_len": .number(16),
            "manifest_norito": .string(manifestBytes),
            "manifest": .object([
                "chunking": .object([
                    "namespace": .string("sorafs"),
                    "name": .string("sf1"),
                    "semver": .string("1.0.0")
                ])
            ]),
            "chunk_plan": .object([
                "schema": .string("sorafs.chunk_fetch_plan.v1"),
                "payload_digest_blake3_hex": .string(String(repeating: "cc", count: 32)),
                "chunk_fetch_specs": .array([
                    .object([
                        "chunk_index": .number(0),
                        "offset": .number(0),
                        "length": .number(4),
                        "digest_blake3": .string(String(repeating: "ee", count: 32))
                    ])
                ])
            ])
        ]
    }

    private func makeSampleManifestBundle(storageTicket: String = String(repeating: "aa", count: 32)) throws -> ToriiDaManifestBundle {
        try ToriiDaManifestBundle(raw: makeSampleManifestRaw(storageTicket: storageTicket))
    }

    private func makeGatewayFetchResult() -> SorafsGatewayFetchResult {
        let report = SorafsGatewayFetchReport(
            chunkCount: 1,
            providerReports: [],
            chunkReceipts: [],
            scoreboard: nil
        )
        return SorafsGatewayFetchResult(
            payload: Data([0x01, 0x02]),
            report: report,
            reportJSON: #"{"chunk_count":1}"#
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    private func makePipelineEnvelope(hashHex: String, marker: UInt8) throws -> SignedTransactionEnvelope {
        guard let hashData = Data(hexString: hashHex) else {
            XCTFail("invalid hash hex \(hashHex)")
            throw IntegrationError.invalidHashEncoding
        }
        let payload = Data([marker, marker ^ 0xFF, 0xA5])
        return SignedTransactionEnvelope(norito: payload,
                                         signedTransaction: payload,
                                         payload: nil,
                                         transactionHash: hashData)
    }

    private func loadDaProofFixture() throws -> (manifest: Data, payload: Data, blobHashHex: String) {
        let fixtureRoot = repositoryRootURL()
            .appendingPathComponent("fixtures/da/reconstruct/rs_parity_v1", isDirectory: true)
        let manifestHexURL = fixtureRoot.appendingPathComponent("manifest.norito.hex")
        let manifestJSONURL = fixtureRoot.appendingPathComponent("manifest.json")
        let payloadURL = fixtureRoot.appendingPathComponent("payload.bin")

        let manifestHex = try String(contentsOf: manifestHexURL, encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let manifestData = try XCTUnwrap(
            Data(hexString: manifestHex),
            "failed to decode DA manifest fixture"
        )
        let payloadData = try Data(contentsOf: payloadURL)
        let manifestJSONData = try Data(contentsOf: manifestJSONURL)
        let manifestObject = try XCTUnwrap(
            try JSONSerialization.jsonObject(with: manifestJSONData) as? [String: Any],
            "DA manifest fixture must be a JSON object"
        )
        let blobArray = try XCTUnwrap(
            manifestObject["blob_hash"] as? [[NSNumber]],
            "blob_hash fixture missing"
        )
        let blobBytes = try XCTUnwrap(blobArray.first, "blob_hash fixture is empty")
        let blobHex = blobBytes.reduce(into: "") { partialResult, value in
            partialResult.append(String(format: "%02x", value.uint8Value))
        }
        return (manifestData, payloadData, blobHex)
    }

    private func repositoryRootURL() -> URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // ToriiClientTests.swift
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // IrohaSwift
    }

    private func makeStubProofSummary() -> ToriiDaProofSummary {
        let proof = ToriiDaProofRecord(
            origin: "explicit",
            leafIndex: 0,
            chunkIndex: 0,
            segmentIndex: 0,
            leafOffset: 0,
            leafLength: 32,
            segmentOffset: 0,
            segmentLength: 32,
            chunkOffset: 0,
            chunkLength: 32,
            payloadLength: 32,
            chunkDigestHex: "aa",
            chunkRootHex: "bb",
            segmentDigestHex: "cc",
            leafDigestHex: "dd",
            leafBytes: Data(),
            segmentLeavesHex: [],
            chunkSegmentsHex: [],
            chunkCount: 1,
            chunkMerklePathHex: [],
            verified: true
        )
        return ToriiDaProofSummary(
            blobHashHex: "aa",
            chunkRootHex: "bb",
            porRootHex: "cc",
            leafCount: 1,
            segmentCount: 1,
            chunkCount: 1,
            sampleCount: 0,
            sampleSeed: 0,
            proofCount: 1,
            proofs: [proof]
        )
    }
}
#endif
