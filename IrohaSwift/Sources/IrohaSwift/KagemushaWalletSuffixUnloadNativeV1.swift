import Foundation
import NoritoBridge

extension KagemushaWalletSuffixUnloadRequestV1 {
  func withNativeRequest<T>(_ body: (UnsafePointer<connect_norito_kagemusha_wallet_suffix_unload_request_v1>) throws -> T) rethrows -> T {
    try requestID.withUnsafeBytes { identity in try (quote ?? Data()).withUnsafeBytes { quote in
      try (certificates ?? Data()).withUnsafeBytes { certificates in try (beneficiary ?? Data()).withUnsafeBytes { beneficiary in
        var value = connect_norito_kagemusha_wallet_suffix_unload_request_v1()
        value.request_id32 = identity.bindMemory(to: UInt8.self).baseAddress
        value.amount = .init(low: amount.low, high: amount.high)
        value.charge_quote = quote.bindMemory(to: UInt8.self).baseAddress; value.charge_quote_length = quote.count
        value.charge_certificates = certificates.bindMemory(to: UInt8.self).baseAddress; value.charge_certificates_length = certificates.count
        value.charge_beneficiary = beneficiary.bindMemory(to: UInt8.self).baseAddress; value.charge_beneficiary_length = beneficiary.count
        return try body(&value)
      }}
    }}
  }
}

extension KagemushaWalletNativeDriverV1 {
  /// Validate the separate suffix geometry before copying any foreign bytes.
  func suffixResult(_ action: (UnsafeMutablePointer<Output>) -> Int32) throws -> KagemushaWalletSuffixUnloadReplyV1 {
    try KagemushaWalletNativeWorkerV1.perform {
      var value = Output()
      let status = action(&value)
      defer { if let bytes = value.bytes { free(bytes) } }
      guard status == 0 ? value.status >= 0 : (status < 0 && value.status == status),
        value.length >= 0, value.length <= KagemushaWalletSuffixUnloadReplyV1.maximumBytes(value.status),
        value.length == 0 || value.bytes != nil else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      let reply = try KagemushaWalletSuffixUnloadReplyV1(status: value.status, reason: value.reason,
        platformCode: value.platform_code, sequenceLow: value.sequence_low, sequenceHigh: value.sequence_high,
        detail: value.detail, bytes: value.bytes.map { Data(bytes: $0, count: value.length) } ?? Data())
      try reply.checked(); return reply
    }
  }
}

private func suffixRequestIdentity(_ value: Data) throws -> Data {
  guard value.count == 32, value.contains(where: { $0 != 0 }) else { throw KagemushaWalletErrorV1.invalidInput }
  return Data(value)
}
private func suffixSignedWire(_ value: Data) throws -> Data {
  guard (1...KagemushaWalletSuffixUnloadV1.signedWireMaximumBytes).contains(value.count)
  else { throw KagemushaWalletErrorV1.invalidInput }
  return Data(value)
}

extension KagemushaWalletSuffixUnloadReplyV1 {
  func ledgerOutcome() throws -> KagemushaWalletLedgerOutcomeV1 {
    try checked()
    switch status {
    case 46: return .awaiting(entrypoint: bytes)
    case 69: return .rejected(.init(height: sequenceLow, commitHash: bytes))
    case 70: return .confirmed(.init(height: sequenceLow, commitHash: bytes))
    case 76: return .stageConfirmed(.init(height: sequenceLow, commitHash: bytes))
    default: throw KagemushaWalletErrorV1.invalidNativeOutput
    }
  }
}

extension KagemushaWalletV1 {
  /// Cheap authenticated scheduling DATA. This does not scan or verify a suffix.
  public func suffixUnloadStatus() throws -> KagemushaWalletSuffixUnloadStatusV1 {
    let owner = try handle()
    let result = try driver.suffixResult { driver.suffixStatus(owner, $0) }
    guard result.status == 77 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    let status = try KagemushaWalletSuffixUnloadStatusV1(result.bytes)
    guard status.state != .requestAbsent else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return status
  }
  /// Exact retained canonical stage. Begin is available after complete preparation, before debit.
  public func suffixUnloadInstruction(requestId: Data, ordinal: KagemushaWalletUInt128V1) throws -> Data {
    let identity = try suffixRequestIdentity(requestId), owner = try handle()
    let result = try driver.suffixResult { out in identity.withUnsafeBytes {
      driver.suffixInstruction(owner, $0.bindMemory(to: UInt8.self).baseAddress, ordinal.low, ordinal.high, out)
    }}
    guard result.status == 75 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return result.bytes
  }
  /// Read a retained envelope by index. Nil means absent, never corrupt or unavailable custody.
  public func suffixUnloadSignedWire(requestId: Data, ordinal: KagemushaWalletUInt128V1, attemptIndex: UInt64) throws -> Data? {
    let identity = try suffixRequestIdentity(requestId), owner = try handle()
    let result = try driver.suffixResult { out in identity.withUnsafeBytes {
      driver.suffixSignedWire(owner, $0.bindMemory(to: UInt8.self).baseAddress, ordinal.low, ordinal.high, attemptIndex, out)
    }}
    guard result.status == 78 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return result.bytes.isEmpty ? nil : result.bytes
  }
  /// Retain and bind this exact signed stage before its first external dispatch.
  public func suffixUnloadAttemptOutcome(requestId: Data, ordinal: KagemushaWalletUInt128V1,
    signedWire: Data) throws -> KagemushaWalletLedgerOutcomeV1 {
    let identity = try suffixRequestIdentity(requestId), wire = try suffixSignedWire(signedWire), owner = try handle()
    return try driver.suffixResult { out in identity.withUnsafeBytes { id in wire.withUnsafeBytes {
      driver.suffixAttempt(owner, id.bindMemory(to: UInt8.self).baseAddress, ordinal.low, ordinal.high,
        $0.bindMemory(to: UInt8.self).baseAddress, $0.count, out)
    }}}.ledgerOutcome()
  }
}

extension KagemushaWalletExchangeCallV1 {
  /// One bounded verification turn. Preparation never approves or debits.
  public func prepareSuffixUnload(_ request: KagemushaWalletSuffixUnloadRequestV1) throws -> KagemushaWalletSuffixUnloadDecisionV1 {
    try task.withUse {
      let owner = try handle()
      return try suffixDecision(requestID: request.requestID) { out in
        request.withNativeRequest { driver.suffixPrepare(owner, exchange.token, $0, out) }
      }
    }
  }
  /// Continue this retained request, including after interrupted Advance.
  public func resumeSuffixUnload(requestId: Data) throws -> KagemushaWalletSuffixUnloadDecisionV1 {
    let identity = try suffixRequestIdentity(requestId)
    return try task.withUse {
      let owner = try handle()
      return try suffixDecision(requestID: identity, allowAbsent: true) { out in identity.withUnsafeBytes {
        driver.suffixResume(owner, exchange.token, $0.bindMemory(to: UInt8.self).baseAddress, out)
      }}
    }
  }
  private func suffixDecision(requestID: Data, allowAbsent: Bool = false, action: (UnsafeMutablePointer<KagemushaWalletNativeDriverV1.Output>) -> Int32) throws -> KagemushaWalletSuffixUnloadDecisionV1 {
    let owner = try handle(); var issued: UInt64 = 0
    do {
      let result = try driver.suffixResult { out in
        let status = action(out)
        if out.pointee.status == 73 { issued = out.pointee.sequence_low }
        return status
      }
      return try result.decision(origin: exchange, requestID: requestID, allowAbsent: allowAbsent)
    } catch KagemushaWalletErrorV1.invalidNativeOutput {
      do {
        if issued == 0 { try wallet.close() }
        else {
          let cleanup = driver.discardReview(owner, exchange.token, issued)
          if ![0, -1, -2].contains(cleanup) { try KagemushaWalletNativeDriverV1.check(cleanup) }
        }
      } catch { try wallet.close() }
      throw KagemushaWalletErrorV1.invalidNativeOutput
    }
  }
  /// Consume the exact issued review after hardware confirmation and certified Begin.
  public func executeReviewedSuffixUnload(_ review: KagemushaWalletSuffixUnloadReviewV1) throws -> KagemushaWalletSuffixUnloadExecutionV1 {
    try task.withUse {
      let owner = try handle(), consumed = try review.consume(origin: exchange)
      return try KagemushaWalletReviewedExecutionV1.run(execute: {
        let result = try driver.suffixResult { driver.suffixExecute(owner, exchange.token, consumed, $0) }
        return try result.execution(origin: exchange, review: review, consumed: consumed)
      }, discard: { driver.discardReview(owner, exchange.token, consumed) }, close: wallet.close)
    }
  }
  /// Authenticate the stage's direct finality; epoch synchronization stays in the generic scheduler.
  public func ingestSuffixUnloadFinality(requestId: Data, ordinal: KagemushaWalletUInt128V1,
    signedWire: Data, finality: Data) throws -> KagemushaWalletLedgerIngestV1 {
    let identity = try suffixRequestIdentity(requestId), wire = try suffixSignedWire(signedWire)
    guard (1...1_327_104).contains(finality.count) else { throw KagemushaWalletErrorV1.invalidInput }
    let original = Data(finality)
    return try task.withUse {
      let owner = try handle()
      let result = try driver.suffixResult { out in identity.withUnsafeBytes { id in
        wire.withUnsafeBytes { wire in original.withUnsafeBytes { proof in
          driver.suffixIngest(owner, exchange.token, id.bindMemory(to: UInt8.self).baseAddress,
            ordinal.low, ordinal.high, wire.bindMemory(to: UInt8.self).baseAddress, wire.count,
            proof.bindMemory(to: UInt8.self).baseAddress, proof.count, out)
        }}
      }}
      if result.status == 68 { return .epochRequired(targetHeight: result.sequenceLow) }
      let decided = try result.ledgerOutcome()
      if case .awaiting = decided { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return .outcome(decided)
    }
  }
}

extension KagemushaWalletExchangeV1 {
  /// Drop an unused exact-origin capability without creating an intent or debit.
  public func discardSuffixUnloadReview(_ review: KagemushaWalletSuffixUnloadReviewV1) throws {
    let owner = try wallet.cleanupHandle(), consumed = try review.consume(origin: self)
    try KagemushaWalletConsumedCleanupV1.run(cleanup: {
      try KagemushaWalletNativeDriverV1.check(wallet.driver.discardReview(owner, token, consumed))
    }, close: wallet.close, retain: { primary, cleanup in
      KagemushaWalletInstalledRuntimeV1.retain(primary, cleanup: cleanup, resource: self.wallet)
    })
  }
}


extension KagemushaWalletSuffixUnloadV1 {
  /// Wrap the exact bounded suffix action under the existing registered ledger wire identity.
  public static func instructionFrame(_ nativeFrame: Data) throws -> TransactionInstructionFrame {
    try TransactionInstructionFrame(wireName: instructionWireName, framedPayload: instructionOriginal(nativeFrame))
  }
}
