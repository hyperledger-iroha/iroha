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

  func reserveOperationID(operation: UInt8, publicBinding _: Data) throws -> Data {
    reservedOperations.append(operation)
    return Data(repeating: operation, count: 32)
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
