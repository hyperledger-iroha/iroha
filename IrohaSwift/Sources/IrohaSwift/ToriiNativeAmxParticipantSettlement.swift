import Foundation

/// Canonical Native AMX participant settlement, without recursive coordinator receipts.
public struct ToriiNativeAmxParticipantSettlement: Decodable, Sendable, Equatable {
    public let blockHeight: UInt64
    public let laneId: UInt32
    public let laneIncarnation: String
    public let dataspaceId: UInt64
    public let transactionCount: UInt64
    public let totalLocalAmount: String
    public let totalXorDue: String
    public let totalXorAfterHaircut: String
    public let totalXorVariance: String
    public let swapMetadata: ToriiLaneSwapMetadata?
    public let receipts: [ToriiLaneSettlementReceipt]
    public let nexusFeeReceipts: [ToriiNexusFeeReceipt]

    private enum CodingKeys: String, CodingKey {
        case blockHeight = "block_height"
        case laneId = "lane_id"
        case laneIncarnation = "lane_incarnation"
        case dataspaceId = "dataspace_id"
        case transactionCount = "tx_count"
        case totalLocalAmount = "total_local_amount"
        case totalXorDue = "total_xor_due"
        case totalXorAfterHaircut = "total_xor_after_haircut"
        case totalXorVariance = "total_xor_variance"
        case swapMetadata = "swap_metadata"
        case receipts
        case nexusFeeReceipts = "nexus_fee_receipts"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: [
                "block_height", "lane_id", "lane_incarnation", "dataspace_id", "tx_count",
                "total_local_amount", "total_xor_due", "total_xor_after_haircut",
                "total_xor_variance", "swap_metadata", "receipts", "nexus_fee_receipts",
            ],
            context: "native AMX participant settlement"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        blockHeight = try container.decode(UInt64.self, forKey: .blockHeight)
        laneId = try container.decode(UInt32.self, forKey: .laneId)
        laneIncarnation = try ToriiNativeAmxWire.canonicalHash(
            container.decode(String.self, forKey: .laneIncarnation),
            key: .laneIncarnation,
            container: container,
            field: "lane_incarnation"
        )
        dataspaceId = try container.decode(UInt64.self, forKey: .dataspaceId)
        transactionCount = try container.decode(UInt64.self, forKey: .transactionCount)
        totalLocalAmount = try decodeCanonicalToriiQuantity(
            container.decode(String.self, forKey: .totalLocalAmount),
            field: "total_local_amount"
        )
        totalXorDue = try decodeCanonicalToriiQuantity(
            container.decode(String.self, forKey: .totalXorDue),
            field: "total_xor_due"
        )
        totalXorAfterHaircut = try decodeCanonicalToriiQuantity(
            container.decode(String.self, forKey: .totalXorAfterHaircut),
            field: "total_xor_after_haircut"
        )
        totalXorVariance = try decodeCanonicalToriiQuantity(
            container.decode(String.self, forKey: .totalXorVariance),
            field: "total_xor_variance"
        )
        guard container.contains(.swapMetadata) else {
            throw DecodingError.keyNotFound(
                CodingKeys.swapMetadata,
                DecodingError.Context(
                    codingPath: container.codingPath,
                    debugDescription: "native AMX participant settlement swap_metadata must be present, including when null."
                )
            )
        }
        swapMetadata = try container.decodeIfPresent(ToriiLaneSwapMetadata.self, forKey: .swapMetadata)
        receipts = try container.decode([ToriiLaneSettlementReceipt].self, forKey: .receipts)
        nexusFeeReceipts = try container.decode([ToriiNexusFeeReceipt].self, forKey: .nexusFeeReceipts)
        let feeSources = Set(nexusFeeReceipts.map { $0.sourceId.lowercased() })
        guard feeSources.count == nexusFeeReceipts.count,
              nexusFeeReceipts.allSatisfy({
                  $0.laneId == laneId && $0.dataspaceId == dataspaceId && $0.blockHeight == blockHeight
              })
        else {
            throw DecodingError.dataCorruptedError(
                forKey: .nexusFeeReceipts,
                in: container,
                debugDescription: "native AMX participant settlement fees have duplicate sources or mismatched coordinates."
            )
        }
    }
}
