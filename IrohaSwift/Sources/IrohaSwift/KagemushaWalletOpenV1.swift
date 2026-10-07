import Foundation
import NoritoBridge

/// Exact original owner frames. Their contents are authenticated exclusively by Native.
public struct KagemushaWalletOpenOriginalsV1: Sendable {
  let originals: [Data]
  public init(credential: Data, enrollmentCertificates: Data, account: Data, assetScope: Data) throws {
    let values = [credential, enrollmentCertificates, account, assetScope]
    guard zip(values, [1_024, 10_000, 4_096, 1_024]).allSatisfy({ !$0.0.isEmpty && $0.0.count <= $0.1 })
    else { throw KagemushaWalletErrorV1.invalidInput }
    originals = values
  }
  func withRequest<T>(_ body: (UnsafePointer<connect_norito_kagemusha_wallet_open_request_v1>) throws -> T) rethrows -> T {
    try originals[0].withUnsafeBytes { a in try originals[1].withUnsafeBytes { b in
      try originals[2].withUnsafeBytes { c in try originals[3].withUnsafeBytes { d in
        var request = connect_norito_kagemusha_wallet_open_request_v1()
        request.credential = a.bindMemory(to: UInt8.self).baseAddress; request.credential_length = a.count
        request.certificates = b.bindMemory(to: UInt8.self).baseAddress; request.certificates_length = b.count
        request.account = c.bindMemory(to: UInt8.self).baseAddress; request.account_length = c.count
        request.asset = d.bindMemory(to: UInt8.self).baseAddress; request.asset_length = d.count
        return try body(&request)
      }}
    }}
  }
}

/// Runtime already provisioned by the embedding app's trusted native startup loader.
/// This handle selects retained native custody; it supplies no trust pins or proof verdicts.
public final class KagemushaWalletRuntimeV1: @unchecked Sendable {
  private let lock = NSLock()
  private var owner: UInt64
  private let driver: KagemushaWalletNativeDriverV1
  private let pending = KagemushaWalletAdmissionLifetimeV1<KagemushaWalletPendingOpenV1>()
  public init(nativeRuntimeHandle: UInt64) throws {
    guard nativeRuntimeHandle > 0 && nativeRuntimeHandle <= UInt64(Int64.max) else { throw KagemushaWalletErrorV1.invalidInput }
    owner = nativeRuntimeHandle
    driver = try KagemushaWalletNativeDriverV1()
  }
  /// Reconcile these originals, retaining the same challenge and pending owner on retry.
  public func begin(_ originals: KagemushaWalletOpenOriginalsV1) throws -> KagemushaWalletPendingOpenV1 {
    lock.lock(); defer { lock.unlock() }
    guard owner != 0 else { throw KagemushaWalletErrorV1.closed }
    let result = try driver.result { out in originals.withRequest { driver.openBegin(owner, $0, out) } }
    guard result.status == 15 && result.sequenceLow == owner else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return try pending.select(result.bytes) { identity in
      KagemushaWalletPendingOpenV1(runtime: self, identity: identity, challenge: result.bytes)
    }
  }
  // Called exclusively under this runtime's lock.
  private func finishNative(_ signature: Data) throws -> KagemushaWalletV1 {
    guard owner != 0 else { throw KagemushaWalletErrorV1.closed }
    let result = try driver.result { out in signature.withUnsafeBytes { driver.openFinish(owner, $0.bindMemory(to: UInt8.self).baseAddress, $0.count, out) } }
    guard result.status == 16 && result.sequenceLow == owner else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    let wallet = KagemushaWalletV1(handle: owner, driver: driver)
    owner = 0
    return wallet
  }
  fileprivate func finish(_ identity: KagemushaWalletAdmissionIdentityV1, signature: Data) throws -> KagemushaWalletV1 {
    lock.lock(); defer { lock.unlock() }
    return try pending.finish(identity) { try finishNative(signature) }
  }
  // Called exclusively under this runtime's lock.
  private func cancelNative() throws {
    guard owner != 0 else { throw KagemushaWalletErrorV1.closed }
    try KagemushaWalletNativeDriverV1.check(driver.openCancel(owner))
  }
  fileprivate func cancel(_ identity: KagemushaWalletAdmissionIdentityV1) throws {
    lock.lock(); defer { lock.unlock() }
    try pending.abandon(identity) { try cancelNative() }
  }
  /// Discard a pending challenge after interrupted begin delivery, retaining native custody.
  public func cancelPendingOpen() throws {
    lock.lock(); defer { lock.unlock() }
    try pending.complete { try cancelNative() }
  }
  /// Retry the retained challenge or recover interrupted finish delivery/registration.
  public func retryOpenCompletion(accountSignature: Data) throws -> KagemushaWalletV1 {
    lock.lock(); defer { lock.unlock() }
    return try pending.complete { try finishNative(accountSignature) }
  }
  /// Successful finish transfers ownership to the wallet; closing this runtime then does nothing.
  public func close() throws {
    lock.lock(); defer { lock.unlock() }
    let value = owner; owner = 0
    pending.clear()
    if value != 0 { try KagemushaWalletNativeDriverV1.check(driver.close(value)) }
  }
  deinit { try? close() }
}

/// Exact retained 32-byte account challenge. The existing Ed25519 account signs these bytes.
public final class KagemushaWalletPendingOpenV1: @unchecked Sendable {
  private let runtime: KagemushaWalletRuntimeV1
  private let identity: KagemushaWalletAdmissionIdentityV1
  public let challenge: Data
  fileprivate init(runtime: KagemushaWalletRuntimeV1, identity: KagemushaWalletAdmissionIdentityV1, challenge: Data) {
    self.runtime = runtime; self.identity = identity; self.challenge = challenge
  }
  /// Ordinary refusal retains this pending owner for retry; success transfers ownership.
  public func finish(accountSignature: Data) throws -> KagemushaWalletV1 {
    try runtime.finish(identity, signature: accountSignature)
  }
  /// Abandon this challenge without admitting ownership or releasing native custody.
  public func cancel() throws { try runtime.cancel(identity) }
  deinit { try? cancel() }
}

// Managed lifetime only. The token cannot select a native handle, trust pin or verdict.
internal final class KagemushaWalletAdmissionIdentityV1 {}
internal final class KagemushaWalletAdmissionLifetimeV1<P: AnyObject> {
  // Pending owns its Runtime. A weak cache prevents the reverse edge from creating an ARC cycle.
  // The separately retained identity also makes deinit cancellation safe after weak zeroing.
  private weak var current: P?
  private var identity: KagemushaWalletAdmissionIdentityV1?
  private var challenge: Data?

  // All production calls are serialized by the actual Runtime lock.
  func select(_ original: Data, create: (KagemushaWalletAdmissionIdentityV1) -> P) throws -> P {
    guard original.count == 32 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    let selectedIdentity: KagemushaWalletAdmissionIdentityV1
    if identity != nil {
      guard challenge == original else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      if let selected = current { return selected }
      // A prior Pending deinit may already be waiting on the Runtime lock. Give the
      // replacement wrapper a fresh managed identity so that old cancellation is stale.
      // The exact retained Native challenge remains unchanged.
      selectedIdentity = KagemushaWalletAdmissionIdentityV1()
    } else {
      selectedIdentity = KagemushaWalletAdmissionIdentityV1()
    }
    let selected = create(selectedIdentity)
    challenge = original
    identity = selectedIdentity
    current = selected
    return selected
  }

  func finish<T>(_ selected: KagemushaWalletAdmissionIdentityV1, action: () throws -> T) throws -> T {
    guard identity === selected else { throw KagemushaWalletErrorV1.closed }
    return try complete(action)
  }

  func abandon(_ selected: KagemushaWalletAdmissionIdentityV1, action: () throws -> Void) throws {
    // An old or completed wrapper must never cancel a subsequent challenge.
    if identity === selected { try complete(action) }
  }

  func complete<T>(_ action: () throws -> T) rethrows -> T {
    let result = try action()
    clear()
    return result
  }

  func clear() {
    current = nil
    identity = nil
    challenge = nil
  }
}
