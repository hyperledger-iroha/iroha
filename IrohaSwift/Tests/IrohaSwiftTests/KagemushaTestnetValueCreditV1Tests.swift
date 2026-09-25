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
      noritoEncode(typeName: schema, payload: Data([1]),
                   flags: NoritoHeader.compactLen, payloadAlignment: 16),
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
    XCTAssertEqual(credit.version, 1)
    XCTAssertEqual(credit.scope.networkID, Data(repeating: 1, count: 32))
    XCTAssertEqual(credit.scope.releaseID, Data(repeating: 2, count: 32))
    XCTAssertEqual(credit.scope.releaseAttestationDigest, Data(repeating: 3, count: 32))
    XCTAssertEqual(credit.scope.assetIdentityDigest, Data(repeating: 4, count: 32))
    XCTAssertEqual(credit.scope.assetIncarnation, Data(repeating: 5, count: 32))
    XCTAssertEqual(credit.scope.assetScale, 2)
    XCTAssertEqual(credit.scope.liabilityPoolID, Data(repeating: 6, count: 32))
    XCTAssertEqual(credit.operationID, operationID)
    XCTAssertEqual(credit.creditID, Data(repeating: 8, count: 32))
    XCTAssertEqual(credit.amount, KagemushaUInt128V1(17))
    XCTAssertEqual(credit.totalAdmitted, KagemushaUInt128V1(23))
    XCTAssertTrue(credit.testnetOnly)
    XCTAssertFalse(credit.hardwareQualified)
    XCTAssertFalse(credit.productionMonetaryAuthorized)
  }

  func testVersionHardwareFlagOperationAndValueMislabelsFailClosed() {
    let replacements: [(Int, Data)] = [
      (0, u16(2)),
      (1, Data([1])),
      (1, Data([2])),
      (2, Data(repeating: 0, count: 32)),
      (3, Data(repeating: 1, count: 32)),
      (7, u32(29)),
      (8, Data(repeating: 4, count: 32)),
      (9, Data(repeating: 9, count: 32)),
      (10, Data(repeating: 0, count: 32)),
      (11, KagemushaUInt128V1(0).littleEndianBytes),
      (12, KagemushaUInt128V1(16).littleEndianBytes),
    ]
    for (index, replacement) in replacements {
      var fields = validFields()
      fields[index] = replacement
      assertInvalid(archive(fields: fields))
    }
  }

  func testFullWidthAtomicAmountIsPreservedWithoutUInt64Truncation() throws {
    var fields = validFields()
    var amount = Data(repeating: 0, count: 16)
    amount[0] = 5
    amount[8] = 1
    var total = amount
    total[0] = 10
    fields[11] = amount
    fields[12] = total
    let credit = try KagemushaTestnetValueCreditBridgeV1.credit(
      operationID: operationID, endpoint: Endpoint(status: 0, archive: archive(fields: fields)))
    XCTAssertEqual(credit.amount.littleEndianBytes, amount)
    XCTAssertEqual(credit.totalAdmitted.littleEndianBytes, total)
  }

  func testMalformedFieldsAndNonCanonicalLengthsFailClosed() {
    var short = validFields()
    short[10] = Data(repeating: 8, count: 31)
    assertInvalid(archive(fields: short))

    var writer = CompactNoritoWriter()
    for field in validFields() { writer.writeField(field) }
    var tailed = writer.data
    tailed.append(0)
    assertInvalid(frame(payload: tailed))

    // A two-byte length for the two-byte version field is not canonical.
    var overlong = Data([0x82, 0x00])
    overlong.append(writer.data.dropFirst())
    assertInvalid(frame(payload: overlong))

    var badPadding = archive()
    badPadding[NoritoHeader.encodedLength] = 1
    assertInvalid(badPadding)

    var badChecksum = archive()
    badChecksum[badChecksum.count - 1] ^= 1
    assertInvalid(badChecksum)
  }

  private let schema = "connect_norito_bridge::KagemushaTestnetMintLedgerCreditArchiveV1"

  private func validFields() -> [Data] {
    [u16(1), Data([0]), Data(repeating: 1, count: 32),
     Data(repeating: 2, count: 32), Data(repeating: 3, count: 32),
     Data(repeating: 4, count: 32), Data(repeating: 5, count: 32),
     u32(2), Data(repeating: 6, count: 32), operationID,
     Data(repeating: 8, count: 32), KagemushaUInt128V1(17).littleEndianBytes,
     KagemushaUInt128V1(23).littleEndianBytes]
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

  private func assertInvalid(_ archive: Data, file: StaticString = #filePath, line: UInt = #line) {
    let endpoint = Endpoint(status: 0, archive: archive)
    XCTAssertThrowsError(try KagemushaTestnetValueCreditBridgeV1.credit(
      operationID: operationID, endpoint: endpoint), file: file, line: line) { error in
        XCTAssertEqual(error as? KagemushaTestnetValueCreditErrorV1,
                       .invalidArchive, file: file, line: line)
      }
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
