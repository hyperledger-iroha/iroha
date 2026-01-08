//! Runtime consensus-mode flip helpers.

use iroha_logger::prelude::*;

use super::*;

impl Actor {
    fn rebuild_npos_state(&mut self) -> Result<Option<[u8; 32]>> {
        let view = self.state.view();
        let mut collectors = super::load_npos_collector_config(&view).or_else(|| {
            Some(NposCollectorConfig {
                seed: super::latest_epoch_seed(&view),
                k: self.config.npos.k_aggregators,
                redundant_send_r: self.config.npos.redundant_send_r,
            })
        });
        let epoch_params = super::load_npos_epoch_params(&view, &self.config);
        let height = view.height() as u64;
        let epoch_seed_for_height = super::npos_seed_for_height(&view, height);
        let schedule = super::EpochScheduleSnapshot::from_world_with_fallback(
            view.world(),
            epoch_params.epoch_length_blocks,
        );
        let target_epoch = schedule.epoch_for_height(height);
        let record_for_target_epoch = view.world().vrf_epochs().get(&target_epoch).cloned();
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
        drop(view);
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

    pub(super) fn apply_mode_flip(&mut self, target: ConsensusMode) -> Result<()> {
        if target == self.consensus_mode {
            self.pending_mode_flip = None;
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
        self.subsystems.da_rbc.rbc.pending.clear();
        self.subsystems.da_rbc.rbc.sessions.clear();
        self.subsystems.propose.proposal_cache = ProposalCache::new(PROPOSAL_CACHE_LIMIT);
        self.subsystems.propose.proposals_seen.clear();
        self.qc_cache.clear();
        self.vote_log.clear();
        let (_, redundant_r) = self.collector_plan_params();
        self.subsystems.propose.collector_redundant_limit = redundant_r.max(1);
        self.pending.pending_replay_last_sent.clear();
        self.pending.missing_block_requests.clear();
        self.subsystems.da_rbc.da.da_bundles.clear();
        self.subsystems.da_rbc.da.da_pin_bundles.clear();
        self.subsystems.da_rbc.da.sealed_commitments.clear();
        self.subsystems.da_rbc.da.sealed_pin_intents.clear();
        self.payload_rebroadcast_log.clear();
        self.block_sync_rebroadcast_log.clear();
        self.block_sync_fetch_log.clear();
        let now = Instant::now();
        let block_time = {
            let view = self.state.view();
            view.world.parameters().sumeragi().block_time()
        };
        let base_pacemaker_interval = pacemaker_base_interval(block_time, &self.config);
        reset_runtime_state_for_mode_flip(
            &mut self.subsystems.propose.pacemaker,
            &mut self.subsystems.propose.new_view_tracker,
            &mut self.phase_tracker,
            &mut self.subsystems.propose.propose_attempt_monitor,
            &mut self.subsystems.propose.pacemaker_backpressure,
            &mut self.subsystems.propose.forced_view_after_timeout,
            &mut self.subsystems.propose.last_pacemaker_attempt,
            &mut self.subsystems.propose.last_successful_proposal,
            &mut self.tick_counter,
            &mut self.qc_signer_tally,
            &mut self.voting_block,
            &mut self.pending_roster_activation,
            &mut self.subsystems.propose.last_empty_child_attempt,
            &self.subsystems.da_rbc.rbc.status_handle,
            &mut self.subsystems.vrf,
            base_pacemaker_interval,
            now,
        );
    }

    fn apply_mode_specific_state(&mut self, target: ConsensusMode) -> Result<()> {
        match target {
            ConsensusMode::Permissioned => {
                self.npos_collectors = None;
                self.epoch_manager = None;
                let height = {
                    let view = self.state.view();
                    view.height() as u64
                };
                super::status::set_prf_context([0; 32], height, 0);
                #[cfg(feature = "telemetry")]
                self.telemetry.set_prf_context(None, height, 0);
            }
            ConsensusMode::Npos => {
                let seed = self.rebuild_npos_state()?;
                if let Some(seed) = seed {
                    let height = {
                        let view = self.state.view();
                        view.height() as u64
                    };
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
            let view = self.state.view();
            let params = view.world.parameters().sumeragi();
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
        let (_mode_tag, _bls_domain, consensus_caps) =
            super::consensus::compute_consensus_handshake_caps_from_view(
                &self.state.view(),
                &self.common_config,
                &self.config,
                &config_caps,
            );
        self.network.update_consensus_caps(consensus_caps, true);
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
