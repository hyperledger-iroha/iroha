import XCTest
@testable import IrohaSwift

final class ConnectSessionTests: XCTestCase {
    private func requireConnectCodec() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.isConnectCodecAvailable,
                      "NoritoBridge connect codec unavailable")
    }

    func testSendOpenSendsControlFrame() async throws {
        try requireConnectCodec()
        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()
        let sessionID = Data(repeating: 0x01, count: 32)
        let session = ConnectSession(sessionID: sessionID, client: client)
        let open = ConnectOpen(appPublicKey: Data(repeating: 0x02, count: 32),
                               appMetadata: ConnectAppMetadata(name: "demo", iconURL: nil, description: nil),
                               constraints: ConnectConstraints(chainID: "chain"),
                               permissions: ConnectPermissions(methods: ["sign"]))
        try await session.sendOpen(open: open)
        let encoded = try XCTUnwrap(stub.sentData.first)
        let decoded = try ConnectCodec.decode(encoded)
        XCTAssertEqual(decoded.sessionID, sessionID)
        XCTAssertEqual(decoded.sequence, 1)
        if case let .control(.open(decodedOpen)) = decoded.kind {
            XCTAssertEqual(decodedOpen, open)
        } else {
            XCTFail("Expected control frame")
        }
    }

    func testNextControlFrameReturnsCounterpartyFrame() async throws {
        try requireConnectCodec()
        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()
        let sessionID = Data(repeating: 0xAA, count: 32)
        let session = ConnectSession(sessionID: sessionID, client: client)

        Task {
            while stub.pendingReceives.isEmpty {
                try await Task.sleep(nanoseconds: 1_000_000)
            }
            let accountID = AccountId.make(publicKey: Data(repeating: 0x11, count: 32),
                                            domain: "wonderland")
            let approve = ConnectApprove(walletPublicKey: Data(repeating: 0xBB, count: 32),
                                         accountID: accountID,
                                         permissions: nil,
                                         proof: nil,
                                         walletSignature: ConnectWalletSignature(algorithm: "ed25519",
                                                                                 signature: Data(repeating: 0x99,
                                                                                                 count: 64)),
                                         walletMetadata: nil)
            let frame = ConnectFrame(sessionID: sessionID,
                                     direction: .walletToApp,
                                     sequence: 5,
                                     kind: .control(.approve(approve)))
            let encoded = try ConnectCodec.encode(frame)
            stub.emit(encoded)
        }

        let control = try await session.nextControlFrame()
        if case .approve(let approve) = control {
            XCTAssertEqual(approve.accountID, AccountId.make(publicKey: Data(repeating: 0x11, count: 32),
                                                             domain: "wonderland"))
        } else {
            XCTFail("expected approve frame")
        }
    }

    func testNextControlFrameDecryptsCiphertextControl() async throws {
        XCTAssertTrue(NoritoNativeBridge.shared.isConnectCryptoAvailable)

        let key = Data(repeating: 0xEF, count: 32)
        let appKey = Data(repeating: 0xAA, count: 32)
        let sessionID = Data(repeating: 0x44, count: 32)

        guard let encrypted = makeEncryptedCloseFrame(key: key,
                                                      sessionID: sessionID,
                                                      direction: .walletToApp,
                                                      sequence: 7,
                                                      reason: "shutdown") else {
            throw XCTSkip("NoritoBridge encryption helpers unavailable")
        }

        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()

        let keys = ConnectDirectionKeys(appToWallet: appKey, walletToApp: key)
        let session = ConnectSession(sessionID: sessionID, client: client, directionKeys: keys)

        Task {
            while stub.pendingReceives.isEmpty {
                try await Task.sleep(nanoseconds: 1_000_000)
            }
            stub.emit(encrypted)
        }

        let control = try await session.nextControlFrame()
        if case .close(let close) = control {
            XCTAssertEqual(close.reason, "shutdown")
            XCTAssertEqual(close.role, .wallet)
        } else {
            XCTFail("expected decrypted close control")
        }
    }

    func testNextEnvelopeDecryptsCiphertextFrame() async throws {
        XCTAssertTrue(NoritoNativeBridge.shared.isConnectCryptoAvailable)

        let key = Data(repeating: 0x11, count: 32)
        let sessionID = Data(repeating: 0x33, count: 32)
        let keys = ConnectDirectionKeys(appToWallet: Data(repeating: 0xAA, count: 32), walletToApp: key)

        guard let encrypted = makeEncryptedCloseFrame(key: key,
                                                      sessionID: sessionID,
                                                      direction: .walletToApp,
                                                      sequence: 9,
                                                      reason: "done") else {
            throw XCTSkip("NoritoBridge encryption helpers unavailable")
        }

        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()

        let session = ConnectSession(sessionID: sessionID, client: client, directionKeys: keys)

        Task {
            while stub.pendingReceives.isEmpty {
                try await Task.sleep(nanoseconds: 1_000_000)
            }
            stub.emit(encrypted)
        }

        let envelope = try await session.nextEnvelope()
        XCTAssertEqual(envelope.sequence, 9)
        if case .controlClose(let close) = envelope.payload {
            XCTAssertEqual(close.reason, "done")
        } else {
            XCTFail("Expected control close payload")
        }
    }

    func testNextEnvelopeThrowsWithoutKeys() async throws {
        XCTAssertTrue(NoritoNativeBridge.shared.isConnectCryptoAvailable)

        let key = Data(repeating: 0x22, count: 32)
        let sessionID = Data(repeating: 0x55, count: 32)

        guard let encrypted = makeEncryptedCloseFrame(key: key,
                                                      sessionID: sessionID,
                                                      direction: .walletToApp,
                                                      sequence: 11,
                                                      reason: "shutdown") else {
            throw XCTSkip("NoritoBridge encryption helpers unavailable")
        }

        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()
        let session = ConnectSession(sessionID: sessionID, client: client)

        Task {
            while stub.pendingReceives.isEmpty {
                try await Task.sleep(nanoseconds: 1_000_000)
            }
            stub.emit(encrypted)
        }

        await XCTAssertThrowsErrorAsync(try await session.nextEnvelope()) { error in
            guard case ConnectSessionError.missingDecryptionKeys = error else {
                return XCTFail("expected missingDecryptionKeys error")
            }
        }
    }
}

private func makeEncryptedCloseFrame(key: Data,
                                     sessionID: Data,
                                     direction: ConnectDirection,
                                     sequence: UInt64,
                                     reason: String) -> Data? {
    guard let envelope = NoritoNativeBridge.shared.encodeEnvelopeControlClose(sequence: sequence,
                                                                              who: .wallet,
                                                                              code: 1000,
                                                                              reason: reason,
                                                                              retryable: false) else {
        return nil
    }
    guard NoritoNativeBridge.shared.decodeEnvelopeJSON(envelope) != nil else {
        return nil
    }
    return NoritoNativeBridge.shared.connectEncryptEnvelope(key: key,
                                                            sessionID: sessionID,
                                                            direction: direction,
                                                            envelope: envelope)
}
