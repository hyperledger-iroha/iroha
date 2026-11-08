import Foundation
import CryptoKit

/// Suffix-stage bounds. Ordinary wallet instructions and peer Payments keep their own bounds.
public enum KagemushaWalletSuffixUnloadV1 {
  public static let instructionWireName = "iroha.kagemusha.wallet.ledger.v1"
  public static let chunkMaximumBytes = 528_416
  public static let instructionMaximumBytes = chunkMaximumBytes + 16_384
  public static let signedWireMaximumBytes = chunkMaximumBytes + 65_536
  /// Exact bounded stage carriage; ordinary actions cannot select the suffix size allowance.
  /// Native and ledger still authenticate every nested original.
  public static func instructionOriginal(_ original: Data) throws -> Data {
    var r = try SuffixFields.canonical(original,
      schema: "iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLedgerV1", maximum: instructionMaximumBytes)
    _ = try r.identity()
    try r.field { action in
      let tag = try action.u32()
      guard (11...13).contains(tag) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      _ = try action.identity(); _ = try action.identity()
      let account = try action.field { try $0.remainder(maximum: 4_096) }
      let ordinal = try action.scalar()
      guard !account.isEmpty, ordinal.high == 0,
        tag == 11 ? ordinal.low == 0 : (tag == 12 ? (1...512).contains(ordinal.low) : (2...513).contains(ordinal.low))
      else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      guard try !action.field({ try $0.vector(maximum: instructionMaximumBytes) }).isEmpty
      else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      if tag == 11 {
        guard try !action.field({ try $0.vector(maximum: instructionMaximumBytes) }).isEmpty
        else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      }
    }
    try r.finish(); return Data(original)
  }
}

/// Immutable request DATA. Native selects the beneficiary account and owns all monetary admission.
public struct KagemushaWalletSuffixUnloadRequestV1: Sendable {
  public let requestID: Data
  public let amount: KagemushaWalletUInt128V1
  public let quote: Data?
  public let certificates: Data?
  public let beneficiary: Data?
  public init(requestID: Data, amount: KagemushaWalletUInt128V1,
    quote: Data? = nil, certificates: Data? = nil, beneficiary: Data? = nil) throws {
    guard requestID.count == 32, requestID.contains(where: { $0 != 0 }), amount.low != 0 || amount.high != 0,
      (quote == nil) == (certificates == nil), (quote == nil) == (beneficiary == nil)
    else { throw KagemushaWalletErrorV1.invalidInput }
    if let quote, let certificates, let beneficiary {
      guard (1...1_024).contains(quote.count), (1...10_034).contains(certificates.count),
        (1...4_096).contains(beneficiary.count) else { throw KagemushaWalletErrorV1.invalidInput }
    }
    self.requestID = Data(requestID); self.amount = amount
    self.quote = quote.map(Data.init); self.certificates = certificates.map(Data.init)
    self.beneficiary = beneficiary.map(Data.init)
  }
}

/// Exact native-selected display DATA. Copying it cannot create an executable review.
public struct KagemushaWalletSuffixUnloadReviewProjectionV1: Sendable {
  public let requestID: Data
  public let authorityDigest: Data
  public let amount: KagemushaWalletUInt128V1
  public let charge: KagemushaWalletUInt128V1
  public var fee: KagemushaWalletUInt128V1 { charge }
  public let grossDebit: KagemushaWalletUInt128V1
  public let netDestinationAmount: KagemushaWalletUInt128V1
  public let stepCount: KagemushaWalletUInt128V1
  public let chunkCount: KagemushaWalletUInt128V1
  public let instructionCount: KagemushaWalletUInt128V1
  public let destinationAccountDigest: Data
  public let destinationAccountOriginal: Data
  public let bytes: Data
  init(_ original: Data) throws {
    var r = try SuffixFields.frame(original, name: "SuffixUnloadReviewProjectionV1", tag: 13, maximum: 12_288)
    requestID = try r.identity()
    _ = try r.identity(); _ = try r.identity()
    authorityDigest = try r.identity()
    _ = try r.identity(); _ = try r.identity(); _ = try r.identity()
    let sequence = try r.scalar(), anchor = try r.scalar()
    stepCount = try r.scalar(); chunkCount = try r.scalar(); instructionCount = try r.scalar()
    try suffixCounts(stepCount, chunkCount, instructionCount)
    guard suffixAdd(anchor, stepCount) == sequence else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    amount = try r.scalar(); charge = try r.scalar(); grossDebit = try r.scalar(); netDestinationAmount = try r.scalar()
    guard amount != .init(low: 0, high: 0), grossDebit == amount,
      suffixSubtract(amount, charge) == netDestinationAmount else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    destinationAccountDigest = try r.identity()
    destinationAccountOriginal = try r.field { try $0.vector(maximum: 4_096) }
    guard !destinationAccountOriginal.isEmpty else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    let quote = try r.word()
    let beneficiary = try r.optional { try $0.vector(maximum: 4_096) }
    let quoted = quote.contains(where: { $0 != 0 })
    guard quoted == (beneficiary != nil), quoted || charge == .init(low: 0, high: 0),
      beneficiary == nil || beneficiary?.isEmpty == false else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    _ = try r.identity(); _ = try r.identity()
    let key = try r.field { try $0.raw(65) }
    guard key.first == 4, (try? P256.Signing.PublicKey(x963Representation: key)) != nil
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    try r.finish(); bytes = Data(original)
  }
}

/// Bounded progress of native verification, without approval or settlement meaning.
public struct KagemushaWalletSuffixUnloadProgressV1: Sendable {
  public let requestID: Data
  public let completedSteps: KagemushaWalletUInt128V1
  public let totalSteps: KagemushaWalletUInt128V1
  init(_ original: Data) throws {
    var r = try SuffixFields.frame(original, name: "SuffixUnloadProgressV1", tag: 14, maximum: 256)
    requestID = try r.identity(); completedSteps = try r.scalar(); totalSteps = try r.scalar()
    try r.finish(); try suffixProgress(completedSteps, totalSteps)
  }
}

/// Durable offline debit and immutable stages. Only final ledger confirmation proves payout.
public struct KagemushaWalletSuffixUnloadReleasedV1: Sendable {
  public let requestID: Data
  public let authorityDigest: Data
  public let amount: KagemushaWalletUInt128V1
  public let stepCount: KagemushaWalletUInt128V1
  public let chunkCount: KagemushaWalletUInt128V1
  public let instructionCount: KagemushaWalletUInt128V1
  init(_ original: Data) throws {
    var r = try SuffixFields.frame(original, name: "SuffixUnloadReleasedV1", tag: 16, maximum: 512)
    requestID = try r.identity(); authorityDigest = try r.identity(); amount = try r.scalar()
    stepCount = try r.scalar(); chunkCount = try r.scalar(); instructionCount = try r.scalar()
    try r.finish(); try suffixCounts(stepCount, chunkCount, instructionCount)
    guard amount != .init(low: 0, high: 0) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
  }
}

/// O(1) wallet scheduling DATA. Polling this does not verify a suffix or authorize a debit.
public struct KagemushaWalletSuffixUnloadStatusV1: Sendable {
  public enum State: UInt16, Sendable { case initializing, notApplicable, eligible, verifying, ready, pending, released, requestAbsent }
  public let state: State
  public let reason: UInt16
  public let currentSequence: KagemushaWalletUInt128V1?
  public let foldedSequence: KagemushaWalletUInt128V1?
  public let requestID: Data?
  public let completedSteps: KagemushaWalletUInt128V1
  public let totalSteps: KagemushaWalletUInt128V1
  init(_ original: Data) throws {
    var r = try SuffixFields.frame(original, name: "SuffixUnloadStatusV1", tag: 15, maximum: 512)
    guard let selected = try State(rawValue: r.field { try $0.u16() })
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    state = selected; reason = try r.field { try $0.u16() }
    currentSequence = try r.optional { try $0.u128() }
    foldedSequence = try r.optional { try $0.u128() }
    requestID = try r.optional { try $0.arrayElements(32) }
    completedSteps = try r.scalar(); totalSteps = try r.scalar(); try r.finish()
    guard state == .notApplicable ? (1...3).contains(reason) : reason == 0
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    if let foldedSequence {
      guard let currentSequence, foldedSequence <= currentSequence else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    }
    if let requestID, !requestID.contains(where: { $0 != 0 }) { throw KagemushaWalletErrorV1.invalidNativeOutput }
    if state == .requestAbsent {
      guard requestID != nil, currentSequence != nil, completedSteps == .init(low: 0, high: 0), totalSteps == .init(low: 0, high: 0)
      else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    } else if state.rawValue >= State.verifying.rawValue {
      guard requestID != nil else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      try suffixProgress(completedSteps, totalSteps)
      if state.rawValue >= State.ready.rawValue, completedSteps != totalSteps { throw KagemushaWalletErrorV1.invalidNativeOutput }
    } else if requestID != nil || completedSteps != .init(low: 0, high: 0) || totalSteps != .init(low: 0, high: 0) {
      throw KagemushaWalletErrorV1.invalidNativeOutput
    }
  }
}

private func suffixProgress(_ completed: KagemushaWalletUInt128V1, _ total: KagemushaWalletUInt128V1) throws {
  guard total.high == 0, (1...4_096).contains(total.low), completed <= total
  else { throw KagemushaWalletErrorV1.invalidNativeOutput }
}
private func suffixCounts(_ steps: KagemushaWalletUInt128V1, _ chunks: KagemushaWalletUInt128V1,
  _ instructions: KagemushaWalletUInt128V1) throws {
  guard chunks.high == 0, (1...512).contains(chunks.low), steps.high == 0,
    (chunks.low...(chunks.low * 8)).contains(steps.low), instructions.high == 0,
    instructions.low == chunks.low + 2 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
}
private func suffixAdd(_ a: KagemushaWalletUInt128V1, _ b: KagemushaWalletUInt128V1) -> KagemushaWalletUInt128V1? {
  let (low, carry) = a.low.addingReportingOverflow(b.low)
  let (upper, overflow) = a.high.addingReportingOverflow(b.high)
  let (high, carryOverflow) = upper.addingReportingOverflow(carry ? 1 : 0)
  return overflow || carryOverflow ? nil : .init(low: low, high: high)
}
private func suffixSubtract(_ a: KagemushaWalletUInt128V1, _ b: KagemushaWalletUInt128V1) -> KagemushaWalletUInt128V1? {
  guard a >= b else { return nil }
  let (low, borrow) = a.low.subtractingReportingOverflow(b.low)
  let high = a.high - b.high - (borrow ? 1 : 0)
  return .init(low: low, high: high)
}

/// Strict reader of bounded, declared COMPACT_LEN display fields.
private struct SuffixFields {
  private let data: Data
  private var offset = 0
  init(_ data: Data) { self.data = Data(data) }
  static func frame(_ original: Data, name: String, tag: UInt16, maximum: Int) throws -> Self {
    var r = try canonical(original,
      schema: "iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::" + name, maximum: maximum)
    guard try r.field({ try $0.u16() }) == 1, try r.field({ try $0.u16() }) == tag
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return r
  }
  static func canonical(_ original: Data, schema: String, maximum: Int) throws -> Self {
    guard (48...maximum).contains(original.count), let frame = noritoDecodeFrame(original),
      frame.paddingLength == 8, frame.header.flags == NoritoHeader.compactLen,
      frame.header.schema == noritoSchemaHash(forTypeName: schema)
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return Self(frame.payload)
  }
  mutating func field<T>(_ read: (inout Self) throws -> T) throws -> T {
    let count = try length(compact: true)
    var child = Self(try raw(count)); let value = try read(&child); try child.finish(); return value
  }
  mutating func optional<T>(_ read: (inout Self) throws -> T) throws -> T? {
    try field { child in
      switch try child.byte() {
      case 0: return nil
      case 1: return try child.field(read)
      default: throw KagemushaWalletErrorV1.invalidNativeOutput
      }
    }
  }
  mutating func scalar() throws -> KagemushaWalletUInt128V1 { try field { try $0.u128() } }
  mutating func word() throws -> Data { try field { try $0.fixedArray(32) } }
  mutating func identity() throws -> Data {
    let value = try word(); guard value.contains(where: { $0 != 0 }) else { throw KagemushaWalletErrorV1.invalidNativeOutput }; return value
  }
  mutating func fixedArray(_ count: Int) throws -> Data {
    try raw(count)
  }
  mutating func arrayElements(_ count: Int) throws -> Data {
    var result = Data(); for _ in 0..<count { result.append(try field { try $0.byte() }) }; return result
  }
  mutating func vector(maximum: Int) throws -> Data {
    let count = try length(compact: false)
    guard count <= maximum else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return try raw(count)
  }
  mutating func u128() throws -> KagemushaWalletUInt128V1 { .init(low: try u64(), high: try u64()) }
  mutating func u16() throws -> UInt16 { let bytes = try raw(2); return UInt16(bytes[0]) | UInt16(bytes[1]) << 8 }
  mutating func u32() throws -> UInt32 { let bytes = try raw(4); return (0..<4).reduce(0) { $0 | UInt32(bytes[$1]) << (8 * $1) } }
  mutating func u64() throws -> UInt64 { let bytes = try raw(8); return (0..<8).reduce(0) { $0 | UInt64(bytes[$1]) << (8 * $1) } }
  mutating func byte() throws -> UInt8 { try raw(1)[0] }
  mutating func raw(_ count: Int) throws -> Data {
    guard count >= 0, count <= data.count - offset else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    defer { offset += count }; return data.subdata(in: offset..<(offset + count))
  }
  mutating func remainder(maximum: Int) throws -> Data {
    guard data.count - offset <= maximum else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return try raw(data.count - offset)
  }
  func finish() throws { guard offset == data.count else { throw KagemushaWalletErrorV1.invalidNativeOutput }; }
  private mutating func length(compact: Bool) throws -> Int {
    var value: UInt64 = 0
    if compact {
      var shift = 0
      while true {
        let byte = try byte(), chunk = byte & 0x7f
        guard shift < 64, shift != 63 || chunk <= 1 else { throw KagemushaWalletErrorV1.invalidNativeOutput }
        value |= UInt64(chunk) << shift
        if byte & 0x80 == 0 {
          guard shift == 0 || chunk != 0 else { throw KagemushaWalletErrorV1.invalidNativeOutput }; break
        }
        shift += 7
      }
    } else { value = try u64() }
    guard value <= UInt64(data.count - offset) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return Int(value)
  }
}
