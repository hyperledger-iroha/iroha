import "@gen/TairaXorJettonWallet.code"
import "constants"
import "errors"
import "jetton-utils"
import "messages"
import "replay-forest"
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
    | SccpDisableMinting
    | TopUpTons

fun assertMasterConfiguration(storage: MasterStorage): BridgeConfig {
    assert (storage.storageVersion == SCCP_STORAGE_VERSION) throw SccpErrors.InvalidConfiguration;
    val config = storage.bridgeConfig.load();
    assertCanonicalBridgeConfigRuntimeV1(config);
    val hashes = config.routeHashes.load();
    val codeHashes = hashes.tail.load().deploymentCodeHashes.load();
    val replay = storage.replay.load();
    sccpReplayValidateForestV1(replay.mints.load());
    sccpReplayValidateForestV1(replay.burns.load());
    assert (getNetworkGlobalId() == config.expectedGlobalId) throw SccpErrors.WrongNetwork;
    assert (
        storage.routeConfigurationHash == hashes.routeConfigurationHash &&
        storage.bridgeConfigCellHash == storage.bridgeConfig.hash()
    ) throw SccpErrors.InvalidConfiguration;
    assert (
        config.expectedGlobalId == SCCP_TON_MAINNET_GLOBAL_ID && config.routeRevision != 0 &&
        config.tairaToTonMultiplier == SCCP_TAIRA_TO_TON_MULTIPLIER &&
        config.maxWrappedSupply > 0 &&
        config.maxWrappedSupply <= SCCP_MAX_JETTON_AMOUNT &&
        codeHashes.jettonMasterCodeHash == contract.getCode().hash() &&
        codeHashes.jettonWalletCodeHash == tairaXorJettonWalletCompiledCode().hash() &&
        storage.bridgeAddress.getWorkchain() == SCCP_WORKCHAIN &&
        storage.totalSupply <= config.maxWrappedSupply &&
        storage.pendingMintCount <= SCCP_PENDING_OPERATION_CAP &&
        ((storage.pendingMintCount == 0) == storage.pendingMints.isEmpty())
    ) throw SccpErrors.InvalidConfiguration;
    return config;
}

fun dispatchWalletMint(
    record: MasterMintRecord,
    messageId: uint256,
    replayWitness: Cell<SccpReplayWitnessV1>,
) {
    val tail = record.tail.load();
    val storage = MasterStorage.load();
    val config = storage.bridgeConfig.load();
    val walletCode = tairaXorJettonWalletCompiledCode();
    val mint = createMessage({
        bounce: BounceMode.RichBounce,
        dest: calcDeployedJettonWallet(
            record.recipient,
            contract.getAddress(),
            storage.bridgeAddress,
            walletCode,
            config.routeRevision,
            storage.routeConfigurationHash,
            config.maxWrappedSupply,
        ),
        value: sccpWalletMintMinimum(),
        body: SccpWalletMint {
            queryId: record.queryId,
            messageId,
            bridgeAddress: record.bridgeAddress,
            jettonAmount: record.jettonAmount,
            tail: (SccpWalletMintTail {
                payloadHash: tail.payloadHash,
                payloadSha256: tail.payloadSha256,
                replayWitness,
            }).toCell(),
        },
    });
    mint.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun sendMintAck(
    destination: address,
    record: MasterMintRecord,
    messageId: uint256,
    requestQueryId: uint64,
) {
    val ack = createMessage({
        bounce: BounceMode.NoBounce,
        dest: destination,
        value: sccpMintBridgeCompletionMinimum(),
        body: sccpMasterMintAckForRequest(record, messageId, requestQueryId),
    });
    ack.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_IGNORE_ERRORS);
}

fun onBouncedMessage(in: InMessageBounced) {
    reserveSccpPreMessageBalance(in.valueCoins, sccpMasterStorageReserve());
    val rich = lazy RichBounceBody.fromSlice(in.bouncedBody);
    val msg = lazy SccpWalletMint.fromSlice(rich.originalBody.beginParse());
    val storage = MasterStorage.load();
    if (!storage.pendingMints.exists(msg.messageId)) {
        return;
    }
    val record = storage.pendingMints.mustGet(msg.messageId).load();
    val config = storage.bridgeConfig.load();
    val recordTail = record.tail.load();
    val messageTail = msg.tail.load();
    assert (
        in.senderAddress ==
        calcAddressOfJettonWallet(
            record.recipient,
            contract.getAddress(),
            storage.bridgeAddress,
            tairaXorJettonWalletCompiledCode(),
            config.routeRevision,
            storage.routeConfigurationHash,
            config.maxWrappedSupply,
        )
    ) throw SccpErrors.InvalidCallback;
    assert (
        record.queryId == msg.queryId && record.bridgeAddress == msg.bridgeAddress &&
        record.jettonAmount == msg.jettonAmount &&
        recordTail.payloadHash == messageTail.payloadHash &&
        recordTail.payloadSha256 == messageTail.payloadSha256
    ) throw SccpErrors.InvalidCallback;
    // A bounce proves only that one asynchronous attempt failed. Another exact
    // retry may already have succeeded, so supply and replay state remain
    // reserved until an authenticated wallet acknowledgement completes them.
}

fun onInternalMessage(in: InMessage) {
    reserveSccpPreMessageBalance(in.valueCoins, sccpMasterStorageReserve());
    val msg = lazy AllowedMasterMessage.fromSlice(in.body);
    match (msg) {
        SccpMasterMint => {
            requireSccpInboundBudget(in.valueCoins, sccpMasterMintMinimum());
            var storage = MasterStorage.load();
            assert (in.senderAddress == storage.bridgeAddress) throw SccpErrors.NotBridge;
            val config = assertMasterConfiguration(storage);
            val tail = msg.tail.load();
            assert (
                msg.messageId != 0 && tail.payloadHash != 0 && tail.payloadSha256 != 0 &&
                msg.recipient.getWorkchain() == SCCP_WORKCHAIN &&
                msg.recipient != contract.getAddress() &&
                sccpAmountWithinRouteCap(msg.jettonAmount, config.maxWrappedSupply)
            ) throw SccpErrors.InvalidMessage;
            val hashes = config.routeHashes.load();
            val replayWitnesses = tail.replayWitnesses.load();
            val replayRecord: SccpTonReplayRecordV1 = {
                operation: SCCP_REPLAY_TON_MASTER_MINT,
                replayId: msg.messageId,
                payloadSha256: tail.payloadSha256,
                amountScale9: msg.jettonAmount as uint128,
                principal: msg.recipient,
                auxiliaryIdentitySha256: sccpReplayAuxiliaryAddressSha256V1(
                    storage.bridgeAddress,
                ),
            };
            val replayDomain = sccpTonReplayDomainHashV1(
                SCCP_REPLAY_TON_MASTER_MINT,
                config.routeRevision,
                hashes.routeConfigurationHash,
                contract.getAddress(),
            );
            var replay = storage.replay.load();
            var mintForest = replay.mints.load();
            val admission = sccpReplayAdmitV1(
                mutate mintForest,
                replayDomain,
                replayRecord,
                replayWitnesses.first,
            );
            val submittedRecord: MasterMintRecord = {
                queryId: msg.queryId,
                bridgeAddress: storage.bridgeAddress,
                recipient: msg.recipient,
                jettonAmount: msg.jettonAmount,
                tail: MasterMintRecordTail {
                    payloadHash: tail.payloadHash,
                    payloadSha256: tail.payloadSha256,
                }.toCell(),
            };
            if (admission.occupiedNow) {
                assert (!storage.mintingDisabled) throw SccpErrors.MintingDisabled;
                assert (
                    sccpPendingCapacityAvailable(storage.pendingMintCount)
                ) throw SccpErrors.PendingCapacityExceeded;
                assert (
                    sccpMintFitsSupplyCap(
                        storage.totalSupply,
                        msg.jettonAmount,
                        config.maxWrappedSupply,
                    )
                ) throw SccpErrors.SupplyCapExceeded;
                storage.totalSupply += msg.jettonAmount;
                storage.pendingMintCount += 1;
                storage.pendingMints.set(msg.messageId, submittedRecord.toCell());
                replay.mints = mintForest.toCell();
                storage.replay = replay.toCell();
                storage.save();
                emitSccpReplayDeltaV1(SCCP_REPLAY_TON_MASTER_MINT, admission);
                dispatchWalletMint(submittedRecord, msg.messageId, replayWitnesses.second);
            } else if (storage.pendingMints.exists(msg.messageId)) {
                val record = storage.pendingMints.mustGet(msg.messageId).load();
                val recordTail = record.tail.load();
                assert (
                    record.queryId == submittedRecord.queryId &&
                    record.bridgeAddress == submittedRecord.bridgeAddress &&
                    record.recipient == submittedRecord.recipient &&
                    record.jettonAmount == submittedRecord.jettonAmount &&
                    recordTail.payloadHash == tail.payloadHash &&
                    recordTail.payloadSha256 == tail.payloadSha256
                ) throw SccpErrors.Replay;
                dispatchWalletMint(record, msg.messageId, replayWitnesses.second);
            } else {
                sendMintAck(storage.bridgeAddress, submittedRecord, msg.messageId, msg.queryId);
            }
        }

        SccpWalletMintAck => {
            requireSccpInboundBudget(in.valueCoins, sccpMintMasterAcknowledgementMinimum());
            var storage = MasterStorage.load();
            val config = assertMasterConfiguration(storage);
            assert (
                storage.pendingMints.exists(msg.messageId)
            ) throw SccpErrors.UnknownPendingOperation;
            val record = storage.pendingMints.mustGet(msg.messageId).load();
            val recordTail = record.tail.load();
            assert (
                record.queryId == msg.queryId && record.bridgeAddress == msg.bridgeAddress &&
                record.jettonAmount == msg.jettonAmount &&
                recordTail.payloadHash == msg.payloadHash &&
                in.senderAddress ==
                calcAddressOfJettonWallet(
                    record.recipient,
                    contract.getAddress(),
                    storage.bridgeAddress,
                    tairaXorJettonWalletCompiledCode(),
                    config.routeRevision,
                    storage.routeConfigurationHash,
                    config.maxWrappedSupply,
                )
            ) throw SccpErrors.InvalidCallback;
            assert (storage.pendingMintCount != 0) throw SccpErrors.InvalidConfiguration;
            storage.pendingMintCount -= 1;
            storage.pendingMints.delete(msg.messageId);
            storage.save();
            sendMintAck(msg.bridgeAddress, record, msg.messageId, record.queryId);
        }

        SccpMasterBurn => {
            requireSccpInboundBudget(in.valueCoins, sccpMasterBurnMinimum());
            var storage = MasterStorage.load();
            val config = assertMasterConfiguration(storage);
            val tail = msg.tail.load();
            val payloadTail = tail.payload.load();
            assert (tail.bridgeAddress == storage.bridgeAddress) throw SccpErrors.NotBridge;
            assert (
                in.senderAddress ==
                calcAddressOfJettonWallet(
                    tail.ownerAddress,
                    contract.getAddress(),
                    storage.bridgeAddress,
                    tairaXorJettonWalletCompiledCode(),
                    config.routeRevision,
                    storage.routeConfigurationHash,
                    config.maxWrappedSupply,
                )
            ) throw SccpErrors.NotValidWallet;
            assert (
                msg.messageId != 0 &&
                sccpAmountWithinRouteCap(
                    msg.jettonAmount,
                    config.maxWrappedSupply,
                ) &&
                payloadTail.payloadHash != 0 &&
                payloadTail.payloadSha256 != 0 &&
                payloadTail.sourceEventDigest != 0
            ) throw SccpErrors.InvalidMessage;
            val replayId = sccpMasterBurnReplayId(tail.bridgeAddress, msg.messageId);
            val replayRecord: SccpTonReplayRecordV1 = {
                operation: SCCP_REPLAY_TON_MASTER_BURN,
                replayId,
                payloadSha256: payloadTail.payloadSha256,
                amountScale9: msg.jettonAmount as uint128,
                principal: tail.ownerAddress,
                auxiliaryIdentitySha256: sccpReplayAuxiliaryWordSha256V1(
                    payloadTail.sourceEventDigest,
                ),
            };
            val replayDomain = sccpTonReplayDomainHashV1(
                SCCP_REPLAY_TON_MASTER_BURN,
                config.routeRevision,
                storage.routeConfigurationHash,
                contract.getAddress(),
            );
            var replay = storage.replay.load();
            var burnForest = replay.burns.load();
            val admission = sccpReplayAdmitV1(
                mutate burnForest,
                replayDomain,
                replayRecord,
                payloadTail.replayWitness,
            );
            if (admission.occupiedNow) {
                assert (storage.totalSupply >= msg.jettonAmount) throw SccpErrors.BalanceError;
                storage.totalSupply -= msg.jettonAmount;
                replay.burns = burnForest.toCell();
                storage.replay = replay.toCell();
                storage.save();
                emitSccpReplayDeltaV1(SCCP_REPLAY_TON_MASTER_BURN, admission);
            }
            val ack = createMessage({
                bounce: BounceMode.NoBounce,
                dest: in.senderAddress,
                value: sccpBurnWalletAcknowledgementMinimum(),
                body: SccpMasterBurnAck {
                    lockSequence: msg.lockSequence,
                    queryId: msg.queryId,
                    messageId: msg.messageId,
                    jettonAmount: msg.jettonAmount,
                    tail: (SccpMasterBurnAckTail {
                        ownerAddress: tail.ownerAddress,
                        payloadHash: payloadTail.payloadHash,
                        sourceEventDigest: payloadTail.sourceEventDigest,
                    }).toCell(),
                },
            });
            ack.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_IGNORE_ERRORS);
        }

        BurnNotificationForMinter => {
            var storage = MasterStorage.load();
            val config = assertMasterConfiguration(storage);
            assert (
                in.senderAddress ==
                calcAddressOfJettonWallet(
                    msg.burnInitiator,
                    contract.getAddress(),
                    storage.bridgeAddress,
                    tairaXorJettonWalletCompiledCode(),
                    config.routeRevision,
                    storage.routeConfigurationHash,
                    config.maxWrappedSupply,
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
            val storage = MasterStorage.load();
            val config = assertMasterConfiguration(storage);
            val ownerCell: Cell<address>? = msg.includeOwnerAddress
                ? msg.ownerAddress.toCell()
                : null;
            var wallet: address? = null;
            if (msg.ownerAddress.getWorkchain() == SCCP_WORKCHAIN) {
                wallet = calcAddressOfJettonWallet(
                    msg.ownerAddress,
                    contract.getAddress(),
                    storage.bridgeAddress,
                    tairaXorJettonWalletCompiledCode(),
                    config.routeRevision,
                    storage.routeConfigurationHash,
                    config.maxWrappedSupply,
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

        SccpDisableMinting => {
            requireSccpInboundBudget(in.valueCoins, sccpMaximumComputeFee());
            var storage = MasterStorage.load();
            val config = assertMasterConfiguration(storage);
            assert (!storage.mintingDisabled) throw SccpErrors.MintingDisabled;
            assert (
                verifyMintBreakerAuthorization(config, contract.getAddress(), msg)
            ) throw SccpErrors.InvalidBreakerAuthorization;
            storage.mintingDisabled = true;
            storage.save();
        }

        TopUpTons => {}
        else => throw 0xFFFF,
    }
}

get fun get_jetton_data(): JettonDataReply {
    val storage = MasterStorage.load();
    return {
        totalSupply: storage.totalSupply,
        mintable: !storage.mintingDisabled,
        adminAddress: null,
        jettonContent: storage.metadata as Cell<OnchainMetadataReply>,
        jettonWalletCode: tairaXorJettonWalletCompiledCode(),
    };
}

get fun get_wallet_address(ownerAddress: address): address {
    val storage = MasterStorage.load();
    val config = storage.bridgeConfig.load();
    return calcAddressOfJettonWallet(
        ownerAddress,
        contract.getAddress(),
        storage.bridgeAddress,
        tairaXorJettonWalletCompiledCode(),
        config.routeRevision,
        storage.routeConfigurationHash,
        config.maxWrappedSupply,
    );
}

get fun get_sccp_route_configuration_hash(): uint256 {
    return MasterStorage.load().routeConfigurationHash;
}

get fun get_sccp_bridge_config_cell_hash(): uint256 {
    return MasterStorage.load().bridgeConfigCellHash;
}

get fun get_sccp_destination_binding_hash(): uint256 {
    return MasterStorage
        .load()
        .bridgeConfig
        .load()
        .routeHashes
        .load()
        .tail
        .load()
        .destinationBindingHash;
}

get fun get_sccp_deployment_code_hashes(): (uint256, uint256, uint256) {
    val hashes = MasterStorage
        .load()
        .bridgeConfig
        .load()
        .routeHashes
        .load()
        .tail
        .load()
        .deploymentCodeHashes
        .load();
    return (hashes.jettonMasterCodeHash, hashes.jettonWalletCodeHash, hashes.routeCodeHash);
}

get fun get_sccp_bridge_address(): address {
    return MasterStorage.load().bridgeAddress;
}

get fun get_sccp_initial_state_hashes(): (uint256, uint256) {
    val storage = MasterStorage.load();
    return (
        contract.getCode().hash(),
        canonicalMasterInitialStorage(storage.bridgeConfig, storage.bridgeAddress).toCell().hash(),
    );
}

get fun get_sccp_mint_breaker_guardians(): (uint256, uint256, uint256, uint256, uint256) {
    val guardians = MasterStorage
        .load()
        .bridgeConfig
        .load()
        .finalityHashes
        .load()
        .tail
        .load()
        .mintBreakerGuardians
        .load();
    val tail = guardians.tail.load();
    return (
        guardians.guardian0,
        guardians.guardian1,
        guardians.guardian2,
        tail.guardian3,
        tail.guardian4,
    );
}

get fun get_sccp_max_wrapped_supply(): coins {
    return MasterStorage.load().bridgeConfig.load().maxWrappedSupply;
}

get fun get_sccp_minting_disabled(): bool {
    return MasterStorage.load().mintingDisabled;
}

get fun get_sccp_pending_mint_count(): uint16 {
    return MasterStorage.load().pendingMintCount;
}

get fun get_sccp_funding_parameters(): (coins, coins, coins) {
    return (sccpMasterMintMinimum(), sccpMasterBurnMinimum(), sccpMasterStorageReserve());
}

fun masterReplayForest(storage: MasterStorage, operation: uint8): SccpReplayForestV1 {
    val replay = storage.replay.load();
    if (operation == SCCP_REPLAY_TON_MASTER_MINT) {
        return replay.mints.load();
    }
    assert (operation == SCCP_REPLAY_TON_MASTER_BURN) throw SccpErrors.InvalidReplayDomain;
    return replay.burns.load();
}

fun masterReplayDomain(storage: MasterStorage, operation: uint8): uint256 {
    val config = storage.bridgeConfig.load();
    return sccpTonReplayDomainHashV1(
        operation,
        config.routeRevision,
        storage.routeConfigurationHash,
        contract.getAddress(),
    );
}

get fun get_sccp_replay_state(
    operation: uint8,
    shard: uint8,
): (uint256, uint256, uint64, uint64) {
    val storage = MasterStorage.load();
    val forest = masterReplayForest(storage, operation);
    return (
        masterReplayDomain(storage, operation),
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
    val storage = MasterStorage.load();
    return sccpReplayVerifyMembershipV1(
        masterReplayForest(storage, operation),
        masterReplayDomain(storage, operation),
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
    val storage = MasterStorage.load();
    return sccpReplayVerifyNonMembershipV1(
        masterReplayForest(storage, operation),
        masterReplayDomain(storage, operation),
        replayId,
        witness,
    );
}
