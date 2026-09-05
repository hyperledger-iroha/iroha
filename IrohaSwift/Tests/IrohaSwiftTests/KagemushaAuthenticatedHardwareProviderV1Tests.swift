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
      core: core
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
    XCTAssertEqual(core.reservedOperations, [1])
    XCTAssertFalse(core.acceptedQualification)
    XCTAssertEqual(transport.operations, [1])
    XCTAssertEqual(transport.acceptedKeys, [nil])
  }

  func testSenderReservationsMatchCanonicalNativeCoreBindings() throws {
    let fixture = try reservationFixture()
    let core = RecordingNativeCore()
    let transport = UnavailableAuthenticatedTransport()
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: core)
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
        operationID: operationID, amount: KagemushaUInt128V1(amount), beneficiary: beneficiary),
      operationID)
    XCTAssertEqual(core.reservations.last?.2, try fixtureBytes(fixture, "redeem_binding_hex"))
    XCTAssertEqual(core.reservedOperations, [5, 5])
    XCTAssertTrue(transport.operations.isEmpty)
  }

  func testRequestAndMintReservationsRetainCallerOwnedIdentity() throws {
    let fixture = try reservationFixture()
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
      fixtureBytes(fixture, "send_request_hex"))
    let core = RecordingNativeCore()
    let provider = KagemushaAuthenticatedHardwareProviderV1(
      transport: UnavailableAuthenticatedTransport(), core: core)
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
        operationID: operationID, amount: request.amount,
        payer: request.recipient, recipient: request.recipient), operationID)
    XCTAssertEqual(core.reservedOperations, [22, 22, 14])
    XCTAssertTrue(core.reservations.allSatisfy { $0.1 == operationID })
  }

  func testSubstitutedReservationsFailBeforeDeviceExecution() throws {
    let requestBytes = try fixtureBytes(reservationFixture(), "send_request_hex")
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(requestBytes)
    let core = RecordingNativeCore()
    core.substituteReservedID = true
    let transport = UnavailableAuthenticatedTransport()
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: core)
    let operationID = Data(repeating: 9, count: 32)
    XCTAssertThrowsError(try provider.qualification())
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
    XCTAssertTrue(transport.operations.isEmpty)
  }

  func testRequestExecutionReservesCallerIntentBeforeReachingHardware() throws {
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
      fixtureBytes(reservationFixture(), "send_request_hex"))
    let core = RecordingNativeCore()
    core.substituteReservedID = true
    let transport = UnavailableAuthenticatedTransport()
    let provider = KagemushaAuthenticatedHardwareProviderV1(transport: transport, core: core)
    let operationID = Data(repeating: 12, count: 32)
    XCTAssertThrowsError(
      try provider.createPaymentRequest(
        operationID: operationID, recipient: request.recipient,
        amount: request.amount, validityWindowMS: 1000))
    XCTAssertEqual(core.reservedOperations, [22])
    XCTAssertEqual(core.reservations.first?.1, operationID)
    XCTAssertTrue(transport.operations.isEmpty)
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
    qualification _: KagemushaHardwareQualificationV1
  ) throws { throw TestFailure.unused }

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
