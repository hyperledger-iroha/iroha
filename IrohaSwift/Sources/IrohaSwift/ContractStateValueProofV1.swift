import Foundation

/// One root-to-leaf branch in an accumulated contract-state map proof.
public struct ContractStateProofStepV1: Decodable, Sendable {
    public let bit: UInt16
    public let prefix: [UInt8]
    /// Canonical checksummed Iroha hash literal of the other subtree.
    public let sibling: String

    public init(bit: UInt16, prefix: [UInt8], sibling: String) {
        self.bit = bit
        self.prefix = prefix
        self.sibling = sibling
    }

    private enum CodingKeys: String, CodingKey { case bit, prefix, sibling }

    public init(from decoder: Decoder) throws {
        let all = try decoder.container(keyedBy: ContractStateProofAnyKey.self)
        guard Set(all.allKeys.map(\.stringValue)) == Set(["bit", "prefix", "sibling"]) else {
            throw DecodingError.dataCorruptedError(
                in: try decoder.singleValueContainer(),
                debugDescription: "Contract-state proof step has missing or unknown fields"
            )
        }
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        bit = try fields.decode(UInt16.self, forKey: .bit)
        prefix = try fields.decode([UInt8].self, forKey: .prefix)
        sibling = try fields.decode(String.self, forKey: .sibling)
    }
}

/// An exact stored byte value and its compressed-map membership path.
///
/// Membership alone does not establish finality. The root supplied to `verify`
/// must be authenticated through linked Sumeragi V2 finality separately.
public struct ContractStateValueInclusionProofV1: Decodable, Sendable {
    public let version: UInt8
    public let path: String
    public let value: [UInt8]
    public let leafCount: UInt64
    public let steps: [ContractStateProofStepV1]

    public init(
        version: UInt8,
        path: String,
        value: [UInt8],
        leafCount: UInt64,
        steps: [ContractStateProofStepV1]
    ) {
        self.version = version
        self.path = path
        self.value = value
        self.leafCount = leafCount
        self.steps = steps
    }

    /// Decode a bounded, duplicate-key-free Norito JSON membership proof.
    /// Call `verify` with an independently authenticated accumulated root next.
    public static func parseJson(_ payload: Data) throws -> Self {
        guard payload.count <= 1024 * 1024 + 128 * 1024 else {
            throw ContractStateValueProofDecodeError.wireLimit
        }
        try StrictJSONDuplicateKeyRejector.rejectDuplicateObjectKeys(
            in: payload,
            requireAllNumbersInteger: true
        )
        return try JSONDecoder().decode(Self.self, from: payload)
    }

    private enum CodingKeys: String, CodingKey {
        case version, path, value, steps
        case leafCount = "leaf_count"
    }

    public init(from decoder: Decoder) throws {
        let all = try decoder.container(keyedBy: ContractStateProofAnyKey.self)
        guard Set(all.allKeys.map(\.stringValue)) == Set(["version", "path", "value", "leaf_count", "steps"]) else {
            throw DecodingError.dataCorruptedError(
                in: try decoder.singleValueContainer(),
                debugDescription: "Contract-state value proof has missing or unknown fields"
            )
        }
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        version = try fields.decode(UInt8.self, forKey: .version)
        path = try fields.decode(String.self, forKey: .path)
        value = try fields.decode([UInt8].self, forKey: .value)
        leafCount = try fields.decode(UInt64.self, forKey: .leafCount)
        steps = try fields.decode([ContractStateProofStepV1].self, forKey: .steps)
    }

    /// Verify exact value membership under a separately authenticated accumulated root.
    public func verify(expectedPath: String, trustedRoot: Data) -> Bool {
        guard version == 1,
              Self.validPath(expectedPath),
              Array(path.utf8) == Array(expectedPath.utf8),
              value.count <= 1024 * 1024,
              leafCount > 0,
              steps.count <= 256,
              (leafCount == 1) == steps.isEmpty,
              Self.markedHash(Array(trustedRoot)) else { return false }

        let key = Self.hash(Array("iroha:contract-state:key:v1\0".utf8), Array(path.utf8))
        let valueHash = Self.hash(Array("iroha:contract-state:value:v1\0".utf8), value)
        var current = Self.hash(Array("iroha:merkle-map:leaf:v1\0".utf8), key, valueHash)
        var previousBit = -1
        var decodedSteps = [(bit: Int, prefix: [UInt8], sibling: [UInt8])]()
        decodedSteps.reserveCapacity(steps.count)
        for step in steps {
            let bit = Int(step.bit)
            guard bit < 256,
                  bit > previousBit,
                  step.prefix == Self.prefix(key, bit: bit),
                  let sibling = try? NetworkId(literal: step.sibling).bytes,
                  Self.markedHash(Array(sibling)) else { return false }
            previousBit = bit
            decodedSteps.append((bit, step.prefix, Array(sibling)))
        }
        for step in decodedSteps.reversed() {
            let right = key[step.bit / 8] & (0x80 >> (step.bit % 8)) != 0
            let leftHash = right ? step.sibling : current
            let rightHash = right ? current : step.sibling
            current = Self.hash(
                Array("iroha:merkle-map:branch:v1\0".utf8),
                [UInt8(step.bit & 0xff), UInt8(step.bit >> 8)],
                step.prefix,
                leftHash,
                rightHash
            )
        }
        let count = (0..<8).map { UInt8(truncatingIfNeeded: leafCount >> ($0 * 8)) }
        return Self.hash(Array("iroha:merkle-map:root:v1\0".utf8), count, current)
            == Array(trustedRoot)
    }

    private static func hash(_ parts: [UInt8]...) -> [UInt8] {
        var input = Data()
        for part in parts { input.append(contentsOf: part) }
        var digest = Array(Blake2b.hash256(input))
        digest[31] |= 1
        return digest
    }

    private static func markedHash(_ bytes: [UInt8]) -> Bool {
        bytes.count == 32 && bytes[31] & 1 == 1
    }

    private static func prefix(_ key: [UInt8], bit: Int) -> [UInt8] {
        var result = key
        let index = bit / 8
        let remainder = bit % 8
        result[index] = remainder == 0
            ? 0
            : result[index] & UInt8(truncatingIfNeeded: 0xff << (8 - remainder))
        if index + 1 < 32 {
            for position in (index + 1)..<32 { result[position] = 0 }
        }
        return result
    }

    private static func validPath(_ path: String) -> Bool {
        let utf8 = Array(path.utf8)
        guard !utf8.isEmpty,
              utf8.count <= 16 * 1024,
              utf8 == Array(path.precomposedStringWithCanonicalMapping.utf8) else { return false }
        return !path.unicodeScalars.contains { scalar in
            let code = scalar.value
            return scalar.properties.isWhitespace
                || scalar.properties.generalCategory == .control
                || code == 0x40 || code == 0x23 || code == 0x24
                || code == 0x061c || code == 0x200e || code == 0x200f
                || (0x202a...0x202e).contains(code)
                || (0x2066...0x2069).contains(code)
        }
    }
}

private struct ContractStateProofAnyKey: CodingKey {
    let stringValue: String
    let intValue: Int? = nil

    init?(stringValue: String) { self.stringValue = stringValue }
    init?(intValue: Int) { return nil }
}

public enum ContractStateValueProofDecodeError: Error, Sendable {
    case wireLimit
}
