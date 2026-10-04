import XCTest
@testable import IrohaSwift

final class ConnectWalletRequestTests: XCTestCase {
    private let baseURL = URL(string: "https://wallet-node.example")!
    private let appKey = Data(repeating: 1, count: 32)
    private let nonce = Data(repeating: 2, count: 16)
    private let token = connectBase64URL(Data(repeating: 7, count: 32))
    private let relay = connectBase64URL(Data(repeating: 8, count: 32))
    private var network: NetworkId {
        try! NetworkId(literal: "hash:4141414141414141414141414141414141414141414141414141414141414141#7023")
    }

    private func uri(_ replacements: [String: String] = [:], omitting: String? = nil) throws -> String {
        let sid = try ConnectCrypto.deriveSessionID(networkID: network, appPublicKey: appKey, nonce: nonce)
        var query = ["sid": connectBase64URL(sid), "network_id": network.literal,
                     "app_pk": connectBase64URL(appKey), "nonce": connectBase64URL(nonce),
                     "node": baseURL.absoluteString, "v": "1", "role": "wallet",
                     "token": token, "relay": relay]
        query.merge(replacements) { _, new in new }
        if let omitting { query.removeValue(forKey: omitting) }
        var components = URLComponents()
        components.scheme = "iroha"
        components.host = "connect"
        components.queryItems = query.sorted { $0.key < $1.key }.map { URLQueryItem(name: $0.key, value: $0.value) }
        return try XCTUnwrap(components.string)
    }

    private func parse(_ literal: String? = nil) throws -> ConnectWalletRequest {
        try ConnectWalletRequest.parse(literal ?? uri(), expectedNetworkID: network, baseURL: baseURL)
    }

    private func openBytes(network: NetworkId? = nil, appKey: Data? = nil, nonce: Data? = nil,
                           direction: ConnectDirection = .appToWallet, sequence: UInt64 = 1) throws -> Data {
        try requireNativeTestCapability(NoritoNativeBridge.shared.isConnectCodecAvailable,
                                        "ConnectWalletRequest tests require the actual native codec")
        let network = network ?? self.network
        let appKey = appKey ?? self.appKey
        let nonce = nonce ?? self.nonce
        let sid = try ConnectCrypto.deriveSessionID(networkID: network, appPublicKey: appKey, nonce: nonce)
        let open = ConnectOpen(appPublicKey: appKey, appMetadata: nil,
                               constraints: ConnectConstraints(networkID: network),
                               permissions: ConnectPermissions(methods: ["SIGN_REQUEST_TX"]))
        let frame = ConnectFrame(sessionID: sid, direction: direction, sequence: sequence,
                                 kind: .control(.open(open)))
        return try ConnectCodec.encode(frame, launchNonce: nonce)
    }

    private func sharedFixture() throws -> [String: Any] {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 { root.deleteLastPathComponent() }
        let bytes = try Data(contentsOf: root.appendingPathComponent("fixtures/connect/session_vectors.json"))
        return try XCTUnwrap(JSONSerialization.jsonObject(with: bytes) as? [String: Any])
    }

    func testLaunchIdentityMatchesSharedCanonicalFixture() throws {
        let fixture = try sharedFixture()
        let request = try parse()
        XCTAssertEqual(request.networkID.literal, fixture["network_id"] as? String)
        XCTAssertEqual(request.sid, fixture["sid_base64url"] as? String)
        XCTAssertEqual(request.sessionID.hexEncodedString(), fixture["sid_hex"] as? String)
        XCTAssertEqual(request.appPublicKey.hexEncodedString(), fixture["app_pk_hex"] as? String)
        XCTAssertEqual(request.nonce.hexEncodedString(), fixture["nonce_hex"] as? String)
        XCTAssertEqual(request.baseURL, baseURL)
        var exported = request.sessionID
        exported[0] ^= 1
        XCTAssertNotEqual(exported, request.sessionID)
    }

    func testCanonicalRequestKeepsTokenOutOfURLDescriptionsAndErrors() throws {
        let request = try parse()
        let transport = try request.makeWebSocketRequest()
        XCTAssertEqual(transport.value(forHTTPHeaderField: "Authorization"), "Bearer \(token)")
        let url = try XCTUnwrap(transport.url)
        XCTAssertEqual(url.scheme, "wss")
        XCTAssertEqual(url.path, "/v1/connect/ws")
        let query = try XCTUnwrap(URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems)
        XCTAssertEqual(Set(query.map(\.name)), ["sid", "role"])
        XCTAssertEqual(query.first(where: { $0.name == "role" })?.value, "wallet")
        for rendered in [url.absoluteString, String(describing: request), String(reflecting: request),
                         String(describing: Array(Mirror(reflecting: request).children))] {
            XCTAssertFalse(rendered.contains(token))
            XCTAssertFalse(rendered.contains(relay))
        }
        XCTAssertThrowsError(try parse(uri(["token": "sensitive-invalid-token"]))) { error in
            XCTAssertFalse(String(describing: error).contains("sensitive-invalid-token"))
            XCTAssertFalse(error.localizedDescription.contains(relay))
        }
    }

    func testRejectsMalformedAuthorityPathFragmentAndQuery() throws {
        let valid = try uri()
        let malformed = [valid.replacingOccurrences(of: "iroha://", with: "https://"),
                         valid.replacingOccurrences(of: "connect?", with: "other?"),
                         valid.replacingOccurrences(of: "connect?", with: "connect/path?"),
                         valid.replacingOccurrences(of: "connect?", with: "user@connect?"),
                         valid.replacingOccurrences(of: "connect?", with: "connect:443?"),
                         valid + "#fragment", valid + "&sid=duplicate", valid + "&%73id=duplicate",
                         valid + "&unknown=value", valid + "&token_wallet=retired", valid + "&nonce"]
        for value in malformed { XCTAssertThrowsError(try parse(value)) }
        for field in ["sid", "network_id", "app_pk", "nonce", "node", "v", "role", "token", "relay"] {
            XCTAssertThrowsError(try parse(uri(omitting: field)))
        }
    }

    func testRejectsIdentityRoleVersionAndCredentialSubstitution() throws {
        let mutations = ["sid": connectBase64URL(Data(repeating: 9, count: 32)),
                         "network_id": TestNetworkIds.canonical.literal,
                         "app_pk": connectBase64URL(Data(repeating: 3, count: 32)),
                         "nonce": connectBase64URL(Data(repeating: 4, count: 16)),
                         "node": "https://foreign.example", "v": "2", "role": "app",
                         "token": token + "=", "relay": "not-a-canonical-token"]
        for (field, value) in mutations { XCTAssertThrowsError(try parse(uri([field: value]))) }
        XCTAssertThrowsError(try parse(uri(["network_id": network.literal.replacingOccurrences(of: "hash:", with: "HASH:")])))
        for (field, count) in [("sid", 32), ("app_pk", 32), ("nonce", 16), ("token", 32), ("relay", 32)] {
            XCTAssertThrowsError(try parse(uri([field: connectBase64URL(Data(repeating: 1, count: count - 1))])))
        }
        XCTAssertThrowsError(try parse(uri(["app_pk": connectBase64URL(Data(repeating: 0, count: 32))])))
        XCTAssertThrowsError(try parse(uri(["nonce": connectBase64URL(Data(repeating: 0, count: 16))])))
    }

    func testConfiguredNodeCannotSmuggleCredentialsQueryOrInsecureTransport() throws {
        for node in ["http://wallet-node.example", "https://user:secret@wallet-node.example",
                     "https://wallet-node.example?token=secret", "https://wallet-node.example#fragment"] {
            XCTAssertThrowsError(try ConnectWalletRequest.parse(uri(["node": node]),
                expectedNetworkID: network, baseURL: XCTUnwrap(URL(string: node))))
        }
    }

    func testNativeOpenIsBoundAndConsumedExactlyOnce() throws {
        let request = try parse()
        let bytes = try openBytes()
        let open = try request.acceptOpen(bytes)
        XCTAssertEqual(open.appPublicKey, appKey)
        XCTAssertEqual(open.constraints.networkID, network)
        XCTAssertEqual(open.permissions?.methods, ["SIGN_REQUEST_TX"])
        XCTAssertThrowsError(try request.acceptOpen(bytes))
        // Reference aliases share the consumed state; copying a reference cannot reset it.
        let alias = request
        XCTAssertThrowsError(try alias.acceptOpen(bytes))
    }

    func testNativeCodecAndOwnerRejectForeignIdentityDirectionCountersAndKindWithoutConsumption() throws {
        let request = try parse()
        let wrongFrames = try [openBytes(network: TestNetworkIds.canonical),
                               openBytes(appKey: Data(repeating: 3, count: 32)),
                               openBytes(nonce: Data(repeating: 4, count: 16))]
        for bytes in wrongFrames { XCTAssertThrowsError(try request.acceptOpen(bytes)) }
        // The canonical native encoder itself refuses invalid Open direction/counter fields.
        // These assertions exercise the combined native-codec/owner boundary, without fabricating wire bytes.
        XCTAssertThrowsError(try request.acceptOpen(openBytes(direction: .walletToApp)))
        XCTAssertThrowsError(try request.acceptOpen(openBytes(sequence: 0)))
        XCTAssertThrowsError(try request.acceptOpen(openBytes(sequence: 2)))
        let ping = ConnectFrame(sessionID: request.sessionID, direction: .appToWallet,
                                sequence: 1, kind: .control(.ping(ConnectPing(nonce: 1))))
        XCTAssertThrowsError(try request.acceptOpen(ConnectCodec.encode(ping)))
        XCTAssertThrowsError(try request.acceptOpen(Data([1, 2, 3])))
        XCTAssertNoThrow(try request.acceptOpen(openBytes()))
    }

    func testConcurrentNativeOpenHasOneConsumer() throws {
        final class Results: @unchecked Sendable {
            let lock = NSLock()
            var successes = 0
            var failures = 0
            func record(_ success: Bool) {
                lock.lock()
                defer { lock.unlock() }
                if success { successes += 1 } else { failures += 1 }
            }
        }
        let request = try parse()
        let bytes = try openBytes()
        let results = Results()
        DispatchQueue.concurrentPerform(iterations: 16) { _ in
            do { _ = try request.acceptOpen(bytes); results.record(true) }
            catch { results.record(false) }
        }
        XCTAssertEqual(results.successes, 1)
        XCTAssertEqual(results.failures, 15)
    }

    func testApprovalPreimageRequiresOpenAndDelegatesEveryOriginalBinding() throws {
        let request = try parse()
        let fixture = try sharedFixture()
        let approval = try XCTUnwrap(fixture["approval"] as? [String: Any])
        let account = try XCTUnwrap(approval["account_id"] as? String)
        let walletKey = try XCTUnwrap(Data(hexString: try XCTUnwrap(approval["wallet_pk_hex"] as? String)))
        let permissions = ConnectPermissions(methods: ["SIGN_REQUEST_TX"], events: ["DISPLAY_REQUEST"])
        let proof = ConnectSignInProof(domain: "wallet.example", uri: "https://wallet.example",
                                      statement: "Approve", issuedAt: "2026-01-01T00:00:00Z", nonce: "original")
        XCTAssertThrowsError(try request.buildApprovalPreimage(walletPublicKey: walletKey,
            accountID: account, permissions: permissions, proof: proof))
        try request.acceptOpen(openBytes())
        let actual = try request.buildApprovalPreimage(walletPublicKey: walletKey,
            accountID: account, permissions: permissions, proof: proof)
        let expected = try ConnectCrypto.buildApprovalPreimage(networkID: network,
            sessionID: request.sessionID, appPublicKey: appKey, walletPublicKey: walletKey,
            accountID: account, permissions: permissions, proof: proof,
            relayAuthHash: ConnectCrypto.relayAuthHash(sessionID: request.sessionID, relayToken: relay))
        XCTAssertEqual(actual, expected)
        let otherRelay = try parse(uri(["relay": connectBase64URL(Data(repeating: 9, count: 32))]))
        try otherRelay.acceptOpen(openBytes())
        XCTAssertNotEqual(actual, try otherRelay.buildApprovalPreimage(walletPublicKey: walletKey,
            accountID: account, permissions: permissions, proof: proof))
        XCTAssertNotEqual(actual, try request.buildApprovalPreimage(walletPublicKey: walletKey,
            accountID: account, permissions: nil, proof: proof))
        XCTAssertNotEqual(actual, try request.buildApprovalPreimage(walletPublicKey: walletKey,
            accountID: account, permissions: permissions, proof: nil))
        XCTAssertThrowsError(try request.buildApprovalPreimage(walletPublicKey: Data(),
            accountID: account, permissions: permissions, proof: proof))
    }

    func testSignedApprovalWireIsLaunchBoundAndPreparedOnce() throws {
        let request = try parse()
        let signer = try SigningKey.ed25519(privateKey: Data(repeating: 0x44, count: 32))
        let account = try AccountId.makeI105(publicKey: signer.publicKey())
        let walletKey = Data(repeating: 3, count: 32)
        let permission = ConnectPermissions(methods: ["SIGN_REQUEST_TX"])
        let raw = try openBytes()
        var invalid = ConnectApprove(walletPublicKey: walletKey, accountID: account,
            permissions: permission, walletSignature: ConnectWalletSignature(algorithm: "ed25519",
                signature: Data(repeating: 0, count: 64)))
        XCTAssertThrowsError(try request.prepareApproval(invalid))
        try request.acceptOpen(raw)
        let preimage = try request.buildApprovalPreimage(walletPublicKey: walletKey,
            accountID: account, permissions: permission, proof: nil)
        let signature = try signer.sign(preimage)
        let valid = ConnectApprove(walletPublicKey: walletKey, accountID: account,
            permissions: permission,
            walletSignature: ConnectWalletSignature(algorithm: "ed25519", signature: signature))
        XCTAssertThrowsError(try request.prepareApproval(invalid))
        invalid = valid
        invalid.walletPublicKey[0] ^= 1
        XCTAssertThrowsError(try request.prepareApproval(invalid))
        invalid = valid
        invalid.permissions = nil
        XCTAssertThrowsError(try request.prepareApproval(invalid))
        let other = try parse(uri(["relay": connectBase64URL(Data(repeating: 9, count: 32))]))
        try other.acceptOpen(raw)
        XCTAssertThrowsError(try other.prepareApproval(valid))
        let bytes = try request.prepareApproval(valid)
        let decoded = try ConnectCodec.decode(bytes)
        XCTAssertEqual(decoded.sessionID, request.sessionID)
        XCTAssertEqual(decoded.direction, .walletToApp)
        XCTAssertEqual(decoded.sequence, 1)
        XCTAssertEqual(decoded.kind, .control(.approve(valid)))
        XCTAssertThrowsError(try request.prepareApproval(valid))
    }

    func testSharedParserPreservesExactAppAndWalletResponseValidation() throws {
        let sid = try ConnectCrypto.deriveSessionID(networkID: network, appPublicKey: appKey, nonce: nonce)
        let appToken = connectBase64URL(Data(repeating: 6, count: 32))
        var raw: [String: ToriiJSONValue] = [
            "sid": .string(connectBase64URL(sid)), "network_id": .string(network.literal),
            "app_pk": .string(connectBase64URL(appKey)), "nonce": .string(connectBase64URL(nonce)),
            "wallet_uri": .string(try uri()), "app_uri": .string(try uri(["role": "app", "token": appToken])),
            "token_app": .string(appToken), "token_wallet": .string(token),
            "token_management": .string(connectBase64URL(Data(repeating: 5, count: 32))),
            "token_relay": .string(relay)
        ]
        let original = try ToriiConnectSessionResponse(raw: raw)
        XCTAssertEqual(try validateConnectSessionResponse(original, expectedNode: baseURL.absoluteString), original)
        raw["app_uri"] = .string(try uri(["role": "app", "token": token]))
        XCTAssertThrowsError(try validateConnectSessionResponse(ToriiConnectSessionResponse(raw: raw),
                                                                expectedNode: baseURL.absoluteString))
        XCTAssertThrowsError(try validateConnectSessionResponse(original, expectedNode: "https://other.example"))
    }

    func testSharedCryptoVectorRemainsExactWithoutTreatingItsLiteralTokensAsLaunchCredentials() throws {
        let fixture = try sharedFixture()
        let approval = try XCTUnwrap(fixture["approval"] as? [String: Any])
        let tokens = try XCTUnwrap(fixture["tokens"] as? [String: String])
        let sid = try XCTUnwrap(Data(hexString: try XCTUnwrap(fixture["sid_hex"] as? String)))
        let relayHash = try ConnectCrypto.relayAuthHash(sessionID: sid, relayToken: XCTUnwrap(tokens["relay"]))
        XCTAssertEqual(relayHash.hexEncodedString(), fixture["relay_auth_hash_hex"] as? String)
        let preimage = try ConnectCrypto.buildApprovalPreimage(networkID: network, sessionID: sid,
            appPublicKey: appKey, walletPublicKey: XCTUnwrap(Data(hexString: XCTUnwrap(approval["wallet_pk_hex"] as? String))),
            accountID: XCTUnwrap(approval["account_id"] as? String), permissions: nil, proof: nil,
            relayAuthHash: relayHash)
        XCTAssertEqual(preimage.hexEncodedString(), approval["approve_preimage_hex"] as? String)
        XCTAssertThrowsError(try parse(uri(["token": XCTUnwrap(tokens["wallet"]), "relay": XCTUnwrap(tokens["relay"])])))
    }
}
