import Foundation
import XCTest
@testable import IrohaSwift

/// Tests only the called pure journal branch, never a synthetic native capability.
final class KagemushaAppApprovalIntentReconciliationV1Tests: XCTestCase {
  private let digest = Data(repeating: 1, count: 32)
  private let raw = Data([1, 2, 3])
  private func classify(_ intent: KagemushaAppAttestAssertionIntentV1, consumed: Bool) throws
    -> KagemushaAppApprovalIntentReconciliationV1.Action {
    try KagemushaAppApprovalIntentReconciliationV1.classify(intent, previousCounter: 7,
      counter: 9, signingDigest: digest, raw: raw, nativeConsumedOriginal: consumed)
  }
  func testArchivedConsumedOriginalNeverRequiresRollingLaterReadyCounterBack() throws {
    XCTAssertEqual(try classify(.ready(counter: 9), consumed: true), .archivedConsumedOriginal)
    XCTAssertEqual(try classify(.ready(counter: 12), consumed: true), .archivedConsumedOriginal)
    XCTAssertThrowsError(try classify(.ready(counter: 8), consumed: true))
    XCTAssertThrowsError(try classify(.ready(counter: 12), consumed: false))
  }
  func testPendingAndCompletedOriginalRequireExactFloorDigestAndRaw() throws {
    XCTAssertEqual(try classify(.pending(previousCounter: 7, selectionDigest: digest), consumed: false), .persistOriginal)
    XCTAssertEqual(try classify(.complete(previousCounter: 7, counter: 9,
      selectionDigest: digest, rawAssertion: raw), consumed: false), .completedOriginal)
    XCTAssertThrowsError(try classify(.pending(previousCounter: 8, selectionDigest: digest), consumed: true))
    XCTAssertThrowsError(try classify(.complete(previousCounter: 7, counter: 9,
      selectionDigest: digest, rawAssertion: Data([3])), consumed: true))
    XCTAssertThrowsError(try classify(.complete(previousCounter: 7, counter: 12,
      selectionDigest: digest, rawAssertion: raw), consumed: true))
  }
}
