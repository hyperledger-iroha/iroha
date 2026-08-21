import CryptoKit
import Foundation

#if canImport(Darwin)
  import Darwin
#endif

/// Failures from the exact Offline Cash V1 secure-device bridge contract.
public enum OfflineCashDeviceLifecycleBridgeErrorV1: Error, Equatable {
  case onlineOnly
  case invalidContract(String)
  case executionFailed
}

/// Exact operations in Core's sealed Offline Cash V1 journal and payment outbox.
public enum OfflineCashDeviceLifecycleOperationV1: UInt8, CaseIterable, Sendable {
  case reserveReceiveIntentAndSign = 1
  case recoverReceiveIntentAndSignature = 2
  case bindReceiveRequestDigest = 3
  case publishSendPayment = 4
  case recoverActiveIntent = 5
  case cancelExpiredReceive = 6
  case commitIntentExactNext = 7
  case recoverTerminal = 8
  case recoverReceiveTerminal = 9
  case signReceiveAcknowledgement = 10
  case stagePayment = 11
  case recoverStagedPaymentDigest = 12
  case publishStagedPayment = 13
  case recoverPublishedPayment = 14
}

/// Stable native result status. Only `success` may carry authoritative bytes.
public enum OfflineCashDeviceLifecycleStatusV1: UInt8, Sendable {
  case success = 0
  case unavailable = 1
  case staleOrConcurrent = 2
  case intentMismatch = 3
  case trustedTimeRejected = 4
  case rejected = 5
  case missing = 6
  case conflict = 7
  case corrupt = 8
  case malformedRequest = 9
}

/// Accepted identity of the complete rollback-resistant secure backend.
public struct OfflineCashDeviceLifecycleCapabilitiesV1: Equatable, Sendable {
  public let hardwarePolicyID: Data
  public let attestationDigest: Data
}

/// Bounded response from the complete secure backend.
public struct OfflineCashDeviceLifecycleResultV1: Equatable, Sendable {
  public let operation: OfflineCashDeviceLifecycleOperationV1
  public let status: OfflineCashDeviceLifecycleStatusV1
  public let payload: Data
  public let authenticator: Data
}

protocol OfflineCashDeviceLifecycleEndpointV1 {
  func capabilities() throws -> Data
  func execute(_ command: Data) throws -> Data
}

///
/// Fail-closed iOS entry point for Offline Cash V1 device state.
///
/// App Attest assertions authenticate online challenges, but do not expose a local atomic journal,
/// trusted clock, exact-next monetary counter, or authenticated payment outbox. This bridge is
/// therefore available only when the loaded native image provides the complete optional secure
/// backend contract. Missing symbols, partial capability frames, malformed replies, and execution
/// failures keep the wallet online-only; no Keychain, App Attest-only, or software fallback exists.
///
public final class OfflineCashDeviceLifecycleBridgeV1 {
  public enum Availability: Sendable {
    case onlineOnly
    case available
  }

  public static let protocolVersion: UInt16 = 1
  public static let maximumCommandPayloadBytes = 64 * 1024
  public static let maximumResponsePayloadBytes = 64 * 1024
  public static let maximumAuthenticatorBytes = 8 * 1024

  public let availability: Availability
  public let acceptedCapabilities: OfflineCashDeviceLifecycleCapabilitiesV1?

  private let endpoint: OfflineCashDeviceLifecycleEndpointV1?

  private init(
    endpoint: OfflineCashDeviceLifecycleEndpointV1?,
    capabilities: OfflineCashDeviceLifecycleCapabilitiesV1?
  ) {
    self.endpoint = endpoint
    acceptedCapabilities = capabilities
    availability = endpoint != nil && capabilities != nil ? .available : .onlineOnly
  }

  /// Discover the optional native secure backend without permitting a software downgrade.
  public static func production() -> OfflineCashDeviceLifecycleBridgeV1 {
    guard let endpoint = NativeEndpoint.create(),
      let encoded = try? endpoint.capabilities(),
      let capabilities = try? Codec.decodeCapabilities(
        encoded,
        expectedPlatform: Codec.iosPlatformCode
      )
    else {
      return onlineOnly()
    }
    return OfflineCashDeviceLifecycleBridgeV1(
      endpoint: endpoint,
      capabilities: capabilities
    )
  }

  /// Construct an explicit online-only bridge for a device without the complete hardware API.
  public static func onlineOnly() -> OfflineCashDeviceLifecycleBridgeV1 {
    OfflineCashDeviceLifecycleBridgeV1(endpoint: nil, capabilities: nil)
  }

  /// Execute one exact canonical Core V1 command through the qualifying native backend.
  ///
  /// The native implementation must decode only the canonical Offline Cash V1 command for the
  /// selected operation. Relabelled Kagemusha V4/V5 archives are invalid inputs.
  public func execute(
    operation: OfflineCashDeviceLifecycleOperationV1,
    requestID: Data,
    canonicalCommand: Data
  ) throws -> OfflineCashDeviceLifecycleResultV1 {
    guard let endpoint else {
      throw OfflineCashDeviceLifecycleBridgeErrorV1.onlineOnly
    }
    var command = try Codec.encodeCommand(
      operation: operation,
      requestID: requestID,
      payload: canonicalCommand
    )
    let commandRange = command.startIndex..<command.endIndex
    defer { command.resetBytes(in: commandRange) }
    var response: Data
    do {
      response = try endpoint.execute(command)
    } catch {
      throw OfflineCashDeviceLifecycleBridgeErrorV1.executionFailed
    }
    let responseRange = response.startIndex..<response.endIndex
    defer { response.resetBytes(in: responseRange) }
    return try Codec.decodeResponse(
      response,
      expectedOperation: operation,
      expectedRequestID: requestID
    )
  }

  static func withEndpointForTests(
    _ endpoint: OfflineCashDeviceLifecycleEndpointV1
  ) throws -> OfflineCashDeviceLifecycleBridgeV1 {
    let capabilities = try Codec.decodeCapabilities(
      endpoint.capabilities(),
      expectedPlatform: Codec.iosPlatformCode
    )
    return OfflineCashDeviceLifecycleBridgeV1(
      endpoint: endpoint,
      capabilities: capabilities
    )
  }

  private final class NativeEndpoint: OfflineCashDeviceLifecycleEndpointV1 {
    #if canImport(Darwin)
      private typealias CapabilitiesFn =
        @convention(c) (
          UnsafeMutablePointer<UInt8>?,
          Int
        ) -> Int32
      private typealias ExecuteFn =
        @convention(c) (
          UnsafePointer<UInt8>?,
          Int,
          UnsafeMutablePointer<UInt8>?,
          Int,
          UnsafeMutablePointer<Int>?
        ) -> Int32

      private let capabilitiesFunction: CapabilitiesFn
      private let executeFunction: ExecuteFn

      private init(
        capabilitiesFunction: @escaping CapabilitiesFn,
        executeFunction: @escaping ExecuteFn
      ) {
        self.capabilitiesFunction = capabilitiesFunction
        self.executeFunction = executeFunction
      }

      static func create() -> NativeEndpoint? {
        // TODO: ship these optional symbols only with an audited device service that provides
        // the complete journal/counter/outbox contract; App Attest by itself is insufficient.
        let (handle, _) = NoritoBridgeLoader.openHandle()
        guard let handle,
          let capabilitiesSymbol = dlsym(
            handle,
            "connect_norito_offline_cash_device_capabilities_v1"
          ),
          let executeSymbol = dlsym(
            handle,
            "connect_norito_offline_cash_device_execute_v1"
          )
        else {
          return nil
        }
        return NativeEndpoint(
          capabilitiesFunction: unsafeBitCast(
            capabilitiesSymbol,
            to: CapabilitiesFn.self
          ),
          executeFunction: unsafeBitCast(executeSymbol, to: ExecuteFn.self)
        )
      }

      func capabilities() throws -> Data {
        var output = Data(repeating: 0, count: Codec.capabilityBytes)
        let outputCapacity = output.count
        let status = output.withUnsafeMutableBytes { raw in
          capabilitiesFunction(
            raw.bindMemory(to: UInt8.self).baseAddress,
            outputCapacity
          )
        }
        guard status == 0 else {
          throw OfflineCashDeviceLifecycleBridgeErrorV1.executionFailed
        }
        return output
      }

      func execute(_ command: Data) throws -> Data {
        let maximum =
          Codec.responseHeaderBytes
          + OfflineCashDeviceLifecycleBridgeV1.maximumResponsePayloadBytes
          + OfflineCashDeviceLifecycleBridgeV1.maximumAuthenticatorBytes
        var output = Data(repeating: 0, count: maximum)
        let outputRange = output.startIndex..<output.endIndex
        defer { output.resetBytes(in: outputRange) }
        let commandCount = command.count
        let outputCapacity = output.count
        var written = 0
        let status = command.withUnsafeBytes { commandRaw in
          output.withUnsafeMutableBytes { outputRaw in
            executeFunction(
              commandRaw.bindMemory(to: UInt8.self).baseAddress,
              commandCount,
              outputRaw.bindMemory(to: UInt8.self).baseAddress,
              outputCapacity,
              &written
            )
          }
        }
        guard status == 0, written >= 0, written <= output.count else {
          throw OfflineCashDeviceLifecycleBridgeErrorV1.executionFailed
        }
        return Data(output.prefix(written))
      }
    #else
      static func create() -> NativeEndpoint? { nil }

      func capabilities() throws -> Data {
        throw OfflineCashDeviceLifecycleBridgeErrorV1.onlineOnly
      }

      func execute(_: Data) throws -> Data {
        throw OfflineCashDeviceLifecycleBridgeErrorV1.onlineOnly
      }
    #endif
  }

  enum Codec {
    static let iosPlatformCode: UInt8 = 2
    static let capabilityBytes = 96
    static let commandHeaderBytes = 80
    static let responseHeaderBytes = 116

    private static let featureOneIntentSlot: UInt32 = 1 << 0
    private static let featureExactNextCounter: UInt32 = 1 << 1
    private static let featureAuthenticatedDurableJournal: UInt32 = 1 << 2
    private static let featureAuthenticatedDurableOutbox: UInt32 = 1 << 3
    private static let featureTrustedTime: UInt32 = 1 << 4
    private static let featureAtomicReserveAndSign: UInt32 = 1 << 5
    private static let featureAtomicCommitAndTerminalReceipt: UInt32 = 1 << 6
    private static let featureTerminalRecovery: UInt32 = 1 << 7
    private static let featureNoSoftwareFallback: UInt32 = 1 << 8
    private static let requiredFeatures =
      featureOneIntentSlot
      | featureExactNextCounter
      | featureAuthenticatedDurableJournal
      | featureAuthenticatedDurableOutbox
      | featureTrustedTime
      | featureAtomicReserveAndSign
      | featureAtomicCommitAndTerminalReceipt
      | featureTerminalRecovery
      | featureNoSoftwareFallback
    private static let capabilityMagic = Data("IOCFJCP1".utf8)
    private static let commandMagic = Data("IOCFJCM1".utf8)
    private static let responseMagic = Data("IOCFJRS1".utf8)

    static func decodeCapabilities(
      _ encoded: Data,
      expectedPlatform: UInt8
    ) throws -> OfflineCashDeviceLifecycleCapabilitiesV1 {
      guard encoded.count == capabilityBytes else {
        throw invalid("invalid capability size")
      }
      var input = Reader(encoded)
      try input.requireMagic(capabilityMagic, label: "capabilities")
      guard try input.readUInt16() == protocolVersion else {
        throw invalid("unsupported bridge version")
      }
      guard try input.readUInt8() == expectedPlatform else {
        throw invalid("platform mismatch")
      }
      guard try input.readUInt8() == 0 else {
        throw invalid("non-canonical capability flags")
      }
      guard try input.readUInt32() == requiredFeatures,
        try input.readUInt32() == UInt32(maximumCommandPayloadBytes),
        try input.readUInt32() == UInt32(maximumResponsePayloadBytes)
      else {
        throw invalid("incomplete secure backend")
      }
      let policy = try input.read(count: 32)
      let attestation = try input.read(count: 32)
      guard try input.readUInt64() == 0,
        isDigest(policy),
        isDigest(attestation),
        policy != attestation,
        input.isAtEnd
      else {
        throw invalid("invalid capability bindings")
      }
      return OfflineCashDeviceLifecycleCapabilitiesV1(
        hardwarePolicyID: policy,
        attestationDigest: attestation
      )
    }

    static func encodeCommand(
      operation: OfflineCashDeviceLifecycleOperationV1,
      requestID: Data,
      payload: Data
    ) throws -> Data {
      guard isDigest(requestID) else {
        throw invalid("requestID must be 32 non-zero bytes")
      }
      guard !payload.isEmpty, payload.count <= maximumCommandPayloadBytes else {
        throw invalid("command payload exceeds its bound")
      }
      var output = Data(capacity: commandHeaderBytes + payload.count)
      output.append(commandMagic)
      output.appendLittleEndian(protocolVersion)
      output.append(operation.rawValue)
      output.append(0)
      output.append(requestID)
      output.appendLittleEndian(UInt32(payload.count))
      output.append(sha256(payload))
      output.append(payload)
      return output
    }

    static func decodeResponse(
      _ encoded: Data,
      expectedOperation: OfflineCashDeviceLifecycleOperationV1,
      expectedRequestID: Data
    ) throws -> OfflineCashDeviceLifecycleResultV1 {
      guard encoded.count >= responseHeaderBytes,
        encoded.count <= responseHeaderBytes
          + maximumResponsePayloadBytes
          + maximumAuthenticatorBytes
      else {
        throw invalid("response size is outside its bound")
      }
      var input = Reader(encoded)
      try input.requireMagic(responseMagic, label: "response")
      guard try input.readUInt16() == protocolVersion else {
        throw invalid("unsupported bridge version")
      }
      guard
        let operation = OfflineCashDeviceLifecycleOperationV1(
          rawValue: try input.readUInt8()
        ), operation == expectedOperation
      else {
        throw invalid("response operation mismatch")
      }
      guard
        let status = OfflineCashDeviceLifecycleStatusV1(
          rawValue: try input.readUInt8()
        )
      else {
        throw invalid("unknown response status")
      }
      guard try input.read(count: 32) == expectedRequestID else {
        throw invalid("response request mismatch")
      }
      let payloadLength = try input.readBoundedLength(
        maximumResponsePayloadBytes,
        label: "payload"
      )
      let authenticatorLength = try input.readBoundedLength(
        maximumAuthenticatorBytes,
        label: "authenticator"
      )
      let payloadDigest = try input.read(count: 32)
      let authenticatorDigest = try input.read(count: 32)
      guard input.remaining == payloadLength + authenticatorLength else {
        throw invalid("response length mismatch")
      }
      let payload = try input.read(count: payloadLength)
      let authenticator = try input.read(count: authenticatorLength)
      guard payloadDigest == sha256(payload),
        authenticatorDigest == sha256(authenticator)
      else {
        throw invalid("response digest mismatch")
      }
      if status == .success {
        guard !payload.isEmpty,
          !authenticator.isEmpty,
          authenticator.contains(where: { $0 != 0 })
        else {
          throw invalid("successful response is unauthenticated")
        }
      } else if !payload.isEmpty || !authenticator.isEmpty {
        throw invalid("failed response exposed bytes")
      }
      return OfflineCashDeviceLifecycleResultV1(
        operation: operation,
        status: status,
        payload: payload,
        authenticator: authenticator
      )
    }

    static func encodeCapabilitiesForTests(
      platform: UInt8,
      policy: Data,
      attestation: Data
    ) throws -> Data {
      guard isDigest(policy), isDigest(attestation), policy != attestation else {
        throw invalid("invalid test capabilities")
      }
      var output = Data(capacity: capabilityBytes)
      output.append(capabilityMagic)
      output.appendLittleEndian(protocolVersion)
      output.append(platform)
      output.append(0)
      output.appendLittleEndian(requiredFeatures)
      output.appendLittleEndian(UInt32(maximumCommandPayloadBytes))
      output.appendLittleEndian(UInt32(maximumResponsePayloadBytes))
      output.append(policy)
      output.append(attestation)
      output.appendLittleEndian(UInt64(0))
      return output
    }

    static func encodeResponseForTests(
      operation: OfflineCashDeviceLifecycleOperationV1,
      status: OfflineCashDeviceLifecycleStatusV1,
      requestID: Data,
      payload: Data,
      authenticator: Data
    ) -> Data {
      var output = Data(
        capacity: responseHeaderBytes + payload.count + authenticator.count
      )
      output.append(responseMagic)
      output.appendLittleEndian(protocolVersion)
      output.append(operation.rawValue)
      output.append(status.rawValue)
      output.append(requestID)
      output.appendLittleEndian(UInt32(payload.count))
      output.appendLittleEndian(UInt32(authenticator.count))
      output.append(sha256(payload))
      output.append(sha256(authenticator))
      output.append(payload)
      output.append(authenticator)
      return output
    }

    private static func isDigest(_ value: Data) -> Bool {
      value.count == 32 && value.contains(where: { $0 != 0 })
    }

    private static func sha256(_ value: Data) -> Data {
      Data(SHA256.hash(data: value))
    }

    private static func invalid(
      _ message: String
    ) -> OfflineCashDeviceLifecycleBridgeErrorV1 {
      .invalidContract(message)
    }

    private struct Reader {
      private let bytes: Data
      private(set) var offset = 0

      init(_ bytes: Data) {
        self.bytes = bytes
      }

      var remaining: Int { bytes.count - offset }
      var isAtEnd: Bool { offset == bytes.count }

      mutating func read(count: Int) throws -> Data {
        guard count >= 0, count <= remaining else {
          throw invalid("truncated frame")
        }
        defer { offset += count }
        return Data(bytes[offset..<(offset + count)])
      }

      mutating func requireMagic(_ magic: Data, label: String) throws {
        guard try read(count: magic.count) == magic else {
          throw invalid("invalid \(label) magic")
        }
      }

      mutating func readUInt8() throws -> UInt8 {
        guard let value = try read(count: 1).first else {
          throw invalid("truncated integer")
        }
        return value
      }

      mutating func readUInt16() throws -> UInt16 {
        let value = try read(count: 2)
        return UInt16(value[value.startIndex])
          | UInt16(value[value.startIndex + 1]) << 8
      }

      mutating func readUInt32() throws -> UInt32 {
        let value = try read(count: 4)
        var result: UInt32 = 0
        for (shift, byte) in value.enumerated() {
          result |= UInt32(byte) << UInt32(shift * 8)
        }
        return result
      }

      mutating func readUInt64() throws -> UInt64 {
        let value = try read(count: 8)
        var result: UInt64 = 0
        for (shift, byte) in value.enumerated() {
          result |= UInt64(byte) << UInt64(shift * 8)
        }
        return result
      }

      mutating func readBoundedLength(
        _ maximum: Int,
        label: String
      ) throws -> Int {
        let value = try readUInt32()
        guard value <= UInt32(maximum) else {
          throw invalid("\(label) exceeds its bound")
        }
        return Int(value)
      }
    }
  }
}

extension Data {
  fileprivate mutating func appendLittleEndian(_ value: UInt16) {
    append(UInt8(truncatingIfNeeded: value))
    append(UInt8(truncatingIfNeeded: value >> 8))
  }

  fileprivate mutating func appendLittleEndian(_ value: UInt32) {
    for shift in stride(from: 0, through: 24, by: 8) {
      append(UInt8(truncatingIfNeeded: value >> UInt32(shift)))
    }
  }

  fileprivate mutating func appendLittleEndian(_ value: UInt64) {
    for shift in stride(from: 0, through: 56, by: 8) {
      append(UInt8(truncatingIfNeeded: value >> UInt64(shift)))
    }
  }
}
