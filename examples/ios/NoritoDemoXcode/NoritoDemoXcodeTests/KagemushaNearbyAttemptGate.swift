// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import Foundation
import Darwin
import IrohaSwift

/// One local clock for all attempt timestamps, including time the phone spends asleep.
/// Different phones and reboots have unrelated origins; never subtract their timestamps.
enum KagemushaNearbyClock {
  static let identity = "darwin-monotonic-raw-including-sleep"
  static func nowNanoseconds() -> UInt64 {
    clock_gettime_nsec_np(CLOCK_MONOTONIC_RAW)
  }
}

/// Local diagnostic ordering and same-clock sample admission; Native owns all money.
struct KagemushaNearbyAttemptGate {
  enum Failure: Error { case order, incomplete, unknownStart, burned }
  enum Role { case sender, receiver }
  enum Record { case hello, authentication, request, payment, credited }
  let role: Role
  let recovery: Bool
  var continuation = false
  private var renewedReview = false
  private var existingOutcome = false
  private var excludedTiming = false
  private(set) var inboundCount = 0
  private(set) var confirmationNanoseconds: UInt64?
  private(set) var terminal = false
  private var paymentReleased = false

  init(role: Role, recovery: Bool, continuation: Bool = false) {
    self.role = role; self.recovery = recovery; self.continuation = continuation
  }
  mutating func admit(_ record: Record) throws {
    guard !terminal else { throw Failure.order }
    let expected: [Record] = role == .sender
      ? [.hello, .authentication, .request, .credited]
      : [.hello, .authentication, .payment]
    guard inboundCount < expected.count, expected[inboundCount] == record else {
      throw Failure.order
    }
    if record == .credited && !paymentReleased { throw Failure.order }
    inboundCount += 1
  }
  mutating func renewed() throws {
    guard !terminal, role == .sender, continuation, !recovery, inboundCount == 3,
      !renewedReview else { throw Failure.order }
    renewedReview = true
  }
  mutating func observedExisting() throws {
    guard !terminal, role == .sender, inboundCount == 3, !paymentReleased else { throw Failure.order }
    existingOutcome = true
  }
  mutating func retainedReview() throws {
    guard !terminal, confirmationNanoseconds != nil, !paymentReleased else { throw Failure.order }
    excludedTiming = true
  }
  mutating func confirmed(at nanoseconds: UInt64) throws {
    guard !terminal, role == .sender, !recovery, inboundCount == 3,
      (!continuation || renewedReview), confirmationNanoseconds == nil, nanoseconds > 0 else { throw Failure.order }
    confirmationNanoseconds = nanoseconds
  }
  mutating func sentPayment() throws {
    guard !terminal, role == .sender, inboundCount == 3, !paymentReleased,
      recovery || existingOutcome || confirmationNanoseconds != nil else { throw Failure.order }
    paymentReleased = true
  }
  static func requireComplete(_ status: Int32) throws {
    guard status == 1 else { throw Failure.incomplete }
  }
  /// Read current evidence from the exact Receive. A collected recovery has no retained
  /// operation output; its permanent credit index supplies the fresh Native original.
  static func receivedCredit(
    status: Int32, recovery: Bool, requestID: Data,
    retainedOutput: (Data) throws -> Void,
    projection: (Data) throws -> ReceivedCredit
  ) throws -> Data {
    guard status == 1 || (recovery && status == 10) else { throw Failure.incomplete }
    if status == 1 { try retainedOutput(requestID) }
    let current = try projection(requestID)
    guard current.isReceiver, !current.isBurned,
      let original = current.original, !original.isEmpty else { throw Failure.incomplete }
    return original
  }
  /// Adapter projection of Native-authenticated DATA; the gate grants no monetary authority.
  struct ReceivedCredit {
    let isReceiver: Bool
    let isBurned: Bool
    let original: Data?
  }
  /// Call after Native verifies the exact Send-bound receipt. Archive progress and a pending
  /// outgoing descriptor do not invalidate delivery; burned evidence cannot pass this positive trial.
  mutating func credited(evidence: KagemushaWalletCreditProjectionV1.Evidence,
                        archive: KagemushaWalletCreditProjectionV1.Archive, corePending _: Bool,
                        arrival: UInt64, verified: UInt64) throws -> (UInt64, UInt64)? {
    guard !terminal, role == .sender, inboundCount == 4 else { throw Failure.order }
    guard archive != .receiver else { throw Failure.incomplete }
    guard evidence != .burned else { throw Failure.burned }
    guard verified >= arrival else { throw Failure.order }
    guard confirmationNanoseconds != nil || recovery || existingOutcome else { throw Failure.unknownStart }
    // A reconnect/restart never invents the missing historical confirmation timestamp.
    guard let start = confirmationNanoseconds else { terminal = true; return nil }
    guard arrival >= start else { throw Failure.order }
    terminal = true
    guard !continuation, !existingOutcome, !excludedTiming else { return nil }
    return (arrival - start, verified - start)
  }
  mutating func receiverDelivered() throws {
    guard !terminal, role == .receiver, inboundCount == 3 else { throw Failure.order }
    terminal = true
  }
  mutating func fail() { terminal = true }
}

/// Main-actor authentication ownership; stale callbacks cannot clear a newer attempt.
struct KagemushaNearbyAuthenticationGate {
  private(set) var activeID: UUID?

  mutating func begin(_ id: UUID) -> Bool {
    guard activeID == nil else { return false }
    activeID = id
    return true
  }
  mutating func finish(_ id: UUID) -> Bool {
    guard activeID == id else { return false }
    activeID = nil
    return true
  }
  mutating func cancel() { activeID = nil }
}
