// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import CryptoKit
import Foundation

/// One already selected, platform access-controlled secure-element applet channel.
protocol KagemushaSecureElementAPDUChannelV1: AnyObject {
  /// Return response data followed by the two-byte ISO 7816 status word.
  func transceive(_ command: Data) throws -> Data
  func close()
}

/// Short-APDU transport for exact ABI-23 lifecycle frames.
///
/// This type only transports bytes. `KagemushaDeviceLifecycleBridgeV1` independently admits the
/// applet only after it returns the exact complete `IKGMJCP1` capability frame.
final class KagemushaSecureElementAPDUEndpointV1: KagemushaDeviceLifecycleEndpointV1 {
  private let channel: any KagemushaSecureElementAPDUChannelV1
  private let lock = NSLock()

  init(channel: any KagemushaSecureElementAPDUChannelV1) {
    self.channel = channel
  }

  func capabilities() throws -> Data {
    try locked {
      let response = try exchange(
        Self.shortCommand(
          instruction: Self.insCapabilities,
          expectedLength: Self.capabilityBytes
        ),
        label: "capabilities"
      )
      guard response.count == Self.capabilityBytes else {
        throw Self.invalid(
          "secure-element capability response must contain exactly \(Self.capabilityBytes) bytes"
        )
      }
      return response
    }
  }

  func execute(_ command: Data) throws -> Data {
    try locked {
      guard (Self.minimumCommandBytes...Self.maximumCommandBytes).contains(command.count) else {
        throw Self.invalid("secure-element command is outside the ABI-23 bound")
      }
      let commandDigest = Self.sha256(command)
      do {
        var begin = Data(capacity: Self.responseMetadataBytes)
        begin.append(Self.uint32LE(UInt32(command.count)))
        begin.append(commandDigest)
        try requireEmpty(
          exchange(
            Self.shortCommand(instruction: Self.insBeginCommand, data: begin),
            label: "begin"
          ),
          label: "begin"
        )

        var offset = 0
        var chunkIndex = 0
        while offset < command.count {
          let count = min(Self.chunkBytes, command.count - offset)
          var chunk = command.subdata(in: offset..<(offset + count))
          let chunkRange = chunk.startIndex..<chunk.endIndex
          defer { chunk.resetBytes(in: chunkRange) }
          try requireEmpty(
            exchange(
              Self.shortCommand(
                instruction: Self.insWriteCommand,
                p1: chunkIndex >> 8,
                p2: chunkIndex,
                data: chunk
              ),
              label: "write chunk \(chunkIndex)"
            ),
            label: "write chunk \(chunkIndex)"
          )
          offset += count
          chunkIndex += 1
        }

        var metadata = try exchange(
          Self.shortCommand(
            instruction: Self.insCommitCommand,
            expectedLength: Self.responseMetadataBytes
          ),
          label: "commit"
        )
        let metadataRange = metadata.startIndex..<metadata.endIndex
        defer { metadata.resetBytes(in: metadataRange) }
        guard metadata.count == Self.responseMetadataBytes else {
          throw Self.invalid(
            "secure-element response metadata must contain exactly \(Self.responseMetadataBytes) bytes"
          )
        }
        let responseLength = try Self.readUInt32LE(metadata, at: 0)
        guard (Self.minimumResponseBytes...Self.maximumResponseBytes).contains(Int(responseLength))
        else {
          throw Self.invalid("secure-element response is outside the ABI-23 bound")
        }
        let expectedDigest = Data(metadata[Self.lengthBytes..<Self.responseMetadataBytes])
        var response = Data(capacity: Int(responseLength))
        chunkIndex = 0
        while response.count < Int(responseLength) {
          let count = min(Self.chunkBytes, Int(responseLength) - response.count)
          var chunk = try exchange(
            Self.shortCommand(
              instruction: Self.insReadResponse,
              p1: chunkIndex >> 8,
              p2: chunkIndex,
              expectedLength: count
            ),
            label: "read chunk \(chunkIndex)"
          )
          let chunkRange = chunk.startIndex..<chunk.endIndex
          defer { chunk.resetBytes(in: chunkRange) }
          guard chunk.count == count else {
            throw Self.invalid("secure-element response chunk \(chunkIndex) has the wrong length")
          }
          response.append(chunk)
          chunkIndex += 1
        }
        guard Self.constantTimeEqual(expectedDigest, Self.sha256(response)) else {
          let responseRange = response.startIndex..<response.endIndex
          response.resetBytes(in: responseRange)
          throw Self.invalid("secure-element response digest mismatch")
        }
        return response
      } catch {
        abortBestEffort()
        throw error
      }
    }
  }

  func close() {
    lock.lock()
    defer { lock.unlock() }
    abortBestEffort()
    channel.close()
  }

  private func locked<T>(_ body: () throws -> T) rethrows -> T {
    lock.lock()
    defer { lock.unlock() }
    return try body()
  }

  private func exchange(_ original: Data, label: String) throws -> Data {
    var command = original
    let commandRange = command.startIndex..<command.endIndex
    defer { command.resetBytes(in: commandRange) }
    var raw = try channel.transceive(command)
    let rawRange = raw.startIndex..<raw.endIndex
    defer { raw.resetBytes(in: rawRange) }
    guard raw.count >= Self.statusBytes else {
      throw Self.invalid("secure-element \(label) response omitted its status word")
    }
    let statusOffset = raw.count - Self.statusBytes
    let status = UInt16(raw[statusOffset]) << 8 | UInt16(raw[statusOffset + 1])
    guard status == Self.successStatus else {
      throw KagemushaDeviceLifecycleBridgeErrorV1.executionFailed
    }
    return Data(raw.prefix(statusOffset))
  }

  private func requireEmpty(_ response: Data, label: String) throws {
    guard response.isEmpty else {
      throw Self.invalid("secure-element \(label) returned unexpected bytes")
    }
  }

  private func abortBestEffort() {
    var command = Self.shortCommand(instruction: Self.insAbortTransport)
    let range = command.startIndex..<command.endIndex
    defer { command.resetBytes(in: range) }
    if var response = try? channel.transceive(command) {
      let responseRange = response.startIndex..<response.endIndex
      response.resetBytes(in: responseRange)
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
    var result = Data([
      cla, instruction, UInt8(truncatingIfNeeded: p1), UInt8(truncatingIfNeeded: p2),
    ])
    if !data.isEmpty {
      result.append(UInt8(data.count))
      result.append(data)
    } else if let expectedLength {
      result.append(UInt8(truncatingIfNeeded: expectedLength))
    }
    return result
  }

  private static func readUInt32LE(_ data: Data, at offset: Int) throws -> UInt32 {
    guard offset >= 0, data.count - offset >= lengthBytes else {
      throw invalid("truncated secure-element response length")
    }
    return (0..<lengthBytes).reduce(UInt32(0)) { value, index in
      value | UInt32(data[offset + index]) << UInt32(index * 8)
    }
  }

  private static func uint32LE(_ value: UInt32) -> Data {
    Data(
      (0..<lengthBytes).map { index in
        UInt8(truncatingIfNeeded: value >> UInt32(index * 8))
      })
  }

  private static func sha256(_ data: Data) -> Data {
    Data(SHA256.hash(data: data))
  }

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
  private static let lengthBytes = 4
  private static let digestBytes = 32
  private static let responseMetadataBytes = lengthBytes + digestBytes
  private static let capabilityBytes = 96
  private static let minimumCommandBytes = 80
  private static let maximumCommandBytes = 80 + 64 * 1024
  private static let minimumResponseBytes = 116
  private static let maximumResponseBytes = 116 + 64 * 1024 + 8 * 1024
  private static let statusBytes = 2
  private static let successStatus: UInt16 = 0x9000
}
