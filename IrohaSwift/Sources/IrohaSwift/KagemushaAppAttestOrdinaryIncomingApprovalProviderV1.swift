import Foundation

/// Actual App Attest signer for opaque incoming W2/W1 originals from the same Native owner.
/// Outgoing and Bootstrap capabilities remain separate; caller data cannot choose W or the key.
public actor KagemushaAppAttestOrdinaryIncomingApprovalProviderV1 {
  private let service: any KagemushaAppAttestServiceV1
  private let intentStore: any KagemushaAppAttestOrdinaryIncomingIntentStoringV1
  private let expectedRelease: KagemushaAppAttestExpectedReleaseV1
  private var active = false
  private var locallyUncertain = false
  public init(service: any KagemushaAppAttestServiceV1,
    intentStore: any KagemushaAppAttestOrdinaryIncomingIntentStoringV1,
    expectedRelease: KagemushaAppAttestExpectedReleaseV1) {
    self.service = service; self.intentStore = intentStore; self.expectedRelease = expectedRelease
  }
  public func approve(_ operation: KagemushaNativePreparedOrdinaryIncomingApprovalV1) async throws
    -> KagemushaNativeOrdinaryIncomingApprovalReceiptV1 {
    guard service.isSupported else { throw KagemushaAppAttestEvidenceErrorV1.unsupportedDevice }
    guard !active else { throw KagemushaAppAttestEvidenceErrorV1.assertionAlreadyInFlight }
    guard !locallyUncertain else { throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown }
    active = true; defer { active = false }
    do {
      let p = try apple(operation)
      let recovered = try operation.recover()
      if recovered.state != 0 { return try finish(operation, projection: p, raw: recovered.raw) }
      let floor = p.counterFloor!, digest = p.approval.clientDataHash
      switch try intentStore.load(keyID: p.keyAlias) {
      case .ready(let counter) where counter == floor:
        try intentStore.reserve(keyID: p.keyAlias, previousCounter: floor, selectionDigest: digest)
      case .pending(let counter, let held) where counter == floor && held == digest:
        break // Native recovery above proved the sole original was never OS-invoked.
      default: throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
      }
      guard try intentStore.load(keyID: p.keyAlias) == .pending(previousCounter: floor, selectionDigest: digest) else {
        throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
      }
      let fenced = try operation.fence() // Main's durable invocation fence precedes Apple.
      if fenced.state != 0 { return try finish(operation, projection: p, raw: fenced.raw) }
      _ = try operation.recheck()
      let raw: Data
      do { raw = try await service.generateAssertion(p.keyAlias, clientDataHash: digest) }
      catch { throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown }
      _ = try evidence(raw, projection: p)
      try operation.retainOriginal(raw)
      return try finish(operation, projection: p, raw: raw)
    } catch { locallyUncertain = true; throw error }
  }
  /// Recovery uses only actual Native-retained raw evidence and never invokes Apple again.
  public func recoverApproval(_ operation: KagemushaNativePreparedOrdinaryIncomingApprovalV1) throws
    -> KagemushaNativeOrdinaryIncomingApprovalReceiptV1 {
    guard !active else { throw KagemushaAppAttestEvidenceErrorV1.assertionAlreadyInFlight }
    active = true; defer { active = false }
    do {
      let p = try apple(operation), recovered = try operation.recover()
      guard recovered.state == 1 || recovered.state == 2 else {
        throw KagemushaAppAttestEvidenceErrorV1.assertionOutcomeUnknown
      }
      let receipt = try finish(operation, projection: p, raw: recovered.raw)
      locallyUncertain = false
      return receipt
    } catch { locallyUncertain = true; throw error }
  }
  private func apple(_ operation: KagemushaNativePreparedOrdinaryIncomingApprovalV1) throws
    -> KagemushaOrdinaryIncomingSigningProjectionV1 {
    let p = try operation.recheck()
    guard p.platform == 4, let floor = p.counterFloor, floor < UInt32.max else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("Incoming Apple approval requires its original key and unexhausted retained floor")
    }
    return p
  }
  private func evidence(_ raw: Data, projection p: KagemushaOrdinaryIncomingSigningProjectionV1) throws
    -> KagemushaIncomingAppAttestOriginalV1 {
    try KagemushaIncomingAppAttestOriginalV1(raw: raw, projection: p, expectedRelease: expectedRelease)
  }
  private func finish(_ operation: KagemushaNativePreparedOrdinaryIncomingApprovalV1,
    projection p: KagemushaOrdinaryIncomingSigningProjectionV1, raw: Data) throws
    -> KagemushaNativeOrdinaryIncomingApprovalReceiptV1 {
    _ = try operation.recheck()
    let original = try evidence(raw, projection: p)
    let recovered = try operation.recover()
    guard recovered.state == 1 || recovered.state == 2, recovered.raw == raw else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    let existing = recovered.state == 2 ? try operation.consumedReceipt(original) : nil
    let floor = p.counterFloor!, digest = p.approval.clientDataHash
    let action = try KagemushaAppApprovalIntentReconciliationV1.classify(
      intentStore.load(keyID: p.keyAlias), previousCounter: floor, counter: original.observedCounter,
      signingDigest: digest, raw: raw, nativeConsumedOriginal: existing != nil)
    switch action {
    case .persistOriginal:
      try intentStore.complete(keyID: p.keyAlias, counter: original.observedCounter,
        selectionDigest: digest, rawAssertion: raw)
    case .completedOriginal: break
    case .archivedConsumedOriginal: return existing!
    }
    guard try intentStore.load(keyID: p.keyAlias) == .complete(previousCounter: floor,
      counter: original.observedCounter, selectionDigest: digest, rawAssertion: raw) else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    if existing == nil { try operation.retainOriginal(raw) }
    let result = try existing ?? operation.consumedReceipt(original)
    try intentStore.advanceAfterNativeIncomingApproval(keyID: p.keyAlias, counter: original.observedCounter,
      signingDigest: digest, rawAssertion: raw, receipt: result)
    guard try intentStore.load(keyID: p.keyAlias) == .ready(counter: original.observedCounter) else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    locallyUncertain = false
    return result
  }
}
