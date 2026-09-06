type ForwardPayloadRemainder = RemainingBitsAndRefs

/// Canonical final-V1 sparse-Merkle witness. `siblings` is a unique bounded
/// snake: three hashes in every nonterminal cell and one to three in the final
/// cell. A zero bitmap has no sibling reference, so the tree contains exactly
/// `ceil(popcount / 3)` sibling cells for every witness.
struct SccpReplayWitnessV1 {
    expectedShardRoot: uint256
    priorRecordDigest: uint256
    siblingBitmap: uint256
    siblings: cell?
}

/// Fresh witnesses for two successive contract boundaries.
struct SccpReplayWitnessPairV1 {
    first: Cell<SccpReplayWitnessV1>
    second: Cell<SccpReplayWitnessV1>
}

/// Bridge, master, and recipient-wallet mint-boundary witnesses.
struct SccpMintReplayWitnessesV1 {
    bridge: Cell<SccpReplayWitnessV1>
    master: Cell<SccpReplayWitnessV1>
    wallet: Cell<SccpReplayWitnessV1>
}

/// Initial direct-wallet burn witnesses for lock, bridge admission,
/// irreversible wallet authorization, and master burn. A rejected pre-admission
/// lock always receives a fresh 0x37 witness on its separate refund call.
struct SccpBurnReplayWitnessesV1 {
    lockDebit: Cell<SccpReplayWitnessV1>
    bridge: Cell<SccpReplayWitnessV1>
    walletAuthorization: Cell<SccpReplayWitnessV1>
    tail: Cell<SccpBurnReplayWitnessesTailV1>
}

struct SccpBurnReplayWitnessesTailV1 {
    master: Cell<SccpReplayWitnessV1>
}

/// Replaceable witnesses used after a transient lock already exists.
struct SccpBurnPrepareRetryWitnessesV1 {
    bridge: Cell<SccpReplayWitnessV1>
    walletAuthorization: Cell<SccpReplayWitnessV1>
    master: Cell<SccpReplayWitnessV1>
}

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

/// Owner-authenticated request to debit one transient outbound burn lock.
struct (0x53434342) SccpWalletPrepareBurn {
    queryId: uint64
    expectedNonce: uint64
    jettonAmount: coins
    tairaRecipient: cell
    replayWitnesses: Cell<SccpBurnReplayWitnessesV1>
}

/// Immutable owner request carried through the self-authenticated key-validity
/// stage before operation 0x36 can debit or occupy replay state.
struct SccpWalletBurnValidationMaterialV1 {
    queryId: uint64
    expectedNonce: uint64
    jettonAmount: coins
    tairaRecipient: cell
    replayWitnesses: Cell<SccpBurnReplayWitnessesV1>
}

/// Continue exact Ed25519 key validation without exposing an external state
/// injection surface. The wallet accepts this only from itself.
struct (0x53565243) SccpWalletValidateBurnRecipient {
    encodedEd25519Key: uint256
    material: Cell<SccpWalletBurnValidationMaterialV1>
}

/// One ordinary-cell representation of an extended Edwards point.
struct SccpEdwardsPointV1 {
    x: uint255
    y: uint255
    z: uint255
    t: uint255
}

/// Self-authenticated bounded fixed-scalar multiplication step. A delayed
/// duplicate must still pass the wallet nonce checks immediately before 0x36.
struct (0x53565347) SccpWalletValidateBurnSubgroup {
    stepsRemaining: uint8
    remainingScalar: uint256
    result: Cell<SccpEdwardsPointV1>
    multiple: Cell<SccpEdwardsPointV1>
    material: Cell<SccpWalletBurnValidationMaterialV1>
}

/// One self-authenticated canonical payload-hash block before 0x36.
struct (0x53564842) SccpWalletBurnHashStep {
    nextBlock: uint8
    chainingState: Cell<SccpBlake2bStateV1>
    material: Cell<SccpWalletBurnValidationMaterialV1>
}

/// Final self-authenticated pre-debit boundary after key and payload hashing.
struct (0x53564342) SccpWalletCommitValidatedBurn {
    payloadHash: uint256
    material: Cell<SccpWalletBurnValidationMaterialV1>
}

/// Retry bridge preparation for an existing refundable lock with fresh proofs.
struct (0x53434352) SccpWalletRetryBurnPrepare {
    lockSequence: uint64
    replayWitnesses: Cell<SccpBurnPrepareRetryWitnessesV1>
}

/// Recredit a rejected lock through operation 0x37.
struct (0x53434346) SccpWalletRefundBurnLock {
    lockSequence: uint64
    replayWitness: Cell<SccpReplayWitnessV1>
}

/// Canonical owner wallet asks the bridge to admit operation 0x31. The first
/// 128 body bits identify the precise bounced attempt.
struct (0x53434250) SccpBridgeBurnPrepare {
    lockSequence: uint64
    attempt: uint32
    queryId: uint64
    outboundNonce: uint64
    ownerAddress: address
    jettonAmount: coins
    tail: Cell<SccpBridgeBurnPrepareTail>
}

struct SccpBridgeBurnPrepareTail {
    payloadHash: uint256
    payloadSha256: uint256
    tairaRecipient: cell
    replayWitnesses: Cell<SccpBurnPrepareRetryWitnessesV1>
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
    replayWitnesses: Cell<SccpMintReplayWitnessesV1>
}

/// Self-authenticated material carried between bounded payload-hash stages.
/// `recipient` and `amountScale9` were obtained by the strict first-stage
/// parser; only this bridge can originate a continuation carrying them.
struct SccpFinalizeMaterialV1 {
    statementHash: uint256
    recipient: address
    amountScale9: uint128
    publicSignals: cell
    proof: cell
    canonicalPayload: cell
    replayWitnesses: Cell<SccpMintReplayWitnessesV1>
}

/// Eight little-endian BLAKE2b chaining words after one complete block.
struct SccpBlake2bStateV1 {
    word0: uint64
    word1: uint64
    word2: uint64
    word3: uint64
    word4: uint64
    word5: uint64
    word6: uint64
    word7: uint64
}

/// Internal continuation for one additional canonical BLAKE2b block.
struct (0x53434853) SccpFinalizeHashStep {
    queryId: uint64
    version: uint16
    messageId: uint256
    nextBlock: uint8
    chainingState: Cell<SccpBlake2bStateV1>
    material: Cell<SccpFinalizeMaterialV1>
}

/// Internal proof/admission stage after the canonical payload hash is complete.
struct (0x53434144) SccpFinalizeAdmission {
    queryId: uint64
    version: uint16
    messageId: uint256
    payloadHash: uint256
    material: Cell<SccpFinalizeMaterialV1>
}

/// Retry a previously reserved asynchronous operation.
struct (0x53434351) SccpRetry {
    queryId: uint64
    messageId: uint256
    replayWitnesses: Cell<SccpReplayWitnessPairV1>
}

/// Route guardians permanently disable new mint admissions. Signatures are
/// fixed to guardian slots, so a key cannot be counted twice.
struct (0x53434344) SccpDisableMinting {
    signature0: cell?
    signature1: cell?
    tail: Cell<SccpDisableMintingTail>
}

struct SccpDisableMintingTail {
    signature2: cell?
    signature3: cell?
    signature4: cell?
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
    payloadSha256: uint256
    replayWitnesses: Cell<SccpReplayWitnessPairV1>
}

/// Master-to-wallet mint request carrying the full replay identity.
struct (0x5343574d) SccpWalletMint {
    queryId: uint64
    messageId: uint256
    bridgeAddress: address
    jettonAmount: coins
    tail: Cell<SccpWalletMintTail>
}

struct SccpWalletMintTail {
    payloadHash: uint256
    payloadSha256: uint256
    replayWitness: Cell<SccpReplayWitnessV1>
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

/// Bridge authorization that turns a transient wallet lock into operation
/// 0x35. `completed` is terminal acknowledgement of an already-finished 0x31.
struct (0x53435742) SccpWalletBurn {
    lockSequence: uint64
    queryId: uint64
    messageId: uint256
    outboundNonce: uint64
    ownerAddress: address
    jettonAmount: coins
    completed: bool
    tail: Cell<SccpWalletBurnTail>
}

struct SccpWalletBurnTail {
    payloadHash: uint256
    payloadSha256: uint256
    sourceEventDigest: uint256
    replayWitnesses: Cell<SccpReplayWitnessPairV1>
}

/// Canonical owner-wallet-to-master burn notification with a full replay id.
struct (0x53434d42) SccpMasterBurn {
    lockSequence: uint64
    queryId: uint64
    messageId: uint256
    jettonAmount: coins
    tail: Cell<SccpMasterBurnTail>
}

struct SccpMasterBurnTail {
    bridgeAddress: address
    ownerAddress: address
    payload: Cell<SccpMasterBurnPayloadTail>
}

struct SccpMasterBurnPayloadTail {
    payloadHash: uint256
    payloadSha256: uint256
    sourceEventDigest: uint256
    replayWitness: Cell<SccpReplayWitnessV1>
}

/// Master-to-bridge authenticated burn acknowledgement.
struct (0xd3434342) SccpMasterBurnAck {
    lockSequence: uint64
    queryId: uint64
    messageId: uint256
    jettonAmount: coins
    tail: Cell<SccpMasterBurnAckTail>
}

struct SccpMasterBurnAckTail {
    ownerAddress: address
    payloadHash: uint256
    sourceEventDigest: uint256
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

/// Permanent replay transition emitted by every fresh boundary admission.
struct (0x53435244) SccpReplayDeltaV1 {
    operation: uint8
    domainHash: uint256
    shard: uint8
    key: uint256
    tail: Cell<SccpReplayDeltaTailV1>
}

struct SccpReplayDeltaTailV1 {
    recordDigest: uint256
    oldRoot: uint256
    newRoot: uint256
    leafCount: uint64
    updateSequence: uint64
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
