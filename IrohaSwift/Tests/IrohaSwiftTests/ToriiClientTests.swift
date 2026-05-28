import XCTest
import CryptoKit
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

#if os(macOS)
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
#endif

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
    case invalidPayloadEncoding
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
    private let encodedRoseAssetID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
    private let roseAssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"

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

    private func assertDecodedPath(_ request: URLRequest, contains expected: String, line: UInt = #line) {
        XCTAssertTrue(
            request.url?.path.contains(expected) == true,
            "expected decoded path to contain \(expected), got \(request.url?.path ?? "<nil>")",
            line: line
        )
    }

    private func irohaSwiftPackageRootURL() -> URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // ToriiClientTests.swift
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
    }

    private func makeClient(baseURL: URL = URL(string: "https://example.test")!) -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        return ToriiClient(baseURL: baseURL, session: session)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testAuthenticationContextAddsWalletHeadersToEveryRequest() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let client = ToriiClient(
            baseURL: URL(string: "https://example.test")!,
            session: session,
            authentication: try ToriiClientAuthentication.bearerToken(
                " wallet-token ",
                accountId: " sora-account-1 ",
                dataspaceId: " mibank.paynet ",
                additionalHeaders: ["X-Trace-Id": "trace-123"]
            )
        )
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/health")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer wallet-token")
            XCTAssertEqual(request.value(forHTTPHeaderField: "X-Account-Id"), "sora-account-1")
            XCTAssertEqual(request.value(forHTTPHeaderField: "X-Dataspace-Id"), "mibank.paynet")
            XCTAssertEqual(request.value(forHTTPHeaderField: "X-Trace-Id"), "trace-123")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/plain")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "text/plain"])!
            return (response, Data("ok".utf8))
        }

        let health = try await client.getHealth()
        XCTAssertEqual(health, "ok")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testAuthenticationContextPreservesJsonRequestHeaders() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let client = ToriiClient(
            baseURL: URL(string: "https://example.test")!,
            session: session,
            authentication: try ToriiClientAuthentication.authorizationHeader(
                "Bearer wallet-token",
                accountId: "sora-account-1",
                dataspaceId: "mibank.paynet"
            )
        )
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/aliases/resolve")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer wallet-token")
            XCTAssertEqual(request.value(forHTTPHeaderField: "X-Account-Id"), "sora-account-1")
            XCTAssertEqual(request.value(forHTTPHeaderField: "X-Dataspace-Id"), "mibank.paynet")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 404,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, Data())
        }

        let resolved = try await client.resolveAccountAlias("missing")
        XCTAssertNil(resolved)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testAuthenticationContextRejectsInsecureTransport() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let client = ToriiClient(
            baseURL: URL(string: "http://example.test")!,
            session: session,
            authentication: try ToriiClientAuthentication.bearerToken(
                "wallet-token",
                accountId: "sora-account-1",
                dataspaceId: "mibank.paynet"
            )
        )

        await XCTAssertThrowsErrorAsync(try await client.getHealth()) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                XCTFail("Expected invalidPayload, got \(error)")
                return
            }
            XCTAssertTrue(reason.contains("refuses insecure transport"))
        }
    }

    func testAuthenticationContextRequiresNonEmptyTokenAndAccount() throws {
        XCTAssertThrowsError(
            try ToriiClientAuthentication.bearerToken(" ", accountId: "sora-account-1")
        )
        XCTAssertThrowsError(
            try ToriiClientAuthentication.bearerToken("wallet-token", accountId: " ")
        )
    }

    private func nodeCapabilitiesBody(
        dataModelVersion: Int = ToriiNodeCapabilities.expectedDataModelVersion,
        signedTransactionSchemaHashHex: String? = ToriiNodeCapabilities.expectedSignedTransactionSchemaHashHex
    ) -> Data {
        var payload: [String: Any] = [
            "abi_version": 1,
            "data_model_version": dataModelVersion
        ]
        if let signedTransactionSchemaHashHex {
            payload["signed_transaction_schema_hash_hex"] = signedTransactionSchemaHashHex
        }
        return (try? JSONSerialization.data(withJSONObject: payload)) ?? Data()
    }

    private func canonicalOwnerLiteral(domain: String = "wonderland") throws -> String {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: 1, count: 32))
        let address = try AccountAddress.fromAccount(publicKey: keypair.publicKey)
        let i105 = try address.toI105(networkPrefix: 0x02F1)
        return i105
    }

    private func noncanonicalOwnerLiteral(domain: String = "wonderland") throws -> String {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: 2, count: 32))
        let address = try AccountAddress.fromAccount(publicKey: keypair.publicKey)
        let canonicalHex = try address.canonicalHex()
        return canonicalHex
    }

    private func makeSignedIdentifierReceiptPayload(policyId: String = "phone#retail",
                                                    accountId: String,
                                                    opaqueId: String,
                                                    receiptHash: String,
                                                    uaid: String,
                                                    backend: String,
                                                    programId: String = "identifier_lookup_retail",
                                                    programDigestHex: String = String(repeating: "11", count: 32),
                                                    inputCiphertextHashHex: String = String(repeating: "ab", count: 32),
                                                    outputCiphertextHashHex: String = String(repeating: "bb", count: 32),
                                                    parameterDigestHex: String = String(repeating: "cd", count: 32),
                                                    evaluationKeyDigestHex: String = String(repeating: "dd", count: 32),
                                                    outputHashHex: String = String(repeating: "22", count: 31) + "23",
                                                    associatedDataHashHex: String = String(repeating: "33", count: 32),
                                                    resolvedAtMs: UInt64 = 42,
                                                    expiresAtMs: UInt64? = 142,
                                                    openingSignatureHex: String = String(repeating: "ff", count: 64))
                                                    -> ToriiIdentifierResolutionPayload {
        ToriiIdentifierResolutionPayload(
            policyId: policyId,
            opaqueId: opaqueId,
            receiptHash: receiptHash,
            uaid: uaid,
            accountId: accountId,
            execution: ToriiIdentifierResolutionExecutionPayload(
                programId: programId,
                programDigest: programDigestHex,
                backend: backend,
                verificationMode: "signed",
                inputCiphertextHash: inputCiphertextHashHex,
                outputCiphertextHash: outputCiphertextHashHex,
                parameterDigest: parameterDigestHex,
                evaluationKeyDigest: evaluationKeyDigestHex,
                outputHash: outputHashHex,
                associatedDataHash: associatedDataHashHex,
                executedAtMs: resolvedAtMs,
                expiresAtMs: expiresAtMs
            ),
            opening: sampleOpening(
                programId: programId,
                inputCiphertextHash: inputCiphertextHashHex,
                outputCiphertextHash: outputCiphertextHashHex,
                parameterDigest: parameterDigestHex,
                evaluationKeyDigest: evaluationKeyDigestHex,
                openedOutputHash: outputHashHex,
                openedAtMs: resolvedAtMs,
                expiresAtMs: expiresAtMs,
                signatureHex: openingSignatureHex
            )
        )
    }

    private func sampleOpening(
        programId: String = "identifier_lookup_retail",
        inputCiphertextHash: String = String(repeating: "ab", count: 32),
        outputCiphertextHash: String = String(repeating: "bb", count: 32),
        parameterDigest: String = String(repeating: "cd", count: 32),
        evaluationKeyDigest: String = String(repeating: "dd", count: 32),
        openedOutputHash: String = String(repeating: "ee", count: 32),
        openedAtMs: UInt64 = 42,
        expiresAtMs: UInt64? = 142,
        signatureHex: String = String(repeating: "ff", count: 64)
    ) -> ToriiRamLfeOutputOpening {
        ToriiRamLfeOutputOpening(
            payload: ToriiRamLfeOutputOpeningPayload(
                programId: programId,
                inputCiphertextHash: inputCiphertextHash,
                outputCiphertextHash: outputCiphertextHash,
                parameterDigest: parameterDigest,
                evaluationKeyDigest: evaluationKeyDigest,
                openedOutputHash: openedOutputHash,
                openedAtMs: openedAtMs,
                expiresAtMs: expiresAtMs
            ),
            signature: signatureHex
        )
    }

    private func signedIdentifierReceiptFixture(
        payload: ToriiIdentifierResolutionPayload,
        canonicalPayloadBytes: Data? = nil
    ) throws -> (resolverPublicKey: String, signatureHex: String) {
        let privateKey = Curve25519.Signing.PrivateKey()
        let multihash = OfflineNorito.publicKeyMultihash(
            algorithm: .ed25519,
            payload: privateKey.publicKey.rawRepresentation
        )
        let payloadBytes: Data
        if let canonicalPayloadBytes {
            payloadBytes = canonicalPayloadBytes
        } else {
            payloadBytes = try ToriiIdentifierReceiptCanonicalEncoder.encodePayload(payload)
        }
        var digest = Blake2b.hash256(payloadBytes)
        digest[digest.count - 1] |= 0x01
        let signature = try privateKey.signature(for: digest)
        return (
            resolverPublicKey: "ed25519:\(multihash)",
            signatureHex: signature.hexUppercased()
        )
    }

    private func noritoField(_ payload: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(payload)
        return writer.data
    }

    private func legacyFlatBytesVec(_ bytes: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeLength(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }

    private func constVecBytes(_ bytes: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeLength(UInt64(bytes.count))
        for byte in bytes {
            writer.writeLength(1)
            writer.writeUInt8(byte)
        }
        return writer.data
    }

    private func readCompactLength(_ data: Data, offset: inout Int) throws -> Int {
        var shift = 0
        var result: UInt64 = 0
        while true {
            guard offset < data.count, shift < 64 else {
                throw TcHelperError.invalidPayloadEncoding
            }
            let byte = data[offset]
            offset += 1
            result |= UInt64(byte & 0x7F) << UInt64(shift)
            if byte & 0x80 == 0 {
                break
            }
            shift += 7
        }
        guard result <= UInt64(Int.max) else {
            throw TcHelperError.invalidPayloadEncoding
        }
        return Int(result)
    }

    private func noritoFieldRange(in data: Data, fieldIndex: Int) throws -> Range<Data.Index> {
        var offset = 0
        for index in 0...fieldIndex {
            let fieldStart = offset
            let fieldLength = try readCompactLength(data, offset: &offset)
            let fieldEnd = offset + fieldLength
            guard fieldEnd <= data.count else {
                throw TcHelperError.invalidPayloadEncoding
            }
            if index == fieldIndex {
                return fieldStart..<fieldEnd
            }
            offset = fieldEnd
        }
        throw TcHelperError.invalidPayloadEncoding
    }

    private func noritoFieldPayload(_ field: Data) throws -> Data {
        var offset = 0
        let fieldLength = try readCompactLength(field, offset: &offset)
        let fieldEnd = offset + fieldLength
        guard fieldEnd == field.count else {
            throw TcHelperError.invalidPayloadEncoding
        }
        return Data(field[offset..<fieldEnd])
    }

    private func payloadBytesWithLegacyFlatOpeningSignature(
        _ payload: ToriiIdentifierResolutionPayload
    ) throws -> Data {
        let signatureHex = payload.opening.signature.trimmingCharacters(in: .whitespacesAndNewlines)
        let normalizedSignatureHex: String
        if signatureHex.hasPrefix("0x") || signatureHex.hasPrefix("0X") {
            normalizedSignatureHex = String(signatureHex.dropFirst(2))
        } else {
            normalizedSignatureHex = signatureHex
        }
        guard let signatureBytes = Data(hexString: normalizedSignatureHex) else {
            throw TcHelperError.invalidHashEncoding
        }

        var canonicalPayload = try ToriiIdentifierReceiptCanonicalEncoder.encodePayload(payload)
        let openingFieldRange = try noritoFieldRange(in: canonicalPayload, fieldIndex: 2)
        let openingField = Data(canonicalPayload[openingFieldRange])
        var openingPayload = try noritoFieldPayload(openingField)
        let signatureFieldRange = try noritoFieldRange(in: openingPayload, fieldIndex: 1)
        openingPayload.replaceSubrange(
            signatureFieldRange,
            with: noritoField(legacyFlatBytesVec(signatureBytes))
        )

        canonicalPayload.replaceSubrange(openingFieldRange, with: noritoField(openingPayload))
        return canonicalPayload
    }

    private func openingSignaturePayload(in encodedPayload: Data) throws -> Data {
        let openingFieldRange = try noritoFieldRange(in: encodedPayload, fieldIndex: 2)
        let openingField = Data(encodedPayload[openingFieldRange])
        let openingPayload = try noritoFieldPayload(openingField)
        let signatureFieldRange = try noritoFieldRange(in: openingPayload, fieldIndex: 1)
        let signatureField = Data(openingPayload[signatureFieldRange])
        return try noritoFieldPayload(signatureField)
    }

    private func identifierReceiptJSON(
        payload: ToriiIdentifierResolutionPayload,
        signatureHex: String
    ) throws -> String {
        let payloadJSON = try XCTUnwrap(
            String(data: JSONEncoder().encode(payload), encoding: .utf8)
        )
        return """
        {
          "payload":\(payloadJSON),
          "attestation":{
            "kind":"signed",
            "signature":"\(signatureHex)"
          }
        }
        """
    }

    private func identifierReceipt(
        payload: ToriiIdentifierResolutionPayload,
        signatureHex: String
    ) throws -> ToriiIdentifierResolutionReceipt {
        try JSONDecoder().decode(
            ToriiIdentifierResolutionReceipt.self,
            from: Data(try identifierReceiptJSON(payload: payload, signatureHex: signatureHex).utf8)
        )
    }

    private func identifierPolicy(
        policyId: String = "phone#retail",
        owner: String,
        resolverPublicKey: String,
        active: Bool = true,
        backend: String = "bfv-affine-sha3-256-v1"
    ) -> ToriiIdentifierPolicySummary {
        ToriiIdentifierPolicySummary(
            policyId: policyId,
            owner: owner,
            active: active,
            normalization: .phoneE164,
            resolverPublicKey: resolverPublicKey,
            backend: backend,
            inputEncryption: "bfv-v1",
            inputEncryptionPublicParameters: nil,
            inputEncryptionPublicParametersDecoded: nil,
            ramFheProfile: nil,
            proofVerifier: nil,
            note: nil
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsAsync() async throws {
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB/assets")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset":"66owaQmAQMuHxPzxUN3bqZ6FJfDa","account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","scope":"global","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(
            accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            asset: nil
        )
        XCTAssertEqual(balances.count, 1)
        XCTAssertEqual(balances.first?.asset, roseAssetDefinitionId)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsAsyncDecodesAssetFieldsDirectly() async throws {
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB/assets")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset":"66owaQmAQMuHxPzxUN3bqZ6FJfDa","account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","scope":"global","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(
            accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            asset: nil
        )
        XCTAssertEqual(balances.count, 1)
        guard let item = balances.first else {
            XCTFail("missing asset balance")
            return
        }
        XCTAssertEqual(item.asset, roseAssetDefinitionId)
        XCTAssertEqual(item.accountId, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(item.scope, "global")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsAsyncDecodesReadableAssetFields() async throws {
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB/assets")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{
              "asset":"66owaQmAQMuHxPzxUN3bqZ6FJfDa",
              "account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
              "scope":"global",
              "asset_name":"USD",
              "asset_alias":"usd#issuer.main",
              "quantity":"10"
            }]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(
            accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            asset: nil
        )
        XCTAssertEqual(balances.count, 1)
        XCTAssertEqual(balances.first?.asset, roseAssetDefinitionId)
        XCTAssertEqual(balances.first?.accountId, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(balances.first?.scope, "global")
        XCTAssertEqual(balances.first?.assetName, "USD")
        XCTAssertEqual(balances.first?.assetAlias, "usd#issuer.main")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsPreservesPercentEncodedPathWithBasePath() async throws {
        let baseURL = URL(string: "https://example.test/api")!
        let client = makeClient(baseURL: baseURL)
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/api/v1/accounts/sorauﾛ1NfｺｷﾘcﾙｦEﾑgsKti4Zﾘ6HKｳZCﾅｸｼ16fvSｲymｶｻﾘﾎ29JNWE/assets")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = "[]".data(using: .utf8)!
            return (response, body)
        }

        let balances = try await client.getAssets(
            accountId: "sorauﾛ1NfｺｷﾘcﾙｦEﾑgsKti4Zﾘ6HKｳZCﾅｸｼ16fvSｲymｶｻﾘﾎ29JNWE",
            asset: nil
        )
        XCTAssertEqual(balances.count, 0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testResolveAssetAliasAsync() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/assets/aliases/resolve")
            XCTAssertEqual(request.httpMethod, "POST")
            let payload = self.bodyJSON(from: request)
            XCTAssertEqual(payload["alias"] as? String, "usd#issuer.main")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
              "alias":"usd#issuer.main",
              "asset_definition_id":"66owaQmAQMuHxPzxUN3bqZ6FJfDa",
              "asset_name":"USD",
              "description":"United States Dollar",
              "logo":"sorafs://logos/usd.png",
              "source":"world_state",
              "alias_binding":{"alias":"usd#issuer.main","status":"permanent","bound_at_ms":1}
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let resolved = try await makeClient().resolveAssetAlias("usd#issuer.main")
        XCTAssertEqual(resolved?.alias, "usd#issuer.main")
        XCTAssertEqual(resolved?.assetDefinitionId, "66owaQmAQMuHxPzxUN3bqZ6FJfDa")
        XCTAssertEqual(resolved?.assetName, "USD")
        XCTAssertEqual(resolved?.description, "United States Dollar")
        XCTAssertEqual(resolved?.logo, "sorafs://logos/usd.png")
        XCTAssertEqual(resolved?.source, "world_state")
        XCTAssertEqual(resolved?.aliasBinding?.status, "permanent")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testResolveAssetAliasReturnsNilOnNotFound() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/assets/aliases/resolve")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 404,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = Data()
            return (response, body)
        }

        let resolved = try await makeClient().resolveAssetAlias("missing#issuer.main")
        XCTAssertNil(resolved)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testResolveAccountAliasAsync() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/aliases/resolve")
            XCTAssertEqual(request.httpMethod, "POST")
            let payload = self.bodyJSON(from: request)
            XCTAssertEqual(payload["alias"] as? String, "alice")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
              "alias":"alice",
              "accountId":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
              "index":7,
              "source":"world_state"
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let resolved = try await makeClient().resolveAccountAlias("alice")
        XCTAssertEqual(resolved?.alias, "alice")
        XCTAssertEqual(resolved?.accountId, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(resolved?.accountIds, ["sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"])
        XCTAssertEqual(resolved?.index, 7)
        XCTAssertEqual(resolved?.source, "world_state")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testResolveAccountAliasParsesAccountIdsArray() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/aliases/resolve")
            XCTAssertEqual(request.httpMethod, "POST")
            let payload = self.bodyJSON(from: request)
            XCTAssertEqual(payload["alias"] as? String, "alice")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
              "alias":"alice",
              "account_ids":[
                "  sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB  ",
                "",
                "sorauﾛ1Nｼﾒnq9A2ｵﾗﾐｵGﾕﾕｸﾜLﾁﾐAfｻQ5Rcj2DRﾒﾀqTgnUoU72NGB"
              ],
              "source":"world_state"
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let resolved = try await makeClient().resolveAccountAlias("alice")
        XCTAssertEqual(resolved?.alias, "alice")
        XCTAssertEqual(resolved?.accountId, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(
            resolved?.accountIds,
            [
                "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                "sorauﾛ1Nｼﾒnq9A2ｵﾗﾐｵGﾕﾕｸﾜLﾁﾐAfｻQ5Rcj2DRﾒﾀqTgnUoU72NGB",
            ]
        )
        XCTAssertEqual(resolved?.source, "world_state")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testResolveAccountAliasReturnsNilOnNotFound() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/aliases/resolve")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 404,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, Data())
        }

        let resolved = try await makeClient().resolveAccountAlias("missing-alias")
        XCTAssertNil(resolved)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListIdentifierPoliciesAsync() async throws {
        let owner = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/identifier-policies")
            XCTAssertEqual(request.httpMethod, "GET")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
              "total": 1,
              "items": [{
                "policy_id":"phone#retail",
                "owner":"\(owner)",
                "active":true,
                "normalization":"phone_e164",
                "resolver_public_key":"ed25519:resolver-key",
                "backend":"bfv-affine-sha3-256-v1",
                "input_encryption":"bfv-v1",
                "input_encryption_public_parameters":"ABCD",
                "input_encryption_public_parameters_decoded":{
                  "parameters":{
                    "polynomial_degree":64,
                    "plaintext_modulus":257,
                    "ciphertext_modulus":1099511627776,
                    "decomposition_base_log":12
                  },
                  "public_key":{
                    "b":[1,2,3],
                    "a":[4,5,6]
                  },
                  "max_input_bytes":32
                },
                "ram_fhe_profile":{
                  "profile_version":1,
                  "register_count":4,
                  "memory_lane_count":32,
                  "ciphertext_mul_per_step":1,
                  "encrypted_input_mode":"encrypted_envelope_v1",
                  "min_ciphertext_modulus":1099511627776
                },
                "note":"retail phone policy"
              }]
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let response = try await makeClient().listIdentifierPolicies()
        XCTAssertEqual(response.total, 1)
        XCTAssertEqual(response.items.count, 1)
        XCTAssertEqual(response.items.first?.policyId, "phone#retail")
        XCTAssertEqual(response.items.first?.programId, "")
        XCTAssertEqual(response.items.first?.outputOpeningPublicKey, "")
        XCTAssertEqual(response.items.first?.owner, owner)
        XCTAssertEqual(response.items.first?.normalization, .phoneE164)
        XCTAssertEqual(response.items.first?.inputEncryption, "bfv-v1")
        XCTAssertEqual(response.items.first?.inputEncryptionPublicParameters, "ABCD")
        XCTAssertEqual(
            response.items.first?.inputEncryptionPublicParametersDecoded?.parameters.polynomialDegree,
            64
        )
        XCTAssertEqual(
            response.items.first?.inputEncryptionPublicParametersDecoded?.parameters.decompositionBaseLog,
            12
        )
        XCTAssertEqual(response.items.first?.ramFheProfile?.profileVersion, 1)
        XCTAssertEqual(response.items.first?.ramFheProfile?.registerCount, 4)
        XCTAssertEqual(response.items.first?.ramFheProfile?.memoryLaneCount, 32)
        XCTAssertEqual(
            response.items.first?.ramFheProfile?.encryptedInputMode,
            .encryptedEnvelopeV1
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListIdentifierPoliciesAcceptsTaggedEncryptedInputMode() async throws {
        let owner = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/identifier-policies")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            let body = """
            {
              "total": 1,
              "items": [{
                "policy_id":"email#retail",
                "owner":"\(owner)",
                "active":true,
                "normalization":"email_address",
                "resolver_public_key":"ed25519:resolver-key",
                "backend":"bfv-programmed-sha3-256-v1",
                "input_encryption":"bfv-v1",
                "input_encryption_public_parameters":"ABCD",
                "input_encryption_public_parameters_decoded":{
                  "parameters":{
                    "polynomial_degree":64,
                    "plaintext_modulus":256,
                    "ciphertext_modulus":4503599627370496,
                    "decomposition_base_log":12
                  },
                  "public_key":{
                    "b":[1,2,3],
                    "a":[4,5,6]
                  },
                  "max_input_bytes":63
                },
                "ram_fhe_profile":{
                  "profile_version":1,
                  "register_count":4,
                  "memory_lane_count":32,
                  "ciphertext_mul_per_step":1,
                  "encrypted_input_mode":{
                    "mode":"EncryptedEnvelopeV1",
                    "value":null
                  },
                  "min_ciphertext_modulus":4503599627370496
                },
                "note":"retail email policy"
              }]
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let response = try await makeClient().listIdentifierPolicies()
        XCTAssertEqual(response.total, 1)
        XCTAssertEqual(response.items.first?.policyId, "email#retail")
        XCTAssertEqual(
            response.items.first?.ramFheProfile?.encryptedInputMode,
            .encryptedEnvelopeV1
        )
        XCTAssertEqual(
            response.items.first?.inputEncryptionPublicParametersDecoded?.maxInputBytes,
            63
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListIdentifierPoliciesAcceptsLiveResolverCanonicalizedInputMode() async throws {
        let owner = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/identifier-policies")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            let body = """
            {
              "total": 1,
              "items": [{
                "policy_id":"email#retail",
                "owner":"\(owner)",
                "active":true,
                "normalization":"email_address",
                "resolver_public_key":"ed25519:resolver-key",
                "backend":"bfv-programmed-sha3-256-v1",
                "input_encryption":"bfv-v1",
                "input_encryption_public_parameters_decoded":{
                  "parameters":{
                    "polynomial_degree":64,
                    "plaintext_modulus":256,
                    "ciphertext_modulus":4503599627370496,
                    "decomposition_base_log":12
                  },
                  "public_key":{
                    "b":[1,2,3],
                    "a":[4,5,6]
                  },
                  "max_input_bytes":63
                },
                "ram_fhe_profile":{
                  "profile_version":1,
                  "register_count":4,
                  "memory_lane_count":32,
                  "ciphertext_mul_per_step":1,
                  "encrypted_input_mode":{
                    "mode":"ResolverCanonicalizedEnvelopeV1",
                    "value":null
                  },
                  "min_ciphertext_modulus":4503599627370496
                },
                "note":"Retail identifier policy email#retail"
              }]
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let response = try await makeClient().listIdentifierPolicies()
        XCTAssertEqual(response.items.first?.policyId, "email#retail")
        XCTAssertEqual(response.items.first?.programId, "")
        XCTAssertEqual(
            response.items.first?.ramFheProfile?.encryptedInputMode,
            .resolverCanonicalizedEnvelopeV1
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListRamLfeProgramPoliciesAsync() async throws {
        let owner = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/ram-lfe/program-policies")
            XCTAssertEqual(request.httpMethod, "GET")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
              "total": 1,
              "items": [{
                "program_id":"identifier_lookup_retail",
                "owner":"\(owner)",
                "active":true,
                "resolver_public_key":"ed25519:resolver-key",
                "backend":"bfv-programmed-sha3-256-v1",
                "verification_mode":"signed",
                "input_encryption":"bfv-v1",
                "input_encryption_public_parameters":"ABCD",
                "input_encryption_public_parameters_decoded":{
                  "parameters":{
                    "polynomial_degree":64,
                    "plaintext_modulus":257,
                    "ciphertext_modulus":1099511627776,
                    "decomposition_base_log":12
                  },
                  "public_key":{
                    "b":[1,2,3],
                    "a":[4,5,6]
                  },
                  "max_input_bytes":32
                },
                "ram_fhe_profile":{
                  "profile_version":1,
                  "register_count":4,
                  "memory_lane_count":32,
                  "ciphertext_mul_per_step":1,
                  "encrypted_input_mode":"encrypted_envelope_v1",
                  "min_ciphertext_modulus":1099511627776
                },
                "note":"retail programmed policy"
              }]
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let response = try await makeClient().listRamLfeProgramPolicies()
        XCTAssertEqual(response.total, 1)
        XCTAssertEqual(response.items.count, 1)
        XCTAssertEqual(response.items.first?.programId, "identifier_lookup_retail")
        XCTAssertEqual(response.items.first?.owner, owner)
        XCTAssertEqual(response.items.first?.verificationMode, "signed")
        XCTAssertEqual(response.items.first?.inputEncryption, "bfv-v1")
        XCTAssertEqual(
            response.items.first?.inputEncryptionPublicParametersDecoded?.parameters.polynomialDegree,
            64
        )
        XCTAssertEqual(response.items.first?.ramFheProfile?.profileVersion, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testExecuteRamLfeProgramAsync() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/ram-lfe/programs/identifier_lookup_retail/execute")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            let payload = self.bodyJSON(from: request)
            XCTAssertNil(payload["input_hex"])
            XCTAssertEqual(payload["encrypted_input"] as? String, "abcd")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
              "program_id":"identifier_lookup_retail",
              "opaque_hash":"opaque-hash-literal",
              "receipt_hash":"receipt-hash-literal",
              "output_ciphertext":"dcba",
              "output_hash":"output-hash-literal",
              "associated_data_hash":"associated-data-hash-literal",
              "executed_at_ms":42,
              "expires_at_ms":142,
              "backend":"bfv-programmed-sha3-256-v1",
              "verification_mode":"signed",
              "receipt":{
                "payload":{
                  "program_id":"identifier_lookup_retail",
                  "program_digest":"\(String(repeating: "11", count: 32))",
                  "backend":"bfv-programmed-sha3-256-v1",
                  "verification_mode":"signed",
                  "output_hash":"\(String(repeating: "22", count: 32))",
                  "associated_data_hash":"\(String(repeating: "33", count: 32))",
                  "executed_at_ms":42,
                  "expires_at_ms":142
                },
                "attestation":{
                  "kind":"signed",
                  "signature":"\(String(repeating: "aa", count: 64))"
                }
              },
              "output_opening":{
                "payload":{
                  "program_id":"identifier_lookup_retail",
                  "input_ciphertext_hash":"\(String(repeating: "44", count: 32))",
                  "output_ciphertext_hash":"\(String(repeating: "55", count: 32))",
                  "parameter_digest":"\(String(repeating: "66", count: 32))",
                  "evaluation_key_digest":"\(String(repeating: "77", count: 32))",
                  "opened_output_hash":"output-hash-literal",
                  "opened_at_ms":43,
                  "expires_at_ms":142
                },
                "signature":"\(String(repeating: "bb", count: 64))"
              }
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let response = try await makeClient().executeRamLfeProgram(
            programId: "identifier_lookup_retail",
            encryptedInputHex: "0xABCD"
        )
        XCTAssertEqual(response?.programId, "identifier_lookup_retail")
        XCTAssertEqual(response?.outputHash, "output-hash-literal")
        XCTAssertEqual(response?.verificationMode, "signed")
        if case let .object(receipt)? = response?.receipt["payload"] {
            XCTAssertNotNil(receipt["program_id"])
        } else {
            XCTFail("missing raw receipt payload")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testExecuteRamLfeProgramReturnsNilOnNotFound() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/ram-lfe/programs/identifier_lookup_retail/execute")
            let payload = self.bodyJSON(from: request)
            XCTAssertEqual(payload["encrypted_input"] as? String, "abcd")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 404,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, Data())
        }

        let response = try await makeClient().executeRamLfeProgram(
            programId: "identifier_lookup_retail",
            encryptedInputHex: "ABCD"
        )
        XCTAssertNil(response)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testVerifyRamLfeReceiptAsync() async throws {
        let receipt: ToriiRamLfeExecutionReceipt = [
            "payload": .object([
                "program_id": .string("identifier_lookup_retail"),
                "program_digest": .string(String(repeating: "11", count: 32)),
                "backend": .string("bfv-programmed-sha3-256-v1"),
                "verification_mode": .string("signed"),
                "output_hash": .string(String(repeating: "22", count: 32)),
                "associated_data_hash": .string(String(repeating: "33", count: 32)),
                "executed_at_ms": .number(42),
                "expires_at_ms": .number(142)
            ]),
            "attestation": .object([
                "kind": .string("signed"),
                "signature": .string(String(repeating: "aa", count: 64))
            ])
        ]
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/ram-lfe/receipts/verify")
            XCTAssertEqual(request.httpMethod, "POST")
            let payload = self.bodyJSON(from: request)
            XCTAssertEqual(payload["output_hex"] as? String, "c0ffee")
            let receiptObject = payload["receipt"] as? [String: Any]
            let payloadObject = receiptObject?["payload"] as? [String: Any]
            XCTAssertNotNil(payloadObject?["program_id"])
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
              "valid":true,
              "program_id":"identifier_lookup_retail",
              "backend":"bfv-programmed-sha3-256-v1",
              "verification_mode":"signed",
              "output_hash":"output-hash-literal",
              "associated_data_hash":"associated-data-hash-literal",
              "output_hash_matches":true
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let response = try await makeClient().verifyRamLfeReceipt(
            receipt: receipt,
            outputHex: "C0FFEE"
        )
        XCTAssertTrue(response.valid)
        XCTAssertEqual(response.programId, "identifier_lookup_retail")
        XCTAssertEqual(response.outputHashMatches, true)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testResolveIdentifierAsync() async throws {
        let accountId = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"
        let opaqueId = "opaque:\(String(repeating: "11", count: 32))"
        let receiptHash = String(repeating: "22", count: 31) + "23"
        let uaid = "uaid:\(String(repeating: "33", count: 31))35"
        let signedPayload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: opaqueId,
            receiptHash: receiptHash,
            uaid: uaid,
            backend: "bfv-affine-sha3-256-v1"
        )
        let signed = try signedIdentifierReceiptFixture(payload: signedPayload)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/identifiers/resolve")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            let payload = self.bodyJSON(from: request)
            XCTAssertEqual(payload["policy_id"] as? String, "phone#retail")
            XCTAssertEqual(payload["encrypted_input"] as? String, "abcd")
            XCTAssertNotNil(payload["output_opening"])
            XCTAssertNotNil(payload["output_opening"])
            XCTAssertNil(payload["input"])
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = try self.identifierReceiptJSON(
                payload: signedPayload,
                signatureHex: signed.signatureHex
            ).data(using: .utf8)!
            return (response, body)
        }

        let receipt = try await makeClient().resolveIdentifier(
            policyId: " phone#retail ",
            encryptedInputHex: "0xABCD",
            outputOpening: signedPayload.opening
        )
        XCTAssertEqual(receipt?.policyId, "phone#retail")
        XCTAssertEqual(receipt?.opaqueId, opaqueId)
        XCTAssertEqual(receipt?.receiptHash, receiptHash)
        XCTAssertEqual(receipt?.uaid, uaid)
        XCTAssertEqual(receipt?.accountId, accountId)
        XCTAssertEqual(receipt?.resolvedAtMs, 42)
        XCTAssertEqual(receipt?.expiresAtMs, 142)
        XCTAssertEqual(receipt?.backend, "bfv-affine-sha3-256-v1")
        let policy = ToriiIdentifierPolicySummary(
            policyId: "phone#retail",
            owner: accountId,
            active: true,
            normalization: .phoneE164,
            resolverPublicKey: signed.resolverPublicKey,
            backend: "bfv-affine-sha3-256-v1",
            inputEncryption: "bfv-v1",
            inputEncryptionPublicParameters: nil,
            inputEncryptionPublicParametersDecoded: nil,
            ramFheProfile: nil,
            proofVerifier: nil,
            note: nil
        )
        XCTAssertEqual(try receipt?.verifyAttestation(using: policy), true)
    }

    func testIdentifierReceiptOpeningSignatureUsesConstVecEncoding() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1",
            openingSignatureHex: "FAFBFC"
        )

        let encoded = try ToriiIdentifierReceiptCanonicalEncoder.encodePayload(payload)

        XCTAssertNotNil(
            encoded.range(of: Data([0x07, 0x03, 0x01, 0xFA, 0x01, 0xFB, 0x01, 0xFC]))
        )
        XCTAssertNil(
            encoded.range(of: Data([0x04, 0x03, 0xFA, 0xFB, 0xFC]))
        )
    }

    func testIdentifierReceiptOpeningSignatureConstVecEncodingUsesLongLengthFraming() throws {
        let accountId = try canonicalOwnerLiteral()
        let longSignature = Data((0..<128).map { UInt8($0) })
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1",
            openingSignatureHex: longSignature.hexUppercased()
        )

        let signaturePayload = try openingSignaturePayload(
            in: ToriiIdentifierReceiptCanonicalEncoder.encodePayload(payload)
        )

        XCTAssertEqual(signaturePayload, constVecBytes(longSignature))
        XCTAssertNotEqual(signaturePayload, legacyFlatBytesVec(longSignature))
        XCTAssertEqual(
            Data(signaturePayload.prefix(6)),
            Data([0x80, 0x01, 0x01, 0x00, 0x01, 0x01])
        )
    }

    func testIdentifierReceiptRejectsMalformedOpeningSignatureHex() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1",
            openingSignatureHex: "0xGG"
        )

        XCTAssertThrowsError(try ToriiIdentifierReceiptCanonicalEncoder.encodePayload(payload)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                XCTFail("expected invalid payload error, got \(error)")
                return
            }
            XCTAssertTrue(reason.contains("payload.opening.signature"))
        }
    }

    func testIdentifierReceiptRejectsLegacyFlatOpeningSignatureAttestation() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1",
            openingSignatureHex: "FAFBFC"
        )
        let currentSigned = try signedIdentifierReceiptFixture(payload: payload)
        let legacySigned = try signedIdentifierReceiptFixture(
            payload: payload,
            canonicalPayloadBytes: payloadBytesWithLegacyFlatOpeningSignature(payload)
        )
        let policy = ToriiIdentifierPolicySummary(
            policyId: "phone#retail",
            owner: accountId,
            active: true,
            normalization: .phoneE164,
            resolverPublicKey: currentSigned.resolverPublicKey,
            backend: "bfv-affine-sha3-256-v1",
            inputEncryption: "bfv-v1",
            inputEncryptionPublicParameters: nil,
            inputEncryptionPublicParametersDecoded: nil,
            ramFheProfile: nil,
            proofVerifier: nil,
            note: nil
        )
        let currentReceipt = try JSONDecoder().decode(
            ToriiIdentifierResolutionReceipt.self,
            from: Data(try identifierReceiptJSON(
                payload: payload,
                signatureHex: currentSigned.signatureHex
            ).utf8)
        )
        let legacyReceipt = try JSONDecoder().decode(
            ToriiIdentifierResolutionReceipt.self,
            from: Data(try identifierReceiptJSON(
                payload: payload,
                signatureHex: legacySigned.signatureHex
            ).utf8)
        )

        XCTAssertEqual(try currentReceipt.verifyAttestation(using: policy), true)
        XCTAssertEqual(try legacyReceipt.verifyAttestation(using: policy), false)
    }

    func testIdentifierReceiptRejectsOpeningSignatureMutationAfterSigning() throws {
        let accountId = try canonicalOwnerLiteral()
        let originalPayload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1",
            openingSignatureHex: String(repeating: "fa", count: 64)
        )
        let mutatedPayload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1",
            openingSignatureHex: String(repeating: "fb", count: 64)
        )
        let signed = try signedIdentifierReceiptFixture(payload: originalPayload)
        let policy = ToriiIdentifierPolicySummary(
            policyId: "phone#retail",
            owner: accountId,
            active: true,
            normalization: .phoneE164,
            resolverPublicKey: signed.resolverPublicKey,
            backend: "bfv-affine-sha3-256-v1",
            inputEncryption: "bfv-v1",
            inputEncryptionPublicParameters: nil,
            inputEncryptionPublicParametersDecoded: nil,
            ramFheProfile: nil,
            proofVerifier: nil,
            note: nil
        )
        let originalReceipt = try JSONDecoder().decode(
            ToriiIdentifierResolutionReceipt.self,
            from: Data(try identifierReceiptJSON(
                payload: originalPayload,
                signatureHex: signed.signatureHex
            ).utf8)
        )
        let mutatedReceipt = try JSONDecoder().decode(
            ToriiIdentifierResolutionReceipt.self,
            from: Data(try identifierReceiptJSON(
                payload: mutatedPayload,
                signatureHex: signed.signatureHex
            ).utf8)
        )

        XCTAssertEqual(try originalReceipt.verifyAttestation(using: policy), true)
        XCTAssertEqual(try mutatedReceipt.verifyAttestation(using: policy), false)
    }

    func testIdentifierReceiptRejectsReceiptHashMutationAfterSigning() throws {
        let accountId = try canonicalOwnerLiteral()
        let originalPayload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let mutatedPayload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "25",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let signed = try signedIdentifierReceiptFixture(payload: originalPayload)
        let policy = identifierPolicy(
            owner: accountId,
            resolverPublicKey: signed.resolverPublicKey
        )
        let originalReceipt = try identifierReceipt(
            payload: originalPayload,
            signatureHex: signed.signatureHex
        )
        let mutatedReceipt = try identifierReceipt(
            payload: mutatedPayload,
            signatureHex: signed.signatureHex
        )

        XCTAssertEqual(try originalReceipt.verifyAttestation(using: policy), true)
        XCTAssertEqual(try mutatedReceipt.verifyAttestation(using: policy), false)
    }

    func testIdentifierReceiptRejectsUaidMutationAfterSigning() throws {
        let accountId = try canonicalOwnerLiteral()
        let originalPayload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let mutatedPayload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))37",
            backend: "bfv-affine-sha3-256-v1"
        )
        let signed = try signedIdentifierReceiptFixture(payload: originalPayload)
        let policy = identifierPolicy(
            owner: accountId,
            resolverPublicKey: signed.resolverPublicKey
        )
        let originalReceipt = try identifierReceipt(
            payload: originalPayload,
            signatureHex: signed.signatureHex
        )
        let mutatedReceipt = try identifierReceipt(
            payload: mutatedPayload,
            signatureHex: signed.signatureHex
        )

        XCTAssertEqual(try originalReceipt.verifyAttestation(using: policy), true)
        XCTAssertEqual(try mutatedReceipt.verifyAttestation(using: policy), false)
    }

    func testIdentifierReceiptRejectsMismatchedPolicySummaryBeforeSignatureVerification() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let signed = try signedIdentifierReceiptFixture(payload: payload)
        let receipt = try identifierReceipt(payload: payload, signatureHex: signed.signatureHex)
        let policy = identifierPolicy(
            policyId: "email#retail",
            owner: accountId,
            resolverPublicKey: signed.resolverPublicKey
        )

        XCTAssertThrowsError(try receipt.verifyAttestation(using: policy)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                XCTFail("expected invalid payload error, got \(error)")
                return
            }
            XCTAssertTrue(reason.contains("does not match policy summary"))
        }
    }

    func testIdentifierReceiptRejectsWrongResolverPublicKey() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let signed = try signedIdentifierReceiptFixture(payload: payload)
        let otherSigned = try signedIdentifierReceiptFixture(payload: payload)
        let receipt = try identifierReceipt(payload: payload, signatureHex: signed.signatureHex)
        let validPolicy = identifierPolicy(
            owner: accountId,
            resolverPublicKey: signed.resolverPublicKey
        )
        let wrongKeyPolicy = identifierPolicy(
            owner: accountId,
            resolverPublicKey: otherSigned.resolverPublicKey
        )

        XCTAssertNotEqual(signed.resolverPublicKey, otherSigned.resolverPublicKey)
        XCTAssertEqual(try receipt.verifyAttestation(using: validPolicy), true)
        XCTAssertEqual(try receipt.verifyAttestation(using: wrongKeyPolicy), false)
    }

    func testIdentifierReceiptRejectsMalformedResolverPublicKey() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let signed = try signedIdentifierReceiptFixture(payload: payload)
        let receipt = try identifierReceipt(payload: payload, signatureHex: signed.signatureHex)
        let validPolicy = identifierPolicy(
            owner: accountId,
            resolverPublicKey: signed.resolverPublicKey
        )
        let malformedPolicy = identifierPolicy(
            owner: accountId,
            resolverPublicKey: "ed25519:not-hex"
        )

        XCTAssertEqual(try receipt.verifyAttestation(using: validPolicy), true)
        XCTAssertThrowsError(try receipt.verifyAttestation(using: malformedPolicy)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                XCTFail("expected invalid payload error, got \(error)")
                return
            }
            XCTAssertTrue(reason.contains("resolverPublicKey"))
        }
    }

    func testIdentifierReceiptRejectsResolverPublicKeyPrefixMismatch() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let signed = try signedIdentifierReceiptFixture(payload: payload)
        let multihash = try XCTUnwrap(signed.resolverPublicKey.split(separator: ":").last)
        let receipt = try identifierReceipt(payload: payload, signatureHex: signed.signatureHex)
        let validPolicy = identifierPolicy(
            owner: accountId,
            resolverPublicKey: signed.resolverPublicKey
        )
        let mismatchedPrefixPolicy = identifierPolicy(
            owner: accountId,
            resolverPublicKey: "secp256k1:\(multihash)"
        )

        XCTAssertEqual(try receipt.verifyAttestation(using: validPolicy), true)
        XCTAssertThrowsError(try receipt.verifyAttestation(using: mismatchedPrefixPolicy)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                XCTFail("expected invalid payload error, got \(error)")
                return
            }
            XCTAssertTrue(reason.contains("prefix does not match"))
        }
    }

    func testIdentifierReceiptRejectsMalformedAttestationSignatureHex() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let signed = try signedIdentifierReceiptFixture(payload: payload)
        let receipt = try identifierReceipt(payload: payload, signatureHex: "GG")
        let policy = identifierPolicy(
            owner: accountId,
            resolverPublicKey: signed.resolverPublicKey
        )

        XCTAssertThrowsError(try receipt.verifyAttestation(using: policy)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                XCTFail("expected invalid payload error, got \(error)")
                return
            }
            XCTAssertTrue(reason.contains("attestation.signature"))
        }
    }

    func testIdentifierReceiptRejectsProofAttestationVerificationWithResolverKey() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let signed = try signedIdentifierReceiptFixture(payload: payload)
        let payloadJSON = try XCTUnwrap(
            String(data: JSONEncoder().encode(payload), encoding: .utf8)
        )
        let receiptJSON = """
        {
          "payload":\(payloadJSON),
          "attestation":{
            "kind":"proof",
            "proof_backend":"ram-lfe-v1",
            "proof_b64":"AAAA"
          }
        }
        """
        let receipt = try JSONDecoder().decode(
            ToriiIdentifierResolutionReceipt.self,
            from: Data(receiptJSON.utf8)
        )
        let policy = identifierPolicy(
            owner: accountId,
            resolverPublicKey: signed.resolverPublicKey
        )

        XCTAssertThrowsError(try receipt.verifyAttestation(using: policy)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                XCTFail("expected invalid payload error, got \(error)")
                return
            }
            XCTAssertTrue(reason.contains("Only signed identifier receipt attestations"))
        }
    }

    func testIdentifierReceiptRejectsSignedAttestationMissingSignatureDuringDecode() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let payloadJSON = try XCTUnwrap(
            String(data: JSONEncoder().encode(payload), encoding: .utf8)
        )
        let receiptJSON = """
        {
          "payload":\(payloadJSON),
          "attestation":{
            "kind":"signed"
          }
        }
        """

        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiIdentifierResolutionReceipt.self,
                from: Data(receiptJSON.utf8)
            )
        ) { error in
            guard case let DecodingError.dataCorrupted(context) = error else {
                XCTFail("expected dataCorrupted decode error, got \(error)")
                return
            }
            XCTAssertTrue(context.debugDescription.contains("require only signature"))
        }
    }

    func testIdentifierReceiptRejectsSignedAttestationWithProofFieldsDuringDecode() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let signed = try signedIdentifierReceiptFixture(payload: payload)
        let payloadJSON = try XCTUnwrap(
            String(data: JSONEncoder().encode(payload), encoding: .utf8)
        )
        let receiptJSON = """
        {
          "payload":\(payloadJSON),
          "attestation":{
            "kind":"signed",
            "signature":"\(signed.signatureHex)",
            "proof_backend":"ram-lfe-v1",
            "proof_b64":"AAAA"
          }
        }
        """

        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiIdentifierResolutionReceipt.self,
                from: Data(receiptJSON.utf8)
            )
        ) { error in
            guard case let DecodingError.dataCorrupted(context) = error else {
                XCTFail("expected dataCorrupted decode error, got \(error)")
                return
            }
            XCTAssertTrue(context.debugDescription.contains("require only signature"))
        }
    }

    func testIdentifierReceiptRejectsUnknownAttestationKindDuringDecode() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let signed = try signedIdentifierReceiptFixture(payload: payload)
        let payloadJSON = try XCTUnwrap(
            String(data: JSONEncoder().encode(payload), encoding: .utf8)
        )
        let receiptJSON = """
        {
          "payload":\(payloadJSON),
          "attestation":{
            "kind":"unsigned",
            "signature":"\(signed.signatureHex)"
          }
        }
        """

        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiIdentifierResolutionReceipt.self,
                from: Data(receiptJSON.utf8)
            )
        ) { error in
            guard case let DecodingError.dataCorrupted(context) = error else {
                XCTFail("expected dataCorrupted decode error, got \(error)")
                return
            }
            XCTAssertTrue(context.debugDescription.contains("attestation kind"))
        }
    }

    func testIdentifierReceiptCanonicalPayloadMatchesLiveToriiFixtureAndRejectsLegacySignature() throws {
        let accountId = "sorauﾛ1NiGｸﾛﾋRuﾎQtﾐpヱﾈｻHﾍﾐ3RZﾕYdvbｺhcｽG8A8ｿRﾗeP1E463"
        let receiptJSON = """
        {
          "payload":{
            "policy_id":"email#retail",
            "execution":{
              "program_id":"email_retail",
              "program_digest":"fe36ceb3996d101200b895fd2a377cce4426426a473da9fe08b2dbd2bd8b9375",
              "backend":"bfv-programmed-sha3-256-v1",
              "verification_mode":"signed",
              "input_ciphertext_hash":"abababababababababababababababababababababababababababababababab",
              "output_ciphertext_hash":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
              "parameter_digest":"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
              "evaluation_key_digest":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
              "output_hash":"72dcdee1435552e943d5e2e1c978d3f728c6a1ce7e6870b50c63568d4876eea5",
              "associated_data_hash":"35b8bc8a30685e7cc5679b6e6a45675539548f5a24326bbee1d8c20e55918f55",
              "executed_at_ms":1776812470694,
              "expires_at_ms":1776812500694
            },
            "opening":{
              "payload":{
                "program_id":"email_retail",
                "input_ciphertext_hash":"abababababababababababababababababababababababababababababababab",
                "output_ciphertext_hash":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "parameter_digest":"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd",
                "evaluation_key_digest":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                "opened_output_hash":"72dcdee1435552e943d5e2e1c978d3f728c6a1ce7e6870b50c63568d4876eea5",
                "opened_at_ms":1776812470694,
                "expires_at_ms":1776812500694
              },
              "signature":"FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
            },
            "opaque_id":"opaque:fd14cb369e853352d4b9c578745627d154471ce5fd3462c4db542c104766e983",
            "receipt_hash":"51bbe55b70e09d4c2bb75d9c31b2cde46a7bdd5414134f6786255c679a68ac53",
            "uaid":"uaid:471b620a99c608af1c7a47199f27b3368ae0ea889a497dd774b52a8287a58393",
            "account_id":"\(accountId)"
          },
          "attestation":{
            "kind":"signed",
            "signature":"4B26BF33F721C551C13F102D4D7F483CB8DD8A13FD6BF4ED26C845E2B69D5D0124B8CFA05493772F6748A42408EEE4542C470B284AB87F686B423F9DF87C8D00"
          }
        }
        """
        let receipt = try JSONDecoder().decode(
            ToriiIdentifierResolutionReceipt.self,
            from: Data(receiptJSON.utf8)
        )
        XCTAssertFalse(try ToriiIdentifierReceiptCanonicalEncoder.encodePayload(receipt.payload).isEmpty)
        let policy = ToriiIdentifierPolicySummary(
            policyId: "email#retail",
            owner: accountId,
            active: true,
            normalization: .emailAddress,
            resolverPublicKey: "ed01200376E59E9078B647F55003896B59758B7BE99908535EC24BAF80A6D52C8B3EB8",
            backend: "bfv-programmed-sha3-256-v1",
            inputEncryption: "bfv-v1",
            inputEncryptionPublicParameters: nil,
            inputEncryptionPublicParametersDecoded: nil,
            ramFheProfile: nil,
            proofVerifier: nil,
            note: nil
        )
        XCTAssertEqual(try receipt.verifyAttestation(using: policy), false)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testResolveIdentifierRejectsInvalidAttestationPayload() async throws {
        let accountId = try canonicalOwnerLiteral()
        let opaqueId = "opaque:\(String(repeating: "41", count: 32))"
        let receiptHash = String(repeating: "51", count: 32)
        let uaid = "uaid:\(String(repeating: "61", count: 31))63"
        let signedPayload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: opaqueId,
            receiptHash: receiptHash,
            uaid: uaid,
            backend: "bfv-affine-sha3-256-v1"
        )
        let signed = try signedIdentifierReceiptFixture(payload: signedPayload)

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/identifiers/resolve")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
              "policy_id":"phone#retail",
              "opaque_id":"\(opaqueId)",
              "receipt_hash":"\(receiptHash)",
              "uaid":"\(uaid)",
              "account_id":"\(accountId)",
              "resolved_at_ms":42,
              "expires_at_ms":142,
              "backend":"bfv-affine-sha3-256-v1",
              "signature":"\(signed.signatureHex)",
              "signature_payload_hex":"01020304A0"
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        do {
            _ = try await makeClient().resolveIdentifier(
                policyId: "phone#retail",
                encryptedInputHex: "ABCD",
                outputOpening: signedPayload.opening
            )
            XCTFail("Expected invalidPayload error")
        } catch let ToriiClientError.invalidPayload(reason) {
            XCTAssertTrue(reason.contains("payload") || reason.contains("attestation"))
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testResolveIdentifierReturnsNilOnNotFound() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/identifiers/resolve")
            XCTAssertEqual(request.httpMethod, "POST")
            let payload = self.bodyJSON(from: request)
            XCTAssertEqual(payload["policy_id"] as? String, "phone#retail")
            XCTAssertEqual(payload["encrypted_input"] as? String, "abcd")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 404,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, Data())
        }

        let receipt = try await makeClient().resolveIdentifier(
            policyId: "phone#retail",
            encryptedInputHex: "0xABCD",
            outputOpening: sampleOpening()
        )
        XCTAssertNil(receipt)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testResolveIdentifierDecodesNestedExecutionPayload() async throws {
        let accountId = try canonicalOwnerLiteral()
        let opaqueId = "opaque:\(String(repeating: "77", count: 32))"
        let receiptHash = String(repeating: "88", count: 31) + "89"
        let outputHash = String(repeating: "22", count: 31) + "23"
        let uaid = "uaid:\(String(repeating: "99", count: 31))9b"
        let signedPayload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: opaqueId,
            receiptHash: receiptHash,
            uaid: uaid,
            backend: "bfv-programmed-sha3-256-v1",
            outputHashHex: outputHash,
            resolvedAtMs: 42,
            expiresAtMs: 142
        )
        let signed = try signedIdentifierReceiptFixture(payload: signedPayload)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/identifiers/resolve")
            XCTAssertEqual(request.httpMethod, "POST")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = try self.identifierReceiptJSON(
                payload: signedPayload,
                signatureHex: signed.signatureHex
            ).data(using: .utf8)!
            return (response, body)
        }

        let receipt = try await makeClient().resolveIdentifier(
            policyId: "phone#retail",
            encryptedInputHex: "ABCD",
            outputOpening: signedPayload.opening
        )
        XCTAssertEqual(receipt?.resolvedAtMs, 42)
        XCTAssertEqual(receipt?.expiresAtMs, 142)
        XCTAssertEqual(receipt?.payload.policyId, "phone#retail")
        XCTAssertEqual(receipt?.payload.opaqueId, opaqueId)
        XCTAssertEqual(receipt?.payload.receiptHash, receiptHash)
        XCTAssertEqual(receipt?.payload.uaid, uaid)
        XCTAssertEqual(receipt?.payload.execution.programId, "identifier_lookup_retail")
        XCTAssertEqual(receipt?.payload.execution.programDigest, String(repeating: "11", count: 32))
        XCTAssertEqual(receipt?.payload.execution.verificationMode, "signed")
        XCTAssertEqual(receipt?.payload.execution.inputCiphertextHash, String(repeating: "ab", count: 32))
        XCTAssertEqual(receipt?.payload.execution.outputCiphertextHash, String(repeating: "bb", count: 32))
        XCTAssertEqual(receipt?.payload.execution.parameterDigest, String(repeating: "cd", count: 32))
        XCTAssertEqual(receipt?.payload.execution.evaluationKeyDigest, String(repeating: "dd", count: 32))
        XCTAssertEqual(receipt?.payload.execution.outputHash, outputHash)
        XCTAssertEqual(receipt?.payload.execution.associatedDataHash, String(repeating: "33", count: 32))
        XCTAssertEqual(receipt?.payload.execution.executedAtMs, 42)
        XCTAssertEqual(receipt?.payload.execution.expiresAtMs, 142)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testIssueIdentifierClaimReceiptAsync() async throws {
        let accountId = try canonicalOwnerLiteral()
        let opaqueId = "opaque:\(String(repeating: "44", count: 31))45"
        let receiptHash = String(repeating: "55", count: 32)
        let uaid = "uaid:\(String(repeating: "66", count: 31))67"
        let signedPayload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: opaqueId,
            receiptHash: receiptHash,
            uaid: uaid,
            backend: "bfv-affine-sha3-256-v1",
            resolvedAtMs: 7,
            expiresAtMs: nil
        )
        let signed = try signedIdentifierReceiptFixture(payload: signedPayload)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(
                request.url?.path,
                "/v1/accounts/\(accountId)/identifiers/claim-receipt"
            )
            XCTAssertEqual(request.httpMethod, "POST")
            let payload = self.bodyJSON(from: request)
            XCTAssertEqual(payload["policy_id"] as? String, "phone#retail")
            XCTAssertEqual(payload["encrypted_input"] as? String, "abcd")
            XCTAssertNotNil(payload["output_opening"])
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = try self.identifierReceiptJSON(
                payload: signedPayload,
                signatureHex: signed.signatureHex
            ).data(using: .utf8)!
            return (response, body)
        }

        let receipt = try await makeClient().issueIdentifierClaimReceipt(
            accountId: accountId,
            policyId: "phone#retail",
            encryptedInputHex: "ABCD",
            outputOpening: signedPayload.opening
        )
        XCTAssertEqual(receipt?.opaqueId, opaqueId)
        XCTAssertEqual(receipt?.receiptHash, receiptHash)
        XCTAssertEqual(receipt?.uaid, uaid)
        XCTAssertEqual(receipt?.accountId, accountId)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testIssueIdentifierClaimReceiptDecodesNestedExecutionPayload() async throws {
        let accountId = try canonicalOwnerLiteral()
        let opaqueId = "opaque:\(String(repeating: "aa", count: 31))ab"
        let receiptHash = String(repeating: "bb", count: 32)
        let programDigest = String(repeating: "12", count: 31) + "13"
        let outputHash = String(repeating: "23", count: 32)
        let associatedDataHash = String(repeating: "34", count: 31) + "35"
        let uaid = "uaid:\(String(repeating: "cc", count: 31))cd"
        let signedPayload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: opaqueId,
            receiptHash: receiptHash,
            uaid: uaid,
            backend: "bfv-programmed-sha3-256-v1",
            programDigestHex: programDigest,
            outputHashHex: outputHash,
            associatedDataHashHex: associatedDataHash,
            resolvedAtMs: 7,
            expiresAtMs: 77
        )
        let signed = try signedIdentifierReceiptFixture(payload: signedPayload)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(
                request.url?.path,
                "/v1/accounts/\(accountId)/identifiers/claim-receipt"
            )
            XCTAssertEqual(request.httpMethod, "POST")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = try self.identifierReceiptJSON(
                payload: signedPayload,
                signatureHex: signed.signatureHex
            ).data(using: .utf8)!
            return (response, body)
        }

        let receipt = try await makeClient().issueIdentifierClaimReceipt(
            accountId: accountId,
            policyId: "phone#retail",
            encryptedInputHex: "ABCD",
            outputOpening: signedPayload.opening
        )
        XCTAssertEqual(receipt?.resolvedAtMs, 7)
        XCTAssertEqual(receipt?.expiresAtMs, 77)
        XCTAssertEqual(receipt?.payload.policyId, "phone#retail")
        XCTAssertEqual(receipt?.payload.opaqueId, opaqueId)
        XCTAssertEqual(receipt?.payload.receiptHash, receiptHash)
        XCTAssertEqual(receipt?.payload.uaid, uaid)
        XCTAssertEqual(receipt?.payload.execution.programId, "identifier_lookup_retail")
        XCTAssertEqual(receipt?.payload.execution.programDigest, programDigest)
        XCTAssertEqual(receipt?.payload.execution.verificationMode, "signed")
        XCTAssertEqual(receipt?.payload.execution.outputHash, outputHash)
        XCTAssertEqual(receipt?.payload.execution.associatedDataHash, associatedDataHash)
        XCTAssertEqual(receipt?.payload.execution.executedAtMs, 7)
        XCTAssertEqual(receipt?.payload.execution.expiresAtMs, 77)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetIdentifierClaimByReceiptHashAsync() async throws {
        let accountId = try canonicalOwnerLiteral()
        StubURLProtocol.handler = { request in
            XCTAssertEqual(
                request.url?.path,
                "/v1/identifiers/receipts/\(String(repeating: "55", count: 32))"
            )
            XCTAssertEqual(request.httpMethod, "GET")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
              "policy_id":"phone#retail",
              "opaque_id":"opaque:\(String(repeating: "44", count: 32))",
              "receipt_hash":"\(String(repeating: "55", count: 32))",
              "uaid":"uaid:\(String(repeating: "66", count: 31))67",
              "account_id":"\(accountId)",
              "verified_at_ms":7
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let claim = try await makeClient().getIdentifierClaimByReceiptHash(String(repeating: "55", count: 32))
        XCTAssertEqual(claim?.policyId, "phone#retail")
        XCTAssertEqual(claim?.accountId, accountId)
        XCTAssertEqual(claim?.verifiedAtMs, 7)
    }

    func testIdentifierNormalizationCanonicalizesPhoneAndEmail() throws {
        XCTAssertEqual(
            try ToriiIdentifierNormalization.phoneE164.normalize(" +1 (555) 123-4567 ", field: "phone"),
            "+15551234567"
        )
        XCTAssertEqual(
            try ToriiIdentifierNormalization.emailAddress.normalize(" Alice.Example@Example.COM ", field: "email"),
            "alice.example@example.com"
        )
        XCTAssertEqual(
            try ToriiIdentifierNormalization.accountNumber.normalize(" gb82-west-1234 ", field: "account"),
            "GB82WEST1234"
        )
    }

    func testIdentifierBfvEnvelopeBuilderProducesDeterministicCiphertext() throws {
        let policy = ToriiIdentifierPolicySummary(
            policyId: "string#retail",
            owner: try canonicalOwnerLiteral(),
            active: true,
            normalization: .exact,
            resolverPublicKey: "ed25519:ed0120" + String(repeating: "11", count: 32),
            backend: "bfv-affine-sha3-256-v1",
            inputEncryption: "bfv-v1",
            inputEncryptionPublicParameters: nil,
            inputEncryptionPublicParametersDecoded: ToriiIdentifierBfvPublicParameters(
                parameters: ToriiIdentifierBfvParameters(
                    polynomialDegree: 8,
                    plaintextModulus: 257,
                    ciphertextModulus: 16_842_752,
                    decompositionBaseLog: 12
                ),
                publicKey: ToriiIdentifierBfvPublicKey(
                    b: [11_472_226, 15_791_131, 10_301_391, 6_321_610, 502_045, 1_948_157, 5_332_249, 12_641_494],
                    a: [3_503_246, 2_379_264, 12_091_019, 30_169, 15_804_162, 8_155_629, 2_418_997, 3_003_107]
                ),
                maxInputBytes: 3
            ),
            ramFheProfile: nil,
            proofVerifier: nil,
            note: nil
        )
        let seedHex = "00112233445566778899AABBCCDDEEFF00112233445566778899AABBCCDDEEFF"
        let expected =
            "4e52543000001042e5b988077612440e4cd45673596b00b0040000000000004887a2a6d485fb5100a804000000000000040000000000000020010000000000008800000000000000080000000000000008000000000000002cab6c00000000000800000000000000440e92000000000008000000000000005a25000000000000080000000000000049671100000000000800000000000000bd3e2300000000000800000000000000403d85000000000008000000000000005619f900000000000800000000000000bd73fc0000000000880000000000000008000000000000000800000000000000ed884300000000000800000000000000dc21b000000000000800000000000000fe7c50000000000008000000000000001639a3000000000008000000000000006b979b00000000000800000000000000ddd4410000000000080000000000000052086600000000000800000000000000ee13ae00000000002001000000000000880000000000000008000000000000000800000000000000d96d690000000000080000000000000092060e0000000000080000000000000034077500000000000800000000000000dcc4190000000000080000000000000062ea230000000000080000000000000055ef0a00000000000800000000000000ac52d400000000000800000000000000e945790000000000880000000000000008000000000000000800000000000000f3214400000000000800000000000000caedd2000000000008000000000000001cfb5b00000000000800000000000000d26e660000000000080000000000000016ec0e000000000008000000000000003cee83000000000008000000000000006d7ef900000000000800000000000000c2fbbb00000000002001000000000000880000000000000008000000000000000800000000000000c9c7eb00000000000800000000000000c8c04800000000000800000000000000ef1e8700000000000800000000000000aed22c000000000008000000000000006021990000000000080000000000000035ac8c00000000000800000000000000d24393000000000008000000000000008a206d0000000000880000000000000008000000000000000800000000000000407ded00000000000800000000000000d79c3400000000000800000000000000a0332c0000000000080000000000000091fe5700000000000800000000000000543de8000000000008000000000000005eb9df00000000000800000000000000a7c213000000000008000000000000006e03c20000000000200100000000000088000000000000000800000000000000080000000000000003d654000000000008000000000000005c874400000000000800000000000000567ab50000000000080000000000000007273100000000000800000000000000ff6d0a00000000000800000000000000077466000000000008000000000000006c1c1a000000000008000000000000006f4fc200000000008800000000000000080000000000000008000000000000002f884f0000000000080000000000000041b0a100000000000800000000000000caf929000000000008000000000000005848730000000000080000000000000061909200000000000800000000000000f5f5dd00000000000800000000000000435a3b000000000008000000000000009a9f690000000000"

        XCTAssertEqual(try policy.encryptInput("ab", seedHex: seedHex), expected)
        let request = try policy.encryptedRequest(
            input: "ab",
            outputOpening: sampleOpening(),
            seedHex: seedHex
        )
        XCTAssertEqual(request.policyId, "string#retail")
        XCTAssertEqual(request.encryptedInputHex, expected)
    }

    func testIdentifierBfvEnvelopeBuilderMatchesLiveJsVector() throws {
        let fixtureURL = irohaSwiftPackageRootURL()
            .appendingPathComponent("Fixtures/js_email_identifier_request.json")
        let fixtureData = try Data(contentsOf: fixtureURL)
        let fixture = try JSONSerialization.jsonObject(with: fixtureData) as? [String: Any]
        let expected = try XCTUnwrap(fixture?["encryptedInput"] as? String)

        let policy = ToriiIdentifierPolicySummary(
            policyId: "email#retail",
            owner: try canonicalOwnerLiteral(),
            active: true,
            normalization: .emailAddress,
            resolverPublicKey: "ed01208FC2E4882B20ABCCBFADB4E44268206E187AEB235A51252F159B3B24D5BB6661",
            backend: "bfv-programmed-sha3-256-v1",
            inputEncryption: "bfv-v1",
            inputEncryptionPublicParameters: nil,
            inputEncryptionPublicParametersDecoded: ToriiIdentifierBfvPublicParameters(
                parameters: ToriiIdentifierBfvParameters(
                    polynomialDegree: 64,
                    plaintextModulus: 256,
                    ciphertextModulus: 4_503_599_627_370_496,
                    decompositionBaseLog: 12
                ),
                publicKey: ToriiIdentifierBfvPublicKey(
                    b: [
                        121937970585568, 2077422026028227, 500805327165639, 2424373013208231,
                        1243826623687677, 3764723070138803, 2777853678689092, 4148792190743456,
                        832919354056448, 1078220173904611, 1449102004009053, 2553195729187374,
                        4121823210086138, 1314721746498746, 2081320919861598, 1293550100235769,
                        752052855416432, 2560964795529688, 4373758947250140, 302739621553461,
                        2576363806012840, 3992909986948675, 468471023959674, 403186621067672,
                        2412531771816291, 1151008441392236, 4235659218269462, 3632712073230975,
                        1570131697783046, 2686064869757573, 868982827285377, 4024361324590714,
                        2720840185948756, 4035919674038070, 1768439826701200, 1795998257831299,
                        3146057215641308, 4427306182373160, 431902047897329, 4103953196264316,
                        252052014793937, 4481957945412857, 313876785458221, 502488979381506,
                        2254533341218653, 378630418191746, 3949757926731121, 3205345961759607,
                        4403697458699262, 1051260385144426, 3165025408388444, 2971268616428220,
                        1438933110049424, 221886932655998, 760759893336199, 2366379419062310,
                        3808463564396841, 3404172544660443, 3109880358158474, 977074504388190,
                        3693464878032463, 4157741571468524, 4422156359690334, 3136084645017157
                    ],
                    a: [
                        1757685446086860, 3389246144977851, 568120110213016, 3749195222357958,
                        505425731783661, 1653917459200990, 3991392281498110, 2498385903989296,
                        2202458345522039, 665774489520137, 3431324343332235, 2757156726851470,
                        3945284631206095, 4260357308916675, 3556193440561259, 528988422073873,
                        3556053776248327, 231714661150035, 4000422747863537, 4440124990980577,
                        1151102360936999, 1203423089979213, 3714754289019569, 365230193721200,
                        4121105395019879, 906275922098612, 2167568203585419, 902141125979404,
                        1847406449459084, 1974477488630907, 3986975207909980, 2303086024281801,
                        1799110714207632, 1984353349506261, 3868774043831877, 3439432886790299,
                        2603619075693252, 2329149836785785, 3805700285192206, 4000022950860341,
                        2467812426805913, 491688654005352, 3108228703212131, 3552150340822500,
                        3495862320984036, 2457966307381587, 654204939969134, 3247840319357347,
                        1494057235954141, 4259088215794420, 3588894761760921, 2147790385334894,
                        2062768833373357, 3953764458945290, 1442228637461419, 3551539910829634,
                        3571737697974589, 3660975499942543, 1729481054172766, 4367395767819851,
                        43579440603412, 3935477944038421, 4132857811135436, 903532232777036
                    ]
                ),
                maxInputBytes: 63
            ),
            ramFheProfile: nil,
            proofVerifier: nil,
            note: nil
        )
        let seedHex = "00112233445566778899AABBCCDDEEFF00112233445566778899AABBCCDDEEFF"
        let actual = try policy.encryptInput("ios.bankb.live@example.com", seedHex: seedHex)

        if actual != expected {
            let mismatchIndex = {
                var index = 0
                for (lhs, rhs) in zip(actual, expected) {
                    if lhs != rhs { return index }
                    index += 1
                }
                return min(actual.count, expected.count)
            }()
            XCTFail("mismatch at offset \(mismatchIndex) actual=\(actual.dropFirst(mismatchIndex).prefix(64)) expected=\(expected.dropFirst(mismatchIndex).prefix(64))")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitTransactionAsync() async throws {
        StubURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/node/capabilities":
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, self.nodeCapabilitiesBody())
            case "/v1/pipeline/transactions":
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-norito")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), ToriiWireFormatPreference.noritoPreferred.acceptHeader)
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 202,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let body = """
                {"payload":{"tx_hash":"abc","submitted_at_ms":1,"submitted_at_height":2,"signer":"signer"},"signature":"deadbeef"}
                """.data(using: .utf8)!
                return (response, body)
            default:
                XCTFail("unexpected request: \(request.url?.path ?? "")")
                let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            }
        }

        let payload = try await makeClient().submitTransaction(data: Data([0x00]))
        XCTAssertEqual(payload?.hash, "abc")
        XCTAssertEqual(payload?.payload.submittedAtMs, 1)
        XCTAssertEqual(payload?.payload.submittedAtHeight, 2)
        XCTAssertEqual(payload?.payload.signer, "signer")
        XCTAssertEqual(payload?.signature, "deadbeef")
    }

    func testSubmitTransactionResponseDecodesCanonicalJsonReceiptFields() throws {
        let body = """
        {
          "payload": {
            "tx_hash": "abc",
            "entrypoint_hash": "entry",
            "signed_transaction_hash": "signed",
            "submitted_at_ms": "1",
            "submitted_at_height": "2",
            "signer": {
              "algorithm": "ed25519",
              "payload": "node-key"
            }
          },
          "signature": {
            "algorithm": "ed25519",
            "payload": "receipt-signature"
          }
        }
        """.data(using: .utf8)!

        let receipt = try JSONDecoder().decode(ToriiSubmitTransactionResponse.self, from: body)

        XCTAssertEqual(receipt.hash, "abc")
        XCTAssertEqual(receipt.payload.entrypointHash, "entry")
        XCTAssertEqual(receipt.payload.signedTransactionHash, "signed")
        XCTAssertEqual(receipt.payload.signerValue["algorithm"], .string("ed25519"))
        XCTAssertEqual(receipt.payload.signerValue["payload"], .string("node-key"))
        XCTAssertEqual(receipt.signatureValue["algorithm"], .string("ed25519"))
        XCTAssertEqual(receipt.signatureValue["payload"], .string("receipt-signature"))
        XCTAssertEqual(receipt.payload.signer, #"{"algorithm":"ed25519","payload":"node-key"}"#)
        XCTAssertEqual(receipt.signature, #"{"algorithm":"ed25519","payload":"receipt-signature"}"#)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitTransactionJsonUsesConfiguredWirePreference() async throws {
        StubURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/node/capabilities":
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, self.nodeCapabilitiesBody())
            case "/v1/pipeline/transactions":
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), ToriiWireFormatPreference.jsonOnly.acceptHeader)
                XCTAssertEqual(self.bodyData(from: request), Data("{\"version\":1,\"content\":{}}".utf8))
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 202,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let body = """
                {"payload":{"tx_hash":"json","submitted_at_ms":1,"submitted_at_height":2,"signer":"json-signer"},"signature":"cafe"}
                """.data(using: .utf8)!
                return (response, body)
            default:
                XCTFail("unexpected request: \(request.url?.path ?? "")")
                let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            }
        }

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let client = ToriiClient(baseURL: URL(string: "https://example.test")!,
                                 session: session,
                                 wireFormatPreference: .jsonOnly)

        let payload = try await client.submitTransaction(jsonData: Data("{\"version\":1,\"content\":{}}".utf8))
        XCTAssertEqual(payload?.hash, "json")
        XCTAssertEqual(payload?.payload.signer, "json-signer")
        XCTAssertEqual(payload?.signature, "cafe")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitTransactionEntrypointAsync() async throws {
        StubURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/node/capabilities":
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, self.nodeCapabilitiesBody())
            case "/v1/pipeline/transaction-entrypoints":
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-norito")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), ToriiWireFormatPreference.noritoPreferred.acceptHeader)
                XCTAssertEqual(self.bodyData(from: request), Data([0xAA, 0xBB]))
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 202,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let body = """
                {"payload":{"tx_hash":"entry","submitted_at_ms":3,"submitted_at_height":4,"signer":"entry-signer"},"signature":"feedface"}
                """.data(using: .utf8)!
                return (response, body)
            default:
                XCTFail("unexpected request: \(request.url?.path ?? "")")
                let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            }
        }

        let payload = try await makeClient().submitTransactionEntrypoint(data: Data([0xAA, 0xBB]))
        XCTAssertEqual(payload?.hash, "entry")
        XCTAssertEqual(payload?.payload.submittedAtMs, 3)
        XCTAssertEqual(payload?.payload.submittedAtHeight, 4)
        XCTAssertEqual(payload?.payload.signer, "entry-signer")
        XCTAssertEqual(payload?.signature, "feedface")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitTransactionRejectCodeHeaderSurfaced() async throws {
        StubURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/node/capabilities":
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, self.nodeCapabilitiesBody())
            case "/v1/pipeline/transactions":
                XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-norito")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), ToriiWireFormatPreference.noritoPreferred.acceptHeader)
                let headers = [
                    "Content-Type": "application/json",
                    "x-iroha-reject-code": "PRTRY:TX_SIGNATURE_MISSING"
                ]
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 400,
                                               httpVersion: nil,
                                               headerFields: headers)!
                let body = """
                {"message":"failed to accept transaction"}
                """.data(using: .utf8)!
                return (response, body)
            default:
                XCTFail("unexpected request: \(request.url?.path ?? "")")
                let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            }
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
            XCTAssertEqual(message, "failed to accept transaction")
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitTransactionRejectsMismatchedDataModelVersion() async throws {
        StubURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/node/capabilities":
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, self.nodeCapabilitiesBody(dataModelVersion: 9))
            case "/v1/pipeline/transactions":
                XCTFail("transaction submitted with data model mismatch")
                let response = HTTPURLResponse(url: request.url!, statusCode: 500, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            default:
                XCTFail("unexpected request: \(request.url?.path ?? "")")
                let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            }
        }

        do {
            _ = try await makeClient().submitTransaction(data: Data([0x02]))
            XCTFail("expected data model mismatch")
        } catch let error as ToriiClientError {
            guard case let .dataModelMismatch(expected, actual) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(expected, ToriiNodeCapabilities.expectedDataModelVersion)
            XCTAssertEqual(actual, 9)
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitTransactionRejectsMissingSignedTransactionSchemaHash() async throws {
        StubURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/node/capabilities":
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, self.nodeCapabilitiesBody(signedTransactionSchemaHashHex: nil))
            case "/v1/pipeline/transactions":
                XCTFail("transaction submitted with missing schema hash")
                let response = HTTPURLResponse(url: request.url!, statusCode: 500, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            default:
                XCTFail("unexpected request: \(request.url?.path ?? "")")
                let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            }
        }

        do {
            _ = try await makeClient().submitTransaction(data: Data([0x03]))
            XCTFail("expected schema mismatch")
        } catch let error as ToriiClientError {
            guard case let .transactionSchemaMismatch(expected, actual) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(expected, ToriiNodeCapabilities.expectedSignedTransactionSchemaHashHex)
            XCTAssertNil(actual)
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitTransactionRejectsInvalidSignedTransactionSchemaHash() async throws {
        StubURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/node/capabilities":
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, self.nodeCapabilitiesBody(signedTransactionSchemaHashHex: "ABC123"))
            case "/v1/pipeline/transactions":
                XCTFail("transaction submitted with invalid schema hash")
                let response = HTTPURLResponse(url: request.url!, statusCode: 500, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            default:
                XCTFail("unexpected request: \(request.url?.path ?? "")")
                let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            }
        }

        do {
            _ = try await makeClient().submitTransaction(data: Data([0x04]))
            XCTFail("expected schema mismatch")
        } catch let error as ToriiClientError {
            guard case let .transactionSchemaMismatch(expected, actual) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(expected, ToriiNodeCapabilities.expectedSignedTransactionSchemaHashHex)
            XCTAssertEqual(actual, "ABC123")
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitTransactionRejectsMismatchedSignedTransactionSchemaHash() async throws {
        let mismatchedHash = "00000000000000000000000000000000"
        StubURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/node/capabilities":
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, self.nodeCapabilitiesBody(signedTransactionSchemaHashHex: mismatchedHash))
            case "/v1/pipeline/transactions":
                XCTFail("transaction submitted with schema hash mismatch")
                let response = HTTPURLResponse(url: request.url!, statusCode: 500, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            default:
                XCTFail("unexpected request: \(request.url?.path ?? "")")
                let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            }
        }

        do {
            _ = try await makeClient().submitTransaction(data: Data([0x05]))
            XCTFail("expected schema mismatch")
        } catch let error as ToriiClientError {
            guard case let .transactionSchemaMismatch(expected, actual) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(expected, ToriiNodeCapabilities.expectedSignedTransactionSchemaHashHex)
            XCTAssertEqual(actual, mismatchedHash)
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitTransactionRejectsMissingCapabilitiesProbe() async throws {
        let lock = NSLock()
        var paths: [String] = []
        StubURLProtocol.handler = { request in
            let path = request.url?.path ?? ""
            lock.lock()
            paths.append(path)
            lock.unlock()
            switch path {
            case "/v1/node/capabilities":
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 404,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "text/plain"])!
                return (response, Data("missing".utf8))
            case "/v1/pipeline/transactions":
                XCTFail("transaction submitted after missing capabilities")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 500,
                                               httpVersion: nil,
                                               headerFields: nil)!
                return (response, Data())
            default:
                XCTFail("unexpected request: \(path)")
                let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            }
        }

        do {
            _ = try await makeClient().submitTransaction(data: Data([0x06]))
            XCTFail("expected missing capabilities to reject submission")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, _, _) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(code, 404)
        } catch {
            XCTFail("unexpected error: \(error)")
        }
        XCTAssertEqual(paths, ["/v1/node/capabilities"])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitTransactionRejectsRateLimitedCapabilitiesProbe() async throws {
        let lock = NSLock()
        var paths: [String] = []
        StubURLProtocol.handler = { request in
            let path = request.url?.path ?? ""
            lock.lock()
            paths.append(path)
            lock.unlock()
            switch path {
            case "/v1/node/capabilities":
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 429,
                                               httpVersion: nil,
                                               headerFields: ["Retry-After": "1"])!
                return (response, Data("rate limited".utf8))
            case "/v1/pipeline/transactions":
                XCTFail("transaction submitted after rate-limited capabilities")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 500,
                                               httpVersion: nil,
                                               headerFields: nil)!
                return (response, Data())
            default:
                XCTFail("unexpected request: \(path)")
                let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            }
        }

        do {
            _ = try await makeClient().submitTransaction(data: Data([0x07]))
            XCTFail("expected rate-limited capabilities to reject submission")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, _, _) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(code, 429)
        } catch {
            XCTFail("unexpected error: \(error)")
        }
        XCTAssertEqual(paths, ["/v1/node/capabilities"])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitTransactionRejectsServerErrorCapabilitiesProbe() async throws {
        let lock = NSLock()
        var paths: [String] = []
        StubURLProtocol.handler = { request in
            let path = request.url?.path ?? ""
            lock.lock()
            paths.append(path)
            lock.unlock()
            switch path {
            case "/v1/node/capabilities":
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 502,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "text/plain"])!
                return (response, Data("bad gateway".utf8))
            case "/v1/pipeline/transactions":
                XCTFail("transaction submitted after failed capabilities")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 500,
                                               httpVersion: nil,
                                               headerFields: nil)!
                return (response, Data())
            default:
                XCTFail("unexpected request: \(path)")
                let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            }
        }

        do {
            _ = try await makeClient().submitTransaction(data: Data([0x08]))
            XCTFail("expected failed capabilities to reject submission")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, _, _) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(code, 502)
        } catch {
            XCTFail("unexpected error: \(error)")
        }
        XCTAssertEqual(paths, ["/v1/node/capabilities"])
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
                "policy": [
                    "ws_max_sessions": 50,
                    "session_ttl_ms": 60000,
                    "relay_enabled": true,
                    "relay_strategy": "broadcast",
                    "relay_effective_strategy": "local_only",
                    "relay_p2p_attached": false,
                    "p2p_ttl_hops": 2
                ],
                "frames_in_total": 11,
                "frames_out_total": 12,
                "ciphertext_total": 13,
                "dedupe_drops_total": 1,
                "buffer_drops_total": 2,
                "plaintext_control_drops_total": 3,
                "monotonic_drops_total": 4,
                "sequence_violation_closes_total": 5,
                "role_direction_mismatch_total": 6,
                "ping_miss_total": 7,
                "p2p_rebroadcasts_total": 8,
                "p2p_rebroadcast_skipped_total": 9,
                "p2p_auth_failures_total": 10,
                "p2p_ttl_drops_total": 11,
                "p2p_unknown_session_drops_total": 12,
                "p2p_session_claims_in_total": 13,
                "p2p_session_claims_installed_total": 14,
                "p2p_session_claim_conflicts_total": 15,
                "p2p_role_consumed_total": 16,
                "p2p_session_terminated_total": 17
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
        XCTAssertEqual(status.policy?.relayStrategy, "broadcast")
        XCTAssertEqual(status.policy?.p2pTtlHops, 2)
        XCTAssertEqual(status.sequenceViolationClosesTotal, 5)
        XCTAssertEqual(status.p2pAuthFailuresTotal, 10)
        XCTAssertEqual(status.p2pSessionClaimsInstalledTotal, 14)
        XCTAssertEqual(status.p2pSessionTerminatedTotal, 17)
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
                "token_management": "token-management",
                "token_relay": "token-relay",
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
        XCTAssertEqual(response.tokenManagement, "token-management")
        XCTAssertEqual(response.tokenRelay, "token-relay")
        XCTAssertEqual(response.extra["custom"], .bool(true))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testDeleteConnectSessionHandles404() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/connect/session/sid-1")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer token-management")
            let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: [:])!
            return (response, Data())
        }
        let deleted = try await makeClient().deleteConnectSession(sid: "sid-1", tokenManagement: "token-management")
        XCTAssertFalse(deleted)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetVpnProfileDeserializesNativeLeaseFields() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/vpn/profile")
            let payload: [String: Any] = [
                "available": true,
                "supported_exit_classes": ["standard"],
                "default_exit_class": "standard",
                "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
                "lease_secs": "900",
                "route_pushes": ["0.0.0.0/0"],
                "excluded_routes": ["10.0.0.0/8"],
                "dns_servers": ["1.1.1.1"],
                "tunnel_addresses": ["10.208.0.2/32"],
                "mtu_bytes": 1280,
                "meter_family": "vpn-standard",
                "display_billing_label": "standard vpn",
                "fee_asset_id": "xor#universal.universal",
                "escrow_account_id": "vpn_escrow",
                "operator_account_id": "vpn_operator",
                "lease_fee_nanos": "1000000",
                "settlement_grace_secs": 60,
                "flow_label_bits": 24,
                "padding_budget_ms": 15,
                "relay_tls_spki_sha256_hex": String(repeating: "ab", count: 32)
            ]
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, try JSONSerialization.data(withJSONObject: payload))
        }
        let profile = try await makeClient().getVpnProfile()
        XCTAssertTrue(profile.available)
        XCTAssertEqual(profile.feeAssetId, "xor#universal.universal")
        XCTAssertEqual(profile.leaseFeeNanos, 1_000_000)
        XCTAssertEqual(profile.flowLabelBits, 24)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRegisterAndUnregisterPushDeviceSignCanonicalBody() async throws {
        let auth = ToriiCanonicalRequestAuth(accountId: "alice",
                                             privateKey: Data(repeating: 7, count: 32),
                                             timestampMs: 1_700_000_000_010,
                                             nonce: "push-nonce-1")
        var callCount = 0
        StubURLProtocol.handler = { request in
            callCount += 1
            XCTAssertEqual(request.url?.path, "/v1/notify/devices")
            XCTAssertEqual(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerAccount), "alice")
            XCTAssertEqual(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature) == nil, false)
            let body = self.bodyJSON(from: request)
            XCTAssertEqual(body["account_id"] as? String, "alice")
            XCTAssertEqual(body["platform"] as? String, "FCM")
            XCTAssertEqual(body["token"] as? String, "token-1")
            XCTAssertEqual(body["topics"] as? [String], ["activity"])
            if callCount == 1 {
                XCTAssertEqual(request.httpMethod, "POST")
            } else {
                XCTAssertEqual(request.httpMethod, "DELETE")
            }
            let response = HTTPURLResponse(url: request.url!, statusCode: 202, httpVersion: nil,
                                           headerFields: [:])!
            return (response, Data())
        }
        let client = makeClient()
        let request = ToriiPushDeviceRequest(accountId: " alice ",
                                             platform: "FCM",
                                             token: " token-1 ",
                                             topics: [" activity "])
        try await client.registerPushDevice(request, canonicalAuth: auth)
        let deleteAuth = ToriiCanonicalRequestAuth(accountId: "alice",
                                                   privateKey: Data(repeating: 7, count: 32),
                                                   timestampMs: 1_700_000_000_011,
                                                   nonce: "push-nonce-2")
        try await client.unregisterPushDevice(request, canonicalAuth: deleteAuth)
        XCTAssertEqual(callCount, 2)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testCreateVpnQuoteSignsAndDeserializesOpenLeaseInstruction() async throws {
        let meteringKey = String(repeating: "ab", count: 32)
        let quoteId = String(repeating: "cd", count: 32)
        let auth = ToriiCanonicalRequestAuth(accountId: "alice",
                                             privateKey: Data(repeating: 7, count: 32),
                                             timestampMs: 1_700_000_000_000,
                                             nonce: "nonce-1")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/vpn/quotes")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerAccount), "alice")
            XCTAssertEqual(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerTimestampMs), "1700000000000")
            XCTAssertEqual(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerNonce), "nonce-1")
            XCTAssertNotNil(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature))
            let body = self.bodyJSON(from: request)
            XCTAssertEqual(body["exit_class"] as? String, "standard")
            XCTAssertEqual(body["metering_public_key_hex"] as? String, meteringKey)
            let payload: [String: Any] = [
                "quote_id": quoteId,
                "lease_id_hex": quoteId,
                "session_id_hex": String(repeating: "ef", count: 16),
                "payment_reference": quoteId,
                "account_id": "alice",
                "exit_class": "standard",
                "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
                "lease_secs": 900,
                "quote_expires_at_ms": 1_700_000_900_000,
                "fee_asset_id": "xor#universal.universal",
                "escrow_account_id": "vpn_escrow",
                "operator_account_id": "vpn_operator",
                "lease_fee_nanos": "1000000",
                "route_pushes": [],
                "excluded_routes": [],
                "dns_servers": ["1.1.1.1"],
                "tunnel_addresses": ["10.208.0.2/32"],
                "mtu_bytes": 1280,
                "meter_family": "vpn-standard",
                "flow_label_bits": 24,
                "padding_budget_ms": 15,
                "relay_tls_spki_sha256_hex": String(repeating: "12", count: 32),
                "metering_public_key_hex": meteringKey,
                "open_lease_instruction": [
                    "wire_id": "OpenVpnLeaseEscrow",
                    "payload_hex": "abcd"
                ],
                "tx_instructions": [
                    ["wire_id": "OpenVpnLeaseEscrow", "payload_hex": "abcd"]
                ]
            ]
            let response = HTTPURLResponse(url: request.url!, statusCode: 201, httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, try JSONSerialization.data(withJSONObject: payload))
        }
        let quote = try await makeClient().createVpnQuote(
            ToriiVpnQuoteCreateRequest(exitClass: "standard", meteringPublicKeyHex: "0x\(meteringKey)"),
            canonicalAuth: auth
        )
        XCTAssertEqual(quote.quoteId, quoteId)
        XCTAssertEqual(quote.openLeaseInstruction?.wireId, "OpenVpnLeaseEscrow")
        XCTAssertEqual(quote.txInstructions.count, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testCreateAndGetVpnSessionUseQuotePaymentAndMeteringKey() async throws {
        let quoteId = String(repeating: "11", count: 32)
        let paymentHash = String(repeating: "22", count: 32)
        let meteringKey = String(repeating: "33", count: 32)
        let helperTicketHex = "5356504e48543100" + String(repeating: "00", count: 248)
        var callCount = 0
        StubURLProtocol.handler = { request in
            callCount += 1
            if callCount == 1 {
                XCTAssertEqual(request.url?.path, "/v1/vpn/sessions")
                XCTAssertEqual(request.httpMethod, "POST")
                let body = self.bodyJSON(from: request)
                XCTAssertEqual(body["quote_id"] as? String, quoteId)
                XCTAssertEqual(body["payment_tx_hash"] as? String, paymentHash)
                XCTAssertEqual(body["metering_public_key_hex"] as? String, meteringKey)
            } else {
                XCTAssertEqual(request.url?.path, "/v1/vpn/sessions/\(quoteId)")
                XCTAssertEqual(request.httpMethod, "GET")
            }
            let payload: [String: Any] = [
                "session_id": quoteId,
                "account_id": "alice",
                "exit_class": "standard",
                "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
                "lease_secs": 900,
                "expires_at_ms": 1_700_000_900_000,
                "connected_at_ms": 1_700_000_000_000,
                "meter_family": "vpn-standard",
                "quote_id": quoteId,
                "payment_reference": quoteId,
                "payment_tx_hash": paymentHash,
                "fee_asset_id": "xor#universal.universal",
                "escrow_account_id": "vpn_escrow",
                "operator_account_id": "vpn_operator",
                "lease_fee_nanos": 1_000_000,
                "flow_label_bits": 24,
                "padding_budget_ms": 15,
                "relay_tls_spki_sha256_hex": NSNull(),
                "route_pushes": [],
                "excluded_routes": [],
                "dns_servers": [],
                "tunnel_addresses": ["10.208.0.2/32"],
                "mtu_bytes": 1280,
                "helper_ticket_hex": helperTicketHex,
                "bytes_in": "0",
                "bytes_out": "0",
                "status": "active"
            ]
            let response = HTTPURLResponse(url: request.url!, statusCode: callCount == 1 ? 201 : 200, httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, try JSONSerialization.data(withJSONObject: payload))
        }
        let client = makeClient()
        let created = try await client.createVpnSession(
            ToriiVpnSessionCreateRequest(quoteId: "0x\(quoteId)",
                                         paymentTransactionHash: paymentHash,
                                         meteringPublicKeyHex: meteringKey)
        )
        let fetched = try await client.getVpnSession(sessionId: quoteId)
        XCTAssertEqual(created.sessionId, quoteId)
        XCTAssertEqual(fetched?.paymentTransactionHash, paymentHash)
        XCTAssertEqual(created.helperTicketHex, helperTicketHex)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitListAndDeleteVpnReceiptsExposeSettlementInstruction() async throws {
        let quoteId = String(repeating: "44", count: 32)
        let settle: [String: Any] = [
            "wire_id": "SettleVpnLease",
            "payload_hex": "cafe"
        ]
        let receiptPayload: [String: Any] = [
            "session_id": quoteId,
            "account_id": "alice",
            "exit_class": "standard",
            "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
            "meter_family": "vpn-standard",
            "connected_at_ms": 1,
            "disconnected_at_ms": 2,
            "duration_ms": 1,
            "bytes_in": 10,
            "bytes_out": "20",
            "status": "settled",
            "receipt_source": "relay",
            "quote_id": quoteId,
            "payment_tx_hash": String(repeating: "55", count: 32),
            "fee_asset_id": "xor#universal.universal",
            "escrow_account_id": "vpn_escrow",
            "operator_account_id": "vpn_operator",
            "lease_fee_nanos": "1000000",
            "earned_fee_nanos": 700000,
            "refunded_fee_nanos": 300000,
            "lease_id_hex": quoteId,
            "settle_lease_instruction": settle,
            "tx_instructions": [settle]
        ]
        var callCount = 0
        StubURLProtocol.handler = { request in
            callCount += 1
            if callCount == 1 {
                XCTAssertEqual(request.url?.path, "/v1/vpn/receipts")
                XCTAssertEqual(request.httpMethod, "POST")
                let body = self.bodyJSON(from: request)
                XCTAssertEqual(body["relay_receipt_hex"] as? String, "abcd")
                XCTAssertEqual(body["client_voucher_hex"] as? String, "1234")
                XCTAssertEqual(body["lease_id_hex"] as? String, quoteId)
                let response = HTTPURLResponse(url: request.url!, statusCode: 201, httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, try JSONSerialization.data(withJSONObject: receiptPayload))
            }
            if callCount == 2 {
                XCTAssertEqual(request.httpMethod, "GET")
                let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, try JSONSerialization.data(withJSONObject: ["items": [receiptPayload], "total": "1"]))
            }
            XCTAssertEqual(request.httpMethod, "DELETE")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, try JSONSerialization.data(withJSONObject: receiptPayload))
        }
        let client = makeClient()
        let submitted = try await client.submitVpnReceipt(
            ToriiVpnReceiptSubmitRequest(relayReceiptHex: "0xabcd",
                                         clientVoucherHex: "1234",
                                         leaseIdHex: quoteId)
        )
        let list = try await client.listVpnReceipts()
        let deleted = try await client.deleteVpnSession(sessionId: quoteId)
        XCTAssertEqual(submitted.settleLeaseInstruction?.wireId, "SettleVpnLease")
        XCTAssertEqual(list.items.first?.earnedFeeNanos, 700_000)
        XCTAssertEqual(deleted?.txInstructions.first?.payloadHex, "cafe")
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
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB/assets")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset":"66owaQmAQMuHxPzxUN3bqZ6FJfDa","account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","scope":"global","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!, session: session)

        let balances = try await sdk.getAssets(
            accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            asset: nil
        )
        XCTAssertEqual(balances.count, 1)
        XCTAssertEqual(balances.first?.asset, roseAssetDefinitionId)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsTrimsAndEncodesAccountLiteral() async throws {
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB/assets")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset":"66owaQmAQMuHxPzxUN3bqZ6FJfDa","account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","scope":"global","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(
            accountId: "  sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB  ",
            asset: nil
        )
        XCTAssertEqual(balances.count, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsRejectsPercentEscapedAccountLiteral() async {
        await XCTAssertThrowsErrorAsync(
            try await makeClient().getAssets(
                accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB%2Fsorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
            ),
            expectation: { _ in }
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsEncodesAssetSelectorFilter() async throws {
        let assetId = roseAssetDefinitionId
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB/assets")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let assetFilter = components?.queryItems?.first(where: { $0.name == "asset" })?.value
            XCTAssertEqual(assetFilter, assetId)
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset":"\(assetId)","account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","scope":"global","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB", asset: assetId)
        XCTAssertEqual(balances.count, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionsEncodesAccountLiteral() async throws {
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB/transactions")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"items":[{"entrypoint_hash":"hash","authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","timestamp_ms":1,"result_ok":true}],"total":1}
            """.data(using: .utf8)!
            return (response, body)
        }

        let transactions = try await makeClient().getTransactions(accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(transactions.total, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionsEncodesAssetIdFilter() async throws {
        let assetId = self.encodedRoseAssetID
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB/transactions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let assetFilter = components?.queryItems?.first(where: { $0.name == "asset_id" })?.value
            XCTAssertEqual(assetFilter, assetId)
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"items":[{"entrypoint_hash":"hash","authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","timestamp_ms":1,"result_ok":true}],"total":1}
            """.data(using: .utf8)!
            return (response, body)
        }

        let transactions = try await makeClient().getTransactions(accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB", assetDefinitionId: assetId)
        XCTAssertEqual(transactions.total, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerAccountQrDecodesResponse() async throws {
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/explorer/accounts/sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB/qr")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            XCTAssertNil(components?.queryItems)
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "canonical_id":"i105example",
                "literal":"i105example",
                "network_prefix":0,
                "error_correction":"M",
                "modules":192,
                "qr_version":5,
                "svg":"<svg viewBox=\\"0 0 192 192\\"></svg>"
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let qr = try await makeClient().getExplorerAccountQr(accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(qr.canonicalId, "i105example")
        XCTAssertEqual(qr.literal, "i105example")
        XCTAssertEqual(qr.networkPrefix, 0)
        XCTAssertEqual(qr.modules, 192)
        XCTAssertEqual(qr.qrVersion, 5)
        XCTAssertTrue(qr.svg.contains("<svg"))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerAccountQrAcceptsAccountAliasPathLiteral() async throws {
        let alias = "operator@banka.universal"
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/explorer/accounts/operator@banka.universal/qr")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "canonical_id":"i105example",
                "literal":"operator@banka.universal",
                "network_prefix":0,
                "error_correction":"M",
                "modules":192,
                "qr_version":5,
                "svg":"<svg viewBox=\\"0 0 192 192\\"></svg>"
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let qr = try await makeClient().getExplorerAccountQr(accountId: alias)
        XCTAssertEqual(qr.literal, alias)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerAccountQrDecodesAlternativeLiteral() async throws {
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/explorer/accounts/sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB/qr")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            XCTAssertNil(components?.queryItems)
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "canonical_id":"i105example",
                "literal":"soraexample",
                "network_prefix":1206,
                "error_correction":"M",
                "modules":192,
                "qr_version":6,
                "svg":"<svg class=\\"qr\\"></svg>"
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let qr = try await makeClient().getExplorerAccountQr(accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(qr.literal, "soraexample")
        XCTAssertEqual(qr.qrVersion, 6)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerInstructionsEncodesQueryAndDecodesResponse() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let queryItems = components?.queryItems ?? []
            let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["page"], "2")
            XCTAssertEqual(query["per_page"], "25")
            XCTAssertEqual(query["account"], "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
            XCTAssertEqual(query["authority"], "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
            XCTAssertEqual(query["transaction_hash"], "deadbeef")
            XCTAssertEqual(query["transaction_status"], "Committed")
            XCTAssertEqual(query["block"], "5")
            XCTAssertEqual(query["kind"], "Transfer")
            XCTAssertEqual(query["asset_id"], "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "pagination": {"page":2,"per_page":25,"total_pages":1,"total_items":1},
                "items": [
                    {
                        "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                        "created_at":"2025-01-01T00:00:00Z",
                        "kind":"Transfer",
                        "r#box":{
                            "scale":"0xdead",
                            "json":{
                                "kind":"Transfer",
                                "payload":{
                                    "Asset":{
                                        "source":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                        "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                        "object":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                        "value":"10"
                                    }
                                },
                                "wire_id":"10",
                                "encoded":"beef"
                            }
                        },
                        "transaction_hash":"hash",
                        "transaction_status":"Committed",
                        "block":5,
                        "index":0
                    }
                ]
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let params = ToriiExplorerInstructionsParams(page: 2,
                                                     perPage: 25,
                                                     account: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                     authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                     transactionHash: "deadbeef",
                                                     transactionStatus: "Committed",
                                                     block: 5,
                                                     kind: "Transfer",
                                                     assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        let page = try await makeClient().getExplorerInstructions(params: params)
        XCTAssertEqual(page.pagination.page, 2)
        XCTAssertEqual(page.pagination.perPage, 25)
        XCTAssertEqual(page.pagination.totalItems, 1)
        XCTAssertEqual(page.items.count, 1)
        let item = page.items[0]
        XCTAssertEqual(item.kind, "Transfer")
        XCTAssertEqual(item.transactionHash, "hash")
        XCTAssertEqual(item.box.scale, "0xdead")
        guard case let .object(payload) = item.box.json else {
            return XCTFail("Expected instruction box json payload to be an object.")
        }
        guard case let .string(kind) = payload["kind"] else {
            return XCTFail("Expected instruction box json to contain a kind string.")
        }
        XCTAssertEqual(kind, "Transfer")
    }

    func testExplorerTransferDetailsParsesAsset() throws {
        let json = """
        {
            "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            "created_at":"2025-01-01T00:00:00Z",
            "kind":"Transfer",
            "r#box":{
                "scale":"0x00",
                "json":{
                    "kind":"Transfer",
                    "payload":{
                        "variant":"Asset",
                        "value":{
                            "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                            "object":"10",
                            "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                        }
                    },
                    "wire_id":"10",
                    "encoded":"beef"
                }
            },
            "transaction_hash":"hash",
            "transaction_status":"Committed",
            "block":1,
            "index":0
        }
        """
        let item = try JSONDecoder().decode(ToriiExplorerInstructionItem.self, from: Data(json.utf8))
        guard let details = item.transferDetails() else {
            return XCTFail("Expected transfer details.")
        }
        switch details {
        case .asset(let asset):
            XCTAssertEqual(asset.destinationAccountId, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
            XCTAssertEqual(asset.amount, "10")
            XCTAssertEqual(asset.senderAccountId, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
            XCTAssertEqual(asset.assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            XCTAssertNil(details.role(for: "sorauﾛ1PgﾉﾀXﾖnWｱﾊｷﾕﾈjｷZﾖrﾅxｲWﾔﾀﾘYヰﾍxｺﾀﾃﾛｽfﾖ2Gｲ8P3LSM"))
            XCTAssertEqual(details.role(for: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"), .sender)
            XCTAssertEqual(details.role(for: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"), .receiver)
            XCTAssertTrue(details.involvesAccount("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"))
            XCTAssertTrue(details.involvesAssetDefinition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"))
            XCTAssertFalse(details.involvesAssetDefinition("61CtjvNd9T3THAR65GsMVHr82Bjc"))
        case .assetBatch:
            XCTFail("Expected asset transfer details.")
        }
    }

    func testExplorerTransferDetailsParsesAssetBatch() throws {
        let json = """
        {
            "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            "created_at":"2025-01-01T00:00:00Z",
            "kind":"Transfer",
            "r#box":{
                "scale":"0x00",
                "json":{
                    "kind":"Transfer",
                    "payload":{
                        "variant":"AssetBatch",
                        "value":{
                            "entries":[
                                {
                                    "from":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "to":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                    "asset_definition":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "amount":"5"
                                },
                                {
                                    "from":"sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM",
                                    "to":"sorauﾛ1PﾜKNﾗ7ｼｺa2WｸｼﾒﾐQﾎbｺﾄocﾆﾁヰJaｱbg6sｾgｲﾖPfX7WAWRY",
                                    "asset_definition":"61CtjvNd9T3THAR65GsMVHr82Bjc",
                                    "amount":"2"
                                }
                            ]
                        }
                    },
                    "wire_id":"10",
                    "encoded":"beef"
                }
            },
            "transaction_hash":"hash",
            "transaction_status":"Committed",
            "block":1,
            "index":0
        }
        """
        let item = try JSONDecoder().decode(ToriiExplorerInstructionItem.self, from: Data(json.utf8))
        guard let details = item.transferDetails() else {
            return XCTFail("Expected transfer details.")
        }
        switch details {
        case .asset:
            XCTFail("Expected batch transfer details.")
        case .assetBatch(let entries):
            XCTAssertEqual(entries.count, 2)
            XCTAssertEqual(entries[0].senderAccountId, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
            XCTAssertEqual(entries[0].receiverAccountId, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
            XCTAssertEqual(entries[0].assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            XCTAssertEqual(entries[0].amount, "5")
            XCTAssertEqual(entries[1].senderAccountId, "sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM")
            XCTAssertEqual(entries[1].receiverAccountId, "sorauﾛ1PﾜKNﾗ7ｼｺa2WｸｼﾒﾐQﾎbｺﾄocﾆﾁヰJaｱbg6sｾgｲﾖPfX7WAWRY")
            XCTAssertEqual(entries[1].assetDefinitionId, "61CtjvNd9T3THAR65GsMVHr82Bjc")
            XCTAssertEqual(entries[1].amount, "2")
            XCTAssertEqual(details.role(for: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"), .sender)
            XCTAssertEqual(details.role(for: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"), .receiver)
            XCTAssertTrue(details.involvesAccount("sorauﾛ1PﾜKNﾗ7ｼｺa2WｸｼﾒﾐQﾎbｺﾄocﾆﾁヰJaｱbg6sｾgｲﾖPfX7WAWRY"))
            XCTAssertTrue(details.involvesAssetDefinition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"))
            XCTAssertTrue(details.involvesAssetDefinition("61CtjvNd9T3THAR65GsMVHr82Bjc"))
            XCTAssertFalse(details.involvesAssetDefinition("5ywNgSPQ5KyuQh7SwaZmwMW4GTXu"))
        }
    }

    func testExplorerTransferRecordsFiltersByAccountAndAssetDefinition() throws {
        let json = """
        {
            "pagination": {"page":1,"per_page":10,"total_pages":1,"total_items":2},
            "items": [
                {
                    "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object":"10",
                                    "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            },
                            "wire_id":"10",
                            "encoded":"beef"
                        }
                    },
                    "transaction_hash":"hash1",
                    "transaction_status":"Committed",
                    "block":1,
                    "index":0
                },
                {
                    "authority":"sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"AssetBatch",
                                "value":{
                                    "entries":[
                                        {
                                            "from":"sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM",
                                            "to":"sorauﾛ1PﾜKNﾗ7ｼｺa2WｸｼﾒﾐQﾎbｺﾄocﾆﾁヰJaｱbg6sｾgｲﾖPfX7WAWRY",
                                            "asset_definition":"61CtjvNd9T3THAR65GsMVHr82Bjc",
                                            "amount":"2"
                                        }
                                    ]
                                }
                            },
                            "wire_id":"10",
                            "encoded":"beef"
                        }
                    },
                    "transaction_hash":"hash2",
                    "transaction_status":"Committed",
                    "block":1,
                    "index":1
                }
            ]
        }
        """
        let page = try JSONDecoder().decode(ToriiExplorerInstructionsPage.self, from: Data(json.utf8))
        XCTAssertEqual(page.transferRecords().count, 2)
        XCTAssertEqual(page.transferRecords(matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D").count, 1)
        XCTAssertEqual(page.transferRecords(matchingAccount: "sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM").count, 1)
        XCTAssertEqual(page.transferRecords(assetDefinitionId: "61CtjvNd9T3THAR65GsMVHr82Bjc").count, 1)
        XCTAssertEqual(page.transferRecords(assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM").count, 1)
        XCTAssertEqual(page.transferRecords(matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                            assetDefinitionId: "61CtjvNd9T3THAR65GsMVHr82Bjc").count, 0)
    }

    func testExplorerTransferSummariesDeriveDirection() throws {
        let json = """
        {
            "pagination": {"page":1,"per_page":10,"total_pages":1,"total_items":1},
            "items": [
                {
                    "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object":"10",
                                    "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            },
                            "wire_id":"10",
                            "encoded":"beef"
                        }
                    },
                    "transaction_hash":"hash1",
                    "transaction_status":"Committed",
                    "block":1,
                    "index":0
                }
            ]
        }
        """
        let page = try JSONDecoder().decode(ToriiExplorerInstructionsPage.self, from: Data(json.utf8))
        let summaries = page.transferSummaries(matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
        XCTAssertEqual(summaries.count, 1)
        let summary = summaries[0]
        XCTAssertEqual(summary.direction, .incoming)
        XCTAssertEqual(summary.senderAccountId, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(summary.receiverAccountId, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
        XCTAssertEqual(summary.assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        XCTAssertEqual(summary.amount, "10")
        XCTAssertTrue(summary.isIncoming)
        XCTAssertFalse(summary.isOutgoing)
        XCTAssertFalse(summary.isSelfTransfer)
        XCTAssertEqual(summary.transferIndex, 0)
        XCTAssertEqual(summary.id, "hash1|0|0")
        XCTAssertEqual(summary.direction(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"), .incoming)
        XCTAssertEqual(summary.counterpartyAccountId(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"), "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertTrue(summary.isIncoming(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"))
        XCTAssertFalse(summary.isOutgoing(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"))
        XCTAssertFalse(summary.isSelfTransfer(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"))
        XCTAssertEqual(summary.signedAmount(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"), "+10")
        XCTAssertEqual(summary.signedAmount(relativeTo: "sorauﾛ1PgﾉﾀXﾖnWｱﾊｷﾕﾈjｷZﾖrﾅxｲWﾔﾀﾘYヰﾍxｺﾀﾃﾛｽfﾖ2Gｲ8P3LSM"), "10")
    }

    func testExplorerTransferSummariesDeriveSelfTransfer() throws {
        let json = """
        {
            "pagination": {"page":1,"per_page":10,"total_pages":1,"total_items":1},
            "items": [
                {
                    "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object":"10",
                                    "destination":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"
                                }
                            },
                            "wire_id":"10",
                            "encoded":"beef"
                        }
                    },
                    "transaction_hash":"hash1",
                    "transaction_status":"Committed",
                    "block":1,
                    "index":0
                }
            ]
        }
        """
        let page = try JSONDecoder().decode(ToriiExplorerInstructionsPage.self, from: Data(json.utf8))
        let summaries = page.transferSummaries(matchingAccount: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(summaries.count, 1)
        let summary = summaries[0]
        XCTAssertEqual(summary.direction, .selfTransfer)
        XCTAssertTrue(summary.isSelfTransfer)
        XCTAssertFalse(summary.isIncoming)
        XCTAssertFalse(summary.isOutgoing)
        XCTAssertEqual(summary.direction(relativeTo: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"), .selfTransfer)
        XCTAssertEqual(summary.counterpartyAccountId(relativeTo: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"), "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertNil(summary.counterpartyAccountId(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"))
        XCTAssertTrue(summary.isSelfTransfer(relativeTo: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"))
        XCTAssertFalse(summary.isIncoming(relativeTo: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"))
        XCTAssertFalse(summary.isOutgoing(relativeTo: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"))
        XCTAssertEqual(summary.signedAmount(relativeTo: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"), "10")
    }

    func testTransferSummarySignedAmountPreservesExistingSign() {
        let outgoing = ToriiExplorerTransferSummary(transactionHash: "hash1",
                                                    block: 1,
                                                    createdAt: "2025-01-01T00:00:00Z",
                                                    status: "Committed",
                                                    authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                    instructionIndex: 0,
                                                    senderAccountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                    receiverAccountId: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                                    amount: "-10",
                                                    direction: .outgoing,
                                                    kind: "Transfer",
                                                    transferIndex: 0)
        XCTAssertEqual(outgoing.signedAmount(relativeTo: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"), "-10")

        let incoming = ToriiExplorerTransferSummary(transactionHash: "hash2",
                                                    block: 1,
                                                    createdAt: "2025-01-01T00:00:00Z",
                                                    status: "Committed",
                                                    authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                    instructionIndex: 0,
                                                    senderAccountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                    receiverAccountId: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                                    amount: "+10",
                                                    direction: .incoming,
                                                    kind: "Transfer",
                                                    transferIndex: 0)
        XCTAssertEqual(incoming.signedAmount(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"), "+10")
    }

    func testExplorerTransferSummariesAssignBatchIndices() throws {
        let json = """
        {
            "pagination": {"page":1,"per_page":10,"total_pages":1,"total_items":1},
            "items": [
                {
                    "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"AssetBatch",
                                "value":{
                                    "entries":[
                                        {
                                            "from":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                            "to":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                            "asset_definition":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                            "amount":"5"
                                        },
                                        {
                                            "from":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                            "to":"sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM",
                                            "asset_definition":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                            "amount":"7"
                                        }
                                    ]
                                }
                            },
                            "wire_id":"10",
                            "encoded":"beef"
                        }
                    },
                    "transaction_hash":"hash1",
                    "transaction_status":"Committed",
                    "block":1,
                    "index":0
                }
            ]
        }
        """
        let page = try JSONDecoder().decode(ToriiExplorerInstructionsPage.self, from: Data(json.utf8))
        let summaries = page.transferSummaries()
        XCTAssertEqual(summaries.count, 2)
        XCTAssertEqual(summaries[0].transferIndex, 0)
        XCTAssertEqual(summaries[1].transferIndex, 1)
        XCTAssertEqual(summaries[0].id, "hash1|0|0")
        XCTAssertEqual(summaries[1].id, "hash1|0|1")
    }

    // MARK: - Mint / Burn instruction parsing

    func testExplorerMintInstructionParsedAsSummary() throws {
        // Real Mint response from Iroha explorer API
        let json = """
        {
            "pagination":{"page":1,"per_page":20,"total_pages":1,"total_items":1},
            "items":[{
                "authority":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                "created_at":"2026-03-17T14:07:35.576Z",
                "kind":"Mint",
                "r#box":{
                    "json":{
                        "encoded":"deadbeef",
                        "kind":"Mint",
                        "payload":{
                            "value":{
                                "destination":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                "object":"500"
                            },
                            "variant":"Asset"
                        },
                        "wire_id":"iroha_data_model::isi::mint_burn::MintBox"
                    }
                },
                "transaction_hash":"9bca4ad18474058cbbad5bbc49e5e11cf58d90fc28b094ac8f8963a5116fdff5",
                "transaction_status":"Committed",
                "block":17,
                "index":0
            }]
        }
        """
        let page = try JSONDecoder().decode(ToriiExplorerInstructionsPage.self, from: Data(json.utf8))
        XCTAssertEqual(page.items.count, 1)

        let item = page.items[0]
        XCTAssertEqual(item.kind, "Mint")

        // transferDetails() should parse Mint payloads
        let details = item.transferDetails()
        XCTAssertNotNil(details, "transferDetails() should parse Mint instructions")

        // Generate summaries relative to the mint recipient
        let accountId = "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
        let summaries = page.transferSummaries(relativeTo: accountId)
        XCTAssertEqual(summaries.count, 1, "Mint should produce exactly 1 summary")

        let summary = summaries[0]
        XCTAssertEqual(summary.kind, "Mint")
        XCTAssertEqual(summary.amount, "500")
        XCTAssertEqual(summary.direction, .incoming, "Mint should always be incoming")
        XCTAssertEqual(summary.status, "Committed")
        XCTAssertEqual(summary.transactionHash, "9bca4ad18474058cbbad5bbc49e5e11cf58d90fc28b094ac8f8963a5116fdff5")
        // assetDefinitionId should remain in canonical Base58 form
        XCTAssertFalse(summary.assetDefinitionId.isEmpty)
        XCTAssertFalse(summary.assetDefinitionId.contains(":"),
                       "assetDefinitionId should decode to unprefixed Base58 form")
        // receiverAccountId should be extracted from the canonical asset ID
        XCTAssertFalse(summary.receiverAccountId.isEmpty, "receiverAccountId should not be empty")
    }

    func testCanonicalMintDestinationStaysCanonical() throws {
        let publicKey = Data(repeating: 0x44, count: 32)
        let address = try AccountAddress.fromAccount(publicKey: publicKey, algorithm: "ed25519")
        let accountId = try address.toI105(networkPrefix: 0x02F1)
        let literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#\(accountId)"
        XCTAssertEqual(try OfflineNorito.canonicalAssetIdLiteral(literal), literal)
        XCTAssertEqual(OfflineNorito.assetDefinitionIdFromLiteral(literal), "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
    }

    func testCanonicalAssetDefinitionLiteralRemainsCanonical() {
        XCTAssertEqual(
            OfflineNorito.assetDefinitionIdFromLiteral("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
            "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
        )
    }

    func testMalformedPublicAssetLiteralReturnsNilDefinition() {
        XCTAssertNil(OfflineNorito.assetDefinitionIdFromLiteral("62Fk4FPcMuLvW5QjDGNF2a4jAmjM#not-an-account"))
    }

    func testDecodeAccountIdReadsNoritoStringField() throws {
        let publicKey = Data(repeating: 0x33, count: 32)
        let address = try AccountAddress.fromAccount(publicKey: publicKey, algorithm: "ed25519")
        let accountId = try address.toI105(networkPrefix: 0x02F1)
        let encodedLength = withUnsafeBytes(of: UInt64(accountId.utf8.count).littleEndian) { Data($0) }
        let encoded = encodedLength + Data(accountId.utf8)
        XCTAssertEqual(try OfflineNorito.decodeAccountId(encoded), accountId)
    }

    func testExplorerBurnInstructionParsedAsSummary() throws {
        let json = """
        {
            "pagination":{"page":1,"per_page":10,"total_pages":1,"total_items":1},
            "items":[{
                "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                "created_at":"2025-06-01T10:00:00Z",
                "kind":"Burn",
                "r#box":{
                    "json":{
                        "encoded":"00",
                        "kind":"Burn",
                        "payload":{
                            "value":{
                                "destination":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                "object":"25"
                            },
                            "variant":"Asset"
                        },
                        "wire_id":"iroha_data_model::isi::mint_burn::BurnBox"
                    }
                },
                "transaction_hash":"burn_hash_1",
                "transaction_status":"Committed",
                "block":5,
                "index":0
            }]
        }
        """
        let page = try JSONDecoder().decode(ToriiExplorerInstructionsPage.self, from: Data(json.utf8))
        XCTAssertEqual(page.items.count, 1)

        let item = page.items[0]
        XCTAssertEqual(item.kind, "Burn")

        let details = item.transferDetails()
        XCTAssertNotNil(details, "transferDetails() should parse Burn instructions")

        let summaries = page.transferSummaries(relativeTo: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(summaries.count, 1, "Burn should produce exactly 1 summary")

        let summary = try XCTUnwrap(summaries.first)
        XCTAssertEqual(summary.kind, "Burn")
        XCTAssertEqual(summary.amount, "25")
        XCTAssertEqual(summary.direction, .outgoing, "Burn should always be outgoing")
    }

    func testExplorerMixedKindsAllParsed() throws {
        // Page with Transfer + Mint + unknown kind — all parseable ones should be included
        let json = """
        {
            "pagination":{"page":1,"per_page":10,"total_pages":1,"total_items":3},
            "items":[
                {
                    "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                    "object":"10"
                                }
                            },
                            "wire_id":"10",
                            "encoded":"beef"
                        }
                    },
                    "transaction_hash":"transfer_hash",
                    "transaction_status":"Committed",
                    "block":1,
                    "index":0
                },
                {
                    "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at":"2025-01-02T00:00:00Z",
                    "kind":"Mint",
                    "r#box":{
                        "json":{
                            "encoded":"00",
                            "kind":"Mint",
                            "payload":{
                                "value":{
                                    "destination":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "object":"100"
                                },
                                "variant":"Asset"
                            },
                            "wire_id":"MintBox"
                        }
                    },
                    "transaction_hash":"mint_hash",
                    "transaction_status":"Committed",
                    "block":2,
                    "index":0
                },
                {
                    "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at":"2025-01-03T00:00:00Z",
                    "kind":"SetKeyValue",
                    "r#box":{
                        "json":{
                            "encoded":"00",
                            "kind":"SetKeyValue",
                            "payload":{
                                "variant":"Domain",
                                "value":{"key":"foo","value":"bar"}
                            },
                            "wire_id":"SetKeyValueBox"
                        }
                    },
                    "transaction_hash":"skv_hash",
                    "transaction_status":"Committed",
                    "block":3,
                    "index":0
                }
            ]
        }
        """
        let page = try JSONDecoder().decode(ToriiExplorerInstructionsPage.self, from: Data(json.utf8))
        XCTAssertEqual(page.items.count, 3, "All 3 items should decode")

        let summaries = page.transferSummaries()
        // Transfer + Mint should parse, SetKeyValue should be skipped
        XCTAssertEqual(summaries.count, 2, "Transfer + Mint should produce summaries, SetKeyValue should be skipped")
        XCTAssertEqual(summaries[0].kind, "Transfer")
        XCTAssertEqual(summaries[1].kind, "Mint")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerInstructionsCompletion() {
        let expectation = expectation(description: "explorer-instructions")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            XCTAssertNil(components?.queryItems)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"pagination":{"page":1,"per_page":10,"total_pages":0,"total_items":0},"items":[]}
            """.data(using: .utf8)!
            return (response, body)
        }

        _ = makeClient().getExplorerInstructions { result in
            switch result {
            case .success(let page):
                XCTAssertEqual(page.items.count, 0)
                XCTAssertEqual(page.pagination.totalItems, 0)
            case .failure(let error):
                XCTFail("Unexpected error: \(error)")
            }
            expectation.fulfill()
        }

        waitForExpectations(timeout: 2.0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransfersFiltersByAccount() async throws {
        let assetIdFilter = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let queryItems = components?.queryItems ?? []
            let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["asset_id"], assetIdFilter)
            XCTAssertEqual(query["kind"], "Transfer")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "pagination": {"page":1,"per_page":10,"total_pages":1,"total_items":2},
                "items": [
                    {
                        "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                        "created_at":"2025-01-01T00:00:00Z",
                        "kind":"Transfer",
                        "r#box":{
                            "scale":"0x00",
                            "json":{
                                "kind":"Transfer",
                                "payload":{
                                    "variant":"Asset",
                                    "value":{
                                        "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                        "object":"10",
                                        "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                    }
                                },
                                "wire_id":"10",
                                "encoded":"beef"
                            }
                        },
                        "transaction_hash":"hash1",
                        "transaction_status":"Committed",
                        "block":1,
                        "index":0
                    },
                    {
                        "authority":"sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM",
                        "created_at":"2025-01-01T00:00:00Z",
                        "kind":"Transfer",
                        "r#box":{
                            "scale":"0x00",
                            "json":{
                                "kind":"Transfer",
                                "payload":{
                                    "variant":"AssetBatch",
                                    "value":{
                                        "entries":[
                                            {
                                                "from":"sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM",
                                                "to":"sorauﾛ1PﾜKNﾗ7ｼｺa2WｸｼﾒﾐQﾎbｺﾄocﾆﾁヰJaｱbg6sｾgｲﾖPfX7WAWRY",
                                                "asset_definition":"61CtjvNd9T3THAR65GsMVHr82Bjc",
                                                "amount":"2"
                                            }
                                        ]
                                    }
                                },
                                "wire_id":"10",
                                "encoded":"beef"
                            }
                        },
                        "transaction_hash":"hash2",
                        "transaction_status":"Committed",
                        "block":1,
                        "index":1
                    }
                ]
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let transfers = try await makeClient().getExplorerTransfers(matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                                    assetDefinitionId: assetIdFilter)
        XCTAssertEqual(transfers.count, 1)
        XCTAssertEqual(transfers.first?.instruction.transactionHash, "hash1")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransactionsEncodesQueryAndDecodesResponse() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/transactions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let queryItems = components?.queryItems ?? []
            let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["page"], "2")
            XCTAssertEqual(query["per_page"], "25")
            XCTAssertEqual(query["authority"], "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
            XCTAssertEqual(query["block"], "5")
            XCTAssertEqual(query["status"], "Committed")
            XCTAssertEqual(query["asset_id"], "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "pagination": {"page":2,"per_page":25,"total_pages":1,"total_items":1},
                "items": [
                    {
                        "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                        "hash":"deadbeef",
                        "block":5,
                        "created_at":"2025-01-01T00:00:00Z",
                        "executable":"Instructions",
                        "status":"Committed"
                    }
                ]
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let params = ToriiExplorerTransactionsParams(page: 2,
                                                     perPage: 25,
                                                     authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                     block: 5,
                                                     status: "Committed",
                                                     assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        let page = try await makeClient().getExplorerTransactions(params: params)
        XCTAssertEqual(page.pagination.page, 2)
        XCTAssertEqual(page.pagination.perPage, 25)
        XCTAssertEqual(page.items.count, 1)
        XCTAssertEqual(page.items.first?.hash, "deadbeef")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransactionsCompletion() {
        let expectation = expectation(description: "explorer-transactions")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/transactions")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"pagination":{"page":1,"per_page":10,"total_pages":1,"total_items":0},"items":[]}
            """.data(using: .utf8)!
            return (response, body)
        }

        _ = makeClient().getExplorerTransactions { result in
            switch result {
            case .success(let page):
                XCTAssertEqual(page.items.count, 0)
                XCTAssertEqual(page.pagination.totalItems, 0)
            case .failure(let error):
                XCTFail("Unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 2.0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetContractActivityEncodesQueryAndDecodesResponse() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/activity")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let queryItems = components?.queryItems ?? []
            let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["limit"], "20")
            XCTAssertEqual(query["offset"], "40")
            XCTAssertEqual(query["authority"], "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
            XCTAssertEqual(query["contract_alias"], "benefits::paynet")
            XCTAssertEqual(query["contract_entrypoint"], "claim")
            XCTAssertEqual(query["since_timestamp_ms"], "1000")
            XCTAssertEqual(query["until_timestamp_ms"], "2000")
            XCTAssertEqual(query["result_ok"], "true")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "items": [
                    {
                        "authority": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                        "timestamp_ms": 1234,
                        "entrypoint_hash": "0xabc",
                        "result_ok": true,
                        "contract_address": "cntr:deadbeef",
                        "contract_alias": "benefits::paynet",
                        "contract_entrypoint": "claim",
                        "contract_payload": {"amount": 500},
                        "gas_asset_id": "gas#paynet",
                        "fee_sponsor": "sponsor@paynet",
                        "gas_limit": 50000
                    }
                ],
                "total": 1
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let params = ToriiContractActivityParams(
            limit: 20,
            offset: 40,
            authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            contractAlias: "benefits::paynet",
            contractEntrypoint: "claim",
            sinceTimestampMs: 1000,
            untilTimestampMs: 2000,
            resultOk: true
        )
        let list = try await makeClient().getContractActivity(params: params)
        XCTAssertEqual(list.total, 1)
        XCTAssertEqual(list.items.first?.contractAlias, "benefits::paynet")
        XCTAssertEqual(list.items.first?.contractEntrypoint, "claim")
        XCTAssertEqual(list.items.first?.timestampMs, 1234)
        guard case let .object(payload)? = list.items.first?.contractPayload,
              case let .number(amount)? = payload["amount"] else {
            return XCTFail("Expected numeric contract payload.")
        }
        XCTAssertEqual(amount, 500)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetContractEventsEncodesQueryAndDecodesResponse() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/events")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let queryItems = components?.queryItems ?? []
            let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["limit"], "10")
            XCTAssertEqual(query["offset"], "5")
            XCTAssertEqual(query["contract_alias"], "benefits::paynet")
            XCTAssertEqual(query["module"], "benefits")
            XCTAssertEqual(query["event_kind"], "spend")
            XCTAssertEqual(query["participant"], "merchant@paynet")
            XCTAssertEqual(query["asset_id"], "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            XCTAssertEqual(query["provenance"], "derived")
            XCTAssertEqual(query["result_ok"], "false")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "items": [
                    {
                        "event_id": "0xabc:0",
                        "schema_version": 1,
                        "provenance": "derived",
                        "authority": "beneficiary@paynet",
                        "timestamp_ms": 1234,
                        "tx_hash_hex": "0xabc",
                        "block_height": 7,
                        "block_hash_hex": "0xblock",
                        "result_ok": false,
                        "contract_address": "cntr:deadbeef",
                        "contract_alias": "benefits::paynet",
                        "module": "benefits",
                        "event_kind": "spend",
                        "participants": ["beneficiary@paynet", "merchant@paynet"],
                        "asset_ids": ["62Fk4FPcMuLvW5QjDGNF2a4jAmjM"],
                        "numeric_fields": {"amount": 125},
                        "payload": {"amount": 125, "merchant_account": "merchant@paynet"},
                        "gas_asset_id": "gas#paynet",
                        "fee_sponsor": "sponsor@paynet",
                        "gas_limit": 70000
                    }
                ],
                "total": 1
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let params = ToriiContractEventParams(
            limit: 10,
            offset: 5,
            contractAlias: "benefits::paynet",
            module: "benefits",
            eventKind: "spend",
            participant: "merchant@paynet",
            assetId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
            provenance: "derived",
            resultOk: false
        )
        let list = try await makeClient().getContractEvents(params: params)
        XCTAssertEqual(list.total, 1)
        XCTAssertEqual(list.items.first?.eventId, "0xabc:0")
        XCTAssertEqual(list.items.first?.participants ?? [], ["beneficiary@paynet", "merchant@paynet"])
        XCTAssertEqual(list.items.first?.assetIds ?? [], ["62Fk4FPcMuLvW5QjDGNF2a4jAmjM"])
        guard case let .number(amount)? = list.items.first?.numericFields?["amount"] else {
            return XCTFail("Expected numeric field.")
        }
        XCTAssertEqual(amount, 125)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetContractEventsCompletion() {
        let expectation = expectation(description: "contract-events")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/events")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = #"{"items":[],"total":0}"#.data(using: .utf8)!
            return (response, body)
        }

        _ = makeClient().getContractEvents { result in
            switch result {
            case .success(let list):
                XCTAssertEqual(list.items.count, 0)
                XCTAssertEqual(list.total, 0)
            case .failure(let error):
                XCTFail("Unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 2.0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransactionDetailEncodesQueryAndDecodesResponse() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/transactions/deadbeef")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            XCTAssertNil(components?.queryItems)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                "hash":"deadbeef",
                "block":5,
                "created_at":"2025-01-01T00:00:00Z",
                "executable":"Instructions",
                "status":"Committed",
                "rejection_reason": null,
                "metadata": {"note":"demo"},
                "nonce": 7,
                "signature": "0xabc",
                "time_to_live": {"ms": 60000}
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let detail = try await makeClient().getExplorerTransactionDetail(hashHex: "deadbeef")
        XCTAssertEqual(detail.hash, "deadbeef")
        XCTAssertEqual(detail.signature, "0xabc")
        XCTAssertEqual(detail.timeToLive?.ms, 60000)
        guard case let .object(meta) = detail.metadata else {
            return XCTFail("Expected metadata object.")
        }
        guard case let .string(note) = meta["note"] else {
            return XCTFail("Expected metadata.note string.")
        }
        XCTAssertEqual(note, "demo")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerInstructionDetailEncodesQueryAndDecodesResponse() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions/deadbeef/3")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            XCTAssertNil(components?.queryItems)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                "created_at":"2025-01-01T00:00:00Z",
                "kind":"Transfer",
                "r#box":{
                    "scale":"0x00",
                    "json":{
                        "kind":"Transfer",
                        "payload":{
                            "variant":"Asset",
                            "value":{
                                "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                "object":"5",
                                "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                            }
                        }
                    }
                },
                "transaction_hash":"hash1",
                "transaction_status":"Committed",
                "block":10,
                "index":3
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let item = try await makeClient().getExplorerInstructionDetail(hashHex: "deadbeef",
                                                                       index: 3)
        XCTAssertEqual(item.kind, "Transfer")
        XCTAssertEqual(item.transactionHash, "hash1")
        XCTAssertEqual(item.index, 3)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransactionDetailCompletion() {
        let expectation = expectation(description: "explorer-transaction-detail")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/transactions/deadbeef")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                "hash":"deadbeef",
                "block":5,
                "created_at":"2025-01-01T00:00:00Z",
                "executable":"Instructions",
                "status":"Committed",
                "rejection_reason": null,
                "metadata": {},
                "nonce": null,
                "signature": "0xabc",
                "time_to_live": null
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        _ = makeClient().getExplorerTransactionDetail(hashHex: "deadbeef") { result in
            switch result {
            case .success(let detail):
                XCTAssertEqual(detail.hash, "deadbeef")
                XCTAssertEqual(detail.signature, "0xabc")
            case .failure(let error):
                XCTFail("Unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 2.0)
    }

    func testExplorerRwasParamsQueryItemsEncodePaginationAndDomain() throws {
        let params = ToriiExplorerRwasParams(page: 2,
                                             perPage: 25,
                                             domain: "commodities.sora")
        let queryItems = try XCTUnwrap(params.queryItems())
        let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
        XCTAssertEqual(query["page"], "2")
        XCTAssertEqual(query["per_page"], "25")
        XCTAssertEqual(query["domain"], "commodities.sora")
    }

    func testExplorerRwaRecordDecodesNullStatusAndMetadataDefaults() throws {
        let json = """
        {
            "id":"lot-001$commodities.sora",
            "owned_by":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            "quantity":"42",
            "held_quantity":"2",
            "primary_reference":"vault://receipts/2",
            "status":null,
            "is_frozen":true,
            "metadata":null
        }
        """
        let record = try JSONDecoder().decode(ToriiExplorerRwaRecord.self, from: Data(json.utf8))
        XCTAssertEqual(record.id, "lot-001$commodities.sora")
        XCTAssertEqual(record.quantity, "42")
        XCTAssertEqual(record.heldQuantity, "2")
        XCTAssertEqual(record.primaryReference, "vault://receipts/2")
        XCTAssertNil(record.status)
        XCTAssertTrue(record.isFrozen)
        XCTAssertTrue(record.metadata.isEmpty)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerRwaDetailEncodesPathAndDecodesResponse() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/rwas/lot-001$commodities.sora")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "id":"lot-001$commodities.sora",
                "owned_by":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                "quantity":"42",
                "held_quantity":"2",
                "primary_reference":"vault://receipts/2",
                "status":null,
                "is_frozen":true,
                "metadata":null
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let detail = try await makeClient().getExplorerRwaDetail(rwaId: "lot-001$commodities.sora")
        XCTAssertEqual(detail.id, "lot-001$commodities.sora")
        XCTAssertEqual(detail.quantity, "42")
        XCTAssertEqual(detail.heldQuantity, "2")
        XCTAssertEqual(detail.primaryReference, "vault://receipts/2")
        XCTAssertNil(detail.status)
        XCTAssertTrue(detail.isFrozen)
        XCTAssertTrue(detail.metadata.isEmpty)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListRwasEncodesOptions() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/rwas")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let query = Dictionary(uniqueKeysWithValues: (components?.queryItems ?? []).map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["limit"], "25")
            XCTAssertEqual(query["offset"], "10")
            XCTAssertEqual(query["sort"], "id")
            let filterValue = try XCTUnwrap(query["filter"])
            let filterData = try XCTUnwrap(filterValue.data(using: .utf8))
            let decodedFilter = try JSONSerialization.jsonObject(with: filterData) as? [String: String]
            XCTAssertEqual(decodedFilter?["id"], "lot-001$commodities.sora")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"items":[{"id":"lot-001$commodities.sora"}],"total":1}
            """.data(using: .utf8)!
            return (response, body)
        }

        let options = ToriiListOptions(filter: .json(.object(["id": .string("lot-001$commodities.sora")])),
                                       sort: .fields(["id"]),
                                       limit: 25,
                                       offset: 10)
        let page = try await makeClient().listRwas(options: options)
        XCTAssertEqual(page.total, 1)
        XCTAssertEqual(page.items.first?.id, "lot-001$commodities.sora")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testQueryRwasPostsEnvelope() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/rwas/query")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            let body = self.bodyJSON(from: request)
            let pagination = body["pagination"] as? [String: Any]
            XCTAssertEqual(pagination?["limit"] as? Int, 10)
            XCTAssertEqual(pagination?["offset"] as? Int, 5)
            let sort = body["sort"] as? [[String: Any]]
            XCTAssertEqual(sort?.first?["key"] as? String, "id")
            XCTAssertEqual(sort?.first?["order"] as? String, "asc")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let payload = """
            {"items":[{"id":"lot-002$commodities.sora"}],"total":1}
            """.data(using: .utf8)!
            return (response, payload)
        }

        let envelope = ToriiQueryEnvelope(
            filter: .object(["id": .object(["eq": .string("lot-002$commodities.sora")])]),
            sort: [ToriiQuerySortKey(key: "id", order: .asc)],
            pagination: ToriiQueryPagination(limit: 10, offset: 5),
            fetchSize: 20
        )
        let page = try await makeClient().queryRwas(envelope)
        XCTAssertEqual(page.total, 1)
        XCTAssertEqual(page.items.first?.id, "lot-002$commodities.sora")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testIterateRwasRespectsPagingAndMaxItems() async throws {
        var observedLimits: [String] = []
        var observedOffsets: [String] = []
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/rwas")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let query = Dictionary(uniqueKeysWithValues: (components?.queryItems ?? []).map { ($0.name, $0.value ?? "") })
            if let limit = query["limit"] {
                observedLimits.append(limit)
            }
            if let offset = query["offset"] {
                observedOffsets.append(offset)
            }
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body: Data
            switch query["offset"] ?? "0" {
            case "0":
                body = """
                {"items":[{"id":"lot-001$commodities.sora"},{"id":"lot-002$commodities.sora"}],"total":4}
                """.data(using: .utf8)!
            case "2":
                body = """
                {"items":[{"id":"lot-003$commodities.sora"}],"total":4}
                """.data(using: .utf8)!
            default:
                body = #"{"items":[],"total":4}"#.data(using: .utf8)!
            }
            return (response, body)
        }

        let stream = makeClient().iterateRwas(pageSize: 2, maxItems: 3)
        var collected: [String] = []
        for try await item in stream {
            collected.append(item.id)
        }
        XCTAssertEqual(collected, ["lot-001$commodities.sora", "lot-002$commodities.sora", "lot-003$commodities.sora"])
        XCTAssertEqual(observedLimits, ["2", "1"])
        XCTAssertEqual(observedOffsets, ["0", "2"])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransactionTransfersAggregatesPages() async throws {
        let assetIdFilter = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
        let pageOne = """
        {
            "pagination": {"page":1,"per_page":1,"total_pages":2,"total_items":2},
            "items": [
                {
                    "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object":"5",
                                    "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            }
                        }
                    },
                    "transaction_hash":"deadbeef",
                    "transaction_status":"Committed",
                    "block":10,
                    "index":0
                }
            ]
        }
        """.data(using: .utf8)!

        let pageTwo = """
        {
            "pagination": {"page":2,"per_page":1,"total_pages":2,"total_items":2},
            "items": [
                {
                    "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at":"2025-01-01T00:00:01Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object":"7",
                                    "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            }
                        }
                    },
                    "transaction_hash":"deadbeef",
                    "transaction_status":"Committed",
                    "block":10,
                    "index":1
                }
            ]
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let queryItems = components?.queryItems ?? []
            let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["transaction_hash"], "deadbeef")
            XCTAssertEqual(query["kind"], "Transfer")
            XCTAssertEqual(query["asset_id"], assetIdFilter)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            switch query["page"] {
            case "1":
                return (response, pageOne)
            case "2":
                return (response, pageTwo)
            default:
                return (response, Data())
            }
        }

        let records = try await makeClient().getExplorerTransactionTransfers(hashHex: "deadbeef",
                                                                             assetDefinitionId: assetIdFilter)
        XCTAssertEqual(records.count, 2)
        XCTAssertEqual(records.first?.instruction.index, 0)
        XCTAssertEqual(records.last?.instruction.index, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransactionTransferSummariesCompletion() {
        let expectation = expectation(description: "explorer-transaction-transfer-summaries")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "pagination": {"page":1,"per_page":50,"total_pages":1,"total_items":1},
                "items": [
                    {
                        "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                        "created_at":"2025-01-01T00:00:00Z",
                        "kind":"Transfer",
                        "r#box":{
                            "scale":"0x00",
                            "json":{
                                "kind":"Transfer",
                                "payload":{
                                    "variant":"Asset",
                                    "value":{
                                        "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                        "object":"5",
                                        "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                    }
                                }
                            }
                        },
                        "transaction_hash":"deadbeef",
                        "transaction_status":"Committed",
                        "block":10,
                        "index":0
                    }
                ]
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        _ = makeClient().getExplorerTransactionTransferSummaries(hashHex: "deadbeef",
                                                                 matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D") { result in
            switch result {
            case .success(let summaries):
                XCTAssertEqual(summaries.count, 1)
                XCTAssertEqual(summaries.first?.transactionHash, "deadbeef")
                XCTAssertEqual(summaries.first?.direction, .incoming)
            case .failure(let error):
                XCTFail("Unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 2.0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransactionTransferSummariesFiltersByAssetId() async throws {
        let body = """
        {
            "pagination": {"page":1,"per_page":50,"total_pages":1,"total_items":2},
            "items": [
                {
                    "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object":"5",
                                    "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            }
                        }
                    },
                    "transaction_hash":"deadbeef",
                    "transaction_status":"Committed",
                    "block":10,
                    "index":0
                },
                {
                    "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at":"2025-01-01T00:00:01Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object":"7",
                                    "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            }
                        }
                    },
                    "transaction_hash":"deadbeef",
                    "transaction_status":"Committed",
                    "block":10,
                    "index":1
                }
            ]
        }
        """
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let queryItems = components?.queryItems ?? []
            let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["transaction_hash"], "deadbeef")
            XCTAssertEqual(query["kind"], "Transfer")
            XCTAssertEqual(query["asset_id"], "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, body)
        }

        let summaries = try await makeClient().getExplorerTransactionTransferSummaries(hashHex: "deadbeef",
                                                                                        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        XCTAssertEqual(summaries.count, 1)
        XCTAssertEqual(summaries.first?.assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerInstructionDetailCompletion() {
        let expectation = expectation(description: "explorer-instruction-detail")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions/deadbeef/0")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                "created_at":"2025-01-01T00:00:00Z",
                "kind":"Transfer",
                "r#box":{
                    "scale":"0x00",
                    "json":{
                        "kind":"Transfer",
                        "payload":{
                            "variant":"Asset",
                            "value":{
                                "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                "object":"5",
                                "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                            }
                        }
                    }
                },
                "transaction_hash":"hash1",
                "transaction_status":"Committed",
                "block":10,
                "index":0
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        _ = makeClient().getExplorerInstructionDetail(hashHex: "deadbeef", index: 0) { result in
            switch result {
            case .success(let item):
                XCTAssertEqual(item.transactionHash, "hash1")
                XCTAssertEqual(item.index, 0)
            case .failure(let error):
                XCTFail("Unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 2.0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransferSummariesFiltersByAccount() async throws {
        let assetIdFilter = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let queryItems = components?.queryItems ?? []
            let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["asset_id"], assetIdFilter)
            XCTAssertEqual(query["kind"], "Transfer")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "pagination": {"page":1,"per_page":10,"total_pages":1,"total_items":1},
                "items": [
                    {
                        "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                        "created_at":"2025-01-01T00:00:00Z",
                        "kind":"Transfer",
                        "r#box":{
                            "scale":"0x00",
                            "json":{
                                "kind":"Transfer",
                                "payload":{
                                    "variant":"Asset",
                                    "value":{
                                        "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                        "object":"10",
                                        "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                    }
                                },
                                "wire_id":"10",
                                "encoded":"beef"
                            }
                        },
                        "transaction_hash":"hash1",
                        "transaction_status":"Committed",
                        "block":1,
                        "index":0
                    }
                ]
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let summaries = try await makeClient().getExplorerTransferSummaries(matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                                            assetDefinitionId: assetIdFilter)
        XCTAssertEqual(summaries.count, 1)
        XCTAssertEqual(summaries.first?.direction, .incoming)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransferSummariesCompletion() {
        let expectation = expectation(description: "explorer-transfer-summaries")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let queryItems = components?.queryItems ?? []
            let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["kind"], "Transfer")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"pagination":{"page":1,"per_page":10,"total_pages":1,"total_items":0},"items":[]}
            """.data(using: .utf8)!
            return (response, body)
        }

        _ = makeClient().getExplorerTransferSummaries { result in
            switch result {
            case .success(let summaries):
                XCTAssertEqual(summaries.count, 0)
            case .failure(let error):
                XCTFail("Unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 2.0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAccountTransferHistoryBuildsTransferQuery() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let queryItems = components?.queryItems ?? []
            let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["kind"], "Transfer")
            XCTAssertEqual(query["page"], "3")
            XCTAssertEqual(query["per_page"], "20")
            XCTAssertEqual(query["asset_id"], "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "pagination": {"page":3,"per_page":20,"total_pages":1,"total_items":0},
                "items": []
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let summaries = try await makeClient().getAccountTransferHistory(accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                                          page: 3,
                                                                          perPage: 20,
                                                                          assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        XCTAssertEqual(summaries.count, 0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAccountTransferHistoryCompletion() {
        let expectation = expectation(description: "account-transfer-history")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"pagination":{"page":1,"per_page":10,"total_pages":1,"total_items":0},"items":[]}
            """.data(using: .utf8)!
            return (response, body)
        }

        _ = makeClient().getAccountTransferHistory(accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB") { result in
            switch result {
            case .success(let summaries):
                XCTAssertEqual(summaries.count, 0)
            case .failure(let error):
                XCTFail("Unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 2.0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionHistoryBuildsTransferQuery() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let queryItems = components?.queryItems ?? []
            let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["kind"], "Transfer")
            XCTAssertEqual(query["page"], "2")
            XCTAssertEqual(query["per_page"], "5")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "pagination": {"page":2,"per_page":5,"total_pages":1,"total_items":0},
                "items": []
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let summaries = try await makeClient().getTransactionHistory(accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                                     page: 2,
                                                                     perPage: 5)
        XCTAssertEqual(summaries.count, 0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionHistoryCompletion() {
        let expectation = expectation(description: "transaction-history")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"pagination":{"page":1,"per_page":10,"total_pages":1,"total_items":0},"items":[]}
            """.data(using: .utf8)!
            return (response, body)
        }

        _ = makeClient().getTransactionHistory(accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB") { result in
            switch result {
            case .success(let summaries):
                XCTAssertEqual(summaries.count, 0)
            case .failure(let error):
                XCTFail("Unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 2.0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testIterateAccountTransferHistoryAcrossPages() async throws {
        var callCount = 0
        StubURLProtocol.handler = { request in
            callCount += 1
            XCTAssertEqual(request.url?.path, "/v1/explorer/instructions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let queryItems = components?.queryItems ?? []
            let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["kind"], "Transfer")
            let expectedPage = callCount == 1 ? "1" : "2"
            XCTAssertEqual(query["page"], expectedPage)
            XCTAssertEqual(query["per_page"], "1")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body: String
            if callCount == 1 {
                body = """
                {
                    "pagination": {"page":1,"per_page":1,"total_pages":2,"total_items":2},
                    "items": [
                        {
                            "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                            "created_at":"2025-01-01T00:00:00Z",
                            "kind":"Transfer",
                            "r#box":{
                                "scale":"0x00",
                                "json":{
                                    "kind":"Transfer",
                                    "payload":{
                                        "variant":"Asset",
                                        "value":{
                                            "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                            "object":"10",
                                            "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                        }
                                    },
                                    "wire_id":"10",
                                    "encoded":"beef"
                                }
                            },
                            "transaction_hash":"hash1",
                            "transaction_status":"Committed",
                            "block":1,
                            "index":0
                        }
                    ]
                }
                """
            } else {
                body = """
                {
                    "pagination": {"page":2,"per_page":1,"total_pages":2,"total_items":2},
                    "items": [
                        {
                            "authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                            "created_at":"2025-01-01T00:00:01Z",
                            "kind":"Transfer",
                            "r#box":{
                                "scale":"0x00",
                                "json":{
                                    "kind":"Transfer",
                                    "payload":{
                                        "variant":"Asset",
                                        "value":{
                                            "source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                            "object":"5",
                                            "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                        }
                                    },
                                    "wire_id":"10",
                                    "encoded":"beef"
                                }
                            },
                            "transaction_hash":"hash2",
                            "transaction_status":"Committed",
                            "block":2,
                            "index":0
                        }
                    ]
                }
                """
            }
            return (response, Data(body.utf8))
        }

        var summaries: [ToriiExplorerTransferSummary] = []
        for try await summary in makeClient().iterateAccountTransferHistory(accountId: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                                            perPage: 1) {
            summaries.append(summary)
        }
        XCTAssertEqual(summaries.count, 2)
        XCTAssertEqual(summaries.first?.transactionHash, "hash1")
        XCTAssertEqual(summaries.last?.transactionHash, "hash2")
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
                    {"id":"wonderland","owned_by":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","metadata":{"theme":"demo"}}
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
        XCTAssertEqual(record.ownedBy, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
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
    func testListSubscriptionPlansEncodesParams() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/subscriptions/plans")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let query = Dictionary(uniqueKeysWithValues: (components?.queryItems ?? []).map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["provider"], "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
            XCTAssertEqual(query["limit"], "10")
            XCTAssertEqual(query["offset"], "5")
            let payload: [String: Any] = [
                "items": [
                    [
                        "plan_id": "plan#subs",
                        "plan": [
                            "provider": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                            "pricing": ["kind": "fixed"]
                        ]
                    ]
                ],
                "total": 1
            ]
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let data = try JSONSerialization.data(withJSONObject: payload)
            return (response, data)
        }

        let params = ToriiSubscriptionPlanListParams(provider: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB", limit: 10, offset: 5)
        let response = try await makeClient().listSubscriptionPlans(params: params)
        XCTAssertEqual(response.total, 1)
        let item = try XCTUnwrap(response.items.first)
        XCTAssertEqual(item.planId, "plan#subs")
        if case let .string(provider)? = item.plan["provider"] {
            XCTAssertEqual(provider, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        } else {
            XCTFail("missing plan provider")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testCreateSubscriptionPlanRejectsRemovedServerSideSigningFlow() async {
        let plan: ToriiSubscriptionPlan = [
            "provider": .string("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"),
            "billing": .object(["kind": .string("monthly")]),
            "pricing": .object(["kind": .string("fixed"), "amount": .string("120")])
        ]
        let requestBody = ToriiSubscriptionPlanCreateRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                             planId: "plan#subs",
                                                             plan: plan)
        await XCTAssertThrowsErrorAsync(try await makeClient().createSubscriptionPlan(requestBody)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(reason.contains("/v1/subscriptions/plans"))
            XCTAssertTrue(reason.contains("locally signed transaction"))
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListSubscriptionsEncodesParams() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/subscriptions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let query = Dictionary(uniqueKeysWithValues: (components?.queryItems ?? []).map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["owned_by"], "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
            XCTAssertEqual(query["provider"], "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
            XCTAssertEqual(query["status"], "active")
            XCTAssertEqual(query["limit"], "25")
            XCTAssertEqual(query["offset"], "0")
            let payload: [String: Any] = [
                "items": [
                    [
                        "subscription_id": "sub-1$subscriptions",
                        "subscription": [
                            "status": "active",
                            "plan_id": "plan#subs"
                        ],
                        "invoice": ["amount": "120"],
                        "plan": ["provider": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"]
                    ]
                ],
                "total": 1
            ]
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let data = try JSONSerialization.data(withJSONObject: payload)
            return (response, data)
        }

        let params = ToriiSubscriptionListParams(ownedBy: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                 provider: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                 status: .active,
                                                 limit: 25,
                                                 offset: 0)
        let response = try await makeClient().listSubscriptions(params: params)
        XCTAssertEqual(response.total, 1)
        let record = try XCTUnwrap(response.items.first)
        XCTAssertEqual(record.subscriptionId, "sub-1$subscriptions")
        if case let .string(status)? = record.subscription["status"] {
            XCTAssertEqual(status, "active")
        } else {
            XCTFail("missing subscription status")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testCreateSubscriptionRejectsRemovedServerSideSigningFlow() async {
        let requestBody = ToriiSubscriptionCreateRequest(authority: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                         subscriptionId: "sub-1$subscriptions",
                                                         planId: "plan#subs",
                                                         billingTriggerId: "sub-bill",
                                                         usageTriggerId: "sub-usage",
                                                         firstChargeMs: 1_704_067_200_000,
                                                         grantUsageToProvider: true)
        await XCTAssertThrowsErrorAsync(try await makeClient().createSubscription(requestBody)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(reason.contains("/v1/subscriptions"))
            XCTAssertTrue(reason.contains("locally signed transaction"))
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetSubscriptionDecodesRecord() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/subscriptions/sub-1$subscriptions")
            let payload: [String: Any] = [
                "subscription_id": "sub-1$subscriptions",
                "subscription": ["status": "active"],
                "invoice": NSNull(),
                "plan": ["provider": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"]
            ]
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let data = try JSONSerialization.data(withJSONObject: payload)
            return (response, data)
        }

        let record = try await makeClient().getSubscription(subscriptionId: "sub-1$subscriptions")
        XCTAssertEqual(record?.subscriptionId, "sub-1$subscriptions")
        if case let .string(provider)? = record?.plan?["provider"] {
            XCTAssertEqual(provider, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        } else {
            XCTFail("missing plan provider")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetSubscriptionReturnsNilFor404() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/subscriptions/sub-404$subscriptions")
            let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: [:])!
            return (response, Data())
        }
        let record = try await makeClient().getSubscription(subscriptionId: "sub-404$subscriptions")
        XCTAssertNil(record)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubscriptionActionsRejectRemovedServerSideSigningFlow() async {
        let subscriptionId = "sub-1$subscriptions"
        let client = makeClient()
        let action = ToriiSubscriptionActionRequest(authority: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
        let chargeAction = ToriiSubscriptionActionRequest(authority: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                          chargeAtMs: 1_704_067_200_000)
        let cancelAction = ToriiSubscriptionActionRequest(authority: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                          cancelMode: .periodEnd)
        let usage = ToriiSubscriptionUsageRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                  unitKey: "compute_ms",
                                                  delta: "3600000",
                                                  usageTriggerId: "sub-usage")
        let cases: [(String, @Sendable () async throws -> Void)] = [
            ("/v1/subscriptions/{subscription_id}/pause", { _ = try await client.pauseSubscription(subscriptionId: subscriptionId, requestBody: action) }),
            ("/v1/subscriptions/{subscription_id}/resume", { _ = try await client.resumeSubscription(subscriptionId: subscriptionId, requestBody: chargeAction) }),
            ("/v1/subscriptions/{subscription_id}/cancel", { _ = try await client.cancelSubscription(subscriptionId: subscriptionId, requestBody: cancelAction) }),
            ("/v1/subscriptions/{subscription_id}/keep", { _ = try await client.keepSubscription(subscriptionId: subscriptionId, requestBody: action) }),
            ("/v1/subscriptions/{subscription_id}/charge-now", { _ = try await client.chargeSubscriptionNow(subscriptionId: subscriptionId, requestBody: chargeAction) }),
            ("/v1/subscriptions/{subscription_id}/usage", { _ = try await client.recordSubscriptionUsage(subscriptionId: subscriptionId, requestBody: usage) })
        ]

        for (endpoint, operation) in cases {
            await XCTAssertThrowsErrorAsync(try await operation()) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload, got \(error)")
                }
                XCTAssertTrue(reason.contains(endpoint))
                XCTAssertTrue(reason.contains("locally signed transaction"))
            }
        }
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
                  "account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                  "label":null,
                  "assets":[{"asset_id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","asset_definition_id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","quantity":"500"}]
                }
              ]
            }
          ]
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/accounts/uaid%3A\(uaidHex)/portfolio"))
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let response = try await makeClient().getUaidPortfolio(uaid: "  UAID:\(uaidHex.uppercased())  ")
        XCTAssertEqual(response.uaid, "uaid:\(uaidHex)")
        XCTAssertEqual(response.totals.accounts, 2)
        XCTAssertEqual(response.dataspaces.first?.accounts.first?.assets.first?.assetId,
                       "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        XCTAssertEqual(response.dataspaces.first?.accounts.first?.assets.first?.assetDefinitionId,
                       "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        XCTAssertEqual(response.dataspaces.first?.accounts.first?.assets.first?.quantity, "500")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetUaidPortfolioIncludesAssetIdQuery() async throws {
        let uaidHex = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543211"
        let assetId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
        let payload = """
        {
          "uaid":"uaid:\(uaidHex)",
          "totals":{"accounts":1,"positions":1},
          "dataspaces":[]
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/accounts/uaid%3A\(uaidHex)/portfolio"))
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let queryItems = components?.queryItems ?? []
            let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["asset_id"], assetId)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        _ = try await makeClient().getUaidPortfolio(uaid: "uaid:\(uaidHex)",
                                                    query: ToriiUaidPortfolioQuery(assetId: assetId))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRegisterAccountCanonicalizesUaidAndIdentityCommitment() async throws {
        let uaidHex = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
        let commitmentHex = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        let responseBody = """
        {
          "account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
          "uaid":"uaid:\(uaidHex)",
          "tx_hash_hex":"\(String(repeating: "22", count: 31))23",
          "status":"accepted"
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/accounts/onboard")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            let payload = self.bodyJSON(from: request)
            XCTAssertEqual(payload["alias"] as? String, "alice@universal")
            XCTAssertEqual(payload["account_id"] as? String, "sora-account")
            XCTAssertEqual(payload["uaid"] as? String, "uaid:\(uaidHex)")
            XCTAssertEqual(payload["identity_commitment_hex"] as? String, commitmentHex)
            XCTAssertNil(payload["identity"])
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 202,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, responseBody)
        }

        let response = try await makeClient().registerAccount(
            ToriiAccountOnboardingRequest(
                alias: "alice@universal",
                accountId: "sora-account",
                uaid: "  UAID:\(uaidHex.uppercased())  ",
                identityCommitmentHex: "  \(commitmentHex.uppercased())  "
            )
        )
        XCTAssertEqual(response.uaid, "uaid:\(uaidHex)")
        XCTAssertEqual(response.status, "accepted")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRegisterAccountRejectsInvalidUaidBeforeNetwork() async {
        StubURLProtocol.handler = { _ in
            XCTFail("registerAccount should validate UAID before dispatch")
            throw URLError(.badURL)
        }
        let invalidUaid = "uaid:\(String(repeating: "10", count: 32))"

        do {
            _ = try await makeClient().registerAccount(
                ToriiAccountOnboardingRequest(
                    alias: "alice@universal",
                    accountId: "sora-account",
                    uaid: invalidUaid
                )
            )
            XCTFail("Expected invalid UAID error")
        } catch {
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRegisterAccountRejectsInvalidIdentityCommitmentBeforeNetwork() async {
        StubURLProtocol.handler = { _ in
            XCTFail("registerAccount should validate identity commitment before dispatch")
            throw URLError(.badURL)
        }
        let uaidHex = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"

        do {
            _ = try await makeClient().registerAccount(
                ToriiAccountOnboardingRequest(
                    alias: "alice@universal",
                    accountId: "sora-account",
                    uaid: "uaid:\(uaidHex)",
                    identityCommitmentHex: "abcd"
                )
            )
            XCTFail("Expected invalid identity commitment error")
        } catch {
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetUaidBindingsReturnsDataspaces() async throws {
        let uaidHex = "abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd"
        let payload = """
        {
          "uaid":"uaid:\(uaidHex)",
          "dataspaces":[
            {"dataspace_id":0,"dataspace_alias":"universal","accounts":["sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"]},
            {"dataspace_id":11,"dataspace_alias":"cbdc","accounts":[]}
          ]
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/space-directory/uaids/uaid%3A\(uaidHex)"))
            XCTAssertNil(request.url?.query)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let response = try await makeClient().getUaidBindings(
            uaid: uaidHex,
            query: ToriiUaidBindingsQuery()
        )
        XCTAssertEqual(response.dataspaces.count, 2)
        XCTAssertEqual(response.dataspaces.first?.accounts.first, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
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
              "accounts":["sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"],
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
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/space-directory/uaids/uaid%3A\(uaidHex)/manifests"))
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let items = Dictionary(uniqueKeysWithValues: (components?.queryItems ?? []).map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(items["dataspace"], "11")
            XCTAssertEqual(items["status"], "inactive")
            XCTAssertEqual(items["limit"], "2")
            XCTAssertEqual(items["offset"], "1")
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
            offset: 1
        )
        let response = try await makeClient().getUaidManifests(uaid: "uaid:\(uaidHex)", query: query)
        XCTAssertEqual(response.total, 1)
        XCTAssertEqual(response.manifests.first?.status, .active)
        XCTAssertEqual(response.manifests.first?.accounts.first, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
    }

    func testUaidBindingsQueryHasNoItems() throws {
        XCTAssertNil(try ToriiUaidBindingsQuery().queryItems())
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
            {"hash":"deadbeef","status":{"kind":"Rejected","block_height":12,"rejection_reason":{"Validation":"missing permission"}},"summary":"Rejected: missing permission","diagnostics":[{"category":"validation","code":"validation","message":"missing permission","decoded_reason":"missing permission","raw_reason":"Validation(missing permission)"}],"scope":"local","resolved_from":"state"}
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
        {"format":"pipeline.recovery.v1","height":42,"dag":{"fingerprint":"abcdef","key_count":1},"txs":[{"hash":"0x01","reads":["account/sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"],"writes":["asset/62Fk4FPcMuLvW5QjDGNF2a4jAmjM"]}]}
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
    func testGetPipelinePreflightAsync() async throws {
        let payload = """
        {"schema_version":1,"chain_height":42,"sumeragi":{"block_time_ms":1000,"commit_time_ms":2000,"stall_threshold_ms":6000},"admission":{"max_signatures":32,"max_instructions":4096,"max_tx_bytes":1048576,"max_decompressed_bytes":1048576,"max_metadata_depth":16},"block":{"max_transactions":512},"pipeline":{"signature_batch_max":0,"signature_batch_max_ed25519":64,"signature_batch_max_secp256k1":16,"signature_batch_max_pqc":8,"signature_batch_max_bls":16,"overlay_max_instructions":0,"ivm_max_decoded_instructions":1048576},"queue":{"size":2,"queued":1,"inflight":1},"fees":{"fee_asset_id":"xor#sora","fee_sink_account_id":"fees@system","base_fee":"0","per_byte_fee":"0","per_instruction_fee":"0","per_gas_unit_fee":"0","sponsorship_enabled":false,"sponsor_max_fee":"0","sponsor_verified_balance_safety_floor":"0","canonical_sponsor_account_id":null,"fee_receipts_activation_height":7,"external_settlement_enabled":false,"burn_from_unix_timestamp_ms":0,"settlement_mode":"direct","successful_claim_fee_exempt_authorities":["authority@system"]}}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/preflight")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let preflight = try await makeClient().getPipelinePreflight()
        let status = try ToriiStatusPayload(raw: [
            "peers": .number(1),
            "queue_size": .number(2),
            "time_since_last_non_empty_block_ms": .number(6001),
            "commit_time_ms": .number(30),
            "txs_approved": .number(0),
            "txs_rejected": .number(0),
            "view_changes": .number(0)
        ])
        XCTAssertEqual(preflight.schemaVersion, 1)
        XCTAssertEqual(preflight.chainHeight, 42)
        XCTAssertEqual(preflight.sumeragi.stallThresholdMs, 6000)
        XCTAssertEqual(preflight.admission.maxTxBytes, 1048576)
        XCTAssertEqual(preflight.pipeline.signatureBatchMaxEd25519, 64)
        XCTAssertEqual(preflight.queue.queued, 1)
        XCTAssertEqual(preflight.fees.baseFee, .string("0"))
        XCTAssertEqual(preflight.fees.successfulClaimFeeExemptAuthorities, ["authority@system"])
        XCTAssertTrue(preflight.isStatusStalled(status))
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
        {"abi_version":1}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/node/capabilities")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let capabilities = try await makeClient().getNodeCapabilities()
        XCTAssertEqual(capabilities.abiVersion, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetOfflineReadinessParsesRecursiveVerifierMetadata() async throws {
        let payload = """
        {
          "offline_note": true,
          "offline_one_use_keys": true,
          "offline_recursive_note_proof": true,
          "offline_recursive_note_proof_backend": "halo2/ipa",
          "offline_recursive_note_proof_circuit_id": "offline-note-recursive",
          "offline_recursive_note_proof_public_inputs_schema_hash": "\(String(repeating: "a", count: 64))",
          "offline_recursive_note_proof_public_instance_columns": 16,
          "offline_recursive_note_proof_verifier_key_id": {
            "backend": "halo2/ipa",
            "name": "offline-note-recursive"
          },
          "offline_fountain_qr": true,
          "offline_sync_optional": true,
          "offline_telemetry": true
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/offline/readiness")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let readiness = try await makeClient().getOfflineReadiness()
        XCTAssertTrue(readiness.offlineNote)
        XCTAssertTrue(readiness.offlineRecursiveNoteProof)
        XCTAssertTrue(readiness.offlineFountainQr)
        XCTAssertTrue(readiness.hasCanonicalRecursiveVerifierMetadata)
        XCTAssertEqual(readiness.offlineRecursiveNoteProofVerifierKeyId?.backend, "halo2/ipa")
        XCTAssertEqual(readiness.offlineRecursiveNoteProofVerifierKeyId?.name, OfflineNoteConstants.recursiveVerifierName)
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
          "transport": {
            "norito_rpc": {
              "enabled": true,
              "stage": "ga",
              "require_mtls": false,
              "canary_allowlist_size": 2
            },
            "streaming": {
              "soranet": {
                "enabled": true,
                "stream_tag": "norito",
                "exit_multiaddr": "/dns/torii/udp/9443/quic",
                "padding_budget_ms": 25,
                "access_kind": "authenticated",
                "gar_category": "soranet-auth",
                "channel_salt": "salt-123",
                "provision_spool_dir": "./storage/streaming/soranet_routes",
                "provision_window_segments": 4,
                "provision_queue_capacity": 256
              }
            }
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
        let transport = try XCTUnwrap(snapshot.transport)
        let noritoRpc = try XCTUnwrap(transport.noritoRpc)
        XCTAssertTrue(noritoRpc.enabled)
        XCTAssertEqual(noritoRpc.stage, "ga")
        XCTAssertFalse(noritoRpc.requireMtls)
        XCTAssertEqual(noritoRpc.canaryAllowlistSize, 2)
        let soranet = try XCTUnwrap(transport.streaming?.soranet)
        XCTAssertTrue(soranet.enabled)
        XCTAssertEqual(soranet.streamTag, "norito")
        XCTAssertEqual(soranet.exitMultiaddr, "/dns/torii/udp/9443/quic")
        XCTAssertEqual(soranet.paddingBudgetMs, 25)
        XCTAssertEqual(soranet.accessKind, "authenticated")
        XCTAssertEqual(soranet.garCategory, "soranet-auth")
        XCTAssertEqual(soranet.channelSalt, "salt-123")
        XCTAssertEqual(soranet.provisionSpoolDir, "./storage/streaming/soranet_routes")
        XCTAssertEqual(soranet.provisionWindowSegments, 4)
        XCTAssertEqual(soranet.provisionQueueCapacity, 256)
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
          "asset":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
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
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/confidential/assets/62Fk4FPcMuLvW5QjDGNF2a4jAmjM/transitions"))
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let policy = try await makeClient().getConfidentialAssetPolicy(assetDefinitionId: "  62Fk4FPcMuLvW5QjDGNF2a4jAmjM  ")
        XCTAssertEqual(policy.assetId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
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
          "asset":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
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
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/confidential/assets/62Fk4FPcMuLvW5QjDGNF2a4jAmjM/transitions"))
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        makeClient().getConfidentialAssetPolicy(assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM") { result in
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
        {"abi_version":1,"upgrade_events_total":{"proposed":5,"activated":6,"canceled":1}}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/runtime/metrics")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let metrics = try await makeClient().getRuntimeMetrics()
        XCTAssertEqual(metrics.abiVersion, 1)
        XCTAssertEqual(metrics.upgradeEventsTotal.proposed, 5)
        XCTAssertEqual(metrics.upgradeEventsTotal.activated, 6)
        XCTAssertEqual(metrics.upgradeEventsTotal.canceled, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetRuntimeAbiActiveAsync() async throws {
        let payload = """
        {"abi_version":1}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/runtime/abi/active")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let snapshot = try await makeClient().getRuntimeAbiActive()
        XCTAssertEqual(snapshot.abiVersion, 1)
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
                  "abi_version": 1,
                  "abi_hash": "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789",
                  "added_syscalls": [],
                  "added_pointer_types": [],
                  "start_height": 100,
                  "end_height": 200
                },
                "status": { "ActivatedAt": 123 },
                "proposer": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
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
        XCTAssertEqual(item.record.proposer, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(item.record.createdHeight, 90)
        guard case let .activatedAt(height) = item.record.status else {
            return XCTFail("Expected ActivatedAt status")
        }
        XCTAssertEqual(height, 123)
        let manifest = item.record.manifest
        XCTAssertEqual(manifest.name, "Upgrade Foo")
        XCTAssertEqual(manifest.description, "Test upgrade")
        XCTAssertEqual(manifest.abiVersion, 1)
        XCTAssertEqual(manifest.addedSyscalls, [])
        XCTAssertEqual(manifest.addedPointerTypes, [])
        XCTAssertEqual(manifest.startHeight, 100)
        XCTAssertEqual(manifest.endHeight, 200)
        XCTAssertEqual(manifest.abiHashHex, "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testProposeRuntimeUpgradeAsync() async throws {
        let manifest = ToriiRuntimeUpgradeManifest(
            name: "Upgrade Foo",
            description: "Test",
            abiVersion: 1,
            abiHashHex: "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789",
            addedSyscalls: [],
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
            XCTAssertEqual(manifestJSON["abi_version"] as? Int, 1)
            XCTAssertEqual(manifestJSON["abi_hash"] as? String, "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
            XCTAssertEqual(manifestJSON["start_height"] as? Int, 123)
            XCTAssertEqual(manifestJSON["end_height"] as? Int, 456)
            if let syscalls = manifestJSON["added_syscalls"] as? [NSNumber] {
                XCTAssertTrue(syscalls.isEmpty)
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
    func testListRuntimeUpgradesRejectsNonV1ManifestAsync() async throws {
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
                  "added_syscalls": [],
                  "added_pointer_types": [],
                  "start_height": 100,
                  "end_height": 200
                },
                "status": { "Proposed": null },
                "proposer": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
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

        do {
            _ = try await makeClient().listRuntimeUpgrades()
            XCTFail("expected listRuntimeUpgrades to reject non-v1 ABI manifests")
        } catch {
            XCTAssertTrue(String(describing: error).contains("abi_version must be 1"))
        }
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
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/zk/vk/halo2%2Fipa/vk%20main"))
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
    func testRegisterVerifyingKeyRejectsRemovedServerSideSigningFlow() async {
        let requestBody = ToriiVerifyingKeyRegisterRequest(
            authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            gasScheduleId: "halo2_default",
            verifyingKeyBytes: Data([0x01, 0x02, 0x03])
        )
        await XCTAssertThrowsErrorAsync(try await makeClient().registerVerifyingKey(requestBody)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(reason.contains("/v1/zk/vk/register"))
            XCTAssertTrue(reason.contains("locally signed transaction"))
        }
    }

    func testRegisterVerifyingKeyRejectsInvalidSchemaHash() {
        let requestBody = ToriiVerifyingKeyRegisterRequest(
            authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
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
            authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
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
    func testUpdateVerifyingKeyRejectsRemovedServerSideSigningFlow() async {
        var requestBody = ToriiVerifyingKeyUpdateRequest(
            authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 2,
            circuitId: "halo2/ipa::transfer_v2",
            publicInputsSchemaHashHex: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        )
        requestBody.verifyingKeyBytes = Data([0xAA])
        requestBody.commitmentHex = "20574662a58708e02e0000000000000000000000000000000000000000000000"
        await XCTAssertThrowsErrorAsync(try await makeClient().updateVerifyingKey(requestBody)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(reason.contains("/v1/zk/vk/update"))
            XCTAssertTrue(reason.contains("locally signed transaction"))
        }
    }

    func testUpdateVerifyingKeyRejectsInvalidCommitmentHex() {
        var requestBody = ToriiVerifyingKeyUpdateRequest(
            authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
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

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamExplorerTransactionsAsync() async throws {
        let ssePayload = """
id: 1
data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","hash":"hash1","block":100,"created_at":"2025-01-01T00:00:00Z","executable":"Instructions","status":"Committed"}

data: {"authority":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D","hash":"hash2","block":101,"created_at":"2025-01-02T00:00:00Z","executable":"Instructions","status":"Rejected"}

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

        let stream = makeClient().streamExplorerTransactions()
        var iterator = stream.makeAsyncIterator()

        let first = try await iterator.next()
        XCTAssertEqual(first?.authority, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(first?.hash, "hash1")
        XCTAssertEqual(first?.block, 100)
        XCTAssertEqual(first?.status, "Committed")

        let second = try await iterator.next()
        XCTAssertEqual(second?.authority, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
        XCTAssertEqual(second?.hash, "hash2")
        XCTAssertEqual(second?.block, 101)
        XCTAssertEqual(second?.status, "Rejected")

        let third = try await iterator.next()
        XCTAssertNil(third)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamExplorerInstructionsAsync() async throws {
        let ssePayload = """
data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

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

        let stream = makeClient().streamExplorerInstructions()
        var iterator = stream.makeAsyncIterator()

        let first = try await iterator.next()
        XCTAssertEqual(first?.authority, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(first?.kind, "Transfer")
        XCTAssertEqual(first?.transactionHash, "hash1")
        XCTAssertEqual(first?.transactionStatus, "Committed")
        XCTAssertEqual(first?.block, 10)
        XCTAssertEqual(first?.index, 0)
        XCTAssertEqual(first?.box.scale, "0x00")

        let second = try await iterator.next()
        XCTAssertNil(second)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamExplorerTransfersAsync() async throws {
        let ssePayload = """
data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"6","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":2}

data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Mint","r#box":{"scale":"0x01","json":{"kind":"Mint","payload":{"variant":"Asset","value":{"destination":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"1"}}}},"transaction_hash":"hash2","transaction_status":"Committed","block":10,"index":1}

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

        let stream = makeClient().streamExplorerTransfers(matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                          assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        var iterator = stream.makeAsyncIterator()

        let first = try await iterator.next()
        XCTAssertEqual(first?.instruction.transactionHash, "hash1")
        switch first?.details {
        case .asset(let asset):
            XCTAssertEqual(asset.senderAccountId, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
            XCTAssertEqual(asset.destinationAccountId, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
            XCTAssertEqual(asset.assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            XCTAssertEqual(asset.amount, "5")
        default:
            XCTFail("Expected asset transfer details")
        }

        let second = try await iterator.next()
        XCTAssertNil(second)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamExplorerTransferSummariesAsync() async throws {
        let ssePayload = """
data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"6","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":1}

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

        let stream = makeClient().streamExplorerTransferSummaries(matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                                  assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        var iterator = stream.makeAsyncIterator()

        let summary = try await iterator.next()
        XCTAssertEqual(summary?.transactionHash, "hash1")
        XCTAssertEqual(summary?.senderAccountId, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(summary?.receiverAccountId, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
        XCTAssertEqual(summary?.assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        XCTAssertEqual(summary?.amount, "5")
        XCTAssertEqual(summary?.direction, .incoming)

        let second = try await iterator.next()
        XCTAssertNil(second)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamAccountTransferHistoryCombinesHistoryAndStream() async throws {
        let historyPayload = """
        {
            "pagination": {
                "page": 1,
                "per_page": 2,
                "total_pages": 1,
                "total_items": 2
            },
            "items": [
                {
                    "authority": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object": "5",
                                    "destination": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            }
                        }
                    },
                    "transaction_hash": "hash1",
                    "transaction_status": "Committed",
                    "block": 10,
                    "index": 0
                },
                {
                    "authority": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object": "6",
                                    "destination": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            }
                        }
                    },
                    "transaction_hash": "hash3",
                    "transaction_status": "Committed",
                    "block": 10,
                    "index": 1
                }
            ]
        }
        """
            .data(using: .utf8)!

        let ssePayload = """
data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"7","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash2","transaction_status":"Committed","block":11,"index":0}

data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"8","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash4","transaction_status":"Committed","block":11,"index":1}

"""
            .data(using: .utf8)!

        var lastEventIdHeader: String?
        StubURLProtocol.handler = { request in
            guard let url = request.url else {
                throw ToriiClientError.invalidResponse
            }
            if url.path == "/v1/explorer/instructions" {
                let components = URLComponents(url: url, resolvingAgainstBaseURL: false)
                let queryItems = components?.queryItems ?? []
                let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
                XCTAssertEqual(query["page"], "1")
                XCTAssertEqual(query["per_page"], "1")
                XCTAssertEqual(query["kind"], "Transfer")
                XCTAssertEqual(query["asset_id"], "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
                let response = HTTPURLResponse(url: url,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, historyPayload)
            }
            if url.path == "/v1/explorer/instructions/stream" {
                lastEventIdHeader = request.value(forHTTPHeaderField: "Last-Event-ID")
                let response = HTTPURLResponse(url: url,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "text/event-stream"])!
                return (response, ssePayload)
            }
            throw ToriiClientError.invalidResponse
        }

        let stream = makeClient().streamAccountTransferHistory(accountId: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                               perPage: 1,
                                                               assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                                               lastEventId: "5")
        var iterator = stream.makeAsyncIterator()

        let first = try await iterator.next()
        XCTAssertEqual(first?.transactionHash, "hash1")

        let second = try await iterator.next()
        XCTAssertEqual(second?.transactionHash, "hash2")

        let third = try await iterator.next()
        XCTAssertNil(third)
        XCTAssertEqual(lastEventIdHeader, "5")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamAccountTransferHistoryPreservesBatchDuplicates() async throws {
        let historyPayload = """
        {
            "pagination": {
                "page": 1,
                "per_page": 1,
                "total_pages": 1,
                "total_items": 1
            },
            "items": [
                {
                    "authority": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "AssetBatch",
                                "value": {
                                    "entries": [
                                        {
                                            "from": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                            "to": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                            "asset_definition": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                            "amount": "5"
                                        },
                                        {
                                            "from": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                            "to": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                            "asset_definition": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                            "amount": "5"
                                        }
                                    ]
                                }
                            }
                        }
                    },
                    "transaction_hash": "hash1",
                    "transaction_status": "Committed",
                    "block": 10,
                    "index": 0
                }
            ]
        }
        """
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            guard let url = request.url else {
                throw ToriiClientError.invalidResponse
            }
            if url.path == "/v1/explorer/instructions" {
                let response = HTTPURLResponse(url: url,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, historyPayload)
            }
            throw ToriiClientError.invalidResponse
        }

        let stream = makeClient().streamAccountTransferHistory(accountId: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                               perPage: 1,
                                                               maxItems: 2)
        var iterator = stream.makeAsyncIterator()

        let first = try await iterator.next()
        let second = try await iterator.next()
        let third = try await iterator.next()

        XCTAssertEqual(first?.transactionHash, "hash1")
        XCTAssertEqual(first?.transferIndex, 0)
        XCTAssertEqual(second?.transactionHash, "hash1")
        XCTAssertEqual(second?.transferIndex, 1)
        XCTAssertNil(third)
    }

#if canImport(Combine)
    @available(iOS 15.0, macOS 12.0, *)
    func testAssetsPublisherDeliversBalances() throws {
        let payload = """
[
  {"asset":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","quantity":"10"},
  {"asset":"5CJ6HCMxWw9xhuHmxDrzEfWGeE7M","quantity":"20"}
]
"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB/assets")
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
        client.assetsPublisher(accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB", limit: 2, scheduler: nil)
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
        XCTAssertEqual(balances.first?.asset, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        XCTAssertEqual(balances.first?.quantity, "10")
        XCTAssertEqual(balances.last?.asset, "5CJ6HCMxWw9xhuHmxDrzEfWGeE7M")
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

    @available(iOS 15.0, macOS 12.0, *)
    func testExplorerTransactionsPublisherDeliversItems() throws {
        let ssePayload = """
data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","hash":"hash1","block":100,"created_at":"2025-01-01T00:00:00Z","executable":"Instructions","status":"Committed"}

data: {"authority":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D","hash":"hash2","block":101,"created_at":"2025-01-02T00:00:00Z","executable":"Instructions","status":"Rejected"}

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
        let valueExpectation = expectation(description: "received transaction events")
        valueExpectation.expectedFulfillmentCount = 2
        let completionExpectation = expectation(description: "publisher completed")

        var hashes: [String] = []
        client.explorerTransactionsPublisher(scheduler: nil)
            .sink { completion in
                if case .failure(let error) = completion {
                    XCTFail("Unexpected failure: \(error)")
                }
                completionExpectation.fulfill()
            } receiveValue: { item in
                hashes.append(item.hash)
                valueExpectation.fulfill()
            }
            .store(in: &cancellables)

        waitForExpectations(timeout: 2.0)
        XCTAssertEqual(hashes, ["hash1", "hash2"])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testExplorerInstructionsPublisherDeliversItems() throws {
        let ssePayload = """
data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

"""
            .data(using: .utf8)!

        var lastEventIdHeader: String?
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/event-stream")
            lastEventIdHeader = request.value(forHTTPHeaderField: "Last-Event-ID")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "text/event-stream"])!
            return (response, ssePayload)
        }

        let client = makeClient()
        var cancellables: Set<AnyCancellable> = []
        let valueExpectation = expectation(description: "received instruction event")
        let completionExpectation = expectation(description: "publisher completed")

        var received: [ToriiExplorerInstructionItem] = []
        client.explorerInstructionsPublisher(lastEventId: "42", scheduler: nil)
            .sink { completion in
                if case .failure(let error) = completion {
                    XCTFail("Unexpected failure: \(error)")
                }
                completionExpectation.fulfill()
            } receiveValue: { item in
                received.append(item)
                valueExpectation.fulfill()
            }
            .store(in: &cancellables)

        waitForExpectations(timeout: 2.0)
        XCTAssertEqual(received.first?.transactionHash, "hash1")
        XCTAssertEqual(lastEventIdHeader, "42")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testExplorerTransfersPublisherDeliversRecords() throws {
        let ssePayload = """
data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"6","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":1}

data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Mint","r#box":{"scale":"0x01","json":{"kind":"Mint","payload":{"variant":"Asset","value":{"destination":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"1"}}}},"transaction_hash":"hash2","transaction_status":"Committed","block":10,"index":1}

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
        let valueExpectation = expectation(description: "received transfer record")
        let completionExpectation = expectation(description: "publisher completed")

        var records: [ToriiExplorerTransferRecord] = []
        client.explorerTransfersPublisher(matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                           assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                           scheduler: nil)
            .sink { completion in
                if case .failure(let error) = completion {
                    XCTFail("Unexpected failure: \(error)")
                }
                completionExpectation.fulfill()
            } receiveValue: { record in
                records.append(record)
                valueExpectation.fulfill()
            }
            .store(in: &cancellables)

        waitForExpectations(timeout: 2.0)
        XCTAssertEqual(records.count, 1)
        XCTAssertEqual(records.first?.instruction.transactionHash, "hash1")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testExplorerTransferSummariesPublisherDeliversItems() throws {
        let ssePayload = """
data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"6","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":1}

"""
            .data(using: .utf8)!

        var lastEventIdHeader: String?
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "text/event-stream")
            lastEventIdHeader = request.value(forHTTPHeaderField: "Last-Event-ID")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "text/event-stream"])!
            return (response, ssePayload)
        }

        let client = makeClient()
        var cancellables: Set<AnyCancellable> = []
        let valueExpectation = expectation(description: "received transfer summary")
        let completionExpectation = expectation(description: "publisher completed")

        var summaries: [ToriiExplorerTransferSummary] = []
        client.explorerTransferSummariesPublisher(lastEventId: "7",
                                                   matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                   assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                                   scheduler: nil)
            .sink { completion in
                if case .failure(let error) = completion {
                    XCTFail("Unexpected failure: \(error)")
                }
                completionExpectation.fulfill()
            } receiveValue: { summary in
                summaries.append(summary)
                valueExpectation.fulfill()
            }
            .store(in: &cancellables)

        waitForExpectations(timeout: 2.0)
        XCTAssertEqual(summaries.count, 1)
        XCTAssertEqual(summaries.first?.transactionHash, "hash1")
        XCTAssertEqual(lastEventIdHeader, "7")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testAccountTransferHistoryPublisherCombinesHistoryAndStream() throws {
        let historyPayload = """
        {
            "pagination": {
                "page": 1,
                "per_page": 2,
                "total_pages": 1,
                "total_items": 2
            },
            "items": [
                {
                    "authority": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object": "5",
                                    "destination": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            }
                        }
                    },
                    "transaction_hash": "hash1",
                    "transaction_status": "Committed",
                    "block": 10,
                    "index": 0
                },
                {
                    "authority": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object": "6",
                                    "destination": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            }
                        }
                    },
                    "transaction_hash": "hash3",
                    "transaction_status": "Committed",
                    "block": 10,
                    "index": 1
                }
            ]
        }
        """
            .data(using: .utf8)!

        let ssePayload = """
data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"7","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash2","transaction_status":"Committed","block":11,"index":0}

data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"8","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash4","transaction_status":"Committed","block":11,"index":1}

"""
            .data(using: .utf8)!

        var lastEventIdHeader: String?
        StubURLProtocol.handler = { request in
            guard let url = request.url else {
                throw ToriiClientError.invalidResponse
            }
            if url.path == "/v1/explorer/instructions" {
                let components = URLComponents(url: url, resolvingAgainstBaseURL: false)
                let queryItems = components?.queryItems ?? []
                let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
                XCTAssertEqual(query["asset_id"], "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
                let response = HTTPURLResponse(url: url,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, historyPayload)
            }
            if url.path == "/v1/explorer/instructions/stream" {
                lastEventIdHeader = request.value(forHTTPHeaderField: "Last-Event-ID")
                let response = HTTPURLResponse(url: url,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "text/event-stream"])!
                return (response, ssePayload)
            }
            throw ToriiClientError.invalidResponse
        }

        let client = makeClient()
        var cancellables: Set<AnyCancellable> = []
        let valueExpectation = expectation(description: "received history + stream items")
        valueExpectation.expectedFulfillmentCount = 2
        let completionExpectation = expectation(description: "publisher completed")

        var hashes: [String] = []
        client.accountTransferHistoryPublisher(accountId: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                               perPage: 1,
                                               assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                               lastEventId: "9",
                                               scheduler: nil)
            .sink { completion in
                if case .failure(let error) = completion {
                    XCTFail("Unexpected failure: \(error)")
                }
                completionExpectation.fulfill()
            } receiveValue: { summary in
                hashes.append(summary.transactionHash)
                valueExpectation.fulfill()
            }
            .store(in: &cancellables)

        waitForExpectations(timeout: 2.0)
        XCTAssertEqual(hashes, ["hash1", "hash2"])
        XCTAssertEqual(lastEventIdHeader, "9")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamTransactionTransferSummariesCombinesHistoryAndStream() async throws {
        let historyPayload = """
        {
            "pagination": {
                "page": 1,
                "per_page": 1,
                "total_pages": 1,
                "total_items": 1
            },
            "items": [
                {
                    "authority": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object": "5",
                                    "destination": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            }
                        }
                    },
                    "transaction_hash": "deadbeef",
                    "transaction_status": "Committed",
                    "block": 10,
                    "index": 0
                },
                {
                    "authority": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object": "6",
                                    "destination": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            }
                        }
                    },
                    "transaction_hash": "deadbeef",
                    "transaction_status": "Committed",
                    "block": 10,
                    "index": 1
                }
            ]
        }
        """
            .data(using: .utf8)!

        let ssePayload = """
data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"7","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"deadbeef","transaction_status":"Committed","block":11,"index":0}

data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"8","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"deadbeef","transaction_status":"Committed","block":11,"index":2}

data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:02Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"9","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"otherhash","transaction_status":"Committed","block":11,"index":1}

"""
            .data(using: .utf8)!

        var lastEventIdHeader: String?
        StubURLProtocol.handler = { request in
            guard let url = request.url else {
                throw ToriiClientError.invalidResponse
            }
            if url.path == "/v1/explorer/instructions" {
                let response = HTTPURLResponse(url: url,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, historyPayload)
            }
            if url.path == "/v1/explorer/instructions/stream" {
                lastEventIdHeader = request.value(forHTTPHeaderField: "Last-Event-ID")
                let response = HTTPURLResponse(url: url,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "text/event-stream"])!
                return (response, ssePayload)
            }
            throw ToriiClientError.invalidResponse
        }

        let stream = makeClient().streamTransactionTransferSummaries(hashHex: "deadbeef",
                                                                     matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                                     assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                                                     lastEventId: "12")
        var iterator = stream.makeAsyncIterator()

        let first = try await iterator.next()
        XCTAssertEqual(first?.transactionHash, "deadbeef")
        XCTAssertEqual(first?.assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")

        let second = try await iterator.next()
        XCTAssertEqual(second?.transactionHash, "deadbeef")
        XCTAssertEqual(second?.assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")

        let third = try await iterator.next()
        XCTAssertNil(third)
        XCTAssertEqual(lastEventIdHeader, "12")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testTransactionTransferSummariesPublisherCombinesHistoryAndStream() throws {
        let historyPayload = """
        {
            "pagination": {
                "page": 1,
                "per_page": 1,
                "total_pages": 1,
                "total_items": 1
            },
            "items": [
                {
                    "authority": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object": "5",
                                    "destination": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            }
                        }
                    },
                    "transaction_hash": "deadbeef",
                    "transaction_status": "Committed",
                    "block": 10,
                    "index": 0
                },
                {
                    "authority": "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                    "object": "6",
                                    "destination": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            }
                        }
                    },
                    "transaction_hash": "deadbeef",
                    "transaction_status": "Committed",
                    "block": 10,
                    "index": 1
                }
            ]
        }
        """
            .data(using: .utf8)!

        let ssePayload = """
data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"7","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"deadbeef","transaction_status":"Committed","block":11,"index":0}

data: {"authority":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","object":"8","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"deadbeef","transaction_status":"Committed","block":11,"index":2}

"""
            .data(using: .utf8)!

        StubURLProtocol.handler = { request in
            guard let url = request.url else {
                throw ToriiClientError.invalidResponse
            }
            if url.path == "/v1/explorer/instructions" {
                let response = HTTPURLResponse(url: url,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, historyPayload)
            }
            if url.path == "/v1/explorer/instructions/stream" {
                let response = HTTPURLResponse(url: url,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "text/event-stream"])!
                return (response, ssePayload)
            }
            throw ToriiClientError.invalidResponse
        }

        let client = makeClient()
        var cancellables: Set<AnyCancellable> = []
        let valueExpectation = expectation(description: "received tx history + stream items")
        valueExpectation.expectedFulfillmentCount = 2
        let completionExpectation = expectation(description: "publisher completed")

        var hashes: [String] = []
        client.transactionTransferSummariesPublisher(hashHex: "deadbeef",
                                                     matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                     assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                                     scheduler: nil)
            .sink { completion in
                if case .failure(let error) = completion {
                    XCTFail("Unexpected failure: \(error)")
                }
                completionExpectation.fulfill()
            } receiveValue: { summary in
                hashes.append(summary.transactionHash)
                valueExpectation.fulfill()
            }
            .store(in: &cancellables)

        waitForExpectations(timeout: 2.0)
        XCTAssertEqual(hashes, ["deadbeef", "deadbeef"])
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
                    "validator_ids": ["sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB", "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"],
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

    @available(iOS 15.0, macOS 12.0, *)
    func testGetSumeragiCommitQcParsesRecordAsync() async throws {
        let blockHash = String(repeating: "a", count: 64)
        let payload = """
        {
            "subject_block_hash": "\(blockHash)",
            "commit_qc": {
                "phase": "Commit",
                "parent_state_root": "\(String(repeating: "b", count: 64))",
                "post_state_root": "\(String(repeating: "c", count: 64))",
                "height": 12,
                "view": 3,
                "epoch": 4,
                "mode_tag": "iroha2-consensus::permissioned-sumeragi@v1",
                "validator_set_hash": "\(String(repeating: "d", count: 64))",
                "validator_set_hash_version": 1,
                "validator_set": ["sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB", "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"],
                "signers_bitmap": "0a",
                "bls_aggregate_signature": "ff"
            }
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/sumeragi/commit_qc/\(blockHash)")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let record = try await makeClient().getSumeragiCommitQc(blockHashHex: "0x\(blockHash)")
        XCTAssertEqual(record.subjectBlockHash, blockHash)
        XCTAssertEqual(record.commitQc?.postStateRoot, String(repeating: "c", count: 64))
        XCTAssertEqual(record.commitQc?.validatorSet.count, 2)
    }

    func testPipelineTransactionEventDecodesNumericDataspaceId() throws {
        let payload = """
        {
            "event": "Transaction",
            "hash": "abc123",
            "status": "Applied",
            "dataspace_id": 9
        }
        """

        let event = try JSONDecoder().decode(
            ToriiPipelineTransactionEvent.self,
            from: Data(payload.utf8)
        )

        XCTAssertEqual(event.dataspaceId, "9")
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
            {"observed_at_ms":10000,"peers":2,"queue_size":4,"queue_queued":3,"queue_inflight":1,"last_block_committed_at_ms":9900,"last_non_empty_block_committed_at_ms":9000,"time_since_last_block_ms":100,"time_since_last_non_empty_block_ms":1000,"commit_time_ms":45,"txs_approved":5,"txs_rejected":1,"view_changes":0}
            """.data(using: .utf8)!,
            """
            {"observed_at_ms":11000,"peers":3,"queue_size":11,"queue_queued":8,"queue_inflight":3,"last_block_committed_at_ms":10900,"last_non_empty_block_committed_at_ms":10000,"time_since_last_block_ms":100,"time_since_last_non_empty_block_ms":1000,"commit_time_ms":120,"txs_approved":9,"txs_rejected":3,"view_changes":2}
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
        XCTAssertEqual(first.status.queueQueued, 3)
        XCTAssertEqual(first.status.queueInflight, 1)
        XCTAssertEqual(first.metrics.timeSinceLastNonEmptyBlockMs, 1000)
        XCTAssertTrue(first.status.isQueueStalled(stallThresholdMs: 999))
        XCTAssertFalse(first.status.isQueueStalled(stallThresholdMs: 1000))
        XCTAssertEqual(first.metrics.queueDelta, 0)
        XCTAssertEqual(first.metrics.txApprovedDelta, 0)
        XCTAssertFalse(first.metrics.hasActivity)

        let second = try await client.getStatusSnapshot()
        XCTAssertEqual(second.status.queueSize, 11)
        XCTAssertEqual(second.metrics.queueQueued, 8)
        XCTAssertEqual(second.metrics.queueInflight, 3)
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
            "lane_governance_sealed_aliases": .array([.string("public"), .string("payments")]),
            "dataspace_catalog": .array([
                .object([
                    "lane_id": .number(7),
                    "lane_alias": .string("lane-alpha"),
                    "dataspace_id": .number(9),
                    "alias": .string("alpha"),
                    "visibility": .string("restricted"),
                    "storage_profile": .string("balanced"),
                    "manifest_required": .bool(true),
                    "manifest_ready": .bool(false),
                    "sealed": .bool(true),
                    "manifest_path": .null,
                    "protected_namespaces": .array([.string("alpha")])
                ])
            ])
        ])

        XCTAssertEqual(payload.laneGovernanceSealedTotal, 2)
        XCTAssertEqual(payload.laneGovernanceSealedAliases, ["public", "payments"])
        XCTAssertEqual(payload.dataspaceCatalog.first?.alias, "alpha")
        XCTAssertEqual(try payload.requireDataspace(alias: "alpha").sealed, true)
        XCTAssertEqual(payload.dataspace(id: 9)?.manifestRequired, true)
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
        XCTAssertTrue(payload.dataspaceCatalog.isEmpty)
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
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/zk/prover/reports/report%2F1"))
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
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/zk/prover/reports/report%2F1"))
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
            XCTAssertEqual(json["authority"] as? String, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
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
        let requestBody = ToriiRegisterContractCodeRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
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
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/zk/attachments/abc%2Fdef"))
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
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/zk/attachments/abc%2Fdef"))
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
        let requestBody = ToriiRegisterContractCodeRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
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

    func testDeployContractRejectsRemovedServerSideSigningFlow() async {
        let req = ToriiDeployContractRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                             codeB64: "AQ==",
                                             contractAlias: "mint::universal")
        await XCTAssertThrowsErrorAsync(try await makeClient().deployContract(req)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(reason.contains("/v1/contracts/deploy"))
            XCTAssertTrue(reason.contains("locally signed transaction"))
        }
    }

    func testDeployContractRejectsInvalidBase64() {
        let request = ToriiDeployContractRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                 codeB64: "%%%",
                                                 contractAlias: "mint::universal")
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testDeployContractEncodesAliasFirstPayload() throws {
        let request = ToriiDeployContractRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                 codeB64: "AQ==",
                                                 contractAlias: "mint::universal",
                                                 leaseExpiryMs: 42)
        let body = try XCTUnwrap(try JSONSerialization.jsonObject(with: JSONEncoder().encode(request)) as? [String: Any])
        XCTAssertEqual(body["authority"] as? String, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        XCTAssertEqual(body["code_b64"] as? String, "AQ==")
        XCTAssertEqual(body["contract_alias"] as? String, "mint::universal")
        XCTAssertEqual(body["lease_expiry_ms"] as? Int, 42)
    }

    func testDeployContractRejectsInvalidAlias() {
        let request = ToriiDeployContractRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                 codeB64: "AQ==",
                                                 contractAlias: "mint@universal")
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testDeployContractParsesUpgradeResponse() throws {
        let codeHash = String(repeating: "a", count: 64)
        let abiHash = String(repeating: "b", count: 64)
        let txHash = String(repeating: "c", count: 64)
        let payload = """
        {"ok":true,"contract_alias":"mint::universal","contract_address":"tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8","previous_contract_address":"tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq9","upgraded":true,"dataspace":"universal","deploy_nonce":7,"tx_hash_hex":"\(txHash)","pipeline_status":{"hash":"\(txHash)","status":{"kind":"Queued","block_height":null,"rejection_reason":null},"summary":"Queued","diagnostics":[],"scope":"local","resolved_from":"queue"},"code_hash_hex":"\(codeHash)","abi_hash_hex":"\(abiHash)"}
        """.data(using: .utf8)!
        let response = try JSONDecoder().decode(ToriiDeployContractResponse.self, from: payload)
        XCTAssertTrue(response.ok)
        XCTAssertEqual(response.contractAlias, "mint::universal")
        XCTAssertEqual(response.contractAddress, "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8")
        XCTAssertEqual(response.previousContractAddress, "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq9")
        XCTAssertTrue(response.upgraded)
        XCTAssertEqual(response.dataspace, "universal")
        XCTAssertEqual(response.deployNonce, 7)
        XCTAssertEqual(response.txHashHex, txHash)
        XCTAssertEqual(response.pipelineStatus?.content.status.kind, "Queued")
        XCTAssertEqual(response.codeHashHex, codeHash)
        XCTAssertEqual(response.abiHashHex, abiHash)
    }

    func testDeployContractInstanceRejectsRemovedServerSideSigningFlow() async {
        let manifest = ToriiContractManifest(compilerFingerprint: "kotodama-0.8")
        let req = ToriiDeployContractInstanceRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                     namespace: "apps",
                                                     contractId: "calc.v1",
                                                     codeB64: "AQ==",
                                                     manifest: manifest)
        await XCTAssertThrowsErrorAsync(try await makeClient().deployContractInstance(req)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(reason.contains("/v1/contracts/instance"))
            XCTAssertTrue(reason.contains("locally signed transaction"))
        }
    }

    func testActivateContractInstanceRejectsRemovedServerSideSigningFlow() async {
        let codeHash = String(repeating: "1", count: 64)
        let req = ToriiActivateContractInstanceRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
                                                       namespace: "apps",
                                                       contractId: "calc.v1",
                                                       codeHash: codeHash)
        await XCTAssertThrowsErrorAsync(try await makeClient().activateContractInstance(req)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(reason.contains("/v1/contracts/instance/activate"))
            XCTAssertTrue(reason.contains("locally signed transaction"))
        }
    }

    func testCallContractParsesResponse() {
        let expectation = expectation(description: "call contract")
        let codeHash = String(repeating: "d", count: 64)
        let abiHash = String(repeating: "e", count: 64)
        let txHash = String(repeating: "f", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/call")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            guard let body = self.bodyData(from: request),
                  let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
                XCTFail("missing JSON body")
                throw NSError(domain: "stub", code: -1)
            }
            XCTAssertEqual(json["authority"] as? String, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
            XCTAssertNil(json["private_key"])
            XCTAssertEqual(json["public_key_hex"] as? String, String(repeating: "1", count: 64))
            XCTAssertEqual(json["signature_b64"] as? String, "AQ==")
            XCTAssertEqual(json["contract_alias"] as? String, "mint::universal")
            XCTAssertEqual(json["entrypoint"] as? String, "create")
            XCTAssertEqual(json["gas_limit"] as? Int, 7)
            XCTAssertEqual(json["gas_asset_id"] as? String, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            XCTAssertEqual(json["fee_sponsor"] as? String, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"submitted":true,"dataspace":"universal","contract_address":"tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8","code_hash_hex":"\(codeHash)","abi_hash_hex":"\(abiHash)","creation_time_ms":321,"tx_hash_hex":"\(txHash)","pipeline_status":{"hash":"\(txHash)","status":{"kind":"Rejected","block_height":12,"rejection_reason":{"Validation":"missing permission"}},"summary":"Rejected: missing permission","diagnostics":[{"category":"validation","code":"validation","message":"missing permission","decoded_reason":"missing permission","raw_reason":"Validation(missing permission)"}],"scope":"local","resolved_from":"state"},"transaction_scaffold_b64":"Aw==","signed_transaction_b64":"AQ==","signing_message_b64":"Ag==","entrypoint":"create"}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiContractCallRequest(
            authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            publicKeyHex: String(repeating: "1", count: 64),
            signatureB64: "AQ==",
            contractAlias: "mint::universal",
            entrypoint: "create",
            payload: .object(["amount": .string("10")]),
            creationTimeMs: 321,
            gasAssetId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
            feeSponsor: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            gasLimit: 7
        )
        makeClient().callContract(request) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
                XCTAssertTrue(response.submitted)
                XCTAssertEqual(response.dataspace, "universal")
                XCTAssertEqual(response.contractAddress, "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8")
                XCTAssertEqual(response.codeHashHex, codeHash)
                XCTAssertEqual(response.abiHashHex, abiHash)
                XCTAssertEqual(response.creationTimeMs, 321)
                XCTAssertEqual(response.txHashHex, txHash)
                XCTAssertEqual(response.pipelineStatus?.content.status.rejectionReason, #"{"Validation":"missing permission"}"#)
                XCTAssertEqual(response.pipelineStatus?.primaryDiagnostic?.decodedReason, "missing permission")
                XCTAssertEqual(response.pipelineStatus?.isRejected, true)
                XCTAssertEqual(response.transactionScaffoldB64, "Aw==")
                XCTAssertEqual(response.signedTransactionB64, "AQ==")
                XCTAssertEqual(response.signingMessageB64, "Ag==")
                XCTAssertEqual(response.entrypoint, "create")
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testCallContractRejectsZeroGasLimit() {
        let request = ToriiContractCallRequest(
            authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            contractAlias: "mint::universal",
            gasLimit: 0
        )
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testCallContractRejectsAmbiguousContractTarget() {
        let request = ToriiContractCallRequest(
            authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            contractAddress: "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8",
            contractAlias: "mint::universal",
            gasLimit: 7
        )
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testProposeMultisigEncodesStructuredJsonInstructions() throws {
        let expectation = expectation(description: "propose multisig")
        let proposalId = String(repeating: "a", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/multisig/propose")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            guard let body = self.bodyData(from: request),
                  let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
                XCTFail("missing JSON body")
                throw NSError(domain: "stub", code: -1)
            }
            XCTAssertEqual(json["multisig_account_alias"] as? String, "cbdc@banka")
            XCTAssertEqual(json["signer_account_id"] as? String, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
            XCTAssertEqual(json["creation_time_ms"] as? Int, 123)
            XCTAssertEqual(json["fee_sponsor"] as? String, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
            let instructions = json["instructions"] as? [[String: Any]]
            XCTAssertEqual(instructions?.first?["kind"] as? String, "Transfer")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"resolved_multisig_account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","submitted":false,"proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","creation_time_ms":123,"signing_message_b64":"AQ=="}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = try ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            signerAccountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            creationTimeMs: 123,
            feeSponsor: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            instructions: [
                try ToriiMultisigProposeInstruction(object: ["kind": .string("Transfer")])
            ]
        )
        makeClient().proposeMultisig(request) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
                XCTAssertEqual(response.resolvedMultisigAccountId, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
                XCTAssertEqual(response.proposalId, proposalId)
                XCTAssertEqual(response.instructionsHash, proposalId)
                XCTAssertEqual(response.creationTimeMs, 123)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testProposeMultisigSendsWholeNoritoDtoBody() {
        let expectation = expectation(description: "propose multisig native body")
        let proposalId = String(repeating: "b", count: 64)
        let noritoBody = Data([0x4e, 0x52, 0x54, 0x30, 0x01, 0x02, 0x03])
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/multisig/propose")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-norito")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            XCTAssertEqual(self.bodyData(from: request), noritoBody)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"resolved_multisig_account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","submitted":false,"proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)"}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        makeClient().proposeMultisig(noritoBody: noritoBody) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
                XCTAssertEqual(response.proposalId, proposalId)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testProposeMultisigInstructionRejectsPerInstructionNoritoBlobs() {
        XCTAssertThrowsError(try ToriiMultisigProposeInstruction(base64: "AQID")) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
        XCTAssertThrowsError(try ToriiMultisigProposeInstruction(noritoInstructionBoxBytes: Data([0x4e, 0x52, 0x54, 0x30]))) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
        XCTAssertThrowsError(try ToriiMultisigProposeInstruction(json: .string("AQID"))) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testProposeMultisigRejectsEmptyInstructionBytesAndBadRequestShape() {
        XCTAssertThrowsError(try ToriiMultisigProposeInstruction(noritoInstructionBoxBytes: Data())) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }

        let signer = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"
        let instruction = try! ToriiMultisigProposeInstruction(object: ["kind": .string("Transfer")])
        let ambiguousSelectorRequest = ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(
                multisigAccountId: signer,
                multisigAccountAlias: "cbdc@banka"
            ),
            signerAccountId: signer,
            instructions: [instruction]
        )
        XCTAssertThrowsError(try JSONEncoder().encode(ambiguousSelectorRequest)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }

        let emptyBatchRequest = ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            signerAccountId: signer,
            instructions: []
        )
        XCTAssertThrowsError(try JSONEncoder().encode(emptyBatchRequest)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testProposeMultisigRejectsMalformedResponseFields() {
        let expectation = expectation(description: "propose multisig malformed response")
        let instruction = try! ToriiMultisigProposeInstruction(object: ["kind": .string("Transfer")])
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/multisig/propose")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"resolved_multisig_account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","instructions_hash":"aa","signing_message_b64":"not base64"}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            signerAccountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            instructions: [instruction]
        )
        makeClient().proposeMultisig(request) { result in
            switch result {
            case .success:
                XCTFail("Malformed multisig response should be rejected")
            case .failure(let error):
                guard case ToriiClientError.decoding = error else {
                    return XCTFail("Expected decoding error, got \(error)")
                }
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testProposeMultisigRejectsFalseOkResponse() {
        let expectation = expectation(description: "propose multisig false ok response")
        let instruction = try! ToriiMultisigProposeInstruction(object: ["kind": .string("Transfer")])
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/multisig/propose")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":false,"resolved_multisig_account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            signerAccountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            instructions: [instruction]
        )
        makeClient().proposeMultisig(request) { result in
            switch result {
            case .success:
                XCTFail("False ok response should be rejected")
            case .failure(let error):
                guard case ToriiClientError.decoding = error else {
                    return XCTFail("Expected decoding error, got \(error)")
                }
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testProposeMultisigRejectsEmptySigningMessageResponse() {
        let expectation = expectation(description: "propose multisig empty signing message")
        let instruction = try! ToriiMultisigProposeInstruction(object: ["kind": .string("Transfer")])
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/multisig/propose")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"resolved_multisig_account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","signing_message_b64":""}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            signerAccountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            instructions: [instruction]
        )
        makeClient().proposeMultisig(request) { result in
            switch result {
            case .success:
                XCTFail("Empty signing message should be rejected")
            case .failure(let error):
                guard case ToriiClientError.decoding = error else {
                    return XCTFail("Expected decoding error, got \(error)")
                }
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testProposeMultisigRejectsNegativeResponseCreationTime() {
        let expectation = expectation(description: "propose multisig negative creation time")
        let instruction = try! ToriiMultisigProposeInstruction(object: ["kind": .string("Transfer")])
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/multisig/propose")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"resolved_multisig_account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","creation_time_ms":-1}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            signerAccountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            instructions: [instruction]
        )
        makeClient().proposeMultisig(request) { result in
            switch result {
            case .success:
                XCTFail("Negative response creation time should be rejected")
            case .failure(let error):
                guard case ToriiClientError.decoding = error else {
                    return XCTFail("Expected decoding error, got \(error)")
                }
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testProposeMultisigContractCallEncodesAliasSelector() {
        let expectation = expectation(description: "propose multisig contract call")
        let proposalId = String(repeating: "a", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/call/multisig/propose")
            XCTAssertEqual(request.httpMethod, "POST")
            guard let body = self.bodyData(from: request),
                  let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
                XCTFail("missing JSON body")
                throw NSError(domain: "stub", code: -1)
            }
            XCTAssertEqual(json["multisig_account_alias"] as? String, "cbdc@banka")
            XCTAssertNil(json["private_key"])
            XCTAssertEqual(json["signer_account_id"] as? String, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
            XCTAssertEqual(json["contract_alias"] as? String, "mint::universal")
            XCTAssertEqual(json["entrypoint"] as? String, "execute")
            XCTAssertEqual(json["gas_limit"] as? Int, 5)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"resolved_multisig_account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","submitted":false,"proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","creation_time_ms":123,"signing_message_b64":"AQ=="}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigContractCallProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            signerAccountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            contractAlias: "mint::universal",
            entrypoint: "execute",
            payload: .object(["amount": .string("10")]),
            gasAssetId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
            feeSponsor: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            gasLimit: 5
        )
        makeClient().proposeMultisigContractCall(request) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
                XCTAssertEqual(response.resolvedMultisigAccountId, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
                XCTAssertEqual(response.proposalId, proposalId)
                XCTAssertEqual(response.instructionsHash, proposalId)
                XCTAssertEqual(response.creationTimeMs, 123)
                XCTAssertEqual(response.signingMessageB64, "AQ==")
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testApproveMultisigContractCallEncodesConcreteSelector() {
        let expectation = expectation(description: "approve multisig contract call")
        let proposalId = String(repeating: "b", count: 64)
        let txHash = String(repeating: "c", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/call/multisig/approve")
            XCTAssertEqual(request.httpMethod, "POST")
            guard let body = self.bodyData(from: request),
                  let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
                XCTFail("missing JSON body")
                throw NSError(domain: "stub", code: -1)
            }
            XCTAssertNil(json["private_key"])
            XCTAssertEqual(json["multisig_account_id"] as? String, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
            XCTAssertEqual(json["signer_account_id"] as? String, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
            XCTAssertEqual(json["proposal_id"] as? String, proposalId)
            XCTAssertEqual(json["signature_b64"] as? String, "AQ==")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"resolved_multisig_account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","submitted":true,"proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","executed_tx_hash_hex":"\(txHash)"}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigContractCallApproveRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"),
            signerAccountId: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            signatureB64: "AQ==",
            proposalId: proposalId
        )
        makeClient().approveMultisigContractCall(request) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
                XCTAssertEqual(response.proposalId, proposalId)
                XCTAssertEqual(response.executedTxHashHex, txHash)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testGetMultisigSpecDecodesResolvedAccount() {
        let expectation = expectation(description: "multisig spec")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/multisig/spec")
            guard let body = self.bodyData(from: request),
                  let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
                XCTFail("missing JSON body")
                throw NSError(domain: "stub", code: -1)
            }
            XCTAssertEqual(json["multisig_account_alias"] as? String, "cbdc@bankb")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"resolved_multisig_account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","spec":{"quorum":2,"transaction_ttl_ms":60000}}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigSpecRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@bankb")
        )
        makeClient().getMultisigSpec(request) { result in
            switch result {
            case .success(let response):
                XCTAssertEqual(response.resolvedMultisigAccountId, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
                XCTAssertEqual(response.spec["quorum"], .number(2))
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testGetMultisigSpecAcceptsDomainScopedAlias() {
        let expectation = expectation(description: "multisig spec with domain-scoped alias")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/multisig/spec")
            guard let body = self.bodyData(from: request),
                  let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
                XCTFail("missing JSON body")
                throw NSError(domain: "stub", code: -1)
            }
            XCTAssertEqual(json["multisig_account_alias"] as? String, "cbdc@banka.universal")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"resolved_multisig_account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","spec":{"quorum":2,"transaction_ttl_ms":60000}}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigSpecRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka.universal")
        )
        makeClient().getMultisigSpec(request) { result in
            switch result {
            case .success(let response):
                XCTAssertEqual(response.resolvedMultisigAccountId, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testGetMultisigSpecRejectsUnsupportedAliasShape() {
        let request = ToriiMultisigSpecRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka.universal.extra")
        )

        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case let ToriiClientError.invalidPayload(message) = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(
                message.contains("name@dataspace or name@domain.dataspace"),
                "Unexpected message: \(message)"
            )
        }
    }

    func testListMultisigProposalsDecodesEntries() {
        let expectation = expectation(description: "multisig proposals list")
        let proposalId = String(repeating: "d", count: 64)
        let approverId = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/multisig/proposals/list")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"resolved_multisig_account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","proposals":[{"proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","proposal":{"approvals":["\(approverId)"]}}]}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposalsListRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka")
        )
        makeClient().listMultisigProposals(request) { result in
            switch result {
            case .success(let response):
                XCTAssertEqual(response.proposals.count, 1)
                XCTAssertEqual(response.proposals.first?.proposalId, proposalId)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testGetMultisigProposalDecodesProposalLookup() {
        let expectation = expectation(description: "multisig proposal get")
        let proposalId = String(repeating: "e", count: 64)
        let approverOne = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"
        let approverTwo = "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/multisig/proposals/get")
            guard let body = self.bodyData(from: request),
                  let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
                XCTFail("missing JSON body")
                throw NSError(domain: "stub", code: -1)
            }
            XCTAssertEqual(json["instructions_hash"] as? String, proposalId)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"resolved_multisig_account_id":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","proposal":{"approvals":["\(approverOne)","\(approverTwo)"]}}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposalGetRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            instructionsHash: proposalId
        )
        makeClient().getMultisigProposal(request) { result in
            switch result {
            case .success(let response):
                XCTAssertEqual(response.proposalId, proposalId)
                XCTAssertEqual(response.instructionsHash, proposalId)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testMultisigSelectorRejectsBothAccountIdAndAlias() throws {
        let selector = ToriiMultisigAccountSelector(
            multisigAccountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            multisigAccountAlias: "cbdc@banka"
        )
        XCTAssertThrowsError(try JSONEncoder().encode(selector)) { error in
            guard case let ToriiClientError.invalidPayload(message) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertTrue(message.contains("exactly one"))
        }
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
            switch request.url?.path {
            case "/v1/node/capabilities":
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, self.nodeCapabilitiesBody())
            case "/v1/pipeline/transactions":
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-norito")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), ToriiWireFormatPreference.noritoPreferred.acceptHeader)
                XCTAssertEqual(self.bodyData(from: request), Data([0x01, 0x02]))
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 202,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let body = """
                {"payload":{"tx_hash":"abc","submitted_at_ms":1,"submitted_at_height":2,"signer":"signer"},"signature":"deadbeef"}
                """.data(using: .utf8)!
                return (response, body)
            default:
                XCTFail("unexpected request: \(request.url?.path ?? "")")
                let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: nil)!
                return (response, Data())
            }
        }

        makeClient().submitTransaction(data: Data([0x01, 0x02])) { result in
            switch result {
            case .success(let payload):
                XCTAssertEqual(payload?.hash, "abc")
                XCTAssertEqual(payload?.payload.submittedAtMs, 1)
                XCTAssertEqual(payload?.payload.submittedAtHeight, 2)
                XCTAssertEqual(payload?.payload.signer, "signer")
                XCTAssertEqual(payload?.signature, "deadbeef")
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testSubmitGovernanceDeployContractProposalEncodesAliasSelector() {
        let expectation = expectation(description: "governance proposal")
        let proposalId = String(repeating: "3", count: 64)
        let codeHash = String(repeating: "4", count: 64)
        let abiHash = String(repeating: "5", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/gov/proposals/deploy-contract")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            let body = self.bodyJSON(from: request)
            XCTAssertEqual(body["contract_alias"] as? String, "demo::universal")
            XCTAssertEqual(body["code_hash"] as? String, codeHash)
            XCTAssertEqual(body["abi_hash"] as? String, abiHash)
            XCTAssertEqual(body["abi_version"] as? String, "1")
            XCTAssertEqual(body["mode"] as? String, "Plain")
            let limits = body["limits"] as? [String: Any]
            XCTAssertEqual(limits?["max_gas"] as? Int, 5000)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let payload = """
            {"ok":true,"proposal_id":"\(proposalId)","tx_instructions":[{"wire_id":"FinalizeReferendum","payload_hex":"00ff"}]}
            """.data(using: .utf8)!
            return (response, payload)
        }

        let request = ToriiGovernanceDeployContractProposalRequest(contractAlias: "demo::universal",
                                                                   codeHashHex: codeHash,
                                                                   abiHashHex: abiHash,
                                                                   abiVersion: "1",
                                                                   window: ToriiGovernanceWindow(lower: 10, upper: 20),
                                                                   mode: .plain,
                                                                   limits: .object(["max_gas": .number(5000)]))
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

    func testGovernanceDeployContractProposalRejectsAmbiguousTarget() throws {
        let request = ToriiGovernanceDeployContractProposalRequest(
            contractAddress: "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8",
            contractAlias: "demo::universal",
            codeHashHex: String(repeating: "4", count: 64),
            abiHashHex: String(repeating: "5", count: 64)
        )
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case let ToriiClientError.invalidPayload(message) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertTrue(message.contains("exactly one of contract_address or contract_alias"))
        }
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
            XCTAssertEqual(body["authority"] as? String, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
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

        let request = ToriiGovernanceZkBallotRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
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
        let request = ToriiGovernanceZkBallotRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
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
        let request = ToriiGovernanceZkBallotRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
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
        let request = ToriiGovernanceZkBallotRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
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
        let request = ToriiGovernanceZkBallotRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
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
        let request = ToriiGovernanceZkBallotRequest(authority: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
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
            XCTAssertTrue(message.contains("owner must be a canonical I105 account id."))
        }
    }

    func testGetGovernanceProposalDecodesRecord() {
        let expectation = expectation(description: "proposal get")
        let proposalId = String(repeating: "6", count: 64)
        let codeHash = String(repeating: "7", count: 64)
        let abiHash = String(repeating: "8", count: 64)
        let contractAddress = "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/gov/proposals/\(proposalId)")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let payload = """
            {"found":true,"proposal":{"proposer":"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB","kind":{"DeployContract":{"contract_address":"\(contractAddress)","code_hash_hex":"\(codeHash)","abi_hash_hex":"\(abiHash)","abi_version":"1"}},"created_height":42,"status":"Approved"}}
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
                XCTAssertEqual(payload.contractAddress, contractAddress)
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
            {"hash":"deadbeef","status":{"kind":"Rejected","block_height":12,"rejection_reason":{"Validation":"missing permission"}},"summary":"Rejected: missing permission","diagnostics":[{"category":"validation","code":"validation","message":"missing permission","decoded_reason":"missing permission","raw_reason":"Validation(missing permission)"}],"scope":"local","resolved_from":"state"}
            """.data(using: .utf8)!
            return (response, body)
        }

        makeClient().getTransactionStatus(hashHex: "deadbeef") { result in
            switch result {
            case .success(let status):
                XCTAssertEqual(status?.kind, "Transaction")
                XCTAssertEqual(status?.content.status.kind, "Rejected")
                XCTAssertEqual(status?.content.status.rejectionReason, #"{"Validation":"missing permission"}"#)
                XCTAssertEqual(status?.summary, "Rejected: missing permission")
                XCTAssertEqual(status?.primaryDiagnostic?.decodedReason, "missing permission")
                XCTAssertEqual(status?.scope, "local")
                XCTAssertEqual(status?.resolvedFrom, "state")
                XCTAssertEqual(status?.isRejected, true)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testGetTransactionStatusReturnsNilFor404() {
        let expectation = expectation(description: "status")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 404,
                                           httpVersion: nil,
                                           headerFields: nil)!
            return (response, nil)
        }

        makeClient().getTransactionStatus(hashHex: "deadbeef") { result in
            switch result {
            case .success(let status):
                XCTAssertNil(status)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionStatusMapsCancelledTransportToCancellationError() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            throw URLError(.cancelled)
        }

        do {
            _ = try await makeClient().getTransactionStatus(hashHex: "deadbeef")
            XCTFail("expected cancellation")
        } catch is CancellationError {
            // expected
        } catch {
            XCTFail("expected CancellationError, got \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testWaitForTerminalTransactionStatusEventUsesSSEAfterInitialStatusWithoutPollingFallback() async throws {
        var statusCallCount = 0
        var sseCallCount = 0
        StubURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/pipeline/transactions/status":
                let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
                XCTAssertEqual(components?.queryItems?.first?.value, "deadbeef")
                statusCallCount += 1
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!
                let body = """
                {"hash":"deadbeef","resolved_from":"cache","scope":"auto","status":{"block_height":168,"kind":"Queued"}}
                """.data(using: .utf8)!
                return (response, body)
            case "/v1/events/sse":
                sseCallCount += 1
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "text/event-stream"]
                )!
                let body = """
                event: Transaction
                data: {"event":"Transaction","hash":"deadbeef","status":"Applied","block_height":169}

                """.data(using: .utf8)!
                return (response, body)
            default:
                throw URLError(.unsupportedURL)
            }
        }

        let event = try await makeClient().waitForTerminalTransactionStatusEvent(
            hashHex: "deadbeef",
            timeout: 2
        )

        XCTAssertEqual(event.hash, "deadbeef")
        XCTAssertEqual(event.status, "Applied")
        XCTAssertEqual(event.blockHeight, 169)
        XCTAssertEqual(statusCallCount, 1)
        XCTAssertEqual(sseCallCount, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionStatusHttpErrorSurfacesBodyMessageAndRejectCode() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 400,
                httpVersion: nil,
                headerFields: ["x-iroha-reject-code": "build_claim_missing"]
            )!
            let body = """
            {"message":"missing build claim for transaction status"}
            """.data(using: .utf8)!
            return (response, body)
        }

        do {
            _ = try await makeClient().getTransactionStatus(hashHex: "deadbeef")
            XCTFail("expected status failure")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, message, rejectCode) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(code, 400)
            XCTAssertEqual(rejectCode, "build_claim_missing")
            XCTAssertEqual(message, "missing build claim for transaction status")
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionStatusHttpErrorUsesNestedJsonMessage() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 502,
                httpVersion: nil,
                headerFields: nil
            )!
            let body = """
            {"error":{"detail":"upstream status pipeline unavailable"}}
            """.data(using: .utf8)!
            return (response, body)
        }

        do {
            _ = try await makeClient().getTransactionStatus(hashHex: "deadbeef")
            XCTFail("expected status failure")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, message, rejectCode) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(code, 502)
            XCTAssertNil(rejectCode)
            XCTAssertEqual(message, "upstream status pipeline unavailable")
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionStatusHttpErrorUsesEnvelopeDetailsRejectCode() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 429,
                httpVersion: nil,
                headerFields: nil
            )!
            let body = """
            {
              "code": "queue_full",
              "message": "transaction queue is at capacity",
              "details": {
                "reject_code": "TX_QUEUE_FULL",
                "retry_after_seconds": 1,
                "queue": {
                  "state": "saturated",
                  "queued": 128,
                  "capacity": 128,
                  "saturated": true
                }
              }
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        do {
            _ = try await makeClient().getTransactionStatus(hashHex: "deadbeef")
            XCTFail("expected status failure")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, message, rejectCode) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(code, 429)
            XCTAssertEqual(rejectCode, "TX_QUEUE_FULL")
            XCTAssertEqual(message, "transaction queue is at capacity")
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionStatusHttpErrorUsesPlainTextBody() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 503,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/plain"]
            )!
            return (response, Data("proxy temporarily unavailable".utf8))
        }

        do {
            _ = try await makeClient().getTransactionStatus(hashHex: "deadbeef")
            XCTFail("expected status failure")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, message, rejectCode) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(code, 503)
            XCTAssertNil(rejectCode)
            XCTAssertEqual(message, "proxy temporarily unavailable")
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionStatusHttpErrorUsesErrorsArrayMessage() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 422,
                httpVersion: nil,
                headerFields: nil
            )!
            let body = """
            {
              "errors": [
                {"message":"status query validation failed"},
                {"message":"hash malformed"}
              ]
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        do {
            _ = try await makeClient().getTransactionStatus(hashHex: "deadbeef")
            XCTFail("expected status failure")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, message, rejectCode) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(code, 422)
            XCTAssertNil(rejectCode)
            XCTAssertEqual(message, "status query validation failed")
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionStatusHttpErrorUsesCompactJsonBody() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 500,
                httpVersion: nil,
                headerFields: nil
            )!
            let body = """
            {"status":"invalid","code":"E123"}
            """.data(using: .utf8)!
            return (response, body)
        }

        do {
            _ = try await makeClient().getTransactionStatus(hashHex: "deadbeef")
            XCTFail("expected status failure")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, message, rejectCode) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(code, 500)
            XCTAssertNil(rejectCode)
            XCTAssertEqual(message, #"{"code":"E123","status":"invalid"}"#)
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionStatusHttpErrorTruncatesOversizedBodyText() async throws {
        let oversized = String(repeating: "x", count: 700)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 500,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/plain"]
            )!
            return (response, Data(oversized.utf8))
        }

        do {
            _ = try await makeClient().getTransactionStatus(hashHex: "deadbeef")
            XCTFail("expected status failure")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, message, rejectCode) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(code, 500)
            XCTAssertNil(rejectCode)
            let value = try XCTUnwrap(message)
            XCTAssertEqual(value.count, 515)
            XCTAssertTrue(value.hasSuffix("..."), "message should be truncated with ASCII ellipsis")
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionStatusMatchesSharedErrorMessageContractFixture() async throws {
        let fixtureCases = try loadTxStatusErrorContractCases()
        XCTAssertFalse(fixtureCases.isEmpty, "fixture cases should not be empty")

        for fixtureCase in fixtureCases {
            StubURLProtocol.handler = { request in
                XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
                var headers: [String: String] = [:]
                if let contentType = fixtureCase.contentType {
                    headers["Content-Type"] = contentType
                }
                if let rejectCode = fixtureCase.rejectCodeHeader {
                    headers[fixtureCase.rejectCodeHeaderName ?? "X-Iroha-Reject-Code"] = rejectCode
                }
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: fixtureCase.statusCode,
                    httpVersion: nil,
                    headerFields: headers.isEmpty ? nil : headers
                )!
                let bodyData: Data?
                if let bodyJSON = fixtureCase.bodyJSON {
                    bodyData = try JSONSerialization.data(withJSONObject: bodyJSON, options: [])
                } else if let bodyText = fixtureCase.bodyText {
                    bodyData = Data(bodyText.utf8)
                } else {
                    bodyData = nil
                }
                return (response, bodyData)
            }

            do {
                _ = try await makeClient().getTransactionStatus(hashHex: "deadbeef")
                XCTFail("\(fixtureCase.id): expected status failure")
            } catch let error as ToriiClientError {
                guard case let .httpStatus(code, message, rejectCode) = error else {
                    return XCTFail("\(fixtureCase.id): unexpected error shape \(error)")
                }
                XCTAssertEqual(code, fixtureCase.statusCode, "\(fixtureCase.id): status code mismatch")
                if let expectedRejectCode = fixtureCase.expectedRejectCode {
                    XCTAssertEqual(rejectCode, expectedRejectCode, "\(fixtureCase.id): reject code mismatch")
                }
                if let expectedMessage = fixtureCase.expectedMessage {
                    XCTAssertEqual(message, expectedMessage, "\(fixtureCase.id): message mismatch")
                }
                if let expectedLength = fixtureCase.expectedMessageLength {
                    XCTAssertEqual(message?.count, expectedLength, "\(fixtureCase.id): message length mismatch")
                }
                if let expectedSuffix = fixtureCase.expectedMessageSuffix {
                    XCTAssertEqual(message?.hasSuffix(expectedSuffix), true, "\(fixtureCase.id): message suffix mismatch")
                }
            } catch {
                XCTFail("\(fixtureCase.id): unexpected error: \(error)")
            }
        }
    }

    func testPipelineStatusStateMapping() throws {
        let nestedJSON = """
        {"kind":"Transaction","content":{"hash":"deadbeef","status":{"kind":"Committed","content":null}}}
        """.data(using: .utf8)!
        let nested = try JSONDecoder().decode(ToriiPipelineTransactionStatus.self, from: nestedJSON)
        XCTAssertEqual(nested.kind, "Transaction")
        XCTAssertEqual(nested.content.hash, "deadbeef")
        XCTAssertEqual(nested.content.status.state, .committed)
        XCTAssertTrue(nested.content.status.state.isKnownTerminalSuccess)
        XCTAssertFalse(PipelineTransactionState.approved.isKnownTerminalSuccess)

        let flatJSON = """
        {"hash":"facefeed","resolved_from":"cache","scope":"auto","status":{"block_height":64,"kind":"Applied"},"diagnostics":[]}
        """.data(using: .utf8)!
        let flat = try JSONDecoder().decode(ToriiPipelineTransactionStatus.self, from: flatJSON)
        XCTAssertEqual(flat.kind, "Transaction")
        XCTAssertEqual(flat.content.hash, "facefeed")
        XCTAssertEqual(flat.content.status.state, .applied)
        XCTAssertTrue(flat.content.status.state.isTerminalSuccess)
        XCTAssertTrue(flat.isCommitted)

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
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
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
    func testGetMetricsRejectsJSONWithoutHeader() async throws {
        let body = #"{"ok":true}"#.data(using: .utf8)!
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/metrics")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: [:])!
            return (response, body)
        }
        do {
            _ = try await makeClient().getMetrics()
            XCTFail("expected missing metrics Content-Type to fail")
        } catch let error as ToriiClientError {
            guard case let .invalidPayload(reason) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertTrue(reason.contains("Content-Type"))
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }
}

#if os(macOS)
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
        try await preparePipelineScenario(.success,
                                          hashHex: scenarioHash,
                                          statusKinds: ["Queued", "Approved", "Committed"])
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
        XCTAssertTrue(status.content.status.state.isTerminalSuccess)
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
#endif

private struct TxStatusErrorContractCase {
    let id: String
    let statusCode: Int
    let contentType: String?
    let bodyJSON: Any?
    let bodyText: String?
    let rejectCodeHeader: String?
    let rejectCodeHeaderName: String?
    let expectedMessage: String?
    let expectedRejectCode: String?
    let expectedMessageLength: Int?
    let expectedMessageSuffix: String?

    init?(raw: [String: Any]) {
        guard let id = raw["id"] as? String,
              let statusCode = raw["status_code"] as? Int
        else {
            return nil
        }
        self.id = id
        self.statusCode = statusCode
        contentType = raw["content_type"] as? String
        bodyJSON = raw["body_json"]
        bodyText = raw["body_text"] as? String
        rejectCodeHeader = raw["reject_code_header"] as? String
        rejectCodeHeaderName = raw["reject_code_header_name"] as? String
        expectedMessage = raw["expected_message"] as? String
        expectedRejectCode = raw["expected_reject_code"] as? String
        expectedMessageLength = raw["expected_message_length"] as? Int
        expectedMessageSuffix = raw["expected_message_suffix"] as? String
    }
}

private func loadTxStatusErrorContractCases() throws -> [TxStatusErrorContractCase] {
    let fixtureURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent() // ToriiClientTests.swift
        .deletingLastPathComponent() // IrohaSwiftTests
        .deletingLastPathComponent() // Tests
        .deletingLastPathComponent() // IrohaSwift
        .appendingPathComponent("fixtures/sdk/tx_status_error_message_contract.json")
    let data = try Data(contentsOf: fixtureURL)
    guard let root = try JSONSerialization.jsonObject(with: data) as? [String: Any],
          let rawCases = root["cases"] as? [[String: Any]]
    else {
        throw NSError(domain: "ToriiClientTests",
                      code: -1,
                      userInfo: [NSLocalizedDescriptionKey: "invalid tx-status error-message contract fixture"])
    }
    return rawCases.compactMap(TxStatusErrorContractCase.init(raw:))
}
