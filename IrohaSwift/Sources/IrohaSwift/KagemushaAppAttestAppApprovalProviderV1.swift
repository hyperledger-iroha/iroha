import Foundation

/// App Attest ordinary approval for a genuine native W ticket only. It cannot sign
/// caller-provided subjects, select another key, or accept a monetary transition.
public actor KagemushaAppAttestAppApprovalProviderV1 {
  private let service: any KagemushaAppAttestServiceV1
  private let intentStore: any KagemushaAppAttestAssertionIntentStoringV1
  private let expectedRelease: KagemushaAppAttestExpectedReleaseV1
  private var invocationInFlight = false
  private var locallyUncertain = false

  public init(service: any KagemushaAppAttestServiceV1,
    intentStore: any KagemushaAppAttestAssertionIntentStoringV1,
    expectedRelease: KagemushaAppAttestExpectedReleaseV1) {
    self.service = service; self.intentStore = intentStore; self.expectedRelease = expectedRelease
  }

  /// The native owner supplies W, key, AppID, original enrollment and counter floor.
  /// Durable local reservation and native invocation fence both precede the device call.
  public func approve(_ operation: KagemushaNativePreparedAppApprovalV1) async throws
    -> KagemushaNativeAppApprovalReceiptV1 {
    guard service.isSupported else { throw KagemushaAppAttestEvidenceErrorV1.unsupportedDevice }
    guard !invocationInFlight else { throw KagemushaAppAttestEvidenceErrorV1.assertionAlreadyInFlight }
    guard !locallyUncertain else { throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown }
    invocationInFlight = true
    defer { invocationInFlight = false }
    do {
      let p = try appleProjection(operation)
      let recovered = try operation.recover()
      if recovered.state != 0 {
        return try finish(operation, p: p, raw: recovered.raw, receipt: recovered.receipt)
      }
      let floor = p.appleCounterFloor!, digest = p.approval!.clientDataHash
      switch try intentStore.load(keyID: p.keyAlias) {
      case .ready(let counter) where counter == floor:
        try intentStore.reserve(keyID: p.keyAlias, previousCounter: floor, selectionDigest: digest)
      case .pending(let counter, let pendingDigest) where counter == floor && pendingDigest == digest:
        // Native recovery above proved this exact ticket has never been invoked.
        // Resume the same reservation, never a replacement operation/nonce/key.
        break
      default: throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
      }
      guard try intentStore.load(keyID: p.keyAlias) == .pending(
        previousCounter: floor, selectionDigest: digest) else {
        throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
      }
      let fenced = try operation.fence()
      if fenced.state != 1 {
        return try finish(operation, p: p, raw: fenced.raw, receipt: fenced.receipt)
      }
      _ = try operation.recheck()
      let raw: Data
      do { raw = try await service.generateAssertion(p.keyAlias, clientDataHash: digest) }
      catch { throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown }
      _ = try evidence(raw, p: p)
      try operation.retainOriginal(raw)
      return try finish(operation, p: p, raw: raw, receipt: Data())
    } catch {
      // A failed/fenced attempt has no implicit platform retry. Recovery can use only
      // its native-retained original evidence, including after local fsync failure.
      locallyUncertain = true
      throw error
    }
  }

  /// Recover only native-retained original evidence; this method never calls Apple.
  public func recoverApproval(_ operation: KagemushaNativePreparedAppApprovalV1) throws
    -> KagemushaNativeAppApprovalReceiptV1 {
    guard !invocationInFlight else { throw KagemushaAppAttestEvidenceErrorV1.assertionAlreadyInFlight }
    invocationInFlight = true
    defer { invocationInFlight = false }
    do {
      let p = try appleProjection(operation)
      let retained = try operation.recover()
      guard retained.state == 1 || retained.state == 2 else {
        throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown
      }
      let receipt = try finish(operation, p: p, raw: retained.raw, receipt: retained.receipt)
      locallyUncertain = false
      return receipt
    } catch { locallyUncertain = true; throw error }
  }

  private func appleProjection(_ operation: KagemushaNativePreparedAppApprovalV1) throws
    -> KagemushaAppPlatformPreparedProjectionV1 {
    let p = try operation.recheck()
    guard p.platform == 4, p.appleCounterFloor != nil, p.approval != nil else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("ordinary Apple approval requires original Apple W")
    }
    return p
  }

  private func evidence(_ raw: Data, p: KagemushaAppPlatformPreparedProjectionV1) throws
    -> KagemushaAppAttestApprovalOriginalV1 {
    try KagemushaAppAttestApprovalOriginalV1(rawAssertion: raw, nativeProjection: p.approval!,
      enrolledKeyID: p.keyID, enrolledPublicKeyX963: p.publicKeyX963,
      expectedAppIDHash: p.appSigningIdentityDigest, expectedRelease: expectedRelease,
      nativeCounterFloor: p.appleCounterFloor!)
  }

  private func finish(_ operation: KagemushaNativePreparedAppApprovalV1,
    p: KagemushaAppPlatformPreparedProjectionV1, raw: Data, receipt bytes: Data) throws
    -> KagemushaNativeAppApprovalReceiptV1 {
    _ = try operation.recheck()
    let original = try evidence(raw, p: p)
    let floor = p.appleCounterFloor!, digest = p.approval!.clientDataHash
    let existingReceipt = bytes.isEmpty ? nil : try operation.recoveredReceipt(bytes, evidence: original)
    let completed = KagemushaAppAttestAssertionIntentV1.complete(previousCounter: floor,
      counter: original.observedCounter, selectionDigest: digest, rawAssertion: raw)
    let action = try KagemushaAppApprovalIntentReconciliationV1.classify(
      try intentStore.load(keyID: p.keyAlias), previousCounter: floor,
      counter: original.observedCounter, signingDigest: digest, raw: raw,
      nativeConsumedOriginal: existingReceipt != nil)
    switch action {
    case .persistOriginal:
      try intentStore.complete(keyID: p.keyAlias, counter: original.observedCounter,
        selectionDigest: digest, rawAssertion: raw)
    case .completedOriginal:
      break
    case .archivedConsumedOriginal:
      // A later genuine ordinary approval may have advanced this same key. Return
      // the archived native-consumed original without any write or counter rollback.
      return existingReceipt!
    }
    guard try intentStore.load(keyID: p.keyAlias) == completed else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    let result = try existingReceipt ?? operation.consume(original: original)
    try intentStore.advanceAfterNativeAppApproval(keyID: p.keyAlias,
      counter: original.observedCounter, signingDigest: digest,
      rawAssertion: raw, receipt: result)
    guard try intentStore.load(keyID: p.keyAlias) == .ready(counter: original.observedCounter) else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    locallyUncertain = false
    return result
  }
}
