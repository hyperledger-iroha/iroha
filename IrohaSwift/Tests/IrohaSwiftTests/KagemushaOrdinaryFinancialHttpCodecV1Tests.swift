import Foundation
import XCTest
@testable import IrohaSwift

/// Untrusted HTTP DATA controls. No response here is admitted by Native or a financial service.
final class KagemushaOrdinaryFinancialHttpCodecV1Tests: XCTestCase {
  func testBothExactRequestEnvelopesKeepOriginalsAndDifferentSchemas() throws {
    let request = Data([1, 2]), signature = Data(repeating: 3, count: 64), proof = Data([4])
    for kind: KagemushaOrdinaryFinancialHttpCodecV1.Kind in [.currentControl, .lineage] {
      let raw = try KagemushaOrdinaryFinancialHttpCodecV1.requestBody(kind, request: request,
        signature: signature, proof: kind == .lineage ? proof : Data())
      let object = try XCTUnwrap(JSONSerialization.jsonObject(with: raw) as? [String: String])
      XCTAssertEqual(object["canonical_request_base64"], request.base64EncodedString())
      XCTAssertEqual(object["account_signature_base64"], signature.base64EncodedString())
      XCTAssertEqual(object.count, kind == .lineage ? 4 : 3)
      XCTAssertEqual(object["proof_bundle_original_base64"], kind == .lineage ? proof.base64EncodedString() : nil)
    }
  }
  func testStableRequestCorrelationUsesSameExactOriginalAndSeparateDomain() throws {
    let original = Data([1, 2, 3])
    let first = try KagemushaOrdinaryFinancialHttpCodecV1.requestID(.currentControl, request: original)
    XCTAssertEqual(first, try KagemushaOrdinaryFinancialHttpCodecV1.requestID(.currentControl, request: original))
    XCTAssertNotEqual(first, try KagemushaOrdinaryFinancialHttpCodecV1.requestID(.currentControl, request: original + Data([0])))
    XCTAssertNotEqual(first, try KagemushaOrdinaryFinancialHttpCodecV1.requestID(.lineage, request: original))
    XCTAssertEqual(UUID(uuidString: first)?.uuidString.lowercased(), first)
  }
  func testUnknownDuplicateAndEscapedDuplicateKeysCannotBecomeOriginals() throws {
    for text in [
      "{\"signed_control_original_base64\":\"AQ==\",\"authority_original_base64\":\"Ag==\",\"extra\":1}",
      "{\"signed_control_original_base64\":\"AQ==\",\"signed_control_original_base64\":\"Ag==\",\"authority_original_base64\":\"Aw==\"}",
      "{\"signed_control_original_base64\":\"AQ==\",\"signed_control_original_base6\\u0034\":\"Ag==\",\"authority_original_base64\":\"Aw==\"}",
      "[]", "null"
    ] { XCTAssertThrowsError(try KagemushaOrdinaryFinancialHttpCodecV1.responseOriginals(.currentControl, raw: Data(text.utf8))) }
  }
  func testOriginalsRejectAlternateBase64NonTextEmptyAndWrongEnvelope() throws {
    for value: Any in ["AQ", "AR==", "AQ==\n", "", 1, NSNull()] {
      let raw = try JSONSerialization.data(withJSONObject: ["signed_control_original_base64": value,
        "authority_original_base64": "Ag=="])
      XCTAssertThrowsError(try KagemushaOrdinaryFinancialHttpCodecV1.responseOriginals(.currentControl, raw: raw))
    }
    let raw = try JSONSerialization.data(withJSONObject: ["signed_control_original_base64": "AQ==",
      "authority_original_base64": "Ag=="])
    XCTAssertEqual(try KagemushaOrdinaryFinancialHttpCodecV1.responseOriginals(.currentControl, raw: raw), [Data([1]), Data([2])])
    XCTAssertThrowsError(try KagemushaOrdinaryFinancialHttpCodecV1.responseOriginals(.lineage, raw: raw))
    XCTAssertThrowsError(try KagemushaOrdinaryFinancialHttpCodecV1.responseOriginals(.currentControl, raw: Data([0xff])))
  }
  func testRequestAndIndividualOriginalBoundsRejectBeforeAdmission() throws {
    let signature = Data(repeating: 1, count: 64)
    XCTAssertThrowsError(try KagemushaOrdinaryFinancialHttpCodecV1.requestBody(.currentControl,
      request: Data(repeating: 1, count: 8193), signature: signature))
    XCTAssertThrowsError(try KagemushaOrdinaryFinancialHttpCodecV1.requestBody(.currentControl,
      request: Data([1]), signature: Data(repeating: 1, count: 63)))
    XCTAssertThrowsError(try KagemushaOrdinaryFinancialHttpCodecV1.requestBody(.lineage,
      request: Data([1]), signature: signature, proof: Data()))
    let raw = try JSONSerialization.data(withJSONObject: ["signed_control_original_base64":
      Data(repeating: 1, count: 65_537).base64EncodedString(), "authority_original_base64": "Ag=="])
    XCTAssertThrowsError(try KagemushaOrdinaryFinancialHttpCodecV1.responseOriginals(.currentControl, raw: raw))
  }
}
