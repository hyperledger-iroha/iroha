import Foundation
import CryptoKit

/// SCCP domain id for SORA/Nexus.
public let sccpDomainSora: UInt32 = 0

/// SCCP domain id for Solana.
public let sccpDomainSolana: UInt32 = 3

/// Proof backend id expected by the Solana SCCP recursive verifier.
public let sccpSolanaRecursiveProofBackendV1 = "sccp-solana-recursive-mainnet-v1"

/// OpenVerify circuit id for the Solana AccountsDB AccountsLtHash source-state proof.
public let sccpSolanaAccountsLtHashOpenVerifyCircuitIdV1 = "sccp-solana-accounts-lt-hash-v1"

/// OpenVerify circuit id for the Solana Tower replay audit role proof.
public let sccpSolanaTowerReplayOpenVerifyCircuitIdV1 = "sccp-solana-tower-replay-v1"

/// OpenVerify circuit id for the Solana full AccountsDB lattice audit role proof.
public let sccpSolanaFullAccountsdbLatticeOpenVerifyCircuitIdV1 =
    "sccp-solana-full-accountsdb-lattice-v1"

/// OpenVerify circuit id for the Solana bank/fork-choice audit role proof.
public let sccpSolanaBankForkChoiceOpenVerifyCircuitIdV1 =
    "sccp-solana-bank-fork-choice-v1"

/// Solana BPF upgradeable loader program id required for verifier ProgramData evidence.
public let sccpSolanaUpgradeableLoaderId = "BPFLoaderUpgradeab1e11111111111111111111111"

private let solanaSccpSourceStateVerificationCircuitIds: Set<String> = [
    sccpSolanaAccountsLtHashOpenVerifyCircuitIdV1,
    sccpSolanaTowerReplayOpenVerifyCircuitIdV1,
    sccpSolanaFullAccountsdbLatticeOpenVerifyCircuitIdV1,
    sccpSolanaBankForkChoiceOpenVerifyCircuitIdV1,
]

/// Solana mainnet-beta AccountsDB verifier profile id for SCCP source proofs.
public let sccpSolanaMainnetAccountsDbVerifierIdV1 =
    "sccp:sol:accounts-db-verifier:accounts-lt-hash-mainnet-beta:v1"

/// Maximum SCCP source-state proof capsule byte length accepted by Rust admission.
public let sccpSourceStateMaxProofBytes = 2 * 1024 * 1024

/// Maximum SCCP source-state proof family and circuit-id UTF-8 byte length.
public let sccpSourceStateMaxProofLabelBytes = 128

/// Maximum native recursive proof payload byte length accepted by Rust admission.
public let sccpNativeRecursiveMaxProofBytes = 2 * 1024 * 1024

/// Solana AccountsDB source-state verifier hash from the Rust template material.
public let sccpSolanaTemplateSourceStateVerifierHashV1 =
    "0x6b4e4106bbb6b343ae1a4a36c9c68756d4454d2167c9b8b2ee3225e39fb0a48b"

private let sccpSolanaTemplateSourceMaterialHashesV1: Set<String> = [
    "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3",
    "0x97ea89019e6c79305d06dfc27640ee14a6b42ba6eaf86e1835ee9b433dba48ba",
    "0xb8358bfef1e428a6a7e9115687cb2b88d9c21dad4021bea3e11d43489eb3dcb0",
    sccpSolanaTemplateSourceStateVerifierHashV1,
    "0x9df7ea90cf1bbba036788b14804f63f4be1e908390be89524fd4486f74344f56",
]

/// Solana mainnet genesis hash used to bind SCCP witness requests.
public let sccpSolanaMainnetGenesisHash = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp"

/// Solana mainnet-beta slots per epoch used by SCCP finality-context checks.
public let sccpSolanaMainnetSlotsPerEpoch: UInt64 = 432_000

/// Minimum rooted-slot distance bound into Solana SCCP Tower lockout evidence.
public let sccpSolanaTowerLockoutConfirmationDepth: UInt64 = 32

/// Number of active post-root votes Solana stores for a rooted Tower.
public let sccpSolanaTowerVoteStackDepth = sccpSolanaTowerLockoutConfirmationDepth - 1

/// Tower-era Solana stake warmup/cooldown rate bound into SCCP StakeHistory replay.
public let sccpSolanaTowerWarmupCooldownRateBps: UInt64 = 900

/// Maximum Solana validator roster entries accepted by SCCP UI/mobile proof helpers.
public let sccpSolanaMaxValidators = 8_192

/// Maximum Solana source Merkle branch siblings accepted by SCCP source adapters.
public let sccpMaxSourceMerkleBranchNodes = 64

/// Solana Vote program id required for SCCP validator vote account openings.
public let sccpSolanaVoteProgramId = Data([
    0x07, 0x61, 0x48, 0x1d, 0x35, 0x74, 0x74, 0xbb,
    0x7c, 0x4d, 0x76, 0x24, 0xeb, 0xd3, 0xbd, 0xb3,
    0xd8, 0x35, 0x5e, 0x73, 0xd1, 0x10, 0x43, 0xfc,
    0x0d, 0xa3, 0x53, 0x80, 0x00, 0x00, 0x00, 0x00,
])

/// Solana Stake program id required for SCCP validator stake account openings.
public let sccpSolanaStakeProgramId = Data([
    0x06, 0xa1, 0xd8, 0x17, 0x91, 0x37, 0x54, 0x2a,
    0x98, 0x34, 0x37, 0xbd, 0xfe, 0x2a, 0x7a, 0xb2,
    0x55, 0x7f, 0x53, 0x5c, 0x8a, 0x78, 0x72, 0x2b,
    0x68, 0xa4, 0x9d, 0xc0, 0x00, 0x00, 0x00, 0x00,
])

/// Solana sysvar owner program id required for SCCP sysvar openings.
public let sccpSolanaSysvarProgramId = Data([
    0x06, 0xa7, 0xd5, 0x17, 0x18, 0x75, 0xf7, 0x29,
    0xc7, 0x3d, 0x93, 0x40, 0x8f, 0x21, 0x61, 0x20,
    0x06, 0x7e, 0xd8, 0x8c, 0x76, 0xe0, 0x8c, 0x28,
    0x7f, 0xc1, 0x94, 0x60, 0x00, 0x00, 0x00, 0x00,
])

/// Solana StakeHistory sysvar account id required for SCCP sysvar openings.
public let sccpSolanaStakeHistorySysvarId = Data([
    0x06, 0xa7, 0xd5, 0x17, 0x19, 0x35, 0x84, 0xd0,
    0xfe, 0xed, 0x9b, 0xb3, 0x43, 0x1d, 0x13, 0x20,
    0x6b, 0xe5, 0x44, 0x28, 0x1b, 0x57, 0xb8, 0x56,
    0x6c, 0xc5, 0x37, 0x5f, 0xf4, 0x00, 0x00, 0x00,
])

/// Solana StakeHistory sysvar entry bound into SCCP stake-history evidence.
public struct SolanaSccpStakeHistoryEntry: Equatable {
    public let epoch: UInt64
    public let effective: UInt64
    public let activating: UInt64
    public let deactivating: UInt64

    public init(epoch: UInt64, effective: UInt64, activating: UInt64, deactivating: UInt64) {
        self.epoch = epoch
        self.effective = effective
        self.activating = activating
        self.deactivating = deactivating
    }
}

/// Parsed fields from a raw Solana `VoteStateVersions` account buffer.
public struct SolanaSccpParsedVoteStateAccountData: Equatable {
    public let nodePubkey: Data
    public let authorizedVoter: Data
    public let authorizedWithdrawer: Data
    public let inflationRewardsCollector: Data
    public let blockRevenueCollector: Data
    public let inflationRewardsCommissionBps: UInt16
    public let blockRevenueCommissionBps: UInt16
    public let pendingDelegatorRewards: UInt64
    public let blsPubkeyCompressed: Data
    public let rootSlot: UInt64
    public let towerVoteSlots: [UInt64]

    public init(
        nodePubkey: Data,
        authorizedVoter: Data,
        authorizedWithdrawer: Data,
        inflationRewardsCollector: Data,
        blockRevenueCollector: Data,
        inflationRewardsCommissionBps: UInt16,
        blockRevenueCommissionBps: UInt16,
        pendingDelegatorRewards: UInt64,
        blsPubkeyCompressed: Data,
        rootSlot: UInt64,
        towerVoteSlots: [UInt64]
    ) {
        self.nodePubkey = nodePubkey
        self.authorizedVoter = authorizedVoter
        self.authorizedWithdrawer = authorizedWithdrawer
        self.inflationRewardsCollector = inflationRewardsCollector
        self.blockRevenueCollector = blockRevenueCollector
        self.inflationRewardsCommissionBps = inflationRewardsCommissionBps
        self.blockRevenueCommissionBps = blockRevenueCommissionBps
        self.pendingDelegatorRewards = pendingDelegatorRewards
        self.blsPubkeyCompressed = blsPubkeyCompressed
        self.rootSlot = rootSlot
        self.towerVoteSlots = towerVoteSlots
    }
}

public typealias SolanaSccpParsedVoteStateV1OrV3AccountData = SolanaSccpParsedVoteStateAccountData

/// Parsed fields from a raw Solana `StakeStateV2::Stake` account buffer.
public struct SolanaSccpParsedStakeStateV2StakeAccountData: Equatable {
    public let staker: Data
    public let withdrawer: Data
    public let voterPubkey: Data
    public let delegatedStake: UInt64
    public let activationEpoch: UInt64
    public let deactivationEpoch: UInt64
    public let warmupCooldownRateBytes: Data
    public let creditsObserved: UInt64
    public let stakeFlags: UInt8

    public init(
        staker: Data,
        withdrawer: Data,
        voterPubkey: Data,
        delegatedStake: UInt64,
        activationEpoch: UInt64,
        deactivationEpoch: UInt64,
        warmupCooldownRateBytes: Data,
        creditsObserved: UInt64,
        stakeFlags: UInt8
    ) {
        self.staker = staker
        self.withdrawer = withdrawer
        self.voterPubkey = voterPubkey
        self.delegatedStake = delegatedStake
        self.activationEpoch = activationEpoch
        self.deactivationEpoch = deactivationEpoch
        self.warmupCooldownRateBytes = warmupCooldownRateBytes
        self.creditsObserved = creditsObserved
        self.stakeFlags = stakeFlags
    }
}

/// Solana account opening metadata used by mobile proof-generation helpers.
public struct SolanaSccpAccountOpeningInput: Equatable {
    public let address: Data
    public let owner: Data
    public let lamports: UInt64
    public let rentEpoch: UInt64
    public let executable: Bool
    public let dataHash: String

    public init(
        address: Data,
        owner: Data,
        lamports: UInt64,
        rentEpoch: UInt64,
        executable: Bool = false,
        dataHash: String
    ) {
        self.address = address
        self.owner = owner
        self.lamports = lamports
        self.rentEpoch = rentEpoch
        self.executable = executable
        self.dataHash = dataHash
    }
}

/// Opened Solana accounts used to build the exact account-inclusion witness.
public struct SolanaSccpOpenedAccountInclusionWitnessInput: Equatable {
    public let finalizedSlot: UInt64
    public let validatorVoteAccountOpenings: [SolanaSccpAccountOpeningInput]
    public let validatorVoteAccountRawData: [Data]
    public let validatorStakeAccountOpenings: [SolanaSccpAccountOpeningInput]
    public let validatorStakeAccountRawData: [Data]
    public let stakeHistorySysvarOpening: SolanaSccpAccountOpeningInput
    public let stakeHistorySysvarRawData: Data
    public let expectedAccountInclusionRoot: String?

    public init(
        finalizedSlot: UInt64,
        validatorVoteAccountOpenings: [SolanaSccpAccountOpeningInput] = [],
        validatorVoteAccountRawData: [Data] = [],
        validatorStakeAccountOpenings: [SolanaSccpAccountOpeningInput] = [],
        validatorStakeAccountRawData: [Data] = [],
        stakeHistorySysvarOpening: SolanaSccpAccountOpeningInput,
        stakeHistorySysvarRawData: Data,
        expectedAccountInclusionRoot: String? = nil
    ) {
        self.finalizedSlot = finalizedSlot
        self.validatorVoteAccountOpenings = validatorVoteAccountOpenings
        self.validatorVoteAccountRawData = validatorVoteAccountRawData
        self.validatorStakeAccountOpenings = validatorStakeAccountOpenings
        self.validatorStakeAccountRawData = validatorStakeAccountRawData
        self.stakeHistorySysvarOpening = stakeHistorySysvarOpening
        self.stakeHistorySysvarRawData = stakeHistorySysvarRawData
        self.expectedAccountInclusionRoot = expectedAccountInclusionRoot
    }
}

/// Exact opened-account inclusion root and branches accepted by the Solana verifier.
public struct SolanaSccpOpenedAccountInclusionWitness: Equatable {
    public let root: String
    public let branches: [[String]]
    public let validatorVoteAccountBranches: [[String]]
    public let validatorStakeAccountBranches: [[String]]
    public let stakeHistorySysvarBranch: [String]
}

/// Opened Solana AccountsLtHash rows supplied by a native/mobile source-state prover.
public struct SolanaSccpOpenedAccountsLtHashContributionsInput: Equatable {
    public let sourceDomain: UInt32
    public let finalizedSlot: UInt64
    public let accountInclusionRoot: String
    public let accountsLtHashChecksum: String
    public let accountsLtHash: Data
    public let validatorVoteAccountOpenings: [SolanaSccpAccountOpeningInput]
    public let validatorVoteAccountRawData: [Data]
    public let validatorVoteAccountLtHashes: [Data]
    public let validatorStakeAccountOpenings: [SolanaSccpAccountOpeningInput]
    public let validatorStakeAccountRawData: [Data]
    public let validatorStakeAccountLtHashes: [Data]
    public let stakeHistorySysvarOpening: SolanaSccpAccountOpeningInput
    public let stakeHistorySysvarRawData: Data
    public let stakeHistorySysvarAccountLtHash: Data

    public init(
        sourceDomain: UInt32 = sccpDomainSolana,
        finalizedSlot: UInt64,
        accountInclusionRoot: String,
        accountsLtHashChecksum: String,
        accountsLtHash: Data,
        validatorVoteAccountOpenings: [SolanaSccpAccountOpeningInput] = [],
        validatorVoteAccountRawData: [Data] = [],
        validatorVoteAccountLtHashes: [Data] = [],
        validatorStakeAccountOpenings: [SolanaSccpAccountOpeningInput] = [],
        validatorStakeAccountRawData: [Data] = [],
        validatorStakeAccountLtHashes: [Data] = [],
        stakeHistorySysvarOpening: SolanaSccpAccountOpeningInput,
        stakeHistorySysvarRawData: Data,
        stakeHistorySysvarAccountLtHash: Data = Data()
    ) {
        self.sourceDomain = sourceDomain
        self.finalizedSlot = finalizedSlot
        self.accountInclusionRoot = accountInclusionRoot
        self.accountsLtHashChecksum = accountsLtHashChecksum
        self.accountsLtHash = accountsLtHash
        self.validatorVoteAccountOpenings = validatorVoteAccountOpenings
        self.validatorVoteAccountRawData = validatorVoteAccountRawData
        self.validatorVoteAccountLtHashes = validatorVoteAccountLtHashes
        self.validatorStakeAccountOpenings = validatorStakeAccountOpenings
        self.validatorStakeAccountRawData = validatorStakeAccountRawData
        self.validatorStakeAccountLtHashes = validatorStakeAccountLtHashes
        self.stakeHistorySysvarOpening = stakeHistorySysvarOpening
        self.stakeHistorySysvarRawData = stakeHistorySysvarRawData
        self.stakeHistorySysvarAccountLtHash = stakeHistorySysvarAccountLtHash
    }
}

/// FastPQ public inputs bound to a Solana AccountsLtHash source-state proof request.
public struct SolanaSccpAccountsLtHashFastpqPublicInputs: Equatable {
    public let dsid: String
    public let slot: String
    public let oldRoot: String
    public let newRoot: String
    public let permRoot: String
    public let txSetHash: String
}

/// One FastPQ transition supplied to a Solana AccountsLtHash source-state prover.
public struct SolanaSccpAccountsLtHashFastpqTransition: Equatable {
    public let key: String
    public let operation: String
    public let oldValue: Data
    public let newValue: Data
}

/// Source-state proof request for the nested Solana AccountsLtHash proof.
public struct SolanaSccpAccountsLtHashProofRequest: Equatable {
    public let version: UInt8
    public let proofFamily: String
    public let circuitId: String
    public let parameterSet: String
    public let sourceDomain: UInt32
    public let finalizedSlot: String
    public let parentSlot: String
    public let sourceStateVerifierId: String
    public let sourceStateVerifierHash: String
    public let accountsLtHashProofPublicInputsHash: String
    public let openedAccountsLtHashContributionsHash: String
    public let openedAccountsLtHashResidualChecksum: String
    public let statementBytes: Data
    public let accountCommitmentBytes: Data
    public let verificationContextBytes: Data
    public let schemaDescriptor: Data
    public let publicInputColumns: [[String]]
    public let fastpqPublicInputs: SolanaSccpAccountsLtHashFastpqPublicInputs
    public let fastpqTransitions: [SolanaSccpAccountsLtHashFastpqTransition]
}

/// Source-state verification proof capsule generated by a user-side prover.
public struct SolanaSccpSourceStateVerificationProof: Equatable {
    public let version: UInt8
    public let proofFamily: String
    public let circuitId: String
    public let proofBytes: Data
    public var proofBase64: String {
        proofBytes.base64EncodedString()
    }

    public init(
        version: UInt8 = 1,
        proofFamily: String = sccpStarkFriProofFamilyV1,
        circuitId: String,
        proofBytes: Data
    ) {
        self.version = version
        self.proofFamily = proofFamily
        self.circuitId = circuitId
        self.proofBytes = proofBytes
    }
}

/// Solana full light-client audit role proven by a user-side prover.
public enum SolanaSccpFullLightClientAuditRole: Equatable {
    case towerReplay
    case fullAccountsdbLattice
    case bankForkChoice
}

/// Input required to build Solana full light-client audit proof requests on iOS clients.
public struct SolanaSccpFullLightClientAuditProofInput: Equatable {
    public let witness: SolanaSccpWitnessInput
    public let openedAccounts: SolanaSccpOpenedAccountsLtHashContributionsInput
    public let accountsLtHashProof: SolanaSccpSourceStateVerificationProof
    public let rootedSlot: UInt64
    public let towerVoteSlots: [UInt64]
    public let epoch: UInt64?
    public let epochStakeRoot: String
    public let stakeActivationHash: String
    public let stakeAccountStateHash: String
    public let stakeHistoryHash: String
    public let stakeHistorySysvarAccountHash: String
    public let sourceTrustAnchorHash: String
    public let consensusVerifierHash: String
    public let messageInclusionVerifierHash: String
    public let finalityPolicyHash: String
    public let sourceAdapterDeploymentReceiptHash: String
    public let adapterVerifierVkHash: String?
    public let solanaTowerReplayVerifierHash: String
    public let solanaFullAccountsdbLatticeVerifierHash: String
    public let solanaBankForkChoiceVerifierHash: String
    public let sourceVerifierMaterialHash: String?
    public let sourceAdapterDeploymentHash: String?
    public let fullLightClientGateHash: String?
    public let finalityContextHash: String?
    public let voteMessageHash: String?
    public let accountsLtHashProofHash: String?
    public let openedAccountsLtHashContributionsHash: String?
    public let openedAccountsLtHashResidualChecksum: String?
    public let towerLockoutHash: String?
    public let towerReplayHash: String?
    public let bankForkHash: String?

    public init(
        witness: SolanaSccpWitnessInput,
        openedAccounts: SolanaSccpOpenedAccountsLtHashContributionsInput,
        accountsLtHashProof: SolanaSccpSourceStateVerificationProof,
        rootedSlot: UInt64,
        towerVoteSlots: [UInt64],
        epoch: UInt64? = nil,
        epochStakeRoot: String,
        stakeActivationHash: String,
        stakeAccountStateHash: String,
        stakeHistoryHash: String,
        stakeHistorySysvarAccountHash: String,
        sourceTrustAnchorHash: String,
        consensusVerifierHash: String,
        messageInclusionVerifierHash: String,
        finalityPolicyHash: String,
        sourceAdapterDeploymentReceiptHash: String,
        adapterVerifierVkHash: String? = nil,
        solanaTowerReplayVerifierHash: String,
        solanaFullAccountsdbLatticeVerifierHash: String,
        solanaBankForkChoiceVerifierHash: String,
        sourceVerifierMaterialHash: String? = nil,
        sourceAdapterDeploymentHash: String? = nil,
        fullLightClientGateHash: String? = nil,
        finalityContextHash: String? = nil,
        voteMessageHash: String? = nil,
        accountsLtHashProofHash: String? = nil,
        openedAccountsLtHashContributionsHash: String? = nil,
        openedAccountsLtHashResidualChecksum: String? = nil,
        towerLockoutHash: String? = nil,
        towerReplayHash: String? = nil,
        bankForkHash: String? = nil
    ) {
        self.witness = witness
        self.openedAccounts = openedAccounts
        self.accountsLtHashProof = accountsLtHashProof
        self.rootedSlot = rootedSlot
        self.towerVoteSlots = towerVoteSlots
        self.epoch = epoch
        self.epochStakeRoot = epochStakeRoot
        self.stakeActivationHash = stakeActivationHash
        self.stakeAccountStateHash = stakeAccountStateHash
        self.stakeHistoryHash = stakeHistoryHash
        self.stakeHistorySysvarAccountHash = stakeHistorySysvarAccountHash
        self.sourceTrustAnchorHash = sourceTrustAnchorHash
        self.consensusVerifierHash = consensusVerifierHash
        self.messageInclusionVerifierHash = messageInclusionVerifierHash
        self.finalityPolicyHash = finalityPolicyHash
        self.sourceAdapterDeploymentReceiptHash = sourceAdapterDeploymentReceiptHash
        self.adapterVerifierVkHash = adapterVerifierVkHash
        self.solanaTowerReplayVerifierHash = solanaTowerReplayVerifierHash
        self.solanaFullAccountsdbLatticeVerifierHash = solanaFullAccountsdbLatticeVerifierHash
        self.solanaBankForkChoiceVerifierHash = solanaBankForkChoiceVerifierHash
        self.sourceVerifierMaterialHash = sourceVerifierMaterialHash
        self.sourceAdapterDeploymentHash = sourceAdapterDeploymentHash
        self.fullLightClientGateHash = fullLightClientGateHash
        self.finalityContextHash = finalityContextHash
        self.voteMessageHash = voteMessageHash
        self.accountsLtHashProofHash = accountsLtHashProofHash
        self.openedAccountsLtHashContributionsHash = openedAccountsLtHashContributionsHash
        self.openedAccountsLtHashResidualChecksum = openedAccountsLtHashResidualChecksum
        self.towerLockoutHash = towerLockoutHash
        self.towerReplayHash = towerReplayHash
        self.bankForkHash = bankForkHash
    }
}

/// FastPQ public inputs bound to a Solana full light-client audit role proof request.
public struct SolanaSccpFullLightClientAuditFastpqPublicInputs: Equatable {
    public let dsid: String
    public let slot: String
    public let oldRoot: String
    public let newRoot: String
    public let permRoot: String
    public let txSetHash: String
}

/// One FastPQ transition supplied to a Solana full light-client audit role prover.
public struct SolanaSccpFullLightClientAuditFastpqTransition: Equatable {
    public let key: String
    public let operation: String
    public let oldValue: Data
    public let newValue: Data
}

/// OpenVerify request for one Solana full light-client audit role proof.
public struct SolanaSccpFullLightClientAuditProofRequest: Equatable {
    public let version: UInt8
    public let proofFamily: String
    public let circuitId: String
    public let parameterSet: String
    public let role: String
    public let roleCode: UInt8
    public let sourceDomain: UInt32
    public let finalizedSlot: String
    public let verifierId: String
    public let verifierHash: String
    public let sourceStateVerifierId: String
    public let sourceStateVerifierHash: String
    public let sourceVerifierMaterialHash: String
    public let sourceAdapterDeploymentHash: String
    public let fullLightClientGateHash: String
    public let finalityContextHash: String
    public let voteMessageHash: String
    public let accountsLtHashProofHash: String
    public let auditStatementHash: String
    public let statementBytes: Data
    public let verificationContextBytes: Data
    public let schemaDescriptor: Data
    public let publicInputColumns: [[String]]
    public let fastpqPublicInputs: SolanaSccpFullLightClientAuditFastpqPublicInputs
    public let fastpqTransitions: [SolanaSccpFullLightClientAuditFastpqTransition]
}

/// Role-separated Solana full light-client audit proof requests.
public struct SolanaSccpFullLightClientAuditProofRequests: Equatable {
    public let towerReplay: SolanaSccpFullLightClientAuditProofRequest
    public let fullAccountsdbLattice: SolanaSccpFullLightClientAuditProofRequest
    public let bankForkChoice: SolanaSccpFullLightClientAuditProofRequest
}

private let sccpSolanaTransactionSignatureBytes = 64
private let sccpSolanaProgramIdBytes = 32
private let sccpSolanaBasisPointsPerUnit: UInt64 = 10_000
private let sccpSolanaStakeStateV2StakeAccountDataLen = 200
private let sccpSolanaVoteStateAccountDataLen = 3_762
private let sccpSolanaMaxAccountRawDataBytes = 65_536
private let sccpSolanaAccountsLtHashBytes = 2_048
private let sccpSolanaMaxBankHardForkHashDataBytes = 1_024
private let sccpSolanaBlsPublicKeyCompressedLen = 48
private let sccpSolanaVoteStateV1_14_11Discriminant: UInt32 = 1
private let sccpSolanaVoteStateV3Discriminant: UInt32 = 2
private let sccpSolanaVoteStateV4Discriminant: UInt32 = 3
private let sccpSolanaVoteStatePriorVoters = 32
private let sccpSolanaVoteStateV4AuthorizedVoters = 4
private let sccpSolanaVoteStateMaxEpochCredits = 64
private let sccpSolanaStakeStateV2StakeDiscriminant: UInt32 = 2
private let sccpSolanaStakeStateV2StakerOffset = 12
private let sccpSolanaStakeStateV2WithdrawerOffset = 44
private let sccpSolanaStakeStateV2VoterPubkeyOffset = 124
private let sccpSolanaStakeStateV2DelegatedStakeOffset = 156
private let sccpSolanaStakeStateV2ActivationEpochOffset = 164
private let sccpSolanaStakeStateV2DeactivationEpochOffset = 172
private let sccpSolanaStakeStateV2WarmupCooldownRateOffset = 180
private let sccpSolanaStakeStateV2WarmupCooldownRateBytes = 8
private let sccpSolanaStakeStateV2LegacyWarmupCooldownRateBytes = Data(
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd0, 0x3f]
)
private let sccpSolanaStakeStateV2CurrentWarmupCooldownRateBytes = Data(
    [0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f]
)
private let sccpSolanaStakeStateV2CreditsObservedOffset = 188
private let sccpSolanaStakeStateV2FlagOffset = 196
private let sccpSolanaStakeStateV2KnownFlagsMask: UInt8 = 0b0000_0001
private let solanaBase58Alphabet = Array("123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".utf8)
private let solanaBase58Index = Dictionary(
    uniqueKeysWithValues: solanaBase58Alphabet.enumerated().map { ($0.element, UInt32($0.offset)) }
)

/// Solana program-instruction envelope encoding used by SCCP verifier submissions.
public let sccpSolanaBorshInstructionV1 = "borsh_instruction_v1"

/// Canonical zero hash used when deployment-bound source adapter material is absent.
public let sccpZeroHashV1 = "0x0000000000000000000000000000000000000000000000000000000000000000"

private let sccpSolanaSubmitMessageProofEntrypointV1 = "submit_sccp_message_proof"
private let sccpSolanaAccountsLtHashProofPublicInputsPrefixV1 =
    "sccp:solana:accounts-lt-proof-public-inputs:v1"
private let sccpSolanaAccountsLtHashOpenedContributionsPrefixV1 =
    "sccp:solana:accounts-lt-opened-contributions:v1"
private let sccpSolanaMainnetGenesisHashPrefixV1 =
    "sccp:solana:mainnet-genesis:v1"
private let sccpSolanaBankHashHardForkDataPrefixV1 =
    "sccp:solana:bank-hash-hard-fork-data:v1"
private let sccpSolanaAccountsLtHashFastpqDsidPrefixV1 =
    "sccp:solana:accounts-lt:fastpq:dsid:v1"
private let sccpSolanaAccountsLtHashFastpqParameterSetV1 = "fastpq-lane-balanced"
private let sccpSolanaAccountsLtHashFastpqStatementKeyV1 =
    "sccp:solana:accounts-lt:v1:statement"
private let sccpSolanaAccountsLtHashFastpqAccountsKeyV1 =
    "sccp:solana:accounts-lt:v1:accounts"
private let sccpSolanaAccountsLtHashFastpqOpenedContributionsKeyV1 =
    "sccp:solana:accounts-lt:v1:opened-contributions"
private let sccpSolanaAccountsLtHashFastpqResidualKeyV1 =
    "sccp:solana:accounts-lt:v1:residual"
private let sccpSolanaAccountsLtHashFastpqContextKeyV1 =
    "sccp:solana:accounts-lt:v1:context"
private let sccpSolanaFullLightClientAuditFastpqDsidPrefixV1 =
    "sccp:solana:full-light-client-audit:fastpq:dsid:v1"
private let sccpSolanaFullLightClientAuditFastpqParameterSetV1 =
    "fastpq-lane-balanced"
private let sccpSolanaFullLightClientAuditFastpqStatementKeyV1 =
    "sccp:solana:full-light-client-audit:v1:statement"
private let sccpSolanaFullLightClientAuditFastpqContextKeyV1 =
    "sccp:solana:full-light-client-audit:v1:context"
private let sccpSolanaFullLightClientAuditFastpqGateKeyV1 =
    "sccp:solana:full-light-client-audit:v1:gate"
private let sccpSolanaFullLightClientAuditStatementPrefixV1 =
    "sccp:solana:full-light-client-audit:statement:v1"
private let sccpSolanaSourceChainKeyV1 = "sol"
private let sccpSolanaSourceProofPlanCodeV1: UInt8 = 3
private let sccpSolanaFinalityModelCodeV1: UInt8 = 3
private let sccpSolanaOpenedLtHashRoleVote: UInt8 = 1
private let sccpSolanaOpenedLtHashRoleStake: UInt8 = 2
private let sccpSolanaOpenedLtHashRoleStakeHistorySysvar: UInt8 = 3
private let sccpSolanaUpgradeableLoaderProgramTag: UInt32 = 2
private let sccpSolanaUpgradeableLoaderProgramdataTag: UInt32 = 3
private let sccpSolanaProgramdataMetadataLength = 45
private let sccpSolanaBpfElfMagic = Data([0x7f, 0x45, 0x4c, 0x46])
private let sccpSolanaRouteCanaryLiveProgramPrefixV1 =
    "iroha:sccp:solana-route-canary-live-program:v1"

/// Solana destination ProgramData evidence collected by UI code before route canary submission.
public struct SolanaSccpRouteCanaryEvidenceInput: Equatable {
    public let routeAllowlistHash: String
    public let destinationBindingHash: String
    public let expectedDestinationBindingHash: String?
    public let sourceVerifierMaterialHash: String
    public let sourceAdapterEngineDeploymentHash: String
    public let verifierIdentity: String
    public let verifierCodeHash: String
    public let solanaRpcCommitment: String
    public let solanaProgramOwner: String
    public let solanaProgramdataOwner: String
    public let solanaProgramImmutable: Bool
    public let solanaProgramAccountDataBase64: String
    public let solanaProgramdataAddress: String
    public let solanaProgramdataSlot: String
    public let solanaExpectedProgramdataSlot: String
    public let solanaProgramAccountContextSlot: String
    public let solanaProgramdataAccountContextSlot: String
    public let solanaProgramdataMetadataBlake2b256: String
    public let solanaProgramdataMetadataBase64: String
    public let solanaProgramdataExecutableBlake2b256: String
    public let solanaProgramdataExecutableBase64: String

    public init(
        routeAllowlistHash: String,
        destinationBindingHash: String,
        expectedDestinationBindingHash: String? = nil,
        sourceVerifierMaterialHash: String,
        sourceAdapterEngineDeploymentHash: String,
        verifierIdentity: String,
        verifierCodeHash: String,
        solanaRpcCommitment: String = "finalized",
        solanaProgramOwner: String = sccpSolanaUpgradeableLoaderId,
        solanaProgramdataOwner: String = sccpSolanaUpgradeableLoaderId,
        solanaProgramImmutable: Bool = true,
        solanaProgramAccountDataBase64: String,
        solanaProgramdataAddress: String,
        solanaProgramdataSlot: String,
        solanaExpectedProgramdataSlot: String,
        solanaProgramAccountContextSlot: String,
        solanaProgramdataAccountContextSlot: String,
        solanaProgramdataMetadataBlake2b256: String,
        solanaProgramdataMetadataBase64: String,
        solanaProgramdataExecutableBlake2b256: String,
        solanaProgramdataExecutableBase64: String
    ) {
        self.routeAllowlistHash = routeAllowlistHash
        self.destinationBindingHash = destinationBindingHash
        self.expectedDestinationBindingHash = expectedDestinationBindingHash
        self.sourceVerifierMaterialHash = sourceVerifierMaterialHash
        self.sourceAdapterEngineDeploymentHash = sourceAdapterEngineDeploymentHash
        self.verifierIdentity = verifierIdentity
        self.verifierCodeHash = verifierCodeHash
        self.solanaRpcCommitment = solanaRpcCommitment
        self.solanaProgramOwner = solanaProgramOwner
        self.solanaProgramdataOwner = solanaProgramdataOwner
        self.solanaProgramImmutable = solanaProgramImmutable
        self.solanaProgramAccountDataBase64 = solanaProgramAccountDataBase64
        self.solanaProgramdataAddress = solanaProgramdataAddress
        self.solanaProgramdataSlot = solanaProgramdataSlot
        self.solanaExpectedProgramdataSlot = solanaExpectedProgramdataSlot
        self.solanaProgramAccountContextSlot = solanaProgramAccountContextSlot
        self.solanaProgramdataAccountContextSlot = solanaProgramdataAccountContextSlot
        self.solanaProgramdataMetadataBlake2b256 = solanaProgramdataMetadataBlake2b256
        self.solanaProgramdataMetadataBase64 = solanaProgramdataMetadataBase64
        self.solanaProgramdataExecutableBlake2b256 = solanaProgramdataExecutableBlake2b256
        self.solanaProgramdataExecutableBase64 = solanaProgramdataExecutableBase64
    }
}

/// Raw Solana SCCP witness data collected by UI code before local proof generation.
public struct SolanaSccpWitnessInput: Equatable {
    public let targetDomain: UInt32
    public let mainnetGenesisHash: String
    public let finalizedSlot: UInt64
    public let parentSlot: UInt64
    public let bankSignatureCount: UInt64
    public let parentBankHash: String
    public let blockhash: String
    public let bankHash: String
    public let transactionStatusRoot: String
    public let messageProofHash: String
    public let accountInclusionRoot: String
    public let accountsLtHashChecksum: String
    public let accountsLtHashProofPublicInputsHash: String?
    public let bankHashHardForkData: Data
    public let accountsLtHash: Data?
    public let transactionSignature: String
    public let emitterProgramId: String
    public let messageId: String
    public let payloadHash: String
    public let commitmentRoot: String
    public let sourceEventDigest: String
    public let sourceStateVerifierId: String
    public let sourceStateVerifierHash: String
    public let statementHash: String
    public let destinationBindingHash: String
    public let inclusionBranch: [Data]
    public let sourceAdapterDeploymentHash: String
    public let sourceAdapterDeploymentReceiptHash: String

    public init(
        targetDomain: UInt32 = sccpDomainSora,
        mainnetGenesisHash: String = sccpSolanaMainnetGenesisHash,
        finalizedSlot: UInt64,
        parentSlot: UInt64,
        bankSignatureCount: UInt64,
        parentBankHash: String,
        blockhash: String,
        bankHash: String,
        transactionStatusRoot: String,
        messageProofHash: String,
        accountInclusionRoot: String,
        accountsLtHashChecksum: String,
        accountsLtHashProofPublicInputsHash: String? = nil,
        bankHashHardForkData: Data = Data(),
        accountsLtHash: Data? = nil,
        transactionSignature: String,
        emitterProgramId: String,
        messageId: String,
        payloadHash: String,
        commitmentRoot: String,
        sourceEventDigest: String,
        sourceStateVerifierId: String = sccpSolanaMainnetAccountsDbVerifierIdV1,
        sourceStateVerifierHash: String = sccpZeroHashV1,
        statementHash: String,
        destinationBindingHash: String,
        inclusionBranch: [Data] = [],
        sourceAdapterDeploymentHash: String = sccpZeroHashV1,
        sourceAdapterDeploymentReceiptHash: String = sccpZeroHashV1
    ) {
        self.targetDomain = targetDomain
        self.mainnetGenesisHash = mainnetGenesisHash
        self.finalizedSlot = finalizedSlot
        self.parentSlot = parentSlot
        self.bankSignatureCount = bankSignatureCount
        self.parentBankHash = parentBankHash
        self.blockhash = blockhash
        self.bankHash = bankHash
        self.transactionStatusRoot = transactionStatusRoot
        self.messageProofHash = messageProofHash
        self.accountInclusionRoot = accountInclusionRoot
        self.accountsLtHashChecksum = accountsLtHashChecksum
        self.accountsLtHashProofPublicInputsHash = accountsLtHashProofPublicInputsHash
        self.bankHashHardForkData = bankHashHardForkData
        self.accountsLtHash = accountsLtHash
        self.transactionSignature = transactionSignature
        self.emitterProgramId = emitterProgramId
        self.messageId = messageId
        self.payloadHash = payloadHash
        self.commitmentRoot = commitmentRoot
        self.sourceEventDigest = sourceEventDigest
        self.sourceStateVerifierId = sourceStateVerifierId
        self.sourceStateVerifierHash = sourceStateVerifierHash
        self.statementHash = statementHash
        self.destinationBindingHash = destinationBindingHash
        self.inclusionBranch = inclusionBranch
        self.sourceAdapterDeploymentHash = sourceAdapterDeploymentHash
        self.sourceAdapterDeploymentReceiptHash = sourceAdapterDeploymentReceiptHash
    }
}

/// Canonical Solana SCCP witness used as prover input.
public struct SolanaSccpWitness: Equatable {
    public let version: UInt8
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let mainnetGenesisHash: String
    public let finalizedSlot: UInt64
    public let parentSlot: UInt64
    public let bankSignatureCount: UInt64
    public let parentBankHash: String
    public let blockhash: String
    public let bankHash: String
    public let transactionStatusRoot: String
    public let messageProofHash: String
    public let accountInclusionRoot: String
    public let accountsLtHashChecksum: String
    public let accountsLtHashProofPublicInputsHash: String
    public let bankHashHardForkData: Data
    public let accountsLtHash: Data?
    public let transactionSignature: String
    public let emitterProgramId: String
    public let messageId: String
    public let payloadHash: String
    public let commitmentRoot: String
    public let sourceEventDigest: String
    public let sourceStateVerifierId: String
    public let sourceStateVerifierHash: String
    public let sourceAdapterDeploymentHash: String
    public let sourceAdapterDeploymentReceiptHash: String
    public let inclusionBranch: [Data]
}

/// Public inputs exposed by a Solana SCCP proof request.
public struct SolanaSccpPublicInputs: Equatable {
    public let messageId: String
    public let payloadHash: String
    public let commitmentRoot: String
    public let finalizedSlot: UInt64
    public let parentSlot: UInt64
    public let bankSignatureCount: UInt64
    public let parentBankHash: String
    public let blockhash: String
    public let bankHash: String
    public let transactionStatusRoot: String
    public let messageProofHash: String
    public let accountInclusionRoot: String
    public let accountsLtHashChecksum: String
    public let accountsLtHashProofPublicInputsHash: String
    public let sourceEventDigest: String
    public let sourceStateVerifierId: String
    public let sourceStateVerifierHash: String
    public let statementHash: String
    public let destinationBindingHash: String
    public let sourceAdapterDeploymentHash: String
    public let sourceAdapterDeploymentReceiptHash: String
    public let sourceAdapterDeploymentBindingHash: String
}

/// Statement and verifier deployment context proved by the local Solana SCCP prover.
public struct SolanaSccpProofContext: Equatable {
    public let version: UInt8
    public let statementHash: String
    public let destinationBindingHash: String
}

/// Source-adapter deployment binding carried by local Solana SCCP proof requests.
public struct SolanaSccpSourceAdapterDeploymentBinding: Equatable {
    public let version: UInt8
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let sourceAdapterDeploymentHash: String
    public let sourceAdapterDeploymentReceiptHash: String
}

/// Request object passed to a linked local Solana SCCP prover.
public struct SolanaSccpProofRequest: Equatable {
    public let version: UInt8
    public let backend: String
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let mainnetGenesisHash: String
    public let witnessHash: String
    public let proofContextHash: String
    public let sourceAdapterDeploymentBindingHash: String
    public let sourceStateVerifierId: String
    public let sourceStateVerifierHash: String
    public let publicInputs: SolanaSccpPublicInputs
    public let witness: SolanaSccpWitness
    public let proofContext: SolanaSccpProofContext
    public let sourceAdapterDeploymentBinding: SolanaSccpSourceAdapterDeploymentBinding
}

/// Proof envelope returned by a linked local Solana SCCP prover.
public struct SolanaSccpProofResult: Equatable {
    public let version: UInt8
    public let backend: String
    public let proofBytes: Data
    public let proofBase64: String
    public let publicInputs: SolanaSccpPublicInputs
    public let witnessHash: String
    public let proofContextHash: String
    public let sourceAdapterDeploymentBindingHash: String
    public let sourceStateVerifierId: String
    public let sourceStateVerifierHash: String
    public let proofContext: SolanaSccpProofContext
    public let sourceAdapterDeploymentBinding: SolanaSccpSourceAdapterDeploymentBinding
    public let envelopeHash: String
}

/// Transparent SCCP public inputs serialized into Solana verifier instruction data.
public struct SolanaSccpSubmissionPublicInputs: Equatable {
    public let version: UInt8
    public let messageId: String
    public let payloadHash: String
    public let targetDomain: UInt32
    public let commitmentRoot: String
    public let finalityHeight: UInt64
    public let finalityBlockHash: String

    public init(version: UInt8 = 1,
                messageId: String,
                payloadHash: String,
                targetDomain: UInt32,
                commitmentRoot: String,
                finalityHeight: UInt64,
                finalityBlockHash: String) {
        self.version = version
        self.messageId = messageId
        self.payloadHash = payloadHash
        self.targetDomain = targetDomain
        self.commitmentRoot = commitmentRoot
        self.finalityHeight = finalityHeight
        self.finalityBlockHash = finalityBlockHash
    }
}

/// Inputs for a Solana SCCP verifier program instruction.
public struct SolanaSccpSubmissionInput: Equatable {
    public let publicInputs: SolanaSccpSubmissionPublicInputs
    public let proofBytes: Data
    public let bundleBytes: Data
    public let statementHash: String
    public let destinationBindingHash: String
    public let proofContextHash: String?
    public let proofResult: SolanaSccpProofResult?

    public init(publicInputs: SolanaSccpSubmissionPublicInputs,
                proofBytes: Data,
                bundleBytes: Data,
                statementHash: String,
                destinationBindingHash: String,
                proofContextHash: String? = nil,
                proofResult: SolanaSccpProofResult? = nil) {
        self.publicInputs = publicInputs
        self.proofBytes = proofBytes
        self.bundleBytes = bundleBytes
        self.statementHash = statementHash
        self.destinationBindingHash = destinationBindingHash
        self.proofContextHash = proofContextHash
        self.proofResult = proofResult
    }

    public init(publicInputs: SolanaSccpSubmissionPublicInputs,
                proofResult: SolanaSccpProofResult,
                bundleBytes: Data) throws {
        let proofResult = try requireWrappedSolanaProofResultForSubmission(
            proofResult,
            publicInputs: publicInputs
        )
        self.publicInputs = publicInputs
        self.proofBytes = proofResult.proofBytes
        self.bundleBytes = bundleBytes
        self.statementHash = proofResult.proofContext.statementHash
        self.destinationBindingHash = proofResult.proofContext.destinationBindingHash
        self.proofContextHash = proofResult.proofContextHash
        self.proofResult = proofResult
    }
}

/// One Solana SCCP submission argument in Rust template order.
public struct SolanaSccpSubmissionArgument: Equatable {
    public let key: String
    public let encoding: String
    public let bytesHex: String
}

/// Prebuilt Solana SCCP verifier instruction data for wallet or RPC submission.
public struct SolanaSccpSubmission: Equatable {
    public let version: UInt8
    public let envelopeEncoding: String
    public let submissionKind: String
    public let verifierEntrypoint: String
    public let proofBytes: Data
    public let publicInputs: SolanaSccpSubmissionPublicInputs
    public let publicInputsBytes: Data
    public let bundleBytes: Data
    public let statementHash: String
    public let destinationBindingHash: String
    public let proofContextHash: String
    public let arguments: [SolanaSccpSubmissionArgument]
    public let instructionData: Data
    public let instructionDataHex: String
    public let envelopeBytes: Data
    public let envelopeHex: String
}

/// Error cases for Solana SCCP local proof request construction.
public enum SolanaSccpProverError: Error, Equatable {
    case invalidString(String)
    case invalidHex32(String)
    case messageProofHashMismatch
    case proofContextHashMismatch
    case sourceAdapterDeploymentBindingMismatch
    case localProverUnavailable
    case emptyProof
    case allZeroProof
}

/// Optional async source for Solana RPC witness material.
public protocol SolanaSccpWitnessProvider {
    func resolveWitness(_ input: SolanaSccpWitnessInput) async throws -> SolanaSccpWitnessInput
}

/// Local-first Solana SCCP proof wrapper. It never fabricates proofs; callers must link a prover.
public final class SolanaSccpProver {
    public typealias ProveFunction = (SolanaSccpProofRequest) async throws -> Data

    private let witnessProvider: SolanaSccpWitnessProvider?
    private let proveFunction: ProveFunction?

    public init(
        witnessProvider: SolanaSccpWitnessProvider? = nil,
        proveFunction: ProveFunction? = nil
    ) {
        self.witnessProvider = witnessProvider
        self.proveFunction = proveFunction
    }

    public func buildRequest(_ input: SolanaSccpWitnessInput) async throws -> SolanaSccpProofRequest {
        let resolved = try await witnessProvider?.resolveWitness(solanaSccpWitnessProviderInputSnapshot(input)) ?? input
        return try buildSolanaSccpProofRequest(resolved)
    }

    public func prove(_ input: SolanaSccpWitnessInput) async throws -> SolanaSccpProofResult {
        let request = try await buildRequest(input)
        guard let proveFunction else {
            throw SolanaSccpProverError.localProverUnavailable
        }
        try requireProductionSolanaSccpProofRequest(request)
        let proofBytes = try await proveFunction(solanaSccpProofRequestCallbackSnapshot(request))
        return try wrapSolanaSccpProofResult(proofBytes: proofBytes, request: request)
    }
}

private func solanaSccpWitnessProviderInputSnapshot(_ input: SolanaSccpWitnessInput) -> SolanaSccpWitnessInput {
    SolanaSccpWitnessInput(
        targetDomain: input.targetDomain,
        mainnetGenesisHash: input.mainnetGenesisHash,
        finalizedSlot: input.finalizedSlot,
        parentSlot: input.parentSlot,
        bankSignatureCount: input.bankSignatureCount,
        parentBankHash: input.parentBankHash,
        blockhash: input.blockhash,
        bankHash: input.bankHash,
        transactionStatusRoot: input.transactionStatusRoot,
        messageProofHash: input.messageProofHash,
        accountInclusionRoot: input.accountInclusionRoot,
        accountsLtHashChecksum: input.accountsLtHashChecksum,
        accountsLtHashProofPublicInputsHash: input.accountsLtHashProofPublicInputsHash,
        bankHashHardForkData: Data(input.bankHashHardForkData),
        accountsLtHash: input.accountsLtHash.map { Data($0) },
        transactionSignature: input.transactionSignature,
        emitterProgramId: input.emitterProgramId,
        messageId: input.messageId,
        payloadHash: input.payloadHash,
        commitmentRoot: input.commitmentRoot,
        sourceEventDigest: input.sourceEventDigest,
        sourceStateVerifierId: input.sourceStateVerifierId,
        sourceStateVerifierHash: input.sourceStateVerifierHash,
        statementHash: input.statementHash,
        destinationBindingHash: input.destinationBindingHash,
        inclusionBranch: input.inclusionBranch.map { Data($0) },
        sourceAdapterDeploymentHash: input.sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash: input.sourceAdapterDeploymentReceiptHash
    )
}

/// Role-separated Solana full light-client audit proof capsules generated by a user-side prover.
public struct SolanaSccpFullLightClientAuditProofs: Equatable {
    public let towerReplay: SolanaSccpSourceStateVerificationProof
    public let fullAccountsdbLattice: SolanaSccpSourceStateVerificationProof
    public let bankForkChoice: SolanaSccpSourceStateVerificationProof
}

/// Local-first source-state proof wrapper for UI and mobile proof engines.
public final class SolanaSccpSourceStateProver {
    public typealias AccountsLtHashProveFunction =
        (SolanaSccpAccountsLtHashProofRequest) async throws -> Data
    public typealias FullLightClientAuditProveFunction =
        (SolanaSccpFullLightClientAuditProofRequest) async throws -> Data

    private let accountsLtHashProveFunction: AccountsLtHashProveFunction?
    private let fullLightClientAuditProveFunction: FullLightClientAuditProveFunction?

    public init(
        accountsLtHashProveFunction: AccountsLtHashProveFunction? = nil,
        fullLightClientAuditProveFunction: FullLightClientAuditProveFunction? = nil
    ) {
        self.accountsLtHashProveFunction = accountsLtHashProveFunction
        self.fullLightClientAuditProveFunction = fullLightClientAuditProveFunction
    }

    public func proveAccountsLtHash(
        witness: SolanaSccpWitnessInput,
        openedAccounts: SolanaSccpOpenedAccountsLtHashContributionsInput
    ) async throws -> SolanaSccpSourceStateVerificationProof {
        try await proveAccountsLtHash(
            request: buildSolanaSccpAccountsLtHashProofRequest(
                witness: witness,
                openedAccounts: openedAccounts
            )
        )
    }

    public func proveAccountsLtHash(
        request: SolanaSccpAccountsLtHashProofRequest
    ) async throws -> SolanaSccpSourceStateVerificationProof {
        try requireSourceStateProofRequestForWrapping(request)
        guard let accountsLtHashProveFunction else {
            throw SolanaSccpProverError.localProverUnavailable
        }
        let proofBytes = try await accountsLtHashProveFunction(
            solanaSccpAccountsLtHashProofRequestCallbackSnapshot(request)
        )
        return try wrapSolanaSccpSourceStateVerificationProof(
            proofBytes: proofBytes,
            request: request
        )
    }

    public func proveFullLightClientAudit(
        _ input: SolanaSccpFullLightClientAuditProofInput
    ) async throws -> SolanaSccpFullLightClientAuditProofs {
        let requests = try buildSolanaSccpFullLightClientAuditProofRequests(input)
        let towerReplay = try await proveFullLightClientAudit(request: requests.towerReplay)
        let fullAccountsdbLattice = try await proveFullLightClientAudit(
            request: requests.fullAccountsdbLattice
        )
        let bankForkChoice = try await proveFullLightClientAudit(request: requests.bankForkChoice)
        return SolanaSccpFullLightClientAuditProofs(
            towerReplay: towerReplay,
            fullAccountsdbLattice: fullAccountsdbLattice,
            bankForkChoice: bankForkChoice
        )
    }

    public func proveFullLightClientAudit(
        request: SolanaSccpFullLightClientAuditProofRequest
    ) async throws -> SolanaSccpSourceStateVerificationProof {
        try requireSourceStateProofRequestForWrapping(request)
        guard let fullLightClientAuditProveFunction else {
            throw SolanaSccpProverError.localProverUnavailable
        }
        let proofBytes = try await fullLightClientAuditProveFunction(
            solanaSccpFullLightClientAuditProofRequestCallbackSnapshot(request)
        )
        return try wrapSolanaSccpSourceStateVerificationProof(
            proofBytes: proofBytes,
            request: request
        )
    }
}

private func solanaSccpProofRequestCallbackSnapshot(
    _ request: SolanaSccpProofRequest
) -> SolanaSccpProofRequest {
    SolanaSccpProofRequest(
        version: request.version,
        backend: request.backend,
        sourceDomain: request.sourceDomain,
        targetDomain: request.targetDomain,
        mainnetGenesisHash: request.mainnetGenesisHash,
        witnessHash: request.witnessHash,
        proofContextHash: request.proofContextHash,
        sourceAdapterDeploymentBindingHash: request.sourceAdapterDeploymentBindingHash,
        sourceStateVerifierId: request.sourceStateVerifierId,
        sourceStateVerifierHash: request.sourceStateVerifierHash,
        publicInputs: request.publicInputs,
        witness: solanaSccpWitnessCallbackSnapshot(request.witness),
        proofContext: request.proofContext,
        sourceAdapterDeploymentBinding: request.sourceAdapterDeploymentBinding
    )
}

private func solanaSccpWitnessCallbackSnapshot(_ witness: SolanaSccpWitness) -> SolanaSccpWitness {
    SolanaSccpWitness(
        version: witness.version,
        sourceDomain: witness.sourceDomain,
        targetDomain: witness.targetDomain,
        mainnetGenesisHash: witness.mainnetGenesisHash,
        finalizedSlot: witness.finalizedSlot,
        parentSlot: witness.parentSlot,
        bankSignatureCount: witness.bankSignatureCount,
        parentBankHash: witness.parentBankHash,
        blockhash: witness.blockhash,
        bankHash: witness.bankHash,
        transactionStatusRoot: witness.transactionStatusRoot,
        messageProofHash: witness.messageProofHash,
        accountInclusionRoot: witness.accountInclusionRoot,
        accountsLtHashChecksum: witness.accountsLtHashChecksum,
        accountsLtHashProofPublicInputsHash: witness.accountsLtHashProofPublicInputsHash,
        bankHashHardForkData: Data(witness.bankHashHardForkData),
        accountsLtHash: witness.accountsLtHash.map { Data($0) },
        transactionSignature: witness.transactionSignature,
        emitterProgramId: witness.emitterProgramId,
        messageId: witness.messageId,
        payloadHash: witness.payloadHash,
        commitmentRoot: witness.commitmentRoot,
        sourceEventDigest: witness.sourceEventDigest,
        sourceStateVerifierId: witness.sourceStateVerifierId,
        sourceStateVerifierHash: witness.sourceStateVerifierHash,
        sourceAdapterDeploymentHash: witness.sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash: witness.sourceAdapterDeploymentReceiptHash,
        inclusionBranch: witness.inclusionBranch.map { Data($0) }
    )
}

private func solanaSccpAccountsLtHashProofRequestCallbackSnapshot(
    _ request: SolanaSccpAccountsLtHashProofRequest
) -> SolanaSccpAccountsLtHashProofRequest {
    SolanaSccpAccountsLtHashProofRequest(
        version: request.version,
        proofFamily: request.proofFamily,
        circuitId: request.circuitId,
        parameterSet: request.parameterSet,
        sourceDomain: request.sourceDomain,
        finalizedSlot: request.finalizedSlot,
        parentSlot: request.parentSlot,
        sourceStateVerifierId: request.sourceStateVerifierId,
        sourceStateVerifierHash: request.sourceStateVerifierHash,
        accountsLtHashProofPublicInputsHash: request.accountsLtHashProofPublicInputsHash,
        openedAccountsLtHashContributionsHash: request.openedAccountsLtHashContributionsHash,
        openedAccountsLtHashResidualChecksum: request.openedAccountsLtHashResidualChecksum,
        statementBytes: Data(request.statementBytes),
        accountCommitmentBytes: Data(request.accountCommitmentBytes),
        verificationContextBytes: Data(request.verificationContextBytes),
        schemaDescriptor: Data(request.schemaDescriptor),
        publicInputColumns: request.publicInputColumns.map { Array($0) },
        fastpqPublicInputs: request.fastpqPublicInputs,
        fastpqTransitions: request.fastpqTransitions.map {
            SolanaSccpAccountsLtHashFastpqTransition(
                key: $0.key,
                operation: $0.operation,
                oldValue: Data($0.oldValue),
                newValue: Data($0.newValue)
            )
        }
    )
}

private func solanaSccpFullLightClientAuditProofRequestCallbackSnapshot(
    _ request: SolanaSccpFullLightClientAuditProofRequest
) -> SolanaSccpFullLightClientAuditProofRequest {
    SolanaSccpFullLightClientAuditProofRequest(
        version: request.version,
        proofFamily: request.proofFamily,
        circuitId: request.circuitId,
        parameterSet: request.parameterSet,
        role: request.role,
        roleCode: request.roleCode,
        sourceDomain: request.sourceDomain,
        finalizedSlot: request.finalizedSlot,
        verifierId: request.verifierId,
        verifierHash: request.verifierHash,
        sourceStateVerifierId: request.sourceStateVerifierId,
        sourceStateVerifierHash: request.sourceStateVerifierHash,
        sourceVerifierMaterialHash: request.sourceVerifierMaterialHash,
        sourceAdapterDeploymentHash: request.sourceAdapterDeploymentHash,
        fullLightClientGateHash: request.fullLightClientGateHash,
        finalityContextHash: request.finalityContextHash,
        voteMessageHash: request.voteMessageHash,
        accountsLtHashProofHash: request.accountsLtHashProofHash,
        auditStatementHash: request.auditStatementHash,
        statementBytes: Data(request.statementBytes),
        verificationContextBytes: Data(request.verificationContextBytes),
        schemaDescriptor: Data(request.schemaDescriptor),
        publicInputColumns: request.publicInputColumns.map { Array($0) },
        fastpqPublicInputs: request.fastpqPublicInputs,
        fastpqTransitions: request.fastpqTransitions.map {
            SolanaSccpFullLightClientAuditFastpqTransition(
                key: $0.key,
                operation: $0.operation,
                oldValue: Data($0.oldValue),
                newValue: Data($0.newValue)
            )
        }
    )
}

private struct NormalizedSolanaRouteCanaryProgramDataEvidence {
    let verifierProgram: Data
    let verifierCodeHash: String
    let rpcCommitment: String
    let programOwner: String
    let programdataOwner: String
    let programAccountData: Data
    let programdataAddress: Data
    let programdataSlot: UInt64
    let expectedProgramdataSlot: UInt64
    let programAccountContextSlot: UInt64
    let programdataAccountContextSlot: UInt64
    let programdataMetadata: Data
    let programdataExecutable: Data
}

/// Canonical live ProgramData evidence bytes for the SORA -> Solana route canary.
public func canonicalSolanaSccpRouteCanaryEvidenceBytes(
    _ input: SolanaSccpRouteCanaryEvidenceInput
) throws -> Data {
    let routeAllowlistHash = try nonZeroHex32Bytes(input.routeAllowlistHash, field: "routeAllowlistHash")
    let destinationBindingHash = try nonZeroHex32Bytes(
        input.destinationBindingHash,
        field: "destinationBindingHash"
    )
    let canonicalSolanaDestinationBindingHash = try sccpDestinationBindingHash(domain: sccpDomainSolana)
    let expectedDestinationBindingHash = try normalizeNonZeroHex32(
        input.expectedDestinationBindingHash ?? canonicalSolanaDestinationBindingHash,
        field: "expectedDestinationBindingHash"
    )
    guard expectedDestinationBindingHash == canonicalSolanaDestinationBindingHash else {
        throw SolanaSccpProverError.invalidString("expectedDestinationBindingHash")
    }
    guard "0x" + destinationBindingHash.hexEncodedString() == canonicalSolanaDestinationBindingHash else {
        throw SolanaSccpProverError.invalidString("destinationBindingHash")
    }
    let sourceVerifierMaterialHash = try nonZeroHex32Bytes(
        input.sourceVerifierMaterialHash,
        field: "sourceVerifierMaterialHash"
    )
    let sourceAdapterEngineDeploymentHash = try nonZeroHex32Bytes(
        input.sourceAdapterEngineDeploymentHash,
        field: "sourceAdapterEngineDeploymentHash"
    )
    try requireSolanaHashRolesDistinct(
        field: "routeCanaryGovernedHashes",
        [
            ("routeAllowlistHash", routeAllowlistHash),
            ("destinationBindingHash", destinationBindingHash),
            ("sourceVerifierMaterialHash", sourceVerifierMaterialHash),
            ("sourceAdapterEngineDeploymentHash", sourceAdapterEngineDeploymentHash),
        ]
    )
    let evidence = try normalizeSolanaRouteCanaryProgramDataEvidence(input)

    var out = Data()
    out.append(1)
    appendU32Le(sccpDomainSora, to: &out)
    appendU32Le(sccpDomainSolana, to: &out)
    out.append(routeAllowlistHash)
    out.append(destinationBindingHash)
    out.append(sourceVerifierMaterialHash)
    out.append(sourceAdapterEngineDeploymentHash)
    out.append(evidence.verifierProgram)
    try out.append(bytesFromHex32(evidence.verifierCodeHash, field: "verifierCodeHash"))
    appendBytesVec(Data(evidence.rpcCommitment.utf8), to: &out)
    appendBytesVec(Data(evidence.programOwner.utf8), to: &out)
    appendBytesVec(Data(evidence.programdataOwner.utf8), to: &out)
    out.append(1)
    appendBytesVec(evidence.programAccountData, to: &out)
    out.append(evidence.programdataAddress)
    appendU64Le(evidence.programdataSlot, to: &out)
    appendU64Le(evidence.expectedProgramdataSlot, to: &out)
    appendU64Le(evidence.programAccountContextSlot, to: &out)
    appendU64Le(evidence.programdataAccountContextSlot, to: &out)
    appendBytesVec(evidence.programdataMetadata, to: &out)
    appendBytesVec(evidence.programdataExecutable, to: &out)
    return out
}

/// Hash Rust verifies for the SORA -> Solana live ProgramData route canary.
public func solanaSccpRouteCanaryEvidenceHash(
    _ input: SolanaSccpRouteCanaryEvidenceInput
) throws -> String {
    hashHex(
        prefix: sccpSolanaRouteCanaryLiveProgramPrefixV1,
        payload: try canonicalSolanaSccpRouteCanaryEvidenceBytes(input)
    )
}

/// Normalize raw Solana SCCP witness data.
public func normalizeSolanaSccpWitness(_ input: SolanaSccpWitnessInput) throws -> SolanaSccpWitness {
    guard input.targetDomain == sccpDomainSora else {
        throw SolanaSccpProverError.invalidString("targetDomain")
    }
    let parentNext = input.parentSlot.addingReportingOverflow(1)
    guard !parentNext.overflow, parentNext.partialValue == input.finalizedSlot else {
        throw SolanaSccpProverError.invalidString("parentSlot")
    }
    guard input.bankSignatureCount != 0 else {
        throw SolanaSccpProverError.invalidString("bankSignatureCount")
    }
    let parentBankHash = try normalizeNonZeroHex32(input.parentBankHash, field: "parentBankHash")
    let bankHash = try normalizeNonZeroHex32(input.bankHash, field: "bankHash")
    let blockhashBytes = try solanaHash32Bytes(input.blockhash, field: "blockhash")
    let transactionStatusRoot = try normalizeNonZeroHex32(input.transactionStatusRoot, field: "transactionStatusRoot")
    let sourceEventDigest = try normalizeNonZeroHex32(input.sourceEventDigest, field: "sourceEventDigest")
    let transactionSignature = try normalizeSolanaBase58Fixed(
        input.transactionSignature,
        field: "transactionSignature",
        byteLength: sccpSolanaTransactionSignatureBytes
    )
    let emitterProgramId = try normalizeSolanaBase58Fixed(
        input.emitterProgramId,
        field: "emitterProgramId",
        byteLength: sccpSolanaProgramIdBytes
    )
    let inclusionBranch = try normalizeInclusionBranch(input.inclusionBranch)
    if !inclusionBranch.isEmpty {
        let derivedTransactionStatusRoot = try solanaSccpTransactionStatusRootFromBranch(
            sourceEventDigest: sourceEventDigest,
            transactionSignature: transactionSignature,
            emitterProgramId: emitterProgramId,
            inclusionBranch: inclusionBranch
        )
        guard derivedTransactionStatusRoot == transactionStatusRoot else {
            throw SolanaSccpProverError.invalidHex32("transactionStatusRoot")
        }
    }
    let messageProofHash = try normalizeSolanaMessageProofHash(
        input.messageProofHash,
        sourceEventDigest: sourceEventDigest,
        transactionStatusRoot: transactionStatusRoot,
        transactionSignature: transactionSignature,
        emitterProgramId: emitterProgramId,
        inclusionBranch: inclusionBranch
    )
    let accountInclusionRoot = try normalizeNonZeroHex32(input.accountInclusionRoot, field: "accountInclusionRoot")
    let accountsLtHashChecksum = try normalizeNonZeroHex32(input.accountsLtHashChecksum, field: "accountsLtHashChecksum")
    guard input.bankHashHardForkData.count <= sccpSolanaMaxBankHardForkHashDataBytes else {
        throw SolanaSccpProverError.invalidString("bankHashHardForkData")
    }
    if let accountsLtHash = input.accountsLtHash {
        let expectedBankHash = try solanaSccpAgaveBankHash(
            parentBankHash: parentBankHash,
            bankSignatureCount: input.bankSignatureCount,
            blockhash: "0x" + blockhashBytes.hexEncodedString(),
            accountsLtHash: accountsLtHash,
            bankHashHardForkData: input.bankHashHardForkData
        )
        guard bankHash == expectedBankHash else {
            throw SolanaSccpProverError.invalidString("bankHash")
        }
        guard try blake3Hash32(accountsLtHash, field: "accountsLtHash") == bytesFromHex32(
            accountsLtHashChecksum,
            field: "accountsLtHashChecksum"
        ) else {
            throw SolanaSccpProverError.invalidString("accountsLtHashChecksum")
        }
    }
    let accountsLtHashProofPublicInputsHash = try solanaSccpAccountsLtHashProofPublicInputsHash(
        sourceDomain: sccpDomainSolana,
        finalizedSlot: input.finalizedSlot,
        parentSlot: input.parentSlot,
        bankSignatureCount: input.bankSignatureCount,
        parentBankHash: parentBankHash,
        bankHash: bankHash,
        blockhash: "0x" + blockhashBytes.hexEncodedString(),
        bankHashHardForkData: input.bankHashHardForkData,
        transactionStatusRoot: transactionStatusRoot,
        accountInclusionRoot: accountInclusionRoot,
        accountsLtHashChecksum: accountsLtHashChecksum,
        accountsLtHash: input.accountsLtHash
    )
    if let supplied = input.accountsLtHashProofPublicInputsHash {
        guard try normalizeHex32(supplied, field: "accountsLtHashProofPublicInputsHash") == accountsLtHashProofPublicInputsHash else {
            throw SolanaSccpProverError.invalidString("accountsLtHashProofPublicInputsHash")
        }
    }
    let deploymentBinding = try normalizeSccpSourceAdapterDeploymentBinding(
        sourceDomain: sccpDomainSolana,
        targetDomain: input.targetDomain,
        sourceAdapterDeploymentHash: input.sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash: input.sourceAdapterDeploymentReceiptHash
    )
    let sourceStateVerifierId = try normalizeNonEmpty(input.sourceStateVerifierId, field: "sourceStateVerifierId")
    let sourceStateVerifierHash = try normalizeHex32(input.sourceStateVerifierHash, field: "sourceStateVerifierHash")
    guard sourceStateVerifierHash == sccpZeroHashV1
        || sourceStateVerifierId == sccpSolanaMainnetAccountsDbVerifierIdV1
    else {
        throw SolanaSccpProverError.invalidString("sourceStateVerifierId")
    }
    return SolanaSccpWitness(
        version: 1,
        sourceDomain: sccpDomainSolana,
        targetDomain: input.targetDomain,
        mainnetGenesisHash: try normalizeNonEmpty(input.mainnetGenesisHash, field: "mainnetGenesisHash"),
        finalizedSlot: input.finalizedSlot,
        parentSlot: input.parentSlot,
        bankSignatureCount: input.bankSignatureCount,
        parentBankHash: parentBankHash,
        blockhash: "0x" + blockhashBytes.hexEncodedString(),
        bankHash: bankHash,
        transactionStatusRoot: transactionStatusRoot,
        messageProofHash: messageProofHash,
        accountInclusionRoot: accountInclusionRoot,
        accountsLtHashChecksum: accountsLtHashChecksum,
        accountsLtHashProofPublicInputsHash: accountsLtHashProofPublicInputsHash,
        bankHashHardForkData: input.bankHashHardForkData,
        accountsLtHash: input.accountsLtHash,
        transactionSignature: transactionSignature,
        emitterProgramId: emitterProgramId,
        messageId: try normalizeHex32(input.messageId, field: "messageId"),
        payloadHash: try normalizeHex32(input.payloadHash, field: "payloadHash"),
        commitmentRoot: try normalizeHex32(input.commitmentRoot, field: "commitmentRoot"),
        sourceEventDigest: sourceEventDigest,
        sourceStateVerifierId: sourceStateVerifierId,
        sourceStateVerifierHash: sourceStateVerifierHash,
        sourceAdapterDeploymentHash: deploymentBinding.sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash: deploymentBinding.sourceAdapterDeploymentReceiptHash,
        inclusionBranch: inclusionBranch
    )
}

/// Canonical bytes hashed by the Solana SCCP proof request.
public func canonicalSolanaSccpWitnessBytes(_ input: SolanaSccpWitnessInput) throws -> Data {
    try canonicalSolanaSccpWitnessBytes(normalizeSolanaSccpWitness(input))
}

/// Build a Solana SCCP proof request for a linked local prover.
public func buildSolanaSccpProofRequest(_ input: SolanaSccpWitnessInput) throws -> SolanaSccpProofRequest {
    let witness = try normalizeSolanaSccpWitness(input)
    let witnessHash = hashHex(prefix: "sccp:solana:witness:v1", payload: try canonicalSolanaSccpWitnessBytes(witness))
    let proofContext = try normalizeSolanaSccpProofContext(
        statementHash: input.statementHash,
        destinationBindingHash: input.destinationBindingHash
    )
    let proofContextHash = try solanaSccpProofContextHash(proofContext)
    let deploymentBinding = try normalizeSccpSourceAdapterDeploymentBinding(
        sourceDomain: witness.sourceDomain,
        targetDomain: witness.targetDomain,
        sourceAdapterDeploymentHash: witness.sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash: witness.sourceAdapterDeploymentReceiptHash
    )
    guard deploymentBinding.sourceAdapterDeploymentHash != sccpZeroHashV1 else {
        throw SolanaSccpProverError.sourceAdapterDeploymentBindingMismatch
    }
    let deploymentBindingHash = try sccpSourceAdapterDeploymentBindingHash(deploymentBinding)
    return SolanaSccpProofRequest(
        version: 1,
        backend: sccpSolanaRecursiveProofBackendV1,
        sourceDomain: sccpDomainSolana,
        targetDomain: witness.targetDomain,
        mainnetGenesisHash: witness.mainnetGenesisHash,
        witnessHash: witnessHash,
        proofContextHash: proofContextHash,
        sourceAdapterDeploymentBindingHash: deploymentBindingHash,
        sourceStateVerifierId: witness.sourceStateVerifierId,
        sourceStateVerifierHash: witness.sourceStateVerifierHash,
        publicInputs: SolanaSccpPublicInputs(
            messageId: witness.messageId,
            payloadHash: witness.payloadHash,
            commitmentRoot: witness.commitmentRoot,
            finalizedSlot: witness.finalizedSlot,
            parentSlot: witness.parentSlot,
            bankSignatureCount: witness.bankSignatureCount,
            parentBankHash: witness.parentBankHash,
            blockhash: witness.blockhash,
            bankHash: witness.bankHash,
            transactionStatusRoot: witness.transactionStatusRoot,
            messageProofHash: witness.messageProofHash,
            accountInclusionRoot: witness.accountInclusionRoot,
            accountsLtHashChecksum: witness.accountsLtHashChecksum,
            accountsLtHashProofPublicInputsHash: witness.accountsLtHashProofPublicInputsHash,
            sourceEventDigest: witness.sourceEventDigest,
            sourceStateVerifierId: witness.sourceStateVerifierId,
            sourceStateVerifierHash: witness.sourceStateVerifierHash,
            statementHash: proofContext.statementHash,
            destinationBindingHash: proofContext.destinationBindingHash,
            sourceAdapterDeploymentHash: deploymentBinding.sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash: deploymentBinding.sourceAdapterDeploymentReceiptHash,
            sourceAdapterDeploymentBindingHash: deploymentBindingHash
        ),
        witness: witness,
        proofContext: proofContext,
        sourceAdapterDeploymentBinding: deploymentBinding
    )
}

/// Normalize source-adapter deployment binding material used by UI-generated source proofs.
public func normalizeSccpSourceAdapterDeploymentBinding(
    sourceDomain: UInt32 = sccpDomainSolana,
    targetDomain: UInt32 = sccpDomainSora,
    sourceAdapterDeploymentHash: String = sccpZeroHashV1,
    sourceAdapterDeploymentReceiptHash: String = sccpZeroHashV1
) throws -> SolanaSccpSourceAdapterDeploymentBinding {
    let deploymentHash = try normalizeHex32(sourceAdapterDeploymentHash, field: "sourceAdapterDeploymentHash")
    let receiptHash = try normalizeHex32(
        sourceAdapterDeploymentReceiptHash,
        field: "sourceAdapterDeploymentReceiptHash"
    )
    guard [
        sccpDomainEthereum,
        sccpDomainBsc,
        sccpDomainSolana,
        sccpDomainTon,
        sccpDomainTron,
    ].contains(sourceDomain) else {
        throw SolanaSccpProverError.sourceAdapterDeploymentBindingMismatch
    }
    guard targetDomain == sccpDomainSora else {
        throw SolanaSccpProverError.sourceAdapterDeploymentBindingMismatch
    }
    let deploymentIsZero = deploymentHash == sccpZeroHashV1
    let receiptIsZero = receiptHash == sccpZeroHashV1
    guard deploymentIsZero == receiptIsZero else {
        throw SolanaSccpProverError.sourceAdapterDeploymentBindingMismatch
    }
    guard deploymentIsZero || deploymentHash != receiptHash else {
        throw SolanaSccpProverError.sourceAdapterDeploymentBindingMismatch
    }
    return SolanaSccpSourceAdapterDeploymentBinding(
        version: 1,
        sourceDomain: sourceDomain,
        targetDomain: targetDomain,
        sourceAdapterDeploymentHash: deploymentHash,
        sourceAdapterDeploymentReceiptHash: receiptHash
    )
}

/// Canonical bytes for source-adapter deployment binding material.
public func canonicalSccpSourceAdapterDeploymentBindingBytes(
    _ binding: SolanaSccpSourceAdapterDeploymentBinding
) throws -> Data {
    var out = Data()
    out.append(binding.version)
    appendU32Le(binding.sourceDomain, to: &out)
    appendU32Le(binding.targetDomain, to: &out)
    try out.append(bytesFromHex32(binding.sourceAdapterDeploymentHash, field: "sourceAdapterDeploymentHash"))
    try out.append(
        bytesFromHex32(
            binding.sourceAdapterDeploymentReceiptHash,
            field: "sourceAdapterDeploymentReceiptHash"
        )
    )
    return out
}

/// Hash of source-adapter deployment binding material included in UI proof requests.
public func sccpSourceAdapterDeploymentBindingHash(
    _ binding: SolanaSccpSourceAdapterDeploymentBinding
) throws -> String {
    hashHex(
        prefix: "sccp:source-adapter-deployment-binding:v1",
        payload: try canonicalSccpSourceAdapterDeploymentBindingBytes(binding)
    )
}

/// Normalize the statement and destination binding context for a Solana SCCP proof request.
public func normalizeSolanaSccpProofContext(
    statementHash: String,
    destinationBindingHash: String
) throws -> SolanaSccpProofContext {
    SolanaSccpProofContext(
        version: 1,
        statementHash: try normalizeNonZeroHex32(statementHash, field: "statementHash"),
        destinationBindingHash: try normalizeNonZeroHex32(destinationBindingHash, field: "destinationBindingHash")
    )
}

/// Canonical bytes hashed into a Solana SCCP proof request context.
public func canonicalSolanaSccpProofContextBytes(_ context: SolanaSccpProofContext) throws -> Data {
    var out = Data()
    out.append(context.version)
    try out.append(bytesFromHex32(context.statementHash, field: "statementHash"))
    try out.append(bytesFromHex32(context.destinationBindingHash, field: "destinationBindingHash"))
    return out
}

/// Hash of the Solana SCCP statement/deployment context checked by submissions.
public func solanaSccpProofContextHash(_ context: SolanaSccpProofContext) throws -> String {
    hashHex(
        prefix: "sccp:solana:proof-context:v1",
        payload: try canonicalSolanaSccpProofContextBytes(context)
    )
}

/// Canonical transparent SCCP public inputs serialized into Solana instruction data.
public func canonicalSolanaSccpSubmissionPublicInputsBytes(_ input: SolanaSccpSubmissionPublicInputs) throws -> Data {
    var out = Data()
    out.append(input.version)
    try out.append(bytesFromHex32(input.messageId, field: "messageId"))
    try out.append(bytesFromHex32(input.payloadHash, field: "payloadHash"))
    appendU32Le(input.targetDomain, to: &out)
    try out.append(bytesFromHex32(input.commitmentRoot, field: "commitmentRoot"))
    appendU64Le(input.finalityHeight, to: &out)
    try out.append(bytesFromHex32(input.finalityBlockHash, field: "finalityBlockHash"))
    return out
}

/// Build Solana verifier program instruction data from UI-generated proof bytes.
public func buildSolanaSccpSubmission(_ input: SolanaSccpSubmissionInput) throws -> SolanaSccpSubmission {
    try requireNativeRecursivePayloadBytes(input.bundleBytes, field: "bundleBytes")
    guard input.publicInputs.version == 1 else {
        throw SolanaSccpProverError.invalidString("publicInputs.version")
    }
    guard input.publicInputs.targetDomain == sccpDomainSolana else {
        throw SolanaSccpProverError.invalidString("publicInputs.targetDomain")
    }
    guard let wrappedProofResult = input.proofResult else {
        throw SolanaSccpProverError.invalidString("proofResult")
    }
    let proofResult = try requireWrappedSolanaProofResultForSubmission(
        wrappedProofResult,
        publicInputs: input.publicInputs
    )
    guard input.proofBytes == proofResult.proofBytes else {
        throw SolanaSccpProverError.invalidString("proofBytes")
    }
    let publicInputsBytes = try canonicalSolanaSccpSubmissionPublicInputsBytes(input.publicInputs)
    let proofContext = proofResult.proofContext
    let proofContextStatementHash = try normalizeHex32(
        proofContext.statementHash,
        field: "proofResult.proofContext.statementHash"
    )
    let proofContextDestinationBindingHash = try normalizeHex32(
        proofContext.destinationBindingHash,
        field: "proofResult.proofContext.destinationBindingHash"
    )
    guard try normalizeHex32(input.statementHash, field: "statementHash") == proofContextStatementHash else {
        throw SolanaSccpProverError.invalidString("statementHash")
    }
    guard try normalizeHex32(input.destinationBindingHash, field: "destinationBindingHash")
        == proofContextDestinationBindingHash else {
        throw SolanaSccpProverError.invalidString("destinationBindingHash")
    }
    let expectedDestinationBindingHash = try sccpDestinationBindingHash(domain: sccpDomainSolana)
    guard proofContextDestinationBindingHash == expectedDestinationBindingHash else {
        throw SolanaSccpProverError.invalidString("destinationBindingHash")
    }
    let expectedProofContextHash = try solanaSccpProofContextHash(proofContext)
    if let supplied = input.proofContextHash,
       try normalizeHex32(supplied, field: "proofContextHash") != expectedProofContextHash {
        throw SolanaSccpProverError.proofContextHashMismatch
    }
    let statementHashBytes = try bytesFromHex32(proofContextStatementHash, field: "statementHash")
    let destinationBindingHashBytes = try bytesFromHex32(
        proofContextDestinationBindingHash,
        field: "destinationBindingHash"
    )
    let proofContextHashBytes = try bytesFromHex32(expectedProofContextHash, field: "proofContextHash")
    let argumentPairs: [(String, Data)] = [
        ("proof_bytes", proofResult.proofBytes),
        ("public_inputs", publicInputsBytes),
        ("bundle_bytes", input.bundleBytes),
        ("statement_hash", statementHashBytes),
        ("destination_binding_hash", destinationBindingHashBytes),
        ("proof_context_hash", proofContextHashBytes),
    ]
    var instructionData = Data()
    appendBytesVec(Data(sccpSolanaSubmitMessageProofEntrypointV1.utf8), to: &instructionData)
    for (_, bytes) in argumentPairs {
        appendBytesVec(bytes, to: &instructionData)
    }
    let arguments = argumentPairs.map { key, bytes in
        SolanaSccpSubmissionArgument(
            key: key,
            encoding: "raw_bytes",
            bytesHex: "0x" + bytes.hexEncodedString()
        )
    }
    return SolanaSccpSubmission(
        version: 1,
        envelopeEncoding: sccpSolanaBorshInstructionV1,
        submissionKind: "program_instruction",
        verifierEntrypoint: sccpSolanaSubmitMessageProofEntrypointV1,
        proofBytes: proofResult.proofBytes,
        publicInputs: input.publicInputs,
        publicInputsBytes: publicInputsBytes,
        bundleBytes: input.bundleBytes,
        statementHash: proofContextStatementHash,
        destinationBindingHash: proofContextDestinationBindingHash,
        proofContextHash: expectedProofContextHash,
        arguments: arguments,
        instructionData: instructionData,
        instructionDataHex: "0x" + instructionData.hexEncodedString(),
        envelopeBytes: instructionData,
        envelopeHex: "0x" + instructionData.hexEncodedString()
    )
}

private func requireNativeRecursivePayloadBytes(_ bytes: Data, field: String) throws {
    guard !bytes.isEmpty else {
        throw SolanaSccpProverError.invalidString(field)
    }
    guard bytes.count <= sccpNativeRecursiveMaxProofBytes else {
        throw SolanaSccpProverError.invalidString(field)
    }
    guard bytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidString(field)
    }
}

/// Canonical bytes used for the Solana message inclusion proof hash.
public func canonicalSolanaSccpMessageProofBytes(
    sourceEventDigest: String,
    transactionStatusRoot: String,
    transactionSignature: String,
    emitterProgramId: String,
    inclusionBranch: [Data]
) throws -> Data {
    var out = Data()
    out.append(1)
    let sourceEventDigestBytes = try bytesFromHex32(sourceEventDigest, field: "sourceEventDigest")
    guard sourceEventDigestBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("sourceEventDigest")
    }
    let transactionStatusRootBytes = try bytesFromHex32(transactionStatusRoot, field: "transactionStatusRoot")
    guard transactionStatusRootBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("transactionStatusRoot")
    }
    out.append(sourceEventDigestBytes)
    out.append(transactionStatusRootBytes)
    appendBytesVec(
        try decodeSolanaBase58Fixed(
            transactionSignature,
            field: "transactionSignature",
            byteLength: sccpSolanaTransactionSignatureBytes
        ),
        to: &out
    )
    appendBytesVec(
        try decodeSolanaBase58Fixed(
            emitterProgramId,
            field: "emitterProgramId",
            byteLength: sccpSolanaProgramIdBytes
        ),
        to: &out
    )
    let normalizedInclusionBranch = try normalizeInclusionBranch(inclusionBranch)
    guard !normalizedInclusionBranch.isEmpty else {
        throw SolanaSccpProverError.invalidString("inclusionBranch")
    }
    appendU32Le(UInt32(normalizedInclusionBranch.count), to: &out)
    for sibling in normalizedInclusionBranch {
        out.append(sibling)
    }
    return out
}

/// Hash the Solana message inclusion proof in the same form expected by SCCP source adapters.
public func solanaSccpMessageProofHash(
    sourceEventDigest: String,
    transactionStatusRoot: String,
    transactionSignature: String,
    emitterProgramId: String,
    inclusionBranch: [Data]
) throws -> String {
    hashHex(
        prefix: "sccp:solana:message-proof:v1",
        payload: try canonicalSolanaSccpMessageProofBytes(
            sourceEventDigest: sourceEventDigest,
            transactionStatusRoot: transactionStatusRoot,
            transactionSignature: transactionSignature,
            emitterProgramId: emitterProgramId,
            inclusionBranch: inclusionBranch
        )
    )
}

/// Canonical bytes for the Solana transaction-status leaf bound into the SCCP root.
public func canonicalSolanaSccpTransactionStatusLeafBytes(
    sourceEventDigest: String,
    transactionSignature: String,
    emitterProgramId: String
) throws -> Data {
    var out = Data()
    out.append(1)
    let sourceEventDigestBytes = try bytesFromHex32(sourceEventDigest, field: "sourceEventDigest")
    guard sourceEventDigestBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("sourceEventDigest")
    }
    out.append(sourceEventDigestBytes)
    appendBytesVec(
        try decodeSolanaBase58Fixed(
            transactionSignature,
            field: "transactionSignature",
            byteLength: sccpSolanaTransactionSignatureBytes
        ),
        to: &out
    )
    appendBytesVec(
        try decodeSolanaBase58Fixed(
            emitterProgramId,
            field: "emitterProgramId",
            byteLength: sccpSolanaProgramIdBytes
        ),
        to: &out
    )
    return out
}

/// Hash the Solana transaction-status leaf that is opened under `transactionStatusRoot`.
public func solanaSccpTransactionStatusLeafHash(
    sourceEventDigest: String,
    transactionSignature: String,
    emitterProgramId: String
) throws -> String {
    hashHex(
        prefix: "sccp:solana:transaction-status-leaf:v1",
        payload: try canonicalSolanaSccpTransactionStatusLeafBytes(
            sourceEventDigest: sourceEventDigest,
            transactionSignature: transactionSignature,
            emitterProgramId: emitterProgramId
        )
    )
}

/// Recompute the Solana transaction-status root from the identity-bound SCCP leaf.
public func solanaSccpTransactionStatusRootFromBranch(
    sourceEventDigest: String,
    transactionSignature: String,
    emitterProgramId: String,
    inclusionBranch: [Data]
) throws -> String {
    let normalizedInclusionBranch = try normalizeInclusionBranch(inclusionBranch)
    guard !normalizedInclusionBranch.isEmpty else {
        throw SolanaSccpProverError.invalidString("inclusionBranch")
    }
    var current = try bytesFromHex32(
        solanaSccpTransactionStatusLeafHash(
            sourceEventDigest: sourceEventDigest,
            transactionSignature: transactionSignature,
            emitterProgramId: emitterProgramId
        ),
        field: "transactionStatusLeafHash"
    )
    for sibling in normalizedInclusionBranch {
        current = sourceMerkleNodeHash(left: current, right: sibling)
    }
    return "0x" + current.hexEncodedString()
}

/// Return the Solana mainnet-beta epoch for a finalized slot.
public func solanaSccpMainnetEpoch(forSlot slot: UInt64) -> UInt64 {
    slot / sccpSolanaMainnetSlotsPerEpoch
}

/// Canonical bytes for the Solana active-stake root checked by SCCP finality contexts.
public func canonicalSolanaSccpEpochStakeRootBytes(
    epoch: UInt64,
    validatorPublicKeys: [Data],
    validatorStakes: [UInt64]
) throws -> Data {
    let rosterBytes = try canonicalSolanaVoteRosterBytes(
        validatorPublicKeys: validatorPublicKeys,
        validatorStakes: validatorStakes
    )
    let rosterHash = hashBytes(prefix: "sccp:solana:vote-roster:v1", payload: rosterBytes)
    var out = Data()
    out.append(1)
    appendU64Le(epoch, to: &out)
    out.append(rosterHash)
    out.append(rosterBytes)
    return out
}

/// Hash the Solana active-stake root used by SCCP finality contexts.
public func solanaSccpEpochStakeRoot(
    epoch: UInt64,
    validatorPublicKeys: [Data],
    validatorStakes: [UInt64]
) throws -> String {
    hashHex(
        prefix: "sccp:solana:epoch-stake-root:v1",
        payload: try canonicalSolanaSccpEpochStakeRootBytes(
            epoch: epoch,
            validatorPublicKeys: validatorPublicKeys,
            validatorStakes: validatorStakes
        )
    )
}

/// Canonical bytes for the Solana active-stake activation evidence checked by SCCP finality contexts.
public func canonicalSolanaSccpStakeActivationBytes(
    epoch: UInt64,
    validatorPublicKeys: [Data],
    validatorStakes: [UInt64],
    validatorActivationEpochs: [UInt64],
    validatorDeactivationEpochs: [UInt64]
) throws -> Data {
    guard validatorActivationEpochs.count == validatorPublicKeys.count else {
        throw SolanaSccpProverError.invalidString("validatorActivationEpochs")
    }
    guard validatorDeactivationEpochs.count == validatorPublicKeys.count else {
        throw SolanaSccpProverError.invalidString("validatorDeactivationEpochs")
    }
    let rosterBytes = try canonicalSolanaVoteRosterBytes(
        validatorPublicKeys: validatorPublicKeys,
        validatorStakes: validatorStakes
    )
    let rosterHash = hashBytes(prefix: "sccp:solana:vote-roster:v1", payload: rosterBytes)
    var out = Data()
    out.append(1)
    appendU64Le(epoch, to: &out)
    out.append(rosterHash)
    appendU32Le(UInt32(validatorPublicKeys.count), to: &out)
    for (index, publicKey) in validatorPublicKeys.enumerated() {
        let activationEpoch = validatorActivationEpochs[index]
        let deactivationEpoch = validatorDeactivationEpochs[index]
        guard activationEpoch < epoch else {
            throw SolanaSccpProverError.invalidString("validatorActivationEpochs[\(index)]")
        }
        guard deactivationEpoch > activationEpoch else {
            throw SolanaSccpProverError.invalidString("validatorDeactivationEpochs[\(index)]")
        }
        appendBytesVec(publicKey, to: &out)
        appendU64Le(validatorStakes[index], to: &out)
        appendU64Le(activationEpoch, to: &out)
        appendU64Le(deactivationEpoch, to: &out)
    }
    return out
}

/// Hash the Solana active-stake activation evidence used by SCCP finality contexts.
public func solanaSccpStakeActivationHash(
    epoch: UInt64,
    validatorPublicKeys: [Data],
    validatorStakes: [UInt64],
    validatorActivationEpochs: [UInt64],
    validatorDeactivationEpochs: [UInt64]
) throws -> String {
    hashHex(
        prefix: "sccp:solana:stake-activation:v1",
        payload: try canonicalSolanaSccpStakeActivationBytes(
            epoch: epoch,
            validatorPublicKeys: validatorPublicKeys,
            validatorStakes: validatorStakes,
            validatorActivationEpochs: validatorActivationEpochs,
            validatorDeactivationEpochs: validatorDeactivationEpochs
        )
    )
}

/// Canonical bytes for a Solana account opening bound into SCCP account-state evidence.
public func canonicalSolanaSccpAccountOpeningBytes(
    address: Data,
    owner: Data,
    lamports: UInt64,
    rentEpoch: UInt64,
    executable: Bool = false,
    dataHash: Data
) throws -> Data {
    guard address.count == 32, address.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidString("address")
    }
    guard owner.count == 32, owner.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidString("owner")
    }
    guard lamports > 0 else {
        throw SolanaSccpProverError.invalidString("lamports")
    }
    guard dataHash.count == 32, dataHash.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("dataHash")
    }
    var out = Data()
    out.append(1)
    appendBytesVec(address, to: &out)
    appendBytesVec(owner, to: &out)
    appendU64Le(lamports, to: &out)
    appendU64Le(rentEpoch, to: &out)
    out.append(executable ? 1 : 0)
    out.append(dataHash)
    return out
}

/// Hash a Solana account opening bound into SCCP account-state evidence.
public func solanaSccpAccountOpeningHash(
    address: Data,
    owner: Data,
    lamports: UInt64,
    rentEpoch: UInt64,
    executable: Bool = false,
    dataHash: Data
) throws -> String {
    hashHex(
        prefix: "sccp:solana:account-opening:v1",
        payload: try canonicalSolanaSccpAccountOpeningBytes(
            address: address,
            owner: owner,
            lamports: lamports,
            rentEpoch: rentEpoch,
            executable: executable,
            dataHash: dataHash
        )
    )
}

/// Hash exact raw Solana account data bytes for account inclusion proofs.
public func solanaSccpAccountRawDataHash(_ rawData: Data) throws -> String {
    guard !rawData.isEmpty, rawData.count <= sccpSolanaMaxAccountRawDataBytes else {
        throw SolanaSccpProverError.invalidString("rawData")
    }
    return hashHex(prefix: "sccp:solana:account-raw-data:v1", payload: rawData)
}

/// Return Agave's 32-byte checksum for a canonical 2048-byte Solana AccountsLtHash.
public func solanaSccpAccountsLtHashChecksum(_ accountsLtHash: Data) throws -> String {
    guard accountsLtHash.count == sccpSolanaAccountsLtHashBytes else {
        throw SolanaSccpProverError.invalidString("accountsLtHash")
    }
    return "0x" + (try blake3Hash32(accountsLtHash, field: "accountsLtHash")).hexEncodedString()
}

/// Return Agave's 2048-byte AccountLtHash contribution for one opened account.
public func solanaSccpAccountLtHash(
    opening: SolanaSccpAccountOpeningInput,
    rawData: Data
) throws -> Data {
    guard opening.address.count == 32 else {
        throw SolanaSccpProverError.invalidString("address")
    }
    guard opening.owner.count == 32 else {
        throw SolanaSccpProverError.invalidString("owner")
    }
    guard rawData.count <= sccpSolanaMaxAccountRawDataBytes else {
        throw SolanaSccpProverError.invalidString("rawData")
    }
    guard opening.lamports > 0 else {
        return Data(repeating: 0, count: sccpSolanaAccountsLtHashBytes)
    }
    var preimage = Data(capacity: 8 + rawData.count + 1 + 32 + 32)
    appendU64Le(opening.lamports, to: &preimage)
    preimage.append(rawData)
    preimage.append(opening.executable ? UInt8(1) : UInt8(0))
    preimage.append(opening.owner)
    preimage.append(opening.address)
    guard let accountLtHash = SolanaSccpBlake3.derive(
        preimage,
        outputLength: sccpSolanaAccountsLtHashBytes
    ), accountLtHash.count == sccpSolanaAccountsLtHashBytes else {
        throw SolanaSccpProverError.invalidString("accountLtHash")
    }
    return accountLtHash
}

/// Return the summed Solana AccountsLtHash contribution for opened account witnesses.
public func solanaSccpAccountsLtHashFromOpenings(
    openings: [SolanaSccpAccountOpeningInput],
    rawDataValues: [Data]
) throws -> Data {
    guard openings.count == rawDataValues.count else {
        throw SolanaSccpProverError.invalidString("openings")
    }
    var out = Data(repeating: 0, count: sccpSolanaAccountsLtHashBytes)
    for index in openings.indices {
        try addAccountsLtHashContribution(
            &out,
            solanaSccpAccountLtHash(opening: openings[index], rawData: rawDataValues[index])
        )
    }
    return out
}

/// Return the bank LtHash residual after subtracting opened account contributions.
public func solanaSccpOpenedAccountsLtHashResidual(
    _ input: SolanaSccpOpenedAccountsLtHashContributionsInput
) throws -> Data {
    try normalizeOpenedAccountsLtHashContributions(input).residualAccountsLtHash
}

/// Return the checksum for unopened AccountsDB LtHash residual state.
public func solanaSccpOpenedAccountsLtHashResidualChecksum(
    _ input: SolanaSccpOpenedAccountsLtHashContributionsInput
) throws -> String {
    "0x" + (try normalizeOpenedAccountsLtHashContributions(input).residualAccountsLtHashChecksum).hexEncodedString()
}

/// Canonical opened AccountsLtHash contribution transcript bytes.
public func canonicalSolanaSccpOpenedAccountsLtHashContributionsBytes(
    _ input: SolanaSccpOpenedAccountsLtHashContributionsInput
) throws -> Data {
    let normalized = try normalizeOpenedAccountsLtHashContributions(input)
    var out = Data()
    out.append(1)
    appendU32Le(normalized.sourceDomain, to: &out)
    appendU64Le(normalized.finalizedSlot, to: &out)
    out.append(normalized.accountInclusionRoot)
    out.append(normalized.accountsLtHashChecksum)
    out.append(normalized.openedAccountsLtHashChecksum)
    out.append(normalized.residualAccountsLtHashChecksum)
    appendBytesVec(normalized.openedAccountsLtHash, to: &out)
    appendBytesVec(normalized.residualAccountsLtHash, to: &out)
    appendU32Le(UInt32(normalized.rows.count), to: &out)
    for row in normalized.rows {
        out.append(row.role)
        out.append(row.address)
        out.append(row.accountHash)
        out.append(row.rawDataHash)
        appendBytesVec(row.accountLtHash, to: &out)
    }
    return out
}

/// Hash opened AccountsLtHash contributions bound into the recursive source-state proof.
public func solanaSccpOpenedAccountsLtHashContributionsHash(
    _ input: SolanaSccpOpenedAccountsLtHashContributionsInput
) throws -> String {
    hashHex(
        prefix: sccpSolanaAccountsLtHashOpenedContributionsPrefixV1,
        payload: try canonicalSolanaSccpOpenedAccountsLtHashContributionsBytes(input)
    )
}

/// Canonical AccountsLtHash commitment bytes consumed by the source-state prover.
public func canonicalSolanaSccpAccountsLtHashCommitmentBytes(
    witness: SolanaSccpWitnessInput,
    openedAccounts: SolanaSccpOpenedAccountsLtHashContributionsInput
) throws -> Data {
    let normalized = try normalizeAccountsLtHashProofRequest(witness: witness, openedAccounts: openedAccounts)
    var out = Data()
    out.append(1)
    appendU32Le(normalized.witness.sourceDomain, to: &out)
    appendU64Le(normalized.witness.finalizedSlot, to: &out)
    try out.append(bytesFromHex32(normalized.witness.accountsLtHashChecksum, field: "accountsLtHashChecksum"))
    try out.append(
        bytesFromHex32(
            normalized.openedContributionsHash,
            field: "openedAccountsLtHashContributionsHash"
        )
    )
    try out.append(bytesFromHex32(normalized.residualChecksum, field: "openedAccountsLtHashResidualChecksum"))
    appendBytesVec(normalized.accountsLtHash, to: &out)
    return out
}

/// Canonical OpenVerify context bytes for Solana AccountsLtHash source proofs.
public func canonicalSolanaSccpAccountsLtHashVerificationContextBytes(
    witness: SolanaSccpWitnessInput,
    openedAccounts: SolanaSccpOpenedAccountsLtHashContributionsInput
) throws -> Data {
    let normalized = try normalizeAccountsLtHashProofRequest(witness: witness, openedAccounts: openedAccounts)
    var out = Data()
    out.append(1)
    try appendString(sccpSolanaAccountsLtHashOpenVerifyCircuitIdV1, field: "circuitId", to: &out)
    try appendString(sccpSolanaAccountsLtHashFastpqParameterSetV1, field: "parameterSet", to: &out)
    try appendString(normalized.witness.sourceStateVerifierId, field: "sourceStateVerifierId", to: &out)
    try out.append(bytesFromHex32(normalized.witness.sourceStateVerifierHash, field: "sourceStateVerifierHash"))
    try out.append(
        bytesFromHex32(
            normalized.witness.accountsLtHashProofPublicInputsHash,
            field: "accountsLtHashProofPublicInputsHash"
        )
    )
    try out.append(
        bytesFromHex32(
            normalized.openedContributionsHash,
            field: "openedAccountsLtHashContributionsHash"
        )
    )
    try out.append(bytesFromHex32(normalized.residualChecksum, field: "openedAccountsLtHashResidualChecksum"))
    return out
}

/// OpenVerify public-input columns for the AccountsLtHash source-state proof.
public func solanaSccpAccountsLtHashPublicInputColumns(
    witness: SolanaSccpWitnessInput,
    openedAccounts: SolanaSccpOpenedAccountsLtHashContributionsInput
) throws -> [[String]] {
    let normalized = try normalizeAccountsLtHashProofRequest(witness: witness, openedAccounts: openedAccounts)
    let witness = normalized.witness
    return [
        ["0x" + sccpWordU32Le(witness.sourceDomain).hexEncodedString()],
        [solanaSccpMainnetGenesisHashPublicInput()],
        ["0x" + sccpWordU64Le(witness.finalizedSlot).hexEncodedString()],
        ["0x" + sccpWordU64Le(witness.parentSlot).hexEncodedString()],
        ["0x" + sccpWordU64Le(witness.bankSignatureCount).hexEncodedString()],
        [witness.parentBankHash],
        [witness.bankHash],
        ["0x" + (try solanaHash32Bytes(witness.blockhash, field: "blockhash")).hexEncodedString()],
        [witness.transactionStatusRoot],
        [witness.accountInclusionRoot],
        [witness.accountsLtHashChecksum],
        [witness.accountsLtHashProofPublicInputsHash],
        [normalized.openedContributionsHash],
        [normalized.residualChecksum],
    ]
}

/// OpenVerify schema descriptor for the AccountsLtHash source-state proof.
public func solanaSccpAccountsLtHashOpenVerifySchemaDescriptor(
    witness: SolanaSccpWitnessInput,
    openedAccounts: SolanaSccpOpenedAccountsLtHashContributionsInput
) throws -> Data {
    let normalized = try normalizeAccountsLtHashProofRequest(witness: witness, openedAccounts: openedAccounts)
    var descriptor = Data()
    descriptor.append(1)
    try appendString(sccpSolanaAccountsLtHashOpenVerifyCircuitIdV1, field: "circuitId", to: &descriptor)
    try appendString(sccpSolanaAccountsLtHashFastpqParameterSetV1, field: "parameterSet", to: &descriptor)
    try appendString(sccpSolanaMainnetGenesisHash, field: "mainnetGenesisHash", to: &descriptor)
    appendU32Le(normalized.witness.sourceDomain, to: &descriptor)
    try appendString("source_state_verifier_id", field: "schemaField", to: &descriptor)
    try appendString(normalized.witness.sourceStateVerifierId, field: "sourceStateVerifierId", to: &descriptor)
    try appendString("source_state_verifier_hash", field: "schemaField", to: &descriptor)
    try descriptor.append(bytesFromHex32(normalized.witness.sourceStateVerifierHash, field: "sourceStateVerifierHash"))
    for requiredInput in [
        "source_domain",
        "mainnet_genesis_hash",
        "finalized_slot",
        "parent_slot",
        "bank_signature_count",
        "parent_bank_hash",
        "bank_hash",
        "blockhash",
        "transaction_status_root",
        "account_inclusion_root",
        "accounts_lt_hash_checksum",
        "accounts_lt_hash_proof_public_inputs_hash",
        "opened_accounts_lt_hash_contributions_hash",
        "opened_accounts_lt_hash_residual_checksum",
    ] {
        try appendString(requiredInput, field: "requiredInput", to: &descriptor)
    }
    return descriptor
}

/// Build the nested Solana AccountsLtHash proof request for UI/mobile source-state provers.
public func buildSolanaSccpAccountsLtHashProofRequest(
    witness: SolanaSccpWitnessInput,
    openedAccounts: SolanaSccpOpenedAccountsLtHashContributionsInput
) throws -> SolanaSccpAccountsLtHashProofRequest {
    let normalized = try normalizeAccountsLtHashProofRequest(witness: witness, openedAccounts: openedAccounts)
    let canonicalWitness = normalized.witness
    let statementBytes = try canonicalSolanaSccpAccountsLtHashProofPublicInputsBytes(
        sourceDomain: canonicalWitness.sourceDomain,
        finalizedSlot: canonicalWitness.finalizedSlot,
        parentSlot: canonicalWitness.parentSlot,
        bankSignatureCount: canonicalWitness.bankSignatureCount,
        parentBankHash: canonicalWitness.parentBankHash,
        bankHash: canonicalWitness.bankHash,
        blockhash: canonicalWitness.blockhash,
        bankHashHardForkData: canonicalWitness.bankHashHardForkData,
        transactionStatusRoot: canonicalWitness.transactionStatusRoot,
        accountInclusionRoot: canonicalWitness.accountInclusionRoot,
        accountsLtHashChecksum: canonicalWitness.accountsLtHashChecksum,
        accountsLtHash: canonicalWitness.accountsLtHash
    )
    let accountCommitmentBytes = try canonicalSolanaSccpAccountsLtHashCommitmentBytes(
        witness: witness,
        openedAccounts: openedAccounts
    )
    let verificationContextBytes = try canonicalSolanaSccpAccountsLtHashVerificationContextBytes(
        witness: witness,
        openedAccounts: openedAccounts
    )
    let dsidHash = try hashBytes(
        prefix: sccpSolanaAccountsLtHashFastpqDsidPrefixV1,
        payload: bytesFromHex32(
            canonicalWitness.accountsLtHashProofPublicInputsHash,
            field: "accountsLtHashProofPublicInputsHash"
        )
    )
    let transitions = [
        SolanaSccpAccountsLtHashFastpqTransition(
            key: sccpSolanaAccountsLtHashFastpqStatementKeyV1,
            operation: "meta_set",
            oldValue: Data(),
            newValue: statementBytes
        ),
        SolanaSccpAccountsLtHashFastpqTransition(
            key: sccpSolanaAccountsLtHashFastpqAccountsKeyV1,
            operation: "meta_set",
            oldValue: Data(),
            newValue: accountCommitmentBytes
        ),
        SolanaSccpAccountsLtHashFastpqTransition(
            key: sccpSolanaAccountsLtHashFastpqOpenedContributionsKeyV1,
            operation: "meta_set",
            oldValue: Data(),
            newValue: try bytesFromHex32(
                normalized.openedContributionsHash,
                field: "openedAccountsLtHashContributionsHash"
            )
        ),
        SolanaSccpAccountsLtHashFastpqTransition(
            key: sccpSolanaAccountsLtHashFastpqResidualKeyV1,
            operation: "meta_set",
            oldValue: Data(),
            newValue: try bytesFromHex32(normalized.residualChecksum, field: "openedAccountsLtHashResidualChecksum")
        ),
        SolanaSccpAccountsLtHashFastpqTransition(
            key: sccpSolanaAccountsLtHashFastpqContextKeyV1,
            operation: "meta_set",
            oldValue: Data(),
            newValue: verificationContextBytes
        ),
    ]
    return SolanaSccpAccountsLtHashProofRequest(
        version: 1,
        proofFamily: "stark-fri-v1",
        circuitId: sccpSolanaAccountsLtHashOpenVerifyCircuitIdV1,
        parameterSet: sccpSolanaAccountsLtHashFastpqParameterSetV1,
        sourceDomain: canonicalWitness.sourceDomain,
        finalizedSlot: String(canonicalWitness.finalizedSlot),
        parentSlot: String(canonicalWitness.parentSlot),
        sourceStateVerifierId: canonicalWitness.sourceStateVerifierId,
        sourceStateVerifierHash: canonicalWitness.sourceStateVerifierHash,
        accountsLtHashProofPublicInputsHash: canonicalWitness.accountsLtHashProofPublicInputsHash,
        openedAccountsLtHashContributionsHash: normalized.openedContributionsHash,
        openedAccountsLtHashResidualChecksum: normalized.residualChecksum,
        statementBytes: statementBytes,
        accountCommitmentBytes: accountCommitmentBytes,
        verificationContextBytes: verificationContextBytes,
        schemaDescriptor: try solanaSccpAccountsLtHashOpenVerifySchemaDescriptor(
            witness: witness,
            openedAccounts: openedAccounts
        ),
        publicInputColumns: try solanaSccpAccountsLtHashPublicInputColumns(
            witness: witness,
            openedAccounts: openedAccounts
        ),
        fastpqPublicInputs: SolanaSccpAccountsLtHashFastpqPublicInputs(
            dsid: "0x" + Data(dsidHash.prefix(16)).hexEncodedString(),
            slot: String(canonicalWitness.finalizedSlot),
            oldRoot: canonicalWitness.parentBankHash,
            newRoot: canonicalWitness.bankHash,
            permRoot: canonicalWitness.accountInclusionRoot,
            txSetHash: canonicalWitness.accountsLtHashProofPublicInputsHash
        ),
        fastpqTransitions: transitions
    )
}

/// Canonical bytes for a Solana account inclusion leaf.
public func canonicalSolanaSccpAccountInclusionLeafBytes(
    finalizedSlot: UInt64,
    address: Data,
    owner: Data,
    lamports: UInt64,
    rentEpoch: UInt64,
    executable: Bool = false,
    dataHash: Data,
    rawDataHash: Data
) throws -> Data {
    guard rawDataHash.count == 32, rawDataHash.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("rawDataHash")
    }
    let openingHashHex = try solanaSccpAccountOpeningHash(
        address: address,
        owner: owner,
        lamports: lamports,
        rentEpoch: rentEpoch,
        executable: executable,
        dataHash: dataHash
    )
    let openingHash = try bytesFromHex32(openingHashHex, field: "openingHash")
    guard address.count == 32, address.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidString("address")
    }
    var out = Data()
    out.append(1)
    appendU64Le(finalizedSlot, to: &out)
    appendBytesVec(address, to: &out)
    out.append(openingHash)
    out.append(rawDataHash)
    return out
}

/// Hash a Solana account inclusion leaf from an opening and exact raw account bytes.
public func solanaSccpAccountInclusionLeafHash(
    finalizedSlot: UInt64,
    address: Data,
    owner: Data,
    lamports: UInt64,
    rentEpoch: UInt64,
    executable: Bool = false,
    dataHash: Data,
    rawData: Data
) throws -> String {
    let rawDataHash = try bytesFromHex32(solanaSccpAccountRawDataHash(rawData), field: "rawDataHash")
    return hashHex(
        prefix: "sccp:solana:account-inclusion-leaf:v1",
        payload: try canonicalSolanaSccpAccountInclusionLeafBytes(
            finalizedSlot: finalizedSlot,
            address: address,
            owner: owner,
            lamports: lamports,
            rentEpoch: rentEpoch,
            executable: executable,
            dataHash: dataHash,
            rawDataHash: rawDataHash
        )
    )
}

private func compareLexicographically(_ left: Data, _ right: Data) -> Int {
    if left.count != right.count {
        return left.count < right.count ? -1 : 1
    }
    for index in 0..<left.count {
        if left[index] != right[index] {
            return left[index] < right[index] ? -1 : 1
        }
    }
    return 0
}

/// Canonical bytes for a directionless Solana account inclusion Merkle node.
public func canonicalSolanaSccpAccountInclusionNodeBytes(left: String, right: String) throws -> Data {
    let leftBytes = try bytesFromHex32(left, field: "left")
    let rightBytes = try bytesFromHex32(right, field: "right")
    guard leftBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("left")
    }
    guard rightBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("right")
    }
    let ordered = compareLexicographically(leftBytes, rightBytes) <= 0
        ? (leftBytes, rightBytes)
        : (rightBytes, leftBytes)
    var out = Data()
    out.append(1)
    out.append(ordered.0)
    out.append(ordered.1)
    return out
}

/// Hash a directionless Solana account inclusion Merkle node.
public func solanaSccpAccountInclusionNodeHash(left: String, right: String) throws -> String {
    hashHex(
        prefix: "sccp:solana:account-inclusion-node:v1",
        payload: try canonicalSolanaSccpAccountInclusionNodeBytes(left: left, right: right)
    )
}

/// Derive a Solana account inclusion root from a leaf and sibling branch.
public func solanaSccpAccountInclusionRootFromBranch(leaf: String, siblings: [String]) throws -> String {
    guard siblings.count <= sccpMaxSourceMerkleBranchNodes else {
        throw SolanaSccpProverError.invalidString("siblings")
    }
    let leafBytes = try bytesFromHex32(leaf, field: "leaf")
    guard leafBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("leaf")
    }
    var current = "0x" + leafBytes.hexEncodedString()
    for (index, sibling) in siblings.enumerated() {
        let siblingHex = try normalizeHex32(sibling, field: "siblings[\(index)]")
        current = try solanaSccpAccountInclusionNodeHash(left: current, right: siblingHex)
    }
    return current
}

/// Build a deterministic Solana account inclusion root and per-leaf branches.
public func solanaSccpAccountInclusionRootAndBranches(
    leaves: [String]
) throws -> (root: String, branches: [[String]]) {
    guard !leaves.isEmpty else {
        throw SolanaSccpProverError.invalidString("leaves")
    }
    var level = try leaves.enumerated().map { index, leaf -> (hash: Data, indexes: [Int]) in
        let hash = try bytesFromHex32(leaf, field: "leaves[\(index)]")
        guard hash.contains(where: { $0 != 0 }) else {
            throw SolanaSccpProverError.invalidHex32("leaves[\(index)]")
        }
        return (hash, [index])
    }
    level.sort { compareLexicographically($0.hash, $1.hash) < 0 }
    for index in 1..<level.count {
        guard level[index - 1].hash != level[index].hash else {
            throw SolanaSccpProverError.invalidString("leaves")
        }
    }
    var branches = Array(repeating: [String](), count: leaves.count)
    while level.count > 1 {
        var next: [(hash: Data, indexes: [Int])] = []
        var index = 0
        while index < level.count {
            if index + 1 >= level.count {
                next.append(level[index])
                index += 2
                continue
            }
            let left = level[index]
            let right = level[index + 1]
            let leftHex = "0x" + left.hash.hexEncodedString()
            let rightHex = "0x" + right.hash.hexEncodedString()
            for leafIndex in left.indexes {
                branches[leafIndex].append(rightHex)
            }
            for leafIndex in right.indexes {
                branches[leafIndex].append(leftHex)
            }
            let parent = try bytesFromHex32(
                solanaSccpAccountInclusionNodeHash(left: leftHex, right: rightHex),
                field: "parent"
            )
            next.append((parent, left.indexes + right.indexes))
            index += 2
        }
        level = next
    }
    return ("0x" + level[0].hash.hexEncodedString(), branches)
}

private func requireUniqueSolanaOpenedAccountAddresses(
    _ openings: [SolanaSccpAccountOpeningInput]
) throws {
    var seenAddresses = Set<Data>()
    for opening in openings {
        guard opening.address.count == 32 else {
            throw SolanaSccpProverError.invalidString("openedAccountAddresses")
        }
        guard seenAddresses.insert(opening.address).inserted else {
            throw SolanaSccpProverError.invalidString("openedAccountAddresses")
        }
    }
}

/// Build the exact opened-account inclusion root and split branches for Solana source proofs.
public func solanaSccpOpenedAccountInclusionWitness(
    _ input: SolanaSccpOpenedAccountInclusionWitnessInput
) throws -> SolanaSccpOpenedAccountInclusionWitness {
    guard input.validatorVoteAccountOpenings.count == input.validatorVoteAccountRawData.count else {
        throw SolanaSccpProverError.invalidString("validatorVoteAccountOpenings")
    }
    guard input.validatorVoteAccountOpenings.count <= sccpSolanaMaxValidators else {
        throw SolanaSccpProverError.invalidString("validatorVoteAccountOpenings")
    }
    guard input.validatorStakeAccountOpenings.count == input.validatorStakeAccountRawData.count else {
        throw SolanaSccpProverError.invalidString("validatorStakeAccountOpenings")
    }
    guard input.validatorStakeAccountOpenings.count <= sccpSolanaMaxValidators else {
        throw SolanaSccpProverError.invalidString("validatorStakeAccountOpenings")
    }
    try requireUniqueSolanaOpenedAccountAddresses(
        input.validatorVoteAccountOpenings
            + input.validatorStakeAccountOpenings
            + [input.stakeHistorySysvarOpening]
    )

    func leaf(opening: SolanaSccpAccountOpeningInput, rawData: Data) throws -> String {
        try solanaSccpAccountInclusionLeafHash(
            finalizedSlot: input.finalizedSlot,
            address: opening.address,
            owner: opening.owner,
            lamports: opening.lamports,
            rentEpoch: opening.rentEpoch,
            executable: opening.executable,
            dataHash: try bytesFromHex32(opening.dataHash, field: "dataHash"),
            rawData: rawData
        )
    }

    let voteLeaves = try input.validatorVoteAccountOpenings.enumerated().map { index, opening in
        try leaf(opening: opening, rawData: input.validatorVoteAccountRawData[index])
    }
    let stakeLeaves = try input.validatorStakeAccountOpenings.enumerated().map { index, opening in
        try leaf(opening: opening, rawData: input.validatorStakeAccountRawData[index])
    }
    let stakeHistoryLeaf = try leaf(
        opening: input.stakeHistorySysvarOpening,
        rawData: input.stakeHistorySysvarRawData
    )
    let witness = try solanaSccpAccountInclusionRootAndBranches(
        leaves: voteLeaves + stakeLeaves + [stakeHistoryLeaf]
    )
    if let expected = input.expectedAccountInclusionRoot,
       try normalizeNonZeroHex32(expected, field: "accountInclusionRoot") != witness.root {
        throw SolanaSccpProverError.invalidString("accountInclusionRoot")
    }
    let voteBranches = Array(witness.branches.prefix(voteLeaves.count))
    let stakeBranches = Array(
        witness.branches.dropFirst(voteLeaves.count).prefix(stakeLeaves.count)
    )
    guard let stakeHistoryBranch = witness.branches.last else {
        throw SolanaSccpProverError.invalidString("stakeHistorySysvarBranch")
    }
    return SolanaSccpOpenedAccountInclusionWitness(
        root: witness.root,
        branches: witness.branches,
        validatorVoteAccountBranches: voteBranches,
        validatorStakeAccountBranches: stakeBranches,
        stakeHistorySysvarBranch: stakeHistoryBranch
    )
}

/// Build the exact opened-account inclusion root and split branches from an LtHash contribution input.
public func solanaSccpOpenedAccountInclusionWitness(
    _ input: SolanaSccpOpenedAccountsLtHashContributionsInput
) throws -> SolanaSccpOpenedAccountInclusionWitness {
    try solanaSccpOpenedAccountInclusionWitness(
        SolanaSccpOpenedAccountInclusionWitnessInput(
            finalizedSlot: input.finalizedSlot,
            validatorVoteAccountOpenings: input.validatorVoteAccountOpenings,
            validatorVoteAccountRawData: input.validatorVoteAccountRawData,
            validatorStakeAccountOpenings: input.validatorStakeAccountOpenings,
            validatorStakeAccountRawData: input.validatorStakeAccountRawData,
            stakeHistorySysvarOpening: input.stakeHistorySysvarOpening,
            stakeHistorySysvarRawData: input.stakeHistorySysvarRawData,
            expectedAccountInclusionRoot: input.accountInclusionRoot
        )
    )
}

/// Canonical bytes for parsed Solana vote account data bound into account openings.
public func canonicalSolanaSccpVoteAccountDataBytes(
    nodePubkey: Data,
    authorizedVoter: Data,
    authorizedWithdrawer: Data,
    inflationRewardsCollector: Data,
    blockRevenueCollector: Data,
    inflationRewardsCommissionBps: UInt16,
    blockRevenueCommissionBps: UInt16,
    pendingDelegatorRewards: UInt64,
    blsPubkeyCompressed: Data = Data(),
    rootSlot: UInt64,
    towerVoteSlots: [UInt64]
) throws -> Data {
    for (field, value) in [
        ("nodePubkey", nodePubkey),
        ("authorizedVoter", authorizedVoter),
        ("authorizedWithdrawer", authorizedWithdrawer),
        ("inflationRewardsCollector", inflationRewardsCollector),
        ("blockRevenueCollector", blockRevenueCollector),
    ] {
        guard value.count == 32, value.contains(where: { $0 != 0 }) else {
            throw SolanaSccpProverError.invalidString(field)
        }
    }
    guard UInt64(inflationRewardsCommissionBps) <= sccpSolanaBasisPointsPerUnit else {
        throw SolanaSccpProverError.invalidString("inflationRewardsCommissionBps")
    }
    guard UInt64(blockRevenueCommissionBps) <= sccpSolanaBasisPointsPerUnit else {
        throw SolanaSccpProverError.invalidString("blockRevenueCommissionBps")
    }
    guard blsPubkeyCompressed.isEmpty || blsPubkeyCompressed.count == sccpSolanaBlsPublicKeyCompressedLen else {
        throw SolanaSccpProverError.invalidString("blsPubkeyCompressed")
    }
    guard blsPubkeyCompressed.isEmpty || blsPubkeyCompressed.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidString("blsPubkeyCompressed")
    }
    guard towerVoteSlots.count == Int(sccpSolanaTowerVoteStackDepth) else {
        throw SolanaSccpProverError.invalidString("towerVoteSlots")
    }
    var previousSlot = rootSlot
    for (index, slot) in towerVoteSlots.enumerated() {
        guard slot > previousSlot else {
            throw SolanaSccpProverError.invalidString("towerVoteSlots[\(index)]")
        }
        previousSlot = slot
    }
    var out = Data()
    out.append(1)
    appendBytesVec(nodePubkey, to: &out)
    appendBytesVec(authorizedVoter, to: &out)
    appendBytesVec(authorizedWithdrawer, to: &out)
    appendBytesVec(inflationRewardsCollector, to: &out)
    appendBytesVec(blockRevenueCollector, to: &out)
    appendU16Le(inflationRewardsCommissionBps, to: &out)
    appendU16Le(blockRevenueCommissionBps, to: &out)
    appendU64Le(pendingDelegatorRewards, to: &out)
    appendBytesVec(blsPubkeyCompressed, to: &out)
    appendU64Le(rootSlot, to: &out)
    appendU32Le(UInt32(towerVoteSlots.count), to: &out)
    for slot in towerVoteSlots {
        appendU64Le(slot, to: &out)
    }
    return out
}

/// Hash parsed Solana vote account data bound into account openings.
public func solanaSccpVoteAccountDataHash(
    nodePubkey: Data,
    authorizedVoter: Data,
    authorizedWithdrawer: Data,
    inflationRewardsCollector: Data,
    blockRevenueCollector: Data,
    inflationRewardsCommissionBps: UInt16,
    blockRevenueCommissionBps: UInt16,
    pendingDelegatorRewards: UInt64,
    blsPubkeyCompressed: Data = Data(),
    rootSlot: UInt64,
    towerVoteSlots: [UInt64]
) throws -> String {
    hashHex(
        prefix: "sccp:solana:vote-account-data:v1",
        payload: try canonicalSolanaSccpVoteAccountDataBytes(
            nodePubkey: nodePubkey,
            authorizedVoter: authorizedVoter,
            authorizedWithdrawer: authorizedWithdrawer,
            inflationRewardsCollector: inflationRewardsCollector,
            blockRevenueCollector: blockRevenueCollector,
            inflationRewardsCommissionBps: inflationRewardsCommissionBps,
            blockRevenueCommissionBps: blockRevenueCommissionBps,
            pendingDelegatorRewards: pendingDelegatorRewards,
            blsPubkeyCompressed: blsPubkeyCompressed,
            rootSlot: rootSlot,
            towerVoteSlots: towerVoteSlots
        )
    )
}

/// Parse raw Solana `VoteStateVersions::V1_14_11`, `V3`, or `V4` account data into SCCP vote fields.
public func solanaSccpVoteAccountDataFromRawVoteState(
    rawData: Data,
    epoch: UInt64,
    voteAccountAddress: Data
) throws -> SolanaSccpParsedVoteStateAccountData {
    guard rawData.count == sccpSolanaVoteStateAccountDataLen else {
        throw SolanaSccpProverError.invalidString("rawData")
    }
    guard voteAccountAddress.count == 32, voteAccountAddress.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidString("voteAccountAddress")
    }
    var cursor = 0

    func readU8(_ field: String) throws -> UInt8 {
        guard cursor + 1 <= rawData.count else {
            throw SolanaSccpProverError.invalidString(field)
        }
        let value = rawData[cursor]
        cursor += 1
        return value
    }

    func readU32(_ field: String) throws -> UInt32 {
        let value = try readU32Le(rawData, offset: cursor, field: field)
        cursor += 4
        return value
    }

    func readU16(_ field: String) throws -> UInt16 {
        let value = try readU16Le(rawData, offset: cursor, field: field)
        cursor += 2
        return value
    }

    func readU64(_ field: String) throws -> UInt64 {
        let value = try readU64Le(rawData, offset: cursor, field: field)
        cursor += 8
        return value
    }

    func readPubkey(_ field: String) throws -> Data {
        guard cursor + 32 <= rawData.count else {
            throw SolanaSccpProverError.invalidString(field)
        }
        let value = rawData[cursor..<(cursor + 32)]
        cursor += 32
        return value
    }

    let variant = try readU32("voteStateVariant")
    let hasLatency: Bool
    switch variant {
    case sccpSolanaVoteStateV1_14_11Discriminant:
        hasLatency = false
    case sccpSolanaVoteStateV3Discriminant:
        hasLatency = true
    case sccpSolanaVoteStateV4Discriminant:
        hasLatency = true
    default:
        throw SolanaSccpProverError.invalidString("rawData")
    }

    let nodePubkey = try readPubkey("nodePubkey")
    let authorizedWithdrawer = try readPubkey("authorizedWithdrawer")
    let inflationRewardsCollector: Data
    let blockRevenueCollector: Data
    let inflationRewardsCommissionBps: UInt16
    let blockRevenueCommissionBps: UInt16
    let pendingDelegatorRewards: UInt64
    let blsPubkeyCompressed: Data
    if variant == sccpSolanaVoteStateV4Discriminant {
        inflationRewardsCollector = try readPubkey("inflationRewardsCollector")
        blockRevenueCollector = try readPubkey("blockRevenueCollector")
        inflationRewardsCommissionBps = try readU16("inflationRewardsCommissionBps")
        blockRevenueCommissionBps = try readU16("blockRevenueCommissionBps")
        guard UInt64(inflationRewardsCommissionBps) <= sccpSolanaBasisPointsPerUnit else {
            throw SolanaSccpProverError.invalidString("inflationRewardsCommissionBps")
        }
        guard UInt64(blockRevenueCommissionBps) <= sccpSolanaBasisPointsPerUnit else {
            throw SolanaSccpProverError.invalidString("blockRevenueCommissionBps")
        }
        pendingDelegatorRewards = try readU64("pendingDelegatorRewards")
        let blsVariant = try readU8("blsPubkeyCompressed")
        if blsVariant == 0 {
            blsPubkeyCompressed = Data()
        } else if blsVariant == 1 {
            guard cursor + sccpSolanaBlsPublicKeyCompressedLen <= rawData.count else {
                throw SolanaSccpProverError.invalidString("blsPubkeyCompressed")
            }
            blsPubkeyCompressed = rawData[cursor..<(cursor + sccpSolanaBlsPublicKeyCompressedLen)]
            cursor += sccpSolanaBlsPublicKeyCompressedLen
        } else {
            throw SolanaSccpProverError.invalidString("blsPubkeyCompressed")
        }
    } else {
        let commission = try readU8("commission")
        inflationRewardsCollector = voteAccountAddress
        blockRevenueCollector = nodePubkey
        inflationRewardsCommissionBps = UInt16(commission) * 100
        blockRevenueCommissionBps = UInt16(sccpSolanaBasisPointsPerUnit)
        pendingDelegatorRewards = 0
        blsPubkeyCompressed = Data()
    }
    guard try readU64("towerVoteSlots") == sccpSolanaTowerVoteStackDepth else {
        throw SolanaSccpProverError.invalidString("towerVoteSlots")
    }
    var towerVoteSlots: [UInt64] = []
    let depth = Int(sccpSolanaTowerVoteStackDepth)
    towerVoteSlots.reserveCapacity(depth)
    for index in 0..<depth {
        if hasLatency {
            _ = try readU8("towerVoteSlots[\(index)].latency")
        }
        let slot = try readU64("towerVoteSlots[\(index)].slot")
        let confirmationCount = try readU32("towerVoteSlots[\(index)].confirmationCount")
        guard confirmationCount == UInt32(depth - index) else {
            throw SolanaSccpProverError.invalidString("towerVoteSlots[\(index)]")
        }
        towerVoteSlots.append(slot)
    }

    guard try readU8("rootSlot") == 1 else {
        throw SolanaSccpProverError.invalidString("rootSlot")
    }
    let rootSlot = try readU64("rootSlot")
    var previousTowerSlot = rootSlot
    for (index, slot) in towerVoteSlots.enumerated() {
        guard previousTowerSlot < slot else {
            throw SolanaSccpProverError.invalidString("towerVoteSlots[\(index)]")
        }
        previousTowerSlot = slot
    }
    let authorizedVoterCount = try readU64("authorizedVoters")
    let authorizedVoterLimit: Int
    if variant == sccpSolanaVoteStateV4Discriminant {
        authorizedVoterLimit = sccpSolanaVoteStateV4AuthorizedVoters
    } else {
        authorizedVoterLimit = sccpSolanaVoteStatePriorVoters
    }
    guard authorizedVoterCount > 0, authorizedVoterCount <= UInt64(authorizedVoterLimit) else {
        throw SolanaSccpProverError.invalidString("authorizedVoters")
    }
    var previousAuthorizedEpoch: UInt64?
    var selectedAuthorizedVoter: Data?
    for index in 0..<Int(authorizedVoterCount) {
        let authorizedEpoch = try readU64("authorizedVoters[\(index)].epoch")
        if let previousAuthorizedEpoch {
            guard previousAuthorizedEpoch < authorizedEpoch else {
                throw SolanaSccpProverError.invalidString("authorizedVoters")
            }
        }
        let authorizedVoter = try readPubkey("authorizedVoters[\(index)].authorizedVoter")
        guard authorizedVoter.contains(where: { $0 != 0 }) else {
            throw SolanaSccpProverError.invalidString("authorizedVoters[\(index)].authorizedVoter")
        }
        if authorizedEpoch <= epoch {
            selectedAuthorizedVoter = authorizedVoter
        }
        previousAuthorizedEpoch = authorizedEpoch
    }
    guard let authorizedVoter = selectedAuthorizedVoter else {
        throw SolanaSccpProverError.invalidString("authorizedVoters")
    }
    if variant != sccpSolanaVoteStateV4Discriminant {
        for index in 0..<sccpSolanaVoteStatePriorVoters {
            let priorVoter = try readPubkey("priorVoters[\(index)].pubkey")
            let fromEpoch = try readU64("priorVoters[\(index)].fromEpoch")
            let untilEpoch = try readU64("priorVoters[\(index)].untilEpoch")
            if !priorVoter.contains(where: { $0 != 0 }) {
                guard fromEpoch == 0, untilEpoch == 0 else {
                    throw SolanaSccpProverError.invalidString("priorVoters[\(index)]")
                }
            } else {
                guard fromEpoch < untilEpoch else {
                    throw SolanaSccpProverError.invalidString("priorVoters[\(index)]")
                }
            }
        }
        let priorVotersIndex = try readU64("priorVoters.index")
        let priorVotersIsEmpty = try readU8("priorVoters.isEmpty")
        guard priorVotersIndex < UInt64(sccpSolanaVoteStatePriorVoters),
              priorVotersIsEmpty == 0 || priorVotersIsEmpty == 1 else {
            throw SolanaSccpProverError.invalidString("priorVoters")
        }
    }
    let epochCreditCount = try readU64("epochCredits")
    guard epochCreditCount <= UInt64(sccpSolanaVoteStateMaxEpochCredits) else {
        throw SolanaSccpProverError.invalidString("epochCredits")
    }
    var previousEpochCreditEpoch: UInt64?
    var previousEpochCreditTotal: UInt64?
    for index in 0..<Int(epochCreditCount) {
        let creditEpoch = try readU64("epochCredits[\(index)].epoch")
        let credits = try readU64("epochCredits[\(index)].credits")
        let previousCredits = try readU64("epochCredits[\(index)].previousCredits")
        guard creditEpoch <= epoch else {
            throw SolanaSccpProverError.invalidString("epochCredits")
        }
        if let previousEpochCreditEpoch {
            guard previousEpochCreditEpoch < creditEpoch else {
                throw SolanaSccpProverError.invalidString("epochCredits")
            }
        }
        guard previousCredits <= credits else {
            throw SolanaSccpProverError.invalidString("epochCredits")
        }
        if let previousEpochCreditTotal {
            guard previousEpochCreditTotal <= previousCredits else {
                throw SolanaSccpProverError.invalidString("epochCredits")
            }
        }
        previousEpochCreditEpoch = creditEpoch
        previousEpochCreditTotal = credits
    }
    let lastTimestampSlot = try readU64("lastTimestamp.slot")
    let lastTimestamp = try readU64("lastTimestamp.timestamp")
    let lastTowerVoteSlot = towerVoteSlots[towerVoteSlots.count - 1]
    if lastTimestampSlot == 0 {
        guard lastTimestamp == 0 else {
            throw SolanaSccpProverError.invalidString("lastTimestamp")
        }
    } else {
        guard lastTimestampSlot <= lastTowerVoteSlot,
              lastTimestamp <= UInt64(Int64.max) else {
            throw SolanaSccpProverError.invalidString("lastTimestamp")
        }
    }
    guard rawData[cursor..<rawData.count].allSatisfy({ $0 == 0 }) else {
        throw SolanaSccpProverError.invalidString("rawDataPadding")
    }
    let parsed = SolanaSccpParsedVoteStateAccountData(
        nodePubkey: nodePubkey,
        authorizedVoter: authorizedVoter,
        authorizedWithdrawer: authorizedWithdrawer,
        inflationRewardsCollector: inflationRewardsCollector,
        blockRevenueCollector: blockRevenueCollector,
        inflationRewardsCommissionBps: inflationRewardsCommissionBps,
        blockRevenueCommissionBps: blockRevenueCommissionBps,
        pendingDelegatorRewards: pendingDelegatorRewards,
        blsPubkeyCompressed: blsPubkeyCompressed,
        rootSlot: rootSlot,
        towerVoteSlots: towerVoteSlots
    )
    _ = try canonicalSolanaSccpVoteAccountDataBytes(
        nodePubkey: parsed.nodePubkey,
        authorizedVoter: parsed.authorizedVoter,
        authorizedWithdrawer: parsed.authorizedWithdrawer,
        inflationRewardsCollector: parsed.inflationRewardsCollector,
        blockRevenueCollector: parsed.blockRevenueCollector,
        inflationRewardsCommissionBps: parsed.inflationRewardsCommissionBps,
        blockRevenueCommissionBps: parsed.blockRevenueCommissionBps,
        pendingDelegatorRewards: parsed.pendingDelegatorRewards,
        blsPubkeyCompressed: parsed.blsPubkeyCompressed,
        rootSlot: parsed.rootSlot,
        towerVoteSlots: parsed.towerVoteSlots
    )
    return parsed
}

/// Hash raw Solana vote account data using the SCCP vote account transcript.
public func solanaSccpVoteAccountDataHashFromRawVoteState(
    rawData: Data,
    epoch: UInt64,
    voteAccountAddress: Data
) throws -> String {
    let parsed = try solanaSccpVoteAccountDataFromRawVoteState(
        rawData: rawData,
        epoch: epoch,
        voteAccountAddress: voteAccountAddress
    )
    return try solanaSccpVoteAccountDataHash(
        nodePubkey: parsed.nodePubkey,
        authorizedVoter: parsed.authorizedVoter,
        authorizedWithdrawer: parsed.authorizedWithdrawer,
        inflationRewardsCollector: parsed.inflationRewardsCollector,
        blockRevenueCollector: parsed.blockRevenueCollector,
        inflationRewardsCommissionBps: parsed.inflationRewardsCommissionBps,
        blockRevenueCommissionBps: parsed.blockRevenueCommissionBps,
        pendingDelegatorRewards: parsed.pendingDelegatorRewards,
        blsPubkeyCompressed: parsed.blsPubkeyCompressed,
        rootSlot: parsed.rootSlot,
        towerVoteSlots: parsed.towerVoteSlots
    )
}

public func solanaSccpVoteAccountDataFromRawVoteStateV1OrV3(
    rawData: Data,
    epoch: UInt64,
    voteAccountAddress: Data
) throws -> SolanaSccpParsedVoteStateV1OrV3AccountData {
    try solanaSccpVoteAccountDataFromRawVoteState(
        rawData: rawData,
        epoch: epoch,
        voteAccountAddress: voteAccountAddress
    )
}

public func solanaSccpVoteAccountDataHashFromRawVoteStateV1OrV3(
    rawData: Data,
    epoch: UInt64,
    voteAccountAddress: Data
) throws -> String {
    try solanaSccpVoteAccountDataHashFromRawVoteState(
        rawData: rawData,
        epoch: epoch,
        voteAccountAddress: voteAccountAddress
    )
}

/// Canonical bytes for parsed Solana stake account data bound into account openings.
public func canonicalSolanaSccpStakeAccountDataBytes(
    staker: Data,
    withdrawer: Data,
    voterPubkey: Data,
    delegatedStake: UInt64,
    activationEpoch: UInt64,
    deactivationEpoch: UInt64,
    warmupCooldownRateBytes: Data = Data([0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f]),
    creditsObserved: UInt64 = 0,
    stakeFlags: UInt8 = 0
) throws -> Data {
    for (field, value) in [
        ("staker", staker),
        ("withdrawer", withdrawer),
        ("voterPubkey", voterPubkey),
    ] {
        guard value.count == 32, value.contains(where: { $0 != 0 }) else {
            throw SolanaSccpProverError.invalidString(field)
        }
    }
    guard delegatedStake > 0 else {
        throw SolanaSccpProverError.invalidString("delegatedStake")
    }
    guard deactivationEpoch > activationEpoch else {
        throw SolanaSccpProverError.invalidString("deactivationEpoch")
    }
    guard warmupCooldownRateBytes.count == sccpSolanaStakeStateV2WarmupCooldownRateBytes else {
        throw SolanaSccpProverError.invalidString("warmupCooldownRateBytes")
    }
    guard warmupCooldownRateBytes == sccpSolanaStakeStateV2LegacyWarmupCooldownRateBytes ||
        warmupCooldownRateBytes == sccpSolanaStakeStateV2CurrentWarmupCooldownRateBytes else {
        throw SolanaSccpProverError.invalidString("warmupCooldownRateBytes")
    }
    guard (stakeFlags & ~sccpSolanaStakeStateV2KnownFlagsMask) == 0 else {
        throw SolanaSccpProverError.invalidString("stakeFlags")
    }
    var out = Data()
    out.append(1)
    appendBytesVec(staker, to: &out)
    appendBytesVec(withdrawer, to: &out)
    appendBytesVec(voterPubkey, to: &out)
    appendU64Le(delegatedStake, to: &out)
    appendU64Le(activationEpoch, to: &out)
    appendU64Le(deactivationEpoch, to: &out)
    appendBytesVec(warmupCooldownRateBytes, to: &out)
    appendU64Le(creditsObserved, to: &out)
    out.append(stakeFlags)
    return out
}

/// Hash parsed Solana stake account data bound into account openings.
public func solanaSccpStakeAccountDataHash(
    staker: Data,
    withdrawer: Data,
    voterPubkey: Data,
    delegatedStake: UInt64,
    activationEpoch: UInt64,
    deactivationEpoch: UInt64,
    warmupCooldownRateBytes: Data = Data([0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f]),
    creditsObserved: UInt64 = 0,
    stakeFlags: UInt8 = 0
) throws -> String {
    hashHex(
        prefix: "sccp:solana:stake-account-data:v1",
        payload: try canonicalSolanaSccpStakeAccountDataBytes(
            staker: staker,
            withdrawer: withdrawer,
            voterPubkey: voterPubkey,
            delegatedStake: delegatedStake,
            activationEpoch: activationEpoch,
            deactivationEpoch: deactivationEpoch,
            warmupCooldownRateBytes: warmupCooldownRateBytes,
            creditsObserved: creditsObserved,
            stakeFlags: stakeFlags
        )
    )
}

/// Parse raw Solana `StakeStateV2::Stake` account data into SCCP stake account fields.
public func solanaSccpStakeAccountDataFromRawStakeStateV2(
    rawData: Data
) throws -> SolanaSccpParsedStakeStateV2StakeAccountData {
    guard rawData.count == sccpSolanaStakeStateV2StakeAccountDataLen else {
        throw SolanaSccpProverError.invalidString("rawData")
    }
    guard try readU32Le(rawData, offset: 0, field: "rawData") == sccpSolanaStakeStateV2StakeDiscriminant else {
        throw SolanaSccpProverError.invalidString("rawData")
    }
    let paddingStart = sccpSolanaStakeStateV2FlagOffset + 1
    guard !rawData[paddingStart..<rawData.count].contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidString("rawData")
    }
    let stakeFlags = rawData[sccpSolanaStakeStateV2FlagOffset]
    guard (stakeFlags & ~sccpSolanaStakeStateV2KnownFlagsMask) == 0 else {
        throw SolanaSccpProverError.invalidString("stakeFlags")
    }
    let parsed = SolanaSccpParsedStakeStateV2StakeAccountData(
        staker: rawData[
            sccpSolanaStakeStateV2StakerOffset..<(sccpSolanaStakeStateV2StakerOffset + 32)
        ],
        withdrawer: rawData[
            sccpSolanaStakeStateV2WithdrawerOffset..<(sccpSolanaStakeStateV2WithdrawerOffset + 32)
        ],
        voterPubkey: rawData[
            sccpSolanaStakeStateV2VoterPubkeyOffset..<(sccpSolanaStakeStateV2VoterPubkeyOffset + 32)
        ],
        delegatedStake: try readU64Le(
            rawData,
            offset: sccpSolanaStakeStateV2DelegatedStakeOffset,
            field: "delegatedStake"
        ),
        activationEpoch: try readU64Le(
            rawData,
            offset: sccpSolanaStakeStateV2ActivationEpochOffset,
            field: "activationEpoch"
        ),
        deactivationEpoch: try readU64Le(
            rawData,
            offset: sccpSolanaStakeStateV2DeactivationEpochOffset,
            field: "deactivationEpoch"
        ),
        warmupCooldownRateBytes: rawData[
            sccpSolanaStakeStateV2WarmupCooldownRateOffset..<(
                sccpSolanaStakeStateV2WarmupCooldownRateOffset + sccpSolanaStakeStateV2WarmupCooldownRateBytes
            )
        ],
        creditsObserved: try readU64Le(
            rawData,
            offset: sccpSolanaStakeStateV2CreditsObservedOffset,
            field: "creditsObserved"
        ),
        stakeFlags: stakeFlags
    )
    _ = try canonicalSolanaSccpStakeAccountDataBytes(
        staker: parsed.staker,
        withdrawer: parsed.withdrawer,
        voterPubkey: parsed.voterPubkey,
        delegatedStake: parsed.delegatedStake,
        activationEpoch: parsed.activationEpoch,
        deactivationEpoch: parsed.deactivationEpoch,
        warmupCooldownRateBytes: parsed.warmupCooldownRateBytes,
        creditsObserved: parsed.creditsObserved,
        stakeFlags: parsed.stakeFlags
    )
    return parsed
}

/// Hash raw Solana `StakeStateV2::Stake` account data using the SCCP stake account transcript.
public func solanaSccpStakeAccountDataHashFromRawStakeStateV2(rawData: Data) throws -> String {
    let parsed = try solanaSccpStakeAccountDataFromRawStakeStateV2(rawData: rawData)
    return try solanaSccpStakeAccountDataHash(
        staker: parsed.staker,
        withdrawer: parsed.withdrawer,
        voterPubkey: parsed.voterPubkey,
        delegatedStake: parsed.delegatedStake,
        activationEpoch: parsed.activationEpoch,
        deactivationEpoch: parsed.deactivationEpoch,
        warmupCooldownRateBytes: parsed.warmupCooldownRateBytes,
        creditsObserved: parsed.creditsObserved,
        stakeFlags: parsed.stakeFlags
    )
}

/// Canonical bytes for Solana vote/stake account state openings bound into SCCP finality contexts.
public func canonicalSolanaSccpStakeAccountStateBytes(
    epoch: UInt64,
    validatorPublicKeys: [Data],
    validatorStakes: [UInt64],
    validatorActivationEpochs: [UInt64],
    validatorDeactivationEpochs: [UInt64],
    validatorVoteAccountAddresses: [Data],
    validatorStakeAccountAddresses: [Data],
    validatorVoteAccountHashes: [Data],
    validatorStakeAccountHashes: [Data]
) throws -> Data {
    guard validatorVoteAccountAddresses.count == validatorPublicKeys.count else {
        throw SolanaSccpProverError.invalidString("validatorVoteAccountAddresses")
    }
    guard validatorStakeAccountAddresses.count == validatorPublicKeys.count else {
        throw SolanaSccpProverError.invalidString("validatorStakeAccountAddresses")
    }
    guard validatorVoteAccountHashes.count == validatorPublicKeys.count else {
        throw SolanaSccpProverError.invalidString("validatorVoteAccountHashes")
    }
    guard validatorStakeAccountHashes.count == validatorPublicKeys.count else {
        throw SolanaSccpProverError.invalidString("validatorStakeAccountHashes")
    }
    let stakeActivationBytes = try canonicalSolanaSccpStakeActivationBytes(
        epoch: epoch,
        validatorPublicKeys: validatorPublicKeys,
        validatorStakes: validatorStakes,
        validatorActivationEpochs: validatorActivationEpochs,
        validatorDeactivationEpochs: validatorDeactivationEpochs
    )
    let stakeActivationHash = hashBytes(
        prefix: "sccp:solana:stake-activation:v1",
        payload: stakeActivationBytes
    )
    var seenVoteAccounts = Set<Data>()
    var seenStakeAccounts = Set<Data>()
    var out = Data()
    out.append(1)
    appendU64Le(epoch, to: &out)
    out.append(stakeActivationHash)
    appendU32Le(UInt32(validatorPublicKeys.count), to: &out)
    for index in validatorPublicKeys.indices {
        let voteAccount = validatorVoteAccountAddresses[index]
        let stakeAccount = validatorStakeAccountAddresses[index]
        let voteAccountHash = validatorVoteAccountHashes[index]
        let stakeAccountHash = validatorStakeAccountHashes[index]
        guard voteAccount.count == 32 else {
            throw SolanaSccpProverError.invalidHex32("validatorVoteAccountAddresses[\(index)]")
        }
        guard voteAccount.contains(where: { $0 != 0 }) else {
            throw SolanaSccpProverError.invalidString("validatorVoteAccountAddresses[\(index)]")
        }
        guard stakeAccount.count == 32 else {
            throw SolanaSccpProverError.invalidHex32("validatorStakeAccountAddresses[\(index)]")
        }
        guard stakeAccount.contains(where: { $0 != 0 }) else {
            throw SolanaSccpProverError.invalidString("validatorStakeAccountAddresses[\(index)]")
        }
        guard voteAccount != stakeAccount else {
            throw SolanaSccpProverError.invalidString("validatorStakeAccountAddresses[\(index)]")
        }
        guard !seenStakeAccounts.contains(voteAccount) else {
            throw SolanaSccpProverError.invalidString("validatorVoteAccountAddresses")
        }
        guard !seenVoteAccounts.contains(stakeAccount) else {
            throw SolanaSccpProverError.invalidString("validatorStakeAccountAddresses")
        }
        guard !seenVoteAccounts.contains(voteAccount) else {
            throw SolanaSccpProverError.invalidString("validatorVoteAccountAddresses")
        }
        guard !seenStakeAccounts.contains(stakeAccount) else {
            throw SolanaSccpProverError.invalidString("validatorStakeAccountAddresses")
        }
        guard voteAccountHash.count == 32, voteAccountHash.contains(where: { $0 != 0 }) else {
            throw SolanaSccpProverError.invalidHex32("validatorVoteAccountHashes[\(index)]")
        }
        guard stakeAccountHash.count == 32, stakeAccountHash.contains(where: { $0 != 0 }) else {
            throw SolanaSccpProverError.invalidHex32("validatorStakeAccountHashes[\(index)]")
        }
        seenVoteAccounts.insert(voteAccount)
        seenStakeAccounts.insert(stakeAccount)
        appendBytesVec(validatorPublicKeys[index], to: &out)
        appendU64Le(validatorStakes[index], to: &out)
        appendU64Le(validatorActivationEpochs[index], to: &out)
        appendU64Le(validatorDeactivationEpochs[index], to: &out)
        appendBytesVec(voteAccount, to: &out)
        appendBytesVec(stakeAccount, to: &out)
        out.append(voteAccountHash)
        out.append(stakeAccountHash)
    }
    return out
}

/// Hash Solana vote/stake account state openings used by SCCP finality contexts.
public func solanaSccpStakeAccountStateHash(
    epoch: UInt64,
    validatorPublicKeys: [Data],
    validatorStakes: [UInt64],
    validatorActivationEpochs: [UInt64],
    validatorDeactivationEpochs: [UInt64],
    validatorVoteAccountAddresses: [Data],
    validatorStakeAccountAddresses: [Data],
    validatorVoteAccountHashes: [Data],
    validatorStakeAccountHashes: [Data]
) throws -> String {
    hashHex(
        prefix: "sccp:solana:stake-account-state:v1",
        payload: try canonicalSolanaSccpStakeAccountStateBytes(
            epoch: epoch,
            validatorPublicKeys: validatorPublicKeys,
            validatorStakes: validatorStakes,
            validatorActivationEpochs: validatorActivationEpochs,
            validatorDeactivationEpochs: validatorDeactivationEpochs,
            validatorVoteAccountAddresses: validatorVoteAccountAddresses,
            validatorStakeAccountAddresses: validatorStakeAccountAddresses,
            validatorVoteAccountHashes: validatorVoteAccountHashes,
            validatorStakeAccountHashes: validatorStakeAccountHashes
        )
    )
}

private struct SolanaSccpBigUInt: Comparable {
    private var limbs: [UInt32]

    init(_ value: UInt64) {
        let low = UInt32(value & 0xffff_ffff)
        let high = UInt32(value >> 32)
        if high == 0 {
            self.limbs = low == 0 ? [] : [low]
        } else {
            self.limbs = [low, high]
        }
    }

    private init(limbs: [UInt32]) {
        self.limbs = limbs
        normalize()
    }

    static func < (lhs: SolanaSccpBigUInt, rhs: SolanaSccpBigUInt) -> Bool {
        if lhs.limbs.count != rhs.limbs.count {
            return lhs.limbs.count < rhs.limbs.count
        }
        if lhs.limbs.isEmpty {
            return false
        }
        for index in stride(from: lhs.limbs.count - 1, through: 0, by: -1) {
            if lhs.limbs[index] != rhs.limbs[index] {
                return lhs.limbs[index] < rhs.limbs[index]
            }
            if index == 0 {
                break
            }
        }
        return false
    }

    static func == (lhs: SolanaSccpBigUInt, rhs: SolanaSccpBigUInt) -> Bool {
        lhs.limbs == rhs.limbs
    }

    func multiplied(by value: UInt64) -> SolanaSccpBigUInt {
        if value == 0 || limbs.isEmpty {
            return SolanaSccpBigUInt(0)
        }
        let low = UInt32(value & 0xffff_ffff)
        let high = UInt32(value >> 32)
        var result = multiplied(byWord: low)
        if high != 0 {
            var highPart = multiplied(byWord: high)
            if !highPart.limbs.isEmpty {
                highPart.limbs.insert(0, at: 0)
                result.add(highPart)
            }
        }
        return result
    }

    private func multiplied(byWord word: UInt32) -> SolanaSccpBigUInt {
        if word == 0 || limbs.isEmpty {
            return SolanaSccpBigUInt(0)
        }
        var out: [UInt32] = []
        var carry: UInt64 = 0
        for limb in limbs {
            let product = UInt64(limb) * UInt64(word) + carry
            out.append(UInt32(product & 0xffff_ffff))
            carry = product >> 32
        }
        if carry > 0 {
            out.append(UInt32(carry))
        }
        return SolanaSccpBigUInt(limbs: out)
    }

    private mutating func add(_ other: SolanaSccpBigUInt) {
        let count = max(limbs.count, other.limbs.count)
        if limbs.count < count {
            limbs.append(contentsOf: repeatElement(0, count: count - limbs.count))
        }
        var carry: UInt64 = 0
        for index in 0..<count {
            let lhs = UInt64(limbs[index])
            let rhs = index < other.limbs.count ? UInt64(other.limbs[index]) : 0
            let sum = lhs + rhs + carry
            limbs[index] = UInt32(sum & 0xffff_ffff)
            carry = sum >> 32
        }
        if carry > 0 {
            limbs.append(UInt32(carry))
        }
        normalize()
    }

    private mutating func normalize() {
        while limbs.last == 0 {
            limbs.removeLast()
        }
    }
}

private struct SolanaSccpStakeActivationStatus {
    let effective: UInt64
    let activating: UInt64
    let deactivating: UInt64
}

private func solanaStakeHistoryEntry(
    in entries: [SolanaSccpStakeHistoryEntry],
    epoch: UInt64
) -> SolanaSccpStakeHistoryEntry? {
    entries.first { $0.epoch == epoch }
}

private func solanaStakeChangeAllowance(
    accountPortion: UInt64,
    clusterPortion: UInt64,
    clusterEffective: UInt64
) -> UInt64 {
    guard accountPortion > 0, clusterPortion > 0, clusterEffective > 0 else {
        return 0
    }
    let numerator = SolanaSccpBigUInt(accountPortion)
        .multiplied(by: clusterEffective)
        .multiplied(by: sccpSolanaTowerWarmupCooldownRateBps)
    let cappedDenominator = SolanaSccpBigUInt(accountPortion)
        .multiplied(by: clusterPortion)
        .multiplied(by: sccpSolanaBasisPointsPerUnit)
    if numerator >= cappedDenominator {
        return accountPortion
    }

    var low: UInt64 = 0
    var high = accountPortion
    while low < high {
        let distance = high - low
        let mid = low + distance / 2 + distance % 2
        let candidate = SolanaSccpBigUInt(mid)
            .multiplied(by: clusterPortion)
            .multiplied(by: sccpSolanaBasisPointsPerUnit)
        if candidate <= numerator {
            low = mid
        } else {
            high = mid - 1
        }
    }
    return low
}

private func solanaStakeAndActivatingV2(
    targetEpoch: UInt64,
    delegatedStake: UInt64,
    activationEpoch: UInt64,
    deactivationEpoch: UInt64,
    stakeHistoryEntries: [SolanaSccpStakeHistoryEntry]
) -> (effective: UInt64, activating: UInt64)? {
    if activationEpoch == UInt64.max {
        return (delegatedStake, 0)
    }
    if activationEpoch == deactivationEpoch {
        return (0, 0)
    }
    if targetEpoch == activationEpoch {
        return (0, delegatedStake)
    }
    if targetEpoch < activationEpoch {
        return (0, 0)
    }
    guard var previousClusterStake = solanaStakeHistoryEntry(
        in: stakeHistoryEntries,
        epoch: activationEpoch
    ) else {
        return (delegatedStake, 0)
    }

    var previousEpoch = activationEpoch
    var activatedStakeAmount: UInt64 = 0
    while true {
        let nextEpoch = previousEpoch.addingReportingOverflow(1)
        guard !nextEpoch.overflow else {
            return nil
        }
        let currentEpoch = nextEpoch.partialValue
        if previousClusterStake.activating == 0 {
            break
        }
        let remainingActivatingStake = delegatedStake - activatedStakeAmount
        let newlyEffectiveStake = max(
            solanaStakeChangeAllowance(
                accountPortion: remainingActivatingStake,
                clusterPortion: previousClusterStake.activating,
                clusterEffective: previousClusterStake.effective
            ),
            1
        )
        let activatedSum = activatedStakeAmount.addingReportingOverflow(newlyEffectiveStake)
        activatedStakeAmount = activatedSum.overflow
            ? delegatedStake
            : min(activatedSum.partialValue, delegatedStake)
        if activatedStakeAmount >= delegatedStake {
            activatedStakeAmount = delegatedStake
            break
        }
        if currentEpoch >= targetEpoch || currentEpoch >= deactivationEpoch {
            break
        }
        guard let currentClusterStake = solanaStakeHistoryEntry(
            in: stakeHistoryEntries,
            epoch: currentEpoch
        ) else {
            break
        }
        previousEpoch = currentEpoch
        previousClusterStake = currentClusterStake
    }

    return (activatedStakeAmount, delegatedStake - activatedStakeAmount)
}

private func solanaDelegationStakeStatusV2(
    targetEpoch: UInt64,
    delegatedStake: UInt64,
    activationEpoch: UInt64,
    deactivationEpoch: UInt64,
    stakeHistoryEntries: [SolanaSccpStakeHistoryEntry]
) -> SolanaSccpStakeActivationStatus? {
    guard let stakeAndActivating = solanaStakeAndActivatingV2(
        targetEpoch: targetEpoch,
        delegatedStake: delegatedStake,
        activationEpoch: activationEpoch,
        deactivationEpoch: deactivationEpoch,
        stakeHistoryEntries: stakeHistoryEntries
    ) else {
        return nil
    }
    if targetEpoch < deactivationEpoch {
        return SolanaSccpStakeActivationStatus(
            effective: stakeAndActivating.effective,
            activating: stakeAndActivating.activating,
            deactivating: 0
        )
    }
    if targetEpoch == deactivationEpoch {
        return SolanaSccpStakeActivationStatus(
            effective: stakeAndActivating.effective,
            activating: 0,
            deactivating: stakeAndActivating.effective
        )
    }
    guard var previousClusterStake = solanaStakeHistoryEntry(
        in: stakeHistoryEntries,
        epoch: deactivationEpoch
    ) else {
        return SolanaSccpStakeActivationStatus(effective: 0, activating: 0, deactivating: 0)
    }

    var previousEpoch = deactivationEpoch
    var remainingDeactivatingStake = stakeAndActivating.effective
    while true {
        let nextEpoch = previousEpoch.addingReportingOverflow(1)
        guard !nextEpoch.overflow else {
            return nil
        }
        let currentEpoch = nextEpoch.partialValue
        if previousClusterStake.deactivating == 0 {
            break
        }
        let newlyDeactivatedStake = max(
            solanaStakeChangeAllowance(
                accountPortion: remainingDeactivatingStake,
                clusterPortion: previousClusterStake.deactivating,
                clusterEffective: previousClusterStake.effective
            ),
            1
        )
        remainingDeactivatingStake = newlyDeactivatedStake >= remainingDeactivatingStake
            ? 0
            : remainingDeactivatingStake - newlyDeactivatedStake
        if remainingDeactivatingStake == 0 {
            break
        }
        if currentEpoch >= targetEpoch {
            break
        }
        guard let currentClusterStake = solanaStakeHistoryEntry(
            in: stakeHistoryEntries,
            epoch: currentEpoch
        ) else {
            break
        }
        previousEpoch = currentEpoch
        previousClusterStake = currentClusterStake
    }

    return SolanaSccpStakeActivationStatus(
        effective: remainingDeactivatingStake,
        activating: 0,
        deactivating: remainingDeactivatingStake
    )
}

/// Solana bincode Vec bytes for StakeHistory sysvar account data.
public func canonicalSolanaSccpStakeHistorySysvarDataBytes(
    stakeHistoryEntries: [SolanaSccpStakeHistoryEntry]
) throws -> Data {
    guard !stakeHistoryEntries.isEmpty, stakeHistoryEntries.count <= 512 else {
        throw SolanaSccpProverError.invalidString("stakeHistoryEntries")
    }
    var previousEpoch: UInt64?
    for entry in stakeHistoryEntries {
        if let previousEpoch {
            guard previousEpoch < entry.epoch else {
                throw SolanaSccpProverError.invalidString("stakeHistoryEntries")
            }
        }
        previousEpoch = entry.epoch
    }

    var out = Data()
    appendU64Le(UInt64(stakeHistoryEntries.count), to: &out)
    for entry in stakeHistoryEntries.reversed() {
        appendU64Le(entry.epoch, to: &out)
        appendU64Le(entry.effective, to: &out)
        appendU64Le(entry.activating, to: &out)
        appendU64Le(entry.deactivating, to: &out)
    }
    return out
}

/// Hash Solana StakeHistory sysvar account data.
public func solanaSccpStakeHistorySysvarDataHash(
    stakeHistoryEntries: [SolanaSccpStakeHistoryEntry]
) throws -> String {
    hashHex(
        prefix: "sccp:solana:stake-history-sysvar-data:v1",
        payload: try canonicalSolanaSccpStakeHistorySysvarDataBytes(
            stakeHistoryEntries: stakeHistoryEntries
        )
    )
}

/// Hash raw Solana StakeHistory sysvar account bytes after bincode Vec validation.
public func solanaSccpStakeHistorySysvarDataHashFromRawData(
    rawData: Data
) throws -> String {
    guard rawData.count >= 8, (rawData.count - 8) % 32 == 0 else {
        throw SolanaSccpProverError.invalidString("rawData")
    }
    let entryCount = try readU64Le(rawData, offset: 0, field: "rawData")
    guard entryCount > 0, entryCount <= 512 else {
        throw SolanaSccpProverError.invalidString("rawData")
    }
    let expectedLength = 8 + Int(entryCount) * 32
    guard rawData.count == expectedLength else {
        throw SolanaSccpProverError.invalidString("rawData")
    }
    var cursor = 8
    var previousEpoch: UInt64?
    for _ in 0..<Int(entryCount) {
        let epoch = try readU64Le(rawData, offset: cursor, field: "rawData")
        cursor += 32
        if let previousEpoch {
            guard previousEpoch > epoch else {
                throw SolanaSccpProverError.invalidString("rawDataOrder")
            }
        }
        previousEpoch = epoch
    }
    return hashHex(
        prefix: "sccp:solana:stake-history-sysvar-data:v1",
        payload: rawData
    )
}

/// Canonical bytes for Solana StakeHistory sysvar evidence bound into SCCP finality contexts.
public func canonicalSolanaSccpStakeHistoryBytes(
    epoch: UInt64,
    validatorPublicKeys: [Data],
    validatorEffectiveStakes: [UInt64],
    validatorDelegatedStakes: [UInt64],
    validatorActivationEpochs: [UInt64],
    validatorDeactivationEpochs: [UInt64],
    validatorVoteAccountAddresses: [Data],
    validatorStakeAccountAddresses: [Data],
    validatorVoteAccountHashes: [Data],
    validatorStakeAccountHashes: [Data],
    stakeHistoryEntries: [SolanaSccpStakeHistoryEntry]
) throws -> Data {
    guard validatorEffectiveStakes.count == validatorPublicKeys.count else {
        throw SolanaSccpProverError.invalidString("validatorEffectiveStakes")
    }
    guard validatorDelegatedStakes.count == validatorPublicKeys.count else {
        throw SolanaSccpProverError.invalidString("validatorDelegatedStakes")
    }
    guard validatorActivationEpochs.count == validatorPublicKeys.count else {
        throw SolanaSccpProverError.invalidString("validatorActivationEpochs")
    }
    guard validatorDeactivationEpochs.count == validatorPublicKeys.count else {
        throw SolanaSccpProverError.invalidString("validatorDeactivationEpochs")
    }
    guard !stakeHistoryEntries.isEmpty, stakeHistoryEntries.count <= 512 else {
        throw SolanaSccpProverError.invalidString("stakeHistoryEntries")
    }
    var previousEpoch: UInt64?
    var signedEpochEntry: SolanaSccpStakeHistoryEntry?
    for (index, entry) in stakeHistoryEntries.enumerated() {
        guard entry.epoch <= epoch else {
            throw SolanaSccpProverError.invalidString("stakeHistoryEntries[\(index)].epoch")
        }
        if let previousEpoch {
            guard previousEpoch < entry.epoch else {
                throw SolanaSccpProverError.invalidString("stakeHistoryEntries")
            }
        }
        previousEpoch = entry.epoch
        if entry.epoch == epoch {
            signedEpochEntry = entry
        }
    }
    guard let signedEpochEntry else {
        throw SolanaSccpProverError.invalidString("stakeHistoryEntries")
    }

    var totalEffectiveStake: UInt64 = 0
    var totalDelegatedStake: UInt64 = 0
    var totalActivatingStake: UInt64 = 0
    var totalDeactivatingStake: UInt64 = 0
    for index in validatorPublicKeys.indices {
        let delegatedStake = validatorDelegatedStakes[index]
        let activationEpoch = validatorActivationEpochs[index]
        let deactivationEpoch = validatorDeactivationEpochs[index]
        guard delegatedStake > 0 else {
            throw SolanaSccpProverError.invalidString("validatorDelegatedStakes[\(index)]")
        }
        guard deactivationEpoch > activationEpoch else {
            throw SolanaSccpProverError.invalidString("validatorDeactivationEpochs[\(index)]")
        }
        guard let status = solanaDelegationStakeStatusV2(
            targetEpoch: epoch,
            delegatedStake: delegatedStake,
            activationEpoch: activationEpoch,
            deactivationEpoch: deactivationEpoch,
            stakeHistoryEntries: stakeHistoryEntries
        ) else {
            throw SolanaSccpProverError.invalidString("stakeHistoryEntries")
        }
        guard status.effective > 0, status.effective == validatorEffectiveStakes[index] else {
            throw SolanaSccpProverError.invalidString("validatorEffectiveStakes[\(index)]")
        }
        let effectiveSum = totalEffectiveStake.addingReportingOverflow(status.effective)
        let delegatedSum = totalDelegatedStake.addingReportingOverflow(delegatedStake)
        let activatingSum = totalActivatingStake.addingReportingOverflow(status.activating)
        let deactivatingSum = totalDeactivatingStake.addingReportingOverflow(status.deactivating)
        guard !effectiveSum.overflow,
              !delegatedSum.overflow,
              !activatingSum.overflow,
              !deactivatingSum.overflow else {
            throw SolanaSccpProverError.invalidString("validatorStakes")
        }
        totalEffectiveStake = effectiveSum.partialValue
        totalDelegatedStake = delegatedSum.partialValue
        totalActivatingStake = activatingSum.partialValue
        totalDeactivatingStake = deactivatingSum.partialValue
    }
    guard totalEffectiveStake > 0, totalDelegatedStake >= totalEffectiveStake else {
        throw SolanaSccpProverError.invalidString("stakeHistoryEntries")
    }
    guard signedEpochEntry.effective == totalEffectiveStake,
          signedEpochEntry.activating >= totalActivatingStake,
          signedEpochEntry.deactivating >= totalDeactivatingStake else {
        throw SolanaSccpProverError.invalidString("stakeHistoryEntries")
    }
    let stakeAccountStateBytes = try canonicalSolanaSccpStakeAccountStateBytes(
        epoch: epoch,
        validatorPublicKeys: validatorPublicKeys,
        validatorStakes: validatorDelegatedStakes,
        validatorActivationEpochs: validatorActivationEpochs,
        validatorDeactivationEpochs: validatorDeactivationEpochs,
        validatorVoteAccountAddresses: validatorVoteAccountAddresses,
        validatorStakeAccountAddresses: validatorStakeAccountAddresses,
        validatorVoteAccountHashes: validatorVoteAccountHashes,
        validatorStakeAccountHashes: validatorStakeAccountHashes
    )
    let stakeAccountStateHash = hashBytes(
        prefix: "sccp:solana:stake-account-state:v1",
        payload: stakeAccountStateBytes
    )

    var out = Data()
    out.append(1)
    appendU64Le(epoch, to: &out)
    out.append(stakeAccountStateHash)
    appendU32Le(UInt32(validatorPublicKeys.count), to: &out)
    for index in validatorPublicKeys.indices {
        appendBytesVec(validatorPublicKeys[index], to: &out)
        appendU64Le(validatorEffectiveStakes[index], to: &out)
        appendU64Le(validatorDelegatedStakes[index], to: &out)
        appendU64Le(validatorActivationEpochs[index], to: &out)
        appendU64Le(validatorDeactivationEpochs[index], to: &out)
    }
    appendU32Le(UInt32(stakeHistoryEntries.count), to: &out)
    for entry in stakeHistoryEntries {
        appendU64Le(entry.epoch, to: &out)
        appendU64Le(entry.effective, to: &out)
        appendU64Le(entry.activating, to: &out)
        appendU64Le(entry.deactivating, to: &out)
    }
    return out
}

/// Hash Solana StakeHistory sysvar evidence used by SCCP finality contexts.
public func solanaSccpStakeHistoryHash(
    epoch: UInt64,
    validatorPublicKeys: [Data],
    validatorEffectiveStakes: [UInt64],
    validatorDelegatedStakes: [UInt64],
    validatorActivationEpochs: [UInt64],
    validatorDeactivationEpochs: [UInt64],
    validatorVoteAccountAddresses: [Data],
    validatorStakeAccountAddresses: [Data],
    validatorVoteAccountHashes: [Data],
    validatorStakeAccountHashes: [Data],
    stakeHistoryEntries: [SolanaSccpStakeHistoryEntry]
) throws -> String {
    hashHex(
        prefix: "sccp:solana:stake-history:v1",
        payload: try canonicalSolanaSccpStakeHistoryBytes(
            epoch: epoch,
            validatorPublicKeys: validatorPublicKeys,
            validatorEffectiveStakes: validatorEffectiveStakes,
            validatorDelegatedStakes: validatorDelegatedStakes,
            validatorActivationEpochs: validatorActivationEpochs,
            validatorDeactivationEpochs: validatorDeactivationEpochs,
            validatorVoteAccountAddresses: validatorVoteAccountAddresses,
            validatorStakeAccountAddresses: validatorStakeAccountAddresses,
            validatorVoteAccountHashes: validatorVoteAccountHashes,
            validatorStakeAccountHashes: validatorStakeAccountHashes,
            stakeHistoryEntries: stakeHistoryEntries
        )
    )
}

/// Canonical bytes for the Solana Tower lockout context checked by SCCP finality votes.
public func canonicalSolanaSccpTowerLockoutBytes(
    epoch: UInt64? = nil,
    finalizedSlot: UInt64,
    rootedSlot: UInt64,
    parentSlot: UInt64,
    parentBankHash: String
) throws -> Data {
    let resolvedEpoch = epoch ?? solanaSccpMainnetEpoch(forSlot: finalizedSlot)
    guard resolvedEpoch == solanaSccpMainnetEpoch(forSlot: finalizedSlot) else {
        throw SolanaSccpProverError.invalidString("epoch")
    }
    guard rootedSlot <= parentSlot else {
        throw SolanaSccpProverError.invalidString("rootedSlot")
    }
    let parentNext = parentSlot.addingReportingOverflow(1)
    guard !parentNext.overflow, parentNext.partialValue == finalizedSlot else {
        throw SolanaSccpProverError.invalidString("parentSlot")
    }
    guard finalizedSlot - rootedSlot >= sccpSolanaTowerVoteStackDepth else {
        throw SolanaSccpProverError.invalidString("rootedSlot")
    }
    let parentBankHashBytes = try bytesFromHex32(parentBankHash, field: "parentBankHash")
    guard parentBankHashBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("parentBankHash")
    }
    var out = Data()
    out.append(1)
    appendU64Le(resolvedEpoch, to: &out)
    appendU64Le(sccpSolanaTowerLockoutConfirmationDepth, to: &out)
    appendU64Le(finalizedSlot, to: &out)
    appendU64Le(rootedSlot, to: &out)
    appendU64Le(parentSlot, to: &out)
    out.append(parentBankHashBytes)
    return out
}

/// Hash the Solana Tower lockout context used by SCCP finality votes.
public func solanaSccpTowerLockoutHash(
    epoch: UInt64? = nil,
    finalizedSlot: UInt64,
    rootedSlot: UInt64,
    parentSlot: UInt64,
    parentBankHash: String
) throws -> String {
    hashHex(
        prefix: "sccp:solana:tower-lockout:v1",
        payload: try canonicalSolanaSccpTowerLockoutBytes(
            epoch: epoch,
            finalizedSlot: finalizedSlot,
            rootedSlot: rootedSlot,
            parentSlot: parentSlot,
            parentBankHash: parentBankHash
        )
    )
}

/// Canonical bytes for the Solana Tower vote-stack transcript checked by SCCP finality votes.
public func canonicalSolanaSccpTowerReplayBytes(
    epoch: UInt64? = nil,
    finalizedSlot: UInt64,
    rootedSlot: UInt64,
    parentSlot: UInt64,
    bankForkHash: String,
    towerVoteSlots: [UInt64]
) throws -> Data {
    let resolvedEpoch = epoch ?? solanaSccpMainnetEpoch(forSlot: finalizedSlot)
    guard resolvedEpoch == solanaSccpMainnetEpoch(forSlot: finalizedSlot) else {
        throw SolanaSccpProverError.invalidString("epoch")
    }
    let parentNext = parentSlot.addingReportingOverflow(1)
    guard !parentNext.overflow, parentNext.partialValue == finalizedSlot else {
        throw SolanaSccpProverError.invalidString("parentSlot")
    }
    guard rootedSlot < finalizedSlot else {
        throw SolanaSccpProverError.invalidString("rootedSlot")
    }
    guard finalizedSlot - rootedSlot >= sccpSolanaTowerVoteStackDepth else {
        throw SolanaSccpProverError.invalidString("rootedSlot")
    }
    let bankForkHashBytes = try bytesFromHex32(bankForkHash, field: "bankForkHash")
    guard bankForkHashBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("bankForkHash")
    }

    let depth = Int(sccpSolanaTowerVoteStackDepth)
    guard towerVoteSlots.count == depth else {
        throw SolanaSccpProverError.invalidString("towerVoteSlots")
    }
    guard towerVoteSlots.first.map({ $0 > rootedSlot }) == true else {
        throw SolanaSccpProverError.invalidString("towerVoteSlots[0]")
    }
    guard towerVoteSlots.last == finalizedSlot else {
        throw SolanaSccpProverError.invalidString("towerVoteSlots")
    }
    guard towerVoteSlots[depth - 2] == parentSlot else {
        throw SolanaSccpProverError.invalidString("towerVoteSlots")
    }
    for index in 1..<towerVoteSlots.count where towerVoteSlots[index - 1] >= towerVoteSlots[index] {
        throw SolanaSccpProverError.invalidString("towerVoteSlots")
    }
    for (index, voteSlot) in towerVoteSlots.enumerated() {
        guard voteSlot <= finalizedSlot else {
            throw SolanaSccpProverError.invalidString("towerVoteSlots[\(index)]")
        }
        let confirmationCount = depth - index
        let lockout = UInt64(1) << confirmationCount
        let expiresAt = voteSlot.addingReportingOverflow(lockout)
        guard !expiresAt.overflow, expiresAt.partialValue > finalizedSlot else {
            throw SolanaSccpProverError.invalidString("towerVoteSlots[\(index)]")
        }
    }

    var out = Data()
    out.append(1)
    appendU64Le(resolvedEpoch, to: &out)
    appendU64Le(sccpSolanaTowerLockoutConfirmationDepth, to: &out)
    appendU64Le(finalizedSlot, to: &out)
    appendU64Le(rootedSlot, to: &out)
    appendU64Le(parentSlot, to: &out)
    out.append(bankForkHashBytes)
    appendU32Le(UInt32(towerVoteSlots.count), to: &out)
    for (index, voteSlot) in towerVoteSlots.enumerated() {
        appendU64Le(voteSlot, to: &out)
        appendU64Le(UInt64(depth - index), to: &out)
    }
    return out
}

/// Hash the Solana Tower vote-stack transcript used by SCCP finality votes.
public func solanaSccpTowerReplayHash(
    epoch: UInt64? = nil,
    finalizedSlot: UInt64,
    rootedSlot: UInt64,
    parentSlot: UInt64,
    bankForkHash: String,
    towerVoteSlots: [UInt64]
) throws -> String {
    hashHex(
        prefix: "sccp:solana:tower-replay:v1",
        payload: try canonicalSolanaSccpTowerReplayBytes(
            epoch: epoch,
            finalizedSlot: finalizedSlot,
            rootedSlot: rootedSlot,
            parentSlot: parentSlot,
            bankForkHash: bankForkHash,
            towerVoteSlots: towerVoteSlots
        )
    )
}

private func sha256Hashv(_ parts: [Data]) -> Data {
    var hasher = SHA256()
    for part in parts {
        hasher.update(data: part)
    }
    return Data(hasher.finalize())
}

/// Recompute Agave's finalized bank hash from SCCP Solana bank-state inputs.
public func solanaSccpAgaveBankHash(
    parentBankHash: String,
    bankSignatureCount: UInt64,
    blockhash: String,
    accountsLtHash: Data,
    bankHashHardForkData: Data = Data()
) throws -> String {
    let parentBankHashBytes = try bytesFromHex32(parentBankHash, field: "parentBankHash")
    guard parentBankHashBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("parentBankHash")
    }
    guard bankSignatureCount != 0 else {
        throw SolanaSccpProverError.invalidString("bankSignatureCount")
    }
    let blockhashBytes = try bytesFromHex32(blockhash, field: "blockhash")
    guard blockhashBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("blockhash")
    }
    guard accountsLtHash.count == sccpSolanaAccountsLtHashBytes else {
        throw SolanaSccpProverError.invalidString("accountsLtHash")
    }
    guard accountsLtHash.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidString("accountsLtHash")
    }
    guard bankHashHardForkData.count <= sccpSolanaMaxBankHardForkHashDataBytes else {
        throw SolanaSccpProverError.invalidString("bankHashHardForkData")
    }

    var signatureCountBytes = Data()
    appendU64Le(bankSignatureCount, to: &signatureCountBytes)
    var bankHash = sha256Hashv([parentBankHashBytes, signatureCountBytes, blockhashBytes])
    bankHash = sha256Hashv([bankHash, accountsLtHash])
    if !bankHashHardForkData.isEmpty {
        bankHash = sha256Hashv([bankHash, bankHashHardForkData])
    }
    return "0x" + bankHash.hexEncodedString()
}

/// Canonical bytes for the Solana bank-fork context checked by SCCP finality votes.
public func canonicalSolanaSccpBankForkBytes(
    epoch: UInt64? = nil,
    finalizedSlot: UInt64,
    parentSlot: UInt64,
    bankSignatureCount: UInt64,
    parentBankHash: String,
    bankHash: String,
    blockhash: String,
    accountsLtHash: Data? = nil,
    bankHashHardForkData: Data = Data(),
    transactionStatusRoot: String,
    accountInclusionRoot: String,
    accountsLtHashChecksum: String
) throws -> Data {
    let resolvedEpoch = epoch ?? solanaSccpMainnetEpoch(forSlot: finalizedSlot)
    guard resolvedEpoch == solanaSccpMainnetEpoch(forSlot: finalizedSlot) else {
        throw SolanaSccpProverError.invalidString("epoch")
    }
    let parentNext = parentSlot.addingReportingOverflow(1)
    guard !parentNext.overflow, parentNext.partialValue == finalizedSlot else {
        throw SolanaSccpProverError.invalidString("parentSlot")
    }
    guard bankSignatureCount != 0 else {
        throw SolanaSccpProverError.invalidString("bankSignatureCount")
    }
    let parentBankHashBytes = try bytesFromHex32(parentBankHash, field: "parentBankHash")
    guard parentBankHashBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("parentBankHash")
    }
    let bankHashBytes = try bytesFromHex32(bankHash, field: "bankHash")
    guard bankHashBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("bankHash")
    }
    guard parentBankHashBytes != bankHashBytes else {
        throw SolanaSccpProverError.invalidString("bankHash")
    }
    let blockhashBytes = try bytesFromHex32(blockhash, field: "blockhash")
    guard blockhashBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("blockhash")
    }
    let transactionStatusRootBytes = try bytesFromHex32(transactionStatusRoot, field: "transactionStatusRoot")
    guard transactionStatusRootBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("transactionStatusRoot")
    }
    let accountInclusionRootBytes = try bytesFromHex32(accountInclusionRoot, field: "accountInclusionRoot")
    guard accountInclusionRootBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("accountInclusionRoot")
    }
    let accountsLtHashChecksumBytes = try bytesFromHex32(accountsLtHashChecksum, field: "accountsLtHashChecksum")
    guard accountsLtHashChecksumBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("accountsLtHashChecksum")
    }
    guard bankHashHardForkData.count <= sccpSolanaMaxBankHardForkHashDataBytes else {
        throw SolanaSccpProverError.invalidString("bankHashHardForkData")
    }
    if let accountsLtHash {
        let expectedBankHash = try bytesFromHex32(
            solanaSccpAgaveBankHash(
                parentBankHash: parentBankHash,
                bankSignatureCount: bankSignatureCount,
                blockhash: blockhash,
                accountsLtHash: accountsLtHash,
                bankHashHardForkData: bankHashHardForkData
            ),
            field: "bankHash"
        )
        guard bankHashBytes == expectedBankHash else {
            throw SolanaSccpProverError.invalidString("bankHash")
        }
        guard try blake3Hash32(accountsLtHash, field: "accountsLtHash") == accountsLtHashChecksumBytes else {
            throw SolanaSccpProverError.invalidString("accountsLtHashChecksum")
        }
    }
    var out = Data()
    out.append(1)
    appendU64Le(resolvedEpoch, to: &out)
    appendU64Le(finalizedSlot, to: &out)
    appendU64Le(parentSlot, to: &out)
    appendU64Le(bankSignatureCount, to: &out)
    out.append(parentBankHashBytes)
    out.append(bankHashBytes)
    out.append(blockhashBytes)
    out.append(transactionStatusRootBytes)
    out.append(accountInclusionRootBytes)
    out.append(accountsLtHashChecksumBytes)
    appendBytesVec(bankHashHardForkData, to: &out)
    return out
}

/// Hash the Solana bank-fork context used by SCCP finality votes.
public func solanaSccpBankForkHash(
    epoch: UInt64? = nil,
    finalizedSlot: UInt64,
    parentSlot: UInt64,
    bankSignatureCount: UInt64,
    parentBankHash: String,
    bankHash: String,
    blockhash: String,
    accountsLtHash: Data? = nil,
    bankHashHardForkData: Data = Data(),
    transactionStatusRoot: String,
    accountInclusionRoot: String,
    accountsLtHashChecksum: String
) throws -> String {
    hashHex(
        prefix: "sccp:solana:bank-fork:v1",
        payload: try canonicalSolanaSccpBankForkBytes(
            epoch: epoch,
            finalizedSlot: finalizedSlot,
            parentSlot: parentSlot,
            bankSignatureCount: bankSignatureCount,
            parentBankHash: parentBankHash,
            bankHash: bankHash,
            blockhash: blockhash,
            accountsLtHash: accountsLtHash,
            bankHashHardForkData: bankHashHardForkData,
            transactionStatusRoot: transactionStatusRoot,
            accountInclusionRoot: accountInclusionRoot,
            accountsLtHashChecksum: accountsLtHashChecksum
        )
    )
}

/// Canonical public inputs for the recursive Solana AccountsDB LtHash proof.
public func canonicalSolanaSccpAccountsLtHashProofPublicInputsBytes(
    sourceDomain: UInt32 = sccpDomainSolana,
    finalizedSlot: UInt64,
    parentSlot: UInt64,
    bankSignatureCount: UInt64,
    parentBankHash: String,
    bankHash: String,
    blockhash: String,
    bankHashHardForkData: Data = Data(),
    transactionStatusRoot: String,
    accountInclusionRoot: String,
    accountsLtHashChecksum: String,
    accountsLtHash: Data? = nil
) throws -> Data {
    guard sourceDomain == sccpDomainSolana else {
        throw SolanaSccpProverError.invalidString("sourceDomain")
    }
    let blockhashBytes = try solanaHash32Bytes(blockhash, field: "blockhash")
    let blockhashHex = "0x" + blockhashBytes.hexEncodedString()
    let bankForkHash = try bytesFromHex32(
        solanaSccpBankForkHash(
            finalizedSlot: finalizedSlot,
            parentSlot: parentSlot,
            bankSignatureCount: bankSignatureCount,
            parentBankHash: parentBankHash,
            bankHash: bankHash,
            blockhash: blockhashHex,
            accountsLtHash: accountsLtHash,
            bankHashHardForkData: bankHashHardForkData,
            transactionStatusRoot: transactionStatusRoot,
            accountInclusionRoot: accountInclusionRoot,
            accountsLtHashChecksum: accountsLtHashChecksum
        ),
        field: "bankForkHash"
    )
    let epoch = solanaSccpMainnetEpoch(forSlot: finalizedSlot)
    var out = Data()
    out.append(1)
    appendU32Le(sourceDomain, to: &out)
    try appendString(sccpSolanaRecursiveProofBackendV1, field: "backend", to: &out)
    try appendString(sccpSolanaMainnetGenesisHash, field: "mainnetGenesisHash", to: &out)
    appendU64Le(epoch, to: &out)
    appendU64Le(finalizedSlot, to: &out)
    appendU64Le(parentSlot, to: &out)
    appendU64Le(bankSignatureCount, to: &out)
    try out.append(bytesFromHex32(parentBankHash, field: "parentBankHash"))
    try out.append(bytesFromHex32(bankHash, field: "bankHash"))
    out.append(blockhashBytes)
    try out.append(bytesFromHex32(transactionStatusRoot, field: "transactionStatusRoot"))
    try out.append(bytesFromHex32(accountInclusionRoot, field: "accountInclusionRoot"))
    try out.append(bytesFromHex32(accountsLtHashChecksum, field: "accountsLtHashChecksum"))
    appendBytesVec(bankHashHardForkData, to: &out)
    out.append(bankForkHash)
    return out
}

/// Hash recursive Solana AccountsDB LtHash proof public inputs.
public func solanaSccpAccountsLtHashProofPublicInputsHash(
    sourceDomain: UInt32 = sccpDomainSolana,
    finalizedSlot: UInt64,
    parentSlot: UInt64,
    bankSignatureCount: UInt64,
    parentBankHash: String,
    bankHash: String,
    blockhash: String,
    bankHashHardForkData: Data = Data(),
    transactionStatusRoot: String,
    accountInclusionRoot: String,
    accountsLtHashChecksum: String,
    accountsLtHash: Data? = nil
) throws -> String {
    hashHex(
        prefix: sccpSolanaAccountsLtHashProofPublicInputsPrefixV1,
        payload: try canonicalSolanaSccpAccountsLtHashProofPublicInputsBytes(
            sourceDomain: sourceDomain,
            finalizedSlot: finalizedSlot,
            parentSlot: parentSlot,
            bankSignatureCount: bankSignatureCount,
            parentBankHash: parentBankHash,
            bankHash: bankHash,
            blockhash: blockhash,
            bankHashHardForkData: bankHashHardForkData,
            transactionStatusRoot: transactionStatusRoot,
            accountInclusionRoot: accountInclusionRoot,
            accountsLtHashChecksum: accountsLtHashChecksum,
            accountsLtHash: accountsLtHash
        )
    )
}

/// Canonical source-state proof capsule bytes hashed by Solana audit role requests.
public func canonicalSolanaSccpSourceStateVerificationProofBytes(
    _ proof: SolanaSccpSourceStateVerificationProof
) throws -> Data {
    guard proof.proofFamily.utf8.count <= sccpSourceStateMaxProofLabelBytes else {
        throw SolanaSccpProverError.invalidString("proofFamily")
    }
    guard proof.circuitId.utf8.count <= sccpSourceStateMaxProofLabelBytes else {
        throw SolanaSccpProverError.invalidString("circuitId")
    }
    guard proof.version == 1,
          proof.proofFamily == sccpStarkFriProofFamilyV1,
          solanaSccpSourceStateVerificationCircuitIds.contains(proof.circuitId) else {
        throw SolanaSccpProverError.invalidString("accountsLtHashProof")
    }
    guard !proof.proofBytes.isEmpty else {
        throw SolanaSccpProverError.invalidString("proofBytes")
    }
    guard proof.proofBytes.count <= sccpSourceStateMaxProofBytes else {
        throw SolanaSccpProverError.invalidString("proofBytes")
    }
    guard proof.proofBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.allZeroProof
    }
    var out = Data()
    out.append(proof.version)
    try appendString(proof.proofFamily, field: "proofFamily", to: &out)
    try appendString(proof.circuitId, field: "circuitId", to: &out)
    appendBytesVec(proof.proofBytes, to: &out)
    return out
}

/// Wrap completed Solana AccountsLtHash proof bytes with the originating request metadata.
public func wrapSolanaSccpSourceStateVerificationProof(
    proofBytes: Data,
    request: SolanaSccpAccountsLtHashProofRequest
) throws -> SolanaSccpSourceStateVerificationProof {
    try requireSourceStateProofRequestForWrapping(request)
    return try wrapSolanaSccpSourceStateVerificationProof(
        proofBytes: proofBytes,
        version: request.version,
        proofFamily: request.proofFamily,
        circuitId: request.circuitId,
        sourceDomain: request.sourceDomain
    )
}

/// Wrap completed Solana full-light audit proof bytes with the originating role request metadata.
public func wrapSolanaSccpSourceStateVerificationProof(
    proofBytes: Data,
    request: SolanaSccpFullLightClientAuditProofRequest
) throws -> SolanaSccpSourceStateVerificationProof {
    try requireSourceStateProofRequestForWrapping(request)
    return try wrapSolanaSccpSourceStateVerificationProof(
        proofBytes: proofBytes,
        version: request.version,
        proofFamily: request.proofFamily,
        circuitId: request.circuitId,
        sourceDomain: request.sourceDomain
    )
}

private func requireSourceStateProofRequestForWrapping(
    _ request: SolanaSccpAccountsLtHashProofRequest
) throws {
    guard request.version == 1 else {
        throw SolanaSccpProverError.invalidString("request.version")
    }
    guard request.proofFamily == sccpStarkFriProofFamilyV1 else {
        throw SolanaSccpProverError.invalidString("request.proofFamily")
    }
    guard request.circuitId == sccpSolanaAccountsLtHashOpenVerifyCircuitIdV1 else {
        throw SolanaSccpProverError.invalidString("request.circuitId")
    }
    guard request.parameterSet == sccpSolanaAccountsLtHashFastpqParameterSetV1 else {
        throw SolanaSccpProverError.invalidString("request.parameterSet")
    }
    guard request.sourceDomain == sccpDomainSolana else {
        throw SolanaSccpProverError.invalidString("request.sourceDomain")
    }
    guard let finalizedSlot = UInt64(request.finalizedSlot),
          let parentSlot = UInt64(request.parentSlot),
          parentSlot != UInt64.max,
          parentSlot + 1 == finalizedSlot else {
        throw SolanaSccpProverError.invalidString("request.parentSlot")
    }
    guard request.sourceStateVerifierId == sccpSolanaMainnetAccountsDbVerifierIdV1 else {
        throw SolanaSccpProverError.invalidString("request.sourceStateVerifierId")
    }
    let sourceStateVerifierHash = try normalizeNonZeroHex32(
        request.sourceStateVerifierHash,
        field: "request.sourceStateVerifierHash"
    )
    guard sourceStateVerifierHash != sccpSolanaTemplateSourceStateVerifierHashV1 else {
        throw SolanaSccpProverError.invalidHex32("request.sourceStateVerifierHash")
    }
    let accountsLtHashProofPublicInputsHash = try normalizeNonZeroHex32(
        request.accountsLtHashProofPublicInputsHash,
        field: "request.accountsLtHashProofPublicInputsHash"
    )
    _ = try normalizeNonZeroHex32(
        request.openedAccountsLtHashContributionsHash,
        field: "request.openedAccountsLtHashContributionsHash"
    )
    _ = try normalizeNonZeroHex32(
        request.openedAccountsLtHashResidualChecksum,
        field: "request.openedAccountsLtHashResidualChecksum"
    )
    try requireSolanaOpenVerifyRequestPayloadForWrapping(
        statementBytes: request.statementBytes,
        accountCommitmentBytes: request.accountCommitmentBytes,
        verificationContextBytes: request.verificationContextBytes,
        schemaDescriptor: request.schemaDescriptor,
        publicInputColumns: request.publicInputColumns,
        fastpqFields: [
            request.fastpqPublicInputs.dsid,
            request.fastpqPublicInputs.slot,
            request.fastpqPublicInputs.oldRoot,
            request.fastpqPublicInputs.newRoot,
            request.fastpqPublicInputs.permRoot,
            request.fastpqPublicInputs.txSetHash,
        ],
        transitionEntries: request.fastpqTransitions.map {
            ($0.key, $0.operation, $0.oldValue, $0.newValue)
        },
        expectedTransitions: [
            (
                sccpSolanaAccountsLtHashFastpqStatementKeyV1,
                "meta_set",
                Data(),
                request.statementBytes
            ),
            (
                sccpSolanaAccountsLtHashFastpqAccountsKeyV1,
                "meta_set",
                Data(),
                request.accountCommitmentBytes
            ),
            (
                sccpSolanaAccountsLtHashFastpqOpenedContributionsKeyV1,
                "meta_set",
                Data(),
                try bytesFromHex32(
                    request.openedAccountsLtHashContributionsHash,
                    field: "request.openedAccountsLtHashContributionsHash"
                )
            ),
            (
                sccpSolanaAccountsLtHashFastpqResidualKeyV1,
                "meta_set",
                Data(),
                try bytesFromHex32(
                    request.openedAccountsLtHashResidualChecksum,
                    field: "request.openedAccountsLtHashResidualChecksum"
                )
            ),
            (
                sccpSolanaAccountsLtHashFastpqContextKeyV1,
                "meta_set",
                Data(),
                request.verificationContextBytes
            ),
        ]
    )
    guard accountsLtHashProofPublicInputsHash == hashHex(
        prefix: sccpSolanaAccountsLtHashProofPublicInputsPrefixV1,
        payload: request.statementBytes
    ) else {
        throw SolanaSccpProverError.invalidString("request.accountsLtHashProofPublicInputsHash")
    }
    let expectedDsid = "0x" + hashBytes(
        prefix: sccpSolanaAccountsLtHashFastpqDsidPrefixV1,
        payload: try bytesFromHex32(
            accountsLtHashProofPublicInputsHash,
            field: "request.accountsLtHashProofPublicInputsHash"
        )
    ).prefix(16).hexEncodedString()
    guard try normalizeHexBytes(
        request.fastpqPublicInputs.dsid,
        field: "request.fastpqPublicInputs.dsid",
        byteCount: 16
    ) == expectedDsid else {
        throw SolanaSccpProverError.invalidString("request.fastpqPublicInputs.dsid")
    }
    guard try normalizeNonZeroHex32(
        request.fastpqPublicInputs.txSetHash,
        field: "request.fastpqPublicInputs.txSetHash"
    ) == accountsLtHashProofPublicInputsHash else {
        throw SolanaSccpProverError.invalidString("request.fastpqPublicInputs.txSetHash")
    }
    try requireSolanaSourceStatePublicInputBindingForWrapping(request)
}

private func requireSourceStateProofRequestForWrapping(
    _ request: SolanaSccpFullLightClientAuditProofRequest
) throws {
    guard request.version == 1 else {
        throw SolanaSccpProverError.invalidString("request.version")
    }
    guard request.proofFamily == sccpStarkFriProofFamilyV1 else {
        throw SolanaSccpProverError.invalidString("request.proofFamily")
    }
    guard request.parameterSet == sccpSolanaFullLightClientAuditFastpqParameterSetV1 else {
        throw SolanaSccpProverError.invalidString("request.parameterSet")
    }
    guard request.sourceDomain == sccpDomainSolana else {
        throw SolanaSccpProverError.invalidString("request.sourceDomain")
    }
    let profile = try auditRoleProfileForRequest(request.role)
    guard request.roleCode == profile.code else {
        throw SolanaSccpProverError.invalidString("request.roleCode")
    }
    guard request.circuitId == profile.circuitId else {
        throw SolanaSccpProverError.invalidString("request.circuitId")
    }
    guard request.verifierId == profile.verifierId else {
        throw SolanaSccpProverError.invalidString("request.verifierId")
    }
    guard UInt64(request.finalizedSlot) != nil else {
        throw SolanaSccpProverError.invalidString("request.finalizedSlot")
    }
    let verifierHash = try normalizeNonZeroHex32(request.verifierHash, field: "request.verifierHash")
    guard request.sourceStateVerifierId == sccpSolanaMainnetAccountsDbVerifierIdV1 else {
        throw SolanaSccpProverError.invalidString("request.sourceStateVerifierId")
    }
    let sourceStateVerifierHash = try normalizeNonZeroHex32(
        request.sourceStateVerifierHash,
        field: "request.sourceStateVerifierHash"
    )
    guard sourceStateVerifierHash != sccpSolanaTemplateSourceStateVerifierHashV1 else {
        throw SolanaSccpProverError.invalidHex32("request.sourceStateVerifierHash")
    }
    var auditStatementHash = ""
    var roleSeparatedRequestHashes = [sourceStateVerifierHash]
    for (field, hash) in [
        ("request.sourceVerifierMaterialHash", request.sourceVerifierMaterialHash),
        ("request.sourceAdapterDeploymentHash", request.sourceAdapterDeploymentHash),
        ("request.fullLightClientGateHash", request.fullLightClientGateHash),
        ("request.finalityContextHash", request.finalityContextHash),
        ("request.voteMessageHash", request.voteMessageHash),
        ("request.accountsLtHashProofHash", request.accountsLtHashProofHash),
        ("request.auditStatementHash", request.auditStatementHash),
    ] {
        let normalizedHash = try normalizeNonZeroHex32(hash, field: field)
        roleSeparatedRequestHashes.append(normalizedHash)
        if field == "request.auditStatementHash" {
            auditStatementHash = normalizedHash
        }
    }
    guard !roleSeparatedRequestHashes.contains(verifierHash) else {
        throw SolanaSccpProverError.invalidString("request.verifierHash")
    }
    try requireSolanaOpenVerifyRequestPayloadForWrapping(
        statementBytes: request.statementBytes,
        accountCommitmentBytes: nil,
        verificationContextBytes: request.verificationContextBytes,
        schemaDescriptor: request.schemaDescriptor,
        publicInputColumns: request.publicInputColumns,
        fastpqFields: [
            request.fastpqPublicInputs.dsid,
            request.fastpqPublicInputs.slot,
            request.fastpqPublicInputs.oldRoot,
            request.fastpqPublicInputs.newRoot,
            request.fastpqPublicInputs.permRoot,
            request.fastpqPublicInputs.txSetHash,
        ],
        transitionEntries: request.fastpqTransitions.map {
            ($0.key, $0.operation, $0.oldValue, $0.newValue)
        },
        expectedTransitions: [
            (
                "0x" + fullLightClientAuditFastpqKey(
                    sccpSolanaFullLightClientAuditFastpqStatementKeyV1,
                    profile: profile
                ).hexEncodedString(),
                "meta_set",
                Data(),
                request.statementBytes
            ),
            (
                "0x" + fullLightClientAuditFastpqKey(
                    sccpSolanaFullLightClientAuditFastpqContextKeyV1,
                    profile: profile
                ).hexEncodedString(),
                "meta_set",
                Data(),
                request.verificationContextBytes
            ),
            (
                "0x" + fullLightClientAuditFastpqKey(
                    sccpSolanaFullLightClientAuditFastpqGateKeyV1,
                    profile: profile
                ).hexEncodedString(),
                "meta_set",
                Data(),
                try bytesFromHex32(request.fullLightClientGateHash, field: "request.fullLightClientGateHash")
            ),
        ]
    )
    guard auditStatementHash == hashHex(
        prefix: sccpSolanaFullLightClientAuditStatementPrefixV1,
        payload: request.statementBytes
    ) else {
        throw SolanaSccpProverError.invalidString("request.auditStatementHash")
    }
    var dsidPreimage = Data([profile.code])
    try dsidPreimage.append(bytesFromHex32(auditStatementHash, field: "request.auditStatementHash"))
    let expectedDsid = "0x" + hashBytes(
        prefix: sccpSolanaFullLightClientAuditFastpqDsidPrefixV1,
        payload: dsidPreimage
    ).prefix(16).hexEncodedString()
    guard try normalizeHexBytes(
        request.fastpqPublicInputs.dsid,
        field: "request.fastpqPublicInputs.dsid",
        byteCount: 16
    ) == expectedDsid else {
        throw SolanaSccpProverError.invalidString("request.fastpqPublicInputs.dsid")
    }
    guard try normalizeNonZeroHex32(
        request.fastpqPublicInputs.txSetHash,
        field: "request.fastpqPublicInputs.txSetHash"
    ) == auditStatementHash else {
        throw SolanaSccpProverError.invalidString("request.fastpqPublicInputs.txSetHash")
    }
    try requireSolanaSourceStatePublicInputBindingForWrapping(request)
}

private func auditRoleProfileForRequest(
    _ role: String
) throws -> SolanaSccpFullLightClientAuditRoleProfile {
    switch try normalizeNonEmpty(role, field: "request.role") {
    case "towerReplay", "tower_replay":
        return auditRoleProfile(.towerReplay)
    case "fullAccountsdbLattice", "full_accountsdb_lattice":
        return auditRoleProfile(.fullAccountsdbLattice)
    case "bankForkChoice", "bank_fork_choice":
        return auditRoleProfile(.bankForkChoice)
    default:
        throw SolanaSccpProverError.invalidString("request.role")
    }
}

private func requireSolanaOpenVerifyRequestPayloadForWrapping(
    statementBytes: Data,
    accountCommitmentBytes: Data?,
    verificationContextBytes: Data,
    schemaDescriptor: Data,
    publicInputColumns: [[String]],
    fastpqFields: [String],
    transitionEntries: [(key: String, operation: String, oldValue: Data, newValue: Data)],
    expectedTransitions: [(key: String, operation: String, oldValue: Data, newValue: Data)]
) throws {
    guard !statementBytes.isEmpty else {
        throw SolanaSccpProverError.invalidString("request.statementBytes")
    }
    if let accountCommitmentBytes {
        guard !accountCommitmentBytes.isEmpty else {
            throw SolanaSccpProverError.invalidString("request.accountCommitmentBytes")
        }
    }
    guard !verificationContextBytes.isEmpty else {
        throw SolanaSccpProverError.invalidString("request.verificationContextBytes")
    }
    guard !schemaDescriptor.isEmpty else {
        throw SolanaSccpProverError.invalidString("request.schemaDescriptor")
    }
    guard !publicInputColumns.isEmpty else {
        throw SolanaSccpProverError.invalidString("request.publicInputColumns")
    }
    for (columnIndex, column) in publicInputColumns.enumerated() {
        guard !column.isEmpty else {
            throw SolanaSccpProverError.invalidString("request.publicInputColumns[\(columnIndex)]")
        }
        for (valueIndex, value) in column.enumerated() {
            _ = try normalizeNonEmpty(
                value,
                field: "request.publicInputColumns[\(columnIndex)][\(valueIndex)]"
            )
        }
    }
    for (index, field) in fastpqFields.enumerated() {
        _ = try normalizeNonEmpty(field, field: "request.fastpqPublicInputs[\(index)]")
    }
    guard !transitionEntries.isEmpty else {
        throw SolanaSccpProverError.invalidString("request.fastpqTransitions")
    }
    for (index, transition) in transitionEntries.enumerated() {
        _ = try normalizeNonEmpty(transition.key, field: "request.fastpqTransitions[\(index)].key")
        _ = try normalizeNonEmpty(transition.operation, field: "request.fastpqTransitions[\(index)].operation")
        guard !transition.newValue.isEmpty else {
            throw SolanaSccpProverError.invalidString("request.fastpqTransitions[\(index)].newValue")
        }
    }
    let actual = transitionEntries.sorted { $0.key < $1.key }
    let expected = expectedTransitions.sorted { $0.key < $1.key }
    guard actual.count == expected.count else {
        throw SolanaSccpProverError.invalidString("request.fastpqTransitions")
    }
    for (actualTransition, expectedTransition) in zip(actual, expected) {
        guard actualTransition.key == expectedTransition.key,
              actualTransition.operation == expectedTransition.operation,
              actualTransition.oldValue == expectedTransition.oldValue,
              actualTransition.newValue == expectedTransition.newValue else {
            throw SolanaSccpProverError.invalidString("request.fastpqTransitions")
        }
    }
}

private func requireSolanaSourceStatePublicInputBindingForWrapping(
    _ request: SolanaSccpAccountsLtHashProofRequest
) throws {
    let publicInputColumns = request.publicInputColumns
    try requireSolanaSourceStateBasePublicInputColumns(publicInputColumns)
    let sourceDomainColumn = "0x" + sccpWordU32Le(sccpDomainSolana).hexEncodedString()
    let mainnetGenesisColumn = solanaSccpMainnetGenesisHashPublicInput()
    guard request.circuitId == sccpSolanaAccountsLtHashOpenVerifyCircuitIdV1 else {
        throw SolanaSccpProverError.invalidString("request.circuitId")
    }
    guard let finalizedSlot = UInt64(request.finalizedSlot),
          let parentSlot = UInt64(request.parentSlot) else {
        throw SolanaSccpProverError.invalidString("request.finalizedSlot")
    }
    try requireSolanaPublicInputColumn(publicInputColumns, 0, sourceDomainColumn, field: "source_domain")
    try requireSolanaPublicInputColumn(publicInputColumns, 1, mainnetGenesisColumn, field: "mainnet_genesis_hash")
    try requireSolanaPublicInputColumn(
        publicInputColumns,
        2,
        "0x" + sccpWordU64Le(finalizedSlot).hexEncodedString(),
        field: "finalized_slot"
    )
    try requireSolanaPublicInputColumn(
        publicInputColumns,
        3,
        "0x" + sccpWordU64Le(parentSlot).hexEncodedString(),
        field: "parent_slot"
    )
    try requireSolanaPublicInputColumn(
        publicInputColumns,
        11,
        normalizeNonZeroHex32(
            request.accountsLtHashProofPublicInputsHash,
            field: "request.accountsLtHashProofPublicInputsHash"
        ),
        field: "accounts_lt_hash_proof_public_inputs_hash"
    )
    try requireSolanaPublicInputColumn(
        publicInputColumns,
        12,
        normalizeNonZeroHex32(
            request.openedAccountsLtHashContributionsHash,
            field: "request.openedAccountsLtHashContributionsHash"
        ),
        field: "opened_accounts_lt_hash_contributions_hash"
    )
    try requireSolanaPublicInputColumn(
        publicInputColumns,
        13,
        normalizeNonZeroHex32(
            request.openedAccountsLtHashResidualChecksum,
            field: "request.openedAccountsLtHashResidualChecksum"
        ),
        field: "opened_accounts_lt_hash_residual_checksum"
    )
}

private func requireSolanaSourceStatePublicInputBindingForWrapping(
    _ request: SolanaSccpFullLightClientAuditProofRequest
) throws {
    let publicInputColumns = request.publicInputColumns
    try requireSolanaSourceStateBasePublicInputColumns(publicInputColumns)
    let sourceDomainColumn = "0x" + sccpWordU32Le(sccpDomainSolana).hexEncodedString()
    let mainnetGenesisColumn = solanaSccpMainnetGenesisHashPublicInput()
    let profile = try auditRoleProfileForRequest(request.role)
    guard let finalizedSlot = UInt64(request.finalizedSlot) else {
        throw SolanaSccpProverError.invalidString("request.finalizedSlot")
    }
    try requireSolanaPublicInputColumn(
        publicInputColumns,
        0,
        "0x" + sccpWordU8(profile.code).hexEncodedString(),
        field: "role"
    )
    try requireSolanaPublicInputColumn(publicInputColumns, 1, sourceDomainColumn, field: "source_domain")
    try requireSolanaPublicInputColumn(publicInputColumns, 2, mainnetGenesisColumn, field: "mainnet_genesis_hash")
    try requireSolanaPublicInputColumn(
        publicInputColumns,
        3,
        "0x" + sccpWordU64Le(finalizedSlot).hexEncodedString(),
        field: "finalized_slot"
    )
    for (index, expected, field) in [
        (
            4,
            try normalizeNonZeroHex32(request.finalityContextHash, field: "request.finalityContextHash"),
            "finality_context_hash"
        ),
        (
            5,
            try normalizeNonZeroHex32(request.auditStatementHash, field: "request.auditStatementHash"),
            "audit_statement_hash"
        ),
        (
            6,
            try normalizeNonZeroHex32(
                request.sourceVerifierMaterialHash,
                field: "request.sourceVerifierMaterialHash"
            ),
            "source_verifier_material_hash"
        ),
        (
            7,
            try normalizeNonZeroHex32(
                request.sourceAdapterDeploymentHash,
                field: "request.sourceAdapterDeploymentHash"
            ),
            "source_adapter_deployment_hash"
        ),
        (
            8,
            try normalizeNonZeroHex32(request.fullLightClientGateHash, field: "request.fullLightClientGateHash"),
            "full_light_client_gate_hash"
        ),
        (
            9,
            try normalizeNonZeroHex32(request.verifierHash, field: "request.verifierHash"),
            "verifier_hash"
        ),
        (
            13,
            try normalizeNonZeroHex32(request.voteMessageHash, field: "request.voteMessageHash"),
            "vote_message_hash"
        ),
        (
            14,
            try normalizeNonZeroHex32(request.accountsLtHashProofHash, field: "request.accountsLtHashProofHash"),
            "accounts_lt_hash_proof_hash"
        ),
    ] {
        try requireSolanaPublicInputColumn(
            publicInputColumns,
            index,
            expected,
            field: field
        )
    }
}

private func requireSolanaSourceStateBasePublicInputColumns(
    _ publicInputColumns: [[String]]
) throws {
    guard !publicInputColumns.isEmpty else {
        throw SolanaSccpProverError.invalidString("request.publicInputColumns")
    }
}

private func requireSolanaPublicInputColumn(
    _ publicInputColumns: [[String]],
    _ index: Int,
    _ expected: String,
    field: String
) throws {
    guard publicInputColumns.indices.contains(index),
          publicInputColumns[index].count == 1,
          try normalizeNonEmpty(
              publicInputColumns[index][0],
              field: "request.publicInputColumns[\(index)][0]"
          ) == expected else {
        throw SolanaSccpProverError.invalidString("request.publicInputColumns.\(field)")
    }
}

private func wrapSolanaSccpSourceStateVerificationProof(
    proofBytes: Data,
    version: UInt8,
    proofFamily: String,
    circuitId: String,
    sourceDomain: UInt32
) throws -> SolanaSccpSourceStateVerificationProof {
    guard version == 1 else {
        throw SolanaSccpProverError.invalidString("version")
    }
    guard proofFamily == sccpStarkFriProofFamilyV1 else {
        throw SolanaSccpProverError.invalidString("proofFamily")
    }
    guard proofFamily.utf8.count <= sccpSourceStateMaxProofLabelBytes else {
        throw SolanaSccpProverError.invalidString("proofFamily")
    }
    guard circuitId.utf8.count <= sccpSourceStateMaxProofLabelBytes else {
        throw SolanaSccpProverError.invalidString("circuitId")
    }
    guard sourceDomain == sccpDomainSolana else {
        throw SolanaSccpProverError.invalidString("sourceDomain")
    }
    guard solanaSccpSourceStateVerificationCircuitIds.contains(circuitId) else {
        throw SolanaSccpProverError.invalidString("circuitId")
    }
    guard !proofBytes.isEmpty else {
        throw SolanaSccpProverError.emptyProof
    }
    guard proofBytes.count <= sccpSourceStateMaxProofBytes else {
        throw SolanaSccpProverError.invalidString("proofBytes")
    }
    guard proofBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.allZeroProof
    }
    return SolanaSccpSourceStateVerificationProof(
        version: version,
        proofFamily: proofFamily,
        circuitId: circuitId,
        proofBytes: proofBytes
    )
}

/// Hash the nested AccountsLtHash source-state proof capsule.
public func solanaSccpAccountsLtHashProofHash(
    _ proof: SolanaSccpSourceStateVerificationProof
) throws -> String {
    guard proof.circuitId == sccpSolanaAccountsLtHashOpenVerifyCircuitIdV1 else {
        throw SolanaSccpProverError.invalidString("accountsLtHashProof")
    }
    return try hashHex(
        prefix: "sccp:solana:accounts-lt-proof:v1",
        payload: canonicalSolanaSccpSourceStateVerificationProofBytes(proof)
    )
}

/// Canonical Solana finality-context bytes bound into full light-client audit requests.
public func canonicalSolanaSccpFullLightClientAuditFinalityContextBytes(
    _ input: SolanaSccpFullLightClientAuditProofInput
) throws -> Data {
    try canonicalFullLightClientAuditFinalityContextBytes(
        normalizeFullLightClientAuditInput(input, role: .towerReplay).context
    )
}

/// Hash Solana finality-context bytes bound into full light-client audit requests.
public func solanaSccpFullLightClientAuditFinalityContextHash(
    _ input: SolanaSccpFullLightClientAuditProofInput
) throws -> String {
    try hashHex(
        prefix: "sccp:solana:finality-context:v1",
        payload: canonicalSolanaSccpFullLightClientAuditFinalityContextBytes(input)
    )
}

/// Canonical finalized-vote message bytes bound into full light-client audit requests.
public func canonicalSolanaSccpFullLightClientAuditVoteMessageBytes(
    _ input: SolanaSccpFullLightClientAuditProofInput
) throws -> Data {
    let value = try normalizeFullLightClientAuditInput(input, role: .towerReplay)
    return try canonicalFullLightClientAuditVoteMessageBytes(
        witness: value.witness,
        finalityContextHash: value.finalityContextHash
    )
}

/// Hash the finalized-vote message bound into full light-client audit requests.
public func solanaSccpFullLightClientAuditVoteMessageHash(
    _ input: SolanaSccpFullLightClientAuditProofInput
) throws -> String {
    try hashHex(
        prefix: "sccp:solana:finalized-vote:v1",
        payload: canonicalSolanaSccpFullLightClientAuditVoteMessageBytes(input)
    )
}

/// Canonical statement bytes for one Solana full light-client audit role.
public func canonicalSolanaSccpFullLightClientAuditStatementBytes(
    _ input: SolanaSccpFullLightClientAuditProofInput,
    role: SolanaSccpFullLightClientAuditRole
) throws -> Data {
    let value = try normalizeFullLightClientAuditInput(input, role: role)
    let profile = auditRoleProfile(role)
    let witness = value.witness
    let context = value.context
    var out = Data()
    out.append(1)
    out.append(profile.code)
    try appendString(profile.circuitId, field: "circuitId", to: &out)
    try appendString(sccpSolanaRecursiveProofBackendV1, field: "backend", to: &out)
    try appendString(sccpSolanaMainnetGenesisHash, field: "mainnetGenesisHash", to: &out)
    appendU32Le(sccpDomainSolana, to: &out)
    appendU64Le(context.epoch, to: &out)
    appendU64Le(witness.finalizedSlot, to: &out)
    appendU64Le(context.rootedSlot, to: &out)
    appendU64Le(context.parentSlot, to: &out)
    try out.append(bytesFromHex32(value.finalityContextHash, field: "finalityContextHash"))
    try out.append(bytesFromHex32(value.voteMessageHash, field: "voteMessageHash"))
    try out.append(bytesFromHex32(value.accountsLtHashProofHash, field: "accountsLtHashProofHash"))
    switch role {
    case .towerReplay:
        try out.append(bytesFromHex32(context.towerLockoutHash, field: "towerLockoutHash"))
        try out.append(bytesFromHex32(context.towerReplayHash, field: "towerReplayHash"))
        try out.append(bytesFromHex32(context.bankForkHash, field: "bankForkHash"))
        try out.append(bytesFromHex32(context.epochStakeRoot, field: "epochStakeRoot"))
        try out.append(bytesFromHex32(context.stakeActivationHash, field: "stakeActivationHash"))
        try out.append(bytesFromHex32(context.stakeAccountStateHash, field: "stakeAccountStateHash"))
        try out.append(bytesFromHex32(context.stakeHistoryHash, field: "stakeHistoryHash"))
        try out.append(bytesFromHex32(context.stakeHistorySysvarAccountHash, field: "stakeHistorySysvarAccountHash"))
        try out.append(bytesFromHex32(context.accountInclusionRoot, field: "accountInclusionRoot"))
        appendU32Le(UInt32(context.towerVoteSlots.count), to: &out)
        context.towerVoteSlots.forEach { appendU64Le($0, to: &out) }
    case .fullAccountsdbLattice:
        try out.append(bytesFromHex32(context.accountInclusionRoot, field: "accountInclusionRoot"))
        try out.append(bytesFromHex32(context.accountsLtHashChecksum, field: "accountsLtHashChecksum"))
        try out.append(
            bytesFromHex32(
                context.accountsLtHashProofPublicInputsHash,
                field: "accountsLtHashProofPublicInputsHash"
            )
        )
        try out.append(
            bytesFromHex32(
                value.openedAccountsLtHashContributionsHash,
                field: "openedAccountsLtHashContributionsHash"
            )
        )
        try out.append(
            bytesFromHex32(
                value.openedAccountsLtHashResidualChecksum,
                field: "openedAccountsLtHashResidualChecksum"
            )
        )
        try out.append(bytesFromHex32(value.accountsLtHashProofHash, field: "accountsLtHashProofHash"))
    case .bankForkChoice:
        try out.append(bytesFromHex32(context.parentBankHash, field: "parentBankHash"))
        try out.append(bytesFromHex32(witness.bankHash, field: "bankHash"))
        try out.append(bytesFromHex32(witness.blockhash, field: "blockhash"))
        try out.append(bytesFromHex32(witness.transactionStatusRoot, field: "transactionStatusRoot"))
        try out.append(bytesFromHex32(context.accountInclusionRoot, field: "accountInclusionRoot"))
        try out.append(bytesFromHex32(context.accountsLtHashChecksum, field: "accountsLtHashChecksum"))
        appendU64Le(context.bankSignatureCount, to: &out)
        appendBytesVec(context.bankHashHardForkData, to: &out)
        try out.append(bytesFromHex32(context.bankForkHash, field: "bankForkHash"))
        try out.append(bytesFromHex32(context.towerReplayHash, field: "towerReplayHash"))
    }
    return out
}

/// Hash one Solana full light-client audit role statement.
public func solanaSccpFullLightClientAuditStatementHash(
    _ input: SolanaSccpFullLightClientAuditProofInput,
    role: SolanaSccpFullLightClientAuditRole
) throws -> String {
    try hashHex(
        prefix: sccpSolanaFullLightClientAuditStatementPrefixV1,
        payload: canonicalSolanaSccpFullLightClientAuditStatementBytes(input, role: role)
    )
}

private func requireFullLightClientAuditRoleRequestHashSeparation(
    _ value: NormalizedSolanaSccpFullLightClientAuditInput,
    statementHash: String
) throws {
    let requestHashes = [
        value.witness.sourceStateVerifierHash,
        value.sourceVerifierMaterialHash,
        value.sourceAdapterDeploymentHash,
        value.fullLightClientGateHash,
        value.finalityContextHash,
        value.voteMessageHash,
        value.accountsLtHashProofHash,
        statementHash,
    ]
    guard !requestHashes.contains(value.verifierHash) else {
        throw SolanaSccpProverError.invalidString("verifierHash")
    }
}

/// OpenVerify public-input columns for one Solana full light-client audit role.
public func solanaSccpFullLightClientAuditPublicInputColumns(
    _ input: SolanaSccpFullLightClientAuditProofInput,
    role: SolanaSccpFullLightClientAuditRole
) throws -> [[String]] {
    let value = try normalizeFullLightClientAuditInput(input, role: role)
    let profile = auditRoleProfile(role)
    var columns = [
        ["0x" + sccpWordU8(profile.code).hexEncodedString()],
        ["0x" + sccpWordU32Le(sccpDomainSolana).hexEncodedString()],
        [solanaSccpMainnetGenesisHashPublicInput()],
        ["0x" + sccpWordU64Le(value.witness.finalizedSlot).hexEncodedString()],
        [value.finalityContextHash],
        [try solanaSccpFullLightClientAuditStatementHash(input, role: role)],
        [value.sourceVerifierMaterialHash],
        [value.sourceAdapterDeploymentHash],
        [value.fullLightClientGateHash],
        [value.verifierHash],
        ["0x" + sccpWordU64Le(value.context.epoch).hexEncodedString()],
        ["0x" + sccpWordU64Le(value.context.rootedSlot).hexEncodedString()],
        ["0x" + sccpWordU64Le(value.context.parentSlot).hexEncodedString()],
        [value.voteMessageHash],
        [value.accountsLtHashProofHash],
    ]
    for column in auditRoleColumns(value, role: role) {
        columns.append([column])
    }
    return columns
}

/// OpenVerify schema descriptor for one Solana full light-client audit role.
public func solanaSccpFullLightClientAuditOpenVerifySchemaDescriptor(
    _ input: SolanaSccpFullLightClientAuditProofInput,
    role: SolanaSccpFullLightClientAuditRole
) throws -> Data {
    let value = try normalizeFullLightClientAuditInput(input, role: role)
    let profile = auditRoleProfile(role)
    var out = Data()
    out.append(1)
    out.append(profile.code)
    try appendString(profile.circuitId, field: "circuitId", to: &out)
    try appendString(sccpSolanaFullLightClientAuditFastpqParameterSetV1, field: "parameterSet", to: &out)
    try appendString(sccpSolanaMainnetGenesisHash, field: "mainnetGenesisHash", to: &out)
    appendU32Le(sccpDomainSolana, to: &out)
    try appendString("verifier_id", field: "schemaField", to: &out)
    try appendString(profile.verifierId, field: "verifierId", to: &out)
    try appendString("verifier_hash", field: "schemaField", to: &out)
    try out.append(bytesFromHex32(value.verifierHash, field: "verifierHash"))
    try appendString("source_verifier_material_hash", field: "schemaField", to: &out)
    try out.append(bytesFromHex32(value.sourceVerifierMaterialHash, field: "sourceVerifierMaterialHash"))
    try appendString("source_adapter_deployment_hash", field: "schemaField", to: &out)
    try out.append(bytesFromHex32(value.sourceAdapterDeploymentHash, field: "sourceAdapterDeploymentHash"))
    try appendString("full_light_client_gate_hash", field: "schemaField", to: &out)
    try out.append(bytesFromHex32(value.fullLightClientGateHash, field: "fullLightClientGateHash"))
    for requiredInput in [
        "role",
        "source_domain",
        "mainnet_genesis_hash",
        "finalized_slot",
        "finality_context_hash",
        "audit_statement_hash",
        "source_verifier_material_hash",
        "source_adapter_deployment_hash",
        "full_light_client_gate_hash",
        "verifier_hash",
        "epoch",
        "rooted_slot",
        "parent_slot",
        "vote_message_hash",
        "accounts_lt_hash_proof_hash",
    ] + profile.requiredInputNames {
        try appendString(requiredInput, field: "requiredInput", to: &out)
    }
    return out
}

/// Build an OpenVerify request for one Solana full light-client audit role.
public func buildSolanaSccpFullLightClientAuditProofRequest(
    _ input: SolanaSccpFullLightClientAuditProofInput,
    role: SolanaSccpFullLightClientAuditRole
) throws -> SolanaSccpFullLightClientAuditProofRequest {
    let value = try normalizeFullLightClientAuditInput(input, role: role)
    let profile = auditRoleProfile(role)
    let statementBytes = try canonicalSolanaSccpFullLightClientAuditStatementBytes(input, role: role)
    let statementHash = try solanaSccpFullLightClientAuditStatementHash(input, role: role)
    try requireFullLightClientAuditRoleRequestHashSeparation(value, statementHash: statementHash)
    let contextBytes = try canonicalFullLightClientAuditContextBytes(value, statementHash: statementHash)
    let transitions = [
        SolanaSccpFullLightClientAuditFastpqTransition(
            key: "0x" + fullLightClientAuditFastpqKey(
                sccpSolanaFullLightClientAuditFastpqStatementKeyV1,
                profile: profile
            ).hexEncodedString(),
            operation: "meta_set",
            oldValue: Data(),
            newValue: statementBytes
        ),
        SolanaSccpFullLightClientAuditFastpqTransition(
            key: "0x" + fullLightClientAuditFastpqKey(
                sccpSolanaFullLightClientAuditFastpqContextKeyV1,
                profile: profile
            ).hexEncodedString(),
            operation: "meta_set",
            oldValue: Data(),
            newValue: contextBytes
        ),
        SolanaSccpFullLightClientAuditFastpqTransition(
            key: "0x" + fullLightClientAuditFastpqKey(
                sccpSolanaFullLightClientAuditFastpqGateKeyV1,
                profile: profile
            ).hexEncodedString(),
            operation: "meta_set",
            oldValue: Data(),
            newValue: try bytesFromHex32(value.fullLightClientGateHash, field: "fullLightClientGateHash")
        ),
    ].sorted { $0.key < $1.key }
    return SolanaSccpFullLightClientAuditProofRequest(
        version: 1,
        proofFamily: sccpStarkFriProofFamilyV1,
        circuitId: profile.circuitId,
        parameterSet: sccpSolanaFullLightClientAuditFastpqParameterSetV1,
        role: profile.name,
        roleCode: profile.code,
        sourceDomain: sccpDomainSolana,
        finalizedSlot: String(value.witness.finalizedSlot),
        verifierId: profile.verifierId,
        verifierHash: value.verifierHash,
        sourceStateVerifierId: value.witness.sourceStateVerifierId,
        sourceStateVerifierHash: value.witness.sourceStateVerifierHash,
        sourceVerifierMaterialHash: value.sourceVerifierMaterialHash,
        sourceAdapterDeploymentHash: value.sourceAdapterDeploymentHash,
        fullLightClientGateHash: value.fullLightClientGateHash,
        finalityContextHash: value.finalityContextHash,
        voteMessageHash: value.voteMessageHash,
        accountsLtHashProofHash: value.accountsLtHashProofHash,
        auditStatementHash: statementHash,
        statementBytes: statementBytes,
        verificationContextBytes: contextBytes,
        schemaDescriptor: try solanaSccpFullLightClientAuditOpenVerifySchemaDescriptor(input, role: role),
        publicInputColumns: try solanaSccpFullLightClientAuditPublicInputColumns(input, role: role),
        fastpqPublicInputs: try fullLightClientAuditFastpqPublicInputs(value, statementHash: statementHash),
        fastpqTransitions: transitions
    )
}

public func buildSolanaSccpTowerReplayProofRequest(
    _ input: SolanaSccpFullLightClientAuditProofInput
) throws -> SolanaSccpFullLightClientAuditProofRequest {
    try buildSolanaSccpFullLightClientAuditProofRequest(input, role: .towerReplay)
}

public func buildSolanaSccpFullAccountsdbLatticeProofRequest(
    _ input: SolanaSccpFullLightClientAuditProofInput
) throws -> SolanaSccpFullLightClientAuditProofRequest {
    try buildSolanaSccpFullLightClientAuditProofRequest(input, role: .fullAccountsdbLattice)
}

public func buildSolanaSccpBankForkChoiceProofRequest(
    _ input: SolanaSccpFullLightClientAuditProofInput
) throws -> SolanaSccpFullLightClientAuditProofRequest {
    try buildSolanaSccpFullLightClientAuditProofRequest(input, role: .bankForkChoice)
}

public func buildSolanaSccpFullLightClientAuditProofRequests(
    _ input: SolanaSccpFullLightClientAuditProofInput
) throws -> SolanaSccpFullLightClientAuditProofRequests {
    try SolanaSccpFullLightClientAuditProofRequests(
        towerReplay: buildSolanaSccpTowerReplayProofRequest(input),
        fullAccountsdbLattice: buildSolanaSccpFullAccountsdbLatticeProofRequest(input),
        bankForkChoice: buildSolanaSccpBankForkChoiceProofRequest(input)
    )
}

/// Wrap externally generated Solana SCCP proof bytes against a canonical production request.
public func wrapSolanaSccpProofResult(
    proofBytes: Data,
    request: SolanaSccpProofRequest
) throws -> SolanaSccpProofResult {
    guard !proofBytes.isEmpty else {
        throw SolanaSccpProverError.emptyProof
    }
    guard proofBytes.count <= sccpNativeRecursiveMaxProofBytes else {
        throw SolanaSccpProverError.invalidString("proofBytes")
    }
    guard proofBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.allZeroProof
    }
    try requireCanonicalSolanaSccpProofRequest(request)
    try requireProductionSolanaSccpProofRequest(request)
    var envelopePayload = try bytesFromHex32(request.witnessHash, field: "witnessHash")
    try envelopePayload.append(bytesFromHex32(request.proofContextHash, field: "proofContextHash"))
    try envelopePayload.append(
        bytesFromHex32(
            request.sourceAdapterDeploymentBindingHash,
            field: "sourceAdapterDeploymentBindingHash"
        )
    )
    envelopePayload.append(proofBytes)
    return SolanaSccpProofResult(
        version: 1,
        backend: request.backend,
        proofBytes: proofBytes,
        proofBase64: proofBytes.base64EncodedString(),
        publicInputs: request.publicInputs,
        witnessHash: request.witnessHash,
        proofContextHash: request.proofContextHash,
        sourceAdapterDeploymentBindingHash: request.sourceAdapterDeploymentBindingHash,
        sourceStateVerifierId: request.sourceStateVerifierId,
        sourceStateVerifierHash: request.sourceStateVerifierHash,
        proofContext: request.proofContext,
        sourceAdapterDeploymentBinding: request.sourceAdapterDeploymentBinding,
        envelopeHash: hashHex(prefix: "sccp:solana:proof-envelope:v1", payload: envelopePayload)
    )
}

private func requireSolanaProofResultSourcePublicInputShape(
    _ publicInputs: SolanaSccpPublicInputs
) throws {
    let parentNext = publicInputs.parentSlot.addingReportingOverflow(1)
    guard !parentNext.overflow, parentNext.partialValue == publicInputs.finalizedSlot else {
        throw SolanaSccpProverError.invalidString("proofResult.publicInputs.parentSlot")
    }
    guard publicInputs.bankSignatureCount != 0 else {
        throw SolanaSccpProverError.invalidString("proofResult.publicInputs.bankSignatureCount")
    }
    _ = try normalizeNonZeroHex32(publicInputs.parentBankHash, field: "proofResult.publicInputs.parentBankHash")
    _ = try normalizeNonZeroHex32(publicInputs.blockhash, field: "proofResult.publicInputs.blockhash")
    _ = try normalizeNonZeroHex32(publicInputs.bankHash, field: "proofResult.publicInputs.bankHash")
    _ = try normalizeNonZeroHex32(
        publicInputs.transactionStatusRoot,
        field: "proofResult.publicInputs.transactionStatusRoot"
    )
    _ = try normalizeNonZeroHex32(publicInputs.messageProofHash, field: "proofResult.publicInputs.messageProofHash")
    _ = try normalizeNonZeroHex32(publicInputs.accountInclusionRoot, field: "proofResult.publicInputs.accountInclusionRoot")
    _ = try normalizeNonZeroHex32(publicInputs.accountsLtHashChecksum, field: "proofResult.publicInputs.accountsLtHashChecksum")
    _ = try normalizeNonZeroHex32(
        publicInputs.accountsLtHashProofPublicInputsHash,
        field: "proofResult.publicInputs.accountsLtHashProofPublicInputsHash"
    )
    _ = try normalizeNonZeroHex32(publicInputs.sourceEventDigest, field: "proofResult.publicInputs.sourceEventDigest")
}

private func requireWrappedSolanaProofResultForSubmission(
    _ proofResult: SolanaSccpProofResult,
    publicInputs: SolanaSccpSubmissionPublicInputs
) throws -> SolanaSccpProofResult {
    guard proofResult.version == 1 else {
        throw SolanaSccpProverError.invalidString("proofResult.version")
    }
    guard proofResult.backend == sccpSolanaRecursiveProofBackendV1 else {
        throw SolanaSccpProverError.invalidString("proofResult.backend")
    }
    guard proofResult.proofContext.version == 1 else {
        throw SolanaSccpProverError.invalidString("proofResult.proofContext.version")
    }
    let expectedProofContextHash = try solanaSccpProofContextHash(proofResult.proofContext)
    guard try normalizeHex32(proofResult.proofContextHash, field: "proofResult.proofContextHash")
        == expectedProofContextHash else {
        throw SolanaSccpProverError.proofContextHashMismatch
    }
    guard !proofResult.proofBytes.isEmpty else {
        throw SolanaSccpProverError.emptyProof
    }
    guard proofResult.proofBytes.count <= sccpNativeRecursiveMaxProofBytes else {
        throw SolanaSccpProverError.invalidString("proofResult.proofBytes")
    }
    guard proofResult.proofBytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.allZeroProof
    }
    guard proofResult.proofBase64 == proofResult.proofBytes.base64EncodedString() else {
        throw SolanaSccpProverError.invalidString("proofResult.proofBase64")
    }
    let envelopeHash = try normalizeHex32(proofResult.envelopeHash, field: "proofResult.envelopeHash")
    guard envelopeHash != sccpZeroHashV1 else {
        throw SolanaSccpProverError.invalidHex32("proofResult.envelopeHash")
    }
    let sourceAdapterDeploymentBindingHash = try normalizeHex32(
        proofResult.sourceAdapterDeploymentBindingHash,
        field: "proofResult.sourceAdapterDeploymentBindingHash"
    )
    guard sourceAdapterDeploymentBindingHash != sccpZeroHashV1 else {
        throw SolanaSccpProverError.invalidHex32("proofResult.sourceAdapterDeploymentBindingHash")
    }
    let deploymentBinding = proofResult.sourceAdapterDeploymentBinding
    guard deploymentBinding.version == 1 else {
        throw SolanaSccpProverError.invalidString("proofResult.sourceAdapterDeploymentBinding.version")
    }
    guard deploymentBinding.sourceDomain == sccpDomainSolana,
          deploymentBinding.targetDomain == sccpDomainSora else {
        throw SolanaSccpProverError.invalidString("proofResult.sourceAdapterDeploymentBinding")
    }
    let deploymentHash = try normalizeHex32(
        deploymentBinding.sourceAdapterDeploymentHash,
        field: "proofResult.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash"
    )
    let deploymentReceiptHash = try normalizeHex32(
        deploymentBinding.sourceAdapterDeploymentReceiptHash,
        field: "proofResult.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash"
    )
    guard deploymentHash != sccpZeroHashV1,
          deploymentReceiptHash != sccpZeroHashV1 else {
        throw SolanaSccpProverError.invalidHex32("proofResult.sourceAdapterDeploymentBinding")
    }
    let expectedSourceAdapterDeploymentBindingHash = try sccpSourceAdapterDeploymentBindingHash(deploymentBinding)
    guard sourceAdapterDeploymentBindingHash == expectedSourceAdapterDeploymentBindingHash else {
        throw SolanaSccpProverError.invalidString("proofResult.sourceAdapterDeploymentBindingHash")
    }
    let witnessHash = try normalizeNonZeroHex32(proofResult.witnessHash, field: "proofResult.witnessHash")
    var envelopePayload = try bytesFromHex32(witnessHash, field: "proofResult.witnessHash")
    try envelopePayload.append(bytesFromHex32(expectedProofContextHash, field: "proofResult.proofContextHash"))
    try envelopePayload.append(
        bytesFromHex32(
            sourceAdapterDeploymentBindingHash,
            field: "proofResult.sourceAdapterDeploymentBindingHash"
        )
    )
    envelopePayload.append(proofResult.proofBytes)
    guard envelopeHash == hashHex(prefix: "sccp:solana:proof-envelope:v1", payload: envelopePayload) else {
        throw SolanaSccpProverError.invalidString("proofResult.envelopeHash")
    }
    guard proofResult.sourceStateVerifierId == sccpSolanaMainnetAccountsDbVerifierIdV1 else {
        throw SolanaSccpProverError.invalidString("proofResult.sourceStateVerifierId")
    }
    let sourceStateVerifierHash = try normalizeHex32(
        proofResult.sourceStateVerifierHash,
        field: "proofResult.sourceStateVerifierHash"
    )
    guard sourceStateVerifierHash != sccpZeroHashV1 else {
        throw SolanaSccpProverError.invalidHex32("proofResult.sourceStateVerifierHash")
    }
    guard sourceStateVerifierHash != sccpSolanaTemplateSourceStateVerifierHashV1 else {
        throw SolanaSccpProverError.invalidHex32("proofResult.sourceStateVerifierHash")
    }
    guard proofResult.publicInputs.sourceStateVerifierId == proofResult.sourceStateVerifierId else {
        throw SolanaSccpProverError.invalidString("proofResult.publicInputs.sourceStateVerifierId")
    }
    guard try normalizeHex32(
        proofResult.publicInputs.sourceStateVerifierHash,
        field: "proofResult.publicInputs.sourceStateVerifierHash"
    ) == sourceStateVerifierHash else {
        throw SolanaSccpProverError.invalidString("proofResult.publicInputs.sourceStateVerifierHash")
    }
    try requireSolanaProofResultSourcePublicInputShape(proofResult.publicInputs)
    let proofContextStatementHash = try normalizeHex32(
        proofResult.proofContext.statementHash,
        field: "proofResult.proofContext.statementHash"
    )
    let proofContextDestinationBindingHash = try normalizeHex32(
        proofResult.proofContext.destinationBindingHash,
        field: "proofResult.proofContext.destinationBindingHash"
    )
    guard try normalizeHex32(
        proofResult.publicInputs.statementHash,
        field: "proofResult.publicInputs.statementHash"
    ) == proofContextStatementHash else {
        throw SolanaSccpProverError.invalidString("proofResult.publicInputs.statementHash")
    }
    guard try normalizeHex32(
        proofResult.publicInputs.destinationBindingHash,
        field: "proofResult.publicInputs.destinationBindingHash"
    ) == proofContextDestinationBindingHash else {
        throw SolanaSccpProverError.invalidString("proofResult.publicInputs.destinationBindingHash")
    }
    guard try normalizeHex32(
        proofResult.publicInputs.sourceAdapterDeploymentHash,
        field: "proofResult.publicInputs.sourceAdapterDeploymentHash"
    ) == deploymentHash else {
        throw SolanaSccpProverError.invalidString("proofResult.publicInputs.sourceAdapterDeploymentHash")
    }
    guard try normalizeHex32(
        proofResult.publicInputs.sourceAdapterDeploymentReceiptHash,
        field: "proofResult.publicInputs.sourceAdapterDeploymentReceiptHash"
    ) == deploymentReceiptHash else {
        throw SolanaSccpProverError.invalidString("proofResult.publicInputs.sourceAdapterDeploymentReceiptHash")
    }
    guard try normalizeHex32(
        proofResult.publicInputs.sourceAdapterDeploymentBindingHash,
        field: "proofResult.publicInputs.sourceAdapterDeploymentBindingHash"
    ) == expectedSourceAdapterDeploymentBindingHash else {
        throw SolanaSccpProverError.invalidString("proofResult.publicInputs.sourceAdapterDeploymentBindingHash")
    }
    guard try normalizeHex32(
        proofResult.publicInputs.messageId,
        field: "proofResult.publicInputs.messageId"
    ) == normalizeHex32(publicInputs.messageId, field: "publicInputs.messageId") else {
        throw SolanaSccpProverError.invalidString("proofResult.publicInputs.messageId")
    }
    guard try normalizeHex32(
        proofResult.publicInputs.payloadHash,
        field: "proofResult.publicInputs.payloadHash"
    ) == normalizeHex32(publicInputs.payloadHash, field: "publicInputs.payloadHash") else {
        throw SolanaSccpProverError.invalidString("proofResult.publicInputs.payloadHash")
    }
    guard try normalizeHex32(
        proofResult.publicInputs.commitmentRoot,
        field: "proofResult.publicInputs.commitmentRoot"
    ) == normalizeHex32(publicInputs.commitmentRoot, field: "publicInputs.commitmentRoot") else {
        throw SolanaSccpProverError.invalidString("proofResult.publicInputs.commitmentRoot")
    }
    guard proofResult.publicInputs.finalizedSlot == publicInputs.finalityHeight else {
        throw SolanaSccpProverError.invalidString("proofResult.publicInputs.finalizedSlot")
    }
    guard try normalizeHex32(
        proofResult.publicInputs.bankHash,
        field: "proofResult.publicInputs.bankHash"
    ) == normalizeHex32(publicInputs.finalityBlockHash, field: "publicInputs.finalityBlockHash") else {
        throw SolanaSccpProverError.invalidString("proofResult.publicInputs.bankHash")
    }
    return proofResult
}

func normalizeSolanaSccpProofResult(
    proofBytes: Data,
    request: SolanaSccpProofRequest
) throws -> SolanaSccpProofResult {
    try wrapSolanaSccpProofResult(proofBytes: proofBytes, request: request)
}

private func requireProductionSolanaSccpProofRequest(_ request: SolanaSccpProofRequest) throws {
    guard request.sourceDomain == sccpDomainSolana && request.targetDomain == sccpDomainSora else {
        throw SolanaSccpProverError.invalidString("targetDomain")
    }
    guard request.mainnetGenesisHash == sccpSolanaMainnetGenesisHash,
          request.witness.mainnetGenesisHash == sccpSolanaMainnetGenesisHash else {
        throw SolanaSccpProverError.invalidString("mainnetGenesisHash")
    }
    guard request.sourceStateVerifierId == sccpSolanaMainnetAccountsDbVerifierIdV1 else {
        throw SolanaSccpProverError.invalidString("sourceStateVerifierId")
    }
    let sourceStateVerifierHash = try normalizeHex32(request.sourceStateVerifierHash, field: "sourceStateVerifierHash")
    guard sourceStateVerifierHash != sccpZeroHashV1 else {
        throw SolanaSccpProverError.invalidHex32("sourceStateVerifierHash")
    }
    guard sourceStateVerifierHash != sccpSolanaTemplateSourceStateVerifierHashV1 else {
        throw SolanaSccpProverError.invalidHex32("sourceStateVerifierHash")
    }
    guard !request.witness.inclusionBranch.isEmpty else {
        throw SolanaSccpProverError.invalidString("inclusionBranch")
    }
    guard let accountsLtHash = request.witness.accountsLtHash else {
        throw SolanaSccpProverError.invalidString("accountsLtHash")
    }
    guard accountsLtHash.count == sccpSolanaAccountsLtHashBytes,
          accountsLtHash.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidString("accountsLtHash")
    }
    guard try normalizeHex32(
        request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash,
        field: "sourceAdapterDeploymentHash"
    ) != sccpZeroHashV1 else {
        throw SolanaSccpProverError.invalidHex32("sourceAdapterDeploymentHash")
    }
    guard try normalizeHex32(
        request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash,
        field: "sourceAdapterDeploymentReceiptHash"
    ) != sccpZeroHashV1 else {
        throw SolanaSccpProverError.invalidHex32("sourceAdapterDeploymentReceiptHash")
    }
}

private func requireCanonicalSolanaSccpProofRequest(_ request: SolanaSccpProofRequest) throws {
    let expected = try buildSolanaSccpProofRequest(SolanaSccpWitnessInput(
        targetDomain: request.witness.targetDomain,
        mainnetGenesisHash: request.witness.mainnetGenesisHash,
        finalizedSlot: request.witness.finalizedSlot,
        parentSlot: request.witness.parentSlot,
        bankSignatureCount: request.witness.bankSignatureCount,
        parentBankHash: request.witness.parentBankHash,
        blockhash: request.witness.blockhash,
        bankHash: request.witness.bankHash,
        transactionStatusRoot: request.witness.transactionStatusRoot,
        messageProofHash: request.witness.messageProofHash,
        accountInclusionRoot: request.witness.accountInclusionRoot,
        accountsLtHashChecksum: request.witness.accountsLtHashChecksum,
        accountsLtHashProofPublicInputsHash: request.witness.accountsLtHashProofPublicInputsHash,
        bankHashHardForkData: request.witness.bankHashHardForkData,
        accountsLtHash: request.witness.accountsLtHash,
        transactionSignature: request.witness.transactionSignature,
        emitterProgramId: request.witness.emitterProgramId,
        messageId: request.witness.messageId,
        payloadHash: request.witness.payloadHash,
        commitmentRoot: request.witness.commitmentRoot,
        sourceEventDigest: request.witness.sourceEventDigest,
        sourceStateVerifierId: request.witness.sourceStateVerifierId,
        sourceStateVerifierHash: request.witness.sourceStateVerifierHash,
        statementHash: request.proofContext.statementHash,
        destinationBindingHash: request.proofContext.destinationBindingHash,
        inclusionBranch: request.witness.inclusionBranch,
        sourceAdapterDeploymentHash: request.witness.sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash: request.witness.sourceAdapterDeploymentReceiptHash
    ))
    guard expected == request else {
        throw SolanaSccpProverError.invalidString("request")
    }
}

private func canonicalSolanaSccpWitnessBytes(_ witness: SolanaSccpWitness) throws -> Data {
    var out = Data()
    out.append(witness.version)
    appendU32Le(witness.sourceDomain, to: &out)
    appendU32Le(witness.targetDomain, to: &out)
    try appendString(witness.mainnetGenesisHash, field: "mainnetGenesisHash", to: &out)
    appendU64Le(witness.finalizedSlot, to: &out)
    appendU64Le(witness.parentSlot, to: &out)
    appendU64Le(witness.bankSignatureCount, to: &out)
    try out.append(solanaHash32Bytes(witness.blockhash, field: "blockhash"))
    try appendString(witness.transactionSignature, field: "transactionSignature", to: &out)
    try appendString(witness.emitterProgramId, field: "emitterProgramId", to: &out)
    try out.append(bytesFromHex32(witness.parentBankHash, field: "parentBankHash"))
    try out.append(bytesFromHex32(witness.bankHash, field: "bankHash"))
    try out.append(bytesFromHex32(witness.transactionStatusRoot, field: "transactionStatusRoot"))
    try out.append(bytesFromHex32(witness.messageProofHash, field: "messageProofHash"))
    try out.append(bytesFromHex32(witness.accountInclusionRoot, field: "accountInclusionRoot"))
    try out.append(bytesFromHex32(witness.accountsLtHashChecksum, field: "accountsLtHashChecksum"))
    try out.append(
        bytesFromHex32(
            witness.accountsLtHashProofPublicInputsHash,
            field: "accountsLtHashProofPublicInputsHash"
        )
    )
    appendBytesVec(witness.bankHashHardForkData, to: &out)
    appendBytesVec(witness.accountsLtHash ?? Data(), to: &out)
    try out.append(bytesFromHex32(witness.messageId, field: "messageId"))
    try out.append(bytesFromHex32(witness.payloadHash, field: "payloadHash"))
    try out.append(bytesFromHex32(witness.commitmentRoot, field: "commitmentRoot"))
    try out.append(bytesFromHex32(witness.sourceEventDigest, field: "sourceEventDigest"))
    try appendString(witness.sourceStateVerifierId, field: "sourceStateVerifierId", to: &out)
    try out.append(bytesFromHex32(witness.sourceStateVerifierHash, field: "sourceStateVerifierHash"))
    try out.append(bytesFromHex32(witness.sourceAdapterDeploymentHash, field: "sourceAdapterDeploymentHash"))
    try out.append(
        bytesFromHex32(
            witness.sourceAdapterDeploymentReceiptHash,
            field: "sourceAdapterDeploymentReceiptHash"
        )
    )
    appendU32Le(UInt32(witness.inclusionBranch.count), to: &out)
    for (index, sibling) in witness.inclusionBranch.enumerated() {
        guard sibling.count == 32 else {
            throw SolanaSccpProverError.invalidHex32("inclusionBranch[\(index)]")
        }
        out.append(sibling)
    }
    return out
}

private func normalizeSolanaMessageProofHash(
    _ value: String,
    sourceEventDigest: String,
    transactionStatusRoot: String,
    transactionSignature: String,
    emitterProgramId: String,
    inclusionBranch: [Data]
) throws -> String {
    guard !inclusionBranch.isEmpty else {
        return try normalizeHex32(value, field: "messageProofHash")
    }
    let derived = try solanaSccpMessageProofHash(
        sourceEventDigest: sourceEventDigest,
        transactionStatusRoot: transactionStatusRoot,
        transactionSignature: transactionSignature,
        emitterProgramId: emitterProgramId,
        inclusionBranch: inclusionBranch
    )
    if value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
        return derived
    }
    let provided = try normalizeHex32(value, field: "messageProofHash")
    guard provided == derived else {
        throw SolanaSccpProverError.messageProofHashMismatch
    }
    return provided
}

private func normalizeInclusionBranch(_ branch: [Data]) throws -> [Data] {
    guard branch.count <= sccpMaxSourceMerkleBranchNodes else {
        throw SolanaSccpProverError.invalidString("inclusionBranch")
    }
    for (index, sibling) in branch.enumerated() {
        guard sibling.count == 32 else {
            throw SolanaSccpProverError.invalidHex32("inclusionBranch[\(index)]")
        }
    }
    return branch
}

private func canonicalSolanaVoteRosterBytes(
    validatorPublicKeys: [Data],
    validatorStakes: [UInt64]
) throws -> Data {
    guard !validatorPublicKeys.isEmpty, validatorPublicKeys.count == validatorStakes.count else {
        throw SolanaSccpProverError.invalidString("validatorStakes")
    }
    guard validatorPublicKeys.count <= sccpSolanaMaxValidators else {
        throw SolanaSccpProverError.invalidString("validatorPublicKeys")
    }
    var seen = Set<Data>()
    var out = Data()
    out.append(1)
    appendU32Le(UInt32(validatorPublicKeys.count), to: &out)
    for (index, publicKey) in validatorPublicKeys.enumerated() {
        guard publicKey.count == 32 else {
            throw SolanaSccpProverError.invalidHex32("validatorPublicKeys[\(index)]")
        }
        guard publicKey.contains(where: { $0 != 0 }) else {
            throw SolanaSccpProverError.invalidString("validatorPublicKeys[\(index)]")
        }
        guard !seen.contains(publicKey) else {
            throw SolanaSccpProverError.invalidString("validatorPublicKeys")
        }
        guard validatorStakes[index] > 0 else {
            throw SolanaSccpProverError.invalidString("validatorStakes[\(index)]")
        }
        seen.insert(publicKey)
        appendBytesVec(publicKey, to: &out)
        appendU64Le(validatorStakes[index], to: &out)
    }
    return out
}

private func normalizeNonEmpty(_ value: String, field: String) throws -> String {
    let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else {
        throw SolanaSccpProverError.invalidString(field)
    }
    return trimmed
}

private func canonicalPositiveU64(_ value: String, field: String) throws -> UInt64 {
    let text = try normalizeNonEmpty(value, field: field)
    guard text == value, text.allSatisfy({ $0.unicodeScalars.allSatisfy { scalar in
        scalar.value >= 48 && scalar.value <= 57
    } }),
          let numeric = UInt64(text), numeric > 0, text == String(numeric) else {
        throw SolanaSccpProverError.invalidString(field)
    }
    return numeric
}

private func strictBase64Data(_ value: String, field: String) throws -> Data {
    guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value,
          let data = Data(base64Encoded: value),
          data.base64EncodedString() == value else {
        throw SolanaSccpProverError.invalidString(field)
    }
    return data
}

private func nonZeroHex32Bytes(_ value: String, field: String) throws -> Data {
    let bytes = try bytesFromHex32(value, field: field)
    guard bytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32(field)
    }
    return bytes
}

private func requireSolanaHashRolesDistinct(
    field: String,
    _ fields: [(String, Data)]
) throws {
    var seen: [Data: String] = [:]
    for (label, bytes) in fields {
        if seen[bytes] != nil {
            throw SolanaSccpProverError.invalidString(field)
        }
        seen[bytes] = label
    }
}

private func solanaUpgradeableProgramAccountData(programdataAddress: Data) -> Data {
    var out = Data()
    appendU32Le(sccpSolanaUpgradeableLoaderProgramTag, to: &out)
    out.append(programdataAddress)
    return out
}

private func solanaImmutableProgramdataMetadata(programdataSlot: UInt64) -> Data {
    var out = Data()
    appendU32Le(sccpSolanaUpgradeableLoaderProgramdataTag, to: &out)
    appendU64Le(programdataSlot, to: &out)
    out.append(0)
    out.append(Data(repeating: 0, count: 32))
    return out
}

private func solanaVerifierProgramCodeHash(_ programBytes: Data) throws -> String {
    guard !programBytes.isEmpty,
          programBytes.contains(where: { $0 != 0 }),
          programBytes.starts(with: sccpSolanaBpfElfMagic) else {
        throw SolanaSccpProverError.invalidString("solanaProgramdataExecutable")
    }
    return "0x" + Blake2b.hash256(programBytes).hexEncodedString()
}

private func normalizeSolanaRouteCanaryProgramDataEvidence(
    _ input: SolanaSccpRouteCanaryEvidenceInput
) throws -> NormalizedSolanaRouteCanaryProgramDataEvidence {
    let verifierProgram = try decodeSolanaBase58Fixed(
        input.verifierIdentity,
        field: "verifierIdentity",
        byteLength: sccpSolanaProgramIdBytes
    )
    let programdataAddress = try decodeSolanaBase58Fixed(
        input.solanaProgramdataAddress,
        field: "solanaProgramdataAddress",
        byteLength: sccpSolanaProgramIdBytes
    )
    guard verifierProgram != programdataAddress else {
        throw SolanaSccpProverError.invalidString("solanaProgramdataAddress")
    }
    let programdataSlot = try canonicalPositiveU64(input.solanaProgramdataSlot, field: "solanaProgramdataSlot")
    let expectedProgramdataSlot = try canonicalPositiveU64(
        input.solanaExpectedProgramdataSlot,
        field: "solanaExpectedProgramdataSlot"
    )
    guard programdataSlot == expectedProgramdataSlot else {
        throw SolanaSccpProverError.invalidString("solanaExpectedProgramdataSlot")
    }
    let programContextSlot = try canonicalPositiveU64(
        input.solanaProgramAccountContextSlot,
        field: "solanaProgramAccountContextSlot"
    )
    let programdataContextSlot = try canonicalPositiveU64(
        input.solanaProgramdataAccountContextSlot,
        field: "solanaProgramdataAccountContextSlot"
    )
    guard programContextSlot >= programdataSlot, programdataContextSlot >= programdataSlot else {
        throw SolanaSccpProverError.invalidString("solanaProgramdataAccountContextSlot")
    }
    let rpcCommitment = try normalizeNonEmpty(input.solanaRpcCommitment, field: "solanaRpcCommitment")
    guard rpcCommitment == "finalized" else {
        throw SolanaSccpProverError.invalidString("solanaRpcCommitment")
    }
    let programOwner = try normalizeNonEmpty(input.solanaProgramOwner, field: "solanaProgramOwner")
    let programdataOwner = try normalizeNonEmpty(input.solanaProgramdataOwner, field: "solanaProgramdataOwner")
    guard programOwner == sccpSolanaUpgradeableLoaderId else {
        throw SolanaSccpProverError.invalidString("solanaProgramOwner")
    }
    guard programdataOwner == sccpSolanaUpgradeableLoaderId else {
        throw SolanaSccpProverError.invalidString("solanaProgramdataOwner")
    }
    guard input.solanaProgramImmutable else {
        throw SolanaSccpProverError.invalidString("solanaProgramImmutable")
    }

    let programAccountData = try strictBase64Data(
        input.solanaProgramAccountDataBase64,
        field: "solanaProgramAccountDataBase64"
    )
    guard programAccountData == solanaUpgradeableProgramAccountData(programdataAddress: programdataAddress) else {
        throw SolanaSccpProverError.invalidString("solanaProgramAccountDataBase64")
    }
    let programdataMetadata = try strictBase64Data(
        input.solanaProgramdataMetadataBase64,
        field: "solanaProgramdataMetadataBase64"
    )
    guard programdataMetadata.count == sccpSolanaProgramdataMetadataLength,
          programdataMetadata == solanaImmutableProgramdataMetadata(programdataSlot: programdataSlot) else {
        throw SolanaSccpProverError.invalidString("solanaProgramdataMetadataBase64")
    }
    let metadataHash = "0x" + Blake2b.hash256(programdataMetadata).hexEncodedString()
    guard try normalizeNonZeroHex32(
        input.solanaProgramdataMetadataBlake2b256,
        field: "solanaProgramdataMetadataBlake2b256"
    ) == metadataHash else {
        throw SolanaSccpProverError.invalidString("solanaProgramdataMetadataBlake2b256")
    }
    let programdataExecutable = try strictBase64Data(
        input.solanaProgramdataExecutableBase64,
        field: "solanaProgramdataExecutableBase64"
    )
    let executableHash = try solanaVerifierProgramCodeHash(programdataExecutable)
    guard try normalizeNonZeroHex32(
        input.solanaProgramdataExecutableBlake2b256,
        field: "solanaProgramdataExecutableBlake2b256"
    ) == executableHash else {
        throw SolanaSccpProverError.invalidString("solanaProgramdataExecutableBlake2b256")
    }
    guard try normalizeNonZeroHex32(input.verifierCodeHash, field: "verifierCodeHash") == executableHash else {
        throw SolanaSccpProverError.invalidString("verifierCodeHash")
    }
    return NormalizedSolanaRouteCanaryProgramDataEvidence(
        verifierProgram: verifierProgram,
        verifierCodeHash: executableHash,
        rpcCommitment: rpcCommitment,
        programOwner: programOwner,
        programdataOwner: programdataOwner,
        programAccountData: programAccountData,
        programdataAddress: programdataAddress,
        programdataSlot: programdataSlot,
        expectedProgramdataSlot: expectedProgramdataSlot,
        programAccountContextSlot: programContextSlot,
        programdataAccountContextSlot: programdataContextSlot,
        programdataMetadata: programdataMetadata,
        programdataExecutable: programdataExecutable
    )
}

private func decodeSolanaBase58(_ value: String, field: String) throws -> Data {
    let text = try normalizeNonEmpty(value, field: field)
    var digits: [UInt8] = []
    for byte in text.utf8 {
        guard let digit = solanaBase58Index[byte] else {
            throw SolanaSccpProverError.invalidString(field)
        }
        var carry = Int(digit)
        for index in digits.indices {
            let value = Int(digits[index]) * 58 + carry
            digits[index] = UInt8(value & 0xff)
            carry = value >> 8
        }
        while carry > 0 {
            digits.append(UInt8(carry & 0xff))
            carry >>= 8
        }
    }
    let leadingZeros = text.utf8.prefix { $0 == 49 }.count
    var out = Data(repeating: 0, count: leadingZeros)
    out.append(contentsOf: digits.reversed())
    return out
}

private func decodeSolanaBase58Fixed(_ value: String, field: String, byteLength: Int) throws -> Data {
    let raw = try decodeSolanaBase58(value, field: field)
    guard raw.count == byteLength, raw.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidString(field)
    }
    return raw
}

private func normalizeSolanaBase58Fixed(_ value: String, field: String, byteLength: Int) throws -> String {
    let text = try normalizeNonEmpty(value, field: field)
    _ = try decodeSolanaBase58Fixed(text, field: field, byteLength: byteLength)
    return text
}

private func normalizeHex32(_ value: String, field: String) throws -> String {
    "0x" + (try bytesFromHex32(value, field: field)).hexEncodedString()
}

private func normalizeNonZeroHex32(_ value: String, field: String) throws -> String {
    let bytes = try bytesFromHex32(value, field: field)
    guard bytes.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32(field)
    }
    return "0x" + bytes.hexEncodedString()
}

private func normalizeHexBytes(_ value: String, field: String, byteCount: Int) throws -> String {
    "0x" + (try bytesFromHex(value, field: field, byteCount: byteCount)).hexEncodedString()
}

private func solanaHash32Bytes(_ value: String, field: String) throws -> Data {
    let text = try normalizeNonEmpty(value, field: field)
    let hex = text.hasPrefix("0x") ? String(text.dropFirst(2)) : text
    if hex.count == 64, solanaIsLowercaseHexBody(hex) {
        let bytes = try bytesFromHex32(text, field: field)
        guard bytes.contains(where: { $0 != 0 }) else {
            throw SolanaSccpProverError.invalidHex32(field)
        }
        return bytes
    }
    return try decodeSolanaBase58Fixed(text, field: field, byteLength: 32)
}

private struct SolanaSccpFullLightClientAuditRoleProfile {
    let name: String
    let code: UInt8
    let circuitId: String
    let verifierId: String
    let requiredInputNames: [String]
}

private struct NormalizedSolanaSccpFullLightClientAuditContext {
    let version: UInt8
    let epoch: UInt64
    let rootedSlot: UInt64
    let parentSlot: UInt64
    let towerVoteSlots: [UInt64]
    let parentBankHash: String
    let bankSignatureCount: UInt64
    let bankHashHardForkData: Data
    let epochStakeRoot: String
    let stakeActivationHash: String
    let stakeAccountStateHash: String
    let stakeHistoryHash: String
    let stakeHistorySysvarAccountHash: String
    let accountInclusionRoot: String
    let accountsLtHashChecksum: String
    let accountsLtHashProofPublicInputsHash: String
    let towerLockoutHash: String
    let towerReplayHash: String
    let bankForkHash: String
}

private struct NormalizedSolanaSccpFullLightClientAuditInput {
    let role: SolanaSccpFullLightClientAuditRole
    let witness: SolanaSccpWitness
    let context: NormalizedSolanaSccpFullLightClientAuditContext
    let sourceVerifierMaterialHash: String
    let sourceAdapterDeploymentHash: String
    let fullLightClientGateHash: String
    let verifierHash: String
    let finalityContextHash: String
    let voteMessageHash: String
    let accountsLtHashProofHash: String
    let openedAccountsLtHashContributionsHash: String
    let openedAccountsLtHashResidualChecksum: String
}

private func auditRoleProfile(_ role: SolanaSccpFullLightClientAuditRole) -> SolanaSccpFullLightClientAuditRoleProfile {
    switch role {
    case .towerReplay:
        return SolanaSccpFullLightClientAuditRoleProfile(
            name: "tower_replay",
            code: 1,
            circuitId: sccpSolanaTowerReplayOpenVerifyCircuitIdV1,
            verifierId: sccpSolanaMainnetTowerReplayVerifierIdV1,
            requiredInputNames: [
                "tower_lockout_hash",
                "tower_replay_hash",
                "bank_fork_hash",
                "epoch_stake_root",
                "stake_activation_hash",
                "stake_account_state_hash",
                "stake_history_hash",
                "stake_history_sysvar_account_hash",
                "account_inclusion_root",
            ]
        )
    case .fullAccountsdbLattice:
        return SolanaSccpFullLightClientAuditRoleProfile(
            name: "full_accountsdb_lattice",
            code: 2,
            circuitId: sccpSolanaFullAccountsdbLatticeOpenVerifyCircuitIdV1,
            verifierId: sccpSolanaMainnetFullAccountsdbLatticeVerifierIdV1,
            requiredInputNames: [
                "account_inclusion_root",
                "accounts_lt_hash_checksum",
                "accounts_lt_hash_proof_public_inputs_hash",
                "opened_accounts_lt_hash_contributions_hash",
                "opened_accounts_lt_hash_residual_checksum",
                "accounts_lt_hash_proof_hash",
            ]
        )
    case .bankForkChoice:
        return SolanaSccpFullLightClientAuditRoleProfile(
            name: "bank_fork_choice",
            code: 3,
            circuitId: sccpSolanaBankForkChoiceOpenVerifyCircuitIdV1,
            verifierId: sccpSolanaMainnetBankForkChoiceVerifierIdV1,
            requiredInputNames: [
                "parent_bank_hash",
                "bank_hash",
                "blockhash",
                "transaction_status_root",
                "account_inclusion_root",
                "accounts_lt_hash_checksum",
                "bank_signature_count",
                "bank_hash_hard_fork_data_hash",
                "bank_fork_hash",
                "tower_replay_hash",
            ]
        )
    }
}

private func roleVerifierHash(
    _ input: SolanaSccpFullLightClientAuditProofInput,
    role: SolanaSccpFullLightClientAuditRole
) throws -> String {
    switch role {
    case .towerReplay:
        return try normalizeNonZeroHex32(input.solanaTowerReplayVerifierHash, field: "solanaTowerReplayVerifierHash")
    case .fullAccountsdbLattice:
        return try normalizeNonZeroHex32(
            input.solanaFullAccountsdbLatticeVerifierHash,
            field: "solanaFullAccountsdbLatticeVerifierHash"
        )
    case .bankForkChoice:
        return try normalizeNonZeroHex32(
            input.solanaBankForkChoiceVerifierHash,
            field: "solanaBankForkChoiceVerifierHash"
        )
    }
}

private func requireFullLightClientAuditRoleSeparation(
    _ input: SolanaSccpFullLightClientAuditProofInput,
    witness: SolanaSccpWitness
) throws {
    let auditHashes = try [
        roleVerifierHash(input, role: .towerReplay),
        roleVerifierHash(input, role: .fullAccountsdbLattice),
        roleVerifierHash(input, role: .bankForkChoice),
    ]
    guard Set(auditHashes).count == auditHashes.count else {
        throw SolanaSccpProverError.invalidString("solanaFullLightClientAuditVerifierHashes")
    }
    guard auditHashes.allSatisfy({ !sccpSolanaTemplateSourceMaterialHashesV1.contains($0) }) else {
        throw SolanaSccpProverError.invalidString("solanaFullLightClientAuditVerifierHashes")
    }
    let existingHashes = try [
        normalizeNonZeroHex32(input.sourceTrustAnchorHash, field: "sourceTrustAnchorHash"),
        normalizeNonZeroHex32(input.consensusVerifierHash, field: "consensusVerifierHash"),
        normalizeNonZeroHex32(input.messageInclusionVerifierHash, field: "messageInclusionVerifierHash"),
        normalizeNonZeroHex32(input.finalityPolicyHash, field: "finalityPolicyHash"),
        witness.sourceStateVerifierHash,
        normalizeNonZeroHex32(
            input.adapterVerifierVkHash ?? sccpSourceAdapterVerifierVkHash(sourceDomain: sccpDomainSolana),
            field: "adapterVerifierVkHash"
        ),
        normalizeNonZeroHex32(input.sourceAdapterDeploymentReceiptHash, field: "sourceAdapterDeploymentReceiptHash"),
    ]
    guard auditHashes.allSatisfy({ auditHash in
        existingHashes.allSatisfy { existingHash in
            existingHash == sccpZeroHashV1 || existingHash != auditHash
        }
    }) else {
        throw SolanaSccpProverError.invalidString("solanaFullLightClientAuditVerifierHashes")
    }
}

private func solanaFullLightClientGateHashFromBoundHashes(
    sourceVerifierMaterialHash: String,
    sourceAdapterDeploymentHash: String,
    towerReplayVerifierHash: String,
    fullAccountsdbLatticeVerifierHash: String,
    bankForkChoiceVerifierHash: String
) throws -> String {
    let verifierHashes: [(String, Data)] = [
        (
            sccpSolanaMainnetTowerReplayVerifierIdV1,
            try bytesFromHex32(
                normalizeNonZeroHex32(towerReplayVerifierHash, field: "solanaTowerReplayVerifierHash"),
                field: "solanaTowerReplayVerifierHash"
            )
        ),
        (
            sccpSolanaMainnetFullAccountsdbLatticeVerifierIdV1,
            try bytesFromHex32(
                normalizeNonZeroHex32(
                    fullAccountsdbLatticeVerifierHash,
                    field: "solanaFullAccountsdbLatticeVerifierHash"
                ),
                field: "solanaFullAccountsdbLatticeVerifierHash"
            )
        ),
        (
            sccpSolanaMainnetBankForkChoiceVerifierIdV1,
            try bytesFromHex32(
                normalizeNonZeroHex32(bankForkChoiceVerifierHash, field: "solanaBankForkChoiceVerifierHash"),
                field: "solanaBankForkChoiceVerifierHash"
            )
        ),
    ]
    var out = Data()
    out.append(1)
    appendU32Le(sccpDomainSolana, to: &out)
    appendU32Le(sccpDomainSora, to: &out)
    try appendString(sccpSolanaSourceChainKeyV1, field: "sourceChain", to: &out)
    out.append(sccpSolanaSourceProofPlanCodeV1)
    out.append(sccpSolanaFinalityModelCodeV1)
    try appendString(sccpSolanaMainnetGenesisHash, field: "mainnetGenesisHash", to: &out)
    try out.append(bytesFromHex32(
        normalizeNonZeroHex32(sourceVerifierMaterialHash, field: "sourceVerifierMaterialHash"),
        field: "sourceVerifierMaterialHash"
    ))
    try out.append(bytesFromHex32(
        normalizeNonZeroHex32(sourceAdapterDeploymentHash, field: "sourceAdapterDeploymentHash"),
        field: "sourceAdapterDeploymentHash"
    ))
    for (verifierId, verifierHash) in verifierHashes {
        try appendString(verifierId, field: "solanaAuditVerifierId", to: &out)
        out.append(verifierHash)
    }
    return hashHex(prefix: "sccp:solana:full-light-client-gate:v1", payload: out)
}

private func normalizeFullLightClientAuditInput(
    _ input: SolanaSccpFullLightClientAuditProofInput,
    role: SolanaSccpFullLightClientAuditRole
) throws -> NormalizedSolanaSccpFullLightClientAuditInput {
    let witness = try normalizeSolanaSccpWitness(
        witnessInputWithOpenedAccountsLtHash(input.witness, openedAccounts: input.openedAccounts)
    )
    guard witness.sourceDomain == sccpDomainSolana, witness.targetDomain == sccpDomainSora else {
        throw SolanaSccpProverError.invalidString("sourceDomain")
    }
    guard witness.mainnetGenesisHash == sccpSolanaMainnetGenesisHash else {
        throw SolanaSccpProverError.invalidString("mainnetGenesisHash")
    }
    guard witness.sourceStateVerifierHash != sccpZeroHashV1,
          witness.sourceStateVerifierHash != sccpSolanaTemplateSourceStateVerifierHashV1 else {
        throw SolanaSccpProverError.invalidHex32("sourceStateVerifierHash")
    }
    try requireFullLightClientAuditRoleSeparation(input, witness: witness)
    let sourceAdapterDeploymentReceiptHash = try normalizeNonZeroHex32(
        input.sourceAdapterDeploymentReceiptHash,
        field: "sourceAdapterDeploymentReceiptHash"
    )
    guard witness.sourceAdapterDeploymentReceiptHash == sourceAdapterDeploymentReceiptHash else {
        throw SolanaSccpProverError.invalidHex32("sourceAdapterDeploymentReceiptHash")
    }
    let sourceVerifierMaterialHash = try sccpSourceVerifierMaterialHash(
        sourceDomain: sccpDomainSolana,
        sourceTrustAnchorHash: input.sourceTrustAnchorHash,
        consensusVerifierHash: input.consensusVerifierHash,
        messageInclusionVerifierHash: input.messageInclusionVerifierHash,
        finalityPolicyHash: input.finalityPolicyHash,
        sourceStateVerifierHash: witness.sourceStateVerifierHash
    )
    if let supplied = input.sourceVerifierMaterialHash,
       try normalizeHex32(supplied, field: "sourceVerifierMaterialHash") != sourceVerifierMaterialHash {
        throw SolanaSccpProverError.invalidHex32("sourceVerifierMaterialHash")
    }
    let sourceAdapterDeploymentHash = try sccpSourceAdapterEngineDeploymentHash(
        sourceDomain: sccpDomainSolana,
        sourceTrustAnchorHash: input.sourceTrustAnchorHash,
        consensusVerifierHash: input.consensusVerifierHash,
        messageInclusionVerifierHash: input.messageInclusionVerifierHash,
        finalityPolicyHash: input.finalityPolicyHash,
        deploymentReceiptHash: sourceAdapterDeploymentReceiptHash,
        adapterVerifierVkHash: input.adapterVerifierVkHash,
        sourceStateVerifierHash: witness.sourceStateVerifierHash,
        solanaTowerReplayVerifierHash: input.solanaTowerReplayVerifierHash,
        solanaFullAccountsdbLatticeVerifierHash: input.solanaFullAccountsdbLatticeVerifierHash,
        solanaBankForkChoiceVerifierHash: input.solanaBankForkChoiceVerifierHash
    )
    if let supplied = input.sourceAdapterDeploymentHash,
       try normalizeHex32(supplied, field: "sourceAdapterDeploymentHash") != sourceAdapterDeploymentHash {
        throw SolanaSccpProverError.invalidHex32("sourceAdapterDeploymentHash")
    }
    let fullLightClientGateHash = try sccpSolanaFullLightClientGateHash(
        sourceDomain: sccpDomainSolana,
        sourceTrustAnchorHash: input.sourceTrustAnchorHash,
        consensusVerifierHash: input.consensusVerifierHash,
        messageInclusionVerifierHash: input.messageInclusionVerifierHash,
        finalityPolicyHash: input.finalityPolicyHash,
        deploymentReceiptHash: sourceAdapterDeploymentReceiptHash,
        solanaTowerReplayVerifierHash: input.solanaTowerReplayVerifierHash,
        solanaFullAccountsdbLatticeVerifierHash: input.solanaFullAccountsdbLatticeVerifierHash,
        solanaBankForkChoiceVerifierHash: input.solanaBankForkChoiceVerifierHash,
        adapterVerifierVkHash: input.adapterVerifierVkHash,
        sourceStateVerifierHash: witness.sourceStateVerifierHash
    )
    if let supplied = input.fullLightClientGateHash,
       try normalizeHex32(supplied, field: "fullLightClientGateHash") != fullLightClientGateHash {
        throw SolanaSccpProverError.invalidHex32("fullLightClientGateHash")
    }
    guard witness.sourceAdapterDeploymentHash == sourceAdapterDeploymentHash else {
        throw SolanaSccpProverError.sourceAdapterDeploymentBindingMismatch
    }
    let context = try normalizeFullLightClientAuditContext(input, witness: witness)
    let finalityContextHash = hashHex(
        prefix: "sccp:solana:finality-context:v1",
        payload: try canonicalFullLightClientAuditFinalityContextBytes(context)
    )
    if let supplied = input.finalityContextHash,
       try normalizeHex32(supplied, field: "finalityContextHash") != finalityContextHash {
        throw SolanaSccpProverError.invalidHex32("finalityContextHash")
    }
    let voteMessageHash = hashHex(
        prefix: "sccp:solana:finalized-vote:v1",
        payload: try canonicalFullLightClientAuditVoteMessageBytes(
            witness: witness,
            finalityContextHash: finalityContextHash
        )
    )
    if let supplied = input.voteMessageHash,
       try normalizeHex32(supplied, field: "voteMessageHash") != voteMessageHash {
        throw SolanaSccpProverError.invalidHex32("voteMessageHash")
    }
    let accountsLtHashProofHash = try solanaSccpAccountsLtHashProofHash(input.accountsLtHashProof)
    if let supplied = input.accountsLtHashProofHash,
       try normalizeHex32(supplied, field: "accountsLtHashProofHash") != accountsLtHashProofHash {
        throw SolanaSccpProverError.invalidHex32("accountsLtHashProofHash")
    }
    let openedHash = try solanaSccpOpenedAccountsLtHashContributionsHash(input.openedAccounts)
    let residualChecksum = try solanaSccpOpenedAccountsLtHashResidualChecksum(input.openedAccounts)
    if let supplied = input.openedAccountsLtHashContributionsHash,
       try normalizeHex32(supplied, field: "openedAccountsLtHashContributionsHash") != openedHash {
        throw SolanaSccpProverError.invalidHex32("openedAccountsLtHashContributionsHash")
    }
    if let supplied = input.openedAccountsLtHashResidualChecksum,
       try normalizeHex32(supplied, field: "openedAccountsLtHashResidualChecksum") != residualChecksum {
        throw SolanaSccpProverError.invalidHex32("openedAccountsLtHashResidualChecksum")
    }
    return NormalizedSolanaSccpFullLightClientAuditInput(
        role: role,
        witness: witness,
        context: context,
        sourceVerifierMaterialHash: sourceVerifierMaterialHash,
        sourceAdapterDeploymentHash: sourceAdapterDeploymentHash,
        fullLightClientGateHash: fullLightClientGateHash,
        verifierHash: try roleVerifierHash(input, role: role),
        finalityContextHash: finalityContextHash,
        voteMessageHash: voteMessageHash,
        accountsLtHashProofHash: accountsLtHashProofHash,
        openedAccountsLtHashContributionsHash: openedHash,
        openedAccountsLtHashResidualChecksum: residualChecksum
    )
}

private func normalizeFullLightClientAuditContext(
    _ input: SolanaSccpFullLightClientAuditProofInput,
    witness: SolanaSccpWitness
) throws -> NormalizedSolanaSccpFullLightClientAuditContext {
    let epoch = input.epoch ?? solanaSccpMainnetEpoch(forSlot: witness.finalizedSlot)
    guard epoch == solanaSccpMainnetEpoch(forSlot: witness.finalizedSlot) else {
        throw SolanaSccpProverError.invalidString("epoch")
    }
    let bankForkHash = try solanaSccpBankForkHash(
        epoch: epoch,
        finalizedSlot: witness.finalizedSlot,
        parentSlot: witness.parentSlot,
        bankSignatureCount: witness.bankSignatureCount,
        parentBankHash: witness.parentBankHash,
        bankHash: witness.bankHash,
        blockhash: witness.blockhash,
        accountsLtHash: witness.accountsLtHash,
        bankHashHardForkData: witness.bankHashHardForkData,
        transactionStatusRoot: witness.transactionStatusRoot,
        accountInclusionRoot: witness.accountInclusionRoot,
        accountsLtHashChecksum: witness.accountsLtHashChecksum
    )
    let towerLockoutHash = try solanaSccpTowerLockoutHash(
        epoch: epoch,
        finalizedSlot: witness.finalizedSlot,
        rootedSlot: input.rootedSlot,
        parentSlot: witness.parentSlot,
        parentBankHash: witness.parentBankHash
    )
    let towerReplayHash = try solanaSccpTowerReplayHash(
        epoch: epoch,
        finalizedSlot: witness.finalizedSlot,
        rootedSlot: input.rootedSlot,
        parentSlot: witness.parentSlot,
        bankForkHash: bankForkHash,
        towerVoteSlots: input.towerVoteSlots
    )
    if let supplied = input.towerLockoutHash,
       try normalizeHex32(supplied, field: "towerLockoutHash") != towerLockoutHash {
        throw SolanaSccpProverError.invalidHex32("towerLockoutHash")
    }
    if let supplied = input.towerReplayHash,
       try normalizeHex32(supplied, field: "towerReplayHash") != towerReplayHash {
        throw SolanaSccpProverError.invalidHex32("towerReplayHash")
    }
    if let supplied = input.bankForkHash,
       try normalizeHex32(supplied, field: "bankForkHash") != bankForkHash {
        throw SolanaSccpProverError.invalidHex32("bankForkHash")
    }
    return NormalizedSolanaSccpFullLightClientAuditContext(
        version: 1,
        epoch: epoch,
        rootedSlot: input.rootedSlot,
        parentSlot: witness.parentSlot,
        towerVoteSlots: input.towerVoteSlots,
        parentBankHash: witness.parentBankHash,
        bankSignatureCount: witness.bankSignatureCount,
        bankHashHardForkData: witness.bankHashHardForkData,
        epochStakeRoot: try normalizeNonZeroHex32(input.epochStakeRoot, field: "epochStakeRoot"),
        stakeActivationHash: try normalizeNonZeroHex32(input.stakeActivationHash, field: "stakeActivationHash"),
        stakeAccountStateHash: try normalizeNonZeroHex32(
            input.stakeAccountStateHash,
            field: "stakeAccountStateHash"
        ),
        stakeHistoryHash: try normalizeNonZeroHex32(input.stakeHistoryHash, field: "stakeHistoryHash"),
        stakeHistorySysvarAccountHash: try normalizeNonZeroHex32(
            input.stakeHistorySysvarAccountHash,
            field: "stakeHistorySysvarAccountHash"
        ),
        accountInclusionRoot: witness.accountInclusionRoot,
        accountsLtHashChecksum: witness.accountsLtHashChecksum,
        accountsLtHashProofPublicInputsHash: witness.accountsLtHashProofPublicInputsHash,
        towerLockoutHash: towerLockoutHash,
        towerReplayHash: towerReplayHash,
        bankForkHash: bankForkHash
    )
}

private func canonicalFullLightClientAuditFinalityContextBytes(
    _ context: NormalizedSolanaSccpFullLightClientAuditContext
) throws -> Data {
    var out = Data()
    out.append(context.version)
    appendU64Le(context.epoch, to: &out)
    appendU64Le(context.rootedSlot, to: &out)
    appendU64Le(context.parentSlot, to: &out)
    appendU32Le(UInt32(context.towerVoteSlots.count), to: &out)
    context.towerVoteSlots.forEach { appendU64Le($0, to: &out) }
    try out.append(bytesFromHex32(context.parentBankHash, field: "parentBankHash"))
    appendU64Le(context.bankSignatureCount, to: &out)
    appendBytesVec(context.bankHashHardForkData, to: &out)
    try out.append(bytesFromHex32(context.epochStakeRoot, field: "epochStakeRoot"))
    try out.append(bytesFromHex32(context.stakeActivationHash, field: "stakeActivationHash"))
    try out.append(bytesFromHex32(context.stakeAccountStateHash, field: "stakeAccountStateHash"))
    try out.append(bytesFromHex32(context.stakeHistoryHash, field: "stakeHistoryHash"))
    try out.append(bytesFromHex32(context.stakeHistorySysvarAccountHash, field: "stakeHistorySysvarAccountHash"))
    try out.append(bytesFromHex32(context.accountInclusionRoot, field: "accountInclusionRoot"))
    try out.append(bytesFromHex32(context.accountsLtHashChecksum, field: "accountsLtHashChecksum"))
    try out.append(
        bytesFromHex32(
            context.accountsLtHashProofPublicInputsHash,
            field: "accountsLtHashProofPublicInputsHash"
        )
    )
    try out.append(bytesFromHex32(context.towerLockoutHash, field: "towerLockoutHash"))
    try out.append(bytesFromHex32(context.towerReplayHash, field: "towerReplayHash"))
    try out.append(bytesFromHex32(context.bankForkHash, field: "bankForkHash"))
    return out
}

private func canonicalFullLightClientAuditVoteMessageBytes(
    witness: SolanaSccpWitness,
    finalityContextHash: String
) throws -> Data {
    var out = Data()
    out.append(1)
    appendU32Le(sccpDomainSolana, to: &out)
    appendU64Le(witness.finalizedSlot, to: &out)
    try out.append(bytesFromHex32(witness.blockhash, field: "blockhash"))
    try out.append(bytesFromHex32(witness.bankHash, field: "bankHash"))
    try out.append(bytesFromHex32(witness.transactionStatusRoot, field: "transactionStatusRoot"))
    try out.append(bytesFromHex32(witness.messageProofHash, field: "messageProofHash"))
    try out.append(bytesFromHex32(finalityContextHash, field: "finalityContextHash"))
    return out
}

private func auditRoleColumns(
    _ value: NormalizedSolanaSccpFullLightClientAuditInput,
    role: SolanaSccpFullLightClientAuditRole
) -> [String] {
    switch role {
    case .towerReplay:
        return [
            value.context.towerLockoutHash,
            value.context.towerReplayHash,
            value.context.bankForkHash,
            value.context.epochStakeRoot,
            value.context.stakeActivationHash,
            value.context.stakeAccountStateHash,
            value.context.stakeHistoryHash,
            value.context.stakeHistorySysvarAccountHash,
            value.context.accountInclusionRoot,
        ]
    case .fullAccountsdbLattice:
        return [
            value.context.accountInclusionRoot,
            value.context.accountsLtHashChecksum,
            value.context.accountsLtHashProofPublicInputsHash,
            value.openedAccountsLtHashContributionsHash,
            value.openedAccountsLtHashResidualChecksum,
            value.accountsLtHashProofHash,
        ]
    case .bankForkChoice:
        return [
            value.context.parentBankHash,
            value.witness.bankHash,
            value.witness.blockhash,
            value.witness.transactionStatusRoot,
            value.context.accountInclusionRoot,
            value.context.accountsLtHashChecksum,
            "0x" + sccpWordU64Le(value.context.bankSignatureCount).hexEncodedString(),
            hashHex(
                prefix: sccpSolanaBankHashHardForkDataPrefixV1,
                payload: value.context.bankHashHardForkData
            ),
            value.context.bankForkHash,
            value.context.towerReplayHash,
        ]
    }
}

private func fullLightClientAuditFastpqPublicInputs(
    _ value: NormalizedSolanaSccpFullLightClientAuditInput,
    statementHash: String
) throws -> SolanaSccpFullLightClientAuditFastpqPublicInputs {
    var dsidPreimage = Data([auditRoleProfile(value.role).code])
    try dsidPreimage.append(bytesFromHex32(statementHash, field: "auditStatementHash"))
    let dsidHash = hashBytes(
        prefix: sccpSolanaFullLightClientAuditFastpqDsidPrefixV1,
        payload: dsidPreimage
    )
    let roots: (String, String, String)
    switch value.role {
    case .towerReplay:
        roots = (value.context.towerLockoutHash, value.context.towerReplayHash, value.context.bankForkHash)
    case .fullAccountsdbLattice:
        roots = (
            value.context.accountInclusionRoot,
            value.context.accountsLtHashChecksum,
            value.openedAccountsLtHashContributionsHash
        )
    case .bankForkChoice:
        roots = (value.context.parentBankHash, value.witness.bankHash, value.context.bankForkHash)
    }
    return SolanaSccpFullLightClientAuditFastpqPublicInputs(
        dsid: "0x" + dsidHash.prefix(16).hexEncodedString(),
        slot: String(value.witness.finalizedSlot),
        oldRoot: roots.0,
        newRoot: roots.1,
        permRoot: roots.2,
        txSetHash: statementHash
    )
}

private func canonicalFullLightClientAuditContextBytes(
    _ value: NormalizedSolanaSccpFullLightClientAuditInput,
    statementHash: String
) throws -> Data {
    let profile = auditRoleProfile(value.role)
    var out = Data()
    out.append(1)
    out.append(profile.code)
    try appendString(profile.circuitId, field: "circuitId", to: &out)
    try appendString(sccpSolanaFullLightClientAuditFastpqParameterSetV1, field: "parameterSet", to: &out)
    try appendString(profile.verifierId, field: "verifierId", to: &out)
    try out.append(bytesFromHex32(value.verifierHash, field: "verifierHash"))
    try out.append(bytesFromHex32(value.sourceVerifierMaterialHash, field: "sourceVerifierMaterialHash"))
    try out.append(bytesFromHex32(value.sourceAdapterDeploymentHash, field: "sourceAdapterDeploymentHash"))
    try out.append(bytesFromHex32(value.fullLightClientGateHash, field: "fullLightClientGateHash"))
    try out.append(bytesFromHex32(value.finalityContextHash, field: "finalityContextHash"))
    try out.append(bytesFromHex32(statementHash, field: "auditStatementHash"))
    return out
}

private func fullLightClientAuditFastpqKey(
    _ prefix: String,
    profile: SolanaSccpFullLightClientAuditRoleProfile
) -> Data {
    var out = Data(prefix.utf8)
    out.append(0)
    out.append(Data(profile.circuitId.utf8))
    return out
}

private func sccpWordU8(_ value: UInt8) -> Data {
    var out = Data(repeating: 0, count: 32)
    out[0] = value
    return out
}

private func bytesFromHex32(_ value: String, field: String) throws -> Data {
    try bytesFromHex(value, field: field, byteCount: 32)
}

private func solanaIsLowercaseHexBody(_ value: String) -> Bool {
    value.utf8.allSatisfy { byte in
        (byte >= 0x30 && byte <= 0x39) || (byte >= 0x61 && byte <= 0x66)
    }
}

private func bytesFromHex(_ value: String, field: String, byteCount: Int) throws -> Data {
    guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
        throw SolanaSccpProverError.invalidHex32(field)
    }
    let hex = value.hasPrefix("0x") ? String(value.dropFirst(2)) : value
    guard hex.unicodeScalars.allSatisfy({ !CharacterSet.whitespacesAndNewlines.contains($0) }) else {
        throw SolanaSccpProverError.invalidHex32(field)
    }
    guard hex.count == byteCount * 2,
          solanaIsLowercaseHexBody(hex),
          let bytes = Data(hexString: hex),
          bytes.count == byteCount else {
        throw SolanaSccpProverError.invalidHex32(field)
    }
    return bytes
}

private func appendString(_ value: String, field: String, to out: inout Data) throws {
    let bytes = Data(try normalizeNonEmpty(value, field: field).utf8)
    appendU32Le(UInt32(bytes.count), to: &out)
    out.append(bytes)
}

private func appendBytesVec(_ value: Data, to out: inout Data) {
    appendU32Le(UInt32(value.count), to: &out)
    out.append(value)
}

private func appendU32Le(_ value: UInt32, to out: inout Data) {
    out.append(UInt8(value & 0xff))
    out.append(UInt8((value >> 8) & 0xff))
    out.append(UInt8((value >> 16) & 0xff))
    out.append(UInt8((value >> 24) & 0xff))
}

private func appendU16Le(_ value: UInt16, to out: inout Data) {
    out.append(UInt8(value & 0xff))
    out.append(UInt8((value >> 8) & 0xff))
}

private func appendU64Le(_ value: UInt64, to out: inout Data) {
    for shift in stride(from: 0, through: 56, by: 8) {
        out.append(UInt8((value >> UInt64(shift)) & 0xff))
    }
}

private func sccpWordU32Le(_ value: UInt32) -> Data {
    var out = Data()
    appendU32Le(value, to: &out)
    out.append(Data(repeating: 0, count: 28))
    return out
}

private func sccpWordU64Le(_ value: UInt64) -> Data {
    var out = Data()
    appendU64Le(value, to: &out)
    out.append(Data(repeating: 0, count: 24))
    return out
}

private func readU32Le(_ data: Data, offset: Int, field: String) throws -> UInt32 {
    guard offset >= 0, offset + 4 <= data.count else {
        throw SolanaSccpProverError.invalidString(field)
    }
    var value: UInt32 = 0
    for index in 0..<4 {
        value |= UInt32(data[offset + index]) << UInt32(index * 8)
    }
    return value
}

private func readU16Le(_ data: Data, offset: Int, field: String) throws -> UInt16 {
    guard offset >= 0, offset + 2 <= data.count else {
        throw SolanaSccpProverError.invalidString(field)
    }
    var value: UInt16 = 0
    for index in 0..<2 {
        value |= UInt16(data[offset + index]) << UInt16(index * 8)
    }
    return value
}

private func readU64Le(_ data: Data, offset: Int, field: String) throws -> UInt64 {
    guard offset >= 0, offset + 8 <= data.count else {
        throw SolanaSccpProverError.invalidString(field)
    }
    var value: UInt64 = 0
    for index in 0..<8 {
        value |= UInt64(data[offset + index]) << UInt64(index * 8)
    }
    return value
}

private struct SolanaOpenedLtHashContributionRow {
    let role: UInt8
    let address: Data
    let accountHash: Data
    let rawDataHash: Data
    let accountLtHash: Data
}

private struct NormalizedSolanaOpenedLtHashContributions {
    let sourceDomain: UInt32
    let finalizedSlot: UInt64
    let accountInclusionRoot: Data
    let accountsLtHashChecksum: Data
    let rows: [SolanaOpenedLtHashContributionRow]
    let openedAccountsLtHash: Data
    let openedAccountsLtHashChecksum: Data
    let residualAccountsLtHash: Data
    let residualAccountsLtHashChecksum: Data
}

private struct NormalizedSolanaAccountsLtHashProofRequest {
    let witness: SolanaSccpWitness
    let opened: NormalizedSolanaOpenedLtHashContributions
    let accountsLtHash: Data
    let openedContributionsHash: String
    let residualChecksum: String
}

private func witnessInputWithOpenedAccountsLtHash(
    _ witness: SolanaSccpWitnessInput,
    openedAccounts: SolanaSccpOpenedAccountsLtHashContributionsInput
) throws -> SolanaSccpWitnessInput {
    if let supplied = witness.accountsLtHash, supplied != openedAccounts.accountsLtHash {
        throw SolanaSccpProverError.invalidString("accountsLtHash")
    }
    return SolanaSccpWitnessInput(
        targetDomain: witness.targetDomain,
        mainnetGenesisHash: witness.mainnetGenesisHash,
        finalizedSlot: witness.finalizedSlot,
        parentSlot: witness.parentSlot,
        bankSignatureCount: witness.bankSignatureCount,
        parentBankHash: witness.parentBankHash,
        blockhash: witness.blockhash,
        bankHash: witness.bankHash,
        transactionStatusRoot: witness.transactionStatusRoot,
        messageProofHash: witness.messageProofHash,
        accountInclusionRoot: witness.accountInclusionRoot,
        accountsLtHashChecksum: witness.accountsLtHashChecksum,
        accountsLtHashProofPublicInputsHash: witness.accountsLtHashProofPublicInputsHash,
        bankHashHardForkData: witness.bankHashHardForkData,
        accountsLtHash: openedAccounts.accountsLtHash,
        transactionSignature: witness.transactionSignature,
        emitterProgramId: witness.emitterProgramId,
        messageId: witness.messageId,
        payloadHash: witness.payloadHash,
        commitmentRoot: witness.commitmentRoot,
        sourceEventDigest: witness.sourceEventDigest,
        sourceStateVerifierId: witness.sourceStateVerifierId,
        sourceStateVerifierHash: witness.sourceStateVerifierHash,
        statementHash: witness.statementHash,
        destinationBindingHash: witness.destinationBindingHash,
        inclusionBranch: witness.inclusionBranch,
        sourceAdapterDeploymentHash: witness.sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash: witness.sourceAdapterDeploymentReceiptHash
    )
}

private func normalizeAccountsLtHashProofRequest(
    witness: SolanaSccpWitnessInput,
    openedAccounts: SolanaSccpOpenedAccountsLtHashContributionsInput
) throws -> NormalizedSolanaAccountsLtHashProofRequest {
    guard witness.sourceStateVerifierId == sccpSolanaMainnetAccountsDbVerifierIdV1 else {
        throw SolanaSccpProverError.invalidString("sourceStateVerifierId")
    }
    let sourceStateVerifierHash = try normalizeHex32(witness.sourceStateVerifierHash, field: "sourceStateVerifierHash")
    guard sourceStateVerifierHash != sccpZeroHashV1 else {
        throw SolanaSccpProverError.invalidHex32("sourceStateVerifierHash")
    }
    guard sourceStateVerifierHash != sccpSolanaTemplateSourceStateVerifierHashV1 else {
        throw SolanaSccpProverError.invalidHex32("sourceStateVerifierHash")
    }
    let witnessWithAccountsLtHash = try witnessInputWithOpenedAccountsLtHash(
        witness,
        openedAccounts: openedAccounts
    )
    let canonicalWitness = try normalizeSolanaSccpWitness(witnessWithAccountsLtHash)
    let opened = try normalizeOpenedAccountsLtHashContributions(openedAccounts)
    guard canonicalWitness.finalizedSlot == opened.finalizedSlot else {
        throw SolanaSccpProverError.invalidString("finalizedSlot")
    }
    guard try bytesFromHex32(canonicalWitness.accountInclusionRoot, field: "accountInclusionRoot") == opened.accountInclusionRoot else {
        throw SolanaSccpProverError.invalidString("accountInclusionRoot")
    }
    guard try bytesFromHex32(canonicalWitness.accountsLtHashChecksum, field: "accountsLtHashChecksum") == opened.accountsLtHashChecksum else {
        throw SolanaSccpProverError.invalidString("accountsLtHashChecksum")
    }
    return NormalizedSolanaAccountsLtHashProofRequest(
        witness: canonicalWitness,
        opened: opened,
        accountsLtHash: openedAccounts.accountsLtHash,
        openedContributionsHash: try solanaSccpOpenedAccountsLtHashContributionsHash(openedAccounts),
        residualChecksum: try solanaSccpOpenedAccountsLtHashResidualChecksum(openedAccounts)
    )
}

private func normalizeOpenedAccountsLtHashContributions(
    _ input: SolanaSccpOpenedAccountsLtHashContributionsInput
) throws -> NormalizedSolanaOpenedLtHashContributions {
    guard input.sourceDomain == sccpDomainSolana else {
        throw SolanaSccpProverError.invalidString("sourceDomain")
    }
    let accountInclusionRoot = try bytesFromHex32(input.accountInclusionRoot, field: "accountInclusionRoot")
    guard accountInclusionRoot.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("accountInclusionRoot")
    }
    let accountsLtHashChecksum = try bytesFromHex32(input.accountsLtHashChecksum, field: "accountsLtHashChecksum")
    guard accountsLtHashChecksum.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidHex32("accountsLtHashChecksum")
    }
    guard input.accountsLtHash.count == sccpSolanaAccountsLtHashBytes else {
        throw SolanaSccpProverError.invalidString("accountsLtHash")
    }
    guard input.accountsLtHash.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidString("accountsLtHash")
    }
    guard try blake3Hash32(input.accountsLtHash, field: "accountsLtHash") == accountsLtHashChecksum else {
        throw SolanaSccpProverError.invalidString("accountsLtHashChecksum")
    }
    let rows = try openedAccountsLtHashContributionRows(input)
    var openedAccountsLtHash = Data(repeating: 0, count: sccpSolanaAccountsLtHashBytes)
    for row in rows {
        try addAccountsLtHashContribution(&openedAccountsLtHash, row.accountLtHash)
    }
    let openedAccountsLtHashChecksum = try blake3Hash32(openedAccountsLtHash, field: "openedAccountsLtHash")
    var residualAccountsLtHash = input.accountsLtHash
    try subtractAccountsLtHashContribution(&residualAccountsLtHash, openedAccountsLtHash)
    guard residualAccountsLtHash.contains(where: { $0 != 0 }) else {
        throw SolanaSccpProverError.invalidString("openedAccountsLtHashResidual")
    }
    let residualAccountsLtHashChecksum = try blake3Hash32(residualAccountsLtHash, field: "residualAccountsLtHash")
    return NormalizedSolanaOpenedLtHashContributions(
        sourceDomain: input.sourceDomain,
        finalizedSlot: input.finalizedSlot,
        accountInclusionRoot: accountInclusionRoot,
        accountsLtHashChecksum: accountsLtHashChecksum,
        rows: rows,
        openedAccountsLtHash: openedAccountsLtHash,
        openedAccountsLtHashChecksum: openedAccountsLtHashChecksum,
        residualAccountsLtHash: residualAccountsLtHash,
        residualAccountsLtHashChecksum: residualAccountsLtHashChecksum
    )
}

private func openedAccountsLtHashContributionRows(
    _ input: SolanaSccpOpenedAccountsLtHashContributionsInput
) throws -> [SolanaOpenedLtHashContributionRow] {
    let deriveVoteLtHashes = input.validatorVoteAccountLtHashes.isEmpty
    let deriveStakeLtHashes = input.validatorStakeAccountLtHashes.isEmpty
    guard input.validatorVoteAccountOpenings.count <= sccpSolanaMaxValidators else {
        throw SolanaSccpProverError.invalidString("validatorVoteAccountOpenings")
    }
    guard input.validatorStakeAccountOpenings.count <= sccpSolanaMaxValidators else {
        throw SolanaSccpProverError.invalidString("validatorStakeAccountOpenings")
    }
    guard input.validatorVoteAccountOpenings.count == input.validatorVoteAccountRawData.count,
          (deriveVoteLtHashes || input.validatorVoteAccountOpenings.count == input.validatorVoteAccountLtHashes.count) else {
        throw SolanaSccpProverError.invalidString("validatorVoteAccountOpenings")
    }
    guard input.validatorStakeAccountOpenings.count == input.validatorStakeAccountRawData.count,
          (deriveStakeLtHashes || input.validatorStakeAccountOpenings.count == input.validatorStakeAccountLtHashes.count) else {
        throw SolanaSccpProverError.invalidString("validatorStakeAccountOpenings")
    }
    var rows: [SolanaOpenedLtHashContributionRow] = []
    var seenAddresses: Set<Data> = []

    func pushRow(
        role: UInt8,
        opening: SolanaSccpAccountOpeningInput,
        rawData: Data,
        suppliedAccountLtHash: Data?,
        field: String,
        allowEmptyDerive: Bool = false
    ) throws {
        guard opening.address.count == 32 else {
            throw SolanaSccpProverError.invalidString("address")
        }
        guard seenAddresses.insert(opening.address).inserted else {
            throw SolanaSccpProverError.invalidString("openedAccountAddresses")
        }
        let expectedAccountLtHash = try solanaSccpAccountLtHash(
            opening: opening,
            rawData: rawData
        )
        let accountLtHash: Data
        if let suppliedAccountLtHash, !(allowEmptyDerive && suppliedAccountLtHash.isEmpty) {
            guard suppliedAccountLtHash.count == sccpSolanaAccountsLtHashBytes else {
                throw SolanaSccpProverError.invalidString(field)
            }
            guard suppliedAccountLtHash == expectedAccountLtHash else {
                throw SolanaSccpProverError.invalidString(field)
            }
            accountLtHash = suppliedAccountLtHash
        } else {
            accountLtHash = expectedAccountLtHash
        }
        let accountHash = try bytesFromHex32(
            solanaSccpAccountOpeningHash(
                address: opening.address,
                owner: opening.owner,
                lamports: opening.lamports,
                rentEpoch: opening.rentEpoch,
                executable: opening.executable,
                dataHash: try bytesFromHex32(opening.dataHash, field: "dataHash")
            ),
            field: "accountHash"
        )
        let rawDataHash = try bytesFromHex32(solanaSccpAccountRawDataHash(rawData), field: "rawDataHash")
        rows.append(
            SolanaOpenedLtHashContributionRow(
                role: role,
                address: opening.address,
                accountHash: accountHash,
                rawDataHash: rawDataHash,
                accountLtHash: accountLtHash
            )
        )
    }

    for index in input.validatorVoteAccountOpenings.indices {
        try pushRow(
            role: sccpSolanaOpenedLtHashRoleVote,
            opening: input.validatorVoteAccountOpenings[index],
            rawData: input.validatorVoteAccountRawData[index],
            suppliedAccountLtHash: deriveVoteLtHashes ? nil : input.validatorVoteAccountLtHashes[index],
            field: "validatorVoteAccountLtHashes[\(index)]"
        )
    }
    for index in input.validatorStakeAccountOpenings.indices {
        try pushRow(
            role: sccpSolanaOpenedLtHashRoleStake,
            opening: input.validatorStakeAccountOpenings[index],
            rawData: input.validatorStakeAccountRawData[index],
            suppliedAccountLtHash: deriveStakeLtHashes ? nil : input.validatorStakeAccountLtHashes[index],
            field: "validatorStakeAccountLtHashes[\(index)]"
        )
    }
    try pushRow(
        role: sccpSolanaOpenedLtHashRoleStakeHistorySysvar,
        opening: input.stakeHistorySysvarOpening,
        rawData: input.stakeHistorySysvarRawData,
        suppliedAccountLtHash: input.stakeHistorySysvarAccountLtHash,
        field: "stakeHistorySysvarAccountLtHash",
        allowEmptyDerive: true
    )
    rows.sort {
        if $0.role != $1.role {
            return $0.role < $1.role
        }
        return $0.address.lexicographicallyPrecedes($1.address)
    }
    return rows
}

private func addAccountsLtHashContribution(_ target: inout Data, _ contribution: Data) throws {
    guard target.count == sccpSolanaAccountsLtHashBytes,
          contribution.count == sccpSolanaAccountsLtHashBytes else {
        throw SolanaSccpProverError.invalidString("accountLtHash")
    }
    for index in 0..<1_024 {
        let offset = index * 2
        let current = UInt16(target[offset]) | (UInt16(target[offset + 1]) << 8)
        let addend = UInt16(contribution[offset]) | (UInt16(contribution[offset + 1]) << 8)
        let mixed = current &+ addend
        target[offset] = UInt8(mixed & 0xff)
        target[offset + 1] = UInt8(mixed >> 8)
    }
}

private func subtractAccountsLtHashContribution(_ target: inout Data, _ contribution: Data) throws {
    guard target.count == sccpSolanaAccountsLtHashBytes,
          contribution.count == sccpSolanaAccountsLtHashBytes else {
        throw SolanaSccpProverError.invalidString("accountLtHash")
    }
    for index in 0..<1_024 {
        let offset = index * 2
        let current = UInt16(target[offset]) | (UInt16(target[offset + 1]) << 8)
        let subtrahend = UInt16(contribution[offset]) | (UInt16(contribution[offset + 1]) << 8)
        let mixed = current &- subtrahend
        target[offset] = UInt8(mixed & 0xff)
        target[offset + 1] = UInt8(mixed >> 8)
    }
}

private func blake3Hash32(_ data: Data, field: String) throws -> Data {
    if let digest = NoritoNativeBridge.shared.blake3Hash(data: data), digest.count == 32 {
        return digest
    }
    guard let fallback = SolanaSccpBlake3.hash(data), fallback.count == 32 else {
        throw SolanaSccpProverError.invalidString(field)
    }
    return fallback
}

private func hashBytes(prefix: String, payload: Data) -> Data {
    var preimage = Data(prefix.utf8)
    preimage.append(payload)
    return Blake2b.hash256(preimage)
}

private func hashHex(prefix: String, payload: Data) -> String {
    "0x" + hashBytes(prefix: prefix, payload: payload).hexEncodedString()
}

private func solanaSccpMainnetGenesisHashPublicInput() -> String {
    hashHex(
        prefix: sccpSolanaMainnetGenesisHashPrefixV1,
        payload: Data(sccpSolanaMainnetGenesisHash.utf8)
    )
}

private func sourceMerkleNodeHash(left: Data, right: Data) -> Data {
    var payload = Data()
    payload.append(left)
    payload.append(right)
    return hashBytes(prefix: "sccp:source:node:v1", payload: payload)
}

private enum SolanaSccpBlake3 {
    private static let blockLen = 64
    private static let chunkLen = 1_024
    private static let outLen = 32
    private static let maxInputLen = 8 + sccpSolanaMaxAccountRawDataBytes + 1 + 32 + 32
    private static let chunkStart: UInt32 = 1
    private static let chunkEnd: UInt32 = 2
    private static let parent: UInt32 = 4
    private static let root: UInt32 = 8
    private static let iv: [UInt32] = [
        0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
        0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
    ]
    private static let permutation = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]

    static func hash(_ data: Data) -> Data? {
        derive(data, outputLength: outLen)
    }

    static func derive(_ data: Data, outputLength: Int) -> Data? {
        guard data.count <= maxInputLen else {
            return nil
        }
        guard outputLength >= 0 else {
            return nil
        }
        let bytes = [UInt8](data)
        let rootOutput = rootOutput(bytes)
        var output = Data(capacity: outputLength)
        var outputBlockCounter: UInt64 = 0
        while output.count < outputLength {
            let words = rootOutput.rootWords(outputBlockCounter: outputBlockCounter)
            for word in words {
                var littleEndian = word.littleEndian
                withUnsafeBytes(of: &littleEndian) { raw in
                    let remaining = outputLength - output.count
                    output.append(contentsOf: raw.prefix(remaining))
                }
                if output.count == outputLength {
                    break
                }
            }
            outputBlockCounter += 1
        }
        return output
    }

    private static func rootOutput(_ bytes: [UInt8]) -> Output {
        let chunkCount = max(1, (bytes.count + chunkLen - 1) / chunkLen)
        return subtreeOutput(bytes, chunkIndex: 0, chunkCount: chunkCount)
    }

    private static func subtreeOutput(_ bytes: [UInt8], chunkIndex: Int, chunkCount: Int) -> Output {
        if chunkCount == 1 {
            let offset = chunkIndex * chunkLen
            let length = min(chunkLen, max(0, bytes.count - offset))
            return chunkOutput(bytes, offset: offset, length: length, chunkCounter: UInt64(chunkIndex))
        }
        let leftCount = leftSubtreeChunkCount(chunkCount)
        let left = subtreeOutput(bytes, chunkIndex: chunkIndex, chunkCount: leftCount).chainingValue()
        let right = subtreeOutput(
            bytes,
            chunkIndex: chunkIndex + leftCount,
            chunkCount: chunkCount - leftCount
        ).chainingValue()
        return parentOutput(left: left, right: right)
    }

    private static func leftSubtreeChunkCount(_ chunkCount: Int) -> Int {
        var power = 1
        while power * 2 < chunkCount {
            power *= 2
        }
        return power
    }

    private static func chunkOutput(
        _ bytes: [UInt8],
        offset: Int,
        length: Int,
        chunkCounter: UInt64
    ) -> Output {
        var cv = iv
        let numBlocks = max(1, (length + blockLen - 1) / blockLen)
        for blockIndex in 0..<numBlocks {
            let blockStart = offset + blockIndex * blockLen
            let blockLength = min(blockLen, max(0, length - blockIndex * blockLen))
            let blockWords = parseBlockWords(bytes, blockStart: blockStart)
            var flags: UInt32 = 0
            if blockIndex == 0 {
                flags |= chunkStart
            }
            if blockIndex == numBlocks - 1 {
                return Output(
                    inputChainingValue: cv,
                    blockWords: blockWords,
                    counter: chunkCounter,
                    blockLen: UInt32(blockLength),
                    flags: flags | chunkEnd
                )
            }
            let state = compress(
                cv: cv,
                blockWords: blockWords,
                counter: chunkCounter,
                blockLen: UInt32(blockLength),
                flags: flags
            )
            for index in 0..<8 {
                cv[index] = state[index] ^ state[index + 8]
            }
        }
        preconditionFailure("unreachable")
    }

    private static func parentOutput(left: [UInt32], right: [UInt32]) -> Output {
        var blockWords = [UInt32](repeating: 0, count: 16)
        for index in 0..<8 {
            blockWords[index] = left[index]
            blockWords[index + 8] = right[index]
        }
        return Output(
            inputChainingValue: iv,
            blockWords: blockWords,
            counter: 0,
            blockLen: UInt32(blockLen),
            flags: parent
        )
    }

    private static func parseBlockWords(_ bytes: [UInt8], blockStart: Int) -> [UInt32] {
        var words = [UInt32](repeating: 0, count: 16)
        for index in 0..<16 {
            let offset = blockStart + index * 4
            let b0 = offset < bytes.count ? UInt32(bytes[offset]) : 0
            let b1 = offset + 1 < bytes.count ? UInt32(bytes[offset + 1]) : 0
            let b2 = offset + 2 < bytes.count ? UInt32(bytes[offset + 2]) : 0
            let b3 = offset + 3 < bytes.count ? UInt32(bytes[offset + 3]) : 0
            words[index] = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
        }
        return words
    }

    private static func compress(
        cv: [UInt32],
        blockWords: [UInt32],
        counter: UInt64,
        blockLen: UInt32,
        flags: UInt32
    ) -> [UInt32] {
        var state = [
            cv[0], cv[1], cv[2], cv[3],
            cv[4], cv[5], cv[6], cv[7],
            iv[0], iv[1], iv[2], iv[3],
            UInt32(truncatingIfNeeded: counter),
            UInt32(truncatingIfNeeded: counter >> 32),
            blockLen,
            flags,
        ]
        var message = blockWords
        for round in 0..<7 {
            mix(&state, 0, 4, 8, 12, message[0], message[1])
            mix(&state, 1, 5, 9, 13, message[2], message[3])
            mix(&state, 2, 6, 10, 14, message[4], message[5])
            mix(&state, 3, 7, 11, 15, message[6], message[7])
            mix(&state, 0, 5, 10, 15, message[8], message[9])
            mix(&state, 1, 6, 11, 12, message[10], message[11])
            mix(&state, 2, 7, 8, 13, message[12], message[13])
            mix(&state, 3, 4, 9, 14, message[14], message[15])
            if round < 6 {
                message = permutation.map { message[$0] }
            }
        }
        return state
    }

    private static func mix(
        _ state: inout [UInt32],
        _ a: Int,
        _ b: Int,
        _ c: Int,
        _ d: Int,
        _ x: UInt32,
        _ y: UInt32
    ) {
        state[a] = state[a] &+ state[b] &+ x
        state[d] = rotateRight(state[d] ^ state[a], by: 16)
        state[c] = state[c] &+ state[d]
        state[b] = rotateRight(state[b] ^ state[c], by: 12)
        state[a] = state[a] &+ state[b] &+ y
        state[d] = rotateRight(state[d] ^ state[a], by: 8)
        state[c] = state[c] &+ state[d]
        state[b] = rotateRight(state[b] ^ state[c], by: 7)
    }

    private static func rotateRight(_ value: UInt32, by amount: UInt32) -> UInt32 {
        (value >> amount) | (value << (32 - amount))
    }

    private struct Output {
        let inputChainingValue: [UInt32]
        let blockWords: [UInt32]
        let counter: UInt64
        let blockLen: UInt32
        let flags: UInt32

        func chainingValue() -> [UInt32] {
            let state = SolanaSccpBlake3.compress(
                cv: inputChainingValue,
                blockWords: blockWords,
                counter: counter,
                blockLen: blockLen,
                flags: flags
            )
            return (0..<8).map { state[$0] ^ state[$0 + 8] }
        }

        func rootWords(outputBlockCounter: UInt64) -> [UInt32] {
            let state = SolanaSccpBlake3.compress(
                cv: inputChainingValue,
                blockWords: blockWords,
                counter: outputBlockCounter,
                blockLen: blockLen,
                flags: flags | SolanaSccpBlake3.root
            )
            var words = [UInt32](repeating: 0, count: 16)
            for index in 0..<8 {
                words[index] = state[index] ^ state[index + 8]
                words[index + 8] = state[index + 8] ^ inputChainingValue[index]
            }
            return words
        }
    }
}
