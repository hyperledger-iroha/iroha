import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaTestnetValueAdmissionV1Tests: XCTestCase {
  private let operationID = Data(repeating: 7, count: 32)

  func testInvalidOperationIDNeverReachesNativeEndpoint() {
    let endpoint = Endpoint(status: 0, archive: archive())
    for invalid in [Data(), Data(repeating: 0, count: 32),
                    Data(repeating: 7, count: 31), Data(repeating: 7, count: 33)] {
      XCTAssertThrowsError(try KagemushaTestnetValueAdmissionBridgeV1.admit(
        operationID: invalid, endpoint: endpoint)) { error in
          XCTAssertEqual(error as? KagemushaTestnetValueAdmissionErrorV1,
                         .invalidOperationID)
        }
    }
    XCTAssertEqual(endpoint.calls, 0)
  }

  func testUnavailableOwnerAndNativeRejectionStayDistinct() {
    for (status, expected) in [
      (-312, KagemushaTestnetValueAdmissionErrorV1.ownerUnavailable),
      (-311, KagemushaTestnetValueAdmissionErrorV1.nativeRejected(-311)),
    ] {
      let endpoint = Endpoint(status: status, archive: archive())
      XCTAssertThrowsError(try KagemushaTestnetValueAdmissionBridgeV1.admit(
        operationID: operationID, endpoint: endpoint)) { error in
          XCTAssertEqual(error as? KagemushaTestnetValueAdmissionErrorV1, expected)
        }
      XCTAssertEqual(endpoint.calls, 1)
    }
  }

  func testMalformedOrWrongSchemaArchiveFailsClosed() {
    let invalid = [
      Data(),
      Data(repeating: 1, count: 769),
      noritoEncode(typeName: "wrong.schema", payload: Data([1]),
                   flags: NoritoHeader.compactLen),
      noritoEncode(typeName: "connect_norito_bridge::KagemushaTestnetValueAdmissionArchiveV1",
                   payload: Data([1]), flags: 0),
    ]
    for value in invalid {
      let endpoint = Endpoint(status: 0, archive: value)
      XCTAssertThrowsError(try KagemushaTestnetValueAdmissionBridgeV1.admit(
        operationID: operationID, endpoint: endpoint)) { error in
          XCTAssertEqual(error as? KagemushaTestnetValueAdmissionErrorV1,
                         .invalidArchive)
        }
    }
  }

  func testOnlyOperationIDCrossesNativeBoundaryAndArchiveCannotQualifyHardware() throws {
    let expected = archive()
    let endpoint = Endpoint(status: 0, archive: expected)
    let admission = try KagemushaTestnetValueAdmissionBridgeV1.admit(
      operationID: operationID, endpoint: endpoint)
    XCTAssertEqual(endpoint.calls, 1)
    XCTAssertEqual(endpoint.operationID, operationID)
    XCTAssertEqual(admission.canonicalArchive, expected)
    XCTAssertTrue(admission.testnetOnly)
    XCTAssertFalse(admission.hardwareQualified)
    XCTAssertFalse(admission.productionMonetaryAuthorized)
  }

  private func archive() -> Data {
    // The native owner alone emits the complete verified payload. The SDK checks its frame.
    noritoEncode(typeName: "connect_norito_bridge::KagemushaTestnetValueAdmissionArchiveV1",
                 payload: Data([1]), flags: NoritoHeader.compactLen)
  }

  private final class Endpoint: KagemushaTestnetValueAdmissionEndpointV1 {
    let status: Int32
    let archive: Data
    var calls = 0
    var operationID: Data?

    init(status: Int32, archive: Data) {
      self.status = status
      self.archive = archive
    }

    func admit(operationID: Data) -> (status: Int32, archive: Data) {
      calls += 1
      self.operationID = operationID
      return (status, archive)
    }
  }
}
