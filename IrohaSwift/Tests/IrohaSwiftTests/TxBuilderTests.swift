import XCTest
@testable import IrohaSwift

private final class StubPipelineClient: ToriiTransactionSubmitting {
    var submitted: [Data] = []
    var submittedModes: [PipelineEndpointMode] = []
    var observedIdempotencyKeys: [String?] = []
    var result: Swift.Result<ToriiSubmitTransactionResponse?, Error> = .success(makeSubmitReceipt())
    var queuedResults: [Swift.Result<ToriiSubmitTransactionResponse?, Error>] = []

    func submitTransaction(data: Data,
                           mode: PipelineEndpointMode,
                           idempotencyKey: String?) async throws -> ToriiSubmitTransactionResponse? {
        submitted.append(data)
        submittedModes.append(mode)
        observedIdempotencyKeys.append(idempotencyKey)
        let nextResult = queuedResults.isEmpty ? result : queuedResults.removeFirst()
        switch nextResult {
        case .success(let response):
            return response
        case .failure(let error):
            throw error
        }
    }
}

private func makeSubmitReceipt() -> ToriiSubmitTransactionResponse {
    ToriiSubmitTransactionResponse(
        payload: .init(txHash: "abc", submittedAtMs: 1, submittedAtHeight: 2, signer: "signer"),
        signature: "deadbeef"
    )
}

private struct StubTransportError: Error {}

private final class PipelineURLProtocol: URLProtocol {
    private static let lock = NSLock()
    private static var statusBodies: [Data] = []
    private static var submitStatusCode: Int = 202
    private static var submitBody: Data = PipelineURLProtocol.defaultSubmitBody
    private static var submitResponses: [(Int, Data)] = []
    private static var observedPaths: [String] = []
    private static var observedIdempotencyKeys: [String?] = []

    override class func canInit(with request: URLRequest) -> Bool { true }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let url = request.url else { return }
        PipelineURLProtocol.recordRequest(request)
        if PipelineURLProtocol.isSubmitPath(url.path) {
            let (code, body) = PipelineURLProtocol.nextSubmitResponse()
            sendResponse(code: code, body: body)
        } else if PipelineURLProtocol.isStatusPath(url.path) {
            let body = PipelineURLProtocol.dequeueStatusBody()
            sendResponse(code: 200, body: body)
        } else if PipelineURLProtocol.isNodeCapabilitiesPath(url.path) {
            sendResponse(code: 200, body: PipelineURLProtocol.nodeCapabilitiesBody)
        } else {
            sendResponse(code: 404, body: Data())
        }
    }

    override func stopLoading() { }

    private func sendResponse(code: Int, body: Data) {
        guard let client = client, let url = request.url,
              let response = HTTPURLResponse(url: url,
                                              statusCode: code,
                                              httpVersion: "HTTP/1.1",
                                              headerFields: ["Content-Type": "application/json"]) else {
            return
        }
        client.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client.urlProtocol(self, didLoad: body)
        client.urlProtocolDidFinishLoading(self)
    }

    static func configure(statuses: [String]) {
        lock.lock()
        statusBodies = statuses.map { makeStatusBody(kind: $0) }
        lock.unlock()
    }

    static func configureSubmissions(responses: [(Int, Data?)] = []) {
        lock.lock()
        submitResponses = responses.map { ($0.0, $0.1 ?? defaultSubmitBody) }
        lock.unlock()
    }

    static func reset() {
        lock.lock()
        statusBodies = []
        submitStatusCode = 202
        submitBody = defaultSubmitBody
        submitResponses = []
        observedPaths = []
        observedIdempotencyKeys = []
        lock.unlock()
    }

    static func drainObservedPaths() -> [String] {
        lock.lock()
        let paths = observedPaths
        observedPaths = []
        lock.unlock()
        return paths
    }

    static func drainObservedIdempotencyKeys() -> [String?] {
        lock.lock()
        let keys = observedIdempotencyKeys
        observedIdempotencyKeys = []
        lock.unlock()
        return keys
    }

    private static func dequeueStatusBody() -> Data {
        lock.lock()
        defer { lock.unlock() }
        if statusBodies.isEmpty {
            return makeStatusBody(kind: "Pending")
        }
        return statusBodies.removeFirst()
    }

    private static func nextSubmitResponse() -> (Int, Data) {
        lock.lock()
        defer { lock.unlock() }
        if !submitResponses.isEmpty {
            return submitResponses.removeFirst()
        }
        return (submitStatusCode, submitBody)
    }

    private static func makeStatusBody(kind: String) -> Data {
        let payload: [String: Any] = [
            "kind": "PipelineTransactionStatus",
            "content": [
                "hash": "abc",
                "status": [
                    "kind": kind,
                    "content": NSNull()
                ]
            ]
        ]
        return (try? JSONSerialization.data(withJSONObject: payload)) ?? Data()
    }

    private static var defaultSubmitBody: Data {
        let body: [String: Any] = [
            "payload": [
                "tx_hash": "abc",
                "submitted_at_ms": 1,
                "submitted_at_height": 2,
                "signer": "signer"
            ],
            "signature": "deadbeef"
        ]
        return (try? JSONSerialization.data(withJSONObject: body)) ?? Data()
    }

    private static func recordRequest(_ request: URLRequest) {
        let path = request.url?.path ?? ""
        let idempotencyKey = request.value(forHTTPHeaderField: "Idempotency-Key")
        lock.lock()
        observedPaths.append(path)
        if isSubmitPath(path) {
            observedIdempotencyKeys.append(idempotencyKey)
        }
        lock.unlock()
    }

    private static func isSubmitPath(_ path: String) -> Bool {
        path.hasSuffix("/transaction")
            || path.hasSuffix("/v1/pipeline/transactions")
            || path.hasSuffix("/v1/transactions")
    }

    private static func isStatusPath(_ path: String) -> Bool {
        path.hasSuffix("/v1/pipeline/transactions/status") || path.hasSuffix("/v1/transactions/status")
    }

    private static func isNodeCapabilitiesPath(_ path: String) -> Bool {
        path.hasSuffix("/v1/node/capabilities")
    }

    private static var nodeCapabilitiesBody: Data {
        let body: [String: Any] = [
            "supported_abi_versions": [1],
            "default_compile_target": 0,
            "data_model_version": ToriiNodeCapabilities.expectedDataModelVersion
        ]
        return (try? JSONSerialization.data(withJSONObject: body)) ?? Data()
    }
}

private func expectedTail(forTTL ttl: UInt64?) -> Data {
    if let ttl {
        var tail = Data([1])
        var value = ttl.littleEndian
        withUnsafeBytes(of: &value) { tail.append(contentsOf: $0) }
        tail.append(contentsOf: [0, 0])
        return tail
    } else {
        return Data([0, 0, 0])
    }
}

final class TxBuilderTests: XCTestCase {
    private static let fixturePrivateKeyHex = "000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F"
    private static let fixtureChainId = "00000000-0000-0000-0000-000000000000"
    private static let fixtureDomain = "wonderland"
    private static let fixtureAssetDefinition = "rose#wonderland"
    private static let fixtureCreationTimeMs: UInt64 = 1_700_000_000_000
    private enum FixtureError: Error { case invalidKey }

    private func makeFixtureKeypair() throws -> Keypair {
        guard let keyData = Data(hexString: Self.fixturePrivateKeyHex) else {
            XCTFail("Invalid fixture private key hex")
            throw FixtureError.invalidKey
        }
        return try Keypair(privateKeyBytes: keyData)
    }

    private func makeShieldRequest(authority: String,
                                   amount: String = "42",
                                   ttlMs: UInt64? = 45) throws -> ShieldRequest {
        let noteCommitment = Data(repeating: 0xAB, count: 32)
        let payload = try ConfidentialEncryptedPayload(ephemeralPublicKey: Data(repeating: 0x01, count: 32),
                                                       nonce: Data(repeating: 0x02, count: 24),
                                                       ciphertext: Data([0xDE, 0xAD, 0xBE, 0xEF]))
        return try ShieldRequest(chainId: Self.fixtureChainId,
                                 authority: authority,
                                 assetDefinitionId: Self.fixtureAssetDefinition,
                                 fromAccountId: authority,
                                 amount: amount,
                                 noteCommitment: noteCommitment,
                                 payload: payload,
                                 ttlMs: ttlMs)
    }

    private func makeProofAttachment() throws -> ProofAttachment {
        let proof = Data(repeating: 0xEE, count: 48)
        let reference = ProofAttachment.VerifyingKeyReference(backend: "halo2/ipa", name: "vk_unshield")
        return try ProofAttachment(backend: "halo2/ipa",
                                   proof: proof,
                                   verifyingKey: .reference(reference))
    }

    private func makeUnshieldRequest(authority: String,
                                     amount: String = "7",
                                     ttlMs: UInt64? = 30) throws -> UnshieldRequest {
        let proof = try makeProofAttachment()
        let inputs = [
            Data(repeating: 0x10, count: 32),
            Data(repeating: 0x20, count: 32),
        ]
        let rootHint = Data(repeating: 0xAA, count: 32)
        return try UnshieldRequest(chainId: Self.fixtureChainId,
                                   authority: authority,
                                   assetDefinitionId: Self.fixtureAssetDefinition,
                                   toAccountId: authority,
                                   publicAmount: amount,
                                   inputs: inputs,
                                   proof: proof,
                                   rootHint: rootHint,
                                   ttlMs: ttlMs)
    }

    private func hexEncoded(_ data: Data) -> String {
        data.map { String(format: "%02x", $0) }.joined()
    }

    private func requireEd25519Encoder() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
                      "Native transaction encoder unavailable")
    }

    private func normalizeNativeSignedTransaction(
        _ native: NativeSignedTransaction
    ) -> (signed: Data, norito: Data) {
        if let framed = noritoDecodeFrame(native.signedBytes) {
            return (signed: framed.payload, norito: native.signedBytes)
        }
        let norito = noritoEncode(typeName: "iroha_data_model::transaction::signed::SignedTransaction",
                                  payload: native.signedBytes,
                                  flags: 0x04)
        return (signed: native.signedBytes, norito: norito)
    }

    private func makeRegisterZkAssetRequest(authority: String,
                                            ttlMs: UInt64? = 30) throws -> RegisterZkAssetRequest {
        let transferVk = try VerifyingKeyIdReference(backend: "halo2/ipa", name: "vk_transfer")
        let unshieldVk = try VerifyingKeyIdReference(backend: "halo2/ipa", name: "vk_unshield")
        return RegisterZkAssetRequest(chainId: Self.fixtureChainId,
                                      authority: authority,
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      mode: .hybrid,
                                      allowShield: true,
                                      allowUnshield: true,
                                      transferVerifyingKey: transferVk,
                                      unshieldVerifyingKey: unshieldVk,
                                      shieldVerifyingKey: nil,
                                      ttlMs: ttlMs)
    }

    func testBuildSignedTransferProducesEnvelope() throws {
        try requireEd25519Encoder()
        let keypair = try Keypair.generate()
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!)
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: AccountId.make(publicKey: keypair.publicKey, domain: "wonderland"),
                                       assetDefinitionId: "rose#wonderland",
                                       quantity: "1",
                                       destination: AccountId.make(publicKey: keypair.publicKey, domain: "wonderland"),
                                       description: nil,
                                       ttlMs: 90)
        let envelope = try sdk.buildSignedTransfer(transfer: transfer, keypair: keypair)
        XCTAssertEqual(String(data: envelope.norito.prefix(4), encoding: .ascii), "NRT0")
        XCTAssertNil(envelope.payload, "Swift encoder does not embed payload bytes yet")
        XCTAssertEqual(envelope.transactionHash.count, 32)
        if let payload = envelope.payload {
            let tail = expectedTail(forTTL: 90)
            XCTAssertEqual(payload.suffix(tail.count), tail)
        }
    }

    func testCreationTimeProviderProducesDeterministicHash() throws {
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let transfer = TransferRequest(chainId: Self.fixtureChainId,
                                       authority: authority,
                                       assetDefinitionId: Self.fixtureAssetDefinition,
                                       quantity: "1",
                                       destination: authority,
                                       description: "deterministic",
                                       ttlMs: 30)

        let fixedClockSdk = IrohaSDK(baseURL: URL(string: "https://example.test")!,
                                     creationTimeProvider: { Self.fixtureCreationTimeMs })
        let first = try fixedClockSdk.buildSignedTransfer(transfer: transfer, keypair: keypair)
        let second = try fixedClockSdk.buildSignedTransfer(transfer: transfer, keypair: keypair)
        XCTAssertEqual(first.transactionHash, second.transactionHash)
        XCTAssertEqual(first.signedTransaction, second.signedTransaction)

        let shiftedClockSdk = IrohaSDK(baseURL: URL(string: "https://example.test")!,
                                       creationTimeProvider: { Self.fixtureCreationTimeMs + 1 })
        let shifted = try shiftedClockSdk.buildSignedTransfer(transfer: transfer, keypair: keypair)
        XCTAssertNotEqual(first.transactionHash, shifted.transactionHash)
    }

    func testDefaultCreationTimeAdvances() {
        let first = IrohaSDK.defaultCreationTimeMs()
        usleep(5_000)
        let second = IrohaSDK.defaultCreationTimeMs()
        XCTAssertGreaterThanOrEqual(second, first + 1)
    }

    func testGetAssetsFailsWhenRestClientUnavailable() {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let expectation = expectation(description: "rest unavailable")
        sdk.getAssets(accountId: "alice@wonderland") { result in
            switch result {
            case .success:
                XCTFail("expected failure when REST client is missing")
            case .failure(let error):
                guard let sdkError = error as? IrohaSDKError else {
                    XCTFail("Unexpected error type: \(error)")
                    break
                }
                XCTAssertEqual(sdkError, .restClientUnavailable)
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerInstructionsFailsWhenRestClientUnavailable() {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let expectation = expectation(description: "rest unavailable")
        sdk.getExplorerInstructions { result in
            switch result {
            case .success:
                XCTFail("expected failure when REST client is missing")
            case .failure(let error):
                XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerInstructionsAsyncFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            _ = try await sdk.getExplorerInstructions()
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransfersFailsWhenRestClientUnavailable() {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let expectation = expectation(description: "rest unavailable")
        sdk.getExplorerTransfers { result in
            switch result {
            case .success:
                XCTFail("expected failure when REST client is missing")
            case .failure(let error):
                XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransfersAsyncFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            _ = try await sdk.getExplorerTransfers()
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransactionsFailsWhenRestClientUnavailable() {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let expectation = expectation(description: "rest unavailable")
        sdk.getExplorerTransactions { result in
            switch result {
            case .success:
                XCTFail("expected failure when REST client is missing")
            case .failure(let error):
                XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransactionsAsyncFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            _ = try await sdk.getExplorerTransactions()
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransactionDetailFailsWhenRestClientUnavailable() {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let expectation = expectation(description: "rest unavailable")
        sdk.getExplorerTransactionDetail(hashHex: "deadbeef") { result in
            switch result {
            case .success:
                XCTFail("expected failure when REST client is missing")
            case .failure(let error):
                XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransactionDetailAsyncFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            _ = try await sdk.getExplorerTransactionDetail(hashHex: "deadbeef")
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerInstructionDetailFailsWhenRestClientUnavailable() {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let expectation = expectation(description: "rest unavailable")
        sdk.getExplorerInstructionDetail(hashHex: "deadbeef", index: 0) { result in
            switch result {
            case .success:
                XCTFail("expected failure when REST client is missing")
            case .failure(let error):
                XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerInstructionDetailAsyncFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            _ = try await sdk.getExplorerInstructionDetail(hashHex: "deadbeef", index: 0)
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransferSummariesFailsWhenRestClientUnavailable() {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let expectation = expectation(description: "rest unavailable")
        sdk.getExplorerTransferSummaries { result in
            switch result {
            case .success:
                XCTFail("expected failure when REST client is missing")
            case .failure(let error):
                XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransferSummariesAsyncFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            _ = try await sdk.getExplorerTransferSummaries()
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAccountTransferHistoryFailsWhenRestClientUnavailable() {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let expectation = expectation(description: "rest unavailable")
        sdk.getAccountTransferHistory(accountId: "alice@wonderland") { result in
            switch result {
            case .success:
                XCTFail("expected failure when REST client is missing")
            case .failure(let error):
                XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAccountTransferHistoryAsyncFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            _ = try await sdk.getAccountTransferHistory(accountId: "alice@wonderland")
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testIterateAccountTransferHistoryFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            for try await _ in sdk.iterateAccountTransferHistory(accountId: "alice@wonderland") {
                XCTFail("expected no items when REST client is missing")
            }
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamExplorerTransactionsFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            for try await _ in sdk.streamExplorerTransactions() {
                XCTFail("expected no items when REST client is missing")
            }
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamExplorerInstructionsFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            for try await _ in sdk.streamExplorerInstructions() {
                XCTFail("expected no items when REST client is missing")
            }
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamExplorerTransfersFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            for try await _ in sdk.streamExplorerTransfers() {
                XCTFail("expected no items when REST client is missing")
            }
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamExplorerTransferSummariesFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            for try await _ in sdk.streamExplorerTransferSummaries() {
                XCTFail("expected no items when REST client is missing")
            }
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamAccountTransferHistoryFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            for try await _ in sdk.streamAccountTransferHistory(accountId: "alice@wonderland") {
                XCTFail("expected no items when REST client is missing")
            }
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    func testGetHealthFailsWhenRestClientUnavailable() {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let expectation = expectation(description: "rest unavailable")
        sdk.getHealth { result in
            switch result {
            case .success:
                XCTFail("expected failure when REST client is missing")
            case .failure(let error):
                XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testGetMetricsFailsWhenRestClientUnavailable() {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let expectation = expectation(description: "rest unavailable")
        sdk.getMetrics { result in
            switch result {
            case .success:
                XCTFail("expected failure when REST client is missing")
            case .failure(let error):
                XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testSubmitUsesInjectedPipelineClient() throws {
        try requireEd25519Encoder()
        let stub = StubPipelineClient()
        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: authority,
                                       assetDefinitionId: "rose#wonderland",
                                       quantity: "1",
                                       destination: authority,
                                       description: nil,
                                       ttlMs: nil)
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        sdk.pipelineSubmitOptions = PipelineSubmitOptions(maxRetries: 0)
        let envelope = try sdk.buildSignedTransfer(transfer: transfer, keypair: keypair)

        let expectation = expectation(description: "submit")
        sdk.submit(envelope: envelope) { error in
            XCTAssertNil(error)
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)

        XCTAssertEqual(stub.submitted.count, 1)
        XCTAssertEqual(stub.submitted.first, envelope.norito)
        XCTAssertEqual(stub.submittedModes, [.pipeline])
    }

    func testBuildRegisterZkAssetProducesEnvelope() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
                      "Native transaction encoder unavailable")
        let keypair = try makeFixtureKeypair()
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!)
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let request = try makeRegisterZkAssetRequest(authority: authority, ttlMs: 60)
        let envelope = try sdk.buildRegisterZkAsset(request: request, keypair: keypair)
        XCTAssertEqual(String(data: envelope.norito.prefix(4), encoding: .ascii), "NRT0")
        XCTAssertEqual(envelope.transactionHash.count, 32)
    }

    func testVerifyingKeyIdReferenceValidation() {
        XCTAssertThrowsError(try VerifyingKeyIdReference(backend: "", name: "vk")) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .emptyBackend)
        }
        XCTAssertThrowsError(try VerifyingKeyIdReference(backend: "halo2", name: "")) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .emptyName)
        }
        XCTAssertThrowsError(try VerifyingKeyIdReference(backend: "halo2:ipa", name: "vk")) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .invalidSeparator)
        }
    }

    func testSubmitPropagatesError() throws {
        try requireEd25519Encoder()
        enum StubError: Error { case failure }
        let stub = StubPipelineClient()
        stub.result = .failure(StubError.failure)
        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: authority,
                                       assetDefinitionId: "rose#wonderland",
                                       quantity: "1",
                                       destination: authority,
                                       description: nil,
                                       ttlMs: nil)
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        sdk.pipelineSubmitOptions = PipelineSubmitOptions(maxRetries: 0)
        let envelope = try sdk.buildSignedTransfer(transfer: transfer, keypair: keypair)

        let expectation = expectation(description: "submit error")
        sdk.submit(envelope: envelope) { error in
            XCTAssertNotNil(error)
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
        XCTAssertEqual(stub.submittedModes, [.pipeline])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitEnvelopeAsync() async throws {
        try requireEd25519Encoder()
        let stub = StubPipelineClient()
        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: authority,
                                       assetDefinitionId: "rose#wonderland",
                                       quantity: "1",
                                       destination: authority,
                                       description: nil)
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let envelope = try sdk.buildSignedTransfer(transfer: transfer, keypair: keypair)
        try await sdk.submit(envelope: envelope)
        XCTAssertEqual(stub.submitted.count, 1)
        XCTAssertEqual(stub.submittedModes, [.pipeline])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitEnvelopeAsyncPropagatesError() async throws {
        try requireEd25519Encoder()
        enum StubError: Error { case failure }
        let stub = StubPipelineClient()
        stub.result = .failure(StubError.failure)
        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: authority,
                                       assetDefinitionId: "rose#wonderland",
                                       quantity: "1",
                                       destination: authority,
                                       description: nil)
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let envelope = try sdk.buildSignedTransfer(transfer: transfer, keypair: keypair)
        do {
            try await sdk.submit(envelope: envelope)
            XCTFail("Expected failure")
        } catch StubError.failure {
            // expected
        } catch {
            XCTFail("Unexpected error: \(error)")
        }
        XCTAssertEqual(Set(stub.submittedModes), [.pipeline])
    }

    func testFallbackTransferMatchesNativeBridge() throws {
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: authority,
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: authority,
                                      description: nil,
                                      ttlMs: 90)

        let fallback = try SwiftTransactionEncoder.encodeTransfer(transfer: request,
                                                                  keypair: keypair,
                                                                  creationTimeMs: Self.fixtureCreationTimeMs)

        guard let native = try? NoritoNativeBridge.shared.encodeTransfer(chainId: request.chainId,
                                                                         authority: request.authority,
                                                                         creationTimeMs: Self.fixtureCreationTimeMs,
                                                                         ttlMs: request.ttlMs,
                                                                         assetDefinitionId: request.assetDefinitionId,
                                                                         quantity: request.quantity,
                                                                         destination: request.destination,
                                                                         privateKey: keypair.privateKeyBytes) else {
            XCTFail("Expected native bridge transfer encoding")
            return
        }

        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, fallback.signedTransaction)
        XCTAssertEqual(native.hash, fallback.transactionHash)
        XCTAssertEqual(normalized.norito, fallback.norito)
    }

    func testSigningKeyTransferMatchesKeypairEncoding() throws {
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }
        let keypair = try makeFixtureKeypair()
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: authority,
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "10",
                                      destination: authority,
                                      description: nil,
                                      ttlMs: 60)
        let withKeypair = try SwiftTransactionEncoder.encodeTransfer(transfer: request,
                                                                     keypair: keypair,
                                                                     creationTimeMs: Self.fixtureCreationTimeMs)
        let withSigningKey = try SwiftTransactionEncoder.encodeTransfer(transfer: request,
                                                                        signingKey: signingKey,
                                                                        creationTimeMs: Self.fixtureCreationTimeMs)
        XCTAssertEqual(withKeypair.signedTransaction, withSigningKey.signedTransaction)
        XCTAssertEqual(withKeypair.transactionHash, withSigningKey.transactionHash)
    }

    func testFallbackMintMatchesNativeBridge() throws {
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let request = MintRequest(chainId: Self.fixtureChainId,
                                  authority: authority,
                                  assetDefinitionId: Self.fixtureAssetDefinition,
                                  quantity: "3.14",
                                  destination: authority,
                                  ttlMs: 45)

        let fallback = try SwiftTransactionEncoder.encodeMint(request: request,
                                                              keypair: keypair,
                                                              creationTimeMs: Self.fixtureCreationTimeMs)

        guard let native = try? NoritoNativeBridge.shared.encodeMint(chainId: request.chainId,
                                                                     authority: request.authority,
                                                                     creationTimeMs: Self.fixtureCreationTimeMs,
                                                                     ttlMs: request.ttlMs,
                                                                     assetDefinitionId: request.assetDefinitionId,
                                                                     quantity: request.quantity,
                                                                     destination: request.destination,
                                                                     privateKey: keypair.privateKeyBytes) else {
            XCTFail("Expected native bridge mint encoding")
            return
        }

        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, fallback.signedTransaction)
        XCTAssertEqual(native.hash, fallback.transactionHash)
        XCTAssertEqual(normalized.norito, fallback.norito)
    }

    func testFallbackBurnMatchesNativeBridge() throws {
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let request = BurnRequest(chainId: Self.fixtureChainId,
                                  authority: authority,
                                  assetDefinitionId: Self.fixtureAssetDefinition,
                                  quantity: "2",
                                  destination: authority,
                                  ttlMs: 120)

        let fallback = try SwiftTransactionEncoder.encodeBurn(request: request,
                                                              keypair: keypair,
                                                              creationTimeMs: Self.fixtureCreationTimeMs)

        guard let native = try? NoritoNativeBridge.shared.encodeBurn(chainId: request.chainId,
                                                                     authority: request.authority,
                                                                     creationTimeMs: Self.fixtureCreationTimeMs,
                                                                     ttlMs: request.ttlMs,
                                                                     assetDefinitionId: request.assetDefinitionId,
                                                                     quantity: request.quantity,
                                                                     destination: request.destination,
                                                                     privateKey: keypair.privateKeyBytes) else {
            XCTFail("Expected native bridge burn encoding")
            return
        }

        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, fallback.signedTransaction)
        XCTAssertEqual(native.hash, fallback.transactionHash)
        XCTAssertEqual(normalized.norito, fallback.norito)
    }

    func testSetMetadataMatchesNativeBridge() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
                      "Native transaction encoder unavailable")
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let value = try NoritoJSON("wonderland")
        let request = SetMetadataRequest(chainId: Self.fixtureChainId,
                                         authority: authority,
                                         target: .account(authority),
                                         key: "display_name",
                                         value: value,
                                         ttlMs: 30)

        let fallback = try SwiftTransactionEncoder.encodeSetMetadata(request: request,
                                                                     keypair: keypair,
                                                                     creationTimeMs: Self.fixtureCreationTimeMs)

        guard let native = try? NoritoNativeBridge.shared.encodeSetKeyValue(
            chainId: request.chainId,
            authority: request.authority,
            creationTimeMs: Self.fixtureCreationTimeMs,
            ttlMs: request.ttlMs,
            targetKind: request.target.targetKind,
            objectId: request.target.objectId,
            key: request.key,
            valueJson: value.data,
            privateKey: keypair.privateKeyBytes
        ) else {
            XCTFail("Expected native bridge set metadata encoding")
            return
        }

        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, fallback.signedTransaction)
        XCTAssertEqual(native.hash, fallback.transactionHash)
        XCTAssertEqual(normalized.norito, fallback.norito)
    }

    func testGovernanceProposeDeployMatchesNativeBridge() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
                      "Native transaction encoder unavailable")
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let codeHash = Data(repeating: 0x11, count: 32)
        let abiHash = Data(repeating: 0x22, count: 32)
        let window = GovernanceWindow(lower: 4, upper: 8)
        let request = ProposeDeployContractRequest(chainId: Self.fixtureChainId,
                                                   authority: authority,
                                                   namespace: "contracts",
                                                   contractId: "demo",
                                                   codeHashHex: hexEncoded(codeHash),
                                                   abiHashHex: hexEncoded(abiHash),
                                                   abiVersion: "1.0.0",
                                                   window: window,
                                                   mode: .plain,
                                                   ttlMs: 20)

        let fallback = try SwiftTransactionEncoder.encodeProposeDeploy(request: request,
                                                                       keypair: keypair,
                                                                       creationTimeMs: Self.fixtureCreationTimeMs)

        guard let native = try? NoritoNativeBridge.shared.encodeGovernanceProposeDeploy(
            chainId: request.chainId,
            authority: request.authority,
            creationTimeMs: Self.fixtureCreationTimeMs,
            ttlMs: request.ttlMs,
            namespace: request.namespace,
            contractId: request.contractId,
            codeHashHex: request.codeHashHex,
            abiHashHex: request.abiHashHex,
            abiVersion: request.abiVersion,
            window: request.window.map { ($0.lower, $0.upper) },
            modeCode: request.mode?.rawValue,
            privateKey: keypair.privateKeyBytes
        ) else {
            XCTFail("Expected native bridge governance encoding")
            return
        }

        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, fallback.signedTransaction)
        XCTAssertEqual(native.hash, fallback.transactionHash)
        XCTAssertEqual(normalized.norito, fallback.norito)
    }

    func testPersistCouncilMatchesNativeBridge() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
                      "Native transaction encoder unavailable")
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let request = PersistCouncilRequest(chainId: Self.fixtureChainId,
                                            authority: authority,
                                            epoch: 7,
                                            members: [authority],
                                            candidatesCount: 1,
                                            derivedBy: .vrf,
                                            ttlMs: 15)

        let fallback = try SwiftTransactionEncoder.encodePersistCouncil(request: request,
                                                                        keypair: keypair,
                                                                        creationTimeMs: Self.fixtureCreationTimeMs)
        let membersJson = try NoritoJSON(request.members).data

        guard let native = try? NoritoNativeBridge.shared.encodeGovernancePersistCouncil(
            chainId: request.chainId,
            authority: request.authority,
            creationTimeMs: Self.fixtureCreationTimeMs,
            ttlMs: request.ttlMs,
            epoch: request.epoch,
            candidatesCount: request.candidatesCount,
            derivedBy: request.derivedBy.rawValue,
            membersJson: membersJson,
            privateKey: keypair.privateKeyBytes
        ) else {
            XCTFail("Expected native bridge persist council encoding")
            return
        }

        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, fallback.signedTransaction)
        XCTAssertEqual(native.hash, fallback.transactionHash)
        XCTAssertEqual(normalized.norito, fallback.norito)
    }

    func testNativeBridgeTransferWhenAvailable() throws {
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }

        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: authority,
                                       assetDefinitionId: "rose#wonderland",
                                       quantity: "1",
                                       destination: authority,
                                       description: nil,
                                       ttlMs: nil)

        guard let native = try? NoritoNativeBridge.shared.encodeTransfer(chainId: transfer.chainId,
                                                                         authority: transfer.authority,
                                                                         creationTimeMs: UInt64(Date().timeIntervalSince1970 * 1000),
                                                                         ttlMs: nil,
                                                                         assetDefinitionId: transfer.assetDefinitionId,
                                                                         quantity: transfer.quantity,
                                                                         destination: transfer.destination,
                                                                         privateKey: keypair.privateKeyBytes) else {
            XCTFail("Expected native bridge to produce transaction")
            return
        }

        XCTAssertEqual(native.hash.count, 32)
        XCTAssertFalse(native.signedBytes.isEmpty)
    }

    func testDecodeSignedTransactionJSONWhenAvailable() throws {
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }

        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: authority,
                                       assetDefinitionId: "rose#wonderland",
                                       quantity: "1",
                                       destination: authority,
                                       description: nil,
                                       ttlMs: nil)
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!)
        let envelope = try sdk.buildSignedTransfer(transfer: transfer, keypair: keypair)
        guard let json = sdk.decodeSignedTransaction(envelope: envelope) else {
            XCTFail("expected JSON from native bridge")
            return
        }
        XCTAssertTrue(json.contains("\"chain\""))
        XCTAssertTrue(json.contains("\"instructions\""))
    }

    func testMintNativeBridgeWhenAvailable() throws {
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }

        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
        let destination = authority

        guard let native = try? NoritoNativeBridge.shared.encodeMint(chainId: "00000000-0000-0000-0000-000000000000",
                                                                     authority: authority,
                                                                     creationTimeMs: UInt64(Date().timeIntervalSince1970 * 1000),
                                                                     ttlMs: nil,
                                                                     assetDefinitionId: "rose#wonderland",
                                                                     quantity: "42",
                                                                     destination: destination,
                                                                     privateKey: keypair.privateKeyBytes) else {
            XCTFail("Expected native bridge to encode mint")
            return
        }

        XCTAssertEqual(native.hash.count, 32)
        XCTAssertFalse(native.signedBytes.isEmpty)
    }

    func testSetMetadataNativeBridgeWhenAvailable() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
                      "Native transaction encoder unavailable")
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }

        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let request = try SetMetadataRequest(chainId: Self.fixtureChainId,
                                             authority: authority,
                                             target: .domain(Self.fixtureDomain),
                                             key: "label",
                                             value: .string("wonderland"),
                                             ttlMs: nil)
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        let envelope = try SwiftTransactionEncoder.encodeSetMetadata(request: request,
                                                                     signingKey: signingKey,
                                                                     creationTimeMs: Self.fixtureCreationTimeMs)
        XCTAssertEqual(envelope.transactionHash.count, 32)
        XCTAssertFalse(envelope.signedTransaction.isEmpty)
    }

    func testProposeDeployNativeBridgeWhenAvailable() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
                      "Native transaction encoder unavailable")
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }

        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let request = ProposeDeployContractRequest(chainId: Self.fixtureChainId,
                                                   authority: authority,
                                                   namespace: "apps",
                                                   contractId: "demo.contract",
                                                   codeHashHex: String(repeating: "aa", count: 32),
                                                   abiHashHex: String(repeating: "bb", count: 32),
                                                   abiVersion: "1",
                                                   window: GovernanceWindow(lower: 1, upper: 5),
                                                   mode: .zk,
                                                   ttlMs: nil)
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        let envelope = try SwiftTransactionEncoder.encodeProposeDeploy(request: request,
                                                                       signingKey: signingKey,
                                                                       creationTimeMs: Self.fixtureCreationTimeMs)
        XCTAssertEqual(envelope.transactionHash.count, 32)
        XCTAssertFalse(envelope.signedTransaction.isEmpty)
    }

    func testBuildMintWithoutBridgeThrows() throws {
        NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(false)
        defer { NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(nil) }

        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
        let destination = authority
        let request = MintRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                  authority: authority,
                                  assetDefinitionId: "rose#wonderland",
                                  quantity: "3.14",
                                  destination: destination,
                                  ttlMs: 45)
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!)
        XCTAssertThrowsError(try sdk.buildMint(mint: request, keypair: keypair)) { error in
            guard case SwiftTransactionEncoderError.nativeBridgeUnavailable = error else {
                XCTFail("Expected nativeBridgeUnavailable error")
                return
            }
        }
    }

    func testBuildSetMetadataWithoutBridgeThrows() throws {
        NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(false)
        defer { NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(nil) }

        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
        let request = try SetMetadataRequest(chainId: Self.fixtureChainId,
                                             authority: authority,
                                             target: .domain(Self.fixtureDomain),
                                             key: "label",
                                             value: .string("wonderland"),
                                             ttlMs: nil)
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!)
        XCTAssertThrowsError(try sdk.buildSetMetadata(request: request, keypair: keypair)) { error in
            guard case SwiftTransactionEncoderError.nativeBridgeUnavailable = error else {
                XCTFail("Expected nativeBridgeUnavailable error")
                return
            }
        }
    }

    func testBuildBurnWithoutBridgeThrows() throws {
        NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(false)
        defer { NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(nil) }

        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
        let destination = authority
        let request = BurnRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                  authority: authority,
                                  assetDefinitionId: "rose#wonderland",
                                  quantity: "2",
                                  destination: destination,
                                  ttlMs: 120)
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!)
        XCTAssertThrowsError(try sdk.buildBurn(burn: request, keypair: keypair)) { error in
            guard case SwiftTransactionEncoderError.nativeBridgeUnavailable = error else {
                XCTFail("Expected nativeBridgeUnavailable error")
                return
            }
        }
    }

    func testShieldRequestValidatesCommitmentLength() throws {
        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let payload = try ConfidentialEncryptedPayload(ephemeralPublicKey: Data(repeating: 0x11, count: 32),
                                                       nonce: Data(repeating: 0x22, count: 24),
                                                       ciphertext: Data([0xAA, 0xBB]))
        XCTAssertThrowsError(
            try ShieldRequest(chainId: Self.fixtureChainId,
                              authority: authority,
                              assetDefinitionId: Self.fixtureAssetDefinition,
                              fromAccountId: authority,
                              amount: "1",
                              noteCommitment: Data(repeating: 0x00, count: 16),
                              payload: payload,
                              ttlMs: 10)
        ) { error in
            guard case ShieldRequestError.invalidNoteCommitmentLength = error else {
                return XCTFail("Unexpected error: \(error)")
            }
        }
    }

    func testShieldEncodingMatchesNativeBridge() throws {
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let request = try makeShieldRequest(authority: authority)
        let fallback = try SwiftTransactionEncoder.encodeShield(request: request,
                                                                keypair: keypair,
                                                                creationTimeMs: Self.fixtureCreationTimeMs)

        guard let native = try? NoritoNativeBridge.shared.encodeShield(chainId: request.chainId,
                                                                       authority: request.authority,
                                                                       creationTimeMs: Self.fixtureCreationTimeMs,
                                                                       ttlMs: request.ttlMs,
                                                                       assetDefinitionId: request.assetDefinitionId,
                                                                       fromAccountId: request.fromAccountId,
                                                                       amount: request.amount,
                                                                       noteCommitment: request.noteCommitment,
                                                                       payloadEphemeral: request.payload.ephemeralPublicKey,
                                                                       payloadNonce: request.payload.nonce,
                                                                       payloadCiphertext: request.payload.ciphertext,
                                                                       privateKey: keypair.privateKeyBytes) else {
            XCTFail("Expected native shield encoding")
            return
        }

        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, fallback.signedTransaction)
        XCTAssertEqual(native.hash, fallback.transactionHash)
        XCTAssertEqual(normalized.norito, fallback.norito)
    }

    func testUnshieldRequestRequiresInputs() throws {
        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let proof = try makeProofAttachment()
        XCTAssertThrowsError(
            try UnshieldRequest(chainId: Self.fixtureChainId,
                                authority: authority,
                                assetDefinitionId: Self.fixtureAssetDefinition,
                                toAccountId: authority,
                                publicAmount: "1",
                                inputs: [],
                                proof: proof)
        ) { error in
            guard case UnshieldRequestError.inputsEmpty = error else {
                return XCTFail("Unexpected error: \(error)")
            }
        }
    }

    func testUnshieldRequestValidatesNullifierLength() throws {
        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let proof = try makeProofAttachment()
        let invalidInput = Data(repeating: 0x11, count: 8)
        XCTAssertThrowsError(
            try UnshieldRequest(chainId: Self.fixtureChainId,
                                authority: authority,
                                assetDefinitionId: Self.fixtureAssetDefinition,
                                toAccountId: authority,
                                publicAmount: "1",
                                inputs: [invalidInput],
                                proof: proof)
        ) { error in
            guard case UnshieldRequestError.invalidNullifierLength = error else {
                return XCTFail("Unexpected error: \(error)")
            }
        }
    }

    func testUnshieldRequestValidatesRootHintLength() throws {
        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let proof = try makeProofAttachment()
        let validInput = Data(repeating: 0x42, count: 32)
        let hint = Data(repeating: 0xFF, count: 8)
        XCTAssertThrowsError(
            try UnshieldRequest(chainId: Self.fixtureChainId,
                                authority: authority,
                                assetDefinitionId: Self.fixtureAssetDefinition,
                                toAccountId: authority,
                                publicAmount: "1",
                                inputs: [validInput],
                                proof: proof,
                                rootHint: hint)
        ) { error in
            guard case UnshieldRequestError.invalidRootHintLength = error else {
                return XCTFail("Unexpected error: \(error)")
            }
        }
    }

    func testUnshieldEncodingMatchesNativeBridge() throws {
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }
        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain)
        let request = try makeUnshieldRequest(authority: authority)
        let fallback = try SwiftTransactionEncoder.encodeUnshield(request: request,
                                                                  keypair: keypair,
                                                                  creationTimeMs: Self.fixtureCreationTimeMs)
        guard let native = try? NoritoNativeBridge.shared.encodeUnshield(chainId: request.chainId,
                                                                         authority: request.authority,
                                                                         creationTimeMs: Self.fixtureCreationTimeMs,
                                                                         ttlMs: request.ttlMs,
                                                                         assetDefinitionId: request.assetDefinitionId,
                                                                         destinationAccountId: request.toAccountId,
                                                                         amount: request.publicAmount,
                                                                         inputs: request.flattenedInputs,
                                                                         proofJSON: try request.proof.encodedJSON(),
                                                                         rootHint: request.rootHint,
                                                                         privateKey: keypair.privateKeyBytes) else {
            XCTFail("Expected native unshield encoding")
            return
        }
        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, fallback.signedTransaction)
        XCTAssertEqual(native.hash, fallback.transactionHash)
        XCTAssertEqual(normalized.norito, fallback.norito)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitAndWaitAsyncSucceeds() async throws {
        try requireEd25519Encoder()
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configure(statuses: ["Queued", "Approved"])
        let sdk = try makePipelineSDK()
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      description: nil,
                                      ttlMs: 60)
        let status = try await sdk.submitAndWait(transfer: request, keypair: keypair)
        XCTAssertEqual(status.content.status.kind, "Approved")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitAndWaitAsyncFailureThrows() async throws {
        try requireEd25519Encoder()
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configure(statuses: ["Rejected"])
        let sdk = try makePipelineSDK()
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      description: nil,
                                      ttlMs: 60)
        do {
            _ = try await sdk.submitAndWait(transfer: request, keypair: keypair)
            XCTFail("Expected pipeline failure")
        } catch let error as PipelineStatusError {
            switch error {
            case .failure(_, let status, _):
                XCTAssertEqual(status, "Rejected")
            default:
                XCTFail("Unexpected error: \(error)")
            }
        } catch {
            XCTFail("Unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitAndWaitCompletionDeliversStatus() async throws {
        try requireEd25519Encoder()
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configure(statuses: ["Approved"])
        let sdk = try makePipelineSDK()
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      description: nil,
                                      ttlMs: 60)
        let expectation = expectation(description: "pipeline completion")
        let task = sdk.submitAndWait(transfer: request, keypair: keypair) { result in
            switch result {
            case .success(let status):
                XCTAssertEqual(status.content.status.kind, "Approved")
                expectation.fulfill()
            case .failure(let error):
                XCTFail("Unexpected error: \(error)")
            }
        }
        await fulfillment(of: [expectation], timeout: 2.0)
        task.cancel()
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitAndWaitUsesDefaultPollOptionsWhenAbsent() async throws {
        try requireEd25519Encoder()
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configure(statuses: [])
        let sdk = try makePipelineSDK()
        sdk.pipelinePollOptions = PipelineStatusPollOptions(pollInterval: 0,
                                                           timeout: 0.1,
                                                           maxAttempts: 1)
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      description: nil,
                                      ttlMs: 60)
        do {
            _ = try await sdk.submitAndWait(transfer: request, keypair: keypair)
            XCTFail("Expected timeout due to default poll options")
        } catch let error as PipelineStatusError {
            if case .timeout = error {
                // expected
            } else {
                XCTFail("Unexpected error: \(error)")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitAndWaitCustomSuccessStates() async throws {
        try requireEd25519Encoder()
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configure(statuses: ["Queued"])
        let sdk = try makePipelineSDK()
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      description: nil,
                                      ttlMs: 60)
        let options = PipelineStatusPollOptions(successStates: Set([.queued]), failureStates: Set())
        let status = try await sdk.submitAndWait(transfer: request, keypair: keypair, pollOptions: options)
        XCTAssertEqual(status.content.status.kind, "Queued")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitAddsDeterministicIdempotencyKeyAcrossRetries() async throws {
        try requireEd25519Encoder()
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configureSubmissions(responses: [(503, nil), (202, nil)])
        let sdk = try makePipelineSDK()
        sdk.pipelineSubmitOptions = PipelineSubmitOptions(maxRetries: 1,
                                                          initialBackoffSeconds: 0,
                                                          backoffMultiplier: 1)
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      description: nil,
                                      ttlMs: 60)
        let envelope = try sdk.buildSignedTransfer(transfer: request, keypair: keypair)
        try await sdk.submit(envelope: envelope)

        let keys = PipelineURLProtocol.drainObservedIdempotencyKeys().compactMap { $0 }
        XCTAssertEqual(keys.count, 2)
        XCTAssertEqual(Set(keys), [envelope.hashHex])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitRetriesOnTransportErrorAsync() async throws {
        try requireEd25519Encoder()
        let stub = StubPipelineClient()
        stub.queuedResults = [
            .failure(ToriiClientError.transport(StubTransportError())),
            .success(makeSubmitReceipt()),
        ]
        let sdk = IrohaSDK(toriiClient: stub,
                           baseURL: URL(string: "https://example.test")!,
                           pipelineSubmitOptions: PipelineSubmitOptions(maxRetries: 2, initialBackoffSeconds: 0, backoffMultiplier: 1))
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      description: nil,
                                      ttlMs: 60)
        let envelope = try sdk.buildSignedTransfer(transfer: request, keypair: keypair)
        try await sdk.submit(envelope: envelope)
        XCTAssertEqual(stub.submitted.count, 2)
        XCTAssertEqual(stub.submittedModes, [.pipeline, .pipeline])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitRetriesExhaustAndThrow() async throws {
        try requireEd25519Encoder()
        let stub = StubPipelineClient()
        stub.queuedResults = [
            .failure(ToriiClientError.transport(StubTransportError())),
            .failure(ToriiClientError.transport(StubTransportError())),
        ]
        let sdk = IrohaSDK(toriiClient: stub,
                           baseURL: URL(string: "https://example.test")!,
                           pipelineSubmitOptions: PipelineSubmitOptions(maxRetries: 1, initialBackoffSeconds: 0, backoffMultiplier: 1))
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey, domain: Self.fixtureDomain),
                                      description: nil,
                                      ttlMs: 60)
        let envelope = try sdk.buildSignedTransfer(transfer: request, keypair: keypair)
        do {
            try await sdk.submit(envelope: envelope)
            XCTFail("Expected retry exhaustion")
        } catch {
            // expected
        }
        XCTAssertEqual(stub.submitted.count, 2)
        XCTAssertEqual(stub.submittedModes, [.pipeline, .pipeline])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testPollPipelineStatusAsync() async throws {
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configure(statuses: ["Committed"])
        let sdk = try makePipelineSDK()
        let status = try await sdk.pollPipelineStatus(hashHex: "abc")
        XCTAssertEqual(status.content.status.kind, "Committed")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testPollPipelineStatusCompletion() async throws {
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configure(statuses: ["Rejected"])
        let sdk = try makePipelineSDK()
        let expectation = expectation(description: "poll completion")
        let task = sdk.pollPipelineStatus(hashHex: "abc") { result in
            switch result {
            case .success:
                XCTFail("Expected failure status")
            case .failure(let error):
                if case .failure(_, let status, _) = error as? PipelineStatusError {
                    XCTAssertEqual(status, "Rejected")
                } else {
                    XCTFail("Unexpected error: \(error)")
                }
            }
            expectation.fulfill()
        }
        await fulfillment(of: [expectation], timeout: 2.0)
        task.cancel()
    }

    @available(iOS 15.0, macOS 12.0, *)
    private func makePipelineSDK() throws -> IrohaSDK {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [PipelineURLProtocol.self]
        let session = URLSession(configuration: config)
        return IrohaSDK(baseURL: URL(string: "https://example.test")!, session: session)
    }
}
