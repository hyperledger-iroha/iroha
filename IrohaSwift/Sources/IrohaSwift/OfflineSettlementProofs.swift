import CryptoKit
import Foundation

public struct ToriiOfflineMutationSettlement: Codable, Sendable, Equatable {
    public let kind: String
    public let operationId: String
    public let chainTxHash: String
    public let entryHash: String
    public let blockHeight: UInt64
    public let preStateHash: String
    public let postStateHash: String
    public let settlementCommitmentHex: String
    public let proof: ToriiOfflineTransparentZkProof

    public init(
        kind: String,
        operationId: String,
        chainTxHash: String,
        entryHash: String,
        blockHeight: UInt64,
        preStateHash: String,
        postStateHash: String,
        settlementCommitmentHex: String,
        proof: ToriiOfflineTransparentZkProof
    ) {
        self.kind = kind
        self.operationId = operationId
        self.chainTxHash = chainTxHash
        self.entryHash = entryHash
        self.blockHeight = blockHeight
        self.preStateHash = preStateHash
        self.postStateHash = postStateHash
        self.settlementCommitmentHex = settlementCommitmentHex
        self.proof = proof
    }

    private enum CodingKeys: String, CodingKey {
        case kind
        case operationId = "operation_id"
        case chainTxHash = "chain_tx_hash"
        case entryHash = "entry_hash"
        case blockHeight = "block_height"
        case preStateHash = "pre_state_hash"
        case postStateHash = "post_state_hash"
        case settlementCommitmentHex = "settlement_commitment_hex"
        case proof
    }
}

public struct ToriiOfflineTransparentZkProof: Codable, Sendable, Equatable {
    public let backend: String
    public let circuitId: String
    public let recursionDepth: UInt8
    public let publicInputsHex: String
    public let envelope: ToriiOfflineStarkVerifyEnvelopeV1

    public init(
        backend: String,
        circuitId: String,
        recursionDepth: UInt8,
        publicInputsHex: String,
        envelope: ToriiOfflineStarkVerifyEnvelopeV1
    ) {
        self.backend = backend
        self.circuitId = circuitId
        self.recursionDepth = recursionDepth
        self.publicInputsHex = publicInputsHex
        self.envelope = envelope
    }

    private enum CodingKeys: String, CodingKey {
        case backend
        case circuitId = "circuit_id"
        case recursionDepth = "recursion_depth"
        case publicInputsHex = "public_inputs_hex"
        case envelope
    }
}

public typealias ToriiOfflineRedeemRequestProof = ToriiOfflineTransparentZkProof

public struct ToriiOfflineStarkVerifyEnvelopeV1: Codable, Sendable, Equatable {
    public let params: ToriiOfflineStarkFriParamsV1
    public let proof: ToriiOfflineStarkProofV1
    public let transcriptLabel: String

    public init(
        params: ToriiOfflineStarkFriParamsV1,
        proof: ToriiOfflineStarkProofV1,
        transcriptLabel: String
    ) {
        self.params = params
        self.proof = proof
        self.transcriptLabel = transcriptLabel
    }

    private enum CodingKeys: String, CodingKey {
        case params
        case proof
        case transcriptLabel = "transcript_label"
    }
}

public struct ToriiOfflineStarkFriParamsV1: Codable, Sendable, Equatable {
    public let version: UInt16
    public let nLog2: UInt8
    public let blowupLog2: UInt8
    public let foldArity: UInt8
    public let queries: UInt16
    public let merkleArity: UInt8
    public let hashFn: UInt8
    public let domainTag: String

    public init(
        version: UInt16,
        nLog2: UInt8,
        blowupLog2: UInt8,
        foldArity: UInt8,
        queries: UInt16,
        merkleArity: UInt8,
        hashFn: UInt8,
        domainTag: String
    ) {
        self.version = version
        self.nLog2 = nLog2
        self.blowupLog2 = blowupLog2
        self.foldArity = foldArity
        self.queries = queries
        self.merkleArity = merkleArity
        self.hashFn = hashFn
        self.domainTag = domainTag
    }

    private enum CodingKeys: String, CodingKey {
        case version
        case nLog2 = "n_log2"
        case blowupLog2 = "blowup_log2"
        case foldArity = "fold_arity"
        case queries
        case merkleArity = "merkle_arity"
        case hashFn = "hash_fn"
        case domainTag = "domain_tag"
    }
}

public struct ToriiOfflineStarkProofV1: Codable, Sendable, Equatable {
    public let version: UInt16
    public let commits: ToriiOfflineStarkCommitmentsV1
    public let queries: [[ToriiOfflineFoldDecommitV1]]

    public init(
        version: UInt16,
        commits: ToriiOfflineStarkCommitmentsV1,
        queries: [[ToriiOfflineFoldDecommitV1]]
    ) {
        self.version = version
        self.commits = commits
        self.queries = queries
    }
}

public struct ToriiOfflineStarkCommitmentsV1: Codable, Sendable, Equatable {
    public let version: UInt16
    public let roots: [String]
    public let compRoot: String?

    public init(version: UInt16, roots: [String], compRoot: String?) {
        self.version = version
        self.roots = roots
        self.compRoot = compRoot
    }

    private enum CodingKeys: String, CodingKey {
        case version
        case roots
        case compRoot = "comp_root"
    }
}

public struct ToriiOfflineFoldDecommitV1: Codable, Sendable, Equatable {
    public let j: UInt32
    public let y0: UInt64
    public let y1: UInt64
    public let pathY0: ToriiOfflineMerklePath
    public let pathY1: ToriiOfflineMerklePath
    public let z: UInt64
    public let pathZ: ToriiOfflineMerklePath

    public init(
        j: UInt32,
        y0: UInt64,
        y1: UInt64,
        pathY0: ToriiOfflineMerklePath,
        pathY1: ToriiOfflineMerklePath,
        z: UInt64,
        pathZ: ToriiOfflineMerklePath
    ) {
        self.j = j
        self.y0 = y0
        self.y1 = y1
        self.pathY0 = pathY0
        self.pathY1 = pathY1
        self.z = z
        self.pathZ = pathZ
    }

    private enum CodingKeys: String, CodingKey {
        case j
        case y0
        case y1
        case pathY0 = "path_y0"
        case pathY1 = "path_y1"
        case z
        case pathZ = "path_z"
    }
}

public struct ToriiOfflineMerklePath: Codable, Sendable, Equatable {
    public let dirs: String
    public let siblings: [String]

    public init(dirs: String, siblings: [String]) {
        self.dirs = dirs
        self.siblings = siblings
    }
}

public enum ToriiOfflineSettlementProofError: LocalizedError, Equatable {
    case missingSettlement
    case invalidSettlement(String)

    public var errorDescription: String? {
        switch self {
        case .missingSettlement:
            return "Offline settlement proof is missing."
        case .invalidSettlement(let message):
            return message
        }
    }
}

public enum ToriiOfflineSettlementProofs {
    public static let settlementBackend = "stark/fri/sha256-goldilocks"
    public static let settlementCircuitId = "offline-bearer-settlement-v1"
    public static let redeemRequestCircuitId = "offline-bearer-redeem-request-v1"

    private static let starkVersion: UInt16 = 1
    private static let starkHashSHA256V1: UInt8 = 1
    private static let starkDomainLog2: UInt8 = 4
    private static let starkBlowupLog2: UInt8 = 3
    private static let starkQueryCount: UInt16 = 8
    private static let zeroFieldElement = Data(repeating: 0, count: 8)

    public static func buildRedeemRequestProof(
        operationId: String,
        accountId: String,
        lineageId: String,
        assetDefinitionId: String,
        amount: String,
        offlinePublicKey: String,
        authorizationId: String,
        preStateHash: String,
        receipts: [ToriiOfflineTransferReceipt]
    ) throws -> ToriiOfflineRedeemRequestProof {
        let canonicalAmount = try ToriiOfflineCashCodec.canonicalAmountString(amount)
        let commitment = try redeemRequestCommitmentHex(
            operationId: operationId,
            accountId: accountId,
            lineageId: lineageId,
            assetDefinitionId: assetDefinitionId,
            amount: canonicalAmount,
            offlinePublicKey: offlinePublicKey,
            authorizationId: authorizationId,
            preStateHash: preStateHash,
            receiptKeys: receiptKeys(receipts)
        )
        return ToriiOfflineTransparentZkProof(
            backend: settlementBackend,
            circuitId: redeemRequestCircuitId,
            recursionDepth: 1,
            publicInputsHex: commitment,
            envelope: synthesizeEnvelope(
                domainTag: commitment,
                transcriptLabel: redeemRequestCircuitId
            )
        )
    }

    public static func verifyRedeemRequestProof(
        proof: ToriiOfflineRedeemRequestProof,
        operationId: String,
        accountId: String,
        lineageId: String,
        assetDefinitionId: String,
        amount: String,
        offlinePublicKey: String,
        authorizationId: String,
        preStateHash: String,
        receipts: [ToriiOfflineTransferReceipt]
    ) throws {
        let expected = try buildRedeemRequestProof(
            operationId: operationId,
            accountId: accountId,
            lineageId: lineageId,
            assetDefinitionId: assetDefinitionId,
            amount: amount,
            offlinePublicKey: offlinePublicKey,
            authorizationId: authorizationId,
            preStateHash: preStateHash,
            receipts: receipts
        )
        let normalizedActual = try normalizeProof(proof)
        let normalizedExpected = try normalizeProof(expected)
        guard normalizedActual == normalizedExpected else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline redeem proof is invalid.")
        }
    }

    public static func buildSettlement(
        kind: String,
        operationId: String,
        accountId: String,
        lineageId: String,
        assetDefinitionId: String,
        amount: String,
        offlinePublicKey: String,
        authorizationId: String,
        preStateHash: String,
        postStateHash: String,
        chainTxHash: String,
        entryHash: String,
        blockHeight: UInt64
    ) throws -> ToriiOfflineMutationSettlement {
        let canonicalAmount = try ToriiOfflineCashCodec.canonicalAmountString(amount)
        let normalizedPreStateHash = try normalizePlainHex(preStateHash, label: "pre-state hash")
        let normalizedPostStateHash = try normalizePlainHex(postStateHash, label: "post-state hash")
        let normalizedChainTxHash = try normalizePlainHex(chainTxHash, label: "chain tx hash")
        let normalizedEntryHash = try normalizePlainHex(entryHash, label: "entry hash")
        let commitment = try settlementCommitmentHex(
            operationId: operationId,
            kind: kind,
            accountId: accountId,
            lineageId: lineageId,
            assetDefinitionId: assetDefinitionId,
            amount: canonicalAmount,
            offlinePublicKey: offlinePublicKey,
            authorizationId: authorizationId,
            preStateHash: normalizedPreStateHash,
            postStateHash: normalizedPostStateHash,
            chainTxHash: normalizedChainTxHash,
            entryHash: normalizedEntryHash,
            blockHeight: blockHeight
        )
        return ToriiOfflineMutationSettlement(
            kind: kind,
            operationId: operationId,
            chainTxHash: normalizedChainTxHash,
            entryHash: normalizedEntryHash,
            blockHeight: blockHeight,
            preStateHash: normalizedPreStateHash,
            postStateHash: normalizedPostStateHash,
            settlementCommitmentHex: commitment,
            proof: ToriiOfflineTransparentZkProof(
                backend: settlementBackend,
                circuitId: settlementCircuitId,
                recursionDepth: 1,
                publicInputsHex: commitment,
                envelope: synthesizeEnvelope(
                    domainTag: commitment,
                    transcriptLabel: settlementCircuitId
                )
            )
        )
    }

    public static func verifySettlement(
        settlement: ToriiOfflineMutationSettlement?,
        kind: String,
        operationId: String,
        accountId: String,
        lineageId: String,
        assetDefinitionId: String,
        amount: String,
        offlinePublicKey: String,
        authorizationId: String,
        preStateHash: String,
        envelopeStateHash: String
    ) throws {
        guard let settlement else {
            throw ToriiOfflineSettlementProofError.missingSettlement
        }
        guard settlement.kind == kind else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline settlement kind is invalid.")
        }
        guard settlement.operationId == operationId else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline settlement operation id is invalid.")
        }
        let normalizedChainTxHash = try normalizePlainHex(settlement.chainTxHash, label: "chain tx hash")
        let normalizedEntryHash = try normalizePlainHex(settlement.entryHash, label: "entry hash")
        let normalizedPreStateHash = try normalizePlainHex(settlement.preStateHash, label: "pre-state hash")
        let normalizedPostStateHash = try normalizePlainHex(settlement.postStateHash, label: "post-state hash")
        let normalizedEnvelopeStateHash = try normalizePlainHex(envelopeStateHash, label: "response state hash")
        guard normalizedPostStateHash == normalizedEnvelopeStateHash else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline settlement post-state hash does not match the response.")
        }
        let canonicalAmount = try ToriiOfflineCashCodec.canonicalAmountString(amount)
        let expectedCommitment = try settlementCommitmentHex(
            operationId: operationId,
            kind: kind,
            accountId: accountId,
            lineageId: lineageId,
            assetDefinitionId: assetDefinitionId,
            amount: canonicalAmount,
            offlinePublicKey: offlinePublicKey,
            authorizationId: authorizationId,
            preStateHash: normalizedPreStateHash,
            postStateHash: normalizedPostStateHash,
            chainTxHash: normalizedChainTxHash,
            entryHash: normalizedEntryHash,
            blockHeight: settlement.blockHeight
        )
        let normalizedCommitment = try normalizePlainHex(
            settlement.settlementCommitmentHex,
            label: "settlement commitment"
        )
        guard normalizedCommitment == expectedCommitment else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline settlement commitment does not match the response.")
        }
        let proof = try normalizeProof(settlement.proof)
        guard proof.backend == settlementBackend else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline settlement proof backend is invalid.")
        }
        guard proof.circuitId == settlementCircuitId else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline settlement proof circuit id is invalid.")
        }
        guard proof.recursionDepth == 1 else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline settlement proof recursion depth is invalid.")
        }
        guard proof.publicInputsHex == expectedCommitment else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline settlement proof public inputs are invalid.")
        }
        guard proof.envelope.params.domainTag == expectedCommitment else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline settlement proof domain tag is invalid.")
        }
        let expectedEnvelope = synthesizeEnvelope(
            domainTag: expectedCommitment,
            transcriptLabel: settlementCircuitId
        )
        let normalizedExpectedEnvelope = try normalizeEnvelope(expectedEnvelope)
        guard proof.envelope == normalizedExpectedEnvelope else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline settlement proof envelope is invalid.")
        }
    }

    private static func settlementCommitmentHex(
        operationId: String,
        kind: String,
        accountId: String,
        lineageId: String,
        assetDefinitionId: String,
        amount: String,
        offlinePublicKey: String,
        authorizationId: String,
        preStateHash: String,
        postStateHash: String,
        chainTxHash: String,
        entryHash: String,
        blockHeight: UInt64
    ) throws -> String {
        try sha256Hex(canonicalJSONData(SettlementCommitmentPayload(
            operationId: operationId,
            kind: kind,
            accountId: accountId,
            lineageId: lineageId,
            assetDefinitionId: assetDefinitionId,
            amount: amount,
            offlinePublicKey: offlinePublicKey,
            authorizationId: authorizationId,
            preStateHash: preStateHash,
            postStateHash: postStateHash,
            chainTxHash: chainTxHash,
            entryHash: entryHash,
            blockHeight: blockHeight
        )))
    }

    private static func redeemRequestCommitmentHex(
        operationId: String,
        accountId: String,
        lineageId: String,
        assetDefinitionId: String,
        amount: String,
        offlinePublicKey: String,
        authorizationId: String,
        preStateHash: String,
        receiptKeys: [String]
    ) throws -> String {
        try sha256Hex(canonicalJSONData(RedeemRequestCommitmentPayload(
            operationId: operationId,
            kind: "redeem_request",
            accountId: accountId,
            lineageId: lineageId,
            assetDefinitionId: assetDefinitionId,
            amount: amount,
            offlinePublicKey: offlinePublicKey,
            authorizationId: authorizationId,
            preStateHash: preStateHash,
            receiptKeys: receiptKeys
        )))
    }

    private static func synthesizeEnvelope(
        domainTag: String,
        transcriptLabel: String
    ) -> ToriiOfflineStarkVerifyEnvelopeV1 {
        let params = ToriiOfflineStarkFriParamsV1(
            version: starkVersion,
            nLog2: starkDomainLog2,
            blowupLog2: starkBlowupLog2,
            foldArity: 2,
            queries: starkQueryCount,
            merkleArity: 2,
            hashFn: starkHashSHA256V1,
            domainTag: domainTag
        )
        let levels = zeroMerkleLevelHashes(maxDepth: Int(starkDomainLog2))
        let requiredLayers = Int(starkDomainLog2)
        let roots = (0...requiredLayers).map { layer in
            prefixedHex(levels[Int(starkDomainLog2) - layer])
        }
        let rootData = roots.compactMap { hex in
            Data(hexString: String(hex.dropFirst(2)))
        }
        let queries = (0..<Int(starkQueryCount)).map { queryIndex in
            synthesizeQueryChain(
                queryIndex: queryIndex,
                params: params,
                transcriptLabel: transcriptLabel,
                roots: rootData,
                levelHashes: levels
            )
        }
        return ToriiOfflineStarkVerifyEnvelopeV1(
            params: params,
            proof: ToriiOfflineStarkProofV1(
                version: starkVersion,
                commits: ToriiOfflineStarkCommitmentsV1(
                    version: starkVersion,
                    roots: roots,
                    compRoot: nil
                ),
                queries: queries
            ),
            transcriptLabel: transcriptLabel
        )
    }

    private static func synthesizeQueryChain(
        queryIndex: Int,
        params: ToriiOfflineStarkFriParamsV1,
        transcriptLabel: String,
        roots: [Data],
        levelHashes: [Data]
    ) -> [ToriiOfflineFoldDecommitV1] {
        let requiredLayers = Int(params.nLog2)
        var indexAtLayer = deriveQueryIndex(
            transcriptLabel: transcriptLabel,
            params: params,
            roots: roots,
            queryIndex: queryIndex
        )
        var chain: [ToriiOfflineFoldDecommitV1] = []
        chain.reserveCapacity(requiredLayers)
        for layer in 0..<requiredLayers {
            let depthCurrent = Int(params.nLog2) - layer
            let depthNext = depthCurrent - 1
            let j = indexAtLayer / 2
            let y0Index = j * 2
            let y1Index = y0Index + 1
            chain.append(
                ToriiOfflineFoldDecommitV1(
                    j: UInt32(j),
                    y0: 0,
                    y1: 0,
                    pathY0: zeroMerklePath(index: y0Index, depth: depthCurrent, levelHashes: levelHashes),
                    pathY1: zeroMerklePath(index: y1Index, depth: depthCurrent, levelHashes: levelHashes),
                    z: 0,
                    pathZ: zeroMerklePath(index: j, depth: depthNext, levelHashes: levelHashes)
                )
            )
            indexAtLayer = j
        }
        return chain
    }

    private static func deriveQueryIndex(
        transcriptLabel: String,
        params: ToriiOfflineStarkFriParamsV1,
        roots: [Data],
        queryIndex: Int
    ) -> Int {
        let domain = 1 << Int(params.nLog2)
        var digest = SHA256()
        digest.update(data: Data("STARK:query-index".utf8))
        digest.update(data: Data(transcriptLabel.utf8))
        digest.update(data: littleEndianData(params.version))
        digest.update(data: Data([
            params.nLog2,
            params.blowupLog2,
            params.foldArity,
            params.merkleArity,
            params.hashFn
        ]))
        digest.update(data: littleEndianData(params.queries))
        digest.update(data: littleEndianData(UInt32(params.domainTag.utf8.count)))
        digest.update(data: Data(params.domainTag.utf8))
        digest.update(data: littleEndianData(UInt64(queryIndex)))
        roots.forEach { digest.update(data: $0) }
        let hash = Data(digest.finalize())
        var value: UInt64 = 0
        for (offset, byte) in hash.prefix(8).enumerated() {
            value |= UInt64(byte) << (offset * 8)
        }
        return Int(value % UInt64(domain))
    }

    private static func zeroMerkleLevelHashes(maxDepth: Int) -> [Data] {
        var levels: [Data] = [leafHash(zeroFieldElement)]
        if maxDepth == 0 {
            return levels
        }
        for _ in 0..<maxDepth {
            let previous = levels[levels.count - 1]
            levels.append(nodeHash(previous, previous))
        }
        return levels
    }

    private static func zeroMerklePath(
        index: Int,
        depth: Int,
        levelHashes: [Data]
    ) -> ToriiOfflineMerklePath {
        var dirs = Data(repeating: 0, count: (depth + 7) / 8)
        var siblings: [String] = []
        siblings.reserveCapacity(depth)
        if depth > 0 {
            for level in 0..<depth {
                if ((index >> level) & 1) == 1 {
                    dirs[level / 8] |= UInt8(1 << (level % 8))
                }
                siblings.append(prefixedHex(levelHashes[level]))
            }
        }
        return ToriiOfflineMerklePath(
            dirs: dirs.base64EncodedString(),
            siblings: siblings
        )
    }

    private static func normalizeProof(
        _ proof: ToriiOfflineTransparentZkProof
    ) throws -> ToriiOfflineTransparentZkProof {
        ToriiOfflineTransparentZkProof(
            backend: proof.backend.trimmingCharacters(in: .whitespacesAndNewlines),
            circuitId: proof.circuitId.trimmingCharacters(in: .whitespacesAndNewlines),
            recursionDepth: proof.recursionDepth,
            publicInputsHex: try normalizePlainHex(proof.publicInputsHex, label: "public inputs"),
            envelope: try normalizeEnvelope(proof.envelope)
        )
    }

    private static func normalizeEnvelope(
        _ envelope: ToriiOfflineStarkVerifyEnvelopeV1
    ) throws -> ToriiOfflineStarkVerifyEnvelopeV1 {
        ToriiOfflineStarkVerifyEnvelopeV1(
            params: ToriiOfflineStarkFriParamsV1(
                version: envelope.params.version,
                nLog2: envelope.params.nLog2,
                blowupLog2: envelope.params.blowupLog2,
                foldArity: envelope.params.foldArity,
                queries: envelope.params.queries,
                merkleArity: envelope.params.merkleArity,
                hashFn: envelope.params.hashFn,
                domainTag: try normalizePlainHex(envelope.params.domainTag, label: "domain tag")
            ),
            proof: ToriiOfflineStarkProofV1(
                version: envelope.proof.version,
                commits: ToriiOfflineStarkCommitmentsV1(
                    version: envelope.proof.commits.version,
                    roots: try envelope.proof.commits.roots.map {
                        try normalizePrefixedHex($0, label: "stark root")
                    },
                    compRoot: try envelope.proof.commits.compRoot.map {
                        try normalizePrefixedHex($0, label: "composition root")
                    }
                ),
                queries: try envelope.proof.queries.map { chain in
                    try chain.map { decommit in
                        ToriiOfflineFoldDecommitV1(
                            j: decommit.j,
                            y0: decommit.y0,
                            y1: decommit.y1,
                            pathY0: try normalizeMerklePath(decommit.pathY0),
                            pathY1: try normalizeMerklePath(decommit.pathY1),
                            z: decommit.z,
                            pathZ: try normalizeMerklePath(decommit.pathZ)
                        )
                    }
                }
            ),
            transcriptLabel: envelope.transcriptLabel.trimmingCharacters(in: .whitespacesAndNewlines)
        )
    }

    private static func normalizeMerklePath(
        _ path: ToriiOfflineMerklePath
    ) throws -> ToriiOfflineMerklePath {
        let trimmedDirs = path.dirs.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmedDirs.isEmpty {
            guard let decoded = Data(base64Encoded: trimmedDirs) else {
                throw ToriiOfflineSettlementProofError.invalidSettlement("Merkle path direction bits are invalid.")
            }
            return ToriiOfflineMerklePath(
                dirs: decoded.base64EncodedString(),
                siblings: try path.siblings.map { try normalizePrefixedHex($0, label: "merkle sibling") }
            )
        }
        return ToriiOfflineMerklePath(
            dirs: "",
            siblings: try path.siblings.map { try normalizePrefixedHex($0, label: "merkle sibling") }
        )
    }

    private static func receiptKeys(_ receipts: [ToriiOfflineTransferReceipt]) -> [String] {
        receipts
            .map { "\($0.transferId):\($0.localRevision)" }
            .sorted()
    }

    private static func leafHash(_ value: Data) -> Data {
        var digest = SHA256()
        digest.update(data: Data("LEAF".utf8))
        digest.update(data: value)
        return Data(digest.finalize())
    }

    private static func nodeHash(_ left: Data, _ right: Data) -> Data {
        var digest = SHA256()
        digest.update(data: left)
        digest.update(data: right)
        return Data(digest.finalize())
    }

    private static func canonicalJSONData<T: Encodable>(_ value: T) throws -> Data {
        let encoder = JSONEncoder()
        if #available(iOS 11.0, macOS 10.13, *) {
            encoder.outputFormatting = [.sortedKeys]
        }
        return try encoder.encode(value)
    }

    private static func sha256Hex(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }

    private static func prefixedHex(_ data: Data) -> String {
        "0x" + data.hexEncodedString()
    }

    private static func littleEndianData(_ value: UInt16) -> Data {
        withUnsafeBytes(of: value.littleEndian) { Data($0) }
    }

    private static func littleEndianData(_ value: UInt32) -> Data {
        withUnsafeBytes(of: value.littleEndian) { Data($0) }
    }

    private static func littleEndianData(_ value: UInt64) -> Data {
        withUnsafeBytes(of: value.littleEndian) { Data($0) }
    }

    private static func normalizePlainHex(_ value: String, label: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let unprefixed = trimmed.hasPrefix("0x") ? String(trimmed.dropFirst(2)) : trimmed
        guard !unprefixed.isEmpty,
              unprefixed.allSatisfy({ $0.isHexDigit }) else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline settlement \(label) is invalid.")
        }
        return unprefixed
    }

    private static func normalizePrefixedHex(_ value: String, label: String) throws -> String {
        "0x" + (try normalizePlainHex(value, label: label))
    }

    private struct SettlementCommitmentPayload: Encodable {
        let operationId: String
        let kind: String
        let accountId: String
        let lineageId: String
        let assetDefinitionId: String
        let amount: String
        let offlinePublicKey: String
        let authorizationId: String
        let preStateHash: String
        let postStateHash: String
        let chainTxHash: String
        let entryHash: String
        let blockHeight: UInt64

        private enum CodingKeys: String, CodingKey {
            case operationId = "operation_id"
            case kind
            case accountId = "account_id"
            case lineageId = "lineage_id"
            case assetDefinitionId = "asset_definition_id"
            case amount
            case offlinePublicKey = "offline_public_key"
            case authorizationId = "authorization_id"
            case preStateHash = "pre_state_hash"
            case postStateHash = "post_state_hash"
            case chainTxHash = "chain_tx_hash"
            case entryHash = "entry_hash"
            case blockHeight = "block_height"
        }
    }

    private struct RedeemRequestCommitmentPayload: Encodable {
        let operationId: String
        let kind: String
        let accountId: String
        let lineageId: String
        let assetDefinitionId: String
        let amount: String
        let offlinePublicKey: String
        let authorizationId: String
        let preStateHash: String
        let receiptKeys: [String]

        private enum CodingKeys: String, CodingKey {
            case operationId = "operation_id"
            case kind
            case accountId = "account_id"
            case lineageId = "lineage_id"
            case assetDefinitionId = "asset_definition_id"
            case amount
            case offlinePublicKey = "offline_public_key"
            case authorizationId = "authorization_id"
            case preStateHash = "pre_state_hash"
            case receiptKeys = "receipt_keys"
        }
    }
}
