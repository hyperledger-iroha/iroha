import Foundation

/// Exact participant route, authority and bounded FIFO source membership.
public struct ToriiNativeAmxParticipantSettlement: Decodable, Sendable, Equatable {
    public let laneId: UInt32
    public let dataspaceId: UInt64
    public let laneIncarnation: String
    public let participantLaneBlockHeight: UInt64
    public let authorityContextHeight: UInt64
    public let previousNativeSettlementHash: String?
    public let sourceIds: [ToriiNativeAmxSourceId]

    private enum CodingKeys: String, CodingKey {
        case laneId = "lane_id"
        case dataspaceId = "dataspace_id"
        case laneIncarnation = "lane_incarnation"
        case participantLaneBlockHeight = "participant_lane_block_height"
        case authorityContextHeight = "authority_context_height"
        case previousNativeSettlementHash = "previous_native_settlement_hash"
        case sourceIds = "source_ids"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: ["lane_id", "dataspace_id", "lane_incarnation", "participant_lane_block_height",
                      "authority_context_height", "previous_native_settlement_hash", "source_ids"],
            context: "native AMX participant settlement"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        laneId = try container.decode(UInt32.self, forKey: .laneId)
        dataspaceId = try container.decode(UInt64.self, forKey: .dataspaceId)
        laneIncarnation = try ToriiNativeAmxWire.canonicalHash(
            container.decode(String.self, forKey: .laneIncarnation),
            key: .laneIncarnation, container: container, field: "participant lane_incarnation"
        )
        participantLaneBlockHeight = try container.decode(UInt64.self, forKey: .participantLaneBlockHeight)
        authorityContextHeight = try container.decode(UInt64.self, forKey: .authorityContextHeight)
        guard container.contains(.previousNativeSettlementHash) else {
            throw DecodingError.keyNotFound(
                CodingKeys.previousNativeSettlementHash,
                DecodingError.Context(codingPath: container.codingPath,
                    debugDescription: "previous_native_settlement_hash is required, including when null.")
            )
        }
        if try container.decodeNil(forKey: .previousNativeSettlementHash) {
            previousNativeSettlementHash = nil
        } else {
            previousNativeSettlementHash = try ToriiNativeAmxWire.canonicalHash(
                container.decode(String.self, forKey: .previousNativeSettlementHash),
                key: .previousNativeSettlementHash, container: container,
                field: "previous_native_settlement_hash"
            )
        }
        var sourceContainer = try container.nestedUnkeyedContainer(forKey: .sourceIds)
        var sources: [ToriiNativeAmxSourceId] = []
        while !sourceContainer.isAtEnd {
            guard sources.count < 4096 else {
                throw DecodingError.dataCorruptedError(
                    forKey: .sourceIds, in: container,
                    debugDescription: "participant settlement source count exceeds 4096."
                )
            }
            sources.append(try sourceContainer.decode(ToriiNativeAmxSourceId.self))
        }
        sourceIds = sources
        let incarnationHex = laneIncarnation.dropFirst(5).prefix(64)
        let previousIsNonzero = previousNativeSettlementHash.map {
            $0.dropFirst(5).prefix(64) != String(repeating: "0", count: 63) + "1"
        } ?? true
        guard participantLaneBlockHeight > 0, authorityContextHeight > 0,
              participantLaneBlockHeight != 1 || previousNativeSettlementHash == nil,
              previousIsNonzero,
              incarnationHex != String(repeating: "0", count: 63) + "1",
              (1...4096).contains(sourceIds.count), Set(sourceIds).count == sourceIds.count
        else {
            throw DecodingError.dataCorruptedError(
                forKey: .sourceIds, in: container,
                debugDescription: "participant settlement requires nonzero authority and 1...4096 unique sources."
            )
        }
    }
}
