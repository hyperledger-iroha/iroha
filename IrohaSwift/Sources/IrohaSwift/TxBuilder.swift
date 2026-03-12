import Foundation

public struct TransferRequest {
    public let chainId: String
    public let authority: String
    public let assetDefinitionId: String // e.g., "aid:2f17c72466f84a4bb8a8e24884fdcd2f"
    public let quantity: String         // decimal string
    public let destination: String      // account id
    public let description: String?
    public let ttlMs: UInt64?
    public let nonce: UInt32?

    public init(chainId: String,
                authority: String,
                assetDefinitionId: String,
                quantity: String,
                destination: String,
                description: String?,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.assetDefinitionId = assetDefinitionId
        self.quantity = quantity
        self.destination = destination
        self.description = description
        self.ttlMs = ttlMs
        self.nonce = nonce
    }
}

public struct MintRequest {
    public let chainId: String
    public let authority: String
    public let assetDefinitionId: String
    public let quantity: String
    public let destination: String
    public let ttlMs: UInt64?
    public let nonce: UInt32?

    public init(chainId: String,
                authority: String,
                assetDefinitionId: String,
                quantity: String,
                destination: String,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.assetDefinitionId = assetDefinitionId
        self.quantity = quantity
        self.destination = destination
        self.ttlMs = ttlMs
        self.nonce = nonce
    }
}

public struct BurnRequest {
    public let chainId: String
    public let authority: String
    public let assetDefinitionId: String
    public let quantity: String
    public let destination: String
    public let ttlMs: UInt64?
    public let nonce: UInt32?

    public init(chainId: String,
                authority: String,
                assetDefinitionId: String,
                quantity: String,
                destination: String,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.assetDefinitionId = assetDefinitionId
        self.quantity = quantity
        self.destination = destination
        self.ttlMs = ttlMs
        self.nonce = nonce
    }
}

public enum MetadataTarget: Sendable {
    case domain(String)
    case account(String)
    case assetDefinition(String)
    case asset(String)

    var targetKind: UInt8 {
        switch self {
        case .domain:
            return 0
        case .account:
            return 1
        case .assetDefinition:
            return 2
        case .asset:
            return 3
        }
    }

    var objectId: String {
        switch self {
        case .domain(let domainId):
            return domainId
        case .account(let accountId):
            return accountId
        case .assetDefinition(let definitionId):
            return definitionId
        case .asset(let assetId):
            return assetId
        }
    }
}

public struct SetMetadataRequest {
    public let chainId: String
    public let authority: String
    public let target: MetadataTarget
    public let key: String
    public let value: NoritoJSON
    public let ttlMs: UInt64?

    public init(chainId: String,
                authority: String,
                target: MetadataTarget,
                key: String,
                value: NoritoJSON,
                ttlMs: UInt64? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.target = target
        self.key = key
        self.value = value
        self.ttlMs = ttlMs
    }

    public init(chainId: String,
                authority: String,
                target: MetadataTarget,
                key: String,
                value: ToriiJSONValue,
                ttlMs: UInt64? = nil) throws {
        let encoded = try NoritoJSON(value)
        self.init(chainId: chainId,
                  authority: authority,
                  target: target,
                  key: key,
                  value: encoded,
                  ttlMs: ttlMs)
    }
}

public struct RemoveMetadataRequest {
    public let chainId: String
    public let authority: String
    public let target: MetadataTarget
    public let key: String
    public let ttlMs: UInt64?

    public init(chainId: String,
                authority: String,
                target: MetadataTarget,
                key: String,
                ttlMs: UInt64? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.target = target
        self.key = key
        self.ttlMs = ttlMs
    }
}

public struct MultisigRegisterRequest {
    public let chainId: String
    public let authority: String
    public let accountId: String
    public let spec: MultisigSpecPayload
    public let ttlMs: UInt64?

    public init(chainId: String,
                authority: String,
                accountId: String,
                spec: MultisigSpecPayload,
                ttlMs: UInt64? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.accountId = accountId
        self.spec = spec
        self.ttlMs = ttlMs
    }
}

public enum VerifyingKeyIdError: Error, LocalizedError, Equatable {
    case emptyBackend
    case emptyName
    case invalidSeparator

    public var errorDescription: String? {
        switch self {
        case .emptyBackend:
            return "Verifying key backend must not be empty."
        case .emptyName:
            return "Verifying key name must not be empty."
        case .invalidSeparator:
            return "Verifying key backend and name must not contain ':' characters."
        }
    }
}

public struct VerifyingKeyIdReference: Equatable, Sendable {
    public let backend: String
    public let name: String

    public init(backend: String, name: String) throws {
        guard !backend.isEmpty else {
            throw VerifyingKeyIdError.emptyBackend
        }
        guard !name.isEmpty else {
            throw VerifyingKeyIdError.emptyName
        }
        guard !backend.contains(":"), !name.contains(":") else {
            throw VerifyingKeyIdError.invalidSeparator
        }
        self.backend = backend
        self.name = name
    }

    var encodedValue: String {
        "\(backend):\(name)"
    }
}

public enum ZkAssetMode: UInt8, Sendable {
    case zkNative = 0
    case hybrid = 1
}

public struct RegisterZkAssetRequest {
    public let chainId: String
    public let authority: String
    public let assetDefinitionId: String
    public let mode: ZkAssetMode
    public let allowShield: Bool
    public let allowUnshield: Bool
    public let transferVerifyingKey: VerifyingKeyIdReference?
    public let unshieldVerifyingKey: VerifyingKeyIdReference?
    public let shieldVerifyingKey: VerifyingKeyIdReference?
    public let ttlMs: UInt64?

    public init(chainId: String,
                authority: String,
                assetDefinitionId: String,
                mode: ZkAssetMode = .hybrid,
                allowShield: Bool = true,
                allowUnshield: Bool = true,
                transferVerifyingKey: VerifyingKeyIdReference? = nil,
                unshieldVerifyingKey: VerifyingKeyIdReference? = nil,
                shieldVerifyingKey: VerifyingKeyIdReference? = nil,
                ttlMs: UInt64? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.assetDefinitionId = assetDefinitionId
        self.mode = mode
        self.allowShield = allowShield
        self.allowUnshield = allowUnshield
        self.transferVerifyingKey = transferVerifyingKey
        self.unshieldVerifyingKey = unshieldVerifyingKey
        self.shieldVerifyingKey = shieldVerifyingKey
        self.ttlMs = ttlMs
    }
}

public enum ShieldRequestError: Error, LocalizedError {
    case invalidNoteCommitmentLength

    public var errorDescription: String? {
        switch self {
        case .invalidNoteCommitmentLength:
            return "Note commitment must be exactly 32 bytes."
        }
    }
}

public struct ShieldRequest {
    public let chainId: String
    public let authority: String
    public let assetDefinitionId: String
    public let fromAccountId: String
    public let amount: String
    public let noteCommitment: Data
    public let payload: ConfidentialEncryptedPayload
    public let ttlMs: UInt64?

    public init(chainId: String,
                authority: String,
                assetDefinitionId: String,
                fromAccountId: String,
                amount: String,
                noteCommitment: Data,
                payload: ConfidentialEncryptedPayload,
                ttlMs: UInt64? = nil) throws {
        guard noteCommitment.count == 32 else {
            throw ShieldRequestError.invalidNoteCommitmentLength
        }
        self.chainId = chainId
        self.authority = authority
        self.assetDefinitionId = assetDefinitionId
        self.fromAccountId = fromAccountId
        self.amount = amount
        self.noteCommitment = noteCommitment
        self.payload = payload
        self.ttlMs = ttlMs
    }
}

public enum UnshieldRequestError: Error, LocalizedError {
    case inputsEmpty
    case invalidNullifierLength(expected: Int, actual: Int)
    case invalidRootHintLength

    public var errorDescription: String? {
        switch self {
        case .inputsEmpty:
            return "Unshield requests must include at least one nullifier."
        case let .invalidNullifierLength(expected, actual):
            return "Nullifier must be exactly \(expected) bytes (found \(actual))."
        case .invalidRootHintLength:
            return "Root hint must be exactly 32 bytes when provided."
        }
    }
}

public struct UnshieldRequest {
    private static let nullifierLength = 32

    public let chainId: String
    public let authority: String
    public let assetDefinitionId: String
    public let toAccountId: String
    public let publicAmount: String
    public let inputs: [Data]
    public let proof: ProofAttachment
    public let rootHint: Data?
    public let ttlMs: UInt64?

    public init(chainId: String,
                authority: String,
                assetDefinitionId: String,
                toAccountId: String,
                publicAmount: String,
                inputs: [Data],
                proof: ProofAttachment,
                rootHint: Data? = nil,
                ttlMs: UInt64? = nil) throws {
        guard !inputs.isEmpty else {
            throw UnshieldRequestError.inputsEmpty
        }
        for nullifier in inputs {
            guard nullifier.count == Self.nullifierLength else {
                throw UnshieldRequestError.invalidNullifierLength(expected: Self.nullifierLength,
                                                                  actual: nullifier.count)
            }
        }
        if let hint = rootHint, hint.count != Self.nullifierLength {
            throw UnshieldRequestError.invalidRootHintLength
        }
        self.chainId = chainId
        self.authority = authority
        self.assetDefinitionId = assetDefinitionId
        self.toAccountId = toAccountId
        self.publicAmount = publicAmount
        self.inputs = inputs
        self.proof = proof
        self.rootHint = rootHint
        self.ttlMs = ttlMs
    }

    var flattenedInputs: Data {
        Data(inputs.flatMap { $0 })
    }
}

public enum ZkTransferRequestError: Error, LocalizedError {
    case inputsEmpty
    case outputsEmpty
    case invalidInputLength(expected: Int, actual: Int)
    case invalidOutputLength(expected: Int, actual: Int)
    case invalidRootHintLength

    public var errorDescription: String? {
        switch self {
        case .inputsEmpty:
            return "ZkTransfer requests must include at least one input."
        case .outputsEmpty:
            return "ZkTransfer requests must include at least one output."
        case let .invalidInputLength(expected, actual):
            return "Input must be exactly \(expected) bytes (found \(actual))."
        case let .invalidOutputLength(expected, actual):
            return "Output must be exactly \(expected) bytes (found \(actual))."
        case .invalidRootHintLength:
            return "Root hint must be exactly 32 bytes when provided."
        }
    }
}

public struct ZkTransferRequest {
    private static let fieldElementLength = 32

    public let chainId: String
    public let authority: String
    public let assetDefinitionId: String
    public let inputs: [Data]
    public let outputs: [Data]
    public let proof: ProofAttachment
    public let rootHint: Data?
    public let ttlMs: UInt64?

    public init(chainId: String,
                authority: String,
                assetDefinitionId: String,
                inputs: [Data],
                outputs: [Data],
                proof: ProofAttachment,
                rootHint: Data? = nil,
                ttlMs: UInt64? = nil) throws {
        guard !inputs.isEmpty else {
            throw ZkTransferRequestError.inputsEmpty
        }
        for input in inputs {
            guard input.count == Self.fieldElementLength else {
                throw ZkTransferRequestError.invalidInputLength(expected: Self.fieldElementLength,
                                                                actual: input.count)
            }
        }
        guard !outputs.isEmpty else {
            throw ZkTransferRequestError.outputsEmpty
        }
        for output in outputs {
            guard output.count == Self.fieldElementLength else {
                throw ZkTransferRequestError.invalidOutputLength(expected: Self.fieldElementLength,
                                                                 actual: output.count)
            }
        }
        if let hint = rootHint, hint.count != Self.fieldElementLength {
            throw ZkTransferRequestError.invalidRootHintLength
        }
        self.chainId = chainId
        self.authority = authority
        self.assetDefinitionId = assetDefinitionId
        self.inputs = inputs
        self.outputs = outputs
        self.proof = proof
        self.rootHint = rootHint
        self.ttlMs = ttlMs
    }

    var flattenedInputs: Data {
        Data(inputs.flatMap { $0 })
    }

    var flattenedOutputs: Data {
        Data(outputs.flatMap { $0 })
    }
}

public struct GovernanceWindow: Sendable {
    public let lower: UInt64
    public let upper: UInt64

    public init(lower: UInt64, upper: UInt64) {
        self.lower = lower
        self.upper = upper
    }
}

public enum GovernanceVotingMode: UInt8, Sendable {
    case zk = 0
    case plain = 1
}

public enum BallotDirection: UInt8, Sendable {
    case aye = 0
    case nay = 1
    case abstain = 2
}

public enum CouncilDerivation: UInt8, Sendable {
    case vrf = 0
    case fallback = 1
}

public struct ProposeDeployContractRequest {
    public let chainId: String
    public let authority: String
    public let namespace: String
    public let contractId: String
    public let codeHashHex: String
    public let abiHashHex: String
    public let abiVersion: String
    public let window: GovernanceWindow?
    public let mode: GovernanceVotingMode?
    public let ttlMs: UInt64?

    public init(chainId: String,
                authority: String,
                namespace: String,
                contractId: String,
                codeHashHex: String,
                abiHashHex: String,
                abiVersion: String,
                window: GovernanceWindow? = nil,
                mode: GovernanceVotingMode? = nil,
                ttlMs: UInt64? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.namespace = namespace
        self.contractId = contractId
        self.codeHashHex = codeHashHex
        self.abiHashHex = abiHashHex
        self.abiVersion = abiVersion
        self.window = window
        self.mode = mode
        self.ttlMs = ttlMs
    }
}

public struct CastPlainBallotRequest {
    public let chainId: String
    public let authority: String
    public let referendumId: String
    public let owner: String
    public let amount: String
    public let durationBlocks: UInt64
    public let direction: BallotDirection
    public let ttlMs: UInt64?

    public init(chainId: String,
                authority: String,
                referendumId: String,
                owner: String,
                amount: String,
                durationBlocks: UInt64,
                direction: BallotDirection,
                ttlMs: UInt64? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.referendumId = referendumId
        self.owner = owner
        self.amount = amount
        self.durationBlocks = durationBlocks
        self.direction = direction
        self.ttlMs = ttlMs
    }
}

public struct CastZkBallotRequest {
    public let chainId: String
    public let authority: String
    public let electionId: String
    public let proofB64: String
    public let publicInputs: NoritoJSON
    public let ttlMs: UInt64?

    public init(chainId: String,
                authority: String,
                electionId: String,
                proofB64: String,
                publicInputs: NoritoJSON,
                ttlMs: UInt64? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.electionId = electionId
        self.proofB64 = proofB64
        self.publicInputs = publicInputs
        self.ttlMs = ttlMs
    }

    public init(chainId: String,
                authority: String,
                electionId: String,
                proofB64: String,
                publicInputs: ToriiJSONValue,
                ttlMs: UInt64? = nil) throws {
        let encoded = try NoritoJSON(publicInputs)
        self.init(chainId: chainId,
                  authority: authority,
                  electionId: electionId,
                  proofB64: proofB64,
                  publicInputs: encoded,
                  ttlMs: ttlMs)
    }
}

public struct EnactReferendumRequest {
    public let chainId: String
    public let authority: String
    public let referendumIdHex: String
    public let preimageHashHex: String
    public let window: GovernanceWindow
    public let ttlMs: UInt64?

    public init(chainId: String,
                authority: String,
                referendumIdHex: String,
                preimageHashHex: String,
                window: GovernanceWindow,
                ttlMs: UInt64? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.referendumIdHex = referendumIdHex
        self.preimageHashHex = preimageHashHex
        self.window = window
        self.ttlMs = ttlMs
    }
}

public struct FinalizeReferendumRequest {
    public let chainId: String
    public let authority: String
    public let referendumId: String
    public let proposalIdHex: String
    public let ttlMs: UInt64?

    public init(chainId: String,
                authority: String,
                referendumId: String,
                proposalIdHex: String,
                ttlMs: UInt64? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.referendumId = referendumId
        self.proposalIdHex = proposalIdHex
        self.ttlMs = ttlMs
    }
}

public struct PersistCouncilRequest {
    public let chainId: String
    public let authority: String
    public let epoch: UInt64
    public let members: [String]
    public let candidatesCount: UInt32
    public let derivedBy: CouncilDerivation
    public let ttlMs: UInt64?

    public init(chainId: String,
                authority: String,
                epoch: UInt64,
                members: [String],
                candidatesCount: UInt32,
                derivedBy: CouncilDerivation,
                ttlMs: UInt64? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.epoch = epoch
        self.members = members
        self.candidatesCount = candidatesCount
        self.derivedBy = derivedBy
        self.ttlMs = ttlMs
    }
}

public struct SignedTransactionEnvelope: Codable, Sendable {
    public let norito: Data
    public let signedTransaction: Data
    public let payload: Data?
    public let transactionHash: Data

    public var hashHex: String {
        transactionHash.map { String(format: "%02x", $0) }.joined()
    }
}

public struct PipelineSubmitOptions: Sendable {
    public typealias IdempotencyKeyFactory = @Sendable (SignedTransactionEnvelope) -> String?

    public static let defaultRetryableStatusCodes: Set<Int> = [429, 500, 502, 503, 504]
    public static let defaultIdempotencyKeyFactory: IdempotencyKeyFactory? = { envelope in
        envelope.hashHex
    }
    public static let `default` = PipelineSubmitOptions()

    public var maxRetries: Int
    public var initialBackoffSeconds: TimeInterval
    public var backoffMultiplier: Double
    public var retryableStatusCodes: Set<Int>
    public var idempotencyKeyFactory: IdempotencyKeyFactory?

    public init(maxRetries: Int = 3,
                initialBackoffSeconds: TimeInterval = 0.5,
                backoffMultiplier: Double = 2.0,
                retryableStatusCodes: Set<Int> = PipelineSubmitOptions.defaultRetryableStatusCodes,
                idempotencyKeyFactory: IdempotencyKeyFactory? = PipelineSubmitOptions.defaultIdempotencyKeyFactory) {
        self.maxRetries = max(maxRetries, 0)
        self.initialBackoffSeconds = max(initialBackoffSeconds, 0)
        self.backoffMultiplier = max(backoffMultiplier, 1)
        self.retryableStatusCodes = retryableStatusCodes
        self.idempotencyKeyFactory = idempotencyKeyFactory
    }
}

public struct PipelineStatusPollOptions: Sendable {
    public static let defaultSuccessStates: Set<PipelineTransactionState> = [.approved, .committed, .applied]
    public static let defaultFailureStates: Set<PipelineTransactionState> = [.rejected, .expired]
    public static let `default` = PipelineStatusPollOptions()

    public var pollInterval: TimeInterval
    public var timeout: TimeInterval
    public var maxAttempts: Int?
    public var successStatuses: Set<String>
    public var failureStatuses: Set<String>

    public init(pollInterval: TimeInterval = 0.5,
                timeout: TimeInterval = 30,
                maxAttempts: Int? = nil,
                successStates: Set<PipelineTransactionState> = PipelineStatusPollOptions.defaultSuccessStates,
                failureStates: Set<PipelineTransactionState> = PipelineStatusPollOptions.defaultFailureStates) {
        self.pollInterval = pollInterval
        self.timeout = timeout
        self.maxAttempts = maxAttempts
        self.successStatuses = Set(successStates.map { $0.kind })
        self.failureStatuses = Set(failureStates.map { $0.kind })
    }

    public init(pollInterval: TimeInterval,
                timeout: TimeInterval,
                maxAttempts: Int? = nil,
                successStatuses: Set<String>,
                failureStatuses: Set<String>) {
        self.pollInterval = pollInterval
        self.timeout = timeout
        self.maxAttempts = maxAttempts
        self.successStatuses = successStatuses
        self.failureStatuses = failureStatuses
    }

    public var successStates: Set<PipelineTransactionState> {
        Set(successStatuses.map { PipelineTransactionState(kind: $0) })
    }

    public var failureStates: Set<PipelineTransactionState> {
        Set(failureStatuses.map { PipelineTransactionState(kind: $0) })
    }
}

public enum PipelineStatusError: Error, LocalizedError {
    case timeout(hash: String, attempts: Int)
    case failure(hash: String, status: String, payload: ToriiPipelineTransactionStatus)

    public var rejectionReason: String? {
        guard case let .failure(_, _, payload) = self else {
            return nil
        }
        return Self.resolveRejectionReason(from: payload)
    }

    public var errorDescription: String? {
        switch self {
        case let .timeout(hash, attempts):
            return "Pipeline transaction \(hash) did not reach a terminal status after \(attempts) attempts."
        case let .failure(hash, status, payload):
            if let reason = Self.resolveRejectionReason(from: payload) {
                return "Pipeline transaction \(hash) failed with status \(status) (reason: \(reason))."
            }
            return "Pipeline transaction \(hash) failed with status \(status)."
        }
    }

    private static func resolveRejectionReason(from payload: ToriiPipelineTransactionStatus) -> String? {
        if let explicit = payload.content.status.rejectionReason?.trimmingCharacters(in: .whitespacesAndNewlines),
           !explicit.isEmpty {
            return explicit
        }
        if payload.content.status.kind == "Rejected",
           let fallback = payload.content.status.content?.trimmingCharacters(in: .whitespacesAndNewlines),
           !fallback.isEmpty {
            return fallback
        }
        return nil
    }
}

public final class IrohaSDK: @unchecked Sendable {
    public let baseURL: URL
    private let toriiClient: ToriiTransactionSubmitting
    private let toriiRestClient: ToriiClient?

    /// Current hardware acceleration settings. Setting this property applies the configuration immediately.
    public var accelerationSettings: AccelerationSettings {
        didSet { accelerationSettings.apply() }
    }

    /// Retry configuration for `/v1/pipeline/transactions` submissions.
    public var pipelineSubmitOptions: PipelineSubmitOptions

    /// Default polling behaviour for `submitAndWait` helpers (see `PipelineStatusPollOptions`).
    public var pipelinePollOptions: PipelineStatusPollOptions

    /// Selects the Torii transaction submission/status endpoints (pipeline-only).
    public var pipelineEndpointMode: PipelineEndpointMode

    /// Optional queue used to persist envelopes when submissions exhaust their retry budget.
    public var pendingTransactionQueue: PendingTransactionQueue?

    /// Provides the creation time (ms since epoch) used when signing transactions.
    public var creationTimeProvider: @Sendable () -> UInt64

    public init(baseURL: URL,
                session: URLSession = .shared,
                accelerationSettings: AccelerationSettings = AccelerationSettings(),
                pipelineSubmitOptions: PipelineSubmitOptions = .default,
                pipelinePollOptions: PipelineStatusPollOptions = .default,
                pipelineEndpointMode: PipelineEndpointMode = .pipeline,
                creationTimeProvider: @escaping @Sendable () -> UInt64 = IrohaSDK.defaultCreationTimeMs) {
        self.baseURL = baseURL
        let client = ToriiClient(baseURL: baseURL, session: session)
        self.toriiClient = client
        self.toriiRestClient = client
        self.accelerationSettings = accelerationSettings
        self.accelerationSettings.apply()
        self.pipelineSubmitOptions = pipelineSubmitOptions
        self.pipelinePollOptions = pipelinePollOptions
        self.pipelineEndpointMode = pipelineEndpointMode
        self.creationTimeProvider = creationTimeProvider
    }

    public init(toriiClient: ToriiTransactionSubmitting,
                baseURL: URL,
                accelerationSettings: AccelerationSettings = AccelerationSettings(),
                pipelineSubmitOptions: PipelineSubmitOptions = .default,
                pipelinePollOptions: PipelineStatusPollOptions = .default,
                pipelineEndpointMode: PipelineEndpointMode = .pipeline,
                creationTimeProvider: @escaping @Sendable () -> UInt64 = IrohaSDK.defaultCreationTimeMs) {
        self.baseURL = baseURL
        self.toriiClient = toriiClient
        self.toriiRestClient = toriiClient as? ToriiClient
        self.accelerationSettings = accelerationSettings
        self.accelerationSettings.apply()
        self.pipelineSubmitOptions = pipelineSubmitOptions
        self.pipelinePollOptions = pipelinePollOptions
        self.pipelineEndpointMode = pipelineEndpointMode
        self.creationTimeProvider = creationTimeProvider
    }

    public convenience init(toriiClient: ToriiClient,
                             accelerationSettings: AccelerationSettings = AccelerationSettings(),
                             pipelineSubmitOptions: PipelineSubmitOptions = .default,
                             pipelinePollOptions: PipelineStatusPollOptions = .default,
                             pipelineEndpointMode: PipelineEndpointMode = .pipeline,
                             creationTimeProvider: @escaping @Sendable () -> UInt64 = IrohaSDK.defaultCreationTimeMs) {
        self.init(toriiClient: toriiClient,
                  baseURL: toriiClient.baseURL,
                  accelerationSettings: accelerationSettings,
                  pipelineSubmitOptions: pipelineSubmitOptions,
                  pipelinePollOptions: pipelinePollOptions,
                  pipelineEndpointMode: pipelineEndpointMode,
                  creationTimeProvider: creationTimeProvider)
    }

    /// Default wall-clock provider for transaction creation timestamps (ms since epoch, clamped at zero).
    @Sendable public static func defaultCreationTimeMs() -> UInt64 {
        let millis = Date().timeIntervalSince1970 * 1_000
        if millis < 0 {
            return 0
        }
        return UInt64(millis.rounded())
    }

    private func makeCreationTimeMs() -> UInt64 {
        creationTimeProvider()
    }

    /// Build a signed transfer payload using the experimental Swift encoder.
    public func buildSignedTransfer(transfer: TransferRequest, keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeTransfer(transfer: transfer,
                                                          keypair: keypair,
                                                          creationTimeMs: creationTimeMs)
    }

    /// Build a signed transfer payload using a `SigningKey`.
    public func buildSignedTransfer(transfer: TransferRequest, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeTransfer(transfer: transfer,
                                                          signingKey: signingKey,
                                                          creationTimeMs: creationTimeMs)
    }

    /// Build and submit a transfer transaction using the experimental Swift encoder.
    public func submit(transfer: TransferRequest, keypair: Keypair, completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildSignedTransfer(transfer: transfer, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    /// Build and submit a transfer transaction using a `SigningKey`.
    public func submit(transfer: TransferRequest, signingKey: SigningKey, completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildSignedTransfer(transfer: transfer, signingKey: signingKey)
        submit(envelope: envelope, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submit(transfer: TransferRequest, keypair: Keypair) async throws {
        let envelope = try buildSignedTransfer(transfer: transfer, keypair: keypair)
        try await submit(envelope: envelope)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submit(transfer: TransferRequest, signingKey: SigningKey) async throws {
        let envelope = try buildSignedTransfer(transfer: transfer, signingKey: signingKey)
        try await submit(envelope: envelope)
    }

    public func buildMint(mint: MintRequest, keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeMint(request: mint,
                                                      keypair: keypair,
                                                      creationTimeMs: creationTimeMs)
    }

    public func buildMint(mint: MintRequest, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeMint(request: mint,
                                                      signingKey: signingKey,
                                                      creationTimeMs: creationTimeMs)
    }

    public func buildBurn(burn: BurnRequest, keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeBurn(request: burn,
                                                      keypair: keypair,
                                                      creationTimeMs: creationTimeMs)
    }

    public func buildBurn(burn: BurnRequest, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeBurn(request: burn,
                                                      signingKey: signingKey,
                                                      creationTimeMs: creationTimeMs)
    }

    public func buildMultisigRegister(request: MultisigRegisterRequest,
                                      keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeMultisigRegister(request: request,
                                                                  keypair: keypair,
                                                                  creationTimeMs: creationTimeMs)
    }

    public func buildMultisigRegister(request: MultisigRegisterRequest,
                                      signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeMultisigRegister(request: request,
                                                                  signingKey: signingKey,
                                                                  creationTimeMs: creationTimeMs)
    }

    public func buildShield(shield: ShieldRequest, keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeShield(request: shield,
                                                        keypair: keypair,
                                                        creationTimeMs: creationTimeMs)
    }

    public func buildShield(shield: ShieldRequest, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeShield(request: shield,
                                                        signingKey: signingKey,
                                                        creationTimeMs: creationTimeMs)
    }

    public func buildUnshield(unshield: UnshieldRequest, keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeUnshield(request: unshield,
                                                          keypair: keypair,
                                                          creationTimeMs: creationTimeMs)
    }

    public func buildUnshield(unshield: UnshieldRequest, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeUnshield(request: unshield,
                                                          signingKey: signingKey,
                                                          creationTimeMs: creationTimeMs)
    }

    public func buildZkTransfer(request: ZkTransferRequest, keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeZkTransfer(request: request,
                                                            keypair: keypair,
                                                            creationTimeMs: creationTimeMs)
    }

    public func buildZkTransfer(request: ZkTransferRequest, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeZkTransfer(request: request,
                                                            signingKey: signingKey,
                                                            creationTimeMs: creationTimeMs)
    }

    /// Build a `ClaimTwitterFollowReward` instruction payload (Norito JSON) for SOC-2 viral incentives.
    public func buildClaimTwitterFollowReward(binding: SocialKeyedHash) throws -> NoritoJSON {
        try SocialInstructionBuilders.claimTwitterFollowReward(binding: binding)
    }

    /// Convenience overload to build a `ClaimTwitterFollowReward` payload from pepper id and digest.
    public func buildClaimTwitterFollowReward(pepperId: String, digest: String) throws -> NoritoJSON {
        try SocialInstructionBuilders.claimTwitterFollowReward(pepperId: pepperId, digest: digest)
    }

    /// Build a `SendToTwitter` instruction payload (Norito JSON) for SOC-2 viral incentives.
    public func buildSendToTwitter(binding: SocialKeyedHash, amount: String) throws -> NoritoJSON {
        try SocialInstructionBuilders.sendToTwitter(binding: binding, amount: amount)
    }

    /// Convenience overload to build a `SendToTwitter` payload from pepper id and digest.
    public func buildSendToTwitter(pepperId: String, digest: String, amount: String) throws -> NoritoJSON {
        try SocialInstructionBuilders.sendToTwitter(pepperId: pepperId, digest: digest, amount: amount)
    }

    /// Build a `CancelTwitterEscrow` instruction payload (Norito JSON) for SOC-2 viral incentives.
    public func buildCancelTwitterEscrow(binding: SocialKeyedHash) throws -> NoritoJSON {
        try SocialInstructionBuilders.cancelTwitterEscrow(binding: binding)
    }

    /// Convenience overload to build a `CancelTwitterEscrow` payload from pepper id and digest.
    public func buildCancelTwitterEscrow(pepperId: String, digest: String) throws -> NoritoJSON {
        try SocialInstructionBuilders.cancelTwitterEscrow(pepperId: pepperId, digest: digest)
    }

    public func buildRegisterZkAsset(request: RegisterZkAssetRequest,
                                     keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeRegisterZkAsset(request: request,
                                                                 keypair: keypair,
                                                                 creationTimeMs: creationTimeMs)
    }

    public func buildRegisterZkAsset(request: RegisterZkAssetRequest,
                                     signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeRegisterZkAsset(request: request,
                                                                 signingKey: signingKey,
                                                                 creationTimeMs: creationTimeMs)
    }

    public func buildSetMetadata(request: SetMetadataRequest, keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeSetMetadata(request: request,
                                                             keypair: keypair,
                                                             creationTimeMs: creationTimeMs)
    }

    public func buildSetMetadata(request: SetMetadataRequest, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeSetMetadata(request: request,
                                                             signingKey: signingKey,
                                                             creationTimeMs: creationTimeMs)
    }

    public func buildRemoveMetadata(request: RemoveMetadataRequest, keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeRemoveMetadata(request: request,
                                                                keypair: keypair,
                                                                creationTimeMs: creationTimeMs)
    }

    public func buildRemoveMetadata(request: RemoveMetadataRequest, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeRemoveMetadata(request: request,
                                                                signingKey: signingKey,
                                                                creationTimeMs: creationTimeMs)
    }

    public func buildProposeDeploy(request: ProposeDeployContractRequest, keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeProposeDeploy(request: request,
                                                               keypair: keypair,
                                                               creationTimeMs: creationTimeMs)
    }

    public func buildProposeDeploy(request: ProposeDeployContractRequest, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeProposeDeploy(request: request,
                                                               signingKey: signingKey,
                                                               creationTimeMs: creationTimeMs)
    }

    public func buildCastPlainBallot(request: CastPlainBallotRequest, keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeCastPlainBallot(request: request,
                                                                 keypair: keypair,
                                                                 creationTimeMs: creationTimeMs)
    }

    public func buildCastPlainBallot(request: CastPlainBallotRequest, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeCastPlainBallot(request: request,
                                                                 signingKey: signingKey,
                                                                 creationTimeMs: creationTimeMs)
    }

    public func buildCastZkBallot(request: CastZkBallotRequest, keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                              keypair: keypair,
                                                              creationTimeMs: creationTimeMs)
    }

    public func buildCastZkBallot(request: CastZkBallotRequest, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                              signingKey: signingKey,
                                                              creationTimeMs: creationTimeMs)
    }

    public func buildEnactReferendum(request: EnactReferendumRequest, keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeEnactReferendum(request: request,
                                                                 keypair: keypair,
                                                                 creationTimeMs: creationTimeMs)
    }

    public func buildEnactReferendum(request: EnactReferendumRequest, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeEnactReferendum(request: request,
                                                                 signingKey: signingKey,
                                                                 creationTimeMs: creationTimeMs)
    }

    public func buildFinalizeReferendum(request: FinalizeReferendumRequest, keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeFinalizeReferendum(request: request,
                                                                    keypair: keypair,
                                                                    creationTimeMs: creationTimeMs)
    }

    public func buildFinalizeReferendum(request: FinalizeReferendumRequest, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeFinalizeReferendum(request: request,
                                                                    signingKey: signingKey,
                                                                    creationTimeMs: creationTimeMs)
    }

    public func buildPersistCouncil(request: PersistCouncilRequest, keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodePersistCouncil(request: request,
                                                                keypair: keypair,
                                                                creationTimeMs: creationTimeMs)
    }

    public func buildPersistCouncil(request: PersistCouncilRequest, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodePersistCouncil(request: request,
                                                                signingKey: signingKey,
                                                                creationTimeMs: creationTimeMs)
    }

    /// Submit a pre-built signed transaction envelope to Torii.
    @discardableResult
    public func submit(envelope: SignedTransactionEnvelope, completion: @Sendable @escaping (Error?) -> Void) -> Task<Void, Never> {
        return Task {
            do {
                _ = try await submitTransactionWithRetry(envelope: envelope,
                                                         options: pipelineSubmitOptions,
                                                         mode: pipelineEndpointMode)
                guard !Task.isCancelled else { return }
                completion(nil)
            } catch {
                guard !Task.isCancelled else { return }
                completion(error)
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submit(envelope: SignedTransactionEnvelope) async throws {
        _ = try await submitTransactionWithRetry(envelope: envelope,
                                                 options: pipelineSubmitOptions,
                                                 mode: pipelineEndpointMode)
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func submitAndWait(envelope: SignedTransactionEnvelope,
                               pollOptions: PipelineStatusPollOptions? = nil,
                               completion: @Sendable @escaping (Result<ToriiPipelineTransactionStatus, Error>) -> Void) -> Task<Void, Never> {
        let options = pollOptions ?? pipelinePollOptions
        return Task {
            do {
                try await submit(envelope: envelope)
                let status = try await awaitPipelineStatus(hashHex: envelope.hashHex,
                                                           pollOptions: options,
                                                           mode: pipelineEndpointMode)
                guard !Task.isCancelled else { return }
                await MainActor.run {
                    completion(.success(status))
                }
            } catch {
                guard !Task.isCancelled else { return }
                await MainActor.run {
                    completion(.failure(error))
                }
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitAndWait(envelope: SignedTransactionEnvelope,
                               pollOptions: PipelineStatusPollOptions? = nil) async throws -> ToriiPipelineTransactionStatus {
        try await submit(envelope: envelope)
        return try await awaitPipelineStatus(hashHex: envelope.hashHex,
                                             pollOptions: pollOptions ?? pipelinePollOptions,
                                             mode: pipelineEndpointMode)
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func pollPipelineStatus(hashHex: String,
                                   pollOptions: PipelineStatusPollOptions? = nil,
                                   completion: @Sendable @escaping (Result<ToriiPipelineTransactionStatus, Error>) -> Void) -> Task<Void, Never> {
        let options = pollOptions ?? pipelinePollOptions
        return Task {
            do {
                let status = try await awaitPipelineStatus(hashHex: hashHex,
                                                           pollOptions: options,
                                                           mode: pipelineEndpointMode)
                guard !Task.isCancelled else { return }
                await MainActor.run {
                    completion(.success(status))
                }
            } catch {
                guard !Task.isCancelled else { return }
                await MainActor.run {
                    completion(.failure(error))
                }
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func pollPipelineStatus(hashHex: String,
                                   pollOptions: PipelineStatusPollOptions? = nil) async throws -> ToriiPipelineTransactionStatus {
        try await awaitPipelineStatus(hashHex: hashHex,
                                      pollOptions: pollOptions ?? pipelinePollOptions,
                                      mode: pipelineEndpointMode)
    }

    public func getPipelineRecovery(height: UInt64, completion: @Sendable @escaping (Result<ToriiPipelineRecovery?, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getPipelineRecovery(height: height, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getPipelineRecovery(height: UInt64) async throws -> ToriiPipelineRecovery? {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getPipelineRecovery(height: height)
    }

    public func getTimeNow(completion: @Sendable @escaping (Result<ToriiTimeSnapshot, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getTimeNow(completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getTimeNow() async throws -> ToriiTimeSnapshot {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getTimeNow()
    }

    public func getTimeStatus(completion: @Sendable @escaping (Result<ToriiTimeStatusSnapshot, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getTimeStatus(completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getTimeStatus() async throws -> ToriiTimeStatusSnapshot {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getTimeStatus()
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getHealth() async throws -> String {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getHealth()
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getMetrics(asText: Bool = false) async throws -> ToriiMetricsResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getMetrics(asText: asText)
    }

    public func getNodeCapabilities(completion: @escaping (Result<ToriiNodeCapabilities, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getNodeCapabilities(completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getNodeCapabilities() async throws -> ToriiNodeCapabilities {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getNodeCapabilities()
    }

    public func getRuntimeMetrics(completion: @escaping (Result<ToriiRuntimeMetrics, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getRuntimeMetrics(completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getRuntimeMetrics() async throws -> ToriiRuntimeMetrics {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getRuntimeMetrics()
    }

    public func getRuntimeAbiActive(completion: @escaping (Result<ToriiRuntimeAbiActive, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getRuntimeAbiActive(completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getRuntimeAbiActive() async throws -> ToriiRuntimeAbiActive {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getRuntimeAbiActive()
    }

    public func getRuntimeAbiHash(completion: @escaping (Result<ToriiRuntimeAbiHash, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getRuntimeAbiHash(completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getRuntimeAbiHash() async throws -> ToriiRuntimeAbiHash {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getRuntimeAbiHash()
    }

    public func listRuntimeUpgrades(completion: @escaping (Result<[ToriiRuntimeUpgradeListItem], Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.listRuntimeUpgrades(completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func listRuntimeUpgrades() async throws -> [ToriiRuntimeUpgradeListItem] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.listRuntimeUpgrades()
    }

    public func proposeRuntimeUpgrade(manifest: ToriiRuntimeUpgradeManifest,
                                      completion: @escaping (Result<ToriiRuntimeUpgradeActionResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.proposeRuntimeUpgrade(manifest: manifest, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func proposeRuntimeUpgrade(manifest: ToriiRuntimeUpgradeManifest) async throws -> ToriiRuntimeUpgradeActionResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.proposeRuntimeUpgrade(manifest: manifest)
    }

    public func activateRuntimeUpgrade(idHex: String,
                                       completion: @escaping (Result<ToriiRuntimeUpgradeActionResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.activateRuntimeUpgrade(idHex: idHex, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func activateRuntimeUpgrade(idHex: String) async throws -> ToriiRuntimeUpgradeActionResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.activateRuntimeUpgrade(idHex: idHex)
    }

    public func cancelRuntimeUpgrade(idHex: String,
                                     completion: @escaping (Result<ToriiRuntimeUpgradeActionResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.cancelRuntimeUpgrade(idHex: idHex, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func cancelRuntimeUpgrade(idHex: String) async throws -> ToriiRuntimeUpgradeActionResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.cancelRuntimeUpgrade(idHex: idHex)
    }

    public func deriveConfidentialKeyset(seedHex: String? = nil,
                                         seedBase64: String? = nil,
                                         completion: @escaping (Result<ConfidentialKeyset, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        let trimmedHex = seedHex?.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedBase64 = seedBase64?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !(trimmedHex?.isEmpty ?? true) || !(trimmedBase64?.isEmpty ?? true) else {
            completion(.failure(ToriiClientError.invalidPayload("Provide either seedHex or seedBase64.")))
            return
        }
        toriiRestClient.deriveConfidentialKeyset(seedHex: trimmedHex, seedBase64: trimmedBase64) { result in
            switch result {
            case .success(let response):
                do {
                    let keyset = try response.asKeyset()
                    completion(.success(keyset))
                } catch {
                    completion(.failure(error))
                }
            case .failure(let error):
                completion(.failure(error))
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func deriveConfidentialKeyset(seedHex: String? = nil,
                                         seedBase64: String? = nil) async throws -> ConfidentialKeyset {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        let trimmedHex = seedHex?.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedBase64 = seedBase64?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !(trimmedHex?.isEmpty ?? true) || !(trimmedBase64?.isEmpty ?? true) else {
            throw ToriiClientError.invalidPayload("Provide either seedHex or seedBase64.")
        }
        let response = try await toriiRestClient.deriveConfidentialKeyset(seedHex: trimmedHex,
                                                                          seedBase64: trimmedBase64)
        return try response.asKeyset()
    }

    public func submitGovernanceDeployContractProposal(_ request: ToriiGovernanceDeployContractProposalRequest,
                                                       completion: @escaping (Result<ToriiGovernanceProposalResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.submitGovernanceDeployContractProposal(request, completion: completion)
    }

    public func submitGovernancePlainBallot(_ request: ToriiGovernancePlainBallotRequest,
                                            completion: @escaping (Result<ToriiGovernanceBallotResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.submitGovernancePlainBallot(request, completion: completion)
    }

    public func submitGovernanceZkBallot(_ request: ToriiGovernanceZkBallotRequest,
                                         completion: @escaping (Result<ToriiGovernanceBallotResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.submitGovernanceZkBallot(request, completion: completion)
    }

    public func finalizeGovernanceReferendum(_ request: ToriiGovernanceFinalizeRequest,
                                             completion: @escaping (Result<ToriiGovernanceFinalizeResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.finalizeGovernanceReferendum(request, completion: completion)
    }

    public func enactGovernanceProposal(_ request: ToriiGovernanceEnactRequest,
                                        completion: @escaping (Result<ToriiGovernanceEnactResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.enactGovernanceProposal(request, completion: completion)
    }

    public func getGovernanceProposal(idHex: String,
                                      completion: @escaping (Result<ToriiGovernanceProposalGetResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getGovernanceProposal(idHex: idHex, completion: completion)
    }

    public func getGovernanceLocks(referendumId: String,
                                   completion: @escaping (Result<ToriiGovernanceLocksResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getGovernanceLocks(referendumId: referendumId, completion: completion)
    }

    public func getGovernanceReferendum(id: String,
                                        completion: @escaping (Result<ToriiGovernanceReferendumResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getGovernanceReferendum(id: id, completion: completion)
    }

    public func getGovernanceTally(id: String,
                                   completion: @escaping (Result<ToriiGovernanceTallyResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getGovernanceTally(id: id, completion: completion)
    }

    public func getGovernanceUnlockStats(height: UInt64? = nil,
                                         referendumId: String? = nil,
                                         completion: @escaping (Result<ToriiGovernanceUnlockStatsResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getGovernanceUnlockStats(height: height,
                                                 referendumId: referendumId,
                                                 completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitGovernanceDeployContractProposal(_ request: ToriiGovernanceDeployContractProposalRequest) async throws -> ToriiGovernanceProposalResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.submitGovernanceDeployContractProposal(request)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitGovernancePlainBallot(_ request: ToriiGovernancePlainBallotRequest) async throws -> ToriiGovernanceBallotResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.submitGovernancePlainBallot(request)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitGovernanceZkBallot(_ request: ToriiGovernanceZkBallotRequest) async throws -> ToriiGovernanceBallotResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.submitGovernanceZkBallot(request)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func finalizeGovernanceReferendum(_ request: ToriiGovernanceFinalizeRequest) async throws -> ToriiGovernanceFinalizeResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.finalizeGovernanceReferendum(request)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func enactGovernanceProposal(_ request: ToriiGovernanceEnactRequest) async throws -> ToriiGovernanceEnactResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.enactGovernanceProposal(request)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getGovernanceProposal(idHex: String) async throws -> ToriiGovernanceProposalGetResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getGovernanceProposal(idHex: idHex)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getGovernanceLocks(referendumId: String) async throws -> ToriiGovernanceLocksResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getGovernanceLocks(referendumId: referendumId)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getGovernanceReferendum(id: String) async throws -> ToriiGovernanceReferendumResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getGovernanceReferendum(id: id)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getGovernanceTally(id: String) async throws -> ToriiGovernanceTallyResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getGovernanceTally(id: id)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getGovernanceUnlockStats(height: UInt64? = nil,
                                         referendumId: String? = nil) async throws -> ToriiGovernanceUnlockStatsResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getGovernanceUnlockStats(height: height, referendumId: referendumId)
    }

    private func submitTransactionWithRetry(envelope: SignedTransactionEnvelope,
                                            options: PipelineSubmitOptions,
                                            mode: PipelineEndpointMode,
                                            skipQueueFlush: Bool = false) async throws -> ToriiSubmitTransactionResponse? {
        if !skipQueueFlush {
            try await flushPendingTransactionsIfNeeded(mode: mode)
        }

        let idempotencyKey = options.idempotencyKeyFactory?(envelope)
        var attempt = 0
        var delay = options.initialBackoffSeconds
        while true {
            do {
                return try await toriiClient.submitTransaction(data: envelope.norito,
                                                               mode: mode,
                                                               idempotencyKey: idempotencyKey)
            } catch let cancellation as CancellationError {
                throw cancellation
            } catch let error as ToriiClientError {
                let retryable = isRetryable(error: error, options: options)
                if attempt < options.maxRetries && retryable {
                    attempt += 1
                } else {
                    if retryable {
                        try enqueuePendingTransaction(envelope)
                    }
                    throw error
                }
            } catch {
                if attempt < options.maxRetries {
                    attempt += 1
                } else {
                    try enqueuePendingTransaction(envelope)
                    throw error
                }
            }

            if delay > 0 {
                let nanosDouble = delay * 1_000_000_000
                let clamped = min(max(nanosDouble, 0), Double(UInt64.max))
                try await Task.sleep(nanoseconds: UInt64(clamped))
            } else {
                await Task.yield()
            }
            delay *= options.backoffMultiplier
        }
    }

    private func enqueuePendingTransaction(_ envelope: SignedTransactionEnvelope) throws {
        guard let queue = pendingTransactionQueue else { return }
        try queue.enqueue(envelope)
    }

    private func requeueRecords(_ records: ArraySlice<PendingTransactionRecord>) throws {
        guard let queue = pendingTransactionQueue else { return }
        for record in records {
            try queue.enqueue(record)
        }
    }

    private func flushPendingTransactionsIfNeeded(mode: PipelineEndpointMode) async throws {
        guard let queue = pendingTransactionQueue else { return }
        let pending = try queue.drain()
        guard !pending.isEmpty else { return }
        for index in pending.indices {
            let record = pending[index]
            do {
                _ = try await submitTransactionWithRetry(envelope: record.envelope,
                                                         options: pipelineSubmitOptions,
                                                         mode: mode,
                                                         skipQueueFlush: true)
            } catch {
                try requeueRecords(pending[index...])
                throw error
            }
        }
    }

    public func submit(mint request: MintRequest, keypair: Keypair, completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildMint(mint: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submit(mint request: MintRequest, keypair: Keypair) async throws {
        let envelope = try buildMint(mint: request, keypair: keypair)
        try await submit(envelope: envelope)
    }

    public func submit(registerZkAsset request: RegisterZkAssetRequest,
                       keypair: Keypair,
                       completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildRegisterZkAsset(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submit(registerZkAsset request: RegisterZkAssetRequest,
                       keypair: Keypair) async throws {
        let envelope = try buildRegisterZkAsset(request: request, keypair: keypair)
        try await submit(envelope: envelope)
    }

    public func submit(burn request: BurnRequest, keypair: Keypair, completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildBurn(burn: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submit(burn request: BurnRequest, keypair: Keypair) async throws {
        let envelope = try buildBurn(burn: request, keypair: keypair)
        try await submit(envelope: envelope)
    }

    public func submit(shield request: ShieldRequest, keypair: Keypair, completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildShield(shield: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submit(shield request: ShieldRequest, keypair: Keypair) async throws {
        let envelope = try buildShield(shield: request, keypair: keypair)
        try await submit(envelope: envelope)
    }

    public func submit(unshield request: UnshieldRequest, keypair: Keypair, completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildUnshield(unshield: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submit(unshield request: UnshieldRequest, keypair: Keypair) async throws {
        let envelope = try buildUnshield(unshield: request, keypair: keypair)
        try await submit(envelope: envelope)
    }

    public func submit(zkTransfer request: ZkTransferRequest, keypair: Keypair, completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildZkTransfer(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submit(zkTransfer request: ZkTransferRequest, keypair: Keypair) async throws {
        let envelope = try buildZkTransfer(request: request, keypair: keypair)
        try await submit(envelope: envelope)
    }

    public func submit(multisigRegister request: MultisigRegisterRequest,
                       keypair: Keypair,
                       completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildMultisigRegister(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    public func submit(multisigRegister request: MultisigRegisterRequest,
                           signingKey: SigningKey,
                           completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildMultisigRegister(request: request, signingKey: signingKey)
        submit(envelope: envelope, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submit(multisigRegister request: MultisigRegisterRequest,
                       keypair: Keypair) async throws {
        let envelope = try buildMultisigRegister(request: request, keypair: keypair)
        try await submit(envelope: envelope)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submit(multisigRegister request: MultisigRegisterRequest,
                       signingKey: SigningKey) async throws {
        let envelope = try buildMultisigRegister(request: request, signingKey: signingKey)
        try await submit(envelope: envelope)
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func submitAndWait(transfer: TransferRequest,
                               keypair: Keypair,
                               pollOptions: PipelineStatusPollOptions? = nil,
                               completion: @Sendable @escaping (Result<ToriiPipelineTransactionStatus, Error>) -> Void) -> Task<Void, Never> {
        do {
            let envelope = try buildSignedTransfer(transfer: transfer, keypair: keypair)
            return submitAndWait(envelope: envelope, pollOptions: pollOptions, completion: completion)
        } catch {
            return Task {
                await MainActor.run {
                    completion(.failure(error))
                }
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitAndWait(transfer: TransferRequest,
                               keypair: Keypair,
                               pollOptions: PipelineStatusPollOptions? = nil) async throws -> ToriiPipelineTransactionStatus {
        let envelope = try buildSignedTransfer(transfer: transfer, keypair: keypair)
        return try await submitAndWait(envelope: envelope, pollOptions: pollOptions)
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func submitAndWait(mint request: MintRequest,
                               keypair: Keypair,
                               pollOptions: PipelineStatusPollOptions? = nil,
                               completion: @Sendable @escaping (Result<ToriiPipelineTransactionStatus, Error>) -> Void) -> Task<Void, Never> {
        do {
            let envelope = try buildMint(mint: request, keypair: keypair)
            return submitAndWait(envelope: envelope, pollOptions: pollOptions, completion: completion)
        } catch {
            return Task {
                await MainActor.run {
                    completion(.failure(error))
                }
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitAndWait(mint request: MintRequest,
                               keypair: Keypair,
                               pollOptions: PipelineStatusPollOptions? = nil) async throws -> ToriiPipelineTransactionStatus {
        let envelope = try buildMint(mint: request, keypair: keypair)
        return try await submitAndWait(envelope: envelope, pollOptions: pollOptions)
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func submitAndWait(burn request: BurnRequest,
                               keypair: Keypair,
                               pollOptions: PipelineStatusPollOptions? = nil,
                               completion: @Sendable @escaping (Result<ToriiPipelineTransactionStatus, Error>) -> Void) -> Task<Void, Never> {
        do {
            let envelope = try buildBurn(burn: request, keypair: keypair)
            return submitAndWait(envelope: envelope, pollOptions: pollOptions, completion: completion)
        } catch {
            return Task {
                await MainActor.run {
                    completion(.failure(error))
                }
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitAndWait(burn request: BurnRequest,
                               keypair: Keypair,
                               pollOptions: PipelineStatusPollOptions? = nil) async throws -> ToriiPipelineTransactionStatus {
        let envelope = try buildBurn(burn: request, keypair: keypair)
        return try await submitAndWait(envelope: envelope, pollOptions: pollOptions)
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func submitAndWait(shield request: ShieldRequest,
                                  keypair: Keypair,
                                  pollOptions: PipelineStatusPollOptions? = nil,
                                  completion: @Sendable @escaping (Result<ToriiPipelineTransactionStatus, Error>) -> Void) -> Task<Void, Never> {
        do {
            let envelope = try buildShield(shield: request, keypair: keypair)
            return submitAndWait(envelope: envelope, pollOptions: pollOptions, completion: completion)
        } catch {
            return Task {
                await MainActor.run {
                    completion(.failure(error))
                }
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitAndWait(shield request: ShieldRequest,
                               keypair: Keypair,
                               pollOptions: PipelineStatusPollOptions? = nil) async throws -> ToriiPipelineTransactionStatus {
        let envelope = try buildShield(shield: request, keypair: keypair)
        return try await submitAndWait(envelope: envelope, pollOptions: pollOptions)
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func submitAndWait(unshield request: UnshieldRequest,
                                   keypair: Keypair,
                                   pollOptions: PipelineStatusPollOptions? = nil,
                                   completion: @Sendable @escaping (Result<ToriiPipelineTransactionStatus, Error>) -> Void) -> Task<Void, Never> {
        do {
            let envelope = try buildUnshield(unshield: request, keypair: keypair)
            return submitAndWait(envelope: envelope, pollOptions: pollOptions, completion: completion)
        } catch {
            return Task {
                await MainActor.run {
                    completion(.failure(error))
                }
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitAndWait(unshield request: UnshieldRequest,
                               keypair: Keypair,
                               pollOptions: PipelineStatusPollOptions? = nil) async throws -> ToriiPipelineTransactionStatus {
        let envelope = try buildUnshield(unshield: request, keypair: keypair)
        return try await submitAndWait(envelope: envelope, pollOptions: pollOptions)
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func submitAndWait(zkTransfer request: ZkTransferRequest,
                                   keypair: Keypair,
                                   pollOptions: PipelineStatusPollOptions? = nil,
                                   completion: @Sendable @escaping (Result<ToriiPipelineTransactionStatus, Error>) -> Void) -> Task<Void, Never> {
        do {
            let envelope = try buildZkTransfer(request: request, keypair: keypair)
            return submitAndWait(envelope: envelope, pollOptions: pollOptions, completion: completion)
        } catch {
            return Task {
                await MainActor.run {
                    completion(.failure(error))
                }
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitAndWait(zkTransfer request: ZkTransferRequest,
                               keypair: Keypair,
                               pollOptions: PipelineStatusPollOptions? = nil) async throws -> ToriiPipelineTransactionStatus {
        let envelope = try buildZkTransfer(request: request, keypair: keypair)
        return try await submitAndWait(envelope: envelope, pollOptions: pollOptions)
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func submitAndWait(multisigRegister request: MultisigRegisterRequest,
                                   keypair: Keypair,
                                   pollOptions: PipelineStatusPollOptions? = nil,
                                   completion: @Sendable @escaping (Result<ToriiPipelineTransactionStatus, Error>) -> Void) -> Task<Void, Never> {
        do {
            let envelope = try buildMultisigRegister(request: request, keypair: keypair)
            return submitAndWait(envelope: envelope, pollOptions: pollOptions, completion: completion)
        } catch {
            return Task {
                await MainActor.run {
                    completion(.failure(error))
                }
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitAndWait(multisigRegister request: MultisigRegisterRequest,
                               keypair: Keypair,
                               pollOptions: PipelineStatusPollOptions? = nil) async throws -> ToriiPipelineTransactionStatus {
        let envelope = try buildMultisigRegister(request: request, keypair: keypair)
        return try await submitAndWait(envelope: envelope, pollOptions: pollOptions)
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func submitAndWait(registerZkAsset request: RegisterZkAssetRequest,
                                   keypair: Keypair,
                                   pollOptions: PipelineStatusPollOptions? = nil,
                                   completion: @Sendable @escaping (Result<ToriiPipelineTransactionStatus, Error>) -> Void) -> Task<Void, Never> {
        do {
            let envelope = try buildRegisterZkAsset(request: request, keypair: keypair)
            return submitAndWait(envelope: envelope, pollOptions: pollOptions, completion: completion)
        } catch {
            return Task {
                await MainActor.run {
                    completion(.failure(error))
                }
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitAndWait(registerZkAsset request: RegisterZkAssetRequest,
                               keypair: Keypair,
                               pollOptions: PipelineStatusPollOptions? = nil) async throws -> ToriiPipelineTransactionStatus {
        let envelope = try buildRegisterZkAsset(request: request, keypair: keypair)
        return try await submitAndWait(envelope: envelope, pollOptions: pollOptions)
    }

    public func decodeSignedTransaction(envelope: SignedTransactionEnvelope) -> String? {
        NoritoNativeBridge.shared.decodeSignedTransaction(envelope.signedTransaction)
    }

    public func getAssets(accountId: String,
                          limit: Int = 100,
                          assetId: String? = nil,
                          completion: @Sendable @escaping (Result<[ToriiAssetBalance], Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getAssets(accountId: accountId, limit: limit, assetId: assetId, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getExplorerInstructions(params: ToriiExplorerInstructionsParams? = nil,
                                         completion: @Sendable @escaping (Result<ToriiExplorerInstructionsPage, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getExplorerInstructions(params: params, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getExplorerTransactions(params: ToriiExplorerTransactionsParams? = nil,
                                         completion: @Sendable @escaping (Result<ToriiExplorerTransactionsPage, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getExplorerTransactions(params: params, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getExplorerTransactionDetail(hashHex: String,
                                              completion: @Sendable @escaping (Result<ToriiExplorerTransactionDetail, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getExplorerTransactionDetail(hashHex: hashHex,
                                                     completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getExplorerInstructionDetail(hashHex: String,
                                              index: UInt64,
                                              completion: @Sendable @escaping (Result<ToriiExplorerInstructionItem, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getExplorerInstructionDetail(hashHex: hashHex,
                                                     index: index,
                                                     completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getExplorerTransfers(params: ToriiExplorerInstructionsParams? = nil,
                                     matchingAccount accountId: String? = nil,
                                     assetDefinitionId: String? = nil,
                                     assetId: String? = nil,
                                     completion: @Sendable @escaping (Result<[ToriiExplorerTransferRecord], Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getExplorerTransfers(params: params,
                                             matchingAccount: accountId,
                                             assetDefinitionId: assetDefinitionId,
                                             assetId: assetId,
                                             completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getExplorerTransferSummaries(params: ToriiExplorerInstructionsParams? = nil,
                                             matchingAccount accountId: String? = nil,
                                             assetDefinitionId: String? = nil,
                                             assetId: String? = nil,
                                             relativeTo relativeAccountId: String? = nil,
                                             completion: @Sendable @escaping (Result<[ToriiExplorerTransferSummary], Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getExplorerTransferSummaries(params: params,
                                                     matchingAccount: accountId,
                                                     assetDefinitionId: assetDefinitionId,
                                                     assetId: assetId,
                                                     relativeTo: relativeAccountId,
                                                     completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getExplorerTransactionTransfers(hashHex: String,
                                                matchingAccount accountId: String? = nil,
                                                assetDefinitionId: String? = nil,
                                                assetId: String? = nil,
                                                maxItems: UInt64? = nil,
                                                completion: @Sendable @escaping (Result<[ToriiExplorerTransferRecord], Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getExplorerTransactionTransfers(hashHex: hashHex,
                                                        matchingAccount: accountId,
                                                        assetDefinitionId: assetDefinitionId,
                                                        assetId: assetId,
                                                        maxItems: maxItems,
                                                        completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getExplorerTransactionTransferSummaries(hashHex: String,
                                                        matchingAccount accountId: String? = nil,
                                                        assetDefinitionId: String? = nil,
                                                        assetId: String? = nil,
                                                        relativeTo relativeAccountId: String? = nil,
                                                        maxItems: UInt64? = nil,
                                                        completion: @Sendable @escaping (Result<[ToriiExplorerTransferSummary], Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getExplorerTransactionTransferSummaries(hashHex: hashHex,
                                                                matchingAccount: accountId,
                                                                assetDefinitionId: assetDefinitionId,
                                                                assetId: assetId,
                                                                relativeTo: relativeAccountId,
                                                                maxItems: maxItems,
                                                                completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getAccountTransferHistory(accountId: String,
                                          page: UInt64? = nil,
                                          perPage: UInt64? = nil,
                                          assetDefinitionId: String? = nil,
                                          assetId: String? = nil,
                                          completion: @Sendable @escaping (Result<[ToriiExplorerTransferSummary], Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getAccountTransferHistory(accountId: accountId,
                                                  page: page,
                                                  perPage: perPage,
                                                  assetDefinitionId: assetDefinitionId,
                                                  assetId: assetId,
                                                  completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getTransactionHistory(accountId: String,
                                      page: UInt64? = nil,
                                      perPage: UInt64? = nil,
                                      assetDefinitionId: String? = nil,
                                      assetId: String? = nil,
                                      completion: @Sendable @escaping (Result<[ToriiExplorerTransferSummary], Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getTransactionHistory(accountId: accountId,
                                              page: page,
                                              perPage: perPage,
                                              assetDefinitionId: assetDefinitionId,
                                              assetId: assetId,
                                              completion: completion)
    }

    public func getTransactionStatus(hashHex: String, completion: @Sendable @escaping (Result<ToriiPipelineTransactionStatus?, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getTransactionStatus(hashHex: hashHex,
                                             mode: pipelineEndpointMode,
                                             completion: completion)
    }

    public func getHealth(completion: @escaping (Result<String, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getHealth(completion: completion)
    }

    public func getMetrics(asText: Bool = false,
                           completion: @escaping (Result<ToriiMetricsResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getMetrics(asText: asText, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    private func awaitPipelineStatus(hashHex: String,
                                     pollOptions: PipelineStatusPollOptions,
                                     mode: PipelineEndpointMode) async throws -> ToriiPipelineTransactionStatus {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await waitForPipelineStatus(hashHex: hashHex,
                                               options: pollOptions,
                                               statusClient: toriiRestClient,
                                               mode: mode)
    }

    @available(iOS 15.0, macOS 12.0, *)
    private func waitForPipelineStatus(hashHex: String,
                                       options: PipelineStatusPollOptions,
                                       statusClient: ToriiClient,
                                       mode: PipelineEndpointMode) async throws -> ToriiPipelineTransactionStatus {
        var attempts = 0
        let deadline = options.timeout > 0 ? Date().addingTimeInterval(options.timeout) : nil
        while true {
            try Task.checkCancellation()
            attempts += 1
            if let status = try await statusClient.getTransactionStatus(hashHex: hashHex,
                                                                         mode: mode) {
                let kind = status.content.status.kind
                if options.successStatuses.contains(kind) {
                    return status
                }
                if options.failureStatuses.contains(kind) {
                    throw PipelineStatusError.failure(hash: hashHex, status: kind, payload: status)
                }
            }
            if let maxAttempts = options.maxAttempts, attempts >= maxAttempts {
                throw PipelineStatusError.timeout(hash: hashHex, attempts: attempts)
            }
            if let deadline, Date() >= deadline {
                throw PipelineStatusError.timeout(hash: hashHex, attempts: attempts)
            }
            let interval = max(options.pollInterval, 0)
            if interval > 0 {
                let nanosDouble = interval * 1_000_000_000
                let clamped = min(max(nanosDouble, 0), Double(UInt64.max))
                try await Task.sleep(nanoseconds: UInt64(clamped))
            } else {
                await Task.yield()
            }
        }
    }

    private func isRetryable(error: ToriiClientError, options: PipelineSubmitOptions) -> Bool {
        switch error {
        case .transport:
            return true
        case let .httpStatus(code, _, _):
            return options.retryableStatusCodes.contains(code)
        default:
            return false
        }
    }
}

// MARK: - Experimental encoder helpers

public enum IrohaSDKError: Error, LocalizedError, Equatable {
    case restClientUnavailable
    case toriiRejected

    public var errorDescription: String? {
        switch self {
        case .restClientUnavailable:
            return "Torii REST client unavailable."
        case .toriiRejected:
            return "Torii rejected the submitted transaction."
        }
    }
}

extension IrohaSDK {
    static func restUnavailableError() -> IrohaSDKError {
        .restClientUnavailable
    }
}

@available(iOS 15.0, macOS 12.0, *)
public extension IrohaSDK {
    func getAssets(accountId: String, limit: Int = 100, assetId: String? = nil) async throws -> [ToriiAssetBalance] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getAssets(accountId: accountId, limit: limit, assetId: assetId)
    }

    func iterateAccountTransferHistory(accountId: String,
                                       page: UInt64? = nil,
                                       perPage: UInt64? = nil,
                                       assetDefinitionId: String? = nil,
                                       assetId: String? = nil,
                                       maxItems: UInt64? = nil) -> AsyncThrowingStream<ToriiExplorerTransferSummary, Error> {
        guard let toriiRestClient else {
            return AsyncThrowingStream { continuation in
                continuation.finish(throwing: Self.restUnavailableError())
            }
        }
        return toriiRestClient.iterateAccountTransferHistory(accountId: accountId,
                                                             page: page,
                                                             perPage: perPage,
                                                             assetDefinitionId: assetDefinitionId,
                                                             assetId: assetId,
                                                             maxItems: maxItems)
    }

    func streamExplorerTransactions(lastEventId: String? = nil) -> AsyncThrowingStream<ToriiExplorerTransactionItem, Error> {
        guard let toriiRestClient else {
            return AsyncThrowingStream { continuation in
                continuation.finish(throwing: Self.restUnavailableError())
            }
        }
        return toriiRestClient.streamExplorerTransactions(lastEventId: lastEventId)
    }

    func streamExplorerInstructions(lastEventId: String? = nil) -> AsyncThrowingStream<ToriiExplorerInstructionItem, Error> {
        guard let toriiRestClient else {
            return AsyncThrowingStream { continuation in
                continuation.finish(throwing: Self.restUnavailableError())
            }
        }
        return toriiRestClient.streamExplorerInstructions(lastEventId: lastEventId)
    }

    func streamExplorerTransfers(lastEventId: String? = nil,
                                 matchingAccount accountId: String? = nil,
                                 assetDefinitionId: String? = nil,
                                 assetId: String? = nil) -> AsyncThrowingStream<ToriiExplorerTransferRecord, Error> {
        guard let toriiRestClient else {
            return AsyncThrowingStream { continuation in
                continuation.finish(throwing: Self.restUnavailableError())
            }
        }
        return toriiRestClient.streamExplorerTransfers(lastEventId: lastEventId,
                                                       matchingAccount: accountId,
                                                       assetDefinitionId: assetDefinitionId,
                                                       assetId: assetId)
    }

    func streamExplorerTransferSummaries(lastEventId: String? = nil,
                                         matchingAccount accountId: String? = nil,
                                         assetDefinitionId: String? = nil,
                                         assetId: String? = nil,
                                         relativeTo relativeAccountId: String? = nil) -> AsyncThrowingStream<ToriiExplorerTransferSummary, Error> {
        guard let toriiRestClient else {
            return AsyncThrowingStream { continuation in
                continuation.finish(throwing: Self.restUnavailableError())
            }
        }
        return toriiRestClient.streamExplorerTransferSummaries(lastEventId: lastEventId,
                                                               matchingAccount: accountId,
                                                               assetDefinitionId: assetDefinitionId,
                                                               assetId: assetId,
                                                               relativeTo: relativeAccountId)
    }

    func streamAccountTransferHistory(accountId: String,
                                      page: UInt64? = nil,
                                      perPage: UInt64? = nil,
                                      assetDefinitionId: String? = nil,
                                      assetId: String? = nil,
                                      lastEventId: String? = nil,
                                      maxItems: UInt64? = nil,
                                      dedupeLimit: Int = 10_000) -> AsyncThrowingStream<ToriiExplorerTransferSummary, Error> {
        guard let toriiRestClient else {
            return AsyncThrowingStream { continuation in
                continuation.finish(throwing: Self.restUnavailableError())
            }
        }
        return toriiRestClient.streamAccountTransferHistory(accountId: accountId,
                                                            page: page,
                                                            perPage: perPage,
                                                            assetDefinitionId: assetDefinitionId,
                                                            assetId: assetId,
                                                            lastEventId: lastEventId,
                                                            maxItems: maxItems,
                                                            dedupeLimit: dedupeLimit)
    }

    func streamTransactionTransferSummaries(hashHex: String,
                                           matchingAccount accountId: String? = nil,
                                           assetDefinitionId: String? = nil,
                                           assetId: String? = nil,
                                           relativeTo relativeAccountId: String? = nil,
                                           lastEventId: String? = nil,
                                           maxItems: UInt64? = nil,
                                           dedupeLimit: Int = 10_000) -> AsyncThrowingStream<ToriiExplorerTransferSummary, Error> {
        guard let toriiRestClient else {
            return AsyncThrowingStream { continuation in
                continuation.finish(throwing: Self.restUnavailableError())
            }
        }
        return toriiRestClient.streamTransactionTransferSummaries(hashHex: hashHex,
                                                                  matchingAccount: accountId,
                                                                  assetDefinitionId: assetDefinitionId,
                                                                  assetId: assetId,
                                                                  relativeTo: relativeAccountId,
                                                                  lastEventId: lastEventId,
                                                                  maxItems: maxItems,
                                                                  dedupeLimit: dedupeLimit)
    }

    func getExplorerInstructions(params: ToriiExplorerInstructionsParams? = nil) async throws -> ToriiExplorerInstructionsPage {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getExplorerInstructions(params: params)
    }

    func getExplorerTransactions(params: ToriiExplorerTransactionsParams? = nil) async throws -> ToriiExplorerTransactionsPage {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getExplorerTransactions(params: params)
    }

    func getExplorerTransactionDetail(hashHex: String) async throws -> ToriiExplorerTransactionDetail {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getExplorerTransactionDetail(hashHex: hashHex)
    }

    func getExplorerInstructionDetail(hashHex: String,
                                      index: UInt64) async throws -> ToriiExplorerInstructionItem {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getExplorerInstructionDetail(hashHex: hashHex,
                                                                      index: index)
    }

    func getExplorerTransfers(params: ToriiExplorerInstructionsParams? = nil,
                              matchingAccount accountId: String? = nil,
                              assetDefinitionId: String? = nil,
                              assetId: String? = nil) async throws -> [ToriiExplorerTransferRecord] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getExplorerTransfers(params: params,
                                                              matchingAccount: accountId,
                                                              assetDefinitionId: assetDefinitionId,
                                                              assetId: assetId)
    }

    func getExplorerTransferSummaries(params: ToriiExplorerInstructionsParams? = nil,
                                      matchingAccount accountId: String? = nil,
                                      assetDefinitionId: String? = nil,
                                      assetId: String? = nil,
                                      relativeTo relativeAccountId: String? = nil) async throws -> [ToriiExplorerTransferSummary] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getExplorerTransferSummaries(params: params,
                                                                      matchingAccount: accountId,
                                                                      assetDefinitionId: assetDefinitionId,
                                                                      assetId: assetId,
                                                                      relativeTo: relativeAccountId)
    }

    func getExplorerTransactionTransfers(hashHex: String,
                                         matchingAccount accountId: String? = nil,
                                         assetDefinitionId: String? = nil,
                                         assetId: String? = nil,
                                         maxItems: UInt64? = nil) async throws -> [ToriiExplorerTransferRecord] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getExplorerTransactionTransfers(hashHex: hashHex,
                                                                         matchingAccount: accountId,
                                                                         assetDefinitionId: assetDefinitionId,
                                                                         assetId: assetId,
                                                                         maxItems: maxItems)
    }

    func getExplorerTransactionTransferSummaries(hashHex: String,
                                                 matchingAccount accountId: String? = nil,
                                                 assetDefinitionId: String? = nil,
                                                 assetId: String? = nil,
                                                 relativeTo relativeAccountId: String? = nil,
                                                 maxItems: UInt64? = nil) async throws -> [ToriiExplorerTransferSummary] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getExplorerTransactionTransferSummaries(hashHex: hashHex,
                                                                                 matchingAccount: accountId,
                                                                                 assetDefinitionId: assetDefinitionId,
                                                                                 assetId: assetId,
                                                                                 relativeTo: relativeAccountId,
                                                                                 maxItems: maxItems)
    }

    func getAccountTransferHistory(accountId: String,
                                   page: UInt64? = nil,
                                   perPage: UInt64? = nil,
                                   assetDefinitionId: String? = nil,
                                   assetId: String? = nil) async throws -> [ToriiExplorerTransferSummary] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getAccountTransferHistory(accountId: accountId,
                                                                   page: page,
                                                                   perPage: perPage,
                                                                   assetDefinitionId: assetDefinitionId,
                                                                   assetId: assetId)
    }

    func getTransactionHistory(accountId: String,
                               page: UInt64? = nil,
                               perPage: UInt64? = nil,
                               assetDefinitionId: String? = nil,
                               assetId: String? = nil) async throws -> [ToriiExplorerTransferSummary] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getTransactionHistory(accountId: accountId,
                                                               page: page,
                                                               perPage: perPage,
                                                               assetDefinitionId: assetDefinitionId,
                                                               assetId: assetId)
    }

    func getTransactionStatus(hashHex: String) async throws -> ToriiPipelineTransactionStatus? {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getTransactionStatus(hashHex: hashHex)
    }
}
