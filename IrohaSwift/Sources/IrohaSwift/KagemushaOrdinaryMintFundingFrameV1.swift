// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation

/// Select one phase on the same retained ordinary Native funding owner.
/// Selectors and returned originals confer no account, debit or financial authority.
public enum KagemushaOrdinaryMintFundingPhaseV1: UInt8, CaseIterable, Sendable {
  case prepareMint = 1, fencePlatform = 2, retainPlatformOriginal = 3
  case proveMint = 4, signAccountConsent = 5, preparePreDebit = 6, fencePreDebit = 7
  case retainCoreDecision = 8, signTransaction = 9, submitTransaction = 10
  case readFinality = 11, recoverPlatform = 12, refreshAccountClock = 13
  case recoverPreDebit = 14, originalPlatformCounter = 15
  case acknowledgeRetainedPlatformOriginal = 16
  /// Read the existing original operation and acknowledged WAL stage. This does not retry effects.
  case retainedProgress = 17
}

/// Invoke the dedicated Native funding ABI. The implementation retains the exact
/// coordinator descriptor; callers supply neither an owner handle nor trust inputs.
public protocol KagemushaNativeMintFundingCoreCoordinatorV1: AnyObject {
  func invokeOrdinaryMintFunding(_ phase: KagemushaOrdinaryMintFundingPhaseV1,
    originals: [Data]) throws -> [Data]
}

/// Sole first-release Norito frame for dedicated ordinary Mint funding. These are
/// full bounded transport originals. Parsing does not admit an issuer, proof or clock.
enum KagemushaOrdinaryMintFundingFrameV1 {
  static let nodeMaximum = 60 * 1024 * 1024
  static let clockMaximum = 16 * 1024 * 1024 + 4_096
  static let controlMaximum = 32_768
  static let decisionMaximum = 16_384
  static let dataMaximum = 131_072
  static let credentialMaximum = 16_384
  static let selectionMaximum = 32_768
  static let requestMaximum = 65_536
  static let platformMaximum = 4_096
  static let finalizedMaximum = 38_273_024
  static let frameMaximum = nodeMaximum + clockMaximum + controlMaximum
    + decisionMaximum + dataMaximum + 4_096
  private static let requestType = "connect_norito_bridge::KagemushaOrdinaryNativeMintFundingRequestV1"
  private static let responseType = "connect_norito_bridge::KagemushaOrdinaryNativeMintFundingResponseV1"

  static func encodeRequest(_ phase: KagemushaOrdinaryMintFundingPhaseV1,
    handle: UInt64, originals: [Data]) throws -> Data {
    try requireRequest(phase, handle: handle, originals: originals)
    return try frame(requestType, phase: phase, handle: handle, fields: originals)
  }

  static func decodeResponse(_ phase: KagemushaOrdinaryMintFundingPhaseV1,
    handle: UInt64, response: Data) throws -> [Data] {
    guard handle != 0, (1...frameMaximum).contains(response.count),
      let archive = noritoDecodeFrame(response), archive.header.flags == NoritoHeader.compactLen,
      archive.header.schema == noritoSchemaHash(forTypeName: responseType),
      archive.paddingLength == 0 else { throw invalid("invalid ordinary Mint funding Norito envelope") }
    var reader = CanonicalNoritoReader(data: archive.payload)
    guard try reader.readCompactField() == CompactNorito.encodeUInt16(1),
      try reader.readCompactField() == Data([phase.rawValue]),
      try reader.readCompactField() == CompactNorito.encodeUInt64(handle)
    else { throw invalid("ordinary Mint funding response correlation differs") }
    let fields = try vector(reader.readCompactField())
    guard reader.remaining() == 0 else { throw invalid("trailing ordinary Mint funding payload") }
    try requireResponse(phase, fields: fields)
    guard try frame(responseType, phase: phase, handle: handle, fields: fields) == response
    else { throw invalid("noncanonical ordinary Mint funding response") }
    return fields
  }

  // Internal data fixture encoder; it grants no Native or financial capability.
  static func encodeResponse(_ phase: KagemushaOrdinaryMintFundingPhaseV1,
    handle: UInt64, fields: [Data]) throws -> Data {
    guard handle != 0 else { throw invalid("zero ordinary Mint funding handle") }
    try requireResponse(phase, fields: fields)
    return try frame(responseType, phase: phase, handle: handle, fields: fields)
  }

  private static func frame(_ type: String, phase: KagemushaOrdinaryMintFundingPhaseV1,
    handle: UInt64, fields: [Data]) throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(CompactNorito.encodeUInt16(1))
    writer.writeField(Data([phase.rawValue]))
    writer.writeField(CompactNorito.encodeUInt64(handle))
    writer.writeField(try CompactNorito.encodeVec(fields, encode: CompactNorito.encodeBytesVec))
    let result = noritoEncode(typeName: type, payload: writer.data,
      flags: NoritoHeader.compactLen, payloadAlignment: 8)
    guard result.count <= frameMaximum else { throw invalid("ordinary Mint funding frame exceeds Native bound") }
    return result
  }

  private static func vector(_ bytes: Data) throws -> [Data] {
    var reader = CanonicalNoritoReader(data: bytes)
    let count = try reader.readUInt64LE()
    guard count <= 8 else { throw invalid("ordinary Mint funding field count exceeds phase grammar") }
    var result: [Data] = []
    for _ in 0..<count {
      var element = CanonicalNoritoReader(data: try reader.readCompactField())
      let length = try element.readUInt64LE()
      guard length <= UInt64(frameMaximum), length == UInt64(element.remaining())
      else { throw invalid("ordinary Mint funding blob length differs") }
      result.append(try element.readBytes(Int(length)))
    }
    guard reader.remaining() == 0 else { throw invalid("trailing ordinary Mint funding vector") }
    return result
  }

  private static func requireRequest(_ phase: KagemushaOrdinaryMintFundingPhaseV1,
    handle: UInt64, originals: [Data]) throws {
    guard handle != 0 else { throw invalid("zero ordinary Mint funding handle") }
    let valid: Bool
    switch phase {
    case .prepareMint:
      valid = originals.count == 1 && originals[0].count == 16
        && originals[0].contains(where: { $0 != 0 })
    case .retainPlatformOriginal:
      valid = originals.count == 1 && bounded(originals[0], platformMaximum)
    case .retainCoreDecision:
      valid = originals.count == 5 && bounded(originals[0], decisionMaximum)
        && bounded(originals[1], clockMaximum) && bounded(originals[2], controlMaximum)
        && bounded(originals[3], dataMaximum) && bounded(originals[4], nodeMaximum)
    default: valid = originals.isEmpty
    }
    guard valid else { throw invalid("ordinary Mint funding request phase originals differ") }
  }

  private static func requireResponse(_ phase: KagemushaOrdinaryMintFundingPhaseV1,
    fields: [Data]) throws {
    let valid: Bool
    switch phase {
    case .prepareMint:
      valid = fields.count == 3 && nonzeroDigest(fields[0])
        && bounded(fields[1], platformMaximum) && bounded(fields[2], credentialMaximum)
    case .proveMint, .signTransaction:
      valid = fields.count == 1 && nonzeroDigest(fields[0])
    case .signAccountConsent:
      valid = fields.count == 1 && fields[0].count == 64
    case .preparePreDebit, .fencePreDebit, .recoverPreDebit:
      valid = fields.count == 8 && bounded(fields[0], selectionMaximum)
        && bounded(fields[1], requestMaximum) && fields[2].count == 64
        && bounded(fields[3], credentialMaximum) && fields[4].count <= platformMaximum
        && bounded(fields[5], clockMaximum) && bounded(fields[6], controlMaximum)
        && bounded(fields[7], controlMaximum)
    case .readFinality:
      valid = fields.count == 2 && ((fields[0] == Data([0]) && fields[1].isEmpty)
        || (fields[0] == Data([1]) && bounded(fields[1], finalizedMaximum)))
    case .recoverPlatform:
      valid = fields.count == 5 && [Data([0]), Data([1]), Data([2]), Data([3])].contains(fields[0])
        && nonzeroDigest(fields[1]) && bounded(fields[2], platformMaximum)
        && bounded(fields[3], credentialMaximum) && fields[4].count <= platformMaximum
        && ([Data([0]), Data([1])].contains(fields[0]) ? fields[4].isEmpty : !fields[4].isEmpty)
    case .retainedProgress:
      valid = fields.count == 2 && nonzeroDigest(fields[0]) && fields[1].count == 1
        && fields[1].first.map { $0 <= 12 } == true
    case .originalPlatformCounter:
      valid = fields.count == 2 && ((fields[0] == Data([5]) && fields[1].isEmpty)
        || (fields[0] == Data([4]) && fields[1].count == 4))
    case .fencePlatform, .retainPlatformOriginal, .retainCoreDecision, .submitTransaction,
      .refreshAccountClock, .acknowledgeRetainedPlatformOriginal:
      valid = fields.isEmpty
    }
    guard valid else { throw invalid("ordinary Mint funding response phase originals differ") }
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
