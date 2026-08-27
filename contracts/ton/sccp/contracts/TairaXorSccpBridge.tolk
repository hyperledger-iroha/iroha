import "@gen/TairaXorJettonWallet.code"
import "constants"
import "errors"
import "jetton-utils"
import "messages"
import "proof-verifier"
import "sccp-codec"
import "storage"

contract TairaXorSccpBridge {
    author: "Hyperledger Iroha"
    version: "1.0.0"
    description: "Immutable exact-route TON/Taira XOR SCCP bridge"
    incomingMessages: AllowedBridgeMessage
    storage: BridgeStorage
}

type AllowedBridgeMessage =
    | SccpFinalizeFromTaira
    | TransferNotificationForRecipient
    | SccpMasterMintAck
    | SccpMasterBurnAck
    | SccpWalletRefundAck
    | SccpRetry
    | TopUpTons

type BridgeBounceMessage = SccpMasterMint | SccpWalletBurn | SccpRefund

fun assertBridgeConfiguration(storage: BridgeStorage): BridgeConfig {
    assert (storage.storageVersion == SCCP_STORAGE_VERSION) throw SccpErrors.InvalidConfiguration;
    val config = storage.config.load();
    val hashes = config.routeHashes.load();
    val hashTail = hashes.tail.load();
    val finality = config.finalityHashes.load();
    val finalityTail = finality.tail.load();
    assert (getNetworkGlobalId() == config.expectedGlobalId) throw SccpErrors.WrongNetwork;
    assert (
        storage.routeConfigurationHash == hashes.routeConfigurationHash &&
        config.routeRevision != 0 &&
        config.tairaToTonMultiplier == SCCP_TAIRA_TO_TON_MULTIPLIER &&
        hashes.sourceLaneHash == sccpLaneHash(config.sourceLaneBytes) &&
        hashes.destinationLaneHash == sccpLaneHash(config.destinationLaneBytes) &&
        finalityTail.verifyingKey.hash() == finalityTail.verifyingKeyCellHash &&
        canonicalBls12381VerifyingKeyHash(finalityTail.verifyingKey) == finality.verifyingKeyHash
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
        finalityTail.verifyingKeyCellHash != 0
    ) throw SccpErrors.InvalidConfiguration;
    return config;
}

fun custodyWalletAddress(storage: BridgeStorage): address {
    return calcAddressOfJettonWallet(
        contract.getAddress(),
        storage.masterAddress,
        tairaXorJettonWalletCompiledCode(),
    );
}

fun dispatchMasterMint(storage: BridgeStorage, messageId: uint256, pending: PendingMint) {
    val request = createMessage({
        bounce: BounceMode.Only256BitsOfBody,
        dest: storage.masterAddress,
        value: SCCP_OPERATION_VALUE,
        body: SccpMasterMint {
            queryId: pending.queryId,
            messageId,
            recipient: pending.recipient,
            jettonAmount: pending.jettonAmount,
            tail: (SccpMasterMintTail {
                payloadHash: pending.payloadHash,
                statementHash: pending.statementHash,
                publicSignals: pending.publicSignals,
                proof: pending.proof,
                canonicalPayload: pending.canonicalPayload,
            }).toCell(),
        },
    });
    request.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun dispatchWalletBurn(storage: BridgeStorage, messageId: uint256, pending: PendingBurn) {
    val request = createMessage({
        bounce: BounceMode.Only256BitsOfBody,
        dest: custodyWalletAddress(storage),
        value: SCCP_OPERATION_VALUE,
        body: SccpWalletBurn {
            queryId: pending.queryId,
            messageId,
            jettonAmount: pending.jettonAmount,
            payloadHash: pending.payloadHash,
            sourceEventDigest: pending.sourceEventDigest,
        },
    });
    request.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun dispatchRefund(storage: BridgeStorage, refundId: uint256, pending: PendingRefund) {
    val request = createMessage({
        bounce: BounceMode.Only256BitsOfBody,
        dest: custodyWalletAddress(storage),
        value: SCCP_OPERATION_VALUE,
        body: SccpRefund {
            queryId: pending.queryId,
            refundId,
            recipientOwner: pending.recipientOwner,
            jettonAmount: pending.jettonAmount,
        },
    });
    request.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun refundIdentity(queryId: uint64, owner: address, amount: coins): uint256 {
    val ownerBytes = canonicalTonAddress36(owner);
    val preimage = beginCell()
        .storeSlice("sccp:ton:refund:v1".literalSlice())
        .storeUint(queryId, 64)
        .storeSlice(ownerBytes.beginParse())
        .storeCoins(amount)
        .endCell();
    val id = keccak256One(preimage.beginParse());
    assert (id != 0) throw SccpErrors.InvalidMessage;
    return id;
}

fun reserveRefund(
    mutate storage: BridgeStorage,
    queryId: uint64,
    owner: address,
    amount: coins,
) {
    val refundId = refundIdentity(queryId, owner, amount);
    var pendingState = storage.pending.load();
    if (!pendingState.refunds.exists(refundId)) {
        pendingState
            .refunds
            .set(
                refundId,
                (PendingRefund {
                    queryId,
                    recipientOwner: owner,
                    jettonAmount: amount,
                }).toCell(),
            );
        storage.pending = pendingState.toCell();
        storage.save();
    }
    dispatchRefund(storage, refundId, pendingState.refunds.mustGet(refundId).load());
}

fun onBouncedMessage(in: InMessageBounced) {
    // The operation remains reserved. `SccpRetry` is permissionless and resends the exact record;
    // no balance or replay state is rolled back merely because a callback bounced.
    in.bouncedBody.skipBouncedPrefix();
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedBridgeMessage.fromSlice(in.body);
    match (msg) {
        SccpFinalizeFromTaira => {
            assert (in.valueCoins >= 2 * SCCP_OPERATION_VALUE) throw SccpErrors.NotEnoughGas;
            var storage = BridgeStorage.load();
            val config = assertBridgeConfiguration(storage);
            val parsed = parseTairaToTonPayload(msg.canonicalPayload, config.routeRevision);
            val payload = decodeCanonicalPayloadCells(msg.canonicalPayload);
            val payloadHash = sccpPayloadHash(payload);
            val messageId = sccpMessageId(config.destinationLaneBytes, payload);
            assert (messageId == msg.messageId) throw SccpErrors.InvalidMessage;
            val hashes = config.routeHashes.load();
            val hashTail = hashes.tail.load();
            val finality = config.finalityHashes.load();
            val finalityTail = finality.tail.load();
            val policy: DestinationProofPolicy = {
                messageId,
                payloadHash,
                statementHash: msg.statementHash,
                routeConfigurationHash: hashes.routeConfigurationHash,
                destinationBindingHash: hashTail.destinationBindingHash,
                semanticProofProfileHash: hashTail.semanticProofProfileHash,
                soraFinalityAnchorHash: finality.soraFinalityAnchorHash,
                verifyingKeyHash: finality.verifyingKeyHash,
                verifyingKeyCellHash: finalityTail.verifyingKeyCellHash,
            };
            assert (
                verifyTairaDestinationProof(
                    policy,
                    msg.publicSignals,
                    msg.proof,
                    finalityTail.verifyingKey,
                )
            ) throw SccpErrors.ProofRejected;

            var replay = storage.replay.load();
            var pendingState = storage.pending.load();
            if (!replay.inbound.exists(messageId)) {
                val amount = parsed.amount * config.tairaToTonMultiplier;
                val pending: PendingMint = {
                    queryId: msg.queryId,
                    recipient: parsed.recipient,
                    jettonAmount: amount,
                    payloadHash,
                    statementHash: msg.statementHash,
                    publicSignals: msg.publicSignals,
                    proof: msg.proof,
                    canonicalPayload: msg.canonicalPayload,
                };
                replay.inbound.set(messageId, SCCP_PENDING_MINT);
                pendingState.mints.set(messageId, pending.toCell());
                storage.replay = replay.toCell();
                storage.pending = pendingState.toCell();
                storage.save();
                reserveToncoinsOnBalance(SCCP_MIN_BRIDGE_RESERVE, RESERVE_MODE_AT_MOST);
                dispatchMasterMint(storage, messageId, pending);
            } else if (replay.inbound.mustGet(messageId) == SCCP_PENDING_MINT) {
                assert (
                    pendingState.mints.exists(messageId)
                ) throw SccpErrors.UnknownPendingOperation;
                dispatchMasterMint(
                    storage,
                    messageId,
                    pendingState.mints.mustGet(messageId).load(),
                );
            } else {
                throw SccpErrors.Replay;
            }
        }

        TransferNotificationForRecipient => {
            var storage = BridgeStorage.load();
            val config = assertBridgeConfiguration(storage);
            assert (
                in.senderAddress == custodyWalletAddress(storage)
            ) throw SccpErrors.NotValidWallet;
            assert (
                msg.transferInitiator != null && msg.jettonAmount != 0
            ) throw SccpErrors.InvalidMessage;
            val owner = msg.transferInitiator!;
            val payloadSlice = decodeForwardPayload(msg.forwardPayload);
            val exactShape = payloadSlice.remainingBitsCount() == 32 + 16 + 64 &&
            payloadSlice.remainingRefsCount() == 1 &&
            payloadSlice.preloadUint(32) == 0x53434342;
            if (!exactShape) {
                reserveRefund(mutate storage, msg.queryId, owner, msg.jettonAmount);
                return;
            }
            val request = SccpOutboundBurnRequest.fromSlice(payloadSlice);
            if (
                request.version != SCCP_MESSAGE_SCHEMA_VERSION ||
                !isCanonicalTairaRecipient(request.tairaRecipient)
            ) {
                reserveRefund(mutate storage, msg.queryId, owner, msg.jettonAmount);
                return;
            }

            val currentNonce = storage.nonces.exists(owner)
                ? storage.nonces.mustGet(owner)
                : 0 as uint64;
            assert (request.expectedNonce == currentNonce) throw SccpErrors.InvalidNonce;
            assert (currentNonce != 0xffffffffffffffff) throw SccpErrors.NonceExhausted;
            val canonical = buildTonToTairaPayload(
                owner,
                request.tairaRecipient,
                currentNonce,
                config.routeRevision,
                msg.jettonAmount as uint128,
            );
            val payloadHash = sccpPayloadHash(canonical);
            val messageId = sccpMessageId(config.sourceLaneBytes, canonical);
            val laneHash = config.routeHashes.load().sourceLaneHash;
            val sourceEventDigest = sccpSourceEventDigest(laneHash, messageId, payloadHash);
            var replay = storage.replay.load();
            assert (!replay.outbound.exists(messageId)) throw SccpErrors.Replay;
            var pendingState = storage.pending.load();
            val canonicalCell = canonical.toCell();
            val pending: PendingBurn = {
                queryId: msg.queryId,
                ownerAddress: owner,
                jettonAmount: msg.jettonAmount,
                payloadHash,
                sourceEventDigest,
                canonicalPayload: canonicalCell,
            };
            replay.outbound.set(messageId, SCCP_PENDING_BURN);
            pendingState.burns.set(messageId, pending.toCell());
            storage.nonces.set(owner, currentNonce + 1);
            storage.replay = replay.toCell();
            storage.pending = pendingState.toCell();
            storage.save();
            dispatchWalletBurn(storage, messageId, pending);
        }

        SccpMasterMintAck => {
            var storage = BridgeStorage.load();
            assertBridgeConfiguration(storage);
            assert (in.senderAddress == storage.masterAddress) throw SccpErrors.NotMaster;
            var replay = storage.replay.load();
            var pendingState = storage.pending.load();
            assert (
                pendingState.mints.exists(msg.messageId) && replay.inbound.exists(msg.messageId) &&
                replay.inbound.mustGet(msg.messageId) == SCCP_PENDING_MINT
            ) throw SccpErrors.InvalidCallback;
            val pending = pendingState.mints.mustGet(msg.messageId).load();
            assert (
                pending.queryId == msg.queryId && pending.recipient == msg.recipient &&
                pending.jettonAmount == msg.jettonAmount &&
                pending.payloadHash == msg.payloadHash
            ) throw SccpErrors.InvalidCallback;
            replay.inbound.set(msg.messageId, SCCP_COMPLETED);
            pendingState.mints.delete(msg.messageId);
            storage.replay = replay.toCell();
            storage.pending = pendingState.toCell();
            storage.save();
        }

        SccpMasterBurnAck => {
            var storage = BridgeStorage.load();
            val config = assertBridgeConfiguration(storage);
            assert (in.senderAddress == storage.masterAddress) throw SccpErrors.NotMaster;
            var replay = storage.replay.load();
            var pendingState = storage.pending.load();
            assert (
                pendingState.burns.exists(msg.messageId) && replay.outbound.exists(msg.messageId) &&
                replay.outbound.mustGet(msg.messageId) == SCCP_PENDING_BURN
            ) throw SccpErrors.InvalidCallback;
            val pending = pendingState.burns.mustGet(msg.messageId).load();
            assert (
                pending.queryId == msg.queryId && pending.jettonAmount == msg.jettonAmount &&
                pending.payloadHash == msg.payloadHash &&
                pending.sourceEventDigest == msg.sourceEventDigest
            ) throw SccpErrors.InvalidCallback;
            replay.outbound.set(msg.messageId, SCCP_COMPLETED);
            pendingState.burns.delete(msg.messageId);
            storage.replay = replay.toCell();
            storage.pending = pendingState.toCell();
            storage.save();

            val payload = decodeCanonicalPayloadCells(pending.canonicalPayload);
            val event = createExternalLogMessage({
                dest: createAddressNone(),
                body: SccpOutboundEvent {
                    version: SCCP_MESSAGE_SCHEMA_VERSION,
                    laneHash: config.routeHashes.load().sourceLaneHash,
                    messageId: msg.messageId,
                    payloadHash: msg.payloadHash,
                    tail: (SccpOutboundEventTail {
                        sourceEventDigest: msg.sourceEventDigest,
                        canonicalPayloadSnake: canonicalPayloadSnake(payload),
                    }).toCell(),
                },
            });
            event.send(SEND_MODE_REGULAR);
        }

        SccpWalletRefundAck => {
            var storage = BridgeStorage.load();
            assertBridgeConfiguration(storage);
            var pendingState = storage.pending.load();
            assert (pendingState.refunds.exists(msg.refundId)) throw SccpErrors.InvalidCallback;
            val pending = pendingState.refunds.mustGet(msg.refundId).load();
            assert (
                pending.queryId == msg.queryId && pending.recipientOwner == msg.recipientOwner &&
                pending.jettonAmount == msg.jettonAmount &&
                in.senderAddress ==
                calcAddressOfJettonWallet(
                    msg.recipientOwner,
                    storage.masterAddress,
                    tairaXorJettonWalletCompiledCode(),
                )
            ) throw SccpErrors.InvalidCallback;
            pendingState.refunds.delete(msg.refundId);
            storage.pending = pendingState.toCell();
            storage.save();
        }

        SccpRetry => {
            val storage = BridgeStorage.load();
            assertBridgeConfiguration(storage);
            val pendingState = storage.pending.load();
            if (pendingState.mints.exists(msg.messageId)) {
                dispatchMasterMint(
                    storage,
                    msg.messageId,
                    pendingState.mints.mustGet(msg.messageId).load(),
                );
                return;
            }
            if (pendingState.burns.exists(msg.messageId)) {
                dispatchWalletBurn(
                    storage,
                    msg.messageId,
                    pendingState.burns.mustGet(msg.messageId).load(),
                );
                return;
            }
            if (pendingState.refunds.exists(msg.messageId)) {
                dispatchRefund(
                    storage,
                    msg.messageId,
                    pendingState.refunds.mustGet(msg.messageId).load(),
                );
                return;
            }
            throw SccpErrors.UnknownPendingOperation;
        }

        TopUpTons => {}
        else => throw 0xFFFF,
    }
}

get fun get_sccp_route_configuration_hash(): uint256 {
    val storage = BridgeStorage.load();
    assert (storage.storageVersion == SCCP_STORAGE_VERSION) throw SccpErrors.InvalidConfiguration;
    return storage.routeConfigurationHash;
}

get fun get_sccp_nonce(owner: address): uint64 {
    val nonces = BridgeStorage.load().nonces;
    return nonces.exists(owner) ? nonces.mustGet(owner) : 0 as uint64;
}

get fun get_sccp_replay_status(messageId: uint256, inbound: bool): uint8? {
    val replay = BridgeStorage.load().replay.load();
    if (inbound) {
        return replay.inbound.exists(messageId) ? replay.inbound.mustGet(messageId) : null;
    }
    return replay.outbound.exists(messageId) ? replay.outbound.mustGet(messageId) : null;
}
