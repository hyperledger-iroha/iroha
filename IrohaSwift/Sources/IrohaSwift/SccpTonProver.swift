import Foundation
import CryptoKit

/// SCCP domain id for TON.
public let sccpDomainTon: UInt32 = 4

/// Proof backend id expected by the TON SCCP verifier contract.
public let sccpTonContractProofBackendV1 = "ton-contract-v1"

/// Recursive proof family accepted by the TON SCCP verifier contract.
public let sccpTonStarkFriProofFamilyV1 = "stark-fri-v1"

/// Source-state verifier profile expected for deployed TON shard-state proofs.
public let sccpTonMainnetShardStateVerifierIdV1 = "sccp:ton:source-state-verifier:shard-state-light-client-mainnet:v1"

private let sccpTonTemplateSourceStateVerifierHashV1 = "540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f"
private let sccpTonTemplateSourceMaterialHashesV1 = [
    "d83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c",
    "b0225e16477ea3420f7d0de76b87b6e99a43ab97f445d8565a384d4b655bc473",
    "89254256421c15da8c92842c7d6f448ef6c1d5ca1e2a173754643425fcee6353",
    "50044ee6db0eb0cdef097e69406b6c30d3406d8f784e8ba34e9b923b38bd0c43",
    sccpTonTemplateSourceStateVerifierHashV1,
]

/// OpenVerify circuit id used by TON shard-state source-state proof requests.
public let sccpTonShardStateOpenVerifyCircuitIdV1 = "sccp-ton-shard-state-light-client-v1"

/// OpenVerify circuit id for the TON masterchain config audit role proof.
public let sccpTonMasterchainConfigOpenVerifyCircuitIdV1 = "sccp-ton-masterchain-config-v1"

/// OpenVerify circuit id for the TON validator-set transition audit role proof.
public let sccpTonValidatorSetTransitionOpenVerifyCircuitIdV1 =
    "sccp-ton-validator-set-transition-v1"

/// OpenVerify circuit id for the TON shard-accounts dictionary audit role proof.
public let sccpTonShardAccountsDictionaryOpenVerifyCircuitIdV1 =
    "sccp-ton-shard-accounts-dictionary-v1"

private let tonSccpSourceStateVerificationCircuitIds: Set<String> = [
    sccpTonShardStateOpenVerifyCircuitIdV1,
    sccpTonMasterchainConfigOpenVerifyCircuitIdV1,
    sccpTonValidatorSetTransitionOpenVerifyCircuitIdV1,
    sccpTonShardAccountsDictionaryOpenVerifyCircuitIdV1,
]

/// TON internal-message body envelope encoding for SCCP submissions.
public let sccpTonMessageBodyBocV1 = "ton_message_body_boc_v1"

/// TON masterchain config parameter carrying the current active validator set.
public let sccpTonCurrentValidatorSetConfigParam: UInt64 = 34

/// TON config dictionary key width for masterchain config parameters.
public let sccpTonConfigParamKeyBits = 32

private let tonMaxSourceMerkleBranchNodes = 64
private let tonMasterchainWorkchainId: Int32 = -1
private let tonMasterchainShard: UInt64 = 0x8000_0000_0000_0000
private let tonBasechainWorkchainId: Int32 = 0
private let tonStarkFriProofFamilyV1 = sccpTonStarkFriProofFamilyV1
private let tonValidatorSetTransitionChainPrefixV1 = "sccp:ton:validator-set-transition-chain:v1"
private let tonShardStateProofPublicInputsPrefixV1 = "sccp:ton:shard-state-proof-public-inputs:v1"
private let tonShardStateFastpqDsidPrefixV1 = "sccp:ton:shard-state:fastpq:dsid:v1"
private let tonShardStateFastpqParameterSetV1 = "fastpq-lane-balanced"
private let tonShardStateFastpqStatementKeyV1 = "sccp:ton:shard-state:v1:statement"
private let tonShardStateFastpqWitnessKeyV1 = "sccp:ton:shard-state:v1:witness"
private let tonShardStateFastpqContextKeyV1 = "sccp:ton:shard-state:v1:context"
private let tonShardStateProofBocPrefixV1 = "sccp:ton:shard-state-proof-boc:v1"
private let tonShardAccountsProofBocPrefixV1 = "sccp:ton:shard-accounts-proof-boc:v1"
private let tonConfigProofBocPrefixV1 = "sccp:ton:config-proof-boc:v1"
private let tonFullLightClientAuditFastpqDsidPrefixV1 =
    "sccp:ton:full-light-client-audit:fastpq:dsid:v1"
private let tonFullLightClientAuditFastpqParameterSetV1 = "fastpq-lane-balanced"
private let tonFullLightClientAuditFastpqStatementKeyV1 =
    "sccp:ton:full-light-client-audit:v1:statement"
private let tonFullLightClientAuditFastpqContextKeyV1 =
    "sccp:ton:full-light-client-audit:v1:context"
private let tonFullLightClientAuditFastpqGateKeyV1 =
    "sccp:ton:full-light-client-audit:v1:gate"
private let tonFullLightClientAuditStatementPrefixV1 =
    "sccp:ton:full-light-client-audit:statement:v1"
private let tonRouteCanaryLiveAccountPrefixV1 =
    "iroha:sccp:ton-route-canary-live-account:v1"

/// TON live-account evidence collected by UI code before route canary submission.
public struct TonSccpRouteCanaryEvidenceInput: Equatable {
    public let routeAllowlistHash: String
    public let destinationBindingHash: String
    public let expectedDestinationBindingHash: String?
    public let sourceVerifierMaterialHash: String
    public let sourceAdapterEngineDeploymentHash: String
    public let verifierContractAddress: String
    public let verifierCodeHash: String
    public let accountStatus: String
    public let accountStateHash: String
    public let lastTransactionLt: String
    public let lastTransactionHash: String
    public let verifierCodeBocRootHash: String

    public init(routeAllowlistHash: String,
                destinationBindingHash: String,
                expectedDestinationBindingHash: String? = nil,
                sourceVerifierMaterialHash: String,
                sourceAdapterEngineDeploymentHash: String,
                verifierContractAddress: String,
                verifierCodeHash: String,
                accountStatus: String = "active",
                accountStateHash: String,
                lastTransactionLt: String,
                lastTransactionHash: String,
                verifierCodeBocRootHash: String) {
        self.routeAllowlistHash = routeAllowlistHash
        self.destinationBindingHash = destinationBindingHash
        self.expectedDestinationBindingHash = expectedDestinationBindingHash
        self.sourceVerifierMaterialHash = sourceVerifierMaterialHash
        self.sourceAdapterEngineDeploymentHash = sourceAdapterEngineDeploymentHash
        self.verifierContractAddress = verifierContractAddress
        self.verifierCodeHash = verifierCodeHash
        self.accountStatus = accountStatus
        self.accountStateHash = accountStateHash
        self.lastTransactionLt = lastTransactionLt
        self.lastTransactionHash = lastTransactionHash
        self.verifierCodeBocRootHash = verifierCodeBocRootHash
    }
}

/// SCCP public inputs shared by TON proof requests and message-body builders.
public struct TonSccpPublicInputsInput: Equatable {
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
                targetDomain: UInt32 = sccpDomainTon,
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

/// Governed SORA -> TON destination binding carried in submission metadata.
public struct TonSccpSubmissionDestinationBindingInput: Equatable {
    public let key: String
    public let bindingHash: String

    public init(key: String, bindingHash: String) {
        self.key = key
        self.bindingHash = bindingHash
    }
}

/// SCCP manifest fields used to derive TON submission metadata.
public struct TonSccpSubmissionManifestInput: Equatable {
    public let version: UInt8
    public let localDomain: UInt32
    public let counterpartyDomain: UInt32
    public let securityModel: String
    public let anchorGovernance: String
    public let verifierTarget: String
    public let verifierBackendFamily: String
    public let proofFamily: String
    public let verifierBackendKey: String
    public let messageBackend: String
    public let registryBackend: String
    public let manifestSeed: String
    public let destinationBinding: TonSccpSubmissionDestinationBindingInput?

    public init(version: UInt8 = 1,
                localDomain: UInt32 = sccpDomainSora,
                counterpartyDomain: UInt32 = sccpDomainTon,
                securityModel: String = "RecursiveZk",
                anchorGovernance: String = "CryptographicProof",
                verifierTarget: String = "TonContract",
                verifierBackendFamily: String = "TonContract",
                proofFamily: String = sccpTonStarkFriProofFamilyV1,
                verifierBackendKey: String = sccpTonContractProofBackendV1,
                messageBackend: String,
                registryBackend: String,
                manifestSeed: String,
                destinationBinding: TonSccpSubmissionDestinationBindingInput? = nil) {
        self.version = version
        self.localDomain = localDomain
        self.counterpartyDomain = counterpartyDomain
        self.securityModel = securityModel
        self.anchorGovernance = anchorGovernance
        self.verifierTarget = verifierTarget
        self.verifierBackendFamily = verifierBackendFamily
        self.proofFamily = proofFamily
        self.verifierBackendKey = verifierBackendKey
        self.messageBackend = messageBackend
        self.registryBackend = registryBackend
        self.manifestSeed = manifestSeed
        self.destinationBinding = destinationBinding
    }
}

/// Inputs for canonical TON submission metadata included in message-body BOCs.
public struct TonSccpSubmissionMetadataInput: Equatable {
    public let manifest: TonSccpSubmissionManifestInput
    public let destinationBinding: TonSccpSubmissionDestinationBindingInput?
    public let destinationBindingHash: String?
    public let publicInputs: TonSccpPublicInputsInput
    public let statementHash: String

    public init(manifest: TonSccpSubmissionManifestInput,
                destinationBinding: TonSccpSubmissionDestinationBindingInput? = nil,
                destinationBindingHash: String? = nil,
                publicInputs: TonSccpPublicInputsInput,
                statementHash: String) {
        self.manifest = manifest
        self.destinationBinding = destinationBinding
        self.destinationBindingHash = destinationBindingHash
        self.publicInputs = publicInputs
        self.statementHash = statementHash
    }
}

/// Inputs for a TON internal message body carrying an SCCP proof submission.
public struct TonSccpMessageBodyInput: Equatable {
    fileprivate let proofResult: TonSccpProofResult
    public let publicInputs: TonSccpPublicInputsInput
    public let proofBytes: Data
    public let bundleBytes: Data
    public let statementHash: String
    public let destinationBindingHash: String
    public let metadataBytes: Data
    public let queryId: UInt64?

    @available(*, unavailable, message: "Use init(proofResult:bundleBytes:metadataBytes:queryId:) with a wrapped TON SCCP proof result.")
    public init(publicInputs: TonSccpPublicInputsInput,
                proofBytes: Data,
                bundleBytes: Data,
                statementHash: String,
                destinationBindingHash: String,
                metadataBytes: Data = Data(),
                queryId: UInt64? = nil) {
        fatalError("Use init(proofResult:bundleBytes:metadataBytes:queryId:) with a wrapped TON SCCP proof result.")
    }

    public init(proofResult: TonSccpProofResult,
                bundleBytes: Data,
                metadataBytes: Data = Data(),
                queryId: UInt64? = nil) throws {
        let proofResult = try requireWrappedTonProofResultForSubmission(proofResult)
        guard bundleBytes == proofResult.bundleBytes else {
            throw TonSccpProverError.invalidField("bundleBytes")
        }
        self.proofResult = proofResult
        self.publicInputs = proofResult.publicInputs
        self.proofBytes = proofResult.proofBytes
        self.bundleBytes = bundleBytes
        self.statementHash = proofResult.proofContext.statementHash
        self.destinationBindingHash = proofResult.proofContext.destinationBindingHash
        self.metadataBytes = metadataBytes
        self.queryId = queryId
    }
}

/// One TON SCCP submission argument in Rust template order.
public struct TonSccpSubmissionArgument: Equatable {
    public let key: String
    public let encoding: String
    public let bytesHex: String
}

/// Prebuilt TON SCCP submission envelope for wallet or liteserver broadcasting.
public struct TonSccpSubmission: Equatable {
    public let version: UInt8
    public let envelopeEncoding: String
    public let submissionKind: String
    public let verifierEntrypoint: String
    public let messageBodyBoc: Data
    public let messageBodyBocHex: String
    public let arguments: [TonSccpSubmissionArgument]
    public let envelopeBytes: Data
    public let envelopeHex: String
}

/// Inputs used to build a local TON SCCP proof request.
public struct TonSccpProofRequestInput: Equatable {
    public let publicInputs: TonSccpPublicInputsInput
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let statementHash: String
    public let destinationBindingHash: String
    public let sourceStateVerifierId: String
    public let sourceStateVerifierHash: String
    public let sourceAdapterDeploymentHash: String
    public let sourceAdapterDeploymentReceiptHash: String
    public let backend: String
    public let sourceDomain: UInt32

    public init(publicInputs: TonSccpPublicInputsInput,
                bundleBytes: Data,
                sourceProofBytes: Data = Data(),
                statementHash: String,
                destinationBindingHash: String,
                sourceStateVerifierId: String = sccpTonMainnetShardStateVerifierIdV1,
                sourceStateVerifierHash: String = sccpZeroHashV1,
                sourceAdapterDeploymentHash: String = sccpZeroHashV1,
                sourceAdapterDeploymentReceiptHash: String = sccpZeroHashV1,
                backend: String = sccpTonContractProofBackendV1,
                sourceDomain: UInt32 = sccpDomainTon) {
        self.publicInputs = publicInputs
        self.bundleBytes = bundleBytes
        self.sourceProofBytes = sourceProofBytes
        self.statementHash = statementHash
        self.destinationBindingHash = destinationBindingHash
        self.sourceStateVerifierId = sourceStateVerifierId
        self.sourceStateVerifierHash = sourceStateVerifierHash
        self.sourceAdapterDeploymentHash = sourceAdapterDeploymentHash
        self.sourceAdapterDeploymentReceiptHash = sourceAdapterDeploymentReceiptHash
        self.backend = backend
        self.sourceDomain = sourceDomain
    }

    public init(publicInputs: TonSccpPublicInputsInput,
                bundleBytes: Data,
                sourceProofBytes: Data = Data(),
                statementHash: String,
                destinationBindingHash: String,
                sourceStateVerifierId: String = sccpTonMainnetShardStateVerifierIdV1,
                sourceStateVerifierHash: String = sccpZeroHashV1,
                sourceAdapterDeploymentBinding: TonSccpSourceAdapterDeploymentBinding,
                backend: String = sccpTonContractProofBackendV1,
                sourceDomain: UInt32 = sccpDomainTon) throws {
        let deploymentBinding = try normalizeTonSccpSourceAdapterDeploymentBinding(
            sourceDomain: sourceAdapterDeploymentBinding.sourceDomain,
            targetDomain: sourceAdapterDeploymentBinding.targetDomain,
            sourceAdapterDeploymentHash: sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash: sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash
        )
        guard deploymentBinding.sourceDomain == sourceDomain else {
            throw TonSccpProverError.invalidField("sourceAdapterDeploymentBinding.sourceDomain")
        }
        guard deploymentBinding.sourceDomain == sccpDomainTon else {
            throw TonSccpProverError.invalidSourceDomain(deploymentBinding.sourceDomain)
        }
        guard deploymentBinding.targetDomain == sccpDomainSora else {
            throw TonSccpProverError.invalidField("sourceAdapterDeploymentBinding.targetDomain")
        }
        guard deploymentBinding.sourceAdapterDeploymentHash != sccpZeroHashV1 else {
            throw TonSccpProverError.sourceAdapterDeploymentBindingMismatch
        }
        self.init(
            publicInputs: publicInputs,
            bundleBytes: bundleBytes,
            sourceProofBytes: sourceProofBytes,
            statementHash: statementHash,
            destinationBindingHash: destinationBindingHash,
            sourceStateVerifierId: sourceStateVerifierId,
            sourceStateVerifierHash: sourceStateVerifierHash,
            sourceAdapterDeploymentHash: deploymentBinding.sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash: deploymentBinding.sourceAdapterDeploymentReceiptHash,
            backend: backend,
            sourceDomain: sourceDomain
        )
    }
}

/// Statement and verifier deployment context proved by the local TON SCCP prover.
public struct TonSccpProofContext: Equatable {
    public let version: UInt8
    public let statementHash: String
    public let destinationBindingHash: String
}

/// Source-adapter deployment binding carried by local TON SCCP proof requests.
public typealias TonSccpSourceAdapterDeploymentBinding = SolanaSccpSourceAdapterDeploymentBinding

/// Request passed to a linked local TON SCCP prover.
public struct TonSccpProofRequest: Equatable {
    public let version: UInt8
    public let backend: String
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let publicInputs: TonSccpPublicInputsInput
    public let publicInputsBytes: Data
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let proofContext: TonSccpProofContext
    public let statementHash: String
    public let destinationBindingHash: String
    public let sourceStateVerifierId: String
    public let sourceStateVerifierHash: String
    public let sourceAdapterDeploymentBindingHash: String
    public let sourceAdapterDeploymentBinding: TonSccpSourceAdapterDeploymentBinding
    public let requestHash: String
}

/// Proof bytes returned by a linked local TON SCCP prover.
public struct TonSccpProofResult: Equatable {
    public let version: UInt8
    public let backend: String
    public let proofBytes: Data
    public let proofBase64: String
    public let publicInputs: TonSccpPublicInputsInput
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let proofContext: TonSccpProofContext
    public let statementHash: String
    public let destinationBindingHash: String
    public let sourceStateVerifierId: String
    public let sourceStateVerifierHash: String
    public let sourceAdapterDeploymentBindingHash: String
    public let sourceAdapterDeploymentBinding: TonSccpSourceAdapterDeploymentBinding
    public let requestHash: String
    public let envelopeHash: String

    public init(version: UInt8,
                backend: String,
                proofBytes: Data,
                proofBase64: String,
                publicInputs: TonSccpPublicInputsInput,
                bundleBytes: Data = Data(),
                sourceProofBytes: Data = Data(),
                proofContext: TonSccpProofContext,
                statementHash: String,
                destinationBindingHash: String,
                sourceStateVerifierId: String,
                sourceStateVerifierHash: String,
                sourceAdapterDeploymentBindingHash: String,
                sourceAdapterDeploymentBinding: TonSccpSourceAdapterDeploymentBinding,
                requestHash: String,
                envelopeHash: String) {
        self.version = version
        self.backend = backend
        self.proofBytes = proofBytes
        self.proofBase64 = proofBase64
        self.publicInputs = publicInputs
        self.bundleBytes = bundleBytes
        self.sourceProofBytes = sourceProofBytes
        self.proofContext = proofContext
        self.statementHash = statementHash
        self.destinationBindingHash = destinationBindingHash
        self.sourceStateVerifierId = sourceStateVerifierId
        self.sourceStateVerifierHash = sourceStateVerifierHash
        self.sourceAdapterDeploymentBindingHash = sourceAdapterDeploymentBindingHash
        self.sourceAdapterDeploymentBinding = sourceAdapterDeploymentBinding
        self.requestHash = requestHash
        self.envelopeHash = envelopeHash
    }
}

/// Error cases for TON SCCP local proof request construction.
public enum TonSccpProverError: Error, Equatable {
    case invalidHex32(String)
    case invalidBranch(String)
    case invalidBoc(String)
    case invalidField(String)
    case invalidSourceDomain(UInt32)
    case sourceAdapterDeploymentBindingMismatch
    case localProverUnavailable
    case emptyProof
    case allZeroProof
}

/// Optional async witness resolver backed by app-controlled TON liteserver calls.
public protocol TonSccpWitnessProvider {
    func resolveWitness(_ input: TonSccpProofRequestInput) async throws -> TonSccpProofRequestInput
}

/// TON validator signature proof transcript material used by validator-set transitions.
public struct TonValidatorSignatureProofInput: Equatable {
    public let version: UInt8
    public let totalWeight: UInt64
    public let signedWeight: UInt64
    public let blockMessageHash: String
    public let validatorPublicKeys: [Data]
    public let validatorWeights: [UInt64]
    public let signersBitmap: Data
    public let signatures: [Data]

    public init(version: UInt8 = 1,
                totalWeight: UInt64,
                signedWeight: UInt64,
                blockMessageHash: String,
                validatorPublicKeys: [Data],
                validatorWeights: [UInt64],
                signersBitmap: Data,
                signatures: [Data]) {
        self.version = version
        self.totalWeight = totalWeight
        self.signedWeight = signedWeight
        self.blockMessageHash = blockMessageHash
        self.validatorPublicKeys = validatorPublicKeys
        self.validatorWeights = validatorWeights
        self.signersBitmap = signersBitmap
        self.signatures = signatures
    }
}

/// TON validator-set transition material used by shard-state source proofs.
public struct TonValidatorSetTransitionProofInput: Equatable {
    public let version: UInt8
    public let sourceDomain: UInt32
    public let fromValidatorSetSeqno: UInt64
    public let toValidatorSetSeqno: UInt64
    public let masterchainSeqno: UInt64
    public let masterchainWorkchainId: Int32
    public let masterchainShard: UInt64
    public let masterchainBlockHash: String
    public let masterchainFileHash: String
    public let parentValidatorSetHash: String
    public let nextValidatorSetHash: String
    public let nextValidatorSetPayload: Data
    public let nextValidatorSetPayloadHash: String
    public let nextValidatorSetConfigHash: String
    public let transitionMessageHash: String
    public let transitionSignatureHash: String
    public let validatorSignatureProof: TonValidatorSignatureProofInput

    public init(version: UInt8 = 1,
                sourceDomain: UInt32 = sccpDomainTon,
                fromValidatorSetSeqno: UInt64,
                toValidatorSetSeqno: UInt64,
                masterchainSeqno: UInt64,
                masterchainWorkchainId: Int32 = -1,
                masterchainShard: UInt64 = 0x8000_0000_0000_0000,
                masterchainBlockHash: String,
                masterchainFileHash: String,
                parentValidatorSetHash: String,
                nextValidatorSetHash: String,
                nextValidatorSetPayload: Data,
                nextValidatorSetPayloadHash: String,
                nextValidatorSetConfigHash: String,
                transitionMessageHash: String,
                transitionSignatureHash: String,
                validatorSignatureProof: TonValidatorSignatureProofInput) {
        self.version = version
        self.sourceDomain = sourceDomain
        self.fromValidatorSetSeqno = fromValidatorSetSeqno
        self.toValidatorSetSeqno = toValidatorSetSeqno
        self.masterchainSeqno = masterchainSeqno
        self.masterchainWorkchainId = masterchainWorkchainId
        self.masterchainShard = masterchainShard
        self.masterchainBlockHash = masterchainBlockHash
        self.masterchainFileHash = masterchainFileHash
        self.parentValidatorSetHash = parentValidatorSetHash
        self.nextValidatorSetHash = nextValidatorSetHash
        self.nextValidatorSetPayload = nextValidatorSetPayload
        self.nextValidatorSetPayloadHash = nextValidatorSetPayloadHash
        self.nextValidatorSetConfigHash = nextValidatorSetConfigHash
        self.transitionMessageHash = transitionMessageHash
        self.transitionSignatureHash = transitionSignatureHash
        self.validatorSignatureProof = validatorSignatureProof
    }
}

/// Witness material for a TON shard-state OpenVerify source-state proof request.
public struct TonShardStateProofRequestInput: Equatable {
    public let sourceDomain: UInt32
    public let masterchainSeqno: UInt64
    public let masterchainWorkchainId: Int32
    public let masterchainShard: UInt64
    public let masterchainBlockHash: String
    public let masterchainFileHash: String
    public let validatorSetHash: String
    public let masterchainConfigRoot: String
    public let masterchainConfigProofHash: String
    public let shardWorkchainId: Int32
    public let shardShard: UInt64
    public let shardSeqno: UInt64
    public let shardBlockHash: String
    public let shardFileHash: String
    public let shardStateRoot: String
    public let transactionRoot: String
    public let transactionLt: UInt64
    public let shardStateDictionaryRoot: String
    public let shardStateDictionaryKeyBitLen: UInt16
    public let shardStateDictionaryKey: Data
    public let masterchainSignatureHash: String
    public let shardProofHash: String
    public let shardStateProofBoc: Data
    public let shardStateDictionaryProofBoc: Data
    public let configDictionaryProofBoc: Data
    public let validatorSetTransitionProofs: [TonValidatorSetTransitionProofInput]
    public let sourceStateVerifierId: String
    public let sourceStateVerifierHash: String
    public let sourceTrustAnchorId: String
    public let sourceTrustAnchorHash: String
    public let consensusVerifierId: String
    public let consensusVerifierHash: String
    public let messageInclusionVerifierId: String
    public let messageInclusionVerifierHash: String
    public let finalityPolicyId: String
    public let finalityPolicyHash: String

    public init(sourceDomain: UInt32 = sccpDomainTon,
                masterchainSeqno: UInt64,
                masterchainWorkchainId: Int32 = -1,
                masterchainShard: UInt64 = 0x8000_0000_0000_0000,
                masterchainBlockHash: String,
                masterchainFileHash: String,
                validatorSetHash: String,
                masterchainConfigRoot: String,
                masterchainConfigProofHash: String,
                shardWorkchainId: Int32 = 0,
                shardShard: UInt64,
                shardSeqno: UInt64,
                shardBlockHash: String,
                shardFileHash: String,
                shardStateRoot: String,
                transactionRoot: String,
                transactionLt: UInt64,
                shardStateDictionaryRoot: String,
                shardStateDictionaryKeyBitLen: UInt16,
                shardStateDictionaryKey: Data,
                masterchainSignatureHash: String,
                shardProofHash: String,
                shardStateProofBoc: Data,
                shardStateDictionaryProofBoc: Data,
                configDictionaryProofBoc: Data,
                validatorSetTransitionProofs: [TonValidatorSetTransitionProofInput] = [],
                sourceStateVerifierId: String = sccpTonMainnetShardStateVerifierIdV1,
                sourceStateVerifierHash: String,
                sourceTrustAnchorId: String = "sccp:ton:source-trust-anchor:ton-mainnet-masterchain:v1",
                sourceTrustAnchorHash: String,
                consensusVerifierId: String = "sccp:ton:consensus-verifier:masterchain-block-proof:v1",
                consensusVerifierHash: String,
                messageInclusionVerifierId: String =
                "sccp:ton:message-inclusion-verifier:shard-transaction-branch:v1",
                messageInclusionVerifierHash: String,
                finalityPolicyId: String = "sccp:ton:finality-policy:masterchain-finality:v1",
                finalityPolicyHash: String) {
        self.sourceDomain = sourceDomain
        self.masterchainSeqno = masterchainSeqno
        self.masterchainWorkchainId = masterchainWorkchainId
        self.masterchainShard = masterchainShard
        self.masterchainBlockHash = masterchainBlockHash
        self.masterchainFileHash = masterchainFileHash
        self.validatorSetHash = validatorSetHash
        self.masterchainConfigRoot = masterchainConfigRoot
        self.masterchainConfigProofHash = masterchainConfigProofHash
        self.shardWorkchainId = shardWorkchainId
        self.shardShard = shardShard
        self.shardSeqno = shardSeqno
        self.shardBlockHash = shardBlockHash
        self.shardFileHash = shardFileHash
        self.shardStateRoot = shardStateRoot
        self.transactionRoot = transactionRoot
        self.transactionLt = transactionLt
        self.shardStateDictionaryRoot = shardStateDictionaryRoot
        self.shardStateDictionaryKeyBitLen = shardStateDictionaryKeyBitLen
        self.shardStateDictionaryKey = shardStateDictionaryKey
        self.masterchainSignatureHash = masterchainSignatureHash
        self.shardProofHash = shardProofHash
        self.shardStateProofBoc = shardStateProofBoc
        self.shardStateDictionaryProofBoc = shardStateDictionaryProofBoc
        self.configDictionaryProofBoc = configDictionaryProofBoc
        self.validatorSetTransitionProofs = validatorSetTransitionProofs
        self.sourceStateVerifierId = sourceStateVerifierId
        self.sourceStateVerifierHash = sourceStateVerifierHash
        self.sourceTrustAnchorId = sourceTrustAnchorId
        self.sourceTrustAnchorHash = sourceTrustAnchorHash
        self.consensusVerifierId = consensusVerifierId
        self.consensusVerifierHash = consensusVerifierHash
        self.messageInclusionVerifierId = messageInclusionVerifierId
        self.messageInclusionVerifierHash = messageInclusionVerifierHash
        self.finalityPolicyId = finalityPolicyId
        self.finalityPolicyHash = finalityPolicyHash
    }
}

/// FastPQ public input tuple used by the TON shard-state OpenVerify request.
public struct TonShardStateFastpqPublicInputs: Equatable {
    public let dsid: String
    public let slot: String
    public let oldRoot: String
    public let newRoot: String
    public let permRoot: String
    public let txSetHash: String
}

/// FastPQ metadata transition emitted by the TON shard-state OpenVerify request.
public struct TonShardStateFastpqTransition: Equatable {
    public let key: String
    public let operation: String
    public let oldValue: String
    public let newValue: String
}

/// Request bytes and metadata for a user-side TON shard-state source-state proof.
public struct TonShardStateProofRequest: Equatable {
    public let version: UInt8
    public let proofFamily: String
    public let circuitId: String
    public let parameterSet: String
    public let sourceDomain: UInt32
    public let masterchainSeqno: UInt64
    public let shardSeqno: UInt64
    public let sourceStateVerifierId: String
    public let sourceStateVerifierHash: String
    public let shardStateProofPublicInputsHash: String
    public let statementBytes: Data
    public let witnessCommitmentBytes: Data
    public let verificationContextBytes: Data
    public let schemaDescriptor: Data
    public let publicInputColumns: [[String]]
    public let fastpqPublicInputs: TonShardStateFastpqPublicInputs
    public let fastpqTransitions: [TonShardStateFastpqTransition]
}

/// Source-state verification proof capsule generated by a user-side TON prover.
public struct TonSccpSourceStateVerificationProof: Equatable {
    public let version: UInt8
    public let proofFamily: String
    public let circuitId: String
    public let proofBytes: Data
    public var proofBase64: String {
        proofBytes.base64EncodedString()
    }

    public init(version: UInt8 = 1,
                proofFamily: String = sccpStarkFriProofFamilyV1,
                circuitId: String = sccpTonShardStateOpenVerifyCircuitIdV1,
                proofBytes: Data) {
        self.version = version
        self.proofFamily = proofFamily
        self.circuitId = circuitId
        self.proofBytes = proofBytes
    }
}

/// TON full light-client audit role proven by a user-side prover.
public enum TonSccpFullLightClientAuditRole: Equatable {
    case masterchainConfig
    case validatorSetTransition
    case shardAccountsDictionary
}

/// Input required to build TON full light-client audit proof requests on iOS clients.
public struct TonSccpFullLightClientAuditProofInput: Equatable {
    public let shardState: TonShardStateProofRequestInput
    public let shardStateVerificationProof: TonSccpSourceStateVerificationProof
    public let validatorSetPayloadHash: String
    public let configLeafHash: String
    public let configValueHash: String
    public let sourceVerifierMaterialHash: String
    public let sourceAdapterDeploymentHash: String
    public let fullLightClientGateHash: String
    public let tonMasterchainConfigVerifierHash: String
    public let tonValidatorSetTransitionVerifierHash: String
    public let tonShardAccountsDictionaryVerifierHash: String
    public let shardStateProofPublicInputsHash: String?
    public let shardStateVerificationProofHash: String?

    public init(
        shardState: TonShardStateProofRequestInput,
        shardStateVerificationProof: TonSccpSourceStateVerificationProof,
        validatorSetPayloadHash: String,
        configLeafHash: String,
        configValueHash: String,
        sourceVerifierMaterialHash: String,
        sourceAdapterDeploymentHash: String,
        fullLightClientGateHash: String,
        tonMasterchainConfigVerifierHash: String,
        tonValidatorSetTransitionVerifierHash: String,
        tonShardAccountsDictionaryVerifierHash: String,
        shardStateProofPublicInputsHash: String? = nil,
        shardStateVerificationProofHash: String? = nil
    ) {
        self.shardState = shardState
        self.shardStateVerificationProof = shardStateVerificationProof
        self.validatorSetPayloadHash = validatorSetPayloadHash
        self.configLeafHash = configLeafHash
        self.configValueHash = configValueHash
        self.sourceVerifierMaterialHash = sourceVerifierMaterialHash
        self.sourceAdapterDeploymentHash = sourceAdapterDeploymentHash
        self.fullLightClientGateHash = fullLightClientGateHash
        self.tonMasterchainConfigVerifierHash = tonMasterchainConfigVerifierHash
        self.tonValidatorSetTransitionVerifierHash = tonValidatorSetTransitionVerifierHash
        self.tonShardAccountsDictionaryVerifierHash = tonShardAccountsDictionaryVerifierHash
        self.shardStateProofPublicInputsHash = shardStateProofPublicInputsHash
        self.shardStateVerificationProofHash = shardStateVerificationProofHash
    }
}

/// FastPQ public inputs bound to a TON full light-client audit role proof request.
public struct TonSccpFullLightClientAuditFastpqPublicInputs: Equatable {
    public let dsid: String
    public let slot: String
    public let oldRoot: String
    public let newRoot: String
    public let permRoot: String
    public let txSetHash: String
}

/// One FastPQ transition supplied to a TON full light-client audit role prover.
public struct TonSccpFullLightClientAuditFastpqTransition: Equatable {
    public let key: String
    public let operation: String
    public let oldValue: String
    public let newValue: String
}

/// OpenVerify request for one TON full light-client audit role proof.
public struct TonSccpFullLightClientAuditProofRequest: Equatable {
    public let version: UInt8
    public let proofFamily: String
    public let circuitId: String
    public let parameterSet: String
    public let role: String
    public let roleCode: UInt8
    public let sourceDomain: UInt32
    public let masterchainSeqno: String
    public let shardSeqno: String
    public let verifierId: String
    public let verifierHash: String
    public let sourceStateVerifierId: String
    public let sourceStateVerifierHash: String
    public let sourceVerifierMaterialHash: String
    public let sourceAdapterDeploymentHash: String
    public let fullLightClientGateHash: String
    public let shardStateProofPublicInputsHash: String
    public let shardStateVerificationProofHash: String
    public let auditStatementHash: String
    public let statementBytes: Data
    public let verificationContextBytes: Data
    public let schemaDescriptor: Data
    public let publicInputColumns: [[String]]
    public let fastpqPublicInputs: TonSccpFullLightClientAuditFastpqPublicInputs
    public let fastpqTransitions: [TonSccpFullLightClientAuditFastpqTransition]
}

/// Role-separated TON full light-client audit proof requests.
public struct TonSccpFullLightClientAuditProofRequests: Equatable {
    public let masterchainConfig: TonSccpFullLightClientAuditProofRequest
    public let validatorSetTransition: TonSccpFullLightClientAuditProofRequest
    public let shardAccountsDictionary: TonSccpFullLightClientAuditProofRequest
}

/// Local-first TON SCCP proof wrapper. It never fabricates proofs; callers must link a prover.
public final class TonSccpProver {
    public typealias ProveFunction = (TonSccpProofRequest) async throws -> Data

    private let witnessProvider: TonSccpWitnessProvider?
    private let proveFunction: ProveFunction?

    public init(witnessProvider: TonSccpWitnessProvider? = nil,
                proveFunction: ProveFunction? = nil) {
        self.witnessProvider = witnessProvider
        self.proveFunction = proveFunction
    }

    public func buildRequest(_ input: TonSccpProofRequestInput) async throws -> TonSccpProofRequest {
        let resolved = try await witnessProvider?.resolveWitness(tonSccpWitnessProviderInputSnapshot(input)) ?? input
        return try buildTonSccpProofRequest(resolved)
    }

    public func prove(_ input: TonSccpProofRequestInput) async throws -> TonSccpProofResult {
        let request = try await buildRequest(input)
        guard let proveFunction else {
            throw TonSccpProverError.localProverUnavailable
        }
        try requireProductionTonSccpProofRequest(request)
        let proofBytes = try await proveFunction(tonSccpProofRequestCallbackSnapshot(request))
        return try wrapTonSccpProofResult(proofBytes: proofBytes, request: request)
    }
}

private func tonSccpWitnessProviderInputSnapshot(_ input: TonSccpProofRequestInput) -> TonSccpProofRequestInput {
    TonSccpProofRequestInput(
        publicInputs: input.publicInputs,
        bundleBytes: Data(input.bundleBytes),
        sourceProofBytes: Data(input.sourceProofBytes),
        statementHash: input.statementHash,
        destinationBindingHash: input.destinationBindingHash,
        sourceStateVerifierId: input.sourceStateVerifierId,
        sourceStateVerifierHash: input.sourceStateVerifierHash,
        sourceAdapterDeploymentHash: input.sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash: input.sourceAdapterDeploymentReceiptHash,
        backend: input.backend,
        sourceDomain: input.sourceDomain
    )
}

private func tonSccpProofRequestCallbackSnapshot(_ request: TonSccpProofRequest) -> TonSccpProofRequest {
    TonSccpProofRequest(
        version: request.version,
        backend: request.backend,
        sourceDomain: request.sourceDomain,
        targetDomain: request.targetDomain,
        publicInputs: request.publicInputs,
        publicInputsBytes: Data(request.publicInputsBytes),
        bundleBytes: Data(request.bundleBytes),
        sourceProofBytes: Data(request.sourceProofBytes),
        proofContext: request.proofContext,
        statementHash: request.statementHash,
        destinationBindingHash: request.destinationBindingHash,
        sourceStateVerifierId: request.sourceStateVerifierId,
        sourceStateVerifierHash: request.sourceStateVerifierHash,
        sourceAdapterDeploymentBindingHash: request.sourceAdapterDeploymentBindingHash,
        sourceAdapterDeploymentBinding: request.sourceAdapterDeploymentBinding,
        requestHash: request.requestHash
    )
}

/// Role-separated TON full light-client audit proof capsules generated by a user-side prover.
public struct TonSccpFullLightClientAuditProofs: Equatable {
    public let masterchainConfig: TonSccpSourceStateVerificationProof
    public let validatorSetTransition: TonSccpSourceStateVerificationProof
    public let shardAccountsDictionary: TonSccpSourceStateVerificationProof
}

/// Local-first source-state proof wrapper for UI and mobile TON proof engines.
public final class TonSccpSourceStateProver {
    public typealias ShardStateProveFunction =
        (TonShardStateProofRequest) async throws -> Data
    public typealias FullLightClientAuditProveFunction =
        (TonSccpFullLightClientAuditProofRequest) async throws -> Data

    private let shardStateProveFunction: ShardStateProveFunction?
    private let fullLightClientAuditProveFunction: FullLightClientAuditProveFunction?

    public init(
        shardStateProveFunction: ShardStateProveFunction? = nil,
        fullLightClientAuditProveFunction: FullLightClientAuditProveFunction? = nil
    ) {
        self.shardStateProveFunction = shardStateProveFunction
        self.fullLightClientAuditProveFunction = fullLightClientAuditProveFunction
    }

    public func proveShardState(
        _ input: TonShardStateProofRequestInput
    ) async throws -> TonSccpSourceStateVerificationProof {
        try await proveShardState(request: buildTonShardStateProofRequest(input))
    }

    public func proveShardState(
        request: TonShardStateProofRequest
    ) async throws -> TonSccpSourceStateVerificationProof {
        guard let shardStateProveFunction else {
            throw TonSccpProverError.localProverUnavailable
        }
        try requireTonSourceStateProofRequestForWrapping(request)
        let proofBytes = try await shardStateProveFunction(
            tonSccpShardStateProofRequestCallbackSnapshot(request)
        )
        return try wrapTonSccpSourceStateVerificationProof(
            proofBytes: proofBytes,
            request: request
        )
    }

    public func proveFullLightClientAudit(
        _ input: TonSccpFullLightClientAuditProofInput
    ) async throws -> TonSccpFullLightClientAuditProofs {
        let requests = try buildTonSccpFullLightClientAuditProofRequests(input)
        let masterchainConfig = try await proveFullLightClientAudit(request: requests.masterchainConfig)
        let validatorSetTransition = try await proveFullLightClientAudit(
            request: requests.validatorSetTransition
        )
        let shardAccountsDictionary = try await proveFullLightClientAudit(
            request: requests.shardAccountsDictionary
        )
        return TonSccpFullLightClientAuditProofs(
            masterchainConfig: masterchainConfig,
            validatorSetTransition: validatorSetTransition,
            shardAccountsDictionary: shardAccountsDictionary
        )
    }

    public func proveFullLightClientAudit(
        request: TonSccpFullLightClientAuditProofRequest
    ) async throws -> TonSccpSourceStateVerificationProof {
        guard let fullLightClientAuditProveFunction else {
            throw TonSccpProverError.localProverUnavailable
        }
        try requireTonSourceStateProofRequestForWrapping(request)
        let proofBytes = try await fullLightClientAuditProveFunction(
            tonSccpFullLightClientAuditProofRequestCallbackSnapshot(request)
        )
        return try wrapTonSccpSourceStateVerificationProof(
            proofBytes: proofBytes,
            request: request
        )
    }
}

private func tonSccpShardStateProofRequestCallbackSnapshot(
    _ request: TonShardStateProofRequest
) -> TonShardStateProofRequest {
    TonShardStateProofRequest(
        version: request.version,
        proofFamily: request.proofFamily,
        circuitId: request.circuitId,
        parameterSet: request.parameterSet,
        sourceDomain: request.sourceDomain,
        masterchainSeqno: request.masterchainSeqno,
        shardSeqno: request.shardSeqno,
        sourceStateVerifierId: request.sourceStateVerifierId,
        sourceStateVerifierHash: request.sourceStateVerifierHash,
        shardStateProofPublicInputsHash: request.shardStateProofPublicInputsHash,
        statementBytes: Data(request.statementBytes),
        witnessCommitmentBytes: Data(request.witnessCommitmentBytes),
        verificationContextBytes: Data(request.verificationContextBytes),
        schemaDescriptor: Data(request.schemaDescriptor),
        publicInputColumns: request.publicInputColumns.map { Array($0) },
        fastpqPublicInputs: TonShardStateFastpqPublicInputs(
            dsid: request.fastpqPublicInputs.dsid,
            slot: request.fastpqPublicInputs.slot,
            oldRoot: request.fastpqPublicInputs.oldRoot,
            newRoot: request.fastpqPublicInputs.newRoot,
            permRoot: request.fastpqPublicInputs.permRoot,
            txSetHash: request.fastpqPublicInputs.txSetHash
        ),
        fastpqTransitions: request.fastpqTransitions.map {
            TonShardStateFastpqTransition(
                key: $0.key,
                operation: $0.operation,
                oldValue: $0.oldValue,
                newValue: $0.newValue
            )
        }
    )
}

private func tonSccpFullLightClientAuditProofRequestCallbackSnapshot(
    _ request: TonSccpFullLightClientAuditProofRequest
) -> TonSccpFullLightClientAuditProofRequest {
    TonSccpFullLightClientAuditProofRequest(
        version: request.version,
        proofFamily: request.proofFamily,
        circuitId: request.circuitId,
        parameterSet: request.parameterSet,
        role: request.role,
        roleCode: request.roleCode,
        sourceDomain: request.sourceDomain,
        masterchainSeqno: request.masterchainSeqno,
        shardSeqno: request.shardSeqno,
        verifierId: request.verifierId,
        verifierHash: request.verifierHash,
        sourceStateVerifierId: request.sourceStateVerifierId,
        sourceStateVerifierHash: request.sourceStateVerifierHash,
        sourceVerifierMaterialHash: request.sourceVerifierMaterialHash,
        sourceAdapterDeploymentHash: request.sourceAdapterDeploymentHash,
        fullLightClientGateHash: request.fullLightClientGateHash,
        shardStateProofPublicInputsHash: request.shardStateProofPublicInputsHash,
        shardStateVerificationProofHash: request.shardStateVerificationProofHash,
        auditStatementHash: request.auditStatementHash,
        statementBytes: Data(request.statementBytes),
        verificationContextBytes: Data(request.verificationContextBytes),
        schemaDescriptor: Data(request.schemaDescriptor),
        publicInputColumns: request.publicInputColumns.map { Array($0) },
        fastpqPublicInputs: TonSccpFullLightClientAuditFastpqPublicInputs(
            dsid: request.fastpqPublicInputs.dsid,
            slot: request.fastpqPublicInputs.slot,
            oldRoot: request.fastpqPublicInputs.oldRoot,
            newRoot: request.fastpqPublicInputs.newRoot,
            permRoot: request.fastpqPublicInputs.permRoot,
            txSetHash: request.fastpqPublicInputs.txSetHash
        ),
        fastpqTransitions: request.fastpqTransitions.map {
            TonSccpFullLightClientAuditFastpqTransition(
                key: $0.key,
                operation: $0.operation,
                oldValue: $0.oldValue,
                newValue: $0.newValue
            )
        }
    )
}

/// Canonical SCCP public-input bytes used by TON proof requests and message bodies.
public func canonicalTonSccpPublicInputsBytes(_ input: TonSccpPublicInputsInput) throws -> Data {
    var out = Data()
    out.append(input.version)
    try out.append(tonBytesFromHex32(input.messageId, field: "messageId"))
    try out.append(tonBytesFromHex32(input.payloadHash, field: "payloadHash"))
    tonAppendU32Le(input.targetDomain, to: &out)
    try out.append(tonBytesFromHex32(input.commitmentRoot, field: "commitmentRoot"))
    tonAppendU64Le(input.finalityHeight, to: &out)
    try out.append(tonBytesFromHex32(input.finalityBlockHash, field: "finalityBlockHash"))
    return out
}

/// Canonical live-account evidence bytes for the SORA -> TON route canary.
public func canonicalTonSccpRouteCanaryEvidenceBytes(
    _ input: TonSccpRouteCanaryEvidenceInput
) throws -> Data {
    let routeAllowlistHash = try tonNonZeroBytesFromHex32(
        input.routeAllowlistHash,
        field: "routeAllowlistHash"
    )
    let destinationBindingHash = try tonNonZeroBytesFromHex32(
        input.destinationBindingHash,
        field: "destinationBindingHash"
    )
    let canonicalTonDestinationBindingHash = try sccpDestinationBindingHash(domain: sccpDomainTon)
    let expectedDestinationBindingHash = try tonNormalizeNonZeroHex32(
        input.expectedDestinationBindingHash ?? canonicalTonDestinationBindingHash,
        field: "expectedDestinationBindingHash"
    )
    guard expectedDestinationBindingHash == canonicalTonDestinationBindingHash else {
        throw TonSccpProverError.invalidField("expectedDestinationBindingHash")
    }
    guard "0x" + destinationBindingHash.hexEncodedString() == canonicalTonDestinationBindingHash else {
        throw TonSccpProverError.invalidField("destinationBindingHash")
    }
    let sourceVerifierMaterialHash = try tonNonZeroBytesFromHex32(
        input.sourceVerifierMaterialHash,
        field: "sourceVerifierMaterialHash"
    )
    let sourceAdapterEngineDeploymentHash = try tonNonZeroBytesFromHex32(
        input.sourceAdapterEngineDeploymentHash,
        field: "sourceAdapterEngineDeploymentHash"
    )
    let verifierContractAddress = try tonNormalizeRawAddress(
        input.verifierContractAddress,
        field: "verifierContractAddress"
    )
    let verifierCodeHash = try tonNonZeroBytesFromHex32(
        input.verifierCodeHash,
        field: "verifierCodeHash"
    )
    let accountStatus = try tonNormalizeActiveAccountStatus(input.accountStatus, field: "accountStatus")
    let accountStateHash = try tonNonZeroBytesFromHex32(input.accountStateHash, field: "accountStateHash")
    let lastTransactionLt = try tonNormalizePositiveDecimalText(
        input.lastTransactionLt,
        field: "lastTransactionLt"
    )
    let lastTransactionHash = try tonNonZeroBytesFromHex32(
        input.lastTransactionHash,
        field: "lastTransactionHash"
    )
    let verifierCodeBocRootHash = try tonNonZeroBytesFromHex32(
        input.verifierCodeBocRootHash,
        field: "verifierCodeBocRootHash"
    )
    guard verifierCodeBocRootHash == verifierCodeHash else {
        throw TonSccpProverError.invalidField("verifierCodeBocRootHash")
    }

    var out = Data()
    out.append(1)
    tonAppendU32Le(sccpDomainSora, to: &out)
    tonAppendU32Le(sccpDomainTon, to: &out)
    out.append(routeAllowlistHash)
    out.append(destinationBindingHash)
    out.append(sourceVerifierMaterialHash)
    out.append(sourceAdapterEngineDeploymentHash)
    tonAppendVector(Data(verifierContractAddress.utf8), to: &out)
    out.append(verifierCodeHash)
    tonAppendVector(Data(accountStatus.utf8), to: &out)
    out.append(accountStateHash)
    tonAppendVector(Data(lastTransactionLt.utf8), to: &out)
    out.append(lastTransactionHash)
    out.append(verifierCodeBocRootHash)
    return out
}

/// Hash Rust verifies for the SORA -> TON live-account route canary.
public func tonSccpRouteCanaryEvidenceHash(_ input: TonSccpRouteCanaryEvidenceInput) throws -> String {
    tonHashHex(
        prefix: tonRouteCanaryLiveAccountPrefixV1,
        payload: try canonicalTonSccpRouteCanaryEvidenceBytes(input)
    )
}

/// Canonical TON shard-proof transcript bytes checked by the SCCP source adapter.
public func canonicalTonSccpShardProofBytes(sourceEventDigest: String,
                                            masterchainSeqno: UInt64,
                                            masterchainBlockHash: String,
                                            shardWorkchainId: Int32,
                                            shardShard: UInt64,
                                            shardSeqno: UInt64,
                                            shardBlockHash: String,
                                            shardFileHash: String,
                                            shardStateRoot: String,
                                            transactionRoot: String,
                                            transactionLt: UInt64,
                                            shardStateLeafIndex: UInt64,
                                            shardStateInclusionBranch: [Data],
                                            inclusionBranch: [Data],
                                            shardStateDictionaryRoot: String? = nil,
                                            shardStateDictionaryKeyBitLen: UInt16? = nil,
                                            shardStateDictionaryKey: Data = Data(),
                                            shardStateDictionaryProofBoc: Data = Data(),
                                            shardStateProofBoc: Data = Data()) throws -> Data {
    let shardStateBranch = try tonNormalizeInclusionBranch(shardStateInclusionBranch)
    let branch = try tonNormalizeInclusionBranch(inclusionBranch)
    let hasDictionaryOpening = shardStateDictionaryRoot != nil
        || shardStateDictionaryKeyBitLen != nil
        || !shardStateDictionaryKey.isEmpty
        || !shardStateDictionaryProofBoc.isEmpty
    if hasDictionaryOpening && shardStateProofBoc.isEmpty {
        throw TonSccpProverError.invalidBranch("shardStateProofBoc")
    }
    if !hasDictionaryOpening && !shardStateProofBoc.isEmpty {
        throw TonSccpProverError.invalidBranch("shardStateProofBoc")
    }
    if hasDictionaryOpening && !shardStateBranch.isEmpty {
        throw TonSccpProverError.invalidBranch("shardStateInclusionBranch")
    }
    guard shardWorkchainId == tonBasechainWorkchainId else {
        throw TonSccpProverError.invalidField("shardWorkchainId")
    }
    guard shardShard != 0 else {
        throw TonSccpProverError.invalidField("shardShard")
    }
    guard shardSeqno != 0 else {
        throw TonSccpProverError.invalidField("shardSeqno")
    }
    guard transactionLt != 0 else {
        throw TonSccpProverError.invalidField("transactionLt")
    }
    var out = Data()
    out.append(1)
    try out.append(tonBytesFromHex32(sourceEventDigest, field: "sourceEventDigest"))
    tonAppendU64Le(masterchainSeqno, to: &out)
    try out.append(tonBytesFromHex32(masterchainBlockHash, field: "masterchainBlockHash"))
    tonAppendI32Le(shardWorkchainId, to: &out)
    tonAppendU64Le(shardShard, to: &out)
    tonAppendU64Le(shardSeqno, to: &out)
    try out.append(tonBytesFromHex32(shardBlockHash, field: "shardBlockHash"))
    try out.append(tonNonZeroBytesFromHex32(shardFileHash, field: "shardFileHash"))
    let shardStateRootBytes = try tonBytesFromHex32(shardStateRoot, field: "shardStateRoot")
    let transactionRootBytes = try tonBytesFromHex32(transactionRoot, field: "transactionRoot")
    out.append(shardStateRootBytes)
    out.append(transactionRootBytes)
    tonAppendU64Le(transactionLt, to: &out)
    if !shardStateProofBoc.isEmpty {
        tonAppendVector(shardStateProofBoc, to: &out)
    }
    if hasDictionaryOpening {
        guard let dictionaryRootInput = shardStateDictionaryRoot else {
            throw TonSccpProverError.invalidBranch("shardStateDictionaryRoot")
        }
        guard let dictionaryKeyBitLen = shardStateDictionaryKeyBitLen else {
            throw TonSccpProverError.invalidBranch("shardStateDictionaryKeyBitLen")
        }
        let dictionaryRoot = try tonBytesFromHex32(dictionaryRootInput, field: "shardStateDictionaryRoot")
        guard dictionaryRoot.contains(where: { $0 != 0 }) else {
            throw TonSccpProverError.invalidBranch("shardStateDictionaryRoot")
        }
        guard dictionaryKeyBitLen == tonShardAccountKeyBits else {
            throw TonSccpProverError.invalidBranch("shardStateDictionaryKeyBitLen")
        }
        guard tonHashmapKeyIsCanonical(
            key: shardStateDictionaryKey,
            keyBitLen: Int(dictionaryKeyBitLen)
        ) else {
            throw TonSccpProverError.invalidBranch("shardStateDictionaryKey")
        }
        guard !shardStateDictionaryProofBoc.isEmpty else {
            throw TonSccpProverError.invalidBranch("shardStateDictionaryProofBoc")
        }
        guard try tonShardStateProofRootHash(shardStateProofBoc) == "0x" + shardStateRootBytes.hexEncodedString() else {
            throw TonSccpProverError.invalidBranch("shardStateProofBoc")
        }
        let shardStateOpening = try tonShardStateAccountsOpening(shardStateProofBoc)
        guard shardStateOpening.accountsRootHash == "0x" + dictionaryRoot.hexEncodedString() else {
            throw TonSccpProverError.invalidBranch("shardStateDictionaryRoot")
        }
        guard shardStateOpening.globalId == tonMainnetGlobalId else {
            throw TonSccpProverError.invalidBranch("shardStateGlobalId")
        }
        guard shardStateOpening.workchainId == Int(tonBasechainWorkchainId) else {
            throw TonSccpProverError.invalidBranch("shardStateWorkchainId")
        }
        guard shardStateOpening.workchainId == Int(shardWorkchainId) else {
            throw TonSccpProverError.invalidBranch("shardWorkchainId")
        }
        guard UInt64(shardStateOpening.seqNo) == shardSeqno else {
            throw TonSccpProverError.invalidBranch("shardStateSeqNo")
        }
        guard shardStateOpening.shardId == shardShard else {
            throw TonSccpProverError.invalidBranch("shardShard")
        }
        guard shardStateOpening.seqNo != 0 else {
            throw TonSccpProverError.invalidBranch("shardStateSeqNo")
        }
        guard shardStateOpening.genUtime != 0 else {
            throw TonSccpProverError.invalidBranch("shardStateGenUtime")
        }
        guard shardStateOpening.genLt != 0 else {
            throw TonSccpProverError.invalidBranch("shardStateGenLt")
        }
        guard UInt64(shardStateOpening.minRefMcSeqno) <= masterchainSeqno else {
            throw TonSccpProverError.invalidBranch("shardStateMinRefMcSeqno")
        }
        guard try tonShardStateAccountKeyMatchesShardPrefix(
            key: shardStateDictionaryKey,
            keyBitLen: Int(dictionaryKeyBitLen),
            opening: shardStateOpening
        ) else {
            throw TonSccpProverError.invalidBranch("shardStateDictionaryKey")
        }
        guard let selectedTransaction = try tonShardAccountsLastTransaction(
            shardStateDictionaryProofBoc,
            key: shardStateDictionaryKey,
            keyBitLen: Int(dictionaryKeyBitLen)
        ), selectedTransaction.hash == "0x" + transactionRootBytes.hexEncodedString() else {
            throw TonSccpProverError.invalidBranch("shardStateDictionaryProofBoc")
        }
        guard selectedTransaction.lt == transactionLt else {
            throw TonSccpProverError.invalidBranch("shardStateDictionaryProofBoc")
        }
        out.append(dictionaryRoot)
        tonAppendU16Le(dictionaryKeyBitLen, to: &out)
        tonAppendVector(shardStateDictionaryKey, to: &out)
        tonAppendVector(shardStateDictionaryProofBoc, to: &out)
    }
    tonAppendU64Le(shardStateLeafIndex, to: &out)
    tonAppendU32Le(UInt32(shardStateBranch.count), to: &out)
    for sibling in shardStateBranch {
        out.append(sibling)
    }
    tonAppendU32Le(UInt32(branch.count), to: &out)
    for sibling in branch {
        out.append(sibling)
    }
    return out
}

/// Hash of the canonical TON shard-proof transcript checked by the source adapter.
public func tonSccpShardProofHash(sourceEventDigest: String,
                                  masterchainSeqno: UInt64,
                                  masterchainBlockHash: String,
                                  shardWorkchainId: Int32,
                                  shardShard: UInt64,
                                  shardSeqno: UInt64,
                                  shardBlockHash: String,
                                  shardFileHash: String,
                                  shardStateRoot: String,
                                  transactionRoot: String,
                                  transactionLt: UInt64,
                                  shardStateLeafIndex: UInt64,
                                  shardStateInclusionBranch: [Data],
                                  inclusionBranch: [Data],
                                  shardStateDictionaryRoot: String? = nil,
                                  shardStateDictionaryKeyBitLen: UInt16? = nil,
                                  shardStateDictionaryKey: Data = Data(),
                                  shardStateDictionaryProofBoc: Data = Data(),
                                  shardStateProofBoc: Data = Data()) throws -> String {
    try tonHashHex(
        prefix: "sccp:ton:shard-proof:v1",
        payload: canonicalTonSccpShardProofBytes(
            sourceEventDigest: sourceEventDigest,
            masterchainSeqno: masterchainSeqno,
            masterchainBlockHash: masterchainBlockHash,
            shardWorkchainId: shardWorkchainId,
            shardShard: shardShard,
            shardSeqno: shardSeqno,
            shardBlockHash: shardBlockHash,
            shardFileHash: shardFileHash,
            shardStateRoot: shardStateRoot,
            transactionRoot: transactionRoot,
            transactionLt: transactionLt,
            shardStateLeafIndex: shardStateLeafIndex,
            shardStateInclusionBranch: shardStateInclusionBranch,
            inclusionBranch: inclusionBranch,
            shardStateDictionaryRoot: shardStateDictionaryRoot,
            shardStateDictionaryKeyBitLen: shardStateDictionaryKeyBitLen,
            shardStateDictionaryKey: shardStateDictionaryKey,
            shardStateDictionaryProofBoc: shardStateDictionaryProofBoc,
            shardStateProofBoc: shardStateProofBoc
        )
    )
}

/// Canonical TON validator-set bytes used by SCCP source trust anchors.
public func canonicalTonValidatorSetBytes(validatorPublicKeys: [Data],
                                          validatorWeights: [UInt64]) throws -> Data {
    let (publicKeys, weights) = try tonNormalizeValidatorSet(
        validatorPublicKeys: validatorPublicKeys,
        validatorWeights: validatorWeights
    )
    var out = Data()
    out.append(1)
    tonAppendU32Le(UInt32(publicKeys.count), to: &out)
    for index in publicKeys.indices {
        out.append(publicKeys[index])
        tonAppendU64Le(weights[index], to: &out)
    }
    return out
}

/// Canonical TON next-validator-set payload bytes used by transition proofs.
public func canonicalTonValidatorSetPayloadBytes(validatorPublicKeys: [Data],
                                                 validatorWeights: [UInt64]) throws -> Data {
    try canonicalTonValidatorSetBytes(
        validatorPublicKeys: validatorPublicKeys,
        validatorWeights: validatorWeights
    )
}

private func tonValidateValidatorSetPayload(_ payload: Data) throws {
    guard payload.count >= 5, payload.first == 1 else {
        throw TonSccpProverError.invalidBranch("validatorSetPayload")
    }
    let count = Int(payload[1])
        | (Int(payload[2]) << 8)
        | (Int(payload[3]) << 16)
        | (Int(payload[4]) << 24)
    guard count > 0, count <= tonMaxValidators, payload.count == 5 + count * 40 else {
        throw TonSccpProverError.invalidBranch("validatorSetPayload")
    }
    var seen = Set<Data>()
    var offset = 5
    for index in 0 ..< count {
        let publicKey = payload.subdata(in: offset ..< offset + 32)
        offset += 32
        guard publicKey.contains(where: { $0 != 0 }) else {
            throw TonSccpProverError.invalidBranch("validatorPublicKeys[\(index)]")
        }
        guard seen.insert(publicKey).inserted else {
            throw TonSccpProverError.invalidBranch("validatorPublicKeys[\(index)]")
        }
        let weightBytes = payload.subdata(in: offset ..< offset + 8)
        offset += 8
        let weight = weightBytes.enumerated().reduce(UInt64(0)) { result, item in
            result | (UInt64(item.element) << UInt64(item.offset * 8))
        }
        guard weight != 0 else {
            throw TonSccpProverError.invalidBranch("validatorWeights[\(index)]")
        }
    }
}

/// SCCP TON validator-set hash derived from a canonical validator-set payload.
public func tonValidatorSetHashFromPayload(payload: Data) throws -> String {
    try tonValidateValidatorSetPayload(payload)
    return tonHashHex(prefix: "sccp:ton:validator-set:v1", payload: payload)
}

/// Hash of the canonical TON next-validator-set transition payload.
public func tonValidatorSetPayloadHash(payload: Data) throws -> String {
    try tonValidateValidatorSetPayload(payload)
    return tonHashHex(prefix: "sccp:ton:validator-set-payload:v1", payload: payload)
}

/// Hash of the canonical TON next-validator-set transition payload.
public func tonValidatorSetPayloadHash(validatorPublicKeys: [Data],
                                       validatorWeights: [UInt64]) throws -> String {
    try tonValidatorSetPayloadHash(
        payload: canonicalTonValidatorSetPayloadBytes(
            validatorPublicKeys: validatorPublicKeys,
            validatorWeights: validatorWeights
        )
    )
}

/// SCCP TON validator-set hash derived from canonical validator keys and weights.
public func tonValidatorSetHash(validatorPublicKeys: [Data],
                                validatorWeights: [UInt64]) throws -> String {
    try tonValidatorSetHashFromPayload(
        payload: canonicalTonValidatorSetBytes(
            validatorPublicKeys: validatorPublicKeys,
            validatorWeights: validatorWeights
        )
    )
}

/// Canonical TON masterchain config leaf bytes.
public func canonicalTonMasterchainConfigLeafBytes(version: UInt8 = 1,
                                                    sourceDomain: UInt32,
                                                    masterchainSeqno: UInt64,
                                                    masterchainBlockHash: String,
                                                    shardStateRoot: String,
                                                    validatorSetHash: String,
                                                    validatorSetPayloadHash: String) throws -> Data {
    guard version == 1 else {
        throw TonSccpProverError.invalidField("TON masterchain config leaf version")
    }
    var out = Data()
    out.append(version)
    tonAppendU32Le(sourceDomain, to: &out)
    tonAppendU64Le(masterchainSeqno, to: &out)
    try out.append(tonBytesFromHex32(masterchainBlockHash, field: "masterchainBlockHash"))
    try out.append(tonBytesFromHex32(shardStateRoot, field: "shardStateRoot"))
    try out.append(tonBytesFromHex32(validatorSetHash, field: "validatorSetHash"))
    try out.append(tonBytesFromHex32(validatorSetPayloadHash, field: "validatorSetPayloadHash"))
    return out
}

/// Hash of the canonical TON masterchain config leaf transcript.
public func tonMasterchainConfigLeafHash(version: UInt8 = 1,
                                         sourceDomain: UInt32,
                                         masterchainSeqno: UInt64,
                                         masterchainBlockHash: String,
                                         shardStateRoot: String,
                                         validatorSetHash: String,
                                         validatorSetPayloadHash: String) throws -> String {
    try tonHashHex(
        prefix: "sccp:ton:masterchain-config-leaf:v1",
        payload: canonicalTonMasterchainConfigLeafBytes(
            version: version,
            sourceDomain: sourceDomain,
            masterchainSeqno: masterchainSeqno,
            masterchainBlockHash: masterchainBlockHash,
            shardStateRoot: shardStateRoot,
            validatorSetHash: validatorSetHash,
            validatorSetPayloadHash: validatorSetPayloadHash
        )
    )
}

/// Canonical TON masterchain config proof bytes.
public func canonicalTonMasterchainConfigProofBytes(version: UInt8 = 1,
                                                    sourceDomain: UInt32,
                                                    masterchainSeqno: UInt64,
                                                    masterchainBlockHash: String,
                                                    shardStateRoot: String,
                                                    configRoot: String,
                                                    validatorSetHash: String,
                                                    validatorSetPayloadHash: String,
                                                    configLeafHash: String,
                                                    configLeafIndex: UInt64,
                                                    configValueHash: String,
                                                    configDictionaryProofBoc: Data,
                                                    configInclusionBranch: [Data]) throws -> Data {
    guard version == 1 else {
        throw TonSccpProverError.invalidField("version")
    }
    guard sourceDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidSourceDomain(sourceDomain)
    }
    guard masterchainSeqno != 0 else {
        throw TonSccpProverError.invalidField("masterchainSeqno")
    }
    guard configLeafIndex == sccpTonCurrentValidatorSetConfigParam else {
        throw TonSccpProverError.invalidField("configLeafIndex")
    }
    let branch = try tonNormalizeInclusionBranch(configInclusionBranch)
    guard branch.isEmpty else {
        throw TonSccpProverError.invalidBranch("configInclusionBranch")
    }
    let masterchainBlockHashBytes = try tonNonZeroBytesFromHex32(
        masterchainBlockHash,
        field: "masterchainBlockHash"
    )
    let shardStateRootBytes = try tonNonZeroBytesFromHex32(shardStateRoot, field: "shardStateRoot")
    let configRootBytes = try tonNonZeroBytesFromHex32(configRoot, field: "configRoot")
    let configValueHashBytes = try tonNonZeroBytesFromHex32(configValueHash, field: "configValueHash")
    guard !configDictionaryProofBoc.isEmpty else {
        throw TonSccpProverError.invalidBoc("configDictionaryProofBoc")
    }
    guard try tonHashmapEProofRootHash(configDictionaryProofBoc) == "0x" + configRootBytes.hexEncodedString() else {
        throw TonSccpProverError.invalidBoc("configDictionaryProofBoc")
    }
    guard try tonHashmapECellRefValueHash(
        configDictionaryProofBoc,
        key: tonCurrentValidatorSetConfigKey(),
        keyBitLen: sccpTonConfigParamKeyBits
    ) == "0x" + configValueHashBytes.hexEncodedString() else {
        throw TonSccpProverError.invalidBoc("configDictionaryProofBoc")
    }
    let validatorSetPayloadHashBytes = try tonBytesFromHex32(
        validatorSetPayloadHash,
        field: "validatorSetPayloadHash"
    )
    guard validatorSetPayloadHashBytes.contains(where: { $0 != 0 }) else {
        throw TonSccpProverError.invalidField("validatorSetPayloadHash")
    }
    guard let validatorSetPayload = try tonConfigValidatorSetPayloadFromProofBoc(configDictionaryProofBoc) else {
        throw TonSccpProverError.invalidBoc("configDictionaryProofBoc")
    }
    guard try tonValidatorSetPayloadHash(payload: validatorSetPayload)
        == "0x" + validatorSetPayloadHashBytes.hexEncodedString() else {
        throw TonSccpProverError.invalidBoc("configDictionaryProofBoc")
    }
    let validatorSetHashBytes = try tonNonZeroBytesFromHex32(validatorSetHash, field: "validatorSetHash")
    guard try tonValidatorSetHashFromPayload(payload: validatorSetPayload)
        == "0x" + validatorSetHashBytes.hexEncodedString() else {
        throw TonSccpProverError.invalidBranch("validatorSetHash")
    }
    let configLeafHashBytes = try tonNonZeroBytesFromHex32(configLeafHash, field: "configLeafHash")
    let expectedConfigLeafHash = try tonMasterchainConfigLeafHash(
        sourceDomain: sourceDomain,
        masterchainSeqno: masterchainSeqno,
        masterchainBlockHash: "0x" + masterchainBlockHashBytes.hexEncodedString(),
        shardStateRoot: "0x" + shardStateRootBytes.hexEncodedString(),
        validatorSetHash: "0x" + validatorSetHashBytes.hexEncodedString(),
        validatorSetPayloadHash: "0x" + validatorSetPayloadHashBytes.hexEncodedString()
    )
    guard configLeafHashBytes == (try tonBytesFromHex32(expectedConfigLeafHash, field: "configLeafHash")) else {
        throw TonSccpProverError.invalidBranch("configLeafHash")
    }
    var out = Data()
    out.append(version)
    tonAppendU32Le(sourceDomain, to: &out)
    tonAppendU64Le(masterchainSeqno, to: &out)
    out.append(masterchainBlockHashBytes)
    out.append(shardStateRootBytes)
    out.append(configRootBytes)
    out.append(validatorSetHashBytes)
    out.append(validatorSetPayloadHashBytes)
    out.append(configLeafHashBytes)
    tonAppendU16Le(UInt16(sccpTonConfigParamKeyBits), to: &out)
    tonAppendU64Le(configLeafIndex, to: &out)
    out.append(configValueHashBytes)
    tonAppendVector(configDictionaryProofBoc, to: &out)
    tonAppendU32Le(UInt32(branch.count), to: &out)
    for sibling in branch {
        tonAppendVector(sibling, to: &out)
    }
    return out
}

/// Hash of the canonical TON masterchain config proof transcript.
public func tonMasterchainConfigProofHash(version: UInt8 = 1,
                                          sourceDomain: UInt32,
                                          masterchainSeqno: UInt64,
                                          masterchainBlockHash: String,
                                          shardStateRoot: String,
                                          configRoot: String,
                                          validatorSetHash: String,
                                          validatorSetPayloadHash: String,
                                          configLeafHash: String,
                                          configLeafIndex: UInt64,
                                          configValueHash: String,
                                          configDictionaryProofBoc: Data,
                                          configInclusionBranch: [Data]) throws -> String {
    try tonHashHex(
        prefix: "sccp:ton:masterchain-config-proof:v1",
        payload: canonicalTonMasterchainConfigProofBytes(
            version: version,
            sourceDomain: sourceDomain,
            masterchainSeqno: masterchainSeqno,
            masterchainBlockHash: masterchainBlockHash,
            shardStateRoot: shardStateRoot,
            configRoot: configRoot,
            validatorSetHash: validatorSetHash,
            validatorSetPayloadHash: validatorSetPayloadHash,
            configLeafHash: configLeafHash,
            configLeafIndex: configLeafIndex,
            configValueHash: configValueHash,
            configDictionaryProofBoc: configDictionaryProofBoc,
            configInclusionBranch: configInclusionBranch
        )
    )
}

/// Canonical TON masterchain block-message bytes signed by validators.
public func canonicalTonMasterchainBlockMessageBytes(sourceDomain: UInt32,
                                                     masterchainSeqno: UInt64,
                                                     masterchainWorkchainId: Int32,
                                                     masterchainShard: UInt64,
                                                     masterchainBlockHash: String,
                                                     masterchainFileHash: String,
                                                     validatorSetHash: String,
                                                     masterchainConfigRoot: String,
                                                     masterchainConfigProofHash: String,
                                                     shardWorkchainId: Int32,
                                                     shardShard: UInt64,
                                                     shardSeqno: UInt64,
                                                     shardBlockHash: String,
                                                     shardFileHash: String,
                                                     shardStateRoot: String,
                                                     transactionRoot: String,
                                                     shardProofHash: String) throws -> Data {
    var out = Data()
    out.append(1)
    tonAppendU32Le(sourceDomain, to: &out)
    tonAppendU64Le(masterchainSeqno, to: &out)
    guard masterchainWorkchainId == tonMasterchainWorkchainId else {
        throw TonSccpProverError.invalidField("masterchainWorkchainId")
    }
    guard masterchainShard == tonMasterchainShard else {
        throw TonSccpProverError.invalidField("masterchainShard")
    }
    tonAppendI32Le(masterchainWorkchainId, to: &out)
    tonAppendU64Le(masterchainShard, to: &out)
    try out.append(tonNonZeroBytesFromHex32(masterchainBlockHash, field: "masterchainBlockHash"))
    try out.append(tonNonZeroBytesFromHex32(masterchainFileHash, field: "masterchainFileHash"))
    try out.append(tonBytesFromHex32(validatorSetHash, field: "validatorSetHash"))
    try out.append(tonBytesFromHex32(masterchainConfigRoot, field: "masterchainConfigRoot"))
    try out.append(tonBytesFromHex32(masterchainConfigProofHash, field: "masterchainConfigProofHash"))
    guard shardWorkchainId == tonBasechainWorkchainId else {
        throw TonSccpProverError.invalidField("shardWorkchainId")
    }
    guard shardShard != 0 else {
        throw TonSccpProverError.invalidField("shardShard")
    }
    guard shardSeqno != 0 else {
        throw TonSccpProverError.invalidField("shardSeqno")
    }
    tonAppendI32Le(shardWorkchainId, to: &out)
    tonAppendU64Le(shardShard, to: &out)
    tonAppendU64Le(shardSeqno, to: &out)
    try out.append(tonBytesFromHex32(shardBlockHash, field: "shardBlockHash"))
    try out.append(tonNonZeroBytesFromHex32(shardFileHash, field: "shardFileHash"))
    try out.append(tonBytesFromHex32(shardStateRoot, field: "shardStateRoot"))
    try out.append(tonBytesFromHex32(transactionRoot, field: "transactionRoot"))
    try out.append(tonBytesFromHex32(shardProofHash, field: "shardProofHash"))
    return out
}

/// Hash of the canonical TON masterchain block-message transcript.
public func tonMasterchainBlockMessageHash(sourceDomain: UInt32,
                                           masterchainSeqno: UInt64,
                                           masterchainWorkchainId: Int32,
                                           masterchainShard: UInt64,
                                           masterchainBlockHash: String,
                                           masterchainFileHash: String,
                                           validatorSetHash: String,
                                           masterchainConfigRoot: String,
                                           masterchainConfigProofHash: String,
                                           shardWorkchainId: Int32,
                                           shardShard: UInt64,
                                           shardSeqno: UInt64,
                                           shardBlockHash: String,
                                           shardFileHash: String,
                                           shardStateRoot: String,
                                           transactionRoot: String,
                                           shardProofHash: String) throws -> String {
    try tonHashHex(
        prefix: "sccp:ton:masterchain-block-message:v1",
        payload: canonicalTonMasterchainBlockMessageBytes(
            sourceDomain: sourceDomain,
            masterchainSeqno: masterchainSeqno,
            masterchainWorkchainId: masterchainWorkchainId,
            masterchainShard: masterchainShard,
            masterchainBlockHash: masterchainBlockHash,
            masterchainFileHash: masterchainFileHash,
            validatorSetHash: validatorSetHash,
            masterchainConfigRoot: masterchainConfigRoot,
            masterchainConfigProofHash: masterchainConfigProofHash,
            shardWorkchainId: shardWorkchainId,
            shardShard: shardShard,
            shardSeqno: shardSeqno,
            shardBlockHash: shardBlockHash,
            shardFileHash: shardFileHash,
            shardStateRoot: shardStateRoot,
            transactionRoot: transactionRoot,
            shardProofHash: shardProofHash
        )
    )
}

/// Canonical TON masterchain validator-signature capsule bytes.
public func canonicalTonMasterchainValidatorSignaturesBytes(_ proof: TonValidatorSignatureProofInput,
                                                            validatorSetHash: String? = nil) throws -> Data {
    let derivedValidatorSetHash = try tonValidatorSetHash(
        validatorPublicKeys: proof.validatorPublicKeys,
        validatorWeights: proof.validatorWeights
    )
    if let validatorSetHash, validatorSetHash != derivedValidatorSetHash {
        throw TonSccpProverError.invalidBranch("validatorSetHash")
    }
    var out = try tonCanonicalValidatorSignatureProofBytes(proof)
    try out.append(tonBytesFromHex32(derivedValidatorSetHash, field: "validatorSetHash"))
    return out
}

/// Hash of the canonical TON masterchain validator-signature capsule transcript.
public func tonMasterchainValidatorSignaturesHash(_ proof: TonValidatorSignatureProofInput,
                                                  validatorSetHash: String? = nil) throws -> String {
    try tonHashHex(
        prefix: "sccp:ton:masterchain-signatures:v1",
        payload: canonicalTonMasterchainValidatorSignaturesBytes(
            proof,
            validatorSetHash: validatorSetHash
        )
    )
}

/// Canonical TON validator-set transition message bytes.
public func canonicalTonValidatorSetTransitionMessageBytes(sourceDomain: UInt32,
                                                           fromValidatorSetSeqno: UInt64,
                                                           toValidatorSetSeqno: UInt64,
                                                           masterchainSeqno: UInt64,
                                                           masterchainWorkchainId: Int32,
                                                           masterchainShard: UInt64,
                                                           masterchainBlockHash: String,
                                                           masterchainFileHash: String,
                                                           parentValidatorSetHash: String,
                                                           nextValidatorSetHash: String,
                                                           nextValidatorSetPayloadHash: String,
                                                           nextValidatorSetConfigHash: String) throws -> Data {
    guard sourceDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidSourceDomain(sourceDomain)
    }
    let nextSeqno = fromValidatorSetSeqno.addingReportingOverflow(1)
    guard !nextSeqno.overflow, nextSeqno.partialValue == toValidatorSetSeqno else {
        throw TonSccpProverError.invalidField("toValidatorSetSeqno")
    }
    guard masterchainSeqno != 0 else {
        throw TonSccpProverError.invalidField("masterchainSeqno")
    }
    var out = Data()
    out.append(1)
    tonAppendU32Le(sourceDomain, to: &out)
    tonAppendU64Le(fromValidatorSetSeqno, to: &out)
    tonAppendU64Le(toValidatorSetSeqno, to: &out)
    tonAppendU64Le(masterchainSeqno, to: &out)
    guard masterchainWorkchainId == tonMasterchainWorkchainId else {
        throw TonSccpProverError.invalidField("masterchainWorkchainId")
    }
    guard masterchainShard == tonMasterchainShard else {
        throw TonSccpProverError.invalidField("masterchainShard")
    }
    tonAppendI32Le(masterchainWorkchainId, to: &out)
    tonAppendU64Le(masterchainShard, to: &out)
    try out.append(tonNonZeroBytesFromHex32(masterchainBlockHash, field: "masterchainBlockHash"))
    try out.append(tonNonZeroBytesFromHex32(masterchainFileHash, field: "masterchainFileHash"))
    try out.append(tonNonZeroBytesFromHex32(parentValidatorSetHash, field: "parentValidatorSetHash"))
    try out.append(tonNonZeroBytesFromHex32(nextValidatorSetHash, field: "nextValidatorSetHash"))
    try out.append(tonBytesFromHex32(nextValidatorSetPayloadHash, field: "nextValidatorSetPayloadHash"))
    try out.append(tonNonZeroBytesFromHex32(nextValidatorSetConfigHash, field: "nextValidatorSetConfigHash"))
    return out
}

/// Hash of the canonical TON validator-set transition message transcript.
public func tonValidatorSetTransitionMessageHash(sourceDomain: UInt32,
                                                 fromValidatorSetSeqno: UInt64,
                                                 toValidatorSetSeqno: UInt64,
                                                 masterchainSeqno: UInt64,
                                                 masterchainWorkchainId: Int32,
                                                 masterchainShard: UInt64,
                                                 masterchainBlockHash: String,
                                                 masterchainFileHash: String,
                                                 parentValidatorSetHash: String,
                                                 nextValidatorSetHash: String,
                                                 nextValidatorSetPayloadHash: String,
                                                 nextValidatorSetConfigHash: String) throws -> String {
    try tonHashHex(
        prefix: "sccp:ton:validator-set-transition-message:v1",
        payload: canonicalTonValidatorSetTransitionMessageBytes(
            sourceDomain: sourceDomain,
            fromValidatorSetSeqno: fromValidatorSetSeqno,
            toValidatorSetSeqno: toValidatorSetSeqno,
            masterchainSeqno: masterchainSeqno,
            masterchainWorkchainId: masterchainWorkchainId,
            masterchainShard: masterchainShard,
            masterchainBlockHash: masterchainBlockHash,
            masterchainFileHash: masterchainFileHash,
            parentValidatorSetHash: parentValidatorSetHash,
            nextValidatorSetHash: nextValidatorSetHash,
            nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash: nextValidatorSetConfigHash
        )
    )
}

/// Canonical TON validator-set transition signature transcript bytes.
public func canonicalTonValidatorSetTransitionSignatureBytes(version: UInt8 = 1,
                                                             sourceDomain: UInt32,
                                                             fromValidatorSetSeqno: UInt64,
                                                             toValidatorSetSeqno: UInt64,
                                                             masterchainSeqno: UInt64,
                                                             masterchainWorkchainId: Int32,
                                                             masterchainShard: UInt64,
                                                             masterchainBlockHash: String,
                                                             masterchainFileHash: String,
                                                             parentValidatorSetHash: String,
                                                             nextValidatorSetHash: String,
                                                             nextValidatorSetPayload: Data,
                                                             nextValidatorSetPayloadHash: String,
                                                             nextValidatorSetConfigHash: String,
                                                             transitionMessageHash: String,
                                                             validatorSignatureProof: TonValidatorSignatureProofInput) throws -> Data {
    guard version == 1 else {
        throw TonSccpProverError.invalidField("version")
    }
    let parentHash = try tonValidatorSetHash(
        validatorPublicKeys: validatorSignatureProof.validatorPublicKeys,
        validatorWeights: validatorSignatureProof.validatorWeights
    )
    let parentHashBytes = try tonBytesFromHex32(parentHash, field: "parentValidatorSetHash")
    let providedParentHashBytes = try tonBytesFromHex32(
        parentValidatorSetHash,
        field: "parentValidatorSetHash"
    )
    guard providedParentHashBytes == parentHashBytes else {
        throw TonSccpProverError.invalidBranch("parentValidatorSetHash")
    }
    guard try tonValidatorSetPayloadHash(payload: nextValidatorSetPayload) == nextValidatorSetPayloadHash else {
        throw TonSccpProverError.invalidBranch("nextValidatorSetPayloadHash")
    }
    guard try tonValidatorSetHashFromPayload(payload: nextValidatorSetPayload) == nextValidatorSetHash else {
        throw TonSccpProverError.invalidBranch("nextValidatorSetHash")
    }
    let transitionMessageHashBytes = try tonBytesFromHex32(
        transitionMessageHash,
        field: "transitionMessageHash"
    )
    let expectedTransitionMessageHash = try tonValidatorSetTransitionMessageHash(
        sourceDomain: sourceDomain,
        fromValidatorSetSeqno: fromValidatorSetSeqno,
        toValidatorSetSeqno: toValidatorSetSeqno,
        masterchainSeqno: masterchainSeqno,
        masterchainWorkchainId: masterchainWorkchainId,
        masterchainShard: masterchainShard,
        masterchainBlockHash: masterchainBlockHash,
        masterchainFileHash: masterchainFileHash,
        parentValidatorSetHash: parentValidatorSetHash,
        nextValidatorSetHash: nextValidatorSetHash,
        nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
        nextValidatorSetConfigHash: nextValidatorSetConfigHash
    )
    guard transitionMessageHashBytes == (try tonBytesFromHex32(
        expectedTransitionMessageHash,
        field: "transitionMessageHash"
    )) else {
        throw TonSccpProverError.invalidBranch("transitionMessageHash")
    }
    guard try tonBytesFromHex32(validatorSignatureProof.blockMessageHash, field: "blockMessageHash")
        == transitionMessageHashBytes else {
        throw TonSccpProverError.invalidBranch("validatorSignatureProof.blockMessageHash")
    }
    var out = Data()
    out.append(version)
    tonAppendU32Le(sourceDomain, to: &out)
    tonAppendU64Le(fromValidatorSetSeqno, to: &out)
    tonAppendU64Le(toValidatorSetSeqno, to: &out)
    tonAppendU64Le(masterchainSeqno, to: &out)
    guard masterchainWorkchainId == tonMasterchainWorkchainId else {
        throw TonSccpProverError.invalidField("masterchainWorkchainId")
    }
    guard masterchainShard == tonMasterchainShard else {
        throw TonSccpProverError.invalidField("masterchainShard")
    }
    tonAppendI32Le(masterchainWorkchainId, to: &out)
    tonAppendU64Le(masterchainShard, to: &out)
    try out.append(tonNonZeroBytesFromHex32(masterchainBlockHash, field: "masterchainBlockHash"))
    try out.append(tonNonZeroBytesFromHex32(masterchainFileHash, field: "masterchainFileHash"))
    out.append(providedParentHashBytes)
    try out.append(tonBytesFromHex32(nextValidatorSetHash, field: "nextValidatorSetHash"))
    tonAppendVector(nextValidatorSetPayload, to: &out)
    try out.append(tonBytesFromHex32(nextValidatorSetPayloadHash, field: "nextValidatorSetPayloadHash"))
    try out.append(tonBytesFromHex32(nextValidatorSetConfigHash, field: "nextValidatorSetConfigHash"))
    out.append(transitionMessageHashBytes)
    out.append(parentHashBytes)
    try out.append(tonCanonicalValidatorSignatureProofBytes(validatorSignatureProof))
    return out
}

/// Hash of the canonical TON validator-set transition signature transcript.
public func tonValidatorSetTransitionSignatureHash(version: UInt8 = 1,
                                                   sourceDomain: UInt32,
                                                   fromValidatorSetSeqno: UInt64,
                                                   toValidatorSetSeqno: UInt64,
                                                   masterchainSeqno: UInt64,
                                                   masterchainWorkchainId: Int32,
                                                   masterchainShard: UInt64,
                                                   masterchainBlockHash: String,
                                                   masterchainFileHash: String,
                                                   parentValidatorSetHash: String,
                                                   nextValidatorSetHash: String,
                                                   nextValidatorSetPayload: Data,
                                                   nextValidatorSetPayloadHash: String,
                                                   nextValidatorSetConfigHash: String,
                                                   transitionMessageHash: String,
                                                   validatorSignatureProof: TonValidatorSignatureProofInput) throws -> String {
    try tonHashHex(
        prefix: "sccp:ton:validator-set-transition-signatures:v1",
        payload: canonicalTonValidatorSetTransitionSignatureBytes(
            version: version,
            sourceDomain: sourceDomain,
            fromValidatorSetSeqno: fromValidatorSetSeqno,
            toValidatorSetSeqno: toValidatorSetSeqno,
            masterchainSeqno: masterchainSeqno,
            masterchainWorkchainId: masterchainWorkchainId,
            masterchainShard: masterchainShard,
            masterchainBlockHash: masterchainBlockHash,
            masterchainFileHash: masterchainFileHash,
            parentValidatorSetHash: parentValidatorSetHash,
            nextValidatorSetHash: nextValidatorSetHash,
            nextValidatorSetPayload: nextValidatorSetPayload,
            nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash: nextValidatorSetConfigHash,
            transitionMessageHash: transitionMessageHash,
            validatorSignatureProof: validatorSignatureProof
        )
    )
}

/// Canonical public input bytes for a TON shard-state source-state OpenVerify statement.
public func canonicalTonShardStateProofPublicInputsBytes(_ input: TonShardStateProofRequestInput) throws -> Data {
    let normalized = try tonNormalizeShardStateSourceStateInput(input)
    var out = Data()
    out.append(normalized.version)
    tonAppendU32Le(normalized.sourceDomain, to: &out)
    tonAppendU64Le(normalized.masterchainSeqno, to: &out)
    tonAppendI32Le(normalized.masterchainWorkchainId, to: &out)
    tonAppendU64Le(normalized.masterchainShard, to: &out)
    try out.append(tonBytesFromHex32(normalized.masterchainBlockHash, field: "masterchainBlockHash"))
    try out.append(tonBytesFromHex32(normalized.masterchainFileHash, field: "masterchainFileHash"))
    try out.append(tonBytesFromHex32(normalized.validatorSetHash, field: "validatorSetHash"))
    try out.append(tonBytesFromHex32(normalized.masterchainConfigRoot, field: "masterchainConfigRoot"))
    try out.append(tonBytesFromHex32(normalized.masterchainConfigProofHash, field: "masterchainConfigProofHash"))
    tonAppendI32Le(normalized.shardWorkchainId, to: &out)
    tonAppendU64Le(normalized.shardShard, to: &out)
    tonAppendU64Le(normalized.shardSeqno, to: &out)
    try out.append(tonBytesFromHex32(normalized.shardBlockHash, field: "shardBlockHash"))
    try out.append(tonBytesFromHex32(normalized.shardFileHash, field: "shardFileHash"))
    try out.append(tonBytesFromHex32(normalized.shardStateRoot, field: "shardStateRoot"))
    try out.append(tonBytesFromHex32(normalized.transactionRoot, field: "transactionRoot"))
    tonAppendU64Le(normalized.transactionLt, to: &out)
    try out.append(tonBytesFromHex32(normalized.shardStateDictionaryRoot, field: "shardStateDictionaryRoot"))
    tonAppendU16Le(normalized.shardStateDictionaryKeyBitLen, to: &out)
    tonAppendVector(normalized.shardStateDictionaryKey, to: &out)
    try out.append(tonBytesFromHex32(normalized.masterchainSignatureHash, field: "masterchainSignatureHash"))
    try out.append(tonBytesFromHex32(normalized.shardProofHash, field: "shardProofHash"))
    try out.append(tonBytesFromHex32(normalized.shardStateProofBocHash, field: "shardStateProofBocHash"))
    try out.append(tonBytesFromHex32(normalized.shardAccountsProofBocHash, field: "shardAccountsProofBocHash"))
    try out.append(tonBytesFromHex32(normalized.configProofBocHash, field: "configProofBocHash"))
    try out.append(tonBytesFromHex32(normalized.transitionChainHash, field: "transitionChainHash"))
    return out
}

/// Hash of the canonical TON shard-state source-state public inputs.
public func tonShardStateProofPublicInputsHash(_ input: TonShardStateProofRequestInput) throws -> String {
    try tonHashHex(
        prefix: tonShardStateProofPublicInputsPrefixV1,
        payload: canonicalTonShardStateProofPublicInputsBytes(input)
    )
}

/// Witness commitment bytes for a TON shard-state source-state OpenVerify proof.
public func canonicalTonShardStateWitnessCommitmentBytes(_ input: TonShardStateProofRequestInput) throws -> Data {
    let normalized = try tonNormalizeShardStateSourceStateInput(input)
    var out = Data()
    out.append(normalized.version)
    tonAppendVector(normalized.shardStateProofBoc, to: &out)
    tonAppendVector(normalized.shardStateDictionaryProofBoc, to: &out)
    tonAppendVector(normalized.configDictionaryProofBoc, to: &out)
    tonAppendU32Le(UInt32(normalized.validatorSetTransitionProofs.count), to: &out)
    for transition in normalized.validatorSetTransitionProofs {
        try out.append(tonCanonicalValidatorSetTransitionProofBytes(transition))
    }
    return out
}

/// Verification context bytes for a TON shard-state source-state OpenVerify proof.
public func canonicalTonShardStateVerificationContextBytes(_ input: TonShardStateProofRequestInput) throws -> Data {
    let normalized = try tonNormalizeShardStateSourceStateInput(input)
    var out = Data()
    out.append(normalized.version)
    try tonAppendString(normalized.sourceStateVerifierId, field: "sourceStateVerifierId", to: &out)
    try out.append(tonBytesFromHex32(normalized.sourceStateVerifierHash, field: "sourceStateVerifierHash"))
    try tonAppendString(normalized.sourceTrustAnchorId, field: "sourceTrustAnchorId", to: &out)
    try out.append(tonBytesFromHex32(normalized.sourceTrustAnchorHash, field: "sourceTrustAnchorHash"))
    try tonAppendString(normalized.consensusVerifierId, field: "consensusVerifierId", to: &out)
    try out.append(tonBytesFromHex32(normalized.consensusVerifierHash, field: "consensusVerifierHash"))
    try tonAppendString(normalized.messageInclusionVerifierId, field: "messageInclusionVerifierId", to: &out)
    try out.append(tonBytesFromHex32(normalized.messageInclusionVerifierHash, field: "messageInclusionVerifierHash"))
    try tonAppendString(normalized.finalityPolicyId, field: "finalityPolicyId", to: &out)
    try out.append(tonBytesFromHex32(normalized.finalityPolicyHash, field: "finalityPolicyHash"))
    return out
}

/// OpenVerify public input columns for a TON shard-state source-state proof.
public func tonShardStatePublicInputColumns(_ input: TonShardStateProofRequestInput) throws -> [[String]] {
    let normalized = try tonNormalizeShardStateSourceStateInput(input)
    let publicInputsHash = try tonShardStateProofPublicInputsHash(input)
    return [
        ["0x" + tonSccpWordU32Le(normalized.sourceDomain).hexEncodedString()],
        ["0x" + tonSccpWordU64Le(normalized.masterchainSeqno).hexEncodedString()],
        ["0x" + tonSccpWordI32Le(normalized.masterchainWorkchainId).hexEncodedString()],
        ["0x" + tonSccpWordU64Le(normalized.masterchainShard).hexEncodedString()],
        [normalized.masterchainBlockHash],
        [normalized.validatorSetHash],
        [normalized.masterchainConfigRoot],
        ["0x" + tonSccpWordI32Le(normalized.shardWorkchainId).hexEncodedString()],
        ["0x" + tonSccpWordU64Le(normalized.shardShard).hexEncodedString()],
        ["0x" + tonSccpWordU64Le(normalized.shardSeqno).hexEncodedString()],
        [normalized.shardBlockHash],
        [normalized.shardStateRoot],
        [normalized.shardStateDictionaryRoot],
        [normalized.transactionRoot],
        ["0x" + tonSccpWordU64Le(normalized.transactionLt).hexEncodedString()],
        [publicInputsHash],
    ]
}

/// OpenVerify schema descriptor for TON shard-state source-state proof requests.
public func tonShardStateOpenVerifySchemaDescriptor(_ input: TonShardStateProofRequestInput) throws -> Data {
    let normalized = try tonNormalizeShardStateSourceStateInput(input)
    var out = Data()
    out.append(normalized.version)
    try tonAppendString(sccpTonShardStateOpenVerifyCircuitIdV1, field: "circuitId", to: &out)
    try tonAppendString(tonShardStateFastpqParameterSetV1, field: "parameterSet", to: &out)
    tonAppendI32Le(Int32(tonMainnetGlobalId), to: &out)
    tonAppendU32Le(normalized.sourceDomain, to: &out)
    for requiredInput in [
        "source_domain",
        "masterchain_seqno",
        "masterchain_workchain_id",
        "masterchain_shard",
        "masterchain_block_hash",
        "validator_set_hash",
        "masterchain_config_root",
        "shard_workchain_id",
        "shard_shard",
        "shard_seqno",
        "shard_block_hash",
        "shard_state_root",
        "shard_state_dictionary_root",
        "transaction_root",
        "transaction_lt",
        "shard_state_proof_public_inputs_hash",
    ] {
        try tonAppendString(requiredInput, field: "requiredInput", to: &out)
    }
    return out
}

/// Build a user-side TON shard-state OpenVerify proof request from witness material.
public func buildTonShardStateProofRequest(_ input: TonShardStateProofRequestInput) throws -> TonShardStateProofRequest {
    let normalized = try tonNormalizeShardStateSourceStateInput(input)
    let statementBytes = try canonicalTonShardStateProofPublicInputsBytes(input)
    let witnessCommitmentBytes = try canonicalTonShardStateWitnessCommitmentBytes(input)
    let verificationContextBytes = try canonicalTonShardStateVerificationContextBytes(input)
    let publicInputsHash = try tonShardStateProofPublicInputsHash(input)
    let dsidHash = try tonHashBytes(
        prefix: tonShardStateFastpqDsidPrefixV1,
        payload: tonBytesFromHex32(publicInputsHash, field: "shardStateProofPublicInputsHash")
    )
    return TonShardStateProofRequest(
        version: 1,
        proofFamily: tonStarkFriProofFamilyV1,
        circuitId: sccpTonShardStateOpenVerifyCircuitIdV1,
        parameterSet: tonShardStateFastpqParameterSetV1,
        sourceDomain: normalized.sourceDomain,
        masterchainSeqno: normalized.masterchainSeqno,
        shardSeqno: normalized.shardSeqno,
        sourceStateVerifierId: normalized.sourceStateVerifierId,
        sourceStateVerifierHash: normalized.sourceStateVerifierHash,
        shardStateProofPublicInputsHash: publicInputsHash,
        statementBytes: statementBytes,
        witnessCommitmentBytes: witnessCommitmentBytes,
        verificationContextBytes: verificationContextBytes,
        schemaDescriptor: try tonShardStateOpenVerifySchemaDescriptor(input),
        publicInputColumns: try tonShardStatePublicInputColumns(input),
        fastpqPublicInputs: TonShardStateFastpqPublicInputs(
            dsid: "0x" + dsidHash.prefix(16).hexEncodedString(),
            slot: String(normalized.masterchainSeqno),
            oldRoot: normalized.masterchainConfigRoot,
            newRoot: normalized.shardStateRoot,
            permRoot: normalized.shardStateDictionaryRoot,
            txSetHash: publicInputsHash
        ),
        fastpqTransitions: [
            TonShardStateFastpqTransition(
                key: tonShardStateFastpqStatementKeyV1,
                operation: "meta_set",
                oldValue: "0x",
                newValue: "0x" + statementBytes.hexEncodedString()
            ),
            TonShardStateFastpqTransition(
                key: tonShardStateFastpqWitnessKeyV1,
                operation: "meta_set",
                oldValue: "0x",
                newValue: "0x" + witnessCommitmentBytes.hexEncodedString()
            ),
            TonShardStateFastpqTransition(
                key: tonShardStateFastpqContextKeyV1,
                operation: "meta_set",
                oldValue: "0x",
                newValue: "0x" + verificationContextBytes.hexEncodedString()
            ),
        ]
    )
}

/// Wrap completed TON shard-state proof bytes with the originating request metadata.
public func wrapTonSccpSourceStateVerificationProof(
    proofBytes: Data,
    request: TonShardStateProofRequest
) throws -> TonSccpSourceStateVerificationProof {
    try requireTonSourceStateProofRequestForWrapping(request)
    return try wrapTonSccpSourceStateVerificationProof(
        proofBytes: proofBytes,
        version: request.version,
        proofFamily: request.proofFamily,
        circuitId: request.circuitId,
        sourceDomain: request.sourceDomain
    )
}

/// Wrap completed TON full-light audit proof bytes with the originating role request metadata.
public func wrapTonSccpSourceStateVerificationProof(
    proofBytes: Data,
    request: TonSccpFullLightClientAuditProofRequest
) throws -> TonSccpSourceStateVerificationProof {
    try requireTonSourceStateProofRequestForWrapping(request)
    return try wrapTonSccpSourceStateVerificationProof(
        proofBytes: proofBytes,
        version: request.version,
        proofFamily: request.proofFamily,
        circuitId: request.circuitId,
        sourceDomain: request.sourceDomain
    )
}

/// Canonical source-state proof capsule bytes hashed by TON audit role requests.
public func canonicalTonSccpSourceStateVerificationProofBytes(
    _ proof: TonSccpSourceStateVerificationProof
) throws -> Data {
    guard proof.version == 1,
          proof.proofFamily == tonStarkFriProofFamilyV1,
          tonSccpSourceStateVerificationCircuitIds.contains(proof.circuitId) else {
        throw TonSccpProverError.invalidField("sourceStateVerificationProof")
    }
    guard !proof.proofBytes.isEmpty else {
        throw TonSccpProverError.invalidField("proofBytes")
    }
    guard proof.proofBytes.count <= sccpNativeRecursiveMaxProofBytes else {
        throw TonSccpProverError.invalidField("proofBytes")
    }
    guard proof.proofBytes.contains(where: { $0 != 0 }) else {
        throw TonSccpProverError.allZeroProof
    }
    var out = Data()
    out.append(proof.version)
    try tonAppendString(proof.proofFamily, field: "proofFamily", to: &out)
    try tonAppendString(proof.circuitId, field: "circuitId", to: &out)
    tonAppendVector(proof.proofBytes, to: &out)
    return out
}

/// Hash the nested TON shard-state source-state verification proof capsule.
public func tonSccpShardStateVerificationProofHash(
    _ proof: TonSccpSourceStateVerificationProof
) throws -> String {
    guard proof.circuitId == sccpTonShardStateOpenVerifyCircuitIdV1 else {
        throw TonSccpProverError.invalidField("shardStateVerificationProof")
    }
    return try tonHashHex(
        prefix: "sccp:ton:source-state-verification-proof:v1",
        payload: canonicalTonSccpSourceStateVerificationProofBytes(proof)
    )
}

private func requireTonSourceStateProofRequestForWrapping(
    _ request: TonShardStateProofRequest
) throws {
    guard request.version == 1 else {
        throw TonSccpProverError.invalidField("request.version")
    }
    guard request.proofFamily == tonStarkFriProofFamilyV1 else {
        throw TonSccpProverError.invalidField("request.proofFamily")
    }
    guard request.circuitId == sccpTonShardStateOpenVerifyCircuitIdV1 else {
        throw TonSccpProverError.invalidField("request.circuitId")
    }
    guard request.parameterSet == tonShardStateFastpqParameterSetV1 else {
        throw TonSccpProverError.invalidField("request.parameterSet")
    }
    guard request.sourceDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidSourceDomain(request.sourceDomain)
    }
    guard request.masterchainSeqno != 0, request.shardSeqno != 0 else {
        throw TonSccpProverError.invalidField("request.seqno")
    }
    guard request.sourceStateVerifierId == sccpTonMainnetShardStateVerifierIdV1 else {
        throw TonSccpProverError.invalidField("request.sourceStateVerifierId")
    }
    let sourceStateVerifierHash = try tonNonZeroBytesFromHex32(
        request.sourceStateVerifierHash,
        field: "request.sourceStateVerifierHash"
    )
    try tonRejectTemplateSourceStateVerifierHash(sourceStateVerifierHash)
    let derivedPublicInputsHash = tonHashHex(
        prefix: tonShardStateProofPublicInputsPrefixV1,
        payload: request.statementBytes
    )
    let suppliedPublicInputsHash = try tonNormalizeNonZeroHex32(
        request.shardStateProofPublicInputsHash,
        field: "request.shardStateProofPublicInputsHash"
    )
    guard suppliedPublicInputsHash == derivedPublicInputsHash else {
        throw TonSccpProverError.invalidField("request.shardStateProofPublicInputsHash")
    }
    let shardDsidHash = try tonHashBytes(
        prefix: tonShardStateFastpqDsidPrefixV1,
        payload: tonBytesFromHex32(derivedPublicInputsHash, field: "request.shardStateProofPublicInputsHash")
    )
    guard request.fastpqPublicInputs.dsid == "0x" + shardDsidHash.prefix(16).hexEncodedString() else {
        throw TonSccpProverError.invalidField("request.fastpqPublicInputs.dsid")
    }
    guard try tonNormalizeNonZeroHex32(
        request.fastpqPublicInputs.txSetHash,
        field: "request.fastpqPublicInputs.txSetHash"
    ) == derivedPublicInputsHash else {
        throw TonSccpProverError.invalidField("request.fastpqPublicInputs.txSetHash")
    }
    try requireTonOpenVerifyRequestPayloadForWrapping(
        statementBytes: request.statementBytes,
        witnessCommitmentBytes: request.witnessCommitmentBytes,
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
            tonTransitionCheck($0)
        },
        expectedTransitionEntries: tonShardStateExpectedTransitionChecks(
            statementBytes: request.statementBytes,
            witnessCommitmentBytes: request.witnessCommitmentBytes,
            verificationContextBytes: request.verificationContextBytes
        )
    )
}

private func requireTonSourceStateProofRequestForWrapping(
    _ request: TonSccpFullLightClientAuditProofRequest
) throws {
    guard request.version == 1 else {
        throw TonSccpProverError.invalidField("request.version")
    }
    guard request.proofFamily == tonStarkFriProofFamilyV1 else {
        throw TonSccpProverError.invalidField("request.proofFamily")
    }
    guard request.parameterSet == tonFullLightClientAuditFastpqParameterSetV1 else {
        throw TonSccpProverError.invalidField("request.parameterSet")
    }
    guard request.sourceDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidSourceDomain(request.sourceDomain)
    }
    let profile = try tonAuditRoleProfileForRequest(request.role)
    guard request.roleCode == profile.code else {
        throw TonSccpProverError.invalidField("request.roleCode")
    }
    guard request.circuitId == profile.circuitId else {
        throw TonSccpProverError.invalidField("request.circuitId")
    }
    guard request.verifierId == profile.verifierId else {
        throw TonSccpProverError.invalidField("request.verifierId")
    }
    guard let masterchainSeqno = UInt64(request.masterchainSeqno),
          let shardSeqno = UInt64(request.shardSeqno),
          masterchainSeqno != 0,
          shardSeqno != 0 else {
        throw TonSccpProverError.invalidField("request.seqno")
    }
    _ = try tonNormalizeNonZeroHex32(request.verifierHash, field: "request.verifierHash")
    guard request.sourceStateVerifierId == sccpTonMainnetShardStateVerifierIdV1 else {
        throw TonSccpProverError.invalidField("request.sourceStateVerifierId")
    }
    let sourceStateVerifierHash = try tonNonZeroBytesFromHex32(
        request.sourceStateVerifierHash,
        field: "request.sourceStateVerifierHash"
    )
    try tonRejectTemplateSourceStateVerifierHash(sourceStateVerifierHash)
    for (field, hash) in [
        ("request.sourceVerifierMaterialHash", request.sourceVerifierMaterialHash),
        ("request.sourceAdapterDeploymentHash", request.sourceAdapterDeploymentHash),
        ("request.fullLightClientGateHash", request.fullLightClientGateHash),
        ("request.shardStateProofPublicInputsHash", request.shardStateProofPublicInputsHash),
        ("request.shardStateVerificationProofHash", request.shardStateVerificationProofHash),
        ("request.auditStatementHash", request.auditStatementHash),
    ] {
        _ = try tonNormalizeNonZeroHex32(hash, field: field)
    }
    let normalizedFullLightClientGateHash = try tonNormalizeNonZeroHex32(
        request.fullLightClientGateHash,
        field: "request.fullLightClientGateHash"
    )
    let normalizedAuditStatementHash = try tonNormalizeNonZeroHex32(
        request.auditStatementHash,
        field: "request.auditStatementHash"
    )
    let derivedAuditStatementHash = tonHashHex(
        prefix: tonFullLightClientAuditStatementPrefixV1,
        payload: request.statementBytes
    )
    guard normalizedAuditStatementHash == derivedAuditStatementHash else {
        throw TonSccpProverError.invalidField("request.auditStatementHash")
    }
    var dsidPreimage = Data()
    dsidPreimage.append(profile.code)
    try dsidPreimage.append(tonBytesFromHex32(normalizedAuditStatementHash, field: "request.auditStatementHash"))
    let auditDsidHash = tonHashBytes(prefix: tonFullLightClientAuditFastpqDsidPrefixV1, payload: dsidPreimage)
    guard request.fastpqPublicInputs.dsid == "0x" + auditDsidHash.prefix(16).hexEncodedString() else {
        throw TonSccpProverError.invalidField("request.fastpqPublicInputs.dsid")
    }
    guard try tonNormalizeNonZeroHex32(
        request.fastpqPublicInputs.txSetHash,
        field: "request.fastpqPublicInputs.txSetHash"
    ) == derivedAuditStatementHash else {
        throw TonSccpProverError.invalidField("request.fastpqPublicInputs.txSetHash")
    }
    try requireTonOpenVerifyRequestPayloadForWrapping(
        statementBytes: request.statementBytes,
        witnessCommitmentBytes: nil,
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
            tonTransitionCheck($0)
        },
        expectedTransitionEntries: tonAuditExpectedTransitionChecks(
            profile: profile,
            statementBytes: request.statementBytes,
            verificationContextBytes: request.verificationContextBytes,
            fullLightClientGateHash: normalizedFullLightClientGateHash
        )
    )
}

private func tonAuditRoleProfileForRequest(
    _ role: String
) throws -> TonSccpFullLightClientAuditRoleProfile {
    switch try tonNormalizeNonEmptyString(role, field: "request.role") {
    case "masterchainConfig", "masterchain_config":
        return tonAuditRoleProfile(.masterchainConfig)
    case "validatorSetTransition", "validator_set_transition":
        return tonAuditRoleProfile(.validatorSetTransition)
    case "shardAccountsDictionary", "shard_accounts_dictionary":
        return tonAuditRoleProfile(.shardAccountsDictionary)
    default:
        throw TonSccpProverError.invalidField("request.role")
    }
}

private struct TonFastpqTransitionCheck: Equatable {
    let key: String
    let operation: String
    let oldValue: String
    let newValue: String
}

private func tonTransitionCheck(_ transition: TonShardStateFastpqTransition) -> TonFastpqTransitionCheck {
    TonFastpqTransitionCheck(
        key: transition.key,
        operation: transition.operation,
        oldValue: transition.oldValue,
        newValue: transition.newValue
    )
}

private func tonTransitionCheck(
    _ transition: TonSccpFullLightClientAuditFastpqTransition
) -> TonFastpqTransitionCheck {
    TonFastpqTransitionCheck(
        key: transition.key,
        operation: transition.operation,
        oldValue: transition.oldValue,
        newValue: transition.newValue
    )
}

private func tonShardStateExpectedTransitionChecks(
    statementBytes: Data,
    witnessCommitmentBytes: Data,
    verificationContextBytes: Data
) -> [TonFastpqTransitionCheck] {
    [
        TonFastpqTransitionCheck(
            key: tonShardStateFastpqStatementKeyV1,
            operation: "meta_set",
            oldValue: "0x",
            newValue: "0x" + statementBytes.hexEncodedString()
        ),
        TonFastpqTransitionCheck(
            key: tonShardStateFastpqWitnessKeyV1,
            operation: "meta_set",
            oldValue: "0x",
            newValue: "0x" + witnessCommitmentBytes.hexEncodedString()
        ),
        TonFastpqTransitionCheck(
            key: tonShardStateFastpqContextKeyV1,
            operation: "meta_set",
            oldValue: "0x",
            newValue: "0x" + verificationContextBytes.hexEncodedString()
        ),
    ]
}

private func tonAuditExpectedTransitionChecks(
    profile: TonSccpFullLightClientAuditRoleProfile,
    statementBytes: Data,
    verificationContextBytes: Data,
    fullLightClientGateHash: String
) -> [TonFastpqTransitionCheck] {
    [
        TonFastpqTransitionCheck(
            key: "0x" + tonFullLightClientAuditFastpqKey(
                tonFullLightClientAuditFastpqStatementKeyV1,
                profile: profile
            ).hexEncodedString(),
            operation: "meta_set",
            oldValue: "0x",
            newValue: "0x" + statementBytes.hexEncodedString()
        ),
        TonFastpqTransitionCheck(
            key: "0x" + tonFullLightClientAuditFastpqKey(
                tonFullLightClientAuditFastpqContextKeyV1,
                profile: profile
            ).hexEncodedString(),
            operation: "meta_set",
            oldValue: "0x",
            newValue: "0x" + verificationContextBytes.hexEncodedString()
        ),
        TonFastpqTransitionCheck(
            key: "0x" + tonFullLightClientAuditFastpqKey(
                tonFullLightClientAuditFastpqGateKeyV1,
                profile: profile
            ).hexEncodedString(),
            operation: "meta_set",
            oldValue: "0x",
            newValue: fullLightClientGateHash
        ),
    ]
}

private func requireTonOpenVerifyRequestPayloadForWrapping(
    statementBytes: Data,
    witnessCommitmentBytes: Data?,
    verificationContextBytes: Data,
    schemaDescriptor: Data,
    publicInputColumns: [[String]],
    fastpqFields: [String],
    transitionEntries: [TonFastpqTransitionCheck],
    expectedTransitionEntries: [TonFastpqTransitionCheck]
) throws {
    guard !statementBytes.isEmpty else {
        throw TonSccpProverError.invalidField("request.statementBytes")
    }
    if let witnessCommitmentBytes {
        guard !witnessCommitmentBytes.isEmpty else {
            throw TonSccpProverError.invalidField("request.witnessCommitmentBytes")
        }
    }
    guard !verificationContextBytes.isEmpty else {
        throw TonSccpProverError.invalidField("request.verificationContextBytes")
    }
    guard !schemaDescriptor.isEmpty else {
        throw TonSccpProverError.invalidField("request.schemaDescriptor")
    }
    guard !publicInputColumns.isEmpty else {
        throw TonSccpProverError.invalidField("request.publicInputColumns")
    }
    for (columnIndex, column) in publicInputColumns.enumerated() {
        guard !column.isEmpty else {
            throw TonSccpProverError.invalidField("request.publicInputColumns[\(columnIndex)]")
        }
        for (valueIndex, value) in column.enumerated() {
            _ = try tonNormalizeNonEmptyString(
                value,
                field: "request.publicInputColumns[\(columnIndex)][\(valueIndex)]"
            )
        }
    }
    for (index, field) in fastpqFields.enumerated() {
        _ = try tonNormalizeNonEmptyString(field, field: "request.fastpqPublicInputs[\(index)]")
    }
    guard !transitionEntries.isEmpty else {
        throw TonSccpProverError.invalidField("request.fastpqTransitions")
    }
    for (index, transition) in transitionEntries.enumerated() {
        _ = try tonNormalizeNonEmptyString(transition.key, field: "request.fastpqTransitions[\(index)].key")
        _ = try tonNormalizeNonEmptyString(
            transition.operation,
            field: "request.fastpqTransitions[\(index)].operation"
        )
        _ = try tonNormalizeNonEmptyString(
            transition.oldValue,
            field: "request.fastpqTransitions[\(index)].oldValue"
        )
        _ = try tonNormalizeNonEmptyString(
            transition.newValue,
            field: "request.fastpqTransitions[\(index)].newValue"
        )
    }
    guard transitionEntries.sorted(by: { $0.key < $1.key }) ==
        expectedTransitionEntries.sorted(by: { $0.key < $1.key }) else {
        throw TonSccpProverError.invalidField("request.fastpqTransitions")
    }
}

private func wrapTonSccpSourceStateVerificationProof(
    proofBytes: Data,
    version: UInt8,
    proofFamily: String,
    circuitId: String,
    sourceDomain: UInt32
) throws -> TonSccpSourceStateVerificationProof {
    guard version == 1 else {
        throw TonSccpProverError.invalidField("sourceStateProof.version")
    }
    guard proofFamily == tonStarkFriProofFamilyV1 else {
        throw TonSccpProverError.invalidField("sourceStateProof.proofFamily")
    }
    guard sourceDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidSourceDomain(sourceDomain)
    }
    guard tonSccpSourceStateVerificationCircuitIds.contains(circuitId) else {
        throw TonSccpProverError.invalidField("sourceStateProof.circuitId")
    }
    guard !proofBytes.isEmpty else {
        throw TonSccpProverError.emptyProof
    }
    guard proofBytes.count <= sccpNativeRecursiveMaxProofBytes else {
        throw TonSccpProverError.invalidField("proofBytes")
    }
    guard proofBytes.contains(where: { $0 != 0 }) else {
        throw TonSccpProverError.allZeroProof
    }
    return TonSccpSourceStateVerificationProof(
        version: version,
        proofFamily: proofFamily,
        circuitId: circuitId,
        proofBytes: proofBytes
    )
}

/// Canonical statement bytes for one TON full light-client audit role.
public func canonicalTonSccpFullLightClientAuditStatementBytes(
    _ input: TonSccpFullLightClientAuditProofInput,
    role: TonSccpFullLightClientAuditRole
) throws -> Data {
    let value = try tonNormalizeFullLightClientAuditInput(input, role: role)
    let profile = tonAuditRoleProfile(role)
    let shardState = value.shardState
    var out = Data()
    out.append(1)
    out.append(profile.code)
    try tonAppendString(profile.circuitId, field: "circuitId", to: &out)
    try tonAppendString(sccpTonContractProofBackendV1, field: "backend", to: &out)
    tonAppendI32Le(Int32(tonMainnetGlobalId), to: &out)
    tonAppendU32Le(shardState.sourceDomain, to: &out)
    tonAppendU64Le(shardState.masterchainSeqno, to: &out)
    tonAppendI32Le(shardState.masterchainWorkchainId, to: &out)
    tonAppendU64Le(shardState.masterchainShard, to: &out)
    try out.append(tonBytesFromHex32(shardState.masterchainBlockHash, field: "masterchainBlockHash"))
    try out.append(tonBytesFromHex32(shardState.masterchainFileHash, field: "masterchainFileHash"))
    try out.append(tonBytesFromHex32(shardState.validatorSetHash, field: "validatorSetHash"))
    try out.append(tonBytesFromHex32(shardState.masterchainConfigRoot, field: "masterchainConfigRoot"))
    try out.append(tonBytesFromHex32(shardState.masterchainConfigProofHash, field: "masterchainConfigProofHash"))
    tonAppendI32Le(shardState.shardWorkchainId, to: &out)
    tonAppendU64Le(shardState.shardShard, to: &out)
    tonAppendU64Le(shardState.shardSeqno, to: &out)
    try out.append(tonBytesFromHex32(shardState.shardBlockHash, field: "shardBlockHash"))
    try out.append(tonBytesFromHex32(shardState.shardFileHash, field: "shardFileHash"))
    try out.append(tonBytesFromHex32(shardState.shardStateRoot, field: "shardStateRoot"))
    try out.append(tonBytesFromHex32(shardState.shardStateDictionaryRoot, field: "shardStateDictionaryRoot"))
    try out.append(tonBytesFromHex32(shardState.transactionRoot, field: "transactionRoot"))
    tonAppendU64Le(shardState.transactionLt, to: &out)
    try out.append(tonBytesFromHex32(shardState.masterchainSignatureHash, field: "masterchainSignatureHash"))
    try out.append(tonBytesFromHex32(shardState.shardProofHash, field: "shardProofHash"))
    try out.append(
        tonBytesFromHex32(value.shardStateVerificationProofHash, field: "shardStateVerificationProofHash")
    )
    try out.append(tonBytesFromHex32(value.shardStateProofPublicInputsHash, field: "shardStateProofPublicInputsHash"))
    switch role {
    case .masterchainConfig:
        try out.append(tonBytesFromHex32(value.validatorSetPayloadHash, field: "validatorSetPayloadHash"))
        try out.append(tonBytesFromHex32(value.configLeafHash, field: "configLeafHash"))
        try out.append(tonBytesFromHex32(value.configValueHash, field: "configValueHash"))
        try out.append(tonBytesFromHex32(shardState.configProofBocHash, field: "configProofBocHash"))
    case .validatorSetTransition:
        try out.append(tonBytesFromHex32(shardState.transitionChainHash, field: "transitionChainHash"))
        tonAppendU32Le(UInt32(shardState.validatorSetTransitionProofs.count), to: &out)
        for transition in shardState.validatorSetTransitionProofs {
            try out.append(tonCanonicalValidatorSetTransitionProofBytes(transition))
        }
    case .shardAccountsDictionary:
        try out.append(tonBytesFromHex32(shardState.shardStateProofBocHash, field: "shardStateProofBocHash"))
        try out.append(tonBytesFromHex32(shardState.shardAccountsProofBocHash, field: "shardAccountsProofBocHash"))
        tonAppendU16Le(shardState.shardStateDictionaryKeyBitLen, to: &out)
        tonAppendVector(shardState.shardStateDictionaryKey, to: &out)
        try out.append(
            tonBytesFromHex32(value.shardStateProofPublicInputsHash, field: "shardStateProofPublicInputsHash")
        )
    }
    return out
}

/// Hash one TON full light-client audit role statement.
public func tonSccpFullLightClientAuditStatementHash(
    _ input: TonSccpFullLightClientAuditProofInput,
    role: TonSccpFullLightClientAuditRole
) throws -> String {
    try tonHashHex(
        prefix: tonFullLightClientAuditStatementPrefixV1,
        payload: canonicalTonSccpFullLightClientAuditStatementBytes(input, role: role)
    )
}

/// OpenVerify public-input columns for one TON full light-client audit role.
public func tonSccpFullLightClientAuditPublicInputColumns(
    _ input: TonSccpFullLightClientAuditProofInput,
    role: TonSccpFullLightClientAuditRole
) throws -> [[String]] {
    let value = try tonNormalizeFullLightClientAuditInput(input, role: role)
    let profile = tonAuditRoleProfile(role)
    let shardState = value.shardState
    let statementHash = try tonSccpFullLightClientAuditStatementHash(input, role: role)
    try tonRequireFullLightClientAuditRequestHashSeparation(value, statementHash: statementHash)
    var columns = [
        ["0x" + tonSccpWordU8(profile.code).hexEncodedString()],
        ["0x" + tonSccpWordU32Le(shardState.sourceDomain).hexEncodedString()],
        ["0x" + tonSccpWordU64Le(shardState.masterchainSeqno).hexEncodedString()],
        [shardState.masterchainBlockHash],
        ["0x" + tonSccpWordU64Le(shardState.shardSeqno).hexEncodedString()],
        [shardState.shardBlockHash],
        [statementHash],
        [value.sourceVerifierMaterialHash],
        [value.sourceAdapterDeploymentHash],
        [value.fullLightClientGateHash],
        [value.verifierHash],
    ]
    for column in try tonAuditRoleColumns(value) {
        columns.append([column])
    }
    return columns
}

/// OpenVerify schema descriptor for one TON full light-client audit role.
public func tonSccpFullLightClientAuditOpenVerifySchemaDescriptor(
    _ input: TonSccpFullLightClientAuditProofInput,
    role: TonSccpFullLightClientAuditRole
) throws -> Data {
    let value = try tonNormalizeFullLightClientAuditInput(input, role: role)
    try tonRequireFullLightClientAuditRequestHashSeparation(value)
    let profile = tonAuditRoleProfile(role)
    let shardState = value.shardState
    var out = Data()
    out.append(1)
    out.append(profile.code)
    try tonAppendString(profile.circuitId, field: "circuitId", to: &out)
    try tonAppendString(tonFullLightClientAuditFastpqParameterSetV1, field: "parameterSet", to: &out)
    tonAppendI32Le(Int32(tonMainnetGlobalId), to: &out)
    tonAppendU32Le(shardState.sourceDomain, to: &out)
    try tonAppendString("verifier_id", field: "schemaField", to: &out)
    try tonAppendString(profile.verifierId, field: "verifierId", to: &out)
    try tonAppendString("verifier_hash", field: "schemaField", to: &out)
    try out.append(tonBytesFromHex32(value.verifierHash, field: "verifierHash"))
    try tonAppendString("source_verifier_material_hash", field: "schemaField", to: &out)
    try out.append(tonBytesFromHex32(value.sourceVerifierMaterialHash, field: "sourceVerifierMaterialHash"))
    try tonAppendString("source_adapter_deployment_hash", field: "schemaField", to: &out)
    try out.append(tonBytesFromHex32(value.sourceAdapterDeploymentHash, field: "sourceAdapterDeploymentHash"))
    try tonAppendString("full_light_client_gate_hash", field: "schemaField", to: &out)
    try out.append(tonBytesFromHex32(value.fullLightClientGateHash, field: "fullLightClientGateHash"))
    for requiredInput in [
        "role",
        "source_domain",
        "masterchain_seqno",
        "masterchain_block_hash",
        "shard_seqno",
        "shard_block_hash",
        "audit_statement_hash",
        "source_verifier_material_hash",
        "source_adapter_deployment_hash",
        "full_light_client_gate_hash",
        "verifier_hash",
    ] + profile.requiredInputNames {
        try tonAppendString(requiredInput, field: "requiredInput", to: &out)
    }
    return out
}

/// Build an OpenVerify request for one TON full light-client audit role.
public func buildTonSccpFullLightClientAuditProofRequest(
    _ input: TonSccpFullLightClientAuditProofInput,
    role: TonSccpFullLightClientAuditRole
) throws -> TonSccpFullLightClientAuditProofRequest {
    let value = try tonNormalizeFullLightClientAuditInput(input, role: role)
    let profile = tonAuditRoleProfile(role)
    let shardState = value.shardState
    let statementBytes = try canonicalTonSccpFullLightClientAuditStatementBytes(input, role: role)
    let statementHash = try tonSccpFullLightClientAuditStatementHash(input, role: role)
    let contextBytes = try tonCanonicalFullLightClientAuditContextBytes(value, statementHash: statementHash)
    let transitions = [
        TonSccpFullLightClientAuditFastpqTransition(
            key: "0x" + tonFullLightClientAuditFastpqKey(
                tonFullLightClientAuditFastpqStatementKeyV1,
                profile: profile
            ).hexEncodedString(),
            operation: "meta_set",
            oldValue: "0x",
            newValue: "0x" + statementBytes.hexEncodedString()
        ),
        TonSccpFullLightClientAuditFastpqTransition(
            key: "0x" + tonFullLightClientAuditFastpqKey(
                tonFullLightClientAuditFastpqContextKeyV1,
                profile: profile
            ).hexEncodedString(),
            operation: "meta_set",
            oldValue: "0x",
            newValue: "0x" + contextBytes.hexEncodedString()
        ),
        TonSccpFullLightClientAuditFastpqTransition(
            key: "0x" + tonFullLightClientAuditFastpqKey(
                tonFullLightClientAuditFastpqGateKeyV1,
                profile: profile
            ).hexEncodedString(),
            operation: "meta_set",
            oldValue: "0x",
            newValue: value.fullLightClientGateHash
        ),
    ].sorted { $0.key < $1.key }
    return TonSccpFullLightClientAuditProofRequest(
        version: 1,
        proofFamily: tonStarkFriProofFamilyV1,
        circuitId: profile.circuitId,
        parameterSet: tonFullLightClientAuditFastpqParameterSetV1,
        role: profile.name,
        roleCode: profile.code,
        sourceDomain: sccpDomainTon,
        masterchainSeqno: String(shardState.masterchainSeqno),
        shardSeqno: String(shardState.shardSeqno),
        verifierId: profile.verifierId,
        verifierHash: value.verifierHash,
        sourceStateVerifierId: shardState.sourceStateVerifierId,
        sourceStateVerifierHash: shardState.sourceStateVerifierHash,
        sourceVerifierMaterialHash: value.sourceVerifierMaterialHash,
        sourceAdapterDeploymentHash: value.sourceAdapterDeploymentHash,
        fullLightClientGateHash: value.fullLightClientGateHash,
        shardStateProofPublicInputsHash: value.shardStateProofPublicInputsHash,
        shardStateVerificationProofHash: value.shardStateVerificationProofHash,
        auditStatementHash: statementHash,
        statementBytes: statementBytes,
        verificationContextBytes: contextBytes,
        schemaDescriptor: try tonSccpFullLightClientAuditOpenVerifySchemaDescriptor(input, role: role),
        publicInputColumns: try tonSccpFullLightClientAuditPublicInputColumns(input, role: role),
        fastpqPublicInputs: try tonFullLightClientAuditFastpqPublicInputs(value, statementHash: statementHash),
        fastpqTransitions: transitions
    )
}

/// Build the TON masterchain-config audit OpenVerify request.
public func buildTonSccpMasterchainConfigProofRequest(
    _ input: TonSccpFullLightClientAuditProofInput
) throws -> TonSccpFullLightClientAuditProofRequest {
    try buildTonSccpFullLightClientAuditProofRequest(input, role: .masterchainConfig)
}

/// Build the TON validator-set transition audit OpenVerify request.
public func buildTonSccpValidatorSetTransitionProofRequest(
    _ input: TonSccpFullLightClientAuditProofInput
) throws -> TonSccpFullLightClientAuditProofRequest {
    try buildTonSccpFullLightClientAuditProofRequest(input, role: .validatorSetTransition)
}

/// Build the TON shard-accounts dictionary audit OpenVerify request.
public func buildTonSccpShardAccountsDictionaryProofRequest(
    _ input: TonSccpFullLightClientAuditProofInput
) throws -> TonSccpFullLightClientAuditProofRequest {
    try buildTonSccpFullLightClientAuditProofRequest(input, role: .shardAccountsDictionary)
}

/// Build all role-separated TON full light-client audit OpenVerify requests.
public func buildTonSccpFullLightClientAuditProofRequests(
    _ input: TonSccpFullLightClientAuditProofInput
) throws -> TonSccpFullLightClientAuditProofRequests {
    try TonSccpFullLightClientAuditProofRequests(
        masterchainConfig: buildTonSccpMasterchainConfigProofRequest(input),
        validatorSetTransition: buildTonSccpValidatorSetTransitionProofRequest(input),
        shardAccountsDictionary: buildTonSccpShardAccountsDictionaryProofRequest(input)
    )
}

/// Deterministic TON query id derived from the SCCP message id.
public func tonSccpSubmissionQueryId(_ publicInputs: TonSccpPublicInputsInput) throws -> UInt64 {
    let messageId = try tonBytesFromHex32(publicInputs.messageId, field: "messageId")
    return messageId.prefix(8).reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
}

private func normalizeTonSubmissionDestinationBinding(
    _ binding: TonSccpSubmissionDestinationBindingInput,
    field: String
) throws -> (key: String, bindingHash: String) {
    (
        key: try tonNormalizeNonEmptyString(binding.key, field: "\(field).key"),
        bindingHash: try tonNormalizeNonZeroHex32(
            binding.bindingHash,
            field: "\(field).bindingHash"
        )
    )
}

/// Return canonical TON submission metadata bytes for mobile wallet packaging.
public func canonicalTonSccpSubmissionMetadataBytes(
    _ input: TonSccpSubmissionMetadataInput
) throws -> Data {
    let manifest = input.manifest
    guard manifest.version == 1 else {
        throw TonSccpProverError.invalidField("manifest.version")
    }
    guard manifest.localDomain == sccpDomainSora else {
        throw TonSccpProverError.invalidField("manifest.localDomain")
    }
    guard manifest.counterpartyDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidField("manifest.counterpartyDomain")
    }
    guard manifest.securityModel == "RecursiveZk" else {
        throw TonSccpProverError.invalidField("securityModel")
    }
    guard manifest.anchorGovernance == "CryptographicProof" else {
        throw TonSccpProverError.invalidField("anchorGovernance")
    }
    guard manifest.verifierTarget == "TonContract" else {
        throw TonSccpProverError.invalidField("verifierTarget")
    }
    guard manifest.verifierBackendFamily == "TonContract" else {
        throw TonSccpProverError.invalidField("verifierBackendFamily")
    }
    guard manifest.proofFamily == sccpTonStarkFriProofFamilyV1 else {
        throw TonSccpProverError.invalidField("proofFamily")
    }
    guard manifest.verifierBackendKey == sccpTonContractProofBackendV1 else {
        throw TonSccpProverError.invalidField("verifierBackendKey")
    }
    guard input.publicInputs.targetDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidField("publicInputs.targetDomain")
    }
    let resolvedBinding = input.destinationBinding ?? manifest.destinationBinding
    guard let resolvedBinding else {
        throw TonSccpProverError.invalidField("destinationBinding")
    }
    let destinationBinding = try normalizeTonSubmissionDestinationBinding(
        resolvedBinding,
        field: "destinationBinding"
    )
    if let manifestBinding = manifest.destinationBinding, let explicitBinding = input.destinationBinding {
        let normalizedManifestBinding = try normalizeTonSubmissionDestinationBinding(
            manifestBinding,
            field: "manifest.destinationBinding"
        )
        let normalizedExplicitBinding = try normalizeTonSubmissionDestinationBinding(
            explicitBinding,
            field: "destinationBinding"
        )
        guard normalizedManifestBinding.key == normalizedExplicitBinding.key,
              normalizedManifestBinding.bindingHash == normalizedExplicitBinding.bindingHash else {
            throw TonSccpProverError.invalidField("destinationBinding")
        }
    }
    if let destinationBindingHash = input.destinationBindingHash {
        let normalizedDestinationBindingHash = try tonNormalizeNonZeroHex32(
            destinationBindingHash,
            field: "destinationBindingHash"
        )
        guard normalizedDestinationBindingHash == destinationBinding.bindingHash else {
            throw TonSccpProverError.invalidField("destinationBindingHash")
        }
    }
    let statementHash = try tonNonZeroBytesFromHex32(
        input.statementHash,
        field: "statementHash"
    )

    var out = Data([1])
    tonAppendU32Le(manifest.localDomain, to: &out)
    tonAppendU32Le(manifest.counterpartyDomain, to: &out)
    out.append(1)
    out.append(1)
    out.append(3)
    out.append(3)
    try tonAppendString(manifest.proofFamily, field: "proofFamily", to: &out)
    try tonAppendString(manifest.verifierBackendKey, field: "verifierBackendKey", to: &out)
    try tonAppendString(manifest.messageBackend, field: "messageBackend", to: &out)
    try tonAppendString(manifest.registryBackend, field: "registryBackend", to: &out)
    try tonAppendString(manifest.manifestSeed, field: "manifestSeed", to: &out)
    try tonAppendString(destinationBinding.key, field: "destinationBinding.key", to: &out)
    out.append(try tonBytesFromHex32(destinationBinding.bindingHash, field: "destinationBinding.bindingHash"))
    out.append(statementHash)
    out.append(try canonicalTonSccpPublicInputsBytes(input.publicInputs))
    return out
}

/// Build the TON BOC internal message body carrying an SCCP proof submission.
public func buildTonSccpMessageBodyBoc(_ input: TonSccpMessageBodyInput) throws -> Data {
    let proofResult = try requireWrappedTonProofResultForSubmission(input.proofResult)
    guard input.publicInputs == proofResult.publicInputs else {
        throw TonSccpProverError.invalidField("publicInputs")
    }
    guard input.proofBytes == proofResult.proofBytes else {
        throw TonSccpProverError.invalidField("proofBytes")
    }
    guard input.bundleBytes == proofResult.bundleBytes else {
        throw TonSccpProverError.invalidField("bundleBytes")
    }
    guard input.statementHash == proofResult.proofContext.statementHash else {
        throw TonSccpProverError.invalidField("statementHash")
    }
    guard input.destinationBindingHash == proofResult.proofContext.destinationBindingHash else {
        throw TonSccpProverError.invalidField("destinationBindingHash")
    }
    guard input.publicInputs.targetDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidField("publicInputs.targetDomain")
    }
    let publicInputsBytes = try canonicalTonSccpPublicInputsBytes(input.publicInputs)
    let statementHash = try tonNonZeroBytesFromHex32(input.statementHash, field: "statementHash")
    let destinationBindingHash = try tonNonZeroBytesFromHex32(
        input.destinationBindingHash,
        field: "destinationBindingHash"
    )
    guard !input.proofBytes.isEmpty else {
        throw TonSccpProverError.emptyProof
    }
    guard input.proofBytes.count <= sccpNativeRecursiveMaxProofBytes else {
        throw TonSccpProverError.invalidField("proofBytes")
    }
    guard input.proofBytes.contains(where: { $0 != 0 }) else {
        throw TonSccpProverError.allZeroProof
    }
    let bundleBytes = try requireTonNativeRecursivePayloadBytes(input.bundleBytes, field: "bundleBytes")
    let queryId: UInt64
    if let suppliedQueryId = input.queryId {
        queryId = suppliedQueryId
    } else {
        queryId = try tonSccpSubmissionQueryId(input.publicInputs)
    }
    var rootData = Data()
    tonAppendU32Be(0x53434350, to: &rootData)
    tonAppendU64Be(queryId, to: &rootData)
    tonAppendU16Be(1, to: &rootData)
    rootData.append(statementHash)
    rootData.append(destinationBindingHash)

    var cells = [TonCell(data: rootData, refs: [])]
    let publicInputsRoot = try tonPushSnakeCells(&cells, bytes: publicInputsBytes)
    let proofRoot = try tonPushSnakeCells(&cells, bytes: input.proofBytes)
    let bundleRoot = try tonPushSnakeCells(&cells, bytes: bundleBytes)
    let metadataRoot = try tonPushSnakeCells(&cells, bytes: input.metadataBytes)
    cells[0].refs = [publicInputsRoot, proofRoot, bundleRoot, metadataRoot]
    return try tonEncodeBocSingleRoot(cells, rootIndex: 0)
}

/// Build a TON SCCP submission envelope for wallet or liteserver broadcasting.
public func buildTonSccpSubmission(_ input: TonSccpMessageBodyInput) throws -> TonSccpSubmission {
    let messageBodyBoc = try buildTonSccpMessageBodyBoc(input)
    let messageBodyBocHex = "0x" + messageBodyBoc.hexEncodedString()
    return TonSccpSubmission(
        version: 1,
        envelopeEncoding: sccpTonMessageBodyBocV1,
        submissionKind: "internal_message",
        verifierEntrypoint: "op::submit_sccp_message_proof",
        messageBodyBoc: messageBodyBoc,
        messageBodyBocHex: messageBodyBocHex,
        arguments: [
            TonSccpSubmissionArgument(
                key: "message_body_boc",
                encoding: "ton_boc",
                bytesHex: messageBodyBocHex
            )
        ],
        envelopeBytes: messageBodyBoc,
        envelopeHex: messageBodyBocHex
    )
}

/// Build a TON SCCP proof request for a linked local prover.
public func buildTonSccpProofRequest(_ input: TonSccpProofRequestInput) throws -> TonSccpProofRequest {
    guard input.sourceDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidSourceDomain(input.sourceDomain)
    }
    guard input.backend == sccpTonContractProofBackendV1 else {
        throw TonSccpProverError.invalidBranch("backend")
    }
    guard input.publicInputs.targetDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidField("publicInputs.targetDomain")
    }
    let bundleBytes = try requireTonNativeRecursivePayloadBytes(input.bundleBytes, field: "bundleBytes")
    let sourceProofBytes = try requireTonOptionalSourceProofBytes(input.sourceProofBytes, field: "sourceProofBytes")
    guard bundleBytes.count <= Int(UInt32.max),
          sourceProofBytes.count <= Int(UInt32.max) else {
        throw TonSccpProverError.invalidField("proof byte length")
    }
    let publicInputsBytes = try canonicalTonSccpPublicInputsBytes(input.publicInputs)
    let proofContext = try normalizeTonSccpProofContext(
        statementHash: input.statementHash,
        destinationBindingHash: input.destinationBindingHash
    )
    let sourceStateVerifierId = try tonNormalizeNonEmptyString(
        input.sourceStateVerifierId,
        field: "sourceStateVerifierId"
    )
    guard sourceStateVerifierId == sccpTonMainnetShardStateVerifierIdV1 else {
        throw TonSccpProverError.invalidField("sourceStateVerifierId")
    }
    let sourceStateVerifierHashBytes = try tonNonZeroBytesFromHex32(
        input.sourceStateVerifierHash,
        field: "sourceStateVerifierHash"
    )
    try tonRejectTemplateSourceStateVerifierHash(sourceStateVerifierHashBytes)
    let sourceStateVerifierHash = "0x" + sourceStateVerifierHashBytes.hexEncodedString()
    let deploymentBinding = try normalizeTonSccpSourceAdapterDeploymentBinding(
        sourceDomain: input.sourceDomain,
        targetDomain: sccpDomainSora,
        sourceAdapterDeploymentHash: input.sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash: input.sourceAdapterDeploymentReceiptHash
    )
    guard deploymentBinding.sourceAdapterDeploymentHash != sccpZeroHashV1 else {
        throw TonSccpProverError.sourceAdapterDeploymentBindingMismatch
    }
    let deploymentBindingHash = try sccpSourceAdapterDeploymentBindingHash(deploymentBinding)
    var preimage = Data()
    preimage.append(publicInputsBytes)
    tonAppendVector(bundleBytes, to: &preimage)
    tonAppendVector(sourceProofBytes, to: &preimage)
    try tonAppendString(sourceStateVerifierId, field: "sourceStateVerifierId", to: &preimage)
    preimage.append(sourceStateVerifierHashBytes)
    try preimage.append(tonBytesFromHex32(proofContext.statementHash, field: "statementHash"))
    try preimage.append(tonBytesFromHex32(proofContext.destinationBindingHash, field: "destinationBindingHash"))
    try preimage.append(tonBytesFromHex32(deploymentBindingHash, field: "sourceAdapterDeploymentBindingHash"))
    return TonSccpProofRequest(
        version: 1,
        backend: input.backend,
        sourceDomain: input.sourceDomain,
        targetDomain: input.publicInputs.targetDomain,
        publicInputs: input.publicInputs,
        publicInputsBytes: publicInputsBytes,
        bundleBytes: bundleBytes,
        sourceProofBytes: sourceProofBytes,
        proofContext: proofContext,
        statementHash: proofContext.statementHash,
        destinationBindingHash: proofContext.destinationBindingHash,
        sourceStateVerifierId: sourceStateVerifierId,
        sourceStateVerifierHash: sourceStateVerifierHash,
        sourceAdapterDeploymentBindingHash: deploymentBindingHash,
        sourceAdapterDeploymentBinding: deploymentBinding,
        requestHash: tonHashHex(prefix: "sccp:ton:proof-request:v1", payload: preimage)
    )
}

@discardableResult
private func requireTonNativeRecursivePayloadBytes(_ bytes: Data, field: String) throws -> Data {
    guard !bytes.isEmpty else {
        throw TonSccpProverError.invalidField(field)
    }
    guard bytes.count <= sccpNativeRecursiveMaxProofBytes else {
        throw TonSccpProverError.invalidField(field)
    }
    guard bytes.contains(where: { $0 != 0 }) else {
        throw TonSccpProverError.invalidField(field)
    }
    return bytes
}

@discardableResult
private func requireTonOptionalSourceProofBytes(_ bytes: Data, field: String) throws -> Data {
    guard bytes.count <= sccpSourceStateMaxProofBytes else {
        throw TonSccpProverError.invalidField(field)
    }
    guard bytes.isEmpty || bytes.contains(where: { $0 != 0 }) else {
        throw TonSccpProverError.invalidField(field)
    }
    return bytes
}

private func normalizeTonSccpProofContext(statementHash: String,
                                          destinationBindingHash: String) throws -> TonSccpProofContext {
    TonSccpProofContext(
        version: 1,
        statementHash: try tonNormalizeNonZeroHex32(statementHash, field: "statementHash"),
        destinationBindingHash: try tonNormalizeNonZeroHex32(
            destinationBindingHash,
            field: "destinationBindingHash"
        )
    )
}

private func normalizeTonSccpSourceAdapterDeploymentBinding(
    sourceDomain: UInt32,
    targetDomain: UInt32,
    sourceAdapterDeploymentHash: String,
    sourceAdapterDeploymentReceiptHash: String
) throws -> TonSccpSourceAdapterDeploymentBinding {
    do {
        return try normalizeSccpSourceAdapterDeploymentBinding(
            sourceDomain: sourceDomain,
            targetDomain: targetDomain,
            sourceAdapterDeploymentHash: sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash: sourceAdapterDeploymentReceiptHash
        )
    } catch let error as SolanaSccpProverError {
        switch error {
        case .invalidHex32(let field):
            throw TonSccpProverError.invalidHex32(field)
        case .sourceAdapterDeploymentBindingMismatch:
            throw TonSccpProverError.sourceAdapterDeploymentBindingMismatch
        default:
            throw error
        }
    }
}

public func wrapTonSccpProofResult(proofBytes: Data,
                                   request: TonSccpProofRequest) throws -> TonSccpProofResult {
    guard !proofBytes.isEmpty else {
        throw TonSccpProverError.emptyProof
    }
    guard proofBytes.count <= sccpNativeRecursiveMaxProofBytes else {
        throw TonSccpProverError.invalidField("proofBytes")
    }
    guard proofBytes.contains(where: { $0 != 0 }) else {
        throw TonSccpProverError.allZeroProof
    }
    try requireProductionTonSccpProofRequest(request)
    var envelopePayload = try tonBytesFromHex32(request.requestHash, field: "requestHash")
    try envelopePayload.append(tonBytesFromHex32(
        request.sourceAdapterDeploymentBindingHash,
        field: "sourceAdapterDeploymentBindingHash"
    ))
    envelopePayload.append(proofBytes)
    return TonSccpProofResult(
        version: 1,
        backend: request.backend,
        proofBytes: proofBytes,
        proofBase64: proofBytes.base64EncodedString(),
        publicInputs: request.publicInputs,
        bundleBytes: request.bundleBytes,
        sourceProofBytes: request.sourceProofBytes,
        proofContext: request.proofContext,
        statementHash: request.statementHash,
        destinationBindingHash: request.destinationBindingHash,
        sourceStateVerifierId: request.sourceStateVerifierId,
        sourceStateVerifierHash: request.sourceStateVerifierHash,
        sourceAdapterDeploymentBindingHash: request.sourceAdapterDeploymentBindingHash,
        sourceAdapterDeploymentBinding: request.sourceAdapterDeploymentBinding,
        requestHash: request.requestHash,
        envelopeHash: tonHashHex(prefix: "sccp:ton:proof-envelope:v1", payload: envelopePayload)
    )
}

private func requireWrappedTonProofResultForSubmission(
    _ proofResult: TonSccpProofResult
) throws -> TonSccpProofResult {
    guard proofResult.backend == sccpTonContractProofBackendV1 else {
        throw TonSccpProverError.invalidField("proofResult.backend")
    }
    let expectedProofContext = try normalizeTonSccpProofContext(
        statementHash: proofResult.statementHash,
        destinationBindingHash: proofResult.destinationBindingHash
    )
    guard proofResult.proofContext == expectedProofContext else {
        throw TonSccpProverError.invalidField("proofResult.proofContext")
    }
    guard proofResult.publicInputs.targetDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidField("proofResult.publicInputs.targetDomain")
    }
    guard !proofResult.proofBytes.isEmpty else {
        throw TonSccpProverError.emptyProof
    }
    guard proofResult.proofBytes.count <= sccpNativeRecursiveMaxProofBytes else {
        throw TonSccpProverError.invalidField("proofResult.proofBytes")
    }
    guard proofResult.proofBytes.contains(where: { $0 != 0 }) else {
        throw TonSccpProverError.allZeroProof
    }
    guard proofResult.proofBase64 == proofResult.proofBytes.base64EncodedString() else {
        throw TonSccpProverError.invalidField("proofResult.proofBase64")
    }
    let sourceStateVerifierId = try tonNormalizeNonEmptyString(
        proofResult.sourceStateVerifierId,
        field: "proofResult.sourceStateVerifierId"
    )
    guard sourceStateVerifierId == sccpTonMainnetShardStateVerifierIdV1 else {
        throw TonSccpProverError.invalidField("proofResult.sourceStateVerifierId")
    }
    let sourceStateVerifierHashBytes = try tonNonZeroBytesFromHex32(
        proofResult.sourceStateVerifierHash,
        field: "proofResult.sourceStateVerifierHash"
    )
    try tonRejectTemplateSourceStateVerifierHash(sourceStateVerifierHashBytes)
    let requestHash = try tonNormalizeHex32(proofResult.requestHash, field: "proofResult.requestHash")
    guard requestHash != sccpZeroHashV1 else {
        throw TonSccpProverError.invalidHex32("proofResult.requestHash")
    }
    let sourceAdapterDeploymentBindingHash = try tonNormalizeHex32(
        proofResult.sourceAdapterDeploymentBindingHash,
        field: "proofResult.sourceAdapterDeploymentBindingHash"
    )
    guard sourceAdapterDeploymentBindingHash != sccpZeroHashV1 else {
        throw TonSccpProverError.invalidHex32("proofResult.sourceAdapterDeploymentBindingHash")
    }
    let deploymentBinding = try normalizeTonSccpSourceAdapterDeploymentBinding(
        sourceDomain: proofResult.sourceAdapterDeploymentBinding.sourceDomain,
        targetDomain: proofResult.sourceAdapterDeploymentBinding.targetDomain,
        sourceAdapterDeploymentHash: proofResult.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash: proofResult.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash
    )
    guard deploymentBinding.sourceDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidField("proofResult.sourceAdapterDeploymentBinding.sourceDomain")
    }
    guard deploymentBinding.targetDomain == sccpDomainSora else {
        throw TonSccpProverError.invalidField("proofResult.sourceAdapterDeploymentBinding.targetDomain")
    }
    guard deploymentBinding.sourceAdapterDeploymentHash != sccpZeroHashV1 else {
        throw TonSccpProverError.invalidField("proofResult.sourceAdapterDeploymentBinding")
    }
    guard try sccpSourceAdapterDeploymentBindingHash(deploymentBinding) == sourceAdapterDeploymentBindingHash else {
        throw TonSccpProverError.invalidField("proofResult.sourceAdapterDeploymentBindingHash")
    }
    let envelopeHash = try tonNormalizeHex32(proofResult.envelopeHash, field: "proofResult.envelopeHash")
    guard envelopeHash != sccpZeroHashV1 else {
        throw TonSccpProverError.invalidHex32("proofResult.envelopeHash")
    }
    var envelopePayload = try tonBytesFromHex32(requestHash, field: "proofResult.requestHash")
    try envelopePayload.append(tonBytesFromHex32(
        sourceAdapterDeploymentBindingHash,
        field: "proofResult.sourceAdapterDeploymentBindingHash"
    ))
    envelopePayload.append(proofResult.proofBytes)
    guard envelopeHash == tonHashHex(prefix: "sccp:ton:proof-envelope:v1", payload: envelopePayload) else {
        throw TonSccpProverError.invalidField("proofResult.envelopeHash")
    }
    try requireTonOptionalSourceProofBytes(proofResult.sourceProofBytes, field: "proofResult.sourceProofBytes")
    try requireTonNativeRecursivePayloadBytes(proofResult.bundleBytes, field: "proofResult.bundleBytes")
    let expectedRequest = try buildTonSccpProofRequest(TonSccpProofRequestInput(
        publicInputs: proofResult.publicInputs,
        bundleBytes: proofResult.bundleBytes,
        sourceProofBytes: proofResult.sourceProofBytes,
        statementHash: proofResult.statementHash,
        destinationBindingHash: proofResult.destinationBindingHash,
        sourceStateVerifierId: sourceStateVerifierId,
        sourceStateVerifierHash: proofResult.sourceStateVerifierHash,
        sourceAdapterDeploymentHash: deploymentBinding.sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash: deploymentBinding.sourceAdapterDeploymentReceiptHash,
        backend: proofResult.backend,
        sourceDomain: sccpDomainTon
    ))
    guard expectedRequest.requestHash == requestHash else {
        throw TonSccpProverError.invalidField("proofResult.requestHash")
    }
    return proofResult
}

private func requireCanonicalTonSccpProofRequest(_ request: TonSccpProofRequest) throws {
    let expected = try buildTonSccpProofRequest(TonSccpProofRequestInput(
        publicInputs: request.publicInputs,
        bundleBytes: request.bundleBytes,
        sourceProofBytes: request.sourceProofBytes,
        statementHash: request.statementHash,
        destinationBindingHash: request.destinationBindingHash,
        sourceStateVerifierId: request.sourceStateVerifierId,
        sourceStateVerifierHash: request.sourceStateVerifierHash,
        sourceAdapterDeploymentHash: request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash,
        sourceAdapterDeploymentReceiptHash: request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash,
        backend: request.backend,
        sourceDomain: request.sourceDomain
    ))
    guard expected == request else {
        throw TonSccpProverError.invalidField("request")
    }
}

private func requireProductionTonSccpProofRequest(_ request: TonSccpProofRequest) throws {
    try requireCanonicalTonSccpProofRequest(request)
    guard request.version == 1 else {
        throw TonSccpProverError.invalidField("request.version")
    }
    guard request.sourceDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidSourceDomain(request.sourceDomain)
    }
    guard request.targetDomain == sccpDomainTon,
          request.publicInputs.targetDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidField("request.targetDomain")
    }
    guard request.backend == sccpTonContractProofBackendV1 else {
        throw TonSccpProverError.invalidBranch("request.backend")
    }
    try requireTonNativeRecursivePayloadBytes(request.bundleBytes, field: "request.bundleBytes")
    try requireTonOptionalSourceProofBytes(request.sourceProofBytes, field: "request.sourceProofBytes")
    guard request.sourceStateVerifierId == sccpTonMainnetShardStateVerifierIdV1 else {
        throw TonSccpProverError.invalidField("request.sourceStateVerifierId")
    }
    let sourceStateVerifierHash = try tonNonZeroBytesFromHex32(
        request.sourceStateVerifierHash,
        field: "request.sourceStateVerifierHash"
    )
    try tonRejectTemplateSourceStateVerifierHash(sourceStateVerifierHash)
    let deploymentBinding = request.sourceAdapterDeploymentBinding
    guard deploymentBinding.sourceDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidField("request.sourceAdapterDeploymentBinding.sourceDomain")
    }
    guard deploymentBinding.targetDomain == sccpDomainSora else {
        throw TonSccpProverError.invalidField("request.sourceAdapterDeploymentBinding.targetDomain")
    }
    guard deploymentBinding.sourceAdapterDeploymentHash != sccpZeroHashV1 else {
        throw TonSccpProverError.invalidField("request.sourceAdapterDeploymentBinding")
    }
    guard try sccpSourceAdapterDeploymentBindingHash(deploymentBinding)
        == request.sourceAdapterDeploymentBindingHash else {
        throw TonSccpProverError.invalidField("request.sourceAdapterDeploymentBindingHash")
    }
}

private struct TonNormalizedValidatorSetTransitionProof {
    let version: UInt8
    let sourceDomain: UInt32
    let fromValidatorSetSeqno: UInt64
    let toValidatorSetSeqno: UInt64
    let masterchainSeqno: UInt64
    let masterchainWorkchainId: Int32
    let masterchainShard: UInt64
    let masterchainBlockHash: String
    let masterchainFileHash: String
    let parentValidatorSetHash: String
    let nextValidatorSetHash: String
    let nextValidatorSetPayload: Data
    let nextValidatorSetPayloadHash: String
    let nextValidatorSetConfigHash: String
    let transitionMessageHash: String
    let transitionSignatureHash: String
    let validatorSignatureProof: TonValidatorSignatureProofInput
}

private struct TonNormalizedShardStateSourceStateInput {
    let version: UInt8
    let sourceDomain: UInt32
    let masterchainSeqno: UInt64
    let masterchainWorkchainId: Int32
    let masterchainShard: UInt64
    let masterchainBlockHash: String
    let masterchainFileHash: String
    let validatorSetHash: String
    let masterchainConfigRoot: String
    let masterchainConfigProofHash: String
    let shardWorkchainId: Int32
    let shardShard: UInt64
    let shardSeqno: UInt64
    let shardBlockHash: String
    let shardFileHash: String
    let shardStateRoot: String
    let transactionRoot: String
    let transactionLt: UInt64
    let shardStateDictionaryRoot: String
    let shardStateDictionaryKeyBitLen: UInt16
    let shardStateDictionaryKey: Data
    let masterchainSignatureHash: String
    let shardProofHash: String
    let shardStateProofBoc: Data
    let shardStateDictionaryProofBoc: Data
    let configDictionaryProofBoc: Data
    let shardStateProofBocHash: String
    let shardAccountsProofBocHash: String
    let configProofBocHash: String
    let validatorSetTransitionProofs: [TonNormalizedValidatorSetTransitionProof]
    let transitionChainHash: String
    let sourceStateVerifierId: String
    let sourceStateVerifierHash: String
    let sourceTrustAnchorId: String
    let sourceTrustAnchorHash: String
    let consensusVerifierId: String
    let consensusVerifierHash: String
    let messageInclusionVerifierId: String
    let messageInclusionVerifierHash: String
    let finalityPolicyId: String
    let finalityPolicyHash: String
}

private struct TonSccpFullLightClientAuditRoleProfile {
    let name: String
    let code: UInt8
    let circuitId: String
    let verifierId: String
    let requiredInputNames: [String]
}

private struct TonNormalizedFullLightClientAuditInput {
    let role: TonSccpFullLightClientAuditRole
    let shardState: TonNormalizedShardStateSourceStateInput
    let sourceVerifierMaterialHash: String
    let sourceAdapterDeploymentHash: String
    let fullLightClientGateHash: String
    let verifierHash: String
    let shardStateProofPublicInputsHash: String
    let shardStateVerificationProofHash: String
    let validatorSetPayloadHash: String
    let configLeafHash: String
    let configValueHash: String
}

private struct TonCell {
    var data: Data
    var refs: [Int]
}

private struct TonBocCell {
    let descriptor: UInt8
    let dataDescriptor: UInt8
    let data: Data
    let refs: [Int]
    let level: UInt8
    let exotic: Bool
}

private enum TonBocCellKind: Equatable {
    case ordinary
    case prunedBranch
    case merkleProof
    case merkleUpdate
}

private struct TonBocPrunedBranch {
    let mask: UInt8
    let hashes: [Data]
    let depths: [UInt16]
}

private struct TonBocComputedCell {
    var mask: UInt8
    var hashes: [Data]
    var depths: [UInt16]
}

private struct TonValidatorDescr {
    let key: UInt16
    let publicKey: Data
    let weight: UInt64
}

private struct TonBocBitReader {
    private let cell: TonBocCell
    private let bitLength: Int
    private var bitOffset = 0
    private var refOffset = 0

    init(cell: TonBocCell) throws {
        self.cell = cell
        self.bitLength = try tonCellSerializedBitLength(dataDescriptor: cell.dataDescriptor, data: cell.data)
    }

    mutating func readBit() throws -> Bool {
        guard bitOffset < bitLength else {
            throw TonSccpProverError.invalidBoc("hashmapBits")
        }
        let byte = cell.data[bitOffset / 8]
        let bit = ((byte >> UInt8(7 - (bitOffset % 8))) & 1) != 0
        bitOffset += 1
        return bit
    }

    mutating func readUInt(bits: Int) throws -> Int {
        var value = 0
        for _ in 0..<bits {
            let bit = try readBit()
            value = (value << 1) | (bit ? 1 : 0)
        }
        return value
    }

    mutating func readUInt64(bits: Int) throws -> UInt64 {
        var value: UInt64 = 0
        for _ in 0..<bits {
            let bit = try readBit()
            value = (value << 1) | (bit ? 1 : 0)
        }
        return value
    }

    mutating func readData(byteCount: Int) throws -> Data {
        var out = Data()
        out.reserveCapacity(byteCount)
        for _ in 0..<byteCount {
            out.append(UInt8(try readUInt(bits: 8)))
        }
        return out
    }

    mutating func skipBits(_ bits: Int) throws {
        guard bits >= 0, bitOffset + bits <= bitLength else {
            throw TonSccpProverError.invalidBoc("cellBits")
        }
        bitOffset += bits
    }

    mutating func readRef() throws -> Int {
        guard refOffset < cell.refs.count else {
            throw TonSccpProverError.invalidBoc("hashmapRefs")
        }
        let ref = cell.refs[refOffset]
        refOffset += 1
        return ref
    }

    var remainingBits: Int {
        bitLength - bitOffset
    }

    var remainingRefs: Int {
        cell.refs.count - refOffset
    }

    var isExhausted: Bool {
        remainingBits == 0 && remainingRefs == 0
    }
}

private let tonBocMagic = Data([0xb5, 0xee, 0x9c, 0x72])
private let tonMaxCellDataBytes = 127
private let tonMaxCellSerializedDataBytes = 128
private let tonMaxBocBytes = 64 * 1024
private let tonMaxBocCells = 4096
private let tonMaxRefs = 4
private let tonMaxValidators = 1024
private let tonShardAccountKeyBits = 256
private let tonValidatorSetKeyBits = 16
private let tonValidatorConstructor = 0x53
private let tonValidatorAddrConstructor = 0x73
private let tonValidatorsConstructor = 0x11
private let tonValidatorsExtConstructor = 0x12
private let tonEd25519PubkeyConstructor = 0x8e81_278a

/// Return root representation hashes for a bounded complete TON BoC.
public func tonBocRootHashes(_ boc: Data) throws -> [String] {
    let parsed = try tonParseBocCompleteOrdinary(boc)
    let hashes = try tonBocCellHashes(parsed.cells)
    return try parsed.roots.map { root in
        guard root >= 0, root < hashes.count else {
            throw TonSccpProverError.invalidBoc("root")
        }
        return "0x" + hashes[root].hashes[3].hexEncodedString()
    }
}

/// Return the single root representation hash for a bounded complete TON BoC.
public func tonBocSingleRootHash(_ boc: Data) throws -> String {
    let roots = try tonBocRootHashes(boc)
    guard roots.count == 1, let root = roots.first else {
        throw TonSccpProverError.invalidBoc("rootCount")
    }
    return root
}

private let tonShardStateUnsplitTag = 0x9023afe2
private let tonMainnetGlobalId = -239
private func tonSignedInt32(from raw: Int) -> Int {
    raw >= (1 << 31) ? raw - (1 << 32) : raw
}

private func tonBocProofRootAndChildIndex(
    parsed: (roots: [Int], cells: [TonBocCell]),
    computed: [TonBocComputedCell]
) throws -> (rootHash: Data, childIndex: Int) {
    guard parsed.roots.count == 1, let rootIndex = parsed.roots.first else {
        throw TonSccpProverError.invalidBoc("rootCount")
    }
    guard rootIndex >= 0, rootIndex < parsed.cells.count, rootIndex < computed.count else {
        throw TonSccpProverError.invalidBoc("root")
    }
    let root = parsed.cells[rootIndex]
    switch try tonBocCellKind(root) {
    case .ordinary:
        return (computed[rootIndex].hashes[3], rootIndex)
    case .merkleProof:
        guard root.refs.count == 1, root.data.count >= 33 else {
            throw TonSccpProverError.invalidBoc("merkleProof")
        }
        return (Data(root.data[1..<33]), root.refs[0])
    case .prunedBranch, .merkleUpdate:
        throw TonSccpProverError.invalidBoc("shardStateRoot")
    }
}

/// Return the committed `ShardStateUnsplit` root hash from a TON shard-state proof BoC.
public func tonShardStateProofRootHash(_ boc: Data) throws -> String {
    let parsed = try tonParseBocCompleteOrdinary(boc)
    let computed = try tonBocCellHashes(parsed.cells)
    let root = try tonBocProofRootAndChildIndex(parsed: parsed, computed: computed)
    return "0x" + root.rootHash.hexEncodedString()
}

/// Return the committed root hash from a bounded TON `HashmapE` proof BoC.
public func tonHashmapEProofRootHash(_ boc: Data) throws -> String {
    let parsed = try tonParseBocCompleteOrdinary(boc)
    let computed = try tonBocCellHashes(parsed.cells)
    let root = try tonBocProofRootAndChildIndex(parsed: parsed, computed: computed)
    return "0x" + root.rootHash.hexEncodedString()
}

private struct TonShardStateAccountsOpening {
    let accountsRootHash: String
    let globalId: Int
    let workchainId: Int
    let seqNo: Int
    let genUtime: Int
    let genLt: UInt64
    let minRefMcSeqno: Int
    let shardPfxBits: Int
    let shardPrefixBits: [Bool]
    let shardId: UInt64
}

private func tonShardStateAccountKeyMatchesShardPrefix(
    key: Data,
    keyBitLen: Int,
    opening: TonShardStateAccountsOpening
) throws -> Bool {
    guard keyBitLen == tonShardAccountKeyBits else {
        return false
    }
    for bitIndex in 0..<opening.shardPfxBits {
        if try tonHashmapKeyBit(key: key, keyBitLen: keyBitLen, bitIndex: bitIndex) != opening.shardPrefixBits[bitIndex] {
            return false
        }
    }
    return true
}

private func tonShardIdFromPrefixBits(shardPfxBits: Int, shardPrefixBits: [Bool]) throws -> UInt64 {
    guard shardPfxBits >= 0, shardPfxBits <= 60 else {
        throw TonSccpProverError.invalidBoc("shardIdentPrefix")
    }
    var shardId: UInt64 = 0
    for bitIndex in 0..<shardPfxBits where shardPrefixBits[bitIndex] {
        shardId |= UInt64(1) << UInt64(63 - bitIndex)
    }
    shardId |= UInt64(1) << UInt64(63 - shardPfxBits)
    return shardId
}

private func tonShardStateUnsplitAccountsOpeningFromCell(
    parsed: (roots: [Int], cells: [TonBocCell]),
    computed: [TonBocComputedCell],
    cellIndex: Int
) throws -> TonShardStateAccountsOpening {
    guard cellIndex >= 0, cellIndex < parsed.cells.count else {
        throw TonSccpProverError.invalidBoc("shardStateCell")
    }
    let cell = parsed.cells[cellIndex]
    guard try tonBocCellKind(cell) == .ordinary else {
        throw TonSccpProverError.invalidBoc("shardStateCell")
    }
    var reader = try TonBocBitReader(cell: cell)
    guard try reader.readUInt(bits: 32) == tonShardStateUnsplitTag else {
        throw TonSccpProverError.invalidBoc("shardStateTag")
    }
    let globalId = tonSignedInt32(from: try reader.readUInt(bits: 32))
    guard try reader.readUInt(bits: 2) == 0 else {
        throw TonSccpProverError.invalidBoc("shardIdentTag")
    }
    let shardPfxBits = try reader.readUInt(bits: 6)
    guard shardPfxBits <= 60 else {
        throw TonSccpProverError.invalidBoc("shardIdentPrefix")
    }
    let workchainId = tonSignedInt32(from: try reader.readUInt(bits: 32))
    var shardPrefixBits: [Bool] = []
    shardPrefixBits.reserveCapacity(64)
    for _ in 0..<64 {
        shardPrefixBits.append(try reader.readBit())
    }
    let seqNo = try reader.readUInt(bits: 32)
    _ = try reader.readUInt(bits: 32)
    let genUtime = try reader.readUInt(bits: 32)
    let genLt = try reader.readUInt64(bits: 64)
    let minRefMcSeqno = try reader.readUInt(bits: 32)
    let outMsgQueueInfoRef = try reader.readRef()
    guard outMsgQueueInfoRef >= 0, outMsgQueueInfoRef < computed.count else {
        throw TonSccpProverError.invalidBoc("outMsgQueueInfo")
    }
    _ = try reader.readBit()
    let accountsRef = try reader.readRef()
    guard accountsRef >= 0, accountsRef < computed.count else {
        throw TonSccpProverError.invalidBoc("accounts")
    }
    let trailingFieldsRef = try reader.readRef()
    guard trailingFieldsRef >= 0, trailingFieldsRef < computed.count else {
        throw TonSccpProverError.invalidBoc("shardStateTrailing")
    }
    if try reader.readBit() {
        guard workchainId != Int(tonBasechainWorkchainId) else {
            throw TonSccpProverError.invalidBoc("basechainCustom")
        }
        let customRef = try reader.readRef()
        guard customRef >= 0, customRef < computed.count else {
            throw TonSccpProverError.invalidBoc("custom")
        }
    }
    guard reader.isExhausted else {
        throw TonSccpProverError.invalidBoc("shardStateTrailing")
    }
    return TonShardStateAccountsOpening(
        accountsRootHash: "0x" + computed[accountsRef].hashes[3].hexEncodedString(),
        globalId: globalId,
        workchainId: workchainId,
        seqNo: seqNo,
        genUtime: genUtime,
        genLt: genLt,
        minRefMcSeqno: minRefMcSeqno,
        shardPfxBits: shardPfxBits,
        shardPrefixBits: shardPrefixBits,
        shardId: try tonShardIdFromPrefixBits(
            shardPfxBits: shardPfxBits,
            shardPrefixBits: shardPrefixBits
        )
    )
}

/// Return the `accounts:^ShardAccounts` root from a TON `ShardStateUnsplit` proof BoC.
public func tonShardStateAccountsRootHash(_ boc: Data) throws -> String {
    return try tonShardStateAccountsOpening(boc).accountsRootHash
}

private func tonShardStateAccountsOpening(_ boc: Data) throws -> TonShardStateAccountsOpening {
    let parsed = try tonParseBocCompleteOrdinary(boc)
    let computed = try tonBocCellHashes(parsed.cells)
    let (_, childIndex) = try tonBocProofRootAndChildIndex(parsed: parsed, computed: computed)
    return try tonShardStateUnsplitAccountsOpeningFromCell(
        parsed: parsed,
        computed: computed,
        cellIndex: childIndex
    )
}

/// Return a TON `HashmapE n ^Cell` value-cell representation hash from a bounded proof BoC.
public func tonHashmapECellRefValueHash(_ boc: Data, key: Data, keyBitLen: Int) throws -> String? {
    guard tonHashmapKeyIsCanonical(key: key, keyBitLen: keyBitLen) else {
        throw TonSccpProverError.invalidBoc("hashmapKey")
    }
    let parsed = try tonParseBocCompleteOrdinary(boc)
    let computed = try tonBocCellHashes(parsed.cells)
    guard parsed.roots.count == 1, let root = parsed.roots.first else {
        throw TonSccpProverError.invalidBoc("rootCount")
    }
    guard let rootIndex = try tonHashmapUnwrapMerkleProofCell(cells: parsed.cells, cellIndex: root) else {
        throw TonSccpProverError.invalidBoc("hashmapRoot")
    }
    let rootCell = parsed.cells[rootIndex]
    guard try tonBocCellKind(rootCell) == .ordinary else {
        throw TonSccpProverError.invalidBoc("hashmapRoot")
    }
    var reader = try TonBocBitReader(cell: rootCell)
    let hasRoot = try reader.readBit()
    if !hasRoot {
        guard reader.isExhausted else {
            throw TonSccpProverError.invalidBoc("hashmapRoot")
        }
        return nil
    }
    guard reader.remainingBits == 0, reader.remainingRefs == 1 else {
        throw TonSccpProverError.invalidBoc("hashmapRoot")
    }
    return try tonHashmapCellRefValueHash(
        cells: parsed.cells,
        computed: computed,
        rootIndex: reader.readRef(),
        key: key,
        keyBitLen: keyBitLen
    )
}

private func tonCurrentValidatorSetConfigKey() -> Data {
    var out = Data()
    tonAppendU32Be(UInt32(sccpTonCurrentValidatorSetConfigParam), to: &out)
    return out
}

/// Decode the config-34 `ValidatorSet` from a TON config dictionary proof BoC.
public func tonConfigValidatorSetPayloadFromProofBoc(_ boc: Data) throws -> Data? {
    let parsed = try tonParseBocCompleteOrdinary(boc)
    _ = try tonBocCellHashes(parsed.cells)
    guard parsed.roots.count == 1, let root = parsed.roots.first else {
        throw TonSccpProverError.invalidBoc("rootCount")
    }
    guard let rootIndex = try tonHashmapUnwrapMerkleProofCell(cells: parsed.cells, cellIndex: root) else {
        throw TonSccpProverError.invalidBoc("configDictionaryRoot")
    }
    let rootCell = parsed.cells[rootIndex]
    guard try tonBocCellKind(rootCell) == .ordinary else {
        throw TonSccpProverError.invalidBoc("configDictionaryRoot")
    }
    var reader = try TonBocBitReader(cell: rootCell)
    let hasRoot = try reader.readBit()
    if !hasRoot {
        guard reader.isExhausted else {
            throw TonSccpProverError.invalidBoc("configDictionaryRoot")
        }
        return nil
    }
    guard reader.remainingBits == 0, reader.remainingRefs == 1 else {
        throw TonSccpProverError.invalidBoc("configDictionaryRoot")
    }
    guard let valueRef = try tonHashmapCellRefValueIndex(
        cells: parsed.cells,
        rootIndex: reader.readRef(),
        key: tonCurrentValidatorSetConfigKey(),
        keyBitLen: sccpTonConfigParamKeyBits
    ) else {
        return nil
    }
    return try tonValidatorSetPayloadFromCell(cells: parsed.cells, cellIndex: valueRef)
}

/// Decode and hash the config-34 `ValidatorSet` from a TON config dictionary proof BoC.
public func tonConfigValidatorSetPayloadHashFromProofBoc(_ boc: Data) throws -> String? {
    guard let payload = try tonConfigValidatorSetPayloadFromProofBoc(boc) else {
        return nil
    }
    return try tonValidatorSetPayloadHash(payload: payload)
}

/// Selected TON ShardAccount last transaction identity.
public struct TonShardAccountLastTransaction: Equatable {
    public let hash: String
    public let lt: UInt64

    public init(hash: String, lt: UInt64) {
        self.hash = hash
        self.lt = lt
    }
}

/// Return the selected ShardAccount last transaction hash and logical time from a TON `ShardAccounts` proof BoC.
public func tonShardAccountsLastTransaction(_ boc: Data, key: Data, keyBitLen: Int) throws -> TonShardAccountLastTransaction? {
    guard keyBitLen == tonShardAccountKeyBits else {
        throw TonSccpProverError.invalidBoc("shardAccountsKeyBits")
    }
    guard tonHashmapKeyIsCanonical(key: key, keyBitLen: keyBitLen) else {
        throw TonSccpProverError.invalidBoc("shardAccountsKey")
    }
    let parsed = try tonParseBocCompleteOrdinary(boc)
    let computed = try tonBocCellHashes(parsed.cells)
    guard parsed.roots.count == 1, let root = parsed.roots.first else {
        throw TonSccpProverError.invalidBoc("rootCount")
    }
    guard let rootIndex = try tonHashmapUnwrapMerkleProofCell(cells: parsed.cells, cellIndex: root) else {
        throw TonSccpProverError.invalidBoc("shardAccountsRoot")
    }
    let rootCell = parsed.cells[rootIndex]
    guard try tonBocCellKind(rootCell) == .ordinary else {
        throw TonSccpProverError.invalidBoc("shardAccountsRoot")
    }
    var reader = try TonBocBitReader(cell: rootCell)
    let hasRoot = try reader.readBit()
    if !hasRoot {
        guard reader.isExhausted else {
            throw TonSccpProverError.invalidBoc("shardAccountsRoot")
        }
        return nil
    }
    guard reader.remainingBits == 0, reader.remainingRefs == 1 else {
        throw TonSccpProverError.invalidBoc("shardAccountsRoot")
    }
    return try tonHashmapShardAccountsLastTransaction(
        cells: parsed.cells,
        computed: computed,
        rootIndex: reader.readRef(),
        key: key,
        keyBitLen: keyBitLen
    )
}

/// Return the selected ShardAccount last transaction hash from a TON `ShardAccounts` proof BoC.
public func tonShardAccountsLastTransactionHash(_ boc: Data, key: Data, keyBitLen: Int) throws -> String? {
    try tonShardAccountsLastTransaction(boc, key: key, keyBitLen: keyBitLen)?.hash
}

private func tonParseBocCompleteOrdinary(_ boc: Data) throws -> (roots: [Int], cells: [TonBocCell]) {
    guard boc.count >= tonBocMagic.count + 2,
          boc.count <= tonMaxBocBytes,
          boc.prefix(tonBocMagic.count) == tonBocMagic else {
        throw TonSccpProverError.invalidBoc("header")
    }
    var offset = tonBocMagic.count
    let flagsSize = boc[offset]
    offset += 1
    let hasIndex = (flagsSize & 0x80) != 0
    let hasCrc32c = (flagsSize & 0x40) != 0
    let hasCacheBits = (flagsSize & 0x20) != 0
    let flags = (flagsSize >> 3) & 0x03
    let sizeBytes = Int(flagsSize & 0x07)
    let offsetBytes = Int(boc[offset])
    offset += 1
    guard !hasCacheBits,
          flags == 0,
          (1...4).contains(sizeBytes),
          (1...8).contains(offsetBytes) else {
        throw TonSccpProverError.invalidBoc("flags")
    }

    let cellsCount = try tonReadSizedUInt(boc, offset: &offset, size: sizeBytes)
    let rootsCount = try tonReadSizedUInt(boc, offset: &offset, size: sizeBytes)
    let absentCount = try tonReadSizedUInt(boc, offset: &offset, size: sizeBytes)
    let totalCellsSize = try tonReadSizedUInt(boc, offset: &offset, size: offsetBytes)
    guard cellsCount > 0,
          cellsCount <= tonMaxBocCells,
          rootsCount > 0,
          rootsCount <= cellsCount,
          absentCount == 0,
          rootsCount + absentCount <= cellsCount else {
        throw TonSccpProverError.invalidBoc("counts")
    }

    var roots: [Int] = []
    roots.reserveCapacity(rootsCount)
    for _ in 0..<rootsCount {
        let root = try tonReadSizedUInt(boc, offset: &offset, size: sizeBytes)
        guard root < cellsCount else {
            throw TonSccpProverError.invalidBoc("root")
        }
        roots.append(root)
    }

    if hasIndex {
        var previous = 0
        for index in 0..<cellsCount {
            let cellOffset = try tonReadSizedUInt(boc, offset: &offset, size: offsetBytes)
            guard cellOffset >= previous, cellOffset <= totalCellsSize else {
                throw TonSccpProverError.invalidBoc("index")
            }
            guard index + 1 != cellsCount || cellOffset == totalCellsSize else {
                throw TonSccpProverError.invalidBoc("index")
            }
            previous = cellOffset
        }
    }

    guard totalCellsSize <= boc.count - offset else {
        throw TonSccpProverError.invalidBoc("cellDataLength")
    }
    let cellDataEnd = offset + totalCellsSize
    let expectedEnd = cellDataEnd + (hasCrc32c ? 4 : 0)
    guard expectedEnd == boc.count else {
        throw TonSccpProverError.invalidBoc("cellDataLength")
    }
    if hasCrc32c {
        var expectedCrc = Data()
        tonAppendU32Le(tonCrc32c(Data(boc[..<cellDataEnd])), to: &expectedCrc)
        guard Data(boc[cellDataEnd..<expectedEnd]) == expectedCrc else {
            throw TonSccpProverError.invalidBoc("crc32c")
        }
    }
    let cellData = Data(boc[offset..<cellDataEnd])
    var cellOffset = 0
    var cells: [TonBocCell] = []
    cells.reserveCapacity(cellsCount)
    for cellIndex in 0..<cellsCount {
        guard cellOffset + 2 <= cellData.count else {
            throw TonSccpProverError.invalidBoc("cell")
        }
        let descriptor = cellData[cellOffset]
        let dataDescriptor = cellData[cellOffset + 1]
        cellOffset += 2
        let refsCount = Int(descriptor & 0x07)
        let exotic = (descriptor & 0x08) != 0
        let hasHashes = (descriptor & 0x10) != 0
        let level = (descriptor >> 5) & 0x07
        let dataBytes = (Int(dataDescriptor) + 1) / 2
        guard refsCount <= tonMaxRefs,
              !hasHashes,
              dataBytes <= tonMaxCellSerializedDataBytes,
              cellOffset + dataBytes <= cellData.count else {
            throw TonSccpProverError.invalidBoc("cellDescriptor")
        }
        let data = Data(cellData[cellOffset..<(cellOffset + dataBytes)])
        guard tonCellDataPaddingIsValid(dataDescriptor: dataDescriptor, data: data) else {
            throw TonSccpProverError.invalidBoc("cellPadding")
        }
        cellOffset += dataBytes
        var refs: [Int] = []
        refs.reserveCapacity(refsCount)
        for _ in 0..<refsCount {
            let refIndex = try tonReadSizedUInt(cellData, offset: &cellOffset, size: sizeBytes)
            guard refIndex < cellsCount, refIndex > cellIndex else {
                throw TonSccpProverError.invalidBoc("refs")
            }
            refs.append(refIndex)
        }
        cells.append(TonBocCell(
            descriptor: descriptor & ~0x10,
            dataDescriptor: dataDescriptor,
            data: data,
            refs: refs,
            level: level,
            exotic: exotic
        ))
    }
    guard cellOffset == cellData.count else {
        throw TonSccpProverError.invalidBoc("trailingCellData")
    }
    return (roots, cells)
}

private func tonBocChildForHashLevel(
    kind: TonBocCellKind,
    computed: TonBocComputedCell,
    level: Int
) -> (hash: Data, depth: UInt16) {
    let childLevel = (kind == .merkleProof || kind == .merkleUpdate) ? level + 1 : level
    let index = min(childLevel, 3)
    return (computed.hashes[index], computed.depths[index])
}

private func tonBocCellHashes(_ cells: [TonBocCell]) throws -> [TonBocComputedCell] {
    let emptyComputed = TonBocComputedCell(
        mask: 0,
        hashes: Array(repeating: Data(repeating: 0, count: 32), count: 4),
        depths: Array(repeating: UInt16(0), count: 4)
    )
    var computed = Array(repeating: emptyComputed, count: cells.count)
    for index in stride(from: cells.count - 1, through: 0, by: -1) {
        let cell = cells[index]
        let kind = try tonBocCellKind(cell)
        let pruned = kind == .prunedBranch ? try tonParsePrunedBranch(cell) : nil
        let mask: UInt8
        switch kind {
        case .ordinary:
            var ordinaryMask = UInt8(0)
            for ref in cell.refs {
                guard ref >= 0, ref < computed.count else {
                    throw TonSccpProverError.invalidBoc("refs")
                }
                ordinaryMask |= computed[ref].mask
            }
            mask = ordinaryMask
        case .prunedBranch:
            mask = pruned!.mask
        case .merkleProof:
            guard tonCellSerializedBitLenIsByteAligned(dataDescriptor: cell.dataDescriptor, data: cell.data),
                  cell.data.count == 35,
                  cell.refs.count == 1 else {
                throw TonSccpProverError.invalidBoc("merkleProof")
            }
            let child = tonChildHashDepthForLevel(computed: computed[cell.refs[0]], level: 0)
            let proofHash = Data(cell.data[1..<33])
            let proofDepth = UInt16(cell.data[33]) << 8 | UInt16(cell.data[34])
            guard proofHash == child.hash, proofDepth == child.depth else {
                throw TonSccpProverError.invalidBoc("merkleProof")
            }
            mask = tonLevelMaskValue(computed[cell.refs[0]].mask >> 1)
        case .merkleUpdate:
            guard tonCellSerializedBitLenIsByteAligned(dataDescriptor: cell.dataDescriptor, data: cell.data),
                  cell.data.count == 69,
                  cell.refs.count == 2 else {
                throw TonSccpProverError.invalidBoc("merkleUpdate")
            }
            for (refPos, hashOffset, depthOffset) in [(0, 1, 65), (1, 33, 67)] {
                let child = tonChildHashDepthForLevel(computed: computed[cell.refs[refPos]], level: 0)
                let proofHash = Data(cell.data[hashOffset..<(hashOffset + 32)])
                let proofDepth = UInt16(cell.data[depthOffset]) << 8 | UInt16(cell.data[depthOffset + 1])
                guard proofHash == child.hash, proofDepth == child.depth else {
                    throw TonSccpProverError.invalidBoc("merkleUpdate")
                }
            }
            mask = tonLevelMaskValue((computed[cell.refs[0]].mask | computed[cell.refs[1]].mask) >> 1)
        }

        guard cell.level == mask else {
            throw TonSccpProverError.invalidBoc("level")
        }

        let totalHashCount = tonLevelMaskHashIndex(mask) + 1
        let hashCount = kind == .prunedBranch ? 1 : totalHashCount
        let hashOffset = totalHashCount - hashCount
        var computedHashes: [Data] = []
        var computedDepths: [UInt16] = []
        var hashIndex = 0
        for levelIndex in 0...Int(tonLevelMaskLevel(mask)) {
            guard tonLevelMaskIsSignificant(mask, level: levelIndex) else {
                continue
            }
            if hashIndex < hashOffset {
                hashIndex += 1
                continue
            }
            let currentData: Data
            if hashIndex == hashOffset {
                guard levelIndex == 0 || kind == .prunedBranch else {
                    throw TonSccpProverError.invalidBoc("hashLevel")
                }
                currentData = cell.data
            } else {
                currentData = computedHashes[hashIndex - hashOffset - 1]
            }

            var currentDepth = 0
            for ref in cell.refs {
                guard ref >= 0, ref < computed.count else {
                    throw TonSccpProverError.invalidBoc("refs")
                }
                let child = tonBocChildForHashLevel(kind: kind, computed: computed[ref], level: levelIndex)
                currentDepth = max(currentDepth, Int(child.depth))
            }
            if !cell.refs.isEmpty {
                currentDepth += 1
            }
            guard currentDepth <= 0xffff else {
                throw TonSccpProverError.invalidBoc("depth")
            }

            let appliedMask = tonLevelMaskApply(mask, level: levelIndex)
            let descriptor = UInt8(cell.refs.count)
                | (kind == .ordinary ? 0 : 0x08)
                | (appliedMask << 5)
            var representation = Data([descriptor, cell.dataDescriptor])
            representation.append(currentData)
            for ref in cell.refs {
                let child = tonBocChildForHashLevel(kind: kind, computed: computed[ref], level: levelIndex)
                tonAppendU16Be(child.depth, to: &representation)
            }
            for ref in cell.refs {
                let child = tonBocChildForHashLevel(kind: kind, computed: computed[ref], level: levelIndex)
                representation.append(child.hash)
            }
            computedHashes.append(Data(SHA256.hash(data: representation)))
            computedDepths.append(UInt16(currentDepth))
            hashIndex += 1
        }

        var resolvedHashes = Array(repeating: Data(repeating: 0, count: 32), count: 4)
        var resolvedDepths = Array(repeating: UInt16(0), count: 4)
        for resolvedLevel in 0..<4 {
            let resolvedHashIndex = tonLevelMaskHashIndex(
                tonLevelMaskApply(mask, level: resolvedLevel)
            )
            if let pruned {
                let thisHashIndex = tonLevelMaskHashIndex(mask)
                if resolvedHashIndex != thisHashIndex {
                    resolvedHashes[resolvedLevel] = pruned.hashes[resolvedHashIndex]
                    resolvedDepths[resolvedLevel] = pruned.depths[resolvedHashIndex]
                } else {
                    resolvedHashes[resolvedLevel] = computedHashes[0]
                    resolvedDepths[resolvedLevel] = computedDepths[0]
                }
            } else {
                resolvedHashes[resolvedLevel] = computedHashes[resolvedHashIndex]
                resolvedDepths[resolvedLevel] = computedDepths[resolvedHashIndex]
            }
        }
        computed[index] = TonBocComputedCell(mask: mask, hashes: resolvedHashes, depths: resolvedDepths)
    }
    return computed
}

private func tonReadSizedUInt(_ data: Data, offset: inout Int, size: Int) throws -> Int {
    guard (1...8).contains(size), offset + size <= data.count else {
        throw TonSccpProverError.invalidBoc("truncated")
    }
    var value = UInt64(0)
    for index in 0..<size {
        value = (value << 8) | UInt64(data[offset + index])
    }
    offset += size
    guard value <= UInt64(Int.max) else {
        throw TonSccpProverError.invalidBoc("sizedUInt")
    }
    return Int(value)
}

private func tonCrc32c(_ data: Data) -> UInt32 {
    var crc = UInt32.max
    for byte in data {
        crc ^= UInt32(byte)
        for _ in 0..<8 {
            let mask = UInt32(0) &- (crc & 1)
            crc = (crc >> 1) ^ (0x82f63b78 & mask)
        }
    }
    return ~crc
}

private func tonCellDataPaddingIsValid(dataDescriptor: UInt8, data: Data) -> Bool {
    dataDescriptor & 1 == 0 || data.last.map { $0 != 0 } == true
}

private func tonCellSerializedBitLenIsByteAligned(dataDescriptor: UInt8, data: Data) -> Bool {
    dataDescriptor & 1 == 0 && Int(dataDescriptor) / 2 == data.count
}

private func tonCellSerializedBitLength(dataDescriptor: UInt8, data: Data) throws -> Int {
    if dataDescriptor & 1 == 0 {
        let byteLength = Int(dataDescriptor) / 2
        guard byteLength == data.count else {
            throw TonSccpProverError.invalidBoc("cellDataLength")
        }
        return byteLength * 8
    }
    let fullBytes = (Int(dataDescriptor) + 1) / 2
    let floorBytes = Int(dataDescriptor) / 2
    guard fullBytes == data.count, floorBytes + 1 == fullBytes, let last = data.last, last != 0 else {
        throw TonSccpProverError.invalidBoc("cellPadding")
    }
    return floorBytes * 8 + (7 - last.trailingZeroBitCount)
}

private func tonHashmapUIntLengthBits(_ maxValue: Int) -> Int {
    var value = maxValue
    var bits = 0
    while value > 0 {
        bits += 1
        value >>= 1
    }
    return bits
}

private func tonHashmapKeyIsCanonical(key: Data, keyBitLen: Int) -> Bool {
    guard keyBitLen >= 0, keyBitLen <= 0xffff else {
        return false
    }
    let expectedBytes = (keyBitLen + 7) / 8
    guard key.count == expectedBytes else {
        return false
    }
    let unused = expectedBytes * 8 - keyBitLen
    return unused == 0 || (key.last.map { ($0 & UInt8((1 << unused) - 1)) == 0 } == true)
}

private func tonHashmapKeyBit(key: Data, keyBitLen: Int, bitIndex: Int) throws -> Bool {
    guard bitIndex < keyBitLen else {
        throw TonSccpProverError.invalidBoc("hashmapKey")
    }
    return ((key[bitIndex / 8] >> UInt8(7 - (bitIndex % 8))) & 1) != 0
}

private func tonHashmapUnwrapMerkleProofCell(cells: [TonBocCell], cellIndex: Int) throws -> Int? {
    guard cellIndex >= 0, cellIndex < cells.count else {
        throw TonSccpProverError.invalidBoc("hashmapCell")
    }
    let cell = cells[cellIndex]
    switch try tonBocCellKind(cell) {
    case .ordinary:
        return cellIndex
    case .merkleProof:
        guard cell.refs.count == 1 else {
            throw TonSccpProverError.invalidBoc("merkleProof")
        }
        return cell.refs[0]
    case .prunedBranch, .merkleUpdate:
        return nil
    }
}

private func tonHashmapReadLabel(
    reader: inout TonBocBitReader,
    key: Data,
    keyBitLen: Int,
    keyOffset: Int,
    maxLength: Int
) throws -> Int? {
    if !(try reader.readBit()) {
        var labelLength = 0
        while try reader.readBit() {
            labelLength += 1
            if labelLength > maxLength {
                return nil
            }
        }
        for offset in 0..<labelLength {
            let actual = try reader.readBit()
            let expected = try tonHashmapKeyBit(
                key: key,
                keyBitLen: keyBitLen,
                bitIndex: keyOffset + offset
            )
            guard actual == expected else {
                return nil
            }
        }
        return labelLength
    }
    if !(try reader.readBit()) {
        let labelLength = try reader.readUInt(bits: tonHashmapUIntLengthBits(maxLength))
        guard labelLength <= maxLength else {
            return nil
        }
        for offset in 0..<labelLength {
            let actual = try reader.readBit()
            let expected = try tonHashmapKeyBit(
                key: key,
                keyBitLen: keyBitLen,
                bitIndex: keyOffset + offset
            )
            guard actual == expected else {
                return nil
            }
        }
        return labelLength
    }
    let labelBit = try reader.readBit()
    let labelLength = try reader.readUInt(bits: tonHashmapUIntLengthBits(maxLength))
    guard labelLength <= maxLength else {
        return nil
    }
    for offset in 0..<labelLength {
        let expected = try tonHashmapKeyBit(
            key: key,
            keyBitLen: keyBitLen,
            bitIndex: keyOffset + offset
        )
        guard labelBit == expected else {
            return nil
        }
    }
    return labelLength
}

private func tonHashmapReadLabelBits(reader: inout TonBocBitReader, maxLength: Int) throws -> [Bool]? {
    if !(try reader.readBit()) {
        var labelLength = 0
        while try reader.readBit() {
            labelLength += 1
            if labelLength > maxLength {
                return nil
            }
        }
        var bits: [Bool] = []
        bits.reserveCapacity(labelLength)
        for _ in 0..<labelLength {
            bits.append(try reader.readBit())
        }
        return bits
    }
    if !(try reader.readBit()) {
        let labelLength = try reader.readUInt(bits: tonHashmapUIntLengthBits(maxLength))
        guard labelLength <= maxLength else {
            return nil
        }
        var bits: [Bool] = []
        bits.reserveCapacity(labelLength)
        for _ in 0..<labelLength {
            bits.append(try reader.readBit())
        }
        return bits
    }
    let labelBit = try reader.readBit()
    let labelLength = try reader.readUInt(bits: tonHashmapUIntLengthBits(maxLength))
    guard labelLength <= maxLength else {
        return nil
    }
    return Array(repeating: labelBit, count: labelLength)
}

private func tonHashmapCellRefValueHash(
    cells: [TonBocCell],
    computed: [TonBocComputedCell],
    rootIndex: Int,
    key: Data,
    keyBitLen: Int
) throws -> String? {
    guard var cellIndex = try tonHashmapUnwrapMerkleProofCell(cells: cells, cellIndex: rootIndex) else {
        return nil
    }
    var keyOffset = 0
    var remaining = keyBitLen
    for _ in 0...cells.count {
        guard let unwrappedCellIndex = try tonHashmapUnwrapMerkleProofCell(
            cells: cells,
            cellIndex: cellIndex
        ) else {
            return nil
        }
        cellIndex = unwrappedCellIndex
        var reader = try TonBocBitReader(cell: cells[cellIndex])
        guard let labelLength = try tonHashmapReadLabel(
            reader: &reader,
            key: key,
            keyBitLen: keyBitLen,
            keyOffset: keyOffset,
            maxLength: remaining
        ) else {
            return nil
        }
        keyOffset += labelLength
        remaining -= labelLength
        if remaining == 0 {
            guard reader.remainingBits == 0, reader.remainingRefs == 1 else {
                return nil
            }
            let valueRef = try reader.readRef()
            guard try tonBocCellKind(cells[valueRef]) != .prunedBranch else {
                return nil
            }
            return "0x" + computed[valueRef].hashes[3].hexEncodedString()
        }
        guard reader.remainingBits == 0, reader.remainingRefs == 2 else {
            return nil
        }
        let nextBit = try tonHashmapKeyBit(key: key, keyBitLen: keyBitLen, bitIndex: keyOffset)
        keyOffset += 1
        remaining -= 1
        let leftRef = try reader.readRef()
        let rightRef = try reader.readRef()
        cellIndex = nextBit ? rightRef : leftRef
    }
    return nil
}

private func tonHashmapCellRefValueIndex(
    cells: [TonBocCell],
    rootIndex: Int,
    key: Data,
    keyBitLen: Int
) throws -> Int? {
    guard var cellIndex = try tonHashmapUnwrapMerkleProofCell(cells: cells, cellIndex: rootIndex) else {
        return nil
    }
    var keyOffset = 0
    var remaining = keyBitLen
    for _ in 0...cells.count {
        guard let unwrappedCellIndex = try tonHashmapUnwrapMerkleProofCell(
            cells: cells,
            cellIndex: cellIndex
        ) else {
            return nil
        }
        cellIndex = unwrappedCellIndex
        var reader = try TonBocBitReader(cell: cells[cellIndex])
        guard let labelLength = try tonHashmapReadLabel(
            reader: &reader,
            key: key,
            keyBitLen: keyBitLen,
            keyOffset: keyOffset,
            maxLength: remaining
        ) else {
            return nil
        }
        keyOffset += labelLength
        remaining -= labelLength
        if remaining == 0 {
            guard reader.remainingBits == 0, reader.remainingRefs == 1 else {
                return nil
            }
            let valueRef = try reader.readRef()
            guard valueRef >= 0, valueRef < cells.count, try tonBocCellKind(cells[valueRef]) != .prunedBranch else {
                return nil
            }
            return valueRef
        }
        guard reader.remainingBits == 0, reader.remainingRefs == 2 else {
            return nil
        }
        let nextBit = try tonHashmapKeyBit(key: key, keyBitLen: keyBitLen, bitIndex: keyOffset)
        keyOffset += 1
        remaining -= 1
        let leftRef = try reader.readRef()
        let rightRef = try reader.readRef()
        cellIndex = nextBit ? rightRef : leftRef
    }
    return nil
}

private func tonBitsToUInt16(_ bits: [Bool]) -> UInt16? {
    guard bits.count <= UInt16.bitWidth else {
        return nil
    }
    var value = UInt16(0)
    for bit in bits {
        value <<= 1
        if bit {
            value += 1
        }
    }
    return value
}

private func tonReadEd25519SigPubkey(reader: inout TonBocBitReader) throws -> Data? {
    guard try reader.readUInt(bits: 32) == tonEd25519PubkeyConstructor else {
        return nil
    }
    return try reader.readData(byteCount: 32)
}

private func tonReadValidatorDescr(reader: inout TonBocBitReader) throws -> (publicKey: Data, weight: UInt64)? {
    let constructor = try reader.readUInt(bits: 8)
    guard constructor == tonValidatorConstructor || constructor == tonValidatorAddrConstructor else {
        return nil
    }
    guard let publicKey = try tonReadEd25519SigPubkey(reader: &reader) else {
        return nil
    }
    let weight = try reader.readUInt64(bits: 64)
    guard weight != 0 else {
        return nil
    }
    if constructor == tonValidatorAddrConstructor {
        try reader.skipBits(256)
    }
    return (publicKey, weight)
}

private func tonHashmapCollectValidatorDescrsFromReader(
    cells: [TonBocCell],
    reader: inout TonBocBitReader,
    remaining: Int,
    prefix: inout [Bool],
    out: inout [TonValidatorDescr],
    budget: inout Int
) throws -> Bool {
    guard budget > 0, out.count <= tonMaxValidators else {
        return false
    }
    budget -= 1
    guard let labelBits = try tonHashmapReadLabelBits(reader: &reader, maxLength: remaining) else {
        return false
    }
    prefix.append(contentsOf: labelBits)
    defer {
        prefix.removeLast(labelBits.count)
    }
    let nextRemaining = remaining - labelBits.count
    if nextRemaining == 0 {
        guard let key = tonBitsToUInt16(prefix),
              let validator = try tonReadValidatorDescr(reader: &reader),
              reader.isExhausted else {
            return false
        }
        out.append(TonValidatorDescr(key: key, publicKey: validator.publicKey, weight: validator.weight))
        return true
    }
    guard reader.remainingBits == 0, reader.remainingRefs == 2 else {
        return false
    }
    let leftRef = try reader.readRef()
    let rightRef = try reader.readRef()

    prefix.append(false)
    let leftOk = try tonHashmapCollectValidatorDescrsFromCell(
        cells: cells,
        cellIndex: leftRef,
        remaining: nextRemaining - 1,
        prefix: &prefix,
        out: &out,
        budget: &budget
    )
    prefix.removeLast()
    guard leftOk else {
        return false
    }

    prefix.append(true)
    let rightOk = try tonHashmapCollectValidatorDescrsFromCell(
        cells: cells,
        cellIndex: rightRef,
        remaining: nextRemaining - 1,
        prefix: &prefix,
        out: &out,
        budget: &budget
    )
    prefix.removeLast()
    return rightOk
}

private func tonHashmapCollectValidatorDescrsFromCell(
    cells: [TonBocCell],
    cellIndex: Int,
    remaining: Int,
    prefix: inout [Bool],
    out: inout [TonValidatorDescr],
    budget: inout Int
) throws -> Bool {
    guard cellIndex >= 0,
          cellIndex < cells.count,
          try tonBocCellKind(cells[cellIndex]) == .ordinary else {
        return false
    }
    var reader = try TonBocBitReader(cell: cells[cellIndex])
    return try tonHashmapCollectValidatorDescrsFromReader(
        cells: cells,
        reader: &reader,
        remaining: remaining,
        prefix: &prefix,
        out: &out,
        budget: &budget
    )
}

private func tonValidatorSetPayloadFromCell(cells: [TonBocCell], cellIndex: Int) throws -> Data {
    guard cellIndex >= 0,
          cellIndex < cells.count,
          try tonBocCellKind(cells[cellIndex]) == .ordinary else {
        throw TonSccpProverError.invalidBoc("validatorSet")
    }
    var reader = try TonBocBitReader(cell: cells[cellIndex])
    let constructor = try reader.readUInt(bits: 8)
    guard constructor == tonValidatorsConstructor || constructor == tonValidatorsExtConstructor else {
        throw TonSccpProverError.invalidBoc("validatorSet")
    }
    let utimeSince = try reader.readUInt64(bits: 32)
    let utimeUntil = try reader.readUInt64(bits: 32)
    guard utimeUntil > utimeSince else {
        throw TonSccpProverError.invalidBoc("validatorSet")
    }
    let total = try reader.readUInt(bits: 16)
    let main = try reader.readUInt(bits: 16)
    guard total > 0, total <= tonMaxValidators, main > 0, main <= total else {
        throw TonSccpProverError.invalidBoc("validatorSet")
    }
    let declaredTotalWeight: UInt64? = constructor == tonValidatorsExtConstructor
        ? try reader.readUInt64(bits: 64)
        : nil
    var entries: [TonValidatorDescr] = []
    entries.reserveCapacity(total)
    var prefix: [Bool] = []
    prefix.reserveCapacity(tonValidatorSetKeyBits)
    var budget = cells.count + 1
    let ok: Bool
    if constructor == tonValidatorsExtConstructor {
        let hasRoot = try reader.readBit()
        guard hasRoot, reader.remainingBits == 0, reader.remainingRefs == 1 else {
            throw TonSccpProverError.invalidBoc("validatorSet")
        }
        ok = try tonHashmapCollectValidatorDescrsFromCell(
            cells: cells,
            cellIndex: reader.readRef(),
            remaining: tonValidatorSetKeyBits,
            prefix: &prefix,
            out: &entries,
            budget: &budget
        )
    } else {
        ok = try tonHashmapCollectValidatorDescrsFromReader(
            cells: cells,
            reader: &reader,
            remaining: tonValidatorSetKeyBits,
            prefix: &prefix,
            out: &entries,
            budget: &budget
        )
    }
    guard ok, entries.count == total, entries.count <= tonMaxValidators else {
        throw TonSccpProverError.invalidBoc("validatorSet")
    }
    entries.sort { $0.key < $1.key }
    for index in 1..<entries.count {
        guard entries[index - 1].key < entries[index].key else {
            throw TonSccpProverError.invalidBoc("validatorSet")
        }
    }
    var totalWeight = UInt64(0)
    for entry in entries {
        let added = totalWeight.addingReportingOverflow(entry.weight)
        guard !added.overflow else {
            throw TonSccpProverError.invalidBoc("validatorSet")
        }
        totalWeight = added.partialValue
    }
    if let declaredTotalWeight {
        guard declaredTotalWeight != 0, declaredTotalWeight == totalWeight else {
            throw TonSccpProverError.invalidBoc("validatorSet")
        }
    }
    return try canonicalTonValidatorSetPayloadBytes(
        validatorPublicKeys: entries.map(\.publicKey),
        validatorWeights: entries.map(\.weight)
    )
}

private func tonSkipVarUInt(reader: inout TonBocBitReader, lengthBits: Int) throws {
    try reader.skipBits(try reader.readUInt(bits: lengthBits) * 8)
}

private func tonSkipCurrencyCollection(reader: inout TonBocBitReader) throws {
    try tonSkipVarUInt(reader: &reader, lengthBits: 4)
    if try reader.readBit() {
        _ = try reader.readRef()
    }
}

private func tonSkipDepthBalanceInfo(reader: inout TonBocBitReader) throws {
    let splitDepth = try reader.readUInt(bits: 5)
    guard splitDepth <= 30 else {
        throw TonSccpProverError.invalidBoc("depthBalanceInfo")
    }
    try tonSkipCurrencyCollection(reader: &reader)
}

private func tonReadShardAccountLastTransaction(
    computed: [TonBocComputedCell],
    reader: inout TonBocBitReader
) throws -> TonShardAccountLastTransaction {
    try tonSkipDepthBalanceInfo(reader: &reader)
    let accountRef = try reader.readRef()
    guard accountRef >= 0, accountRef < computed.count else {
        throw TonSccpProverError.invalidBoc("shardAccountRef")
    }
    let lastTransactionHash = try reader.readData(byteCount: 32)
    let lastTransactionLt = try reader.readUInt64(bits: 64)
    guard lastTransactionLt != 0 else {
        throw TonSccpProverError.invalidBoc("shardAccountLt")
    }
    guard reader.isExhausted else {
        throw TonSccpProverError.invalidBoc("shardAccount")
    }
    return TonShardAccountLastTransaction(
        hash: "0x" + lastTransactionHash.hexEncodedString(),
        lt: lastTransactionLt
    )
}

private func tonHashmapShardAccountsLastTransaction(
    cells: [TonBocCell],
    computed: [TonBocComputedCell],
    rootIndex: Int,
    key: Data,
    keyBitLen: Int
) throws -> TonShardAccountLastTransaction? {
    guard var cellIndex = try tonHashmapUnwrapMerkleProofCell(cells: cells, cellIndex: rootIndex) else {
        return nil
    }
    var keyOffset = 0
    var remaining = keyBitLen
    for _ in 0...cells.count {
        guard let unwrappedCellIndex = try tonHashmapUnwrapMerkleProofCell(
            cells: cells,
            cellIndex: cellIndex
        ) else {
            return nil
        }
        cellIndex = unwrappedCellIndex
        var reader = try TonBocBitReader(cell: cells[cellIndex])
        guard let labelLength = try tonHashmapReadLabel(
            reader: &reader,
            key: key,
            keyBitLen: keyBitLen,
            keyOffset: keyOffset,
            maxLength: remaining
        ) else {
            return nil
        }
        keyOffset += labelLength
        remaining -= labelLength
        if remaining == 0 {
            return try tonReadShardAccountLastTransaction(computed: computed, reader: &reader)
        }
        let nextBit = try tonHashmapKeyBit(key: key, keyBitLen: keyBitLen, bitIndex: keyOffset)
        keyOffset += 1
        remaining -= 1
        let leftRef = try reader.readRef()
        let rightRef = try reader.readRef()
        try tonSkipDepthBalanceInfo(reader: &reader)
        guard reader.isExhausted else {
            return nil
        }
        cellIndex = nextBit ? rightRef : leftRef
    }
    return nil
}

private func tonLevelMaskValue(_ mask: UInt8) -> UInt8 {
    mask & 0x07
}

private func tonLevelMaskLevel(_ mask: UInt8) -> UInt8 {
    var value = tonLevelMaskValue(mask)
    var level = UInt8(0)
    while value != 0 {
        level += 1
        value >>= 1
    }
    return level
}

private func tonLevelMaskHashIndex(_ mask: UInt8) -> Int {
    var value = tonLevelMaskValue(mask)
    var count = 0
    while value != 0 {
        count += Int(value & 1)
        value >>= 1
    }
    return count
}

private func tonLevelMaskApply(_ mask: UInt8, level: Int) -> UInt8 {
    level == 0 ? 0 : tonLevelMaskValue(mask) & UInt8((1 << level) - 1)
}

private func tonLevelMaskIsSignificant(_ mask: UInt8, level: Int) -> Bool {
    level == 0 || ((tonLevelMaskValue(mask) >> UInt8(level - 1)) & 1) != 0
}

private func tonChildHashDepthForLevel(
    computed: TonBocComputedCell,
    level: Int
) -> (hash: Data, depth: UInt16) {
    let index = min(level, 3)
    return (computed.hashes[index], computed.depths[index])
}

private func tonBocCellKind(_ cell: TonBocCell) throws -> TonBocCellKind {
    guard cell.exotic else {
        return .ordinary
    }
    guard let type = cell.data.first else {
        throw TonSccpProverError.invalidBoc("exoticType")
    }
    switch type {
    case 1:
        return .prunedBranch
    case 3:
        return .merkleProof
    case 4:
        return .merkleUpdate
    default:
        throw TonSccpProverError.invalidBoc("exoticType")
    }
}

private func tonParsePrunedBranch(_ cell: TonBocCell) throws -> TonBocPrunedBranch {
    guard tonCellSerializedBitLenIsByteAligned(dataDescriptor: cell.dataDescriptor, data: cell.data),
          cell.refs.isEmpty,
          cell.data.count >= 2,
          cell.data.first == 1 else {
        throw TonSccpProverError.invalidBoc("prunedBranch")
    }
    if cell.data.count == 35 {
        return TonBocPrunedBranch(
            mask: 1,
            hashes: [Data(cell.data[1..<33])],
            depths: [UInt16(cell.data[33]) << 8 | UInt16(cell.data[34])]
        )
    }
    let mask = tonLevelMaskValue(cell.data[1])
    let level = Int(tonLevelMaskLevel(mask))
    guard (1...3).contains(level), cell.data.count == 2 + level * 34 else {
        throw TonSccpProverError.invalidBoc("prunedBranch")
    }

    var hashes: [Data] = []
    hashes.reserveCapacity(level)
    for index in 0..<level {
        let start = 2 + index * 32
        hashes.append(Data(cell.data[start..<(start + 32)]))
    }

    let depthsStart = 2 + level * 32
    var depths: [UInt16] = []
    depths.reserveCapacity(level)
    for index in 0..<level {
        let start = depthsStart + index * 2
        depths.append(UInt16(cell.data[start]) << 8 | UInt16(cell.data[start + 1]))
    }
    return TonBocPrunedBranch(mask: mask, hashes: hashes, depths: depths)
}

private func tonPushSnakeCells(_ cells: inout [TonCell], bytes: Data) throws -> Int {
    let start = cells.count
    guard !bytes.isEmpty else {
        guard cells.count + 1 <= tonMaxBocCells else {
            throw TonSccpProverError.invalidField("messageBodyBoc")
        }
        cells.append(TonCell(data: Data(), refs: []))
        return start
    }
    let chunkCount = (bytes.count + tonMaxCellDataBytes - 1) / tonMaxCellDataBytes
    guard cells.count + chunkCount <= tonMaxBocCells else {
        throw TonSccpProverError.invalidField("messageBodyBoc")
    }
    for index in 0..<chunkCount {
        let chunkStart = index * tonMaxCellDataBytes
        let chunkEnd = min(chunkStart + tonMaxCellDataBytes, bytes.count)
        let refs = index + 1 == chunkCount ? [] : [start + index + 1]
        cells.append(TonCell(data: bytes[chunkStart..<chunkEnd], refs: refs))
    }
    return start
}

private func tonEncodeBocSingleRoot(_ cells: [TonCell], rootIndex: Int) throws -> Data {
    guard !cells.isEmpty, cells.count <= tonMaxBocCells, rootIndex >= 0, rootIndex < cells.count else {
        throw TonSccpProverError.emptyProof
    }
    let sizeBytes = tonMinSizeBytes(max(cells.count, rootIndex))
    let cellsBytes = try tonSerializeCells(cells, sizeBytes: sizeBytes)
    let offsetBytes = tonMinSizeBytes(cellsBytes.count)
    var out = Data()
    out.append(tonBocMagic)
    out.append(UInt8(sizeBytes))
    out.append(UInt8(offsetBytes))
    out.append(tonSizedUInt(cells.count, size: sizeBytes))
    out.append(tonSizedUInt(1, size: sizeBytes))
    out.append(tonSizedUInt(0, size: sizeBytes))
    out.append(tonSizedUInt(cellsBytes.count, size: offsetBytes))
    out.append(tonSizedUInt(rootIndex, size: sizeBytes))
    out.append(cellsBytes)
    return out
}

private func tonSerializeCells(_ cells: [TonCell], sizeBytes: Int) throws -> Data {
    var out = Data()
    for cell in cells {
        guard cell.data.count <= tonMaxCellDataBytes, cell.refs.count <= tonMaxRefs else {
            throw TonSccpProverError.emptyProof
        }
        out.append(UInt8(cell.refs.count))
        out.append(UInt8(cell.data.count * 2))
        out.append(cell.data)
        for ref in cell.refs {
            guard ref >= 0, ref < cells.count else {
                throw TonSccpProverError.emptyProof
            }
            out.append(tonSizedUInt(ref, size: sizeBytes))
        }
    }
    return out
}

private func tonMinSizeBytes(_ value: Int) -> Int {
    for size in 1...7 where UInt64(value) <= ((UInt64(1) << UInt64(size * 8)) - 1) {
        return size
    }
    return 7
}

private func tonSizedUInt(_ value: Int, size: Int) -> Data {
    var working = UInt64(value)
    var out = Data(repeating: 0, count: size)
    for index in stride(from: size - 1, through: 0, by: -1) {
        out[index] = UInt8(working & 0xff)
        working >>= 8
    }
    return out
}

private func tonBytesFromHex32(_ value: String, field: String) throws -> Data {
    guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
        throw TonSccpProverError.invalidHex32(field)
    }
    var hex = value
    if hex.lowercased().hasPrefix("0x") {
        hex.removeFirst(2)
    }
    guard hex.unicodeScalars.allSatisfy({ !CharacterSet.whitespacesAndNewlines.contains($0) }) else {
        throw TonSccpProverError.invalidHex32(field)
    }
    hex = hex.lowercased()
    guard hex.count == 64, let bytes = Data(hexString: hex), bytes.count == 32 else {
        throw TonSccpProverError.invalidHex32(field)
    }
    return bytes
}

private func tonNormalizeHex32(_ value: String, field: String) throws -> String {
    "0x" + (try tonBytesFromHex32(value, field: field)).hexEncodedString()
}

private func tonNormalizeNonZeroHex32(_ value: String, field: String) throws -> String {
    "0x" + (try tonNonZeroBytesFromHex32(value, field: field)).hexEncodedString()
}

private func tonNormalizeNonEmptyString(_ value: String, field: String) throws -> String {
    let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else {
        throw TonSccpProverError.invalidField(field)
    }
    return trimmed
}

private func tonNormalizePositiveDecimalText(_ value: String, field: String) throws -> String {
    guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value,
          !value.isEmpty,
          value.unicodeScalars.allSatisfy({ $0.value >= 48 && $0.value <= 57 }),
          value.first != "0" else {
        throw TonSccpProverError.invalidField(field)
    }
    return value
}

private func tonNormalizeActiveAccountStatus(_ value: String, field: String) throws -> String {
    guard value == "active" else {
        throw TonSccpProverError.invalidField(field)
    }
    return value
}

private func tonNormalizeRawAddress(_ value: String, field: String) throws -> String {
    guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
        throw TonSccpProverError.invalidField(field)
    }
    let parts = value.split(separator: ":", omittingEmptySubsequences: false)
    guard parts.count == 2 else {
        throw TonSccpProverError.invalidField(field)
    }
    let workchain = String(parts[0])
    let accountHex = String(parts[1])
    let digits = workchain.hasPrefix("-") ? String(workchain.dropFirst()) : workchain
    guard !digits.isEmpty,
          !workchain.hasPrefix("+"),
          !(workchain.hasPrefix("-") && digits == "0"),
          !(digits.count > 1 && digits.hasPrefix("0")),
          digits.unicodeScalars.allSatisfy({ $0.value >= 48 && $0.value <= 57 }),
          let workchainId = Int32(workchain) else {
        throw TonSccpProverError.invalidField(field)
    }
    guard workchainId == tonBasechainWorkchainId else {
        throw TonSccpProverError.invalidField(field)
    }
    guard accountHex.count == 64,
          accountHex.unicodeScalars.allSatisfy({
              ($0.value >= 48 && $0.value <= 57) || ($0.value >= 97 && $0.value <= 102)
          }),
          let account = Data(hexString: accountHex),
          account.count == 32,
          account.contains(where: { $0 != 0 }) else {
        throw TonSccpProverError.invalidField(field)
    }
    return value
}

private func tonNonZeroBytesFromHex32(_ value: String, field: String) throws -> Data {
    let bytes = try tonBytesFromHex32(value, field: field)
    guard bytes.contains(where: { $0 != 0 }) else {
        throw TonSccpProverError.invalidHex32(field)
    }
    return bytes
}

private func tonNormalizeInclusionBranch(_ branch: [Data]) throws -> [Data] {
    guard branch.count <= tonMaxSourceMerkleBranchNodes else {
        throw TonSccpProverError.invalidBranch("inclusionBranch")
    }
    return try branch.enumerated().map { index, sibling in
        guard sibling.count == 32 else {
            throw TonSccpProverError.invalidBranch("inclusionBranch[\(index)]")
        }
        return sibling
    }
}

private func tonNormalizeValidatorSet(validatorPublicKeys: [Data],
                                      validatorWeights: [UInt64]) throws -> ([Data], [UInt64]) {
    guard !validatorPublicKeys.isEmpty,
          validatorPublicKeys.count <= tonMaxValidators,
          validatorPublicKeys.count == validatorWeights.count else {
        throw TonSccpProverError.invalidBranch("validatorPublicKeys")
    }
    var seen = Set<Data>()
    let keys = try validatorPublicKeys.enumerated().map { index, publicKey -> Data in
        guard publicKey.count == 32 else {
            throw TonSccpProverError.invalidBranch("validatorPublicKeys[\(index)]")
        }
        guard publicKey.contains(where: { $0 != 0 }) else {
            throw TonSccpProverError.invalidBranch("validatorPublicKeys[\(index)]")
        }
        guard !seen.contains(publicKey) else {
            throw TonSccpProverError.invalidBranch("validatorPublicKeys[\(index)]")
        }
        seen.insert(publicKey)
        return publicKey
    }
    for (index, weight) in validatorWeights.enumerated() where weight == 0 {
        throw TonSccpProverError.invalidBranch("validatorWeights[\(index)]")
    }
    return (keys, validatorWeights)
}

private func tonAppendVector(_ value: Data, to out: inout Data) {
    tonAppendU32Le(UInt32(value.count), to: &out)
    out.append(value)
}

private func tonAppendString(_ value: String, field: String, to out: inout Data) throws {
    let bytes = Data(try tonNormalizeNonEmptyString(value, field: field).utf8)
    tonAppendU32Le(UInt32(bytes.count), to: &out)
    out.append(bytes)
}

private func tonSignerIndices(signersBitmap: Data, validatorCount: Int) throws -> [Int] {
    guard signersBitmap.count == (validatorCount + 7) / 8 else {
        throw TonSccpProverError.invalidBranch("signersBitmap")
    }
    var indices: [Int] = []
    for (byteIndex, byte) in signersBitmap.enumerated() {
        for bit in 0 ..< 8 where ((byte >> UInt8(bit)) & 1) == 1 {
            let index = byteIndex * 8 + bit
            guard index < validatorCount else {
                throw TonSccpProverError.invalidBranch("signersBitmap")
            }
            indices.append(index)
        }
    }
    return indices
}

private func tonCanonicalValidatorSignatureProofBytes(_ proof: TonValidatorSignatureProofInput) throws -> Data {
    let (publicKeys, weights) = try tonNormalizeValidatorSet(
        validatorPublicKeys: proof.validatorPublicKeys,
        validatorWeights: proof.validatorWeights
    )
    guard proof.version == 1 else {
        throw TonSccpProverError.invalidBranch("version")
    }
    var totalWeight: UInt64 = 0
    for weight in weights {
        let added = totalWeight.addingReportingOverflow(weight)
        guard !added.overflow else {
            throw TonSccpProverError.invalidBranch("totalWeight")
        }
        totalWeight = added.partialValue
    }
    guard totalWeight == proof.totalWeight else {
        throw TonSccpProverError.invalidBranch("totalWeight")
    }
    let signerIndices = try tonSignerIndices(
        signersBitmap: proof.signersBitmap,
        validatorCount: publicKeys.count
    )
    guard !signerIndices.isEmpty, proof.signatures.count == signerIndices.count else {
        throw TonSccpProverError.invalidBranch("signatures")
    }
    var signedWeight: UInt64 = 0
    for index in signerIndices {
        let added = signedWeight.addingReportingOverflow(weights[index])
        guard !added.overflow else {
            throw TonSccpProverError.invalidBranch("signedWeight")
        }
        signedWeight = added.partialValue
    }
    guard signedWeight == proof.signedWeight else {
        throw TonSccpProverError.invalidBranch("signedWeight")
    }
    let twoThirdsFloor = (proof.totalWeight / 3) * 2 + ((proof.totalWeight % 3) * 2) / 3
    guard proof.signedWeight > twoThirdsFloor else {
        throw TonSccpProverError.invalidBranch("signedWeight")
    }
    var out = Data()
    out.append(proof.version)
    tonAppendU64Le(proof.totalWeight, to: &out)
    tonAppendU64Le(proof.signedWeight, to: &out)
    try out.append(tonNonZeroBytesFromHex32(proof.blockMessageHash, field: "blockMessageHash"))
    tonAppendU32Le(UInt32(publicKeys.count), to: &out)
    for publicKey in publicKeys {
        tonAppendVector(publicKey, to: &out)
    }
    tonAppendU32Le(UInt32(weights.count), to: &out)
    for weight in weights {
        tonAppendU64Le(weight, to: &out)
    }
    tonAppendVector(proof.signersBitmap, to: &out)
    tonAppendU32Le(UInt32(proof.signatures.count), to: &out)
    for (index, signature) in proof.signatures.enumerated() {
        guard signature.count == 64 else {
            throw TonSccpProverError.invalidBranch("signatures[\(index)]")
        }
        guard signature.contains(where: { $0 != 0 }) else {
            throw TonSccpProverError.invalidBranch("signatures[\(index)]")
        }
        tonAppendVector(signature, to: &out)
    }
    return out
}

private func tonBoundedBocHash(prefix: String, value: Data, field: String) throws -> (Data, String) {
    guard !value.isEmpty else {
        throw TonSccpProverError.invalidBoc(field)
    }
    guard value.count <= tonMaxBocBytes else {
        throw TonSccpProverError.invalidBoc(field)
    }
    return (value, tonHashHex(prefix: prefix, payload: value))
}

private func tonRejectTemplateSourceStateVerifierHash(_ value: Data) throws {
    guard value.hexEncodedString() != sccpTonTemplateSourceStateVerifierHashV1 else {
        throw TonSccpProverError.invalidField("sourceStateVerifierHash")
    }
}

private func tonNormalizeValidatorSetTransitionForSourceState(_ input: TonValidatorSetTransitionProofInput) throws
    -> TonNormalizedValidatorSetTransitionProof {
    guard input.version == 1 else {
        throw TonSccpProverError.invalidField("validatorSetTransitionProof.version")
    }
    guard input.sourceDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidSourceDomain(input.sourceDomain)
    }
    guard input.masterchainWorkchainId == tonMasterchainWorkchainId else {
        throw TonSccpProverError.invalidField("masterchainWorkchainId")
    }
    guard input.masterchainShard == tonMasterchainShard else {
        throw TonSccpProverError.invalidField("masterchainShard")
    }
    let transitionSignatureHash = try tonNormalizeHex32(
        input.transitionSignatureHash,
        field: "transitionSignatureHash"
    )
    let expectedTransitionSignatureHash = try tonValidatorSetTransitionSignatureHash(
        version: input.version,
        sourceDomain: input.sourceDomain,
        fromValidatorSetSeqno: input.fromValidatorSetSeqno,
        toValidatorSetSeqno: input.toValidatorSetSeqno,
        masterchainSeqno: input.masterchainSeqno,
        masterchainWorkchainId: input.masterchainWorkchainId,
        masterchainShard: input.masterchainShard,
        masterchainBlockHash: input.masterchainBlockHash,
        masterchainFileHash: input.masterchainFileHash,
        parentValidatorSetHash: input.parentValidatorSetHash,
        nextValidatorSetHash: input.nextValidatorSetHash,
        nextValidatorSetPayload: input.nextValidatorSetPayload,
        nextValidatorSetPayloadHash: input.nextValidatorSetPayloadHash,
        nextValidatorSetConfigHash: input.nextValidatorSetConfigHash,
        transitionMessageHash: input.transitionMessageHash,
        validatorSignatureProof: input.validatorSignatureProof
    )
    guard try tonBytesFromHex32(transitionSignatureHash, field: "transitionSignatureHash")
        == (try tonBytesFromHex32(expectedTransitionSignatureHash, field: "transitionSignatureHash")) else {
        throw TonSccpProverError.invalidField("transitionSignatureHash")
    }
    return TonNormalizedValidatorSetTransitionProof(
        version: input.version,
        sourceDomain: input.sourceDomain,
        fromValidatorSetSeqno: input.fromValidatorSetSeqno,
        toValidatorSetSeqno: input.toValidatorSetSeqno,
        masterchainSeqno: input.masterchainSeqno,
        masterchainWorkchainId: input.masterchainWorkchainId,
        masterchainShard: input.masterchainShard,
        masterchainBlockHash: try tonNormalizeNonZeroHex32(
            input.masterchainBlockHash,
            field: "masterchainBlockHash"
        ),
        masterchainFileHash: try tonNormalizeNonZeroHex32(
            input.masterchainFileHash,
            field: "masterchainFileHash"
        ),
        parentValidatorSetHash: try tonNormalizeHex32(
            input.parentValidatorSetHash,
            field: "parentValidatorSetHash"
        ),
        nextValidatorSetHash: try tonNormalizeHex32(input.nextValidatorSetHash, field: "nextValidatorSetHash"),
        nextValidatorSetPayload: input.nextValidatorSetPayload,
        nextValidatorSetPayloadHash: try tonNormalizeHex32(
            input.nextValidatorSetPayloadHash,
            field: "nextValidatorSetPayloadHash"
        ),
        nextValidatorSetConfigHash: try tonNormalizeHex32(
            input.nextValidatorSetConfigHash,
            field: "nextValidatorSetConfigHash"
        ),
        transitionMessageHash: try tonNormalizeHex32(
            input.transitionMessageHash,
            field: "transitionMessageHash"
        ),
        transitionSignatureHash: transitionSignatureHash,
        validatorSignatureProof: input.validatorSignatureProof
    )
}

private func tonCanonicalValidatorSetTransitionProofBytes(
    _ transition: TonNormalizedValidatorSetTransitionProof
) throws -> Data {
    var out = Data()
    out.append(transition.version)
    tonAppendU32Le(transition.sourceDomain, to: &out)
    tonAppendU64Le(transition.fromValidatorSetSeqno, to: &out)
    tonAppendU64Le(transition.toValidatorSetSeqno, to: &out)
    tonAppendU64Le(transition.masterchainSeqno, to: &out)
    tonAppendI32Le(transition.masterchainWorkchainId, to: &out)
    tonAppendU64Le(transition.masterchainShard, to: &out)
    try out.append(tonBytesFromHex32(transition.masterchainBlockHash, field: "masterchainBlockHash"))
    try out.append(tonBytesFromHex32(transition.masterchainFileHash, field: "masterchainFileHash"))
    try out.append(tonBytesFromHex32(transition.parentValidatorSetHash, field: "parentValidatorSetHash"))
    try out.append(tonBytesFromHex32(transition.nextValidatorSetHash, field: "nextValidatorSetHash"))
    tonAppendVector(transition.nextValidatorSetPayload, to: &out)
    try out.append(tonBytesFromHex32(transition.nextValidatorSetPayloadHash, field: "nextValidatorSetPayloadHash"))
    try out.append(tonBytesFromHex32(transition.nextValidatorSetConfigHash, field: "nextValidatorSetConfigHash"))
    try out.append(tonBytesFromHex32(transition.transitionMessageHash, field: "transitionMessageHash"))
    try out.append(tonBytesFromHex32(transition.transitionSignatureHash, field: "transitionSignatureHash"))
    try out.append(tonCanonicalValidatorSignatureProofBytes(transition.validatorSignatureProof))
    return out
}

private func tonValidatorSetTransitionChainHash(
    _ transitions: [TonNormalizedValidatorSetTransitionProof]
) throws -> String {
    var out = Data()
    out.append(1)
    tonAppendU32Le(UInt32(transitions.count), to: &out)
    for transition in transitions {
        try out.append(tonCanonicalValidatorSetTransitionProofBytes(transition))
    }
    return tonHashHex(prefix: tonValidatorSetTransitionChainPrefixV1, payload: out)
}

private func tonAuditRoleProfile(
    _ role: TonSccpFullLightClientAuditRole
) -> TonSccpFullLightClientAuditRoleProfile {
    switch role {
    case .masterchainConfig:
        return TonSccpFullLightClientAuditRoleProfile(
            name: "masterchain_config",
            code: 1,
            circuitId: sccpTonMasterchainConfigOpenVerifyCircuitIdV1,
            verifierId: sccpTonMainnetMasterchainConfigVerifierIdV1,
            requiredInputNames: [
                "masterchain_config_root",
                "masterchain_config_proof_hash",
                "validator_set_payload_hash",
                "config_leaf_hash",
                "config_value_hash",
                "config_proof_boc_hash",
            ]
        )
    case .validatorSetTransition:
        return TonSccpFullLightClientAuditRoleProfile(
            name: "validator_set_transition",
            code: 2,
            circuitId: sccpTonValidatorSetTransitionOpenVerifyCircuitIdV1,
            verifierId: sccpTonMainnetValidatorSetTransitionVerifierIdV1,
            requiredInputNames: [
                "source_trust_anchor_hash",
                "validator_set_hash",
                "validator_set_transition_chain_hash",
                "masterchain_signature_hash",
                "validator_set_transition_count",
            ]
        )
    case .shardAccountsDictionary:
        return TonSccpFullLightClientAuditRoleProfile(
            name: "shard_accounts_dictionary",
            code: 3,
            circuitId: sccpTonShardAccountsDictionaryOpenVerifyCircuitIdV1,
            verifierId: sccpTonMainnetShardAccountsDictionaryVerifierIdV1,
            requiredInputNames: [
                "shard_state_root",
                "shard_state_dictionary_root",
                "transaction_root",
                "shard_state_proof_boc_hash",
                "shard_accounts_proof_boc_hash",
                "shard_state_verification_proof_hash",
            ]
        )
    }
}

private func tonAuditRoleVerifierHash(
    _ input: TonSccpFullLightClientAuditProofInput,
    role: TonSccpFullLightClientAuditRole
) throws -> String {
    switch role {
    case .masterchainConfig:
        return try tonNormalizeNonZeroHex32(
            input.tonMasterchainConfigVerifierHash,
            field: "tonMasterchainConfigVerifierHash"
        )
    case .validatorSetTransition:
        return try tonNormalizeNonZeroHex32(
            input.tonValidatorSetTransitionVerifierHash,
            field: "tonValidatorSetTransitionVerifierHash"
        )
    case .shardAccountsDictionary:
        return try tonNormalizeNonZeroHex32(
            input.tonShardAccountsDictionaryVerifierHash,
            field: "tonShardAccountsDictionaryVerifierHash"
        )
    }
}

private func tonNormalizeFullLightClientAuditInput(
    _ input: TonSccpFullLightClientAuditProofInput,
    role: TonSccpFullLightClientAuditRole
) throws -> TonNormalizedFullLightClientAuditInput {
    let shardState = try tonNormalizeShardStateSourceStateInput(input.shardState)
    let sourceVerifierMaterialHash = try sccpSourceVerifierMaterialHash(
        sourceDomain: shardState.sourceDomain,
        sourceTrustAnchorHash: shardState.sourceTrustAnchorHash,
        consensusVerifierHash: shardState.consensusVerifierHash,
        messageInclusionVerifierHash: shardState.messageInclusionVerifierHash,
        finalityPolicyHash: shardState.finalityPolicyHash,
        sourceStateVerifierHash: shardState.sourceStateVerifierHash
    )
    guard try tonNormalizeNonZeroHex32(
        input.sourceVerifierMaterialHash,
        field: "sourceVerifierMaterialHash"
    ) == sourceVerifierMaterialHash else {
        throw TonSccpProverError.invalidField("sourceVerifierMaterialHash")
    }
    let sourceAdapterDeploymentHash = try tonNormalizeNonZeroHex32(
        input.sourceAdapterDeploymentHash,
        field: "sourceAdapterDeploymentHash"
    )
    let fullLightClientGateHash = try tonNormalizeNonZeroHex32(
        input.fullLightClientGateHash,
        field: "fullLightClientGateHash"
    )
    let auditRoleHashes = try [
        tonAuditRoleVerifierHash(input, role: .masterchainConfig),
        tonAuditRoleVerifierHash(input, role: .validatorSetTransition),
        tonAuditRoleVerifierHash(input, role: .shardAccountsDictionary),
    ]
    try tonRequireFullLightClientAuditRoleSeparation(
        auditRoleHashes: auditRoleHashes,
        existingHashes: [
            shardState.sourceTrustAnchorHash,
            shardState.consensusVerifierHash,
            shardState.messageInclusionVerifierHash,
            shardState.finalityPolicyHash,
            shardState.sourceStateVerifierHash,
        ]
    )
    let shardStateProofPublicInputsHash = try tonShardStateProofPublicInputsHash(input.shardState)
    if let supplied = input.shardStateProofPublicInputsHash,
       try tonNormalizeHex32(supplied, field: "shardStateProofPublicInputsHash")
        != shardStateProofPublicInputsHash {
        throw TonSccpProverError.invalidField("shardStateProofPublicInputsHash")
    }
    let shardStateVerificationProofHash = try tonSccpShardStateVerificationProofHash(
        input.shardStateVerificationProof
    )
    if let supplied = input.shardStateVerificationProofHash,
       try tonNormalizeHex32(supplied, field: "shardStateVerificationProofHash")
        != shardStateVerificationProofHash {
        throw TonSccpProverError.invalidField("shardStateVerificationProofHash")
    }
    if shardState.sourceTrustAnchorHash == shardState.validatorSetHash,
       !shardState.validatorSetTransitionProofs.isEmpty {
        throw TonSccpProverError.invalidField("validatorSetTransitionProofs")
    }
    if shardState.sourceTrustAnchorHash != shardState.validatorSetHash,
       shardState.validatorSetTransitionProofs.isEmpty {
        throw TonSccpProverError.invalidField("validatorSetTransitionProofs")
    }
    let validatorSetPayloadHash = try tonNormalizeNonZeroHex32(
        input.validatorSetPayloadHash,
        field: "validatorSetPayloadHash"
    )
    let configLeafHash = try tonNormalizeNonZeroHex32(input.configLeafHash, field: "configLeafHash")
    let configValueHash = try tonNormalizeNonZeroHex32(input.configValueHash, field: "configValueHash")
    let expectedConfigProofHash = try tonMasterchainConfigProofHash(
        sourceDomain: shardState.sourceDomain,
        masterchainSeqno: shardState.masterchainSeqno,
        masterchainBlockHash: shardState.masterchainBlockHash,
        shardStateRoot: shardState.shardStateRoot,
        configRoot: shardState.masterchainConfigRoot,
        validatorSetHash: shardState.validatorSetHash,
        validatorSetPayloadHash: validatorSetPayloadHash,
        configLeafHash: configLeafHash,
        configLeafIndex: sccpTonCurrentValidatorSetConfigParam,
        configValueHash: configValueHash,
        configDictionaryProofBoc: shardState.configDictionaryProofBoc,
        configInclusionBranch: []
    )
    guard expectedConfigProofHash == shardState.masterchainConfigProofHash else {
        throw TonSccpProverError.invalidField("masterchainConfigProofHash")
    }
    return TonNormalizedFullLightClientAuditInput(
        role: role,
        shardState: shardState,
        sourceVerifierMaterialHash: sourceVerifierMaterialHash,
        sourceAdapterDeploymentHash: sourceAdapterDeploymentHash,
        fullLightClientGateHash: fullLightClientGateHash,
        verifierHash: auditRoleHashes[Int(tonAuditRoleProfile(role).code) - 1],
        shardStateProofPublicInputsHash: shardStateProofPublicInputsHash,
        shardStateVerificationProofHash: shardStateVerificationProofHash,
        validatorSetPayloadHash: validatorSetPayloadHash,
        configLeafHash: configLeafHash,
        configValueHash: configValueHash
    )
}

private func tonRequireFullLightClientAuditRoleSeparation(
    auditRoleHashes: [String],
    existingHashes: [String]
) throws {
    let auditBytes = try auditRoleHashes.map {
        try tonBytesFromHex32($0, field: "tonAuditVerifierHash")
    }
    for index in auditBytes.indices {
        let verifierHash = auditBytes[index]
        for otherIndex in auditBytes.indices where otherIndex > index {
            if verifierHash == auditBytes[otherIndex] {
                throw TonSccpProverError.invalidField("tonAuditVerifierHash")
            }
        }
        if sccpTonTemplateSourceMaterialHashesV1.contains(verifierHash.hexEncodedString()) {
            throw TonSccpProverError.invalidField("tonAuditVerifierHash")
        }
        for existingHash in existingHashes {
            let existingBytes = try tonBytesFromHex32(existingHash, field: "tonAuditExistingHash")
            if existingBytes.contains(where: { $0 != 0 }) && verifierHash == existingBytes {
                throw TonSccpProverError.invalidField("tonAuditVerifierHash")
            }
        }
    }
}

private func tonRequireFullLightClientAuditRequestHashSeparation(
    _ value: TonNormalizedFullLightClientAuditInput,
    statementHash: String? = nil
) throws {
    let verifierHash = try tonBytesFromHex32(value.verifierHash, field: "tonAuditVerifierHash")
    var requestHashes = [
        value.shardState.sourceStateVerifierHash,
        value.sourceVerifierMaterialHash,
        value.sourceAdapterDeploymentHash,
        value.fullLightClientGateHash,
        value.shardStateProofPublicInputsHash,
        value.shardStateVerificationProofHash,
        value.shardState.masterchainConfigProofHash,
        value.shardState.masterchainSignatureHash,
        value.shardState.shardProofHash,
        value.shardState.transitionChainHash,
    ]
    requestHashes.append(contentsOf: try tonAuditRoleColumns(value))
    if let statementHash {
        requestHashes.append(statementHash)
    }
    for requestHash in requestHashes {
        let requestBytes = try tonBytesFromHex32(requestHash, field: "tonAuditRequestHash")
        if requestBytes.contains(where: { $0 != 0 }) && requestBytes == verifierHash {
            throw TonSccpProverError.invalidField("tonAuditVerifierHash")
        }
    }
}

private func tonAuditRoleColumns(_ value: TonNormalizedFullLightClientAuditInput) throws -> [String] {
    let shardState = value.shardState
    switch value.role {
    case .masterchainConfig:
        return [
            shardState.masterchainConfigRoot,
            shardState.masterchainConfigProofHash,
            value.validatorSetPayloadHash,
            value.configLeafHash,
            value.configValueHash,
            shardState.configProofBocHash,
        ]
    case .validatorSetTransition:
        return [
            shardState.sourceTrustAnchorHash,
            shardState.validatorSetHash,
            shardState.transitionChainHash,
            shardState.masterchainSignatureHash,
            "0x" + tonSccpWordU64Le(UInt64(shardState.validatorSetTransitionProofs.count)).hexEncodedString(),
        ]
    case .shardAccountsDictionary:
        return [
            shardState.shardStateRoot,
            shardState.shardStateDictionaryRoot,
            shardState.transactionRoot,
            shardState.shardStateProofBocHash,
            shardState.shardAccountsProofBocHash,
            value.shardStateVerificationProofHash,
        ]
    }
}

private func tonFullLightClientAuditFastpqPublicInputs(
    _ value: TonNormalizedFullLightClientAuditInput,
    statementHash: String
) throws -> TonSccpFullLightClientAuditFastpqPublicInputs {
    var dsidPreimage = Data()
    dsidPreimage.append(tonAuditRoleProfile(value.role).code)
    try dsidPreimage.append(tonBytesFromHex32(statementHash, field: "auditStatementHash"))
    let dsidHash = tonHashBytes(prefix: tonFullLightClientAuditFastpqDsidPrefixV1, payload: dsidPreimage)
    let shardState = value.shardState
    let roots: (String, String, String)
    switch value.role {
    case .masterchainConfig:
        roots = (
            shardState.masterchainConfigRoot,
            shardState.validatorSetHash,
            shardState.masterchainConfigProofHash
        )
    case .validatorSetTransition:
        roots = (
            shardState.sourceTrustAnchorHash,
            shardState.validatorSetHash,
            shardState.transitionChainHash
        )
    case .shardAccountsDictionary:
        roots = (
            shardState.shardStateRoot,
            shardState.transactionRoot,
            shardState.shardStateDictionaryRoot
        )
    }
    return TonSccpFullLightClientAuditFastpqPublicInputs(
        dsid: "0x" + dsidHash.prefix(16).hexEncodedString(),
        slot: String(shardState.masterchainSeqno),
        oldRoot: roots.0,
        newRoot: roots.1,
        permRoot: roots.2,
        txSetHash: statementHash
    )
}

private func tonCanonicalFullLightClientAuditContextBytes(
    _ value: TonNormalizedFullLightClientAuditInput,
    statementHash: String
) throws -> Data {
    try tonRequireFullLightClientAuditRequestHashSeparation(value, statementHash: statementHash)
    let profile = tonAuditRoleProfile(value.role)
    var out = Data()
    out.append(1)
    out.append(profile.code)
    try tonAppendString(profile.circuitId, field: "circuitId", to: &out)
    try tonAppendString(tonFullLightClientAuditFastpqParameterSetV1, field: "parameterSet", to: &out)
    try tonAppendString(profile.verifierId, field: "verifierId", to: &out)
    try out.append(tonBytesFromHex32(value.verifierHash, field: "verifierHash"))
    try out.append(tonBytesFromHex32(value.sourceVerifierMaterialHash, field: "sourceVerifierMaterialHash"))
    try out.append(tonBytesFromHex32(value.sourceAdapterDeploymentHash, field: "sourceAdapterDeploymentHash"))
    try out.append(tonBytesFromHex32(value.fullLightClientGateHash, field: "fullLightClientGateHash"))
    try out.append(tonBytesFromHex32(value.shardStateProofPublicInputsHash, field: "shardStateProofPublicInputsHash"))
    try out.append(tonBytesFromHex32(statementHash, field: "auditStatementHash"))
    return out
}

private func tonFullLightClientAuditFastpqKey(
    _ prefix: String,
    profile: TonSccpFullLightClientAuditRoleProfile
) -> Data {
    var out = Data(prefix.utf8)
    out.append(0)
    out.append(Data(profile.circuitId.utf8))
    return out
}

private func tonNormalizeShardStateSourceStateInput(_ input: TonShardStateProofRequestInput) throws
    -> TonNormalizedShardStateSourceStateInput {
    guard input.sourceDomain == sccpDomainTon else {
        throw TonSccpProverError.invalidSourceDomain(input.sourceDomain)
    }
    guard input.masterchainWorkchainId == tonMasterchainWorkchainId else {
        throw TonSccpProverError.invalidField("masterchainWorkchainId")
    }
    guard input.masterchainShard == tonMasterchainShard else {
        throw TonSccpProverError.invalidField("masterchainShard")
    }
    guard input.shardWorkchainId == tonBasechainWorkchainId else {
        throw TonSccpProverError.invalidField("shardWorkchainId")
    }
    guard input.shardShard != 0 else {
        throw TonSccpProverError.invalidField("shardShard")
    }
    guard input.shardSeqno != 0 else {
        throw TonSccpProverError.invalidField("shardSeqno")
    }
    guard input.transactionLt != 0 else {
        throw TonSccpProverError.invalidField("transactionLt")
    }
    guard input.shardStateDictionaryKeyBitLen == tonShardAccountKeyBits else {
        throw TonSccpProverError.invalidField("shardStateDictionaryKeyBitLen")
    }
    guard tonHashmapKeyIsCanonical(
        key: input.shardStateDictionaryKey,
        keyBitLen: Int(input.shardStateDictionaryKeyBitLen)
    ) else {
        throw TonSccpProverError.invalidField("shardStateDictionaryKey")
    }
    let (shardStateProofBoc, shardStateProofBocHash) = try tonBoundedBocHash(
        prefix: tonShardStateProofBocPrefixV1,
        value: input.shardStateProofBoc,
        field: "shardStateProofBoc"
    )
    let (shardStateDictionaryProofBoc, shardAccountsProofBocHash) = try tonBoundedBocHash(
        prefix: tonShardAccountsProofBocPrefixV1,
        value: input.shardStateDictionaryProofBoc,
        field: "shardStateDictionaryProofBoc"
    )
    let (configDictionaryProofBoc, configProofBocHash) = try tonBoundedBocHash(
        prefix: tonConfigProofBocPrefixV1,
        value: input.configDictionaryProofBoc,
        field: "configDictionaryProofBoc"
    )
    let shardStateRoot = try tonNormalizeNonZeroHex32(input.shardStateRoot, field: "shardStateRoot")
    let transactionRoot = try tonNormalizeNonZeroHex32(input.transactionRoot, field: "transactionRoot")
    let dictionaryRoot = try tonNormalizeNonZeroHex32(
        input.shardStateDictionaryRoot,
        field: "shardStateDictionaryRoot"
    )
    guard try tonShardStateProofRootHash(shardStateProofBoc) == shardStateRoot else {
        throw TonSccpProverError.invalidBoc("shardStateProofBoc")
    }
    let opening = try tonShardStateAccountsOpening(shardStateProofBoc)
    guard opening.accountsRootHash == dictionaryRoot else {
        throw TonSccpProverError.invalidBoc("shardStateProofBoc")
    }
    guard opening.globalId == tonMainnetGlobalId else {
        throw TonSccpProverError.invalidBoc("shardStateGlobalId")
    }
    guard opening.workchainId == Int(tonBasechainWorkchainId),
          opening.workchainId == Int(input.shardWorkchainId) else {
        throw TonSccpProverError.invalidBoc("shardStateWorkchainId")
    }
    guard UInt64(opening.seqNo) == input.shardSeqno else {
        throw TonSccpProverError.invalidBoc("shardStateSeqNo")
    }
    guard opening.shardId == input.shardShard else {
        throw TonSccpProverError.invalidBoc("shardShard")
    }
    guard opening.seqNo != 0, opening.genUtime != 0, opening.genLt != 0 else {
        throw TonSccpProverError.invalidBoc("shardStateMetadata")
    }
    guard UInt64(opening.minRefMcSeqno) <= input.masterchainSeqno else {
        throw TonSccpProverError.invalidBoc("shardStateMinRefMcSeqno")
    }
    guard try tonShardStateAccountKeyMatchesShardPrefix(
        key: input.shardStateDictionaryKey,
        keyBitLen: Int(input.shardStateDictionaryKeyBitLen),
        opening: opening
    ) else {
        throw TonSccpProverError.invalidField("shardStateDictionaryKey")
    }
    guard try tonHashmapEProofRootHash(shardStateDictionaryProofBoc) == dictionaryRoot else {
        throw TonSccpProverError.invalidBoc("shardStateDictionaryProofBoc")
    }
    guard let selectedTransaction = try tonShardAccountsLastTransaction(
        shardStateDictionaryProofBoc,
        key: input.shardStateDictionaryKey,
        keyBitLen: Int(input.shardStateDictionaryKeyBitLen)
    ), selectedTransaction.hash == transactionRoot else {
        throw TonSccpProverError.invalidBoc("shardStateDictionaryProofBoc")
    }
    guard selectedTransaction.lt == input.transactionLt else {
        throw TonSccpProverError.invalidBoc("shardStateDictionaryProofBoc")
    }
    let transitions = try input.validatorSetTransitionProofs.map {
        try tonNormalizeValidatorSetTransitionForSourceState($0)
    }
    let sourceStateVerifierId = try tonNormalizeNonEmptyString(
        input.sourceStateVerifierId,
        field: "sourceStateVerifierId"
    )
    guard sourceStateVerifierId == sccpTonMainnetShardStateVerifierIdV1 else {
        throw TonSccpProverError.invalidField("sourceStateVerifierId")
    }
    let sourceStateVerifierHashBytes = try tonNonZeroBytesFromHex32(
        input.sourceStateVerifierHash,
        field: "sourceStateVerifierHash"
    )
    try tonRejectTemplateSourceStateVerifierHash(sourceStateVerifierHashBytes)
    let sourceStateVerifierHash = "0x" + sourceStateVerifierHashBytes.hexEncodedString()
    return TonNormalizedShardStateSourceStateInput(
        version: 1,
        sourceDomain: input.sourceDomain,
        masterchainSeqno: input.masterchainSeqno,
        masterchainWorkchainId: input.masterchainWorkchainId,
        masterchainShard: input.masterchainShard,
        masterchainBlockHash: try tonNormalizeNonZeroHex32(input.masterchainBlockHash, field: "masterchainBlockHash"),
        masterchainFileHash: try tonNormalizeNonZeroHex32(input.masterchainFileHash, field: "masterchainFileHash"),
        validatorSetHash: try tonNormalizeHex32(input.validatorSetHash, field: "validatorSetHash"),
        masterchainConfigRoot: try tonNormalizeHex32(input.masterchainConfigRoot, field: "masterchainConfigRoot"),
        masterchainConfigProofHash: try tonNormalizeHex32(
            input.masterchainConfigProofHash,
            field: "masterchainConfigProofHash"
        ),
        shardWorkchainId: input.shardWorkchainId,
        shardShard: input.shardShard,
        shardSeqno: input.shardSeqno,
        shardBlockHash: try tonNormalizeHex32(input.shardBlockHash, field: "shardBlockHash"),
        shardFileHash: try tonNormalizeNonZeroHex32(input.shardFileHash, field: "shardFileHash"),
        shardStateRoot: shardStateRoot,
        transactionRoot: transactionRoot,
        transactionLt: input.transactionLt,
        shardStateDictionaryRoot: dictionaryRoot,
        shardStateDictionaryKeyBitLen: input.shardStateDictionaryKeyBitLen,
        shardStateDictionaryKey: input.shardStateDictionaryKey,
        masterchainSignatureHash: try tonNormalizeHex32(
            input.masterchainSignatureHash,
            field: "masterchainSignatureHash"
        ),
        shardProofHash: try tonNormalizeHex32(input.shardProofHash, field: "shardProofHash"),
        shardStateProofBoc: shardStateProofBoc,
        shardStateDictionaryProofBoc: shardStateDictionaryProofBoc,
        configDictionaryProofBoc: configDictionaryProofBoc,
        shardStateProofBocHash: shardStateProofBocHash,
        shardAccountsProofBocHash: shardAccountsProofBocHash,
        configProofBocHash: configProofBocHash,
        validatorSetTransitionProofs: transitions,
        transitionChainHash: try tonValidatorSetTransitionChainHash(transitions),
        sourceStateVerifierId: sourceStateVerifierId,
        sourceStateVerifierHash: sourceStateVerifierHash,
        sourceTrustAnchorId: try tonNormalizeNonEmptyString(input.sourceTrustAnchorId, field: "sourceTrustAnchorId"),
        sourceTrustAnchorHash: try tonNormalizeNonZeroHex32(
            input.sourceTrustAnchorHash,
            field: "sourceTrustAnchorHash"
        ),
        consensusVerifierId: try tonNormalizeNonEmptyString(input.consensusVerifierId, field: "consensusVerifierId"),
        consensusVerifierHash: try tonNormalizeNonZeroHex32(
            input.consensusVerifierHash,
            field: "consensusVerifierHash"
        ),
        messageInclusionVerifierId: try tonNormalizeNonEmptyString(
            input.messageInclusionVerifierId,
            field: "messageInclusionVerifierId"
        ),
        messageInclusionVerifierHash: try tonNormalizeNonZeroHex32(
            input.messageInclusionVerifierHash,
            field: "messageInclusionVerifierHash"
        ),
        finalityPolicyId: try tonNormalizeNonEmptyString(input.finalityPolicyId, field: "finalityPolicyId"),
        finalityPolicyHash: try tonNormalizeNonZeroHex32(input.finalityPolicyHash, field: "finalityPolicyHash")
    )
}

private func tonAppendU16Be(_ value: UInt16, to out: inout Data) {
    out.append(UInt8((value >> 8) & 0xff))
    out.append(UInt8(value & 0xff))
}

private func tonAppendU16Le(_ value: UInt16, to out: inout Data) {
    out.append(UInt8(value & 0xff))
    out.append(UInt8((value >> 8) & 0xff))
}

private func tonAppendU32Le(_ value: UInt32, to out: inout Data) {
    out.append(UInt8(value & 0xff))
    out.append(UInt8((value >> 8) & 0xff))
    out.append(UInt8((value >> 16) & 0xff))
    out.append(UInt8((value >> 24) & 0xff))
}

private func tonAppendI32Le(_ value: Int32, to out: inout Data) {
    tonAppendU32Le(UInt32(bitPattern: value), to: &out)
}

private func tonAppendU32Be(_ value: UInt32, to out: inout Data) {
    out.append(UInt8((value >> 24) & 0xff))
    out.append(UInt8((value >> 16) & 0xff))
    out.append(UInt8((value >> 8) & 0xff))
    out.append(UInt8(value & 0xff))
}

private func tonAppendU64Le(_ value: UInt64, to out: inout Data) {
    for shift in stride(from: 0, through: 56, by: 8) {
        out.append(UInt8((value >> UInt64(shift)) & 0xff))
    }
}

private func tonSccpWordU32Le(_ value: UInt32) -> Data {
    var out = Data()
    tonAppendU32Le(value, to: &out)
    out.append(Data(repeating: 0, count: 28))
    return out
}

private func tonSccpWordU8(_ value: UInt8) -> Data {
    var out = Data(repeating: 0, count: 32)
    out[0] = value
    return out
}

private func tonSccpWordI32Le(_ value: Int32) -> Data {
    var out = Data()
    tonAppendI32Le(value, to: &out)
    out.append(Data(repeating: 0, count: 28))
    return out
}

private func tonSccpWordU64Le(_ value: UInt64) -> Data {
    var out = Data()
    tonAppendU64Le(value, to: &out)
    out.append(Data(repeating: 0, count: 24))
    return out
}

private func tonAppendU64Be(_ value: UInt64, to out: inout Data) {
    for shift in stride(from: 56, through: 0, by: -8) {
        out.append(UInt8((value >> UInt64(shift)) & 0xff))
    }
}

private func tonHashHex(prefix: String, payload: Data) -> String {
    "0x" + tonHashBytes(prefix: prefix, payload: payload).hexEncodedString()
}

private func tonHashBytes(prefix: String, payload: Data) -> Data {
    var preimage = Data(prefix.utf8)
    preimage.append(payload)
    return Blake2b.hash256(preimage)
}
