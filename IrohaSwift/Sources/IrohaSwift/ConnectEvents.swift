import Foundation

public struct ConnectBalanceAsset: Codable, Equatable, Sendable {
    public var assetId: String
    public var assetDefinitionId: String?
    public var quantity: String
    public var precision: Int?

    public init(assetId: String,
                assetDefinitionId: String? = nil,
                quantity: String,
                precision: Int? = nil) {
        self.assetId = assetId
        self.assetDefinitionId = assetDefinitionId
        self.quantity = quantity
        self.precision = precision
    }

    private enum CodingKeys: String, CodingKey {
        case assetId = "asset_id"
        case assetDefinitionId = "asset_definition_id"
        case quantity
        case precision
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        assetId = try container.decode(String.self, forKey: .assetId)
        assetDefinitionId = try container.decodeIfPresent(String.self, forKey: .assetDefinitionId)
        let rawQuantity = try container.decode(String.self, forKey: .quantity)
        do {
            quantity = try KotodamaNumericV1Codec.decodeQuantityJSON(rawQuantity).canonicalString
        } catch {
            throw DecodingError.dataCorruptedError(
                forKey: .quantity,
                in: container,
                debugDescription: "quantity must be a canonical non-negative Kotodama V1 quantity"
            )
        }
        precision = try container.decodeIfPresent(Int.self, forKey: .precision)
        if let precision, precision < 0 {
            throw DecodingError.dataCorruptedError(
                forKey: .precision,
                in: container,
                debugDescription: "precision must be non-negative"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        let canonicalQuantity: String
        do {
            canonicalQuantity = try KotodamaNumericV1Codec.decodeQuantityJSON(quantity).canonicalString
        } catch {
            throw EncodingError.invalidValue(
                quantity,
                EncodingError.Context(
                    codingPath: encoder.codingPath + [CodingKeys.quantity],
                    debugDescription: "quantity must be a canonical non-negative Kotodama V1 quantity"
                )
            )
        }
        if let precision, precision < 0 {
            throw EncodingError.invalidValue(
                precision,
                EncodingError.Context(
                    codingPath: encoder.codingPath + [CodingKeys.precision],
                    debugDescription: "precision must be non-negative"
                )
            )
        }
        try container.encode(assetId, forKey: .assetId)
        try container.encodeIfPresent(assetDefinitionId, forKey: .assetDefinitionId)
        try container.encode(canonicalQuantity, forKey: .quantity)
        try container.encodeIfPresent(precision, forKey: .precision)
    }
}

public struct ConnectBalanceSnapshot: Equatable, Sendable {
    public var accountID: String
    public var assets: [ConnectBalanceAsset]
    public var lastUpdatedMs: UInt64?
    public var metadata: [String: String]?
    public var queueDiagnostics: ConnectQueueSnapshot?
    public var sequence: UInt64?
    public var receivedAt: Date?

    public init(accountID: String,
                assets: [ConnectBalanceAsset],
                lastUpdatedMs: UInt64? = nil,
                metadata: [String: String]? = nil,
                queueDiagnostics: ConnectQueueSnapshot? = nil,
                sequence: UInt64? = nil,
                receivedAt: Date? = nil) {
        self.accountID = accountID
        self.assets = assets
        self.lastUpdatedMs = lastUpdatedMs
        self.metadata = metadata
        self.queueDiagnostics = queueDiagnostics
        self.sequence = sequence
        self.receivedAt = receivedAt
    }
}

public struct ConnectEvent: Equatable, Sendable {
    public let sequence: UInt64
    public let direction: ConnectDirection
    public let payload: ConnectEnvelopePayload
    public let receivedAt: Date

    public init(sequence: UInt64,
                direction: ConnectDirection,
                payload: ConnectEnvelopePayload,
                receivedAt: Date = Date()) {
        self.sequence = sequence
        self.direction = direction
        self.payload = payload
        self.receivedAt = receivedAt
    }
}

public enum ConnectEventPayloadKind: String, Sendable, CaseIterable {
    case signRequestTx
    case signRequestRaw
    case signResultOk
    case signResultErr
    case displayRequest
    case controlClose
    case controlReject
    case balanceSnapshot
}

public struct ConnectEventFilter: Sendable {
    public var directions: Set<ConnectDirection>?
    public var payloadKinds: Set<ConnectEventPayloadKind>?
    public var accountIDs: Set<String>?

    public init(directions: Set<ConnectDirection>? = nil,
                payloadKinds: Set<ConnectEventPayloadKind>? = nil,
                accountIDs: Set<String>? = nil) {
        self.directions = directions
        self.payloadKinds = payloadKinds
        self.accountIDs = accountIDs?.isEmpty == true ? nil : accountIDs
    }

    public static func balanceSnapshots(accountID: String?) -> ConnectEventFilter {
        let accounts = accountID.map { Set([$0.lowercased()]) }
        return ConnectEventFilter(directions: nil,
                                  payloadKinds: [.balanceSnapshot],
                                  accountIDs: accounts)
    }

    public func matches(_ event: ConnectEvent) -> Bool {
        if let directions, !directions.contains(event.direction) {
            return false
        }
        if let payloadKinds, !payloadKinds.contains(event.payload.eventKind) {
            return false
        }
        if let accountIDs {
            guard case .balanceSnapshot(let snapshot) = event.payload else { return false }
            let key = snapshot.accountID.lowercased()
            return accountIDs.contains(key)
        }
        return true
    }
}

extension ConnectEnvelopePayload {
    var balanceSnapshotPayload: ConnectBalanceSnapshot? {
        if case .balanceSnapshot(let snapshot) = self {
            return snapshot
        }
        return nil
    }

    var eventKind: ConnectEventPayloadKind {
        switch self {
        case .signRequestTx:
            return .signRequestTx
        case .signRequestRaw:
            return .signRequestRaw
        case .signResultOk:
            return .signResultOk
        case .signResultErr:
            return .signResultErr
        case .displayRequest:
            return .displayRequest
        case .controlClose:
            return .controlClose
        case .controlReject:
            return .controlReject
        case .balanceSnapshot:
            return .balanceSnapshot
        }
    }
}

extension ConnectBalanceAsset {
    init(json: [String: Any]) throws {
        guard let assetId = json["asset_id"] as? String,
              let quantity = json["quantity"] as? String else {
            throw ConnectEnvelopeError.invalidPayload
        }
        let canonicalQuantity: String
        do {
            canonicalQuantity = try KotodamaNumericV1Codec
                .decodeQuantityJSON(quantity).canonicalString
        } catch {
            throw ConnectEnvelopeError.invalidPayload
        }
        let definition = json["asset_definition_id"] as? String
        let precisionValue: Int?
        if let precisionRaw = json["precision"] {
            guard let parsed = StrictJSONNumber.int(from: precisionRaw), parsed >= 0 else {
                throw ConnectEnvelopeError.invalidPayload
            }
            precisionValue = parsed
        } else {
            precisionValue = nil
        }
        self.init(assetId: assetId,
                  assetDefinitionId: definition,
                  quantity: canonicalQuantity,
                  precision: precisionValue)
    }
}

extension ConnectBalanceSnapshot {
    init(json: [String: Any]) throws {
        guard let accountID = json["account_id"] as? String else {
            throw ConnectEnvelopeError.invalidPayload
        }
        guard let assetsArray = json["assets"] as? [[String: Any]] else {
            throw ConnectEnvelopeError.invalidPayload
        }
        let assets = try assetsArray.map { try ConnectBalanceAsset(json: $0) }
        let metadata = try ConnectBalanceSnapshot.parseMetadata(json["metadata"])
        let lastUpdated: UInt64?
        if let raw = json["last_updated_ms"] {
            guard let parsed = StrictJSONNumber.uint64(from: raw) else {
                throw ConnectEnvelopeError.invalidPayload
            }
            lastUpdated = parsed
        } else {
            lastUpdated = nil
        }
        self.init(accountID: accountID,
                  assets: assets,
                  lastUpdatedMs: lastUpdated,
                  metadata: metadata)
    }

    private static func parseMetadata(_ value: Any?) throws -> [String: String]? {
        guard let value else { return nil }
        guard let dict = value as? [String: Any] else {
            throw ConnectEnvelopeError.invalidPayload
        }
        var metadata: [String: String] = [:]
        for (key, raw) in dict {
            guard let stringValue = raw as? String else {
                throw ConnectEnvelopeError.invalidPayload
            }
            metadata[key] = stringValue
        }
        return metadata
    }
}
