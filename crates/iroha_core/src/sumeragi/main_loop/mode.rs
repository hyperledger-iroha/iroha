//! Runtime consensus-mode flip helpers.

use std::time::SystemTime;

use iroha_config::parameters::actual::SumeragiNposTimeouts;
use iroha_logger::prelude::*;

use super::*;

impl Actor {
    #[allow(clippy::unnecessary_wraps)]
    fn rebuild_npos_state(&mut self) -> Result<Option<[u8; 32]>> {
        let height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let chain_id = self.state.chain_id_ref().clone();
        let world = self.state.world_view();
        let mut collectors = super::load_npos_collector_config_from_world(&world, &chain_id)
            .or_else(|| {
                Some(NposCollectorConfig {
                    seed: super::npos_seed_for_height_from_world(&world, &chain_id, height),
                    k: self.config.collectors.k,
                    redundant_send_r: self.config.collectors.redundant_send_r,
                })
            });
        let epoch_params = super::load_npos_epoch_params_from_world(&world, &self.config.npos);
        let epoch_seed_for_height =
            super::npos_seed_for_height_from_world(&world, &chain_id, height);
        let schedule = super::EpochScheduleSnapshot::from_world_with_fallback(
            &world,
            epoch_params.epoch_length_blocks,
        );
        let target_epoch = schedule.epoch_for_height(height);
        let record_for_target_epoch = world.vrf_epochs().get(&target_epoch).cloned();
        let mut manager = EpochManager::new_from_chain(&self.common_config.chain);
        manager.set_params(
            epoch_params.epoch_length_blocks,
            epoch_params.commit_deadline_offset,
            epoch_params.reveal_deadline_offset,
        );
        if let Some(record) = record_for_target_epoch.as_ref() {
            manager.restore_from_record(record);
        } else {
            manager.set_epoch_seed(epoch_seed_for_height);
            manager.set_epoch(target_epoch);
        }
        let roster_len = self.effective_commit_topology().len();
        let indices = compute_roster_indices_from_topology(
            &self.effective_commit_topology(),
            self.epoch_roster_provider.as_ref(),
        );
        apply_roster_indices_to_manager(&mut manager, roster_len, indices);
        let seed = manager.seed();
        let epoch_length_blocks = manager.epoch_length_blocks();
        let commit_deadline_offset = manager.commit_window_end();
        let reveal_deadline_offset = manager.reveal_window_end();
        if let Some(cfg) = collectors.as_mut() {
            cfg.seed = seed;
        }
        self.epoch_manager = Some(manager);
        self.npos_collectors = collectors;
        super::status::set_epoch_parameters(
            epoch_length_blocks,
            commit_deadline_offset,
            reveal_deadline_offset,
        );
        #[cfg(feature = "telemetry")]
        self.telemetry.set_epoch_parameters(
            epoch_length_blocks,
            commit_deadline_offset,
            reveal_deadline_offset,
        );
        Ok(Some(seed))
    }

    #[allow(clippy::unnecessary_wraps)]
    fn rebuild_permissioned_prf_state(&mut self) -> Result<Option<[u8; 32]>> {
        let height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let chain_id = self.state.chain_id_ref().clone();
        let world = self.state.world_view();
        let epoch_params = super::load_npos_epoch_params_from_world(&world, &self.config.npos);
        let schedule = super::EpochScheduleSnapshot::from_world_with_fallback(
            &world,
            epoch_params.epoch_length_blocks,
        );
        let target_epoch = schedule.epoch_for_height(height);
        let record_for_target_epoch = world.vrf_epochs().get(&target_epoch).cloned();
        let mut manager = EpochManager::new_from_chain(&self.common_config.chain);
        manager.set_params(
            epoch_params.epoch_length_blocks,
            epoch_params.commit_deadline_offset,
            epoch_params.reveal_deadline_offset,
        );
        if let Some(record) = record_for_target_epoch.as_ref() {
            manager.restore_from_record(record);
        } else {
            let seed = super::prf_seed_for_height_from_world(&world, &chain_id, height);
            manager.set_epoch_seed(seed);
            manager.set_epoch(target_epoch);
        }
        let roster_len = self.effective_commit_topology().len();
        let indices = compute_roster_indices_from_topology(
            &self.effective_commit_topology(),
            self.epoch_roster_provider.as_ref(),
        );
        apply_roster_indices_to_manager(&mut manager, roster_len, indices);
        let seed = manager.seed();
        self.epoch_manager = Some(manager);
        self.npos_collectors = None;
        super::status::set_epoch_parameters(
            epoch_params.epoch_length_blocks,
            epoch_params.commit_deadline_offset,
            epoch_params.reveal_deadline_offset,
        );
        #[cfg(feature = "telemetry")]
        self.telemetry.set_epoch_parameters(
            epoch_params.epoch_length_blocks,
            epoch_params.commit_deadline_offset,
            epoch_params.reveal_deadline_offset,
        );
        Ok(Some(seed))
    }

    pub(super) fn apply_mode_flip(&mut self, target: ConsensusMode) -> Result<()> {
        if target == self.consensus_mode {
            self.pending_mode_flip = None;
            return Ok(());
        }
        let processing_hash = self.pending.pending_processing.get();
        let inflight = self.subsystems.commit.inflight.as_ref();
        if processing_hash.is_some() || inflight.is_some() {
            let reason = match (processing_hash.is_some(), inflight.is_some()) {
                (true, true) => "commit_pipeline_busy",
                (true, false) => "pending_processing",
                (false, true) => "commit_inflight",
                (false, false) => "commit_pipeline_idle",
            };
            let now_ms = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
                .unwrap_or(0);
            super::status::note_mode_flip_blocked(reason, now_ms);
            #[cfg(feature = "telemetry")]
            self.telemetry
                .inc_mode_flip_blocked(self.mode_tag(), now_ms);
            debug!(
                reason,
                processing = ?processing_hash,
                inflight_hash = ?inflight.as_ref().map(|entry| entry.block_hash),
                "deferring runtime consensus mode flip while commit pipeline is active"
            );
            return Ok(());
        }
        info!(
            from = ?self.consensus_mode,
            to = ?target,
            height = self.last_committed_height,
            "applying runtime consensus mode flip"
        );
        self.reset_mode_flip_state();
        self.apply_mode_specific_state(target)?;
        self.consensus_mode = target;
        self.finalize_mode_flip_status();
        self.pending_mode_flip = None;
        self.record_mode_flip_success();
        Ok(())
    }

    /// Clear cached consensus state when switching consensus mode.
    pub(crate) fn reset_mode_flip_state(&mut self) {
        self.subsystems.propose.collector_plan = None;
        self.subsystems.propose.collector_plan_subject = None;
        self.subsystems.propose.collector_plan_targets.clear();
        self.subsystems.propose.collectors_contacted.clear();
        self.subsystems.propose.collector_role_index = None;
        self.pending.pending_blocks.clear();
        self.subsystems.validation.inflight.clear();
        self.subsystems.validation.superseded_results.clear();
        self.pending.pending_fetch_requests.clear();
        self.subsystems.da_rbc.rbc.pending.clear();
        self.subsystems.da_rbc.rbc.sessions.clear();
        self.subsystems.da_rbc.rbc.deliver_deferral.clear();
        self.subsystems.da_rbc.rbc.outbound_chunks.clear();
        self.subsystems.da_rbc.rbc.outbound_cursor = None;
        self.subsystems.propose.proposal_cache =
            ProposalCache::new(self.recovery_pending_proposal_cap());
        self.subsystems.propose.proposals_seen.clear();
        self.qc_cache.clear();
        self.vote_log.clear();
        self.vote_validation_cache.clear();
        self.deferred_votes.clear();
        self.consensus_recovery.clear();
        self.deferred_qcs.clear();
        self.deferred_qc_roster_state.clear();
        self.deferred_missing_payload_qcs.clear();
        self.quarantined_block_sync_qcs.clear();
        self.vote_roster_cache.clear();
        let now = Instant::now();
        let (effective_mode, pacemaker_block_time, pacemaker_timeouts) = {
            let height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
            let world = self.state.world_view();
            let effective_mode = super::effective_consensus_mode_for_height_from_world(
                &world,
                height,
                self.config.consensus_mode,
            );
            let block_time = super::resolve_npos_block_time_from_world(&world);
            let timeouts = if matches!(effective_mode, ConsensusMode::Npos) {
                super::resolve_npos_timeouts_from_world(&world, &self.config.npos)
            } else {
                SumeragiNposTimeouts::from_block_time(block_time)
            };
            (effective_mode, block_time, timeouts)
        };
        let (_, redundant_r) = self.collector_plan_params_for_mode(effective_mode);
        self.subsystems.propose.collector_redundant_limit = redundant_r.max(1);
        self.no_roster_fallback_recovery.clear();
        self.pending.missing_block_requests.clear();
        self.subsystems.da_rbc.da.da_bundles.clear();
        self.subsystems.da_rbc.da.da_pin_bundles.clear();
        self.subsystems.da_rbc.da.sealed_commitments.clear();
        self.subsystems.da_rbc.da.sealed_pin_intents.clear();
        self.new_view_rebroadcast_log.clear();
        self.proposal_rebroadcast_log.clear();
        self.payload_rebroadcast_log.clear();
        self.block_sync_rebroadcast_log.clear();
        self.block_sync_fetch_log.clear();
        self.block_sync_warning_log.clear();
        self.qc_insufficient_warning_log.clear();
        self.tick_lag_last_progress_at = now;
        self.tick_lag_last_progress_height = self.state.committed_height();
        self.tick_lag_last_progress_queue_len = self.queue.active_len();
        self.tick_lag_last_progress_pending_blocks = self.pending.pending_blocks.len();
        self.tick_lag_warn_streak = 0;
        self.tick_lag_last_warn = None;
        self.hotspot_log_summary.reset(now);
        let base_pacemaker_interval = pacemaker_base_interval_with_propose_timeout(
            pacemaker_block_time,
            pacemaker_timeouts.propose,
            &self.config,
        );
        self.phase_ema = PhaseEma::new(&pacemaker_timeouts);
        reset_runtime_state_for_mode_flip(
            &mut self.subsystems.propose.pacemaker,
            &mut self.subsystems.propose.new_view_tracker,
            &mut self.phase_tracker,
            &mut self.subsystems.propose.propose_attempt_monitor,
            &mut self.subsystems.propose.pacemaker_backpressure,
            &mut self.subsystems.propose.pacemaker_backpressure_tracker,
            &mut self.subsystems.propose.forced_view_after_timeout,
            &mut self.subsystems.propose.last_missing_qc_timeout_trigger,
            &mut self.subsystems.propose.last_missing_qc_reacquire_attempt,
            &mut self.subsystems.propose.proposal_liveness,
            &mut self.subsystems.propose.last_pacemaker_attempt,
            &mut self.subsystems.propose.last_successful_proposal,
            &mut self.tick_counter,
            &mut self.qc_signer_tally,
            &mut self.voting_block,
            &mut self.pending_roster_activation,
            &self.subsystems.da_rbc.rbc.status_handle,
            &mut self.subsystems.vrf,
            base_pacemaker_interval,
            now,
        );
        self.seed_phase_ema_metrics();
        let world = self.state.world_view();
        self.update_effective_timing_status_from_world(&world, effective_mode);
    }

    fn apply_mode_specific_state(&mut self, target: ConsensusMode) -> Result<()> {
        match target {
            ConsensusMode::Permissioned => {
                let seed = self.rebuild_permissioned_prf_state()?;
                let height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
                if let Some(seed) = seed {
                    super::status::set_prf_context(seed, height, 0);
                    #[cfg(feature = "telemetry")]
                    self.telemetry.set_prf_context(Some(seed), height, 0);
                }
            }
            ConsensusMode::Npos => {
                let seed = self.rebuild_npos_state()?;
                if let Some(seed) = seed {
                    let height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
                    super::status::set_prf_context(seed, height, 0);
                    #[cfg(feature = "telemetry")]
                    self.telemetry.set_prf_context(Some(seed), height, 0);
                }
            }
        }
        Ok(())
    }

    fn finalize_mode_flip_status(&mut self) {
        self.highest_qc = None;
        self.locked_qc = None;
        super::status::set_highest_qc(0, 0);
        super::status::set_highest_qc_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
            [0; Hash::LENGTH],
        )));
        super::status::set_locked_qc(0, 0, None);
        let (staged_mode_tag, staged_mode_activation_height) = {
            let world = self.state.world_view();
            let params = world.parameters().sumeragi();
            super::staged_mode_info(params)
        };
        super::status::set_mode_tags(
            self.mode_tag(),
            staged_mode_tag,
            staged_mode_activation_height,
        );
        super::status::set_mode_activation_lag(None);
        #[cfg(feature = "telemetry")]
        self.telemetry.set_mode_tags(
            self.mode_tag(),
            staged_mode_tag,
            staged_mode_activation_height,
        );
        #[cfg(feature = "telemetry")]
        self.telemetry.set_mode_activation_lag(None);
        let config_caps = self.recompute_consensus_caps();
        let height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let world = self.state.world_view();
        let (_mode_tag, _bls_domain, consensus_caps) =
            super::consensus::compute_consensus_handshake_caps_from_world(
                &world,
                height,
                &self.common_config,
                &self.config,
                &config_caps,
            );
        // Do not forcibly disconnect peers during an on-chain cutover. Nodes may reach the
        // activation height at slightly different times; keeping existing connections alive
        // prevents a lagging peer from being locked out before it can apply the same flip and
        // catch up via block sync.
        self.network.update_consensus_caps(consensus_caps, false);
    }

    fn record_mode_flip_success(&self) {
        let now_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        super::status::note_mode_flip_success(now_ms);
        #[cfg(feature = "telemetry")]
        self.telemetry
            .inc_mode_flip_success(self.mode_tag(), now_ms);
    }
}
