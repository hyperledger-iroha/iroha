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
    case 0: bounds = [32,4096,1024]
    case 1,5: bounds = [64,0,0]
    case 2,7,8,10: bounds = [0,0,0]
    case 4: bounds = [32,65536,4096]
    case 6: bounds = [KagemushaWalletEnrollmentV1.RESULT_MAX_BYTES,0,0]
    case 9: bounds = [KagemushaWalletEnrollmentV1.PERMIT_MAX_BYTES,0,0]
    case 11: bounds = [16_384,4096,16_384]
    case 12: bounds = [KagemushaWalletEnrollmentV1.RESULT_MAX_BYTES,4096,0]
    case 13,16,17,18: bounds = [0,0,0]
    case 14: bounds = [1,0,0]
    case 15: bounds = [1,65536,0]
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
/// Bounded sensitive originals. Authentication belongs exclusively to the installed Native owner.
public struct KagemushaWalletEnrollmentSessionOriginalsV1: Sendable, CustomStringConvertible {
  public var description: String { "KagemushaWalletEnrollmentSessionOriginalsV1(originals=[REDACTED])" }
  let originals: [Data]
  public init(accessToken: Data, dpopProof: Data, attestationRootDER: Data) throws {
    let values = [accessToken, dpopProof, attestationRootDER]
    guard !accessToken.isEmpty, !attestationRootDER.isEmpty,
      zip(values, [16_384, 4096, 16_384]).allSatisfy({ $0.0.count <= $0.1 })
    else { throw KagemushaWalletErrorV1.invalidInput }
    originals = values.map { Data([UInt8]($0)) }
  }
}
/// Opaque enrollment custody created only by trusted native deployment initialization.
/// This owner transfers to original wallet open only after complete source qualification.
public final class KagemushaWalletEnrollmentV1: KagemushaWalletCleanupResourceV1, @unchecked Sendable {
  public static let REQUEST_MAX_BYTES = 524_288
  public static let RESULT_MAX_BYTES = 262_144
  public static let DISPATCH_MAX_BYTES = 16_384
  public static let PERMIT_MAX_BYTES = 2_048
  private let lock = NSLock()
  private var lease: KagemushaWalletNativeLeaseV1?
  private let driver: KagemushaWalletNativeDriverV1
  private var appleCollectionActive = false
  private var retirementRequested = false
  private var appleReturned: (UInt8, Data)?
  /// Exact asset frame selected from the authenticated installed application release.
  public let assetScopeOriginal: Data
  public let assetScale: UInt32
  init(lease: KagemushaWalletNativeLeaseV1, driver: KagemushaWalletNativeDriverV1, assetScope: Data, assetScale: UInt32) {
    self.driver = driver; self.lease = lease; self.assetScopeOriginal = Data([UInt8](assetScope)); self.assetScale = assetScale
  }
  var cleanupLease: (any KagemushaWalletCleanupLeaseV1)? {
    lock.lock(); defer { lock.unlock() }; return lease
  }
  private func call(_ input: KagemushaWalletEnrollmentInputV1) throws -> KagemushaWalletCallV1 {
    guard !appleCollectionActive || [13,14,15,17].contains(input.selector) else { throw KagemushaWalletErrorV1.native(status: -11, reason: -1, platformCode: 0) }
    guard let lease, !retirementRequested || input.selector == 15 else { throw KagemushaWalletErrorV1.closed }
    let owner = try lease.handle()
    let reply = try driver.result { out in input.withRequest { driver.enrollment(owner, $0, out) } }
    let expected: Set<Int32>
    switch input.selector {
    case 0: expected = [27]
    case 1,2: expected = [19,20,21,22]
    case 3,4: expected = [23,24]
    case 5: expected = [24]
    case 6: expected = [25]
    case 7: expected = [26]
    case 9: expected = [18]
    case 10: expected = [28]
    case 13: expected = [38]
    case 14,15,17: expected = [39]
    case 16: expected = [20,24]
    case 18: expected = [20,25]
    default: expected = []
    }
    let bound: Int
    switch reply.status {
    case 18,23: bound = 32
    case 19: bound = 161
    case 24: bound = Self.REQUEST_MAX_BYTES
    case 25: bound = Self.RESULT_MAX_BYTES
    case 27: bound = Self.DISPATCH_MAX_BYTES
    case 28: bound = 1024
    case 38: bound = 73_740
    default: bound = 0
    }
    guard expected.contains(reply.status), reply.sequenceLow == owner,
      reply.sequenceHigh == 0, reply.detail == 0,
      bound == 0 ? reply.bytes.isEmpty : (1...bound).contains(reply.bytes.count),
      ![18,23].contains(reply.status) || reply.bytes.count == 32,
      reply.status != 19 || reply.bytes.count == 161 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return reply
  }
  private func exact(_ input: KagemushaWalletEnrollmentInputV1, _ status: Int32) throws -> Data {
    let reply = try call(input)
    guard reply.status == status else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return reply.bytes
  }
  /// Native retains request identity and returns exact issuer dispatch DATA, without a key grant.
  public func begin(requestID: Data, account: Data) throws -> Data {
    lock.lock(); defer { lock.unlock() }
    guard requestID.count == 32 else { throw KagemushaWalletErrorV1.invalidInput }
    return try exact(KagemushaWalletEnrollmentInputV1(0,requestID,account,assetScopeOriginal),27)
  }
  /// Authenticate the signed issuer permit before returning the existing-account challenge.
  public func acceptPermit(originalPermit: Data) throws -> Data {
    lock.lock(); defer { lock.unlock() }
    guard (1...Self.PERMIT_MAX_BYTES).contains(originalPermit.count) else { throw KagemushaWalletErrorV1.invalidInput }
    return try exact(KagemushaWalletEnrollmentInputV1(9,originalPermit),18)
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
  /// Exact authenticated E5 already retained by Native; no vendor effect is repeated.
  public func retainedRequest() throws -> Data? {
    lock.lock(); defer { lock.unlock() }
    let reply = try call(KagemushaWalletEnrollmentInputV1(16))
    return reply.status == 24 ? reply.bytes : nil
  }
  /// Exact verified durable E6; errors are never interpreted as absence.
  public func retainedResult() throws -> Data? {
    lock.lock(); defer { lock.unlock() }
    let reply = try call(KagemushaWalletEnrollmentInputV1(18))
    return reply.status == 25 ? reply.bytes : nil
  }
  /// Original App Attest bytes only; Native binds the exact selected generated payment key.
  public func prepareRequest(keyID: Data, attestation: Data, keyBindingAssertion: Data) throws -> KagemushaWalletEnrollmentRequestV1 {
    lock.lock(); defer { lock.unlock() }
    let reply = try call(KagemushaWalletEnrollmentInputV1(4,keyID,attestation,keyBindingAssertion))
    switch reply.status { case 23: return .accountChallenge(reply.bytes); case 24: return .retained(reply.bytes); default: throw KagemushaWalletErrorV1.invalidNativeOutput }
  }
  /// Collect each App Attest effect once under this same Native owner and live permit.
  /// Returned originals are retained before cancellation/owner guards, including uncertain storage.
  public func collectAppleEvidence(platform: KagemushaWalletApplePlatformV1,
    target: KagemushaWalletEnrollmentTargetV1,
    requireCurrent: @escaping @Sendable () throws -> Void) async throws -> KagemushaWalletAppleEnrollmentEvidenceV1 {
    try requireCurrent()
    try beginAppleCollection(platform: platform, target: target)
    defer { endAppleCollection() }
    guard let slot = KagemushaWalletAppleSlotV1(target.slot) else { throw KagemushaWalletErrorV1.invalidInput }
    return try await platform.collectEnrollmentEvidence(slot: slot, paymentPublicKey: target.paymentKey,
      challengeDigest: target.challengeDigest, journal: EnrollmentAppleJournalAdapterV1(owner: self, requireCurrent: requireCurrent))
  }
  private func beginAppleCollection(platform: KagemushaWalletApplePlatformV1, target: KagemushaWalletEnrollmentTargetV1) throws {
    lock.lock(); defer { lock.unlock() }
    guard !appleCollectionActive, !retirementRequested, lease?.ownsPlatform(platform) == true else { throw KagemushaWalletErrorV1.closed }
    guard case .evidence(let actual) = try progress(call(KagemushaWalletEnrollmentInputV1(2))),
      actual.slot == target.slot, actual.paymentKey == target.paymentKey,
      actual.challengeDigest == target.challengeDigest, actual.keyBindingDigest == target.keyBindingDigest
    else { throw KagemushaWalletErrorV1.invalidInput }
    appleCollectionActive = true
  }
  private func endAppleCollection() { lock.lock(); appleCollectionActive = false; lock.unlock() }
  private func flushAppleReturned() throws {
    if let (stage, original) = appleReturned {
      _ = try exact(KagemushaWalletEnrollmentInputV1(15, Data([stage]), original), 39)
      appleReturned = nil
    }
  }
  /// Native retains the account-authorized E5 before these exact bytes may be sent.
  public func retainRequest(accountSignature: Data) throws -> Data {
    lock.lock(); defer { lock.unlock() }
    return try exact(KagemushaWalletEnrollmentInputV1(5,accountSignature),24)
  }
  /// Verify and retain exact issuer evidence and initial credential. This is not ledger activation.
  public func acceptCredential(issuerResult: Data) throws -> Data {
    lock.lock(); defer { lock.unlock() }
    guard (1...Self.RESULT_MAX_BYTES).contains(issuerResult.count) else { throw KagemushaWalletErrorV1.invalidInput }
    return try exact(KagemushaWalletEnrollmentInputV1(6,issuerResult),25)
  }
  /// Load actual signed complete sources before transferring the same native handle.
  public func loadRuntime() throws -> KagemushaWalletRuntimeV1 {
    lock.lock(); defer { lock.unlock() }
    _ = try exact(KagemushaWalletEnrollmentInputV1(7),26)
    guard let lease else { throw KagemushaWalletErrorV1.closed }
    let runtime = KagemushaWalletRuntimeV1(lease: lease, driver: driver)
    self.lease = nil
    return runtime
  }
  /// Transfer the source-qualified same runtime to the maintained account-admission owner.
  public func loadInstalledRuntime() throws -> KagemushaWalletInstalledRuntimeV1 {
    try KagemushaWalletInstalledRuntimeV1.requireNoUnreleasedAdmissions()
    return KagemushaWalletInstalledRuntimeV1.adoptEnrolled(try loadRuntime())
  }
  /// Permanently abandon the unused enrollment and return exact retained signed ledger bytes.
  /// Native refuses after Bootstrap commits. This is not a ledger acknowledgement.
  public func abandon() throws -> Data {
    lock.lock(); defer { lock.unlock() }
    return try exact(KagemushaWalletEnrollmentInputV1(10),28)
  }
  public func close() throws {
    lock.lock(); defer { lock.unlock() }
    retirementRequested = true
    guard !appleCollectionActive else { throw KagemushaWalletErrorV1.native(status: -5, reason: -1, platformCode: 0) }
    try flushAppleReturned()
    try lease?.close()
  }
  deinit { try? close() }
}

// Internal protocol calls are serialized with all other operations of the same actual lease.
extension KagemushaWalletEnrollmentV1: KagemushaWalletAppleEvidenceJournalV1 {
  func originals() throws -> [Data] {
    lock.lock(); defer { lock.unlock() }
    try flushAppleReturned()
    let bytes = try exact(KagemushaWalletEnrollmentInputV1(13), 38)
    var offset = 0, result: [Data] = []
    for bound in [4096, 65536, 4096] {
      guard offset + 4 <= bytes.count else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      let count = bytes.subdata(in: offset..<offset + 4).reduce(0) { ($0 << 8) | Int($1) }
      offset += 4
      guard count <= bound, count <= bytes.count - offset else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      result.append(bytes.subdata(in: offset..<offset + count)); offset += count
    }
    guard offset == bytes.count else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return result
  }
  func begin(_ stage: UInt8) throws {
    lock.lock(); defer { lock.unlock() }
    guard appleCollectionActive, !retirementRequested else { throw KagemushaWalletErrorV1.closed }
    _ = try exact(KagemushaWalletEnrollmentInputV1(14, Data([stage])), 39)
  }
  func retain(_ stage: UInt8, _ original: Data) throws {
    lock.lock(); defer { lock.unlock() }
    if let (heldStage, held) = appleReturned {
      guard heldStage == stage, held == original else { throw KagemushaWalletErrorV1.invalidInput }
    } else { appleReturned = (stage, Data([UInt8](original))) }
    try flushAppleReturned()
  }
  func complete() throws {
    lock.lock(); defer { lock.unlock() }
    _ = try exact(KagemushaWalletEnrollmentInputV1(17), 39)
  }
}

private struct EnrollmentAppleJournalAdapterV1: KagemushaWalletAppleEvidenceJournalV1 {
  let owner: KagemushaWalletEnrollmentV1
  let requireCurrent: @Sendable () throws -> Void
  func originals() throws -> [Data] { try requireCurrent(); return try owner.originals() }
  func begin(_ stage: UInt8) throws { try requireCurrent(); try owner.begin(stage) }
  func retain(_ stage: UInt8, _ original: Data) throws {
    // The actual vendor return reaches Native before any app callback may reject ownership.
    try owner.retain(stage, original)
    try requireCurrent()
  }
  func complete() throws { try requireCurrent(); try owner.complete() }
}
