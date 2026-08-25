import Foundation
import XCTest
@testable import IrohaSwift

final class NetworkIdTests: XCTestCase {
    private let literal =
        "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149"
    private let noritoJSONLiteral =
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"

    func testCanonicalLiteralAndRawBytesRoundTrip() throws {
        let networkId = try NetworkId(literal: literal)
        XCTAssertEqual(networkId.literal, literal)
        XCTAssertEqual(networkId.description, literal)
        XCTAssertEqual(networkId.bytes.count, NetworkId.byteCount)
        XCTAssertEqual(
            networkId.bytes.map { String(format: "%02x", $0) }.joined(),
            literal
        )
        XCTAssertEqual(try NetworkId(bytes: networkId.bytes), networkId)
    }

    func testNoritoJSONKeepsTaggedChecksummedHashContract() throws {
        let networkId = try NetworkId(literal: literal)
        let encoded = try JSONEncoder().encode(networkId)
        XCTAssertEqual(String(decoding: encoded, as: UTF8.self), "\"\(noritoJSONLiteral)\"")
        XCTAssertEqual(try JSONDecoder().decode(NetworkId.self, from: encoded), networkId)
        XCTAssertEqual(
            try JSONDecoder().decode(
                NetworkId.self,
                from: Data("\"\(noritoJSONLiteral)\"".utf8)
            ),
            networkId
        )

        XCTAssertThrowsError(
            try JSONDecoder().decode(NetworkId.self, from: Data("\"\(literal)\"".utf8))
        )
        XCTAssertThrowsError(try NetworkId(literal: noritoJSONLiteral))
    }

    func testRetiredLabelsAndNoncanonicalLiteralsAreRejected() {
        for invalid in [
            "00000042",
            literal.uppercased(),
            "hash:\(literal)",
            noritoJSONLiteral,
            String(literal.dropLast()) + "8",
            " \(literal)",
        ] {
            XCTAssertThrowsError(try NetworkId(literal: invalid))
        }
    }

    func testRawBytesRequireExactMarkedHash() throws {
        let canonical = try NetworkId(literal: literal).bytes
        XCTAssertThrowsError(try NetworkId(bytes: Data(canonical.dropLast())))
        var unmarked = canonical
        unmarked[unmarked.count - 1] &= 0xFE
        XCTAssertThrowsError(try NetworkId(bytes: unmarked))
    }
}
