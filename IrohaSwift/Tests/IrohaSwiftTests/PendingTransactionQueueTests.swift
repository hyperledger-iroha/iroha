import XCTest
@testable import IrohaSwift

private final class QueueStubPipelineClient: ToriiTransactionSubmitting {
    var submitted: [Data] = []
    var queuedResults: [Swift.Result<ToriiSubmitTransactionResponse?, Error>] = []
    var defaultResult: Swift.Result<ToriiSubmitTransactionResponse?, Error> =
        .success(QueueStubPipelineClient.makeReceipt())

    private static func makeReceipt() -> ToriiSubmitTransactionResponse {
        ToriiSubmitTransactionResponse(
            payload: .init(txHash: "abc", submittedAtMs: 1, submittedAtHeight: 2, signer: "signer"),
            signature: "deadbeef"
        )
    }

    func submitTransaction(data: Data,
                           mode: PipelineEndpointMode,
                           idempotencyKey: String?) async throws -> ToriiSubmitTransactionResponse? {
        submitted.append(data)
        if !queuedResults.isEmpty {
            let next = queuedResults.removeFirst()
            switch next {
            case .success(let response):
                return response
            case .failure(let error):
                throw error
            }
        }
        switch defaultResult {
        case .success(let response):
            return response
        case .failure(let error):
            throw error
        }
    }
}

final class PendingTransactionQueueTests: XCTestCase {
    private func requireNativeEncoder() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
                      "Native transaction encoder unavailable")
    }

    private func makeEnvelope() -> SignedTransactionEnvelope {
        let payload = Data([0x00, 0x01, 0x02])
        let signed = Data([0xAA, 0xBB])
        let hash = Data(repeating: 0x11, count: 32)
        return SignedTransactionEnvelope(norito: payload, signedTransaction: signed, payload: nil, transactionHash: hash)
    }

    private func makeQueueURL(_ name: String = UUID().uuidString) -> URL {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("irohaSwiftQueueTests", isDirectory: true)
        let url = directory.appendingPathComponent("\(name).queue")
        try? FileManager.default.removeItem(at: url)
        return url
    }

    func testFileQueueRoundTrip() throws {
        let url = makeQueueURL()
        let queue = try FilePendingTransactionQueue(fileURL: url)
        let envelope = makeEnvelope()
        try queue.enqueue(envelope)
        XCTAssertEqual(try queue.size(), 1)

        let drained = try queue.drain()
        XCTAssertEqual(drained.count, 1)
        XCTAssertEqual(drained.first?.envelope.norito, envelope.norito)
        XCTAssertEqual(drained.first?.envelope.transactionHash, envelope.transactionHash)
        XCTAssertEqual(try queue.size(), 0)
    }

    func testDrainRejectsOversizeFile() throws {
        let url = makeQueueURL("oversize")
        let config = FilePendingTransactionQueueConfiguration(maxRecords: 4, maxBytes: 16)
        _ = try FilePendingTransactionQueue(fileURL: url, configuration: config)
        try Data(repeating: 0x41, count: 32).write(to: url)

        let queue = try FilePendingTransactionQueue(fileURL: url, configuration: config)
        XCTAssertThrowsError(try queue.drain()) { error in
            guard case FilePendingTransactionQueueError.overflowBytes = error else {
                return XCTFail("expected overflowBytes error, got \(error)")
            }
        }
    }

    func testDrainRejectsCorruptedLines() throws {
        let url = makeQueueURL("corrupted-line")
        _ = try FilePendingTransactionQueue(fileURL: url)
        guard let data = "%%%".data(using: .utf8) else {
            XCTFail("failed to encode corrupted payload")
            return
        }
        try data.write(to: url)

        let queue = try FilePendingTransactionQueue(fileURL: url)
        XCTAssertThrowsError(try queue.drain()) { error in
            guard case FilePendingTransactionQueueError.corruptedEntry(let index) = error else {
                return XCTFail("expected corrupted entry error, got \(error)")
            }
            XCTAssertEqual(index, 0)
        }
    }

    func testEnqueueRejectsOverflowRecords() throws {
        let url = makeQueueURL("enqueue-limit")
        let config = FilePendingTransactionQueueConfiguration(maxRecords: 1, maxBytes: 1 << 20)
        let queue = try FilePendingTransactionQueue(fileURL: url, configuration: config)
        try queue.enqueue(makeEnvelope())

        XCTAssertThrowsError(try queue.enqueue(makeEnvelope())) { error in
            guard case FilePendingTransactionQueueError.overflowRecords(let limit) = error else {
                return XCTFail("expected overflow records error, got \(error)")
            }
            XCTAssertEqual(limit, config.maxRecords)
        }
    }

    func testRejectsTooManyRecords() throws {
        let url = makeQueueURL("too-many")
        let config = FilePendingTransactionQueueConfiguration(maxRecords: 1, maxBytes: 1 << 20)
        _ = try FilePendingTransactionQueue(fileURL: url, configuration: config)
        let record = PendingTransactionRecord(envelope: makeEnvelope())
        let encoded = try JSONEncoder().encode(record).base64EncodedString()
        let content = [encoded, encoded].joined(separator: "\n") + "\n"
        try Data(content.utf8).write(to: url)

        let queue = try FilePendingTransactionQueue(fileURL: url, configuration: config)
        XCTAssertThrowsError(try queue.drain()) { error in
            guard case FilePendingTransactionQueueError.overflowRecords(let limit) = error else {
                return XCTFail("expected overflow records error, got \(error)")
            }
            XCTAssertEqual(limit, config.maxRecords)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSdkQueuesAfterRetryExhaustion() async throws {
        try requireNativeEncoder()
        let stub = QueueStubPipelineClient()
        stub.defaultResult = .failure(ToriiClientError.transport(URLError(.notConnectedToInternet)))
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        sdk.pipelineSubmitOptions = PipelineSubmitOptions(maxRetries: 0)
        let queueURL = makeQueueURL("retry-exhausted")
        sdk.pendingTransactionQueue = try FilePendingTransactionQueue(fileURL: queueURL)

        let keypair = try Keypair.generate()
        let transfer = TransferRequest(chainId: "chain",
                                       authority: AccountId.make(publicKey: keypair.publicKey, domain: "wonderland"),
                                       assetDefinitionId: "rose#wonderland",
                                       quantity: "1",
                                       destination: AccountId.make(publicKey: keypair.publicKey, domain: "wonderland"),
                                       description: nil,
                                       ttlMs: nil)
        do {
            try await sdk.submit(transfer: transfer, keypair: keypair)
            XCTFail("Expected submit to throw")
        } catch {
            // Expected
        }
        let queued = try sdk.pendingTransactionQueue?.drain()
        XCTAssertEqual(queued?.count, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testQueuedTransactionsFlushBeforeNewSubmission() async throws {
        try requireNativeEncoder()
        let stub = QueueStubPipelineClient()
        stub.queuedResults = [.success(nil), .success(nil)]
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let tempQueue = try FilePendingTransactionQueue(fileURL: makeQueueURL("flush"))
        sdk.pendingTransactionQueue = tempQueue

        // Queue a manual envelope.
        let manualEnvelope = makeEnvelope()
        try tempQueue.enqueue(manualEnvelope)

        let keypair = try Keypair.generate()
        let transfer = TransferRequest(chainId: "chain",
                                       authority: AccountId.make(publicKey: keypair.publicKey, domain: "wonderland"),
                                       assetDefinitionId: "rose#wonderland",
                                       quantity: "1",
                                       destination: AccountId.make(publicKey: keypair.publicKey, domain: "wonderland"),
                                       description: nil,
                                       ttlMs: nil)
        try await sdk.submit(transfer: transfer, keypair: keypair)

        XCTAssertEqual(stub.submitted.count, 2)
        XCTAssertEqual(try tempQueue.size(), 0)
    }
}
