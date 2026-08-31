import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import XCTest
@testable import IrohaSwift

/// A transport stub isolated from the broad Torii client test suite.
private final class GovernanceBallotStubURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data?))?

    override class func canInit(with request: URLRequest) -> Bool { true }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(
                self,
                didFailWithError: NSError(domain: "GovernanceBallotStub", code: -1)
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

/// Exercises governance ballot encoding and pre-dispatch trust boundaries.
final class ToriiGovernanceBallotModelTests: XCTestCase {
    private let canonicalSigningSeed = Data(repeating: 0x41, count: 32)

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

    private func governanceAuth(accountId: String) -> ToriiCanonicalRequestAuth {
        ToriiCanonicalRequestAuth(
            accountId: accountId,
            privateKey: Data(repeating: 1, count: 32),
            timestampMs: 4_102_444_801_000,
            nonce: "governance-ballot-test"
        )
    }

    override func tearDown() {
        GovernanceBallotStubURLProtocol.handler = nil
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

    private func makeClient(
        baseURL: URL = URL(string: "https://example.test")!,
        defaultHeaders: [String: String] = [:]
    ) -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [GovernanceBallotStubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        return ToriiClient(
            baseURL: baseURL,
            session: session,
            defaultHeaders: defaultHeaders,
            localSigningContext: ToriiLocalSigningContext(networkId: TestNetworkIds.canonical)
        )
    }

    private func canonicalOwnerLiteral(
        domain: String = "wonderland",
        chainDiscriminant: UInt16 = AccountId.defaultNetworkPrefix
    ) throws -> String {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: 1, count: 32))
        let address = try AccountAddress.fromAccount(publicKey: keypair.publicKey)
        return try address.toI105(networkPrefix: chainDiscriminant)
    }

    func testGovernanceBallotDraftResponseIsExact() throws {
        let valid = Data(
            "{\"drafted\":true,\"tx_instructions\":[{\"wire_id\":\"CastZkBallot\",\"payload_hex\":\"00\"}]}".utf8
        )
        let decoded = try JSONDecoder().decode(ToriiGovernanceBallotResponse.self, from: valid)
        XCTAssertTrue(decoded.drafted)
        XCTAssertEqual(decoded.txInstructions.count, 1)

        for invalid in [
            "{\"ok\":false,\"accepted\":false,\"reason\":\"invalid ballot\",\"tx_instructions\":[]}",
            "{\"drafted\":false,\"tx_instructions\":[{\"wire_id\":\"CastZkBallot\",\"payload_hex\":\"00\"}]}",
            "{\"drafted\":true,\"tx_instructions\":[]}",
            "{\"drafted\":true,\"tx_instructions\":[{\"wire_id\":\"Cast ZkBallot\",\"payload_hex\":\"00\"}]}",
            "{\"drafted\":true,\"tx_instructions\":[{\"wire_id\":\"CastZkBallot\",\"payload_hex\":\"AA\"}]}",
        ] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiGovernanceBallotResponse.self,
                    from: Data(invalid.utf8)
                )
            )
        }
    }

    func testSubmitGovernancePlainBallotRequiresCanonicalLosslessQuantity() throws {
        let canonical = "18446744073709551616.25"
        let request = ToriiGovernancePlainBallotRequest(
            authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            networkId: TestNetworkIds.canonical,
            referendumId: "ref-1",
            owner: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            amount: canonical,
            durationBlocks: 5,
            direction: .aye
        )
        let data = try JSONEncoder().encode(request)
        let json = try XCTUnwrap(
            JSONSerialization.jsonObject(with: data) as? [String: Any]
        )
        XCTAssertEqual(json["amount"] as? String, canonical)
        XCTAssertEqual(json["duration_blocks"] as? String, "5")
        XCTAssertEqual(json["direction"] as? String, "Aye")

        let overflowing = String(repeating: "9", count: 155)
        for invalid in ["+1", "01", "1.0", "1.2300", " 1", "1 ", "-1", overflowing] {
            let invalidRequest = ToriiGovernancePlainBallotRequest(
                authority: request.authority,
                networkId: request.networkId,
                referendumId: request.referendumId,
                owner: request.owner,
                amount: invalid,
                durationBlocks: request.durationBlocks,
                direction: request.direction
            )
            XCTAssertThrowsError(
                try JSONEncoder().encode(invalidRequest),
                "noncanonical amount \(invalid) must be rejected"
            )
        }
    }

    func testGovernanceMutationContextsAndProofsAreExactBeforeEncoding() throws {
        let owner = try canonicalOwnerLiteral()
        let invalidEncoders: [() throws -> Data] = [
            {
                try JSONEncoder().encode(ToriiGovernanceZkBallotV1Request(
                    authority: owner,
                    networkId: TestNetworkIds.canonical,
                    electionId: "election-1 ",
                    backend: "halo2/ipa",
                    envelopeB64: "AQIDBA=="
                ))
            },
            {
                try JSONEncoder().encode(ToriiGovernanceZkBallotProofRequest(
                    authority: owner,
                    networkId: TestNetworkIds.canonical,
                    electionId: "election-1",
                    ballot: .init(
                        backend: "halo2/ipa",
                        envelopeBytesB64: "AQIDBA== "
                    )
                ))
            },
            {
                try JSONEncoder().encode(ToriiGovernancePlainBallotRequest(
                    authority: owner,
                    networkId: TestNetworkIds.canonical,
                    referendumId: "referendum 1",
                    owner: owner,
                    amount: "250",
                    durationBlocks: 12,
                    direction: .aye
                ))
            },
            {
                try JSONEncoder().encode(ToriiGovernanceZkBallotV1Request(
                    authority: owner,
                    networkId: TestNetworkIds.canonical,
                    electionId: "election/1",
                    backend: "halo2/ipa",
                    envelopeB64: "AQIDBA=="
                ))
            },
            {
                try JSONEncoder().encode(ToriiGovernanceZkBallotProofRequest(
                    authority: owner,
                    networkId: TestNetworkIds.canonical,
                    electionId: ".hidden",
                    ballot: .init(
                        backend: "halo2/ipa",
                        envelopeBytesB64: "AQIDBA=="
                    )
                ))
            },
            {
                try JSONEncoder().encode(ToriiGovernancePlainBallotRequest(
                    authority: owner,
                    networkId: TestNetworkIds.canonical,
                    referendumId: "投票",
                    owner: owner,
                    amount: "250",
                    durationBlocks: 12,
                    direction: .aye
                ))
            },
        ]
        for encode in invalidEncoders {
            XCTAssertThrowsError(try encode())
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testInvalidGovernanceZkPublicInputsFailBeforeTransportDispatch() async throws {
        let owner = try canonicalOwnerLiteral()
        let request = ToriiGovernanceZkBallotV1Request(
            authority: owner,
            networkId: TestNetworkIds.canonical,
            electionId: "election-1",
            backend: "halo2/ipa",
            envelopeB64: "AQIDBA==",
            publicInputs: .init(
                rootHint: "not-hex",
                owner: owner,
                amount: "250",
                durationBlocks: 12
            )
        )
        var dispatched = false
        GovernanceBallotStubURLProtocol.handler = { request in
            dispatched = true
            XCTFail("invalid governance request reached transport dispatch: \(request)")
            return (
                HTTPURLResponse(url: request.url!, statusCode: 500, httpVersion: nil, headerFields: nil)!,
                Data()
            )
        }

        await XCTAssertThrowsErrorAsync(
            try await makeClient().submitGovernanceZkBallotV1(
                request,
                canonicalAuth: governanceAuth(accountId: owner)
            )
        ) { error in
            guard case let ToriiClientError.invalidPayload(message) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertTrue(message.contains("root_hint"))
        }
        XCTAssertFalse(dispatched)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGovernanceBallotRejectsForeignNetworkAndMismatchedPrincipalBeforeDispatch() async throws {
        let owner = try canonicalOwnerLiteral()
        var dispatchCount = 0
        GovernanceBallotStubURLProtocol.handler = { request in
            dispatchCount += 1
            XCTFail("foreign or unauthenticated governance ballot reached transport: \(request)")
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
        let foreign = ToriiGovernancePlainBallotRequest(
            authority: owner,
            networkId: TestNetworkIds.other,
            referendumId: "referendum-1",
            owner: owner,
            amount: "1",
            durationBlocks: 1,
            direction: .aye
        )
        await XCTAssertThrowsErrorAsync(
            try await makeClient().submitGovernancePlainBallot(
                foreign,
                canonicalAuth: governanceAuth(accountId: owner)
            )
        ) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("unexpected foreign-network error: \(error)")
            }
            XCTAssertTrue(reason.contains("exact NetworkId"))
        }

        let canonical = ToriiGovernancePlainBallotRequest(
            authority: owner,
            networkId: TestNetworkIds.canonical,
            referendumId: "referendum-1",
            owner: owner,
            amount: "1",
            durationBlocks: 1,
            direction: .aye
        )
        await XCTAssertThrowsErrorAsync(
            try await makeClient().submitGovernancePlainBallot(
                canonical,
                canonicalAuth: canonicalReadAuth
            )
        ) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("unexpected principal-mismatch error: \(error)")
            }
            XCTAssertTrue(reason.contains("must equal governance ballot authority"))
        }
        XCTAssertEqual(dispatchCount, 0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGovernanceBallotRedirectResponseIsNotRetried() async throws {
        let owner = try canonicalOwnerLiteral()
        var dispatchCount = 0
        GovernanceBallotStubURLProtocol.handler = { request in
            dispatchCount += 1
            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 307,
                    httpVersion: nil,
                    headerFields: ["Location": "https://other.example/v1/gov/ballots/plain"]
                )!,
                Data()
            )
        }
        let request = ToriiGovernancePlainBallotRequest(
            authority: owner,
            networkId: TestNetworkIds.canonical,
            referendumId: "referendum-1",
            owner: owner,
            amount: "1",
            durationBlocks: 1,
            direction: .aye
        )
        await XCTAssertThrowsErrorAsync(
            try await makeClient().submitGovernancePlainBallot(
                request,
                canonicalAuth: governanceAuth(accountId: owner)
            )
        ) { error in
            guard case let ToriiClientError.httpStatus(code, _, _) = error else {
                return XCTFail("unexpected redirect error: \(error)")
            }
            XCTAssertEqual(code, 307)
        }
        XCTAssertEqual(dispatchCount, 1)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testGovernanceZkBackendsRejectWhitespaceAndControlsBeforeDispatch() async throws {
        let owner = try canonicalOwnerLiteral()
        var dispatched = false
        GovernanceBallotStubURLProtocol.handler = { request in
            dispatched = true
            XCTFail("invalid governance backend reached transport dispatch: \(request)")
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

        for backend in ["", " halo2/ipa", "halo2/ipa ", "halo2 /ipa", "halo2\t/ipa", "halo2\u{0000}/ipa"] {
            let envelope = ToriiGovernanceZkBallotV1Request(
                authority: owner,
                networkId: TestNetworkIds.canonical,
                electionId: "election-1",
                backend: backend,
                envelopeB64: "AQIDBA=="
            )
            await XCTAssertThrowsErrorAsync(
                try await makeClient().submitGovernanceZkBallotV1(
                    envelope,
                    canonicalAuth: governanceAuth(accountId: owner)
                )
            ) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("unexpected flat-v1 error for \(backend.debugDescription): \(error)")
                }
                XCTAssertTrue(reason.contains("backend"))
            }

            let proof = ToriiGovernanceZkBallotProofRequest(
                authority: owner,
                networkId: TestNetworkIds.canonical,
                electionId: "election-1",
                ballot: .init(
                    backend: backend,
                    envelopeBytesB64: "AQIDBA=="
                )
            )
            await XCTAssertThrowsErrorAsync(
                try await makeClient().submitGovernanceZkBallotProofV1(
                    proof,
                    canonicalAuth: governanceAuth(accountId: owner)
                )
            ) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("unexpected proof-v1 error for \(backend.debugDescription): \(error)")
                }
                XCTAssertTrue(reason.contains("backend"))
            }
            XCTAssertFalse(dispatched)
        }
    }

    func testSubmitGovernanceZkBallotV1EncodesFlatTypedEnvelope() throws {
        let expectation = expectation(description: "zk ballot v1")
        let owner = try canonicalOwnerLiteral()
        GovernanceBallotStubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/gov/ballots/zk-v1")
            let body = self.bodyJSON(from: request)
            XCTAssertEqual(body["network_id"] as? String, TestNetworkIds.canonical.literal)
            XCTAssertNil(body["chain_id"])
            XCTAssertEqual(
                request.value(forHTTPHeaderField: "X-Iroha-Account"),
                try AccountAddress.parseEncoded(owner).canonicalHex()
            )
            XCTAssertEqual(body["backend"] as? String, "halo2/ipa")
            XCTAssertEqual(body["envelope_b64"] as? String, "AQIDBA==")
            XCTAssertNil(body["public"])
            XCTAssertEqual(body["root_hint"] as? String, String(repeating: "11", count: 32))
            XCTAssertEqual(body["owner"] as? String, owner)
            XCTAssertEqual(body["amount"] as? String, "250")
            XCTAssertEqual(body["duration_blocks"] as? Int, 12)
            XCTAssertEqual(body["direction"] as? String, "Nay")
            XCTAssertEqual(body["nullifier"] as? String, String(repeating: "22", count: 32))
            return (
                HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                headerFields: ["Content-Type": "application/json"])!,
                Data("{\"drafted\":true,\"tx_instructions\":[{\"wire_id\":\"CastZkBallot\",\"payload_hex\":\"00\"}]}".utf8)
            )
        }
        let request = ToriiGovernanceZkBallotV1Request(
            authority: owner,
            networkId: TestNetworkIds.canonical,
            electionId: "election-1",
            backend: "halo2/ipa",
            envelopeB64: "AQIDBA==",
            publicInputs: .init(
                rootHint: "0x\(String(repeating: "11", count: 32))",
                owner: owner,
                amount: "250",
                durationBlocks: 12,
                direction: .nay,
                nullifier: "blake2b32:\(String(repeating: "22", count: 32))"
            )
        )
        makeClient().submitGovernanceZkBallotV1(
            request,
            canonicalAuth: governanceAuth(accountId: owner)
        ) { result in
            if case .failure(let error) = result {
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testSubmitGovernanceZkBallotProofV1EncodesTypedNestedProof() throws {
        let expectation = expectation(description: "zk ballot proof v1")
        let owner = try canonicalOwnerLiteral()
        GovernanceBallotStubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/gov/ballots/zk-v1/ballot-proof")
            let body = self.bodyJSON(from: request)
            XCTAssertEqual(body["network_id"] as? String, TestNetworkIds.canonical.literal)
            XCTAssertNil(body["chain_id"])
            let ballot = body["ballot"] as? [String: Any]
            XCTAssertEqual(ballot?["backend"] as? String, "halo2/ipa")
            XCTAssertEqual(ballot?["envelope_bytes"] as? String, "AQIDBA==")
            XCTAssertEqual(ballot?["owner"] as? String, owner)
            XCTAssertEqual(ballot?["amount"] as? String, "250")
            XCTAssertEqual(ballot?["duration_blocks"] as? Int, 12)
            XCTAssertEqual(ballot?["direction"] as? String, "Abstain")
            XCTAssertEqual(ballot?["root_hint"] as? String, String(repeating: "33", count: 32))
            XCTAssertEqual(ballot?["nullifier"] as? String, String(repeating: "44", count: 32))
            return (
                HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                headerFields: ["Content-Type": "application/json"])!,
                Data("{\"drafted\":true,\"tx_instructions\":[{\"wire_id\":\"CastZkBallot\",\"payload_hex\":\"00\"}]}".utf8)
            )
        }
        let request = ToriiGovernanceZkBallotProofRequest(
            authority: owner,
            networkId: TestNetworkIds.canonical,
            electionId: "election-1",
            ballot: .init(
                backend: "halo2/ipa",
                envelopeBytesB64: "AQIDBA==",
                publicInputs: .init(
                    rootHint: String(repeating: "33", count: 32),
                    owner: owner,
                    amount: "250",
                    durationBlocks: 12,
                    direction: .abstain,
                    nullifier: String(repeating: "44", count: 32)
                )
            )
        )
        makeClient().submitGovernanceZkBallotProofV1(
            request,
            canonicalAuth: governanceAuth(accountId: owner)
        ) { result in
            if case .failure(let error) = result {
                XCTFail("unexpected error: \(error)")
            }
            expectation.fulfill()
        }
        waitForExpectations(timeout: 1)
    }

    func testGovernanceTypedRequestBodiesCannotEmitPrivateKeyAliases() throws {
        let owner = try canonicalOwnerLiteral()
        let publicInputs = GovernanceZkBallotPublicInputs(
            owner: owner,
            amount: "250",
            durationBlocks: 12
        )
        let bodies = [
            try JSONEncoder().encode(ToriiGovernanceDeployContractProposalRequest(
                proposalOperator: owner,
                contractAlias: "demo::universal",
                codeHash: Data(repeating: 0x11, count: 32),
                abiHash: Data(repeating: 0x22, count: 32),
                manifestProvenance: .init(
                    signer: "ed25519:public",
                    signature: "ed25519:signature"
                )
            )),
            try JSONEncoder().encode(ToriiGovernancePlainBallotRequest(
                authority: owner,
                networkId: TestNetworkIds.canonical,
                referendumId: "referendum-1",
                owner: owner,
                amount: "250",
                durationBlocks: 12,
                direction: .aye
            )),
            try JSONEncoder().encode(ToriiGovernanceZkBallotV1Request(
                authority: owner,
                networkId: TestNetworkIds.canonical,
                electionId: "election-1",
                backend: "halo2/ipa",
                envelopeB64: "AQIDBA==",
                publicInputs: publicInputs
            )),
            try JSONEncoder().encode(ToriiGovernanceZkBallotProofRequest(
                authority: owner,
                networkId: TestNetworkIds.canonical,
                electionId: "election-1",
                ballot: .init(
                    backend: "halo2/ipa",
                    envelopeBytesB64: "AQIDBA==",
                    publicInputs: publicInputs
                )
            )),
        ]
        let privateAliases = [
            "private_key", "privateKey", "private_key_hex", "privateKeyHex",
            "private_key_bytes", "privateKeyBytes", "private_key_seed", "privateKeySeed",
            "private_key_multihash", "privateKeyMultihash", "private_key_algorithm",
            "privateKeyAlgorithm",
        ]
        for data in bodies {
            let json = try XCTUnwrap(
                JSONSerialization.jsonObject(with: data) as? [String: Any]
            )
            let wire = try JSONSerialization.data(withJSONObject: json, options: [.sortedKeys])
            let text = try XCTUnwrap(String(data: wire, encoding: .utf8))
            for alias in privateAliases {
                XCTAssertFalse(text.contains("\"\(alias)\""), "emitted private alias \(alias)")
            }
        }
    }

}
