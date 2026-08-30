import Foundation

/// Application-persisted Sumeragi-v2 checkpoint authenticated by ABI-22 native code.
public struct AuthenticatedFinalityCheckpointV1: Equatable, Sendable {
    public static let contextIdBytes = 32
    public static let projectionBytes = 8 + contextIdBytes

    public let height: Int64
    public let heightContextId: Data

    public init(height: Int64, heightContextId: Data) throws {
        guard height > 0,
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashBytes(heightContextId) else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        self.height = height
        self.heightContextId = Data(heightContextId)
    }

    /// Exact ABI-22 persistence form: positive u63 big-endian followed by a marked context id.
    public init(projectionBytes: Data) throws {
        guard projectionBytes.count == Self.projectionBytes else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        var rawHeight: UInt64 = 0
        for byte in projectionBytes.prefix(8) {
            rawHeight = (rawHeight << 8) | UInt64(byte)
        }
        guard rawHeight > 0, rawHeight <= UInt64(Int64.max) else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        try self.init(
            height: Int64(rawHeight),
            heightContextId: Data(projectionBytes.dropFirst(8))
        )
    }

    public var projection: Data {
        var value = UInt64(height).bigEndian
        var output = withUnsafeBytes(of: &value) { Data($0) }
        output.append(heightContextId)
        return output
    }
}

/// Native-canonical, content-addressed page of contiguous bridge finality proofs.
public struct AuthenticatedFinalityProofPageV1: Equatable, Sendable {
    public static let maximumProofCount = 64
    public static let maximumProofBytes = 9 * 1_024 * 1_024
    public static let maximumPageBytes = 64 * 1_024 * 1_024

    public let evidenceArchive: Data
    public let hashHex: String

    public init(evidenceArchive: Data, hashHex: String) throws {
        guard !evidenceArchive.isEmpty,
              evidenceArchive.count <= Self.maximumPageBytes,
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(hashHex),
              hashHex == AuthenticatedKagemushaFinalityValidationV1.markedHashHex(
                  evidenceArchive
              ) else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        self.evidenceArchive = Data(evidenceArchive)
        self.hashHex = hashHex
    }
}

/// Native-verified authority-split transaction-details projection.
///
/// Its result and height remain routing hints until finalized outcome projection succeeds.
public struct AuthenticatedCommittedTransactionResultV2: Decodable, Equatable, Sendable {
    public let transactionHashHex: String
    public let queryAuthority: String
    public let transactionAuthority: String
    public let blockHashHex: String
    public let resultHashHex: String
    public let resultOk: Bool
    public let rejectionMessage: String?
    public let committedBlockHeight: Int64
    public let transactionDetailsHashHex: String

    private enum CodingKeys: String, CodingKey {
        case version
        case transactionHashHex = "transaction_hash_hex"
        case queryAuthority = "query_authority"
        case transactionAuthority = "transaction_authority"
        case blockHashHex = "block_hash_hex"
        case resultHashHex = "result_hash_hex"
        case resultOk = "result_ok"
        case rejectionMessage = "rejection_message"
        case committedBlockHeight = "committed_block_height"
        case transactionDetailsHashHex = "transaction_details_hash_hex"
    }

    public init(from decoder: Decoder) throws {
        try AuthenticatedKagemushaFinalityValidationV1.requireExactFields(
            decoder,
            [
                "version", "transaction_hash_hex", "query_authority",
                "transaction_authority", "block_hash_hex", "result_hash_hex", "result_ok",
                "rejection_message", "committed_block_height", "transaction_details_hash_hex",
            ]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard try container.decode(UInt32.self, forKey: .version) == 2 else {
            throw DecodingError.dataCorruptedError(
                forKey: .version,
                in: container,
                debugDescription: "authenticated transaction-details result version must be 2"
            )
        }
        let transactionHashHex = try container.decode(String.self, forKey: .transactionHashHex)
        let queryAuthority = try container.decode(String.self, forKey: .queryAuthority)
        let transactionAuthority = try container.decode(String.self, forKey: .transactionAuthority)
        let blockHashHex = try container.decode(String.self, forKey: .blockHashHex)
        let resultHashHex = try container.decode(String.self, forKey: .resultHashHex)
        let resultOk = try container.decode(Bool.self, forKey: .resultOk)
        guard container.contains(.rejectionMessage) else {
            throw DecodingError.keyNotFound(
                CodingKeys.rejectionMessage,
                .init(codingPath: container.codingPath, debugDescription: "rejection_message is required")
            )
        }
        let rejectionMessage = try container.decodeIfPresent(String.self, forKey: .rejectionMessage)
        let heightText = try container.decode(String.self, forKey: .committedBlockHeight)
        let transactionDetailsHashHex = try container.decode(
            String.self,
            forKey: .transactionDetailsHashHex
        )
        guard AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(transactionHashHex),
              AuthenticatedKagemushaFinalityValidationV1.isCanonicalText(
                  queryAuthority,
                  maximumUTF8Bytes: 16 * 1_024
              ),
              AuthenticatedKagemushaFinalityValidationV1.isCanonicalText(
                  transactionAuthority,
                  maximumUTF8Bytes: 16 * 1_024
              ),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(blockHashHex),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(resultHashHex),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(
                  transactionDetailsHashHex
              ),
              let committedBlockHeight =
                  AuthenticatedKagemushaFinalityValidationV1.positiveInt64(heightText) else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: container.codingPath, debugDescription: "transaction-details result is not canonical")
            )
        }
        if resultOk {
            guard rejectionMessage == nil else {
                throw DecodingError.dataCorruptedError(
                    forKey: .rejectionMessage,
                    in: container,
                    debugDescription: "applied routing hint cannot carry a rejection message"
                )
            }
        } else {
            guard let rejectionMessage,
                  AuthenticatedKagemushaFinalityValidationV1.isCanonicalText(
                      rejectionMessage,
                      maximumUTF8Bytes: 1_024
                  ) else {
                throw DecodingError.dataCorruptedError(
                    forKey: .rejectionMessage,
                    in: container,
                    debugDescription: "rejected routing hint requires a bounded canonical message"
                )
            }
        }
        self.transactionHashHex = transactionHashHex
        self.queryAuthority = queryAuthority
        self.transactionAuthority = transactionAuthority
        self.blockHashHex = blockHashHex
        self.resultHashHex = resultHashHex
        self.resultOk = resultOk
        self.rejectionMessage = rejectionMessage
        self.committedBlockHeight = committedBlockHeight
        self.transactionDetailsHashHex = transactionDetailsHashHex
    }
}

struct PrivacyAuthenticatedTransactionDetailsPreparationV2: Sendable {
    let archive: Data
    let signingDigest: Data
}

/// Exact signed-query preparation and exact Torii response retained for finality verification.
public struct AuthenticatedTransactionDetailsCarrierV2: Equatable, Sendable {
    /// Opaque native query preparation; persist with the evidence bundle for crash recovery.
    public let nativePreparationArchive: Data
    /// Exact canonical `/v1/pipeline/transactions/details` response bytes.
    public let responseNorito: Data
    public let transactionHashHex: String
    public let queryAuthority: String
    public let transactionAuthority: String
    /// Untrusted routing hint. Never release value or terminal state from this field.
    public let committedBlockHeightHint: Int64
    /// Untrusted routing hint. Never release value or terminal state from this field.
    public let resultOkHint: Bool
    public let transactionDetailsHashHex: String

    init(
        preparation: PrivacyAuthenticatedTransactionDetailsPreparationV2,
        responseNorito: Data,
        projection: AuthenticatedCommittedTransactionResultV2
    ) throws {
        guard !preparation.archive.isEmpty,
              preparation.archive.count <= 64 * 1_024,
              !responseNorito.isEmpty,
              responseNorito.count <= 64 * 1_024 * 1_024,
              projection.transactionDetailsHashHex
                == AuthenticatedKagemushaFinalityValidationV1.markedHashHex(
                    responseNorito
                ) else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        nativePreparationArchive = Data(preparation.archive)
        self.responseNorito = Data(responseNorito)
        transactionHashHex = projection.transactionHashHex
        queryAuthority = projection.queryAuthority
        transactionAuthority = projection.transactionAuthority
        committedBlockHeightHint = projection.committedBlockHeight
        resultOkHint = projection.resultOk
        transactionDetailsHashHex = projection.transactionDetailsHashHex
    }
}

/// Exact Kagemusha issuer result independently authenticated by validator finality evidence.
public struct AuthenticatedFinalizedKagemushaOutcomeV1: Decodable, Equatable, Sendable {
    public enum TerminalState: String, Decodable, Sendable {
        case applied
        case rejected
    }

    public let terminalState: TerminalState
    public let operationId: Data
    public let operationKind: String
    public let transactionHashHex: String
    public let queryAuthority: String
    public let transactionAuthority: String
    public let blockHashHex: String
    public let resultHashHex: String
    public let committedBlockHeight: Int64
    public let finalizedCheckpoint: AuthenticatedFinalityCheckpointV1
    public let executedBlockWireHashHex: String
    public let rejectionCode: String?
    public let rejectionMessage: String?
    public let evidenceIdHex: String
    public let transactionDetailsHashHex: String
    public let finalityPageHashHex: String

    private enum CodingKeys: String, CodingKey {
        case version
        case terminalState = "terminal_state"
        case operationIdHex = "operation_id_hex"
        case operationKind = "operation_kind"
        case transactionHashHex = "transaction_hash_hex"
        case queryAuthority = "query_authority"
        case transactionAuthority = "transaction_authority"
        case blockHashHex = "block_hash_hex"
        case resultHashHex = "result_hash_hex"
        case committedBlockHeight = "committed_block_height"
        case finalizedCheckpointHex = "finalized_checkpoint_hex"
        case executedBlockWireHashHex = "executed_block_wire_hash_hex"
        case rejectionCode = "rejection_code"
        case rejectionMessage = "rejection_message"
        case evidenceIdHex = "evidence_id_hex"
        case transactionDetailsHashHex = "transaction_details_hash_hex"
        case finalityPageHashHex = "finality_page_hash_hex"
    }

    public init(from decoder: Decoder) throws {
        try AuthenticatedKagemushaFinalityValidationV1.requireExactFields(
            decoder,
            [
                "version", "terminal_state", "operation_id_hex", "operation_kind",
                "transaction_hash_hex", "query_authority", "transaction_authority",
                "block_hash_hex", "result_hash_hex", "committed_block_height",
                "finalized_checkpoint_hex", "executed_block_wire_hash_hex", "rejection_code",
                "rejection_message", "evidence_id_hex", "transaction_details_hash_hex",
                "finality_page_hash_hex",
            ]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard try container.decode(UInt32.self, forKey: .version) == 1 else {
            throw DecodingError.dataCorruptedError(
                forKey: .version,
                in: container,
                debugDescription: "finalized Kagemusha outcome version must be 1"
            )
        }
        let terminalState = try container.decode(TerminalState.self, forKey: .terminalState)
        let operationIdHex = try container.decode(String.self, forKey: .operationIdHex)
        let operationKind = try container.decode(String.self, forKey: .operationKind)
        let transactionHashHex = try container.decode(String.self, forKey: .transactionHashHex)
        let queryAuthority = try container.decode(String.self, forKey: .queryAuthority)
        let transactionAuthority = try container.decode(String.self, forKey: .transactionAuthority)
        let blockHashHex = try container.decode(String.self, forKey: .blockHashHex)
        let resultHashHex = try container.decode(String.self, forKey: .resultHashHex)
        let heightText = try container.decode(String.self, forKey: .committedBlockHeight)
        let checkpointHex = try container.decode(String.self, forKey: .finalizedCheckpointHex)
        let executedBlockWireHashHex = try container.decode(
            String.self,
            forKey: .executedBlockWireHashHex
        )
        guard container.contains(.rejectionCode), container.contains(.rejectionMessage) else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: container.codingPath, debugDescription: "rejection fields must be present even when null")
            )
        }
        let rejectionCode = try container.decodeIfPresent(String.self, forKey: .rejectionCode)
        let rejectionMessage = try container.decodeIfPresent(String.self, forKey: .rejectionMessage)
        let evidenceIdHex = try container.decode(String.self, forKey: .evidenceIdHex)
        let transactionDetailsHashHex = try container.decode(
            String.self,
            forKey: .transactionDetailsHashHex
        )
        let finalityPageHashHex = try container.decode(String.self, forKey: .finalityPageHashHex)
        guard let operationId = AuthenticatedKagemushaFinalityValidationV1.decodeLowerHex(
                  operationIdHex,
                  exactBytes: 32
              ),
              operationId.contains(where: { $0 != 0 }),
              operationKind == "top_up" || operationKind == "redeem",
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(transactionHashHex),
              AuthenticatedKagemushaFinalityValidationV1.isCanonicalText(
                  queryAuthority,
                  maximumUTF8Bytes: 16 * 1_024
              ),
              AuthenticatedKagemushaFinalityValidationV1.isCanonicalText(
                  transactionAuthority,
                  maximumUTF8Bytes: 16 * 1_024
              ),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(blockHashHex),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(resultHashHex),
              let committedBlockHeight =
                  AuthenticatedKagemushaFinalityValidationV1.positiveInt64(heightText),
              let checkpointBytes = AuthenticatedKagemushaFinalityValidationV1.decodeLowerHex(
                  checkpointHex,
                  exactBytes: AuthenticatedFinalityCheckpointV1.projectionBytes
              ),
              let finalizedCheckpoint = try? AuthenticatedFinalityCheckpointV1(
                  projectionBytes: checkpointBytes
              ),
              finalizedCheckpoint.height == committedBlockHeight,
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(
                  executedBlockWireHashHex
              ),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(evidenceIdHex),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(
                  transactionDetailsHashHex
              ),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(finalityPageHashHex) else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: container.codingPath, debugDescription: "finalized Kagemusha outcome is not canonical")
            )
        }
        switch terminalState {
        case .applied:
            guard rejectionCode == nil, rejectionMessage == nil else {
                throw DecodingError.dataCorrupted(
                    .init(codingPath: container.codingPath, debugDescription: "applied outcome cannot carry rejection fields")
                )
            }
        case .rejected:
            guard let rejectionCode,
                  let rejectionMessage,
                  AuthenticatedKagemushaFinalityValidationV1
                    .committedRejectionCodes.contains(rejectionCode),
                  AuthenticatedKagemushaFinalityValidationV1.isCanonicalText(
                      rejectionCode,
                      maximumUTF8Bytes: 128
                  ),
                  AuthenticatedKagemushaFinalityValidationV1.isCanonicalText(
                      rejectionMessage,
                      maximumUTF8Bytes: 1_024
                  ) else {
                throw DecodingError.dataCorrupted(
                    .init(codingPath: container.codingPath, debugDescription: "rejected outcome requires typed rejection fields")
                )
            }
        }
        self.terminalState = terminalState
        self.operationId = operationId
        self.operationKind = operationKind
        self.transactionHashHex = transactionHashHex
        self.queryAuthority = queryAuthority
        self.transactionAuthority = transactionAuthority
        self.blockHashHex = blockHashHex
        self.resultHashHex = resultHashHex
        self.committedBlockHeight = committedBlockHeight
        self.finalizedCheckpoint = finalizedCheckpoint
        self.executedBlockWireHashHex = executedBlockWireHashHex
        self.rejectionCode = rejectionCode
        self.rejectionMessage = rejectionMessage
        self.evidenceIdHex = evidenceIdHex
        self.transactionDetailsHashHex = transactionDetailsHashHex
        self.finalityPageHashHex = finalityPageHashHex
    }
}

/// Closed ABI-22 classification of a finalized Exact12 transaction rejection.
public enum AuthenticatedPrivacyActionRejectionCodeV1: String, CaseIterable, Decodable,
    Equatable, Sendable
{
    case accountDoesNotExist = "account_does_not_exist"
    case limitCheck = "limit_check"
    case validation
    case instructionExecution = "instruction_execution"
    case ivmExecution = "ivm_execution"
    case triggerExecution = "trigger_execution"

    public var canonicalLabel: String { rawValue }
}

/// Exact rejected Exact12 action independently authenticated by block and Sumeragi-v2 finality.
public struct AuthenticatedFinalizedPrivacyActionRejectionV1: Decodable, Equatable, Sendable {
    public let networkId: NetworkId
    public let protocolId: PrivacyProtocolIdV1
    public let operationSchema: PrivacyOperationSchemaV1
    public let ledgerEffectKind: PrivacyLedgerEffectKindV1
    public let transactionHashHex: String
    public let actionIndex: UInt32
    public let transactionIntentDigest: Data
    public let statementDigest: Data
    public let proofEnvelopeHash: Data
    public let queryAuthority: String
    public let transactionAuthority: String
    public let blockHashHex: String
    public let resultHashHex: String
    public let rejectionCode: AuthenticatedPrivacyActionRejectionCodeV1
    public let rejectionMessage: String
    public let committedBlockHeight: Int64
    public let finalizedCheckpoint: AuthenticatedFinalityCheckpointV1
    public let executedBlockWireHashHex: String
    public let evidenceIdHex: String
    public let transactionDetailsHashHex: String
    public let finalityPageHashHex: String

    public var networkIdHex: String { networkId.bytes.hexEncodedString() }

    private enum CodingKeys: String, CodingKey {
        case version
        case networkIdHex = "network_id_hex"
        case protocolId = "protocol_id"
        case operationSchema = "operation_schema"
        case ledgerEffectKind = "ledger_effect_kind"
        case transactionHashHex = "transaction_hash_hex"
        case actionIndex = "action_index"
        case transactionIntentDigestHex = "transaction_intent_digest_hex"
        case statementDigestHex = "statement_digest_hex"
        case proofEnvelopeHashHex = "proof_envelope_hash_hex"
        case queryAuthority = "query_authority"
        case transactionAuthority = "transaction_authority"
        case blockHashHex = "block_hash_hex"
        case resultHashHex = "result_hash_hex"
        case rejectionCode = "rejection_code"
        case rejectionMessage = "rejection_message"
        case committedBlockHeight = "committed_block_height"
        case finalizedCheckpointHex = "finalized_checkpoint_hex"
        case executedBlockWireHashHex = "executed_block_wire_hash_hex"
        case evidenceIdHex = "evidence_id_hex"
        case transactionDetailsHashHex = "transaction_details_hash_hex"
        case finalityPageHashHex = "finality_page_hash_hex"
    }

    public init(from decoder: Decoder) throws {
        try AuthenticatedKagemushaFinalityValidationV1.requireExactFields(
            decoder,
            [
                "version", "network_id_hex", "protocol_id", "operation_schema",
                "ledger_effect_kind", "transaction_hash_hex", "action_index",
                "transaction_intent_digest_hex", "statement_digest_hex",
                "proof_envelope_hash_hex", "query_authority", "transaction_authority",
                "block_hash_hex", "result_hash_hex", "rejection_code",
                "rejection_message", "committed_block_height", "finalized_checkpoint_hex",
                "executed_block_wire_hash_hex", "evidence_id_hex",
                "transaction_details_hash_hex", "finality_page_hash_hex",
            ]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard try container.decode(UInt32.self, forKey: .version) == 1 else {
            throw DecodingError.dataCorruptedError(
                forKey: .version,
                in: container,
                debugDescription: "finalized Exact12 rejection version must be 1"
            )
        }
        let networkIdHex = try container.decode(String.self, forKey: .networkIdHex)
        let protocolLabel = try container.decode(String.self, forKey: .protocolId)
        let operationLabel = try container.decode(String.self, forKey: .operationSchema)
        let ledgerEffectLabel = try container.decode(String.self, forKey: .ledgerEffectKind)
        let transactionHashHex = try container.decode(String.self, forKey: .transactionHashHex)
        let actionIndex = try container.decode(UInt32.self, forKey: .actionIndex)
        let intentHex = try container.decode(String.self, forKey: .transactionIntentDigestHex)
        let statementHex = try container.decode(String.self, forKey: .statementDigestHex)
        let envelopeHex = try container.decode(String.self, forKey: .proofEnvelopeHashHex)
        let queryAuthority = try container.decode(String.self, forKey: .queryAuthority)
        let transactionAuthority = try container.decode(String.self, forKey: .transactionAuthority)
        let blockHashHex = try container.decode(String.self, forKey: .blockHashHex)
        let resultHashHex = try container.decode(String.self, forKey: .resultHashHex)
        let rejectionCode = try container.decode(
            AuthenticatedPrivacyActionRejectionCodeV1.self,
            forKey: .rejectionCode
        )
        let rejectionMessage = try container.decode(String.self, forKey: .rejectionMessage)
        let heightText = try container.decode(String.self, forKey: .committedBlockHeight)
        let checkpointHex = try container.decode(String.self, forKey: .finalizedCheckpointHex)
        let executedBlockWireHashHex = try container.decode(
            String.self,
            forKey: .executedBlockWireHashHex
        )
        let evidenceIdHex = try container.decode(String.self, forKey: .evidenceIdHex)
        let transactionDetailsHashHex = try container.decode(
            String.self,
            forKey: .transactionDetailsHashHex
        )
        let finalityPageHashHex = try container.decode(String.self, forKey: .finalityPageHashHex)
        guard let networkIdBytes = AuthenticatedKagemushaFinalityValidationV1.decodeLowerHex(
                  networkIdHex,
                  exactBytes: 32
              ),
              let networkId = try? NetworkId(bytes: networkIdBytes),
              let protocolId = PrivacyProtocolIdV1(rawValue: protocolLabel),
              let operationSchema = PrivacyOperationSchemaV1.allCases.first(where: {
                  $0.canonicalLabel == operationLabel
              }),
              let ledgerEffectKind = PrivacyLedgerEffectKindV1(rawValue: ledgerEffectLabel),
              protocolId == operationSchema.protocolId,
              ledgerEffectKind == operationSchema.ledgerEffectKind,
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(transactionHashHex),
              actionIndex == 0,
              let transactionIntentDigest = Self.nonzeroDigest(intentHex),
              let statementDigest = Self.nonzeroDigest(statementHex),
              let proofEnvelopeHash = Self.nonzeroDigest(envelopeHex),
              AuthenticatedKagemushaFinalityValidationV1.isCanonicalText(
                  queryAuthority,
                  maximumUTF8Bytes: 16 * 1_024
              ),
              AuthenticatedKagemushaFinalityValidationV1.isCanonicalText(
                  transactionAuthority,
                  maximumUTF8Bytes: 16 * 1_024
              ),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(blockHashHex),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(resultHashHex),
              AuthenticatedKagemushaFinalityValidationV1.isCanonicalText(
                  rejectionMessage,
                  maximumUTF8Bytes: 1_024
              ),
              let committedBlockHeight =
                  AuthenticatedKagemushaFinalityValidationV1.positiveInt64(heightText),
              let checkpointBytes = AuthenticatedKagemushaFinalityValidationV1.decodeLowerHex(
                  checkpointHex,
                  exactBytes: AuthenticatedFinalityCheckpointV1.projectionBytes
              ),
              let finalizedCheckpoint = try? AuthenticatedFinalityCheckpointV1(
                  projectionBytes: checkpointBytes
              ),
              finalizedCheckpoint.height == committedBlockHeight,
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(
                  executedBlockWireHashHex
              ),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(evidenceIdHex),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(
                  transactionDetailsHashHex
              ),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(finalityPageHashHex) else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "finalized Exact12 rejection is not canonical"
                )
            )
        }
        self.networkId = networkId
        self.protocolId = protocolId
        self.operationSchema = operationSchema
        self.ledgerEffectKind = ledgerEffectKind
        self.transactionHashHex = transactionHashHex
        self.actionIndex = actionIndex
        self.transactionIntentDigest = transactionIntentDigest
        self.statementDigest = statementDigest
        self.proofEnvelopeHash = proofEnvelopeHash
        self.queryAuthority = queryAuthority
        self.transactionAuthority = transactionAuthority
        self.blockHashHex = blockHashHex
        self.resultHashHex = resultHashHex
        self.rejectionCode = rejectionCode
        self.rejectionMessage = rejectionMessage
        self.committedBlockHeight = committedBlockHeight
        self.finalizedCheckpoint = finalizedCheckpoint
        self.executedBlockWireHashHex = executedBlockWireHashHex
        self.evidenceIdHex = evidenceIdHex
        self.transactionDetailsHashHex = transactionDetailsHashHex
        self.finalityPageHashHex = finalityPageHashHex
    }

    private static func nonzeroDigest(_ value: String) -> Data? {
        guard let bytes = AuthenticatedKagemushaFinalityValidationV1.decodeLowerHex(
                  value,
                  exactBytes: 32
              ),
              bytes.contains(where: { $0 != 0 }) else {
            return nil
        }
        return bytes
    }
}

/// Exact identity authenticated by the installed specialized ABI-21/V4 top-up proof.
public struct VerifiedTopUpFinalityV4: Decodable, Equatable, Sendable {
    public let operationId: Data
    public let transactionHashHex: String
    public let committedBlockHeight: Int64
    public let blockHashHex: String
    public let heightContextId: Data

    private enum CodingKeys: String, CodingKey {
        case version
        case operationIdHex = "operation_id_hex"
        case transactionHashHex = "transaction_hash_hex"
        case committedBlockHeight = "committed_block_height"
        case blockHashHex = "block_hash_hex"
        case heightContextIdHex = "height_context_id_hex"
    }

    public init(from decoder: Decoder) throws {
        try AuthenticatedKagemushaFinalityValidationV1.requireExactFields(
            decoder,
            [
                "version", "operation_id_hex", "transaction_hash_hex",
                "committed_block_height", "block_hash_hex", "height_context_id_hex",
            ]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard try container.decode(UInt32.self, forKey: .version) == 4,
              let operationId = AuthenticatedKagemushaFinalityValidationV1.decodeLowerHex(
                  try container.decode(String.self, forKey: .operationIdHex),
                  exactBytes: 32
              ),
              operationId.contains(where: { $0 != 0 }),
              let height = AuthenticatedKagemushaFinalityValidationV1.positiveInt64(
                  try container.decode(String.self, forKey: .committedBlockHeight)
              ),
              let contextId = AuthenticatedKagemushaFinalityValidationV1.decodeLowerHex(
                  try container.decode(String.self, forKey: .heightContextIdHex),
                  exactBytes: 32
              ),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashBytes(contextId) else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: container.codingPath, debugDescription: "verified top-up finality projection is not canonical")
            )
        }
        let transactionHashHex = try container.decode(String.self, forKey: .transactionHashHex)
        let blockHashHex = try container.decode(String.self, forKey: .blockHashHex)
        guard AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(transactionHashHex),
              AuthenticatedKagemushaFinalityValidationV1.isMarkedHashHex(blockHashHex) else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: container.codingPath, debugDescription: "verified top-up hashes are not canonical")
            )
        }
        self.operationId = operationId
        self.transactionHashHex = transactionHashHex
        committedBlockHeight = height
        self.blockHashHex = blockHashHex
        heightContextId = contextId
    }
}

extension PrivacyNativeBridge {
    static func prepareAuthenticatedTransactionDetailsV2(
        networkId: NetworkId,
        queryAuthority: String,
        expectedTransactionAuthority: String,
        transactionHashHex: String,
        creationTimeMs: UInt64,
        nonce: Data
    ) throws -> PrivacyAuthenticatedTransactionDetailsPreparationV2 {
        guard isNativeAvailable,
              let prepared = try NoritoNativeBridge.shared
                .authenticatedTransactionDetailsPrepareV2(
                    networkId: networkId.bytes,
                    queryAuthority: queryAuthority,
                    expectedTransactionAuthority: expectedTransactionAuthority,
                    transactionHashHex: transactionHashHex,
                    creationTimeMs: creationTimeMs,
                    nonce: nonce
                ),
              prepared.signingDigest.count == 32 else {
            throw PrivacyCompiledProfileCatalogBridgeError.nativeUnavailable
        }
        return PrivacyAuthenticatedTransactionDetailsPreparationV2(
            archive: prepared.preparation,
            signingDigest: prepared.signingDigest
        )
    }

    static func finalizeAuthenticatedTransactionDetailsV2(
        _ preparation: PrivacyAuthenticatedTransactionDetailsPreparationV2,
        signature: Data
    ) throws -> Data {
        guard isNativeAvailable,
              let body = try NoritoNativeBridge.shared.authenticatedTransactionDetailsFinalizeV2(
                  preparation: preparation.archive,
                  signature: signature
              ) else {
            throw PrivacyCompiledProfileCatalogBridgeError.nativeUnavailable
        }
        return body
    }

    static func bindTransactionDetailsCarrierV2(
        preparation: PrivacyAuthenticatedTransactionDetailsPreparationV2,
        response: Data
    ) throws -> AuthenticatedTransactionDetailsCarrierV2 {
        guard isNativeAvailable,
              let json = try NoritoNativeBridge.shared.authenticatedTransactionDetailsProjectResultV2(
                  preparation: preparation.archive,
                  response: response
              ) else {
            throw PrivacyCompiledProfileCatalogBridgeError.nativeUnavailable
        }
        do {
            let projection = try JSONDecoder().decode(
                AuthenticatedCommittedTransactionResultV2.self,
                from: json
            )
            return try AuthenticatedTransactionDetailsCarrierV2(
                preparation: preparation,
                responseNorito: response,
                projection: projection
            )
        } catch {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
    }

    public static func bindFinalityProofPageV1(
        _ finalityProofsNorito: [Data]
    ) throws -> AuthenticatedFinalityProofPageV1 {
        guard (1...AuthenticatedFinalityProofPageV1.maximumProofCount)
                .contains(finalityProofsNorito.count) else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        var aggregate = 0
        for proof in finalityProofsNorito {
            guard !proof.isEmpty,
                  proof.count <= AuthenticatedFinalityProofPageV1.maximumProofBytes,
                  aggregate <= AuthenticatedFinalityProofPageV1.maximumPageBytes - proof.count else {
                throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
            }
            aggregate += proof.count
        }
        guard isNativeAvailable,
              let projection = try NoritoNativeBridge.shared.authenticatedFinalityProofPageBindV1(
                  finalityProofsNorito
              ) else {
            throw PrivacyCompiledProfileCatalogBridgeError.nativeUnavailable
        }
        return try AuthenticatedFinalityProofPageV1(
            evidenceArchive: projection.archive,
            hashHex: projection.hashHex
        )
    }

    public static func verifyFinalityPageV1(
        networkId: NetworkId,
        trustedCheckpoint: AuthenticatedFinalityCheckpointV1,
        page: AuthenticatedFinalityProofPageV1
    ) throws -> AuthenticatedFinalityCheckpointV1 {
        guard isNativeAvailable,
              let projection = try NoritoNativeBridge.shared.authenticatedFinalityPageVerifyV1(
                  networkId: networkId.bytes,
                  trustedCheckpoint: trustedCheckpoint.projection,
                  page: page.evidenceArchive
              ) else {
            throw PrivacyCompiledProfileCatalogBridgeError.nativeUnavailable
        }
        return try AuthenticatedFinalityCheckpointV1(projectionBytes: projection)
    }

    public static func projectFinalizedKagemushaOutcomeV1(
        carrier: AuthenticatedTransactionDetailsCarrierV2,
        expectedOperationId: Data,
        expectedKind: String,
        expectedRequestNorito: Data,
        networkId: NetworkId,
        trustedCheckpoint: AuthenticatedFinalityCheckpointV1,
        finalityPage: AuthenticatedFinalityProofPageV1,
        executedBlockWire: Data
    ) throws -> AuthenticatedFinalizedKagemushaOutcomeV1 {
        guard expectedOperationId.count == 32,
              expectedOperationId.contains(where: { $0 != 0 }),
              expectedKind == "top_up" || expectedKind == "redeem",
              !expectedRequestNorito.isEmpty,
              !executedBlockWire.isEmpty,
              executedBlockWire.count <= 32 * 1_024 * 1_024,
              isNativeAvailable,
              let json = try NoritoNativeBridge.shared
                .authenticatedFinalizedKagemushaOutcomeProjectV1(
                    preparation: carrier.nativePreparationArchive,
                    response: carrier.responseNorito,
                    expectedOperationId: expectedOperationId,
                    expectedKind: expectedKind,
                    expectedRequest: expectedRequestNorito,
                    networkId: networkId.bytes,
                    trustedCheckpoint: trustedCheckpoint.projection,
                    finalityPage: finalityPage.evidenceArchive,
                    executedBlockWire: executedBlockWire
                ) else {
            throw PrivacyCompiledProfileCatalogBridgeError.nativeUnavailable
        }
        let outcome: AuthenticatedFinalizedKagemushaOutcomeV1
        do {
            outcome = try JSONDecoder().decode(
                AuthenticatedFinalizedKagemushaOutcomeV1.self,
                from: json
            )
        } catch {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        guard outcome.committedBlockHeight == carrier.committedBlockHeightHint,
              (outcome.terminalState == .applied) == carrier.resultOkHint,
              outcome.transactionHashHex == carrier.transactionHashHex,
              outcome.queryAuthority == carrier.queryAuthority,
              outcome.transactionAuthority == carrier.transactionAuthority,
              outcome.transactionDetailsHashHex == carrier.transactionDetailsHashHex,
              outcome.transactionDetailsHashHex
                == AuthenticatedKagemushaFinalityValidationV1.markedHashHex(
                    carrier.responseNorito
                ),
              outcome.finalityPageHashHex == finalityPage.hashHex,
              outcome.finalityPageHashHex
                == AuthenticatedKagemushaFinalityValidationV1.markedHashHex(
                    finalityPage.evidenceArchive
                ),
              outcome.executedBlockWireHashHex
                == AuthenticatedKagemushaFinalityValidationV1.markedHashHex(
                    executedBlockWire
                ) else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        return outcome
    }

    /// Verify a rejected Exact12 action against its exact binding, executed block, and QC page.
    public static func projectFinalizedPrivacyActionRejectionV1(
        carrier: AuthenticatedTransactionDetailsCarrierV2,
        operation: PrivacyActionOperationViewV1,
        networkId: NetworkId,
        trustedCheckpoint: AuthenticatedFinalityCheckpointV1,
        finalityPage: AuthenticatedFinalityProofPageV1,
        executedBlockWire: Data
    ) throws -> AuthenticatedFinalizedPrivacyActionRejectionV1 {
        guard !carrier.resultOkHint,
              operation.operationSchema.rawValue <= UInt32(Int32.max),
              !executedBlockWire.isEmpty,
              executedBlockWire.count <= 32 * 1_024 * 1_024 else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        var requestedActionBinding = Data()
        requestedActionBinding.reserveCapacity(96)
        requestedActionBinding.append(operation.transactionIntentDigest)
        requestedActionBinding.append(operation.statementDigest)
        requestedActionBinding.append(operation.proofEnvelopeHash)
        guard requestedActionBinding.count == 96,
              isNativeAvailable,
              let json = try NoritoNativeBridge.shared
                .authenticatedFinalizedPrivacyActionRejectionProjectV1(
                    preparation: carrier.nativePreparationArchive,
                    response: carrier.responseNorito,
                    operationIndex: Int32(operation.operationSchema.rawValue),
                    actionIndex: 0,
                    requestedActionBinding: requestedActionBinding,
                    networkId: networkId.bytes,
                    trustedCheckpoint: trustedCheckpoint.projection,
                    finalityPage: finalityPage.evidenceArchive,
                    executedBlockWire: executedBlockWire
                ) else {
            throw PrivacyCompiledProfileCatalogBridgeError.nativeUnavailable
        }
        let rejection: AuthenticatedFinalizedPrivacyActionRejectionV1
        do {
            rejection = try JSONDecoder().decode(
                AuthenticatedFinalizedPrivacyActionRejectionV1.self,
                from: json
            )
        } catch {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        guard rejection.networkId == networkId,
              rejection.protocolId == operation.protocolId,
              rejection.operationSchema == operation.operationSchema,
              rejection.ledgerEffectKind == operation.ledgerEffectKind,
              rejection.transactionHashHex == operation.transactionHash.hexEncodedString(),
              rejection.transactionHashHex == carrier.transactionHashHex,
              rejection.actionIndex == 0,
              rejection.transactionIntentDigest == operation.transactionIntentDigest,
              rejection.statementDigest == operation.statementDigest,
              rejection.proofEnvelopeHash == operation.proofEnvelopeHash,
              rejection.queryAuthority == carrier.queryAuthority,
              rejection.transactionAuthority == carrier.transactionAuthority,
              rejection.committedBlockHeight == carrier.committedBlockHeightHint,
              rejection.finalizedCheckpoint.height > trustedCheckpoint.height,
              rejection.transactionDetailsHashHex == carrier.transactionDetailsHashHex,
              rejection.transactionDetailsHashHex
                == AuthenticatedKagemushaFinalityValidationV1.markedHashHex(
                    carrier.responseNorito
                ),
              rejection.finalityPageHashHex == finalityPage.hashHex,
              rejection.finalityPageHashHex
                == AuthenticatedKagemushaFinalityValidationV1.markedHashHex(
                    finalityPage.evidenceArchive
                ),
              rejection.executedBlockWireHashHex
                == AuthenticatedKagemushaFinalityValidationV1.markedHashHex(
                    executedBlockWire
                ) else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        return rejection
    }

    public static func projectVerifiedTopUpFinalityV4(
        anchor: KagemushaTopUpAnchor,
        finalityProof: KagemushaTopUpFinalityProof,
        rosterArtifact: KagemushaTopUpFinalityRosterArtifactArchive
    ) throws -> VerifiedTopUpFinalityV4 {
        guard isNativeAvailable,
              let json = try NoritoNativeBridge.shared.kagemushaTopUpFinalityProjectV4(
                  anchor: anchor.noritoArchive(),
                  finalityProof: finalityProof.noritoArchive,
                  rosterArtifact: rosterArtifact.noritoArchive
              ) else {
            throw PrivacyCompiledProfileCatalogBridgeError.nativeUnavailable
        }
        do {
            return try JSONDecoder().decode(VerifiedTopUpFinalityV4.self, from: json)
        } catch {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
    }

    public static func requireKagemushaTopUpFinalityAgreementV1(
        outcome: AuthenticatedFinalizedKagemushaOutcomeV1,
        specialized: VerifiedTopUpFinalityV4
    ) throws {
        guard outcome.terminalState == .applied,
              outcome.operationKind == "top_up",
              outcome.operationId == specialized.operationId,
              outcome.transactionHashHex == specialized.transactionHashHex,
              outcome.committedBlockHeight == specialized.committedBlockHeight,
              outcome.blockHashHex == specialized.blockHashHex,
              outcome.finalizedCheckpoint.heightContextId == specialized.heightContextId else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
    }
}

private enum AuthenticatedKagemushaFinalityValidationV1 {
    static let committedRejectionCodes: Set<String> = [
        "account_does_not_exist", "limit_check", "validation",
        "instruction_execution", "ivm_execution", "trigger_execution",
    ]

    static func requireExactFields(
        _ decoder: Decoder,
        _ expected: Set<String>
    ) throws {
        let actual = Set(
            try decoder.container(keyedBy: AuthenticatedKagemushaFinalityCodingKeyV1.self)
                .allKeys.map(\.stringValue)
        )
        guard actual == expected else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: decoder.codingPath, debugDescription: "finality JSON fields are not exact")
            )
        }
    }

    static func decodeLowerHex(_ value: String, exactBytes: Int) -> Data? {
        guard value.utf8.count == exactBytes * 2,
              value.utf8.allSatisfy({
                  (0x30...0x39).contains($0) || (0x61...0x66).contains($0)
              }) else {
            return nil
        }
        var output = Data()
        output.reserveCapacity(exactBytes)
        var high: UInt8?
        for byte in value.utf8 {
            let nibble: UInt8 = byte <= 0x39 ? byte - 0x30 : byte - 0x61 + 10
            if let first = high {
                output.append((first << 4) | nibble)
                high = nil
            } else {
                high = nibble
            }
        }
        return output.count == exactBytes ? output : nil
    }

    static func isMarkedHashHex(_ value: String) -> Bool {
        guard let bytes = decodeLowerHex(value, exactBytes: 32) else { return false }
        return isMarkedHashBytes(bytes)
    }

    static func isMarkedHashBytes(_ value: Data) -> Bool {
        value.count == 32 && value.last.map { ($0 & 1) == 1 } == true
    }

    static func markedHashHex(_ value: Data) -> String {
        var digest = Blake2b.hash256(value)
        precondition(digest.count == 32, "Blake2b-256 returned an invalid digest width")
        digest[digest.count - 1] |= 1
        return digest.hexEncodedString()
    }

    static func positiveInt64(_ value: String) -> Int64? {
        guard !value.isEmpty,
              value == "0" || !value.hasPrefix("0"),
              value.utf8.allSatisfy({ (0x30...0x39).contains($0) }),
              let parsed = Int64(value), parsed > 0 else {
            return nil
        }
        return parsed
    }

    static func isCanonicalText(_ value: String, maximumUTF8Bytes: Int) -> Bool {
        !value.isEmpty
            && value.utf8.count <= maximumUTF8Bytes
            && value == value.trimmingCharacters(in: .whitespacesAndNewlines)
            && !value.unicodeScalars.contains { CharacterSet.controlCharacters.contains($0) }
    }
}

private struct AuthenticatedKagemushaFinalityCodingKeyV1: CodingKey {
    let stringValue: String
    let intValue: Int? = nil

    init?(stringValue: String) {
        self.stringValue = stringValue
    }

    init?(intValue: Int) {
        return nil
    }
}
