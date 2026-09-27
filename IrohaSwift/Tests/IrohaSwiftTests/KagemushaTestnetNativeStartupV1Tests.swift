import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaTestnetNativeStartupV1Tests: XCTestCase {
  func testInvalidLengthNeverTouchesNativeEndpoint() {
    let endpoint = Endpoint()
    for bytes in [Data(), Data(repeating: 1, count: 1_048_577)] {
      XCTAssertThrowsError(try KagemushaTestnetNativeStartupV1.activate(
        signedBootstrap: bytes, endpoint: endpoint)) {
          XCTAssertEqual($0 as? KagemushaTestnetNativeStartupErrorV1, .invalidBootstrapLength)
        }
    }
    XCTAssertEqual(endpoint.contractCalls, 0)
    XCTAssertTrue(endpoint.received.isEmpty)
  }

  func testWrongOrMissingContractNeverActivates() {
    for contract: [UInt32]? in [nil, [], [1], [2, 1_048_576], [1, 1_048_575],
                               [1, 1_048_576, 0]] {
      let endpoint = Endpoint(contract: contract)
      XCTAssertThrowsError(try KagemushaTestnetNativeStartupV1.activate(
        signedBootstrap: Data([7]), endpoint: endpoint)) {
          XCTAssertEqual($0 as? KagemushaTestnetNativeStartupErrorV1, .contractMismatch)
        }
      XCTAssertTrue(endpoint.received.isEmpty)
    }
  }

  func testExactOpaquePackageReachesNativeWithoutAddingTrustInputs() throws {
    let endpoint = Endpoint()
    for bytes in [Data([0, 1, 255]), Data(repeating: 9, count: 1_048_576)] {
      try KagemushaTestnetNativeStartupV1.activate(signedBootstrap: bytes, endpoint: endpoint)
      XCTAssertEqual(endpoint.received.last, bytes)
    }
    XCTAssertEqual(endpoint.contractCalls, 2)
  }

  func testOnlyZeroMeansSuccessfulActivation() {
    for status: Int32 in [-312, -311, -1, 1, Int32.max] {
      let endpoint = Endpoint(status: status)
      XCTAssertThrowsError(try KagemushaTestnetNativeStartupV1.activate(
        signedBootstrap: Data([1]), endpoint: endpoint)) {
          XCTAssertEqual($0 as? KagemushaTestnetNativeStartupErrorV1,
            status == -312 ? .nativeContextUnavailable : .nativeRejected(status))
        }
      XCTAssertEqual(endpoint.received.count, 1)
    }
  }

  func testEachRetryReachesNativeAuthentication() throws {
    let endpoint = Endpoint()
    let bytes = Data([1, 2, 3])
    try KagemushaTestnetNativeStartupV1.activate(signedBootstrap: bytes, endpoint: endpoint)
    endpoint.status = -311
    XCTAssertThrowsError(try KagemushaTestnetNativeStartupV1.activate(
      signedBootstrap: bytes, endpoint: endpoint))
    XCTAssertEqual(endpoint.received, [bytes, bytes])
  }

  private final class Endpoint: KagemushaTestnetNativeStartupEndpointV1 {
    let words: [UInt32]?
    var status: Int32
    var contractCalls = 0
    var received: [Data] = []

    init(contract: [UInt32]? = [1, 1_048_576], status: Int32 = 0) {
      words = contract
      self.status = status
    }

    func contract() -> [UInt32]? {
      contractCalls += 1
      return words
    }

    func activate(signedBootstrap: Data) -> Int32 {
      received.append(signedBootstrap)
      return status
    }
  }
}
