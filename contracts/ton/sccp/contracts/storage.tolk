import "@stdlib/strings"
import "constants"
import "errors"

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
}

struct FinalityHashes {
    soraFinalityAnchorHash: uint256
    verifierCircuitHash: uint256
    verifyingKeyHash: uint256
    tail: Cell<FinalityHashesTail>
}

struct FinalityHashesTail {
    proofProfileCommitment: uint256
    embeddedVerifierCodeHash: uint256
    /// TON representation hash of the exact immutable key cell.
    verifyingKeyCellHash: uint256
    verifyingKey: cell
    /// Immutable, route-bound 3-of-5 emergency mint controller.
    mintBreakerGuardians: Cell<MintBreakerGuardians>
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
    statementHash: uint256
    publicSignals: cell
    proof: cell
    canonicalPayload: cell
}

struct PendingBurn {
    queryId: uint64
    ownerAddress: address
    jettonAmount: coins
    payloadHash: uint256
    sourceEventDigest: uint256
    canonicalPayload: cell
}

struct PendingRefund {
    queryId: uint64
    recipientOwner: address
    jettonAmount: coins
}

struct BridgeReplayState {
    inbound: map<uint256, uint8>
    outbound: map<uint256, uint8>
}

struct BridgePendingState {
    mints: map<uint256, Cell<PendingMint>>
    burns: map<uint256, Cell<PendingBurn>>
    refunds: map<uint256, Cell<PendingRefund>>
    mintCount: uint16
    burnCount: uint16
    refundCount: uint16
}

struct BridgeStorage {
    // Source-proof parsers authenticate these first 264 data-root bits directly.
    storageVersion: uint8
    routeConfigurationHash: uint256
    config: Cell<BridgeConfig>
    masterAddress: address
    nonces: map<address, uint64>
    replay: Cell<BridgeReplayState>
    pending: Cell<BridgePendingState>
    /// Monotonic identity allocated before each rejected-deposit refund.
    refundSequence: uint64
    /// One-way breaker state. Canonical deployment always starts enabled.
    mintingDisabled: bool
}

struct MasterMintRecord {
    status: uint8
    queryId: uint64
    bridgeAddress: address
    recipient: address
    jettonAmount: coins
    payloadHash: uint256
}

struct MasterBurnRecord {
    status: uint8
    queryId: uint64
    bridgeAddress: address
    jettonAmount: coins
    payloadHash: uint256
    sourceEventDigest: uint256
}

struct MasterReplayState {
    mints: map<uint256, Cell<MasterMintRecord>>
    burns: map<uint256, Cell<MasterBurnRecord>>
}

struct MasterStorage {
    storageVersion: uint8
    routeConfigurationHash: uint256
    totalSupply: coins
    metadata: cell
    bridgeConfig: Cell<BridgeConfig>
    replay: Cell<MasterReplayState>
    /// Number of master-to-wallet mints awaiting an authenticated ack.
    pendingMintCount: uint16
    /// One-way breaker state. Canonical deployment always starts enabled.
    mintingDisabled: bool
}

/// Exact debit record retained by a wallet while an SCCP burn can be retried.
struct WalletBurnRecord {
    status: uint8
    queryId: uint64
    bridgeAddress: address
    jettonAmount: coins
    payloadHash: uint256
    sourceEventDigest: uint256
}

/// Exact source/destination tuple retained for an SCCP refund replay.
struct WalletRefundRecord {
    status: uint8
    queryId: uint64
    bridgeAddress: address
    recipientOwner: address
    jettonAmount: coins
}

struct WalletReplayState {
    mints: map<uint256, uint8>
    burns: map<uint256, Cell<WalletBurnRecord>>
    refunds: map<uint256, Cell<WalletRefundRecord>>
    mintCount: uint16
    burnCount: uint16
    refundCount: uint16
}

struct WalletStorage {
    jettonBalance: coins
    ownerAddress: address
    masterAddress: address
    replay: Cell<WalletReplayState>
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

/// Empty bridge replay state used by the only canonical V1 deployment data.
fun emptyBridgeReplayState(): Cell<BridgeReplayState> {
    return BridgeReplayState {
        inbound: [],
        outbound: [],
    }.toCell();
}

/// Empty bridge operation state used by the only canonical V1 deployment data.
fun emptyBridgePendingState(): Cell<BridgePendingState> {
    return BridgePendingState {
        mints: [],
        burns: [],
        refunds: [],
        mintCount: 0,
        burnCount: 0,
        refundCount: 0,
    }.toCell();
}

/// Empty Jetton-master replay state used by the only canonical V1 deployment data.
fun emptyMasterReplayState(): Cell<MasterReplayState> {
    return MasterReplayState {
        mints: [],
        burns: [],
    }.toCell();
}

/// Build the canonical bridge StateInit data for a governed route.
///
/// Governance commits the resulting data-cell hash. Keeping every mutable
/// counter and dictionary fixed at zero/empty prevents a deployment from
/// preoccupying replay slots, skipping outbound nonces, or exhausting refund
/// identities before the first authenticated message.
fun canonicalBridgeInitialStorage(
    config: Cell<BridgeConfig>,
    masterAddress: address,
): BridgeStorage {
    val loadedConfig = config.load();
    assert (loadedConfig.maxWrappedSupply > 0) throw SccpErrors.InvalidConfiguration;
    val routeConfigurationHash = loadedConfig.routeHashes.load().routeConfigurationHash;
    return {
        storageVersion: SCCP_STORAGE_VERSION,
        routeConfigurationHash,
        config,
        masterAddress,
        nonces: [],
        replay: emptyBridgeReplayState(),
        pending: emptyBridgePendingState(),
        refundSequence: 0,
        mintingDisabled: false,
    };
}

/// Build the canonical Jetton-master StateInit data for a governed route.
///
/// Governance commits the resulting data-cell hash. Supply and all replay
/// dictionaries therefore start from a unique zero state.
fun canonicalMasterInitialStorage(
    metadata: cell,
    bridgeConfig: Cell<BridgeConfig>,
): MasterStorage {
    val loadedConfig = bridgeConfig.load();
    assert (loadedConfig.maxWrappedSupply > 0) throw SccpErrors.InvalidConfiguration;
    val routeConfigurationHash = loadedConfig.routeHashes.load().routeConfigurationHash;
    return {
        storageVersion: SCCP_STORAGE_VERSION,
        routeConfigurationHash,
        totalSupply: 0,
        metadata,
        bridgeConfig,
        replay: emptyMasterReplayState(),
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
