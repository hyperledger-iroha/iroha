import Foundation

/// Native cleanup observation for one exact exchange; it grants no monetary permission.
public enum KagemushaWalletExchangeEndV1: UInt32, Sendable {
  case ended = 0, ending = 1, alreadyEnded = 2
}

/// Local accepted-task lifetime only. Native owns exchange occupancy and every admission gate.
/// No lock is held during a Native call, worker wait, actor hop or caller callback.
final class KagemushaWalletExchangeLifetimeV1: @unchecked Sendable {
  private let condition = NSCondition()
  private var accepting = true
  private var children = 0
  private var ended = false
  private final class EndAttempt {
    var result: Result<KagemushaWalletExchangeEndV1, Error>?
  }
  private var joining: EndAttempt?
  private let endNative: @Sendable () throws -> KagemushaWalletExchangeEndV1

  init(endNative: @escaping @Sendable () throws -> KagemushaWalletExchangeEndV1) {
    self.endNative = endNative
  }

  /// Record acceptance on the caller before placing its work on any queue.
  func admit() throws -> KagemushaWalletExchangeTaskV1 {
    condition.lock(); defer { condition.unlock() }
    guard accepting, children < Int.max else { throw KagemushaWalletErrorV1.closed }
    children += 1
    return .init(owner: self)
  }

  fileprivate func completed() {
    condition.lock()
    precondition(children > 0)
    children -= 1
    condition.broadcast(); condition.unlock()
  }

  func requestEnd() {
    condition.lock(); accepting = false; condition.unlock()
  }

  /// Fence immediately, then join accepted tasks before Native end. Invoke off MainActor,
  /// after finishing this caller's task. A failed attempt keeps the same Native identity.
  func end() throws -> KagemushaWalletExchangeEndV1 {
    condition.lock()
    accepting = false
    if ended { condition.unlock(); return .alreadyEnded }
    if let current = joining {
      // This caller owns this attempt's result even if a later explicit retry starts first.
      while current.result == nil { condition.wait() }
      let result = current.result!
      condition.unlock(); return try result.get()
    }
    let current = EndAttempt()
    joining = current
    while children != 0 { condition.wait() }
    condition.unlock()
    let result = Result { try endNative() }
    condition.lock()
    if case .success(let value) = result { ended = value != .ending }
    current.result = result; joining = nil
    condition.broadcast(); condition.unlock()
    return try result.get()
  }
}

/// One accepted task. Aliases share the same finish fence; work already entered remains owned.
final class KagemushaWalletExchangeTaskV1: @unchecked Sendable {
  private let lock = NSLock()
  private var owner: KagemushaWalletExchangeLifetimeV1?
  private var finishing = false
  private var executing = 0

  fileprivate init(owner: KagemushaWalletExchangeLifetimeV1) { self.owner = owner }

  func withUse<T>(_ action: () throws -> T) throws -> T {
    try beginUse()
    defer { endUse() }
    return try action()
  }

  func withUse<T>(_ action: () async throws -> T) async throws -> T {
    try beginUse()
    defer { endUse() }
    return try await action()
  }

  private func beginUse() throws {
    lock.lock(); defer { lock.unlock() }
    guard !finishing, owner != nil, executing < Int.max else {
      throw KagemushaWalletErrorV1.closed
    }
    executing += 1
  }

  private func endUse() {
    lock.lock()
    precondition(executing > 0)
    executing -= 1
    let release = finishing && executing == 0 ? owner : nil
    if release != nil { owner = nil }
    lock.unlock()
    release?.completed()
  }

  /// Nonblocking: an operation already using the task releases its last ownership on return.
  func finish() {
    lock.lock()
    finishing = true
    let release = executing == 0 ? owner : nil
    if release != nil { owner = nil }
    lock.unlock()
    release?.completed()
  }

  deinit { finish() }
}
