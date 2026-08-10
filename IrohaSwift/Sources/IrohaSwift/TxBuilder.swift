import Foundation

/// Validation errors for mixed executable transaction authoring.
public enum ExecutableBatchInputError: Error, LocalizedError, Equatable, Sendable {
    case emptyBatch
    case invalidInstructionWireName
    case invalidInstructionFrame
    case privacyExact12CapabilityAdmissionRequired
    case invalidContractAddress
    case invalidExpectedCodeHashLength(Int)
    case invalidExpectedCodeHashMarker
    case invalidEntrypoint
    case contractArgumentsTooLarge(Int)
    case missingGasLimit
    case zeroTimeToLive
    case zeroNonce

    public var errorDescription: String? {
        switch self {
        case .emptyBatch:
            return "An executable batch must contain at least one item."
        case .invalidInstructionWireName:
            return "Instruction wire name must be exact non-empty text."
        case .invalidInstructionFrame:
            return "Instruction payload must contain a valid Norito frame."
        case .privacyExact12CapabilityAdmissionRequired:
            return "SubmitPrivacyProofV1 must be constructed with an Exact12 capability admission."
        case .invalidContractAddress:
            return "Contract address must be a canonical lowercase V1 Bech32m literal."
        case let .invalidExpectedCodeHashLength(length):
            return "Expected contract code hash must be 32 bytes (received \(length))."
        case .invalidExpectedCodeHashMarker:
            return "Expected contract code hash must use Iroha's marked hash encoding."
        case .invalidEntrypoint:
            return "Contract entrypoint must be exact non-empty text without whitespace."
        case let .contractArgumentsTooLarge(length):
            return "Contract arguments exceed the 1048576-byte wire limit (received \(length))."
        case .missingGasLimit:
            return "A batch containing a contract call requires a signature-bound gas limit."
        case .zeroTimeToLive:
            return "Transaction time-to-live must be non-zero."
        case .zeroNonce:
            return "Transaction nonce must be non-zero."
        }
    }
}

/// Canonical dynamic instruction frame accepted by `InstructionBox`.
public struct TransactionInstructionFrame: Equatable, Sendable {
    public let wireName: String
    public let framedPayload: Data
    private let privacyProtocolId: PrivacyProtocolIdV1?
    private let privacyAdmission: PrivacyExact12CapabilityTupleAdmissionV1?

    public init(wireName: String, framedPayload: Data) throws {
        guard !wireName.isEmpty,
              wireName == wireName.trimmingCharacters(in: .whitespacesAndNewlines) else {
            throw ExecutableBatchInputError.invalidInstructionWireName
        }
        guard wireName != PrivacyExact12FixtureCodecV1.submitProofWireId else {
            throw ExecutableBatchInputError.privacyExact12CapabilityAdmissionRequired
        }
        guard noritoDecodeFrame(framedPayload) != nil else {
            throw ExecutableBatchInputError.invalidInstructionFrame
        }
        self.wireName = wireName
        self.framedPayload = Data(framedPayload)
        privacyProtocolId = nil
        privacyAdmission = nil
    }

    /// Construct the sole retained privacy instruction only after fresh
    /// committed/native Exact12 tuple admission. The token is revalidated here;
    /// merely retaining a token from an older binary is insufficient.
    public static func privacyExact12SubmitProof(
        protocolId: PrivacyProtocolIdV1,
        framedPayload: Data,
        admission: PrivacyExact12CapabilityTupleAdmissionV1
    ) throws -> TransactionInstructionFrame {
        try PrivacyExact12CapabilityAdmissionV1.requireForConstruction(
            admission,
            protocolId: protocolId,
            submitProofInstructionNorito: framedPayload
        )
        return TransactionInstructionFrame(
            admittedPrivacyWireName: PrivacyExact12FixtureCodecV1.submitProofWireId,
            framedPayload: framedPayload,
            protocolId: protocolId,
            admission: admission
        )
    }

    private init(
        admittedPrivacyWireName: String,
        framedPayload: Data,
        protocolId: PrivacyProtocolIdV1,
        admission: PrivacyExact12CapabilityTupleAdmissionV1
    ) {
        wireName = admittedPrivacyWireName
        self.framedPayload = Data(framedPayload)
        privacyProtocolId = protocolId
        privacyAdmission = admission
    }

    /// Encode the dynamic `InstructionBox` pair under the V1 `COMPACT_LEN` layout.
    func compactInstructionBoxPayload() throws -> Data {
        if wireName == PrivacyExact12FixtureCodecV1.submitProofWireId {
            guard let privacyProtocolId, let privacyAdmission else {
                throw ExecutableBatchInputError.privacyExact12CapabilityAdmissionRequired
            }
            // Re-run the ABI22 catalog getter+validator and the exact manifest/
            // envelope tuple comparison at final encoding. A previously issued
            // token cannot turn a missing or stale native artifact into authority.
            try PrivacyExact12CapabilityAdmissionV1.requireForConstruction(
                privacyAdmission,
                protocolId: privacyProtocolId,
                submitProofInstructionNorito: framedPayload
            )
        } else if privacyProtocolId != nil || privacyAdmission != nil {
            throw ExecutableBatchInputError.privacyExact12CapabilityAdmissionRequired
        }
        var framedBytes = CompactNoritoWriter()
        // `COMPACT_LEN` changes field prefixes, not the `Vec<u8>` element count.
        framedBytes.writeUInt64LE(UInt64(framedPayload.count))
        framedBytes.writeBytes(framedPayload)

        var instruction = CompactNoritoWriter()
        instruction.writeField(CompactNorito.encodeString(wireName))
        instruction.writeField(framedBytes.data)
        return instruction.data
    }

    public static func == (
        lhs: TransactionInstructionFrame,
        rhs: TransactionInstructionFrame
    ) -> Bool {
        lhs.wireName == rhs.wireName
            && lhs.framedPayload == rhs.framedPayload
            && lhs.privacyProtocolId == rhs.privacyProtocolId
            && (lhs.privacyAdmission != nil) == (rhs.privacyAdmission != nil)
    }
}

/// Signature-bound invocation of one deployed contract revision.
public struct TransactionContractInvocation: Equatable, Sendable {
    public static let maximumArgumentsBytes = 1024 * 1024

    public let contractAddress: String
    public let expectedCodeHash: Data
    public let entrypoint: String
    public let arguments: Data?

    public init(contractAddress: String,
                expectedCodeHash: Data,
                entrypoint: String,
                arguments: Data? = nil) throws {
        guard ContractAddressV1.isCanonical(contractAddress) else {
            throw ExecutableBatchInputError.invalidContractAddress
        }
        guard expectedCodeHash.count == 32 else {
            throw ExecutableBatchInputError.invalidExpectedCodeHashLength(expectedCodeHash.count)
        }
        guard let finalHashByte = expectedCodeHash.last, finalHashByte & 1 == 1 else {
            throw ExecutableBatchInputError.invalidExpectedCodeHashMarker
        }
        guard !entrypoint.isEmpty,
              entrypoint == entrypoint.trimmingCharacters(in: .whitespacesAndNewlines),
              !entrypoint.contains(where: \.isWhitespace) else {
            throw ExecutableBatchInputError.invalidEntrypoint
        }
        if let arguments, arguments.count > Self.maximumArgumentsBytes {
            throw ExecutableBatchInputError.contractArgumentsTooLarge(arguments.count)
        }
        self.contractAddress = contractAddress
        self.expectedCodeHash = expectedCodeHash
        self.entrypoint = entrypoint
        self.arguments = arguments
    }
}

/// One ordered item in an atomic mixed executable batch.
public enum TransactionBatchEntry: Equatable, Sendable {
    case instruction(TransactionInstructionFrame)
    case contractCall(TransactionContractInvocation)
}

public struct TransferRequest: Sendable {
    public let networkId: NetworkId
    public let authority: String
    public let assetDefinitionId: String // e.g., "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
    public let quantity: String         // decimal string
    public let destination: String      // i105 account id
    public let description: String?
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?
    public let nonce: UInt32?

    public init(networkId: NetworkId,
                authority: String,
                assetDefinitionId: String,
                quantity: String,
                destination: String,
                description: String?,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000,
                nonce: UInt32? = nil) {
        self.networkId = networkId
        self.authority = authority
        self.assetDefinitionId = assetDefinitionId
        self.quantity = quantity
        self.destination = destination
        self.description = description
        self.feePayment = feePayment
        self.ttlMs = ttlMs
        self.nonce = nonce
    }
}

public struct MintRequest {
    public let networkId: NetworkId
    public let authority: String
    public let assetDefinitionId: String
    public let quantity: String
    public let destination: String
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?
    public let nonce: UInt32?

    public init(networkId: NetworkId,
                authority: String,
                assetDefinitionId: String,
                quantity: String,
                destination: String,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000,
                nonce: UInt32? = nil) {
        self.networkId = networkId
        self.authority = authority
        self.assetDefinitionId = assetDefinitionId
        self.quantity = quantity
        self.destination = destination
        self.feePayment = feePayment
        self.ttlMs = ttlMs
        self.nonce = nonce
    }
}

public struct BurnRequest {
    public let networkId: NetworkId
    public let authority: String
    public let assetDefinitionId: String
    public let quantity: String
    public let destination: String
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?
    public let nonce: UInt32?

    public init(networkId: NetworkId,
                authority: String,
                assetDefinitionId: String,
                quantity: String,
                destination: String,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000,
                nonce: UInt32? = nil) {
        self.networkId = networkId
        self.authority = authority
        self.assetDefinitionId = assetDefinitionId
        self.quantity = quantity
        self.destination = destination
        self.feePayment = feePayment
        self.ttlMs = ttlMs
        self.nonce = nonce
    }
}

public enum MetadataTarget: Sendable {
    case domain(String)
    case account(String)
    case rwa(String)
    case assetDefinition(String)
    case asset(String)

    var targetKind: UInt8 {
        switch self {
        case .domain:
            return 0
        case .account:
            return 1
        case .rwa:
            return 4
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
        case .rwa(let rwaId):
            return rwaId
        case .assetDefinition(let definitionId):
            return definitionId
        case .asset(let assetId):
            return assetId
        }
    }
}

public struct SetMetadataRequest {
    public let networkId: NetworkId
    public let authority: String
    public let target: MetadataTarget
    public let key: String
    public let value: NoritoJSON
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?

    public init(networkId: NetworkId,
                authority: String,
                target: MetadataTarget,
                key: String,
                value: NoritoJSON,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000) {
        self.networkId = networkId
        self.authority = authority
        self.target = target
        self.key = key
        self.value = value
        self.feePayment = feePayment
        self.ttlMs = ttlMs
    }

    public init(networkId: NetworkId,
                authority: String,
                target: MetadataTarget,
                key: String,
                value: ToriiJSONValue,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000) throws {
        let encoded = try NoritoJSON(value)
        self.init(networkId: networkId,
                  authority: authority,
                  target: target,
                  key: key,
                  value: encoded,
                  feePayment: feePayment,
                  ttlMs: ttlMs)
    }
}

public struct RemoveMetadataRequest {
    public let networkId: NetworkId
    public let authority: String
    public let target: MetadataTarget
    public let key: String
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?

    public init(networkId: NetworkId,
                authority: String,
                target: MetadataTarget,
                key: String,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000) {
        self.networkId = networkId
        self.authority = authority
        self.target = target
        self.key = key
        self.feePayment = feePayment
        self.ttlMs = ttlMs
    }
}

public struct MultisigRegisterRequest {
    public let networkId: NetworkId
    public let authority: String
    public let accountId: String
    public let spec: MultisigSpecPayload
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?

    public init(networkId: NetworkId,
                authority: String,
                accountId: String,
                spec: MultisigSpecPayload,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000) {
        self.networkId = networkId
        self.authority = authority
        self.accountId = accountId
        self.spec = spec
        self.feePayment = feePayment
        self.ttlMs = ttlMs
    }
}

public struct ClaimIdentifierRequest {
    public let networkId: NetworkId
    public let authority: String
    public let accountId: String
    public let receipt: ToriiIdentifierResolutionReceipt
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?

    public init(networkId: NetworkId,
                authority: String,
                accountId: String,
                receipt: ToriiIdentifierResolutionReceipt,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000) {
        self.networkId = networkId
        self.authority = authority
        self.accountId = accountId
        self.receipt = receipt
        self.feePayment = feePayment
        self.ttlMs = ttlMs
    }
}

/// Inputs for the atomic nonce- and alias-CAS guarded contract deployment instruction.
public struct CommitContractDeploymentRequest {
    public let networkId: NetworkId
    public let authority: String
    public let expectedDeployNonce: UInt64
    public let contractAddress: String
    public let codeHashHex: String
    public let contractAlias: String
    public let leaseExpiryMs: UInt64?
    public let expectedPreviousContractAddress: String?
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?

    public init(networkId: NetworkId, authority: String, expectedDeployNonce: UInt64,
                contractAddress: String, codeHashHex: String, contractAlias: String,
                leaseExpiryMs: UInt64? = nil,
                expectedPreviousContractAddress: String? = nil,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000) {
        self.networkId = networkId
        self.authority = authority
        self.expectedDeployNonce = expectedDeployNonce
        self.contractAddress = contractAddress
        self.codeHashHex = codeHashHex
        self.contractAlias = contractAlias
        self.leaseExpiryMs = leaseExpiryMs
        self.expectedPreviousContractAddress = expectedPreviousContractAddress
        self.feePayment = feePayment
        self.ttlMs = ttlMs
    }
}

public enum VerifyingKeyIdError: Error, LocalizedError, Equatable {
    case emptyBackend
    case emptyName
    case invalidSeparator
    case surroundingWhitespace

    public var errorDescription: String? {
        switch self {
        case .emptyBackend:
            return "Verifying key backend must not be empty."
        case .emptyName:
            return "Verifying key name must not be empty."
        case .invalidSeparator:
            return "Verifying key backend and name must not contain ':' characters."
        case .surroundingWhitespace:
            return "Verifying key backend and name must not contain surrounding whitespace."
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
        guard backend.trimmingCharacters(in: .whitespacesAndNewlines) == backend,
              name.trimmingCharacters(in: .whitespacesAndNewlines) == name else {
            throw VerifyingKeyIdError.surroundingWhitespace
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

public enum RegisterZkAssetRequestError: Error, LocalizedError, Equatable, Sendable {
    case shieldVerifierRequiresUnshieldVerifier

    public var errorDescription: String? {
        switch self {
        case .shieldVerifierRequiresUnshieldVerifier:
            return "A shield verifier requires an unshield verifier so shielded funds remain redeemable."
        }
    }
}

public struct RegisterZkAssetRequest {
    public let networkId: NetworkId
    public let authority: String
    public let assetDefinitionId: String
    public let unshieldVerifyingKey: VerifyingKeyIdReference?
    public let shieldVerifyingKey: VerifyingKeyIdReference?
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?

    public init(networkId: NetworkId,
                authority: String,
                assetDefinitionId: String,
                unshieldVerifyingKey: VerifyingKeyIdReference? = nil,
                shieldVerifyingKey: VerifyingKeyIdReference? = nil,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000) throws {
        guard shieldVerifyingKey == nil || unshieldVerifyingKey != nil else {
            throw RegisterZkAssetRequestError.shieldVerifierRequiresUnshieldVerifier
        }
        self.networkId = networkId
        self.authority = authority
        self.assetDefinitionId = assetDefinitionId
        self.unshieldVerifyingKey = unshieldVerifyingKey
        self.shieldVerifyingKey = shieldVerifyingKey
        self.feePayment = feePayment
        self.ttlMs = ttlMs
    }
}

public struct GovernanceWindow: Sendable {
    public let lower: UInt64
    public let upper: UInt64

    public init(lower: UInt64, upper: UInt64) throws {
        guard lower <= upper else {
            throw TransactionInputError.invalidGovernanceWindow(lower: lower, upper: upper)
        }
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

public struct ProposeDeployContractRequest {
    public let networkId: NetworkId
    public let authority: String
    public let contractAddress: String
    public let codeHashHex: String
    public let abiHashHex: String
    public let abiVersion: String
    public let window: GovernanceWindow?
    public let mode: GovernanceVotingMode?
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?

    public init(networkId: NetworkId,
                authority: String,
                contractAddress: String,
                codeHashHex: String,
                abiHashHex: String,
                abiVersion: String,
                window: GovernanceWindow? = nil,
                mode: GovernanceVotingMode? = nil,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000) throws {
        guard abiVersion == "1" else {
            throw TransactionInputError.invalidGovernanceAbiVersion(abiVersion)
        }
        self.networkId = networkId
        self.authority = authority
        self.contractAddress = contractAddress
        self.codeHashHex = codeHashHex
        self.abiHashHex = abiHashHex
        self.abiVersion = abiVersion
        self.window = window
        self.mode = mode
        self.feePayment = feePayment
        self.ttlMs = ttlMs
    }
}

public struct CastPlainBallotRequest {
    public let networkId: NetworkId
    public let authority: String
    public let referendumId: String
    public let owner: String
    public let amount: String
    public let durationBlocks: UInt64
    public let direction: BallotDirection
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?

    public init(networkId: NetworkId,
                authority: String,
                referendumId: String,
                owner: String,
                amount: String,
                durationBlocks: UInt64,
                direction: BallotDirection,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000) {
        self.networkId = networkId
        self.authority = authority
        self.referendumId = referendumId
        self.owner = owner
        self.amount = amount
        self.durationBlocks = durationBlocks
        self.direction = direction
        self.feePayment = feePayment
        self.ttlMs = ttlMs
    }
}

public struct CastZkBallotRequest {
    public let networkId: NetworkId
    public let authority: String
    public let electionId: String
    public let proofB64: String
    public let publicInputs: GovernanceZkBallotPublicInputs
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?

    public init(networkId: NetworkId,
                authority: String,
                electionId: String,
                proofB64: String,
                publicInputs: GovernanceZkBallotPublicInputs = .init(),
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000) {
        self.networkId = networkId
        self.authority = authority
        self.electionId = electionId
        self.proofB64 = proofB64
        self.publicInputs = publicInputs
        self.feePayment = feePayment
        self.ttlMs = ttlMs
    }
}

public struct EnactReferendumRequest {
    public let networkId: NetworkId
    public let authority: String
    public let referendumIdHex: String
    public let preimageHashHex: String
    public let window: GovernanceWindow
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?

    public init(networkId: NetworkId,
                authority: String,
                referendumIdHex: String,
                preimageHashHex: String,
                window: GovernanceWindow,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000) {
        self.networkId = networkId
        self.authority = authority
        self.referendumIdHex = referendumIdHex
        self.preimageHashHex = preimageHashHex
        self.window = window
        self.feePayment = feePayment
        self.ttlMs = ttlMs
    }
}

public struct FinalizeReferendumRequest {
    public let networkId: NetworkId
    public let authority: String
    public let referendumId: String
    public let proposalIdHex: String
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?

    public init(networkId: NetworkId,
                authority: String,
                referendumId: String,
                proposalIdHex: String,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000) {
        self.networkId = networkId
        self.authority = authority
        self.referendumId = referendumId
        self.proposalIdHex = proposalIdHex
        self.feePayment = feePayment
        self.ttlMs = ttlMs
    }
}

public struct PersistCouncilRequest {
    public let networkId: NetworkId
    public let authority: String
    public let epoch: UInt64
    public let members: [String]
    public let feePayment: FeePaymentIntent
    public let ttlMs: UInt64?

    public init(networkId: NetworkId,
                authority: String,
                epoch: UInt64,
                members: [String],
                feePayment: FeePaymentIntent,
                ttlMs: UInt64? = 100_000) {
        self.networkId = networkId
        self.authority = authority
        self.epoch = epoch
        self.members = members
        self.feePayment = feePayment
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

    public static let defaultIdempotencyKeyFactory: IdempotencyKeyFactory? = { envelope in
        envelope.hashHex
    }
    public static let `default` = PipelineSubmitOptions()

    /// Builds the optional server-side deduplication key for the one submission attempt.
    ///
    /// The SDK never uses this key as authority to replay the signed envelope. After an
    /// ambiguous transport outcome, reconcile `SignedTransactionEnvelope.hashHex` through
    /// the pipeline status API before the application decides what to do next.
    public var idempotencyKeyFactory: IdempotencyKeyFactory?

    public init(idempotencyKeyFactory: IdempotencyKeyFactory? = PipelineSubmitOptions.defaultIdempotencyKeyFactory) {
        self.idempotencyKeyFactory = idempotencyKeyFactory
    }
}

public struct PipelineStatusPollOptions: Sendable {
    public static let `default` = PipelineStatusPollOptions()

    public var pollInterval: TimeInterval
    public var timeout: TimeInterval
    public var maxAttempts: Int?

    public init(pollInterval: TimeInterval = 0.5,
                timeout: TimeInterval = 30,
                maxAttempts: Int? = nil) {
        self.pollInterval = pollInterval
        self.timeout = timeout
        self.maxAttempts = maxAttempts
    }

    public var failureStates: Set<PipelineTransactionState> {
        [.rejected, .expired]
    }

    public var failureStatuses: Set<String> {
        [PipelineTransactionState.rejected.kind, PipelineTransactionState.expired.kind]
    }
}

public enum PipelineStatusError: Error, LocalizedError {
    case timeout(hash: String, attempts: Int)
    case failure(hash: String, status: String, payload: ToriiPipelineTransactionStatus)

    public var errorDescription: String? {
        switch self {
        case let .timeout(hash, attempts):
            return "Pipeline transaction \(hash) did not reach a terminal status after \(attempts) attempts."
        case let .failure(hash, status, _):
            return "Pipeline transaction \(hash) failed with status \(status)."
        }
    }
}

/// Exact cached top-up-shield verifier identity that an online preparation is
/// allowed to use. A wallet builds this from its authenticated product
/// capability and must refresh that capability after verifier rotation.
public struct KagemushaTopUpShieldVerifierBinding: Equatable, Sendable {
    public let backend: String
    public let name: String
    public let version: UInt32
    public let circuitID: String
    public let commitment: String
    public let publicInputsSchemaHash: String
    public let maximumProofBytes: UInt32
    public let activationHeight: UInt64
    public let withdrawalHeight: UInt64?

    public init(
        backend: String,
        name: String,
        version: UInt32,
        circuitID: String,
        commitment: String,
        publicInputsSchemaHash: String,
        maximumProofBytes: UInt32,
        activationHeight: UInt64,
        withdrawalHeight: UInt64?
    ) throws {
        guard backend == "halo2/ipa",
              !name.isEmpty,
              name.utf8.count <= 256,
              name.utf8.allSatisfy({ $0 >= 0x21 && $0 <= 0x7E }),
              circuitID == KagemushaRecursiveSpend.topUpShieldCircuitID,
              let commitmentBytes = Data(hexString: commitment),
              commitmentBytes.count == 32,
              commitmentBytes.contains(where: { $0 != 0 }),
              let schemaBytes = Data(hexString: publicInputsSchemaHash),
              schemaBytes.count == 32,
              schemaBytes.contains(where: { $0 != 0 }),
              maximumProofBytes > 0,
              maximumProofBytes <= 192 * 1024,
              withdrawalHeight.map({ $0 > activationHeight }) != false else {
            throw KagemushaRecursiveSpendError.invalidField("topUpShieldVerifierBinding")
        }
        self.backend = backend
        self.name = name
        self.version = version
        self.circuitID = circuitID
        self.commitment = commitment
        self.publicInputsSchemaHash = publicInputsSchemaHash
        self.maximumProofBytes = maximumProofBytes
        self.activationHeight = activationHeight
        self.withdrawalHeight = withdrawalHeight
    }

    public init(_ verifier: ToriiKagemushaActiveTopUpShieldVerifier) throws {
        try self.init(
            backend: verifier.id.backend,
            name: verifier.id.name,
            version: verifier.version,
            circuitID: verifier.circuitId,
            commitment: verifier.commitment,
            publicInputsSchemaHash: verifier.publicInputsSchemaHash,
            maximumProofBytes: verifier.maxProofBytes,
            activationHeight: verifier.activationHeight,
            withdrawalHeight: verifier.withdrawalHeight
        )
    }

    fileprivate func matches(_ verifier: ToriiKagemushaActiveTopUpShieldVerifier) -> Bool {
        backend == verifier.id.backend
            && name == verifier.id.name
            && version == verifier.version
            && circuitID == verifier.circuitId
            && commitment == verifier.commitment
            && publicInputsSchemaHash == verifier.publicInputsSchemaHash
            && maximumProofBytes == verifier.maxProofBytes
            && activationHeight == verifier.activationHeight
            && withdrawalHeight == verifier.withdrawalHeight
    }
}

/// Legacy-named product expectation supplied by the app for an online top-up.
/// It binds the selected asset and verifier lifecycle; it is not universal
/// offline discovery or backend readiness.
public struct KagemushaTopUpShieldReadinessExpectation: Equatable, Sendable {
    public let assetDefinitionID: String
    public let assetScale: UInt32
    public let minimumEvaluatedBlockHeight: UInt64
    public let verifier: KagemushaTopUpShieldVerifierBinding

    public init(
        assetDefinitionID: String,
        assetScale: UInt32,
        minimumEvaluatedBlockHeight: UInt64,
        verifier: KagemushaTopUpShieldVerifierBinding
    ) throws {
        guard AssetDefinitionAddress.decode(assetDefinitionID) != nil,
              assetScale <= KagemushaScaledAmount.maximumScale,
              verifier.activationHeight <= minimumEvaluatedBlockHeight,
              verifier.withdrawalHeight.map({ minimumEvaluatedBlockHeight < $0 }) != false else {
            throw KagemushaRecursiveSpendError.invalidField("topUpReadinessExpectation")
        }
        self.assetDefinitionID = assetDefinitionID
        self.assetScale = assetScale
        self.minimumEvaluatedBlockHeight = minimumEvaluatedBlockHeight
        self.verifier = verifier
    }
}

/// Reproducible live binding retained with a staged top-up until finality.
public struct KagemushaTopUpShieldSnapshotBinding: Equatable, Sendable {
    public let assetDefinitionID: String
    public let assetScale: UInt32
    public let evaluatedBlockHeight: UInt64
    public let evaluatedBlockHash: Data
    public let verifier: KagemushaTopUpShieldVerifierBinding
    public let initialRoot: Data
    public let leafIndex: UInt32
}

/// Unsigned top-up plus the exact live readiness/tree observation used to
/// construct it. Wallets persist this atomically with note secrets before
/// signing or submitting the request.
public struct KagemushaTopUpShieldPreparation: Equatable, Sendable {
    public let unsigned: KagemushaRecursiveSpendTopUpUnsignedV4
    public let opening: KagemushaNoteOpening
    /// Exact post-top-up membership and dummy-zero paths retained in encrypted
    /// local state. This witness never enters the Torii top-up request.
    public let membershipWitness: KagemushaNoteMembershipWitness
    public let binding: KagemushaTopUpShieldSnapshotBinding
}

public final class IrohaSDK: @unchecked Sendable {
    public let baseURL: URL
    public let defaultSigningAlgorithm: SigningAlgorithm
    private let toriiClient: ToriiTransactionSubmitting
    private let toriiRestClient: ToriiClient?

    /// Current hardware acceleration settings. Setting this property applies the configuration immediately.
    public var accelerationSettings: AccelerationSettings {
        didSet { accelerationSettings.apply() }
    }

    /// One-shot submission configuration for `/v1/pipeline/transactions`.
    public var pipelineSubmitOptions: PipelineSubmitOptions

    /// Default polling behaviour for `submitAndWait` helpers (see `PipelineStatusPollOptions`).
    public var pipelinePollOptions: PipelineStatusPollOptions

    /// Selects the Torii transaction submission/status endpoints (pipeline-only).
    public var pipelineEndpointMode: PipelineEndpointMode

    /// Provides the creation time (ms since epoch) used when signing transactions.
    public var creationTimeProvider: @Sendable () -> UInt64

    public init(baseURL: URL,
                session: URLSession = .shared,
                defaultSigningAlgorithm: SigningAlgorithm = .ed25519,
                accelerationSettings: AccelerationSettings = AccelerationSettings(),
                pipelineSubmitOptions: PipelineSubmitOptions = .default,
                pipelinePollOptions: PipelineStatusPollOptions = .default,
                pipelineEndpointMode: PipelineEndpointMode = .pipeline,
                creationTimeProvider: (@Sendable () -> UInt64)? = nil) {
        self.baseURL = baseURL
        self.defaultSigningAlgorithm = defaultSigningAlgorithm
        let client = ToriiClient(baseURL: baseURL, session: session)
        self.toriiClient = client
        self.toriiRestClient = client
        self.accelerationSettings = accelerationSettings
        self.accelerationSettings.apply()
        self.pipelineSubmitOptions = pipelineSubmitOptions
        self.pipelinePollOptions = pipelinePollOptions
        self.pipelineEndpointMode = pipelineEndpointMode
        self.creationTimeProvider = creationTimeProvider ?? { client.recommendedCreationTimeMs() }
    }

    public init(toriiClient: ToriiTransactionSubmitting,
                baseURL: URL,
                defaultSigningAlgorithm: SigningAlgorithm = .ed25519,
                accelerationSettings: AccelerationSettings = AccelerationSettings(),
                pipelineSubmitOptions: PipelineSubmitOptions = .default,
                pipelinePollOptions: PipelineStatusPollOptions = .default,
                pipelineEndpointMode: PipelineEndpointMode = .pipeline,
                creationTimeProvider: (@Sendable () -> UInt64)? = nil) {
        self.baseURL = baseURL
        self.defaultSigningAlgorithm = defaultSigningAlgorithm
        self.toriiClient = toriiClient
        self.toriiRestClient = toriiClient as? ToriiClient
        self.accelerationSettings = accelerationSettings
        self.accelerationSettings.apply()
        self.pipelineSubmitOptions = pipelineSubmitOptions
        self.pipelinePollOptions = pipelinePollOptions
        self.pipelineEndpointMode = pipelineEndpointMode
        if let creationTimeProvider {
            self.creationTimeProvider = creationTimeProvider
        } else if let restClient = toriiClient as? ToriiClient {
            self.creationTimeProvider = { restClient.recommendedCreationTimeMs() }
        } else {
            self.creationTimeProvider = IrohaSDK.defaultCreationTimeMs
        }
    }

    public convenience init(toriiClient: ToriiClient,
                             defaultSigningAlgorithm: SigningAlgorithm = .ed25519,
                             accelerationSettings: AccelerationSettings = AccelerationSettings(),
                             pipelineSubmitOptions: PipelineSubmitOptions = .default,
                             pipelinePollOptions: PipelineStatusPollOptions = .default,
                             pipelineEndpointMode: PipelineEndpointMode = .pipeline,
                             creationTimeProvider: (@Sendable () -> UInt64)? = nil) {
        self.init(toriiClient: toriiClient,
                  baseURL: toriiClient.baseURL,
                  defaultSigningAlgorithm: defaultSigningAlgorithm,
                  accelerationSettings: accelerationSettings,
                  pipelineSubmitOptions: pipelineSubmitOptions,
                  pipelinePollOptions: pipelinePollOptions,
                  pipelineEndpointMode: pipelineEndpointMode,
                  creationTimeProvider: creationTimeProvider)
    }

    /// Build an unsigned Kagemusha top-up from the caller-supplied product
    /// verifier binding and current authoritative next-zero Merkle path.
    ///
    /// The secret witness is consumed only by the local native prover. The
    /// returned preparation contains the spendable note descriptor, opaque
    /// shield proof, exact verifier/tree binding, and local note opening to
    /// persist atomically in encrypted storage before submission. Only the
    /// unsigned request is sent to Torii; the opening remains local.
    @available(iOS 15.0, macOS 12.0, *)
    public func prepareKagemushaTopUpShield(
        networkId: NetworkId,
        assetId: String,
        amount: KagemushaScaledAmount,
        payer: String,
        operationId: Data,
        opening: KagemushaNoteOpening,
        artifactBinding: KagemushaRecursiveSpendArtifactBindingV4,
        expectedReadiness: KagemushaTopUpShieldReadinessExpectation,
        canonicalAuth: ToriiCanonicalRequestAuth
    ) async throws -> KagemushaTopUpShieldPreparation {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        let canonicalAssetId = try KagemushaRecursiveSpendCodecs.canonicalAssetID(assetId)
        let assetParts = canonicalAssetId.split(
            separator: "#",
            omittingEmptySubsequences: false
        )
        guard assetParts.count == 2 || assetParts.count == 3 else {
            throw KagemushaRecursiveSpendError.invalidField("assetId")
        }
        let assetDefinitionId = String(assetParts[0])
        let verifier = expectedReadiness.verifier
        guard assetDefinitionId == expectedReadiness.assetDefinitionID,
              amount.scale == expectedReadiness.assetScale,
              let verifierCommitment = Data(hexString: verifier.commitment),
              verifierCommitment.count == 32 else {
            throw KagemushaRecursiveSpendError.invalidField("topUp.productCapability")
        }
        _ = try await toriiRestClient.getOfflineCapability()
        let snapshot = try await toriiRestClient.getZkAssetMerklePathSnapshot(
            asset: assetDefinitionId,
            commitments: [],
            canonicalAuth: canonicalAuth
        )
        guard snapshot.evaluatedBlockHeight >= expectedReadiness.minimumEvaluatedBlockHeight,
              verifier.activationHeight <= snapshot.evaluatedBlockHeight,
              verifier.withdrawalHeight.map({ snapshot.evaluatedBlockHeight < $0 }) != false else {
            throw KagemushaRecursiveSpendError.invalidField("topUp.productCapability.snapshot")
        }
        guard snapshot.frontierLen + 1
                < ToriiZkMerklePathResponse.confidentialTreeCapacityV2 else {
            throw KagemushaRecursiveSpendError.invalidField("topUp.zeroPath.capacity")
        }
        let zeroPath = try snapshot.validatedNextZeroPath()
        guard zeroPath.rootAtHeight == snapshot.root,
              zeroPath.leafIndex == UInt64(snapshot.frontierLen) else {
            throw KagemushaRecursiveSpendError.invalidField("topUp.zeroPath")
        }
        let unsigned = try KagemushaTopUpShieldBuildRequestV4(
            networkID: networkId,
            assetID: canonicalAssetId,
            amount: amount,
            payer: payer,
            operationID: operationId,
            opening: opening,
            leafIndex: UInt32(zeroPath.leafIndex),
            zeroPath: PrivacyConfidentialMerklePathWitnessV2(path: zeroPath),
            shieldVerifierID: "\(verifier.backend):\(verifier.name)",
            shieldVerifierCommitment: verifierCommitment,
            artifactBinding: artifactBinding
        ).buildUnsigned()
        guard try zeroPath.root(
            replacingLeafWith: unsigned.currentNote.noteCommitment
        ) == unsigned.shieldEvidence.finalizedRoot,
              unsigned.shieldEvidence.initialRoot == zeroPath.rootAtHeight,
              unsigned.shieldEvidence.leafIndex == UInt32(zeroPath.leafIndex) else {
            throw KagemushaRecursiveSpendError.invalidField("topUp.membershipWitness")
        }
        let dummyZeroPath = try zeroPath.nextZeroPathAfterInsertion(
            commitment: unsigned.currentNote.noteCommitment,
            expectedRoot: unsigned.shieldEvidence.finalizedRoot
        )
        let membershipWitness = try KagemushaNoteMembershipWitness(
            leafIndex: UInt32(zeroPath.leafIndex),
            inputPath: PrivacyConfidentialMerklePathWitnessV2(
                siblings: zeroPath.siblings,
                directions: zeroPath.directions,
                root: unsigned.shieldEvidence.finalizedRoot
            ),
            dummyInputPath: PrivacyConfidentialMerklePathWitnessV2(path: dummyZeroPath)
        )
        return KagemushaTopUpShieldPreparation(
            unsigned: unsigned,
            opening: opening,
            membershipWitness: membershipWitness,
            binding: KagemushaTopUpShieldSnapshotBinding(
                assetDefinitionID: assetDefinitionId,
                assetScale: amount.scale,
                evaluatedBlockHeight: snapshot.evaluatedBlockHeight,
                evaluatedBlockHash: snapshot.evaluatedBlockHash,
                verifier: expectedReadiness.verifier,
                initialRoot: zeroPath.rootAtHeight,
                leafIndex: UInt32(zeroPath.leafIndex)
            )
        )
    }

    /// Generates a new signing key using `defaultSigningAlgorithm`.
    @available(macOS 10.15, iOS 13.0, *)
    public func generateSigningKey(
        metadata: SigningMetadata = SigningMetadata()
    ) throws -> SigningKey {
        switch defaultSigningAlgorithm {
        case .ed25519:
            let keypair = try Keypair.generate()
            return try SigningKey.ed25519(privateKey: keypair.privateKeyBytes, metadata: metadata)
        default:
            var seed = Data((0..<32).map { _ in UInt8.random(in: UInt8.min...UInt8.max) })
            defer { seed.resetBytes(in: 0..<seed.count) }
            guard let derived = NoritoNativeBridge.shared.keypairFromSeed(
                algorithm: defaultSigningAlgorithm,
                seed: seed
            ) else {
                throw SigningKeyError.unsupportedAlgorithm(defaultSigningAlgorithm.wireName)
            }
            return try SigningKey.native(algorithm: defaultSigningAlgorithm,
                                         privateKey: derived.privateKey,
                                         metadata: metadata)
        }
    }

    /// Derives a signing key from seed material using `defaultSigningAlgorithm`.
    @available(macOS 10.15, iOS 13.0, *)
    public func signingKey(
        fromSeed seed: Data,
        metadata: SigningMetadata = SigningMetadata()
    ) throws -> SigningKey {
        switch defaultSigningAlgorithm {
        case .ed25519:
            if let derived = NoritoNativeBridge.shared.keypairFromSeed(
                algorithm: .ed25519,
                seed: seed
            ) {
                return try SigningKey.ed25519(privateKey: derived.privateKey, metadata: metadata)
            }
            return try SigningKey.ed25519(privateKey: seed, metadata: metadata)
        default:
            guard let derived = NoritoNativeBridge.shared.keypairFromSeed(
                algorithm: defaultSigningAlgorithm,
                seed: seed
            ) else {
                throw SigningKeyError.unsupportedAlgorithm(defaultSigningAlgorithm.wireName)
            }
            return try SigningKey.native(algorithm: defaultSigningAlgorithm,
                                         privateKey: derived.privateKey,
                                         metadata: metadata)
        }
    }

    /// Default wall-clock provider for transaction creation timestamps (ms since epoch, clamped at zero).
    @Sendable public static func defaultCreationTimeMs() -> UInt64 {
        let millis = Date().timeIntervalSince1970 * 1_000
        if millis < 0 {
            return 0
        }
        let rounded = UInt64(millis.rounded())
        let safetyMarginMs: UInt64 = 10_000
        return rounded > safetyMarginMs ? rounded - safetyMarginMs : 0
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

    /// Build and sign one atomic ordered mix of native instructions and deployed-contract calls.
    public func buildSignedExecutableBatch(
        networkId: NetworkId,
        authority: String,
        entries: [TransactionBatchEntry],
        feePayment: FeePaymentIntent,
        ttlMs: UInt64? = 100_000,
        nonce: UInt32? = nil,
        signingKey: SigningKey
    ) throws -> SignedTransactionEnvelope {
        try SingleInstructionSwiftNoritoEncoder.encodeExecutableBatch(
            networkId: networkId,
            authority: authority,
            creationTimeMs: makeCreationTimeMs(),
            ttlMs: ttlMs,
            nonce: nonce,
            entries: entries,
            feePayment: feePayment,
            signingKey: signingKey
        )
    }

    /// Ed25519 convenience overload for an atomic mixed executable batch.
    public func buildSignedExecutableBatch(
        networkId: NetworkId,
        authority: String,
        entries: [TransactionBatchEntry],
        feePayment: FeePaymentIntent,
        ttlMs: UInt64? = 100_000,
        nonce: UInt32? = nil,
        keypair: Keypair
    ) throws -> SignedTransactionEnvelope {
        try buildSignedExecutableBatch(
            networkId: networkId,
            authority: authority,
            entries: entries,
            feePayment: feePayment,
            ttlMs: ttlMs,
            nonce: nonce,
            signingKey: SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        )
    }

    /// Verify the planner commitment and exact frames, then locally sign one
    /// indivisible ordinary transaction containing the complete setup vector.
    public func buildAliasSetupPlan(
        _ request: AliasSetupPlanRequestV1,
        networkId: NetworkId,
        plan: AliasTransactionPlanV1,
        bodyEncoder: (AliasTransactionPlanBodyV1) throws -> Data,
        feePayment: FeePaymentIntent,
        ttlMs: UInt64? = 100_000,
        signingKey: SigningKey,
        frameCodec: (String, Data) throws -> DecodedEnsureAliasFrame =
            NativeAliasNoritoRegistryCodec.shared.decodeAndReencodeEnsureAlias
    ) throws -> SignedTransactionEnvelope {
        try SingleInstructionSwiftNoritoEncoder.encodeAliasSetupPlan(
            request: request,
            networkId: networkId,
            plan: plan,
            bodyEncoder: bodyEncoder,
            creationTimeMs: makeCreationTimeMs(),
            ttlMs: ttlMs,
            feePayment: feePayment,
            signingKey: signingKey,
            decodeAndReencode: frameCodec
        )
    }

    /// Ed25519 convenience overload for a verified atomic alias setup plan.
    public func buildAliasSetupPlan(
        _ request: AliasSetupPlanRequestV1,
        networkId: NetworkId,
        plan: AliasTransactionPlanV1,
        bodyEncoder: (AliasTransactionPlanBodyV1) throws -> Data,
        feePayment: FeePaymentIntent,
        ttlMs: UInt64? = 100_000,
        keypair: Keypair,
        frameCodec: (String, Data) throws -> DecodedEnsureAliasFrame =
            NativeAliasNoritoRegistryCodec.shared.decodeAndReencodeEnsureAlias
    ) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try buildAliasSetupPlan(
            request,
            networkId: networkId,
            plan: plan,
            bodyEncoder: bodyEncoder,
            feePayment: feePayment,
            ttlMs: ttlMs,
            signingKey: signingKey,
            frameCodec: frameCodec
        )
    }

    /// Build, locally sign, and submit one verified alias setup transaction.
    @available(iOS 15.0, macOS 12.0, *)
    public func submitAliasSetupPlan(
        _ request: AliasSetupPlanRequestV1,
        networkId: NetworkId,
        plan: AliasTransactionPlanV1,
        bodyEncoder: (AliasTransactionPlanBodyV1) throws -> Data,
        feePayment: FeePaymentIntent,
        ttlMs: UInt64? = 100_000,
        signingKey: SigningKey,
        frameCodec: (String, Data) throws -> DecodedEnsureAliasFrame =
            NativeAliasNoritoRegistryCodec.shared.decodeAndReencodeEnsureAlias
    ) async throws {
        let envelope = try buildAliasSetupPlan(
            request,
            networkId: networkId,
            plan: plan,
            bodyEncoder: bodyEncoder,
            feePayment: feePayment,
            ttlMs: ttlMs,
            signingKey: signingKey,
            frameCodec: frameCodec
        )
        try await submit(envelope: envelope)
    }

    /// Verify a lease-renewal or auto-renew plan and locally sign its one exact
    /// instruction. Exact auto-renew no-ops return `nil` without a transaction.
    public func buildAliasLifecyclePlan(
        _ request: AliasLifecyclePlanRequestV1,
        networkId: NetworkId,
        plan: AliasLifecycleTransactionPlanV1,
        bodyEncoder: (AliasLifecycleTransactionPlanBodyV1) throws -> Data,
        feePayment: FeePaymentIntent,
        ttlMs: UInt64? = 100_000,
        signingKey: SigningKey,
        frameCodec: (String, Data) throws -> DecodedAliasLifecycleFrame =
            NativeAliasNoritoRegistryCodec.shared.decodeAndReencodeLifecycle
    ) throws -> SignedTransactionEnvelope? {
        try SingleInstructionSwiftNoritoEncoder.encodeAliasLifecyclePlan(
            request: request,
            networkId: networkId,
            plan: plan,
            bodyEncoder: bodyEncoder,
            creationTimeMs: makeCreationTimeMs(),
            ttlMs: ttlMs,
            feePayment: feePayment,
            signingKey: signingKey,
            decodeAndReencode: frameCodec
        )
    }

    /// Ed25519 convenience overload for a verified alias lifecycle plan.
    public func buildAliasLifecyclePlan(
        _ request: AliasLifecyclePlanRequestV1,
        networkId: NetworkId,
        plan: AliasLifecycleTransactionPlanV1,
        bodyEncoder: (AliasLifecycleTransactionPlanBodyV1) throws -> Data,
        feePayment: FeePaymentIntent,
        ttlMs: UInt64? = 100_000,
        keypair: Keypair,
        frameCodec: (String, Data) throws -> DecodedAliasLifecycleFrame =
            NativeAliasNoritoRegistryCodec.shared.decodeAndReencodeLifecycle
    ) throws -> SignedTransactionEnvelope? {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try buildAliasLifecyclePlan(
            request,
            networkId: networkId,
            plan: plan,
            bodyEncoder: bodyEncoder,
            feePayment: feePayment,
            ttlMs: ttlMs,
            signingKey: signingKey,
            frameCodec: frameCodec
        )
    }

    /// Build and submit one verified lifecycle transaction. Returns `false`
    /// for an exact auto-renew no-op and never submits an empty transaction.
    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func submitAliasLifecyclePlan(
        _ request: AliasLifecyclePlanRequestV1,
        networkId: NetworkId,
        plan: AliasLifecycleTransactionPlanV1,
        bodyEncoder: (AliasLifecycleTransactionPlanBodyV1) throws -> Data,
        feePayment: FeePaymentIntent,
        ttlMs: UInt64? = 100_000,
        signingKey: SigningKey,
        frameCodec: (String, Data) throws -> DecodedAliasLifecycleFrame =
            NativeAliasNoritoRegistryCodec.shared.decodeAndReencodeLifecycle
    ) async throws -> Bool {
        guard let envelope = try buildAliasLifecyclePlan(
            request,
            networkId: networkId,
            plan: plan,
            bodyEncoder: bodyEncoder,
            feePayment: feePayment,
            ttlMs: ttlMs,
            signingKey: signingKey,
            frameCodec: frameCodec
        ) else { return false }
        try await submit(envelope: envelope)
        return true
    }

    /// Ed25519 convenience overload for lifecycle-plan submission.
    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func submitAliasLifecyclePlan(
        _ request: AliasLifecyclePlanRequestV1,
        networkId: NetworkId,
        plan: AliasLifecycleTransactionPlanV1,
        bodyEncoder: (AliasLifecycleTransactionPlanBodyV1) throws -> Data,
        feePayment: FeePaymentIntent,
        ttlMs: UInt64? = 100_000,
        keypair: Keypair,
        frameCodec: (String, Data) throws -> DecodedAliasLifecycleFrame =
            NativeAliasNoritoRegistryCodec.shared.decodeAndReencodeLifecycle
    ) async throws -> Bool {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try await submitAliasLifecyclePlan(
            request,
            networkId: networkId,
            plan: plan,
            bodyEncoder: bodyEncoder,
            feePayment: feePayment,
            ttlMs: ttlMs,
            signingKey: signingKey,
            frameCodec: frameCodec
        )
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

    public func submit(claimIdentifier request: ClaimIdentifierRequest,
                       keypair: Keypair,
                       completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildClaimIdentifier(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    public func submit(claimIdentifier request: ClaimIdentifierRequest,
                       signingKey: SigningKey,
                       completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildClaimIdentifier(request: request, signingKey: signingKey)
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

    @available(iOS 15.0, macOS 12.0, *)
    public func submit(claimIdentifier request: ClaimIdentifierRequest,
                       keypair: Keypair) async throws {
        let envelope = try buildClaimIdentifier(request: request, keypair: keypair)
        try await submit(envelope: envelope)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submit(claimIdentifier request: ClaimIdentifierRequest,
                       signingKey: SigningKey) async throws {
        let envelope = try buildClaimIdentifier(request: request, signingKey: signingKey)
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

    public func buildClaimIdentifier(request: ClaimIdentifierRequest,
                                     keypair: Keypair) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeClaimIdentifier(request: request,
                                                                 keypair: keypair,
                                                                 creationTimeMs: creationTimeMs)
    }

    public func buildClaimIdentifier(request: ClaimIdentifierRequest,
                                     signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let creationTimeMs = makeCreationTimeMs()
        return try SwiftTransactionEncoder.encodeClaimIdentifier(request: request,
                                                                 signingKey: signingKey,
                                                                 creationTimeMs: creationTimeMs)
    }

    public func buildCommitContractDeployment(request: CommitContractDeploymentRequest,
                                              keypair: Keypair) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeCommitContractDeployment(
            request: request,
            signingKey: .ed25519(privateKey: keypair.privateKeyBytes),
            creationTimeMs: makeCreationTimeMs()
        )
    }

    public func buildCommitContractDeployment(request: CommitContractDeploymentRequest,
                                              signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeCommitContractDeployment(
            request: request,
            signingKey: signingKey,
            creationTimeMs: makeCreationTimeMs()
        )
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

    /// Build a `SendToTwitter` instruction from a validated lossless quantity value.
    public func buildSendToTwitter(binding: SocialKeyedHash,
                                   amount: KotodamaQuantity) throws -> NoritoJSON {
        try SocialInstructionBuilders.sendToTwitter(binding: binding, amount: amount)
    }

    /// Convenience overload to build a `SendToTwitter` payload from pepper id and digest.
    public func buildSendToTwitter(pepperId: String, digest: String, amount: String) throws -> NoritoJSON {
        try SocialInstructionBuilders.sendToTwitter(pepperId: pepperId, digest: digest, amount: amount)
    }

    /// Convenience overload accepting a validated lossless quantity value.
    public func buildSendToTwitter(pepperId: String,
                                   digest: String,
                                   amount: KotodamaQuantity) throws -> NoritoJSON {
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
                _ = try await submitTransactionOnce(envelope: envelope,
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
        _ = try await submitTransactionOnce(envelope: envelope,
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

    public func getPipelinePreflight(completion: @Sendable @escaping (Result<ToriiPipelinePreflight, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getPipelinePreflight(completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getPipelinePreflight() async throws -> ToriiPipelinePreflight {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getPipelinePreflight()
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

    public func getNodeCapabilities(canonicalAuth: ToriiCanonicalRequestAuth,
                                    completion: @escaping (Result<ToriiNodeCapabilities, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getNodeCapabilities(canonicalAuth: canonicalAuth, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getNodeCapabilities(canonicalAuth: ToriiCanonicalRequestAuth) async throws -> ToriiNodeCapabilities {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getNodeCapabilities(canonicalAuth: canonicalAuth)
    }

    public func getRuntimeMetrics(canonicalAuth: ToriiCanonicalRequestAuth,
                                  completion: @escaping (Result<ToriiRuntimeMetrics, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getRuntimeMetrics(canonicalAuth: canonicalAuth, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getRuntimeMetrics(canonicalAuth: ToriiCanonicalRequestAuth) async throws -> ToriiRuntimeMetrics {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getRuntimeMetrics(canonicalAuth: canonicalAuth)
    }

    public func getRuntimeAbiActive(canonicalAuth: ToriiCanonicalRequestAuth,
                                    completion: @escaping (Result<ToriiRuntimeAbiActive, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getRuntimeAbiActive(canonicalAuth: canonicalAuth, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getRuntimeAbiActive(canonicalAuth: ToriiCanonicalRequestAuth) async throws -> ToriiRuntimeAbiActive {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getRuntimeAbiActive(canonicalAuth: canonicalAuth)
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
        let trimmedHex = seedHex?.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedBase64 = seedBase64?.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            let keyset = try ConfidentialKeyset.derive(seedHex: trimmedHex, seedBase64: trimmedBase64)
            completion(.success(keyset))
        } catch {
            completion(.failure(error))
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func deriveConfidentialKeyset(seedHex: String? = nil,
                                         seedBase64: String? = nil) async throws -> ConfidentialKeyset {
        let trimmedHex = seedHex?.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedBase64 = seedBase64?.trimmingCharacters(in: .whitespacesAndNewlines)
        return try ConfidentialKeyset.derive(seedHex: trimmedHex, seedBase64: trimmedBase64)
    }

    public func submitGovernanceDeployContractProposal(_ request: ToriiGovernanceDeployContractProposalRequest,
                                                       canonicalAuth: ToriiCanonicalRequestAuth,
                                                       completion: @escaping (Result<ToriiGovernanceProposalResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.submitGovernanceDeployContractProposal(
            request, canonicalAuth: canonicalAuth, completion: completion
        )
    }

    public func submitGovernancePlainBallot(_ request: ToriiGovernancePlainBallotRequest,
                                            canonicalAuth: ToriiCanonicalRequestAuth,
                                            completion: @escaping (Result<ToriiGovernanceBallotResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.submitGovernancePlainBallot(
            request,
            canonicalAuth: canonicalAuth,
            completion: completion
        )
    }

    public func submitGovernanceParliamentBallot(_ request: ToriiGovernanceParliamentBallotRequest,
                                                 canonicalAuth: ToriiCanonicalRequestAuth,
                                                 completion: @escaping (Result<ToriiGovernanceBallotResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.submitGovernanceParliamentBallot(
            request,
            canonicalAuth: canonicalAuth,
            completion: completion
        )
    }

    public func submitGovernanceZkBallotV1(_ request: ToriiGovernanceZkBallotV1Request,
                                           canonicalAuth: ToriiCanonicalRequestAuth,
                                           completion: @escaping (Result<ToriiGovernanceBallotResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.submitGovernanceZkBallotV1(
            request,
            canonicalAuth: canonicalAuth,
            completion: completion
        )
    }

    public func submitGovernanceZkBallotProofV1(_ request: ToriiGovernanceZkBallotProofRequest,
                                                canonicalAuth: ToriiCanonicalRequestAuth,
                                                completion: @escaping (Result<ToriiGovernanceBallotResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.submitGovernanceZkBallotProofV1(
            request,
            canonicalAuth: canonicalAuth,
            completion: completion
        )
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
                                        canonicalAuth: ToriiCanonicalRequestAuth,
                                        completion: @escaping (Result<ToriiGovernanceEnactResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.enactGovernanceProposal(
            request, canonicalAuth: canonicalAuth, completion: completion
        )
    }

    public func getGovernanceProposal(idHex: String,
                                      canonicalAuth: ToriiCanonicalRequestAuth,
                                      completion: @escaping (Result<ToriiGovernanceProposalGetResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getGovernanceProposal(
            idHex: idHex, canonicalAuth: canonicalAuth, completion: completion
        )
    }

    public func getGovernanceLocks(referendumId: String,
                                   canonicalAuth: ToriiCanonicalRequestAuth,
                                   completion: @escaping (Result<ToriiGovernanceLocksResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getGovernanceLocks(
            referendumId: referendumId, canonicalAuth: canonicalAuth,
            completion: completion
        )
    }

    public func getGovernanceReferendum(id: String,
                                        canonicalAuth: ToriiCanonicalRequestAuth,
                                        completion: @escaping (Result<ToriiGovernanceReferendumResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getGovernanceReferendum(
            id: id, canonicalAuth: canonicalAuth, completion: completion
        )
    }

    public func getGovernanceTally(id: String,
                                   canonicalAuth: ToriiCanonicalRequestAuth,
                                   completion: @escaping (Result<ToriiGovernanceTallyResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getGovernanceTally(
            id: id, canonicalAuth: canonicalAuth, completion: completion
        )
    }

    public func getGovernanceUnlockStats(height: UInt64? = nil,
                                         referendumId: String? = nil,
                                         canonicalAuth: ToriiCanonicalRequestAuth,
                                         completion: @escaping (Result<ToriiGovernanceUnlockStatsResponse, Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getGovernanceUnlockStats(height: height,
                                                 referendumId: referendumId,
                                                 canonicalAuth: canonicalAuth,
                                                 completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitGovernanceDeployContractProposal(
        _ request: ToriiGovernanceDeployContractProposalRequest,
        canonicalAuth: ToriiCanonicalRequestAuth
    ) async throws -> ToriiGovernanceProposalResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.submitGovernanceDeployContractProposal(
            request, canonicalAuth: canonicalAuth
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitGovernancePlainBallot(
        _ request: ToriiGovernancePlainBallotRequest,
        canonicalAuth: ToriiCanonicalRequestAuth
    ) async throws -> ToriiGovernanceBallotResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.submitGovernancePlainBallot(
            request,
            canonicalAuth: canonicalAuth
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitGovernanceParliamentBallot(
        _ request: ToriiGovernanceParliamentBallotRequest,
        canonicalAuth: ToriiCanonicalRequestAuth
    ) async throws -> ToriiGovernanceBallotResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.submitGovernanceParliamentBallot(
            request,
            canonicalAuth: canonicalAuth
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitGovernanceZkBallotV1(
        _ request: ToriiGovernanceZkBallotV1Request,
        canonicalAuth: ToriiCanonicalRequestAuth
    ) async throws -> ToriiGovernanceBallotResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.submitGovernanceZkBallotV1(
            request,
            canonicalAuth: canonicalAuth
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func submitGovernanceZkBallotProofV1(
        _ request: ToriiGovernanceZkBallotProofRequest,
        canonicalAuth: ToriiCanonicalRequestAuth
    ) async throws -> ToriiGovernanceBallotResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.submitGovernanceZkBallotProofV1(
            request,
            canonicalAuth: canonicalAuth
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func finalizeGovernanceReferendum(_ request: ToriiGovernanceFinalizeRequest) async throws -> ToriiGovernanceFinalizeResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.finalizeGovernanceReferendum(request)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func enactGovernanceProposal(
        _ request: ToriiGovernanceEnactRequest,
        canonicalAuth: ToriiCanonicalRequestAuth
    ) async throws -> ToriiGovernanceEnactResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.enactGovernanceProposal(
            request, canonicalAuth: canonicalAuth
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getGovernanceProposal(
        idHex: String, canonicalAuth: ToriiCanonicalRequestAuth
    ) async throws -> ToriiGovernanceProposalGetResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getGovernanceProposal(
            idHex: idHex, canonicalAuth: canonicalAuth
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getGovernanceLocks(
        referendumId: String, canonicalAuth: ToriiCanonicalRequestAuth
    ) async throws -> ToriiGovernanceLocksResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getGovernanceLocks(
            referendumId: referendumId, canonicalAuth: canonicalAuth
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getGovernanceReferendum(
        id: String, canonicalAuth: ToriiCanonicalRequestAuth
    ) async throws -> ToriiGovernanceReferendumResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getGovernanceReferendum(
            id: id, canonicalAuth: canonicalAuth
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getGovernanceTally(
        id: String, canonicalAuth: ToriiCanonicalRequestAuth
    ) async throws -> ToriiGovernanceTallyResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getGovernanceTally(
            id: id, canonicalAuth: canonicalAuth
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getGovernanceUnlockStats(height: UInt64? = nil,
                                         referendumId: String? = nil,
                                         canonicalAuth: ToriiCanonicalRequestAuth) async throws -> ToriiGovernanceUnlockStatsResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getGovernanceUnlockStats(
            height: height, referendumId: referendumId,
            canonicalAuth: canonicalAuth
        )
    }

    private func submitTransactionOnce(envelope: SignedTransactionEnvelope,
                                       options: PipelineSubmitOptions,
                                       mode: PipelineEndpointMode) async throws -> ToriiSubmitTransactionResponse? {
        let idempotencyKey = options.idempotencyKeyFactory?(envelope)
        return try await toriiClient.submitTransaction(data: envelope.norito,
                                                       mode: mode,
                                                       idempotencyKey: idempotencyKey)
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
        NoritoNativeBridge.shared.decodeSignedTransaction(envelope.norito)
    }

    public func getAssets(accountId: String,
                          limit: Int = 100,
                          asset: String? = nil,
                          scope: String? = nil,
                          completion: @Sendable @escaping (Result<[ToriiAssetBalance], Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getAssets(accountId: accountId, limit: limit, asset: asset, scope: scope, completion: completion)
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
                                     completion: @Sendable @escaping (Result<[ToriiExplorerTransferRecord], Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getExplorerTransfers(params: params,
                                             matchingAccount: accountId,
                                             assetDefinitionId: assetDefinitionId,
                                             completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getExplorerTransferSummaries(params: ToriiExplorerInstructionsParams? = nil,
                                             matchingAccount accountId: String? = nil,
                                             assetDefinitionId: String? = nil,
                                             relativeTo relativeAccountId: String? = nil,
                                             completion: @Sendable @escaping (Result<[ToriiExplorerTransferSummary], Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getExplorerTransferSummaries(params: params,
                                                     matchingAccount: accountId,
                                                     assetDefinitionId: assetDefinitionId,
                                                     relativeTo: relativeAccountId,
                                                     completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getExplorerTransactionTransfers(hashHex: String,
                                                matchingAccount accountId: String? = nil,
                                                assetDefinitionId: String? = nil,
                                                maxItems: UInt64? = nil,
                                                completion: @Sendable @escaping (Result<[ToriiExplorerTransferRecord], Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getExplorerTransactionTransfers(hashHex: hashHex,
                                                        matchingAccount: accountId,
                                                        assetDefinitionId: assetDefinitionId,
                                                        maxItems: maxItems,
                                                        completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getExplorerTransactionTransferSummaries(hashHex: String,
                                                        matchingAccount accountId: String? = nil,
                                                        assetDefinitionId: String? = nil,
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
                                                                relativeTo: relativeAccountId,
                                                                maxItems: maxItems,
                                                                completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getAccountTransferHistory(accountId: String,
                                          page: UInt64? = nil,
                                          perPage: UInt64? = nil,
                                          assetDefinitionId: String? = nil,
                                          completion: @Sendable @escaping (Result<[ToriiExplorerTransferSummary], Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getAccountTransferHistory(accountId: accountId,
                                                  page: page,
                                                  perPage: perPage,
                                                  assetDefinitionId: assetDefinitionId,
                                                  completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func getTransactionHistory(accountId: String,
                                      page: UInt64? = nil,
                                      perPage: UInt64? = nil,
                                      assetDefinitionId: String? = nil,
                                      completion: @Sendable @escaping (Result<[ToriiExplorerTransferSummary], Error>) -> Void) {
        guard let toriiRestClient else {
            completion(.failure(Self.restUnavailableError()))
            return
        }
        toriiRestClient.getTransactionHistory(accountId: accountId,
                                              page: page,
                                              perPage: perPage,
                                              assetDefinitionId: assetDefinitionId,
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
                let kind = status.status.kind
                if status.status.state == .applied {
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
                try await Task.sleep(
                    nanoseconds: StrictJSONNumber.saturatingNanoseconds(from: interval)
                )
            } else {
                await Task.yield()
            }
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
    func getAssets(accountId: String,
                   limit: Int = 100,
                   asset: String? = nil,
                   scope: String? = nil) async throws -> [ToriiAssetBalance] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getAssets(accountId: accountId, limit: limit, asset: asset, scope: scope)
    }

    func prepareDetachedAssetTransfer(
        _ request: ToriiAssetTransferRequest
    ) async throws -> ToriiAssetTransferDraft {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.prepareDetachedAssetTransfer(request)
    }

    func finalizeDetachedAssetTransfer(
        _ draft: ToriiAssetTransferDraft,
        publicKeyHex: String,
        signatureBase64: String
    ) throws -> ToriiDetachedAssetTransferSubmissionEvidence {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try toriiRestClient.finalizeDetachedAssetTransfer(
            draft,
            publicKeyHex: publicKeyHex,
            signatureBase64: signatureBase64
        )
    }

    func finalizeDetachedAssetTransfer(
        _ draft: ToriiAssetTransferDraft,
        signingKey: SigningKey
    ) throws -> ToriiDetachedAssetTransferSubmissionEvidence {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try toriiRestClient.finalizeDetachedAssetTransfer(
            draft,
            signingKey: signingKey
        )
    }

    func submitFinalizedDetachedAssetTransfer(
        _ evidence: ToriiDetachedAssetTransferSubmissionEvidence
    ) async throws -> ToriiAssetTransferResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.submitFinalizedDetachedAssetTransfer(evidence)
    }

    func reconcileDetachedAssetTransferSubmission(
        _ evidence: ToriiDetachedAssetTransferSubmissionEvidence,
        pollOptions: PipelineStatusPollOptions? = nil,
        mode: PipelineEndpointMode? = nil
    ) async throws -> ToriiPipelineTransactionStatus {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.reconcileDetachedAssetTransferSubmission(
            evidence,
            pollOptions: pollOptions ?? pipelinePollOptions,
            mode: mode ?? pipelineEndpointMode
        )
    }

    func recoverDetachedAssetTransferSubmission(
        _ evidence: ToriiDetachedAssetTransferSubmissionEvidence,
        pollOptions: PipelineStatusPollOptions? = nil,
        mode: PipelineEndpointMode? = nil
    ) async throws -> ToriiPipelineTransactionStatus {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.recoverDetachedAssetTransferSubmission(
            evidence,
            pollOptions: pollOptions ?? pipelinePollOptions,
            mode: mode ?? pipelineEndpointMode
        )
    }

    func submitDetachedAssetTransfer(
        _ draft: ToriiAssetTransferDraft,
        publicKeyHex: String,
        signatureBase64: String
    ) async throws -> ToriiAssetTransferResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.submitDetachedAssetTransfer(
            draft,
            publicKeyHex: publicKeyHex,
            signatureBase64: signatureBase64
        )
    }

    func submitDetachedAssetTransfer(
        _ draft: ToriiAssetTransferDraft,
        signingKey: SigningKey
    ) async throws -> ToriiAssetTransferResponse {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.submitDetachedAssetTransfer(
            draft,
            signingKey: signingKey
        )
    }

    func waitForDetachedAssetTransferFinality(
        _ draft: ToriiAssetTransferDraft,
        submittedResponse: ToriiAssetTransferResponse,
        pollOptions: PipelineStatusPollOptions? = nil,
        mode: PipelineEndpointMode? = nil
    ) async throws -> ToriiPipelineTransactionStatus {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.waitForDetachedAssetTransferFinality(
            draft,
            submittedResponse: submittedResponse,
            pollOptions: pollOptions ?? pipelinePollOptions,
            mode: mode ?? pipelineEndpointMode
        )
    }

    func iterateAccountTransferHistory(accountId: String,
                                       page: UInt64? = nil,
                                       perPage: UInt64? = nil,
                                       assetDefinitionId: String? = nil,
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
                                 assetDefinitionId: String? = nil) -> AsyncThrowingStream<ToriiExplorerTransferRecord, Error> {
        guard let toriiRestClient else {
            return AsyncThrowingStream { continuation in
                continuation.finish(throwing: Self.restUnavailableError())
            }
        }
        return toriiRestClient.streamExplorerTransfers(lastEventId: lastEventId,
                                                       matchingAccount: accountId,
                                                       assetDefinitionId: assetDefinitionId)
    }

    func streamExplorerTransferSummaries(lastEventId: String? = nil,
                                         matchingAccount accountId: String? = nil,
                                         assetDefinitionId: String? = nil,
                                         relativeTo relativeAccountId: String? = nil) -> AsyncThrowingStream<ToriiExplorerTransferSummary, Error> {
        guard let toriiRestClient else {
            return AsyncThrowingStream { continuation in
                continuation.finish(throwing: Self.restUnavailableError())
            }
        }
        return toriiRestClient.streamExplorerTransferSummaries(lastEventId: lastEventId,
                                                               matchingAccount: accountId,
                                                               assetDefinitionId: assetDefinitionId,
                                                               relativeTo: relativeAccountId)
    }

    func streamAccountTransferHistory(accountId: String,
                                      page: UInt64? = nil,
                                      perPage: UInt64? = nil,
                                      assetDefinitionId: String? = nil,
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
                                                            lastEventId: lastEventId,
                                                            maxItems: maxItems,
                                                            dedupeLimit: dedupeLimit)
    }

    func streamTransactionTransferSummaries(hashHex: String,
                                           matchingAccount accountId: String? = nil,
                                           assetDefinitionId: String? = nil,
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
                              assetDefinitionId: String? = nil) async throws -> [ToriiExplorerTransferRecord] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getExplorerTransfers(params: params,
                                                              matchingAccount: accountId,
                                                              assetDefinitionId: assetDefinitionId)
    }

    func getExplorerTransferSummaries(params: ToriiExplorerInstructionsParams? = nil,
                                      matchingAccount accountId: String? = nil,
                                      assetDefinitionId: String? = nil,
                                      relativeTo relativeAccountId: String? = nil) async throws -> [ToriiExplorerTransferSummary] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getExplorerTransferSummaries(params: params,
                                                                      matchingAccount: accountId,
                                                                      assetDefinitionId: assetDefinitionId,
                                                                      relativeTo: relativeAccountId)
    }

    func getExplorerTransactionTransfers(hashHex: String,
                                         matchingAccount accountId: String? = nil,
                                         assetDefinitionId: String? = nil,
                                         maxItems: UInt64? = nil) async throws -> [ToriiExplorerTransferRecord] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getExplorerTransactionTransfers(hashHex: hashHex,
                                                                         matchingAccount: accountId,
                                                                         assetDefinitionId: assetDefinitionId,
                                                                         maxItems: maxItems)
    }

    func getExplorerTransactionTransferSummaries(hashHex: String,
                                                 matchingAccount accountId: String? = nil,
                                                 assetDefinitionId: String? = nil,
                                                 relativeTo relativeAccountId: String? = nil,
                                                 maxItems: UInt64? = nil) async throws -> [ToriiExplorerTransferSummary] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getExplorerTransactionTransferSummaries(hashHex: hashHex,
                                                                                 matchingAccount: accountId,
                                                                                 assetDefinitionId: assetDefinitionId,
                                                                                 relativeTo: relativeAccountId,
                                                                                 maxItems: maxItems)
    }

    func getAccountTransferHistory(accountId: String,
                                   page: UInt64? = nil,
                                   perPage: UInt64? = nil,
                                   assetDefinitionId: String? = nil) async throws -> [ToriiExplorerTransferSummary] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getAccountTransferHistory(accountId: accountId,
                                                                   page: page,
                                                                   perPage: perPage,
                                                                   assetDefinitionId: assetDefinitionId)
    }

    func getTransactionHistory(accountId: String,
                               page: UInt64? = nil,
                               perPage: UInt64? = nil,
                               assetDefinitionId: String? = nil) async throws -> [ToriiExplorerTransferSummary] {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getTransactionHistory(accountId: accountId,
                                                               page: page,
                                                               perPage: perPage,
                                                               assetDefinitionId: assetDefinitionId)
    }

    func getTransactionStatus(hashHex: String) async throws -> ToriiPipelineTransactionStatus? {
        guard let toriiRestClient else {
            throw Self.restUnavailableError()
        }
        return try await toriiRestClient.getTransactionStatus(hashHex: hashHex)
    }
}
