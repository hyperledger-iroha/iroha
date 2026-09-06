import "@stdlib/strings"
import "constants"
import "errors"
import "proof-verifier"
import "replay-forest"
import "sccp-codec"

/// Immutable route hashes. Every role must be nonzero and distinct.
struct RouteHashes {
    sourceLaneHash: uint256
    destinationLaneHash: uint256
    routeConfigurationHash: uint256
    tail: Cell<RouteHashesTail>
}

struct RouteHashesTail {
    destinationBindingHash: uint256
    semanticProofProfileHash: uint256
    deploymentCodeHashes: Cell<DeploymentCodeHashes>
}

struct FinalityHashes {
    soraFinalityAnchorHash: uint256
    verifierCircuitHash: uint256
    verifyingKeyHash: uint256
    tail: Cell<FinalityHashesTail>
}

struct FinalityHashesTail {
    proofProfileCommitment: uint256
    /// Exactly five nonzero, strictly increasing big-endian keys. Placement
    /// immediately after the proof-profile commitment matches both governed
    /// TON binding preimages.
    mintBreakerGuardians: Cell<MintBreakerGuardians>
    embeddedVerifierCodeHash: uint256
    /// TON representation hash of the exact immutable key cell.
    verifyingKeyCellHash: uint256
    verifyingKey: cell
    /// Exact TEP-64 metadata committed into the canonical master StateInit.
    masterMetadata: cell
}

/// Tail split keeps the five fixed guardian keys in ordinary cells.
struct MintBreakerGuardiansTail {
    guardian3: uint256
    guardian4: uint256
}

/// Canonically ordered Ed25519 public keys for the disable-only controller.
struct MintBreakerGuardians {
    guardian0: uint256
    guardian1: uint256
    guardian2: uint256
    tail: Cell<MintBreakerGuardiansTail>
}

/// Exact deployed code identities committed by both TON route hashes.
struct DeploymentCodeHashes {
    jettonMasterCodeHash: uint256
    jettonWalletCodeHash: uint256
    routeCodeHash: uint256
}

/// Shared immutable bridge configuration committed by master and bridge StateInit.
struct BridgeConfig {
    expectedGlobalId: int32
    routeRevision: uint32
    tairaToTonMultiplier: uint64
    /// Positive immutable ceiling shared with the governed SORA route.
    maxWrappedSupply: coins
    sourceLaneBytes: cell
    destinationLaneBytes: cell
    routeHashes: Cell<RouteHashes>
    finalityHashes: Cell<FinalityHashes>
}

struct PendingMint {
    queryId: uint64
    recipient: address
    jettonAmount: coins
    payloadHash: uint256
    payloadSha256: uint256
}

struct PendingBurn {
    lockSequence: uint64
    queryId: uint64
    outboundNonce: uint64
    sourceWallet: address
    ownerAddress: address
    jettonAmount: coins
    tail: Cell<PendingBurnTail>
}

struct PendingBurnTail {
    payloadHash: uint256
    sourceEventDigest: uint256
    canonicalPayload: cell
}

struct BridgeReplayState {
    inboundMint: Cell<SccpReplayForestV1>
    outboundBurn: Cell<SccpReplayForestV1>
}

struct BridgePendingState {
    mints: map<uint256, Cell<PendingMint>>
    burns: map<uint256, Cell<PendingBurn>>
    mintCount: uint16
    burnCount: uint16
}

struct BridgeStorage {
    // Source-proof parsers authenticate these first 264 data-root bits directly.
    storageVersion: uint8
    routeConfigurationHash: uint256
    /// Exact immutable route/configuration cell committed by StateInit.
    bridgeConfigCellHash: uint256
    config: Cell<BridgeConfig>
    replay: Cell<BridgeReplayState>
    pending: Cell<BridgePendingState>
    /// One-way breaker state. Canonical deployment always starts enabled.
    mintingDisabled: bool
}

struct MasterMintRecord {
    queryId: uint64
    bridgeAddress: address
    recipient: address
    jettonAmount: coins
    tail: Cell<MasterMintRecordTail>
}

struct MasterMintRecordTail {
    payloadHash: uint256
    payloadSha256: uint256
}

struct MasterReplayState {
    mints: Cell<SccpReplayForestV1>
    burns: Cell<SccpReplayForestV1>
}

struct MasterStorage {
    storageVersion: uint8
    routeConfigurationHash: uint256
    /// Exact immutable route/configuration cell committed by StateInit.
    bridgeConfigCellHash: uint256
    totalSupply: coins
    metadata: cell
    bridgeConfig: Cell<BridgeConfig>
    /// Exact bridge address committed into this master's StateInit.
    bridgeAddress: address
    replay: Cell<MasterReplayState>
    pendingMints: map<uint256, Cell<MasterMintRecord>>
    /// Number of master-to-wallet mints awaiting an authenticated ack.
    pendingMintCount: uint16
    /// One-way breaker state. Canonical deployment always starts enabled.
    mintingDisabled: bool
}

/// Exact debit record retained by a wallet while an SCCP burn can be retried.
struct WalletBurnRecord {
    lockSequence: uint64
    queryId: uint64
    outboundNonce: uint64
    bridgeAddress: address
    ownerAddress: address
    jettonAmount: coins
    tail: Cell<WalletBurnRecordTail>
}

struct WalletBurnRecordTail {
    payloadHash: uint256
    payloadSha256: uint256
    sourceEventDigest: uint256
}

struct WalletReplayState {
    mintCredits: Cell<SccpReplayForestV1>
    /// 0x35: bridge-authorized irreversible burn transition.
    burnAuthorizations: Cell<SccpReplayForestV1>
    /// 0x36: owner-authenticated transient burn-lock debit.
    burnLocks: Cell<SccpReplayForestV1>
    /// 0x37: rejected-lock recredit.
    burnRefunds: Cell<SccpReplayForestV1>
}

const SCCP_WALLET_LOCK_PREPARING = 1
const SCCP_WALLET_LOCK_REFUNDABLE = 2

/// Immutable economics committed by an owner-authenticated outbound lock.
struct WalletBurnLockSemantic {
    queryId: uint64
    outboundNonce: uint64
    jettonAmount: coins
    tairaRecipient: cell
    canonicalPayload: cell
    payloadHash: uint256
    payloadSha256: uint256
}

/// Mutable delivery state is kept outside [WalletBurnLockSemantic]. Witnesses
/// are supplied afresh on every prepare/refund retry and are never semantic.
struct WalletBurnLock {
    status: uint8
    attempt: uint32
    semantic: Cell<WalletBurnLockSemantic>
}

struct WalletPendingState {
    burns: map<uint256, Cell<WalletBurnRecord>>
    burnCount: uint16
}

struct WalletStorageTail {
    bridgeAddress: address
    routeRevision: uint32
    routeConfigurationHash: uint256
    maxWrappedSupply: coins
    outboundNonce: uint64
    nextLockSequence: uint64
    lockedBurns: map<uint64, Cell<WalletBurnLock>>
    lockedBurnCount: uint16
}

struct WalletStorage {
    jettonBalance: coins
    ownerAddress: address
    masterAddress: address
    replay: Cell<WalletReplayState>
    pending: Cell<WalletPendingState>
    tail: Cell<WalletStorageTail>
}

struct JettonWalletDataReply {
    jettonBalance: coins
    ownerAddress: address
    masterAddress: address
    jettonWalletCode: cell
}

struct JettonDataReply {
    totalSupply: int
    mintable: bool
    adminAddress: address?
    jettonContent: Cell<OnchainMetadataReply>
    jettonWalletCode: cell
}

struct (0x00) OnchainMetadataReply {
    contentDict: map<uint256, string_prefixed0x>
}

fun storageGuardiansAreCanonical(guardians: MintBreakerGuardians): bool {
    val tail = guardians.tail.load();
    return guardians.guardian0 != 0 && guardians.guardian0 < guardians.guardian1 &&
    guardians.guardian1 <
    guardians.guardian2 &&
    guardians.guardian2 <
    tail.guardian3 &&
    tail.guardian3 <
    tail.guardian4;
}

/// Final-V1 TON destination binding. Guardian keys are concatenated in strict
/// key order immediately after `proofProfileCommitment`, before the semantic
/// and finality hashes, matching the governed registry byte preimage.
fun sccpTonDestinationBindingHashV1(config: BridgeConfig): uint256 {
    val hashes = config.routeHashes.load();
    val hashTail = hashes.tail.load();
    val codes = hashTail.deploymentCodeHashes.load();
    val semanticProfileHash = hashTail.semanticProofProfileHash;
    val finality = config.finalityHashes.load();
    val finalityTail = finality.tail.load();
    val guardians = finalityTail.mintBreakerGuardians.load();
    val guardianTail = guardians.tail.load();
    val backend = "ton-groth16-bls12381-v1".literalSlice();
    val network = canonicalSccpTonMainnetNetworkBytesV1().beginParse();
    val prefix = beginCell()
        .storeSlice("iroha:sccp:ton-destination-binding:v1".literalSlice())
        .storeUint(1, 8)
        .storeUint32Le(backend.remainingBitsCount() / 8)
        .storeSlice(backend)
        .storeUint32Le(network.remainingBitsCount() / 8)
        .endCell();
    val domains = beginCell()
        .storeUint32Le(0xffffff11)
        .storeUint32Le(SCCP_DOMAIN_SORA)
        .storeUint32Le(SCCP_DOMAIN_TON)
        .endCell();
    val deployedCodes = beginCell()
        .storeUint(codes.jettonMasterCodeHash, 256)
        .storeUint(codes.jettonWalletCodeHash, 256)
        .storeUint(codes.routeCodeHash, 256)
        .endCell();
    val verifierHashes = beginCell()
        .storeUint(finalityTail.embeddedVerifierCodeHash, 256)
        .storeUint(finality.verifierCircuitHash, 256)
        .storeUint(finality.verifyingKeyHash, 256)
        .endCell();
    val proofAndGuardians01 = beginCell()
        .storeUint(finalityTail.proofProfileCommitment, 256)
        .storeUint(guardians.guardian0, 256)
        .storeUint(guardians.guardian1, 256)
        .endCell();
    val guardians234 = beginCell()
        .storeUint(guardians.guardian2, 256)
        .storeUint(guardianTail.guardian3, 256)
        .storeUint(guardianTail.guardian4, 256)
        .endCell();
    val semanticAndFinality = beginCell()
        .storeUint(semanticProfileHash, 256)
        .storeUint(finality.soraFinalityAnchorHash, 256)
        .endCell();
    return sccpReplaySha256Eight(
        prefix.beginParse(),
        network,
        domains.beginParse(),
        deployedCodes.beginParse(),
        verifierHashes.beginParse(),
        proofAndGuardians01.beginParse(),
        guardians234.beginParse(),
        semanticAndFinality.beginParse(),
    );
}

fun sccpTonDeploymentConfigHashV1(
    config: BridgeConfig,
    destinationBindingHash: uint256,
): uint256 {
    val hashes = config.routeHashes.load();
    val hashTail = hashes.tail.load();
    val codes = hashTail.deploymentCodeHashes.load();
    val semanticProfileHash = hashTail.semanticProofProfileHash;
    val finality = config.finalityHashes.load();
    val finalityTail = finality.tail.load();
    val guardians = finalityTail.mintBreakerGuardians.load();
    val guardianTail = guardians.tail.load();
    val deployedCodes = beginCell()
        .storeUint(codes.jettonMasterCodeHash, 256)
        .storeUint(codes.jettonWalletCodeHash, 256)
        .storeUint(codes.routeCodeHash, 256)
        .endCell();
    val verifierHashes = beginCell()
        .storeUint(finalityTail.embeddedVerifierCodeHash, 256)
        .storeUint(finality.verifierCircuitHash, 256)
        .storeUint(finality.verifyingKeyHash, 256)
        .endCell();
    val proofAndGuardians01 = beginCell()
        .storeUint(finalityTail.proofProfileCommitment, 256)
        .storeUint(guardians.guardian0, 256)
        .storeUint(guardians.guardian1, 256)
        .endCell();
    val guardians234 = beginCell()
        .storeUint(guardians.guardian2, 256)
        .storeUint(guardianTail.guardian3, 256)
        .storeUint(guardianTail.guardian4, 256)
        .endCell();
    val policy = beginCell()
        .storeUint(semanticProfileHash, 256)
        .storeUint(finality.soraFinalityAnchorHash, 256)
        .storeUint(destinationBindingHash, 256)
        .endCell();
    return sccpReplaySha256Five(
        deployedCodes.beginParse(),
        verifierHashes.beginParse(),
        proofAndGuardians01.beginParse(),
        guardians234.beginParse(),
        policy.beginParse(),
    );
}

fun sccpTonAssetRouteConfigHashV1(config: BridgeConfig): uint256 {
    val asset = "xor".literalSlice();
    val route = "taira_ton_xor".literalSlice();
    val preimage = beginCell()
        .storeUint32Le(asset.remainingBitsCount() / 8)
        .storeSlice(asset)
        .storeUint32Le(route.remainingBitsCount() / 8)
        .storeSlice(route)
        .storeUint32Le(config.routeRevision)
        .storeUint64Le(config.tairaToTonMultiplier)
        .storeUint128Le(config.maxWrappedSupply as uint128)
        .endCell();
    return sccpReplaySha256One(preimage.beginParse());
}

fun sccpExactTonRouteConfigurationHashV1(config: BridgeConfig): uint256 {
    val hashes = config.routeHashes.load();
    val destinationBindingHash = sccpTonDestinationBindingHashV1(config);
    val deploymentConfigHash = sccpTonDeploymentConfigHashV1(
        config,
        destinationBindingHash,
    );
    val network = canonicalSccpTonMainnetNetworkBytesV1().beginParse();
    val prefix = beginCell()
        .storeSlice("sccp:concrete-route-config:v1".literalSlice())
        .storeUint(1, 8)
        .storeUint32Le(SCCP_DOMAIN_TON)
        .storeUint32Le(network.remainingBitsCount() / 8)
        .endCell();
    val route = beginCell()
        .storeUint32Le(0xffffff11)
        .storeUint(hashes.sourceLaneHash, 256)
        .storeUint(hashes.destinationLaneHash, 256)
        .storeUint(deploymentConfigHash, 256)
        .endCell();
    val assetRouteConfigHash = beginCell()
        .storeUint(sccpTonAssetRouteConfigHashV1(config), 256)
        .endCell();
    return sccpReplaySha256Four(
        prefix.beginParse(),
        network,
        route.beginParse(),
        assetRouteConfigHash.beginParse(),
    );
}

/// Validate every StateInit-bound route role before constructing a deployable
/// bridge, master, or wallet address.
fun assertCanonicalBridgeConfigV1(config: BridgeConfig) {
    assertCanonicalBridgeConfigRuntimeV1(config);
    assertCanonicalSourceLaneRuntimeV1(config);
    assertCanonicalDestinationLaneRuntimeV1(config);
    val finality = config.finalityHashes.load();
    val finalityTail = finality.tail.load();
    val hashes = config.routeHashes.load();
    assert (
        finality.verifyingKeyHash == canonicalBls12381VerifyingKeyHash(finalityTail.verifyingKey) &&
        hashes.sourceLaneHash == sccpLaneHash(config.sourceLaneBytes) &&
        hashes.destinationLaneHash == sccpLaneHash(config.destinationLaneBytes) &&
        hashes.tail.load().destinationBindingHash == sccpTonDestinationBindingHashV1(config) &&
        hashes.routeConfigurationHash == sccpExactTonRouteConfigurationHashV1(config)
    ) throw SccpErrors.InvalidConfiguration;
}

/// Cheap runtime validation for every handler. The full canonical BLS key-byte
/// commitment is rechecked by the proof verifier on mint admission and by the
/// unique StateInit constructors; burn/ack traffic must not pay that pairing-
/// key traversal again.
fun assertCanonicalBridgeConfigRuntimeV1(config: BridgeConfig) {
    val hashes = config.routeHashes.load();
    val hashTail = hashes.tail.load();
    val finality = config.finalityHashes.load();
    val finalityTail = finality.tail.load();
    val guardians = finalityTail.mintBreakerGuardians.load();
    val codes = hashTail.deploymentCodeHashes.load();
    assert (
        config.expectedGlobalId == SCCP_TON_MAINNET_GLOBAL_ID && config.routeRevision != 0 &&
        config.tairaToTonMultiplier == SCCP_TAIRA_TO_TON_MULTIPLIER &&
        config.maxWrappedSupply > 0 &&
        config.maxWrappedSupply <= SCCP_MAX_JETTON_AMOUNT &&
        storageGuardiansAreCanonical(guardians) &&
        finalityTail.verifyingKey.hash() == finalityTail.verifyingKeyCellHash
    ) throw SccpErrors.InvalidConfiguration;
    assert (
        hashes.sourceLaneHash != 0 && hashes.destinationLaneHash != 0 &&
        hashes.routeConfigurationHash != 0 &&
        hashTail.destinationBindingHash != 0 &&
        hashTail.semanticProofProfileHash != 0 &&
        finality.soraFinalityAnchorHash != 0 &&
        finality.verifierCircuitHash != 0 &&
        finality.verifyingKeyHash != 0 &&
        finalityTail.proofProfileCommitment != 0 &&
        finalityTail.embeddedVerifierCodeHash != 0 &&
        finalityTail.verifyingKeyCellHash != 0 &&
        codes.jettonMasterCodeHash != 0 &&
        codes.jettonWalletCodeHash != 0 &&
        codes.routeCodeHash != 0
    ) throw SccpErrors.InvalidConfiguration;
    assert (
        hashes.sourceLaneHash != hashes.destinationLaneHash &&
        hashes.sourceLaneHash != hashes.routeConfigurationHash &&
        hashes.destinationLaneHash != hashes.routeConfigurationHash &&
        hashTail.destinationBindingHash != hashTail.semanticProofProfileHash &&
        finality.soraFinalityAnchorHash != finality.verifierCircuitHash &&
        finalityTail.proofProfileCommitment != finalityTail.embeddedVerifierCodeHash &&
        codes.jettonMasterCodeHash != codes.jettonWalletCodeHash &&
        codes.jettonMasterCodeHash != codes.routeCodeHash &&
        codes.jettonWalletCodeHash != codes.routeCodeHash
    ) throw SccpErrors.HashRoleAlias;
}

fun assertCanonicalSourceLaneRuntimeV1(config: BridgeConfig) {
    assert (
        config.sourceLaneBytes.hash() == canonicalSccpTonToSoraLaneBytesV1().hash() &&
        config.routeHashes.load().sourceLaneHash == SCCP_TON_TO_SORA_LANE_HASH_V1
    ) throw SccpErrors.InvalidConfiguration;
}

fun assertCanonicalDestinationLaneRuntimeV1(config: BridgeConfig) {
    assert (
        config.destinationLaneBytes.hash() == canonicalSccpSoraToTonLaneBytesV1().hash() &&
        config.routeHashes.load().destinationLaneHash == SCCP_SORA_TO_TON_LANE_HASH_V1
    ) throw SccpErrors.InvalidConfiguration;
}

/// Empty bridge replay state used by the only canonical V1 deployment data.
fun emptyBridgeReplayState(): Cell<BridgeReplayState> {
    return BridgeReplayState {
        inboundMint: emptySccpReplayForestV1(),
        outboundBurn: emptySccpReplayForestV1(),
    }.toCell();
}

/// Empty bridge operation state used by the only canonical V1 deployment data.
fun emptyBridgePendingState(): Cell<BridgePendingState> {
    return BridgePendingState {
        mints: [],
        burns: [],
        mintCount: 0,
        burnCount: 0,
    }.toCell();
}

/// Empty Jetton-master replay state used by the only canonical V1 deployment data.
fun emptyMasterReplayState(): Cell<MasterReplayState> {
    return MasterReplayState {
        mints: emptySccpReplayForestV1(),
        burns: emptySccpReplayForestV1(),
    }.toCell();
}

fun emptyWalletReplayState(): Cell<WalletReplayState> {
    return WalletReplayState {
        mintCredits: emptySccpReplayForestV1(),
        burnAuthorizations: emptySccpReplayForestV1(),
        burnLocks: emptySccpReplayForestV1(),
        burnRefunds: emptySccpReplayForestV1(),
    }.toCell();
}

fun emptyWalletPendingState(): Cell<WalletPendingState> {
    return WalletPendingState {
        burns: [],
        burnCount: 0,
    }.toCell();
}

/// Build the canonical bridge StateInit data for a governed route.
///
/// Governance commits the resulting data-cell hash. Keeping every mutable
/// counter and dictionary fixed at zero/empty prevents a deployment from
/// preoccupying replay slots or exhausting pending capacity before the first
/// authenticated message.
fun canonicalBridgeInitialStorage(
    config: Cell<BridgeConfig>,
): BridgeStorage {
    val loadedConfig = config.load();
    assertCanonicalBridgeConfigV1(loadedConfig);
    val routeConfigurationHash = loadedConfig.routeHashes.load().routeConfigurationHash;
    return {
        storageVersion: SCCP_STORAGE_VERSION,
        routeConfigurationHash,
        bridgeConfigCellHash: config.hash(),
        config,
        replay: emptyBridgeReplayState(),
        pending: emptyBridgePendingState(),
        mintingDisabled: false,
    };
}

/// Build the canonical Jetton-master StateInit data for a governed route.
///
/// Governance commits the resulting data-cell hash. Supply and all replay
/// dictionaries therefore start from a unique zero state.
fun canonicalMasterInitialStorage(
    bridgeConfig: Cell<BridgeConfig>,
    bridgeAddress: address,
): MasterStorage {
    val loadedConfig = bridgeConfig.load();
    assertCanonicalBridgeConfigV1(loadedConfig);
    return canonicalMasterInitialStorageFromValidatedConfig(bridgeConfig, bridgeAddress);
}

/// Reconstruct the already-validated canonical master data without repeating
/// the expensive verifying-key byte commitment on every bridge operation.
fun canonicalMasterInitialStorageFromValidatedConfig(
    bridgeConfig: Cell<BridgeConfig>,
    bridgeAddress: address,
): MasterStorage {
    val loadedConfig = bridgeConfig.load();
    assert (bridgeAddress.getWorkchain() == SCCP_WORKCHAIN) throw SccpErrors.InvalidConfiguration;
    val routeConfigurationHash = loadedConfig.routeHashes.load().routeConfigurationHash;
    return {
        storageVersion: SCCP_STORAGE_VERSION,
        routeConfigurationHash,
        bridgeConfigCellHash: bridgeConfig.hash(),
        totalSupply: 0,
        metadata: loadedConfig.finalityHashes.load().tail.load().masterMetadata,
        bridgeConfig,
        bridgeAddress,
        replay: emptyMasterReplayState(),
        pendingMints: [],
        pendingMintCount: 0,
        mintingDisabled: false,
    };
}

fun BridgeStorage.load(): BridgeStorage {
    return BridgeStorage.fromCell(contract.getData());
}

fun BridgeStorage.save(self) {
    contract.setData(self.toCell());
}

fun MasterStorage.load(): MasterStorage {
    return MasterStorage.fromCell(contract.getData());
}

fun MasterStorage.save(self) {
    contract.setData(self.toCell());
}

fun WalletStorage.load(): WalletStorage {
    return WalletStorage.fromCell(contract.getData());
}

fun WalletStorage.save(self) {
    contract.setData(self.toCell());
}
