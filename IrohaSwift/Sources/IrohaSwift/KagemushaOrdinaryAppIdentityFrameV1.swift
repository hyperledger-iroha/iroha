import CryptoKit
import Foundation

/// Closed method21 grammar. Data validation cannot mint native authority.
enum KagemushaOrdinaryAppIdentityFrameV1 {
  static func validateRequest(_ f: [Data]) throws {
    guard !f.isEmpty else { throw invalid() }
    let phase=try number(f[0])
    if phase == 11 || phase == 12 || phase == 15 { guard f.count == 1 else { throw invalid() }; return }
    if phase == 1 { throw invalid() }
    guard f.count >= 2, f[1].count == 8, f[1].contains(where:{$0 != 0}) else { throw invalid() }
    switch phase {
    case 2,4,7,8,9,14: guard f.count == 2 else { throw invalid() }
    case 6: guard f.count == 3, f[2].count == 314 else { throw invalid() }
    case 13:
      guard f.count == 3 else { throw invalid() }
      _ = try KagemushaOrdinaryAppIdentityPreparedProjectionV1.challenge(transport: f[2])
    case 3: guard f.count == 3 else { throw invalid() }; try reference(f[2])
    case 5:
      guard f.count == 5, point(f[2]), (1...65_536).contains(f[3].count),
        f[4].count <= 65_536, f[4].isEmpty || f[3].count == 65_536 else { throw invalid() }
    case 10: guard f.count == 3, try number(f[2]) <= 1 else { throw invalid() }
    default: throw invalid()
    }
  }
  static func validateResponse(_ q: [Data], _ r: [Data]) throws {
    try validateRequest(q)
    switch try number(q[0]) {
    case 11: guard r.count == 1, digest(r[0]) else { throw invalid() }
    case 12: _ = try KagemushaOrdinaryAppIdentityReservationProjectionV1(r)
    case 15:
      guard r.count == 3, r[0].count == 8, r[0].contains(where: { $0 != 0 }),
        r[1] != r[2] else { throw invalid() }
      try accountText(r[1]); try accountText(r[2])
    case 13:
      let prepared = try KagemushaOrdinaryAppIdentityPreparedProjectionV1(r)
      guard prepared.signedChallenge == q[2] else { throw invalid() }
    case 14: guard r.count == 1, r[0].count <= 16_384 else { throw invalid() }
    case 2:
      guard r.count == 2, r[0].count == 1 else { throw invalid() }
      if r[0] == Data([1]) { guard r[1].isEmpty else { throw invalid() } }
      else if r[0] == Data([2]) { try reference(r[1]) } else { throw invalid() }
    case 3: guard r.count == 1, r[0] == Data(SHA256.hash(data:q[2])) else { throw invalid() }
    case 4:
      guard r.count == 4, r[0].count == 1 else { throw invalid() }
      if r[0] == Data([1]) { try rawMetadata(r[1],r[2],r[3],present:false) }
      else if r[0] == Data([2]) { try rawMetadata(r[1],r[2],r[3],present:true) } else { throw invalid() }
    case 5:
      guard r.count == 2, r[0] == Data(SHA256.hash(data:q[3]+q[4])),
        r[1] == Data(SHA256.hash(data:q[2])) else { throw invalid() }
    case 6,8: guard r.count == 2, digest(r[0]),digest(r[1]) else { throw invalid() }
    case 7:
      guard r.count == 7, r[0].count == 1, r[0][0] <= 5 else { throw invalid() }
      let state=r[0][0]
      if state < 2 { guard r[1].isEmpty else { throw invalid() } } else { try reference(r[1]) }
      try rawMetadata(r[2],r[3],r[4],present:state>=4)
      if state == 5 { guard r[5].count == 314,digest(r[6]) else { throw invalid() } }
      else { guard r[5].isEmpty,r[6].isEmpty else { throw invalid() } }
    case 9: guard r.isEmpty else { throw invalid() }
    case 10:
      guard r.count == 4,r[0] == q[2],digest(r[2]) else { throw invalid() }
      let total=Int(try number(r[3])), index=Int(try number(r[0]))
      guard (1...131_072).contains(total),index*65_536 < total,
        r[1].count == min(65_536,total-index*65_536) else { throw invalid() }
    default: throw invalid()
    }
  }
  private static func rawMetadata(_ p:Data,_ h:Data,_ n:Data,present:Bool) throws {
    let length=try number(n)
    guard present ? (point(p) && digest(h) && length > 0 && length <= 131_072)
      : (p.isEmpty && h.isEmpty && length == 0) else { throw invalid() }
  }
  private static func accountText(_ data: Data) throws {
    guard (1...2_048).contains(data.count), !data.contains(0),
      String(data: data, encoding: .utf8) != nil else { throw invalid() }
  }
  private static func reference(_ data:Data) throws {
    guard (1...255).contains(data.count),!data.contains(0),String(data:data,encoding:.utf8) != nil else { throw invalid() }
  }
  private static func number(_ data:Data) throws -> UInt32 {
    guard data.count == 4 else { throw invalid() };return KagemushaAppPlatformPreparedProjectionV1.u32(data)
  }
  private static func digest(_ d:Data)->Bool { KagemushaOrdinaryAppIdentityPreparedProjectionV1.digest(d) }
  private static func point(_ d:Data)->Bool { KagemushaOrdinaryAppIdentityPreparedProjectionV1.point(d) }
  private static func invalid()->KagemushaCoreCoordinatorErrorV1 { .invalidFrame("ordinary identity phase grammar differs") }
}
