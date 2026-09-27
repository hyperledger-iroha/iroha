// Message and cell layouts of the SCCP v1 TON contracts (§5.3.2, §5.3.3).
//
// Field order is serialization order; every struct is the exact TL-B of the
// spec. Variable-shape cells (`SnakeBytes`, `HashChunk`, `MemberChunk`,
// `Signatures`) are typed as `cell` and parsed by `sccp-cells.tolk`, which
// rejects every non-canonical shape.

/// Second cell of an attestation statement (768 bits).
struct AttestationTail {
    historyRoot: uint256
    rosterDigest: uint256
    nextRosterDigest: uint256
}

/// Attestation statement of §3.6.1 in its TON cell form (800 bits + tail).
struct Attestation {
    height: uint64
    epoch: uint64
    timestampMs: uint64
    messageCount: uint32
    historySize: uint64
    blockHash: uint256
    sccpRoot: uint256
    tail: Cell<AttestationTail>
}

/// Position of a transfer leaf in a block commitment tree plus its payload.
struct MessageProof {
    leafIndex: uint32
    pathLen: uint8
    /// `^SnakeBytes` with the §3.2 payload.
    payload: cell
    /// `(Maybe ^HashChunk)`, present iff `pathLen > 0`.
    path: cell?
}

/// Block `{height, sccpRoot, messageCount}` and its history path (§3.5).
struct HistoryProof {
    height: uint64
    sccpRoot: uint256
    messageCount: uint32
    leafIndex: uint64
    pathLen: uint8
    path: cell?
}

/// Next roster generation supplied to `sccp_rotate`.
struct RosterSpec {
    generation: uint64
    validFromMs: uint64
    validUntilMs: uint64
    n: uint8
    t: uint8
    /// `^MemberChunk`.
    members: cell
}

/// Parliament control proof (§5.1.6).
struct ControlProof {
    controlNonce: uint64
    paused: bool
    leafIndex: uint32
    pathLen: uint8
    path: cell?
}

/// `consume_tail#_` of every bucket consume round trip.
struct ConsumeTail {
    /// `SCCP_CONSUME_KIND_MINT` or `SCCP_CONSUME_KIND_VOID`.
    kind: uint1
    /// Mint recipient (owner of the Jetton wallet); `addr_none` for voids.
    recipient: address?
    /// Excess destination chosen by the caller.
    response: address
    messageId: uint256
}

/// TEP-74 burn custom payload required by SCCP wallets.
struct (0x53434254) SccpBurnToTaira {
    /// `^SnakeBytes` with the `taira_account` recipient bytes (1..=1024).
    recipient: cell
}

// ---------------------------------------------------------------------------
// Messages to the minter.

struct (0x53434930) SccpInit {
    queryId: uint64
}

struct (0x53434631) SccpFinalize {
    queryId: uint64
    response: address
    attestation: Cell<Attestation>
    /// `^Signatures`.
    signatures: cell
    message: Cell<MessageProof>
}

struct (0x53434632) SccpFinalizeHistorical {
    queryId: uint64
    response: address
    attestation: Cell<Attestation>
    signatures: cell
    history: Cell<HistoryProof>
    message: Cell<MessageProof>
}

struct (0x53435631) SccpVoidExpired {
    queryId: uint64
    response: address
    nonce: uint64
    attestation: Cell<Attestation>
    signatures: cell
    message: Cell<MessageProof>
}

struct (0x53435632) SccpVoidExpiredHistorical {
    queryId: uint64
    response: address
    nonce: uint64
    attestation: Cell<Attestation>
    signatures: cell
    history: Cell<HistoryProof>
    message: Cell<MessageProof>
}

struct (0x53435633) SccpVoidFrozen {
    queryId: uint64
    response: address
    firstNonce: uint64
    count: uint16
}

struct (0x53435231) SccpRotate {
    queryId: uint64
    response: address
    attestation: Cell<Attestation>
    signatures: cell
    next: Cell<RosterSpec>
}

struct (0x53434d31) SccpApplyControl {
    queryId: uint64
    response: address
    attestation: Cell<Attestation>
    signatures: cell
    control: Cell<ControlProof>
}

struct (0x53434d32) SccpApplyControlHistorical {
    queryId: uint64
    response: address
    attestation: Cell<Attestation>
    signatures: cell
    history: Cell<HistoryProof>
    control: Cell<ControlProof>
}

struct (0x53434431) SccpDeployBuckets {
    queryId: uint64
    response: address
    count: uint8
}

/// Bucket reply: the nonce was unset and is now consumed (bounceable).
struct (0x53434332) SccpConsumed {
    queryId: uint64
    nonce: uint64
    amount: uint96
    tail: Cell<ConsumeTail>
}

/// Bucket reply: the nonce was already consumed (non-bounceable).
struct (0x53434336) SccpAlreadyConsumed {
    queryId: uint64
    nonce: uint64
    amount: uint96
    tail: Cell<ConsumeTail>
}

/// Bucket reply: the whole range is now consumed (bounceable).
struct (0x53434334) SccpRangeConsumed {
    queryId: uint64
    first: uint64
    count: uint16
}

/// Bucket reply: some nonce of the range was already consumed.
struct (0x53434337) SccpRangeRejected {
    queryId: uint64
    first: uint64
    count: uint16
}

/// TEP-74 burn notification extended with the SCCP recipient (§5.3.3).
struct (0x7bdd97de) BurnNotificationForMinter {
    queryId: uint64
    jettonAmount: coins
    burnInitiator: address
    sendExcessesTo: address?
    sccp: Cell<SccpBurnToTaira>
}

/// `top_up query_id:uint64`: anyone may add value to a minter, wallet or bucket.
struct (0xd372158c) TopUpTons {
    queryId: uint64
}

/// TEP-89 `provide_wallet_address`.
struct (0x2c76b973) RequestWalletAddress {
    queryId: uint64
    ownerAddress: address
    includeOwnerAddress: bool
}

/// TEP-89 `take_wallet_address`.
struct (0xd1735400) ResponseWalletAddress {
    queryId: uint64
    jettonWalletAddress: address?
    ownerAddress: Cell<address>?
}

// ---------------------------------------------------------------------------
// Messages to a consumption bucket (sender MUST be the minter).

/// The first 256 body bits are `op ‖ query_id ‖ nonce ‖ amount`, so a bounce
/// returns the nonce and amount to the minter.
struct (0x53434331) SccpConsume {
    queryId: uint64
    nonce: uint64
    amount: uint96
    tail: Cell<ConsumeTail>
}

struct (0x53434333) SccpConsumeRange {
    queryId: uint64
    first: uint64
    count: uint16
}

struct (0x53434335) SccpUnconsume {
    nonce: uint64
}

// ---------------------------------------------------------------------------
// External-out messages of the minter.

struct (0x5343464e) SccpFinalizedEvent {
    messageId: uint256
    nonce: uint64
}

/// `messageId = 0` for frozen voids.
struct (0x5343564f) SccpVoidedEvent {
    messageId: uint256
    firstNonce: uint64
    count: uint16
}

struct (0x53435454) SccpTransferToTairaEvent {
    messageId: uint256
    nonce: uint64
    sender: address
    amount: coins
    /// `^SnakeBytes` with the §3.2 payload.
    payload: cell
}

struct (0x5343434e) SccpControlAppliedEvent {
    controlNonce: uint64
    paused: bool
}

// ---------------------------------------------------------------------------
// TEP-74 wallet messages.

type ForwardPayloadRemainder = RemainingBitsAndRefs

struct (0b0) PayloadInline {
    value: RemainingBitsAndRefs
}

struct (0b1) PayloadInRef {
    value: Cell<RemainingBitsAndRefs>
}

struct (0x0f8a7ea5) AskToTransfer {
    queryId: uint64
    jettonAmount: coins
    transferRecipient: address
    sendExcessesTo: address?
    customPayload: cell?
    forwardTonAmount: coins
    @abi.clientType(PayloadInline | PayloadInRef)
    forwardPayload: ForwardPayloadRemainder
}

struct (0x7362d09c) TransferNotificationForRecipient {
    queryId: uint64
    jettonAmount: coins
    transferInitiator: address?
    @abi.clientType(PayloadInline | PayloadInRef)
    forwardPayload: ForwardPayloadRemainder
}

struct (0x178d4519) InternalTransferStep {
    queryId: uint64
    jettonAmount: coins
    transferInitiator: address?
    sendExcessesTo: address?
    forwardTonAmount: coins
    @abi.clientType(PayloadInline | PayloadInRef)
    forwardPayload: ForwardPayloadRemainder
}

struct (0xd53276db) ReturnExcessesBack {
    queryId: uint64
}

/// TEP-74 burn; SCCP wallets require `customPayload = sccp_burn_to_taira`.
struct (0x595f07bc) AskToBurn {
    queryId: uint64
    jettonAmount: coins
    sendExcessesTo: address?
    customPayload: cell?
}

/// Checks a TEP-74 `Either Cell ^Cell` forward payload.
fun ForwardPayloadRemainder.checkIsCorrectTlbEither(self) {
    var mutableCopy = self;
    if (mutableCopy.loadMaybeRef() != null) {
        mutableCopy.assertEnd();
    }
}
