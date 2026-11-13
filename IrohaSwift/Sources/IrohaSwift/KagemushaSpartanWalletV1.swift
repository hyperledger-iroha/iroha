import Foundation
import NoritoBridge

/// Exact future-profile account admission DATA; only Native grants authority.
public struct KagemushaSpartanAccountOriginalsV1: Sendable {
  let originals: KagemushaWalletOpenOriginalsV1
  public init(credential: Data, enrollmentCertificate: Data, account: Data, assetScope: Data) throws {
    guard credential.count == 596, enrollmentCertificate.count == 172
    else { throw KagemushaWalletErrorV1.invalidInput }
    originals = try .init(credential: credential, enrollmentCertificates: enrollmentCertificate,
      account: account, assetScope: assetScope)
  }
}

/// Experimental proof backend sharing the admitted wallet's exclusive hardware custody owner.
public final class KagemushaSpartanWalletV1: @unchecked Sendable {
  private let owner: KagemushaWalletNativeLeaseV1
  private let driver: KagemushaSpartanDriverV1
  public let parameterId: Data
  public let slotId: Data
  fileprivate init(owner: KagemushaWalletNativeLeaseV1, driver: KagemushaSpartanDriverV1, parameterId: Data, slotId: Data) {
    self.owner = owner; self.driver = driver; self.parameterId = parameterId; self.slotId = slotId
  }

  /// Native authenticates the complete witness and freezes a hardware-signable review.
  public func prepare(witness: Data, predecessorHead: Data) throws -> KagemushaSpartanReviewV1 {
    guard !witness.isEmpty, witness.count <= Self.maximumRecordBytes,
      predecessorHead.count <= 1_048_576 else { throw KagemushaWalletErrorV1.invalidInput }
    let handle = try owner.handle()
    let reply = try driver.call { out in
      slotId.withUnsafeBytes { slot in witness.withUnsafeBytes { witness in predecessorHead.withUnsafeBytes { head in
        driver.prepare(handle, slot.bindMemory(to: UInt8.self).baseAddress,
          witness.bindMemory(to: UInt8.self).baseAddress, witness.count,
          head.bindMemory(to: UInt8.self).baseAddress, head.count, out)
      } } }
    }
    try reply.requireShape(status: 82, count: 291, token: true)
    do {
      return try .init(owner: self, token: reply.sequenceLow, projection: .init(reply.bytes))
    } catch {
      try? discard(reply.sequenceLow)
      throw error
    }
  }

  /// Reconcile durable selection and read retained originals. Retry only these exact Payment bytes.
  public func output(operationId: Data, part: KagemushaSpartanOutputV1) throws -> KagemushaSpartanRetainedOutputV1 {
    guard operationId.count == 32 else { throw KagemushaWalletErrorV1.invalidInput }
    let handle = try owner.handle()
    let reply = try driver.call { out in slotId.withUnsafeBytes { slot in operationId.withUnsafeBytes { operation in
      driver.output(handle, slot.bindMemory(to: UInt8.self).baseAddress,
        operation.bindMemory(to: UInt8.self).baseAddress, part.rawValue, out)
    } } }
    return try .init(reply)
  }

  /// Full native proof verification, binding the returned Request to the exact Payment bytes.
  public func verifyPayment(_ payment: Data) throws -> KagemushaSpartanVerifiedPaymentV1 {
    guard !payment.isEmpty, payment.count <= 65_536 else { throw KagemushaWalletErrorV1.invalidInput }
    let handle = try owner.handle()
    let reply = try driver.call { out in payment.withUnsafeBytes {
      driver.verifyPayment(handle, $0.bindMemory(to: UInt8.self).baseAddress, $0.count, out)
    } }
    try reply.requireShape(status: 83, count: 514)
    return .init(digest: reply.bytes.subdata(in: 0..<32), request: reply.bytes.subdata(in: 32..<514))
  }

  fileprivate func execute(_ token: UInt64) throws -> KagemushaSpartanExecutionV1 {
    let handle = try owner.handle()
    return try .init(driver.call { driver.execute(handle, token, $0) })
  }
  fileprivate func discard(_ token: UInt64) throws {
    let handle = try owner.handle()
    try driver.call(control: true) { driver.discard(handle, token, $0) }.requireShape(status: 84, count: 0)
  }
  /// Closing retains committed custody and exact outputs.
  public func close() throws { try owner.close() }
  deinit { try? owner.close() }
  static let maximumRecordBytes = 4 * 1024 * 1024
}

/// Retained installed backend before the existing account signs Native's fresh challenge.
public final class KagemushaSpartanRuntimeV1: @unchecked Sendable {
  private let lock = NSLock()
  private var lease: KagemushaWalletNativeLeaseV1?
  private let driver: KagemushaSpartanDriverV1
  public let parameterId: Data
  fileprivate init(lease: KagemushaWalletNativeLeaseV1, driver: KagemushaSpartanDriverV1, parameterId: Data) {
    self.lease = lease; self.driver = driver; self.parameterId = parameterId
  }
  /// Install public verifier programs against Native's independently installed root and scheme.
  static func install(lease: KagemushaWalletNativeLeaseV1, programBundle: Data, signedManifest: Data, signature: Data) throws -> KagemushaSpartanRuntimeV1 {
    guard !programBundle.isEmpty, programBundle.count <= 256 * 1024 * 1024,
      !signedManifest.isEmpty, signedManifest.count <= 16_384,
      signature.count == 64 else { throw KagemushaWalletErrorV1.invalidInput }
    let driver = try KagemushaSpartanDriverV1()
    let handle = try lease.handle()
    let reply = try driver.call { out in
      programBundle.withUnsafeBytes { programs in signedManifest.withUnsafeBytes { manifest in signature.withUnsafeBytes { signature in
        driver.install(handle, programs.bindMemory(to: UInt8.self).baseAddress, programs.count,
          manifest.bindMemory(to: UInt8.self).baseAddress, manifest.count,
          signature.bindMemory(to: UInt8.self).baseAddress, signature.count, out)
      } } }
    }
    try reply.requireShape(status: 81, count: 32, token: true)
    guard reply.sequenceLow == handle else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return .init(lease: lease, driver: driver, parameterId: reply.bytes)
  }
  /// Return the exact challenge for the existing Ed25519 account. No payment key is created.
  public func beginAccount(originals: KagemushaSpartanAccountOriginalsV1) throws -> Data {
    lock.lock(); defer { lock.unlock() }
    guard let lease else { throw KagemushaWalletErrorV1.closed }
    let handle = try lease.handle()
    let reply = try driver.call { out in originals.originals.withRequest { request in
      driver.beginAccount(handle, request, out)
    } }
    try reply.requireShape(status: 15, count: 32, token: true)
    guard reply.sequenceLow == handle, reply.bytes.contains(where: { $0 != 0 })
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return reply.bytes
  }
  /// Finish Native account admission and transfer this same exclusive custody lease once.
  public func authorizeAccount(signature: Data) throws -> KagemushaSpartanWalletV1 {
    guard signature.count == 64 else { throw KagemushaWalletErrorV1.invalidInput }
    lock.lock(); defer { lock.unlock() }
    guard let lease else { throw KagemushaWalletErrorV1.closed }
    let handle = try lease.handle()
    let reply = try driver.call { out in signature.withUnsafeBytes {
      driver.authorizeAccount(handle, $0.bindMemory(to: UInt8.self).baseAddress, $0.count, out)
    } }
    try reply.requireShape(status: 85, count: 64, token: true)
    guard reply.sequenceLow == handle, reply.bytes.prefix(32) == parameterId,
      reply.bytes.suffix(32).contains(where: { $0 != 0 }) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    let wallet = KagemushaSpartanWalletV1(owner: lease, driver: driver, parameterId: parameterId,
      slotId: reply.bytes.subdata(in: 32..<64))
    self.lease = nil
    return wallet
  }
  public func close() throws {
    lock.lock(); defer { lock.unlock() }
    try lease?.close(); lease = nil
  }
  deinit { try? close() }
}

public enum KagemushaSpartanOutputV1: UInt32, Sendable { case head = 0, payment = 1, completion = 2 }
public enum KagemushaSpartanOperationV1: UInt8, Sendable { case bootstrap = 1, load = 2, send = 3, receive = 4 }

public enum KagemushaSpartanRetainedOutputV1: Sendable {
  case unknown, pending, archived, deliveryLoss, released(Data)
  init(_ reply: KagemushaSpartanReplyV1) throws {
    try reply.requireShape(status: reply.status, count: reply.status == 1 ? nil : 0)
    switch reply.status {
    case 0: self = .unknown
    case 2: self = .pending
    case 4: self = .archived
    case 5: self = .deliveryLoss
    case 1:
      guard !reply.bytes.isEmpty else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      self = .released(reply.bytes)
    default: throw KagemushaWalletErrorV1.invalidNativeOutput
    }
  }
}

/// Native's exact review for confirmation UI; no caller-supplied validity flag.
public struct KagemushaSpartanReviewProjectionV1: Sendable {
  public let operation: KagemushaSpartanOperationV1
  public let schemeId, walletId, operationId, capsuleDigest: Data
  public let amount, fee: KagemushaWalletUInt128V1
  public let receiverWalletId, receiverAccountId, beforeHead, afterHead: Data
  init(_ original: Data) throws {
    let bytes = [UInt8](original)
    guard bytes.count == 291, bytes[0] == 1, bytes[1] == 0,
      let operation = KagemushaSpartanOperationV1(rawValue: bytes[2])
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    func word(_ offset: Int) -> UInt64 {
      (0..<8).reduce(UInt64(0)) { $0 | (UInt64(bytes[offset + $1]) << ($1 * 8)) }
    }
    func data(_ offset: Int) -> Data { Data(bytes[offset..<offset + 32]) }
    self.operation = operation
    schemeId = data(3); walletId = data(35); operationId = data(67); capsuleDigest = data(99)
    amount = .init(low: word(131), high: word(139)); fee = .init(low: word(147), high: word(155))
    receiverWalletId = data(163); receiverAccountId = data(195); beforeHead = data(227); afterHead = data(259)
  }
}

/// Review bound to one Native owner and capsule; pending execution is retryable.
public final class KagemushaSpartanReviewV1: @unchecked Sendable {
  private let owner: KagemushaSpartanWalletV1
  private let lock = NSLock()
  private var token: UInt64
  private var executing = false
  public let projection: KagemushaSpartanReviewProjectionV1
  fileprivate init(owner: KagemushaSpartanWalletV1, token: UInt64, projection: KagemushaSpartanReviewProjectionV1) {
    self.owner = owner; self.token = token; self.projection = projection
  }
  private func begin() throws -> UInt64 {
    lock.lock(); defer { lock.unlock() }
    guard token > 0, !executing else { throw KagemushaWalletErrorV1.invalidInput }
    executing = true; return token
  }
  private func finish(consumed: Bool) {
    lock.lock(); defer { lock.unlock() }
    if consumed { token = 0 }; executing = false
  }
  /// Invoke only after fresh hardware-backed biometric/device-credential confirmation of this exact review.
  public func execute() throws -> KagemushaSpartanExecutionV1 {
    let value = try begin()
    do {
      let result = try owner.execute(value)
      finish(consumed: result.status != .pending)
      return result
    } catch {
      // A failed transport/allocation may hide Selected. Retry only this frozen token.
      finish(consumed: false); throw error
    }
  }
  public func discard() throws {
    lock.lock(); let alreadyConsumed = token == 0; lock.unlock()
    if alreadyConsumed { return }
    let value = try begin()
    do { try owner.discard(value); finish(consumed: true) }
    catch { finish(consumed: false); throw error }
  }
  deinit { try? discard() }
}

/// Uncertain statuses require recovery of the same operation, never a replacement debit.
public struct KagemushaSpartanExecutionV1: Sendable {
  public enum Status: Int32, Sendable { case released = 1, pending = 2, notPerformed = 3, archived = 4, deliveryLoss = 5 }
  public let status: Status
  public let sequence: KagemushaWalletUInt128V1
  public let detail: UInt32
  public let completion: Data?
  init(_ reply: KagemushaSpartanReplyV1) throws {
    guard let status = Status(rawValue: reply.status), (status == .released) == !reply.bytes.isEmpty
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    self.status = status; sequence = .init(low: reply.sequenceLow, high: reply.sequenceHigh)
    detail = reply.detail; completion = status == .released ? reply.bytes : nil
  }
}
public struct KagemushaSpartanVerifiedPaymentV1: Sendable {
  public let digest: Data
  public let request: Data
}

struct KagemushaSpartanReplyV1 {
  let status: Int32
  let sequenceLow, sequenceHigh: UInt64
  let detail: UInt32
  let bytes: Data
  func requireShape(status: Int32, count: Int? = nil, token: Bool = false) throws {
    guard self.status == status, count == nil || bytes.count == count,
      sequenceHigh == 0, detail == 0,
      token ? (sequenceLow > 0 && sequenceLow <= UInt64(Int64.max)) : sequenceLow == 0
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
  }
}

fileprivate final class KagemushaSpartanDriverV1: @unchecked Sendable {
  typealias Output = connect_norito_kagemusha_wallet_result_v1
  typealias Install = @convention(c) (UInt64, UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
  typealias Prepare = @convention(c) (UInt64, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
  typealias Execute = @convention(c) (UInt64, UInt64, UnsafeMutablePointer<Output>?) -> Int32
  typealias Read = @convention(c) (UInt64, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, UInt32, UnsafeMutablePointer<Output>?) -> Int32
  typealias Verify = @convention(c) (UInt64, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
  typealias BeginAccount = @convention(c) (UInt64, UnsafePointer<connect_norito_kagemusha_wallet_open_request_v1>?, UnsafeMutablePointer<Output>?) -> Int32
  typealias Free = @convention(c) (UnsafeMutableRawPointer?) -> Void
  let install: Install, prepare: Prepare, execute: Execute, output: Read, verifyPayment: Verify, discard: Execute
  let beginAccount: BeginAccount, authorizeAccount: Verify
  private let free: Free
  init() throws {
    func symbol<T>(_ name: String, _ type: T.Type) throws -> T {
      guard let value = NoritoNativeBridge.shared.resolveNativeSymbol(name, as: type)
      else { throw KagemushaWalletErrorV1.bridgeUnavailable }
      return value
    }
    install = try symbol("connect_norito_kagemusha_spartan_install_v1", Install.self)
    prepare = try symbol("connect_norito_kagemusha_spartan_prepare_v1", Prepare.self)
    execute = try symbol("connect_norito_kagemusha_spartan_execute_v1", Execute.self)
    output = try symbol("connect_norito_kagemusha_spartan_output_v1", Read.self)
    verifyPayment = try symbol("connect_norito_kagemusha_spartan_verify_payment_v1", Verify.self)
    discard = try symbol("connect_norito_kagemusha_spartan_discard_v1", Execute.self)
    beginAccount = try symbol("connect_norito_kagemusha_spartan_begin_account_v1", BeginAccount.self)
    authorizeAccount = try symbol("connect_norito_kagemusha_spartan_authorize_account_v1", Verify.self)
    free = try symbol("connect_norito_free", Free.self)
  }
  func call(control: Bool = false, _ action: (UnsafeMutablePointer<Output>) -> Int32) throws -> KagemushaSpartanReplyV1 {
    func run() throws -> KagemushaSpartanReplyV1 {
      var output = Output()
      let status = action(&output)
      defer { if let bytes = output.bytes { free(bytes) } }
      try KagemushaWalletNativeDriverV1.check(status, reason: output.reason, platform: output.platform_code)
      guard output.reason == -1, output.platform_code == 0, output.length >= 0,
        output.length <= KagemushaSpartanWalletV1.maximumRecordBytes,
        output.length == 0 || output.bytes != nil else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return .init(status: output.status, sequenceLow: output.sequence_low, sequenceHigh: output.sequence_high,
        detail: output.detail, bytes: output.bytes.map { Data(bytes: $0, count: output.length) } ?? Data())
    }
    return try control ? run() : KagemushaWalletNativeWorkerV1.perform(run)
  }
}
