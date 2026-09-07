import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaCoreCoordinatorArchiveV1Tests: XCTestCase {
  func testRustArchivesRoundTripExactly() throws {
    let fixture = try CoordinatorArchiveFixtureV1()
    XCTAssertEqual(try KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(fixture.preparation), try fixture.bytes("preparation"))
    XCTAssertEqual(try KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(fixture.candidate), try fixture.bytes("candidate"))
    XCTAssertEqual(try KagemushaCoreCoordinatorArchiveV1.encodeRecoveryShape(fixture.recovery), try fixture.bytes("recovery"))
    let receipt = try KagemushaCoreCoordinatorArchiveV1.decodeRedemptionTerminalReceiptShapeExact(fixture.bytes("redemption_terminal_receipt"))
    XCTAssertEqual(try KagemushaCoreCoordinatorArchiveV1.encodeRedemptionTerminalReceiptShape(receipt), try fixture.bytes("redemption_terminal_receipt"))
    XCTAssertEqual(fixture.candidate.preparation, fixture.preparation)
    XCTAssertEqual(fixture.recovery.operationID, fixture.preparation.operationID)
    XCTAssertEqual(fixture.recovery.inputsDigest, fixture.preparation.inputsDigest)
  }

  func testArchivesRejectTailsTruncationsWrongSchemasAndVersions() throws {
    let fixture = try CoordinatorArchiveFixtureV1()
    let cases: [(String, (Data) throws -> Void)] = [
      ("preparation", { _ = try KagemushaCoreCoordinatorArchiveV1.decodePreparationShapeExact($0) }),
      ("candidate", { _ = try KagemushaCoreCoordinatorArchiveV1.decodeCandidateShapeExact($0) }),
      ("recovery", { _ = try KagemushaCoreCoordinatorArchiveV1.decodeRecoveryShapeExact($0) }),
      ("redemption_terminal_receipt", { _ = try KagemushaCoreCoordinatorArchiveV1.decodeRedemptionTerminalReceiptShapeExact($0) }),
    ]
    for (name, decode) in cases {
      let bytes = try fixture.bytes(name)
      for invalid in [Data(), Data(bytes.dropLast()), bytes + Data([0]), Data(repeating: 1, count: 16 * 1024 + 1)] {
        XCTAssertThrowsError(try decode(invalid), name)
      }
      for (other, _) in cases where other != name { XCTAssertThrowsError(try decode(fixture.bytes(other))) }
      let frame = try XCTUnwrap(noritoDecodeFrame(bytes))
      var payload = frame.payload
      XCTAssertEqual(payload[0], 2)
      payload[1] = 2
      let schema = name == "redemption_terminal_receipt"
        ? "iroha.kagemusha.device.v1.redemption-terminal-receipt"
        : "iroha.kagemusha.core.v1.sender-" + name
      let invalidVersion = noritoEncode(typeName: schema, payload: payload,
        flags: NoritoHeader.compactLen, payloadAlignment: name == "redemption_terminal_receipt" ? 8 : 16)
      XCTAssertThrowsError(try decode(invalidVersion), name)
    }
  }

  func testSenderInputDigestMatchesRustAndBindsCallerAndAmount() throws {
    let fixture = try CoordinatorArchiveFixtureV1()
    let preparation = fixture.preparation
    let digest = try KagemushaCoreCoordinatorArchiveV1.senderInputsDigestShape(
      operationID: preparation.operationID, context: preparation.context, inputs: fixture.inputs)
    XCTAssertEqual(digest, preparation.inputsDigest)
    XCTAssertNotEqual(try KagemushaCoreCoordinatorArchiveV1.senderInputsDigestShape(
      operationID: Data(repeating: 9, count: 32), context: preparation.context, inputs: fixture.inputs), digest)
    let beneficiary = try KagemushaNoritoV1.decodePaymentRequestShapeExact(fixture.paymentRequest).recipient
    let redemption = KagemushaDeviceSenderPublicInputsV1.redeemSplit(amount: .init(7), beneficiary: beneficiary)
    XCTAssertNotEqual(try KagemushaCoreCoordinatorArchiveV1.senderInputsDigestShape(
      operationID: preparation.operationID, context: preparation.context, inputs: redemption), digest)
    XCTAssertThrowsError(try KagemushaCoreCoordinatorArchiveV1.senderInputsDigestShape(
      operationID: preparation.operationID, context: preparation.context,
      inputs: .redeemSplit(amount: .init(0), beneficiary: beneficiary)))
    XCTAssertThrowsError(try KagemushaCoreCoordinatorArchiveV1.senderInputsDigestShape(
      operationID: Data(repeating: 0, count: 32), context: preparation.context, inputs: fixture.inputs))
  }

  func testCandidateRejectsSubstitutedInputSelector() throws {
    let fixture = try CoordinatorArchiveFixtureV1()
    let candidate = fixture.candidate
    let invalid = try KagemushaNativeSenderCandidateV1(preparation: candidate.preparation,
      selector: .init(inputsDigest: Data(repeating: 9, count: 32), preparationID: candidate.selector.preparationID),
      candidateDigest: candidate.candidateDigest, hardwareCommitAuthorization: candidate.hardwareCommitAuthorization)
    XCTAssertThrowsError(try KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(invalid))
  }

  func testTerminalEnvelopeDigestBindsAllBytesAndBounds() throws {
    let fixture = try CoordinatorArchiveFixtureV1()
    let digest = try KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(fixture.payment)
    XCTAssertEqual(digest.count, 32)
    XCTAssertNotEqual(try KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(fixture.payment + Data([0])), digest)
    XCTAssertThrowsError(try KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(Data()))
    XCTAssertThrowsError(try KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(Data(repeating: 1, count: 7937)))
  }
}

/// Rust-produced canonical shape fixtures; these do not qualify a native provider.
struct CoordinatorArchiveFixtureV1 {
  private let root: [String: Any]
  let preparation: KagemushaNativeSenderPreparationV1
  let candidate: KagemushaNativeSenderCandidateV1
  let recovery: KagemushaNativeSenderRecoveryV1
  let paymentRequest: Data
  let payment: Data
  let acknowledgement: Data
  var inputs: KagemushaDeviceSenderPublicInputsV1 { .sendSplit(canonicalRequest: paymentRequest) }

  init() throws {
    root = try Self.readJSON("kagemusha_core_coordinator_archives_v1.json")
    XCTAssertEqual(root["schema"] as? String, "iroha.kagemusha.core.v1.archive-fixtures")
    XCTAssertEqual(root["version"] as? Int, 1)
    preparation = try KagemushaCoreCoordinatorArchiveV1.decodePreparationShapeExact(Self.bytes(root, "preparation"))
    candidate = try KagemushaCoreCoordinatorArchiveV1.decodeCandidateShapeExact(Self.bytes(root, "candidate"))
    recovery = try KagemushaCoreCoordinatorArchiveV1.decodeRecoveryShapeExact(Self.bytes(root, "recovery"))
    let paymentFixture = try Self.readJSON("kagemusha_v1.json")
    paymentRequest = try Self.bytes(paymentFixture, "payment_request")
    payment = try Self.bytes(paymentFixture, "payment")
    acknowledgement = try Self.bytes(paymentFixture, "acknowledgement")
  }

  func bytes(_ name: String) throws -> Data { try Self.bytes(root, name) }

  private static func readJSON(_ name: String) throws -> [String: Any] {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while directory.path != "/" {
      let path = directory.appendingPathComponent("fixtures/offline/" + name)
      if FileManager.default.fileExists(atPath: path.path) {
        return try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: path)) as? [String: Any])
      }
      directory.deleteLastPathComponent()
    }
    throw NSError(domain: "missing canonical coordinator fixture", code: 1)
  }

  private static func bytes(_ root: [String: Any], _ name: String) throws -> Data {
    let row = try XCTUnwrap(root[name] as? [String: Any])
    let hex = try XCTUnwrap(row["norito_hex"] as? String)
    guard hex.count.isMultiple(of: 2) else { throw NSError(domain: "invalid fixture hex", code: 1) }
    var result = Data()
    var index = hex.startIndex
    while index < hex.endIndex {
      let end = hex.index(index, offsetBy: 2)
      result.append(try XCTUnwrap(UInt8(hex[index..<end], radix: 16)))
      index = end
    }
    if let length = row["byte_len"] as? Int { XCTAssertEqual(result.count, length) }
    return result
  }
}
