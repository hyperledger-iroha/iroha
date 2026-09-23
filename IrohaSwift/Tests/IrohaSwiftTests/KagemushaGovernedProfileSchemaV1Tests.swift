import Foundation
import XCTest
@testable import IrohaSwift

/// First-release profile wire shape and app-class admission boundaries.
final class KagemushaGovernedProfileSchemaV1Tests: XCTestCase {
  func testPlatformTagsAndExactGuaranteeMasks() {
    let expected: [(KagemushaHardwarePlatformClassV1, UInt32, UInt32, Bool)] = [
      (.androidOEMService, 0, 0x0000_ffff, false),
      (.appleOEMService, 1, 0x0000_ffff, false),
      (.dedicatedSecureElement, 2, 0x0000_ffff, false),
      (.otherQualified, 3, 0x0000_ffff, false),
      (.appleAppAttest, 4, 0x0007_0000, true),
      (.androidKeyMint, 5, 0x000b_0000, true),
    ]
    for (platform, tag, mask, ordinaryApp) in expected {
      XCTAssertEqual(platform.rawValue, tag)
      XCTAssertEqual(platform.requiredCapabilityMask, mask)
      XCTAssertEqual(platform.isOrdinaryApp, ordinaryApp)
    }
    XCTAssertNil(KagemushaHardwarePlatformClassV1(rawValue: 6))
  }

  func testProfileAndCredentialNewFieldsRoundTripExactly() throws {
    for platform in KagemushaHardwarePlatformClassV1.allCases {
      let profile = try makeProfile(platform: platform)
      let profileWire = try KagemushaNoritoV1.encodeHardwareProfileShape(profile)
      XCTAssertEqual(try KagemushaNoritoV1.decodeHardwareProfileShapeExact(profileWire), profile)
      XCTAssertEqual(profile.capabilityMask, platform.requiredCapabilityMask)
      XCTAssertEqual(profile.appAttestationAuthorityPolicyDigest, digest(16))
      XCTAssertThrowsError(try KagemushaNoritoV1.decodeHardwareProfileShapeExact(
        Data(profileWire.dropLast())))
    }
    let credential = try makeCredential()
    let credentialWire = try KagemushaNoritoV1.encodeHardwareCredentialShape(credential)
    XCTAssertEqual(try KagemushaNoritoV1.decodeHardwareCredentialShapeExact(credentialWire), credential)
    XCTAssertEqual(credential.appPolicyBindingDigest, digest(28))
    XCTAssertThrowsError(try KagemushaNoritoV1.decodeHardwareCredentialShapeExact(
      Data(credentialWire.dropLast())))
  }

  func testWrongCapabilityMaskAndMissingPolicyDigestAreRejected() throws {
    for platform in KagemushaHardwarePlatformClassV1.allCases {
      XCTAssertThrowsError(try makeProfile(
        platform: platform, mask: platform.requiredCapabilityMask ^ 1))
    }
    XCTAssertThrowsError(try makeProfile(
      platform: .appleAppAttest, authorityDigest: Data(repeating: 0, count: 32)))
    XCTAssertThrowsError(try makeCredential(
      bindingDigest: Data(repeating: 0, count: 32)))
  }

  func testOrdinaryAppCannotEnterMonetaryQualificationBeforeProofFold() throws {
    let credential = try makeCredential()
    for platform in [KagemushaHardwarePlatformClassV1.appleAppAttest, .androidKeyMint] {
      let profile = try makeProfile(platform: platform)
      XCTAssertThrowsError(try KagemushaHardwareQualificationV1(
        releaseID: digest(30), hardwarePolicyDigest: digest(31),
        coreAuthorizationKeyReference: digest(32), profile: profile,
        credential: credential)) { error in
          XCTAssertEqual(error as? KagemushaWalletErrorV1, .nativeVerificationRequired)
        }
    }
    let oem = try makeProfile(platform: .appleOEMService)
    XCTAssertNoThrow(try KagemushaHardwareQualificationV1(
      releaseID: digest(30), hardwarePolicyDigest: digest(31),
      coreAuthorizationKeyReference: digest(32), profile: oem,
      credential: credential))
  }

  func testAppDeviceBindingMatchesRustVectorAndSelectedPolicy() throws {
    let vector = try KagemushaAppDevicePolicyBindingV1(
      appSigningIdentityDigest: digest(1), appReleaseDigest: digest(2),
      releaseID: digest(3), hardwareProfileID: digest(4),
      deviceKeyReference: digest(5), laneID: digest(6))
    XCTAssertEqual(vector.canonicalDigest.map { String(format: "%02x", $0) }.joined(),
      "d57622e75f9a50b61596d18accaeffe2e0f64531b6e7a57ec7cfde1c1b352243")

    let profile = try makeProfile(platform: .appleAppAttest)
    let selectedReleaseID = digest(30)
    let selectedIdentity = digest(31)
    let selectedAppRelease = digest(32)
    let binding = try KagemushaAppDevicePolicyBindingV1(
      appSigningIdentityDigest: selectedIdentity, appReleaseDigest: selectedAppRelease,
      releaseID: selectedReleaseID, hardwareProfileID: profile.hardwareProfileID,
      deviceKeyReference: digest(25), laneID: digest(23))
    let credential = try makeCredential(bindingDigest: binding.canonicalDigest)
    XCTAssertNoThrow(try credential.validateAppPolicyBindingShapeForRelease(
      profile: profile, releaseID: selectedReleaseID,
      governedAuthorityPolicyDigest: profile.appAttestationAuthorityPolicyDigest,
      expectedPlatformClass: .appleAppAttest,
      appSigningIdentityDigest: selectedIdentity, appReleaseDigest: selectedAppRelease))
    XCTAssertThrowsError(try credential.validateAppPolicyBindingShapeForRelease(
      profile: profile, releaseID: digest(40),
      governedAuthorityPolicyDigest: profile.appAttestationAuthorityPolicyDigest,
      expectedPlatformClass: .appleAppAttest,
      appSigningIdentityDigest: selectedIdentity, appReleaseDigest: selectedAppRelease))
    XCTAssertThrowsError(try credential.validateAppPolicyBindingShapeForRelease(
      profile: profile, releaseID: selectedReleaseID,
      governedAuthorityPolicyDigest: digest(41), expectedPlatformClass: .appleAppAttest,
      appSigningIdentityDigest: selectedIdentity, appReleaseDigest: selectedAppRelease))
    XCTAssertThrowsError(try credential.validateAppPolicyBindingShapeForRelease(
      profile: profile, releaseID: selectedReleaseID,
      governedAuthorityPolicyDigest: profile.appAttestationAuthorityPolicyDigest,
      expectedPlatformClass: .androidKeyMint,
      appSigningIdentityDigest: selectedIdentity, appReleaseDigest: selectedAppRelease))
  }

  private func makeProfile(
    platform: KagemushaHardwarePlatformClassV1,
    mask: UInt32? = nil,
    authorityDigest: Data? = nil
  ) throws -> KagemushaHardwareProfileV1 {
    try KagemushaHardwareProfileV1(
      hardwareProfileID: digest(1), providerID: digest(2), platformClass: platform,
      productClassDigest: digest(3), firmwarePolicyDigest: digest(4),
      enrollmentAttestationVerifierDigest: digest(5), attestationTrustRootsDigest: digest(6),
      allowedSuiteCommitment: digest(7), policyEpoch: 1,
      governanceCredentialPublicKey: key(),
      capabilityMask: mask ?? platform.requiredCapabilityMask,
      qualificationReportDigest: digest(8), validFromMS: 1, expiresAtMS: 100,
      appAttestationAuthorityPolicyDigest: authorityDigest ?? digest(16))
  }

  private func makeCredential(bindingDigest: Data? = nil) throws
    -> KagemushaHardwareCredentialV1
  {
    var signature = Data(repeating: 0, count: 64)
    signature[31] = 1
    signature[63] = 1
    return try KagemushaHardwareCredentialV1(
      credentialID: digest(20), networkID: digest(21), hardwareProfileID: digest(1),
      suiteID: digest(22), firmwarePolicyDigest: digest(4), policyEpoch: 1,
      laneCommitment: digest(23), hardwareEpochID: digest(24), hardwareEpochGeneration: 1,
      devicePublicKey: key(), deviceKeyReference: digest(25), issuedAtMS: 2,
      expiresAtMS: 99, appPolicyBindingDigest: bindingDigest ?? digest(28),
      governanceSignature: try KagemushaDeviceSignatureV1(rawBytes: signature))
  }

  private func digest(_ byte: UInt8) -> Data { Data(repeating: byte, count: 32) }

  private func key() throws -> KagemushaDevicePublicKeyV1 {
    try KagemushaDevicePublicKeyV1(sec1Bytes: Data([4]) + Data(repeating: 1, count: 64))
  }
}
