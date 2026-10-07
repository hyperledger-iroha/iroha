import Foundation

/// Native wallet failure; uncertain outcomes require reconciliation, never a replacement debit.
public enum KagemushaWalletErrorV1: Error, Equatable, Sendable {
  case bridgeUnavailable, artifactsUnavailable, invalidInput, invalidNativeOutput, closed
  case native(status: Int32, reason: Int32, platformCode: Int32)
}

/// Explicit custody result with original canonical bytes.
public struct KagemushaWalletCallV1: Sendable {
  /// Unknown0, complete1, pending2, not performed3, archived4, delivery loss5,
  /// idle6, caught up7, checkpoint8, folded9, CreditStatus10, preparing11, setup12,
  /// timeChallenge13, timeRetained14, accountChallenge15, opened16, activation17.
  public let status: Int32
  public let sequenceLow: UInt64
  public let sequenceHigh: UInt64
  /// Checkpoint ordinal; for not-performed: stale0, capacity1, invalid2.
  public let detail: UInt32
  /// Exact retained bytes for complete1, CreditStatus10, setup12, timeChallenge13,
  /// accountChallenge15 or activation17.
  public let bytes: Data
  init(status: Int32, sequenceLow: UInt64, sequenceHigh: UInt64, detail: UInt32, bytes: Data) throws
  {
    guard (0...17).contains(status), bytes.count <= (status == 17 ? 16_384 : 10_000),
      (status == 1 || status == 10 || status == 12 || status == 13 || status == 15 || status == 17) ? !bytes.isEmpty : bytes.isEmpty,
      ![12, 14, 17].contains(status) || (sequenceLow == 0 && sequenceHigh == 0 && detail == 0),
      ![13, 15, 16].contains(status) || (sequenceLow > 0 && sequenceLow <= UInt64(Int64.max) && sequenceHigh == 0 && detail == 0),
      ![13, 15].contains(status) || bytes.count == 32,
      status != 13 || bytes.contains(where: { $0 != 0 })
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    self.status = status
    self.sequenceLow = sequenceLow
    self.sequenceHigh = sequenceHigh
    self.detail = detail
    self.bytes = bytes
  }
}

import NoritoBridge

/// Exclusive native owner. Use worker threads for proving. Activity/payment calls can preempt
/// folding because no Swift lock is held across native calls. Closing preserves committed bytes.
/// Created only after original account admission through a native-provisioned runtime.
public final class KagemushaWalletV1: @unchecked Sendable {
  private let lock = NSLock()
  private var owner: UInt64
  private let driver: KagemushaWalletNativeDriverV1
  private let setupOrigin = KagemushaWalletSetupOriginV1()
  init(handle: UInt64, driver: KagemushaWalletNativeDriverV1) {
    self.owner = handle
    self.driver = driver
  }
  private func handle() throws -> UInt64 {
    lock.lock()
    defer { lock.unlock() }
    guard owner != 0 else { throw KagemushaWalletErrorV1.closed }
    return owner
  }
  /// Join cooperative work and release custody, without deleting keys, markers or Payments.
  public func close() throws {
    lock.lock()
    let value = owner
    owner = 0
    lock.unlock()
    if value != 0 { try KagemushaWalletNativeDriverV1.check(driver.close(value)) }
  }
  deinit { try? close() }
  /// Enable folding while foreground or charging; leaving both requests cooperative cancellation.
  public func setActivity(foreground: Bool, charging: Bool) throws {
    try KagemushaWalletNativeDriverV1.check(
      driver.activity(try handle(), foreground ? 1 : 0, charging ? 1 : 0))
  }
  private func execute(_ input: KagemushaWalletOperationInputV1) throws -> KagemushaWalletCallV1 {
    let value = try handle()
    return try driver.result { out in
      input.withRequest { request in driver.execute(value, request, out) }
    }.completion()
  }
  private func setup(_ input: KagemushaWalletSetupInputV1) throws -> KagemushaWalletCallV1 {
    let value = try handle()
    return try driver.result { out in input.withRequest { driver.setup(value, $0, out) } }
  }
  /// Complete restart-safe native enrollment Bootstrap.
  public func bootstrap() throws -> KagemushaWalletCallV1 { try setup(.init(selector: 0)).completion() }
  /// Exact retained Activate frame for ledger submission; this is not activation confirmation.
  public func activation() throws -> Data {
    let result = try setup(.init(selector: 15))
    guard result.status == 17 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return result.bytes
  }
  /// Sign an Offer with a retained native nonce; retries return its exact original bytes.
  public func offer(setupId: Data, amount: KagemushaWalletUInt128V1) throws -> KagemushaWalletCallV1 {
    let result = try setup(.init(selector: 1, identity: setupId, amount: amount))
    _ = try result.original()
    return result
  }
  /// Authenticate the payer Offer and issue a Request from native current policy.
  public func request(setupId: Data, offer: Data, feeSchedule: Data? = nil, feeCertificate: Data? = nil) throws -> KagemushaWalletCallV1 {
    let result = try setup(.request(identity: setupId, offer: offer,
      feeSchedule: feeSchedule, feeCertificate: feeCertificate))
    _ = try result.original()
    return result
  }
  /// Native verifies delivery evidence and derives the private Archive operation.
  public func acceptCredited(_ original: Data) throws -> KagemushaWalletCallV1 {
    try setup(.init(selector: 3, first: original)).completion()
  }
  /// Begin a direct issuer exchange. Only the fresh nonce leaves native clock custody.
  public func beginTimeExchange() throws -> KagemushaWalletTimeExchangeV1 {
    try setup(.init(selector: 4)).exchange(origin: setupOrigin)
  }
  /// Consume the native exchange token and retain its verified anchor before TimeAnchor refresh.
  /// Invalid local bounds leave the challenge usable; dispatch consumes it even on failure.
  public func finishTimeExchange(_ exchange: KagemushaWalletTimeExchangeV1, anchor: Data, certificate: Data) throws -> KagemushaWalletCallV1 {
    let input = try KagemushaWalletSetupInputV1(selector: 5,
      token: exchange.tokenFor(origin: setupOrigin), first: anchor, second: certificate)
    _ = try handle()
    try exchange.consume(origin: setupOrigin)
    return try setup(input).timeRetained()
  }
  /// Discard an unanswered native time exchange without accepting or refreshing an anchor.
  public func cancelTimeExchange(_ exchange: KagemushaWalletTimeExchangeV1) throws {
    let input = try KagemushaWalletSetupInputV1(selector: 6,
      token: exchange.tokenFor(origin: setupOrigin))
    _ = try handle()
    try exchange.consume(origin: setupOrigin)
    let result = try setup(input)
    guard result.status == 6, result.sequenceLow == 0, result.sequenceHigh == 0,
      result.detail == 0 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
  }
  /// Frame exact original bytes for peer transport; this performs no monetary admission.
  public func envelope(_ kind: KagemushaWalletTransportKindV1, original: Data) throws -> Data {
    try setup(.init(selector: 6 + kind.rawValue, first: original)).original()
  }
  /// Extract one expected original kind under this native wallet's scheme, without accepting value.
  public func original(_ kind: KagemushaWalletTransportKindV1, envelope: Data) throws -> Data {
    try setup(.init(selector: 10 + kind.rawValue, first: envelope)).original()
  }
  /// Load exact ordinary-ledger receipt and compact finality originals.
  public func load(requestId: Data, receipt: Data, finality: Data) throws -> KagemushaWalletCallV1 {
    try execute(KagemushaWalletOperationInputV1(requestId: requestId, selector: 0, first: receipt, second: finality))
  }
  /// Irreversible Send to the exact receiver-signed Request; Native authenticates every field.
  public func send(requestId: Data, request: Data) throws -> KagemushaWalletCallV1 {
    try execute(KagemushaWalletOperationInputV1(requestId: requestId, selector: 1, first: request))
  }
  /// Native selects the durably issued Request using the Payment's canonical digest.
  public func receive(requestId: Data, payment: Data, payerCredential: Data, certificates: Data) throws -> KagemushaWalletCallV1 {
    try execute(KagemushaWalletOperationInputV1(requestId: requestId, selector: 2, first: payment, second: payerCredential, third: certificates))
  }
  /// Apply signed policy originals; Native derives effective values and local map changes.
  public func refresh(requestId: Data, kind: KagemushaWalletRefreshKindV1, update: Data, certificates: Data) throws -> KagemushaWalletCallV1 {
    try execute(KagemushaWalletOperationInputV1(requestId: requestId, selector: kind.rawValue, first: update, second: certificates))
  }
  /// Unload gross value to the credential account; a quote requires its certificate set.
  public func unload(requestId: Data, amount: KagemushaWalletUInt128V1, quote: Data? = nil, certificates: Data? = nil) throws -> KagemushaWalletCallV1 {
    guard (quote == nil) == (certificates == nil) else { throw KagemushaWalletErrorV1.invalidInput }
    return try execute(KagemushaWalletOperationInputV1(requestId: requestId, selector: 8, amount: amount, first: quote ?? Data(), second: certificates ?? Data()))
  }
  /// Enter Retiring under native folded-state checks.
  public func retire(requestId: Data) throws -> KagemushaWalletCallV1 {
    try execute(KagemushaWalletOperationInputV1(requestId: requestId, selector: 9))
  }
  /// Distinguish durable preparation from irreversible pending and retained completion.
  public func requestStatus(requestId: Data) throws -> KagemushaWalletCallV1 {
    guard requestId.count == 32, requestId.contains(where: { $0 != 0 }) else { throw KagemushaWalletErrorV1.invalidInput }
    let value = try handle()
    return try driver.result { out in
      requestId.withUnsafeBytes { driver.requestStatus(value, $0.bindMemory(to: UInt8.self).baseAddress, out) }
    }.completion()
  }
  /// Return original retained output without a second signature, proof or debit.
  public func retry(operationId: Data) throws -> KagemushaWalletCallV1 {
    guard operationId.count == 32 else { throw KagemushaWalletErrorV1.invalidInput }
    let value = try handle()
    return try driver.result { out in
      operationId.withUnsafeBytes {
        driver.retry(value, $0.bindMemory(to: UInt8.self).baseAddress, out)
      }
    }.completion()
  }
  /// Reconcile and finish the selected operation.
  public func resume() throws -> KagemushaWalletCallV1 {
    let value = try handle()
    return try driver.result { driver.resume(value, $0) }.completion()
  }
  /// Compute/persist at most one checkpoint or final Ω.
  public func foldOnce() throws -> KagemushaWalletCallV1 {
    let value = try handle()
    return try driver.result { driver.fold(value, $0) }.completion()
  }
  /// Source-selected ownership and proof backlog. Call on a worker; this grants no operation readiness.
  public func snapshot() throws -> KagemushaWalletSnapshotV1 {
    var output = connect_norito_kagemusha_wallet_snapshot_v1_t()
    let status = driver.snapshot(try handle(), &output)
    try KagemushaWalletNativeDriverV1.check(status, reason: output.reason, platform: output.platform_code)
    return try KagemushaWalletSnapshotV1(output)
  }
  /// Canonical CreditStatus from the permanent first-credit index.
  public func creditStatus(creditId: Data, paymentDigest: Data) throws -> KagemushaWalletCallV1 {
    guard creditId.count == 32, paymentDigest.count == 32 else {
      throw KagemushaWalletErrorV1.invalidInput
    }
    let value = try handle()
    return try driver.result { out in
      creditId.withUnsafeBytes { credit in
        paymentDigest.withUnsafeBytes { payment in
          driver.credit(
            value, credit.bindMemory(to: UInt8.self).baseAddress,
            payment.bindMemory(to: UInt8.self).baseAddress, out)
        }
      }
    }.completion()
  }
}

/// Existing signed update classes; these values never select proof keys or state roots.
public enum KagemushaWalletRefreshKindV1: UInt32, Sendable {
  case credential = 3, schemePolicy = 4, blacklist = 5, timeAnchor = 6, quotaShare = 7
}

/// Fixed foreign intake only; Native performs canonical decoding and all monetary admission.
struct KagemushaWalletOperationInputV1 {
  let requestId: Data
  let selector: UInt32
  let amount: KagemushaWalletUInt128V1
  let first: Data
  let second: Data
  let third: Data
  init(requestId: Data, selector: UInt32, amount: KagemushaWalletUInt128V1 = .init(low: 0, high: 0),
       first: Data = Data(), second: Data = Data(), third: Data = Data()) throws {
    let limits: [Int]
    switch selector {
    case 0: limits = [512, 16_384, 0]
    case 1: limits = [10_000, 0, 0]
    case 2: limits = [10_000, 1_024, 10_000]
    case 3, 4, 8: limits = [1_024, 10_000, 0]
    case 5: limits = [65_536 * 34 + 512, 10_000, 0]
    case 6: limits = [512, 10_000, 0]
    case 7: limits = [8_192, 10_000, 0]
    case 9: limits = [0, 0, 0]
    default: throw KagemushaWalletErrorV1.invalidInput
    }
    let nonzero = amount.low != 0 || amount.high != 0
    guard requestId.count == 32, requestId.contains(where: { $0 != 0 }),
      selector == 8 ? nonzero : !nonzero else { throw KagemushaWalletErrorV1.invalidInput }
    for (bytes, limit) in zip([first, second, third], limits) {
      guard bytes.count <= limit, !(selector < 8 && limit != 0 && bytes.isEmpty)
      else { throw KagemushaWalletErrorV1.invalidInput }
    }
    guard selector != 8 || first.isEmpty == second.isEmpty else { throw KagemushaWalletErrorV1.invalidInput }
    self.requestId = requestId; self.selector = selector; self.amount = amount
    self.first = first; self.second = second; self.third = third
  }
  func withRequest<T>(_ body: (UnsafePointer<connect_norito_kagemusha_wallet_operation_request_v1>) -> T) -> T {
    requestId.withUnsafeBytes { identity in first.withUnsafeBytes { a in
      second.withUnsafeBytes { b in third.withUnsafeBytes { c in
        var request = connect_norito_kagemusha_wallet_operation_request_v1()
        request.request_id = identity.bindMemory(to: UInt8.self).baseAddress
        request.selector = selector
        request.amount = .init(low: amount.low, high: amount.high)
        request.first = a.bindMemory(to: UInt8.self).baseAddress; request.first_length = a.count
        request.second = b.bindMemory(to: UInt8.self).baseAddress; request.second_length = b.count
        request.third = c.bindMemory(to: UInt8.self).baseAddress; request.third_length = c.count
        return body(&request)
      }}
    }}
  }
}

final class KagemushaWalletNativeDriverV1: @unchecked Sendable {
  typealias Output = connect_norito_kagemusha_wallet_result_v1
  typealias OpenBegin = @convention(c) (UInt64, UnsafePointer<connect_norito_kagemusha_wallet_open_request_v1>?, UnsafeMutablePointer<Output>?) -> Int32
  typealias OpenFinish = @convention(c) (UInt64, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
  typealias Close = @convention(c) (UInt64) -> Int32
  typealias Activity = @convention(c) (UInt64, UInt8, UInt8) -> Int32
  typealias Execute =
    @convention(c) (UInt64, UnsafePointer<connect_norito_kagemusha_wallet_operation_request_v1>?, UnsafeMutablePointer<Output>?) -> Int32
  typealias Setup = @convention(c) (UInt64, UnsafePointer<connect_norito_kagemusha_wallet_setup_request_v1>?, UnsafeMutablePointer<Output>?) -> Int32
  typealias Retry =
    @convention(c) (UInt64, UnsafePointer<UInt8>?, UnsafeMutablePointer<Output>?) -> Int32
  typealias Simple = @convention(c) (UInt64, UnsafeMutablePointer<Output>?) -> Int32
  typealias Credit =
    @convention(c) (
      UInt64, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, UnsafeMutablePointer<Output>?
    ) -> Int32
  typealias Snapshot = @convention(c) (UInt64, UnsafeMutablePointer<connect_norito_kagemusha_wallet_snapshot_v1_t>?) -> Int32
  typealias Free = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
  let openBegin: OpenBegin
  let openFinish: OpenFinish
  let openCancel: Close
  let close: Close
  let activity: Activity
  let execute: Execute
  let setup: Setup
  let requestStatus: Retry
  let retry: Retry
  let resume: Simple
  let fold: Simple
  let credit: Credit
  let snapshot: Snapshot
  let free: Free
  init() throws {
    func symbol<T>(_ name: String, _ type: T.Type) throws -> T {
      guard
        let value = NoritoNativeBridge.shared.resolveNativeSymbol(
          name, as: type)
      else { throw KagemushaWalletErrorV1.bridgeUnavailable }
      return value
    }
    let revision = try symbol(
      "connect_norito_kagemusha_wallet_revision_v1", (@convention(c) () -> UInt32).self)
    guard revision() == 1 else { throw KagemushaWalletErrorV1.bridgeUnavailable }
    openBegin = try symbol("connect_norito_kagemusha_wallet_open_begin_v1", OpenBegin.self)
    openFinish = try symbol("connect_norito_kagemusha_wallet_open_finish_v1", OpenFinish.self)
    openCancel = try symbol("connect_norito_kagemusha_wallet_open_cancel_v1", Close.self)
    close = try symbol("connect_norito_kagemusha_wallet_close_v1", Close.self)
    activity = try symbol("connect_norito_kagemusha_wallet_activity_v1", Activity.self)
    execute = try symbol("connect_norito_kagemusha_wallet_execute_v1", Execute.self)
    setup = try symbol("connect_norito_kagemusha_wallet_setup_v1", Setup.self)
    requestStatus = try symbol("connect_norito_kagemusha_wallet_request_status_v1", Retry.self)
    retry = try symbol("connect_norito_kagemusha_wallet_retry_v1", Retry.self)
    resume = try symbol("connect_norito_kagemusha_wallet_resume_v1", Simple.self)
    fold = try symbol("connect_norito_kagemusha_wallet_fold_v1", Simple.self)
    credit = try symbol("connect_norito_kagemusha_wallet_credit_status_v1", Credit.self)
    snapshot = try symbol("connect_norito_kagemusha_wallet_snapshot_v1", Snapshot.self)
    guard
      let value = NoritoNativeBridge.shared.resolveNativeSymbol(
        "connect_norito_free", as: Free.self)
    else { throw KagemushaWalletErrorV1.bridgeUnavailable }
    free = value
  }
  static func check(_ status: Int32, reason: Int32 = -1, platform: Int32 = 0) throws {
    if status == 0 { return }
    if status == -2 { throw KagemushaWalletErrorV1.closed }
    if status == -4 { throw KagemushaWalletErrorV1.artifactsUnavailable }
    throw KagemushaWalletErrorV1.native(status: status, reason: reason, platformCode: platform)
  }
  func result(_ action: (UnsafeMutablePointer<Output>) -> Int32) throws -> KagemushaWalletCallV1 {
    var value = Output()
    let status = action(&value)
    defer { if let bytes = value.bytes { free(bytes) } }
    try Self.check(status, reason: value.reason, platform: value.platform_code)
    guard value.status >= 0, value.length >= 0, value.length <= (value.status == 17 ? 16_384 : 10_000), value.length == 0 || value.bytes != nil
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return try KagemushaWalletCallV1(
      status: value.status, sequenceLow: value.sequence_low, sequenceHigh: value.sequence_high,
      detail: value.detail,
      bytes: value.bytes.map { Data(bytes: $0, count: value.length) } ?? Data())
  }
}

func kagemushaWalletCallbacksV1(_ platform: KagemushaWalletApplePlatformV1)
  -> connect_norito_kagemusha_platform_v1
{
  var table = connect_norito_kagemusha_platform_v1()
  table.version = 1
  table.anchor_policy = 1
  table.context = Unmanaged.passUnretained(platform).toOpaque()
  table.retain = { pointer in
    if let pointer { _ = Unmanaged<KagemushaWalletApplePlatformV1>.fromOpaque(pointer).retain() }
  }
  table.release = { pointer in
    if let pointer { Unmanaged<KagemushaWalletApplePlatformV1>.fromOpaque(pointer).release() }
  }
  table.invoke = { context, operation, slot, input, length, auxiliary, output, capacity, reply in
    guard let reply else { return }
    reply.pointee = connect_norito_kagemusha_platform_reply_v1(
      tag: 2, reason: 4, code: 0, length: 0)
    guard let context else { return }
    let platform = Unmanaged<KagemushaWalletApplePlatformV1>.fromOpaque(context)
      .takeUnretainedValue()
    func failure(_ reason: KagemushaWalletAppleUnavailableV1, tag: UInt32 = 2) {
      reply.pointee.tag = tag
      switch reason {
      case .locked: reply.pointee.reason = 0
      case .beforeFirstUnlock: reply.pointee.reason = 1
      case .io(let code):
        reply.pointee.reason = 3
        reply.pointee.code = code
      case .platform(let code):
        reply.pointee.reason = 4
        reply.pointee.code = code
      case .keyUnusable: reply.pointee.reason = 5
      }
    }
    func success(_ bytes: Data = Data()) {
      guard bytes.count <= capacity, bytes.isEmpty || output != nil else {
        reply.pointee.tag = UInt32.max
        return
      }
      if let output, !bytes.isEmpty { bytes.copyBytes(to: output, count: bytes.count) }
      reply.pointee.tag = 0
      reply.pointee.length = bytes.count
    }
    func probe(_ result: KagemushaWalletAppleProbeV1<Data>) {
      switch result {
      case .present(let bytes): success(bytes)
      case .absent: reply.pointee.tag = 1
      case .unavailable(let reason): failure(reason)
      }
    }
    func publish(_ result: KagemushaWalletApplePublishOutcomeV1) {
      switch result {
      case .published: success()
      case .uncertain(let reason): failure(reason, tag: 4)
      case .notPublished(let reason):
        switch reason {
        case .destinationExists: reply.pointee.tag = 3
        case .destinationAbsent: reply.pointee.tag = 5
        case .noSpace: failure(.io(28))
        case .failed(let reason): failure(reason)
        }
      }
    }
    if operation == 10 {
      guard slot == nil, length == 0, auxiliary == 0 else { return }
      switch platform.keyEnumerate() {
      case .success(let slots):
        var bytes = Data()
        for slot in slots { bytes.append(slot.bytes) }
        success(bytes)
      case .failure(let reason): failure(reason)
      }
      return
    }
    if operation == 7 {
      switch platform.storageState() {
      case .success: success()
      case .failure(let reason): failure(reason)
      }
      return
    }
    if operation == 8 || operation == 9 {
      let value = operation == 8 ? platform.bootSessionUUID() : platform.custodyRootPath()
      switch value {
      case .success(let text): success(Data(text.utf8))
      case .failure(let reason): failure(reason)
      }
      return
    }
    guard let slot, let parsed = KagemushaWalletAppleSlotV1(Data(bytes: slot, count: 32)),
      length <= 256, length == 0 || input != nil
    else { return }
    let bytes = input.map { Data(bytes: $0, count: length) } ?? Data()
    switch operation {
    case 0: probe(platform.keyProbe(parsed))
    case 1:
      guard
        let profile = KagemushaWalletAppleKeyProfileV1(rawValue: UInt8(exactly: auxiliary) ?? 0),
        let request = KagemushaWalletAppleKeyGenerationRequestV1(
          challengeDigest: bytes, profile: profile)
      else { return }
      switch platform.keyGenerate(parsed, request) {
      case .generated(let key): success(key)
      case .alreadyPresent: reply.pointee.tag = 3
      case .unavailable(let reason): failure(reason)
      }
    case 2:
      switch platform.keySign(parsed, message: bytes) {
      case .success(let signature): success(signature)
      case .failure(let reason): failure(reason)
      }
    case 3:
      switch platform.keyDelete(parsed) {
      case .removed: success()
      case .notRemoved(let reason): failure(reason)
      case .uncertain(let reason): failure(reason, tag: 4)
      }
    case 4: probe(platform.anchorRead(parsed))
    case 5: publish(platform.anchorCreate(parsed, value: bytes))
    case 6: publish(platform.anchorUpdate(parsed, value: bytes))
    default: break
    }
  }
  return table
}
