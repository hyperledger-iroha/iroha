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

    private func makeClient(baseURL: URL = URL(string: "https://example.test")!) -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        return ToriiClient(baseURL: baseURL, session: session)
    }

    private func nodeCapabilitiesBody(dataModelVersion: Int = ToriiNodeCapabilities.expectedDataModelVersion) -> Data {
        let payload: [String: Any] = [
            "abi_version": 1,
            "data_model_version": dataModelVersion
        ]
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

    private func signedIdentifierReceiptFixture(accountId: String,
                                                resolvedAtMs: UInt64 = 42,
                                                expiresAtMs: UInt64? = 142) throws -> (resolverPublicKey: String, signatureHex: String, signaturePayloadHex: String) {
        let privateKey = Curve25519.Signing.PrivateKey()
        let multihash = OfflineNorito.publicKeyMultihash(
            algorithm: .ed25519,
            payload: privateKey.publicKey.rawRepresentation
        )
        let payloadBytes = Data([0x01, 0x02, 0x03, 0x04, 0xA0])
        var digest = Blake2b.hash256(payloadBytes)
        digest[digest.count - 1] |= 0x01
        let signature = try privateKey.signature(for: digest)
        return (
            resolverPublicKey: "ed25519:\(multihash)",
            signatureHex: signature.hexUppercased(),
            signaturePayloadHex: payloadBytes.hexUppercased()
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsAsync() async throws {
        StubURLProtocol.handler = { request in
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/accounts/sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB/assets"))
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset":"66owaQmAQMuHxPzxUN3bqZ6FJfDa","account_id":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","scope":"global","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(
            accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            asset: nil
        )
        XCTAssertEqual(balances.count, 1)
        XCTAssertEqual(balances.first?.asset, roseAssetDefinitionId)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsAsyncDecodesAssetFieldsDirectly() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/accounts/sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB/assets"))
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset":"66owaQmAQMuHxPzxUN3bqZ6FJfDa","account_id":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","scope":"global","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(
            accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            asset: nil
        )
        XCTAssertEqual(balances.count, 1)
        guard let item = balances.first else {
            XCTFail("missing asset balance")
            return
        }
        XCTAssertEqual(item.asset, roseAssetDefinitionId)
        XCTAssertEqual(item.accountId, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
        XCTAssertEqual(item.scope, "global")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsAsyncDecodesReadableAssetFields() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/accounts/sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB/assets"))
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{
              "asset":"66owaQmAQMuHxPzxUN3bqZ6FJfDa",
              "account_id":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
              "scope":"global",
              "asset_name":"USD",
              "asset_alias":"usd#issuer.main",
              "quantity":"10"
            }]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(
            accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            asset: nil
        )
        XCTAssertEqual(balances.count, 1)
        XCTAssertEqual(balances.first?.asset, roseAssetDefinitionId)
        XCTAssertEqual(balances.first?.accountId, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
        XCTAssertEqual(balances.first?.scope, "global")
        XCTAssertEqual(balances.first?.assetName, "USD")
        XCTAssertEqual(balances.first?.assetAlias, "usd#issuer.main")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsPreservesPercentEncodedPathWithBasePath() async throws {
        let baseURL = URL(string: "https://example.test/api")!
        let client = makeClient(baseURL: baseURL)
        StubURLProtocol.handler = { request in
            // Use absoluteString to verify percent-encoding is preserved.
            // URL.path always returns decoded path (@ instead of %40) by design.
            XCTAssertTrue(request.url!.absoluteString.contains("/api/v1/accounts/sorauロ1NfコキリcルヲEムgsKti4Zリ6HKウZCナクシ16fvSイymカサリホ29JNWE/assets"))
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = "[]".data(using: .utf8)!
            return (response, body)
        }

        let balances = try await client.getAssets(
            accountId: "sorauロ1NfコキリcルヲEムgsKti4Zリ6HKウZCナクシ16fvSイymカサリホ29JNWE",
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
              "accountId":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
              "index":7,
              "source":"world_state"
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let resolved = try await makeClient().resolveAccountAlias("alice")
        XCTAssertEqual(resolved?.alias, "alice")
        XCTAssertEqual(resolved?.accountId, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
        XCTAssertEqual(resolved?.index, 7)
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
        let owner = "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"
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
                  "encrypted_input_mode":"resolver_canonicalized_envelope_v1",
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
            .resolverCanonicalizedEnvelopeV1
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListIdentifierPoliciesAcceptsTaggedEncryptedInputMode() async throws {
        let owner = "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"
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
                    "mode":"ResolverCanonicalizedEnvelopeV1",
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
            .resolverCanonicalizedEnvelopeV1
        )
        XCTAssertEqual(
            response.items.first?.inputEncryptionPublicParametersDecoded?.maxInputBytes,
            63
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListRamLfeProgramPoliciesAsync() async throws {
        let owner = "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"
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
                  "encrypted_input_mode":"resolver_canonicalized_envelope_v1",
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
            XCTAssertEqual(payload["input_hex"] as? String, "abcd")
            XCTAssertNil(payload["encrypted_input"])
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
              "program_id":"identifier_lookup_retail",
              "opaque_hash":"opaque-hash-literal",
              "receipt_hash":"receipt-hash-literal",
              "output_hex":"c0ffee",
              "output_hash":"output-hash-literal",
              "associated_data_hash":"associated-data-hash-literal",
              "executed_at_ms":42,
              "expires_at_ms":142,
              "backend":"bfv-programmed-sha3-256-v1",
              "verification_mode":"signed",
              "receipt":{
                "payload":{
                  "program_id":{"name":"identifier_lookup_retail"},
                  "program_digest":"hash:\(String(repeating: "11", count: 32).uppercased())#ABCD",
                  "backend":"bfv-programmed-sha3-256-v1",
                  "verification_mode":{"mode":"Signed","value":null},
                  "output_hash":"hash:\(String(repeating: "22", count: 32).uppercased())#BCDE",
                  "associated_data_hash":"hash:\(String(repeating: "33", count: 32).uppercased())#CDEF",
                  "executed_at_ms":42,
                  "expires_at_ms":142
                },
                "signature":"\(String(repeating: "aa", count: 64))"
              }
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let response = try await makeClient().executeRamLfeProgram(
            programId: "identifier_lookup_retail",
            inputHex: "0xABCD"
        )
        XCTAssertEqual(response?.programId, "identifier_lookup_retail")
        XCTAssertEqual(response?.outputHex, "c0ffee")
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
                "program_id": .object(["name": .string("identifier_lookup_retail")]),
                "program_digest": .string("hash:\(String(repeating: "11", count: 32).uppercased())#ABCD"),
                "backend": .string("bfv-programmed-sha3-256-v1"),
                "verification_mode": .object([
                    "mode": .string("Signed"),
                    "value": .null
                ]),
                "output_hash": .string("hash:\(String(repeating: "22", count: 32).uppercased())#BCDE"),
                "associated_data_hash": .string("hash:\(String(repeating: "33", count: 32).uppercased())#CDEF"),
                "executed_at_ms": .number(42),
                "expires_at_ms": .number(142)
            ]),
            "signature": .string(String(repeating: "aa", count: 64))
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
        let accountId = "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"
        let opaqueId = "opaque:\(String(repeating: "11", count: 32))"
        let receiptHash = String(repeating: "22", count: 32)
        let uaid = "uaid:\(String(repeating: "33", count: 31))35"
        let signed = try signedIdentifierReceiptFixture(accountId: accountId)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/identifiers/resolve")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            let payload = self.bodyJSON(from: request)
            XCTAssertEqual(payload["policy_id"] as? String, "phone#retail")
            XCTAssertEqual(payload["input"] as? String, "+1 (555) 123-4567")
            XCTAssertNil(payload["encrypted_input"])
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
              "accountId":"\(accountId)",
              "resolved_at_ms":42,
              "expires_at_ms":142,
              "backend":"bfv-affine-sha3-256-v1",
              "signature":"\(signed.signatureHex)",
              "signature_payload_hex":"\(signed.signaturePayloadHex)",
              "signature_payload":{
                "policy_id":"phone#retail",
                "opaque_id":"\(opaqueId)",
                "receipt_hash":"\(receiptHash)",
                "uaid":"\(uaid)",
                "accountId":"\(accountId)",
                "resolved_at_ms":42,
                "expires_at_ms":142
              }
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let receipt = try await makeClient().resolveIdentifier(
            policyId: " phone#retail ",
            input: " +1 (555) 123-4567 "
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
            note: nil
        )
        XCTAssertEqual(try receipt?.verifySignature(using: policy), true)
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
            encryptedInputHex: "0xABCD"
        )
        XCTAssertNil(receipt)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testResolveIdentifierDecodesNestedExecutionPayload() async throws {
        let accountId = try canonicalOwnerLiteral()
        let opaqueId = "opaque:\(String(repeating: "77", count: 32))"
        let receiptHash = String(repeating: "88", count: 32)
        let uaid = "uaid:\(String(repeating: "99", count: 31))9b"
        let signed = try signedIdentifierReceiptFixture(accountId: accountId,
                                                        resolvedAtMs: 42,
                                                        expiresAtMs: 142)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/identifiers/resolve")
            XCTAssertEqual(request.httpMethod, "POST")
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
              "accountId":"\(accountId)",
              "resolved_at_ms":42,
              "expires_at_ms":142,
              "backend":"bfv-programmed-sha3-256-v1",
              "signature":"\(signed.signatureHex)",
              "signature_payload_hex":"\(signed.signaturePayloadHex)",
              "signature_payload":{
                "policy_id":{
                  "kind":"phone",
                  "business_rule":"retail"
                },
                "execution":{
                  "program_id":{
                    "name":"identifier_lookup_retail"
                  },
                  "program_digest":"hash:\(String(repeating: "11", count: 32).uppercased())#ABCD",
                  "backend":"bfv-programmed-sha3-256-v1",
                  "verification_mode":{
                    "mode":"Signed",
                    "value":null
                  },
                  "output_hash":"hash:\(String(repeating: "22", count: 32).uppercased())#BCDE",
                  "associated_data_hash":"hash:\(String(repeating: "33", count: 32).uppercased())#CDEF",
                  "executed_at_ms":42,
                  "expires_at_ms":142
                },
                "opaque_id":[
                  "hash:\(String(repeating: "77", count: 32).uppercased())#1234"
                ],
                "receipt_hash":"hash:\(receiptHash.uppercased())#5678",
                "uaid":[
                  "hash:\(String(repeating: "99", count: 31).uppercased())9B#9ABC"
                ],
                "accountId":"\(accountId)"
              }
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let receipt = try await makeClient().resolveIdentifier(
            policyId: "phone#retail",
            encryptedInputHex: "ABCD"
        )
        XCTAssertEqual(receipt?.resolvedAtMs, 42)
        XCTAssertEqual(receipt?.expiresAtMs, 142)
        XCTAssertEqual(receipt?.signaturePayload.policyId, "phone#retail")
        XCTAssertEqual(receipt?.signaturePayload.opaqueId, opaqueId)
        XCTAssertEqual(receipt?.signaturePayload.receiptHash, receiptHash)
        XCTAssertEqual(receipt?.signaturePayload.uaid, uaid)
        XCTAssertEqual(receipt?.signaturePayload.execution?.programId, "identifier_lookup_retail")
        XCTAssertEqual(receipt?.signaturePayload.execution?.programDigest, String(repeating: "11", count: 32))
        XCTAssertEqual(receipt?.signaturePayload.execution?.verificationMode, "signed")
        XCTAssertEqual(receipt?.signaturePayload.execution?.outputHash, String(repeating: "22", count: 32))
        XCTAssertEqual(receipt?.signaturePayload.execution?.associatedDataHash, String(repeating: "33", count: 32))
        XCTAssertEqual(receipt?.signaturePayload.execution?.executedAtMs, 42)
        XCTAssertEqual(receipt?.signaturePayload.execution?.expiresAtMs, 142)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testIssueIdentifierClaimReceiptAsync() async throws {
        let accountId = try canonicalOwnerLiteral()
        let opaqueId = "opaque:\(String(repeating: "44", count: 32))"
        let receiptHash = String(repeating: "55", count: 32)
        let uaid = "uaid:\(String(repeating: "66", count: 31))67"
        let signed = try signedIdentifierReceiptFixture(accountId: accountId,
                                                        resolvedAtMs: 7,
                                                        expiresAtMs: nil)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(
                request.url?.path,
                "/v1/accounts/\(accountId)/identifiers/claim-receipt"
            )
            XCTAssertEqual(request.httpMethod, "POST")
            let payload = self.bodyJSON(from: request)
            XCTAssertEqual(payload["policy_id"] as? String, "phone#retail")
            XCTAssertEqual(payload["encrypted_input"] as? String, "abcd")
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
              "accountId":"\(accountId)",
              "resolved_at_ms":7,
              "backend":"bfv-affine-sha3-256-v1",
              "signature":"\(signed.signatureHex)",
              "signature_payload_hex":"\(signed.signaturePayloadHex)",
              "signature_payload":{
                "policy_id":"phone#retail",
                "opaque_id":"\(opaqueId)",
                "receipt_hash":"\(receiptHash)",
                "uaid":"\(uaid)",
                "accountId":"\(accountId)",
                "resolved_at_ms":7
              }
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let receipt = try await makeClient().issueIdentifierClaimReceipt(
            accountId: accountId,
            policyId: "phone#retail",
            encryptedInputHex: "ABCD"
        )
        XCTAssertEqual(receipt?.opaqueId, opaqueId)
        XCTAssertEqual(receipt?.receiptHash, receiptHash)
        XCTAssertEqual(receipt?.uaid, uaid)
        XCTAssertEqual(receipt?.accountId, accountId)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testIssueIdentifierClaimReceiptDecodesNestedExecutionPayload() async throws {
        let accountId = try canonicalOwnerLiteral()
        let opaqueId = "opaque:\(String(repeating: "aa", count: 32))"
        let receiptHash = String(repeating: "bb", count: 32)
        let uaid = "uaid:\(String(repeating: "cc", count: 31))cd"
        let signed = try signedIdentifierReceiptFixture(accountId: accountId,
                                                        resolvedAtMs: 7,
                                                        expiresAtMs: 77)
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
            let body = """
            {
              "policy_id":"phone#retail",
              "opaque_id":"\(opaqueId)",
              "receipt_hash":"\(receiptHash)",
              "uaid":"\(uaid)",
              "accountId":"\(accountId)",
              "resolved_at_ms":7,
              "expires_at_ms":77,
              "backend":"bfv-programmed-sha3-256-v1",
              "signature":"\(signed.signatureHex)",
              "signature_payload_hex":"\(signed.signaturePayloadHex)",
              "signature_payload":{
                "policy_id":{
                  "kind":"phone",
                  "business_rule":"retail"
                },
                "execution":{
                  "program_id":{
                    "name":"identifier_lookup_retail"
                  },
                  "program_digest":"hash:\(String(repeating: "12", count: 32).uppercased())#CDEF",
                  "backend":"bfv-programmed-sha3-256-v1",
                  "verification_mode":{
                    "mode":"Signed",
                    "value":null
                  },
                  "output_hash":"hash:\(String(repeating: "23", count: 32).uppercased())#DEF0",
                  "associated_data_hash":"hash:\(String(repeating: "34", count: 32).uppercased())#EF01",
                  "executed_at_ms":7,
                  "expires_at_ms":77
                },
                "opaque_id":[
                  "hash:\(String(repeating: "aa", count: 32).uppercased())#0ABC"
                ],
                "receipt_hash":"hash:\(receiptHash.uppercased())#1BCD",
                "uaid":[
                  "hash:\(String(repeating: "cc", count: 31).uppercased())CD#2CDE"
                ],
                "accountId":"\(accountId)"
              }
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let receipt = try await makeClient().issueIdentifierClaimReceipt(
            accountId: accountId,
            policyId: "phone#retail",
            encryptedInputHex: "ABCD"
        )
        XCTAssertEqual(receipt?.resolvedAtMs, 7)
        XCTAssertEqual(receipt?.expiresAtMs, 77)
        XCTAssertEqual(receipt?.signaturePayload.policyId, "phone#retail")
        XCTAssertEqual(receipt?.signaturePayload.opaqueId, opaqueId)
        XCTAssertEqual(receipt?.signaturePayload.receiptHash, receiptHash)
        XCTAssertEqual(receipt?.signaturePayload.uaid, uaid)
        XCTAssertEqual(receipt?.signaturePayload.execution?.programId, "identifier_lookup_retail")
        XCTAssertEqual(receipt?.signaturePayload.execution?.programDigest, String(repeating: "12", count: 32))
        XCTAssertEqual(receipt?.signaturePayload.execution?.verificationMode, "signed")
        XCTAssertEqual(receipt?.signaturePayload.execution?.outputHash, String(repeating: "23", count: 32))
        XCTAssertEqual(receipt?.signaturePayload.execution?.associatedDataHash, String(repeating: "34", count: 32))
        XCTAssertEqual(receipt?.signaturePayload.execution?.executedAtMs, 7)
        XCTAssertEqual(receipt?.signaturePayload.execution?.expiresAtMs, 77)
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
              "accountId":"\(accountId)",
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

    func testIdentifierLookupRequestBuilderCanonicalizesPolicyInput() throws {
        let policy = ToriiIdentifierPolicySummary(
            policyId: "phone#retail",
            owner: try canonicalOwnerLiteral(),
            active: true,
            normalization: .phoneE164,
            resolverPublicKey: "ed25519:ed0120" + String(repeating: "11", count: 32),
            backend: "bfv-affine-sha3-256-v1",
            inputEncryption: "bfv-v1",
            inputEncryptionPublicParameters: nil,
            inputEncryptionPublicParametersDecoded: nil,
            ramFheProfile: nil,
            note: nil
        )
        let request = try policy.plaintextRequest(input: " +1 (555) 123-4567 ")
        XCTAssertEqual(request.policyId, "phone#retail")
        XCTAssertEqual(request.input, "+15551234567")
        XCTAssertNil(request.encryptedInputHex)
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
                    plaintextModulus: 256,
                    ciphertextModulus: 16_777_216,
                    decompositionBaseLog: 12
                ),
                publicKey: ToriiIdentifierBfvPublicKey(
                    b: [11_472_226, 15_791_131, 10_301_391, 6_321_610, 502_045, 1_948_157, 5_332_249, 12_641_494],
                    a: [3_503_246, 2_379_264, 12_091_019, 30_169, 15_804_162, 8_155_629, 2_418_997, 3_003_107]
                ),
                maxInputBytes: 3
            ),
            ramFheProfile: nil,
            note: nil
        )
        let seedHex = "00112233445566778899AABBCCDDEEFF00112233445566778899AABBCCDDEEFF"
        let expected =
            "4e525430000035a9bf76d68dbb0c35a9bf76d68dbb0c00b0040000000000007f6fd892e275492500a804000000000000040000000000000020010000000000008800000000000000080000000000000008000000000000002bab6f00000000000800000000000000440e93000000000008000000000000005b2502000000000008000000000000004a671400000000000800000000000000bc3e2600000000000800000000000000413d86000000000008000000000000005619f800000000000800000000000000bd73fa0000000000880000000000000008000000000000000800000000000000ee884300000000000800000000000000dd21b100000000000800000000000000fe7c52000000000008000000000000001639a5000000000008000000000000006a979d00000000000800000000000000ddd4430000000000080000000000000051086700000000000800000000000000ef13ae00000000002001000000000000880000000000000008000000000000000800000000000000776dc80000000000080000000000000093060d0000000000080000000000000033077500000000000800000000000000ddc4190000000000080000000000000062ea230000000000080000000000000056ef0b00000000000800000000000000ab52d500000000000800000000000000e9457c0000000000880000000000000008000000000000000800000000000000f2214200000000000800000000000000c9edcf000000000008000000000000001dfb5a00000000000800000000000000d16e640000000000080000000000000016ec0f000000000008000000000000003dee83000000000008000000000000006e7efa00000000000800000000000000c1fbbc0000000000200100000000000088000000000000000800000000000000080000000000000066c74d00000000000800000000000000c9c04900000000000800000000000000f01e8700000000000800000000000000aed22c000000000008000000000000006121980000000000080000000000000036ac8d00000000000800000000000000d143930000000000080000000000000089206d0000000000880000000000000008000000000000000800000000000000417ded00000000000800000000000000d79c33000000000008000000000000009f332d0000000000080000000000000091fe5700000000000800000000000000533de8000000000008000000000000005db9df00000000000800000000000000a8c213000000000008000000000000006e03c20000000000200100000000000088000000000000000800000000000000080000000000000003d656000000000008000000000000005d874500000000000800000000000000567ab30000000000080000000000000007272f00000000000800000000000000ff6d0a00000000000800000000000000077467000000000008000000000000006d1c1a00000000000800000000000000704fc100000000008800000000000000080000000000000008000000000000002f884f0000000000080000000000000041b0a000000000000800000000000000cbf92a000000000008000000000000005748720000000000080000000000000060909200000000000800000000000000f5f5dc00000000000800000000000000445a3a00000000000800000000000000999f680000000000"

        XCTAssertEqual(try policy.encryptInput("ab", seedHex: seedHex), expected)
        let request = try policy.encryptedRequest(input: "ab", seedHex: seedHex)
        XCTAssertEqual(request.policyId, "string#retail")
        XCTAssertNil(request.input)
        XCTAssertEqual(request.encryptedInputHex, expected)
    }

    func testIdentifierBfvEnvelopeBuilderMatchesLiveJsVector() throws {
        let fixtureURL = URL(fileURLWithPath: "/tmp/js_email_identifier_request.json")
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
            note: nil
        )
        let seedHex = "00112233445566778899AABBCCDDEEFF00112233445566778899AABBCCDDEEFF"
        let actual = try policy.encryptInput("ios.ubl.live@example.com", seedHex: seedHex)

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
            case "/transaction":
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-norito")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/x-norito, application/json")
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
            case "/transaction":
                XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-norito")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/x-norito, application/json")
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
            case "/transaction":
                XCTFail("transaction submitted with incompatible data model")
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
            guard case let .incompatibleDataModel(expected, actual) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(expected, ToriiNodeCapabilities.expectedDataModelVersion)
            XCTAssertEqual(actual, 9)
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
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/accounts/sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB/assets"))
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset":"66owaQmAQMuHxPzxUN3bqZ6FJfDa","account_id":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","scope":"global","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!, session: session)

        let balances = try await sdk.getAssets(
            accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            asset: nil
        )
        XCTAssertEqual(balances.count, 1)
        XCTAssertEqual(balances.first?.asset, roseAssetDefinitionId)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsTrimsAndEncodesAccountLiteral() async throws {
        StubURLProtocol.handler = { request in
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/accounts/sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB/assets"))
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset":"66owaQmAQMuHxPzxUN3bqZ6FJfDa","account_id":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","scope":"global","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(
            accountId: "  sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB  ",
            asset: nil
        )
        XCTAssertEqual(balances.count, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsRejectsPercentEscapedAccountLiteral() async {
        await XCTAssertThrowsErrorAsync(
            try await makeClient().getAssets(
                accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB%2Fsorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
            ),
            expectation: { _ in }
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsEncodesAssetSelectorFilter() async throws {
        let assetId = roseAssetDefinitionId
        StubURLProtocol.handler = { request in
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/accounts/sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB/assets"))
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let assetFilter = components?.queryItems?.first(where: { $0.name == "asset" })?.value
            XCTAssertEqual(assetFilter, assetId)
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset":"\(assetId)","account_id":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","scope":"global","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB", asset: assetId)
        XCTAssertEqual(balances.count, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionsEncodesAccountLiteral() async throws {
        StubURLProtocol.handler = { request in
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/accounts/sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB/transactions"))
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"items":[{"entrypoint_hash":"hash","authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","timestamp_ms":1,"result_ok":true}],"total":1}
            """.data(using: .utf8)!
            return (response, body)
        }

        let transactions = try await makeClient().getTransactions(accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
        XCTAssertEqual(transactions.total, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionsEncodesAssetIdFilter() async throws {
        let assetId = self.encodedRoseAssetID
        StubURLProtocol.handler = { request in
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/accounts/sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB/transactions"))
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let assetFilter = components?.queryItems?.first(where: { $0.name == "asset" })?.value
            XCTAssertEqual(assetFilter, assetId)
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"items":[{"entrypoint_hash":"hash","authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","timestamp_ms":1,"result_ok":true}],"total":1}
            """.data(using: .utf8)!
            return (response, body)
        }

        let transactions = try await makeClient().getTransactions(accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB", assetDefinitionId: assetId)
        XCTAssertEqual(transactions.total, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerAccountQrDecodesResponse() async throws {
        StubURLProtocol.handler = { request in
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/explorer/accounts/sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB/qr"))
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

        let qr = try await makeClient().getExplorerAccountQr(accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
        XCTAssertEqual(qr.canonicalId, "i105example")
        XCTAssertEqual(qr.literal, "i105example")
        XCTAssertEqual(qr.networkPrefix, 0)
        XCTAssertEqual(qr.modules, 192)
        XCTAssertEqual(qr.qrVersion, 5)
        XCTAssertTrue(qr.svg.contains("<svg"))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerAccountQrAcceptsAccountAliasPathLiteral() async throws {
        let alias = "operator@hbl.universal"
        StubURLProtocol.handler = { request in
            XCTAssertTrue(
                request.url!.absoluteString.contains(
                    "/v1/explorer/accounts/operator%40hbl.universal/qr"
                )
            )
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "canonical_id":"i105example",
                "literal":"operator@hbl.universal",
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
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/explorer/accounts/sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB/qr"))
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

        let qr = try await makeClient().getExplorerAccountQr(accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
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
            XCTAssertEqual(query["account"], "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
            XCTAssertEqual(query["authority"], "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
            XCTAssertEqual(query["transaction_hash"], "deadbeef")
            XCTAssertEqual(query["transaction_status"], "Committed")
            XCTAssertEqual(query["block"], "5")
            XCTAssertEqual(query["kind"], "Transfer")
            XCTAssertEqual(query["asset"], "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "pagination": {"page":2,"per_page":25,"total_pages":1,"total_items":1},
                "items": [
                    {
                        "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                        "created_at":"2025-01-01T00:00:00Z",
                        "kind":"Transfer",
                        "box":{
                            "scale":"0xdead",
                            "json":{
                                "kind":"Transfer",
                                "payload":{
                                    "Asset":{
                                        "source":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                                        "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
                                                     account: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                                                     authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
            "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            "created_at":"2025-01-01T00:00:00Z",
            "kind":"Transfer",
            "r#box":{
                "scale":"0x00",
                "json":{
                    "kind":"Transfer",
                    "payload":{
                        "variant":"Asset",
                        "value":{
                            "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                            "object":"10",
                            "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
            XCTAssertEqual(asset.destinationAccountId, "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
            XCTAssertEqual(asset.amount, "10")
            XCTAssertNil(asset.senderAccountId)
            XCTAssertEqual(asset.assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            XCTAssertNil(details.role(for: "sorauロ1PgノタXヨnWアハキユネjキZヨrナxイWヤタリYヰヘxコタテロスfヨ2Gイ8P3LSM"))
            XCTAssertEqual(details.role(for: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"), .receiver)
            XCTAssertTrue(details.involvesAccount("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"))
            XCTAssertTrue(details.involvesAssetDefinition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"))
            XCTAssertFalse(details.involvesAssetDefinition("61CtjvNd9T3THAR65GsMVHr82Bjc"))
        case .assetBatch:
            XCTFail("Expected asset transfer details.")
        }
    }

    func testExplorerTransferDetailsParsesAssetBatch() throws {
        let json = """
        {
            "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            "created_at":"2025-01-01T00:00:00Z",
            "kind":"Transfer",
            "box":{
                "scale":"0x00",
                "json":{
                    "kind":"Transfer",
                    "payload":{
                        "variant":"AssetBatch",
                        "value":{
                            "entries":[
                                {
                                    "from":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                                    "to":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                                    "asset_definition":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "amount":"5"
                                },
                                {
                                    "from":"sorauロ1Pタレソ1ヘカsFイAfセeB3スヱヱウcyハyケ1ツネヰヰ6ロヰEAテアウヨLPN4XM",
                                    "to":"sorauロ1PワKNラ7シコa2WクシメミQホbコトocニチヰJaアbg6sセgイヨPfX7WAWRY",
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
            XCTAssertEqual(entries[0].senderAccountId, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
            XCTAssertEqual(entries[0].receiverAccountId, "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
            XCTAssertEqual(entries[0].assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            XCTAssertEqual(entries[0].amount, "5")
            XCTAssertEqual(entries[1].senderAccountId, "sorauロ1Pタレソ1ヘカsFイAfセeB3スヱヱウcyハyケ1ツネヰヰ6ロヰEAテアウヨLPN4XM")
            XCTAssertEqual(entries[1].receiverAccountId, "sorauロ1PワKNラ7シコa2WクシメミQホbコトocニチヰJaアbg6sセgイヨPfX7WAWRY")
            XCTAssertEqual(entries[1].assetDefinitionId, "61CtjvNd9T3THAR65GsMVHr82Bjc")
            XCTAssertEqual(entries[1].amount, "2")
            XCTAssertEqual(details.role(for: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"), .sender)
            XCTAssertEqual(details.role(for: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"), .receiver)
            XCTAssertTrue(details.involvesAccount("sorauロ1PワKNラ7シコa2WクシメミQホbコトocニチヰJaアbg6sセgイヨPfX7WAWRY"))
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
                    "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "object":"10",
                                    "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
                    "authority":"sorauロ1Pタレソ1ヘカsFイAfセeB3スヱヱウcyハyケ1ツネヰヰ6ロヰEAテアウヨLPN4XM",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"AssetBatch",
                                "value":{
                                    "entries":[
                                        {
                                            "from":"sorauロ1Pタレソ1ヘカsFイAfセeB3スヱヱウcyハyケ1ツネヰヰ6ロヰEAテアウヨLPN4XM",
                                            "to":"sorauロ1PワKNラ7シコa2WクシメミQホbコトocニチヰJaアbg6sセgイヨPfX7WAWRY",
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
        XCTAssertEqual(page.transferRecords(matchingAccount: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D").count, 1)
        XCTAssertEqual(page.transferRecords(matchingAccount: "sorauロ1Pタレソ1ヘカsFイAfセeB3スヱヱウcyハyケ1ツネヰヰ6ロヰEAテアウヨLPN4XM").count, 1)
        XCTAssertEqual(page.transferRecords(assetDefinitionId: "61CtjvNd9T3THAR65GsMVHr82Bjc").count, 1)
        XCTAssertEqual(page.transferRecords(assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM").count, 1)
        XCTAssertEqual(page.transferRecords(matchingAccount: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                                            assetDefinitionId: "61CtjvNd9T3THAR65GsMVHr82Bjc").count, 0)
    }

    func testExplorerTransferSummariesDeriveDirection() throws {
        let json = """
        {
            "pagination": {"page":1,"per_page":10,"total_pages":1,"total_items":1},
            "items": [
                {
                    "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "object":"10",
                                    "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
        let summaries = page.transferSummaries(matchingAccount: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
        XCTAssertEqual(summaries.count, 1)
        let summary = summaries[0]
        XCTAssertEqual(summary.direction, .incoming)
        XCTAssertEqual(summary.senderAccountId, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
        XCTAssertEqual(summary.receiverAccountId, "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
        XCTAssertEqual(summary.assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        XCTAssertEqual(summary.amount, "10")
        XCTAssertTrue(summary.isIncoming)
        XCTAssertFalse(summary.isOutgoing)
        XCTAssertFalse(summary.isSelfTransfer)
        XCTAssertEqual(summary.transferIndex, 0)
        XCTAssertEqual(summary.id, "hash1|0|0")
        XCTAssertEqual(summary.direction(relativeTo: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"), .incoming)
        XCTAssertEqual(summary.counterpartyAccountId(relativeTo: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"), "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
        XCTAssertTrue(summary.isIncoming(relativeTo: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"))
        XCTAssertFalse(summary.isOutgoing(relativeTo: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"))
        XCTAssertFalse(summary.isSelfTransfer(relativeTo: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"))
        XCTAssertEqual(summary.signedAmount(relativeTo: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"), "+10")
        XCTAssertEqual(summary.signedAmount(relativeTo: "sorauロ1PgノタXヨnWアハキユネjキZヨrナxイWヤタリYヰヘxコタテロスfヨ2Gイ8P3LSM"), "10")
    }

    func testExplorerTransferSummariesDeriveSelfTransfer() throws {
        let json = """
        {
            "pagination": {"page":1,"per_page":10,"total_pages":1,"total_items":1},
            "items": [
                {
                    "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "object":"10",
                                    "destination":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"
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
        let summaries = page.transferSummaries(matchingAccount: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
        XCTAssertEqual(summaries.count, 1)
        let summary = summaries[0]
        XCTAssertEqual(summary.direction, .selfTransfer)
        XCTAssertTrue(summary.isSelfTransfer)
        XCTAssertFalse(summary.isIncoming)
        XCTAssertFalse(summary.isOutgoing)
        XCTAssertEqual(summary.direction(relativeTo: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"), .selfTransfer)
        XCTAssertEqual(summary.counterpartyAccountId(relativeTo: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"), "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
        XCTAssertNil(summary.counterpartyAccountId(relativeTo: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"))
        XCTAssertTrue(summary.isSelfTransfer(relativeTo: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"))
        XCTAssertFalse(summary.isIncoming(relativeTo: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"))
        XCTAssertFalse(summary.isOutgoing(relativeTo: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"))
        XCTAssertEqual(summary.signedAmount(relativeTo: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"), "10")
    }

    func testTransferSummarySignedAmountPreservesExistingSign() {
        let outgoing = ToriiExplorerTransferSummary(transactionHash: "hash1",
                                                    block: 1,
                                                    createdAt: "2025-01-01T00:00:00Z",
                                                    status: "Committed",
                                                    authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                                                    instructionIndex: 0,
                                                    senderAccountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                                                    receiverAccountId: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                                                    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                                    amount: "-10",
                                                    direction: .outgoing,
                                                    kind: "Transfer",
                                                    transferIndex: 0)
        XCTAssertEqual(outgoing.signedAmount(relativeTo: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"), "-10")

        let incoming = ToriiExplorerTransferSummary(transactionHash: "hash2",
                                                    block: 1,
                                                    createdAt: "2025-01-01T00:00:00Z",
                                                    status: "Committed",
                                                    authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                                                    instructionIndex: 0,
                                                    senderAccountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                                                    receiverAccountId: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                                                    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                                    amount: "+10",
                                                    direction: .incoming,
                                                    kind: "Transfer",
                                                    transferIndex: 0)
        XCTAssertEqual(incoming.signedAmount(relativeTo: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"), "+10")
    }

    func testExplorerTransferSummariesAssignBatchIndices() throws {
        let json = """
        {
            "pagination": {"page":1,"per_page":10,"total_pages":1,"total_items":1},
            "items": [
                {
                    "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"AssetBatch",
                                "value":{
                                    "entries":[
                                        {
                                            "from":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                                            "to":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                                            "asset_definition":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                            "amount":"5"
                                        },
                                        {
                                            "from":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                                            "to":"sorauロ1Pタレソ1ヘカsFイAfセeB3スヱヱウcyハyケ1ツネヰヰ6ロヰEAテアウヨLPN4XM",
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
                "authority":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
        let accountId = "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
        print("[TEST] Mint summary assetDefinitionId = \(summary.assetDefinitionId)")
        print("[TEST] Mint summary receiverAccountId = \(summary.receiverAccountId)")
        print("[TEST] Mint summary senderAccountId = \(summary.senderAccountId)")
    }

    func testCanonicalMintDestinationStaysCanonical() throws {
        let publicKey = Data(repeating: 0x44, count: 32)
        let address = try AccountAddress.fromAccount(publicKey: publicKey, algorithm: "ed25519")
        let accountId = try address.toI105(networkPrefix: 0x02F1)
        let literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#\(accountId)"
        XCTAssertEqual(try OfflineNorito.canonicalAssetIdLiteral(literal), literal)
        XCTAssertEqual(OfflineNorito.assetDefinitionIdFromLiteral(literal), "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
    }

    func testExplorerBurnInstructionParsedAsSummary() throws {
        let json = """
        {
            "pagination":{"page":1,"per_page":10,"total_pages":1,"total_items":1},
            "items":[{
                "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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

        let summaries = page.transferSummaries(relativeTo: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
        XCTAssertEqual(summaries.count, 1, "Burn should produce exactly 1 summary")

        let summary = summaries[0]
        XCTAssertEqual(summary.kind, "Burn")
        XCTAssertEqual(summary.amount, "25")
        XCTAssertEqual(summary.direction, .outgoing, "Burn should always be outgoing")
        print("[TEST] Burn summary assetDefinitionId = \(summary.assetDefinitionId)")
    }

    func testExplorerMixedKindsAllParsed() throws {
        // Page with Transfer + Mint + unknown kind — all parseable ones should be included
        let json = """
        {
            "pagination":{"page":1,"per_page":10,"total_pages":1,"total_items":3},
            "items":[
                {
                    "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
                    "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
                    "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
            XCTAssertEqual(query["asset"], assetIdFilter)
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
                        "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                        "created_at":"2025-01-01T00:00:00Z",
                        "kind":"Transfer",
                        "box":{
                            "scale":"0x00",
                            "json":{
                                "kind":"Transfer",
                                "payload":{
                                    "variant":"Asset",
                                    "value":{
                                        "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                        "object":"10",
                                        "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
                        "authority":"sorauロ1Pタレソ1ヘカsFイAfセeB3スヱヱウcyハyケ1ツネヰヰ6ロヰEAテアウヨLPN4XM",
                        "created_at":"2025-01-01T00:00:00Z",
                        "kind":"Transfer",
                        "box":{
                            "scale":"0x00",
                            "json":{
                                "kind":"Transfer",
                                "payload":{
                                    "variant":"AssetBatch",
                                    "value":{
                                        "entries":[
                                            {
                                                "from":"sorauロ1Pタレソ1ヘカsFイAfセeB3スヱヱウcyハyケ1ツネヰヰ6ロヰEAテアウヨLPN4XM",
                                                "to":"sorauロ1PワKNラ7シコa2WクシメミQホbコトocニチヰJaアbg6sセgイヨPfX7WAWRY",
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

        let transfers = try await makeClient().getExplorerTransfers(matchingAccount: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
            XCTAssertEqual(query["authority"], "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
            XCTAssertEqual(query["block"], "5")
            XCTAssertEqual(query["status"], "Committed")
            XCTAssertEqual(query["asset"], "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "pagination": {"page":2,"per_page":25,"total_pages":1,"total_items":1},
                "items": [
                    {
                        "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
                                                     authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
                "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
                "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                "created_at":"2025-01-01T00:00:00Z",
                "kind":"Transfer",
                "box":{
                    "scale":"0x00",
                    "json":{
                        "kind":"Transfer",
                        "payload":{
                            "variant":"Asset",
                            "value":{
                                "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                "object":"5",
                                "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
                "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
                                             domain: "commodities")
        let queryItems = try XCTUnwrap(params.queryItems())
        let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
        XCTAssertEqual(query["page"], "2")
        XCTAssertEqual(query["per_page"], "25")
        XCTAssertEqual(query["domain"], "commodities")
    }

    func testExplorerRwaRecordDecodesNullStatusAndMetadataDefaults() throws {
        let json = """
        {
            "id":"lot-001$commodities",
            "owned_by":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            "quantity":"42",
            "held_quantity":"2",
            "primary_reference":"vault://receipts/2",
            "status":null,
            "is_frozen":true,
            "metadata":null
        }
        """
        let record = try JSONDecoder().decode(ToriiExplorerRwaRecord.self, from: Data(json.utf8))
        XCTAssertEqual(record.id, "lot-001$commodities")
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
            XCTAssertEqual(request.url?.path, "/v1/explorer/rwas/lot-001$commodities")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
                "id":"lot-001$commodities",
                "owned_by":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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

        let detail = try await makeClient().getExplorerRwaDetail(rwaId: "lot-001$commodities")
        XCTAssertEqual(detail.id, "lot-001$commodities")
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
            XCTAssertEqual(decodedFilter?["id"], "lot-001$commodities")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"items":[{"id":"lot-001$commodities"}],"total":1}
            """.data(using: .utf8)!
            return (response, body)
        }

        let options = ToriiListOptions(filter: .json(.object(["id": .string("lot-001$commodities")])),
                                       sort: .fields(["id"]),
                                       limit: 25,
                                       offset: 10)
        let page = try await makeClient().listRwas(options: options)
        XCTAssertEqual(page.total, 1)
        XCTAssertEqual(page.items.first?.id, "lot-001$commodities")
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
            {"items":[{"id":"lot-002$commodities"}],"total":1}
            """.data(using: .utf8)!
            return (response, payload)
        }

        let envelope = ToriiQueryEnvelope(
            filter: .object(["id": .object(["eq": .string("lot-002$commodities")])]),
            sort: [ToriiQuerySortKey(key: "id", order: .asc)],
            pagination: ToriiQueryPagination(limit: 10, offset: 5),
            fetchSize: 20
        )
        let page = try await makeClient().queryRwas(envelope)
        XCTAssertEqual(page.total, 1)
        XCTAssertEqual(page.items.first?.id, "lot-002$commodities")
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
                {"items":[{"id":"lot-001$commodities"},{"id":"lot-002$commodities"}],"total":4}
                """.data(using: .utf8)!
            case "2":
                body = """
                {"items":[{"id":"lot-003$commodities"}],"total":4}
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
        XCTAssertEqual(collected, ["lot-001$commodities", "lot-002$commodities", "lot-003$commodities"])
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
                    "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "object":"5",
                                    "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
                    "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at":"2025-01-01T00:00:01Z",
                    "kind":"Transfer",
                    "box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "object":"7",
                                    "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
            XCTAssertEqual(query["asset"], assetIdFilter)
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
                        "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                        "created_at":"2025-01-01T00:00:00Z",
                        "kind":"Transfer",
                        "box":{
                            "scale":"0x00",
                            "json":{
                                "kind":"Transfer",
                                "payload":{
                                    "variant":"Asset",
                                    "value":{
                                        "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                        "object":"5",
                                        "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
                                                                 matchingAccount: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D") { result in
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
                    "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "object":"5",
                                    "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
                    "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at":"2025-01-01T00:00:01Z",
                    "kind":"Transfer",
                    "box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"61CtjvNd9T3THAR65GsMVHr82Bjc",
                                    "object":"7",
                                    "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
            XCTAssertEqual(query["asset"], "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
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
                "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                "created_at":"2025-01-01T00:00:00Z",
                "kind":"Transfer",
                "box":{
                    "scale":"0x00",
                    "json":{
                        "kind":"Transfer",
                        "payload":{
                            "variant":"Asset",
                            "value":{
                                "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                "object":"5",
                                "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
            XCTAssertEqual(query["asset"], assetIdFilter)
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
                        "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                        "created_at":"2025-01-01T00:00:00Z",
                        "kind":"Transfer",
                        "box":{
                            "scale":"0x00",
                            "json":{
                                "kind":"Transfer",
                                "payload":{
                                    "variant":"Asset",
                                    "value":{
                                        "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                        "object":"10",
                                        "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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

        let summaries = try await makeClient().getExplorerTransferSummaries(matchingAccount: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
            XCTAssertEqual(query["asset"], "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
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

        let summaries = try await makeClient().getAccountTransferHistory(accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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

        _ = makeClient().getAccountTransferHistory(accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB") { result in
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

        let summaries = try await makeClient().getTransactionHistory(accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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

        _ = makeClient().getTransactionHistory(accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB") { result in
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
                            "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                            "created_at":"2025-01-01T00:00:00Z",
                            "kind":"Transfer",
                            "box":{
                                "scale":"0x00",
                                "json":{
                                    "kind":"Transfer",
                                    "payload":{
                                        "variant":"Asset",
                                        "value":{
                                            "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                            "object":"10",
                                            "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
                            "authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                            "created_at":"2025-01-01T00:00:01Z",
                            "kind":"Transfer",
                            "box":{
                                "scale":"0x00",
                                "json":{
                                    "kind":"Transfer",
                                    "payload":{
                                        "variant":"Asset",
                                        "value":{
                                            "source":"61CtjvNd9T3THAR65GsMVHr82Bjc",
                                            "object":"5",
                                            "destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
        for try await summary in makeClient().iterateAccountTransferHistory(accountId: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
                    {"id":"wonderland","owned_by":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","metadata":{"theme":"demo"}}
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
        XCTAssertEqual(record.ownedBy, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
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
            XCTAssertEqual(query["provider"], "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
            XCTAssertEqual(query["limit"], "10")
            XCTAssertEqual(query["offset"], "5")
            let payload: [String: Any] = [
                "items": [
                    [
                        "plan_id": "plan#subs",
                        "plan": [
                            "provider": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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

        let params = ToriiSubscriptionPlanListParams(provider: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB", limit: 10, offset: 5)
        let response = try await makeClient().listSubscriptionPlans(params: params)
        XCTAssertEqual(response.total, 1)
        let item = try XCTUnwrap(response.items.first)
        XCTAssertEqual(item.planId, "plan#subs")
        if case let .string(provider)? = item.plan["provider"] {
            XCTAssertEqual(provider, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
        } else {
            XCTFail("missing plan provider")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testCreateSubscriptionPlanRejectsRemovedServerSideSigningFlow() async {
        let plan: ToriiSubscriptionPlan = [
            "provider": .string("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"),
            "billing": .object(["kind": .string("monthly")]),
            "pricing": .object(["kind": .string("fixed"), "amount": .string("120")])
        ]
        let requestBody = ToriiSubscriptionPlanCreateRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
            XCTAssertEqual(query["owned_by"], "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
            XCTAssertEqual(query["provider"], "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
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
                        "plan": ["provider": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"]
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

        let params = ToriiSubscriptionListParams(ownedBy: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                                                 provider: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
        let requestBody = ToriiSubscriptionCreateRequest(authority: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
                "plan": ["provider": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"]
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
            XCTAssertEqual(provider, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
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
        let action = ToriiSubscriptionActionRequest(authority: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
        let chargeAction = ToriiSubscriptionActionRequest(authority: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                                                          chargeAtMs: 1_704_067_200_000)
        let cancelAction = ToriiSubscriptionActionRequest(authority: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                                                          cancelMode: .periodEnd)
        let usage = ToriiSubscriptionUsageRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
                  "account_id":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
    func testGetUaidBindingsReturnsDataspaces() async throws {
        let uaidHex = "abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd"
        let payload = """
        {
          "uaid":"uaid:\(uaidHex)",
          "dataspaces":[
            {"dataspace_id":0,"dataspace_alias":"universal","accounts":["sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"]},
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
        XCTAssertEqual(response.dataspaces.first?.accounts.first, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
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
              "accounts":["sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"],
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
        XCTAssertEqual(response.manifests.first?.accounts.first, "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
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
        {"format":"pipeline.recovery.v1","height":42,"dag":{"fingerprint":"abcdef","key_count":1},"txs":[{"hash":"0x01","reads":["account/sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"],"writes":["asset/62Fk4FPcMuLvW5QjDGNF2a4jAmjM"]}]}
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
        {"abi_version":1}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/node/capabilities")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let capabilities = try await makeClient().getNodeCapabilities()
        XCTAssertEqual(capabilities.abiVersion, 1)
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
                "proposer": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
        XCTAssertEqual(item.record.proposer, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
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
                "proposer": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
            authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
            authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
            authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
            authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
            authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","hash":"hash1","block":100,"created_at":"2025-01-01T00:00:00Z","executable":"Instructions","status":"Committed"}

data: {"authority":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D","hash":"hash2","block":101,"created_at":"2025-01-02T00:00:00Z","executable":"Instructions","status":"Rejected"}

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
        XCTAssertEqual(first?.authority, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
        XCTAssertEqual(first?.hash, "hash1")
        XCTAssertEqual(first?.block, 100)
        XCTAssertEqual(first?.status, "Committed")

        let second = try await iterator.next()
        XCTAssertEqual(second?.authority, "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
        XCTAssertEqual(second?.hash, "hash2")
        XCTAssertEqual(second?.block, 101)
        XCTAssertEqual(second?.status, "Rejected")

        let third = try await iterator.next()
        XCTAssertNil(third)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamExplorerInstructionsAsync() async throws {
        let ssePayload = """
data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"5","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

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
        XCTAssertEqual(first?.authority, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
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
data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"5","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc","object":"6","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":2}

data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Mint","box":{"scale":"0x01","json":{"kind":"Mint","payload":{"variant":"Asset","value":{"asset":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","quantity":"1"}}}},"transaction_hash":"hash2","transaction_status":"Committed","block":10,"index":1}

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

        let stream = makeClient().streamExplorerTransfers(matchingAccount: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                                                          assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        var iterator = stream.makeAsyncIterator()

        let first = try await iterator.next()
        XCTAssertEqual(first?.instruction.transactionHash, "hash1")
        switch first?.details {
        case .asset(let asset):
            XCTAssertNil(asset.senderAccountId)
            XCTAssertEqual(asset.destinationAccountId, "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
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
data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"5","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc","object":"6","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":1}

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

        let stream = makeClient().streamExplorerTransferSummaries(matchingAccount: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                                                                  assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        var iterator = stream.makeAsyncIterator()

        let summary = try await iterator.next()
        XCTAssertEqual(summary?.transactionHash, "hash1")
        XCTAssertEqual(summary?.senderAccountId, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
        XCTAssertEqual(summary?.receiverAccountId, "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
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
                    "authority": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "object": "5",
                                    "destination": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
                    "authority": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source": "61CtjvNd9T3THAR65GsMVHr82Bjc",
                                    "object": "6",
                                    "destination": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"5","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"7","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash2","transaction_status":"Committed","block":11,"index":0}

data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc","object":"8","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash4","transaction_status":"Committed","block":11,"index":1}

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
                XCTAssertEqual(query["asset"], "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
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

        let stream = makeClient().streamAccountTransferHistory(accountId: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
                    "authority": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "AssetBatch",
                                "value": {
                                    "entries": [
                                        {
                                            "from": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                                            "to": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
                                            "asset_definition": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                            "amount": "5"
                                        },
                                        {
                                            "from": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                                            "to": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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

        let stream = makeClient().streamAccountTransferHistory(accountId: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
            // URL.path always returns decoded path. Check absoluteString to verify encoding.
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/accounts/sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB/assets"))
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
        client.assetsPublisher(accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB", limit: 2, scheduler: nil)
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
data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","hash":"hash1","block":100,"created_at":"2025-01-01T00:00:00Z","executable":"Instructions","status":"Committed"}

data: {"authority":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D","hash":"hash2","block":101,"created_at":"2025-01-02T00:00:00Z","executable":"Instructions","status":"Rejected"}

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
data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"5","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

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
data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"5","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc","object":"6","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":1}

data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Mint","box":{"scale":"0x01","json":{"kind":"Mint","payload":{"variant":"Asset","value":{"asset":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","quantity":"1"}}}},"transaction_hash":"hash2","transaction_status":"Committed","block":10,"index":1}

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
        client.explorerTransfersPublisher(matchingAccount: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"5","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc","object":"6","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":1}

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
                                                   matchingAccount: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
                    "authority": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "object": "5",
                                    "destination": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
                    "authority": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source": "61CtjvNd9T3THAR65GsMVHr82Bjc",
                                    "object": "6",
                                    "destination": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"5","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"7","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash2","transaction_status":"Committed","block":11,"index":0}

data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc","object":"8","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"hash4","transaction_status":"Committed","block":11,"index":1}

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
                XCTAssertEqual(query["asset"], "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
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
        client.accountTransferHistoryPublisher(accountId: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
                    "authority": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "object": "5",
                                    "destination": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
                    "authority": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source": "61CtjvNd9T3THAR65GsMVHr82Bjc",
                                    "object": "6",
                                    "destination": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"7","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"deadbeef","transaction_status":"Committed","block":11,"index":0}

data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc","object":"8","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"deadbeef","transaction_status":"Committed","block":11,"index":2}

data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:02Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"9","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"otherhash","transaction_status":"Committed","block":11,"index":1}

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
                                                                     matchingAccount: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
                    "authority": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "object": "5",
                                    "destination": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
                    "authority": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source": "61CtjvNd9T3THAR65GsMVHr82Bjc",
                                    "object": "6",
                                    "destination": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"7","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"deadbeef","transaction_status":"Committed","block":11,"index":0}

data: {"authority":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc","object":"8","destination":"sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"}}}},"transaction_hash":"deadbeef","transaction_status":"Committed","block":11,"index":2}

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
                                                     matchingAccount: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
                    "validator_ids": ["sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB", "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"],
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
                "validator_set": ["sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB", "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"],
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
            XCTAssertEqual(json["authority"] as? String, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
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
        let requestBody = ToriiRegisterContractCodeRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
        let requestBody = ToriiRegisterContractCodeRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
        let req = ToriiDeployContractRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                                             codeB64: "AQ==")
        await XCTAssertThrowsErrorAsync(try await makeClient().deployContract(req)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(reason.contains("/v1/contracts/deploy"))
            XCTAssertTrue(reason.contains("locally signed transaction"))
        }
    }

    func testDeployContractRejectsInvalidBase64() {
        let request = ToriiDeployContractRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                                                 codeB64: "%%%")
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testDeployContractRejectsUnsupportedFields() {
        let request = ToriiDeployContractRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
                                                 codeB64: "AQ==",
                                                 codeHash: String(repeating: "a", count: 64))
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testDeployContractInstanceRejectsRemovedServerSideSigningFlow() async {
        let manifest = ToriiContractManifest(compilerFingerprint: "kotodama-0.8")
        let req = ToriiDeployContractInstanceRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
        let req = ToriiActivateContractInstanceRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
            XCTAssertEqual(json["authority"] as? String, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
            XCTAssertNil(json["private_key"])
            XCTAssertEqual(json["public_key_hex"] as? String, String(repeating: "1", count: 64))
            XCTAssertEqual(json["signature_b64"] as? String, "AQ==")
            XCTAssertEqual(json["namespace"] as? String, "apps")
            XCTAssertEqual(json["contract_id"] as? String, "mint")
            XCTAssertEqual(json["entrypoint"] as? String, "create")
            XCTAssertEqual(json["gas_limit"] as? Int, 7)
            XCTAssertEqual(json["gas_asset_id"] as? String, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            XCTAssertEqual(json["fee_sponsor"] as? String, "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"submitted":true,"namespace":"apps","contract_id":"mint","code_hash_hex":"\(codeHash)","abi_hash_hex":"\(abiHash)","creation_time_ms":321,"tx_hash_hex":"\(txHash)","signed_transaction_b64":"AQ==","signing_message_b64":"Ag==","entrypoint":"create"}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiContractCallRequest(
            authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            publicKeyHex: String(repeating: "1", count: 64),
            signatureB64: "AQ==",
            namespace: "apps",
            contractId: "mint",
            entrypoint: "create",
            payload: .object(["amount": .string("10")]),
            creationTimeMs: 321,
            gasAssetId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
            feeSponsor: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
            gasLimit: 7
        )
        makeClient().callContract(request) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
                XCTAssertTrue(response.submitted)
                XCTAssertEqual(response.namespace, "apps")
                XCTAssertEqual(response.contractId, "mint")
                XCTAssertEqual(response.codeHashHex, codeHash)
                XCTAssertEqual(response.abiHashHex, abiHash)
                XCTAssertEqual(response.creationTimeMs, 321)
                XCTAssertEqual(response.txHashHex, txHash)
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
            authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            namespace: "apps",
            contractId: "mint",
            gasLimit: 0
        )
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
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
            XCTAssertEqual(json["multisig_account_alias"] as? String, "cbdc@hbl")
            XCTAssertNil(json["private_key"])
            XCTAssertEqual(json["signer_account_id"] as? String, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
            XCTAssertEqual(json["namespace"] as? String, "apps")
            XCTAssertEqual(json["contract_id"] as? String, "mint")
            XCTAssertEqual(json["entrypoint"] as? String, "execute")
            XCTAssertEqual(json["gas_limit"] as? Int, 5)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"resolved_multisig_account_id":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","submitted":false,"proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","creation_time_ms":123,"signing_message_b64":"AQ=="}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigContractCallProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@hbl"),
            signerAccountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            namespace: "apps",
            contractId: "mint",
            entrypoint: "execute",
            payload: .object(["amount": .string("10")]),
            gasAssetId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
            feeSponsor: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
            gasLimit: 5
        )
        makeClient().proposeMultisigContractCall(request) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
                XCTAssertEqual(response.resolvedMultisigAccountId, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
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
            XCTAssertEqual(json["multisig_account_id"] as? String, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
            XCTAssertEqual(json["signer_account_id"] as? String, "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D")
            XCTAssertEqual(json["proposal_id"] as? String, proposalId)
            XCTAssertEqual(json["signature_b64"] as? String, "AQ==")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"resolved_multisig_account_id":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","submitted":true,"proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","executed_tx_hash_hex":"\(txHash)"}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigContractCallApproveRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"),
            signerAccountId: "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
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
            XCTAssertEqual(json["multisig_account_alias"] as? String, "cbdc@ubl")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"resolved_multisig_account_id":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","spec":{"quorum":2,"transaction_ttl_ms":60000}}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigSpecRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@ubl")
        )
        makeClient().getMultisigSpec(request) { result in
            switch result {
            case .success(let response):
                XCTAssertEqual(response.resolvedMultisigAccountId, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
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
            XCTAssertEqual(json["multisig_account_alias"] as? String, "cbdc@hbl.universal")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"resolved_multisig_account_id":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","spec":{"quorum":2,"transaction_ttl_ms":60000}}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigSpecRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@hbl.universal")
        )
        makeClient().getMultisigSpec(request) { result in
            switch result {
            case .success(let response):
                XCTAssertEqual(response.resolvedMultisigAccountId, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testGetMultisigSpecRejectsUnsupportedAliasShape() {
        let request = ToriiMultisigSpecRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@hbl.universal.extra")
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
        let approverId = "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/multisig/proposals/list")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"resolved_multisig_account_id":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","proposals":[{"proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","proposal":{"approvals":["\(approverId)"]}}]}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposalsListRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@hbl")
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
        let approverOne = "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"
        let approverTwo = "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
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
            {"resolved_multisig_account_id":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","proposal":{"approvals":["\(approverOne)","\(approverTwo)"]}}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposalGetRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@hbl"),
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
            multisigAccountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            multisigAccountAlias: "cbdc@hbl"
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
            case "/transaction":
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-norito")
                XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/x-norito, application/json")
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
            XCTAssertEqual(body["authority"] as? String, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
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

        let request = ToriiGovernanceZkBallotRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
        let request = ToriiGovernanceZkBallotRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
        let request = ToriiGovernanceZkBallotRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
        let request = ToriiGovernanceZkBallotRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
        let request = ToriiGovernanceZkBallotRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
        let request = ToriiGovernanceZkBallotRequest(authority: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
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
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/gov/proposals/\(proposalId)")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let payload = """
            {"found":true,"proposal":{"proposer":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","kind":{"DeployContract":{"namespace":"apps","contract_id":"demo","code_hash_hex":"\(codeHash)","abi_hash_hex":"\(abiHash)","abi_version":"1"}},"created_height":42,"status":"Approved"}}
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
    func testGetTransactionStatusHttpErrorFallsBackToPlainTextBody() async throws {
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
    func testGetTransactionStatusHttpErrorFallsBackToCompactJsonBody() async throws {
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
