import Foundation
import NoritoBridge

/// Native-selected hardware target, never an attestation verdict or key-creation grant.
public struct KagemushaWalletEnrollmentTargetV1: Sendable {
  public let slot: Data
  public let paymentKey: Data
  public let challengeDigest: Data
  public let keyBindingDigest: Data
  init(_ bytes: Data) throws {
    guard bytes.count == 161 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    slot = bytes.subdata(in: 0..<32); paymentKey = bytes.subdata(in: 32..<97)
    challengeDigest = bytes.subdata(in: 97..<129); keyBindingDigest = bytes.subdata(in: 129..<161)
  }
}
/// Enrollment progress; unavailable storage remains a thrown native error, never pending/absent.
public enum KagemushaWalletEnrollmentProgressV1: Sendable {
  case evidence(KagemushaWalletEnrollmentTargetV1)
  case pending
  case abandoned
  case bootstrapSelected
}
/// Existing-account authorization or the exact already-retained network request.
public enum KagemushaWalletEnrollmentRequestV1: Sendable {
  case accountChallenge(Data)
  case retained(Data)
}
struct KagemushaWalletEnrollmentInputV1 {
  let selector: UInt32
  let originals: [Data]
  init(_ selector: UInt32, _ first: Data = Data(), _ second: Data = Data(), _ third: Data = Data()) throws {
    let bounds: [Int]
    switch selector {
    case 0: bounds = [1024,4096,1024]
    case 1,5: bounds = [64,0,0]
    case 2,7,8: bounds = [0,0,0]
    case 4: bounds = [32,65536,4096]
    case 6: bounds = [262144,0,0]
    default: throw KagemushaWalletErrorV1.invalidInput
    }
    let originals = [first,second,third]
    guard zip(originals,bounds).allSatisfy({ $0.0.count <= $0.1 }) else { throw KagemushaWalletErrorV1.invalidInput }
    self.selector = selector; self.originals = originals
  }
  func withRequest<T>(_ body: (UnsafePointer<connect_norito_kagemusha_wallet_enrollment_request_v1>) throws -> T) rethrows -> T {
    try originals[0].withUnsafeBytes { a in try originals[1].withUnsafeBytes { b in try originals[2].withUnsafeBytes { c in
      var request = connect_norito_kagemusha_wallet_enrollment_request_v1()
      request.selector = selector
      request.first = a.bindMemory(to: UInt8.self).baseAddress; request.first_length = a.count
      request.second = b.bindMemory(to: UInt8.self).baseAddress; request.second_length = b.count
      request.third = c.bindMemory(to: UInt8.self).baseAddress; request.third_length = c.count
      request.certificates = nil; request.certificate_count = 0
      return try body(&request)
    }}}
  }
}
/// Opaque enrollment custody created only by trusted native deployment initialization.
/// This owner transfers to original wallet open only after complete source qualification.
public final class KagemushaWalletEnrollmentV1: @unchecked Sendable {
  private let lock = NSLock()
  private var owner: UInt64
  private let driver: KagemushaWalletNativeDriverV1
  public init(nativeEnrollmentHandle: UInt64) throws {
    guard nativeEnrollmentHandle > 0 && nativeEnrollmentHandle <= UInt64(Int64.max) else { throw KagemushaWalletErrorV1.invalidInput }
    owner = nativeEnrollmentHandle; driver = try KagemushaWalletNativeDriverV1()
  }
  private func call(_ input: KagemushaWalletEnrollmentInputV1) throws -> KagemushaWalletCallV1 {
    guard owner != 0 else { throw KagemushaWalletErrorV1.closed }
    let reply = try driver.result { out in input.withRequest { driver.enrollment(owner, $0, out) } }
    guard (18...26).contains(reply.status) && reply.sequenceLow == owner else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return reply
  }
  private func exact(_ input: KagemushaWalletEnrollmentInputV1, _ status: Int32) throws -> Data {
    let reply = try call(input)
    guard reply.status == status else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return reply.bytes
  }
  /// Existing account signs these exact native bytes before payment-key generation.
  public func begin(challenge: Data, account: Data, assetScope: Data) throws -> Data {
    lock.lock(); defer { lock.unlock() }
    return try exact(KagemushaWalletEnrollmentInputV1(0,challenge,account,assetScope),18)
  }
  private func progress(_ reply: KagemushaWalletCallV1) throws -> KagemushaWalletEnrollmentProgressV1 {
    switch reply.status {
    case 19: return .evidence(try KagemushaWalletEnrollmentTargetV1(reply.bytes))
    case 20: return .pending
    case 21: return .abandoned
    case 22: return .bootstrapSelected
    default: throw KagemushaWalletErrorV1.invalidNativeOutput
    }
  }
  /// A rejected native account signature consumes the challenge; begin afresh.
  public func authorize(accountSignature: Data) throws -> KagemushaWalletEnrollmentProgressV1 {
    lock.lock(); defer { lock.unlock() }
    return try progress(call(KagemushaWalletEnrollmentInputV1(1,accountSignature)))
  }
  public func progress() throws -> KagemushaWalletEnrollmentProgressV1 {
    lock.lock(); defer { lock.unlock() }
    return try progress(call(KagemushaWalletEnrollmentInputV1(2)))
  }
  /// Original App Attest bytes only; Native binds the exact selected generated payment key.
  public func prepareRequest(keyID: Data, attestation: Data, keyBindingAssertion: Data) throws -> KagemushaWalletEnrollmentRequestV1 {
    lock.lock(); defer { lock.unlock() }
    let reply = try call(KagemushaWalletEnrollmentInputV1(4,keyID,attestation,keyBindingAssertion))
    switch reply.status { case 23: return .accountChallenge(reply.bytes); case 24: return .retained(reply.bytes); default: throw KagemushaWalletErrorV1.invalidNativeOutput }
  }
  /// Native retains the account-authorized E5 before these exact bytes may be sent.
  public func retainRequest(accountSignature: Data) throws -> Data {
    lock.lock(); defer { lock.unlock() }
    return try exact(KagemushaWalletEnrollmentInputV1(5,accountSignature),24)
  }
  /// Verify and retain exact issuer evidence and initial credential. This is not ledger activation.
  public func acceptCredential(issuerResult: Data) throws -> Data {
    lock.lock(); defer { lock.unlock() }
    return try exact(KagemushaWalletEnrollmentInputV1(6,issuerResult),25)
  }
  /// Load actual signed complete sources before transferring the same native handle.
  public func loadRuntime() throws -> KagemushaWalletRuntimeV1 {
    lock.lock(); defer { lock.unlock() }
    _ = try exact(KagemushaWalletEnrollmentInputV1(7),26)
    let runtime = try KagemushaWalletRuntimeV1(nativeRuntimeHandle: owner)
    owner = 0
    return runtime
  }
  public func close() throws {
    lock.lock(); defer { lock.unlock() }
    let value = owner; owner = 0
    if value != 0 { try KagemushaWalletNativeDriverV1.check(driver.close(value)) }
  }
  deinit { try? close() }
}
