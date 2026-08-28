import "@stdlib/gas-payments"
import "constants"
import "errors"
import "messages"
import "sccp-codec"
import "storage"

fun sccpMaximumComputeFee(): coins {
    return calculateGasFee(SCCP_WORKCHAIN, SCCP_MAX_TRANSACTION_GAS);
}

fun sccpMaximumForwardFee(bits: int, cells: int): coins {
    return calculateForwardFee(SCCP_WORKCHAIN, bits, cells);
}

fun sccpSmallForwardFee(): coins {
    return sccpMaximumForwardFee(
        SCCP_MAX_SMALL_MESSAGE_BITS,
        SCCP_MAX_SMALL_MESSAGE_CELLS,
    );
}

fun sccpOneWitnessForwardFee(): coins {
    return sccpMaximumForwardFee(
        SCCP_MAX_ONE_WITNESS_MESSAGE_BITS,
        SCCP_MAX_ONE_WITNESS_MESSAGE_CELLS,
    );
}

fun sccpTwoWitnessForwardFee(): coins {
    return sccpMaximumForwardFee(
        SCCP_MAX_TWO_WITNESS_MESSAGE_BITS,
        SCCP_MAX_TWO_WITNESS_MESSAGE_CELLS,
    );
}

fun sccpThreeWitnessForwardFee(): coins {
    return sccpMaximumForwardFee(
        SCCP_MAX_THREE_WITNESS_MESSAGE_BITS,
        SCCP_MAX_THREE_WITNESS_MESSAGE_CELLS,
    );
}

fun sccpFourWitnessForwardFee(): coins {
    return sccpMaximumForwardFee(
        SCCP_MAX_FOUR_WITNESS_MESSAGE_BITS,
        SCCP_MAX_FOUR_WITNESS_MESSAGE_CELLS,
    );
}

fun sccpWalletDeployForwardFee(): coins {
    return sccpMaximumForwardFee(
        SCCP_MAX_WALLET_DEPLOY_MESSAGE_BITS,
        SCCP_MAX_WALLET_DEPLOY_MESSAGE_CELLS,
    );
}

/// The terminal bridge burn handler emits one bounded external log.
fun sccpBurnBridgeCompletionMinimum(): coins {
    return sccpMaximumComputeFee() + sccpSmallForwardFee();
}

/// Wallet consumes the master acknowledgement and funds the terminal bridge.
fun sccpBurnWalletAcknowledgementMinimum(): coins {
    return sccpMaximumComputeFee() + sccpSmallForwardFee() + sccpBurnBridgeCompletionMinimum();
}

fun sccpMasterBurnMinimum(): coins {
    return sccpMaximumComputeFee() + sccpSmallForwardFee() + sccpBurnWalletAcknowledgementMinimum();
}

fun sccpWalletBurnAuthorizationMinimum(): coins {
    return sccpMaximumComputeFee() + sccpOneWitnessForwardFee() + sccpMasterBurnMinimum();
}

fun sccpBridgeBurnPrepareMinimum(): coins {
    return sccpMaximumComputeFee() + sccpTwoWitnessForwardFee() +
    sccpWalletBurnAuthorizationMinimum();
}

fun sccpWalletBurnCommitMinimum(): coins {
    return sccpMaximumComputeFee() + sccpThreeWitnessForwardFee() + sccpBridgeBurnPrepareMinimum();
}

fun sccpWalletBurnHashMinimum(remainingBlocks: int): coins {
    assert (
        remainingBlocks > 0 && remainingBlocks <= SCCP_MAX_TON_TO_TAIRA_HASH_BLOCKS
    ) throw SccpErrors.InvalidMessage;
    return sccpWalletBurnCommitMinimum() +
    remainingBlocks * (sccpMaximumComputeFee() + sccpFourWitnessForwardFee());
}

fun sccpWalletBurnSubgroupMinimum(remainingSteps: int): coins {
    assert (
        remainingSteps > 0 && remainingSteps <= SCCP_ED25519_SUBGROUP_MAX_STEPS
    ) throw SccpErrors.InvalidMessage;
    return sccpWalletBurnHashMinimum(SCCP_MAX_TON_TO_TAIRA_HASH_BLOCKS) +
    remainingSteps * (sccpMaximumComputeFee() + sccpFourWitnessForwardFee());
}

fun sccpWalletBurnRecoveryMinimum(): coins {
    return sccpMaximumComputeFee() + sccpFourWitnessForwardFee() +
    sccpWalletBurnSubgroupMinimum(SCCP_ED25519_SUBGROUP_MAX_STEPS);
}

fun sccpWalletBurnPrepareMinimum(): coins {
    return sccpMaximumComputeFee() + sccpFourWitnessForwardFee() + sccpWalletBurnRecoveryMinimum();
}

fun sccpMintBridgeCompletionMinimum(): coins {
    return sccpMaximumComputeFee();
}

fun sccpMintMasterAcknowledgementMinimum(): coins {
    return sccpMaximumComputeFee() + sccpSmallForwardFee() + sccpMintBridgeCompletionMinimum();
}

fun sccpWalletMintMinimum(): coins {
    // This hop may deploy the canonical wallet. Fund its complete five-year
    // capped-state reserve in the same message; an already deployed wallet
    // retains the excess as an authenticated top-up.
    return sccpWalletStorageReserve() + sccpMaximumComputeFee() + sccpSmallForwardFee() +
    sccpMintMasterAcknowledgementMinimum();
}

fun sccpMasterMintMinimum(): coins {
    return sccpMaximumComputeFee() + sccpWalletDeployForwardFee() + sccpWalletMintMinimum();
}

fun sccpBridgeFinalizeAdmissionMinimum(): coins {
    return sccpMaximumComputeFee() + sccpTwoWitnessForwardFee() + sccpMasterMintMinimum();
}

/// Value needed by the remaining self-authenticated payload-hash blocks and
/// the final proof/replay admission. Every hash stage forwards the complete
/// proof and three bounded witnesses, so it is charged at the largest typed
/// continuation profile.
fun sccpBridgeFinalizeContinuationMinimum(remainingHashBlocks: int): coins {
    assert (
        remainingHashBlocks >= 0 && remainingHashBlocks <= SCCP_MAX_TAIRA_TO_TON_HASH_BLOCKS
    ) throw SccpErrors.InvalidMessage;
    return sccpBridgeFinalizeAdmissionMinimum() +
    remainingHashBlocks * (sccpMaximumComputeFee() + sccpThreeWitnessForwardFee());
}

fun sccpBridgeFinalizeMinimum(): coins {
    return sccpBridgeFinalizeContinuationMinimum(SCCP_MAX_TAIRA_TO_TON_HASH_BLOCKS);
}

fun sccpBridgeRetryMinimum(): coins {
    val mint = sccpMaximumComputeFee() + sccpTwoWitnessForwardFee() + sccpMasterMintMinimum();
    val burn = sccpMaximumComputeFee() + sccpTwoWitnessForwardFee() +
    sccpWalletBurnAuthorizationMinimum();
    return mint > burn ? mint : burn;
}

fun sccpBridgeStorageReserve(): coins {
    return calculateStorageFee(
        SCCP_WORKCHAIN,
        SCCP_STORAGE_RESERVE_SECONDS,
        SCCP_MAX_BRIDGE_ACCOUNT_BITS,
        SCCP_MAX_BRIDGE_ACCOUNT_CELLS,
    );
}

fun sccpMasterStorageReserve(): coins {
    return calculateStorageFee(
        SCCP_WORKCHAIN,
        SCCP_STORAGE_RESERVE_SECONDS,
        SCCP_MAX_MASTER_ACCOUNT_BITS,
        SCCP_MAX_MASTER_ACCOUNT_CELLS,
    );
}

fun sccpWalletStorageReserve(): coins {
    return calculateStorageFee(
        SCCP_WORKCHAIN,
        SCCP_STORAGE_RESERVE_SECONDS,
        SCCP_MAX_WALLET_ACCOUNT_BITS,
        SCCP_MAX_WALLET_ACCOUNT_CELLS,
    );
}

fun requireSccpInboundBudget(value: coins, minimum: coins) {
    assert (value >= minimum) throw SccpErrors.NotEnoughGas;
}

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

/// Checked supply-cap predicate. Subtraction is evaluated only after proving
/// that the persisted supply is not already above its immutable ceiling.
fun sccpMintFitsSupplyCap(totalSupply: coins, mintAmount: coins, cap: coins): bool {
    return cap > 0 && cap <= SCCP_MAX_JETTON_AMOUNT && totalSupply <= cap && mintAmount > 0 &&
    mintAmount <= SCCP_MAX_JETTON_AMOUNT &&
    mintAmount <= cap - totalSupply;
}

/// Every amount crossing the SCCP/Jetton boundary is a positive VarUInteger
/// 16 value and is additionally bounded by the immutable route cap.
fun sccpAmountWithinRouteCap(amount: int, cap: coins): bool {
    return cap > 0 && cap <= SCCP_MAX_JETTON_AMOUNT && amount > 0 &&
    amount <= SCCP_MAX_JETTON_AMOUNT &&
    amount <= cap;
}

/// Checked scale conversion. Division proves multiplication cannot exceed the
/// 120-bit Jetton range or the route cap before the product is evaluated.
fun sccpCheckedScaleAmount(amount: uint128, multiplier: uint64, cap: coins): coins {
    assert (
        multiplier != 0 && cap > 0 && cap <= SCCP_MAX_JETTON_AMOUNT && amount != 0 &&
        amount <= cap / multiplier
    ) throw SccpErrors.InvalidAmount;
    val scaled = amount * multiplier;
    assert (sccpAmountWithinRouteCap(scaled, cap)) throw SccpErrors.InvalidAmount;
    return scaled as coins;
}

/// Guardian keys are immutable, nonzero, distinct, and canonically ordered.
fun mintBreakerGuardiansAreCanonical(guardians: MintBreakerGuardians): bool {
    val tail = guardians.tail.load();
    return guardians.guardian0 != 0 && guardians.guardian0 < guardians.guardian1 &&
    guardians.guardian1 <
    guardians.guardian2 &&
    guardians.guardian2 <
    tail.guardian3 &&
    tail.guardian3 <
    tail.guardian4;
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

/// Namespace the Jetton master's burn replay slot to the authenticated bridge
/// owner. Raw SCCP message ids are public; without this scope, any unrelated
/// wallet owner could burn first under a victim's id and poison its direct
/// owner-wallet burn.
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
    val tail = record.tail.load();
    return {
        queryId: requestQueryId,
        messageId,
        recipient: record.recipient,
        jettonAmount: record.jettonAmount,
        payloadHash: tail.payloadHash,
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

/// Each canonical owner wallet owns a fixed-state monotonic nonce. A request
/// can consume it only when it matches and incrementing cannot overflow.
fun sccpOutboundNonceCanAdvance(expectedNonce: uint64, currentNonce: uint64): bool {
    return expectedNonce == currentNonce && currentNonce != 0xffffffffffffffff;
}

/// Require a retried burn or its bounce to match the debit already reserved
/// under the message id. Callers must resend values from this record.
fun sccpWalletBurnRecordMatches(
    record: WalletBurnRecord,
    lockSequence: uint64,
    queryId: uint64,
    outboundNonce: uint64,
    bridgeAddress: address,
    ownerAddress: address,
    jettonAmount: coins,
    payloadHash: uint256,
    sourceEventDigest: uint256,
): bool {
    val tail = record.tail.load();
    return record.lockSequence == lockSequence && record.queryId == queryId &&
    record.outboundNonce == outboundNonce &&
    record.bridgeAddress == bridgeAddress &&
    record.ownerAddress == ownerAddress &&
    record.jettonAmount == jettonAmount &&
    tail.payloadHash == payloadHash &&
    tail.sourceEventDigest == sourceEventDigest;
}

fun calcDeployedJettonWallet(
    ownerAddress: address,
    masterAddress: address,
    bridgeAddress: address,
    jettonWalletCode: cell,
    routeRevision: uint32,
    routeConfigurationHash: uint256,
    maxWrappedSupply: coins,
): AutoDeployAddress {
    val emptyWalletStorage: WalletStorage = {
        jettonBalance: 0,
        ownerAddress,
        masterAddress,
        replay: emptyWalletReplayState(),
        pending: emptyWalletPendingState(),
        tail: (WalletStorageTail {
            bridgeAddress,
            routeRevision,
            routeConfigurationHash,
            maxWrappedSupply,
            outboundNonce: 0,
            nextLockSequence: 0,
            lockedBurns: [],
            lockedBurnCount: 0,
        }).toCell(),
    };
    return {
        workchain: SCCP_WORKCHAIN,
        stateInit: { code: jettonWalletCode, data: emptyWalletStorage.toCell() },
    };
}

fun calcAddressOfJettonWallet(
    ownerAddress: address,
    masterAddress: address,
    bridgeAddress: address,
    jettonWalletCode: cell,
    routeRevision: uint32,
    routeConfigurationHash: uint256,
    maxWrappedSupply: coins,
): address {
    return calcDeployedJettonWallet(
        ownerAddress,
        masterAddress,
        bridgeAddress,
        jettonWalletCode,
        routeRevision,
        routeConfigurationHash,
        maxWrappedSupply,
    ).calculateAddress();
}
