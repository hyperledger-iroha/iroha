// SccpTairaXorMinter: TEP-74 Jetton master of Taira XOR on TON with the SCCP
// v1 bridge built in (specs/sccp.md §5.1, §5.3).
//
// There is no owner, admin, upgrade path or setter. The minting pause changes
// only through `sccp_apply_control` / `sccp_apply_control_historical` with a
// Parliament control leaf attested by the Taira bridge roster (§5.1.6).
// Replay flags live in `SccpConsumedBucket` children (512 nonces each), which
// the minter activates once per index (§5.3.4 step 2).
//
// Every asynchronous step either completes, bounces back to a handler that
// undoes its effect, or is recorded in `retries` for a permissionless
// `sccp_retry` (§5.3.4). External-out events are sent with bounce on action
// failure, so a failed event aborts and bounces the whole step.

import "@stdlib/gas-payments"
import "@stdlib/strings"
import "sccp-addresses"
import "sccp-cells"
import "sccp-constants"
import "sccp-crypto"
import "sccp-errors"
import "sccp-fees"
import "sccp-messages"
import "sccp-storage"
import "sccp-verify"

contract SccpTairaXorMinter {
    author: "Hyperledger Iroha"
    version: "1.0.0"
    description: "SCCP v1 Taira XOR Jetton master with the bridge built in (TEP-74, TEP-89)"
    storage: MinterStorage
    incomingMessages: MinterMessage
}

type MinterMessage =
    | SccpInit
    | SccpFinalize
    | SccpFinalizeHistorical
    | SccpVoidExpired
    | SccpVoidExpiredHistorical
    | SccpVoidFrozen
    | SccpRotate
    | SccpApplyControl
    | SccpApplyControlHistorical
    | SccpDeployBuckets
    | SccpRetry
    | SccpConsumed
    | SccpAlreadyConsumed
    | SccpConsumeReverted
    | SccpRangeConsumed
    | SccpRangeRejected
    | BurnNotificationForMinter
    | TopUpTons
    | RequestWalletAddress

/// Outgoing messages whose bounces the minter handles (§5.3.4 steps 2, 6, 7).
type MinterBounce = SccpConsume | InternalTransferStep | SccpUnconsume | SccpActivateBucket

/// Members, size and threshold of the roster that signed an attestation.
struct SignerSet {
    members: cell
    n: int
    t: int
}

/// A statement whose signatures verified against an accepted roster.
struct VerifiedAttestation {
    a: Attestation
    tail: AttestationTail
}

// ---------------------------------------------------------------------------
// Shared checks.

/// §5.1.2: the current roster while valid, or the previous one within its
/// grace-capped validity. Members come from storage.
fun acceptedSigners(
    roster: Cell<RosterState>,
    prevRoster: Cell<PrevRoster>?,
    rosterDigest: int,
    nowMs: int,
): SignerSet {
    assert (rosterDigest != 0) throw SccpErrors.RosterNotAccepted;
    val current = roster.load();
    if ((rosterDigest == current.digest) & (nowMs <= current.validUntilMs)) {
        return {
            members: current.members,
            n: current.n,
            t: current.t,
        };
    }
    if (prevRoster != null) {
        val prev = prevRoster!.load();
        if ((rosterDigest == prev.digest) & (prev.digest != 0) & (nowMs <= prev.validUntilMs)) {
            return {
                members: prev.members,
                n: prev.n,
                t: prev.t,
            };
        }
    }
    throw SccpErrors.RosterNotAccepted;
}

/// Parses an attestation, checks the §3.6.1 invariants, the roster
/// acceptance and at least `t` valid signatures over its digest.
fun verifyAttestation(
    networkId: int,
    roster: Cell<RosterState>,
    prevRoster: Cell<PrevRoster>?,
    attestation: Cell<Attestation>,
    signatures: cell,
    nowMs: int,
): VerifiedAttestation {
    val a = attestation.load();
    val tail = a.tail.load();
    sccpCheckAttestation(a, tail);
    val signers = acceptedSigners(roster, prevRoster, tail.rosterDigest, nowMs);
    sccpVerifySignatures(
        sccpAttestationDigest(networkId, a, tail),
        signatures,
        signers.members,
        signers.n,
        signers.t,
    );
    return { a, tail };
}

/// Buckets a consume on bucket `idx` deploys: 0 when it exists, else
/// `idx − deployed + 1` up to `SCCP_MAX_DEPLOY_AHEAD` (§5.3.4 step 2).
fun bucketDeployCount(idx: int, deployed: int): int {
    if (idx < deployed) {
        return 0;
    }
    assert (idx < deployed + SCCP_MAX_DEPLOY_AHEAD) throw SccpErrors.BucketNotDeployed;
    return idx - deployed + 1;
}

/// Activates bucket `index` (bounceable, canonical StateInit attached): the
/// only message that makes a bucket accept flags (§5.3.4 step 2).
fun sendBucketActivation(minter: address, bucketCode: cell, index: int, queryId: int) {
    val activate = createMessage({
        bounce: BounceMode.Only256BitsOfBody,
        dest: sccpBucketDeployment(minter, index, bucketCode),
        value: sccpBucketActivationValue(),
        body: SccpActivateBucket { queryId, index },
    });
    activate.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

/// Activates buckets `[first, end)`, in order.
fun sendBucketActivations(minter: address, bucketCode: cell, first: int, end: int, queryId: int) {
    var i = first;
    while (i < end) {
        sendBucketActivation(minter, bucketCode, i, queryId);
        i += 1;
    }
}

/// Sends `body` to bucket `idx` (never with a StateInit).
fun sendToBucket<TBody>(
    minter: address,
    bucketCode: cell,
    idx: int,
    body: TBody,
    value: coins,
    mode: int,
) {
    val msg = createMessage({
        bounce: BounceMode.Only256BitsOfBody,
        dest: sccpBucketAddress(minter, idx, bucketCode),
        value,
        body,
    });
    msg.send(mode);
}

/// Sends a bounceable `sccp_unconsume{nonce}` (§5.3.4 step 7); its bounce
/// records a retry.
fun sendUnconsume(minter: address, bucketCode: cell, nonce: int, mode: int) {
    sendToBucket(
        minter,
        bucketCode,
        nonce >> SCCP_BUCKET_SHIFT,
        SccpUnconsume { nonce },
        sccpUnconsumeValue(),
        mode,
    );
}

/// Emits an external-out event (§5.3.3). A failed event fails the action
/// phase and bounces the inbound message, so no step completes without it.
fun emitEvent<TBody>(body: TBody) {
    val event = createExternalLogMessage({ dest: createAddressNone(), body });
    event.send(SEND_MODE_REGULAR | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

/// Returns everything above the reserved floor to `response` (TEP-74 excesses).
fun returnExcess(response: address, queryId: int) {
    val excess = createMessage({
        bounce: BounceMode.NoBounce,
        dest: response,
        value: 0,
        body: ReturnExcessesBack { queryId },
    });
    excess.send(SEND_MODE_CARRY_ALL_BALANCE | SEND_MODE_IGNORE_ERRORS);
}

// ---------------------------------------------------------------------------
// Finalize and void (§5.1.3, §5.1.8, §5.3.4 steps 1–2).

fun handleConsumeRequest(
    valueCoins: coins,
    kind: int,
    queryId: int,
    response: address,
    nonceArgument: int,
    attestation: Cell<Attestation>,
    signatures: cell,
    history: Cell<HistoryProof>?,
    message: Cell<MessageProof>,
) {
    var st = lazy MinterStorage.load();
    assert (st.initialized) throw SccpErrors.NotInitialized;
    assert (sccpGlobalId() == SCCP_TON_GLOBAL_ID) throw SccpErrors.WrongChain;
    val isMint = kind == SCCP_CONSUME_KIND_MINT;
    if (isMint) {
        assert (!st.mintingPaused) throw SccpErrors.MintingIsPaused;
    }
    val config = st.config.load();
    val proof = message.load();
    val minter = contract.getAddress();
    val minterHash = minter.accountId();
    val payload = sccpParseInboundPayload(proof.payload, config.routeRevision, minterHash);
    if (!isMint) {
        assert (nonceArgument == payload.nonce) throw SccpErrors.BadNonce;
    }
    val idx = payload.nonce >> SCCP_BUCKET_SHIFT;
    val deployed = st.deployedBuckets;
    val deployCount = bucketDeployCount(idx, deployed);
    val required = isMint
        ? sccpFinalizeRequiredValue(deployCount)
        : sccpVoidRequiredValue(deployCount);
    // §5.3.5: the value is checked before any signature is verified.
    sccpCheckEntryValue(valueCoins, required);

    val nowMs = sccpNowMs();
    val verified = verifyAttestation(
        config.tairaNetworkId,
        st.roster,
        st.prevRoster,
        attestation,
        signatures,
        nowMs,
    );
    val messageId = sccpInboundMessageId(config.tairaNetworkId, payload.payloadHash);
    sccpVerifyLeafInclusion(
        sccpTransferLeaf(messageId, minterHash),
        proof.leafIndex,
        proof.pathLen,
        proof.path,
        verified.a,
        verified.tail,
        history,
    );
    if (isMint) {
        assert (nowMs <= payload.deadlineMs) throw SccpErrors.DeadlinePassed;
    } else {
        assert (nowMs > payload.deadlineMs) throw SccpErrors.DeadlineNotReached;
    }

    var amount = 0;
    var recipient: address? = null;
    if (isMint) {
        amount = payload.amount;
        assert (
            st.totalSupply + st.pendingSupply + amount <= config.maxSupply
        ) throw SccpErrors.SupplyCapExceeded;
        st.pendingSupply += amount;
        recipient = address.fromWorkchainAndHash(SCCP_WORKCHAIN, payload.recipientHash);
    }
    if (deployCount > 0) {
        st.deployedBuckets = idx + 1;
    }
    st.save();

    sccpReserveMinterFloor(valueCoins, required);
    if (deployCount > 0) {
        sendBucketActivations(minter, config.bucketCode, deployed, idx + 1, queryId);
    }
    val tail: ConsumeTail = {
        kind,
        recipient,
        response,
        messageId,
    };
    sendToBucket(
        minter,
        config.bucketCode,
        idx,
        SccpConsume {
            queryId,
            nonce: payload.nonce,
            amount,
            tail: tail.toCell(),
        },
        0,
        SEND_MODE_CARRY_ALL_BALANCE | SEND_MODE_BOUNCE_ON_ACTION_FAIL,
    );
}

// ---------------------------------------------------------------------------
// Parliament controls (§5.1.6).

fun handleApplyControl(
    valueCoins: coins,
    queryId: int,
    response: address,
    attestation: Cell<Attestation>,
    signatures: cell,
    history: Cell<HistoryProof>?,
    control: Cell<ControlProof>,
) {
    var st = lazy MinterStorage.load();
    assert (st.initialized) throw SccpErrors.NotInitialized;
    assert (sccpGlobalId() == SCCP_TON_GLOBAL_ID) throw SccpErrors.WrongChain;
    val required = sccpApplyControlRequiredValue();
    sccpCheckEntryValue(valueCoins, required);
    val nowMs = sccpNowMs();
    val config = st.config.load();
    val verified = verifyAttestation(
        config.tairaNetworkId,
        st.roster,
        st.prevRoster,
        attestation,
        signatures,
        nowMs,
    );
    val proof = control.load();
    val leaf = sccpControlLeaf(
        config.tairaNetworkId,
        contract.getAddress().accountId(),
        config.routeRevision,
        proof.controlNonce,
        proof.paused,
    );
    sccpVerifyLeafInclusion(
        leaf,
        proof.leafIndex,
        proof.pathLen,
        proof.path,
        verified.a,
        verified.tail,
        history,
    );
    assert (proof.controlNonce > st.controlNonce) throw SccpErrors.StaleControl;
    st.mintingPaused = proof.paused;
    st.controlNonce = proof.controlNonce;
    st.opCount += 1;
    st.save();

    sccpReserveMinterFloor(valueCoins, required);
    emitEvent(SccpControlAppliedEvent {
        controlNonce: proof.controlNonce,
        paused: proof.paused,
    });
    returnExcess(response, queryId);
}

// ---------------------------------------------------------------------------
// Message dispatch.

fun onInternalMessage(in: InMessage) {
    val msg = lazy MinterMessage.fromSlice(in.body);

    match (msg) {
        SccpFinalize => {
            handleConsumeRequest(
                in.valueCoins,
                SCCP_CONSUME_KIND_MINT,
                msg.queryId,
                msg.response,
                0,
                msg.attestation,
                msg.signatures,
                null,
                msg.message,
            );
        }

        SccpFinalizeHistorical => {
            handleConsumeRequest(
                in.valueCoins,
                SCCP_CONSUME_KIND_MINT,
                msg.queryId,
                msg.response,
                0,
                msg.attestation,
                msg.signatures,
                msg.history,
                msg.message,
            );
        }

        SccpVoidExpired => {
            handleConsumeRequest(
                in.valueCoins,
                SCCP_CONSUME_KIND_VOID,
                msg.queryId,
                msg.response,
                msg.nonce,
                msg.attestation,
                msg.signatures,
                null,
                msg.message,
            );
        }

        SccpVoidExpiredHistorical => {
            handleConsumeRequest(
                in.valueCoins,
                SCCP_CONSUME_KIND_VOID,
                msg.queryId,
                msg.response,
                msg.nonce,
                msg.attestation,
                msg.signatures,
                msg.history,
                msg.message,
            );
        }

        SccpApplyControl => {
            handleApplyControl(
                in.valueCoins,
                msg.queryId,
                msg.response,
                msg.attestation,
                msg.signatures,
                null,
                msg.control,
            );
        }

        SccpApplyControlHistorical => {
            handleApplyControl(
                in.valueCoins,
                msg.queryId,
                msg.response,
                msg.attestation,
                msg.signatures,
                msg.history,
                msg.control,
            );
        }

        SccpConsumed => {
            var st = lazy MinterStorage.load();
            val config = st.config.load();
            val minter = contract.getAddress();
            val nonce = msg.nonce;
            assert (
                in.senderAddress ==
                sccpBucketAddress(minter, nonce >> SCCP_BUCKET_SHIFT, config.bucketCode)
            ) throw SccpErrors.NotBucket;
            val tail = msg.tail.load();
            // §5.3.4 step 4: the value was checked at entry and the floor was
            // topped up there, so this reply pays only its own step. Any
            // failure bounces to the bucket, which clears the flag and reports
            // `sccp_consume_reverted`.
            sccpReservePriorBalance(in.valueCoins);
            if (tail.kind == SCCP_CONSUME_KIND_MINT) {
                val amount = msg.amount as coins;
                st.pendingSupply -= amount;
                st.totalSupply += amount;
                st.opCount += 1;
                st.save();
                emitEvent(SccpFinalizedEvent {
                    messageId: tail.messageId,
                    nonce,
                });
                val mint = createMessage({
                    bounce: BounceMode.Only256BitsOfBody,
                    dest: sccpWalletDeployment(tail.recipient!, minter, config.walletCode),
                    value: 0,
                    body: InternalTransferStep {
                        queryId: nonce,
                        jettonAmount: amount,
                        transferInitiator: null,
                        sendExcessesTo: tail.response,
                        forwardTonAmount: 0,
                        forwardPayload: beginCell().storeUint(0, 1).toSlice(),
                    },
                });
                mint.send(SEND_MODE_CARRY_ALL_BALANCE | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
            } else {
                st.opCount += 1;
                st.save();
                emitEvent(SccpVoidedEvent {
                    messageId: tail.messageId,
                    firstNonce: nonce,
                    count: 1,
                });
                returnExcess(tail.response, msg.queryId);
            }
        }

        SccpAlreadyConsumed => {
            var st = lazy MinterStorage.load();
            val config = st.config.load();
            val nonce = msg.nonce;
            assert (
                in.senderAddress ==
                sccpBucketAddress(
                    contract.getAddress(),
                    nonce >> SCCP_BUCKET_SHIFT,
                    config.bucketCode,
                )
            ) throw SccpErrors.NotBucket;
            // §5.3.4 step 5; a failure bounces to the bucket, which reports
            // `sccp_consume_reverted`.
            val tail = msg.tail.load();
            if (tail.kind == SCCP_CONSUME_KIND_MINT) {
                st.pendingSupply -= msg.amount;
                st.save();
            }
            sccpReservePriorBalance(in.valueCoins);
            returnExcess(tail.response, msg.queryId);
        }

        SccpConsumeReverted => {
            // §5.3.4 step 4: the minter bounced `sccp_consumed` or
            // `sccp_already_consumed`, so the pending amount was never moved.
            var st = lazy MinterStorage.load();
            val config = st.config.load();
            val nonce = msg.nonce;
            assert (
                in.senderAddress ==
                sccpBucketAddress(
                    contract.getAddress(),
                    nonce >> SCCP_BUCKET_SHIFT,
                    config.bucketCode,
                )
            ) throw SccpErrors.NotBucket;
            st.pendingSupply -= msg.amount;
            st.save();
        }

        SccpRangeConsumed => {
            var st = lazy MinterStorage.load();
            val config = st.config.load();
            val first = msg.first;
            assert (
                in.senderAddress ==
                sccpBucketAddress(
                    contract.getAddress(),
                    first >> SCCP_BUCKET_SHIFT,
                    config.bucketCode,
                )
            ) throw SccpErrors.NotBucket;
            st.opCount += 1;
            st.save();
            // paid by the value attached to the range request; a failure
            // bounces and the bucket clears the range
            emitEvent(SccpVoidedEvent {
                messageId: 0,
                firstNonce: first,
                count: msg.count,
            });
        }

        SccpRangeRejected => {
            val st = lazy MinterStorage.load();
            val config = st.config.load();
            assert (
                in.senderAddress ==
                sccpBucketAddress(
                    contract.getAddress(),
                    msg.first >> SCCP_BUCKET_SHIFT,
                    config.bucketCode,
                )
            ) throw SccpErrors.NotBucket;
            // the value stays in the minter; the excess was returned at request time
        }

        SccpVoidFrozen => {
            var st = lazy MinterStorage.load();
            assert (st.initialized) throw SccpErrors.NotInitialized;
            val nowMs = sccpNowMs();
            val roster = st.roster.load();
            assert (nowMs > roster.validUntilMs) throw SccpErrors.NotFrozen;
            if (st.prevRoster != null) {
                assert (nowMs > st.prevRoster!.load().validUntilMs) throw SccpErrors.NotFrozen;
            }
            val first = msg.firstNonce;
            val count = msg.count;
            assert ((count >= 1) & (count <= SCCP_BUCKET_BITS)) throw SccpErrors.BadRange;
            val idx = first >> SCCP_BUCKET_SHIFT;
            assert (((first + count - 1) >> SCCP_BUCKET_SHIFT) == idx) throw SccpErrors.BadRange;
            val deployed = st.deployedBuckets;
            val deployCount = bucketDeployCount(idx, deployed);
            val required = sccpVoidFrozenRequiredValue(deployCount);
            sccpCheckEntryValue(in.valueCoins, required);
            if (deployCount > 0) {
                st.deployedBuckets = idx + 1;
                st.save();
            }
            val config = st.config.load();
            val minter = contract.getAddress();
            sccpReserveMinterFloor(in.valueCoins, required);
            if (deployCount > 0) {
                sendBucketActivations(minter, config.bucketCode, deployed, idx + 1, msg.queryId);
            }
            sendToBucket(
                minter,
                config.bucketCode,
                idx,
                SccpConsumeRange {
                    queryId: msg.queryId,
                    first,
                    count,
                },
                sccpRangeConsumeValue(),
                SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL,
            );
            returnExcess(msg.response, msg.queryId);
        }

        SccpRotate => {
            var st = lazy MinterStorage.load();
            assert (st.initialized) throw SccpErrors.NotInitialized;
            val required = sccpRotateRequiredValue();
            sccpCheckEntryValue(in.valueCoins, required);
            val nowMs = sccpNowMs();
            val config = st.config.load();
            val roster = st.roster.load();
            val a = msg.attestation.load();
            val tail = a.tail.load();
            sccpCheckAttestation(a, tail);
            // §5.1.5 step 1: the current roster only.
            assert (tail.rosterDigest != 0) throw SccpErrors.RosterNotAccepted;
            assert (tail.rosterDigest == roster.digest) throw SccpErrors.RosterNotAccepted;
            assert (nowMs <= roster.validUntilMs) throw SccpErrors.RosterNotAccepted;
            sccpVerifySignatures(
                sccpAttestationDigest(config.tairaNetworkId, a, tail),
                msg.signatures,
                roster.members,
                roster.n,
                roster.t,
            );
            // Step 2.
            assert (tail.nextRosterDigest != 0) throw SccpErrors.BadRotation;
            val next = msg.next.load();
            val nextDigest = sccpRosterDigest(
                config.tairaNetworkId,
                next.generation,
                next.validFromMs,
                next.validUntilMs,
                next.n,
                next.t,
                next.members,
            );
            assert (nextDigest == tail.nextRosterDigest) throw SccpErrors.BadRotation;
            // Step 3.
            assert (next.generation == roster.generation + 1) throw SccpErrors.BadRotation;
            assert (next.validFromMs == a.timestampMs) throw SccpErrors.BadRotation;
            sccpCheckRosterValidity(next.validFromMs, next.validUntilMs, nowMs);
            // Step 4. `next.members` is stored as sent: `sccpRosterDigest` walked
            // every chunk and rejected exotic cells, so it is canonical and ordinary.
            val prev: PrevRoster = {
                digest: roster.digest,
                validUntilMs: min(roster.validUntilMs, nowMs + SCCP_PREVIOUS_ROSTER_GRACE_MS),
                n: roster.n,
                t: roster.t,
                members: roster.members,
            };
            val installed: RosterState = {
                digest: nextDigest,
                generation: next.generation,
                validFromMs: next.validFromMs,
                validUntilMs: next.validUntilMs,
                n: next.n,
                t: next.t,
                members: next.members,
            };
            st.prevRoster = prev.toCell();
            st.roster = installed.toCell();
            st.save();
            sccpReserveMinterFloor(in.valueCoins, required);
            emitEvent(SccpRosterRotatedEvent {
                generation: next.generation,
                digest: nextDigest,
                validUntilMs: next.validUntilMs,
            });
            returnExcess(msg.response, msg.queryId);
        }

        SccpDeployBuckets => {
            var st = lazy MinterStorage.load();
            assert (st.initialized) throw SccpErrors.NotInitialized;
            val count = msg.count;
            assert (
                (count >= 1) & (count <= SCCP_MAX_DEPLOY_BUCKETS_PER_CALL)
            ) throw SccpErrors.BadRange;
            val required = sccpDeployBucketsRequiredValue(count);
            sccpCheckEntryValue(in.valueCoins, required);
            val first = st.deployedBuckets;
            st.deployedBuckets = first + count;
            st.save();
            val config = st.config.load();
            sccpReserveMinterFloor(in.valueCoins, required);
            sendBucketActivations(
                contract.getAddress(),
                config.bucketCode,
                first,
                first + count,
                msg.queryId,
            );
            returnExcess(msg.response, msg.queryId);
        }

        SccpRetry => {
            // §5.3.4: re-runs a recorded step whose message bounced. The entry
            // is removed before the resend and recorded again only if the
            // resend bounces, so each recorded step runs at most once.
            var st = lazy MinterStorage.load();
            assert (st.initialized) throw SccpErrors.NotInitialized;
            val required = sccpRetryRequiredValue();
            sccpCheckEntryValue(in.valueCoins, required);
            val nonce = msg.nonce;
            val entry = st.retries.get(nonce);
            assert (entry.isFound) throw SccpErrors.NothingToRetry;
            val kind = entry.loadValue();
            st.retries.delete(nonce);
            st.save();
            val config = st.config.load();
            val minter = contract.getAddress();
            sccpReserveMinterFloor(in.valueCoins, required);
            if (kind == SCCP_RETRY_ACTIVATE) {
                sendBucketActivation(
                    minter,
                    config.bucketCode,
                    nonce >> SCCP_BUCKET_SHIFT,
                    msg.queryId,
                );
            } else {
                sendUnconsume(
                    minter,
                    config.bucketCode,
                    nonce,
                    SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL,
                );
            }
            returnExcess(msg.response, msg.queryId);
        }

        BurnNotificationForMinter => {
            var st = lazy MinterStorage.load();
            assert (st.initialized) throw SccpErrors.NotInitialized;
            val config = st.config.load();
            val owner = msg.burnInitiator;
            assert (
                in.senderAddress ==
                sccpWalletAddress(owner, contract.getAddress(), config.walletCode)
            ) throw SccpErrors.NotValidWallet;
            assert (sccpIsBasechainStd(owner)) throw SccpErrors.WrongWorkchain;
            val amount = msg.jettonAmount;
            assert ((amount > 0) & (amount < SCCP_TON_AMOUNT_LIMIT)) throw SccpErrors.BadAmount;
            val burn = msg.sccp.load();
            val (recipientChunks, recipientLen) = sccpParseSnakeBytes(
                burn.recipient,
                SCCP_MAX_TAIRA_ACCOUNT_BYTES,
                SccpErrors.BadRecipient,
            );

            val nonce = st.outboundNonce;
            val payload = sccpBuildOutboundPayload(
                nonce,
                config.routeRevision,
                amount,
                owner.accountId(),
                recipientChunks,
                recipientLen,
            );
            val messageId = sccpOutboundMessageId(config.tairaNetworkId, payload.payloadHash);
            st.totalSupply -= amount;
            st.outboundNonce = nonce + 1;
            st.opCount += 1;
            st.save();

            // §5.3.5: a burn never pays toward the minter floor, and a failed
            // event bounces `burn_notification`, which restores the wallet.
            sccpReservePriorBalance(in.valueCoins);
            emitEvent(SccpTransferToTairaEvent {
                messageId,
                nonce,
                sender: owner,
                amount,
                payload: payload.cell,
            });
            if (msg.sendExcessesTo != null) {
                returnExcess(msg.sendExcessesTo!, msg.queryId);
            }
        }

        SccpInit => {
            var st = MinterStorage.load();
            assert (!st.initialized) throw SccpErrors.AlreadyInitialized;
            assert (sccpGlobalId() == SCCP_TON_GLOBAL_ID) throw SccpErrors.WrongChain;
            // Canonical initial data (§5.3.1).
            assert (
                (st.totalSupply == 0) & (st.pendingSupply == 0) & (st.outboundNonce == 0) &
                (st.opCount == 0) &
                (st.controlNonce == 0) &
                (!st.mintingPaused) &
                (st.deployedBuckets == 0) &
                (st.prevRoster == null) &
                st.retries.isEmpty()
            ) throw SccpErrors.BadConfig;
            val config = st.config.load();
            assert (
                (config.tairaNetworkId != 0) & (config.routeRevision != 0) & (config.maxSupply != 0)
            ) throw SccpErrors.BadConfig;
            val roster = st.roster.load();
            val digest = sccpRosterDigest(
                config.tairaNetworkId,
                roster.generation,
                roster.validFromMs,
                roster.validUntilMs,
                roster.n,
                roster.t,
                roster.members,
            );
            assert (
                (digest == roster.digest) & (digest == config.initialDigest)
            ) throw SccpErrors.BadRoster;
            assert (roster.generation == config.initialGeneration) throw SccpErrors.BadRoster;
            sccpCheckRosterValidity(roster.validFromMs, roster.validUntilMs, sccpNowMs());
            st.initialized = true;
            st.save();
        }

        RequestWalletAddress => {
            val st = lazy MinterStorage.load();
            val config = st.config.load();
            val ownerAddress: Cell<address>? = msg.includeOwnerAddress
                ? msg.ownerAddress.toCell()
                : null;
            var walletAddress: address? = null;
            if (sccpIsBasechainStd(msg.ownerAddress)) {
                walletAddress = sccpWalletAddress(
                    msg.ownerAddress,
                    contract.getAddress(),
                    config.walletCode,
                );
            }
            val respondMsg = createMessage({
                bounce: BounceMode.NoBounce,
                dest: in.senderAddress,
                value: 0,
                body: ResponseWalletAddress {
                    queryId: msg.queryId,
                    jettonWalletAddress: walletAddress,
                    ownerAddress,
                },
            });
            respondMsg.send(
                SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_BOUNCE_ON_ACTION_FAIL,
            );
        }

        TopUpTons => {
            // anyone may add to the minter floor (§5.3.5)
        }

        else => {
            assert (in.body.isEmpty()) throw SccpErrors.InvalidMessage;
        }
    }
}

/// Records `kind` for `key` in `retries` (§5.3.4).
fun MinterStorage.recordRetry(mutate self, key: int, kind: int) {
    self.retries.set(key, kind);
}

/// §5.3.4 step 6 (bounced `sccp_consume`: release the pending supply; the
/// value stays), step 7 (bounced `internal_transfer`: burn the unminted supply
/// and ask the bucket to clear the flag, so the message becomes finalizable
/// again) and the recorded retries: a bounced `sccp_unconsume` or
/// `sccp_activate_bucket` changed nothing at the bucket, so the step is
/// recorded for `sccp_retry`.
fun onBouncedMessage(in: InMessageBounced) {
    // A bounce may carry little value. Only the network sets the bounced flag,
    // and only for a message the minter sent, so the minter pays this handler.
    setGasLimitToMaximum();
    in.bouncedBody.skipBouncedPrefix();
    val msg = lazy MinterBounce.fromSlice(in.bouncedBody);

    match (msg) {
        SccpConsume => {
            var st = lazy MinterStorage.load();
            val config = st.config.load();
            val nonce = msg.nonce;
            assert (
                in.senderAddress ==
                sccpBucketAddress(
                    contract.getAddress(),
                    nonce >> SCCP_BUCKET_SHIFT,
                    config.bucketCode,
                )
            ) throw SccpErrors.NotBucket;
            st.pendingSupply -= msg.amount;
            st.save();
        }
        InternalTransferStep => {
            // Only the network sets the bounced flag, so this is a mint the
            // minter sent to a canonical wallet (`query_id = nonce`).
            var st = lazy MinterStorage.load();
            val config = st.config.load();
            val nonce = msg.queryId;
            st.totalSupply -= msg.jettonAmount;
            // The bounce handler cannot bounce: send the unconsume only when the
            // balance surely pays it, and otherwise record it for `sccp_retry`.
            if (
                contract.getOriginalBalance() >=
                sccpUnconsumeCost() + sccpGasFee(SCCP_GAS_MINTER_BOUNCE)
            ) {
                sendUnconsume(
                    contract.getAddress(),
                    config.bucketCode,
                    nonce,
                    SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_IGNORE_ERRORS,
                );
            } else {
                st.recordRetry(nonce, SCCP_RETRY_UNCONSUME);
            }
            st.save();
        }
        SccpUnconsume => {
            var st = lazy MinterStorage.load();
            val config = st.config.load();
            val nonce = msg.nonce;
            assert (
                in.senderAddress ==
                sccpBucketAddress(
                    contract.getAddress(),
                    nonce >> SCCP_BUCKET_SHIFT,
                    config.bucketCode,
                )
            ) throw SccpErrors.NotBucket;
            st.recordRetry(nonce, SCCP_RETRY_UNCONSUME);
            st.save();
        }
        SccpActivateBucket => {
            var st = lazy MinterStorage.load();
            val config = st.config.load();
            val index = msg.index;
            assert (
                in.senderAddress ==
                sccpBucketAddress(contract.getAddress(), index, config.bucketCode)
            ) throw SccpErrors.NotBucket;
            st.recordRetry(index << SCCP_BUCKET_SHIFT, SCCP_RETRY_ACTIVATE);
            st.save();
        }
        else => {
            // range requests, reverted reports and excess returns: the value stays
        }
    }
}

// ---------------------------------------------------------------------------
// Get methods (§5.3.6).

/// TEP-64 on-chain content built at run time from constants (§5.3.1).
fun buildJettonContent(): Cell<OnchainMetadataReply> {
    var metadata: OnchainMetadataReply = { contentDict: [] };
    metadata.contentDict.set("name".sha256(), "Taira XOR".prefixWith00());
    metadata.contentDict.set("symbol".sha256(), "tXOR".prefixWith00());
    metadata.contentDict.set("decimals".sha256(), "9".prefixWith00());
    return metadata.toCell();
}

get fun get_jetton_data(): JettonDataReply {
    val st = lazy MinterStorage.load();
    val config = st.config.load();
    return {
        totalSupply: st.totalSupply,
        mintable: true,
        adminAddress: null,
        jettonContent: buildJettonContent(),
        jettonWalletCode: config.walletCode,
    };
}

get fun get_wallet_address(ownerAddress: address): address {
    val st = lazy MinterStorage.load();
    val config = st.config.load();
    return sccpWalletAddress(ownerAddress, contract.getAddress(), config.walletCode);
}

get fun get_sccp_state(): SccpStateReply {
    val st = MinterStorage.load();
    val config = st.config.load();
    val roster = st.roster.load();
    var prevDigest = 0;
    var prevValidUntilMs = 0;
    if (st.prevRoster != null) {
        val prev = st.prevRoster!.load();
        prevDigest = prev.digest;
        prevValidUntilMs = prev.validUntilMs;
    }
    return {
        initialized: st.initialized,
        tairaNetworkId: config.tairaNetworkId,
        routeRevision: config.routeRevision,
        initialGeneration: config.initialGeneration,
        initialDigest: config.initialDigest,
        digest: roster.digest,
        generation: roster.generation,
        validUntilMs: roster.validUntilMs,
        prevDigest,
        prevValidUntilMs,
        outboundNonce: st.outboundNonce,
        opCount: st.opCount,
        mintingPaused: st.mintingPaused,
        controlNonce: st.controlNonce,
        totalSupply: st.totalSupply,
        pendingSupply: st.pendingSupply,
        maxSupply: config.maxSupply,
        deployedBuckets: st.deployedBuckets,
    };
}

/// Members of the current roster, in §3.7 order.
get fun get_sccp_members(): array<int> {
    val st = lazy MinterStorage.load();
    val roster = st.roster.load();
    val (members, _) = sccpLoadMembers(roster.members, roster.n);
    return members;
}

get fun get_bucket_address(index: int): address {
    val st = lazy MinterStorage.load();
    val config = st.config.load();
    return sccpBucketAddress(contract.getAddress(), index, config.bucketCode);
}

/// Deploy count a consume of `nonce` would need now (at most 4 ahead).
fun deployCountForNonce(nonce: int): int {
    val st = lazy MinterStorage.load();
    return bucketDeployCount(nonce >> SCCP_BUCKET_SHIFT, st.deployedBuckets);
}

/// `finalize_required_value()` for the message with outbound `nonce` (§5.3.5).
get fun finalize_required_value(nonce: int): int {
    return sccpMinterQuote(sccpFinalizeRequiredValue(deployCountForNonce(nonce)));
}

/// `void_required_value()` for the message with outbound `nonce` (§5.3.5).
get fun void_required_value(nonce: int): int {
    return sccpMinterQuote(sccpVoidRequiredValue(deployCountForNonce(nonce)));
}

/// Required value of `sccp_void_frozen` starting at `firstNonce`.
get fun void_frozen_required_value(firstNonce: int): int {
    return sccpMinterQuote(sccpVoidFrozenRequiredValue(deployCountForNonce(firstNonce)));
}

/// Required value of `sccp_deploy_buckets` for `count` buckets.
get fun deploy_buckets_required_value(count: int): int {
    return sccpMinterQuote(sccpDeployBucketsRequiredValue(count));
}

/// Required value of `sccp_apply_control` and `sccp_apply_control_historical`.
get fun apply_control_required_value(): int {
    return sccpMinterQuote(sccpApplyControlRequiredValue());
}

/// Required value of one `sccp_rotate`.
get fun rotate_required_value(): int {
    return sccpMinterQuote(sccpRotateRequiredValue());
}

/// Required value of `sccp_retry`.
get fun retry_required_value(): int {
    return sccpMinterQuote(sccpRetryRequiredValue());
}

/// `MINTER_FLOOR` at current prices (§5.3.5).
get fun minter_floor(): int {
    return sccpMinterFloor();
}

/// The retry recorded for `nonce`: 0 (none), `SCCP_RETRY_UNCONSUME` (1) or
/// `SCCP_RETRY_ACTIVATE` (2, key = first nonce of the bucket).
get fun get_sccp_retry(nonce: int): int {
    val st = lazy MinterStorage.load();
    val entry = st.retries.get(nonce);
    return entry.isFound ? entry.loadValue() : 0;
}
