import CryptoKit
import Foundation

/// Exact existing Native TransitionProofStatementV1 digest preimage, joined to S.
/// It is BE64(40), the NUL-terminated domain40, BE64(1089), then body1089 with
/// LE body integers. This accepts the complete original; there is no new encoder,
/// 93-cell reconstruction, State/Guard verifier, issuer or current-owner surrogate.
public struct KagemushaOrdinaryTransitionStatementProjectionV1: Sendable {
  private let original: Data
  public var canonicalPreimage: Data { original }
  public var digest: Data { Data(SHA256.hash(data: original)) }
  public var operationTag: UInt8 { original[56 + 132] }

  public static func requireOriginal(canonicalModelPreimage: Data,
    cashApproval: KagemushaOrdinaryCashApprovalProjectionV1) throws -> Self {
    let bytes = [UInt8](canonicalModelPreimage)
    guard bytes.count == 1145 else { throw invalid("transition original width") }
    let domain = [UInt8]("iroha:kagemusha:v1:transition-statement\0".utf8)
    guard unsigned64BE(bytes, at: 0) == 40, Array(bytes[8..<48]) == domain,
      unsigned64BE(bytes, at: 48) == 1089 else { throw invalid("transition domain or length") }
    guard Array(bytes[56..<60]) == [1, 0, 1, 0] else { throw invalid("first-release version") }
    guard (1...5).contains(bytes[188]), bytes[188] == cashApproval.operationTag else { throw invalid("cash operation") }
    let original = Data(bytes)
    guard Data(SHA256.hash(data: original)) == cashApproval.transitionStatementDigest else { throw invalid("complete transition digest") }
    let selection = cashApproval.canonicalFinancialSubject
    guard Data(bytes[(56 + 373)..<(56 + 405)]) == Data(bytes[(56 + 405)..<(56 + 437)]) else {
      throw invalid("first-release predecessor/successor release")
    }
    // Exact offsets in commitments.rs. Full SHA binds every other body byte.
    for (bodyOffset, selectionOffset) in [(405, 59), (501, 251), (541, 187), (573, 219)] {
      guard Data(bytes[(56 + bodyOffset)..<(56 + bodyOffset + 32)]) ==
        Data(selection[selectionOffset..<(selectionOffset + 32)]) else { throw invalid("release/profile/lane original") }
    }
    guard Data(bytes[(56 + 533)..<(56 + 541)]) == Data(selection[283..<291]) else { throw invalid("policy epoch original") }
    return Self(original: original)
  }

  private static func unsigned64BE(_ bytes: [UInt8], at offset: Int) -> UInt64 {
    bytes[offset..<(offset + 8)].reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
  }
  private static func invalid(_ reason: String) -> KagemushaOrdinaryCashApprovalProjectionErrorV1 {
    .invalidOriginal(reason)
  }
}
