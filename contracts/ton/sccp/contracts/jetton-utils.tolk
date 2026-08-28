import "constants"
import "errors"
import "messages"
import "sccp-codec"
import "storage"

/// Compute the exact balance that must survive this message. `originalBalance`
/// includes the inbound value at the start of the compute phase, so checked
/// subtraction reconstructs the pre-message balance.
fun sccpPreMessageReserveAmount(
    originalBalance: coins,
    inboundValue: coins,
    protocolMinimum: coins,
): coins {
    assert (originalBalance >= inboundValue) throw SccpErrors.ReserveInvariant;
    val beforeMessage = originalBalance - inboundValue;
    return beforeMessage >= protocolMinimum ? beforeMessage : protocolMinimum;
}

/// Install the reserve before any outbound action. Exact mode is intentional:
/// `AT_MOST` would silently weaken the storage-rent invariant.
fun reserveSccpPreMessageBalance(inboundValue: coins, protocolMinimum: coins) {
    val originalBalance = contract.getOriginalBalance();
    val reserve = sccpPreMessageReserveAmount(
        originalBalance,
        inboundValue,
        protocolMinimum,
    );
    assert (originalBalance >= reserve) throw SccpErrors.NotEnoughGas;
    reserveToncoinsOnBalance(
        reserve,
        RESERVE_MODE_EXACT_AMOUNT | RESERVE_MODE_BOUNCE_ON_ACTION_FAIL,
    );
}

/// Explicit excess value derived solely from the current inbound message.
/// One operation allowance remains unallocated for forwarding and action fees.
fun sccpBoundedInboundExcess(inboundValue: coins, committedValue: coins): coins {
    assert (inboundValue >= committedValue) throw SccpErrors.NotEnoughGas;
    val available = inboundValue - committedValue;
    return available > SCCP_OPERATION_VALUE ? available - SCCP_OPERATION_VALUE : 0;
}

fun sccpPendingCapacityAvailable(count: uint16): bool {
    return count < SCCP_PENDING_OPERATION_CAP;
}

/// Guardian keys are immutable, nonzero, distinct, and canonically ordered.
fun mintBreakerGuardiansAreCanonical(guardians: MintBreakerGuardians): bool {
    val tail = guardians.tail.load();
    return guardians.guardian0 != 0 &&
    guardians.guardian0 < guardians.guardian1 &&
    guardians.guardian1 < guardians.guardian2 &&
    guardians.guardian2 < tail.guardian3 &&
    tail.guardian3 < tail.guardian4;
}

/// Exact cell signed by a breaker guardian. The target contract address stops
/// an authorization for the bridge being replayed against the master.
fun sccpMintBreakerAuthorization(config: BridgeConfig, target: address): cell {
    return beginCell()
        .storeSlice("sccp:ton:mint-breaker:disable:v1".literalSlice())
        .storeAddress(target)
        .storeInt(config.expectedGlobalId, 32)
        .storeUint(config.routeRevision, 32)
        .storeUint(config.routeHashes.load().routeConfigurationHash, 256)
        .storeUint(1, 8)
        .endCell();
}

fun mintBreakerSignatureValid(
    signature: cell?,
    digest: uint256,
    guardian: uint256,
): bool {
    if (signature == null) {
        return false;
    }
    val encoded = signature!.beginParse();
    if (encoded.remainingBitsCount() != 512 || encoded.remainingRefsCount() != 0) {
        return false;
    }
    return isSignatureValid(digest, encoded, guardian);
}

/// Verify the immutable fixed-slot 3-of-5 controller. Every supplied signature
/// must be canonical and valid; omitted slots do not count.
fun verifyMintBreakerAuthorization(
    config: BridgeConfig,
    target: address,
    message: SccpDisableMinting,
): bool {
    val guardians = config.finalityHashes.load().tail.load().mintBreakerGuardians.load();
    if (!mintBreakerGuardiansAreCanonical(guardians)) {
        return false;
    }
    val signatures = message.tail.load();
    val digest = sccpMintBreakerAuthorization(config, target).hash();
    var validCount = 0;
    if (message.signature0 != null) {
        if (!mintBreakerSignatureValid(message.signature0, digest, guardians.guardian0)) {
            return false;
        }
        validCount += 1;
    }
    if (message.signature1 != null) {
        if (!mintBreakerSignatureValid(message.signature1, digest, guardians.guardian1)) {
            return false;
        }
        validCount += 1;
    }
    if (signatures.signature2 != null) {
        if (!mintBreakerSignatureValid(signatures.signature2, digest, guardians.guardian2)) {
            return false;
        }
        validCount += 1;
    }
    val guardianTail = guardians.tail.load();
    if (signatures.signature3 != null) {
        if (!mintBreakerSignatureValid(signatures.signature3, digest, guardianTail.guardian3)) {
            return false;
        }
        validCount += 1;
    }
    if (signatures.signature4 != null) {
        if (!mintBreakerSignatureValid(signatures.signature4, digest, guardianTail.guardian4)) {
            return false;
        }
        validCount += 1;
    }
    return validCount >= SCCP_MINT_BREAKER_THRESHOLD;
}

/// Bind each refund to one unique rejected deposit, even when a wallet reuses
/// its query id, owner, and amount for multiple transfers.
fun sccpRefundIdentity(
    refundSequence: uint64,
    queryId: uint64,
    owner: address,
    amount: coins,
): uint256 {
    val ownerBytes = canonicalInternalTonAddress36(owner);
    // `coins` is a VarUInteger 16 (at most 120 value bits), so every Jetton
    // amount can be represented here. Do not apply the governed bridge amount
    // cap: malformed deposits above that cap must still remain refundable.
    assert (amount > 0) throw SccpErrors.InvalidAmount;
    val preimage = beginCell()
        .storeSlice("sccp:ton:refund:v1".literalSlice())
        .storeUint(refundSequence, 64)
        .storeUint(queryId, 64)
        .storeSlice(ownerBytes.beginParse())
        // Keep the preimage byte-aligned for HASHEXT and make the numeric
        // encoding independent of VarUInteger length-prefix boundaries.
        .storeUint(amount, 128)
        .endCell();
    val id = keccak256One(preimage.beginParse());
    assert (id != 0) throw SccpErrors.InvalidMessage;
    return id;
}

/// Scope recipient-wallet replay protection to the authenticated source
/// bridge. An unrelated wallet that copies a visible refund id cannot reserve
/// the canonical bridge's destination replay slot.
fun sccpWalletRefundDestinationReplayId(bridgeAddress: address, refundId: uint256): uint256 {
    assert (refundId != 0) throw SccpErrors.InvalidMessage;
    val bridgeBytes = canonicalInternalTonAddress36(bridgeAddress);
    val preimage = beginCell()
        .storeSlice("sccp:ton:refund-destination:v1".literalSlice())
        .storeSlice(bridgeBytes.beginParse())
        .storeUint(refundId, 256)
        .endCell();
    val id = keccak256One(preimage.beginParse());
    assert (id != 0) throw SccpErrors.InvalidMessage;
    return id;
}

/// Namespace the Jetton master's burn replay slot to the authenticated bridge
/// owner. Raw SCCP message ids are public; without this scope, any unrelated
/// wallet owner could burn first under a victim's id and strand the canonical
/// custody-wallet withdrawal.
fun sccpMasterBurnReplayId(bridgeAddress: address, messageId: uint256): uint256 {
    assert (messageId != 0) throw SccpErrors.InvalidMessage;
    val bridgeBytes = canonicalInternalTonAddress36(bridgeAddress);
    val preimage = beginCell()
        .storeSlice("sccp:ton:master-burn-replay:v1".literalSlice())
        .storeSlice(bridgeBytes.beginParse())
        .storeUint(messageId, 256)
        .endCell();
    val id = keccak256One(preimage.beginParse());
    assert (id != 0) throw SccpErrors.InvalidMessage;
    return id;
}

/// Build a replay acknowledgement for the current verified request. The
/// stored record supplies value-bearing fields, but its first submitter's
/// operational query id must not pin later callers.
fun sccpMasterMintAckForRequest(
    record: MasterMintRecord,
    messageId: uint256,
    requestQueryId: uint64,
): SccpMasterMintAck {
    return {
        queryId: requestQueryId,
        messageId,
        recipient: record.recipient,
        jettonAmount: record.jettonAmount,
        payloadHash: record.payloadHash,
    };
}

fun isCanonicalSccpMessageVersion(version: uint16): bool {
    return version == SCCP_MESSAGE_SCHEMA_VERSION;
}

/// A retry must replace the outgoing operation value and leave a second
/// operation-value allowance for deterministic inbound/outbound gas costs.
fun sccpRetryValueIsSufficient(value: coins): bool {
    return value >= 2 * SCCP_OPERATION_VALUE;
}

/// Fund the source-wallet forwarding hop, the destination-wallet notification
/// hop, and the bridge operation without drawing down any contract reserve.
fun sccpTransferRequestValueIsSufficient(value: coins, forwardTonAmount: coins): bool {
    return forwardTonAmount == 0 ||
    (forwardTonAmount >= 2 * SCCP_OPERATION_VALUE &&
    value >= forwardTonAmount + 2 * SCCP_OPERATION_VALUE);
}

/// Ensure the destination wallet received both the advertised notification
/// value and its own fixed operation allowance before it credits Jettons.
fun sccpInternalTransferValueIsSufficient(value: coins, forwardTonAmount: coins): bool {
    return forwardTonAmount == 0 ||
    (forwardTonAmount >= 2 * SCCP_OPERATION_VALUE &&
    value >= forwardTonAmount + SCCP_OPERATION_VALUE);
}

/// A stale, future, or exhausted outbound nonce is recoverable because the
/// custody wallet has already received the user's Jettons.
fun sccpOutboundNonceCanAdvance(expectedNonce: uint64, currentNonce: uint64): bool {
    return expectedNonce == currentNonce && currentNonce != 0xffffffffffffffff;
}

/// Require a retried burn or its bounce to match the debit already reserved
/// under the message id. Callers must resend values from this record.
fun sccpWalletBurnRecordMatches(
    record: WalletBurnRecord,
    status: uint8,
    queryId: uint64,
    bridgeAddress: address,
    jettonAmount: coins,
    payloadHash: uint256,
    sourceEventDigest: uint256,
): bool {
    return record.status == status && record.queryId == queryId &&
    record.bridgeAddress == bridgeAddress &&
    record.jettonAmount == jettonAmount &&
    record.payloadHash == payloadHash &&
    record.sourceEventDigest == sourceEventDigest;
}

/// Require both sides of a refund replay to retain the exact first source,
/// recipient, query, and amount tuple. Callers must resend values from this
/// record rather than from a duplicate message.
fun sccpWalletRefundRecordMatches(
    record: WalletRefundRecord,
    status: uint8,
    queryId: uint64,
    bridgeAddress: address,
    recipientOwner: address,
    jettonAmount: coins,
): bool {
    return record.status == status && record.queryId == queryId &&
    record.bridgeAddress == bridgeAddress &&
    record.recipientOwner == recipientOwner &&
    record.jettonAmount == jettonAmount;
}

fun emptyWalletReplayState(): Cell<WalletReplayState> {
    return WalletReplayState {
        mints: [],
        burns: [],
        refunds: [],
        mintCount: 0,
        burnCount: 0,
        refundCount: 0,
    }.toCell();
}

fun calcDeployedJettonWallet(
    ownerAddress: address,
    masterAddress: address,
    jettonWalletCode: cell,
): AutoDeployAddress {
    val emptyWalletStorage: WalletStorage = {
        jettonBalance: 0,
        ownerAddress,
        masterAddress,
        replay: emptyWalletReplayState(),
    };
    return {
        workchain: SCCP_WORKCHAIN,
        stateInit: { code: jettonWalletCode, data: emptyWalletStorage.toCell() },
    };
}

fun calcAddressOfJettonWallet(
    ownerAddress: address,
    masterAddress: address,
    jettonWalletCode: cell,
): address {
    return calcDeployedJettonWallet(
        ownerAddress,
        masterAddress,
        jettonWalletCode,
    ).calculateAddress();
}
