import CryptoKit
import Foundation

/// Closed additive method19/20 grammar. Shape validation grants no native owner.
enum KagemushaAppPlatformFrameV1 {
  static func validateRequest(_ method: KagemushaCoreCoordinatorMethodV1, _ f: [Data]) throws {
    guard method == .appOperationApproval || method == .appEnrollmentPossession,
      let phase = phase(f) else { throw invalid("invalid app platform method or phase") }
    switch phase {
    case 1:
      guard f.count == 2, KagemushaAppPlatformPreparedProjectionV1.digest(f[1]) else {
        throw invalid("invalid original app operation identity")
      }
    case 2, 4, 5, 6, 7:
      guard f.count == 2, ticket(f[1]) else { throw invalid("invalid app platform ticket") }
    case 3:
      guard f.count == 3, ticket(f[1]), (1...4096).contains(f[2].count) else {
        throw invalid("invalid original platform evidence")
      }
    default: throw invalid("unknown app platform phase")
    }
  }

  static func validateResponse(_ method: KagemushaCoreCoordinatorMethodV1,
    _ originalRequest: [Data], _ originalResponse: [Data]) throws {
    let request = originalRequest.map { Data($0) }, f = originalResponse.map { Data($0) }
    try validateRequest(method, request)
    let purpose: UInt8 = method == .appOperationApproval ? 1 : 2
    switch phase(request)! {
    case 1:
      _ = try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
        approvalID: purpose == 1 ? request[1] : nil,
        enrollmentID: purpose == 2 ? request[1] : nil)
    case 2, 5:
      guard f.count == 3, f[0].count == 1 else { throw invalid("invalid original recovery state") }
      let state = f[0][0], fence = phase(request) == 2
      let uninvoked: UInt8 = fence ? 1 : 0
      let retained: UInt8 = fence ? 2 : 1
      let consumed: UInt8 = fence ? 3 : 2
      if state == uninvoked {
        guard f[1].isEmpty, f[2].isEmpty else { throw invalid("uninvoked state carries result") }
      } else if state == retained {
        guard (1...4096).contains(f[1].count), f[2].isEmpty else {
          throw invalid("retained state lacks exact original")
        }
      } else if state == consumed {
        guard (1...4096).contains(f[1].count) else { throw invalid("consumed state lacks original") }
        let receipt = try KagemushaAppPlatformReceiptProjectionV1(f[2])
        guard receipt.purpose == purpose, receipt.ticket == request[1],
          receipt.rawEvidenceDigest == Data(SHA256.hash(data: f[1])) else {
          throw invalid("recovery receipt substitutes original evidence")
        }
      } else { throw invalid("unknown original recovery state") }
    case 3:
      guard f.count == 1, f[0] == Data(SHA256.hash(data: request[2])) else {
        throw invalid("native retention digest differs from original")
      }
    case 4:
      guard f.count == 1 else { throw invalid("invalid native app receipt fields") }
      let receipt = try KagemushaAppPlatformReceiptProjectionV1(f[0])
      guard receipt.purpose == purpose, receipt.ticket == request[1] else {
        throw invalid("native app receipt substitutes purpose or ticket")
      }
    case 6:
      guard f.count == 2, f.allSatisfy(KagemushaAppPlatformPreparedProjectionV1.digest) else {
        throw invalid("invalid current native scope response")
      }
    case 7:
      guard f.isEmpty else { throw invalid("cancel response carries authority") }
    default: throw invalid("unknown app platform phase")
    }
  }

  private static func phase(_ f: [Data]) -> UInt32? {
    guard let first = f.first, first.count == 4 else { return nil }
    return KagemushaAppPlatformPreparedProjectionV1.u32(first)
  }
  private static func ticket(_ f: Data) -> Bool {
    f.count == 8 && KagemushaAppPlatformPreparedProjectionV1.nonzero(f)
  }
  private static func invalid(_ message: String) -> KagemushaCoreCoordinatorErrorV1 { .invalidFrame(message) }
}

/// Fixed184 transport projection; parsing alone does not authenticate a receipt.
struct KagemushaAppPlatformReceiptProjectionV1: Sendable {
  let canonicalBytes: Data
  let purpose: UInt8
  let ticket, originalID, nativeScope, challengeDigest, rawEvidenceDigest, originalScopeDigest: Data
  let appleCounter: UInt32?
  init(_ input: Data) throws {
    let bytes = Data(input)
    guard bytes.count == 184, bytes.prefix(8) == Data("KGMAPP1\0".utf8),
      bytes[8] == 1, bytes[9] == 0, [UInt8(1), 2].contains(bytes[10]),
      KagemushaAppPlatformPreparedProjectionV1.nonzero(Data(bytes[11..<19])) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid fixed native app receipt")
    }
    let digests = (0..<5).map { Data(bytes[(19 + $0 * 32)..<(51 + $0 * 32)]) }
    guard digests.allSatisfy(KagemushaAppPlatformPreparedProjectionV1.digest),
      [UInt8(0), 1].contains(bytes[179]),
      bytes[179] == 1 || bytes[180..<184].allSatisfy({ $0 == 0 }) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid native app receipt scope or counter")
    }
    let counter = KagemushaAppPlatformPreparedProjectionV1.u32(Data(bytes[180..<184]))
    guard bytes[179] == 0 || counter > 0 else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("invalid native Apple receipt counter")
    }
    canonicalBytes = bytes; purpose = bytes[10]; ticket = Data(bytes[11..<19])
    originalID = digests[0]; nativeScope = digests[1]; challengeDigest = digests[2]
    rawEvidenceDigest = digests[3]; originalScopeDigest = digests[4]
    appleCounter = bytes[179] == 1 ? counter : nil
  }
}
