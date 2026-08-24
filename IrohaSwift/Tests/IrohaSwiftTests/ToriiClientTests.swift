import XCTest
import CryptoKit
#if canImport(Combine)
import Combine
#endif
@testable import IrohaSwift

func testFeePayment(gasLimit: UInt64? = nil) -> FeePaymentIntent {
    .authority(chargeLimits: [], gasLimit: gasLimit)
}

func testFeePaymentObject(_ intent: FeePaymentIntent) -> [String: Any] {
    let data = try! intent.canonicalJSONData()
    return try! JSONSerialization.jsonObject(with: data) as! [String: Any]
}

/// Deterministic stand-in for the exact Norito body encoder supplied by an app.
/// Receipt-verifier tests separately pin the protocol domain and hash marker.
private func encodeTestCanonicalOnboardingBody(
    _ body: ToriiAccountOnboardingPlanBody
) throws -> Data {
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.sortedKeys]
    return try encoder.encode(body)
}

private func kagemushaOperationRequestArchive(
    schema: String,
    fieldCount: Int,
    operationIdFieldIndex: Int
) -> Data {
    var payload = CompactNoritoWriter()
    for index in 0..<fieldCount {
        let field: Data
        if index == 0 {
            field = CompactNorito.encodeUInt16(KagemushaRecursiveSpend.wireVersionV4)
        } else if index == operationIdFieldIndex {
            field = Data(repeating: 0x11, count: 32)
        } else {
            field = Data([UInt8(index + 1)])
        }
        payload.writeField(field)
    }
    return KagemushaRecursiveSpend.frameArchive(
        schema: schema,
        payload: payload.data
    )
}

final class StubURLProtocol: URLProtocol {
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

private func canonicalVerifierRecordArchive(
    seed: UInt8,
    verifierKeyLength: Int = 96
) throws -> Data {
    guard verifierKeyLength > 0 else {
        throw NSError(
            domain: "ToriiClientTests",
            code: -1,
            userInfo: [NSLocalizedDescriptionKey: "verifierKeyLength must be positive"]
        )
    }
    return noritoEncode(
        typeName: "iroha_data_model::proof::VerifyingKeyRecord",
        payload: Data(repeating: seed, count: verifierKeyLength),
        flags: NoritoHeader.compactLen
    )
}

private func nativeAmxDiagnosticsPayload(
    preparePhase: String = "prepare",
    signature: [UInt8]? = nil,
    duplicateLeg: Bool = false,
    secondEntrypointHash: String? = nil,
    authorityContextHeight: Int = 40,
    receiptLaneIncarnation: String? = nil,
    receiptProposalHash: String? = nil
) throws -> Data {
    guard let golden = try loadNativeAmxGroupedFixture()["golden"] as? [String: Any],
          var diagnostics = golden["expected_diagnostics"] as? [String: Any],
          var commitments =
              diagnostics["lane_settlement_commitments"] as? [[String: Any]],
          !commitments.isEmpty,
          var nativeReceipts =
              commitments[0]["native_amx_receipts"] as? [[String: Any]],
          !nativeReceipts.isEmpty,
          var legs = nativeReceipts[0]["legs"] as? [[String: Any]],
          legs.count >= 2
    else {
        throw NSError(
            domain: "SumeragiV2Fixture",
            code: 1,
            userInfo: [
                NSLocalizedDescriptionKey:
                    "native AMX grouped fixture lacks canonical diagnostics evidence",
            ]
        )
    }

    func mutateQcBody(
        legAt index: Int,
        qcKey: String,
        _ mutate: (inout [String: Any]) -> Void
    ) {
        var qc = legs[index][qcKey] as! [String: Any]
        var body = qc["body"] as! [String: Any]
        mutate(&body)
        qc["body"] = body
        legs[index][qcKey] = qc
    }

    mutateQcBody(legAt: 0, qcKey: "prepare_qc") { body in
        body["phase"] = ["phase": preparePhase, "detail": NSNull()]
    }
    if let signature {
        var qc = legs[0]["prepare_qc"] as! [String: Any]
        qc["bls_aggregate_signature"] = signature
        legs[0]["prepare_qc"] = qc
    }
    if duplicateLeg {
        legs[1] = legs[0]
    }
    if let secondEntrypointHash {
        for qcKey in ["prepare_qc", "commit_qc"] {
            mutateQcBody(legAt: 1, qcKey: qcKey) { body in
                body["tx_entrypoint_hash"] = secondEntrypointHash
            }
        }
    }

    nativeReceipts[0]["authority_context_height"] = authorityContextHeight
    if let receiptLaneIncarnation {
        nativeReceipts[0]["lane_incarnation"] = receiptLaneIncarnation
    }
    if let receiptProposalHash {
        nativeReceipts[0]["coordinator_proposal_hash"] = receiptProposalHash
    }
    nativeReceipts[0]["legs"] = legs

    var commitment = commitments[0]
    commitment["total_local_amount"] = "170141183460469231731687303715884105851"
    commitment["total_xor_due"] = "100.25"
    commitment["total_xor_after_haircut"] = "90.2"
    commitment["total_xor_variance"] = "10.05"
    commitment["swap_metadata"] = [
        "epsilon_bps": 25,
        "twap_window_seconds": 60,
        "liquidity_profile": ["profile": "Tier2", "state": NSNull()],
        "twap_local_per_xor": "12.5",
        "volatility_class": ["bucket": "Stable", "state": NSNull()],
    ]
    commitment["nexus_fee_receipts"] = [[
        "version": 1,
        "source_id": String(repeating: "CD", count: 32),
        "dataspace_id": 11,
        "lane_id": 7,
        "block_height": 42,
        "payer_account_id": "payer",
        "fee_asset_id": "xor#universal",
        "fee_amount": "18446744073709551616.25",
        "schedule": [
            "tx_bytes_len": 1024,
            "instruction_count": 2,
            "gas_used": 99,
            "base_fee": "1.25",
            "per_byte_fee": "0.01",
            "per_instruction_fee": "2",
            "per_gas_unit_fee": "0.125",
        ],
    ]]
    commitment["native_amx_receipts"] = nativeReceipts
    commitments[0] = commitment
    diagnostics["lane_settlement_commitments"] = commitments

    diagnostics["tx_queue_depth"] = 2
    diagnostics["tx_queue_capacity"] = 64
    diagnostics["tx_queue_retained_bytes"] = 1024
    diagnostics["tx_queue_max_retained_bytes"] = 8192
    diagnostics["tx_queue_oldest_queued_age_ms"] = 5
    let laneIncarnation = commitment["lane_incarnation"] as! String
    diagnostics["lane_relay_envelopes"] = [[
        "lane_id": 7,
        "lane_incarnation": laneIncarnation,
        "dataspace_id": 11,
        "block_height": 42,
        "block_header": [
            "height": 42,
            "prev_block_hash": NSNull(),
            "merkle_root": NSNull(),
            "result_merkle_root": NSNull(),
            "da_proof_policies_hash": NSNull(),
            "da_commitments_hash": NSNull(),
            "da_pin_intents_hash": NSNull(),
            "sccp_commitment_root": NSNull(),
            "creation_time_ms": 1_700_000_000_000,
            "view_change_index": 9,
            "confidential_features": NSNull(),
        ],
        "qc": NSNull(),
        "da_commitment_hash": NSNull(),
        "lane_block_descriptor_hash": nativeAmxTestHash(0x93),
        "settlement_commitment": commitment,
        "settlement_hash": nativeAmxTestHash(0x95),
        "rbc_bytes_total": 2048,
        "manifest_root": String(repeating: "EF", count: 32),
        "fastpq_proof": [
            "proof_digest": nativeAmxTestHash(0x97),
            "verified_at_height": 43,
        ],
    ]]
    return try JSONSerialization.data(withJSONObject: diagnostics)
}
private func mutatedNativeAmxDiagnosticsPayload(
    _ mutate: (inout [String: Any]) throws -> Void
) throws -> Data {
    guard var payload = try JSONSerialization.jsonObject(
        with: nativeAmxDiagnosticsPayload()
    ) as? [String: Any] else {
        throw NSError(domain: "SumeragiV2Fixture", code: 1)
    }
    try mutate(&payload)
    return try JSONSerialization.data(withJSONObject: payload)
}

private func mutateFirstNativeAmxLeg(
    in root: inout [String: Any],
    _ mutate: (inout [String: Any]) -> Void
) {
    var commitments = root["lane_settlement_commitments"] as! [[String: Any]]
    var receipts = commitments[0]["native_amx_receipts"] as! [[String: Any]]
    var legs = receipts[0]["legs"] as! [[String: Any]]
    mutate(&legs[0])
    receipts[0]["legs"] = legs
    commitments[0]["native_amx_receipts"] = receipts
    root["lane_settlement_commitments"] = commitments
}

private func mutateFirstNativeAmxQcBody(
    in root: inout [String: Any],
    qcKey: String,
    _ mutate: (inout [String: Any]) -> Void
) {
    mutateFirstNativeAmxLeg(in: &root) { leg in
        var qc = leg[qcKey] as! [String: Any]
        var body = qc["body"] as! [String: Any]
        mutate(&body)
        qc["body"] = body
        leg[qcKey] = qc
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
        "schema": "sorafs.chunk_fetch_plan.v1",
        "payload_digest_blake3_hex": String(repeating: "ef", count: 32),
        "chunk_fetch_specs": [[
            "chunk_index": 0,
            "offset": 0,
            "length": 4,
            "digest_blake3": String(repeating: "22", count: 32)
        ]]
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
func tcMakeClient() -> ToriiClient {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [StubURLProtocol.self]
    let session = URLSession(configuration: configuration)
    return ToriiClient(baseURL: URL(string: "https://example.test")!, session: session)
}

func tcBodyJSON(from request: URLRequest) -> [String: Any] {
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

final class ToriiClientTests: XCTestCase {
    private static let operatorSigningContext: ToriiOperatorSigningContext = {
        let signingKey = try! SigningKey.ed25519(
            privateKey: Data(repeating: 0x5A, count: 32)
        )
        return try! ToriiOperatorSigningContext(
            networkId: TestNetworkIds.canonical,
            signingKey: signingKey
        )
    }()

    private static let pipelineHash = String(repeating: "d", count: 64)
    private let canonicalSigningSeed = Data(repeating: 0x41, count: 32)
    private let onboardingToken = String(repeating: "T", count: 32)
    private let encodedRoseAssetID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
    private let roseAssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
    private let vpnRelayIdHex =
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"

    private var authority: String {
        try! Keypair(privateKeyBytes: canonicalSigningSeed)
            .accountId(networkPrefix: AccountId.defaultNetworkPrefix)
    }

    private var canonicalReadAuth: ToriiCanonicalRequestAuth {
        ToriiCanonicalRequestAuth(
            accountId: authority,
            privateKey: canonicalSigningSeed,
            timestampMs: 4_102_444_801_000,
            nonce: "canonical-read-test"
        )
    }

    private func canonicalReadAuth(accountId: String,
                                   privateKeyByte: UInt8) -> ToriiCanonicalRequestAuth {
        ToriiCanonicalRequestAuth(
            accountId: accountId,
            privateKey: Data(repeating: privateKeyByte, count: 32),
            timestampMs: 4_102_444_801_000,
            nonce: "canonical-application-post-test"
        )
    }

    private func noncanonicalStandardBase64PadBitAlias(_ encoded: String) -> String {
        toriiClientTestNoncanonicalBase64PadBitAlias(encoded)
    }

    override func tearDown() {
        StubURLProtocol.handler = nil
        super.tearDown()
    }

    private func bodyData(from request: URLRequest) -> Data? {
        toriiClientTestBodyData(from: request)
    }

    private func bodyJSON(from request: URLRequest) -> [String: Any] {
        toriiClientTestBodyJSON(from: request)
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

    private func repositoryRootURL() -> URL {
        irohaSwiftPackageRootURL().deletingLastPathComponent()
    }

    private func makeClient(
        baseURL: URL = URL(string: "https://example.test")!,
        defaultHeaders: [String: String] = [:],
        operatorSigningContext: ToriiOperatorSigningContext? = ToriiClientTests.operatorSigningContext
    ) -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        return ToriiClient(
            baseURL: baseURL,
            session: session,
            defaultHeaders: defaultHeaders,
            localSigningContext: ToriiLocalSigningContext(networkId: TestNetworkIds.canonical),
            operatorSigningContext: operatorSigningContext
        )
    }

    private func assertOperatorAuthentication(
        _ request: URLRequest,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        for header in [
            "X-Iroha-Operator-Public-Key",
            "X-Iroha-Operator-Timestamp-Ms",
            "X-Iroha-Operator-Nonce",
            "X-Iroha-Operator-Signature",
        ] {
            XCTAssertNotNil(
                request.value(forHTTPHeaderField: header),
                "missing \(header)",
                file: file,
                line: line
            )
        }
        XCTAssertTrue(request.httpBody?.isEmpty ?? true, file: file, line: line)
        XCTAssertNil(request.value(forHTTPHeaderField: "Authorization"), file: file, line: line)
        XCTAssertNil(request.value(forHTTPHeaderField: "X-API-Token"), file: file, line: line)
    }

    private func canonicalUnsignedFeePayload(
        domain: ToriiJSONValue = .object([
            "kind": .string("network"),
            "value": .string(TestNetworkIds.canonical.literal),
        ])
    ) throws -> [String: ToriiJSONValue] {
        let feePayment = try JSONDecoder().decode(
            ToriiJSONValue.self,
            from: FeePaymentIntent.authority(chargeLimits: [], gasLimit: nil)
                .canonicalJSONData()
        )
        return [
            "domain": domain,
            "authority": .string(authority),
            "creation_time_ms": .number(1),
            "instructions": .object([:]),
            "time_to_live_ms": .number(100_000),
            "fee_payment": feePayment,
            "admission_intent": .object(
                ["intent": .string("ordinary"), "value": .null]
            ),
            "metadata": .object([:]),
            "attachments": .null,
        ]
    }

    private func assertToriiInvalidPayload(
        contains expected: String,
        operation: () async throws -> Void
    ) async {
        do {
            try await operation()
            XCTFail("expected bounded Torii response failure")
        } catch ToriiClientError.invalidPayload(let message) {
            XCTAssertTrue(
                message.contains(expected),
                "expected \(expected), got \(message)"
            )
        } catch {
            XCTFail("unexpected bounded Torii response failure: \(error)")
        }
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

        let resolved = try await client.resolveAccountAlias("missing@universal")
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

    private func canonicalOwnerLiteral(
        domain: String = "wonderland",
        chainDiscriminant: UInt16 = AccountId.defaultNetworkPrefix
    ) throws -> String {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: 1, count: 32))
        let address = try AccountAddress.fromAccount(publicKey: keypair.publicKey)
        let i105 = try address.toI105(networkPrefix: chainDiscriminant)
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
                                                    verificationMode: String = "signed",
                                                    programId: String = "identifier_lookup_retail",
                                                    openingProgramId: String? = nil,
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
                verificationMode: verificationMode,
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
                programId: openingProgramId ?? programId,
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

    private func ramLfeProgramPoliciesJSON(
        programId: String = "identifier_lookup_retail",
        owner: String = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
        resolverPublicKey: String = "ed25519:resolver-key",
        backend: String = "bfv-programmed-sha3-256-v1",
        verificationMode: String = "signed",
        inputEncryption: String = "bfv-v1",
        inputEncryptionPublicParameters: String = "ABCD",
        proofBackend: String = "halo2-ipa",
        circuitId: String = "ram-lfe-v1",
        publicInputsSchemaHash: String = String(repeating: "44", count: 32),
        verifyingKeyBytesB64: String = "AQID"
    ) -> Data {
        """
        {
          "total":1,
          "items":[{
            "program_id":"\(programId)",
            "owner":"\(owner)",
            "active":true,
            "resolver_public_key":"\(resolverPublicKey)",
            "backend":"\(backend)",
            "verification_mode":"\(verificationMode)",
            "input_encryption":"\(inputEncryption)",
            "input_encryption_public_parameters":"\(inputEncryptionPublicParameters)",
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
            "proof_verifier":{
              "proof_backend":"\(proofBackend)",
              "circuit_id":"\(circuitId)",
              "public_inputs_schema_hash":"\(publicInputsSchemaHash)",
              "verifying_key_bytes_b64":"\(verifyingKeyBytesB64)"
            },
            "note":"retail programmed policy"
          }]
        }
        """.data(using: .utf8)!
    }

    private func signedIdentifierReceiptFixture(
        payload: ToriiIdentifierResolutionPayload,
        canonicalPayloadBytes: Data? = nil
    ) throws -> (resolverPublicKey: String, signatureHex: String) {
        let privateKey = Curve25519.Signing.PrivateKey()
        let multihash = CanonicalNorito.publicKeyMultihash(
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
        var writer = CompactNoritoWriter()
        writer.writeField(payload)
        return writer.data
    }

    private func legacyFlatBytesVec(_ bytes: Data) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }

    private func constVecBytes(_ bytes: Data) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(bytes.count))
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
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV/assets")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset":"66owaQmAQMuHxPzxUN3bqZ6FJfDa","account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","scope":"global","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(
            accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            asset: nil
        )
        XCTAssertEqual(balances.count, 1)
        XCTAssertEqual(balances.first?.asset, roseAssetDefinitionId)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsAsyncDecodesAssetFieldsDirectly() async throws {
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV/assets")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset":"66owaQmAQMuHxPzxUN3bqZ6FJfDa","account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","scope":"global","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(
            accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            asset: nil
        )
        XCTAssertEqual(balances.count, 1)
        guard let item = balances.first else {
            XCTFail("missing asset balance")
            return
        }
        XCTAssertEqual(item.asset, roseAssetDefinitionId)
        XCTAssertEqual(item.accountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
        XCTAssertEqual(item.scope, "global")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsAsyncDecodesReadableAssetFields() async throws {
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV/assets")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{
              "asset":"66owaQmAQMuHxPzxUN3bqZ6FJfDa",
              "account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
              "scope":"global",
              "asset_name":"USD",
              "asset_alias":"usd#issuer.main",
              "quantity":"10"
            }]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(
            accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            asset: nil
        )
        XCTAssertEqual(balances.count, 1)
        XCTAssertEqual(balances.first?.asset, roseAssetDefinitionId)
        XCTAssertEqual(balances.first?.accountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
            XCTAssertEqual(payload["alias"] as? String, "alice@universal")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = """
            {
              "alias":"alice@universal",
              "account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
              "index":7,
              "source":"world_state"
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let resolved = try await makeClient().resolveAccountAlias("alice@universal")
        XCTAssertEqual(resolved?.alias, "alice@universal")
        XCTAssertEqual(resolved?.accountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
        XCTAssertEqual(resolved?.index, 7)
        XCTAssertEqual(resolved?.source, "world_state")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testResolveRestrictedAccountAliasUsesCanonicalAuthentication() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/aliases/resolve")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(
                request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerAccount),
                try AccountAddress.parseEncoded(self.authority).canonicalHex()
            )
            XCTAssertEqual(
                request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerTimestampMs),
                "4102444801000"
            )
            XCTAssertEqual(
                request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerNonce),
                "canonical-read-test"
            )
            XCTAssertNotNil(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature))
            XCTAssertEqual(self.bodyJSON(from: request)["alias"] as? String, "merchant@private")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            let body = """
            {
              "alias":"merchant@private",
              "account_id":"\(self.authority)",
              "source":"world_state"
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        let resolved = try await makeClient().resolveAccountAlias(
            "merchant@private",
            canonicalAuth: canonicalReadAuth
        )
        XCTAssertEqual(resolved?.accountId, authority)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testPlanAliasSetupUsesCanonicalAuthenticationWithoutMutation() async throws {
        let resolved = try ResolvedAccountAliasV1(
            canonicalName: "merchant@private",
            dataspaceId: 7
        )
        let guardValue = try AliasQuoteGuardV1(
            expectedPolicyVersion: 1,
            expectedPaymentAsset: "4rPeAP6jAjiLVZThZYwwPRBuQagt",
            maxAmount: "5",
            validUntilMs: 4_102_444_802_000
        )
        let setup = try AliasSetupPlanRequestV1(
            intents: [
                EnsureAlias(
                    intent: .accountAlias(
                        try AliasAccountIntentV1(
                            alias: resolved,
                            targetAccount: authority,
                            provision: .existing,
                            role: .primary
                        )
                    ),
                    acquisition: try AliasLeaseAcquisitionV1(termYears: 1),
                    quoteGuard: guardValue
                )
            ]
        )
        let responsePlan = try AliasTransactionPlanV1(
            body: try AliasTransactionPlanBodyV1(
                authority: authority,
                networkId: TestNetworkIds.canonical,
                anchor: try AliasPlanAnchorV1(
                    blockHeight: 9,
                    blockHash: String(repeating: "01", count: 32)
                ),
                resources: [],
                instructions: [],
                totalsByAsset: [],
                warnings: [],
                blockers: [],
                validUntilMs: 4_102_444_802_000
            ),
            planHash: String(repeating: "02", count: 32)
        )
        let responseBody = try JSONEncoder().encode(responsePlan)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/aliases/setup/plan")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(
                request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerAccount),
                try AccountAddress.parseEncoded(self.authority).canonicalHex()
            )
            XCTAssertNotNil(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature))
            XCTAssertEqual(self.bodyJSON(from: request)["schema_version"] as? Int, 1)
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, responseBody)
        }

        let plan = try await makeClient().planAliasSetup(setup, canonicalAuth: canonicalReadAuth)
        XCTAssertEqual(plan, responsePlan)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testAliasLifecyclePlannersUseCanonicalAuthenticationAndExactOperation() async throws {
        let resolved = try ResolvedAccountAliasV1(
            canonicalName: "merchant@private",
            dataspaceId: 7
        )
        let guardValue = try AliasQuoteGuardV1(
            expectedPolicyVersion: 1,
            expectedPaymentAsset: "4rPeAP6jAjiLVZThZYwwPRBuQagt",
            maxAmount: "5",
            validUntilMs: 4_102_444_802_000
        )
        let renewal = try RenewAliasLease(
            target: .accountAlias(resolved),
            expectedCurrentExpiryMs: 1_000,
            targetExpiryMs: 2_000,
            quoteGuard: guardValue
        )
        let renewalRequest = AliasLeaseRenewPlanRequestV1(renewal: renewal)
        let quote = try AliasLeaseQuoteV1(
            target: renewal.target,
            pricingClass: 1,
            exactAmount: "1",
            quoteGuard: guardValue,
            expiresAtMs: renewal.targetExpiryMs,
            graceExpiresAtMs: 3_000,
            redemptionExpiresAtMs: 4_000
        )
        let renewalPlan = try AliasLifecycleTransactionPlanV1(
            body: try AliasLifecycleTransactionPlanBodyV1(
                authority: authority,
                networkId: TestNetworkIds.canonical,
                anchor: try AliasPlanAnchorV1(
                    blockHeight: 9,
                    blockHash: String(repeating: "01", count: 32)
                ),
                operation: .renewLease(renewal),
                disposition: .apply,
                instruction: try AliasFramedInstructionV1(
                    wireId: RenewAliasLease.wireId,
                    framedPayload: Data([1, 2, 3])
                ),
                quote: quote,
                totalsByAsset: [try AliasAssetTotalV1(
                    paymentAsset: guardValue.expectedPaymentAsset,
                    amount: quote.exactAmount
                )],
                warnings: [],
                blockers: [],
                validUntilMs: guardValue.validUntilMs
            ),
            planHash: String(repeating: "02", count: 32)
        )
        let renewalResponse = try JSONEncoder().encode(renewalPlan)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/aliases/lease/renew/plan")
            XCTAssertEqual(
                request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerAccount),
                try AccountAddress.parseEncoded(self.authority).canonicalHex()
            )
            XCTAssertEqual(self.bodyJSON(from: request)["schema_version"] as? Int, 1)
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, renewalResponse)
        }
        let returnedRenewalPlan = try await makeClient().planAliasLeaseRenewal(
            renewalRequest,
            canonicalAuth: canonicalReadAuth
        )
        XCTAssertEqual(returnedRenewalPlan, renewalPlan)

        let configuration = ConfigureAliasAutoRenew(
            target: .accountAlias(resolved),
            expectedRevision: 4,
            config: nil
        )
        let autoRequest = AliasAutoRenewPlanRequestV1(configuration: configuration)
        let autoPlan = try AliasLifecycleTransactionPlanV1(
            body: try AliasLifecycleTransactionPlanBodyV1(
                authority: authority,
                networkId: TestNetworkIds.canonical,
                anchor: renewalPlan.body.anchor,
                operation: .configureAutoRenew(configuration),
                disposition: .noOp,
                instruction: nil,
                quote: nil,
                totalsByAsset: [],
                warnings: [],
                blockers: [],
                validUntilMs: 4_102_444_802_000
            ),
            planHash: String(repeating: "03", count: 32)
        )
        let autoResponse = try JSONEncoder().encode(autoPlan)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/aliases/auto-renew/plan")
            XCTAssertNotNil(
                request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature)
            )
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, autoResponse)
        }
        let returnedAutoRenewPlan = try await makeClient().planAliasAutoRenew(
            autoRequest,
            canonicalAuth: canonicalReadAuth
        )
        XCTAssertEqual(returnedAutoRenewPlan, autoPlan)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testTypedAliasIndexAndByAccountReadsUseVisibilityAwareRoutes() async throws {
        XCTAssertThrowsError(
            try ToriiAliasesByAccountRequest(
                accountId: authority,
                domain: "banka"
            )
        )
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/aliases/resolve-index")
            XCTAssertNil(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature))
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (
                response,
                Data("""
                {"index":7,"alias":"merchant@paynet","account_id":"\(self.authority)","source":"active_sns"}
                """.utf8)
            )
        }
        let indexed = try await makeClient().resolveAccountAliasIndex(7)
        XCTAssertEqual(indexed?.alias, "merchant@paynet")
        XCTAssertEqual(indexed?.accountId, authority)

        let lookup = try ToriiAliasesByAccountRequest(
            accountId: authority,
            dataspace: "paynet"
        )
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/aliases/by-account")
            XCTAssertNotNil(
                request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature)
            )
            let requestBody = self.bodyJSON(from: request)
            XCTAssertEqual(requestBody["account_id"] as? String, self.authority)
            XCTAssertEqual(requestBody["dataspace"] as? String, "paynet")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (
                response,
                Data("""
                {"account_id":"\(self.authority)","total":1,"items":[{"alias":"merchant@paynet","dataspace":"paynet","is_primary":true}],"source":"fanout"}
                """.utf8)
            )
        }
        let aliases = try await makeClient().aliasesByAccount(
            lookup,
            canonicalAuth: canonicalReadAuth
        )
        XCTAssertEqual(aliases?.total, 1)
        XCTAssertEqual(aliases?.items.first?.alias, "merchant@paynet")

        let unsorted = Data("""
        {"account_id":"\(authority)","total":2,"items":[{"alias":"z@paynet","dataspace":"paynet","is_primary":false},{"alias":"a@paynet","dataspace":"paynet","is_primary":true}]}
        """.utf8)
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiAliasesByAccountResponse.self, from: unsorted)
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testTypedAliasReadsRejectSubstitutedResponseSelectors() async throws {
        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (
                response,
                Data("""
                {"alias":"other@paynet","account_id":"\(self.authority)","source":"active_sns"}
                """.utf8)
            )
        }
        await XCTAssertThrowsErrorAsync(
            try await makeClient().resolveAccountAlias("merchant@paynet")
        ) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(reason.contains("exact request"))
        }

        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (
                response,
                Data("""
                {"index":8,"alias":"merchant@paynet","account_id":"\(self.authority)","source":"active_sns"}
                """.utf8)
            )
        }
        await XCTAssertThrowsErrorAsync(
            try await makeClient().resolveAccountAliasIndex(7)
        ) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(reason.contains("exact request"))
        }

        let lookup = try ToriiAliasesByAccountRequest(
            accountId: authority,
            dataspace: "paynet",
            domain: "banka"
        )
        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (
                response,
                Data("""
                {"account_id":"\(self.authority)","total":1,"items":[{"alias":"merchant@other.paynet","dataspace":"paynet","domain":"other","is_primary":true}],"source":"fanout"}
                """.utf8)
            )
        }
        await XCTAssertThrowsErrorAsync(
            try await makeClient().aliasesByAccount(
                lookup,
                canonicalAuth: canonicalReadAuth
            )
        ) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(reason.contains("exact account and scope"))
        }
    }

    func testTypedAliasReadsRejectUnknownResponseFields() {
        let accountId = authority
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiAliasIndexResolution.self,
                from: Data("""
                {"index":7,"alias":"merchant@paynet","account_id":"\(accountId)","legacy":true}
                """.utf8)
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiAliasesByAccountResponse.self,
                from: Data("""
                {"account_id":"\(accountId)","total":0,"items":[],"legacy":true}
                """.utf8)
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiAliasesByAccountResponse.self,
                from: Data("""
                {"account_id":"\(accountId)","total":1,"items":[{"alias":"merchant@paynet","dataspace":"paynet","is_primary":true,"legacy":true}]}
                """.utf8)
            )
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testResolveAccountAliasRejectsRetiredAndUnknownResponseFields() async throws {
        let accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        for retiredField in [
            "\"accountId\":\"\(accountId)\"",
            "\"account_ids\":[\"\(accountId)\"]",
            "\"accountIds\":[\"\(accountId)\"]",
            "\"unexpected\":true",
        ] {
            StubURLProtocol.handler = { request in
                XCTAssertEqual(request.url?.path, "/v1/aliases/resolve")
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!
                let body = """
                {"alias":"alice@universal","account_id":"\(accountId)",\(retiredField)}
                """.data(using: .utf8)!
                return (response, body)
            }

            await XCTAssertThrowsErrorAsync(
                try await makeClient().resolveAccountAlias("alice@universal")
            ) { _ in }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testResolveAccountAliasRejectsNonExactResponseFields() async throws {
        let accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let canonical = """
        {
          "alias":"alice@universal",
          "account_id":"\(accountId)",
          "index":7,
          "source":"world_state"
        }
        """
        let cases = [
            (
                "account alias resolution.alias",
                "\"alias\":\"alice@universal\"",
                "\"alias\":\" alice@universal\""
            ),
            (
                "account alias resolution.account_id",
                "\"account_id\":\"\(accountId)\"",
                "\"account_id\":\" \(accountId)\""
            ),
            (
                "account alias resolution.source",
                "\"source\":\"world_state\"",
                "\"source\":\"world_state \""
            )
        ]

        for (field, needle, replacement) in cases {
            StubURLProtocol.handler = { request in
                XCTAssertEqual(request.url?.path, "/v1/aliases/resolve")
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!
                let body = canonical.replacingOccurrences(of: needle, with: replacement)
                    .data(using: .utf8)!
                return (response, body)
            }

            do {
                _ = try await makeClient().resolveAccountAlias("alice@universal")
                XCTFail("expected \(field) exactness failure")
            } catch let ToriiClientError.decoding(underlying) {
                guard case let DecodingError.dataCorrupted(context) = underlying else {
                    return XCTFail("expected dataCorrupted decode error for \(field), got \(underlying)")
                }
                XCTAssertTrue(
                    context.debugDescription.contains(field),
                    "expected \(field) failure, got \(context.debugDescription)"
                )
            } catch {
                XCTFail("expected dataCorrupted decode error for \(field), got \(error)")
            }
        }
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

        let resolved = try await makeClient().resolveAccountAlias("missing-alias@universal")
        XCTAssertNil(resolved)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListIdentifierPoliciesAsync() async throws {
        let owner = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
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
                  "max_input_bytes":32,
                  "norito_length_encoding":"u64-v1"
                },
                "ram_fhe_profile":{
                  "profile_version":1,
                  "register_count":4,
                  "memory_lane_count":32,
                  "ciphertext_mul_per_step":1,
                  "encrypted_input_mode":"encrypted_envelope_v1",
                  "min_ciphertext_modulus":1099511627776
                },
                "proof_verifier":{
                  "proof_backend":"halo2-ipa",
                  "circuit_id":"identifier-ram-lfe-v1",
                  "public_inputs_schema_hash":"\(String(repeating: "66", count: 32))",
                  "verifying_key_bytes_b64":"AQID"
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
        XCTAssertEqual(
            response.items.first?.inputEncryptionPublicParametersDecoded?.noritoLengthEncoding,
            "u64-v1"
        )
        XCTAssertEqual(response.items.first?.ramFheProfile?.profileVersion, 1)
        XCTAssertEqual(response.items.first?.ramFheProfile?.registerCount, 4)
        XCTAssertEqual(response.items.first?.ramFheProfile?.memoryLaneCount, 32)
        XCTAssertEqual(
            response.items.first?.ramFheProfile?.encryptedInputMode,
            .encryptedEnvelopeV1
        )
        XCTAssertEqual(response.items.first?.proofVerifier?.proofBackend, "halo2-ipa")
        XCTAssertEqual(response.items.first?.proofVerifier?.publicInputsSchemaHash, String(repeating: "66", count: 32))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListIdentifierPoliciesRejectsNonExactMetadata() async throws {
        let owner = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let canonical = """
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
              "max_input_bytes":32,
              "norito_length_encoding":"u64-v1"
            },
            "note":"retail phone policy"
          }]
        }
        """
        let cases = [
            (
                "identifier policy.owner",
                "\"owner\":\"\(owner)\"",
                "\"owner\":\" \(owner)\""
            ),
            (
                "identifier policy.normalization",
                "\"normalization\":\"phone_e164\"",
                "\"normalization\":\"Phone_E164\""
            ),
            (
                "identifier policy.backend",
                "\"backend\":\"bfv-affine-sha3-256-v1\"",
                "\"backend\":\"bfv-affine-sha3-256-v1 \""
            ),
            (
                "identifier policy.input_encryption",
                "\"input_encryption\":\"bfv-v1\"",
                "\"input_encryption\":\"BFV-v1\""
            ),
            (
                "identifier policy.input_encryption_public_parameters",
                "\"input_encryption_public_parameters\":\"ABCD\"",
                "\"input_encryption_public_parameters\":\" ABCD\""
            ),
            (
                "identifier policy.input_encryption_public_parameters_decoded.norito_length_encoding",
                "\"norito_length_encoding\":\"u64-v1\"",
                "\"norito_length_encoding\":\" u64-v1\""
            ),
            (
                "identifier policy.note",
                "\"note\":\"retail phone policy\"",
                "\"note\":\"retail phone policy \""
            )
        ]

        for (field, needle, replacement) in cases {
            StubURLProtocol.handler = { request in
                XCTAssertEqual(request.url?.path, "/v1/identifier-policies")
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!
                let body = canonical.replacingOccurrences(of: needle, with: replacement)
                    .data(using: .utf8)!
                return (response, body)
            }

            do {
                _ = try await makeClient().listIdentifierPolicies()
                XCTFail("expected \(field) exactness failure")
            } catch let ToriiClientError.decoding(underlying) {
                guard case let DecodingError.dataCorrupted(context) = underlying else {
                    return XCTFail("expected dataCorrupted decode error for \(field), got \(underlying)")
                }
                XCTAssertTrue(
                    context.debugDescription.contains(field),
                    "expected \(field) failure, got \(context.debugDescription)"
                )
            } catch {
                XCTFail("expected dataCorrupted decode error for \(field), got \(error)")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListIdentifierPoliciesAcceptsTaggedEncryptedInputMode() async throws {
        let owner = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
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
        let owner = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
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
        let owner = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/ram-lfe/program-policies")
            XCTAssertEqual(request.httpMethod, "GET")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let body = self.ramLfeProgramPoliciesJSON(owner: owner)
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
        XCTAssertEqual(response.items.first?.proofVerifier?.proofBackend, "halo2-ipa")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRamLfeProgramPolicyParserRejectsNonExactFieldsAsync() async throws {
        let owner = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let cases: [(field: String, body: Data)] = [
            ("ram-lfe program policy.program_id", ramLfeProgramPoliciesJSON(programId: " identifier_lookup_retail")),
            ("ram-lfe program policy.owner", ramLfeProgramPoliciesJSON(owner: "\(owner) ")),
            ("ram-lfe program policy.resolver_public_key", ramLfeProgramPoliciesJSON(resolverPublicKey: " ed25519:resolver-key")),
            ("ram-lfe program policy.backend", ramLfeProgramPoliciesJSON(backend: "BFV-programmed-sha3-256-v1")),
            ("ram-lfe program policy.verification_mode", ramLfeProgramPoliciesJSON(verificationMode: " signed")),
            ("ram-lfe program policy.input_encryption", ramLfeProgramPoliciesJSON(inputEncryption: "bfv-v1 ")),
            (
                "ram-lfe program policy.input_encryption_public_parameters",
                ramLfeProgramPoliciesJSON(inputEncryptionPublicParameters: " ABCD")
            ),
            ("ram-lfe proof verifier.proof_backend", ramLfeProgramPoliciesJSON(proofBackend: " halo2-ipa")),
            ("ram-lfe proof verifier.circuit_id", ramLfeProgramPoliciesJSON(circuitId: "ram-lfe-v1 ")),
            (
                "ram-lfe proof verifier.public_inputs_schema_hash",
                ramLfeProgramPoliciesJSON(publicInputsSchemaHash: " \(String(repeating: "44", count: 32))")
            ),
            ("ram-lfe proof verifier.verifying_key_bytes_b64", ramLfeProgramPoliciesJSON(verifyingKeyBytesB64: "AQID "))
        ]
        for testCase in cases {
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiRamLfeProgramPolicyListResponse.self, from: testCase.body)
            ) { error in
                XCTAssertTrue(
                    String(describing: error).contains(testCase.field),
                    "\(testCase.field): \(error)"
                )
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testExecuteRamLfeProgramAsync() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/ram-lfe/programs/identifier_lookup_retail/execute")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            XCTAssertEqual(
                request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerAccount),
                try AccountAddress.parseEncoded(self.authority).canonicalHex()
            )
            XCTAssertNotNil(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature))
            let payload = self.bodyJSON(from: request)
            XCTAssertNil(payload["input_hex"])
            XCTAssertEqual(payload["encrypted_input"] as? String, "abcd")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, self.ramLfeExecuteResponseJSON())
        }

        let response = try await makeClient().executeRamLfeProgram(
            programId: "identifier_lookup_retail",
            encryptedInputHex: "0xABCD",
            canonicalAuth: canonicalReadAuth
        )
        XCTAssertEqual(response?.programId, "identifier_lookup_retail")
        XCTAssertEqual(response?.outputCiphertext, "C0FFEE")
        XCTAssertEqual(response?.outputHash, String(repeating: "44", count: 32))
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
            encryptedInputHex: "ABCD",
            canonicalAuth: canonicalReadAuth
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
            XCTAssertEqual(
                request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerAccount),
                try AccountAddress.parseEncoded(self.authority).canonicalHex()
            )
            let payload = self.bodyJSON(from: request)
            XCTAssertEqual(payload["output_hex"] as? String, "c0ffee")
            let receiptObject = payload["receipt"] as? [String: Any]
            let payloadObject = receiptObject?["payload"] as? [String: Any]
            XCTAssertNotNil(payloadObject?["program_id"])
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, self.ramLfeReceiptVerifyResponseJSON())
        }

        let response = try await makeClient().verifyRamLfeReceipt(
            receipt: receipt,
            outputHex: "C0FFEE",
            canonicalAuth: canonicalReadAuth
        )
        XCTAssertTrue(response.valid)
        XCTAssertEqual(response.programId, "identifier_lookup_retail")
        XCTAssertEqual(response.outputHashMatches, true)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRamLfeResponseParsersRejectNonExactFieldsAsync() async throws {
        let executeCases: [(field: String, body: Data)] = [
            ("program_id", ramLfeExecuteResponseJSON(programId: " identifier_lookup_retail")),
            ("opaque_hash", ramLfeExecuteResponseJSON(opaqueHash: "\(String(repeating: "11", count: 32)) ")),
            ("receipt_hash", ramLfeExecuteResponseJSON(receiptHash: " \(String(repeating: "22", count: 32))")),
            ("output_ciphertext", ramLfeExecuteResponseJSON(outputCiphertext: " C0FFEE")),
            ("output_hash", ramLfeExecuteResponseJSON(outputHash: "\(String(repeating: "44", count: 32)) ")),
            ("associated_data_hash", ramLfeExecuteResponseJSON(associatedDataHash: " \(String(repeating: "55", count: 32))")),
            ("backend", ramLfeExecuteResponseJSON(backend: "BFV-programmed-sha3-256-v1")),
            ("verification_mode", ramLfeExecuteResponseJSON(verificationMode: " signed"))
        ]
        for testCase in executeCases {
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiRamLfeExecuteResponse.self, from: testCase.body)
            ) { error in
                XCTAssertTrue(
                    String(describing: error).contains("ram-lfe execute response.\(testCase.field)"),
                    "\(testCase.field): \(error)"
                )
            }
        }

        let verifyCases: [(field: String, body: Data)] = [
            ("program_id", ramLfeReceiptVerifyResponseJSON(programId: "identifier_lookup_retail ")),
            ("backend", ramLfeReceiptVerifyResponseJSON(backend: " bfv-programmed-sha3-256-v1")),
            ("verification_mode", ramLfeReceiptVerifyResponseJSON(verificationMode: "Signed")),
            ("output_hash", ramLfeReceiptVerifyResponseJSON(outputHash: " \(String(repeating: "44", count: 32))")),
            (
                "associated_data_hash",
                ramLfeReceiptVerifyResponseJSON(associatedDataHash: "\(String(repeating: "55", count: 32)) ")
            )
        ]
        for testCase in verifyCases {
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiRamLfeReceiptVerifyResponse.self, from: testCase.body)
            ) { error in
                XCTAssertTrue(
                    String(describing: error).contains("ram-lfe receipt verify response.\(testCase.field)"),
                    "\(testCase.field): \(error)"
                )
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testResolveIdentifierAsync() async throws {
        let accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
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
            XCTAssertEqual(
                request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerAccount),
                try AccountAddress.parseEncoded(self.authority).canonicalHex()
            )
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
            outputOpening: signedPayload.opening,
            canonicalAuth: canonicalReadAuth
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

    func testIdentifierReceiptDecodeRejectsPaddedAccountIdBeforeSignatureVerification() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let signed = try signedIdentifierReceiptFixture(payload: payload)
        var receiptObject = try XCTUnwrap(
            JSONSerialization.jsonObject(
                with: Data(try identifierReceiptJSON(payload: payload, signatureHex: signed.signatureHex).utf8)
            ) as? [String: Any]
        )
        var payloadObject = try XCTUnwrap(receiptObject["payload"] as? [String: Any])
        payloadObject["account_id"] = " \(accountId) "
        receiptObject["payload"] = payloadObject

        let receiptData = try JSONSerialization.data(withJSONObject: receiptObject, options: [])
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiIdentifierResolutionReceipt.self, from: receiptData)
        ) { error in
            guard case let DecodingError.dataCorrupted(context) = error else {
                XCTFail("expected dataCorrupted decode error, got \(error)")
                return
            }
            XCTAssertTrue(context.debugDescription.contains("payload.account_id"))
            XCTAssertTrue(context.debugDescription.contains("surrounding whitespace"))
        }
    }

    func testIdentifierReceiptPreservesTairaAccountRoundTrip() throws {
        let accountId = try canonicalOwnerLiteral(
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let json = try JSONEncoder().encode(payload)
        let decoded = try JSONDecoder().decode(
            ToriiIdentifierResolutionPayload.self,
            from: json
        )

        XCTAssertEqual(decoded.accountId, accountId)
        XCTAssertEqual(
            try AccountAddress.inspectI105NetworkPrefix(decoded.accountId)
                .chainDiscriminant,
            SccpV1.tairaI105DiscriminantV1
        )
        XCTAssertFalse(
            try ToriiIdentifierReceiptCanonicalEncoder.encodePayload(decoded)
                .isEmpty
        )
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
            encoded.range(of: Data([0x0E, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xFA, 0x01, 0xFB, 0x01, 0xFC]))
        )
        XCTAssertNil(
            encoded.range(of: Data([0x0B, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFA, 0xFB, 0xFC]))
        )
    }

    func testIdentifierReceiptOpeningSignatureConstVecEncodingUsesFixedU64LengthFraming() throws {
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
            Data([0x80, 0x00, 0x00, 0x00, 0x00, 0x00])
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

    func testIdentifierReceiptRejectsMalformedMlDsaAttestationLengthsBeforeBridge() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let params = MlDsaSuite.mlDsa65.parameters()
        let publicKey = Data(repeating: 0xA5, count: params.publicKeyLength)
        let publicKeyMultihash = CanonicalNorito.publicKeyMultihash(
            algorithm: .mlDsa,
            payload: publicKey
        )
        let resolverPublicKey = "ml-dsa:\(publicKeyMultihash)"
        let policy = identifierPolicy(
            owner: accountId,
            resolverPublicKey: resolverPublicKey
        )

        let shortSignature = Data(repeating: 0x11, count: params.signatureLength - 1)
        let overlongSignature = Data(repeating: 0x22, count: params.signatureLength + 1)
        let shortReceipt = try identifierReceipt(
            payload: payload,
            signatureHex: shortSignature.hexUppercased()
        )
        let overlongReceipt = try identifierReceipt(
            payload: payload,
            signatureHex: overlongSignature.hexUppercased()
        )

        XCTAssertEqual(try shortReceipt.verifyAttestation(using: policy), false)
        XCTAssertEqual(try overlongReceipt.verifyAttestation(using: policy), false)
    }

    func testIdentifierReceiptRejectsMalformedEd25519AttestationRBeforeCryptoKit() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let signed = try signedIdentifierReceiptFixture(payload: payload)
        let policy = identifierPolicy(
            owner: accountId,
            resolverPublicKey: signed.resolverPublicKey
        )
        let validReceipt = try identifierReceipt(
            payload: payload,
            signatureHex: signed.signatureHex
        )
        XCTAssertEqual(try validReceipt.verifyAttestation(using: policy), true)

        let smallOrderR = Data([
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])
        let noncanonicalR = Data([
            0xEE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F,
        ])

        for (label, replacementR) in [
            ("small-order", smallOrderR),
            ("noncanonical", noncanonicalR),
        ] {
            var signature = try XCTUnwrap(Data(hexString: signed.signatureHex))
            signature.replaceSubrange(0..<replacementR.count, with: replacementR)
            let receipt = try identifierReceipt(
                payload: payload,
                signatureHex: signature.hexUppercased()
            )

            XCTAssertEqual(try receipt.verifyAttestation(using: policy), false, label)
        }
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

    func testIdentifierReceiptRejectsMalformedEd25519ResolverPublicKeyBeforeCryptoKit() throws {
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
        XCTAssertEqual(try receipt.verifyAttestation(using: validPolicy), true)

        let smallOrderKey = Data([
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])
        let noncanonicalKey = Data([
            0xEE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F,
        ])

        for (label, rawPublicKey) in [
            ("all-zero", Data(repeating: 0, count: 32)),
            ("small-order", smallOrderKey),
            ("noncanonical", noncanonicalKey),
        ] {
            let multihash = CanonicalNorito.publicKeyMultihash(
                algorithm: .ed25519,
                payload: rawPublicKey
            )
            let malformedPolicy = identifierPolicy(
                owner: accountId,
                resolverPublicKey: "ed25519:\(multihash)"
            )
            XCTAssertThrowsError(try receipt.verifyAttestation(using: malformedPolicy), label) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    XCTFail("expected invalid payload error, got \(error)")
                    return
                }
                XCTAssertTrue(reason.contains("valid Ed25519 public key"), label)
            }
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

        XCTAssertThrowsError(try identifierReceipt(payload: payload, signatureHex: "GG")) { error in
            guard case let DecodingError.dataCorrupted(context) = error else {
                XCTFail("expected dataCorrupted decode error, got \(error)")
                return
            }
            XCTAssertTrue(context.debugDescription.contains("signature"))
            XCTAssertTrue(context.debugDescription.contains("valid hex"))
        }
    }

    func testIdentifierReceiptCanonicalPayloadRejectsNonExactExecutionTags() throws {
        let accountId = try canonicalOwnerLiteral()
        for (payload, expectedReason) in [
            (
                makeSignedIdentifierReceiptPayload(
                    policyId: " phone#retail",
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1"
                ),
                "policyId"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    policyId: "phone#retail ",
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1"
                ),
                "policyId"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    policyId: "phone #retail",
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1"
                ),
                "policyId"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    policyId: "phone# retail",
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1"
                ),
                "policyId"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1",
                    programId: " identifier_lookup_retail"
                ),
                "programId"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: " \(accountId)",
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1"
                ),
                "accountId"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: "\(accountId) ",
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1"
                ),
                "accountId"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: accountId,
                    opaqueId: " opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1"
                ),
                "opaque_id"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: "\(String(repeating: "22", count: 31))23 ",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1"
                ),
                "receipt_hash"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: " uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1"
                ),
                "uaid"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1",
                    programDigestHex: " \(String(repeating: "11", count: 32))"
                ),
                "program_digest"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1",
                    inputCiphertextHashHex: "\(String(repeating: "ab", count: 32)) "
                ),
                "input_ciphertext_hash"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1",
                    programId: "identifier_lookup_retail "
                ),
                "programId"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1",
                    openingProgramId: " identifier_lookup_retail"
                ),
                "programId"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1",
                    openingProgramId: "identifier_lookup_retail "
                ),
                "programId"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: " bfv-affine-sha3-256-v1"
                ),
                "backend"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "BFV-AFFINE-SHA3-256-V1"
                ),
                "backend"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1",
                    verificationMode: "signed "
                ),
                "verificationMode"
            ),
            (
                makeSignedIdentifierReceiptPayload(
                    accountId: accountId,
                    opaqueId: "opaque:\(String(repeating: "11", count: 32))",
                    receiptHash: String(repeating: "22", count: 31) + "23",
                    uaid: "uaid:\(String(repeating: "33", count: 31))35",
                    backend: "bfv-affine-sha3-256-v1",
                    verificationMode: "Signed"
                ),
                "verificationMode"
            ),
        ] {
            XCTAssertThrowsError(try ToriiIdentifierReceiptCanonicalEncoder.encodePayload(payload)) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    XCTFail("expected invalid payload error, got \(error)")
                    return
                }
                let normalizedReason = reason.replacingOccurrences(of: "_", with: "").lowercased()
                let normalizedExpectedReason = expectedReason.replacingOccurrences(of: "_", with: "").lowercased()
                XCTAssertTrue(normalizedReason.contains(normalizedExpectedReason), reason)
            }
        }
    }

    func testIdentifierReceiptDecodeRejectsNonExactExecutionTags() throws {
        let accountId = try canonicalOwnerLiteral()
        let payload = makeSignedIdentifierReceiptPayload(
            accountId: accountId,
            opaqueId: "opaque:\(String(repeating: "11", count: 32))",
            receiptHash: String(repeating: "22", count: 31) + "23",
            uaid: "uaid:\(String(repeating: "33", count: 31))35",
            backend: "bfv-affine-sha3-256-v1"
        )
        let payloadData = try JSONEncoder().encode(payload)
        let payloadObject = try XCTUnwrap(
            JSONSerialization.jsonObject(with: payloadData) as? [String: Any]
        )

        for policyId in [" phone#retail", "phone#retail ", "phone #retail", "phone# retail"] {
            var mutatedPayload = payloadObject
            mutatedPayload["policy_id"] = policyId
            let mutatedData = try JSONSerialization.data(
                withJSONObject: mutatedPayload,
                options: []
            )

            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiIdentifierResolutionPayload.self,
                    from: mutatedData
                )
            ) { error in
                guard case let DecodingError.dataCorrupted(context) = error else {
                    XCTFail("expected dataCorrupted decode error, got \(error)")
                    return
                }
                XCTAssertTrue(context.debugDescription.contains("policy_id"))
            }
        }

        for programId in [" identifier_lookup_retail", "identifier_lookup_retail "] {
            var mutatedPayload = payloadObject
            var execution = try XCTUnwrap(mutatedPayload["execution"] as? [String: Any])
            execution["program_id"] = programId
            mutatedPayload["execution"] = execution
            let mutatedData = try JSONSerialization.data(
                withJSONObject: mutatedPayload,
                options: []
            )

            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiIdentifierResolutionPayload.self,
                    from: mutatedData
                )
            ) { error in
                guard case let DecodingError.dataCorrupted(context) = error else {
                    XCTFail("expected dataCorrupted decode error, got \(error)")
                    return
                }
                XCTAssertTrue(context.debugDescription.contains("program_id"))
            }

            mutatedPayload = payloadObject
            var opening = try XCTUnwrap(mutatedPayload["opening"] as? [String: Any])
            var openingPayload = try XCTUnwrap(opening["payload"] as? [String: Any])
            openingPayload["program_id"] = programId
            opening["payload"] = openingPayload
            mutatedPayload["opening"] = opening
            let mutatedOpeningData = try JSONSerialization.data(
                withJSONObject: mutatedPayload,
                options: []
            )

            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiIdentifierResolutionPayload.self,
                    from: mutatedOpeningData
                )
            ) { error in
                guard case let DecodingError.dataCorrupted(context) = error else {
                    XCTFail("expected dataCorrupted decode error, got \(error)")
                    return
                }
                XCTAssertTrue(context.debugDescription.contains("program_id"))
            }
        }

        for paddedAccountId in [" \(accountId)", "\(accountId) "] {
            var mutatedPayload = payloadObject
            mutatedPayload["account_id"] = paddedAccountId
            let mutatedData = try JSONSerialization.data(
                withJSONObject: mutatedPayload,
                options: []
            )

            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiIdentifierResolutionPayload.self,
                    from: mutatedData
                )
            ) { error in
                guard case let DecodingError.dataCorrupted(context) = error else {
                    XCTFail("expected dataCorrupted decode error, got \(error)")
                    return
                }
                XCTAssertTrue(context.debugDescription.contains("payload.account_id"))
                XCTAssertTrue(context.debugDescription.contains("surrounding whitespace"))
            }
        }

        let hashExactnessCases: [(path: [String], value: String, reason: String)] = [
            (["opaque_id"], " \(try XCTUnwrap(payloadObject["opaque_id"] as? String))", "opaque_id"),
            (["receipt_hash"], "\(try XCTUnwrap(payloadObject["receipt_hash"] as? String)) ", "receipt_hash"),
            (["uaid"], " \(try XCTUnwrap(payloadObject["uaid"] as? String))", "uaid"),
            (
                ["execution", "program_digest"],
                " \(try XCTUnwrap(try XCTUnwrap(payloadObject["execution"] as? [String: Any])["program_digest"] as? String))",
                "program_digest"
            ),
            (
                ["opening", "payload", "input_ciphertext_hash"],
                "\(try XCTUnwrap(try XCTUnwrap(try XCTUnwrap(payloadObject["opening"] as? [String: Any])["payload"] as? [String: Any])["input_ciphertext_hash"] as? String)) ",
                "input_ciphertext_hash"
            ),
        ]
        for testCase in hashExactnessCases {
            var mutatedPayload = payloadObject
            if testCase.path.count == 1 {
                mutatedPayload[testCase.path[0]] = testCase.value
            } else if testCase.path == ["execution", "program_digest"] {
                var execution = try XCTUnwrap(mutatedPayload["execution"] as? [String: Any])
                execution["program_digest"] = testCase.value
                mutatedPayload["execution"] = execution
            } else {
                var opening = try XCTUnwrap(mutatedPayload["opening"] as? [String: Any])
                var openingPayload = try XCTUnwrap(opening["payload"] as? [String: Any])
                openingPayload["input_ciphertext_hash"] = testCase.value
                opening["payload"] = openingPayload
                mutatedPayload["opening"] = opening
            }
            let mutatedData = try JSONSerialization.data(
                withJSONObject: mutatedPayload,
                options: []
            )
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiIdentifierResolutionPayload.self,
                    from: mutatedData
                )
            ) { error in
                if case let DecodingError.dataCorrupted(context) = error {
                    XCTAssertTrue(context.debugDescription.contains(testCase.reason))
                } else if case let ToriiClientError.invalidPayload(reason) = error {
                    XCTAssertTrue(reason.contains(testCase.reason))
                } else {
                    XCTFail("expected exactness decode error, got \(error)")
                }
            }
        }

        var emptyAccountPayload = payloadObject
        emptyAccountPayload["account_id"] = " \n\t "
        let emptyAccountData = try JSONSerialization.data(
            withJSONObject: emptyAccountPayload,
            options: []
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiIdentifierResolutionPayload.self,
                from: emptyAccountData
            )
        ) { error in
            guard case let DecodingError.dataCorrupted(context) = error else {
                XCTFail("expected dataCorrupted decode error, got \(error)")
                return
            }
            XCTAssertTrue(context.debugDescription.contains("account_id"))
        }

        for (field, value) in [
            ("backend", " bfv-affine-sha3-256-v1"),
            ("backend", "BFV-AFFINE-SHA3-256-V1"),
            ("verification_mode", "signed "),
            ("verification_mode", "Signed"),
        ] {
            var mutatedPayload = payloadObject
            var execution = try XCTUnwrap(mutatedPayload["execution"] as? [String: Any])
            execution[field] = value
            mutatedPayload["execution"] = execution
            let mutatedData = try JSONSerialization.data(
                withJSONObject: mutatedPayload,
                options: []
            )

            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiIdentifierResolutionPayload.self,
                    from: mutatedData
                )
            ) { error in
                guard case let DecodingError.dataCorrupted(context) = error else {
                    XCTFail("expected dataCorrupted decode error, got \(error)")
                    return
                }
                XCTAssertTrue(context.debugDescription.contains(field))
            }
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

    func testIdentifierReceiptRejectsPaddedProofAttestationBackendDuringDecode() throws {
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

        for proofBackend in [" ram-lfe-v1", "ram-lfe-v1 "] {
            let receiptJSON = """
            {
              "payload":\(payloadJSON),
              "attestation":{
                "kind":"proof",
                "proof_backend":"\(proofBackend)",
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
                XCTAssertTrue(context.debugDescription.contains("proof_backend"))
            }
        }
    }

    func testIdentifierReceiptRejectsNonExactAttestationKindDuringDecode() throws {
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

        for kind in [" signed", "signed ", "Signed"] {
            let receiptJSON = """
            {
              "payload":\(payloadJSON),
              "attestation":{
                "kind":"\(kind)",
                "signature":"A1B2C3D4"
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
                XCTAssertTrue(context.debugDescription.contains("kind"))
            }
        }
    }

    func testIdentifierReceiptRejectsPaddedPayloadAccountIdDuringDecode() throws {
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

        for paddedAccountId in [" \(accountId)", "\(accountId) "] {
            let paddedPayloadJSON = payloadJSON.replacingOccurrences(
                of: "\"account_id\":\"\(accountId)\"",
                with: "\"account_id\":\"\(paddedAccountId)\""
            )
            let receiptJSON = """
            {
              "payload":\(paddedPayloadJSON),
              "attestation":{
                "kind":"signed",
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
                XCTAssertTrue(context.debugDescription.contains("payload.account_id"))
                XCTAssertTrue(context.debugDescription.contains("surrounding whitespace"))
            }
        }
    }

    func testIdentifierReceiptRejectsMalformedProofAttestationBase64DuringDecode() throws {
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
            "kind":"proof",
            "proof_backend":"ram-lfe-v1",
            "proof_b64":"@@@"
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
            XCTAssertTrue(context.debugDescription.contains("proof_b64"))
        }
    }

    func testIdentifierReceiptRejectsPaddedProofAttestationBase64DuringDecode() throws {
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

        for proofB64 in [" AAAA", "AAAA "] {
            let receiptJSON = """
            {
              "payload":\(payloadJSON),
              "attestation":{
                "kind":"proof",
                "proof_backend":"ram-lfe-v1",
                "proof_b64":"\(proofB64)"
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
                XCTAssertTrue(context.debugDescription.contains("proof_b64"))
            }
        }
    }

    func testIdentifierReceiptVerifierMatchesSharedReceiptVectors() throws {
        let fixtureURL = repositoryRootURL()
            .appendingPathComponent("fixtures/soracloud/identifier_receipt_vectors_v1.json")
        let fixtureData = try Data(contentsOf: fixtureURL)
        let fixture = try XCTUnwrap(
            JSONSerialization.jsonObject(with: fixtureData) as? [String: Any]
        )
        XCTAssertEqual(try string(fixture, "vector_set"), "identifier-receipt-attestation-v1")
        let receiptObject = try object(fixture, "receipt")
        let policyObject = try object(fixture, "policy")
        let receipt = try identifierReceipt(fromFixture: receiptObject)
        let policy = try identifierReceiptPolicy(fromFixture: policyObject)

        let payloadBytes = try ToriiIdentifierReceiptCanonicalEncoder.encodePayload(receipt.payload)
        XCTAssertEqual(sha256Hex(payloadBytes), try string(fixture, "canonical_payload_sha256"))
        XCTAssertEqual(try receipt.verifyAttestation(using: policy), true)

        for vector in try objectArray(fixture, "attestation_vectors") {
            let name = try string(vector, "name")
            let attestationObject = try object(vector, "attestation")
            let attestation = try identifierReceiptAttestation(fromFixture: attestationObject)
            let encoded = try ToriiIdentifierReceiptCanonicalEncoder.encodeAttestation(attestation)
            XCTAssertEqual(
                encoded.count,
                try int(vector, "expected_attestation_bytes"),
                "\(name) attestation byte length"
            )
            XCTAssertEqual(
                sha256Hex(encoded),
                try string(vector, "expected_attestation_sha256"),
                "\(name) attestation digest"
            )
            if attestation.kind == "proof" {
                var proofReceiptObject = receiptObject
                proofReceiptObject["attestation"] = attestationObject
                let proofReceipt = try identifierReceipt(fromFixture: proofReceiptObject)
                XCTAssertThrowsError(
                    try proofReceipt.verifyAttestation(using: policy),
                    "\(name) proof verifier gate"
                ) { error in
                    guard case let ToriiClientError.invalidPayload(reason) = error else {
                        XCTFail("expected invalid payload error, got \(error)")
                        return
                    }
                    XCTAssertTrue(reason.contains("Only signed identifier receipt attestations"))
                }
            }
        }

        for negative in try objectArray(fixture, "negative_cases") {
            let negativeName = try string(negative, "name")
            let mutation = try string(negative, "mutation")
            var mutatedReceiptObject = receiptObject
            var mutatedPolicyObject = policyObject
            switch mutation {
            case "receipt.payload.execution.output_ciphertext_hash":
                var payload = try object(mutatedReceiptObject, "payload")
                var execution = try object(payload, "execution")
                execution["output_ciphertext_hash"] = try string(negative, "value")
                payload["execution"] = execution
                mutatedReceiptObject["payload"] = payload
            case "policy.resolver_public_key":
                mutatedPolicyObject["resolver_public_key"] = try string(negative, "value")
            case "policy.policy_id":
                mutatedPolicyObject["policy_id"] = try string(negative, "value")
            case "receipt.attestation.signature":
                var attestation = try object(mutatedReceiptObject, "attestation")
                attestation["signature"] = try string(negative, "value")
                mutatedReceiptObject["attestation"] = attestation
            case "receipt.attestation":
                mutatedReceiptObject["attestation"] = try object(negative, "value")
            default:
                XCTFail("Unhandled receipt vector mutation \(mutation)")
                continue
            }

            if let expectedError = negative["expected_error_contains"] as? String {
                do {
                    let mutatedReceipt = try identifierReceipt(fromFixture: mutatedReceiptObject)
                    let mutatedPolicy = try identifierReceiptPolicy(fromFixture: mutatedPolicyObject)
                    XCTAssertThrowsError(
                        try mutatedReceipt.verifyAttestation(using: mutatedPolicy),
                        negativeName
                    )
                } catch let DecodingError.dataCorrupted(context) {
                    XCTAssertTrue(
                        context.debugDescription.localizedCaseInsensitiveContains(expectedError),
                        negativeName
                    )
                }
            } else {
                let mutatedReceipt = try identifierReceipt(fromFixture: mutatedReceiptObject)
                let mutatedPolicy = try identifierReceiptPolicy(fromFixture: mutatedPolicyObject)
                XCTAssertEqual(
                    try mutatedReceipt.verifyAttestation(using: mutatedPolicy),
                    try bool(negative, "expected_result"),
                    negativeName
                )
            }
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

    func testIdentifierReceiptRejectsPaddedSignedAttestationSignatureDuringDecode() throws {
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
            "signature":" \(signed.signatureHex)"
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
            XCTAssertTrue(context.debugDescription.contains("signature must be exact"))
        }
    }

    func testIdentifierReceiptRejectsPaddedOpeningSignatureDuringDecode() throws {
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
        ).replacingOccurrences(
            of: "\"signature\":\"\(payload.opening.signature)\"",
            with: "\"signature\":\"\(payload.opening.signature) \""
        )
        let receiptJSON = """
        {
          "payload":\(payloadJSON),
          "attestation":{
            "kind":"signed",
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
            XCTAssertTrue(context.debugDescription.contains("opening.signature must be exact"))
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
                outputOpening: signedPayload.opening,
                canonicalAuth: canonicalReadAuth
            )
            XCTFail("Expected invalidPayload error")
        } catch let ToriiClientError.invalidPayload(reason) {
            XCTAssertTrue(reason.contains("claim-receipt decode failed"))
            XCTAssertTrue(reason.contains("body_len="))
            XCTAssertTrue(reason.contains("body_sha256="))
            XCTAssertFalse(reason.contains(accountId))
            XCTAssertFalse(reason.contains(opaqueId))
            XCTAssertFalse(reason.contains("signature_payload_hex"))
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
            outputOpening: sampleOpening(),
            canonicalAuth: canonicalReadAuth
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
            outputOpening: signedPayload.opening,
            canonicalAuth: canonicalReadAuth
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
            outputOpening: signedPayload.opening,
            canonicalAuth: canonicalReadAuth(accountId: accountId, privateKeyByte: 1)
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
            outputOpening: signedPayload.opening,
            canonicalAuth: canonicalReadAuth(accountId: accountId, privateKeyByte: 1)
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
    func testIssueIdentifierClaimReceiptRejectsPathAccountSubstitutionBeforeDispatch() async throws {
        let accountId = try canonicalOwnerLiteral()
        StubURLProtocol.handler = { _ in
            XCTFail("path/account substitution must fail before dispatch")
            throw URLError(.badURL)
        }

        await assertToriiInvalidPayload(contains: "claim-receipt path accountId") {
            _ = try await makeClient().issueIdentifierClaimReceipt(
                accountId: accountId,
                policyId: "phone#retail",
                encryptedInputHex: "ABCD",
                outputOpening: sampleOpening(),
                canonicalAuth: canonicalReadAuth
            )
        }
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

    @available(iOS 15.0, macOS 12.0, *)
    func testGetIdentifierClaimByReceiptHashRejectsNonExactClaimFieldsAsync() async throws {
        let accountId = try canonicalOwnerLiteral()
        let receiptHash = String(repeating: "55", count: 32)
        let opaqueId = "opaque:\(String(repeating: "44", count: 32))"
        let uaid = "uaid:\(String(repeating: "66", count: 31))67"

        func claimRecordJSON(policyId: String = "phone#retail",
                             opaqueId: String = opaqueId,
                             receiptHash: String = receiptHash,
                             uaid: String = uaid,
                             accountId: String = accountId) -> Data {
            """
            {
              "policy_id":"\(policyId)",
              "opaque_id":"\(opaqueId)",
              "receipt_hash":"\(receiptHash)",
              "uaid":"\(uaid)",
              "account_id":"\(accountId)",
              "verified_at_ms":7
            }
            """.data(using: .utf8)!
        }

        let cases: [(field: String, body: Data)] = [
            ("policy_id", claimRecordJSON(policyId: " phone#retail")),
            ("opaque_id", claimRecordJSON(opaqueId: "\(opaqueId) ")),
            ("receipt_hash", claimRecordJSON(receiptHash: " \(receiptHash)")),
            ("uaid", claimRecordJSON(uaid: "\(uaid) ")),
            ("account_id", claimRecordJSON(accountId: " \(accountId)")),
        ]

        for testCase in cases {
            StubURLProtocol.handler = { request in
                XCTAssertEqual(
                    request.url?.path,
                    "/v1/identifiers/receipts/\(receiptHash)"
                )
                XCTAssertEqual(request.httpMethod, "GET")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, testCase.body)
            }

            do {
                _ = try await makeClient().getIdentifierClaimByReceiptHash(receiptHash)
                XCTFail("expected non-exact claim record \(testCase.field) to fail")
            } catch {
                XCTAssertTrue(
                    String(describing: error).contains("identifier claim record.\(testCase.field)"),
                    "\(testCase.field): \(error)"
                )
            }
        }
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
            "4e52543000001042e5b988077612440e4cd45673596b00b004000000000000dd479e32bf99dbd000a804000000000000040000000000000020010000000000008800000000000000080000000000000008000000000000002dac6c00000000000800000000000000440e92000000000008000000000000005b2600000000000008000000000000004a681100000000000800000000000000bc3d2300000000000800000000000000413e85000000000008000000000000005619f900000000000800000000000000bd73fc0000000000880000000000000008000000000000000800000000000000ee894300000000000800000000000000dd22b000000000000800000000000000fe7c50000000000008000000000000001639a3000000000008000000000000006a969b00000000000800000000000000ddd4410000000000080000000000000051076600000000000800000000000000ef14ae00000000002001000000000000880000000000000008000000000000000800000000000000d86c690000000000080000000000000093070e0000000000080000000000000033067500000000000800000000000000ddc5190000000000080000000000000062ea230000000000080000000000000056f00a00000000000800000000000000ab51d400000000000800000000000000e945790000000000880000000000000008000000000000000800000000000000f2204400000000000800000000000000c9ecd2000000000008000000000000001dfc5b00000000000800000000000000d16d660000000000080000000000000016ec0e000000000008000000000000003def83000000000008000000000000006e7ff900000000000800000000000000c1fabb00000000002001000000000000880000000000000008000000000000000800000000000000c8c6eb00000000000800000000000000c9c14800000000000800000000000000f01f8700000000000800000000000000aed22c000000000008000000000000006122990000000000080000000000000036ad8c00000000000800000000000000d1429300000000000800000000000000891f6d0000000000880000000000000008000000000000000800000000000000417eed00000000000800000000000000d79c34000000000008000000000000009f322c0000000000080000000000000091fe5700000000000800000000000000533ce8000000000008000000000000005db8df00000000000800000000000000a8c313000000000008000000000000006e03c20000000000200100000000000088000000000000000800000000000000080000000000000003d654000000000008000000000000005d884400000000000800000000000000567ab50000000000080000000000000007273100000000000800000000000000ff6d0a00000000000800000000000000077466000000000008000000000000006d1d1a000000000008000000000000007050c200000000008800000000000000080000000000000008000000000000002f884f0000000000080000000000000041b0a100000000000800000000000000cbfa290000000000080000000000000057477300000000000800000000000000608f9200000000000800000000000000f5f5dd00000000000800000000000000445b3b00000000000800000000000000999e690000000000"

        XCTAssertEqual(try policy.encryptInput("ab", seedHex: seedHex), expected)
        let request = try policy.encryptedRequest(
            input: "ab",
            outputOpening: sampleOpening(),
            seedHex: seedHex
        )
        XCTAssertEqual(request.policyId, "string#retail")
        XCTAssertEqual(request.encryptedInputHex, expected)
    }

    func testIdentifierBfvEnvelopeBuilderRejectsOverwideInputProfile() throws {
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
                maxInputBytes: 64
            ),
            ramFheProfile: nil,
            proofVerifier: nil,
            note: nil
        )

        XCTAssertThrowsError(
            try policy.encryptInput(
                "ab",
                seedHex: "00112233445566778899AABBCCDDEEFF00112233445566778899AABBCCDDEEFF"
            )
        ) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(reason.contains("registered RAM-LFE"))
        }
    }

    func testIdentifierBfvEnvelopeBuilderMatchesSharedSoracloudVectors() throws {
        let fixtureURL = repositoryRootURL()
            .appendingPathComponent("fixtures/soracloud/bfv_identifier_vectors_v1.json")
        let fixtureData = try Data(contentsOf: fixtureURL)
        let fixture = try XCTUnwrap(
            JSONSerialization.jsonObject(with: fixtureData) as? [String: Any]
        )
        XCTAssertEqual(try string(fixture, "vector_set"), "soracloud-bfv-identifier-envelope-v1")
        try assertBfvOperationKeyComponentVectors(try object(fixture, "operation_vectors"))
        let policy = try bfvPolicy(fromFixture: object(fixture, "policy"))
        let vectors = try objectArray(fixture, "vectors")
        var observedDigests = Set<String>()

        for vector in vectors {
            let vectorName = try string(vector, "name")
            let ciphertextHex = try policy.encryptInput(
                string(vector, "input_utf8"),
                seedHex: string(vector, "seed_hex")
            )
            XCTAssertEqual(
                ciphertextHex.count / 2,
                try int(vector, "expected_ciphertext_bytes"),
                "\(vectorName) ciphertext byte length"
            )
            let ciphertext = try XCTUnwrap(Data(hexString: ciphertextHex))
            let digest = sha256Hex(ciphertext)
            XCTAssertEqual(
                digest,
                try string(vector, "expected_ciphertext_sha256"),
                "\(vectorName) ciphertext digest"
            )
            XCTAssertTrue(
                observedDigests.insert(digest).inserted,
                "fixture ciphertext digest must be unique: \(digest)"
            )
        }
    }

    func testIdentifierBfvEnvelopeBuilderMatchesSharedSoracloudOperationInputVectors() throws {
        let fixtureURL = repositoryRootURL()
            .appendingPathComponent("fixtures/soracloud/bfv_identifier_vectors_v1.json")
        let fixtureData = try Data(contentsOf: fixtureURL)
        let fixture = try XCTUnwrap(
            JSONSerialization.jsonObject(with: fixtureData) as? [String: Any]
        )
        let operationVectors = try object(fixture, "operation_vectors")
        XCTAssertEqual(try string(operationVectors, "vector_set"), "soracloud-bfv-operation-v1")
        let policy = ToriiIdentifierPolicySummary(
            policyId: "soracloud-operation#fixture",
            owner: try canonicalOwnerLiteral(),
            active: true,
            normalization: .exact,
            resolverPublicKey: "ed25519:ed0120" + String(repeating: "11", count: 32),
            backend: "bfv-programmed-sha3-256-v1",
            inputEncryption: "bfv-v1",
            inputEncryptionPublicParameters: nil,
            inputEncryptionPublicParametersDecoded: try bfvParameters(
                fromFixture: object(operationVectors, "public_parameters_decoded")
            ),
            ramFheProfile: nil,
            proofVerifier: nil,
            note: nil
        )
        var checkedInputs = 0
        var observedDigests = Set<String>()
        for vector in try objectArray(operationVectors, "vectors") {
            let vectorName = try string(vector, "name")
            for input in try objectArray(vector, "inputs") {
                if input["packed_slots"] != nil {
                    continue
                }
                let seedUtf8 = try string(input, "seed_utf8")
                let inputBytes = try XCTUnwrap(Data(hexString: try string(input, "input_hex")))
                let inputString = String(decoding: inputBytes, as: UTF8.self)
                let seed = Data(seedUtf8.utf8).hexEncodedString()
                let ciphertextHex = try policy.encryptInput(inputString, seedHex: seed)
                XCTAssertEqual(
                    ciphertextHex.count / 2,
                    try int(input, "expected_ciphertext_bytes"),
                    "\(vectorName)/\(seedUtf8) ciphertext byte length"
                )
                let ciphertext = try XCTUnwrap(Data(hexString: ciphertextHex))
                let digest = sha256Hex(ciphertext)
                XCTAssertEqual(
                    digest,
                    try string(input, "expected_ciphertext_sha256"),
                    "\(vectorName)/\(seedUtf8) ciphertext digest"
                )
                XCTAssertTrue(
                    observedDigests.insert(digest).inserted,
                    "operation input digest must be unique: \(digest)"
                )
                checkedInputs += 1
            }
        }
        XCTAssertEqual(checkedInputs, 8)
    }

    func testSharedSoracloudBfvKeyBundleComponentVectorsAreComplete() throws {
        let fixture = try loadSharedBfvFixture()

        try assertBfvOperationKeyComponentVectors(try object(fixture, "operation_vectors"))
    }

    func testSharedSoracloudBfvKeyBundleComponentVectorsRejectAdversarialDrift() throws {
        var missingComponent = try object(loadSharedBfvFixture(), "operation_vectors")
        var missingEvaluationKey = try object(missingComponent, "evaluation_key_bundle")
        var missingEntries = try objectArray(missingEvaluationKey, "relinearization_entries")
        missingEntries[0].removeValue(forKey: "b_sha256")
        missingEvaluationKey["relinearization_entries"] = missingEntries
        missingComponent["evaluation_key_bundle"] = missingEvaluationKey
        XCTAssertThrowsError(try assertBfvOperationKeyComponentVectors(missingComponent))

        var duplicateComponent = try object(loadSharedBfvFixture(), "operation_vectors")
        var duplicateEvaluationKey = try object(duplicateComponent, "evaluation_key_bundle")
        var duplicateEntries = try objectArray(duplicateEvaluationKey, "relinearization_entries")
        duplicateEntries[1]["a_sha256"] = try string(duplicateEntries[0], "b_sha256")
        duplicateEvaluationKey["relinearization_entries"] = duplicateEntries
        duplicateComponent["evaluation_key_bundle"] = duplicateEvaluationKey
        XCTAssertThrowsError(try assertBfvOperationKeyComponentVectors(duplicateComponent))

        var lowercaseComponent = try object(loadSharedBfvFixture(), "operation_vectors")
        var lowercaseEvaluationKey = try object(lowercaseComponent, "evaluation_key_bundle")
        var lowercaseEntries = try objectArray(lowercaseEvaluationKey, "relinearization_entries")
        lowercaseEntries[0]["b_sha256"] = try string(lowercaseEntries[0], "b_sha256").lowercased()
        lowercaseEvaluationKey["relinearization_entries"] = lowercaseEntries
        lowercaseComponent["evaluation_key_bundle"] = lowercaseEvaluationKey
        XCTAssertThrowsError(try assertBfvOperationKeyComponentVectors(lowercaseComponent))

        var zeroRefresh = try object(loadSharedBfvFixture(), "operation_vectors")
        var rotationKeys = try objectArray(zeroRefresh, "rotation_keys")
        var firstRotation = rotationKeys[0]
        var refreshComponents = try object(firstRotation, "zero_refresh_components")
        refreshComponents["c1_sha256"] = String(repeating: "0", count: 64)
        firstRotation["zero_refresh_components"] = refreshComponents
        rotationKeys[0] = firstRotation
        zeroRefresh["rotation_keys"] = rotationKeys
        XCTAssertThrowsError(try assertBfvOperationKeyComponentVectors(zeroRefresh))

        var countDrift = try object(loadSharedBfvFixture(), "operation_vectors")
        var bootstrap = try object(countDrift, "bootstrap_key")
        var bootstrapComponents = try object(bootstrap, "zero_refresh_components")
        bootstrapComponents["coefficient_count"] = 63
        bootstrap["zero_refresh_components"] = bootstrapComponents
        countDrift["bootstrap_key"] = bootstrap
        XCTAssertThrowsError(try assertBfvOperationKeyComponentVectors(countDrift))

        var rotationCountDrift = try object(loadSharedBfvFixture(), "operation_vectors")
        var rotationEvaluationKey = try object(rotationCountDrift, "evaluation_key_bundle")
        rotationEvaluationKey["rotation_key_count"] = 99
        rotationCountDrift["evaluation_key_bundle"] = rotationEvaluationKey
        XCTAssertThrowsError(try assertBfvOperationKeyComponentVectors(rotationCountDrift))

        var missingFullBootstrapMaterial = try object(loadSharedBfvFixture(), "operation_vectors")
        missingFullBootstrapMaterial.removeValue(forKey: "full_bootstrap_material")
        XCTAssertThrowsError(try assertBfvOperationKeyComponentVectors(missingFullBootstrapMaterial))

        var verifierCommitmentDrift = try object(loadSharedBfvFixture(), "operation_vectors")
        var driftedMaterial = try object(verifierCommitmentDrift, "full_bootstrap_material")
        driftedMaterial["vk_commitment_hex"] = try string(driftedMaterial, "expected_material_digest_hex")
        verifierCommitmentDrift["full_bootstrap_material"] = driftedMaterial
        XCTAssertThrowsError(try assertBfvOperationKeyComponentVectors(verifierCommitmentDrift))

        var noncanonicalMaterialDigest = try object(loadSharedBfvFixture(), "operation_vectors")
        var noncanonicalMaterial = try object(noncanonicalMaterialDigest, "full_bootstrap_material")
        noncanonicalMaterial["expected_material_digest_hex"] = try string(noncanonicalMaterial, "expected_material_digest_hex").uppercased()
        noncanonicalMaterialDigest["full_bootstrap_material"] = noncanonicalMaterial
        XCTAssertThrowsError(try assertBfvOperationKeyComponentVectors(noncanonicalMaterialDigest))
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
                {"payload":{"entrypoint_hash":"abc","signed_transaction_hash":null,"submitted_at_ms":1,"submitted_at_height":2,"signer":"signer"},"signature":"deadbeef"}
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

        XCTAssertEqual(receipt.hash, "entry")
        XCTAssertEqual(receipt.payload.entrypointHash, "entry")
        XCTAssertEqual(receipt.payload.signedTransactionHash, "signed")
        XCTAssertEqual(receipt.payload.signerValue["algorithm"], .string("ed25519"))
        XCTAssertEqual(receipt.payload.signerValue["payload"], .string("node-key"))
        XCTAssertEqual(receipt.signatureValue["algorithm"], .string("ed25519"))
        XCTAssertEqual(receipt.signatureValue["payload"], .string("receipt-signature"))
        XCTAssertEqual(receipt.payload.signer, #"{"algorithm":"ed25519","payload":"node-key"}"#)
        XCTAssertEqual(receipt.signature, #"{"algorithm":"ed25519","payload":"receipt-signature"}"#)

        let missingNullableHash = """
        {"payload":{"entrypoint_hash":"entry","submitted_at_ms":1,"submitted_at_height":2,"signer":"node"},"signature":"sig"}
        """.data(using: .utf8)!
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSubmitTransactionResponse.self, from: missingNullableHash)
        )

        let unknownPayloadField = """
        {"payload":{"entrypoint_hash":"entry","signed_transaction_hash":null,"submitted_at_ms":1,"submitted_at_height":2,"signer":"node","legacy":null},"signature":"sig"}
        """.data(using: .utf8)!
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSubmitTransactionResponse.self, from: unknownPayloadField)
        )
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
                {"payload":{"entrypoint_hash":"json","signed_transaction_hash":null,"submitted_at_ms":1,"submitted_at_height":2,"signer":"json-signer"},"signature":"cafe"}
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

    private func sccpAbiWord(_ value: UInt32) -> Data {
        var out = Data(repeating: 0, count: 32)
        out[28] = UInt8((value >> 24) & 0xff)
        out[29] = UInt8((value >> 16) & 0xff)
        out[30] = UInt8((value >> 8) & 0xff)
        out[31] = UInt8(value & 0xff)
        return out
    }

    private func hexString(_ data: Data) -> String {
        data.map { String(format: "%02x", $0) }.joined()
    }

    private func sha256Hex(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02X", $0) }.joined()
    }

    private func matchingVerifyingKeyCommitmentHex(backend: String, bytes: Data) -> String {
        let backendBytes = Data(backend.utf8)
        var preimage = Data("iroha:zk:v1:vk".utf8)
        preimage.append(u64BigEndianData(UInt64(backendBytes.count)))
        preimage.append(backendBytes)
        preimage.append(u64BigEndianData(UInt64(bytes.count)))
        preimage.append(bytes)
        return Data(SHA256.hash(data: preimage)).hexEncodedString()
    }

    private func expectedVerifierRegistryBackendRejection(_ backend: String) -> String {
        backend.trimmingCharacters(in: .whitespacesAndNewlines) == backend
            ? "backend is not an exact supported verifier-registry label: \(backend)."
            : "backend must not contain surrounding whitespace."
    }

    private func u64BigEndianData(_ value: UInt64) -> Data {
        var bigEndian = value.bigEndian
        return withUnsafeBytes(of: &bigEndian) { Data($0) }
    }

    private func loadSharedBfvFixture() throws -> [String: Any] {
        let fixtureURL = repositoryRootURL()
            .appendingPathComponent("fixtures/soracloud/bfv_identifier_vectors_v1.json")
        let fixtureData = try Data(contentsOf: fixtureURL)
        return try XCTUnwrap(
            JSONSerialization.jsonObject(with: fixtureData) as? [String: Any]
        )
    }

    private func identifierReceiptPolicy(
        fromFixture policy: [String: Any]
    ) throws -> ToriiIdentifierPolicySummary {
        ToriiIdentifierPolicySummary(
            policyId: try string(policy, "policy_id"),
            owner: try string(policy, "owner"),
            active: try bool(policy, "active"),
            normalization: .phoneE164,
            resolverPublicKey: try string(policy, "resolver_public_key"),
            backend: try string(policy, "backend"),
            inputEncryption: policy["input_encryption"] as? String,
            inputEncryptionPublicParameters: nil,
            inputEncryptionPublicParametersDecoded: nil,
            ramFheProfile: nil,
            proofVerifier: nil,
            note: nil
        )
    }

    private func identifierReceipt(
        fromFixture receipt: [String: Any]
    ) throws -> ToriiIdentifierResolutionReceipt {
        let data = try JSONSerialization.data(withJSONObject: receipt)
        return try JSONDecoder().decode(ToriiIdentifierResolutionReceipt.self, from: data)
    }

    private func identifierReceiptAttestation(
        fromFixture attestation: [String: Any]
    ) throws -> ToriiIdentifierReceiptAttestation {
        let data = try JSONSerialization.data(withJSONObject: attestation)
        return try JSONDecoder().decode(ToriiIdentifierReceiptAttestation.self, from: data)
    }

    private func bfvPolicy(fromFixture policy: [String: Any]) throws -> ToriiIdentifierPolicySummary {
        ToriiIdentifierPolicySummary(
            policyId: try string(policy, "policy_id"),
            owner: try string(policy, "owner"),
            active: try bool(policy, "active"),
            normalization: .exact,
            resolverPublicKey: try string(policy, "resolver_public_key"),
            backend: try string(policy, "backend"),
            inputEncryption: try string(policy, "input_encryption"),
            inputEncryptionPublicParameters: nil,
            inputEncryptionPublicParametersDecoded: try bfvParameters(
                fromFixture: object(policy, "input_encryption_public_parameters_decoded")
            ),
            ramFheProfile: nil,
            proofVerifier: nil,
            note: nil
        )
    }

    private func bfvParameters(fromFixture params: [String: Any]) throws -> ToriiIdentifierBfvPublicParameters {
        let rawParameters = try object(params, "parameters")
        let rawPublicKey = try object(params, "public_key")
        return ToriiIdentifierBfvPublicParameters(
            parameters: ToriiIdentifierBfvParameters(
                polynomialDegree: UInt32(try int(rawParameters, "polynomial_degree")),
                plaintextModulus: try uint64(rawParameters, "plaintext_modulus"),
                ciphertextModulus: try uint64(rawParameters, "ciphertext_modulus"),
                decompositionBaseLog: UInt8(try int(rawParameters, "decomposition_base_log"))
            ),
            publicKey: ToriiIdentifierBfvPublicKey(
                b: try uint64Array(rawPublicKey, "b"),
                a: try uint64Array(rawPublicKey, "a")
            ),
            maxInputBytes: UInt16(try int(params, "max_input_bytes")),
            noritoLengthEncoding: params["norito_length_encoding"] as? String
        )
    }

    private func object(_ root: [String: Any], _ key: String) throws -> [String: Any] {
        guard let value = root[key] as? [String: Any] else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(key) must be an object"])
        }
        return value
    }

    private func objectArray(_ root: [String: Any], _ key: String) throws -> [[String: Any]] {
        guard let value = root[key] as? [[String: Any]] else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(key) must be an object array"])
        }
        return value
    }

    private func string(_ root: [String: Any], _ key: String) throws -> String {
        guard let value = root[key] as? String else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(key) must be a string"])
        }
        return value
    }

    private func assertBfvOperationKeyComponentVectors(_ operationVectors: [String: Any]) throws {
        try assertBfvEqual(try string(operationVectors, "vector_set"), "soracloud-bfv-operation-v1", "operation vector set")
        let publicParameters = try object(operationVectors, "public_parameters")
        let publicDegree = try int(publicParameters, "polynomial_degree")
        try assertBfvRnsModulusChainFixture(operationVectors, publicDegree: publicDegree)
        let evaluationKey = try object(operationVectors, "evaluation_key_bundle")
        try assertBfvEqual(
            try int(evaluationKey, "decomposition_base_log"),
            try int(publicParameters, "decomposition_base_log"),
            "evaluation-key decomposition base log"
        )
        try assertBfvEqual(
            try int(evaluationKey, "decomposition_digit_count"),
            try int(evaluationKey, "relinearization_entry_count"),
            "evaluation-key decomposition digit count"
        )
        let entries = try objectArray(evaluationKey, "relinearization_entries")
        try assertBfvEqual(entries.count, try int(evaluationKey, "relinearization_entry_count"), "relinearization entry count")
        var componentDigests = Set<String>()
        for (index, entry) in entries.enumerated() {
            try assertBfvEqual(try int(entry, "index"), index, "relinearization entry \(index) index")
            try assertBfvEqual(try int(entry, "coefficient_count"), publicDegree, "relinearization entry \(index) coefficient count")
            try assertBfvComponentDigest("relinearization entry \(index) b", try string(entry, "b_sha256"), seen: &componentDigests)
            try assertBfvComponentDigest("relinearization entry \(index) a", try string(entry, "a_sha256"), seen: &componentDigests)
        }
        let galoisKeys = try objectArray(operationVectors, "galois_keys")
        XCTAssertEqual(galoisKeys.count, try int(evaluationKey, "galois_key_count"))
        for key in galoisKeys {
            let power = try int(key, "automorphism_power")
            let keyEntries = try objectArray(key, "entries")
            XCTAssertEqual(keyEntries.count, try int(key, "entry_count"))
            for (index, entry) in keyEntries.enumerated() {
                XCTAssertEqual(try int(entry, "index"), index)
                XCTAssertEqual(try int(entry, "coefficient_count"), publicDegree)
                try assertBfvComponentDigest("Galois key \(power) entry \(index) b", try string(entry, "b_sha256"), seen: &componentDigests)
                try assertBfvComponentDigest("Galois key \(power) entry \(index) a", try string(entry, "a_sha256"), seen: &componentDigests)
            }
        }
        let galoisSwitchVectors = try objectArray(operationVectors, "galois_switch_vectors")
        XCTAssertFalse(galoisSwitchVectors.isEmpty)
        for vector in galoisSwitchVectors {
            let name = try string(vector, "name")
            let power = try int(vector, "automorphism_power")
            XCTAssertTrue(try galoisKeys.contains { try int($0, "automorphism_power") == power })
            guard let plaintextSlots = vector["input_plaintext_slots"] as? [Int] else {
                throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "input_plaintext_slots must be an integer array"])
            }
            XCTAssertFalse(plaintextSlots.isEmpty)
            XCTAssertTrue(plaintextSlots.allSatisfy { $0 >= 0 })
            XCTAssertGreaterThan(try int(vector, "expected_input_ciphertext_bytes"), 0)
            XCTAssertGreaterThan(try int(vector, "expected_output_ciphertext_bytes"), 0)
            assertBfvUpperSha256("Galois switch vector \(name) input", try string(vector, "expected_input_ciphertext_sha256"))
            assertBfvUpperSha256("Galois switch vector \(name) output", try string(vector, "expected_output_ciphertext_sha256"))
            assertBfvUpperSha256("Galois switch vector \(name) plaintext", try string(vector, "expected_plaintext_sha256"))
            let components = try object(vector, "output_components")
            XCTAssertEqual(try int(components, "coefficient_count"), publicDegree)
            try assertBfvComponentDigest("Galois switch vector \(name) c0", try string(components, "c0_sha256"), seen: &componentDigests)
            try assertBfvComponentDigest("Galois switch vector \(name) c1", try string(components, "c1_sha256"), seen: &componentDigests)
        }
        let packedGaloisSwitchVectors = try objectArray(operationVectors, "packed_galois_switch_vectors")
        XCTAssertFalse(packedGaloisSwitchVectors.isEmpty)
        for vector in packedGaloisSwitchVectors {
            let name = try string(vector, "name")
            let power = try int(vector, "automorphism_power")
            XCTAssertTrue(try galoisKeys.contains { try int($0, "automorphism_power") == power })
            guard let inputSlots = vector["input_packed_slots"] as? [Int] else {
                throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "input_packed_slots must be an integer array"])
            }
            guard let permutation = vector["expected_slot_permutation"] as? [Int] else {
                throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "expected_slot_permutation must be an integer array"])
            }
            guard let outputSlots = vector["expected_packed_slots"] as? [Int] else {
                throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "expected_packed_slots must be an integer array"])
            }
            XCTAssertEqual(inputSlots.count, publicDegree)
            XCTAssertEqual(permutation.count, publicDegree)
            XCTAssertEqual(outputSlots.count, publicDegree)
            XCTAssertTrue(inputSlots.allSatisfy { $0 >= 0 })
            XCTAssertTrue(permutation.allSatisfy { $0 >= 0 })
            XCTAssertTrue(outputSlots.allSatisfy { $0 >= 0 })
            assertBfvUpperSha256("packed Galois switch vector \(name) packed plaintext", try string(vector, "expected_packed_plaintext_sha256"))
            assertBfvUpperSha256("packed Galois switch vector \(name) input", try string(vector, "expected_input_ciphertext_sha256"))
            assertBfvUpperSha256("packed Galois switch vector \(name) output", try string(vector, "expected_output_ciphertext_sha256"))
            assertBfvUpperSha256("packed Galois switch vector \(name) plaintext", try string(vector, "expected_plaintext_coefficients_sha256"))
            let components = try object(vector, "output_components")
            XCTAssertEqual(try int(components, "coefficient_count"), publicDegree)
            try assertBfvComponentDigest("packed Galois switch vector \(name) c0", try string(components, "c0_sha256"), seen: &componentDigests)
            try assertBfvComponentDigest("packed Galois switch vector \(name) c1", try string(components, "c1_sha256"), seen: &componentDigests)
        }
        let rotationKeys = try objectArray(operationVectors, "rotation_keys")
        try assertBfvEqual(rotationKeys.count, try int(evaluationKey, "rotation_key_count"), "rotation key count")
        for key in rotationKeys {
            let components = try object(key, "zero_refresh_components")
            let steps = try int(key, "rotation_steps")
            try assertBfvEqual(try int(components, "coefficient_count"), publicDegree, "rotation key \(steps) coefficient count")
            try assertBfvComponentDigest("rotation key \(steps) c0", try string(components, "c0_sha256"), seen: &componentDigests)
            try assertBfvComponentDigest("rotation key \(steps) c1", try string(components, "c1_sha256"), seen: &componentDigests)
        }
        let bootstrap = try object(operationVectors, "bootstrap_key")
        try assertBfvEqual(try string(bootstrap, "key_id"), try string(evaluationKey, "bootstrap_key_id"), "bootstrap key id")
        try assertBfvEqual(try int(bootstrap, "max_refresh_rounds"), try int(evaluationKey, "bootstrap_max_refresh_rounds"), "bootstrap max refresh rounds")
        XCTAssertGreaterThan(try int(bootstrap, "max_refresh_rounds"), 0)
        let bootstrapComponents = try object(bootstrap, "zero_refresh_components")
        try assertBfvEqual(try int(bootstrapComponents, "coefficient_count"), publicDegree, "bootstrap coefficient count")
        try assertBfvComponentDigest("bootstrap c0", try string(bootstrapComponents, "c0_sha256"), seen: &componentDigests)
        try assertBfvComponentDigest("bootstrap c1", try string(bootstrapComponents, "c1_sha256"), seen: &componentDigests)
        let roundRefreshes = try objectArray(bootstrap, "round_refreshes")
        XCTAssertEqual(roundRefreshes.count, try int(bootstrap, "max_refresh_rounds"))
        for (index, refresh) in roundRefreshes.enumerated() {
            XCTAssertEqual(try int(refresh, "round_index"), index)
            XCTAssertGreaterThan(try int(refresh, "expected_refresh_bytes"), 0)
            assertBfvUpperSha256("bootstrap round \(index) refresh", try string(refresh, "expected_refresh_sha256"))
            let components = try object(refresh, "components")
            XCTAssertEqual(try int(components, "coefficient_count"), publicDegree)
            if index == 0 {
                XCTAssertEqual(try string(components, "c0_sha256"), try string(bootstrapComponents, "c0_sha256"))
                XCTAssertEqual(try string(components, "c1_sha256"), try string(bootstrapComponents, "c1_sha256"))
                assertBfvUpperSha256("bootstrap round 0 c0", try string(components, "c0_sha256"))
                assertBfvUpperSha256("bootstrap round 0 c1", try string(components, "c1_sha256"))
            } else {
                try assertBfvComponentDigest("bootstrap round \(index) c0", try string(components, "c0_sha256"), seen: &componentDigests)
                try assertBfvComponentDigest("bootstrap round \(index) c1", try string(components, "c1_sha256"), seen: &componentDigests)
            }
        }
        XCTAssertEqual(try string(roundRefreshes[0], "expected_refresh_sha256"), try string(bootstrap, "expected_zero_refresh_sha256"))
        if roundRefreshes.count > 1 {
            XCTAssertNotEqual(try string(roundRefreshes[0], "expected_refresh_sha256"), try string(roundRefreshes[1], "expected_refresh_sha256"))
        }
        try assertBfvFullBootstrapMaterialFixture(operationVectors)
        let bootstrapRefreshVectors = try objectArray(operationVectors, "bootstrap_refresh_vectors")
        XCTAssertFalse(bootstrapRefreshVectors.isEmpty)
        for vector in bootstrapRefreshVectors {
            let name = try string(vector, "name")
            XCTAssertEqual(try string(vector, "key_id"), try string(bootstrap, "key_id"))
            let refreshRounds = try int(vector, "refresh_rounds")
            XCTAssertGreaterThan(refreshRounds, 0)
            XCTAssertLessThanOrEqual(refreshRounds, try int(bootstrap, "max_refresh_rounds"))
            guard let plaintextSlots = vector["input_plaintext_slots"] as? [Int] else {
                throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "input_plaintext_slots must be an integer array"])
            }
            XCTAssertFalse(plaintextSlots.isEmpty)
            XCTAssertTrue(plaintextSlots.allSatisfy { $0 >= 0 })
            XCTAssertGreaterThan(try int(vector, "expected_input_ciphertext_bytes"), 0)
            XCTAssertGreaterThan(try int(vector, "expected_output_ciphertext_bytes"), 0)
            assertBfvUpperSha256("bootstrap refresh vector \(name) input", try string(vector, "expected_input_ciphertext_sha256"))
            assertBfvUpperSha256("bootstrap refresh vector \(name) output", try string(vector, "expected_output_ciphertext_sha256"))
            assertBfvUpperSha256("bootstrap refresh vector \(name) plaintext", try string(vector, "expected_plaintext_sha256"))
            let components = try object(vector, "output_components")
            XCTAssertEqual(try int(components, "coefficient_count"), publicDegree)
            try assertBfvComponentDigest("bootstrap refresh vector \(name) c0", try string(components, "c0_sha256"), seen: &componentDigests)
            try assertBfvComponentDigest("bootstrap refresh vector \(name) c1", try string(components, "c1_sha256"), seen: &componentDigests)
        }
        let runtimeVectors = try objectArray(operationVectors, "vectors")
        for vector in runtimeVectors {
            let vectorName = try string(vector, "name")
            let expectedDepth = try string(vector, "operation") == "Multiply"
                ? balancedBfvMultiplicationDepth(try objectArray(vector, "inputs").count)
                : 0
            XCTAssertEqual(
                try int(vector, "requested_multiplication_depth"),
                expectedDepth,
                "\(vectorName) requested multiplication depth"
            )
        }
        guard let packedRotate = runtimeVectors.first(where: { ($0["name"] as? String) == "soracloud-packed-rotate-left-output" }) else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "packed RotateLeft operation vector must be present"])
        }
        XCTAssertEqual(try string(packedRotate, "operation"), "RotateLeft")
        XCTAssertEqual(try int(packedRotate, "rotation_steps"), publicDegree / 2)
        let packedRotatePower = try int(packedRotate, "automorphism_power")
        XCTAssertEqual(packedRotatePower, publicDegree + 1)
        XCTAssertTrue(try galoisKeys.contains { try int($0, "automorphism_power") == packedRotatePower })
        let packedRotateInputs = try objectArray(packedRotate, "inputs")
        XCTAssertEqual(packedRotateInputs.count, 1)
        let packedRotateInput = packedRotateInputs[0]
        guard let inputSlots = packedRotateInput["packed_slots"] as? [Int] else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "packed RotateLeft packed_slots must be an integer array"])
        }
        guard let outputSlots = packedRotate["expected_packed_slots"] as? [Int] else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "packed RotateLeft expected_packed_slots must be an integer array"])
        }
        XCTAssertEqual(inputSlots.count, publicDegree)
        XCTAssertEqual(outputSlots.count, publicDegree)
        XCTAssertTrue(inputSlots.allSatisfy { $0 >= 0 })
        XCTAssertTrue(outputSlots.allSatisfy { $0 >= 0 })
        XCTAssertGreaterThan(try int(packedRotateInput, "expected_ciphertext_bytes"), 0)
        XCTAssertGreaterThan(try int(packedRotate, "expected_output_ciphertext_bytes"), 0)
        assertBfvUpperSha256("packed RotateLeft input plaintext", try string(packedRotateInput, "expected_packed_plaintext_sha256"))
        assertBfvUpperSha256("packed RotateLeft input", try string(packedRotateInput, "expected_ciphertext_sha256"))
        assertBfvUpperSha256("packed RotateLeft output", try string(packedRotate, "expected_output_ciphertext_sha256"))
        assertBfvUpperSha256("packed RotateLeft plaintext", try string(packedRotate, "expected_plaintext_coefficients_sha256"))
        let packedRotateComponents = try object(packedRotate, "output_components")
        XCTAssertEqual(try int(packedRotateComponents, "coefficient_count"), publicDegree)
        try assertBfvComponentDigest("packed RotateLeft c0", try string(packedRotateComponents, "c0_sha256"), seen: &componentDigests)
        try assertBfvComponentDigest("packed RotateLeft c1", try string(packedRotateComponents, "c1_sha256"), seen: &componentDigests)

        guard let packedRotateSchedule = runtimeVectors.first(where: { ($0["name"] as? String) == "soracloud-packed-rotate-left-schedule-output" }) else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "packed RotateLeft schedule vector must be present"])
        }
        XCTAssertEqual(try string(packedRotateSchedule, "operation"), "RotateLeft")
        XCTAssertEqual(try int(packedRotateSchedule, "rotation_steps"), 1)
        guard let schedulePowers = packedRotateSchedule["automorphism_powers"] as? [Int] else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "packed RotateLeft automorphism_powers must be an integer array"])
        }
        XCTAssertGreaterThan(schedulePowers.count, 1)
        for power in schedulePowers {
            XCTAssertGreaterThan(power, 0)
            XCTAssertTrue(try galoisKeys.contains { try int($0, "automorphism_power") == power })
        }
        let packedRotateScheduleInputs = try objectArray(packedRotateSchedule, "inputs")
        XCTAssertEqual(packedRotateScheduleInputs.count, 1)
        let packedRotateScheduleInput = packedRotateScheduleInputs[0]
        guard let scheduleInputSlots = packedRotateScheduleInput["packed_slots"] as? [Int] else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "packed RotateLeft schedule packed_slots must be an integer array"])
        }
        guard let scheduleOutputSlots = packedRotateSchedule["expected_packed_slots"] as? [Int] else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "packed RotateLeft schedule expected_packed_slots must be an integer array"])
        }
        XCTAssertEqual(scheduleInputSlots.count, publicDegree)
        XCTAssertEqual(scheduleOutputSlots.count, publicDegree)
        XCTAssertEqual(Array(scheduleInputSlots.dropFirst()) + [scheduleInputSlots[0]], scheduleOutputSlots)
        XCTAssertTrue(scheduleInputSlots.allSatisfy { $0 >= 0 })
        XCTAssertTrue(scheduleOutputSlots.allSatisfy { $0 >= 0 })
        XCTAssertGreaterThan(try int(packedRotateScheduleInput, "expected_ciphertext_bytes"), 0)
        XCTAssertGreaterThan(try int(packedRotateSchedule, "expected_output_ciphertext_bytes"), 0)
        assertBfvUpperSha256("packed RotateLeft schedule input plaintext", try string(packedRotateScheduleInput, "expected_packed_plaintext_sha256"))
        assertBfvUpperSha256("packed RotateLeft schedule input", try string(packedRotateScheduleInput, "expected_ciphertext_sha256"))
        assertBfvUpperSha256("packed RotateLeft schedule output", try string(packedRotateSchedule, "expected_output_ciphertext_sha256"))
        assertBfvUpperSha256("packed RotateLeft schedule plaintext", try string(packedRotateSchedule, "expected_plaintext_coefficients_sha256"))
        let packedRotateScheduleComponents = try object(packedRotateSchedule, "output_components")
        XCTAssertEqual(try int(packedRotateScheduleComponents, "coefficient_count"), publicDegree)
        try assertBfvComponentDigest("packed RotateLeft schedule c0", try string(packedRotateScheduleComponents, "c0_sha256"), seen: &componentDigests)
        try assertBfvComponentDigest("packed RotateLeft schedule c1", try string(packedRotateScheduleComponents, "c1_sha256"), seen: &componentDigests)
    }

    private func assertBfvFullBootstrapMaterialFixture(_ operationVectors: [String: Any]) throws {
        let material = try object(operationVectors, "full_bootstrap_material")
        try assertBfvEqual(try string(material, "circuit_id"), "iroha_bfv_full_bootstrap_v1", "full-bootstrap circuit id")
        try assertBfvEqual(try int(material, "max_bootstrap_depth"), 1, "full-bootstrap max depth")

        let digestFields = [
            "parameter_digest_hex",
            "rns_modulus_chain_digest_hex",
            "key_switch_decomposition_chain_digest_hex",
            "coefficient_to_slot_key_digest_hex",
            "slot_to_coefficient_key_digest_hex",
            "blind_rotation_key_digest_hex",
            "sample_extraction_key_digest_hex",
            "accumulator_digest_hex",
            "proof_public_input_schema_digest_hex",
            "prover_key_digest_hex",
            "prover_key_material_commitment_hex",
            "verifier_key_digest_hex",
            "verifier_key_material_commitment_hex",
            "vk_commitment_hex",
            "expected_material_digest_hex",
        ]
        let digestValues = try digestFields.map { field -> String in
            let value = try string(material, field)
            try requireBfvLowerDigest("full-bootstrap material \(field)", value)
            return value
        }
        try assertBfvEqual(
            try string(try object(operationVectors, "rns_modulus_chain"), "expected_digest_hex"),
            try string(material, "rns_modulus_chain_digest_hex"),
            "full-bootstrap RNS digest"
        )
        try assertBfvEqual(
            try string(material, "verifier_key_material_commitment_hex"),
            try string(material, "vk_commitment_hex"),
            "full-bootstrap verifier-key commitment"
        )
        XCTAssertNotEqual(
            try string(material, "verifier_key_digest_hex"),
            try string(material, "vk_commitment_hex"),
            "full-bootstrap verifier-key commitment must be distinct from the artifact digest"
        )
        var uniqueDigestValues: [String] = []
        for (field, value) in zip(digestFields, digestValues) where field != "vk_commitment_hex" {
            uniqueDigestValues.append(value)
        }
        try assertBfvEqual(Set(uniqueDigestValues).count, uniqueDigestValues.count, "full-bootstrap material digest roles")
    }

    private func assertBfvEqual<T: Equatable>(_ actual: T, _ expected: T, _ label: String) throws {
        guard actual == expected else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(label) mismatch"])
        }
    }

    private func assertBfvRnsModulusChainFixture(_ operationVectors: [String: Any], publicDegree: Int) throws {
        let rns = try object(operationVectors, "rns_modulus_chain")
        let moduli = try uint64Array(rns, "moduli")
        XCTAssertFalse(moduli.isEmpty)
        XCTAssertEqual(moduli, moduli.sorted())
        XCTAssertTrue(moduli.allSatisfy { $0 > 2 && $0 % 2 == 1 })
        XCTAssertTrue(try string(rns, "product").allSatisfy(\.isNumber))
        assertBfvLowerDigest("RNS modulus-chain digest", try string(rns, "expected_digest_hex"))

        let samples = try object(rns, "sample_polynomials")
        XCTAssertEqual(try uint64Array(samples, "lhs_coefficients").count, publicDegree)
        XCTAssertEqual(try uint64Array(samples, "rhs_coefficients").count, publicDegree)
        for label in ["lhs", "rhs", "sum", "negacyclic_product"] {
            try assertBfvRnsPolynomialFixture(
                label,
                try object(samples, label),
                publicDegree: publicDegree,
                limbCount: moduli.count
            )
        }
    }

    private func assertBfvRnsPolynomialFixture(
        _ label: String,
        _ polynomial: [String: Any],
        publicDegree: Int,
        limbCount: Int
    ) throws {
        XCTAssertEqual(try int(polynomial, "coefficient_count"), publicDegree)
        let limbHashes = try stringArray(polynomial, "residue_limb_sha256")
        XCTAssertEqual(limbHashes.count, limbCount)
        assertBfvUpperSha256("\(label) RNS reconstructed coefficients", try string(polynomial, "reconstructed_sha256"))
        for (index, digest) in limbHashes.enumerated() {
            assertBfvUpperSha256("\(label) RNS residue limb \(index)", digest)
        }
    }

    private func assertBfvComponentDigest(_ label: String, _ value: String, seen: inout Set<String>) throws {
        guard value.count == 64 else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(label) must be 32-byte hex"])
        }
        let nonHex = value.rangeOfCharacter(from: CharacterSet(charactersIn: "0123456789ABCDEF").inverted)
        guard nonHex == nil else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(label) must be uppercase hex"])
        }
        guard value != String(repeating: "0", count: 64) else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(label) must not be zero"])
        }
        let inserted = seen.insert(value).inserted
        guard inserted else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(label) must be unique"])
        }
    }

    private func assertBfvUpperSha256(_ label: String, _ value: String) {
        XCTAssertEqual(value.count, 64, "\(label) must be 32-byte hex")
        XCTAssertNil(value.rangeOfCharacter(from: CharacterSet(charactersIn: "0123456789ABCDEF").inverted), "\(label) must be uppercase hex")
        XCTAssertNotEqual(value, String(repeating: "0", count: 64), "\(label) must not be zero")
    }

    private func balancedBfvMultiplicationDepth(_ inputCount: Int) -> Int {
        XCTAssertGreaterThan(inputCount, 0, "BFV multiplication depth requires at least one input")
        var covered = 1
        var depth = 0
        while covered < inputCount {
            covered *= 2
            depth += 1
        }
        return depth
    }

    private func assertBfvLowerDigest(_ label: String, _ value: String) {
        XCTAssertEqual(value.count, 64, "\(label) must be 32-byte hex")
        XCTAssertNil(value.rangeOfCharacter(from: CharacterSet(charactersIn: "0123456789abcdef").inverted), "\(label) must be lowercase hex")
        XCTAssertNotEqual(value, String(repeating: "0", count: 64), "\(label) must not be zero")
    }

    private func requireBfvLowerDigest(_ label: String, _ value: String) throws {
        guard value.count == 64 else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(label) must be 32-byte hex"])
        }
        guard value.rangeOfCharacter(from: CharacterSet(charactersIn: "0123456789abcdef").inverted) == nil else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(label) must be lowercase hex"])
        }
        guard value != String(repeating: "0", count: 64) else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(label) must not be zero"])
        }
    }

    private func bool(_ root: [String: Any], _ key: String) throws -> Bool {
        guard let value = root[key] as? Bool else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(key) must be a bool"])
        }
        return value
    }

    private func int(_ root: [String: Any], _ key: String) throws -> Int {
        guard let value = root[key] as? NSNumber else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(key) must be a number"])
        }
        return value.intValue
    }

    private func uint64(_ root: [String: Any], _ key: String) throws -> UInt64 {
        if let value = root[key] as? NSNumber {
            return UInt64(truncating: value)
        }
        if let value = root[key] as? String, let parsed = UInt64(value) {
            return parsed
        }
        throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(key) must be an unsigned integer"])
    }

    private func uint64Array(_ root: [String: Any], _ key: String) throws -> [UInt64] {
        guard let values = root[key] as? [Any] else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(key) must be a number array"])
        }
        return try values.map { value in
            if let number = value as? NSNumber {
                return UInt64(truncating: number)
            }
            if let string = value as? String, let parsed = UInt64(string) {
                return parsed
            }
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(key) must contain unsigned integers"])
        }
    }

    private func stringArray(_ root: [String: Any], _ key: String) throws -> [String] {
        guard let values = root[key] as? [String] else {
            throw NSError(domain: "ToriiClientTests", code: 1, userInfo: [NSLocalizedDescriptionKey: "\(key) must be a string array"])
        }
        return values
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
                {"payload":{"entrypoint_hash":"entry","signed_transaction_hash":null,"submitted_at_ms":3,"submitted_at_height":4,"signer":"entry-signer"},"signature":"feedface"}
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
            XCTAssertEqual(request.url?.path, "/v1/connect/status/aggregate")
            self.assertOperatorAuthentication(request)
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
            XCTAssertEqual(request.url?.path, "/v1/connect/status/aggregate")
            let response = HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil, headerFields: [:])!
            return (response, Data())
        }
        let snapshot = try await makeClient().getConnectStatus()
        XCTAssertNil(snapshot)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testCreateConnectSessionPostsPayload() async throws {
        let networkID = TestNetworkIds.canonical
        let appPublicKey = Data(repeating: 0x31, count: 32)
        let nonce = Data(repeating: 0x32, count: 16)
        let sid = try ConnectCrypto.deriveSessionID(
            networkID: networkID,
            appPublicKey: appPublicKey,
            nonce: nonce
        )
        let sidLiteral = toriiClientTestBase64URL(sid)
        let appPublicKeyLiteral = toriiClientTestBase64URL(appPublicKey)
        let nonceLiteral = toriiClientTestBase64URL(nonce)
        let connectResponse = toriiClientTestConnectSessionResponse(
            sid: sidLiteral,
            networkID: networkID.literal,
            appPublicKey: appPublicKeyLiteral,
            nonce: nonceLiteral,
            node: "node-1"
        )
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/connect/session")
            XCTAssertEqual(request.httpMethod, "POST")
            let body = self.bodyJSON(from: request)
            XCTAssertEqual(body["sid"] as? String, sidLiteral)
            XCTAssertEqual(body["network_id"] as? String, networkID.literal)
            XCTAssertEqual(body["app_pk"] as? String, appPublicKeyLiteral)
            XCTAssertEqual(body["nonce"] as? String, nonceLiteral)
            XCTAssertEqual(body["node"] as? String, "node-1")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let data = try JSONSerialization.data(withJSONObject: connectResponse.payload)
            return (response, data)
        }
        let response = try await makeClient().createConnectSession(
            networkID: networkID,
            appPublicKey: appPublicKey,
            nonce: nonce,
            node: "node-1"
        )
        XCTAssertEqual(response.sid, sidLiteral)
        XCTAssertEqual(response.tokenWallet, connectResponse.tokenWallet)
        XCTAssertEqual(response.tokenManagement, connectResponse.tokenManagement)
        XCTAssertEqual(response.tokenRelay, connectResponse.tokenRelay)
        XCTAssertTrue(response.extra.isEmpty)
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

    private func vpnProfileResponsePayload() -> [String: Any] {
        [
            "available": true,
            "supported_exit_classes": ["standard", "low-latency", "high-security"],
            "default_exit_class": "standard",
            "relay_endpoint": "/dns4/vpn.sora.org/udp/443/quic",
            "lease_secs": 900,
            "dns_push_interval_secs": 60,
            "route_pushes": ["0.0.0.0/0"],
            "excluded_routes": ["10.0.0.0/8"],
            "dns_servers": ["1.1.1.1"],
            "tunnel_addresses": ["10.208.0.2/32"],
            "mtu_bytes": 1280,
            "meter_family": "vpn-standard",
            "display_billing_label": "standard vpn",
            "operator_account_id": "vpn_operator",
            "lease_fee": "1000000.25",
            "settlement_grace_secs": 60,
            "flow_label_bits": 24,
            "padding_budget_ms": 15,
            "relay_id_hex": vpnRelayIdHex,
            "descriptor_commit_hex": String(repeating: "cd", count: 32),
            "tls_server_name": "vpn.sora.org",
            "relay_tls_spki_sha256_hex": String(repeating: "ab", count: 32),
            "relay_certificate_sha256_hex": String(repeating: "ef", count: 32),
            "directory_snapshot_digest_hex": String(repeating: "42", count: 32)
        ]
    }

    private func vpnQuoteResponsePayload() -> [String: Any] {
        let quoteId = String(repeating: "11", count: 32)
        let instruction: [String: Any] = ["wire_id": "OpenVpnLeaseEscrow", "payload_hex": "abcd"]
        return [
            "quote_id": quoteId,
            "lease_id_hex": quoteId,
            "session_id_hex": String(repeating: "22", count: 16),
            "payment_reference": quoteId,
            "account_id": "alice",
            "exit_class": "standard",
            "relay_endpoint": "/dns4/vpn.sora.org/udp/443/quic",
            "lease_secs": 900,
            "quote_expires_at_ms": 1_700_000_900_000,
            "fee_asset_id": "xor#universal.universal",
            "escrow_account_id": "vpn_escrow",
            "operator_account_id": "vpn_operator",
            "lease_fee": "1000000.25",
            "route_pushes": [],
            "excluded_routes": [],
            "dns_servers": ["1.1.1.1"],
            "tunnel_addresses": ["10.208.0.2/32"],
            "mtu_bytes": 1280,
            "meter_family": "vpn-standard",
            "flow_label_bits": 24,
            "padding_budget_ms": 15,
            "relay_id_hex": vpnRelayIdHex,
            "descriptor_commit_hex": String(repeating: "cd", count: 32),
            "tls_server_name": "vpn.sora.org",
            "relay_tls_spki_sha256_hex": String(repeating: "ab", count: 32),
            "relay_certificate_sha256_hex": String(repeating: "ef", count: 32),
            "directory_snapshot_digest_hex": String(repeating: "42", count: 32),
            "metering_public_key_hex": String(repeating: "44", count: 32),
            "open_lease_instruction": instruction
        ]
    }

    private func vpnSessionResponsePayload() -> [String: Any] {
        let sessionId = String(repeating: "55", count: 32)
        return [
            "session_id": sessionId,
            "account_id": "alice",
            "exit_class": "standard",
            "relay_endpoint": "/dns4/vpn.sora.org/udp/443/quic",
            "lease_secs": 900,
            "expires_at_ms": 1_700_000_900_000,
            "connected_at_ms": 1_700_000_000_000,
            "meter_family": "vpn-standard",
            "quote_id": sessionId,
            "payment_reference": sessionId,
            "payment_tx_hash": String(repeating: "66", count: 32),
            "fee_asset_id": "xor#universal.universal",
            "escrow_account_id": "vpn_escrow",
            "operator_account_id": "vpn_operator",
            "lease_fee": "1000000.25",
            "flow_label_bits": 24,
            "padding_budget_ms": 15,
            "relay_id_hex": vpnRelayIdHex,
            "descriptor_commit_hex": String(repeating: "cd", count: 32),
            "tls_server_name": "vpn.sora.org",
            "relay_tls_spki_sha256_hex": String(repeating: "ab", count: 32),
            "relay_certificate_sha256_hex": String(repeating: "ef", count: 32),
            "directory_snapshot_digest_hex": String(repeating: "42", count: 32),
            "route_pushes": [],
            "excluded_routes": [],
            "dns_servers": [],
            "tunnel_addresses": ["10.208.0.2/32"],
            "mtu_bytes": 1280,
            "helper_ticket_hex": "5356504e48543100" + String(repeating: "00", count: 688),
            "bytes_in": 0,
            "bytes_out": 0,
            "status": "active"
        ]
    }

    private func vpnReceiptResponsePayload() -> [String: Any] {
        let sessionId = String(repeating: "77", count: 32)
        return [
            "session_id": sessionId,
            "account_id": "alice",
            "exit_class": "standard",
            "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
            "meter_family": "vpn-standard",
            "connected_at_ms": 1,
            "disconnected_at_ms": 2,
            "duration_ms": 1,
            "bytes_in": 10,
            "bytes_out": 20,
            "status": "settled",
            "receipt_source": "relay",
            "quote_id": sessionId,
            "payment_tx_hash": String(repeating: "88", count: 32),
            "fee_asset_id": "xor#universal.universal",
            "escrow_account_id": "vpn_escrow",
            "operator_account_id": "vpn_operator",
            "lease_fee": "1000000.25",
            "earned_fee": "700000.125",
            "refunded_fee": "300000.125",
            "lease_id_hex": sessionId,
            "settle_lease_instruction": NSNull()
        ]
    }

    func testVpnResponseModelsRejectUnknownFields() throws {
        let decoder = JSONDecoder()
        let fixtures: [(String, [String: Any], (Data) throws -> Void)] = [
            ("profile", vpnProfileResponsePayload(), { _ = try decoder.decode(ToriiVpnProfile.self, from: $0) }),
            ("quote", vpnQuoteResponsePayload(), { _ = try decoder.decode(ToriiVpnQuote.self, from: $0) }),
            ("session", vpnSessionResponsePayload(), { _ = try decoder.decode(ToriiVpnSession.self, from: $0) }),
            ("receipt", vpnReceiptResponsePayload(), { _ = try decoder.decode(ToriiVpnReceipt.self, from: $0) }),
            ("receipt list", ["items": [vpnReceiptResponsePayload()], "total": 1], {
                _ = try decoder.decode(ToriiVpnReceiptListResponse.self, from: $0)
            })
        ]

        for (label, original, decode) in fixtures {
            var payload = original
            payload["unexpected"] = true
            let data = try JSONSerialization.data(withJSONObject: payload)
            XCTAssertThrowsError(try decode(data), label)
        }

        var quote = vpnQuoteResponsePayload()
        var instruction = try XCTUnwrap(quote["open_lease_instruction"] as? [String: Any])
        instruction["unexpected"] = true
        quote["open_lease_instruction"] = instruction
        let nestedData = try JSONSerialization.data(withJSONObject: quote)
        XCTAssertThrowsError(try decoder.decode(ToriiVpnQuote.self, from: nestedData))
    }

    func testVpnResponseModelsRejectNonCanonicalIdentifiersAndHashes() throws {
        let decoder = JSONDecoder()

        var profile = vpnProfileResponsePayload()
        profile["relay_tls_spki_sha256_hex"] = String(repeating: "AB", count: 32)
        XCTAssertThrowsError(
            try decoder.decode(
                ToriiVpnProfile.self,
                from: JSONSerialization.data(withJSONObject: profile)
            )
        )

        var quote = vpnQuoteResponsePayload()
        quote["quote_id"] = "0x" + String(repeating: "11", count: 32)
        XCTAssertThrowsError(
            try decoder.decode(
                ToriiVpnQuote.self,
                from: JSONSerialization.data(withJSONObject: quote)
            )
        )
        quote = vpnQuoteResponsePayload()
        quote["session_id_hex"] = String(repeating: "AA", count: 16)
        XCTAssertThrowsError(
            try decoder.decode(
                ToriiVpnQuote.self,
                from: JSONSerialization.data(withJSONObject: quote)
            )
        )

        var session = vpnSessionResponsePayload()
        session["session_id"] = String(repeating: "AA", count: 32)
        XCTAssertThrowsError(
            try decoder.decode(
                ToriiVpnSession.self,
                from: JSONSerialization.data(withJSONObject: session)
            )
        )
        session = vpnSessionResponsePayload()
        session["payment_tx_hash"] = "0x" + String(repeating: "66", count: 32)
        XCTAssertThrowsError(
            try decoder.decode(
                ToriiVpnSession.self,
                from: JSONSerialization.data(withJSONObject: session)
            )
        )

        var receipt = vpnReceiptResponsePayload()
        receipt["lease_id_hex"] = "0x" + String(repeating: "77", count: 32)
        XCTAssertThrowsError(
            try decoder.decode(
                ToriiVpnReceipt.self,
                from: JSONSerialization.data(withJSONObject: receipt)
            )
        )
        receipt = vpnReceiptResponsePayload()
        receipt["payment_tx_hash"] = String(repeating: "AA", count: 32)
        XCTAssertThrowsError(
            try decoder.decode(
                ToriiVpnReceipt.self,
                from: JSONSerialization.data(withJSONObject: receipt)
            )
        )
    }

    func testVpnResponseModelsRejectMissingRequiredFields() throws {
        let decoder = JSONDecoder()
        let fixtures: [(String, [String: Any], String, (Data) throws -> Void)] = [
            ("nullable profile TLS pin", vpnProfileResponsePayload(), "relay_tls_spki_sha256_hex", {
                _ = try decoder.decode(ToriiVpnProfile.self, from: $0)
            }),
            ("nullable quote instruction", vpnQuoteResponsePayload(), "open_lease_instruction", {
                _ = try decoder.decode(ToriiVpnQuote.self, from: $0)
            }),
            ("required session route list", vpnSessionResponsePayload(), "route_pushes", {
                _ = try decoder.decode(ToriiVpnSession.self, from: $0)
            }),
            ("nullable receipt instruction", vpnReceiptResponsePayload(), "settle_lease_instruction", {
                _ = try decoder.decode(ToriiVpnReceipt.self, from: $0)
            }),
            ("required receipt-list items", ["items": [vpnReceiptResponsePayload()], "total": 1], "items", {
                _ = try decoder.decode(ToriiVpnReceiptListResponse.self, from: $0)
            })
        ]

        for (label, original, missingKey, decode) in fixtures {
            var payload = original
            payload.removeValue(forKey: missingKey)
            let data = try JSONSerialization.data(withJSONObject: payload)
            XCTAssertThrowsError(try decode(data), label)
        }

        var quote = vpnQuoteResponsePayload()
        var instruction = try XCTUnwrap(quote["open_lease_instruction"] as? [String: Any])
        instruction.removeValue(forKey: "payload_hex")
        quote["open_lease_instruction"] = instruction
        XCTAssertThrowsError(
            try decoder.decode(
                ToriiVpnQuote.self,
                from: JSONSerialization.data(withJSONObject: quote)
            )
        )
    }

    func testVpnResponseModelsRejectOpenApiConstraintViolations() throws {
        let decoder = JSONDecoder()
        let invalidProfileFields: [(String, Any)] = [
            ("supported_exit_classes", ["standard", "low-latency"]),
            ("supported_exit_classes", ["standard", "standard", "high-security"]),
            ("default_exit_class", "unsupported"),
            ("lease_secs", 0),
            ("lease_secs", 4_294_967_296 as UInt64),
            ("mtu_bytes", 1_279),
            ("settlement_grace_secs", 0),
            ("flow_label_bits", 23),
            ("padding_budget_ms", 0),
            ("route_pushes", NSNull())
        ]
        for (field, value) in invalidProfileFields {
            var profile = vpnProfileResponsePayload()
            profile[field] = value
            XCTAssertThrowsError(
                try decoder.decode(
                    ToriiVpnProfile.self,
                    from: JSONSerialization.data(withJSONObject: profile)
                ),
                field
            )
        }

        var quoteWithRetiredInstructions = vpnQuoteResponsePayload()
        quoteWithRetiredInstructions["tx_instructions"] = []
        XCTAssertThrowsError(
            try decoder.decode(
                ToriiVpnQuote.self,
                from: JSONSerialization.data(withJSONObject: quoteWithRetiredInstructions)
            )
        )

        for (field, value) in [("exit_class", "unsupported"), ("status", "settled")] {
            var session = vpnSessionResponsePayload()
            session[field] = value
            XCTAssertThrowsError(
                try decoder.decode(
                    ToriiVpnSession.self,
                    from: JSONSerialization.data(withJSONObject: session)
                ),
                field
            )
        }

        let invalidReceipts: [(String, Any)] = [
            ("status", "active"),
            ("receipt_source", "operator"),
            ("tx_instructions", [])
        ]
        for (field, value) in invalidReceipts {
            var receipt = vpnReceiptResponsePayload()
            receipt[field] = value
            XCTAssertThrowsError(
                try decoder.decode(
                    ToriiVpnReceipt.self,
                    from: JSONSerialization.data(withJSONObject: receipt)
                ),
                field
            )
        }

        var oversizedItems: [String: Any] = [
            "items": Array(repeating: vpnReceiptResponsePayload(), count: 25),
            "total": 24
        ]
        XCTAssertThrowsError(
            try decoder.decode(
                ToriiVpnReceiptListResponse.self,
                from: JSONSerialization.data(withJSONObject: oversizedItems)
            )
        )
        oversizedItems = ["items": [], "total": 25]
        XCTAssertThrowsError(
            try decoder.decode(
                ToriiVpnReceiptListResponse.self,
                from: JSONSerialization.data(withJSONObject: oversizedItems)
            )
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetVpnProfileDeserializesNativeLeaseFields() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/vpn/profile")
            let payload: [String: Any] = [
                "available": true,
                "supported_exit_classes": ["standard", "low-latency", "high-security"],
                "default_exit_class": "standard",
                "relay_endpoint": "/dns4/vpn.sora.org/udp/443/quic",
                "lease_secs": 900,
                "dns_push_interval_secs": 60,
                "route_pushes": ["0.0.0.0/0"],
                "excluded_routes": ["10.0.0.0/8"],
                "dns_servers": ["1.1.1.1"],
                "tunnel_addresses": ["10.208.0.2/32"],
                "mtu_bytes": 1280,
                "meter_family": "vpn-standard",
                "display_billing_label": "standard vpn",
                "operator_account_id": "vpn_operator",
                "lease_fee": "1000000.25",
                "settlement_grace_secs": 60,
                "flow_label_bits": 24,
                "padding_budget_ms": 15,
                "relay_id_hex": self.vpnRelayIdHex,
                "descriptor_commit_hex": String(repeating: "cd", count: 32),
                "tls_server_name": "vpn.sora.org",
                "relay_tls_spki_sha256_hex": String(repeating: "ab", count: 32),
                "relay_certificate_sha256_hex": String(repeating: "ef", count: 32),
                "directory_snapshot_digest_hex": String(repeating: "42", count: 32)
            ]
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, try JSONSerialization.data(withJSONObject: payload))
        }
        let profile = try await makeClient().getVpnProfile()
        XCTAssertTrue(profile.available)
        XCTAssertEqual(profile.leaseFee, "1000000.25")
        XCTAssertEqual(profile.dnsPushIntervalSecs, 60)
        XCTAssertEqual(profile.flowLabelBits, 24)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testVpnRoutesRejectInsecureBaseURLBeforeTransportDispatch() async {
        let client = makeClient(baseURL: URL(string: "http://example.test")!)
        let sessionId = String(repeating: "11", count: 32)
        let auth = ToriiCanonicalRequestAuth(
            accountId: "alice@universal",
            privateKey: Data(repeating: 7, count: 32),
            timestampMs: 1_700_000_000_000,
            nonce: "vpn-https-test"
        )
        let operations: [(String, () async throws -> Void)] = [
            ("profile", { _ = try await client.getVpnProfile() }),
            ("quote", {
                _ = try await client.createVpnQuote(
                    ToriiVpnQuoteCreateRequest(
                        exitClass: "standard",
                        meteringPublicKeyHex: String(repeating: "22", count: 32)
                    ),
                    canonicalAuth: auth
                )
            }),
            ("session create", {
                _ = try await client.createVpnSession(
                    ToriiVpnSessionCreateRequest(
                        exitClass: "standard",
                        quoteId: sessionId,
                        paymentTransactionHash: String(repeating: "33", count: 32),
                        meteringPublicKeyHex: String(repeating: "44", count: 32)
                    ),
                    canonicalAuth: auth
                )
            }),
            ("session read", {
                _ = try await client.getVpnSession(sessionId: sessionId, canonicalAuth: auth)
            }),
            ("session delete", {
                _ = try await client.deleteVpnSession(sessionId: sessionId, canonicalAuth: auth)
            }),
            ("receipt submit", {
                _ = try await client.submitVpnReceipt(
                    ToriiVpnReceiptSubmitRequest(
                        relayReceiptHex: "abcd",
                        clientVoucherHex: "beef",
                        leaseIdHex: sessionId
                    ),
                    canonicalAuth: auth
                )
            }),
            ("receipt list", {
                _ = try await client.listVpnReceipts(canonicalAuth: auth)
            }),
        ]

        var dispatched = false
        StubURLProtocol.handler = { request in
            dispatched = true
            XCTFail("insecure VPN request reached transport dispatch: \(request)")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 500,
                httpVersion: nil,
                headerFields: nil
            )!
            return (response, Data())
        }

        for (label, operation) in operations {
            await XCTAssertThrowsErrorAsync(try await operation()) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("expected HTTPS rejection for \(label), got \(error)")
                }
                XCTAssertEqual(
                    reason,
                    "Sora VPN Torii requests require an HTTPS base URL.",
                    label
                )
            }
            XCTAssertFalse(dispatched, "\(label) reached transport dispatch")
        }
    }

    func testVpnProfileRejectsMissingOrOutOfRangeDnsPushInterval() throws {
        let validPayload: [String: Any] = [
            "available": true,
            "supported_exit_classes": ["standard", "low-latency", "high-security"],
            "default_exit_class": "standard",
            "relay_endpoint": "/dns4/vpn.sora.org/udp/443/quic",
            "lease_secs": 900,
            "dns_push_interval_secs": 60,
            "route_pushes": ["0.0.0.0/0"],
            "excluded_routes": ["10.0.0.0/8"],
            "dns_servers": ["1.1.1.1"],
            "tunnel_addresses": ["10.208.0.2/32"],
            "mtu_bytes": 1280,
            "meter_family": "vpn-standard",
            "display_billing_label": "standard vpn",
            "operator_account_id": "vpn_operator",
            "lease_fee": "1000000.25",
            "settlement_grace_secs": 60,
            "flow_label_bits": 24,
            "padding_budget_ms": 15,
            "relay_id_hex": vpnRelayIdHex,
            "descriptor_commit_hex": String(repeating: "cd", count: 32),
            "tls_server_name": "vpn.sora.org",
            "relay_tls_spki_sha256_hex": String(repeating: "ab", count: 32),
            "relay_certificate_sha256_hex": String(repeating: "ef", count: 32),
            "directory_snapshot_digest_hex": String(repeating: "42", count: 32)
        ]
        var missing = validPayload
        missing.removeValue(forKey: "dns_push_interval_secs")
        var belowMinimum = validPayload
        belowMinimum["dns_push_interval_secs"] = 29
        var nonInteger = validPayload
        nonInteger["dns_push_interval_secs"] = "60"

        for (label, payload) in [
            ("missing", missing),
            ("below minimum", belowMinimum),
            ("non-integer", nonInteger)
        ] {
            let data = try JSONSerialization.data(withJSONObject: payload)
            XCTAssertThrowsError(try JSONDecoder().decode(ToriiVpnProfile.self, from: data), label)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRegisterAndUnregisterPushDeviceSignCanonicalBody() async throws {
        let auth = ToriiCanonicalRequestAuth(accountId: "alice@universal",
                                             privateKey: Data(repeating: 7, count: 32),
                                             timestampMs: 1_700_000_000_010,
                                             nonce: "push-nonce-1")
        var callCount = 0
        StubURLProtocol.handler = { request in
            callCount += 1
            XCTAssertEqual(request.url?.path, "/v1/notify/devices")
            XCTAssertEqual(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerAccount), "alice@universal")
            XCTAssertEqual(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature) == nil, false)
            let body = self.bodyJSON(from: request)
            XCTAssertEqual(body["account_id"] as? String, "alice@universal")
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
        let request = ToriiPushDeviceRequest(accountId: " alice@universal ",
                                             platform: "FCM",
                                             token: " token-1 ",
                                             topics: [" activity "])
        try await client.registerPushDevice(request, canonicalAuth: auth)
        let deleteAuth = ToriiCanonicalRequestAuth(accountId: "alice@universal",
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
        let auth = ToriiCanonicalRequestAuth(accountId: "alice@universal",
                                             privateKey: Data(repeating: 7, count: 32),
                                             timestampMs: 1_700_000_000_000,
                                             nonce: "nonce-1")
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/vpn/quotes")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerAccount), "alice@universal")
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
                "account_id": "alice@universal",
                "exit_class": "standard",
                "relay_endpoint": "/dns4/vpn.sora.org/udp/443/quic",
                "lease_secs": 900,
                "quote_expires_at_ms": 1_700_000_900_000,
                "fee_asset_id": "xor#universal.universal",
                "escrow_account_id": "vpn_escrow",
                "operator_account_id": "vpn_operator",
                "lease_fee": "1000000.25",
                "route_pushes": [],
                "excluded_routes": [],
                "dns_servers": ["1.1.1.1"],
                "tunnel_addresses": ["10.208.0.2/32"],
                "mtu_bytes": 1280,
                "meter_family": "vpn-standard",
                "flow_label_bits": 24,
                "padding_budget_ms": 15,
                "relay_id_hex": self.vpnRelayIdHex,
                "descriptor_commit_hex": String(repeating: "cd", count: 32),
                "tls_server_name": "vpn.sora.org",
                "relay_tls_spki_sha256_hex": String(repeating: "ab", count: 32),
                "relay_certificate_sha256_hex": String(repeating: "ef", count: 32),
                "directory_snapshot_digest_hex": String(repeating: "42", count: 32),
                "metering_public_key_hex": meteringKey,
                "open_lease_instruction": [
                    "wire_id": "OpenVpnLeaseEscrow",
                    "payload_hex": "abcd"
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
        XCTAssertEqual(quote.leaseFee, "1000000.25")
        XCTAssertEqual(quote.openLeaseInstruction.wireId, "OpenVpnLeaseEscrow")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testCreateVpnQuoteRejectsPaddedCanonicalAuthBeforeRequest() async throws {
        let meteringKey = String(repeating: "ab", count: 32)
        let request = ToriiVpnQuoteCreateRequest(
            exitClass: "standard",
            meteringPublicKeyHex: meteringKey
        )
        var called = false
        StubURLProtocol.handler = { request in
            called = true
            let response = HTTPURLResponse(url: request.url!, statusCode: 500, httpVersion: nil, headerFields: [:])!
            return (response, Data())
        }
        let client = makeClient()

        let paddedAccount = ToriiCanonicalRequestAuth(
            accountId: " alice",
            privateKey: Data(repeating: 7, count: 32),
            timestampMs: 1_700_000_000_000,
            nonce: "nonce-1"
        )
        do {
            _ = try await client.createVpnQuote(request, canonicalAuth: paddedAccount)
            XCTFail("padded canonical auth account should reject")
        } catch {
            XCTAssertEqual(error as? ToriiCanonicalRequestError, .invalidAccountId)
        }
        XCTAssertFalse(called)

        let paddedNonce = ToriiCanonicalRequestAuth(
            accountId: "alice@universal",
            privateKey: Data(repeating: 7, count: 32),
            timestampMs: 1_700_000_000_000,
            nonce: "nonce-1 "
        )
        do {
            _ = try await client.createVpnQuote(request, canonicalAuth: paddedNonce)
            XCTFail("padded canonical auth nonce should reject")
        } catch {
            XCTAssertEqual(error as? ToriiCanonicalRequestError, .invalidNonce)
        }
        XCTAssertFalse(called)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testCreateAndGetVpnSessionUseQuotePaymentAndMeteringKey() async throws {
        let quoteId = String(repeating: "11", count: 32)
        let paymentHash = String(repeating: "22", count: 32)
        let meteringKey = String(repeating: "33", count: 32)
        let helperTicketHex = "5356504e48543100" + String(repeating: "00", count: 688)
        let auth = ToriiCanonicalRequestAuth(accountId: "alice@universal",
                                             privateKey: Data(repeating: 7, count: 32),
                                             timestampMs: 1_700_000_000_020,
                                             nonce: "vpn-session-nonce")
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
                "account_id": "alice@universal",
                "exit_class": "standard",
                "relay_endpoint": "/dns4/vpn.sora.org/udp/443/quic",
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
                "lease_fee": "1000000.25",
                "flow_label_bits": 24,
                "padding_budget_ms": 15,
                "relay_id_hex": self.vpnRelayIdHex,
                "descriptor_commit_hex": String(repeating: "cd", count: 32),
                "tls_server_name": "vpn.sora.org",
                "relay_tls_spki_sha256_hex": String(repeating: "ab", count: 32),
                "relay_certificate_sha256_hex": String(repeating: "ef", count: 32),
                "directory_snapshot_digest_hex": String(repeating: "42", count: 32),
                "route_pushes": [],
                "excluded_routes": [],
                "dns_servers": [],
                "tunnel_addresses": ["10.208.0.2/32"],
                "mtu_bytes": 1280,
                "helper_ticket_hex": helperTicketHex,
                "bytes_in": 0,
                "bytes_out": 0,
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
                                         meteringPublicKeyHex: meteringKey),
            canonicalAuth: auth
        )
        let fetched = try await client.getVpnSession(sessionId: quoteId, canonicalAuth: auth)
        XCTAssertEqual(created.sessionId, quoteId)
        XCTAssertEqual(created.leaseFee, "1000000.25")
        XCTAssertEqual(fetched?.paymentTransactionHash, paymentHash)
        XCTAssertEqual(created.helperTicketHex, helperTicketHex)
        XCTAssertEqual(created.helperTicketHex.utf8.count, 1_392)
    }

    func testVpnSessionRejectsNonCanonicalHelperTicketHex() throws {
        let sessionId = String(repeating: "11", count: 32)
        let paymentHash = String(repeating: "22", count: 32)
        let validHelperTicketHex = "5356504e48543100" + String(repeating: "00", count: 688)

        func sessionData(helperTicketHex: String) throws -> Data {
            let payload: [String: Any] = [
                "session_id": sessionId,
                "account_id": "alice",
                "exit_class": "standard",
                "relay_endpoint": "/dns4/vpn.sora.org/udp/443/quic",
                "lease_secs": 900,
                "expires_at_ms": 1_700_000_900_000,
                "connected_at_ms": 1_700_000_000_000,
                "meter_family": "vpn-standard",
                "quote_id": sessionId,
                "payment_reference": sessionId,
                "payment_tx_hash": paymentHash,
                "fee_asset_id": "xor#universal.universal",
                "escrow_account_id": "vpn_escrow",
                "operator_account_id": "vpn_operator",
                "lease_fee": "1000000.25",
                "flow_label_bits": 24,
                "padding_budget_ms": 15,
                "relay_id_hex": vpnRelayIdHex,
                "descriptor_commit_hex": String(repeating: "cd", count: 32),
                "tls_server_name": "vpn.sora.org",
                "relay_tls_spki_sha256_hex": String(repeating: "ab", count: 32),
                "relay_certificate_sha256_hex": String(repeating: "ef", count: 32),
                "directory_snapshot_digest_hex": String(repeating: "42", count: 32),
                "route_pushes": [],
                "excluded_routes": [],
                "dns_servers": [],
                "tunnel_addresses": ["10.208.0.2/32"],
                "mtu_bytes": 1280,
                "helper_ticket_hex": helperTicketHex,
                "bytes_in": 0,
                "bytes_out": 0,
                "status": "active"
            ]
            return try JSONSerialization.data(withJSONObject: payload)
        }

        let invalidValues = [
            "0x" + validHelperTicketHex,
            validHelperTicketHex.uppercased(),
            String(validHelperTicketHex.dropLast(2))
        ]
        for invalidValue in invalidValues {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiVpnSession.self,
                    from: sessionData(helperTicketHex: invalidValue)
                )
            )
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSubmitListAndDeleteVpnReceiptsExposeSettlementInstruction() async throws {
        let quoteId = String(repeating: "44", count: 32)
        let auth = ToriiCanonicalRequestAuth(accountId: "alice@universal",
                                             privateKey: Data(repeating: 7, count: 32),
                                             timestampMs: 1_700_000_000_030,
                                             nonce: "vpn-receipt-nonce")
        let settle: [String: Any] = [
            "wire_id": "SettleVpnLease",
            "payload_hex": "cafe"
        ]
        let receiptPayload: [String: Any] = [
            "session_id": quoteId,
            "account_id": "alice@universal",
            "exit_class": "standard",
            "relay_endpoint": "/dns4/vpn.sora.org/tcp/443/wss",
            "meter_family": "vpn-standard",
            "connected_at_ms": 1,
            "disconnected_at_ms": 2,
            "duration_ms": 1,
            "bytes_in": 10,
            "bytes_out": 20,
            "status": "settled",
            "receipt_source": "relay",
            "quote_id": quoteId,
            "payment_tx_hash": String(repeating: "55", count: 32),
            "fee_asset_id": "xor#universal.universal",
            "escrow_account_id": "vpn_escrow",
            "operator_account_id": "vpn_operator",
            "lease_fee": "1000000.25",
            "earned_fee": "700000.125",
            "refunded_fee": "300000.125",
            "lease_id_hex": quoteId,
            "settle_lease_instruction": settle
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
                return (response, try JSONSerialization.data(withJSONObject: ["items": [receiptPayload], "total": 1]))
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
                                         leaseIdHex: quoteId),
            canonicalAuth: auth
        )
        let list = try await client.listVpnReceipts(canonicalAuth: auth)
        let deleted = try await client.deleteVpnSession(sessionId: quoteId, canonicalAuth: auth)
        XCTAssertEqual(submitted.settleLeaseInstruction?.wireId, "SettleVpnLease")
        XCTAssertEqual(list.items.first?.earnedFee, "700000.125")
        XCTAssertEqual(deleted?.settleLeaseInstruction?.payloadHex, "cafe")
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
            "chunk_plan":{"schema":"sorafs.chunk_fetch_plan.v1","payload_digest_blake3_hex":"\(String(repeating: "ef", count: 32))","chunk_fetch_specs":[{"chunk_index":0,"offset":0,"length":4,"digest_blake3":"\(String(repeating: "22", count: 32))"}]}
        }
        """
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/da/manifests/\(ticket)")
            XCTAssertNil(request.url?.query)
            expectation.fulfill()
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, body.data(using: .utf8))
        }

        let bundle = try await makeClient().getDaManifestBundle(storageTicketHex: "0x\(ticket)")
        await fulfillment(of: [expectation], timeout: 1.0)
        XCTAssertEqual(bundle.storageTicketHex, ticket)
        XCTAssertEqual(bundle.blobHashHex, String(repeating: "ef", count: 32))
        XCTAssertEqual(bundle.manifestBytes, Data("manifest-data".utf8))
        XCTAssertEqual(bundle.laneId, 7)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSorafsGatewayProviderEncodesAndValidatesTrustInputs() throws {
        let provider = try SorafsGatewayProvider(
            name: "alpha",
            providerIdHex: String(repeating: "01", count: 32),
            gatewayPublicKeyHex: String(repeating: "ab", count: 32),
            baseURL: URL(string: "https://gateway.test/")!,
            streamTokenB64: Data("token".utf8).base64EncodedString(),
            privacyEventsURL: URL(string: "https://gateway.test/privacy/events")!
        )
        let encoded = try JSONEncoder().encode(provider)
        let object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: encoded) as? [String: String]
        )
        XCTAssertEqual(object["gateway_public_key_hex"], String(repeating: "ab", count: 32))

        for key in [
            String(repeating: "0", count: 64),
            String(repeating: "AB", count: 32),
            "0x" + String(repeating: "ab", count: 32),
            String(repeating: "ab", count: 31)
        ] {
            XCTAssertThrowsError(try SorafsGatewayProvider(
                name: "alpha",
                providerIdHex: String(repeating: "01", count: 32),
                gatewayPublicKeyHex: key,
                baseURL: URL(string: "https://gateway.test/")!,
                streamTokenB64: Data("token".utf8).base64EncodedString()
            ))
        }
        for rawURL in [
            "http://gateway.test/",
            "https://user@gateway.test/",
            "https://gateway.test:443/",
            "https://gateway.test/path",
            "https://gateway.test/?query=1",
            "https://localhost/",
            "https://127.0.0.1/",
            "https://10.0.0.1/",
            "https://169.254.169.254/",
            "https://192.0.2.1/",
            "https://198.51.100.1/",
            "https://203.0.113.1/",
            "https://[::1]/",
            "https://[fc00::1]/",
            "https://[fe80::1]/",
            "https://[2001:db8::1]/",
            "https://[::ffff:127.0.0.1]/"
        ] {
            XCTAssertThrowsError(try SorafsGatewayProvider(
                name: "alpha",
                providerIdHex: String(repeating: "01", count: 32),
                gatewayPublicKeyHex: String(repeating: "ab", count: 32),
                baseURL: URL(string: rawURL)!,
                streamTokenB64: Data("token".utf8).base64EncodedString()
            ))
        }
        XCTAssertNoThrow(try SorafsGatewayProvider(
            name: "alpha",
            providerIdHex: String(repeating: "01", count: 32),
            gatewayPublicKeyHex: String(repeating: "ab", count: 32),
            baseURL: URL(string: "https://gateway.test/")!,
            streamTokenB64: Data(repeating: 0, count: 2 * 1_024).base64EncodedString()
        ))
        XCTAssertThrowsError(try SorafsGatewayProvider(
            name: "alpha",
            providerIdHex: String(repeating: "01", count: 32),
            gatewayPublicKeyHex: String(repeating: "ab", count: 32),
            baseURL: URL(string: "https://gateway.test/")!,
            streamTokenB64: String(repeating: "A", count: 4 * 1_024 + 1)
        ))
        XCTAssertThrowsError(try SorafsGatewayProvider(
            name: "alpha",
            providerIdHex: String(repeating: "01", count: 32),
            gatewayPublicKeyHex: String(repeating: "ab", count: 32),
            baseURL: URL(string: "https://gateway.test/")!,
            streamTokenB64: Data(repeating: 0, count: 2 * 1_024 + 1).base64EncodedString()
        ))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testFetchDaPayloadViaGatewayUsesProvidedManifest() async throws {
        let bundle = try tcMakeSampleManifestBundle()
        let provider = try SorafsGatewayProvider(
            name: "alpha",
            providerIdHex: String(repeating: "01", count: 32),
            gatewayPublicKeyHex: String(repeating: "ab", count: 32),
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
            gatewayPublicKeyHex: String(repeating: "ab", count: 32),
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
            gatewayPublicKeyHex: String(repeating: "ab", count: 32),
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

    #if canImport(Darwin)
    func testNativeDaProofSummaryGeneratorEmitsExplicitProofs() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAvailable,
            "Norito bridge unavailable on this platform"
        )
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
            try failRequiredNativeTestCapability(
                "Native DA proof summary generator unavailable in this environment"
            )
        }
        XCTAssertEqual(summary.blobHashHex.lowercased(), fixture.blobHashHex.lowercased())
        XCTAssertEqual(summary.sampleCount, 0)
        XCTAssertEqual(summary.proofCount, 2)
        XCTAssertEqual(summary.proofs.count, 2)
        XCTAssertTrue(summary.proofs.allSatisfy { $0.origin == "explicit" })
        XCTAssertEqual(summary.proofs.first?.leafIndex, 0)
    }
    #endif

    @available(iOS 15.0, macOS 12.0, *)
    func testFetchDaPayloadViaGatewayAttachesProofSummary() async throws {
        let bundle = try tcMakeSampleManifestBundle()
        let provider = try SorafsGatewayProvider(
            name: "delta",
            providerIdHex: String(repeating: "04", count: 32),
            gatewayPublicKeyHex: String(repeating: "ab", count: 32),
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
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV/assets")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset":"66owaQmAQMuHxPzxUN3bqZ6FJfDa","account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","scope":"global","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!, session: session)

        let balances = try await sdk.getAssets(
            accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            asset: nil
        )
        XCTAssertEqual(balances.count, 1)
        XCTAssertEqual(balances.first?.asset, roseAssetDefinitionId)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsRejectsPaddedAccountLiteralBeforeNetwork() async {
        StubURLProtocol.handler = { request in
            XCTFail("getAssets should reject a padded account literal before dispatch")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 500,
                httpVersion: nil,
                headerFields: nil
            )!
            return (response, Data())
        }

        await XCTAssertThrowsErrorAsync(
            try await makeClient().getAssets(
                accountId: "  sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV  ",
                asset: nil
            )
        ) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload for padded accountId, got \(error)")
            }
            XCTAssertEqual(reason, "accountId must not contain surrounding whitespace.")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsPreservesTairaAccountDiscriminant() async throws {
        let accountId = try AccountAddress
            .fromAccount(publicKey: Data(repeating: 0x61, count: 32))
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(
                request,
                contains: "/v1/accounts/\(accountId)/assets"
            )
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, Data("[]".utf8))
        }

        let balances = try await makeClient().getAssets(
            accountId: accountId,
            asset: nil
        )
        XCTAssertTrue(balances.isEmpty)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsRejectsPercentEscapedAccountLiteral() async {
        await XCTAssertThrowsErrorAsync(
            try await makeClient().getAssets(
                accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV%2Fsorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
            ),
            expectation: { _ in }
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsEncodesAssetSelectorFilter() async throws {
        let assetId = roseAssetDefinitionId
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV/assets")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let assetFilter = components?.queryItems?.first(where: { $0.name == "asset" })?.value
            XCTAssertEqual(assetFilter, assetId)
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            [{"asset":"\(assetId)","account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","scope":"global","quantity":"10"}]
            """.data(using: .utf8)!
            return (response, body)
        }

        let balances = try await makeClient().getAssets(accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", asset: assetId)
        XCTAssertEqual(balances.count, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsEncodesScopeSelectorFilter() async throws {
        let accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/accounts/\(accountId)/assets")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let scopeFilter = components?.queryItems?.first(where: { $0.name == "scope" })?.value
            XCTAssertEqual(scopeFilter, "global")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, "[]".data(using: .utf8)!)
        }

        let balances = try await makeClient().getAssets(accountId: accountId, scope: "global")
        XCTAssertEqual(balances.count, 0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetAssetsRejectsPaddedScopeBeforeNetwork() async {
        let accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        StubURLProtocol.handler = { request in
            XCTFail("getAssets should validate scope before dispatch")
            let response = HTTPURLResponse(url: request.url!, statusCode: 500, httpVersion: nil, headerFields: nil)!
            return (response, Data())
        }

        await XCTAssertThrowsErrorAsync(
            try await makeClient().getAssets(accountId: accountId, scope: " global")
        ) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload for padded scope, got \(error)")
            }
            XCTAssertTrue(
                reason.contains("scope must not contain surrounding whitespace"),
                "Expected scope whitespace diagnostic, got \(reason)"
            )
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testAccountAssetQueryHelpersRejectSurroundingWhitespace() async {
        let accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let assetId = roseAssetDefinitionId
        let cases: [(String, () async throws -> Void)] = [
            (
                "account assets asset selector",
                { _ = try await self.makeClient().getAssets(accountId: accountId, asset: " \(assetId)") }
            ),
            (
                "account transactions asset selector",
                { _ = try await self.makeClient().getTransactions(accountId: accountId, assetDefinitionId: "\(assetId) ") }
            ),
            (
                "explorer transfers asset selector",
                { _ = try await self.makeClient().getExplorerTransfers(assetDefinitionId: " \(assetId)") }
            ),
            (
                "explorer transfer summaries asset selector",
                { _ = try await self.makeClient().getExplorerTransferSummaries(assetDefinitionId: "\(assetId) ") }
            ),
        ]

        for (label, action) in cases {
            await XCTAssertThrowsErrorAsync(try await action()) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload for \(label), got \(error)")
                }
                XCTAssertTrue(
                    reason.contains("surrounding whitespace"),
                    "Expected whitespace diagnostic for \(label), got \(reason)"
                )
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionsEncodesAccountLiteral() async throws {
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV/transactions")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"items":[{"entrypoint_hash":"hash","authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","timestamp_ms":1,"result_ok":true}],"total":1}
            """.data(using: .utf8)!
            return (response, body)
        }

        let transactions = try await makeClient().getTransactions(accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
        XCTAssertEqual(transactions.total, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTransactionsEncodesAssetIdFilter() async throws {
        let assetId = self.encodedRoseAssetID
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV/transactions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let assetFilter = components?.queryItems?.first(where: { $0.name == "asset_id" })?.value
            XCTAssertEqual(assetFilter, assetId)
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"items":[{"entrypoint_hash":"hash","authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","timestamp_ms":1,"result_ok":true}],"total":1}
            """.data(using: .utf8)!
            return (response, body)
        }

        let transactions = try await makeClient().getTransactions(accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", assetDefinitionId: assetId)
        XCTAssertEqual(transactions.total, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetExplorerAccountQrDecodesResponse() async throws {
        StubURLProtocol.handler = { request in
            self.assertDecodedPath(request, contains: "/v1/explorer/accounts/sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV/qr")
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

        let qr = try await makeClient().getExplorerAccountQr(accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
            self.assertDecodedPath(request, contains: "/v1/explorer/accounts/sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV/qr")
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

        let qr = try await makeClient().getExplorerAccountQr(accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
            XCTAssertEqual(query["authority"], "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
                        "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                        "created_at":"2025-01-01T00:00:00Z",
                        "kind":"Transfer",
                        "r#box":{
                            "scale":"0xdead",
                            "json":{
                                "kind":"Transfer",
                                "payload":{
                                    "Asset":{
                                        "source":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                                                     authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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

    func testCanonicalQuerySelectorsRejectSurroundingWhitespace() {
        let accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let assetId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
        let cases: [(String, () throws -> Void)] = [
            (
                "explorer instructions account",
                { _ = try ToriiExplorerInstructionsParams(account: " \(accountId)").queryItems() }
            ),
            (
                "explorer instructions authority",
                { _ = try ToriiExplorerInstructionsParams(authority: "\(accountId) ").queryItems() }
            ),
            (
                "explorer instructions asset",
                { _ = try ToriiExplorerInstructionsParams(assetDefinitionId: " \(assetId)").queryItems() }
            ),
            (
                "explorer transactions authority",
                { _ = try ToriiExplorerTransactionsParams(authority: "\(accountId) ").queryItems() }
            ),
            (
                "explorer transactions asset",
                { _ = try ToriiExplorerTransactionsParams(assetDefinitionId: "\(assetId) ").queryItems() }
            ),
            (
                "explorer rwas owner",
                { _ = try ToriiExplorerRwasParams(ownedBy: " \(accountId)").queryItems() }
            ),
            (
                "contract activity authority",
                { _ = try ToriiContractActivityParams(authority: "\(accountId) ").queryItems() }
            ),
            (
                "contract activity address",
                { _ = try ToriiContractActivityParams(contractAddress: " cntr:deadbeef").queryItems() }
            ),
            (
                "contract activity alias",
                { _ = try ToriiContractActivityParams(contractAlias: "benefits::paynet ").queryItems() }
            ),
            (
                "contract event authority",
                { _ = try ToriiContractEventParams(authority: " \(accountId)").queryItems() }
            ),
            (
                "contract event address",
                { _ = try ToriiContractEventParams(contractAddress: "cntr:deadbeef ").queryItems() }
            ),
            (
                "contract event alias",
                { _ = try ToriiContractEventParams(contractAlias: " benefits::paynet").queryItems() }
            ),
            (
                "contract event participant",
                { _ = try ToriiContractEventParams(participant: "merchant@paynet ").queryItems() }
            ),
            (
                "contract event asset",
                { _ = try ToriiContractEventParams(assetId: " \(assetId)").queryItems() }
            ),
            (
                "subscription owner",
                { _ = try ToriiSubscriptionListParams(ownedBy: "\(accountId) ").queryItems() }
            ),
            (
                "uaid portfolio asset",
                { _ = try ToriiUaidPortfolioQuery(assetId: " \(assetId)").queryItems() }
            ),
        ]

        for (label, action) in cases {
            XCTAssertThrowsError(try action(), label) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload for \(label), got \(error)")
                }
                XCTAssertTrue(
                    reason.contains("surrounding whitespace"),
                    "Expected whitespace diagnostic for \(label), got \(reason)"
                )
            }
        }
    }

    func testExplorerTransferDetailsParsesAsset() throws {
        let json = """
        {
            "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "created_at":"2025-01-01T00:00:00Z",
            "kind":"Transfer",
            "r#box":{
                "scale":"0x00",
                "json":{
                    "kind":"Transfer",
                    "payload":{
                        "variant":"Asset",
                        "value":{
                            "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
            XCTAssertEqual(asset.senderAccountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
            XCTAssertEqual(asset.assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            XCTAssertNil(details.role(for: "sorauﾛ1PgﾉﾀXﾖnWｱﾊｷﾕﾈjｷZﾖrﾅxｲWﾔﾀﾘYヰﾍxｺﾀﾃﾛｽfﾖ2Gｲ8P3LSM"))
            XCTAssertEqual(details.role(for: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), .sender)
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
            "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                                    "from":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
            XCTAssertEqual(entries[0].senderAccountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
            XCTAssertEqual(entries[0].receiverAccountId, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
            XCTAssertEqual(entries[0].assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            XCTAssertEqual(entries[0].amount, "5")
            XCTAssertEqual(entries[1].senderAccountId, "sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM")
            XCTAssertEqual(entries[1].receiverAccountId, "sorauﾛ1PﾜKNﾗ7ｼｺa2WｸｼﾒﾐQﾎbｺﾄocﾆﾁヰJaｱbg6sｾgｲﾖPfX7WAWRY")
            XCTAssertEqual(entries[1].assetDefinitionId, "61CtjvNd9T3THAR65GsMVHr82Bjc")
            XCTAssertEqual(entries[1].amount, "2")
            XCTAssertEqual(details.role(for: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), .sender)
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
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
        XCTAssertEqual(summary.senderAccountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
        XCTAssertEqual(summary.receiverAccountId, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
        XCTAssertEqual(summary.assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        XCTAssertEqual(summary.amount, "10")
        XCTAssertTrue(summary.isIncoming)
        XCTAssertFalse(summary.isOutgoing)
        XCTAssertFalse(summary.isSelfTransfer)
        XCTAssertEqual(summary.transferIndex, 0)
        XCTAssertEqual(summary.id, "hash1|0|0")
        XCTAssertEqual(summary.direction(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"), .incoming)
        XCTAssertEqual(summary.counterpartyAccountId(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"), "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                    "object":"10",
                                    "destination":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
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
        let summaries = page.transferSummaries(matchingAccount: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
        XCTAssertEqual(summaries.count, 1)
        let summary = summaries[0]
        XCTAssertEqual(summary.direction, .selfTransfer)
        XCTAssertTrue(summary.isSelfTransfer)
        XCTAssertFalse(summary.isIncoming)
        XCTAssertFalse(summary.isOutgoing)
        XCTAssertEqual(summary.direction(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), .selfTransfer)
        XCTAssertEqual(summary.counterpartyAccountId(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
        XCTAssertNil(summary.counterpartyAccountId(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"))
        XCTAssertTrue(summary.isSelfTransfer(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"))
        XCTAssertFalse(summary.isIncoming(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"))
        XCTAssertFalse(summary.isOutgoing(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"))
        XCTAssertEqual(summary.signedAmount(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), "10")
    }

    func testTransferSummarySignedAmountPreservesExistingSign() {
        let outgoing = ToriiExplorerTransferSummary(transactionHash: "hash1",
                                                    block: 1,
                                                    createdAt: "2025-01-01T00:00:00Z",
                                                    status: "Committed",
                                                    authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                                    instructionIndex: 0,
                                                    senderAccountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                                    receiverAccountId: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                                    amount: "-10",
                                                    direction: .outgoing,
                                                    kind: "Transfer",
                                                    transferIndex: 0)
        XCTAssertEqual(outgoing.signedAmount(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), "-10")

        let incoming = ToriiExplorerTransferSummary(transactionHash: "hash2",
                                                    block: 1,
                                                    createdAt: "2025-01-01T00:00:00Z",
                                                    status: "Committed",
                                                    authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                                    instructionIndex: 0,
                                                    senderAccountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                                            "from":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                            "to":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                            "asset_definition":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                            "amount":"5"
                                        },
                                        {
                                            "from":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
        XCTAssertEqual(try CanonicalNorito.canonicalAssetIdLiteral(literal), literal)
        XCTAssertEqual(CanonicalNorito.assetDefinitionIdFromLiteral(literal), "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
    }

    func testCanonicalAssetDefinitionLiteralRemainsCanonical() {
        XCTAssertEqual(
            CanonicalNorito.assetDefinitionIdFromLiteral("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
            "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
        )
    }

    func testMalformedPublicAssetLiteralReturnsNilDefinition() {
        XCTAssertNil(CanonicalNorito.assetDefinitionIdFromLiteral("62Fk4FPcMuLvW5QjDGNF2a4jAmjM#not-an-account"))
    }

    func testDecodeAccountIdReadsNoritoStringField() throws {
        let publicKey = Data(repeating: 0x33, count: 32)
        let address = try AccountAddress.fromAccount(publicKey: publicKey, algorithm: "ed25519")
        let accountId = try address.toI105(networkPrefix: 0x02F1)
        let encodedLength = withUnsafeBytes(of: UInt64(accountId.utf8.count).littleEndian) { Data($0) }
        let encoded = encodedLength + Data(accountId.utf8)
        XCTAssertEqual(try CanonicalNorito.decodeAccountId(encoded), accountId)
    }

    func testExplorerBurnInstructionParsedAsSummary() throws {
        let json = """
        {
            "pagination":{"page":1,"per_page":10,"total_pages":1,"total_items":1},
            "items":[{
                "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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

        let summaries = page.transferSummaries(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                        "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                        "created_at":"2025-01-01T00:00:00Z",
                        "kind":"Transfer",
                        "r#box":{
                            "scale":"0x00",
                            "json":{
                                "kind":"Transfer",
                                "payload":{
                                    "variant":"Asset",
                                    "value":{
                                        "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
            XCTAssertEqual(query["authority"], "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
                        "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                                                     authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
            XCTAssertEqual(query["authority"], "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
                        "authority": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                        "timestamp_ms": 1234,
                        "entrypoint_hash": "0xabc",
                        "result_ok": true,
                        "contract_address": "cntr:deadbeef",
                        "contract_alias": "benefits::paynet",
                        "contract_entrypoint": "claim",
                        "contract_payload": {"amount": 500},
                        "fee_payment": {
                            "payer": "authority",
                            "value": {"charge_limits": [], "gas_limit": 50000}
                        }
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
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
        XCTAssertEqual(list.items.first?.feePayment, testFeePayment(gasLimit: 50_000))
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
                        "fee_payment": {
                            "payer": "authority",
                            "value": {"charge_limits": [], "gas_limit": 70000}
                        }
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
        XCTAssertEqual(list.items.first?.feePayment, testFeePayment(gasLimit: 70_000))
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
                "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                "created_at":"2025-01-01T00:00:00Z",
                "kind":"Transfer",
                "r#box":{
                    "scale":"0x00",
                    "json":{
                        "kind":"Transfer",
                        "payload":{
                            "variant":"Asset",
                            "value":{
                                "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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

    func testExplorerRwasParamsQueryItemsEncodeCursorAndDomain() throws {
        let params = ToriiExplorerRwasParams(cursor: "Y3Vyc29y",
                                             limit: 25,
                                             domain: "commodities.sora")
        let queryItems = try XCTUnwrap(params.queryItems())
        let query = Dictionary(uniqueKeysWithValues: queryItems.map { ($0.name, $0.value ?? "") })
        XCTAssertEqual(query["cursor"], "Y3Vyc29y")
        XCTAssertEqual(query["limit"], "25")
        XCTAssertEqual(query["domain"], "commodities.sora")
        XCTAssertNil(query["page"])
        XCTAssertNil(query["per_page"])
    }

    func testExplorerRwasParamsRejectInvalidCursorAndLimit() {
        for cursor in ["", "padded=", "a", "contains space"] {
            XCTAssertThrowsError(try ToriiExplorerRwasParams(cursor: cursor).queryItems())
        }
        for limit: UInt32 in [0, 101] {
            XCTAssertThrowsError(try ToriiExplorerRwasParams(limit: limit).queryItems())
        }
    }

    func testExplorerRwaCursorPageDecodesExactContract() throws {
        let json = """
        {
          "pagination":{"limit":2,"next_cursor":"Y3Vyc29y","has_more":true},
          "items":[]
        }
        """
        let page = try JSONDecoder().decode(ToriiExplorerRwasPage.self, from: Data(json.utf8))
        XCTAssertEqual(page.pagination.limit, 2)
        XCTAssertEqual(page.pagination.nextCursor, "Y3Vyc29y")
        XCTAssertTrue(page.pagination.hasMore)
    }

    func testExplorerRwaCursorPageRejectsRetiredUnknownAndInconsistentFields() {
        let retired = """
        {"page":1,"per_page":25,"total_pages":1,"total_items":0}
        """
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiExplorerCursorMeta.self, from: Data(retired.utf8))
        )

        let inconsistent = """
        {"limit":25,"next_cursor":null,"has_more":true}
        """
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiExplorerCursorMeta.self, from: Data(inconsistent.utf8))
        )

        let unknownOuter = """
        {
          "pagination":{"limit":25,"next_cursor":null,"has_more":false},
          "items":[],
          "total_items":0
        }
        """
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiExplorerRwasPage.self, from: Data(unknownOuter.utf8))
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testIterateExplorerRwasFollowsCursorAcrossEmptyScanPage() async throws {
        var observedCursors: [String?] = []
        var observedLimits: [String?] = []
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/explorer/rwas")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let query = Dictionary(
                uniqueKeysWithValues: (components?.queryItems ?? []).map { ($0.name, $0.value ?? "") }
            )
            observedCursors.append(query["cursor"])
            observedLimits.append(query["limit"])
            XCTAssertEqual(query["domain"], "commodities")

            let body: String
            switch query["cursor"] {
            case nil:
                body = """
                {
                  "pagination":{"limit":2,"next_cursor":"Y3Vyc29yLTE","has_more":true},
                  "items":[]
                }
                """
            case "Y3Vyc29yLTE":
                body = """
                {
                  "pagination":{"limit":2,"next_cursor":"Y3Vyc29yLTI","has_more":true},
                  "items":[{
                    "id":"lot-001$commodities.sora",
                    "owned_by":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "quantity":"1",
                    "held_quantity":"0",
                    "primary_reference":"vault://receipts/1",
                    "status":null,
                    "is_frozen":false,
                    "metadata":{}
                  }]
                }
                """
            case "Y3Vyc29yLTI":
                body = """
                {
                  "pagination":{"limit":2,"next_cursor":null,"has_more":false},
                  "items":[{
                    "id":"lot-002$commodities.sora",
                    "owned_by":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "quantity":"2",
                    "held_quantity":"0",
                    "primary_reference":"vault://receipts/2",
                    "status":null,
                    "is_frozen":false,
                    "metadata":{}
                  }]
                }
                """
            default:
                XCTFail("Unexpected cursor")
                body = """
                {"pagination":{"limit":2,"next_cursor":null,"has_more":false},"items":[]}
                """
            }
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, Data(body.utf8))
        }

        let stream = makeClient().iterateExplorerRwas(
            params: ToriiExplorerRwasParams(domain: "commodities")
        )
        var identifiers: [String] = []
        for try await item in stream {
            identifiers.append(item.id)
        }

        XCTAssertEqual(identifiers, ["lot-001$commodities.sora", "lot-002$commodities.sora"])
        XCTAssertEqual(observedCursors, [nil, "Y3Vyc29yLTE", "Y3Vyc29yLTI"])
        XCTAssertEqual(observedLimits, [nil, "2", "2"])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testIterateExplorerRwasRejectsRepeatedCursor() async throws {
        StubURLProtocol.handler = { request in
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let query = Dictionary(
                uniqueKeysWithValues: (components?.queryItems ?? []).map { ($0.name, $0.value ?? "") }
            )
            let cursor = query["cursor"]
            let body = """
            {
              "pagination":{"limit":1,"next_cursor":"Y3Vyc29yLTE","has_more":true},
              "items":[]
            }
            """
            if cursor != nil {
                XCTAssertEqual(cursor, "Y3Vyc29yLTE")
            }
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, Data(body.utf8))
        }

        do {
            for try await _ in makeClient().iterateExplorerRwas() {}
            XCTFail("Expected a repeated-cursor error")
        } catch {
            XCTAssertTrue(String(describing: error).contains("repeated a cursor"))
        }
    }

    func testExplorerRwaRecordDecodesNullStatusAndMetadataDefaults() throws {
        let json = """
        {
            "id":"lot-001$commodities.sora",
            "owned_by":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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

    func testAssetAndRwaReadbacksRejectNoncanonicalQuantities() throws {
        for quantity in ["-1", "01", "1.0", "1.20", " 1", "1e0"] {
            let assetJSON = """
            {"asset":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","quantity":"\(quantity)"}
            """
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiAssetBalance.self, from: Data(assetJSON.utf8)),
                "accepted asset quantity \(quantity)"
            )

            let rwaJSON = """
            {
              "id":"lot-001$commodities.sora",
              "owned_by":"owner",
              "quantity":"\(quantity)",
              "held_quantity":"0",
              "primary_reference":"vault://receipts/2",
              "status":null,
              "is_frozen":false,
              "metadata":{}
            }
            """
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiExplorerRwaRecord.self, from: Data(rwaJSON.utf8)),
                "accepted RWA quantity \(quantity)"
            )
        }

        let badHeld = """
        {
          "id":"lot-001$commodities.sora",
          "owned_by":"owner",
          "quantity":"1",
          "held_quantity":"0.0",
          "primary_reference":"vault://receipts/2",
          "status":null,
          "is_frozen":false,
          "metadata":{}
        }
        """
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiExplorerRwaRecord.self, from: Data(badHeld.utf8))
        )
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
                "owned_by":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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

    func testQueryEnvelopeRejectsBlankSelectFieldPaths() throws {
        let envelope = ToriiQueryEnvelope(selectEntries: [.fieldPath(" ")])
        XCTAssertThrowsError(try JSONEncoder().encode(envelope)) { error in
            let description = String(describing: error)
            XCTAssertTrue(description.contains("select field path must not be empty"))
        }
    }

    func testQueryEnvelopeRejectsBlankQueryName() throws {
        let envelope = ToriiQueryEnvelope(query: " ", select: Optional<[String]>.none)
        XCTAssertThrowsError(try JSONEncoder().encode(envelope)) { error in
            let description = String(describing: error)
            XCTAssertTrue(description.contains("query must be a non-empty string"))
        }
    }

    func testQueryEnvelopeRejectsInvalidCountMode() throws {
        let envelope = ToriiQueryEnvelope(select: Optional<[String]>.none, countMode: "full")
        XCTAssertThrowsError(try JSONEncoder().encode(envelope)) { error in
            let description = String(describing: error)
            XCTAssertTrue(description.contains("countMode must be bounded or exact"))
        }
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
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at":"2025-01-01T00:00:01Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                        "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                        "created_at":"2025-01-01T00:00:00Z",
                        "kind":"Transfer",
                        "r#box":{
                            "scale":"0x00",
                            "json":{
                                "kind":"Transfer",
                                "payload":{
                                    "variant":"Asset",
                                    "value":{
                                        "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at":"2025-01-01T00:00:01Z",
                    "kind":"Transfer",
                    "r#box":{
                        "scale":"0x00",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                "created_at":"2025-01-01T00:00:00Z",
                "kind":"Transfer",
                "r#box":{
                    "scale":"0x00",
                    "json":{
                        "kind":"Transfer",
                        "payload":{
                            "variant":"Asset",
                            "value":{
                                "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                        "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                        "created_at":"2025-01-01T00:00:00Z",
                        "kind":"Transfer",
                        "r#box":{
                            "scale":"0x00",
                            "json":{
                                "kind":"Transfer",
                                "payload":{
                                    "variant":"Asset",
                                    "value":{
                                        "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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

        let summaries = try await makeClient().getAccountTransferHistory(accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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

        _ = makeClient().getAccountTransferHistory(accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV") { result in
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

        let summaries = try await makeClient().getTransactionHistory(accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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

        _ = makeClient().getTransactionHistory(accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV") { result in
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
                            "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                            "created_at":"2025-01-01T00:00:00Z",
                            "kind":"Transfer",
                            "r#box":{
                                "scale":"0x00",
                                "json":{
                                    "kind":"Transfer",
                                    "payload":{
                                        "variant":"Asset",
                                        "value":{
                                            "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                            "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                            "created_at":"2025-01-01T00:00:01Z",
                            "kind":"Transfer",
                            "r#box":{
                                "scale":"0x00",
                                "json":{
                                    "kind":"Transfer",
                                    "payload":{
                                        "variant":"Asset",
                                        "value":{
                                            "source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                    {"id":"wonderland","owned_by":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","metadata":{"theme":"demo"}}
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
        XCTAssertEqual(record.ownedBy, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
            XCTAssertEqual(query["provider"], "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
            XCTAssertEqual(query["limit"], "10")
            XCTAssertEqual(query["offset"], "5")
            let payload: [String: Any] = [
                "items": [
                    [
                        "plan_id": "plan#subs",
                        "plan": [
                            "provider": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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

        let params = ToriiSubscriptionPlanListParams(provider: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", limit: 10, offset: 5)
        let response = try await makeClient().listSubscriptionPlans(params: params)
        XCTAssertEqual(response.total, 1)
        let item = try XCTUnwrap(response.items.first)
        XCTAssertEqual(item.planId, "plan#subs")
        if case let .string(provider)? = item.plan["provider"] {
            XCTAssertEqual(provider, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
        } else {
            XCTFail("missing plan provider")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testListSubscriptionsEncodesParams() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/subscriptions")
            let components = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)
            let query = Dictionary(uniqueKeysWithValues: (components?.queryItems ?? []).map { ($0.name, $0.value ?? "") })
            XCTAssertEqual(query["owned_by"], "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
            XCTAssertEqual(query["provider"], "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
                        "plan": ["provider": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"]
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
                                                 provider: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
    func testGetSubscriptionDecodesRecord() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/subscriptions/sub-1$subscriptions")
            let payload: [String: Any] = [
                "subscription_id": "sub-1$subscriptions",
                "subscription": ["status": "active"],
                "invoice": NSNull(),
                "plan": ["provider": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"]
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
            XCTAssertEqual(provider, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
                  "account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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

        let response = try await makeClient().getUaidPortfolio(uaid: "UAID:\(uaidHex.uppercased())")
        XCTAssertEqual(response.uaid, "uaid:\(uaidHex)")
        XCTAssertEqual(response.totals.accounts, 2)
        XCTAssertEqual(response.dataspaces.first?.accounts.first?.assets.first?.assetId,
                       "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        XCTAssertEqual(response.dataspaces.first?.accounts.first?.assets.first?.assetDefinitionId,
                       "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        XCTAssertEqual(response.dataspaces.first?.accounts.first?.assets.first?.quantity, "500")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetUaidPortfolioRejectsPaddedLiteralBeforeNetwork() async {
        StubURLProtocol.handler = { _ in
            XCTFail("getUaidPortfolio should validate UAID before dispatch")
            throw URLError(.badURL)
        }
        let uaidHex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

        for literal in [
            " uaid:\(uaidHex)",
            "uaid:\(uaidHex) ",
            "uaid: \(uaidHex)"
        ] {
            await XCTAssertThrowsErrorAsync(try await makeClient().getUaidPortfolio(uaid: literal)) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload error")
                }
                XCTAssertTrue(reason.contains("uaid must not contain surrounding whitespace"))
            }
        }
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
    private func onboardingPlanReceipt(
        request: ToriiAccountOnboardingPlanRequest,
        disposition: AliasPlanDispositionV1 = .create,
        chainDiscriminant: UInt16 = AccountId.defaultNetworkPrefix
    ) throws -> ToriiAccountOnboardingPlanReceipt {
        let authorityKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x51, count: 32))
        let authority = try AccountId.makeI105(
            publicKey: authorityKey.publicKey(),
            networkPrefix: chainDiscriminant
        )
        let alias = try ResolvedAccountAliasV1(
            canonicalName: request.alias,
            dataspaceId: 0
        )
        let intent = AliasIntentV1.accountAlias(
            try AliasAccountIntentV1(
                alias: alias,
                targetAccount: request.accountId,
                provision: .create,
                role: .primary
            )
        )
        let acquisition = try AliasLeaseAcquisitionV1(termYears: 1)
        let quoteGuard = try AliasQuoteGuardV1(
            expectedPolicyVersion: 1,
            expectedPaymentAsset: "4rPeAP6jAjiLVZThZYwwPRBuQagt",
            maxAmount: "0",
            validUntilMs: UInt64.max
        )
        let instructions = disposition == .noOp
            ? []
            : [try AliasFramedInstructionV1(
                wireId: EnsureAlias.wireId,
                framedPayload: Data([1, 2, 3])
            )]
        let body = ToriiAccountOnboardingPlanBody(
            version: ToriiAccountOnboardingPlanBody.version,
            request: request,
            authority: authority,
            networkId: TestNetworkIds.canonical,
            anchor: try AliasPlanAnchorV1(
                blockHeight: 1,
                blockHash: String(repeating: "11", count: 32)
            ),
            resource: AliasPlanResourceV1(
                intent: intent,
                disposition: disposition,
                quote: nil,
                instructionIndex: instructions.isEmpty ? nil : 0
            ),
            acquisition: acquisition,
            quoteGuard: quoteGuard,
            instructions: instructions,
            ownerAutoRenewInstruction: nil,
            validUntilMs: UInt64.max
        )
        let bodyBytes = try encodeTestCanonicalOnboardingBody(body)
        let planHash = try ToriiAccountOnboardingReceiptVerifier.canonicalHash(
            canonicalBodyNorito: bodyBytes
        )
        return ToriiAccountOnboardingPlanReceipt(
            body: body,
            planHash: try ToriiAccountOnboardingReceiptVerifier.canonicalHashLiteral(
                canonicalBodyNorito: bodyBytes
            ),
            signature: .string(try authorityKey.sign(planHash).hexUppercased())
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testAccountOnboardingReceiptVerifiesDomainHashAndAuthoritySignature() throws {
        let request = try ToriiAccountOnboardingPlanRequest(
            alias: "alice@universal",
            accountId: canonicalOwnerLiteral(
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
        )
        let receipt = try onboardingPlanReceipt(
            request: request,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
        let canonicalBody = try encodeTestCanonicalOnboardingBody(receipt.body)

        XCTAssertNoThrow(
            try ToriiAccountOnboardingReceiptVerifier.verify(
                receipt,
                for: request,
                canonicalBodyNorito: canonicalBody,
                expectedAuthority: receipt.body.authority,
                expectedNetworkId: receipt.body.networkId
            )
        )

        var expectedHash = Blake2b.hash256(
            Data("iroha:account-onboarding-plan-receipt:v1\0".utf8) + canonicalBody
        )
        expectedHash[expectedHash.index(before: expectedHash.endIndex)] |= 1
        XCTAssertEqual(
            receipt.planHash,
            try ToriiAccountOnboardingReceiptVerifier.canonicalHashLiteral(
                canonicalBodyNorito: canonicalBody
            )
        )

        var tamperedBody = canonicalBody
        tamperedBody.append(0)
        XCTAssertThrowsError(
            try ToriiAccountOnboardingReceiptVerifier.verify(
                receipt,
                for: request,
                canonicalBodyNorito: tamperedBody,
                expectedAuthority: receipt.body.authority,
                expectedNetworkId: receipt.body.networkId
            )
        ) { error in
            XCTAssertEqual(
                error as? ToriiAccountOnboardingReceiptVerificationError,
                .planHashMismatch
            )
        }

        guard case let .string(signatureHex) = receipt.signature,
              var signature = Data(hexString: signatureHex) else {
            return XCTFail("expected canonical signature hex")
        }
        signature[signature.startIndex] ^= 1
        let badSignature = ToriiAccountOnboardingPlanReceipt(
            body: receipt.body,
            planHash: receipt.planHash,
            signature: .string(signature.hexUppercased())
        )
        XCTAssertThrowsError(
            try ToriiAccountOnboardingReceiptVerifier.verify(
                badSignature,
                for: request,
                canonicalBodyNorito: canonicalBody,
                expectedAuthority: receipt.body.authority,
                expectedNetworkId: receipt.body.networkId
            )
        ) { error in
            XCTAssertEqual(
                error as? ToriiAccountOnboardingReceiptVerificationError,
                .signatureMismatch
            )
        }

        let wrongAuthority = try AccountId.makeI105(
            publicKey: SigningKey.ed25519(privateKey: Data(repeating: 0x52, count: 32)).publicKey()
        )
        XCTAssertThrowsError(
            try ToriiAccountOnboardingReceiptVerifier.verify(
                receipt,
                for: request,
                canonicalBodyNorito: canonicalBody,
                expectedAuthority: wrongAuthority,
                expectedNetworkId: receipt.body.networkId
            )
        ) { error in
            XCTAssertEqual(
                error as? ToriiAccountOnboardingReceiptVerificationError,
                .authorityMismatch
            )
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testAccountOnboardingReceiptRequiresExactPinnedNetworkId() throws {
        let request = try ToriiAccountOnboardingPlanRequest(
            alias: "alice@universal",
            accountId: canonicalOwnerLiteral()
        )
        let receipt = try onboardingPlanReceipt(request: request)
        let canonicalBody = try encodeTestCanonicalOnboardingBody(receipt.body)

        XCTAssertThrowsError(
            try ToriiAccountOnboardingReceiptVerifier.verify(
                receipt,
                for: request,
                canonicalBodyNorito: canonicalBody,
                expectedAuthority: receipt.body.authority,
                expectedNetworkId: TestNetworkIds.other
            )
        ) { error in
            XCTAssertEqual(
                error as? ToriiAccountOnboardingReceiptVerificationError,
                .networkIdMismatch
            )
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testProductionOnboardingBodyEncoderUsesNoritoOrFailsClosed() throws {
        let request = try ToriiAccountOnboardingPlanRequest(
            alias: "alice@universal",
            accountId: canonicalOwnerLiteral()
        )
        let receipt = try onboardingPlanReceipt(request: request)
        let transportJSON = try encodeTestCanonicalOnboardingBody(receipt.body)
        do {
            let encoded = try ToriiAccountOnboardingPlanBodyNorito.encode(receipt.body)
            XCTAssertFalse(encoded.isEmpty)
            XCTAssertNotEqual(encoded, transportJSON)
        } catch {
            XCTAssertEqual(
                error as? ToriiAccountOnboardingReceiptVerificationError,
                .canonicalBodyEncodingUnavailable
            )
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testAccountOnboardingPlansThenAppliesExactReceipt() async throws {
        let accountId = try canonicalOwnerLiteral()
        let intent = try ToriiAccountOnboardingPlanRequest(
            alias: "alice@universal",
            accountId: accountId,
            permissions: [" CanFoo ", "CanBar", "CanFoo"]
        )
        let receipt = try onboardingPlanReceipt(request: intent)
        let receiptBody = try JSONEncoder().encode(receipt)
        var requestCount = 0

        StubURLProtocol.handler = { request in
            requestCount += 1
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            XCTAssertEqual(
                request.value(forHTTPHeaderField: ToriiAccountOnboardingTokenHeader),
                self.onboardingToken
            )
            XCTAssertEqual(
                request.allHTTPHeaderFields?.keys.filter {
                    $0.caseInsensitiveCompare(ToriiAccountOnboardingTokenHeader) == .orderedSame
                }.count,
                1
            )
            let rawBody = try XCTUnwrap(self.bodyData(from: request))
            let rawText = String(decoding: rawBody, as: UTF8.self)
            XCTAssertFalse(rawText.contains(self.onboardingToken))
            XCTAssertFalse(rawText.contains("private_key"))
            XCTAssertFalse(rawText.contains("public_key_hex"))
            XCTAssertFalse(rawText.contains("uaid"))

            if request.url?.path == "/v1/accounts/onboard/plan" {
                let decoded = try JSONDecoder().decode(
                    ToriiAccountOnboardingPlanRequest.self,
                    from: rawBody
                )
                XCTAssertEqual(decoded, intent)
                let payload = self.bodyJSON(from: request)
                XCTAssertEqual(
                    Set(payload.keys),
                    Set(["version", "alias", "account_id", "permissions"])
                )
                XCTAssertEqual(payload["permissions"] as? [String], ["CanBar", "CanFoo"])
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!
                return (response, receiptBody)
            }

            XCTAssertEqual(request.url?.path, "/v1/accounts/onboard")
            let decoded = try JSONDecoder().decode(
                ToriiAccountOnboardingApplyRequest.self,
                from: rawBody
            )
            XCTAssertEqual(decoded.receipt, receipt)
            let responseBody = """
            {
              "account_id":"\(accountId)",
              "alias":"alice@universal",
              "tx_hash_hex":"\(String(repeating: "ab", count: 32))",
              "status":"Queued",
              "disposition":{"kind":"create","value":null}
            }
            """.data(using: .utf8)!
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 202,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, responseBody)
        }

        let client = makeClient(defaultHeaders: [
            ToriiAPITokenHeader: "global-api-token",
            ToriiAccountOnboardingTokenHeader.lowercased(): "retired-default-token-must-not-be-used"
        ])
        let planned = try await client.planAccountOnboarding(
            intent,
            onboardingToken: onboardingToken,
            expectedAuthority: receipt.body.authority,
            expectedNetworkId: receipt.body.networkId,
            bodyEncoder: encodeTestCanonicalOnboardingBody
        )
        XCTAssertEqual(planned, receipt)
        let applied = try await client.applyAccountOnboarding(
            planned,
            onboardingToken: onboardingToken,
            expectedAuthority: receipt.body.authority,
            expectedNetworkId: receipt.body.networkId,
            bodyEncoder: encodeTestCanonicalOnboardingBody
        )
        XCTAssertEqual(applied.status, .queued)
        XCTAssertEqual(applied.disposition, .create)
        XCTAssertEqual(applied.txHashHex, String(repeating: "ab", count: 32))
        XCTAssertEqual(requestCount, 2)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testAccountOnboardingExactReplayReturnsUnchanged() async throws {
        let accountId = try canonicalOwnerLiteral()
        let intent = try ToriiAccountOnboardingPlanRequest(
            alias: "alice@universal",
            accountId: accountId
        )
        let receipt = try onboardingPlanReceipt(request: intent, disposition: .noOp)

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/accounts/onboard")
            let responseBody = """
            {
              "account_id":"\(accountId)",
              "alias":"alice@universal",
              "status":"Unchanged",
              "disposition":{"kind":"no_op","value":null}
            }
            """.data(using: .utf8)!
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, responseBody)
        }

        let result = try await makeClient().applyAccountOnboarding(
            receipt,
            onboardingToken: onboardingToken,
            expectedAuthority: receipt.body.authority,
            expectedNetworkId: receipt.body.networkId,
            bodyEncoder: encodeTestCanonicalOnboardingBody
        )
        XCTAssertEqual(result.status, .unchanged)
        XCTAssertNil(result.txHashHex)
        XCTAssertEqual(result.disposition, .noOp)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testAccountOnboardingRejectsMalformedTokenBeforeNetwork() async throws {
        StubURLProtocol.handler = { _ in
            XCTFail("malformed onboarding token reached HTTP dispatch")
            throw URLError(.badURL)
        }
        let intent = try ToriiAccountOnboardingPlanRequest(
            alias: "alice@universal",
            accountId: canonicalOwnerLiteral()
        )
        let expectedAuthority = try canonicalOwnerLiteral()
        let expectedNetworkId = TestNetworkIds.canonical
        for token in [
            "",
            String(repeating: "T", count: 31),
            String(repeating: "T", count: 257),
            String(repeating: "T", count: 31) + " ",
            String(repeating: "T", count: 31) + "é"
        ] {
            do {
                _ = try await makeClient().planAccountOnboarding(
                    intent,
                    onboardingToken: token,
                    expectedAuthority: expectedAuthority,
                    expectedNetworkId: expectedNetworkId,
                    bodyEncoder: encodeTestCanonicalOnboardingBody
                )
                XCTFail("Expected malformed onboarding token to fail")
            } catch {
                guard case let ToriiClientError.invalidPayload(message) = error else {
                    return XCTFail("Expected invalidPayload error, got \(error)")
                }
                if !token.isEmpty {
                    XCTAssertFalse(message.contains(token))
                }
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testAccountOnboardingRejectsInvalidTrustPinsBeforeNetwork() async throws {
        StubURLProtocol.handler = { _ in
            XCTFail("invalid onboarding trust pin reached HTTP dispatch")
            throw URLError(.badURL)
        }
        let intent = try ToriiAccountOnboardingPlanRequest(
            alias: "alice@universal",
            accountId: canonicalOwnerLiteral()
        )
        let expectedAuthority = try canonicalOwnerLiteral()

        do {
            _ = try await makeClient().planAccountOnboarding(
                intent,
                onboardingToken: onboardingToken,
                expectedAuthority: "not-an-account-id",
                expectedNetworkId: TestNetworkIds.canonical,
                bodyEncoder: encodeTestCanonicalOnboardingBody
            )
            XCTFail("Expected invalid authority pin to fail")
        } catch {
            XCTAssertEqual(
                error as? ToriiAccountOnboardingReceiptVerificationError,
                .invalidAuthority
            )
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testAccountOnboardingPlanDoesNotFollowRedirectsAndRedactsToken() async throws {
        var requestCount = 0
        StubURLProtocol.handler = { request in
            requestCount += 1
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 307,
                httpVersion: nil,
                headerFields: [
                    "Location": "https://redirect.example/v1/accounts/onboard/plan",
                    "x-iroha-reject-code": self.onboardingToken
                ]
            )!
            return (
                response,
                Data("{\"message\":\"server echoed \(self.onboardingToken)\"}".utf8)
            )
        }
        let intent = try ToriiAccountOnboardingPlanRequest(
            alias: "alice@universal",
            accountId: canonicalOwnerLiteral()
        )
        let expectedAuthority = try canonicalOwnerLiteral()
        let expectedNetworkId = TestNetworkIds.canonical

        do {
            _ = try await makeClient().planAccountOnboarding(
                intent,
                onboardingToken: onboardingToken,
                expectedAuthority: expectedAuthority,
                expectedNetworkId: expectedNetworkId,
                bodyEncoder: encodeTestCanonicalOnboardingBody
            )
            XCTFail("Expected redirect response to fail closed")
        } catch {
            guard case let ToriiClientError.httpStatus(code, message, rejectCode) = error else {
                return XCTFail("Expected HTTP status error, got \(error)")
            }
            XCTAssertEqual(code, 307)
            XCTAssertEqual(message, "server echoed <redacted>")
            XCTAssertEqual(rejectCode, "<redacted>")
            XCTAssertFalse(error.localizedDescription.contains(onboardingToken))
        }
        XCTAssertEqual(requestCount, 1)
    }
    func testGetUaidBindingsReturnsDataspaces() async throws {
        let uaidHex = "abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd"
        let payload = """
        {
          "uaid":"uaid:\(uaidHex)",
          "dataspaces":[
            {"dataspace_id":0,"dataspace_alias":"universal","accounts":["sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"]},
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
        XCTAssertEqual(response.dataspaces.first?.accounts.first, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
            XCTAssertEqual(components?.queryItems?.first(where: { $0.name == "hash" })?.value, Self.pipelineHash)
            XCTAssertEqual(components?.queryItems?.first(where: { $0.name == "scope" })?.value, "auto")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"hash":"\(Self.pipelineHash)","status":{"kind":"Rejected","block_height":12},"scope":"global","resolved_from":"state"}
            """.data(using: .utf8)!
            return (response, body)
        }

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!, session: session)

        let status = try await sdk.getTransactionStatus(hashHex: Self.pipelineHash)
        XCTAssertEqual(status?.hash, Self.pipelineHash)
        XCTAssertEqual(status?.status.kind, "Rejected")
        XCTAssertEqual(status?.status.blockHeight, 12)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetPipelineRecoveryAsync() async throws {
        let payload = """
        {"format":"pipeline.recovery.v1","height":42,"dag":{"fingerprint":"abcdef","key_count":1},"txs":[{"hash":"0x01","reads":["account/sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"],"writes":["asset/62Fk4FPcMuLvW5QjDGNF2a4jAmjM"]}]}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/recovery/42")
            self.assertOperatorAuthentication(request)
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
            self.assertOperatorAuthentication(request)
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
        {"schema_version":1,"chain_height":42,"sumeragi":{"block_time_ms":1000,"commit_time_ms":2000,"stall_threshold_ms":6000},"admission":{"max_signatures":32,"max_instructions":4096,"max_tx_bytes":1048576,"max_decompressed_bytes":1048576,"max_metadata_depth":16},"block":{"max_transactions":512},"pipeline":{"signature_batch_max":0,"signature_batch_max_ed25519":64,"signature_batch_max_secp256k1":16,"signature_batch_max_pqc":8,"signature_batch_max_bls":16,"overlay_max_instructions":0,"ivm_max_decoded_instructions":1048576},"queue":{"size":2,"queued":1,"inflight":1},"fees":{"fee_asset_id":"xor#sora","fee_sink_account_id":"fees@system","base_fee":"0","per_byte_fee":"0","per_instruction_fee":"0","per_gas_unit_fee":"0","sponsor_vault_custody_account_id":"sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53","settlement_mode":"direct","successful_claim_fee_exempt_authorities":["authority@system"]}}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/preflight")
            self.assertOperatorAuthentication(request)
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
        XCTAssertEqual(
            preflight.fees.sponsorVaultCustodyAccountId,
            "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"
        )
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
            self.assertOperatorAuthentication(request)
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let sdk = IrohaSDK(
            baseURL: URL(string: "https://example.test")!,
            session: session,
            operatorSigningContext: Self.operatorSigningContext
        )

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

        let capabilities = try await makeClient().getNodeCapabilities(canonicalAuth: canonicalReadAuth)
        XCTAssertEqual(capabilities.abiVersion, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testKagemushaVerifierProjectionInitializerPreservesExactRecord() throws {
        let id = try ToriiVerifyingKeyId(
            backend: "halo2/ipa",
            name: KagemushaRecursiveSpend.VerifierRole.transfer.registryName
        )
        let verifier = try ToriiKagemushaActiveTransferVerifier(
            id: id,
            version: 7,
            circuitId: KagemushaRecursiveSpend.VerifierRole.transfer.circuitID,
            commitment: String(repeating: "ab", count: 32),
            publicInputsSchemaHash: String(repeating: "cd", count: 32),
            maxProofBytes: 4_096,
            activationHeight: 11,
            withdrawalHeight: 19
        )

        XCTAssertEqual(verifier.id, id)
        XCTAssertEqual(verifier.version, 7)
        XCTAssertEqual(
            verifier.circuitId,
            KagemushaRecursiveSpend.VerifierRole.transfer.circuitID
        )
        XCTAssertEqual(verifier.commitment, String(repeating: "ab", count: 32))
        XCTAssertEqual(
            verifier.publicInputsSchemaHash,
            String(repeating: "cd", count: 32)
        )
        XCTAssertEqual(verifier.maxProofBytes, 4_096)
        XCTAssertEqual(verifier.activationHeight, 11)
        XCTAssertEqual(verifier.withdrawalHeight, 19)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testKagemushaVerifierProjectionInitializerRejectsUntrustedRecords() throws {
        let id = try ToriiVerifyingKeyId(
            backend: "halo2/ipa",
            name: KagemushaRecursiveSpend.VerifierRole.transfer.registryName
        )
        let canonicalHash = String(repeating: "ab", count: 32)
        let schemaHash = String(repeating: "cd", count: 32)

        func construct(
            version: UInt32 = 7,
            circuitId: String = KagemushaRecursiveSpend.VerifierRole.transfer.circuitID,
            commitment: String = canonicalHash,
            publicInputsSchemaHash: String = schemaHash,
            maxProofBytes: UInt32 = 4_096,
            activationHeight: UInt64 = 11,
            withdrawalHeight: UInt64? = 19
        ) throws -> ToriiKagemushaActiveTransferVerifier {
            try ToriiKagemushaActiveTransferVerifier(
                id: id,
                version: version,
                circuitId: circuitId,
                commitment: commitment,
                publicInputsSchemaHash: publicInputsSchemaHash,
                maxProofBytes: maxProofBytes,
                activationHeight: activationHeight,
                withdrawalHeight: withdrawalHeight
            )
        }

        XCTAssertThrowsError(try construct(version: 0))
        XCTAssertThrowsError(try construct(commitment: "AB" + String(repeating: "ab", count: 31)))
        XCTAssertThrowsError(try construct(commitment: String(repeating: "0", count: 64)))
        XCTAssertThrowsError(
            try construct(publicInputsSchemaHash: String(repeating: "0", count: 64))
        )
        XCTAssertThrowsError(try construct(circuitId: "../substituted"))
        XCTAssertThrowsError(try construct(maxProofBytes: 0))
        XCTAssertThrowsError(try construct(withdrawalHeight: 11))
        XCTAssertThrowsError(try construct(withdrawalHeight: 10))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetOfflineCapabilityParsesExactUniversalContractOnExactRoute() async throws {
        let payload = """
        {
          "mandatory": false,
          "cash_handoff_capability": "cash_handoff_v1",
          "required_bridge_abi_version": 22,
          "max_hops": 8,
          "ready": true,
          "assets": [],
          "blockers": []
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/offline/readiness")
            XCTAssertNil(request.url?.query)
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, payload)
        }

        let status = try await makeClient().getOfflineCapability()
        XCTAssertFalse(status.mandatory)
        XCTAssertEqual(status.cashHandoffCapability, "cash_handoff_v1")
        XCTAssertEqual(status.requiredBridgeAbiVersion, 22)
        XCTAssertEqual(status.maxHops, 8)
        XCTAssertTrue(status.ready)
        XCTAssertTrue(status.assets.isEmpty)
        XCTAssertTrue(status.blockers.isEmpty)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetOfflineCapabilityRejectsNonUniversalClaims() async throws {
        let invalidPayloads = [
            #"{"mandatory":true,"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":22,"max_hops":8,"ready":true,"assets":[],"blockers":[]}"#,
            #"{"mandatory":false,"cash_handoff_capability":"cash_handoff_v2","required_bridge_abi_version":22,"max_hops":8,"ready":true,"assets":[],"blockers":[]}"#,
            #"{"mandatory":false,"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":21,"max_hops":8,"ready":true,"assets":[],"blockers":[]}"#,
            #"{"mandatory":false,"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":22,"max_hops":9,"ready":true,"assets":[],"blockers":[]}"#,
            #"{"mandatory":false,"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":22,"max_hops":8,"ready":false,"assets":[],"blockers":[]}"#,
            #"{"mandatory":false,"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":22,"max_hops":8,"ready":true,"assets":[{}],"blockers":[]}"#,
            #"{"mandatory":false,"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":22,"max_hops":8,"ready":true,"assets":[],"blockers":[{"code":"unexpected","message":"unexpected"}]}"#,
            #"{"mandatory":false,"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":22,"max_hops":8,"ready":true,"assets":[],"blockers":[],"future":true}"#,
        ]

        for payload in invalidPayloads {
            StubURLProtocol.handler = { request in
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!
                return (response, Data(payload.utf8))
            }
            do {
                _ = try await makeClient().getOfflineCapability()
                XCTFail("expected non-universal offline capability to fail")
            } catch {
                // Exact universal discovery is fail-closed.
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetOfflineCapabilityRejectsDuplicateKeysAndInvalidUtf8() async throws {
        let payloads = [
            Data(#"{"mandatory":false,"mandatory":false,"cash_handoff_capability":"cash_handoff_v1","required_bridge_abi_version":22,"max_hops":8,"ready":true,"assets":[],"blockers":[]}"#.utf8),
            Data([0xff, 0xfe, 0xfd]),
        ]
        for payload in payloads {
            StubURLProtocol.handler = { request in
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!
                return (response, payload)
            }
            do {
                _ = try await makeClient().getOfflineCapability()
                XCTFail("expected malformed offline capability to fail")
            } catch {
                // Duplicate and non-UTF-8 JSON must never be accepted.
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testOfflineOperationsUseCanonicalPathsAndDirectNoritoBodies() async throws {
        let operationId = String(repeating: "11", count: 32)
        func reference(_ kind: KagemushaOperationKind) throws -> KagemushaOperationReference {
            try KagemushaOperationReference(
                operationId: operationId,
                kind: kind,
                state: .pending,
                transactionHash: String(repeating: "22", count: 32),
                statusUri: "/v1/offline/operations/\(operationId)",
                submittedAtMs: 1_700_000_000_000
            )
        }
        let topUpResponseArchive = KagemushaOperationCodec.encodeReference(try reference(.topUp))
        let redeemResponseArchive = KagemushaOperationCodec.encodeReference(try reference(.redeem))
        let topUpRequestArchive = kagemushaOperationRequestArchive(
            schema: KagemushaRecursiveSpend.topUpRequestWireName,
            fieldCount: 8,
            operationIdFieldIndex: 6
        )
        let redeemRequestArchive = kagemushaOperationRequestArchive(
            schema: KagemushaRecursiveSpend.redeemRequestWireName,
            fieldCount: 10,
            operationIdFieldIndex: 8
        )
        let pendingStatusArchive = try XCTUnwrap(Data(hexString:
            "4e5254300000fb04214104df1bdcd39249bddd4db23a009600000000000000bdfee2508f80055702000000000000000000000000414031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131040000000041403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323208ffffffffffffffff"
        ))

        StubURLProtocol.handler = { request in
            let path = request.url?.path
            let responseBody: Data
            let status: Int
            switch path {
            case "/v1/offline/top-up":
                status = 202
                responseBody = topUpResponseArchive
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(self.bodyData(from: request), topUpRequestArchive)
                XCTAssertEqual(request.value(forHTTPHeaderField: "Idempotency-Key"), operationId)
                XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-norito")
            case "/v1/offline/redeem":
                status = 202
                responseBody = redeemResponseArchive
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(self.bodyData(from: request), redeemRequestArchive)
                XCTAssertEqual(request.value(forHTTPHeaderField: "Idempotency-Key"), operationId)
                XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-norito")
            case "/v1/offline/operations/\(operationId)":
                status = 200
                responseBody = pendingStatusArchive
                XCTAssertEqual(request.httpMethod, "GET")
            default:
                throw ToriiClientError.invalidURL(path ?? "")
            }
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/x-norito")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: status,
                httpVersion: nil,
                headerFields: [
                    "Content-Type": "application/x-norito",
                    "Location": "/v1/offline/operations/\(operationId)",
                ]
            )!
            return (response, responseBody)
        }

        let client = makeClient()
        let acceptedTopUp = try await client.submitKagemushaTopUp(
            KagemushaTopUpRequest(noritoArchive: topUpRequestArchive)
        )
        XCTAssertEqual(acceptedTopUp, try reference(.topUp))
        let acceptedRedeem = try await client.submitKagemushaRedeem(
            KagemushaRedeemRequest(noritoArchive: redeemRequestArchive)
        )
        XCTAssertEqual(acceptedRedeem, try reference(.redeem))
        let operationStatus = try await client.getKagemushaOperationStatus(
            operationId: operationId,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
        XCTAssertEqual(
            operationStatus,
            .pending(try .init(
                operationId: operationId,
                kind: .topUp,
                transactionHash: String(repeating: "22", count: 32),
                submittedAtMs: UInt64.max
            ))
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testOfflineStatus404PreservesCanonicalRejectCodeFromToriiResponse() async throws {
        let operationId = String(repeating: "11", count: 32)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(
                request.url?.path,
                "/v1/offline/operations/\(operationId)"
            )
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 404,
                httpVersion: nil,
                headerFields: [
                    "Content-Type": "application/x-norito",
                    "x-iroha-reject-code": "offline_operation_not_found",
                ]
            )!
            return (response, Data([0xff, 0xfe, 0xfd]))
        }

        do {
            _ = try await makeClient().getKagemushaOperationStatus(
                operationId: operationId,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
            XCTFail("missing operation status must return typed HTTP failure")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, _, rejectCode) = error else {
                return XCTFail("unexpected Torii failure: \(error)")
            }
            XCTAssertEqual(code, 404)
            XCTAssertEqual(rejectCode, "offline_operation_not_found")
            XCTAssertTrue(
                KagemushaOperationFinalityCoordinator
                    .statusResourceIsMissing(after: error)
            )
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testBodyOnlyOfflineStatus404CannotAuthorizeSubmission() async throws {
        let operationId = String(repeating: "11", count: 32)
        let request = try KagemushaTopUpRequest(
            noritoArchive: kagemushaOperationRequestArchive(
                schema: KagemushaRecursiveSpend.topUpRequestWireName,
                fieldCount: 8,
                operationIdFieldIndex: 6
            )
        )
        var methods: [String] = []
        StubURLProtocol.handler = { urlRequest in
            methods.append(urlRequest.httpMethod ?? "")
            XCTAssertEqual(
                urlRequest.url?.path,
                "/v1/offline/operations/\(operationId)"
            )
            let response = HTTPURLResponse(
                url: urlRequest.url!,
                statusCode: 404,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (
                response,
                Data(#"{"code":"offline_operation_not_found"}"#.utf8)
            )
        }

        do {
            _ = try await KagemushaOperationFinalityCoordinator.resolve(
                operation: .topUp(request),
                transport: makeClient(),
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1,
                initialState: 0,
                continuity: .unaccepted,
                existingDefinitiveSubmissionFailure: { _ in nil },
                revalidateBeforeSubmission: { _ in
                    XCTFail("body-only 404 must not reach revalidation")
                },
                markSubmissionAttempt: { state in
                    XCTFail("body-only 404 must not persist an attempt")
                    return state
                },
                recordAcceptance: { _, state in state },
                recordObservation: { _, _, state in state },
                recordRejection: { _, _, state in state },
                recordDefinitiveSubmissionFailure: { _, state in state }
            )
            XCTFail("body-only 404 must remain an ordinary HTTP failure")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, _, rejectCode) = error else {
                return XCTFail("unexpected Torii failure: \(error)")
            }
            XCTAssertEqual(code, 404)
            XCTAssertNil(rejectCode)
            XCTAssertFalse(
                KagemushaOperationFinalityCoordinator
                    .statusResourceIsMissing(after: error)
            )
        }
        XCTAssertEqual(methods, ["GET"])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testBodyOnlyOfflineSubmissionCodesCannotBecomeDefinitive() async throws {
        let request = try KagemushaTopUpRequest(
            noritoArchive: kagemushaOperationRequestArchive(
                schema: KagemushaRecursiveSpend.topUpRequestWireName,
                fieldCount: 8,
                operationIdFieldIndex: 6
            )
        )

        for (statusCode, bodyCode) in [
            (400, "offline_top_up_invalid"),
            (413, "request_payload_too_large"),
        ] {
            StubURLProtocol.handler = { urlRequest in
                XCTAssertEqual(urlRequest.url?.path, "/v1/offline/top-up")
                let response = HTTPURLResponse(
                    url: urlRequest.url!,
                    statusCode: statusCode,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!
                return (
                    response,
                    Data("{\"code\":\"\(bodyCode)\"}".utf8)
                )
            }

            do {
                _ = try await makeClient().submitKagemushaTopUp(request)
                XCTFail("rejected submission must fail")
            } catch let error as ToriiClientError {
                guard case let .httpStatus(
                    actualStatus,
                    _,
                    rejectCode
                ) = error else {
                    return XCTFail("unexpected Torii failure: \(error)")
                }
                XCTAssertEqual(actualStatus, statusCode)
                XCTAssertNil(rejectCode)
                XCTAssertEqual(
                    KagemushaSubmissionFailureClassifier.classify(
                        error,
                        target: .offlineTopUp
                    ),
                    .ambiguous
                )
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testOfflineSubmissionClassifierRequiresExactEndpointPairs() async throws {
        let operationId = String(repeating: "11", count: 32)
        let request = try KagemushaTopUpRequest(
            noritoArchive: kagemushaOperationRequestArchive(
                schema: KagemushaRecursiveSpend.topUpRequestWireName,
                fieldCount: 8,
                operationIdFieldIndex: 6
            )
        )

        let cases: [(Int, String, Bool)] = [
            (400, "offline_top_up_invalid", true),
            (400, "PRTRY:TX_SIGNATURE_INVALID", true),
            (409, "operation_id_conflict", true),
            (400, "request_norito_invalid", false),
            (409, "PRTRY:ALREADY_ENQUEUED", false),
            (413, "request_payload_too_large", false),
        ]
        for (statusCode, rejectCode, isDefinitive) in cases {
            StubURLProtocol.handler = { urlRequest in
                XCTAssertEqual(urlRequest.url?.path, "/v1/offline/top-up")
                XCTAssertEqual(
                    urlRequest.value(forHTTPHeaderField: "Idempotency-Key"),
                    operationId
                )
                let response = HTTPURLResponse(
                    url: urlRequest.url!,
                    statusCode: statusCode,
                    httpVersion: nil,
                    headerFields: [
                        "Content-Type": "application/x-norito",
                        "x-iroha-reject-code": rejectCode,
                    ]
                )!
                return (response, Data([0xff, 0xfe, 0xfd]))
            }

            do {
                _ = try await makeClient().submitKagemushaTopUp(request)
                XCTFail("rejected submission must fail")
            } catch let error as ToriiClientError {
                guard case let .httpStatus(
                    actualStatus,
                    _,
                    actualRejectCode
                ) = error else {
                    return XCTFail("unexpected Torii failure: \(error)")
                }
                XCTAssertEqual(actualStatus, statusCode)
                XCTAssertEqual(actualRejectCode, rejectCode)
                let disposition = KagemushaSubmissionFailureClassifier.classify(
                    error,
                    target: .offlineTopUp
                )
                if isDefinitive {
                    guard case let .definitivePreAdmission(failure) = disposition else {
                        return XCTFail("exact endpoint pair must be definitive")
                    }
                    XCTAssertEqual(failure.target, .offlineTopUp)
                    XCTAssertEqual(failure.statusCode, statusCode)
                    XCTAssertEqual(failure.rejectCode, rejectCode)
                    XCTAssertNil(failure.message)
                } else {
                    XCTAssertEqual(disposition, .ambiguous)
                }
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testOfflineOperationResponsesAreStreamingBoundedBeforeCodecParsing() async throws {
        let operationId = String(repeating: "11", count: 32)

        func install(
            status: Int,
            headers: [String: String],
            body: Data
        ) {
            StubURLProtocol.handler = { request in
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: status,
                    httpVersion: nil,
                    headerFields: headers
                )!
                return (response, body)
            }
        }

        install(
            status: 200,
            headers: [
                "Content-Type": "application/x-norito",
                "Content-Length": String(
                    KagemushaOperationCodec.statusMaximumArchiveBytes + 1
                ),
            ],
            body: Data([0x00])
        )
        await assertToriiInvalidPayload(contains: "declares more than") {
            _ = try await self.makeClient().getKagemushaOperationStatus(
                operationId: operationId,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
        }

        install(
            status: 200,
            headers: ["Content-Type": "application/x-norito"],
            body: Data(
                repeating: 0xa5,
                count: KagemushaOperationCodec.statusMaximumArchiveBytes + 1
            )
        )
        await assertToriiInvalidPayload(contains: "response exceeded") {
            _ = try await self.makeClient().getKagemushaOperationStatus(
                operationId: operationId,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
        }

        install(
            status: 200,
            headers: ["Content-Type": "application/x-norito"],
            body: Data(
                repeating: 0xa5,
                count: KagemushaOperationCodec.statusMaximumArchiveBytes
            )
        )
        do {
            _ = try await makeClient().getKagemushaOperationStatus(
                operationId: operationId,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
            XCTFail("exact-limit non-Norito bytes must reach the codec")
        } catch let error as KagemushaOperationError {
            XCTAssertEqual(error, .invalidNoritoArchive)
        }

        install(
            status: 404,
            headers: [
                "Content-Type": "application/x-norito",
                "x-iroha-reject-code": "offline_operation_not_found",
            ],
            body: Data(
                repeating: 0xa5,
                count: KagemushaOperationCodec.statusMaximumArchiveBytes + 1
            )
        )
        do {
            _ = try await makeClient().getKagemushaOperationStatus(
                operationId: operationId,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
            XCTFail("oversized 404 must fail before absence classification")
        } catch let error as ToriiClientError {
            guard case .invalidPayload = error else {
                return XCTFail("unexpected oversized 404 error: \(error)")
            }
            XCTAssertFalse(
                KagemushaOperationFinalityCoordinator
                    .statusResourceIsMissing(after: error)
            )
        }

        let request = try KagemushaTopUpRequest(
            noritoArchive: kagemushaOperationRequestArchive(
                schema: KagemushaRecursiveSpend.topUpRequestWireName,
                fieldCount: 8,
                operationIdFieldIndex: 6
            )
        )

        install(
            status: 202,
            headers: [
                "Content-Type": "application/x-norito",
                "Location": "/v1/offline/operations/\(operationId)",
                "Content-Length": String(
                    KagemushaOperationCodec.referenceMaximumArchiveBytes + 1
                ),
            ],
            body: Data([0x00])
        )
        await assertToriiInvalidPayload(contains: "declares more than") {
            _ = try await self.makeClient().submitKagemushaTopUp(request)
        }

        install(
            status: 202,
            headers: [
                "Content-Type": "application/x-norito",
                "Location": "/v1/offline/operations/\(operationId)",
            ],
            body: Data(
                repeating: 0xa5,
                count: KagemushaOperationCodec.referenceMaximumArchiveBytes + 1
            )
        )
        await assertToriiInvalidPayload(contains: "response exceeded") {
            _ = try await self.makeClient().submitKagemushaTopUp(request)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testOfflineSubmissionRejectsUnboundReferencesMediaTypesAndLocations() async throws {
        let submittedOperationId = String(repeating: "11", count: 32)
        let otherOperationId = String(repeating: "33", count: 32)
        let transactionHash = String(repeating: "22", count: 32)
        let request = try KagemushaTopUpRequest(
            noritoArchive: kagemushaOperationRequestArchive(
                schema: KagemushaRecursiveSpend.topUpRequestWireName,
                fieldCount: 8,
                operationIdFieldIndex: 6
            )
        )

        func reference(operationId: String, kind: KagemushaOperationKind) throws -> Data {
            KagemushaOperationCodec.encodeReference(try KagemushaOperationReference(
                operationId: operationId,
                kind: kind,
                state: .pending,
                transactionHash: transactionHash,
                statusUri: "/v1/offline/operations/\(operationId)",
                submittedAtMs: 1
            ))
        }

        let cases: [(Data, [String: String], String)] = [
            (
                try reference(operationId: otherOperationId, kind: .topUp),
                [
                    "Content-Type": "application/x-norito",
                    "Location": "/v1/offline/operations/\(submittedOperationId)",
                ],
                "does not match the submitted command"
            ),
            (
                try reference(operationId: submittedOperationId, kind: .redeem),
                [
                    "Content-Type": "application/x-norito",
                    "Location": "/v1/offline/operations/\(submittedOperationId)",
                ],
                "does not match the submitted command"
            ),
            (
                try reference(operationId: submittedOperationId, kind: .topUp),
                ["Content-Type": "application/x-norito"],
                "Location must match"
            ),
            (
                try reference(operationId: submittedOperationId, kind: .topUp),
                [
                    "Content-Type": "application/x-norito",
                    "Location": "/v1/offline/operations/\(otherOperationId)",
                ],
                "Location must match"
            ),
            (
                try reference(operationId: submittedOperationId, kind: .topUp),
                [
                    "Content-Type": "application/json",
                    "Location": "/v1/offline/operations/\(submittedOperationId)",
                ],
                "Content-Type must be application/x-norito"
            ),
            (
                try reference(operationId: submittedOperationId, kind: .topUp),
                ["Location": "/v1/offline/operations/\(submittedOperationId)"],
                "Content-Type must be application/x-norito"
            ),
        ]

        for (body, headers, expectedMessage) in cases {
            StubURLProtocol.handler = { urlRequest in
                let response = HTTPURLResponse(
                    url: urlRequest.url!,
                    statusCode: 202,
                    httpVersion: nil,
                    headerFields: headers
                )!
                return (response, body)
            }
            do {
                _ = try await makeClient().submitKagemushaTopUp(request)
                XCTFail("expected unbound Offline operation response to fail")
            } catch {
                XCTAssertTrue(
                    String(describing: error).contains(expectedMessage),
                    "expected \(expectedMessage), got \(error)"
                )
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testOfflineStatusRejectsWrongResourceIdentityAndMediaType() async throws {
        let operationId = String(repeating: "11", count: 32)
        let otherOperationId = String(repeating: "33", count: 32)
        let pendingStatus = try XCTUnwrap(Data(hexString:
            "4e5254300000fb04214104df1bdcd39249bddd4db23a009600000000000000bdfee2508f80055702000000000000000000000000414031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131040000000041403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323208ffffffffffffffff"
        ))

        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/x-norito"]
            )!
            return (response, pendingStatus)
        }
        do {
            _ = try await makeClient().getKagemushaOperationStatus(
                operationId: otherOperationId,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
            XCTFail("expected operation identity mismatch to fail")
        } catch {
            XCTAssertTrue(String(describing: error).contains("operation_id does not match"))
        }

        for headers in [
            ["Content-Type": "application/json"],
            [:],
        ] {
            StubURLProtocol.handler = { request in
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: headers
                )!
                return (response, pendingStatus)
            }
            do {
                _ = try await makeClient().getKagemushaOperationStatus(
                    operationId: operationId,
                    chainDiscriminant: SccpV1.tairaI105DiscriminantV1
                )
                XCTFail("expected invalid operation media type to fail")
            } catch {
                XCTAssertTrue(
                    String(describing: error).contains(
                        "Content-Type must be application/x-norito"
                    )
                )
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testOfflineCapabilityRequiresJsonResponseMediaType() async throws {
        let payload = """
        {
          "mandatory": false,
          "cash_handoff_capability": "cash_handoff_v1",
          "required_bridge_abi_version": 22,
          "max_hops": 8,
          "ready": true,
          "assets": [],
          "blockers": []
        }
        """.data(using: .utf8)!

        for headers in [
            ["Content-Type": "application/x-norito"],
            [:],
        ] {
            StubURLProtocol.handler = { request in
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: headers
                )!
                return (response, payload)
            }
            do {
                _ = try await makeClient().getOfflineCapability()
                XCTFail("expected invalid capability media type to fail")
            } catch {
                XCTAssertTrue(
                    String(describing: error).contains("Content-Type must be application/json")
                )
            }
        }
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
    func testAuthenticationContextDoesNotRetainOnboardingCredential() {
        let authentication = ToriiClientAuthentication(headers: [
            ToriiAccountOnboardingTokenHeader.lowercased(): String(repeating: "T", count: 32)
        ])

        XCTAssertFalse(authentication.headers.keys.contains {
            $0.caseInsensitiveCompare(ToriiAccountOnboardingTokenHeader) == .orderedSame
        })
    }

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

    private func verifyingKeyDraftResponse(
        payload: Data? = nil
    ) -> Data {
        let payload = payload ?? canonicalVerifyingKeyTransactionPayload()
        return try! JSONSerialization.data(withJSONObject: [
            "submitted": false,
            "transaction_payload_b64": payload.base64EncodedString(),
            "signing_message_b64": IrohaHash.hash(payload).base64EncodedString(),
        ])
    }

    private func canonicalVerifyingKeyTransactionPayload(
        wireName: String =
            "iroha_data_model::isi::verifying_keys::RegisterVerifyingKey",
        networkId: NetworkId = TestNetworkIds.canonical,
        domainOverride: Data? = nil,
        authority: String =
            "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
        backend: String = "halo2/ipa",
        name: String = "vk_main",
        version: UInt32 = 1,
        circuitId: String = "halo2/ipa::transfer_v1",
        schemaHash: Data = Data(repeating: 0xee, count: 32),
        gasScheduleId: String? = "halo2_default",
        keyBytes: Data? = Data([0x01, 0x02, 0x03]),
        recordVersion: UInt32? = nil
    ) -> Data {
        func string(_ value: String) -> Data {
            CompactNorito.encodeString(value)
        }
        func option(_ value: Data?) -> Data {
            var writer = CompactNoritoWriter()
            guard let value else {
                writer.writeUInt8(0)
                return writer.data
            }
            writer.writeUInt8(1)
            writer.writeField(value)
            return writer.data
        }
        func byteVector(_ value: Data) -> Data {
            var writer = CompactNoritoWriter()
            writer.writeUInt64LE(UInt64(value.count))
            writer.writeBytes(value)
            return writer.data
        }
        func uint32(_ value: UInt32) -> Data {
            var writer = CompactNoritoWriter()
            writer.writeUInt32LE(value)
            return writer.data
        }
        func uint64(_ value: UInt64) -> Data {
            var writer = CompactNoritoWriter()
            writer.writeUInt64LE(value)
            return writer.data
        }
        func structure(_ fields: [Data]) -> Data {
            var writer = CompactNoritoWriter()
            for field in fields {
                writer.writeField(field)
            }
            return writer.data
        }

        let commitment: Data
        if let keyBytes {
            let backendBytes = Data(backend.utf8)
            var preimage = Data("iroha:zk:v1:vk".utf8)
            var backendLength = UInt64(backendBytes.count).bigEndian
            var keyLength = UInt64(keyBytes.count).bigEndian
            preimage.append(withUnsafeBytes(of: &backendLength) { Data($0) })
            preimage.append(backendBytes)
            preimage.append(withUnsafeBytes(of: &keyLength) { Data($0) })
            preimage.append(keyBytes)
            commitment = Data(SHA256.hash(data: preimage))
        } else {
            commitment = Data(repeating: 0x44, count: 32)
        }
        let id = structure([string(backend), string(name)])
        let inlineKey = keyBytes.map {
            structure([string(backend), byteVector($0)])
        }
        let record = structure([
            uint32(recordVersion ?? version),
            string(circuitId),
            option(nil),
            string("core"),
            uint32(backend.hasPrefix("stark/") ? 1 : 0),
            string("unknown"),
            schemaHash,
            commitment,
            uint32(UInt32(keyBytes?.count ?? 1)),
            uint32(0),
            option(gasScheduleId.map(string)),
            option(nil),
            option(nil),
            option(nil),
            option(nil),
            option(inlineKey),
            Data([1]),
        ])
        let instructionBody = structure([id, record])
        let archive = noritoEncode(
            typeName: wireName,
            payload: instructionBody,
            flags: NoritoHeader.compactLen
        )
        let instruction = structure([string(wireName), byteVector(archive)])
        var sequence = CompactNoritoWriter()
        sequence.writeUInt64LE(1)
        sequence.writeField(instruction)
        var executable = CompactNoritoWriter()
        executable.writeUInt32LE(0)
        executable.writeField(sequence.data)

        var networkDomain = CompactNoritoWriter()
        networkDomain.writeUInt32LE(0)
        networkDomain.writeField(networkId.bytes)
        let authorityPayload = try! AccountAddress.parseEncoded(authority)
            .compactNoritoAccountControllerPayload()
        var feeAuthority = CompactNoritoWriter()
        feeAuthority.writeField(uint64(0))
        feeAuthority.writeField(Data([0]))
        var feePayment = CompactNoritoWriter()
        feePayment.writeUInt32LE(0)
        feePayment.writeField(feeAuthority.data)
        var payload = CompactNoritoWriter()
        for field in [
            domainOverride ?? networkDomain.data,
            authorityPayload,
            uint64(0),
            executable.data,
            option(uint64(100_000)),
            Data([0]),
            feePayment.data,
            uint64(0),
            Data([0]),
        ] {
            payload.writeField(field)
        }
        return payload.data
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

        let metrics = try await makeClient().getRuntimeMetrics(canonicalAuth: canonicalReadAuth)
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

        let snapshot = try await makeClient().getRuntimeAbiActive(canonicalAuth: canonicalReadAuth)
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
                "proposer": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
        XCTAssertEqual(item.record.proposer, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
                "proposer": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
        let recordNorito = try canonicalVerifierRecordArchive(seed: 0x63)
        let payload = """
        {
          "id": { "backend": "halo2/ipa", "name": "vk main" },
          "record_norito_base64": "\(recordNorito.base64EncodedString())",
          "record": {
            "version": 2,
            "circuit_id": "halo2/ipa::transfer_v2",
            "owner_manifest_id": "manifest-v2",
            "namespace": "offline_kagemusha",
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
        XCTAssertEqual(detail.record.ownerManifestId, "manifest-v2")
        XCTAssertEqual(detail.record.namespace, "offline_kagemusha")
        XCTAssertEqual(detail.record.publicInputsSchemaHashHex,
                       "fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3")
        XCTAssertEqual(detail.record.inlineKey?.backend, "halo2/ipa")
        XCTAssertEqual(detail.record.inlineKey?.bytes, Data([0x01, 0x02, 0x03]))
        XCTAssertEqual(detail.recordNorito, recordNorito)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetVerifyingKeyRejectsCrossWiredDetail() async throws {
        let recordNorito = try canonicalVerifierRecordArchive(seed: 0x64)
        let payload = """
        {
          "id": { "backend": "halo2/ipa", "name": "different-vk" },
          "record_norito_base64": "\(recordNorito.base64EncodedString())",
          "record": {
            "version": 3,
            "circuit_id": "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4",
            "owner_manifest_id": "confidential-v3",
            "namespace": "offline_kagemusha",
            "backend": "halo2/ipa",
            "curve": "pallas",
            "public_inputs_schema_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "commitment": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "vk_len": 96,
            "max_proof_bytes": 196608,
            "status": "Active"
          }
        }
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertTrue(request.url!.absoluteString.contains("/v1/zk/vk/halo2%2Fipa/unshield-v3"))
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, payload)
        }

        do {
            _ = try await makeClient().getVerifyingKey(
                backend: "halo2/ipa",
                name: "unshield-v3"
            )
            XCTFail("cross-wired verifier detail must be rejected")
        } catch let error as ToriiClientError {
            guard case let .invalidPayload(reason) = error else {
                return XCTFail("unexpected Torii error: \(error)")
            }
            XCTAssertEqual(
                reason,
                "verifying-key detail identifier does not match the requested verifier key"
            )
        }
    }

    func testVerifyingKeyDetailPreservesExactNoritoRecord() throws {
        let recordNorito = try canonicalVerifierRecordArchive(seed: 0x71)
        let payload = """
        {
          "id": { "backend": "halo2/ipa", "name": "unshield-v3" },
          "record_norito_base64": "\(recordNorito.base64EncodedString())",
          "record": {
            "version": 3,
            "circuit_id": "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4",
            "owner_manifest_id": "confidential-v3",
            "namespace": "offline_kagemusha",
            "backend": "halo2/ipa",
            "curve": "pallas",
            "public_inputs_schema_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "commitment": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "vk_len": 96,
            "max_proof_bytes": 196608,
            "status": "Active"
          }
        }
        """.data(using: .utf8)!

        let detail = try JSONDecoder().decode(ToriiVerifyingKeyDetail.self, from: payload)
        XCTAssertEqual(detail.id.backend, "halo2/ipa")
        XCTAssertEqual(detail.id.name, "unshield-v3")
        XCTAssertEqual(detail.recordNorito, recordNorito)
    }

    func testVerifyingKeyDetailRejectsNoncanonicalRecordNoritoBase64() throws {
        let recordNorito = try canonicalVerifierRecordArchive(
            seed: 0x79,
            verifierKeyLength: 96
        )
        let canonical = recordNorito.base64EncodedString()
        XCTAssertTrue(canonical.hasSuffix("="))

        func payload(recordBase64: String?) -> Data {
            let archiveField = recordBase64.map {
                "\"record_norito_base64\": \"\($0)\","
            } ?? ""
            return """
            {
              "id": { "backend": "halo2/ipa", "name": "unshield-v3" },
              \(archiveField)
              "record": {
                "version": 3,
                "circuit_id": "unshield-v3",
                "backend": "halo2/ipa",
                "curve": "pallas",
                "public_inputs_schema_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "commitment": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "vk_len": 96,
                "max_proof_bytes": 196608,
                "status": "Active"
              }
            }
            """.data(using: .utf8)!
        }

        for noncanonical in ["", " \(canonical)", String(canonical.dropLast())] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiVerifyingKeyDetail.self,
                    from: payload(recordBase64: noncanonical)
                )
            ) { error in
                XCTAssertTrue(String(describing: error).contains("record_norito_base64"))
            }
        }

        let legacyDetail = try JSONDecoder().decode(
            ToriiVerifyingKeyDetail.self,
            from: payload(recordBase64: nil)
        )
        XCTAssertNil(legacyDetail.recordNorito)
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
    func testVerifyingKeyReadPathsRejectUnsupportedProductionBackendsBeforeRequest() async {
        var requests = 0
        StubURLProtocol.handler = { request in
            requests += 1
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, Data("[]".utf8))
        }

        for backend in [
            " halo2/ipa",
            "halo2/ipa ",
            "\thalo2/ipa",
            "halo2/ipa\n",
            "halo2\u{FF0F}ipa",
            "halo2/\u{200B}ipa",
            "h\u{0430}lo2/ipa",
            "stark/fri/miden",
            " stark/fri/sha256-goldilocks",
            "stark/fri/sha256-goldilocks ",
            "halo2/ipa/orchard",
            "halo2/kzg",
            "halo2/ipa\0",
            "mock/dev"
        ] {
            await XCTAssertThrowsErrorAsync(
                try await makeClient().getVerifyingKey(backend: backend, name: "vk_main")
            ) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload, got \(error)")
                }
                XCTAssertEqual(reason, expectedVerifierRegistryBackendRejection(backend))
            }
            await XCTAssertThrowsErrorAsync(
                try await makeClient().listVerifyingKeys(query: ToriiVerifyingKeyListQuery(backend: backend))
            ) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload, got \(error)")
                }
                XCTAssertEqual(reason, expectedVerifierRegistryBackendRejection(backend))
            }
            XCTAssertThrowsError(try ToriiVerifyingKeyListQuery(backend: backend).queryItems()) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload, got \(error)")
                }
                XCTAssertEqual(reason, expectedVerifierRegistryBackendRejection(backend))
            }
        }
        XCTAssertEqual(requests, 0)
    }

    func testVerifyingKeyListDecodingRejectsNonCanonicalResponseBackends() {
        let record = """
        {
          "version": 1,
          "circuit_id": "halo2/ipa::transfer_v1",
          "backend": "halo2/ipa",
          "curve": "pallas",
          "public_inputs_schema_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          "commitment": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
          "vk_len": 32,
          "max_proof_bytes": 4096,
          "status": "Active"
        }
        """
        let payloads = [
            #"[{ "backend": " halo2/ipa", "name": "flat_vk" }]"#,
            #"[{ "id": { "backend": "halo2/ipa ", "name": "object_vk" } }]"#,
            #"[{ "backend": "halo2\uFF0Fipa", "name": "fullwidth_slash_vk" }]"#,
            #"[{ "id": { "backend": "h\u0430lo2/ipa", "name": "cyrillic_a_vk" } }]"#,
            """
            [{
              "id": { "backend": "halo2/ipa", "name": "record_vk" },
              "record": {
                "version": 1,
                "circuit_id": "halo2/ipa::transfer_v1",
                "backend": "\\thalo2/ipa",
                "curve": "pallas",
                "public_inputs_schema_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "commitment": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "vk_len": 32,
                "max_proof_bytes": 4096,
                "status": "Active"
              }
            }]
            """,
            """
            [{
              "id": { "backend": "halo2/ipa", "name": "inline_vk" },
              "record": \(record.dropLast()) ,
                "key": { "backend": "halo2/ipa\\n", "bytes_b64": "AQID" }
              }
            }]
            """,
            """
            [{
              "id": { "backend": "halo2/ipa", "name": "zero_width_vk" },
              "record": \(record.dropLast()) ,
                "key": { "backend": "halo2/\\u200Bipa", "bytes_b64": "AQID" }
              }
            }]
            """
        ]
        let canonicalDiagnostics = [
            "backend is not an exact supported verifier-registry label:",
            "backend must not contain surrounding whitespace"
        ]
        for payload in payloads {
            let data = payload.data(using: .utf8)!
            XCTAssertThrowsError(try JSONDecoder().decode([ToriiVerifyingKeyListItem].self, from: data)) { error in
                let description = String(describing: error)
                XCTAssertEqual(
                    canonicalDiagnostics.filter { description.contains($0) }.count,
                    1,
                    description
                )
            }
        }
    }

    func testVerifyingKeyListDecodingRejectsPaddedSelectorMetadata() {
        let payloads = [
            """
            [{
              "id": { "backend": "halo2/ipa", "name": " vk_main" },
              "record": {
                "version": 1,
                "circuit_id": "halo2/ipa::transfer_v1",
                "backend": "halo2/ipa",
                "curve": "pallas",
                "public_inputs_schema_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "commitment": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "vk_len": 32,
                "max_proof_bytes": 4096,
                "gas_schedule_id": "halo2_default",
                "status": "Active"
              }
            }]
            """,
            """
            [{
              "id": { "backend": "halo2/ipa", "name": "vk_main" },
              "record": {
                "version": 1,
                "circuit_id": " halo2/ipa::transfer_v1",
                "backend": "halo2/ipa",
                "curve": "pallas",
                "public_inputs_schema_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "commitment": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "vk_len": 32,
                "max_proof_bytes": 4096,
                "gas_schedule_id": "halo2_default",
                "status": "Active"
              }
            }]
            """,
            """
            [{
              "id": { "backend": "halo2/ipa", "name": "vk_main" },
              "record": {
                "version": 1,
                "circuit_id": "halo2/ipa::transfer_v1",
                "backend": "halo2/ipa",
                "curve": "pallas",
                "public_inputs_schema_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "commitment": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "vk_len": 32,
                "max_proof_bytes": 4096,
                "gas_schedule_id": "halo2_default ",
                "status": "Active"
              }
            }]
            """
        ]

        for payload in payloads {
            let data = payload.data(using: .utf8)!
            XCTAssertThrowsError(try JSONDecoder().decode([ToriiVerifyingKeyListItem].self, from: data)) { error in
                XCTAssertTrue(String(describing: error).contains("surrounding whitespace"))
            }
        }
    }

    func testVerifyingKeyDetailDecodingRejectsWithdrawHeightBeforeActivationHeight() {
        let payload = """
        {
          "id": { "backend": "halo2/ipa", "name": "vk_main" },
          "record": {
            "version": 2,
            "circuit_id": "halo2/ipa::transfer_v2",
            "backend": "halo2/ipa",
            "curve": "pallas",
            "public_inputs_schema_hash": "fae4cbe786f280b4e2184dbb06305fe46b7aee20464c0be96023ffd8eac064d3",
            "commitment": "20574662a58708e02e0000000000000000000000000000000000000000000000",
            "vk_len": 96,
            "max_proof_bytes": 8192,
            "gas_schedule_id": "halo2_default",
            "activation_height": 10,
            "withdraw_height": 9,
            "status": "Active"
          }
        }
        """.data(using: .utf8)!
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiVerifyingKeyDetail.self, from: payload)) { error in
            XCTAssertTrue(String(describing: error).contains("withdraw_height"))
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRegisterVerifyingKeyReturnsUnsignedTransactionDraft() async throws {
        let requestBody = ToriiVerifyingKeyRegisterRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            gasScheduleId: "halo2_default",
            verifyingKeyBytes: Data([0x01, 0x02, 0x03])
        )
        var didSendRequest = false
        StubURLProtocol.handler = { request in
            didSendRequest = true
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/zk/vk/register")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            let body = self.bodyJSON(from: request)
            XCTAssertEqual(body["authority"] as? String, requestBody.authority)
            XCTAssertNil(body["private_key"])
            XCTAssertEqual(body["backend"] as? String, "halo2/ipa")
            XCTAssertEqual(body["name"] as? String, "vk_main")
            XCTAssertEqual(body["vk_bytes"] as? String, "AQID")
            XCTAssertEqual(body["vk_len"] as? Int, 3)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, self.verifyingKeyDraftResponse())
        }
        let draft = try await makeClient().registerVerifyingKey(requestBody)
        XCTAssertFalse(draft.submitted)
        XCTAssertEqual(draft.transactionPayload, canonicalVerifyingKeyTransactionPayload())
        XCTAssertEqual(draft.signingMessage, IrohaHash.hash(draft.transactionPayload))
        XCTAssertTrue(didSendRequest)
    }

    func testRegisterVerifyingKeyRejectsInvalidSchemaHash() {
        let requestBody = ToriiVerifyingKeyRegisterRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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

    @available(iOS 15.0, macOS 12.0, *)
    func testVerifyingKeyTransactionDraftRejectsMalformedOrRetiredFields() async throws {
        let request = ToriiVerifyingKeyRegisterRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: String(repeating: "e", count: 64),
            gasScheduleId: "halo2_default",
            verifyingKeyBytes: Data([0x01, 0x02, 0x03])
        )
        let payload = canonicalVerifyingKeyTransactionPayload()
        let payloadB64 = payload.base64EncodedString()
        let signingB64 = IrohaHash.hash(payload).base64EncodedString()
        let cases: [(String, [String: Any])] = [
            (
                "submitted transaction",
                [
                    "submitted": true,
                    "transaction_payload_b64": payloadB64,
                    "signing_message_b64": signingB64,
                ]
            ),
            (
                "retired private key",
                [
                    "submitted": false,
                    "transaction_payload_b64": payloadB64,
                    "signing_message_b64": signingB64,
                    "private_key": "ed25519:must-not-be-accepted",
                ]
            ),
            (
                "missing signing message",
                [
                    "submitted": false,
                    "transaction_payload_b64": payloadB64,
                ]
            ),
            (
                "noncanonical payload base64",
                [
                    "submitted": false,
                    "transaction_payload_b64": "AQI",
                    "signing_message_b64": signingB64,
                ]
            ),
            (
                "noncanonical transaction payload",
                [
                    "submitted": false,
                    "transaction_payload_b64": Data([0x01, 0x02, 0x03]).base64EncodedString(),
                    "signing_message_b64": IrohaHash.hash(Data([0x01, 0x02, 0x03])).base64EncodedString(),
                ]
            ),
            (
                "wrong-size signing message",
                [
                    "submitted": false,
                    "transaction_payload_b64": payloadB64,
                    "signing_message_b64": Data(repeating: 0x01, count: 31).base64EncodedString(),
                ]
            ),
            (
                "mismatched signing message",
                [
                    "submitted": false,
                    "transaction_payload_b64": payloadB64,
                    "signing_message_b64": Data(repeating: 0x02, count: 32).base64EncodedString(),
                ]
            ),
        ]

        for (label, object) in cases {
            let data = try JSONSerialization.data(withJSONObject: object)
            StubURLProtocol.handler = { request in
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!
                return (response, data)
            }
            await XCTAssertThrowsErrorAsync(
                try await makeClient().registerVerifyingKey(request),
                label
            ) { _ in }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testVerifyingKeyDraftRequiresImmutableLocalSigningContextBeforeDispatch() async {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let client = ToriiClient(
            baseURL: URL(string: "https://example.test")!,
            session: session
        )
        let request = ToriiVerifyingKeyRegisterRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: String(repeating: "e", count: 64),
            gasScheduleId: "halo2_default",
            verifyingKeyBytes: Data([0x01, 0x02, 0x03])
        )
        var didDispatch = false
        StubURLProtocol.handler = { request in
            didDispatch = true
            return (
                HTTPURLResponse(url: request.url!, statusCode: 500,
                                httpVersion: nil, headerFields: nil)!,
                Data()
            )
        }

        await XCTAssertThrowsErrorAsync(
            try await client.registerVerifyingKey(request)
        ) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload error, got \(error)")
            }
            XCTAssertTrue(reason.contains("ToriiLocalSigningContext"))
        }
        XCTAssertFalse(didDispatch)
    }

    func testLocalSigningContextPinsNominalNetworkId() {
        let context = ToriiLocalSigningContext(networkId: TestNetworkIds.canonical)

        XCTAssertEqual(context.networkId, TestNetworkIds.canonical)
        XCTAssertEqual(context.networkId.bytes, TestNetworkIds.canonical.bytes)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRegisterVerifyingKeyRejectsSemanticallySubstitutedDrafts() async throws {
        let request = ToriiVerifyingKeyRegisterRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: String(repeating: "e", count: 64),
            gasScheduleId: "halo2_default",
            verifyingKeyBytes: Data([0x01, 0x02, 0x03])
        )
        let cases: [(label: String, payload: Data, expectedReason: String)] = [
            (
                "wrong operation",
                canonicalVerifyingKeyTransactionPayload(
                    wireName: "iroha_data_model::isi::verifying_keys::UpdateVerifyingKey"
                ),
                "requested registry operation"
            ),
            (
                "wrong identifier",
                canonicalVerifyingKeyTransactionPayload(name: "vk_substituted"),
                "identifier does not match"
            ),
            (
                "wrong full record",
                canonicalVerifyingKeyTransactionPayload(recordVersion: 2),
                "full requested record"
            ),
            (
                "wrong network",
                canonicalVerifyingKeyTransactionPayload(networkId: TestNetworkIds.other),
                "configured network"
            ),
            (
                "wrong authority",
                canonicalVerifyingKeyTransactionPayload(
                    authority: "sorauﾛ1NfｺｷﾘcﾙｦEﾑgsKti4Zﾘ6HKｳZCﾅｸｼ16fvSｲymｶｻﾘﾎ29JNWE"
                ),
                "requested authority"
            ),
        ]

        for item in cases {
            StubURLProtocol.handler = { request in
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!
                return (
                    response,
                    self.verifyingKeyDraftResponse(payload: item.payload)
                )
            }
            await XCTAssertThrowsErrorAsync(
                try await makeClient().registerVerifyingKey(request),
                item.label
            ) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload error, got \(error)")
                }
                XCTAssertTrue(reason.contains(item.expectedReason), "\(item.label): \(reason)")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRegisterVerifyingKeyRejectsGenesisUnknownLegacyAndUnmarkedTransactionDomains() async throws {
        let request = ToriiVerifyingKeyRegisterRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: String(repeating: "e", count: 64),
            gasScheduleId: "halo2_default",
            verifyingKeyBytes: Data([0x01, 0x02, 0x03])
        )
        func domain(kind: UInt32, value: Data? = nil, trailing: Data = Data()) -> Data {
            var writer = CompactNoritoWriter()
            writer.writeUInt32LE(kind)
            if let value {
                writer.writeField(value)
            }
            writer.writeBytes(trailing)
            return writer.data
        }
        var unmarked = TestNetworkIds.canonical.bytes
        unmarked[unmarked.index(before: unmarked.endIndex)] &= 0xfe
        var legacyChain = CompactNoritoWriter()
        legacyChain.writeField(CompactNorito.encodeString("test-chain"))
        let cases: [(label: String, domain: Data, expectedReason: String)] = [
            ("genesis", domain(kind: 1), "TransactionDomain::Network"),
            ("unknown", domain(kind: 2), "TransactionDomain::Network"),
            ("legacy chain", legacyChain.data, "TransactionDomain::Network"),
            ("unmarked network", domain(kind: 0, value: unmarked), "invalid canonical NetworkId"),
            (
                "trailing network data",
                domain(kind: 0, value: TestNetworkIds.canonical.bytes, trailing: Data([0])),
                "exactly one NetworkId"
            ),
        ]

        for item in cases {
            StubURLProtocol.handler = { request in
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!
                return (
                    response,
                    self.verifyingKeyDraftResponse(
                        payload: self.canonicalVerifyingKeyTransactionPayload(
                            domainOverride: item.domain
                        )
                    )
                )
            }
            await XCTAssertThrowsErrorAsync(
                try await makeClient().registerVerifyingKey(request),
                item.label
            ) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload error, got \(error)")
                }
                XCTAssertTrue(reason.contains(item.expectedReason), "\(item.label): \(reason)")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testFeeQuoteSendsOnlyCanonicalNetworkTransactionDomain() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/fees/quote")
            let body = self.bodyJSON(from: request)
            let payload = body["payload"] as? [String: Any]
            let domain = payload?["domain"] as? [String: Any]
            XCTAssertEqual(domain?["kind"] as? String, "network")
            XCTAssertEqual(
                domain?["value"] as? String,
                TestNetworkIds.canonical.literal
            )
            XCTAssertEqual(Set(domain?.keys.map { $0 } ?? []), Set(["kind", "value"]))
            XCTAssertNil(payload?["chain"])
            XCTAssertNil(payload?["chainId"])
            XCTAssertNil(payload?["chain_id"])
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            let responseBody = Data(
                """
                {
                  "intent":{"payer":"authority","value":{"charge_limits":[],"gas_limit":null}},
                  "observation":{"ledger_time_ms":1,"next_block_height":1,"route_dataspace_id":0},
                  "components":[],
                  "capacities":[],
                  "decision":{
                    "status":"accepted",
                    "value":{"debit_source":{"kind":"account","value":"\(self.authority)"}}
                  }
                }
                """.utf8
            )
            return (response, responseBody)
        }

        let quote = try await makeClient().quoteFees(
            unsignedPayload: try canonicalUnsignedFeePayload(),
            canonicalAuth: canonicalReadAuth
        )

        XCTAssertEqual(quote.observation.nextBlockHeight, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testFeeQuoteRejectsRetiredIdentityAliasesAndNonNetworkDomainsBeforeDispatch() async throws {
        let invalidDomains: [(label: String, value: ToriiJSONValue)] = [
            ("genesis", .object(["kind": .string("genesis")])),
            ("unknown kind", .object([
                "kind": .string("unknown"),
                "value": .string(TestNetworkIds.canonical.literal),
            ])),
            ("unmarked alias", .object([
                "kind": .string("network"),
                "value": .string("hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91148#A2F0"),
            ])),
            ("extra field", .object([
                "kind": .string("network"),
                "value": .string(TestNetworkIds.canonical.literal),
                "chain": .string("legacy"),
            ])),
        ]

        for item in invalidDomains {
            await assertToriiInvalidPayload(contains: "domain") {
                _ = try await makeClient().quoteFees(
                    unsignedPayload: try canonicalUnsignedFeePayload(domain: item.value),
                    canonicalAuth: canonicalReadAuth
                )
            }
        }
        for alias in ["chain", "chainId", "chain_id"] {
            var payload = try canonicalUnsignedFeePayload()
            payload[alias] = .string("legacy")
            await assertToriiInvalidPayload(contains: alias) {
                _ = try await makeClient().quoteFees(
                    unsignedPayload: payload,
                    canonicalAuth: canonicalReadAuth
                )
            }
        }
        var missingAttachments = try canonicalUnsignedFeePayload()
        missingAttachments.removeValue(forKey: "attachments")
        await assertToriiInvalidPayload(contains: "exact TransactionPayload field set") {
            _ = try await makeClient().quoteFees(
                unsignedPayload: missingAttachments,
                canonicalAuth: canonicalReadAuth
            )
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRegisterVerifyingKeyRequiresHttp200AndUniqueResponseFields() async throws {
        let request = ToriiVerifyingKeyRegisterRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: String(repeating: "a", count: 64),
            gasScheduleId: "halo2_default",
            verifyingKeyBytes: Data([0x01])
        )
        StubURLProtocol.handler = { urlRequest in
            let response = HTTPURLResponse(
                url: urlRequest.url!,
                statusCode: 202,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, self.verifyingKeyDraftResponse())
        }
        await XCTAssertThrowsErrorAsync(
            try await makeClient().registerVerifyingKey(request)
        ) { error in
            guard case let ToriiClientError.httpStatus(code, _, _) = error else {
                return XCTFail("Expected HTTP status error, got \(error)")
            }
            XCTAssertEqual(code, 202)
        }

        StubURLProtocol.handler = { urlRequest in
            let response = HTTPURLResponse(
                url: urlRequest.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            let payload = self.canonicalVerifyingKeyTransactionPayload()
            let signing = IrohaHash.hash(payload)
            let duplicate = """
            {"submitted":false,"submitted":false,"transaction_payload_b64":"\(payload.base64EncodedString())","signing_message_b64":"\(signing.base64EncodedString())"}
            """
            return (response, Data(duplicate.utf8))
        }
        await XCTAssertThrowsErrorAsync(
            try await makeClient().registerVerifyingKey(request)
        ) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error, got \(error)")
            }
        }
    }

    func testRegisterVerifyingKeyRejectsPaddedSelectorMetadata() {
        let base = ToriiVerifyingKeyRegisterRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: String(repeating: "a", count: 64),
            gasScheduleId: "halo2_default",
            verifyingKeyBytes: Data([0x01, 0x02])
        )

        var paddedName = base
        paddedName.name = " vk_main"
        XCTAssertThrowsError(try JSONEncoder().encode(paddedName)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload error")
            }
            XCTAssertTrue(reason.contains("name"))
            XCTAssertTrue(reason.contains("surrounding whitespace"))
        }

        var paddedCircuit = base
        paddedCircuit.circuitId = "halo2/ipa::transfer_v1 "
        XCTAssertThrowsError(try JSONEncoder().encode(paddedCircuit)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload error")
            }
            XCTAssertTrue(reason.contains("circuit_id"))
            XCTAssertTrue(reason.contains("surrounding whitespace"))
        }

        var paddedGasSchedule = base
        paddedGasSchedule.gasScheduleId = " halo2_default"
        XCTAssertThrowsError(try JSONEncoder().encode(paddedGasSchedule)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload error")
            }
            XCTAssertTrue(reason.contains("gas_schedule_id"))
            XCTAssertTrue(reason.contains("surrounding whitespace"))
        }

        XCTAssertThrowsError(try ToriiVerifyingKeyId(backend: "halo2/ipa", name: "vk_main ")) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload error")
            }
            XCTAssertTrue(reason.contains("name"))
            XCTAssertTrue(reason.contains("surrounding whitespace"))
        }
    }

    func testRegisterVerifyingKeyRejectsVkLengthMismatch() {
        var requestBody = ToriiVerifyingKeyRegisterRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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

    func testRegisterVerifyingKeyRejectsLengthOnlyVerifierMaterial() {
        var requestBody = ToriiVerifyingKeyRegisterRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: String(repeating: "a", count: 64),
            gasScheduleId: "halo2_default"
        )
        requestBody.verifyingKeyLength = 3
        XCTAssertThrowsError(try JSONEncoder().encode(requestBody)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload error")
            }
            XCTAssertTrue(reason.contains("commitment_hex"))
        }
    }

    func testVerifyingKeyRequestsRejectMismatchedInlineCommitment() {
        let bytes = Data([0x01, 0x02, 0x03])
        var registerRequest = ToriiVerifyingKeyRegisterRequest(
            authority: "alice",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: String(repeating: "a", count: 64),
            gasScheduleId: "halo2_default",
            verifyingKeyBytes: bytes
        )
        registerRequest.commitmentHex = String(repeating: "0", count: 64)
        XCTAssertThrowsError(try JSONEncoder().encode(registerRequest)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload error")
            }
            XCTAssertTrue(reason.contains("commitment_hex must match domain-separated SHA-256"))
        }

        var updateRequest = ToriiVerifyingKeyUpdateRequest(
            authority: "alice",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 2,
            circuitId: "halo2/ipa::transfer_v2",
            publicInputsSchemaHashHex: String(repeating: "b", count: 64)
        )
        updateRequest.verifyingKeyBytes = bytes
        updateRequest.commitmentHex = matchingVerifyingKeyCommitmentHex(
            backend: "halo2/ipa",
            bytes: bytes
        )
        XCTAssertNoThrow(try JSONEncoder().encode(updateRequest))
    }

    func testVerifyingKeyRequestsRejectWithdrawHeightBeforeActivationHeight() {
        let bytes = Data([0x01, 0x02, 0x03])
        var registerRequest = ToriiVerifyingKeyRegisterRequest(
            authority: "alice",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 1,
            circuitId: "halo2/ipa::transfer_v1",
            publicInputsSchemaHashHex: String(repeating: "a", count: 64),
            gasScheduleId: "halo2_default",
            verifyingKeyBytes: bytes
        )
        registerRequest.activationHeight = 10
        registerRequest.withdrawHeight = 9
        XCTAssertThrowsError(try JSONEncoder().encode(registerRequest)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload error")
            }
            XCTAssertTrue(reason.contains("withdraw_height"))
        }

        var updateRequest = ToriiVerifyingKeyUpdateRequest(
            authority: "alice",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 2,
            circuitId: "halo2/ipa::transfer_v2",
            publicInputsSchemaHashHex: String(repeating: "b", count: 64)
        )
        updateRequest.verifyingKeyBytes = bytes
        updateRequest.activationHeight = 10
        updateRequest.withdrawHeight = 9
        XCTAssertThrowsError(try JSONEncoder().encode(updateRequest)) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("Expected invalidPayload error")
            }
            XCTAssertTrue(reason.contains("withdraw_height"))
        }
    }

    func testVerifyingKeyRequestsRejectUnsupportedProductionBackendsBeforeEncoding() {
        let unsupported = [
            "halo2/unknown-native-v1",
            "halo2/ipa:unknown-native-v1",
            "stark/unknown-native-v1",
            " halo2/ipa",
            "halo2/ipa ",
            "\thalo2/ipa",
            "halo2/ipa\n",
            "halo2\u{FF0F}ipa",
            "halo2/\u{200B}ipa",
            "h\u{0430}lo2/ipa",
            "stark/fri/miden",
            " stark/fri/sha256-goldilocks",
            "stark/fri/sha256-goldilocks ",
            "halo2/ipa/orchard",
            "halo2/kzg",
            "halo2/ipa\0",
            "mock/dev"
        ]

        for backend in unsupported {
            let registerRequest = ToriiVerifyingKeyRegisterRequest(
                authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                backend: backend,
                name: "vk_main",
                version: 1,
                circuitId: "halo2/ipa::transfer_v1",
                publicInputsSchemaHashHex: String(repeating: "a", count: 64),
                gasScheduleId: "halo2_default"
            )
            XCTAssertThrowsError(try JSONEncoder().encode(registerRequest), backend) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload error")
                }
                XCTAssertEqual(reason, expectedVerifierRegistryBackendRejection(backend))
            }

            let updateRequest = ToriiVerifyingKeyUpdateRequest(
                authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                backend: backend,
                name: "vk_main",
                version: 2,
                circuitId: "halo2/ipa::transfer_v2",
                publicInputsSchemaHashHex: String(repeating: "b", count: 64)
            )
            XCTAssertThrowsError(try JSONEncoder().encode(updateRequest), backend) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload error")
                }
                XCTAssertEqual(reason, expectedVerifierRegistryBackendRejection(backend))
            }
        }
    }

    func testVerifyingKeyRequestsRejectBlankNamesBeforeEncoding() {
        let blankNames = ["", "   ", "\t", "\n"]

        for name in blankNames {
            let registerRequest = ToriiVerifyingKeyRegisterRequest(
                authority: "alice",
                backend: "halo2/ipa",
                name: name,
                version: 1,
                circuitId: "halo2/ipa::transfer_v1",
                publicInputsSchemaHashHex: String(repeating: "a", count: 64),
                gasScheduleId: "halo2_default"
            )
            XCTAssertThrowsError(try JSONEncoder().encode(registerRequest),
                                 "register must reject blank VK name \(String(reflecting: name))") { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload error")
                }
                XCTAssertTrue(reason.contains("name must be a non-empty string"))
            }

            let updateRequest = ToriiVerifyingKeyUpdateRequest(
                authority: "alice",
                backend: "halo2/ipa",
                name: name,
                version: 2,
                circuitId: "halo2/ipa::transfer_v2",
                publicInputsSchemaHashHex: String(repeating: "b", count: 64)
            )
            XCTAssertThrowsError(try JSONEncoder().encode(updateRequest),
                                 "update must reject blank VK name \(String(reflecting: name))") { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload error")
                }
                XCTAssertTrue(reason.contains("name must be a non-empty string"))
            }
        }
    }

    func testVerifyingKeyRequestsRejectBlankAuthorityBeforeEncoding() {
        let blankValues = ["", "   ", "\t", "\n"]

        for blank in blankValues {
            let registerWithBlankAuthority = ToriiVerifyingKeyRegisterRequest(
                authority: blank,
                backend: "halo2/ipa",
                name: "vk_main",
                version: 1,
                circuitId: "halo2/ipa::transfer_v1",
                publicInputsSchemaHashHex: String(repeating: "a", count: 64),
                gasScheduleId: "halo2_default"
            )
            XCTAssertThrowsError(try JSONEncoder().encode(registerWithBlankAuthority),
                                 "register must reject blank authority \(String(reflecting: blank))") { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload error")
                }
                XCTAssertTrue(reason.contains("authority must be a non-empty string"))
            }

            let updateWithBlankAuthority = ToriiVerifyingKeyUpdateRequest(
                authority: blank,
                backend: "halo2/ipa",
                name: "vk_main",
                version: 2,
                circuitId: "halo2/ipa::transfer_v2",
                publicInputsSchemaHashHex: String(repeating: "b", count: 64)
            )
            XCTAssertThrowsError(try JSONEncoder().encode(updateWithBlankAuthority),
                                 "update must reject blank authority \(String(reflecting: blank))") { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload error")
                }
                XCTAssertTrue(reason.contains("authority must be a non-empty string"))
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testUpdateVerifyingKeyReturnsUnsignedTransactionDraft() async throws {
        var requestBody = ToriiVerifyingKeyUpdateRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            backend: "halo2/ipa",
            name: "vk_main",
            version: 2,
            circuitId: "halo2/ipa::transfer_v2",
            publicInputsSchemaHashHex: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        )
        requestBody.verifyingKeyBytes = Data([0xAA])
        requestBody.commitmentHex = matchingVerifyingKeyCommitmentHex(
            backend: "halo2/ipa",
            bytes: Data([0xAA])
        )
        var didSendRequest = false
        StubURLProtocol.handler = { request in
            didSendRequest = true
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/zk/vk/update")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            let body = self.bodyJSON(from: request)
            XCTAssertEqual(body["authority"] as? String, requestBody.authority)
            XCTAssertNil(body["private_key"])
            XCTAssertEqual(body["backend"] as? String, "halo2/ipa")
            XCTAssertEqual(body["name"] as? String, "vk_main")
            XCTAssertEqual(body["commitment_hex"] as? String, requestBody.commitmentHex)
            XCTAssertEqual(body["vk_bytes"] as? String, "qg==")
            XCTAssertEqual(body["vk_len"] as? Int, 1)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let payload = self.canonicalVerifyingKeyTransactionPayload(
                wireName: "iroha_data_model::isi::verifying_keys::UpdateVerifyingKey",
                version: 2,
                circuitId: "halo2/ipa::transfer_v2",
                schemaHash: Data(repeating: 0xff, count: 32),
                gasScheduleId: nil,
                keyBytes: Data([0xAA])
            )
            return (response, self.verifyingKeyDraftResponse(payload: payload))
        }
        let draft = try await makeClient().updateVerifyingKey(requestBody)
        XCTAssertFalse(draft.submitted)
        XCTAssertEqual(
            draft.transactionPayload,
            canonicalVerifyingKeyTransactionPayload(
                wireName: "iroha_data_model::isi::verifying_keys::UpdateVerifyingKey",
                version: 2,
                circuitId: "halo2/ipa::transfer_v2",
                schemaHash: Data(repeating: 0xff, count: 32),
                gasScheduleId: nil,
                keyBytes: Data([0xAA])
            )
        )
        XCTAssertEqual(draft.signingMessage, IrohaHash.hash(draft.transactionPayload))
        XCTAssertTrue(didSendRequest)
    }

    func testUpdateVerifyingKeyRejectsInvalidCommitmentHex() {
        var requestBody = ToriiVerifyingKeyUpdateRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
    func testStreamVerifyingKeyEventsDoesNotEmitLastEventIdHeader() async throws {
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

        let stream = makeClient().streamVerifyingKeyEvents()
        var iterator = stream.makeAsyncIterator()
        let event = try await iterator.next()
        guard case .registered? = event?.event else {
            return XCTFail("Expected registered event")
        }
        let finished = try await iterator.next()
        XCTAssertNil(finished)
        XCTAssertNil(lastEventIdHeader)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamExplorerTransactionsAsync() async throws {
        let ssePayload = """
id: 1
data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","hash":"hash1","block":100,"created_at":"2025-01-01T00:00:00Z","executable":"Instructions","status":"Committed"}

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
        XCTAssertEqual(first?.authority, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

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
        XCTAssertEqual(first?.authority, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"6","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":2}

data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:01Z","kind":"Mint","r#box":{"scale":"0x01","json":{"kind":"Mint","payload":{"variant":"Asset","value":{"destination":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"1"}}}},"transaction_hash":"hash2","transaction_status":"Committed","block":10,"index":1}

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
            XCTAssertEqual(asset.senderAccountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"6","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":1}

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
        XCTAssertEqual(summary?.senderAccountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
                    "authority": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                    "authority": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"7","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash2","transaction_status":"Committed","block":11,"index":0}

data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"8","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash4","transaction_status":"Committed","block":11,"index":1}

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
                    "authority": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                                            "from": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                            "to": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                            "asset_definition": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                            "amount": "5"
                                        },
                                        {
                                            "from": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
            self.assertDecodedPath(request, contains: "/v1/accounts/sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV/assets")
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
        client.assetsPublisher(accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", limit: 2, scheduler: nil)
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
            XCTAssertNil(request.value(forHTTPHeaderField: "Last-Event-ID"))
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
    func testVerifyingKeyEventsPublisherPreservesTypedTerminalStreamError() throws {
        let ssePayload = """
event: stream_error
data: {"code":"stream_lagged","message":"The stream lost buffered events.","dropped_messages":3,"replay_available":false}

""".data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertNil(request.value(forHTTPHeaderField: "Last-Event-ID"))
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "text/event-stream"])!
            return (response, ssePayload)
        }

        let completionExpectation = expectation(description: "publisher surfaced terminal error")
        var cancellables: Set<AnyCancellable> = []
        makeClient().verifyingKeyEventsPublisher(scheduler: nil)
            .sink { completion in
                switch completion {
                case .finished:
                    XCTFail("Expected terminal stream error")
                case .failure(let clientError):
                    guard case let .stream(error) = clientError else {
                        XCTFail("Expected ToriiClientError.stream, got \(clientError)")
                        completionExpectation.fulfill()
                        return
                    }
                    XCTAssertEqual(error.code, "stream_lagged")
                    XCTAssertEqual(error.droppedMessages, 3)
                    XCTAssertFalse(error.replayAvailable)
                }
                completionExpectation.fulfill()
            } receiveValue: { _ in
                XCTFail("Terminal stream error must not yield a verifying-key event")
            }
            .store(in: &cancellables)

        waitForExpectations(timeout: 2.0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testExplorerTransactionsPublisherDeliversItems() throws {
        let ssePayload = """
data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","hash":"hash1","block":100,"created_at":"2025-01-01T00:00:00Z","executable":"Instructions","status":"Committed"}

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
data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

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
data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"6","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":1}

data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:01Z","kind":"Mint","r#box":{"scale":"0x01","json":{"kind":"Mint","payload":{"variant":"Asset","value":{"destination":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","object":"1"}}}},"transaction_hash":"hash2","transaction_status":"Committed","block":10,"index":1}

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
data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"6","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":1}

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
                    "authority": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                    "authority": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:00Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"5","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash1","transaction_status":"Committed","block":10,"index":0}

data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"7","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash2","transaction_status":"Committed","block":11,"index":0}

data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"8","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"hash4","transaction_status":"Committed","block":11,"index":1}

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
                    "authority": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                    "authority": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"7","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"deadbeef","transaction_status":"Committed","block":11,"index":0}

data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"8","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"deadbeef","transaction_status":"Committed","block":11,"index":2}

data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:02Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"9","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"otherhash","transaction_status":"Committed","block":11,"index":1}

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
                    "authority": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
                    "authority": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at": "2025-01-01T00:00:00Z",
                    "kind": "Transfer",
                    "r#box": {
                        "scale": "0x00",
                        "json": {
                            "kind": "Transfer",
                            "payload": {
                                "variant": "Asset",
                                "value": {
                                    "source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"7","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"deadbeef","transaction_status":"Committed","block":11,"index":0}

data: {"authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","created_at":"2025-01-01T00:00:01Z","kind":"Transfer","r#box":{"scale":"0x00","json":{"kind":"Transfer","payload":{"variant":"Asset","value":{"source":"61CtjvNd9T3THAR65GsMVHr82Bjc#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","object":"8","destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"}}}},"transaction_hash":"deadbeef","transaction_status":"Committed","block":11,"index":2}

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
        for backend in [
            " halo2/ipa",
            "halo2/ipa ",
            "\thalo2/ipa",
            "halo2/ipa\n",
            "stark/fri/miden",
            "halo2/ipa/orchard",
            "halo2/kzg",
            "halo2:ipa",
            "mock/dev"
        ] {
            XCTAssertThrowsError(try ToriiVerifyingKeyEventFilter(backend: backend, name: "vk").queryItems()) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload error")
                }
                XCTAssertEqual(reason, expectedVerifierRegistryBackendRejection(backend))
            }
        }

        for name in ["", "   ", "\t", "\n", "vk:main"] {
            XCTAssertThrowsError(try ToriiVerifyingKeyEventFilter(backend: "halo2/ipa", name: name).queryItems()) { error in
                guard case ToriiClientError.invalidPayload = error else {
                    return XCTFail("Expected invalidPayload error")
                }
            }
        }
    }

    func testVerifyingKeyEventFilterCanonicalizesNameBeforeEncoding() throws {
        let queryItems = try XCTUnwrap(ToriiVerifyingKeyEventFilter(backend: "halo2/ipa",
                                                                    name: " vk_main ").queryItems())
        let filterValue = try XCTUnwrap(queryItems.first { $0.name == "filter" }?.value)
        let data = try XCTUnwrap(filterValue.data(using: .utf8))
        let decoded = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let verifyingKey = try XCTUnwrap(decoded["VerifyingKey"] as? [String: Any])
        let matcher = try XCTUnwrap(verifyingKey["id_matcher"] as? [String: Any])
        XCTAssertEqual(matcher["backend"] as? String, "halo2/ipa")
        XCTAssertEqual(matcher["name"] as? String, "vk_main")
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
    func testStreamTriggerEventsDoesNotEmitLastEventIdHeader() async throws {
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

        let stream = makeClient().streamTriggerEvents()
        var iterator = stream.makeAsyncIterator()
        let event = try await iterator.next()
        guard case let .deleted(id)? = event?.event else {
            return XCTFail("Expected deleted trigger event")
        }
        XCTAssertEqual(id, "nightly-tick")
        XCTAssertNil(lastEventIdHeader)
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
    func testStreamProofEventsDoesNotEmitLastEventIdHeader() async throws {
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

        let stream = makeClient().streamProofEvents()
        var iterator = stream.makeAsyncIterator()
        let event = try await iterator.next()
        guard case .rejected? = event?.event else {
            return XCTFail("Expected rejected proof event")
        }
        XCTAssertNil(lastEventIdHeader)
        let finished = try await iterator.next()
        XCTAssertNil(finished)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamVerifyingKeyEventsSurfacesTerminalStreamError() async throws {
        let ssePayload = """
event: stream_error
data: {"code":"stream_lagged","message":"The stream lost buffered events.","dropped_messages":7,"replay_available":false}

""".data(using: .utf8)!

        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        var iterator = makeClient().streamVerifyingKeyEvents().makeAsyncIterator()
        do {
            _ = try await iterator.next()
            XCTFail("Expected terminal stream error")
        } catch let ToriiClientError.stream(error) {
            XCTAssertEqual(error.code, "stream_lagged")
            XCTAssertEqual(error.message, "The stream lost buffered events.")
            XCTAssertEqual(error.droppedMessages, 7)
            XCTAssertFalse(error.replayAvailable)
        } catch {
            XCTFail("Expected ToriiClientError.stream, got \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testStreamProofEventsSurfacesTerminalStreamErrorBeforePayloadProjection() async throws {
        let ssePayload = """
event: stream_error
data: {"code":"stream_source_closed","message":"The event source closed.","dropped_messages":null,"replay_available":false}

""".data(using: .utf8)!

        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        var iterator = makeClient().streamProofEvents().makeAsyncIterator()
        do {
            _ = try await iterator.next()
            XCTFail("Expected terminal stream error")
        } catch let ToriiClientError.stream(error) {
            XCTAssertEqual(error.code, "stream_source_closed")
            XCTAssertEqual(error.message, "The event source closed.")
            XCTAssertNil(error.droppedMessages)
            XCTAssertFalse(error.replayAvailable)
        } catch {
            XCTFail("Expected ToriiClientError.stream, got \(error)")
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testTypedEventStreamsFailClosedOnMalformedTerminalStreamErrors() async throws {
        let malformedPayloads = [
            "null",
            "{\"code\":\"stream_lagged\",\"code\":\"stream_source_closed\",\"message\":\"closed\",\"dropped_messages\":null,\"replay_available\":false}",
            "{\"code\":\"stream_lagged\",\"message\":\"closed\",\"dropped_messages\":null}",
            "{\"code\":\"stream_lagged\",\"message\":\"closed\",\"dropped_messages\":-1,\"replay_available\":false}",
            "{\"code\":\"stream_lagged\",\"message\":\"closed\",\"dropped_messages\":null,\"replay_available\":\"false\"}",
            "{\"code\":\"stream_lagged\",\"message\":\"closed\",\"dropped_messages\":null,\"replay_available\":false,\"category\":\"Proof\"}",
        ]

        for payload in malformedPayloads {
            let ssePayload = "event: stream_error\ndata: \(payload)\n\n".data(using: .utf8)!
            StubURLProtocol.handler = { request in
                let response = HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "text/event-stream"]
                )!
                return (response, ssePayload)
            }

            var iterator = makeClient().streamTriggerEvents().makeAsyncIterator()
            do {
                _ = try await iterator.next()
                XCTFail("Expected malformed terminal stream error to fail closed: \(payload)")
            } catch ToriiClientError.invalidPayload {
                // Expected.
            } catch {
                XCTFail("Expected ToriiClientError.invalidPayload, got \(error)")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testTransactionStatusStreamStillFiltersOrdinaryNonTransactionEvents() async throws {
        let ssePayload = """
data: {"event":"Block","hash":"\(Self.pipelineHash)","status":"Applied"}

data: {"event":"Transaction","hash":"\(Self.pipelineHash)","status":"Applied","block_height":17}

""".data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertNil(request.value(forHTTPHeaderField: "Last-Event-ID"))
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "text/event-stream"]
            )!
            return (response, ssePayload)
        }

        var iterator = makeClient().streamTransactionStatusEvents(hashHex: Self.pipelineHash).makeAsyncIterator()
        let event = try await iterator.next()
        XCTAssertEqual(event?.event, "Transaction")
        XCTAssertEqual(event?.blockHeight, 17)
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
        for backend in [
            " halo2/ipa",
            "halo2/ipa ",
            "\thalo2/ipa",
            "halo2/ipa\n",
            "stark/fri/miden",
            "halo2/ipa/orchard",
            "halo2/kzg",
            "halo2:ipa",
            "mock/dev"
        ] {
            XCTAssertThrowsError(try ToriiProofEventFilter(backend: backend,
                                                           proofHashHex: String(repeating: "a", count: 64),
                                                           includeVerified: true,
                                                           includeRejected: true).queryItems()) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload error")
                }
                XCTAssertEqual(reason, expectedVerifierRegistryBackendRejection(backend))
            }
        }

        for proofHashHex in [
            "",
            "abc",
            String(repeating: "z", count: 64),
            String(repeating: "a", count: 63),
            "0x" + String(repeating: "a", count: 63)
        ] {
            XCTAssertThrowsError(try ToriiProofEventFilter(backend: "halo2/ipa",
                                                           proofHashHex: proofHashHex,
                                                           includeVerified: true,
                                                           includeRejected: true).queryItems()) { error in
                guard case ToriiClientError.invalidPayload = error else {
                    return XCTFail("Expected invalidPayload error")
                }
            }
        }
    }

    func testProofEventFilterCanonicalizesHashBeforeEncoding() throws {
        let queryItems = try XCTUnwrap(ToriiProofEventFilter(backend: "halo2/ipa",
                                                             proofHashHex: "0x" + String(repeating: "A", count: 64),
                                                             includeVerified: true,
                                                             includeRejected: true).queryItems())
        let filterValue = try XCTUnwrap(queryItems.first { $0.name == "filter" }?.value)
        let data = try XCTUnwrap(filterValue.data(using: .utf8))
        let decoded = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let proof = try XCTUnwrap(decoded["Proof"] as? [String: Any])
        let matcher = try XCTUnwrap(proof["id_matcher"] as? [String: Any])
        XCTAssertEqual(matcher["backend"] as? String, "halo2/ipa")
        XCTAssertEqual(matcher["hash_hex"] as? String, String(repeating: "a", count: 64))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGetTimeStatusAsync() async throws {
        let payload = """
        {"peers":3,"samples":[{"peer":"peer1","last_offset_ms":1,"last_rtt_ms":2,"count":3}],"rtt":{"buckets":[{"le":5,"count":10}],"sum_ms":20,"count":10},"note":"NTS running"}
        """.data(using: .utf8)!

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/time/status")
            self.assertOperatorAuthentication(request)
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
    func testCoreAndPipelineOperatorReadsRejectMissingContextBeforeDispatch() async {
        StubURLProtocol.handler = { request in
            XCTFail("operator request dispatched without a signing context: \(request)")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 500,
                httpVersion: nil,
                headerFields: nil
            )!
            return (response, Data())
        }
        let client = makeClient(operatorSigningContext: nil)
        let reads: [(String, () async throws -> Void)] = [
            ("pipeline recovery", { _ = try await client.getPipelineRecovery(height: 42) }),
            ("pipeline preflight", { _ = try await client.getPipelinePreflight() }),
            ("time status", { _ = try await client.getTimeStatus() }),
        ]

        for (name, read) in reads {
            do {
                try await read()
                XCTFail("\(name) must require an operator signing context")
            } catch {
                XCTAssertTrue(String(describing: error).contains("ToriiOperatorSigningContext"))
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSumeragiOperatorReadsRejectMissingContextAndFallbackAuthBeforeDispatch() async {
        StubURLProtocol.handler = { request in
            XCTFail("operator request should not dispatch without a clean signing context")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 500,
                httpVersion: nil,
                headerFields: nil
            )!
            return (response, Data())
        }

        do {
            _ = try await makeClient(operatorSigningContext: nil).getSumeragiStatus()
            XCTFail("missing operator context must fail before dispatch")
        } catch {
            XCTAssertTrue(String(describing: error).contains("ToriiOperatorSigningContext"))
        }

        for header in ["Authorization", "X-API-Token", "X-Iroha-Operator-Signature"] {
            do {
                _ = try await makeClient(defaultHeaders: [header: "retired"])
                    .getSumeragiStatus()
                XCTFail("fallback or precomputed authentication must fail before dispatch")
            } catch {
                XCTAssertTrue(String(describing: error).contains("reject"))
            }
        }
    }

    func testGetSumeragiStatusParsesAuthoritativeV2SnapshotAsync() async throws {
        let contextHash = nativeAmxTestHash(0xA7)
        let parentHash = nativeAmxTestHash(0xB1)
        let blockHash = nativeAmxTestHash(0xB3)
        let payloadHash = nativeAmxTestHash(0xB5)
        let subject: [String: Any] = [
            "parent_block_hash": parentHash,
            "block_hash": blockHash,
            "payload_hash": payloadHash,
        ]
        let executionCommitment: [String: Any] = [
            "parent_state_root": nativeAmxTestHash(0xC1),
            "post_state_root": nativeAmxTestHash(0xC3),
            "ordinary_writes_root": nativeAmxTestHash(0xC5),
            "topup_anchor_count": 0,
            "native_amx_application_manifest_version":
                ToriiSumeragiV2ExecutionCommitment.canonicalNativeAmxApplicationManifestVersion,
            "native_amx_application_manifest_root":
                ToriiSumeragiV2ExecutionCommitment.nativeAmxApplicationManifestEmptyRoot,
            "native_amx_application_manifest_count": 0,
            "lane_finality_manifest": NSNull(),
            "merge_carrier": NSNull(),
            "executed_block_wire_len": 123,
            "executed_block_wire_hash": nativeAmxTestHash(0xC7),
        ]
        let prepareQC: [String: Any] = [
            "round": [
                "context_id": [contextHash],
                "height": 15,
                "view": 3,
            ],
            "proposal_round": [
                "context_id": [contextHash],
                "height": 15,
                "view": 3,
            ],
            "phase": ["phase": "prepare", "details": NSNull()],
            "subject": subject,
            "execution_commitment": executionCommitment,
        ]
        let commitQC: [String: Any] = [
            "round": [
                "context_id": [contextHash],
                "height": 14,
                "view": 2,
            ],
            "proposal_round": [
                "context_id": [contextHash],
                "height": 14,
                "view": 2,
            ],
            "phase": ["phase": "commit", "details": NSNull()],
            "subject": subject,
            "execution_commitment": executionCommitment,
        ]
        let payload = try JSONSerialization.data(withJSONObject: [
            "protocol_version": 4,
            "node_fingerprint": nativeAmxTestHash(0xA1),
            "build_fingerprint": nativeAmxTestHash(0xA3),
            "config_fingerprint": nativeAmxTestHash(0xA5),
            "restart_required": false,
            "height_context_id": [contextHash],
            "height": 15,
            "view": 4,
            "phase": ["phase": "commit", "details": NSNull()],
            "leader": 1,
            "locked_prepare_qc": prepareQC,
            "highest_prepare_qc": prepareQC,
            "last_timeout_certificate": [
                "round": [
                    "context_id": [contextHash],
                    "height": 15,
                    "view": 3,
                ],
                "highest_prepare_qc": prepareQC,
                "certificate_hash": nativeAmxTestHash(0xB7),
            ],
            "body_state": ["state": "validated", "details": NSNull()],
            "pending_persistence_id": 17,
            "last_committed_height": 14,
            "last_committed_subject": subject,
            "height_context": sumeragiV2TestHeightContext(),
            "last_commit_qc": [
                "certificate": commitQC,
                "validator_count": 4,
                "signer_count": 3,
                "min_signers": 3,
                "signed_power": 3,
                "total_power": 4,
            ],
            "liveness": sumeragiV2TestLiveness(),
        ])
        var servedPayload = payload
        var servedStatus = 200
        var servedHeaders = ["Content-Type": "application/json"]
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/sumeragi/status")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            self.assertOperatorAuthentication(request)
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: servedStatus,
                httpVersion: nil,
                headerFields: servedHeaders
            )!
            return (response, servedPayload)
        }
        let snapshot = try await makeClient().getSumeragiStatus()
        XCTAssertEqual(snapshot.protocolVersion, SumeragiV2ConsensusMessage.protocolVersion)
        XCTAssertEqual(snapshot.nodeFingerprint, nativeAmxTestHash(0xA1))
        XCTAssertEqual(snapshot.buildFingerprint, nativeAmxTestHash(0xA3))
        XCTAssertEqual(snapshot.configFingerprint, nativeAmxTestHash(0xA5))
        XCTAssertFalse(snapshot.restartRequired)
        XCTAssertEqual(snapshot.heightContextID.hash, contextHash)
        XCTAssertEqual(snapshot.height, 15)
        XCTAssertEqual(snapshot.view, 4)
        XCTAssertEqual(snapshot.phase, .commit)
        XCTAssertEqual(snapshot.leader, 1)
        XCTAssertEqual(snapshot.lockedPrepareQC?.round.view, 3)
        XCTAssertEqual(snapshot.lockedPrepareQC?.proposalRound.view, 3)
        XCTAssertEqual(snapshot.highestPrepareQC?.phase, .prepare)
        XCTAssertEqual(
            snapshot.highestPrepareQC?.executionCommitment.executedBlockWireLen,
            123
        )
        XCTAssertEqual(
            snapshot.highestPrepareQC?.executionCommitment.executedBlockWireHash,
            nativeAmxTestHash(0xC7)
        )
        XCTAssertEqual(
            snapshot.highestPrepareQC?.executionCommitment
                .nativeAmxApplicationManifestRoot,
            ToriiSumeragiV2ExecutionCommitment.nativeAmxApplicationManifestEmptyRoot
        )
        XCTAssertEqual(
            snapshot.lastTimeoutCertificate?.highestPrepareQC?.subject.blockHash,
            blockHash
        )
        XCTAssertEqual(snapshot.bodyState, .validated)
        XCTAssertEqual(snapshot.pendingPersistenceID, 17)
        XCTAssertEqual(snapshot.lastCommittedHeight, 14)
        XCTAssertEqual(snapshot.lastCommittedSubject?.payloadHash, payloadHash)
        XCTAssertEqual(snapshot.heightContext.validatorCount, 4)
        XCTAssertEqual(snapshot.lastCommitQC?.certificate.phase, .commit)
        XCTAssertEqual(snapshot.liveness.generation, 2)

        let invalidResponses: [(Data, Int, [String: String], String)] = [
            (
                duplicateSumeragiRootField(#"{"protocol_version":4,"#, in: payload),
                200, ["Content-Type": "application/json"], "duplicate object keys"
            ),
            (Data([0xff]), 200, ["Content-Type": "application/json"], "UTF-8 JSON"),
            (Data("{}".utf8), 200, ["Content-Type": "text/plain"], "Content-Type"),
            (Data("{}".utf8), 503, ["Content-Type": "application/json"], "503"),
            (
                Data("{}".utf8), 200,
                ["Content-Type": "application/json", "Content-Length": "1048577"], "1048576-byte limit"
            ),
            (
                Data(repeating: 0x20, count: 1_048_577), 200,
                ["Content-Type": "application/json"], "1048576-byte limit"
            ),
        ]
        for (body, statusCode, headers, errorFragment) in invalidResponses {
            servedPayload = body
            servedStatus = statusCode
            servedHeaders = headers
            do {
                _ = try await makeClient().getSumeragiStatus()
                XCTFail("invalid status response must fail closed")
            } catch {
                XCTAssertTrue(String(describing: error).contains(errorFragment))
            }
        }
    }

    func testSumeragiExecutionCommitmentRejectsNoncanonicalNativeAmxManifest() throws {
        let emptyRoot = ToriiSumeragiV2ExecutionCommitment.nativeAmxApplicationManifestEmptyRoot
        let base: [String: Any] = [
            "parent_state_root": nativeAmxTestHash(0xC1),
            "post_state_root": nativeAmxTestHash(0xC3),
            "ordinary_writes_root": nativeAmxTestHash(0xC5),
            "topup_anchor_count": 0,
            "native_amx_application_manifest_version":
                ToriiSumeragiV2ExecutionCommitment.canonicalNativeAmxApplicationManifestVersion,
            "native_amx_application_manifest_root": emptyRoot,
            "native_amx_application_manifest_count": 0,
            "lane_finality_manifest": NSNull(),
            "merge_carrier": NSNull(),
            "executed_block_wire_len": 123,
            "executed_block_wire_hash": nativeAmxTestHash(0xC7),
        ]
        func decode(_ value: [String: Any]) throws {
            _ = try JSONDecoder().decode(
                ToriiSumeragiV2ExecutionCommitment.self,
                from: JSONSerialization.data(withJSONObject: value)
            )
        }

        try decode(base)
        var wrongVersion = base
        wrongVersion["native_amx_application_manifest_version"] = 2
        XCTAssertThrowsError(try decode(wrongVersion))
        var nonemptyRootAtZero = base
        nonemptyRootAtZero["native_amx_application_manifest_root"] =
            nativeAmxTestHash(0xD1)
        XCTAssertThrowsError(try decode(nonemptyRootAtZero))
        var emptyRootAtOne = base
        emptyRootAtOne["native_amx_application_manifest_count"] = 1
        XCTAssertThrowsError(try decode(emptyRootAtOne))
        var oversized = base
        oversized["native_amx_application_manifest_count"] =
            ToriiSumeragiV2ExecutionCommitment
                .maximumNativeAmxApplicationManifestLeafCount + 1
        oversized["native_amx_application_manifest_root"] = nativeAmxTestHash(0xD1)
        XCTAssertThrowsError(try decode(oversized))
        var missingRoot = base
        missingRoot.removeValue(forKey: "native_amx_application_manifest_root")
        XCTAssertThrowsError(try decode(missingRoot))
    }

    func testSumeragiExecutionCommitmentRequiresExactMergeCarrierProjection() throws {
        let emptyRoot =
            ToriiSumeragiV2ExecutionCommitment.nativeAmxApplicationManifestEmptyRoot
        let base: [String: Any] = [
            "parent_state_root": nativeAmxTestHash(0xC1),
            "post_state_root": nativeAmxTestHash(0xC3),
            "ordinary_writes_root": nativeAmxTestHash(0xC5),
            "topup_anchor_count": 0,
            "native_amx_application_manifest_version": 1,
            "native_amx_application_manifest_root": emptyRoot,
            "native_amx_application_manifest_count": 0,
            "lane_finality_manifest": NSNull(),
            "merge_carrier": NSNull(),
            "executed_block_wire_len": 123,
            "executed_block_wire_hash": nativeAmxTestHash(0xC7),
        ]
        func decode(_ value: [String: Any]) throws -> ToriiSumeragiV2ExecutionCommitment {
            try JSONDecoder().decode(
                ToriiSumeragiV2ExecutionCommitment.self,
                from: JSONSerialization.data(withJSONObject: value)
            )
        }

        XCTAssertNil(try decode(base).mergeCarrier)
        XCTAssertEqual(try decode(base).executedBlockWireLen, 123)
        var carried = base
        carried["merge_carrier"] = [
            "version": 1,
            "entry_hash": nativeAmxTestHash(0xD1),
        ]
        XCTAssertEqual(try decode(carried).mergeCarrier?.entryHash, nativeAmxTestHash(0xD1))

        var missing = base
        missing.removeValue(forKey: "merge_carrier")
        XCTAssertThrowsError(try decode(missing))
        var malformed = base
        malformed["merge_carrier"] = "carrier"
        XCTAssertThrowsError(try decode(malformed))
        var wrongVersion = carried
        wrongVersion["merge_carrier"] = [
            "version": 2,
            "entry_hash": nativeAmxTestHash(0xD1),
        ]
        XCTAssertThrowsError(try decode(wrongVersion))
        var missingVersion = carried
        missingVersion["merge_carrier"] = [
            "entry_hash": nativeAmxTestHash(0xD1),
        ]
        XCTAssertThrowsError(try decode(missingVersion))
        var missingEntryHash = carried
        missingEntryHash["merge_carrier"] = ["version": 1]
        XCTAssertThrowsError(try decode(missingEntryHash))
        var badHash = carried
        badHash["merge_carrier"] = ["version": 1, "entry_hash": "bad"]
        XCTAssertThrowsError(try decode(badHash))
        var unknown = carried
        unknown["merge_carrier"] = [
            "version": 1,
            "entry_hash": nativeAmxTestHash(0xD1),
            "future": true,
        ]
        XCTAssertThrowsError(try decode(unknown))

        var missingWireLen = base
        missingWireLen.removeValue(forKey: "executed_block_wire_len")
        XCTAssertThrowsError(try decode(missingWireLen))
        for invalidWireLen: Any in [0, -1, true, "123", 1.5, NSNull()] {
            var malformedWireLen = base
            malformedWireLen["executed_block_wire_len"] = invalidWireLen
            XCTAssertThrowsError(try decode(malformedWireLen))
        }
    }

    func testSumeragiDiagnosticsPreservesNativeAmxV2AndNexusFeeReceipts() throws {
        let snapshot = try JSONDecoder().decode(
            ToriiSumeragiDiagnosticsSnapshot.self,
            from: nativeAmxDiagnosticsPayload()
        )

        let commitment = try XCTUnwrap(snapshot.laneSettlementCommitments.first)
        XCTAssertEqual(commitment.totalLocalAmount, "170141183460469231731687303715884105851")
        XCTAssertEqual(commitment.totalXorDue, "100.25")
        let receipt = try XCTUnwrap(commitment.nativeAmxReceipts.first)
        let firstLeg = try XCTUnwrap(receipt.legs.first)
        XCTAssertEqual(commitment.totalXorAfterHaircut, "90.2")
        XCTAssertEqual(commitment.totalXorVariance, "10.05")
        XCTAssertEqual(commitment.swapMetadata?.liquidityProfile, .tier2)
        XCTAssertEqual(commitment.swapMetadata?.volatilityClass, .stable)
        XCTAssertEqual(
            commitment.nexusFeeReceipts.first?.feeAmount,
            "18446744073709551616.25"
        )
        XCTAssertEqual(receipt.version, 2)
        XCTAssertEqual(receipt.legs.count, 2)
        XCTAssertEqual(
            receipt.networkId,
            "hash:AC23881CE29F6466B8710D9683F4F28D49E8335C63E913FACB109303298ED833#0628"
        )
        XCTAssertEqual(
            receipt.planDigest,
            "hash:98E3EE29122BBFA40FEF5FF5E694F8F695D2772A31BAADE91755A2AEA030DB93#9DCD"
        )
        XCTAssertEqual(
            receipt.laneIncarnation,
            "hash:27146A48934D7179538FC7D9F474067D761989CED98D26A46AF5EB2575A7547D#8337"
        )
        XCTAssertEqual(receipt.authorityContextHeight, 40)
        XCTAssertEqual(receipt.laneBlockHeight, 42)
        XCTAssertEqual(receipt.laneBlockView, 9)
        XCTAssertEqual(
            receipt.coordinatorProposalHash,
            "hash:AAC0F352914C21699F3F8D571196C9A5DFCAA9EF1272A7DEFA7FFD35A93C21AD#8B3F"
        )
        XCTAssertEqual(firstLeg.prepareQc.body.phase, .prepare)
        XCTAssertEqual(firstLeg.commitQc.body.phase, .commit)
        XCTAssertEqual(firstLeg.prepareQc.body.round.height, 40)
        XCTAssertEqual(firstLeg.prepareQc.body.epoch, 3)
        XCTAssertEqual(firstLeg.prepareQc.body.plannedCoordinatorBlockHeight, 42)
        XCTAssertEqual(firstLeg.prepareQc.validatorSetPops.count, 4)
        XCTAssertEqual(firstLeg.prepareQc.validatorSetPops.first?.count, 96)
        XCTAssertEqual(firstLeg.prepareQc.signersBitmap, [7])
        XCTAssertEqual(firstLeg.prepareQc.blsAggregateSignature.count, 96)
        XCTAssertEqual(
            snapshot.laneRelayEnvelopes.first?.settlementCommitment.nativeAmxReceipts,
            commitment.nativeAmxReceipts
        )
        let application = try XCTUnwrap(snapshot.nativeAmxParticipantApplications.first)
        XCTAssertEqual(application.laneID, 8)
        XCTAssertEqual(application.dataspaceID, 12)
        XCTAssertEqual(application.participantHeight, 42)
        XCTAssertEqual(application.predecessorHeight, 41)
        XCTAssertEqual(application.sourceCount, 2)
        XCTAssertEqual(application.applicationBlockHeight, 42)
        XCTAssertEqual(application.state, .durablyApplied)
    }

    func testSumeragiDiagnosticsAutonomousExecutionStagesAndConflict() throws {
        var row: [String: Any] = [
            "lane_id": 9, "dataspace_id": 13,
            "lane_incarnation": nativeAmxTestHash(0xB8),
            "lane_block_height": 8, "lane_block_view": 1,
            "proposal_height": 10, "proposal_view": 2,
            "reservation_owner_hash": nativeAmxTestHash(0x5D),
            "proposal_identity_hash": nativeAmxTestHash(0x5E),
            "reservation_group_hash": nativeAmxTestHash(0x5F),
            "proposal_hash": nativeAmxTestHash(0x60),
            "descriptor_hash": nativeAmxTestHash(0x9E),
            "executable_payload_hash": nativeAmxTestHash(0x61),
            "source_bundle_hash": nativeAmxTestHash(0x62),
            "merge_entry_hash": nativeAmxTestHash(0x63),
            "application_block_height": 42,
            "application_block_hash": nativeAmxTestHash(0xA2),
            "reservation_count": 2, "transaction_count": 2,
            "highest_durable_stage": "kura_wsv_application_receipt_durable",
            "stuck_reason": "queue_finalization_unverifiable",
        ]
        let data = try mutatedNativeAmxDiagnosticsPayload { root in
            root["autonomous_lane_executions"] = [row]
        }
        let snapshot = try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self, from: data)
        XCTAssertEqual(snapshot.autonomousLaneExecutions.first?.mergeEntryHash,
                       nativeAmxTestHash(0x63))
        XCTAssertEqual(snapshot.autonomousLaneExecutions.first?.proposalIdentityHash,
                       nativeAmxTestHash(0x5E))

        let duplicate = try mutatedNativeAmxDiagnosticsPayload { root in
            root["autonomous_lane_executions"] = [row, row]
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self, from: duplicate)
        )
        row["reservation_count"] = 1
        let mismatchedCounts = try mutatedNativeAmxDiagnosticsPayload { root in
            root["autonomous_lane_executions"] = [row]
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self,
                                     from: mismatchedCounts)
        )
        row["highest_durable_stage"] = "conflict"
        row["stuck_reason"] = "evidence_conflict"
        let conflict = try mutatedNativeAmxDiagnosticsPayload { root in
            root["autonomous_lane_executions"] = [row]
        }
        XCTAssertNoThrow(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self, from: conflict)
        )

        let missingRequiredVector = try mutatedNativeAmxDiagnosticsPayload { root in
            root.removeValue(forKey: "autonomous_lane_executions")
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: missingRequiredVector
            )
        )
    }

    func testSumeragiDiagnosticsReservationsDurableIdentityAndGeometryAreExact() throws {
        func appliedRow() -> [String: Any] {
            [
                "lane_id": 9, "dataspace_id": 13,
                "lane_incarnation": nativeAmxTestHash(0xB8),
                "lane_block_height": 8, "lane_block_view": 1,
                "proposal_height": 10, "proposal_view": 2,
                "reservation_owner_hash": nativeAmxTestHash(0x5D),
                "proposal_identity_hash": nativeAmxTestHash(0x5E),
                "reservation_group_hash": nativeAmxTestHash(0x5F),
                "proposal_hash": nativeAmxTestHash(0x60),
                "descriptor_hash": nativeAmxTestHash(0x9E),
                "executable_payload_hash": nativeAmxTestHash(0x61),
                "source_bundle_hash": nativeAmxTestHash(0x62),
                "merge_entry_hash": nativeAmxTestHash(0x63),
                "application_block_height": 42,
                "application_block_hash": nativeAmxTestHash(0xA2),
                "reservation_count": 2, "transaction_count": 2,
                "highest_durable_stage": "kura_wsv_application_receipt_durable",
                "stuck_reason": "queue_finalization_unverifiable",
            ]
        }
        func reservationRow() -> [String: Any] {
            var row = appliedRow()
            for field in [
                "proposal_view", "proposal_hash", "descriptor_hash", "executable_payload_hash",
                "source_bundle_hash", "merge_entry_hash", "application_block_height",
                "application_block_hash",
            ] {
                row.removeValue(forKey: field)
            }
            row["highest_durable_stage"] = "reservations_durable"
            row["stuck_reason"] = "awaiting_executable_payload"
            return row
        }
        func data(_ row: [String: Any]) throws -> Data {
            try mutatedNativeAmxDiagnosticsPayload { root in
                root["autonomous_lane_executions"] = [row]
            }
        }

        let reservation = try JSONDecoder().decode(
            ToriiSumeragiDiagnosticsSnapshot.self,
            from: data(reservationRow())
        ).autonomousLaneExecutions[0]
        XCTAssertNil(reservation.proposalHash)
        XCTAssertNil(reservation.descriptorHash)
        XCTAssertNil(reservation.proposalView)
        XCTAssertEqual(reservation.stuckReason, .awaitingExecutablePayload)

        for field in [
            "reservation_owner_hash", "proposal_identity_hash", "reservation_group_hash",
        ] {
            for invalid: Any in [NSNull(), "hash:" + String(repeating: "00", count: 32) + "#6A0A",
                                 [UInt8](repeating: 1, count: 32), String(repeating: "ab", count: 32)] {
                var row = appliedRow()
                row[field] = invalid
                XCTAssertThrowsError(
                    try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self,
                                             from: data(row))
                )
            }
            var missing = appliedRow()
            missing.removeValue(forKey: field)
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self,
                                         from: data(missing))
            )
        }

        for missingField in ["proposal_hash", "descriptor_hash"] {
            var row = appliedRow()
            row.removeValue(forKey: missingField)
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self, from: data(row))
            )
        }
        var missingFinalizedPair = appliedRow()
        missingFinalizedPair["proposal_hash"] = NSNull()
        missingFinalizedPair["descriptor_hash"] = NSNull()
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self,
                                     from: data(missingFinalizedPair))
        )
        var missingAuthenticatedView = appliedRow()
        missingAuthenticatedView.removeValue(forKey: "proposal_view")
        XCTAssertNil(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self,
                                     from: data(missingAuthenticatedView))
                .autonomousLaneExecutions[0].proposalView
        )
        var nullReservationView = reservationRow()
        nullReservationView["proposal_view"] = NSNull()
        XCTAssertNil(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self,
                                     from: data(nullReservationView))
                .autonomousLaneExecutions[0].proposalView
        )
        var reservationWithView = reservationRow()
        reservationWithView["proposal_view"] = 0
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self,
                                     from: data(reservationWithView))
        )

        for field in [
            "executable_payload_hash", "source_bundle_hash", "merge_entry_hash",
        ] {
            var row = reservationRow()
            row[field] = nativeAmxTestHash(0xA4)
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self, from: data(row))
            )
        }
        var reservationWithFinalizedIdentity = reservationRow()
        reservationWithFinalizedIdentity["proposal_hash"] = nativeAmxTestHash(0xA5)
        reservationWithFinalizedIdentity["descriptor_hash"] = nativeAmxTestHash(0xA6)
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self,
                                     from: data(reservationWithFinalizedIdentity))
        )
        var oldReason = reservationRow()
        oldReason["stuck_reason"] = "awaiting_payload_availability"
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self, from: data(oldReason))
        )
        var wrongCount = reservationRow()
        wrongCount["reservation_count"] = 1
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self, from: data(wrongCount))
        )

        let first = appliedRow()
        var sameProvisionalIdentity = appliedRow()
        sameProvisionalIdentity["proposal_hash"] = nativeAmxTestHash(0xA7)
        sameProvisionalIdentity["descriptor_hash"] = nativeAmxTestHash(0xA8)
        let duplicateIdentity = try mutatedNativeAmxDiagnosticsPayload { root in
            root["autonomous_lane_executions"] = [first, sameProvisionalIdentity]
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self,
                                     from: duplicateIdentity)
        )

        var descendingFirst = appliedRow()
        descendingFirst["proposal_identity_hash"] = nativeAmxTestHash(0x90)
        var descendingSecond = appliedRow()
        descendingSecond["proposal_identity_hash"] = nativeAmxTestHash(0x80)
        let orderingDrift = try mutatedNativeAmxDiagnosticsPayload { root in
            root["autonomous_lane_executions"] = [descendingFirst, descendingSecond]
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self,
                                     from: orderingDrift)
        )
    }

    func testSumeragiDiagnosticsRejectsUnknownAutonomousLaneExecutionField() throws {
        let row: [String: Any] = [
            "lane_id": 9, "dataspace_id": 13,
            "lane_incarnation": nativeAmxTestHash(0xB8),
            "lane_block_height": 8, "lane_block_view": 1,
            "proposal_height": 10,
            "reservation_owner_hash": nativeAmxTestHash(0x5D),
            "proposal_identity_hash": nativeAmxTestHash(0x5E),
            "reservation_group_hash": nativeAmxTestHash(0x5F),
            "reservation_count": 2, "transaction_count": 2,
            "highest_durable_stage": "reservations_durable",
            "stuck_reason": "awaiting_executable_payload",
            "unexpected_field": true,
        ]
        let data = try mutatedNativeAmxDiagnosticsPayload { root in
            root["autonomous_lane_executions"] = [row]
        }

        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self, from: data)
        ) { error in
            XCTAssertTrue(String(describing: error).contains("unexpected_field"))
        }
    }

    func testSumeragiDiagnosticsRejectsMalformedNativeAmxApplicationRows() throws {
        let duplicateRoute = try mutatedNativeAmxDiagnosticsPayload { root in
            let applications =
                root["native_amx_participant_applications"] as! [[String: Any]]
            root["native_amx_participant_applications"] = applications + applications
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: duplicateRoute
            )
        )

        let missingApplicationHash = try mutatedNativeAmxDiagnosticsPayload { root in
            var applications =
                root["native_amx_participant_applications"] as! [[String: Any]]
            applications[0].removeValue(forKey: "application_block_hash")
            root["native_amx_participant_applications"] = applications
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: missingApplicationHash
            )
        )

        let sourceOverflow = try mutatedNativeAmxDiagnosticsPayload { root in
            var applications =
                root["native_amx_participant_applications"] as! [[String: Any]]
            applications[0]["source_count"] = 4_097
            root["native_amx_participant_applications"] = applications
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: sourceOverflow
            )
        )

        let stateGeometryError =
            "Native AMX participant diagnostics state and application identity are inconsistent"
        for state in ["certified_pending_carrier", "conflict"] {
            let unexpectedApplicationIdentity = try mutatedNativeAmxDiagnosticsPayload { root in
                var applications =
                    root["native_amx_participant_applications"] as! [[String: Any]]
                applications[0]["state"] = state
                root["native_amx_participant_applications"] = applications
            }
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiSumeragiDiagnosticsSnapshot.self,
                    from: unexpectedApplicationIdentity
                )
            ) { error in
                XCTAssertTrue(String(describing: error).contains(stateGeometryError))
            }
        }
        for state in ["committed_evidence_pending", "durably_applied"] {
            let missingApplicationIdentity = try mutatedNativeAmxDiagnosticsPayload { root in
                var applications =
                    root["native_amx_participant_applications"] as! [[String: Any]]
                applications[0]["state"] = state
                applications[0].removeValue(forKey: "application_block_height")
                applications[0].removeValue(forKey: "application_block_hash")
                root["native_amx_participant_applications"] = applications
            }
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiSumeragiDiagnosticsSnapshot.self,
                    from: missingApplicationIdentity
                )
            ) { error in
                XCTAssertTrue(String(describing: error).contains(stateGeometryError))
            }
        }
    }

    func testGetSumeragiDiagnosticsParsesTypedLaneEvidenceAsync() async throws {
        let payload = try nativeAmxDiagnosticsPayload()
        var servedPayload = payload
        var servedHeaders = ["Content-Type": "application/json"]
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/sumeragi/diagnostics")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            self.assertOperatorAuthentication(request)
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: servedHeaders
            )!
            return (response, servedPayload)
        }

        let snapshot = try await makeClient().getSumeragiDiagnostics()
        XCTAssertEqual(snapshot.txQueueDepth, 2)
        XCTAssertEqual(snapshot.txQueueCapacity, 64)
        XCTAssertEqual(snapshot.laneSettlementCommitments.count, 1)
        XCTAssertEqual(snapshot.laneRelayEnvelopes.count, 1)

        let invalidResponses: [(Data, [String: String], String)] = [
            (
                duplicateSumeragiRootField(#"{"tx_queue_depth":2,"#, in: payload),
                ["Content-Type": "application/json"], "duplicate object keys"
            ),
            (Data([0xff]), ["Content-Type": "application/json"], "UTF-8 JSON"),
            (
                Data("{}".utf8),
                ["Content-Type": "application/json", "Content-Length": "16777217"],
                "16777216-byte limit"
            ),
        ]
        for (body, headers, errorFragment) in invalidResponses {
            servedPayload = body
            servedHeaders = headers
            do {
                _ = try await makeClient().getSumeragiDiagnostics()
                XCTFail("invalid diagnostics response must fail closed")
            } catch {
                XCTAssertTrue(String(describing: error).contains(errorFragment))
            }
        }
    }

    func testGetSumeragiDiagnosticsCompletionParsesTypedLaneEvidence() throws {
        let payload = try nativeAmxDiagnosticsPayload()
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/sumeragi/diagnostics")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, payload)
        }

        let completed = expectation(description: "diagnostics completion")
        let task = makeClient().getSumeragiDiagnostics { result in
            switch result {
            case .success(let snapshot):
                XCTAssertEqual(snapshot.laneSettlementCommitments.count, 1)
            case .failure(let error):
                XCTFail("unexpected diagnostics failure: \(error)")
            }
            completed.fulfill()
        }
        wait(for: [completed], timeout: 2)
        _ = task
    }

    func testSumeragiDiagnosticsRejectsWrongNativeAmxPhaseAndDuplicateLegs() throws {
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: nativeAmxDiagnosticsPayload(preparePhase: "commit")
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: nativeAmxDiagnosticsPayload(duplicateLeg: true)
            )
        )
    }

    func testSumeragiDiagnosticsRejectsNativeAmxSignatureAndPopLengthDrift() throws {
        for length in [95, 97] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiSumeragiDiagnosticsSnapshot.self,
                    from: nativeAmxDiagnosticsPayload(
                        signature: [UInt8](repeating: 0x9A, count: length)
                    )
                )
            )

            let malformedPop = try mutatedNativeAmxDiagnosticsPayload { root in
                mutateFirstNativeAmxLeg(in: &root) { leg in
                    var qc = leg["prepare_qc"] as! [String: Any]
                    qc["validator_set_pops"] = Array(
                        repeating: [UInt8](repeating: 0x5A, count: length),
                        count: 4
                    )
                    leg["prepare_qc"] = qc
                }
            }
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiSumeragiDiagnosticsSnapshot.self,
                    from: malformedPop
                )
            )
        }

        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: nativeAmxDiagnosticsPayload(
                    signature: [UInt8](repeating: 0, count: 96)
                )
            )
        )
        let zeroPop = try mutatedNativeAmxDiagnosticsPayload { root in
            mutateFirstNativeAmxLeg(in: &root) { leg in
                var qc = leg["prepare_qc"] as! [String: Any]
                qc["validator_set_pops"] = Array(
                    repeating: [UInt8](repeating: 0, count: 96),
                    count: 4
                )
                leg["prepare_qc"] = qc
            }
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self, from: zeroPop)
        )
    }

    func testSumeragiDiagnosticsRejectsLowercaseSourceAndLegacyReceipt() throws {
        let lowercaseSource = try mutatedNativeAmxDiagnosticsPayload { root in
            var commitments = root["lane_settlement_commitments"] as! [[String: Any]]
            var receipts = commitments[0]["native_amx_receipts"] as! [[String: Any]]
            receipts[0]["source_id"] = String(repeating: "ab", count: 32)
            commitments[0]["native_amx_receipts"] = receipts
            root["lane_settlement_commitments"] = commitments
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: lowercaseSource
            )
        )

        let legacyReceipt = try mutatedNativeAmxDiagnosticsPayload { root in
            var commitments = root["lane_settlement_commitments"] as! [[String: Any]]
            var receipts = commitments[0]["native_amx_receipts"] as! [[String: Any]]
            receipts[0]["version"] = 1
            commitments[0]["native_amx_receipts"] = receipts
            root["lane_settlement_commitments"] = commitments
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: legacyReceipt
            )
        )
    }

    func testSumeragiDiagnosticsRejectsNoncanonicalNexusFeeQuantities() throws {
        let overflowing = String(repeating: "9", count: 155)
        let invalidAmounts: [Any] = [
            1, "+1", "01", "1.0", "1.2300", " 1", "1 ", "-1", overflowing,
        ]
        for invalid in invalidAmounts {
            let payload = try mutatedNativeAmxDiagnosticsPayload { root in
                var commitments = root["lane_settlement_commitments"] as! [[String: Any]]
                var receipts = commitments[0]["nexus_fee_receipts"] as! [[String: Any]]
                receipts[0]["fee_amount"] = invalid
                commitments[0]["nexus_fee_receipts"] = receipts
                root["lane_settlement_commitments"] = commitments
            }
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self, from: payload),
                "noncanonical fee_amount \(invalid) must be rejected"
            )
        }

        let schedulePayload = try mutatedNativeAmxDiagnosticsPayload { root in
            var commitments = root["lane_settlement_commitments"] as! [[String: Any]]
            var receipts = commitments[0]["nexus_fee_receipts"] as! [[String: Any]]
            var schedule = receipts[0]["schedule"] as! [String: Any]
            schedule["base_fee"] = "2.0"
            receipts[0]["schedule"] = schedule
            commitments[0]["nexus_fee_receipts"] = receipts
            root["lane_settlement_commitments"] = commitments
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self, from: schedulePayload)
        )
    }

    func testSumeragiDiagnosticsRejectsNativeAmxContextAndEpochDrift() throws {
        let contextMismatch = try mutatedNativeAmxDiagnosticsPayload { root in
            mutateFirstNativeAmxQcBody(in: &root, qcKey: "commit_qc") { body in
                var round = body["round"] as! [String: Any]
                round["context_id"] = [nativeAmxTestHash(0xD1)]
                body["round"] = round
            }
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: contextMismatch
            )
        )

        let epochMismatch = try mutatedNativeAmxDiagnosticsPayload { root in
            mutateFirstNativeAmxQcBody(in: &root, qcKey: "commit_qc") { body in
                body["epoch"] = 3
            }
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: epochMismatch
            )
        )
    }

    func testSumeragiDiagnosticsRejectsFlattenedNativeAmxPhaseAndSessionTampering() throws {
        let flattenedPhase = try mutatedNativeAmxDiagnosticsPayload { root in
            mutateFirstNativeAmxQcBody(in: &root, qcKey: "prepare_qc") { body in
                body["phase"] = "prepare"
            }
        }
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: flattenedPhase
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: nativeAmxDiagnosticsPayload(
                    receiptLaneIncarnation: nativeAmxTestHash(0x99)
                )
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: nativeAmxDiagnosticsPayload(authorityContextHeight: 0)
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: nativeAmxDiagnosticsPayload(
                    receiptProposalHash: nativeAmxTestHash(0x57)
                )
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: nativeAmxDiagnosticsPayload(
                    secondEntrypointHash: nativeAmxTestHash(0x33)
                )
            )
        )
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

    func testSumeragiV2StatusRejectsLegacyMissingAndMalformedShapes() throws {
        func payload(_ mutate: (inout [String: Any]) -> Void) throws -> Data {
            var value: [String: Any] = [
                "protocol_version": 4,
                "node_fingerprint": nativeAmxTestHash(0xA1),
                "build_fingerprint": nativeAmxTestHash(0xA3),
                "config_fingerprint": nativeAmxTestHash(0xA5),
                "restart_required": false,
                "height_context_id": [nativeAmxTestHash(0xA7)],
                "height": 1,
                "view": 0,
                "phase": ["phase": "awaiting_proposal", "details": NSNull()],
                "leader": 0,
                "body_state": ["state": "missing", "details": NSNull()],
                "last_committed_height": 0,
                "height_context": sumeragiV2TestHeightContext(),
                "liveness": sumeragiV2TestLiveness(),
            ]
            mutate(&value)
            return try JSONSerialization.data(withJSONObject: value)
        }
        func assertRejected(_ data: Data) {
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiSumeragiStatusSnapshot.self, from: data)
            )
        }

        assertRejected(try payload { $0["protocol_version"] = 3 })
        assertRejected(try payload { $0.removeValue(forKey: "restart_required") })
        assertRejected(try payload { $0.removeValue(forKey: "height_context") })
        assertRejected(try payload { $0.removeValue(forKey: "liveness") })
        assertRejected(
            try payload {
                var context = $0["height_context"] as! [String: Any]
                context["legacy_epoch_start"] = 0
                $0["height_context"] = context
            }
        )
        assertRejected(
            try payload {
                var liveness = $0["liveness"] as! [String: Any]
                liveness.removeValue(forKey: "ignore_counts")
                $0["liveness"] = liveness
            }
        )
        assertRejected(try payload { $0["phase"] = "awaiting_proposal" })
        assertRejected(
            try payload {
                $0["phase"] = ["phase": "AwaitingProposal", "details": NSNull()]
            }
        )
        assertRejected(
            try payload {
                $0["body_state"] = ["state": "Missing", "details": NSNull()]
            }
        )
        assertRejected(
            try payload {
                $0["phase"] = ["phase": "prepare", "details": NSNull()]
            }
        )
        assertRejected(try payload { $0["rbc_status"] = "delivered" })
        assertRejected(try payload { $0["lane_settlement_commitments"] = [] })
        assertRejected(
            try payload {
                $0["phase"] = ["phase": "awaiting_proposal", "details": "legacy"]
            }
        )
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

        makeClient().uploadAttachment(data: Data("test".utf8), contentType: "application/json", canonicalAuth: canonicalReadAuth) { result in
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

        makeClient().listAttachments(canonicalAuth: canonicalReadAuth) { result in
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

        makeClient().getAttachment(id: attachmentId, canonicalAuth: canonicalReadAuth) { result in
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

        makeClient().deleteAttachment(id: attachmentId, canonicalAuth: canonicalReadAuth) { result in
            if case let .failure(error) = result {
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testFetchContractManifestParsesResponse() {
        let expectation = expectation(description: "fetch manifest")
        let codeHash = String(repeating: "b", count: 64)
        let abiHash = String(repeating: "d", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/code/\(codeHash)")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"manifest":{"seiyaku_name":null,"code_hash":"hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#ABA2","abi_hash":"hash:DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD#F071","compiler_fingerprint":"rustc","features_bitmap":1,"access_set_hints":{"read_keys":["account:alice#wonderland"],"write_keys":[]},"entrypoints":null,"states":null,"error_codes":null},"code_hash":"\(codeHash)","abi_hash":"\(abiHash)"}
            """.data(using: .utf8)!
            return (response, body)
        }

        makeClient().fetchContractManifest(codeHashHex: codeHash) { result in
            switch result {
            case .success(let record):
                XCTAssertEqual(record.manifest.codeHash, codeHash)
                XCTAssertEqual(record.manifest.abiHash, abiHash)
                XCTAssertEqual(record.codeHash, codeHash)
                XCTAssertEqual(record.abiHash, abiHash)
                XCTAssertEqual(record.manifest.accessSetHints?.readKeys, ["account:alice#wonderland"])
                XCTAssertEqual(record.manifest.accessSetHints?.writeKeys ?? [], [])
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testContractManifestRecordRejectsMismatchedHashConveniences() throws {
        let manifest = #"{"code_hash":"hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#ABA2","abi_hash":"hash:DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD#F071"}"#
        let valid = "{\"manifest\":\(manifest),\"code_hash\":\"\(String(repeating: "b", count: 64))\",\"abi_hash\":\"\(String(repeating: "d", count: 64))\"}"
        let record = try JSONDecoder().decode(
            ToriiContractManifestRecord.self,
            from: Data(valid.utf8)
        )
        XCTAssertEqual(record.codeHash, record.manifest.codeHash)
        XCTAssertEqual(record.abiHash, record.manifest.abiHash)

        let invalid = [
            "{\"manifest\":\(manifest),\"code_hash\":\"\(String(repeating: "d", count: 64))\",\"abi_hash\":\"\(String(repeating: "d", count: 64))\"}",
            "{\"manifest\":\(manifest),\"code_hash\":\"\(String(repeating: "B", count: 64))\",\"abi_hash\":\"\(String(repeating: "d", count: 64))\"}",
            "{\"manifest\":\(manifest),\"abi_hash\":\"\(String(repeating: "d", count: 64))\"}",
            "{\"manifest\":\(manifest),\"code_hash\":\"\(String(repeating: "b", count: 64))\",\"abi_hash\":\"\(String(repeating: "d", count: 64))\",\"code_bytes\":null}",
        ]
        for payload in invalid {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiContractManifestRecord.self,
                    from: Data(payload.utf8)
                ),
                "accepted inconsistent manifest response: \(payload)"
            )
        }
    }

    func testContractManifestPreservesExactV1InterfaceShape() throws {
        let payload = """
        {
          "seiyaku_name":"Ledger",
          "code_hash":"hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#ABA2",
          "abi_hash":"hash:DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD#F071",
          "compiler_fingerprint":"kotodama_lang",
          "features_bitmap":0,
          "access_set_hints":{
            "read_keys":["state:Balances"],
            "write_keys":["state:Balances"],
            "dynamic_reads":[{
              "base_key":"state:Balances",
              "key_type":"AccountId",
              "bound_kind":"take",
              "max_keys":64
            }],
            "dynamic_writes":[]
          },
          "entrypoints":[{
            "name":"transfer",
            "kind":{"kind":"Kotoage","value":null},
            "params":[
              {"name":"request","type_name":"struct Transfer"},
              {"name":"tags","type_name":"List<Name, 64>"}
            ],
            "argument_schema":{"fields":[
              {"name":"request","ty":{"nodes":[
                {"kind":"Struct","value":{"name":"Transfer","fields":["amount","memo"]}},
                {"kind":"Leaf","value":{"kind":"Quantity","value":null}},
                {"kind":"Option","value":null},
                {"kind":"Leaf","value":{"kind":"String","value":null}}
              ]}},
              {"name":"tags","ty":{"nodes":[
                {"kind":"List","value":{"capacity":64}},
                {"kind":"Leaf","value":{"kind":"Name","value":null}}
              ]}}
            ]},
            "return_type":"Result<(bool, decimal), string>",
            "return_schema":{"nodes":[
              {"kind":"Result","value":null},
              {"kind":"Tuple","value":2},
              {"kind":"Leaf","value":{"kind":"Bool","value":null}},
              {"kind":"Leaf","value":{"kind":"Decimal","value":null}},
              {"kind":"Leaf","value":{"kind":"String","value":null}}
            ]},
            "permission":"TransferAsset",
            "read_keys":["state:Balances"],
            "write_keys":["state:Balances"],
            "access_hints_complete":true,
            "access_hints_skipped":[],
            "triggers":[{
              "id":"settle",
              "repeats":{"Exactly":2},
              "filter":"TlJUMAAAl9+YQQ4oJZjALRf6FAto0QAKAAAAAAAAANzCjydU9+jNAgIAAAAFBAAAAAA=",
              "authority":null,
              "metadata":{"purpose":"daily-settlement","round":7},
              "callback":{"namespace":null,"entrypoint":"transfer"}
            }]
          }],
          "states":[{"name":"Balances","type_name":"StateMap<AccountId, quantity>"}],
          "error_codes":[{
            "namespace":"TransferError",
            "name":"InsufficientFunds",
            "code":1001
          }],
          "kotoba":[{
            "msg_id":"transfer.denied",
            "translations":[
              {"lang":"en","text":"Transfer denied"},
              {"lang":"ja","text":"送金は拒否されました"}
            ]
          }],
          "provenance":{"signer":"ed25519:fixture","signature":"fixture-signature"}
        }
        """.data(using: .utf8)!

        let manifest = try JSONDecoder().decode(ToriiContractManifest.self, from: payload)
        XCTAssertEqual(manifest.seiyakuName, "Ledger")
        XCTAssertEqual(manifest.codeHash, String(repeating: "b", count: 64))
        XCTAssertEqual(manifest.abiHash, String(repeating: "d", count: 64))
        XCTAssertEqual(manifest.accessSetHints?.dynamicReads.first?.maxKeys, 64)
        let entrypoint = try XCTUnwrap(manifest.entrypoints?.first)
        XCTAssertEqual(entrypoint.kind, .kotoage)
        XCTAssertEqual(entrypoint.argumentSchema?.fields.first?.type.wordCount, 2)
        XCTAssertEqual(entrypoint.argumentSchema?.fields.last?.type.wordCount, 1)
        guard case let .list(listNode)? = entrypoint.argumentSchema?.fields.last?.type.nodes.first else {
            return XCTFail("expected bounded list argument schema")
        }
        XCTAssertEqual(listNode.capacity, 64)
        XCTAssertEqual(entrypoint.returnSchema?.wordCount, 1)
        guard case let .leaf(returnLeaf)? = entrypoint.returnSchema?.nodes[2] else {
            return XCTFail("expected bool return leaf")
        }
        XCTAssertEqual(returnLeaf, .bool)
        XCTAssertEqual(entrypoint.triggers.first?.callback.entrypoint, "transfer")
        XCTAssertEqual(entrypoint.triggers.first?.metadata["round"], .number(7))
        XCTAssertEqual(manifest.states?.first?.typeName, "StateMap<AccountId, quantity>")
        XCTAssertEqual(manifest.errorCodes?.first?.code, 1001)
        XCTAssertEqual(manifest.kotoba?.first?.translations.last?.language, "ja")
        XCTAssertEqual(manifest.provenance?.signer, "ed25519:fixture")

        let encoded = try JSONEncoder().encode(manifest)
        let object = try XCTUnwrap(
            try JSONSerialization.jsonObject(with: encoded) as? [String: Any]
        )
        XCTAssertEqual(object["seiyaku_name"] as? String, "Ledger")
        XCTAssertEqual(
            object["code_hash"] as? String,
            "hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#ABA2"
        )
        XCTAssertEqual(
            object["abi_hash"] as? String,
            "hash:DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD#F071"
        )
        let entrypoints = try XCTUnwrap(object["entrypoints"] as? [[String: Any]])
        XCTAssertNotNil(entrypoints[0]["argument_schema"])
        XCTAssertNotNil(entrypoints[0]["return_schema"])
        XCTAssertEqual((entrypoints[0]["triggers"] as? [Any])?.count, 1)
        XCTAssertEqual((object["states"] as? [Any])?.count, 1)
        XCTAssertEqual((object["error_codes"] as? [Any])?.count, 1)
        XCTAssertEqual((object["kotoba"] as? [Any])?.count, 1)
        XCTAssertNotNil(object["provenance"] as? [String: Any])
    }

    func testEntrypointSchemaUsesOneFlatPreorderTapeAndExactReservedNames() throws {
        func leaf(_ kind: String) -> String {
            "{\"kind\":\"Leaf\",\"value\":{\"kind\":\"\(kind)\",\"value\":null}}"
        }
        func structNode(_ name: String, _ fields: [String]) -> String {
            let encodedFields = fields.map { "\"\($0)\"" }.joined(separator: ",")
            return "{\"kind\":\"Struct\",\"value\":{\"name\":\"\(name)\",\"fields\":[\(encodedFields)]}}"
        }
        func decode(_ nodes: [String]) throws -> ToriiEntrypointValueTypeV1 {
            let payload = "{\"nodes\":[\(nodes.joined(separator: ","))]}"
            return try JSONDecoder().decode(
                ToriiEntrypointValueTypeV1.self,
                from: Data(payload.utf8)
            )
        }

        let views: [(String, [String], [String])] = [
            (
                "AccountView",
                ["id", "metadata"],
                [leaf("AccountId"), leaf("Json")]
            ),
            (
                "AssetView",
                ["id", "amount"],
                [leaf("AssetId"), leaf("Quantity")]
            ),
            (
                "AssetDefinitionView",
                ["id", "name", "description", "owned_by", "total_quantity", "metadata"],
                [
                    leaf("AssetDefinitionId"),
                    leaf("String"),
                    #"{"kind":"Option","value":null}"#,
                    leaf("String"),
                    leaf("AccountId"),
                    leaf("Quantity"),
                    leaf("Json"),
                ]
            ),
            (
                "DomainView",
                ["id", "owned_by", "metadata"],
                [leaf("DomainId"), leaf("AccountId"), leaf("Json")]
            ),
            (
                "NftView",
                ["id", "owned_by", "content"],
                [leaf("NftId"), leaf("AccountId"), leaf("Json")]
            ),
        ]

        for (name, fields, children) in views {
            let viewNodes = [structNode(name, fields)] + children
            let view = try decode(viewNodes)
            XCTAssertEqual(view.canonicalTypeName, name)

            let optional = try decode([#"{"kind":"Option","value":null}"#] + viewNodes)
            XCTAssertEqual(optional.canonicalTypeName, "Option<\(name)>")

            let pageNodes = [
                structNode("QueryPage", ["items", "next_offset"]),
                #"{"kind":"List","value":{"capacity":64}}"#,
            ] + viewNodes + [
                #"{"kind":"Option","value":null}"#,
                leaf("Int"),
            ]
            let page = try decode(pageNodes)
            XCTAssertEqual(page.canonicalTypeName, "QueryPage<\(name)>")
            XCTAssertEqual(page.wordCount, 2)

            let encoded = try JSONEncoder().encode(page)
            let object = try XCTUnwrap(
                try JSONSerialization.jsonObject(with: encoded) as? [String: Any]
            )
            let encodedNodes = try XCTUnwrap(object["nodes"] as? [[String: Any]])
            let listPayload = try XCTUnwrap(encodedNodes[1]["value"] as? [String: Any])
            XCTAssertEqual(listPayload["capacity"] as? Int, 64)
            XCTAssertEqual(Set(listPayload.keys), ["capacity"])
            XCTAssertEqual(
                try JSONDecoder().decode(ToriiEntrypointValueTypeV1.self, from: encoded),
                page
            )
        }

        let pair = try decode([
            structNode("Pair", ["left", "right"]),
            leaf("Int"),
            leaf("Bool"),
        ])
        XCTAssertEqual(pair.canonicalTypeName, "struct Pair")
    }

    func testEntrypointSchemaRejectsLegacyTruncatedDeepAndForgedTapes() throws {
        func leaf(_ kind: String) -> String {
            "{\"kind\":\"Leaf\",\"value\":{\"kind\":\"\(kind)\",\"value\":null}}"
        }
        func structNode(_ name: String, _ fields: [String]) -> String {
            let encodedFields = fields.map { "\"\($0)\"" }.joined(separator: ",")
            return "{\"kind\":\"Struct\",\"value\":{\"name\":\"\(name)\",\"fields\":[\(encodedFields)]}}"
        }
        func decode(_ nodes: [String]) throws -> ToriiEntrypointValueTypeV1 {
            let payload = "{\"nodes\":[\(nodes.joined(separator: ","))]}"
            return try JSONDecoder().decode(
                ToriiEntrypointValueTypeV1.self,
                from: Data(payload.utf8)
            )
        }
        func reject(_ nodes: [String], _ label: String) {
            XCTAssertThrowsError(try decode(nodes), "accepted \(label)")
        }

        let views: [(String, [String], [String])] = [
            ("AccountView", ["id", "metadata"], [leaf("AccountId"), leaf("Json")]),
            ("AssetView", ["id", "amount"], [leaf("AssetId"), leaf("Quantity")]),
            (
                "AssetDefinitionView",
                ["id", "name", "description", "owned_by", "total_quantity", "metadata"],
                [
                    leaf("AssetDefinitionId"),
                    leaf("String"),
                    #"{"kind":"Option","value":null}"#,
                    leaf("String"),
                    leaf("AccountId"),
                    leaf("Quantity"),
                    leaf("Json"),
                ]
            ),
            (
                "DomainView",
                ["id", "owned_by", "metadata"],
                [leaf("DomainId"), leaf("AccountId"), leaf("Json")]
            ),
            (
                "NftView",
                ["id", "owned_by", "content"],
                [leaf("NftId"), leaf("AccountId"), leaf("Json")]
            ),
        ]

        for (name, fields, children) in views {
            let validView = [structNode(name, fields)] + children
            var wrongFields = fields
            wrongFields[wrongFields.count - 1] = "forged"
            reject(
                [structNode(name, wrongFields)] + children,
                "\(name) with forged fields"
            )

            var wrongLeaf = children
            wrongLeaf[wrongLeaf.count - 1] = leaf("Blob")
            reject(
                [structNode(name, fields)] + wrongLeaf,
                "\(name) with forged leaf kind"
            )

            let pagePrefix = [
                structNode("QueryPage", ["items", "next_offset"]),
                #"{"kind":"List","value":{"capacity":64}}"#,
            ]
            reject(
                pagePrefix + validView + [
                    #"{"kind":"Option","value":null}"#,
                    leaf("String"),
                ],
                "QueryPage<\(name)> with non-int next_offset"
            )
            reject(
                [
                    structNode("QueryPage", ["items", "next_offset"]),
                    #"{"kind":"List","value":{"capacity":32}}"#,
                ] + validView + [
                    #"{"kind":"Option","value":null}"#,
                    leaf("Int"),
                ],
                "QueryPage<\(name)> with capacity below 64"
            )
        }

        reject(
            [
                #"{"kind":"List","value":{"capacity":64,"element":{"nodes":[{"kind":"Leaf","value":{"kind":"Name","value":null}}]}}}"#,
            ],
            "retired nested list element"
        )
        reject([#"{"kind":"List","value":{"capacity":64}}"#], "truncated list tape")
        reject([leaf("Bool"), leaf("Bool")], "extra root node")
        reject(
            [#"{"kind":"List","value":{"capacity":0}}"#, leaf("Int")],
            "zero list capacity"
        )
        reject(
            [#"{"kind":"List","value":{"capacity":65}}"#, leaf("Int")],
            "list capacity above 64"
        )

        let forgedForEncoding = ToriiEntrypointValueTypeV1(nodes: [
            .structType(
                ToriiEntrypointStructTypeNodeV1(
                    name: "AccountView",
                    fields: ["id", "metadata"]
                )
            ),
            .leaf(.accountId),
            .leaf(.blob),
        ])
        XCTAssertThrowsError(try JSONEncoder().encode(forgedForEncoding))
        XCTAssertThrowsError(
            try JSONEncoder().encode(
                ToriiEntrypointValueTypeV1(
                    nodes: [.list(ToriiEntrypointListTypeNodeV1(capacity: 64))]
                )
            )
        )

        let atLimit = Array(
            repeating: #"{"kind":"List","value":{"capacity":1}}"#,
            count: 255
        ) + [leaf("Int")]
        XCTAssertEqual(try decode(atLimit).wordCount, 1)
        reject(
            Array(
                repeating: #"{"kind":"List","value":{"capacity":1}}"#,
                count: 256
            ) + [leaf("Int")],
            "schema depth and node budget overflow"
        )
    }

    func testContractManifestRejectsNoncanonicalV1InterfaceShapes() throws {
        let validLeaf = #"{"kind":"Leaf","value":{"kind":"Bool","value":null}}"#
        var cases = [
            #"{"seiyaku_name":" Ledger "}"#,
            #"{"seiyaku_name":"Amount"}"#,
            #"{"seiyaku_name":"amount"}"#,
            #"{"seiyaku_name":"seiyaku"}"#,
            #"{"seiyaku_name":"match"}"#,
            #"{"seiyaku_name":"__kotodama_quantity_ratio_round"}"#,
            #"{"seiyaku_name":"__kotodama_decimal_to_int_trunc"}"#,
            #"{"seiyaku_name":"__kotodama_decimal_to_int_round"}"#,
            #"{"states":[{"name":"Amount","type_name":"quantity"}]}"#,
            #"{"states":[{"name":"Balances","type_name":"Transfer{Amount: quantity}"}]}"#,
            #"{"states":[{"name":"Balances","type_name":"StateMap<AccountId, Amount>"}]}"#,
            #"{"states":[{"name":"Balances","type_name":"StateMap<AccountId, amount>"}]}"#,
            #"{"states":[{"name":"Balances","type_name":"Amount: quantity"}]}"#,
            #"{"states":[{"name":"Balances","type_name":"StateMap<AccountId, Amount: quantity>"}]}"#,
            #"{"states":[{"name":"Balances","type_name":"Transfer{value: Result<int, Amount: quantity>}"}]}"#,
            #"{"states":[{"name":"Balances","type_name":"Transfer{amount: Amount}"}]}"#,
            #"{"states":[{"name":"Balances","type_name":"Amount{amount: quantity}"}]}"#,
            #"{"code_hash":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}"#,
            #"{"code_hash":"hash:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb#ABA2"}"#,
            #"{"code_hash":"hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#0000"}"#,
            #"{"code_hash":"hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#aba2"}"#,
            #"{"code_hash":"hash:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA#0E5B"}"#,
            #"{"provenance":"not-an-object"}"#,
            #"{"entrypoints":[{"name":"run","kind":{"kind":"Public","value":null},"params":[],"argument_schema":null,"return_type":null,"return_schema":null,"permission":null,"read_keys":[],"write_keys":[],"access_hints_complete":true,"access_hints_skipped":[],"triggers":[]}]}"#,
            #"{"entrypoints":[{"name":"Amount","kind":{"kind":"View","value":null},"params":[],"argument_schema":null,"return_type":null,"return_schema":null,"permission":null,"read_keys":[],"write_keys":[],"access_hints_complete":true,"access_hints_skipped":[],"triggers":[]}]}"#,
            #"{"entrypoints":[{"name":"run","kind":{"kind":"View","value":null},"params":[{"name":"Amount","type_name":"bool"}],"argument_schema":{"fields":[{"name":"Amount","ty":{"nodes":[{"kind":"Leaf","value":{"kind":"Bool","value":null}}]}}]},"return_type":null,"return_schema":null,"permission":null,"read_keys":[],"write_keys":[],"access_hints_complete":true,"access_hints_skipped":[],"triggers":[]}]}"#,
            #"{"entrypoints":[{"name":"run","kind":{"kind":"Kotoage","value":"Kotoage"},"params":[],"argument_schema":null,"return_type":null,"return_schema":null,"permission":null,"read_keys":[],"write_keys":[],"access_hints_complete":true,"access_hints_skipped":[],"triggers":[]}]}"#,
            "{\"entrypoints\":[{\"name\":\"run\",\"kind\":{\"kind\":\"Kotoage\",\"value\":null},\"params\":[{\"name\":\"flag\",\"type_name\":\"bool\"}],\"argument_schema\":{\"fields\":[{\"name\":\"flag\",\"ty\":{\"nodes\":[{\"kind\":\"Tuple\",\"value\":1},\(validLeaf)]}}]},\"return_type\":null,\"return_schema\":null,\"permission\":null,\"read_keys\":[],\"write_keys\":[],\"access_hints_complete\":true,\"access_hints_skipped\":[],\"triggers\":[]}]}",
            #"{"entrypoints":[{"name":"run","kind":{"kind":"Kotoage","value":null},"params":[],"argument_schema":null,"return_type":"bool","return_schema":{"nodes":[{"kind":"Leaf","value":{"kind":"Bool","value":null}},{"kind":"Leaf","value":{"kind":"Bool","value":null}}]},"permission":null,"read_keys":[],"write_keys":[],"access_hints_complete":true,"access_hints_skipped":[],"triggers":[]}]}"#,
            #"{"entrypoints":[{"name":"run","kind":{"kind":"Kotoage","value":null},"params":[],"argument_schema":null,"return_type":null,"return_schema":null,"permission":null,"read_keys":[],"write_keys":[],"access_hints_complete":true,"access_hints_skipped":[],"triggers":["not-an-object"]}]}"#,
            #"{"error_codes":[{"namespace":"Failure","name":"Denied","code":0}]}"#,
            #"{"error_codes":[{"namespace":"Failure","name":"Amount","code":1}]}"#,
            #"{"entrypoints":[{"name":"run","kind":{"kind":"Kotoage","value":null},"params":[],"argument_schema":null,"return_type":null,"return_schema":{"nodes":[{"kind":"Leaf","value":{"kind":"Bool","value":null}}]},"permission":null,"read_keys":[],"write_keys":[],"access_hints_complete":true,"access_hints_skipped":[],"triggers":[]}]}"#,
            #"{"entrypoints":[{"name":"run","kind":{"kind":"Kotoage","value":null},"params":[{"name":"flag","type_name":"bool"}],"argument_schema":{"fields":[{"name":"different","ty":{"nodes":[{"kind":"Leaf","value":{"kind":"Bool","value":null}}]}}]},"return_type":null,"return_schema":null,"permission":null,"read_keys":[],"write_keys":[],"access_hints_complete":true,"access_hints_skipped":[],"triggers":[]}]}"#,
            #"{"entrypoints":[{"name":"run","kind":{"kind":"Kotoage","value":null},"params":[{"name":"tags","type_name":"List<Name, 64>"}],"argument_schema":{"fields":[{"name":"tags","ty":{"nodes":[{"kind":"List","value":{"capacity":65}},{"kind":"Leaf","value":{"kind":"Name","value":null}}]}}]},"return_type":null,"return_schema":null,"permission":null,"read_keys":[],"write_keys":[],"access_hints_complete":true,"access_hints_skipped":[],"triggers":[]}]}"#,
        ]
        let wideLeaves = Array(repeating: validLeaf, count: 14).joined(separator: ",")
        cases.append(
            "{\"entrypoints\":[{\"name\":\"run\",\"kind\":{\"kind\":\"Kotoage\",\"value\":null},\"params\":[],\"argument_schema\":null,\"return_type\":\"wide tuple\",\"return_schema\":{\"nodes\":[{\"kind\":\"Tuple\",\"value\":14},\(wideLeaves)]},\"permission\":null,\"read_keys\":[],\"write_keys\":[],\"access_hints_complete\":true,\"access_hints_skipped\":[],\"triggers\":[]}]}")

        for payload in cases {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiContractManifest.self,
                    from: Data(payload.utf8)
                ),
                "accepted noncanonical manifest payload: \(payload)"
            )
        }
    }

    func testContractManifestAllowsAmountAsStructFieldIdentifier() throws {
        let payload =
            #"{"states":[{"name":"Balances","type_name":"Transfer{amount: quantity}"}]}"#
        let manifest = try JSONDecoder().decode(
            ToriiContractManifest.self,
            from: Data(payload.utf8)
        )

        XCTAssertEqual(manifest.states?.first?.typeName, "Transfer{amount: quantity}")
        let encoded = try JSONEncoder().encode(manifest)
        let decoded = try JSONDecoder().decode(ToriiContractManifest.self, from: encoded)
        XCTAssertEqual(decoded.states?.first?.typeName, "Transfer{amount: quantity}")
    }

    func testContractManifestStateTypesUseExactCanonicalV1Grammar() throws {
        func decode(_ typeName: String) throws -> ToriiContractManifest {
            let data = try JSONSerialization.data(
                withJSONObject: [
                    "states": [
                        ["name": "Balances", "type_name": typeName],
                    ],
                ]
            )
            return try JSONDecoder().decode(ToriiContractManifest.self, from: data)
        }

        let scalarTypes = [
            "int", "decimal", "quantity", "bool", "string", "bytes", "DataSpaceId",
            "AccountId", "AssetDefinitionId", "AssetId", "NftId", "DomainId", "Name", "Json",
        ]
        let stateMapKeyTypes = scalarTypes.filter { $0 != "Json" }
        let canonical = scalarTypes + stateMapKeyTypes.map {
            "StateMap<\($0), quantity>"
        } + [
            "(int, decimal)",
            "Option<Result<quantity, string>>",
            "List<Transfer{amount: quantity}, 64>",
            "StateMap<AccountId, Transfer{amount: quantity}>",
            "StateMap<Name, Transfer{amount: quantity, memo: Option<string>}>",
        ]
        for typeName in canonical {
            let manifest = try decode(typeName)
            XCTAssertEqual(manifest.states?.first?.typeName, typeName)
            XCTAssertNoThrow(try JSONEncoder().encode(manifest))
        }

        let noncanonical = [
            "Amount",
            "amount",
            "Transfer{amount: amount}",
            "Transfer{amount:: quantity}",
            "Transfer{Amount: quantity}",
            "Transfer{amount:quantity}",
            "Transfer{amount:  quantity}",
            "Transfer {amount: quantity}",
            "Transfer{}",
            "Transfer{amount: quantity, amount: int}",
            "Transfer{__kotodama_link_amount: quantity}",
            "(int)",
            "(int,decimal)",
            "Option<quantity",
            "Transfer{amount: Option<quantity>}}",
            "Option<StateMap<AccountId, quantity>>",
            "StateMap<AccountId, StateMap<Name, quantity>>",
            "StateMap<Json, quantity>",
            "StateMap<AccountId , quantity>",
            "StateMap<AccountId,quantity>",
            "List<quantity, 0>",
            "List<quantity, 01>",
            "List<quantity, 65>",
            "Trаnsfer{amount: quantity}",
            "Transfer{amount: quаntity}",
        ]
        for typeName in noncanonical {
            XCTAssertThrowsError(try decode(typeName), "accepted state type \(typeName)")
            XCTAssertThrowsError(
                try JSONEncoder().encode(
                    ToriiContractStateDescriptor(name: "Balances", typeName: typeName)
                ),
                "encoded state type \(typeName)"
            )
        }
    }

    func testContractManifestStateTypeParserEnforcesDepthAndNodeBudgets() throws {
        func decode(_ typeName: String) throws -> ToriiContractManifest {
            let data = try JSONSerialization.data(
                withJSONObject: [
                    "states": [
                        ["name": "Balances", "type_name": typeName],
                    ],
                ]
            )
            return try JSONDecoder().decode(ToriiContractManifest.self, from: data)
        }

        let maximumDepth = String(repeating: "Option<", count: 255)
            + "int"
            + String(repeating: ">", count: 255)
        XCTAssertNoThrow(try decode(maximumDepth))
        let excessiveDepth = "Option<" + maximumDepth + ">"
        XCTAssertThrowsError(try decode(excessiveDepth))
        let maximumMapValueDepth = String(repeating: "Option<", count: 254)
            + "int"
            + String(repeating: ">", count: 254)
        XCTAssertNoThrow(try decode("StateMap<AccountId, \(maximumMapValueDepth)>"))
        XCTAssertThrowsError(try decode("StateMap<AccountId, \(maximumDepth)>"))

        let maximumNodes = "("
            + Array(repeating: "int", count: 255).joined(separator: ", ")
            + ")"
        XCTAssertNoThrow(try decode(maximumNodes))
        XCTAssertNoThrow(try decode("StateMap<AccountId, \(maximumNodes)>"))
        let excessiveNodes = String(maximumNodes.dropLast()) + ", int)"
        XCTAssertThrowsError(try decode(excessiveNodes))
        XCTAssertThrowsError(try decode("StateMap<AccountId, \(excessiveNodes)>"))
    }

    func testContractManifestDynamicAccessHintsUseExactV1Policy() throws {
        func payload(
            baseKey: String = "state:Balances",
            keyType: String = "AccountId",
            boundKind: String = "take",
            maxKeys: Int = 1,
            includeUnknown: Bool = false,
            stateName: String = "Balances",
            stateKeyType: String? = nil
        ) throws -> Data {
            var dynamicHint: [String: Any] = [
                "base_key": baseKey,
                "key_type": keyType,
                "bound_kind": boundKind,
                "max_keys": maxKeys,
            ]
            if includeUnknown {
                dynamicHint["unknown"] = true
            }
            return try JSONSerialization.data(withJSONObject: [
                "access_set_hints": [
                    "read_keys": [],
                    "write_keys": [],
                    "dynamic_reads": [dynamicHint],
                    "dynamic_writes": [],
                ],
                "states": [
                    [
                        "name": stateName,
                        "type_name": "StateMap<\(stateKeyType ?? keyType), quantity>",
                    ],
                ],
            ])
        }

        func decode(
            baseKey: String = "state:Balances",
            keyType: String = "AccountId",
            boundKind: String = "take",
            maxKeys: Int = 1,
            includeUnknown: Bool = false,
            stateName: String = "Balances",
            stateKeyType: String? = nil
        ) throws -> ToriiContractDynamicAccessHint {
            let manifest = try JSONDecoder().decode(
                ToriiContractManifest.self,
                from: payload(
                    baseKey: baseKey,
                    keyType: keyType,
                    boundKind: boundKind,
                    maxKeys: maxKeys,
                    includeUnknown: includeUnknown,
                    stateName: stateName,
                    stateKeyType: stateKeyType
                )
            )
            return try XCTUnwrap(manifest.accessSetHints?.dynamicReads.first)
        }

        let keyTypes = [
            "int",
            "decimal",
            "quantity",
            "bool",
            "string",
            "bytes",
            "DataSpaceId",
            "AccountId",
            "AssetDefinitionId",
            "AssetId",
            "NftId",
            "DomainId",
            "Name",
        ]
        for keyType in keyTypes {
            XCTAssertEqual(try decode(keyType: keyType).keyType, keyType)
            XCTAssertNoThrow(
                try JSONEncoder().encode(
                    ToriiContractDynamicAccessHint(
                        baseKey: "state:Balances",
                        keyType: keyType,
                        boundKind: "range",
                        maxKeys: 64
                    )
                )
            )
        }
        for boundKind in ["range", "take"] {
            XCTAssertEqual(try decode(boundKind: boundKind).boundKind, boundKind)
        }
        for baseKey in ["state:Balances", "state:amount"] {
            let stateName = baseKey == "state:amount" ? "amount" : "Balances"
            XCTAssertEqual(
                try decode(baseKey: baseKey, stateName: stateName).baseKey,
                baseKey
            )
            XCTAssertNoThrow(
                try JSONEncoder().encode(
                    ToriiContractDynamicAccessHint(
                        baseKey: baseKey,
                        keyType: "AccountId",
                        boundKind: "take",
                        maxKeys: 1
                    )
                )
            )
        }
        for maxKeys in [1, 64] {
            XCTAssertEqual(try decode(maxKeys: maxKeys).maxKeys, UInt32(maxKeys))
        }

        let invalidBaseKeys = [
            "",
            "state:",
            "state:*",
            "state:Balances.more",
            "state:Balances:Other",
            "state:Amount",
            "state:state:Balances",
            "state:match",
            "state:StateMap",
            "state:__kotodama_link_Balances",
            "state: Balances",
            "state:Balances ",
            "state:Бalances",
            "states:Balances",
            "Balances",
            "state:amount.more",
        ]
        for baseKey in invalidBaseKeys {
            XCTAssertThrowsError(try decode(baseKey: baseKey))
            XCTAssertThrowsError(
                try JSONEncoder().encode(
                    ToriiContractDynamicAccessHint(
                        baseKey: baseKey,
                        keyType: "AccountId",
                        boundKind: "take",
                        maxKeys: 1
                    )
                )
            )
        }

        let invalidKeyTypes = [
            "",
            "Json",
            "Int",
            "Amount",
            "amount",
            "AccountID",
            "Transfer",
            "StateMap",
            "StateMap<AccountId, quantity>",
            " AccountId",
            "AccountId ",
            "АccountId",
        ]
        for keyType in invalidKeyTypes {
            XCTAssertThrowsError(try decode(keyType: keyType))
            XCTAssertThrowsError(
                try JSONEncoder().encode(
                    ToriiContractDynamicAccessHint(
                        baseKey: "state:Balances",
                        keyType: keyType,
                        boundKind: "take",
                        maxKeys: 1
                    )
                )
            )
        }

        for boundKind in ["", "Range", "Take", "all", "prefix", "range ", " take"] {
            XCTAssertThrowsError(try decode(boundKind: boundKind))
            XCTAssertThrowsError(
                try JSONEncoder().encode(
                    ToriiContractDynamicAccessHint(
                        baseKey: "state:Balances",
                        keyType: "AccountId",
                        boundKind: boundKind,
                        maxKeys: 1
                    )
                )
            )
        }
        for maxKeys in [-1, 0, 65, 4_294_967_296] {
            XCTAssertThrowsError(try decode(maxKeys: maxKeys))
        }
        for maxKeys in [UInt32(0), UInt32(65), UInt32.max] {
            XCTAssertThrowsError(
                try JSONEncoder().encode(
                    ToriiContractDynamicAccessHint(
                        baseKey: "state:Balances",
                        keyType: "AccountId",
                        boundKind: "take",
                        maxKeys: maxKeys
                    )
                )
            )
        }
        XCTAssertThrowsError(try decode(includeUnknown: true))
    }

    func testContractManifestDynamicAccessHintsResolveExactDeclaredStateMaps() throws {
        let canonical = ToriiContractDynamicAccessHint(
            baseKey: "state:Balances",
            keyType: "AccountId",
            boundKind: "take",
            maxKeys: 1
        )

        func manifest(
            reads: [ToriiContractDynamicAccessHint],
            writes: [ToriiContractDynamicAccessHint],
            stateName: String = "Balances",
            stateType: String = "StateMap<AccountId, quantity>"
        ) -> ToriiContractManifest {
            ToriiContractManifest(
                accessSetHints: ToriiContractAccessSetHints(
                    dynamicReads: reads,
                    dynamicWrites: writes
                ),
                states: [
                    ToriiContractStateDescriptor(name: stateName, typeName: stateType),
                ]
            )
        }

        func payload(_ value: ToriiContractManifest) throws -> Data {
            let reads = value.accessSetHints?.dynamicReads ?? []
            let writes = value.accessSetHints?.dynamicWrites ?? []
            func render(_ hint: ToriiContractDynamicAccessHint) -> [String: Any] {
                [
                    "base_key": hint.baseKey,
                    "key_type": hint.keyType,
                    "bound_kind": hint.boundKind,
                    "max_keys": hint.maxKeys,
                ]
            }
            return try JSONSerialization.data(withJSONObject: [
                "access_set_hints": [
                    "read_keys": [],
                    "write_keys": [],
                    "dynamic_reads": reads.map(render),
                    "dynamic_writes": writes.map(render),
                ],
                "states": (value.states ?? []).map {
                    ["name": $0.name, "type_name": $0.typeName]
                },
            ])
        }

        let malformed = [
            manifest(reads: [canonical, canonical], writes: []),
            manifest(reads: [], writes: [canonical, canonical]),
            manifest(
                reads: [
                    ToriiContractDynamicAccessHint(
                        baseKey: "state:Missing",
                        keyType: "AccountId",
                        boundKind: "take",
                        maxKeys: 1
                    ),
                ],
                writes: []
            ),
            manifest(reads: [canonical], writes: [], stateType: "quantity"),
            manifest(
                reads: [
                    ToriiContractDynamicAccessHint(
                        baseKey: "state:Balances",
                        keyType: "Name",
                        boundKind: "take",
                        maxKeys: 1
                    ),
                ],
                writes: []
            ),
        ]
        for value in malformed {
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiContractManifest.self, from: payload(value))
            )
            XCTAssertThrowsError(try JSONEncoder().encode(value))
        }

        let amount = ToriiContractDynamicAccessHint(
            baseKey: "state:amount",
            keyType: "AccountId",
            boundKind: "range",
            maxKeys: 64
        )
        let accepted = manifest(
            reads: [amount],
            writes: [amount],
            stateName: "amount"
        )
        let decoded = try JSONDecoder().decode(
            ToriiContractManifest.self,
            from: payload(accepted)
        )
        XCTAssertEqual(decoded.accessSetHints?.dynamicReads.first?.baseKey, "state:amount")
        XCTAssertNoThrow(try JSONEncoder().encode(accepted))
    }

    func testContractManifestRejectsRetiredErrorNamespaces() throws {
        for retired in ["Amount", "amount"] {
            let errorPayload =
                #"{"error_codes":[{"namespace":""#
                + retired
                + #"","name":"Denied","code":7}]}"#
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiContractManifest.self,
                    from: Data(errorPayload.utf8)
                )
            ) { error in
                XCTAssertTrue(
                    String(describing: error).contains("namespace"),
                    "missing error namespace diagnostic: \(error)"
                )
            }
            XCTAssertThrowsError(
                try JSONEncoder().encode(
                    ToriiContractErrorCodeDescriptor(
                        namespace: retired,
                        name: "Denied",
                        code: 7
                    )
                ),
                "encoded retired error namespace \(retired)"
            )
        }
    }

    func testContractManifestTriggerBoundariesRejectExactAmountSourceFormOnly() throws {
        let filter = "TlJUMAAAl9+YQQ4oJZjALRf6FAto0QAKAAAAAAAAANzCjydU9+jNAgIAAAAFBAAAAAA="

        func payload(id: String, namespace: String?) -> Data {
            let encodedNamespace = namespace.map { "\"\($0)\"" } ?? "null"
            return Data(
                """
                {"id":"\(id)","repeats":{"Indefinitely":null},"filter":"\(filter)","authority":null,"metadata":{},"callback":{"namespace":\(encodedNamespace),"entrypoint":"transfer"}}
                """.utf8
            )
        }

        for (id, namespace) in [("Amount", nil), ("tick", "Amount")] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiContractTriggerDescriptor.self,
                    from: payload(id: id, namespace: namespace)
                )
            )
        }

        let lowercase = try JSONDecoder().decode(
            ToriiContractTriggerDescriptor.self,
            from: payload(id: "amount", namespace: "RemoteLedger")
        )
        XCTAssertEqual(lowercase.id, "amount")
        XCTAssertEqual(lowercase.callback.namespace, "RemoteLedger")
    }

    func testContractManifestEnforcesStrictCrossFieldInvariants() throws {
        let boolLeaf = #"{"kind":"Leaf","value":{"kind":"Bool","value":null}}"#
        let boolSchema = "{\"fields\":[{\"name\":\"flag\",\"ty\":{\"nodes\":[\(boolLeaf)]}}]}"
        let validFilter = "TlJUMAAAl9+YQQ4oJZjALRf6FAto0QAKAAAAAAAAANzCjydU9+jNAgIAAAAFBAAAAAA="

        func descriptor(name: String = "run",
                        kind: String = "Kotoage",
                        params: String = "[]",
                        argumentSchema: String = "null",
                        returnType: String = "null",
                        returnSchema: String = "null",
                        permission: String = "\"Run\"",
                        complete: String = "true",
                        skipped: String = "[]",
                        triggers: String = "[]") -> String {
            """
            {"name":"\(name)","kind":{"kind":"\(kind)","value":null},"params":\(params),"argument_schema":\(argumentSchema),"return_type":\(returnType),"return_schema":\(returnSchema),"permission":\(permission),"read_keys":[],"write_keys":[],"access_hints_complete":\(complete),"access_hints_skipped":\(skipped),"triggers":\(triggers)}
            """
        }

        func manifest(_ entrypoints: [String]) -> String {
            "{\"entrypoints\":[\(entrypoints.joined(separator: ","))]}"
        }

        func trigger(id: String = "tick", callback: String = "run") -> String {
            """
            {"id":"\(id)","repeats":{"Indefinitely":null},"filter":"\(validFilter)","authority":null,"metadata":{},"callback":{"namespace":null,"entrypoint":"\(callback)"}}
            """
        }

        let invalid = [
            #"{"unknown":true}"#,
            #"{"features_bitmap":4}"#,
            #"{"seiyaku_name":"Option"}"#,
            #"{"seiyaku_name":"__kotodama_link_private"}"#,
            #"{"seiyaku_name":"state_map_get"}"#,
            #"{"states":[{"name":"Option","type_name":"bool"}]}"#,
            #"{"error_codes":[{"namespace":"Option","name":"Denied","code":1}]}"#,
            #"{"provenance":{"signer":"fixture","signature":"sig","unknown":true}}"#,
            manifest([descriptor(permission: "null")]),
            manifest([descriptor(name: "start", kind: "Hajimari", permission: "null")]),
            manifest([descriptor(name: "始まり", kind: "Hajimari", permission: "\"Deploy\"")]),
            manifest([descriptor(name: "hajimari", kind: "View", permission: "null")]),
            manifest([descriptor(
                params: #"[{"name":"flag","type_name":"int"}]"#,
                argumentSchema: boolSchema
            )]),
            manifest([descriptor(
                returnType: "\"int\"",
                returnSchema: "{\"nodes\":[\(boolLeaf)]}"
            )]),
            manifest([descriptor(complete: "true", skipped: "[\"dynamic\"]")]),
            manifest([descriptor(complete: "false", skipped: "[]")]),
            manifest([descriptor(triggers: "[\(trigger(callback: "missing"))]")]),
            manifest([
                descriptor(name: "inspect", kind: "View", permission: "null"),
                descriptor(triggers: "[\(trigger(callback: "inspect"))]"),
            ]),
            manifest([descriptor(triggers: "[\(trigger()),\(trigger())]")]),
            manifest([descriptor(triggers: "[\(trigger().replacingOccurrences(of: #"{"Indefinitely":null}"#, with: #"{"kind":"Indefinitely","value":null}"#))]")]),
            manifest([descriptor(triggers: "[\(trigger().replacingOccurrences(of: validFilter, with: "%%%"))]")]),
            manifest([descriptor().replacingOccurrences(
                of: #""triggers":[]"#,
                with: #""triggers":[],"unknown":true"#
            )]),
        ]

        for payload in invalid {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiContractManifest.self,
                    from: Data(payload.utf8)
                ),
                "accepted inconsistent manifest payload: \(payload)"
            )
        }
    }

    func testContractManifestAcceptsRomanizedAndJapaneseLifecycleSelectors() throws {
        let selectors = [
            ("hajimari", "Hajimari"),
            ("始まり", "Hajimari"),
            ("kaizen", "Kaizen"),
            ("改善", "Kaizen"),
        ]
        for (name, kind) in selectors {
            let payload = """
            {"entrypoints":[{"name":"\(name)","kind":{"kind":"\(kind)","value":null},"params":[],"argument_schema":null,"return_type":null,"return_schema":null,"permission":null,"read_keys":[],"write_keys":[],"access_hints_complete":true,"access_hints_skipped":[],"triggers":[]}]}
            """
            let manifest = try JSONDecoder().decode(
                ToriiContractManifest.self,
                from: Data(payload.utf8)
            )
            XCTAssertEqual(manifest.entrypoints?.first?.name, name)
        }
    }

    func testContractManifestAcceptsOnlyBrandedV1EntrypointKinds() throws {
        let cases: [(String, ToriiContractEntrypointKind)] = [
            ("Kotoage", .kotoage),
            ("View", .view),
            ("Hajimari", .hajimari),
            ("Kaizen", .kaizen),
        ]
        for (label, expected) in cases {
            let payload = #"{"kind":"\#(label)","value":null}"#
            XCTAssertEqual(
                try JSONDecoder().decode(
                    ToriiContractEntrypointKind.self,
                    from: Data(payload.utf8)
                ),
                expected
            )
        }
    }

    func testContractManifestAcceptsOnlyFirstReleaseNumericLeafKinds() throws {
        for (label, expected, canonicalName) in [
            ("Int", ToriiEntrypointValueKindV1.int, "int"),
            ("Decimal", ToriiEntrypointValueKindV1.decimal, "decimal"),
            ("Quantity", ToriiEntrypointValueKindV1.quantity, "quantity"),
        ] {
            let payload = #"{"nodes":[{"kind":"Leaf","value":{"kind":"\#(label)","value":null}}]}"#
            let valueType = try JSONDecoder().decode(
                ToriiEntrypointValueTypeV1.self,
                from: Data(payload.utf8)
            )
            guard case let .leaf(kind) = valueType.nodes[0] else {
                return XCTFail("expected numeric leaf")
            }
            XCTAssertEqual(kind, expected)
            XCTAssertEqual(valueType.canonicalTypeName, canonicalName)
        }

        for retired in ["U128", "Amount"] {
            let payload = #"{"nodes":[{"kind":"Leaf","value":{"kind":"\#(retired)","value":null}}]}"#
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiEntrypointValueTypeV1.self,
                    from: Data(payload.utf8)
                )
            )
        }
    }

    func testContractManifestDecodesCheckedInCanonicalKotodamaManifests() throws {
        let fixtures = [
            ("authority_probe.manifest.json", "AuthorityProbe"),
            ("irohaswap.manifest.json", "IrohaSwap"),
            ("ivm_smoke.manifest.json", "SmokeTransfer"),
            ("prediction_market.manifest.json", "PredictionMarket"),
        ]
        let demo = repositoryRootURL().appendingPathComponent("demo", isDirectory: true)
        for (filename, expectedName) in fixtures {
            let data = try Data(contentsOf: demo.appendingPathComponent(filename))
            let manifest = try JSONDecoder().decode(ToriiContractManifest.self, from: data)
            XCTAssertEqual(manifest.seiyakuName, expectedName)
            XCTAssertEqual(manifest.codeHash?.count, 64)
            XCTAssertEqual(manifest.abiHash?.count, 64)
            XCTAssertFalse(manifest.entrypoints?.isEmpty ?? true)
        }
    }

    func testDeployContractInstanceRejectsRemovedServerSideSigningFlow() async {
        let manifest = ToriiContractManifest(compilerFingerprint: "kotodama-0.8")
        let req = ToriiDeployContractInstanceRequest(authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
        let req = ToriiActivateContractInstanceRequest(authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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

    func testContractCallBoundaryConsumesSharedRustArgumentRecordFixture() throws {
        let fixtureURL = repositoryRootURL()
            .appendingPathComponent("fixtures/kotodama/entrypoint_argument_record_v1.json")
        let fixtureData = try Data(contentsOf: fixtureURL)
        guard let fixture = try JSONSerialization.jsonObject(with: fixtureData) as? [String: Any],
              let boundary = fixture["torii_boundary"] as? [String: Any],
              let authority = boundary["authority"] as? String,
              let contractAlias = boundary["contract_alias"] as? String,
              let entrypoint = boundary["entrypoint"] as? String,
              let fixturePayload = boundary["payload"] as? [String: Any],
              let fixtureFeePayment = boundary["fee_payment"] as? [String: Any],
              let schema = fixture["entrypoint_argument_schema_v1"] as? [String: Any],
              let schemaHash = schema["schema_hash_hex"] as? String,
              let record = fixture["entrypoint_argument_record_v1"] as? [String: Any],
              let recordHex = record["norito_hex"] as? String else {
            return XCTFail("invalid shared Kotodama argument-record fixture")
        }
        XCTAssertEqual(fixture["codec"] as? String, "EntrypointArgumentRecordV1")
        XCTAssertEqual(fixture["generator"] as? String, "ivm::encode_argument_record_from_json")
        XCTAssertNotNil(schemaHash.range(of: "^[0-9a-f]{64}$", options: .regularExpression))
        XCTAssertNotNil(recordHex.range(of: "^(?:[0-9a-f]{2})+$", options: .regularExpression))

        let payloadData = try JSONSerialization.data(withJSONObject: fixturePayload)
        let payload = try JSONDecoder().decode(ToriiJSONValue.self, from: payloadData)
        let feePaymentData = try JSONSerialization.data(withJSONObject: fixtureFeePayment)
        let feePayment = try JSONDecoder().decode(FeePaymentIntent.self, from: feePaymentData)
        let request = ToriiContractCallRequest(
            authority: authority,
            contractAlias: contractAlias,
            entrypoint: entrypoint,
            payload: payload,
            feePayment: feePayment
        )
        let encoded = try JSONEncoder().encode(request)
        guard let submitted = try JSONSerialization.jsonObject(with: encoded) as? [String: Any],
              let submittedPayload = submitted["payload"] as? [String: Any] else {
            return XCTFail("contract call did not encode a JSON object")
        }

        XCTAssertEqual(submitted["authority"] as? String, authority)
        XCTAssertEqual(submitted["contract_alias"] as? String, contractAlias)
        XCTAssertNil(submitted["contract_address"])
        XCTAssertEqual(submitted["entrypoint"] as? String, entrypoint)
        XCTAssertEqual(submittedPayload as NSDictionary, fixturePayload as NSDictionary)
        XCTAssertEqual(
            submitted["fee_payment"] as? NSDictionary,
            fixtureFeePayment as NSDictionary
        )
        XCTAssertNil(submitted["gas_limit"])
        XCTAssertNil(submitted["argument_record"])
        XCTAssertNil(submitted["argument_record_norito_hex"])
    }

    func testCallContractParsesResponse() {
        let expectation = expectation(description: "call contract")
        let feePayment = testFeePayment(gasLimit: 7)
        let codeHash = String(repeating: "d", count: 64)
        let abiHash = String(repeating: "e", count: 64)
        let txHash = String(repeating: "f", count: 64)
        let entrypointHash = String(repeating: "a", count: 64)
        let payloadDigest = String(repeating: "b", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/contracts/call")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            guard let body = self.bodyData(from: request),
                  let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
                XCTFail("missing JSON body")
                throw NSError(domain: "stub", code: -1)
            }
            XCTAssertEqual(json["authority"] as? String, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
            XCTAssertNil(json["private_key"])
            XCTAssertEqual(json["public_key_hex"] as? String, String(repeating: "1", count: 64))
            XCTAssertEqual(json["signature_b64"] as? String, "AQ==")
            XCTAssertEqual(json["contract_alias"] as? String, "mint::universal")
            XCTAssertEqual(json["entrypoint"] as? String, "create")
            XCTAssertEqual(
                json["fee_payment"] as? NSDictionary,
                testFeePaymentObject(feePayment) as NSDictionary
            )
            XCTAssertNil(json["gas_limit"])
            XCTAssertNil(json["gas_asset_id"])
            XCTAssertNil(json["fee_sponsor"])
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"submitted":true,"dataspace":"universal","contract_address":"irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh","code_hash_hex":"\(codeHash)","abi_hash_hex":"\(abiHash)","creation_time_ms":321,"transaction_ttl_ms":60000,"tx_hash_hex":"\(txHash)","pipeline_status":{"hash":"\(txHash)","status":{"kind":"Rejected","block_height":12,"rejection_reason":{"Validation":"missing permission"}},"summary":"Rejected: missing permission","diagnostics":[{"category":"validation","code":"validation","message":"missing permission","decoded_reason":"missing permission","raw_reason":"Validation(missing permission)"}],"scope":"local","resolved_from":"state"},"entrypoint_hash_hex":"\(entrypointHash)","entrypoint":"create","operation_receipt":{"operation_kind":"contract_call","status":"submitted","transport":"torii","dataspace":"universal","contract_alias":"mint::universal","contract_address":"irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh","code_hash_hex":"\(codeHash)","abi_hash_hex":"\(abiHash)","tx_hash_hex":"\(txHash)","entrypoint":"create","entrypoint_hash_hex":"\(entrypointHash)","gas_limit":7,"gas_used":3,"fee_payment":{"payer":"authority","value":{"charge_limits":[],"gas_limit":7}},"payload_digest_hex":"\(payloadDigest)"}}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiContractCallRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            publicKeyHex: String(repeating: "1", count: 64),
            signatureB64: "AQ==",
            contractAlias: "mint::universal",
            entrypoint: "create",
            payload: .object(["amount": .string("10")]),
            creationTimeMs: 321,
            feePayment: feePayment
        )
        makeClient().callContract(request) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
                XCTAssertTrue(response.submitted)
                XCTAssertEqual(response.dataspace, "universal")
                XCTAssertEqual(response.contractAddress, "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh")
                XCTAssertEqual(response.codeHashHex, codeHash)
                XCTAssertEqual(response.abiHashHex, abiHash)
                XCTAssertEqual(response.creationTimeMs, 321)
                XCTAssertEqual(response.txHashHex, txHash)
                XCTAssertEqual(response.pipelineStatus?.status.blockHeight, 12)
                XCTAssertEqual(response.pipelineStatus?.isRejected, true)
                XCTAssertEqual(response.transactionTtlMs, 60_000)
                XCTAssertEqual(response.entrypointHashHex, entrypointHash)
                XCTAssertNil(response.transactionPayloadB64)
                XCTAssertNil(response.signingMessageB64)
                XCTAssertEqual(response.entrypoint, "create")
                XCTAssertEqual(response.operationReceipt.operationKind, "contract_call")
                XCTAssertEqual(response.operationReceipt.gasLimit, 7)
                XCTAssertEqual(response.operationReceipt.gasUsed, 3)
                XCTAssertEqual(response.operationReceipt.feePayment, feePayment)
                XCTAssertEqual(response.operationReceipt.payloadDigestHex, payloadDigest)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testCallContractRejectsZeroGasLimit() {
        let request = ToriiContractCallRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            contractAlias: "mint::universal",
            entrypoint: "create",
            feePayment: testFeePayment(gasLimit: 0)
        )
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testCallContractRejectsAmbiguousContractTarget() {
        let request = ToriiContractCallRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            contractAddress: "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh",
            contractAlias: "mint::universal",
            entrypoint: "create",
            feePayment: testFeePayment(gasLimit: 7)
        )
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testCallContractRejectsBlankEntrypoint() {
        let request = ToriiContractCallRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            contractAlias: "mint::universal",
            entrypoint: "   ",
            feePayment: testFeePayment(gasLimit: 7)
        )
        XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testCallContractResponseRequiresOperationReceipt() {
        let codeHash = String(repeating: "d", count: 64)
        let abiHash = String(repeating: "e", count: 64)
        let payload = """
        {"ok":true,"submitted":true,"dataspace":"universal","code_hash_hex":"\(codeHash)","abi_hash_hex":"\(abiHash)","creation_time_ms":1,"entrypoint":"create"}
        """.data(using: .utf8)!

        XCTAssertThrowsError(try JSONDecoder().decode(ToriiContractCallResponse.self, from: payload))
    }

    func testProposeMultisigEncodesStructuredJsonInstructions() throws {
        let expectation = expectation(description: "propose multisig")
        let proposalId = String(repeating: "a", count: 64)
        let feePayment = testFeePayment()
        let resolvedAccount = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let transactionPayload = try CanonicalUnsignedTransactionTestSupport.genericPayload(
            authority: resolvedAccount,
            creationTimeMs: 123,
            feePayment: feePayment
        )
        let signingMessage = IrohaHash.hash(transactionPayload)
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
            XCTAssertEqual(json["signer_account_id"] as? String, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
            XCTAssertEqual(json["creation_time_ms"] as? Int, 123)
            XCTAssertEqual(
                json["fee_payment"] as? NSDictionary,
                testFeePaymentObject(feePayment) as NSDictionary
            )
            XCTAssertNil(json["fee_sponsor"])
            XCTAssertNil(json["validation_fee_policy_version"])
            XCTAssertNil(json["validation_fee_policy_hash"])
            XCTAssertNil(json["validation_fee_instruction_index"])
            XCTAssertNil(json["validation_fee_transfer_entry_index"])
            let instructions = json["instructions"] as? [[String: Any]]
            XCTAssertEqual(instructions?.first?["kind"] as? String, "Transfer")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"resolved_multisig_account_id":"\(resolvedAccount)","submitted":false,"proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","creation_time_ms":123,"transaction_payload_b64":"\(transactionPayload.base64EncodedString())","signing_message_b64":"\(signingMessage.base64EncodedString())"}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            signerAccountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            creationTimeMs: 123,
            instructions: [
                try ToriiMultisigProposeInstruction(object: ["kind": .string("Transfer")])
            ],
            feePayment: feePayment
        )
        makeClient().proposeMultisig(request) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
                XCTAssertEqual(response.resolvedMultisigAccountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
                XCTAssertEqual(response.proposalId, proposalId)
                XCTAssertEqual(response.instructionsHash, proposalId)
                XCTAssertEqual(response.creationTimeMs, 123)
                XCTAssertEqual(response.transactionPayloadB64, transactionPayload.base64EncodedString())
                XCTAssertEqual(response.signingMessageB64, signingMessage.base64EncodedString())
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testProposeMultisigEncodesValidationFeePolicyMetadataAsCanonicalStrings() throws {
        let signer = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let request = ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountId: signer),
            signerAccountId: signer,
            validationFeePolicyVersion: 7,
            validationFeePolicyHash: "0X" + String(repeating: "AB", count: 32),
            validationFeeInstructionIndex: 1,
            validationFeeTransferEntryIndex: 2,
            instructions: [try ToriiMultisigProposeInstruction(base64: "AQID")],
            feePayment: .authority(chargeLimits: [], gasLimit: nil)
        )

        let payload = try XCTUnwrap(
            JSONSerialization.jsonObject(with: JSONEncoder().encode(request)) as? [String: Any]
        )
        XCTAssertEqual(payload["validation_fee_policy_version"] as? String, "7")
        XCTAssertEqual(payload["validation_fee_policy_hash"] as? String, String(repeating: "ab", count: 32))
        XCTAssertEqual(payload["validation_fee_instruction_index"] as? String, "1")
        XCTAssertEqual(payload["validation_fee_transfer_entry_index"] as? String, "2")
    }

    func testProposeMultisigRejectsIncompleteValidationFeePolicyMetadata() throws {
        let signer = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let instruction = try ToriiMultisigProposeInstruction(base64: "AQID")
        let feePayment = FeePaymentIntent.authority(chargeLimits: [], gasLimit: nil)
        let hash = String(repeating: "ab", count: 32)
        let malformedRequests = [
            ToriiMultisigProposeRequest(
                selector: ToriiMultisigAccountSelector(multisigAccountId: signer),
                signerAccountId: signer,
                validationFeePolicyVersion: 7,
                instructions: [instruction],
                feePayment: feePayment
            ),
            ToriiMultisigProposeRequest(
                selector: ToriiMultisigAccountSelector(multisigAccountId: signer),
                signerAccountId: signer,
                validationFeePolicyHash: hash,
                instructions: [instruction],
                feePayment: feePayment
            ),
            ToriiMultisigProposeRequest(
                selector: ToriiMultisigAccountSelector(multisigAccountId: signer),
                signerAccountId: signer,
                validationFeeInstructionIndex: 1,
                instructions: [instruction],
                feePayment: feePayment
            ),
            ToriiMultisigProposeRequest(
                selector: ToriiMultisigAccountSelector(multisigAccountId: signer),
                signerAccountId: signer,
                validationFeePolicyVersion: 7,
                validationFeePolicyHash: hash,
                validationFeeTransferEntryIndex: 2,
                instructions: [instruction],
                feePayment: feePayment
            ),
            ToriiMultisigProposeRequest(
                selector: ToriiMultisigAccountSelector(multisigAccountId: signer),
                signerAccountId: signer,
                validationFeePolicyVersion: 7,
                validationFeePolicyHash: "not-a-policy-hash",
                validationFeeInstructionIndex: 0,
                instructions: [instruction],
                feePayment: feePayment
            )
        ]

        for request in malformedRequests {
            XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
                guard case ToriiClientError.invalidPayload = error else {
                    return XCTFail("Expected invalidPayload, got \(error)")
                }
            }
        }
    }

    func testProposeMultisigSendsWholeNoritoDtoBody() {
        let expectation = expectation(description: "propose multisig native body")
        let proposalId = String(repeating: "b", count: 64)
        let noritoBody = Data([0x4e, 0x52, 0x54, 0x30, 0x01, 0x02, 0x03])
        let resolvedAccount = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let transactionPayload = try! CanonicalUnsignedTransactionTestSupport.genericPayload(
            authority: resolvedAccount
        )
        let signingMessage = IrohaHash.hash(transactionPayload)
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
            {"ok":true,"resolved_multisig_account_id":"\(resolvedAccount)","submitted":false,"proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","transaction_payload_b64":"\(transactionPayload.base64EncodedString())","signing_message_b64":"\(signingMessage.base64EncodedString())"}
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

    func testProposeMultisigInstructionEncodesNativeInstructionStrings() throws {
        let base64Instruction = try ToriiMultisigProposeInstruction(base64: "AQID")
        let encodedBase64 = try JSONEncoder().encode(base64Instruction)
        XCTAssertEqual(String(data: encodedBase64, encoding: .utf8), "\"AQID\"")

        let bytesInstruction = try ToriiMultisigProposeInstruction(
            noritoInstructionBoxBytes: Data([0x4e, 0x52, 0x54, 0x30])
        )
        let encodedBytes = try JSONEncoder().encode(bytesInstruction)
        XCTAssertEqual(String(data: encodedBytes, encoding: .utf8), "\"TlJUMA==\"")

        XCTAssertThrowsError(try ToriiMultisigProposeInstruction(json: .string("AQID"))) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }
    }

    func testSwiftNexusTransferInstructionBoxUsesNativeNoritoWhenBridgeAvailable() throws {
        let authority = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let destination = "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
        let sourceAsset = "61CtjvNd9T3THAR65GsMVHr82Bjc#\(authority)"
        let instruction = try SwiftNexusTransactionCodec().buildTransferInstructionBox(
            input: NexusTransferInput(
                sourceAssetID: sourceAsset,
                quantity: "5",
                destinationAccountID: destination,
                feePayment: .authority(chargeLimits: [], gasLimit: nil),
            ),
            accountChainDiscriminant: 753
        )
        let nativeMagic = Data([0x4e, 0x52, 0x54, 0x30])
        try requireNativeTestCapability(
            instruction.prefix(nativeMagic.count) == nativeMagic,
            "Native transfer InstructionBox bridge is not available in this artifact."
        )

        let wrapped = try ToriiMultisigProposeInstruction(noritoInstructionBoxBytes: instruction)
        let encoded = try JSONEncoder().encode(wrapped)
        let base64 = try JSONDecoder().decode(String.self, from: encoded)
        let decoded = try XCTUnwrap(Data(base64Encoded: base64))
        XCTAssertEqual(decoded.prefix(nativeMagic.count), nativeMagic)
    }

    func testProposeMultisigRejectsEmptyInstructionBytesAndBadRequestShape() {
        XCTAssertThrowsError(try ToriiMultisigProposeInstruction(noritoInstructionBoxBytes: Data())) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }

        let signer = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let instruction = try! ToriiMultisigProposeInstruction(object: ["kind": .string("Transfer")])
        let ambiguousSelectorRequest = ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(
                multisigAccountId: signer,
                multisigAccountAlias: "cbdc@banka"
            ),
            signerAccountId: signer,
            instructions: [instruction],
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
        )
        XCTAssertThrowsError(try JSONEncoder().encode(ambiguousSelectorRequest)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload error")
            }
        }

        let emptyBatchRequest = ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            signerAccountId: signer,
            instructions: [],
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
            {"ok":true,"resolved_multisig_account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","instructions_hash":"aa","signing_message_b64":"not base64"}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            signerAccountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            instructions: [instruction],
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
            {"ok":false,"resolved_multisig_account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            signerAccountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            instructions: [instruction],
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
            {"ok":true,"resolved_multisig_account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","signing_message_b64":""}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            signerAccountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            instructions: [instruction],
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
            {"ok":true,"resolved_multisig_account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","creation_time_ms":-1}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            signerAccountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            instructions: [instruction],
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
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
        let feePayment = testFeePayment(gasLimit: 5)
        let resolvedAccount = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let transactionPayload = try! CanonicalUnsignedTransactionTestSupport.genericPayload(
            authority: resolvedAccount,
            creationTimeMs: 123,
            feePayment: feePayment
        )
        let signingMessage = IrohaHash.hash(transactionPayload)
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
            XCTAssertEqual(json["signer_account_id"] as? String, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
            XCTAssertEqual(json["contract_alias"] as? String, "mint::universal")
            XCTAssertEqual(json["entrypoint"] as? String, "execute")
            XCTAssertEqual(
                json["fee_payment"] as? NSDictionary,
                testFeePaymentObject(feePayment) as NSDictionary
            )
            XCTAssertNil(json["gas_limit"])
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"resolved_multisig_account_id":"\(resolvedAccount)","submitted":false,"proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","creation_time_ms":123,"transaction_payload_b64":"\(transactionPayload.base64EncodedString())","signing_message_b64":"\(signingMessage.base64EncodedString())"}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigContractCallProposeRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            signerAccountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            contractAlias: "mint::universal",
            entrypoint: "execute",
            payload: .object(["amount": .string("10")]),
            feePayment: feePayment
        )
        makeClient().proposeMultisigContractCall(request) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
                XCTAssertEqual(response.resolvedMultisigAccountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
                XCTAssertEqual(response.proposalId, proposalId)
                XCTAssertEqual(response.instructionsHash, proposalId)
                XCTAssertEqual(response.creationTimeMs, 123)
                XCTAssertEqual(response.transactionPayloadB64, transactionPayload.base64EncodedString())
                XCTAssertEqual(response.signingMessageB64, signingMessage.base64EncodedString())
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testSignatureB64RequestsRejectNoncanonicalBase64Text() throws {
        let account = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let signer = "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
        let canonicalSignature = Data(repeating: 0x01, count: 64).base64EncodedString()
        let invalidSignatures = [
            " \(canonicalSignature)",
            noncanonicalStandardBase64PadBitAlias(canonicalSignature)
        ]

        for signatureB64 in invalidSignatures {
            XCTAssertThrowsError(
                try JSONEncoder().encode(
                    ToriiContractCallRequest(
                        authority: account,
                        signatureB64: signatureB64,
                        contractAlias: "mint::universal",
                        entrypoint: "create",
                        feePayment: testFeePayment(gasLimit: 1)
                    )
                )
            ) { error in
                XCTAssertTrue("\(error)".contains("signature_b64"))
            }

            XCTAssertThrowsError(
                try JSONEncoder().encode(
                    ToriiMultisigProposeRequest(
                        selector: ToriiMultisigAccountSelector(multisigAccountId: account),
                        signerAccountId: signer,
                        signatureB64: signatureB64,
                        instructions: [try ToriiMultisigProposeInstruction(base64: "AQID")],
                        feePayment: .authority(chargeLimits: [], gasLimit: nil),
                    )
                )
            ) { error in
                XCTAssertTrue("\(error)".contains("signature_b64"))
            }

            XCTAssertThrowsError(
                try JSONEncoder().encode(
                    ToriiMultisigContractCallProposeRequest(
                        selector: ToriiMultisigAccountSelector(multisigAccountId: account),
                        signerAccountId: signer,
                        signatureB64: signatureB64,
                        contractAlias: "mint::universal",
                        entrypoint: "create",
                        feePayment: testFeePayment(gasLimit: 1)
                    )
                )
            ) { error in
                XCTAssertTrue("\(error)".contains("signature_b64"))
            }

            XCTAssertThrowsError(
                try JSONEncoder().encode(
                    ToriiMultisigContractCallApproveRequest(
                        selector: ToriiMultisigAccountSelector(multisigAccountId: account),
                        signerAccountId: signer,
                        signatureB64: signatureB64,
                        proposalId: String(repeating: "b", count: 64),
                        feePayment: testFeePayment()
                    )
                )
            ) { error in
                XCTAssertTrue("\(error)".contains("signature_b64"))
            }

        }
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
            XCTAssertEqual(json["multisig_account_id"] as? String, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
            XCTAssertEqual(json["signer_account_id"] as? String, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
            XCTAssertEqual(json["proposal_id"] as? String, proposalId)
            XCTAssertEqual(json["signature_b64"] as? String, "AQ==")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"ok":true,"resolved_multisig_account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","submitted":true,"proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","tx_hash_hex":"\(txHash)","executed_tx_hash_hex":"\(txHash)"}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigContractCallApproveRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"),
            signerAccountId: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            signatureB64: "AQ==",
            proposalId: proposalId,
            feePayment: testFeePayment()
        )
        makeClient().approveMultisigContractCall(request) { result in
            switch result {
            case .success(let response):
                XCTAssertTrue(response.ok)
                XCTAssertEqual(response.proposalId, proposalId)
                XCTAssertEqual(response.txHashHex, txHash)
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
            {"resolved_multisig_account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","spec":{"quorum":2,"transaction_ttl_ms":60000}}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigSpecRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@bankb")
        )
        makeClient().getMultisigSpec(request, canonicalAuth: canonicalReadAuth) { result in
            switch result {
            case .success(let response):
                XCTAssertEqual(response.resolvedMultisigAccountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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
            {"resolved_multisig_account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","spec":{"quorum":2,"transaction_ttl_ms":60000}}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigSpecRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka.universal")
        )
        makeClient().getMultisigSpec(request, canonicalAuth: canonicalReadAuth) { result in
            switch result {
            case .success(let response):
                XCTAssertEqual(response.resolvedMultisigAccountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
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

    func testQueryMultisigProposalsDecodesEntries() {
        let expectation = expectation(description: "multisig proposals query")
        let proposalId = String(repeating: "d", count: 64)
        let approverId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/multisig/proposals/query")
            XCTAssertNotEqual(request.url?.path, "/v1/multisig/proposals/list")
            guard let body = self.bodyData(from: request),
                  let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] else {
                XCTFail("missing JSON body")
                throw NSError(domain: "stub", code: -1)
            }
            XCTAssertEqual(json["status"] as? [String], ["FINALIZED"])
            XCTAssertEqual(json["cursor"] as? String, "page-1")
            XCTAssertEqual(json["limit"] as? Int, 25)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let bodyData = """
            {"resolved_multisig_account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","proposals":[{"proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","operation_type":"TRANSFER","intent":{"asset_definition_id":"pkr#sbp"},"proposal":{"approvals":["\(approverId)"]},"status":"FINALIZED","terminal_at_ms":123}],"next_cursor":"page-2"}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposalsQueryRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            status: [.finalized],
            cursor: "page-1",
            limit: 25
        )
        makeClient().queryMultisigProposals(request, canonicalAuth: canonicalReadAuth) { result in
            switch result {
            case .success(let response):
                XCTAssertEqual(response.proposals.count, 1)
                XCTAssertEqual(response.proposals.first?.proposalId, proposalId)
                XCTAssertEqual(response.proposals.first?.operationType, "TRANSFER")
                XCTAssertEqual(response.proposals.first?.status, .finalized)
                XCTAssertEqual(response.proposals.first?.terminalAtMs, 123)
                XCTAssertEqual(response.nextCursor, "page-2")
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testResolveMultisigProposalDecodesProposalResolveResponse() {
        let expectation = expectation(description: "multisig proposal resolve")
        let proposalId = String(repeating: "e", count: 64)
        let approverOne = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let approverTwo = "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/multisig/proposals/resolve")
            XCTAssertNotEqual(request.url?.path, "/v1/multisig/proposals/get")
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
            {"resolved_multisig_account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","operation_type":"TRANSFER","intent":null,"proposal":{"approvals":["\(approverOne)","\(approverTwo)"]},"status":"COLLECTING_SIGNATURES","terminal_at_ms":null}
            """.data(using: .utf8)!
            return (response, bodyData)
        }

        let request = ToriiMultisigProposalsResolveRequest(
            selector: ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka"),
            instructionsHash: proposalId
        )
        makeClient().resolveMultisigProposal(request, canonicalAuth: canonicalReadAuth) { result in
            switch result {
            case .success(let response):
                XCTAssertEqual(response.proposalId, proposalId)
                XCTAssertEqual(response.instructionsHash, proposalId)
                XCTAssertEqual(response.operationType, "TRANSFER")
                XCTAssertEqual(response.status, .collectingSignatures)
                XCTAssertNil(response.terminalAtMs)
            case .failure(let error):
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testQueryMultisigProposalsRejectsInvalidPaginationAndDuplicateStatuses() throws {
        let selector = ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka")
        for request in [
            ToriiMultisigProposalsQueryRequest(
                selector: selector,
                status: [.finalized, .finalized]
            ),
            ToriiMultisigProposalsQueryRequest(selector: selector, cursor: " "),
            ToriiMultisigProposalsQueryRequest(selector: selector, limit: 0),
            ToriiMultisigProposalsQueryRequest(
                selector: selector,
                cursor: String(repeating: "x", count: 513)
            )
        ] {
            XCTAssertThrowsError(try JSONEncoder().encode(request))
        }
    }

    func testResolveMultisigProposalRejectsMissingOrDualProposalSelectors() throws {
        let selector = ToriiMultisigAccountSelector(multisigAccountAlias: "cbdc@banka")
        let proposalId = String(repeating: "f", count: 64)
        for request in [
            ToriiMultisigProposalsResolveRequest(selector: selector),
            ToriiMultisigProposalsResolveRequest(
                selector: selector,
                proposalId: proposalId,
                instructionsHash: proposalId
            )
        ] {
            XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
                guard case let ToriiClientError.invalidPayload(message) = error else {
                    return XCTFail("unexpected error: \(error)")
                }
                XCTAssertTrue(message.contains("exactly one"))
            }
        }
    }

    func testMultisigProposalResponseRejectsMissingCurrentContractFields() throws {
        let proposalId = String(repeating: "f", count: 64)
        let missingStatus = """
        {"resolved_multisig_account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","operation_type":"TRANSFER","proposal":{}}
        """.data(using: .utf8)!
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiMultisigProposalResolveResponse.self, from: missingStatus))

        let unknownStatus = """
        {"resolved_multisig_account_id":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","proposal_id":"\(proposalId)","instructions_hash":"\(proposalId)","operation_type":"TRANSFER","intent":null,"proposal":{},"status":"READY_TO_SUBMIT","terminal_at_ms":null}
        """.data(using: .utf8)!
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiMultisigProposalResolveResponse.self, from: unknownStatus))
    }

    func testMultisigSelectorRejectsBothAccountIdAndAlias() throws {
        let selector = ToriiMultisigAccountSelector(
            multisigAccountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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

        makeClient().fetchContractCodeBytes(codeHashHex: codeHash, canonicalAuth: canonicalReadAuth) { result in
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

        makeClient().fetchContractCodeBytes(codeHashHex: codeHash, canonicalAuth: canonicalReadAuth) { result in
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
                {"payload":{"entrypoint_hash":"abc","signed_transaction_hash":null,"submitted_at_ms":1,"submitted_at_height":2,"signer":"signer"},"signature":"deadbeef"}
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
        let encodedCodeHash = "BlAkE2b32:0X\(codeHash)"
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
            XCTAssertNil(body["limits"])
            let provenance = body["manifest_provenance"] as? [String: Any]
            XCTAssertEqual(provenance?["signer"] as? String, "ed25519:public")
            XCTAssertEqual(provenance?["signature"] as? String, "ed25519:signature")
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
                                                                   codeHashHex: encodedCodeHash,
                                                                   abiHashHex: abiHash,
                                                                   abiVersion: "1",
                                                                   window: ToriiGovernanceWindow(lower: 10, upper: 20),
                                                                   mode: .plain,
                                                                   manifestProvenance: .init(
                                                                    signer: "ed25519:public",
                                                                    signature: "ed25519:signature"
                                                                   ))
        makeClient().submitGovernanceDeployContractProposal(request, canonicalAuth: canonicalReadAuth) { result in
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
            contractAddress: "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh",
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

    func testGovernanceDeployContractProposalRejectsNonV1AbiVersion() {
        for abiVersion in ["2", "01", " 1", "1 "] {
            let request = ToriiGovernanceDeployContractProposalRequest(
                contractAlias: "demo::universal",
                codeHashHex: String(repeating: "4", count: 64),
                abiHashHex: String(repeating: "5", count: 64),
                abiVersion: abiVersion
            )
            XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
                guard case let ToriiClientError.invalidPayload(message) = error else {
                    return XCTFail("unexpected error: \(error)")
                }
                XCTAssertTrue(message.contains("abi_version must be exactly 1"))
            }
        }
    }

    func testGovernanceWindowEncodesEntireUInt64Domain() throws {
        let window = ToriiGovernanceWindow(lower: 0, upper: UInt64.max)
        let data = try JSONEncoder().encode(window)
        let decoded = try JSONDecoder().decode(ToriiGovernanceWindow.self, from: data)
        XCTAssertEqual(decoded.lower, 0)
        XCTAssertEqual(decoded.upper, UInt64.max)
        XCTAssertTrue(
            String(decoding: data, as: UTF8.self)
                .contains("\"upper\":18446744073709551615")
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGovernanceWindowRejectsReversedBoundsBeforeTransportDispatch() async {
        var dispatched = false
        StubURLProtocol.handler = { request in
            dispatched = true
            XCTFail("reversed governance window reached transport dispatch: \(request)")
            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 500,
                    httpVersion: nil,
                    headerFields: nil
                )!,
                Data()
            )
        }
        let request = ToriiGovernanceDeployContractProposalRequest(
            contractAlias: "demo::universal",
            codeHashHex: String(repeating: "4", count: 64),
            abiHashHex: String(repeating: "5", count: 64),
            window: .init(lower: 2, upper: 1)
        )

        await XCTAssertThrowsErrorAsync(
            try await makeClient().submitGovernanceDeployContractProposal(request, canonicalAuth: canonicalReadAuth)
        ) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertTrue(reason.contains("upper bound must not precede"))
        }
        XCTAssertFalse(dispatched)
    }

    func testGovernanceDeployContractProposalRejectsUndeclaredHashAliases() {
        let hash = String(repeating: "4", count: 64)
        for codeHash in [
            ":\(hash)",
            " \(hash)",
            "\(hash) ",
            "blake2b32:\(hash):ignored",
            "sha256:\(hash)"
        ] {
            let request = ToriiGovernanceDeployContractProposalRequest(
                contractAlias: "demo::universal",
                codeHashHex: codeHash,
                abiHashHex: String(repeating: "5", count: 64)
            )
            XCTAssertThrowsError(try JSONEncoder().encode(request)) { error in
                guard case let ToriiClientError.invalidPayload(message) = error else {
                    return XCTFail("unexpected error: \(error)")
                }
                XCTAssertTrue(message.contains("code_hash must be a 32-byte hex string"))
            }
        }
    }

    func testFinalizeGovernanceRequiresOneExactProposalFingerprint() throws {
        let proposalId = String(repeating: "ab", count: 32)
        let request = ToriiGovernanceFinalizeRequest(
            referendumId: proposalId,
            proposalId: proposalId
        )
        let data = try JSONEncoder().encode(request)
        let json = try XCTUnwrap(
            JSONSerialization.jsonObject(with: data) as? [String: Any]
        )
        XCTAssertEqual(json["referendum_id"] as? String, proposalId)
        XCTAssertEqual(json["proposal_id"] as? String, proposalId)

        for invalidReferendum in [
            "", "ref-1", " ref-1", "ref-1 ", "ref 1", "ref\t1", "ref\u{0000}1",
            "ref/1", ".hidden", "ref%31", "投票", String(repeating: "a", count: 129),
            "0x\(proposalId)", proposalId.uppercased()
        ] {
            XCTAssertThrowsError(
                try JSONEncoder().encode(
                    ToriiGovernanceFinalizeRequest(
                        referendumId: invalidReferendum,
                        proposalId: proposalId
                    )
                )
            )
        }
        for invalidProposal in [
            ":\(proposalId)",
            "0x\(proposalId)",
            proposalId.uppercased(),
            " \(proposalId)",
            "\(proposalId) ",
            "blake2b32:\(proposalId)",
            "sha256:\(proposalId)"
        ] {
            XCTAssertThrowsError(
                try JSONEncoder().encode(
                    ToriiGovernanceFinalizeRequest(
                        referendumId: proposalId,
                        proposalId: invalidProposal
                    )
                )
            )
        }
        XCTAssertThrowsError(
            try JSONEncoder().encode(
                ToriiGovernanceFinalizeRequest(
                    referendumId: proposalId,
                    proposalId: String(repeating: "7", count: 64)
                )
            )
        ) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertTrue(reason.contains("referendum_id must equal proposal_id"))
        }
    }

    func testEnactGovernanceEncodesOnlyExactLowercaseProposalId() throws {
        let proposalId = String(repeating: "7", count: 64)
        let data = try JSONEncoder().encode(ToriiGovernanceEnactRequest(proposalId: proposalId))
        let json = try XCTUnwrap(
            JSONSerialization.jsonObject(with: data) as? [String: Any]
        )
        XCTAssertEqual(json["proposal_id"] as? String, proposalId)
        XCTAssertEqual(Set(json.keys), Set(["proposal_id"]))

        for invalid in ["0x\(proposalId)", String(repeating: "A", count: 64), " \(proposalId)"] {
            XCTAssertThrowsError(
                try JSONEncoder().encode(ToriiGovernanceEnactRequest(proposalId: invalid))
            ) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("unexpected error: \(error)")
                }
                XCTAssertTrue(reason.contains("exactly 64 lowercase hexadecimal"))
            }
        }
    }

    func testGetGovernanceProposalDecodesRecord() {
        let expectation = expectation(description: "proposal lookup")
        let proposalId = String(repeating: "6", count: 64)
        let codeHash = String(repeating: "7", count: 64)
        let abiHash = String(repeating: "8", count: 64)
        let contractAddress = "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh"
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/gov/proposals/\(proposalId)")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let payload = """
            {"found":true,"proposal":{"proposer":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV","kind":{"DeployContract":{"contract_address":"\(contractAddress)","code_hash_hex":"\(codeHash)","abi_hash_hex":"\(abiHash)","abi_version":"1"}},"created_height":42,"status":"Approved"}}
            """.data(using: .utf8)!
            return (response, payload)
        }

        makeClient().getGovernanceProposal(idHex: proposalId, canonicalAuth: canonicalReadAuth) { result in
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

    @available(iOS 15.0, macOS 12.0, *)
    func testGovernanceGetIdentifiersUseCanonicalUnreservedPathSegments() async throws {
        let proposalId = String(repeating: "ab", count: 32)
        var requestedURLs: [String] = []
        StubURLProtocol.handler = { request in
            let url = try XCTUnwrap(request.url)
            requestedURLs.append(url.absoluteString)
            let body: Data
            switch url.absoluteString {
            case "https://example.test/v1/gov/proposals/\(proposalId)":
                body = Data(#"{"found":false,"proposal":null}"#.utf8)
            case "https://example.test/v1/gov/referenda/ref.referendum~1":
                body = Data(#"{"found":false,"referendum":null}"#.utf8)
            case "https://example.test/v1/gov/tally/ref_tally-2":
                body = Data(
                    #"{"referendum_id":"ref_tally-2","approve":"0","reject":"0","abstain":"0"}"#.utf8
                )
            case "https://example.test/v1/gov/locks/Ref3":
                body = Data(#"{"found":false,"referendum_id":"Ref3","locks":null}"#.utf8)
            default:
                XCTFail("unexpected governance GET URL: \(url.absoluteString)")
                body = Data()
            }
            let response = HTTPURLResponse(
                url: url,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (response, body)
        }

        let client = makeClient()
        let proposal = try await client.getGovernanceProposal(idHex: proposalId, canonicalAuth: canonicalReadAuth)
        let referendum = try await client.getGovernanceReferendum(id: "ref.referendum~1", canonicalAuth: canonicalReadAuth)
        let tally = try await client.getGovernanceTally(id: "ref_tally-2", canonicalAuth: canonicalReadAuth)
        let locks = try await client.getGovernanceLocks(referendumId: "Ref3", canonicalAuth: canonicalReadAuth)

        XCTAssertFalse(proposal.found)
        XCTAssertFalse(referendum.found)
        XCTAssertEqual(tally.referendumId, "ref_tally-2")
        XCTAssertEqual(locks.referendumId, "Ref3")
        XCTAssertEqual(requestedURLs, [
            "https://example.test/v1/gov/proposals/\(proposalId)",
            "https://example.test/v1/gov/referenda/ref.referendum~1",
            "https://example.test/v1/gov/tally/ref_tally-2",
            "https://example.test/v1/gov/locks/Ref3",
        ])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGovernanceGetIdentifiersRejectAliasesBeforeTransportDispatch() async throws {
        let client = makeClient()
        let proposalId = String(repeating: "ab", count: 32)
        let invalidCalls: [(String, () async throws -> Void)] = [
            ("uppercase proposal", {
                _ = try await client.getGovernanceProposal(idHex: proposalId.uppercased(), canonicalAuth: canonicalReadAuth)
            }),
            ("prefixed proposal", {
                _ = try await client.getGovernanceProposal(idHex: "0x\(proposalId)", canonicalAuth: canonicalReadAuth)
            }),
            ("padded proposal", {
                _ = try await client.getGovernanceProposal(idHex: " \(proposalId)", canonicalAuth: canonicalReadAuth)
            }),
            ("slash proposal", {
                _ = try await client.getGovernanceProposal(idHex: "proposal/segment", canonicalAuth: canonicalReadAuth)
            }),
            ("padded referendum", {
                _ = try await client.getGovernanceReferendum(id: " ref-1", canonicalAuth: canonicalReadAuth)
            }),
            ("internal referendum whitespace", {
                _ = try await client.getGovernanceReferendum(id: "ref 1", canonicalAuth: canonicalReadAuth)
            }),
            ("slash referendum", {
                _ = try await client.getGovernanceReferendum(id: "ref/1", canonicalAuth: canonicalReadAuth)
            }),
            ("leading-dot referendum", {
                _ = try await client.getGovernanceReferendum(id: ".hidden", canonicalAuth: canonicalReadAuth)
            }),
            ("percent referendum", {
                _ = try await client.getGovernanceReferendum(id: "ref%31", canonicalAuth: canonicalReadAuth)
            }),
            ("unicode referendum", {
                _ = try await client.getGovernanceReferendum(id: "投票", canonicalAuth: canonicalReadAuth)
            }),
            ("tally tab", {
                _ = try await client.getGovernanceTally(id: "ref\t1", canonicalAuth: canonicalReadAuth)
            }),
            ("overlong tally", {
                _ = try await client.getGovernanceTally(
                    id: String(repeating: "a", count: 129), canonicalAuth: canonicalReadAuth
                )
            }),
            ("locks control", {
                _ = try await client.getGovernanceLocks(referendumId: "ref\u{0000}1", canonicalAuth: canonicalReadAuth)
            }),
            ("locks unicode whitespace", {
                _ = try await client.getGovernanceLocks(referendumId: "ref\u{2003}1", canonicalAuth: canonicalReadAuth)
            }),
        ]
        var dispatched = false
        StubURLProtocol.handler = { request in
            dispatched = true
            XCTFail("invalid governance identifier reached transport dispatch: \(request)")
            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 500,
                    httpVersion: nil,
                    headerFields: nil
                )!,
                Data()
            )
        }

        for (label, invoke) in invalidCalls {
            await XCTAssertThrowsErrorAsync(try await invoke()) { error in
                guard case ToriiClientError.invalidPayload = error else {
                    return XCTFail("expected invalidPayload for \(label), got \(error)")
                }
            }
            XCTAssertFalse(dispatched, "\(label) reached transport dispatch")
        }
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

        makeClient().getGovernanceUnlockStats(height: 120, referendumId: "ref-1", canonicalAuth: canonicalReadAuth) { result in
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
            XCTAssertEqual(components?.queryItems?.first(where: { $0.name == "hash" })?.value, Self.pipelineHash)
            XCTAssertEqual(components?.queryItems?.first(where: { $0.name == "scope" })?.value, "auto")
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: ["Content-Type": "application/json"])!
            let body = """
            {"hash":"\(Self.pipelineHash)","status":{"kind":"Rejected","block_height":12},"scope":"global","resolved_from":"state"}
            """.data(using: .utf8)!
            return (response, body)
        }

        makeClient().getTransactionStatus(hashHex: Self.pipelineHash) { result in
            switch result {
            case .success(let status):
                XCTAssertEqual(status?.hash, Self.pipelineHash)
                XCTAssertEqual(status?.status.kind, "Rejected")
                XCTAssertEqual(status?.status.blockHeight, 12)
                XCTAssertEqual(status?.scope, "global")
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

        makeClient().getTransactionStatus(hashHex: Self.pipelineHash) { result in
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
            _ = try await makeClient().getTransactionStatus(hashHex: Self.pipelineHash)
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
            {
              "code":"body_fallback_must_not_override_header",
              "message":"missing build claim for transaction status"
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        do {
            _ = try await makeClient().getTransactionStatus(hashHex: Self.pipelineHash)
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
    func testGetTransactionStatusHttpErrorDoesNotTrustTopLevelEnvelopeCode() async throws {
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 503,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            let body = """
            {
              "code": "offline_topup_finality_proof_unavailable",
              "message": "The finalized proof is not available yet.",
              "details": {"retry_after_ms": 250}
            }
            """.data(using: .utf8)!
            return (response, body)
        }

        do {
            _ = try await makeClient().getTransactionStatus(hashHex: Self.pipelineHash)
            XCTFail("expected status failure")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, message, rejectCode) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(code, 503)
            XCTAssertNil(rejectCode)
            XCTAssertEqual(message, "The finalized proof is not available yet.")
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
            _ = try await makeClient().getTransactionStatus(hashHex: Self.pipelineHash)
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
    func testGetTransactionStatusHttpErrorDoesNotTrustEnvelopeDetailsRejectCode() async throws {
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
            _ = try await makeClient().getTransactionStatus(hashHex: Self.pipelineHash)
            XCTFail("expected status failure")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, message, rejectCode) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(code, 429)
            XCTAssertNil(rejectCode)
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
            _ = try await makeClient().getTransactionStatus(hashHex: Self.pipelineHash)
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
            _ = try await makeClient().getTransactionStatus(hashHex: Self.pipelineHash)
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
            _ = try await makeClient().getTransactionStatus(hashHex: Self.pipelineHash)
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
            _ = try await makeClient().getTransactionStatus(hashHex: Self.pipelineHash)
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
                _ = try await makeClient().getTransactionStatus(hashHex: Self.pipelineHash)
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
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiPipelineTransactionStatus.self, from: nestedJSON)
        )

        let committedJSON = """
        {"hash":"\(Self.pipelineHash)","status":{"kind":"Committed"},"scope":"global","resolved_from":"cache"}
        """.data(using: .utf8)!
        let committed = try JSONDecoder().decode(
            ToriiPipelineTransactionStatus.self,
            from: committedJSON
        )
        XCTAssertEqual(committed.status.state, .committed)
        XCTAssertFalse(committed.status.state.isKnownTerminalSuccess)
        XCTAssertFalse(committed.isTerminal)
        XCTAssertFalse(PipelineTransactionState.approved.isKnownTerminalSuccess)

        let flatJSON = """
        {"hash":"\(Self.pipelineHash)","resolved_from":"state","scope":"global","status":{"block_height":64,"kind":"Applied"}}
        """.data(using: .utf8)!
        let flat = try JSONDecoder().decode(ToriiPipelineTransactionStatus.self, from: flatJSON)
        XCTAssertEqual(flat.hash, Self.pipelineHash)
        XCTAssertEqual(flat.status.state, .applied)
        XCTAssertTrue(flat.status.state.isTerminalSuccess)
        XCTAssertTrue(flat.isApplied)

        let retiredDetailJSON = """
        {"hash":"\(Self.pipelineHash)","status":{"kind":"Rejected","rejection_reason":"secret"},"scope":"global","resolved_from":"state","summary":"secret","diagnostics":[]}
        """.data(using: .utf8)!
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiPipelineTransactionStatus.self, from: retiredDetailJSON)
        )

        let unknownKindJSON = """
        {"hash":"\(Self.pipelineHash)","status":{"kind":"Finalizing"},"scope":"global","resolved_from":"state"}
        """.data(using: .utf8)!
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiPipelineTransactionStatus.self, from: unknownKindJSON)
        )

        let nullHeightJSON = """
        {"hash":"\(Self.pipelineHash)","status":{"kind":"Applied","block_height":null},"scope":"global","resolved_from":"state"}
        """.data(using: .utf8)!
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiPipelineTransactionStatus.self, from: nullHeightJSON)
        )

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

    @available(iOS 15.0, macOS 12.0, *)
    func testRegisterSoraFsPinManifestPostsOnlySignedNoritoAndReturnsAdmission() async throws {
        let signedBytes = Data([0x01, 0x02, 0x03])
        let envelope = SignedTransactionEnvelope(
            norito: signedBytes,
            signedTransaction: signedBytes,
            payload: nil,
            transactionHash: Data(repeating: 0xAA, count: 32)
        )
        let txHash = String(repeating: "a", count: 64)
        let manifestDigest = String(repeating: "b", count: 64)
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/sorafs/pin/register")
            XCTAssertEqual(
                request.value(forHTTPHeaderField: "Content-Type"),
                "application/x-norito"
            )
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            XCTAssertEqual(self.bodyData(from: request), signedBytes)
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 202,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            let body = Data(
                #"{"status":"submitted","tx_hash_hex":"\#(txHash)","manifest_digest_hex":"\#(manifestDigest)"}"#.utf8
            )
            return (response, body)
        }

        let admission = try await makeClient().registerSoraFsPinManifest(envelope)

        XCTAssertEqual(admission.status, "submitted")
        XCTAssertEqual(admission.txHashHex, txHash)
        XCTAssertEqual(admission.manifestDigestHex, manifestDigest)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRegisterSoraFsPinManifestRejectsPreFinalityFeeClaims() async throws {
        let signedBytes = Data([0x01])
        let envelope = SignedTransactionEnvelope(
            norito: signedBytes,
            signedTransaction: signedBytes,
            payload: nil,
            transactionHash: Data(repeating: 0xAA, count: 32)
        )
        StubURLProtocol.handler = { request in
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 202,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            let body = Data(
                #"{"status":"submitted","tx_hash_hex":"\#(String(repeating: "a", count: 64))","manifest_digest_hex":"\#(String(repeating: "b", count: 64))","pin_fee":"1"}"#.utf8
            )
            return (response, body)
        }

        await XCTAssertThrowsErrorAsync(
            try await makeClient().registerSoraFsPinManifest(envelope)
        ) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("expected invalidPayload, got \(error)")
            }
        }
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
