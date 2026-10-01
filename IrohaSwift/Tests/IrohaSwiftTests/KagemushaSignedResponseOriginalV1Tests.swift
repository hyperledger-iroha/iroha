import Foundation
import XCTest
@testable import IrohaSwift

// Public synthetic framing only. These helpers perform no native authentication and
// provide no device qualification, release admission, proof or monetary authority.
func testSignedDeviceResponseFrame(operation: UInt8, status: KagemushaDeviceLifecycleStatusV1,
  requestID: Data, payload: Data, authenticator: Data) throws -> Data {
  guard let selected = KagemushaDeviceLifecycleOperationV1(rawValue: operation) else {
    throw KagemushaAuthenticatedHardwareProviderErrorV1.invalidContract("invalid synthetic operation")
  }
  return KagemushaDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(operation: selected,
    status: status, requestID: requestID, payload: payload, authenticator: authenticator)
}

func testAuthenticatedDeviceResponse(operation: UInt8, status: KagemushaDeviceLifecycleStatusV1,
  canonicalReply: Data, authenticator: Data, requestID: Data = Data(repeating: 1, count: 32)) throws
  -> KagemushaAuthenticatedDeviceResponseV1 {
  let original = try testSignedDeviceResponseFrame(operation: operation, status: status,
    requestID: requestID, payload: canonicalReply, authenticator: authenticator)
  return try KagemushaAuthenticatedDeviceResponseV1(operation: operation, status: status,
    canonicalReply: canonicalReply, authenticator: authenticator, requestID: requestID,
    canonicalResponseFrame: original)
}

final class KagemushaSignedResponseOriginalV1Tests: XCTestCase {
  private let id = Data(repeating: 0x31, count: 32)
  private let reply = Data([0x41, 0x42])
  private var signature: Data {
    var value = Data(repeating: 0, count: 64); value[31] = 1; value[63] = 1; return value
  }
  private func frame(operation: UInt8 = 7, requestID: Data? = nil,
    payload: Data? = nil, authenticator: Data? = nil) throws -> Data {
    try testSignedDeviceResponseFrame(operation: operation, status: .success,
      requestID: requestID ?? id, payload: payload ?? reply, authenticator: authenticator ?? signature)
  }

  func testAuthenticatedResponseRetainsExactOriginalWithDefensiveOwnership() throws {
    var original = try frame()
    let retained = original
    let value = try KagemushaAuthenticatedDeviceResponseV1(operation: 7, status: .success,
      canonicalReply: reply, authenticator: signature, requestID: id, canonicalResponseFrame: original)
    original[0] ^= 1
    var callerCopy = value.canonicalResponseFrame; callerCopy[1] ^= 1
    XCTAssertEqual(value.canonicalResponseFrame, retained)
    XCTAssertNoThrow(try value.recheckOriginal(requestID: id))
    XCTAssertThrowsError(try value.recheckOriginal(requestID: Data(repeating: 0x32, count: 32)))
  }

  func testAuthenticatedFieldsCannotBeJoinedToSubstitutedOriginal() throws {
    var changedSignature = signature; changedSignature[63] = 2
    let originals = try [frame(operation: 8), frame(requestID: Data(repeating: 0x32, count: 32)),
      frame(payload: Data([0x43])), frame(authenticator: changedSignature)]
    for original in originals {
      XCTAssertThrowsError(try KagemushaAuthenticatedDeviceResponseV1(operation: 7, status: .success,
        canonicalReply: reply, authenticator: signature, requestID: id, canonicalResponseFrame: original))
    }
  }

  func testPartialDigestTamperedAndTrailingOriginalsCannotBeAccepted() throws {
    let original = try frame()
    var digestTamper = original; digestTamper[52] ^= 1
    for malformed in [Data(), Data(original.dropLast()), original + Data([0]), digestTamper] {
      XCTAssertThrowsError(try KagemushaAuthenticatedDeviceResponseV1(operation: 7, status: .success,
        canonicalReply: reply, authenticator: signature, requestID: id, canonicalResponseFrame: malformed))
    }
  }

  func testSerializedIntentRetainsOriginalAndRejectsMissingLegacyOrSubstitution() throws {
    let original = try frame()
    var value = try KagemushaOperationIntentV1(applicationScope: Data([1]), qualificationScope: Data([2]),
      operation: 7, operationID: id, purpose: "original7", arguments: Data(), publicBinding: Data([3]),
      canonicalCommand: Data([4]))
    value.canonicalReply = reply; value.responseAuthenticator = signature; value.originalResponse = original
    let wire = try JSONEncoder().encode(value)
    let reopened = try JSONDecoder().decode(KagemushaOperationIntentV1.self, from: wire)
    XCTAssertNoThrow(try reopened.validate())
    XCTAssertEqual(reopened.originalResponse, original)
    var incomplete = reopened; incomplete.originalResponse = nil
    XCTAssertThrowsError(try incomplete.validate())
    for substitute in try [frame(operation: 8), frame(requestID: Data(repeating: 0x32, count: 32)),
      frame(payload: Data([0x43]))] {
      var changed = reopened; changed.originalResponse = substitute
      XCTAssertThrowsError(try changed.validate())
      XCTAssertThrowsError(try changed.validateSuccessor(of: reopened))
    }
    var legacy = try XCTUnwrap(JSONSerialization.jsonObject(with: wire) as? [String: Any])
    legacy["version"] = 2
    let legacyDecoded = try JSONDecoder().decode(KagemushaOperationIntentV1.self,
      from: JSONSerialization.data(withJSONObject: legacy))
    XCTAssertThrowsError(try legacyDecoded.validate())
  }

  func testDurableAcceptedOriginalSurvivesLostSaveAndExactRetry() throws {
    let store = TestOperationIntentStore(), scope = Data([2]), command = Data([3])
    let owner = KagemushaOperationIntentOwnerV1(store: store)
    try owner.willDispatch(operation: 7, operationID: id, command: command, qualificationScope: scope)
    let original = try frame()
    store.failAfterSave = true
    XCTAssertThrowsError(try owner.accepted(operation: 7, operationID: id, command: command,
      reply: reply, authenticator: signature, originalResponse: original, qualificationScope: scope))
    store.failAfterSave = false
    let reopened = KagemushaOperationIntentOwnerV1(store: store)
    store.failAfterSave = true
    XCTAssertNoThrow(try reopened.accepted(operation: 7, operationID: id, command: command,
      reply: reply, authenticator: signature, originalResponse: original, qualificationScope: scope))
    XCTAssertEqual(try store.load(operation: 7, operationID: id)?.originalResponse, original)
    XCTAssertThrowsError(try reopened.accepted(operation: 7, operationID: id, command: command,
      reply: reply, authenticator: signature, originalResponse: try frame(operation: 8), qualificationScope: scope))
    XCTAssertEqual(try store.load(operation: 7, operationID: id)?.originalResponse, original)
  }

  func testMethod6RequiresBoundedOriginalSuccessForExactCandidateOperation() throws {
    let fixture = try CoordinatorArchiveFixtureV1()
    let operationID = fixture.candidate.preparation.operationID
    let candidate = try KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(fixture.candidate)
    let original = try frame(requestID: operationID)
    let request = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.buildTerminalEnvelope,
      fields: [candidate, original])
    XCTAssertEqual(try KagemushaCoreCoordinatorFrameV1.decodeRequest(.buildTerminalEnvelope, frame: request),
      [candidate, original])
    let failure = try testSignedDeviceResponseFrame(operation: 7, status: .unavailable,
      requestID: operationID, payload: Data(), authenticator: Data())
    for changed in try [reply, frame(operation: 8, requestID: operationID), frame(),
      failure, original + Data([0]), Data(repeating: 1, count: 65_717)] {
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.buildTerminalEnvelope,
        fields: [candidate, changed]))
    }
  }
}
