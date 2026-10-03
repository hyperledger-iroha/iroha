import Foundation
import XCTest
@testable import IrohaSwift

/// Mathematical codec DATA only; no fake endpoint, native grant or hardware positive.
final class KagemushaOrdinaryNativeArchiveV1Tests: XCTestCase {
  func testCanonicalCompactArchivePreservesNestedVectorAndUnsignedID() throws {
    let fields = [Data(), Data([1]), Data(repeating: 7, count: 128)]
    let raw = KagemushaOrdinaryNativeArchiveV1.encode(schema: KagemushaOrdinaryNativeArchiveV1.outgoingResponse,
      phase: 11, id: UInt64.max, fields: fields)
    XCTAssertEqual(raw[39], NoritoHeader.compactLen)
    let result = try KagemushaOrdinaryNativeArchiveV1.decode(raw,
      schema: KagemushaOrdinaryNativeArchiveV1.outgoingResponse, phase: 11, id: UInt64.max)
    XCTAssertEqual(result.fields, fields); XCTAssertEqual(result.id, UInt64.max)
  }
  func testResponseIdentityPhaseAndSchemaCannotBeSubstituted() throws {
    let schema = KagemushaOrdinaryNativeArchiveV1.outgoingResponse
    let raw = KagemushaOrdinaryNativeArchiveV1.encode(schema: schema, phase: 14, id: 9, fields: [])
    XCTAssertThrowsError(try KagemushaOrdinaryNativeArchiveV1.decode(raw, schema: schema, phase: 15, id: 9))
    XCTAssertThrowsError(try KagemushaOrdinaryNativeArchiveV1.decode(raw, schema: schema, phase: 14, id: 10))
    XCTAssertThrowsError(try KagemushaOrdinaryNativeArchiveV1.decode(raw,
      schema: KagemushaOrdinaryNativeArchiveV1.currentResponse, phase: 14, id: 9))
  }
  func testValidChecksumCannotPermitAlternateScalarLengthOrFlags() throws {
    let schema = KagemushaOrdinaryNativeArchiveV1.currentResponse
    let raw = KagemushaOrdinaryNativeArchiveV1.encode(schema: schema, phase: 3, id: 9, fields: [])
    var payload = try XCTUnwrap(noritoDecodeFrame(raw)).payload
    payload.replaceSubrange(0..<1, with: Data([0x82, 0])) // Overlong compact prefix for u16.
    let overlong = noritoEncode(typeName: schema, payload: payload, flags: 2, payloadAlignment: 8)
    XCTAssertThrowsError(try KagemushaOrdinaryNativeArchiveV1.decode(overlong, schema: schema, phase: 3, id: 9))
    let fixedFlags = noritoEncode(typeName: schema, payload: try XCTUnwrap(noritoDecodeFrame(raw)).payload,
      flags: 0, payloadAlignment: 8)
    XCTAssertThrowsError(try KagemushaOrdinaryNativeArchiveV1.decode(fixedFlags, schema: schema, phase: 3, id: 9))
    XCTAssertThrowsError(try KagemushaOrdinaryNativeArchiveV1.decode(raw + Data([0]), schema: schema, phase: 3, id: 9))
    XCTAssertThrowsError(try KagemushaOrdinaryNativeArchiveV1.decode(Data(raw.dropLast()), schema: schema, phase: 3, id: 9))
  }
  func testFiniteFieldCensusAndPhaseInputsAreClosed() throws {
    let schema = KagemushaOrdinaryNativeArchiveV1.startupResponse
    let raw = KagemushaOrdinaryNativeArchiveV1.encode(schema: schema, phase: 1, id: 9,
      fields: Array(repeating: Data(), count: 15))
    XCTAssertThrowsError(try KagemushaOrdinaryNativeArchiveV1.decode(raw, schema: schema, phase: 1, id: nil))
    for phase: UInt8 in [0, 1, 2, 3, 4, 19, 255] {
      XCTAssertThrowsError(try KagemushaOrdinaryOutgoingFrameV1.requireRequest(phase, []))
    }
    XCTAssertThrowsError(try KagemushaOrdinaryOutgoingFrameV1.requireRequest(14, [Data(repeating: 0, count: 32)]))
    XCTAssertThrowsError(try KagemushaOrdinaryOutgoingFrameV1.requireRequest(10, [Data(repeating: 1, count: 4097)]))
    XCTAssertThrowsError(try KagemushaOrdinaryOutgoingFrameV1.requireResponse(14, [Data([1])]))
    XCTAssertThrowsError(try KagemushaOrdinaryOutgoingFrameV1.requireResponse(11, [Data([0]), Data([1]), Data()]))
  }
}
