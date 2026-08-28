import "@gen/TairaXorJettonMaster.code"
import "@gen/TairaXorJettonWallet.code"
import "constants"
import "errors"
import "jetton-utils"
import "messages"
import "proof-verifier"
import "replay-forest"
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
    | SccpFinalizeHashStep
    | SccpFinalizeAdmission
    | SccpBridgeBurnPrepare
    | SccpMasterMintAck
    | SccpMasterBurnAck
    | SccpRetry
    | SccpDisableMinting
    | TopUpTons

type BridgeBounceMessage = SccpMasterMint | SccpWalletBurn

fun assertBridgeConfiguration(storage: BridgeStorage): BridgeConfig {
    assert (storage.storageVersion == SCCP_STORAGE_VERSION) throw SccpErrors.InvalidConfiguration;
    val config = storage.config.load();
    assertCanonicalBridgeConfigRuntimeV1(config);
    val hashes = config.routeHashes.load();
    val codeHashes = hashes.tail.load().deploymentCodeHashes.load();
    val pending = storage.pending.load();
    val replay = storage.replay.load();
    sccpReplayValidateForestV1(replay.inboundMint.load());
    sccpReplayValidateForestV1(replay.outboundBurn.load());
    assert (getNetworkGlobalId() == config.expectedGlobalId) throw SccpErrors.WrongNetwork;
    assert (
        storage.routeConfigurationHash == hashes.routeConfigurationHash &&
        storage.bridgeConfigCellHash == storage.config.hash() &&
        config.expectedGlobalId == SCCP_TON_MAINNET_GLOBAL_ID &&
        config.routeRevision != 0 &&
        config.tairaToTonMultiplier == SCCP_TAIRA_TO_TON_MULTIPLIER &&
        config.maxWrappedSupply > 0 &&
        config.maxWrappedSupply <= SCCP_MAX_JETTON_AMOUNT &&
        codeHashes.routeCodeHash == contract.getCode().hash() &&
        codeHashes.jettonMasterCodeHash == tairaXorJettonMasterCompiledCode().hash() &&
        codeHashes.jettonWalletCodeHash == tairaXorJettonWalletCompiledCode().hash() &&
        pending.mintCount <= SCCP_PENDING_OPERATION_CAP &&
        pending.burnCount <= SCCP_PENDING_OPERATION_CAP &&
        ((pending.mintCount == 0) == pending.mints.isEmpty()) &&
        ((pending.burnCount == 0) == pending.burns.isEmpty())
    ) throw SccpErrors.InvalidConfiguration;
    return config;
}

fun canonicalMasterStateInit(storage: BridgeStorage): ContractState {
    return {
        code: tairaXorJettonMasterCompiledCode(),
        data: canonicalMasterInitialStorageFromValidatedConfig(
            storage.config,
            contract.getAddress(),
        ).toCell(),
    };
}

fun masterAddress(storage: BridgeStorage): address {
    return AutoDeployAddress {
        workchain: SCCP_WORKCHAIN,
        stateInit: canonicalMasterStateInit(storage),
    }.calculateAddress();
}

fun dispatchMasterMint(
    storage: BridgeStorage,
    messageId: uint256,
    pending: PendingMint,
    replayWitnesses: Cell<SccpReplayWitnessPairV1>,
) {
    val request = createMessage({
        bounce: BounceMode.Only256BitsOfBody,
        dest: masterAddress(storage),
        value: sccpMasterMintMinimum(),
        body: SccpMasterMint {
            queryId: pending.queryId,
            messageId,
            recipient: pending.recipient,
            jettonAmount: pending.jettonAmount,
            tail: (SccpMasterMintTail {
                payloadHash: pending.payloadHash,
                payloadSha256: pending.payloadSha256,
                replayWitnesses,
            }).toCell(),
        },
    });
    request.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun dispatchWalletBurn(
    _storage: BridgeStorage,
    messageId: uint256,
    pending: PendingBurn,
    replayWitnesses: Cell<SccpReplayWitnessPairV1>,
    completed: bool = false,
) {
    val pendingTail = pending.tail.load();
    val request = createMessage({
        bounce: BounceMode.Only256BitsOfBody,
        dest: pending.sourceWallet,
        value: completed
            ? sccpBurnWalletAcknowledgementMinimum()
            : sccpWalletBurnAuthorizationMinimum(),
        body: SccpWalletBurn {
            lockSequence: pending.lockSequence,
            queryId: pending.queryId,
            messageId,
            outboundNonce: pending.outboundNonce,
            ownerAddress: pending.ownerAddress,
            jettonAmount: pending.jettonAmount,
            completed,
            tail: (SccpWalletBurnTail {
                payloadHash: pendingTail.payloadHash,
                payloadSha256: sccpReplayPayloadSha256V1(
                    decodeCanonicalPayloadCells(pendingTail.canonicalPayload),
                ),
                sourceEventDigest: pendingTail.sourceEventDigest,
                replayWitnesses,
            }).toCell(),
        },
    });
    request.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun dispatchFinalizeHashStep(
    queryId: uint64,
    version: uint16,
    messageId: uint256,
    nextBlock: uint8,
    chainingState: Cell<SccpBlake2bStateV1>,
    material: Cell<SccpFinalizeMaterialV1>,
    value: coins,
) {
    val continuation = createMessage({
        bounce: BounceMode.Only256BitsOfBody,
        dest: contract.getAddress(),
        value,
        body: SccpFinalizeHashStep {
            queryId,
            version,
            messageId,
            nextBlock,
            chainingState,
            material,
        },
    });
    continuation.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun dispatchFinalizeAdmission(
    queryId: uint64,
    version: uint16,
    messageId: uint256,
    payloadHash: uint256,
    material: Cell<SccpFinalizeMaterialV1>,
) {
    val admission = createMessage({
        bounce: BounceMode.Only256BitsOfBody,
        dest: contract.getAddress(),
        value: sccpBridgeFinalizeAdmissionMinimum(),
        body: SccpFinalizeAdmission { queryId, version, messageId, payloadHash, material },
    });
    admission.send(SEND_MODE_PAY_FEES_SEPARATELY | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun onBouncedMessage(in: InMessageBounced) {
    reserveSccpPreMessageBalance(in.valueCoins, sccpBridgeStorageReserve());
    // The operation remains reserved. `SccpRetry` is permissionless and resends the exact record;
    // no balance or replay state is rolled back merely because a callback bounced.
    in.bouncedBody.skipBouncedPrefix();
}

fun onInternalMessage(in: InMessage) {
    reserveSccpPreMessageBalance(in.valueCoins, sccpBridgeStorageReserve());
    val msg = lazy AllowedBridgeMessage.fromSlice(in.body);
    match (msg) {
        SccpFinalizeFromTaira => {
            requireSccpInboundBudget(in.valueCoins, sccpBridgeFinalizeMinimum());
            assert (isCanonicalSccpMessageVersion(msg.version)) throw SccpErrors.InvalidMessage;
            val payload = decodeCanonicalPayloadCells(msg.canonicalPayload);
            val config = assertBridgeConfiguration(BridgeStorage.load());
            assertCanonicalDestinationLaneRuntimeV1(config);
            val parsed = parseProvenTairaToTonPayloadCells(payload, config.routeRevision);
            val blockCount = sccpPayloadHashBlockCountV1(payload);
            assert (
                blockCount > 0 && blockCount <= SCCP_MAX_TAIRA_TO_TON_HASH_BLOCKS
            ) throw SccpErrors.InvalidPayload;
            val material = SccpFinalizeMaterialV1 {
                statementHash: msg.statementHash,
                recipient: parsed.recipient,
                amountScale9: parsed.amount,
                publicSignals: msg.publicSignals,
                proof: msg.proof,
                canonicalPayload: msg.canonicalPayload,
                replayWitnesses: msg.replayWitnesses,
            }.toCell();
            val state = sccpPayloadHashBlockV1(
                payload,
                sccpBlake2bInitialStateV1(),
                0,
            );
            if (blockCount == 1) {
                dispatchFinalizeAdmission(
                    msg.queryId,
                    msg.version,
                    msg.messageId,
                    sccpPayloadHashFromStateV1(state),
                    material,
                );
            } else {
                dispatchFinalizeHashStep(
                    msg.queryId,
                    msg.version,
                    msg.messageId,
                    1,
                    state,
                    material,
                    sccpBridgeFinalizeContinuationMinimum(blockCount - 1),
                );
            }
        }

        SccpFinalizeHashStep => {
            assert (in.senderAddress == contract.getAddress()) throw SccpErrors.NotBridge;
            requireSccpInboundBudget(
                in.valueCoins,
                sccpBridgeFinalizeAdmissionMinimum(),
            );
            assert (isCanonicalSccpMessageVersion(msg.version)) throw SccpErrors.InvalidMessage;
            val material = msg.material.load();
            val payload = decodeCanonicalPayloadCells(material.canonicalPayload);
            val blockCount = sccpPayloadHashBlockCountV1(payload);
            assert (
                blockCount > 1 && blockCount <= SCCP_MAX_TAIRA_TO_TON_HASH_BLOCKS &&
                msg.nextBlock > 0 &&
                msg.nextBlock <
                blockCount
            ) throw SccpErrors.InvalidPayload;
            requireSccpInboundBudget(
                in.valueCoins,
                sccpBridgeFinalizeContinuationMinimum(blockCount - msg.nextBlock),
            );
            val state = sccpPayloadHashBlockV1(
                payload,
                msg.chainingState,
                msg.nextBlock,
            );
            val following = (msg.nextBlock + 1) as int;
            if (following == blockCount) {
                dispatchFinalizeAdmission(
                    msg.queryId,
                    msg.version,
                    msg.messageId,
                    sccpPayloadHashFromStateV1(state),
                    msg.material,
                );
            } else {
                dispatchFinalizeHashStep(
                    msg.queryId,
                    msg.version,
                    msg.messageId,
                    following as uint8,
                    state,
                    msg.material,
                    sccpBridgeFinalizeContinuationMinimum(blockCount - following),
                );
            }
        }

        SccpFinalizeAdmission => {
            assert (in.senderAddress == contract.getAddress()) throw SccpErrors.NotBridge;
            requireSccpInboundBudget(in.valueCoins, sccpBridgeFinalizeAdmissionMinimum());
            assert (isCanonicalSccpMessageVersion(msg.version)) throw SccpErrors.InvalidMessage;
            val material = msg.material.load();
            val payload = decodeCanonicalPayloadCells(material.canonicalPayload);
            val payloadHash = msg.payloadHash;
            assert (payloadHash != 0) throw SccpErrors.InvalidPayload;
            var storage = BridgeStorage.load();
            val config = assertBridgeConfiguration(storage);
            val parsed: ParsedTairaToTonTransfer = {
                recipient: material.recipient,
                amount: material.amountScale9,
            };
            val messageId = sccpMessageId(config.destinationLaneBytes, payload);
            assert (messageId == msg.messageId) throw SccpErrors.InvalidMessage;
            val hashes = config.routeHashes.load();
            val hashTail = hashes.tail.load();
            val finality = config.finalityHashes.load();
            val finalityTail = finality.tail.load();
            val policy: DestinationProofPolicy = {
                messageId,
                payloadHash,
                statementHash: material.statementHash,
                routeConfigurationHash: hashes.routeConfigurationHash,
                destinationBindingHash: hashTail.destinationBindingHash,
                semanticProofProfileHash: hashTail.semanticProofProfileHash,
                soraFinalityAnchorHash: finality.soraFinalityAnchorHash,
                verifyingKeyHash: finality.verifyingKeyHash,
                verifyingKeyCellHash: finalityTail.verifyingKeyCellHash,
            };
            assert (
                verifyTairaDestinationProofWithBoundKey(
                    policy,
                    material.publicSignals,
                    material.proof,
                    finalityTail.verifyingKey,
                )
            ) throw SccpErrors.ProofRejected;

            val replayWitnesses = material.replayWitnesses.load();
            val payloadSha256 = sccpReplayPayloadSha256V1(payload);
            val amount = sccpCheckedScaleAmount(
                parsed.amount,
                config.tairaToTonMultiplier,
                config.maxWrappedSupply,
            );
            val replayRecord: SccpTonReplayRecordV1 = {
                operation: SCCP_REPLAY_TON_BRIDGE_INBOUND_MINT,
                replayId: messageId,
                payloadSha256,
                amountScale9: parsed.amount,
                principal: parsed.recipient,
                auxiliaryIdentitySha256: sccpReplayAuxiliaryWordSha256V1(
                    hashTail.destinationBindingHash,
                ),
            };
            val replayDomain = sccpTonReplayDomainHashV1(
                SCCP_REPLAY_TON_BRIDGE_INBOUND_MINT,
                config.routeRevision,
                hashes.routeConfigurationHash,
                contract.getAddress(),
            );
            var replay = storage.replay.load();
            var inboundForest = replay.inboundMint.load();
            val admission = sccpReplayAdmitV1(
                mutate inboundForest,
                replayDomain,
                replayRecord,
                replayWitnesses.bridge,
            );
            var pendingState = storage.pending.load();
            val downstreamWitnesses = SccpReplayWitnessPairV1 {
                first: replayWitnesses.master,
                second: replayWitnesses.wallet,
            }.toCell();
            if (admission.occupiedNow) {
                assert (!storage.mintingDisabled) throw SccpErrors.MintingDisabled;
                assert (
                    sccpPendingCapacityAvailable(pendingState.mintCount)
                ) throw SccpErrors.PendingCapacityExceeded;
                assert (
                    amount > 0 && amount <= config.maxWrappedSupply
                ) throw SccpErrors.SupplyCapExceeded;
                val pending: PendingMint = {
                    queryId: msg.queryId,
                    recipient: parsed.recipient,
                    jettonAmount: amount,
                    payloadHash,
                    payloadSha256,
                };
                pendingState.mints.set(messageId, pending.toCell());
                pendingState.mintCount += 1;
                replay.inboundMint = inboundForest.toCell();
                storage.replay = replay.toCell();
                storage.pending = pendingState.toCell();
                storage.save();
                emitSccpReplayDeltaV1(SCCP_REPLAY_TON_BRIDGE_INBOUND_MINT, admission);
                dispatchMasterMint(storage, messageId, pending, downstreamWitnesses);
            } else if (pendingState.mints.exists(messageId)) {
                val pending = pendingState.mints.mustGet(messageId).load();
                assert (
                    pending.queryId == msg.queryId && pending.recipient == parsed.recipient &&
                    pending.jettonAmount == amount &&
                    pending.payloadHash == payloadHash &&
                    pending.payloadSha256 == payloadSha256
                ) throw SccpErrors.Replay;
                dispatchMasterMint(
                    storage,
                    messageId,
                    pending,
                    downstreamWitnesses,
                );
            }
        }

        SccpBridgeBurnPrepare => {
            requireSccpInboundBudget(in.valueCoins, sccpBridgeBurnPrepareMinimum());
            var storage = BridgeStorage.load();
            val config = assertBridgeConfiguration(storage);
            val expectedWallet = calcAddressOfJettonWallet(
                msg.ownerAddress,
                masterAddress(storage),
                contract.getAddress(),
                tairaXorJettonWalletCompiledCode(),
                config.routeRevision,
                storage.routeConfigurationHash,
                config.maxWrappedSupply,
            );
            assert (in.senderAddress == expectedWallet) throw SccpErrors.NotValidWallet;
            val prepareTail = msg.tail.load();
            assert (
                msg.ownerAddress.getWorkchain() == SCCP_WORKCHAIN &&
                sccpAmountWithinRouteCap(msg.jettonAmount, config.maxWrappedSupply) &&
                prepareTail.payloadHash != 0 &&
                prepareTail.payloadSha256 != 0
            ) throw SccpErrors.InvalidMessage;
            val (recipientShapeValid, recipientBytes) = readTairaRecipientBytes(
                prepareTail.tairaRecipient,
            );
            assert (recipientShapeValid) throw SccpErrors.InvalidPayload;
            val canonical = buildTonToTairaPayloadFromValidatedRecipientBytes(
                msg.ownerAddress,
                recipientBytes,
                msg.outboundNonce,
                config.routeRevision,
                msg.jettonAmount as uint128,
            );
            val canonicalCell = canonical.toCell();
            val payloadHash = prepareTail.payloadHash;
            val payloadSha256 = sccpReplayPayloadSha256V1(canonical);
            assert (payloadSha256 == prepareTail.payloadSha256) throw SccpErrors.InvalidPayload;
            val messageId = sccpMessageId(config.sourceLaneBytes, canonical);
            val sourceEventDigest = sccpSourceEventDigest(
                config.routeHashes.load().sourceLaneHash,
                messageId,
                payloadHash,
            );
            val witnesses = prepareTail.replayWitnesses.load();
            val replayRecord: SccpTonReplayRecordV1 = {
                operation: SCCP_REPLAY_TON_BRIDGE_OUTBOUND_BURN,
                replayId: messageId,
                payloadSha256,
                amountScale9: msg.jettonAmount as uint128,
                principal: msg.ownerAddress,
                auxiliaryIdentitySha256: sccpReplayAuxiliaryWordSha256V1(sourceEventDigest),
            };
            val replayDomain = sccpTonReplayDomainHashV1(
                SCCP_REPLAY_TON_BRIDGE_OUTBOUND_BURN,
                config.routeRevision,
                storage.routeConfigurationHash,
                contract.getAddress(),
            );
            var replay = storage.replay.load();
            var outboundForest = replay.outboundBurn.load();
            val admission = sccpReplayAdmitV1(
                mutate outboundForest,
                replayDomain,
                replayRecord,
                witnesses.bridge,
            );
            val submitted: PendingBurn = {
                lockSequence: msg.lockSequence,
                queryId: msg.queryId,
                outboundNonce: msg.outboundNonce,
                sourceWallet: in.senderAddress,
                ownerAddress: msg.ownerAddress,
                jettonAmount: msg.jettonAmount,
                tail: (PendingBurnTail {
                    payloadHash,
                    sourceEventDigest,
                    canonicalPayload: canonicalCell,
                }).toCell(),
            };
            var pendingState = storage.pending.load();
            val downstream = SccpReplayWitnessPairV1 {
                first: witnesses.walletAuthorization,
                second: witnesses.master,
            }.toCell();
            if (admission.occupiedNow) {
                assert (
                    sccpPendingCapacityAvailable(pendingState.burnCount)
                ) throw SccpErrors.PendingCapacityExceeded;
                pendingState.burns.set(messageId, submitted.toCell());
                pendingState.burnCount += 1;
                replay.outboundBurn = outboundForest.toCell();
                storage.replay = replay.toCell();
                storage.pending = pendingState.toCell();
                storage.save();
                emitSccpReplayDeltaV1(SCCP_REPLAY_TON_BRIDGE_OUTBOUND_BURN, admission);
                dispatchWalletBurn(storage, messageId, submitted, downstream);
            } else if (pendingState.burns.exists(messageId)) {
                val pending = pendingState.burns.mustGet(messageId).load();
                val pendingTail = pending.tail.load();
                assert (
                    pending.lockSequence == submitted.lockSequence &&
                    pending.queryId == submitted.queryId &&
                    pending.outboundNonce == submitted.outboundNonce &&
                    pending.sourceWallet == submitted.sourceWallet &&
                    pending.ownerAddress == submitted.ownerAddress &&
                    pending.jettonAmount == submitted.jettonAmount &&
                    pendingTail.payloadHash == payloadHash &&
                    pendingTail.sourceEventDigest == sourceEventDigest &&
                    pendingTail.canonicalPayload.hash() == canonicalCell.hash()
                ) throw SccpErrors.Replay;
                dispatchWalletBurn(storage, messageId, pending, downstream);
            } else {
                dispatchWalletBurn(storage, messageId, submitted, downstream, true);
            }
        }

        SccpMasterMintAck => {
            requireSccpInboundBudget(in.valueCoins, sccpMintBridgeCompletionMinimum());
            var storage = BridgeStorage.load();
            assertBridgeConfiguration(storage);
            assert (in.senderAddress == masterAddress(storage)) throw SccpErrors.NotMaster;
            var pendingState = storage.pending.load();
            assert (pendingState.mints.exists(msg.messageId)) throw SccpErrors.InvalidCallback;
            val pending = pendingState.mints.mustGet(msg.messageId).load();
            assert (
                pending.queryId == msg.queryId && pending.recipient == msg.recipient &&
                pending.jettonAmount == msg.jettonAmount &&
                pending.payloadHash == msg.payloadHash
            ) throw SccpErrors.InvalidCallback;
            pendingState.mints.delete(msg.messageId);
            assert (pendingState.mintCount != 0) throw SccpErrors.InvalidConfiguration;
            pendingState.mintCount -= 1;
            storage.pending = pendingState.toCell();
            storage.save();
        }

        SccpMasterBurnAck => {
            requireSccpInboundBudget(in.valueCoins, sccpBurnBridgeCompletionMinimum());
            var storage = BridgeStorage.load();
            val config = assertBridgeConfiguration(storage);
            assertCanonicalSourceLaneRuntimeV1(config);
            var pendingState = storage.pending.load();
            assert (pendingState.burns.exists(msg.messageId)) throw SccpErrors.InvalidCallback;
            val pending = pendingState.burns.mustGet(msg.messageId).load();
            val pendingTail = pending.tail.load();
            val ackTail = msg.tail.load();
            assert (
                in.senderAddress == pending.sourceWallet &&
                pending.lockSequence == msg.lockSequence &&
                pending.queryId == msg.queryId &&
                pending.ownerAddress == ackTail.ownerAddress &&
                pending.jettonAmount == msg.jettonAmount &&
                pendingTail.payloadHash == ackTail.payloadHash &&
                pendingTail.sourceEventDigest == ackTail.sourceEventDigest
            ) throw SccpErrors.InvalidCallback;
            pendingState.burns.delete(msg.messageId);
            assert (pendingState.burnCount != 0) throw SccpErrors.InvalidConfiguration;
            pendingState.burnCount -= 1;
            storage.pending = pendingState.toCell();
            storage.save();

            val payload = decodeCanonicalPayloadCells(pendingTail.canonicalPayload);
            val event = createExternalLogMessage({
                dest: createAddressNone(),
                body: SccpOutboundEvent {
                    version: SCCP_MESSAGE_SCHEMA_VERSION,
                    laneHash: config.routeHashes.load().sourceLaneHash,
                    messageId: msg.messageId,
                    payloadHash: ackTail.payloadHash,
                    tail: (SccpOutboundEventTail {
                        sourceEventDigest: ackTail.sourceEventDigest,
                        canonicalPayloadSnake: canonicalPayloadSnake(payload),
                    }).toCell(),
                },
            });
            event.send(SEND_MODE_REGULAR);
        }

        SccpRetry => {
            requireSccpInboundBudget(in.valueCoins, sccpBridgeRetryMinimum());
            val storage = BridgeStorage.load();
            assertBridgeConfiguration(storage);
            val pendingState = storage.pending.load();
            val replayWitnesses = msg.replayWitnesses;
            if (pendingState.mints.exists(msg.messageId)) {
                dispatchMasterMint(
                    storage,
                    msg.messageId,
                    pendingState.mints.mustGet(msg.messageId).load(),
                    replayWitnesses,
                );
                return;
            }
            if (pendingState.burns.exists(msg.messageId)) {
                dispatchWalletBurn(
                    storage,
                    msg.messageId,
                    pendingState.burns.mustGet(msg.messageId).load(),
                    replayWitnesses,
                );
                return;
            }
            throw SccpErrors.UnknownPendingOperation;
        }

        SccpDisableMinting => {
            requireSccpInboundBudget(in.valueCoins, sccpMaximumComputeFee());
            var storage = BridgeStorage.load();
            val config = assertBridgeConfiguration(storage);
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

get fun get_sccp_route_configuration_hash(): uint256 {
    val storage = BridgeStorage.load();
    assert (storage.storageVersion == SCCP_STORAGE_VERSION) throw SccpErrors.InvalidConfiguration;
    return storage.routeConfigurationHash;
}

get fun get_sccp_bridge_config_cell_hash(): uint256 {
    return BridgeStorage.load().bridgeConfigCellHash;
}

get fun get_sccp_destination_binding_hash(): uint256 {
    return BridgeStorage.load().config.load().routeHashes.load().tail.load().destinationBindingHash;
}

get fun get_sccp_deployment_code_hashes(): (uint256, uint256, uint256) {
    val hashes = BridgeStorage
        .load()
        .config
        .load()
        .routeHashes
        .load()
        .tail
        .load()
        .deploymentCodeHashes
        .load();
    return (hashes.jettonMasterCodeHash, hashes.jettonWalletCodeHash, hashes.routeCodeHash);
}

get fun get_sccp_master_address(): address {
    return masterAddress(BridgeStorage.load());
}

get fun get_sccp_master_initial_state_hashes(): (uint256, uint256) {
    val init = canonicalMasterStateInit(BridgeStorage.load());
    return (init.code.hash(), init.data.hash());
}

get fun get_sccp_mint_breaker_guardians(): (uint256, uint256, uint256, uint256, uint256) {
    val guardians = BridgeStorage
        .load()
        .config
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

fun bridgeReplayForest(storage: BridgeStorage, operation: uint8): SccpReplayForestV1 {
    val replay = storage.replay.load();
    if (operation == SCCP_REPLAY_TON_BRIDGE_INBOUND_MINT) {
        return replay.inboundMint.load();
    }
    assert (operation == SCCP_REPLAY_TON_BRIDGE_OUTBOUND_BURN) throw SccpErrors.InvalidReplayDomain;
    return replay.outboundBurn.load();
}

fun bridgeReplayDomain(storage: BridgeStorage, operation: uint8): uint256 {
    val config = storage.config.load();
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
    val storage = BridgeStorage.load();
    val forest = bridgeReplayForest(storage, operation);
    return (
        bridgeReplayDomain(storage, operation),
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
    val storage = BridgeStorage.load();
    return sccpReplayVerifyMembershipV1(
        bridgeReplayForest(storage, operation),
        bridgeReplayDomain(storage, operation),
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
    val storage = BridgeStorage.load();
    return sccpReplayVerifyNonMembershipV1(
        bridgeReplayForest(storage, operation),
        bridgeReplayDomain(storage, operation),
        replayId,
        witness,
    );
}

get fun get_sccp_max_wrapped_supply(): coins {
    return BridgeStorage.load().config.load().maxWrappedSupply;
}

get fun get_sccp_minting_disabled(): bool {
    return BridgeStorage.load().mintingDisabled;
}

get fun get_sccp_pending_counts(): (uint16, uint16) {
    val pending = BridgeStorage.load().pending.load();
    return (pending.mintCount, pending.burnCount);
}

get fun get_sccp_funding_parameters(): (coins, coins, coins) {
    return (sccpBridgeFinalizeMinimum(), sccpBridgeRetryMinimum(), sccpBridgeStorageReserve());
}
