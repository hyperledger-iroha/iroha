import "@stdlib/strings"

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
}

/// Shared immutable bridge configuration committed by master and bridge StateInit.
struct BridgeConfig {
    expectedGlobalId: int32
    routeRevision: uint32
    tairaToTonMultiplier: uint64
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
}

struct WalletReplayState {
    mints: map<uint256, uint8>
    burns: map<uint256, uint8>
    refunds: map<uint256, uint8>
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
