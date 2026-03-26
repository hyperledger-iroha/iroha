import XCTest
import CryptoKit
@testable import IrohaSwift

final class ToriiCanonicalRequestTests: XCTestCase {
    func testCanonicalQuerySorting() {
        let rendered = ToriiCanonicalRequest.canonicalQueryString(from: "b=2&a=3&b=1&space=a+b")
        XCTAssertEqual(rendered, "a=3&b=1&b=2&space=a+b")
    }

    func testHeadersProduceVerifiableSignature() throws {
        let seed = Data(repeating: 7, count: 32)
        let url = URL(string: "https://example.com/v1/accounts/sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB/assets?limit=5")!
        let body = Data("{\"foo\":1}".utf8)
        let timestampMs: UInt64 = 1_717_171_717_000
        let nonce = "swift-torii-canonical-nonce"

        let headers = try ToriiCanonicalRequest.buildHeaders(
            method: "get",
            url: url,
            body: body,
            accountId: "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            privateKey: seed,
            timestampMs: timestampMs,
            nonce: nonce
        )
        let message = ToriiCanonicalRequest.signatureMessage(
            method: "get",
            url: url,
            body: body,
            timestampMs: timestampMs,
            nonce: nonce
        )
        guard let signatureB64 = headers[ToriiCanonicalRequest.headerSignature],
              let signature = Data(base64Encoded: signatureB64) else {
            XCTFail("signature header missing")
            return
        }
        let publicKey = try Curve25519.Signing.PrivateKey(rawRepresentation: seed).publicKey
        XCTAssertEqual(headers[ToriiCanonicalRequest.headerTimestampMs], String(timestampMs))
        XCTAssertEqual(headers[ToriiCanonicalRequest.headerNonce], nonce)
        XCTAssertTrue(publicKey.isValidSignature(signature, for: message))
    }
}
