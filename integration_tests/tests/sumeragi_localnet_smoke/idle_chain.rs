//! Four-validator qualification for bounded progress, clean idle, and proposal work.
use super::*;
use iroha::data_model::{block::execution_output::ExecutionOutputV1, prelude::FindAccountById};

/// Bind the governed fee quote before fixing the transaction's entrypoint identity.
async fn paid_transaction(
    client: &iroha::blocking::Client,
    instruction: InstructionBox,
) -> Result<iroha::data_model::transaction::SignedTransaction> {
    let account = client.account_client();
    let mut payload = account.prepare_transaction(iroha::client::AccountTransactionDraft::new(
        [instruction],
        iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    ))?;
    let quote = account
        .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })
        .await?;
    ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "proposal-work fee quote changed the selected payer or gas bound"
    );
    payload.fee_payment = quote.intent;
    Ok(account.sign_transaction(payload)?)
}

/// Replay this small fixture's canonical blocks through the authorized, bounded stream.
async fn read_peer_blocks(
    peer: &iroha_test_network::NetworkPeer,
) -> Result<Vec<iroha::data_model::block::SignedBlock>> {
    tokio::time::timeout(STATUS_POLL_TIMEOUT, async {
        let height = status_retry::read(|| peer.status()).await?.blocks;
        ensure!(
            (1..=64).contains(&height),
            "idle-chain fixture has an unexpected replay height: {height}"
        );
        let client = peer.client();
        let mut stream = client
            .account_client()
            .blocks()
            .subscribe(nonzero!(1_u64))
            .await?;
        let mut blocks = Vec::<iroha::data_model::block::SignedBlock>::new();
        for expected_height in 1..=height {
            let block = stream
                .try_next()
                .await?
                .ok_or_else(|| eyre!("block replay ended before height {expected_height}"))?;
            ensure!(
                block.header().height().get() == expected_height,
                "block replay returned height {} instead of {expected_height}",
                block.header().height()
            );
            if let Some(previous) = blocks.last() {
                ensure!(
                    block.header().prev_block_hash() == Some(previous.hash()),
                    "block replay changed canonical ancestry at height {expected_height}"
                );
            } else {
                ensure!(
                    block.header().prev_block_hash().is_none(),
                    "genesis replay has a predecessor"
                );
            }
            blocks.push(block);
        }
        stream.close().await?;
        blocks.reverse();
        Ok(blocks)
    })
    .await
    .map_err(|_| {
        eyre!(
            "canonical block replay from {} exceeded {STATUS_POLL_TIMEOUT:?}",
            peer.mnemonic()
        )
    })?
}
#[allow(clippy::too_many_lines)]
pub(super) async fn run_permissioned_progress() -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_block_cadence(SMOKE_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_permissioned_consensus()
        .with_config_layer(|layer| {
            layer.write(["network", "transaction_gossip_period_ms"], 200_i64);
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(permissioned_localnet_produces_blocks_within_bound),
    )
    .await?
    else {
        ensure!(
            !fail_on_sandbox_skip(),
            "sandboxed skip surfaced and {} is enabled",
            FAIL_ON_SANDBOX_SKIP_ENV
        );
        return Ok(());
    };
    let result: Result<()> = async {
        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        let baseline_statuses = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        let baseline_height = baseline_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let warmup_height = baseline_height.saturating_add(1);
        for peer in network.peers() {
            let message = format!("localnet warmup block {}", peer.mnemonic());
            let client = peer.client();
            read_on_dedicated_thread(move || {
                client.submit::<InstructionBox>(
                    Log::new(Level::INFO, message).into(),
                    iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
            })
                .await
                .wrap_err_with(|| {
                    format!("failed to submit warmup log instruction to {}", peer.mnemonic())
                })?;
        }
        wait_for_converged_height(&network, warmup_height, Duration::from_secs(45)).await?;
        let warmup_statuses = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        let baseline_height = warmup_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let baseline_view_changes: Vec<u64> = warmup_statuses
            .iter()
            .map(|status| status.view_changes.into())
            .collect();
        let peer_count = network.peers().len();
        let fault_tolerance = peer_count.saturating_sub(1) / 3;
        let max_extra_view_changes = u64::try_from(fault_tolerance.saturating_add(2))
            .unwrap_or(u64::MAX);
        ensure!(!network.peers().is_empty(), "network must have at least one peer");
        for peer in network.peers() {
            let message = format!("localnet bounded block {}", peer.mnemonic());
            let client = peer.client();
            read_on_dedicated_thread(move || {
                client.submit::<InstructionBox>(
                    Log::new(Level::INFO, message).into(),
                    iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                )
            })
                .await
                .wrap_err_with(|| {
                    format!("failed to submit log instruction to {}", peer.mnemonic())
                })?;
        }
        let target_height = baseline_height.saturating_add(1);
        let start = Instant::now();
        wait_for_converged_height(&network, target_height, Duration::from_secs(45)).await?;
        let elapsed = start.elapsed();
        ensure!(
            elapsed <= Duration::from_secs(15),
            "block production exceeded bound: elapsed={:?}",
            elapsed
        );
        let after_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        ensure!(
            after_statuses
                .iter()
                .all(|status| status.blocks >= target_height),
            "not all peers reached target height {target_height}: {after_statuses:?}"
        );
        for (idx, status) in after_statuses.iter().enumerate() {
            let before = baseline_view_changes.get(idx).copied().unwrap_or_default();
            ensure!(
                u64::from(status.view_changes) <= before.saturating_add(max_extra_view_changes),
                "peer {idx} experienced repeated view changes: before={before}, after={}, max_extra={max_extra_view_changes}",
                status.view_changes,
            );
        }
        let min_view_changes = after_statuses
            .iter()
            .map(|status| u64::from(status.view_changes))
            .min()
            .unwrap_or_default();
        let max_view_changes = after_statuses
            .iter()
            .map(|status| u64::from(status.view_changes))
            .max()
            .unwrap_or_default();
        ensure!(
            max_view_changes.saturating_sub(min_view_changes) <= max_extra_view_changes,
            "view_change counters diverged across peers: {after_statuses:?}"
        );
        network.shutdown().await;
        Ok(())
    }
    .await;
    if sandbox::handle_result(
        result,
        stringify!(permissioned_localnet_produces_blocks_within_bound),
    )?
    .is_none()
    {
        return Ok(());
    }
    Ok(())
}
#[allow(clippy::too_many_lines)]
pub(super) async fn run() -> Result<()> {
    const TIP_POLL: Duration = Duration::from_millis(250);
    const PROGRESS_TIMEOUT: Duration = Duration::from_secs(60);
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_block_cadence(SMOKE_PIPELINE_TIME)
        .with_permissioned_consensus()
        .with_config_layer(|layer| {
            layer.write(["network", "transaction_gossip_period_ms"], 200_i64);
        });
    let context = stringify!(permissioned_idle_chain_advances_only_for_external_or_internal_work);
    let Some(network) = sandbox::start_network_async_or_skip(builder, context).await? else {
        ensure!(
            !fail_on_sandbox_skip(),
            "sandboxed skip surfaced and {} is enabled",
            FAIL_ON_SANDBOX_SKIP_ENV
        );
        return Ok(());
    };
    let result: Result<()> = async {
        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        wait_for_converged_height(&network, 1, PROGRESS_TIMEOUT).await?;
        let query_tips = || async {
                try_join_all(network.peers().iter().map(|peer| async move {
                        let blocks = read_peer_blocks(peer).await
                            .wrap_err_with(|| format!("replay blocks from {}", peer.mnemonic()))?;
                        let latest = blocks
                            .first()
                            .ok_or_else(|| eyre!("peer {} returned an empty chain", peer.mnemonic()))?;
                        Ok::<_, eyre::Report>((latest.header().height().get(), latest.hash()))
                    })).await
        };
        let block_sync_gossip_period = network.block_sync_gossip_period();
        let retransmit_observation = block_sync_gossip_period.saturating_mul(2);
        let commit_quorum_observation = network.da_commit_quorum_timeout().saturating_mul(2);
        let idle_observation = SMOKE_PIPELINE_TIME
            .saturating_mul(10)
            .max(retransmit_observation)
            .max(commit_quorum_observation);
        let mut baseline_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        ensure!(
            baseline_statuses.len() == 4,
            "idle-chain gate requires four live status responses"
        );
        let mut baseline_height = baseline_statuses[0].blocks;
        let mut baseline_non_empty = baseline_statuses[0].blocks_non_empty;
        ensure!(
            baseline_statuses.iter().all(|status| {
                status.blocks == baseline_height
                    && status.blocks_non_empty == baseline_non_empty
                    && status.queue_size == 0
            }),
            "validators did not begin the idle observation at one empty-queue height: {baseline_statuses:?}"
        );
        let mut baseline_tips = query_tips().await?;
        let mut canonical_baseline_tip = baseline_tips
            .first()
            .copied()
            .ok_or_else(|| eyre!("idle-chain gate started without validators"))?;
        ensure!(
            canonical_baseline_tip.0 == baseline_height
                && baseline_tips
                    .iter()
                    .all(|tip| tip == &canonical_baseline_tip),
            "validators did not begin the idle observation at one canonical tip: statuses={baseline_statuses:?}, tips={baseline_tips:?}"
        );
        // Startup may legitimately finish delayed genesis-adjacent internal
        // work. Require one complete commit-quorum window at a canonical,
        // empty-queue tip before beginning the qualification window. Any tip
        // movement during settling must itself be attributable to non-empty
        // work; a delayed empty successor is already a gate failure.
        let baseline_settle_window = network.da_commit_quorum_timeout();
        let baseline_settle_deadline = Instant::now() + PROGRESS_TIMEOUT;
        let mut baseline_stable_since = Instant::now();
        loop {
            sleep(TIP_POLL).await;
            let statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
            let tips = query_tips().await?;
            let canonical_tip = tips
                .first()
                .copied()
                .ok_or_else(|| eyre!("idle-chain settling lost all validators"))?;
            ensure!(
                statuses.len() == 4
                    && statuses.iter().all(|status| {
                        status.blocks == canonical_tip.0 && status.queue_size == 0
                    })
                    && tips.iter().all(|tip| tip == &canonical_tip),
                "idle-chain baseline did not expose one settled canonical tip: statuses={statuses:?}, tips={tips:?}"
            );
            let observed_non_empty = statuses[0].blocks_non_empty;
            ensure!(
                statuses
                    .iter()
                    .all(|status| status.blocks_non_empty == observed_non_empty),
                "idle-chain baseline non-empty counters diverged: {statuses:?}"
            );
            if canonical_tip != canonical_baseline_tip
                || observed_non_empty != baseline_non_empty
            {
                let blocks = read_peer_blocks(network.peer()).await
                .wrap_err("query delayed baseline work")?;
                let delayed = blocks
                    .iter()
                    .filter(|block| block.header().height().get() > baseline_height)
                    .collect::<Vec<_>>();
                ensure!(
                    !delayed.is_empty()
                        && delayed.iter().all(|block| !block.is_empty())
                        && observed_non_empty
                            == baseline_non_empty
                                .saturating_add(u64::try_from(delayed.len()).unwrap_or(u64::MAX)),
                    "idle-chain baseline was contaminated by unattributed or empty delayed work: previous_height={baseline_height}, previous_non_empty={baseline_non_empty}, observed={statuses:?}, delayed={delayed:?}"
                );
                baseline_statuses = statuses;
                baseline_height = canonical_tip.0;
                baseline_non_empty = observed_non_empty;
                baseline_tips = tips;
                canonical_baseline_tip = canonical_tip;
                baseline_stable_since = Instant::now();
            }
            if baseline_stable_since.elapsed() >= baseline_settle_window {
                break;
            }
            ensure!(
                Instant::now() < baseline_settle_deadline,
                "idle-chain baseline never settled for {baseline_settle_window:?} within {PROGRESS_TIMEOUT:?}"
            );
        }
        // Two complete DA commit-quorum windows cover ten signed cadences as
        // well as two explicit retransmission intervals. A clean height must
        // remain at one exact tip throughout that entire protocol observation
        // interval.
        let idle_deadline = Instant::now() + idle_observation;
        while Instant::now() < idle_deadline {
            sleep(TIP_POLL).await;
            let statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
            ensure!(
                statuses.iter().all(|status| {
                    status.blocks == baseline_height
                        && status.blocks_non_empty == baseline_non_empty
                        && status.queue_size == 0
                }),
                "clean idle height manufactured a block within {idle_observation:?}: baseline={baseline_statuses:?}, observed={statuses:?}"
            );
            let tips = query_tips().await?;
            ensure!(
                tips == baseline_tips,
                "clean idle height changed canonical tip within {idle_observation:?}: baseline={baseline_tips:?}, observed={tips:?}"
            );
        }
        let client = network.client();
        let external_transaction = paid_transaction(&client, InstructionBox::from(Log::new(
                Level::INFO,
                "proposal-work external transaction".to_owned(),
            ))).await?;
        let external_entrypoint = external_transaction.hash_as_entrypoint();
        let submission_client = client.clone();
        read_on_dedicated_thread(move || submission_client.submit_transaction(&external_transaction))
            .await
            .wrap_err("submit explicit proposal-work transaction")?;
        let external_height = baseline_height.saturating_add(1);
        wait_for_converged_height(&network, external_height, PROGRESS_TIMEOUT).await?;
        let mut external_block_hash = None;
        for (index, peer) in network.peers().iter().enumerate() {
            let blocks = read_peer_blocks(peer).await?;
            let matches = blocks
                .iter()
                .filter(|block| {
                    block
                        .network_input_hashes()
                        .any(|hash| hash == external_entrypoint)
                })
                .collect::<Vec<_>>();
            ensure!(
                matches.len() == 1,
                "peer {index} recorded the explicit proposal work {} time(s)",
                matches.len()
            );
            let block = matches[0];
            ensure!(
                block.header().height().get() == external_height
                    && !block.is_empty()
                    && block.external_entrypoint_count() == 1,
                "peer {index} did not commit the explicit transaction in one non-empty successor: {block:?}"
            );
            if let Some(expected) = external_block_hash {
                ensure!(
                    block.hash() == expected,
                    "peer {index} committed the explicit transaction in another block"
                );
            } else {
                external_block_hash = Some(block.hash());
            }
        }
        let trigger_id: iroha::data_model::trigger::TriggerId =
            "proposal_work_scheduled_once".parse()?;
        let marker_key: Name = "proposal_work_scheduled_marker".parse()?;
        let marker_value = Json::new("executed");
        // Sumeragi §4.5 authorizes native carriers only for opted-in committed
        // schedules. PreCommit callbacks cannot wake an idle chain. A periodic
        // schedule avoids losing the due interval while registration commits;
        // both the success count and autonomous attempt allowance are exactly one.
        let schedule = iroha::data_model::events::time::Schedule::starting_at(
            SystemTime::now().duration_since(UNIX_EPOCH)?,
        )
        .with_period(SMOKE_PIPELINE_TIME);
        let mut metadata = Metadata::default();
        metadata.insert("native_progress_attempts".parse()?, Json::new(1_u32));
        let trigger = Trigger::new(
            trigger_id.clone(),
            Action::new(
                vec![InstructionBox::from(SetKeyValue::account(
                    ALICE_ID.clone(),
                    marker_key.clone(),
                    marker_value.clone(),
                ))],
                Repeats::Exactly(1),
                ALICE_ID.clone(),
                TimeEventFilter::new(ExecutionTime::Schedule(schedule)),
            )
            .expect("single-attempt scheduled trigger is a valid action")
            .with_metadata(metadata),
        );
        let registration_transaction = paid_transaction(&client,
            InstructionBox::from(Register::trigger(trigger))).await?;
        let registration_entrypoint = registration_transaction.hash_as_entrypoint();
        let submission_client = client.clone();
        read_on_dedicated_thread(move || submission_client.submit_transaction(&registration_transaction))
            .await
            .wrap_err("register single-attempt proposal-work scheduled trigger")?;
        let registration_height = external_height.saturating_add(1);
        let internal_height = registration_height.saturating_add(1);
        wait_for_converged_height(&network, internal_height, PROGRESS_TIMEOUT).await?;
        let mut registration_block_hash = None;
        let mut internal_block_hash = None;
        for (index, peer) in network.peers().iter().enumerate() {
            let blocks = read_peer_blocks(peer).await?;
            let registration_matches = blocks
                .iter()
                .filter(|block| {
                    block
                        .network_input_hashes()
                        .any(|hash| hash == registration_entrypoint)
                })
                .collect::<Vec<_>>();
            ensure!(
                registration_matches.len() == 1,
                "peer {index} recorded trigger registration {} time(s)",
                registration_matches.len()
            );
            let registration_block = registration_matches[0];
            ensure!(
                registration_block.header().height().get() == registration_height
                    && !registration_block.is_empty()
                    && registration_block.external_entrypoint_count() == 1,
                "peer {index} did not commit trigger registration in one non-empty successor"
            );
            if let Some(expected) = registration_block_hash {
                ensure!(
                    registration_block.hash() == expected,
                    "peer {index} committed trigger registration in another block"
                );
            } else {
                registration_block_hash = Some(registration_block.hash());
            }
            let trigger_blocks = blocks
                .iter()
                .filter(|block| block.execution_outputs().iter().any(|output| matches!(output, ExecutionOutputV1::Time(row) if row.invocation.trigger.trigger_id == trigger_id)))
                .collect::<Vec<_>>();
            ensure!(
                trigger_blocks.len() == 1,
                "peer {index} recorded the single-attempt scheduled trigger in {} blocks",
                trigger_blocks.len()
            );
            let trigger_block = trigger_blocks[0];
            ensure!(
                trigger_block.header().height().get() == internal_height
                    && !trigger_block.is_empty()
                    && trigger_block.external_entrypoint_count() == 1
                    && trigger_block.execution_outputs().iter().filter(|output| matches!(output, ExecutionOutputV1::Time(_))).count() == 1,
                "peer {index} did not commit one native scheduled trigger carrier: {trigger_block:?}"
            );
            let [iroha::data_model::transaction::TransactionEntrypoint::External(carrier)] =
                trigger_block.external_entrypoints_slice()
            else {
                bail!("peer {index} scheduled carrier is not one signed transaction");
            };
            ensure!(
                iroha_core::smartcontracts::isi::sccp::keepalive::valid_shape(carrier.payload()),
                "peer {index} scheduled work lacks its standalone signed Keepalive"
            );
            carrier.verify_signature()?;
            let keeper = carrier
                .payload()
                .authority
                .try_signatory()
                .ok_or_else(|| eyre!("native carrier has no single keeper signatory"))?
                .to_string();
            // A validator's native duty has separate Ed25519 custody. Bind the
            // carrier to the public key reported by an actual node at startup,
            // without reading or exporting its private keeper-key file.
            let mut keeper_belongs_to_node = false;
            for validator in network.peers() {
                let log_path = validator
                    .latest_stdout_log_path()
                    .ok_or_else(|| eyre!("validator has no startup log"))?;
                let log = tokio::fs::read_to_string(log_path).await?;
                keeper_belongs_to_node |= log.lines().any(|line| {
                    line.contains("SCCP keeper key loaded") && line.contains(&keeper)
                });
            }
            ensure!(
                keeper_belongs_to_node,
                "peer {index} scheduled carrier was not signed by a native node keeper"
            );
            if let Some(expected) = internal_block_hash {
                ensure!(
                    trigger_block.hash() == expected,
                    "peer {index} committed another internal trigger carrier"
                );
            } else {
                internal_block_hash = Some(trigger_block.hash());
            }
        }
        let query_client = client.clone();
        let alice = read_on_dedicated_thread(move || {
            Ok(query_client.client().query_single(FindAccountById::new(ALICE_ID.clone()))?)
        })
        .await?;
        ensure!(
            alice.metadata().get(&marker_key) == Some(&marker_value),
            "single-attempt scheduled carrier did not apply its queryable state effect"
        );
        let post_work_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let expected_non_empty = baseline_non_empty.saturating_add(3);
        ensure!(
            post_work_statuses.iter().all(|status| {
                status.blocks == internal_height
                    && status.blocks_non_empty == expected_non_empty
                    && status.queue_size == 0
            }),
            "external, registration, and native scheduled carriers did not converge exactly: {post_work_statuses:?}"
        );
        let post_work_tips = query_tips().await?;
        let expected_internal_hash = internal_block_hash
            .ok_or_else(|| eyre!("four-peer gate observed no internal carrier hash"))?;
        ensure!(
            post_work_tips.iter().all(|(height, hash)| {
                *height == internal_height && *hash == expected_internal_hash
            }),
            "validators did not converge on the internal-only carrier: {post_work_tips:?}"
        );
        let post_work_deadline = Instant::now() + idle_observation;
        while Instant::now() < post_work_deadline {
            sleep(TIP_POLL).await;
            let statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
            ensure!(
                statuses.iter().all(|status| {
                    status.blocks == internal_height
                        && status.blocks_non_empty == expected_non_empty
                        && status.queue_size == 0
                }),
                "one-shot internal work did not return to a clean idle height: expected={post_work_statuses:?}, observed={statuses:?}"
            );
            let tips = query_tips().await?;
            ensure!(
                tips == post_work_tips,
                "canonical tip advanced again after one-shot internal work: expected={post_work_tips:?}, observed={tips:?}"
            );
        }
        eprintln!(
            "EX-297 idle-chain evidence: cadence={SMOKE_PIPELINE_TIME:?}, block_sync_gossip_period={block_sync_gossip_period:?}, retransmit_window={retransmit_observation:?}, commit_quorum_window={commit_quorum_observation:?}, baseline_settle_window={baseline_settle_window:?}, idle_window={idle_observation:?}, baseline_height={baseline_height}, external_height={external_height}, trigger_registration_height={registration_height}, internal_trigger_height={internal_height}, final_tip_hash={expected_internal_hash}"
        );
        Ok(())
    }
    .await;
    network.shutdown().await;
    if sandbox::handle_result(result, context)?.is_none() {
        return Ok(());
    }
    Ok(())
}
