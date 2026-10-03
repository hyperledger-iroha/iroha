import CryptoKit
import Foundation

/// The signed purpose selected by the Native financial owner.
public enum KagemushaOrdinaryCashApprovalPurposeV1: UInt8, Sendable {
  case prepareTransition = 2
  case monetaryTransition = 1
}

/// Failure of a detached original projection; no Native readiness is implied.
public enum KagemushaOrdinaryCashApprovalProjectionErrorV1: Error, Equatable, Sendable {
  case invalidOriginal(String)
}

/// Copied, independently retained public originals. This value authenticates no
/// credential, nonce, clock, platform evidence, lease or current financial owner.
public struct KagemushaOrdinaryCashApprovalOriginalBindingV1: Sendable {
  fileprivate let fields: [Data]
  fileprivate let selection: Data

  public init(operationID: Data, accountBinding: Data, authorityPolicyDigest: Data,
    attestedKeyID: Data, enrollmentDigest: Data, normalizedGuardDigest: Data,
    originalSelection: Data) throws {
    let values = [operationID, accountBinding, authorityPolicyDigest, attestedKeyID,
      enrollmentDigest, normalizedGuardDigest]
    guard values.allSatisfy({ $0.count == 32 && $0.contains(where: { $0 != 0 }) }),
      originalSelection.count == 460 else {
      throw KagemushaOrdinaryCashApprovalProjectionErrorV1.invalidOriginal("retained identity or selection width")
    }
    fields = values.map { Data([UInt8]($0)) }
    selection = Data([UInt8](originalSelection))
  }
}

/// Exact ordinary cash W325/S460 originals, detached from Native owner custody.
/// Preparation and terminal approvals have distinct signed purposes and entry
/// points. SHA256 consumes raw S460 once, including its existing domain/length.
/// No decoder grants signing, issuer, State/Guard, clock, recovery or money authority.
public struct KagemushaOrdinaryCashApprovalProjectionV1: Sendable {
  public let purpose: KagemushaOrdinaryCashApprovalPurposeV1
  private let originalW: Data
  private let originalS: Data

  public var canonicalSigningBytes: Data { originalW }
  public var canonicalFinancialSubject: Data { originalS }
  public var operationID: Data { Data(originalW[53..<85]) }
  public var subjectSigningDigest: Data { Data(SHA256.hash(data: originalS)) }
  public var clientDataHash: Data { Data(SHA256.hash(data: originalW)) }
  public var transitionStatementDigest: Data { Data(originalS[332..<364]) }
  public var operationTag: UInt8 { originalS[331] }
  /// Complete unsigned 128-bit LE original; independent of State sequence/revision.
  public var logicalIndexBeforeLE: Data { Data(originalS[428..<444]) }
  public var logicalIndexAfterLE: Data { Data(originalS[444..<460]) }

  public static func requirePreparation(nativeSigningBytes: Data, nativeFinancialSubject: Data,
    binding: KagemushaOrdinaryCashApprovalOriginalBindingV1) throws -> Self {
    try requireProjection(.prepareTransition, nativeSigningBytes, nativeFinancialSubject, binding)
  }

  public static func requireTerminal(nativeSigningBytes: Data, nativeFinancialSubject: Data,
    binding: KagemushaOrdinaryCashApprovalOriginalBindingV1) throws -> Self {
    try requireProjection(.monetaryTransition, nativeSigningBytes, nativeFinancialSubject, binding)
  }

  /// Distinct incoming purpose2 DATA projection; no Native signing holder is constructed.
  public static func requireIncomingPreparation(nativeSigningBytes: Data, nativeFinancialSubject: Data,
    binding: KagemushaOrdinaryCashApprovalOriginalBindingV1) throws -> Self {
    let value = try requireProjection(.prepareTransition, nativeSigningBytes, nativeFinancialSubject, binding)
    let w = [UInt8](value.originalW)
    guard [UInt8(1), 3].contains(value.operationTag),
      unsigned64(w, at: 317) - unsigned64(w, at: 309) <= 10_000 else {
      throw invalid("incoming preparation operation or interval")
    }
    return value
  }

  /// Distinct incoming purpose1 DATA projection with both complete candidate/body selectors.
  public static func requireIncomingTerminal(nativeSigningBytes: Data, nativeFinancialSubject: Data,
    binding: KagemushaOrdinaryCashApprovalOriginalBindingV1) throws -> Self {
    try requireProjection(.monetaryTransition, nativeSigningBytes, nativeFinancialSubject, binding,
      ordinaryIncomingTerminal: true)
  }

  // Actual Rust model fixtures precede ordinary credential authentication. Their
  // independent enrollment/S markers may differ. Production callers require binding.
  static func requireModelMessageShape(_ purpose: KagemushaOrdinaryCashApprovalPurposeV1,
    nativeSigningBytes: Data, nativeFinancialSubject: Data) throws -> Self {
    let operation = nativeFinancialSubject.dropFirst(331).first
    return try requireProjection(purpose, nativeSigningBytes, nativeFinancialSubject, nil,
      ordinaryIncomingTerminal: purpose == .monetaryTransition && (operation == 1 || operation == 3))
  }

  private static func requireProjection(_ purpose: KagemushaOrdinaryCashApprovalPurposeV1,
    _ wOriginal: Data, _ sOriginal: Data, _ binding: KagemushaOrdinaryCashApprovalOriginalBindingV1?,
    ordinaryIncomingTerminal: Bool = false) throws -> Self {
    let w = [UInt8](wOriginal), s = [UInt8](sOriginal)
    guard w.count == 325, s.count == 460 else { throw invalid("message width") }
    try requireFrame(w, domain: "iroha:kagemusha:v1:app-operation-approval\0", bodyLength: 275)
    try requireFrame(s, domain: "iroha:kagemusha:v1:hardware-transition-selection\0", bodyLength: 403)
    guard w[52] == purpose.rawValue, (1...5).contains(s[331]) else { throw invalid("cash purpose or operation") }
    guard [59, 91, 123, 155, 187, 219, 251, 291, 332].allSatisfy({ present(s, at: $0) }),
      unsigned64(s, at: 283) > 0, unsigned64(s, at: 323) > 0 else { throw invalid("selection identity or epoch") }
    var carry: UInt16 = 1
    for index in 0..<16 {
      let sum = UInt16(s[428 + index]) + carry
      guard s[444 + index] == UInt8(truncatingIfNeeded: sum) else { throw invalid("exact-next index") }
      carry = sum >> 8
    }
    guard carry == 0 else { throw invalid("index overflow") }
    let candidate = present(s, at: 364), terminal = present(s, at: 396)
    switch purpose {
    case .prepareTransition:
      guard !candidate, !terminal else { throw invalid("preparation terminal commitment") }
    case .monetaryTransition:
      guard !ordinaryIncomingTerminal || s[331] == 1 || s[331] == 3 else {
        throw invalid("incoming terminal operation")
      }
      let commitmentsRequired = ordinaryIncomingTerminal || s[331] == 2 || s[331] == 4
      guard candidate == commitmentsRequired, terminal == commitmentsRequired else { throw invalid("terminal operation commitment") }
    }
    guard (0..<8).allSatisfy({ present(w, at: 53 + $0 * 32) }) else { throw invalid("approval identity") }
    let issued = unsigned64(w, at: 309), expires = unsigned64(w, at: 317)
    guard issued > 0, expires > issued, expires - issued <= 120_000 else { throw invalid("original interval") }
    let subject = Data(s)
    guard Data(w[245..<277]) == Data(SHA256.hash(data: subject)) else { throw invalid("raw S digest") }
    if let binding {
      guard Data(s[155..<187]) == Data(w[213..<245]), subject == binding.selection else {
        throw invalid("retained credential or full selection")
      }
      for (offset, original) in zip([53, 117, 149, 181, 213, 277], binding.fields) {
        guard Data(w[offset..<(offset + 32)]) == original else { throw invalid("retained public identity") }
      }
    }
    return Self(purpose: purpose, originalW: Data(w), originalS: subject)
  }

  private static func requireFrame(_ bytes: [UInt8], domain: String, bodyLength: UInt64) throws {
    let prefix = [UInt8](domain.utf8), version = prefix.count + 8
    guard bytes.starts(with: prefix), unsigned64(bytes, at: prefix.count) == bodyLength,
      bytes[version] == 1, bytes[version + 1] == 0 else { throw invalid("domain, length or version") }
  }

  private static func present(_ bytes: [UInt8], at offset: Int) -> Bool {
    bytes[offset..<(offset + 32)].contains(where: { $0 != 0 })
  }
  private static func unsigned64(_ bytes: [UInt8], at offset: Int) -> UInt64 {
    bytes[offset..<(offset + 8)].enumerated().reduce(UInt64(0)) {
      $0 | UInt64($1.element) << UInt64($1.offset * 8)
    }
  }
  private static func invalid(_ reason: String) -> KagemushaOrdinaryCashApprovalProjectionErrorV1 {
    .invalidOriginal(reason)
  }
}
