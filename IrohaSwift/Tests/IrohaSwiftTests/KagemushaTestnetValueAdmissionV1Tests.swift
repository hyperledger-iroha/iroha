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
      frame(payload: Data([1])),
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

  func testForeignOperationHardwareFlagAndInvalidFinalityFailClosed() {
    for (index, replacement) in [
      (1, Data([1])),
      (9, Data(repeating: 9, count: 32)),
      (11, KagemushaUInt128V1(0).littleEndianBytes),
      (15, u64(0)),
      (16, Data(repeating: 0, count: 32)),
    ] {
      var fields = validFields()
      fields[index] = replacement
      assertInvalid(archive(fields: fields))
    }
  }

  func testMalformedOrNoncanonicalFieldsFailClosed() {
    var short = validFields()
    short[10] = Data(repeating: 8, count: 31)
    assertInvalid(archive(fields: short))

    var writer = CompactNoritoWriter()
    for field in validFields() { writer.writeField(field) }
    var tailed = writer.data
    tailed.append(0)
    assertInvalid(frame(payload: tailed))

    var overlong = Data([0x82, 0x00])
    overlong.append(writer.data.dropFirst())
    assertInvalid(frame(payload: overlong))

    var badPadding = archive()
    badPadding[NoritoHeader.encodedLength] = 1
    assertInvalid(badPadding)
  }

  private let schema = "connect_norito_bridge::KagemushaTestnetValueAdmissionArchiveV1"

  private func validFields() -> [Data] {
    [u16(1), Data([0]), Data(repeating: 1, count: 32),
     Data(repeating: 2, count: 32), Data(repeating: 3, count: 32),
     Data(repeating: 4, count: 32), Data(repeating: 5, count: 32),
     u32(2), Data(repeating: 6, count: 32), operationID,
     Data(repeating: 8, count: 32), KagemushaUInt128V1(17).littleEndianBytes,
     Data(repeating: 10, count: 32), Data(repeating: 11, count: 32),
     Data(repeating: 12, count: 32), u64(13), Data(repeating: 14, count: 32)]
  }

  private func archive(fields: [Data]? = nil) -> Data {
    var writer = CompactNoritoWriter()
    for field in fields ?? validFields() { writer.writeField(field) }
    return frame(payload: writer.data)
  }

  private func frame(payload: Data) -> Data {
    noritoEncode(typeName: schema, payload: payload,
                 flags: NoritoHeader.compactLen, payloadAlignment: 16)
  }

  private func u16(_ value: UInt16) -> Data {
    var writer = CompactNoritoWriter()
    writer.writeUInt16LE(value)
    return writer.data
  }

  private func u32(_ value: UInt32) -> Data {
    var writer = CompactNoritoWriter()
    writer.writeUInt32LE(value)
    return writer.data
  }

  private func u64(_ value: UInt64) -> Data {
    var writer = CompactNoritoWriter()
    writer.writeUInt64LE(value)
    return writer.data
  }

  private func assertInvalid(_ archive: Data, file: StaticString = #filePath, line: UInt = #line) {
    let endpoint = Endpoint(status: 0, archive: archive)
    XCTAssertThrowsError(try KagemushaTestnetValueAdmissionBridgeV1.admit(
      operationID: operationID, endpoint: endpoint), file: file, line: line) { error in
        XCTAssertEqual(error as? KagemushaTestnetValueAdmissionErrorV1,
                       .invalidArchive, file: file, line: line)
      }
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
