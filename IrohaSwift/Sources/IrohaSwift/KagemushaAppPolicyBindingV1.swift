import CryptoKit
import Foundation

/// Stable app/release/device/lane approval bound into a governed credential.
///
/// This digest does not verify an app or authorize a transition. The release and
/// authority policy values must be selected independently of device evidence.
public struct KagemushaAppDevicePolicyBindingV1: Equatable, Sendable {
  private static let domain = Data("iroha:kagemusha:v1:app-device-static-binding\0".utf8)

  public let appSigningIdentityDigest: Data
  public let appReleaseDigest: Data
  public let releaseID: Data
  public let hardwareProfileID: Data
  public let deviceKeyReference: Data
  public let laneID: Data

  public init(
    appSigningIdentityDigest: Data, appReleaseDigest: Data, releaseID: Data,
    hardwareProfileID: Data, deviceKeyReference: Data, laneID: Data
  ) throws {
    self.appSigningIdentityDigest = try kagemushaDigest(
      appSigningIdentityDigest, "appSigningIdentityDigest")
    self.appReleaseDigest = try kagemushaDigest(appReleaseDigest, "appReleaseDigest")
    self.releaseID = try kagemushaDigest(releaseID, "releaseID")
    self.hardwareProfileID = try kagemushaDigest(hardwareProfileID, "hardwareProfileID")
    self.deviceKeyReference = try kagemushaDigest(deviceKeyReference, "deviceKeyReference")
    self.laneID = try kagemushaDigest(laneID, "laneID")
  }

  /// Exact Rust V1 SHA-256 preimage: domain followed by six raw 32-byte values.
  public var canonicalDigest: Data {
    var preimage = Self.domain
    preimage.append(appSigningIdentityDigest)
    preimage.append(appReleaseDigest)
    preimage.append(releaseID)
    preimage.append(hardwareProfileID)
    preimage.append(deviceKeyReference)
    preimage.append(laneID)
    return Data(SHA256.hash(data: preimage))
  }
}

extension KagemushaHardwareCredentialV1 {
  /// Compare a governed credential with an independently selected release and
  /// app policy. Native Core remains responsible for signature and proof checks.
  public func validateAppPolicyBindingShapeForRelease(
    profile: KagemushaHardwareProfileV1, releaseID: Data,
    governedAuthorityPolicyDigest: Data,
    expectedPlatformClass: KagemushaHardwarePlatformClassV1,
    appSigningIdentityDigest: Data, appReleaseDigest: Data
  ) throws {
    guard profile.platformClass == expectedPlatformClass,
      profile.hardwareProfileID == hardwareProfileID,
      profile.firmwarePolicyDigest == firmwarePolicyDigest,
      profile.policyEpoch == policyEpoch,
      profile.appAttestationAuthorityPolicyDigest == governedAuthorityPolicyDigest
    else { throw kagemushaInvalid("appPolicy.profile") }
    let binding = try KagemushaAppDevicePolicyBindingV1(
      appSigningIdentityDigest: appSigningIdentityDigest,
      appReleaseDigest: appReleaseDigest, releaseID: releaseID,
      hardwareProfileID: hardwareProfileID, deviceKeyReference: deviceKeyReference,
      laneID: laneCommitment)
    guard appPolicyBindingDigest == binding.canonicalDigest else {
      throw kagemushaInvalid("appPolicy.credentialBinding")
    }
  }
}
