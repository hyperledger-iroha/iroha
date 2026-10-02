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

  /// Correlate a retained signed preparation with the original native E owner.
  /// This is SHA256 of C's canonical domain, length and body, excluding its signature.
  /// Current native scope and policy must still pass; this value conveys no authority.
  public func originalEnrollmentChallengeHash() throws -> Data {
    _ = try recheck()
    return Data(originalID)
  }

  /// Read the exact native-consumed Apple E assertion for final issuer transport.
  /// This never invokes the device or infers completion from a local assertion journal.
  /// The current original owner and two matching native readbacks must retain the same
  /// purpose-bound receipt; unavailable or substituted evidence revokes this bridge.
  public func recoverOriginalConsumedAssertion() throws
    -> KagemushaNativeConsumedAppEnrollmentPossessionOriginalV1 {
    do {
      let p = try recheck()
      guard p.platform == 4, let floor = p.appleCounterFloor,
        p.approval == nil, p.credentialDigest.isEmpty else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("consumed Apple enrollment original unavailable")
      }
      let retained = try recover()
      guard retained.state == 2, (1...4096).contains(retained.raw.count) else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("enrollment possession is not consumed")
      }
      let receipt = try KagemushaAppPlatformReceiptProjectionV1(retained.receipt)
      guard receipt.purpose == 2, receipt.ticket == p.ticket,
        receipt.originalID == originalID, receipt.nativeScope == p.nativeScope,
        receipt.challengeDigest == Data(SHA256.hash(data: p.signingBytes)),
        receipt.rawEvidenceDigest == Data(SHA256.hash(data: retained.raw)),
        receipt.originalScopeDigest == p.nativeScope,
        let counter = receipt.appleCounter, counter > floor else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("consumed enrollment original differs")
      }
      let checked = try recover()
      guard checked.state == 2, checked.raw == retained.raw, checked.receipt == retained.receipt else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("consumed enrollment readback changed")
      }
      _ = try recheck()
      return KagemushaNativeConsumedAppEnrollmentPossessionOriginalV1(rawAssertion: retained.raw,
        receipt: KagemushaNativeAppEnrollmentPossessionReceiptV1(original: p, receipt: receipt))
    } catch {
      try? bridge.close()
      throw error
    }
  }

  /// Return an original issuer-signed canonical credential archive to the same native
  /// pending owner. Native Core authenticates, correlates and durably retains it.
  /// A completed E may recover the identical previously issued credential after C's
  /// short interval; current native policy, credential and Integrity still must pass.
  /// This method makes no issuer request, device assertion or financial-ready result.
  public func acceptOriginalFinalCredential(_ canonicalSignedCredential: Data) throws
    -> KagemushaNativeOrdinaryAppIdentityConfirmationV1 {
    // Reject before dispatch and retain a defensive copy of the untrusted original.
    // Swift does not decode this archive into authority or select its issuer/policy.
    guard (1...16384).contains(canonicalSignedCredential.count) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid final app identity original size")
    }
    let originalCredential = Data(canonicalSignedCredential)
    _ = try recheck()
    let fields = try call(8, raw: originalCredential)
    let accepted = try KagemushaAppEnrollmentFinalIdentityProjectionV1(
      nativeFields: fields, originalPendingScope: original.nativeScope)
    _ = try recheck()
    return KagemushaNativeOrdinaryAppIdentityConfirmationV1(accepted: accepted)
  }

  /// Cancel this original ticket. Native Core rejects cancellation after invocation.
  public func cancel() throws { _ = try call(7) }

  /// Offer the exact FI challenge and signing message after native final-identity admission.
  /// Native phase9 authenticates and journals the originals; no caller constructs the holder.
  /// The returned ceremony retains wallet/FI originals but does not enable cash dispatch.
  public func prepareRetailEnrollment(originalChallenge: Data, accountSigningMessage: Data,
    identity: KagemushaNativeOrdinaryAppIdentityConfirmationV1) throws
    -> KagemushaNativePreparedRetailEnrollmentV1 {
    try KagemushaNativePreparedRetailEnrollmentV1.prepare(bridge: bridge, possession: self,
      identity: identity, originalChallenge: originalChallenge, accountSigningMessage: accountSigningMessage)
  }

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

/// Original assertion and receipt read from the same consumed native E owner.
/// These public transport bytes do not create a credential or financial authority.
public struct KagemushaNativeConsumedAppEnrollmentPossessionOriginalV1: Sendable {
  public let rawAssertion: Data
  public let receipt: KagemushaNativeAppEnrollmentPossessionReceiptV1

  fileprivate init(rawAssertion: Data, receipt: KagemushaNativeAppEnrollmentPossessionReceiptV1) {
    self.rawAssertion = Data(rawAssertion)
    self.receipt = receipt
  }
}

/// Native confirmation that the same pending E owner retained a current signed app
/// identity. It grants no financial readiness and is distinct from the E receipt.
/// Only the genuine prepared holder's guarded phase8 path can create this value.
public struct KagemushaNativeOrdinaryAppIdentityConfirmationV1: Sendable {
  public let credentialDigest: Data
  public let pendingScope: Data

  fileprivate init(accepted: KagemushaAppEnrollmentFinalIdentityProjectionV1) {
    credentialDigest = accepted.credentialDigest; pendingScope = accepted.pendingScope
  }
}

extension KagemushaCoreCoordinatorBridgeV1 {
  /// Select the held pending original by SHA256(C canonical signing bytes), never its stable attempt ID.
  /// No caller constructs E, changes C or supplies pending authority.
  /// Missing signed raw-attestation admission, original owner or scope remains unavailable.
  public func prepareAppEnrollmentPossession(originalEnrollmentChallengeHash: Data) throws
    -> KagemushaNativePreparedAppEnrollmentPossessionV1 {
    try KagemushaNativePreparedAppEnrollmentPossessionV1.prepare(bridge: self, id: originalEnrollmentChallengeHash)
  }
}
