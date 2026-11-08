import Foundation

/// In-memory, one-use Native review, bound to the exact issuing exchange.
public final class KagemushaWalletSuffixUnloadReviewV1: @unchecked Sendable {
  private let lock = NSLock()
  private let origin: AnyObject
  private var token: UInt64
  public let projection: KagemushaWalletSuffixUnloadReviewProjectionV1
  public var requestID: Data { projection.requestID }
  private init(origin: AnyObject, token: UInt64, projection: KagemushaWalletSuffixUnloadReviewProjectionV1) {
    self.origin = origin; self.token = token; self.projection = projection
  }
  static func fromNative(origin: AnyObject, requestID: Data, reply: KagemushaWalletSuffixUnloadReplyV1) throws -> KagemushaWalletSuffixUnloadReviewV1 {
    guard reply.status == 73 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    let projection = try KagemushaWalletSuffixUnloadReviewProjectionV1(reply.bytes)
    guard projection.requestID == requestID else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return .init(origin: origin, token: reply.sequenceLow, projection: projection)
  }
  func consume(origin: AnyObject) throws -> UInt64 {
    lock.lock(); defer { lock.unlock() }
    guard self.origin === origin else { throw KagemushaWalletErrorV1.invalidInput }
    guard token != 0 else { throw KagemushaWalletErrorV1.closed }
    let selected = token; token = 0; return selected
  }
  func restore(origin: AnyObject, consumed: UInt64, reply: KagemushaWalletSuffixUnloadReplyV1) throws -> KagemushaWalletReviewMaintenanceWaitV1 {
    lock.lock(); defer { lock.unlock() }
    guard self.origin === origin, token == 0, consumed > 0, reply.status == 65,
      reply.sequenceLow == consumed, [1, 6].contains(reply.detail),
      let wait = KagemushaWalletReviewMaintenanceWaitV1(rawValue: reply.detail)
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    token = consumed; return wait
  }
}

/// Preparation never performs an irreversible debit or establishes online payout.
public enum KagemushaWalletSuffixUnloadDecisionV1: Sendable {
  /// Named recovery authenticated that this exact request has never been retained.
  case notStarted
  case ready(KagemushaWalletSuffixUnloadReviewV1)
  case verifying(KagemushaWalletSuffixUnloadProgressV1)
  case released(KagemushaWalletSuffixUnloadReleasedV1)
  case pending
  case notApplicable(reason: UInt32)
  case waiting(KagemushaWalletReviewMaintenanceWaitV1)
}

/// Released is an offline debit. Pending continues the same retained request.
public enum KagemushaWalletSuffixUnloadExecutionV1: Sendable {
  case released(KagemushaWalletSuffixUnloadReleasedV1)
  case pending
  case retainedReview(KagemushaWalletSuffixUnloadReviewV1, KagemushaWalletReviewMaintenanceWaitV1)
}

/// Route-specific geometry; ordinary wallet results never reinterpret these status tags.
struct KagemushaWalletSuffixUnloadReplyV1: Sendable {
  let status: Int32, reason: Int32, platformCode: Int32
  let sequenceLow: UInt64, sequenceHigh: UInt64
  let detail: UInt32
  let bytes: Data
  static func maximumBytes(_ status: Int32) -> Int {
    switch status {
    case 72: return 256
    case 73: return 12_288
    case 74, 77: return 512
    case 75: return KagemushaWalletSuffixUnloadV1.instructionMaximumBytes
    case 78: return KagemushaWalletSuffixUnloadV1.signedWireMaximumBytes
    case 46, 69, 70, 76: return 32
    default: return 0
    }
  }
  init(status: Int32, reason: Int32 = -1, platformCode: Int32 = 0,
    sequenceLow: UInt64 = 0, sequenceHigh: UInt64 = 0, detail: UInt32 = 0, bytes: Data = Data()) throws {
    guard bytes.count <= Self.maximumBytes(status) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    if status < 0 {
      guard sequenceLow == 0, sequenceHigh == 0, detail == 0, bytes.isEmpty
      else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    } else {
      guard reason == -1, platformCode == 0, sequenceHigh == 0
      else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      let valid: Bool
      let word = bytes.count == 32 && bytes.contains(where: { $0 != 0 })
      switch status {
      case 2: valid = sequenceLow == 0 && detail == 0 && bytes.isEmpty
      case 46: valid = sequenceLow == 0 && detail == 0 && word
      case 65: valid = sequenceLow > 0 && sequenceLow <= UInt64(Int64.max) && [1, 6].contains(detail) && bytes.isEmpty
      case 67: valid = sequenceLow == 0 && [1, 6].contains(detail) && bytes.isEmpty
      case 68: valid = sequenceLow != 0 && detail == 0 && bytes.isEmpty
      case 69, 70, 76: valid = sequenceLow >= 2 && detail == 0 && word
      case 71: valid = sequenceLow == 0 && (1...3).contains(detail) && bytes.isEmpty
      case 72, 74, 75, 77: valid = sequenceLow == 0 && detail == 0 && !bytes.isEmpty
      case 73: valid = sequenceLow > 0 && sequenceLow <= UInt64(Int64.max) && detail == 0 && !bytes.isEmpty
      case 78: valid = sequenceLow == 0 && detail == 0
      default: valid = false
      }
      guard valid else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    }
    self.status = status; self.reason = reason; self.platformCode = platformCode
    self.sequenceLow = sequenceLow; self.sequenceHigh = sequenceHigh; self.detail = detail
    self.bytes = Data(bytes)
  }
  func checked() throws {
    if status == -2 { throw KagemushaWalletErrorV1.closed }
    if status == -4 { throw KagemushaWalletErrorV1.artifactsUnavailable }
    if status < 0 { throw KagemushaWalletErrorV1.native(status: status, reason: reason, platformCode: platformCode) }
  }
  func decision(origin: AnyObject, requestID: Data, allowAbsent: Bool = false) throws -> KagemushaWalletSuffixUnloadDecisionV1 {
    try checked()
    switch status {
    case 2: return .pending
    case 67:
      guard let wait = KagemushaWalletReviewMaintenanceWaitV1(rawValue: detail) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return .waiting(wait)
    case 71: return .notApplicable(reason: detail)
    case 72:
      let progress = try KagemushaWalletSuffixUnloadProgressV1(bytes)
      guard progress.requestID == requestID else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return .verifying(progress)
    case 73: return .ready(try .fromNative(origin: origin, requestID: requestID, reply: self))
    case 77:
      let observation = try KagemushaWalletSuffixUnloadStatusV1(bytes)
      guard allowAbsent, observation.state == .requestAbsent, observation.requestID == requestID
      else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return .notStarted
    case 74:
      let summary = try KagemushaWalletSuffixUnloadReleasedV1(bytes)
      guard summary.requestID == requestID else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return .released(summary)
    default: throw KagemushaWalletErrorV1.invalidNativeOutput
    }
  }
  func execution(origin: AnyObject, review: KagemushaWalletSuffixUnloadReviewV1, consumed: UInt64) throws -> KagemushaWalletSuffixUnloadExecutionV1 {
    try checked()
    switch status {
    case 2: return .pending
    case 65: return .retainedReview(review, try review.restore(origin: origin, consumed: consumed, reply: self))
    case 74:
      let summary = try KagemushaWalletSuffixUnloadReleasedV1(bytes)
      let selected = review.projection
      guard summary.requestID == selected.requestID, summary.authorityDigest == selected.authorityDigest,
        summary.amount == selected.amount, summary.stepCount == selected.stepCount,
        summary.chunkCount == selected.chunkCount, summary.instructionCount == selected.instructionCount
      else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return .released(summary)
    default: throw KagemushaWalletErrorV1.invalidNativeOutput
    }
  }
}
