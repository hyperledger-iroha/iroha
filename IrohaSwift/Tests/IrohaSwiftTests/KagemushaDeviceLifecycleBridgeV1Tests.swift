import Foundation
import XCTest

@testable import IrohaSwift

final class KagemushaDeviceLifecycleBridgeV1Tests: XCTestCase {
  func testExactFrameSurvivesTransportBufferClearingAndCallerMutation() throws {
    let requestID = fixed(0x11, count: 32)
    let expected = KagemushaDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
      operation: .readActiveHardwareCredential,
      status: .success,
      requestID: requestID,
      payload: Data([4, 5]),
      authenticator: fixed(0x44, count: 64)
    )
    var transportBuffer = expected
    let decoded = try KagemushaDeviceLifecycleBridgeV1.decodeUnverifiedResponse(
      transportBuffer, expectedOperation: .readActiveHardwareCredential,
      expectedRequestID: requestID
    )
    transportBuffer.resetBytes(in: transportBuffer.startIndex..<transportBuffer.endIndex)
    var callerCopy = decoded.canonicalResponseFrame
    callerCopy.resetBytes(in: callerCopy.startIndex..<callerCopy.endIndex)
    XCTAssertEqual(decoded.canonicalResponseFrame, expected)
    XCTAssertEqual(decoded.payload, Data([4, 5]))

    // This injected endpoint verifies buffer plumbing only, not hardware qualification.
    let endpoint = FakeEndpoint()
    endpoint.operation = .readActiveHardwareCredential
    let bridge = try KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(endpoint)
    let result = try bridge.executeAuthenticated(
      operation: .readActiveHardwareCredential, requestID: requestID,
      canonicalCommand: Data([1]), acceptedDevicePublicKey: nil
    )
    XCTAssertEqual(result.canonicalResponseFrame, expected)
  }

  func testNativeContractVectorProbeIsBoundedWhenLinked() {
    XCTAssertEqual(KagemushaDeviceLifecycleBridgeV1.maximumNativeContractVectorBytes, 4 * 1024)
    if let vector = KagemushaDeviceLifecycleBridgeV1.nativeContractVector() {
      XCTAssertFalse(vector.isEmpty)
      XCTAssertLessThanOrEqual(
        vector.count,
        KagemushaDeviceLifecycleBridgeV1.maximumNativeContractVectorBytes
      )
    }
  }

  func testUnverifiedResponseDecoderRequiresNonzeroRequestBinding() throws {
    for requestID in [Data(), fixed(0, count: 32), fixed(0x11, count: 31)] {
      let frame = KagemushaDeviceLifecycleBridgeV1.Codec.encodeResponseForTests(
        operation: .readActiveHardwareCredential, status: .success,
        requestID: requestID, payload: Data([1]), authenticator: fixed(0x44, count: 64))
      XCTAssertThrowsError(try KagemushaDeviceLifecycleBridgeV1.decodeUnverifiedResponse(
        frame, expectedOperation: .readActiveHardwareCredential, expectedRequestID: requestID))
    }
  }

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

  func testVerifierReceivesExactCommandAndRejectsReplyForAnotherCommand() throws {
    let endpoint = FakeEndpoint()
    endpoint.operation = .readPendingCreditWatermark
    let expected = try KagemushaDeviceOperationCodecV1.encodeControlCommand(
      .readPendingCreditWatermark(watermark: nil, target: .drainAll))
    endpoint.authenticatedCommand = expected
    let bridge = try KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(endpoint)
    let id = fixed(0x11, count: 32)
    _ = try bridge.executeAuthenticated(operation: .readPendingCreditWatermark,
      requestID: id, canonicalCommand: expected, acceptedDevicePublicKey: devicePublicKey())
    XCTAssertEqual(endpoint.verifiedCommand, expected)
    XCTAssertEqual(endpoint.executedCommand, expected)

    let substituted = try KagemushaDeviceOperationCodecV1.encodeControlCommand(
      .readPendingCreditWatermark(watermark: nil, target: .requiredBalance(.init(1))))
    XCTAssertThrowsError(try bridge.executeAuthenticated(operation: .readPendingCreditWatermark,
      requestID: id, canonicalCommand: substituted, acceptedDevicePublicKey: devicePublicKey()))
    XCTAssertEqual(endpoint.verifiedCommand, substituted)
    XCTAssertEqual(endpoint.executedCommand, substituted)
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
  var authenticatedCommand: Data?
  var executedCommand: Data?
  var verifiedCommand: Data?
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
    executedCommand = Data(command.dropFirst(80))
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
    canonicalCommand: Data,
    operation _: KagemushaDeviceLifecycleOperationV1,
    requestID _: Data,
    hardwarePolicyID _: Data,
    qualificationReportDigest _: Data,
    acceptedDevicePublicKey _: Data?
  ) -> Bool {
    verifiedCommand = canonicalCommand
    return (authenticatedCommand == nil || authenticatedCommand == canonicalCommand)
      && authenticator.count == 64 && authenticator.contains(where: { $0 != 0 })
  }
}

extension Data {
  fileprivate var hex: String {
    map { String(format: "%02x", $0) }.joined()
  }
}
