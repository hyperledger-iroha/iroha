//! VRF message handlers and local emission state.

use std::sync::Arc;

use iroha_logger::prelude::*;

use crate::sumeragi::message::BlockMessageWire;

use super::*;

/// Domain separator for VRF commit input derivation.
const VRF_INPUT_DOMAIN: &[u8] = b"iroha:npos:vrf:input:v1";

pub(super) fn derive_vrf_material_from_key(
    chain_hash: &Hash,
    private_key: &PrivateKey,
    epoch: u64,
    signer: ValidatorIndex,
) -> ([u8; 32], [u8; 32]) {
    let mut msg = Vec::with_capacity(
        VRF_INPUT_DOMAIN.len() + chain_hash.as_ref().len() + core::mem::size_of::<u64>() * 2,
    );
    msg.extend_from_slice(VRF_INPUT_DOMAIN);
    msg.extend_from_slice(chain_hash.as_ref());
    msg.extend_from_slice(&epoch.to_be_bytes());
    msg.extend_from_slice(&u64::from(signer).to_be_bytes());
    let signature = Signature::new(private_key, &msg);
    let reveal: [u8; 32] = Hash::new(signature.payload()).into();
    let commitment: [u8; 32] = Hash::new(reveal).into();
    (reveal, commitment)
}

/// Local VRF emission state for the current epoch.
#[derive(Clone, Debug)]
pub(super) struct VrfLocalState {
    pub(super) epoch: u64,
    pub(super) reveal: [u8; 32],
    pub(super) commitment: [u8; 32],
    pub(super) commit_sent: bool,
    pub(super) reveal_sent: bool,
}

impl VrfLocalState {
    pub(super) fn new(epoch: u64) -> Self {
        Self {
            epoch,
            reveal: [0; 32],
            commitment: [0; 32],
            commit_sent: false,
            reveal_sent: false,
        }
    }

    pub(super) fn ensure_epoch(&mut self, epoch: u64) {
        if self.epoch != epoch {
            self.epoch = epoch;
            self.reveal = [0; 32];
            self.commitment = [0; 32];
            self.commit_sent = false;
            self.reveal_sent = false;
        }
    }

    pub(super) fn note_commit(&mut self, epoch: u64, commitment: [u8; 32]) {
        self.ensure_epoch(epoch);
        self.commitment = commitment;
        self.commit_sent = true;
    }

    pub(super) fn note_reveal(&mut self, epoch: u64, reveal: [u8; 32]) {
        self.ensure_epoch(epoch);
        self.reveal = reveal;
        self.reveal_sent = true;
    }

    #[cfg(test)]
    pub(super) fn commitment(&self) -> [u8; 32] {
        self.commitment
    }

    #[cfg(test)]
    pub(super) fn reveal(&self) -> [u8; 32] {
        self.reveal
    }

    #[cfg(test)]
    pub(super) fn epoch(&self) -> u64 {
        self.epoch
    }

    #[cfg(test)]
    pub(super) fn commit_sent(&self) -> bool {
        self.commit_sent
    }

    #[cfg(test)]
    pub(super) fn reveal_sent(&self) -> bool {
        self.reveal_sent
    }
}

/// VRF handler state scoped to the consensus actor.
#[derive(Debug, Default)]
pub(super) struct VrfActor {
    local: Option<VrfLocalState>,
}

impl VrfActor {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn reset(&mut self) {
        self.local = None;
    }

    #[allow(dead_code)]
    pub(super) fn is_empty(&self) -> bool {
        self.local.is_none()
    }

    pub(super) fn state_mut(
        &mut self,
        consensus_mode: ConsensusMode,
        epoch: u64,
    ) -> Option<&mut VrfLocalState> {
        if !matches!(
            consensus_mode,
            ConsensusMode::Permissioned | ConsensusMode::Npos
        ) {
            return None;
        }
        let state = self.local.get_or_insert_with(|| VrfLocalState::new(epoch));
        state.ensure_epoch(epoch);
        Some(state)
    }

    pub(super) fn note_commit(
        &mut self,
        consensus_mode: ConsensusMode,
        epoch: u64,
        commitment: [u8; 32],
    ) {
        if let Some(state) = self.state_mut(consensus_mode, epoch) {
            state.note_commit(epoch, commitment);
        }
    }

    pub(super) fn note_reveal(
        &mut self,
        consensus_mode: ConsensusMode,
        epoch: u64,
        reveal: [u8; 32],
    ) {
        if let Some(state) = self.state_mut(consensus_mode, epoch) {
            state.note_reveal(epoch, reveal);
        }
    }
}

impl Actor {
    fn derive_vrf_material(&self, epoch: u64, signer: ValidatorIndex) -> ([u8; 32], [u8; 32]) {
        derive_vrf_material_from_key(
            &self.chain_hash,
            self.common_config.key_pair.private_key(),
            epoch,
            signer,
        )
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn maybe_emit_vrf_messages(
        &mut self,
        height: u64,
        roster_len_hint: u32,
        local_signer: ValidatorIndex,
    ) -> Result<()> {
        let (position_opt, commit_end, reveal_end, epoch) = {
            let Some(manager) = self.epoch_manager.as_mut() else {
                return Ok(());
            };
            let position = manager.position_in_epoch(height);
            let commit_end = manager.commit_window_end();
            let reveal_end = manager.reveal_window_end();
            let epoch = manager.epoch_for_height(height);
            (position, commit_end, reveal_end, epoch)
        };
        let Some(position) = position_opt else {
            return Ok(());
        };

        let mut pending_commit: Option<crate::sumeragi::consensus::VrfCommit> = None;

        if position <= commit_end {
            let commit_needed =
                if let Some(state) = self.subsystems.vrf.state_mut(self.consensus_mode, epoch) {
                    !state.commit_sent
                } else {
                    false
                };

            if commit_needed {
                let (reveal, commitment) = self.derive_vrf_material(epoch, local_signer);
                if let Some(state) = self.subsystems.vrf.state_mut(self.consensus_mode, epoch) {
                    if !state.commit_sent {
                        state.reveal = reveal;
                        state.commitment = commitment;
                        state.commit_sent = true;
                        state.reveal_sent = false;
                        pending_commit = Some(crate::sumeragi::consensus::VrfCommit {
                            epoch,
                            commitment,
                            signer: local_signer,
                        });
                    }
                }
            }
        }

        if let Some(commit_msg) = pending_commit {
            let note_result =
                self.process_vrf_commit(height, roster_len_hint, commit_msg, Some(local_signer))?;
            match note_result {
                VrfNoteResult::Accepted | VrfNoteResult::AcceptedLate => {
                    let topology_peers = self.effective_commit_topology();
                    let local_peer_id = self.common_config.peer.id().clone();
                    let msg = Arc::new(BlockMessage::VrfCommit(commit_msg));
                    let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
                    for peer in &topology_peers {
                        if peer == &local_peer_id {
                            continue;
                        }
                        self.schedule_background(BackgroundRequest::Post {
                            peer: peer.clone(),
                            msg: BlockMessageWire::with_encoded(
                                Arc::clone(&msg),
                                Arc::clone(&encoded),
                            ),
                        });
                    }
                }
                other => {
                    if let Some(state) = self.subsystems.vrf.state_mut(self.consensus_mode, epoch) {
                        state.commit_sent = false;
                    }
                    if other == VrfNoteResult::RejectedOutOfWindow {
                        debug!(
                            epoch,
                            height,
                            pos = position,
                            "skipping local VRF commit: outside window"
                        );
                    }
                }
            }
        }

        let mut pending_reveal: Option<crate::sumeragi::consensus::VrfReveal> = None;
        let mut reveal_blocked_missing_commit = false;
        let mut reveal_value = [0u8; 32];
        let mut stored_commitment = [0u8; 32];
        let mut needs_derivation = false;
        let mut should_emit_reveal = false;

        if position > commit_end && position <= reveal_end {
            if let Some(state) = self.subsystems.vrf.state_mut(self.consensus_mode, epoch) {
                if state.reveal_sent {
                    // Reveal already sent for this epoch.
                } else if !state.commit_sent {
                    reveal_blocked_missing_commit = true;
                } else {
                    should_emit_reveal = true;
                    reveal_value = state.reveal;
                    stored_commitment = state.commitment;
                    needs_derivation = state.reveal == [0; 32];
                }
            }
        }

        if reveal_blocked_missing_commit {
            debug!(
                epoch,
                height,
                pos = position,
                "skipping local VRF reveal: commitment not recorded"
            );
        } else if should_emit_reveal {
            if needs_derivation {
                let (derived_reveal, commitment) = self.derive_vrf_material(epoch, local_signer);
                if stored_commitment != [0; 32] && stored_commitment != commitment {
                    warn!(
                        epoch,
                        signer = local_signer,
                        "local VRF commitment mismatch while deriving reveal"
                    );
                }
                if let Some(state) = self.subsystems.vrf.state_mut(self.consensus_mode, epoch) {
                    if state.commit_sent && !state.reveal_sent {
                        state.reveal = derived_reveal;
                        state.commitment = commitment;
                    }
                }
                reveal_value = derived_reveal;
            }

            pending_reveal = Some(crate::sumeragi::consensus::VrfReveal {
                epoch,
                reveal: reveal_value,
                signer: local_signer,
            });
        }

        if let Some(reveal_msg) = pending_reveal {
            let note_result =
                self.process_vrf_reveal(height, roster_len_hint, reveal_msg, Some(local_signer))?;
            match note_result {
                VrfNoteResult::Accepted | VrfNoteResult::AcceptedLate => {
                    if let Some(state) = self.subsystems.vrf.state_mut(self.consensus_mode, epoch) {
                        state.reveal_sent = true;
                    }
                    let topology_peers = self.effective_commit_topology();
                    let local_peer_id = self.common_config.peer.id().clone();
                    let msg = Arc::new(BlockMessage::VrfReveal(reveal_msg));
                    let encoded = Arc::new(BlockMessageWire::encode_message(msg.as_ref()));
                    for peer in &topology_peers {
                        if peer == &local_peer_id {
                            continue;
                        }
                        self.schedule_background(BackgroundRequest::Post {
                            peer: peer.clone(),
                            msg: BlockMessageWire::with_encoded(
                                Arc::clone(&msg),
                                Arc::clone(&encoded),
                            ),
                        });
                    }
                }
                other => {
                    if other == VrfNoteResult::RejectedOutOfWindow {
                        debug!(
                            epoch,
                            height,
                            pos = position,
                            "skipping local VRF reveal: outside window"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    fn process_vrf_commit(
        &mut self,
        height: u64,
        roster_len_hint: u32,
        commit: crate::sumeragi::consensus::VrfCommit,
        local_signer: Option<ValidatorIndex>,
    ) -> Result<VrfNoteResult> {
        let Some(manager) = self.epoch_manager.as_mut() else {
            return Err(eyre!("epoch manager unavailable"));
        };

        let note_result = match manager.try_note_commit_at_height(height, commit) {
            VrfNoteResult::Accepted => {
                #[cfg(feature = "telemetry")]
                self.telemetry.inc_vrf_commit_emitted();
                let snapshot = manager.snapshot_current_epoch(roster_len_hint, height);
                let seed = manager.seed();
                self.persist_vrf_snapshot(snapshot, false, None)?;
                self.refresh_npos_seed(seed, height, super::commit::EpochRefreshPhase::PreCommit);
                VrfNoteResult::Accepted
            }
            VrfNoteResult::AcceptedLate => {
                #[cfg(feature = "telemetry")]
                self.telemetry.inc_vrf_commit_emitted();
                let snapshot = manager.snapshot_current_epoch(roster_len_hint, height);
                self.persist_vrf_snapshot(snapshot, false, None)?;
                VrfNoteResult::AcceptedLate
            }
            VrfNoteResult::RejectedEpochMismatch => {
                warn!(
                    epoch = commit.epoch,
                    "rejected VRF commit: epoch mismatch (local epoch {})",
                    manager.epoch()
                );
                VrfNoteResult::RejectedEpochMismatch
            }
            VrfNoteResult::RejectedOutOfWindow => {
                warn!(
                    height,
                    epoch = commit.epoch,
                    "rejected VRF commit: outside commit window"
                );
                VrfNoteResult::RejectedOutOfWindow
            }
            VrfNoteResult::RejectedUnknownSigner => {
                warn!(
                    height,
                    epoch = commit.epoch,
                    signer = commit.signer,
                    "rejected VRF commit: signer not in active roster"
                );
                VrfNoteResult::RejectedUnknownSigner
            }
            VrfNoteResult::RejectedInvalidReveal => {
                warn!("rejected VRF commit: invalid reveal (commit path)");
                VrfNoteResult::RejectedInvalidReveal
            }
        };

        if matches!(
            note_result,
            VrfNoteResult::Accepted | VrfNoteResult::AcceptedLate
        ) {
            if let Some(local_idx) = local_signer {
                if local_idx == commit.signer {
                    self.subsystems.vrf.note_commit(
                        self.consensus_mode,
                        commit.epoch,
                        commit.commitment,
                    );
                }
            }
        }

        Ok(note_result)
    }

    fn process_vrf_reveal(
        &mut self,
        height: u64,
        roster_len_hint: u32,
        reveal: crate::sumeragi::consensus::VrfReveal,
        local_signer: Option<ValidatorIndex>,
    ) -> Result<VrfNoteResult> {
        let Some(manager) = self.epoch_manager.as_mut() else {
            return Err(eyre!("epoch manager unavailable"));
        };

        let note_result = match manager.try_note_reveal_at_height(height, reveal) {
            VrfNoteResult::Accepted => {
                #[cfg(feature = "telemetry")]
                self.telemetry.inc_vrf_reveal_emitted();
                let snapshot = manager.snapshot_current_epoch(roster_len_hint, height);
                let late_reveals_total = snapshot.late_reveals.len() as u64;
                let seed = manager.seed();
                self.persist_vrf_snapshot(snapshot, false, None)?;
                super::status::set_vrf_late_reveals_total(late_reveals_total);
                self.refresh_npos_seed(seed, height, super::commit::EpochRefreshPhase::PreCommit);
                super::status::set_prf_context(seed, height, 0);
                #[cfg(feature = "telemetry")]
                self.telemetry.set_prf_context(Some(seed), height, 0);
                VrfNoteResult::Accepted
            }
            VrfNoteResult::AcceptedLate => {
                #[cfg(feature = "telemetry")]
                self.telemetry.inc_vrf_reveal_late();
                let snapshot = manager.snapshot_current_epoch(roster_len_hint, height);
                let late_reveals_total = snapshot.late_reveals.len() as u64;
                self.persist_vrf_snapshot(snapshot, false, None)?;
                super::status::set_vrf_late_reveals_total(late_reveals_total);
                VrfNoteResult::AcceptedLate
            }
            VrfNoteResult::RejectedEpochMismatch => {
                warn!(
                    epoch = reveal.epoch,
                    "rejected VRF reveal: epoch mismatch (local epoch {})",
                    manager.epoch()
                );
                VrfNoteResult::RejectedEpochMismatch
            }
            VrfNoteResult::RejectedOutOfWindow => {
                warn!(
                    height,
                    epoch = reveal.epoch,
                    "rejected VRF reveal: outside reveal window"
                );
                VrfNoteResult::RejectedOutOfWindow
            }
            VrfNoteResult::RejectedUnknownSigner => {
                warn!(
                    height,
                    epoch = reveal.epoch,
                    signer = reveal.signer,
                    "rejected VRF reveal: signer not in active roster"
                );
                VrfNoteResult::RejectedUnknownSigner
            }
            VrfNoteResult::RejectedInvalidReveal => {
                warn!(
                    epoch = reveal.epoch,
                    "rejected VRF reveal: commitment mismatch or unknown signer"
                );
                VrfNoteResult::RejectedInvalidReveal
            }
        };

        if matches!(
            note_result,
            VrfNoteResult::Accepted | VrfNoteResult::AcceptedLate
        ) {
            if let Some(local_idx) = local_signer {
                if local_idx == reveal.signer {
                    self.subsystems.vrf.note_reveal(
                        self.consensus_mode,
                        reveal.epoch,
                        reveal.reveal,
                    );
                }
            }
        }

        Ok(note_result)
    }

    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn persist_vrf_snapshot(
        &self,
        snapshot: EpochSnapshot,
        finalized: bool,
        election: Option<ValidatorElectionOutcome>,
    ) -> Result<()> {
        let record = self.snapshot_to_vrf_record(snapshot, finalized, election);
        let mut world = self.state.world.block();
        world.vrf_epochs.insert(record.epoch, record);
        world.commit();
        Ok(())
    }

    pub(super) fn snapshot_to_vrf_record(
        &self,
        snapshot: EpochSnapshot,
        finalized: bool,
        election: Option<ValidatorElectionOutcome>,
    ) -> VrfEpochRecord {
        let EpochSnapshot {
            epoch,
            seed,
            commits,
            reveals,
            late_reveals,
            committed_no_reveal,
            no_participation,
            roster_len,
            updated_at_height,
        } = snapshot;

        let mut participants: BTreeMap<u32, VrfParticipantRecord> = BTreeMap::new();
        for (signer, commitment) in &commits {
            let entry = participants.entry(*signer).or_insert(VrfParticipantRecord {
                signer: *signer,
                commitment: None,
                reveal: None,
                last_updated_height: updated_at_height,
            });
            entry.commitment = Some(*commitment);
            entry.last_updated_height = updated_at_height;
        }
        for (signer, reveal) in &reveals {
            let entry = participants.entry(*signer).or_insert(VrfParticipantRecord {
                signer: *signer,
                commitment: None,
                reveal: None,
                last_updated_height: updated_at_height,
            });
            entry.reveal = Some(*reveal);
            entry.last_updated_height = updated_at_height;
        }
        let late_reveals_records: Vec<VrfLateRevealRecord> = late_reveals
            .into_iter()
            .map(|(signer, reveal, noted_at_height)| VrfLateRevealRecord {
                signer,
                reveal,
                noted_at_height,
            })
            .collect();

        let (penalties_applied, penalties_applied_at_height) = {
            let view = self.state.world.vrf_epochs.view();
            view.get(&epoch).map_or((false, None), |rec| {
                (rec.penalties_applied, rec.penalties_applied_at_height)
            })
        };

        let (epoch_length, commit_deadline_offset, reveal_deadline_offset) =
            self.epoch_manager.as_ref().map_or(
                (
                    self.config.npos.epoch_length_blocks,
                    self.config.npos.vrf.commit_deadline_offset_blocks,
                    self.config.npos.vrf.reveal_deadline_offset_blocks,
                ),
                |manager| {
                    (
                        manager.epoch_length_blocks(),
                        manager.commit_window_end(),
                        manager.reveal_window_end(),
                    )
                },
            );

        VrfEpochRecord {
            epoch,
            seed,
            epoch_length,
            commit_deadline_offset,
            reveal_deadline_offset,
            roster_len,
            finalized,
            updated_at_height,
            participants: participants.into_values().collect(),
            late_reveals: late_reveals_records,
            committed_no_reveal: if finalized {
                committed_no_reveal
            } else {
                Vec::new()
            },
            no_participation: if finalized {
                no_participation
            } else {
                Vec::new()
            },
            penalties_applied,
            penalties_applied_at_height,
            validator_election: election,
        }
    }

    pub(super) fn handle_vrf_commit(
        &mut self,
        commit: crate::sumeragi::consensus::VrfCommit,
    ) -> Result<()> {
        if !matches!(
            self.consensus_mode,
            ConsensusMode::Permissioned | ConsensusMode::Npos
        ) {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::VrfCommit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::ModeMismatch,
            );
            return Ok(());
        }
        let (height, roster_len, roster_indices) = self.current_height_and_roster();
        let local_signer = self.local_validator_index_current();
        if let Some(manager) = self.epoch_manager.as_mut() {
            apply_roster_indices_to_manager(manager, roster_len, roster_indices);
        } else {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::VrfCommit,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::ModeMismatch,
            );
            return Ok(());
        }
        let roster_len_hint = u32::try_from(roster_len).unwrap_or_else(|_| {
            warn!(
                roster_len,
                "validator roster exceeds u32::MAX; snapshot hint clamped to u32::MAX"
            );
            u32::MAX
        });
        let _ = self.process_vrf_commit(height, roster_len_hint, commit, local_signer)?;
        Ok(())
    }

    pub(super) fn handle_vrf_reveal(
        &mut self,
        reveal: crate::sumeragi::consensus::VrfReveal,
    ) -> Result<()> {
        if !matches!(
            self.consensus_mode,
            ConsensusMode::Permissioned | ConsensusMode::Npos
        ) {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::VrfReveal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::ModeMismatch,
            );
            return Ok(());
        }
        let (height, roster_len, roster_indices) = self.current_height_and_roster();
        let local_signer = self.local_validator_index_current();
        if let Some(manager) = self.epoch_manager.as_mut() {
            apply_roster_indices_to_manager(manager, roster_len, roster_indices);
        } else {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::VrfReveal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::ModeMismatch,
            );
            return Ok(());
        }
        let roster_len_hint = u32::try_from(roster_len).unwrap_or_else(|_| {
            warn!(
                roster_len,
                "validator roster exceeds u32::MAX; snapshot hint clamped to u32::MAX"
            );
            u32::MAX
        });
        let _ = self.process_vrf_reveal(height, roster_len_hint, reveal, local_signer)?;
        Ok(())
    }
}
