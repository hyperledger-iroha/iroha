import "@gen/TairaXorJettonWallet.code"
import "constants"
import "errors"
import "jetton-utils"
import "messages"
import "proof-verifier"
import "sccp-codec"
import "storage"

contract TairaXorJettonMaster {
    author: "Hyperledger Iroha"
    version: "1.0.0"
    description: "Immutable 9-decimal Taira XOR Jetton master governed by SCCP proofs"
    incomingMessages: AllowedMasterMessage
    storage: MasterStorage
}

type AllowedMasterMessage =
    | SccpMasterMint
    | SccpWalletMintAck
    | SccpMasterBurn
    | BurnNotificationForMinter
    | RequestWalletAddress
    | TopUpTons

fun assertMasterConfiguration(storage: MasterStorage): BridgeConfig {
    assert (storage.storageVersion == SCCP_STORAGE_VERSION) throw SccpErrors.InvalidConfiguration;
    val config = storage.bridgeConfig.load();
    val hashes = config.routeHashes.load();
    val finality = config.finalityHashes.load();
    val finalityTail = finality.tail.load();
    assert (getNetworkGlobalId() == config.expectedGlobalId) throw SccpErrors.WrongNetwork;
    assert (
        storage.routeConfigurationHash == hashes.routeConfigurationHash
    ) throw SccpErrors.InvalidConfiguration;
    assert (
        config.routeRevision != 0 && config.tairaToTonMultiplier == SCCP_TAIRA_TO_TON_MULTIPLIER
    ) throw SccpErrors.InvalidConfiguration;
    assert (
        hashes.sourceLaneHash == sccpLaneHash(config.sourceLaneBytes) &&
        hashes.destinationLaneHash == sccpLaneHash(config.destinationLaneBytes) &&
        finalityTail.verifyingKey.hash() == finalityTail.verifyingKeyCellHash &&
        canonicalBls12381VerifyingKeyHash(finalityTail.verifyingKey) == finality.verifyingKeyHash
    ) throw SccpErrors.InvalidConfiguration;
    return config;
}

fun dispatchWalletMint(record: MasterMintRecord, messageId: uint256) {
    val walletCode = tairaXorJettonWalletCompiledCode();
    val mint = createMessage({
        bounce: BounceMode.Only256BitsOfBody,
        dest: calcDeployedJettonWallet(record.recipient, contract.getAddress(), walletCode),
        value: SCCP_OPERATION_VALUE,
        body: SccpWalletMint {
            queryId: record.queryId,
            messageId,
            bridgeAddress: record.bridgeAddress,
            jettonAmount: record.jettonAmount,
            payloadHash: record.payloadHash,
        },
    });
    mint.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun sendMintAck(destination: address, record: MasterMintRecord, messageId: uint256) {
    val ack = createMessage({
        bounce: BounceMode.NoBounce,
        dest: destination,
        value: 0,
        body: SccpMasterMintAck {
            queryId: record.queryId,
            messageId,
            recipient: record.recipient,
            jettonAmount: record.jettonAmount,
            payloadHash: record.payloadHash,
        },
    });
    ack.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_IGNORE_ERRORS);
}

fun onBouncedMessage(in: InMessageBounced) {
    in.bouncedBody.skipBouncedPrefix();
    val msg = lazy SccpWalletMint.fromSlice(in.bouncedBody);
    var storage = MasterStorage.load();
    var replay = storage.replay.load();
    assert (replay.mints.exists(msg.messageId)) throw SccpErrors.UnknownPendingOperation;
    val record = replay.mints.mustGet(msg.messageId).load();
    if (
        record.status == SCCP_PENDING_MINT && record.queryId == msg.queryId &&
        record.jettonAmount == msg.jettonAmount &&
        record.payloadHash == msg.payloadHash
    ) {
        assert (storage.totalSupply >= msg.jettonAmount) throw SccpErrors.BalanceError;
        storage.totalSupply -= msg.jettonAmount;
        replay.mints.delete(msg.messageId);
        storage.replay = replay.toCell();
        storage.save();
    }
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMasterMessage.fromSlice(in.body);
    match (msg) {
        SccpMasterMint => {
            assert (in.valueCoins >= SCCP_OPERATION_VALUE) throw SccpErrors.NotEnoughGas;
            var storage = MasterStorage.load();
            val config = assertMasterConfiguration(storage);
            val tail = msg.tail.load();
            val parsed = parseTairaToTonPayload(tail.canonicalPayload, config.routeRevision);
            val payload = decodeCanonicalPayloadCells(tail.canonicalPayload);
            val payloadHash = sccpPayloadHash(payload);
            val messageId = sccpMessageId(config.destinationLaneBytes, payload);
            assert (
                messageId == msg.messageId && payloadHash == tail.payloadHash &&
                parsed.recipient == msg.recipient &&
                parsed.amount * config.tairaToTonMultiplier == msg.jettonAmount
            ) throw SccpErrors.InvalidPayload;
            val hashes = config.routeHashes.load();
            val hashTail = hashes.tail.load();
            val finality = config.finalityHashes.load();
            val finalityTail = finality.tail.load();
            val policy: DestinationProofPolicy = {
                messageId,
                payloadHash,
                statementHash: tail.statementHash,
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
                    tail.publicSignals,
                    tail.proof,
                    finalityTail.verifyingKey,
                )
            ) throw SccpErrors.ProofRejected;

            var replay = storage.replay.load();
            if (!replay.mints.exists(messageId)) {
                val record: MasterMintRecord = {
                    status: SCCP_PENDING_MINT,
                    queryId: msg.queryId,
                    bridgeAddress: in.senderAddress,
                    recipient: msg.recipient,
                    jettonAmount: msg.jettonAmount,
                    payloadHash,
                };
                storage.totalSupply += msg.jettonAmount;
                replay.mints.set(messageId, record.toCell());
                storage.replay = replay.toCell();
                storage.save();
                reserveToncoinsOnBalance(SCCP_MIN_MASTER_RESERVE, RESERVE_MODE_AT_MOST);
                dispatchWalletMint(record, messageId);
            } else {
                val record = replay.mints.mustGet(messageId).load();
                assert (
                    record.recipient == msg.recipient && record.jettonAmount == msg.jettonAmount &&
                    record.payloadHash == tail.payloadHash
                ) throw SccpErrors.Replay;
                if (record.status == SCCP_COMPLETED) {
                    sendMintAck(in.senderAddress, record, messageId);
                } else {
                    dispatchWalletMint(record, messageId);
                }
            }
        }

        SccpWalletMintAck => {
            var storage = MasterStorage.load();
            assertMasterConfiguration(storage);
            var replay = storage.replay.load();
            assert (replay.mints.exists(msg.messageId)) throw SccpErrors.UnknownPendingOperation;
            var record = replay.mints.mustGet(msg.messageId).load();
            assert (
                record.status == SCCP_PENDING_MINT && record.queryId == msg.queryId &&
                record.jettonAmount == msg.jettonAmount &&
                record.payloadHash == msg.payloadHash &&
                in.senderAddress ==
                calcAddressOfJettonWallet(
                    record.recipient,
                    contract.getAddress(),
                    tairaXorJettonWalletCompiledCode(),
                )
            ) throw SccpErrors.InvalidCallback;
            record.status = SCCP_COMPLETED;
            replay.mints.set(msg.messageId, record.toCell());
            storage.replay = replay.toCell();
            storage.save();
            sendMintAck(msg.bridgeAddress, record, msg.messageId);
        }

        SccpMasterBurn => {
            var storage = MasterStorage.load();
            assertMasterConfiguration(storage);
            assert (
                in.senderAddress ==
                calcAddressOfJettonWallet(
                    msg.bridgeAddress,
                    contract.getAddress(),
                    tairaXorJettonWalletCompiledCode(),
                )
            ) throw SccpErrors.NotValidWallet;
            val tail = msg.tail.load();
            var replay = storage.replay.load();
            if (!replay.burns.exists(msg.messageId)) {
                assert (storage.totalSupply >= msg.jettonAmount) throw SccpErrors.BalanceError;
                val record: MasterBurnRecord = {
                    status: SCCP_COMPLETED,
                    queryId: msg.queryId,
                    bridgeAddress: msg.bridgeAddress,
                    jettonAmount: msg.jettonAmount,
                    payloadHash: tail.payloadHash,
                    sourceEventDigest: tail.sourceEventDigest,
                };
                storage.totalSupply -= msg.jettonAmount;
                replay.burns.set(msg.messageId, record.toCell());
                storage.replay = replay.toCell();
                storage.save();
            } else {
                val record = replay.burns.mustGet(msg.messageId).load();
                assert (
                    record.status == SCCP_COMPLETED && record.bridgeAddress == msg.bridgeAddress &&
                    record.jettonAmount == msg.jettonAmount &&
                    record.payloadHash == tail.payloadHash &&
                    record.sourceEventDigest == tail.sourceEventDigest
                ) throw SccpErrors.Replay;
            }
            val ack = createMessage({
                bounce: BounceMode.NoBounce,
                dest: msg.bridgeAddress,
                value: 0,
                body: SccpMasterBurnAck {
                    queryId: msg.queryId,
                    messageId: msg.messageId,
                    jettonAmount: msg.jettonAmount,
                    payloadHash: tail.payloadHash,
                    sourceEventDigest: tail.sourceEventDigest,
                },
            });
            ack.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_IGNORE_ERRORS);
        }

        BurnNotificationForMinter => {
            var storage = MasterStorage.load();
            assertMasterConfiguration(storage);
            assert (
                in.senderAddress ==
                calcAddressOfJettonWallet(
                    msg.burnInitiator,
                    contract.getAddress(),
                    tairaXorJettonWalletCompiledCode(),
                )
            ) throw SccpErrors.NotValidWallet;
            assert (storage.totalSupply >= msg.jettonAmount) throw SccpErrors.BalanceError;
            storage.totalSupply -= msg.jettonAmount;
            storage.save();
            if (msg.sendExcessesTo != null) {
                val excess = createMessage({
                    bounce: BounceMode.NoBounce,
                    dest: msg.sendExcessesTo!,
                    value: 0,
                    body: ReturnExcessesBack { queryId: msg.queryId },
                });
                excess.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_IGNORE_ERRORS);
            }
        }

        RequestWalletAddress => {
            val ownerCell: Cell<address>? = msg.includeOwnerAddress
                ? msg.ownerAddress.toCell()
                : null;
            var wallet: address? = null;
            if (msg.ownerAddress.getWorkchain() == SCCP_WORKCHAIN) {
                wallet = calcAddressOfJettonWallet(
                    msg.ownerAddress,
                    contract.getAddress(),
                    tairaXorJettonWalletCompiledCode(),
                );
            }
            val response = createMessage({
                bounce: BounceMode.NoBounce,
                dest: in.senderAddress,
                value: 0,
                body: ResponseWalletAddress {
                    queryId: msg.queryId,
                    jettonWalletAddress: wallet,
                    ownerAddress: ownerCell,
                },
            });
            response.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_IGNORE_ERRORS);
        }

        TopUpTons => {}
        else => throw 0xFFFF,
    }
}

get fun get_jetton_data(): JettonDataReply {
    val storage = MasterStorage.load();
    return {
        totalSupply: storage.totalSupply,
        mintable: true,
        adminAddress: null,
        jettonContent: storage.metadata as Cell<OnchainMetadataReply>,
        jettonWalletCode: tairaXorJettonWalletCompiledCode(),
    };
}

get fun get_wallet_address(ownerAddress: address): address {
    return calcAddressOfJettonWallet(
        ownerAddress,
        contract.getAddress(),
        tairaXorJettonWalletCompiledCode(),
    );
}

get fun get_sccp_route_configuration_hash(): uint256 {
    return MasterStorage.load().routeConfigurationHash;
}
