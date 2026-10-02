import CryptoKit
import Foundation

/// Exact bounded public projection of a native preparation. This parser does not
/// construct an owner, permit, identity credential or monetary capability.
struct KagemushaAppPlatformPreparedProjectionV1: Sendable {
  let ticket: Data
  let signingBytes: Data
  let platform: UInt8
  let keyAlias: String
  let generationChallenge: Data
  let publicKeyX963: Data
  let keyID: Data
  let enrollmentChallenge: Data
  let credentialDigest: Data
  let nativeScope: Data
  let appleCounterFloor: UInt32?
  let androidLevelsMask: UInt8
  let appSigningIdentityDigest: Data
  let financialSubject: Data
  let approval: KagemushaAppApprovalSigningProjectionV1?
  let bootstrapApproval: KagemushaBootstrapAppApprovalSigningProjectionV1?

  init(nativeFields: [Data], approvalID: Data?, enrollmentChallengeHash: Data?) throws {
    try self.init(nativeFields: nativeFields, approvalID: approvalID,
      enrollmentChallengeHash: enrollmentChallengeHash, bootstrapOperationID: nil, bootstrapCredentialDigest: nil)
  }

  init(nativeBootstrapFields: [Data], operationID: Data, credentialDigest: Data) throws {
    try self.init(nativeFields: nativeBootstrapFields, approvalID: nil,
      enrollmentChallengeHash: nil, bootstrapOperationID: operationID, bootstrapCredentialDigest: credentialDigest)
  }

  /// Phase1 accepts only purpose2 ordinary preparation W; Bootstrap has its own typed phase8 entry.
  static func validateApprovalTransport(_ fields: [Data], operationID: Data) throws {
    _ = try Self(nativeFields: fields, approvalID: operationID, enrollmentChallengeHash: nil)
  }

  /// Phase8 validates only exact zero-State Bootstrap transport, granting no native holder.
  static func validateBootstrapTransport(_ fields: [Data], operationID: Data) throws {
    guard fields.count == 14 else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid native bootstrap fields")
    }
    _ = try Self(nativeBootstrapFields: fields, operationID: operationID, credentialDigest: fields[8])
  }

  private init(nativeFields: [Data], approvalID: Data?, enrollmentChallengeHash: Data?,
    bootstrapOperationID: Data?, bootstrapCredentialDigest: Data?) throws {
    let f = nativeFields.map { Data($0) }
    guard f.count == 14, [approvalID, enrollmentChallengeHash, bootstrapOperationID].compactMap({ $0 }).count == 1,
      f[0].count == 8, Self.nonzero(f[0]), f[2].count == 1,
      [UInt8(4), 5].contains(f[2][0]), (1...255).contains(f[3].count),
      !f[3].contains(0), let alias = String(data: f[3], encoding: .utf8),
      Self.digest(f[4]), f[5].count == 65, f[5].first == 4,
      (try? P256.Signing.PublicKey(x963Representation: f[5])) != nil,
      Self.digest(f[6]), f[6] == Data(SHA256.hash(data: f[5])),
      Self.digest(f[9]), f[11].count == 1, Self.digest(f[12]) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid native platform preparation")
    }
    let challenge = try KagemushaOrdinaryAppEnrollmentProjectionV1(f[7])
    guard challenge.platform == f[2][0], f[4] == Data(SHA256.hash(data: f[7])) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("substituted original enrollment challenge")
    }
    let floor: UInt32?
    if f[2][0] == 4 {
      guard f[10].count == 4, f[11] == Data([0]),
        let aliasDigest = Data(base64Encoded: alias), aliasDigest.count == 32,
        aliasDigest.base64EncodedString() == alias, aliasDigest == f[6] else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid native Apple key or counter policy")
      }
      floor = Self.u32(f[10])
      guard floor! < UInt32.max else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("exhausted native Apple counter")
      }
    } else {
      guard f[10].isEmpty, [UInt8(1), 2, 3].contains(f[11][0]),
        alias == challenge.androidKeyAlias else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid native Android security policy")
      }
      floor = nil
    }
    let approval: KagemushaAppApprovalSigningProjectionV1?
    let bootstrapApproval: KagemushaBootstrapAppApprovalSigningProjectionV1?
    if let id = approvalID ?? bootstrapOperationID {
      let w: KagemushaAppOperationApprovalWrapperV1
      if let digest = bootstrapCredentialDigest {
        let bootstrap = try KagemushaBootstrapAppApprovalSigningProjectionV1(nativeSigningBytes: f[1],
          nativeFinancialSubject: f[13], credentialDigest: digest)
        bootstrapApproval = bootstrap; approval = nil; w = bootstrap.wrapper
      } else {
        let ordinary = try KagemushaAppApprovalSigningProjectionV1(nativePreparationSigningBytes: f[1],
          nativeFinancialSubject: f[13])
        approval = ordinary; bootstrapApproval = nil; w = ordinary.wrapper
      }
      guard Self.digest(id), w.operationID == id, Self.digest(f[8]),
        w.enrollmentDigest == f[8], w.attestedKeyID == f[6],
        Data(w.canonicalFinancialSubject[155..<187]) == f[8],
        Self.u64(w.canonicalFinancialSubject, 323) == challenge.hardwareEpoch,
        Data(w.canonicalFinancialSubject[59..<91]) == challenge.releaseID,
        Data(w.canonicalFinancialSubject[187..<219]) == challenge.networkID,
        Data(w.canonicalFinancialSubject[219..<251]) == challenge.laneID,
        Data(w.canonicalFinancialSubject[251..<283]) == challenge.profileID,
        Self.u64(w.canonicalFinancialSubject, 283) == challenge.policyEpoch,
        w.accountBinding == challenge.accountBinding,
        w.authorityPolicyDigest == challenge.appAuthorityPolicyDigest else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("native W scope or key differs")
      }
    } else {
      guard f[8].isEmpty, f[13].isEmpty else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("pending enrollment carries credential or financial subject")
      }
      try Self.validateEnrollmentPossession(f[1], id: enrollmentChallengeHash!, key: f[6], challenge: challenge)
      approval = nil; bootstrapApproval = nil
    }
    ticket = f[0]; signingBytes = f[1]; platform = f[2][0]; keyAlias = alias
    generationChallenge = f[4]; publicKeyX963 = f[5]; keyID = f[6]
    enrollmentChallenge = f[7]; credentialDigest = f[8]; nativeScope = f[9]
    appleCounterFloor = floor; androidLevelsMask = f[11][0]
    appSigningIdentityDigest = f[12]; financialSubject = f[13]; self.approval = approval; self.bootstrapApproval = bootstrapApproval
  }

  private static func validateEnrollmentPossession(_ e: Data, id: Data, key: Data,
    challenge c: KagemushaOrdinaryAppEnrollmentProjectionV1) throws {
    let domain = Data("iroha:kagemusha:v1:app-enrollment-possession\0".utf8)
    let start = domain.count + 8
    guard e.count == start + 371, e.starts(with: domain), u64(e, domain.count) == 371,
      e[start] == 1, e[start + 1] == 0, e[start + 2] == 1 else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid native E layout or purpose")
    }
    let fields = (0..<11).map { Data(e[(start + 3 + $0 * 32)..<(start + 35 + $0 * 32)]) }
    let issue = u64(e, start + 355), expiry = u64(e, start + 363)
    guard fields.allSatisfy(digest), fields[0] == id,
      fields[0] == Data(SHA256.hash(data:c.canonicalSigningBytes)),
      fields[1] == c.clientNonce, fields[2] == c.serverNonce,
      fields[3] == c.accountBinding, fields[4] == c.networkID,
      fields[5] == c.appAuthorityPolicyDigest, fields[6] == c.releaseID,
      fields[7] == c.profileID, fields[8] == c.laneID, fields[9] == key,
      fields[1] != fields[2], issue > 0, expiry > issue, expiry - issue <= 120_000,
      issue == c.issuedAtMS, expiry == c.expiresAtMS else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("native E original scope differs")
    }
  }

  static func digest(_ bytes: Data) -> Bool { bytes.count == 32 && nonzero(bytes) }
  static func nonzero(_ bytes: Data) -> Bool { bytes.contains { $0 != 0 } }
  static func u32(_ bytes: Data) -> UInt32 {
    bytes.enumerated().reduce(UInt32(0)) { $0 | UInt32($1.element) << ($1.offset * 8) }
  }
  static func u64(_ bytes: Data, _ offset: Int) -> UInt64 {
    bytes[offset..<(offset + 8)].enumerated().reduce(UInt64(0)) {
      $0 | UInt64($1.element) << ($1.offset * 8)
    }
  }
}

/// Structural C451 decoder; freshness and issuer authority remain native checks.
struct KagemushaOrdinaryAppEnrollmentProjectionV1: Sendable {
  let canonicalSigningBytes: Data
  let platform: UInt8
  let enrollmentID, clientNonce, serverNonce, accountBinding, networkID: Data
  let laneID, releaseID, profileID, appAuthorityPolicyDigest: Data
  let policyEpoch, hardwareEpoch, issuedAtMS, expiresAtMS: UInt64

  init(_ input: Data) throws {
    let c = Data(input)
    let domain = Data("iroha:kagemusha:v1:ordinary-app-enrollment-challenge\0".utf8)
    let start = domain.count + 8
    guard c.count == start + 451, c.starts(with: domain),
      KagemushaAppPlatformPreparedProjectionV1.u64(c, domain.count) == 451,
      c[start] == 1, c[start + 1] == 0, [UInt8(1), 2].contains(c[start + 2]) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid original C451 layout")
    }
    let f = (0..<13).map { Data(c[(start + 3 + $0 * 32)..<(start + 35 + $0 * 32)]) }
    let policyEpoch = KagemushaAppPlatformPreparedProjectionV1.u64(c, start + 419)
    let hardwareEpoch = KagemushaAppPlatformPreparedProjectionV1.u64(c, start + 427)
    let issue = KagemushaAppPlatformPreparedProjectionV1.u64(c, start + 435)
    let expiry = KagemushaAppPlatformPreparedProjectionV1.u64(c, start + 443)
    guard f.allSatisfy(KagemushaAppPlatformPreparedProjectionV1.digest), f[1] != f[2],
      policyEpoch > 0, hardwareEpoch > 0, issue > 0, expiry > issue,
      expiry - issue <= 120_000 else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid original C451 scope or interval")
    }
    // C uses its model-owned ordinary tag1Android/2Apple. The coordinator
    // separately projects the platform enum tag5Android/4Apple; no alternate C wire.
    platform = c[start + 2] == 1 ? 5 : 4
    enrollmentID = f[0]; clientNonce = f[1]; serverNonce = f[2]
    accountBinding = f[3]; networkID = f[4]; laneID = f[5]; releaseID = f[6]
    profileID = f[7]; appAuthorityPolicyDigest = f[10]
    self.policyEpoch = policyEpoch; self.hardwareEpoch = hardwareEpoch
    issuedAtMS = issue; expiresAtMS = expiry; canonicalSigningBytes = c
  }

  /// Correlation only: native custody independently retains this exact original alias.
  var androidKeyAlias: String? {
    guard platform == 5 else { return nil }
    let domain = Data("iroha:kagemusha:v1:ordinary-app-key-alias\0".utf8)
    let hash = Data(SHA256.hash(data: domain + canonicalSigningBytes))
    return "kagemusha-ordinary-app-v1-" + hash.map { String(format: "%02x", $0) }.joined()
  }
}
