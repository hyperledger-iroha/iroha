import XCTest
@testable import IrohaSwift

final class KagemushaWalletV1Tests: XCTestCase {
  func testStagingDispositionHasOnlyDurableOutcomes() {
    XCTAssertEqual(KagemushaHardwareStageDispositionV1.staged, .staged)
    XCTAssertEqual(KagemushaHardwareStageDispositionV1.exactDuplicate, .exactDuplicate)
  }

  func testThreeMessageProviderSurfaceCompiles() {
    func requireProvider(_ provider: any KagemushaHardwareProviderV1) {
      _ = provider
    }
    _ = requireProvider
  }
}
