import XCTest
@testable import IrohaSwift

final class NetworkIdTests: XCTestCase {
    private let literal =
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"

    func testCanonicalLiteralAndRawBytesRoundTrip() throws {
        let networkId = try NetworkId(literal: literal)
        XCTAssertEqual(networkId.literal, literal)
        XCTAssertEqual(networkId.bytes.count, NetworkId.byteCount)
        XCTAssertEqual(try NetworkId(bytes: networkId.bytes), networkId)
        XCTAssertEqual(try JSONDecoder().decode(NetworkId.self, from: JSONEncoder().encode(networkId)), networkId)
    }

    func testRetiredLabelsAndNoncanonicalLiteralsAreRejected() {
        for invalid in [
            "00000042",
            String(literal.dropFirst(5).prefix(64)),
            literal.lowercased(),
            String(literal.dropLast()) + "0",
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
