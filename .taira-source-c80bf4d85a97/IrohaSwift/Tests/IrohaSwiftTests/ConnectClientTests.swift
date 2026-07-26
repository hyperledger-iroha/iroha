import XCTest
@testable import IrohaSwift

final class ConnectClientTests: XCTestCase {
    private final class RequestBox: @unchecked Sendable {
        var request: URLRequest?
    }

    private func requireConnectCodec() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.isConnectCodecAvailable,
                      "NoritoBridge connect codec unavailable")
    }

    func testStartResumesOnlyOnce() async {
        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()
        await client.start()
        XCTAssertEqual(stub.resumeCallCount, 1)
    }

    func testSendStoresDataOnStub() async throws {
        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()
        try await client.send(data: Data([0x01, 0x02]))
        XCTAssertEqual(stub.sentData, [Data([0x01, 0x02])])
    }

    func testReceiveSuspendsUntilDataArrives() async throws {
        try requireConnectCodec()
        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()

        let expectation = expectation(description: "receive")
        Task {
            let frame = try await client.receiveFrame()
            XCTAssertEqual(frame.sequence, 9)
            expectation.fulfill()
        }

        // Wait for the receive continuation to register.
        while stub.pendingReceives.isEmpty {
            try await Task.sleep(nanoseconds: 1_000_000)
        }
        let sessionID = Data(repeating: 0x11, count: 32)
        let frame = ConnectFrame(sessionID: sessionID,
                                 direction: .walletToApp,
                                 sequence: 9,
                                 kind: .control(.ping(ConnectPing(nonce: 1))))
        let encoded = try ConnectCodec.encode(frame)
        stub.emit(encoded)

        await fulfillment(of: [expectation], timeout: 1)
    }

    func testSendFrameUsesCoder() async throws {
        try requireConnectCodec()
        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()
        let appPublicKey = Data(repeating: 0xFF, count: 32)
        let sessionID = Data(repeating: 0x22, count: 32)
        let open = ConnectOpen(appPublicKey: appPublicKey,
                               appMetadata: ConnectAppMetadata(name: "demo", iconURL: nil, description: nil),
                               constraints: ConnectConstraints(chainID: "chain"),
                               permissions: ConnectPermissions(methods: ["sign"]))
        let frame = ConnectFrame(sessionID: sessionID,
                                 direction: .appToWallet,
                                 sequence: 1,
                                 kind: .control(.open(open)))
        try await client.send(frame: frame)
        let decoded = try XCTUnwrap(stub.sentData.first.flatMap { try? ConnectCodec.decode($0) })
        XCTAssertEqual(decoded.sessionID, sessionID)
        XCTAssertEqual(decoded.direction, .appToWallet)
        if case let .control(.open(decodedOpen)) = decoded.kind {
            XCTAssertEqual(decodedOpen.appPublicKey, appPublicKey)
            XCTAssertEqual(decodedOpen.constraints.chainID, "chain")
        } else {
            XCTFail("Expected open frame")
        }
    }

    func testCloseCancelsUnderlyingTask() async {
        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()
        await client.close(code: .goingAway, reason: Data("bye".utf8))
        XCTAssertEqual(stub.cancelled?.code, .goingAway)
        XCTAssertEqual(stub.cancelled?.reason, Data("bye".utf8))
    }

    func testSendFailsAfterClose() async {
        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()
        await client.close()

        await XCTAssertThrowsErrorAsync(try await client.send(data: Data([0xAA]))) { error in
            guard case ConnectClient.ClientError.closed = error else {
                return XCTFail("expected ConnectClient closed error, got \(error)")
            }
        }
    }

    func testReceiveFailsAfterClose() async {
        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()
        await client.close()

        await XCTAssertThrowsErrorAsync(try await client.receive()) { error in
            guard case ConnectClient.ClientError.closed = error else {
                return XCTFail("expected ConnectClient closed error, got \(error)")
            }
        }
    }

    func testBuildsConnectWebSocketURL() throws {
        let base = URL(string: "https://torii.example/api")!
        let url = try ConnectClient.makeWebSocketURL(baseURL: base,
                                                     sid: "session-123",
                                                     role: .app)
        let components = try XCTUnwrap(URLComponents(url: url, resolvingAgainstBaseURL: false))
        XCTAssertEqual(components.scheme, "wss")
        XCTAssertEqual(components.host, "torii.example")
        XCTAssertEqual(components.path, "/api/v1/connect/ws")
        let items = try XCTUnwrap(components.queryItems)
        XCTAssertTrue(items.contains(where: { $0.name == "sid" && $0.value == "session-123" }))
        XCTAssertTrue(items.contains(where: { $0.name == "role" && $0.value == "app" }))
        XCTAssertFalse(items.contains(where: { $0.name == "token" }))
    }

    func testBuildsConnectWebSocketURLRejectsEmptySid() {
        XCTAssertThrowsError(try ConnectClient.makeWebSocketURL(baseURL: URL(string: "http://localhost")!,
                                                                sid: " ",
                                                                role: .wallet)) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("expected invalidPayload, got \(error)")
            }
        }
    }

    func testBuildsConnectWebSocketRequestAddsAuthorization() throws {
        let base = URL(string: "https://torii.example")!
        let request = try ConnectClient.makeWebSocketRequest(baseURL: base,
                                                             sid: "session-abc",
                                                             role: .wallet,
                                                             token: "token-xyz")
        let auth = request.value(forHTTPHeaderField: "Authorization")
        XCTAssertEqual(auth, "Bearer token-xyz")
    }

    func testBuildsConnectWebSocketRequestRejectsEmptyToken() {
        XCTAssertThrowsError(try ConnectClient.makeWebSocketRequest(baseURL: URL(string: "http://localhost")!,
                                                                    sid: "session-123",
                                                                    role: .app,
                                                                    token: " ")) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("expected invalidPayload, got \(error)")
            }
        }
    }

    func testBuildsConnectWebSocketRequestRejectsInsecureTransport() {
        XCTAssertThrowsError(try ConnectClient.makeWebSocketRequest(baseURL: URL(string: "http://localhost")!,
                                                                    sid: "session-123",
                                                                    role: .app,
                                                                    token: "token-xyz")) { error in
            guard case let ToriiClientError.invalidPayload(reason) = error else {
                return XCTFail("expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(reason.contains("refuses insecure WebSocket protocol"))
        }
    }

    func testInitWithRequestUsesFactory() {
        let box = RequestBox()
        let stub = StubWebSocketTask()
        let factory = ConnectWebSocketFactory { request in
            box.request = request
            return stub
        }
        let url = URL(string: "wss://example.test")!
        var request = URLRequest(url: url)
        request.setValue("Bearer token-123", forHTTPHeaderField: "Authorization")

        _ = ConnectClient(request: request, webSocketFactory: factory)

        XCTAssertEqual(box.request?.url, url)
        XCTAssertEqual(box.request?.value(forHTTPHeaderField: "Authorization"), "Bearer token-123")
    }
}
