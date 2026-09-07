import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaAssetDefinitionIDV1Tests: XCTestCase {
  private let uuid: [UInt8] = [
    0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b,
    0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd, 0xcd, 0x2f,
  ]

  private func payload(_ bytes: [UInt8]) -> Data {
    Data(bytes.flatMap { [1, $0] })
  }

  func testRejectsNonV4UUIDVersions() throws {
    for version: UInt8 in 0..<16 where version != 4 {
      var invalid = uuid
      invalid[6] = (version << 4) | (invalid[6] & 0x0f)
      XCTAssertThrowsError(try KagemushaAssetDefinitionIDV1(canonicalPayload: payload(invalid)),
        "UUID version \(version)")
    }
  }

  func testRejectsNonRFC4122Variants() throws {
    for variant: UInt8 in 0..<4 where variant != 2 {
      var invalid = uuid
      invalid[8] = (variant << 6) | (invalid[8] & 0x3f)
      XCTAssertThrowsError(try KagemushaAssetDefinitionIDV1(canonicalPayload: payload(invalid)),
        "UUID variant \(variant)")
    }
  }

  func testRequiresExactCanonicalByteElementArray() throws {
    let valid = payload(uuid)
    for length in 0..<valid.count {
      XCTAssertThrowsError(try KagemushaAssetDefinitionIDV1(canonicalPayload: valid.prefix(length)))
    }
    for invalid in [Data(uuid), valid + Data([0]), Data(repeating: 1, count: 513)] {
      XCTAssertThrowsError(try KagemushaAssetDefinitionIDV1(canonicalPayload: invalid))
    }
    for index in stride(from: 0, to: valid.count, by: 2) {
      for invalidLength: UInt8 in [0, 2, 0x81] {
        var invalid = valid
        invalid[index] = invalidLength
        XCTAssertThrowsError(try KagemushaAssetDefinitionIDV1(canonicalPayload: invalid))
      }
    }
  }

  func testAcceptsAllUUIDv4VersionAndVariantPayloadBitsAndCanonicalLiteral() throws {
    // Rust validates only the version nibble and RFC4122 variant bits; the
    // remaining bits are unrestricted, including otherwise zero or all-one UUIDs.
    for lowVersion: UInt8 in 0..<16 {
      for lowVariant: UInt8 in 0..<64 {
        var valid = uuid
        valid[6] = 0x40 | lowVersion
        valid[8] = 0x80 | lowVariant
        XCTAssertEqual(try KagemushaAssetDefinitionIDV1(canonicalPayload: payload(valid)).canonicalPayload,
          payload(valid))
      }
    }
    for byte: UInt8 in [0, 0xff] {
      var valid = [UInt8](repeating: byte, count: 16)
      valid[6] = (valid[6] & 0x0f) | 0x40
      valid[8] = (valid[8] & 0x3f) | 0x80
      XCTAssertNoThrow(try KagemushaAssetDefinitionIDV1(canonicalPayload: payload(valid)))
    }
    let literal = try XCTUnwrap(AssetDefinitionAddress.encode(uuidBytes: Data(uuid)))
    XCTAssertEqual(try KagemushaAssetDefinitionIDV1(literal).canonicalPayload, payload(uuid))
  }

  func testRebasesSlicesAndOwnsBytesAfterCallerMutation() throws {
    let expected = payload(uuid)
    let slice = (Data([9, 9]) + expected).dropFirst(2)
    XCTAssertEqual(try KagemushaAssetDefinitionIDV1(canonicalPayload: slice).canonicalPayload, expected)
    let mutable = NSMutableData(data: expected)
    let asset = try KagemushaAssetDefinitionIDV1(canonicalPayload: Data(referencing: mutable))
    mutable.resetBytes(in: NSRange(location: 0, length: expected.count))
    var returned = asset.canonicalPayload
    returned[13] = 0
    XCTAssertEqual(asset.canonicalPayload, expected)
  }

  func testAggregateCodecRejectsMalformedUUIDWithValidChecksum() throws {
    let aggregate = try KagemushaAggregateStateCommitmentV1(
      releaseID: Data(repeating: 1, count: 32), networkID: Data(repeating: 3, count: 32),
      asset: .init(canonicalPayload: payload(uuid)),
      assetIncarnation: .init(bytes: Data(repeating: 5, count: 32)), scale: 2,
      liabilityPoolID: Data(repeating: 6, count: 32), laneID: Data(repeating: 7, count: 32),
      hardwareEpochID: Data(repeating: 8, count: 32), keyReference: Data(repeating: 9, count: 32),
      hardwarePolicyID: Data(repeating: 10, count: 32), sequence: .init(0),
      stateCommitment: Data(repeating: 11, count: 32))
    let valid = try KagemushaNoritoV1.encodeAggregateStateShape(aggregate)
    XCTAssertEqual(try KagemushaNoritoV1.decodeAggregateStateShapeExact(valid), aggregate)
    let frame = try XCTUnwrap(noritoDecodeFrame(valid))
    for invalidFieldIndex in [13, 17] {
      var reader = CanonicalNoritoReader(data: frame.payload)
      var fields: [Data] = []
      while reader.remaining() > 0 { fields.append(try reader.readCompactField()) }
      XCTAssertEqual(fields[3], payload(uuid))
      fields[3][invalidFieldIndex] = 0
      var writer = CompactNoritoWriter()
      fields.forEach { writer.writeField($0) }
      let invalid = noritoEncode(
        typeName: "iroha_data_model::kagemusha::kagemusha_v1::KagemushaAggregateStateCommitmentV1",
        payload: writer.data, flags: NoritoHeader.compactLen, payloadAlignment: 16)
      XCTAssertNotNil(noritoDecodeFrame(invalid), "Valid framing must reach the shared asset decoder")
      XCTAssertThrowsError(try KagemushaNoritoV1.decodeAggregateStateShapeExact(invalid),
        "Malformed asset field byte \(invalidFieldIndex)")
    }
  }
}
