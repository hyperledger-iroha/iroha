import XCTest
@testable import IrohaSwift

final class ConnectSessionTests: XCTestCase {
    private func requireConnectCodec() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isConnectCodecAvailable,
            "NoritoBridge connect codec unavailable"
        )
    }

    private func requireConnectCrypto() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isConnectCryptoAvailable,
            "NoritoBridge connect crypto unavailable"
        )
    }

    func testSendOpenSendsControlFrame() async throws {
        try requireConnectCodec()
        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()
        let appPublicKey = Data(repeating: 0x02, count: 32)
        let nonce = Data(repeating: 0x03, count: 16)
        let sessionID = try ConnectCrypto.deriveSessionID(
            networkID: TestNetworkIds.canonical,
            appPublicKey: appPublicKey,
            nonce: nonce
        )
        let session = try ConnectSession(networkID: TestNetworkIds.canonical,
                                         appPublicKey: appPublicKey,
                                         nonce: nonce,
                                         relayToken: "relay-token",
                                         client: client)
        let open = ConnectOpen(appPublicKey: appPublicKey,
                               appMetadata: ConnectAppMetadata(name: "demo", iconURL: nil, description: nil),
                               constraints: ConnectConstraints(networkID: TestNetworkIds.canonical),
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
        await XCTAssertThrowsErrorAsync(try await session.sendOpen(open: open)) { error in
            guard case ConnectSessionError.protocolViolation = error else {
                return XCTFail("expected duplicate Open rejection")
            }
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
        let accountPublicKey = try Keypair(
            privateKeyBytes: Data(repeating: 0x11, count: 32)
        ).publicKey

        let emitTask = Task {
            while stub.pendingReceives.isEmpty {
                try await Task.sleep(nanoseconds: 1_000_000)
            }
            let accountID = AccountId.make(publicKey: accountPublicKey)
            let walletSignature = try Self.validEd25519Signature(message: "connect approve")
            let approve = ConnectApprove(walletPublicKey: Data(repeating: 0xBB, count: 32),
                                         accountID: accountID,
                                         permissions: nil,
                                         proof: nil,
                                         walletSignature: ConnectWalletSignature(algorithm: "ed25519",
                                                                                 signature: walletSignature),
                                         walletMetadata: nil)
            let frame = ConnectFrame(sessionID: sessionID,
                                     direction: .walletToApp,
                                     sequence: 5,
                                     kind: .control(.approve(approve)))
            let encoded = try ConnectCodec.encode(frame)
            stub.emit(encoded)
        }

        let control = try await session.nextControlFrame()
        try await emitTask.value
        if case .approve(let approve) = control {
            XCTAssertEqual(approve.accountID, AccountId.make(publicKey: accountPublicKey))
        } else {
            XCTFail("expected approve frame")
        }
    }

    func testSendOpenRejectsNetworkAndAppKeySubstitution() async throws {
        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        let appPublicKey = Data(repeating: 0x41, count: 32)
        let session = try ConnectSession(
            networkID: TestNetworkIds.canonical,
            appPublicKey: appPublicKey,
            nonce: Data(repeating: 0x42, count: 16),
            relayToken: "relay-token",
            client: client
        )
        let wrongNetwork = ConnectOpen(
            appPublicKey: appPublicKey,
            appMetadata: nil,
            constraints: ConnectConstraints(networkID: TestNetworkIds.other),
            permissions: nil
        )
        await XCTAssertThrowsErrorAsync(try await session.sendOpen(open: wrongNetwork)) { error in
            guard case ConnectSessionError.protocolViolation = error else {
                return XCTFail("expected wrong-network rejection")
            }
        }
        var wrongAppKey = appPublicKey
        wrongAppKey[0] ^= 1
        let wrongApp = ConnectOpen(
            appPublicKey: wrongAppKey,
            appMetadata: nil,
            constraints: ConnectConstraints(networkID: TestNetworkIds.canonical),
            permissions: nil
        )
        await XCTAssertThrowsErrorAsync(try await session.sendOpen(open: wrongApp)) { error in
            guard case ConnectSessionError.protocolViolation = error else {
                return XCTFail("expected app-key substitution rejection")
            }
        }
        XCTAssertTrue(stub.sentData.isEmpty)
    }

    private static func validEd25519Signature(message: String) throws -> Data {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x63, count: 32))
        return try signingKey.sign(Data(message.utf8))
    }

    func testNextControlFrameDecryptsCiphertextControl() async throws {
        try requireConnectCrypto()

        let key = Data(repeating: 0xEF, count: 32)
        let appKey = Data(repeating: 0xAA, count: 32)
        let sessionID = Data(repeating: 0x44, count: 32)

        guard let encrypted = makeEncryptedCloseFrame(key: key,
                                                      sessionID: sessionID,
                                                      direction: .walletToApp,
                                                      sequence: 7,
                                                      reason: "shutdown") else {
            try failRequiredNativeTestCapability(
                "NoritoBridge encryption helpers unavailable"
            )
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
        try requireConnectCrypto()

        let key = Data(repeating: 0x11, count: 32)
        let sessionID = Data(repeating: 0x33, count: 32)
        let keys = ConnectDirectionKeys(appToWallet: Data(repeating: 0xAA, count: 32), walletToApp: key)

        guard let encrypted = makeEncryptedCloseFrame(key: key,
                                                      sessionID: sessionID,
                                                      direction: .walletToApp,
                                                      sequence: 9,
                                                      reason: "done") else {
            try failRequiredNativeTestCapability(
                "NoritoBridge encryption helpers unavailable"
            )
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
        try requireConnectCrypto()

        let key = Data(repeating: 0x22, count: 32)
        let sessionID = Data(repeating: 0x55, count: 32)

        guard let encrypted = makeEncryptedCloseFrame(key: key,
                                                      sessionID: sessionID,
                                                      direction: .walletToApp,
                                                      sequence: 11,
                                                      reason: "shutdown") else {
            try failRequiredNativeTestCapability(
                "NoritoBridge encryption helpers unavailable"
            )
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

    func testSendCloseIsIdempotentUnderConcurrency() async throws {
        try requireConnectCodec()
        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()
        let session = ConnectSession(sessionID: Data(repeating: 0xAB, count: 32), client: client)

        try await withThrowingTaskGroup(of: Void.self) { group in
            for _ in 0..<8 {
                group.addTask {
                    try await session.sendClose()
                }
            }
            try await group.waitForAll()
        }

        XCTAssertEqual(stub.sentData.count, 1, "only one close frame should be emitted")
        let frameData = try XCTUnwrap(stub.sentData.first)
        let decoded = try ConnectCodec.decode(frameData)
        if case let .control(.close(close)) = decoded.kind {
            XCTAssertEqual(close.code, 1000)
            XCTAssertEqual(close.role, .app)
        } else {
            XCTFail("expected close control frame")
        }
    }

    func testMissingKeysDoesNotConsumeFlowControlToken() async throws {
        try requireConnectCrypto()

        let walletKey = Data(repeating: 0x61, count: 32)
        let appKey = Data(repeating: 0x62, count: 32)
        let sessionID = Data(repeating: 0x73, count: 32)
        guard let encrypted = makeEncryptedCloseFrame(key: walletKey,
                                                      sessionID: sessionID,
                                                      direction: .walletToApp,
                                                      sequence: 21,
                                                      reason: "missing-key") else {
            try failRequiredNativeTestCapability(
                "NoritoBridge encryption helpers unavailable"
            )
        }

        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()
        let session = ConnectSession(sessionID: sessionID,
                                     client: client,
                                     flowControl: ConnectFlowControlWindow(appToWallet: 1,
                                                                           walletToApp: 1))

        stub.emit(encrypted)
        await XCTAssertThrowsErrorAsync(try await session.nextEnvelope()) { error in
            guard case ConnectSessionError.missingDecryptionKeys = error else {
                return XCTFail("expected missingDecryptionKeys error")
            }
        }

        session.setDirectionKeys(ConnectDirectionKeys(appToWallet: appKey, walletToApp: walletKey))
        stub.emit(encrypted)
        let envelope = try await session.nextEnvelope()
        XCTAssertEqual(envelope.sequence, 21)
    }

    func testDecryptFailureDoesNotConsumeFlowControlToken() async throws {
        try requireConnectCrypto()

        let correctWalletKey = Data(repeating: 0x71, count: 32)
        let wrongWalletKey = Data(repeating: 0x72, count: 32)
        let appKey = Data(repeating: 0x73, count: 32)
        let sessionID = Data(repeating: 0x74, count: 32)
        guard let encrypted = makeEncryptedCloseFrame(key: correctWalletKey,
                                                      sessionID: sessionID,
                                                      direction: .walletToApp,
                                                      sequence: 34,
                                                      reason: "retry-decrypt") else {
            try failRequiredNativeTestCapability(
                "NoritoBridge encryption helpers unavailable"
            )
        }

        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()
        let session = ConnectSession(sessionID: sessionID,
                                     client: client,
                                     directionKeys: ConnectDirectionKeys(appToWallet: appKey,
                                                                         walletToApp: wrongWalletKey),
                                     flowControl: ConnectFlowControlWindow(appToWallet: 1,
                                                                           walletToApp: 1))

        stub.emit(encrypted)
        await XCTAssertThrowsErrorAsync(try await session.nextEnvelope()) { error in
            guard error is ConnectEnvelopeError else {
                return XCTFail("expected envelope error from decryption failure")
            }
        }

        session.setDirectionKeys(ConnectDirectionKeys(appToWallet: appKey, walletToApp: correctWalletKey))
        stub.emit(encrypted)
        let envelope = try await session.nextEnvelope()
        XCTAssertEqual(envelope.sequence, 34)
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
