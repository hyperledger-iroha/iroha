import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaTestnetFinalizedMintObservationV1Tests: XCTestCase {
  private let operationID = Data(repeating: 7, count: 32)
  private let originalStatusJSON = Data(" { \"state\": \"Applied\" }\n".utf8)
  private let publicInputs = Data([1])
  private let pairedProof = Data([2])

  private var anchor: KagemushaFinalityTrustAnchorV1 {
    get throws {
      try KagemushaFinalityTrustAnchorV1(
        networkID: Data(repeating: 3, count: 32), blockHeight: 7,
        heightContextID: Data(repeating: 5, count: 32))
    }
  }

  func testInvalidInputsNeverReachNativeEndpoint() throws {
    let endpoint = Endpoint(status: 0, archive: archive())
    let anchor = try self.anchor
    let invalid: [(Data, Data, Data, Data)] = [
      (Data(), originalStatusJSON, publicInputs, pairedProof),
      (Data(repeating: 0, count: 32), originalStatusJSON, publicInputs, pairedProof),
      (Data(repeating: 7, count: 31), originalStatusJSON, publicInputs, pairedProof),
      (Data(repeating: 7, count: 33), originalStatusJSON, publicInputs, pairedProof),
      (operationID, Data(), publicInputs, pairedProof),
      (operationID, Data(repeating: 1, count: 16_777_217), publicInputs, pairedProof),
      (operationID, originalStatusJSON, Data(), pairedProof),
      (operationID, originalStatusJSON, Data(repeating: 1, count: 4_097), pairedProof),
      (operationID, originalStatusJSON, publicInputs, Data()),
      (operationID, originalStatusJSON, publicInputs, Data(repeating: 2, count: 6_529)),
    ]
    for (operationID, originalStatusJSON, publicInputs, pairedProof) in invalid {
      XCTAssertThrowsError(try KagemushaTestnetFinalizedMintObservationBridgeV1.observe(
        operationID: operationID, originalStatusJSON: originalStatusJSON,
        trustAnchor: anchor, publicInputsArchive: publicInputs,
        pairedProofArchive: pairedProof, endpoint: endpoint)) { error in
          XCTAssertEqual(error as? KagemushaTestnetFinalizedMintObservationErrorV1, .invalidInput)
        }
    }
    XCTAssertEqual(endpoint.calls, 0)
  }

  func testAnchorRequiresExactMarkedHashesAndPositiveHeight() {
    for (network, height, context) in [
      (Data(repeating: 3, count: 31), UInt64(7), Data(repeating: 5, count: 32)),
      (Data(repeating: 2, count: 32), UInt64(7), Data(repeating: 5, count: 32)),
      (Data(repeating: 3, count: 32), UInt64(0), Data(repeating: 5, count: 32)),
      (Data(repeating: 3, count: 32), UInt64(7), Data(repeating: 5, count: 31)),
      (Data(repeating: 3, count: 32), UInt64(7), Data(repeating: 4, count: 32)),
    ] {
      XCTAssertThrowsError(try KagemushaFinalityTrustAnchorV1(
        networkID: network, blockHeight: height, heightContextID: context))
    }
  }

  func testMissingOwnerAndNativeRejectionAreDistinct() throws {
    let anchor = try self.anchor
    for (status, expected) in [
      (-312, KagemushaTestnetFinalizedMintObservationErrorV1.ownerUnavailable),
      (-311, KagemushaTestnetFinalizedMintObservationErrorV1.nativeRejected(-311)),
    ] {
      let endpoint = Endpoint(status: status, archive: archive())
      XCTAssertThrowsError(try KagemushaTestnetFinalizedMintObservationBridgeV1.observe(
        operationID: operationID, originalStatusJSON: originalStatusJSON,
        trustAnchor: anchor, publicInputsArchive: publicInputs,
        pairedProofArchive: pairedProof, endpoint: endpoint)) { error in
          XCTAssertEqual(error as? KagemushaTestnetFinalizedMintObservationErrorV1, expected)
        }
      XCTAssertEqual(endpoint.calls, 1)
    }
  }

  func testMalformedOrWrongSchemaOutputFailsClosed() throws {
    let anchor = try self.anchor
    let invalid = [
      Data(),
      Data(repeating: 3, count: 513),
      noritoEncode(typeName: "wrong.schema", payload: Data([1]),
                   flags: NoritoHeader.compactLen),
      noritoEncode(
        typeName: "connect_norito_bridge::KagemushaTestnetFinalizedMintObservationArchiveV1",
        payload: Data([1]), flags: 0),
    ]
    for archive in invalid {
      let endpoint = Endpoint(status: 0, archive: archive)
      XCTAssertThrowsError(try KagemushaTestnetFinalizedMintObservationBridgeV1.observe(
        operationID: operationID, originalStatusJSON: originalStatusJSON,
        trustAnchor: anchor, publicInputsArchive: publicInputs,
        pairedProofArchive: pairedProof, endpoint: endpoint)) { error in
          XCTAssertEqual(error as? KagemushaTestnetFinalizedMintObservationErrorV1,
                         .invalidObservation)
        }
    }
  }

  func testOriginalBytesPassThroughAndOpaqueArchiveStaysUnqualified() throws {
    let anchor = try self.anchor
    let encoded = archive()
    let endpoint = Endpoint(status: 0, archive: encoded)
    let observation = try KagemushaTestnetFinalizedMintObservationBridgeV1.observe(
      operationID: operationID, originalStatusJSON: originalStatusJSON,
      trustAnchor: anchor, publicInputsArchive: publicInputs,
      pairedProofArchive: pairedProof, endpoint: endpoint)
    XCTAssertEqual(endpoint.calls, 1)
    XCTAssertEqual(endpoint.operationID, operationID)
    XCTAssertEqual(endpoint.originalStatusJSON, originalStatusJSON)
    XCTAssertEqual(endpoint.trustAnchor, anchor)
    XCTAssertEqual(endpoint.publicInputsArchive, publicInputs)
    XCTAssertEqual(endpoint.pairedProofArchive, pairedProof)
    XCTAssertEqual(observation.canonicalArchive, encoded)
    XCTAssertFalse(observation.hardwareQualified)
    XCTAssertFalse(observation.monetaryAuthorized)
  }

  private func archive() -> Data {
    // The mock tests framing only; native owns the complete archived field layout.
    noritoEncode(
      typeName: "connect_norito_bridge::KagemushaTestnetFinalizedMintObservationArchiveV1",
      payload: Data([1]), flags: NoritoHeader.compactLen)
  }

  private final class Endpoint: KagemushaTestnetFinalizedMintObservationEndpointV1 {
    let status: Int32
    let archive: Data
    var calls = 0
    var operationID: Data?
    var originalStatusJSON: Data?
    var trustAnchor: KagemushaFinalityTrustAnchorV1?
    var publicInputsArchive: Data?
    var pairedProofArchive: Data?

    init(status: Int32, archive: Data) {
      self.status = status
      self.archive = archive
    }

    func observe(
      operationID: Data, originalStatusJSON: Data,
      trustAnchor: KagemushaFinalityTrustAnchorV1,
      publicInputsArchive: Data, pairedProofArchive: Data
    ) -> (status: Int32, archive: Data) {
      calls += 1
      self.operationID = operationID
      self.originalStatusJSON = originalStatusJSON
      self.trustAnchor = trustAnchor
      self.publicInputsArchive = publicInputsArchive
      self.pairedProofArchive = pairedProofArchive
      return (status, archive)
    }
  }
}
