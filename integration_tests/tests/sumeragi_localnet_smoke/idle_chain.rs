//! Four-validator qualification for bounded progress, clean idle, and proposal work.
use super::*;
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
            layer
                .write(["network", "transaction_gossip_period_ms"], 200_i64)
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    2_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    1_i64,
                );
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
            peer.client()
                .submit::<InstructionBox>(Log::new(Level::INFO, message).into(), iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None))
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
            peer.client()
                .submit::<InstructionBox>(Log::new(Level::INFO, message).into(), iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None))
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
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    const TIP_POLL: Duration = Duration::from_millis(250);
    const PROGRESS_TIMEOUT: Duration = Duration::from_secs(60);
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_block_cadence(SMOKE_PIPELINE_TIME)
        .with_permissioned_consensus()
        .with_config_layer(|layer| {
            layer
                .write(["network", "transaction_gossip_period_ms"], 200_i64)
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    2_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    1_i64,
                );
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
        let query_tips = || -> Result<Vec<(u64, HashOf<Header>)>> {
            network
                .peers()
                .iter()
                .map(|peer| {
                    let blocks = peer
                        .client()
                        .query(FindBlocks)
                        .execute_all()
                        .wrap_err_with(|| format!("query blocks from {}", peer.mnemonic()))?;
                    let latest = blocks
                        .first()
                        .ok_or_else(|| eyre!("peer {} returned an empty chain", peer.mnemonic()))?;
                    Ok((latest.header().height().get(), latest.hash()))
                })
                .collect()
        };
        let cadence_ms = u64::try_from(SMOKE_PIPELINE_TIME.as_millis())
            .wrap_err("idle-chain cadence overflows canonical millisecond width")?;
        let (_, retransmit_interval_ms) =
            iroha_config::parameters::actual::sumeragi_v2_timing_ms(cadence_ms)
                .wrap_err("derive idle-chain Sumeragi v2 timing")?;
        let retransmit_observation =
            Duration::from_millis(retransmit_interval_ms).saturating_mul(2);
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
        let mut baseline_tips = query_tips()?;
        let mut canonical_baseline_tip = baseline_tips
            .first()
            .cloned()
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
            let tips = query_tips()?;
            let canonical_tip = tips
                .first()
                .cloned()
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
                let blocks = network
                    .client()
                    .query(FindBlocks)
                    .execute_all()
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
            let tips = query_tips()?;
            ensure!(
                tips == baseline_tips,
                "clean idle height changed canonical tip within {idle_observation:?}: baseline={baseline_tips:?}, observed={tips:?}"
            );
        }
        let client = network.client();
        let external_transaction = client.build_transaction(
            [InstructionBox::from(Log::new(
                Level::INFO,
                "proposal-work external transaction".to_owned(),
            ))],
            iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        );
        let external_entrypoint = external_transaction.hash_as_entrypoint();
        client
            .submit_transaction(&external_transaction)
            .wrap_err("submit explicit proposal-work transaction")?;
        let external_height = baseline_height.saturating_add(1);
        wait_for_converged_height(&network, external_height, PROGRESS_TIMEOUT).await?;
        let mut external_block_hash = None;
        for (index, peer) in network.peers().iter().enumerate() {
            let blocks = peer.client().query(FindBlocks).execute_all()?;
            let matches = blocks
                .iter()
                .filter(|block| {
                    block
                        .entrypoint_hashes()
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
            "proposal_work_precommit_once".parse()?;
        let marker_key: Name = "proposal_work_precommit_marker".parse()?;
        let marker_value = Json::new("executed");
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
                TimeEventFilter::new(ExecutionTime::PreCommit),
            )
            .expect("one-shot PreCommit trigger is a valid action"),
        );
        let registration_transaction = client.build_transaction(
            [InstructionBox::from(Register::trigger(trigger))],
            iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        );
        let registration_entrypoint = registration_transaction.hash_as_entrypoint();
        client
            .submit_transaction(&registration_transaction)
            .wrap_err("register one-shot proposal-work PreCommit trigger")?;
        let registration_height = external_height.saturating_add(1);
        let internal_height = registration_height.saturating_add(1);
        wait_for_converged_height(&network, internal_height, PROGRESS_TIMEOUT).await?;
        let mut registration_block_hash = None;
        let mut internal_block_hash = None;
        for (index, peer) in network.peers().iter().enumerate() {
            let blocks = peer.client().query(FindBlocks).execute_all()?;
            let registration_matches = blocks
                .iter()
                .filter(|block| {
                    block
                        .entrypoint_hashes()
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
                .filter(|block| block.time_triggers().any(|entry| entry.id == trigger_id))
                .collect::<Vec<_>>();
            ensure!(
                trigger_blocks.len() == 1,
                "peer {index} recorded the one-shot PreCommit trigger in {} blocks",
                trigger_blocks.len()
            );
            let trigger_block = trigger_blocks[0];
            ensure!(
                trigger_block.header().height().get() == internal_height
                    && !trigger_block.is_empty()
                    && trigger_block.external_entrypoint_count() == 0
                    && trigger_block.time_triggers().len() == 1,
                "peer {index} did not commit one internal-only trigger carrier: {trigger_block:?}"
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
        let alice = client.query_single(FindAccountById::new(ALICE_ID.clone()))?;
        ensure!(
            alice.metadata().get(&marker_key) == Some(&marker_value),
            "one-shot PreCommit carrier did not apply its queryable state effect"
        );
        let post_work_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let expected_non_empty = baseline_non_empty.saturating_add(3);
        ensure!(
            post_work_statuses.iter().all(|status| {
                status.blocks == internal_height
                    && status.blocks_non_empty == expected_non_empty
                    && status.queue_size == 0
            }),
            "external, registration, and internal-only carriers did not converge exactly: {post_work_statuses:?}"
        );
        let post_work_tips = query_tips()?;
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
            let tips = query_tips()?;
            ensure!(
                tips == post_work_tips,
                "canonical tip advanced again after one-shot internal work: expected={post_work_tips:?}, observed={tips:?}"
            );
        }
        eprintln!(
            "EX-297 idle-chain evidence: cadence={SMOKE_PIPELINE_TIME:?}, retransmit_interval_ms={retransmit_interval_ms}, retransmit_window={retransmit_observation:?}, commit_quorum_window={commit_quorum_observation:?}, baseline_settle_window={baseline_settle_window:?}, idle_window={idle_observation:?}, baseline_height={baseline_height}, external_height={external_height}, trigger_registration_height={registration_height}, internal_trigger_height={internal_height}, final_tip_hash={expected_internal_hash}"
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
