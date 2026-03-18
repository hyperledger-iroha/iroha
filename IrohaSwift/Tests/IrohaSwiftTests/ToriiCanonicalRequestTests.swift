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
        let url = URL(string: "https://example.com/v1/accounts/6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn/assets?limit=5")!
        let body = Data("{\"foo\":1}".utf8)

        let headers = try ToriiCanonicalRequest.buildHeaders(
            method: "get",
            url: url,
            body: body,
            accountId: "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
            privateKey: seed
        )
        let message = ToriiCanonicalRequest.canonicalRequestMessage(method: "get", url: url, body: body)
        guard let signatureB64 = headers[ToriiCanonicalRequest.headerSignature],
              let signature = Data(base64Encoded: signatureB64) else {
            XCTFail("signature header missing")
            return
        }
        let publicKey = try Curve25519.Signing.PrivateKey(rawRepresentation: seed).publicKey
        XCTAssertTrue(publicKey.isValidSignature(signature, for: message))
    }
}
