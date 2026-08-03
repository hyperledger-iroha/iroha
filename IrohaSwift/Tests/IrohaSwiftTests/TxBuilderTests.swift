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
    private static var statuses: [(kind: String, rejectionReason: String?)] = []
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
            let components = URLComponents(url: url, resolvingAgainstBaseURL: false)
            let hash = components?.queryItems?.first(where: { $0.name == "hash" })?.value ?? ""
            let body = PipelineURLProtocol.dequeueStatusBody(hash: hash)
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

    static func configure(statuses: [String], rejectionReason: String? = nil) {
        lock.lock()
        self.statuses = statuses.map { ($0, rejectionReason) }
        lock.unlock()
    }

    static func configureSubmissions(responses: [(Int, Data?)] = []) {
        lock.lock()
        submitResponses = responses.map { ($0.0, $0.1 ?? defaultSubmitBody) }
        lock.unlock()
    }

    static func reset() {
        lock.lock()
        statuses = []
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

    private static func dequeueStatusBody(hash: String) -> Data {
        lock.lock()
        defer { lock.unlock() }
        if statuses.isEmpty {
            return makeStatusBody(hash: hash, kind: "Queued")
        }
        let next = statuses.removeFirst()
        return makeStatusBody(
            hash: hash,
            kind: next.kind,
            rejectionReason: next.rejectionReason
        )
    }

    private static func nextSubmitResponse() -> (Int, Data) {
        lock.lock()
        defer { lock.unlock() }
        if !submitResponses.isEmpty {
            return submitResponses.removeFirst()
        }
        return (submitStatusCode, submitBody)
    }

    private static func makeStatusBody(
        hash: String,
        kind: String,
        rejectionReason: String? = nil
    ) -> Data {
        var status: [String: Any] = [
            "kind": kind
        ]
        if kind == "Applied" {
            status["block_height"] = 7
        }
        let terminal = ["Applied", "Rejected", "Expired"].contains(kind)
        let diagnostics: [[String: Any]] = rejectionReason.map {
            [[
                "category": "validation",
                "message": $0,
                "decoded_reason": $0
            ]]
        } ?? []
        let payload: [String: Any] = [
            "hash": hash,
            "status": status,
            "summary": rejectionReason.map { "\(kind): \($0)" } ?? kind,
            "diagnostics": diagnostics,
            "scope": "global",
            "resolved_from": terminal ? "state" : "cache"
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
        path.hasSuffix("/v1/pipeline/transactions")
    }

    private static func isStatusPath(_ path: String) -> Bool {
        path.hasSuffix("/v1/pipeline/transactions/status")
    }

    private static func isNodeCapabilitiesPath(_ path: String) -> Bool {
        path.hasSuffix("/v1/node/capabilities")
    }

    private static var nodeCapabilitiesBody: Data {
        let body: [String: Any] = [
            "abi_version": 1,
            "data_model_version": ToriiNodeCapabilities.expectedDataModelVersion,
            "signed_transaction_schema_hash_hex": ToriiNodeCapabilities.expectedSignedTransactionSchemaHashHex
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
    private static let pipelineHash = String(repeating: "a", count: 64)
    private static let fixturePrivateKeyHex = "000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F"
    private static let fixtureChainId = "00000000-0000-0000-0000-000000000000"
    private static let fixtureDomain = "wonderland.universal"
    private static let fixtureGovernanceContractAddress =
        "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh"
    private static let fixtureClaimPolicyId = "email#retail"
    private static let fixtureClaimProgramId = "identifier_lookup_retail"
    private static let fixtureClaimProgramDigestHex =
        "1111111111111111111111111111111111111111111111111111111111111111"
    private static let fixtureClaimOutputHashHex =
        "2222222222222222222222222222222222222222222222222222222222222223"
    private static let fixtureClaimAssociatedDataHashHex =
        "3333333333333333333333333333333333333333333333333333333333333333"
    private static let fixtureClaimOpaqueIdHex =
        "d82f9eab952f7a5241bb2339c0095ebc61958428164ab820fad85952f3574585"
    private static let fixtureClaimReceiptHashHex =
        "032df7e7370e04ddbabf0cd40932935a1d2c77a9b8d723bbb9f1472f2791cc71"
    private static let fixtureClaimUaidHex =
        "c60973f731ccb57008687f9bc38cc712e3be7ab46d99a1beffd1c9fd61e60a87"
    private static let fixtureClaimResolvedAtMs: UInt64 = 1_764_450_000_024
    private static let fixtureClaimExpiresAtMs: UInt64 = 1_764_453_000_056
    private static let fixtureClaimAccountMultihash =
        "ed01205634E9071E8662974A22F137972663C4644DC3546A1938E1CAC58DE4CBA8D965"
    private static let fixtureClaimSignatureHex =
        "9262CA8C755D47207ED0CD2E19892DFAA4612701A36DCAF87173D42CC754DFB6A66158856FDFD25974C2A11E9FC32940CA0DF18CAC25A38CB5DEDC4625E67900"
    private static let fixtureExplorerAccountId = AccountId.make(publicKey: Data(repeating: 0x2A, count: 32))
    private static let fixtureAssetDefinition = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
    private static let fixtureCreationTimeMs: UInt64 = 1_700_000_000_000
    private enum FixtureError: Error { case invalidKey }

    private func makeFixtureKeypair() throws -> Keypair {
        guard let keyData = Data(hexString: Self.fixturePrivateKeyHex) else {
            XCTFail("Invalid fixture private key hex")
            throw FixtureError.invalidKey
        }
        return try Keypair(privateKeyBytes: keyData)
    }


    private func hexEncoded(_ data: Data) -> String {
        data.map { String(format: "%02x", $0) }.joined()
    }

    private func asciiOccurrenceCount(_ needle: String, in data: Data) -> Int {
        let needleBytes = Array(needle.utf8)
        guard !needleBytes.isEmpty else { return 0 }
        let haystack = Array(data)
        guard haystack.count >= needleBytes.count else { return 0 }
        var count = 0
        for offset in 0...(haystack.count - needleBytes.count) {
            if Array(haystack[offset..<(offset + needleBytes.count)]) == needleBytes {
                count += 1
            }
        }
        return count
    }

    private func dataContainsASCII(_ needle: String, in data: Data) -> Bool {
        asciiOccurrenceCount(needle, in: data) > 0
    }

    private func makeNativeClaimIdentifierReceiptJSON(
        _ receipt: ToriiIdentifierResolutionReceipt
    ) throws -> Data {
        try JSONEncoder().encode(receipt)
    }

    private func requireEd25519Encoder() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
            "Native transaction encoder unavailable"
        )
    }

    private func normalizeNativeSignedTransaction(
        _ native: NativeSignedTransaction
    ) -> (signed: Data, norito: Data) {
        XCTAssertEqual(native.signedBytes.first, 1)
        return (signed: Data(native.signedBytes.dropFirst()), norito: native.signedBytes)
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
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                      ttlMs: ttlMs)
    }

    private func makeClaimIdentifierRequest(authority: String,
                                            ttlMs: UInt64? = 30) throws -> ClaimIdentifierRequest {
        guard let multihashBytes = Data(hexString: Self.fixtureClaimAccountMultihash),
              multihashBytes.count == 35,
              multihashBytes.prefix(3) == Data([0xED, 0x01, 0x20]) else {
            throw FixtureError.invalidKey
        }
        let claimAccountId = AccountId.make(publicKey: Data(multihashBytes.dropFirst(3)))
        let payload = ToriiIdentifierResolutionPayload(
            policyId: Self.fixtureClaimPolicyId,
            opaqueId: "opaque:\(Self.fixtureClaimOpaqueIdHex)",
            receiptHash: Self.fixtureClaimReceiptHashHex,
            uaid: "uaid:\(Self.fixtureClaimUaidHex)",
            accountId: claimAccountId,
            execution: ToriiIdentifierResolutionExecutionPayload(
                programId: Self.fixtureClaimProgramId,
                programDigest: Self.fixtureClaimProgramDigestHex,
                backend: "bfv-programmed-sha3-256-v1",
                verificationMode: "signed",
                inputCiphertextHash: String(repeating: "ab", count: 32),
                outputCiphertextHash: String(repeating: "bb", count: 32),
                parameterDigest: String(repeating: "cd", count: 32),
                evaluationKeyDigest: String(repeating: "dd", count: 32),
                outputHash: Self.fixtureClaimOutputHashHex,
                associatedDataHash: Self.fixtureClaimAssociatedDataHashHex,
                executedAtMs: Self.fixtureClaimResolvedAtMs,
                expiresAtMs: Self.fixtureClaimExpiresAtMs
            ),
            opening: ToriiRamLfeOutputOpening(
                payload: ToriiRamLfeOutputOpeningPayload(
                    programId: Self.fixtureClaimProgramId,
                    inputCiphertextHash: String(repeating: "ab", count: 32),
                    outputCiphertextHash: String(repeating: "bb", count: 32),
                    parameterDigest: String(repeating: "cd", count: 32),
                    evaluationKeyDigest: String(repeating: "dd", count: 32),
                    openedOutputHash: Self.fixtureClaimOutputHashHex,
                    openedAtMs: Self.fixtureClaimResolvedAtMs,
                    expiresAtMs: Self.fixtureClaimExpiresAtMs
                ),
                signature: Self.fixtureClaimSignatureHex
            )
        )
        guard let payloadJSON = String(data: try JSONEncoder().encode(payload), encoding: .utf8) else {
            throw FixtureError.invalidKey
        }
        let receiptJSON = """
        {
          "payload":\(payloadJSON),
          "attestation":{
            "kind":"signed",
            "algorithm":"ed25519",
            "signature":"\(Self.fixtureClaimSignatureHex)"
          }
        }
        """
        let receipt = try JSONDecoder().decode(
            ToriiIdentifierResolutionReceipt.self,
            from: Data(receiptJSON.utf8)
        )
        return ClaimIdentifierRequest(chainId: Self.fixtureChainId,
                                      authority: authority,
                                      accountId: claimAccountId,
                                      receipt: receipt,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                      ttlMs: ttlMs)
    }

    func testBuildSignedTransferProducesEnvelope() throws {
        try requireEd25519Encoder()
        let keypair = try Keypair.generate()
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!)
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: AccountId.make(publicKey: keypair.publicKey),
                                       assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                       quantity: "1",
                                       destination: AccountId.make(publicKey: keypair.publicKey),
                                       description: nil,
                                       feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                       ttlMs: 90)
        let envelope = try sdk.buildSignedTransfer(transfer: transfer, keypair: keypair)
        XCTAssertEqual(envelope.norito.first, 1)
        XCTAssertEqual(Data(envelope.norito.dropFirst()), envelope.signedTransaction)
        XCTAssertNil(envelope.payload, "Swift encoder does not embed payload bytes yet")
        XCTAssertEqual(envelope.transactionHash.count, 32)
        if let payload = envelope.payload {
            let tail = expectedTail(forTTL: 90)
            XCTAssertEqual(payload.suffix(tail.count), tail)
        }
    }

    func testBuildSignedExecutableBatchPreservesMixedOrderAndTag() throws {
        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let instruction = try TransactionInstructionFrame(
            wireName: "iroha_data_model::isi::Log",
            framedPayload: noritoEncode(
                typeName: "iroha_data_model::isi::Log",
                payload: Data([1, 2, 3]),
                flags: 0
            )
        )
        let invocation = try TransactionContractInvocation(
            contractAddress: "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh",
            expectedCodeHash: Data(repeating: 0xA5, count: 32),
            entrypoint: "run",
            arguments: Data([4, 5, 6])
        )
        let sdk = IrohaSDK(
            toriiClient: StubPipelineClient(),
            baseURL: URL(string: "https://torii.example")!,
            creationTimeProvider: { Self.fixtureCreationTimeMs }
        )
        let envelope = try sdk.buildSignedExecutableBatch(
            chainId: Self.fixtureChainId,
            authority: authority,
            entries: [
                .instruction(instruction),
                .contractCall(invocation),
                .instruction(instruction),
            ],
            feePayment: .authority(chargeLimits: [], gasLimit: 100_000),
            ttlMs: 60,
            nonce: 7,
            keypair: keypair
        )

        var signedReader = CanonicalNoritoReader(data: envelope.signedTransaction)
        let transactionSignature = try signedReader.readCompactField()
        var transactionSignatureReader = CanonicalNoritoReader(data: transactionSignature)
        let signaturePayload = try transactionSignatureReader.readCompactField()
        XCTAssertEqual(transactionSignatureReader.remaining(), 0)
        var signatureReader = CanonicalNoritoReader(data: signaturePayload)
        XCTAssertEqual(try signatureReader.readUInt64LE(), 64)
        for _ in 0..<64 {
            XCTAssertEqual(try signatureReader.readCompactField().count, 1)
        }
        XCTAssertEqual(signatureReader.remaining(), 0)

        let payload = try signedReader.readCompactField()
        XCTAssertEqual(try signedReader.readCompactField(), Data([0]))
        XCTAssertEqual(signedReader.remaining(), 0)
        var payloadReader = CanonicalNoritoReader(data: payload)
        let chainIdPayload = try payloadReader.readCompactField()
        var chainIdReader = CanonicalNoritoReader(data: chainIdPayload)
        XCTAssertEqual(
            try chainIdReader.readCompactField(),
            CompactNorito.encodeString(Self.fixtureChainId)
        )
        XCTAssertEqual(chainIdReader.remaining(), 0)
        _ = try payloadReader.readCompactField()
        _ = try payloadReader.readCompactField()
        let executable = try payloadReader.readCompactField()
        var executableReader = CanonicalNoritoReader(data: executable)
        XCTAssertEqual(try executableReader.readUInt32LE(), 4)
        let sequence = try executableReader.readCompactField()
        var sequenceReader = CanonicalNoritoReader(data: sequence)
        XCTAssertEqual(try sequenceReader.readUInt64LE(), 3)
        var first = CanonicalNoritoReader(data: try sequenceReader.readCompactField())
        var second = CanonicalNoritoReader(data: try sequenceReader.readCompactField())
        var third = CanonicalNoritoReader(data: try sequenceReader.readCompactField())
        XCTAssertEqual(try first.readUInt32LE(), 0)
        XCTAssertEqual(try second.readUInt32LE(), 1)
        XCTAssertEqual(try third.readUInt32LE(), 0)
        XCTAssertEqual(
            try first.readCompactField(),
            instruction.compactInstructionBoxPayload()
        )
        _ = try second.readCompactField()
        XCTAssertEqual(
            try third.readCompactField(),
            instruction.compactInstructionBoxPayload()
        )
        XCTAssertEqual(first.remaining(), 0)
        XCTAssertEqual(second.remaining(), 0)
        XCTAssertEqual(third.remaining(), 0)
        XCTAssertEqual(sequenceReader.remaining(), 0)
    }

    func testExecutableBatchRejectsEmptyAndMissingContractGasLimit() throws {
        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        XCTAssertThrowsError(try TransactionContractInvocation(
            contractAddress: "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh",
            expectedCodeHash: Data(repeating: 0x10, count: 32),
            entrypoint: "run"
        )) { error in
            XCTAssertEqual(error as? ExecutableBatchInputError, .invalidExpectedCodeHashMarker)
        }
        let invocation = try TransactionContractInvocation(
            contractAddress: "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh",
            expectedCodeHash: Data(repeating: 0x11, count: 32),
            entrypoint: "run"
        )
        let sdk = IrohaSDK(
            toriiClient: StubPipelineClient(),
            baseURL: URL(string: "https://torii.example")!,
            creationTimeProvider: { Self.fixtureCreationTimeMs }
        )

        XCTAssertThrowsError(try sdk.buildSignedExecutableBatch(
            chainId: Self.fixtureChainId,
            authority: authority,
            entries: [],
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
            keypair: keypair
        )) { error in
            XCTAssertEqual(error as? ExecutableBatchInputError, .emptyBatch)
        }
        XCTAssertThrowsError(try sdk.buildSignedExecutableBatch(
            chainId: Self.fixtureChainId,
            authority: authority,
            entries: [.contractCall(invocation)],
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
            keypair: keypair
        )) { error in
            XCTAssertEqual(error as? ExecutableBatchInputError, .missingGasLimit)
        }
    }

    func testContractInvocationRequiresCanonicalV1Bech32mAddress() throws {
        let validAddresses = [
            "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh",
            "irohac1qyqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpk8qhfvtnk",
        ]
        for address in validAddresses {
            XCTAssertNoThrow(try TransactionContractInvocation(
                contractAddress: address,
                expectedCodeHash: Data(repeating: 0x11, count: 32),
                entrypoint: "run"
            ))
        }

        let invalidAddresses = [
            "abc",
            " irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh",
            "TAIRAC1QYQQQQQQQQQQQQPUTUV64ZHF0A0A4HHLQDJ2LHNWUZQ4XJQDDCYQ8",
            "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyqp",
            "irohac1qyqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpk8q7ca9ly",
            "irohac1qgqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpk8qhk43nl",
            "irohac1qyqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpkk75nd5",
            "irohac1qyqqqqqqqqqqqzgfpg9scrgwpugpzysnzs23v9ccrydpk8p2lc7wy",
        ]
        for address in invalidAddresses {
            XCTAssertThrowsError(try TransactionContractInvocation(
                contractAddress: address,
                expectedCodeHash: Data(repeating: 0x11, count: 32),
                entrypoint: "run"
            )) { error in
                XCTAssertEqual(error as? ExecutableBatchInputError, .invalidContractAddress)
            }
        }
    }

    func testBuildAliasSetupPlanVerifiesAndSignsOneAtomicFrameVector() throws {
        try requireEd25519Encoder()
        let canonicalBody = Data([1, 3, 3, 7, 9])
        let authority = Self.fixtureExplorerAccountId
        let alias = try ResolvedAccountAliasV1(
            canonicalName: "merchant@banka.paynet",
            dataspaceId: 7
        )
        let intent = AliasIntentV1.accountAlias(
            try AliasAccountIntentV1(
                alias: alias,
                targetAccount: authority,
                provision: .existing,
                role: .additional
            )
        )
        let ensure = EnsureAlias(
            intent: intent,
            acquisition: try AliasLeaseAcquisitionV1(termYears: 1),
            quoteGuard: try AliasQuoteGuardV1(
                expectedPolicyVersion: 1,
                expectedPaymentAsset: Self.fixtureAssetDefinition,
                maxAmount: "0",
                validUntilMs: Self.fixtureCreationTimeMs + 60_000
            )
        )
        let request = try AliasSetupPlanRequestV1(intents: [ensure])
        let frame = try AliasFramedInstructionV1(
            wireId: EnsureAlias.wireId,
            framedPayload: Data([0x4e, 0x52, 0x54, 0x30])
        )
        let plan = try AliasTransactionPlanV1(
            body: try AliasTransactionPlanBodyV1(
                authority: authority,
                chainId: Self.fixtureChainId,
                anchor: try AliasPlanAnchorV1(
                    blockHeight: 9,
                    blockHash: String(repeating: "01", count: 32)
                ),
                resources: [
                    AliasPlanResourceV1(
                        intent: intent,
                        disposition: .repair,
                        quote: nil,
                        instructionIndex: 0
                    )
                ],
                instructions: [frame],
                totalsByAsset: [],
                warnings: [],
                blockers: [],
                validUntilMs: Self.fixtureCreationTimeMs + 60_000
            ),
            planHash: AliasPlanVerifier.canonicalHash(
                canonicalBodyNorito: canonicalBody
            ).hexEncodedString()
        )
        let sdk = IrohaSDK(
            toriiClient: StubPipelineClient(),
            baseURL: URL(string: "https://torii.example")!,
            creationTimeProvider: { Self.fixtureCreationTimeMs }
        )
        let envelope = try sdk.buildAliasSetupPlan(
            request,
            plan: plan,
            bodyEncoder: { _ in canonicalBody },
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
            keypair: makeFixtureKeypair(),
            frameCodec: { _, payload in
                DecodedEnsureAliasFrame(instruction: ensure, reencodedFrame: payload)
            }
        )

        XCTAssertFalse(envelope.norito.isEmpty)
        XCTAssertEqual(asciiOccurrenceCount(EnsureAlias.wireId, in: envelope.norito), 1)
        XCTAssertThrowsError(
            try sdk.buildAliasSetupPlan(
                request,
                plan: plan,
                bodyEncoder: { _ in canonicalBody },
                feePayment: .authority(chargeLimits: [], gasLimit: nil),
                keypair: makeFixtureKeypair(),
                frameCodec: { _, payload in
                    var changed = payload
                    changed[changed.startIndex] ^= 1
                    return DecodedEnsureAliasFrame(
                        instruction: ensure,
                        reencodedFrame: changed
                    )
                }
            )
        )
    }

    func testBuildAliasLifecyclePlanSignsApplyAndSkipsNoOp() throws {
        try requireEd25519Encoder()
        let authority = Self.fixtureExplorerAccountId
        let alias = try ResolvedAccountAliasV1(canonicalName: "merchant@paynet", dataspaceId: 7)
        let guardValue = try AliasQuoteGuardV1(
            expectedPolicyVersion: 1,
            expectedPaymentAsset: Self.fixtureAssetDefinition,
            maxAmount: "2",
            validUntilMs: Self.fixtureCreationTimeMs + 60_000
        )
        let renewal = try RenewAliasLease(
            target: .accountAlias(alias),
            expectedCurrentExpiryMs: 10,
            targetExpiryMs: 20,
            quoteGuard: guardValue
        )
        let renewalRequest = AliasLifecyclePlanRequestV1.leaseRenewal(
            AliasLeaseRenewPlanRequestV1(renewal: renewal)
        )
        let bodyBytes = Data([9, 8, 7, 6])
        let plan = try AliasLifecycleTransactionPlanV1(
            body: try AliasLifecycleTransactionPlanBodyV1(
                authority: authority,
                chainId: Self.fixtureChainId,
                anchor: try AliasPlanAnchorV1(
                    blockHeight: 9,
                    blockHash: String(repeating: "01", count: 32)
                ),
                operation: .renewLease(renewal),
                disposition: .apply,
                instruction: try AliasFramedInstructionV1(
                    wireId: RenewAliasLease.wireId,
                    framedPayload: Data([0x4e, 0x52, 0x54, 0x30])
                ),
                quote: try AliasLeaseQuoteV1(
                    target: renewal.target,
                    pricingClass: 1,
                    exactAmount: "1",
                    quoteGuard: guardValue,
                    expiresAtMs: renewal.targetExpiryMs,
                    graceExpiresAtMs: 30,
                    redemptionExpiresAtMs: 40
                ),
                totalsByAsset: [try AliasAssetTotalV1(
                    paymentAsset: guardValue.expectedPaymentAsset,
                    amount: "1"
                )],
                warnings: [],
                blockers: [],
                validUntilMs: guardValue.validUntilMs
            ),
            planHash: AliasPlanVerifier.canonicalLifecycleHash(
                canonicalBodyNorito: bodyBytes
            ).hexEncodedString()
        )
        let sdk = IrohaSDK(
            toriiClient: StubPipelineClient(),
            baseURL: URL(string: "https://torii.example")!,
            creationTimeProvider: { Self.fixtureCreationTimeMs }
        )
        let envelope = try XCTUnwrap(sdk.buildAliasLifecyclePlan(
            renewalRequest,
            plan: plan,
            bodyEncoder: { _ in bodyBytes },
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
            keypair: makeFixtureKeypair(),
            frameCodec: { _, payload in
                DecodedAliasLifecycleFrame(
                    operation: .renewLease(renewal),
                    reencodedFrame: payload
                )
            }
        ))
        XCTAssertEqual(asciiOccurrenceCount(RenewAliasLease.wireId, in: envelope.norito), 1)

        let noOpBytes = Data([1, 1, 2, 3])
        let configuration = ConfigureAliasAutoRenew(
            target: .accountAlias(alias),
            expectedRevision: 0,
            config: nil
        )
        let autoRenewRequest = AliasLifecyclePlanRequestV1.autoRenew(
            AliasAutoRenewPlanRequestV1(configuration: configuration)
        )
        let noOp = try AliasLifecycleTransactionPlanV1(
            body: try AliasLifecycleTransactionPlanBodyV1(
                authority: authority,
                chainId: Self.fixtureChainId,
                anchor: plan.body.anchor,
                operation: .configureAutoRenew(configuration),
                disposition: .noOp,
                instruction: nil,
                quote: nil,
                totalsByAsset: [],
                warnings: [],
                blockers: [],
                validUntilMs: Self.fixtureCreationTimeMs + 60_000
            ),
            planHash: AliasPlanVerifier.canonicalLifecycleHash(
                canonicalBodyNorito: noOpBytes
            ).hexEncodedString()
        )
        XCTAssertNil(try sdk.buildAliasLifecyclePlan(
            autoRenewRequest,
            plan: noOp,
            bodyEncoder: { _ in noOpBytes },
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
            keypair: makeFixtureKeypair(),
            frameCodec: { _, payload in
                DecodedAliasLifecycleFrame(
                    operation: .configureAutoRenew(configuration),
                    reencodedFrame: payload
                )
            }
        ))
    }

    func testCreationTimeProviderProducesDeterministicHash() throws {
        try requireEd25519Encoder()

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let transfer = TransferRequest(chainId: Self.fixtureChainId,
                                       authority: authority,
                                       assetDefinitionId: Self.fixtureAssetDefinition,
                                       quantity: "1",
                                       destination: authority,
                                       description: "deterministic",
                                       feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
        sdk.getAssets(accountId: Self.fixtureExplorerAccountId) { result in
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
    func testGetExplorerTransactionTransfersFailsWhenRestClientUnavailable() {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let expectation = expectation(description: "rest unavailable")
        sdk.getExplorerTransactionTransfers(hashHex: "deadbeef") { result in
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
    func testGetExplorerTransactionTransfersAsyncFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            _ = try await sdk.getExplorerTransactionTransfers(hashHex: "deadbeef")
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerTransactionTransferSummariesFailsWhenRestClientUnavailable() {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let expectation = expectation(description: "rest unavailable")
        sdk.getExplorerTransactionTransferSummaries(hashHex: "deadbeef") { result in
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
    func testGetExplorerTransactionTransferSummariesAsyncFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            _ = try await sdk.getExplorerTransactionTransferSummaries(hashHex: "deadbeef")
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamTransactionTransferSummariesFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            for try await _ in sdk.streamTransactionTransferSummaries(hashHex: "deadbeef") {
                XCTFail("expected no items when REST client is missing")
            }
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
        sdk.getAccountTransferHistory(accountId: Self.fixtureExplorerAccountId) { result in
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
            _ = try await sdk.getAccountTransferHistory(accountId: Self.fixtureExplorerAccountId)
            XCTFail("expected failure when REST client is missing")
        } catch {
            XCTAssertEqual(error as? IrohaSDKError, .restClientUnavailable)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionHistoryFailsWhenRestClientUnavailable() {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        let expectation = expectation(description: "rest unavailable")
        sdk.getTransactionHistory(accountId: Self.fixtureExplorerAccountId) { result in
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
    func testGetTransactionHistoryAsyncFailsWhenRestClientUnavailable() async {
        let stub = StubPipelineClient()
        let sdk = IrohaSDK(toriiClient: stub, baseURL: URL(string: "https://example.test")!)
        do {
            _ = try await sdk.getTransactionHistory(accountId: Self.fixtureExplorerAccountId)
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
            for try await _ in sdk.iterateAccountTransferHistory(accountId: Self.fixtureExplorerAccountId) {
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
            for try await _ in sdk.streamAccountTransferHistory(accountId: Self.fixtureExplorerAccountId) {
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
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: authority,
                                       assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                       quantity: "1",
                                       destination: authority,
                                       description: nil,
                                       feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
            "Native transaction encoder unavailable"
        )
        let keypair = try makeFixtureKeypair()
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!)
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let request = try makeRegisterZkAssetRequest(authority: authority, ttlMs: 60)
        let envelope = try sdk.buildRegisterZkAsset(request: request, keypair: keypair)
        XCTAssertEqual(envelope.norito.first, 1)
        XCTAssertEqual(Data(envelope.norito.dropFirst()), envelope.signedTransaction)
        XCTAssertEqual(envelope.transactionHash.count, 32)
    }

    func testBuildClaimIdentifierProducesEnvelope() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
            "Native transaction encoder unavailable"
        )
        let keypair = try makeFixtureKeypair()
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!)
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let request = try makeClaimIdentifierRequest(authority: authority, ttlMs: 60)
        XCTAssertEqual(request.receipt.attestation.algorithm, SigningAlgorithm.ed25519.wireName)
        let envelope = try sdk.buildClaimIdentifier(request: request, keypair: keypair)
        XCTAssertEqual(envelope.norito.first, 1)
        XCTAssertEqual(Data(envelope.norito.dropFirst()), envelope.signedTransaction)
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
        XCTAssertThrowsError(try VerifyingKeyIdReference(backend: " halo2/ipa ", name: "vk")) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .surroundingWhitespace)
        }
        XCTAssertThrowsError(try VerifyingKeyIdReference(backend: "halo2/ipa", name: " vk ")) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .surroundingWhitespace)
        }
    }

    func testSubmitPropagatesError() throws {
        try requireEd25519Encoder()
        enum StubError: Error { case failure }
        let stub = StubPipelineClient()
        stub.result = .failure(StubError.failure)
        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: authority,
                                       assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                       quantity: "1",
                                       destination: authority,
                                       description: nil,
                                       feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: authority,
                                       assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                       quantity: "1",
                                       destination: authority,
                                       description: nil,
                                       feePayment: .authority(chargeLimits: [], gasLimit: nil),)
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
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: authority,
                                       assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                       quantity: "1",
                                       destination: authority,
                                       description: nil,
                                       feePayment: .authority(chargeLimits: [], gasLimit: nil),)
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

    func testSwiftTransferMatchesNativeBridge() throws {
        try requireEd25519Encoder()

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: authority,
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: authority,
                                      description: nil,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                      ttlMs: 90)

        let swift = try SwiftTransactionEncoder.encodeTransfer(transfer: request,
                                                               keypair: keypair,
                                                               creationTimeMs: Self.fixtureCreationTimeMs)

        guard let native = try? NoritoNativeBridge.shared.encodeTransfer(chainId: request.chainId,
                                                                         authority: request.authority,
                                                                         creationTimeMs: Self.fixtureCreationTimeMs,
                                                                         ttlMs: request.ttlMs,
                                                                         assetDefinitionId: request.assetDefinitionId,
                                                                         quantity: request.quantity,
                                                                         destination: request.destination,
                                                                         feePaymentJSON: try request.feePayment.canonicalJSONData(),
                                                                         privateKey: keypair.privateKeyBytes) else {
            XCTFail("Expected native bridge transfer encoding")
            return
        }

        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, swift.signedTransaction)
        XCTAssertEqual(native.hash, swift.transactionHash)
        XCTAssertEqual(normalized.norito, swift.norito)
    }

    func testSigningKeyTransferMatchesKeypairEncoding() throws {
        try requireEd25519Encoder()
        let keypair = try makeFixtureKeypair()
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: authority,
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "10",
                                      destination: authority,
                                      description: nil,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
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

    func testSwiftMintMatchesNativeBridge() throws {
        try requireEd25519Encoder()

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let request = MintRequest(chainId: Self.fixtureChainId,
                                  authority: authority,
                                  assetDefinitionId: Self.fixtureAssetDefinition,
                                  quantity: "3.14",
                                  destination: authority,
                                  feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                  ttlMs: 45)

        let swift = try SwiftTransactionEncoder.encodeMint(request: request,
                                                           keypair: keypair,
                                                           creationTimeMs: Self.fixtureCreationTimeMs)

        guard let native = try? NoritoNativeBridge.shared.encodeMint(chainId: request.chainId,
                                                                     authority: request.authority,
                                                                     creationTimeMs: Self.fixtureCreationTimeMs,
                                                                     ttlMs: request.ttlMs,
                                                                     assetDefinitionId: request.assetDefinitionId,
                                                                     quantity: request.quantity,
                                                                     destination: request.destination,
                                                                     feePaymentJSON: try request.feePayment.canonicalJSONData(),
                                                                     privateKey: keypair.privateKeyBytes) else {
            XCTFail("Expected native bridge mint encoding")
            return
        }

        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, swift.signedTransaction)
        XCTAssertEqual(native.hash, swift.transactionHash)
        XCTAssertEqual(normalized.norito, swift.norito)
    }

    func testSwiftBurnMatchesNativeBridge() throws {
        try requireEd25519Encoder()

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let request = BurnRequest(chainId: Self.fixtureChainId,
                                  authority: authority,
                                  assetDefinitionId: Self.fixtureAssetDefinition,
                                  quantity: "2",
                                  destination: authority,
                                  feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                  ttlMs: 120)

        let swift = try SwiftTransactionEncoder.encodeBurn(request: request,
                                                           keypair: keypair,
                                                           creationTimeMs: Self.fixtureCreationTimeMs)

        guard let native = try? NoritoNativeBridge.shared.encodeBurn(chainId: request.chainId,
                                                                     authority: request.authority,
                                                                     creationTimeMs: Self.fixtureCreationTimeMs,
                                                                     ttlMs: request.ttlMs,
                                                                     assetDefinitionId: request.assetDefinitionId,
                                                                     quantity: request.quantity,
                                                                     destination: request.destination,
                                                                     feePaymentJSON: try request.feePayment.canonicalJSONData(),
                                                                     privateKey: keypair.privateKeyBytes) else {
            XCTFail("Expected native bridge burn encoding")
            return
        }

        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, swift.signedTransaction)
        XCTAssertEqual(native.hash, swift.transactionHash)
        XCTAssertEqual(normalized.norito, swift.norito)
    }

    func testSetMetadataMatchesNativeBridge() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
            "Native transaction encoder unavailable"
        )

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let value = try NoritoJSON("wonderland")
        let request = SetMetadataRequest(chainId: Self.fixtureChainId,
                                         authority: authority,
                                         target: .account(authority),
                                         key: "display_name",
                                         value: value,
                                         feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                         ttlMs: 30)

        let swift = try SwiftTransactionEncoder.encodeSetMetadata(request: request,
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
            feePaymentJSON: try request.feePayment.canonicalJSONData(),
            privateKey: keypair.privateKeyBytes
        ) else {
            XCTFail("Expected native bridge set metadata encoding")
            return
        }

        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, swift.signedTransaction)
        XCTAssertEqual(native.hash, swift.transactionHash)
        XCTAssertEqual(normalized.norito, swift.norito)
    }

    func testClaimIdentifierMatchesNativeBridge() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
            "Native transaction encoder unavailable"
        )

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let request = try makeClaimIdentifierRequest(authority: authority, ttlMs: 75)

        let swift = try SwiftTransactionEncoder.encodeClaimIdentifier(request: request,
                                                                      keypair: keypair,
                                                                      creationTimeMs: Self.fixtureCreationTimeMs)

        let receiptJSON = try makeNativeClaimIdentifierReceiptJSON(request.receipt)
        guard let native = try? NoritoNativeBridge.shared.encodeClaimIdentifier(
            chainId: request.chainId,
            authority: request.authority,
            creationTimeMs: Self.fixtureCreationTimeMs,
            ttlMs: request.ttlMs,
            accountId: request.accountId,
            receiptJSON: receiptJSON,
            feePaymentJSON: try request.feePayment.canonicalJSONData(),
            privateKey: keypair.privateKeyBytes,
            algorithm: .ed25519
        ) else {
            XCTFail("Expected native bridge claim identifier encoding")
            return
        }

        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, swift.signedTransaction)
        XCTAssertEqual(native.hash, swift.transactionHash)
        XCTAssertEqual(normalized.norito, swift.norito)
    }

    func testGovernanceWindowAndDeployAbiAreExactByConstruction() throws {
        let fullRange = try GovernanceWindow(lower: 0, upper: UInt64.max)
        XCTAssertEqual(fullRange.lower, 0)
        XCTAssertEqual(fullRange.upper, UInt64.max)

        XCTAssertThrowsError(try GovernanceWindow(lower: 2, upper: 1)) { error in
            XCTAssertEqual(
                error as? TransactionInputError,
                .invalidGovernanceWindow(lower: 2, upper: 1)
            )
        }

        for abiVersion in ["", "01", "2", " 1", "1 "] {
            XCTAssertThrowsError(
                try ProposeDeployContractRequest(
                    chainId: Self.fixtureChainId,
                    authority: "authority",
                    contractAddress: Self.fixtureGovernanceContractAddress,
                    codeHashHex: String(repeating: "11", count: 32),
                    abiHashHex: String(repeating: "22", count: 32),
                    abiVersion: abiVersion,
                    feePayment: .authority(chargeLimits: [], gasLimit: nil)
                )
            ) { error in
                XCTAssertEqual(
                    error as? TransactionInputError,
                    .invalidGovernanceAbiVersion(abiVersion)
                )
            }
        }
    }

    func testGovernanceProposeDeployMatchesNativeBridge() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
            "Native transaction encoder unavailable"
        )

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let codeHash = Data(repeating: 0x11, count: 32)
        let abiHash = Data(repeating: 0x22, count: 32)
        let window = try GovernanceWindow(lower: 4, upper: 8)
        let request = try ProposeDeployContractRequest(chainId: Self.fixtureChainId,
                                                       authority: authority,
                                                       contractAddress: Self.fixtureGovernanceContractAddress,
                                                       codeHashHex: hexEncoded(codeHash),
                                                       abiHashHex: hexEncoded(abiHash),
                                                       abiVersion: "1",
                                                       window: window,
                                                       mode: .plain,
                                                       feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                                       ttlMs: 20)

        let swift = try SwiftTransactionEncoder.encodeProposeDeploy(request: request,
                                                                    keypair: keypair,
                                                                    creationTimeMs: Self.fixtureCreationTimeMs)

        guard let native = try? NoritoNativeBridge.shared.encodeGovernanceProposeDeploy(
            chainId: request.chainId,
            authority: request.authority,
            creationTimeMs: Self.fixtureCreationTimeMs,
            ttlMs: request.ttlMs,
            contractAddress: request.contractAddress,
            codeHashHex: request.codeHashHex,
            abiHashHex: request.abiHashHex,
            abiVersion: request.abiVersion,
            window: request.window.map { ($0.lower, $0.upper) },
            modeCode: request.mode?.rawValue,
            feePaymentJSON: try request.feePayment.canonicalJSONData(),
            privateKey: keypair.privateKeyBytes
        ) else {
            XCTFail("Expected native bridge governance encoding")
            return
        }

        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, swift.signedTransaction)
        XCTAssertEqual(native.hash, swift.transactionHash)
        XCTAssertEqual(normalized.norito, swift.norito)
    }

    func testPersistCouncilMatchesNativeBridge() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
            "Native transaction encoder unavailable"
        )

        let keypair = try makeFixtureKeypair()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let request = PersistCouncilRequest(chainId: Self.fixtureChainId,
                                            authority: authority,
                                            epoch: 7,
                                            members: [authority],
                                            feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                            ttlMs: 15)

        let swift = try SwiftTransactionEncoder.encodePersistCouncil(request: request,
                                                                     keypair: keypair,
                                                                     creationTimeMs: Self.fixtureCreationTimeMs)
        let membersJson = try NoritoJSON(request.members).data

        guard let native = try? NoritoNativeBridge.shared.encodeGovernancePersistCouncil(
            chainId: request.chainId,
            authority: request.authority,
            creationTimeMs: Self.fixtureCreationTimeMs,
            ttlMs: request.ttlMs,
            epoch: request.epoch,
            membersJson: membersJson,
            feePaymentJSON: try request.feePayment.canonicalJSONData(),
            privateKey: keypair.privateKeyBytes
        ) else {
            XCTFail("Expected native bridge persist council encoding")
            return
        }

        let normalized = normalizeNativeSignedTransaction(native)
        XCTAssertEqual(normalized.signed, swift.signedTransaction)
        XCTAssertEqual(native.hash, swift.transactionHash)
        XCTAssertEqual(normalized.norito, swift.norito)
    }

    func testNativeBridgeTransferWhenAvailable() throws {
        try requireEd25519Encoder()

        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: authority,
                                       assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                       quantity: "1",
                                       destination: authority,
                                       description: nil,
                                       feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                       ttlMs: nil)

        guard let native = try? NoritoNativeBridge.shared.encodeTransfer(chainId: transfer.chainId,
                                                                         authority: transfer.authority,
                                                                         creationTimeMs: UInt64(Date().timeIntervalSince1970 * 1000),
                                                                         ttlMs: nil,
                                                                         assetDefinitionId: transfer.assetDefinitionId,
                                                                         quantity: transfer.quantity,
                                                                         destination: transfer.destination,
                                                                         feePaymentJSON: try transfer.feePayment.canonicalJSONData(),
                                                                         privateKey: keypair.privateKeyBytes) else {
            XCTFail("Expected native bridge to produce transaction")
            return
        }

        XCTAssertEqual(native.hash.count, 32)
        XCTAssertFalse(native.signedBytes.isEmpty)
    }

    func testDecodeSignedTransactionJSONWhenAvailable() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAvailable,
            "NoritoBridge native encoder not linked"
        )

        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: authority,
                                       assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                       quantity: "1",
                                       destination: authority,
                                       description: nil,
                                       feePayment: .authority(chargeLimits: [], gasLimit: nil),
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

    func testDecodeSignedTransactionJSONIncludesSponsorProgramIntent() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAvailable,
            "NoritoBridge native encoder not linked"
        )

        let keypair = try Keypair.generate()
        let sponsorKeypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let feeSponsor = AccountId.make(publicKey: sponsorKeypair.publicKey)
        let programId = try FeeSponsorProgramId(sponsor: feeSponsor, name: "wallet_fx")
        let transfer = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                       authority: authority,
                                       assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                       quantity: "1",
                                       destination: authority,
                                       description: nil,
                                       feePayment: .sponsor(
                                           programId: programId,
                                           programRevision: 3,
                                           chargeLimits: [],
                                           gasLimit: nil
                                       ),
                                       ttlMs: nil)
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!)
        let envelope = try sdk.buildSignedTransfer(transfer: transfer, keypair: keypair)
        guard let json = sdk.decodeSignedTransaction(envelope: envelope) else {
            XCTFail("expected JSON from native bridge")
            return
        }

        XCTAssertTrue(json.contains("\"fee_payment\""), json)
        XCTAssertTrue(json.contains("\"program_revision\":3"), json)
        XCTAssertTrue(json.contains(feeSponsor), json)
    }

    func testMintNativeBridgeWhenAvailable() throws {
        try requireEd25519Encoder()

        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let destination = authority

        guard let native = try? NoritoNativeBridge.shared.encodeMint(chainId: "00000000-0000-0000-0000-000000000000",
                                                                     authority: authority,
                                                                     creationTimeMs: UInt64(Date().timeIntervalSince1970 * 1000),
                                                                     ttlMs: nil,
                                                                     assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                                                     quantity: "42",
                                                                     destination: destination,
                                                                     feePaymentJSON: try FeePaymentIntent.authority(
                                                                         chargeLimits: [],
                                                                         gasLimit: nil
                                                                     ).canonicalJSONData(),
                                                                     privateKey: keypair.privateKeyBytes) else {
            XCTFail("Expected native bridge to encode mint")
            return
        }

        XCTAssertEqual(native.hash.count, 32)
        XCTAssertFalse(native.signedBytes.isEmpty)
    }

    func testSetMetadataNativeBridgeWhenAvailable() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
            "Native transaction encoder unavailable"
        )

        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let request = try SetMetadataRequest(chainId: Self.fixtureChainId,
                                             authority: authority,
                                             target: .domain(Self.fixtureDomain),
                                             key: "label",
                                             value: .string("wonderland"),
                                             feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                             ttlMs: nil)
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        let envelope = try SwiftTransactionEncoder.encodeSetMetadata(request: request,
                                                                     signingKey: signingKey,
                                                                     creationTimeMs: Self.fixtureCreationTimeMs)
        XCTAssertEqual(envelope.transactionHash.count, 32)
        XCTAssertFalse(envelope.signedTransaction.isEmpty)
    }

    func testProposeDeployNativeBridgeWhenAvailable() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
            "Native transaction encoder unavailable"
        )

        let keypair = try Keypair.generate()
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let request = try ProposeDeployContractRequest(chainId: Self.fixtureChainId,
                                                       authority: authority,
                                                       contractAddress: Self.fixtureGovernanceContractAddress,
                                                       codeHashHex: String(repeating: "aa", count: 32),
                                                       abiHashHex: String(repeating: "bb", count: 32),
                                                       abiVersion: "1",
                                                       window: GovernanceWindow(lower: 1, upper: 5),
                                                       mode: .zk,
                                                       feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let destination = authority
        let request = MintRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                  authority: authority,
                                  assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                  quantity: "3.14",
                                  destination: destination,
                                  feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let request = try SetMetadataRequest(chainId: Self.fixtureChainId,
                                             authority: authority,
                                             target: .domain(Self.fixtureDomain),
                                             key: "label",
                                             value: .string("wonderland"),
                                             feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let destination = authority
        let request = BurnRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                  authority: authority,
                                  assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                  quantity: "2",
                                  destination: destination,
                                  feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                  ttlMs: 120)
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!)
        XCTAssertThrowsError(try sdk.buildBurn(burn: request, keypair: keypair)) { error in
            guard case SwiftTransactionEncoderError.nativeBridgeUnavailable = error else {
                XCTFail("Expected nativeBridgeUnavailable error")
                return
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitAndWaitAsyncSucceeds() async throws {
        try requireEd25519Encoder()
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configure(statuses: ["Queued", "Approved", "Committed", "Applied"])
        let sdk = try makePipelineSDK()
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey),
                                      description: nil,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                      ttlMs: 60)
        let status = try await sdk.submitAndWait(transfer: request, keypair: keypair)
        XCTAssertEqual(status.status.kind, "Applied")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitAndWaitAsyncDoesNotTreatApprovedAsDefaultSuccess() async throws {
        try requireEd25519Encoder()
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configure(statuses: ["Approved"])
        let sdk = try makePipelineSDK()
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey),
                                      description: nil,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                      ttlMs: 60)
        let options = PipelineStatusPollOptions(pollInterval: 0,
                                                timeout: 0.1,
                                                maxAttempts: 2)
        do {
            _ = try await sdk.submitAndWait(transfer: request, keypair: keypair, pollOptions: options)
            XCTFail("Expected Approved-only status stream to time out")
        } catch let error as PipelineStatusError {
            if case .timeout = error {
                // expected
            } else {
                XCTFail("Unexpected error: \(error)")
            }
        } catch {
            XCTFail("Unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitAndWaitAsyncDoesNotTreatCommittedAsSuccess() async throws {
        try requireEd25519Encoder()
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configure(statuses: ["Committed"])
        let sdk = try makePipelineSDK()
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey),
                                      description: nil,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                      ttlMs: 60)
        let options = PipelineStatusPollOptions(pollInterval: 0,
                                                timeout: 0.1,
                                                maxAttempts: 2)
        do {
            _ = try await sdk.submitAndWait(transfer: request, keypair: keypair, pollOptions: options)
            XCTFail("Expected Committed-only status stream to time out")
        } catch let error as PipelineStatusError {
            if case .timeout = error {
                // expected
            } else {
                XCTFail("Unexpected error: \(error)")
            }
        } catch {
            XCTFail("Unexpected error: \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitAndWaitAsyncFailureThrows() async throws {
        try requireEd25519Encoder()
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configure(statuses: ["Rejected"])
        let sdk = try makePipelineSDK()
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey),
                                      description: nil,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
    func testSubmitAndWaitAsyncFailureIncludesRejectionReason() async throws {
        try requireEd25519Encoder()
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configure(statuses: ["Rejected"], rejectionReason: "build_claim_missing")
        let sdk = try makePipelineSDK()
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey),
                                      description: nil,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                      ttlMs: 60)
        do {
            _ = try await sdk.submitAndWait(transfer: request, keypair: keypair)
            XCTFail("Expected pipeline failure")
        } catch let error as PipelineStatusError {
            switch error {
            case .failure:
                XCTAssertEqual(error.rejectionReason, "build_claim_missing")
                XCTAssertTrue(error.localizedDescription.contains("build_claim_missing"))
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
        PipelineURLProtocol.configure(statuses: ["Approved", "Applied"])
        let sdk = try makePipelineSDK()
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey),
                                      description: nil,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                      ttlMs: 60)
        let expectation = expectation(description: "pipeline completion")
        let task = sdk.submitAndWait(transfer: request, keypair: keypair) { result in
            switch result {
            case .success(let status):
                XCTAssertEqual(status.status.kind, "Applied")
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
                                      authority: AccountId.make(publicKey: keypair.publicKey),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey),
                                      description: nil,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
    func testSubmitAndWaitDoesNotAllowCustomNonFinalSuccessStates() async throws {
        try requireEd25519Encoder()
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configure(statuses: ["Approved"])
        let sdk = try makePipelineSDK()
        let keypair = try makeFixtureKeypair()
        let request = TransferRequest(chainId: Self.fixtureChainId,
                                      authority: AccountId.make(publicKey: keypair.publicKey),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey),
                                      description: nil,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                      ttlMs: 60)
        let options = PipelineStatusPollOptions(pollInterval: 0, timeout: 0.1, maxAttempts: 2)
        do {
            _ = try await sdk.submitAndWait(transfer: request, keypair: keypair, pollOptions: options)
            XCTFail("Expected Approved-only status to remain non-final")
        } catch let error as PipelineStatusError {
            guard case .timeout = error else {
                return XCTFail("Unexpected error: \(error)")
            }
        }
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
                                      authority: AccountId.make(publicKey: keypair.publicKey),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey),
                                      description: nil,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
                                      authority: AccountId.make(publicKey: keypair.publicKey),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey),
                                      description: nil,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
                                      authority: AccountId.make(publicKey: keypair.publicKey),
                                      assetDefinitionId: Self.fixtureAssetDefinition,
                                      quantity: "1",
                                      destination: AccountId.make(publicKey: keypair.publicKey),
                                      description: nil,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
        PipelineURLProtocol.configure(statuses: ["Committed", "Applied"])
        let sdk = try makePipelineSDK()
        let status = try await sdk.pollPipelineStatus(hashHex: Self.pipelineHash)
        XCTAssertEqual(status.status.kind, "Applied")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testPollPipelineStatusCompletion() async throws {
        PipelineURLProtocol.reset()
        PipelineURLProtocol.configure(statuses: ["Rejected"])
        let sdk = try makePipelineSDK()
        let expectation = expectation(description: "poll completion")
        let task = sdk.pollPipelineStatus(hashHex: Self.pipelineHash) { result in
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
