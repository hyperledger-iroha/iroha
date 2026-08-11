import Foundation
import XCTest
@testable import IrohaSwift

private final class AssetTransferStubURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data?))?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(
                self,
                didFailWithError: NSError(domain: "AssetTransferStub", code: -1)
            )
            return
        }
        do {
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            if let data {
                client?.urlProtocol(self, didLoad: data)
            }
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}

private final class AssetTransferMonotonicClock: @unchecked Sendable {
    private let lock = NSLock()
    private var values: [UInt64]

    init(_ values: [UInt64]) {
        precondition(!values.isEmpty)
        self.values = values
    }

    func now() -> UInt64 {
        lock.lock()
        defer { lock.unlock() }
        guard values.count > 1 else { return values[0] }
        return values.removeFirst()
    }
}

final class ToriiAssetTransferTests: XCTestCase {
    private static let authorityKey = try! Keypair(privateKeyBytes: Data(repeating: 0x41, count: 32))
    private static let destinationKey = try! Keypair(privateKeyBytes: Data(repeating: 0x42, count: 32))
    private static let sponsorKey = try! Keypair(privateKeyBytes: Data(repeating: 0x43, count: 32))

    private static let authority = try! authorityKey.accountId()
    private static let destination = try! destinationKey.accountId()
    private static let sponsor = try! sponsorKey.accountId()
    private static let sponsorProgram = try! FeeSponsorProgramId(
        sponsor: sponsor,
        name: "wallet_fx"
    )
    private static let feePayment = FeePaymentIntent.sponsor(
        programId: sponsorProgram,
        programRevision: 1,
        chargeLimits: [],
        gasLimit: nil
    )
    private static let assetDefinitionId: String = {
        var bytes = Data(repeating: 0, count: 16)
        bytes[0] = 0x10
        bytes[6] = 0x40
        bytes[8] = 0x80
        bytes[15] = 0x7F
        return AssetDefinitionAddressCodec.definitionLiteral(uuidBytes: bytes)!
    }()
    private static let otherAssetDefinitionId: String = {
        var bytes = Data(repeating: 0, count: 16)
        bytes[0] = 0x20
        bytes[6] = 0x40
        bytes[8] = 0x80
        bytes[15] = 0x7E
        return AssetDefinitionAddressCodec.definitionLiteral(uuidBytes: bytes)!
    }()

    private func request(
        authority: String = ToriiAssetTransferTests.authority,
        assetDefinitionId: String = ToriiAssetTransferTests.assetDefinitionId,
        scope: String = "dataspace:10",
        amount: String = "1.25",
        destination: String = ToriiAssetTransferTests.destination,
        memo: String? = "invoice 42",
        feePayment: FeePaymentIntent = ToriiAssetTransferTests.feePayment,
        creationTimeMs: UInt64 = 1_700_000_000_000,
        transactionTtlMs: UInt64 = 120_000,
        publicKeyHex: String? = nil,
        signatureBase64: String? = nil
    ) -> ToriiAssetTransferRequest {
        ToriiAssetTransferRequest(
            authority: authority,
            assetDefinitionId: assetDefinitionId,
            assetBalanceScope: scope,
            amount: amount,
            destination: destination,
            memo: memo,
            feePayment: feePayment,
            creationTimeMs: creationTimeMs,
            transactionTtlMs: transactionTtlMs,
            publicKeyHex: publicKeyHex,
            signatureBase64: signatureBase64
        )
    }

    override func tearDown() {
        AssetTransferStubURLProtocol.handler = nil
        super.tearDown()
    }

    private func requestBody(_ request: URLRequest) throws -> Data {
        if let body = request.httpBody {
            return body
        }
        guard let stream = request.httpBodyStream else {
            throw NSError(domain: "ToriiAssetTransferTests", code: 1)
        }
        stream.open()
        defer { stream.close() }
        var body = Data()
        var buffer = [UInt8](repeating: 0, count: 4_096)
        while stream.hasBytesAvailable {
            let count = stream.read(&buffer, maxLength: buffer.count)
            guard count >= 0 else {
                throw stream.streamError ?? NSError(
                    domain: "ToriiAssetTransferTests",
                    code: 2
                )
            }
            guard count > 0 else { break }
            body.append(buffer, count: count)
        }
        return body
    }

    func testRequestUsesOnlyExactSharpWireFields() throws {
        let encoded = try JSONEncoder().encode(request())
        let object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: encoded) as? [String: Any]
        )
        XCTAssertEqual(
            Set(object.keys),
            Set([
                "authority", "asset_definition_id", "asset_balance_scope", "amount",
                "destination", "memo", "fee_payment", "creation_time_ms",
                "transaction_ttl_ms",
            ])
        )
        XCTAssertEqual(object["asset_balance_scope"] as? String, "dataspace:10")
        let feePayment = try XCTUnwrap(object["fee_payment"] as? [String: Any])
        XCTAssertEqual(feePayment["payer"] as? String, "sponsor")
        let feePaymentValue = try XCTUnwrap(feePayment["value"] as? [String: Any])
        XCTAssertEqual(feePaymentValue["program_revision"] as? UInt64, 1)
        XCTAssertNil(object["private_key"])
        XCTAssertNil(object["nonce"])
        XCTAssertNil(object["metadata"])
        XCTAssertNil(object["signature_b64"])

        let publicKeyHex = Self.authorityKey.publicKey.map { String(format: "%02x", $0) }.joined()
        let signatureBase64 = Data(repeating: 0x55, count: 64).base64EncodedString()
        let submitObject = try XCTUnwrap(
            JSONSerialization.jsonObject(
                with: try JSONEncoder().encode(
                    request(
                        publicKeyHex: publicKeyHex,
                        signatureBase64: signatureBase64
                    )
                )
            ) as? [String: Any]
        )
        XCTAssertEqual(submitObject["public_key_hex"] as? String, publicKeyHex)
        XCTAssertEqual(submitObject["signature_base64"] as? String, signatureBase64)
        XCTAssertNil(submitObject["signature_b64"])
    }

    func testRequestRejectsNoncanonicalScopesAmountsAndMemos() {
        for scope in [
            "Global", " global", "global ", "dataspace:", "dataspace:01",
            "dataspace:+1", "dataspace:18446744073709551616",
        ] {
            XCTAssertThrowsError(try JSONEncoder().encode(request(scope: scope)), scope)
        }
        for amount in [
            "", "0", "-1", "+1", "01", "1.0", "1.230", "1e0", " 1", "1 ",
            "0.00000000000000000000000000001", String(repeating: "9", count: 200),
        ] {
            XCTAssertThrowsError(try JSONEncoder().encode(request(amount: amount)), amount)
        }
        for memo in [String(), "line\nbreak", String(repeating: "x", count: 257)] {
            XCTAssertThrowsError(try JSONEncoder().encode(request(memo: memo)))
        }
    }

    func testRequestRejectsInvalidTimesAndHalfSigningStates() {
        XCTAssertThrowsError(
            try JSONEncoder().encode(request(creationTimeMs: 0))
        )
        XCTAssertThrowsError(
            try JSONEncoder().encode(request(transactionTtlMs: 0))
        )
        XCTAssertThrowsError(
            try JSONEncoder().encode(
                request(
                    transactionTtlMs: ToriiAssetTransferRequest.maximumTransactionTtlMs + 1
                )
            )
        )
        let publicKeyHex = Self.authorityKey.publicKey.map { String(format: "%02x", $0) }.joined()
        XCTAssertThrowsError(
            try JSONEncoder().encode(request(publicKeyHex: publicKeyHex))
        )
        XCTAssertThrowsError(
            try JSONEncoder().encode(
                request(signatureBase64: Data(repeating: 0x55, count: 64).base64EncodedString())
            )
        )
        XCTAssertThrowsError(
            try JSONEncoder().encode(
                request(
                    publicKeyHex: publicKeyHex.uppercased(),
                    signatureBase64: Data(repeating: 0x55, count: 64).base64EncodedString()
                )
            )
        )
        XCTAssertThrowsError(
            try JSONEncoder().encode(
                request(publicKeyHex: publicKeyHex, signatureBase64: "not-base64")
            )
        )
    }

    func testRequestRejectsMalformedIdentifiersKeysAndSignatures() {
        for authority in [
            "", "alice@wonderland", " \(Self.authority)", Self.authority + " ",
            String(repeating: "a", count: 513),
        ] {
            XCTAssertThrowsError(try JSONEncoder().encode(request(authority: authority)), authority)
        }
        for destination in ["", "bob@wonderland", Self.destination + "\n"] {
            XCTAssertThrowsError(
                try JSONEncoder().encode(request(destination: destination)),
                destination
            )
        }
        for definition in [
            "", "0x\(Self.assetDefinitionId)", Self.assetDefinitionId + " ",
            String(repeating: "1", count: 65),
        ] {
            XCTAssertThrowsError(
                try JSONEncoder().encode(request(assetDefinitionId: definition)),
                definition
            )
        }

        let publicKeyHex = Self.authorityKey.publicKey.map { String(format: "%02x", $0) }.joined()
        let signature = Data(repeating: 0x55, count: 64).base64EncodedString()
        for malformedKey in [
            String(repeating: "0", count: 64),
            String(repeating: "A", count: 64),
            "0x" + String(publicKeyHex.dropLast(2)),
            String(publicKeyHex.dropLast()),
            String(publicKeyHex.dropLast()) + "g",
        ] {
            XCTAssertThrowsError(
                try JSONEncoder().encode(
                    request(publicKeyHex: malformedKey, signatureBase64: signature)
                ),
                malformedKey
            )
        }
        for malformedSignature in [
            Data(repeating: 0, count: 64).base64EncodedString(),
            Data(repeating: 1, count: 63).base64EncodedString(),
            Data(repeating: 1, count: 65).base64EncodedString(),
            String(repeating: "!", count: 88),
            String(signature.dropLast()),
            signature + "\n",
            Data(repeating: 0xfb, count: 64).base64EncodedString()
                .replacingOccurrences(of: "+", with: "-")
                .replacingOccurrences(of: "/", with: "_"),
        ] {
            XCTAssertThrowsError(
                try JSONEncoder().encode(
                    request(publicKeyHex: publicKeyHex, signatureBase64: malformedSignature)
                ),
                malformedSignature
            )
        }
        XCTAssertThrowsError(
            try JSONEncoder().encode(
                request(creationTimeMs: UInt64.max, transactionTtlMs: 1)
            )
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testPreparationRejectsStaleFutureAndExpiredRequestsBeforeNetwork() async {
        var calls = 0
        AssetTransferStubURLProtocol.handler = { request in
            calls += 1
            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 500,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Data()
            )
        }
        let now = 1_700_000_000_000 as UInt64
        let invalidRequests = [
            request(creationTimeMs: now - ToriiAssetTransferRequest.maximumCreationAgeMs - 1),
            request(creationTimeMs: now + ToriiAssetTransferRequest.maximumFutureSkewMs + 1),
            request(creationTimeMs: now - 120_000, transactionTtlMs: 120_000),
        ]
        for invalid in invalidRequests {
            do {
                _ = try await makeClient(now: now).prepareDetachedAssetTransfer(invalid)
                XCTFail("invalid transfer time reached the network")
            } catch {}
        }
        XCTAssertEqual(calls, 0)
    }

    func testResponseModelsRejectUnknownAndNoncanonicalFields() throws {
        let valid = responseObject()
        let decoded = try JSONDecoder().decode(
            ToriiAssetTransferResponse.self,
            from: try JSONSerialization.data(withJSONObject: valid)
        )
        XCTAssertFalse(decoded.submitted)
        XCTAssertEqual(decoded.intent.assetBalanceScope, "dataspace:10")
        XCTAssertEqual(decoded.transactionPayloadB64, canonicalAssetPayload().base64EncodedString())
        XCTAssertEqual(decoded.signingMessageB64, hashBytes().base64EncodedString())
        XCTAssertNil(decoded.transactionHashHex)

        var topLevelUnknown = valid
        topLevelUnknown["signed_transaction_base64"] = "AQ=="
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiAssetTransferResponse.self,
                from: try JSONSerialization.data(withJSONObject: topLevelUnknown)
            )
        )

        var intentUnknown = valid
        var intent = intentUnknown["intent"] as! [String: Any]
        intent["asset_id"] = Self.assetDefinitionId
        intentUnknown["intent"] = intent
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiAssetTransferResponse.self,
                from: try JSONSerialization.data(withJSONObject: intentUnknown)
            )
        )

        var legacySignature = valid
        legacySignature["signing_payload"] = [
            "payload_base64": hashBytes().base64EncodedString(),
            "algorithm": "ed25519",
        ]
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiAssetTransferResponse.self,
                from: try JSONSerialization.data(withJSONObject: legacySignature)
            )
        )

        var uppercaseHash = valid
        var uppercaseReceipt = uppercaseHash["receipt"] as! [String: Any]
        uppercaseReceipt["payload_signing_hash_hex"] = hashHex(hashBytes()).uppercased()
        uppercaseHash["receipt"] = uppercaseReceipt
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiAssetTransferResponse.self,
                from: try JSONSerialization.data(withJSONObject: uppercaseHash)
            )
        )

        var retiredScaffold = valid
        retiredScaffold["transaction_scaffold_base64"] = "AQ=="
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiAssetTransferResponse.self,
                from: try JSONSerialization.data(withJSONObject: retiredScaffold)
            )
        )
    }

    func testPreparedResponseRejectsEveryPhaseReceiptAndHashSubstitution() throws {
        let mutations: [(inout [String: Any]) -> Void] = [
            { $0["ok"] = false },
            { $0["submitted"] = true },
            { $0.removeValue(forKey: "signing_message_b64") },
            { $0.removeValue(forKey: "transaction_payload_b64") },
            { $0["transaction_hash_hex"] = self.hashHex(0x33) },
            { $0["entrypoint_hash_hex"] = self.hashHex(0x33) },
            {
                $0["signing_message_b64"] = Data(repeating: 0x12, count: 32)
                    .base64EncodedString()
            },
            {
                var receipt = $0["receipt"] as! [String: Any]
                receipt["operation_kind"] = "contract_call"
                $0["receipt"] = receipt
            },
            {
                var receipt = $0["receipt"] as! [String: Any]
                receipt["status"] = "submitted"
                $0["receipt"] = receipt
            },
            {
                var receipt = $0["receipt"] as! [String: Any]
                receipt["transport"] = "legacy"
                $0["receipt"] = receipt
            },
            {
                var receipt = $0["receipt"] as! [String: Any]
                receipt["payload_signing_hash_hex"] = self.hashHex(0x12)
                $0["receipt"] = receipt
            },
            {
                var receipt = $0["receipt"] as! [String: Any]
                var intent = receipt["intent"] as! [String: Any]
                intent["amount"] = "2"
                receipt["intent"] = intent
                $0["receipt"] = receipt
            },
            {
                var intent = $0["intent"] as! [String: Any]
                intent["authority"] = Self.destination
                $0["intent"] = intent
            },
        ]
        for (index, mutation) in mutations.enumerated() {
            var candidate = responseObject()
            mutation(&candidate)
            XCTAssertThrowsError(
                try decodeResponse(candidate),
                "prepared response mutation \(index) must fail"
            )
        }
    }

    func testSubmittedResponseRequiresCanonicalQueuedLocalPhase() throws {
        let valid = submittedResponseObject()
        let decoded = try decodeResponse(valid)
        XCTAssertTrue(decoded.submitted)
        XCTAssertEqual(decoded.pipelineStatus?.state, .queued)

        let mutations: [(inout [String: Any]) -> Void] = [
            { $0["ok"] = false },
            { $0["submitted"] = false },
            { $0["signing_payload"] = [
                "payload_base64": self.hashBytes().base64EncodedString(),
                "algorithm": "ed25519",
            ] },
            { $0["transaction_scaffold_base64"] = "AQ==" },
            { $0["placeholder_transaction_hash_hex"] = self.hashHex(0x22) },
            { $0.removeValue(forKey: "transaction_hash_hex") },
            { $0.removeValue(forKey: "entrypoint_hash_hex") },
            { $0["entrypoint_hash_hex"] = self.hashHex(0x34) },
            {
                $0["transaction_hash_hex"] = String(repeating: "0", count: 64)
                $0["entrypoint_hash_hex"] = String(repeating: "0", count: 64)
            },
            { $0.removeValue(forKey: "pipeline_status") },
            {
                var pipeline = $0["pipeline_status"] as! [String: Any]
                pipeline["hash"] = self.hashHex(0x34)
                $0["pipeline_status"] = pipeline
            },
            {
                var pipeline = $0["pipeline_status"] as! [String: Any]
                pipeline["status"] = ["kind": "Applied", "block_height": 1]
                $0["pipeline_status"] = pipeline
            },
            {
                var pipeline = $0["pipeline_status"] as! [String: Any]
                pipeline["scope"] = "global"
                $0["pipeline_status"] = pipeline
            },
            {
                var pipeline = $0["pipeline_status"] as! [String: Any]
                pipeline["resolved_from"] = "state"
                $0["pipeline_status"] = pipeline
            },
            {
                var pipeline = $0["pipeline_status"] as! [String: Any]
                pipeline["status"] = ["kind": "Queued", "block_height": 1]
                $0["pipeline_status"] = pipeline
            },
            {
                var pipeline = $0["pipeline_status"] as! [String: Any]
                pipeline["status"] = ["kind": "Queued", "rejection_reason": "hostile"]
                $0["pipeline_status"] = pipeline
            },
            {
                var receipt = $0["receipt"] as! [String: Any]
                receipt["status"] = "pending_signature"
                $0["receipt"] = receipt
            },
            {
                var receipt = $0["receipt"] as! [String: Any]
                receipt["transaction_hash_hex"] = self.hashHex(0x34)
                $0["receipt"] = receipt
            },
            {
                var receipt = $0["receipt"] as! [String: Any]
                receipt["placeholder_transaction_hash_hex"] = self.hashHex(0x22)
                $0["receipt"] = receipt
            },
        ]
        for (index, mutation) in mutations.enumerated() {
            var candidate = valid
            mutation(&candidate)
            XCTAssertThrowsError(
                try decodeResponse(candidate),
                "submitted response mutation \(index) must fail"
            )
        }
    }

    func testSubmittedResponseAcceptsCanonicalAppliedExactReplayPhase() throws {
        let applied = try decodeResponse(appliedReplayResponseObject())
        XCTAssertTrue(applied.submitted)
        XCTAssertEqual(applied.pipelineStatus?.state, .applied)
        XCTAssertEqual(applied.pipelineStatus?.scope, "local")
        XCTAssertEqual(applied.pipelineStatus?.resolvedFrom, "state")
        XCTAssertEqual(applied.receipt.status, "applied")

        let prepared = try decodeResponse(responseObject())
        XCTAssertNoThrow(
            try ToriiAssetTransferDraft.validateSubmittedBindings(
                applied,
                request: request(),
                preparedIntent: prepared.intent,
                signingMessage: hashBytes(),
                payloadSigningHashHex: hashHex(hashBytes()),
                finalization: DetachedTransactionFinalization(
                    payloadSigningHash: hashBytes(),
                    transactionHash: Data(repeating: 0x33, count: 32),
                    entrypointHash: Data(repeating: 0x33, count: 32)
                )
            )
        )

        var wrongReceipt = appliedReplayResponseObject()
        var receipt = wrongReceipt["receipt"] as! [String: Any]
        receipt["status"] = "submitted"
        wrongReceipt["receipt"] = receipt
        XCTAssertThrowsError(try decodeResponse(wrongReceipt))

        var wrongResolution = appliedReplayResponseObject()
        var pipeline = wrongResolution["pipeline_status"] as! [String: Any]
        pipeline["resolved_from"] = "queue"
        wrongResolution["pipeline_status"] = pipeline
        XCTAssertThrowsError(try decodeResponse(wrongResolution))

        var zeroHeight = appliedReplayResponseObject()
        pipeline = zeroHeight["pipeline_status"] as! [String: Any]
        pipeline["status"] = ["kind": "Applied", "block_height": 0]
        zeroHeight["pipeline_status"] = pipeline
        XCTAssertThrowsError(try decodeResponse(zeroHeight))
    }

    func testCanonicalPayloadValidatorRejectsEverySignedFieldSubstitution() throws {
        let exactBytes = canonicalAssetPayload()
        let exact = try ToriiCanonicalTransactionDraft.decode(
            transactionPayloadB64: exactBytes.base64EncodedString(),
            signingMessageB64: IrohaHash.hash(exactBytes).base64EncodedString(),
            context: "asset transfer test"
        )
        XCTAssertNoThrow(
            try ToriiAssetTransferDraft.validateTransactionPayloadBindings(
                exact.payload,
                request: request(),
                expectedNetworkId: TestNetworkIds.canonical
            )
        )

        let hostile: [(ToriiAssetTransferRequest, String)] = [
            (request(authority: Self.destination), "authority"),
            (request(creationTimeMs: 1_700_000_000_001), "creation time"),
            (request(transactionTtlMs: 120_001), "ttl"),
            (request(assetDefinitionId: Self.otherAssetDefinitionId), "asset"),
            (request(scope: "global"), "scope"),
            (request(destination: Self.authority), "destination"),
            (request(amount: "2"), "amount"),
            (request(memo: "changed"), "metadata"),
            (request(feePayment: .authority(chargeLimits: [], gasLimit: nil)), "fee"),
        ]
        for (candidateRequest, label) in hostile {
            let candidateBytes = canonicalAssetPayload(candidateRequest)
            let candidate = try ToriiCanonicalTransactionDraft.decode(
                transactionPayloadB64: candidateBytes.base64EncodedString(),
                signingMessageB64: IrohaHash.hash(candidateBytes).base64EncodedString(),
                context: "hostile asset transfer test"
            )
            XCTAssertThrowsError(
                try ToriiAssetTransferDraft.validateTransactionPayloadBindings(
                    candidate.payload,
                    request: request(),
                    expectedNetworkId: TestNetworkIds.canonical
                ),
                "\(label) substitution must fail"
            )
        }
        let wrongNetworkBytes = canonicalAssetPayload(networkId: TestNetworkIds.other)
        let wrongNetwork = try ToriiCanonicalTransactionDraft.decode(
            transactionPayloadB64: wrongNetworkBytes.base64EncodedString(),
            signingMessageB64: IrohaHash.hash(wrongNetworkBytes).base64EncodedString(),
            context: "wrong-network asset transfer test"
        )
        XCTAssertThrowsError(
            try ToriiAssetTransferDraft.validateTransactionPayloadBindings(
                wrongNetwork.payload,
                request: request(),
                expectedNetworkId: TestNetworkIds.canonical
            )
        )

        var trailing = exactBytes
        trailing.append(0)
        XCTAssertThrowsError(
            try ToriiCanonicalTransactionDraft.decode(
                transactionPayloadB64: trailing.base64EncodedString(),
                signingMessageB64: IrohaHash.hash(trailing).base64EncodedString(),
                context: "trailing asset transfer test"
            )
        )
    }

    func testSubmittedAndFinalityBindingsRejectSignatureHashAndStatusSubstitution() throws {
        let prepared = try decodeResponse(responseObject())
        let submitted = try decodeResponse(submittedResponseObject())
        let signingMessage = hashBytes()
        let finalization = DetachedTransactionFinalization(
            payloadSigningHash: hashBytes(),
            transactionHash: Data(repeating: 0x33, count: 32),
            entrypointHash: Data(repeating: 0x33, count: 32)
        )
        XCTAssertNoThrow(
            try ToriiAssetTransferDraft.validateSubmittedBindings(
                submitted,
                request: request(),
                preparedIntent: prepared.intent,
                signingMessage: signingMessage,
                payloadSigningHashHex: hashHex(hashBytes()),
                finalization: finalization
            )
        )

        let hostileFinalizations = [
            DetachedTransactionFinalization(
                payloadSigningHash: Data(repeating: 0x12, count: 32),
                transactionHash: Data(repeating: 0x33, count: 32),
                entrypointHash: Data(repeating: 0x33, count: 32)
            ),
            DetachedTransactionFinalization(
                payloadSigningHash: hashBytes(),
                transactionHash: Data(repeating: 0x34, count: 32),
                entrypointHash: Data(repeating: 0x33, count: 32)
            ),
            DetachedTransactionFinalization(
                payloadSigningHash: hashBytes(),
                transactionHash: Data(repeating: 0x33, count: 32),
                entrypointHash: Data(repeating: 0x34, count: 32)
            ),
        ]
        for finalization in hostileFinalizations {
            XCTAssertThrowsError(
                try ToriiAssetTransferDraft.validateSubmittedBindings(
                    submitted,
                    request: request(),
                    preparedIntent: prepared.intent,
                    signingMessage: signingMessage,
                    payloadSigningHashHex: hashHex(hashBytes()),
                    finalization: finalization
                )
            )
        }
        XCTAssertThrowsError(
            try ToriiAssetTransferDraft.validateSubmittedBindings(
                submitted,
                request: request(),
                preparedIntent: prepared.intent,
                signingMessage: signingMessage,
                payloadSigningHashHex: hashHex(0x12),
                finalization: finalization
            )
        )

        var changedIntentObject = submittedResponseObject()
        var changedIntent = changedIntentObject["intent"] as! [String: Any]
        changedIntent["amount"] = "2"
        changedIntentObject["intent"] = changedIntent
        var changedIntentReceipt = changedIntentObject["receipt"] as! [String: Any]
        changedIntentReceipt["intent"] = changedIntent
        changedIntentObject["receipt"] = changedIntentReceipt
        let changedIntentResponse = try decodeResponse(changedIntentObject)
        XCTAssertThrowsError(
            try ToriiAssetTransferDraft.validateSubmittedBindings(
                changedIntentResponse,
                request: request(),
                preparedIntent: prepared.intent,
                signingMessage: signingMessage,
                payloadSigningHashHex: hashHex(hashBytes()),
                finalization: finalization
            )
        )

        var changedReceiptObject = submittedResponseObject()
        var changedReceipt = changedReceiptObject["receipt"] as! [String: Any]
        changedReceipt["payload_signing_hash_hex"] = hashHex(0x12)
        changedReceiptObject["receipt"] = changedReceipt
        let changedReceiptResponse = try decodeResponse(changedReceiptObject)
        XCTAssertThrowsError(
            try ToriiAssetTransferDraft.validateSubmittedBindings(
                changedReceiptResponse,
                request: request(),
                preparedIntent: prepared.intent,
                signingMessage: signingMessage,
                payloadSigningHashHex: hashHex(hashBytes()),
                finalization: finalization
            )
        )

        let validStatus = try pipelineStatus()
        XCTAssertNoThrow(
            try ToriiAssetTransferDraft.validateAuthoritativeFinality(
                validStatus,
                expectedTransactionHashHex: hashHex(0x33)
            )
        )
        let statusMutations: [(inout [String: Any]) -> Void] = [
            { $0["hash"] = self.hashHex(0x34) },
            { $0["status"] = ["kind": "Queued"] },
            { $0["status"] = ["kind": "Applied", "block_height": 0] },
            { $0["status"] = ["kind": "Applied"] },
            { $0["status"] = [
                "kind": "Applied", "block_height": 44, "rejection_reason": "hostile",
            ] },
            { $0["scope"] = "local" },
            { $0["resolved_from"] = "cache" },
        ]
        for mutation in statusMutations {
            var object = pipelineStatusObject()
            mutation(&object)
            let status = try JSONDecoder().decode(
                ToriiPipelineTransactionStatus.self,
                from: JSONSerialization.data(withJSONObject: object)
            )
            XCTAssertThrowsError(
                try ToriiAssetTransferDraft.validateAuthoritativeFinality(
                    status,
                    expectedTransactionHashHex: hashHex(0x33)
                )
            )
        }
    }

    func testDetachedSignatureValidatorRejectsWrongAuthorityPayloadKeyAndSignature() throws {
        let payload = hashBytes()
        let publicKeyHex = Self.authorityKey.publicKey.map { String(format: "%02x", $0) }.joined()
        let signature = try Self.authorityKey.sign(payload)
        let signatureBase64 = signature.base64EncodedString()
        XCTAssertNoThrow(
            try ToriiAssetTransferDraft.validateDetachedSignature(
                publicKeyHex: publicKeyHex,
                signatureBase64: signatureBase64,
                authority: Self.authority,
                signingPayload: payload
            )
        )
        XCTAssertThrowsError(
            try ToriiAssetTransferDraft.validateDetachedSignature(
                publicKeyHex: publicKeyHex,
                signatureBase64: signatureBase64,
                authority: Self.destination,
                signingPayload: payload
            )
        )
        XCTAssertThrowsError(
            try ToriiAssetTransferDraft.validateDetachedSignature(
                publicKeyHex: publicKeyHex,
                signatureBase64: signatureBase64,
                authority: Self.authority,
                signingPayload: Data(repeating: 0x12, count: 32)
            )
        )
        var tampered = signature
        tampered[17] ^= 0x80
        XCTAssertThrowsError(
            try ToriiAssetTransferDraft.validateDetachedSignature(
                publicKeyHex: publicKeyHex,
                signatureBase64: tampered.base64EncodedString(),
                authority: Self.authority,
                signingPayload: payload
            )
        )
        let wrongKeyHex = Self.destinationKey.publicKey.map {
            String(format: "%02x", $0)
        }.joined()
        XCTAssertThrowsError(
            try ToriiAssetTransferDraft.validateDetachedSignature(
                publicKeyHex: wrongKeyHex,
                signatureBase64: signatureBase64,
                authority: Self.authority,
                signingPayload: payload
            )
        )
        for malformed in [
            String(repeating: "0", count: 64),
            String(repeating: "A", count: 64),
        ] {
            XCTAssertThrowsError(
                try ToriiAssetTransferDraft.validateDetachedSignature(
                    publicKeyHex: malformed,
                    signatureBase64: signatureBase64,
                    authority: Self.authority,
                    signingPayload: payload
                )
            )
        }
        for malformed in [
            Data(repeating: 0, count: 64).base64EncodedString(),
            Data(repeating: 1, count: 63).base64EncodedString(),
            signatureBase64 + "\n",
        ] {
            XCTAssertThrowsError(
                try ToriiAssetTransferDraft.validateDetachedSignature(
                    publicKeyHex: publicKeyHex,
                    signatureBase64: malformed,
                    authority: Self.authority,
                    signingPayload: payload
                )
            )
        }
    }

    func testExactPayloadFinalizationUsesCanonicalSignedTransactionLayout() throws {
        let transactionPayload = canonicalAssetPayload()
        let signingMessage = IrohaHash.hash(transactionPayload)
        let signature = try Self.authorityKey.sign(signingMessage)
        let finalized = try ToriiCanonicalTransactionDraft.finalize(
            transactionPayload: transactionPayload,
            publicKey: Self.authorityKey.publicKey,
            signature: signature
        )

        var signatureBytes = CompactNoritoWriter()
        signatureBytes.writeUInt64LE(UInt64(signature.count))
        for byte in signature {
            signatureBytes.writeField(Data([byte]))
        }
        var signatureSet = CompactNoritoWriter()
        signatureSet.writeField(signatureBytes.data)
        var signed = CompactNoritoWriter()
        signed.writeField(signatureSet.data)
        signed.writeField(transactionPayload)
        signed.writeField(Data([0]))
        var expected = Data([1])
        expected.append(signed.data)

        XCTAssertEqual(finalized.signedTransaction, expected)
        XCTAssertEqual(finalized.finalization.payloadSigningHash, signingMessage)
        XCTAssertEqual(
            finalized.finalization.transactionHash,
            CanonicalUnsignedTransactionTestSupport.transactionHash(for: transactionPayload)
        )
        XCTAssertEqual(
            try ToriiCanonicalTransactionDraft.transactionPayload(
                fromVersionedSignedTransaction: finalized.signedTransaction,
                context: "asset finalization test"
            ),
            transactionPayload
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testNetworkBoundaryRejectsDuplicateKeysAndOversizedResponsesBeforePayloadDecode() async {
        let now = 1_700_000_000_000 as UInt64
        let hostileResponses: [(headers: [String: String], body: Data)] = [
            (
                ["Content-Type": "application/json"],
                Data("{\"ok\":true,\"ok\":false}".utf8)
            ),
            (
                ["Content-Type": "application/json"],
                Data("{\"receipt\":{\"status\":\"a\",\"status\":\"b\"}}".utf8)
            ),
            (
                ["Content-Type": "application/json"],
                Data([0xFF, 0xFE])
            ),
            (
                [
                    "Content-Type": "application/json",
                    "Content-Length": String(32 * 1_024 * 1_024 + 1),
                ],
                Data()
            ),
        ]
        for hostile in hostileResponses {
            var calls = 0
            AssetTransferStubURLProtocol.handler = { request in
                calls += 1
                XCTAssertEqual(request.url?.path, "/v1/assets/transfer")
                XCTAssertEqual(request.httpMethod, "POST")
                return (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 200,
                        httpVersion: nil,
                        headerFields: hostile.headers
                    )!,
                    hostile.body
                )
            }
            do {
                _ = try await makeClient(now: now).prepareDetachedAssetTransfer(request())
                XCTFail("hostile transfer response was accepted")
            } catch {}
            XCTAssertEqual(calls, 1)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testTimeNowBodyCorrectsLargeClockSkewForTransferValidation() async throws {
        let cases: [(local: UInt64, server: UInt64, host: String)] = [
            (1_900_000_000_000, 1_700_000_000_000, "clock-ahead.example"),
            (1_500_000_000_000, 1_700_000_100_000, "clock-behind.example"),
        ]
        for value in cases {
            AssetTransferStubURLProtocol.handler = { request in
                XCTAssertEqual(request.url?.path, "/v1/time/now")
                let body = try JSONSerialization.data(withJSONObject: [
                    "now": value.server,
                    "offset_ms": 0,
                    "confidence_ms": 2,
                ])
                return (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 200,
                        httpVersion: nil,
                        headerFields: ["Content-Type": "application/json"]
                    )!,
                    body
                )
            }
            let client = makeClient(
                baseURL: URL(string: "https://\(value.host)")!,
                now: value.local
            )
            let snapshot = try await client.getTimeNow()
            XCTAssertEqual(snapshot.now, value.server)
            XCTAssertEqual(
                client.recommendedCreationTimeMs(),
                value.server - 10_000,
                "authoritative body time must correct local skew for \(value.host)"
            )

            var transferCalls = 0
            AssetTransferStubURLProtocol.handler = { request in
                transferCalls += 1
                XCTAssertEqual(request.url?.path, "/v1/assets/transfer")
                let body = try self.requestBody(request)
                let object = try XCTUnwrap(
                    JSONSerialization.jsonObject(with: body) as? [String: Any]
                )
                XCTAssertEqual(
                    object["creation_time_ms"] as? UInt64,
                    value.server - 10_000
                )
                return (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 503,
                        httpVersion: nil,
                        headerFields: ["Content-Type": "application/json"]
                    )!,
                    Data(#"{"code":"unavailable"}"#.utf8)
                )
            }
            do {
                _ = try await client.prepareDetachedAssetTransfer(
                    request(creationTimeMs: value.server - 10_000)
                )
                XCTFail("stubbed transfer failure was accepted")
            } catch let ToriiClientError.httpStatus(code, _, _) {
                XCTAssertEqual(code, 503)
            } catch {
                XCTFail("corrected transfer time failed before HTTP: \(error)")
            }
            XCTAssertEqual(
                transferCalls,
                1,
                "corrected server time must be used for transfer request validation"
            )
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testTransportLossReturnsDurableUncertainSubmissionEvidence() async throws {
        let fixture = try await makeDraftFixture()
        let evidence = try fixture.client.finalizeDetachedAssetTransfer(
            fixture.draft,
            signingKey: fixture.signingKey
        )
        XCTAssertEqual(evidence.expiresAtMs, 1_700_000_120_000)
        XCTAssertEqual(
            try JSONDecoder().decode(
                ToriiDetachedAssetTransferSubmissionEvidence.self,
                from: JSONEncoder().encode(evidence)
            ),
            evidence,
            "durable evidence must survive strict Codable reconstruction"
        )
        XCTAssertEqual(
            try ToriiDetachedAssetTransferSubmissionEvidence(
                networkId: evidence.networkId,
                chainId: evidence.chainId,
                submittedRequest: evidence.submittedRequest,
                signedTransaction: evidence.signedTransaction,
                finalization: evidence.finalization
            ),
            evidence,
            "already signed transaction finalization must be idempotent"
        )

        AssetTransferStubURLProtocol.handler = { _ in
            throw URLError(.networkConnectionLost)
        }
        do {
            _ = try await fixture.client.submitFinalizedDetachedAssetTransfer(
                evidence
            )
            XCTFail("transport loss must produce typed uncertain evidence")
        } catch let uncertain as ToriiDetachedAssetTransferSubmissionUncertainError {
            XCTAssertEqual(uncertain.evidence, evidence)
            XCTAssertEqual(uncertain.evidence.transactionHashHex, evidence.transactionHashHex)
            guard case .transport = uncertain.cause else {
                return XCTFail("transport loss used unexpected cause: \(uncertain.cause)")
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testReplayConflictReconcilesOnlyTheExactAuthoritativeHash() async throws {
        let fixture = try await makeDraftFixture()
        let evidence = try fixture.client.finalizeDetachedAssetTransfer(
            fixture.draft,
            signingKey: fixture.signingKey
        )
        AssetTransferStubURLProtocol.handler = { request in
            (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 409,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Data(#"{"code":"already_enqueued"}"#.utf8)
            )
        }
        let uncertain: ToriiDetachedAssetTransferSubmissionUncertainError
        do {
            _ = try await fixture.client.submitFinalizedDetachedAssetTransfer(
                evidence
            )
            return XCTFail("HTTP 409 replay conflict must be uncertain")
        } catch let error as ToriiDetachedAssetTransferSubmissionUncertainError {
            uncertain = error
            guard case let .httpStatus(code, _, _) = error.cause else {
                return XCTFail("409 used unexpected cause: \(error.cause)")
            }
            XCTAssertEqual(code, 409)
        }

        AssetTransferStubURLProtocol.handler = { request in
            let query = URLComponents(
                url: request.url!,
                resolvingAgainstBaseURL: false
            )?.queryItems
            XCTAssertEqual(
                query?.first(where: { $0.name == "hash" })?.value,
                evidence.transactionHashHex
            )
            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                try JSONSerialization.data(
                    withJSONObject: self.pipelineStatusObject(
                        hash: evidence.transactionHashHex
                    )
                )
            )
        }
        let status = try await fixture.client.reconcileDetachedAssetTransferSubmission(
            uncertain.evidence,
            pollOptions: PipelineStatusPollOptions(
                pollInterval: 0,
                timeout: 1,
                maxAttempts: 1
            )
        )
        XCTAssertEqual(status.hash, evidence.transactionHashHex)
        XCTAssertEqual(status.state, .applied)
        XCTAssertEqual(status.scope, "global")
        XCTAssertEqual(status.resolvedFrom, "state")
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testDefiniteBadRequestIsNotWrappedAsUncertainSubmission() async throws {
        let fixture = try await makeDraftFixture()
        let evidence = try fixture.client.finalizeDetachedAssetTransfer(
            fixture.draft,
            signingKey: fixture.signingKey
        )
        AssetTransferStubURLProtocol.handler = { request in
            (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 400,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                Data(#"{"code":"invalid_signature"}"#.utf8)
            )
        }
        do {
            _ = try await fixture.client.submitFinalizedDetachedAssetTransfer(
                evidence
            )
            XCTFail("definite HTTP 400 must fail")
        } catch is ToriiDetachedAssetTransferSubmissionUncertainError {
            XCTFail("definite ordinary 4xx must not be marked uncertain")
        } catch let ToriiClientError.httpStatus(code, _, _) {
            XCTAssertEqual(code, 400)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testEvidenceCodableRejectsUnknownHashBase64RequestAndOverflowTampering() async throws {
        let fixture = try await makeDraftFixture()
        let evidence = try fixture.client.finalizeDetachedAssetTransfer(
            fixture.draft,
            signingKey: fixture.signingKey
        )
        let encoded = try JSONEncoder().encode(evidence)
        let original = try XCTUnwrap(
            JSONSerialization.jsonObject(with: encoded) as? [String: Any]
        )

        var hostileObjects: [[String: Any]] = []
        var unknown = original
        unknown["legacy_transaction"] = "forbidden"
        hostileObjects.append(unknown)
        var badHash = original
        badHash["transaction_hash_hex"] = String(repeating: "ab", count: 32)
        hostileObjects.append(badHash)
        var badBase64 = original
        badBase64["signed_transaction_base64"] = "AQ=="
        hostileObjects.append(badBase64)
        var changedRequest = original
        var requestObject = try XCTUnwrap(
            changedRequest["submitted_request"] as? [String: Any]
        )
        requestObject["amount"] = "2"
        changedRequest["submitted_request"] = requestObject
        hostileObjects.append(changedRequest)

        for (index, object) in hostileObjects.enumerated() {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiDetachedAssetTransferSubmissionEvidence.self,
                    from: JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
                ),
                "hostile evidence mutation \(index) must fail"
            )
        }

        let encodedText = try XCTUnwrap(String(data: encoded, encoding: .utf8))
        let overflow = encodedText.replacingOccurrences(
            of: "\"creation_time_ms\":1700000000000",
            with: "\"creation_time_ms\":18446744073709551615"
        )
        XCTAssertNotEqual(overflow, encodedText)
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiDetachedAssetTransferSubmissionEvidence.self,
                from: Data(overflow.utf8)
            ),
            "creation time plus TTL overflow must fail before expiresAtMs is exposed"
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRecoveryAfterCrashBeforePostReplaysOnlyExactPersistedEvidence() async throws {
        let fixture = try await makeDraftFixture()
        let evidence = try fixture.client.finalizeDetachedAssetTransfer(
            fixture.draft,
            signingKey: fixture.signingKey
        )
        var statusCalls = 0
        var postCalls = 0
        var paths: [String] = []
        AssetTransferStubURLProtocol.handler = { request in
            let path = try XCTUnwrap(request.url?.path)
            paths.append(path)
            switch path {
            case "/v1/pipeline/transactions/status":
                statusCalls += 1
                if statusCalls == 1 {
                    return (
                        HTTPURLResponse(
                            url: request.url!,
                            statusCode: 404,
                            httpVersion: nil,
                            headerFields: nil
                        )!,
                        nil
                    )
                }
                return (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 200,
                        httpVersion: nil,
                        headerFields: ["Content-Type": "application/json"]
                    )!,
                    try JSONSerialization.data(withJSONObject: self.pipelineStatusObject(
                        hash: evidence.transactionHashHex
                    ))
                )
            case "/v1/time/now":
                return try self.timeNowResponse(
                    request: request,
                    now: evidence.submittedRequest.creationTimeMs + 1_000
                )
            case "/v1/assets/transfer":
                postCalls += 1
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(
                    try JSONDecoder().decode(
                        ToriiAssetTransferRequest.self,
                        from: try XCTUnwrap(request.httpBody)
                    ),
                    evidence.submittedRequest
                )
                return (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 200,
                        httpVersion: nil,
                        headerFields: ["Content-Type": "application/json"]
                    )!,
                    try JSONSerialization.data(
                        withJSONObject: self.submittedResponseObject(evidence: evidence)
                    )
                )
            default:
                throw URLError(.unsupportedURL)
            }
        }

        let status = try await fixture.client.recoverDetachedAssetTransferSubmission(
            evidence,
            pollOptions: .init(pollInterval: 0, timeout: 1, maxAttempts: 1)
        )
        XCTAssertEqual(status.state, .applied)
        XCTAssertEqual(postCalls, 1)
        XCTAssertEqual(
            paths,
            [
                "/v1/pipeline/transactions/status",
                "/v1/time/now",
                "/v1/assets/transfer",
                "/v1/pipeline/transactions/status",
            ]
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRecoveryUsesAuthoritativeValidationTimeDespiteHostileLocalClock() async throws {
        let fixture = try await makeDraftFixture()
        let evidence = try fixture.client.finalizeDetachedAssetTransfer(
            fixture.draft,
            signingKey: fixture.signingKey
        )
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AssetTransferStubURLProtocol.self]
        let monotonicClock = AssetTransferMonotonicClock([
            1_000,
            UInt64.max / 2,
        ])
        let recoveryClient = ToriiClient(
            baseURL: URL(string: "https://clock-skewed-torii.example")!,
            session: URLSession(configuration: configuration),
            currentTimeMilliseconds: { UInt64.max - 1_000 },
            currentMonotonicMilliseconds: { monotonicClock.now() }
        )
        var statusCalls = 0
        var postCalls = 0
        AssetTransferStubURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/pipeline/transactions/status":
                statusCalls += 1
                if statusCalls == 1 {
                    return (
                        HTTPURLResponse(
                            url: request.url!,
                            statusCode: 404,
                            httpVersion: nil,
                            headerFields: nil
                        )!,
                        nil
                    )
                }
                return (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 200,
                        httpVersion: nil,
                        headerFields: ["Content-Type": "application/json"]
                    )!,
                    try JSONSerialization.data(withJSONObject: self.pipelineStatusObject(
                        hash: evidence.transactionHashHex
                    ))
                )
            case "/v1/time/now":
                return try self.timeNowResponse(
                    request: request,
                    now: evidence.submittedRequest.creationTimeMs + 1_000
                )
            case "/v1/assets/transfer":
                postCalls += 1
                XCTAssertEqual(
                    try JSONDecoder().decode(
                        ToriiAssetTransferRequest.self,
                        from: try XCTUnwrap(request.httpBody)
                    ),
                    evidence.submittedRequest
                )
                return (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 200,
                        httpVersion: nil,
                        headerFields: ["Content-Type": "application/json"]
                    )!,
                    try JSONSerialization.data(
                        withJSONObject: self.submittedResponseObject(evidence: evidence)
                    )
                )
            default:
                throw URLError(.unsupportedURL)
            }
        }

        let status = try await recoveryClient.recoverDetachedAssetTransferSubmission(
            evidence,
            pollOptions: .init(pollInterval: 0, timeout: 1, maxAttempts: 1)
        )
        XCTAssertEqual(status.state, .applied)
        XCTAssertEqual(postCalls, 1)
        XCTAssertEqual(statusCalls, 2)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRecoveryOfAlreadyAppliedEvidenceNeverPosts() async throws {
        let fixture = try await makeDraftFixture()
        let evidence = try fixture.client.finalizeDetachedAssetTransfer(
            fixture.draft,
            signingKey: fixture.signingKey
        )
        var calls = 0
        AssetTransferStubURLProtocol.handler = { request in
            calls += 1
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                try JSONSerialization.data(withJSONObject: self.pipelineStatusObject(
                    hash: evidence.transactionHashHex
                ))
            )
        }
        let status = try await fixture.client.recoverDetachedAssetTransferSubmission(
            evidence,
            pollOptions: .init(pollInterval: 0, timeout: 1, maxAttempts: 1)
        )
        XCTAssertEqual(status.state, .applied)
        XCTAssertEqual(calls, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRecoveryAcceptsIdempotentAppliedReplayAfterStatusRace() async throws {
        let fixture = try await makeDraftFixture()
        let evidence = try fixture.client.finalizeDetachedAssetTransfer(
            fixture.draft,
            signingKey: fixture.signingKey
        )
        var statusCalls = 0
        var postCalls = 0
        AssetTransferStubURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/pipeline/transactions/status":
                statusCalls += 1
                if statusCalls == 1 {
                    return (
                        HTTPURLResponse(
                            url: request.url!,
                            statusCode: 404,
                            httpVersion: nil,
                            headerFields: nil
                        )!,
                        nil
                    )
                }
                return (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 200,
                        httpVersion: nil,
                        headerFields: ["Content-Type": "application/json"]
                    )!,
                    try JSONSerialization.data(withJSONObject: self.pipelineStatusObject(
                        hash: evidence.transactionHashHex
                    ))
                )
            case "/v1/time/now":
                return try self.timeNowResponse(
                    request: request,
                    now: evidence.submittedRequest.creationTimeMs + 1_000
                )
            case "/v1/assets/transfer":
                postCalls += 1
                XCTAssertEqual(
                    try JSONDecoder().decode(
                        ToriiAssetTransferRequest.self,
                        from: try XCTUnwrap(request.httpBody)
                    ),
                    evidence.submittedRequest
                )
                return (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 200,
                        httpVersion: nil,
                        headerFields: ["Content-Type": "application/json"]
                    )!,
                    try JSONSerialization.data(
                        withJSONObject: self.appliedReplayResponseObject(evidence: evidence)
                    )
                )
            default:
                throw URLError(.unsupportedURL)
            }
        }

        let status = try await fixture.client.recoverDetachedAssetTransferSubmission(
            evidence,
            pollOptions: .init(pollInterval: 0, timeout: 1, maxAttempts: 1)
        )
        XCTAssertEqual(status.state, .applied)
        XCTAssertEqual(postCalls, 1)
        XCTAssertEqual(statusCalls, 2)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRecoveryTreatsExactReplayConflictAsReconciliationSignal() async throws {
        let fixture = try await makeDraftFixture()
        let evidence = try fixture.client.finalizeDetachedAssetTransfer(
            fixture.draft,
            signingKey: fixture.signingKey
        )
        var statusCalls = 0
        var postCalls = 0
        AssetTransferStubURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/pipeline/transactions/status":
                statusCalls += 1
                if statusCalls == 1 {
                    return (
                        HTTPURLResponse(
                            url: request.url!,
                            statusCode: 404,
                            httpVersion: nil,
                            headerFields: nil
                        )!,
                        nil
                    )
                }
                return (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 200,
                        httpVersion: nil,
                        headerFields: ["Content-Type": "application/json"]
                    )!,
                    try JSONSerialization.data(withJSONObject: self.pipelineStatusObject(
                        hash: evidence.transactionHashHex
                    ))
                )
            case "/v1/time/now":
                return try self.timeNowResponse(
                    request: request,
                    now: evidence.submittedRequest.creationTimeMs + 1_000
                )
            case "/v1/assets/transfer":
                postCalls += 1
                XCTAssertEqual(
                    try JSONDecoder().decode(
                        ToriiAssetTransferRequest.self,
                        from: try XCTUnwrap(request.httpBody)
                    ),
                    evidence.submittedRequest
                )
                return (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 409,
                        httpVersion: nil,
                        headerFields: ["x-iroha-reject-code": "PRTRY:ALREADY_ENQUEUED"]
                    )!,
                    Data(#"{"code":"already_enqueued"}"#.utf8)
                )
            default:
                throw URLError(.unsupportedURL)
            }
        }
        let status = try await fixture.client.recoverDetachedAssetTransferSubmission(
            evidence,
            pollOptions: .init(pollInterval: 0, timeout: 1, maxAttempts: 1)
        )
        XCTAssertEqual(status.state, .applied)
        XCTAssertEqual(postCalls, 1)
        XCTAssertEqual(statusCalls, 2)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRecoveryPreservesEvidenceAcrossDefiniteReplayRejectionRace() async throws {
        let fixture = try await makeDraftFixture()
        let evidence = try fixture.client.finalizeDetachedAssetTransfer(
            fixture.draft,
            signingKey: fixture.signingKey
        )
        var statusCalls = 0
        var postCalls = 0
        AssetTransferStubURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/pipeline/transactions/status":
                statusCalls += 1
                return (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 404,
                        httpVersion: nil,
                        headerFields: nil
                    )!,
                    nil
                )
            case "/v1/time/now":
                return try self.timeNowResponse(
                    request: request,
                    now: evidence.submittedRequest.creationTimeMs + 1_000
                )
            case "/v1/assets/transfer":
                postCalls += 1
                return (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 400,
                        httpVersion: nil,
                        headerFields: ["Content-Type": "application/json"]
                    )!,
                    Data(#"{"code":"temporarily_not_admissible"}"#.utf8)
                )
            default:
                throw URLError(.unsupportedURL)
            }
        }

        do {
            _ = try await fixture.client.recoverDetachedAssetTransferSubmission(
                evidence,
                pollOptions: .init(pollInterval: 0, timeout: 1, maxAttempts: 1)
            )
            XCTFail("a status-absent replay rejection must preserve uncertain evidence")
        } catch let error as ToriiDetachedAssetTransferSubmissionUncertainError {
            XCTAssertEqual(error.evidence, evidence)
            guard case let .httpStatus(code, _, _) = error.cause else {
                return XCTFail("unexpected replay rejection cause: \(error.cause)")
            }
            XCTAssertEqual(code, 400)
        }
        XCTAssertEqual(postCalls, 1)
        XCTAssertEqual(statusCalls, 2)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testRecoveryExpiresAbsentEvidenceWithoutPosting() async throws {
        let fixture = try await makeDraftFixture()
        let evidence = try fixture.client.finalizeDetachedAssetTransfer(
            fixture.draft,
            signingKey: fixture.signingKey
        )
        var postCalls = 0
        AssetTransferStubURLProtocol.handler = { request in
            switch request.url?.path {
            case "/v1/pipeline/transactions/status":
                return (
                    HTTPURLResponse(
                        url: request.url!,
                        statusCode: 404,
                        httpVersion: nil,
                        headerFields: nil
                    )!,
                    nil
                )
            case "/v1/time/now":
                return try self.timeNowResponse(request: request, now: evidence.expiresAtMs)
            case "/v1/assets/transfer":
                postCalls += 1
                throw URLError(.dataNotAllowed)
            default:
                throw URLError(.unsupportedURL)
            }
        }
        do {
            _ = try await fixture.client.recoverDetachedAssetTransferSubmission(
                evidence,
                pollOptions: .init(pollInterval: 0, timeout: 1, maxAttempts: 1)
            )
            XCTFail("expired absent evidence must terminate recovery")
        } catch let error as ToriiDetachedAssetTransferRecoveryExpiredError {
            XCTAssertEqual(error.evidence, evidence)
        }
        XCTAssertEqual(postCalls, 0)
    }

    private func responseObject() -> [String: Any] {
        let transactionPayload = canonicalAssetPayload()
        let signingMessage = IrohaHash.hash(transactionPayload)
        let intent: [String: Any] = [
            "chain_id": "asset-transfer-test",
            "authority": Self.authority,
            "asset_definition_id": Self.assetDefinitionId,
            "asset_balance_scope": "dataspace:10",
            "amount": "1.25",
            "destination": Self.destination,
            "memo": "invoice 42",
            "fee_payment": feePaymentObject(Self.feePayment),
            "creation_time_ms": 1_700_000_000_000 as UInt64,
            "transaction_ttl_ms": 120_000 as UInt64,
        ]
        let receipt: [String: Any] = [
            "operation_kind": "asset_transfer",
            "status": "pending_signature",
            "transport": "torii",
            "intent": intent,
            "payload_signing_hash_hex": hashHex(signingMessage),
        ]
        return [
            "ok": true,
            "submitted": false,
            "intent": intent,
            "transaction_payload_b64": transactionPayload.base64EncodedString(),
            "signing_message_b64": signingMessage.base64EncodedString(),
            "receipt": receipt,
        ]
    }

    private func submittedResponseObject() -> [String: Any] {
        let intent = responseObject()["intent"] as! [String: Any]
        let transactionHash = hashHex(0x33)
        let receipt: [String: Any] = [
            "operation_kind": "asset_transfer",
            "status": "submitted",
            "transport": "torii",
            "intent": intent,
            "payload_signing_hash_hex": hashHex(hashBytes()),
            "transaction_hash_hex": transactionHash,
            "entrypoint_hash_hex": transactionHash,
        ]
        return [
            "ok": true,
            "submitted": true,
            "intent": intent,
            "transaction_hash_hex": transactionHash,
            "entrypoint_hash_hex": transactionHash,
            "pipeline_status": [
                "hash": transactionHash,
                "status": ["kind": "Queued"],
                "summary": "Queued",
                "diagnostics": [],
                "scope": "local",
                "resolved_from": "queue",
            ],
            "receipt": receipt,
        ]
    }

    private func appliedReplayResponseObject() -> [String: Any] {
        var object = submittedResponseObject()
        var pipeline = object["pipeline_status"] as! [String: Any]
        pipeline["status"] = ["kind": "Applied", "block_height": 44]
        pipeline["summary"] = "Applied at block 44"
        pipeline["resolved_from"] = "state"
        object["pipeline_status"] = pipeline
        var receipt = object["receipt"] as! [String: Any]
        receipt["status"] = "applied"
        object["receipt"] = receipt
        return object
    }

    private func submittedResponseObject(
        evidence: ToriiDetachedAssetTransferSubmissionEvidence
    ) -> [String: Any] {
        let request = evidence.submittedRequest
        var intent: [String: Any] = [
            "chain_id": evidence.chainId,
            "authority": request.authority,
            "asset_definition_id": request.assetDefinitionId,
            "asset_balance_scope": request.assetBalanceScope,
            "amount": request.amount,
            "destination": request.destination,
            "creation_time_ms": request.creationTimeMs,
            "transaction_ttl_ms": request.transactionTtlMs,
        ]
        if let memo = request.memo { intent["memo"] = memo }
        intent["fee_payment"] = feePaymentObject(request.feePayment)
        let receipt: [String: Any] = [
            "operation_kind": "asset_transfer",
            "status": "submitted",
            "transport": "torii",
            "intent": intent,
            "payload_signing_hash_hex": evidence.payloadSigningHashHex,
            "transaction_hash_hex": evidence.transactionHashHex,
            "entrypoint_hash_hex": evidence.entrypointHashHex,
        ]
        return [
            "ok": true,
            "submitted": true,
            "intent": intent,
            "transaction_hash_hex": evidence.transactionHashHex,
            "entrypoint_hash_hex": evidence.entrypointHashHex,
            "pipeline_status": [
                "hash": evidence.transactionHashHex,
                "status": ["kind": "Queued"],
                "summary": "Queued",
                "diagnostics": [],
                "scope": "local",
                "resolved_from": "queue",
            ],
            "receipt": receipt,
        ]
    }

    private func appliedReplayResponseObject(
        evidence: ToriiDetachedAssetTransferSubmissionEvidence
    ) -> [String: Any] {
        var object = submittedResponseObject(evidence: evidence)
        var pipeline = object["pipeline_status"] as! [String: Any]
        pipeline["status"] = ["kind": "Applied", "block_height": 44]
        pipeline["summary"] = "Applied at block 44"
        pipeline["resolved_from"] = "state"
        object["pipeline_status"] = pipeline
        var receipt = object["receipt"] as! [String: Any]
        receipt["status"] = "applied"
        object["receipt"] = receipt
        return object
    }

    private func timeNowResponse(
        request: URLRequest,
        now: UInt64
    ) throws -> (HTTPURLResponse, Data?) {
        (
            HTTPURLResponse(
                url: try XCTUnwrap(request.url),
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!,
            try JSONSerialization.data(withJSONObject: [
                "now": now,
                "offset_ms": 0,
                "confidence_ms": 1,
            ])
        )
    }

    private func decodeResponse(_ object: [String: Any]) throws -> ToriiAssetTransferResponse {
        try JSONDecoder().decode(
            ToriiAssetTransferResponse.self,
            from: JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        )
    }

    private func pipelineStatusObject() -> [String: Any] {
        pipelineStatusObject(hash: hashHex(0x33))
    }

    private func pipelineStatusObject(hash: String) -> [String: Any] {
        [
            "hash": hash,
            "status": ["kind": "Applied", "block_height": 44],
            "summary": "Applied at block 44",
            "diagnostics": [],
            "scope": "global",
            "resolved_from": "state",
        ]
    }

    private func pipelineStatus() throws -> ToriiPipelineTransactionStatus {
        try JSONDecoder().decode(
            ToriiPipelineTransactionStatus.self,
            from: JSONSerialization.data(withJSONObject: pipelineStatusObject())
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    private func makeClient(now: UInt64) -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AssetTransferStubURLProtocol.self]
        return ToriiClient(
            baseURL: URL(string: "https://torii.example")!,
            session: URLSession(configuration: configuration),
            localSigningContext: ToriiLocalSigningContext(
                networkId: TestNetworkIds.canonical
            ),
            currentTimeMilliseconds: { now }
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    private func makeClient(
        baseURL: URL,
        now: UInt64,
        monotonicNow: UInt64 = 1_000
    ) -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AssetTransferStubURLProtocol.self]
        return ToriiClient(
            baseURL: baseURL,
            session: URLSession(configuration: configuration),
            localSigningContext: ToriiLocalSigningContext(
                networkId: TestNetworkIds.canonical
            ),
            currentTimeMilliseconds: { now },
            currentMonotonicMilliseconds: { monotonicNow }
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    private func makeDraftFixture(
        now: UInt64 = 1_700_000_000_000,
        amount: String = "1.25"
    ) async throws -> (
        client: ToriiClient,
        draft: ToriiAssetTransferDraft,
        signingKey: SigningKey
    ) {
        let request = self.request(
            amount: amount,
            memo: nil,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
            creationTimeMs: now
        )
        let transactionPayload = try CanonicalUnsignedTransactionTestSupport.assetPayload(
            request: request,
            networkId: TestNetworkIds.canonical
        )
        let signingMessage = IrohaHash.hash(transactionPayload)
        let intent: [String: Any] = [
            "chain_id": "asset-transfer-test",
            "authority": request.authority,
            "asset_definition_id": request.assetDefinitionId,
            "asset_balance_scope": request.assetBalanceScope,
            "amount": request.amount,
            "destination": request.destination,
            "fee_payment": feePaymentObject(request.feePayment),
            "creation_time_ms": request.creationTimeMs,
            "transaction_ttl_ms": request.transactionTtlMs,
        ]
        let receipt: [String: Any] = [
            "operation_kind": "asset_transfer",
            "status": "pending_signature",
            "transport": "torii",
            "intent": intent,
            "payload_signing_hash_hex": hashHex(signingMessage),
        ]
        let prepared: [String: Any] = [
            "ok": true,
            "submitted": false,
            "intent": intent,
            "transaction_payload_b64": transactionPayload.base64EncodedString(),
            "signing_message_b64": signingMessage.base64EncodedString(),
            "receipt": receipt,
        ]
        let preparedData = try JSONSerialization.data(
            withJSONObject: prepared,
            options: [.sortedKeys]
        )
        AssetTransferStubURLProtocol.handler = { urlRequest in
            XCTAssertEqual(urlRequest.url?.path, "/v1/assets/transfer")
            return (
                HTTPURLResponse(
                    url: urlRequest.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                preparedData
            )
        }
        let client = makeClient(now: now)
        let draft = try await client.prepareDetachedAssetTransfer(request)
        let signingKey = try SigningKey.ed25519(
            privateKey: Self.authorityKey.privateKeyBytes
        )
        return (client, draft, signingKey)
    }

    private func canonicalAssetPayload(
        _ requestOverride: ToriiAssetTransferRequest? = nil,
        networkId: NetworkId = TestNetworkIds.canonical
    ) -> Data {
        try! CanonicalUnsignedTransactionTestSupport.assetPayload(
            request: requestOverride ?? request(),
            networkId: networkId
        )
    }

    private func hashBytes() -> Data {
        IrohaHash.hash(canonicalAssetPayload())
    }

    private func feePaymentObject(_ intent: FeePaymentIntent) -> [String: Any] {
        let data = try! intent.canonicalJSONData()
        return try! JSONSerialization.jsonObject(with: data) as! [String: Any]
    }

    private func hashHex(_ byte: UInt8) -> String {
        Data(repeating: byte, count: 32).map { String(format: "%02x", $0) }.joined()
    }

    private func hashHex(_ bytes: Data) -> String {
        bytes.map { String(format: "%02x", $0) }.joined()
    }
}
