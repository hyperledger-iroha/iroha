import CryptoKit
import Foundation

/// SCCP domain id for Ethereum.
public let sccpDomainEthereum: UInt32 = 1
/// Ethereum mainnet EVM chain id.
public let sccpEthereumMainnetChainId: UInt64 = 1
/// Ethereum mainnet EVM chain id encoded as a 32-byte ABI word.
public let sccpEthereumMainnetNetworkId =
    "0x0000000000000000000000000000000000000000000000000000000000000001"

/// SCCP domain id for BNB Smart Chain.
public let sccpDomainBsc: UInt32 = 2
/// BNB Smart Chain mainnet EVM chain id.
public let sccpBscMainnetChainId: UInt64 = 56
/// BNB Smart Chain mainnet EVM chain id encoded as a 32-byte ABI word.
public let sccpBscMainnetNetworkId =
    "0x0000000000000000000000000000000000000000000000000000000000000038"

/// SCCP domain id for TRON.
public let sccpDomainTron: UInt32 = 5

/// SCCP domain id for SORA Kusama.
public let sccpDomainSoraKusama: UInt32 = 6

/// SCCP domain id for SORA Polkadot.
public let sccpDomainSoraPolkadot: UInt32 = 7

/// SCCP domain id for SORA2.
public let sccpDomainSora2: UInt32 = 8

/// OpenVerify circuit id used by every SCCP source-adapter verifier profile.
public let sccpSourceAdapterOpenVerifyCircuitIdV1 = "sccp-source-adapter-v1"

/// FastPQ parameter set used by the canonical SCCP source-adapter verifier profile.
public let sccpSourceAdapterFastpqParameterSetV1 = "fastpq-lane-balanced"

/// Proof family used by canonical SCCP source-adapter deployments.
public let sccpStarkFriProofFamilyV1 = "stark-fri-v1"

private let sccpEvmDestinationBindingLabelV1 =
    Data("iroha:sccp:evm-destination-binding:v1".utf8)
private let sccpTronDestinationBindingLabelV1 =
    Data("iroha:sccp:tron-destination-binding:v1".utf8)

/// OpenVerify circuit id used by Substrate runtime-storage source-state proofs.
public let sccpSubstrateRuntimeStorageOpenVerifyCircuitIdV1 =
    "sccp-substrate-runtime-storage-v1"

/// Solana Tower replay verifier id used by audited full light-client deployments.
public let sccpSolanaMainnetTowerReplayVerifierIdV1 =
    "sccp:sol:light-client:tower-replay-mainnet-beta:v1"

/// Solana full AccountsDB lattice verifier id used by audited full light-client deployments.
public let sccpSolanaMainnetFullAccountsdbLatticeVerifierIdV1 =
    "sccp:sol:light-client:full-accountsdb-lattice-mainnet-beta:v1"

/// Solana bank/fork-choice verifier id used by audited full light-client deployments.
public let sccpSolanaMainnetBankForkChoiceVerifierIdV1 =
    "sccp:sol:light-client:bank-fork-choice-mainnet-beta:v1"

/// TON masterchain config verifier id used by audited full light-client deployments.
public let sccpTonMainnetMasterchainConfigVerifierIdV1 =
    "sccp:ton:light-client:masterchain-config-mainnet:v1"

/// TON validator-set transition verifier id used by audited full light-client deployments.
public let sccpTonMainnetValidatorSetTransitionVerifierIdV1 =
    "sccp:ton:light-client:validator-set-transition-mainnet:v1"

/// TON shard-accounts dictionary verifier id used by audited full light-client deployments.
public let sccpTonMainnetShardAccountsDictionaryVerifierIdV1 =
    "sccp:ton:light-client:shard-accounts-dictionary-mainnet:v1"

private let sccpEvmMaxReceiptValueBytes = 16 * 1024
private let sccpEvmMaxBlockReceipts = 4096
private let sccpSourceAdapterFastpqTraceRootV1: UInt64 = 0x002A_247F_81C6_F850
private let sccpSourceAdapterFastpqLdeRootV1: UInt64 = 0x6026_3388_DBBF_9B2A
private let sccpSourceAdapterFastpqOmegaCosetV1: UInt64 = 0x6AF3_25E8_25AD_5C18
private let sccpEvmReceiptRootValueMarker = Data("sccp:evm:receipt-root-value:v1".utf8)
private let sccpEthExecutionPayloadBodyFieldIndex: UInt64 = 9
private let sccpEthExecutionPayloadBodyBranchDepth = 4
private let sccpEthMainnetSyncCommitteeAuthorities = 512
private let sccpEthMaxSyncCommitteeAuthorities = sccpEthMainnetSyncCommitteeAuthorities
private let sccpEthSyncCommitteePublicKeyBytes = 48
private let sccpEthSyncCommitteePopBytes = 96
private let sccpEthSyncCommitteeSignatureBytes = 96
private let sccpEthMaxSyncCommitteeSignatureBytes = 192
public let sccpEthMainnetSlotsPerEpoch: UInt64 = 32
public let sccpEthMainnetEpochsPerSyncCommitteePeriod: UInt64 = 256
public let sccpEthMainnetSlotsPerSyncCommitteePeriod: UInt64 =
    sccpEthMainnetSlotsPerEpoch * sccpEthMainnetEpochsPerSyncCommitteePeriod
private let sccpEthMaxSyncCommitteePayloadBytes =
    1 + 4 + sccpEthMaxSyncCommitteeAuthorities *
        (4 + sccpEthSyncCommitteePublicKeyBytes + 8 + 4 + sccpEthSyncCommitteePopBytes)
private let sccpEthMaxSyncCommitteeSignersBitmapBytes = (sccpEthMaxSyncCommitteeAuthorities + 7) / 8
private let sccpBscMaxParliaValidators = 255
private let sccpBscMaxValidatorSetPayloadBytes = 1 + 4 + sccpBscMaxParliaValidators * (20 + 8)
private let sccpBscParliaEpochLengthBlocks: UInt64 = 200
private let sccpTronMaxRawHeaderBytes = 16 * 1024
private let sccpTronMaxReceiptValueBytes = 16 * 1024
private let sccpSubstrateMaxAuthorities = 2048
private let sccpSubstrateMaxAuthoritySetPayloadBytes = 1 + 4 + sccpSubstrateMaxAuthorities * (32 + 8)
private let sccpSubstrateSystemEventsStorageKey = try! sourceProofBytesFromHex32(
    "0x26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7",
    field: "systemEventsStorageKey"
)
private let sccpSubstrateRuntimeStorageFastpqParameterSetV1 = "fastpq-lane-balanced"
private let sccpSubstrateRuntimeStorageProofPublicInputsPrefixV1 =
    "sccp:substrate:runtime-storage-proof-public-inputs:v1"
private let sccpSubstrateRuntimeStorageFastpqDsidPrefixV1 =
    "sccp:substrate:runtime-storage:fastpq:dsid:v1"
private let sccpSubstrateRuntimeStorageFastpqStatementKeyV1 =
    "sccp:substrate:runtime-storage:v1:statement"
private let sccpSubstrateRuntimeStorageFastpqContextKeyV1 =
    "sccp:substrate:runtime-storage:v1:context"
private let sccpSubstrateRuntimeStorageFastpqStorageKeyV1 =
    "sccp:substrate:runtime-storage:v1:storage-key"
private let sccpTronMaxTransactionBytes = 64 * 1024
private let sccpTronMaxTransactionMerkleBranchNodes = 64
private let sccpTronSourceCallSignatures = 1
private let sccpTronMaxWitnesses = 64
private let sccpTronSourceMessageCallAbi = Data("submitSccpSourceEvent(uint32,uint32,bytes32)".utf8)
private let sccpTronTriggerSmartContractTypeUrl =
    Data("type.googleapis.com/protocol.TriggerSmartContract".utf8)
private let sccpTronReceiptRootValueMarker = Data("sccp:tron:receipt-root-value:v1".utf8)
private let sccpSecp256k1ScalarOrderBe = Data([
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
    0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b,
    0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41
])
private let sccpSecp256k1ScalarHalfOrderBe = Data([
    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50, 0x1d,
    0xdf, 0xe9, 0x2f, 0x46, 0x68, 0x1b, 0x20, 0xa0
])
private let sccpSecp256k1FieldPrimeBe = Data([
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xfe, 0xff, 0xff, 0xfc, 0x2f
])
private let sccpSecp256k1GeneratorXBe = Data(hexString:
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
)!
private let sccpSecp256k1GeneratorYBe = Data(hexString:
    "483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8"
)!

/// Error cases for SCCP source proof-hash construction.
public enum SccpSourceProofHashError: Error, Equatable {
    case invalidHex20(String)
    case invalidHex32(String)
    case invalidBranch(String)
    case invalidValidatorSet(String)
    case invalidRlp(String)
    case unsupportedSourceAdapterDomain(String)
    case unsupportedDestinationBindingDomain(String)
    case invalidSourceMaterial(String)
}

/// Ethereum receipt-trie proof material derived from an execution block's receipt list.
public struct EvmReceiptTrieProof: Equatable {
    public let receiptsRoot: String
    public let receiptRlp: String
    public let receiptTrieKey: String
    public let receiptTrieProofNodes: [Data]
}

/// FastPQ public inputs used by Substrate runtime-storage source-state proofs.
public struct SubstrateSccpRuntimeStorageFastpqPublicInputs: Equatable {
    public let dsid: String
    public let slot: String
    public let oldRoot: String
    public let newRoot: String
    public let permRoot: String
    public let txSetHash: String
}

/// FastPQ metadata transition used by Substrate runtime-storage source-state proofs.
public struct SubstrateSccpRuntimeStorageFastpqTransition: Equatable {
    public let key: String
    public let operation: String
    public let oldValue: String
    public let newValue: String
}

/// Deterministic proof request for a mobile Substrate runtime-storage prover.
public struct SubstrateSccpRuntimeStorageProofRequest: Equatable {
    public let version: UInt8
    public let proofFamily: String
    public let circuitId: String
    public let parameterSet: String
    public let sourceDomain: UInt32
    public let finalizedBlockNumber: String
    public let grandpaSetId: String
    public let sourceStateVerifierId: String
    public let sourceStateVerifierHash: String
    public let runtimeStorageProofPublicInputsHash: String
    public let storageProofHash: String
    public let statementBytes: Data
    public let verificationContextBytes: Data
    public let schemaDescriptor: Data
    public let publicInputColumns: [[String]]
    public let fastpqPublicInputs: SubstrateSccpRuntimeStorageFastpqPublicInputs
    public let fastpqTransitions: [SubstrateSccpRuntimeStorageFastpqTransition]
}

/// One BSC ValidatorSet storage-slot proof transcript entry.
public struct BscValidatorStorageProof: Equatable {
    public let version: UInt8
    public let validatorIndex: UInt32
    public let storageSlot: String
    public let storageValue: Data
    public let storageValueHash: String
    public let storageProofNodes: [Data]

    public init(version: UInt8 = 1,
                validatorIndex: UInt32,
                storageSlot: String,
                storageValue: Data,
                storageValueHash: String,
                storageProofNodes: [Data]) {
        self.version = version
        self.validatorIndex = validatorIndex
        self.storageSlot = storageSlot
        self.storageValue = storageValue
        self.storageValueHash = storageValueHash
        self.storageProofNodes = storageProofNodes
    }
}

/// BSC ValidatorSet account/storage proof transcript material.
public struct BscValidatorSetMetadataProof: Equatable {
    public let version: UInt8
    public let stateRoot: String
    public let nextValidatorSetPayloadHash: String
    public let validatorContractAddress: Data
    public let accountProofNodes: [Data]
    public let storageRoot: String
    public let validatorSetLengthSlot: String
    public let validatorSetLengthValue: Data
    public let validatorSetLengthValueHash: String
    public let validatorSetLengthProofNodes: [Data]
    public let validatorStorageProofs: [BscValidatorStorageProof]

    public init(version: UInt8 = 1,
                stateRoot: String,
                nextValidatorSetPayloadHash: String,
                validatorContractAddress: Data,
                accountProofNodes: [Data],
                storageRoot: String,
                validatorSetLengthSlot: String,
                validatorSetLengthValue: Data,
                validatorSetLengthValueHash: String,
                validatorSetLengthProofNodes: [Data],
                validatorStorageProofs: [BscValidatorStorageProof]) {
        self.version = version
        self.stateRoot = stateRoot
        self.nextValidatorSetPayloadHash = nextValidatorSetPayloadHash
        self.validatorContractAddress = validatorContractAddress
        self.accountProofNodes = accountProofNodes
        self.storageRoot = storageRoot
        self.validatorSetLengthSlot = validatorSetLengthSlot
        self.validatorSetLengthValue = validatorSetLengthValue
        self.validatorSetLengthValueHash = validatorSetLengthValueHash
        self.validatorSetLengthProofNodes = validatorSetLengthProofNodes
        self.validatorStorageProofs = validatorStorageProofs
    }
}

/// BSC Parlia commit-seal transcript material.
public struct BscCommitSealProof: Equatable {
    public let version: UInt8
    public let totalPower: UInt64
    public let signedPower: UInt64
    public let commitMessageHash: String
    public let validatorPublicKeys: [Data]
    public let validatorPowers: [UInt64]
    public let signersBitmap: Data
    public let signatures: [Data]
    public let validatorSetHash: String?

    public init(version: UInt8 = 1,
                totalPower: UInt64,
                signedPower: UInt64,
                commitMessageHash: String,
                validatorPublicKeys: [Data],
                validatorPowers: [UInt64],
                signersBitmap: Data,
                signatures: [Data],
                validatorSetHash: String? = nil) {
        self.version = version
        self.totalPower = totalPower
        self.signedPower = signedPower
        self.commitMessageHash = commitMessageHash
        self.validatorPublicKeys = validatorPublicKeys
        self.validatorPowers = validatorPowers
        self.signersBitmap = signersBitmap
        self.signatures = signatures
        self.validatorSetHash = validatorSetHash
    }
}

/// Canonical governed EVM-family destination binding derived from deployment material.
public struct EvmSccpDestinationBinding: Equatable {
    public let version: UInt8
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let networkId: String
    public let verifierAddress: String
    public let bridgeAddress: String
    public let verifierCodeHash: String
    public let verifierKeyHash: String
    public let verifierBackend: String
    public let proofFamily: String
    public let key: String
    public let hash: String
}

/// Canonical governed TRON destination binding derived from deployment material.
public struct TronSccpDestinationBinding: Equatable {
    public let version: UInt8
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let networkId: String
    public let verifierAddress: String
    public let verifierCodeHash: String
    public let verifierKeyHash: String
    public let verifierBackend: String
    public let proofFamily: String
    public let key: String
    public let hash: String
}

/// Canonical OpenVerify verifier-key commitment for an SCCP source-adapter lane.
public func sccpSourceAdapterVerifierVkHash(sourceDomain: UInt32,
                                            targetDomain: UInt32 = sccpDomainSora) throws -> String {
    guard targetDomain == sccpDomainSora else {
        throw SccpSourceProofHashError.unsupportedSourceAdapterDomain("targetDomain")
    }
    let profile = try sccpSourceAdapterVerifierProfile(sourceDomain: sourceDomain)
    var verifier = Data()
    verifier.append(1)
    sourceProofAppendDataVector(Data(sccpSourceAdapterOpenVerifyCircuitIdV1.utf8), to: &verifier)
    sourceProofAppendDataVector(Data(profile.chain.utf8), to: &verifier)
    sourceProofAppendU32Le(sourceDomain, to: &verifier)
    sourceProofAppendU32Le(targetDomain, to: &verifier)
    verifier.append(profile.proofPlan)
    verifier.append(profile.finalityModel)
    sourceProofAppendDataVector(Data(sccpSourceAdapterFastpqParameterSetV1.utf8), to: &verifier)
    sourceProofAppendU32Le(128, to: &verifier)
    sourceProofAppendU32Le(23, to: &verifier)
    sourceProofAppendU32Le(16, to: &verifier)
    sourceProofAppendU64Le(sccpSourceAdapterFastpqTraceRootV1, to: &verifier)
    sourceProofAppendU32Le(19, to: &verifier)
    sourceProofAppendU64Le(sccpSourceAdapterFastpqLdeRootV1, to: &verifier)
    sourceProofAppendU32Le(65_536, to: &verifier)
    verifier.append(1)
    sourceProofAppendU32Le(19, to: &verifier)
    sourceProofAppendU64Le(sccpSourceAdapterFastpqOmegaCosetV1, to: &verifier)
    sourceProofAppendDataVector(Data("Goldilocks".utf8), to: &verifier)
    sourceProofAppendDataVector(Data("18446744069414584321".utf8), to: &verifier)
    sourceProofAppendU32Le(2, to: &verifier)
    sourceProofAppendDataVector(Data("Poseidon2(Goldilocks)".utf8), to: &verifier)
    sourceProofAppendDataVector(Data("SHA3-256".utf8), to: &verifier)
    sourceProofAppendU32Le(8, to: &verifier)
    sourceProofAppendU32Le(8, to: &verifier)
    sourceProofAppendU32Le(8, to: &verifier)
    sourceProofAppendU32Le(46, to: &verifier)

    var preimage = Data(sccpSourceAdapterOpenVerifyCircuitIdV1.utf8)
    preimage.append(verifier)
    return "0x" + Data(SHA256.hash(data: preimage)).hexEncodedString()
}

/// Governed destination binding key for a native SCCP lane.
public func sccpDestinationBindingKey(domain: UInt32) throws -> String {
    try sccpDestinationBindingProfile(targetDomain: domain).bindingKey
}

/// Canonical `SccpDestinationBindingV1` hash for a native SCCP lane.
public func sccpDestinationBindingHash(domain: UInt32) throws -> String {
    let profile = try sccpDestinationBindingProfile(targetDomain: domain)
    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(sccpDomainSora, to: &out)
    sourceProofAppendU32Le(domain, to: &out)
    out.append(1)
    out.append(1)
    out.append(profile.verifierTarget)
    out.append(profile.backendFamily)
    sourceProofAppendDataVector(Data(profile.bindingKey.utf8), to: &out)
    sourceProofAppendDataVector(Data(profile.manifestSeed.utf8), to: &out)
    sourceProofAppendDataVector(Data(sccpStarkFriProofFamilyV1.utf8), to: &out)
    sourceProofAppendDataVector(Data(profile.verifierBackend.utf8), to: &out)
    return try sourceProofHashHex(prefix: "sccp:destination:binding:v1", payload: out)
}

/// Governed EVM-family destination binding for UI-side SCCP proof generation.
public func sccpEvmDestinationBinding(
    sourceDomain: UInt32 = sccpDomainSora,
    targetDomain: UInt32 = sccpDomainEthereum,
    networkId: String,
    verifierAddress: String,
    bridgeAddress: String,
    verifierCodeHash: String,
    verifierKeyHash: String,
    verifierBackend: String = sccpEvmGroth16Bn254ProofBackendV1,
    proofFamily: String = sccpStarkFriProofFamilyV1
) throws -> EvmSccpDestinationBinding {
    guard sourceDomain == sccpDomainSora else {
        throw SccpSourceProofHashError.unsupportedDestinationBindingDomain("sourceDomain")
    }
    guard targetDomain == sccpDomainEthereum || targetDomain == sccpDomainBsc else {
        throw SccpSourceProofHashError.unsupportedDestinationBindingDomain("targetDomain")
    }
    guard verifierBackend == sccpEvmGroth16Bn254ProofBackendV1 else {
        throw SccpSourceProofHashError.invalidSourceMaterial("verifierBackend")
    }
    guard proofFamily == sccpStarkFriProofFamilyV1 else {
        throw SccpSourceProofHashError.invalidSourceMaterial("proofFamily")
    }

    let networkIdBytes = try sourceProofNonZeroBytesFromHex32(networkId, field: "networkId")
    let verifierAddressBytes = try sourceProofNonZeroBytesFromHex20(
        verifierAddress,
        field: "verifierAddress"
    )
    let bridgeAddressBytes = try sourceProofNonZeroBytesFromHex20(
        bridgeAddress,
        field: "bridgeAddress"
    )
    guard verifierAddressBytes != bridgeAddressBytes else {
        throw SccpSourceProofHashError.invalidSourceMaterial("bridgeAddress")
    }
    let verifierCodeHashBytes = try sourceProofNonZeroBytesFromHex32(
        verifierCodeHash,
        field: "verifierCodeHash"
    )
    let verifierKeyHashBytes = try sourceProofNonZeroBytesFromHex32(
        verifierKeyHash,
        field: "verifierKeyHash"
    )

    let normalizedNetworkId = "0x" + networkIdBytes.hexEncodedString()
    let normalizedVerifierAddress = "0x" + verifierAddressBytes.hexEncodedString()
    let normalizedBridgeAddress = "0x" + bridgeAddressBytes.hexEncodedString()
    let normalizedVerifierCodeHash = "0x" + verifierCodeHashBytes.hexEncodedString()
    let normalizedVerifierKeyHash = "0x" + verifierKeyHashBytes.hexEncodedString()
    let key = [
        "evm",
        String(sourceDomain),
        String(targetDomain),
        networkIdBytes.hexEncodedString(),
        normalizedVerifierAddress,
        normalizedBridgeAddress,
        normalizedVerifierCodeHash,
        normalizedVerifierKeyHash
    ].joined(separator: ":")

    var payload = Data(irohaKeccak256(sccpEvmDestinationBindingLabelV1))
    payload.append(irohaKeccak256(Data(verifierBackend.utf8)))
    payload.append(irohaKeccak256(Data(proofFamily.utf8)))
    payload.append(networkIdBytes)
    sourceProofAppendAbiU32(sourceDomain, to: &payload)
    sourceProofAppendAbiU32(targetDomain, to: &payload)
    sourceProofAppendAbiAddress20(verifierAddressBytes, to: &payload)
    sourceProofAppendAbiAddress20(bridgeAddressBytes, to: &payload)
    payload.append(verifierCodeHashBytes)
    payload.append(verifierKeyHashBytes)
    let hash = "0x" + irohaKeccak256(payload).hexEncodedString()

    return EvmSccpDestinationBinding(
        version: 1,
        sourceDomain: sourceDomain,
        targetDomain: targetDomain,
        networkId: normalizedNetworkId,
        verifierAddress: normalizedVerifierAddress,
        bridgeAddress: normalizedBridgeAddress,
        verifierCodeHash: normalizedVerifierCodeHash,
        verifierKeyHash: normalizedVerifierKeyHash,
        verifierBackend: verifierBackend,
        proofFamily: proofFamily,
        key: key,
        hash: hash
    )
}

/// Canonical governed EVM-family destination binding hash for UI-side proof requests.
public func sccpEvmDestinationBindingHash(
    sourceDomain: UInt32 = sccpDomainSora,
    targetDomain: UInt32 = sccpDomainEthereum,
    networkId: String,
    verifierAddress: String,
    bridgeAddress: String,
    verifierCodeHash: String,
    verifierKeyHash: String,
    verifierBackend: String = sccpEvmGroth16Bn254ProofBackendV1,
    proofFamily: String = sccpStarkFriProofFamilyV1
) throws -> String {
    try sccpEvmDestinationBinding(
        sourceDomain: sourceDomain,
        targetDomain: targetDomain,
        networkId: networkId,
        verifierAddress: verifierAddress,
        bridgeAddress: bridgeAddress,
        verifierCodeHash: verifierCodeHash,
        verifierKeyHash: verifierKeyHash,
        verifierBackend: verifierBackend,
        proofFamily: proofFamily
    ).hash
}

/// Governed Ethereum mainnet destination binding for UI-side SCCP proof generation.
public func sccpEthereumMainnetDestinationBinding(
    verifierAddress: String,
    bridgeAddress: String,
    verifierCodeHash: String,
    verifierKeyHash: String,
    networkId: String = sccpEthereumMainnetNetworkId
) throws -> EvmSccpDestinationBinding {
    let binding = try sccpEvmDestinationBinding(
        sourceDomain: sccpDomainSora,
        targetDomain: sccpDomainEthereum,
        networkId: networkId,
        verifierAddress: verifierAddress,
        bridgeAddress: bridgeAddress,
        verifierCodeHash: verifierCodeHash,
        verifierKeyHash: verifierKeyHash
    )
    guard binding.networkId == sccpEthereumMainnetNetworkId else {
        throw SccpSourceProofHashError.invalidSourceMaterial("networkId")
    }
    return binding
}

/// Canonical governed Ethereum mainnet destination binding hash.
public func sccpEthereumMainnetDestinationBindingHash(
    verifierAddress: String,
    bridgeAddress: String,
    verifierCodeHash: String,
    verifierKeyHash: String,
    networkId: String = sccpEthereumMainnetNetworkId
) throws -> String {
    try sccpEthereumMainnetDestinationBinding(
        verifierAddress: verifierAddress,
        bridgeAddress: bridgeAddress,
        verifierCodeHash: verifierCodeHash,
        verifierKeyHash: verifierKeyHash,
        networkId: networkId
    ).hash
}

/// Governed BSC mainnet destination binding for UI-side SCCP proof generation.
public func sccpBscMainnetDestinationBinding(
    verifierAddress: String,
    bridgeAddress: String,
    verifierCodeHash: String,
    verifierKeyHash: String,
    networkId: String = sccpBscMainnetNetworkId
) throws -> EvmSccpDestinationBinding {
    let binding = try sccpEvmDestinationBinding(
        sourceDomain: sccpDomainSora,
        targetDomain: sccpDomainBsc,
        networkId: networkId,
        verifierAddress: verifierAddress,
        bridgeAddress: bridgeAddress,
        verifierCodeHash: verifierCodeHash,
        verifierKeyHash: verifierKeyHash
    )
    guard binding.networkId == sccpBscMainnetNetworkId else {
        throw SccpSourceProofHashError.invalidSourceMaterial("networkId")
    }
    return binding
}

/// Canonical governed BSC mainnet destination binding hash.
public func sccpBscMainnetDestinationBindingHash(
    verifierAddress: String,
    bridgeAddress: String,
    verifierCodeHash: String,
    verifierKeyHash: String,
    networkId: String = sccpBscMainnetNetworkId
) throws -> String {
    try sccpBscMainnetDestinationBinding(
        verifierAddress: verifierAddress,
        bridgeAddress: bridgeAddress,
        verifierCodeHash: verifierCodeHash,
        verifierKeyHash: verifierKeyHash,
        networkId: networkId
    ).hash
}

/// Governed TRON destination binding for UI-side SCCP proof generation.
public func sccpTronDestinationBinding(
    sourceDomain: UInt32 = sccpDomainSora,
    targetDomain: UInt32 = sccpDomainTron,
    networkId: String,
    verifierAddress: String,
    verifierCodeHash: String,
    verifierKeyHash: String,
    verifierBackend: String = sccpTronGroth16Bn254ProofBackendV1,
    proofFamily: String = sccpStarkFriProofFamilyV1
) throws -> TronSccpDestinationBinding {
    guard sourceDomain == sccpDomainSora else {
        throw SccpSourceProofHashError.unsupportedDestinationBindingDomain("sourceDomain")
    }
    guard targetDomain == sccpDomainTron else {
        throw SccpSourceProofHashError.unsupportedDestinationBindingDomain("targetDomain")
    }
    guard verifierBackend == sccpTronGroth16Bn254ProofBackendV1 else {
        throw SccpSourceProofHashError.invalidSourceMaterial("verifierBackend")
    }
    guard proofFamily == sccpStarkFriProofFamilyV1 else {
        throw SccpSourceProofHashError.invalidSourceMaterial("proofFamily")
    }

    let networkIdBytes = try sourceProofNonZeroBytesFromHex32(networkId, field: "networkId")
    let verifierPayload = try sourceProofTronBase58CheckPayload(
        verifierAddress,
        field: "verifierAddress"
    )
    let verifierCodeHashBytes = try sourceProofNonZeroBytesFromHex32(
        verifierCodeHash,
        field: "verifierCodeHash"
    )
    let verifierKeyHashBytes = try sourceProofNonZeroBytesFromHex32(
        verifierKeyHash,
        field: "verifierKeyHash"
    )
    let normalizedNetworkId = "0x" + networkIdBytes.hexEncodedString()
    let normalizedVerifierCodeHash = "0x" + verifierCodeHashBytes.hexEncodedString()
    let normalizedVerifierKeyHash = "0x" + verifierKeyHashBytes.hexEncodedString()
    let normalizedVerifierAddress = verifierAddress
    let key = [
        "tron",
        String(sourceDomain),
        String(targetDomain),
        networkIdBytes.hexEncodedString(),
        normalizedVerifierAddress,
        normalizedVerifierCodeHash,
        normalizedVerifierKeyHash
    ].joined(separator: ":")

    var payload = Data(irohaKeccak256(sccpTronDestinationBindingLabelV1))
    payload.append(irohaKeccak256(Data(verifierBackend.utf8)))
    payload.append(irohaKeccak256(Data(proofFamily.utf8)))
    payload.append(networkIdBytes)
    sourceProofAppendAbiU32(sourceDomain, to: &payload)
    sourceProofAppendAbiU32(targetDomain, to: &payload)
    sourceProofAppendAbiBytes21(verifierPayload, to: &payload)
    payload.append(verifierCodeHashBytes)
    payload.append(verifierKeyHashBytes)
    let hash = "0x" + irohaKeccak256(payload).hexEncodedString()

    return TronSccpDestinationBinding(
        version: 1,
        sourceDomain: sourceDomain,
        targetDomain: targetDomain,
        networkId: normalizedNetworkId,
        verifierAddress: normalizedVerifierAddress,
        verifierCodeHash: normalizedVerifierCodeHash,
        verifierKeyHash: normalizedVerifierKeyHash,
        verifierBackend: verifierBackend,
        proofFamily: proofFamily,
        key: key,
        hash: hash
    )
}

/// Canonical governed TRON destination binding hash for UI-side proof requests.
public func sccpTronDestinationBindingHash(
    sourceDomain: UInt32 = sccpDomainSora,
    targetDomain: UInt32 = sccpDomainTron,
    networkId: String,
    verifierAddress: String,
    verifierCodeHash: String,
    verifierKeyHash: String,
    verifierBackend: String = sccpTronGroth16Bn254ProofBackendV1,
    proofFamily: String = sccpStarkFriProofFamilyV1
) throws -> String {
    try sccpTronDestinationBinding(
        sourceDomain: sourceDomain,
        targetDomain: targetDomain,
        networkId: networkId,
        verifierAddress: verifierAddress,
        verifierCodeHash: verifierCodeHash,
        verifierKeyHash: verifierKeyHash,
        verifierBackend: verifierBackend,
        proofFamily: proofFamily
    ).hash
}

/// Canonical governed source-verifier material record bytes.
public func canonicalSccpSourceVerifierMaterialBytes(
    sourceDomain: UInt32,
    sourceTrustAnchorHash: String,
    consensusVerifierHash: String,
    messageInclusionVerifierHash: String,
    finalityPolicyHash: String,
    sourceStateVerifierHash: String? = nil,
    bridgeAddress: String? = nil,
    sourceBridgeEmitterCodeHash: String? = nil,
    networkId: String? = nil,
    ownerAddress: String? = nil,
    configHash: String? = nil
) throws -> Data {
    let material = try normalizeSccpSourceMaterial(
        sourceDomain: sourceDomain,
        sourceTrustAnchorHash: sourceTrustAnchorHash,
        consensusVerifierHash: consensusVerifierHash,
        messageInclusionVerifierHash: messageInclusionVerifierHash,
        finalityPolicyHash: finalityPolicyHash,
        sourceStateVerifierHash: sourceStateVerifierHash,
        bridgeAddress: bridgeAddress,
        sourceBridgeEmitterCodeHash: sourceBridgeEmitterCodeHash,
        networkId: networkId,
        ownerAddress: ownerAddress,
        configHash: configHash
    )
    var out = Data()
    appendSccpSourceMaterialFields(material, to: &out)
    out.append(0)
    return out
}

/// Canonical governed source-verifier material record hash.
public func sccpSourceVerifierMaterialHash(
    sourceDomain: UInt32,
    sourceTrustAnchorHash: String,
    consensusVerifierHash: String,
    messageInclusionVerifierHash: String,
    finalityPolicyHash: String,
    sourceStateVerifierHash: String? = nil,
    bridgeAddress: String? = nil,
    sourceBridgeEmitterCodeHash: String? = nil,
    networkId: String? = nil,
    ownerAddress: String? = nil,
    configHash: String? = nil
) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:source-verifier-material-record:v1",
        payload: canonicalSccpSourceVerifierMaterialBytes(
            sourceDomain: sourceDomain,
            sourceTrustAnchorHash: sourceTrustAnchorHash,
            consensusVerifierHash: consensusVerifierHash,
            messageInclusionVerifierHash: messageInclusionVerifierHash,
            finalityPolicyHash: finalityPolicyHash,
            sourceStateVerifierHash: sourceStateVerifierHash,
            bridgeAddress: bridgeAddress,
            sourceBridgeEmitterCodeHash: sourceBridgeEmitterCodeHash,
            networkId: networkId,
            ownerAddress: ownerAddress,
            configHash: configHash
        )
    )
}

/// Canonical governed source-adapter deployment record bytes.
public func canonicalSccpSourceAdapterEngineDeploymentBytes(
    sourceDomain: UInt32,
    sourceTrustAnchorHash: String,
    consensusVerifierHash: String,
    messageInclusionVerifierHash: String,
    finalityPolicyHash: String,
    deploymentReceiptHash: String,
    targetDomain: UInt32 = sccpDomainSora,
    adapterVerifierVkHash: String? = nil,
    sourceStateVerifierHash: String? = nil,
    bridgeAddress: String? = nil,
    sourceBridgeEmitterCodeHash: String? = nil,
    networkId: String? = nil,
    ownerAddress: String? = nil,
    configHash: String? = nil,
    solanaTowerReplayVerifierHash: String? = nil,
    solanaFullAccountsdbLatticeVerifierHash: String? = nil,
    solanaBankForkChoiceVerifierHash: String? = nil,
    tonMasterchainConfigVerifierHash: String? = nil,
    tonValidatorSetTransitionVerifierHash: String? = nil,
    tonShardAccountsDictionaryVerifierHash: String? = nil
) throws -> Data {
    guard targetDomain == sccpDomainSora else {
        throw SccpSourceProofHashError.unsupportedSourceAdapterDomain("targetDomain")
    }
    let material = try normalizeSccpSourceMaterial(
        sourceDomain: sourceDomain,
        sourceTrustAnchorHash: sourceTrustAnchorHash,
        consensusVerifierHash: consensusVerifierHash,
        messageInclusionVerifierHash: messageInclusionVerifierHash,
        finalityPolicyHash: finalityPolicyHash,
        sourceStateVerifierHash: sourceStateVerifierHash,
        bridgeAddress: bridgeAddress,
        sourceBridgeEmitterCodeHash: sourceBridgeEmitterCodeHash,
        networkId: networkId,
        ownerAddress: ownerAddress,
        configHash: configHash
    )
    let canonicalVkHash = try sccpSourceAdapterVerifierVkHash(
        sourceDomain: material.sourceDomain,
        targetDomain: targetDomain
    )
    let normalizedVkHash = try adapterVerifierVkHash.map {
        try sourceProofNormalizeHex32($0, field: "adapterVerifierVkHash")
    } ?? canonicalVkHash
    guard normalizedVkHash == canonicalVkHash else {
        throw SccpSourceProofHashError.invalidSourceMaterial("adapterVerifierVkHash")
    }
    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(material.sourceDomain, to: &out)
    sourceProofAppendU32Le(targetDomain, to: &out)
    sourceProofAppendDataVector(Data(material.profile.chain.utf8), to: &out)
    out.append(material.profile.proofPlan)
    out.append(material.profile.finalityModel)
    sourceProofAppendDataVector(Data(sccpStarkFriProofFamilyV1.utf8), to: &out)
    sourceProofAppendDataVector(Data(sccpSourceAdapterOpenVerifyCircuitIdV1.utf8), to: &out)
    let adapterVerifierVkHashData = try sourceProofBytesFromHex32(
        normalizedVkHash,
        field: "adapterVerifierVkHash"
    )
    let deploymentReceiptHashData = try sourceProofNonZeroBytesFromHex32(
        deploymentReceiptHash,
        field: "deploymentReceiptHash"
    )
    try requirePairwiseNonzeroSourceRoleHashSeparation(
        [
            ("sourceTrustAnchorHash", material.sourceTrustAnchorHash),
            ("consensusVerifierHash", material.consensusVerifierHash),
            ("messageInclusionVerifierHash", material.messageInclusionVerifierHash),
            ("finalityPolicyHash", material.finalityPolicyHash),
            ("sourceStateVerifierHash", material.sourceStateVerifierHash),
            ("adapterVerifierVkHash", adapterVerifierVkHashData),
            ("sourceBridgeEmitterCodeHash", material.sourceBridgeEmitterCodeHash),
            ("sourceBridgeNetworkId", material.sourceBridgeNetworkId),
            ("sourceBridgeConfigHash", material.sourceBridgeConfigHash),
            ("deploymentReceiptHash", deploymentReceiptHashData),
        ],
        label: "sourceAdapterDeploymentRoleHash"
    )
    out.append(adapterVerifierVkHashData)
    appendSccpSourceComponentFields(material, to: &out)
    out.append(deploymentReceiptHashData)
    try appendSccpSourceAdapterDeploymentSolanaAuditFields(
        sourceDomain: material.sourceDomain,
        towerReplayVerifierHash: solanaTowerReplayVerifierHash,
        fullAccountsdbLatticeVerifierHash: solanaFullAccountsdbLatticeVerifierHash,
        bankForkChoiceVerifierHash: solanaBankForkChoiceVerifierHash,
        existingRoleHashes: [
            material.sourceTrustAnchorHash,
            material.consensusVerifierHash,
            material.messageInclusionVerifierHash,
            material.finalityPolicyHash,
            material.sourceStateVerifierHash,
            adapterVerifierVkHashData,
            material.sourceBridgeEmitterCodeHash,
            material.sourceBridgeNetworkId,
            material.sourceBridgeConfigHash,
            deploymentReceiptHashData,
        ],
        to: &out
    )
    try appendSccpSourceAdapterDeploymentTonAuditFields(
        sourceDomain: material.sourceDomain,
        masterchainConfigVerifierHash: tonMasterchainConfigVerifierHash,
        validatorSetTransitionVerifierHash: tonValidatorSetTransitionVerifierHash,
        shardAccountsDictionaryVerifierHash: tonShardAccountsDictionaryVerifierHash,
        existingRoleHashes: [
            material.sourceTrustAnchorHash,
            material.consensusVerifierHash,
            material.messageInclusionVerifierHash,
            material.finalityPolicyHash,
            material.sourceStateVerifierHash,
            adapterVerifierVkHashData,
            material.sourceBridgeEmitterCodeHash,
            material.sourceBridgeNetworkId,
            material.sourceBridgeConfigHash,
            deploymentReceiptHashData,
        ],
        to: &out
    )
    return out
}

/// Canonical governed source-adapter deployment record hash.
public func sccpSourceAdapterEngineDeploymentHash(
    sourceDomain: UInt32,
    sourceTrustAnchorHash: String,
    consensusVerifierHash: String,
    messageInclusionVerifierHash: String,
    finalityPolicyHash: String,
    deploymentReceiptHash: String,
    targetDomain: UInt32 = sccpDomainSora,
    adapterVerifierVkHash: String? = nil,
    sourceStateVerifierHash: String? = nil,
    bridgeAddress: String? = nil,
    sourceBridgeEmitterCodeHash: String? = nil,
    networkId: String? = nil,
    ownerAddress: String? = nil,
    configHash: String? = nil,
    solanaTowerReplayVerifierHash: String? = nil,
    solanaFullAccountsdbLatticeVerifierHash: String? = nil,
    solanaBankForkChoiceVerifierHash: String? = nil,
    tonMasterchainConfigVerifierHash: String? = nil,
    tonValidatorSetTransitionVerifierHash: String? = nil,
    tonShardAccountsDictionaryVerifierHash: String? = nil
) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:source-adapter-engine-deployment:v1",
        payload: canonicalSccpSourceAdapterEngineDeploymentBytes(
            sourceDomain: sourceDomain,
            sourceTrustAnchorHash: sourceTrustAnchorHash,
            consensusVerifierHash: consensusVerifierHash,
            messageInclusionVerifierHash: messageInclusionVerifierHash,
            finalityPolicyHash: finalityPolicyHash,
            deploymentReceiptHash: deploymentReceiptHash,
            targetDomain: targetDomain,
            adapterVerifierVkHash: adapterVerifierVkHash,
            sourceStateVerifierHash: sourceStateVerifierHash,
            bridgeAddress: bridgeAddress,
            sourceBridgeEmitterCodeHash: sourceBridgeEmitterCodeHash,
            networkId: networkId,
            ownerAddress: ownerAddress,
            configHash: configHash,
            solanaTowerReplayVerifierHash: solanaTowerReplayVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash: solanaFullAccountsdbLatticeVerifierHash,
            solanaBankForkChoiceVerifierHash: solanaBankForkChoiceVerifierHash,
            tonMasterchainConfigVerifierHash: tonMasterchainConfigVerifierHash,
            tonValidatorSetTransitionVerifierHash: tonValidatorSetTransitionVerifierHash,
            tonShardAccountsDictionaryVerifierHash: tonShardAccountsDictionaryVerifierHash
        )
    )
}

/// Canonical Solana full light-client deployment-gate hash for audited verifier bundles.
public func sccpSolanaFullLightClientGateHash(
    sourceDomain: UInt32 = sccpDomainSolana,
    sourceTrustAnchorHash: String,
    consensusVerifierHash: String,
    messageInclusionVerifierHash: String,
    finalityPolicyHash: String,
    deploymentReceiptHash: String,
    solanaTowerReplayVerifierHash: String,
    solanaFullAccountsdbLatticeVerifierHash: String,
    solanaBankForkChoiceVerifierHash: String,
    targetDomain: UInt32 = sccpDomainSora,
    adapterVerifierVkHash: String? = nil,
    sourceStateVerifierHash: String? = nil,
    bridgeAddress: String? = nil,
    sourceBridgeEmitterCodeHash: String? = nil,
    networkId: String? = nil,
    ownerAddress: String? = nil,
    configHash: String? = nil
) throws -> String {
    guard sourceDomain == sccpDomainSolana, targetDomain == sccpDomainSora else {
        throw SccpSourceProofHashError.unsupportedSourceAdapterDomain("sourceDomain")
    }
    let material = try normalizeSccpSourceMaterial(
        sourceDomain: sourceDomain,
        sourceTrustAnchorHash: sourceTrustAnchorHash,
        consensusVerifierHash: consensusVerifierHash,
        messageInclusionVerifierHash: messageInclusionVerifierHash,
        finalityPolicyHash: finalityPolicyHash,
        sourceStateVerifierHash: sourceStateVerifierHash,
        bridgeAddress: bridgeAddress,
        sourceBridgeEmitterCodeHash: sourceBridgeEmitterCodeHash,
        networkId: networkId,
        ownerAddress: ownerAddress,
        configHash: configHash
    )
    let verifierHashes: [(String, String, Data)] = [
        (
            sccpSolanaMainnetTowerReplayVerifierIdV1,
            "solanaTowerReplayVerifierHash",
            try sourceProofNonZeroBytesFromHex32(
                solanaTowerReplayVerifierHash,
                field: "solanaTowerReplayVerifierHash"
            )
        ),
        (
            sccpSolanaMainnetFullAccountsdbLatticeVerifierIdV1,
            "solanaFullAccountsdbLatticeVerifierHash",
            try sourceProofNonZeroBytesFromHex32(
                solanaFullAccountsdbLatticeVerifierHash,
                field: "solanaFullAccountsdbLatticeVerifierHash"
            )
        ),
        (
            sccpSolanaMainnetBankForkChoiceVerifierIdV1,
            "solanaBankForkChoiceVerifierHash",
            try sourceProofNonZeroBytesFromHex32(
                solanaBankForkChoiceVerifierHash,
                field: "solanaBankForkChoiceVerifierHash"
            )
        ),
    ]
    let materialHash = try sccpSourceVerifierMaterialHash(
        sourceDomain: sourceDomain,
        sourceTrustAnchorHash: sourceTrustAnchorHash,
        consensusVerifierHash: consensusVerifierHash,
        messageInclusionVerifierHash: messageInclusionVerifierHash,
        finalityPolicyHash: finalityPolicyHash,
        sourceStateVerifierHash: sourceStateVerifierHash,
        bridgeAddress: bridgeAddress,
        sourceBridgeEmitterCodeHash: sourceBridgeEmitterCodeHash,
        networkId: networkId,
        ownerAddress: ownerAddress,
        configHash: configHash
    )
    let deploymentHash = try sccpSourceAdapterEngineDeploymentHash(
        sourceDomain: sourceDomain,
        sourceTrustAnchorHash: sourceTrustAnchorHash,
        consensusVerifierHash: consensusVerifierHash,
        messageInclusionVerifierHash: messageInclusionVerifierHash,
        finalityPolicyHash: finalityPolicyHash,
        deploymentReceiptHash: deploymentReceiptHash,
        targetDomain: targetDomain,
        adapterVerifierVkHash: adapterVerifierVkHash,
        sourceStateVerifierHash: sourceStateVerifierHash,
        bridgeAddress: bridgeAddress,
        sourceBridgeEmitterCodeHash: sourceBridgeEmitterCodeHash,
        networkId: networkId,
        ownerAddress: ownerAddress,
        configHash: configHash,
        solanaTowerReplayVerifierHash: solanaTowerReplayVerifierHash,
        solanaFullAccountsdbLatticeVerifierHash: solanaFullAccountsdbLatticeVerifierHash,
        solanaBankForkChoiceVerifierHash: solanaBankForkChoiceVerifierHash
    )
    let canonicalAdapterVerifierVkHash = try sccpSourceAdapterVerifierVkHash(
        sourceDomain: sourceDomain,
        targetDomain: targetDomain
    )
    let adapterVerifierVkHashData = try sourceProofBytesFromHex32(
        try adapterVerifierVkHash.map {
            try sourceProofNormalizeHex32($0, field: "adapterVerifierVkHash")
        } ?? canonicalAdapterVerifierVkHash,
        field: "adapterVerifierVkHash"
    )
    let deploymentReceiptHashData = try sourceProofNonZeroBytesFromHex32(
        deploymentReceiptHash,
        field: "deploymentReceiptHash"
    )
    try requireSolanaFullLightClientAuditRoleSeparation(
        verifierHashes: verifierHashes,
        existingRoleHashes: [
            material.sourceTrustAnchorHash,
            material.consensusVerifierHash,
            material.messageInclusionVerifierHash,
            material.finalityPolicyHash,
            material.sourceStateVerifierHash,
            adapterVerifierVkHashData,
            material.sourceBridgeEmitterCodeHash,
            material.sourceBridgeNetworkId,
            material.sourceBridgeConfigHash,
            deploymentReceiptHashData,
        ]
    )

    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(material.sourceDomain, to: &out)
    sourceProofAppendU32Le(targetDomain, to: &out)
    sourceProofAppendDataVector(Data(material.profile.chain.utf8), to: &out)
    out.append(material.profile.proofPlan)
    out.append(material.profile.finalityModel)
    sourceProofAppendDataVector(Data(sccpSolanaMainnetGenesisHash.utf8), to: &out)
    try out.append(sourceProofBytesFromHex32(materialHash, field: "sourceVerifierMaterialHash"))
    try out.append(sourceProofBytesFromHex32(deploymentHash, field: "sourceAdapterDeploymentHash"))
    for (verifierId, _, verifierHash) in verifierHashes {
        sourceProofAppendDataVector(Data(verifierId.utf8), to: &out)
        out.append(verifierHash)
    }
    return try sourceProofHashHex(prefix: "sccp:solana:full-light-client-gate:v1", payload: out)
}

/// Canonical TON full light-client deployment-gate hash for audited verifier bundles.
public func sccpTonFullLightClientGateHash(
    sourceDomain: UInt32 = sccpDomainTon,
    sourceTrustAnchorHash: String,
    consensusVerifierHash: String,
    messageInclusionVerifierHash: String,
    finalityPolicyHash: String,
    deploymentReceiptHash: String,
    tonMasterchainConfigVerifierHash: String,
    tonValidatorSetTransitionVerifierHash: String,
    tonShardAccountsDictionaryVerifierHash: String,
    targetDomain: UInt32 = sccpDomainSora,
    adapterVerifierVkHash: String? = nil,
    sourceStateVerifierHash: String? = nil,
    bridgeAddress: String? = nil,
    sourceBridgeEmitterCodeHash: String? = nil,
    networkId: String? = nil,
    ownerAddress: String? = nil,
    configHash: String? = nil
) throws -> String {
    guard sourceDomain == sccpDomainTon, targetDomain == sccpDomainSora else {
        throw SccpSourceProofHashError.unsupportedSourceAdapterDomain("sourceDomain")
    }
    let material = try normalizeSccpSourceMaterial(
        sourceDomain: sourceDomain,
        sourceTrustAnchorHash: sourceTrustAnchorHash,
        consensusVerifierHash: consensusVerifierHash,
        messageInclusionVerifierHash: messageInclusionVerifierHash,
        finalityPolicyHash: finalityPolicyHash,
        sourceStateVerifierHash: sourceStateVerifierHash,
        bridgeAddress: bridgeAddress,
        sourceBridgeEmitterCodeHash: sourceBridgeEmitterCodeHash,
        networkId: networkId,
        ownerAddress: ownerAddress,
        configHash: configHash
    )
    let verifierHashes: [(String, String, Data)] = [
        (
            sccpTonMainnetMasterchainConfigVerifierIdV1,
            "tonMasterchainConfigVerifierHash",
            try sourceProofNonZeroBytesFromHex32(
                tonMasterchainConfigVerifierHash,
                field: "tonMasterchainConfigVerifierHash"
            )
        ),
        (
            sccpTonMainnetValidatorSetTransitionVerifierIdV1,
            "tonValidatorSetTransitionVerifierHash",
            try sourceProofNonZeroBytesFromHex32(
                tonValidatorSetTransitionVerifierHash,
                field: "tonValidatorSetTransitionVerifierHash"
            )
        ),
        (
            sccpTonMainnetShardAccountsDictionaryVerifierIdV1,
            "tonShardAccountsDictionaryVerifierHash",
            try sourceProofNonZeroBytesFromHex32(
                tonShardAccountsDictionaryVerifierHash,
                field: "tonShardAccountsDictionaryVerifierHash"
            )
        ),
    ]
    let auditExistingAdapterVerifierVkHash = try sourceProofBytesFromHex32(
        adapterVerifierVkHash ?? sccpSourceAdapterVerifierVkHash(
            sourceDomain: sourceDomain,
            targetDomain: targetDomain
        ),
        field: "adapterVerifierVkHash"
    )
    let auditExistingDeploymentReceiptHash = try sourceProofNonZeroBytesFromHex32(
        deploymentReceiptHash,
        field: "deploymentReceiptHash"
    )
    try requireTonFullLightClientAuditRoleSeparation(
        verifierHashes: verifierHashes,
        existingRoleHashes: [
            material.sourceTrustAnchorHash,
            material.consensusVerifierHash,
            material.messageInclusionVerifierHash,
            material.finalityPolicyHash,
            material.sourceStateVerifierHash,
            auditExistingAdapterVerifierVkHash,
            material.sourceBridgeEmitterCodeHash,
            material.sourceBridgeNetworkId,
            material.sourceBridgeConfigHash,
            auditExistingDeploymentReceiptHash,
        ]
    )
    let materialHash = try sccpSourceVerifierMaterialHash(
        sourceDomain: sourceDomain,
        sourceTrustAnchorHash: sourceTrustAnchorHash,
        consensusVerifierHash: consensusVerifierHash,
        messageInclusionVerifierHash: messageInclusionVerifierHash,
        finalityPolicyHash: finalityPolicyHash,
        sourceStateVerifierHash: sourceStateVerifierHash,
        bridgeAddress: bridgeAddress,
        sourceBridgeEmitterCodeHash: sourceBridgeEmitterCodeHash,
        networkId: networkId,
        ownerAddress: ownerAddress,
        configHash: configHash
    )
    let deploymentHash = try sccpSourceAdapterEngineDeploymentHash(
        sourceDomain: sourceDomain,
        sourceTrustAnchorHash: sourceTrustAnchorHash,
        consensusVerifierHash: consensusVerifierHash,
        messageInclusionVerifierHash: messageInclusionVerifierHash,
        finalityPolicyHash: finalityPolicyHash,
        deploymentReceiptHash: deploymentReceiptHash,
        targetDomain: targetDomain,
        adapterVerifierVkHash: adapterVerifierVkHash,
        sourceStateVerifierHash: sourceStateVerifierHash,
        bridgeAddress: bridgeAddress,
        sourceBridgeEmitterCodeHash: sourceBridgeEmitterCodeHash,
        networkId: networkId,
        ownerAddress: ownerAddress,
        configHash: configHash,
        tonMasterchainConfigVerifierHash: tonMasterchainConfigVerifierHash,
        tonValidatorSetTransitionVerifierHash: tonValidatorSetTransitionVerifierHash,
        tonShardAccountsDictionaryVerifierHash: tonShardAccountsDictionaryVerifierHash
    )

    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(material.sourceDomain, to: &out)
    sourceProofAppendU32Le(targetDomain, to: &out)
    sourceProofAppendDataVector(Data(material.profile.chain.utf8), to: &out)
    out.append(material.profile.proofPlan)
    out.append(material.profile.finalityModel)
    var globalId = Int32(-239).littleEndian
    withUnsafeBytes(of: &globalId) { out.append(contentsOf: $0) }
    sourceProofAppendDataVector(Data(sccpTonShardStateOpenVerifyCircuitIdV1.utf8), to: &out)
    sourceProofAppendDataVector(Data(sccpSourceAdapterFastpqParameterSetV1.utf8), to: &out)
    sourceProofAppendDataVector(Data(material.profile.sourceStateVerifierId.utf8), to: &out)
    out.append(material.sourceStateVerifierHash)
    try out.append(sourceProofBytesFromHex32(materialHash, field: "sourceVerifierMaterialHash"))
    try out.append(sourceProofBytesFromHex32(deploymentHash, field: "sourceAdapterDeploymentHash"))
    for (verifierId, _, verifierHash) in verifierHashes {
        sourceProofAppendDataVector(Data(verifierId.utf8), to: &out)
        out.append(verifierHash)
    }
    return try sourceProofHashHex(prefix: "sccp:ton:full-light-client-gate:v1", payload: out)
}

/// Canonical Ethereum receipt-proof transcript bytes checked by the SCCP source adapter.
public func canonicalEvmSccpReceiptProofBytes(sourceDomain: UInt32 = sccpDomainEthereum,
                                              sourceEventDigest: String,
                                              beaconSlot: UInt64,
                                              executionBlockNumber: UInt64,
                                              executionBlockHash: String,
                                              executionReceiptsRoot: String,
                                              beaconFinalizedRoot: String,
                                              syncCommitteeRoot: String,
                                              receiptRootIndex: UInt64,
                                              receiptTrieProofNodes: [Data],
                                              inclusionBranch: [Data]) throws -> Data {
    guard sourceDomain == sccpDomainEthereum else {
        throw SccpSourceProofHashError.invalidValidatorSet("sourceDomain")
    }
    try sourceProofValidateTronMptProofNodes(receiptTrieProofNodes)
    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(sourceDomain, to: &out)
    try out.append(sourceProofNonZeroBytesFromHex32(sourceEventDigest, field: "sourceEventDigest"))
    sourceProofAppendU64Le(beaconSlot, to: &out)
    sourceProofAppendU64Le(executionBlockNumber, to: &out)
    try out.append(sourceProofBytesFromHex32(executionBlockHash, field: "executionBlockHash"))
    try out.append(sourceProofBytesFromHex32(executionReceiptsRoot, field: "executionReceiptsRoot"))
    try out.append(sourceProofBytesFromHex32(beaconFinalizedRoot, field: "beaconFinalizedRoot"))
    try out.append(sourceProofBytesFromHex32(syncCommitteeRoot, field: "syncCommitteeRoot"))
    sourceProofAppendU64Le(receiptRootIndex, to: &out)
    sourceProofAppendU32Le(UInt32(receiptTrieProofNodes.count), to: &out)
    for node in receiptTrieProofNodes {
        sourceProofAppendDataVector(node, to: &out)
    }
    try sourceProofAppendBranch(inclusionBranch, to: &out, requireNonEmpty: true)
    return out
}

/// Hash of the canonical Ethereum receipt-proof transcript checked by the source adapter.
public func evmSccpReceiptProofHash(sourceDomain: UInt32 = sccpDomainEthereum,
                                    sourceEventDigest: String,
                                    beaconSlot: UInt64,
                                    executionBlockNumber: UInt64,
                                    executionBlockHash: String,
                                    executionReceiptsRoot: String,
                                    beaconFinalizedRoot: String,
                                    syncCommitteeRoot: String,
                                    receiptRootIndex: UInt64,
                                    receiptTrieProofNodes: [Data],
                                    inclusionBranch: [Data]) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:evm:receipt-proof:v1",
        payload: canonicalEvmSccpReceiptProofBytes(
            sourceDomain: sourceDomain,
            sourceEventDigest: sourceEventDigest,
            beaconSlot: beaconSlot,
            executionBlockNumber: executionBlockNumber,
            executionBlockHash: executionBlockHash,
            executionReceiptsRoot: executionReceiptsRoot,
            beaconFinalizedRoot: beaconFinalizedRoot,
            syncCommitteeRoot: syncCommitteeRoot,
            receiptRootIndex: receiptRootIndex,
            receiptTrieProofNodes: receiptTrieProofNodes,
            inclusionBranch: inclusionBranch
        )
    )
}

/// Canonical ETH sync-committee payload bytes checked by transition proofs.
public func canonicalEthSyncCommitteePayloadBytes(syncCommitteePublicKeys: [Data],
                                                  syncCommitteeWeights: [UInt64],
                                                  syncCommitteePops: [Data]) throws -> Data {
    guard syncCommitteePublicKeys.count == syncCommitteeWeights.count,
          syncCommitteePublicKeys.count == syncCommitteePops.count else {
        throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePublicKeys")
    }
    guard syncCommitteePublicKeys.count <= Int(UInt32.max) else {
        throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePublicKeys")
    }
    guard syncCommitteePublicKeys.count == sccpEthMainnetSyncCommitteeAuthorities else {
        throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePublicKeys")
    }
    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(UInt32(syncCommitteePublicKeys.count), to: &out)
    var seenPublicKeys = Set<String>()
    for index in syncCommitteePublicKeys.indices {
        let publicKey = syncCommitteePublicKeys[index]
        guard publicKey.count == sccpEthSyncCommitteePublicKeyBytes else {
            throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePublicKeys[\(index)]")
        }
        guard publicKey.contains(where: { $0 != 0 }) else {
            throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePublicKeys[\(index)]")
        }
        guard seenPublicKeys.insert(publicKey.hexEncodedString()).inserted else {
            throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePublicKeys[\(index)]")
        }
        guard syncCommitteeWeights[index] == 1 else {
            throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteeWeights[\(index)]")
        }
        let pop = syncCommitteePops[index]
        guard pop.count == sccpEthSyncCommitteePopBytes else {
            throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePops[\(index)]")
        }
        guard pop.contains(where: { $0 != 0 }) else {
            throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePops[\(index)]")
        }
        sourceProofAppendDataVector(publicKey, to: &out)
        sourceProofAppendU64Le(syncCommitteeWeights[index], to: &out)
        sourceProofAppendDataVector(pop, to: &out)
    }
    return out
}

/// SCCP ETH sync-committee hash derived from a canonical sync-committee payload.
public func ethSyncCommitteeHashFromPayload(payload: Data) throws -> String {
    try sourceProofValidateEthSyncCommitteePayload(payload)
    return try sourceProofHashHex(prefix: "sccp:eth:sync-committee:v1", payload: payload)
}

/// SCCP ETH sync-committee hash derived from committee witness material.
public func ethSyncCommitteeHash(syncCommitteePublicKeys: [Data],
                                 syncCommitteeWeights: [UInt64],
                                 syncCommitteePops: [Data]) throws -> String {
    try ethSyncCommitteeHashFromPayload(
        payload: canonicalEthSyncCommitteePayloadBytes(
            syncCommitteePublicKeys: syncCommitteePublicKeys,
            syncCommitteeWeights: syncCommitteeWeights,
            syncCommitteePops: syncCommitteePops
        )
    )
}

/// Hash of the canonical ETH sync-committee transition payload.
public func ethSyncCommitteePayloadHash(payload: Data) throws -> String {
    try sourceProofValidateEthSyncCommitteePayload(payload)
    return try sourceProofHashHex(prefix: "sccp:eth:sync-committee-payload:v1", payload: payload)
}

/// Hash of the canonical ETH sync-committee transition payload.
public func ethSyncCommitteePayloadHash(syncCommitteePublicKeys: [Data],
                                        syncCommitteeWeights: [UInt64],
                                        syncCommitteePops: [Data]) throws -> String {
    try ethSyncCommitteePayloadHash(
        payload: canonicalEthSyncCommitteePayloadBytes(
            syncCommitteePublicKeys: syncCommitteePublicKeys,
            syncCommitteeWeights: syncCommitteeWeights,
            syncCommitteePops: syncCommitteePops
        )
    )
}

/// Return the Ethereum mainnet sync-committee period for a beacon slot.
public func ethMainnetSyncCommitteePeriodForSlot(_ slot: UInt64) -> UInt64 {
    slot / sccpEthMainnetSlotsPerSyncCommitteePeriod
}

private func requireEthMainnetTransitionPeriods(fromSyncPeriod: UInt64,
                                                toSyncPeriod: UInt64,
                                                transitionSlot: UInt64) throws {
    guard transitionSlot != 0 else {
        throw SccpSourceProofHashError.invalidValidatorSet("transitionSlot")
    }
    let nextPeriod = fromSyncPeriod.addingReportingOverflow(1)
    guard !nextPeriod.overflow,
          nextPeriod.partialValue == toSyncPeriod else {
        throw SccpSourceProofHashError.invalidValidatorSet("toSyncPeriod")
    }
    guard ethMainnetSyncCommitteePeriodForSlot(transitionSlot) == fromSyncPeriod else {
        throw SccpSourceProofHashError.invalidValidatorSet("transitionSlot")
    }
}

/// Canonical ETH sync-committee transition message bytes.
public func canonicalEthSyncCommitteeTransitionMessageBytes(sourceDomain: UInt32 = sccpDomainEthereum,
                                                            fromSyncPeriod: UInt64,
                                                            toSyncPeriod: UInt64,
                                                            transitionSlot: UInt64,
                                                            finalizedBeaconRoot: String,
                                                            parentSyncCommitteeHash: String,
                                                            nextSyncCommitteeHash: String,
                                                            nextSyncCommitteePayloadHash: String,
                                                            nextSyncCommitteeBranchHash: String) throws -> Data {
    guard sourceDomain == sccpDomainEthereum else {
        throw SccpSourceProofHashError.invalidValidatorSet("sourceDomain")
    }
    try requireEthMainnetTransitionPeriods(
        fromSyncPeriod: fromSyncPeriod,
        toSyncPeriod: toSyncPeriod,
        transitionSlot: transitionSlot
    )
    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(sourceDomain, to: &out)
    sourceProofAppendU64Le(fromSyncPeriod, to: &out)
    sourceProofAppendU64Le(toSyncPeriod, to: &out)
    sourceProofAppendU64Le(transitionSlot, to: &out)
    try out.append(sourceProofBytesFromHex32(finalizedBeaconRoot, field: "finalizedBeaconRoot"))
    try out.append(sourceProofBytesFromHex32(parentSyncCommitteeHash, field: "parentSyncCommitteeHash"))
    try out.append(sourceProofBytesFromHex32(nextSyncCommitteeHash, field: "nextSyncCommitteeHash"))
    try out.append(sourceProofBytesFromHex32(nextSyncCommitteePayloadHash, field: "nextSyncCommitteePayloadHash"))
    try out.append(sourceProofBytesFromHex32(nextSyncCommitteeBranchHash, field: "nextSyncCommitteeBranchHash"))
    return out
}

/// Hash of the canonical ETH sync-committee transition message transcript.
public func ethSyncCommitteeTransitionMessageHash(sourceDomain: UInt32 = sccpDomainEthereum,
                                                  fromSyncPeriod: UInt64,
                                                  toSyncPeriod: UInt64,
                                                  transitionSlot: UInt64,
                                                  finalizedBeaconRoot: String,
                                                  parentSyncCommitteeHash: String,
                                                  nextSyncCommitteeHash: String,
                                                  nextSyncCommitteePayloadHash: String,
                                                  nextSyncCommitteeBranchHash: String) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:eth:sync-committee-transition-message:v1",
        payload: canonicalEthSyncCommitteeTransitionMessageBytes(
            sourceDomain: sourceDomain,
            fromSyncPeriod: fromSyncPeriod,
            toSyncPeriod: toSyncPeriod,
            transitionSlot: transitionSlot,
            finalizedBeaconRoot: finalizedBeaconRoot,
            parentSyncCommitteeHash: parentSyncCommitteeHash,
            nextSyncCommitteeHash: nextSyncCommitteeHash,
            nextSyncCommitteePayloadHash: nextSyncCommitteePayloadHash,
            nextSyncCommitteeBranchHash: nextSyncCommitteeBranchHash
        )
    )
}

/// Canonical ETH beacon sync-committee proof bytes embedded in transition transcripts.
public func canonicalEthBeaconSyncCommitteeProofBytes(version: UInt8 = 1,
                                                       totalWeight: UInt64,
                                                       signedWeight: UInt64,
                                                       syncCommitteeMessageHash: String,
                                                       syncCommitteePublicKeys: [Data],
                                                       syncCommitteeWeights: [UInt64],
                                                       syncCommitteePops: [Data],
                                                       signersBitmap: Data,
                                                       aggregateSignature: Data) throws -> Data {
    guard version == 1 else {
        throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteeProof.version")
    }
    _ = try canonicalEthSyncCommitteePayloadBytes(
        syncCommitteePublicKeys: syncCommitteePublicKeys,
        syncCommitteeWeights: syncCommitteeWeights,
        syncCommitteePops: syncCommitteePops
    )
    var computedTotalWeight: UInt64 = 0
    for weight in syncCommitteeWeights {
        let sum = computedTotalWeight.addingReportingOverflow(weight)
        guard !sum.overflow else {
            throw SccpSourceProofHashError.invalidValidatorSet("totalWeight")
        }
        computedTotalWeight = sum.partialValue
    }
    guard totalWeight == computedTotalWeight else {
        throw SccpSourceProofHashError.invalidValidatorSet("totalWeight")
    }
    guard signersBitmap.count == (syncCommitteePublicKeys.count + 7) / 8 else {
        throw SccpSourceProofHashError.invalidValidatorSet("signersBitmap")
    }
    var signerIndices: [Int] = []
    for (byteIndex, value) in signersBitmap.enumerated() {
        for bit in 0 ..< 8 where ((value >> UInt8(bit)) & 1) == 1 {
            let signerIndex = byteIndex * 8 + bit
            guard signerIndex < syncCommitteePublicKeys.count else {
                throw SccpSourceProofHashError.invalidValidatorSet("signersBitmap")
            }
            signerIndices.append(signerIndex)
        }
    }
    guard !signerIndices.isEmpty else {
        throw SccpSourceProofHashError.invalidValidatorSet("signersBitmap")
    }
    var computedSignedWeight: UInt64 = 0
    for signerIndex in signerIndices {
        let sum = computedSignedWeight.addingReportingOverflow(syncCommitteeWeights[signerIndex])
        guard !sum.overflow else {
            throw SccpSourceProofHashError.invalidValidatorSet("signedWeight")
        }
        computedSignedWeight = sum.partialValue
    }
    guard signedWeight == computedSignedWeight else {
        throw SccpSourceProofHashError.invalidValidatorSet("signedWeight")
    }
    let floorTwoThirds = (totalWeight / 3) * 2 + ((totalWeight % 3) * 2) / 3
    guard signedWeight > floorTwoThirds else {
        throw SccpSourceProofHashError.invalidValidatorSet("signedWeight")
    }
    guard aggregateSignature.count == sccpEthSyncCommitteeSignatureBytes else {
        throw SccpSourceProofHashError.invalidValidatorSet("aggregateSignature")
    }
    guard aggregateSignature.contains(where: { $0 != 0 }) else {
        throw SccpSourceProofHashError.invalidValidatorSet("aggregateSignature")
    }
    var out = Data()
    out.append(version)
    sourceProofAppendU64Le(totalWeight, to: &out)
    sourceProofAppendU64Le(signedWeight, to: &out)
    try out.append(sourceProofBytesFromHex32(syncCommitteeMessageHash, field: "syncCommitteeMessageHash"))
    sourceProofAppendU32Le(UInt32(syncCommitteePublicKeys.count), to: &out)
    for publicKey in syncCommitteePublicKeys {
        sourceProofAppendDataVector(publicKey, to: &out)
    }
    sourceProofAppendU32Le(UInt32(syncCommitteeWeights.count), to: &out)
    for weight in syncCommitteeWeights {
        sourceProofAppendU64Le(weight, to: &out)
    }
    sourceProofAppendU32Le(UInt32(syncCommitteePops.count), to: &out)
    for pop in syncCommitteePops {
        sourceProofAppendDataVector(pop, to: &out)
    }
    sourceProofAppendDataVector(signersBitmap, to: &out)
    sourceProofAppendDataVector(aggregateSignature, to: &out)
    return out
}

/// Canonical ETH sync-committee transition signature bytes.
public func canonicalEthSyncCommitteeTransitionSignatureBytes(version: UInt8 = 1,
                                                              sourceDomain: UInt32 = sccpDomainEthereum,
                                                              fromSyncPeriod: UInt64,
                                                              toSyncPeriod: UInt64,
                                                              transitionSlot: UInt64,
                                                              finalizedBeaconRoot: String,
                                                              parentSyncCommitteeHash: String,
                                                              nextSyncCommitteeHash: String,
                                                              nextSyncCommitteePayload: Data,
                                                              nextSyncCommitteePayloadHash: String,
                                                              nextSyncCommitteeBranchHash: String,
                                                              transitionMessageHash: String,
                                                              proofVersion: UInt8 = 1,
                                                              totalWeight: UInt64,
                                                              signedWeight: UInt64,
                                                              syncCommitteePublicKeys: [Data],
                                                              syncCommitteeWeights: [UInt64],
                                                              syncCommitteePops: [Data],
                                                              signersBitmap: Data,
                                                              aggregateSignature: Data) throws -> Data {
    guard version == 1 else {
        throw SccpSourceProofHashError.invalidValidatorSet("ETH sync-committee transition signature version")
    }
    guard sourceDomain == sccpDomainEthereum else {
        throw SccpSourceProofHashError.invalidValidatorSet("sourceDomain")
    }
    try requireEthMainnetTransitionPeriods(
        fromSyncPeriod: fromSyncPeriod,
        toSyncPeriod: toSyncPeriod,
        transitionSlot: transitionSlot
    )
    let derivedPayloadHash = try ethSyncCommitteePayloadHash(payload: nextSyncCommitteePayload)
    guard derivedPayloadHash.lowercased() == nextSyncCommitteePayloadHash.lowercased() else {
        throw SccpSourceProofHashError.invalidValidatorSet("nextSyncCommitteePayloadHash")
    }
    let derivedNextHash = try ethSyncCommitteeHashFromPayload(payload: nextSyncCommitteePayload)
    guard derivedNextHash.lowercased() == nextSyncCommitteeHash.lowercased() else {
        throw SccpSourceProofHashError.invalidValidatorSet("nextSyncCommitteeHash")
    }
    let parentHash = try ethSyncCommitteeHash(
        syncCommitteePublicKeys: syncCommitteePublicKeys,
        syncCommitteeWeights: syncCommitteeWeights,
        syncCommitteePops: syncCommitteePops
    )
    var out = Data()
    out.append(version)
    sourceProofAppendU32Le(sourceDomain, to: &out)
    sourceProofAppendU64Le(fromSyncPeriod, to: &out)
    sourceProofAppendU64Le(toSyncPeriod, to: &out)
    sourceProofAppendU64Le(transitionSlot, to: &out)
    try out.append(sourceProofBytesFromHex32(finalizedBeaconRoot, field: "finalizedBeaconRoot"))
    try out.append(sourceProofBytesFromHex32(parentSyncCommitteeHash, field: "parentSyncCommitteeHash"))
    try out.append(sourceProofBytesFromHex32(nextSyncCommitteeHash, field: "nextSyncCommitteeHash"))
    sourceProofAppendDataVector(nextSyncCommitteePayload, to: &out)
    try out.append(sourceProofBytesFromHex32(nextSyncCommitteePayloadHash, field: "nextSyncCommitteePayloadHash"))
    try out.append(sourceProofBytesFromHex32(nextSyncCommitteeBranchHash, field: "nextSyncCommitteeBranchHash"))
    try out.append(sourceProofBytesFromHex32(transitionMessageHash, field: "transitionMessageHash"))
    try out.append(sourceProofBytesFromHex32(parentHash, field: "parentSyncCommitteeHash"))
    try out.append(
        canonicalEthBeaconSyncCommitteeProofBytes(
            version: proofVersion,
            totalWeight: totalWeight,
            signedWeight: signedWeight,
            syncCommitteeMessageHash: transitionMessageHash,
            syncCommitteePublicKeys: syncCommitteePublicKeys,
            syncCommitteeWeights: syncCommitteeWeights,
            syncCommitteePops: syncCommitteePops,
            signersBitmap: signersBitmap,
            aggregateSignature: aggregateSignature
        )
    )
    return out
}

/// Hash of the canonical ETH sync-committee transition signature transcript.
public func ethSyncCommitteeTransitionSignatureHash(version: UInt8 = 1,
                                                    sourceDomain: UInt32 = sccpDomainEthereum,
                                                    fromSyncPeriod: UInt64,
                                                    toSyncPeriod: UInt64,
                                                    transitionSlot: UInt64,
                                                    finalizedBeaconRoot: String,
                                                    parentSyncCommitteeHash: String,
                                                    nextSyncCommitteeHash: String,
                                                    nextSyncCommitteePayload: Data,
                                                    nextSyncCommitteePayloadHash: String,
                                                    nextSyncCommitteeBranchHash: String,
                                                    transitionMessageHash: String,
                                                    proofVersion: UInt8 = 1,
                                                    totalWeight: UInt64,
                                                    signedWeight: UInt64,
                                                    syncCommitteePublicKeys: [Data],
                                                    syncCommitteeWeights: [UInt64],
                                                    syncCommitteePops: [Data],
                                                    signersBitmap: Data,
                                                    aggregateSignature: Data) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:eth:sync-committee-transition-signature:v1",
        payload: canonicalEthSyncCommitteeTransitionSignatureBytes(
            version: version,
            sourceDomain: sourceDomain,
            fromSyncPeriod: fromSyncPeriod,
            toSyncPeriod: toSyncPeriod,
            transitionSlot: transitionSlot,
            finalizedBeaconRoot: finalizedBeaconRoot,
            parentSyncCommitteeHash: parentSyncCommitteeHash,
            nextSyncCommitteeHash: nextSyncCommitteeHash,
            nextSyncCommitteePayload: nextSyncCommitteePayload,
            nextSyncCommitteePayloadHash: nextSyncCommitteePayloadHash,
            nextSyncCommitteeBranchHash: nextSyncCommitteeBranchHash,
            transitionMessageHash: transitionMessageHash,
            proofVersion: proofVersion,
            totalWeight: totalWeight,
            signedWeight: signedWeight,
            syncCommitteePublicKeys: syncCommitteePublicKeys,
            syncCommitteeWeights: syncCommitteeWeights,
            syncCommitteePops: syncCommitteePops,
            signersBitmap: signersBitmap,
            aggregateSignature: aggregateSignature
        )
    )
}

/// Canonical BSC receipt-proof transcript bytes checked by the SCCP source adapter.
public func canonicalBscSccpReceiptProofBytes(sourceDomain: UInt32 = sccpDomainBsc,
                                              sourceEventDigest: String,
                                              validatorEpoch: UInt64,
                                              blockNumber: UInt64,
                                              blockHash: String,
                                              receiptsRoot: String,
                                              validatorSetHash: String,
                                              commitSealHash: String,
                                              receiptRootIndex: UInt64,
                                              receiptTrieProofNodes: [Data],
                                              inclusionBranch: [Data]) throws -> Data {
    guard sourceDomain == sccpDomainBsc else {
        throw SccpSourceProofHashError.invalidValidatorSet("sourceDomain")
    }
    try sourceProofValidateTronMptProofNodes(receiptTrieProofNodes)
    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(sourceDomain, to: &out)
    try out.append(sourceProofNonZeroBytesFromHex32(sourceEventDigest, field: "sourceEventDigest"))
    sourceProofAppendU64Le(validatorEpoch, to: &out)
    sourceProofAppendU64Le(blockNumber, to: &out)
    try out.append(sourceProofBytesFromHex32(blockHash, field: "blockHash"))
    try out.append(sourceProofBytesFromHex32(receiptsRoot, field: "receiptsRoot"))
    try out.append(sourceProofBytesFromHex32(validatorSetHash, field: "validatorSetHash"))
    try out.append(sourceProofBytesFromHex32(commitSealHash, field: "commitSealHash"))
    sourceProofAppendU64Le(receiptRootIndex, to: &out)
    sourceProofAppendU32Le(UInt32(receiptTrieProofNodes.count), to: &out)
    for node in receiptTrieProofNodes {
        sourceProofAppendDataVector(node, to: &out)
    }
    try sourceProofAppendBranch(inclusionBranch, to: &out, requireNonEmpty: true)
    return out
}

/// Hash of the canonical BSC receipt-proof transcript checked by the source adapter.
public func bscSccpReceiptProofHash(sourceDomain: UInt32 = sccpDomainBsc,
                                    sourceEventDigest: String,
                                    validatorEpoch: UInt64,
                                    blockNumber: UInt64,
                                    blockHash: String,
                                    receiptsRoot: String,
                                    validatorSetHash: String,
                                    commitSealHash: String,
                                    receiptRootIndex: UInt64,
                                    receiptTrieProofNodes: [Data],
                                    inclusionBranch: [Data]) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:bsc:receipt-proof:v1",
        payload: canonicalBscSccpReceiptProofBytes(
            sourceDomain: sourceDomain,
            sourceEventDigest: sourceEventDigest,
            validatorEpoch: validatorEpoch,
            blockNumber: blockNumber,
            blockHash: blockHash,
            receiptsRoot: receiptsRoot,
            validatorSetHash: validatorSetHash,
            commitSealHash: commitSealHash,
            receiptRootIndex: receiptRootIndex,
            receiptTrieProofNodes: receiptTrieProofNodes,
            inclusionBranch: inclusionBranch
        )
    )
}

/// Canonical BSC validator-set payload bytes checked by transition proofs.
public func canonicalBscValidatorSetPayloadBytes(validatorAddresses: [String],
                                                 validatorPowers: [UInt64]) throws -> Data {
    guard !validatorAddresses.isEmpty, validatorAddresses.count == validatorPowers.count else {
        throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses")
    }
    guard validatorAddresses.count <= Int(UInt32.max) else {
        throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses")
    }
    guard validatorAddresses.count <= sccpBscMaxParliaValidators else {
        throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses")
    }
    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(UInt32(validatorAddresses.count), to: &out)
    var seenAddresses = Set<String>()
    for (index, pair) in zip(validatorAddresses, validatorPowers).enumerated() {
        let address = try sourceProofBytesFromHex20(pair.0, field: "validatorAddresses[\(index)]")
        guard address.contains(where: { $0 != 0 }) else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses[\(index)]")
        }
        let addressHex = address.hexEncodedString()
        guard seenAddresses.insert(addressHex).inserted else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses[\(index)]")
        }
        guard pair.1 != 0 else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorPowers[\(index)]")
        }
        out.append(address)
        sourceProofAppendU64Le(pair.1, to: &out)
    }
    return out
}

/// Hash of the canonical BSC validator-set transition payload.
public func bscValidatorSetPayloadHash(payload: Data) -> String {
    sourceProofKeccakHashHex(prefix: "sccp:bsc:validator-set-payload:v1", payload: payload)
}

/// Hash of the canonical BSC validator-set transition payload.
public func bscValidatorSetPayloadHash(validatorAddresses: [String],
                                       validatorPowers: [UInt64]) throws -> String {
    try bscValidatorSetPayloadHash(
        payload: canonicalBscValidatorSetPayloadBytes(
            validatorAddresses: validatorAddresses,
            validatorPowers: validatorPowers
        )
    )
}

/// SCCP BSC validator-set hash derived from a canonical validator-set payload.
public func bscValidatorSetHashFromPayload(payload: Data) throws -> String {
    try sourceProofValidateBscValidatorSetPayload(payload)
    return sourceProofKeccakHashHex(prefix: "sccp:bsc:validator-set:v1", payload: payload)
}

/// SCCP BSC validator-set hash derived from a canonical validator-set payload.
public func bscValidatorSetHashFromPayload(validatorAddresses: [String],
                                           validatorPowers: [UInt64]) throws -> String {
    try bscValidatorSetHashFromPayload(
        payload: canonicalBscValidatorSetPayloadBytes(
            validatorAddresses: validatorAddresses,
            validatorPowers: validatorPowers
        )
    )
}

/// Canonical BSC Parlia commit-message transcript bytes.
public func canonicalBscCommitMessageBytes(version: UInt8 = 1,
                                           sourceDomain: UInt32 = sccpDomainBsc,
                                           validatorEpoch: UInt64,
                                           blockNumber: UInt64,
                                           blockHash: String,
                                           receiptsRoot: String,
                                           validatorSetHash: String) throws -> Data {
    guard version == 1 else {
        throw SccpSourceProofHashError.invalidValidatorSet("BSC commit message version")
    }
    guard sourceDomain == sccpDomainBsc else {
        throw SccpSourceProofHashError.invalidValidatorSet("sourceDomain")
    }
    var out = Data()
    out.append(version)
    sourceProofAppendU32Le(sourceDomain, to: &out)
    sourceProofAppendU64Le(validatorEpoch, to: &out)
    sourceProofAppendU64Le(blockNumber, to: &out)
    try out.append(sourceProofBytesFromHex32(blockHash, field: "blockHash"))
    try out.append(sourceProofBytesFromHex32(receiptsRoot, field: "receiptsRoot"))
    try out.append(sourceProofBytesFromHex32(validatorSetHash, field: "validatorSetHash"))
    return out
}

/// Hash of a canonical BSC Parlia commit-message transcript.
public func bscCommitMessageHash(version: UInt8 = 1,
                                 sourceDomain: UInt32 = sccpDomainBsc,
                                 validatorEpoch: UInt64,
                                 blockNumber: UInt64,
                                 blockHash: String,
                                 receiptsRoot: String,
                                 validatorSetHash: String) throws -> String {
    sourceProofKeccakHashHex(
        prefix: "sccp:bsc:commit-message:v1",
        payload: try canonicalBscCommitMessageBytes(
            version: version,
            sourceDomain: sourceDomain,
            validatorEpoch: validatorEpoch,
            blockNumber: blockNumber,
            blockHash: blockHash,
            receiptsRoot: receiptsRoot,
            validatorSetHash: validatorSetHash
        )
    )
}

/// Canonical BSC Parlia commit-seal transcript bytes after validating every signer.
public func canonicalBscCommitSealBytes(_ proof: BscCommitSealProof) throws -> Data {
    guard proof.version == 1 else {
        throw SccpSourceProofHashError.invalidValidatorSet("BSC commit seal version")
    }
    let commitMessageHash = try sourceProofNonZeroBytesFromHex32(
        proof.commitMessageHash,
        field: "commitMessageHash"
    )
    guard !proof.validatorPublicKeys.isEmpty,
          proof.validatorPublicKeys.count == proof.validatorPowers.count,
          proof.validatorPublicKeys.count <= sourceProofBscMaxParliaValidators else {
        throw SccpSourceProofHashError.invalidValidatorSet("validatorPublicKeys")
    }
    var validatorAddresses: [Data] = []
    var seenAddresses = Set<String>()
    for (index, publicKey) in proof.validatorPublicKeys.enumerated() {
        let address = try sourceProofBscValidatorAddress20(publicKey: publicKey, field: "validatorPublicKeys[\(index)]")
        guard seenAddresses.insert(address.hexEncodedString()).inserted else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorPublicKeys[\(index)]")
        }
        validatorAddresses.append(address)
    }
    let validatorSetPayload = try sourceProofCanonicalBscValidatorSetPayloadBytes(
        addresses: validatorAddresses,
        powers: proof.validatorPowers
    )
    var validatorSetPreimage = Data("sccp:bsc:validator-set:v1".utf8)
    validatorSetPreimage.append(validatorSetPayload)
    let validatorSetHash = irohaKeccak256(validatorSetPreimage)
    if let suppliedValidatorSetHash = proof.validatorSetHash {
        let supplied = try sourceProofBytesFromHex32(suppliedValidatorSetHash, field: "validatorSetHash")
        guard supplied == validatorSetHash else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorSetHash")
        }
    }
    var computedTotalPower: UInt64 = 0
    for (index, power) in proof.validatorPowers.enumerated() {
        guard power != 0 else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorPowers[\(index)]")
        }
        let sum = computedTotalPower.addingReportingOverflow(power)
        guard !sum.overflow else {
            throw SccpSourceProofHashError.invalidValidatorSet("totalPower")
        }
        computedTotalPower = sum.partialValue
    }
    guard computedTotalPower == proof.totalPower else {
        throw SccpSourceProofHashError.invalidValidatorSet("totalPower")
    }
    let signerIndices = try sourceProofBscSignerIndices(
        signersBitmap: proof.signersBitmap,
        rosterLength: validatorAddresses.count
    )
    guard proof.signatures.count == signerIndices.count else {
        throw SccpSourceProofHashError.invalidValidatorSet("signatures")
    }
    var computedSignedPower: UInt64 = 0
    for (signatureIndex, pair) in zip(proof.signatures, signerIndices).enumerated() {
        let signature = pair.0
        let signerIndex = pair.1
        guard sourceProofTronRecoverableSignatureIsCanonical(signature) else {
            throw SccpSourceProofHashError.invalidValidatorSet("signatures[\(signatureIndex)]")
        }
        guard let recoveredAddress = sourceProofTronRecoveredSignerAddress20(
            messageHash: commitMessageHash,
            signature: signature
        ), recoveredAddress == validatorAddresses[signerIndex] else {
            throw SccpSourceProofHashError.invalidValidatorSet("signatures[\(signatureIndex)]")
        }
        let sum = computedSignedPower.addingReportingOverflow(proof.validatorPowers[signerIndex])
        guard !sum.overflow else {
            throw SccpSourceProofHashError.invalidValidatorSet("signedPower")
        }
        computedSignedPower = sum.partialValue
    }
    guard computedSignedPower == proof.signedPower else {
        throw SccpSourceProofHashError.invalidValidatorSet("signedPower")
    }
    let floorTwoThirds = (proof.totalPower / 3) * 2 + ((proof.totalPower % 3) * 2) / 3
    guard computedSignedPower > floorTwoThirds else {
        throw SccpSourceProofHashError.invalidValidatorSet("signedPower")
    }

    var out = Data()
    out.append(proof.version)
    sourceProofAppendU64Le(proof.totalPower, to: &out)
    sourceProofAppendU64Le(proof.signedPower, to: &out)
    out.append(commitMessageHash)
    out.append(validatorSetHash)
    sourceProofAppendDataVector(proof.signersBitmap, to: &out)
    sourceProofAppendU32Le(UInt32(proof.signatures.count), to: &out)
    for signature in proof.signatures {
        sourceProofAppendDataVector(signature, to: &out)
    }
    return out
}

/// Hash of a validated BSC Parlia commit-seal transcript.
public func bscCommitSealHash(_ proof: BscCommitSealProof) throws -> String {
    sourceProofKeccakHashHex(
        prefix: "sccp:bsc:commit-seal:v1",
        payload: try canonicalBscCommitSealBytes(proof)
    )
}

/// Hash of a BSC ValidatorSet storage value opened by the transition metadata proof.
public func bscValidatorSetStorageValueHash(storageValue: Data) -> String {
    sourceProofKeccakHashHex(prefix: "sccp:bsc:validator-set-storage-value:v1", payload: storageValue)
}

/// Canonical BSC ValidatorSet account/storage proof transcript bytes.
public func canonicalBscValidatorSetMetadataProofBytes(_ proof: BscValidatorSetMetadataProof) throws -> Data {
    guard proof.version == 1 else {
        throw SccpSourceProofHashError.invalidValidatorSet("BSC ValidatorSet metadata proof version")
    }
    guard proof.validatorContractAddress.count == 20 else {
        throw SccpSourceProofHashError.invalidValidatorSet("validatorContractAddress")
    }
    try sourceProofValidateMptProofNodes(proof.accountProofNodes, field: "accountProofNodes")
    try sourceProofValidateMptProofNodes(
        proof.validatorSetLengthProofNodes,
        field: "validatorSetLengthProofNodes"
    )
    guard !proof.validatorStorageProofs.isEmpty,
          proof.validatorStorageProofs.count <= sourceProofBscMaxParliaValidators else {
        throw SccpSourceProofHashError.invalidValidatorSet("validatorStorageProofs")
    }
    var out = Data()
    out.append(proof.version)
    try out.append(sourceProofBytesFromHex32(proof.stateRoot, field: "stateRoot"))
    try out.append(sourceProofBytesFromHex32(proof.nextValidatorSetPayloadHash, field: "nextValidatorSetPayloadHash"))
    sourceProofAppendDataVector(proof.validatorContractAddress, to: &out)
    sourceProofAppendU32Le(UInt32(proof.accountProofNodes.count), to: &out)
    for node in proof.accountProofNodes {
        sourceProofAppendDataVector(node, to: &out)
    }
    try out.append(sourceProofBytesFromHex32(proof.storageRoot, field: "storageRoot"))
    try out.append(sourceProofBytesFromHex32(proof.validatorSetLengthSlot, field: "validatorSetLengthSlot"))
    let validatorSetLengthValueHash = try sourceProofBytesFromHex32(
        proof.validatorSetLengthValueHash,
        field: "validatorSetLengthValueHash"
    )
    let expectedValidatorSetLengthValueHash = try sourceProofBytesFromHex32(
        bscValidatorSetStorageValueHash(storageValue: proof.validatorSetLengthValue),
        field: "validatorSetLengthValueHash"
    )
    guard validatorSetLengthValueHash == expectedValidatorSetLengthValueHash else {
        throw SccpSourceProofHashError.invalidValidatorSet("validatorSetLengthValueHash")
    }
    sourceProofAppendDataVector(proof.validatorSetLengthValue, to: &out)
    out.append(validatorSetLengthValueHash)
    sourceProofAppendU32Le(UInt32(proof.validatorSetLengthProofNodes.count), to: &out)
    for node in proof.validatorSetLengthProofNodes {
        sourceProofAppendDataVector(node, to: &out)
    }
    sourceProofAppendU32Le(UInt32(proof.validatorStorageProofs.count), to: &out)
    for storageProof in proof.validatorStorageProofs {
        try out.append(sourceProofCanonicalBscValidatorStorageProofBytes(storageProof))
    }
    return out
}

/// Hash of the canonical BSC ValidatorSet account/storage proof transcript.
public func bscValidatorSetMetadataProofHash(_ proof: BscValidatorSetMetadataProof) throws -> String {
    let payload = try canonicalBscValidatorSetMetadataProofBytes(proof)
    return sourceProofKeccakHashHex(prefix: "sccp:bsc:validator-set-metadata:v1", payload: payload)
}

/// Canonical BSC ValidatorSet transition message bytes signed by Parlia validators.
public func canonicalBscValidatorSetTransitionMessageBytes(sourceDomain: UInt32 = sccpDomainBsc,
                                                           fromValidatorEpoch: UInt64,
                                                           toValidatorEpoch: UInt64,
                                                           transitionBlockNumber: UInt64,
                                                           transitionBlockHash: String,
                                                           parentValidatorSetHash: String,
                                                           nextValidatorSetHash: String,
                                                           nextValidatorSetPayloadHash: String,
                                                           validatorSetMetadataProofHash: String) throws -> Data {
    guard sourceDomain == sccpDomainBsc else {
        throw SccpSourceProofHashError.invalidValidatorSet("sourceDomain")
    }
    guard fromValidatorEpoch < UInt64.max, fromValidatorEpoch + 1 == toValidatorEpoch else {
        throw SccpSourceProofHashError.invalidValidatorSet("toValidatorEpoch")
    }
    let expectedTransitionBlock = toValidatorEpoch.multipliedReportingOverflow(
        by: sccpBscParliaEpochLengthBlocks
    )
    guard !expectedTransitionBlock.overflow,
          transitionBlockNumber == expectedTransitionBlock.partialValue else {
        throw SccpSourceProofHashError.invalidValidatorSet("transitionBlockNumber")
    }
    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(sourceDomain, to: &out)
    sourceProofAppendU64Le(fromValidatorEpoch, to: &out)
    sourceProofAppendU64Le(toValidatorEpoch, to: &out)
    sourceProofAppendU64Le(transitionBlockNumber, to: &out)
    try out.append(sourceProofBytesFromHex32(transitionBlockHash, field: "transitionBlockHash"))
    try out.append(sourceProofBytesFromHex32(parentValidatorSetHash, field: "parentValidatorSetHash"))
    try out.append(sourceProofBytesFromHex32(nextValidatorSetHash, field: "nextValidatorSetHash"))
    try out.append(sourceProofBytesFromHex32(nextValidatorSetPayloadHash, field: "nextValidatorSetPayloadHash"))
    try out.append(sourceProofBytesFromHex32(validatorSetMetadataProofHash, field: "validatorSetMetadataProofHash"))
    return out
}

/// Hash of the canonical BSC ValidatorSet transition message signed by Parlia validators.
public func bscValidatorSetTransitionMessageHash(sourceDomain: UInt32 = sccpDomainBsc,
                                                 fromValidatorEpoch: UInt64,
                                                 toValidatorEpoch: UInt64,
                                                 transitionBlockNumber: UInt64,
                                                 transitionBlockHash: String,
                                                 parentValidatorSetHash: String,
                                                 nextValidatorSetHash: String,
                                                 nextValidatorSetPayloadHash: String,
                                                 validatorSetMetadataProofHash: String) throws -> String {
    let payload = try canonicalBscValidatorSetTransitionMessageBytes(
        sourceDomain: sourceDomain,
        fromValidatorEpoch: fromValidatorEpoch,
        toValidatorEpoch: toValidatorEpoch,
        transitionBlockNumber: transitionBlockNumber,
        transitionBlockHash: transitionBlockHash,
        parentValidatorSetHash: parentValidatorSetHash,
        nextValidatorSetHash: nextValidatorSetHash,
        nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
        validatorSetMetadataProofHash: validatorSetMetadataProofHash
    )
    return sourceProofKeccakHashHex(prefix: "sccp:bsc:validator-set-transition-message:v1", payload: payload)
}

/// Extract canonical BSC validator-set payload bytes from Parlia header extraData.
public func bscValidatorSetPayloadFromParliaExtra(_ extraData: Data) throws -> Data {
    let candidates = try sourceProofBscParliaPayloadCandidates(extraData: extraData)
    guard candidates.count == 1 else {
        throw SccpSourceProofHashError.invalidValidatorSet("extraData")
    }
    return candidates[0]
}

/// Extract canonical BSC validator-set payload bytes from a Parlia epoch header RLP.
public func bscValidatorSetPayloadFromHeaderRlp(_ headerRlp: Data) throws -> Data {
    let fields = try sourceProofRlpListByteFields(headerRlp)
    guard fields.count >= 13 else {
        throw SccpSourceProofHashError.invalidRlp("headerRlp")
    }
    return try bscValidatorSetPayloadFromParliaExtra(fields[12])
}

/// SSZ `ExecutionPayloadHeader` root derived from a Deneb/Fulu execution header RLP.
public func ethExecutionPayloadHeaderRootFromRlp(_ headerRlp: Data) throws -> String {
    let fields = try sourceProofRlpListByteFields(headerRlp)
    guard fields.count >= 19 else {
        throw SccpSourceProofHashError.invalidRlp("headerRlp")
    }
    let root = try sourceProofSszMerkleizeChunks([
        sourceProofSszByteVectorRoot(fields[0], expectedLength: 32, field: "parentHash"),
        sourceProofSszByteVectorRoot(fields[2], expectedLength: 20, field: "feeRecipient"),
        sourceProofSszByteVectorRoot(fields[3], expectedLength: 32, field: "stateRoot"),
        sourceProofSszByteVectorRoot(fields[5], expectedLength: 32, field: "receiptsRoot"),
        sourceProofSszByteVectorRoot(fields[6], expectedLength: 256, field: "logsBloom"),
        sourceProofSszByteVectorRoot(fields[13], expectedLength: 32, field: "prevRandao"),
        sourceProofSszU64ChunkFromRlp(fields[8], field: "blockNumber"),
        sourceProofSszU64ChunkFromRlp(fields[9], field: "gasLimit"),
        sourceProofSszU64ChunkFromRlp(fields[10], field: "gasUsed"),
        sourceProofSszU64ChunkFromRlp(fields[11], field: "timestamp"),
        try sourceProofSszByteListRoot(fields[12], maxLength: 32, field: "extraData"),
        try sourceProofSszU256ChunkFromRlp(fields[15], field: "baseFeePerGas"),
        Data(irohaKeccak256(headerRlp)),
        sourceProofSszByteVectorRoot(fields[4], expectedLength: 32, field: "transactionsRoot"),
        sourceProofSszByteVectorRoot(fields[16], expectedLength: 32, field: "withdrawalsRoot"),
        sourceProofSszU64ChunkFromRlp(fields[17], field: "blobGasUsed"),
        sourceProofSszU64ChunkFromRlp(fields[18], field: "excessBlobGas"),
    ])
    return "0x" + root.hexEncodedString()
}

/// SSZ `BeaconBlockBody` root derived from the execution-payload branch.
public func ethBeaconBodyRootFromExecutionPayloadBranch(executionPayloadHeaderRoot: String,
                                                        executionPayloadBranch: [Data]) throws -> String {
    guard executionPayloadBranch.count == sccpEthExecutionPayloadBodyBranchDepth else {
        throw SccpSourceProofHashError.invalidBranch("executionPayloadBranch")
    }
    let root = try sourceProofSszMerkleRootFromBranch(
        leaf: sourceProofBytesFromHex32(executionPayloadHeaderRoot, field: "executionPayloadHeaderRoot"),
        leafIndex: sccpEthExecutionPayloadBodyFieldIndex,
        branch: executionPayloadBranch,
        field: "executionPayloadBranch"
    )
    return "0x" + root.hexEncodedString()
}

/// SSZ `BeaconBlockHeader` root from UI/mobile prover witness material.
public func ethBeaconBlockHeaderRoot(beaconSlot: UInt64,
                                     beaconProposerIndex: UInt64,
                                     beaconParentRoot: String,
                                     beaconStateRoot: String,
                                     beaconBodyRoot: String) throws -> String {
    let root = try sourceProofSszMerkleizeChunks([
        sourceProofSszU64Chunk(beaconSlot),
        sourceProofSszU64Chunk(beaconProposerIndex),
        sourceProofBytesFromHex32(beaconParentRoot, field: "beaconParentRoot"),
        sourceProofBytesFromHex32(beaconStateRoot, field: "beaconStateRoot"),
        sourceProofBytesFromHex32(beaconBodyRoot, field: "beaconBodyRoot"),
    ])
    return "0x" + root.hexEncodedString()
}

/// Typed EVM-family MPT value envelope carrying an SCCP receipt root.
public func canonicalEvmReceiptRootMptValue(receiptRoot: String) throws -> Data {
    let root = try sourceProofNonZeroBytesFromHex32(receiptRoot, field: "receiptRoot")
    let value = sourceProofRlpList([
        sourceProofRlpString(sccpEvmReceiptRootValueMarker),
        sourceProofRlpString(root),
    ])
    guard !value.isEmpty, value.count <= sccpEvmMaxReceiptValueBytes else {
        throw SccpSourceProofHashError.invalidValidatorSet("receiptRootMptValue")
    }
    return value
}

/// Typed TRON MPT value envelope carrying an SCCP receipt root.
public func canonicalTronReceiptRootMptValue(receiptRoot: String) throws -> Data {
    let root = try sourceProofNonZeroBytesFromHex32(receiptRoot, field: "receiptRoot")
    let value = sourceProofRlpList([
        sourceProofRlpString(sccpTronReceiptRootValueMarker),
        sourceProofRlpString(root),
    ])
    guard !value.isEmpty, value.count <= sccpTronMaxReceiptValueBytes else {
        throw SccpSourceProofHashError.invalidValidatorSet("receiptRootMptValue")
    }
    return value
}

public func canonicalTronSccpReceiptProofBytes(sourceEventDigest: String,
                                               receiptRoot: String,
                                               transactionRoot: String,
                                               inclusionBranch: [Data]) throws -> Data {
    var out = Data()
    out.append(1)
    try out.append(sourceProofNonZeroBytesFromHex32(sourceEventDigest, field: "sourceEventDigest"))
    try out.append(sourceProofNonZeroBytesFromHex32(receiptRoot, field: "receiptRoot"))
    try out.append(sourceProofNonZeroBytesFromHex32(transactionRoot, field: "transactionRoot"))
    try sourceProofAppendBranch(inclusionBranch, to: &out, requireNonEmpty: true)
    return out
}

/// Hash of the canonical TRON receipt-proof transcript checked by the source adapter.
public func tronSccpReceiptProofHash(sourceEventDigest: String,
                                     receiptRoot: String,
                                     transactionRoot: String,
                                     inclusionBranch: [Data]) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:tron:receipt-proof:v1",
        payload: canonicalTronSccpReceiptProofBytes(
            sourceEventDigest: sourceEventDigest,
            receiptRoot: receiptRoot,
            transactionRoot: transactionRoot,
            inclusionBranch: inclusionBranch
        )
    )
}

/// Canonical TRON receipt-state/MPT transcript bytes checked by the SCCP source adapter.
public func canonicalTronSccpReceiptStateProofBytes(sourceEventDigest: String,
                                                    receiptRoot: String,
                                                    transactionRoot: String,
                                                    receiptRootIndex: UInt64,
                                                    receiptTrieProofNodes: [Data],
                                                    inclusionBranch: [Data]) throws -> Data {
    try sourceProofValidateTronMptProofNodes(receiptTrieProofNodes)
    var out = Data()
    out.append(1)
    try out.append(sourceProofNonZeroBytesFromHex32(sourceEventDigest, field: "sourceEventDigest"))
    try out.append(sourceProofNonZeroBytesFromHex32(receiptRoot, field: "receiptRoot"))
    try out.append(sourceProofNonZeroBytesFromHex32(transactionRoot, field: "transactionRoot"))
    sourceProofAppendU64Le(receiptRootIndex, to: &out)
    sourceProofAppendU32Le(UInt32(receiptTrieProofNodes.count), to: &out)
    for node in receiptTrieProofNodes {
        sourceProofAppendDataVector(node, to: &out)
    }
    try sourceProofAppendBranch(inclusionBranch, to: &out, requireNonEmpty: true)
    return out
}

/// Hash of the canonical TRON receipt-state/MPT transcript checked by the source adapter.
public func tronSccpReceiptStateProofHash(sourceEventDigest: String,
                                          receiptRoot: String,
                                          transactionRoot: String,
                                          receiptRootIndex: UInt64,
                                          receiptTrieProofNodes: [Data],
                                          inclusionBranch: [Data]) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:tron:receipt-state-proof:v1",
        payload: canonicalTronSccpReceiptStateProofBytes(
            sourceEventDigest: sourceEventDigest,
            receiptRoot: receiptRoot,
            transactionRoot: transactionRoot,
            receiptRootIndex: receiptRootIndex,
            receiptTrieProofNodes: receiptTrieProofNodes,
            inclusionBranch: inclusionBranch
        )
    )
}

/// TRON TVM calldata for `submitSccpSourceEvent(uint32,uint32,bytes32)`.
public func tronSccpSourceMessageCallData(sourceDomain: UInt32,
                                          targetDomain: UInt32,
                                          sourceEventDigest: String) throws -> Data {
    guard sourceDomain == sccpDomainTron else {
        throw SccpSourceProofHashError.invalidValidatorSet("sourceDomain")
    }
    guard targetDomain == sccpDomainSora else {
        throw SccpSourceProofHashError.invalidValidatorSet("targetDomain")
    }
    var out = Data(irohaKeccak256(sccpTronSourceMessageCallAbi).prefix(4))
    sourceProofAppendAbiU32(sourceDomain, to: &out)
    sourceProofAppendAbiU32(targetDomain, to: &out)
    try out.append(sourceProofNonZeroBytesFromHex32(sourceEventDigest, field: "sourceEventDigest"))
    return out
}

private func sourceProofReadProtobufBytesField(
    _ bytes: [UInt8],
    cursor: inout Int,
    field: String
) throws -> Data {
    let rawLength = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: field)
    guard let length = Int(exactly: rawLength) else {
        throw SccpSourceProofHashError.invalidValidatorSet(field)
    }
    let end = cursor + length
    guard length >= 0, end <= bytes.count else {
        throw SccpSourceProofHashError.invalidValidatorSet(field)
    }
    let value = Data(bytes[cursor..<end])
    cursor = end
    return value
}

private func sourceProofTronTransactionResultSuccess(_ result: Data) throws -> Bool {
    let bytes = [UInt8](result)
    var cursor = 0
    var feeSeen = false
    var retSeen = false
    var contractRetSeen = false
    while cursor < bytes.count {
        let key = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "transactionResult")
        let fieldNumber = key >> 3
        let wireType = key & 0x07
        switch (fieldNumber, wireType) {
        case (1, 0):
            guard !feeSeen else { return false }
            feeSeen = true
            _ = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "transactionResult")
        case (2, 0):
            guard !retSeen else { return false }
            retSeen = true
            guard try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "transactionResult") == 0 else {
                return false
            }
        case (3, 0):
            guard !contractRetSeen else { return false }
            contractRetSeen = true
            guard try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "transactionResult") == 1 else {
                return false
            }
        default:
            return false
        }
    }
    return contractRetSeen
}

private func sourceProofReadTronAnyValue(_ parameter: Data) throws -> Data? {
    let bytes = [UInt8](parameter)
    var cursor = 0
    var typeUrl: Data?
    var value: Data?
    while cursor < bytes.count {
        let key = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "triggerParameter")
        let fieldNumber = key >> 3
        let wireType = key & 0x07
        switch (fieldNumber, wireType) {
        case (1, 2):
            guard typeUrl == nil else { return nil }
            typeUrl = try sourceProofReadProtobufBytesField(bytes, cursor: &cursor, field: "triggerParameter")
        case (2, 2):
            guard value == nil else { return nil }
            value = try sourceProofReadProtobufBytesField(bytes, cursor: &cursor, field: "triggerParameter")
        default:
            return nil
        }
    }
    return typeUrl == sccpTronTriggerSmartContractTypeUrl ? value : nil
}

private func sourceProofTronTriggerSourceCallOwnerAddress(_ trigger: Data,
                                                          sourceEventDigest: Data,
                                                          expectedContractAddress: Data? = nil,
                                                          expectedOwnerAddress: Data? = nil) throws -> Data? {
    let bytes = [UInt8](trigger)
    var cursor = 0
    var ownerAddress: Data?
    var contractAddress: Data?
    var data: Data?
    var callValueSeen = false
    var callTokenValueSeen = false
    var tokenIdSeen = false
    while cursor < bytes.count {
        let key = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "triggerContract")
        let fieldNumber = key >> 3
        let wireType = key & 0x07
        switch (fieldNumber, wireType) {
        case (1, 2):
            guard ownerAddress == nil else { return nil }
            ownerAddress = try sourceProofReadProtobufBytesField(bytes, cursor: &cursor, field: "triggerContract")
        case (2, 2):
            guard contractAddress == nil else { return nil }
            contractAddress = try sourceProofReadProtobufBytesField(bytes, cursor: &cursor, field: "triggerContract")
        case (3, 0):
            guard !callValueSeen else { return nil }
            callValueSeen = true
            guard try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "triggerContract") == 0 else {
                return nil
            }
        case (4, 2):
            guard data == nil else { return nil }
            data = try sourceProofReadProtobufBytesField(bytes, cursor: &cursor, field: "triggerContract")
        case (5, 0):
            guard !callTokenValueSeen else { return nil }
            callTokenValueSeen = true
            guard try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "triggerContract") == 0 else {
                return nil
            }
        case (6, 0):
            guard !tokenIdSeen else { return nil }
            tokenIdSeen = true
            guard try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "triggerContract") == 0 else {
                return nil
            }
        default:
            return nil
        }
    }
    var expected = Data(irohaKeccak256(sccpTronSourceMessageCallAbi).prefix(4))
    sourceProofAppendAbiU32(sccpDomainTron, to: &expected)
    sourceProofAppendAbiU32(sccpDomainSora, to: &expected)
    expected.append(sourceEventDigest)
    guard let ownerAddress,
          let contractAddress,
          sourceProofIsNonZeroTronAddress(ownerAddress),
          sourceProofIsNonZeroTronAddress(contractAddress) else {
        return nil
    }
    let ownerAddress20 = Data(ownerAddress.dropFirst())
    guard (expectedContractAddress.map { Data(contractAddress.dropFirst()) == $0 } ?? true),
          (expectedOwnerAddress.map { ownerAddress20 == $0 } ?? true),
          data == expected else {
        return nil
    }
    return ownerAddress20
}

private func sourceProofTronContractSourceCallOwnerAddress(_ contract: Data,
                                                           sourceEventDigest: Data,
                                                           expectedContractAddress: Data? = nil,
                                                           expectedOwnerAddress: Data? = nil) throws -> Data? {
    let bytes = [UInt8](contract)
    var cursor = 0
    var contractType: UInt64?
    var parameter: Data?
    while cursor < bytes.count {
        let key = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "transactionContract")
        let fieldNumber = key >> 3
        let wireType = key & 0x07
        switch (fieldNumber, wireType) {
        case (1, 0):
            guard contractType == nil else { return nil }
            contractType = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "transactionContract")
        case (2, 2):
            guard parameter == nil else { return nil }
            parameter = try sourceProofReadProtobufBytesField(bytes, cursor: &cursor, field: "transactionContract")
        default:
            return nil
        }
    }
    guard contractType == 31, let parameter, let trigger = try sourceProofReadTronAnyValue(parameter) else {
        return nil
    }
    return try sourceProofTronTriggerSourceCallOwnerAddress(
        trigger,
        sourceEventDigest: sourceEventDigest,
        expectedContractAddress: expectedContractAddress,
        expectedOwnerAddress: expectedOwnerAddress
    )
}

private func sourceProofTronRawDataSourceCallOwnerAddress(_ rawData: Data,
                                                          sourceEventDigest: Data,
                                                          expectedContractAddress: Data? = nil,
                                                          expectedOwnerAddress: Data? = nil) throws -> Data? {
    let bytes = [UInt8](rawData)
    var cursor = 0
    var refBlockBytesSeen = false
    var refBlockNumSeen = false
    var refBlockHashSeen = false
    var expirationMs: UInt64?
    var timestampMs: UInt64?
    var feeLimitSeen = false
    var contractCount = 0
    var ownerAddress: Data?
    while cursor < bytes.count {
        let key = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "rawData")
        let fieldNumber = key >> 3
        let wireType = key & 0x07
        switch (fieldNumber, wireType) {
        case (1, 2):
            guard !refBlockBytesSeen else { return nil }
            refBlockBytesSeen = true
            let value = try sourceProofReadProtobufBytesField(bytes, cursor: &cursor, field: "rawData")
            guard value.count == 2, value.contains(where: { $0 != 0 }) else { return nil }
        case (3, 0):
            guard !refBlockNumSeen else { return nil }
            refBlockNumSeen = true
            _ = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "rawData")
        case (4, 2):
            guard !refBlockHashSeen else { return nil }
            refBlockHashSeen = true
            let value = try sourceProofReadProtobufBytesField(bytes, cursor: &cursor, field: "rawData")
            guard value.count == 8, value.contains(where: { $0 != 0 }) else { return nil }
        case (8, 0):
            guard expirationMs == nil else { return nil }
            expirationMs = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "rawData")
            guard expirationMs != 0 else { return nil }
        case (11, 2):
            contractCount += 1
            guard contractCount <= 1 else { return nil }
            ownerAddress = try sourceProofTronContractSourceCallOwnerAddress(
                sourceProofReadProtobufBytesField(bytes, cursor: &cursor, field: "rawData"),
                sourceEventDigest: sourceEventDigest,
                expectedContractAddress: expectedContractAddress,
                expectedOwnerAddress: expectedOwnerAddress
            )
        case (14, 0):
            guard timestampMs == nil else { return nil }
            timestampMs = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "rawData")
            guard timestampMs != 0 else { return nil }
        case (18, 0):
            guard !feeLimitSeen else { return nil }
            feeLimitSeen = true
            guard try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "rawData") != 0 else {
                return nil
            }
        default:
            return nil
        }
    }
    guard refBlockBytesSeen &&
        refBlockHashSeen &&
        expirationMs != nil &&
        timestampMs != nil &&
        expirationMs! > timestampMs! &&
        feeLimitSeen &&
        contractCount == 1,
        let ownerAddress else {
        return nil
    }
    return ownerAddress
}

private func sourceProofValidateTronTransactionSourceCall(_ transactionBytes: Data,
                                                         sourceEventDigest: Data,
                                                         expectedContractAddress: Data? = nil,
                                                         expectedOwnerAddress: Data? = nil) throws {
    let bytes = [UInt8](transactionBytes)
    var cursor = 0
    var rawData: Data?
    var signatures: [Data] = []
    var resultCount = 0
    var resultSuccess = false
    while cursor < bytes.count {
        let key = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: "transactionBytes")
        let fieldNumber = key >> 3
        let wireType = key & 0x07
        switch (fieldNumber, wireType) {
        case (1, 2):
            guard rawData == nil else { throw SccpSourceProofHashError.invalidValidatorSet("transactionBytes") }
            rawData = try sourceProofReadProtobufBytesField(bytes, cursor: &cursor, field: "transactionBytes")
        case (2, 2):
            guard signatures.count < sccpTronSourceCallSignatures else {
                throw SccpSourceProofHashError.invalidValidatorSet("transactionBytes")
            }
            let signature = try sourceProofReadProtobufBytesField(bytes, cursor: &cursor, field: "transactionBytes")
            guard sourceProofTronRecoverableSignatureIsCanonical(signature) else {
                throw SccpSourceProofHashError.invalidValidatorSet("transactionBytes")
            }
            signatures.append(signature)
        case (5, 2):
            guard resultCount < 1 else { throw SccpSourceProofHashError.invalidValidatorSet("transactionBytes") }
            resultSuccess = try sourceProofTronTransactionResultSuccess(
                sourceProofReadProtobufBytesField(bytes, cursor: &cursor, field: "transactionBytes")
            )
            resultCount += 1
        default:
            throw SccpSourceProofHashError.invalidValidatorSet("transactionBytes")
        }
    }
    guard let rawData,
          signatures.count == sccpTronSourceCallSignatures,
          resultCount == 1,
          resultSuccess,
          let ownerAddress = try sourceProofTronRawDataSourceCallOwnerAddress(
              rawData,
              sourceEventDigest: sourceEventDigest,
              expectedContractAddress: expectedContractAddress,
              expectedOwnerAddress: expectedOwnerAddress
          ),
          sourceProofTronRecoveredSignerAddress20(
              messageHash: Data(SHA256.hash(data: rawData)),
              signature: signatures[0]
          ) == ownerAddress else {
        throw SccpSourceProofHashError.invalidValidatorSet("transactionBytes")
    }
}

/// Canonical TRON transaction-Merkle source-call transcript bytes checked by the SCCP source adapter.
private func sourceProofTronTransactionMerkleRoot(transactionBytes: Data,
                                                  transactionIndex: UInt64,
                                                  transactionCount: UInt64,
                                                  transactionMerkleBranch: [Data]) throws -> Data {
    var current = Data(SHA256.hash(data: transactionBytes))
    var index = transactionIndex
    var count = transactionCount
    var branchCursor = 0
    while count > 1 {
        if index & 1 == 0 {
            if index + 1 < count {
                guard branchCursor < transactionMerkleBranch.count else {
                    throw SccpSourceProofHashError.invalidValidatorSet("transactionMerkleBranch")
                }
                current = sourceProofSszHashNode(current, transactionMerkleBranch[branchCursor])
                branchCursor += 1
            }
        } else {
            guard branchCursor < transactionMerkleBranch.count else {
                throw SccpSourceProofHashError.invalidValidatorSet("transactionMerkleBranch")
            }
            current = sourceProofSszHashNode(transactionMerkleBranch[branchCursor], current)
            branchCursor += 1
        }
        index >>= 1
        count = (count + 1) / 2
    }
    guard branchCursor == transactionMerkleBranch.count else {
        throw SccpSourceProofHashError.invalidValidatorSet("transactionMerkleBranch")
    }
    return current
}

public func canonicalTronSccpTransactionSourceProofBytes(sourceEventDigest: String,
                                                         receiptRoot: String,
                                                         transactionRoot: String,
                                                         transactionIndex: UInt64,
                                                         transactionCount: UInt64,
                                                         transactionBytes: Data,
                                                         transactionMerkleBranch: [Data],
                                                         inclusionBranch: [Data],
                                                         sourceBridgeEmitterAddress: String? = nil,
                                                         sourceBridgeOwnerAddress: String? = nil) throws -> Data {
    guard transactionCount != 0, transactionIndex < transactionCount else {
        throw SccpSourceProofHashError.invalidValidatorSet("transactionIndex")
    }
    guard !transactionBytes.isEmpty, transactionBytes.count <= sccpTronMaxTransactionBytes else {
        throw SccpSourceProofHashError.invalidValidatorSet("transactionBytes")
    }
    let sourceEventDigestBytes = try sourceProofNonZeroBytesFromHex32(sourceEventDigest, field: "sourceEventDigest")
    let expectedContractAddress = try sourceBridgeEmitterAddress.map {
        try sourceProofNonZeroBytesFromHex20($0, field: "sourceBridgeEmitterAddress")
    }
    let expectedOwnerAddress = try sourceBridgeOwnerAddress.map {
        try sourceProofNonZeroBytesFromHex20($0, field: "sourceBridgeOwnerAddress")
    }
    try sourceProofValidateTronTransactionSourceCall(
        transactionBytes,
        sourceEventDigest: sourceEventDigestBytes,
        expectedContractAddress: expectedContractAddress,
        expectedOwnerAddress: expectedOwnerAddress
    )
    try sourceProofValidateTronTransactionMerkleBranch(transactionMerkleBranch)
    let transactionRootBytes = try sourceProofNonZeroBytesFromHex32(transactionRoot, field: "transactionRoot")
    guard try sourceProofTronTransactionMerkleRoot(
        transactionBytes: transactionBytes,
        transactionIndex: transactionIndex,
        transactionCount: transactionCount,
        transactionMerkleBranch: transactionMerkleBranch
    ) == transactionRootBytes else {
        throw SccpSourceProofHashError.invalidValidatorSet("transactionRoot")
    }

    var out = Data()
    out.append(1)
    out.append(sourceEventDigestBytes)
    try out.append(sourceProofNonZeroBytesFromHex32(receiptRoot, field: "receiptRoot"))
    out.append(transactionRootBytes)
    sourceProofAppendU64Le(transactionIndex, to: &out)
    sourceProofAppendU64Le(transactionCount, to: &out)
    sourceProofAppendDataVector(transactionBytes, to: &out)
    sourceProofAppendU32Le(UInt32(transactionMerkleBranch.count), to: &out)
    for sibling in transactionMerkleBranch {
        out.append(sibling)
    }
    try sourceProofAppendBranch(inclusionBranch, to: &out, requireNonEmpty: true)
    return out
}

/// Hash of the canonical TRON transaction-Merkle source-call transcript.
public func tronSccpTransactionSourceProofHash(sourceEventDigest: String,
                                               receiptRoot: String,
                                               transactionRoot: String,
                                               transactionIndex: UInt64,
                                               transactionCount: UInt64,
                                               transactionBytes: Data,
                                               transactionMerkleBranch: [Data],
                                               inclusionBranch: [Data],
                                               sourceBridgeEmitterAddress: String? = nil,
                                               sourceBridgeOwnerAddress: String? = nil) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:tron:transaction-source-proof:v1",
        payload: canonicalTronSccpTransactionSourceProofBytes(
            sourceEventDigest: sourceEventDigest,
            receiptRoot: receiptRoot,
            transactionRoot: transactionRoot,
            transactionIndex: transactionIndex,
            transactionCount: transactionCount,
            transactionBytes: transactionBytes,
            transactionMerkleBranch: transactionMerkleBranch,
            inclusionBranch: inclusionBranch,
            sourceBridgeEmitterAddress: sourceBridgeEmitterAddress,
            sourceBridgeOwnerAddress: sourceBridgeOwnerAddress
        )
    )
}

/// Canonical TRON `BlockHeader.raw_data` bytes checked by the SCCP source adapter.
public func canonicalTronRawBlockHeaderBytes(number: UInt64,
                                             txTrieRoot: String,
                                             accountStateRoot: String,
                                             parentBlockId: String,
                                             witnessAddress: String,
                                             headerVersion: UInt32,
                                             timestampMs: UInt64) throws -> Data {
    guard number != 0, headerVersion != 0, timestampMs != 0 else {
        throw SccpSourceProofHashError.invalidValidatorSet("rawBlockHeader")
    }
    let txTrieRootBytes = try sourceProofBytesFromHex32(txTrieRoot, field: "txTrieRoot")
    let accountStateRootBytes = try sourceProofBytesFromHex32(accountStateRoot, field: "accountStateRoot")
    let parentBlockIdBytes = try sourceProofBytesFromHex32(parentBlockId, field: "parentBlockId")
    let witnessAddressBytes = try sourceProofBytesFromHex21(witnessAddress, field: "witnessAddress")
    guard !txTrieRootBytes.allSatisfy({ $0 == 0 }),
          !accountStateRootBytes.allSatisfy({ $0 == 0 }),
          !parentBlockIdBytes.allSatisfy({ $0 == 0 }),
          sourceProofIsNonZeroTronAddress(witnessAddressBytes) else {
        throw SccpSourceProofHashError.invalidValidatorSet("rawBlockHeader")
    }
    var out = Data()
    sourceProofAppendProtobufU64(fieldNumber: 1, value: timestampMs, to: &out)
    sourceProofAppendProtobufBytes(fieldNumber: 2, value: txTrieRootBytes, to: &out)
    sourceProofAppendProtobufBytes(fieldNumber: 3, value: parentBlockIdBytes, to: &out)
    sourceProofAppendProtobufU64(fieldNumber: 7, value: number, to: &out)
    sourceProofAppendProtobufBytes(fieldNumber: 9, value: witnessAddressBytes, to: &out)
    sourceProofAppendProtobufU64(fieldNumber: 10, value: UInt64(headerVersion), to: &out)
    sourceProofAppendProtobufBytes(fieldNumber: 11, value: accountStateRootBytes, to: &out)
    return out
}

/// SHA-256 hash of TRON `BlockHeader.raw_data` bytes.
public func tronRawBlockHeaderHash(rawData: Data) -> String {
    "0x" + Data(SHA256.hash(data: rawData)).hexEncodedString()
}

/// TRON block id derived from block number and raw-data hash.
public func tronBlockIdFromRawDataHash(number: UInt64, rawDataHash: String) throws -> String {
    guard number != 0 else {
        throw SccpSourceProofHashError.invalidValidatorSet("number")
    }
    var blockId = try sourceProofBytesFromHex32(rawDataHash, field: "rawDataHash")
    var numberBytes = Data()
    sourceProofAppendU64Be(number, to: &numberBytes)
    blockId.replaceSubrange(0..<8, with: numberBytes)
    return "0x" + blockId.hexEncodedString()
}

private struct SourceProofTronRawBlockHeaderFields {
    let number: UInt64
    let txTrieRoot: Data
    let accountStateRoot: Data
    let parentBlockId: Data
    let witnessAddress: Data
    let headerVersion: UInt32
    let timestampMs: UInt64
}

private func sourceProofProtobufVarintLength(_ value: UInt64) -> Int {
    var working = value
    var length = 1
    while working >= 0x80 {
        length += 1
        working >>= 7
    }
    return length
}

private func sourceProofReadCanonicalProtobufVarint(
    _ bytes: [UInt8],
    cursor: inout Int,
    field: String
) throws -> UInt64 {
    let start = cursor
    var value: UInt64 = 0
    var shift: UInt64 = 0
    for index in 0..<10 {
        guard cursor < bytes.count else {
            throw SccpSourceProofHashError.invalidValidatorSet(field)
        }
        let byte = bytes[cursor]
        cursor += 1
        let chunk = UInt64(byte & 0x7f)
        guard !(index == 9 && chunk > 1) else {
            throw SccpSourceProofHashError.invalidValidatorSet(field)
        }
        value |= chunk << shift
        if byte & 0x80 == 0 {
            guard cursor - start == sourceProofProtobufVarintLength(value) else {
                throw SccpSourceProofHashError.invalidValidatorSet(field)
            }
            return value
        }
        shift += 7
    }
    throw SccpSourceProofHashError.invalidValidatorSet(field)
}

private func sourceProofDecodeTronRawBlockHeaderFields(
    _ rawData: Data,
    field: String
) throws -> SourceProofTronRawBlockHeaderFields {
    let bytes = [UInt8](rawData)
    var cursor = 0
    var number: UInt64?
    var txTrieRoot: Data?
    var accountStateRoot: Data?
    var parentBlockId: Data?
    var witnessIdSeen = false
    var witnessAddress: Data?
    var headerVersion: UInt32?
    var timestampMs: UInt64?

    func readBytes(_ byteLength: Int) throws -> Data {
        let length = Int(try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: field))
        let end = cursor + length
        guard length == byteLength, end <= bytes.count else {
            throw SccpSourceProofHashError.invalidValidatorSet(field)
        }
        let value = Data(bytes[cursor..<end])
        cursor = end
        return value
    }

    while cursor < bytes.count {
        let key = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: field)
        let fieldNumber = key >> 3
        let wireType = key & 0x07
        switch (fieldNumber, wireType) {
        case (1, 0):
            guard timestampMs == nil else { throw SccpSourceProofHashError.invalidValidatorSet(field) }
            timestampMs = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: field)
        case (2, 2):
            guard txTrieRoot == nil else { throw SccpSourceProofHashError.invalidValidatorSet(field) }
            txTrieRoot = try readBytes(32)
        case (3, 2):
            guard parentBlockId == nil else { throw SccpSourceProofHashError.invalidValidatorSet(field) }
            parentBlockId = try readBytes(32)
        case (7, 0):
            guard number == nil else { throw SccpSourceProofHashError.invalidValidatorSet(field) }
            number = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: field)
        case (8, 0):
            guard !witnessIdSeen else { throw SccpSourceProofHashError.invalidValidatorSet(field) }
            witnessIdSeen = true
            _ = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: field)
        case (9, 2):
            guard witnessAddress == nil else { throw SccpSourceProofHashError.invalidValidatorSet(field) }
            witnessAddress = try readBytes(21)
        case (10, 0):
            guard headerVersion == nil else { throw SccpSourceProofHashError.invalidValidatorSet(field) }
            let value = try sourceProofReadCanonicalProtobufVarint(bytes, cursor: &cursor, field: field)
            guard value <= UInt64(UInt32.max) else {
                throw SccpSourceProofHashError.invalidValidatorSet(field)
            }
            headerVersion = UInt32(value)
        case (11, 2):
            guard accountStateRoot == nil else { throw SccpSourceProofHashError.invalidValidatorSet(field) }
            accountStateRoot = try readBytes(32)
        default:
            throw SccpSourceProofHashError.invalidValidatorSet(field)
        }
    }

    guard let number,
          let txTrieRoot,
          let accountStateRoot,
          let parentBlockId,
          let witnessAddress,
          let headerVersion,
          let timestampMs,
          number != 0,
          timestampMs != 0,
          headerVersion != 0,
          !txTrieRoot.allSatisfy({ $0 == 0 }),
          !accountStateRoot.allSatisfy({ $0 == 0 }),
          !parentBlockId.allSatisfy({ $0 == 0 }),
          sourceProofIsNonZeroTronAddress(witnessAddress) else {
        throw SccpSourceProofHashError.invalidValidatorSet(field)
    }
    return SourceProofTronRawBlockHeaderFields(
        number: number,
        txTrieRoot: txTrieRoot,
        accountStateRoot: accountStateRoot,
        parentBlockId: parentBlockId,
        witnessAddress: witnessAddress,
        headerVersion: headerVersion,
        timestampMs: timestampMs
    )
}

private func sourceProofTronBlockId(number: UInt64, rawDataHash: Data) -> Data {
    var blockId = rawDataHash
    var numberBytes = Data()
    sourceProofAppendU64Be(number, to: &numberBytes)
    blockId.replaceSubrange(0..<8, with: numberBytes)
    return blockId
}

private func sourceProofTronRecoverableSignatureIsCanonical(_ signature: Data) -> Bool {
    guard signature.count == 65 else {
        return false
    }
    let recoveryId = signature[signature.index(signature.startIndex, offsetBy: 64)]
    guard (0...3).contains(Int(recoveryId)) || (27...30).contains(Int(recoveryId)) else {
        return false
    }
    let rValue = signature.subdata(in: 0..<32)
    let sValue = signature.subdata(in: 32..<64)
    return !rValue.allSatisfy({ $0 == 0 }) &&
        sourceProofCompareBytes(rValue, sccpSecp256k1ScalarOrderBe) < 0 &&
        !sValue.allSatisfy({ $0 == 0 }) &&
        sourceProofCompareBytes(sValue, sccpSecp256k1ScalarHalfOrderBe) <= 0
}

private struct SourceProofBigUInt: Equatable {
    private var limbs: [UInt32]

    var isZero: Bool {
        limbs.count == 1 && limbs[0] == 0
    }

    var bitWidth: Int {
        guard let last = limbs.last, last != 0 else {
            return 0
        }
        return (limbs.count - 1) * 32 + (32 - last.leadingZeroBitCount)
    }

    init(_ value: UInt32) {
        self.limbs = [value]
        normalize()
    }

    init(bigEndian data: Data) {
        var values: [UInt32] = []
        var end = data.count
        while end > 0 {
            let start = max(0, end - 4)
            var limb: UInt32 = 0
            for byte in data[start..<end] {
                limb = (limb << 8) | UInt32(byte)
            }
            values.append(limb)
            end = start
        }
        self.limbs = values.isEmpty ? [0] : values
        normalize()
    }

    private init(limbs: [UInt32]) {
        self.limbs = limbs.isEmpty ? [0] : limbs
        normalize()
    }

    mutating private func normalize() {
        while limbs.count > 1 && limbs.last == 0 {
            limbs.removeLast()
        }
    }

    func compare(_ other: SourceProofBigUInt) -> Int {
        if limbs.count != other.limbs.count {
            return limbs.count < other.limbs.count ? -1 : 1
        }
        for index in stride(from: limbs.count - 1, through: 0, by: -1) {
            if limbs[index] != other.limbs[index] {
                return limbs[index] < other.limbs[index] ? -1 : 1
            }
        }
        return 0
    }

    func bit(at index: Int) -> Bool {
        guard index >= 0 else {
            return false
        }
        let limbIndex = index / 32
        guard limbIndex < limbs.count else {
            return false
        }
        return ((limbs[limbIndex] >> UInt32(index % 32)) & 1) == 1
    }

    func adding(_ other: SourceProofBigUInt) -> SourceProofBigUInt {
        let count = max(limbs.count, other.limbs.count)
        var out = [UInt32](repeating: 0, count: count)
        var carry: UInt64 = 0
        for index in 0..<count {
            let left = index < limbs.count ? UInt64(limbs[index]) : 0
            let right = index < other.limbs.count ? UInt64(other.limbs[index]) : 0
            let sum = left + right + carry
            out[index] = UInt32(sum & 0xffff_ffff)
            carry = sum >> 32
        }
        if carry != 0 {
            out.append(UInt32(carry))
        }
        return SourceProofBigUInt(limbs: out)
    }

    func subtracting(_ other: SourceProofBigUInt) -> SourceProofBigUInt {
        precondition(compare(other) >= 0)
        var out = [UInt32](repeating: 0, count: limbs.count)
        var borrow: Int64 = 0
        for index in 0..<limbs.count {
            let left = Int64(limbs[index])
            let right = index < other.limbs.count ? Int64(other.limbs[index]) : 0
            var diff = left - right - borrow
            if diff < 0 {
                diff += 0x1_0000_0000
                borrow = 1
            } else {
                borrow = 0
            }
            out[index] = UInt32(diff)
        }
        return SourceProofBigUInt(limbs: out)
    }

    func shiftedRight(_ bitCount: Int) -> SourceProofBigUInt {
        guard bitCount > 0 else {
            return self
        }
        let limbShift = bitCount / 32
        let bitShift = bitCount % 32
        guard limbShift < limbs.count else {
            return SourceProofBigUInt(0)
        }
        var out: [UInt32] = []
        out.reserveCapacity(limbs.count - limbShift)
        for index in limbShift..<limbs.count {
            var value = UInt64(limbs[index] >> UInt32(bitShift))
            if bitShift != 0 && index + 1 < limbs.count {
                value |= UInt64(limbs[index + 1]) << UInt64(32 - bitShift)
            }
            out.append(UInt32(value & 0xffff_ffff))
        }
        return SourceProofBigUInt(limbs: out)
    }

    func modulo(_ modulus: SourceProofBigUInt) -> SourceProofBigUInt {
        precondition(!modulus.isZero)
        guard !isZero else {
            return SourceProofBigUInt(0)
        }
        var result = SourceProofBigUInt(0)
        let one = SourceProofBigUInt(1)
        for index in stride(from: bitWidth - 1, through: 0, by: -1) {
            result = sourceProofSecpModDouble(result, modulus: modulus)
            if bit(at: index) {
                result = sourceProofSecpModAdd(result, one, modulus: modulus)
            }
        }
        return result
    }

    func fixedBigEndianData(byteCount: Int) -> Data {
        var out = [UInt8](repeating: 0, count: byteCount)
        for limbIndex in limbs.indices {
            let limb = limbs[limbIndex]
            for byteOffset in 0..<4 {
                let outIndex = byteCount - 1 - (limbIndex * 4 + byteOffset)
                if outIndex >= 0 {
                    out[outIndex] = UInt8((limb >> UInt32(byteOffset * 8)) & 0xff)
                }
            }
        }
        return Data(out)
    }
}

private struct SourceProofSecpFieldElement: Equatable {
    private static let modulus: [UInt64] = [
        0xffff_fffe_ffff_fc2f,
        0xffff_ffff_ffff_ffff,
        0xffff_ffff_ffff_ffff,
        0xffff_ffff_ffff_ffff
    ]
    private static let reductionConstant: UInt64 = 0x1_0000_03d1

    private let limbs: [UInt64]

    private var isZero: Bool {
        limbs.allSatisfy { $0 == 0 }
    }

    init(_ value: SourceProofBigUInt) {
        self.init(bigEndian: value.fixedBigEndianData(byteCount: 32))
    }

    init(bigEndian data: Data) {
        let bytes = [UInt8](data)
        var parsed = [UInt64](repeating: 0, count: 4)
        for limbIndex in 0..<4 {
            var limb: UInt64 = 0
            for byteOffset in 0..<8 {
                let byteIndex = bytes.count - 1 - (limbIndex * 8 + byteOffset)
                if byteIndex >= 0 {
                    limb |= UInt64(bytes[byteIndex]) << UInt64(byteOffset * 8)
                }
            }
            parsed[limbIndex] = limb
        }
        self.limbs = SourceProofSecpFieldElement.normalized(parsed)
    }

    private init(limbs: [UInt64], normalized: Bool) {
        self.limbs = normalized ? limbs : SourceProofSecpFieldElement.normalized(limbs)
    }

    func bigUInt() -> SourceProofBigUInt {
        var out = [UInt8](repeating: 0, count: 32)
        for limbIndex in 0..<4 {
            let limb = limbs[limbIndex]
            for byteOffset in 0..<8 {
                let outIndex = 31 - (limbIndex * 8 + byteOffset)
                out[outIndex] = UInt8((limb >> UInt64(byteOffset * 8)) & 0xff)
            }
        }
        return SourceProofBigUInt(bigEndian: Data(out))
    }

    func adding(_ other: SourceProofSecpFieldElement) -> SourceProofSecpFieldElement {
        var wide = [UInt64](repeating: 0, count: 5)
        var carry: UInt64 = 0
        for index in 0..<4 {
            let (sum1, overflow1) = limbs[index].addingReportingOverflow(other.limbs[index])
            let (sum2, overflow2) = sum1.addingReportingOverflow(carry)
            wide[index] = sum2
            carry = (overflow1 ? 1 : 0) + (overflow2 ? 1 : 0)
        }
        wide[4] = carry
        return SourceProofSecpFieldElement(limbs: SourceProofSecpFieldElement.reduce(wide), normalized: true)
    }

    func subtracting(_ other: SourceProofSecpFieldElement) -> SourceProofSecpFieldElement {
        if SourceProofSecpFieldElement.compare(limbs, other.limbs) >= 0 {
            return SourceProofSecpFieldElement(
                limbs: SourceProofSecpFieldElement.subtract(limbs, other.limbs),
                normalized: true
            )
        }
        return adding(other.negated())
    }

    func negated() -> SourceProofSecpFieldElement {
        guard !isZero else {
            return self
        }
        return SourceProofSecpFieldElement(
            limbs: SourceProofSecpFieldElement.subtract(SourceProofSecpFieldElement.modulus, limbs),
            normalized: true
        )
    }

    func multiplied(by other: SourceProofSecpFieldElement) -> SourceProofSecpFieldElement {
        var wide = [UInt64](repeating: 0, count: 9)
        for leftIndex in 0..<4 {
            var carry: UInt64 = 0
            for rightIndex in 0..<4 {
                let product = limbs[leftIndex].multipliedFullWidth(by: other.limbs[rightIndex])
                let target = leftIndex + rightIndex
                let (sum1, overflow1) = wide[target].addingReportingOverflow(product.low)
                let (sum2, overflow2) = sum1.addingReportingOverflow(carry)
                wide[target] = sum2
                var nextCarry = product.high
                if overflow1 {
                    nextCarry = nextCarry &+ 1
                }
                if overflow2 {
                    nextCarry = nextCarry &+ 1
                }
                carry = nextCarry
            }
            var target = leftIndex + 4
            while carry != 0 {
                let (sum, overflow) = wide[target].addingReportingOverflow(carry)
                wide[target] = sum
                carry = overflow ? 1 : 0
                target += 1
            }
        }
        return SourceProofSecpFieldElement(limbs: SourceProofSecpFieldElement.reduce(wide), normalized: true)
    }

    func squared() -> SourceProofSecpFieldElement {
        multiplied(by: self)
    }

    private static func normalized(_ input: [UInt64]) -> [UInt64] {
        var limbs = reduce(input)
        while compare(limbs, modulus) >= 0 {
            limbs = subtract(limbs, modulus)
        }
        return limbs
    }

    private static func reduce(_ input: [UInt64]) -> [UInt64] {
        var acc = input
        if acc.count < 5 {
            acc.append(contentsOf: repeatElement(0, count: 5 - acc.count))
        }
        while acc.count > 4 && acc[4...].contains(where: { $0 != 0 }) {
            let overflow = Array(acc.dropFirst(4))
            for index in 4..<acc.count {
                acc[index] = 0
            }
            for (offset, value) in overflow.enumerated() where value != 0 {
                addProduct(value, by: 977, shiftedBy: offset, to: &acc)
                addShifted32(value, shiftedBy: offset, to: &acc)
            }
        }
        var out = Array(acc.prefix(4))
        while compare(out, modulus) >= 0 {
            out = subtract(out, modulus)
        }
        return out
    }

    private static func addProduct(_ value: UInt64,
                                   by small: UInt64,
                                   shiftedBy limbOffset: Int,
                                   to acc: inout [UInt64]) {
        let product = value.multipliedFullWidth(by: small)
        add(product.low, at: limbOffset, to: &acc)
        if product.high != 0 {
            add(product.high, at: limbOffset + 1, to: &acc)
        }
    }

    private static func addShifted32(_ value: UInt64,
                                     shiftedBy limbOffset: Int,
                                     to acc: inout [UInt64]) {
        add(value << 32, at: limbOffset, to: &acc)
        let high = value >> 32
        if high != 0 {
            add(high, at: limbOffset + 1, to: &acc)
        }
    }

    private static func add(_ value: UInt64, at index: Int, to acc: inout [UInt64]) {
        var target = index
        var carry = value
        while carry != 0 {
            while target >= acc.count {
                acc.append(0)
            }
            let (sum, overflow) = acc[target].addingReportingOverflow(carry)
            acc[target] = sum
            carry = overflow ? 1 : 0
            target += 1
        }
    }

    private static func compare(_ left: [UInt64], _ right: [UInt64]) -> Int {
        for index in stride(from: 3, through: 0, by: -1) {
            let leftValue = index < left.count ? left[index] : 0
            let rightValue = index < right.count ? right[index] : 0
            if leftValue != rightValue {
                return leftValue < rightValue ? -1 : 1
            }
        }
        return 0
    }

    private static func subtract(_ left: [UInt64], _ right: [UInt64]) -> [UInt64] {
        var out = [UInt64](repeating: 0, count: 4)
        var borrow: UInt64 = 0
        for index in 0..<4 {
            let rightValue = (index < right.count ? right[index] : 0) &+ borrow
            let overflowRight = rightValue < borrow
            let (diff, overflowDiff) = left[index].subtractingReportingOverflow(rightValue)
            out[index] = diff
            borrow = (overflowRight || overflowDiff) ? 1 : 0
        }
        return out
    }
}

private struct SourceProofSecpAffinePoint {
    let x: SourceProofBigUInt
    let y: SourceProofBigUInt
}

private struct SourceProofSecpJacobianPoint {
    let x: SourceProofBigUInt
    let y: SourceProofBigUInt
    let z: SourceProofBigUInt
    let isInfinity: Bool

    static var infinity: SourceProofSecpJacobianPoint {
        SourceProofSecpJacobianPoint(
            x: SourceProofBigUInt(0),
            y: SourceProofBigUInt(0),
            z: SourceProofBigUInt(0),
            isInfinity: true
        )
    }
}

private let sourceProofSecpFieldPrime = SourceProofBigUInt(bigEndian: sccpSecp256k1FieldPrimeBe)
private let sourceProofSecpScalarOrder = SourceProofBigUInt(bigEndian: sccpSecp256k1ScalarOrderBe)
private let sourceProofSecpFieldSqrtExponent =
    sourceProofSecpFieldPrime.adding(SourceProofBigUInt(1)).shiftedRight(2)
private let sourceProofSecpGenerator = SourceProofSecpAffinePoint(
    x: SourceProofBigUInt(bigEndian: sccpSecp256k1GeneratorXBe),
    y: SourceProofBigUInt(bigEndian: sccpSecp256k1GeneratorYBe)
)

private func sourceProofSecpModAdd(_ left: SourceProofBigUInt,
                                   _ right: SourceProofBigUInt,
                                   modulus: SourceProofBigUInt) -> SourceProofBigUInt {
    if modulus == sourceProofSecpFieldPrime {
        return SourceProofSecpFieldElement(left)
            .adding(SourceProofSecpFieldElement(right))
            .bigUInt()
    }
    let sum = left.adding(right)
    return sum.compare(modulus) >= 0 ? sum.subtracting(modulus) : sum
}

private func sourceProofSecpModDouble(_ value: SourceProofBigUInt,
                                      modulus: SourceProofBigUInt) -> SourceProofBigUInt {
    sourceProofSecpModAdd(value, value, modulus: modulus)
}

private func sourceProofSecpModSub(_ left: SourceProofBigUInt,
                                   _ right: SourceProofBigUInt,
                                   modulus: SourceProofBigUInt) -> SourceProofBigUInt {
    if modulus == sourceProofSecpFieldPrime {
        return SourceProofSecpFieldElement(left)
            .subtracting(SourceProofSecpFieldElement(right))
            .bigUInt()
    }
    if left.compare(right) >= 0 {
        return left.subtracting(right)
    }
    return modulus.subtracting(right.subtracting(left))
}

private func sourceProofSecpModNegate(_ value: SourceProofBigUInt,
                                      modulus: SourceProofBigUInt) -> SourceProofBigUInt {
    if modulus == sourceProofSecpFieldPrime {
        return SourceProofSecpFieldElement(value).negated().bigUInt()
    }
    return value.isZero ? value : modulus.subtracting(value)
}

private func sourceProofSecpModMul(_ left: SourceProofBigUInt,
                                   _ right: SourceProofBigUInt,
                                   modulus: SourceProofBigUInt) -> SourceProofBigUInt {
    if modulus == sourceProofSecpFieldPrime {
        return SourceProofSecpFieldElement(left)
            .multiplied(by: SourceProofSecpFieldElement(right))
            .bigUInt()
    }
    var result = SourceProofBigUInt(0)
    var addend = left.modulo(modulus)
    for index in 0..<right.bitWidth {
        if right.bit(at: index) {
            result = sourceProofSecpModAdd(result, addend, modulus: modulus)
        }
        addend = sourceProofSecpModDouble(addend, modulus: modulus)
    }
    return result
}

private func sourceProofSecpModSquare(_ value: SourceProofBigUInt,
                                      modulus: SourceProofBigUInt) -> SourceProofBigUInt {
    sourceProofSecpModMul(value, value, modulus: modulus)
}

private func sourceProofSecpModPow(_ base: SourceProofBigUInt,
                                   _ exponent: SourceProofBigUInt,
                                   modulus: SourceProofBigUInt) -> SourceProofBigUInt {
    var result = SourceProofBigUInt(1)
    var power = base.modulo(modulus)
    for index in 0..<exponent.bitWidth {
        if exponent.bit(at: index) {
            result = sourceProofSecpModMul(result, power, modulus: modulus)
        }
        power = sourceProofSecpModSquare(power, modulus: modulus)
    }
    return result
}

private func sourceProofSecpModInverse(_ value: SourceProofBigUInt,
                                       modulus: SourceProofBigUInt) -> SourceProofBigUInt? {
    guard !value.isZero else {
        return nil
    }
    return sourceProofSecpModPow(
        value,
        modulus.subtracting(SourceProofBigUInt(2)),
        modulus: modulus
    )
}

private func sourceProofSecpJacobian(from point: SourceProofSecpAffinePoint) -> SourceProofSecpJacobianPoint {
    SourceProofSecpJacobianPoint(
        x: point.x,
        y: point.y,
        z: SourceProofBigUInt(1),
        isInfinity: false
    )
}

private func sourceProofSecpAffine(from point: SourceProofSecpJacobianPoint) -> SourceProofSecpAffinePoint? {
    guard !point.isInfinity else {
        return nil
    }
    guard let zInverse = sourceProofSecpModInverse(point.z, modulus: sourceProofSecpFieldPrime) else {
        return nil
    }
    let z2 = sourceProofSecpModSquare(zInverse, modulus: sourceProofSecpFieldPrime)
    let z3 = sourceProofSecpModMul(z2, zInverse, modulus: sourceProofSecpFieldPrime)
    return SourceProofSecpAffinePoint(
        x: sourceProofSecpModMul(point.x, z2, modulus: sourceProofSecpFieldPrime),
        y: sourceProofSecpModMul(point.y, z3, modulus: sourceProofSecpFieldPrime)
    )
}

private func sourceProofSecpJacobianDouble(_ point: SourceProofSecpJacobianPoint) -> SourceProofSecpJacobianPoint {
    guard !point.isInfinity, !point.y.isZero else {
        return .infinity
    }
    let two = SourceProofBigUInt(2)
    let three = SourceProofBigUInt(3)
    let eight = SourceProofBigUInt(8)
    let a = sourceProofSecpModSquare(point.x, modulus: sourceProofSecpFieldPrime)
    let b = sourceProofSecpModSquare(point.y, modulus: sourceProofSecpFieldPrime)
    let c = sourceProofSecpModSquare(b, modulus: sourceProofSecpFieldPrime)
    let xPlusB = sourceProofSecpModAdd(point.x, b, modulus: sourceProofSecpFieldPrime)
    let dInner = sourceProofSecpModSub(
        sourceProofSecpModSub(
            sourceProofSecpModSquare(xPlusB, modulus: sourceProofSecpFieldPrime),
            a,
            modulus: sourceProofSecpFieldPrime
        ),
        c,
        modulus: sourceProofSecpFieldPrime
    )
    let d = sourceProofSecpModMul(two, dInner, modulus: sourceProofSecpFieldPrime)
    let e = sourceProofSecpModMul(three, a, modulus: sourceProofSecpFieldPrime)
    let f = sourceProofSecpModSquare(e, modulus: sourceProofSecpFieldPrime)
    let x3 = sourceProofSecpModSub(
        sourceProofSecpModSub(f, d, modulus: sourceProofSecpFieldPrime),
        d,
        modulus: sourceProofSecpFieldPrime
    )
    let y3 = sourceProofSecpModSub(
        sourceProofSecpModMul(e, sourceProofSecpModSub(d, x3, modulus: sourceProofSecpFieldPrime), modulus: sourceProofSecpFieldPrime),
        sourceProofSecpModMul(eight, c, modulus: sourceProofSecpFieldPrime),
        modulus: sourceProofSecpFieldPrime
    )
    let z3 = sourceProofSecpModMul(
        two,
        sourceProofSecpModMul(point.y, point.z, modulus: sourceProofSecpFieldPrime),
        modulus: sourceProofSecpFieldPrime
    )
    return SourceProofSecpJacobianPoint(x: x3, y: y3, z: z3, isInfinity: false)
}

private func sourceProofSecpJacobianAdd(_ left: SourceProofSecpJacobianPoint,
                                        _ right: SourceProofSecpJacobianPoint) -> SourceProofSecpJacobianPoint {
    if left.isInfinity {
        return right
    }
    if right.isInfinity {
        return left
    }
    let two = SourceProofBigUInt(2)
    let z1z1 = sourceProofSecpModSquare(left.z, modulus: sourceProofSecpFieldPrime)
    let z2z2 = sourceProofSecpModSquare(right.z, modulus: sourceProofSecpFieldPrime)
    let u1 = sourceProofSecpModMul(left.x, z2z2, modulus: sourceProofSecpFieldPrime)
    let u2 = sourceProofSecpModMul(right.x, z1z1, modulus: sourceProofSecpFieldPrime)
    let s1 = sourceProofSecpModMul(
        left.y,
        sourceProofSecpModMul(right.z, z2z2, modulus: sourceProofSecpFieldPrime),
        modulus: sourceProofSecpFieldPrime
    )
    let s2 = sourceProofSecpModMul(
        right.y,
        sourceProofSecpModMul(left.z, z1z1, modulus: sourceProofSecpFieldPrime),
        modulus: sourceProofSecpFieldPrime
    )
    let h = sourceProofSecpModSub(u2, u1, modulus: sourceProofSecpFieldPrime)
    let sDelta = sourceProofSecpModSub(s2, s1, modulus: sourceProofSecpFieldPrime)
    if h.isZero {
        return sDelta.isZero ? sourceProofSecpJacobianDouble(left) : .infinity
    }
    let i = sourceProofSecpModSquare(sourceProofSecpModMul(two, h, modulus: sourceProofSecpFieldPrime), modulus: sourceProofSecpFieldPrime)
    let j = sourceProofSecpModMul(h, i, modulus: sourceProofSecpFieldPrime)
    let r = sourceProofSecpModMul(two, sDelta, modulus: sourceProofSecpFieldPrime)
    let v = sourceProofSecpModMul(u1, i, modulus: sourceProofSecpFieldPrime)
    let x3 = sourceProofSecpModSub(
        sourceProofSecpModSub(sourceProofSecpModSquare(r, modulus: sourceProofSecpFieldPrime), j, modulus: sourceProofSecpFieldPrime),
        sourceProofSecpModMul(two, v, modulus: sourceProofSecpFieldPrime),
        modulus: sourceProofSecpFieldPrime
    )
    let y3 = sourceProofSecpModSub(
        sourceProofSecpModMul(r, sourceProofSecpModSub(v, x3, modulus: sourceProofSecpFieldPrime), modulus: sourceProofSecpFieldPrime),
        sourceProofSecpModMul(two, sourceProofSecpModMul(s1, j, modulus: sourceProofSecpFieldPrime), modulus: sourceProofSecpFieldPrime),
        modulus: sourceProofSecpFieldPrime
    )
    let z3 = sourceProofSecpModMul(
        sourceProofSecpModSub(
            sourceProofSecpModSub(
                sourceProofSecpModSquare(sourceProofSecpModAdd(left.z, right.z, modulus: sourceProofSecpFieldPrime), modulus: sourceProofSecpFieldPrime),
                z1z1,
                modulus: sourceProofSecpFieldPrime
            ),
            z2z2,
            modulus: sourceProofSecpFieldPrime
        ),
        h,
        modulus: sourceProofSecpFieldPrime
    )
    return SourceProofSecpJacobianPoint(x: x3, y: y3, z: z3, isInfinity: false)
}

private func sourceProofSecpScalarMultiply(_ scalar: SourceProofBigUInt,
                                           _ point: SourceProofSecpAffinePoint) -> SourceProofSecpJacobianPoint {
    var result = SourceProofSecpJacobianPoint.infinity
    var addend = sourceProofSecpJacobian(from: point)
    for index in 0..<scalar.bitWidth {
        if scalar.bit(at: index) {
            result = sourceProofSecpJacobianAdd(result, addend)
        }
        addend = sourceProofSecpJacobianDouble(addend)
    }
    return result
}

private func sourceProofTronRecoveredSignerAddress20(messageHash: Data, signature: Data) -> Data? {
    guard messageHash.count == 32,
          sourceProofTronRecoverableSignatureIsCanonical(signature) else {
        return nil
    }
    let r = SourceProofBigUInt(bigEndian: signature.subdata(in: 0..<32))
    let s = SourceProofBigUInt(bigEndian: signature.subdata(in: 32..<64))
    let recoveryByte = Int(signature[signature.index(signature.startIndex, offsetBy: 64)])
    let recoveryId = recoveryByte >= 27 ? recoveryByte - 27 : recoveryByte
    var x = r
    if recoveryId >= 2 {
        x = x.adding(sourceProofSecpScalarOrder)
    }
    guard x.compare(sourceProofSecpFieldPrime) < 0 else {
        return nil
    }
    let x2 = sourceProofSecpModSquare(x, modulus: sourceProofSecpFieldPrime)
    let x3 = sourceProofSecpModMul(x2, x, modulus: sourceProofSecpFieldPrime)
    let alpha = sourceProofSecpModAdd(x3, SourceProofBigUInt(7), modulus: sourceProofSecpFieldPrime)
    var y = sourceProofSecpModPow(alpha, sourceProofSecpFieldSqrtExponent, modulus: sourceProofSecpFieldPrime)
    guard sourceProofSecpModSquare(y, modulus: sourceProofSecpFieldPrime) == alpha else {
        return nil
    }
    if y.bit(at: 0) != ((recoveryId & 1) == 1) {
        y = sourceProofSecpModNegate(y, modulus: sourceProofSecpFieldPrime)
    }
    let rPoint = SourceProofSecpAffinePoint(x: x, y: y)
    guard let rInverse = sourceProofSecpModInverse(r, modulus: sourceProofSecpScalarOrder) else {
        return nil
    }
    let e = SourceProofBigUInt(bigEndian: messageHash).modulo(sourceProofSecpScalarOrder)
    let eNeg = sourceProofSecpModNegate(e, modulus: sourceProofSecpScalarOrder)
    let u1 = sourceProofSecpModMul(eNeg, rInverse, modulus: sourceProofSecpScalarOrder)
    let u2 = sourceProofSecpModMul(s, rInverse, modulus: sourceProofSecpScalarOrder)
    let publicKeyJacobian = sourceProofSecpJacobianAdd(
        sourceProofSecpScalarMultiply(u1, sourceProofSecpGenerator),
        sourceProofSecpScalarMultiply(u2, rPoint)
    )
    guard let publicKey = sourceProofSecpAffine(from: publicKeyJacobian) else {
        return nil
    }
    var uncompressed = publicKey.x.fixedBigEndianData(byteCount: 32)
    uncompressed.append(publicKey.y.fixedBigEndianData(byteCount: 32))
    return Data(irohaKeccak256(uncompressed).suffix(20))
}

private func sourceProofIsNonZeroTronAddress(_ address: Data) -> Bool {
    address.count == 21 &&
        address.first == 0x41 &&
        !address.dropFirst().allSatisfy({ $0 == 0 })
}

private func sourceProofValidateTronWitnessSchedulePayload(_ payload: Data) throws {
    guard payload.count >= 5, payload[payload.startIndex] == 1 else {
        throw SccpSourceProofHashError.invalidValidatorSet("witnessSchedulePayload")
    }
    let countRange = payload.index(payload.startIndex, offsetBy: 1)..<payload.index(payload.startIndex, offsetBy: 5)
    let witnessCount = Int(sourceProofReadU32Le(payload.subdata(in: countRange)))
    guard witnessCount > 0,
          witnessCount <= sccpTronMaxWitnesses,
          payload.count == 5 + witnessCount * 29 else {
        throw SccpSourceProofHashError.invalidValidatorSet("witnessSchedulePayload")
    }
    var cursor = payload.index(payload.startIndex, offsetBy: 5)
    var seenAddresses = Set<String>()
    var totalWeight: UInt64 = 0
    for index in 0..<witnessCount {
        let addressEnd = payload.index(cursor, offsetBy: 21)
        let address = payload.subdata(in: cursor..<addressEnd)
        cursor = addressEnd
        guard sourceProofIsNonZeroTronAddress(address) else {
            throw SccpSourceProofHashError.invalidValidatorSet("witnessSchedulePayload[\(index)]")
        }
        guard seenAddresses.insert(address.hexEncodedString()).inserted else {
            throw SccpSourceProofHashError.invalidValidatorSet("witnessSchedulePayload[\(index)]")
        }
        let weightEnd = payload.index(cursor, offsetBy: 8)
        let weight = sourceProofReadU64Le(payload.subdata(in: cursor..<weightEnd))
        cursor = weightEnd
        guard weight != 0 else {
            throw SccpSourceProofHashError.invalidValidatorSet("witnessSchedulePayload[\(index)]")
        }
        let total = totalWeight.addingReportingOverflow(weight)
        guard !total.overflow else {
            throw SccpSourceProofHashError.invalidValidatorSet("witnessSchedulePayload")
        }
        totalWeight = total.partialValue
    }
}

private struct SourceProofNormalizedTronWitnessSeal {
    let version: UInt8
    let totalWeight: UInt64
    let signedWeight: UInt64
    let solidBlockMessageHash: Data
    let witnessAddresses: [Data]
    let witnessWeights: [UInt64]
    let signersBitmap: Data
    let signatures: [Data]
    let witnessScheduleHash: Data
}

private struct SourceProofNormalizedTronWitnessScheduleTransitionMessage {
    let version: UInt8
    let sourceDomain: UInt32
    let fromWitnessScheduleEpoch: UInt64
    let toWitnessScheduleEpoch: UInt64
    let transitionBlockNumber: UInt64
    let transitionBlockHash: Data
    let parentWitnessScheduleHash: Data
    let nextWitnessScheduleHash: Data
    let nextWitnessSchedulePayloadHash: Data
}

private func sourceProofCompareU64Product(_ left: UInt64,
                                          _ leftMultiplier: UInt64,
                                          _ right: UInt64,
                                          _ rightMultiplier: UInt64) -> Int {
    let leftProduct = left.multipliedFullWidth(by: leftMultiplier)
    let rightProduct = right.multipliedFullWidth(by: rightMultiplier)
    if leftProduct.high != rightProduct.high {
        return leftProduct.high < rightProduct.high ? -1 : 1
    }
    if leftProduct.low != rightProduct.low {
        return leftProduct.low < rightProduct.low ? -1 : 1
    }
    return 0
}

private func sourceProofTronWitnessSealSignerIndices(signersBitmap: Data,
                                                     rosterCount: Int) throws -> [Int] {
    guard rosterCount > 0, signersBitmap.count == (rosterCount + 7) / 8 else {
        throw SccpSourceProofHashError.invalidValidatorSet("signersBitmap")
    }
    var indices: [Int] = []
    for (byteIndex, value) in signersBitmap.enumerated() {
        for bitIndex in 0..<8 {
            guard ((value >> UInt8(bitIndex)) & 1) != 0 else {
                continue
            }
            let witnessIndex = byteIndex * 8 + bitIndex
            guard witnessIndex < rosterCount else {
                throw SccpSourceProofHashError.invalidValidatorSet("signersBitmap")
            }
            indices.append(witnessIndex)
        }
    }
    guard !indices.isEmpty else {
        throw SccpSourceProofHashError.invalidValidatorSet("signersBitmap")
    }
    return indices
}

private func sourceProofNormalizeTronWitnessSeal(version: UInt8,
                                                 totalWeight: UInt64,
                                                 signedWeight: UInt64,
                                                 solidBlockMessageHash: String,
                                                 witnessAddresses: [String],
                                                 witnessWeights: [UInt64],
                                                 signersBitmap: Data,
                                                 signatures: [Data]) throws -> SourceProofNormalizedTronWitnessSeal {
    guard version == 1,
          totalWeight != 0,
          signedWeight != 0,
          !witnessAddresses.isEmpty,
          witnessAddresses.count == witnessWeights.count,
          witnessAddresses.count <= sccpTronMaxWitnesses else {
        throw SccpSourceProofHashError.invalidValidatorSet("witnessSeal")
    }
    let messageHash = try sourceProofNonZeroBytesFromHex32(
        solidBlockMessageHash,
        field: "solidBlockMessageHash"
    )
    var normalizedAddresses: [Data] = []
    var normalizedWeights: [UInt64] = []
    var seenAddresses = Set<String>()
    var computedTotalWeight: UInt64 = 0
    for (index, pair) in zip(witnessAddresses, witnessWeights).enumerated() {
        let address = try sourceProofBytesFromHex21(pair.0, field: "witnessAddresses[\(index)]")
        guard sourceProofIsNonZeroTronAddress(address),
              seenAddresses.insert(address.hexEncodedString()).inserted,
              pair.1 != 0 else {
            throw SccpSourceProofHashError.invalidValidatorSet("witnessAddresses[\(index)]")
        }
        let total = computedTotalWeight.addingReportingOverflow(pair.1)
        guard !total.overflow else {
            throw SccpSourceProofHashError.invalidValidatorSet("witnessWeights")
        }
        computedTotalWeight = total.partialValue
        normalizedAddresses.append(address)
        normalizedWeights.append(pair.1)
    }
    guard computedTotalWeight == totalWeight else {
        throw SccpSourceProofHashError.invalidValidatorSet("totalWeight")
    }
    let signerIndices = try sourceProofTronWitnessSealSignerIndices(
        signersBitmap: signersBitmap,
        rosterCount: normalizedAddresses.count
    )
    guard signatures.count == signerIndices.count else {
        throw SccpSourceProofHashError.invalidValidatorSet("signatures")
    }
    var computedSignedWeight: UInt64 = 0
    var normalizedSignatures: [Data] = []
    for (signatureIndex, witnessIndex) in signerIndices.enumerated() {
        let signature = signatures[signatureIndex]
        guard sourceProofTronRecoverableSignatureIsCanonical(signature) else {
            throw SccpSourceProofHashError.invalidValidatorSet("signatures[\(signatureIndex)]")
        }
        guard let recovered = sourceProofTronRecoveredSignerAddress20(
            messageHash: messageHash,
            signature: signature
        ), recovered == Data(normalizedAddresses[witnessIndex].dropFirst()) else {
            throw SccpSourceProofHashError.invalidValidatorSet("signatures[\(signatureIndex)]")
        }
        let signed = computedSignedWeight.addingReportingOverflow(normalizedWeights[witnessIndex])
        guard !signed.overflow else {
            throw SccpSourceProofHashError.invalidValidatorSet("signedWeight")
        }
        computedSignedWeight = signed.partialValue
        normalizedSignatures.append(signature)
    }
    guard computedSignedWeight == signedWeight,
          sourceProofCompareU64Product(computedSignedWeight, 3, computedTotalWeight, 2) > 0 else {
        throw SccpSourceProofHashError.invalidValidatorSet("signedWeight")
    }
    let witnessPayload = try canonicalTronWitnessSchedulePayloadBytes(
        witnessAddresses: witnessAddresses,
        witnessWeights: witnessWeights
    )
    let witnessScheduleHash = try sourceProofBytesFromHex32(
        tronWitnessScheduleHashFromPayload(payload: witnessPayload),
        field: "witnessScheduleHash"
    )
    return SourceProofNormalizedTronWitnessSeal(
        version: version,
        totalWeight: totalWeight,
        signedWeight: signedWeight,
        solidBlockMessageHash: messageHash,
        witnessAddresses: normalizedAddresses,
        witnessWeights: normalizedWeights,
        signersBitmap: signersBitmap,
        signatures: normalizedSignatures,
        witnessScheduleHash: witnessScheduleHash
    )
}

private func sourceProofCanonicalTronWitnessSealProofBytes(
    _ proof: SourceProofNormalizedTronWitnessSeal
) throws -> Data {
    var out = Data()
    out.append(proof.version)
    sourceProofAppendU64Le(proof.totalWeight, to: &out)
    sourceProofAppendU64Le(proof.signedWeight, to: &out)
    out.append(proof.solidBlockMessageHash)
    sourceProofAppendU32Le(UInt32(proof.witnessAddresses.count), to: &out)
    for address in proof.witnessAddresses {
        sourceProofAppendDataVector(address, to: &out)
    }
    sourceProofAppendU32Le(UInt32(proof.witnessWeights.count), to: &out)
    for weight in proof.witnessWeights {
        sourceProofAppendU64Le(weight, to: &out)
    }
    sourceProofAppendDataVector(proof.signersBitmap, to: &out)
    sourceProofAppendU32Le(UInt32(proof.signatures.count), to: &out)
    for signature in proof.signatures {
        sourceProofAppendDataVector(signature, to: &out)
    }
    return out
}

private func sourceProofNormalizeTronWitnessScheduleTransitionMessage(version: UInt8,
                                                                      sourceDomain: UInt32,
                                                                      fromWitnessScheduleEpoch: UInt64,
                                                                      toWitnessScheduleEpoch: UInt64,
                                                                      transitionBlockNumber: UInt64,
                                                                      transitionBlockHash: String,
                                                                      parentWitnessScheduleHash: String,
                                                                      nextWitnessScheduleHash: String,
                                                                      nextWitnessSchedulePayloadHash: String?,
                                                                      nextWitnessSchedulePayload: Data?) throws -> SourceProofNormalizedTronWitnessScheduleTransitionMessage {
    guard version == 1,
          sourceDomain == sccpDomainTron,
          fromWitnessScheduleEpoch < UInt64.max,
          fromWitnessScheduleEpoch + 1 == toWitnessScheduleEpoch,
          transitionBlockNumber != 0 else {
        throw SccpSourceProofHashError.invalidValidatorSet("witnessScheduleTransitionMessage")
    }
    let transitionBlockHashBytes = try sourceProofNonZeroBytesFromHex32(
        transitionBlockHash,
        field: "transitionBlockHash"
    )
    let parentScheduleHash = try sourceProofNonZeroBytesFromHex32(
        parentWitnessScheduleHash,
        field: "parentWitnessScheduleHash"
    )
    let nextScheduleHash = try sourceProofNonZeroBytesFromHex32(
        nextWitnessScheduleHash,
        field: "nextWitnessScheduleHash"
    )
    let payloadHash: Data
    if let nextWitnessSchedulePayloadHash {
        payloadHash = try sourceProofNonZeroBytesFromHex32(
            nextWitnessSchedulePayloadHash,
            field: "nextWitnessSchedulePayloadHash"
        )
    } else if let nextWitnessSchedulePayload {
        payloadHash = try sourceProofBytesFromHex32(
            tronWitnessSchedulePayloadHash(payload: nextWitnessSchedulePayload),
            field: "nextWitnessSchedulePayloadHash"
        )
    } else {
        throw SccpSourceProofHashError.invalidValidatorSet("nextWitnessSchedulePayloadHash")
    }
    if let nextWitnessSchedulePayload {
        let derivedPayloadHash = try sourceProofBytesFromHex32(
            tronWitnessSchedulePayloadHash(payload: nextWitnessSchedulePayload),
            field: "nextWitnessSchedulePayloadHash"
        )
        guard payloadHash == derivedPayloadHash else {
            throw SccpSourceProofHashError.invalidValidatorSet("nextWitnessSchedulePayloadHash")
        }
        let derivedScheduleHash = try sourceProofBytesFromHex32(
            tronWitnessScheduleHashFromPayload(payload: nextWitnessSchedulePayload),
            field: "nextWitnessScheduleHash"
        )
        guard nextScheduleHash == derivedScheduleHash else {
            throw SccpSourceProofHashError.invalidValidatorSet("nextWitnessScheduleHash")
        }
    }
    return SourceProofNormalizedTronWitnessScheduleTransitionMessage(
        version: version,
        sourceDomain: sourceDomain,
        fromWitnessScheduleEpoch: fromWitnessScheduleEpoch,
        toWitnessScheduleEpoch: toWitnessScheduleEpoch,
        transitionBlockNumber: transitionBlockNumber,
        transitionBlockHash: transitionBlockHashBytes,
        parentWitnessScheduleHash: parentScheduleHash,
        nextWitnessScheduleHash: nextScheduleHash,
        nextWitnessSchedulePayloadHash: payloadHash
    )
}

private func sourceProofCanonicalTronWitnessScheduleTransitionMessageBytes(
    _ message: SourceProofNormalizedTronWitnessScheduleTransitionMessage
) throws -> Data {
    var out = Data()
    out.append(message.version)
    sourceProofAppendU32Le(message.sourceDomain, to: &out)
    sourceProofAppendU64Le(message.fromWitnessScheduleEpoch, to: &out)
    sourceProofAppendU64Le(message.toWitnessScheduleEpoch, to: &out)
    sourceProofAppendU64Le(message.transitionBlockNumber, to: &out)
    out.append(message.transitionBlockHash)
    out.append(message.parentWitnessScheduleHash)
    out.append(message.nextWitnessScheduleHash)
    out.append(message.nextWitnessSchedulePayloadHash)
    return out
}

private func sourceProofCompareBytes(_ left: Data, _ right: Data) -> Int {
    if left.count != right.count {
        return left.count - right.count
    }
    for (leftByte, rightByte) in zip(left, right) where leftByte != rightByte {
        return Int(leftByte) - Int(rightByte)
    }
    return 0
}

/// Canonical parent-linked TRON solid-block header proof bytes.
public func canonicalTronSolidBlockHeaderProofBytes(rawData: Data,
                                                    witnessSignature: Data,
                                                    parentRawData: Data,
                                                    parentWitnessSignature: Data,
                                                    rawDataHash: String,
                                                    parentRawDataHash: String,
                                                    blockId: String,
                                                    txTrieRoot: String,
                                                    accountStateRoot: String,
                                                    parentBlockId: String,
                                                    witnessAddress: String,
                                                    timestampMs: UInt64,
                                                    headerVersion: UInt32,
                                                    version: UInt8 = 1) throws -> Data {
    guard version == 1, !rawData.isEmpty, !parentRawData.isEmpty,
          rawData.count <= sccpTronMaxRawHeaderBytes,
          parentRawData.count <= sccpTronMaxRawHeaderBytes,
          witnessSignature.count == 65, parentWitnessSignature.count == 65,
          sourceProofTronRecoverableSignatureIsCanonical(witnessSignature),
          sourceProofTronRecoverableSignatureIsCanonical(parentWitnessSignature),
          timestampMs != 0, headerVersion != 0 else {
        throw SccpSourceProofHashError.invalidValidatorSet("solidBlockHeaderProof")
    }
    let txTrieRootBytes = try sourceProofBytesFromHex32(txTrieRoot, field: "txTrieRoot")
    let accountStateRootBytes = try sourceProofBytesFromHex32(accountStateRoot, field: "accountStateRoot")
    let parentBlockIdBytes = try sourceProofBytesFromHex32(parentBlockId, field: "parentBlockId")
    let witnessAddressBytes = try sourceProofBytesFromHex21(witnessAddress, field: "witnessAddress")
    guard !txTrieRootBytes.allSatisfy({ $0 == 0 }),
          !accountStateRootBytes.allSatisfy({ $0 == 0 }),
          !parentBlockIdBytes.allSatisfy({ $0 == 0 }),
          sourceProofIsNonZeroTronAddress(witnessAddressBytes) else {
        throw SccpSourceProofHashError.invalidValidatorSet("solidBlockHeaderProof")
    }
    let fields = try sourceProofDecodeTronRawBlockHeaderFields(rawData, field: "rawData")
    let parentFields = try sourceProofDecodeTronRawBlockHeaderFields(parentRawData, field: "parentRawData")
    let rawDataHashBytes = try sourceProofBytesFromHex32(rawDataHash, field: "rawDataHash")
    let parentRawDataHashBytes = try sourceProofBytesFromHex32(parentRawDataHash, field: "parentRawDataHash")
    let blockIdBytes = try sourceProofBytesFromHex32(blockId, field: "blockId")
    guard rawDataHashBytes == Data(SHA256.hash(data: rawData)),
          parentRawDataHashBytes == Data(SHA256.hash(data: parentRawData)),
          blockIdBytes == sourceProofTronBlockId(number: fields.number, rawDataHash: rawDataHashBytes),
          parentBlockIdBytes == fields.parentBlockId,
          parentBlockIdBytes == sourceProofTronBlockId(
              number: parentFields.number,
              rawDataHash: parentRawDataHashBytes
          ),
          parentFields.number < UInt64.max,
          parentFields.number + 1 == fields.number,
          parentFields.timestampMs < fields.timestampMs,
          txTrieRootBytes == fields.txTrieRoot,
          accountStateRootBytes == fields.accountStateRoot,
          witnessAddressBytes == fields.witnessAddress,
          timestampMs == fields.timestampMs,
          headerVersion == fields.headerVersion else {
        throw SccpSourceProofHashError.invalidValidatorSet("solidBlockHeaderProof")
    }
    var out = Data()
    out.append(version)
    sourceProofAppendDataVector(rawData, to: &out)
    sourceProofAppendDataVector(witnessSignature, to: &out)
    sourceProofAppendDataVector(parentRawData, to: &out)
    sourceProofAppendDataVector(parentWitnessSignature, to: &out)
    out.append(rawDataHashBytes)
    out.append(parentRawDataHashBytes)
    out.append(blockIdBytes)
    out.append(txTrieRootBytes)
    out.append(accountStateRootBytes)
    out.append(parentBlockIdBytes)
    sourceProofAppendDataVector(witnessAddressBytes, to: &out)
    sourceProofAppendU64Le(timestampMs, to: &out)
    sourceProofAppendU32Le(headerVersion, to: &out)
    return out
}

/// Hash of canonical parent-linked TRON solid-block header proof bytes.
public func tronSolidBlockHeaderProofHash(rawData: Data,
                                          witnessSignature: Data,
                                          parentRawData: Data,
                                          parentWitnessSignature: Data,
                                          rawDataHash: String,
                                          parentRawDataHash: String,
                                          blockId: String,
                                          txTrieRoot: String,
                                          accountStateRoot: String,
                                          parentBlockId: String,
                                          witnessAddress: String,
                                          timestampMs: UInt64,
                                          headerVersion: UInt32,
                                          version: UInt8 = 1) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:tron:solid-block-header-proof:v1",
        payload: canonicalTronSolidBlockHeaderProofBytes(
            rawData: rawData,
            witnessSignature: witnessSignature,
            parentRawData: parentRawData,
            parentWitnessSignature: parentWitnessSignature,
            rawDataHash: rawDataHash,
            parentRawDataHash: parentRawDataHash,
            blockId: blockId,
            txTrieRoot: txTrieRoot,
            accountStateRoot: accountStateRoot,
            parentBlockId: parentBlockId,
            witnessAddress: witnessAddress,
            timestampMs: timestampMs,
            headerVersion: headerVersion,
            version: version
        )
    )
}

/// Canonical TRON DPoS witness-schedule payload bytes checked by transition proofs.
public func canonicalTronWitnessSchedulePayloadBytes(witnessAddresses: [String],
                                                     witnessWeights: [UInt64]) throws -> Data {
    guard !witnessAddresses.isEmpty, witnessAddresses.count == witnessWeights.count else {
        throw SccpSourceProofHashError.invalidValidatorSet("witnessAddresses")
    }
    guard witnessAddresses.count <= sccpTronMaxWitnesses else {
        throw SccpSourceProofHashError.invalidValidatorSet("witnessAddresses")
    }
    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(UInt32(witnessAddresses.count), to: &out)
    var seenAddresses = Set<String>()
    var totalWeight: UInt64 = 0
    for (index, pair) in zip(witnessAddresses, witnessWeights).enumerated() {
        let address = try sourceProofBytesFromHex21(pair.0, field: "witnessAddresses[\(index)]")
        guard sourceProofIsNonZeroTronAddress(address) else {
            throw SccpSourceProofHashError.invalidValidatorSet("witnessAddresses[\(index)]")
        }
        let addressHex = address.hexEncodedString()
        guard seenAddresses.insert(addressHex).inserted else {
            throw SccpSourceProofHashError.invalidValidatorSet("witnessAddresses[\(index)]")
        }
        guard pair.1 != 0 else {
            throw SccpSourceProofHashError.invalidValidatorSet("witnessWeights[\(index)]")
        }
        let total = totalWeight.addingReportingOverflow(pair.1)
        guard !total.overflow else {
            throw SccpSourceProofHashError.invalidValidatorSet("witnessWeights")
        }
        totalWeight = total.partialValue
        out.append(address)
        sourceProofAppendU64Le(pair.1, to: &out)
    }
    return out
}

/// Hash of the canonical TRON DPoS witness-schedule transition payload.
public func tronWitnessSchedulePayloadHash(payload: Data) throws -> String {
    try sourceProofValidateTronWitnessSchedulePayload(payload)
    return try sourceProofHashHex(prefix: "sccp:tron:witness-schedule-payload:v1", payload: payload)
}

/// Hash of the canonical TRON DPoS witness-schedule transition payload.
public func tronWitnessSchedulePayloadHash(witnessAddresses: [String],
                                           witnessWeights: [UInt64]) throws -> String {
    try tronWitnessSchedulePayloadHash(
        payload: canonicalTronWitnessSchedulePayloadBytes(
            witnessAddresses: witnessAddresses,
            witnessWeights: witnessWeights
        )
    )
}

/// SCCP TRON witness-schedule hash derived from a canonical witness-schedule payload.
public func tronWitnessScheduleHashFromPayload(payload: Data) throws -> String {
    try sourceProofValidateTronWitnessSchedulePayload(payload)
    return try sourceProofHashHex(prefix: "sccp:tron:witness-schedule:v1", payload: payload)
}

/// SCCP TRON witness-schedule hash derived from a canonical witness-schedule payload.
public func tronWitnessScheduleHashFromPayload(witnessAddresses: [String],
                                               witnessWeights: [UInt64]) throws -> String {
    try tronWitnessScheduleHashFromPayload(
        payload: canonicalTronWitnessSchedulePayloadBytes(
            witnessAddresses: witnessAddresses,
            witnessWeights: witnessWeights
        )
    )
}

/// Canonical TRON DPoS solid-block witness message bytes.
public func canonicalTronSolidBlockMessageBytes(sourceDomain: UInt32,
                                                solidBlockNumber: UInt64,
                                                blockHash: String,
                                                witnessScheduleHash: String,
                                                receiptRoot: String,
                                                transactionRoot: String,
                                                receiptProofHash: String,
                                                version: UInt8 = 1) throws -> Data {
    guard version == 1, sourceDomain == sccpDomainTron, solidBlockNumber != 0 else {
        throw SccpSourceProofHashError.invalidValidatorSet("solidBlockMessage")
    }
    let blockHashBytes = try sourceProofNonZeroBytesFromHex32(blockHash, field: "blockHash")
    let witnessScheduleHashBytes = try sourceProofNonZeroBytesFromHex32(
        witnessScheduleHash,
        field: "witnessScheduleHash"
    )
    let receiptRootBytes = try sourceProofNonZeroBytesFromHex32(receiptRoot, field: "receiptRoot")
    let transactionRootBytes = try sourceProofNonZeroBytesFromHex32(transactionRoot, field: "transactionRoot")
    let receiptProofHashBytes = try sourceProofNonZeroBytesFromHex32(receiptProofHash, field: "receiptProofHash")
    var out = Data()
    out.append(version)
    sourceProofAppendU32Le(sourceDomain, to: &out)
    sourceProofAppendU64Le(solidBlockNumber, to: &out)
    out.append(blockHashBytes)
    out.append(witnessScheduleHashBytes)
    out.append(receiptRootBytes)
    out.append(transactionRootBytes)
    out.append(receiptProofHashBytes)
    return out
}

/// Hash of canonical TRON DPoS solid-block witness message bytes.
public func tronSolidBlockMessageHash(sourceDomain: UInt32,
                                      solidBlockNumber: UInt64,
                                      blockHash: String,
                                      witnessScheduleHash: String,
                                      receiptRoot: String,
                                      transactionRoot: String,
                                      receiptProofHash: String,
                                      version: UInt8 = 1) throws -> String {
    try sourceProofKeccakHashHex(
        prefix: "sccp:tron:solid-block-message:v1",
        payload: canonicalTronSolidBlockMessageBytes(
            sourceDomain: sourceDomain,
            solidBlockNumber: solidBlockNumber,
            blockHash: blockHash,
            witnessScheduleHash: witnessScheduleHash,
            receiptRoot: receiptRoot,
            transactionRoot: transactionRoot,
            receiptProofHash: receiptProofHash,
            version: version
        )
    )
}

/// Canonical TRON DPoS witness-seal certificate bytes.
public func canonicalTronWitnessSealBytes(totalWeight: UInt64,
                                          signedWeight: UInt64,
                                          solidBlockMessageHash: String,
                                          witnessAddresses: [String],
                                          witnessWeights: [UInt64],
                                          signersBitmap: Data,
                                          signatures: [Data],
                                          version: UInt8 = 1) throws -> Data {
    let proof = try sourceProofNormalizeTronWitnessSeal(
        version: version,
        totalWeight: totalWeight,
        signedWeight: signedWeight,
        solidBlockMessageHash: solidBlockMessageHash,
        witnessAddresses: witnessAddresses,
        witnessWeights: witnessWeights,
        signersBitmap: signersBitmap,
        signatures: signatures
    )
    var out = try sourceProofCanonicalTronWitnessSealProofBytes(proof)
    out.append(proof.witnessScheduleHash)
    return out
}

/// Hash of canonical TRON DPoS witness-seal certificate bytes.
public func tronWitnessSealHash(totalWeight: UInt64,
                                signedWeight: UInt64,
                                solidBlockMessageHash: String,
                                witnessAddresses: [String],
                                witnessWeights: [UInt64],
                                signersBitmap: Data,
                                signatures: [Data],
                                version: UInt8 = 1) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:tron:witness-seal:v1",
        payload: canonicalTronWitnessSealBytes(
            totalWeight: totalWeight,
            signedWeight: signedWeight,
            solidBlockMessageHash: solidBlockMessageHash,
            witnessAddresses: witnessAddresses,
            witnessWeights: witnessWeights,
            signersBitmap: signersBitmap,
            signatures: signatures,
            version: version
        )
    )
}

/// Canonical TRON witness-schedule transition message bytes.
public func canonicalTronWitnessScheduleTransitionMessageBytes(sourceDomain: UInt32,
                                                               fromWitnessScheduleEpoch: UInt64,
                                                               toWitnessScheduleEpoch: UInt64,
                                                               transitionBlockNumber: UInt64,
                                                               transitionBlockHash: String,
                                                               parentWitnessScheduleHash: String,
                                                               nextWitnessScheduleHash: String,
                                                               nextWitnessSchedulePayloadHash: String? = nil,
                                                               nextWitnessSchedulePayload: Data? = nil,
                                                               version: UInt8 = 1) throws -> Data {
    try sourceProofCanonicalTronWitnessScheduleTransitionMessageBytes(
        sourceProofNormalizeTronWitnessScheduleTransitionMessage(
            version: version,
            sourceDomain: sourceDomain,
            fromWitnessScheduleEpoch: fromWitnessScheduleEpoch,
            toWitnessScheduleEpoch: toWitnessScheduleEpoch,
            transitionBlockNumber: transitionBlockNumber,
            transitionBlockHash: transitionBlockHash,
            parentWitnessScheduleHash: parentWitnessScheduleHash,
            nextWitnessScheduleHash: nextWitnessScheduleHash,
            nextWitnessSchedulePayloadHash: nextWitnessSchedulePayloadHash,
            nextWitnessSchedulePayload: nextWitnessSchedulePayload
        )
    )
}

/// Hash of a canonical TRON witness-schedule transition message transcript.
public func tronWitnessScheduleTransitionMessageHash(sourceDomain: UInt32,
                                                     fromWitnessScheduleEpoch: UInt64,
                                                     toWitnessScheduleEpoch: UInt64,
                                                     transitionBlockNumber: UInt64,
                                                     transitionBlockHash: String,
                                                     parentWitnessScheduleHash: String,
                                                     nextWitnessScheduleHash: String,
                                                     nextWitnessSchedulePayloadHash: String? = nil,
                                                     nextWitnessSchedulePayload: Data? = nil,
                                                     version: UInt8 = 1) throws -> String {
    try sourceProofKeccakHashHex(
        prefix: "sccp:tron:witness-schedule-transition-message:v1",
        payload: canonicalTronWitnessScheduleTransitionMessageBytes(
            sourceDomain: sourceDomain,
            fromWitnessScheduleEpoch: fromWitnessScheduleEpoch,
            toWitnessScheduleEpoch: toWitnessScheduleEpoch,
            transitionBlockNumber: transitionBlockNumber,
            transitionBlockHash: transitionBlockHash,
            parentWitnessScheduleHash: parentWitnessScheduleHash,
            nextWitnessScheduleHash: nextWitnessScheduleHash,
            nextWitnessSchedulePayloadHash: nextWitnessSchedulePayloadHash,
            nextWitnessSchedulePayload: nextWitnessSchedulePayload,
            version: version
        )
    )
}

/// Canonical TRON witness-schedule transition seal bytes.
public func canonicalTronWitnessScheduleTransitionSealBytes(sourceDomain: UInt32,
                                                            fromWitnessScheduleEpoch: UInt64,
                                                            toWitnessScheduleEpoch: UInt64,
                                                            transitionBlockNumber: UInt64,
                                                            transitionBlockHash: String,
                                                            parentWitnessScheduleHash: String,
                                                            nextWitnessScheduleHash: String,
                                                            nextWitnessSchedulePayload: Data,
                                                            transitionMessageHash: String,
                                                            totalWeight: UInt64,
                                                            signedWeight: UInt64,
                                                            witnessAddresses: [String],
                                                            witnessWeights: [UInt64],
                                                            signersBitmap: Data,
                                                            signatures: [Data],
                                                            version: UInt8 = 1) throws -> Data {
    let message = try sourceProofNormalizeTronWitnessScheduleTransitionMessage(
        version: version,
        sourceDomain: sourceDomain,
        fromWitnessScheduleEpoch: fromWitnessScheduleEpoch,
        toWitnessScheduleEpoch: toWitnessScheduleEpoch,
        transitionBlockNumber: transitionBlockNumber,
        transitionBlockHash: transitionBlockHash,
        parentWitnessScheduleHash: parentWitnessScheduleHash,
        nextWitnessScheduleHash: nextWitnessScheduleHash,
        nextWitnessSchedulePayloadHash: nil,
        nextWitnessSchedulePayload: nextWitnessSchedulePayload
    )
    let transitionMessageHashBytes = try sourceProofNonZeroBytesFromHex32(
        transitionMessageHash,
        field: "transitionMessageHash"
    )
    let expectedTransitionMessageHash = try sourceProofBytesFromHex32(
        tronWitnessScheduleTransitionMessageHash(
            sourceDomain: sourceDomain,
            fromWitnessScheduleEpoch: fromWitnessScheduleEpoch,
            toWitnessScheduleEpoch: toWitnessScheduleEpoch,
            transitionBlockNumber: transitionBlockNumber,
            transitionBlockHash: transitionBlockHash,
            parentWitnessScheduleHash: parentWitnessScheduleHash,
            nextWitnessScheduleHash: nextWitnessScheduleHash,
            nextWitnessSchedulePayloadHash: nil,
            nextWitnessSchedulePayload: nextWitnessSchedulePayload,
            version: version
        ),
        field: "transitionMessageHash"
    )
    guard transitionMessageHashBytes == expectedTransitionMessageHash else {
        throw SccpSourceProofHashError.invalidValidatorSet("transitionMessageHash")
    }
    let proof = try sourceProofNormalizeTronWitnessSeal(
        version: version,
        totalWeight: totalWeight,
        signedWeight: signedWeight,
        solidBlockMessageHash: transitionMessageHash,
        witnessAddresses: witnessAddresses,
        witnessWeights: witnessWeights,
        signersBitmap: signersBitmap,
        signatures: signatures
    )
    guard proof.witnessScheduleHash == message.parentWitnessScheduleHash else {
        throw SccpSourceProofHashError.invalidValidatorSet("parentWitnessScheduleHash")
    }
    var out = Data()
    out.append(version)
    sourceProofAppendU32Le(message.sourceDomain, to: &out)
    sourceProofAppendU64Le(message.fromWitnessScheduleEpoch, to: &out)
    sourceProofAppendU64Le(message.toWitnessScheduleEpoch, to: &out)
    sourceProofAppendU64Le(message.transitionBlockNumber, to: &out)
    out.append(message.transitionBlockHash)
    out.append(message.parentWitnessScheduleHash)
    out.append(message.nextWitnessScheduleHash)
    sourceProofAppendDataVector(nextWitnessSchedulePayload, to: &out)
    out.append(message.nextWitnessSchedulePayloadHash)
    out.append(transitionMessageHashBytes)
    out.append(proof.witnessScheduleHash)
    out.append(try sourceProofCanonicalTronWitnessSealProofBytes(proof))
    return out
}

/// Hash of canonical TRON witness-schedule transition seal bytes.
public func tronWitnessScheduleTransitionSealHash(sourceDomain: UInt32,
                                                  fromWitnessScheduleEpoch: UInt64,
                                                  toWitnessScheduleEpoch: UInt64,
                                                  transitionBlockNumber: UInt64,
                                                  transitionBlockHash: String,
                                                  parentWitnessScheduleHash: String,
                                                  nextWitnessScheduleHash: String,
                                                  nextWitnessSchedulePayload: Data,
                                                  transitionMessageHash: String,
                                                  totalWeight: UInt64,
                                                  signedWeight: UInt64,
                                                  witnessAddresses: [String],
                                                  witnessWeights: [UInt64],
                                                  signersBitmap: Data,
                                                  signatures: [Data],
                                                  version: UInt8 = 1) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:tron:witness-schedule-transition-seal:v1",
        payload: canonicalTronWitnessScheduleTransitionSealBytes(
            sourceDomain: sourceDomain,
            fromWitnessScheduleEpoch: fromWitnessScheduleEpoch,
            toWitnessScheduleEpoch: toWitnessScheduleEpoch,
            transitionBlockNumber: transitionBlockNumber,
            transitionBlockHash: transitionBlockHash,
            parentWitnessScheduleHash: parentWitnessScheduleHash,
            nextWitnessScheduleHash: nextWitnessScheduleHash,
            nextWitnessSchedulePayload: nextWitnessSchedulePayload,
            transitionMessageHash: transitionMessageHash,
            totalWeight: totalWeight,
            signedWeight: signedWeight,
            witnessAddresses: witnessAddresses,
            witnessWeights: witnessWeights,
            signersBitmap: signersBitmap,
            signatures: signatures,
            version: version
        )
    )
}

/// Canonical Substrate storage-proof transcript bytes checked by the SCCP source adapter.
public func canonicalSubstrateSccpStorageProofBytes(sourceDomain: UInt32,
                                                    sourceEventDigest: String,
                                                    sourceEventLeafIndex: UInt64,
                                                    finalizedBlockNumber: UInt64,
                                                    grandpaSetId: UInt64,
                                                    blockHash: String,
                                                    authoritySetHash: String,
                                                    eventsRoot: String,
                                                    inclusionBranch: [Data]) throws -> Data {
    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(sourceDomain, to: &out)
    try out.append(sourceProofNonZeroBytesFromHex32(sourceEventDigest, field: "sourceEventDigest"))
    out.append(sccpSubstrateSystemEventsStorageKey)
    sourceProofAppendU64Le(sourceEventLeafIndex, to: &out)
    sourceProofAppendU64Le(finalizedBlockNumber, to: &out)
    sourceProofAppendU64Le(grandpaSetId, to: &out)
    try out.append(sourceProofBytesFromHex32(blockHash, field: "blockHash"))
    try out.append(sourceProofBytesFromHex32(authoritySetHash, field: "authoritySetHash"))
    try out.append(sourceProofBytesFromHex32(eventsRoot, field: "eventsRoot"))
    try sourceProofAppendBranch(inclusionBranch, to: &out)
    return out
}

/// Hash of the canonical Substrate storage-proof transcript checked by the source adapter.
public func substrateSccpStorageProofHash(sourceDomain: UInt32,
                                          sourceEventDigest: String,
                                          sourceEventLeafIndex: UInt64,
                                          finalizedBlockNumber: UInt64,
                                          grandpaSetId: UInt64,
                                          blockHash: String,
                                          authoritySetHash: String,
                                          eventsRoot: String,
                                          inclusionBranch: [Data]) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:substrate:storage-proof:v1",
        payload: canonicalSubstrateSccpStorageProofBytes(
            sourceDomain: sourceDomain,
            sourceEventDigest: sourceEventDigest,
            sourceEventLeafIndex: sourceEventLeafIndex,
            finalizedBlockNumber: finalizedBlockNumber,
            grandpaSetId: grandpaSetId,
            blockHash: blockHash,
            authoritySetHash: authoritySetHash,
            eventsRoot: eventsRoot,
            inclusionBranch: inclusionBranch
        )
    )
}

private func isSubstrateRuntimeStorageSourceDomain(_ sourceDomain: UInt32) -> Bool {
    sourceDomain == sccpDomainSoraKusama ||
        sourceDomain == sccpDomainSoraPolkadot ||
        sourceDomain == sccpDomainSora2
}

private let sccpSourceMaterialTonTemplateSourceStateVerifierHashV1 =
    "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f"
private let sccpSourceMaterialTonTemplateComponentHashesV1: [(String, String)] = [
    ("sourceTrustAnchorHash", "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c"),
    ("consensusVerifierHash", "0xb0225e16477ea3420f7d0de76b87b6e99a43ab97f445d8565a384d4b655bc473"),
    ("messageInclusionVerifierHash", "0x89254256421c15da8c92842c7d6f448ef6c1d5ca1e2a173754643425fcee6353"),
    ("sourceStateVerifierHash", sccpSourceMaterialTonTemplateSourceStateVerifierHashV1),
    ("finalityPolicyHash", "0x50044ee6db0eb0cdef097e69406b6c30d3406d8f784e8ba34e9b923b38bd0c43"),
]
private let sccpSourceMaterialSolanaTemplateComponentHashesV1: [(String, String)] = [
    ("sourceTrustAnchorHash", "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3"),
    ("consensusVerifierHash", "0x97ea89019e6c79305d06dfc27640ee14a6b42ba6eaf86e1835ee9b433dba48ba"),
    ("messageInclusionVerifierHash", "0xb8358bfef1e428a6a7e9115687cb2b88d9c21dad4021bea3e11d43489eb3dcb0"),
    ("sourceStateVerifierHash", sccpSolanaTemplateSourceStateVerifierHashV1),
    ("finalityPolicyHash", "0x9df7ea90cf1bbba036788b14804f63f4be1e908390be89524fd4486f74344f56"),
]
private let sccpSourceMaterialTronTemplateComponentHashesV1: [(String, String)] = [
    ("sourceTrustAnchorHash", "0x3550934cbdfe49449ec4aa383dcea7674541fedf66ab6159b1ed2f2c0be4755c"),
    ("consensusVerifierHash", "0x8a1de96a869b2f28f197a7835597f17cf77ff45f7cbb77da2f7c48e87df8c5ea"),
    ("messageInclusionVerifierHash", "0xf39db56474b288680ad9561389cca7a841bd1fd223719255324705e1038fcacc"),
    ("finalityPolicyHash", "0xad5a6a4f200e070400b5aaa1b7976c639e67571eb711eb6f69d01e3615423864"),
]
private let sccpEthSourceBridgeConfigLabelV1 =
    Data("iroha:sccp:eth-source-bridge-config:v1".utf8)
private let sccpTronSourceBridgeConfigLabelV1 =
    Data("iroha:sccp:tron-source-bridge-config:v1".utf8)

private func rejectTonTemplateSourceMaterialComponent(
    _ value: Data,
    field: String,
    sourceDomain: UInt32
) throws {
    guard sourceDomain == sccpDomainTon else {
        return
    }
    for (templateField, templateHash) in sccpSourceMaterialTonTemplateComponentHashesV1
        where templateField == field {
        if let templateBytes = try? sourceProofBytesFromHex32(templateHash, field: field),
           value == templateBytes {
            throw SccpSourceProofHashError.invalidSourceMaterial(field)
        }
    }
}

private func rejectSolanaTemplateSourceMaterialComponent(
    _ value: Data,
    field: String,
    sourceDomain: UInt32
) throws {
    guard sourceDomain == sccpDomainSolana else {
        return
    }
    for (templateField, templateHash) in sccpSourceMaterialSolanaTemplateComponentHashesV1
        where templateField == field {
        if let templateBytes = try? sourceProofBytesFromHex32(templateHash, field: field),
           value == templateBytes {
            throw SccpSourceProofHashError.invalidSourceMaterial(field)
        }
    }
}

private func rejectTronTemplateSourceMaterialComponent(
    _ value: Data,
    field: String,
    sourceDomain: UInt32
) throws {
    guard sourceDomain == sccpDomainTron else {
        return
    }
    for (templateField, templateHash) in sccpSourceMaterialTronTemplateComponentHashesV1
        where templateField == field {
        if let templateBytes = try? sourceProofBytesFromHex32(templateHash, field: field),
           value == templateBytes {
            throw SccpSourceProofHashError.invalidSourceMaterial(field)
        }
    }
}

private func sourceProofAbiWordAddress20(_ value: Data) -> Data {
    var out = Data(repeating: 0, count: 32)
    out.replaceSubrange(12..<32, with: value)
    return out
}

private func tronSourceBridgeConfigHash(sourceDomain: UInt32,
                                        bridgeAddress: Data,
                                        networkId: Data,
                                        ownerAddress: Data) -> Data {
    var payload = Data(irohaKeccak256(sccpTronSourceBridgeConfigLabelV1))
    payload.append(sourceProofAbiWordAddress20(bridgeAddress))
    payload.append(networkId)
    sourceProofAppendAbiU32(sourceDomain, to: &payload)
    sourceProofAppendAbiU32(sccpDomainSora, to: &payload)
    payload.append(sourceProofAbiWordAddress20(ownerAddress))
    return Data(irohaKeccak256(payload))
}

private func ethSourceBridgeConfigHash(sourceDomain: UInt32,
                                       bridgeAddress: Data,
                                       networkId: Data,
                                       codeHash: Data) -> Data {
    var payload = Data(irohaKeccak256(sccpEthSourceBridgeConfigLabelV1))
    payload.append(sourceProofAbiWordAddress20(bridgeAddress))
    payload.append(networkId)
    sourceProofAppendAbiU32(sourceDomain, to: &payload)
    sourceProofAppendAbiU32(sccpDomainSora, to: &payload)
    payload.append(codeHash)
    return Data(irohaKeccak256(payload))
}

private func substrateTemplateSourceStateVerifierHash(sourceDomain: UInt32) -> String? {
    switch sourceDomain {
    case sccpDomainSoraKusama:
        return "0xaf2d28b3e07447239f28e90ce4fdee7e6cd3778c087eaeda7170781eb4b76b9c"
    case sccpDomainSoraPolkadot:
        return "0x664576f1a2409099c3b7dba82512c8757501f2869aedda0e45f858572b940b5d"
    case sccpDomainSora2:
        return "0x20509eb56524c727b6d028cc6b43f10c17048d31b92d5a96d41c0512d16267ef"
    default:
        return nil
    }
}

private func substrateRuntimeStorageSourceMaterial(
    sourceDomain: UInt32,
    sourceTrustAnchorHash: String,
    consensusVerifierHash: String,
    messageInclusionVerifierHash: String,
    finalityPolicyHash: String,
    sourceStateVerifierHash: String
) throws -> SccpNormalizedSourceMaterial {
    guard isSubstrateRuntimeStorageSourceDomain(sourceDomain) else {
        throw SccpSourceProofHashError.unsupportedSourceAdapterDomain("sourceDomain")
    }
    let material = try normalizeSccpSourceMaterial(
        sourceDomain: sourceDomain,
        sourceTrustAnchorHash: sourceTrustAnchorHash,
        consensusVerifierHash: consensusVerifierHash,
        messageInclusionVerifierHash: messageInclusionVerifierHash,
        finalityPolicyHash: finalityPolicyHash,
        sourceStateVerifierHash: sourceStateVerifierHash,
        bridgeAddress: nil,
        sourceBridgeEmitterCodeHash: nil,
        networkId: nil,
        ownerAddress: nil,
        configHash: nil
    )
    guard !material.profile.sourceStateVerifierId.isEmpty,
          material.sourceStateVerifierHash.contains(where: { $0 != 0 }) else {
        throw SccpSourceProofHashError.invalidSourceMaterial("sourceStateVerifierHash")
    }
    if let templateHash = substrateTemplateSourceStateVerifierHash(sourceDomain: sourceDomain),
       let templateBytes = try? sourceProofBytesFromHex32(templateHash, field: "sourceStateVerifierHash"),
       material.sourceStateVerifierHash == templateBytes {
        throw SccpSourceProofHashError.invalidSourceMaterial("sourceStateVerifierHash")
    }
    return material
}

private func sourceProofWordU32Le(_ value: UInt32) -> Data {
    var out = Data(repeating: 0, count: 32)
    var word = Data()
    sourceProofAppendU32Le(value, to: &word)
    out.replaceSubrange(0..<4, with: word)
    return out
}

private func sourceProofWordU64Le(_ value: UInt64) -> Data {
    var out = Data(repeating: 0, count: 32)
    var word = Data()
    sourceProofAppendU64Le(value, to: &word)
    out.replaceSubrange(0..<8, with: word)
    return out
}

/// Canonical OpenVerify statement bytes for a Substrate runtime-storage source-state proof.
public func canonicalSubstrateSccpRuntimeStorageVerificationStatementBytes(
    sourceDomain: UInt32,
    sourceEventDigest: String,
    sourceEventLeafIndex: UInt64,
    finalizedBlockNumber: UInt64,
    grandpaSetId: UInt64,
    blockHash: String,
    authoritySetHash: String,
    eventsRoot: String,
    storageProofHash: String? = nil,
    inclusionBranch: [Data]
) throws -> Data {
    guard isSubstrateRuntimeStorageSourceDomain(sourceDomain) else {
        throw SccpSourceProofHashError.unsupportedSourceAdapterDomain("sourceDomain")
    }
    let statement = try canonicalSubstrateSccpStorageProofBytes(
        sourceDomain: sourceDomain,
        sourceEventDigest: sourceEventDigest,
        sourceEventLeafIndex: sourceEventLeafIndex,
        finalizedBlockNumber: finalizedBlockNumber,
        grandpaSetId: grandpaSetId,
        blockHash: blockHash,
        authoritySetHash: authoritySetHash,
        eventsRoot: eventsRoot,
        inclusionBranch: inclusionBranch
    )
    if let storageProofHash {
        let actual = try sourceProofHashHex(prefix: "sccp:substrate:storage-proof:v1", payload: statement)
        let expected = "0x" + (try sourceProofBytesFromHex32(storageProofHash, field: "storageProofHash")).hexEncodedString()
        guard actual == expected else {
            throw SccpSourceProofHashError.invalidSourceMaterial("storageProofHash")
        }
    }
    return statement
}

/// Hash of Substrate runtime-storage OpenVerify public inputs.
public func substrateSccpRuntimeStorageProofPublicInputsHash(
    sourceDomain: UInt32,
    sourceEventDigest: String,
    sourceEventLeafIndex: UInt64,
    finalizedBlockNumber: UInt64,
    grandpaSetId: UInt64,
    blockHash: String,
    authoritySetHash: String,
    eventsRoot: String,
    storageProofHash: String? = nil,
    inclusionBranch: [Data]
) throws -> String {
    try sourceProofHashHex(
        prefix: sccpSubstrateRuntimeStorageProofPublicInputsPrefixV1,
        payload: canonicalSubstrateSccpRuntimeStorageVerificationStatementBytes(
            sourceDomain: sourceDomain,
            sourceEventDigest: sourceEventDigest,
            sourceEventLeafIndex: sourceEventLeafIndex,
            finalizedBlockNumber: finalizedBlockNumber,
            grandpaSetId: grandpaSetId,
            blockHash: blockHash,
            authoritySetHash: authoritySetHash,
            eventsRoot: eventsRoot,
            storageProofHash: storageProofHash,
            inclusionBranch: inclusionBranch
        )
    )
}

/// OpenVerify context bytes binding Substrate runtime-storage proofs to governed verifier material.
public func canonicalSubstrateSccpRuntimeStorageVerificationContextBytes(
    sourceDomain: UInt32,
    sourceEventDigest: String,
    sourceEventLeafIndex: UInt64,
    finalizedBlockNumber: UInt64,
    grandpaSetId: UInt64,
    blockHash: String,
    authoritySetHash: String,
    eventsRoot: String,
    sourceTrustAnchorHash: String,
    consensusVerifierHash: String,
    messageInclusionVerifierHash: String,
    finalityPolicyHash: String,
    sourceStateVerifierHash: String,
    storageProofHash: String? = nil,
    inclusionBranch: [Data]
) throws -> Data {
    let material = try substrateRuntimeStorageSourceMaterial(
        sourceDomain: sourceDomain,
        sourceTrustAnchorHash: sourceTrustAnchorHash,
        consensusVerifierHash: consensusVerifierHash,
        messageInclusionVerifierHash: messageInclusionVerifierHash,
        finalityPolicyHash: finalityPolicyHash,
        sourceStateVerifierHash: sourceStateVerifierHash
    )
    var out = Data()
    out.append(1)
    sourceProofAppendDataVector(Data(sccpSubstrateRuntimeStorageOpenVerifyCircuitIdV1.utf8), to: &out)
    sourceProofAppendDataVector(Data(sccpSubstrateRuntimeStorageFastpqParameterSetV1.utf8), to: &out)
    sourceProofAppendDataVector(Data(material.profile.sourceStateVerifierId.utf8), to: &out)
    out.append(material.sourceStateVerifierHash)
    sourceProofAppendDataVector(Data(material.profile.sourceTrustAnchorId.utf8), to: &out)
    out.append(material.sourceTrustAnchorHash)
    sourceProofAppendDataVector(Data(material.profile.consensusVerifierId.utf8), to: &out)
    out.append(material.consensusVerifierHash)
    sourceProofAppendDataVector(Data(material.profile.messageInclusionVerifierId.utf8), to: &out)
    out.append(material.messageInclusionVerifierHash)
    sourceProofAppendDataVector(Data(material.profile.finalityPolicyId.utf8), to: &out)
    out.append(material.finalityPolicyHash)
    try out.append(sourceProofBytesFromHex32(
        substrateSccpRuntimeStorageProofPublicInputsHash(
            sourceDomain: sourceDomain,
            sourceEventDigest: sourceEventDigest,
            sourceEventLeafIndex: sourceEventLeafIndex,
            finalizedBlockNumber: finalizedBlockNumber,
            grandpaSetId: grandpaSetId,
            blockHash: blockHash,
            authoritySetHash: authoritySetHash,
            eventsRoot: eventsRoot,
            storageProofHash: storageProofHash,
            inclusionBranch: inclusionBranch
        ),
        field: "runtimeStorageProofPublicInputsHash"
    ))
    return out
}

/// OpenVerify public-input columns for Substrate runtime-storage proofs.
public func substrateSccpRuntimeStoragePublicInputColumns(
    sourceDomain: UInt32,
    sourceEventDigest: String,
    sourceEventLeafIndex: UInt64,
    finalizedBlockNumber: UInt64,
    grandpaSetId: UInt64,
    blockHash: String,
    authoritySetHash: String,
    eventsRoot: String,
    storageProofHash: String? = nil,
    inclusionBranch: [Data]
) throws -> [[String]] {
    let computedStorageProofHash = try substrateSccpStorageProofHash(
        sourceDomain: sourceDomain,
        sourceEventDigest: sourceEventDigest,
        sourceEventLeafIndex: sourceEventLeafIndex,
        finalizedBlockNumber: finalizedBlockNumber,
        grandpaSetId: grandpaSetId,
        blockHash: blockHash,
        authoritySetHash: authoritySetHash,
        eventsRoot: eventsRoot,
        inclusionBranch: inclusionBranch
    )
    if let storageProofHash {
        let supplied = "0x" + (try sourceProofBytesFromHex32(storageProofHash, field: "storageProofHash")).hexEncodedString()
        guard supplied == computedStorageProofHash else {
            throw SccpSourceProofHashError.invalidSourceMaterial("storageProofHash")
        }
    }
    return [
        ["0x" + sourceProofWordU32Le(sourceDomain).hexEncodedString()],
        ["0x" + sourceProofWordU64Le(finalizedBlockNumber).hexEncodedString()],
        ["0x" + sourceProofWordU64Le(grandpaSetId).hexEncodedString()],
        ["0x" + (try sourceProofBytesFromHex32(blockHash, field: "blockHash")).hexEncodedString()],
        ["0x" + (try sourceProofBytesFromHex32(authoritySetHash, field: "authoritySetHash")).hexEncodedString()],
        ["0x" + (try sourceProofBytesFromHex32(eventsRoot, field: "eventsRoot")).hexEncodedString()],
        [computedStorageProofHash],
        ["0x" + (try sourceProofBytesFromHex32(sourceEventDigest, field: "sourceEventDigest")).hexEncodedString()],
        ["0x" + sccpSubstrateSystemEventsStorageKey.hexEncodedString()],
        ["0x" + sourceProofWordU64Le(sourceEventLeafIndex).hexEncodedString()],
        [try substrateSccpRuntimeStorageProofPublicInputsHash(
            sourceDomain: sourceDomain,
            sourceEventDigest: sourceEventDigest,
            sourceEventLeafIndex: sourceEventLeafIndex,
            finalizedBlockNumber: finalizedBlockNumber,
            grandpaSetId: grandpaSetId,
            blockHash: blockHash,
            authoritySetHash: authoritySetHash,
            eventsRoot: eventsRoot,
            storageProofHash: computedStorageProofHash,
            inclusionBranch: inclusionBranch
        )]
    ]
}

/// OpenVerify schema descriptor for Substrate runtime-storage proofs.
public func substrateSccpRuntimeStorageOpenVerifySchemaDescriptor(sourceDomain: UInt32) throws -> Data {
    guard isSubstrateRuntimeStorageSourceDomain(sourceDomain) else {
        throw SccpSourceProofHashError.unsupportedSourceAdapterDomain("sourceDomain")
    }
    let adapterProfile = try sccpSourceAdapterVerifierProfile(sourceDomain: sourceDomain)
    var out = Data()
    out.append(1)
    sourceProofAppendDataVector(Data(sccpSubstrateRuntimeStorageOpenVerifyCircuitIdV1.utf8), to: &out)
    sourceProofAppendDataVector(Data(sccpSubstrateRuntimeStorageFastpqParameterSetV1.utf8), to: &out)
    sourceProofAppendDataVector(Data(adapterProfile.chain.utf8), to: &out)
    sourceProofAppendU32Le(sourceDomain, to: &out)
    for requiredInput in [
        "source_domain",
        "finalized_block_number",
        "grandpa_set_id",
        "block_hash",
        "authority_set_hash",
        "events_root",
        "storage_proof_hash",
        "source_event_digest",
        "system_events_storage_key",
        "source_event_leaf_index",
        "runtime_storage_proof_public_inputs_hash"
    ] {
        sourceProofAppendDataVector(Data(requiredInput.utf8), to: &out)
    }
    return out
}

/// Deterministic Substrate runtime-storage proof request for mobile prover engines.
public func buildSubstrateSccpRuntimeStorageProofRequest(
    sourceDomain: UInt32,
    sourceEventDigest: String,
    sourceEventLeafIndex: UInt64,
    finalizedBlockNumber: UInt64,
    grandpaSetId: UInt64,
    blockHash: String,
    authoritySetHash: String,
    eventsRoot: String,
    sourceTrustAnchorHash: String,
    consensusVerifierHash: String,
    messageInclusionVerifierHash: String,
    finalityPolicyHash: String,
    sourceStateVerifierHash: String,
    storageProofHash: String? = nil,
    inclusionBranch: [Data]
) throws -> SubstrateSccpRuntimeStorageProofRequest {
    let material = try substrateRuntimeStorageSourceMaterial(
        sourceDomain: sourceDomain,
        sourceTrustAnchorHash: sourceTrustAnchorHash,
        consensusVerifierHash: consensusVerifierHash,
        messageInclusionVerifierHash: messageInclusionVerifierHash,
        finalityPolicyHash: finalityPolicyHash,
        sourceStateVerifierHash: sourceStateVerifierHash
    )
    let statement = try canonicalSubstrateSccpRuntimeStorageVerificationStatementBytes(
        sourceDomain: sourceDomain,
        sourceEventDigest: sourceEventDigest,
        sourceEventLeafIndex: sourceEventLeafIndex,
        finalizedBlockNumber: finalizedBlockNumber,
        grandpaSetId: grandpaSetId,
        blockHash: blockHash,
        authoritySetHash: authoritySetHash,
        eventsRoot: eventsRoot,
        storageProofHash: storageProofHash,
        inclusionBranch: inclusionBranch
    )
    let computedStorageProofHash = try sourceProofHashHex(prefix: "sccp:substrate:storage-proof:v1", payload: statement)
    let publicInputsHash = try substrateSccpRuntimeStorageProofPublicInputsHash(
        sourceDomain: sourceDomain,
        sourceEventDigest: sourceEventDigest,
        sourceEventLeafIndex: sourceEventLeafIndex,
        finalizedBlockNumber: finalizedBlockNumber,
        grandpaSetId: grandpaSetId,
        blockHash: blockHash,
        authoritySetHash: authoritySetHash,
        eventsRoot: eventsRoot,
        storageProofHash: computedStorageProofHash,
        inclusionBranch: inclusionBranch
    )
    let context = try canonicalSubstrateSccpRuntimeStorageVerificationContextBytes(
        sourceDomain: sourceDomain,
        sourceEventDigest: sourceEventDigest,
        sourceEventLeafIndex: sourceEventLeafIndex,
        finalizedBlockNumber: finalizedBlockNumber,
        grandpaSetId: grandpaSetId,
        blockHash: blockHash,
        authoritySetHash: authoritySetHash,
        eventsRoot: eventsRoot,
        sourceTrustAnchorHash: sourceTrustAnchorHash,
        consensusVerifierHash: consensusVerifierHash,
        messageInclusionVerifierHash: messageInclusionVerifierHash,
        finalityPolicyHash: finalityPolicyHash,
        sourceStateVerifierHash: sourceStateVerifierHash,
        storageProofHash: computedStorageProofHash,
        inclusionBranch: inclusionBranch
    )
    let publicInputsHashBytes = try sourceProofBytesFromHex32(publicInputsHash, field: "runtimeStorageProofPublicInputsHash")
    var dsidPreimage = Data(sccpSubstrateRuntimeStorageFastpqDsidPrefixV1.utf8)
    dsidPreimage.append(publicInputsHashBytes)
    let dsid = Data(Blake2b.hash256(dsidPreimage).prefix(16))
    let transitions = [
        SubstrateSccpRuntimeStorageFastpqTransition(
            key: sccpSubstrateRuntimeStorageFastpqStatementKeyV1,
            operation: "meta_set",
            oldValue: "0x",
            newValue: "0x" + statement.hexEncodedString()
        ),
        SubstrateSccpRuntimeStorageFastpqTransition(
            key: sccpSubstrateRuntimeStorageFastpqContextKeyV1,
            operation: "meta_set",
            oldValue: "0x",
            newValue: "0x" + context.hexEncodedString()
        ),
        SubstrateSccpRuntimeStorageFastpqTransition(
            key: sccpSubstrateRuntimeStorageFastpqStorageKeyV1,
            operation: "meta_set",
            oldValue: "0x",
            newValue: "0x" + sccpSubstrateSystemEventsStorageKey.hexEncodedString()
        )
    ].sorted { $0.key < $1.key }
    return SubstrateSccpRuntimeStorageProofRequest(
        version: 1,
        proofFamily: sccpStarkFriProofFamilyV1,
        circuitId: sccpSubstrateRuntimeStorageOpenVerifyCircuitIdV1,
        parameterSet: sccpSubstrateRuntimeStorageFastpqParameterSetV1,
        sourceDomain: sourceDomain,
        finalizedBlockNumber: String(finalizedBlockNumber),
        grandpaSetId: String(grandpaSetId),
        sourceStateVerifierId: material.profile.sourceStateVerifierId,
        sourceStateVerifierHash: "0x" + material.sourceStateVerifierHash.hexEncodedString(),
        runtimeStorageProofPublicInputsHash: publicInputsHash,
        storageProofHash: computedStorageProofHash,
        statementBytes: statement,
        verificationContextBytes: context,
        schemaDescriptor: try substrateSccpRuntimeStorageOpenVerifySchemaDescriptor(sourceDomain: sourceDomain),
        publicInputColumns: try substrateSccpRuntimeStoragePublicInputColumns(
            sourceDomain: sourceDomain,
            sourceEventDigest: sourceEventDigest,
            sourceEventLeafIndex: sourceEventLeafIndex,
            finalizedBlockNumber: finalizedBlockNumber,
            grandpaSetId: grandpaSetId,
            blockHash: blockHash,
            authoritySetHash: authoritySetHash,
            eventsRoot: eventsRoot,
            storageProofHash: computedStorageProofHash,
            inclusionBranch: inclusionBranch
        ),
        fastpqPublicInputs: SubstrateSccpRuntimeStorageFastpqPublicInputs(
            dsid: "0x" + dsid.hexEncodedString(),
            slot: String(finalizedBlockNumber),
            oldRoot: "0x" + (try sourceProofBytesFromHex32(authoritySetHash, field: "authoritySetHash")).hexEncodedString(),
            newRoot: "0x" + (try sourceProofBytesFromHex32(blockHash, field: "blockHash")).hexEncodedString(),
            permRoot: "0x" + (try sourceProofBytesFromHex32(eventsRoot, field: "eventsRoot")).hexEncodedString(),
            txSetHash: publicInputsHash
        ),
        fastpqTransitions: transitions
    )
}

/// Canonical Substrate GRANDPA authority-set payload bytes checked by transition proofs.
public func canonicalSubstrateAuthoritySetPayloadBytes(authorityPublicKeys: [String],
                                                       authorityWeights: [UInt64]) throws -> Data {
    guard !authorityPublicKeys.isEmpty, authorityPublicKeys.count == authorityWeights.count else {
        throw SccpSourceProofHashError.invalidValidatorSet("authorityPublicKeys")
    }
    guard authorityPublicKeys.count <= sccpSubstrateMaxAuthorities else {
        throw SccpSourceProofHashError.invalidValidatorSet("authorityPublicKeys")
    }
    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(UInt32(authorityPublicKeys.count), to: &out)
    var seenPublicKeys = Set<String>()
    for (index, pair) in zip(authorityPublicKeys, authorityWeights).enumerated() {
        let publicKey = try sourceProofBytesFromHex32(pair.0, field: "authorityPublicKeys[\(index)]")
        guard publicKey.contains(where: { $0 != 0 }) else {
            throw SccpSourceProofHashError.invalidValidatorSet("authorityPublicKeys[\(index)]")
        }
        let publicKeyHex = publicKey.hexEncodedString()
        guard seenPublicKeys.insert(publicKeyHex).inserted else {
            throw SccpSourceProofHashError.invalidValidatorSet("authorityPublicKeys[\(index)]")
        }
        guard pair.1 != 0 else {
            throw SccpSourceProofHashError.invalidValidatorSet("authorityWeights[\(index)]")
        }
        out.append(publicKey)
        sourceProofAppendU64Le(pair.1, to: &out)
    }
    return out
}

/// Hash of the canonical Substrate GRANDPA authority-set transition payload.
public func substrateAuthoritySetPayloadHash(payload: Data) throws -> String {
    try sourceProofValidateSubstrateAuthoritySetPayload(payload)
    return try sourceProofHashHex(prefix: "sccp:substrate:authority-set-payload:v1", payload: payload)
}

/// Hash of the canonical Substrate GRANDPA authority-set transition payload.
public func substrateAuthoritySetPayloadHash(authorityPublicKeys: [String],
                                             authorityWeights: [UInt64]) throws -> String {
    try substrateAuthoritySetPayloadHash(
        payload: canonicalSubstrateAuthoritySetPayloadBytes(
            authorityPublicKeys: authorityPublicKeys,
            authorityWeights: authorityWeights
        )
    )
}

/// SCCP Substrate authority-set hash derived from a canonical authority-set payload.
public func substrateAuthoritySetHashFromPayload(payload: Data) throws -> String {
    try sourceProofValidateSubstrateAuthoritySetPayload(payload)
    return try sourceProofHashHex(prefix: "sccp:substrate:authority-set:v1", payload: payload)
}

/// SCCP Substrate authority-set hash derived from a canonical authority-set payload.
public func substrateAuthoritySetHashFromPayload(authorityPublicKeys: [String],
                                                 authorityWeights: [UInt64]) throws -> String {
    try substrateAuthoritySetHashFromPayload(
        payload: canonicalSubstrateAuthoritySetPayloadBytes(
            authorityPublicKeys: authorityPublicKeys,
            authorityWeights: authorityWeights
        )
    )
}

/// Canonical Substrate authority-set transition message bytes.
public func canonicalSubstrateAuthoritySetTransitionMessageBytes(sourceDomain: UInt32,
                                                                 fromGrandpaSetId: UInt64,
                                                                 toGrandpaSetId: UInt64,
                                                                 transitionBlockNumber: UInt64,
                                                                 transitionBlockHash: String,
                                                                 parentAuthoritySetHash: String,
                                                                 nextAuthoritySetHash: String,
                                                                 nextAuthoritySetPayloadHash: String) throws -> Data {
    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(sourceDomain, to: &out)
    sourceProofAppendU64Le(fromGrandpaSetId, to: &out)
    sourceProofAppendU64Le(toGrandpaSetId, to: &out)
    sourceProofAppendU64Le(transitionBlockNumber, to: &out)
    try out.append(sourceProofBytesFromHex32(transitionBlockHash, field: "transitionBlockHash"))
    try out.append(sourceProofBytesFromHex32(parentAuthoritySetHash, field: "parentAuthoritySetHash"))
    try out.append(sourceProofBytesFromHex32(nextAuthoritySetHash, field: "nextAuthoritySetHash"))
    try out.append(sourceProofBytesFromHex32(nextAuthoritySetPayloadHash, field: "nextAuthoritySetPayloadHash"))
    return out
}

/// Hash of the canonical Substrate authority-set transition message transcript.
public func substrateAuthoritySetTransitionMessageHash(sourceDomain: UInt32,
                                                       fromGrandpaSetId: UInt64,
                                                       toGrandpaSetId: UInt64,
                                                       transitionBlockNumber: UInt64,
                                                       transitionBlockHash: String,
                                                       parentAuthoritySetHash: String,
                                                       nextAuthoritySetHash: String,
                                                       nextAuthoritySetPayloadHash: String) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:substrate:authority-set-transition-message:v1",
        payload: canonicalSubstrateAuthoritySetTransitionMessageBytes(
            sourceDomain: sourceDomain,
            fromGrandpaSetId: fromGrandpaSetId,
            toGrandpaSetId: toGrandpaSetId,
            transitionBlockNumber: transitionBlockNumber,
            transitionBlockHash: transitionBlockHash,
            parentAuthoritySetHash: parentAuthoritySetHash,
            nextAuthoritySetHash: nextAuthoritySetHash,
            nextAuthoritySetPayloadHash: nextAuthoritySetPayloadHash
        )
    )
}

/// Canonical Substrate GRANDPA justification proof bytes.
public func canonicalSubstrateGrandpaJustificationProofBytes(version: UInt8 = 1,
                                                             totalWeight: UInt64,
                                                             signedWeight: UInt64,
                                                             precommitMessageHash: String,
                                                             authorityPublicKeys: [String],
                                                             authorityWeights: [UInt64],
                                                             signersBitmap: Data,
                                                             signatures: [Data]) throws -> Data {
    guard version == 1 else {
        throw SccpSourceProofHashError.invalidValidatorSet("Substrate GRANDPA justification version")
    }
    guard !authorityPublicKeys.isEmpty, authorityPublicKeys.count == authorityWeights.count else {
        throw SccpSourceProofHashError.invalidValidatorSet("authorityPublicKeys")
    }
    guard authorityPublicKeys.count <= sccpSubstrateMaxAuthorities else {
        throw SccpSourceProofHashError.invalidValidatorSet("authorityPublicKeys")
    }
    guard signatures.count <= sccpSubstrateMaxAuthorities else {
        throw SccpSourceProofHashError.invalidValidatorSet("signatures")
    }
    let precommitMessageHashBytes = try sourceProofBytesFromHex32(precommitMessageHash, field: "precommitMessageHash")
    var authorityPublicKeyBytes: [Data] = []
    var seenPublicKeys = Set<String>()
    for (index, publicKey) in authorityPublicKeys.enumerated() {
        let publicKeyBytes = try sourceProofBytesFromHex32(publicKey, field: "authorityPublicKeys[\(index)]")
        guard publicKeyBytes.contains(where: { $0 != 0 }) else {
            throw SccpSourceProofHashError.invalidValidatorSet("authorityPublicKeys[\(index)]")
        }
        guard seenPublicKeys.insert(publicKeyBytes.hexEncodedString()).inserted else {
            throw SccpSourceProofHashError.invalidValidatorSet("authorityPublicKeys[\(index)]")
        }
        authorityPublicKeyBytes.append(publicKeyBytes)
    }
    var normalizedWeights: [UInt64] = []
    var computedTotalWeight: UInt64 = 0
    for (index, weight) in authorityWeights.enumerated() {
        guard weight != 0 else {
            throw SccpSourceProofHashError.invalidValidatorSet("authorityWeights[\(index)]")
        }
        let addition = computedTotalWeight.addingReportingOverflow(weight)
        guard !addition.overflow else {
            throw SccpSourceProofHashError.invalidValidatorSet("totalWeight")
        }
        computedTotalWeight = addition.partialValue
        normalizedWeights.append(weight)
    }
    guard totalWeight == computedTotalWeight else {
        throw SccpSourceProofHashError.invalidValidatorSet("totalWeight")
    }
    guard signersBitmap.count == (authorityPublicKeyBytes.count + 7) / 8 else {
        throw SccpSourceProofHashError.invalidValidatorSet("signersBitmap")
    }
    var signerIndices: [Int] = []
    for (byteIndex, value) in signersBitmap.enumerated() {
        for bit in 0..<8 where ((Int(value) >> bit) & 1) == 1 {
            let signerIndex = byteIndex * 8 + bit
            guard signerIndex < authorityPublicKeyBytes.count else {
                throw SccpSourceProofHashError.invalidValidatorSet("signersBitmap")
            }
            signerIndices.append(signerIndex)
        }
    }
    guard !signerIndices.isEmpty else {
        throw SccpSourceProofHashError.invalidValidatorSet("signersBitmap")
    }
    guard signatures.count == signerIndices.count else {
        throw SccpSourceProofHashError.invalidValidatorSet("signatures")
    }
    var computedSignedWeight: UInt64 = 0
    for signerIndex in signerIndices {
        let addition = computedSignedWeight.addingReportingOverflow(normalizedWeights[signerIndex])
        guard !addition.overflow else {
            throw SccpSourceProofHashError.invalidValidatorSet("signedWeight")
        }
        computedSignedWeight = addition.partialValue
    }
    guard signedWeight == computedSignedWeight else {
        throw SccpSourceProofHashError.invalidValidatorSet("signedWeight")
    }
    let floorTwoThirds = (totalWeight / 3) * 2 + ((totalWeight % 3) * 2) / 3
    guard signedWeight > floorTwoThirds else {
        throw SccpSourceProofHashError.invalidValidatorSet("signedWeight")
    }
    for (index, signature) in signatures.enumerated() {
        guard signature.count == 64 else {
            throw SccpSourceProofHashError.invalidValidatorSet("signatures[\(index)]")
        }
        guard signature.contains(where: { $0 != 0 }) else {
            throw SccpSourceProofHashError.invalidValidatorSet("signatures[\(index)]")
        }
    }
    var out = Data()
    out.append(version)
    sourceProofAppendU64Le(totalWeight, to: &out)
    sourceProofAppendU64Le(signedWeight, to: &out)
    out.append(precommitMessageHashBytes)
    sourceProofAppendU32Le(UInt32(authorityPublicKeyBytes.count), to: &out)
    for publicKeyBytes in authorityPublicKeyBytes {
        sourceProofAppendDataVector(publicKeyBytes, to: &out)
    }
    sourceProofAppendU32Le(UInt32(normalizedWeights.count), to: &out)
    for weight in normalizedWeights {
        sourceProofAppendU64Le(weight, to: &out)
    }
    sourceProofAppendDataVector(signersBitmap, to: &out)
    sourceProofAppendU32Le(UInt32(signatures.count), to: &out)
    for signature in signatures {
        sourceProofAppendDataVector(signature, to: &out)
    }
    return out
}

/// Canonical Substrate authority-set transition justification bytes.
public func canonicalSubstrateAuthoritySetTransitionJustificationBytes(version: UInt8 = 1,
                                                                       sourceDomain: UInt32,
                                                                       fromGrandpaSetId: UInt64,
                                                                       toGrandpaSetId: UInt64,
                                                                       transitionBlockNumber: UInt64,
                                                                       transitionBlockHash: String,
                                                                       parentAuthoritySetHash: String,
                                                                       nextAuthoritySetHash: String,
                                                                       nextAuthoritySetPayload: Data,
                                                                       nextAuthoritySetPayloadHash: String,
                                                                       transitionMessageHash: String,
                                                                       proofVersion: UInt8 = 1,
                                                                       totalWeight: UInt64,
                                                                       signedWeight: UInt64,
                                                                       authorityPublicKeys: [String],
                                                                       authorityWeights: [UInt64],
                                                                       signersBitmap: Data,
                                                                       signatures: [Data]) throws -> Data {
    guard version == 1 else {
        throw SccpSourceProofHashError.invalidValidatorSet("Substrate authority-set transition justification version")
    }
    let derivedPayloadHash = try substrateAuthoritySetPayloadHash(payload: nextAuthoritySetPayload)
    guard derivedPayloadHash.lowercased() == nextAuthoritySetPayloadHash.lowercased() else {
        throw SccpSourceProofHashError.invalidValidatorSet("nextAuthoritySetPayloadHash")
    }
    let derivedNextHash = try substrateAuthoritySetHashFromPayload(payload: nextAuthoritySetPayload)
    guard derivedNextHash.lowercased() == nextAuthoritySetHash.lowercased() else {
        throw SccpSourceProofHashError.invalidValidatorSet("nextAuthoritySetHash")
    }
    let derivedParentHash = try substrateAuthoritySetHashFromPayload(
        authorityPublicKeys: authorityPublicKeys,
        authorityWeights: authorityWeights
    )
    guard derivedParentHash.lowercased() == parentAuthoritySetHash.lowercased() else {
        throw SccpSourceProofHashError.invalidValidatorSet("parentAuthoritySetHash")
    }

    var out = Data()
    out.append(version)
    sourceProofAppendU32Le(sourceDomain, to: &out)
    sourceProofAppendU64Le(fromGrandpaSetId, to: &out)
    sourceProofAppendU64Le(toGrandpaSetId, to: &out)
    sourceProofAppendU64Le(transitionBlockNumber, to: &out)
    try out.append(sourceProofBytesFromHex32(transitionBlockHash, field: "transitionBlockHash"))
    try out.append(sourceProofBytesFromHex32(parentAuthoritySetHash, field: "parentAuthoritySetHash"))
    try out.append(sourceProofBytesFromHex32(nextAuthoritySetHash, field: "nextAuthoritySetHash"))
    sourceProofAppendDataVector(nextAuthoritySetPayload, to: &out)
    try out.append(sourceProofBytesFromHex32(nextAuthoritySetPayloadHash, field: "nextAuthoritySetPayloadHash"))
    try out.append(sourceProofBytesFromHex32(transitionMessageHash, field: "transitionMessageHash"))
    try out.append(sourceProofBytesFromHex32(derivedParentHash, field: "parentAuthoritySetHash"))
    try out.append(
        canonicalSubstrateGrandpaJustificationProofBytes(
            version: proofVersion,
            totalWeight: totalWeight,
            signedWeight: signedWeight,
            precommitMessageHash: transitionMessageHash,
            authorityPublicKeys: authorityPublicKeys,
            authorityWeights: authorityWeights,
            signersBitmap: signersBitmap,
            signatures: signatures
        )
    )
    return out
}

/// Hash of the canonical Substrate authority-set transition justification transcript.
public func substrateAuthoritySetTransitionJustificationHash(version: UInt8 = 1,
                                                             sourceDomain: UInt32,
                                                             fromGrandpaSetId: UInt64,
                                                             toGrandpaSetId: UInt64,
                                                             transitionBlockNumber: UInt64,
                                                             transitionBlockHash: String,
                                                             parentAuthoritySetHash: String,
                                                             nextAuthoritySetHash: String,
                                                             nextAuthoritySetPayload: Data,
                                                             nextAuthoritySetPayloadHash: String,
                                                             transitionMessageHash: String,
                                                             proofVersion: UInt8 = 1,
                                                             totalWeight: UInt64,
                                                             signedWeight: UInt64,
                                                             authorityPublicKeys: [String],
                                                             authorityWeights: [UInt64],
                                                             signersBitmap: Data,
                                                             signatures: [Data]) throws -> String {
    try sourceProofHashHex(
        prefix: "sccp:substrate:authority-set-transition-justification:v1",
        payload: canonicalSubstrateAuthoritySetTransitionJustificationBytes(
            version: version,
            sourceDomain: sourceDomain,
            fromGrandpaSetId: fromGrandpaSetId,
            toGrandpaSetId: toGrandpaSetId,
            transitionBlockNumber: transitionBlockNumber,
            transitionBlockHash: transitionBlockHash,
            parentAuthoritySetHash: parentAuthoritySetHash,
            nextAuthoritySetHash: nextAuthoritySetHash,
            nextAuthoritySetPayload: nextAuthoritySetPayload,
            nextAuthoritySetPayloadHash: nextAuthoritySetPayloadHash,
            transitionMessageHash: transitionMessageHash,
            proofVersion: proofVersion,
            totalWeight: totalWeight,
            signedWeight: signedWeight,
            authorityPublicKeys: authorityPublicKeys,
            authorityWeights: authorityWeights,
            signersBitmap: signersBitmap,
            signatures: signatures
        )
    )
}

private func sourceProofAppendBranch(_ branch: [Data], to out: inout Data, requireNonEmpty: Bool = false) throws {
    if requireNonEmpty && branch.isEmpty {
        throw SccpSourceProofHashError.invalidBranch("inclusionBranch")
    }
    sourceProofAppendU32Le(UInt32(branch.count), to: &out)
    for (index, sibling) in branch.enumerated() {
        guard sibling.count == 32 else {
            throw SccpSourceProofHashError.invalidBranch("inclusionBranch[\(index)]")
        }
        out.append(sibling)
    }
}

private func sccpSourceAdapterVerifierProfile(
    sourceDomain: UInt32
) throws -> (chain: String, proofPlan: UInt8, finalityModel: UInt8) {
    switch sourceDomain {
    case sccpDomainEthereum:
        return ("eth", 1, 1)
    case sccpDomainBsc:
        return ("bsc", 2, 2)
    case sccpDomainSolana:
        return ("sol", 3, 3)
    case sccpDomainTon:
        return ("ton", 4, 4)
    case sccpDomainTron:
        return ("tron", 5, 5)
    case sccpDomainSoraKusama:
        return ("sora-kusama", 6, 6)
    case sccpDomainSoraPolkadot:
        return ("sora-polkadot", 6, 6)
    case sccpDomainSora2:
        return ("sora2", 6, 6)
    default:
        throw SccpSourceProofHashError.unsupportedSourceAdapterDomain("sourceDomain")
    }
}

private func sccpDestinationBindingProfile(
    targetDomain: UInt32
) throws -> (
    verifierTarget: UInt8,
    backendFamily: UInt8,
    bindingKey: String,
    manifestSeed: String,
    verifierBackend: String
) {
    switch targetDomain {
    case sccpDomainSolana:
        return (
            2,
            2,
            "sccp:0:3:sol:solana-program-v1:2",
            "iroha:sccp:bridge-proof:message:stark-fri:v1:sol",
            "solana-program-v1"
        )
    case sccpDomainTon:
        return (
            3,
            3,
            "sccp:0:4:ton:ton-contract-v1:3",
            "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
            "ton-contract-v1"
        )
    case sccpDomainSoraKusama:
        return (
            5,
            5,
            "sccp:0:6:sora-kusama:substrate-runtime-v1:5",
            "iroha:sccp:bridge-proof:message:stark-fri:v1:sora-kusama",
            "substrate-runtime-v1"
        )
    case sccpDomainSoraPolkadot:
        return (
            5,
            5,
            "sccp:0:7:sora-polkadot:substrate-runtime-v1:5",
            "iroha:sccp:bridge-proof:message:stark-fri:v1:sora-polkadot",
            "substrate-runtime-v1"
        )
    case sccpDomainSora2:
        return (
            5,
            5,
            "sccp:0:8:sora2:substrate-runtime-v1:5",
            "iroha:sccp:bridge-proof:message:stark-fri:v1:sora2",
            "substrate-runtime-v1"
        )
    default:
        throw SccpSourceProofHashError.unsupportedDestinationBindingDomain("targetDomain")
    }
}

private struct SccpSourceRecordProfile {
    let chain: String
    let proofPlan: UInt8
    let finalityModel: UInt8
    let sourceTrustAnchorId: String
    let consensusVerifierId: String
    let messageInclusionVerifierId: String
    let finalityPolicyId: String
    let sourceStateVerifierId: String
    let sourceBridgeEmitterId: String
    let requiresSourceBridge: Bool
    let requiresSourceBridgeConfig: Bool
}

private struct SccpNormalizedSourceMaterial {
    let sourceDomain: UInt32
    let profile: SccpSourceRecordProfile
    let sourceTrustAnchorHash: Data
    let consensusVerifierHash: Data
    let messageInclusionVerifierHash: Data
    let finalityPolicyHash: Data
    let sourceStateVerifierHash: Data
    let sourceBridgeEmitterAddress: Data
    let sourceBridgeEmitterCodeHash: Data
    let sourceBridgeNetworkId: Data
    let sourceBridgeOwnerAddress: Data
    let sourceBridgeConfigHash: Data
}

private func sccpSourceRecordProfile(sourceDomain: UInt32) throws -> SccpSourceRecordProfile {
    let adapterProfile = try sccpSourceAdapterVerifierProfile(sourceDomain: sourceDomain)
    switch sourceDomain {
    case sccpDomainEthereum:
        return SccpSourceRecordProfile(
            chain: adapterProfile.chain,
            proofPlan: adapterProfile.proofPlan,
            finalityModel: adapterProfile.finalityModel,
            sourceTrustAnchorId: "sccp:eth:source-trust-anchor:ethereum-mainnet-beacon-finalized-checkpoint:v1",
            consensusVerifierId: "sccp:eth:consensus-verifier:beacon-sync-committee-execution-header-mainnet:v1",
            messageInclusionVerifierId: "sccp:eth:message-inclusion-verifier:execution-receipt-trie-branch-mainnet:v1",
            finalityPolicyId: "sccp:eth:finality-policy:beacon-finalized-checkpoint-mainnet:v1",
            sourceStateVerifierId: "",
            sourceBridgeEmitterId: "sccp:eth:source-bridge-emitter:ethereum-mainnet:v1",
            requiresSourceBridge: true,
            requiresSourceBridgeConfig: true
        )
    case sccpDomainBsc:
        return SccpSourceRecordProfile(
            chain: adapterProfile.chain,
            proofPlan: adapterProfile.proofPlan,
            finalityModel: adapterProfile.finalityModel,
            sourceTrustAnchorId: "sccp:bsc:source-trust-anchor:bsc-mainnet-validator-set:v1",
            consensusVerifierId: "sccp:bsc:consensus-verifier:validator-set-seal-mainnet:v1",
            messageInclusionVerifierId: "sccp:bsc:message-inclusion-verifier:receipt-trie-branch-mainnet:v1",
            finalityPolicyId: "sccp:bsc:finality-policy:validator-set-finality-mainnet:v1",
            sourceStateVerifierId: "",
            sourceBridgeEmitterId: "sccp:bsc:source-bridge-emitter:bsc-mainnet:v1",
            requiresSourceBridge: true,
            requiresSourceBridgeConfig: false
        )
    case sccpDomainSolana:
        return SccpSourceRecordProfile(
            chain: adapterProfile.chain,
            proofPlan: adapterProfile.proofPlan,
            finalityModel: adapterProfile.finalityModel,
            sourceTrustAnchorId: "sccp:sol:source-trust-anchor:solana-mainnet-beta-genesis:v1",
            consensusVerifierId: "sccp:sol:consensus-verifier:finalized-slot-bankhash-mainnet-beta:v1",
            messageInclusionVerifierId: "sccp:sol:message-inclusion-verifier:transaction-status-root-branch:v1",
            finalityPolicyId: "sccp:sol:finality-policy:finalized-slot-mainnet-beta:v1",
            sourceStateVerifierId: "sccp:sol:accounts-db-verifier:accounts-lt-hash-mainnet-beta:v1",
            sourceBridgeEmitterId: "",
            requiresSourceBridge: false,
            requiresSourceBridgeConfig: false
        )
    case sccpDomainTon:
        return SccpSourceRecordProfile(
            chain: adapterProfile.chain,
            proofPlan: adapterProfile.proofPlan,
            finalityModel: adapterProfile.finalityModel,
            sourceTrustAnchorId: "sccp:ton:source-trust-anchor:ton-mainnet-masterchain:v1",
            consensusVerifierId: "sccp:ton:consensus-verifier:masterchain-block-proof:v1",
            messageInclusionVerifierId: "sccp:ton:message-inclusion-verifier:shard-transaction-branch:v1",
            finalityPolicyId: "sccp:ton:finality-policy:masterchain-finality:v1",
            sourceStateVerifierId: "sccp:ton:source-state-verifier:shard-state-light-client-mainnet:v1",
            sourceBridgeEmitterId: "",
            requiresSourceBridge: false,
            requiresSourceBridgeConfig: false
        )
    case sccpDomainTron:
        return SccpSourceRecordProfile(
            chain: adapterProfile.chain,
            proofPlan: adapterProfile.proofPlan,
            finalityModel: adapterProfile.finalityModel,
            sourceTrustAnchorId: "sccp:tron:source-trust-anchor:mainnet-witness-schedule:v1",
            consensusVerifierId: "sccp:tron:consensus-verifier:dpos-solid-block-mainnet:v1",
            messageInclusionVerifierId: "sccp:tron:message-inclusion-verifier:transaction-source-mainnet:v1",
            finalityPolicyId: "sccp:tron:finality-policy:solid-block-mainnet:v1",
            sourceStateVerifierId: "",
            sourceBridgeEmitterId: "sccp:tron:source-bridge-emitter:tron-mainnet:v1",
            requiresSourceBridge: true,
            requiresSourceBridgeConfig: true
        )
    case sccpDomainSoraKusama:
        return SccpSourceRecordProfile(
            chain: adapterProfile.chain,
            proofPlan: adapterProfile.proofPlan,
            finalityModel: adapterProfile.finalityModel,
            sourceTrustAnchorId: "sccp:sora-kusama:source-trust-anchor:grandpa-authority-set:v1",
            consensusVerifierId: "sccp:sora-kusama:consensus-verifier:grandpa-finalized-header:v1",
            messageInclusionVerifierId: "sccp:sora-kusama:message-inclusion-verifier:events-storage-proof:v1",
            finalityPolicyId: "sccp:sora-kusama:finality-policy:grandpa-finality:v1",
            sourceStateVerifierId: "sccp:sora-kusama:source-state-verifier:runtime-storage-proof:v1",
            sourceBridgeEmitterId: "",
            requiresSourceBridge: false,
            requiresSourceBridgeConfig: false
        )
    case sccpDomainSoraPolkadot:
        return SccpSourceRecordProfile(
            chain: adapterProfile.chain,
            proofPlan: adapterProfile.proofPlan,
            finalityModel: adapterProfile.finalityModel,
            sourceTrustAnchorId: "sccp:sora-polkadot:source-trust-anchor:grandpa-authority-set:v1",
            consensusVerifierId: "sccp:sora-polkadot:consensus-verifier:grandpa-finalized-header:v1",
            messageInclusionVerifierId: "sccp:sora-polkadot:message-inclusion-verifier:events-storage-proof:v1",
            finalityPolicyId: "sccp:sora-polkadot:finality-policy:grandpa-finality:v1",
            sourceStateVerifierId: "sccp:sora-polkadot:source-state-verifier:runtime-storage-proof:v1",
            sourceBridgeEmitterId: "",
            requiresSourceBridge: false,
            requiresSourceBridgeConfig: false
        )
    case sccpDomainSora2:
        return SccpSourceRecordProfile(
            chain: adapterProfile.chain,
            proofPlan: adapterProfile.proofPlan,
            finalityModel: adapterProfile.finalityModel,
            sourceTrustAnchorId: "sccp:sora2:source-trust-anchor:grandpa-authority-set:v1",
            consensusVerifierId: "sccp:sora2:consensus-verifier:grandpa-finalized-header:v1",
            messageInclusionVerifierId: "sccp:sora2:message-inclusion-verifier:events-storage-proof:v1",
            finalityPolicyId: "sccp:sora2:finality-policy:grandpa-finality:v1",
            sourceStateVerifierId: "sccp:sora2:source-state-verifier:runtime-storage-proof:v1",
            sourceBridgeEmitterId: "",
            requiresSourceBridge: false,
            requiresSourceBridgeConfig: false
        )
    default:
        throw SccpSourceProofHashError.unsupportedSourceAdapterDomain("sourceDomain")
    }
}

private func normalizeSccpSourceMaterial(
    sourceDomain: UInt32,
    sourceTrustAnchorHash: String,
    consensusVerifierHash: String,
    messageInclusionVerifierHash: String,
    finalityPolicyHash: String,
    sourceStateVerifierHash: String?,
    bridgeAddress: String?,
    sourceBridgeEmitterCodeHash: String?,
    networkId: String?,
    ownerAddress: String?,
    configHash: String?
) throws -> SccpNormalizedSourceMaterial {
    let profile = try sccpSourceRecordProfile(sourceDomain: sourceDomain)
    let stateHash: Data
    if profile.sourceStateVerifierId.isEmpty {
        guard sourceStateVerifierHash == nil else {
            throw SccpSourceProofHashError.invalidSourceMaterial("sourceStateVerifierHash")
        }
        stateHash = Data(repeating: 0, count: 32)
    } else {
        guard let sourceStateVerifierHash else {
            throw SccpSourceProofHashError.invalidSourceMaterial("sourceStateVerifierHash")
        }
        stateHash = try sourceProofNonZeroBytesFromHex32(
            sourceStateVerifierHash,
            field: "sourceStateVerifierHash"
        )
        try rejectTonTemplateSourceMaterialComponent(
            stateHash,
            field: "sourceStateVerifierHash",
            sourceDomain: sourceDomain
        )
        try rejectSolanaTemplateSourceMaterialComponent(
            stateHash,
            field: "sourceStateVerifierHash",
            sourceDomain: sourceDomain
        )
    }
    let bridgeEmitterAddress: Data
    let bridgeEmitterCodeHash: Data
    if profile.requiresSourceBridge {
        guard let bridgeAddress else {
            throw SccpSourceProofHashError.invalidSourceMaterial("bridgeAddress")
        }
        guard let sourceBridgeEmitterCodeHash else {
            throw SccpSourceProofHashError.invalidSourceMaterial("sourceBridgeEmitterCodeHash")
        }
        bridgeEmitterAddress = try sourceProofNonZeroBytesFromHex20(bridgeAddress, field: "bridgeAddress")
        bridgeEmitterCodeHash = try sourceProofNonZeroBytesFromHex32(
            sourceBridgeEmitterCodeHash,
            field: "sourceBridgeEmitterCodeHash"
        )
    } else {
        guard bridgeAddress == nil else {
            throw SccpSourceProofHashError.invalidSourceMaterial("sourceBridgeEmitterAddress")
        }
        guard sourceBridgeEmitterCodeHash == nil else {
            throw SccpSourceProofHashError.invalidSourceMaterial("sourceBridgeEmitterCodeHash")
        }
        bridgeEmitterAddress = Data()
        bridgeEmitterCodeHash = Data(repeating: 0, count: 32)
    }
    let bridgeNetworkId: Data
    let bridgeOwnerAddress: Data
    let bridgeConfigHash: Data
    if sourceDomain == sccpDomainEthereum {
        guard let networkId else {
            throw SccpSourceProofHashError.invalidSourceMaterial("networkId")
        }
        guard ownerAddress == nil else {
            throw SccpSourceProofHashError.invalidSourceMaterial("sourceBridgeOwnerAddress")
        }
        guard let configHash else {
            throw SccpSourceProofHashError.invalidSourceMaterial("configHash")
        }
        bridgeNetworkId = try sourceProofNonZeroBytesFromHex32(networkId, field: "networkId")
        bridgeOwnerAddress = Data()
        bridgeConfigHash = try sourceProofNonZeroBytesFromHex32(configHash, field: "configHash")
        let ethMainnetNetworkId = try sourceProofBytesFromHex32(
            sccpEthereumMainnetNetworkId,
            field: "sourceBridgeNetworkId"
        )
        guard bridgeNetworkId == ethMainnetNetworkId else {
            throw SccpSourceProofHashError.invalidSourceMaterial("sourceBridgeNetworkId")
        }
        guard bridgeConfigHash == ethSourceBridgeConfigHash(
            sourceDomain: sourceDomain,
            bridgeAddress: bridgeEmitterAddress,
            networkId: bridgeNetworkId,
            codeHash: bridgeEmitterCodeHash
        ) else {
            throw SccpSourceProofHashError.invalidSourceMaterial("sourceBridgeConfigHash")
        }
    } else if profile.requiresSourceBridgeConfig {
        guard let networkId else {
            throw SccpSourceProofHashError.invalidSourceMaterial("networkId")
        }
        guard let ownerAddress else {
            throw SccpSourceProofHashError.invalidSourceMaterial("ownerAddress")
        }
        guard let configHash else {
            throw SccpSourceProofHashError.invalidSourceMaterial("configHash")
        }
        bridgeNetworkId = try sourceProofNonZeroBytesFromHex32(networkId, field: "networkId")
        bridgeOwnerAddress = try sourceProofNonZeroBytesFromHex20(ownerAddress, field: "ownerAddress")
        bridgeConfigHash = try sourceProofNonZeroBytesFromHex32(configHash, field: "configHash")
        if sourceDomain == sccpDomainTron,
           bridgeConfigHash != tronSourceBridgeConfigHash(
               sourceDomain: sourceDomain,
               bridgeAddress: bridgeEmitterAddress,
               networkId: bridgeNetworkId,
               ownerAddress: bridgeOwnerAddress
           ) {
            throw SccpSourceProofHashError.invalidSourceMaterial("sourceBridgeConfigHash")
        }
    } else {
        guard networkId == nil else {
            throw SccpSourceProofHashError.invalidSourceMaterial("sourceBridgeNetworkId")
        }
        guard ownerAddress == nil else {
            throw SccpSourceProofHashError.invalidSourceMaterial("sourceBridgeOwnerAddress")
        }
        guard configHash == nil else {
            throw SccpSourceProofHashError.invalidSourceMaterial("sourceBridgeConfigHash")
        }
        bridgeNetworkId = Data(repeating: 0, count: 32)
        bridgeOwnerAddress = Data()
        bridgeConfigHash = Data(repeating: 0, count: 32)
    }
    let normalizedSourceTrustAnchorHash = try sourceProofNonZeroBytesFromHex32(
        sourceTrustAnchorHash,
        field: "sourceTrustAnchorHash"
    )
    try rejectTonTemplateSourceMaterialComponent(
        normalizedSourceTrustAnchorHash,
        field: "sourceTrustAnchorHash",
        sourceDomain: sourceDomain
    )
    try rejectSolanaTemplateSourceMaterialComponent(
        normalizedSourceTrustAnchorHash,
        field: "sourceTrustAnchorHash",
        sourceDomain: sourceDomain
    )
    try rejectTronTemplateSourceMaterialComponent(
        normalizedSourceTrustAnchorHash,
        field: "sourceTrustAnchorHash",
        sourceDomain: sourceDomain
    )
    let normalizedConsensusVerifierHash = try sourceProofNonZeroBytesFromHex32(
        consensusVerifierHash,
        field: "consensusVerifierHash"
    )
    try rejectTonTemplateSourceMaterialComponent(
        normalizedConsensusVerifierHash,
        field: "consensusVerifierHash",
        sourceDomain: sourceDomain
    )
    try rejectSolanaTemplateSourceMaterialComponent(
        normalizedConsensusVerifierHash,
        field: "consensusVerifierHash",
        sourceDomain: sourceDomain
    )
    try rejectTronTemplateSourceMaterialComponent(
        normalizedConsensusVerifierHash,
        field: "consensusVerifierHash",
        sourceDomain: sourceDomain
    )
    let normalizedMessageInclusionVerifierHash = try sourceProofNonZeroBytesFromHex32(
        messageInclusionVerifierHash,
        field: "messageInclusionVerifierHash"
    )
    try rejectTonTemplateSourceMaterialComponent(
        normalizedMessageInclusionVerifierHash,
        field: "messageInclusionVerifierHash",
        sourceDomain: sourceDomain
    )
    try rejectSolanaTemplateSourceMaterialComponent(
        normalizedMessageInclusionVerifierHash,
        field: "messageInclusionVerifierHash",
        sourceDomain: sourceDomain
    )
    try rejectTronTemplateSourceMaterialComponent(
        normalizedMessageInclusionVerifierHash,
        field: "messageInclusionVerifierHash",
        sourceDomain: sourceDomain
    )
    let normalizedFinalityPolicyHash = try sourceProofNonZeroBytesFromHex32(
        finalityPolicyHash,
        field: "finalityPolicyHash"
    )
    try rejectTonTemplateSourceMaterialComponent(
        normalizedFinalityPolicyHash,
        field: "finalityPolicyHash",
        sourceDomain: sourceDomain
    )
    try rejectSolanaTemplateSourceMaterialComponent(
        normalizedFinalityPolicyHash,
        field: "finalityPolicyHash",
        sourceDomain: sourceDomain
    )
    try rejectTronTemplateSourceMaterialComponent(
        normalizedFinalityPolicyHash,
        field: "finalityPolicyHash",
        sourceDomain: sourceDomain
    )
    try requirePairwiseNonzeroSourceRoleHashSeparation(
        [
            ("sourceTrustAnchorHash", normalizedSourceTrustAnchorHash),
            ("consensusVerifierHash", normalizedConsensusVerifierHash),
            ("messageInclusionVerifierHash", normalizedMessageInclusionVerifierHash),
            ("finalityPolicyHash", normalizedFinalityPolicyHash),
            ("sourceStateVerifierHash", stateHash),
            ("sourceBridgeEmitterCodeHash", bridgeEmitterCodeHash),
            ("sourceBridgeNetworkId", bridgeNetworkId),
            ("sourceBridgeConfigHash", bridgeConfigHash),
        ],
        label: "sourceVerifierMaterialRoleHash"
    )
    return SccpNormalizedSourceMaterial(
        sourceDomain: sourceDomain,
        profile: profile,
        sourceTrustAnchorHash: normalizedSourceTrustAnchorHash,
        consensusVerifierHash: normalizedConsensusVerifierHash,
        messageInclusionVerifierHash: normalizedMessageInclusionVerifierHash,
        finalityPolicyHash: normalizedFinalityPolicyHash,
        sourceStateVerifierHash: stateHash,
        sourceBridgeEmitterAddress: bridgeEmitterAddress,
        sourceBridgeEmitterCodeHash: bridgeEmitterCodeHash,
        sourceBridgeNetworkId: bridgeNetworkId,
        sourceBridgeOwnerAddress: bridgeOwnerAddress,
        sourceBridgeConfigHash: bridgeConfigHash
    )
}

private func requirePairwiseNonzeroSourceRoleHashSeparation(
    _ roleHashes: [(String, Data)],
    label: String
) throws {
    for index in roleHashes.indices {
        let (roleField, roleHash) = roleHashes[index]
        guard !roleHash.allSatisfy({ $0 == 0 }) else { continue }
        for otherIndex in roleHashes.indices where otherIndex > index {
            let (otherRoleField, otherRoleHash) = roleHashes[otherIndex]
            guard !otherRoleHash.allSatisfy({ $0 == 0 }) else { continue }
            if roleHash == otherRoleHash {
                throw SccpSourceProofHashError.invalidSourceMaterial(
                    "\(label):\(otherRoleField):\(roleField)"
                )
            }
        }
    }
}

private func appendSccpSourceMaterialFields(
    _ material: SccpNormalizedSourceMaterial,
    to out: inout Data
) {
    out.append(1)
    sourceProofAppendU32Le(material.sourceDomain, to: &out)
    sourceProofAppendDataVector(Data(material.profile.chain.utf8), to: &out)
    out.append(material.profile.proofPlan)
    out.append(material.profile.finalityModel)
    sourceProofAppendDataVector(Data(sccpSourceAdapterOpenVerifyCircuitIdV1.utf8), to: &out)
    appendSccpSourceComponentFields(material, to: &out)
}

private func appendSccpSourceComponentFields(
    _ material: SccpNormalizedSourceMaterial,
    to out: inout Data
) {
    sourceProofAppendDataVector(Data(material.profile.sourceTrustAnchorId.utf8), to: &out)
    out.append(material.sourceTrustAnchorHash)
    sourceProofAppendDataVector(Data(material.profile.consensusVerifierId.utf8), to: &out)
    out.append(material.consensusVerifierHash)
    sourceProofAppendDataVector(Data(material.profile.messageInclusionVerifierId.utf8), to: &out)
    out.append(material.messageInclusionVerifierHash)
    sourceProofAppendDataVector(Data(material.profile.finalityPolicyId.utf8), to: &out)
    out.append(material.finalityPolicyHash)
    sourceProofAppendDataVector(Data(material.profile.sourceStateVerifierId.utf8), to: &out)
    out.append(material.sourceStateVerifierHash)
    sourceProofAppendDataVector(Data(material.profile.sourceBridgeEmitterId.utf8), to: &out)
    sourceProofAppendDataVector(material.sourceBridgeEmitterAddress, to: &out)
    out.append(material.sourceBridgeEmitterCodeHash)
    out.append(material.sourceBridgeNetworkId)
    sourceProofAppendDataVector(material.sourceBridgeOwnerAddress, to: &out)
    out.append(material.sourceBridgeConfigHash)
}

private func appendSccpSourceAdapterDeploymentSolanaAuditFields(
    sourceDomain: UInt32,
    towerReplayVerifierHash: String?,
    fullAccountsdbLatticeVerifierHash: String?,
    bankForkChoiceVerifierHash: String?,
    existingRoleHashes: [Data],
    to out: inout Data
) throws {
    let verifierHashes: [(String, String, Data)] = [
        (
            sccpSolanaMainnetTowerReplayVerifierIdV1,
            "solanaTowerReplayVerifierHash",
            try towerReplayVerifierHash.map {
                try sourceProofBytesFromHex32($0, field: "solanaTowerReplayVerifierHash")
            } ?? Data(repeating: 0, count: 32)
        ),
        (
            sccpSolanaMainnetFullAccountsdbLatticeVerifierIdV1,
            "solanaFullAccountsdbLatticeVerifierHash",
            try fullAccountsdbLatticeVerifierHash.map {
                try sourceProofBytesFromHex32($0, field: "solanaFullAccountsdbLatticeVerifierHash")
            } ?? Data(repeating: 0, count: 32)
        ),
        (
            sccpSolanaMainnetBankForkChoiceVerifierIdV1,
            "solanaBankForkChoiceVerifierHash",
            try bankForkChoiceVerifierHash.map {
                try sourceProofBytesFromHex32($0, field: "solanaBankForkChoiceVerifierHash")
            } ?? Data(repeating: 0, count: 32)
        ),
    ]
    let nonzeroCount = verifierHashes.filter { $0.2.contains { byte in byte != 0 } }.count
    guard nonzeroCount == 0 || (sourceDomain == sccpDomainSolana && nonzeroCount == verifierHashes.count) else {
        throw SccpSourceProofHashError.invalidSourceMaterial("solanaAuditVerifierHash")
    }
    guard nonzeroCount > 0 else {
        return
    }
    try requireSolanaFullLightClientAuditRoleSeparation(
        verifierHashes: verifierHashes,
        existingRoleHashes: existingRoleHashes
    )

    out.append(1)
    for (verifierId, _, verifierHash) in verifierHashes {
        sourceProofAppendDataVector(Data(verifierId.utf8), to: &out)
        out.append(verifierHash)
    }
}

private func requireSolanaFullLightClientAuditRoleSeparation(
    verifierHashes: [(String, String, Data)],
    existingRoleHashes: [Data]
) throws {
    for index in verifierHashes.indices {
        let verifierHash = verifierHashes[index].2
        for otherIndex in verifierHashes.indices where otherIndex > index {
            if verifierHash == verifierHashes[otherIndex].2 {
                throw SccpSourceProofHashError.invalidSourceMaterial("solanaAuditVerifierHash")
            }
        }
        for (_, templateHash) in sccpSourceMaterialSolanaTemplateComponentHashesV1 {
            if let templateBytes = try? sourceProofBytesFromHex32(
                templateHash,
                field: "solanaTemplateSourceMaterialHash"
            ), verifierHash == templateBytes {
                throw SccpSourceProofHashError.invalidSourceMaterial("solanaAuditVerifierHash")
            }
        }
        for existingRoleHash in existingRoleHashes
            where !existingRoleHash.allSatisfy({ $0 == 0 }) && verifierHash == existingRoleHash
        {
            throw SccpSourceProofHashError.invalidSourceMaterial("solanaAuditVerifierHash")
        }
    }
}

private func appendSccpSourceAdapterDeploymentTonAuditFields(
    sourceDomain: UInt32,
    masterchainConfigVerifierHash: String?,
    validatorSetTransitionVerifierHash: String?,
    shardAccountsDictionaryVerifierHash: String?,
    existingRoleHashes: [Data],
    to out: inout Data
) throws {
    let verifierHashes: [(String, String, Data)] = [
        (
            sccpTonMainnetMasterchainConfigVerifierIdV1,
            "tonMasterchainConfigVerifierHash",
            try masterchainConfigVerifierHash.map {
                try sourceProofBytesFromHex32($0, field: "tonMasterchainConfigVerifierHash")
            } ?? Data(repeating: 0, count: 32)
        ),
        (
            sccpTonMainnetValidatorSetTransitionVerifierIdV1,
            "tonValidatorSetTransitionVerifierHash",
            try validatorSetTransitionVerifierHash.map {
                try sourceProofBytesFromHex32($0, field: "tonValidatorSetTransitionVerifierHash")
            } ?? Data(repeating: 0, count: 32)
        ),
        (
            sccpTonMainnetShardAccountsDictionaryVerifierIdV1,
            "tonShardAccountsDictionaryVerifierHash",
            try shardAccountsDictionaryVerifierHash.map {
                try sourceProofBytesFromHex32($0, field: "tonShardAccountsDictionaryVerifierHash")
            } ?? Data(repeating: 0, count: 32)
        ),
    ]
    let nonzeroCount = verifierHashes.filter { $0.2.contains { byte in byte != 0 } }.count
    guard nonzeroCount == 0 || (sourceDomain == sccpDomainTon && nonzeroCount == verifierHashes.count) else {
        throw SccpSourceProofHashError.invalidSourceMaterial("tonAuditVerifierHash")
    }
    guard nonzeroCount > 0 else {
        return
    }
    try requireTonFullLightClientAuditRoleSeparation(
        verifierHashes: verifierHashes,
        existingRoleHashes: existingRoleHashes
    )

    out.append(2)
    for (verifierId, _, verifierHash) in verifierHashes {
        sourceProofAppendDataVector(Data(verifierId.utf8), to: &out)
        out.append(verifierHash)
    }
}

private func requireTonFullLightClientAuditRoleSeparation(
    verifierHashes: [(String, String, Data)],
    existingRoleHashes: [Data]
) throws {
    for index in verifierHashes.indices {
        let verifierHash = verifierHashes[index].2
        for otherIndex in verifierHashes.indices where otherIndex > index {
            if verifierHash == verifierHashes[otherIndex].2 {
                throw SccpSourceProofHashError.invalidSourceMaterial("tonAuditVerifierHash")
            }
        }
        for (_, templateHash) in sccpSourceMaterialTonTemplateComponentHashesV1 {
            if let templateBytes = try? sourceProofBytesFromHex32(
                templateHash,
                field: "tonTemplateSourceMaterialHash"
            ), verifierHash == templateBytes {
                throw SccpSourceProofHashError.invalidSourceMaterial("tonAuditVerifierHash")
            }
        }
        for existingRoleHash in existingRoleHashes
            where !existingRoleHash.allSatisfy({ $0 == 0 }) && verifierHash == existingRoleHash
        {
            throw SccpSourceProofHashError.invalidSourceMaterial("tonAuditVerifierHash")
        }
    }
}

private func sourceProofValidateTronMptProofNodes(_ nodes: [Data]) throws {
    try sourceProofValidateMptProofNodes(nodes, field: "receiptTrieProofNodes")
}

private func sourceProofValidateMptProofNodes(_ nodes: [Data], field: String) throws {
    guard !nodes.isEmpty, nodes.count <= 64 else {
        throw SccpSourceProofHashError.invalidValidatorSet(field)
    }
    for (index, node) in nodes.enumerated() {
        guard !node.isEmpty, node.count <= 16 * 1024 else {
            throw SccpSourceProofHashError.invalidValidatorSet("\(field)[\(index)]")
        }
    }
}

private func sourceProofValidateTronTransactionMerkleBranch(_ branch: [Data]) throws {
    guard branch.count <= sccpTronMaxTransactionMerkleBranchNodes else {
        throw SccpSourceProofHashError.invalidValidatorSet("transactionMerkleBranch")
    }
    for (index, sibling) in branch.enumerated() {
        guard sibling.count == 32 else {
            throw SccpSourceProofHashError.invalidBranch("transactionMerkleBranch[\(index)]")
        }
    }
}

private func sourceProofAppendDataVector(_ value: Data, to out: inout Data) {
    sourceProofAppendU32Le(UInt32(value.count), to: &out)
    out.append(value)
}

private func sourceProofCanonicalBscValidatorStorageProofBytes(_ proof: BscValidatorStorageProof) throws -> Data {
    guard proof.version == 1 else {
        throw SccpSourceProofHashError.invalidValidatorSet("BSC validator storage proof version")
    }
    try sourceProofValidateMptProofNodes(proof.storageProofNodes, field: "storageProofNodes")
    var out = Data()
    out.append(proof.version)
    sourceProofAppendU32Le(proof.validatorIndex, to: &out)
    try out.append(sourceProofBytesFromHex32(proof.storageSlot, field: "storageSlot"))
    let storageValueHash = try sourceProofBytesFromHex32(proof.storageValueHash, field: "storageValueHash")
    let expectedStorageValueHash = try sourceProofBytesFromHex32(
        bscValidatorSetStorageValueHash(storageValue: proof.storageValue),
        field: "storageValueHash"
    )
    guard storageValueHash == expectedStorageValueHash else {
        throw SccpSourceProofHashError.invalidValidatorSet("storageValueHash")
    }
    sourceProofAppendDataVector(proof.storageValue, to: &out)
    out.append(storageValueHash)
    sourceProofAppendU32Le(UInt32(proof.storageProofNodes.count), to: &out)
    for node in proof.storageProofNodes {
        sourceProofAppendDataVector(node, to: &out)
    }
    return out
}

private func sourceProofReadU32Le(_ bytes: [UInt8], cursor: inout Int) throws -> UInt32 {
    guard cursor + 4 <= bytes.count else {
        throw SccpSourceProofHashError.invalidValidatorSet("payload")
    }
    let value = UInt32(bytes[cursor])
        | (UInt32(bytes[cursor + 1]) << 8)
        | (UInt32(bytes[cursor + 2]) << 16)
        | (UInt32(bytes[cursor + 3]) << 24)
    cursor += 4
    return value
}

private func sourceProofReadU64Le(_ bytes: [UInt8], cursor: inout Int) throws -> UInt64 {
    guard cursor + 8 <= bytes.count else {
        throw SccpSourceProofHashError.invalidValidatorSet("payload")
    }
    var value: UInt64 = 0
    for shift in 0..<8 {
        value |= UInt64(bytes[cursor + shift]) << UInt64(shift * 8)
    }
    cursor += 8
    return value
}

private func sourceProofValidateBscValidatorSetPayload(_ payload: Data) throws {
    guard payload.count <= sccpBscMaxValidatorSetPayloadBytes else {
        throw SccpSourceProofHashError.invalidValidatorSet("validatorSetPayload")
    }
    let bytes = [UInt8](payload)
    var cursor = 0
    guard !bytes.isEmpty, bytes[cursor] == 1 else {
        throw SccpSourceProofHashError.invalidValidatorSet("validatorSetPayload")
    }
    cursor += 1
    let count = Int(try sourceProofReadU32Le(bytes, cursor: &cursor))
    guard count > 0,
          count <= sccpBscMaxParliaValidators,
          bytes.count - cursor == count * (20 + 8) else {
        throw SccpSourceProofHashError.invalidValidatorSet("validatorSetPayload")
    }
    var seenAddresses = Set<String>()
    for index in 0..<count {
        let addressEnd = cursor + 20
        guard addressEnd <= bytes.count else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses[\(index)]")
        }
        let address = Data(bytes[cursor..<addressEnd])
        cursor = addressEnd
        guard address.contains(where: { $0 != 0 }) else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses[\(index)]")
        }
        guard seenAddresses.insert(address.hexEncodedString()).inserted else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses[\(index)]")
        }
        let power = try sourceProofReadU64Le(bytes, cursor: &cursor)
        guard power != 0 else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorPowers[\(index)]")
        }
    }
    guard cursor == bytes.count else {
        throw SccpSourceProofHashError.invalidValidatorSet("validatorSetPayload")
    }
}

private func sourceProofValidateEthSyncCommitteePayload(_ payload: Data) throws {
    guard payload.count <= sccpEthMaxSyncCommitteePayloadBytes else {
        throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePayload")
    }
    let bytes = [UInt8](payload)
    var cursor = 0
    guard !bytes.isEmpty, bytes[cursor] == 1 else {
        throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePayload")
    }
    cursor += 1
    let count = Int(try sourceProofReadU32Le(bytes, cursor: &cursor))
    guard count == sccpEthMainnetSyncCommitteeAuthorities else {
        throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePayload")
    }
    var seenPublicKeys = Set<String>()
    for index in 0..<count {
        let publicKeyLength = Int(try sourceProofReadU32Le(bytes, cursor: &cursor))
        guard publicKeyLength == sccpEthSyncCommitteePublicKeyBytes,
              cursor + publicKeyLength <= bytes.count else {
            throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePublicKeys[\(index)]")
        }
        let publicKey = Data(bytes[cursor..<cursor + publicKeyLength])
        cursor += publicKeyLength
        guard publicKey.contains(where: { $0 != 0 }) else {
            throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePublicKeys[\(index)]")
        }
        guard seenPublicKeys.insert(publicKey.hexEncodedString()).inserted else {
            throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePublicKeys[\(index)]")
        }
        let weight = try sourceProofReadU64Le(bytes, cursor: &cursor)
        guard weight == 1 else {
            throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteeWeights[\(index)]")
        }
        let popLength = Int(try sourceProofReadU32Le(bytes, cursor: &cursor))
        guard popLength == sccpEthSyncCommitteePopBytes,
              cursor + popLength <= bytes.count else {
            throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePops[\(index)]")
        }
        let pop = Data(bytes[cursor..<cursor + popLength])
        guard pop.contains(where: { $0 != 0 }) else {
            throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePops[\(index)]")
        }
        cursor += popLength
    }
    guard cursor == bytes.count else {
        throw SccpSourceProofHashError.invalidValidatorSet("syncCommitteePayload")
    }
}

private func sourceProofValidateSubstrateAuthoritySetPayload(_ payload: Data) throws {
    guard payload.count <= sccpSubstrateMaxAuthoritySetPayloadBytes else {
        throw SccpSourceProofHashError.invalidValidatorSet("authoritySetPayload")
    }
    let bytes = [UInt8](payload)
    var cursor = 0
    guard !bytes.isEmpty, bytes[cursor] == 1 else {
        throw SccpSourceProofHashError.invalidValidatorSet("authoritySetPayload")
    }
    cursor += 1
    let count = Int(try sourceProofReadU32Le(bytes, cursor: &cursor))
    guard count > 0, count <= sccpSubstrateMaxAuthorities, bytes.count - cursor == count * 40 else {
        throw SccpSourceProofHashError.invalidValidatorSet("authoritySetPayload")
    }
    var seenPublicKeys = Set<String>()
    for index in 0..<count {
        guard cursor + 32 <= bytes.count else {
            throw SccpSourceProofHashError.invalidValidatorSet("authorityPublicKeys[\(index)]")
        }
        let publicKey = Data(bytes[cursor..<cursor + 32])
        cursor += 32
        guard publicKey.contains(where: { $0 != 0 }) else {
            throw SccpSourceProofHashError.invalidValidatorSet("authorityPublicKeys[\(index)]")
        }
        guard seenPublicKeys.insert(publicKey.hexEncodedString()).inserted else {
            throw SccpSourceProofHashError.invalidValidatorSet("authorityPublicKeys[\(index)]")
        }
        let weight = try sourceProofReadU64Le(bytes, cursor: &cursor)
        guard weight != 0 else {
            throw SccpSourceProofHashError.invalidValidatorSet("authorityWeights[\(index)]")
        }
    }
    guard cursor == bytes.count else {
        throw SccpSourceProofHashError.invalidValidatorSet("authoritySetPayload")
    }
}

private let sourceProofBscParliaExtraVanityBytes = 32
private let sourceProofBscParliaExtraSealBytes = 65
private let sourceProofBscParliaValidatorAddressBytes = 20
private let sourceProofBscParliaValidatorBlsKeyBytes = 48
private let sourceProofBscMaxParliaValidators = 255

private func sourceProofCanonicalBscValidatorSetPayloadBytes(addresses: [Data]) throws -> Data {
    guard !addresses.isEmpty, addresses.count <= sourceProofBscMaxParliaValidators else {
        throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses")
    }
    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(UInt32(addresses.count), to: &out)
    var seenAddresses = Set<String>()
    for (index, address) in addresses.enumerated() {
        guard address.count == sourceProofBscParliaValidatorAddressBytes else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses[\(index)]")
        }
        guard address.contains(where: { $0 != 0 }) else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses[\(index)]")
        }
        guard seenAddresses.insert(address.hexEncodedString()).inserted else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses[\(index)]")
        }
        out.append(address)
        sourceProofAppendU64Le(1, to: &out)
    }
    return out
}

private func sourceProofCanonicalBscValidatorSetPayloadBytes(addresses: [Data],
                                                             powers: [UInt64]) throws -> Data {
    guard !addresses.isEmpty,
          addresses.count == powers.count,
          addresses.count <= sourceProofBscMaxParliaValidators else {
        throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses")
    }
    var out = Data()
    out.append(1)
    sourceProofAppendU32Le(UInt32(addresses.count), to: &out)
    var seenAddresses = Set<String>()
    for (index, pair) in zip(addresses, powers).enumerated() {
        let address = pair.0
        let power = pair.1
        guard address.count == sourceProofBscParliaValidatorAddressBytes else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses[\(index)]")
        }
        guard address.contains(where: { $0 != 0 }) else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses[\(index)]")
        }
        guard seenAddresses.insert(address.hexEncodedString()).inserted else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorAddresses[\(index)]")
        }
        guard power != 0 else {
            throw SccpSourceProofHashError.invalidValidatorSet("validatorPowers[\(index)]")
        }
        out.append(address)
        sourceProofAppendU64Le(power, to: &out)
    }
    return out
}

private func sourceProofBscSignerIndices(signersBitmap: Data, rosterLength: Int) throws -> [Int] {
    guard signersBitmap.count == (rosterLength + 7) / 8 else {
        throw SccpSourceProofHashError.invalidValidatorSet("signersBitmap")
    }
    var indices: [Int] = []
    for (byteIndex, value) in [UInt8](signersBitmap).enumerated() {
        for bit in 0..<8 {
            let validatorIndex = byteIndex * 8 + bit
            let bitSet = (value & UInt8(1 << bit)) != 0
            if validatorIndex >= rosterLength {
                guard !bitSet else {
                    throw SccpSourceProofHashError.invalidValidatorSet("signersBitmap")
                }
            } else if bitSet {
                indices.append(validatorIndex)
            }
        }
    }
    guard !indices.isEmpty else {
        throw SccpSourceProofHashError.invalidValidatorSet("signersBitmap")
    }
    return indices
}

private func sourceProofBscValidatorAddress20(publicKey: Data, field: String) throws -> Data {
    let bytes = [UInt8](publicKey)
    let x: SourceProofBigUInt
    let y: SourceProofBigUInt
    if bytes.count == 33, bytes[0] == 0x02 || bytes[0] == 0x03 {
        x = SourceProofBigUInt(bigEndian: Data(bytes[1..<33]))
        guard x.compare(sourceProofSecpFieldPrime) < 0 else {
            throw SccpSourceProofHashError.invalidValidatorSet(field)
        }
        let x2 = sourceProofSecpModSquare(x, modulus: sourceProofSecpFieldPrime)
        let x3 = sourceProofSecpModMul(x2, x, modulus: sourceProofSecpFieldPrime)
        let alpha = sourceProofSecpModAdd(x3, SourceProofBigUInt(7), modulus: sourceProofSecpFieldPrime)
        var candidateY = sourceProofSecpModPow(alpha, sourceProofSecpFieldSqrtExponent, modulus: sourceProofSecpFieldPrime)
        guard sourceProofSecpModSquare(candidateY, modulus: sourceProofSecpFieldPrime) == alpha else {
            throw SccpSourceProofHashError.invalidValidatorSet(field)
        }
        if candidateY.bit(at: 0) != (bytes[0] == 0x03) {
            candidateY = sourceProofSecpModNegate(candidateY, modulus: sourceProofSecpFieldPrime)
        }
        y = candidateY
    } else if bytes.count == 65, bytes[0] == 0x04 {
        x = SourceProofBigUInt(bigEndian: Data(bytes[1..<33]))
        y = SourceProofBigUInt(bigEndian: Data(bytes[33..<65]))
        guard x.compare(sourceProofSecpFieldPrime) < 0,
              y.compare(sourceProofSecpFieldPrime) < 0 else {
            throw SccpSourceProofHashError.invalidValidatorSet(field)
        }
        let x2 = sourceProofSecpModSquare(x, modulus: sourceProofSecpFieldPrime)
        let x3 = sourceProofSecpModMul(x2, x, modulus: sourceProofSecpFieldPrime)
        let alpha = sourceProofSecpModAdd(x3, SourceProofBigUInt(7), modulus: sourceProofSecpFieldPrime)
        guard sourceProofSecpModSquare(y, modulus: sourceProofSecpFieldPrime) == alpha else {
            throw SccpSourceProofHashError.invalidValidatorSet(field)
        }
    } else {
        throw SccpSourceProofHashError.invalidValidatorSet(field)
    }
    var uncompressed = x.fixedBigEndianData(byteCount: 32)
    uncompressed.append(y.fixedBigEndianData(byteCount: 32))
    return Data(irohaKeccak256(uncompressed).suffix(20))
}

private func sourceProofBscParliaPayloadCandidates(extraData: Data) throws -> [Data] {
    let minimumExtra = sourceProofBscParliaExtraVanityBytes + sourceProofBscParliaExtraSealBytes
    guard extraData.count > minimumExtra else {
        return []
    }
    let bytes = [UInt8](extraData)
    let regionStart = sourceProofBscParliaExtraVanityBytes
    let regionEnd = extraData.count - sourceProofBscParliaExtraSealBytes
    let validatorRegion = Array(bytes[regionStart..<regionEnd])
    guard !validatorRegion.isEmpty else {
        return []
    }
    var candidates: [Data] = []
    func pushCandidate(_ addresses: [Data]) {
        if let payload = try? sourceProofCanonicalBscValidatorSetPayloadBytes(addresses: addresses),
           !candidates.contains(payload) {
            candidates.append(payload)
        }
    }

    if validatorRegion.count % sourceProofBscParliaValidatorAddressBytes == 0 {
        let count = validatorRegion.count / sourceProofBscParliaValidatorAddressBytes
        if count <= sourceProofBscMaxParliaValidators {
            var addresses: [Data] = []
            for offset in stride(from: 0, to: validatorRegion.count, by: sourceProofBscParliaValidatorAddressBytes) {
                addresses.append(Data(validatorRegion[offset..<offset + sourceProofBscParliaValidatorAddressBytes]))
            }
            pushCandidate(addresses)
        }
    }

    let lubanCount = Int(validatorRegion[0])
    let lubanStride = sourceProofBscParliaValidatorAddressBytes + sourceProofBscParliaValidatorBlsKeyBytes
    let lubanRegionLength = 1 + lubanCount * lubanStride
    if lubanCount != 0,
       lubanCount <= sourceProofBscMaxParliaValidators,
       validatorRegion.count >= lubanRegionLength {
        var addresses: [Data] = []
        for index in 0..<lubanCount {
            let start = 1 + index * lubanStride
            addresses.append(Data(validatorRegion[start..<start + sourceProofBscParliaValidatorAddressBytes]))
        }
        pushCandidate(addresses)
    }

    return candidates
}

/// Canonical legacy or EIP-2718 typed Ethereum receipt RLP bytes.
public func canonicalEvmReceiptRlp(_ receipt: [String: Any]) throws -> Data {
    let status = try sourceProofEthereumRpcQuantity(receipt["status"], field: "receipt.status")
    guard status == 0 || status == 1 else {
        throw SccpSourceProofHashError.invalidRlp("receipt.status")
    }
    let payload = sourceProofRlpList([
        sourceProofRlpString(sourceProofMinimalBigEndianBytes(status)),
        sourceProofRlpString(
            sourceProofMinimalBigEndianBytes(
                try sourceProofEthereumRpcQuantity(
                    sourceProofStrictFirstPresent(
                        receipt,
                        field: "receipt.cumulativeGasUsed",
                        "cumulativeGasUsed",
                        "cumulative_gas_used"
                    ),
                    field: "receipt.cumulativeGasUsed"
                )
            )
        ),
        try sourceProofRlpString(
            sourceProofEthereumRpcHexBytes(
                sourceProofStrictFirstPresent(
                    receipt,
                    field: "receipt.logsBloom",
                    "logsBloom",
                    "logs_bloom"
                ),
                field: "receipt.logsBloom",
                byteLength: 256,
                nonzero: false
            )
        ),
        try sourceProofRlpList(sourceProofEvmReceiptLogsForRlp(receipt))
    ])
    guard let receiptType = try sourceProofEvmReceiptType(receipt) else {
        return payload
    }
    var out = Data([receiptType])
    out.append(payload)
    return out
}

/// Raw Ethereum receipt-trie key: RLP(transactionIndex), not a hashed secure-trie key.
public func evmReceiptTrieKey(_ transactionIndex: Any) throws -> String {
    let index = try sourceProofEthereumUnsignedInteger(transactionIndex, field: "transactionIndex")
    return "0x" + sourceProofRlpString(sourceProofMinimalBigEndianBytes(index)).hexEncodedString()
}

/// Build a receipt-trie proof from an ordered `eth_getBlockReceipts` response.
public func buildEvmReceiptTrieProofFromReceipts(
    _ receipts: [[String: Any]],
    transactionIndex: Any
) throws -> EvmReceiptTrieProof {
    guard !receipts.isEmpty, receipts.count <= sccpEvmMaxBlockReceipts else {
        throw SccpSourceProofHashError.invalidValidatorSet("blockReceipts")
    }
    let targetIndex = try sourceProofEthereumUnsignedInteger(
        transactionIndex,
        field: "transactionIndex",
        max: UInt64(receipts.count - 1)
    )
    var items: [SourceProofEvmTrieItem] = []
    items.reserveCapacity(receipts.count)
    var seenTransactionHashes = Set<Data>()
    var targetReceiptRlp: Data?
    for (index, receipt) in receipts.enumerated() {
        let receiptIndex = try sourceProofEthereumRpcQuantity(
            sourceProofStrictFirstPresent(
                receipt,
                field: "blockReceipts[\(index)].transactionIndex",
                "transactionIndex",
                "transaction_index"
            ),
            field: "blockReceipts[\(index)].transactionIndex"
        )
        guard receiptIndex == UInt64(index) else {
            throw SccpSourceProofHashError.invalidRlp("blockReceipts[\(index)].transactionIndex")
        }
        let transactionHash = try sourceProofEthereumRpcHexBytes(
            sourceProofStrictFirstPresent(
                receipt,
                field: "blockReceipts[\(index)].transactionHash",
                "transactionHash",
                "transaction_hash"
            ),
            field: "blockReceipts[\(index)].transactionHash",
            byteLength: 32
        )
        guard seenTransactionHashes.insert(transactionHash).inserted else {
            throw SccpSourceProofHashError.invalidRlp("blockReceipts.transactionHash")
        }
        let encodedReceipt = try canonicalEvmReceiptRlp(receipt)
        if receiptIndex == targetIndex {
            targetReceiptRlp = encodedReceipt
        }
        let key = sourceProofRlpString(sourceProofMinimalBigEndianBytes(UInt64(index)))
        items.append(SourceProofEvmTrieItem(path: sourceProofBytesToNibbles(key), value: encodedReceipt))
    }
    let root = try sourceProofBuildEvmTrieNode(items)
    let receiptsRoot = "0x" + Data(irohaKeccak256(try sourceProofEncodeEvmTrieNode(root))).hexEncodedString()
    let receiptTrieKey = sourceProofRlpString(sourceProofMinimalBigEndianBytes(targetIndex))
    let proofNodes = try sourceProofCollectEvmTrieProofNodes(root, path: sourceProofBytesToNibbles(receiptTrieKey))
    try sourceProofValidateMptProofNodes(proofNodes, field: "receiptTrieProofNodes")
    guard let targetReceiptRlp else {
        throw SccpSourceProofHashError.invalidRlp("transactionIndex")
    }
    return EvmReceiptTrieProof(
        receiptsRoot: receiptsRoot,
        receiptRlp: "0x" + targetReceiptRlp.hexEncodedString(),
        receiptTrieKey: "0x" + receiptTrieKey.hexEncodedString(),
        receiptTrieProofNodes: proofNodes
    )
}

private func sourceProofEvmReceiptType(_ receipt: [String: Any]) throws -> UInt8? {
    guard let typeInput = receipt["type"] else {
        return nil
    }
    let receiptType = try sourceProofEthereumRpcQuantity(typeInput, field: "receipt.type")
    if receiptType == 0 {
        return nil
    }
    guard receiptType <= 0x7f else {
        throw SccpSourceProofHashError.invalidRlp("receipt.type")
    }
    let admittedType = UInt8(receiptType)
    guard (0x01...0x04).contains(admittedType) else {
        throw SccpSourceProofHashError.invalidRlp("receipt.type")
    }
    return admittedType
}

private func sourceProofEvmReceiptLogsForRlp(_ receipt: [String: Any]) throws -> [Data] {
    let logs: [[String: Any]]
    if let typedLogs = receipt["logs"] as? [[String: Any]] {
        logs = typedLogs
    } else if let rawLogs = receipt["logs"] as? [Any] {
        logs = try rawLogs.enumerated().map { index, value in
            guard let log = value as? [String: Any] else {
                throw SccpSourceProofHashError.invalidRlp("receipt.logs[\(index)]")
            }
            return log
        }
    } else {
        throw SccpSourceProofHashError.invalidRlp("receipt.logs")
    }
    return try logs.enumerated().map { index, log in
        if (log["removed"] as? Bool) == true {
            throw SccpSourceProofHashError.invalidRlp("receipt.logs[\(index)]")
        }
        guard let topics = log["topics"] as? [Any], topics.count <= 4 else {
            throw SccpSourceProofHashError.invalidRlp("receipt.logs[\(index)].topics")
        }
        let encodedTopics = try topics.enumerated().map { topicIndex, topic in
            try sourceProofRlpString(
                sourceProofEthereumRpcHexBytes(
                    topic,
                    field: "receipt.logs[\(index)].topics[\(topicIndex)]",
                    byteLength: 32,
                    nonzero: false
                )
            )
        }
        return try sourceProofRlpList([
            sourceProofRlpString(
                sourceProofEthereumRpcHexBytes(
                    log["address"],
                    field: "receipt.logs[\(index)].address",
                    byteLength: 20,
                    nonzero: false
                )
            ),
            sourceProofRlpList(encodedTopics),
            sourceProofRlpString(
                sourceProofEthereumRpcHexBytes(
                    log["data"],
                    field: "receipt.logs[\(index)].data",
                    nonzero: false,
                    allowEmpty: true
                )
            )
        ])
    }
}

private struct SourceProofEvmTrieItem {
    let path: [UInt8]
    let value: Data
}

private final class SourceProofEvmTrieNode {
    enum Kind {
        case leaf(path: [UInt8], value: Data)
        case `extension`(path: [UInt8], child: SourceProofEvmTrieNode)
        case branch(children: [SourceProofEvmTrieNode?], value: Data)
    }

    let kind: Kind
    var rlp: Data?

    init(_ kind: Kind) {
        self.kind = kind
    }
}

private func sourceProofBuildEvmTrieNode(_ items: [SourceProofEvmTrieItem]) throws -> SourceProofEvmTrieNode {
    guard !items.isEmpty else {
        throw SccpSourceProofHashError.invalidRlp("trie")
    }
    if items.count == 1 {
        return SourceProofEvmTrieNode(.leaf(path: items[0].path, value: items[0].value))
    }
    let prefix = sourceProofLongestCommonNibblePrefix(items.map(\.path))
    if !prefix.isEmpty {
        return try SourceProofEvmTrieNode(
            .extension(
                path: prefix,
                child: sourceProofBuildEvmTrieNode(
                    items.map { SourceProofEvmTrieItem(path: Array($0.path.dropFirst(prefix.count)), value: $0.value) }
                )
            )
        )
    }
    var grouped = Array(repeating: [SourceProofEvmTrieItem](), count: 16)
    var branchValue = Data()
    for item in items {
        if item.path.isEmpty {
            branchValue = item.value
        } else {
            grouped[Int(item.path[0])].append(
                SourceProofEvmTrieItem(path: Array(item.path.dropFirst()), value: item.value)
            )
        }
    }
    let children = try grouped.map { group -> SourceProofEvmTrieNode? in
        group.isEmpty ? nil : try sourceProofBuildEvmTrieNode(group)
    }
    return SourceProofEvmTrieNode(.branch(children: children, value: branchValue))
}

private func sourceProofEncodeEvmTrieNode(_ node: SourceProofEvmTrieNode) throws -> Data {
    if let rlp = node.rlp {
        return rlp
    }
    let encoded: Data
    switch node.kind {
    case let .leaf(path, value):
        encoded = sourceProofRlpList([
            sourceProofRlpString(try sourceProofEncodeEvmTrieCompactPath(path, leaf: true)),
            sourceProofRlpString(value)
        ])
    case let .extension(path, child):
        encoded = try sourceProofRlpList([
            sourceProofRlpString(sourceProofEncodeEvmTrieCompactPath(path, leaf: false)),
            sourceProofRlpString(sourceProofEvmTrieNodeReference(child))
        ])
    case let .branch(children, value):
        var fields = try children.map { child -> Data in
            let reference: Data
            if let child {
                reference = try sourceProofEvmTrieNodeReference(child)
            } else {
                reference = Data()
            }
            return sourceProofRlpString(reference)
        }
        fields.append(sourceProofRlpString(value))
        encoded = sourceProofRlpList(fields)
    }
    node.rlp = encoded
    return encoded
}

private func sourceProofEvmTrieNodeReference(_ node: SourceProofEvmTrieNode) throws -> Data {
    let rlp = try sourceProofEncodeEvmTrieNode(node)
    return rlp.count < 32 ? rlp : Data(irohaKeccak256(rlp))
}

private func sourceProofCollectEvmTrieProofNodes(_ node: SourceProofEvmTrieNode, path: [UInt8]) throws -> [Data] {
    var proof = [try sourceProofEncodeEvmTrieNode(node)]
    switch node.kind {
    case let .leaf(nodePath, _):
        guard nodePath == path else {
            throw SccpSourceProofHashError.invalidRlp("receiptTrieProofNodes")
        }
    case let .extension(nodePath, child):
        guard path.count >= nodePath.count, Array(path.prefix(nodePath.count)) == nodePath else {
            throw SccpSourceProofHashError.invalidRlp("receiptTrieProofNodes")
        }
        proof.append(contentsOf: try sourceProofCollectEvmTrieProofNodes(child, path: Array(path.dropFirst(nodePath.count))))
    case let .branch(children, value):
        if path.isEmpty {
            guard !value.isEmpty else {
                throw SccpSourceProofHashError.invalidRlp("receiptTrieProofNodes")
            }
        } else {
            guard let child = children[Int(path[0])] else {
                throw SccpSourceProofHashError.invalidRlp("receiptTrieProofNodes")
            }
            proof.append(contentsOf: try sourceProofCollectEvmTrieProofNodes(child, path: Array(path.dropFirst())))
        }
    }
    return proof
}

private func sourceProofEncodeEvmTrieCompactPath(_ nibbles: [UInt8], leaf: Bool) throws -> Data {
    for nibble in nibbles where nibble > 15 {
        throw SccpSourceProofHashError.invalidRlp("triePath")
    }
    let flags: UInt8 = leaf ? 2 : 0
    var out = Data()
    var start = 0
    if nibbles.count % 2 == 1 {
        out.append(((flags + 1) << 4) | nibbles[0])
        start = 1
    } else {
        out.append(flags << 4)
    }
    var index = start
    while index < nibbles.count {
        out.append((nibbles[index] << 4) | nibbles[index + 1])
        index += 2
    }
    return out
}

private func sourceProofBytesToNibbles(_ bytes: Data) -> [UInt8] {
    bytes.flatMap { byte in [byte >> 4, byte & 0x0f] }
}

private func sourceProofLongestCommonNibblePrefix(_ paths: [[UInt8]]) -> [UInt8] {
    guard var prefix = paths.first else {
        return []
    }
    for path in paths.dropFirst() {
        var index = 0
        let limit = min(prefix.count, path.count)
        while index < limit, prefix[index] == path[index] {
            index += 1
        }
        prefix.removeSubrange(index..<prefix.count)
        if prefix.isEmpty {
            break
        }
    }
    return prefix
}

private func sourceProofFirstPresent(_ input: [String: Any], _ keys: String...) -> Any? {
    for key in keys where input.keys.contains(key) {
        return input[key]
    }
    return nil
}

private func sourceProofStrictFirstPresent(
    _ input: [String: Any],
    field: String,
    _ keys: String...
) throws -> Any? {
    var selected: Any?
    var found = false
    for key in keys where input.keys.contains(key) {
        guard !found else {
            throw SccpSourceProofHashError.invalidRlp(field)
        }
        selected = input[key]
        found = true
    }
    return selected
}

private func sourceProofEthereumRpcQuantity(_ value: Any?, field: String) throws -> UInt64 {
    guard let text = value as? String,
          text.trimmingCharacters(in: .whitespacesAndNewlines) == text,
          text.hasPrefix("0x") else {
        throw SccpSourceProofHashError.invalidRlp(field)
    }
    let hex = String(text.dropFirst(2))
    guard !hex.isEmpty,
          hex == "0" || (hex.first != "0" && hex.allSatisfy { sourceProofIsLowerHex($0) }),
          let value = UInt64(hex, radix: 16) else {
        throw SccpSourceProofHashError.invalidRlp(field)
    }
    return value
}

private func sourceProofEthereumUnsignedInteger(_ value: Any?,
                                                field: String,
                                                max: UInt64 = UInt64.max) throws -> UInt64 {
    let parsed: UInt64
    switch value {
    case let value as UInt64:
        parsed = value
    case let value as UInt32:
        parsed = UInt64(value)
    case let value as UInt:
        parsed = UInt64(value)
    case let value as Int:
        guard value >= 0 else {
            throw SccpSourceProofHashError.invalidRlp(field)
        }
        parsed = UInt64(value)
    case let text as String:
        guard text.trimmingCharacters(in: .whitespacesAndNewlines) == text else {
            throw SccpSourceProofHashError.invalidRlp(field)
        }
        if text.hasPrefix("0x") {
            parsed = try sourceProofEthereumRpcQuantity(text, field: field)
        } else {
            guard !text.isEmpty,
                  text == "0" || (text.first != "0" && text.allSatisfy({ sourceProofIsDecimalDigit($0) })),
                  let value = UInt64(text, radix: 10) else {
                throw SccpSourceProofHashError.invalidRlp(field)
            }
            parsed = value
        }
    default:
        throw SccpSourceProofHashError.invalidRlp(field)
    }
    guard parsed <= max else {
        throw SccpSourceProofHashError.invalidRlp(field)
    }
    return parsed
}

private func sourceProofEthereumRpcHexBytes(_ value: Any?,
                                            field: String,
                                            byteLength: Int? = nil,
                                            nonzero: Bool = true,
                                            allowEmpty: Bool = false) throws -> Data {
    guard let text = value as? String,
          text.trimmingCharacters(in: .whitespacesAndNewlines) == text,
          text.hasPrefix("0x") else {
        throw SccpSourceProofHashError.invalidRlp(field)
    }
    let hex = String(text.dropFirst(2))
    guard (allowEmpty || !hex.isEmpty),
          hex.count % 2 == 0,
          hex.allSatisfy({ sourceProofIsLowerHex($0) }) else {
        throw SccpSourceProofHashError.invalidRlp(field)
    }
    if let byteLength, hex.count != byteLength * 2 {
        throw SccpSourceProofHashError.invalidRlp(field)
    }
    let decoded = hex.isEmpty ? Data() : Data(hexString: hex)
    guard let bytes = decoded, bytes.count == hex.count / 2 else {
        throw SccpSourceProofHashError.invalidRlp(field)
    }
    if nonzero, !bytes.contains(where: { $0 != 0 }) {
        throw SccpSourceProofHashError.invalidRlp(field)
    }
    return bytes
}

private func sourceProofIsLowerHex(_ character: Character) -> Bool {
    "0123456789abcdef".contains(character)
}

private func sourceProofIsDecimalDigit(_ character: Character) -> Bool {
    "0123456789".contains(character)
}

private func sourceProofMinimalBigEndianBytes(_ value: UInt64) -> Data {
    if value == 0 {
        return Data()
    }
    var working = value
    var bytes: [UInt8] = []
    while working > 0 {
        bytes.insert(UInt8(working & 0xff), at: 0)
        working >>= 8
    }
    return Data(bytes)
}

private enum SourceProofRlpItem {
    case bytes(Data)
    case list(Data)
}

private func sourceProofRlpLengthPrefix(_ length: Int, shortOffset: UInt8, longOffset: UInt8) -> Data {
    if length < 56 {
        return Data([shortOffset + UInt8(length)])
    }
    var remaining = length
    var lengthBytes: [UInt8] = []
    while remaining > 0 {
        lengthBytes.insert(UInt8(remaining & 0xff), at: 0)
        remaining >>= 8
    }
    var out = Data([longOffset + UInt8(lengthBytes.count)])
    out.append(contentsOf: lengthBytes)
    return out
}

private func sourceProofRlpString(_ value: Data) -> Data {
    if value.count == 1, let first = value.first, first < 0x80 {
        return value
    }
    var out = sourceProofRlpLengthPrefix(value.count, shortOffset: 0x80, longOffset: 0xb7)
    out.append(value)
    return out
}

private func sourceProofRlpList(_ fields: [Data]) -> Data {
    var payload = Data()
    for field in fields {
        payload.append(field)
    }
    var out = sourceProofRlpLengthPrefix(payload.count, shortOffset: 0xc0, longOffset: 0xf7)
    out.append(payload)
    return out
}

private func sourceProofReadRlpLength(_ bytes: [UInt8], offset: Int, lengthOfLength: Int) throws -> Int {
    guard lengthOfLength > 0, lengthOfLength <= 8, offset + lengthOfLength <= bytes.count else {
        throw SccpSourceProofHashError.invalidRlp("length")
    }
    guard bytes[offset] != 0 else {
        throw SccpSourceProofHashError.invalidRlp("length")
    }
    var length = 0
    for index in 0..<lengthOfLength {
        length = length * 256 + Int(bytes[offset + index])
    }
    return length
}

private func sourceProofRlpItemAt(_ bytes: [UInt8], cursor: Int) throws -> (SourceProofRlpItem, Int) {
    guard cursor < bytes.count else {
        throw SccpSourceProofHashError.invalidRlp("cursor")
    }
    let first = bytes[cursor]
    if first <= 0x7f {
        return (.bytes(Data([first])), cursor + 1)
    }
    if first <= 0xb7 {
        let length = Int(first - 0x80)
        let start = cursor + 1
        let end = start + length
        guard end <= bytes.count, !(length == 1 && bytes[start] < 0x80) else {
            throw SccpSourceProofHashError.invalidRlp("string")
        }
        return (.bytes(Data(bytes[start..<end])), end)
    }
    if first <= 0xbf {
        let lengthOfLength = Int(first - 0xb7)
        let length = try sourceProofReadRlpLength(bytes, offset: cursor + 1, lengthOfLength: lengthOfLength)
        guard length >= 56 else {
            throw SccpSourceProofHashError.invalidRlp("string")
        }
        let start = cursor + 1 + lengthOfLength
        let end = start + length
        guard end <= bytes.count else {
            throw SccpSourceProofHashError.invalidRlp("string")
        }
        return (.bytes(Data(bytes[start..<end])), end)
    }
    if first <= 0xf7 {
        let length = Int(first - 0xc0)
        let start = cursor + 1
        let end = start + length
        guard end <= bytes.count else {
            throw SccpSourceProofHashError.invalidRlp("list")
        }
        return (.list(Data(bytes[start..<end])), end)
    }
    let lengthOfLength = Int(first - 0xf7)
    let length = try sourceProofReadRlpLength(bytes, offset: cursor + 1, lengthOfLength: lengthOfLength)
    guard length >= 56 else {
        throw SccpSourceProofHashError.invalidRlp("list")
    }
    let start = cursor + 1 + lengthOfLength
    let end = start + length
    guard end <= bytes.count else {
        throw SccpSourceProofHashError.invalidRlp("list")
    }
    return (.list(Data(bytes[start..<end])), end)
}

private func sourceProofRlpListByteFields(_ data: Data) throws -> [Data] {
    let bytes = [UInt8](data)
    let (outer, cursor) = try sourceProofRlpItemAt(bytes, cursor: 0)
    guard cursor == bytes.count else {
        throw SccpSourceProofHashError.invalidRlp("headerRlp")
    }
    guard case let .list(listPayload) = outer else {
        throw SccpSourceProofHashError.invalidRlp("headerRlp")
    }
    let listBytes = [UInt8](listPayload)
    var fields: [Data] = []
    var innerCursor = 0
    while innerCursor < listBytes.count {
        let (item, nextCursor) = try sourceProofRlpItemAt(listBytes, cursor: innerCursor)
        guard case let .bytes(field) = item else {
            throw SccpSourceProofHashError.invalidRlp("headerRlp")
        }
        fields.append(field)
        innerCursor = nextCursor
    }
    return fields
}

private func sourceProofSszHashNode(_ left: Data, _ right: Data) -> Data {
    var preimage = Data()
    preimage.append(left)
    preimage.append(right)
    return Data(SHA256.hash(data: preimage))
}

private func sourceProofSszMerkleizeChunks(_ inputChunks: [Data]) throws -> Data {
    if inputChunks.isEmpty {
        return Data(repeating: 0, count: 32)
    }
    var chunks = inputChunks
    for chunk in chunks where chunk.count != 32 {
        throw SccpSourceProofHashError.invalidValidatorSet("sszChunk")
    }
    var paddedLength = 1
    while paddedLength < chunks.count {
        paddedLength *= 2
    }
    while chunks.count < paddedLength {
        chunks.append(Data(repeating: 0, count: 32))
    }
    while chunks.count > 1 {
        var next: [Data] = []
        next.reserveCapacity(chunks.count / 2)
        for index in stride(from: 0, to: chunks.count, by: 2) {
            next.append(sourceProofSszHashNode(chunks[index], chunks[index + 1]))
        }
        chunks = next
    }
    return chunks[0]
}

private func sourceProofReadMinimalBeU64(_ bytes: Data, field: String) throws -> UInt64 {
    if bytes.isEmpty {
        return 0
    }
    guard bytes.count <= 8, !(bytes.count > 1 && bytes[0] == 0) else {
        throw SccpSourceProofHashError.invalidRlp(field)
    }
    var out: UInt64 = 0
    for byte in bytes {
        out = (out << 8) | UInt64(byte)
    }
    return out
}

private func sourceProofSszU64Chunk(_ value: UInt64) -> Data {
    var out = Data(repeating: 0, count: 32)
    for index in 0..<8 {
        out[index] = UInt8((value >> UInt64(index * 8)) & 0xff)
    }
    return out
}

private func sourceProofSszU64ChunkFromRlp(_ bytes: Data, field: String) throws -> Data {
    try sourceProofSszU64Chunk(sourceProofReadMinimalBeU64(bytes, field: field))
}

private func sourceProofSszU256ChunkFromRlp(_ bytes: Data, field: String) throws -> Data {
    guard bytes.count <= 32, !(bytes.count > 1 && bytes[0] == 0) else {
        throw SccpSourceProofHashError.invalidRlp(field)
    }
    var out = Data(repeating: 0, count: 32)
    for index in 0..<bytes.count {
        out[index] = bytes[bytes.count - 1 - index]
    }
    return out
}

private func sourceProofSszByteVectorRoot(_ bytes: Data, expectedLength: Int, field: String) throws -> Data {
    guard bytes.count == expectedLength else {
        throw SccpSourceProofHashError.invalidValidatorSet(field)
    }
    var chunks: [Data] = []
    for offset in stride(from: 0, to: bytes.count, by: 32) {
        var chunk = Data(repeating: 0, count: 32)
        let end = min(offset + 32, bytes.count)
        chunk.replaceSubrange(0..<(end - offset), with: bytes[offset..<end])
        chunks.append(chunk)
    }
    return try sourceProofSszMerkleizeChunks(chunks)
}

private func sourceProofSszMixInLength(root: Data, length: Int) -> Data {
    sourceProofSszHashNode(root, sourceProofSszU64Chunk(UInt64(length)))
}

private func sourceProofSszByteListRoot(_ bytes: Data, maxLength: Int, field: String) throws -> Data {
    guard bytes.count <= maxLength else {
        throw SccpSourceProofHashError.invalidValidatorSet(field)
    }
    let limitChunks = max(1, (maxLength + 31) / 32)
    var chunks: [Data] = []
    for offset in stride(from: 0, to: bytes.count, by: 32) {
        var chunk = Data(repeating: 0, count: 32)
        let end = min(offset + 32, bytes.count)
        chunk.replaceSubrange(0..<(end - offset), with: bytes[offset..<end])
        chunks.append(chunk)
    }
    while chunks.count < limitChunks {
        chunks.append(Data(repeating: 0, count: 32))
    }
    return try sourceProofSszMixInLength(root: sourceProofSszMerkleizeChunks(chunks), length: bytes.count)
}

private func sourceProofSszMerkleRootFromBranch(leaf: Data,
                                                leafIndex: UInt64,
                                                branch: [Data],
                                                field: String) throws -> Data {
    guard leaf.count == 32 else {
        throw SccpSourceProofHashError.invalidValidatorSet(field)
    }
    var current = leaf
    var index = leafIndex
    for (branchIndex, sibling) in branch.enumerated() {
        guard sibling.count == 32 else {
            throw SccpSourceProofHashError.invalidBranch("\(field)[\(branchIndex)]")
        }
        current = (index & 1) == 1
            ? sourceProofSszHashNode(sibling, current)
            : sourceProofSszHashNode(current, sibling)
        index >>= 1
    }
    return current
}

private func sourceProofBytesFromHex32(_ value: String, field: String) throws -> Data {
    do {
        return try sourceProofBytesFromHex(value, field: field, byteLength: 32)
    } catch {
        throw SccpSourceProofHashError.invalidHex32(field)
    }
}

private func sourceProofNonZeroBytesFromHex32(_ value: String, field: String) throws -> Data {
    let bytes = try sourceProofBytesFromHex32(value, field: field)
    guard bytes.contains(where: { $0 != 0 }) else {
        throw SccpSourceProofHashError.invalidHex32(field)
    }
    return bytes
}

private func sourceProofBytesFromHex20(_ value: String, field: String) throws -> Data {
    do {
        return try sourceProofBytesFromHex(value, field: field, byteLength: 20)
    } catch {
        throw SccpSourceProofHashError.invalidHex20(field)
    }
}

private func sourceProofNonZeroBytesFromHex20(_ value: String, field: String) throws -> Data {
    let bytes = try sourceProofBytesFromHex20(value, field: field)
    guard bytes.contains(where: { $0 != 0 }) else {
        throw SccpSourceProofHashError.invalidHex20(field)
    }
    return bytes
}

private func sourceProofBytesFromHex21(_ value: String, field: String) throws -> Data {
    do {
        return try sourceProofBytesFromHex(value, field: field, byteLength: 21)
    } catch {
        throw SccpSourceProofHashError.invalidValidatorSet(field)
    }
}

private func sourceProofBytesFromHex(_ value: String, field: String, byteLength: Int) throws -> Data {
    guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
        throw SccpSourceProofHashError.invalidValidatorSet(field)
    }
    var hex = value
    if hex.lowercased().hasPrefix("0x") {
        hex.removeFirst(2)
    }
    hex = hex.lowercased()
    guard hex.count == byteLength * 2, let bytes = Data(hexString: hex), bytes.count == byteLength else {
        throw SccpSourceProofHashError.invalidValidatorSet(field)
    }
    return bytes
}

private func sourceProofNormalizeHex32(_ value: String, field: String) throws -> String {
    "0x" + (try sourceProofBytesFromHex32(value, field: field)).hexEncodedString()
}

private func sourceProofAppendU32Le(_ value: UInt32, to out: inout Data) {
    out.append(UInt8(value & 0xff))
    out.append(UInt8((value >> 8) & 0xff))
    out.append(UInt8((value >> 16) & 0xff))
    out.append(UInt8((value >> 24) & 0xff))
}

private func sourceProofAppendAbiU32(_ value: UInt32, to out: inout Data) {
    out.append(contentsOf: repeatElement(UInt8(0), count: 28))
    out.append(UInt8((value >> 24) & 0xff))
    out.append(UInt8((value >> 16) & 0xff))
    out.append(UInt8((value >> 8) & 0xff))
    out.append(UInt8(value & 0xff))
}

private func sourceProofAppendAbiAddress20(_ value: Data, to out: inout Data) {
    precondition(value.count == 20)
    out.append(contentsOf: repeatElement(UInt8(0), count: 12))
    out.append(value)
}

private func sourceProofAppendAbiBytes21(_ value: Data, to out: inout Data) {
    precondition(value.count == 21)
    out.append(contentsOf: repeatElement(UInt8(0), count: 11))
    out.append(value)
}

private let sourceProofBase58Alphabet = Array(
    "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".utf8
)
private let sourceProofBase58Index: [UInt8: UInt8] = {
    var index: [UInt8: UInt8] = [:]
    for (offset, byte) in sourceProofBase58Alphabet.enumerated() {
        index[byte] = UInt8(offset)
    }
    return index
}()

private func sourceProofBase58Decode(_ value: String, field: String) throws -> Data {
    guard !value.isEmpty else {
        throw SccpSourceProofHashError.invalidSourceMaterial(field)
    }
    guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
        throw SccpSourceProofHashError.invalidSourceMaterial(field)
    }
    var bytes: [UInt8] = []
    for byte in value.utf8 {
        guard let digit = sourceProofBase58Index[byte] else {
            throw SccpSourceProofHashError.invalidSourceMaterial(field)
        }
        var carry = Int(digit)
        if !bytes.isEmpty {
            for index in stride(from: bytes.count - 1, through: 0, by: -1) {
                carry += Int(bytes[index]) * 58
                bytes[index] = UInt8(carry & 0xff)
                carry >>= 8
            }
        }
        while carry > 0 {
            bytes.insert(UInt8(carry & 0xff), at: 0)
            carry >>= 8
        }
    }
    let leadingZeroCount = value.utf8.prefix { $0 == UInt8(ascii: "1") }.count
    return Data(Array(repeating: UInt8(0), count: leadingZeroCount) + bytes)
}

func sourceProofTronBase58CheckPayload(_ value: String, field: String) throws -> Data {
    let decoded = try sourceProofBase58Decode(value, field: field)
    guard decoded.count == 25 else {
        throw SccpSourceProofHashError.invalidSourceMaterial(field)
    }
    let payload = Data(decoded.prefix(21))
    let checksum = Data(decoded.suffix(4))
    let first = Data(SHA256.hash(data: payload))
    let expectedChecksum = Data(Data(SHA256.hash(data: first)).prefix(4))
    guard checksum == expectedChecksum,
          payload.first == 0x41,
          payload.dropFirst().contains(where: { $0 != 0 }) else {
        throw SccpSourceProofHashError.invalidSourceMaterial(field)
    }
    return payload
}

private func sourceProofAppendU64Le(_ value: UInt64, to out: inout Data) {
    for shift in stride(from: 0, through: 56, by: 8) {
        out.append(UInt8((value >> UInt64(shift)) & 0xff))
    }
}

private func sourceProofAppendU64Be(_ value: UInt64, to out: inout Data) {
    for shift in stride(from: 56, through: 0, by: -8) {
        out.append(UInt8((value >> UInt64(shift)) & 0xff))
    }
}

private func sourceProofReadU32Le(_ bytes: Data) -> UInt32 {
    var value: UInt32 = 0
    for (index, byte) in bytes.enumerated() {
        value |= UInt32(byte) << UInt32(index * 8)
    }
    return value
}

private func sourceProofReadU64Le(_ bytes: Data) -> UInt64 {
    var value: UInt64 = 0
    for (index, byte) in bytes.enumerated() {
        value |= UInt64(byte) << UInt64(index * 8)
    }
    return value
}

private func sourceProofAppendProtobufVarint(_ value: UInt64, to out: inout Data) {
    var working = value
    repeat {
        var byte = UInt8(working & 0x7f)
        working >>= 7
        if working != 0 {
            byte |= 0x80
        }
        out.append(byte)
    } while working != 0
}

private func sourceProofAppendProtobufU64(fieldNumber: UInt64, value: UInt64, to out: inout Data) {
    sourceProofAppendProtobufVarint((fieldNumber << 3) | 0, to: &out)
    sourceProofAppendProtobufVarint(value, to: &out)
}

private func sourceProofAppendProtobufBytes(fieldNumber: UInt64, value: Data, to out: inout Data) {
    sourceProofAppendProtobufVarint((fieldNumber << 3) | 2, to: &out)
    sourceProofAppendProtobufVarint(UInt64(value.count), to: &out)
    out.append(value)
}

private func sourceProofHashHex(prefix: String, payload: Data) throws -> String {
    var preimage = Data(prefix.utf8)
    preimage.append(payload)
    return "0x" + Blake2b.hash256(preimage).hexEncodedString()
}

private func sourceProofKeccakHashHex(prefix: String, payload: Data) -> String {
    var preimage = Data(prefix.utf8)
    preimage.append(payload)
    return "0x" + irohaKeccak256(preimage).hexEncodedString()
}
