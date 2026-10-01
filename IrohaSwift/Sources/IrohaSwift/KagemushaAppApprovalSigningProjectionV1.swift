import CryptoKit
import Foundation

/// Structural read-only projection of native W. Decoding this value creates no owner,
/// prepared capability, signature permission, platform counter, or monetary authority.
/// The public platform signer receives only the genuine coordinator's opaque preparation.
struct KagemushaAppApprovalSigningProjectionV1: Sendable {
  static let signingDomain = Data("iroha:kagemusha:v1:app-operation-approval\0".utf8)
  static let bodyBytes = 275
  static let maximumLifetimeMS: UInt64 = 120_000

  let canonicalSigningBytes: Data
  let canonicalFinancialSubject: Data
  let operationID: Data
  let nonce: Data
  let accountBinding: Data
  let authorityPolicyDigest: Data
  let attestedKeyID: Data
  let enrollmentDigest: Data
  let subjectSigningDigest: Data
  let normalizedGuardDigest: Data
  let issuedAtMS: UInt64
  let expiresAtMS: UInt64

  /// Called only while validating a native preparation projection. This initializer is
  /// deliberately internal, and its result is not a platform-signing capability.
  init(nativeSigningBytes: Data, nativeFinancialSubject: Data) throws {
    let wrapper = Data(nativeSigningBytes)
    let start = Self.signingDomain.count + 8
    guard wrapper.count == start + Self.bodyBytes,
      wrapper.starts(with: Self.signingDomain),
      Self.u64(wrapper, at: Self.signingDomain.count) == UInt64(Self.bodyBytes),
      wrapper[start] == 1, wrapper[start + 1] == 0,
      wrapper[start + 2] == 1 else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid app approval W layout or purpose")
    }
    let subject = try KagemushaAppAttestTransitionBindingV1(
      coreSelectionSigningBytes: nativeFinancialSubject).canonicalSelectionSigningBytes
    guard (1...5).contains(subject[331]) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("Bootstrap is not an ordinary W financial subject")
    }
    let fields = (0..<8).map { index in
      Data(wrapper[(start + 3 + index * 32)..<(start + 3 + (index + 1) * 32)])
    }
    let issued = Self.u64(wrapper, at: start + 259)
    let expires = Self.u64(wrapper, at: start + 267)
    guard fields.allSatisfy({ $0.contains(where: { $0 != 0 }) }),
      fields[6] == Data(SHA256.hash(data: subject)), issued > 0, expires > issued,
      expires - issued <= Self.maximumLifetimeMS else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid native app approval subject or interval")
    }
    canonicalSigningBytes = wrapper
    canonicalFinancialSubject = subject
    operationID = fields[0]; nonce = fields[1]; accountBinding = fields[2]
    authorityPolicyDigest = fields[3]; attestedKeyID = fields[4]
    enrollmentDigest = fields[5]; subjectSigningDigest = fields[6]
    normalizedGuardDigest = fields[7]; issuedAtMS = issued; expiresAtMS = expires
  }

  /// App Attest's clientDataHash hashes exact W once. S and a prehashed W are distinct.
  var clientDataHash: Data { Data(SHA256.hash(data: canonicalSigningBytes)) }

  private static func u64(_ bytes: Data, at offset: Int) -> UInt64 {
    bytes[offset..<(offset + 8)].enumerated().reduce(UInt64(0)) {
      $0 | UInt64($1.element) << UInt64($1.offset * 8)
    }
  }
}

/// Original Apple equation for W; this projection is evidence, never a native approval.
/// A genuine owner must independently verify the enrolled credential, current W, counter
/// floor, original journal and its financial proof before it can consume these bytes.
struct KagemushaAppAttestApprovalOriginalV1: Sendable {
  let rawAssertion: Data
  let observedCounter: UInt32
  let clientDataHash: Data

  init(rawAssertion: Data, nativeProjection: KagemushaAppApprovalSigningProjectionV1,
    enrolledKeyID: Data, enrolledPublicKeyX963: Data, expectedAppIDHash: Data,
    expectedRelease: KagemushaAppAttestExpectedReleaseV1, nativeCounterFloor: UInt32) throws {
    guard rawAssertion.count <= 4_096,
      enrolledKeyID == nativeProjection.attestedKeyID,
      enrolledKeyID == Data(SHA256.hash(data: enrolledPublicKeyX963)) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("app approval enrolled key or original differs")
    }
    let evidence = try KagemushaAppAttestAssertionEvidenceV1(rawAssertion: rawAssertion,
      clientDataHash: nativeProjection.clientDataHash, expectedAppIDHash: expectedAppIDHash,
      expectedRelease: expectedRelease, enrolledAssertionPublicKeyX963: enrolledPublicKeyX963)
    guard evidence.authenticatorData.count == 37, evidence.authenticatorData[32] == 0x40,
      nativeCounterFloor < UInt32.max, evidence.signCount > nativeCounterFloor else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("app approval original counter or platform equation differs")
    }
    self.rawAssertion = Data(evidence.rawAssertion)
    observedCounter = evidence.signCount
    clientDataHash = nativeProjection.clientDataHash
  }
}
