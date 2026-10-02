// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation

/// Open only the existing Native installer and retain its genuine ordinary account original.
/// The independently registered Native runtime/account producer remains mandatory. This
/// factory accepts no root, account, bootstrap, endpoint or authority projection and grants
/// no enrollment, financial control, hardware qualification or monetary readiness.
public enum KagemushaOrdinaryRuntimeStartupV1 {
  public static func openInitialAccount(storagePath: String) throws
    -> KagemushaOrdinaryNativeAccountSessionV1 {
    try openInitialAccount(storagePath: storagePath,
      openCoordinator: KagemushaNativeCoreCoordinatorAdapterV1.open(storagePath:))
  }

  /// Internal lifetime/refusal seam only. Scripted coordinators never qualify Native custody.
  static func openInitialAccount(storagePath: String,
    openCoordinator: (String) throws -> KagemushaNativeCoreCoordinatorAdapterV1) throws
    -> KagemushaOrdinaryNativeAccountSessionV1 {
    let coordinator = try openCoordinator(storagePath)
    do {
      // Native install already performs initial acquisition for its registered runtime.
      // Phase15 reads its same current W/S; no second startup read or C reservation occurs.
      let selection = try coordinator.currentWalletAccountSelection()
      return KagemushaOrdinaryNativeAccountSessionV1(coordinator: coordinator, selection: selection)
    } catch {
      try? coordinator.close()
      throw error
    }
  }
}

/// One retained genuine SDK coordinator and its opaque ordinary W/S original.
/// Returned originals remain independently guarded by Native. The session owns teardown;
/// callers must keep this session for their whole composition and never reconstruct it from
/// public account strings, transport records or an enrollment acknowledgement.
public final class KagemushaOrdinaryNativeAccountSessionV1: @unchecked Sendable {
  private let coordinator: KagemushaNativeCoreCoordinatorAdapterV1
  private let selection: KagemushaNativeWalletAccountSelectionOriginalV1
  private let lock = NSLock()
  private var unusable = false

  fileprivate init(coordinator: KagemushaNativeCoreCoordinatorAdapterV1,
    selection: KagemushaNativeWalletAccountSelectionOriginalV1) {
    self.coordinator = coordinator
    self.selection = selection
  }

  deinit { try? close() }

  /// Recheck the same Native session and exact W/S originals; failure freezes this owner.
  public func requireCurrent() throws { try guarded {} }

  /// Return the same retained Native coordinator after checking its original W/S owner.
  /// Subsequent Native calls still enforce their own original custody and authority.
  public func originalCoordinator() throws -> KagemushaNativeCoreCoordinatorAdapterV1 {
    try guarded { coordinator }
  }

  /// Return the retained opaque original, never a replacement read or account DTO.
  public func originalAccountSelection() throws -> KagemushaNativeWalletAccountSelectionOriginalV1 {
    try guarded { selection }
  }

  /// Freeze locally before teardown. Repeated close does nothing, including after uncertainty.
  /// Reopening/recovery is owned by the genuine Native lifecycle, never by this session.
  public func close() throws {
    lock.lock()
    defer { lock.unlock() }
    guard !unusable else { return }
    unusable = true
    try coordinator.close()
  }

  private func guarded<T>(_ action: () -> T) throws -> T {
    lock.lock()
    defer { lock.unlock() }
    guard !unusable else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    do {
      try selection.requireCurrent()
      return action()
    } catch {
      unusable = true
      try? coordinator.close()
      throw error
    }
  }
}
