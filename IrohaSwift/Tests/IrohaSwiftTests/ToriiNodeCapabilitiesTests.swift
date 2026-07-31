@testable import IrohaSwift
import XCTest

/// Verifies the client capability values pinned by the current Torii wire contract.
final class ToriiNodeCapabilitiesTests: XCTestCase {
    func testExpectedDataModelVersionMatchesCurrentWireContract() {
        XCTAssertEqual(ToriiNodeCapabilities.expectedDataModelVersion, 4)
    }
}
