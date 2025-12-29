import XCTest
@testable import IrohaSwift

final class Blake2bTests: XCTestCase {
    func testVectorABC() {
        let digest = Blake2b.hash256(Data("abc".utf8))
        let hex = digest.map { String(format: "%02x", $0) }.joined()
        // Known blake2b-256("abc")
        XCTAssertEqual(hex, "bddd813c634239723171ef3fee98579b94964e3bb1cb3e427262c8c068d52319")
    }
}
