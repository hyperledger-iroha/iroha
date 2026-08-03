// Native AMX recovery and publication tests included at integration-crate scope.
fn native_amx_soak_iterations() -> Result<usize> {
    let raw = std::env::var(NATIVE_AMX_SOAK_ITERATIONS_ENV)
        .unwrap_or_else(|_| NATIVE_AMX_SOAK_ITERATIONS_DEFAULT.to_string());
    let iterations = raw.parse::<usize>().wrap_err_with(|| {
        format!("{NATIVE_AMX_SOAK_ITERATIONS_ENV} must be an integer in 1..={NATIVE_AMX_SOAK_ITERATIONS_MAX}")
    })?;
    ensure!(
        (1..=NATIVE_AMX_SOAK_ITERATIONS_MAX).contains(&iterations),
        "{NATIVE_AMX_SOAK_ITERATIONS_ENV} must be in 1..={NATIVE_AMX_SOAK_ITERATIONS_MAX}, got {iterations}"
    );
    Ok(iterations)
}

fn native_amx_bootstrap_transaction(submitter: &Client) -> Result<SignedTransaction> {
    let acme_dataspace = DataSpaceId::new(ACME_DATASPACE);
    let bank_dataspace = DataSpaceId::new(BANK_DATASPACE);
    let instructions = vec![
        dataspace_setup_instruction("acme", acme_dataspace, &submitter.account)?,
        dataspace_setup_instruction("bank", bank_dataspace, &submitter.account)?,
        domain_setup_instruction_in_dataspace(
            &DomainId::try_new("soakbootstrapmerchant", "acme")?,
            acme_dataspace,
            &submitter.account,
        )?,
        domain_setup_instruction_in_dataspace(
            &DomainId::try_new("soakbootstrapvault", "bank")?,
            bank_dataspace,
            &submitter.account,
        )?,
    ];
    Ok(submitter.build_transaction(
        instructions,
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    ))
}

fn native_amx_soak_transactions(
    submitter: &Client,
    iteration: usize,
) -> Result<Vec<SignedTransaction>> {
    let acme_dataspace = DataSpaceId::new(ACME_DATASPACE);
    let bank_dataspace = DataSpaceId::new(BANK_DATASPACE);
    let mut transactions = (0..NATIVE_AMX_GROUP_SIZE)
        .map(|member| {
            let merchant_domain =
                DomainId::try_new(format!("soakmerchant{iteration:03}{member}"), "acme")
                    .wrap_err("construct grouped soak merchant domain")?;
            let treasury_domain =
                DomainId::try_new(format!("soakbankvault{iteration:03}{member}"), "bank")
                    .wrap_err("construct grouped soak bank domain")?;
            let instructions = vec![
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
            ];
            Ok(submitter.build_transaction(
                instructions,
                FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    transactions.sort_by_key(native_amx_source_id);
    Ok(transactions)
}

async fn submit_grouped_native_amx_transactions(
    submitter: &Client,
    transactions: Vec<SignedTransaction>,
    context: &str,
) -> Result<GroupedNativeAmxEvidence> {
    ensure!(
        transactions.len() == NATIVE_AMX_GROUP_SIZE,
        "{context}: expected exactly {NATIVE_AMX_GROUP_SIZE} grouped transactions"
    );
    let payloads = transactions
        .iter()
        .map(Client::prepare_transaction_payload)
        .collect::<Vec<_>>();
    submitter
        .submit_prepared_transaction_payload_batch_async(&payloads)
        .await
        .wrap_err_with(|| format!("{context}: submit exact two-source Torii batch"))?;

    let first_entrypoint = transactions[0].hash_as_entrypoint();
    let block = wait_for_block_with_entrypoint(submitter, first_entrypoint, context).await?;
    for transaction in &transactions {
        ensure!(
            block
                .entrypoint_hashes()
                .any(|hash| hash == transaction.hash_as_entrypoint()),
            "{context}: Torii accepted the two-source batch but the sources landed in separate canonical blocks"
        );
    }
    assert_grouped_native_amx_execution(&block, &transactions)
        .wrap_err_with(|| format!("{context}: validate grouped Native AMX carrier evidence"))
}

async fn advance_past_native_amx_eviction_tail(
    submitter: &Client,
    target_height: u64,
    context: &str,
) -> Result<(HashOf<TransactionEntrypoint>, SignedBlock)> {
    let mut last_height = target_height;
    let mut final_barrier = None;
    for offset in 0..3 {
        let transaction = submitter.build_transaction(
            [InstructionBox::from(Log::new(
                Level::INFO,
                format!("{context}: post-carrier eviction-tail barrier {offset}"),
            ))],
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        );
        let entrypoint_hash = transaction.hash_as_entrypoint();
        submit_and_wait_for_approval(submitter, transaction).await?;
        let block = wait_for_block_with_entrypoint(
            submitter,
            entrypoint_hash,
            &format!("{context}: eviction-tail barrier {offset}"),
        )
        .await?;
        last_height = block.header().height().get();
        final_barrier = Some((entrypoint_hash, block));
    }
    ensure!(
        last_height > target_height.saturating_add(2),
        "{context}: carrier height {target_height} remained inside the two-block Kura eviction tail at height {last_height}"
    );
    final_barrier.ok_or_else(|| eyre!("{context}: no eviction-tail barrier was committed"))
}

fn offline_kura_config(store_dir: std::path::PathBuf) -> KuraConfig {
    KuraConfig {
        init_mode: InitMode::Strict,
        store_dir: WithOrigin::inline(store_dir),
        max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: NonZeroUsize::new(2).expect("two is non-zero"),
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: FsyncMode::Batched,
        fsync_interval: defaults::kura::FSYNC_INTERVAL,
        block_sync_roster_retention: defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
        roster_sidecar_retention: defaults::kura::ROSTER_SIDECAR_RETENTION,
        replica_advert: defaults::kura::REPLICA_ADVERT_POLICY,
    }
}

fn decode_block_index_entry(bytes: &[u8], height: u64) -> Result<(u64, u64)> {
    ensure!(height > 0, "block index height must be positive");
    let index = usize::try_from(height.saturating_sub(1))?;
    let start = index
        .checked_mul(BLOCK_INDEX_ENTRY_BYTES)
        .ok_or_else(|| eyre!("block index byte offset overflow"))?;
    let end = start
        .checked_add(BLOCK_INDEX_ENTRY_BYTES)
        .ok_or_else(|| eyre!("block index byte range overflow"))?;
    let entry = bytes
        .get(start..end)
        .ok_or_else(|| eyre!("block index omits height {height}"))?;
    let offset = u64::from_le_bytes(entry[..8].try_into().expect("index offset is eight bytes"));
    let length = u64::from_le_bytes(entry[8..].try_into().expect("index length is eight bytes"));
    Ok((offset, length))
}

fn native_amx_primary_blocks_dir(peer: &NetworkPeer) -> std::path::PathBuf {
    ActualLaneConfig::from_catalog(&native_amx_lane_catalog())
        .primary()
        .blocks_dir(peer.kura_store_dir())
}

fn native_amx_block_index_entry(peer: &NetworkPeer, height: u64) -> Result<(u64, u64)> {
    decode_block_index_entry(
        &fs::read(native_amx_primary_blocks_dir(peer).join("blocks.index"))?,
        height,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeAmxArtifactSelection {
    All,
    Receipts,
    Manifests,
}

fn canonical_native_amx_height_artifact(name: &str) -> Option<(NativeAmxArtifactSelection, u64)> {
    for (prefix, selection) in [
        (
            NATIVE_AMX_MANIFEST_FILE_PREFIX,
            NativeAmxArtifactSelection::Manifests,
        ),
        (
            NATIVE_AMX_RECEIPT_FILE_PREFIX,
            NativeAmxArtifactSelection::Receipts,
        ),
    ] {
        let Some(height) = name
            .strip_prefix(prefix)
            .and_then(|height| height.strip_suffix(NATIVE_AMX_EVIDENCE_FILE_SUFFIX))
        else {
            continue;
        };
        if height.len() != 20 || !height.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let height = height.parse::<u64>().ok()?;
        if height == 0 || format!("{prefix}{height:020}{NATIVE_AMX_EVIDENCE_FILE_SUFFIX}") != name {
            return None;
        }
        return Some((selection, height));
    }
    None
}

fn native_amx_artifact_snapshot(
    peer: &NetworkPeer,
    selection: NativeAmxArtifactSelection,
) -> Result<Vec<(String, Hash)>> {
    let lane_config = ActualLaneConfig::from_catalog(&native_amx_lane_catalog());
    let bank_entry = lane_config
        .entry(LaneId::new(BANK_LANE))
        .ok_or_else(|| eyre!("Native AMX lane catalog omitted BANK storage"))?;
    let artifact_dir = bank_entry
        .blocks_dir(peer.kura_store_dir())
        .join("lane_artifacts");
    let mut snapshot = Vec::new();
    for entry in fs::read_dir(&artifact_dir)
        .wrap_err_with(|| format!("scan Native AMX evidence {}", artifact_dir.display()))?
    {
        let entry = entry.wrap_err_with(|| {
            format!("read Native AMX evidence entry {}", artifact_dir.display())
        })?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| eyre!("Native AMX evidence file name is not UTF-8"))?;
        let artifact_selection = if name == NATIVE_AMX_LATEST_POINTER_FILE {
            Some(NativeAmxArtifactSelection::Receipts)
        } else {
            canonical_native_amx_height_artifact(&name).map(|(selection, _)| selection)
        };
        let Some(artifact_selection) = artifact_selection else {
            ensure!(
                !name.starts_with("native_amx_"),
                "unexpected, temporary, or legacy Native AMX evidence file: {}",
                artifact_dir.join(&name).display()
            );
            continue;
        };
        if !matches!(selection, NativeAmxArtifactSelection::All) && selection != artifact_selection
        {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .wrap_err_with(|| format!("inspect Native AMX evidence {}", path.display()))?;
        ensure!(
            metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
            "Native AMX evidence must be a regular non-symlink file: {}",
            path.display()
        );
        let bytes = fs::read(&path)
            .wrap_err_with(|| format!("read Native AMX evidence {}", path.display()))?;
        ensure!(
            !bytes.is_empty(),
            "Native AMX evidence file is empty: {}",
            path.display()
        );
        snapshot.push((name, Hash::new(&bytes)));
    }
    snapshot.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    Ok(snapshot)
}

fn native_amx_evidence_artifact_snapshot(peer: &NetworkPeer) -> Result<Vec<(String, Hash)>> {
    let snapshot = native_amx_artifact_snapshot(peer, NativeAmxArtifactSelection::All)?;
    ensure!(
        snapshot
            .iter()
            .any(|(name, _)| name.starts_with(NATIVE_AMX_MANIFEST_FILE_PREFIX)),
        "Native AMX evidence snapshot omitted standalone manifests"
    );
    ensure!(
        snapshot
            .iter()
            .any(|(name, _)| name.starts_with(NATIVE_AMX_RECEIPT_FILE_PREFIX)),
        "Native AMX evidence snapshot omitted standalone receipts"
    );
    ensure!(
        snapshot
            .iter()
            .any(|(name, _)| name == NATIVE_AMX_LATEST_POINTER_FILE),
        "Native AMX evidence snapshot omitted the latest pointer"
    );
    Ok(snapshot)
}

fn evict_native_amx_carrier_body_offline(peer: &NetworkPeer, height: u64) -> Result<u64> {
    let catalog = native_amx_lane_catalog();
    let lane_config = ActualLaneConfig::from_catalog(&catalog);
    let config = offline_kura_config(peer.kura_store_dir());
    let (kura, block_count) =
        Kura::new_with_configured_lane_catalog(&config, &lane_config, &catalog)?;
    ensure!(
        u64::try_from(block_count.0)?.saturating_sub(2) > height,
        "Native AMX carrier height {height} is still inside the two-block eviction tail at durable height {}",
        block_count.0
    );
    let height =
        NonZeroUsize::new(usize::try_from(height)?).ok_or_else(|| eyre!("zero carrier height"))?;
    let payload_len = kura
        .advertise_required_replicas_for_bench(height)
        .ok_or_else(|| eyre!("Native AMX carrier has no inline body to evict"))?;
    let freed = kura.evict_block_bodies_for_bench(payload_len)?;
    ensure!(
        freed >= payload_len,
        "Native AMX carrier eviction freed {freed} bytes, below selected body length {payload_len}"
    );
    kura.remove_evicted_block_sidecar_for_testing(height)?;
    drop(kura);

    let height_u64 = u64::try_from(height.get())?;
    let (offset, retained_len) = native_amx_block_index_entry(peer, height_u64)?;
    ensure!(
        offset == EVICTED_BLOCK_INDEX_START && retained_len == payload_len,
        "Native AMX carrier index was not durably marked evicted: offset={offset}, length={retained_len}, expected={payload_len}"
    );
    ensure!(
        !native_amx_primary_blocks_dir(peer)
            .join("da_blocks")
            .join(format!("{height_u64:020}.norito"))
            .exists(),
        "Native AMX remote-recovery fixture retained a local DA body"
    );
    Ok(payload_len)
}

fn remove_latest_native_amx_manifest_offline(
    peer: &NetworkPeer,
    evidence: &GroupedNativeAmxEvidence,
) -> Result<()> {
    let catalog = native_amx_lane_catalog();
    let lane_config = ActualLaneConfig::from_catalog(&catalog);
    let config = offline_kura_config(peer.kura_store_dir());
    let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &catalog)?;
    let descriptor = &evidence.bank_leg.participant_proposal.descriptor;
    kura.remove_latest_native_amx_participant_manifest_for_testing(
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
        descriptor.lane_block_height,
        evidence.block.hash(),
    )?;
    drop(kura);
    Ok(())
}

fn ensure_entrypoint_committed_once(
    client: &Client,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    context: &str,
) -> Result<()> {
    let occurrences = client
        .query(FindBlocks)
        .execute_all()
        .wrap_err_with(|| format!("{context}: query canonical blocks"))?
        .iter()
        .map(|block| {
            block
                .entrypoint_hashes()
                .filter(|hash| *hash == entrypoint_hash)
                .count()
        })
        .sum::<usize>();
    ensure!(
        occurrences == 1,
        "{context}: expected one canonical application for {entrypoint_hash}, observed {occurrences}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mixed_dataspace_native_amx_routes_and_commits_with_receipts() -> Result<()> {
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn native_amx_queue_journal_replays_plan_after_restart() -> Result<()> {
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn musubi_publication_below_quorum_queue_crash_replay_keeps_projection_tuple_absent()
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
            [InstructionBox::from(RegisterProviderOwner::new(
                musubi_fault_provider(),
                submitter.account.clone(),
            ))],
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
                    block
                        .entrypoint_hashes()
                        .enumerate()
                        .filter_map(|(entrypoint_index, hash)| {
                            (hash == publish_entrypoint).then_some((block, entrypoint_index))
                        })
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
            // can assemble and publish the executable payload. Progress is
            // impossible until that exact author restarts; requiring a live
            // commit here would turn these cuts into deterministic timeouts.
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
            if let Some(expected) = canonical_publication_block {
                ensure!(
                    publication_block.hash() == expected,
                    "{context}: post-replay peer {index} stored another publication block"
                );
            } else {
                canonical_publication_block = Some(publication_block.hash());
            }
        }
        Ok(())
    }
    .await;

    network.shutdown().await;
    result?;
    Ok(true)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn musubi_selectable_publication_phase_cut_matrix_is_atomic_after_replay() -> Result<()> {
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
        if !run_selectable_musubi_publication_phase_cut(phase, label).await? {
            return Ok(());
        }
    }
    Ok(())
}
