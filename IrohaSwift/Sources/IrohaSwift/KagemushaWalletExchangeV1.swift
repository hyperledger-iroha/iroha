import Foundation
import NoritoBridge

extension KagemushaWalletV1 {
  /// Record requested exchange occupancy synchronously before UI, actor or worker enqueue.
  public func beginExchange() throws -> KagemushaWalletExchangeV1 {
    let owner = try handle()
    do {
      let result = try driver.controlResult { driver.beginExchange(owner, $0) }
      guard result.status == 60 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return .init(wallet: self, token: result.sequenceLow)
    } catch KagemushaWalletErrorV1.invalidNativeOutput {
      // An undisclosed capability cannot be orphaned after malformed delivery.
      try close()
      throw KagemushaWalletErrorV1.invalidNativeOutput
    }
  }
  func endExchange(_ token: UInt64) throws -> KagemushaWalletExchangeEndV1 {
    let owner = try cleanupHandle()
    let result = try driver.controlResult { driver.endExchange(owner, token, $0) }
    guard result.status == 61, let end = KagemushaWalletExchangeEndV1(rawValue: result.detail)
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return end
  }
}

/// A Native-recorded logical exchange, retained across review, UI and carrier waits.
/// Reserve it synchronously before enqueueing work. Native owns all admission decisions.
public final class KagemushaWalletExchangeV1: @unchecked Sendable {
  let wallet: KagemushaWalletV1
  let token: UInt64
  private let lifetime: KagemushaWalletExchangeLifetimeV1
  init(wallet: KagemushaWalletV1, token: UInt64) {
    self.wallet = wallet; self.token = token
    lifetime = .init { try wallet.endExchange(token) }
  }
  /// Accept a task before its actor, worker, UI or transport enqueue. Finish it after teardown.
  public func admit() throws -> KagemushaWalletExchangeCallV1 {
    _ = try wallet.handle()
    return .init(exchange: self, task: try lifetime.admit())
  }
  /// Fence new tasks and join accepted work before Native end. Call off MainActor and only
  /// after finishing this caller's task. Failure retains this exact exchange for retry.
  public func end() throws -> KagemushaWalletExchangeEndV1 { try lifetime.end() }
  /// Fence admission synchronously before enqueueing an off-main end/join task.
  /// Exact-origin cleanup remains available; this cannot admit executable work.
  public func requestEnd() { lifetime.requestEnd() }
  deinit {
    // Every accepted task strongly retains this exchange, so no task can remain here.
    do { _ = try lifetime.end() } catch { try? wallet.close() }
  }
}

/// One accepted task on an exact exchange. Copying this reference shares its finish fence.
/// Fresh mutations exist only here; a wallet observation cannot create executable admission.
public final class KagemushaWalletExchangeCallV1: @unchecked Sendable {
  public let exchange: KagemushaWalletExchangeV1
  let task: KagemushaWalletExchangeTaskV1
  var wallet: KagemushaWalletV1 { exchange.wallet }
  var driver: KagemushaWalletNativeDriverV1 { wallet.driver }
  var deletionGate: KagemushaWalletDeletionGateV1 { wallet.deletionGate }
  init(exchange: KagemushaWalletExchangeV1, task: KagemushaWalletExchangeTaskV1) {
    self.exchange = exchange; self.task = task
  }
  /// Fence future use; already running Native work remains owned until its return.
  public func finish() { task.finish() }
  deinit { task.finish() }
  func handle() throws -> UInt64 { try wallet.handle() }
  func setup(_ input: KagemushaWalletSetupInputV1) throws -> KagemushaWalletCallV1 {
    let owner = try handle()
    return try driver.result { out in
      input.withRequest { driver.setup(owner, exchange.token, $0, out) }
    }
  }
  private func execute(_ input: KagemushaWalletOperationInputV1) throws -> KagemushaWalletCallV1 {
    let value = try handle()
    return try driver.result { out in
      input.withRequest { request in driver.execute(value, exchange.token, request, out) }
    }.completion()
  }

  /// Complete restart-safe native enrollment Bootstrap.
  public func bootstrap() throws -> KagemushaWalletCallV1 {
    return try task.withUse { try setup(.init(selector: 0)).completion() }
  }
  /// Exact retained Activate frame for ledger submission; this is not activation confirmation.
  public func activation() throws -> Data {
    return try task.withUse {
      let result = try setup(.init(selector: 15))
      guard result.status == 17 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return result.bytes
    }
  }
  /// Exact native signed CloseLoads frame for ledger submission; this is not closure confirmation.
  /// Reuse requestId for exact retries; use a fresh id after a preissued Load and a new complete consumer.
  public func closeLoads(requestId: Data) throws -> Data {
    return try task.withUse {
      let result = try setup(.init(selector: 19, identity: requestId))
      guard result.status == 30 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return result.bytes
    }
  }
  /// Verify one bounded next Sumeragi finality original; native persists its selected prefix.
  public func ingestLedgerFinality(_ original: Data) throws -> KagemushaWalletLedgerProgressV1 {
    return try task.withUse {
      try .init(setup(.init(selector: 23, first: original)))
    }
  }
  /// Verify one exact incumbent boundary and atomically retain its successor committee.
  public func ingestEpochBoundary(expectedEpoch: UInt64, original: Data) throws -> KagemushaWalletEpochProgressV1 {
    return try task.withUse {
      try .init(setup(.init(selector: 53, amount: .init(low: expectedEpoch, high: 0), first: original)))
    }
  }
  /// Authenticate exact payout-row and full World originals against native's selected tip.
  /// Only successful durable acknowledgement releases the separate retained fee claim.
  public func acknowledgeFeePayout(creditId: Data, world: Data, payout: Data) throws {
    return try task.withUse {
      let result = try setup(.init(selector: 25, identity: creditId, first: world, second: payout))
      guard result.status == 35 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    }
  }
  /// Issue the receiver's open Request for `amount` from native current policy, with the optional
  /// fee schedule and its signer certificate. It names no payer: the payer's Native completes it.
  /// Retries with the same setup id return its exact original bytes.
  public func request(setupId: Data, amount: KagemushaWalletUInt128V1, feeSchedule: Data? = nil,
    feeCertificate: Data? = nil) throws -> KagemushaWalletCallV1 {
    return try task.withUse {
      let result = try setup(.request(identity: setupId, amount: amount,
        feeSchedule: feeSchedule, feeCertificate: feeCertificate))
      _ = try result.original()
      return result
    }
  }
  /// Begin a direct issuer exchange. Only the fresh nonce leaves native clock custody.
  public func beginTimeExchange() throws -> KagemushaWalletTimeExchangeV1 {
    return try task.withUse {
      try setup(.init(selector: 4)).exchange(origin: exchange)
    }
  }
  /// Consume the native exchange token and retain its verified anchor before TimeAnchor refresh.
  /// Invalid local bounds leave the challenge usable; dispatch consumes it even on failure.
  public func finishTimeExchange(_ exchange: KagemushaWalletTimeExchangeV1, anchor: Data, certificate: Data) throws -> KagemushaWalletCallV1 {
    return try task.withUse {
      let input = try KagemushaWalletSetupInputV1(selector: 5,
        token: exchange.tokenFor(origin: self.exchange), first: anchor, second: certificate)
      _ = try handle()
      try exchange.consume(origin: self.exchange)
      return try setup(input).timeRetained()
    }
  }
  /// Load exact ordinary-ledger receipt and native BLS finality originals.
  public func load(requestId: Data, receipt: Data, finality: Data) throws -> KagemushaWalletCallV1 {
    return try task.withUse {
      try execute(KagemushaWalletOperationInputV1(requestId: requestId, selector: 0, first: receipt, second: finality))
    }
  }
  /// Authenticate the fully folded Send source before fresh hardware UI confirmation. Native
  /// completes the receiver's open Request with this wallet's account and next send ordinal.
  public func reviewSend(requestId: Data, request: Data, destinationAccountOriginal: Data,
    expectation: KagemushaWalletReviewExpectationV1) throws -> KagemushaWalletReviewDecisionV1 {
    return try task.withUse {
      guard !request.isEmpty, request.count <= 10_034,
        !destinationAccountOriginal.isEmpty, destinationAccountOriginal.count <= 4_096
      else { throw KagemushaWalletErrorV1.invalidInput }
      return try review(requestId: requestId, expectation: expectation, selector: 1, amount: .init(low: 0, high: 0), first: request,
        second: destinationAccountOriginal, expected: .send)
    }
  }
  /// Native selects the durably issued Request using the Payment's canonical digest.
  public func receive(requestId: Data, payment: Data, payerCredential: Data, certificates: Data) throws -> KagemushaWalletCallV1 {
    return try task.withUse {
      try execute(KagemushaWalletOperationInputV1(requestId: requestId, selector: 2, first: payment, second: payerCredential, third: certificates))
    }
  }
  /// Forward the exact Payment and the payer companion that travelled beside it. Native checks
  /// the companion against the Payment and performs ordinary Receive authentication; the
  /// companion supplies no monetary authority.
  public func receiveWithCompanion(requestId: Data, payment: Data, companion: Data) throws -> KagemushaWalletCallV1 {
    return try task.withUse {
      try execute(KagemushaWalletOperationInputV1(requestId: requestId, selector: 10, first: payment, second: companion))
    }
  }
  /// Apply signed policy originals; Native derives effective values and local map changes.
  public func refresh(requestId: Data, kind: KagemushaWalletRefreshKindV1, update: Data, certificates: Data) throws -> KagemushaWalletCallV1 {
    return try task.withUse {
      try execute(KagemushaWalletOperationInputV1(requestId: requestId, selector: kind.rawValue, first: update, second: certificates))
    }
  }
  /// Install a signed blacklist locally; a replacement's list version must increase and
  /// reinstalling the held list is idempotent. The wallet then refuses to pay a listed receiver
  /// and to accept money from a listed payer (`blockedStatus`). Nothing is sent to
  /// peers or proved; this is no wallet operation and has no retry identity. Complete carries the
  /// held list's 32-byte object digest.
  public func installBlacklist(list: Data, certificates: Data) throws -> KagemushaWalletCallV1 {
    return try task.withUse {
      try execute(KagemushaWalletOperationInputV1(requestId: Data(count: 32), selector: 5, first: list, second: certificates))
    }
  }
  /// Authenticate exact Unload gross debit and optional signed charge before fresh confirmation.
  public func reviewUnload(requestId: Data, expectation: KagemushaWalletReviewExpectationV1, amount: KagemushaWalletUInt128V1, quote: Data? = nil, certificates: Data? = nil,
    chargeBeneficiary: Data? = nil) throws -> KagemushaWalletReviewDecisionV1 {
    return try task.withUse {
      guard amount.low != 0 || amount.high != 0, (quote == nil) == (certificates == nil),
        (quote == nil) == (chargeBeneficiary == nil),
        quote.map({ !$0.isEmpty && $0.count <= 1_024 }) ?? true,
        certificates.map({ !$0.isEmpty && $0.count <= 10_034 }) ?? true,
        chargeBeneficiary.map({ !$0.isEmpty && $0.count <= 4_096 }) ?? true else { throw KagemushaWalletErrorV1.invalidInput }
      let companion: Data
      if let certificates, let chargeBeneficiary {
        companion = try KagemushaWalletUnloadChargeReviewV1.encode(certificates: certificates, beneficiary: chargeBeneficiary)
      } else { companion = Data() }
      return try review(requestId: requestId, expectation: expectation, selector: 8, amount: amount, first: quote ?? Data(), second: companion, expected: .unload)
    }
  }
  private func review(requestId: Data, expectation: KagemushaWalletReviewExpectationV1,
    selector: UInt32, amount: KagemushaWalletUInt128V1, first: Data, second: Data,
    expected: KagemushaWalletReviewProjectionV1.Kind) throws -> KagemushaWalletReviewDecisionV1 {
    let input = try KagemushaWalletReviewInputV1(requestID: requestId, expectation: expectation,
      selector: selector, amount: amount, first: first, second: second)
    let handle = try handle()
    var token: UInt64 = 0
    do {
      let reply = try driver.reviewResult { out in
        input.withRequest { request in
          let status = driver.review(handle, exchange.token, request, out)
          if out.pointee.status == 18 { token = out.pointee.sequence_low }
          return status
        }
      }
      return try reply.decision(origin: exchange, requestID: requestId, expected: expected)
    } catch KagemushaWalletErrorV1.invalidNativeOutput {
      let primary = KagemushaWalletErrorV1.invalidNativeOutput
      do {
        if token == 0 { try wallet.close() }
        else { try KagemushaWalletNativeDriverV1.check(driver.discardReview(handle, exchange.token, token)) }
      } catch {
        // A token delivery error cannot orphan the actual Native capability. Close the owner;
        // a failed retirement is sticky and retains its actual callbacks/custody resource.
        do { try wallet.close() } catch { throw error }
      }
      throw primary
    }
  }
  /// Consume the exact reviewed Native action after fresh hardware confirmation. Native rechecks its whole source.
  public func executeReviewed(_ review: KagemushaWalletReviewV1) throws -> KagemushaWalletReviewedExecutionResultV1 {
    return try task.withUse {
      let handle = try handle(), token = try review.consume(origin: exchange)
      return try KagemushaWalletReviewedExecutionV1.run(execute: {
        let result = try driver.result { driver.executeReviewed(handle, exchange.token, token, $0) }
        let decoded = try KagemushaWalletReviewedExecutionResultV1(result)
        if case .retainedReview = decoded {
          try review.retainAfterPreflight(origin: exchange, consumed: token, result: result)
        }
        return decoded
      }, discard: { driver.discardReview(handle, exchange.token, token) }, close: wallet.close)
    }
  }
  /// Enter Retiring under native folded-state checks.
  public func retire(requestId: Data) throws -> KagemushaWalletCallV1 {
    return try task.withUse {
      try execute(KagemushaWalletOperationInputV1(requestId: requestId, selector: 9))
    }
  }
  /// Obtain native-selected display DATA for explicit destructive confirmation.
  public func reviewCustodyDeletion() throws -> KagemushaWalletDeletionReviewV1 {
    return try task.withUse {
      try deletionGate.review(exchange: exchange) { try setup(.init(selector: 48)) }
    }
  }
  /// Permanently destroy payment-key custody. Display the review warning first.
  /// Any failure freezes ordinary calls until resume proves a definitive outcome.
  public func destructivelyDeleteCustody(_ review: KagemushaWalletDeletionReviewV1) throws -> Data {
    return try task.withUse {
      try deletionGate.confirm(review, exchange: exchange) { token in try wallet.deletionSetup(.init(selector: 49, token: token), exchangeID: exchange.token) }
    }
  }
}

extension KagemushaWalletExchangeV1 {
  private func consumedCleanup<Value>(_ action: () throws -> Value) throws -> Value {
    try KagemushaWalletConsumedCleanupV1.run(cleanup: action, close: wallet.close,
      retain: { primary, cleanup in
        KagemushaWalletInstalledRuntimeV1.retain(primary, cleanup: cleanup, resource: self.wallet)
      })
  }
  private func setup(_ input: KagemushaWalletSetupInputV1) throws -> KagemushaWalletCallV1 {
    let owner = try wallet.cleanupHandle()
    return try wallet.driver.result { out in
      input.withRequest { wallet.driver.setup(owner, token, $0, out) }
    }
  }
  /// Discard an unanswered native time exchange without accepting or refreshing an anchor.
  public func cancelTimeExchange(_ exchange: KagemushaWalletTimeExchangeV1) throws {
    let input = try KagemushaWalletSetupInputV1(selector: 6,
      token: exchange.tokenFor(origin: self))
    _ = try wallet.cleanupHandle()
    try exchange.consume(origin: self)
    try consumedCleanup {
      let result = try setup(input)
      guard result.status == 6, result.sequenceLow == 0, result.sequenceHigh == 0,
        result.detail == 0 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    }
  }
  /// Worker-only discard; it creates no intent, signature, proof or debit.
  public func discardReview(_ review: KagemushaWalletReviewV1) throws {
    let handle = try wallet.cleanupHandle(), token = try review.consume(origin: self)
    try consumedCleanup {
      try KagemushaWalletNativeDriverV1.check(wallet.driver.discardReview(handle, self.token, token))
    }
  }
  /// Discard an unused review; this cannot undo a deletion attempt.
  public func discardCustodyDeletionReview(_ review: KagemushaWalletDeletionReviewV1) throws {
    try wallet.deletionGate.discard(review, exchange: self) { token in
      try consumedCleanup {
        let result = try setup(.init(selector: 51, token: token))
        guard result.status == 55 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
        return result
      }
    }
  }
}
