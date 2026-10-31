// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import Foundation
import IrohaSwift

/// Synchronous mailbox ingress for the exact published wallet. It records Native exchange
/// occupancy and local task acceptance before the command is placed on the device actor.
final class KagemushaProbeExchangeIngress: @unchecked Sendable {
  private let lock = NSLock()
  private var wallet: KagemushaWalletV1?
  private var exchange: KagemushaWalletExchangeV1?
  private var opening = false
  private var retired = true

  func publish(_ wallet: KagemushaWalletV1) throws {
    lock.lock(); defer { lock.unlock() }
    guard self.wallet == nil, exchange == nil, !opening else { throw EnrollmentProbeFailure.lifecycle }
    self.wallet = wallet; retired = false
  }

  func begin() throws {
    lock.lock()
    guard !retired, !opening, exchange == nil, let selected = wallet else {
      lock.unlock(); throw EnrollmentProbeFailure.lifecycle
    }
    opening = true; lock.unlock()
    do {
      let issued = try selected.beginExchange()
      lock.lock()
      let current = !retired && wallet === selected
      if current { exchange = issued }
      opening = false; lock.unlock()
      guard current else {
        issued.requestEnd(); _ = try issued.end()
        throw EnrollmentProbeFailure.lifecycle
      }
    } catch {
      lock.lock(); opening = false; lock.unlock(); throw error
    }
  }

  func admit() throws -> KagemushaWalletExchangeCallV1 {
    lock.lock()
    let selected = retired ? nil : exchange
    lock.unlock()
    guard let selected else { throw EnrollmentProbeFailure.lifecycle }
    return try selected.admit()
  }

  func requestEnd() throws {
    lock.lock(); let selected = exchange; lock.unlock()
    guard let selected else { throw EnrollmentProbeFailure.lifecycle }
    selected.requestEnd()
  }

  func end() async throws -> KagemushaWalletExchangeEndV1 {
    let selected = try selectedExchange()
    selected.requestEnd()
    // Detached cleanup is joined even if the mailbox task is cancelled.
    let result = try await Task.detached { try selected.end() }.value
    if result != .ending { clear(selected) }
    return result
  }

  private func selectedExchange() throws -> KagemushaWalletExchangeV1 {
    lock.lock(); defer { lock.unlock() }
    guard let exchange else { throw EnrollmentProbeFailure.lifecycle }
    return exchange
  }

  private func clear(_ selected: KagemushaWalletExchangeV1) {
    lock.lock(); defer { lock.unlock() }
    if exchange === selected { exchange = nil }
  }

  func invalidate() {
    lock.lock(); retired = true; let selected = exchange; lock.unlock()
    selected?.requestEnd()
  }

  func closed() {
    lock.lock(); retired = true; wallet = nil; let retained = exchange; exchange = nil; lock.unlock()
    // Do not destroy a Native cleanup owner while holding the ingress lock.
    withExtendedLifetime(retained) {}
  }
}

extension KagemushaMonetaryProbeManifest {
  /// Only commands with fresh mutations need executable admission. Recovery and observations
  /// remain Native-selected; this table never decides idle eligibility.
  static let exchangeActions: Set<String> = [
    "bootstrap", "activation", "retain-activation", "activation-finality",
    "prepare-load", "load", "ledger-finality", "epoch-boundary", "request", "review-send",
    "review-unload", "continue-review-send", "continue-review-unload", "retry-reviewed", "decide-review", "receive", "observe-credited", "unload-finality",
    "refresh-time", "begin-time", "finish-time", "nearby-prepare", "nearby-send",
    "nearby-recover-send", "nearby-continue-send", "nearby-receive", "nearby-recover-receive", "qr-send", "qr-receive",
    "qr-recover-send", "qr-continue-send", "qr-recover-receive",
  ]
}
