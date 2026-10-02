import CryptoKit
import Foundation

/// Data-only exact W/S correlation shared by distinct Bootstrap and monetary holders.
/// Parsing creates no capability, native capture or platform invocation permission.
struct KagemushaAppOperationApprovalWrapperV1: Sendable {
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
    try self.init(nativeSigningBytes: nativeSigningBytes, nativeFinancialSubject: nativeFinancialSubject, purpose: 1)
  }

  /// Distinct purpose2 preparation layout; no terminal/Bootstrap grammar is accepted here.
  init(nativePreparationSigningBytes: Data, nativeFinancialSubject: Data) throws {
    try self.init(nativeSigningBytes: nativePreparationSigningBytes, nativeFinancialSubject: nativeFinancialSubject, purpose: 2)
  }

  private init(nativeSigningBytes: Data, nativeFinancialSubject: Data, purpose: UInt8) throws {
    let wrapper = Data(nativeSigningBytes)
    let start = Self.signingDomain.count + 8
    guard wrapper.count == start + Self.bodyBytes,
      wrapper.starts(with: Self.signingDomain),
      Self.u64(wrapper, at: Self.signingDomain.count) == UInt64(Self.bodyBytes),
      wrapper[start] == 1, wrapper[start + 1] == 0,
      wrapper[start + 2] == purpose else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid app approval W layout or purpose")
    }
    let subject = try (purpose == 2
      ? KagemushaAppAttestTransitionBindingV1(corePreparationSigningBytes: nativeFinancialSubject)
      : KagemushaAppAttestTransitionBindingV1(coreSelectionSigningBytes: nativeFinancialSubject)).canonicalSelectionSigningBytes
    let fields = (0..<8).map { index in
      Data(wrapper[(start + 3 + index * 32)..<(start + 3 + (index + 1) * 32)])
    }
    let issued = Self.u64(wrapper, at: start + 259)
    let expires = Self.u64(wrapper, at: start + 267)
    guard fields.allSatisfy({ $0.contains(where: { $0 != 0 }) }),
      fields[6] == Data(SHA256.hash(data: subject)), issued > 0, expires > issued,
      expires - issued <= (purpose == 2 ? 10_000 : Self.maximumLifetimeMS) else {
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
