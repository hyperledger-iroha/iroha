import Foundation
import XCTest
@testable import IrohaSwift

/// Closed ABI DATA fixtures; no simulated fixture claims Native admission or durable custody.
final class KagemushaWalletDeferredArchiveV1Tests: XCTestCase {
  private func call(_ status: Int32, sequence: UInt64 = 0, high: UInt64 = 0,
    detail: UInt32 = 0, bytes: Data = Data()) throws -> KagemushaWalletCallV1 {
    try .init(status: status, sequenceLow: sequence, sequenceHigh: high, detail: detail, bytes: bytes)
  }
  private func wrapped(kind: UInt32, detail: UInt32 = 0, bytes: Data = Data()) -> Data {
    func le<T: FixedWidthInteger>(_ value: T) -> Data {
      Data((0..<MemoryLayout<T>.size).map { UInt8(truncatingIfNeeded: value >> (8 * $0)) })
    }
    return le(kind) + le(detail) + le(UInt64(7)) + le(UInt64(9)) + le(UInt32(bytes.count)) + bytes
  }

  func testExchangeControlsHaveExactNonMonetaryGeometry() throws {
    for token in [UInt64(1), UInt64(Int64.max)] {
      let value = try call(60, sequence: token)
      XCTAssertEqual(value.sequenceLow, token)
      XCTAssertThrowsError(try value.completion())
    }
    for token in [UInt64(0), UInt64.max] { XCTAssertThrowsError(try call(60, sequence: token)) }
    XCTAssertThrowsError(try call(60, sequence: 1, high: 1))
    XCTAssertThrowsError(try call(60, sequence: 1, detail: 1))
    XCTAssertThrowsError(try call(60, sequence: 1, bytes: Data([1])))
    for detail in UInt32(0)...2 {
      let value = try call(61, detail: detail)
      XCTAssertEqual(KagemushaWalletExchangeEndV1(rawValue: value.detail)?.rawValue, detail)
      XCTAssertThrowsError(try value.completion())
    }
    XCTAssertThrowsError(try call(61, detail: 3))
    XCTAssertThrowsError(try call(61, sequence: 1))
  }

  func testIdleAndAllWaitReasonsCannotBecomeMonetaryCompletion() throws {
    guard case .noWork = try KagemushaWalletDeferredArchiveTurnV1(call(62)) else {
      return XCTFail("no-work discriminator lost")
    }
    for detail in UInt32(1)...6 {
      let result = try call(63, detail: detail)
      guard case .waiting(let wait) = try KagemushaWalletDeferredArchiveTurnV1(result) else {
        return XCTFail("wait discriminator lost")
      }
      XCTAssertEqual(wait.rawValue, detail)
      XCTAssertThrowsError(try result.completion())
      XCTAssertThrowsError(try call(63, sequence: 1, detail: detail))
    }
    for detail in [UInt32(0), 7, .max] { XCTAssertThrowsError(try call(63, detail: detail)) }
    XCTAssertThrowsError(try call(62, detail: 1))
    XCTAssertThrowsError(try call(62, bytes: Data([1])))
  }

  func testArchiveTurnPreservesEveryInnerCompletionAndDisposition() throws {
    for kind in [UInt32(1), 2, 3, 4, 5, 10] {
      let original = [1, 10].contains(kind) ? Data([0, 255, 3]) : Data()
      for disposition in UInt32(1)...2 {
        let outer = try call(64, detail: disposition, bytes: wrapped(kind: kind, bytes: original))
        let inner: KagemushaWalletCallV1
        switch try KagemushaWalletDeferredArchiveTurnV1(outer) {
        case .reconciled(let value): XCTAssertEqual(disposition, 1); inner = value
        case .executed(let value): XCTAssertEqual(disposition, 2); inner = value
        default: return XCTFail("completion discriminator lost")
        }
        XCTAssertEqual(inner.status, Int32(kind)); XCTAssertEqual(inner.sequenceLow, 7)
        XCTAssertEqual(inner.sequenceHigh, 9); XCTAssertEqual(inner.bytes, original)
        XCTAssertThrowsError(try outer.completion())
      }
    }
  }

  func testArchiveTurnRejectsUnknownNestedKindsLengthsAndOuterScalars() throws {
    for kind in [UInt32(0), 6, 11, 47, 60, 64, .max] {
      XCTAssertThrowsError(try KagemushaWalletDeferredArchiveTurnV1(call(64, detail: 2,
        bytes: wrapped(kind: kind))))
    }
    let valid = wrapped(kind: 1, bytes: Data([7]))
    for invalid in [Data(valid.dropLast()), valid + Data([0]), Data(valid.prefix(27)),
      wrapped(kind: 2, bytes: Data([1])), wrapped(kind: 1)] {
      XCTAssertThrowsError(try KagemushaWalletDeferredArchiveTurnV1(call(64, detail: 2, bytes: invalid)))
    }
    XCTAssertThrowsError(try call(64, sequence: 1, detail: 2, bytes: valid))
    XCTAssertThrowsError(try call(64, detail: 0, bytes: valid))
    XCTAssertThrowsError(try call(64, detail: 3, bytes: valid))
    XCTAssertThrowsError(try call(64, detail: 2, bytes: Data(repeating: 0, count: 10_029)))
    XCTAssertNoThrow(try KagemushaWalletDeferredArchiveTurnV1(call(64, detail: 2,
      bytes: wrapped(kind: 1, bytes: Data(repeating: 1, count: 10_000)))))
  }

  func testNativeSelectedMaintenanceInputsRejectHostAuthorityAndRetiredProducers() throws {
    for selector in [UInt32(57), 58] {
      XCTAssertNoThrow(try KagemushaWalletSetupInputV1(selector: selector))
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector, identity: Data(repeating: 1, count: 32)))
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector, token: 1))
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector, amount: .init(low: 1, high: 0)))
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector, first: Data([1])))
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector, second: Data([1])))
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector, third: Data([1])))
    }
    for selector in [UInt32(3), 41, 54] {
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector, first: Data([1])))
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector,
        identity: Data(repeating: 1, count: 32), first: Data([1])))
    }
  }

  func testActualNativeControlsAndMaintenanceRefuseMissingOwnerWithEmptyOutput() throws {
    let driver = try KagemushaWalletNativeDriverV1()
    var actions: [(UnsafeMutablePointer<KagemushaWalletNativeDriverV1.Output>) -> Int32] = [
      { driver.beginExchange(0, $0) }, { driver.endExchange(0, 1, $0) },
    ]
    for selector in [UInt32(57), 58] {
      let input = try KagemushaWalletSetupInputV1(selector: selector)
      actions.append { output in
        return input.withRequest { driver.setup(0, 0, $0, output) }
      }
    }
    for action in actions {
      XCTAssertThrowsError(try driver.controlResult { output in
        let status = action(output)
        XCTAssertEqual(status, -2); XCTAssertEqual(output.pointee.status, -2)
        XCTAssertEqual(output.pointee.reason, -1); XCTAssertEqual(output.pointee.platform_code, 0)
        XCTAssertEqual(output.pointee.sequence_low, 0); XCTAssertEqual(output.pointee.sequence_high, 0)
        XCTAssertEqual(output.pointee.detail, 0); XCTAssertEqual(output.pointee.length, 0)
        XCTAssertNil(output.pointee.bytes)
        return status
      }) { XCTAssertEqual($0 as? KagemushaWalletErrorV1, .closed) }
    }
  }
  func testActualNativeRetiredArchiveIntakesRejectBeforeOwnerWithEmptyOutput() throws {
    let driver = try KagemushaWalletNativeDriverV1()
    let source = try KagemushaWalletSetupInputV1(selector: 56,
      identity: Data(repeating: 9, count: 32), first: Data([0xa5]))
    for selector in [UInt32(3), 41, 54] {
      for exchange in [UInt64(0), 1] {
        XCTAssertThrowsError(try driver.controlResult { output in
          source.withRequest { request in
            var retired = request.pointee; retired.selector = selector
            let status = driver.setup(0, exchange, &retired, output)
            XCTAssertEqual(status, -1); XCTAssertEqual(output.pointee.status, -1)
            XCTAssertEqual(output.pointee.reason, -1); XCTAssertEqual(output.pointee.platform_code, 0)
            XCTAssertEqual(output.pointee.sequence_low, 0); XCTAssertEqual(output.pointee.sequence_high, 0)
            XCTAssertEqual(output.pointee.detail, 0); XCTAssertEqual(output.pointee.length, 0)
            XCTAssertNil(output.pointee.bytes)
            return status
          }
        }) { XCTAssertEqual($0 as? KagemushaWalletErrorV1,
          .native(status: -1, reason: -1, platformCode: 0)) }
      }
    }
  }

}
