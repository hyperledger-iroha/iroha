import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaTestnetValueCreditV1Tests: XCTestCase {
  private let operationID = Data(repeating: 7, count: 32)

  func testInvalidOperationIDNeverReachesNativeEndpoint() {
    let endpoint = Endpoint(status: 0, archive: archive())
    for invalid in [Data(), Data(repeating: 0, count: 32),
                    Data(repeating: 7, count: 31), Data(repeating: 7, count: 33)] {
      XCTAssertThrowsError(try KagemushaTestnetValueCreditBridgeV1.credit(
        operationID: invalid, endpoint: endpoint)) { error in
          XCTAssertEqual(error as? KagemushaTestnetValueCreditErrorV1,
                         .invalidOperationID)
        }
    }
    XCTAssertEqual(endpoint.calls, 0)
  }

  func testUnavailableLedgerAndNativeRejectionStayDistinct() {
    for (status, expected) in [
      (-312, KagemushaTestnetValueCreditErrorV1.ledgerUnavailable),
      (-311, KagemushaTestnetValueCreditErrorV1.nativeRejected(-311)),
    ] {
      let endpoint = Endpoint(status: status, archive: archive())
      XCTAssertThrowsError(try KagemushaTestnetValueCreditBridgeV1.credit(
        operationID: operationID, endpoint: endpoint)) { error in
          XCTAssertEqual(error as? KagemushaTestnetValueCreditErrorV1, expected)
        }
      XCTAssertEqual(endpoint.calls, 1)
    }
  }

  func testMalformedOrWrongSchemaArchiveFailsClosed() {
    let invalid = [
      Data(),
      Data(repeating: 1, count: 513),
      noritoEncode(typeName: "wrong.schema", payload: Data([1]),
                   flags: NoritoHeader.compactLen),
      noritoEncode(typeName: "connect_norito_bridge::KagemushaTestnetMintLedgerCreditArchiveV1",
                   payload: Data([1]), flags: 0),
    ]
    for value in invalid {
      let endpoint = Endpoint(status: 0, archive: value)
      XCTAssertThrowsError(try KagemushaTestnetValueCreditBridgeV1.credit(
        operationID: operationID, endpoint: endpoint)) { error in
          XCTAssertEqual(error as? KagemushaTestnetValueCreditErrorV1,
                         .invalidArchive)
        }
    }
  }

  func testOnlyOperationIDCrossesNativeBoundaryAndCreditCannotQualifyHardware() throws {
    let expected = archive()
    let endpoint = Endpoint(status: 0, archive: expected)
    let credit = try KagemushaTestnetValueCreditBridgeV1.credit(
      operationID: operationID, endpoint: endpoint)
    XCTAssertEqual(endpoint.calls, 1)
    XCTAssertEqual(endpoint.operationID, operationID)
    XCTAssertEqual(credit.canonicalArchive, expected)
    XCTAssertTrue(credit.testnetOnly)
    XCTAssertFalse(credit.hardwareQualified)
    XCTAssertFalse(credit.productionMonetaryAuthorized)
  }

  private func archive() -> Data {
    // The native ledger alone emits the complete counted-credit payload.
    noritoEncode(typeName: "connect_norito_bridge::KagemushaTestnetMintLedgerCreditArchiveV1",
                 payload: Data([1]), flags: NoritoHeader.compactLen)
  }

  private final class Endpoint: KagemushaTestnetValueCreditEndpointV1 {
    let status: Int32
    let archive: Data
    var calls = 0
    var operationID: Data?

    init(status: Int32, archive: Data) {
      self.status = status
      self.archive = archive
    }

    func credit(operationID: Data) -> (status: Int32, archive: Data) {
      calls += 1
      self.operationID = operationID
      return (status, archive)
    }
  }
}
