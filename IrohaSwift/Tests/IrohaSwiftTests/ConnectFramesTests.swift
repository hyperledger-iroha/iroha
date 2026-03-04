import XCTest
@testable import IrohaSwift

final class ConnectFramesTests: XCTestCase {
    private func requireConnectCodec() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.isConnectCodecAvailable,
                      "NoritoBridge connect codec unavailable")
    }

    func testFrameRoundTrip() throws {
        try requireConnectCodec()
        let frame = ConnectFrame(sessionID: Data(repeating: 0x01, count: 32),
                                 direction: .appToWallet,
                                 sequence: 42,
                                 kind: .ciphertext(ConnectCiphertext(payload: Data([0xAA]))))
        let encoded = try ConnectCodec.encode(frame)
        let decoded = try ConnectCodec.decode(encoded)
        XCTAssertEqual(decoded.sessionID, frame.sessionID)
        XCTAssertEqual(decoded.direction, frame.direction)
        XCTAssertEqual(decoded.sequence, frame.sequence)
        if case let .ciphertext(ct) = decoded.kind {
            XCTAssertEqual(ct.payload, Data([0xAA]))
        } else {
            XCTFail("expected ciphertext")
        }
    }

    func testOpenFrameRoundTrip() throws {
        try requireConnectCodec()
        let open = ConnectOpen(appPublicKey: Data(repeating: 0x11, count: 32),
                               appMetadata: ConnectAppMetadata(name: "demo", iconURL: nil, description: nil),
                               constraints: ConnectConstraints(chainID: "chain"),
                               permissions: ConnectPermissions(methods: ["sign"]))
        let frame = ConnectFrame(sessionID: Data(repeating: 0x22, count: 32),
                                 direction: .appToWallet,
                                 sequence: 1,
                                 kind: .control(.open(open)))
        let encoded = try ConnectCodec.encode(frame)
        let decoded = try ConnectCodec.decode(encoded)
        XCTAssertEqual(decoded.sessionID, frame.sessionID)
        XCTAssertEqual(decoded.direction, frame.direction)
        XCTAssertEqual(decoded.sequence, frame.sequence)
        if case let .control(.open(decodedOpen)) = decoded.kind {
            XCTAssertEqual(decodedOpen, open)
        } else {
            XCTFail("expected open frame")
        }
    }

    func testPermissionsJSONRoundTrip() throws {
        let permissions = ConnectPermissions(methods: ["sign"], events: ["event"], resources: ["accounts"])
        let data = ConnectCodec.encodePermissionsJSON(permissions)
        let decoded = try data.flatMap { try ConnectCodec.decodePermissionsJSON($0) }
        XCTAssertEqual(decoded, permissions)
    }

    func testPermissionsJSONAllowsMissingEvents() throws {
        let json: [String: Any] = [
            "methods": ["sign"]
        ]
        let data = try JSONSerialization.data(withJSONObject: json, options: [])
        let decoded = try ConnectCodec.decodePermissionsJSON(data)
        XCTAssertEqual(decoded, ConnectPermissions(methods: ["sign"], events: [], resources: nil))
    }

    func testProofJSONRoundTrip() throws {
        let proof = ConnectSignInProof(domain: "example.com",
                                       uri: "https://example.com",
                                       statement: "Sign in",
                                       issuedAt: "now",
                                       nonce: "123")
        let data = ConnectCodec.encodeProofJSON(proof)
        let decoded = try data.flatMap { try ConnectCodec.decodeProofJSON($0) }
        XCTAssertEqual(decoded, proof)
    }

    func testRejectFrameRoundTrip() throws {
        try requireConnectCodec()
        let frame = ConnectFrame(sessionID: Data(repeating: 0xAA, count: 32),
                                 direction: .walletToApp,
                                 sequence: 7,
                                 kind: .control(.reject(ConnectReject(code: 42,
                                                                      codeID: "USER_DENIED",
                                                                      reason: "nope"))))
        let encoded = try ConnectCodec.encode(frame)
        let decoded = try ConnectCodec.decode(encoded)
        XCTAssertEqual(decoded.kind, frame.kind)
    }

    func testCloseFrameRoundTrip() throws {
        try requireConnectCodec()
        let close = ConnectClose(role: .app, code: 1000, reason: "done", retryable: false)
        let frame = ConnectFrame(sessionID: Data(repeating: 0xBB, count: 32),
                                 direction: .appToWallet,
                                 sequence: 9,
                                 kind: .control(.close(close)))
        let encoded = try ConnectCodec.encode(frame)
        let decoded = try ConnectCodec.decode(encoded)
        XCTAssertEqual(decoded.kind, frame.kind)
    }

    func testPingPongRoundTrip() throws {
        try requireConnectCodec()
        let ping = ConnectFrame(sessionID: Data(repeating: 0xCC, count: 32),
                                direction: .appToWallet,
                                sequence: 11,
                                kind: .control(.ping(ConnectPing(nonce: 55))))
        let pong = ConnectFrame(sessionID: Data(repeating: 0xCC, count: 32),
                                direction: .walletToApp,
                                sequence: 12,
                                kind: .control(.pong(ConnectPong(nonce: 56))))
        XCTAssertEqual(try ConnectCodec.decode(ConnectCodec.encode(ping)).kind, ping.kind)
        XCTAssertEqual(try ConnectCodec.decode(ConnectCodec.encode(pong)).kind, pong.kind)
    }

    func testPermissionsJSONRejectsUnknownKey() throws {
        let json: [String: Any] = [
            "methods": ["sign"],
            "events": ["event"],
            "extra": "nope"
        ]
        let data = try JSONSerialization.data(withJSONObject: json, options: [])
        XCTAssertThrowsError(try ConnectCodec.decodePermissionsJSON(data))
    }

    func testPermissionsJSONRejectsNonStringValues() throws {
        let json: [String: Any] = [
            "methods": ["sign", 1],
            "events": ["event"]
        ]
        let data = try JSONSerialization.data(withJSONObject: json, options: [])
        XCTAssertThrowsError(try ConnectCodec.decodePermissionsJSON(data))
    }

    func testAppMetadataJSONRejectsUnknownKey() throws {
        let json: [String: Any] = [
            "name": "Demo",
            "url": "https://example.com",
            "extra": "nope"
        ]
        let data = try JSONSerialization.data(withJSONObject: json, options: [])
        XCTAssertThrowsError(try ConnectCodec.decodeAppMetadataJSON(data))
    }

    func testAppMetadataJSONAcceptsIconUrlAlias() throws {
        let json: [String: Any] = [
            "name": "Demo",
            "icon_url": "https://example.com/icon.png"
        ]
        let data = try JSONSerialization.data(withJSONObject: json, options: [])
        let decoded = try ConnectCodec.decodeAppMetadataJSON(data)
        XCTAssertEqual(decoded?.name, "Demo")
        XCTAssertEqual(decoded?.iconURL, "https://example.com/icon.png")
    }

    func testAppMetadataJSONAllowsIconHash() throws {
        let json: [String: Any] = [
            "name": "Demo",
            "icon_hash": "abc123"
        ]
        let data = try JSONSerialization.data(withJSONObject: json, options: [])
        let decoded = try ConnectCodec.decodeAppMetadataJSON(data)
        XCTAssertEqual(decoded?.name, "Demo")
        XCTAssertNil(decoded?.iconURL)
        XCTAssertEqual(decoded?.iconHash, "abc123")
    }

    func testAppMetadataJSONEncodesDescription() throws {
        let metadata = ConnectAppMetadata(name: nil, iconURL: nil, description: "Hello")
        let data = ConnectCodec.encodeAppMetadataJSON(metadata)
        let payload = try XCTUnwrap(data)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: payload) as? [String: Any])
        XCTAssertEqual(json["description"] as? String, "Hello")
        XCTAssertNil(json["name"])
        XCTAssertNil(json["url"])
    }

    func testAppMetadataJSONEncodesIconHash() throws {
        let metadata = ConnectAppMetadata(name: "Demo",
                                          iconURL: nil,
                                          description: nil,
                                          iconHash: "abc123")
        let data = ConnectCodec.encodeAppMetadataJSON(metadata)
        let payload = try XCTUnwrap(data)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: payload) as? [String: Any])
        XCTAssertEqual(json["name"] as? String, "Demo")
        XCTAssertEqual(json["icon_hash"] as? String, "abc123")
        XCTAssertNil(json["url"])
    }

    func testProofJSONRejectsNonStringValue() throws {
        let json: [String: Any] = [
            "domain": 1
        ]
        let data = try JSONSerialization.data(withJSONObject: json, options: [])
        XCTAssertThrowsError(try ConnectCodec.decodeProofJSON(data))
    }
}

final class ConnectCodecBridgeAvailabilityTests: XCTestCase {
    override func tearDown() {
        NoritoNativeBridge.shared.overrideConnectCodecAvailabilityForTests(nil)
        super.tearDown()
    }

    func testEncodeThrowsWhenBridgeDisabled() {
        NoritoNativeBridge.shared.overrideConnectCodecAvailabilityForTests(false)
        let frame = ConnectFrame(sessionID: Data(repeating: 0x11, count: 32),
                                 direction: .appToWallet,
                                 sequence: 0,
                                 kind: .control(.ping(ConnectPing(nonce: 1))))
        XCTAssertThrowsError(try ConnectCodec.encode(frame)) { error in
            XCTAssertEqual(error as? ConnectCodecError, .bridgeUnavailable)
        }
    }

    func testDecodeThrowsWhenBridgeDisabled() {
        NoritoNativeBridge.shared.overrideConnectCodecAvailabilityForTests(false)
        XCTAssertThrowsError(try ConnectCodec.decode(Data())) { error in
            XCTAssertEqual(error as? ConnectCodecError, .bridgeUnavailable)
        }
    }

    func testDecodeFailsOnInvalidBytes() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.isConnectCodecAvailable,
                      "NoritoBridge connect codec unavailable")
        let sessionID = Data(repeating: 0x22, count: 32)
        let frame = ConnectFrame(sessionID: sessionID,
                                 direction: .appToWallet,
                                 sequence: 1,
                                 kind: .control(.ping(ConnectPing(nonce: 1))))
        let encoded = try ConnectCodec.encode(frame)
        let truncated = encoded.prefix(max(encoded.count / 2, 1))
        XCTAssertThrowsError(try ConnectCodec.decode(Data(truncated))) { error in
            XCTAssertEqual(error as? ConnectCodecError, .decodeFailed)
        }
    }
}
