import Foundation
import XCTest

@testable import IrohaSwift

final class KagemushaAuthenticatedHardwareProviderV1Tests: XCTestCase {
  func testAuthenticatedResponseRequiresCanonicalReplyAndLowSSignature() throws {
    let signature = lowSSignature()
    for operation in UInt8(1)...UInt8(22) {
      XCTAssertNoThrow(
        try KagemushaAuthenticatedDeviceResponseV1(
          operation: operation,
          status: .success,
          canonicalReply: Data([1]),
          authenticator: signature
        )
      )
    }
    XCTAssertThrowsError(
      try KagemushaAuthenticatedDeviceResponseV1(
        operation: 1,
        status: .success,
        canonicalReply: Data(),
        authenticator: signature
      )
    )
    XCTAssertThrowsError(
      try KagemushaAuthenticatedDeviceResponseV1(
        operation: 1,
        status: .success,
        canonicalReply: Data([1]),
        authenticator: Data(repeating: 0xff, count: 64)
      )
    )
    for operation in [UInt8(0), UInt8(23), UInt8.max] {
      XCTAssertThrowsError(
        try KagemushaAuthenticatedDeviceResponseV1(
          operation: operation,
          status: .success,
          canonicalReply: Data([1]),
          authenticator: signature
        )
      )
    }
  }

  func testFailedResponseCannotExposeUnauthenticatedBytes() throws {
    XCTAssertNoThrow(
      try KagemushaAuthenticatedDeviceResponseV1(
        operation: 21,
        status: .missing,
        canonicalReply: Data(),
        authenticator: Data()
      )
    )
    XCTAssertThrowsError(
      try KagemushaAuthenticatedDeviceResponseV1(
        operation: 21,
        status: .missing,
        canonicalReply: Data([1]),
        authenticator: Data()
      )
    )
  }

  func testOnlineOnlyLifecycleBridgeCannotQualifyAsAuthenticatedTransport() throws {
    let transport: any KagemushaNativeAuthenticatedDeviceTransportV1 =
      KagemushaDeviceLifecycleBridgeV1.onlineOnly()
    XCTAssertThrowsError(try transport.hardwarePolicyID()) { error in
      XCTAssertEqual(error as? KagemushaDeviceLifecycleBridgeErrorV1, .onlineOnly)
    }
    XCTAssertThrowsError(try transport.qualificationReportDigest()) { error in
      XCTAssertEqual(error as? KagemushaDeviceLifecycleBridgeErrorV1, .onlineOnly)
    }
    XCTAssertThrowsError(
      try transport.executeAndVerify(
        operation: 1,
        requestID: Data(repeating: 1, count: 32),
        canonicalCommand: Data([1]),
        acceptedDevicePublicKey: nil
      )
    ) { error in
      XCTAssertEqual(error as? KagemushaDeviceLifecycleBridgeErrorV1, .onlineOnly)
    }
  }

  func testUnavailableOperationOneKeepsHighLevelProviderClosed() throws {
    let transport = UnavailableAuthenticatedTransport()
    let core = RecordingNativeCore()
    let provider = KagemushaAuthenticatedHardwareProviderV1(
      transport: transport,
      core: core, intentOwner: testOperationIntentOwner()
    )

    XCTAssertThrowsError(try provider.qualification()) { error in
      XCTAssertEqual(
        error as? KagemushaAuthenticatedHardwareProviderErrorV1,
        .operationFailed(
          operation: 1,
          status: .unavailable
        )
      )
    }
    XCTAssertEqual(core.reservedOperations, [])
    XCTAssertFalse(core.acceptedQualification)
    XCTAssertEqual(transport.operations, [1])
    XCTAssertEqual(transport.acceptedKeys, [nil])
  }

  func testSenderReservationsMatchCanonicalNativeCoreBindings() throws {
    let fixture = try reservationFixture()
    let core = RecordingNativeCore()
    let transport = try QualificationOnlyTransport()
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: core, intentOwner: testOperationIntentOwner())
    let operationID = Data(repeating: 7, count: 32)
    XCTAssertEqual(
      try provider.reservePaymentOperationID(
        operationID: operationID, canonicalRequest: fixtureBytes(fixture, "send_request_hex")),
      operationID)
    XCTAssertEqual(core.reservations.last?.2, try fixtureBytes(fixture, "send_binding_hex"))
    let amount = try XCTUnwrap(UInt64(try XCTUnwrap(fixture["redeem_amount_decimal"])))
    let beneficiary = try KagemushaAccountIDV1(
      canonicalPayload: fixtureBytes(fixture, "redeem_beneficiary_payload_hex"))
    XCTAssertEqual(
      try provider.reserveRedemptionOperationID(
        operationID: Data(repeating: 6, count: 32), amount: KagemushaUInt128V1(amount), beneficiary: beneficiary),
      Data(repeating: 6, count: 32))
    XCTAssertEqual(core.reservations.last?.2, try fixtureBytes(fixture, "redeem_binding_hex"))
    XCTAssertEqual(core.reservedOperations, [5, 5])
    XCTAssertEqual(transport.operations, [1])
  }

  func testRequestAndMintReservationsRetainCallerOwnedIdentity() throws {
    let fixture = try reservationFixture()
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
      fixtureBytes(fixture, "send_request_hex"))
    let core = RecordingNativeCore()
    let provider = KagemushaAuthenticatedHardwareProviderV1(
      transport: try QualificationOnlyTransport(), core: core, intentOwner: testOperationIntentOwner())
    let operationID = Data(repeating: 8, count: 32)
    for _ in 0..<2 {
      XCTAssertEqual(
        try provider.reservePaymentRequestOperationID(
          operationID: operationID, recipient: request.recipient,
          amount: request.amount, validityWindowMS: 1000), operationID)
    }
    XCTAssertEqual(core.reservations[0].2, core.reservations[1].2)
    XCTAssertEqual(
      core.reservations[0].2,
      try KagemushaDeviceOperationCodecV1.encodeControlCommand(
        .createSignedPaymentRequest(
          requestID: operationID, recipient: request.recipient,
          amount: request.amount, validityWindowMS: 1000)))
    XCTAssertEqual(
      try provider.reserveMintOperationID(
        operationID: Data(repeating: 10, count: 32), amount: request.amount,
        payer: request.recipient, recipient: request.recipient), Data(repeating: 10, count: 32))
    XCTAssertEqual(core.reservedOperations, [22, 22, 14])
    XCTAssertTrue(core.reservations.prefix(2).allSatisfy { $0.1 == operationID })
    XCTAssertEqual(core.reservations.last?.1, Data(repeating: 10, count: 32))
  }

  func testSubstitutedReservationsFailBeforeDeviceExecution() throws {
    let requestBytes = try fixtureBytes(reservationFixture(), "send_request_hex")
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(requestBytes)
    let core = RecordingNativeCore()
    core.substituteReservedID = true
    let transport = try QualificationOnlyTransport()
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: core, intentOwner: testOperationIntentOwner())
    let operationID = Data(repeating: 9, count: 32)
    XCTAssertNoThrow(try provider.qualification())
    XCTAssertThrowsError(
      try provider.reservePaymentOperationID(operationID: operationID, canonicalRequest: requestBytes))
    XCTAssertThrowsError(
      try provider.reservePaymentRequestOperationID(
        operationID: operationID, recipient: request.recipient,
        amount: request.amount, validityWindowMS: 1000))
    XCTAssertThrowsError(
      try provider.reserveMintOperationID(
        operationID: operationID, amount: request.amount,
        payer: request.recipient, recipient: request.recipient))
    XCTAssertThrowsError(
      try provider.reserveRedemptionOperationID(
        operationID: operationID, amount: request.amount, beneficiary: request.recipient))
    XCTAssertThrowsError(
      try provider.reservePaymentOperationID(
        operationID: Data(repeating: 0, count: 32), canonicalRequest: requestBytes))
    XCTAssertEqual(transport.operations, [1])
  }

  func testRequestExecutionReservesCallerIntentBeforeReachingHardware() throws {
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
      fixtureBytes(reservationFixture(), "send_request_hex"))
    let core = RecordingNativeCore()
    core.substituteReservedID = true
    let transport = try QualificationOnlyTransport()
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: core, intentOwner: testOperationIntentOwner())
    let operationID = Data(repeating: 12, count: 32)
    XCTAssertThrowsError(
      try provider.createPaymentRequest(
        operationID: operationID, recipient: request.recipient,
        amount: request.amount, validityWindowMS: 1000))
    XCTAssertEqual(core.reservedOperations, [22])
    XCTAssertEqual(core.reservations.first?.1, operationID)
    XCTAssertEqual(transport.operations, [1])
  }

  private func reservationFixture() throws -> [String: String] {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while directory.path != "/" {
      let path = directory.appendingPathComponent("fixtures/offline/kagemusha_sender_reservation_v1.json")
      if FileManager.default.fileExists(atPath: path.path) {
        return try JSONDecoder().decode([String: String].self, from: Data(contentsOf: path))
      }
      directory.deleteLastPathComponent()
    }
    throw NSError(domain: "missing sender reservation fixture", code: 1)
  }

  private func fixtureBytes(_ fixture: [String: String], _ key: String) throws -> Data {
    let hex = try XCTUnwrap(fixture[key])
    var result = Data()
    var index = hex.startIndex
    while index < hex.endIndex {
      let end = hex.index(index, offsetBy: 2)
      result.append(try XCTUnwrap(UInt8(hex[index..<end], radix: 16)))
      index = end
    }
    return result
  }

  private func lowSSignature() -> Data {
    var scalar = Data(repeating: 0, count: 32)
    scalar[31] = 1
    return scalar + scalar
  }
}

private final class UnavailableAuthenticatedTransport:
  KagemushaNativeAuthenticatedDeviceTransportV1
{
  var operations: [UInt8] = []
  var acceptedKeys: [Data?] = []

  func hardwarePolicyID() throws -> Data { Data(repeating: 2, count: 32) }

  func qualificationReportDigest() throws -> Data { Data(repeating: 3, count: 32) }

  func executeAndVerify(
    operation: UInt8,
    requestID _: Data,
    canonicalCommand _: Data,
    acceptedDevicePublicKey: Data?
  ) throws -> KagemushaAuthenticatedDeviceResponseV1 {
    operations.append(operation)
    acceptedKeys.append(acceptedDevicePublicKey)
    return try KagemushaAuthenticatedDeviceResponseV1(
      operation: operation,
      status: .unavailable,
      canonicalReply: Data(),
      authenticator: Data()
    )
  }
}

private final class RecordingNativeCore: KagemushaNativeCoreCoordinatorV1 {
  var reservedOperations: [UInt8] = []
  var acceptedQualification = false
  var substituteReservedID = false
  var reservations: [(UInt8, Data, Data)] = []

  func beginObservation(operation: UInt8, canonicalCommand: Data) throws -> Data {
    Data((0..<32).map { _ in UInt8.random(in: 1...255) })
  }

  func reserveOperationID(operation: UInt8, operationID: Data, publicBinding: Data) throws -> Data {
    reservedOperations.append(operation)
    reservations.append((operation, operationID, publicBinding))
    return substituteReservedID ? Data(repeating: 0xff, count: 32) : operationID
  }

  func acceptQualification(
    _: KagemushaHardwareQualificationV1,
    hardwarePolicyDigest _: Data
  ) throws {
    acceptedQualification = true
  }

  func acceptAuthenticatedDeviceReply(
    operation _: UInt8,
    requestID _: Data,
    canonicalCommand _: Data,
    canonicalReply _: Data,
    responseAuthenticator _: Data,
    qualification _: KagemushaHardwareQualificationV1
  ) throws {}

  func beginSenderTransition(
    operationID _: Data,
    inputs _: KagemushaDeviceSenderPublicInputsV1,
    qualification _: KagemushaHardwareQualificationV1
  ) throws -> KagemushaNativeSenderPreparationV1 { throw TestFailure.unused }

  func provePreparedSenderTransition(
    preparation _: KagemushaNativeSenderPreparationV1,
    authenticatedPreparationReply _: Data
  ) throws -> KagemushaNativeSenderCandidateV1 { throw TestFailure.unused }

  func terminalEnvelope(
    candidate _: KagemushaNativeSenderCandidateV1,
    authenticatedCommitReply _: Data
  ) throws -> Data { throw TestFailure.unused }

  func acceptInstalledTerminal(
    candidate _: KagemushaNativeSenderCandidateV1,
    canonicalEnvelope _: Data,
    authenticatedInstallReply _: Data,
    authenticatedInstalledReply _: Data,
    authenticatedWalletSnapshotReply _: Data
  ) throws -> KagemushaHardwareTerminalResultV1 { throw TestFailure.unused }

  func senderRecovery(
    kind _: KagemushaNativeSenderKindV1,
    terminalID _: Data,
    qualification _: KagemushaHardwareQualificationV1
  ) throws -> KagemushaNativeSenderRecoveryV1? { throw TestFailure.unused }

  func senderRecoveryByOperationID(
    kind _: KagemushaNativeSenderKindV1,
    operationID _: Data,
    qualification _: KagemushaHardwareQualificationV1
  ) throws -> KagemushaNativeSenderRecoveryV1? { throw TestFailure.unused }

  func recoverTerminalEnvelope(
    recovery _: KagemushaNativeSenderRecoveryV1,
    authenticatedInstalledReply _: Data
  ) throws -> Data { throw TestFailure.unused }

  func outboxRelease(
    creditID _: Data,
    inputs _: KagemushaDeviceSenderPublicInputsV1,
    canonicalPayment _: Data,
    terminalReceipt _: KagemushaDeviceSenderTerminalReceiptV1,
    qualification _: KagemushaHardwareQualificationV1
  ) throws -> KagemushaNativeOutboxReleaseV1 { throw TestFailure.unused }

  private enum TestFailure: Error { case unused }
}

private final class QualificationOnlyTransport: KagemushaNativeAuthenticatedDeviceTransportV1 {
  let qualification: KagemushaHardwareQualificationV1
  var operations: [UInt8] = []
  init() throws { qualification = try AuthenticatedProviderFixtureV1().qualification }
  func hardwarePolicyID() throws -> Data { qualification.hardwarePolicyDigest }
  func qualificationReportDigest() throws -> Data { qualification.profile.qualificationReportDigest }
  func executeAndVerify(operation: UInt8, requestID: Data, canonicalCommand: Data,
    acceptedDevicePublicKey: Data?) throws -> KagemushaAuthenticatedDeviceResponseV1 {
    operations.append(operation)
    guard operation == 1 else { throw NSError(domain: "unexpected hardware mutation", code: 1) }
    let q = qualification
    let profile = try XCTUnwrap(noritoDecodeFrame(KagemushaNoritoV1.encodeHardwareProfileShape(q.profile)))
    let credential = try XCTUnwrap(noritoDecodeFrame(KagemushaNoritoV1.encodeHardwareCredentialShape(q.credential)))
    var payload = Data()
    for field in [Data([1, 0]), Data([1]), q.releaseID, q.hardwarePolicyDigest,
      q.coreAuthorizationKeyReference, profile.payload, credential.payload] {
      var size = field.count
      repeat { let byte = UInt8(size & 0x7f); size >>= 7; payload.append(size == 0 ? byte : byte | 0x80) } while size != 0
      payload.append(field)
    }
    let reply = noritoEncode(typeName: "iroha.kagemusha.device.v1.active-hardware-credential-reply",
      payload: payload, flags: NoritoHeader.compactLen, payloadAlignment: 8)
    var signature = Data(repeating: 0, count: 64); signature[31] = 1; signature[63] = 1
    return try KagemushaAuthenticatedDeviceResponseV1(operation: 1, status: .success,
      canonicalReply: reply, authenticator: signature)
  }
}
