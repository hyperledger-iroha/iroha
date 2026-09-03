// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import CryptoKit
import Foundation

#if OFFLINE_SECURE_ELEMENT_CREDENTIAL && canImport(SecureElementCredential)
  import SecureElementCredential
#endif

/// Exact Apple-provisioned credential and applet identity used for wired lifecycle APDUs.
public struct KagemushaSecureElementCredentialConfigurationV1: Equatable, Sendable {
  public let credentialIdentifier: UUID
  public let instanceAID: Data

  public init(credentialIdentifier: UUID, instanceAID: Data) {
    precondition((5...16).contains(instanceAID.count), "instanceAID must contain 5...16 bytes")
    precondition(instanceAID.contains { $0 != 0 }, "instanceAID must be non-zero")
    self.credentialIdentifier = credentialIdentifier
    self.instanceAID = instanceAID
  }

  public static func foundation(credentialIdentifier: UUID)
    -> KagemushaSecureElementCredentialConfigurationV1
  {
    KagemushaSecureElementCredentialConfigurationV1(
      credentialIdentifier: credentialIdentifier,
      instanceAID: Data([0xf0, 0x4f, 0x44, 0x4a, 0x52, 0x4e, 0x00, 0x01])
    )
  }
}

#if OFFLINE_SECURE_ELEMENT_CREDENTIAL && canImport(SecureElementCredential)
  /// Async wired-mode session admitted only by the exact ABI-23 applet capability frame.
  ///
  /// Targets must define `OFFLINE_SECURE_ELEMENT_CREDENTIAL` only after Apple grants the Secure
  /// Element Credential entitlement and provisions the named applet. The compile gate prevents a
  /// non-entitled build from calling `CredentialSession.startSession()`.
  @available(iOS 18.1, *)
  public final class KagemushaSecureElementCredentialSessionV1: @unchecked Sendable {
    public let acceptedCapabilities: KagemushaDeviceLifecycleCapabilitiesV1

    private let session: CredentialSession
    private let gate = AsyncGate()

    private init(
      session: CredentialSession,
      acceptedCapabilities: KagemushaDeviceLifecycleCapabilitiesV1
    ) {
      self.session = session
      self.acceptedCapabilities = acceptedCapabilities
    }

    /// Open the exact installed credential without blocking the caller's executor.
    public static func openIfAvailable(
      configuration: KagemushaSecureElementCredentialConfigurationV1
    ) async -> KagemushaSecureElementCredentialSessionV1? {
      do {
        guard try await CredentialSession.isEligible else { return nil }
        let session = try await CredentialSession.startSession()
        do {
          let matching = try await session.listCredentials().filter {
            $0.identifier == configuration.credentialIdentifier
          }
          guard matching.count == 1,
            case .installed(let instances) = matching[0].state,
            instances.count == 1,
            instances[0].instanceAID == configuration.instanceAID
          else {
            try? await session.invalidate()
            return nil
          }
          try await session.enterWiredMode(using: matching[0])
          let frame = try await APDU.capabilities(session: session)
          let capabilities = try KagemushaDeviceLifecycleBridgeV1.Codec.decodeCapabilities(
            frame,
            expectedPlatform: KagemushaDeviceLifecycleBridgeV1.Codec.iosPlatformCode
          )
          return KagemushaSecureElementCredentialSessionV1(
            session: session,
            acceptedCapabilities: capabilities
          )
        } catch {
          try? await session.endWiredMode()
          try? await session.invalidate()
          return nil
        }
      } catch {
        return nil
      }
    }

    /// Execute one canonical lifecycle command entirely through the admitted wired-mode applet.
    public func execute(
      operation: KagemushaDeviceLifecycleOperationV1,
      requestID: Data,
      canonicalCommand: Data
    ) async throws -> KagemushaDeviceLifecycleResultV1 {
      try await gate.withLock {
        var command = try KagemushaDeviceLifecycleBridgeV1.Codec.encodeCommand(
          operation: operation,
          requestID: requestID,
          payload: canonicalCommand
        )
        let commandRange = command.startIndex..<command.endIndex
        defer { command.resetBytes(in: commandRange) }
        var response = try await APDU.execute(command, session: session)
        let responseRange = response.startIndex..<response.endIndex
        defer { response.resetBytes(in: responseRange) }
        return try KagemushaDeviceLifecycleBridgeV1.Codec.decodeResponse(
          response,
          expectedOperation: operation,
          expectedRequestID: requestID
        )
      }
    }

    public func close() async {
      await gate.withLockWithoutThrowing {
        try? await self.session.endWiredMode()
        try? await self.session.invalidate()
      }
    }

    private actor AsyncGate {
      private var occupied = false
      private var waiters: [CheckedContinuation<Void, Never>] = []

      func withLock<T>(_ operation: () async throws -> T) async rethrows -> T {
        await acquire()
        defer { release() }
        return try await operation()
      }

      func withLockWithoutThrowing(_ operation: () async -> Void) async {
        await acquire()
        defer { release() }
        await operation()
      }

      private func acquire() async {
        if !occupied {
          occupied = true
          return
        }
        await withCheckedContinuation { continuation in
          waiters.append(continuation)
        }
      }

      private func release() {
        if waiters.isEmpty {
          occupied = false
        } else {
          waiters.removeFirst().resume()
        }
      }
    }

    private enum APDU {
      static func capabilities(session: CredentialSession) async throws -> Data {
        let response = try await exchange(
          shortCommand(instruction: insCapabilities, expectedLength: capabilityBytes),
          label: "capabilities",
          session: session
        )
        guard response.count == capabilityBytes else {
          throw invalid("secure-element capability response must contain exactly 96 bytes")
        }
        return response
      }

      static func execute(_ command: Data, session: CredentialSession) async throws -> Data {
        guard (minimumCommandBytes...maximumCommandBytes).contains(command.count) else {
          throw invalid("secure-element command is outside the ABI-23 bound")
        }
        do {
          var begin = uint32LE(UInt32(command.count))
          begin.append(sha256(command))
          try requireEmpty(
            await exchange(
              shortCommand(instruction: insBeginCommand, data: begin),
              label: "begin",
              session: session
            ),
            label: "begin"
          )
          var offset = 0
          var index = 0
          while offset < command.count {
            let count = min(chunkBytes, command.count - offset)
            var chunk = command.subdata(in: offset..<(offset + count))
            let range = chunk.startIndex..<chunk.endIndex
            defer { chunk.resetBytes(in: range) }
            try requireEmpty(
              await exchange(
                shortCommand(
                  instruction: insWriteCommand,
                  p1: index >> 8,
                  p2: index,
                  data: chunk
                ),
                label: "write chunk \(index)",
                session: session
              ),
              label: "write chunk \(index)"
            )
            offset += count
            index += 1
          }
          var metadata = try await exchange(
            shortCommand(instruction: insCommitCommand, expectedLength: metadataBytes),
            label: "commit",
            session: session
          )
          let metadataRange = metadata.startIndex..<metadata.endIndex
          defer { metadata.resetBytes(in: metadataRange) }
          guard metadata.count == metadataBytes else {
            throw invalid("secure-element response metadata must contain exactly 36 bytes")
          }
          let responseLength = Int(try readUInt32LE(metadata))
          guard (minimumResponseBytes...maximumResponseBytes).contains(responseLength) else {
            throw invalid("secure-element response is outside the ABI-23 bound")
          }
          let expectedDigest = Data(metadata[4..<36])
          var response = Data(capacity: responseLength)
          index = 0
          while response.count < responseLength {
            let count = min(chunkBytes, responseLength - response.count)
            var chunk = try await exchange(
              shortCommand(
                instruction: insReadResponse,
                p1: index >> 8,
                p2: index,
                expectedLength: count
              ),
              label: "read chunk \(index)",
              session: session
            )
            let range = chunk.startIndex..<chunk.endIndex
            defer { chunk.resetBytes(in: range) }
            guard chunk.count == count else {
              throw invalid("secure-element response chunk \(index) has the wrong length")
            }
            response.append(chunk)
            index += 1
          }
          guard constantTimeEqual(expectedDigest, sha256(response)) else {
            let range = response.startIndex..<response.endIndex
            response.resetBytes(in: range)
            throw invalid("secure-element response digest mismatch")
          }
          return response
        } catch {
          await abortBestEffort(session: session)
          throw error
        }
      }

      private static func exchange(
        _ original: Data,
        label: String,
        session: CredentialSession
      ) async throws -> Data {
        var command = original
        let commandRange = command.startIndex..<command.endIndex
        defer { command.resetBytes(in: commandRange) }
        var raw = try await session.transceive(command)
        let rawRange = raw.startIndex..<raw.endIndex
        defer { raw.resetBytes(in: rawRange) }
        guard raw.count >= 2 else {
          throw invalid("secure-element \(label) response omitted its status word")
        }
        let statusOffset = raw.count - 2
        let status = UInt16(raw[statusOffset]) << 8 | UInt16(raw[statusOffset + 1])
        guard status == 0x9000 else {
          throw KagemushaDeviceLifecycleBridgeErrorV1.executionFailed
        }
        return Data(raw.prefix(statusOffset))
      }

      private static func abortBestEffort(session: CredentialSession) async {
        var command = shortCommand(instruction: insAbortTransport)
        let range = command.startIndex..<command.endIndex
        defer { command.resetBytes(in: range) }
        if var response = try? await session.transceive(command) {
          let responseRange = response.startIndex..<response.endIndex
          response.resetBytes(in: responseRange)
        }
      }

      private static func requireEmpty(_ response: Data, label: String) throws {
        guard response.isEmpty else {
          throw invalid("secure-element \(label) returned unexpected bytes")
        }
      }

      private static func shortCommand(
        instruction: UInt8,
        p1: Int = 0,
        p2: Int = 0,
        data: Data = Data(),
        expectedLength: Int? = nil
      ) -> Data {
        precondition(data.count <= 255)
        precondition(expectedLength == nil || (1...256).contains(expectedLength!))
        precondition(data.isEmpty || expectedLength == nil)
        var output = Data([
          cla, instruction, UInt8(truncatingIfNeeded: p1), UInt8(truncatingIfNeeded: p2),
        ])
        if !data.isEmpty {
          output.append(UInt8(data.count))
          output.append(data)
        } else if let expectedLength {
          output.append(UInt8(truncatingIfNeeded: expectedLength))
        }
        return output
      }

      private static func readUInt32LE(_ data: Data) throws -> UInt32 {
        guard data.count >= 4 else { throw invalid("truncated response length") }
        return (0..<4).reduce(UInt32(0)) { value, index in
          value | UInt32(data[index]) << UInt32(index * 8)
        }
      }

      private static func uint32LE(_ value: UInt32) -> Data {
        Data(
          (0..<4).map { index in
            UInt8(truncatingIfNeeded: value >> UInt32(index * 8))
          })
      }

      private static func sha256(_ data: Data) -> Data { Data(SHA256.hash(data: data)) }

      private static func constantTimeEqual(_ left: Data, _ right: Data) -> Bool {
        guard left.count == right.count else { return false }
        var difference: UInt8 = 0
        for (a, b) in zip(left, right) { difference |= a ^ b }
        return difference == 0
      }

      private static func invalid(_ reason: String) -> KagemushaDeviceLifecycleBridgeErrorV1 {
        .invalidContract(reason)
      }

      private static let cla: UInt8 = 0x80
      private static let insCapabilities: UInt8 = 0x11
      private static let insBeginCommand: UInt8 = 0x12
      private static let insWriteCommand: UInt8 = 0x13
      private static let insCommitCommand: UInt8 = 0x14
      private static let insReadResponse: UInt8 = 0x15
      private static let insAbortTransport: UInt8 = 0x16
      private static let chunkBytes = 224
      private static let metadataBytes = 36
      private static let capabilityBytes = 96
      private static let minimumCommandBytes = 80
      private static let maximumCommandBytes = 80 + 64 * 1024
      private static let minimumResponseBytes = 116
      private static let maximumResponseBytes = 116 + 64 * 1024 + 8 * 1024
    }
  }
#else
  /// Compile-gated placeholder for builds without Apple's restricted SE credential entitlement.
  @available(iOS 18.1, *)
  public final class KagemushaSecureElementCredentialSessionV1: @unchecked Sendable {
    public let acceptedCapabilities: KagemushaDeviceLifecycleCapabilitiesV1? = nil

    public static func openIfAvailable(
      configuration _: KagemushaSecureElementCredentialConfigurationV1
    ) async -> KagemushaSecureElementCredentialSessionV1? {
      nil
    }

    public func execute(
      operation _: KagemushaDeviceLifecycleOperationV1,
      requestID _: Data,
      canonicalCommand _: Data
    ) async throws -> KagemushaDeviceLifecycleResultV1 {
      throw KagemushaDeviceLifecycleBridgeErrorV1.onlineOnly
    }

    public func close() async {}
  }
#endif
