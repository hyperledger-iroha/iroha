import Foundation

public enum ZkAssetMerklePathError: Error, Equatable, LocalizedError {
    case invalidField(String)
    case verificationFailed(String)

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Invalid zk-asset Merkle path field: \(field)."
        case let .verificationFailed(field):
            return "Zk-asset Merkle path verification failed: \(field)."
        }
    }
}

public protocol ZkAssetMerkleHasher: Sendable {
    func hashPair(left: Data, right: Data) throws -> Data
}

public struct PastaPoseidonNodeHasher: ZkAssetMerkleHasher, Sendable {
    public init() {}

    public func hashPair(left: Data, right: Data) throws -> Data {
        guard let lhs = PastaFp.fromCanonicalBytes(left) else {
            throw ZkAssetMerklePathError.invalidField("left")
        }
        guard let rhs = PastaFp.fromCanonicalBytes(right) else {
            throw ZkAssetMerklePathError.invalidField("right")
        }
        return ConfidentialNoteCrypto.poseidonPair(lhs, rhs).canonicalBytes()
    }
}

public protocol ZkAssetMerklePathProvider {
    func getMerklePathForCommitment(asset: String, commitment: Data) async throws -> ZkAssetMerklePath
    func getMerklePaths(asset: String, commitments: [Data]) async throws -> [ZkAssetMerklePath]
}

public struct LocalZkAssetMerklePathProvider: ZkAssetMerklePathProvider {
    public static let confidentialTreeDepthV2 = 16
    public static let confidentialTreeCapacityV2 = 1 << confidentialTreeDepthV2

    private let roots: [Data]
    private let commitments: [Data]
    private let hasher: ZkAssetMerkleHasher

    public init(
        rootHistory: [Data],
        commitmentHistory: [Data],
        hasher: ZkAssetMerkleHasher = PastaPoseidonNodeHasher()
    ) throws {
        self.roots = try rootHistory.enumerated().map { index, root in
            try Self.fixed32(root, field: "rootHistory[\(index)]")
        }
        self.commitments = try commitmentHistory.enumerated().map { index, commitment in
            try Self.fixed32(commitment, field: "commitmentHistory[\(index)]")
        }
        self.hasher = hasher
        guard commitmentHistory.count <= Self.confidentialTreeCapacityV2 else {
            throw ZkAssetMerklePathError.invalidField("commitmentHistory")
        }
    }

    public func getMerklePathForCommitment(
        asset: String,
        commitment: Data
    ) async throws -> ZkAssetMerklePath {
        try validateAsset(asset)
        let copiedCommitment = try Self.fixed32(commitment, field: "commitment")
        let matches = commitments.indices.filter { commitments[$0] == copiedCommitment }
        guard matches.count == 1, let leafIndex = matches.first else {
            throw ZkAssetMerklePathError.invalidField("commitment")
        }
        return try computePath(leafIndex: leafIndex)
    }

    public func getMerklePaths(
        asset: String,
        commitments: [Data]
    ) async throws -> [ZkAssetMerklePath] {
        try validateAsset(asset)
        var out: [ZkAssetMerklePath] = []
        out.reserveCapacity(commitments.count)
        for commitment in commitments {
            out.append(try await getMerklePathForCommitment(asset: asset, commitment: commitment))
        }
        return out
    }

    private func computePath(leafIndex: Int) throws -> ZkAssetMerklePath {
        var layer = commitments.map { Data($0) }
        layer.reserveCapacity(Self.confidentialTreeCapacityV2)
        while layer.count < Self.confidentialTreeCapacityV2 {
            layer.append(Data(repeating: 0, count: 32))
        }

        var siblings: [Data] = []
        siblings.reserveCapacity(Self.confidentialTreeDepthV2)
        var directions = Data()
        directions.reserveCapacity(Self.confidentialTreeDepthV2)
        var currentIndex = leafIndex
        for _ in 0..<Self.confidentialTreeDepthV2 {
            let isRight = currentIndex % 2 == 1
            let siblingIndex = isRight ? currentIndex - 1 : currentIndex + 1
            directions.append(isRight ? 1 : 0)
            siblings.append(layer[siblingIndex])

            var next: [Data] = []
            next.reserveCapacity(layer.count / 2)
            var index = 0
            while index < layer.count {
                next.append(try hasher.hashPair(left: layer[index], right: layer[index + 1]))
                index += 2
            }
            layer = next
            currentIndex /= 2
        }

        guard let root = layer.first, layer.count == 1 else {
            throw ZkAssetMerklePathError.verificationFailed("root")
        }
        if let latest = roots.last, latest != root {
            throw ZkAssetMerklePathError.verificationFailed("rootHistory")
        }
        return try ZkAssetMerklePath(
            leafIndex: UInt64(leafIndex),
            siblings: siblings,
            directions: directions,
            rootAtHeight: root,
            heightOrIndex: UInt64(commitments.count)
        )
    }

    private static func fixed32(_ value: Data, field: String) throws -> Data {
        guard value.count == 32 else {
            throw ZkAssetMerklePathError.invalidField(field)
        }
        return Data(value)
    }

    private func validateAsset(_ asset: String) throws {
        let trimmed = asset.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed == asset else {
            throw ZkAssetMerklePathError.invalidField("asset")
        }
    }
}

public struct ZkAssetMerklePath: Equatable, Sendable {
    public let leafIndex: UInt64
    public let siblings: [Data]
    public let directions: Data
    public let rootAtHeight: Data
    public let heightOrIndex: UInt64

    public init(
        leafIndex: UInt64,
        siblings: [Data],
        directions: Data,
        rootAtHeight: Data,
        heightOrIndex: UInt64
    ) throws {
        guard directions.count == siblings.count else {
            throw ZkAssetMerklePathError.invalidField("directions")
        }
        guard rootAtHeight.count == 32 else {
            throw ZkAssetMerklePathError.invalidField("rootAtHeight")
        }
        guard directions.count < UInt64.bitWidth else {
            throw ZkAssetMerklePathError.invalidField("directions")
        }
        guard (leafIndex >> UInt64(directions.count)) == 0 else {
            throw ZkAssetMerklePathError.invalidField("leafIndex")
        }

        var copiedSiblings: [Data] = []
        copiedSiblings.reserveCapacity(siblings.count)
        for (index, sibling) in siblings.enumerated() {
            guard sibling.count == 32 else {
                throw ZkAssetMerklePathError.invalidField("siblings[\(index)]")
            }
            copiedSiblings.append(Data(sibling))
        }
        for (index, direction) in directions.enumerated() {
            guard direction == 0 || direction == 1 else {
                throw ZkAssetMerklePathError.invalidField("directions[\(index)]")
            }
            let expected = UInt8((leafIndex >> UInt64(index)) & 1)
            guard direction == expected else {
                throw ZkAssetMerklePathError.invalidField("directions[\(index)]")
            }
        }

        self.leafIndex = leafIndex
        self.siblings = copiedSiblings
        self.directions = Data(directions)
        self.rootAtHeight = Data(rootAtHeight)
        self.heightOrIndex = heightOrIndex
    }

    public func verify(
        commitment: Data,
        expectedRoot: Data,
        hasher: ZkAssetMerkleHasher = PastaPoseidonNodeHasher()
    ) throws -> Bool {
        guard commitment.count == 32 else {
            throw ZkAssetMerklePathError.invalidField("commitment")
        }
        guard expectedRoot.count == 32 else {
            throw ZkAssetMerklePathError.invalidField("expectedRoot")
        }
        guard rootAtHeight == expectedRoot else {
            return false
        }
        var current = Data(commitment)
        for index in siblings.indices {
            if directions[index] == 0 {
                current = try hasher.hashPair(left: current, right: siblings[index])
            } else {
                current = try hasher.hashPair(left: siblings[index], right: current)
            }
        }
        return current == expectedRoot
    }
}

public struct ToriiZkMerklePathRequest: Encodable, Sendable {
    public let assetId: String
    public let commitments: [Data]

    public init(assetId: String, commitments: [Data]) throws {
        let trimmed = assetId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed == assetId else {
            throw ZkAssetMerklePathError.invalidField("assetId")
        }
        for (index, commitment) in commitments.enumerated() where commitment.count != 32 {
            throw ZkAssetMerklePathError.invalidField("commitments[\(index)]")
        }
        self.assetId = assetId
        self.commitments = commitments.map { Data($0) }
    }

    private enum CodingKeys: String, CodingKey {
        case assetId = "asset_id"
        case commitments
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(assetId, forKey: .assetId)
        try container.encode(commitments.map { $0.hexEncodedString() }, forKey: .commitments)
    }
}

public struct ToriiZkMerklePathEntry: Decodable, Equatable, Sendable {
    public let commitment: Data
    public let leafIndex: Int
    public let siblings: [Data]
    public let directions: Data
    public let witnessNodes: [Data]
    public let root: Data

    private enum CodingKeys: String, CodingKey {
        case commitment
        case leafIndex = "leaf_index"
        case siblings
        case directions
        case witnessNodes = "witness_nodes"
        case root
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        commitment = try Self.decodeFixed32Hex(container, key: .commitment)
        leafIndex = try container.decode(Int.self, forKey: .leafIndex)
        guard leafIndex >= 0, leafIndex <= Int(Int32.max) else {
            throw DecodingError.dataCorruptedError(
                forKey: .leafIndex,
                in: container,
                debugDescription: "leaf_index must be a u32-compatible integer"
            )
        }
        siblings = try container.decode([String].self, forKey: .siblings).enumerated().map {
            try Self.fixed32Hex($0.element, field: "siblings[\($0.offset)]")
        }
        let directionInts = try container.decode([Int].self, forKey: .directions)
        var directionBytes = Data()
        directionBytes.reserveCapacity(directionInts.count)
        for (index, value) in directionInts.enumerated() {
            guard value == 0 || value == 1 else {
                throw DecodingError.dataCorruptedError(
                    forKey: .directions,
                    in: container,
                    debugDescription: "directions[\(index)] must be 0 or 1"
                )
            }
            directionBytes.append(UInt8(value))
        }
        directions = directionBytes
        witnessNodes = try container.decode([String].self, forKey: .witnessNodes)
            .enumerated()
            .map { try Self.fixed32Hex($0.element, field: "witness_nodes[\($0.offset)]") }
        root = try Self.decodeFixed32Hex(container, key: .root)
        guard siblings.count == directions.count,
              siblings.count == witnessNodes.count else {
            throw DecodingError.dataCorrupted(.init(
                codingPath: decoder.codingPath,
                debugDescription: "path depths must match"
            ))
        }
        guard directions.count < Int.bitWidth,
              (leafIndex >> directions.count) == 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .leafIndex,
                in: container,
                debugDescription: "leaf_index must fit within path depth"
            )
        }
        for (index, direction) in directions.enumerated() {
            let expected = UInt8((leafIndex >> index) & 1)
            guard direction == expected else {
                throw DecodingError.dataCorruptedError(
                    forKey: .directions,
                    in: container,
                    debugDescription: "directions[\(index)] must match leaf_index bit"
                )
            }
        }
    }

    private static func decodeFixed32Hex(
        _ container: KeyedDecodingContainer<CodingKeys>,
        key: CodingKeys
    ) throws -> Data {
        try fixed32Hex(container.decode(String.self, forKey: key), field: key.stringValue)
    }

    static func fixed32Hex(_ value: String, field: String) throws -> Data {
        guard value.count == 64,
              value == value.lowercased(),
              let bytes = Data(hexString: value),
              bytes.count == 32
        else {
            throw DecodingError.dataCorrupted(.init(
                codingPath: [],
                debugDescription: "\(field) must be 32-byte lowercase hex"
            ))
        }
        return bytes
    }
}

public struct ToriiZkMerklePathResponse: Decodable, Equatable, Sendable {
    public let root: Data
    public let frontierLen: Int
    public let treeDepth: Int
    public let paths: [ToriiZkMerklePathEntry]

    private enum CodingKeys: String, CodingKey {
        case root
        case frontierLen = "frontier_len"
        case treeDepth = "tree_depth"
        case paths
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        root = try ToriiZkMerklePathEntry.fixed32Hex(
            container.decode(String.self, forKey: .root),
            field: "root"
        )
        frontierLen = try container.decode(Int.self, forKey: .frontierLen)
        treeDepth = try container.decode(Int.self, forKey: .treeDepth)
        paths = try container.decode([ToriiZkMerklePathEntry].self, forKey: .paths)
        guard frontierLen >= 0, frontierLen <= Int(Int32.max) else {
            throw DecodingError.dataCorruptedError(
                forKey: .frontierLen,
                in: container,
                debugDescription: "frontier_len must be a u32-compatible integer"
            )
        }
        guard treeDepth >= 0, treeDepth <= Int(Int32.max) else {
            throw DecodingError.dataCorruptedError(
                forKey: .treeDepth,
                in: container,
                debugDescription: "tree_depth must be a u32-compatible integer"
            )
        }
        for (index, path) in paths.enumerated() {
            guard path.root == root else {
                throw DecodingError.dataCorrupted(.init(
                    codingPath: decoder.codingPath,
                    debugDescription: "paths[\(index)].root must match response root"
                ))
            }
            guard path.leafIndex < frontierLen else {
                throw DecodingError.dataCorrupted(.init(
                    codingPath: decoder.codingPath,
                    debugDescription: "paths[\(index)].leaf_index must be below frontier_len"
                ))
            }
            guard path.siblings.count == treeDepth,
                  path.directions.count == treeDepth,
                  path.witnessNodes.count == treeDepth else {
                throw DecodingError.dataCorrupted(.init(
                    codingPath: decoder.codingPath,
                    debugDescription: "paths[\(index)] depths must match tree_depth"
                ))
            }
        }
    }

    public static func decodeStrict(_ data: Data) throws -> ToriiZkMerklePathResponse {
        try StrictJSONDuplicateKeyRejector.rejectDuplicateObjectKeys(in: data)
        return try JSONDecoder().decode(ToriiZkMerklePathResponse.self, from: data)
    }

    public func validatedPaths(
        expectedCommitments: [Data],
        hasher: ZkAssetMerkleHasher = PastaPoseidonNodeHasher()
    ) throws -> [ZkAssetMerklePath] {
        guard paths.count == expectedCommitments.count else {
            throw ZkAssetMerklePathError.invalidField("paths")
        }
        var out: [ZkAssetMerklePath] = []
        out.reserveCapacity(paths.count)
        for (index, entry) in paths.enumerated() {
            guard entry.commitment == expectedCommitments[index] else {
                throw ZkAssetMerklePathError.invalidField("paths[\(index)].commitment")
            }
            guard entry.siblings.count == treeDepth,
                  entry.directions.count == treeDepth,
                  entry.witnessNodes.count == treeDepth
            else {
                throw ZkAssetMerklePathError.invalidField("paths[\(index)].siblings")
            }
            guard entry.root == root else {
                throw ZkAssetMerklePathError.invalidField("paths[\(index)].root")
            }
            guard entry.leafIndex < frontierLen else {
                throw ZkAssetMerklePathError.invalidField("paths[\(index)].leaf_index")
            }
            let path = try ZkAssetMerklePath(
                leafIndex: UInt64(entry.leafIndex),
                siblings: entry.siblings,
                directions: entry.directions,
                rootAtHeight: root,
                heightOrIndex: UInt64(frontierLen)
            )
            guard try path.verify(
                commitment: expectedCommitments[index],
                expectedRoot: root,
                hasher: hasher
            ) else {
                throw ZkAssetMerklePathError.verificationFailed("paths[\(index)]")
            }
            out.append(path)
        }
        return out
    }
}

enum StrictJSONDuplicateKeyRejector {
    static func rejectDuplicateObjectKeys(in data: Data) throws {
        guard let text = String(data: data, encoding: .utf8) else {
            throw ZkAssetMerklePathError.invalidField("json")
        }
        var parser = Parser(text)
        try parser.parse()
    }

    private struct Parser {
        private let text: String
        private var index: String.Index

        init(_ text: String) {
            self.text = text
            self.index = text.startIndex
        }

        mutating func parse() throws {
            try parseValue()
            skipWhitespace()
            guard index == text.endIndex else {
                throw ZkAssetMerklePathError.invalidField("json")
            }
        }

        private mutating func parseValue() throws {
            skipWhitespace()
            guard let character = peek() else {
                throw ZkAssetMerklePathError.invalidField("json")
            }
            switch character {
            case "{":
                try parseObject()
            case "[":
                try parseArray()
            case "\"":
                _ = try parseString()
            case "-", "0"..."9":
                try parseNumber()
            case "t":
                try consume("true")
            case "f":
                try consume("false")
            case "n":
                try consume("null")
            default:
                throw ZkAssetMerklePathError.invalidField("json")
            }
        }

        private mutating func parseObject() throws {
            try consume("{")
            skipWhitespace()
            var keys = Set<String>()
            if consumeIf("}") {
                return
            }
            while true {
                skipWhitespace()
                guard peek() == "\"" else {
                    throw ZkAssetMerklePathError.invalidField("json.key")
                }
                let key = try parseString()
                guard keys.insert(key).inserted else {
                    throw ZkAssetMerklePathError.invalidField("json.duplicateKey")
                }
                skipWhitespace()
                try consume(":")
                try parseValue()
                skipWhitespace()
                if consumeIf("}") {
                    return
                }
                try consume(",")
            }
        }

        private mutating func parseArray() throws {
            try consume("[")
            skipWhitespace()
            if consumeIf("]") {
                return
            }
            while true {
                try parseValue()
                skipWhitespace()
                if consumeIf("]") {
                    return
                }
                try consume(",")
            }
        }

        private mutating func parseString() throws -> String {
            try consume("\"")
            var scalars = String.UnicodeScalarView()
            while let character = peek() {
                if character == "\"" {
                    advance()
                    return String(scalars)
                }
                if character == "\\" {
                    advance()
                    guard let escaped = peek() else {
                        throw ZkAssetMerklePathError.invalidField("json.string")
                    }
                    advance()
                    switch escaped {
                    case "\"", "\\", "/":
                        scalars.append(escaped.unicodeScalars.first!)
                    case "b":
                        scalars.append(UnicodeScalar(0x08)!)
                    case "f":
                        scalars.append(UnicodeScalar(0x0c)!)
                    case "n":
                        scalars.append(UnicodeScalar(0x0a)!)
                    case "r":
                        scalars.append(UnicodeScalar(0x0d)!)
                    case "t":
                        scalars.append(UnicodeScalar(0x09)!)
                    case "u":
                        let scalar = try parseUnicodeEscape()
                        scalars.append(scalar)
                    default:
                        throw ZkAssetMerklePathError.invalidField("json.string")
                    }
                } else {
                    guard character.unicodeScalars.allSatisfy({ $0.value >= 0x20 }) else {
                        throw ZkAssetMerklePathError.invalidField("json.string")
                    }
                    for scalar in character.unicodeScalars {
                        scalars.append(scalar)
                    }
                    advance()
                }
            }
            throw ZkAssetMerklePathError.invalidField("json.string")
        }

        private mutating func parseUnicodeEscape() throws -> UnicodeScalar {
            let high = try parseHexQuad()
            if (0xd800...0xdbff).contains(high) {
                let saved = index
                if consumeIf("\\") && consumeIf("u") {
                    let low = try parseHexQuad()
                    guard (0xdc00...0xdfff).contains(low) else {
                        throw ZkAssetMerklePathError.invalidField("json.string")
                    }
                    let value = 0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00)
                    guard let scalar = UnicodeScalar(value) else {
                        throw ZkAssetMerklePathError.invalidField("json.string")
                    }
                    return scalar
                }
                index = saved
                throw ZkAssetMerklePathError.invalidField("json.string")
            }
            guard !(0xdc00...0xdfff).contains(high),
                  let scalar = UnicodeScalar(high) else {
                throw ZkAssetMerklePathError.invalidField("json.string")
            }
            return scalar
        }

        private mutating func parseHexQuad() throws -> UInt32 {
            var value: UInt32 = 0
            for _ in 0..<4 {
                guard let character = peek(),
                      let digit = character.hexDigitValue else {
                    throw ZkAssetMerklePathError.invalidField("json.string")
                }
                value = value * 16 + UInt32(digit)
                advance()
            }
            return value
        }

        private mutating func parseNumber() throws {
            if consumeIf("-") {}
            guard let first = peek(), first.isNumber else {
                throw ZkAssetMerklePathError.invalidField("json.number")
            }
            if first == "0" {
                advance()
            } else {
                while let character = peek(), character.isNumber {
                    advance()
                }
            }
            if consumeIf(".") {
                throw ZkAssetMerklePathError.invalidField("json.number")
            }
            if let character = peek(), character == "e" || character == "E" {
                throw ZkAssetMerklePathError.invalidField("json.number")
            }
        }

        private mutating func skipWhitespace() {
            while let character = peek(), character == " " || character == "\n" || character == "\r" || character == "\t" {
                advance()
            }
        }

        private mutating func consume(_ literal: String) throws {
            guard text[index...].hasPrefix(literal) else {
                throw ZkAssetMerklePathError.invalidField("json")
            }
            index = text.index(index, offsetBy: literal.count)
        }

        private mutating func consumeIf(_ literal: String) -> Bool {
            guard text[index...].hasPrefix(literal) else {
                return false
            }
            index = text.index(index, offsetBy: literal.count)
            return true
        }

        private func peek() -> Character? {
            index == text.endIndex ? nil : text[index]
        }

        private mutating func advance() {
            index = text.index(after: index)
        }
    }
}

extension ToriiClient: ZkAssetMerklePathProvider {
    public func getMerklePaths(asset: String, commitments: [Data]) async throws -> [ZkAssetMerklePath] {
        try await getZkAssetMerklePaths(asset: asset, commitments: commitments)
    }
}
