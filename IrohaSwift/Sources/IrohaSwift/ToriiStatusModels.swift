import Foundation

public struct ToriiTimeSnapshot: Decodable, Sendable {
  public let now: UInt64
  public let offset_ms: Int64
  public let confidence_ms: UInt64

  private enum CodingKeys: String, CodingKey {
    case now
    case offset_ms = "offset_ms"
    case confidence_ms = "confidence_ms"
  }
}

public struct ToriiTimeStatusSnapshot: Decodable, Sendable {
  public struct Sample: Decodable, Sendable {
    public let peer: String
    public let last_offset_ms: Int64
    public let last_rtt_ms: UInt64
    public let count: UInt64

    private enum CodingKeys: String, CodingKey {
      case peer
      case last_offset_ms = "last_offset_ms"
      case last_rtt_ms = "last_rtt_ms"
      case count
    }
  }

  public struct RTTBucket: Decodable, Sendable {
    public let le: UInt64
    public let count: UInt64
  }

  public struct RTTSnapshot: Decodable, Sendable {
    public let buckets: [RTTBucket]
    public let sum_ms: UInt64
    public let count: UInt64

    private enum CodingKeys: String, CodingKey {
      case buckets
      case sum_ms = "sum_ms"
      case count
    }
  }

  public let peers: UInt64
  public let samples: [Sample]
  public let rtt: RTTSnapshot?
  public let note: String?
}
