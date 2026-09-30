import Foundation

/// Exact dataspace ownership of a complete contract artifact in one selected network.
public struct ContractArtifactId: Codable, Hashable, Sendable {
    /// Full unsigned 64-bit dataspace identifier.
    public let dataspaceId: UInt64
    /// Canonical checksummed hash of the entire deployable artifact.
    public let codeHash: String
    /// Canonical lowercase hash used in Torii paths.
    public let codeHashHex: String

    public init(dataspaceId: UInt64, codeHash: String) throws {
        guard let hex = ToriiCanonicalHashLiteral.normalizedHex(from: codeHash) else {
            throw ToriiClientError.invalidPayload("Artifact codeHash must be a canonical marked hash literal.")
        }
        self.dataspaceId = dataspaceId
        self.codeHash = codeHash
        self.codeHashHex = hex
    }

    private enum CodingKeys: String, CodingKey {
        case dataspaceId = "dataspace_id"
        case codeHash = "code_hash"
    }

    public init(from decoder: Decoder) throws {
        let fields = try decoder.container(keyedBy: ToriiAnyCodingKey.self)
        guard fields.allKeys.allSatisfy({ ["dataspace_id", "code_hash"].contains($0.stringValue) }) else {
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "Unknown artifact identity field."))
        }
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(dataspaceId: container.decode(UInt64.self, forKey: .dataspaceId),
                      codeHash: container.decode(String.self, forKey: .codeHash))
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(dataspaceId, forKey: .dataspaceId)
        try container.encode(codeHash, forKey: .codeHash)
    }
}
