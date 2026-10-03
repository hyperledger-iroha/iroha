import CryptoKit
import Foundation

/// Closed method19/20 grammar, including the native FI ceremony. Shape grants no owner.
enum KagemushaAppPlatformFrameV1 {
  static func validateRequest(_ method: KagemushaCoreCoordinatorMethodV1, _ f: [Data]) throws {
    guard method == .appOperationApproval || method == .appEnrollmentPossession,
      let phase = phase(f) else { throw invalid("invalid app platform method or phase") }
    switch phase {
    case 1:
      guard f.count == 2, KagemushaAppPlatformPreparedProjectionV1.digest(f[1]) else {
        throw invalid("invalid original app operation identity")
      }
    case 15:
      if method == .appEnrollmentPossession {
        guard f.count == 3, ticket(f[1]), f[2].count == 4,
          KagemushaAppPlatformPreparedProjectionV1.u32(f[2]) < 4 else {
          throw invalid("invalid complete financial Start chunk request")
        }
      } else {
        guard f.count == 3, f[1].count == 4 else {
          throw invalid("invalid ordinary business preparation")
        }
        switch KagemushaAppPlatformPreparedProjectionV1.u32(f[1]) {
        case 2: guard (1...4096).contains(f[2].count) else { throw invalid("invalid receiver request original") }
        case 4: guard f[2].count == 16, KagemushaAppPlatformPreparedProjectionV1.nonzero(f[2]) else { throw invalid("invalid positive u128 amount") }
        default: throw invalid("unknown ordinary business operation")
        }
      }
    case 2, 4, 5, 6, 7:
      guard f.count == 2, ticket(f[1]) else { throw invalid("invalid app platform ticket") }
    case 3:
      guard f.count == 3, ticket(f[1]), (1...4096).contains(f[2].count) else {
        throw invalid("invalid original platform evidence")
      }
    case 8:
      if method == .appOperationApproval {
        guard f.count == 2, KagemushaAppPlatformPreparedProjectionV1.digest(f[1]) else {
          throw invalid("invalid original bootstrap operation identity")
        }
      } else {
        guard f.count == 3, ticket(f[1]), (1...16384).contains(f[2].count) else {
          throw invalid("invalid final app identity original")
        }
      }
    case 9:
      guard method == .appEnrollmentPossession, f.count == 4, ticket(f[1]),
        (1...32768).contains(f[2].count), KagemushaAppPlatformPreparedProjectionV1.digest(f[3]) else {
        throw invalid("invalid original retail enrollment challenge")
      }
    case 10, 13, 14:
      guard method == .appEnrollmentPossession, f.count == 2, ticket(f[1]) else {
        throw invalid("invalid retail enrollment ticket")
      }
    case 11:
      guard method == .appEnrollmentPossession, f.count == 3, ticket(f[1]), f[2].count == 64 else {
        throw invalid("invalid original wallet signature")
      }
    case 12:
      guard method == .appEnrollmentPossession, f.count == 3, ticket(f[1]),
        (1...16384).contains(f[2].count) else {
        throw invalid("invalid original retail enrollment certificate")
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
      if purpose == 1 {
        try KagemushaAppPlatformPreparedProjectionV1.validateApprovalTransport(f, operationID: request[1])
      } else {
        _ = try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
          approvalID: nil, enrollmentChallengeHash: request[1])
      }
    case 15:
      if method == .appEnrollmentPossession {
        guard f.count == 6, f[0] == request[2], f[3].count == 4,
          [f[2], f[4], f[5]].allSatisfy(KagemushaAppPlatformPreparedProjectionV1.digest) else {
          throw invalid("invalid complete financial Start chunk metadata")
        }
        let total = Int(KagemushaAppPlatformPreparedProjectionV1.u32(f[3]))
        let offset = Int(KagemushaAppPlatformPreparedProjectionV1.u32(request[2])) * 65536
        guard (1...262144).contains(total), offset < total,
          f[1].count == min(65536, total - offset) else {
          throw invalid("invalid complete financial Start chunk length")
        }
      } else {
        guard f.count == 1, KagemushaAppPlatformPreparedProjectionV1.digest(f[0]) else {
          throw invalid("invalid Native reserved ordinary operation identity")
        }
      }
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
    case 8:
      if method == .appOperationApproval {
        try KagemushaAppPlatformPreparedProjectionV1.validateBootstrapTransport(f, operationID: request[1])
      } else {
        guard f.count == 2, f.allSatisfy(KagemushaAppPlatformPreparedProjectionV1.digest) else {
          throw invalid("invalid retained final app identity response")
        }
      }
    case 9:
      guard f.count == 5, ticket(f[0]), f[1] == request[2], f[2] == request[3],
        KagemushaAppPlatformPreparedProjectionV1.digest(f[3]),
        KagemushaAppPlatformPreparedProjectionV1.digest(f[4]) else {
        throw invalid("retail preparation substitutes original challenge or message")
      }
    case 10:
      guard f.count == 2,
        (f[0] == Data([1]) && f[1].isEmpty) || (f[0] == Data([2]) && f[1].count == 64) else {
        throw invalid("invalid wallet invocation fence")
      }
    case 11:
      guard f.count == 1, f[0] == Data(SHA256.hash(data: request[2])) else {
        throw invalid("native wallet retention substitutes original signature")
      }
    case 12:
      guard f.count == 2, f.allSatisfy(KagemushaAppPlatformPreparedProjectionV1.digest) else {
        throw invalid("invalid retained retail enrollment response")
      }
    case 13:
      guard f.count == 3, f[0].count == 1 else { throw invalid("invalid retail recovery fields") }
      switch f[0][0] {
      case 0, 1:
        guard f[1].isEmpty, f[2].isEmpty else { throw invalid("unretained action carries originals") }
      case 2:
        guard f[1].count == 64, f[2].isEmpty else { throw invalid("invalid retained wallet signature") }
      case 3:
        guard f[1].count == 64, (1...16384).contains(f[2].count) else {
          throw invalid("invalid retained retail certificate")
        }
      default: throw invalid("unknown retail recovery state")
      }
    case 14:
      guard f.isEmpty else { throw invalid("retail cancellation carries result") }
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

/// Bounded public digest projection of native phase8. Parsing these fields alone
/// creates no native holder, possession receipt or financial capability.
struct KagemushaAppEnrollmentFinalIdentityProjectionV1: Sendable {
  let credentialDigest: Data
  let pendingScope: Data

  init(nativeFields: [Data], originalPendingScope: Data) throws {
    let fields = nativeFields.map { Data($0) }
    guard KagemushaAppPlatformPreparedProjectionV1.digest(originalPendingScope),
      fields.count == 2,
      fields.allSatisfy(KagemushaAppPlatformPreparedProjectionV1.digest),
      fields[1] == originalPendingScope else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("final app identity substituted pending scope")
    }
    credentialDigest = fields[0]; pendingScope = fields[1]
  }
}
