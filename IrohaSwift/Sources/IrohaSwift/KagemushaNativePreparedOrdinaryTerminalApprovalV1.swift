import Foundation

/// Distinct opaque W1 preparation from actual Native phase8. No purpose, key or cancel input.
public final class KagemushaNativePreparedOrdinaryTerminalApprovalV1: @unchecked Sendable {
  private let binding: KagemushaOrdinaryNativeBindingV1
  private let original: KagemushaOrdinaryTerminalProjectionV1
  private let lock = NSLock()
  private var unusable = false
  private init(binding: KagemushaOrdinaryNativeBindingV1, fields: [Data]) throws {
    self.binding = binding; original = try KagemushaOrdinaryTerminalProjectionV1(fields)
    _ = try recheck()
  }
  static func selected(binding: KagemushaOrdinaryNativeBindingV1, reserveKey: Data) throws
    -> KagemushaNativePreparedOrdinaryTerminalApprovalV1 {
    try KagemushaNativePreparedOrdinaryTerminalApprovalV1(binding: binding, fields: binding.outgoing(8, fields: [reserveKey]))
  }
  func recheck() throws -> KagemushaOrdinaryTerminalProjectionV1 {
    try guarded {
      try binding.requireOpen()
      let checked = try KagemushaOrdinaryTerminalProjectionV1(binding.outgoing(18))
      guard checked.fields == original.fields else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("held W1/key/counter original changed")
      }
      try binding.requireOpen()
      return checked
    }
  }
  func recover() throws -> (state: UInt8, raw: Data, wrapper: Data) {
    _ = try recheck()
    return try recovery(guarded { try binding.outgoing(11) })
  }
  func fence() throws -> (state: UInt8, raw: Data, wrapper: Data) {
    _ = try recheck()
    return try recovery(guarded { try binding.outgoing(9) })
  }
  func retainOriginal(_ raw: Data) throws {
    _ = try recheck()
    try guarded {
      let response = try binding.outgoing(10, fields: [raw])
      guard response[0] == KagemushaOrdinaryOutgoingFrameV1.sha(raw) else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("Native W1 retained another raw original")
      }
    }
    _ = try recheck()
  }
  func consumedReceipt(_ evidence: KagemushaAppAttestApprovalOriginalV1) throws
    -> KagemushaNativeOrdinaryTerminalApprovalReceiptV1 {
    let p = try recheck()
    let held = try recover()
    guard p.platform == 4, held.state == 2, held.raw == evidence.rawAssertion,
      evidence.clientDataHash == p.approval.clientDataHash,
      !held.wrapper.isEmpty else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("Native has not consumed this exact Apple W1 original")
    }
    _ = try recheck()
    return KagemushaNativeOrdinaryTerminalApprovalReceiptV1(projection: p, evidence: evidence, wrapper: held.wrapper)
  }
  private func recovery(_ fields: [Data]) throws -> (state: UInt8, raw: Data, wrapper: Data) {
    _ = try recheck()
    return (fields[0][0], Data(fields[1]), Data(fields[2]))
  }
  private func guarded<T>(_ body: () throws -> T) throws -> T {
    lock.lock(); defer { lock.unlock() }
    guard !unusable else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    do { return try body() }
    catch { unusable = true; try? binding.revoke(); throw error }
  }
}

/// Native-consumed W1 identity evidence, distinct from the purpose2 platform receipt.
/// The complete canonical approval wrapper is retained as DATA; it grants no committed money.
public struct KagemushaNativeOrdinaryTerminalApprovalReceiptV1: Sendable {
  public let operationID: Data
  public let keyID: Data
  public let keyAlias: String
  public let signingDigest: Data
  public let rawAssertionDigest: Data
  public let observedCounter: UInt32
  public let canonicalApprovalOriginal: Data
  fileprivate init(projection: KagemushaOrdinaryTerminalProjectionV1,
    evidence: KagemushaAppAttestApprovalOriginalV1, wrapper: Data) {
    operationID = projection.approval.operationID; keyID = projection.keyID; keyAlias = projection.keyAlias
    signingDigest = evidence.clientDataHash
    rawAssertionDigest = KagemushaOrdinaryOutgoingFrameV1.sha(evidence.rawAssertion)
    observedCounter = evidence.observedCounter; canonicalApprovalOriginal = Data(wrapper)
  }
}

/// No compatibility default: a store must actually persist this distinct consumed W1 original.
public protocol KagemushaAppAttestOrdinaryTerminalIntentStoringV1: KagemushaAppAttestAssertionIntentStoringV1 {
  func advanceAfterNativeTerminalApproval(keyID: String, counter: UInt32,
    signingDigest: Data, rawAssertion: Data, receipt: KagemushaNativeOrdinaryTerminalApprovalReceiptV1) throws
}
