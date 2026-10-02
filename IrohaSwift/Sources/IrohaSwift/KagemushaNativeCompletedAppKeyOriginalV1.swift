// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation
import CryptoKit

/// Bounded historical metadata from one completed Native app-key enrollment.
/// These data fields create no current PI, signing, counter or financial capability.
public struct KagemushaCompletedAppKeyMetadataV1: Sendable {
  public let enrollmentID: Data
  public let keyReference: String
  public let generationChallengeDigest: Data
  public let publicKey: Data
  public let attestedKeyID: Data
  public let androidSecurityMask: UInt8
  public let credentialOriginal: Data
  public let financialCertificateOriginal: Data
  public let integrityPolicyOriginal: Data
  public let cloudProjectNumber: UInt64
  public let credentialDigest: Data

  fileprivate init(fields: [Data]) throws {
    guard fields.count == 11,
      fields.allSatisfy({ $0.count <= 65_536 }),
      fields.reduce(0, { $0 + $1.count }) <= 131_072,
      [0, 2, 4, 10].allSatisfy({ fields[$0].count == 32 && fields[$0].contains(where: { $0 != 0 }) }),
      let alias = String(data: fields[1], encoding: .utf8), !alias.isEmpty,
      fields[1].count <= 255, !fields[1].contains(0),
      fields[3].count == 65, fields[3].first == 4,
      fields[5].count == 1, fields[9].count == 8,
      !fields[6].isEmpty, !fields[7].isEmpty
    else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid completed app-key metadata") }
    _ = try P256.Signing.PublicKey(x963Representation: fields[3])
    let project = fields[9].enumerated().reduce(UInt64(0)) {
      $0 | UInt64($1.element) << ($1.offset * 8)
    }
    let mask = fields[5][0]
    guard (mask == 0 && fields[8].isEmpty && project == 0
      && alias == fields[4].base64EncodedString())
      || ([UInt8(1), 2, 3].contains(mask) && !fields[8].isEmpty && project != 0)
    else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("completed key platform policy differs") }
    enrollmentID = fields[0]; keyReference = alias
    generationChallengeDigest = fields[2]; publicKey = fields[3]; attestedKeyID = fields[4]
    androidSecurityMask = mask; credentialOriginal = fields[6]
    financialCertificateOriginal = fields[7]; integrityPolicyOriginal = fields[8]
    cloudProjectNumber = project; credentialDigest = fields[10]
  }
}

/// Same-descriptor, move-independent metadata holder with no public byte initializer.
/// Each read rechecks every completed original. Substitution or failure permanently freezes it.
public final class KagemushaNativeCompletedAppKeyOriginalV1: @unchecked Sendable {
  private let bridge: KagemushaCoreCoordinatorBridgeV1
  private let original: [Data]
  private let lock = NSLock()
  private var unusable = false

  private init(bridge: KagemushaCoreCoordinatorBridgeV1, fields: [Data]) {
    self.bridge = bridge
    original = fields.map { Data($0) }
  }

  static func fromNative(bridge: KagemushaCoreCoordinatorBridgeV1) throws
    -> KagemushaNativeCompletedAppKeyOriginalV1 {
    let fields = try bridge.completedAppKeyFields()
    let result = KagemushaNativeCompletedAppKeyOriginalV1(bridge: bridge, fields: fields)
    try result.requireOriginal()
    return result
  }

  public func requireOriginal() throws {
    lock.lock(); defer { lock.unlock() }
    try recheckLocked()
  }

  public func metadata() throws -> KagemushaCompletedAppKeyMetadataV1 {
    lock.lock(); defer { lock.unlock() }
    try recheckLocked()
    let result = try KagemushaCompletedAppKeyMetadataV1(fields: original)
    try recheckLocked()
    return result
  }

  func requireForCoordinator(_ other: KagemushaCoreCoordinatorBridgeV1) throws {
    lock.lock(); defer { lock.unlock() }
    guard bridge === other else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("completed key belongs to another Native coordinator")
    }
    try recheckLocked()
  }

  private func recheckLocked() throws {
    guard !unusable else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    do {
      guard try bridge.completedAppKeyFields() == original else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("completed app-key originals changed")
      }
    } catch {
      unusable = true
      try? bridge.close()
      throw error
    }
  }
}

extension KagemushaCoreCoordinatorBridgeV1 {
  public func completedAppKeyOriginal() throws -> KagemushaNativeCompletedAppKeyOriginalV1 {
    try KagemushaNativeCompletedAppKeyOriginalV1.fromNative(bridge: self)
  }
}

/// Decode the sole actual Native phase10 response with the maintained Norito codec.
/// The fixture encoder is internal; neither decoding nor encoding creates custody.
enum KagemushaCompletedAppKeyFrameV1 {
  static let frameMaximum = 192 * 1024
  private static let responseType = "connect_norito_bridge::KagemushaOrdinaryNativeIntegrityRefreshResponseV1"

  static func decodeResponse(handle: UInt64, response: Data) throws -> [Data] {
    guard handle != 0, (1...frameMaximum).contains(response.count),
      let archive = noritoDecodeFrame(response), archive.header.flags == NoritoHeader.compactLen,
      archive.header.schema == noritoSchemaHash(forTypeName: responseType), archive.paddingLength == 0
    else { throw invalid() }
    var reader = CanonicalNoritoReader(data: archive.payload)
    guard try reader.readCompactField() == CompactNorito.encodeUInt16(1),
      try reader.readCompactField() == Data([10]),
      try reader.readCompactField() == CompactNorito.encodeUInt64(handle)
    else { throw invalid() }
    var vector = CanonicalNoritoReader(data: try reader.readCompactField())
    guard try vector.readUInt64LE() == 11 else { throw invalid() }
    var fields: [Data] = []
    for _ in 0..<11 {
      var element = CanonicalNoritoReader(data: try vector.readCompactField())
      let length = try element.readUInt64LE()
      guard length <= 65_536, length == UInt64(element.remaining()) else { throw invalid() }
      fields.append(try element.readBytes(Int(length)))
    }
    guard reader.remaining() == 0, vector.remaining() == 0 else { throw invalid() }
    _ = try KagemushaCompletedAppKeyMetadataV1(fields: fields)
    guard try encodeResponse(handle: handle, fields: fields) == response else { throw invalid() }
    return fields
  }

  static func encodeResponse(handle: UInt64, fields: [Data]) throws -> Data {
    guard handle != 0 else { throw invalid() }
    _ = try KagemushaCompletedAppKeyMetadataV1(fields: fields)
    var writer = CompactNoritoWriter()
    writer.writeField(CompactNorito.encodeUInt16(1))
    writer.writeField(Data([10]))
    writer.writeField(CompactNorito.encodeUInt64(handle))
    writer.writeField(try CompactNorito.encodeVec(fields, encode: CompactNorito.encodeBytesVec))
    let result = noritoEncode(typeName: responseType, payload: writer.data,
      flags: NoritoHeader.compactLen, payloadAlignment: 8)
    guard result.count <= frameMaximum else { throw invalid() }
    return result
  }

  private static func invalid() -> KagemushaCoreCoordinatorErrorV1 {
    .invalidFrame("invalid completed app-key Native response")
  }
}
