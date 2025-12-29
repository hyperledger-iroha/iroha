import XCTest
@testable import IrohaSwift

private final class NoritoRpcURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data?))?

    override class func canInit(with request: URLRequest) -> Bool { true }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(self, didFailWithError: NSError(domain: "NoritoRPC", code: -1))
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

@available(iOS 15.0, macOS 12.0, *)
final class NoritoRpcClientTests: XCTestCase {
    override func tearDown() {
        super.tearDown()
        NoritoRpcURLProtocol.handler = nil
    }

    func testCallUsesDefaultHeadersAndPayload() async throws {
        let session = makeSession()
        let client = NoritoRpcClient(baseURL: URL(string: "https://example.org/base")!,
                                     session: session,
                                     defaultHeaders: ["X-Test": "42"])
        let expectation = XCTestExpectation(description: "request observed")
        NoritoRpcURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-norito")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/x-norito")
            XCTAssertEqual(request.value(forHTTPHeaderField: "X-Test"), "42")
            XCTAssertEqual(request.url?.absoluteString, "https://example.org/base/v1/pipeline")
            XCTAssertEqual(self.readBody(request), Data([0xAA, 0xBB]))
            expectation.fulfill()
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!
            return (response, Data([0x01, 0x02]))
        }

        let data = try await client.call(path: "v1/pipeline", payload: Data([0xAA, 0xBB]))
        await fulfillment(of: [expectation], timeout: 1.0)
        XCTAssertEqual(data, Data([0x01, 0x02]))
    }

    func testCallAllowsHeaderOverrides() async throws {
        let session = makeSession()
        let client = NoritoRpcClient(baseURL: URL(string: "https://example.org/api")!,
                                     session: session,
                                     defaultHeaders: ["X-Test": "default", "X-Remove": "gone"])
        let expectation = XCTestExpectation(description: "override observed")
        NoritoRpcURLProtocol.handler = { request in
            XCTAssertNil(request.value(forHTTPHeaderField: "Accept"), "Accept should be removed when nil provided")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/custom")
            XCTAssertEqual(request.value(forHTTPHeaderField: "X-Trace"), "abc")
            XCTAssertNil(request.value(forHTTPHeaderField: "X-Remove"))
            XCTAssertEqual(request.httpMethod, "PUT")
            expectation.fulfill()
            let response = HTTPURLResponse(url: request.url!, statusCode: 204, httpVersion: nil, headerFields: nil)!
            return (response, Data())
        }

        let data = try await client.call(
            path: "/v1/custom",
            payload: Data(),
            method: "put",
            headers: [
                "X-Trace": "abc",
                "x-remove": nil,
                "Content-Type": "application/custom",
                "Accept": nil,
            ],
            accept: nil
        )
        await fulfillment(of: [expectation], timeout: 1.0)
        XCTAssertTrue(data.isEmpty)
    }

    func testCallAppendsQueryParameters() async throws {
        let session = makeSession()
        let client = NoritoRpcClient(baseURL: URL(string: "https://example.org/torii")!,
                                     session: session)
        let expectation = XCTestExpectation(description: "query observed")
        NoritoRpcURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/resource")
            XCTAssertEqual(request.url?.query, "foo=bar%20baz&count=42")
            expectation.fulfill()
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!
            return (response, Data([0xEE]))
        }

        let params = [
            "foo": "bar baz",
            "skip": nil,
            "count": "42",
        ]
        let data = try await client.call(path: "/v1/resource",
                                         payload: Data([0x00]),
                                         params: params)
        await fulfillment(of: [expectation], timeout: 1.0)
        XCTAssertEqual(data, Data([0xEE]))
    }

    func testCallThrowsOnNonSuccessStatus() async {
        let session = makeSession()
        let client = NoritoRpcClient(baseURL: URL(string: "https://example.org")!,
                                     session: session)
        NoritoRpcURLProtocol.handler = { request in
            let response = HTTPURLResponse(url: request.url!, statusCode: 503, httpVersion: nil, headerFields: nil)!
            return (response, Data("gateway maintenance".utf8))
        }

        await assertThrowsErrorAsync(try await client.call(path: "/v1/down", payload: Data())) { error in
            guard let rpcError = error as? NoritoRpcError else {
                XCTFail("Expected NoritoRpcError, got \(error)")
                return
            }
            XCTAssertEqual(rpcError.statusCode, 503)
            XCTAssertEqual(rpcError.body, "gateway maintenance")
        }
    }

    private func makeSession() -> URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [NoritoRpcURLProtocol.self]
        return URLSession(configuration: configuration)
    }

    private func readBody(_ request: URLRequest) -> Data? {
        if let body = request.httpBody {
            return body
        }
        guard let stream = request.httpBodyStream else { return nil }
        stream.open()
        defer { stream.close() }
        var buffer = [UInt8](repeating: 0, count: 1024)
        var data = Data()
        while stream.hasBytesAvailable {
            let read = stream.read(&buffer, maxLength: buffer.count)
            if read > 0 {
                data.append(buffer, count: read)
            } else {
                break
            }
        }
        return data.isEmpty ? nil : data
    }
}

@available(iOS 15.0, macOS 12.0, *)
private func assertThrowsErrorAsync<T>(_ expression: @autoclosure @escaping () async throws -> T,
                                       _ message: @autoclosure () -> String = "",
                                       file: StaticString = #filePath,
                                       line: UInt = #line,
                                       _ errorHandler: (_ error: Error) -> Void) async {
    do {
        _ = try await expression()
        XCTFail(message(), file: file, line: line)
    } catch {
        errorHandler(error)
    }
}
