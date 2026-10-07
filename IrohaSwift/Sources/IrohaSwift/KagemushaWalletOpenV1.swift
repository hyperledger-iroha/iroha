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
  public init(nativeRuntimeHandle: UInt64) throws {
    guard nativeRuntimeHandle > 0 && nativeRuntimeHandle <= UInt64(Int64.max) else { throw KagemushaWalletErrorV1.invalidInput }
    owner = nativeRuntimeHandle
    driver = try KagemushaWalletNativeDriverV1()
  }
  /// Begin with original issuer/account frames. Native reconciles before sampling its challenge.
  public func begin(_ originals: KagemushaWalletOpenOriginalsV1) throws -> KagemushaWalletPendingOpenV1 {
    lock.lock(); defer { lock.unlock() }
    guard owner != 0 else { throw KagemushaWalletErrorV1.closed }
    let result = try driver.result { out in originals.withRequest { driver.openBegin(owner, $0, out) } }
    guard result.status == 15 && result.sequenceLow == owner else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return KagemushaWalletPendingOpenV1(runtime: self, challenge: result.bytes)
  }
  /// Begin strict account admission with exact Native-retained E5/E6 originals after enrollment handoff.
  public func beginEnrolled() throws -> KagemushaWalletPendingOpenV1 {
    lock.lock(); defer { lock.unlock() }
    guard owner != 0 else { throw KagemushaWalletErrorV1.closed }
    let input = try KagemushaWalletEnrollmentInputV1(8)
    let result = try driver.result { out in input.withRequest { driver.enrollment(owner,$0,out) } }
    guard result.status == 15 && result.sequenceLow == owner else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return KagemushaWalletPendingOpenV1(runtime:self,challenge:result.bytes)
  }
  fileprivate func finish(_ signature: Data) throws -> KagemushaWalletV1 {
    lock.lock(); defer { lock.unlock() }
    guard owner != 0 else { throw KagemushaWalletErrorV1.closed }
    let result = try driver.result { out in signature.withUnsafeBytes { driver.openFinish(owner, $0.bindMemory(to: UInt8.self).baseAddress, $0.count, out) } }
    guard result.status == 16 && result.sequenceLow == owner else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    let wallet = KagemushaWalletV1(handle: owner, driver: driver)
    owner = 0
    return wallet
  }
  /// Discard a pending challenge after interrupted begin delivery, retaining native custody.
  public func cancelPendingOpen() throws {
    lock.lock(); defer { lock.unlock() }
    guard owner != 0 else { throw KagemushaWalletErrorV1.closed }
    try KagemushaWalletNativeDriverV1.check(driver.openCancel(owner))
  }
  /// Recover exact successful finish after interrupted output or registration delivery.
  /// A rejected authorization requires a fresh begin, rather than reuse of its challenge.
  public func retryOpenCompletion(accountSignature: Data) throws -> KagemushaWalletV1 {
    try finish(accountSignature)
  }
  /// Close only unadmitted custody; a successful finish transfers ownership to its wallet.
  public func close() throws {
    lock.lock(); defer { lock.unlock() }
    let value = owner; owner = 0
    if value != 0 { try KagemushaWalletNativeDriverV1.check(driver.close(value)) }
  }
  deinit { try? close() }
}

/// Single-use account challenge. The existing Ed25519 account signs these exact 32 bytes.
public final class KagemushaWalletPendingOpenV1: @unchecked Sendable {
  private let lock = NSLock()
  private var runtime: KagemushaWalletRuntimeV1?
  public let challenge: Data
  fileprivate init(runtime: KagemushaWalletRuntimeV1, challenge: Data) { self.runtime = runtime; self.challenge = challenge }
  /// Failed finish consumes this challenge; the retained runtime can begin afresh.
  public func finish(accountSignature: Data) throws -> KagemushaWalletV1 {
    lock.lock(); let selected = runtime; runtime = nil; lock.unlock()
    guard let selected else { throw KagemushaWalletErrorV1.closed }
    return try selected.finish(accountSignature)
  }
  /// Abandon without admitting an owner or releasing the runtime's custody.
  public func cancel() throws {
    lock.lock(); let selected = runtime; runtime = nil; lock.unlock()
    if let selected { try selected.cancelPendingOpen() }
  }
  deinit { try? cancel() }
}
