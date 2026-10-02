import CryptoKit
import Foundation

/// Closed transport grammar. No decoded projection constructs a Native owner or grant.
enum KagemushaOrdinaryOutgoingFrameV1 {
  static let requestMaximum = 192 * 1024
  static let signedMaximum = 32 * 1024
  static let dataMaximum = 128 * 1024
  static let authorityMaximum = 128 * 1024 * 1024
  static let proofMaximum = 64 * 1024 * 1024

  static func requireRequest(_ phase: UInt8, _ fields: [Data]) throws {
    let valid: Bool
    switch phase {
    case 5, 6, 9, 11, 12, 13, 16, 18: valid = fields.isEmpty
    case 8, 14, 15, 17: valid = fields.count == 1 && digest(fields[0])
    case 10: valid = fields.count == 1 && (1...4096).contains(fields[0].count)
    case 7: valid = fields.count == 3 && (1...signedMaximum).contains(fields[0].count) &&
      (1...dataMaximum).contains(fields[1].count) && (1...authorityMaximum).contains(fields[2].count)
    default: valid = false
    }
    guard valid else { throw invalid("ordinary outgoing request grammar differs") }
  }
  static func requireResponse(_ phase: UInt8, _ fields: [Data]) throws {
    let valid: Bool
    switch phase {
    case 5, 7, 10, 12: valid = fields.count == 1 && digest(fields[0])
    case 6, 13:
      valid = fields.count == 5 && [Data([0]), Data([2])].contains(fields[0]) &&
        (1...requestMaximum).contains(fields[1].count) && fields[2].count == 64 &&
        (1...proofMaximum).contains(fields[3].count) && digest(fields[4]) && sha(fields[1]) == fields[4]
    case 8, 18: _ = try KagemushaOrdinaryTerminalProjectionV1(fields); return
    case 9, 11:
      valid = fields.count == 3 && [Data([0]), Data([1]), Data([2])].contains(fields[0]) &&
        (fields[0] == Data([0]) ? fields[1].isEmpty && fields[2].isEmpty :
          (1...4096).contains(fields[1].count) && (1...signedMaximum).contains(fields[2].count))
    case 14, 15, 16: valid = fields.isEmpty
    case 17: valid = fields.count == 1 && (1...proofMaximum).contains(fields[0].count)
    default: valid = false
    }
    guard valid else { throw invalid("ordinary outgoing response grammar differs") }
  }
  static func digest(_ bytes: Data) -> Bool { bytes.count == 32 && bytes.contains { $0 != 0 } }
  static func sha(_ bytes: Data) -> Data { Data(SHA256.hash(data: bytes)) }
  static func invalid(_ message: String) -> KagemushaCoreCoordinatorErrorV1 { .invalidFrame(message) }
}

/// Exact purpose1 W/S/C/key projection, distinct from purpose2 and zero-State Bootstrap.
struct KagemushaOrdinaryTerminalProjectionV1: Sendable {
  let fields: [Data]
  let approval: KagemushaAppApprovalSigningProjectionV1
  let platform: UInt8
  let keyAlias: String
  let publicKey: Data
  let keyID: Data
  let appID: Data
  let counterFloor: UInt32?

  init(_ fields: [Data]) throws {
    let f = fields.map { Data($0) }
    guard f.count == 14, KagemushaOrdinaryOutgoingFrameV1.digest(f[0]),
      f[1].count == 325, f[9].count == 460, [Data([4]), Data([5])].contains(f[2]),
      (1...128).contains(f[3].count), !f[3].contains(0),
      let alias = String(data: f[3], encoding: .utf8), Data(alias.utf8) == f[3],
      !alias.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
      [f[4], f[6], f[8], f[12]].allSatisfy(KagemushaOrdinaryOutgoingFrameV1.digest),
      f[5].count == 65, f[5].first == 4,
      (try? P256.Signing.PublicKey(x963Representation: f[5])) != nil,
      KagemushaOrdinaryOutgoingFrameV1.sha(f[5]) == f[6],
      (1...KagemushaOrdinaryOutgoingFrameV1.signedMaximum).contains(f[13].count) else {
      throw KagemushaOrdinaryOutgoingFrameV1.invalid("ordinary terminal transport differs")
    }
    let c = try KagemushaOrdinaryAppEnrollmentProjectionV1(f[7])
    let w = try KagemushaAppApprovalSigningProjectionV1(nativeSigningBytes: f[1], nativeFinancialSubject: f[9])
    _ = try KagemushaOrdinaryCashApprovalProjectionV1.requireTerminal(nativeSigningBytes: f[1],
      nativeFinancialSubject: f[9], binding: KagemushaOrdinaryCashApprovalOriginalBindingV1(
        operationID: f[0], accountBinding: c.accountBinding,
        authorityPolicyDigest: c.appAuthorityPolicyDigest, attestedKeyID: f[6],
        enrollmentDigest: f[8], normalizedGuardDigest: w.normalizedGuardDigest,
        originalSelection: f[9]))
    let s = w.canonicalFinancialSubject
    guard c.platform == f[2][0], KagemushaOrdinaryOutgoingFrameV1.sha(f[7]) == f[4],
      w.operationID == f[0], w.attestedKeyID == f[6], w.enrollmentDigest == f[8],
      w.accountBinding == c.accountBinding, w.authorityPolicyDigest == c.appAuthorityPolicyDigest,
      Data(s[155..<187]) == f[8], Data(s[59..<91]) == c.releaseID,
      Data(s[187..<219]) == c.networkID, Data(s[219..<251]) == c.laneID,
      Data(s[251..<283]) == c.profileID,
      KagemushaAppPlatformPreparedProjectionV1.u64(s, 283) == c.policyEpoch,
      KagemushaAppPlatformPreparedProjectionV1.u64(s, 323) == c.hardwareEpoch,
      [UInt8(2), 4].contains(s[331]) else {
      throw KagemushaOrdinaryOutgoingFrameV1.invalid("ordinary terminal W/S/C/key scope differs")
    }
    let floor: UInt32?
    if c.platform == 4 {
      guard alias == f[6].base64EncodedString(), f[10].count == 4, f[11] == Data([0]) else {
        throw KagemushaOrdinaryOutgoingFrameV1.invalid("ordinary terminal Apple key or floor differs")
      }
      floor = KagemushaAppPlatformPreparedProjectionV1.u32(f[10])
      guard floor! < UInt32.max else { throw KagemushaOrdinaryOutgoingFrameV1.invalid("Apple terminal counter exhausted") }
    } else {
      guard alias == c.androidKeyAlias, f[10].isEmpty, [Data([1]), Data([2])].contains(f[11]) else {
        throw KagemushaOrdinaryOutgoingFrameV1.invalid("ordinary terminal Android policy differs")
      }
      floor = nil
    }
    self.fields = f; approval = w; platform = c.platform; keyAlias = alias
    publicKey = f[5]; keyID = f[6]; appID = f[12]; counterFloor = floor
  }
}
