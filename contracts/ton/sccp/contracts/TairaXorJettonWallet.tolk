import "constants"
import "errors"
import "jetton-utils"
import "messages"
import "replay-forest"
import "sccp-codec"
import "storage"

contract TairaXorJettonWallet {
    author: "Hyperledger Iroha"
    version: "1.0.0"
    description: "Immutable TEP-74 Taira XOR wallet with replay-safe SCCP settlement"
    incomingMessages: AllowedWalletMessage
    storage: WalletStorage
}

type AllowedWalletMessage =
    | AskToTransfer
    | AskToBurn
    | InternalTransferStep
    | SccpWalletMint
    | SccpWalletPrepareBurn
    | SccpWalletValidateBurnRecipient
    | SccpWalletValidateBurnSubgroup
    | SccpWalletBurnHashStep
    | SccpWalletCommitValidatedBurn
    | SccpWalletRetryBurnPrepare
    | SccpWalletRefundBurnLock
    | SccpWalletBurn
    | SccpMasterBurnAck
    | TopUpTons

type WalletBounceMessage =
    | InternalTransferStep
    | BurnNotificationForMinter
    | SccpMasterBurn
    | SccpWalletMintAck

fun requireOperationValue(value: coins) {
    assert (value >= SCCP_OPERATION_VALUE) throw SccpErrors.NotEnoughGas;
}

fun assertWalletConfiguration(storage: WalletStorage) {
    assert (
        getNetworkGlobalId() == SCCP_TON_MAINNET_GLOBAL_ID &&
        storage.tail.load().routeRevision != 0 &&
        storage.tail.load().routeConfigurationHash != 0 &&
        storage.tail.load().maxWrappedSupply > 0 &&
        storage.tail.load().maxWrappedSupply <= SCCP_MAX_JETTON_AMOUNT &&
        storage.ownerAddress.getWorkchain() == SCCP_WORKCHAIN &&
        storage.masterAddress.getWorkchain() == SCCP_WORKCHAIN &&
        storage.tail.load().bridgeAddress.getWorkchain() == SCCP_WORKCHAIN &&
        storage.ownerAddress != storage.masterAddress &&
        storage.masterAddress != storage.tail.load().bridgeAddress
    ) throw SccpErrors.InvalidConfiguration;
    val replay = storage.replay.load();
    sccpReplayValidateForestV1(replay.mintCredits.load());
    sccpReplayValidateForestV1(replay.burnAuthorizations.load());
    sccpReplayValidateForestV1(replay.burnLocks.load());
    sccpReplayValidateForestV1(replay.burnRefunds.load());
    val pending = storage.pending.load();
    val walletState = storage.tail.load();
    assert (
        pending.burnCount <= SCCP_PENDING_OPERATION_CAP &&
        ((pending.burnCount == 0) == pending.burns.isEmpty()) &&
        walletState.lockedBurnCount <= SCCP_PENDING_OPERATION_CAP &&
        ((walletState.lockedBurnCount == 0) == walletState.lockedBurns.isEmpty())
    ) throw SccpErrors.InvalidConfiguration;
}

fun sendInternalTransfer(
    destinationOwner: address,
    storage: WalletStorage,
    queryId: uint64,
    amount: coins,
    excesses: address?,
    forwardTonAmount: coins,
    forwardPayload: ForwardPayloadRemainder,
) {
    val transfer = createMessage({
        bounce: BounceMode.RichBounce,
        dest: calcDeployedJettonWallet(
            destinationOwner,
            storage.masterAddress,
            storage.tail.load().bridgeAddress,
            contract.getCode(),
            storage.tail.load().routeRevision,
            storage.tail.load().routeConfigurationHash,
            storage.tail.load().maxWrappedSupply,
        ),
        value: sccpMasterBurnMinimum(),
        body: InternalTransferStep {
            queryId,
            jettonAmount: amount,
            transferInitiator: storage.ownerAddress,
            sendExcessesTo: excesses,
            forwardTonAmount,
            forwardPayload,
        },
    });
    transfer.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun dispatchMasterBurn(
    storage: WalletStorage,
    messageId: uint256,
    record: WalletBurnRecord,
    replayWitness: Cell<SccpReplayWitnessV1>,
) {
    val tail = record.tail.load();
    val burn = createMessage({
        bounce: BounceMode.RichBounce,
        dest: storage.masterAddress,
        value: sccpMasterBurnMinimum(),
        body: SccpMasterBurn {
            lockSequence: record.lockSequence,
            queryId: record.queryId,
            messageId,
            jettonAmount: record.jettonAmount,
            tail: (SccpMasterBurnTail {
                bridgeAddress: record.bridgeAddress,
                ownerAddress: record.ownerAddress,
                payload: (SccpMasterBurnPayloadTail {
                    payloadHash: tail.payloadHash,
                    payloadSha256: tail.payloadSha256,
                    sourceEventDigest: tail.sourceEventDigest,
                    replayWitness,
                }).toCell(),
            }).toCell(),
        },
    });
    burn.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun sendBridgeBurnAck(
    destination: address,
    lockSequence: uint64,
    queryId: uint64,
    messageId: uint256,
    ownerAddress: address,
    amount: coins,
    payloadHash: uint256,
    sourceEventDigest: uint256,
) {
    val ack = createMessage({
        bounce: BounceMode.NoBounce,
        dest: destination,
        value: sccpBurnBridgeCompletionMinimum(),
        body: SccpMasterBurnAck {
            lockSequence,
            queryId,
            messageId,
            jettonAmount: amount,
            tail: (SccpMasterBurnAckTail { ownerAddress, payloadHash, sourceEventDigest }).toCell(),
        },
    });
    ack.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_IGNORE_ERRORS);
}

fun walletBurnLockReplayRecord(
    storage: WalletStorage,
    operation: uint8,
    lockSequence: uint64,
    semantic: WalletBurnLockSemantic,
): SccpTonReplayRecordV1 {
    return {
        operation,
        replayId: sccpReplayWalletBurnLockIdV1(contract.getAddress(), lockSequence),
        payloadSha256: semantic.payloadSha256,
        amountScale9: semantic.jettonAmount as uint128,
        principal: storage.ownerAddress,
        auxiliaryIdentitySha256: sccpReplayAuxiliaryAddressSha256V1(
            storage.tail.load().bridgeAddress,
        ),
    };
}

fun walletBurnLockMatches(
    semantic: WalletBurnLockSemantic,
    queryId: uint64,
    outboundNonce: uint64,
    amount: coins,
    recipient: cell,
): bool {
    return semantic.queryId == queryId && semantic.outboundNonce == outboundNonce &&
    semantic.jettonAmount == amount &&
    semantic.tairaRecipient.hash() == recipient.hash();
}

fun dispatchBridgeBurnPrepare(
    storage: WalletStorage,
    lockSequence: uint64,
    lock: WalletBurnLock,
    witnesses: SccpBurnPrepareRetryWitnessesV1,
) {
    val semantic = lock.semantic.load();
    val prepare = createMessage({
        // The first 96 post-opcode bits identify a single attempt even when
        // TON returns only the canonical truncated bounce body.
        bounce: BounceMode.Only256BitsOfBody,
        dest: storage.tail.load().bridgeAddress,
        value: sccpBridgeBurnPrepareMinimum(),
        body: SccpBridgeBurnPrepare {
            lockSequence,
            attempt: lock.attempt,
            queryId: semantic.queryId,
            outboundNonce: semantic.outboundNonce,
            ownerAddress: storage.ownerAddress,
            jettonAmount: semantic.jettonAmount,
            tail: (SccpBridgeBurnPrepareTail {
                payloadHash: semantic.payloadHash,
                payloadSha256: semantic.payloadSha256,
                tairaRecipient: semantic.tairaRecipient,
                replayWitnesses: witnesses.toCell(),
            }).toCell(),
        },
    });
    prepare.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun dispatchBurnSubgroupValidation(
    stepsRemaining: uint8,
    remainingScalar: uint256,
    result: EdwardsPoint,
    multiple: EdwardsPoint,
    material: Cell<SccpWalletBurnValidationMaterialV1>,
) {
    val continuation = createMessage({
        bounce: BounceMode.Only256BitsOfBody,
        dest: contract.getAddress(),
        value: sccpWalletBurnSubgroupMinimum(stepsRemaining),
        body: SccpWalletValidateBurnSubgroup {
            stepsRemaining,
            remainingScalar,
            result: ed25519PointCell(result),
            multiple: ed25519PointCell(multiple),
            material,
        },
    });
    continuation.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun dispatchBurnPayloadHash(
    nextBlock: uint8,
    chainingState: Cell<SccpBlake2bStateV1>,
    material: Cell<SccpWalletBurnValidationMaterialV1>,
    remainingBlocks: int,
) {
    val continuation = createMessage({
        bounce: BounceMode.Only256BitsOfBody,
        dest: contract.getAddress(),
        value: sccpWalletBurnHashMinimum(remainingBlocks),
        body: SccpWalletBurnHashStep { nextBlock, chainingState, material },
    });
    continuation.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun dispatchValidatedBurnCommit(
    payloadHash: uint256,
    material: Cell<SccpWalletBurnValidationMaterialV1>,
) {
    val continuation = createMessage({
        bounce: BounceMode.Only256BitsOfBody,
        dest: contract.getAddress(),
        value: sccpWalletBurnCommitMinimum(),
        body: SccpWalletCommitValidatedBurn { payloadHash, material },
    });
    continuation.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

/// Occupy 0x36 and debit only after every canonical I105 and Ed25519 check has
/// completed. State is re-read here so two in-flight requests for the same
/// wallet nonce cannot both cross the irreversible boundary.
fun commitValidatedWalletBurn(
    material: SccpWalletBurnValidationMaterialV1,
    payloadHash: uint256,
) {
    assert (payloadHash != 0) throw SccpErrors.InvalidPayload;
    var storage = WalletStorage.load();
    assertWalletConfiguration(storage);
    var walletState = storage.tail.load();
    assert (
        sccpOutboundNonceCanAdvance(material.expectedNonce, walletState.outboundNonce)
    ) throw SccpErrors.InvalidNonce;
    assert (walletState.nextLockSequence != 0xffffffffffffffff) throw SccpErrors.NonceExhausted;
    assert (
        sccpPendingCapacityAvailable(walletState.lockedBurnCount)
    ) throw SccpErrors.PendingCapacityExceeded;
    assert (
        sccpAmountWithinRouteCap(material.jettonAmount, walletState.maxWrappedSupply) &&
        storage.jettonBalance >= material.jettonAmount
    ) throw SccpErrors.InvalidMessage;
    val (recipientShapeValid, recipientBytes) = readTairaRecipientBytes(material.tairaRecipient);
    assert (recipientShapeValid) throw SccpErrors.InvalidPayload;
    val canonical = buildTonToTairaPayloadFromValidatedRecipientBytes(
        storage.ownerAddress,
        recipientBytes,
        walletState.outboundNonce,
        walletState.routeRevision,
        material.jettonAmount as uint128,
    );
    val semantic: WalletBurnLockSemantic = {
        queryId: material.queryId,
        outboundNonce: walletState.outboundNonce,
        jettonAmount: material.jettonAmount,
        tairaRecipient: material.tairaRecipient,
        canonicalPayload: canonical.toCell(),
        payloadHash,
        payloadSha256: sccpReplayPayloadSha256V1(canonical),
    };
    val lockSequence = walletState.nextLockSequence;
    val supplied = material.replayWitnesses.load();
    var replay = storage.replay.load();
    var lockForest = replay.burnLocks.load();
    val admission = sccpReplayAdmitV1(
        mutate lockForest,
        walletReplayDomain(storage, SCCP_REPLAY_TON_WALLET_BURN_LOCK),
        walletBurnLockReplayRecord(
            storage,
            SCCP_REPLAY_TON_WALLET_BURN_LOCK,
            lockSequence,
            semantic,
        ),
        supplied.lockDebit,
    );
    assert (admission.occupiedNow) throw SccpErrors.Replay;
    val lock: WalletBurnLock = {
        status: SCCP_WALLET_LOCK_PREPARING,
        attempt: 1,
        semantic: semantic.toCell(),
    };
    storage.jettonBalance -= material.jettonAmount;
    walletState.outboundNonce += 1;
    walletState.nextLockSequence += 1;
    walletState.lockedBurns.set(lockSequence, lock.toCell());
    walletState.lockedBurnCount += 1;
    replay.burnLocks = lockForest.toCell();
    storage.replay = replay.toCell();
    storage.tail = walletState.toCell();
    storage.save();
    emitSccpReplayDeltaV1(SCCP_REPLAY_TON_WALLET_BURN_LOCK, admission);
    val witnessTail = supplied.tail.load();
    dispatchBridgeBurnPrepare(
        storage,
        lockSequence,
        lock,
        {
            bridge: supplied.bridge,
            walletAuthorization: supplied.walletAuthorization,
            master: witnessTail.master,
        },
    );
}

fun onBouncedMessage(in: InMessageBounced) {
    reserveSccpPreMessageBalance(in.valueCoins, sccpWalletStorageReserve());
    var truncated = in.bouncedBody;
    truncated.skipBouncedPrefix();
    if (truncated.remainingBitsCount() >= 32 + 64 + 32 && truncated.preloadUint(32) == 0x53434250) {
        truncated.skipBits(32);
        val lockSequence = truncated.loadUint(64) as uint64;
        val attempt = truncated.loadUint(32) as uint32;
        var storage = WalletStorage.load();
        assertWalletConfiguration(storage);
        assert (in.senderAddress == storage.tail.load().bridgeAddress) throw SccpErrors.NotBridge;
        var walletState = storage.tail.load();
        if (walletState.lockedBurns.exists(lockSequence)) {
            var lock = walletState.lockedBurns.mustGet(lockSequence).load();
            if (lock.status == SCCP_WALLET_LOCK_PREPARING && lock.attempt == attempt) {
                // A matching bounce proves this exact bridge attempt did not
                // occupy 0x31. Refund still requires a fresh 0x37 witness.
                lock.status = SCCP_WALLET_LOCK_REFUNDABLE;
                walletState.lockedBurns.set(lockSequence, lock.toCell());
                storage.tail = walletState.toCell();
                storage.save();
            }
        }
        return;
    }
    val rich = lazy RichBounceBody.fromSlice(in.bouncedBody);
    val msg = lazy WalletBounceMessage.fromSlice(rich.originalBody.beginParse());
    var storage = WalletStorage.load();
    assertWalletConfiguration(storage);
    val pending = storage.pending.load();
    match (msg) {
        InternalTransferStep => {
            assert (
                msg.transferInitiator != null && msg.transferInitiator! == storage.ownerAddress
            ) throw SccpErrors.InvalidCallback;
            storage.jettonBalance += msg.jettonAmount;
        }
        BurnNotificationForMinter => {
            assert (in.senderAddress == storage.masterAddress) throw SccpErrors.InvalidCallback;
            storage.jettonBalance += msg.jettonAmount;
        }
        SccpMasterBurn => {
            assert (in.senderAddress == storage.masterAddress) throw SccpErrors.InvalidCallback;
            assert (pending.burns.exists(msg.messageId)) throw SccpErrors.UnknownPendingOperation;
            val record = pending.burns.mustGet(msg.messageId).load();
            val tail = msg.tail.load();
            val payload = tail.payload.load();
            assert (
                sccpWalletBurnRecordMatches(
                    record,
                    msg.lockSequence,
                    msg.queryId,
                    record.outboundNonce,
                    tail.bridgeAddress,
                    tail.ownerAddress,
                    msg.jettonAmount,
                    payload.payloadHash,
                    payload.sourceEventDigest,
                )
            ) throw SccpErrors.InvalidCallback;
            // One failed attempt cannot release a debit that another exact
            // in-flight attempt may already have settled at the master.
        }
        SccpWalletMintAck => {
            assert (in.senderAddress == storage.masterAddress) throw SccpErrors.InvalidCallback;
            // Minted Jettons remain reserved because a sibling acknowledgement
            // may already have completed the master-side operation.
        }
        else => throw 0xFFFF,
    }
    storage.save();
}

fun onInternalMessage(in: InMessage) {
    reserveSccpPreMessageBalance(in.valueCoins, sccpWalletStorageReserve());
    val msg = lazy AllowedWalletMessage.fromSlice(in.body);
    match (msg) {
        InternalTransferStep => {
            assert (
                sccpInternalTransferValueIsSufficient(in.valueCoins, msg.forwardTonAmount)
            ) throw SccpErrors.NotEnoughGas;
            var storage = WalletStorage.load();
            assertWalletConfiguration(storage);
            if (in.senderAddress != storage.masterAddress) {
                assert (msg.transferInitiator != null) throw SccpErrors.NotValidWallet;
                assert (
                    in.senderAddress ==
                    calcAddressOfJettonWallet(
                        msg.transferInitiator!,
                        storage.masterAddress,
                        storage.tail.load().bridgeAddress,
                        contract.getCode(),
                        storage.tail.load().routeRevision,
                        storage.tail.load().routeConfigurationHash,
                        storage.tail.load().maxWrappedSupply,
                    )
                ) throw SccpErrors.NotValidWallet;
            }
            storage.jettonBalance += msg.jettonAmount;
            storage.save();

            if (msg.forwardTonAmount != 0) {
                val notification = createMessage({
                    bounce: BounceMode.NoBounce,
                    dest: storage.ownerAddress,
                    value: msg.forwardTonAmount,
                    body: TransferNotificationForRecipient {
                        queryId: msg.queryId,
                        jettonAmount: msg.jettonAmount,
                        transferInitiator: msg.transferInitiator,
                        forwardPayload: msg.forwardPayload,
                    },
                });
                notification.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
            }
            if (msg.sendExcessesTo != null) {
                val boundedExcess = sccpBoundedInboundExcess(
                    in.valueCoins,
                    msg.forwardTonAmount,
                );
                val excess = createMessage({
                    bounce: BounceMode.NoBounce,
                    dest: msg.sendExcessesTo!,
                    value: boundedExcess,
                    body: ReturnExcessesBack { queryId: msg.queryId },
                });
                // This is intentionally an ordinary, explicitly bounded send. The exact reserve
                // protects the pre-message balance and the value cannot exceed this inbound hop.
                excess.send(
                    SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_IGNORE_ERRORS,
                );
            }
        }

        AskToTransfer => {
            msg.forwardPayload.checkIsCorrectTlbEither();
            requireOperationValue(in.valueCoins);
            assert (
                sccpTransferRequestValueIsSufficient(in.valueCoins, msg.forwardTonAmount)
            ) throw SccpErrors.NotEnoughGas;
            assert (
                msg.transferRecipient.getWorkchain() == SCCP_WORKCHAIN
            ) throw SccpErrors.WrongWorkchain;
            var storage = WalletStorage.load();
            assertWalletConfiguration(storage);
            assert (in.senderAddress == storage.ownerAddress) throw SccpErrors.NotOwner;
            assert (
                msg.jettonAmount != 0 && storage.jettonBalance >= msg.jettonAmount
            ) throw SccpErrors.BalanceError;
            storage.jettonBalance -= msg.jettonAmount;
            storage.save();
            sendInternalTransfer(
                msg.transferRecipient,
                storage,
                msg.queryId,
                msg.jettonAmount,
                msg.sendExcessesTo,
                msg.forwardTonAmount,
                msg.forwardPayload,
            );
        }

        AskToBurn => {
            requireOperationValue(in.valueCoins);
            var storage = WalletStorage.load();
            assertWalletConfiguration(storage);
            assert (in.senderAddress == storage.ownerAddress) throw SccpErrors.NotOwner;
            assert (
                msg.jettonAmount != 0 && storage.jettonBalance >= msg.jettonAmount
            ) throw SccpErrors.BalanceError;
            storage.jettonBalance -= msg.jettonAmount;
            storage.save();
            val burn = createMessage({
                bounce: BounceMode.RichBounce,
                dest: storage.masterAddress,
                value: 0,
                body: BurnNotificationForMinter {
                    queryId: msg.queryId,
                    jettonAmount: msg.jettonAmount,
                    burnInitiator: storage.ownerAddress,
                    sendExcessesTo: msg.sendExcessesTo,
                },
            });
            burn.send(
                SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_BOUNCE_ON_ACTION_FAIL,
            );
        }

        SccpWalletMint => {
            requireSccpInboundBudget(in.valueCoins, sccpWalletMintMinimum());
            var storage = WalletStorage.load();
            assertWalletConfiguration(storage);
            assert (in.senderAddress == storage.masterAddress) throw SccpErrors.NotMaster;
            assert (
                msg.bridgeAddress == storage.tail.load().bridgeAddress
            ) throw SccpErrors.NotBridge;
            val tail = msg.tail.load();
            assert (
                msg.messageId != 0 && tail.payloadHash != 0 && tail.payloadSha256 != 0 &&
                sccpAmountWithinRouteCap(msg.jettonAmount, storage.tail.load().maxWrappedSupply) &&
                msg.messageId != tail.payloadHash
            ) throw SccpErrors.InvalidMessage;
            var replay = storage.replay.load();
            var mintForest = replay.mintCredits.load();
            val replayRecord: SccpTonReplayRecordV1 = {
                operation: SCCP_REPLAY_TON_WALLET_MINT_CREDIT,
                replayId: msg.messageId,
                payloadSha256: tail.payloadSha256,
                amountScale9: msg.jettonAmount as uint128,
                principal: storage.ownerAddress,
                auxiliaryIdentitySha256: sccpReplayAuxiliaryAddressSha256V1(
                    msg.bridgeAddress,
                ),
            };
            val replayDomain = sccpTonReplayDomainHashV1(
                SCCP_REPLAY_TON_WALLET_MINT_CREDIT,
                storage.tail.load().routeRevision,
                storage.tail.load().routeConfigurationHash,
                contract.getAddress(),
            );
            val admission = sccpReplayAdmitV1(
                mutate mintForest,
                replayDomain,
                replayRecord,
                tail.replayWitness,
            );
            if (admission.occupiedNow) {
                storage.jettonBalance += msg.jettonAmount;
                replay.mintCredits = mintForest.toCell();
                storage.replay = replay.toCell();
                storage.save();
                emitSccpReplayDeltaV1(SCCP_REPLAY_TON_WALLET_MINT_CREDIT, admission);
            }
            val ack = createMessage({
                bounce: BounceMode.RichBounce,
                dest: storage.masterAddress,
                value: sccpMintMasterAcknowledgementMinimum(),
                body: SccpWalletMintAck {
                    queryId: msg.queryId,
                    messageId: msg.messageId,
                    bridgeAddress: msg.bridgeAddress,
                    jettonAmount: msg.jettonAmount,
                    payloadHash: tail.payloadHash,
                },
            });
            ack.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
        }

        SccpWalletPrepareBurn => {
            requireSccpInboundBudget(in.valueCoins, sccpWalletBurnPrepareMinimum());
            val storage = WalletStorage.load();
            assertWalletConfiguration(storage);
            assert (in.senderAddress == storage.ownerAddress) throw SccpErrors.NotOwner;
            val walletState = storage.tail.load();
            assert (
                sccpOutboundNonceCanAdvance(msg.expectedNonce, walletState.outboundNonce)
            ) throw SccpErrors.InvalidNonce;
            assert (
                walletState.nextLockSequence != 0xffffffffffffffff
            ) throw SccpErrors.NonceExhausted;
            assert (
                sccpPendingCapacityAvailable(walletState.lockedBurnCount)
            ) throw SccpErrors.PendingCapacityExceeded;
            assert (
                sccpAmountWithinRouteCap(msg.jettonAmount, walletState.maxWrappedSupply) &&
                storage.jettonBalance >= msg.jettonAmount
            ) throw SccpErrors.InvalidMessage;
            val (recipientValid, encodedKey) = decodeCanonicalTairaRecipientKey(
                msg.tairaRecipient,
            );
            assert (recipientValid) throw SccpErrors.InvalidPayload;
            val continuation = createMessage({
                bounce: BounceMode.Only256BitsOfBody,
                dest: contract.getAddress(),
                value: sccpWalletBurnRecoveryMinimum(),
                body: SccpWalletValidateBurnRecipient {
                    encodedEd25519Key: encodedKey as uint256,
                    material: (SccpWalletBurnValidationMaterialV1 {
                        queryId: msg.queryId,
                        expectedNonce: msg.expectedNonce,
                        jettonAmount: msg.jettonAmount,
                        tairaRecipient: msg.tairaRecipient,
                        replayWitnesses: msg.replayWitnesses,
                    }).toCell(),
                },
            });
            continuation.send(
                SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL,
            );
        }

        SccpWalletValidateBurnRecipient => {
            assert (in.senderAddress == contract.getAddress()) throw SccpErrors.NotValidWallet;
            requireSccpInboundBudget(in.valueCoins, sccpWalletBurnRecoveryMinimum());
            val material = msg.material.load();
            val storage = WalletStorage.load();
            assertWalletConfiguration(storage);
            val walletState = storage.tail.load();
            assert (
                sccpOutboundNonceCanAdvance(material.expectedNonce, walletState.outboundNonce)
            ) throw SccpErrors.InvalidNonce;
            assert (
                walletState.nextLockSequence != 0xffffffffffffffff
            ) throw SccpErrors.NonceExhausted;
            assert (
                sccpPendingCapacityAvailable(walletState.lockedBurnCount)
            ) throw SccpErrors.PendingCapacityExceeded;
            assert (
                sccpAmountWithinRouteCap(
                    material.jettonAmount,
                    storage.tail.load().maxWrappedSupply,
                ) &&
                storage.jettonBalance >= material.jettonAmount
            ) throw SccpErrors.InvalidMessage;
            val (recovered, x, y) = recoverAdmittedEd25519Point(msg.encodedEd25519Key);
            assert (recovered) throw SccpErrors.InvalidPayload;
            dispatchBurnSubgroupValidation(
                SCCP_ED25519_SUBGROUP_MAX_STEPS as uint8,
                SCCP_ED25519_SUBGROUP_ORDER as uint256,
                ed25519IdentityPoint(),
                ed25519MultiplePoint(x, y),
                msg.material,
            );
        }

        SccpWalletValidateBurnSubgroup => {
            assert (in.senderAddress == contract.getAddress()) throw SccpErrors.NotValidWallet;
            assert (
                msg.stepsRemaining > 0 && msg.stepsRemaining <= SCCP_ED25519_SUBGROUP_MAX_STEPS &&
                msg.remainingScalar > 0 &&
                msg.remainingScalar <= SCCP_ED25519_SUBGROUP_ORDER
            ) throw SccpErrors.InvalidPayload;
            requireSccpInboundBudget(
                in.valueCoins,
                sccpWalletBurnSubgroupMinimum(msg.stepsRemaining),
            );
            val (result, multiple, remainingScalar) = ed25519PrimeSubgroupStep(
                ed25519PointFromCell(msg.result),
                ed25519PointFromCell(msg.multiple),
                msg.remainingScalar,
                SCCP_ED25519_SUBGROUP_BITS_PER_STEP,
            );
            if (msg.stepsRemaining == 1) {
                assert (
                    remainingScalar == 0 && ed25519SubgroupResultIsIdentity(result)
                ) throw SccpErrors.InvalidPayload;
                dispatchBurnPayloadHash(
                    0,
                    sccpBlake2bInitialStateV1(),
                    msg.material,
                    SCCP_MAX_TON_TO_TAIRA_HASH_BLOCKS,
                );
            } else {
                assert (remainingScalar != 0) throw SccpErrors.InvalidPayload;
                dispatchBurnSubgroupValidation(
                    (msg.stepsRemaining - 1) as uint8,
                    remainingScalar as uint256,
                    result,
                    multiple,
                    msg.material,
                );
            }
        }

        SccpWalletBurnHashStep => {
            assert (in.senderAddress == contract.getAddress()) throw SccpErrors.NotValidWallet;
            requireSccpInboundBudget(in.valueCoins, sccpWalletBurnCommitMinimum());
            val material = msg.material.load();
            val storage = WalletStorage.load();
            assertWalletConfiguration(storage);
            val (recipientShapeValid, recipientBytes) = readTairaRecipientBytes(
                material.tairaRecipient,
            );
            assert (recipientShapeValid) throw SccpErrors.InvalidPayload;
            val canonical = buildTonToTairaPayloadFromValidatedRecipientBytes(
                storage.ownerAddress,
                recipientBytes,
                material.expectedNonce,
                storage.tail.load().routeRevision,
                material.jettonAmount as uint128,
            );
            val blockCount = sccpPayloadHashBlockCountV1(canonical);
            assert (
                blockCount > 0 && blockCount <= SCCP_MAX_TON_TO_TAIRA_HASH_BLOCKS && msg.nextBlock <
                blockCount
            ) throw SccpErrors.InvalidPayload;
            requireSccpInboundBudget(
                in.valueCoins,
                sccpWalletBurnHashMinimum(blockCount - msg.nextBlock),
            );
            val state = sccpPayloadHashBlockV1(
                canonical,
                msg.chainingState,
                msg.nextBlock,
            );
            val following = (msg.nextBlock + 1) as int;
            if (following == blockCount) {
                dispatchValidatedBurnCommit(
                    sccpPayloadHashFromStateV1(state),
                    msg.material,
                );
            } else {
                dispatchBurnPayloadHash(
                    following as uint8,
                    state,
                    msg.material,
                    blockCount - following,
                );
            }
        }

        SccpWalletCommitValidatedBurn => {
            assert (in.senderAddress == contract.getAddress()) throw SccpErrors.NotValidWallet;
            requireSccpInboundBudget(in.valueCoins, sccpWalletBurnCommitMinimum());
            commitValidatedWalletBurn(msg.material.load(), msg.payloadHash);
        }

        SccpWalletRetryBurnPrepare => {
            requireSccpInboundBudget(in.valueCoins, sccpWalletBurnPrepareMinimum());
            var storage = WalletStorage.load();
            assertWalletConfiguration(storage);
            var walletState = storage.tail.load();
            assert (
                walletState.lockedBurns.exists(msg.lockSequence)
            ) throw SccpErrors.UnknownPendingOperation;
            var lock = walletState.lockedBurns.mustGet(msg.lockSequence).load();
            assert (
                lock.status == SCCP_WALLET_LOCK_REFUNDABLE && lock.attempt != 0xffffffff
            ) throw SccpErrors.WrongPendingOperation;
            lock.status = SCCP_WALLET_LOCK_PREPARING;
            lock.attempt += 1;
            walletState.lockedBurns.set(msg.lockSequence, lock.toCell());
            storage.tail = walletState.toCell();
            storage.save();
            dispatchBridgeBurnPrepare(
                storage,
                msg.lockSequence,
                lock,
                msg.replayWitnesses.load(),
            );
        }

        SccpWalletRefundBurnLock => {
            requireSccpInboundBudget(in.valueCoins, sccpMaximumComputeFee());
            var storage = WalletStorage.load();
            assertWalletConfiguration(storage);
            var walletState = storage.tail.load();
            if (!walletState.lockedBurns.exists(msg.lockSequence)) {
                return;
            }
            val lock = walletState.lockedBurns.mustGet(msg.lockSequence).load();
            assert (
                lock.status == SCCP_WALLET_LOCK_REFUNDABLE
            ) throw SccpErrors.WrongPendingOperation;
            val semantic = lock.semantic.load();
            var replay = storage.replay.load();
            var refundForest = replay.burnRefunds.load();
            val admission = sccpReplayAdmitV1(
                mutate refundForest,
                walletReplayDomain(storage, SCCP_REPLAY_TON_WALLET_BURN_REFUND),
                walletBurnLockReplayRecord(
                    storage,
                    SCCP_REPLAY_TON_WALLET_BURN_REFUND,
                    msg.lockSequence,
                    semantic,
                ),
                msg.replayWitness,
            );
            assert (admission.occupiedNow) throw SccpErrors.InvalidReplayForest;
            storage.jettonBalance += semantic.jettonAmount;
            walletState.lockedBurns.delete(msg.lockSequence);
            assert (walletState.lockedBurnCount != 0) throw SccpErrors.InvalidConfiguration;
            walletState.lockedBurnCount -= 1;
            replay.burnRefunds = refundForest.toCell();
            storage.replay = replay.toCell();
            storage.tail = walletState.toCell();
            storage.save();
            emitSccpReplayDeltaV1(SCCP_REPLAY_TON_WALLET_BURN_REFUND, admission);
        }

        SccpWalletBurn => {
            requireSccpInboundBudget(
                in.valueCoins,
                msg.completed
                    ? sccpBurnWalletAcknowledgementMinimum()
                    : sccpWalletBurnAuthorizationMinimum(),
            );
            var storage = WalletStorage.load();
            assertWalletConfiguration(storage);
            assert (
                in.senderAddress == storage.tail.load().bridgeAddress
            ) throw SccpErrors.NotBridge;
            val tail = msg.tail.load();
            assert (
                msg.ownerAddress == storage.ownerAddress && msg.messageId != 0 &&
                sccpAmountWithinRouteCap(msg.jettonAmount, storage.tail.load().maxWrappedSupply) &&
                tail.payloadHash != 0 &&
                tail.payloadSha256 != 0 &&
                tail.sourceEventDigest != 0
            ) throw SccpErrors.InvalidMessage;
            val replayWitnesses = tail.replayWitnesses.load();
            var replay = storage.replay.load();
            var authorizationForest = replay.burnAuthorizations.load();
            val replayRecord: SccpTonReplayRecordV1 = {
                operation: SCCP_REPLAY_TON_WALLET_BURN_AUTHORIZATION,
                replayId: msg.messageId,
                payloadSha256: tail.payloadSha256,
                amountScale9: msg.jettonAmount as uint128,
                principal: storage.ownerAddress,
                auxiliaryIdentitySha256: sccpReplayAuxiliaryWordSha256V1(
                    tail.sourceEventDigest,
                ),
            };
            val admission = sccpReplayAdmitV1(
                mutate authorizationForest,
                walletReplayDomain(storage, SCCP_REPLAY_TON_WALLET_BURN_AUTHORIZATION),
                replayRecord,
                replayWitnesses.first,
            );
            val record: WalletBurnRecord = {
                lockSequence: msg.lockSequence,
                queryId: msg.queryId,
                outboundNonce: msg.outboundNonce,
                bridgeAddress: storage.tail.load().bridgeAddress,
                ownerAddress: storage.ownerAddress,
                jettonAmount: msg.jettonAmount,
                tail: (WalletBurnRecordTail {
                    payloadHash: tail.payloadHash,
                    payloadSha256: tail.payloadSha256,
                    sourceEventDigest: tail.sourceEventDigest,
                }).toCell(),
            };
            var pending = storage.pending.load();
            var walletState = storage.tail.load();
            if (admission.occupiedNow) {
                assert (!msg.completed) throw SccpErrors.InvalidCallback;
                assert (
                    walletState.lockedBurns.exists(msg.lockSequence) &&
                    sccpPendingCapacityAvailable(pending.burnCount)
                ) throw SccpErrors.UnknownPendingOperation;
                val lock = walletState.lockedBurns.mustGet(msg.lockSequence).load();
                val semantic = lock.semantic.load();
                assert (
                    lock.status == SCCP_WALLET_LOCK_PREPARING &&
                    walletBurnLockMatches(
                        semantic,
                        msg.queryId,
                        msg.outboundNonce,
                        msg.jettonAmount,
                        semantic.tairaRecipient,
                    ) &&
                    semantic.payloadSha256 == tail.payloadSha256 &&
                    semantic.payloadHash == tail.payloadHash
                ) throw SccpErrors.Replay;
                pending.burns.set(msg.messageId, record.toCell());
                pending.burnCount += 1;
                walletState.lockedBurns.delete(msg.lockSequence);
                assert (walletState.lockedBurnCount != 0) throw SccpErrors.InvalidConfiguration;
                walletState.lockedBurnCount -= 1;
                replay.burnAuthorizations = authorizationForest.toCell();
                storage.replay = replay.toCell();
                storage.pending = pending.toCell();
                storage.tail = walletState.toCell();
                storage.save();
                emitSccpReplayDeltaV1(SCCP_REPLAY_TON_WALLET_BURN_AUTHORIZATION, admission);
                dispatchMasterBurn(storage, msg.messageId, record, replayWitnesses.second);
            } else if (pending.burns.exists(msg.messageId)) {
                val existing = pending.burns.mustGet(msg.messageId).load();
                val existingTail = existing.tail.load();
                assert (
                    sccpWalletBurnRecordMatches(
                        existing,
                        msg.lockSequence,
                        msg.queryId,
                        msg.outboundNonce,
                        storage.tail.load().bridgeAddress,
                        storage.ownerAddress,
                        msg.jettonAmount,
                        tail.payloadHash,
                        tail.sourceEventDigest,
                    ) &&
                    existingTail.payloadSha256 == tail.payloadSha256
                ) throw SccpErrors.Replay;
                dispatchMasterBurn(storage, msg.messageId, existing, replayWitnesses.second);
            } else {
                sendBridgeBurnAck(
                    storage.tail.load().bridgeAddress,
                    msg.lockSequence,
                    msg.queryId,
                    msg.messageId,
                    storage.ownerAddress,
                    msg.jettonAmount,
                    tail.payloadHash,
                    tail.sourceEventDigest,
                );
            }
        }

        SccpMasterBurnAck => {
            requireSccpInboundBudget(in.valueCoins, sccpBurnWalletAcknowledgementMinimum());
            var storage = WalletStorage.load();
            assertWalletConfiguration(storage);
            assert (in.senderAddress == storage.masterAddress) throw SccpErrors.NotMaster;
            var pending = storage.pending.load();
            assert (pending.burns.exists(msg.messageId)) throw SccpErrors.InvalidCallback;
            val record = pending.burns.mustGet(msg.messageId).load();
            val tail = record.tail.load();
            val ackTail = msg.tail.load();
            assert (
                record.lockSequence == msg.lockSequence && record.queryId == msg.queryId &&
                record.ownerAddress == ackTail.ownerAddress &&
                record.jettonAmount == msg.jettonAmount &&
                tail.payloadHash == ackTail.payloadHash &&
                tail.sourceEventDigest == ackTail.sourceEventDigest
            ) throw SccpErrors.InvalidCallback;
            pending.burns.delete(msg.messageId);
            assert (pending.burnCount != 0) throw SccpErrors.InvalidConfiguration;
            pending.burnCount -= 1;
            storage.pending = pending.toCell();
            storage.save();
            sendBridgeBurnAck(
                record.bridgeAddress,
                record.lockSequence,
                record.queryId,
                msg.messageId,
                record.ownerAddress,
                record.jettonAmount,
                tail.payloadHash,
                tail.sourceEventDigest,
            );
        }

        TopUpTons => {}
        else => throw 0xFFFF,
    }
}

get fun get_wallet_data(): JettonWalletDataReply {
    val storage = WalletStorage.load();
    return {
        jettonBalance: storage.jettonBalance,
        ownerAddress: storage.ownerAddress,
        masterAddress: storage.masterAddress,
        jettonWalletCode: contract.getCode(),
    };
}

get fun get_sccp_pending_counts(): (uint16, uint16) {
    val storage = WalletStorage.load();
    return (storage.pending.load().burnCount, storage.tail.load().lockedBurnCount);
}

get fun get_sccp_outbound_state(): (uint64, uint64, uint16) {
    val storage = WalletStorage.load();
    val walletState = storage.tail.load();
    return (walletState.outboundNonce, walletState.nextLockSequence, walletState.lockedBurnCount);
}

get fun get_sccp_route_binding(): (address, address, uint256, coins) {
    val storage = WalletStorage.load();
    return (
        storage.masterAddress,
        storage.tail.load().bridgeAddress,
        storage.tail.load().routeConfigurationHash,
        storage.tail.load().maxWrappedSupply,
    );
}

get fun get_sccp_funding_parameters(): (coins, coins, coins, coins) {
    return (
        sccpWalletBurnPrepareMinimum(),
        sccpWalletBurnAuthorizationMinimum(),
        sccpMaximumComputeFee(),
        sccpWalletStorageReserve(),
    );
}

fun walletReplayForest(storage: WalletStorage, operation: uint8): SccpReplayForestV1 {
    val replay = storage.replay.load();
    if (operation == SCCP_REPLAY_TON_WALLET_MINT_CREDIT) {
        return replay.mintCredits.load();
    }
    if (operation == SCCP_REPLAY_TON_WALLET_BURN_AUTHORIZATION) {
        return replay.burnAuthorizations.load();
    }
    if (operation == SCCP_REPLAY_TON_WALLET_BURN_LOCK) {
        return replay.burnLocks.load();
    }
    assert (operation == SCCP_REPLAY_TON_WALLET_BURN_REFUND) throw SccpErrors.InvalidReplayDomain;
    return replay.burnRefunds.load();
}

fun walletReplayDomain(storage: WalletStorage, operation: uint8): uint256 {
    return sccpTonReplayDomainHashV1(
        operation,
        storage.tail.load().routeRevision,
        storage.tail.load().routeConfigurationHash,
        contract.getAddress(),
    );
}

get fun get_sccp_replay_state(
    operation: uint8,
    shard: uint8,
): (uint256, uint256, uint64, uint64) {
    val storage = WalletStorage.load();
    val forest = walletReplayForest(storage, operation);
    return (
        walletReplayDomain(storage, operation),
        sccpReplayShardRootV1(forest, shard),
        forest.leafCount,
        forest.updateSequence,
    );
}

get fun get_sccp_verify_replay_membership(
    operation: uint8,
    replayId: uint256,
    payloadSha256: uint256,
    amountScale9: uint128,
    principal: address,
    auxiliaryIdentitySha256: uint256,
    witness: Cell<SccpReplayWitnessV1>,
): bool {
    val storage = WalletStorage.load();
    return sccpReplayVerifyMembershipV1(
        walletReplayForest(storage, operation),
        walletReplayDomain(storage, operation),
        {
            operation,
            replayId,
            payloadSha256,
            amountScale9,
            principal,
            auxiliaryIdentitySha256,
        },
        witness,
    );
}

get fun get_sccp_verify_replay_non_membership(
    operation: uint8,
    replayId: uint256,
    witness: Cell<SccpReplayWitnessV1>,
): bool {
    val storage = WalletStorage.load();
    return sccpReplayVerifyNonMembershipV1(
        walletReplayForest(storage, operation),
        walletReplayDomain(storage, operation),
        replayId,
        witness,
    );
}
