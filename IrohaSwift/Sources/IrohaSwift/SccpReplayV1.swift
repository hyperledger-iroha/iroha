import CryptoKit
import Foundation

/// Closed SCCP replay boundaries shared by SORA and destination contracts.
public enum SccpReplayBoundaryV1: UInt8, CaseIterable, Sendable {
    case soraOutboundLock = 0x01
    case soraInboundRelease = 0x02
    case evmSourceBurn = 0x10
    case evmDestinationMint = 0x11
    case tronSourceBurn = 0x20
    case tronDestinationMint = 0x21
    case tonBridgeInboundMint = 0x30
    case tonBridgeOutboundBurn = 0x31
    case tonMasterMint = 0x32
    case tonMasterBurn = 0x33
    case tonWalletMintCredit = 0x34
    case tonWalletBurnDebit = 0x35
    case tonWalletRefundDebit = 0x36
    case tonWalletRefundCredit = 0x37
}

/// Canonical contract identity committed by a replay domain.
public enum SccpReplayActorV1: Sendable {
    case route
    case evm(Data)
    case tron(Data)
    case ton(workchain: Int32, account: Data)
}

/// Canonical economic principal committed by an occupied replay leaf.
public struct SccpReplayPrincipalV1: Sendable {
    fileprivate let kind: UInt8
    fileprivate let bytes: Data

    private init(kind: UInt8, bytes: Data) {
        self.kind = kind
        self.bytes = Data(bytes)
    }

    /// Construct a SORA principal from one exact canonical compact-Norito
    /// domainless `AccountId` controller payload.
    public static func soraAccount(_ canonicalAccountId: Data) throws -> Self {
        guard !canonicalAccountId.isEmpty,
              canonicalAccountId.count <= 0xffff,
              AccountAddress.isCanonicalCompactNoritoAccountControllerPayload(canonicalAccountId)
        else {
            throw SccpV1Error.invalid("SORA replay principal is not a canonical AccountId")
        }
        return Self(kind: 1, bytes: canonicalAccountId)
    }

    /// Construct an EVM principal from one nonzero 20-byte address.
    public static func evm(_ address: Data) throws -> Self {
        Self(kind: 2, bytes: try SccpReplayV1.exact(address, count: 20, label: "EVM replay principal"))
    }

    /// Construct a TRON principal from one nonzero 20-byte account payload.
    public static func tron(_ address: Data) throws -> Self {
        Self(kind: 3, bytes: try SccpReplayV1.exact(address, count: 20, label: "TRON replay principal"))
    }

    /// Construct a TON principal from its signed workchain and account bytes.
    public static func ton(workchain: Int32, account: Data) throws -> Self {
        let value = UInt32(bitPattern: workchain)
        let prefix = Data([
            UInt8(truncatingIfNeeded: value >> 24),
            UInt8(truncatingIfNeeded: value >> 16),
            UInt8(truncatingIfNeeded: value >> 8),
            UInt8(truncatingIfNeeded: value),
        ])
        return Self(
            kind: 4,
            bytes: prefix + (try SccpReplayV1.exact(account, count: 32, label: "TON replay principal"))
        )
    }
}

/// Canonically compressed sparse-Merkle witness in increasing leaf-up order.
public struct SccpSparseMerkleWitnessV1: Sendable {
    public let expectedShardRoot: Data
    public let priorRecordDigest: Data
    public let siblingBitmap: Data
    public let siblings: [Data]

    public init(
        expectedShardRoot: Data,
        priorRecordDigest: Data,
        siblingBitmap: Data,
        siblings: [Data]
    ) throws {
        self.expectedShardRoot = try SccpReplayV1.exact(
            expectedShardRoot, count: 32, label: "expected shard root")
        self.priorRecordDigest = try SccpReplayV1.exact(
            priorRecordDigest, count: 32, label: "prior record digest", nonzero: false)
        self.siblingBitmap = try SccpReplayV1.exact(
            siblingBitmap, count: 32, label: "sibling bitmap", nonzero: false)
        self.siblings = try siblings.enumerated().map { index, value in
            try SccpReplayV1.exact(value, count: 32, label: "sibling[\(index)]")
        }
    }
}

/// Result of reconstructing one canonical replay witness.
public struct SccpReplayWitnessRootV1: Sendable {
    public let root: Data
    public let expectedRoot: Data
    public let shard: UInt8
    public var matchesExpectedRoot: Bool { root == expectedRoot }
}

/// SHA-256 sparse-Merkle replay hashing shared with Rust and destination runtimes.
public enum SccpReplayV1 {
    public static let depth = 248
    private static let magic = Data("SCCP-REPLAY-SMT-V1".utf8)

    /// Hash one exact production replay domain.
    public static func domainHash(
        source: SccpNetworkV1,
        target: SccpNetworkV1,
        boundary: SccpReplayBoundaryV1,
        routeRevision: UInt32,
        routeConfigurationHash: Data,
        actor: SccpReplayActorV1
    ) throws -> Data {
        guard source.isReplayProduction, target.isReplayProduction, routeRevision != 0 else {
            throw SccpV1Error.invalid("replay domain must use production networks and a nonzero revision")
        }
        let actorValue = try actorParts(actor)
        guard validDirection(source: source, target: target, boundary: boundary, actorKind: actorValue.kind) else {
            throw SccpV1Error.invalid("invalid replay boundary, direction, or actor")
        }
        return hash([
            magic,
            Data([0]),
            unsignedBE(UInt64(source.tag), count: 4),
            unsignedBE(UInt64(target.tag), count: 4),
            Data([boundary.rawValue]),
            unsignedBE(UInt64(routeRevision), count: 4),
            try exact(routeConfigurationHash, count: 32, label: "route configuration hash"),
            Data([actorValue.kind]),
            unsignedBE(UInt64(actorValue.bytes.count), count: 2),
            actorValue.bytes,
        ])
    }

    /// Derive the full replay key; its first byte selects the shard.
    public static func replayKey(domainHash: Data, replayId: Data) throws -> Data {
        hash([
            magic,
            Data([1]),
            try exact(domainHash, count: 32, label: "domain hash"),
            try exact(replayId, count: 32, label: "replay id"),
        ])
    }

    /// Hash one occupied record with an exact 16-byte, big-endian scale-9 u128 amount.
    public static func recordDigest(
        operation: SccpReplayBoundaryV1,
        replayId: Data,
        payloadSHA256: Data,
        amountScale9BE: Data,
        principal: SccpReplayPrincipalV1,
        auxiliaryIdentitySHA256: Data
    ) throws -> Data {
        let amount = try exact(amountScale9BE, count: 16, label: "scale-9 amount")
        let principalValue = try principalParts(principal)
        let principalDigest = hash([
            magic,
            Data([3, principalValue.kind]),
            unsignedBE(UInt64(principalValue.bytes.count), count: 2),
            principalValue.bytes,
        ])
        let auxiliary = hash([
            magic,
            Data([4, operation.rawValue]),
            try exact(auxiliaryIdentitySHA256, count: 32, label: "auxiliary identity SHA-256"),
        ])
        return hash([
            magic,
            Data([2, operation.rawValue]),
            try exact(replayId, count: 32, label: "replay id"),
            try exact(payloadSHA256, count: 32, label: "payload SHA-256"),
            amount,
            principalDigest,
            auxiliary,
        ])
    }

    /// Return all 249 canonical empty hashes in leaf-up order.
    public static func emptyHashes() -> [Data] {
        var hashes = [hash([magic, Data([0x10])])]
        hashes.reserveCapacity(depth + 1)
        for level in 0..<depth {
            hashes.append(parent(level: level, left: hashes[level], right: hashes[level]))
        }
        return hashes
    }

    /// Strictly reconstruct one compressed membership or non-membership witness.
    public static func rootFromWitness(
        key keyValue: Data,
        recordDigest: Data?,
        witness: SccpSparseMerkleWitnessV1
    ) throws -> SccpReplayWitnessRootV1 {
        let key = try exact(keyValue, count: 32, label: "replay key")
        let bitmap = [UInt8](witness.siblingBitmap)
        guard bitmap[0] == 0 else {
            throw SccpV1Error.invalid("witness bitmap has reserved high bits")
        }
        let setBits = bitmap.reduce(0) { $0 + $1.nonzeroBitCount }
        guard setBits == witness.siblings.count, setBits <= depth else {
            throw SccpV1Error.invalid("witness sibling count does not match bitmap")
        }
        let empty = emptyHashes()
        var current: Data
        if let recordDigest {
            let digest = try exact(recordDigest, count: 32, label: "record digest")
            guard digest == witness.priorRecordDigest else {
                throw SccpV1Error.invalid("membership witness record digest mismatch")
            }
            current = hash([magic, Data([0x11]), key, digest])
        } else {
            guard witness.priorRecordDigest.allSatisfy({ $0 == 0 }) else {
                throw SccpV1Error.invalid("non-membership witness has an occupied digest")
            }
            current = empty[0]
        }
        let keyBytes = [UInt8](key)
        var supplied = 0
        for level in 0..<depth {
            var sibling = empty[level]
            if bit(bitmap, level: level) {
                sibling = witness.siblings[supplied]
                supplied += 1
                guard sibling != empty[level] else {
                    throw SccpV1Error.invalid("witness explicitly encodes a default sibling")
                }
            }
            current = bit(keyBytes, level: level)
                ? parent(level: level, left: sibling, right: current)
                : parent(level: level, left: current, right: sibling)
        }
        return SccpReplayWitnessRootV1(
            root: current,
            expectedRoot: witness.expectedShardRoot,
            shard: key.first!
        )
    }

    static func exact(
        _ value: Data,
        count: Int,
        label: String,
        nonzero: Bool = true
    ) throws -> Data {
        guard value.count == count, !nonzero || value.contains(where: { $0 != 0 }) else {
            throw SccpV1Error.invalid("\(label) must be \(nonzero ? "nonzero " : "")\(count) bytes")
        }
        return Data(value)
    }

    private static func actorParts(_ actor: SccpReplayActorV1) throws -> (kind: UInt8, bytes: Data) {
        switch actor {
        case .route:
            return (0, Data())
        case let .evm(address):
            return (1, try exact(address, count: 20, label: "EVM replay actor"))
        case let .tron(address):
            return (2, try exact(address, count: 20, label: "TRON replay actor"))
        case let .ton(workchain, account):
            return (3, signedI32BE(workchain) + (try exact(account, count: 32, label: "TON replay actor")))
        }
    }

    private static func principalParts(
        _ principal: SccpReplayPrincipalV1
    ) throws -> (kind: UInt8, bytes: Data) {
        guard !principal.bytes.isEmpty, principal.bytes.count <= 0xffff else {
            throw SccpV1Error.invalid("invalid replay principal length")
        }
        return (principal.kind, Data(principal.bytes))
    }

    private static func validDirection(
        source: SccpNetworkV1,
        target: SccpNetworkV1,
        boundary: SccpReplayBoundaryV1,
        actorKind: UInt8
    ) -> Bool {
        switch boundary {
        case .soraOutboundLock:
            source == .soraTaira && target.isExternal && actorKind == 0
        case .soraInboundRelease:
            source.isExternal && target == .soraTaira && actorKind == 0
        case .evmSourceBurn:
            [.ethereumMainnet, .bscMainnet].contains(source) && target == .soraTaira && actorKind == 1
        case .evmDestinationMint:
            source == .soraTaira && [.ethereumMainnet, .bscMainnet].contains(target) && actorKind == 1
        case .tronSourceBurn:
            source == .tronMainnet && target == .soraTaira && actorKind == 2
        case .tronDestinationMint:
            source == .soraTaira && target == .tronMainnet && actorKind == 2
        case .tonBridgeInboundMint, .tonMasterMint, .tonWalletMintCredit,
             .tonWalletRefundDebit, .tonWalletRefundCredit:
            source == .soraTaira && target == .tonMainnet && actorKind == 3
        case .tonBridgeOutboundBurn, .tonMasterBurn, .tonWalletBurnDebit:
            source == .tonMainnet && target == .soraTaira && actorKind == 3
        }
    }

    private static func parent(level: Int, left: Data, right: Data) -> Data {
        hash([magic, Data([0x12]), unsignedBE(UInt64(level), count: 2), left, right])
    }

    private static func bit(_ bytes: [UInt8], level: Int) -> Bool {
        (bytes[31 - level / 8] & (UInt8(1) << UInt8(level % 8))) != 0
    }

    private static func unsignedBE(_ value: UInt64, count: Int) -> Data {
        Data((0..<count).map { shift in
            UInt8(truncatingIfNeeded: value >> UInt64((count - 1 - shift) * 8))
        })
    }

    private static func signedI32BE(_ value: Int32) -> Data {
        unsignedBE(UInt64(UInt32(bitPattern: value)), count: 4)
    }

    private static func hash(_ parts: [Data]) -> Data {
        var input = Data()
        for part in parts { input.append(part) }
        return Data(SHA256.hash(data: input))
    }
}

private extension SccpNetworkV1 {
    var isReplayProduction: Bool {
        switch self {
        case .soraTaira, .ethereumMainnet, .bscMainnet, .tronMainnet, .tonMainnet: true
        default: false
        }
    }
}
