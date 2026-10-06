import Foundation

/// Native wallet failure; uncertain outcomes require reconciliation, never a replacement debit.
public enum KagemushaWalletErrorV1: Error, Equatable, Sendable {
  case bridgeUnavailable, artifactsUnavailable, invalidInput, invalidNativeOutput, closed
  case native(status: Int32, reason: Int32, platformCode: Int32)
}

/// Explicit custody result with original canonical bytes.
public struct KagemushaWalletCallV1: Sendable {
  /// Unknown0, complete1, pending2, not performed3, archived4, delivery loss5,
  /// idle6, caught up7, checkpoint8, folded9, CreditStatus10.
  public let status: Int32
  public let sequenceLow: UInt64
  public let sequenceHigh: UInt64
  /// Checkpoint ordinal; for not-performed: stale0, capacity1, invalid2.
  public let detail: UInt32
  /// Exact retained bytes; present only for complete1 or CreditStatus10.
  public let bytes: Data
  init(status: Int32, sequenceLow: UInt64, sequenceHigh: UInt64, detail: UInt32, bytes: Data) throws
  {
    guard (0...10).contains(status), bytes.count <= 10_000,
      (status == 1 || status == 10) ? !bytes.isEmpty : bytes.isEmpty
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    self.status = status
    self.sequenceLow = sequenceLow
    self.sequenceHigh = sequenceHigh
    self.detail = detail
    self.bytes = bytes
  }
}

#if canImport(NoritoBridge)
  import NoritoBridge

  /// Exclusive native owner. Use worker threads for proving. Activity/payment calls can preempt
  /// folding because no Swift lock is held across native calls. Closing preserves committed bytes.
  /// Open currently throws artifactsUnavailable until the authenticated operation/Λ/Ω loader exists.
  public final class KagemushaWalletV1: @unchecked Sendable {
    private let lock = NSLock()
    private var owner: UInt64
    private let driver: KagemushaWalletNativeDriverV1
    /// Open one enrolled incarnation with its authenticated native artifact identity.
    public init(
      platform: KagemushaWalletApplePlatformV1, slot: Data, scheme: Data, wallet: Data,
      artifact: Data
    ) throws {
      for value in [slot, scheme, wallet, artifact] {
        guard value.count == 32, value.contains(where: { $0 != 0 }) else {
          throw KagemushaWalletErrorV1.invalidInput
        }
      }
      let driver = try KagemushaWalletNativeDriverV1()
      var table = kagemushaWalletCallbacksV1(platform)
      var handle: UInt64 = 0
      let status = withExtendedLifetime(platform) {
        slot.withUnsafeBytes { a in
          scheme.withUnsafeBytes { b in
            wallet.withUnsafeBytes { c in
              artifact.withUnsafeBytes { d in
                driver.open(
                  &table, a.bindMemory(to: UInt8.self).baseAddress,
                  b.bindMemory(to: UInt8.self).baseAddress,
                  c.bindMemory(to: UInt8.self).baseAddress,
                  d.bindMemory(to: UInt8.self).baseAddress, &handle)
              }
            }
          }
        }
      }
      try KagemushaWalletNativeDriverV1.check(status)
      guard handle > 0 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
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
    /// Enable folding while foreground or charging; neither state cancels a running checkpoint.
    public func setActivity(foreground: Bool, charging: Bool) throws {
      try KagemushaWalletNativeDriverV1.check(
        driver.activity(try handle(), foreground ? 1 : 0, charging ? 1 : 0))
    }
    /// Commit canonical Norito FrozenTransition; native verification precedes Advance.
    public func commit(_ frozen: Data) throws -> KagemushaWalletCallV1 {
      guard frozen.count <= 264_192 else { throw KagemushaWalletErrorV1.invalidInput }
      let value = try handle()
      return try driver.result { out in
        frozen.withUnsafeBytes {
          driver.commit(value, $0.bindMemory(to: UInt8.self).baseAddress, $0.count, out)
        }
      }
    }
    /// Return original retained output without a second signature, proof or debit.
    public func retry(operationId: Data) throws -> KagemushaWalletCallV1 {
      guard operationId.count == 32 else { throw KagemushaWalletErrorV1.invalidInput }
      let value = try handle()
      return try driver.result { out in
        operationId.withUnsafeBytes {
          driver.retry(value, $0.bindMemory(to: UInt8.self).baseAddress, out)
        }
      }
    }
    /// Reconcile and finish the selected operation.
    public func resume() throws -> KagemushaWalletCallV1 {
      let value = try handle()
      return try driver.result { driver.resume(value, $0) }
    }
    /// Compute/persist at most one checkpoint or final Ω.
    public func foldOnce() throws -> KagemushaWalletCallV1 {
      let value = try handle()
      return try driver.result { driver.fold(value, $0) }
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
      }
    }
  }

  private final class KagemushaWalletNativeDriverV1: @unchecked Sendable {
    typealias Output = connect_norito_kagemusha_wallet_result_v1
    typealias Open =
      @convention(c) (
        UnsafePointer<connect_norito_kagemusha_platform_v1>?, UnsafePointer<UInt8>?,
        UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?,
        UnsafeMutablePointer<UInt64>?
      ) -> Int32
    typealias Close = @convention(c) (UInt64) -> Int32
    typealias Activity = @convention(c) (UInt64, UInt8, UInt8) -> Int32
    typealias Commit =
      @convention(c) (UInt64, UnsafePointer<UInt8>?, Int, UnsafeMutablePointer<Output>?) -> Int32
    typealias Retry =
      @convention(c) (UInt64, UnsafePointer<UInt8>?, UnsafeMutablePointer<Output>?) -> Int32
    typealias Simple = @convention(c) (UInt64, UnsafeMutablePointer<Output>?) -> Int32
    typealias Credit =
      @convention(c) (
        UInt64, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, UnsafeMutablePointer<Output>?
      ) -> Int32
    typealias Free = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
    let open: Open
    let close: Close
    let activity: Activity
    let commit: Commit
    let retry: Retry
    let resume: Simple
    let fold: Simple
    let credit: Credit
    let free: Free
    init() throws {
      func symbol<T>(_ suffix: String, _ type: T.Type) throws -> T {
        guard
          let value = NoritoNativeBridge.shared.resolveNativeSymbol(
            "connect_norito_kagemusha_wallet_\(suffix)_v1", as: type)
        else { throw KagemushaWalletErrorV1.bridgeUnavailable }
        return value
      }
      let revision = try symbol("revision", (@convention(c) () -> UInt32).self)
      guard revision() == 1 else { throw KagemushaWalletErrorV1.bridgeUnavailable }
      open = try symbol("open", Open.self)
      close = try symbol("close", Close.self)
      activity = try symbol("activity", Activity.self)
      commit = try symbol("commit", Commit.self)
      retry = try symbol("retry", Retry.self)
      resume = try symbol("resume", Simple.self)
      fold = try symbol("fold", Simple.self)
      credit = try symbol("credit_status", Credit.self)
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
      guard value.status >= 0, value.length <= 10_000, value.length == 0 || value.bytes != nil
      else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return try KagemushaWalletCallV1(
        status: value.status, sequenceLow: value.sequence_low, sequenceHigh: value.sequence_high,
        detail: value.detail,
        bytes: value.bytes.map { Data(bytes: $0, count: value.length) } ?? Data())
    }
  }

  private func kagemushaWalletCallbacksV1(_ platform: KagemushaWalletApplePlatformV1)
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
#endif
