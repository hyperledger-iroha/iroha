import Foundation
import NoritoBridge

/// Opaque one-use native direct-time exchange, bound to the logical exchange that created it.
public final class KagemushaWalletTimeExchangeV1: @unchecked Sendable, CustomStringConvertible {
  private let origin: AnyObject
  private let token: UInt64
  private let lock = NSLock()
  private var consumed = false
  /// Fresh native nonce sent to the issuer; no local clock reading is exposed.
  public let nonce: Data
  init(origin: AnyObject, token: UInt64, nonce: Data) {
    self.origin = origin; self.token = token; self.nonce = kagemushaWalletSetupCopyV1(nonce)
  }
  func tokenFor(origin: AnyObject) throws -> UInt64 {
    lock.lock(); defer { lock.unlock() }
    guard origin === self.origin, !consumed else { throw KagemushaWalletErrorV1.invalidInput }
    return token
  }
  func consume(origin: AnyObject) throws {
    lock.lock(); defer { lock.unlock() }
    guard origin === self.origin, !consumed else { throw KagemushaWalletErrorV1.invalidInput }
    consumed = true
  }
  public var description: String { "KagemushaWalletTimeExchangeV1(challenge=[REDACTED])" }
}

extension KagemushaWalletCallV1 {
  func feeClaimInput(beneficiary: Data) throws -> KagemushaWalletSetupInputV1 {
    guard status == 31 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return try .init(selector: 26, first: bytes, second: beneficiary)
  }
  func feeClaimOriginal() throws -> Data {
    guard status == 36 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return bytes
  }
  func unloadClaimOriginal() throws -> Data {
    guard status == 48 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return bytes
  }
  func creditedInput() throws -> KagemushaWalletSetupInputV1 {
    guard status == 1 || status == 10 else { throw KagemushaWalletErrorV1.invalidInput }
    return try .init(selector: status == 10 ? 17 : 16, first: bytes)
  }
  func completion() throws -> Self {
    guard status <= 11 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return self
  }
  func original() throws -> Data {
    guard status == 12 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return bytes
  }
  func exchange(origin: AnyObject) throws -> KagemushaWalletTimeExchangeV1 {
    guard status == 13 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return KagemushaWalletTimeExchangeV1(origin: origin, token: sequenceLow, nonce: bytes)
  }
  func timeRetained() throws -> Self {
    guard status == 14 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return self
  }
}

/// Fixed typed bounds only. Rust authenticates every original and selects state and proof owners.
struct KagemushaWalletSetupInputV1 {
  /// Closed current ABI classification. Native independently rejects every wrong origin.
  var requiresExchange: Bool {
    ![8, 9, 10, 12, 13, 14, 16, 17, 18, 20, 21, 22, 24, 26, 29, 33, 37,
      38, 40, 42, 43, 44, 45, 47, 50, 52, 55, 57, 58].contains(selector)
  }
  let selector: UInt32
  let identity: Data
  let amount: KagemushaWalletUInt128V1
  let token: UInt64
  let first: Data
  let second: Data
  let third: Data
  init(selector: UInt32, identity: Data = Data(repeating: 0, count: 32),
       amount: KagemushaWalletUInt128V1 = .init(low: 0, high: 0), token: UInt64 = 0,
       first: Data = Data(), second: Data = Data(), third: Data = Data()) throws {
    let limits: [Int]
    switch selector {
    case 0, 4, 6, 15, 18, 19, 20, 24, 27, 38, 43, 47, 48, 49, 50, 51, 52, 57, 58: limits = [0, 0, 0]
    case 53: limits = [256 * 1024, 0, 0]
    case 21, 22: limits = [21_024, 0, 0]
    case 23: limits = [36 * 1024 * 1024, 0, 0]
    case 25: limits = [32 * 1024 * 1024, 1024, 0]
    case 26: limits = [21_024, 16_384, 0]
    case 45: limits = [16_384, 0, 0]
    case 29, 30, 33, 35, 37, 46: limits = [65_536, 0, 0]
    case 34, 36: limits = [65_536, 36 * 1024 * 1024, 0]
    case 2: limits = [0, 1_024, 512]
    case 8...10, 12...14, 16...17, 42, 44, 55, 56: limits = [10_000, 0, 0]
    case 39, 40: limits = [10_000, 10_000, 0]
    case 5: limits = [512, 512, 0]
    default: throw KagemushaWalletErrorV1.invalidInput
    }
    guard identity.count == 32, ([2, 19, 20, 25, 27, 30, 33, 34, 39, 42, 43, 44, 45, 56].contains(selector)) == identity.contains(where: { $0 != 0 }),
      ([47, 53].contains(selector) || ([2, 27, 39].contains(selector)) == (amount.low != 0 || amount.high != 0)), ([5, 6, 29, 49, 51].contains(selector)) == (token != 0),
      selector != 53 || (amount.high == 0 && !first.isEmpty),
      token <= UInt64(Int64.max), selector != 29 || ((1...3).contains(token) && !first.isEmpty), zip([first, second, third], limits).allSatisfy({ $0.count <= $1 }),
      ![30, 33, 35, 37, 42, 44, 46, 55, 56].contains(selector) || !first.isEmpty,
      selector != 2 || second.isEmpty == third.isEmpty, selector != 39 || first.isEmpty == second.isEmpty,
      (!(8...10).contains(selector) && !(12...14).contains(selector) && !(16...17).contains(selector) && !(21...23).contains(selector)) || !first.isEmpty, ![5, 25, 26, 34, 36, 40].contains(selector) || (!first.isEmpty && !second.isEmpty)
    else { throw KagemushaWalletErrorV1.invalidInput }
    self.selector = selector; self.identity = kagemushaWalletSetupCopyV1(identity)
    self.amount = amount; self.token = token
    self.first = kagemushaWalletSetupCopyV1(first); self.second = kagemushaWalletSetupCopyV1(second)
    self.third = kagemushaWalletSetupCopyV1(third)
  }
  static func request(identity: Data, amount: KagemushaWalletUInt128V1, feeSchedule: Data?,
    feeCertificate: Data?) throws -> Self {
    guard (feeSchedule == nil) == (feeCertificate == nil),
      feeSchedule == nil || (!(feeSchedule?.isEmpty ?? true) && !(feeCertificate?.isEmpty ?? true))
    else { throw KagemushaWalletErrorV1.invalidInput }
    return try Self(selector: 2, identity: identity, amount: amount,
      second: feeSchedule ?? Data(), third: feeCertificate ?? Data())
  }
  static func requestWithFeePolicy(identity: Data, amount: KagemushaWalletUInt128V1,
    schedule: Data?, certificates: Data?) throws -> Self {
    guard (schedule == nil) == (certificates == nil),
      schedule.map({ !$0.isEmpty }) ?? true,
      certificates.map({ !$0.isEmpty }) ?? true
    else { throw KagemushaWalletErrorV1.invalidInput }
    return try Self(selector: 39, identity: identity, amount: amount,
      first: schedule ?? Data(), second: certificates ?? Data())
  }
  func withRequest<T>(_ body: (UnsafePointer<connect_norito_kagemusha_wallet_setup_request_v1>) -> T) -> T {
    identity.withUnsafeBytes { id in first.withUnsafeBytes { a in second.withUnsafeBytes { b in third.withUnsafeBytes { c in
      var value = connect_norito_kagemusha_wallet_setup_request_v1()
      value.setup_id = id.bindMemory(to: UInt8.self).baseAddress
      value.selector = selector; value.amount = .init(low: amount.low, high: amount.high); value.token = token
      value.first = a.bindMemory(to: UInt8.self).baseAddress; value.first_length = a.count
      value.second = b.bindMemory(to: UInt8.self).baseAddress; value.second_length = b.count
      value.third = c.bindMemory(to: UInt8.self).baseAddress; value.third_length = c.count
      return body(&value)
    }}}}
  }
}

private func kagemushaWalletSetupCopyV1(_ original: Data) -> Data {
  guard !original.isEmpty else { return Data() }
  return original.withUnsafeBytes { Data(bytes: $0.baseAddress!, count: $0.count) }
}

/// Exact peer-envelope kind; it cannot select proof keys or operation authority.
public enum KagemushaWalletTransportKindV1: UInt32, Sendable {
  case request = 2, payment = 3, credited = 4
}

/// Native worker scheduling state; durable current backlog remains available in `snapshot()`.
public struct KagemushaWalletBackgroundStatusV1: Sendable {
  /// A parked worker retains no upgraded wallet reference or proof workspace.
  public enum Phase: UInt32, Sendable { case notStarted = 0, parked = 1, running = 2 }
  public let phase: Phase
  public let eligible: Bool
  /// Last durable backlog observed by the worker; nil until its first observation.
  public let observedBacklog: KagemushaWalletUInt128V1?
  init(_ value: KagemushaWalletCallV1) throws {
    guard value.status == 29, value.bytes.isEmpty, value.detail & ~15 == 0,
      let phase = Phase(rawValue: value.detail & 3),
      value.detail & 8 != 0 || (value.sequenceLow == 0 && value.sequenceHigh == 0)
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    self.phase = phase; self.eligible = value.detail & 4 != 0
    self.observedBacklog = value.detail & 8 != 0 ? .init(low: value.sequenceLow, high: value.sequenceHigh) : nil
  }
}

/// Exact retained online fee-claim originals. This is neither payment delivery nor payout acknowledgement.
public struct KagemushaWalletFeeClaimV1: Sendable {
  public let payment: Data
  public let request: Data
  init(payment: Data, request: Data) {
    self.payment = kagemushaWalletSetupCopyV1(payment)
    self.request = kagemushaWalletSetupCopyV1(request)
  }
}
/// Last durably selected native Global-chain decision; it grants no claim acknowledgement by itself.
public struct KagemushaWalletLedgerProgressV1: Sendable {
  public let height: UInt64
  public let blockHash: Data
  init(_ result: KagemushaWalletCallV1) throws {
    guard result.status == 33 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    self.height = result.sequenceLow; self.blockHash = kagemushaWalletSetupCopyV1(result.bytes)
  }
}

/// Protected native epoch progress. Older retained epochs continue to authorize old receipts.
public struct KagemushaWalletEpochProgressV1: Equatable, Sendable {
  public let epoch: UInt64
  public let firstHeight: UInt64
  public let boundaryHeight: UInt64
  init(_ result: KagemushaWalletCallV1) throws {
    guard result.status == 57, result.bytes.count == 16 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    func word(_ start: Int) -> UInt64 {
      (0..<8).reduce(UInt64(0)) { $0 | UInt64(result.bytes[start + $1]) << ($1 * 8) }
    }
    epoch = result.sequenceLow; firstHeight = word(0); boundaryHeight = word(8)
    guard firstHeight > 0, boundaryHeight >= firstHeight else { throw KagemushaWalletErrorV1.invalidNativeOutput }
  }
}

/// Native collection progress, separate from payment completion or destruction of custody.
public enum KagemushaWalletCollectionStatusV1: Equatable, Sendable {
  /// Payment priority or disabled background work; no sequence was collected.
  case idle
  /// One durable collection action finished for this historical sequence.
  case progress(KagemushaWalletUInt128V1)
  /// Selected witnesses are collected; permanent replay records and required originals remain.
  case collected(KagemushaWalletUInt128V1)
  init(_ value: KagemushaWalletCallV1, expectedSequence: KagemushaWalletUInt128V1) throws {
    guard value.bytes.isEmpty, value.detail == 0 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    let sequence = KagemushaWalletUInt128V1(low: value.sequenceLow, high: value.sequenceHigh)
    switch value.status {
    case 50:
      guard sequence.low == 0 && sequence.high == 0 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      self = .idle
    case 51, 52:
      guard sequence == expectedSequence else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      self = value.status == 51 ? .progress(sequence) : .collected(sequence)
    default: throw KagemushaWalletErrorV1.invalidNativeOutput
    }
  }
}

/// Native-selected digests for locating exact policy originals. This DATA grants no Request authority.
public struct KagemushaWalletRequestFeeSelectionV1: Sendable, Equatable {
  public let assetDigest: Data
  /// An all-zero digest represents Native's selected zero-fee rule.
  public let feeDigest: Data
  init(_ result: KagemushaWalletCallV1) throws {
    guard result.status == 12, result.bytes.count == 64 else {
      throw KagemushaWalletErrorV1.invalidNativeOutput
    }
    assetDigest = kagemushaWalletSetupCopyV1(Data(result.bytes.prefix(32)))
    feeDigest = kagemushaWalletSetupCopyV1(Data(result.bytes.suffix(32)))
  }
}

extension KagemushaWalletV1 {
  public func requestFeeSelection() throws -> KagemushaWalletRequestFeeSelectionV1 {
    try .init(setup(.init(selector: 38)))
  }
  /// Validate exact PolicyData envelopes with Native's selected policy; this does not refresh it.
  public func validateRequestFeePolicy(schedule: Data, certificates: Data) throws -> KagemushaWalletRequestFeeSelectionV1 {
    try .init(setup(.init(selector: 40, first: schedule, second: certificates)))
  }
  /// Reconcile the same Send-bound Archive without replacing its delivery evidence.
  public func creditedForSendStatus(sendRequestId: Data, original: Data) throws -> KagemushaWalletCallV1 {
    try setup(.init(selector: 42, identity: sendRequestId, first: original)).completion()
  }
}

extension KagemushaWalletExchangeCallV1 {
  /// Issue the receiver's open Request for `amount` under exact PolicyData `schedule` and
  /// `certificates` envelopes, authenticated against current Native policy; neither means the zero
  /// fee. Retries with the same setup id return its exact original bytes.
  public func requestWithFeePolicy(setupId: Data, amount: KagemushaWalletUInt128V1,
    schedule: Data? = nil, certificates: Data? = nil) throws -> KagemushaWalletCallV1 {
    return try task.withUse {
      let result = try setup(.requestWithFeePolicy(identity: setupId, amount: amount,
        schedule: schedule, certificates: certificates))
      _ = try result.original()
      return result
    }
  }
}
