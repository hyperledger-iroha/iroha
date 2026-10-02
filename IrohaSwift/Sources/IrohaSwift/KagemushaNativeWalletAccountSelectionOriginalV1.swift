// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation

/// Exact original W/S data from one retained installed Native account session.
/// No public byte, DTO or account-ID constructor can recreate this holder. Each access
/// rechecks its original coordinator and session. It grants no signing or monetary authority.
public final class KagemushaNativeWalletAccountSelectionOriginalV1: @unchecked Sendable {
  private let bridge: KagemushaCoreCoordinatorBridgeV1
  private let original: [Data]
  private let lock = NSLock()
  private var unusable = false

  private init(bridge: KagemushaCoreCoordinatorBridgeV1, fields: [Data]) {
    self.bridge = bridge
    original = fields.map { Data($0) }
  }

  static func fromNative(bridge: KagemushaCoreCoordinatorBridgeV1) throws
    -> KagemushaNativeWalletAccountSelectionOriginalV1 {
    let fields = try bridge.invoke(.preparedOrdinaryAppIdentity,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(15)])
    let result = KagemushaNativeWalletAccountSelectionOriginalV1(bridge: bridge, fields: fields)
    try result.requireCurrent()
    return result
  }

  /// Return the original wallet W only after the same Native owner rechecks it.
  public func walletAccountId() throws -> String { try field(1) }

  /// Return the original Ed25519 multisig member S after its Native owner rechecks it.
  public func signatoryAccountId() throws -> String { try field(2) }

  /// Require the exact original session, wallet and signatory from the retained owner.
  public func requireCurrent() throws {
    lock.lock()
    defer { lock.unlock() }
    try recheckLocked()
  }

  func requireForCoordinator(_ other: KagemushaCoreCoordinatorBridgeV1) throws {
    lock.lock()
    defer { lock.unlock() }
    guard bridge === other else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("original wallet selection belongs to another Native coordinator")
    }
    try recheckLocked()
  }

  private func field(_ index: Int) throws -> String {
    lock.lock()
    defer { lock.unlock() }
    try recheckLocked()
    guard let value = String(data: original[index], encoding: .utf8) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("original Native wallet account text differs")
    }
    try recheckLocked()
    return value
  }

  private func recheckLocked() throws {
    guard !unusable else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    let current: [Data]
    do {
      current = try bridge.invoke(.preparedOrdinaryAppIdentity,
        fields: [KagemushaCoreCoordinatorFrameV1.u32(15)])
    } catch {
      unusable = true
      throw error
    }
    guard original == current else {
      unusable = true
      try? bridge.close()
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("Native current wallet/session substituted the original W/S selection")
    }
  }
}

extension KagemushaCoreCoordinatorBridgeV1 {
  /// Copy W/S data from the installed account session using the fixed method21 phase15.
  /// Native validates canonical multisig membership. This call reserves and signs nothing.
  public func currentWalletAccountSelection() throws
    -> KagemushaNativeWalletAccountSelectionOriginalV1 {
    try KagemushaNativeWalletAccountSelectionOriginalV1.fromNative(bridge: self)
  }
}
