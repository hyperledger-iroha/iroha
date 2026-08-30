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
        operation: .commitIntentExactNext,
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
      operation: .cancelExpiredReceive,
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
          operation: .cancelExpiredReceive,
          status: .success,
          requestID: fixed(0x11, count: 32),
          payload: Data([4]),
          authenticator: fixed(0x44, count: 64)
        )
      response[8] = retiredVersion
      XCTAssertThrowsError(
        try OfflineCashDeviceLifecycleBridgeV1.Codec.decodeResponse(
          response,
          expectedOperation: .cancelExpiredReceive,
          expectedRequestID: fixed(0x11, count: 32)
        )
      )
    }
  }

  func testPartialCapabilityAndUnauthenticatedSuccessFailClosed() throws {
    for featureBit in 0..<9 {
      let partial = FakeEndpoint()
      let byteIndex = 12 + featureBit / 8
      partial.capabilityFrame[byteIndex] &= ~UInt8(1 << (featureBit % 8))
      XCTAssertThrowsError(
        try OfflineCashDeviceLifecycleBridgeV1.withEndpointForTests(partial),
        "accepted missing feature bit \(featureBit)"
      )
    }

    let endpoint = FakeEndpoint()
    endpoint.authenticator = Data(repeating: 0, count: 64)
    let bridge =
      try OfflineCashDeviceLifecycleBridgeV1
      .withEndpointForTests(endpoint)
    XCTAssertThrowsError(
      try bridge.execute(
        operation: .recoverTerminal,
        requestID: fixed(0x11, count: 32),
        canonicalCommand: Data([1])
      ))
  }

  func testProductionCannotBeEnabledByOptionalSymbolsOrStructuralCapabilities() throws {
    let bridge = OfflineCashDeviceLifecycleBridgeV1.production()
    XCTAssertEqual(bridge.availability, .onlineOnly)
    XCTAssertNil(bridge.acceptedCapabilities)

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
    let production = try XCTUnwrap(
      source.components(
        separatedBy:
          "public static func production() -> OfflineCashDeviceLifecycleBridgeV1 {"
      ).last?
        .components(separatedBy: "  /// Construct an explicit online-only bridge").first
    )
    XCTAssertTrue(production.contains("onlineOnly()"))
    XCTAssertFalse(production.contains("capabilities"))
    XCTAssertFalse(source.contains("dlsym"))
    XCTAssertFalse(source.contains("NativeEndpoint"))
    XCTAssertFalse(source.contains("connect_norito_offline_cash_device_capabilities_v1"))
    XCTAssertFalse(source.contains("connect_norito_offline_cash_device_execute_v1"))
    XCTAssertTrue(source.contains("static func withEndpointForTests("))
  }

  private func fixed(_ value: UInt8, count: Int) -> Data {
    Data(repeating: value, count: count)
  }
}

private final class FakeEndpoint: OfflineCashDeviceLifecycleEndpointV1 {
  var operation: OfflineCashDeviceLifecycleOperationV1 = .recoverTerminal
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
