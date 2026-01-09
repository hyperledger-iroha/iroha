import XCTest
#if canImport(Combine)
import Combine
#endif
@testable import IrohaSwift

private final class StubURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data?))?

    override class func canInit(with request: URLRequest) -> Bool { true }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(self, didFailWithError: NSError(domain: "Stub", code: -1))
            return
        }
        do {
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            if let data { client?.urlProtocol(self, didLoad: data) }
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}

private final class ToriiMockProcess {
    private let process: Process
    private let stdoutPipe: Pipe
    private let stderrPipe: Pipe
    let baseURL: URL

    init?() {
        let candidates = ["python3", "python"]
        var lastError: Error?
        var launchedProcess: Process?
        var stdout: Pipe?
        var stderr: Pipe?
        var baseURL: URL?

        for candidate in candidates {
            let proc = Process()
            proc.executableURL = URL(fileURLWithPath: "/usr/bin/env")
            proc.arguments = [candidate, "-m", "iroha_torii_client.mock", "--stdio"]
            proc.environment = Self.makeEnvironment()
            stdout = Pipe()
            stderr = Pipe()
            proc.standardOutput = stdout
            proc.standardError = stderr

            do {
                try proc.run()
            } catch {
                lastError = error
                continue
            }

            if let url = Self.readBaseURL(from: stdout!) {
                launchedProcess = proc
                baseURL = url
                break
            }

            Self.terminateProcess(proc)
        }

        guard let runningProcess = launchedProcess,
              let runningStdout = stdout,
              let runningStderr = stderr,
              let resolvedURL = baseURL
        else {
            if let error = lastError {
                FileHandle.standardError.write(Data("Torii mock launch error: \(error)\n".utf8))
            }
            return nil
        }

        process = runningProcess
        stdoutPipe = runningStdout
        stderrPipe = runningStderr
        self.baseURL = resolvedURL
    }

    deinit {
        stop()
    }

    func stop() {
        Self.terminateProcess(process)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func resetState() async throws {
        var request = URLRequest(url: baseURL.appendingPathComponent("__mock__/reset"))
        request.httpMethod = "POST"
        let session = URLSession(configuration: .ephemeral)
        let (_, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse,
              (200..<300).contains(http.statusCode)
        else {
            throw URLError(.badServerResponse)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func configurePipeline(scenario: String? = nil,
                           hash: String? = nil,
                           statusKinds: [String]? = nil,
                           repeatLast: Bool? = nil,
                           accepted: Bool? = nil,
                           submitStatus: Int? = nil) async throws {
        var payload: [String: Any] = [:]
        if let scenario { payload["scenario"] = scenario }
        if let hash { payload["hash"] = hash }
        if let statusKinds {
            payload["statuses"] = statusKinds.map { ["kind": $0] }
        }
        if let repeatLast { payload["repeat_last"] = repeatLast }
        if let accepted { payload["accepted"] = accepted }
        if let submitStatus { payload["submit_status"] = submitStatus }
        var request = URLRequest(url: baseURL.appendingPathComponent("__mock__/pipeline/config"))
        request.httpMethod = "POST"
        request.httpBody = try JSONSerialization.data(withJSONObject: payload, options: [])
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        let session = URLSession(configuration: .ephemeral)
        let (_, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse,
              (200..<300).contains(http.statusCode)
        else {
            throw URLError(.badServerResponse)
        }
    }

    private static func makeEnvironment() -> [String: String] {
        var env = ProcessInfo.processInfo.environment
        let repositoryRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // ToriiClientTests.swift
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // IrohaSwift
        let pythonPath = repositoryRoot.appendingPathComponent("python").path
        if let existing = env["PYTHONPATH"], !existing.isEmpty {
            env["PYTHONPATH"] = "\(pythonPath):\(existing)"
        } else {
            env["PYTHONPATH"] = pythonPath
        }
        env["PYTHONUNBUFFERED"] = "1"
        return env
    }

    fileprivate static func terminateProcess(_ process: Process, timeout: TimeInterval = 1.0) {
        guard process.isRunning else { return }
        process.terminate()
        if !waitForExit(process, timeout: timeout) {
            process.interrupt()
            _ = waitForExit(process, timeout: timeout)
        }
    }

    fileprivate static func waitForExit(_ process: Process, timeout: TimeInterval) -> Bool {
        if !process.isRunning { return true }
        let semaphore = DispatchSemaphore(value: 0)
        let previousHandler = process.terminationHandler
        process.terminationHandler = { terminated in
            previousHandler?(terminated)
            semaphore.signal()
        }
        if !process.isRunning {
            process.terminationHandler = previousHandler
            return true
        }
        let result = semaphore.wait(timeout: .now() + timeout)
        process.terminationHandler = previousHandler
        return result == .success
    }

    private static func readBaseURL(from pipe: Pipe, timeout: TimeInterval = 5.0) -> URL? {
        let handle = pipe.fileHandleForReading
        let semaphore = DispatchSemaphore(value: 0)
        let lock = NSLock()
        var data = Data()
        var didSignal = false

        // Avoid blocking reads if the mock never writes to stdout.
        handle.readabilityHandler = { fileHandle in
            let chunk = fileHandle.availableData
            lock.lock()
            if !chunk.isEmpty {
                data.append(chunk)
            }
            let hasNewline = data.contains(0x0A)
            if !didSignal && (hasNewline || chunk.isEmpty) {
                didSignal = true
                semaphore.signal()
            }
            lock.unlock()
            if hasNewline {
                fileHandle.readabilityHandler = nil
            }
        }

        _ = semaphore.wait(timeout: .now() + timeout)
        handle.readabilityHandler = nil

        lock.lock()
        let snapshot = data
        lock.unlock()

        guard let lineData = snapshot.split(separator: 0x0A, maxSplits: 1, omittingEmptySubsequences: true).first,
              let line = String(data: Data(lineData), encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
              let jsonData = line.data(using: .utf8),
              let decoded = try? JSONSerialization.jsonObject(with: jsonData) as? [String: Any],
              let urlString = decoded["base_url"] as? String,
              let url = URL(string: urlString)
        else {
            return nil
        }
        return url
    }
}

final class ToriiMockProcessTests: XCTestCase {
    func testTerminateProcessReturnsPromptly() throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sleep")
        process.arguments = ["1"]
        try process.run()
        let start = Date()
        ToriiMockProcess.terminateProcess(process, timeout: 0.05)
        let elapsed = Date().timeIntervalSince(start)
        XCTAssertLessThan(elapsed, 1.0)
        process.waitUntilExit()
    }
}

private final class StubGatewayFetcher: SorafsGatewayFetching, @unchecked Sendable {
    var capturedPlan: ToriiJSONValue?
    var capturedProviders: [SorafsGatewayProvider]?
    var capturedOptions: SorafsGatewayFetchOptions?
    var fetchCount = 0
    var result: SorafsGatewayFetchResult

    init(result: SorafsGatewayFetchResult) {
        self.result = result
    }

    func fetchGatewayPayload(
        plan: ToriiJSONValue,
        providers: [SorafsGatewayProvider],
        options: SorafsGatewayFetchOptions?,
        cancellationHandler: (() -> Void)?
    ) async throws -> SorafsGatewayFetchResult {
        fetchCount += 1
        capturedPlan = plan
        capturedProviders = providers
        capturedOptions = options
        return result
    }
}

private struct StubProofSummaryGenerator: DaProofSummaryGenerating, @unchecked Sendable {
    let summary: ToriiDaProofSummary

    func makeProofSummary(manifest: Data,
                          payload: Data,
                          options: ToriiDaProofSummaryOptions) throws -> ToriiDaProofSummary {
        summary
    }
}

private enum DaTestFixtures {
    static let manifestBytes = Data("swift-da-manifest".utf8)
    static let manifestHandle = "chunking.demo@1.0.0"
    static let storageTicketHex = String(repeating: "AB", count: 32)
    static let clientBlobHex = String(repeating: "CD", count: 32)
    static let blobHashHex = String(repeating: "EF", count: 32)
    static let chunkRootHex = String(repeating: "12", count: 32)
    static let manifestHashHex = String(repeating: "34", count: 32)

    private static let manifestDictionary: [String: Any] = [
        "chunker_handle": manifestHandle,
        "metadata": ["note": "swift-da-fixture"]
    ]

    private static let chunkPlanDictionary: [String: Any] = [
        "plan_id": "demo-plan",
        "chunks": 4
    ]

    static var storageTicketInput: String { "0x\(storageTicketHex.uppercased())" }

    static func manifestJSONValue() throws -> ToriiJSONValue {
        try jsonValue(from: manifestDictionary)
    }

    static func chunkPlanJSONValue() throws -> ToriiJSONValue {
        try jsonValue(from: chunkPlanDictionary)
    }

    static func responseBody() throws -> Data {
        let payload: [String: Any] = [
            "storage_ticket": storageTicketInput,
            "client_blob_id": "0x\(clientBlobHex.uppercased())",
            "blob_hash": "0x\(blobHashHex)",
            "manifest_hash": "0x\(manifestHashHex)",
            "chunk_root": "0x\(chunkRootHex)",
            "lane_id": 2,
            "epoch": 7,
            "manifest_len": manifestBytes.count,
            "manifest_norito": manifestBytes.base64EncodedString(),
            "manifest": manifestDictionary,
            "chunk_plan": chunkPlanDictionary
        ]
        return try JSONSerialization.data(withJSONObject: payload, options: [.sortedKeys])
    }

    static func manifestBundle() throws -> ToriiDaManifestBundle {
        try JSONDecoder().decode(ToriiDaManifestBundle.self, from: responseBody())
    }

    private static func jsonValue(from dictionary: [String: Any]) throws -> ToriiJSONValue {
        let data = try JSONSerialization.data(withJSONObject: dictionary, options: [.sortedKeys])
        return try JSONDecoder().decode(ToriiJSONValue.self, from: data)
    }
}

// Shared test helpers to keep Torii client DA fixtures deterministic across suites.
fileprivate func tcMakeClient() -> ToriiClient {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [StubURLProtocol.self]
    let session = URLSession(configuration: configuration)
    return ToriiClient(baseURL: URL(string: "https://example.test")!, session: session)
}

fileprivate func tcBodyJSON(from request: URLRequest) -> [String: Any] {
    var data: Data?
    if let direct = request.httpBody {
        data = direct
    } else if let stream = request.httpBodyStream {
        stream.open()
        defer { stream.close() }
        var buffer = [UInt8](repeating: 0, count: 1024)
        var collected = Data()
        while stream.hasBytesAvailable {
            let read = stream.read(&buffer, maxLength: buffer.count)
            if read <= 0 { break }
            collected.append(buffer, count: read)
        }
        data = collected.isEmpty ? nil : collected
    }
    guard
        let raw = data,
        let object = try? JSONSerialization.jsonObject(with: raw),
        let dictionary = object as? [String: Any]
    else { return [:] }
    return dictionary
}

fileprivate func tcMakeSampleManifestRaw(storageTicket: String = String(repeating: "aa", count: 32)) -> [String: ToriiJSONValue] {
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
        "chunk_plan": .array([
            .object([
                "chunk_index": .number(0),
                "offset": .number(0),
                "length": .number(4),
                "digest_blake3": .string(String(repeating: "ee", count: 32))
            ])
        ])
    ]
}

fileprivate func tcMakeSampleManifestBundle(storageTicket: String = String(repeating: "aa", count: 32)) throws -> ToriiDaManifestBundle {
    try ToriiDaManifestBundle(raw: tcMakeSampleManifestRaw(storageTicket: storageTicket))
}

fileprivate func tcMakeGatewayFetchResult() -> SorafsGatewayFetchResult {
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
fileprivate enum TcHelperError: Error {
    case invalidHashEncoding
}

fileprivate func tcMakePipelineEnvelope(hashHex: String, marker: UInt8) throws -> SignedTransactionEnvelope {
    guard let hashData = Data(hexString: hashHex) else {
        throw TcHelperError.invalidHashEncoding
    }
    let payload = Data([marker, marker ^ 0xFF, 0xA5])
    return SignedTransactionEnvelope(norito: payload,
                                     signedTransaction: payload,
                                     payload: nil,
                                     transactionHash: hashData)
}

fileprivate func tcLoadDaProofFixture() throws -> (manifest: Data, payload: Data, blobHashHex: String) {
    let fixtureRoot = tcRepositoryRootURL()
        .appendingPathComponent("fixtures/da/reconstruct/rs_parity_v1", isDirectory: true)
    let manifestHexURL = fixtureRoot.appendingPathComponent("manifest.norito.hex")
    let manifestJSONURL = fixtureRoot.appendingPathComponent("manifest.json")
    let payloadURL = fixtureRoot.appendingPathComponent("payload.bin")

    let manifestHex = try String(contentsOf: manifestHexURL, encoding: .utf8)
        .trimmingCharacters(in: .whitespacesAndNewlines)
    guard let manifestData = Data(hexString: manifestHex) else {
        throw XCTSkip("failed to decode DA manifest fixture")
    }
    let payloadData = try Data(contentsOf: payloadURL)
    let manifestJSONData = try Data(contentsOf: manifestJSONURL)
    guard
        let manifestObject = try JSONSerialization.jsonObject(with: manifestJSONData) as? [String: Any],
        let blobArray = manifestObject["blob_hash"] as? [[NSNumber]],
        let blobBytes = blobArray.first
    else {
        throw XCTSkip("blob_hash fixture missing")
    }
    let blobHex = blobBytes.reduce(into: "") { partialResult, value in
        partialResult.append(String(format: "%02x", value.uint8Value))
    }
    return (manifestData, payloadData, blobHex)
}

fileprivate func tcRepositoryRootURL() -> URL {
    URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent() // ToriiClientTests.swift
        .deletingLastPathComponent() // IrohaSwiftTests
        .deletingLastPathComponent() // Tests
        .deletingLastPathComponent() // IrohaSwift
}

fileprivate func tcMakeStubProofSummary() -> ToriiDaProofSummary {
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
        chunkRootsHex: [],
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

final class ToriiClientTests: XCTestCase {
    override func tearDown() {
        StubURLProtocol.handler = nil
        super.tearDown()
    }

    private func bodyData(from request: URLRequest) -> Data? {
        if let data = request.httpBody {
            return data
        }
        guard let stream = request.httpBodyStream else { return nil }
        stream.open()
        defer { stream.close() }
        var buffer = [UInt8](repeating: 0, count: 1024)
        var data = Data()
        while stream.hasBytesAvailable {
            let read = stream.read(&buffer, maxLength: buffer.count)
            if read <= 0 { break }
            data.append(buffer, count: read)
        }
        return data.isEmpty ? nil : data
    }

    private func bodyJSON(from request: URLRequest) -> [String: Any] {
        guard let data = bodyData(from: request),
              let object = try? JSONSerialization.jsonObject(with: data),
              let dictionary = object as? [String: Any] else {
            return [:]
        }
        return dictionary
    }

    private func makeClient() -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        return ToriiClient(baseURL: URL(string: "https://example.test")!, session: session)
    }

    private func canonicalOwnerLiteral(domain: String = "wonderland") throws -> String {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: 1, count: 32))
        let address = try AccountAddress.fromAccount(domain: domain, publicKey: keypair.publicKey)
        let ih58 = try address.toIH58(networkPrefix: 0x02F1)
        return "\(ih58)@\(domain)"
    }

    private func noncanonicalOwnerLiteral(domain: String = "wonderland") throws -> String {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: 2, count: 32))
        let address = try AccountAddress.fromAccount(domain: domain, publicKey: keypair.publicKey)
        let canonicalHex = try address.canonicalHex()
        return "\(canonicalHex)@\(domain)"
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsAsync() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/accounts/alice%40wonderland/assets")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset_id":"rose#wonderland##alice@wonderland","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(accountId: "alice@wonderland")
        XCTAssertEqual(balances.count, 1)
        XCTAssertEqual(balances.first?.asset_id, "rose#wonderland##alice@wonderland")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitTransactionAsync() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions")
            XCTAssertEqual(request.httpMethod, "POST")
            let response = HTTPURLResponse(url: request.url!, statusCode: 202, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"hash":"abc","accepted":true}
            """.data(using: .utf8)!
            return (response, body)
        }

        let payload = try await makeClient().submitTransaction(data: Data([0x00]))
        XCTAssertEqual(payload?.hash, "abc")
        XCTAssertEqual(payload?.accepted, true)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitTransactionRejectCodeHeaderSurfaced() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions")
            let headers = [
                "Content-Type": "application/json",
                "x-iroha-reject-code": "PRTRY:TX_SIGNATURE_MISSING"
            ]
            let response = HTTPURLResponse(url: request.url!, statusCode: 400, httpVersion: nil, headerFields: headers)!
            let body = """
            {"message":"failed to accept transaction"}
            """.data(using: .utf8)!
            return (response, body)
        }

        do {
            _ = try await makeClient().submitTransaction(data: Data([0x01]))
            XCTFail("expected rejection")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, message, rejectCode) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(code, 400)
            XCTAssertEqual(rejectCode, "PRTRY:TX_SIGNATURE_MISSING")
            XCTAssertEqual(message, HTTPURLResponse.localizedString(forStatusCode: 400))
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetConnectStatusParsesSnapshot() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/connect/status")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            let payload: [String: Any] = [
                "enabled": true,
                "sessions_total": 10,
                "sessions_active": 7,
                "per_ip_sessions": [
                    ["ip": "1.1.1.1", "sessions": 3]
                ],
                "buffered_sessions": 2,
                "total_buffer_bytes": 128,
                "dedupe_size": 4,
                "policy": ["ws_max_sessions": 50, "session_ttl_ms": 60000, "relay_enabled": true],
                "frames_in_total": 11,
                "frames_out_total": 12,
                "ciphertext_total": 13,
                "dedupe_drops_total": 1,
                "buffer_drops_total": 2,
                "plaintext_control_drops_total": 3,
                "monotonic_drops_total": 4,
                "ping_miss_total": 5
            ]
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let data = try JSONSerialization.data(withJSONObject: payload)
            return (response, data)
        }
        let snapshot = try await makeClient().getConnectStatus()
        let status = try XCTUnwrap(snapshot)
        XCTAssertTrue(status.enabled)
        XCTAssertEqual(status.sessionsTotal, 10)
        XCTAssertEqual(status.perIpSessions.first?.ip, "1.1.1.1")
        XCTAssertEqual(status.policy?.wsMaxSessions, 50)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetConnectStatusReturnsNilFor404() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/connect/status")
            let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: [:])!
            return (response, Data())
        }
        let snapshot = try await makeClient().getConnectStatus()
        XCTAssertNil(snapshot)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testCreateConnectSessionPostsPayload() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/connect/session")
            XCTAssertEqual(request.httpMethod, "POST")
            let body = self.bodyJSON(from: request)
            XCTAssertEqual(body["sid"] as? String, "abc")
            XCTAssertEqual(body["node"] as? String, "node-1")
            let payload: [String: Any] = [
                "sid": "abc",
                "wallet_uri": "wallet://demo",
                "app_uri": "app://demo",
                "token_app": "token-app",
                "token_wallet": "token-wallet",
                "custom": true
            ]
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let data = try JSONSerialization.data(withJSONObject: payload)
            return (response, data)
        }
        let response = try await makeClient().createConnectSession(sid: " abc ", node: "node-1")
        XCTAssertEqual(response.sid, "abc")
        XCTAssertEqual(response.tokenWallet, "token-wallet")
        XCTAssertEqual(response.extra["custom"], .bool(true))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testDeleteConnectSessionHandles404() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/connect/session/sid-1")
            let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: [:])!
            return (response, Data())
        }
        let deleted = try await makeClient().deleteConnectSession(sid: "sid-1")
        XCTAssertFalse(deleted)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListConnectAppsParsesPage() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/connect/app/apps")
            let components = try XCTUnwrap(URLComponents(url: request.url!, resolvingAgainstBaseURL: false))
            XCTAssertTrue(try XCTUnwrap(components.queryItems).contains { $0.name == "cursor" && $0.value == "cursor-1" })
            XCTAssertTrue(try XCTUnwrap(components.queryItems).contains { $0.name == "limit" && $0.value == "5" })
            let payload: [String: Any] = [
                "items": [
                    [
                        "app_id": "demo-app",
                        "display_name": "Demo",
                        "description": "desc",
                        "icon_url": "https://example.test/icon.png",
                        "namespaces": ["sora"],
                        "metadata": ["a": 1],
                        "policy": ["relay_enabled": true],
                        "custom": "ok"
                    ]
                ],
                "total": 1,
                "next_cursor": "cursor-2",
                "page_note": "ok"
            ]
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let data = try JSONSerialization.data(withJSONObject: payload)
            return (response, data)
        }
        let page = try await makeClient().listConnectApps(options: ToriiConnectAppListOptions(limit: 5, cursor: "cursor-1"))
        XCTAssertEqual(page.items.first?.appId, "demo-app")
        XCTAssertEqual(page.nextCursor, "cursor-2")
        XCTAssertEqual(page.extra["page_note"], .string("ok"))
        XCTAssertEqual(page.items.first?.extra["custom"], .string("ok"))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetConnectAppParsesRecord() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/connect/app/apps/demo-app")
            let payload: [String: Any] = [
                "app_id": "demo-app",
                "namespaces": ["sora"],
                "metadata": ["k": "v"],
                "policy": [:]
            ]
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let data = try JSONSerialization.data(withJSONObject: payload)
            return (response, data)
        }
        let record = try await makeClient().getConnectApp(appId: "demo-app")
        XCTAssertEqual(record.appId, "demo-app")
        XCTAssertEqual(record.namespaces, ["sora"])
        XCTAssertEqual(record.metadata["k"], .string("v"))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRegisterConnectAppAllowsEmptyResponse() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/connect/app/apps")
            XCTAssertEqual(request.httpMethod, "POST")
            let response = HTTPURLResponse(url: request.url!, statusCode: 202, httpVersion: nil, headerFields: [:])!
            return (response, Data())
        }
        let input = ToriiConnectAppUpsertInput(appId: "demo", namespaces: ["sora"])
        let record = try await makeClient().registerConnectApp(input)
        XCTAssertNil(record)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testDeleteConnectAppReturnsTrueOn204() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/connect/app/apps/demo")
            let response = HTTPURLResponse(url: request.url!, statusCode: 204, httpVersion: nil, headerFields: [:])!
            return (response, Data())
        }
        let deleted = try await makeClient().deleteConnectApp(appId: "demo")
        XCTAssertTrue(deleted)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetConnectAppPolicyParsesPayload() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/connect/app/policy")
            let payload: [String: Any] = [
                "relay_enabled": true,
                "ws_max_sessions": 10,
                "ws_per_ip_max_sessions": 5,
                "ws_rate_per_ip_per_min": 50,
                "session_ttl_ms": 1000,
                "frame_max_bytes": 4096,
                "session_buffer_max_bytes": 8192,
                "ping_interval_ms": 200,
                "ping_miss_tolerance": 2,
                "ping_min_interval_ms": 100,
                "note": "ok"
            ]
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let data = try JSONSerialization.data(withJSONObject: payload)
            return (response, data)
        }
        let policy = try await makeClient().getConnectAppPolicy()
        XCTAssertEqual(policy.wsMaxSessions, 10)
        XCTAssertEqual(policy.extra["note"], .string("ok"))
        XCTAssertEqual(policy.relayEnabled, true)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testUpdateConnectAppPolicyPostsPayload() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/connect/app/policy")
            XCTAssertEqual(request.httpMethod, "POST")
            let body = self.bodyJSON(from: request)
            XCTAssertEqual(body["ws_max_sessions"] as? Int, 20)
            XCTAssertEqual(body["relay_enabled"] as? Bool, false)
            let payload: [String: Any] = [
                "ws_max_sessions": 20,
                "relay_enabled": false
            ]
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let data = try JSONSerialization.data(withJSONObject: payload)
            return (response, data)
        }
        let update = ToriiConnectAppPolicyUpdate(relayEnabled: false, wsMaxSessions: 20)
        let policy = try await makeClient().updateConnectAppPolicy(update)
        XCTAssertEqual(policy.wsMaxSessions, 20)
        XCTAssertEqual(policy.relayEnabled, false)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testConnectAdmissionManifestGetAndSet() async throws {
        var callCount = 0
        StubURLProtocol.handler = { request in
            callCount += 1
            if request.httpMethod == "PUT" {
                XCTAssertEqual(request.url?.path, "/v1/connect/app/manifest")
                let body = self.bodyJSON(from: request)
                XCTAssertEqual(body["version"] as? Int, 3)
                XCTAssertEqual((body["entries"] as? [[String: Any]])?.first?["app_id"] as? String, "demo")
                let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let payload: [String: Any] = [
                    "entries": [
                        [
                            "app_id": "demo",
                            "namespaces": ["sora"],
                            "metadata": [:],
                            "policy": [:]
                        ]
                    ]
                ]
                let data = try JSONSerialization.data(withJSONObject: payload)
                return (response, data)
            } else {
                XCTAssertEqual(request.url?.path, "/v1/connect/app/manifest")
                let payload: [String: Any] = [
                    "version": 2,
                    "manifest_hash": "abcd",
                    "updated_at": "ts",
                    "entries": [
                        [
                            "app_id": "demo",
                            "namespaces": ["sora", "nexus"],
                            "metadata": ["k": "v"],
                            "policy": ["p": 1],
                            "extra_field": true
                        ]
                    ],
                    "note": "ok"
                ]
                let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let data = try JSONSerialization.data(withJSONObject: payload)
                return (response, data)
            }
        }
        let manifest = try await makeClient().getConnectAdmissionManifest()
        XCTAssertEqual(manifest.entries.first?.appId, "demo")
        XCTAssertEqual(manifest.entries.first?.extra["extra_field"], .bool(true))
        let entry = try ToriiConnectAdmissionManifestEntry(appId: "demo", namespaces: ["sora"])
        let updated = try await makeClient().setConnectAdmissionManifest(ToriiConnectAdmissionManifestInput(version: 3, entries: [entry]))
        XCTAssertEqual(updated.entries.count, 1)
        XCTAssertEqual(callCount, 2)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetDaManifestBundleFetchesManifest() async throws {
        let expectation = expectation(description: "manifest request")
        let ticket = String(repeating: "ab", count: 32)
        let manifestB64 = Data("manifest-data".utf8).base64EncodedString()
        let samplingPlan = """
        {"assignment_hash":"\(String(repeating: "bb", count: 32))","sample_window":4,"samples":[{"index":2,"role":"global_parity","group":1}]}
        """
        let blockHash = String(repeating: "cc", count: 32)
        let body = """
        {
            "storage_ticket":"\(ticket)",
            "client_blob_id":"\(String(repeating: "cd", count: 32))",
            "blob_hash":"\(String(repeating: "ef", count: 32))",
            "manifest_hash":"\(String(repeating: "99", count: 32))",
            "chunk_root":"\(String(repeating: "11", count: 32))",
            "lane_id":7,
            "epoch":11,
            "manifest_len":\(manifestB64.count),
            "manifest_norito":"\(manifestB64)",
            "manifest":{"chunking":{"namespace":"sorafs","name":"sf1","semver":"1.2.3"}},
            "chunk_plan":[{"chunk_index":0,"offset":0,"length":4,"digest_blake3":"\(String(repeating: "22", count: 32))"}],
            "sampling_plan":\(samplingPlan)
        }
        """
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/da/manifests/\(ticket)")
            XCTAssertEqual(request.url?.query, "block_hash=\(blockHash)")
            expectation.fulfill()
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, body.data(using: .utf8))
        }

        let bundle = try await makeClient().getDaManifestBundle(storageTicketHex: "0x\(ticket)", blockHashHex: blockHash)
        await fulfillment(of: [expectation], timeout: 1.0)
        XCTAssertEqual(bundle.storageTicketHex, ticket)
        XCTAssertEqual(bundle.blobHashHex, String(repeating: "ef", count: 32))
        XCTAssertEqual(bundle.manifestBytes, Data("manifest-data".utf8))
        XCTAssertEqual(bundle.laneId, 7)
        XCTAssertEqual(bundle.samplingPlan?.assignmentHashHex, String(repeating: "bb", count: 32))
        XCTAssertEqual(bundle.samplingPlan?.sampleWindow, 4)
        XCTAssertEqual(bundle.samplingPlan?.samples.first?.index, 2)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testPersistDaManifestBundleWritesSamplingPlan() async throws {
        let ticket = String(repeating: "aa", count: 32)
        var raw = tcMakeSampleManifestRaw(storageTicket: ticket)
        raw["sampling_plan"] = .object([
            "assignment_hash": .string(String(repeating: "bb", count: 32)),
            "sample_window": .number(3),
            "samples": .array([
                .object([
                    "index": .number(1),
                    "role": .string("data"),
                    "group": .number(0)
                ])
            ])
        ])
        let bundle = try ToriiDaManifestBundle(raw: raw)
        let tmp = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let paths = try ToriiClient.persistDaManifestBundle(bundle,
                                                            outputDir: tmp,
                                                            label: nil,
                                                            fileManager: .default)
        let samplingPath = paths.samplingPlanURL
        XCTAssertNotNil(samplingPath)
        let data = try Data(contentsOf: samplingPath!)
        let json = try JSONSerialization.jsonObject(with: data, options: []) as? [String: Any]
        guard let window = json?["sample_window"] as? NSNumber else {
            return XCTFail("missing sample_window in persisted sampling plan")
        }
        XCTAssertEqual(window.intValue, 3)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testFetchDaPayloadViaGatewayUsesProvidedManifest() async throws {
        let bundle = try tcMakeSampleManifestBundle()
        let provider = try SorafsGatewayProvider(
            name: "alpha",
            providerIdHex: String(repeating: "01", count: 32),
            baseURL: URL(string: "https://gateway.test")!,
            streamTokenB64: Data("token".utf8).base64EncodedString()
        )
        let fetcher = StubGatewayFetcher(result: tcMakeGatewayFetchResult())
        let session = try await tcMakeClient().fetchDaPayloadViaGateway(
            manifestBundle: bundle,
            providers: [provider],
            orchestrator: fetcher
        )
        XCTAssertEqual(session.manifest.storageTicketHex, bundle.storageTicketHex)
        XCTAssertEqual(session.chunkerHandle, "sorafs.sf1@1.0.0")
        XCTAssertEqual(fetcher.fetchCount, 1)
        XCTAssertEqual(fetcher.capturedPlan, bundle.chunkPlan)
        XCTAssertEqual(fetcher.capturedProviders?.first?.providerIdHex, provider.providerIdHex)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testFetchDaPayloadViaGatewayDownloadsManifestWhenTicketProvided() async throws {
        let expectation = expectation(description: "manifest fetch")
        let ticket = String(repeating: "aa", count: 32)
        let raw = tcMakeSampleManifestRaw(storageTicket: ticket)
        let json = try ToriiJSONValue.object(raw).encodedData()
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/da/manifests/\(ticket)")
            expectation.fulfill()
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, json)
        }
        let provider = try SorafsGatewayProvider(
            name: "beta",
            providerIdHex: String(repeating: "02", count: 32),
            baseURL: URL(string: "https://gateway.test")!,
            streamTokenB64: Data("stream".utf8).base64EncodedString()
        )
        let fetcher = StubGatewayFetcher(result: tcMakeGatewayFetchResult())
        let session = try await tcMakeClient().fetchDaPayloadViaGateway(
            storageTicketHex: "0x\(ticket)",
            providers: [provider],
            orchestrator: fetcher
        )
        await fulfillment(of: [expectation], timeout: 1.0)
        XCTAssertEqual(session.manifest.storageTicketHex, ticket)
        XCTAssertEqual(fetcher.fetchCount, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testFetchDaPayloadViaGatewayRequiresInputs() async throws {
        let provider = try SorafsGatewayProvider(
            name: "gamma",
            providerIdHex: String(repeating: "03", count: 32),
            baseURL: URL(string: "https://gateway.test")!,
            streamTokenB64: Data("stream".utf8).base64EncodedString()
        )
        do {
            _ = try await makeClient().fetchDaPayloadViaGateway(
                providers: [provider],
                orchestrator: StubGatewayFetcher(result: tcMakeGatewayFetchResult())
            )
            XCTFail("Expected invalidPayload error")
        } catch {
            guard case ToriiClientError.invalidPayload = error else {
                XCTFail("Expected invalidPayload error, got \(error)")
                return
            }
        }
    }

    func testNativeDaProofSummaryGeneratorEmitsExplicitProofs() throws {
        #if !canImport(Darwin)
        throw XCTSkip("Norito bridge unavailable on this platform")
        #else
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("Norito bridge unavailable on this platform")
        }
        let fixture = try tcLoadDaProofFixture()
        let options = ToriiDaProofSummaryOptions(sampleCount: 0, sampleSeed: 42, leafIndexes: [0, 1, 1])
        let summary: ToriiDaProofSummary
        do {
            summary = try NativeDaProofSummaryGenerator.shared.makeProofSummary(
                manifest: fixture.manifest,
                payload: fixture.payload,
                options: options
            )
        } catch ToriiClientError.invalidPayload {
            throw XCTSkip("Native DA proof summary generator unavailable in this environment")
        }
        XCTAssertEqual(summary.blobHashHex.lowercased(), fixture.blobHashHex.lowercased())
        XCTAssertEqual(summary.sampleCount, 0)
        XCTAssertEqual(summary.proofCount, 2)
        XCTAssertEqual(summary.proofs.count, 2)
        XCTAssertTrue(summary.proofs.allSatisfy { $0.origin == "explicit" })
        XCTAssertEqual(summary.proofs.first?.leafIndex, 0)
        #endif
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testFetchDaPayloadViaGatewayAttachesProofSummary() async throws {
        let bundle = try tcMakeSampleManifestBundle()
        let provider = try SorafsGatewayProvider(
            name: "delta",
            providerIdHex: String(repeating: "04", count: 32),
            baseURL: URL(string: "https://gateway.test")!,
            streamTokenB64: Data("token".utf8).base64EncodedString()
        )
        let fetcher = StubGatewayFetcher(result: tcMakeGatewayFetchResult())
        let stubSummary = tcMakeStubProofSummary()
        let generator = StubProofSummaryGenerator(summary: stubSummary)
        let session = try await tcMakeClient().fetchDaPayloadViaGateway(
            manifestBundle: bundle,
            providers: [provider],
            proofSummaryOptions: ToriiDaProofSummaryOptions(sampleCount: 0, leafIndexes: [0]),
            orchestrator: fetcher,
            proofSummaryGenerator: generator
        )
        XCTAssertNotNil(session.proofSummary)
        XCTAssertEqual(session.proofSummary?.proofCount, stubSummary.proofCount)
        XCTAssertEqual(session.proofSummary?.proofs.first?.origin, stubSummary.proofs.first?.origin)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testIrohaSDKGetAssetsAsyncUsesREST() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/accounts/alice%40wonderland/assets")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset_id":"rose#wonderland##alice@wonderland","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!, session: session)

        let balances = try await sdk.getAssets(accountId: "alice@wonderland")
        XCTAssertEqual(balances.count, 1)
        XCTAssertEqual(balances.first?.asset_id, "rose#wonderland##alice@wonderland")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsTrimsAndEncodesAccountLiteral() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/accounts/alice%40wonderland/assets")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset_id":"rose#wonderland##alice@wonderland","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(accountId: "  alice@wonderland  ")
        XCTAssertEqual(balances.count, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsEncodesPercentInAccountLiteral() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/accounts/alice%252Fbob%40wonderland/assets")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset_id":"rose#wonderland##alice@wonderland","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(accountId: "alice%2Fbob@wonderland")
        XCTAssertEqual(balances.count, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionsEncodesAccountLiteral() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/accounts/alice%40wonderland/transactions")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"items":[{"entrypoint_hash":"hash","authority":"alice@wonderland","timestamp_ms":1,"result_ok":true}],"total":1}
            """.data(using: .utf8)!
            return (response, body)
        }

        let transactions = try await makeClient().getTransactions(accountId: "alice@wonderland")
        XCTAssertEqual(transactions.total, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerAccountQrDecodesResponse() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/accounts/alice%40wonderland/qr")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            XCTAssertNil(components?.queryItems)
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "canonical_id":"alice@wonderland",
                "literal":"alice@wonderland",
                "address_format":"ih58",
                "network_prefix":0,
                "error_correction":"M",
                "modules":192,
                "qr_version":5,
                "svg":"<svg viewBox=\\"0 0 192 192\\"></svg>"
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let qr = try await makeClient().getExplorerAccountQr(accountId: "alice@wonderland")
        XCTAssertEqual(qr.canonicalId, "alice@wonderland")
        XCTAssertEqual(qr.literal, "alice@wonderland")
        XCTAssertEqual(qr.addressFormat, "ih58")
        XCTAssertEqual(qr.preferredFormat, .ih58)
        XCTAssertEqual(qr.networkPrefix, 0)
        XCTAssertEqual(qr.modules, 192)
        XCTAssertEqual(qr.qrVersion, 5)
        XCTAssertTrue(qr.svg.contains("<svg"))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerAccountQrSupportsAddressFormatPreference() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/accounts/alice%40wonderland/qr")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            XCTAssertEqual(components?.queryItems?.first?.name, "address_format")
            XCTAssertEqual(components?.queryItems?.first?.value, "compressed")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "canonical_id":"alice@wonderland",
                "literal":"snx1example@wonderland",
                "address_format":"compressed",
                "network_prefix":1206,
                "error_correction":"M",
                "modules":192,
                "qr_version":6,
                "svg":"<svg class=\\"qr\\"></svg>"
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let qr = try await makeClient().getExplorerAccountQr(accountId: "alice@wonderland",
                                                             addressFormat: .compressed)
        XCTAssertEqual(qr.addressFormat, "compressed")
        XCTAssertEqual(qr.preferredFormat, .compressed)
        XCTAssertEqual(qr.literal, "snx1example@wonderland")
        XCTAssertEqual(qr.qrVersion, 6)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListDomainsEncodesOptions() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/domains")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let query = Dictionary(uniqueKeysWithValues: (components?.queryItems ?? []).map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["limit"], "25")
            XCTAssertEqual(query["offset"], "10")
            XCTAssertEqual(query["sort"], "name,-created_at")
            let filterValue = try XCTUnwrap(query["filter"])
            let filterData = try XCTUnwrap(filterValue.data(using: .utf8))
            let decodedFilter = try JSONSerialization.jsonObject(with: filterData) as? [String: String]
            XCTAssertEqual(decodedFilter?["id"], "wonderland")
            let body = """
            {
                "items": [
                    {"id":"wonderland","owned_by":"alice@wonderland","metadata":{"theme":"demo"}}
                ],
                "total": 1
            }
            """.data(using: .utf8)!
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, body)
        }

        let options = ToriiListOptions(
            filter: .json(.object(["id": .string("wonderland")])),
            sort: .fields(["name", "-created_at"]),
            limit: 25,
            offset: 10
        )
        let page = try await makeClient().listDomains(options: options)
        XCTAssertEqual(page.total, 1)
        let record = try XCTUnwrap(page.items.first)
        XCTAssertEqual(record.id, "wonderland")
        XCTAssertEqual(record.ownedBy, "alice@wonderland")
        if case let .string(theme)? = record.metadata["theme"] {
            XCTAssertEqual(theme, "demo")
        } else {
            XCTFail("expected metadata value")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testIterateDomainsRespectsPagingAndMaxItems() async throws {
        var observedLimits: [String] = []
        var observedOffsets: [String] = []
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/domains")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let queryItems = components?.queryItems ?? []
            let dictionary = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
            if let limitValue = dictionary["limit"] {
                observedLimits.append(limitValue)
            }
            if let offsetValue = dictionary["offset"] {
                observedOffsets.append(offsetValue)
            }
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body: Data
            switch dictionary["offset"] ?? "0" {
            case "0":
                body = """
                {"items":[
                    {"id":"domain-1","metadata":{}},
                    {"id":"domain-2","metadata":{}}
                ],"total":4}
                """.data(using: .utf8)!
            case "2":
                body = """
                {"items":[{"id":"domain-3","metadata":{}}],"total":4}
                """.data(using: .utf8)!
            default:
                body = #"{"items":[],"total":4}"#.data(using: .utf8)!
            }
            return (response, body)
        }

        let stream = makeClient().iterateDomains(pageSize: 2, maxItems: 3)
        var collected: [String] = []
        for try await record in stream {
            collected.append(record.id)
        }
        XCTAssertEqual(collected, ["domain-1", "domain-2", "domain-3"])
        XCTAssertEqual(observedLimits, ["2", "1"])
        XCTAssertEqual(observedOffsets, ["0", "2"])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetUaidPortfolioNormalizesLiteral() async throws {
        let uaidHex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        let payload = """
        {
          "uaid":"uaid:\(uaidHex)",
          "totals":{"accounts":2,"positions":3},
          "dataspaces":[
            {
              "dataspace_id":0,
              "dataspace_alias":"universal",
              "accounts":[
                {
                  "account_id":"holder@global",
                  "label":null,
                  "assets":[{"asset_id":"cash#global::holder@global","asset_definition_id":"cash#global","quantity":"500"}]
                }
              ]
            }
          ]
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/accounts/uaid%3A\(uaidHex)/portfolio")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let response = try await makeClient().getUaidPortfolio(uaid: "  UAID:\(uaidHex.uppercased())  ")
        XCTAssertEqual(response.uaid, "uaid:\(uaidHex)")
        XCTAssertEqual(response.totals.accounts, 2)
        XCTAssertEqual(response.dataspaces.first?.accounts.first?.assets.first?.quantity, "500")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetUaidBindingsReturnsDataspaces() async throws {
        let uaidHex = "abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd"
        let payload = """
        {
          "uaid":"uaid:\(uaidHex)",
          "dataspaces":[
            {"dataspace_id":0,"dataspace_alias":"universal","accounts":["holder@global"]},
            {"dataspace_id":11,"dataspace_alias":"cbdc","accounts":[]}
          ]
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/space-directory/uaids/uaid%3A\(uaidHex)")
            XCTAssertEqual(request.url?.query, "address_format=compressed")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let response = try await makeClient().getUaidBindings(
            uaid: uaidHex,
            query: ToriiUaidBindingsQuery(addressFormat: "compressed")
        )
        XCTAssertEqual(response.dataspaces.count, 2)
        XCTAssertEqual(response.dataspaces.first?.accounts.first, "holder@global")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetUaidManifestsAppliesQueryItems() async throws {
        let uaidHex = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543211"
        let payload = """
        {
          "uaid":"uaid:\(uaidHex)",
          "total":1,
          "manifests":[
            {
              "dataspace_id":11,
              "dataspace_alias":"cbdc",
              "manifest_hash":"00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
              "status":"Active",
              "lifecycle":{"activated_epoch":4096,"expired_epoch":null,"revocation":null},
              "accounts":["holder@cbdc"],
              "manifest":{
                "version":"V1",
                "uaid":"uaid:\(uaidHex)",
                "dataspace":11,
                "issued_ms":100,
                "activation_epoch":200,
                "expiry_epoch":null,
                "entries":[{"scope":{"program":"cbdc.transfer"},"effect":{"Allow":{"max_amount":"500","window":"PerDay"}}}]
              }
            }
          ]
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/space-directory/uaids/uaid%3A\(uaidHex)/manifests")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let items = Dictionary(uniqueKeysWithValues: (components?.queryItems ?? []).map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(items["dataspace"], "11")
            XCTAssertEqual(items["status"], "inactive")
            XCTAssertEqual(items["limit"], "2")
            XCTAssertEqual(items["offset"], "1")
            XCTAssertEqual(items["address_format"], "compressed")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let query = ToriiUaidManifestQuery(
            dataspaceId: 11,
            status: .inactive,
            limit: 2,
            offset: 1,
            addressFormat: "compressed"
        )
        let response = try await makeClient().getUaidManifests(uaid: "uaid:\(uaidHex)", query: query)
        XCTAssertEqual(response.total, 1)
        XCTAssertEqual(response.manifests.first?.status, .active)
        XCTAssertEqual(response.manifests.first?.accounts.first, "holder@cbdc")
    }

    func testUaidBindingsQueryRejectsAddressFormatAlias() {
        let query = ToriiUaidBindingsQuery(addressFormat: "IH-b32")
        XCTAssertThrowsError(try query.queryItems()) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
        }
    }

    func testUaidBindingsQueryRejectsInvalidAddressFormat() {
        let query = ToriiUaidBindingsQuery(addressFormat: "public-key")
        XCTAssertThrowsError(try query.queryItems()) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
        }
    }

    func testUaidManifestQueryRejectsAddressFormatAlias() {
        let query = ToriiUaidManifestQuery(addressFormat: "  canonical ")
        XCTAssertThrowsError(try query.queryItems()) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
        }
    }

    func testUaidManifestQueryRejectsInvalidAddressFormat() {
        let query = ToriiUaidManifestQuery(addressFormat: "canonical_hex")
        XCTAssertThrowsError(try query.queryItems()) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetUaidPortfolioRejectsInvalidLiteral() async {
        do {
            _ = try await makeClient().getUaidPortfolio(uaid: "bad")
            XCTFail("Expected invalid UAID error")
        } catch {
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetUaidPortfolioRejectsInvalidLsb() async {
        let uaidHex = String(repeating: "10", count: 32)
        do {
            _ = try await makeClient().getUaidPortfolio(uaid: "uaid:\(uaidHex)")
            XCTFail("Expected invalid UAID error")
        } catch {
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testIrohaSDKGetTransactionStatusAsyncUsesREST() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            XCTAssertEqual(components?.queryItems?.first?.value, "deadbeef")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"kind":"Transaction","content":{"hash":"deadbeef","status":{"kind":"Committed","content":null}}}
            """.data(using: .utf8)!
            return (response, body)
        }

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!, session: session)

        let status = try await sdk.getTransactionStatus(hashHex: "deadbeef")
        XCTAssertEqual(status?.kind, "Transaction")
        XCTAssertEqual(status?.content.status.kind, "Committed")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetPipelineRecoveryAsync() async throws {
        let payload = """
        {"format":"pipeline.recovery.v1","height":42,"dag":{"fingerprint":"abcdef","key_count":1},"txs":[{"hash":"0x01","reads":["account/alice@wonderland"],"writes":["asset/rose#wonderland"]}]}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/recovery/42")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let recovery = try await makeClient().getPipelineRecovery(height: 42)
        XCTAssertEqual(recovery?.format, "pipeline.recovery.v1")
        XCTAssertEqual(recovery?.height, 42)
        XCTAssertEqual(recovery?.dag.fingerprint, "abcdef")
        XCTAssertEqual(recovery?.txs.first?.hash, "0x01")
    }

    func testGetPipelineRecoveryReturnsNilOn404() {
        let expectation = expectation(description: "recovery")
        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!
            return (response, nil)
        }

        makeClient().getPipelineRecovery(height: 99) { result in
            switch result {
            case .success(let recovery):
                XCTAssertNil(recovery)
            case .failure(let error):
                XCTFail("Unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testIrohaSDKGetPipelineRecoveryAsyncUsesREST() async throws {
        let payload = """
        {"format":"pipeline.recovery.v1","height":7,"dag":{"fingerprint":"cafebabe","key_count":2},"txs":[]}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/recovery/7")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!, session: session)

        let recovery = try await sdk.getPipelineRecovery(height: 7)
        XCTAssertEqual(recovery?.dag.fingerprint, "cafebabe")
        XCTAssertEqual(recovery?.height, 7)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testIrohaSDKGetTimeNowAsync() async throws {
        let payload = """
        {"now":42,"offset_ms":0,"confidence_ms":1}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/time/now")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!, session: session)

        let snapshot = try await sdk.getTimeNow()
        XCTAssertEqual(snapshot.now, 42)
        XCTAssertEqual(snapshot.confidence_ms, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTimeNowAsync() async throws {
        let payload = """
        {"now":1700000000123,"offset_ms":5,"confidence_ms":2}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/time/now")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let snapshot = try await makeClient().getTimeNow()
        XCTAssertEqual(snapshot.now, 1_700_000_000_123)
        XCTAssertEqual(snapshot.offset_ms, 5)
        XCTAssertEqual(snapshot.confidence_ms, 2)
    }

    func testGetTimeNowCompletion() {
        let expectation = expectation(description: "time-now")
        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"now":10,"offset_ms":-1,"confidence_ms":0}
            """.data(using: .utf8)!
            return (response, body)
        }

        makeClient().getTimeNow { result in
            switch result {
            case .success(let snapshot):
                XCTAssertEqual(snapshot.now, 10)
                XCTAssertEqual(snapshot.offset_ms, -1)
            case .failure(let error):
                XCTFail("Unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetNodeCapabilitiesAsync() async throws {
        let payload = """
        {"supported_abi_versions":[1,2,3],"default_compile_target":2}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/node/capabilities")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let capabilities = try await makeClient().getNodeCapabilities()
        XCTAssertEqual(capabilities.supportedAbiVersions, [1, 2, 3])
        XCTAssertEqual(capabilities.defaultCompileTarget, 2)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetConfigurationParsesConfidentialGas() async throws {
        let payload = """
        {
          "public_key":"ed0123",
          "logger":{"level":"Info","filter":null},
          "network":{
            "block_gossip_size":32,
            "block_gossip_period_ms":150,
            "transaction_gossip_size":16,
            "transaction_gossip_period_ms":75
          },
          "queue":{"capacity":2048},
          "confidential_gas":{
            "proof_base":10,
            "per_public_input":2,
            "per_proof_byte":3,
            "per_nullifier":4,
            "per_commitment":5
          },
          "nexus": {
            "axt": {
              "slot_length_ms": 1000,
              "max_clock_skew_ms": 250,
              "proof_cache_ttl_slots": 3,
              "replay_retention_slots": 64
            }
          }
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/configuration")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let snapshot = try await makeClient().getConfiguration()
        XCTAssertEqual(snapshot.publicKeyHex, "ed0123")
        XCTAssertEqual(snapshot.logger.level, "Info")
        XCTAssertNil(snapshot.logger.filter)
        XCTAssertEqual(snapshot.network.blockGossipSize, 32)
        XCTAssertEqual(snapshot.network.transactionGossipPeriodMs, 75)
        XCTAssertEqual(snapshot.queue?.capacity, 2048)
        let gas = try XCTUnwrap(snapshot.confidentialGas)
        XCTAssertEqual(gas.proofBase, 10)
        XCTAssertEqual(gas.perPublicInput, 2)
        XCTAssertEqual(gas.perProofByte, 3)
        XCTAssertEqual(gas.perNullifier, 4)
        XCTAssertEqual(gas.perCommitment, 5)
        let axt = try XCTUnwrap(snapshot.nexus?.axt)
        XCTAssertEqual(axt.slotLengthMs, 1_000)
        XCTAssertEqual(axt.maxClockSkewMs, 250)
        XCTAssertEqual(axt.proofCacheTtlSlots, 3)
        XCTAssertEqual(axt.replayRetentionSlots, 64)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetConfidentialGasScheduleNilWhenMissing() async throws {
        let payload = """
        {
          "public_key":"ed0123",
          "logger":{"level":"Info","filter":null},
          "network":{
            "block_gossip_size":32,
            "block_gossip_period_ms":150,
            "transaction_gossip_size":16,
            "transaction_gossip_period_ms":75
          },
          "queue":{"capacity":2048}
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/configuration")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let schedule = try await makeClient().getConfidentialGasSchedule()
        XCTAssertNil(schedule)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetConfidentialAssetPolicyAsync() async throws {
        let payload = """
        {
          "asset_id":"rose#wonderland",
          "block_height":1024,
          "current_mode":"Convertible",
          "effective_mode":"Convertible",
          "vk_set_hash":"0123ABCD",
          "poseidon_params_id":7,
          "pedersen_params_id":11,
          "pending_transition":{
            "transition_id":"DEADBEEF",
            "previous_mode":"Convertible",
            "new_mode":"ShieldedOnly",
            "effective_height":2048,
            "conversion_window":200,
            "window_open_height":1848
          }
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/confidential/assets/rose%23wonderland/transitions")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let policy = try await makeClient().getConfidentialAssetPolicy(assetDefinitionId: "  rose#wonderland  ")
        XCTAssertEqual(policy.assetId, "rose#wonderland")
        XCTAssertEqual(policy.blockHeight, 1024)
        XCTAssertEqual(policy.currentMode, "Convertible")
        XCTAssertEqual(policy.effectiveMode, "Convertible")
        XCTAssertEqual(policy.vkSetHashHex, "0123ABCD")
        XCTAssertEqual(policy.poseidonParamsId, 7)
        XCTAssertEqual(policy.pedersenParamsId, 11)
        XCTAssertEqual(policy.pendingTransition?.transitionId, "DEADBEEF")
        XCTAssertEqual(policy.pendingTransition?.previousMode, "Convertible")
        XCTAssertEqual(policy.pendingTransition?.newMode, "ShieldedOnly")
        XCTAssertEqual(policy.pendingTransition?.effectiveHeight, 2048)
        XCTAssertEqual(policy.pendingTransition?.conversionWindow, 200)
        XCTAssertEqual(policy.pendingTransition?.windowOpenHeight, 1848)
    }

    func testGetConfidentialAssetPolicyCompletion() {
        let expectation = expectation(description: "conf-policy")
        let payload = """
        {
          "asset_id":"rose#wonderland",
          "block_height":42,
          "current_mode":"TransparentOnly",
          "effective_mode":"TransparentOnly",
          "vk_set_hash":null,
          "poseidon_params_id":null,
          "pedersen_params_id":null,
          "pending_transition":null
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/confidential/assets/rose%23wonderland/transitions")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        makeClient().getConfidentialAssetPolicy(assetDefinitionId: "rose#wonderland") { result in
            switch result {
            case .success(let policy):
                XCTAssertEqual(policy.blockHeight, 42)
                XCTAssertNil(policy.vkSetHashHex)
                XCTAssertNil(policy.pendingTransition)
            case .failure(let error):
                XCTFail("Unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetConfidentialAssetPolicyRejectsEmptyId() async {
        do {
            _ = try await makeClient().getConfidentialAssetPolicy(assetDefinitionId: "   ")
            XCTFail("Expected rejection for blank asset id")
        } catch {
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
        }
    }
}

final class ToriiClientHeaderTests: XCTestCase {
    func testDecodePdpCommitmentHeaderDecodesData() throws {
        let payload = Data([0x01, 0x02, 0x03])
        let header = payload.base64EncodedString()

        let decoded = try decodePdpCommitmentHeader([ToriiPdpCommitmentHeader: header])

        XCTAssertEqual(decoded, payload)
    }

    func testDecodePdpCommitmentHeaderFromResponse() throws {
        let payload = Data([0xAA, 0xBB])
        let header = payload.base64EncodedString()
        let response = HTTPURLResponse(
            url: URL(string: "https://example.com")!,
            statusCode: 202,
            httpVersion: nil,
            headerFields: ["Sora-PDP-Commitment": header]
        )!

        let decoded = try decodePdpCommitmentHeader(from: response)

        XCTAssertEqual(decoded, payload)
    }

    func testDecodePdpCommitmentHeaderRejectsInvalidPayload() {
        XCTAssertThrowsError(
            try decodePdpCommitmentHeader([ToriiPdpCommitmentHeader: "###"])
        ) { error in
            guard case ToriiClientError.invalidPayload = error else {
                XCTFail("Expected invalidPayload but got \(error)")
                return
            }
        }
    }
}

    @available(iOS 15.0, macOS 12.0, *)
    func testGetRuntimeMetricsAsync() async throws {
        let payload = """
        {"active_abi_versions_count":4,"upgrade_events_total":{"proposed":5,"activated":6,"canceled":1}}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/runtime/metrics")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let metrics = try await makeClient().getRuntimeMetrics()
        XCTAssertEqual(metrics.activeAbiVersionsCount, 4)
        XCTAssertEqual(metrics.upgradeEventsTotal.proposed, 5)
        XCTAssertEqual(metrics.upgradeEventsTotal.activated, 6)
        XCTAssertEqual(metrics.upgradeEventsTotal.canceled, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetRuntimeAbiActiveAsync() async throws {
        let payload = """
        {"active_versions":[1,4,7],"default_compile_target":4}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/runtime/abi/active")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let snapshot = try await makeClient().getRuntimeAbiActive()
        XCTAssertEqual(snapshot.activeVersions, [1, 4, 7])
        XCTAssertEqual(snapshot.defaultCompileTarget, 4)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetRuntimeAbiHashAsync() async throws {
        let payload = """
        {"policy":"V1","abi_hash_hex":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/runtime/abi/hash")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let hash = try await makeClient().getRuntimeAbiHash()
        XCTAssertEqual(hash.policy, "V1")
        XCTAssertEqual(hash.abiHashHex, "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListRuntimeUpgradesAsync() async throws {
        let upgradeId = String(repeating: "9", count: 64)
        let payload = """
        {
          "items": [
            {
              "id_hex": "\(upgradeId)",
              "record": {
                "manifest": {
                  "name": "Upgrade Foo",
                  "description": "Test upgrade",
                  "abi_version": 2,
                  "abi_hash": "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789",
                  "added_syscalls": [42],
                  "added_pointer_types": [7],
                  "start_height": 100,
                  "end_height": 200
                },
                "status": { "ActivatedAt": 123 },
                "proposer": "alice@wonderland",
                "created_height": 90
              }
            }
          ]
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/runtime/upgrades")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let upgrades = try await makeClient().listRuntimeUpgrades()
        XCTAssertEqual(upgrades.count, 1)
        let item = upgrades[0]
        XCTAssertEqual(item.idHex, upgradeId)
        XCTAssertEqual(item.record.proposer, "alice@wonderland")
        XCTAssertEqual(item.record.createdHeight, 90)
        guard case let .activatedAt(height) = item.record.status else {
            return XCTFail("Expected ActivatedAt status")
        }
        XCTAssertEqual(height, 123)
        let manifest = item.record.manifest
        XCTAssertEqual(manifest.name, "Upgrade Foo")
        XCTAssertEqual(manifest.description, "Test upgrade")
        XCTAssertEqual(manifest.abiVersion, 2)
        XCTAssertEqual(manifest.addedSyscalls, [42])
        XCTAssertEqual(manifest.addedPointerTypes, [7])
        XCTAssertEqual(manifest.startHeight, 100)
        XCTAssertEqual(manifest.endHeight, 200)
        XCTAssertEqual(manifest.abiHashHex, "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testProposeRuntimeUpgradeAsync() async throws {
        let manifest = ToriiRuntimeUpgradeManifest(
            name: "Upgrade Foo",
            description: "Test",
            abiVersion: 3,
            abiHashHex: "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789",
            addedSyscalls: [10, 11],
            addedPointerTypes: [],
            startHeight: 123,
            endHeight: 456
        )
        let expectedResponse = """
        {"ok":true,"tx_instructions":[{"wire_id":"Upgrade","payload_hex":"00"}]}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/runtime/upgrades/propose")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            let json = self.bodyJSON(from: request)
            guard let manifestJSON = json["manifest"] as? [String: Any] else {
                return (HTTPURLResponse(url: request.url!, statusCode: 400, httpVersion: nil, headerFields: nil)!, Data())
            }
            XCTAssertEqual(manifestJSON["name"] as? String, "Upgrade Foo")
            XCTAssertEqual(manifestJSON["abi_version"] as? Int, 3)
            XCTAssertEqual(manifestJSON["abi_hash"] as? String, "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
            XCTAssertEqual(manifestJSON["start_height"] as? Int, 123)
            XCTAssertEqual(manifestJSON["end_height"] as? Int, 456)
            if let syscalls = manifestJSON["added_syscalls"] as? [NSNumber] {
                XCTAssertEqual(syscalls.map(\.intValue), [10, 11])
            }
            if let pointerTypes = manifestJSON["added_pointer_types"] as? [NSNumber] {
                XCTAssertTrue(pointerTypes.isEmpty)
            }
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, expectedResponse)
        }

        let action = try await makeClient().proposeRuntimeUpgrade(manifest: manifest)
        XCTAssertTrue(action.ok)
        XCTAssertEqual(action.txInstructions.first?.wireId, "Upgrade")
        XCTAssertEqual(action.txInstructions.first?.payloadHex, "00")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testActivateRuntimeUpgradeAsync() async throws {
        let upgradeId = String(repeating: "a", count: 64)
        let expectedResponse = """
        {"ok":true,"tx_instructions":[{"wire_id":"Activate","payload_hex":"11"}]}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/runtime/upgrades/activate/\(upgradeId)")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            XCTAssertEqual(request.httpBody ?? Data(), Data())
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, expectedResponse)
        }

        let action = try await makeClient().activateRuntimeUpgrade(idHex: "  \(upgradeId) ")
        XCTAssertTrue(action.ok)
        XCTAssertEqual(action.txInstructions.first?.wireId, "Activate")
        XCTAssertEqual(action.txInstructions.first?.payloadHex, "11")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testCancelRuntimeUpgradeAsync() async throws {
        let upgradeId = String(repeating: "b", count: 64)
        let expectedResponse = """
        {"ok":true,"tx_instructions":[{"wire_id":"Cancel","payload_hex":"22"}]}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/runtime/upgrades/cancel/\(upgradeId)")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            XCTAssertEqual(request.httpBody ?? Data(), Data())
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, expectedResponse)
        }

        let action = try await makeClient().cancelRuntimeUpgrade(idHex: upgradeId)
        XCTAssertTrue(action.ok)
        XCTAssertEqual(action.txInstructions.first?.wireId, "Cancel")
        XCTAssertEqual(action.txInstructions.first?.payloadHex, "22")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetVerifyingKeyAsync() async throws {
        let payload = """
        {
          "id": { "backend": "halo2/ipa", "name": "vk main" },
          "record": {
            "version": 2,
            "circuit_id": "halo2/ipa::transfer_v2",
            "backend": "halo2/ipa",
            "curve": "pallas",
            "public_inputs_schema_hash": "fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3",
            "commitment": "20574662a58708e02e0000000000000000000000000000000000000000000000",
            "vk_len": 3,
            "max_proof_bytes": 8192,
            "gas_schedule_id": "halo2_default",
            "metadata_uri_cid": "ipfs://vk-meta",
            "vk_bytes_cid": "ipfs://vk-bundle",
            "activation_height": 1024,
            "status": "Active",
            "key": { "backend": "halo2/ipa", "bytes_b64": "AQID" }
          }
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/zk/vk/halo2%2Fipa/vk%20main")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, payload)
        }

        let detail = try await makeClient().getVerifyingKey(backend: "halo2/ipa", name: "vk main")
        XCTAssertEqual(detail.id.backend, "halo2/ipa")
        XCTAssertEqual(detail.id.name, "vk main")
        XCTAssertEqual(detail.record.version, 2)
        XCTAssertEqual(detail.record.publicInputsSchemaHashHex,
                       "fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3")
        XCTAssertEqual(detail.record.inlineKey?.backend, "halo2/ipa")
        XCTAssertEqual(detail.record.inlineKey?.bytes, Data([0x01, 0x02, 0x03]))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListVerifyingKeysAsync() async throws {
        let payload = """
        [
          { "backend": "halo2/ipa", "name": "vk_ids_only" },
          {
            "id": { "backend": "halo2/ipa", "name": "vk_full" },
            "record": {
              "version": 5,
              "circuit_id": "halo2/ipa::transfer_v5",
              "backend": "halo2/ipa",
              "curve": "pallas",
              "public_inputs_schema_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
              "commitment": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
              "vk_len": 32,
              "max_proof_bytes": 4096,
              "gas_schedule_id": "halo2_default",
              "status": "Active"
            }
          }
        ]
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/zk/vk")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let items = components?.queryItems ?? []
            let dictionary = Dictionary(uniqueKeysWithValues: items.compactMap { item in
                item.value.map { (item.name, $0) }
            })
            XCTAssertEqual(dictionary["backend"], "halo2/ipa")
            XCTAssertEqual(dictionary["status"], "Active")
            XCTAssertEqual(dictionary["name_contains"], "vk")
            XCTAssertEqual(dictionary["limit"], "2")
            XCTAssertEqual(dictionary["offset"], "1")
            XCTAssertEqual(dictionary["order"], "asc")
            XCTAssertEqual(dictionary["ids_only"], "true")

            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, payload)
        }

        let query = ToriiVerifyingKeyListQuery(
            backend: "halo2/ipa",
            status: .active,
            nameContains: "vk",
            limit: 2,
            offset: 1,
            order: .ascending,
            idsOnly: true
        )
        let keys = try await makeClient().listVerifyingKeys(query: query)
        XCTAssertEqual(keys.count, 2)
        XCTAssertEqual(keys[0].id.name, "vk_ids_only")
        XCTAssertNil(keys[0].record)
        XCTAssertEqual(keys[1].record?.version, 5)
        XCTAssertEqual(keys[1].record?.status, .active)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListVerifyingKeysHandlesEnvelope() async throws {
        let payload = """
        {
          "items": [
            {
              "id": { "backend": "halo2/ipa", "name": "vk_enveloped" },
              "record": {
                "version": 1,
                "circuit_id": "halo2/ipa::transfer_v1",
                "backend": "halo2/ipa",
                "curve": "pallas",
                "public_inputs_schema_hash": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                "commitment": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                "vk_len": 64,
                "max_proof_bytes": 2048,
                "gas_schedule_id": "halo2_default",
                "status": "Active"
              }
            }
          ]
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, payload)
        }

        let keys = try await makeClient().listVerifyingKeys()
        XCTAssertEqual(keys.count, 1)
        XCTAssertEqual(keys[0].id.name, "vk_enveloped")
        XCTAssertEqual(keys[0].record?.verifyingKeyLength, 64)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRegisterVerifyingKeyEncodesBody() async throws {
        let requestBody = ToriiVerifyingKeyRegisterRequest(
            authority: "alice@wonderland",
            privateKey: "ed0120...",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            gasScheduleId: "halo2_default",
            verifyingKeyBytes: Data([0x01, 0x02, 0x03])
        )

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/zk/vk/register")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            let json = self.bodyJSON(from: request)
            XCTAssertEqual(json["authority"] as? String, "alice@wonderland")
            XCTAssertEqual(json["backend"] as? String, "halo2/ipa")
            XCTAssertEqual(json["public_inputs_schema_hash_hex"] as? String,
                           "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")
            XCTAssertEqual(json["vk_bytes"] as? String, Data([0x01, 0x02, 0x03]).base64EncodedString())
            XCTAssertEqual(json["vk_len"] as? Int, 3)
            XCTAssertNil(json["vk_bytes_cid"])
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 202,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, Data())
        }

        try await makeClient().registerVerifyingKey(requestBody)
    }

    func testRegisterVerifyingKeyRejectsInvalidSchemaHash() {
        let requestBody = ToriiVerifyingKeyRegisterRequest(
            authority: "alice@wonderland",
            privateKey: "ed0120...",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: "abc",
            gasScheduleId: "halo2_default"
        )
        XCTAssertThrowsError(try JSONEncoder().encode(requestBody)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testRegisterVerifyingKeyRejectsVkLengthMismatch() {
        var requestBody = ToriiVerifyingKeyRegisterRequest(
            authority: "alice@wonderland",
            privateKey: "ed0120...",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: String(repeating: "a", count: 64),
            gasScheduleId: "halo2_default",
            verifyingKeyBytes: Data([0x01, 0x02])
        )
        requestBody.verifyingKeyLength = 3
        XCTAssertThrowsError(try JSONEncoder().encode(requestBody)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testUpdateVerifyingKeyEncodesBody() async throws {
        var requestBody = ToriiVerifyingKeyUpdateRequest(
            authority: "alice@wonderland",
            privateKey: "ed0120...",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 2,
            circuitId: "halo2/ipa::transfer_v2",
            publicInputsSchemaHashHex: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        )
        requestBody.verifyingKeyBytes = Data([0xAA])
        requestBody.commitmentHex = "20574662a58708e02e0000000000000000000000000000000000000000000000"

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/zk/vk/update")
            let json = self.bodyJSON(from: request)
            XCTAssertEqual(json["public_inputs_schema_hash_hex"] as? String,
                           "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
            XCTAssertEqual(json["vk_bytes"] as? String, Data([0xAA]).base64EncodedString())
            XCTAssertEqual(json["vk_len"] as? Int, 1)
            XCTAssertEqual(json["commitment_hex"] as? String,
                           "20574662a58708e02e0000000000000000000000000000000000000000000000")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 202,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, Data())
        }

        try await makeClient().updateVerifyingKey(requestBody)
    }

    func testUpdateVerifyingKeyRejectsInvalidCommitmentHex() {
        var requestBody = ToriiVerifyingKeyUpdateRequest(
            authority: "alice@wonderland",
            privateKey: "ed0120...",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 2,
            circuitId: "halo2/ipa::transfer_v2",
            publicInputsSchemaHashHex: String(repeating: "a", count: 64)
        )
        requestBody.commitmentHex = "deadbeef"
        XCTAssertThrowsError(try JSONEncoder().encode(requestBody)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamVerifyingKeyEventsAsync() async throws {
        let ssePayload = """
id: 15
event: message
data: {"VerifyingKey":{"Registered":{"id":{"backend":"halo2/ipa","name":"vk_main"},"record":{"version":2,"circuit_id":"halo2/ipa::transfer_v2","backend":"halo2/ipa","curve":"pallas","public_inputs_schema_hash":"fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3","commitment":"20574662a58708e02e0000000000000000000000000000000000000000000000","vk_len":96,"max_proof_bytes":8192,"gas_schedule_id":"halo2_default","status":"Active"}}}}

id: 16
data: {"VerifyingKey":{"Updated":{"id":{"backend":"halo2/ipa","name":"vk_main"},"record":{"version":3,"circuit_id":"halo2/ipa::transfer_v3","backend":"halo2/ipa","curve":"pallas","public_inputs_schema_hash":"fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3","commitment":"20574662a58708e02e0000000000000000000000000000000000000000000000","vk_len":96,"max_proof_bytes":8192,"gas_schedule_id":"halo2_default","status":"Active"}}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/event-stream")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamVerifyingKeyEvents(filter: ToriiVerifyingKeyEventFilter(backend: "halo2/ipa",
                                                                                                name: "vk_main"))
        var iterator = stream.makeAsyncIterator()

        let first = try await iterator.next()
        guard case let .registered(id, record)? = first?.event else {
            return XCTFail("Expected registered event")
        }
        XCTAssertEqual(first?.eventId, "15")
        XCTAssertEqual(id.backend, "halo2/ipa")
        XCTAssertEqual(id.name, "vk_main")
        XCTAssertEqual(record.version, 2)
        XCTAssertEqual(first?.rawEvent.contains("Registered"), true)

        let second = try await iterator.next()
        guard case let .updated(updatedId, updatedRecord)? = second?.event else {
            return XCTFail("Expected updated event")
        }
        XCTAssertEqual(second?.eventId, "16")
        XCTAssertEqual(updatedId.backend, "halo2/ipa")
        XCTAssertEqual(updatedId.name, "vk_main")
        XCTAssertEqual(updatedRecord.version, 3)

        let third = try await iterator.next()
        XCTAssertNil(third)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamVerifyingKeyEventsRejectsMultiplePayloadKinds() async throws {
        let ssePayload = """
id: 91
data: {"VerifyingKey":{"Registered":{"id":{"backend":"halo2/ipa","name":"vk_main"},"record":{"version":2,"circuit_id":"halo2/ipa::transfer_v2","backend":"halo2/ipa","curve":"pallas","public_inputs_schema_hash":"fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3","commitment":"20574662a58708e02e0000000000000000000000000000000000000000000000","vk_len":96,"max_proof_bytes":8192,"gas_schedule_id":"halo2_default","status":"Active"}},"Updated":{"id":{"backend":"halo2/ipa","name":"vk_main"},"record":{"version":2,"circuit_id":"halo2/ipa::transfer_v2","backend":"halo2/ipa","curve":"pallas","public_inputs_schema_hash":"fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3","commitment":"20574662a58708e02e0000000000000000000000000000000000000000000000","vk_len":96,"max_proof_bytes":8192,"gas_schedule_id":"halo2_default","status":"Active"}}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/event-stream")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamVerifyingKeyEvents()
        var iterator = stream.makeAsyncIterator()
        do {
            _ = try await iterator.next()
            XCTFail("Expected verifying key event decoding error")
        } catch {
            guard case ToriiClientError.decoding = error else {
                return XCTFail("Expected ToriiClientError.decoding")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamVerifyingKeyEventsRejectsInvalidIdComponent() async throws {
        let ssePayload = """
id: 92
data: {"VerifyingKey":{"Registered":{"id":{"backend":"halo2:ipa","name":"vk_main"},"record":{"version":2,"circuit_id":"halo2/ipa::transfer_v2","backend":"halo2/ipa","curve":"pallas","public_inputs_schema_hash":"fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3","commitment":"20574662a58708e02e0000000000000000000000000000000000000000000000","vk_len":96,"max_proof_bytes":8192,"gas_schedule_id":"halo2_default","status":"Active"}}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamVerifyingKeyEvents()
        var iterator = stream.makeAsyncIterator()
        do {
            _ = try await iterator.next()
            XCTFail("Expected verifying key event decoding error")
        } catch {
            guard case ToriiClientError.decoding = error else {
                return XCTFail("Expected ToriiClientError.decoding")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamVerifyingKeyEventsRejectsInvalidRecordHex() async throws {
        let ssePayload = """
id: 93
data: {"VerifyingKey":{"Registered":{"id":{"backend":"halo2/ipa","name":"vk_main"},"record":{"version":2,"circuit_id":"halo2/ipa::transfer_v2","backend":"halo2/ipa","curve":"pallas","public_inputs_schema_hash":"zz","commitment":"20574662a58708e02e0000000000000000000000000000000000000000000000","vk_len":96,"max_proof_bytes":8192,"gas_schedule_id":"halo2_default","status":"Active"}}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamVerifyingKeyEvents()
        var iterator = stream.makeAsyncIterator()
        do {
            _ = try await iterator.next()
            XCTFail("Expected verifying key event decoding error")
        } catch {
            guard case ToriiClientError.decoding = error else {
                return XCTFail("Expected ToriiClientError.decoding")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamVerifyingKeyEventsRejectsNegativeVkLength() async throws {
        let ssePayload = """
id: 94
data: {"VerifyingKey":{"Registered":{"id":{"backend":"halo2/ipa","name":"vk_main"},"record":{"version":2,"circuit_id":"halo2/ipa::transfer_v2","backend":"halo2/ipa","curve":"pallas","public_inputs_schema_hash":"fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3","commitment":"20574662a58708e02e0000000000000000000000000000000000000000000000","vk_len":-1,"max_proof_bytes":8192,"gas_schedule_id":"halo2_default","status":"Active"}}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamVerifyingKeyEvents()
        var iterator = stream.makeAsyncIterator()
        do {
            _ = try await iterator.next()
            XCTFail("Expected verifying key event decoding error")
        } catch {
            guard case ToriiClientError.decoding = error else {
                return XCTFail("Expected ToriiClientError.decoding")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamVerifyingKeyEventsRejectsEmptyInlineKeyBytes() async throws {
        let ssePayload = """
id: 95
data: {"VerifyingKey":{"Registered":{"id":{"backend":"halo2/ipa","name":"vk_main"},"record":{"version":2,"circuit_id":"halo2/ipa::transfer_v2","backend":"halo2/ipa","curve":"pallas","public_inputs_schema_hash":"fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3","commitment":"20574662a58708e02e0000000000000000000000000000000000000000000000","vk_len":96,"max_proof_bytes":8192,"gas_schedule_id":"halo2_default","status":"Active","key":{"backend":"halo2/ipa","bytes_b64":""}}}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamVerifyingKeyEvents()
        var iterator = stream.makeAsyncIterator()
        do {
            _ = try await iterator.next()
            XCTFail("Expected verifying key event decoding error")
        } catch {
            guard case ToriiClientError.decoding = error else {
                return XCTFail("Expected ToriiClientError.decoding")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamVerifyingKeyEventsRejectsInlineKeyBackendMismatch() async throws {
        let ssePayload = """
id: 96
data: {"VerifyingKey":{"Registered":{"id":{"backend":"halo2/ipa","name":"vk_main"},"record":{"version":2,"circuit_id":"halo2/ipa::transfer_v2","backend":"halo2/ipa","curve":"pallas","public_inputs_schema_hash":"fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3","commitment":"20574662a58708e02e0000000000000000000000000000000000000000000000","vk_len":2,"max_proof_bytes":8192,"gas_schedule_id":"halo2_default","status":"Active","key":{"backend":"halo2/ipa-alt","bytes_b64":"AQI="}}}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamVerifyingKeyEvents()
        var iterator = stream.makeAsyncIterator()
        do {
            _ = try await iterator.next()
            XCTFail("Expected verifying key event decoding error")
        } catch {
            guard case ToriiClientError.decoding = error else {
                return XCTFail("Expected ToriiClientError.decoding")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamVerifyingKeyEventsRejectsInlineKeyLengthMismatch() async throws {
        let ssePayload = """
id: 97
data: {"VerifyingKey":{"Registered":{"id":{"backend":"halo2/ipa","name":"vk_main"},"record":{"version":2,"circuit_id":"halo2/ipa::transfer_v2","backend":"halo2/ipa","curve":"pallas","public_inputs_schema_hash":"fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3","commitment":"20574662a58708e02e0000000000000000000000000000000000000000000000","vk_len":3,"max_proof_bytes":8192,"gas_schedule_id":"halo2_default","status":"Active","key":{"backend":"halo2/ipa","bytes_b64":"AQI="}}}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamVerifyingKeyEvents()
        var iterator = stream.makeAsyncIterator()
        do {
            _ = try await iterator.next()
            XCTFail("Expected verifying key event decoding error")
        } catch {
            guard case ToriiClientError.decoding = error else {
                return XCTFail("Expected ToriiClientError.decoding")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamVerifyingKeyEventsIncludesLastEventIdHeader() async throws {
        let ssePayload = """
id: 21
data: {"VerifyingKey":{"Registered":{"id":{"backend":"halo2/ipa","name":"vk_main"},"record":{"version":1,"circuit_id":"halo2/ipa::transfer_v1","backend":"halo2/ipa","curve":"pallas","public_inputs_schema_hash":"fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3","commitment":"20574662a58708e02e0000000000000000000000000000000000000000000000","vk_len":96,"max_proof_bytes":8192,"gas_schedule_id":"halo2_default","status":"Active"}}}}

"""
            .data(using: .utf8)!

        var lastEventIdHeader: String?
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/event-stream")
            lastEventIdHeader = request.value(forHTTPHeaderField: "Last-Event-ID")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamVerifyingKeyEvents(lastEventId: "99")
        var iterator = stream.makeAsyncIterator()
        let event = try await iterator.next()
        guard case .registered? = event?.event else {
            return XCTFail("Expected registered event")
        }
        let finished = try await iterator.next()
        XCTAssertNil(finished)
        XCTAssertEqual(lastEventIdHeader, "99")
    }

#if canImport(Combine)
    @available(iOS 15.0, macOS 12.0, *)
    func testAssetsPublisherDeliversBalances() throws {
        let payload = """
[
  {"asset_id":"rose#wonderland","quantity":"10"},
  {"asset_id":"ksm#wonderland","quantity":"20"}
]
"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/accounts/alice%40wonderland/assets")
            XCTAssertEqual(request.url?.query, "limit=2")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let client = makeClient()
        var cancellables: Set<AnyCancellable> = []
        let valueExpectation = expectation(description: "received balances")
        let completionExpectation = expectation(description: "publisher finished")

        var balances: [ToriiAssetBalance] = []
        client.assetsPublisher(accountId: "alice@wonderland", limit: 2, scheduler: nil)
            .sink { completion in
                if case .failure(let error) = completion {
                    XCTFail("Unexpected failure: \(error)")
                }
                completionExpectation.fulfill()
            } receiveValue: { value in
                balances = value
                valueExpectation.fulfill()
            }
            .store(in: &cancellables)

        waitForExpectations(timeout: 2.0)
        XCTAssertEqual(balances.count, 2)
        XCTAssertEqual(balances.first?.asset_id, "rose#wonderland")
        XCTAssertEqual(balances.first?.quantity, "10")
        XCTAssertEqual(balances.last?.asset_id, "ksm#wonderland")
        XCTAssertEqual(balances.last?.quantity, "20")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testVerifyingKeyEventsPublisherBridgesSseStream() throws {
        let ssePayload = """
id: 15
event: message
data: {"VerifyingKey":{"Registered":{"id":{"backend":"halo2/ipa","name":"vk_main"},"record":{"version":2,"circuit_id":"halo2/ipa::transfer_v2","backend":"halo2/ipa","curve":"pallas","public_inputs_schema_hash":"fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3","commitment":"20574662a58708e02e0000000000000000000000000000000000000000000000","vk_len":96,"max_proof_bytes":8192,"gas_schedule_id":"halo2_default","status":"Active"}}}}

id: 16
data: {"VerifyingKey":{"Updated":{"id":{"backend":"halo2/ipa","name":"vk_main"},"record":{"version":3,"circuit_id":"halo2/ipa::transfer_v3","backend":"halo2/ipa","curve":"pallas","public_inputs_schema_hash":"fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3","commitment":"20574662a58708e02e0000000000000000000000000000000000000000000000","vk_len":96,"max_proof_bytes":8192,"gas_schedule_id":"halo2_default","status":"Active"}}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/event-stream")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "text/event-stream"])!
            return (response, ssePayload)
        }

        let client = makeClient()
        var cancellables: Set<AnyCancellable> = []
        let valueExpectation = expectation(description: "received events")
        valueExpectation.expectedFulfillmentCount = 2
        let completionExpectation = expectation(description: "publisher completed")

        var events: [ToriiVerifyingKeyEventMessage] = []
        client.verifyingKeyEventsPublisher(filter: ToriiVerifyingKeyEventFilter(backend: "halo2/ipa", name: "vk_main"),
                                           scheduler: nil)
            .sink { completion in
                if case .failure(let error) = completion {
                    XCTFail("Unexpected failure: \(error)")
                }
                completionExpectation.fulfill()
            } receiveValue: { message in
                events.append(message)
                valueExpectation.fulfill()
            }
            .store(in: &cancellables)

        waitForExpectations(timeout: 2.0)

        guard events.count == 2 else {
            return XCTFail("Expected two events")
        }
        guard case let .registered(id, record) = events[0].event else {
            return XCTFail("Expected registered event")
        }
        XCTAssertEqual(events[0].eventId, "15")
        XCTAssertEqual(id.backend, "halo2/ipa")
        XCTAssertEqual(id.name, "vk_main")
        XCTAssertEqual(record.status, .active)

        guard case let .updated(updatedId, updatedRecord) = events[1].event else {
            return XCTFail("Expected updated event")
        }
        XCTAssertEqual(events[1].eventId, "16")
        XCTAssertEqual(updatedId.backend, "halo2/ipa")
        XCTAssertEqual(updatedId.name, "vk_main")
        XCTAssertEqual(updatedRecord.version, 3)
    }
#endif

    func testVerifyingKeyEventFilterRequiresBackendAndName() {
        XCTAssertThrowsError(try ToriiVerifyingKeyEventFilter(backend: "halo2/ipa", name: nil).queryItems()) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testVerifyingKeyEventFilterRejectsInvalidBackendOrName() {
        XCTAssertThrowsError(try ToriiVerifyingKeyEventFilter(backend: " ", name: "vk").queryItems()) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
        XCTAssertThrowsError(try ToriiVerifyingKeyEventFilter(backend: "halo2/ipa", name: "vk:main").queryItems()) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamTriggerEventsAsync() async throws {
        let ssePayload = """
id: 101
event: lifecycle
data: {"Trigger":{"Created":"nightly-tick"}}

id: 102
data: {"Trigger":{"Deleted":"nightly-tick"}}

id: 103
data: {"Trigger":{"Extended":{"trigger":"nightly-tick","by":3}}}

id: 104
data: {"Trigger":{"Shortened":{"trigger":"nightly-tick","by":1}}}

id: 105
data: {"Trigger":{"MetadataInserted":{"target":"nightly-tick","key":"mode","value":"fast"}}}

id: 106
data: {"Trigger":{"MetadataRemoved":{"target":"nightly-tick","key":"mode","value":null}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/event-stream")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        var filter = ToriiTriggerEventFilter(triggerId: "nightly-tick")
        filter.includeMetadataInserted = true
        filter.includeMetadataRemoved = true

        let stream = makeClient().streamTriggerEvents(filter: filter)
        var iterator = stream.makeAsyncIterator()

        let created = try await iterator.next()
        guard case let .created(id)? = created?.event else {
            return XCTFail("Expected created trigger event")
        }
        XCTAssertEqual(created?.eventId, "101")
        XCTAssertEqual(created?.eventName, "lifecycle")
        XCTAssertEqual(id, "nightly-tick")
        XCTAssertTrue(created?.rawEvent.contains("Created") ?? false)

        let deleted = try await iterator.next()
        guard case let .deleted(deletedId)? = deleted?.event else {
            return XCTFail("Expected deleted trigger event")
        }
        XCTAssertEqual(deleted?.eventId, "102")
        XCTAssertEqual(deletedId, "nightly-tick")

        let extended = try await iterator.next()
        guard case let .extended(extensionChange)? = extended?.event else {
            return XCTFail("Expected extended trigger event")
        }
        XCTAssertEqual(extended?.eventId, "103")
        XCTAssertEqual(extensionChange.triggerId, "nightly-tick")
        XCTAssertEqual(extensionChange.delta, 3)

        let shortened = try await iterator.next()
        guard case let .shortened(shortenChange)? = shortened?.event else {
            return XCTFail("Expected shortened trigger event")
        }
        XCTAssertEqual(shortened?.eventId, "104")
        XCTAssertEqual(shortenChange.triggerId, "nightly-tick")
        XCTAssertEqual(shortenChange.delta, 1)

        let inserted = try await iterator.next()
        guard case let .metadataInserted(metadata)? = inserted?.event else {
            return XCTFail("Expected metadata inserted trigger event")
        }
        XCTAssertEqual(inserted?.eventId, "105")
        XCTAssertEqual(metadata.triggerId, "nightly-tick")
        XCTAssertEqual(metadata.key, "mode")
        XCTAssertEqual(metadata.value, .string("fast"))

        let removed = try await iterator.next()
        guard case let .metadataRemoved(metadataRemoved)? = removed?.event else {
            return XCTFail("Expected metadata removed trigger event")
        }
        XCTAssertEqual(removed?.eventId, "106")
        XCTAssertEqual(metadataRemoved.triggerId, "nightly-tick")
        XCTAssertEqual(metadataRemoved.key, "mode")
        XCTAssertEqual(metadataRemoved.value, .null)

        let finished = try await iterator.next()
        XCTAssertNil(finished)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamTriggerEventsRejectsMultiplePayloadKinds() async throws {
        let ssePayload = """
id: 301
data: {"Trigger":{"Created":"nightly-tick","Deleted":"nightly-tick"}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/event-stream")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamTriggerEvents()
        var iterator = stream.makeAsyncIterator()
        do {
            _ = try await iterator.next()
            XCTFail("Expected trigger event decoding error")
        } catch {
            guard case ToriiClientError.decoding = error else {
                return XCTFail("Expected ToriiClientError.decoding")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamTriggerEventsIncludesLastEventIdHeader() async throws {
        let ssePayload = """
id: 205
data: {"Trigger":{"Deleted":"nightly-tick"}}

"""
            .data(using: .utf8)!

        var lastEventIdHeader: String?
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/event-stream")
            lastEventIdHeader = request.value(forHTTPHeaderField: "Last-Event-ID")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamTriggerEvents(lastEventId: "resume-me")
        var iterator = stream.makeAsyncIterator()
        let event = try await iterator.next()
        guard case let .deleted(id)? = event?.event else {
            return XCTFail("Expected deleted trigger event")
        }
        XCTAssertEqual(id, "nightly-tick")
        XCTAssertEqual(lastEventIdHeader, "resume-me")
        let finished = try await iterator.next()
        XCTAssertNil(finished)
    }

    func testTriggerEventFilterRequiresAtLeastOneEventType() {
        XCTAssertThrowsError(
            try ToriiTriggerEventFilter(includeCreated: false,
                                        includeDeleted: false,
                                        includeExtended: false,
                                        includeShortened: false,
                                        includeMetadataInserted: false,
                                        includeMetadataRemoved: false).queryItems()
        ) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testTriggerEventFilterEncodesMatcherAndEventSet() throws {
        let filter = ToriiTriggerEventFilter(triggerId: "nightly-tick",
                                             includeCreated: true,
                                             includeDeleted: false,
                                             includeExtended: true,
                                             includeShortened: false,
                                             includeMetadataInserted: false,
                                             includeMetadataRemoved: true)
        let queryItems = try XCTUnwrap(filter.queryItems())
        XCTAssertEqual(queryItems.count, 1)
        XCTAssertEqual(queryItems[0].name, "filter")
        let data = try XCTUnwrap(queryItems[0].value?.data(using: .utf8))
        let decoded = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let trigger = try XCTUnwrap(decoded["Trigger"] as? [String: Any])
        XCTAssertEqual(trigger["id_matcher"] as? String, "nightly-tick")
        let eventSet = try XCTUnwrap(trigger["event_set"] as? [String: Any])
        XCTAssertEqual(eventSet["Created"] as? Bool, true)
        XCTAssertEqual(eventSet["Deleted"] as? Bool, false)
        XCTAssertEqual(eventSet["Extended"] as? Bool, true)
        XCTAssertEqual(eventSet["Shortened"] as? Bool, false)
        XCTAssertEqual(eventSet["MetadataInserted"] as? Bool, false)
        XCTAssertEqual(eventSet["MetadataRemoved"] as? Bool, true)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamProofEventsAsync() async throws {
        let ssePayload = """
id: 42
        data: {"Proof":{"Verified":{"id":{"backend":"halo2/ipa","proof_hash_hex":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"vk_ref":{"backend":"halo2/ipa","name":"vk_main"},"vk_commitment":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","call_hash":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","envelope_hash":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}}}

id: 43
        data: {"Proof":{"Rejected":{"id":{"backend":"halo2/ipa","proof_hash_hex":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/event-stream")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamProofEvents(filter: ToriiProofEventFilter(backend: "halo2/ipa",
                                                                                  proofHashHex: String(repeating: "a", count: 64),
                                                                                  includeVerified: true,
                                                                                  includeRejected: true))
        var iterator = stream.makeAsyncIterator()

        let verified = try await iterator.next()
        guard case let .verified(payload)? = verified?.event else {
            return XCTFail("Expected verified proof event")
        }
        XCTAssertEqual(verified?.eventId, "42")
        XCTAssertEqual(payload.id.backend, "halo2/ipa")
        XCTAssertEqual(payload.id.proofHashHex, String(repeating: "a", count: 64))
        XCTAssertEqual(payload.verifyingKeyId?.name, "vk_main")
        XCTAssertEqual(payload.verifyingKeyCommitmentHex, String(repeating: "b", count: 64))
        XCTAssertEqual(payload.callHashHex, String(repeating: "c", count: 64))
        XCTAssertEqual(payload.envelopeHashHex, String(repeating: "d", count: 64))

        let rejected = try await iterator.next()
        guard case let .rejected(rejectedPayload)? = rejected?.event else {
            return XCTFail("Expected rejected proof event")
        }
        XCTAssertEqual(rejected?.eventId, "43")
        XCTAssertEqual(rejectedPayload.id.proofHashHex, String(repeating: "a", count: 64))
        XCTAssertNil(rejectedPayload.verifyingKeyId)

        let finished = try await iterator.next()
        XCTAssertNil(finished)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamProofEventsRejectsMultiplePayloadKinds() async throws {
        let ssePayload = """
id: 77
data: {"Proof":{"Verified":{"id":{"backend":"halo2/ipa","proof_hash_hex":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}},"Rejected":{"id":{"backend":"halo2/ipa","proof_hash_hex":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/event-stream")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamProofEvents()
        var iterator = stream.makeAsyncIterator()
        do {
            _ = try await iterator.next()
            XCTFail("Expected proof event decoding error")
        } catch {
            guard case ToriiClientError.decoding = error else {
                return XCTFail("Expected ToriiClientError.decoding")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamProofEventsRejectsInvalidProofHashHex() async throws {
        let ssePayload = """
id: 90
data: {"Proof":{"Rejected":{"id":{"backend":"halo2/ipa","proof_hash_hex":"abcd"}}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamProofEvents()
        var iterator = stream.makeAsyncIterator()
        do {
            _ = try await iterator.next()
            XCTFail("Expected proof event decoding error")
        } catch {
            guard case ToriiClientError.decoding = error else {
                return XCTFail("Expected ToriiClientError.decoding")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamProofEventsRejectsInvalidCommitmentHex() async throws {
        let ssePayload = """
id: 91
data: {"Proof":{"Verified":{"id":{"backend":"halo2/ipa","proof_hash_hex":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"vk_commitment":"zzzz"}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamProofEvents()
        var iterator = stream.makeAsyncIterator()
        do {
            _ = try await iterator.next()
            XCTFail("Expected proof event decoding error")
        } catch {
            guard case ToriiClientError.decoding = error else {
                return XCTFail("Expected ToriiClientError.decoding")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamProofEventsRejectsInvalidBackend() async throws {
        let ssePayload = """
id: 92
data: {"Proof":{"Rejected":{"id":{"backend":"halo2:ipa","proof_hash_hex":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}}}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamProofEvents()
        var iterator = stream.makeAsyncIterator()
        do {
            _ = try await iterator.next()
            XCTFail("Expected proof event decoding error")
        } catch {
            guard case ToriiClientError.decoding = error else {
                return XCTFail("Expected ToriiClientError.decoding")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamProofEventsIncludesLastEventIdHeader() async throws {
        let ssePayload = """
id: 88
        data: {"Proof":{"Rejected":{"id":{"backend":"halo2/ipa","proof_hash_hex":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}}}}

"""
            .data(using: .utf8)!

        var lastEventIdHeader: String?
        StubURLProtocol.handler = { request in
            lastEventIdHeader = request.value(forHTTPHeaderField: "Last-Event-ID")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        let stream = makeClient().streamProofEvents(lastEventId: "123")
        var iterator = stream.makeAsyncIterator()
        let event = try await iterator.next()
        guard case .rejected? = event?.event else {
            return XCTFail("Expected rejected proof event")
        }
        XCTAssertEqual(lastEventIdHeader, "123")
        let finished = try await iterator.next()
        XCTAssertNil(finished)
    }

    func testProofEventFilterRequiresBackendAndHash() {
        XCTAssertThrowsError(try ToriiProofEventFilter(backend: "halo2/ipa", proofHashHex: nil).queryItems()) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
        XCTAssertThrowsError(try ToriiProofEventFilter(backend: "halo2/ipa",
                                                       proofHashHex: "abc",
                                                       includeVerified: true,
                                                       includeRejected: true).queryItems()) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testProofEventFilterRejectsInvalidBackendOrHash() {
        let invalidHash = String(repeating: "z", count: 64)
        XCTAssertThrowsError(try ToriiProofEventFilter(backend: "halo2:ipa",
                                                       proofHashHex: String(repeating: "a", count: 64),
                                                       includeVerified: true,
                                                       includeRejected: true).queryItems()) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
        XCTAssertThrowsError(try ToriiProofEventFilter(backend: "halo2/ipa",
                                                       proofHashHex: invalidHash,
                                                       includeVerified: true,
                                                       includeRejected: true).queryItems()) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }


    @available(iOS 15.0, macOS 12.0, *)
    func testGetTimeStatusAsync() async throws {
        let payload = """
        {"peers":3,"samples":[{"peer":"peer1","last_offset_ms":1,"last_rtt_ms":2,"count":3}],"rtt":{"buckets":[{"le":5,"count":10}],"sum_ms":20,"count":10},"note":"NTS running"}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/time/status")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let status = try await makeClient().getTimeStatus()
        XCTAssertEqual(status.peers, 3)
        XCTAssertEqual(status.samples.first?.peer, "peer1")
        XCTAssertEqual(status.rtt?.buckets.first?.le, 5)
        XCTAssertEqual(status.note, "NTS running")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetSumeragiStatusParsesMembershipAsync() async throws {
        let payload = """
        {"leader_index":1,"membership":{"height":11,"view":3,"epoch":2,"view_hash":"deadbeef"}}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/sumeragi/status")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let snapshot = try await makeClient().getSumeragiStatus()
        XCTAssertEqual(snapshot.membership?.height, 11)
        XCTAssertEqual(snapshot.membership?.view, 3)
        XCTAssertEqual(snapshot.membership?.epoch, 2)
        XCTAssertEqual(snapshot.membership?.viewHash, "deadbeef")
        guard case let .number(leaderIndex)? = snapshot.fields["leader_index"] else {
            XCTFail("Expected leader_index to be decoded as number")
            return
        }
        XCTAssertEqual(leaderIndex, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetSumeragiStatusDecodesLaneSnapshots() async throws {
        let payload = """
        {
            "membership": {"height": 15, "view": 4, "epoch": 2, "view_hash": "cab00d1e"},
            "lane_commitments": [
                {
                    "block_height": 42,
                    "lane_id": 7,
                    "tx_count": 3,
                    "total_chunks": 5,
                    "rbc_bytes_total": 2048,
                    "teu_total": 96,
                    "block_hash": "deadbeef"
                }
            ],
            "dataspace_commitments": [
                {
                    "block_height": 42,
                    "lane_id": 7,
                    "dataspace_id": 9,
                    "tx_count": 1,
                    "total_chunks": 2,
                    "rbc_bytes_total": 512,
                    "teu_total": 32,
                    "block_hash": "feedface"
                }
            ],
            "lane_governance": [
                {
                    "lane_id": 7,
                    "alias": "payments",
                    "dataspace_id": 9,
                    "visibility": "public",
                    "storage_profile": "full_replica",
                    "governance": "parliament",
                    "manifest_required": true,
                    "manifest_ready": true,
                    "manifest_path": "/etc/lanes/payments.json",
                    "validator_ids": ["alice@test", "bob@test"],
                    "quorum": 2,
                    "protected_namespaces": ["treasury"],
                    "runtime_upgrade": {
                        "allow": true,
                        "require_metadata": true,
                        "metadata_key": "upgrade-id",
                        "allowed_ids": ["payments-v1"]
                    },
                    "privacy_commitments": [
                        {
                            "id": 5,
                            "scheme": "merkle",
                            "merkle": {"root": "0xaaaabbbb", "max_depth": 16}
                        },
                        {
                            "id": 6,
                            "scheme": "snark",
                            "snark": {
                                "circuit_id": 2,
                                "verifying_key_digest": "0x11112222",
                                "statement_hash": "0x33334444",
                                "proof_hash": "0x55556666"
                            }
                        }
                    ]
                }
            ]
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/sumeragi/status")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, payload)
        }

        let snapshot = try await makeClient().getSumeragiStatus()
        XCTAssertEqual(snapshot.membership?.height, 15)
        XCTAssertEqual(snapshot.laneCommitments.count, 1)
        XCTAssertEqual(snapshot.laneCommitments.first?.laneId, 7)
        XCTAssertEqual(snapshot.laneCommitments.first?.teuTotal, 96)
        XCTAssertEqual(snapshot.dataspaceCommitments.first?.dataspaceId, 9)
        XCTAssertEqual(snapshot.dataspaceCommitments.first?.rbcBytesTotal, 512)
        XCTAssertEqual(snapshot.laneGovernance.first?.alias, "payments")
        XCTAssertEqual(snapshot.laneGovernance.first?.dataspaceId, 9)
        XCTAssertEqual(snapshot.laneGovernance.first?.visibility, "public")
        XCTAssertEqual(snapshot.laneGovernance.first?.storageProfile, "full_replica")
        XCTAssertEqual(snapshot.laneGovernance.first?.validatorIds.count, 2)
        XCTAssertEqual(snapshot.laneGovernance.first?.runtimeUpgrade?.metadataKey, "upgrade-id")
        XCTAssertEqual(snapshot.laneGovernance.first?.privacyCommitments.count, 2)
        XCTAssertEqual(snapshot.laneGovernance.first?.privacyCommitments.first?.merkle?.maxDepth, 16)
        XCTAssertEqual(snapshot.laneGovernance.first?.privacyCommitments.last?.snark?.circuitId, 2)
        guard case let .array(governanceRaw)? = snapshot.fields["lane_governance"] else {
            return XCTFail("Expected raw governance array in fields")
        }
        XCTAssertEqual(governanceRaw.count, 1)
    }

    func testSumeragiMembershipDecodingWithoutViewHash() throws {
        let payload = """
        {"membership":{"height":5,"view":2,"epoch":1}}
        """.data(using: .utf8)!

        let snapshot = try JSONDecoder().decode(ToriiSumeragiStatusSnapshot.self, from: payload)
        XCTAssertEqual(snapshot.membership?.height, 5)
        XCTAssertEqual(snapshot.membership?.view, 2)
        XCTAssertEqual(snapshot.membership?.epoch, 1)
        XCTAssertNil(snapshot.membership?.viewHash)
        XCTAssertNil(snapshot.fields["leader_index"])
    }

    func testSumeragiStatusDecodesModeAndConsensusCaps() throws {
        let payload: [String: Any] = [
            "mode_tag": "iroha2-consensus::permissioned-sumeragi@v1",
            "staged_mode_tag": "iroha2-consensus::npos-sumeragi@v1",
            "staged_mode_activation_height": 10,
            "mode_activation_lag_blocks": 2,
            "consensus_caps": [
                "collectors_k": 2,
                "redundant_send_r": 1,
                "da_enabled": true,
                "rbc_chunk_max_bytes": 1024,
                "rbc_session_ttl_ms": 5000,
                "rbc_store_max_sessions": 64,
                "rbc_store_soft_sessions": 32,
                "rbc_store_max_bytes": 4096,
                "rbc_store_soft_bytes": 2048,
            ],
        ]
        let data = try JSONSerialization.data(withJSONObject: payload, options: [])
        let snapshot = try JSONDecoder().decode(ToriiSumeragiStatusSnapshot.self, from: data)
        XCTAssertEqual(snapshot.modeTag, "iroha2-consensus::permissioned-sumeragi@v1")
        XCTAssertEqual(snapshot.stagedModeTag, "iroha2-consensus::npos-sumeragi@v1")
        XCTAssertEqual(snapshot.stagedModeActivationHeight, 10)
        XCTAssertEqual(snapshot.modeActivationLagBlocks, 2)
        XCTAssertEqual(snapshot.consensusCaps?.collectorsK, 2)
        XCTAssertEqual(snapshot.consensusCaps?.rbcChunkMaxBytes, 1024)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStatusSnapshotTracksMetrics() async throws {
        var responses = [
            """
            {"peers":2,"queue_size":4,"commit_time_ms":45,"txs_approved":5,"txs_rejected":1,"view_changes":0}
            """.data(using: .utf8)!,
            """
            {"peers":3,"queue_size":11,"commit_time_ms":120,"txs_approved":9,"txs_rejected":3,"view_changes":2}
            """.data(using: .utf8)!,
        ]

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/status")
            guard let body = responses.first else {
                throw NSError(domain: "Stub", code: -1)
            }
            responses.removeFirst()
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, body)
        }

        let client = makeClient()
        let first = try await client.getStatusSnapshot()
        XCTAssertEqual(first.status.queueSize, 4)
        XCTAssertEqual(first.metrics.queueDelta, 0)
        XCTAssertEqual(first.metrics.txApprovedDelta, 0)
        XCTAssertFalse(first.metrics.hasActivity)

        let second = try await client.getStatusSnapshot()
        XCTAssertEqual(second.status.queueSize, 11)
        XCTAssertEqual(second.metrics.queueDelta, 7)
        XCTAssertEqual(second.metrics.txApprovedDelta, 4)
        XCTAssertEqual(second.metrics.txRejectedDelta, 2)
        XCTAssertEqual(second.metrics.viewChangeDelta, 2)
        XCTAssertTrue(second.metrics.hasActivity)
    }

    func testStatusStateDropsStaleSamples() throws {
        func makePayload(queue: Int, approved: Int, rejected: Int, viewChanges: Int) throws -> ToriiStatusPayload {
            try ToriiStatusPayload(raw: [
                "peers": .number(Double(queue)),
                "queue_size": .number(Double(queue)),
                "commit_time_ms": .number(30),
                "txs_approved": .number(Double(approved)),
                "txs_rejected": .number(Double(rejected)),
                "view_changes": .number(Double(viewChanges))
            ])
        }

        var state = ToriiStatusState()
        let slow = try makePayload(queue: 4, approved: 5, rejected: 1, viewChanges: 0)
        let fast = try makePayload(queue: 6, approved: 7, rejected: 2, viewChanges: 1)
        let newer = try makePayload(queue: 8, approved: 10, rejected: 3, viewChanges: 1)

        let slowSequence = state.reserveSequence()
        let fastSequence = state.reserveSequence()

        let fastMetrics = state.record(fast, sequence: fastSequence)
        XCTAssertEqual(fastMetrics.queueDelta, 0)
        XCTAssertEqual(fastMetrics.txApprovedDelta, 0)

        let staleMetrics = state.record(slow, sequence: slowSequence)
        XCTAssertEqual(staleMetrics.queueDelta, 0)
        XCTAssertEqual(staleMetrics.txApprovedDelta, 0)
        XCTAssertEqual(staleMetrics.txRejectedDelta, 0)

        let newerSequence = state.reserveSequence()
        let newerMetrics = state.record(newer, sequence: newerSequence)
        XCTAssertEqual(newerMetrics.queueDelta, 2)
        XCTAssertEqual(newerMetrics.txApprovedDelta, 3)
        XCTAssertEqual(newerMetrics.txRejectedDelta, 1)
        XCTAssertEqual(newerMetrics.viewChangeDelta, 0)
    }

    func testStatusPayloadDecodesGovernanceSeals() throws {
        let payload = try ToriiStatusPayload(raw: [
            "peers": .number(2),
            "queue_size": .number(1),
            "commit_time_ms": .number(42),
            "txs_approved": .number(7),
            "txs_rejected": .number(1),
            "view_changes": .number(0),
            "lane_governance_sealed_total": .number(2),
            "lane_governance_sealed_aliases": .array([.string("public"), .string("payments")])
        ])

        XCTAssertEqual(payload.laneGovernanceSealedTotal, 2)
        XCTAssertEqual(payload.laneGovernanceSealedAliases, ["public", "payments"])
    }

    func testStatusPayloadDefaultsGovernanceSealsWhenMissing() throws {
        let payload = try ToriiStatusPayload(raw: [
            "peers": .number(1),
            "queue_size": .number(0),
            "commit_time_ms": .number(15),
            "txs_approved": .number(3),
            "txs_rejected": .number(0),
            "view_changes": .number(0)
        ])

        XCTAssertEqual(payload.laneGovernanceSealedTotal, 0)
        XCTAssertTrue(payload.laneGovernanceSealedAliases.isEmpty)
    }

    func testGetTimeStatusCompletion() {
        let expectation = expectation(description: "time-status")
        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"peers":0,"samples":[],"note":"empty"}
            """.data(using: .utf8)!
            return (response, body)
        }

        makeClient().getTimeStatus { result in
            switch result {
            case .success(let status):
                XCTAssertEqual(status.peers, 0)
                XCTAssertEqual(status.note, "empty")
            case .failure(let error):
                XCTFail("Unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testUploadAttachmentParsesMetadata() {
        let expectation = expectation(description: "upload")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/zk/attachments")
            XCTAssertEqual(request.httpMethod, "POST")
            let response = HTTPURLResponse(url: request.url!, statusCode: 201, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"id":"abc","content_type":"application/json","size":42,"created_ms":1234,"tenant":"token:xyz"}
            """.data(using: .utf8)!
            return (response, body)
        }

        makeClient().uploadAttachment(data: Data("test".utf8), contentType: "application/json") { result in
            switch result {
            case .success(let meta):
                XCTAssertEqual(meta.id, "abc")
                XCTAssertEqual(meta.content_type, "application/json")
                XCTAssertEqual(meta.size, 42)
                XCTAssertEqual(meta.created_ms, 1234)
                XCTAssertEqual(meta.tenant, "token:xyz")
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }

        waitForExpectations(timeout: 1)
    }

    func testListAttachmentsDecodesArray() {
        let expectation = expectation(description: "list attachments")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/zk/attachments")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"id":"one","content_type":"text/plain","size":1,"created_ms":1},{"id":"two","content_type":"application/json","size":2,"created_ms":2,"tenant":"anon"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        makeClient().listAttachments { result in
            switch result {
            case .success(let metas):
                XCTAssertEqual(metas.count, 2)
                XCTAssertEqual(metas[0].id, "one")
                XCTAssertNil(metas[0].tenant)
                XCTAssertEqual(metas[1].tenant, "anon")
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }

        waitForExpectations(timeout: 1)
    }

    func testListProverReportsAppliesFilterAndDecodes() {
        let expectation = expectation(description: "list prover reports")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/zk/prover/reports")
            let comps = request.url.flatMap { URLComponents(url: $0, resolvingAgainstBaseURL: false) }
            let items = comps?.queryItems ?? []
            let dict = Dictionary(uniqueKeysWithValues: items.map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(dict["ok_only"], "true")
            XCTAssertEqual(dict["content_type"], "application/json")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"id":"abc","ok":true,"content_type":"application/json","size":10,"created_ms":1,"processed_ms":2,"latency_ms":1,"zk1_tags":["TEST"]}]
            """.data(using: .utf8)!
            return (response, body)
        }

        var filter = ToriiProverReportsFilter()
        filter.okOnly = true
        filter.contentType = "application/json"

        makeClient().listProverReports(filter: filter) { result in
            switch result {
            case .success(let reports):
                XCTAssertEqual(reports.count, 1)
                XCTAssertEqual(reports.first?.zk1_tags ?? [], ["TEST"])
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }

        waitForExpectations(timeout: 1)
    }

    func testCountProverReportsRejectsFractionalCount() {
        let expectation = expectation(description: "count prover reports")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/zk/prover/reports/count")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"count":1.5}
            """.data(using: .utf8)!
            return (response, body)
        }

        makeClient().countProverReports { result in
            switch result {
            case .success:
                XCTFail("expected failure for fractional count")
            case .failure(let error):
                guard case ToriiClientError.invalidPayload = error else {
                    XCTFail("unexpected error: \(error)")
                    break
                }
            }
            expectation.fulfill()
        }

        waitForExpectations(timeout: 1)
    }

    func testGetProverReportEncodesId() {
        let expectation = expectation(description: "get prover report")
        let reportId = "report/1"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/zk/prover/reports/report%2F1")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"id":"report/1","ok":true,"content_type":"application/json","size":10,"created_ms":1,"processed_ms":2}
            """.data(using: .utf8)!
            return (response, body)
        }

        makeClient().getProverReport(id: reportId) { result in
            switch result {
            case .success(let report):
                XCTAssertEqual(report.id, reportId)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testDeleteProverReportEncodesId() {
        let expectation = expectation(description: "delete prover report")
        let reportId = "report/1"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/zk/prover/reports/report%2F1")
            XCTAssertEqual(request.httpMethod, "DELETE")
            let response = HTTPURLResponse(url: request.url!, statusCode: 204, httpVersion: nil, headerFields: nil)!
            return (response, Data())
        }

        makeClient().deleteProverReport(id: reportId) { result in
            if case let .failure(error) = result {
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testRegisterContractCodePostsJSON() {
        let expectation = expectation(description: "register contract")
        let codeHash = String(repeating: "a", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/code")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            guard let body = self.bodyData(from: request),
                  let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
                XCTFail("missing JSON body")
                throw NSError(domain: "stub", code: -1)
            }
            XCTAssertEqual(json["authority"] as? String, "alice@wonderland")
            XCTAssertEqual(json["private_key"] as? String, "ed25519:secret")
            let manifest = json["manifest"] as? [String: Any]
            XCTAssertEqual(manifest?["code_hash"] as? String, codeHash)
            let hints = manifest?["access_set_hints"] as? [String: Any]
            XCTAssertEqual(hints?["read_keys"] as? [String], ["account:alice#wonderland"])
            XCTAssertEqual(hints?["write_keys"] as? [String], ["asset:coin#wonderland"])
            let response = HTTPURLResponse(url: request.url!, statusCode: 202, httpVersion: nil, headerFields: nil)!
            return (response, Data())
        }

        let manifest = ToriiRegisterContractCodeRequest.Manifest(
            codeHash: codeHash,
            accessSetHints: ToriiContractAccessSetHints(
                readKeys: ["account:alice#wonderland"],
                writeKeys: ["asset:coin#wonderland"]
            )
        )
        let requestBody = ToriiRegisterContractCodeRequest(authority: "alice@wonderland",
                                                           privateKey: "ed25519:secret",
                                                           manifest: manifest)
        makeClient().registerContractCode(requestBody) { result in
            if case .failure(let error) = result {
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testGetAttachmentEncodesId() {
        let expectation = expectation(description: "get attachment")
        let attachmentId = "abc/def"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/zk/attachments/abc%2Fdef")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/octet-stream"])!
            return (response, Data([0x01]))
        }

        makeClient().getAttachment(id: attachmentId) { result in
            switch result {
            case .success(let payload):
                XCTAssertEqual(payload.0, Data([0x01]))
                XCTAssertEqual(payload.1, "application/octet-stream")
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testDeleteAttachmentEncodesId() {
        let expectation = expectation(description: "delete attachment")
        let attachmentId = "abc/def"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/zk/attachments/abc%2Fdef")
            XCTAssertEqual(request.httpMethod, "DELETE")
            let response = HTTPURLResponse(url: request.url!, statusCode: 204, httpVersion: nil, headerFields: nil)!
            return (response, Data())
        }

        makeClient().deleteAttachment(id: attachmentId) { result in
            if case let .failure(error) = result {
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testRegisterContractCodeRejectsInvalidCodeHash() {
        let manifest = ToriiRegisterContractCodeRequest.Manifest(codeHash: "abc")
        let requestBody = ToriiRegisterContractCodeRequest(authority: "alice@wonderland",
                                                           privateKey: "ed25519:secret",
                                                           manifest: manifest)
        XCTAssertThrowsError(try JSONEncoder().encode(requestBody)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testFetchContractManifestParsesResponse() {
        let expectation = expectation(description: "fetch manifest")
        let codeHash = String(repeating: "b", count: 64)
        let abiHash = String(repeating: "c", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/code/\(codeHash)")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"manifest":{"code_hash":"\(codeHash)","abi_hash":"\(abiHash)","compiler_fingerprint":"rustc","features_bitmap":1,"access_set_hints":{"read_keys":["account:alice#wonderland"],"write_keys":[]}},"code_bytes":null}
            """.data(using: .utf8)!
            return (response, body)
        }

        makeClient().fetchContractManifest(codeHashHex: codeHash) { result in
            switch result {
            case .success(let record):
                XCTAssertEqual(record.manifest.codeHash, codeHash)
                XCTAssertEqual(record.manifest.abiHash, abiHash)
                XCTAssertEqual(record.manifest.accessSetHints?.readKeys, ["account:alice#wonderland"])
                XCTAssertEqual(record.manifest.accessSetHints?.writeKeys ?? [], [])
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testDeployContractParsesResponse() {
        let expectation = expectation(description: "deploy contract")
        let codeHash = String(repeating: "d", count: 64)
        let abiHash = String(repeating: "e", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/deploy")
            XCTAssertEqual(request.httpMethod, "POST")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"ok":true,"code_hash_hex":"\(codeHash)","abi_hash_hex":"\(abiHash)"}
            """.data(using: .utf8)!
            return (response, body)
        }

        let req = ToriiDeployContractRequest(authority: "alice@wonderland",
                                             privateKey: "ed25519:secret",
                                             codeB64: "AQ==")
        makeClient().deployContract(req) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
                XCTAssertEqual(response.codeHashHex, codeHash)
                XCTAssertEqual(response.abiHashHex, abiHash)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testDeployContractRejectsInvalidBase64() {
        let request = ToriiDeployContractRequest(authority: "alice@wonderland",
                                                 privateKey: "ed25519:secret",
                                                 codeB64: "%%%")
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testDeployContractRejectsUnsupportedFields() {
        let request = ToriiDeployContractRequest(authority: "alice@wonderland",
                                                 privateKey: "ed25519:secret",
                                                 codeB64: "AQ==",
                                                 codeHash: String(repeating: "a", count: 64))
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testDeployContractInstanceParsesResponse() {
        let expectation = expectation(description: "deploy contract instance")
        let codeHash = String(repeating: "f", count: 64)
        let abiHash = String(repeating: "0", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/instance")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            guard let body = self.bodyData(from: request),
                  let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
                XCTFail("missing JSON body")
                throw NSError(domain: "stub", code: -1)
            }
            XCTAssertEqual(json["authority"] as? String, "alice@wonderland")
            XCTAssertEqual(json["private_key"] as? String, "ed25519:secret")
            XCTAssertEqual(json["namespace"] as? String, "apps")
            XCTAssertEqual(json["contract_id"] as? String, "calc.v1")
            XCTAssertEqual(json["code_b64"] as? String, "AQ==")
            let manifest = json["manifest"] as? [String: Any]
            XCTAssertEqual(manifest?["compiler_fingerprint"] as? String, "kotodama-0.8")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"namespace":"apps","contract_id":"calc.v1","code_hash_hex":"\(codeHash)","abi_hash_hex":"\(abiHash)"}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let manifest = ToriiContractManifest(compilerFingerprint: "kotodama-0.8")
        let req = ToriiDeployContractInstanceRequest(authority: "alice@wonderland",
                                                     privateKey: "ed25519:secret",
                                                     namespace: "apps",
                                                     contractId: "calc.v1",
                                                     codeB64: "AQ==",
                                                     manifest: manifest)
        makeClient().deployContractInstance(req) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
                XCTAssertEqual(response.namespace, "apps")
                XCTAssertEqual(response.contractId, "calc.v1")
                XCTAssertEqual(response.codeHashHex, codeHash)
                XCTAssertEqual(response.abiHashHex, abiHash)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testActivateContractInstanceParsesResponse() {
        let expectation = expectation(description: "activate contract instance")
        let codeHash = String(repeating: "1", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/instance/activate")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            guard let body = self.bodyData(from: request),
                  let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
                XCTFail("missing JSON body")
                throw NSError(domain: "stub", code: -1)
            }
            XCTAssertEqual(json["authority"] as? String, "alice@wonderland")
            XCTAssertEqual(json["private_key"] as? String, "ed25519:secret")
            XCTAssertEqual(json["namespace"] as? String, "apps")
            XCTAssertEqual(json["contract_id"] as? String, "calc.v1")
            XCTAssertEqual(json["code_hash"] as? String, codeHash)
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let req = ToriiActivateContractInstanceRequest(authority: "alice@wonderland",
                                                       privateKey: "ed25519:secret",
                                                       namespace: "apps",
                                                       contractId: "calc.v1",
                                                       codeHash: codeHash)
        makeClient().activateContractInstance(req) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testFetchContractCodeBytesDecodesResponse() {
        let expectation = expectation(description: "fetch code bytes")
        let codeHash = String(repeating: "2", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/code-bytes/\(codeHash)")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"code_b64":"AAAA"}
            """.data(using: .utf8)!
            return (response, body)
        }

        makeClient().fetchContractCodeBytes(codeHashHex: codeHash) { result in
            switch result {
            case .success(let record):
                XCTAssertEqual(record.codeB64, "AAAA")
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testFetchContractCodeBytesRejectsInvalidBase64() {
        let expectation = expectation(description: "fetch code bytes invalid b64")
        let codeHash = String(repeating: "2", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/code-bytes/\(codeHash)")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"code_b64":"%%%"}
            """.data(using: .utf8)!
            return (response, body)
        }

        makeClient().fetchContractCodeBytes(codeHashHex: codeHash) { result in
            switch result {
            case .success:
                XCTFail("expected invalid base64 decoding failure")
            case .failure(let error):
                guard case ToriiClientError.decoding = error else {
                    return XCTFail("unexpected error: \(error)")
                }
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testSubmitTransactionPostsNorito() {
        let expectation = expectation(description: "submit transaction")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-norito")
            XCTAssertEqual(self.bodyData(from: request), Data([0x01, 0x02]))
            let response = HTTPURLResponse(url: request.url!, statusCode: 202, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"hash":"abc","accepted":true}
            """.data(using: .utf8)!
            return (response, body)
        }

        makeClient().submitTransaction(data: Data([0x01, 0x02])) { result in
            switch result {
            case .success(let payload):
                XCTAssertEqual(payload?.hash, "abc")
                XCTAssertEqual(payload?.accepted, true)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testSubmitGovernanceDeployContractProposal() {
        let expectation = expectation(description: "governance proposal")
        let proposalId = String(repeating: "3", count: 64)
        let codeHash = String(repeating: "4", count: 64)
        let abiHash = String(repeating: "5", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/gov/proposals/deploy-contract")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            let body = self.bodyJSON(from: request)
            XCTAssertEqual(body["namespace"] as? String, "apps")
            XCTAssertEqual(body["contract_id"] as? String, "demo.contract")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let payload = """
            {"ok":true,"proposal_id":"\(proposalId)","tx_instructions":[{"wire_id":"FinalizeReferendum","payload_hex":"00ff"}]}
            """.data(using: .utf8)!
            return (response, payload)
        }

        let request = ToriiGovernanceDeployContractProposalRequest(namespace: "apps",
                                                                   contractId: "demo.contract",
                                                                   codeHashHex: codeHash,
                                                                   abiHashHex: abiHash,
                                                                   abiVersion: "1",
                                                                   window: ToriiGovernanceWindow(lower: 10, upper: 20))
        makeClient().submitGovernanceDeployContractProposal(request) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
                XCTAssertEqual(response.proposalId, proposalId)
                XCTAssertEqual(response.txInstructions.first?.wireId, "FinalizeReferendum")
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testFinalizeGovernanceEncodesProposalId() {
        let proposalId = String(repeating: "6", count: 64)
        let request = ToriiGovernanceFinalizeRequest(referendumId: "ref-1", proposalId: proposalId)
        do {
            let data = try JSONEncoder().encode(request)
            guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                return XCTFail("missing JSON body")
            }
            XCTAssertEqual(json["referendum_id"] as? String, "ref-1")
            XCTAssertEqual(json["proposal_id"] as? String, proposalId)
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    func testEnactGovernanceEncodesProposalIdAndPreimage() {
        let proposalId = String(repeating: "7", count: 64)
        let preimage = String(repeating: "8", count: 64)
        let request = ToriiGovernanceEnactRequest(proposalId: proposalId,
                                                  preimageHash: preimage,
                                                  window: ToriiGovernanceWindow(lower: 10, upper: 20))
        do {
            let data = try JSONEncoder().encode(request)
            guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                return XCTFail("missing JSON body")
            }
            XCTAssertEqual(json["proposal_id"] as? String, proposalId)
            XCTAssertEqual(json["preimage_hash"] as? String, preimage)
            let window = json["window"] as? [String: Any]
            XCTAssertEqual(window?["lower"] as? Int, 10)
            XCTAssertEqual(window?["upper"] as? Int, 20)
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    func testSubmitGovernanceZkBallotEncodesPublicInputs() {
        let expectation = expectation(description: "zk ballot")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/gov/ballots/zk")
            let body = self.bodyJSON(from: request)
            XCTAssertEqual(body["authority"] as? String, "alice@wonderland")
            XCTAssertEqual(body["chain_id"] as? String, "chain")
            XCTAssertEqual(body["election_id"] as? String, "election-1")
            let publicInputs = body["public"] as? [String: Any]
            XCTAssertEqual(publicInputs?["foo"] as? String, "bar")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let payload = """
            {"ok":true,"accepted":true,"reason":null,"tx_instructions":[]}
            """.data(using: .utf8)!
            return (response, payload)
        }

        let request = ToriiGovernanceZkBallotRequest(authority: "alice@wonderland",
                                                     chainId: "chain",
                                                     electionId: "election-1",
                                                     proofB64: "AAAA",
                                                     publicInputs: ["foo": .string("bar")])
        makeClient().submitGovernanceZkBallot(request) { result in
            if case .failure(let error) = result {
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testSubmitGovernanceZkBallotRejectsIncompleteLockHints() throws {
        let owner = try canonicalOwnerLiteral()
        let request = ToriiGovernanceZkBallotRequest(authority: "alice@wonderland",
                                                     chainId: "chain",
                                                     electionId: "election-1",
                                                     proofB64: "AAAA",
                                                     publicInputs: ["owner": .string(owner)])
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case let ToriiClientError.invalidPayload(message) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertTrue(message.contains("owner, amount, and duration_blocks"))
        }
    }

    func testSubmitGovernanceZkBallotRejectsDeprecatedPublicInputs() throws {
        let owner = try canonicalOwnerLiteral()
        let request = ToriiGovernanceZkBallotRequest(authority: "alice@wonderland",
                                                     chainId: "chain",
                                                     electionId: "election-1",
                                                     proofB64: "AAAA",
                                                     publicInputs: [
                                                        "owner": .string(owner),
                                                        "amount": .string("250"),
                                                        "durationBlocks": .number(12),
                                                        "rootHintHex": .string("0x\(String(repeating: "Aa", count: 32))"),
                                                        "nullifierHex": .string("blake2b32:\(String(repeating: "BB", count: 32))"),
                                                     ])
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case let ToriiClientError.invalidPayload(message) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertTrue(message.contains("durationBlocks"))
        }
    }

    func testSubmitGovernanceZkBallotNormalizesPublicInputs() throws {
        let owner = try canonicalOwnerLiteral()
        let request = ToriiGovernanceZkBallotRequest(authority: "alice@wonderland",
                                                     chainId: "chain",
                                                     electionId: "election-1",
                                                     proofB64: "AAAA",
                                                     publicInputs: [
                                                        "owner": .string(owner),
                                                        "amount": .string("250"),
                                                        "duration_blocks": .number(12),
                                                        "root_hint": .string("0x\(String(repeating: "Cc", count: 32))"),
                                                        "nullifier": .string("blake2b32:\(String(repeating: "DD", count: 32))"),
                                                     ])
        let data = try JSONEncoder().encode(request)
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let publicInputs = json["public"] as? [String: Any] else {
            return XCTFail("missing public inputs")
        }
        XCTAssertEqual(publicInputs["root_hint"] as? String, String(repeating: "cc", count: 32))
        XCTAssertEqual(publicInputs["nullifier"] as? String, String(repeating: "dd", count: 32))
    }

    func testSubmitGovernanceZkBallotRejectsInvalidHexHints() throws {
        let owner = try canonicalOwnerLiteral()
        let request = ToriiGovernanceZkBallotRequest(authority: "alice@wonderland",
                                                     chainId: "chain",
                                                     electionId: "election-1",
                                                     proofB64: "AAAA",
                                                     publicInputs: [
                                                        "owner": .string(owner),
                                                        "amount": .string("250"),
                                                        "duration_blocks": .number(12),
                                                        "root_hint": .string("not-hex"),
                                                     ])
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case let ToriiClientError.invalidPayload(message) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertTrue(message.contains("root_hint"))
        }
    }

    func testSubmitGovernanceZkBallotRejectsNoncanonicalOwner() throws {
        let owner = try noncanonicalOwnerLiteral()
        let request = ToriiGovernanceZkBallotRequest(authority: "alice@wonderland",
                                                     chainId: "chain",
                                                     electionId: "election-1",
                                                     proofB64: "AAAA",
                                                     publicInputs: [
                                                        "owner": .string(owner),
                                                        "amount": .string("250"),
                                                        "duration_blocks": .number(12),
                                                     ])
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case let ToriiClientError.invalidPayload(message) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertTrue(message.contains("owner must use canonical account id form"))
        }
    }

    func testGetGovernanceProposalDecodesRecord() {
        let expectation = expectation(description: "proposal get")
        let proposalId = String(repeating: "6", count: 64)
        let codeHash = String(repeating: "7", count: 64)
        let abiHash = String(repeating: "8", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/gov/proposals/\(proposalId)")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let payload = """
            {"found":true,"proposal":{"proposer":"alice@wonderland","kind":{"DeployContract":{"namespace":"apps","contract_id":"demo","code_hash_hex":"\(codeHash)","abi_hash_hex":"\(abiHash)","abi_version":"1"}},"created_height":42,"status":"Approved"}}
            """.data(using: .utf8)!
            return (response, payload)
        }

        makeClient().getGovernanceProposal(idHex: proposalId) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.found)
                XCTAssertEqual(response.proposal?.createdHeight, 42)
                guard case .deployContract(let payload) = response.proposal?.kind else {
                    return XCTFail("expected deploy contract kind")
                }
                XCTAssertEqual(payload.contractId, "demo")
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testGetGovernanceUnlockStatsAddsQuery() {
        let expectation = expectation(description: "unlock stats")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/gov/unlocks/stats")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let params = components?.queryItems?.reduce(into: [String: String]()) { result, item in
                if let value = item.value {
                    result[item.name] = value
                }
            }
            XCTAssertEqual(params?["height"], "120")
            XCTAssertEqual(params?["referendum_id"], "ref-1")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let payload = """
            {"height_current":120,"expired_locks_now":2,"referenda_with_expired":1,"last_sweep_height":100}
            """.data(using: .utf8)!
            return (response, payload)
        }

        makeClient().getGovernanceUnlockStats(height: 120, referendumId: "ref-1") { result in
            switch result {
            case .success(let stats):
                XCTAssertEqual(stats.expiredLocksNow, 2)
                XCTAssertEqual(stats.referendaWithExpired, 1)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testGetTransactionStatusFetchesJSON() {
        let expectation = expectation(description: "status")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            XCTAssertEqual(components?.queryItems?.first?.value, "deadbeef")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"kind":"Transaction","content":{"hash":"deadbeef","status":{"kind":"Committed","content":null}}}
            """.data(using: .utf8)!
            return (response, body)
        }

        makeClient().getTransactionStatus(hashHex: "deadbeef") { result in
            switch result {
            case .success(let status):
                XCTAssertEqual(status?.kind, "Transaction")
                XCTAssertEqual(status?.content.status.kind, "Committed")
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testPipelineStatusStateMapping() throws {
        let json = """
        {"kind":"Transaction","content":{"hash":"deadbeef","status":{"kind":"Committed","content":null}}}
        """.data(using: .utf8)!
        let decoded = try JSONDecoder().decode(ToriiPipelineTransactionStatus.self, from: json)
        XCTAssertEqual(decoded.content.status.state, .committed)
        XCTAssertTrue(decoded.content.status.state.isKnownTerminalSuccess)
        let other = PipelineTransactionState(kind: "CustomStatus")
        if case let .other(value) = other {
            XCTAssertEqual(value, "CustomStatus")
        } else {
            XCTFail("Expected other state")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetHealthReturnsText() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/health")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/plain")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "text/plain"])!
            return (response, Data("Healthy".utf8))
        }
        let text = try await makeClient().getHealth()
        XCTAssertEqual(text, "Healthy")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetMetricsAsText() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/metrics")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/plain")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "text/plain"])!
            return (response, Data("metric_total 1\n".utf8))
        }
        let result = try await makeClient().getMetrics(asText: true)
        let expected = ToriiMetricsResponse.text("metric_total 1\n")
        XCTAssertEqual(result, expected)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetMetricsPrefersJSON() async throws {
        let body = #"{"ok":true}"#.data(using: .utf8)!
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/metrics")
            XCTAssertNil(request.value(forHTTPHeaderField: "Accept"))
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, body)
        }
        let result = try await makeClient().getMetrics()
        let expectedJSON = ToriiJSONValue.object(["ok": .bool(true)])
        XCTAssertEqual(result, ToriiMetricsResponse.json(expectedJSON))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetMetricsParsesJSONWithoutHeader() async throws {
        let body = #"{"ok":true}"#.data(using: .utf8)!
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/metrics")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: [:])!
            return (response, body)
        }
        let result = try await makeClient().getMetrics()
        let expectedJSON = ToriiJSONValue.object(["ok": .bool(true)])
        XCTAssertEqual(result, ToriiMetricsResponse.json(expectedJSON))
    }
}

final class ToriiClientIntegrationTests: XCTestCase {
    private var mock: ToriiMockProcess?

    override func setUpWithError() throws {
        try super.setUpWithError()
        guard let server = ToriiMockProcess() else {
            throw XCTSkip("python interpreter not available for Torii mock")
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
        let session = URLSession(configuration: .ephemeral)
        let client = ToriiClient(baseURL: mock.baseURL, session: session)
        let payload = Data("{\"hello\":\"swift\"}".utf8)

        var attachmentId: String?
        let uploadExpectation = expectation(description: "upload")
        client.uploadAttachment(data: payload, contentType: "application/json") { result in
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
        client.listAttachments { result in
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
        client.getAttachment(id: id) { result in
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
        client.deleteAttachment(id: id) { result in
            if case let .failure(error) = result {
                XCTFail("delete failed: \(error)")
            }
            deleteExpectation.fulfill()
        }
        wait(for: [deleteExpectation], timeout: 5)

        let listAfterExpectation = expectation(description: "list after")
        client.listAttachments { result in
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
        let client = ToriiClient(baseURL: mock.baseURL, session: URLSession(configuration: .ephemeral))

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
        let scenarioHash = "feedfacecafebeefcafedeadbeef0001"
        try await preparePipelineScenario(.success, hashHex: scenarioHash)
        let mock = try XCTUnwrap(self.mock)
        let session = URLSession(configuration: .ephemeral)
        let client = ToriiClient(baseURL: mock.baseURL, session: session)
        let sdk = IrohaSDK(toriiClient: client)
        sdk.pipelineSubmitOptions = PipelineSubmitOptions(maxRetries: 0,
                                                          initialBackoffSeconds: 0,
                                                          backoffMultiplier: 1)
        sdk.pipelinePollOptions = PipelineStatusPollOptions(pollInterval: 0.01, timeout: 1)
        let envelope = try tcMakePipelineEnvelope(hashHex: scenarioHash, marker: 0x11)
        let status = try await sdk.submitAndWait(envelope: envelope)
        XCTAssertEqual(status.content.hash, scenarioHash)
        XCTAssertTrue(PipelineStatusPollOptions.defaultSuccessStates.contains(status.content.status.state))
        XCTAssertTrue(status.content.status.state.isKnownTerminalSuccess)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testPipelineSubmitAndWaitFailureAgainstMock() async throws {
        let scenarioHash = "feedfacecafebeefcafedeadbeef0002"
        try await preparePipelineScenario(.failure, hashHex: scenarioHash)
        let mock = try XCTUnwrap(self.mock)
        let session = URLSession(configuration: .ephemeral)
        let client = ToriiClient(baseURL: mock.baseURL, session: session)
        let sdk = IrohaSDK(toriiClient: client)
        sdk.pipelineSubmitOptions = PipelineSubmitOptions(maxRetries: 0,
                                                          initialBackoffSeconds: 0,
                                                          backoffMultiplier: 1)
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
            XCTAssertEqual(payload.content.hash, scenarioHash)
            XCTAssertEqual(payload.content.status.kind, "Rejected")
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testPipelineSubmitAndWaitTimeoutAgainstMock() async throws {
        let scenarioHash = "feedfacecafebeefcafedeadbeef0003"
        try await preparePipelineScenario(.timeout,
                                          hashHex: scenarioHash,
                                          statusKinds: ["Queued"],
                                          repeatLast: true)
        let mock = try XCTUnwrap(self.mock)
        let session = URLSession(configuration: .ephemeral)
        let client = ToriiClient(baseURL: mock.baseURL, session: session)
        let sdk = IrohaSDK(toriiClient: client)
        sdk.pipelineSubmitOptions = PipelineSubmitOptions(maxRetries: 0,
                                                          initialBackoffSeconds: 0,
                                                          backoffMultiplier: 1)
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
        let chunkPlanObject: [String: Any] = ["chunks": [["index": 0, "size": 262_144]]]
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

    @available(iOS 15.0, macOS 12.0, *)
    func testFetchDaPayloadViaGatewayUsesInjectedOrchestrator() async throws {
        let ticket = String(repeating: "e", count: 64)
        let manifestPayload = Data([0xAA, 0xBB, 0xCC])
        let manifestObject: [String: Any] = ["chunker_handle": "demo.chunker@2.1.0"]
        let chunkPlanObject: [String: Any] = ["chunks": [["index": 0, "size": 1]]]
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
        var submission = ToriiDaBlobSubmission(
            payload: Data("payload".utf8),
            laneId: 9,
            epoch: 4,
            sequence: 2,
            metadata: [
                ToriiDaMetadataEntry(key: "da.stream", value: Data("demo".utf8))
            ],
            clientBlobId: digest,
            privateKeyHex: String(repeating: "11", count: 32)
        )
        submission.codec = "application/octet-stream"

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/da/ingest")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            let body = tcBodyJSON(from: request)
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
        guard let rentQuote = receipt.rentQuote else {
            return XCTFail("missing rent quote")
        }
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
            "chunk_plan": .array([
                .object([
                    "chunk_index": .number(0),
                    "offset": .number(0),
                    "length": .number(4),
                    "digest_blake3": .string(String(repeating: "ee", count: 32))
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
        guard let manifestData = Data(hexString: manifestHex) else {
            throw XCTSkip("failed to decode DA manifest fixture")
        }
        let payloadData = try Data(contentsOf: payloadURL)
        let manifestJSONData = try Data(contentsOf: manifestJSONURL)
        guard
            let manifestObject = try JSONSerialization.jsonObject(with: manifestJSONData) as? [String: Any],
            let blobArray = manifestObject["blob_hash"] as? [[NSNumber]],
            let blobBytes = blobArray.first
        else {
            throw XCTSkip("blob_hash fixture missing")
        }
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
            chunkRootsHex: [],
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
