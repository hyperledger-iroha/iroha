import Foundation
import XCTest

@testable import IrohaSwift

final class KagemushaDeviceLifecycleBridgeV1Tests: XCTestCase {
  func testUnsupportedDeviceRemainsOnlineOnly() throws {
    let bridge = KagemushaDeviceLifecycleBridgeV1.onlineOnly()
    XCTAssertEqual(bridge.availability, .onlineOnly)
    XCTAssertNil(bridge.acceptedCapabilities)
    XCTAssertThrowsError(
      try bridge.executeAuthenticated(
        operation: .prepareExactNextTransition,
        requestID: fixed(0x11, count: 32),
        canonicalCommand: Data([1]),
        acceptedDevicePublicKey: nil
      )
    ) { error in
      XCTAssertEqual(
        error as? KagemushaDeviceLifecycleBridgeErrorV1,
        .onlineOnly
      )
    }
  }

  func testExactCapabilitiesUnlockEveryJournalAndOutboxOperation() throws {
    let endpoint = FakeEndpoint()
    let bridge =
      try KagemushaDeviceLifecycleBridgeV1
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
      KagemushaDeviceLifecycleOperationV1.allCases.map(\.rawValue),
      (1...22).map(UInt8.init)
    )
    XCTAssertEqual(
      KagemushaDeviceLifecycleOperationV1.allCases.map { String(describing: $0) },
      [
        "readActiveHardwareCredential",
        "stageInboundPayment",
        "recoverStagedInboundPayment",
        "recoverInboundInboxPage",
        "prepareExactNextTransition",
        "recoverPreparedTransition",
        "commitVerifiedCandidateAndSignTerminal",
        "recoverTerminalOutcome",
        "installTerminalEnvelope",
        "recoverInstalledEnvelopeOrStateProof",
        "signReceiveAcknowledgement",
        "releaseOutboxEntry",
        "readTrustedTimeOrLease",
        "prepareMintAuthorization",
        "recoverMintAuthorization",
        "verifyAuthorizationAndStageMintCredit",
        "foldReceiveCredit",
        "readPendingCreditWatermark",
        "rotateHardwareEpoch",
        "bootstrapAggregateState",
        "recoverWalletSnapshot",
        "createSignedPaymentRequest",
      ]
    )
    XCTAssertEqual(
      KagemushaDeviceLifecycleCapabilityV1.allCases.map(\.rawValue),
      (0..<16).map { UInt32(1) << UInt32($0) }
    )
    XCTAssertEqual(
      KagemushaDeviceLifecycleCapabilityV1.allCases.map { String(describing: $0) },
      [
        "exactNextPredecessorConsumption",
        "oneUseSuccessorAuthorization",
        "rollbackResistantCounterAndJournal",
        "sealedTransitionRecovery",
        "receiverBoundCreditCommit",
        "rollbackResistantAcceptedCreditInbox",
        "authenticatedInboundStaging",
        "authoritativeReplayRootRecovery",
        "senderOutboxReservation",
        "authenticatedDurableRetryOutbox",
        "atomicVerifiedCandidateCommit",
        "recoverableTerminalCommitCertificate",
        "trustedTimeOrLease",
        "kagemushaHardwareEpochRotation",
        "rollbackSafeCounterRollover",
        "noSoftwareFallback",
      ]
    )
    XCTAssertEqual(
      KagemushaDeviceLifecycleStatusV1.allCases.map(\.rawValue),
      (0...10).map(UInt8.init)
    )
    XCTAssertEqual(
      KagemushaDeviceLifecycleStatusV1.allCases.map { String(describing: $0) },
      [
        "success", "unavailable", "staleOrConcurrent", "bindingMismatch",
        "trustedTimeRejected", "rejected", "missing", "conflict", "corrupt",
        "malformedRequest", "recoveryRequired",
      ]
    )

    for operation in KagemushaDeviceLifecycleOperationV1.allCases {
      endpoint.operation = operation
      let result = try bridge.executeAuthenticated(
        operation: operation,
        requestID: fixed(0x11, count: 32),
        canonicalCommand: Data([1, 2, 3]),
        acceptedDevicePublicKey: operation == .readActiveHardwareCredential
          ? nil : devicePublicKey()
      )
      XCTAssertEqual(result.status, .success)
      XCTAssertEqual(result.payload, Data([4, 5]))
      XCTAssertEqual(result.authenticator, fixed(0x44, count: 64))
    }
  }

  func testCommandFramingIsCanonicalAndOldVersionsFailClosed() throws {
    let command = try KagemushaDeviceLifecycleBridgeV1.Codec.encodeCommand(
      operation: .stageInboundPayment,
      requestID: fixed(0x11, count: 32),
      payload: Data([1, 2, 3])
    )
    XCTAssertEqual(
      command.hex,
      "494b474d4a434d3101000200"
        + String(repeating: "11", count: 32)
        + "03000000"
        + "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81"
        + "010203"
    )

    for retiredVersion: UInt8 in [4, 5] {
      var response = KagemushaDeviceLifecycleBridgeV1.Codec
        .encodeResponseForTests(
          operation: .stageInboundPayment,
          status: .success,
          requestID: fixed(0x11, count: 32),
          payload: Data([4]),
          authenticator: fixed(0x44, count: 64)
        )
      response[8] = retiredVersion
      XCTAssertThrowsError(
        try KagemushaDeviceLifecycleBridgeV1.Codec.decodeResponse(
          response,
          expectedOperation: .stageInboundPayment,
          expectedRequestID: fixed(0x11, count: 32)
        )
      )
    }

    for unknownOperation: UInt8 in [0, 28, 255] {
      var response = KagemushaDeviceLifecycleBridgeV1.Codec
        .encodeResponseForTests(
          operation: .stageInboundPayment,
          status: .success,
          requestID: fixed(0x11, count: 32),
          payload: Data([4]),
          authenticator: fixed(0x44, count: 64)
        )
      response[10] = unknownOperation
      XCTAssertThrowsError(
        try KagemushaDeviceLifecycleBridgeV1.Codec.decodeResponse(
          response,
          expectedOperation: .stageInboundPayment,
          expectedRequestID: fixed(0x11, count: 32)
        )
      )
    }

    var unknownStatus = KagemushaDeviceLifecycleBridgeV1.Codec
      .encodeResponseForTests(
        operation: .stageInboundPayment,
        status: .success,
        requestID: fixed(0x11, count: 32),
        payload: Data([4]),
        authenticator: fixed(0x44, count: 64)
      )
    unknownStatus[11] = 11
    XCTAssertThrowsError(
      try KagemushaDeviceLifecycleBridgeV1.Codec.decodeResponse(
        unknownStatus,
        expectedOperation: .stageInboundPayment,
        expectedRequestID: fixed(0x11, count: 32)
      )
    )

    let recoveryRequired = KagemushaDeviceLifecycleBridgeV1.Codec
      .encodeResponseForTests(
        operation: .recoverTerminalOutcome,
        status: .recoveryRequired,
        requestID: fixed(0x11, count: 32),
        payload: Data(),
        authenticator: Data()
      )
    XCTAssertEqual(
      try KagemushaDeviceLifecycleBridgeV1.Codec.decodeResponse(
        recoveryRequired,
        expectedOperation: .recoverTerminalOutcome,
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
        try KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(partial),
        "accepted missing feature bit \(featureBit)"
      )
    }

    let unknownFeature = FakeEndpoint()
    unknownFeature.capabilityFrame[14] = 1
    XCTAssertThrowsError(
      try KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(unknownFeature)
    )

    let endpoint = FakeEndpoint()
    endpoint.authenticator = Data(repeating: 0, count: 64)
    let bridge =
      try KagemushaDeviceLifecycleBridgeV1
      .withEndpointForTests(endpoint)
    XCTAssertThrowsError(
      try bridge.executeAuthenticated(
        operation: .recoverTerminalOutcome,
        requestID: fixed(0x11, count: 32),
        canonicalCommand: Data([1]),
        acceptedDevicePublicKey: devicePublicKey()
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
          "Sources/IrohaSwift/KagemushaDeviceLifecycleBridgeV1.swift"
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

  private func devicePublicKey() -> Data {
    Data([4]) + Data(repeating: 0x55, count: 64)
  }
}

private final class FakeEndpoint: KagemushaDeviceLifecycleEndpointV1 {
  var operation: KagemushaDeviceLifecycleOperationV1 = .recoverTerminalOutcome
  var authenticator = Data(repeating: 0x44, count: 64)
  var capabilityFrame = try! KagemushaDeviceLifecycleBridgeV1.Codec
    .encodeCapabilitiesForTests(
      platform: 2,
      policy: Data(repeating: 0x22, count: 32),
      attestation: Data(repeating: 0x33, count: 32)
    )

  func capabilities() throws -> Data { capabilityFrame }

  func execute(_ command: Data) throws -> Data {
    XCTAssertEqual(Data(command.prefix(8)), Data("IKGMJCM1".utf8))
    let requestID = Data(command[12..<44])
    return KagemushaDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
      operation: operation,
      status: .success,
      requestID: requestID,
      payload: Data([4, 5]),
      authenticator: authenticator
    )
  }

  func verifyResponseAuthenticator(
    response _: Data,
    operation _: KagemushaDeviceLifecycleOperationV1,
    requestID _: Data,
    hardwarePolicyID _: Data,
    qualificationReportDigest _: Data,
    acceptedDevicePublicKey _: Data?
  ) -> Bool {
    authenticator.count == 64 && authenticator.contains(where: { $0 != 0 })
  }
}

extension Data {
  fileprivate var hex: String {
    map { String(format: "%02x", $0) }.joined()
  }
}
