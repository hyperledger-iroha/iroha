//! Native AMX routing, replay, and rotating-validator qualification scenarios.

use super::*;

pub(super) async fn run_mixed_dataspace_native_amx_routes_and_commits_with_receipts() -> Result<()>
{
    init_instruction_registry();
    let context = stringify!(mixed_dataspace_native_amx_routes_and_commits_with_receipts);
    let Some(network) = sandbox::start_network_async_or_skip(localnet_builder(), context).await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        let submit_peer = network
            .peers()
            .get(PEERS - 1)
            .ok_or_else(|| eyre!("expected {PEERS} peers"))?;
        let submitter = submit_peer.client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
        let merchant_domain =
            DomainId::try_new("merchant", "acme").expect("merchant domain");
        let treasury_domain =
            DomainId::try_new("bankvault", "bank").expect("bank vault domain");
        let acme_dataspace = DataSpaceId::new(ACME_DATASPACE);
        let bank_dataspace = DataSpaceId::new(BANK_DATASPACE);
        let transaction = submitter.build_transaction(
            [
                dataspace_setup_instruction("acme", acme_dataspace, &submitter.account)?,
                dataspace_setup_instruction("bank", bank_dataspace, &submitter.account)?,
                domain_setup_instruction_in_dataspace(
                    &merchant_domain,
                    acme_dataspace,
                    &submitter.account,
                )?,
                domain_setup_instruction_in_dataspace(
                    &treasury_domain,
                    bank_dataspace,
                    &submitter.account,
                )?,
            ],
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        );
        let entrypoint_hash = transaction.hash_as_entrypoint();

        let approved_route =
            submit_and_wait_for_approval(&submitter, transaction.clone()).await?;
        if let Some((lane_id, dataspace_id)) = approved_route {
            ensure!(
                (lane_id == LaneId::new(ACME_LANE)
                    && dataspace_id == DataSpaceId::new(ACME_DATASPACE))
                    || (lane_id == LaneId::new(UNIVERSAL_LANE)
                        && dataspace_id == DataSpaceId::UNIVERSAL),
                "approved route should be deterministic coordinator metadata; got lane {}, dataspace {}",
                lane_id.as_u32(),
                dataspace_id.as_u64()
            );
        }

        let committed_block =
            wait_for_block_with_entrypoint(&submitter, entrypoint_hash, context).await?;
        let receipt = assert_native_amx_execution_context(&committed_block, &transaction)?;
        let relay = wait_for_all_peers_to_observe_native_amx_evidence(
            &network,
            &transaction,
            committed_block.hash(),
            &receipt,
            context,
        )
        .await?;
        assert_native_amx_relay_tamper_matrix(&relay, &receipt)?;
        wait_for_diagnostics_native_amx_receipt(&submitter, &receipt, context).await?;

        submitter.submit::<InstructionBox>(
            Log::new(
                Level::INFO,
                "native AMX routing receipt convergence tick".to_owned(),
            )
            .into(),
            FeePaymentIntent::authority(Vec::new(), None),
        )?;

        Ok(())
    }
    .await;

    network.shutdown().await;
    result
}

pub(super) async fn run_native_amx_queue_journal_replays_plan_after_restart() -> Result<()> {
    init_instruction_registry();
    let context = stringify!(native_amx_queue_journal_replays_plan_after_restart);
    let Some(network) = sandbox::start_network_async_or_skip(localnet_builder(), context).await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|layer| ConfigLayer(layer.into_owned()))
            .collect();
        let admitting_peer = network
            .peers()
            .get(PEERS - 1)
            .cloned()
            .ok_or_else(|| eyre!("expected {PEERS} peers"))?;
        let submitter = admitting_peer.client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
        let merchant_domain =
            DomainId::try_new("journalmerchant", "acme").expect("merchant domain");
        let treasury_domain =
            DomainId::try_new("journalbankvault", "bank").expect("bank vault domain");
        let acme_dataspace = DataSpaceId::new(ACME_DATASPACE);
        let bank_dataspace = DataSpaceId::new(BANK_DATASPACE);
        let transaction = submitter.build_transaction(
            [
                dataspace_setup_instruction("acme", acme_dataspace, &submitter.account)?,
                dataspace_setup_instruction("bank", bank_dataspace, &submitter.account)?,
                domain_setup_instruction_in_dataspace(
                    &merchant_domain,
                    acme_dataspace,
                    &submitter.account,
                )?,
                domain_setup_instruction_in_dataspace(
                    &treasury_domain,
                    bank_dataspace,
                    &submitter.account,
                )?,
            ],
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        );
        let entrypoint_hash = transaction.hash_as_entrypoint();

        let submitter_for_submit = submitter.clone();
        let transaction_for_submit = transaction.clone();
        spawn_blocking(move || submitter_for_submit.submit_transaction(&transaction_for_submit))
            .await
            .map_err(|err| eyre!("submit task join error: {err}"))?
            .map_err(|err| eyre!("failed to submit journaled native AMX transaction: {err}"))?;

        admitting_peer.shutdown().await;
        admitting_peer
            .start_checked(config_layers.iter().cloned(), None)
            .await
            .wrap_err("restart admitting peer")?;

        let restarted_client =
            admitting_peer.client_for(&ALICE_ID, PrivateKey::clone(ALICE_KEYPAIR.private_key()));
        let block = timeout(
            STATUS_WAIT_TIMEOUT,
            wait_for_block_with_entrypoint(
                &restarted_client,
                entrypoint_hash,
                "journal replay after restart",
            ),
        )
        .await
        .map_err(|_| {
            eyre!("timed out waiting for journaled native AMX transaction after restart")
        })??;
        let receipt = assert_native_amx_execution_context(&block, &transaction)?;
        let relay = wait_for_all_peers_to_observe_native_amx_evidence(
            &network,
            &transaction,
            block.hash(),
            &receipt,
            context,
        )
        .await?;
        assert_native_amx_relay_tamper_matrix(&relay, &receipt)?;
        wait_for_diagnostics_native_amx_receipt(&restarted_client, &receipt, context).await?;

        Ok(())
    }
    .await;

    network.shutdown().await;
    result
}

pub(super) async fn run_musubi_publication_below_quorum_queue_crash_replay_keeps_projection_tuple_absent()
-> Result<()> {
    init_instruction_registry();
    let context = stringify!(
        musubi_publication_below_quorum_queue_crash_replay_keeps_projection_tuple_absent
    );
    let Some(network) =
        sandbox::start_network_async_or_skip(musubi_fault_localnet_builder(), context).await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|layer| ConfigLayer(layer.into_owned()))
            .collect();
        let peers = network.peers().iter().cloned().collect::<Vec<_>>();
        ensure!(
            peers.len() == PEERS,
            "Musubi crash replay requires four peers"
        );
        let admitting_peer = peers
            .last()
            .cloned()
            .ok_or_else(|| eyre!("Musubi crash replay has no admitting peer"))?;
        let submitter = admitting_peer.client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());

        let provider_transaction = submitter.build_transaction(
            [Box::new(RegisterProviderOwner::new(
                musubi_fault_provider(),
                submitter.account.clone(),
            ))
            .into_instruction_box()],
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        );
        submit_approved_and_wait_for_all_peers(
            &network,
            &submitter,
            provider_transaction,
            "register Musubi crash-replay seed provider",
        )
        .await?;

        let acme_dataspace = DataSpaceId::new(ACME_DATASPACE);
        let domain =
            DomainId::try_new(MUSUBI_FAULT_DOMAIN, "acme").expect("Musubi fault namespace domain");
        let namespace_home_transaction = submitter.build_transaction(
            [
                dataspace_setup_instruction("acme", acme_dataspace, &submitter.account)?,
                domain_setup_instruction_in_dataspace(&domain, acme_dataspace, &submitter.account)?,
            ],
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        );
        submit_approved_and_wait_for_all_peers(
            &network,
            &submitter,
            namespace_home_transaction,
            "establish Musubi crash-replay namespace home",
        )
        .await?;

        let binding = musubi_fault_namespace_binding();
        let binding_transaction = submitter.build_transaction(
            [InstructionBox::from(RegisterMusubiNamespaceBindingV1::new(
                binding.clone(),
                1,
            ))],
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        );
        let binding_block = submit_approved_and_wait_for_all_peers(
            &network,
            &submitter,
            binding_transaction.clone(),
            "register Musubi crash-replay namespace binding",
        )
        .await?;
        assert_musubi_universal_home_execution_context(&binding_block, &binding_transaction)?;

        let commitment = musubi_fault_archive_commitment();
        let archive_id = commitment.archive_id();
        let (manifest, lock) = musubi_fault_release_manifest_and_lock();
        let (_, genesis_hash, latest_time_ms) = musubi_fault_snapshot_and_time(&submitter)?;
        let staging_receipt = musubi_fault_staging_receipt(
            &submitter,
            genesis_hash,
            latest_time_ms,
            &commitment,
            &manifest,
        );
        let archive_transaction = submitter.build_transaction(
            [InstructionBox::from(RegisterMusubiArchiveV1::new(
                commitment,
                staging_receipt,
                1,
            ))],
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        );
        submit_approved_and_wait_for_all_peers(
            &network,
            &submitter,
            archive_transaction,
            "register unavailable Musubi crash-replay archive",
        )
        .await?;

        let (snapshot, _, _) = musubi_fault_snapshot_and_time(&submitter)?;
        let publication = MusubiPublicationV1 {
            manifest: manifest.clone(),
            resolution: MusubiResolutionProofV1 { snapshot, lock },
        };
        publication
            .validate()
            .expect("Musubi crash-replay publication is structurally valid");
        let release = manifest.release.clone();
        for (index, peer) in peers.iter().enumerate() {
            assert_musubi_publication_absent(
                &peer.client(),
                &release,
                archive_id,
                &format!("pre-crash peer {index}"),
            )?;
        }

        let publish_transaction = submitter.build_transaction(
            [InstructionBox::from(PublishMusubiReleaseV1::new(
                binding.namespace,
                publication,
                None,
                1,
                None,
            ))],
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        );
        let publish_entrypoint = publish_transaction.hash_as_entrypoint();

        // Stop every other validator before Torii acceptance so the publication cannot
        // acquire a consensus QC. The final peer durably queues the exact publication
        // transaction and then crashes; restart must replay that queue entry.
        for peer in peers.iter().take(PEERS - 1) {
            peer.shutdown().await;
        }
        let submitter_for_submit = submitter.clone();
        let publish_for_submit = publish_transaction.clone();
        spawn_blocking(move || submitter_for_submit.submit_transaction(&publish_for_submit))
            .await
            .map_err(|error| eyre!("Musubi fault submit task join error: {error}"))?
            .wrap_err("submit journaled Musubi publication while below consensus quorum")?;
        admitting_peer.shutdown().await;

        // The selectable-archive three-cut matrix below covers the execution
        // phases; this unavailable-archive case remains a queue-journal replay smoke.
        for peer in &peers {
            peer.start_checked(config_layers.iter().cloned(), None)
                .await
                .wrap_err_with(|| {
                    format!("restart Musubi crash-replay peer {}", peer.mnemonic())
                })?;
        }

        let restarted_client =
            admitting_peer.client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
        wait_for_rejected_transaction(
            &restarted_client,
            &publish_transaction,
            "finalized replication quorum",
            "replayed Musubi publication",
        )
        .await?;

        let barrier_transaction = restarted_client.build_transaction(
            [InstructionBox::from(Log::new(
                Level::INFO,
                "Musubi publication crash-replay visibility barrier".to_owned(),
            ))],
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        );
        submit_approved_and_wait_for_all_peers(
            &network,
            &restarted_client,
            barrier_transaction,
            "Musubi crash-replay post-rejection barrier",
        )
        .await?;

        let mut canonical_snapshot: Option<MusubiRegistrySnapshotV1> = None;
        let mut canonical_rejection_block: Option<HashOf<Header>> = None;
        for (index, peer) in peers.iter().enumerate() {
            let client = peer.client();
            let snapshot = assert_musubi_publication_absent(
                &client,
                &release,
                archive_id,
                &format!("post-recovery peer {index}"),
            )?;
            if let Some(expected) = canonical_snapshot.as_ref() {
                ensure!(
                    &snapshot == expected,
                    "post-recovery peer {index} exposed the absence tuple at another snapshot"
                );
            } else {
                canonical_snapshot = Some(snapshot);
            }
            let blocks = client.query(FindBlocks).execute_all()?;
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
                "post-recovery peer {index} recorded the replayed publication {} time(s)",
                occurrences.len()
            );
            let (rejection_block, entrypoint_index) = occurrences[0];
            let rejection = rejection_block.error(entrypoint_index).ok_or_else(|| {
                eyre!("post-recovery peer {index} applied the replayed publication")
            })?;
            ensure!(
                format!("{rejection:?}").contains("finalized replication quorum"),
                "post-recovery peer {index} recorded the wrong rejection: {rejection:?}"
            );
            if let Some(expected) = canonical_rejection_block {
                ensure!(
                    rejection_block.hash() == expected,
                    "post-recovery peer {index} recorded the rejection in a different block"
                );
            } else {
                canonical_rejection_block = Some(rejection_block.hash());
            }
        }

        wait_for_rejected_transaction(
            &restarted_client,
            &publish_transaction,
            "finalized replication quorum",
            "stable replayed Musubi publication status",
        )
        .await?;
        Ok(())
    }
    .await;

    network.shutdown().await;
    result
}

pub(super) async fn run_native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs()
-> Result<()> {
    init_instruction_registry();
    let context =
        stringify!(native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs);
    if !multilane_release_gate_requested(context)? {
        return Ok(());
    }
    eprintln!("[multilane-release-gate] started: {context}");
    let iterations = native_amx_soak_iterations()?;
    let Some(network) = sandbox::start_network_async_or_skip(localnet_builder(), context).await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|layer| ConfigLayer(layer.into_owned()))
            .collect();
        let bootstrap_submitter = network
            .peers()
            .first()
            .ok_or_else(|| eyre!("Native AMX release network has no bootstrap peer"))?
            .client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
        let bootstrap_transaction = native_amx_bootstrap_transaction(&bootstrap_submitter)?;
        let bootstrap_entrypoint = bootstrap_transaction.hash_as_entrypoint();
        submit_and_wait_for_approval(&bootstrap_submitter, bootstrap_transaction.clone()).await?;
        let bootstrap_block = wait_for_block_with_entrypoint(
            &bootstrap_submitter,
            bootstrap_entrypoint,
            "Native AMX dataspace bootstrap",
        )
        .await?;
        let bootstrap_receipt =
            assert_native_amx_execution_context(&bootstrap_block, &bootstrap_transaction)?;
        wait_for_all_peers_to_observe_native_amx_evidence(
            &network,
            &bootstrap_transaction,
            bootstrap_block.hash(),
            &bootstrap_receipt,
            "Native AMX dataspace bootstrap convergence",
        )
        .await?;

        let mut observed_sources = BTreeSet::new();
        let mut pruning_evidence: Option<GroupedNativeAmxEvidence> = None;

        for iteration in 0..iterations {
            let offline_index = iteration % PEERS;
            let submit_index = (offline_index + 1) % PEERS;
            let offline_peer = network
                .peers()
                .get(offline_index)
                .cloned()
                .ok_or_else(|| eyre!("iteration {iteration}: missing offline peer"))?;
            let submit_peer = network
                .peers()
                .get(submit_index)
                .ok_or_else(|| eyre!("iteration {iteration}: missing submit peer"))?;
            let submitter =
                submit_peer.client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
            let transactions = native_amx_soak_transactions(&submitter, iteration)?;

            offline_peer.shutdown().await;

            // Always restart the rotated validator, even if the three-live-peer
            // commit attempt fails. This keeps the failure diagnostic local to
            // the iteration and lets network teardown remain deterministic.
            let outage_result: Result<GroupedNativeAmxEvidence> = async {
                let evidence = submit_grouped_native_amx_transactions(
                    &submitter,
                    transactions,
                    &format!("iteration {iteration}: three-live-validator commit"),
                )
                .await?;
                for receipt in &evidence.receipts {
                    ensure!(
                        observed_sources.insert(receipt.source_id),
                        "iteration {iteration}: a grouped source identity was reused"
                    );
                    let [first, second] = receipt.legs.as_slice() else {
                        return Err(eyre!(
                            "iteration {iteration}: expected exactly two participant legs"
                        ));
                    };
                    ensure!(
                        first.prepare_qc.body != second.prepare_qc.body
                            && first.commit_qc.body != second.commit_qc.body,
                        "iteration {iteration}: participant routes did not retain independent phase-QC bodies"
                    );
                }
                Ok(evidence)
            }
            .await;

            let restart_result = offline_peer
                .start_checked(config_layers.iter().cloned(), None)
                .await
                .wrap_err_with(|| {
                    format!("iteration {iteration}: restart validator {offline_index}")
                });
            restart_result?;
            let evidence = outage_result?;

            let mut canonical_group_relay: Option<LaneRelayEnvelope> = None;
            for (member, (transaction, receipt)) in evidence
                .transactions
                .iter()
                .zip(&evidence.receipts)
                .enumerate()
            {
                let relay = wait_for_all_peers_to_observe_native_amx_evidence(
                    &network,
                    transaction,
                    evidence.block.hash(),
                    receipt,
                    &format!(
                        "iteration {iteration}: grouped member {member} post-restart convergence"
                    ),
                )
                .await?;
                assert_native_amx_relay_tamper_matrix(&relay, receipt)?;
                if let Some(canonical) = canonical_group_relay.as_ref() {
                    ensure!(
                        relay.settlement_commitment == canonical.settlement_commitment,
                        "iteration {iteration}: grouped sources exposed different coordinator settlements"
                    );
                } else {
                    canonical_group_relay = Some(relay);
                }
            }
            let relay_sources = canonical_group_relay
                .as_ref()
                .ok_or_else(|| eyre!("iteration {iteration}: grouped relay was not published"))?
                .settlement_commitment
                .native_amx_receipts
                .iter()
                .map(|receipt| receipt.source_id)
                .collect::<BTreeSet<_>>();
            ensure!(
                relay_sources
                    == evidence
                        .ordered_sources
                        .iter()
                        .copied()
                        .collect::<BTreeSet<_>>(),
                "iteration {iteration}: coordinator relay did not bind the exact grouped source membership"
            );

            for (peer_index, peer) in network.peers().iter().enumerate() {
                let client = peer.client();
                for transaction in &evidence.transactions {
                    ensure_entrypoint_committed_once(
                        &client,
                        transaction.hash_as_entrypoint(),
                        &format!("iteration {iteration}: peer {peer_index}"),
                    )?;
                }
                wait_for_grouped_native_amx_durable_application(
                    &client,
                    &evidence,
                    &format!("iteration {iteration}: peer {peer_index}"),
                )
                .await?;
                let diagnostics = client.get_sumeragi_diagnostics().wrap_err_with(|| {
                    format!("iteration {iteration}: peer {peer_index} diagnostics")
                })?;
                let same_route_rows = diagnostics
                    .native_amx_participant_applications
                    .iter()
                    .filter(|row| {
                        row.application_block_hash == Some(evidence.block.hash())
                            && row.lane_id == LaneId::new(ACME_LANE)
                            && row.dataspace_id == DataSpaceId::new(ACME_DATASPACE)
                    })
                    .count();
                ensure!(
                    same_route_rows == 0,
                    "iteration {iteration}: peer {peer_index} published a forbidden separate same-route coordinator marker"
                );
            }
            pruning_evidence = Some(evidence);
        }

        ensure!(
            observed_sources.len() == iterations.saturating_mul(NATIVE_AMX_GROUP_SIZE),
            "fault soak lost or duplicated Native AMX source identities"
        );

        let pruning_evidence =
            pruning_evidence.ok_or_else(|| eyre!("fault soak produced no grouped evidence"))?;
        let pruning_peer = network
            .peers()
            .first()
            .cloned()
            .ok_or_else(|| eyre!("missing Native AMX pruning peer"))?;
        let pruning_submitter = network
            .peers()
            .get(1)
            .ok_or_else(|| eyre!("missing pruning-tail submit peer"))?
            .client_for(&ALICE_ID, ALICE_KEYPAIR.private_key().clone());
        let (barrier_entrypoint, barrier_block) = advance_past_native_amx_eviction_tail(
            &pruning_submitter,
            pruning_evidence.block.header().height().get(),
            context,
        )
        .await?;
        timeout(
            STATUS_WAIT_TIMEOUT,
            pruning_peer.once_block(barrier_block.header().height().get()),
        )
        .await
        .wrap_err("pruning peer did not durably cross the carrier eviction tail")?;
        let pruning_barrier = wait_for_block_with_entrypoint(
            &pruning_peer.client(),
            barrier_entrypoint,
            "pruning peer exact eviction-tail barrier",
        )
        .await?;
        ensure!(
            pruning_barrier.hash() == barrier_block.hash(),
            "pruning peer observed a different eviction-tail barrier identity"
        );
        pruning_peer.shutdown().await;
        let evidence_artifacts = native_amx_evidence_artifact_snapshot(&pruning_peer)?;
        let receipt_artifacts = native_amx_artifact_snapshot(
            &pruning_peer,
            NativeAmxArtifactSelection::Receipts,
        )?;
        let manifest_artifacts = native_amx_artifact_snapshot(
            &pruning_peer,
            NativeAmxArtifactSelection::Manifests,
        )?;
        let eviction_height = pruning_evidence.block.header().height().get();
        let evicted_payload_len =
            evict_native_amx_carrier_body_offline(&pruning_peer, eviction_height)?;
        ensure!(
            native_amx_evidence_artifact_snapshot(&pruning_peer)? == evidence_artifacts,
            "Native AMX body eviction changed durable receipt/manifest/index evidence"
        );
        remove_latest_native_amx_manifest_offline(&pruning_peer, &pruning_evidence)?;
        ensure!(
            native_amx_artifact_snapshot(
                &pruning_peer,
                NativeAmxArtifactSelection::Receipts,
            )? == receipt_artifacts,
            "Native AMX remote-recovery fixture changed receipt/latest-index evidence"
        );
        ensure!(
            native_amx_artifact_snapshot(
                &pruning_peer,
                NativeAmxArtifactSelection::Manifests,
            )? != manifest_artifacts,
            "Native AMX remote-recovery fixture failed to create an exact manifest gap"
        );
        pruning_peer
            .start_checked(config_layers.iter().cloned(), None)
            .await
            .wrap_err("restart Native AMX peer after authenticated carrier eviction")?;
        ensure!(
            native_amx_block_index_entry(&pruning_peer, eviction_height)?
                == (EVICTED_BLOCK_INDEX_START, evicted_payload_len),
            "Native AMX restart reinserted the evicted carrier body into inline Kura storage"
        );

        let recovered_block = wait_for_block_with_entrypoint(
            &pruning_peer.client(),
            pruning_evidence.transactions[0].hash_as_entrypoint(),
            "post-pruning Native AMX carrier recovery",
        )
        .await?;
        ensure!(
            recovered_block.hash() == pruning_evidence.block.hash(),
            "authenticated recovery returned a different Native AMX carrier identity"
        );
        ensure!(
            native_amx_primary_blocks_dir(&pruning_peer)
                .join("da_blocks")
                .join(format!("{eviction_height:020}.norito"))
                .is_file(),
            "authenticated CommitQC-signer recovery did not restore the local DA body"
        );
        let recovered_evidence =
            assert_grouped_native_amx_execution(&recovered_block, &pruning_evidence.transactions)?;
        ensure!(
            recovered_evidence.receipts == pruning_evidence.receipts
                && recovered_evidence.bank_leg == pruning_evidence.bank_leg
                && recovered_evidence.ordered_sources == pruning_evidence.ordered_sources,
            "authenticated recovery changed the exact Native AMX manifest-backed group evidence"
        );
        ensure!(
            native_amx_evidence_artifact_snapshot(&pruning_peer)? == evidence_artifacts,
            "Native AMX startup recovery changed exact durable manifest/receipt/index artifacts"
        );
        for (peer_index, peer) in network.peers().iter().enumerate() {
            let client = peer.client();
            wait_for_grouped_native_amx_durable_application(
                &client,
                &pruning_evidence,
                &format!("post-pruning peer {peer_index} durable evidence"),
            )
            .await?;
            for transaction in &pruning_evidence.transactions {
                ensure_entrypoint_committed_once(
                    &client,
                    transaction.hash_as_entrypoint(),
                    &format!("post-pruning peer {peer_index} exact-once"),
                )?;
            }
        }
        ensure!(
            native_amx_block_index_entry(&pruning_peer, eviction_height)?
                == (EVICTED_BLOCK_INDEX_START, evicted_payload_len),
            "Native AMX proof recovery repopulated the inline carrier body"
        );
        eprintln!("{NATIVE_AMX_GROUPED_PRUNING_MARKER}");
        Ok(())
    }
    .await;

    network.shutdown().await;
    result?;
    eprintln!("[multilane-release-gate] completed: {context}");
    Ok(())
}
