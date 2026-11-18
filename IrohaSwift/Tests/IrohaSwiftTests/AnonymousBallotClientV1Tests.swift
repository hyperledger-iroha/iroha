import Foundation
import XCTest
@testable import IrohaSwift

private func fixture(_ name: String) throws -> Data {
    let root = (0..<4).reduce(URL(fileURLWithPath: #filePath)) { url, _ in url.deletingLastPathComponent() }
    return try Data(contentsOf: root.appendingPathComponent("fixtures/governance/anonymous_ballot_http_v1/" + name))
}

private final class BallotProtocol: URLProtocol, @unchecked Sendable {
    final class Capture: @unchecked Sendable {
        private let lock = NSLock()
        private var requests: [(URLRequest, Data)] = []
        func add(_ request: URLRequest, body: Data) { lock.withLock { requests.append((request, body)) } }
        func take() -> [(URLRequest, Data)] { lock.withLock { defer { requests.removeAll() }; return requests } }
    }
    static let capture = Capture()
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        var body = request.httpBody ?? Data()
        if let input = request.httpBodyStream {
            input.open()
            defer { input.close() }
            var buffer = [UInt8](repeating: 0, count: 8192)
            while true {
                let count = input.read(&buffer, maxLength: buffer.count)
                if count < 0 { client?.urlProtocol(self, didFailWithError: input.streamError!); return }
                if count == 0 { break }
                body.append(contentsOf: buffer.prefix(count))
            }
        }
        Self.capture.add(request, body: body)
        let url = request.url!
        let mode = url.pathComponents[1]
        let post = request.httpMethod == "POST"
        var bytes: Data
        do { bytes = post ? Data() : try fixture(url.path.contains("/boards/") ? "board.nrt" : "attempt.nrt") }
        catch { client?.urlProtocol(self, didFailWithError: error); return }
        var headers = ["Content-Type": "application/x-norito", "Set-Cookie": "identity=never"]
        var status = post ? 202 : 200
        switch mode {
        case "compressed", "oversized":
            status = 503
            headers[mode == "compressed" ? "Content-Encoding" : "Content-Length"] = mode == "compressed" ? "gzip" : "65537"
            headers["Retry-After"] = "7"
        case "refusal":
            status = 429
            bytes = Data(#"{"code":"anonymous_ballot_throttled","message":"remote text"}"#.utf8)
            headers["Retry-After"] = "3"
            headers["Content-Type"] = "application/json"
        case "redirect": status = 307; headers["Location"] = "/unexpected"
        case "wrong": status = 200; headers["Content-Type"] = "application/json"
        case "body": bytes = Data([1])
        case "empty": bytes = Data()
        case "large": headers["Content-Length"] = "1048577"
        default: break
        }
        let response = HTTPURLResponse(url: url, statusCode: status, httpVersion: "HTTP/1.1", headerFields: headers)!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: bytes)
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}

final class AnonymousBallotClientV1Tests: XCTestCase {
    func testWholeRecordsAndCredentialFreeSubmission() async throws {
        func transport(_ mode: String) throws -> AnonymousBallotClientV1 {
            try AnonymousBallotClientV1(baseURL: URL(string: "https://ballot.invalid/" + mode)!,
                                        allowLocalHTTP: false, protocolClasses: [BallotProtocol.self])
        }
        func fails(_ action: () async throws -> Void) async {
            do { try await action(); XCTFail("Expected a transport refusal") } catch {}
        }
        func bytes(_ hex: String) -> Data {
            var digits = hex[...], result = Data()
            while !digits.isEmpty { result.append(UInt8(digits.prefix(2), radix: 16)!); digits = digits.dropFirst(2) }
            return result
        }
        let scopeHex = "856bb8bcfa0804b4df16c68d0aa758937016ac87a3e783fa4b434ebeb5f4cd584e4abe3bf0b859b89863fb653f620aba7b7ea2ac4343c8723a7021a15cc01d2d"
        let attemptHex = "c53a4fa9eae4964c1f6b2e60740ee721971fa82748df98af93bc606bea20fcc4"
        let board = bytes(scopeHex), attempt = bytes(attemptHex)
        let api = try transport("good"), frame = try fixture("ballot.nrt")
        try await api.submitAnonymousBallot(frame)
        let boardBytes = try await api.getAnonymousBallotBoard(scope: board)
        let attemptBytes = try await api.getAnonymousParliamentAttempt(id: attempt)
        XCTAssertEqual(boardBytes, try fixture("board.nrt"))
        XCTAssertEqual(attemptBytes, try fixture("attempt.nrt"))
        let captured = BallotProtocol.capture.take()
        XCTAssertEqual(captured.first?.1, frame)
        let observed = captured.map { $0.0 }
        XCTAssertEqual(observed.map(\.httpMethod), ["POST", "GET", "GET"])
        XCTAssertEqual(observed.compactMap { $0.url?.path }, [
            "/good/v1/gov/anonymous-ballots",
            "/good/v1/gov/anonymous-ballots/boards/" + scopeHex,
            "/good/v1/gov/anonymous-ballots/attempts/" + attemptHex,
        ])
        XCTAssertEqual(observed[0].value(forHTTPHeaderField: "Content-Type"), "application/x-norito")
        for request in observed {
            for name in ["Authorization", "Cookie", "X-API-Token", "X-Iroha-Account", "Referer"] {
                XCTAssertNil(request.value(forHTTPHeaderField: name))
            }
        }
        XCTAssertThrowsError(try AnonymousBallotClientV1(baseURL: URL(string: "https://a:b@ballot.invalid")!))
        XCTAssertThrowsError(try AnonymousBallotClientV1(baseURL: URL(string: "https://ballot.invalid/?token=abc")!))
        XCTAssertThrowsError(try AnonymousBallotClientV1(baseURL: URL(string: "http://127.0.0.1")!))
        await fails { try await api.submitAnonymousBallot(Data(repeating: 0, count: 262145)) }
        await fails { _ = try await api.getAnonymousParliamentAttempt(id: board) }
        for mode in ["wrong", "body", "redirect"] {
            await fails { try await transport(mode).submitAnonymousBallot(frame) }
        }
        for mode in ["wrong", "empty", "large"] {
            await fails { _ = try await transport(mode).getAnonymousBallotBoard(scope: board) }
        }
        do {
            try await transport("refusal").submitAnonymousBallot(frame)
            XCTFail("Expected the node's refusal")
        } catch let error as AnonymousBallotErrorV1 {
            XCTAssertEqual(error, .refused(status: 429, code: "anonymous_ballot_throttled", retryAfterSeconds: 3))
        }
        for mode in ["compressed", "oversized"] {
            do {
                try await transport(mode).submitAnonymousBallot(frame)
                XCTFail("Expected the proxy's HTTP refusal")
            } catch let error as AnonymousBallotErrorV1 {
                XCTAssertEqual(error, .refused(status: 503, code: nil, retryAfterSeconds: 7))
            }
        }
        XCTAssertFalse(BallotProtocol.capture.take().contains { $0.0.url?.path == "/unexpected" })
    }
}
