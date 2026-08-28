import "constants"
import "errors"
import "jetton-utils"
import "messages"
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
    | SccpWalletBurn
    | SccpRefund
    | SccpWalletRefundTransfer
    | TopUpTons

type WalletBounceMessage =
    | InternalTransferStep
    | BurnNotificationForMinter
    | SccpMasterBurn
    | SccpWalletMintAck
    | SccpWalletRefundTransfer
    | SccpWalletRefundAck

fun requireOperationValue(value: coins) {
    assert (value >= SCCP_OPERATION_VALUE) throw SccpErrors.NotEnoughGas;
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
        dest: calcDeployedJettonWallet(destinationOwner, storage.masterAddress, contract.getCode()),
        value: 0,
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

fun onBouncedMessage(in: InMessageBounced) {
    val rich = lazy RichBounceBody.fromSlice(in.bouncedBody);
    val msg = lazy WalletBounceMessage.fromSlice(rich.originalBody.beginParse());
    var storage = WalletStorage.load();
    val replay = storage.replay.load();
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
            assert (replay.burns.exists(msg.messageId)) throw SccpErrors.UnknownPendingOperation;
            val record = replay.burns.mustGet(msg.messageId).load();
            val tail = msg.tail.load();
            assert (
                sccpWalletBurnRecordMatches(
                    record,
                    SCCP_PENDING_BURN,
                    msg.queryId,
                    msg.bridgeAddress,
                    msg.jettonAmount,
                    tail.payloadHash,
                    tail.sourceEventDigest,
                )
            ) throw SccpErrors.InvalidCallback;
            // One failed attempt cannot release a debit that another exact
            // in-flight attempt may already have settled at the master.
        }
        SccpWalletMintAck => {
            assert (in.senderAddress == storage.masterAddress) throw SccpErrors.InvalidCallback;
            assert (
                replay.mints.exists(msg.messageId) &&
                (replay.mints.mustGet(msg.messageId) == SCCP_PENDING_MINT ||
                replay.mints.mustGet(msg.messageId) == SCCP_COMPLETED)
            ) throw SccpErrors.InvalidCallback;
            // Minted Jettons remain reserved because a sibling acknowledgement
            // may already have completed the master-side operation.
        }
        SccpWalletRefundTransfer => {
            assert (replay.refunds.exists(msg.refundId)) throw SccpErrors.UnknownPendingOperation;
            val record = replay.refunds.mustGet(msg.refundId).load();
            assert (
                sccpWalletRefundRecordMatches(
                    record,
                    SCCP_PENDING_BURN,
                    msg.queryId,
                    msg.bridgeAddress,
                    msg.recipientOwner,
                    msg.jettonAmount,
                )
            ) throw SccpErrors.InvalidCallback;
            assert (
                in.senderAddress ==
                calcAddressOfJettonWallet(
                    record.recipientOwner,
                    storage.masterAddress,
                    contract.getCode(),
                )
            ) throw SccpErrors.InvalidCallback;
            // Keep the refund debit reserved; retry is driven by bridge state.
        }
        SccpWalletRefundAck => {
            // A recipient keeps the already credited refund. A replay from the custody wallet
            // causes the acknowledgement to be sent again without crediting twice.
        }
        else => throw 0xFFFF,
    }
    storage.save();
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedWalletMessage.fromSlice(in.body);
    match (msg) {
        InternalTransferStep => {
            assert (
                sccpInternalTransferValueIsSufficient(in.valueCoins, msg.forwardTonAmount)
            ) throw SccpErrors.NotEnoughGas;
            var storage = WalletStorage.load();
            if (in.senderAddress != storage.masterAddress) {
                assert (msg.transferInitiator != null) throw SccpErrors.NotValidWallet;
                assert (
                    in.senderAddress ==
                    calcAddressOfJettonWallet(
                        msg.transferInitiator!,
                        storage.masterAddress,
                        contract.getCode(),
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
                val excess = createMessage({
                    bounce: BounceMode.NoBounce,
                    dest: msg.sendExcessesTo!,
                    value: 0,
                    body: ReturnExcessesBack { queryId: msg.queryId },
                });
                // Mode 64 returns only value remaining from this inbound transfer. It cannot sweep
                // pre-existing wallet balance or operator top-ups to an attacker-chosen address.
                excess.send(
                    SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_IGNORE_ERRORS,
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
            assert (
                in.senderAddress == WalletStorage.load().masterAddress
            ) throw SccpErrors.NotMaster;
            assert (
                msg.messageId != 0 && msg.payloadHash != 0 && msg.jettonAmount != 0 &&
                msg.messageId != msg.payloadHash
            ) throw SccpErrors.InvalidMessage;
            var storage = WalletStorage.load();
            var replay = storage.replay.load();
            if (!replay.mints.exists(msg.messageId)) {
                storage.jettonBalance += msg.jettonAmount;
                replay.mints.set(msg.messageId, SCCP_PENDING_MINT);
                storage.replay = replay.toCell();
                storage.save();
            } else {
                val status = replay.mints.mustGet(msg.messageId);
                assert (
                    status == SCCP_PENDING_MINT || status == SCCP_COMPLETED
                ) throw SccpErrors.Replay;
            }
            val ack = createMessage({
                bounce: BounceMode.RichBounce,
                dest: storage.masterAddress,
                value: 0,
                body: SccpWalletMintAck {
                    queryId: msg.queryId,
                    messageId: msg.messageId,
                    bridgeAddress: msg.bridgeAddress,
                    jettonAmount: msg.jettonAmount,
                    payloadHash: msg.payloadHash,
                },
            });
            ack.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
        }

        SccpWalletBurn => {
            requireOperationValue(in.valueCoins);
            var storage = WalletStorage.load();
            assert (in.senderAddress == storage.ownerAddress) throw SccpErrors.NotOwner;
            assert (
                msg.messageId != 0 && msg.payloadHash != 0 && msg.sourceEventDigest != 0 &&
                msg.messageId != msg.payloadHash &&
                msg.messageId != msg.sourceEventDigest &&
                msg.payloadHash != msg.sourceEventDigest
            ) throw SccpErrors.InvalidMessage;
            var replay = storage.replay.load();
            var record: WalletBurnRecord;
            if (!replay.burns.exists(msg.messageId)) {
                assert (
                    msg.jettonAmount != 0 && storage.jettonBalance >= msg.jettonAmount
                ) throw SccpErrors.BalanceError;
                storage.jettonBalance -= msg.jettonAmount;
                record = {
                    status: SCCP_PENDING_BURN,
                    queryId: msg.queryId,
                    bridgeAddress: storage.ownerAddress,
                    jettonAmount: msg.jettonAmount,
                    payloadHash: msg.payloadHash,
                    sourceEventDigest: msg.sourceEventDigest,
                };
                replay.burns.set(msg.messageId, record.toCell());
                storage.replay = replay.toCell();
                storage.save();
            } else {
                record = replay.burns.mustGet(msg.messageId).load();
                assert (
                    sccpWalletBurnRecordMatches(
                        record,
                        SCCP_PENDING_BURN,
                        msg.queryId,
                        storage.ownerAddress,
                        msg.jettonAmount,
                        msg.payloadHash,
                        msg.sourceEventDigest,
                    )
                ) throw SccpErrors.Replay;
            }
            val burn = createMessage({
                bounce: BounceMode.RichBounce,
                dest: storage.masterAddress,
                value: 0,
                body: SccpMasterBurn {
                    queryId: record.queryId,
                    messageId: msg.messageId,
                    bridgeAddress: record.bridgeAddress,
                    jettonAmount: record.jettonAmount,
                    tail: (SccpMasterBurnTail {
                        payloadHash: record.payloadHash,
                        sourceEventDigest: record.sourceEventDigest,
                    }).toCell(),
                },
            });
            burn.send(
                SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_BOUNCE_ON_ACTION_FAIL,
            );
        }

        SccpRefund => {
            requireOperationValue(in.valueCoins);
            var storage = WalletStorage.load();
            assert (in.senderAddress == storage.ownerAddress) throw SccpErrors.NotOwner;
            assert (msg.refundId != 0 && msg.jettonAmount != 0) throw SccpErrors.InvalidMessage;
            var replay = storage.replay.load();
            var record: WalletRefundRecord;
            if (!replay.refunds.exists(msg.refundId)) {
                assert (storage.jettonBalance >= msg.jettonAmount) throw SccpErrors.BalanceError;
                storage.jettonBalance -= msg.jettonAmount;
                record = {
                    status: SCCP_PENDING_BURN,
                    queryId: msg.queryId,
                    bridgeAddress: storage.ownerAddress,
                    recipientOwner: msg.recipientOwner,
                    jettonAmount: msg.jettonAmount,
                };
                replay.refunds.set(msg.refundId, record.toCell());
                storage.replay = replay.toCell();
                storage.save();
            } else {
                record = replay.refunds.mustGet(msg.refundId).load();
                assert (
                    sccpWalletRefundRecordMatches(
                        record,
                        SCCP_PENDING_BURN,
                        msg.queryId,
                        storage.ownerAddress,
                        msg.recipientOwner,
                        msg.jettonAmount,
                    )
                ) throw SccpErrors.Replay;
            }
            val refund = createMessage({
                bounce: BounceMode.RichBounce,
                dest: calcDeployedJettonWallet(
                    record.recipientOwner,
                    storage.masterAddress,
                    contract.getCode(),
                ),
                value: 0,
                body: SccpWalletRefundTransfer {
                    queryId: record.queryId,
                    refundId: msg.refundId,
                    bridgeAddress: record.bridgeAddress,
                    recipientOwner: record.recipientOwner,
                    jettonAmount: record.jettonAmount,
                },
            });
            refund.send(
                SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_BOUNCE_ON_ACTION_FAIL,
            );
        }

        SccpWalletRefundTransfer => {
            var storage = WalletStorage.load();
            assert (msg.recipientOwner == storage.ownerAddress) throw SccpErrors.InvalidCallback;
            assert (
                in.senderAddress ==
                calcAddressOfJettonWallet(
                    msg.bridgeAddress,
                    storage.masterAddress,
                    contract.getCode(),
                )
            ) throw SccpErrors.NotValidWallet;
            var replay = storage.replay.load();
            var record: WalletRefundRecord;
            val destinationReplayId = sccpWalletRefundDestinationReplayId(
                msg.bridgeAddress,
                msg.refundId,
            );
            if (!replay.refunds.exists(destinationReplayId)) {
                storage.jettonBalance += msg.jettonAmount;
                record = {
                    status: SCCP_COMPLETED,
                    queryId: msg.queryId,
                    bridgeAddress: msg.bridgeAddress,
                    recipientOwner: msg.recipientOwner,
                    jettonAmount: msg.jettonAmount,
                };
                replay.refunds.set(destinationReplayId, record.toCell());
                storage.replay = replay.toCell();
                storage.save();
            } else {
                record = replay.refunds.mustGet(destinationReplayId).load();
                assert (
                    sccpWalletRefundRecordMatches(
                        record,
                        SCCP_COMPLETED,
                        msg.queryId,
                        msg.bridgeAddress,
                        msg.recipientOwner,
                        msg.jettonAmount,
                    )
                ) throw SccpErrors.Replay;
            }
            val ack = createMessage({
                bounce: BounceMode.NoBounce,
                dest: record.bridgeAddress,
                value: 0,
                body: SccpWalletRefundAck {
                    queryId: record.queryId,
                    refundId: msg.refundId,
                    recipientOwner: record.recipientOwner,
                    jettonAmount: record.jettonAmount,
                },
            });
            ack.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_IGNORE_ERRORS);
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
