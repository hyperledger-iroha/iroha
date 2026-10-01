import CryptoKit
import Foundation

/// Bounded correlation data only. This decoder never reconstructs a native owner.
struct KagemushaOrdinaryAppIdentityPreparedProjectionV1: Sendable {
  let ticket, signedChallenge, signingBytes, generationChallenge, nativeScope: Data
  let platform, androidLevelsMask: UInt8
  let originalAlias: String
  let challenge: KagemushaOrdinaryAppEnrollmentProjectionV1

  init(_ fields: [Data]) throws {
    let f = fields.map { Data($0) }
    guard f.count == 8, f[0].count == 8,
      KagemushaAppPlatformPreparedProjectionV1.nonzero(f[0]), f[1].count == 515,
      f[4].count == 1, f[6].count == 1, Self.digest(f[7]) else { throw Self.invalid() }
    let c = try KagemushaOrdinaryAppEnrollmentProjectionV1(f[2])
    guard Data(f[1].prefix(451)) == Data(f[2].suffix(451)),
      KagemushaAppPlatformPreparedProjectionV1.nonzero(Data(f[1].suffix(64))),
      f[3] == Data(SHA256.hash(data: f[2])), c.platform == f[4][0] else { throw Self.invalid() }
    let alias: String
    if c.platform == 4 {
      guard f[5].isEmpty, f[6] == Data([0]) else { throw Self.invalid() }; alias = ""
    } else {
      guard [UInt8(1), 2, 3].contains(f[6][0]),
        let name = String(data: f[5], encoding: .utf8), name == c.androidKeyAlias else { throw Self.invalid() }
      alias = name
    }
    ticket=f[0]; signedChallenge=f[1]; signingBytes=f[2]; generationChallenge=f[3]
    nativeScope=f[7]; platform=c.platform; androidLevelsMask=f[6][0]; originalAlias=alias; challenge=c
  }
  func validateKeyReference(_ bytes: Data, point: Data? = nil) throws -> String {
    guard (1...255).contains(bytes.count), !bytes.contains(0),
      let name = String(data: bytes, encoding: .utf8) else { throw Self.invalid() }
    if platform == 4 {
      guard let keyID = Data(base64Encoded: name), Self.digest(keyID),
        keyID.base64EncodedString() == name,
        point == nil || keyID == Data(SHA256.hash(data: point!)) else { throw Self.invalid() }
    } else { guard name == originalAlias else { throw Self.invalid() } }
    return name
  }
  /// Structural transport projection only; native phase 13 authenticates the issuer signature.
  static func challenge(transport: Data) throws -> KagemushaOrdinaryAppEnrollmentProjectionV1 {
    guard transport.count == 515,
      KagemushaAppPlatformPreparedProjectionV1.nonzero(Data(transport.suffix(64))) else { throw invalid() }
    let domain = Data("iroha:kagemusha:v1:ordinary-app-enrollment-challenge\0".utf8)
    let size = withUnsafeBytes(of: UInt64(451).littleEndian) { Data($0) }
    return try KagemushaOrdinaryAppEnrollmentProjectionV1(domain + size + Data(transport.prefix(451)))
  }
  static func digest(_ bytes: Data) -> Bool { KagemushaAppPlatformPreparedProjectionV1.digest(bytes) }
  static func point(_ bytes: Data) -> Bool {
    bytes.count == 65 && bytes.first == 4 && (try? P256.Signing.PublicKey(x963Representation: bytes)) != nil
  }
  static func invalid() -> KagemushaCoreCoordinatorErrorV1 { .invalidFrame("ordinary identity original projection differs") }
}

/// One exact bounded C21 recovery projection, not a recovered capability or verdict.
struct KagemushaOrdinaryAppIdentityRecoveryProjectionV1: Sendable {
  let state: UInt8
  let keyReference: String
  let point, rawDigest, rawAdmission, pendingScope: Data
  let rawLength: Int
  init(_ f: [Data], original: KagemushaOrdinaryAppIdentityPreparedProjectionV1) throws {
    guard f.count == 7, f[0].count == 1, f[0][0] <= 5, f[4].count == 4 else {
      throw KagemushaOrdinaryAppIdentityPreparedProjectionV1.invalid()
    }
    state=f[0][0]; rawLength=Int(KagemushaAppPlatformPreparedProjectionV1.u32(f[4]))
    if state < 2 { guard f[1].isEmpty else { throw KagemushaOrdinaryAppIdentityPreparedProjectionV1.invalid() }; keyReference="" }
    else { keyReference=try original.validateKeyReference(f[1], point:state>=4 ? f[2] : nil) }
    if state >= 4 {
      guard KagemushaOrdinaryAppIdentityPreparedProjectionV1.point(f[2]),
        KagemushaOrdinaryAppIdentityPreparedProjectionV1.digest(f[3]), (1...131_072).contains(rawLength) else {
        throw KagemushaOrdinaryAppIdentityPreparedProjectionV1.invalid()
      }
    } else { guard f[2].isEmpty, f[3].isEmpty, rawLength == 0 else { throw KagemushaOrdinaryAppIdentityPreparedProjectionV1.invalid() } }
    if state == 5 {
      guard f[5].count == 314, KagemushaOrdinaryAppIdentityPreparedProjectionV1.digest(f[6]) else {
        throw KagemushaOrdinaryAppIdentityPreparedProjectionV1.invalid()
      }
    } else { guard f[5].isEmpty, f[6].isEmpty else { throw KagemushaOrdinaryAppIdentityPreparedProjectionV1.invalid() } }
    point=f[2]; rawDigest=f[3]; rawAdmission=f[5]; pendingScope=f[6]
  }
}

/// Structural join to the source-only native raw-admission readback. Its Ed
/// signature is authenticated by native phase6, never by this projection decoder.
struct KagemushaOrdinaryAppIdentityRawProjectionV1 {
  let transport:Data
  init(_ a:Data,original:KagemushaOrdinaryAppIdentityPreparedProjectionV1,
    retained:KagemushaOrdinaryAppIdentityRecoveryProjectionV1) throws {
    let c=original.challenge
    guard a.count == 314,a[0] == 1,a[1] == 0,a[2] == 1,
      Data(a[3..<35]) == original.generationChallenge,
      Data(a[35..<67]) == c.appAuthorityPolicyDigest,
      a[67] == (c.platform == 4 ? 2 : 1),
      c.platform == 4 ? a[68] == 3 : [UInt8(1),2].contains(a[68]),
      Data(a[69..<134]) == retained.point,
      Data(a[134..<166]) == Data(SHA256.hash(data:retained.point)),
      Data(a[166..<198]) == retained.rawDigest,
      KagemushaOrdinaryAppIdentityPreparedProjectionV1.digest(Data(a[198..<230])),
      Data(a[230..<234]) == Data(repeating:0,count:4),
      KagemushaAppPlatformPreparedProjectionV1.u64(a,234) == c.issuedAtMS,
      KagemushaAppPlatformPreparedProjectionV1.u64(a,242) == c.expiresAtMS,
      KagemushaAppPlatformPreparedProjectionV1.nonzero(Data(a[250..<314])) else {
      throw KagemushaOrdinaryAppIdentityPreparedProjectionV1.invalid()
    }
    transport=a
  }
  func pendingScope(nativeScope:Data) -> Data {
    Data(SHA256.hash(data:Data("iroha:kagemusha:v1:pending-raw-app-identity-scope\0".utf8)
      + nativeScope + withUnsafeBytes(of:UInt64(314).littleEndian){Data($0)} + transport))
  }
}

/// The actual called provider decision for recovered device outcomes. Unknown
/// invocations never become a generation or attestation retry permission.
enum KagemushaOrdinaryAppIdentityRecoveryActionV1: Equatable {
  case generate, attest, complete
  static func classify(_ state:UInt8) throws -> Self {
    switch state {
    case 0:return .generate
    case 2:return .attest
    case 4,5:return .complete
    case 1,3:throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown
    default:throw KagemushaOrdinaryAppIdentityPreparedProjectionV1.invalid()
    }
  }
}

/// Exact native reservation carrier. Decoding it creates neither a native owner nor authority.
struct KagemushaOrdinaryAppIdentityReservationProjectionV1: Sendable {
  let fields: [Data]
  var ticket: Data { fields[0] }
  init(_ offered: [Data]) throws {
    let f = offered.map { Data($0) }
    guard f.count == 8, f[0].count == 8, f[0].contains(where: { $0 != 0 }),
      (1...2048).contains(f[1].count), !f[1].contains(0),
      String(data: f[1], encoding: .utf8) != nil,
      (2...6).allSatisfy({ KagemushaOrdinaryAppIdentityPreparedProjectionV1.digest(f[$0]) }),
      f[7] == Self.requestID(nonce: f[2]) else {
      throw KagemushaOrdinaryAppIdentityPreparedProjectionV1.invalid()
    }
    fields = f
  }
  /// Correlation only. Native admission independently verifies account custody and the signed original.
  func matches(_ challenge: KagemushaOrdinaryAppEnrollmentProjectionV1) -> Bool {
    let body = Data(challenge.canonicalSigningBytes.suffix(451))
    return fields[2] == challenge.clientNonce && fields[3] == challenge.releaseID
      && fields[4] == challenge.profileID && fields[5] == challenge.laneID
      && fields[6] == Data(body[(3 + 11 * 32)..<(35 + 11 * 32)])
  }
  private static func requestID(nonce: Data) -> Data {
    var bytes = Array(nonce.prefix(16)); bytes[6] = (bytes[6] & 0x0f) | 0x40
    bytes[8] = (bytes[8] & 0x3f) | 0x80
    let digits = Array("0123456789abcdef".utf8)
    let hex = bytes.flatMap { [digits[Int($0 >> 4)], digits[Int($0 & 15)]] }
    let text = [0..<8, 8..<12, 12..<16, 16..<20, 20..<32]
      .map { String(decoding: hex[$0], as: UTF8.self) }.joined(separator: "-")
    return Data(text.utf8)
  }
}
