import Foundation

/// One exact public conviction weight from a standalone election tally.
public struct ToriiElectionTallyWeightV1: Decodable, Equatable, Sendable {
    /// Canonical decimal representation of the unquoted JSON u128 integer.
    public let decimalString: String

    public init(from decoder: Decoder) throws {
        let context = DecodingError.Context(
            codingPath: decoder.codingPath,
            debugDescription: "election tally weight must be an unquoted u128 JSON integer"
        )
        guard let lexemes = decoder.userInfo[exactJSONNumberLexemesUserInfoKey] as? [String: String],
              let token = lexemes[exactJSONNumberCodingPathKey(decoder.codingPath)],
              let value = SccpUInt128.parse(token) else {
            throw DecodingError.dataCorrupted(context)
        }
        decimalString = value.decimalString
    }

    fileprivate func checkedAdding(_ other: Self) -> Self? {
        let left = Array(decimalString.utf8.reversed())
        let right = Array(other.decimalString.utf8.reversed())
        var digits: [UInt8] = []
        digits.reserveCapacity(max(left.count, right.count) + 1)
        var carry: UInt8 = 0
        for index in 0..<max(left.count, right.count) {
            let lhs = index < left.count ? left[index] - 0x30 : 0
            let rhs = index < right.count ? right[index] - 0x30 : 0
            let sum = lhs + rhs + carry
            digits.append(0x30 + sum % 10)
            carry = sum / 10
        }
        if carry != 0 { digits.append(0x30 + carry) }
        let total = String(decoding: digits.reversed(), as: UTF8.self)
        guard let parsed = SccpUInt128.parse(total) else { return nil }
        return Self(decimalString: parsed.decimalString)
    }

    private init(decimalString: String) {
        self.decimalString = decimalString
    }
}

/// The exact four-field Torii V1 projection of one election's public tally.
public struct ToriiElectionTallyResponseV1: Decodable, Equatable, Sendable {
    static let maximumResponseBytes = 8 * 1024

    /// Committed height at which the tally was evaluated.
    public let evaluatedBlockHeight: UInt64
    /// Lowercase hash of the evaluated block, or zero at height zero.
    public let evaluatedBlockHash: String
    /// Whether the election is finalized in the evaluated state.
    public let finalized: Bool
    /// Exact public option weights, in election option order.
    public let tally: [ToriiElectionTallyWeightV1]

    private struct Field: CodingKey {
        let stringValue: String
        let intValue: Int? = nil

        init(stringValue: String) { self.stringValue = stringValue }
        init?(intValue: Int) { return nil }
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Field.self)
        let expected: Set<String> = [
            "evaluated_block_height", "evaluated_block_hash", "finalized", "tally",
        ]
        guard Set(container.allKeys.map(\.stringValue)) == expected else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: decoder.codingPath,
                      debugDescription: "election tally requires exactly four V1 fields")
            )
        }
        let heightKey = Field(stringValue: "evaluated_block_height")
        let hashKey = Field(stringValue: "evaluated_block_hash")
        let finalizedKey = Field(stringValue: "finalized")
        let tallyKey = Field(stringValue: "tally")
        let lexemes = decoder.userInfo[exactJSONNumberLexemesUserInfoKey] as? [String: String]
        let heightPath = exactJSONNumberCodingPathKey(decoder.codingPath + [heightKey])
        guard let heightToken = lexemes?[heightPath],
              let canonicalHeight = SccpUInt128.parse(heightToken),
              let height = UInt64(canonicalHeight.decimalString) else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: decoder.codingPath + [heightKey],
                      debugDescription: "election tally height must be an unquoted u64 JSON integer")
            )
        }
        let hash = try container.decode(String.self, forKey: hashKey)
        let hashBytes = Array(hash.utf8)
        guard hashBytes.count == 64,
              hashBytes.allSatisfy({ (0x30...0x39).contains($0) || (0x61...0x66).contains($0) }),
              (height == 0) == (hash == String(repeating: "0", count: 64)) else {
            throw DecodingError.dataCorruptedError(
                forKey: hashKey, in: container,
                debugDescription: "election tally has invalid evaluated block coordinates"
            )
        }
        let final = try container.decode(Bool.self, forKey: finalizedKey)
        let weights = try container.decode([ToriiElectionTallyWeightV1].self, forKey: tallyKey)
        guard (2...64).contains(weights.count) else {
            throw DecodingError.dataCorruptedError(
                forKey: tallyKey, in: container,
                debugDescription: "election tally must contain 2...64 weights"
            )
        }
        var total = weights[0]
        for weight in weights.dropFirst() {
            guard let next = total.checkedAdding(weight) else {
                throw DecodingError.dataCorruptedError(
                    forKey: tallyKey, in: container,
                    debugDescription: "election tally aggregate exceeds u128"
                )
            }
            total = next
        }
        evaluatedBlockHeight = height
        evaluatedBlockHash = hash
        finalized = final
        tally = weights
    }

    static func decodeExact(from data: Data) throws -> Self {
        guard !data.isEmpty, data.count <= maximumResponseBytes else {
            throw ToriiClientError.invalidPayload("election tally response exceeds its 8 KiB bound")
        }
        let decoder = JSONDecoder()
        decoder.userInfo[exactJSONNumberLexemesUserInfoKey] = try ExactJSONNumberLexemeScanner.scan(data)
        return try decoder.decode(Self.self, from: data)
    }
}

struct ToriiElectionTallyRequestV1: Encodable {
    let electionId: String

    enum CodingKeys: String, CodingKey {
        case electionId = "election_id"
    }
}
