import Foundation
import XCTest
@testable import IrohaSwift

/// Queue/lifetime fixtures only. Native alone authenticates idle eligibility and monetary work.
final class KagemushaWalletExchangeLifetimeV1Tests: XCTestCase {
  private final class Counter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0
    func next() -> Int { lock.lock(); defer { lock.unlock() }; count += 1; return count }
    var value: Int { lock.lock(); defer { lock.unlock() }; return count }
  }

  func testAdmittedQueuedTaskKeepsNativeExchangeUntilExplicitFinish() throws {
    let native = Counter()
    let lifetime = KagemushaWalletExchangeLifetimeV1 { _ = native.next(); return .ended }
    let accepted = try lifetime.admit()
    let ending = expectation(description: "end entered")
    let ended = expectation(description: "end joined queued task")
    DispatchQueue.global().async {
      ending.fulfill()
      defer { ended.fulfill() }
      do {
        let result = try lifetime.end()
        XCTAssertEqual(result, .ended)
      }
      catch { XCTFail("exchange cleanup failed: \(error)") }
    }
    wait(for: [ending], timeout: 2)
    // The task was accepted before enqueue. It may start while end is waiting.
    XCTAssertEqual(try accepted.withUse { 17 }, 17)
    XCTAssertEqual(native.value, 0)
    accepted.finish()
    wait(for: [ended], timeout: 2)
    XCTAssertEqual(native.value, 1)
    XCTAssertThrowsError(try lifetime.admit())
    XCTAssertThrowsError(try accepted.withUse { XCTFail("finished alias dispatched") })
    XCTAssertEqual(try lifetime.end(), .alreadyEnded)
    XCTAssertEqual(native.value, 1)
  }

  func testFinishDuringBlockedNativeCallDoesNotReleaseItsOwnership() throws {
    let entered = DispatchSemaphore(value: 0), release = DispatchSemaphore(value: 0)
    let native = Counter()
    let lifetime = KagemushaWalletExchangeLifetimeV1 { _ = native.next(); return .ended }
    let accepted = try lifetime.admit()
    let workReturned = expectation(description: "work returned")
    DispatchQueue.global().async {
      defer { workReturned.fulfill() }
      do {
        try accepted.withUse {
          entered.signal()
          XCTAssertEqual(release.wait(timeout: .now() + 2), .success)
        }
      } catch { XCTFail("admitted work failed: \(error)") }
    }
    XCTAssertEqual(entered.wait(timeout: .now() + 2), .success)
    accepted.finish()
    XCTAssertThrowsError(try accepted.withUse { XCTFail("new use after finish") })
    let ended = expectation(description: "native end after running task")
    DispatchQueue.global().async {
      defer { ended.fulfill() }
      do {
        let result = try lifetime.end()
        XCTAssertEqual(result, .ended)
      }
      catch { XCTFail("exchange cleanup failed: \(error)") }
    }
    XCTAssertEqual(native.value, 0)
    release.signal()
    wait(for: [workReturned, ended], timeout: 2)
    XCTAssertEqual(native.value, 1)
  }

  func testFailedEndFencesNewWorkAndRetriesSameCleanup() throws {
    let attempts = Counter()
    let lifetime = KagemushaWalletExchangeLifetimeV1 {
      if attempts.next() == 1 { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return .alreadyEnded
    }
    let accepted = try lifetime.admit()
    accepted.finish()
    XCTAssertThrowsError(try lifetime.end()) {
      XCTAssertEqual($0 as? KagemushaWalletErrorV1, .invalidNativeOutput)
    }
    XCTAssertThrowsError(try lifetime.admit())
    XCTAssertEqual(attempts.value, 1)
    XCTAssertEqual(try lifetime.end(), .alreadyEnded)
    XCTAssertEqual(attempts.value, 2)
    XCTAssertEqual(try lifetime.end(), .alreadyEnded)
    XCTAssertEqual(attempts.value, 2)
  }

  func testNativeEndingRemainsRetainedForExplicitRetry() throws {
    let attempts = Counter()
    let lifetime = KagemushaWalletExchangeLifetimeV1 {
      attempts.next() == 1 ? .ending : .alreadyEnded
    }
    XCTAssertEqual(try lifetime.end(), .ending)
    XCTAssertThrowsError(try lifetime.admit())
    XCTAssertEqual(try lifetime.end(), .alreadyEnded)
    XCTAssertEqual(attempts.value, 2)
  }

  func testThrowingWorkKeepsTaskAcceptedUntilFinish() throws {
    let native = Counter()
    let lifetime = KagemushaWalletExchangeLifetimeV1 { _ = native.next(); return .ended }
    let accepted = try lifetime.admit()
    XCTAssertThrowsError(try accepted.withUse { throw KagemushaWalletErrorV1.invalidInput })
    XCTAssertEqual(try accepted.withUse { 3 }, 3)
    XCTAssertEqual(native.value, 0)
    accepted.finish()
    XCTAssertEqual(try lifetime.end(), .ended)
    XCTAssertEqual(native.value, 1)
  }

  func testAliasFinishAndDestructionReleaseOnlyOneAcceptedTask() throws {
    let native = Counter()
    let lifetime = KagemushaWalletExchangeLifetimeV1 { _ = native.next(); return .ended }
    var accepted: KagemushaWalletExchangeTaskV1? = try lifetime.admit()
    let alias = try XCTUnwrap(accepted)
    accepted?.finish(); alias.finish(); accepted = nil
    XCTAssertThrowsError(try alias.withUse { XCTFail("alias replay") })
    XCTAssertEqual(try lifetime.end(), .ended)
    XCTAssertEqual(native.value, 1)
  }

  func testUnqueuedTaskDestructionReleasesAdmission() throws {
    let lifetime = KagemushaWalletExchangeLifetimeV1 { .ended }
    var accepted: KagemushaWalletExchangeTaskV1? = try lifetime.admit()
    XCTAssertNotNil(accepted)
    accepted = nil
    XCTAssertEqual(try lifetime.end(), .ended)
  }
  func testRequestEndFencesBeforeCleanupEnqueueWithoutFinishingAcceptedWork() throws {
    let native = Counter()
    let lifetime = KagemushaWalletExchangeLifetimeV1 { _ = native.next(); return .ended }
    let accepted = try lifetime.admit()
    lifetime.requestEnd()
    XCTAssertThrowsError(try lifetime.admit())
    XCTAssertEqual(native.value, 0, "fencing cannot dispatch Native cleanup")
    XCTAssertEqual(try accepted.withUse { 23 }, 23)
    accepted.finish()
    XCTAssertEqual(try lifetime.end(), .ended)
    XCTAssertEqual(native.value, 1)
  }

}
