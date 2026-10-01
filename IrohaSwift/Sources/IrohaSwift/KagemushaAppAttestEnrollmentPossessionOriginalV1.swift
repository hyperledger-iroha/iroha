import CryptoKit
import Foundation

/// Local Apple equation for native E; not a pending admission, final credential,
/// W approval or monetary capability. Core independently verifies its original.
struct KagemushaAppAttestEnrollmentPossessionOriginalV1: Sendable {
  let rawAssertion: Data
  let observedCounter: UInt32
  let clientDataHash: Data
  /// Actual signed extensions, or explicit unavailable measurement for legacy37.
  /// This local equation does not admit a release policy or native pending owner.
  let releaseMeasurement: KagemushaAppAttestReleaseMeasurementV1

  init(rawAssertion: Data, nativeProjection p: KagemushaAppPlatformPreparedProjectionV1,
    expectedRelease: KagemushaAppAttestExpectedReleaseV1) throws {
    guard p.platform == 4, p.approval == nil, p.credentialDigest.isEmpty,
      p.financialSubject.isEmpty, let floor = p.appleCounterFloor,
      floor < UInt32.max, rawAssertion.count <= 311,
      p.keyID == Data(SHA256.hash(data: p.publicKeyX963)) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("Apple possession requires original pending E")
    }
    let hash = Data(SHA256.hash(data: p.signingBytes))
    let evidence = try KagemushaAppAttestAssertionEvidenceV1(rawAssertion: rawAssertion,
      clientDataHash: hash, expectedAppIDHash: p.appSigningIdentityDigest,
      expectedRelease: expectedRelease, enrolledAssertionPublicKeyX963: p.publicKeyX963)
    // The shared verifier authenticates the complete original extensions against
    // expectedRelease. Native Core separately checks its governed release digest.
    // Legacy37/0x40 remains explicitly unavailable; measured suffixes admit the
    // existing shared policy's 0x40 or 0xc0 flags without normalizing any bytes.
    guard (37...206).contains(evidence.authenticatorData.count),
      [UInt8(0x40), 0xc0].contains(evidence.authenticatorData[32]),
      evidence.signCount > floor else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("Apple E original counter or equation differs")
    }
    self.rawAssertion = Data(evidence.rawAssertion)
    observedCounter = evidence.signCount; clientDataHash = hash
    releaseMeasurement = evidence.releaseMeasurement
  }
}
