//! Selectable Native AMX phase-cut qualification and replay evidence.

use iroha_test_network::NativeAmxFaultPhase;

use super::*;

fn musubi_selectable_fault_localnet_builder() -> NetworkBuilder {
    let treasury = gas_account()
        .canonical_i105()
        .expect("canonical SoraFS pin-fee treasury");
    musubi_fault_localnet_builder()
        .with_consensus_message_control()
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanIssueSorafsReplicationOrder),
            ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanCompleteSorafsReplicationOrder),
            ALICE_ID.clone(),
        ))
        .with_config_layer(move |layer| {
            layer.write(
                ["governance", "sorafs_pin_fee_treasury_account"],
                treasury.clone(),
            );
        })
}

async fn run_selectable_musubi_publication_phase_cut(
    phase: NativeAmxFaultPhase,
    phase_label: &str,
) -> Result<bool> {
    let context = format!("selectable Musubi publication phase cut {phase_label}");
    let Some(network) =
        sandbox::start_network_async_or_skip(musubi_selectable_fault_localnet_builder(), &context)
            .await?
    else {
        return Ok(false);
    };

    let result: Result<()> = async {
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|layer| ConfigLayer(layer.into_owned()))
            .collect();
        let peers = network.peers().iter().cloned().collect::<Vec<_>>();
        ensure!(
            peers.len() == PEERS,
            "{context}: phase cut requires exactly four voting peers"
        );
        let submitter = peers[0].client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
        let fixture = prepare_selectable_musubi_publication(&network, &submitter, &context).await?;

        let mut pre_cut_snapshot = None;
        for (index, peer) in peers.iter().enumerate() {
            let snapshot = assert_selectable_musubi_archive_without_release(
                &peer.client(),
                &fixture,
                &format!("{context}: pre-cut peer {index}"),
            )?;
            if let Some(expected) = pre_cut_snapshot.as_ref() {
                ensure!(
                    &snapshot == expected,
                    "{context}: pre-cut peer {index} queried another finalized snapshot"
                );
            } else {
                pre_cut_snapshot = Some(snapshot);
            }
        }

        // Fresh Native AMX PrepareQC/CommitQC assembly runs only on the
        // deterministic autonomous coordinator-lane author. Derive that peer
        // from the exact durable universal-lane frontier and its embedded
        // authority committee; the global Sumeragi leader is unrelated.
        let target_index = next_universal_autonomous_lane_author_peer(&peers, &context)?;
        let target = peers[target_index].clone();
        let live_submitter = peers
            .iter()
            .enumerate()
            .find(|(index, _)| *index != target_index)
            .map(|(_, peer)| peer.client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone()))
            .ok_or_else(|| eyre!("{context}: phase cut has no live ingress peer"))?;
        let mut pre_cut_tip = None;
        let mut pre_cut_blocks_non_empty = None;
        for (index, peer) in peers.iter().enumerate() {
            let status = peer
                .status()
                .await
                .wrap_err_with(|| format!("{context}: query pre-cut status from peer {index}"))?;
            let blocks = peer.client().query(FindBlocks).execute_all()?;
            let latest = blocks
                .first()
                .ok_or_else(|| eyre!("{context}: pre-cut peer {index} returned an empty chain"))?;
            let observed_tip = (latest.header().height().get(), latest.hash());
            let observed_non_empty = u64::try_from(
                blocks.iter().filter(|block| !block.is_empty()).count(),
            )
            .wrap_err_with(|| format!("{context}: pre-cut non-empty count overflows u64"))?;
            ensure!(
                status.blocks == observed_tip.0
                    && status.blocks_non_empty == observed_non_empty
                    && status.queue_size == 0,
                "{context}: pre-cut peer {index} did not expose one settled empty-queue tip: status={status:?}, tip={observed_tip:?}, queried_non_empty={observed_non_empty}"
            );
            if let Some(expected) = pre_cut_tip {
                ensure!(
                    observed_tip == expected,
                    "{context}: pre-cut peer {index} did not share the canonical tip"
                );
            } else {
                pre_cut_tip = Some(observed_tip);
            }
            if let Some(expected) = pre_cut_blocks_non_empty {
                ensure!(
                    observed_non_empty == expected,
                    "{context}: pre-cut peer {index} reported another non-empty height"
                );
            } else {
                pre_cut_blocks_non_empty = Some(observed_non_empty);
            }
        }
        let (pre_cut_height, pre_cut_hash) =
            pre_cut_tip.ok_or_else(|| eyre!("{context}: phase cut has no canonical pre-cut tip"))?;
        let pre_cut_blocks_non_empty = pre_cut_blocks_non_empty
            .ok_or_else(|| eyre!("{context}: phase cut has no pre-cut non-empty counter"))?;
        let signed_cadence = network.block_cadence();
        let cadence_ms = u64::try_from(signed_cadence.as_millis())
            .wrap_err_with(|| format!("{context}: signed cadence overflows u64 milliseconds"))?;
        let (_, retransmit_interval_ms) =
            iroha_config::parameters::actual::sumeragi_v2_timing_ms(cadence_ms)
                .wrap_err_with(|| format!("{context}: derive Sumeragi v2 timing"))?;
        let retransmit_observation =
            Duration::from_millis(retransmit_interval_ms).saturating_mul(2);
        let commit_quorum_observation = network.da_commit_quorum_timeout().saturating_mul(2);
        let recovery_quiescence_observation = signed_cadence
            .saturating_mul(10)
            .max(retransmit_observation)
            .max(commit_quorum_observation);
        let source_id = native_amx_source_id(&fixture.transaction);
        let target_control = target
            .consensus_message_control()
            .ok_or_else(|| eyre!("{context}: target peer lacks Native AMX fault control"))?;
        let revision = target_control
            .arm_native_amx_fault(phase, source_id)
            .wrap_err_with(|| format!("{context}: arm exact phase cut"))?;
        let transaction_for_submit = fixture.transaction.clone();
        let submitter_for_submit = live_submitter.clone();
        spawn_blocking(move || submitter_for_submit.submit_transaction(&transaction_for_submit))
            .await
            .map_err(|error| eyre!("{context}: publication submit task failed: {error}"))?
            .wrap_err_with(|| format!("{context}: submit exact publication"))?;

        let ack = target_control
            .wait_for_native_amx_fault(revision, phase, source_id, STATUS_WAIT_TIMEOUT)
            .await
            .wrap_err_with(|| format!("{context}: wait for durable phase acknowledgement"))?;
        ensure!(
            ack.revision == revision && ack.phase == phase && ack.source_id == source_id,
            "{context}: durable phase acknowledgement did not bind the exact publication"
        );

        let publish_entrypoint = fixture.transaction.hash_as_entrypoint();
        let mut recovery_heartbeat = None;
        let live_block_before_restart = if phase == NativeAmxFaultPhase::BeforeWorldCommit {
            // This cut is after the complete block overlay exists. The other
            // three validators must finalize the exact publication while the
            // target remains down, proving there was no target-local WSV leak.
            let live_block = wait_for_block_with_entrypoint(
                &live_submitter,
                publish_entrypoint,
                &format!("{context}: three live validators before target restart"),
            )
            .await?;
            assert_musubi_universal_home_execution_context(&live_block, &fixture.transaction)?;
            ensure!(
                live_block.header().height().get() == pre_cut_height.saturating_add(1)
                    && !live_block.is_empty(),
                "{context}: before-world-commit publication was not the exact non-empty successor of pre-cut tip h{pre_cut_height} {pre_cut_hash}: {live_block:?}"
            );
            for (index, peer) in peers
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != target_index)
            {
                let peer_block = wait_for_block_with_entrypoint(
                    &peer.client(),
                    publish_entrypoint,
                    &format!("{context}: live peer {index} before target restart"),
                )
                .await?;
                ensure!(
                    peer_block.hash() == live_block.hash(),
                    "{context}: live peer {index} committed a different publication block"
                );
                assert_selectable_musubi_publication_present(
                    &peer.client(),
                    &fixture,
                    &format!("{context}: live peer {index} before restart"),
                )?;
            }
            Some(live_block)
        } else {
            // Prepare/Commit cuts abort the sole autonomous author before it
            // can assemble and publish the executable payload. The bounded
            // recovery path advances one empty global heartbeat, but the exact
            // publication remains deferred until that author restarts.
            for (index, peer) in peers
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != target_index)
            {
                assert_selectable_musubi_archive_without_release(
                    &peer.client(),
                    &fixture,
                    &format!("{context}: live peer {index} before author restart"),
                )?;
            }

            let heartbeat_deadline = Instant::now() + STATUS_WAIT_TIMEOUT;
            let heartbeat = loop {
                let mut canonical = None;
                let mut all_live_peers_observed = true;
                for (index, peer) in peers
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != target_index)
                {
                    let blocks = peer.client().query(FindBlocks).execute_all()?;
                    ensure!(
                        !blocks.iter().any(|block| {
                            block
                                .entrypoint_hashes()
                                .any(|hash| hash == publish_entrypoint)
                        }),
                        "{context}: live peer {index} committed the deferred publication before author restart"
                    );
                    let successors = blocks
                        .iter()
                        .filter(|block| block.header().height().get() > pre_cut_height)
                        .collect::<Vec<_>>();
                    ensure!(
                        successors.len() <= 1,
                        "{context}: live peer {index} manufactured {} successors while one recovery heartbeat was armed",
                        successors.len()
                    );
                    let Some(block) = successors.first().copied() else {
                        all_live_peers_observed = false;
                        continue;
                    };
                    ensure!(
                        block.header().height().get() == pre_cut_height.saturating_add(1)
                            && block.is_empty()
                            && block.external_entrypoint_count() == 0
                            && block.time_triggers().len() == 0,
                        "{context}: live peer {index} did not commit the exact empty recovery-heartbeat successor: {block:?}"
                    );
                    let observed = (block.header().height().get(), block.hash());
                    if let Some(expected) = canonical {
                        ensure!(
                            observed == expected,
                            "{context}: live peer {index} committed another recovery heartbeat"
                        );
                    } else {
                        canonical = Some(observed);
                    }
                }
                if all_live_peers_observed {
                    break canonical.ok_or_else(|| {
                        eyre!("{context}: no live peer returned a recovery heartbeat")
                    })?;
                }
                ensure!(
                    Instant::now() < heartbeat_deadline,
                    "{context}: timed out waiting for the explicitly armed recovery heartbeat after pre-cut tip h{pre_cut_height} {pre_cut_hash}"
                );
                sleep(STATUS_POLL_INTERVAL).await;
            };

            for (index, peer) in peers
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != target_index)
            {
                let status = peer.status().await.wrap_err_with(|| {
                    format!("{context}: query recovery-heartbeat status from peer {index}")
                })?;
                ensure!(
                    status.blocks == heartbeat.0
                        && status.blocks_non_empty == pre_cut_blocks_non_empty,
                    "{context}: live peer {index} did not expose height-only recovery progress: before_non_empty={pre_cut_blocks_non_empty}, status={status:?}"
                );
            }

            // Keep the author down for ten signed cadences, two retransmission
            // intervals, and two DA commit-quorum windows. The owner-scoped
            // heartbeat is one-shot: it must not turn the stalled publication
            // into an empty-block loop anywhere in that protocol window.
            let quiescence_deadline = Instant::now() + recovery_quiescence_observation;
            while Instant::now() < quiescence_deadline {
                sleep(STATUS_POLL_INTERVAL).await;
                for (index, peer) in peers
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != target_index)
                {
                    let blocks = peer.client().query(FindBlocks).execute_all()?;
                    let latest = blocks.first().ok_or_else(|| {
                        eyre!("{context}: live peer {index} returned an empty chain")
                    })?;
                    ensure!(
                        latest.header().height().get() == heartbeat.0
                            && latest.hash() == heartbeat.1
                            && blocks
                                .iter()
                                .filter(|block| {
                                    block.header().height().get() > pre_cut_height
                                        && block.is_empty()
                                })
                                .count()
                                == 1
                            && !blocks.iter().any(|block| {
                                block
                                    .entrypoint_hashes()
                                    .any(|hash| hash == publish_entrypoint)
                            }),
                        "{context}: live peer {index} advanced beyond the one armed heartbeat before author restart"
                    );
                }
            }
            recovery_heartbeat = Some(heartbeat);
            None
        };

        ensure!(
            target.shutdown_if_started().await,
            "{context}: aborted target peer had no reapable run"
        );
        target
            .start_checked(config_layers.iter().cloned(), None)
            .await
            .wrap_err_with(|| format!("{context}: restart phase-cut target"))?;

        let live_block = match live_block_before_restart {
            Some(block) => block,
            None => {
                let block = wait_for_block_with_entrypoint(
                    &live_submitter,
                    publish_entrypoint,
                    &format!("{context}: publication after autonomous-author restart"),
                )
                .await?;
                assert_musubi_universal_home_execution_context(&block, &fixture.transaction)?;
                block
            }
        };

        let barrier_transaction = live_submitter.build_transaction(
            [InstructionBox::from(Log::new(
                Level::INFO,
                format!("Musubi selectable publication {phase_label} restart barrier"),
            ))],
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        );
        submit_approved_and_wait_for_all_peers(
            &network,
            &live_submitter,
            barrier_transaction,
            &format!("{context}: post-restart visibility barrier"),
        )
        .await?;

        // The barrier proves the restarted peer caught the same canonical
        // publication block, rather than executing a second copy locally.
        ensure!(
            live_block
                .entrypoint_hashes()
                .any(|hash| hash == publish_entrypoint),
            "{context}: selected publication block lost the exact entrypoint"
        );

        let mut canonical_snapshot = None;
        let mut canonical_publication_block = None;
        for (index, peer) in peers.iter().enumerate() {
            let client = peer.client();
            let snapshot = assert_selectable_musubi_publication_present(
                &client,
                &fixture,
                &format!("{context}: post-replay peer {index}"),
            )?;
            if let Some(expected) = canonical_snapshot.as_ref() {
                ensure!(
                    &snapshot == expected,
                    "{context}: post-replay peer {index} exposed another registry snapshot"
                );
            } else {
                canonical_snapshot = Some(snapshot);
            }

            let blocks = client.query(FindBlocks).execute_all()?;
            let empty_successors = blocks
                .iter()
                .filter(|block| {
                    block.header().height().get() > pre_cut_height && block.is_empty()
                })
                .collect::<Vec<_>>();
            if let Some((heartbeat_height, heartbeat_hash)) = recovery_heartbeat {
                ensure!(
                    empty_successors.len() == 1
                        && empty_successors[0].header().height().get() == heartbeat_height
                        && empty_successors[0].hash() == heartbeat_hash,
                    "{context}: post-replay peer {index} did not retain exactly one canonical recovery heartbeat: {empty_successors:?}"
                );
            } else {
                ensure!(
                    empty_successors.is_empty(),
                    "{context}: before-world-commit peer {index} retained an unexpected empty successor: {empty_successors:?}"
                );
            }
            let occurrences = blocks
                .iter()
                .flat_map(|block| {
                    block.entrypoint_hashes().enumerate().filter_map(
                        move |(entrypoint_index, hash)| {
                            (hash == publish_entrypoint).then_some((block, entrypoint_index))
                        },
                    )
                })
                .collect::<Vec<_>>();
            ensure!(
                occurrences.len() == 1,
                "{context}: post-replay peer {index} recorded the publication {} time(s)",
                occurrences.len()
            );
            let (publication_block, entrypoint_index) = occurrences[0];
            ensure!(
                publication_block.error(entrypoint_index).is_none(),
                "{context}: post-replay peer {index} retained a rejected publication occurrence"
            );
            if let Some((heartbeat_height, _)) = recovery_heartbeat {
                ensure!(
                    publication_block.header().height().get() > heartbeat_height,
                    "{context}: post-replay peer {index} ordered the deferred publication before its recovery heartbeat"
                );
            }
            if let Some(expected) = canonical_publication_block {
                ensure!(
                    publication_block.hash() == expected,
                    "{context}: post-replay peer {index} stored another publication block"
                );
            } else {
                canonical_publication_block = Some(publication_block.hash());
            }
        }
        if let Some((heartbeat_height, heartbeat_hash)) = recovery_heartbeat {
            eprintln!(
                "EX-297 recovery-heartbeat evidence: phase={phase_label}, cadence={signed_cadence:?}, retransmit_interval_ms={retransmit_interval_ms}, retransmit_window={retransmit_observation:?}, commit_quorum_window={commit_quorum_observation:?}, quiescence_window={recovery_quiescence_observation:?}, pre_cut_height={pre_cut_height}, pre_cut_hash={pre_cut_hash}, heartbeat_height={heartbeat_height}, heartbeat_hash={heartbeat_hash}, pre_cut_blocks_non_empty={pre_cut_blocks_non_empty}"
            );
        }
        let publication_block_hash = canonical_publication_block
            .ok_or_else(|| eyre!("{context}: no canonical publication block was observed"))?;
        let heartbeat_height = recovery_heartbeat
            .map(|(height, _)| height.to_string())
            .unwrap_or_else(|| "none".to_owned());
        let heartbeat_hash = recovery_heartbeat
            .map(|(_, hash)| hash.to_string())
            .unwrap_or_else(|| "none".to_owned());
        eprintln!(
            "EX-297 phase-completion evidence: phase={phase_label}, pre_cut_height={pre_cut_height}, pre_cut_hash={pre_cut_hash}, pre_cut_blocks_non_empty={pre_cut_blocks_non_empty}, heartbeat_height={heartbeat_height}, heartbeat_hash={heartbeat_hash}, publication_block_hash={publication_block_hash}"
        );
        Ok(())
    }
    .await;

    network.shutdown().await;
    result?;
    Ok(true)
}

pub(super) async fn run() -> Result<()> {
    init_instruction_registry();
    let context = stringify!(musubi_selectable_publication_phase_cut_matrix_is_atomic_after_replay);
    if !multilane_release_gate_requested(context)? {
        return Ok(());
    }
    for (phase, label) in [
        (NativeAmxFaultPhase::AfterPrepareQc, "after-prepare-qc"),
        (NativeAmxFaultPhase::AfterCommitQc, "after-commit-qc"),
        (
            NativeAmxFaultPhase::BeforeWorldCommit,
            "before-world-commit",
        ),
    ] {
        ensure!(
            run_selectable_musubi_publication_phase_cut(phase, label).await?,
            "{context}: sandbox restrictions skipped required phase {label}"
        );
    }
    Ok(())
}
