import Foundation

/// Pure local journal reconciliation, never authentication of a native receipt.
/// The called provider derives nativeConsumedOriginal only after its opaque owner
/// reauthenticates the exact archived raw/receipt under current native custody.
enum KagemushaAppApprovalIntentReconciliationV1 {
  enum Action: Equatable { case persistOriginal, completedOriginal, archivedConsumedOriginal }

  static func classify(_ intent: KagemushaAppAttestAssertionIntentV1,
    previousCounter: UInt32, counter: UInt32, signingDigest: Data, raw: Data,
    nativeConsumedOriginal: Bool) throws -> Action {
    guard previousCounter < UInt32.max, counter > previousCounter,
      signingDigest.count == 32, signingDigest.contains(where: { $0 != 0 }),
      (1...4096).contains(raw.count) else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    switch intent {
    case .pending(let floor, let digest) where floor == previousCounter && digest == signingDigest:
      return .persistOriginal
    case .complete(let floor, let observed, let digest, let original)
      where floor == previousCounter && observed == counter && digest == signingDigest && original == raw:
      return .completedOriginal
    case .ready(let current) where nativeConsumedOriginal && current >= counter:
      return .archivedConsumedOriginal
    default: throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
  }
}
