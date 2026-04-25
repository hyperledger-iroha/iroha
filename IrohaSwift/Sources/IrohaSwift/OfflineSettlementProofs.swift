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
    ) throws {
        self.kind = kind
        self.operationId = operationId
        self.chainTxHash = try OfflineSettlementFieldCanonicalizer.plainHex(
            chainTxHash,
            label: "chain tx hash"
        )
        self.entryHash = try OfflineSettlementFieldCanonicalizer.plainHex(
            entryHash,
            label: "entry hash"
        )
        self.blockHeight = blockHeight
        self.preStateHash = try OfflineSettlementFieldCanonicalizer.plainHex(
            preStateHash,
            label: "pre-state hash"
        )
        self.postStateHash = try OfflineSettlementFieldCanonicalizer.plainHex(
            postStateHash,
            label: "post-state hash"
        )
        self.settlementCommitmentHex = try OfflineSettlementFieldCanonicalizer.plainHex(
            settlementCommitmentHex,
            label: "settlement commitment"
        )
        self.proof = proof
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            kind: container.decode(String.self, forKey: .kind),
            operationId: container.decode(String.self, forKey: .operationId),
            chainTxHash: container.decode(String.self, forKey: .chainTxHash),
            entryHash: container.decode(String.self, forKey: .entryHash),
            blockHeight: container.decode(UInt64.self, forKey: .blockHeight),
            preStateHash: container.decode(String.self, forKey: .preStateHash),
            postStateHash: container.decode(String.self, forKey: .postStateHash),
            settlementCommitmentHex: container.decode(String.self, forKey: .settlementCommitmentHex),
            proof: container.decode(ToriiOfflineTransparentZkProof.self, forKey: .proof)
        )
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
    ) throws {
        self.backend = OfflineSettlementFieldCanonicalizer.trimmed(backend)
        self.circuitId = OfflineSettlementFieldCanonicalizer.trimmed(circuitId)
        self.recursionDepth = recursionDepth
        self.publicInputsHex = try OfflineSettlementFieldCanonicalizer.plainHex(
            publicInputsHex,
            label: "public inputs"
        )
        self.envelope = envelope
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            backend: container.decode(String.self, forKey: .backend),
            circuitId: container.decode(String.self, forKey: .circuitId),
            recursionDepth: container.decode(UInt8.self, forKey: .recursionDepth),
            publicInputsHex: container.decode(String.self, forKey: .publicInputsHex),
            envelope: container.decode(ToriiOfflineStarkVerifyEnvelopeV1.self, forKey: .envelope)
        )
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
        self.transcriptLabel = OfflineSettlementFieldCanonicalizer.trimmed(transcriptLabel)
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            params: try container.decode(ToriiOfflineStarkFriParamsV1.self, forKey: .params),
            proof: try container.decode(ToriiOfflineStarkProofV1.self, forKey: .proof),
            transcriptLabel: try container.decode(String.self, forKey: .transcriptLabel)
        )
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
    ) throws {
        self.version = version
        self.nLog2 = nLog2
        self.blowupLog2 = blowupLog2
        self.foldArity = foldArity
        self.queries = queries
        self.merkleArity = merkleArity
        self.hashFn = hashFn
        self.domainTag = try OfflineSettlementFieldCanonicalizer.plainHex(
            domainTag,
            label: "domain tag"
        )
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            version: container.decode(UInt16.self, forKey: .version),
            nLog2: container.decode(UInt8.self, forKey: .nLog2),
            blowupLog2: container.decode(UInt8.self, forKey: .blowupLog2),
            foldArity: container.decode(UInt8.self, forKey: .foldArity),
            queries: container.decode(UInt16.self, forKey: .queries),
            merkleArity: container.decode(UInt8.self, forKey: .merkleArity),
            hashFn: container.decode(UInt8.self, forKey: .hashFn),
            domainTag: container.decode(String.self, forKey: .domainTag)
        )
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
    public let compValues: [ToriiOfflineStarkCompositionValueV1]?
    public let air: ToriiOfflineStarkAirProofV1?

    public init(
        version: UInt16,
        commits: ToriiOfflineStarkCommitmentsV1,
        queries: [[ToriiOfflineFoldDecommitV1]],
        compValues: [ToriiOfflineStarkCompositionValueV1]? = nil,
        air: ToriiOfflineStarkAirProofV1? = nil
    ) {
        self.version = version
        self.commits = commits
        self.queries = queries
        self.compValues = compValues
        self.air = air
    }

    private enum CodingKeys: String, CodingKey {
        case version
        case commits
        case queries
        case compValues = "comp_values"
        case air
    }
}

public struct ToriiOfflineStarkCommitmentsV1: Codable, Sendable, Equatable {
    public let version: UInt16
    public let roots: [String]
    public let compRoot: String?

    public init(version: UInt16, roots: [String], compRoot: String?) throws {
        self.version = version
        self.roots = try roots.map {
            try OfflineSettlementFieldCanonicalizer.plainHex($0, label: "stark root")
        }
        self.compRoot = try compRoot.map {
            try OfflineSettlementFieldCanonicalizer.plainHex($0, label: "composition root")
        }
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            version: container.decode(UInt16.self, forKey: .version),
            roots: container.decode([String].self, forKey: .roots),
            compRoot: container.decodeIfPresent(String.self, forKey: .compRoot)
        )
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

public struct ToriiOfflineStarkCompositionTermV1: Codable, Sendable, Equatable {
    public let wireIndex: UInt32
    public let value: UInt64
    public let coeff: UInt64

    public init(wireIndex: UInt32, value: UInt64, coeff: UInt64) {
        self.wireIndex = wireIndex
        self.value = value
        self.coeff = coeff
    }

    private enum CodingKeys: String, CodingKey {
        case wireIndex = "wire_index"
        case value
        case coeff
    }
}

public struct ToriiOfflineStarkCompositionValueV1: Codable, Sendable, Equatable {
    public let leaf: UInt64
    public let constant: UInt64
    public let zCoeff: UInt64
    public let auxTerms: [ToriiOfflineStarkCompositionTermV1]
    public let path: ToriiOfflineMerklePath

    public init(
        leaf: UInt64,
        constant: UInt64,
        zCoeff: UInt64,
        auxTerms: [ToriiOfflineStarkCompositionTermV1],
        path: ToriiOfflineMerklePath
    ) {
        self.leaf = leaf
        self.constant = constant
        self.zCoeff = zCoeff
        self.auxTerms = auxTerms
        self.path = path
    }

    private enum CodingKeys: String, CodingKey {
        case leaf
        case constant
        case zCoeff = "z_coeff"
        case auxTerms = "aux_terms"
        case path
    }
}

public struct ToriiOfflineStarkAirOpeningV1: Codable, Sendable, Equatable {
    public let index: UInt32
    public let row: [UInt64]
    public let nextRow: [UInt64]
    public let rowPath: ToriiOfflineMerklePath
    public let nextRowPath: ToriiOfflineMerklePath
    public let compositionValue: UInt64
    public let compositionPath: ToriiOfflineMerklePath

    public init(
        index: UInt32,
        row: [UInt64],
        nextRow: [UInt64],
        rowPath: ToriiOfflineMerklePath,
        nextRowPath: ToriiOfflineMerklePath,
        compositionValue: UInt64,
        compositionPath: ToriiOfflineMerklePath
    ) {
        self.index = index
        self.row = row
        self.nextRow = nextRow
        self.rowPath = rowPath
        self.nextRowPath = nextRowPath
        self.compositionValue = compositionValue
        self.compositionPath = compositionPath
    }

    private enum CodingKeys: String, CodingKey {
        case index
        case row
        case nextRow = "next_row"
        case rowPath = "row_path"
        case nextRowPath = "next_row_path"
        case compositionValue = "composition_value"
        case compositionPath = "composition_path"
    }
}

public struct ToriiOfflineStarkAirProofV1: Codable, Sendable, Equatable {
    public let version: UInt16
    public let circuitId: String
    public let publicDigest: String
    public let traceRoot: String
    public let compositionRoot: String
    public let traceWidth: UInt16
    public let openings: [ToriiOfflineStarkAirOpeningV1]

    public init(
        version: UInt16,
        circuitId: String,
        publicDigest: String,
        traceRoot: String,
        compositionRoot: String,
        traceWidth: UInt16,
        openings: [ToriiOfflineStarkAirOpeningV1]
    ) throws {
        self.version = version
        self.circuitId = OfflineSettlementFieldCanonicalizer.trimmed(circuitId)
        self.publicDigest = try OfflineSettlementFieldCanonicalizer.plainHex(
            publicDigest,
            label: "STARK AIR public digest"
        )
        self.traceRoot = try OfflineSettlementFieldCanonicalizer.plainHex(
            traceRoot,
            label: "STARK AIR trace root"
        )
        self.compositionRoot = try OfflineSettlementFieldCanonicalizer.plainHex(
            compositionRoot,
            label: "STARK AIR composition root"
        )
        self.traceWidth = traceWidth
        self.openings = openings
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            version: container.decode(UInt16.self, forKey: .version),
            circuitId: container.decode(String.self, forKey: .circuitId),
            publicDigest: container.decode(String.self, forKey: .publicDigest),
            traceRoot: container.decode(String.self, forKey: .traceRoot),
            compositionRoot: container.decode(String.self, forKey: .compositionRoot),
            traceWidth: container.decode(UInt16.self, forKey: .traceWidth),
            openings: container.decode([ToriiOfflineStarkAirOpeningV1].self, forKey: .openings)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case version
        case circuitId = "circuit_id"
        case publicDigest = "public_digest"
        case traceRoot = "trace_root"
        case compositionRoot = "composition_root"
        case traceWidth = "trace_width"
        case openings
    }
}

public struct ToriiOfflineMerklePath: Codable, Sendable, Equatable {
    /// Direction bits stored in-memory as base64. The JSON wire format is the
    /// canonical Torii `Vec<u8>` representation: an array of byte values.
    public let dirs: String
    public let siblings: [String]

    public init(dirs: String, siblings: [String]) throws {
        self.dirs = try OfflineSettlementFieldCanonicalizer.base64(
            dirs,
            label: "Merkle path direction bits"
        )
        self.siblings = try siblings.map {
            try OfflineSettlementFieldCanonicalizer.plainHex($0, label: "merkle sibling")
        }
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let arr = try container.decode([UInt8].self, forKey: .dirs)
        try self.init(
            dirs: Data(arr).base64EncodedString(),
            siblings: container.decode([String].self, forKey: .siblings)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(siblings, forKey: .siblings)
        guard let decoded = Data(base64Encoded: dirs) else {
            throw EncodingError.invalidValue(
                dirs,
                EncodingError.Context(
                    codingPath: container.codingPath + [CodingKeys.dirs],
                    debugDescription: "Merkle path direction bits must be valid base64."
                )
            )
        }
        let bytes = [UInt8](decoded)
        try container.encode(bytes, forKey: .dirs)
    }

    private enum CodingKeys: String, CodingKey {
        case dirs, siblings
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

private enum OfflineSettlementFieldCanonicalizer {
    static func trimmed(_ value: String) -> String {
        value.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    static func plainHex(_ value: String, label: String) throws -> String {
        let normalized = trimmed(value).lowercased()
        guard !normalized.isEmpty,
              !normalized.hasPrefix("0x"),
              normalized.allSatisfy({ $0.isHexDigit }) else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline settlement \(label) is invalid.")
        }
        return normalized
    }

    static func base64(_ value: String, label: String) throws -> String {
        let normalized = trimmed(value)
        if normalized.isEmpty {
            return ""
        }
        guard let decoded = Data(base64Encoded: normalized) else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("\(label) are invalid.")
        }
        return decoded.base64EncodedString()
    }
}

public enum ToriiOfflineSettlementProofs {
    public static let settlementBackend = "stark/fri/sha256-goldilocks"
    public static let settlementCircuitId = "offline-bearer-settlement-v1"
    public static let redeemRequestCircuitId = "offline-bearer-redeem-request-v1"
    public static let sourceLineageCircuitId = "offline-source-lineage-v1"
    public static let sourceLineageFastpqParameter = "fastpq-lane-balanced"

    private static let sourceLineageTransferPayloadPrefix = "wallet-offline-transfer:"
    private static let starkVersion: UInt16 = 1
    private static let starkHashSHA256V1: UInt8 = 1
    private static let starkDomainLog2: UInt8 = 4
    private static let starkBlowupLog2: UInt8 = 3
    private static let starkQueryCount: UInt16 = 8
    private static let starkBindingConstant: UInt64 = 23
    private static let starkBindingZCoefficient: UInt64 = 29
    private static let sourceLineageMaxRecursionDepth = 8
    private static let sourceLineageMaxWitnessBytes = 256 * 1024
    private static let sourceLineageMaxAncestryReceipts = 256
    private static let goldilocksModulus: UInt64 = 18_446_744_069_414_584_321
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
        return try ToriiOfflineTransparentZkProof(
            backend: settlementBackend,
            circuitId: redeemRequestCircuitId,
            recursionDepth: 1,
            publicInputsHex: commitment,
            envelope: try synthesizeEnvelope(
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
        let canonicalAmount = try ToriiOfflineCashCodec.canonicalAmountString(amount)
        let expectedCommitment = try redeemRequestCommitmentHex(
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
        let normalizedActual = try normalizeProof(proof)
        guard normalizedActual.backend == settlementBackend else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline redeem proof backend is invalid.")
        }
        guard normalizedActual.circuitId == redeemRequestCircuitId else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline redeem proof circuit id is invalid.")
        }
        guard normalizedActual.recursionDepth == 1 else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline redeem proof recursion depth is invalid.")
        }
        guard normalizedActual.publicInputsHex == expectedCommitment else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline redeem proof public inputs are invalid.")
        }
        guard normalizedActual.envelope.params.domainTag == expectedCommitment else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline redeem proof domain tag is invalid.")
        }
        try validateTransparentEnvelope(
            normalizedActual.envelope,
            expectedDomainTag: expectedCommitment,
            transcriptLabel: redeemRequestCircuitId,
            context: "redeem proof"
        )
    }

    public static func buildSourceLineageEnvelope(
        publicInputs: ToriiOfflineSourceLineagePublicInputs,
        witnessPayload: String
    ) throws -> ToriiOfflineSourceLineageEnvelope {
        let normalizedWitness = witnessPayload.trimmingCharacters(in: .whitespacesAndNewlines)
        guard normalizedWitness.utf8.count <= sourceLineageMaxWitnessBytes else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Source lineage proof witness exceeds supported limit.")
        }
        let commitment = try sourceLineagePublicInputsCommitmentHex(
            publicInputs,
            witnessPayload: normalizedWitness
        )
        let fastpqProof = try buildSourceLineageFastpqProof(
            publicInputs: publicInputs,
            witnessPayload: normalizedWitness,
            publicInputsCommitmentHex: commitment
        )
        return try ToriiOfflineSourceLineageEnvelope(
            publicInputs: publicInputs,
            witnessPayload: normalizedWitness,
            fastpqProof: fastpqProof,
            proof: ToriiOfflineTransparentZkProof(
                backend: settlementBackend,
                circuitId: sourceLineageCircuitId,
                recursionDepth: 1,
                publicInputsHex: commitment,
                envelope: try synthesizeEnvelope(
                    domainTag: commitment,
                    transcriptLabel: sourceLineageCircuitId,
                    airCircuitId: sourceLineageCircuitId,
                    airPublicDigest: Data(hexString: commitment),
                    includeComposition: false
                )
            )
        )
    }

    public static func verifySourceLineageEnvelope(
        _ envelope: ToriiOfflineSourceLineageEnvelope,
        expectedTransferId: String,
        recipientLineageId: String,
        assetDefinitionId: String,
        amount: String,
        issuerPublicKeyBase64: String,
        revokedVerdictIds: Set<String> = []
    ) throws {
        var sourceLineageNullifiers = Set<String>()
        try verifySourceLineageEnvelopeWithContext(
            envelope,
            expectedTransferId: expectedTransferId,
            recipientLineageId: recipientLineageId,
            assetDefinitionId: assetDefinitionId,
            amount: amount,
            issuerPublicKeyBase64: issuerPublicKeyBase64,
            revokedVerdictIds: revokedVerdictIds,
            sourceLineageNullifiers: &sourceLineageNullifiers,
            sourceDepth: 0
        )
    }

    private static func verifySourceLineageEnvelopeWithContext(
        _ envelope: ToriiOfflineSourceLineageEnvelope,
        expectedTransferId: String,
        recipientLineageId: String,
        assetDefinitionId: String,
        amount: String,
        issuerPublicKeyBase64: String,
        revokedVerdictIds: Set<String>,
        sourceLineageNullifiers: inout Set<String>,
        sourceDepth: Int
    ) throws {
        let witnessPayload = envelope.witnessPayload.trimmingCharacters(in: .whitespacesAndNewlines)
        guard sourceDepth < sourceLineageMaxRecursionDepth,
              witnessPayload.utf8.count <= sourceLineageMaxWitnessBytes,
              sourceLineageNullifiers.insert(envelope.publicInputs.sourceNullifier).inserted else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Source lineage proof witness exceeds supported limit.")
        }
        try validateCanonicalAssetDefinitionIdLiteral(
            assetDefinitionId,
            fieldName: "source_lineage_proof.expected_asset_definition_id"
        )
        try validateCanonicalAssetDefinitionIdLiteral(
            envelope.publicInputs.assetDefinitionId,
            fieldName: "source_lineage_proof.public_inputs.asset_definition_id"
        )
        let sourcePayload = try decodeSourceLineageWitnessPayload(witnessPayload)
        guard envelope.version == 1,
              envelope.circuitId == sourceLineageCircuitId,
              !witnessPayload.isEmpty,
              envelope.publicInputs.transferId == expectedTransferId,
              envelope.publicInputs.recipientLineageId == recipientLineageId,
              envelope.publicInputs.assetDefinitionId == assetDefinitionId,
              try ToriiOfflineCashCodec.compareAmounts(envelope.publicInputs.amount, amount) == .orderedSame,
              envelope.publicInputs.deviceProofCounter > 0 else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Source lineage proof does not match the receipt.")
        }
        try validateSourceLineageWitnessBindings(
            sourcePayload,
            publicInputs: envelope.publicInputs,
            expectedTransferId: expectedTransferId,
            recipientLineageId: recipientLineageId,
            assetDefinitionId: assetDefinitionId,
            amount: amount,
            issuerPublicKeyBase64: issuerPublicKeyBase64,
            revokedVerdictIds: revokedVerdictIds,
            sourceLineageNullifiers: &sourceLineageNullifiers,
            sourceDepth: sourceDepth + 1
        )
        let expectedNullifier = try sourceLineageNullifierHex(envelope.publicInputs)
        guard envelope.publicInputs.sourceNullifier == expectedNullifier else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Source lineage proof nullifier is invalid.")
        }
        let commitment = try sourceLineagePublicInputsCommitmentHex(
            envelope.publicInputs,
            witnessPayload: witnessPayload
        )
        try verifySourceLineageFastpqProof(
            envelope.fastpqProof,
            publicInputs: envelope.publicInputs,
            witnessPayload: witnessPayload,
            publicInputsCommitmentHex: commitment
        )
        let normalizedActual = try normalizeProof(envelope.proof)
        guard normalizedActual.backend == settlementBackend,
              normalizedActual.circuitId == sourceLineageCircuitId,
              normalizedActual.recursionDepth == 1,
              normalizedActual.publicInputsHex == commitment,
              normalizedActual.envelope.params.domainTag == commitment else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Source lineage proof is invalid.")
        }
        guard let air = normalizedActual.envelope.proof.air,
              air.circuitId == sourceLineageCircuitId,
              air.publicDigest == commitment else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Source lineage proof AIR binding is invalid.")
        }
        try validateTransparentEnvelope(
            normalizedActual.envelope,
            expectedDomainTag: commitment,
            transcriptLabel: sourceLineageCircuitId,
            expectedAirCircuitId: sourceLineageCircuitId,
            expectedAirPublicDigest: commitment,
            includeComposition: false,
            context: "source lineage proof"
        )
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
        return try ToriiOfflineMutationSettlement(
            kind: kind,
            operationId: operationId,
            chainTxHash: normalizedChainTxHash,
            entryHash: normalizedEntryHash,
            blockHeight: blockHeight,
            preStateHash: normalizedPreStateHash,
            postStateHash: normalizedPostStateHash,
            settlementCommitmentHex: commitment,
            proof: try ToriiOfflineTransparentZkProof(
                backend: settlementBackend,
                circuitId: settlementCircuitId,
                recursionDepth: 1,
                publicInputsHex: commitment,
                envelope: try synthesizeEnvelope(
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
        try validateTransparentEnvelope(
            proof.envelope,
            expectedDomainTag: expectedCommitment,
            transcriptLabel: settlementCircuitId,
            context: "settlement proof"
        )
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

    public static func sourceLineagePublicInputsCommitmentHex(
        _ publicInputs: ToriiOfflineSourceLineagePublicInputs,
        witnessPayload: String
    ) throws -> String {
        let witnessPayloadHash = sha256Hex(Data(witnessPayload.trimmingCharacters(in: .whitespacesAndNewlines).utf8))
        return try sha256Hex(canonicalJSONData(SourceLineageProofCommitmentPayload(
            circuitId: sourceLineageCircuitId,
            publicInputs: publicInputs,
            witnessPayloadHash: witnessPayloadHash
        )))
    }

    private static func buildSourceLineageFastpqProof(
        publicInputs: ToriiOfflineSourceLineagePublicInputs,
        witnessPayload: String,
        publicInputsCommitmentHex: String
    ) throws -> ToriiOfflineSourceLineageFastpqProof {
        let requestJson = try sourceLineageFastpqProofRequestJson(
            publicInputs: publicInputs,
            witnessPayload: witnessPayload,
            publicInputsCommitmentHex: publicInputsCommitmentHex
        )
        guard let artifactJson = try NoritoNativeBridge.shared.sourceLineageFastpqProof(requestJson: requestJson),
              !artifactJson.isEmpty else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Source lineage FastPQ prover is unavailable."
            )
        }
        let proof = try JSONDecoder().decode(ToriiOfflineSourceLineageFastpqProof.self, from: artifactJson)
        guard proof.parameter == sourceLineageFastpqParameter,
              !proof.proofBytesBase64.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !proof.proofSha256.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !proof.batchManifestSha256.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Source lineage FastPQ proof metadata is invalid."
            )
        }
        return proof
    }

    private static func verifySourceLineageFastpqProof(
        _ proof: ToriiOfflineSourceLineageFastpqProof,
        publicInputs: ToriiOfflineSourceLineagePublicInputs,
        witnessPayload: String,
        publicInputsCommitmentHex: String
    ) throws {
        guard proof.parameter == sourceLineageFastpqParameter,
              !proof.proofBytesBase64.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !proof.proofSha256.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !proof.batchManifestSha256.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Source lineage FastPQ proof metadata is invalid."
            )
        }
        guard let proofBytes = Data(base64Encoded: proof.proofBytesBase64),
              sha256Hex(proofBytes) == proof.proofSha256 else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Source lineage FastPQ proof digest is invalid."
            )
        }
        let requestJson = try sourceLineageFastpqProofRequestJson(
            publicInputs: publicInputs,
            witnessPayload: witnessPayload,
            publicInputsCommitmentHex: publicInputsCommitmentHex
        )
        let artifactJson = try JSONEncoder().encode(proof)
        guard try NoritoNativeBridge.shared.verifySourceLineageFastpqProof(
            requestJson: requestJson,
            artifactJson: artifactJson
        ) == true else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Source lineage FastPQ verifier is unavailable."
            )
        }
    }

    private static func sourceLineageFastpqProofRequestJson(
        publicInputs: ToriiOfflineSourceLineagePublicInputs,
        witnessPayload: String,
        publicInputsCommitmentHex: String
    ) throws -> Data {
        try JSONEncoder().encode(SourceLineageFastpqProofRequest(
            transferId: publicInputs.transferId,
            sourceReceiptHash: publicInputs.sourceReceiptHash,
            sourceNullifier: publicInputs.sourceNullifier,
            publicInputsCommitmentHex: publicInputsCommitmentHex,
            witnessPayload: witnessPayload.trimmingCharacters(in: .whitespacesAndNewlines)
        ))
    }

    public static func sourceLineageNullifierHex(
        _ publicInputs: ToriiOfflineSourceLineagePublicInputs
    ) throws -> String {
        try sha256Hex(canonicalJSONData(SourceLineageNullifierPayload(
            circuitId: sourceLineageCircuitId,
            transferId: publicInputs.transferId,
            sourceReceiptHash: publicInputs.sourceReceiptHash,
            senderLineageId: publicInputs.senderLineageId,
            recipientLineageId: publicInputs.recipientLineageId,
            assetDefinitionId: publicInputs.assetDefinitionId,
            amount: try ToriiOfflineCashCodec.canonicalAmountString(publicInputs.amount),
            sourceLocalRevision: publicInputs.sourceLocalRevision
        )))
    }

    private static func decodeSourceLineageWitnessPayload(
        _ rawPayload: String
    ) throws -> SourceLineageWitnessPayload {
        let trimmed = rawPayload.trimmingCharacters(in: .whitespacesAndNewlines)
        let encoded: String
        if trimmed.hasPrefix(sourceLineageTransferPayloadPrefix) {
            encoded = String(trimmed.dropFirst(sourceLineageTransferPayloadPrefix.count))
                .trimmingCharacters(in: .whitespacesAndNewlines)
        } else {
            encoded = trimmed
        }

        if let decoded = try? decodeBase64URL(encoded),
           let payload = try? JSONDecoder().decode(SourceLineageWitnessPayload.self, from: decoded) {
            return payload
        }

        do {
            return try JSONDecoder().decode(
                SourceLineageWitnessPayload.self,
                from: Data(encoded.utf8)
            )
        } catch {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Source lineage proof witness payload is invalid."
            )
        }
    }

    private static func validateSourceLineageWitnessBindings(
        _ sourcePayload: SourceLineageWitnessPayload,
        publicInputs: ToriiOfflineSourceLineagePublicInputs,
        expectedTransferId: String,
        recipientLineageId: String,
        assetDefinitionId: String,
        amount: String,
        issuerPublicKeyBase64: String,
        revokedVerdictIds: Set<String>,
        sourceLineageNullifiers: inout Set<String>,
        sourceDepth: Int
    ) throws {
        let receipt = sourcePayload.receipt
        let sourceReceiptHash = try sha256Hex(canonicalJSONData(receipt))
        try validateSourceLineageWitnessPayload(
            sourcePayload,
            issuerPublicKeyBase64: issuerPublicKeyBase64,
            revokedVerdictIds: revokedVerdictIds,
            sourceLineageNullifiers: &sourceLineageNullifiers,
            sourceDepth: sourceDepth
        )
        guard sourcePayload.version == 1,
              sourcePayload.anchor.lineageId == receipt.lineageId,
              sourcePayload.anchor.accountId == receipt.accountId,
              sourcePayload.anchor.deviceId == receipt.deviceId,
              sourcePayload.anchor.offlinePublicKey == receipt.offlinePublicKey,
              receipt.direction == ToriiOfflineTransferDirection.outgoing.rawValue,
              receipt.transferId == expectedTransferId,
              receipt.counterpartyLineageId == recipientLineageId,
              try ToriiOfflineCashCodec.compareAmounts(receipt.amount, amount) == .orderedSame,
              publicInputs.transferId == receipt.transferId,
              publicInputs.sourceReceiptHash == sourceReceiptHash,
              publicInputs.senderLineageId == receipt.lineageId,
              publicInputs.recipientLineageId == receipt.counterpartyLineageId,
              publicInputs.assetDefinitionId == assetDefinitionId,
              publicInputs.assetDefinitionId == sourcePayload.anchor.assetDefinitionId,
              publicInputs.sourcePreStateHash == receipt.preStateHash,
              publicInputs.sourcePostStateHash == receipt.postStateHash,
              publicInputs.sourceLocalRevision == receipt.localRevision,
              publicInputs.deviceProofKeyId == receipt.attestation.keyId,
              publicInputs.deviceProofCounter == receipt.attestation.counter else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Source lineage proof does not match the receipt."
            )
        }

        do {
            try ToriiOfflineCashCodec.verifyReceiptSignature(try receipt.cashReceipt())
        } catch {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Source lineage proof witness signature is invalid."
            )
        }
    }

    private static func validateSourceLineageWitnessPayload(
        _ sourcePayload: SourceLineageWitnessPayload,
        issuerPublicKeyBase64: String,
        revokedVerdictIds: Set<String>,
        sourceLineageNullifiers: inout Set<String>,
        sourceDepth: Int
    ) throws {
        guard sourcePayload.ancestryReceipts.count <= sourceLineageMaxAncestryReceipts else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Source lineage proof witness ancestry exceeds supported limit."
            )
        }
        try validateCanonicalLineageStateIdentifiers(
            sourcePayload.anchor,
            fieldName: "source_payload.anchor"
        )
        try ToriiOfflineCashCodec.verifyIssuerSignature(
            authorization: sourcePayload.anchor.authorization,
            issuerPublicKeyBase64: issuerPublicKeyBase64
        )
        try ToriiOfflineCashCodec.verifyIssuerSignature(
            lineageState: sourcePayload.anchor,
            issuerPublicKeyBase64: issuerPublicKeyBase64
        )

        var currentBalance = sourcePayload.anchor.balance
        var currentParked = try minimumRequiredLockedBalance(
            totalBalance: currentBalance,
            authorization: sourcePayload.anchor.authorization,
            nowMs: sourcePayload.ancestryReceipts.first?.createdAtMs
                ?? sourcePayload.receipt.createdAtMs
        )
        var currentHash = sourcePayload.anchor.serverStateHash
        var currentRevision = sourcePayload.anchor.pendingLocalRevision
        var counterBook: [String: UInt64] = [:]
        var seenSenderStates = Set<String>()

        for receipt in sourcePayload.ancestryReceipts.sorted(by: { $0.localRevision < $1.localRevision }) {
            try validateCanonicalTransferReceiptIdentifiers(
                receipt,
                fieldName: "source_payload.ancestry_receipts"
            )
            try validateSourceLineageReceipt(
                receipt,
                expectedLineageId: sourcePayload.anchor.lineageId,
                expectedOfflinePublicKey: sourcePayload.anchor.offlinePublicKey,
                expectedAssetDefinitionId: sourcePayload.anchor.assetDefinitionId,
                currentBalance: &currentBalance,
                currentParked: &currentParked,
                currentHash: &currentHash,
                currentRevision: &currentRevision,
                counterBook: &counterBook,
                seenSenderStates: &seenSenderStates,
                issuerPublicKeyBase64: issuerPublicKeyBase64,
                revokedVerdictIds: revokedVerdictIds,
                sourceLineageNullifiers: &sourceLineageNullifiers,
                sourceDepth: sourceDepth,
                duplicateMessage: "duplicate sender state in ancestry receipts"
            )
        }

        try validateCanonicalTransferReceiptIdentifiers(
            sourcePayload.receipt,
            fieldName: "source_payload.receipt"
        )
        try validateSourceLineageReceipt(
            sourcePayload.receipt,
            expectedLineageId: sourcePayload.anchor.lineageId,
            expectedOfflinePublicKey: sourcePayload.anchor.offlinePublicKey,
            expectedAssetDefinitionId: sourcePayload.anchor.assetDefinitionId,
            currentBalance: &currentBalance,
            currentParked: &currentParked,
            currentHash: &currentHash,
            currentRevision: &currentRevision,
            counterBook: &counterBook,
            seenSenderStates: &seenSenderStates,
            issuerPublicKeyBase64: issuerPublicKeyBase64,
            revokedVerdictIds: revokedVerdictIds,
            sourceLineageNullifiers: &sourceLineageNullifiers,
            sourceDepth: sourceDepth,
            duplicateMessage: "duplicate sender state in outgoing payload"
        )
    }

    private static func validateSourceLineageReceipt(
        _ receipt: SourceLineageWitnessReceipt,
        expectedLineageId: String,
        expectedOfflinePublicKey: String,
        expectedAssetDefinitionId: String,
        currentBalance: inout String,
        currentParked: inout String,
        currentHash: inout String,
        currentRevision: inout UInt64,
        counterBook: inout [String: UInt64],
        seenSenderStates: inout Set<String>,
        issuerPublicKeyBase64: String,
        revokedVerdictIds: Set<String>,
        sourceLineageNullifiers: inout Set<String>,
        sourceDepth: Int,
        duplicateMessage: String
    ) throws {
        let stateKey = "\(receipt.lineageId):\(receipt.localRevision)"
        guard seenSenderStates.insert(stateKey).inserted else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(duplicateMessage)
        }
        try ToriiOfflineCashCodec.verifyReceiptSignature(try receipt.cashReceipt())
        try validateAttestationHash(receipt)
        try validateCounter(receipt.attestation, counterBook: &counterBook)
        currentBalance = try validateLocalContinuity(
            receipt,
            expectedLineageId: expectedLineageId,
            expectedOfflinePublicKey: expectedOfflinePublicKey,
            expectedAssetDefinitionId: expectedAssetDefinitionId,
            currentBalance: currentBalance,
            currentParked: currentParked,
            currentHash: currentHash,
            currentRevision: currentRevision,
            issuerPublicKeyBase64: issuerPublicKeyBase64,
            revokedVerdictIds: revokedVerdictIds,
            sourceLineageNullifiers: &sourceLineageNullifiers,
            sourceDepth: sourceDepth
        )
        currentParked = receipt.postLockedBalance
        currentHash = receipt.postStateHash
        currentRevision = receipt.localRevision
    }

    private static func validateLocalContinuity(
        _ receipt: SourceLineageWitnessReceipt,
        expectedLineageId: String,
        expectedOfflinePublicKey: String,
        expectedAssetDefinitionId: String,
        currentBalance: String,
        currentParked: String,
        currentHash: String,
        currentRevision: UInt64,
        issuerPublicKeyBase64: String,
        revokedVerdictIds: Set<String>,
        sourceLineageNullifiers: inout Set<String>,
        sourceDepth: Int
    ) throws -> String {
        guard receipt.lineageId == expectedLineageId,
              receipt.offlinePublicKey == expectedOfflinePublicKey,
              receipt.localRevision == currentRevision + 1,
              receipt.preBalance == currentBalance,
              receipt.preLockedBalance == currentParked,
              receipt.preStateHash == currentHash,
              receipt.sourcePayload == nil else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline cash continuity proof is invalid."
            )
        }
        guard receipt.direction == ToriiOfflineTransferDirection.incoming.rawValue
                || receipt.sourceLineageProof == nil else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Source lineage proof is only valid for incoming offline receipts."
            )
        }

        let expectedPostBalance: String
        switch receipt.direction {
        case ToriiOfflineTransferDirection.outgoing.rawValue:
            try validateReceiptAuthorization(
                receipt,
                requiresActiveAuthorization: true,
                issuerPublicKeyBase64: issuerPublicKeyBase64,
                revokedVerdictIds: revokedVerdictIds
            )
            let spendable = try ToriiOfflineCashCodec.subtractAmounts(currentBalance, currentParked)
            guard try ToriiOfflineCashCodec.compareAmounts(receipt.amount, spendable) != .orderedDescending else {
                throw ToriiOfflineSettlementProofError.invalidSettlement(
                    "Offline outgoing receipt exceeds sender spendable balance."
                )
            }
            expectedPostBalance = try ToriiOfflineCashCodec.subtractAmounts(currentBalance, receipt.amount)
        case ToriiOfflineTransferDirection.incoming.rawValue:
            try validateReceiptAuthorization(
                receipt,
                requiresActiveAuthorization: false,
                issuerPublicKeyBase64: issuerPublicKeyBase64,
                revokedVerdictIds: revokedVerdictIds
            )
            guard let sourceLineageProof = receipt.sourceLineageProof else {
                throw ToriiOfflineSettlementProofError.invalidSettlement(
                    "Incoming receipt is missing source lineage proof."
                )
            }
            try verifySourceLineageEnvelopeWithContext(
                sourceLineageProof,
                expectedTransferId: receipt.transferId,
                recipientLineageId: receipt.lineageId,
                assetDefinitionId: expectedAssetDefinitionId,
                amount: receipt.amount,
                issuerPublicKeyBase64: issuerPublicKeyBase64,
                revokedVerdictIds: revokedVerdictIds,
                sourceLineageNullifiers: &sourceLineageNullifiers,
                sourceDepth: sourceDepth
            )
            expectedPostBalance = try ToriiOfflineCashCodec.addAmounts(currentBalance, receipt.amount)
        default:
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline receipt direction must be incoming or outgoing."
            )
        }

        try validateParkedContinuity(receipt, expectedPostBalance: expectedPostBalance)
        guard let direction = ToriiOfflineTransferDirection(rawValue: receipt.direction) else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline receipt direction must be incoming or outgoing."
            )
        }
        let expectedPostHash = try ToriiOfflineCashCodec.nextLocalStateHash(
            lineageId: receipt.lineageId,
            previousStateHash: currentHash,
            transferId: receipt.transferId,
            direction: direction,
            counterpartyLineageId: receipt.counterpartyLineageId,
            amount: receipt.amount,
            localRevision: receipt.localRevision,
            postBalance: expectedPostBalance,
            postLockedBalance: receipt.postLockedBalance
        )
        guard receipt.postBalance == expectedPostBalance,
              receipt.postStateHash == expectedPostHash else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline cash continuity proof is invalid."
            )
        }
        return expectedPostBalance
    }

    private static func validateReceiptAuthorization(
        _ receipt: SourceLineageWitnessReceipt,
        requiresActiveAuthorization: Bool,
        issuerPublicKeyBase64: String,
        revokedVerdictIds: Set<String>
    ) throws {
        guard let authorization = receipt.authorization else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline transfer receipt is missing an authorization snapshot."
            )
        }
        try ToriiOfflineCashCodec.verifyIssuerSignature(
            authorization: authorization.cashAuthorization(),
            issuerPublicKeyBase64: issuerPublicKeyBase64
        )
        guard authorization.lineageId == receipt.lineageId,
              authorization.accountId == receipt.accountId,
              authorization.deviceId == receipt.deviceId,
              authorization.offlinePublicKey == receipt.offlinePublicKey,
              authorization.appAttestKeyId == receipt.attestation.keyId else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline transfer authorization does not match the sender offline cash lineage."
            )
        }
        let revoked = Set(revokedVerdictIds.map { $0.lowercased() })
        guard !revoked.contains(authorization.verdictId.lowercased()) else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline transfer authorization has been revoked."
            )
        }
        guard !requiresActiveAuthorization ||
                (receipt.createdAtMs >= authorization.issuedAtMs &&
                 receipt.createdAtMs <= authorization.expiresAtMs) else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline transfer authorization is expired."
            )
        }
        if requiresActiveAuthorization,
           try ToriiOfflineCashCodec.compareAmounts(
            receipt.amount,
            authorization.maxTxValue
           ) == .orderedDescending {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline transfer exceeds the sender authorization policy."
            )
        }
    }

    private static func validateAttestationHash(_ receipt: SourceLineageWitnessReceipt) throws {
        let operation: String
        let transferPayload: Data
        switch receipt.direction {
        case ToriiOfflineTransferDirection.incoming.rawValue:
            operation = "receive"
            transferPayload = try canonicalJSONData(SourceLineageAttestationReceivePayload(
                lineageId: receipt.lineageId,
                transferId: receipt.transferId,
                amount: receipt.amount,
                senderLineageId: receipt.counterpartyLineageId
            ))
        case ToriiOfflineTransferDirection.outgoing.rawValue:
            operation = "send"
            transferPayload = try canonicalJSONData(SourceLineageAttestationSendPayload(
                lineageId: receipt.lineageId,
                transferId: receipt.transferId,
                amount: receipt.amount,
                receiverLineageId: receipt.counterpartyLineageId
            ))
        default:
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline receipt direction must be incoming or outgoing."
            )
        }
        let expected = try sha256Hex(canonicalJSONData(SourceLineageAttestationChallengePayload(
            accountId: receipt.accountId,
            lineageId: receipt.lineageId,
            operation: operation,
            payloadHash: sha256Hex(transferPayload)
        )))
        guard receipt.attestation.challengeHashHex == expected else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline transfer attestation challenge hash is invalid."
            )
        }
    }

    private static func validateCounter(
        _ attestation: SourceLineageWitnessAttestation,
        counterBook: inout [String: UInt64]
    ) throws {
        let previous = counterBook[attestation.keyId] ?? 0
        guard attestation.counter > previous else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline transfer counter replay detected."
            )
        }
        counterBook[attestation.keyId] = attestation.counter
    }

    private static func minimumRequiredLockedBalance(
        totalBalance: String,
        authorization: ToriiOfflineSpendAuthorization?,
        nowMs: UInt64
    ) throws -> String {
        let canonicalTotal = try ToriiOfflineCashCodec.canonicalAmountString(totalBalance)
        guard let authorization else {
            return canonicalTotal
        }
        if nowMs < authorization.issuedAtMs || nowMs > authorization.expiresAtMs {
            return canonicalTotal
        }
        if try ToriiOfflineCashCodec.compareAmounts(
            canonicalTotal,
            authorization.policyMaxBalance
        ) != .orderedDescending {
            return "0"
        }
        return try ToriiOfflineCashCodec.subtractAmounts(canonicalTotal, authorization.policyMaxBalance)
    }

    private static func validateParkedContinuity(
        _ receipt: SourceLineageWitnessReceipt,
        expectedPostBalance: String
    ) throws {
        guard let authorization = try receipt.authorization?.cashAuthorization() else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline transfer receipt is missing an authorization snapshot."
            )
        }
        let minimumPreParked = try minimumRequiredLockedBalance(
            totalBalance: receipt.preBalance,
            authorization: authorization,
            nowMs: receipt.createdAtMs
        )
        let minimumPostParked = try minimumRequiredLockedBalance(
            totalBalance: expectedPostBalance,
            authorization: authorization,
            nowMs: receipt.createdAtMs
        )
        switch receipt.direction {
        case ToriiOfflineTransferDirection.outgoing.rawValue:
            guard receipt.preLockedBalance == minimumPreParked,
                  receipt.postLockedBalance == minimumPostParked else {
                throw ToriiOfflineSettlementProofError.invalidSettlement(
                    "Offline cash locked-balance continuity is invalid."
                )
            }
        case ToriiOfflineTransferDirection.incoming.rawValue:
            guard try ToriiOfflineCashCodec.compareAmounts(
                receipt.preLockedBalance,
                minimumPreParked
            ) != .orderedAscending,
                  try ToriiOfflineCashCodec.compareAmounts(
                    receipt.postLockedBalance,
                    minimumPostParked
                  ) != .orderedAscending,
                  try ToriiOfflineCashCodec.compareAmounts(
                    receipt.preLockedBalance,
                    receipt.preBalance
                  ) != .orderedDescending,
                  try ToriiOfflineCashCodec.compareAmounts(
                    receipt.postLockedBalance,
                    expectedPostBalance
                  ) != .orderedDescending else {
                throw ToriiOfflineSettlementProofError.invalidSettlement(
                    "Offline cash locked-balance continuity is invalid."
                )
            }
        default:
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline receipt direction must be incoming or outgoing."
            )
        }
    }

    private static func validateCanonicalAccountIdLiteral(
        _ value: String,
        fieldName: String
    ) throws {
        do {
            _ = try AccountAddress.parseEncoded(value.trimmingCharacters(in: .whitespacesAndNewlines))
        } catch {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "\(fieldName) must be a canonical I105 account id."
            )
        }
    }

    private static func validateCanonicalAssetDefinitionIdLiteral(
        _ value: String,
        fieldName: String
    ) throws {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard AssetDefinitionAddress.decode(trimmed) != nil else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "\(fieldName) must be a canonical Base58 asset definition id."
            )
        }
    }

    private static func validateCanonicalAuthorizationIdentifiers(
        _ authorization: SourceLineageWitnessAuthorization,
        fieldName: String
    ) throws {
        try validateCanonicalAccountIdLiteral(
            authorization.accountId,
            fieldName: "\(fieldName).account_id"
        )
    }

    private static func validateCanonicalAuthorizationIdentifiers(
        _ authorization: ToriiOfflineSpendAuthorization,
        fieldName: String
    ) throws {
        try validateCanonicalAccountIdLiteral(
            authorization.accountId,
            fieldName: "\(fieldName).account_id"
        )
    }

    private static func validateCanonicalLineageStateIdentifiers(
        _ state: ToriiOfflineCashState,
        fieldName: String
    ) throws {
        try validateCanonicalAccountIdLiteral(
            state.accountId,
            fieldName: "\(fieldName).account_id"
        )
        try validateCanonicalAssetDefinitionIdLiteral(
            state.assetDefinitionId,
            fieldName: "\(fieldName).asset_definition_id"
        )
        try validateCanonicalAuthorizationIdentifiers(
            state.authorization,
            fieldName: "\(fieldName).authorization"
        )
    }

    private static func validateCanonicalTransferReceiptIdentifiers(
        _ receipt: SourceLineageWitnessReceipt,
        fieldName: String
    ) throws {
        try validateCanonicalAccountIdLiteral(
            receipt.accountId,
            fieldName: "\(fieldName).account_id"
        )
        try validateCanonicalAccountIdLiteral(
            receipt.counterpartyAccountId,
            fieldName: "\(fieldName).counterparty_account_id"
        )
        if let authorization = receipt.authorization {
            try validateCanonicalAuthorizationIdentifiers(
                authorization,
                fieldName: "\(fieldName).authorization"
            )
        }
    }

    private static func decodeBase64URL(_ raw: String) throws -> Data {
        var normalized = raw
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        while normalized.count % 4 != 0 {
            normalized.append("=")
        }
        guard let decoded = Data(base64Encoded: normalized) else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Source lineage proof witness payload is invalid."
            )
        }
        return decoded
    }

    private static func synthesizeEnvelope(
        domainTag: String,
        transcriptLabel: String,
        airCircuitId: String = "composition-v1",
        airPublicDigest: Data? = nil,
        includeComposition: Bool = true
    ) throws -> ToriiOfflineStarkVerifyEnvelopeV1 {
        let params = try ToriiOfflineStarkFriParamsV1(
            version: starkVersion,
            nLog2: starkDomainLog2,
            blowupLog2: starkBlowupLog2,
            foldArity: 2,
            queries: starkQueryCount,
            merkleArity: 2,
            hashFn: starkHashSHA256V1,
            domainTag: domainTag
        )
        let publicDigest = airPublicDigest ?? starkAirPublicDigestFromComposition(
            domainTag: domainTag,
            transcriptLabel: transcriptLabel
        )
        let domain = 1 << Int(starkDomainLog2)
        let airRows = (0..<domain).map { index in
            starkAirRow(index: index, publicDigest: publicDigest)
        }
        let traceLevels = merkleLevelsFromHashes(airRows.map(starkAirTraceLeafHash))
        let traceRoot = merkleRootFromLevels(traceLevels)
        let levels = zeroMerkleLevelHashes(maxDepth: Int(starkDomainLog2))
        let requiredLayers = Int(starkDomainLog2)
        let compositionRoot = levels[Int(starkDomainLog2)]
        let roots = (0...requiredLayers).map { layer in
            prefixedHex(levels[Int(starkDomainLog2) - layer])
        }
        let rootData = roots.compactMap { hex in Data(hexString: hex) }
        let queryRoots = rootData + [traceRoot, compositionRoot, publicDigest]
        let queries = try (0..<Int(starkQueryCount)).map { queryIndex in
            try synthesizeQueryChain(
                queryIndex: queryIndex,
                params: params,
                transcriptLabel: transcriptLabel,
                roots: queryRoots,
                levelHashes: levels
            )
        }
        let airOpenings = try (0..<Int(starkQueryCount)).map { queryIndex in
            let index = deriveQueryIndex(
                transcriptLabel: transcriptLabel,
                params: params,
                roots: queryRoots,
                queryIndex: queryIndex
            )
            let nextIndex = (index + 1) % domain
            return try ToriiOfflineStarkAirOpeningV1(
                index: UInt32(index),
                row: airRows[index],
                nextRow: airRows[nextIndex],
                rowPath: merklePathFromLevels(index: index, levels: traceLevels),
                nextRowPath: merklePathFromLevels(index: nextIndex, levels: traceLevels),
                compositionValue: 0,
                compositionPath: zeroMerklePath(index: index, depth: Int(starkDomainLog2), levelHashes: levels)
            )
        }
        let compositionLeaf = offlineStarkBindingCompositionLeaf(
            domainTag: domainTag,
            transcriptLabel: transcriptLabel
        )
        let compositionLevels = merkleLevelsFromValues([compositionLeaf])
        let compositionValue = try ToriiOfflineStarkCompositionValueV1(
            leaf: compositionLeaf,
            constant: starkBindingConstant,
            zCoeff: starkBindingZCoefficient,
            auxTerms: offlineStarkBindingTerms(domainTag: domainTag, transcriptLabel: transcriptLabel),
            path: merklePathFromLevels(index: 0, levels: compositionLevels)
        )
        return ToriiOfflineStarkVerifyEnvelopeV1(
            params: params,
            proof: ToriiOfflineStarkProofV1(
                version: starkVersion,
                commits: try ToriiOfflineStarkCommitmentsV1(
                    version: starkVersion,
                    roots: roots,
                    compRoot: includeComposition ? prefixedHex(merkleRootFromLevels(compositionLevels)) : nil
                ),
                queries: queries,
                compValues: includeComposition ? Array(repeating: compositionValue, count: queries.count) : nil,
                air: try ToriiOfflineStarkAirProofV1(
                    version: starkVersion,
                    circuitId: airCircuitId,
                    publicDigest: prefixedHex(publicDigest),
                    traceRoot: prefixedHex(traceRoot),
                    compositionRoot: prefixedHex(compositionRoot),
                    traceWidth: 6,
                    openings: airOpenings
                )
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
    ) throws -> [ToriiOfflineFoldDecommitV1] {
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
            let pathY0 = try zeroMerklePath(index: y0Index, depth: depthCurrent, levelHashes: levelHashes)
            let pathY1 = try zeroMerklePath(index: y1Index, depth: depthCurrent, levelHashes: levelHashes)
            let pathZ = try zeroMerklePath(index: j, depth: depthNext, levelHashes: levelHashes)
            chain.append(
                ToriiOfflineFoldDecommitV1(
                    j: UInt32(j),
                    y0: 0,
                    y1: 0,
                    pathY0: pathY0,
                    pathY1: pathY1,
                    z: 0,
                    pathZ: pathZ
                )
            )
            indexAtLayer = j
        }
        return chain
    }

    private static func validateTransparentEnvelope(
        _ envelope: ToriiOfflineStarkVerifyEnvelopeV1,
        expectedDomainTag: String,
        transcriptLabel: String,
        expectedAirCircuitId: String? = nil,
        expectedAirPublicDigest: String? = nil,
        includeComposition: Bool = true,
        context: String
    ) throws {
        guard envelope.transcriptLabel == transcriptLabel else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline \(context) transcript label is invalid.")
        }
        guard envelope.params.version == starkVersion,
              envelope.params.nLog2 == starkDomainLog2,
              envelope.params.blowupLog2 == starkBlowupLog2,
              envelope.params.foldArity == 2,
              envelope.params.queries == starkQueryCount,
              envelope.params.merkleArity == 2,
              envelope.params.hashFn == starkHashSHA256V1,
              envelope.params.domainTag == expectedDomainTag
        else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline \(context) STARK parameters are invalid.")
        }
        guard envelope.proof.version == starkVersion else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline \(context) version is invalid.")
        }

        let levels = zeroMerkleLevelHashes(maxDepth: Int(starkDomainLog2))
        let requiredLayers = Int(starkDomainLog2)
        let expectedRoots = (0...requiredLayers).map { layer in
            prefixedHex(levels[Int(starkDomainLog2) - layer])
        }
        let expectedCompositionRoot = prefixedHex(fieldLeafHash(offlineStarkBindingCompositionLeaf(
            domainTag: expectedDomainTag,
            transcriptLabel: transcriptLabel
        )))
        guard envelope.proof.commits.version == starkVersion,
              envelope.proof.commits.roots == expectedRoots,
              envelope.proof.commits.compRoot == (includeComposition ? expectedCompositionRoot : nil)
        else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline \(context) commitments are invalid.")
        }
        let expectedDigest = expectedAirPublicDigest ?? prefixedHex(starkAirPublicDigestFromComposition(
            domainTag: expectedDomainTag,
            transcriptLabel: transcriptLabel
        ))
        guard let air = envelope.proof.air,
              air.version == starkVersion,
              air.circuitId == (expectedAirCircuitId ?? "composition-v1"),
              air.publicDigest == expectedDigest,
              air.compositionRoot == prefixedHex(levels[Int(starkDomainLog2)]),
              air.traceWidth == 6,
              air.openings.count == Int(starkQueryCount) else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline \(context) AIR binding is invalid.")
        }
        guard let publicDigest = Data(hexString: expectedDigest) else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline \(context) AIR binding is invalid.")
        }
        let domain = 1 << Int(starkDomainLog2)
        let airRows = (0..<domain).map { index in
            starkAirRow(index: index, publicDigest: publicDigest)
        }
        let traceLevels = merkleLevelsFromHashes(airRows.map(starkAirTraceLeafHash))
        let traceRoot = merkleRootFromLevels(traceLevels)
        guard air.traceRoot == prefixedHex(traceRoot) else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline \(context) AIR trace root is invalid."
            )
        }
        if includeComposition {
            guard envelope.proof.compValues?.count == Int(starkQueryCount) else {
                throw ToriiOfflineSettlementProofError.invalidSettlement("Offline \(context) composition binding is invalid.")
            }
        } else if envelope.proof.compValues != nil {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline \(context) composition binding is invalid.")
        }
        guard envelope.proof.queries.count == Int(starkQueryCount) else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline \(context) query count is invalid.")
        }
        let queryRootsData = try expectedRoots.map { root -> Data in
            guard let data = Data(hexString: root) else {
                throw ToriiOfflineSettlementProofError.invalidSettlement(
                    "Offline \(context) commitments are invalid."
                )
            }
            return data
        } + [traceRoot, levels[Int(starkDomainLog2)], publicDigest]
        let expectedCompositionValue = try ToriiOfflineStarkCompositionValueV1(
            leaf: offlineStarkBindingCompositionLeaf(
                domainTag: expectedDomainTag,
                transcriptLabel: transcriptLabel
            ),
            constant: starkBindingConstant,
            zCoeff: starkBindingZCoefficient,
            auxTerms: offlineStarkBindingTerms(
                domainTag: expectedDomainTag,
                transcriptLabel: transcriptLabel
            ),
            path: merklePathFromLevels(
                index: 0,
                levels: merkleLevelsFromValues([
                    offlineStarkBindingCompositionLeaf(
                        domainTag: expectedDomainTag,
                        transcriptLabel: transcriptLabel
                    )
                ])
            )
        )
        for (queryIndex, chain) in envelope.proof.queries.enumerated() {
            let baseIndex = deriveQueryIndex(
                transcriptLabel: transcriptLabel,
                params: envelope.params,
                roots: queryRootsData,
                queryIndex: queryIndex
            )
            try validateStarkAirOpening(
                air.openings[queryIndex],
                baseIndex: baseIndex,
                airRows: airRows,
                traceLevels: traceLevels,
                compositionLevels: levels,
                context: context
            )
            try validateTransparentQueryChain(
                chain,
                baseIndex: baseIndex,
                levels: levels,
                context: context
            )
            if includeComposition, envelope.proof.compValues?[queryIndex] != expectedCompositionValue {
                throw ToriiOfflineSettlementProofError.invalidSettlement(
                    "Offline \(context) composition binding is invalid."
                )
            }
        }
    }

    private static func validateStarkAirOpening(
        _ opening: ToriiOfflineStarkAirOpeningV1,
        baseIndex: Int,
        airRows: [[UInt64]],
        traceLevels: [[Data]],
        compositionLevels: [Data],
        context: String
    ) throws {
        let nextIndex = (baseIndex + 1) % airRows.count
        guard opening.index == UInt32(baseIndex),
              opening.row == airRows[baseIndex],
              opening.nextRow == airRows[nextIndex],
              opening.rowPath == (try merklePathFromLevels(index: baseIndex, levels: traceLevels)),
              opening.nextRowPath == (try merklePathFromLevels(index: nextIndex, levels: traceLevels)),
              opening.compositionValue == 0,
              opening.compositionPath == (try zeroMerklePath(
                  index: baseIndex,
                  depth: Int(starkDomainLog2),
                  levelHashes: compositionLevels
              )) else {
            throw ToriiOfflineSettlementProofError.invalidSettlement(
                "Offline \(context) AIR opening is invalid."
            )
        }
    }

    private static func validateTransparentQueryChain(
        _ chain: [ToriiOfflineFoldDecommitV1],
        baseIndex: Int,
        levels: [Data],
        context: String
    ) throws {
        let requiredLayers = Int(starkDomainLog2)
        guard chain.count == requiredLayers else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline \(context) query depth is invalid.")
        }
        var indexAtLayer = baseIndex
        for (layer, decommit) in chain.enumerated() {
            let depthCurrent = Int(starkDomainLog2) - layer
            let depthNext = depthCurrent - 1
            let maxJ = 1 << (depthCurrent - 1)
            let expectedJ = indexAtLayer / 2
            guard Int(decommit.j) < maxJ, decommit.j == UInt32(expectedJ) else {
                throw ToriiOfflineSettlementProofError.invalidSettlement("Offline \(context) query index is invalid.")
            }
            guard decommit.y0 == 0, decommit.y1 == 0, decommit.z == 0 else {
                throw ToriiOfflineSettlementProofError.invalidSettlement("Offline \(context) fold values are invalid.")
            }
            let y0Index = expectedJ * 2
            let y1Index = y0Index + 1
            guard decommit.pathY0 == (try zeroMerklePath(index: y0Index, depth: depthCurrent, levelHashes: levels)),
                  decommit.pathY1 == (try zeroMerklePath(index: y1Index, depth: depthCurrent, levelHashes: levels)),
                  decommit.pathZ == (try zeroMerklePath(index: expectedJ, depth: depthNext, levelHashes: levels))
            else {
                throw ToriiOfflineSettlementProofError.invalidSettlement("Offline \(context) Merkle paths are invalid.")
            }
            indexAtLayer = expectedJ
        }
        guard indexAtLayer == 0 else {
            throw ToriiOfflineSettlementProofError.invalidSettlement("Offline \(context) final fold is invalid.")
        }
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

    private static func merkleLevelsFromValues(_ values: [UInt64]) -> [[Data]] {
        merkleLevelsFromHashes(values.map { fieldLeafHash($0) })
    }

    private static func merkleLevelsFromHashes(_ leaves: [Data]) -> [[Data]] {
        precondition(!leaves.isEmpty)
        var current = leaves
        var levels: [[Data]] = []
        while true {
            levels.append(current)
            if current.count == 1 {
                break
            }
            if current.count % 2 == 1 {
                current.append(current[current.count - 1])
            }
            var next: [Data] = []
            next.reserveCapacity(current.count / 2)
            for index in stride(from: 0, to: current.count, by: 2) {
                next.append(nodeHash(current[index], current[index + 1]))
            }
            current = next
        }
        return levels
    }

    private static func merkleRootFromLevels(_ levels: [[Data]]) -> Data {
        levels.last?.first ?? Data()
    }

    private static func merklePathFromLevels(index: Int, levels: [[Data]]) throws -> ToriiOfflineMerklePath {
        precondition(!levels.isEmpty)
        precondition(index >= 0 && index < levels[0].count)
        let depth = levels.count - 1
        var dirs = Data(repeating: 0, count: (depth + 7) / 8)
        var siblings: [String] = []
        siblings.reserveCapacity(depth)
        var currentIndex = index
        if depth > 0 {
            for levelIndex in 0..<depth {
                let level = levels[levelIndex]
                let siblingIndex: Int
                if currentIndex % 2 == 0 {
                    siblingIndex = min(currentIndex + 1, level.count - 1)
                } else {
                    dirs[levelIndex / 8] |= UInt8(1 << (levelIndex % 8))
                    siblingIndex = currentIndex - 1
                }
                siblings.append(prefixedHex(level[siblingIndex]))
                currentIndex /= 2
            }
        }
        return try ToriiOfflineMerklePath(dirs: dirs.base64EncodedString(), siblings: siblings)
    }

    private static func zeroMerklePath(
        index: Int,
        depth: Int,
        levelHashes: [Data]
    ) throws -> ToriiOfflineMerklePath {
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
        return try ToriiOfflineMerklePath(
            dirs: dirs.base64EncodedString(),
            siblings: siblings
        )
    }

    private static func normalizeProof(
        _ proof: ToriiOfflineTransparentZkProof
    ) throws -> ToriiOfflineTransparentZkProof {
        try ToriiOfflineTransparentZkProof(
            backend: proof.backend,
            circuitId: proof.circuitId,
            recursionDepth: proof.recursionDepth,
            publicInputsHex: proof.publicInputsHex,
            envelope: try normalizeEnvelope(proof.envelope)
        )
    }

    private static func normalizeEnvelope(
        _ envelope: ToriiOfflineStarkVerifyEnvelopeV1
    ) throws -> ToriiOfflineStarkVerifyEnvelopeV1 {
        ToriiOfflineStarkVerifyEnvelopeV1(
            params: try ToriiOfflineStarkFriParamsV1(
                version: envelope.params.version,
                nLog2: envelope.params.nLog2,
                blowupLog2: envelope.params.blowupLog2,
                foldArity: envelope.params.foldArity,
                queries: envelope.params.queries,
                merkleArity: envelope.params.merkleArity,
                hashFn: envelope.params.hashFn,
                domainTag: envelope.params.domainTag
            ),
            proof: ToriiOfflineStarkProofV1(
                version: envelope.proof.version,
                commits: try ToriiOfflineStarkCommitmentsV1(
                    version: envelope.proof.commits.version,
                    roots: envelope.proof.commits.roots,
                    compRoot: envelope.proof.commits.compRoot
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
                },
                compValues: envelope.proof.compValues,
                air: envelope.proof.air
            ),
            transcriptLabel: envelope.transcriptLabel
        )
    }

    private static func normalizeMerklePath(
        _ path: ToriiOfflineMerklePath
    ) throws -> ToriiOfflineMerklePath {
        try ToriiOfflineMerklePath(
            dirs: path.dirs,
            siblings: path.siblings
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

    private static func starkAirPublicDigestFromComposition(
        domainTag: String,
        transcriptLabel: String
    ) -> Data {
        var digest = SHA256()
        digest.update(data: Data("iroha:zk:stark:air-public-digest:v1".utf8))
        digest.update(data: littleEndianData(starkBindingConstant))
        digest.update(data: littleEndianData(starkBindingZCoefficient))
        let terms = offlineStarkBindingTerms(domainTag: domainTag, transcriptLabel: transcriptLabel)
        digest.update(data: littleEndianData(UInt64(terms.count)))
        for term in terms {
            digest.update(data: littleEndianData(term.wireIndex))
            digest.update(data: littleEndianData(term.value))
            digest.update(data: littleEndianData(term.coeff))
        }
        return Data(digest.finalize())
    }

    private static func offlineStarkBindingTerms(
        domainTag: String,
        transcriptLabel: String
    ) -> [ToriiOfflineStarkCompositionTermV1] {
        var preimage = Data("iroha:offline:stark-binding-air:v1".utf8)
        preimage.append(littleEndianData(UInt64(domainTag.utf8.count)))
        preimage.append(Data(domainTag.utf8))
        preimage.append(littleEndianData(UInt64(transcriptLabel.utf8.count)))
        preimage.append(Data(transcriptLabel.utf8))

        let digest = Data(SHA256.hash(data: preimage))
        return (0..<4).map { index in
            let offset = index * 8
            var word: UInt64 = 0
            for byteOffset in 0..<8 {
                word |= UInt64(digest[offset + byteOffset]) << (byteOffset * 8)
            }
            let value = word >= goldilocksModulus ? word &- goldilocksModulus : word
            return ToriiOfflineStarkCompositionTermV1(
                wireIndex: UInt32(index),
                value: value,
                coeff: UInt64(index + 31)
            )
        }
    }

    private static func offlineStarkBindingCompositionLeaf(
        domainTag: String,
        transcriptLabel: String
    ) -> UInt64 {
        var expected = starkBindingConstant
        for term in offlineStarkBindingTerms(domainTag: domainTag, transcriptLabel: transcriptLabel) {
            expected = addGoldilocks(expected, multiplyGoldilocks(term.coeff, term.value))
        }
        expected = addGoldilocks(expected, multiplyGoldilocks(starkBindingZCoefficient, 0))
        return expected
    }

    private static func starkAirDigestLimbs(_ publicDigest: Data) -> [UInt64] {
        precondition(publicDigest.count == 32)
        return (0..<4).map { index in
            let offset = index * 8
            var word: UInt64 = 0
            for byteOffset in 0..<8 {
                word |= UInt64(publicDigest[offset + byteOffset]) << (byteOffset * 8)
            }
            return word >= goldilocksModulus ? word &- goldilocksModulus : word
        }
    }

    private static func starkAirRow(index: Int, publicDigest: Data) -> [UInt64] {
        let limbs = starkAirDigestLimbs(publicDigest)
        return [
            UInt64(index) % goldilocksModulus,
            limbs[0],
            limbs[1],
            limbs[2],
            limbs[3],
            6
        ]
    }

    private static func starkAirTraceLeafHash(_ row: [UInt64]) -> Data {
        var digest = SHA256()
        digest.update(data: Data("STARK:AIR:TRACE:ROW:V1".utf8))
        digest.update(data: littleEndianData(UInt64(row.count)))
        for value in row {
            digest.update(data: littleEndianData(value))
        }
        return Data(digest.finalize())
    }

    private static func fieldLeafHash(_ value: UInt64) -> Data {
        var digest = SHA256()
        digest.update(data: Data("LEAF".utf8))
        digest.update(data: littleEndianData(value))
        return Data(digest.finalize())
    }

    private static func addGoldilocks(_ lhs: UInt64, _ rhs: UInt64) -> UInt64 {
        let (sum, overflow) = lhs.addingReportingOverflow(rhs)
        return mod128(high: overflow ? 1 : 0, low: sum, modulus: goldilocksModulus)
    }

    private static func multiplyGoldilocks(_ lhs: UInt64, _ rhs: UInt64) -> UInt64 {
        let fullWidth = lhs.multipliedFullWidth(by: rhs)
        return mod128(high: fullWidth.high, low: fullWidth.low, modulus: goldilocksModulus)
    }

    private static func mod128(high: UInt64, low: UInt64, modulus: UInt64) -> UInt64 {
        precondition(modulus > 0)
        var remainder: UInt64 = 0
        for bit in stride(from: 63, through: 0, by: -1) {
            remainder = stepMod(
                remainder: remainder,
                bit: (high >> UInt64(bit)) & 1,
                modulus: modulus
            )
        }
        for bit in stride(from: 63, through: 0, by: -1) {
            remainder = stepMod(
                remainder: remainder,
                bit: (low >> UInt64(bit)) & 1,
                modulus: modulus
            )
        }
        return remainder
    }

    private static func stepMod(remainder: UInt64, bit: UInt64, modulus: UInt64) -> UInt64 {
        let doubled: UInt64
        if remainder >= modulus &- remainder {
            doubled = remainder &- (modulus &- remainder)
        } else {
            doubled = remainder &+ remainder
        }
        if bit == 0 {
            return doubled
        }
        if doubled == modulus &- 1 {
            return 0
        }
        return doubled &+ 1
    }

    private static func canonicalJSONData<T: Encodable>(_ value: T) throws -> Data {
        try ToriiOfflineCashCodec.canonicalData(value)
    }

    private static func sha256Hex(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }

    private static func prefixedHex(_ data: Data) -> String {
        data.hexEncodedString()
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
        try OfflineSettlementFieldCanonicalizer.plainHex(value, label: label)
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

    private struct SourceLineageWitnessPayload: Codable {
        let version: Int
        let anchor: ToriiOfflineCashState
        let ancestryReceipts: [SourceLineageWitnessReceipt]
        let receipt: SourceLineageWitnessReceipt

        private enum CodingKeys: String, CodingKey {
            case version
            case anchor
            case ancestryReceipts = "ancestry_receipts"
            case receipt
        }
    }

    private struct SourceLineageWitnessReceipt: Codable {
        let version: Int
        let transferId: String
        let direction: String
        let lineageId: String
        let accountId: String
        let deviceId: String
        let offlinePublicKey: String
        let preBalance: String
        let postBalance: String
        let preLockedBalance: String
        let postLockedBalance: String
        let preStateHash: String
        let postStateHash: String
        let localRevision: UInt64
        let counterpartyLineageId: String
        let counterpartyAccountId: String
        let counterpartyDeviceId: String
        let counterpartyOfflinePublicKey: String
        let amount: String
        let authorization: SourceLineageWitnessAuthorization?
        let attestation: SourceLineageWitnessAttestation
        let sourceLineageProof: ToriiOfflineSourceLineageEnvelope?
        let sourcePayload: String?
        let senderSignatureBase64: String
        let createdAtMs: UInt64

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            version = try container.decode(Int.self, forKey: .version)
            transferId = try container.decode(String.self, forKey: .transferId)
            direction = try container.decode(String.self, forKey: .direction)
            lineageId = try container.decode(String.self, forKey: .lineageId)
            accountId = try container.decode(String.self, forKey: .accountId)
            deviceId = try container.decode(String.self, forKey: .deviceId)
            offlinePublicKey = try container.decode(String.self, forKey: .offlinePublicKey)
            preBalance = try container.decode(String.self, forKey: .preBalance)
            postBalance = try container.decode(String.self, forKey: .postBalance)
            preLockedBalance = try container.decode(String.self, forKey: .preLockedBalance)
            postLockedBalance = try container.decode(String.self, forKey: .postLockedBalance)
            preStateHash = try container.decode(String.self, forKey: .preStateHash)
            postStateHash = try container.decode(String.self, forKey: .postStateHash)
            localRevision = try container.decode(UInt64.self, forKey: .localRevision)
            counterpartyLineageId = try container.decode(String.self, forKey: .counterpartyLineageId)
            counterpartyAccountId = try container.decode(String.self, forKey: .counterpartyAccountId)
            counterpartyDeviceId = try container.decode(String.self, forKey: .counterpartyDeviceId)
            counterpartyOfflinePublicKey = try container.decode(
                String.self,
                forKey: .counterpartyOfflinePublicKey
            )
            amount = try container.decode(String.self, forKey: .amount)
            authorization = try container.decodeIfPresent(
                SourceLineageWitnessAuthorization.self,
                forKey: .authorization
            )
            attestation = try container.decodeIfPresent(
                SourceLineageWitnessAttestation.self,
                forKey: .attestation
            ) ?? container.decode(SourceLineageWitnessAttestation.self, forKey: .deviceProof)
            sourceLineageProof = try container.decodeIfPresent(
                ToriiOfflineSourceLineageEnvelope.self,
                forKey: .sourceLineageProof
            )
            sourcePayload = try container.decodeIfPresent(String.self, forKey: .sourcePayload)
            senderSignatureBase64 = try container.decode(String.self, forKey: .senderSignatureBase64)
            createdAtMs = try container.decode(UInt64.self, forKey: .createdAtMs)
        }

        func encode(to encoder: Encoder) throws {
            var container = encoder.container(keyedBy: CodingKeys.self)
            try container.encode(version, forKey: .version)
            try container.encode(transferId, forKey: .transferId)
            try container.encode(direction, forKey: .direction)
            try container.encode(lineageId, forKey: .lineageId)
            try container.encode(accountId, forKey: .accountId)
            try container.encode(deviceId, forKey: .deviceId)
            try container.encode(offlinePublicKey, forKey: .offlinePublicKey)
            try container.encode(preBalance, forKey: .preBalance)
            try container.encode(postBalance, forKey: .postBalance)
            try container.encode(preLockedBalance, forKey: .preLockedBalance)
            try container.encode(postLockedBalance, forKey: .postLockedBalance)
            try container.encode(preStateHash, forKey: .preStateHash)
            try container.encode(postStateHash, forKey: .postStateHash)
            try container.encode(localRevision, forKey: .localRevision)
            try container.encode(counterpartyLineageId, forKey: .counterpartyLineageId)
            try container.encode(counterpartyAccountId, forKey: .counterpartyAccountId)
            try container.encode(counterpartyDeviceId, forKey: .counterpartyDeviceId)
            try container.encode(counterpartyOfflinePublicKey, forKey: .counterpartyOfflinePublicKey)
            try container.encode(amount, forKey: .amount)
            try container.encodeIfPresent(authorization, forKey: .authorization)
            try container.encode(attestation, forKey: .attestation)
            try container.encodeIfPresent(sourceLineageProof, forKey: .sourceLineageProof)
            try container.encodeIfPresent(sourcePayload, forKey: .sourcePayload)
            try container.encode(senderSignatureBase64, forKey: .senderSignatureBase64)
            try container.encode(createdAtMs, forKey: .createdAtMs)
        }

        func cashReceipt() throws -> ToriiOfflineTransferReceipt {
            guard let receiptDirection = ToriiOfflineTransferDirection(rawValue: direction) else {
                throw ToriiOfflineSettlementProofError.invalidSettlement(
                    "Source lineage proof does not match the receipt."
                )
            }
            guard let authorization else {
                throw ToriiOfflineSettlementProofError.invalidSettlement(
                    "Source lineage proof witness signature is invalid."
                )
            }
            let cashAuthorization = try authorization.cashAuthorization()
            return try ToriiOfflineTransferReceipt(
                version: version,
                transferId: transferId,
                direction: receiptDirection,
                lineageId: lineageId,
                accountId: accountId,
                deviceId: deviceId,
                offlinePublicKey: offlinePublicKey,
                preBalance: preBalance,
                postBalance: postBalance,
                preLockedBalance: preLockedBalance,
                postLockedBalance: postLockedBalance,
                preStateHash: preStateHash,
                postStateHash: postStateHash,
                localRevision: localRevision,
                counterpartyLineageId: counterpartyLineageId,
                counterpartyAccountId: counterpartyAccountId,
                counterpartyDeviceId: counterpartyDeviceId,
                counterpartyOfflinePublicKey: counterpartyOfflinePublicKey,
                amount: amount,
                authorization: cashAuthorization,
                deviceProof: ToriiOfflineDeviceProof(
                    platform: cashAuthorization.deviceBinding.platform,
                    attestationKeyId: attestation.keyId,
                    challengeHashHex: attestation.challengeHashHex,
                    assertionBase64: attestation.assertionBase64,
                    counter: attestation.counter
                ),
                sourceLineageProof: sourceLineageProof,
                sourcePayload: sourcePayload,
                senderSignatureBase64: senderSignatureBase64,
                createdAtMs: createdAtMs
            )
        }

        private enum CodingKeys: String, CodingKey {
            case version
            case transferId = "transfer_id"
            case direction
            case lineageId = "lineage_id"
            case accountId = "account_id"
            case deviceId = "device_id"
            case offlinePublicKey = "offline_public_key"
            case preBalance = "pre_balance"
            case postBalance = "post_balance"
            case preLockedBalance = "pre_locked_balance"
            case postLockedBalance = "post_locked_balance"
            case preStateHash = "pre_state_hash"
            case postStateHash = "post_state_hash"
            case localRevision = "local_revision"
            case counterpartyLineageId = "counterparty_lineage_id"
            case counterpartyAccountId = "counterparty_account_id"
            case counterpartyDeviceId = "counterparty_device_id"
            case counterpartyOfflinePublicKey = "counterparty_offline_public_key"
            case amount
            case authorization
            case attestation
            case deviceProof = "device_proof"
            case sourceLineageProof = "source_lineage_proof"
            case sourcePayload = "source_payload"
            case senderSignatureBase64 = "sender_signature_base64"
            case createdAtMs = "created_at_ms"
        }
    }

    private struct SourceLineageWitnessAuthorization: Codable {
        let authorizationId: String
        let lineageId: String
        let accountId: String
        let deviceId: String
        let offlinePublicKey: String
        let verdictId: String
        let maxBalance: String
        let maxTxValue: String
        let issuedAtMs: UInt64
        let refreshAtMs: UInt64
        let expiresAtMs: UInt64
        let deviceBinding: ToriiOfflineDeviceBinding?
        let appAttestKeyId: String
        let issuerSignatureBase64: String

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            authorizationId = try container.decode(String.self, forKey: .authorizationId)
            lineageId = try container.decode(String.self, forKey: .lineageId)
            accountId = try container.decode(String.self, forKey: .accountId)
            verdictId = try container.decode(String.self, forKey: .verdictId)
            maxBalance = try container.decode(String.self, forKey: .maxBalance)
            maxTxValue = try container.decode(String.self, forKey: .maxTxValue)
            issuedAtMs = try container.decode(UInt64.self, forKey: .issuedAtMs)
            refreshAtMs = try container.decode(UInt64.self, forKey: .refreshAtMs)
            expiresAtMs = try container.decode(UInt64.self, forKey: .expiresAtMs)
            deviceBinding = try container.decodeIfPresent(
                ToriiOfflineDeviceBinding.self,
                forKey: .deviceBinding
            )
            guard let resolvedDeviceId = try container.decodeIfPresent(String.self, forKey: .deviceId)
                    ?? deviceBinding?.deviceId,
                  let resolvedOfflinePublicKey = try container.decodeIfPresent(
                    String.self,
                    forKey: .offlinePublicKey
                  ) ?? deviceBinding?.offlinePublicKey,
                  let resolvedAppAttestKeyId = try container.decodeIfPresent(
                    String.self,
                    forKey: .appAttestKeyId
                  ) ?? deviceBinding?.attestationKeyId else {
                throw ToriiOfflineSettlementProofError.invalidSettlement(
                    "Source lineage proof witness payload is invalid."
                )
            }
            deviceId = resolvedDeviceId
            offlinePublicKey = resolvedOfflinePublicKey
            appAttestKeyId = resolvedAppAttestKeyId
            issuerSignatureBase64 = try container.decode(String.self, forKey: .issuerSignatureBase64)
        }

        func cashAuthorization() throws -> ToriiOfflineSpendAuthorization {
            guard let deviceBinding else {
                throw ToriiOfflineSettlementProofError.invalidSettlement(
                    "Source lineage proof witness signature is invalid."
                )
            }
            return try ToriiOfflineSpendAuthorization(
                authorizationId: authorizationId,
                lineageId: lineageId,
                accountId: accountId,
                verdictId: verdictId,
                policyMaxBalance: maxBalance,
                policyMaxTxValue: maxTxValue,
                issuedAtMs: issuedAtMs,
                refreshAtMs: refreshAtMs,
                expiresAtMs: expiresAtMs,
                deviceBinding: deviceBinding,
                issuerSignatureBase64: issuerSignatureBase64
            )
        }

        private enum CodingKeys: String, CodingKey {
            case authorizationId = "authorization_id"
            case lineageId = "lineage_id"
            case accountId = "account_id"
            case deviceId = "device_id"
            case offlinePublicKey = "offline_public_key"
            case verdictId = "verdict_id"
            case maxBalance = "max_balance"
            case maxTxValue = "max_tx_value"
            case issuedAtMs = "issued_at_ms"
            case refreshAtMs = "refresh_at_ms"
            case expiresAtMs = "expires_at_ms"
            case deviceBinding = "device_binding"
            case appAttestKeyId = "app_attest_key_id"
            case issuerSignatureBase64 = "issuer_signature_base64"
        }
    }

    private struct SourceLineageWitnessAttestation: Codable {
        let keyId: String
        let counter: UInt64
        let assertionBase64: String
        let challengeHashHex: String
        let attestationReportBase64: String?
        let iosTeamId: String?
        let iosBundleId: String?
        let iosEnvironment: String?

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            keyId = try container.decodeIfPresent(String.self, forKey: .keyId)
                ?? container.decode(String.self, forKey: .attestationKeyId)
            counter = try container.decodeIfPresent(UInt64.self, forKey: .counter) ?? 0
            assertionBase64 = try container.decode(String.self, forKey: .assertionBase64)
            challengeHashHex = try container.decode(String.self, forKey: .challengeHashHex)
            attestationReportBase64 = try container.decodeIfPresent(
                String.self,
                forKey: .attestationReportBase64
            )
            iosTeamId = try container.decodeIfPresent(String.self, forKey: .iosTeamId)
            iosBundleId = try container.decodeIfPresent(String.self, forKey: .iosBundleId)
            iosEnvironment = try container.decodeIfPresent(String.self, forKey: .iosEnvironment)
        }

        func encode(to encoder: Encoder) throws {
            var container = encoder.container(keyedBy: CodingKeys.self)
            try container.encode(keyId, forKey: .keyId)
            try container.encode(counter, forKey: .counter)
            try container.encode(assertionBase64, forKey: .assertionBase64)
            try container.encode(challengeHashHex, forKey: .challengeHashHex)
            try container.encodeIfPresent(attestationReportBase64, forKey: .attestationReportBase64)
            try container.encodeIfPresent(iosTeamId, forKey: .iosTeamId)
            try container.encodeIfPresent(iosBundleId, forKey: .iosBundleId)
            try container.encodeIfPresent(iosEnvironment, forKey: .iosEnvironment)
        }

        private enum CodingKeys: String, CodingKey {
            case keyId = "key_id"
            case attestationKeyId = "attestation_key_id"
            case counter
            case assertionBase64 = "assertion_base64"
            case challengeHashHex = "challenge_hash_hex"
            case attestationReportBase64 = "attestation_report_base64"
            case iosTeamId = "ios_team_id"
            case iosBundleId = "ios_bundle_id"
            case iosEnvironment = "ios_environment"
        }
    }

    private struct SourceLineageAttestationSendPayload: Encodable {
        let lineageId: String
        let transferId: String
        let amount: String
        let receiverLineageId: String

        private enum CodingKeys: String, CodingKey {
            case lineageId = "lineage_id"
            case transferId = "transfer_id"
            case amount
            case receiverLineageId = "receiver_lineage_id"
        }
    }

    private struct SourceLineageAttestationReceivePayload: Encodable {
        let lineageId: String
        let transferId: String
        let amount: String
        let senderLineageId: String

        private enum CodingKeys: String, CodingKey {
            case lineageId = "lineage_id"
            case transferId = "transfer_id"
            case amount
            case senderLineageId = "sender_lineage_id"
        }
    }

    private struct SourceLineageAttestationChallengePayload: Encodable {
        let accountId: String
        let lineageId: String
        let operation: String
        let payloadHash: String

        private enum CodingKeys: String, CodingKey {
            case accountId = "account_id"
            case lineageId = "lineage_id"
            case operation
            case payloadHash = "payload_hash"
        }
    }

    private struct SourceLineageProofCommitmentPayload: Encodable {
        let circuitId: String
        let publicInputs: ToriiOfflineSourceLineagePublicInputs
        let witnessPayloadHash: String

        private enum CodingKeys: String, CodingKey {
            case circuitId = "circuit_id"
            case publicInputs = "public_inputs"
            case witnessPayloadHash = "witness_payload_hash"
        }
    }

    private struct SourceLineageFastpqProofRequest: Encodable {
        let transferId: String
        let sourceReceiptHash: String
        let sourceNullifier: String
        let publicInputsCommitmentHex: String
        let witnessPayload: String

        private enum CodingKeys: String, CodingKey {
            case transferId = "transfer_id"
            case sourceReceiptHash = "source_receipt_hash"
            case sourceNullifier = "source_nullifier"
            case publicInputsCommitmentHex = "public_inputs_commitment_hex"
            case witnessPayload = "witness_payload"
        }
    }

    private struct SourceLineageNullifierPayload: Encodable {
        let circuitId: String
        let transferId: String
        let sourceReceiptHash: String
        let senderLineageId: String
        let recipientLineageId: String
        let assetDefinitionId: String
        let amount: String
        let sourceLocalRevision: UInt64

        private enum CodingKeys: String, CodingKey {
            case circuitId = "circuit_id"
            case transferId = "transfer_id"
            case sourceReceiptHash = "source_receipt_hash"
            case senderLineageId = "sender_lineage_id"
            case recipientLineageId = "recipient_lineage_id"
            case assetDefinitionId = "asset_definition_id"
            case amount
            case sourceLocalRevision = "source_local_revision"
        }
    }
}
