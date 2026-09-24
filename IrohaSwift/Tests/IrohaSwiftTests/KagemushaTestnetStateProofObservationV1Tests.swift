import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaTestnetStateProofObservationV1Tests: XCTestCase {
  private let publicInputs = Data([1])
  private let pairedProof = Data([2])

  func testBoundedInputsNeverReachNativeEndpoint() {
    let endpoint = Endpoint(status: 0, archive: archive())
    for (publicInputs, pairedProof) in [
      (Data(), self.pairedProof),
      (Data(repeating: 1, count: 4_097), self.pairedProof),
      (self.publicInputs, Data()),
      (self.publicInputs, Data(repeating: 2, count: 6_529)),
    ] {
      XCTAssertThrowsError(try KagemushaTestnetStateProofObservationBridgeV1.observe(
        publicInputsArchive: publicInputs, pairedProofArchive: pairedProof,
        endpoint: endpoint)) { error in
          XCTAssertEqual(error as? KagemushaTestnetStateProofObservationErrorV1, .invalidInput)
        }
    }
    XCTAssertEqual(endpoint.calls, 0)
  }

  func testMissingNativeOwnerFailsClosed() {
    let endpoint = Endpoint(status: -312, archive: Data())
    XCTAssertThrowsError(try KagemushaTestnetStateProofObservationBridgeV1.observe(
      publicInputsArchive: publicInputs, pairedProofArchive: pairedProof,
      endpoint: endpoint)) { error in
        XCTAssertEqual(error as? KagemushaTestnetStateProofObservationErrorV1, .ownerUnavailable)
      }
    XCTAssertEqual(endpoint.calls, 1)
  }

  func testNativeRejectionAndInvalidOutputNeverBecomeObservation() {
    let cases: [(Int32, Data, KagemushaTestnetStateProofObservationErrorV1)] = [
      (-310, self.archive(), KagemushaTestnetStateProofObservationErrorV1.nativeRejected(-310)),
      (0, Data(), .invalidObservation),
      (0, Data(repeating: 3, count: 257), .invalidObservation),
      (0, noritoEncode(typeName: "wrong.schema", payload: Data([1]),
                       flags: NoritoHeader.compactLen), .invalidObservation),
      (0, noritoEncode(
        typeName: "connect_norito_bridge::KagemushaTestnetStateObservationArchiveV1",
        payload: Data([1]), flags: 0), .invalidObservation),
    ]
    for (status, archive, expected) in cases {
      let endpoint = Endpoint(status: status, archive: archive)
      XCTAssertThrowsError(try KagemushaTestnetStateProofObservationBridgeV1.observe(
        publicInputsArchive: publicInputs, pairedProofArchive: pairedProof,
        endpoint: endpoint)) { error in
          XCTAssertEqual(error as? KagemushaTestnetStateProofObservationErrorV1, expected)
        }
    }
  }

  func testObservationStaysUnsignedAndNonmonetary() throws {
    // This synthetic archive tests only Swift framing; proof verification is
    // exclusively the installed native owner's responsibility.
    let encoded = archive()
    let endpoint = Endpoint(status: 0, archive: encoded)
    let observation = try KagemushaTestnetStateProofObservationBridgeV1.observe(
      publicInputsArchive: publicInputs, pairedProofArchive: pairedProof,
      endpoint: endpoint)
    XCTAssertEqual(observation.canonicalArchive, encoded)
    XCTAssertFalse(observation.hardwareQualified)
    XCTAssertFalse(observation.monetaryAuthorized)
    XCTAssertEqual(endpoint.calls, 1)
  }

  private func archive() -> Data {
    noritoEncode(
      typeName: "connect_norito_bridge::KagemushaTestnetStateObservationArchiveV1",
      payload: Data([1]), flags: NoritoHeader.compactLen)
  }

  private final class Endpoint: KagemushaTestnetStateProofObservationEndpointV1 {
    let status: Int32
    let archive: Data
    var calls = 0

    init(status: Int32, archive: Data) {
      self.status = status
      self.archive = archive
    }

    func observe(publicInputsArchive: Data, pairedProofArchive: Data)
      -> (status: Int32, archive: Data)
    {
      calls += 1
      return (status, archive)
    }
  }
}
