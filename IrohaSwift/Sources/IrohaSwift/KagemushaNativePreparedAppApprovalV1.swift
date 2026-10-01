import CryptoKit
import Foundation

/// Process-local original native W capability. No byte/DTO initializer is public.
/// Identity approval is separate from a proved monetary transition and StateGuard.
public final class KagemushaNativePreparedAppApprovalV1: @unchecked Sendable {
  private let bridge: KagemushaCoreCoordinatorBridgeV1
  private let original: KagemushaAppPlatformPreparedProjectionV1

  private init(bridge: KagemushaCoreCoordinatorBridgeV1,
    original: KagemushaAppPlatformPreparedProjectionV1) {
    self.bridge = bridge; self.original = original
  }

  static func prepare(bridge: KagemushaCoreCoordinatorBridgeV1, id: Data) throws
    -> KagemushaNativePreparedAppApprovalV1 {
    let fields = try bridge.invoke(.appOperationApproval,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(1), id])
    let projection = try KagemushaAppPlatformPreparedProjectionV1(
      nativeFields: fields, approvalID: id, enrollmentChallengeHash: nil)
    let result = KagemushaNativePreparedAppApprovalV1(bridge: bridge, original: projection)
    _ = try result.recheck()
    return result
  }

  /// Read exact W only after the original native owner has rechecked scope and time.
  /// These bytes are correlation data and cannot recreate this capability.
  public func signingBytes() throws -> Data { try recheck().signingBytes }

  /// Cancel this original ticket. Native Core rejects cancellation after invocation.
  public func cancel() throws { _ = try call(7) }

  func recheck() throws -> KagemushaAppPlatformPreparedProjectionV1 {
    let checked = try call(6)
    guard checked[0] == original.nativeScope,
      checked[1] == Data(SHA256.hash(data: original.signingBytes)) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("current owner substituted app approval scope")
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

  func consume(original evidence: KagemushaAppAttestApprovalOriginalV1) throws
    -> KagemushaNativeAppApprovalReceiptV1 {
    _ = try recheck()
    let fields = try call(4)
    return try receipt(fields[0], evidence: evidence)
  }

  func recoveredReceipt(_ bytes: Data, evidence: KagemushaAppAttestApprovalOriginalV1) throws
    -> KagemushaNativeAppApprovalReceiptV1 {
    _ = try recheck()
    return try receipt(bytes, evidence: evidence)
  }

  private func receipt(_ bytes: Data, evidence: KagemushaAppAttestApprovalOriginalV1) throws
    -> KagemushaNativeAppApprovalReceiptV1 {
    let r = try KagemushaAppPlatformReceiptProjectionV1(bytes)
    guard let w = original.approval, r.purpose == 1, r.ticket == original.ticket,
      r.originalID == w.operationID, r.nativeScope == original.nativeScope,
      r.challengeDigest == w.clientDataHash,
      r.rawEvidenceDigest == Data(SHA256.hash(data: evidence.rawAssertion)),
      r.originalScopeDigest == original.credentialDigest,
      r.appleCounter == evidence.observedCounter else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("native receipt differs from original Apple W")
    }
    return KagemushaNativeAppApprovalReceiptV1(original: original, receipt: r)
  }

  private func call(_ phase: UInt32, raw: Data? = nil) throws -> [Data] {
    var fields = [KagemushaCoreCoordinatorFrameV1.u32(phase), original.ticket]
    if let raw { fields.append(raw) }
    return try bridge.invoke(.appOperationApproval, fields: fields)
  }
}

/// Exact native-consumed original app identity approval, never monetary authority.
/// It is created only by the genuine bridge's purpose-bound native consume/recovery.
public struct KagemushaNativeAppApprovalReceiptV1: Sendable {
  public let operationID: Data
  public let keyID: Data
  public let keyAlias: String
  public let signingDigest: Data
  public let rawAssertionDigest: Data
  public let observedCounter: UInt32
  public let canonicalReceipt: Data

  fileprivate init(original: KagemushaAppPlatformPreparedProjectionV1,
    receipt: KagemushaAppPlatformReceiptProjectionV1) {
    operationID = receipt.originalID; keyID = original.keyID; keyAlias = original.keyAlias
    signingDigest = receipt.challengeDigest; rawAssertionDigest = receipt.rawEvidenceDigest
    observedCounter = receipt.appleCounter!; canonicalReceipt = receipt.canonicalBytes
  }
}

extension KagemushaCoreCoordinatorBridgeV1 {
  /// Select an existing original native financial operation; no caller constructs S/W.
  /// Missing original owner, credential, proof input or current scope remains unavailable.
  public func prepareAppApproval(originalOperationID: Data) throws
    -> KagemushaNativePreparedAppApprovalV1 {
    try KagemushaNativePreparedAppApprovalV1.prepare(bridge: self, id: originalOperationID)
  }
}
