import Foundation
import NoritoBridge

/// Native-authenticated review DATA. Copying it creates no capability or monetary operation.
public struct KagemushaWalletReviewProjectionV1: Sendable {
  static let fixedByteCount = 495
  static let maximumByteCount = fixedByteCount + 4_096
  public enum Kind: Sendable { case send, unload }
  public let kind: Kind
  public let amount: KagemushaWalletUInt128V1
  public let fee: KagemushaWalletUInt128V1
  public let grossDebit: KagemushaWalletUInt128V1
  public let netDestinationAmount: KagemushaWalletUInt128V1
  private let retained: Data
  init(_ original: Data) throws {
    guard original.count >= Self.fixedByteCount, original.count <= Self.maximumByteCount
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    let bytes = [UInt8](original)
    guard Array(bytes[0..<8]) == [75,87,79,82,86,49,0,0], [1,8].contains(bytes[8])
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    func word(_ offset: Int) -> Bool { bytes[offset..<(offset+32)].contains(where: { $0 != 0 }) }
    func scalar(_ offset: Int) -> KagemushaWalletUInt128V1 {
      func limb(_ at: Int) -> UInt64 { var value: UInt64 = 0; for i in 0..<8 { value |= UInt64(bytes[at+i]) << (8*i) }; return value }
      return .init(low: limb(offset), high: limb(offset+8))
    }
    kind = bytes[8] == 1 ? .send : .unload
    var accountLength: UInt32 = 0
    for i in 0..<4 { accountLength |= UInt32(bytes[491+i]) << (8*i) }
    guard accountLength <= 4_096, bytes.count == Self.fixedByteCount + Int(accountLength),
      kind == .send ? accountLength > 0 : accountLength == 0
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    amount = scalar(9); fee = scalar(25); grossDebit = scalar(41); netDestinationAmount = scalar(57)
    guard amount.low != 0 || amount.high != 0, bytes[426] == 4,
      bytes[427..<491].contains(where: { $0 != 0 }),
      [106,202,234,266,298,330,362,394].allSatisfy(word),
      kind == .send ? (bytes[73] == 1 && word(74) && word(138) && !word(170)) : (bytes[73] == 0 && !word(74) && !word(138))
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    retained = Data(original)
  }
  /// Exact whole projection for fresh hardware confirmation. It is never reassembled locally.
  public var bytes: Data { retained }
  public var receiverWalletID: Data? { kind == .send ? retained.subdata(in: 74..<106) : nil }
  private func word(_ index: Int) -> Data { retained.subdata(in: (106+index*32)..<(138+index*32)) }
  public var destinationAccountDigest: Data { word(0) }
  /// Exact Native-authenticated canonical AccountId original for Send; absent for Unload.
  /// Render only under independently authenticated installed network presentation selection.
  public var destinationAccountOriginal: Data? {
    kind == .send ? retained.subdata(in: Self.fixedByteCount..<retained.count) : nil
  }
  public var requestDigest: Data { word(1) }
  public var chargeQuoteDigest: Data { word(2) }
  public var schemeID: Data { word(3) }
  public var walletID: Data { word(4) }
  public var currentHead: Data { word(5) }
  public var sourceStateCommitment: Data { word(6) }
  public var sourceCapsuleDigest: Data { word(7) }
  public var credentialDigest: Data { word(8) }
  public var artifactManifestDigest: Data { word(9) }
  public var paymentPublicKey: Data { retained.subdata(in: 426..<491) }
}

/// Move-only owner-local Native capability; a projection cannot reconstruct one.
public final class KagemushaWalletReviewV1: @unchecked Sendable {
  private let lock = NSLock()
  private let origin: AnyObject
  private var token: UInt64
  public let requestID: Data
  public let projection: KagemushaWalletReviewProjectionV1
  private init(origin: AnyObject, token: UInt64, requestID: Data, projection: KagemushaWalletReviewProjectionV1) {
    self.origin = origin; self.token = token; self.requestID = Data(requestID); self.projection = projection
  }
  static func fromNative(origin: AnyObject, token: UInt64, requestID: Data, projection: KagemushaWalletReviewProjectionV1) -> KagemushaWalletReviewV1 {
    .init(origin: origin, token: token, requestID: requestID, projection: projection)
  }
  func consume(origin: AnyObject) throws -> UInt64 {
    lock.lock(); defer { lock.unlock() }
    guard self.origin === origin else { throw KagemushaWalletErrorV1.invalidInput }
    guard token != 0 else { throw KagemushaWalletErrorV1.closed }
    let value = token; token = 0; return value
  }
  /// Only the exact typed Native preflight response can restore the same consumed token.
  /// A generic failure or arbitrary display data never makes a review reusable.
  func retainAfterPreflight(origin: AnyObject, consumed: UInt64,
    result: KagemushaWalletCallV1) throws {
    guard result.status == 65, result.sequenceLow == consumed,
      result.sequenceHigh == 0, result.bytes.isEmpty,
      KagemushaWalletReviewMaintenanceWaitV1(rawValue: result.detail) != nil else {
      throw KagemushaWalletErrorV1.invalidNativeOutput
    }
    lock.lock(); defer { lock.unlock() }
    guard self.origin === origin, token == 0 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    token = consumed
  }
}

/// Comparison DATA only. Native independently authenticates current source and exact absence.
public enum KagemushaWalletReviewExpectationV1: Sendable {
  case fresh
  case originalUnstarted(Data)
  func original(kind: KagemushaWalletReviewProjectionV1.Kind) throws -> Data {
    switch self {
    case .fresh: return Data()
    case .originalUnstarted(let bytes):
      guard let projection = try? KagemushaWalletReviewProjectionV1(bytes), projection.kind == kind
      else { throw KagemushaWalletErrorV1.invalidInput }
      return projection.bytes
    }
  }
}

/// Closed financial-review maintenance observations. No value grants dispatch permission.
public enum KagemushaWalletReviewMaintenanceWaitV1: UInt32, Sendable {
  case selectedPending = 1, archiveFold = 2, busy = 6
}

/// An already bound request. Absence is intentionally not representable here.
public enum KagemushaWalletBoundRequestV1: Sendable {
  case preparing
  case outcome(KagemushaWalletCallV1)
  init(_ result: KagemushaWalletCallV1) throws {
    guard result.status == 66, result.sequenceLow == 0, result.sequenceHigh == 0,
      result.detail == 0, (28...10_062).contains(result.bytes.count)
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    let bytes = [UInt8](result.bytes)
    func u32(_ at: Int) -> UInt32 { (0..<4).reduce(0) { $0 | UInt32(bytes[at+$1]) << (8*$1) } }
    func u64(_ at: Int) -> UInt64 { (0..<8).reduce(0) { $0 | UInt64(bytes[at+$1]) << (8*$1) } }
    let kind = u32(0), detail = u32(4), low = u64(8), high = u64(16), count = u32(24)
    guard [1,2,3,4,5,10,11].contains(kind), count <= 10_034, bytes.count == 28+Int(count),
      low == 0, high == 0 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    if kind == 11 {
      guard detail == 0, count == 0 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      self = .preparing
    } else {
      guard kind == 3 ? [0,2].contains(detail) : detail == 0 else {
        throw KagemushaWalletErrorV1.invalidNativeOutput
      }
      self = .outcome(try KagemushaWalletCallV1(status: Int32(kind), sequenceLow: low,
        sequenceHigh: high, detail: detail, bytes: Data(bytes.dropFirst(28))).completion())
    }
  }
}

/// Only Ready contains a new actual capability; existing work and waits contain no token.
public enum KagemushaWalletReviewDecisionV1: Sendable {
  case ready(KagemushaWalletReviewV1)
  case existing(KagemushaWalletBoundRequestV1)
  case waiting(KagemushaWalletReviewMaintenanceWaitV1)
}

/// A historical outcome is separate from execution and cannot provide a new timing sample.
public enum KagemushaWalletReviewedExecutionResultV1: Sendable {
  case executed(KagemushaWalletCallV1)
  case existing(KagemushaWalletBoundRequestV1)
  case retainedReview(KagemushaWalletReviewMaintenanceWaitV1)
  init(_ result: KagemushaWalletCallV1) throws {
    switch result.status {
    case 66: self = .existing(try .init(result))
    case 65:
      guard let wait = KagemushaWalletReviewMaintenanceWaitV1(rawValue: result.detail) else {
        throw KagemushaWalletErrorV1.invalidNativeOutput
      }
      self = .retainedReview(wait)
    case 1,2,3,4,5,10:
      guard result.sequenceLow == 0, result.sequenceHigh == 0,
        result.status == 3 ? [0,2].contains(result.detail) : result.detail == 0 else {
        throw KagemushaWalletErrorV1.invalidNativeOutput
      }
      self = .executed(try result.completion())
    default: throw KagemushaWalletErrorV1.invalidNativeOutput
    }
  }
}

/// Financial review18 has its own geometry, distinct from Enrollment18 and completion.
struct KagemushaWalletReviewReplyV1 {
  let status: Int32, reason: Int32, platformCode: Int32
  let sequenceLow: UInt64, sequenceHigh: UInt64
  let detail: UInt32
  let bytes: Data
  func decision(origin: AnyObject, requestID: Data,
    expected: KagemushaWalletReviewProjectionV1.Kind) throws -> KagemushaWalletReviewDecisionV1 {
    guard reason == -1, platformCode == 0, requestID.count == 32,
      requestID.contains(where: { $0 != 0 }) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    if status == 18 {
      guard sequenceLow > 0, sequenceLow <= UInt64(Int64.max), sequenceHigh == 0, detail == 0
      else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      let projection = try KagemushaWalletReviewProjectionV1(bytes)
      guard projection.kind == expected else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return .ready(.fromNative(origin: origin, token: sequenceLow, requestID: requestID, projection: projection))
    }
    let result = try KagemushaWalletCallV1(status: status, sequenceLow: sequenceLow,
      sequenceHigh: sequenceHigh, detail: detail, bytes: bytes)
    if status == 66 { return .existing(try .init(result)) }
    if status == 67, let wait = KagemushaWalletReviewMaintenanceWaitV1(rawValue: detail) {
      return .waiting(wait)
    }
    throw KagemushaWalletErrorV1.invalidNativeOutput
  }
}

/// Fixed foreign bounds/copies only; Native authenticates all financial inputs.
struct KagemushaWalletReviewInputV1 {
  let selector: UInt32
  let requestID: Data
  let expectedProjection: Data
  let amount: KagemushaWalletUInt128V1
  let first: Data
  let second: Data
  init(requestID: Data, expectation: KagemushaWalletReviewExpectationV1, selector: UInt32, amount: KagemushaWalletUInt128V1 = .init(low: 0, high: 0),
       first: Data = Data(), second: Data = Data()) throws {
    guard requestID.count == 32, requestID.contains(where: { $0 != 0 }) else {
      throw KagemushaWalletErrorV1.invalidInput
    }
    let nonzero = amount.low != 0 || amount.high != 0
    switch selector {
    case 1:
      guard !nonzero, !first.isEmpty, first.count <= 10_034,
        !second.isEmpty, second.count <= 4_096
      else { throw KagemushaWalletErrorV1.invalidInput }
    case 8:
      guard nonzero, first.count <= 1_024, first.isEmpty == second.isEmpty
      else { throw KagemushaWalletErrorV1.invalidInput }
      if !second.isEmpty { try KagemushaWalletUnloadChargeReviewV1.validate(second) }
    default: throw KagemushaWalletErrorV1.invalidInput
    }
    self.requestID = Data(requestID)
    expectedProjection = try expectation.original(kind: selector == 1 ? .send : .unload)
    self.selector = selector; self.amount = amount; self.first = first; self.second = second
  }
  func withRequest<T>(_ body: (UnsafePointer<connect_norito_kagemusha_wallet_review_request_v1>) throws -> T) rethrows -> T {
    try requestID.withUnsafeBytes { id in try expectedProjection.withUnsafeBytes { expected in
    try first.withUnsafeBytes { a in try second.withUnsafeBytes { b in
      var request = connect_norito_kagemusha_wallet_review_request_v1()
      request.request_id32 = id.bindMemory(to: UInt8.self).baseAddress
      request.expectation = expectedProjection.isEmpty ? 0 : 1
      request.expected_projection = expected.bindMemory(to: UInt8.self).baseAddress
      request.expected_projection_length = expected.count
      request.selector = selector; request.amount = .init(low: amount.low, high: amount.high)
      request.first = a.bindMemory(to: UInt8.self).baseAddress; request.first_length = a.count
      request.second = b.bindMemory(to: UInt8.self).baseAddress; request.second_length = b.count
      return try body(&request)
    }}}}
  }
}
