import Foundation

/// Network-time estimate together with the server's sample and health evidence.
/// A fallback clock cannot establish transaction expiry or permit a retry.
public struct ToriiTimeSnapshot: Decodable, Sendable {
  public enum EnforcementMode: String, Decodable, Sendable {
    case warn
    case reject
  }

  public struct Health: Decodable, Sendable {
    public let healthy: Bool
    public let min_samples_ok: Bool
    public let offset_ok: Bool
    public let confidence_ok: Bool
  }

  public let now: UInt64
  public let offset_ms: Int64
  public let confidence_ms: UInt64
  public let sample_count: UInt64
  public let peer_count: UInt64
  public let enforcement_mode: EnforcementMode
  public let fallback: Bool
  public let health: Health

  /// Conservative network time, present only when the complete snapshot is healthy.
  public var healthyLowerBoundMs: UInt64? {
    guard now > 0, confidence_ms <= now, sample_count > 0, peer_count > 0,
          !fallback, health.healthy, health.min_samples_ok,
          health.offset_ok, health.confidence_ok else { return nil }
    return now - confidence_ms
  }

  private enum CodingKeys: String, CodingKey {
    case now
    case offset_ms = "offset_ms"
    case confidence_ms = "confidence_ms"
    case sample_count = "sample_count"
    case peer_count = "peer_count"
    case enforcement_mode = "enforcement_mode"
    case fallback
    case health
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
