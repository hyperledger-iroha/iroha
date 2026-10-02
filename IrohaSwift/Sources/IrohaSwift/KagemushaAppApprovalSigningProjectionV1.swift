import CryptoKit
import Foundation

/// Structural read-only projection of native W. Decoding this value creates no owner,
/// prepared capability, signature permission, platform counter, or monetary authority.
/// The public platform signer receives only the genuine coordinator's opaque preparation.
struct KagemushaAppApprovalSigningProjectionV1: Sendable {
  static let signingDomain = KagemushaAppOperationApprovalWrapperV1.signingDomain
  static let bodyBytes = KagemushaAppOperationApprovalWrapperV1.bodyBytes
  static let maximumLifetimeMS = KagemushaAppOperationApprovalWrapperV1.maximumLifetimeMS
  let wrapper: KagemushaAppOperationApprovalWrapperV1
  var canonicalSigningBytes: Data { wrapper.canonicalSigningBytes }
  var canonicalFinancialSubject: Data { wrapper.canonicalFinancialSubject }
  var operationID: Data { wrapper.operationID }
  var nonce: Data { wrapper.nonce }
  var accountBinding: Data { wrapper.accountBinding }
  var authorityPolicyDigest: Data { wrapper.authorityPolicyDigest }
  var attestedKeyID: Data { wrapper.attestedKeyID }
  var enrollmentDigest: Data { wrapper.enrollmentDigest }
  var subjectSigningDigest: Data { wrapper.subjectSigningDigest }
  var normalizedGuardDigest: Data { wrapper.normalizedGuardDigest }
  var clientDataHash: Data { wrapper.clientDataHash }
  var issuedAtMS: UInt64 { wrapper.issuedAtMS }
  var expiresAtMS: UInt64 { wrapper.expiresAtMS }

  /// Purpose2 ordinary preparation is separate from the terminal layout below.
  init(nativePreparationSigningBytes: Data, nativeFinancialSubject: Data) throws {
    wrapper = try KagemushaAppOperationApprovalWrapperV1(nativePreparationSigningBytes: nativePreparationSigningBytes,
      nativeFinancialSubject: nativeFinancialSubject)
  }

  init(nativeSigningBytes: Data, nativeFinancialSubject: Data) throws {
    let parsed = try KagemushaAppOperationApprovalWrapperV1(nativeSigningBytes: nativeSigningBytes,
      nativeFinancialSubject: nativeFinancialSubject)
    guard (1...5).contains(parsed.canonicalFinancialSubject[331]) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("Bootstrap is not an ordinary W financial subject")
    }
    wrapper = parsed
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
    // The shared verifier authenticates the complete release measurement suffix;
    // the original bytes remain unchanged and Native independently checks policy.
    guard (37...206).contains(evidence.authenticatorData.count),
      [UInt8(0x40), 0xc0].contains(evidence.authenticatorData[32]),
      nativeCounterFloor < UInt32.max, evidence.signCount > nativeCounterFloor else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("app approval original counter or platform equation differs")
    }
    self.rawAssertion = Data(evidence.rawAssertion)
    observedCounter = evidence.signCount
    clientDataHash = nativeProjection.clientDataHash
  }
}
