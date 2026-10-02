// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation
import CryptoKit

/// Select an operation on the same retained ordinary Native Cash owner.
/// These selectors and returned archives grant no financial or signing authority.
public enum KagemushaOrdinaryIncomingPhaseV1: UInt8, CaseIterable, Sendable {
  case prepareFinalizedMint = 1, fencePreparation = 2, retainPreparationAssertion = 3
  case recoverPreparation = 4, proveCandidate = 5, reserveTransport = 6
  case retainGlobalResult = 7, prepareTerminal = 8, fenceTerminal = 9
  case retainTerminalAssertion = 10, recoverTerminal = 11, proveCommit = 12
  case commitTransport = 13, advanceState = 14, acknowledge = 15
  case refreshAccountClock = 16, prepareReceive = 17, originalPlatformCounter = 18
  case originalPlatformSigning = 19
}

/// The sole first-release Norito transport for the ordinary incoming lifecycle.
/// Full originals use the actual release-derived Native ceilings. Parsing is data-only;
/// Native still authenticates every original, account, proof, clock and durable transition.
enum KagemushaOrdinaryIncomingFrameV1 {
  static let finalizedMaximum = 38_273_024
  static let creditMaximum = 7_936
  static let outgoingMaximum = 17_043_456
  static let receivedMaximum = 16_973_824
  static let signedMaximum = 32_768
  static let dataMaximum = 131_072
  static let authorityMaximum = 134_217_728
  static let requestMaximum = 196_608
  static let reserveMaximum = 101_498_880
  static let commitMaximum = 135_266_304
  static let frameMaximum = authorityMaximum + signedMaximum + finalizedMaximum
    + creditMaximum + outgoingMaximum + receivedMaximum + 4_096
  private static let requestType = "connect_norito_bridge::KagemushaOrdinaryNativeIncomingRequestV1"
  private static let responseType = "connect_norito_bridge::KagemushaOrdinaryNativeIncomingResponseV1"

  static func encodeRequest(_ phase: KagemushaOrdinaryIncomingPhaseV1,
    handle: UInt64, originals: [Data]) throws -> Data {
    try requireRequest(phase, handle: handle, originals: originals)
    return try frame(requestType, phase: phase, handle: handle, fields: originals)
  }

  static func decodeResponse(_ phase: KagemushaOrdinaryIncomingPhaseV1,
    handle: UInt64, response: Data) throws -> [Data] {
    guard handle != 0, (1...frameMaximum).contains(response.count),
      let archive = noritoDecodeFrame(response), archive.header.flags == NoritoHeader.compactLen,
      archive.header.schema == noritoSchemaHash(forTypeName: responseType),
      archive.paddingLength == 0 else { throw invalid("invalid ordinary incoming Norito envelope") }
    var reader = CanonicalNoritoReader(data: archive.payload)
    guard try reader.readCompactField() == CompactNorito.encodeUInt16(1),
      try reader.readCompactField() == Data([phase.rawValue]),
      try reader.readCompactField() == CompactNorito.encodeUInt64(handle)
    else { throw invalid("ordinary incoming response correlation differs") }
    let fields = try vector(reader.readCompactField())
    guard reader.remaining() == 0 else { throw invalid("trailing ordinary incoming payload") }
    try requireResponse(phase, fields: fields)
    guard try frame(responseType, phase: phase, handle: handle, fields: fields) == response
    else { throw invalid("noncanonical ordinary incoming response") }
    return fields
  }

  // Internal fixture encoder. It creates no Native capabilities or proof qualification.
  static func encodeResponse(_ phase: KagemushaOrdinaryIncomingPhaseV1,
    handle: UInt64, fields: [Data]) throws -> Data {
    guard handle != 0 else { throw invalid("zero ordinary incoming handle") }
    try requireResponse(phase, fields: fields)
    return try frame(responseType, phase: phase, handle: handle, fields: fields)
  }

  private static func frame(_ type: String, phase: KagemushaOrdinaryIncomingPhaseV1,
    handle: UInt64, fields: [Data]) throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(CompactNorito.encodeUInt16(1))
    writer.writeField(Data([phase.rawValue]))
    writer.writeField(CompactNorito.encodeUInt64(handle))
    writer.writeField(try CompactNorito.encodeVec(fields, encode: CompactNorito.encodeBytesVec))
    let result = noritoEncode(typeName: type, payload: writer.data,
      flags: NoritoHeader.compactLen, payloadAlignment: 8)
    guard result.count <= frameMaximum else { throw invalid("ordinary incoming frame exceeds release bound") }
    return result
  }

  private static func vector(_ bytes: Data) throws -> [Data] {
    var reader = CanonicalNoritoReader(data: bytes)
    let count = try reader.readUInt64LE()
    guard count <= 11 else { throw invalid("ordinary incoming field count exceeds phase grammar") }
    var result: [Data] = []
    for _ in 0..<count {
      var element = CanonicalNoritoReader(data: try reader.readCompactField())
      let length = try element.readUInt64LE()
      guard length <= UInt64(frameMaximum), length == UInt64(element.remaining())
      else { throw invalid("ordinary incoming blob length differs") }
      result.append(try element.readBytes(Int(length)))
    }
    guard reader.remaining() == 0 else { throw invalid("trailing ordinary incoming vector") }
    return result
  }

  static func requireRequest(_ phase: KagemushaOrdinaryIncomingPhaseV1,
    handle: UInt64, originals: [Data]) throws {
    guard handle != 0 else { throw invalid("zero ordinary incoming handle") }
    try requireOriginals(phase, originals: originals)
  }

  static func requireOriginals(_ phase: KagemushaOrdinaryIncomingPhaseV1, originals: [Data]) throws {
    let valid: Bool
    switch phase {
    case .prepareFinalizedMint:
      valid = originals.count == 2 && bounded(originals[0], finalizedMaximum)
        && bounded(originals[1], creditMaximum)
    case .prepareReceive:
      valid = originals.count == 3 && nonzeroDigest(originals[0])
        && bounded(originals[1], outgoingMaximum) && bounded(originals[2], receivedMaximum)
    case .originalPlatformCounter, .originalPlatformSigning:
      valid = originals.count == 2 && nonzeroDigest(originals[0])
        && [Data([1]), Data([2])].contains(originals[1])
    case .retainPreparationAssertion, .retainTerminalAssertion:
      valid = originals.count == 1 && bounded(originals[0], 4_096)
    case .retainGlobalResult:
      valid = originals.count == 3 && bounded(originals[0], signedMaximum)
        && bounded(originals[1], dataMaximum) && bounded(originals[2], authorityMaximum)
    case .prepareTerminal, .advanceState, .acknowledge:
      valid = originals.count == 1 && nonzeroDigest(originals[0])
    default: valid = originals.isEmpty
    }
    guard valid else { throw invalid("ordinary incoming request phase originals differ") }
  }

  static func requireResponse(_ phase: KagemushaOrdinaryIncomingPhaseV1,
    fields: [Data]) throws {
    let valid: Bool
    switch phase {
    case .prepareFinalizedMint, .prepareTerminal, .prepareReceive:
      valid = fields.count == 4 && nonzeroDigest(fields[0]) && fields[1].count == 325
        && fields[2].count == 460 && bounded(fields[3], signedMaximum)
    case .fencePreparation, .recoverPreparation, .fenceTerminal, .recoverTerminal:
      valid = fields.count == 3 && [Data([0]), Data([1]), Data([2])].contains(fields[0])
        && fields[1].count <= 4_096 && fields[2].count <= signedMaximum
        && (fields[0] == Data([0]) ? fields[1].isEmpty && fields[2].isEmpty
          : !fields[1].isEmpty && !fields[2].isEmpty)
    case .retainPreparationAssertion, .proveCandidate, .retainGlobalResult,
      .retainTerminalAssertion, .proveCommit:
      valid = fields.count == 1 && nonzeroDigest(fields[0])
    case .reserveTransport, .commitTransport:
      valid = fields.count == 5 && [Data([0]), Data([2])].contains(fields[0])
        && bounded(fields[1], requestMaximum) && fields[2].count == 64
        && bounded(fields[3], phase == .reserveTransport ? reserveMaximum : commitMaximum)
        && fields[4] == Data(SHA256.hash(data: fields[1]))
    case .originalPlatformCounter:
      valid = fields.count == 2 && ((fields[0] == Data([5]) && fields[1].isEmpty)
        || (fields[0] == Data([4]) && fields[1].count == 4))
    case .originalPlatformSigning:
      _ = try KagemushaOrdinaryIncomingSigningProjectionV1(fields)
      return
    case .advanceState, .acknowledge, .refreshAccountClock: valid = fields.isEmpty
    }
    guard valid else { throw invalid("ordinary incoming response phase originals differ") }
  }

  private static func bounded(_ data: Data, _ maximum: Int) -> Bool {
    (1...maximum).contains(data.count)
  }
  private static func nonzeroDigest(_ data: Data) -> Bool {
    data.count == 32 && data.contains(where: { $0 != 0 })
  }
  private static func invalid(_ message: String) -> KagemushaCoreCoordinatorErrorV1 {
    .invalidFrame(message)
  }
}
