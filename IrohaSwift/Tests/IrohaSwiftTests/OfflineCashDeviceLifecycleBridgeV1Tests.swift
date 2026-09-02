import Foundation
import XCTest

@testable import IrohaSwift

final class OfflineCashDeviceLifecycleBridgeV1Tests: XCTestCase {
  func testUnsupportedDeviceRemainsOnlineOnly() throws {
    let bridge = OfflineCashDeviceLifecycleBridgeV1.onlineOnly()
    XCTAssertEqual(bridge.availability, .onlineOnly)
    XCTAssertNil(bridge.acceptedCapabilities)
    XCTAssertThrowsError(
      try bridge.execute(
        operation: .prepareExactNextTransition,
        requestID: fixed(0x11, count: 32),
        canonicalCommand: Data([1])
      )
    ) { error in
      XCTAssertEqual(
        error as? OfflineCashDeviceLifecycleBridgeErrorV1,
        .onlineOnly
      )
    }
  }

  func testExactCapabilitiesUnlockEveryJournalAndOutboxOperation() throws {
    let endpoint = FakeEndpoint()
    let bridge =
      try OfflineCashDeviceLifecycleBridgeV1
      .withEndpointForTests(endpoint)
    XCTAssertEqual(bridge.availability, .available)
    XCTAssertEqual(
      bridge.acceptedCapabilities?.hardwarePolicyID,
      fixed(0x22, count: 32)
    )
    XCTAssertEqual(
      endpoint.capabilityFrame[12..<16],
      Data([0xff, 0xff, 0x00, 0x00])
    )
    XCTAssertEqual(
      OfflineCashDeviceLifecycleOperationV1.allCases.map(\.rawValue),
      (1...24).map(UInt8.init)
    )
    XCTAssertEqual(
      OfflineCashDeviceLifecycleOperationV1.allCases.map { String(describing: $0) },
      [
        "readActiveHardwareCredential",
        "prepareAcceptanceIntentAuthorization",
        "recoverAcceptanceIntentAuthorization",
        "verifyAuthorizationReserveInboxAndIssueAcceptanceTicket",
        "recoverAcceptanceTicket",
        "stageInboundPayment",
        "recoverStagedInboundPayment",
        "recoverInboundInboxPage",
        "prepareExactNextTransition",
        "recoverPreparedTransition",
        "abandonUncommittedPreparedTransition",
        "commitVerifiedCandidate",
        "recoverTerminalCommitCertificate",
        "installFinalCommitWrapper",
        "recoverInstalledEnvelopeOrStateProof",
        "signReceiveAcknowledgement",
        "releaseOutboxEntry",
        "readTrustedTimeOrLease",
        "prepareMintAuthorization",
        "recoverMintAuthorization",
        "verifyAuthorizationAndStageMintCredit",
        "foldReceive",
        "readPendingCreditWatermark",
        "rotateHardwareEpoch",
      ]
    )
    XCTAssertEqual(
      OfflineCashDeviceLifecycleCapabilityV1.allCases.map(\.rawValue),
      (0..<16).map { UInt32(1) << UInt32($0) }
    )
    XCTAssertEqual(
      OfflineCashDeviceLifecycleCapabilityV1.allCases.map { String(describing: $0) },
      [
        "exactNextPredecessorConsumption",
        "oneUseSuccessorAuthorization",
        "rollbackResistantCounterAndJournal",
        "sealedTransitionRecovery",
        "oneUseAcceptanceTickets",
        "durableInboxReservation",
        "authenticatedInboundStaging",
        "authoritativeReplayRootRecovery",
        "senderOutboxReservation",
        "authenticatedDurableRetryOutbox",
        "atomicVerifiedCandidateCommit",
        "recoverableTerminalCommitCertificate",
        "trustedTimeOrLease",
        "offlineHardwareEpochRotation",
        "rollbackSafeCounterRollover",
        "noSoftwareFallback",
      ]
    )
    XCTAssertEqual(
      OfflineCashDeviceLifecycleStatusV1.allCases.map(\.rawValue),
      (0...10).map(UInt8.init)
    )
    XCTAssertEqual(
      OfflineCashDeviceLifecycleStatusV1.allCases.map { String(describing: $0) },
      [
        "success", "unavailable", "staleOrConcurrent", "bindingMismatch",
        "trustedTimeRejected", "rejected", "missing", "conflict", "corrupt",
        "malformedRequest", "recoveryRequired",
      ]
    )

    for operation in OfflineCashDeviceLifecycleOperationV1.allCases {
      endpoint.operation = operation
      let result = try bridge.execute(
        operation: operation,
        requestID: fixed(0x11, count: 32),
        canonicalCommand: Data([1, 2, 3])
      )
      XCTAssertEqual(result.status, .success)
      XCTAssertEqual(result.payload, Data([4, 5]))
      XCTAssertEqual(result.authenticator, fixed(0x44, count: 64))
    }
  }

  func testCommandFramingIsCanonicalAndOldVersionsFailClosed() throws {
    let command = try OfflineCashDeviceLifecycleBridgeV1.Codec.encodeCommand(
      operation: .stageInboundPayment,
      requestID: fixed(0x11, count: 32),
      payload: Data([1, 2, 3])
    )
    XCTAssertEqual(
      command.hex,
      "494f43464a434d3101000600"
        + String(repeating: "11", count: 32)
        + "03000000"
        + "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81"
        + "010203"
    )

    for retiredVersion: UInt8 in [4, 5] {
      var response = OfflineCashDeviceLifecycleBridgeV1.Codec
        .encodeResponseForTests(
          operation: .stageInboundPayment,
          status: .success,
          requestID: fixed(0x11, count: 32),
          payload: Data([4]),
          authenticator: fixed(0x44, count: 64)
        )
      response[8] = retiredVersion
      XCTAssertThrowsError(
        try OfflineCashDeviceLifecycleBridgeV1.Codec.decodeResponse(
          response,
          expectedOperation: .stageInboundPayment,
          expectedRequestID: fixed(0x11, count: 32)
        )
      )
    }

    for unknownOperation: UInt8 in [0, 25] {
      var response = OfflineCashDeviceLifecycleBridgeV1.Codec
        .encodeResponseForTests(
          operation: .stageInboundPayment,
          status: .success,
          requestID: fixed(0x11, count: 32),
          payload: Data([4]),
          authenticator: fixed(0x44, count: 64)
        )
      response[10] = unknownOperation
      XCTAssertThrowsError(
        try OfflineCashDeviceLifecycleBridgeV1.Codec.decodeResponse(
          response,
          expectedOperation: .stageInboundPayment,
          expectedRequestID: fixed(0x11, count: 32)
        )
      )
    }

    var unknownStatus = OfflineCashDeviceLifecycleBridgeV1.Codec
      .encodeResponseForTests(
        operation: .stageInboundPayment,
        status: .success,
        requestID: fixed(0x11, count: 32),
        payload: Data([4]),
        authenticator: fixed(0x44, count: 64)
      )
    unknownStatus[11] = 11
    XCTAssertThrowsError(
      try OfflineCashDeviceLifecycleBridgeV1.Codec.decodeResponse(
        unknownStatus,
        expectedOperation: .stageInboundPayment,
        expectedRequestID: fixed(0x11, count: 32)
      )
    )

    let recoveryRequired = OfflineCashDeviceLifecycleBridgeV1.Codec
      .encodeResponseForTests(
        operation: .recoverTerminalCommitCertificate,
        status: .recoveryRequired,
        requestID: fixed(0x11, count: 32),
        payload: Data(),
        authenticator: Data()
      )
    XCTAssertEqual(
      try OfflineCashDeviceLifecycleBridgeV1.Codec.decodeResponse(
        recoveryRequired,
        expectedOperation: .recoverTerminalCommitCertificate,
        expectedRequestID: fixed(0x11, count: 32)
      ).status,
      .recoveryRequired
    )
  }

  func testPartialCapabilityAndUnauthenticatedSuccessFailClosed() throws {
    for featureBit in 0..<16 {
      let partial = FakeEndpoint()
      let byteIndex = 12 + featureBit / 8
      partial.capabilityFrame[byteIndex] &= ~UInt8(1 << (featureBit % 8))
      XCTAssertThrowsError(
        try OfflineCashDeviceLifecycleBridgeV1.withEndpointForTests(partial),
        "accepted missing feature bit \(featureBit)"
      )
    }

    let unknownFeature = FakeEndpoint()
    unknownFeature.capabilityFrame[14] = 1
    XCTAssertThrowsError(
      try OfflineCashDeviceLifecycleBridgeV1.withEndpointForTests(unknownFeature)
    )

    let endpoint = FakeEndpoint()
    endpoint.authenticator = Data(repeating: 0, count: 64)
    let bridge =
      try OfflineCashDeviceLifecycleBridgeV1
      .withEndpointForTests(endpoint)
    XCTAssertThrowsError(
      try bridge.execute(
        operation: .recoverTerminalCommitCertificate,
        requestID: fixed(0x11, count: 32),
        canonicalCommand: Data([1])
      ))
  }

  func testNativeOutputOwnerWipesItsFullAllocationOnEveryExit() throws {
    let packageRoot = URL(fileURLWithPath: #filePath)
      .deletingLastPathComponent()
      .deletingLastPathComponent()
      .deletingLastPathComponent()
    let source = try String(
      contentsOf:
        packageRoot
        .appendingPathComponent(
          "Sources/IrohaSwift/OfflineCashDeviceLifecycleBridgeV1.swift"
        ),
      encoding: .utf8
    )
    let nativeExecute = try XCTUnwrap(
      source.components(separatedBy: "      func execute(_ command: Data) throws -> Data {").last?
        .components(separatedBy: "    #else").first
    )
    XCTAssertTrue(
      nativeExecute.contains("let outputRange = output.startIndex..<output.endIndex")
    )
    XCTAssertTrue(nativeExecute.contains("defer { output.resetBytes(in: outputRange) }"))
    XCTAssertLessThan(
      try XCTUnwrap(
        nativeExecute.range(of: "defer { output.resetBytes(in: outputRange) }")?.lowerBound),
      try XCTUnwrap(nativeExecute.range(of: "executeFunction(")?.lowerBound)
    )
  }

  private func fixed(_ value: UInt8, count: Int) -> Data {
    Data(repeating: value, count: count)
  }
}

private final class FakeEndpoint: OfflineCashDeviceLifecycleEndpointV1 {
  var operation: OfflineCashDeviceLifecycleOperationV1 = .recoverTerminalCommitCertificate
  var authenticator = Data(repeating: 0x44, count: 64)
  var capabilityFrame = try! OfflineCashDeviceLifecycleBridgeV1.Codec
    .encodeCapabilitiesForTests(
      platform: 2,
      policy: Data(repeating: 0x22, count: 32),
      attestation: Data(repeating: 0x33, count: 32)
    )

  func capabilities() throws -> Data { capabilityFrame }

  func execute(_ command: Data) throws -> Data {
    XCTAssertEqual(Data(command.prefix(8)), Data("IOCFJCM1".utf8))
    let requestID = Data(command[12..<44])
    return OfflineCashDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
      operation: operation,
      status: .success,
      requestID: requestID,
      payload: Data([4, 5]),
      authenticator: authenticator
    )
  }
}

extension Data {
  fileprivate var hex: String {
    map { String(format: "%02x", $0) }.joined()
  }
}
