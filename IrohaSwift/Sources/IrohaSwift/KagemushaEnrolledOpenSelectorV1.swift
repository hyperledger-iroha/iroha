import Foundation

/// Untrusted immutable institution, authentication and ledger scope used for owner lookup.
///
/// These values grant no MiBank approval, enrollment, hardware custody or monetary authority.
/// A native backend must derive its authenticated runtime independently and compare every field.
public struct KagemushaRetailEnrollmentRuntimeProjectionV1: Equatable, Sendable {
  public let fiID: String
  public let ledgerDataspaceID: UInt64
  public let authenticationNamespace: String
  public let networkID: Data
  public let asset: KagemushaAssetDefinitionIDV1
  public let assetIncarnation: KagemushaAssetIncarnationV1
  public let scale: UInt32

  public init(
    fiID: String, ledgerDataspaceID: UInt64, authenticationNamespace: String,
    networkID: Data, asset: KagemushaAssetDefinitionIDV1,
    assetIncarnation: KagemushaAssetIncarnationV1, scale: UInt32
  ) throws {
    try enrolledSelectorName(fiID)
    try enrolledSelectorName(authenticationNamespace)
    try kagemushaHeader(1, networkID, scale)
    guard networkID.last.map({ $0 & 1 == 1 }) == true else {
      throw kagemushaInvalid("enrolledSelector.networkID")
    }
    self.fiID = fiID
    self.ledgerDataspaceID = ledgerDataspaceID
    self.authenticationNamespace = authenticationNamespace
    self.networkID = Data(Array(networkID))
    self.asset = asset
    self.assetIncarnation = try KagemushaAssetIncarnationV1(
      bytes: Data(Array(assetIncarnation.bytes)))
    self.scale = scale
  }
}

/// Untrusted account and lane correlation, with no storage path or authorization flags.
public struct KagemushaRetailEnrollmentOwnerProjectionV1: Equatable, Sendable {
  public let accountID: KagemushaAccountIDV1
  public let runtime: KagemushaRetailEnrollmentRuntimeProjectionV1
  public let laneID: Data

  public init(
    accountID: KagemushaAccountIDV1,
    runtime: KagemushaRetailEnrollmentRuntimeProjectionV1, laneID: Data
  ) throws {
    let account = try KagemushaAccountIDV1(
      canonicalPayload: Data(Array(accountID.canonicalPayload)))
    var controller = CanonicalNoritoReader(data: account.canonicalPayload)
    guard try controller.readUInt32LE() == 0 else {
      throw kagemushaInvalid("enrolledSelector.accountController")
    }
    var key = CanonicalNoritoReader(data: try controller.readCompactField())
    guard controller.remaining() == 0, try key.readUInt64LE() == 33,
      try key.readCompactField() == Data([0])
    else { throw kagemushaInvalid("enrolledSelector.accountAlgorithm") }
    // KagemushaAccountIDV1 already validates the remaining Ed25519 key encoding
    // and key admission. This check selects its sole supported controller/algorithm.
    self.accountID = account
    self.runtime = runtime
    self.laneID = Data(Array(try kagemushaDigest(laneID, "enrolledSelector.laneID")))
  }

  /// Derive the exact domain-separated full-owner identity; this is not evidence of enrollment.
  public func enrollmentID() throws -> Data {
    try KagemushaNoritoV1.retailEnrollmentIdentityShape(self)
  }
}

/// Sole canonical untrusted projection for a native enrolled-owner lookup.
///
/// The digest commits to the complete owner, but proves neither MiBank approval nor
/// possession. The native lifecycle must authenticate both independently before opening.
public struct KagemushaEnrolledOpenSelectorV1: Equatable, Sendable {
  public static let maximumCanonicalBytes = 16 * 1024
  public let version: UInt16
  public let owner: KagemushaRetailEnrollmentOwnerProjectionV1
  public let enrollmentID: Data

  public init(owner: KagemushaRetailEnrollmentOwnerProjectionV1) throws {
    try self.init(version: 1, owner: owner, enrollmentID: owner.enrollmentID())
  }

  /// Validate an explicit decoded identity without granting it native authority.
  public init(
    version: UInt16, owner: KagemushaRetailEnrollmentOwnerProjectionV1, enrollmentID: Data
  ) throws {
    guard version == 1 else { throw kagemushaInvalid("enrolledSelector.version") }
    guard try enrollmentID == owner.enrollmentID() else {
      throw kagemushaInvalid("enrolledSelector.enrollmentID")
    }
    self.version = version
    self.owner = owner
    self.enrollmentID = Data(Array(enrollmentID))
  }

  public func canonicalBytes() throws -> Data {
    try KagemushaNoritoV1.encodeEnrolledOpenSelectorShape(self)
  }

  public static func decodeCanonicalExact(_ bytes: Data) throws -> Self {
    try KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(bytes)
  }
}

private func enrolledSelectorName(_ value: String) throws {
  // Rust Name rejects non-NFC input instead of normalizing it. Swift String equality
  // uses canonical equivalence, so equality here must compare the actual UTF-8 bytes.
  guard !value.isEmpty, value.utf8.count <= 255,
    value.utf8.elementsEqual(value.precomposedStringWithCanonicalMapping.utf8),
    value.unicodeScalars.allSatisfy({ scalar in
      let code = scalar.value
      let control = code <= 0x1f || (0x7f...0x9f).contains(code)
      let whitespace = (0x2000...0x200a).contains(code)
        || [0x20, 0xa0, 0x1680, 0x2028, 0x2029, 0x202f, 0x205f, 0x3000].contains(code)
      let bidi = [0x061c, 0x200e, 0x200f].contains(code)
        || (0x202a...0x202e).contains(code) || (0x2066...0x2069).contains(code)
      return !control && !whitespace && !bidi && ![0x40, 0x23, 0x24].contains(code)
    })
  else { throw kagemushaInvalid("enrolledSelector.name") }
}
