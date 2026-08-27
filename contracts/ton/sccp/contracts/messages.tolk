type ForwardPayloadRemainder = RemainingBitsAndRefs

struct (0b0) PayloadInline {
    value: RemainingBitsAndRefs
}

struct (0b1) PayloadInRef {
    value: Cell<RemainingBitsAndRefs>
}

/// TEP-74 transfer request.
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

/// TEP-74 internal transfer.
struct (0x178d4519) InternalTransferStep {
    queryId: uint64
    jettonAmount: coins
    transferInitiator: address?
    sendExcessesTo: address?
    forwardTonAmount: coins
    @abi.clientType(PayloadInline | PayloadInRef)
    forwardPayload: ForwardPayloadRemainder
}

/// TEP-74 transfer notification.
struct (0x7362d09c) TransferNotificationForRecipient {
    queryId: uint64
    jettonAmount: coins
    transferInitiator: address?
    @abi.clientType(PayloadInline | PayloadInRef)
    forwardPayload: ForwardPayloadRemainder
}

/// TEP-74 burn request.
struct (0x595f07bc) AskToBurn {
    queryId: uint64
    jettonAmount: coins
    sendExcessesTo: address?
    customPayload: cell?
}

/// TEP-74 burn notification.
struct (0x7bdd97de) BurnNotificationForMinter {
    queryId: uint64
    jettonAmount: coins
    burnInitiator: address
    sendExcessesTo: address?
}

struct (0xd53276db) ReturnExcessesBack {
    queryId: uint64
}

struct (0x2c76b973) RequestWalletAddress {
    queryId: uint64
    ownerAddress: address
    includeOwnerAddress: bool
}

struct (0xd1735400) ResponseWalletAddress {
    queryId: uint64
    jettonWalletAddress: address?
    ownerAddress: Cell<address>?
}

/// No-op TON balance top-up accepted by every SCCP contract.
struct (0xd372158c) TopUpTons {}

/// SCCP request embedded in a Jetton transfer's forward payload.
struct (0x53434342) SccpOutboundBurnRequest {
    version: uint16
    expectedNonce: uint64
    tairaRecipient: cell
}

/// Fail-closed Taira-to-TON finalization request.
struct (0x53434350) SccpFinalizeFromTaira {
    queryId: uint64
    version: uint16
    messageId: uint256
    statementHash: uint256
    publicSignals: cell
    proof: cell
    canonicalPayload: cell
}

/// Retry a previously reserved asynchronous operation.
struct (0x53434351) SccpRetry {
    queryId: uint64
    messageId: uint256
}

/// Bridge-to-master mint request. Replay identity is the full message id.
struct (0x5343434d) SccpMasterMint {
    queryId: uint64
    messageId: uint256
    recipient: address
    jettonAmount: coins
    tail: Cell<SccpMasterMintTail>
}

struct SccpMasterMintTail {
    payloadHash: uint256
    statementHash: uint256
    publicSignals: cell
    proof: cell
    canonicalPayload: cell
}

/// Master-to-wallet mint request carrying the full replay identity.
struct (0x5343574d) SccpWalletMint {
    queryId: uint64
    messageId: uint256
    bridgeAddress: address
    jettonAmount: coins
    payloadHash: uint256
}

/// Wallet-to-master authenticated mint acknowledgement.
struct (0xd343574d) SccpWalletMintAck {
    queryId: uint64
    messageId: uint256
    bridgeAddress: address
    jettonAmount: coins
    payloadHash: uint256
}

/// Master-to-bridge authenticated mint acknowledgement.
struct (0xd343434d) SccpMasterMintAck {
    queryId: uint64
    messageId: uint256
    recipient: address
    jettonAmount: coins
    payloadHash: uint256
}

/// Bridge-to-custody-wallet SCCP burn request.
struct (0x53435742) SccpWalletBurn {
    queryId: uint64
    messageId: uint256
    jettonAmount: coins
    payloadHash: uint256
    sourceEventDigest: uint256
}

/// Custody-wallet-to-master burn notification with a full replay id.
struct (0x53434d42) SccpMasterBurn {
    queryId: uint64
    messageId: uint256
    bridgeAddress: address
    jettonAmount: coins
    tail: Cell<SccpMasterBurnTail>
}

struct SccpMasterBurnTail {
    payloadHash: uint256
    sourceEventDigest: uint256
}

/// Master-to-bridge authenticated burn acknowledgement.
struct (0xd3434342) SccpMasterBurnAck {
    queryId: uint64
    messageId: uint256
    jettonAmount: coins
    payloadHash: uint256
    sourceEventDigest: uint256
}

/// Bridge-to-custody-wallet refund request for rejected forward payloads.
struct (0x53435246) SccpRefund {
    queryId: uint64
    refundId: uint256
    recipientOwner: address
    jettonAmount: coins
}

/// Custody-wallet-to-recipient-wallet replay-safe refund transfer.
struct (0x53435752) SccpWalletRefundTransfer {
    queryId: uint64
    refundId: uint256
    bridgeAddress: address
    recipientOwner: address
    jettonAmount: coins
}

/// Recipient-wallet-to-bridge authenticated refund acknowledgement.
struct (0xd3435752) SccpWalletRefundAck {
    queryId: uint64
    refundId: uint256
    recipientOwner: address
    jettonAmount: coins
}

/// Canonical external-out event body proved by TON source admission.
struct (0x53434350) SccpOutboundEvent {
    version: uint16
    laneHash: uint256
    messageId: uint256
    payloadHash: uint256
    tail: Cell<SccpOutboundEventTail>
}

/// Tail of the canonical TON SCCP external-out body. Splitting here is mandatory: the four hash
/// roles plus opcode/version would exceed TVM's 1023-bit ordinary-cell limit.
struct SccpOutboundEventTail {
    sourceEventDigest: uint256
    canonicalPayloadSnake: cell
}

/// Exact four-cell representation of canonical SCCP payload bytes. `header` is exactly 50 bytes;
/// the remaining canonical bytes are split into the next 100, next 100, and final 0..124 bytes.
/// The same cells are linked directly as a four-cell snake in the external-out event.
struct CanonicalPayloadCells {
    header: cell
    chunk1: cell
    chunk2: cell
    chunk3: cell
}

fun ForwardPayloadRemainder.checkIsCorrectTlbEither(self) {
    var mutableCopy = self;
    if (mutableCopy.loadMaybeRef() != null) {
        mutableCopy.assertEnd();
    }
}
