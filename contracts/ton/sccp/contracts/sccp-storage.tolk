// Persistent layouts of the SCCP v1 TON contracts (§5.3.1, §5.3.3).
//
// Field order is serialization order and matches the TL-B of the spec
// exactly; `scripts/generate_ton_sccp_stateinit_golden.py` rebuilds the
// canonical initial minter data independently and pins it in
// `fixtures/sccp/ton_stateinit_v1.json`.

import "@stdlib/strings"

/// `minter_config#_` — immutable deployment configuration.
struct MinterConfig {
    tairaNetworkId: uint256
    routeRevision: uint32
    maxSupply: coins
    initialGeneration: uint64
    initialDigest: uint256
    walletCode: cell
    bucketCode: cell
}

/// `roster_state#_` — current roster generation with its members.
struct RosterState {
    digest: uint256
    generation: uint64
    validFromMs: uint64
    validUntilMs: uint64
    n: uint8
    t: uint8
    /// `^MemberChunk`, maximal 6-address chunks.
    members: cell
}

/// `prev_roster#_`; `validUntilMs` is already grace-capped (§5.1.5 step 4).
struct PrevRoster {
    digest: uint256
    validUntilMs: uint64
    n: uint8
    t: uint8
    members: cell
}

/// `minter_data#_` (MinterData of §5.3.1).
struct MinterStorage {
    initialized: bool
    totalSupply: coins
    pendingSupply: coins
    outboundNonce: uint64
    opCount: uint64
    controlNonce: uint64
    mintingPaused: bool
    deployedBuckets: uint64
    config: Cell<MinterConfig>
    roster: Cell<RosterState>
    prevRoster: Cell<PrevRoster>?
}

fun MinterStorage.load(): MinterStorage {
    return MinterStorage.fromCell(contract.getData());
}

fun MinterStorage.save(self) {
    contract.setData(self.toCell());
}

/// TEP-74 wallet storage.
struct WalletStorage {
    jettonBalance: coins
    ownerAddress: address
    minterAddress: address
}

fun WalletStorage.load(): WalletStorage {
    return WalletStorage.fromCell(contract.getData());
}

fun WalletStorage.save(self) {
    contract.setData(self.toCell());
}

/// `bucket_data#_ minter:MsgAddressInt index:uint64 bits:uint512`; the 512
/// flags are stored as their high and low 256-bit halves, so flag `i` is bit
/// `i` of the 512-bit big-endian integer `bitsHi ‖ bitsLo`.
struct BucketStorage {
    minterAddress: address
    index: uint64
    bitsHi: uint256
    bitsLo: uint256
}

fun BucketStorage.load(): BucketStorage {
    return BucketStorage.fromCell(contract.getData());
}

fun BucketStorage.save(self) {
    contract.setData(self.toCell());
}

/// TEP-74 `get_wallet_data` reply.
struct JettonWalletDataReply {
    jettonBalance: coins
    ownerAddress: address
    minterAddress: address
    jettonWalletCode: cell
}

/// TEP-64 on-chain content: `onchain#00 data:(HashmapE 256 ^ContentData)`.
struct (0x00) OnchainMetadataReply {
    contentDict: map<uint256, string_prefixed0x>
}

/// TEP-74 `get_jetton_data` reply.
struct JettonDataReply {
    totalSupply: int
    mintable: bool
    adminAddress: address?
    jettonContent: Cell<OnchainMetadataReply>
    jettonWalletCode: cell
}

/// `get_sccp_state` reply (§5.3.6), in the order of the spec.
struct SccpStateReply {
    initialized: bool
    tairaNetworkId: uint256
    routeRevision: uint32
    initialGeneration: uint64
    initialDigest: uint256
    digest: uint256
    generation: uint64
    validUntilMs: uint64
    prevDigest: uint256
    prevValidUntilMs: uint64
    outboundNonce: uint64
    opCount: uint64
    mintingPaused: bool
    controlNonce: uint64
    totalSupply: coins
    pendingSupply: coins
    maxSupply: coins
    deployedBuckets: uint64
}

/// `get_bucket_data` reply of a consumption bucket.
struct BucketDataReply {
    minterAddress: address
    index: uint64
    bitsHi: uint256
    bitsLo: uint256
}
