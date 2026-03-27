import XCTest
@testable import IrohaSwift

final class TransportSecurityTests: XCTestCase {
    func testRejectsInsecureCanonicalAuthHeaders() throws {
        let baseURL = try XCTUnwrap(URL(string: "http://example.com"))
        let targetURL = try XCTUnwrap(URL(string: "http://example.com/query"))
        let violation = IrohaTransportSecurity.httpViolation(
            context: "ToriiClient",
            baseURL: baseURL,
            targetURL: targetURL,
            headers: [
                "X-Iroha-Account": "alice",
                "X-Iroha-Signature": "deadbeef",
                "X-Iroha-Timestamp-Ms": "123",
                "X-Iroha-Nonce": "456"
            ],
            body: nil
        )
        XCTAssertEqual(
            violation,
            "ToriiClient refuses insecure transport over http; use https."
        )
    }

    func testRejectsInsecureSeedBody() throws {
        let baseURL = try XCTUnwrap(URL(string: "http://example.com"))
        let targetURL = try XCTUnwrap(URL(string: "http://example.com/confidential"))
        let body = #"{"seed_hex":"deadbeef"}"#.data(using: .utf8)
        let violation = IrohaTransportSecurity.httpViolation(
            context: "ToriiClient",
            baseURL: baseURL,
            targetURL: targetURL,
            headers: [:],
            body: body
        )
        XCTAssertEqual(
            violation,
            "ToriiClient refuses insecure transport over http; use https."
        )
    }

    func testDeriveConfidentialKeysetAcceptsSeedHexAndBase64() throws {
        let seedHex = String(repeating: "42", count: 32)
        let seedData = try XCTUnwrap(Data(hexString: seedHex))
        let fromHex = try ConfidentialKeyset.derive(seedHex: seedHex)
        let fromBase64 = try ConfidentialKeyset.derive(seedBase64: seedData.base64EncodedString())
        XCTAssertEqual(fromHex, fromBase64)
        XCTAssertEqual(fromHex.spendKey, seedData)
    }
}
