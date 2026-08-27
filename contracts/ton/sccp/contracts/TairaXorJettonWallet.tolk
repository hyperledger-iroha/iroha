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
        bounce: BounceMode.Only256BitsOfBody,
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
    in.bouncedBody.skipBouncedPrefix();
    val msg = lazy WalletBounceMessage.fromSlice(in.bouncedBody);
    var storage = WalletStorage.load();
    var replay = storage.replay.load();
    match (msg) {
        InternalTransferStep => {
            storage.jettonBalance += msg.jettonAmount;
        }
        BurnNotificationForMinter => {
            storage.jettonBalance += msg.jettonAmount;
        }
        SccpMasterBurn => {
            if (
                replay.burns.exists(msg.messageId) &&
                replay.burns.mustGet(msg.messageId) == SCCP_PENDING_BURN
            ) {
                storage.jettonBalance += msg.jettonAmount;
                replay.burns.delete(msg.messageId);
            }
        }
        SccpWalletMintAck => {
            if (
                replay.mints.exists(msg.messageId) &&
                replay.mints.mustGet(msg.messageId) == SCCP_PENDING_MINT
            ) {
                assert (storage.jettonBalance >= msg.jettonAmount) throw SccpErrors.BalanceError;
                storage.jettonBalance -= msg.jettonAmount;
                replay.mints.delete(msg.messageId);
            }
        }
        SccpWalletRefundTransfer => {
            if (
                replay.refunds.exists(msg.refundId) &&
                replay.refunds.mustGet(msg.refundId) == SCCP_PENDING_BURN
            ) {
                storage.jettonBalance += msg.jettonAmount;
                replay.refunds.delete(msg.refundId);
            }
        }
        SccpWalletRefundAck => {
            // A recipient keeps the already credited refund. A replay from the custody wallet
            // causes the acknowledgement to be sent again without crediting twice.
        }
        else => throw 0xFFFF,
    }
    storage.replay = replay.toCell();
    storage.save();
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedWalletMessage.fromSlice(in.body);
    match (msg) {
        InternalTransferStep => {
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
                reserveToncoinsOnBalance(SCCP_MIN_WALLET_RESERVE, RESERVE_MODE_AT_MOST);
                val excess = createMessage({
                    bounce: BounceMode.NoBounce,
                    dest: msg.sendExcessesTo!,
                    value: 0,
                    body: ReturnExcessesBack { queryId: msg.queryId },
                });
                // SAFETY: the fixed wallet reserve is established immediately above; only the
                // unreserved balance is returned and a failed excess refund cannot affect Jettons.
                excess.send(SEND_MODE_CARRY_ALL_BALANCE | SEND_MODE_IGNORE_ERRORS);
            }
        }

        AskToTransfer => {
            msg.forwardPayload.checkIsCorrectTlbEither();
            requireOperationValue(in.valueCoins);
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
                bounce: BounceMode.Only256BitsOfBody,
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
                bounce: BounceMode.Only256BitsOfBody,
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
            if (!replay.burns.exists(msg.messageId)) {
                assert (
                    msg.jettonAmount != 0 && storage.jettonBalance >= msg.jettonAmount
                ) throw SccpErrors.BalanceError;
                storage.jettonBalance -= msg.jettonAmount;
                replay.burns.set(msg.messageId, SCCP_PENDING_BURN);
                storage.replay = replay.toCell();
                storage.save();
            } else {
                val status = replay.burns.mustGet(msg.messageId);
                assert (status == SCCP_PENDING_BURN) throw SccpErrors.Replay;
            }
            val burn = createMessage({
                bounce: BounceMode.Only256BitsOfBody,
                dest: storage.masterAddress,
                value: 0,
                body: SccpMasterBurn {
                    queryId: msg.queryId,
                    messageId: msg.messageId,
                    bridgeAddress: storage.ownerAddress,
                    jettonAmount: msg.jettonAmount,
                    tail: (SccpMasterBurnTail {
                        payloadHash: msg.payloadHash,
                        sourceEventDigest: msg.sourceEventDigest,
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
            if (!replay.refunds.exists(msg.refundId)) {
                assert (storage.jettonBalance >= msg.jettonAmount) throw SccpErrors.BalanceError;
                storage.jettonBalance -= msg.jettonAmount;
                replay.refunds.set(msg.refundId, SCCP_PENDING_BURN);
                storage.replay = replay.toCell();
                storage.save();
            } else {
                val status = replay.refunds.mustGet(msg.refundId);
                assert (status == SCCP_PENDING_BURN) throw SccpErrors.Replay;
            }
            val refund = createMessage({
                bounce: BounceMode.Only256BitsOfBody,
                dest: calcDeployedJettonWallet(
                    msg.recipientOwner,
                    storage.masterAddress,
                    contract.getCode(),
                ),
                value: 0,
                body: SccpWalletRefundTransfer {
                    queryId: msg.queryId,
                    refundId: msg.refundId,
                    bridgeAddress: storage.ownerAddress,
                    recipientOwner: msg.recipientOwner,
                    jettonAmount: msg.jettonAmount,
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
            if (!replay.refunds.exists(msg.refundId)) {
                storage.jettonBalance += msg.jettonAmount;
                replay.refunds.set(msg.refundId, SCCP_COMPLETED);
                storage.replay = replay.toCell();
                storage.save();
            }
            val ack = createMessage({
                bounce: BounceMode.NoBounce,
                dest: msg.bridgeAddress,
                value: 0,
                body: SccpWalletRefundAck {
                    queryId: msg.queryId,
                    refundId: msg.refundId,
                    recipientOwner: msg.recipientOwner,
                    jettonAmount: msg.jettonAmount,
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
