import XCTest
@testable import IrohaSwift
import CryptoKit

final class CanonicalRequestTests: XCTestCase {
    func testCanonicalQuerySorting() {
        let rendered = CanonicalRequest.canonicalQueryString(from: "b=2&a=3&b=1&space=a+b")
        XCTAssertEqual(rendered, "a=3&b=1&b=2&space=a+b")
    }

    func testCanonicalQueryEncodesNonAscii() {
        let cafe = "caf\u{00E9}"
        let rendered = CanonicalRequest.canonicalQueryString(from: "name=\(cafe)")
        XCTAssertEqual(rendered, "name=caf%C3%A9")
    }

    func testSigningHeadersAreVerifiable() throws {
        guard #available(macOS 10.15, iOS 13.0, *) else {
            throw XCTSkip("CryptoKit not available")
        }
        let seed = Data(repeating: 5, count: 32)
        let signingKey = try SigningKey.ed25519(privateKey: seed)
        let message = CanonicalRequest.canonicalMessage(
            method: "get",
            path: "/v1/accounts/alice@wonderland/assets",
            query: "limit=1",
            body: Data("{\"foo\":1}".utf8)
        )
        let headers = try CanonicalRequest.signingHeaders(
            accountId: "alice@wonderland",
            method: "get",
            path: "/v1/accounts/alice@wonderland/assets",
            query: "limit=1",
            body: Data("{\"foo\":1}".utf8),
            signer: signingKey
        )
        let sigB64 = headers["X-Iroha-Signature"] ?? ""
        let signature = Data(base64Encoded: sigB64) ?? Data()
        let publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: signingKey.publicKey())
        XCTAssertTrue(publicKey.isValidSignature(signature, for: message))
    }
}
