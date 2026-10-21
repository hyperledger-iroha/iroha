import Foundation

/// Native-selected wait reason. This observation cannot be supplied as admission authority.
public enum KagemushaWalletMaintenanceWaitV1: UInt32, Sendable {
  case selectedPending = 1, archiveFold = 2, foldBacklog = 3, exchangeOpen = 4, notDue = 5, busy = 6
}

/// One bounded Native-selected maintenance turn, separate from Payment delivery evidence.
public enum KagemushaWalletDeferredArchiveTurnV1: Sendable {
  case noWork
  case waiting(KagemushaWalletMaintenanceWaitV1)
  case reconciled(KagemushaWalletCallV1)
  case executed(KagemushaWalletCallV1)

  init(_ result: KagemushaWalletCallV1) throws {
    switch result.status {
    case 62: self = .noWork
    case 63:
      guard let wait = KagemushaWalletMaintenanceWaitV1(rawValue: result.detail) else {
        throw KagemushaWalletErrorV1.invalidNativeOutput
      }
      self = .waiting(wait)
    case 64:
      let bytes = [UInt8](result.bytes)
      guard (28...10_028).contains(bytes.count) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      func u32(_ offset: Int) -> UInt32 {
        (0..<4).reduce(UInt32(0)) { $0 | UInt32(bytes[offset + $1]) << (8 * $1) }
      }
      func u64(_ offset: Int) -> UInt64 {
        (0..<8).reduce(UInt64(0)) { $0 | UInt64(bytes[offset + $1]) << (8 * $1) }
      }
      let kind = u32(0), count = u32(24)
      guard [1, 2, 3, 4, 5, 10].contains(kind), count <= 10_000,
        bytes.count == 28 + Int(count) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      let completion = try KagemushaWalletCallV1(status: Int32(kind), sequenceLow: u64(8),
        sequenceHigh: u64(16), detail: u32(4), bytes: Data(bytes.dropFirst(28))).completion()
      self = result.detail == 1 ? .reconciled(completion) : .executed(completion)
    default: throw KagemushaWalletErrorV1.invalidNativeOutput
    }
  }
}

extension KagemushaWalletV1 {
  /// Verify at most one Native-selected retained intake after interruption. No host locator,
  /// replacement Send identity or caller financial verdict participates in recovery.
  public func resumeDeferredReceipt() throws -> KagemushaWalletCreditProjectionV1? {
    let result = try setup(.init(selector: 57))
    if result.status == 62 { return nil }
    return try KagemushaWalletCreditProjectionV1(result).requirePayer()
  }
  /// Attempt one bounded Native maintenance turn. Native selects recovery and rechecks actual
  /// exchange occupancy, fold coverage, charging, count and age immediately before dispatch.
  public func drainDeferredArchive() throws -> KagemushaWalletDeferredArchiveTurnV1 {
    try .init(setup(.init(selector: 58)))
  }
}
