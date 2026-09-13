import XCTest
import Foundation
@testable import IrohaSwift

final class KagemushaWalletV1Tests: XCTestCase {
  func testWalletReservationRejectsProviderIdentitySubstitution() throws {
    let operationID = Data(repeating: 10, count: 32)
    XCTAssertEqual(try kagemushaReserveOperationIDV1(operationID) { $0 }, operationID)
    XCTAssertThrowsError(
      try kagemushaReserveOperationIDV1(operationID) { _ in Data(repeating: 11, count: 32) })
    var called = false
    XCTAssertThrowsError(
      try kagemushaReserveOperationIDV1(Data(repeating: 0, count: 32)) { value in
        called = true
        return value
      })
    XCTAssertFalse(called)
  }

  func testV1MonetaryOperationsAreTheSixAggregateBalanceTransitions() {
    XCTAssertEqual(
      KagemushaOperationKindV1.allCases.map(\.rawValue),
      [0, 1, 2, 3, 4, 5]
    )
    XCTAssertEqual(KagemushaOperationKindV1.receiveFold.rawValue, 3)
    XCTAssertEqual(KagemushaOperationKindV1.rotate.rawValue, 5)
  }
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
  @MainActor
  func testNonblockingSnapshotProbeKeepsMainActorAvailableToHeldNativeGate() async {
    let gate = KagemushaForegroundGateV1(sharedLock: NSRecursiveLock())
    let finished = expectation(description: "native worker finished after MainActor admission")
    let admitted = DispatchSemaphore(value: 0)
    DispatchQueue.global().async {
      gate.withLock {
        Task { @MainActor in
          XCTAssertNil(gate.tryWithLock { true }, "Busy native work must reject without waiting")
          admitted.signal()
        }
        XCTAssertEqual(admitted.wait(timeout: .now() + 2), .success,
          "A blocking MainActor probe deadlocks native admission")
      }
      finished.fulfill()
    }
    await fulfillment(of: [finished], timeout: 3)
    XCTAssertEqual(gate.tryWithLock { true }, true)
  }

  @MainActor
  func testNonblockingSnapshotProbeAlsoRejectsDirectProviderOperationLock() async {
    let providerLock = NSRecursiveLock()
    let gate = KagemushaForegroundGateV1(sharedLock: providerLock)
    let finished = expectation(description: "provider worker finished")
    let admitted = DispatchSemaphore(value: 0)
    DispatchQueue.global().async {
      providerLock.lock()
      Task { @MainActor in
        XCTAssertNil(gate.tryWithLock { true })
        admitted.signal()
      }
      XCTAssertEqual(admitted.wait(timeout: .now() + 2), .success)
      providerLock.unlock()
      finished.fulfill()
    }
    await fulfillment(of: [finished], timeout: 3)
    XCTAssertEqual(gate.tryWithLock { true }, true)
  }

  func testScopedSnapshotActionExcludesConcurrentOwnerUntilActionReturns() {
    let gate = KagemushaForegroundGateV1(sharedLock: NSRecursiveLock())
    let probed = DispatchSemaphore(value: 0)
    XCTAssertEqual(gate.tryWithLock {
      DispatchQueue.global().async {
        XCTAssertNil(gate.tryWithLock { true }, "The final resume action must keep its state lease")
        probed.signal()
      }
      XCTAssertEqual(probed.wait(timeout: .now() + 2), .success)
      return true
    }, true)
    XCTAssertEqual(gate.tryWithLock { true }, true)
  }

}
