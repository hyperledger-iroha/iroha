import CryptoKit
import Foundation

/// Process-local original native E capability. No byte/DTO initializer is public.
/// The pending raw-attestation authority precedes final credential issuance; this
/// possession receipt cannot approve W, S, or a monetary transition.
public final class KagemushaNativePreparedAppEnrollmentPossessionV1: @unchecked Sendable {
  private let bridge: KagemushaCoreCoordinatorBridgeV1
  private let original: KagemushaAppPlatformPreparedProjectionV1
  private let originalID: Data

  private init(bridge: KagemushaCoreCoordinatorBridgeV1,
    original: KagemushaAppPlatformPreparedProjectionV1, originalID: Data) {
    self.bridge = bridge; self.original = original; self.originalID = originalID
  }

  static func prepare(bridge: KagemushaCoreCoordinatorBridgeV1, id: Data) throws
    -> KagemushaNativePreparedAppEnrollmentPossessionV1 {
    let fields = try bridge.invoke(.appEnrollmentPossession,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(1), id])
    let projection = try KagemushaAppPlatformPreparedProjectionV1(
      nativeFields: fields, approvalID: nil, enrollmentChallengeHash: id)
    let result = KagemushaNativePreparedAppEnrollmentPossessionV1(bridge: bridge, original: projection, originalID: id)
    _ = try result.recheck()
    return result
  }

  /// Read exact E only after the original native owner has rechecked scope and time.
  /// These bytes are correlation data and cannot recreate this capability.
  public func signingBytes() throws -> Data { try recheck().signingBytes }

  /// Cancel this original ticket. Native Core rejects cancellation after invocation.
  public func cancel() throws { _ = try call(7) }

  func recheck() throws -> KagemushaAppPlatformPreparedProjectionV1 {
    let checked = try call(6)
    guard checked[0] == original.nativeScope,
      checked[1] == Data(SHA256.hash(data: original.signingBytes)) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("current owner substituted enrollment possession scope")
    }
    return original
  }

  /// Durably fence before device invocation. Native rejects an invoked attempt whose
  /// original evidence is unknown; it never returns that attempt as freshly uninvoked.
  func fence() throws -> (state: UInt8, raw: Data, receipt: Data) {
    _ = try recheck()
    let fields = try call(2)
    return (fields[0][0], fields[1], fields[2])
  }

  func recover() throws -> (state: UInt8, raw: Data, receipt: Data) {
    _ = try recheck()
    let fields = try call(5)
    return (fields[0][0], fields[1], fields[2])
  }

  func retainOriginal(_ raw: Data) throws {
    // Native phase3 admits only the original fenced ticket and persists raw evidence.
    // A rejected/uncertain retention remains frozen; it never causes another assertion.
    _ = try call(3, raw: raw)
    _ = try recheck()
  }

  func consume(original evidence: KagemushaAppAttestEnrollmentPossessionOriginalV1) throws
    -> KagemushaNativeAppEnrollmentPossessionReceiptV1 {
    _ = try recheck()
    let fields = try call(4)
    return try receipt(fields[0], evidence: evidence)
  }

  func recoveredReceipt(_ bytes: Data, evidence: KagemushaAppAttestEnrollmentPossessionOriginalV1) throws
    -> KagemushaNativeAppEnrollmentPossessionReceiptV1 {
    _ = try recheck()
    return try receipt(bytes, evidence: evidence)
  }

  private func receipt(_ bytes: Data, evidence: KagemushaAppAttestEnrollmentPossessionOriginalV1) throws
    -> KagemushaNativeAppEnrollmentPossessionReceiptV1 {
    let r = try KagemushaAppPlatformReceiptProjectionV1(bytes)
    guard original.approval == nil, original.credentialDigest.isEmpty,
      r.purpose == 2, r.ticket == original.ticket,
      r.originalID == originalID, r.nativeScope == original.nativeScope,
      r.challengeDigest == Data(SHA256.hash(data: original.signingBytes)),
      r.rawEvidenceDigest == Data(SHA256.hash(data: evidence.rawAssertion)),
      r.originalScopeDigest == original.nativeScope,
      r.appleCounter == evidence.observedCounter else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("native receipt differs from original Apple E")
    }
    return KagemushaNativeAppEnrollmentPossessionReceiptV1(original: original, receipt: r)
  }

  private func call(_ phase: UInt32, raw: Data? = nil) throws -> [Data] {
    var fields = [KagemushaCoreCoordinatorFrameV1.u32(phase), original.ticket]
    if let raw { fields.append(raw) }
    return try bridge.invoke(.appEnrollmentPossession, fields: fields)
  }
}

/// Exact native-consumed original enrollment possession, never an app credential
/// or monetary authority. Final issuance remains the separately authenticated issuer.
/// It is created only by the genuine bridge's purpose-bound native consume/recovery.
public struct KagemushaNativeAppEnrollmentPossessionReceiptV1: Sendable {
  public let enrollmentChallengeHash: Data
  public let keyID: Data
  public let keyAlias: String
  public let signingDigest: Data
  public let rawAssertionDigest: Data
  public let observedCounter: UInt32
  public let canonicalReceipt: Data

  fileprivate init(original: KagemushaAppPlatformPreparedProjectionV1,
    receipt: KagemushaAppPlatformReceiptProjectionV1) {
    enrollmentChallengeHash = receipt.originalID; keyID = original.keyID; keyAlias = original.keyAlias
    signingDigest = receipt.challengeDigest; rawAssertionDigest = receipt.rawEvidenceDigest
    observedCounter = receipt.appleCounter!; canonicalReceipt = receipt.canonicalBytes
  }
}

extension KagemushaCoreCoordinatorBridgeV1 {
  /// Select the held pending original by SHA(full C), never its stable attempt ID.
  /// No caller constructs E, changes C or supplies pending authority.
  /// Missing signed raw-attestation admission, original owner or scope remains unavailable.
  public func prepareAppEnrollmentPossession(originalEnrollmentChallengeHash: Data) throws
    -> KagemushaNativePreparedAppEnrollmentPossessionV1 {
    try KagemushaNativePreparedAppEnrollmentPossessionV1.prepare(bridge: self, id: originalEnrollmentChallengeHash)
  }
}
