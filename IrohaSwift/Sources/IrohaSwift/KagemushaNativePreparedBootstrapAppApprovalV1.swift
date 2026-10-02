import CryptoKit
import Foundation

/// The same FI-retained Native zero-state Bootstrap W. No archive initializer is public.
/// Its captured receipt cannot approve a generic monetary operation or a State transition.
public final class KagemushaNativePreparedBootstrapAppApprovalV1: @unchecked Sendable {
  private let bridge: KagemushaCoreCoordinatorBridgeV1
  private let retail: KagemushaNativePreparedRetailEnrollmentV1
  private let original: KagemushaAppPlatformPreparedProjectionV1
  private let enrollmentID: Data
  private let lock = NSRecursiveLock()
  // 0 prepared, 1 invoked/unknown, 2 raw retained, 3 consumed. Observations never regress.
  private var stage: UInt8 = 0
  private var rawOriginal: Data?
  private var receiptOriginal: Data?
  private var unusable = false

  private init(bridge: KagemushaCoreCoordinatorBridgeV1, retail: KagemushaNativePreparedRetailEnrollmentV1,
    original: KagemushaAppPlatformPreparedProjectionV1, enrollmentID: Data) {
    self.bridge = bridge; self.retail = retail; self.original = original
    self.enrollmentID = Data(enrollmentID)
  }

  static func prepare(bridge: KagemushaCoreCoordinatorBridgeV1, retail: KagemushaNativePreparedRetailEnrollmentV1,
    operationID: Data) throws -> KagemushaNativePreparedBootstrapAppApprovalV1 {
    do {
      let fi = try retail.recheckForBootstrap()
      let fields = try bridge.invoke(.appOperationApproval,
        fields: [KagemushaCoreCoordinatorFrameV1.u32(8), operationID])
      let projection = try KagemushaAppPlatformPreparedProjectionV1(nativeBootstrapFields: fields,
        operationID: operationID, credentialDigest: fi.credentialDigest)
      let result = KagemushaNativePreparedBootstrapAppApprovalV1(bridge: bridge, retail: retail,
        original: projection, enrollmentID: fi.enrollmentID)
      _ = try result.recheck()
      return result
    } catch { try? bridge.close(); throw error }
  }

  /// Read exact W after the original FI and Native Bootstrap owners recheck current custody.
  public func signingBytes() throws -> Data { try recheck().signingBytes }

  /// Cancel only the original uninvoked attempt; Native rejects any previously fenced attempt.
  public func cancel() throws {
    try guarded {
      _ = try recheck()
      guard try readRecovery().state == 0 else { throw Self.invalid() }
      _ = try call(7)
      unusable = true
    }
  }

  func recheck() throws -> KagemushaAppPlatformPreparedProjectionV1 {
    try guarded {
      let fi = try retail.recheckForBootstrap()
      let app = fi.app
      guard fi.enrollmentID == enrollmentID, fi.credentialDigest == original.credentialDigest,
        original.bootstrapApproval != nil, original.approval == nil,
        app.platform == original.platform, app.keyAlias == original.keyAlias,
        app.publicKeyX963 == original.publicKeyX963, app.keyID == original.keyID,
        app.enrollmentChallenge == original.enrollmentChallenge,
        app.generationChallenge == original.generationChallenge,
        app.appSigningIdentityDigest == original.appSigningIdentityDigest else { throw Self.invalid() }
      let checked = try call(6)
      guard checked[0] == original.nativeScope,
        checked[1] == Data(SHA256.hash(data: original.signingBytes)) else { throw Self.invalid() }
      return original
    }
  }

  func fence() throws -> (state: UInt8, raw: Data, receipt: Data) {
    try guarded {
      _ = try recheck()
      let before = try readRecovery()
      // Once invoked locally, a prepared recovery response cannot renew permission.
      guard stage != 1 else { throw Self.invalid() }
      let fields = try call(2)
      let result = (state: fields[0][0], raw: fields[1], receipt: fields[2])
      if result.state == 1 {
        guard before.state == 0, stage == 0 else { throw Self.invalid() }
        stage = 1
      } else {
        try observe(stage: result.state, raw: result.raw, receipt: result.receipt)
      }
      _ = try recheck()
      return result
    }
  }

  func recover() throws -> (state: UInt8, raw: Data, receipt: Data) {
    try guarded { _ = try recheck(); let result = try readRecovery(); _ = try recheck(); return result }
  }

  func retainOriginal(_ bytes: Data) throws {
    let offered = Data(bytes)
    guard (1...4096).contains(offered.count) else { throw Self.invalid() }
    try guarded {
      _ = try recheck()
      guard stage >= 1, rawOriginal == nil || rawOriginal == offered else { throw Self.invalid() }
      _ = try call(3, raw: offered)
      let recovered = try readRecovery()
      guard recovered.state >= 1, recovered.raw == offered else { throw Self.invalid() }
      _ = try recheck()
    }
  }

  func consume(original evidence: KagemushaAppAttestBootstrapApprovalOriginalV1) throws
    -> KagemushaNativeCapturedBootstrapAppApprovalReceiptV1 {
    try guarded {
      _ = try recheck()
      let recovered = try readRecovery()
      guard recovered.state >= 1, recovered.raw == evidence.rawAssertion else { throw Self.invalid() }
      let fields = try call(4)
      let result = try receipt(fields[0], evidence: evidence)
      try observe(stage: 3, raw: evidence.rawAssertion, receipt: fields[0])
      let after = try readRecovery()
      guard after.state == 2, after.receipt == fields[0] else { throw Self.invalid() }
      _ = try recheck()
      return result
    }
  }

  func recoveredReceipt(_ bytes: Data, evidence: KagemushaAppAttestBootstrapApprovalOriginalV1) throws
    -> KagemushaNativeCapturedBootstrapAppApprovalReceiptV1 {
    try guarded {
      _ = try recheck()
      let recovered = try readRecovery()
      guard recovered.state == 2, recovered.receipt == bytes, recovered.raw == evidence.rawAssertion else {
        throw Self.invalid()
      }
      let result = try receipt(bytes, evidence: evidence)
      _ = try recheck()
      return result
    }
  }

  private func receipt(_ bytes: Data, evidence: KagemushaAppAttestBootstrapApprovalOriginalV1) throws
    -> KagemushaNativeCapturedBootstrapAppApprovalReceiptV1 {
    let r = try KagemushaAppPlatformReceiptProjectionV1(bytes)
    guard let w = original.bootstrapApproval, evidence.clientDataHash == w.clientDataHash, r.purpose == 1, r.ticket == original.ticket,
      r.originalID == w.operationID, r.nativeScope == original.nativeScope,
      r.challengeDigest == w.clientDataHash,
      r.rawEvidenceDigest == Data(SHA256.hash(data: evidence.rawAssertion)),
      r.originalScopeDigest == original.credentialDigest,
      r.appleCounter == evidence.observedCounter else { throw Self.invalid() }
    return KagemushaNativeCapturedBootstrapAppApprovalReceiptV1(original: original,
      receipt: r, enrollmentID: enrollmentID)
  }
  private func readRecovery() throws -> (state: UInt8, raw: Data, receipt: Data) {
    let fields = try call(5), state = fields[0][0]
    try observe(stage: state == 0 ? 0 : state + 1, raw: fields[1], receipt: fields[2])
    return (state, fields[1], fields[2])
  }

  private func observe(stage next: UInt8, raw: Data, receipt: Data) throws {
    guard next >= stage, rawOriginal == nil || rawOriginal == raw,
      receiptOriginal == nil || receiptOriginal == receipt else { throw Self.invalid() }
    if next >= 2 { rawOriginal = Data(raw) }
    if next == 3 { receiptOriginal = Data(receipt) }
    stage = next
  }

  private func call(_ phase: UInt32, raw: Data? = nil) throws -> [Data] {
    var fields = [KagemushaCoreCoordinatorFrameV1.u32(phase), original.ticket]
    if let raw { fields.append(raw) }
    return try bridge.invoke(.appOperationApproval, fields: fields)
  }
  private func guarded<T>(_ operation: () throws -> T) throws -> T {
    lock.lock(); defer { lock.unlock() }
    guard !unusable else { throw Self.invalid() }
    do { return try operation() }
    catch { unusable = true; try? bridge.close(); throw error }
  }
  private static func invalid() -> KagemushaCoreCoordinatorErrorV1 {
    .invalidFrame("Native Bootstrap original differs from retained FI")
  }
}

/// Native's correlated receipt for fsynced Bootstrap capture, restricted to the same zero state.
/// The actual captured capability remains in Native custody; these bytes cannot reconstruct it.
public struct KagemushaNativeCapturedBootstrapAppApprovalReceiptV1: Sendable {
  public let enrollmentID: Data
  public let operationID: Data
  public let keyID: Data
  public let keyAlias: String
  public let signingDigest: Data
  public let rawAssertionDigest: Data
  public let observedCounter: UInt32
  public let canonicalReceipt: Data

  fileprivate init(original: KagemushaAppPlatformPreparedProjectionV1,
    receipt: KagemushaAppPlatformReceiptProjectionV1, enrollmentID: Data) {
    self.enrollmentID = Data(enrollmentID); operationID = receipt.originalID
    keyID = original.keyID; keyAlias = original.keyAlias; signingDigest = receipt.challengeDigest
    rawAssertionDigest = receipt.rawEvidenceDigest; observedCounter = receipt.appleCounter!
    canonicalReceipt = receipt.canonicalBytes
  }
}

/// The original Apple equation over exact Bootstrap W, never a captured Native capability.
struct KagemushaAppAttestBootstrapApprovalOriginalV1: Sendable {
  let rawAssertion: Data
  let observedCounter: UInt32
  let clientDataHash: Data

  init(rawAssertion: Data, nativeProjection: KagemushaBootstrapAppApprovalSigningProjectionV1,
    enrolledKeyID: Data, enrolledPublicKeyX963: Data, expectedAppIDHash: Data,
    expectedRelease: KagemushaAppAttestExpectedReleaseV1, nativeCounterFloor: UInt32) throws {
    guard (1...311).contains(rawAssertion.count), enrolledKeyID == nativeProjection.attestedKeyID,
      enrolledKeyID == Data(SHA256.hash(data: enrolledPublicKeyX963)) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("Bootstrap Apple key differs")
    }
    let evidence = try KagemushaAppAttestAssertionEvidenceV1(rawAssertion: rawAssertion,
      clientDataHash: nativeProjection.clientDataHash, expectedAppIDHash: expectedAppIDHash,
      expectedRelease: expectedRelease, enrolledAssertionPublicKeyX963: enrolledPublicKeyX963)
    guard (37...206).contains(evidence.authenticatorData.count),
      nativeCounterFloor < UInt32.max, evidence.signCount > nativeCounterFloor else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("Bootstrap Apple original counter differs")
    }
    self.rawAssertion = Data(evidence.rawAssertion); observedCounter = evidence.signCount
    clientDataHash = nativeProjection.clientDataHash
  }
}
