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

/// Deterministic Sumeragi membership hash snapshot mirrored from `/v1/sumeragi/status`.
public struct ToriiSumeragiMembershipSnapshot: Decodable, Sendable {
  /// Block height covered by the membership digest.
  public let height: UInt64
  /// Consensus view associated with the digest.
  public let view: UInt64
  /// Epoch identifier paired with the digest.
  public let epoch: UInt64
  /// Optional canonical digest for the membership (hex encoded).
  public let viewHash: String?

  private enum CodingKeys: String, CodingKey {
    case height
    case view
    case epoch
    case viewHash = "view_hash"
  }
}

public struct ToriiLaneCommitmentSnapshot: Decodable, Sendable, Equatable {
  public let blockHeight: UInt64
  public let laneId: UInt64
  public let txCount: UInt64
  public let totalChunks: UInt64
  public let rbcBytesTotal: UInt64
  public let teuTotal: UInt64
  public let blockHash: String

  private enum CodingKeys: String, CodingKey {
    case blockHeight = "block_height"
    case laneId = "lane_id"
    case txCount = "tx_count"
    case totalChunks = "total_chunks"
    case rbcBytesTotal = "rbc_bytes_total"
    case teuTotal = "teu_total"
    case blockHash = "block_hash"
  }
}

public struct ToriiDataspaceCommitmentSnapshot: Decodable, Sendable, Equatable {
  public let blockHeight: UInt64
  public let laneId: UInt64
  public let dataspaceId: UInt64
  public let txCount: UInt64
  public let totalChunks: UInt64
  public let rbcBytesTotal: UInt64
  public let teuTotal: UInt64
  public let blockHash: String

  private enum CodingKeys: String, CodingKey {
    case blockHeight = "block_height"
    case laneId = "lane_id"
    case dataspaceId = "dataspace_id"
    case txCount = "tx_count"
    case totalChunks = "total_chunks"
    case rbcBytesTotal = "rbc_bytes_total"
    case teuTotal = "teu_total"
    case blockHash = "block_hash"
  }
}
