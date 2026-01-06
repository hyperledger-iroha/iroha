//! Epoch manager for `NPoS` Sumeragi.
//!
//! Maintains deterministic per-epoch PRF seed `S_e`, fixed at epoch start and
//! derived at epoch boundaries from the prior epoch seed plus the reveals
//! observed during that epoch. The seed stays stable within an epoch so leader
//! and collector selection is deterministic across peers, while the manager
//! still tracks commit/reveal participation for penalties.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::blake2::{Blake2b512, Digest as _};
use iroha_data_model::{ChainId, consensus::VrfEpochRecord};

use crate::sumeragi::consensus::{VrfCommit, VrfReveal};

#[derive(Debug, Clone)]
struct LateRevealEntry {
    reveal: [u8; 32],
    noted_at_height: u64,
}

/// Epoch manager holding the current epoch index and seed `S_e`.
#[derive(Debug, Clone)]
pub struct EpochManager {
    epoch: u64,
    seed: [u8; 32],
    /// Epoch length in blocks
    epoch_length_blocks: u64,
    /// Commit window deadline offset (blocks since epoch start)
    commit_deadline_offset: u64,
    /// Reveal window deadline offset (blocks since epoch start)
    reveal_deadline_offset: u64,
    /// Accumulated reveals for the current epoch keyed by signer index
    reveals: BTreeMap<u32, [u8; 32]>,
    /// Accumulated commits for the current epoch keyed by signer index
    commits: BTreeMap<u32, [u8; 32]>,
    /// Late reveals accepted after the reveal window (do not mutate seed).
    late_reveals: BTreeMap<u32, LateRevealEntry>,
    /// Validator roster snapshot (set of `ValidatorIndex`) for the current epoch (optional)
    validator_roster: Option<std::collections::BTreeSet<u32>>,
    /// Last computed penalties (epoch, list of signer indices) to be consumed by caller
    last_penalties: Option<(u64, Vec<u32>)>,
    /// Detailed penalties: `(epoch, committed_without_reveal, neither_committed_nor_revealed, roster_len)`
    last_penalties_detailed: Option<(u64, Vec<u32>, Vec<u32>, u32)>,
    /// Snapshot of the most recently finalized epoch (captured before clearing state).
    last_epoch_snapshot: Option<EpochSnapshot>,
}

/// Snapshot of VRF participation state at a particular point within or at the end of an epoch.
#[derive(Debug, Clone)]
pub(crate) struct EpochSnapshot {
    pub epoch: u64,
    pub seed: [u8; 32],
    pub commits: Vec<(u32, [u8; 32])>,
    pub reveals: Vec<(u32, [u8; 32])>,
    pub late_reveals: Vec<(u32, [u8; 32], u64)>,
    pub committed_no_reveal: Vec<u32>,
    pub no_participation: Vec<u32>,
    pub roster_len: u32,
    pub updated_at_height: u64,
}

impl EpochManager {
    /// Construct an `EpochManager` with a seed derived from `chain_id`.
    pub fn new_from_chain(chain_id: &ChainId) -> Self {
        let hash = iroha_crypto::Hash::new(chain_id.clone().into_inner().as_bytes());
        let seed: [u8; 32] = hash.into();
        Self {
            epoch: 0,
            seed,
            epoch_length_blocks: 3600,
            commit_deadline_offset: 100,
            reveal_deadline_offset: 140,
            reveals: BTreeMap::new(),
            commits: BTreeMap::new(),
            late_reveals: BTreeMap::new(),
            validator_roster: None,
            last_penalties: None,
            last_penalties_detailed: None,
            last_epoch_snapshot: None,
        }
    }

    /// Current epoch index.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Set the current epoch index explicitly.
    pub(crate) fn set_epoch(&mut self, epoch: u64) {
        self.epoch = epoch;
    }

    /// Reset epoch state to the supplied index and seed, clearing accumulated VRF inputs.
    pub(crate) fn reset_epoch_state(&mut self, epoch: u64, seed: [u8; 32]) {
        self.epoch = epoch;
        self.seed = seed;
        self.reveals.clear();
        self.clear_commits();
        self.late_reveals.clear();
        self.validator_roster = None;
        self.last_penalties = None;
        self.last_penalties_detailed = None;
        self.last_epoch_snapshot = None;
    }

    /// Current per-epoch PRF seed `S_e`, fixed at epoch start.
    /// Reveals are mixed into the next epoch seed at rollover.
    pub fn seed(&self) -> [u8; 32] {
        self.seed
    }

    /// Override the per-epoch seed used for leader/collector selection.
    pub fn set_epoch_seed(&mut self, seed: [u8; 32]) {
        self.seed = seed;
    }

    /// Advance to the next epoch by evolving the seed deterministically from collected reveals.
    pub fn next_epoch(&mut self) {
        let new_seed = self.current_entropy();
        self.seed = new_seed;
        self.epoch = self.epoch.saturating_add(1);
        self.reveals.clear();
        self.clear_commits();
        self.late_reveals.clear();
        self.validator_roster = None;
    }

    /// Configure epoch parameters (length and window offsets).
    pub fn set_params(&mut self, epoch_len: u64, commit_off: u64, reveal_off: u64) {
        self.epoch_length_blocks = epoch_len.max(1);
        self.commit_deadline_offset = commit_off.min(self.epoch_length_blocks);
        self.reveal_deadline_offset = reveal_off.min(self.epoch_length_blocks);
    }

    /// Return the 1-based position of `height` within the current epoch, if defined.
    pub fn position_in_epoch(&self, height: u64) -> Option<u64> {
        if self.epoch_length_blocks == 0 || height == 0 {
            None
        } else {
            Some(self.pos_in_epoch(height))
        }
    }

    /// Epoch length in blocks.
    pub fn epoch_length_blocks(&self) -> u64 {
        self.epoch_length_blocks
    }

    /// Inclusive end (1-based position) of the commit window for the current epoch.
    pub fn commit_window_end(&self) -> u64 {
        self.commit_deadline_offset.min(self.epoch_length_blocks)
    }

    /// Inclusive end (1-based position) of the reveal window for the current epoch.
    pub fn reveal_window_end(&self) -> u64 {
        self.reveal_deadline_offset.min(self.epoch_length_blocks)
    }

    /// Check whether a position (1-based) falls inside the commit window.
    pub fn is_commit_window_position(&self, pos: u64) -> bool {
        self.is_in_commit_window(pos)
    }

    /// Check whether a position (1-based) falls inside the reveal window.
    pub fn is_reveal_window_position(&self, pos: u64) -> bool {
        self.is_in_reveal_window(pos)
    }

    /// Note a VRF commit (skeleton; currently unused for validation). Returns whether accepted.
    pub fn try_note_commit_at_height(&mut self, height: u64, c: VrfCommit) -> VrfNoteResult {
        let Some(pos) = self.position_in_epoch(height) else {
            return VrfNoteResult::RejectedOutOfWindow;
        };
        if c.epoch != self.epoch {
            return VrfNoteResult::RejectedEpochMismatch;
        }
        if let Some(roster) = self.validator_roster.as_ref() {
            if !roster.contains(&c.signer) {
                return VrfNoteResult::RejectedUnknownSigner;
            }
        }
        if !self.is_in_commit_window(pos) {
            return VrfNoteResult::RejectedOutOfWindow;
        }
        // Record commitment (by signer)
        self.commits_mut().insert(c.signer, c.commitment);
        VrfNoteResult::Accepted
    }

    /// Note a VRF reveal for the current epoch. Returns whether accepted and reason if rejected.
    pub fn try_note_reveal_at_height(&mut self, height: u64, r: VrfReveal) -> VrfNoteResult {
        let Some(pos) = self.position_in_epoch(height) else {
            return VrfNoteResult::RejectedOutOfWindow;
        };
        if r.epoch != self.epoch {
            return VrfNoteResult::RejectedEpochMismatch;
        }
        if let Some(roster) = self.validator_roster.as_ref() {
            if !roster.contains(&r.signer) {
                return VrfNoteResult::RejectedUnknownSigner;
            }
        }
        if !self.is_in_reveal_window(pos) {
            if self.is_in_commit_window(pos) {
                return VrfNoteResult::RejectedOutOfWindow;
            }
            let Some(commit) = self.commits().get(&r.signer).copied() else {
                return VrfNoteResult::RejectedOutOfWindow;
            };
            let c: [u8; 32] = iroha_crypto::Hash::new(r.reveal).into();
            if c != commit {
                return VrfNoteResult::RejectedInvalidReveal;
            }
            self.late_reveals.insert(
                r.signer,
                LateRevealEntry {
                    reveal: r.reveal,
                    noted_at_height: height,
                },
            );
            return VrfNoteResult::AcceptedLate;
        }
        // Verify against prior commitment if any: blake2b32(reveal) == commitment
        if let Some(commit) = self.commits().get(&r.signer).copied() {
            let c: [u8; 32] = iroha_crypto::Hash::new(r.reveal).into();
            if c != commit {
                return VrfNoteResult::RejectedInvalidReveal;
            }
        }
        self.reveals.insert(r.signer, r.reveal);
        self.late_reveals.remove(&r.signer);
        VrfNoteResult::Accepted
    }

    /// Called on each block commit; advances epoch and computes new seed at epoch boundaries.
    pub fn on_block_commit(&mut self, height: u64) {
        if self.epoch_length_blocks == 0 {
            return;
        }
        if height > 0 && height.is_multiple_of(self.epoch_length_blocks) {
            // Compute penalties at epoch end.
            let mut committed_no_reveal: Vec<u32> = Vec::new();
            for signer in self.commits.keys() {
                if !self.reveals.contains_key(signer) && !self.late_reveals.contains_key(signer) {
                    committed_no_reveal.push(*signer);
                }
            }
            // Compute neither-committed-nor-revealed using roster snapshot (exact set when available)
            let roster_set: BTreeSet<u32> = if let Some(r) = &self.validator_roster {
                r.clone()
            } else {
                // Fallback: derive contiguous set 0..=max_index from observed signers.
                let max_idx = self
                    .commits
                    .keys()
                    .chain(self.reveals.keys())
                    .copied()
                    .max();
                max_idx.map_or_else(BTreeSet::new, |idx| (0..=idx).collect())
            };
            let mut participated: BTreeSet<u32> = BTreeSet::new();
            for k in self.commits.keys() {
                participated.insert(*k);
            }
            for k in self.reveals.keys() {
                participated.insert(*k);
            }
            for k in self.late_reveals.keys() {
                participated.insert(*k);
            }
            let no_participation: Vec<u32> =
                roster_set.difference(&participated).copied().collect();
            let committed_no_reveal_snapshot = committed_no_reveal.clone();
            let no_participation_snapshot = no_participation.clone();
            self.last_penalties = Some((self.epoch, committed_no_reveal));
            let roster_len_u32 = u32::try_from(roster_set.len()).unwrap_or(u32::MAX);
            self.last_penalties_detailed = Some((
                self.epoch,
                committed_no_reveal_snapshot.clone(),
                no_participation_snapshot.clone(),
                roster_len_u32,
            ));
            self.last_epoch_snapshot = Some(EpochSnapshot {
                epoch: self.epoch,
                seed: self.seed,
                commits: self.commits.iter().map(|(idx, val)| (*idx, *val)).collect(),
                reveals: self.reveals.iter().map(|(idx, val)| (*idx, *val)).collect(),
                late_reveals: self
                    .late_reveals
                    .iter()
                    .map(|(idx, entry)| (*idx, entry.reveal, entry.noted_at_height))
                    .collect(),
                committed_no_reveal: committed_no_reveal_snapshot,
                no_participation: no_participation_snapshot,
                roster_len: roster_len_u32,
                updated_at_height: height,
            });
            let new_seed = self.current_entropy();
            self.seed = new_seed;
            self.epoch = self.epoch.saturating_add(1);
            self.reveals.clear();
            self.clear_commits();
            self.late_reveals.clear();
            self.validator_roster = None;
        }
    }

    /// Snapshot validator roster indices for the current epoch.
    pub fn set_validator_roster_indices(&mut self, indices: impl IntoIterator<Item = u32>) {
        self.validator_roster = Some(indices.into_iter().collect());
    }

    /// Take last penalties report if available.
    pub fn take_last_penalties(&mut self) -> Option<(u64, Vec<u32>)> {
        self.last_penalties.take()
    }

    /// Take last detailed penalties if available.
    pub fn take_last_penalties_detailed(&mut self) -> Option<(u64, Vec<u32>, Vec<u32>, u32)> {
        self.last_penalties_detailed.take()
    }

    /// Take the most recently finalized epoch snapshot, if any.
    pub(crate) fn take_last_epoch_snapshot(&mut self) -> Option<EpochSnapshot> {
        self.last_epoch_snapshot.take()
    }

    /// Snapshot the current epoch state for persistence/telemetry.
    pub(crate) fn snapshot_current_epoch(
        &self,
        roster_len_hint: u32,
        updated_at_height: u64,
    ) -> EpochSnapshot {
        let roster_len = self
            .validator_roster
            .as_ref()
            .and_then(|set| u32::try_from(set.len()).ok())
            .unwrap_or(roster_len_hint);
        EpochSnapshot {
            epoch: self.epoch,
            seed: self.seed(),
            commits: self.commits.iter().map(|(idx, val)| (*idx, *val)).collect(),
            reveals: self.reveals.iter().map(|(idx, val)| (*idx, *val)).collect(),
            late_reveals: self
                .late_reveals
                .iter()
                .map(|(idx, entry)| (*idx, entry.reveal, entry.noted_at_height))
                .collect(),
            committed_no_reveal: Vec::new(),
            no_participation: Vec::new(),
            roster_len,
            updated_at_height,
        }
    }

    /// Restore epoch manager state from a persisted VRF epoch record.
    pub(crate) fn restore_from_record(&mut self, record: &VrfEpochRecord) {
        self.epoch = record.epoch;
        self.seed = record.seed;
        self.set_params(
            record.epoch_length,
            record.commit_deadline_offset,
            record.reveal_deadline_offset,
        );
        self.reveals = record
            .participants
            .iter()
            .filter_map(|p| p.reveal.map(|rev| (p.signer, rev)))
            .collect();
        self.commits = record
            .participants
            .iter()
            .filter_map(|p| p.commitment.map(|commit| (p.signer, commit)))
            .collect();
        self.late_reveals = record
            .late_reveals
            .iter()
            .map(|entry| {
                (
                    entry.signer,
                    LateRevealEntry {
                        reveal: entry.reveal,
                        noted_at_height: entry.noted_at_height,
                    },
                )
            })
            .collect();
        if record.roster_len > 0 {
            let mut roster = std::collections::BTreeSet::new();
            for idx in 0..record.roster_len {
                roster.insert(idx);
            }
            self.validator_roster = Some(roster);
        } else {
            self.validator_roster = None;
        }
        self.last_penalties = None;
        self.last_penalties_detailed = None;
        self.last_epoch_snapshot = None;
        if record.finalized {
            // Finalized records represent completed epochs. Advance the seed/epoch to avoid
            // restoring into an already-closed window if the next-epoch snapshot was not saved.
            let next_seed = self.current_entropy();
            self.seed = next_seed;
            self.epoch = self.epoch.saturating_add(1);
            self.reveals.clear();
            self.clear_commits();
            self.late_reveals.clear();
            self.validator_roster = None;
        }
    }

    /// Epoch index that a block at `height` belongs to.
    pub fn epoch_for_height(&self, height: u64) -> u64 {
        if self.epoch_length_blocks == 0 || height == 0 {
            return 0;
        }
        (height - 1) / self.epoch_length_blocks
    }

    fn pos_in_epoch(&self, height: u64) -> u64 {
        if self.epoch_length_blocks == 0 {
            return 0;
        }
        // 1-based position in epoch window
        let pos0 = (height - 1) % self.epoch_length_blocks;
        pos0 + 1
    }

    fn is_in_commit_window(&self, pos: u64) -> bool {
        pos > 0 && pos <= self.commit_deadline_offset.min(self.epoch_length_blocks)
    }

    fn is_in_reveal_window(&self, pos: u64) -> bool {
        let start = self.commit_deadline_offset.min(self.epoch_length_blocks);
        let end = self.reveal_deadline_offset.min(self.epoch_length_blocks);
        pos > start && pos <= end
    }

    // Commit storage (per-epoch). We keep it opaque to allow future refactors.
    fn commits(&self) -> &BTreeMap<u32, [u8; 32]> {
        &self.commits
    }
    fn commits_mut(&mut self) -> &mut BTreeMap<u32, [u8; 32]> {
        &mut self.commits
    }
    fn clear_commits(&mut self) {
        self.commits.clear();
    }

    fn current_entropy(&self) -> [u8; 32] {
        let mut h = Blake2b512::new();
        iroha_crypto::blake2::digest::Update::update(&mut h, &self.seed);
        for (signer, reveal) in &self.reveals {
            iroha_crypto::blake2::digest::Update::update(&mut h, &signer.to_be_bytes());
            iroha_crypto::blake2::digest::Update::update(&mut h, reveal);
        }
        let digest = iroha_crypto::blake2::Digest::finalize(h);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest[..32]);
        out
    }
}

#[cfg(test)]
impl EpochManager {
    /// Test-only: return current validator roster snapshot length if present.
    pub fn test_current_roster_len(&self) -> Option<usize> {
        self.validator_roster
            .as_ref()
            .map(std::collections::BTreeSet::len)
    }

    /// Test-only accessor returning the number of recorded late reveals.
    pub fn test_late_reveals_len(&self) -> usize {
        self.late_reveals.len()
    }
}

/// Result of attempting to note a VRF commit/reveal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrfNoteResult {
    /// Input accepted for the current epoch in the appropriate window.
    Accepted,
    /// Input accepted as a late reveal (clears penalties but does not mutate the seed).
    AcceptedLate,
    /// Rejected because the epoch index does not match the manager's current epoch.
    RejectedEpochMismatch,
    /// Rejected because the current height is not within the appropriate window.
    RejectedOutOfWindow,
    /// Rejected because the signer is not part of the active validator roster.
    RejectedUnknownSigner,
    /// Rejected because the reveal does not match a prior commitment.
    RejectedInvalidReveal,
}

#[cfg(test)]
mod tests {
    use iroha_data_model::consensus::{VrfEpochRecord, VrfLateRevealRecord, VrfParticipantRecord};

    use super::*;
    #[test]
    fn commit_and_reveal_window_and_epoch_rollover() {
        let chain = ChainId::from("iroha:test:epoch");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(10, 3, 6);
        // Height 1..3: commit window
        for h in 1..=3 {
            let _ = em.try_note_commit_at_height(
                h,
                VrfCommit {
                    epoch: 0,
                    commitment: [1u8; 32],
                    signer: 1,
                },
            );
        }
        // Prepare a reveal matching commitment
        let mut h = iroha_crypto::blake2::Blake2b512::new();
        iroha_crypto::blake2::digest::Update::update(&mut h, &[9u8; 32]);
        let d = iroha_crypto::blake2::Digest::finalize(h);
        let mut commit = [0u8; 32];
        commit.copy_from_slice(&d[..32]);
        let _ = em.try_note_commit_at_height(
            3,
            VrfCommit {
                epoch: 0,
                commitment: commit,
                signer: 2,
            },
        );
        // Height 4..6: reveal window
        for hh in 4..=6 {
            let _ = em.try_note_reveal_at_height(
                hh,
                VrfReveal {
                    epoch: 0,
                    reveal: [0u8; 32],
                    signer: 1,
                },
            );
        }
        // Valid reveal for signer 2
        let _ = em.try_note_reveal_at_height(
            6,
            VrfReveal {
                epoch: 0,
                reveal: [9u8; 32],
                signer: 2,
            },
        );
        // Epoch boundary at height 10: evolves seed and clears state
        let s_before = em.seed();
        em.on_block_commit(10);
        let s_after = em.seed();
        assert_ne!(s_before, s_after);
        assert_eq!(em.epoch(), 1);
    }

    #[test]
    fn set_epoch_overrides_epoch_index() {
        let chain = ChainId::from("iroha:test:epoch-set");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(10, 3, 6);
        em.set_epoch(4);
        assert_eq!(em.epoch(), 4);
    }

    #[test]
    fn reset_epoch_state_clears_accumulated_inputs() {
        let chain = ChainId::from("iroha:test:epoch-reset");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(10, 3, 6);
        let _ = em.try_note_commit_at_height(
            2,
            VrfCommit {
                epoch: 0,
                commitment: [1u8; 32],
                signer: 1,
            },
        );
        let _ = em.try_note_reveal_at_height(
            5,
            VrfReveal {
                epoch: 0,
                reveal: [2u8; 32],
                signer: 1,
            },
        );
        em.set_validator_roster_indices(0..3);
        let snapshot = em.snapshot_current_epoch(3, 5);
        assert!(
            !snapshot.commits.is_empty() || !snapshot.reveals.is_empty(),
            "expected VRF inputs to be recorded"
        );

        let seed = [0xAB; 32];
        em.reset_epoch_state(2, seed);
        let snapshot = em.snapshot_current_epoch(3, 5);
        assert!(snapshot.commits.is_empty());
        assert!(snapshot.reveals.is_empty());
        assert!(snapshot.late_reveals.is_empty());
        assert_eq!(em.epoch(), 2);
        assert_eq!(em.seed(), seed);
        assert_eq!(em.test_current_roster_len(), None);
    }

    #[test]
    fn invalid_reveal_and_out_of_window_ignored_and_penalty_applies() {
        let chain = ChainId::from("iroha:test:epoch2");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(10, 3, 6);
        // Commit in window for signer 3
        let _ = em.try_note_commit_at_height(
            2,
            VrfCommit {
                epoch: 0,
                commitment: [2u8; 32],
                signer: 3,
            },
        );
        // Invalid reveal (hash does not match commit) for signer 3 within reveal window
        let _ = em.try_note_reveal_at_height(
            5,
            VrfReveal {
                epoch: 0,
                reveal: [7u8; 32],
                signer: 3,
            },
        );
        // Commit outside window for signer 4 (ignored)
        let _ = em.try_note_commit_at_height(
            8,
            VrfCommit {
                epoch: 0,
                commitment: [1u8; 32],
                signer: 4,
            },
        );
        // Epoch boundary -> compute penalties
        em.on_block_commit(10);
        let penalties = em.take_last_penalties().unwrap();
        assert_eq!(penalties.0, 0);
        assert_eq!(penalties.1, vec![3]); // signer 3 committed but did not (validly) reveal
    }

    #[test]
    fn try_note_commit_and_reveal_return_status() {
        let chain = ChainId::from("iroha:test:epoch3");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(10, 3, 6);
        // Out of commit window
        let rc = em.try_note_commit_at_height(
            9,
            VrfCommit {
                epoch: 0,
                commitment: [1u8; 32],
                signer: 0,
            },
        );
        assert_eq!(rc, VrfNoteResult::RejectedOutOfWindow);
        // Valid commit then invalid reveal (mismatch)
        let _ = em.try_note_commit_at_height(
            2,
            VrfCommit {
                epoch: 0,
                commitment: [2u8; 32],
                signer: 1,
            },
        );
        let rr = em.try_note_reveal_at_height(
            5,
            VrfReveal {
                epoch: 0,
                reveal: [9u8; 32],
                signer: 1,
            },
        );
        assert_eq!(rr, VrfNoteResult::RejectedInvalidReveal);
    }

    #[test]
    fn vrf_commit_rejects_unknown_signer() {
        let chain = ChainId::from("iroha:test:epoch_unknown_commit");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(10, 3, 6);
        em.set_validator_roster_indices([0, 1]);
        let rc = em.try_note_commit_at_height(
            2,
            VrfCommit {
                epoch: 0,
                commitment: [3u8; 32],
                signer: 2,
            },
        );
        assert_eq!(rc, VrfNoteResult::RejectedUnknownSigner);
    }

    #[test]
    fn vrf_reveal_rejects_unknown_signer() {
        let chain = ChainId::from("iroha:test:epoch_unknown_reveal");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(10, 3, 6);
        em.set_validator_roster_indices([0, 1]);
        let rr = em.try_note_reveal_at_height(
            5,
            VrfReveal {
                epoch: 0,
                reveal: [4u8; 32],
                signer: 2,
            },
        );
        assert_eq!(rr, VrfNoteResult::RejectedUnknownSigner);
    }

    #[test]
    fn vrf_reveal_rejects_early_reveal() {
        let chain = ChainId::from("iroha:test:epoch_early_reveal");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(10, 3, 6);

        let reveal = [0x33; 32];
        let commit: [u8; 32] = iroha_crypto::Hash::new(reveal).into();
        assert_eq!(
            em.try_note_commit_at_height(
                2,
                VrfCommit {
                    epoch: 0,
                    commitment: commit,
                    signer: 0,
                },
            ),
            VrfNoteResult::Accepted
        );

        let rr = em.try_note_reveal_at_height(
            2,
            VrfReveal {
                epoch: 0,
                reveal,
                signer: 0,
            },
        );
        assert_eq!(rr, VrfNoteResult::RejectedOutOfWindow);
        assert_eq!(em.test_late_reveals_len(), 0);
    }

    #[test]
    fn late_reveal_clears_penalty_without_seed_change() {
        let chain = ChainId::from("iroha:test:epoch_late");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(10, 3, 6);

        let reveal = [0x55; 32];
        let commit: [u8; 32] = iroha_crypto::Hash::new(reveal).into();
        assert_eq!(
            em.try_note_commit_at_height(
                2,
                VrfCommit {
                    epoch: 0,
                    commitment: commit,
                    signer: 0,
                },
            ),
            VrfNoteResult::Accepted
        );
        let seed_before = em.seed();
        assert_eq!(
            em.try_note_reveal_at_height(
                7,
                VrfReveal {
                    epoch: 0,
                    reveal,
                    signer: 0,
                },
            ),
            VrfNoteResult::AcceptedLate
        );
        assert_eq!(em.seed(), seed_before);
        assert_eq!(em.test_late_reveals_len(), 1);

        em.on_block_commit(10);
        let penalties = em.take_last_penalties().unwrap();
        assert!(penalties.1.is_empty(), "late reveal should clear penalties");
    }

    #[test]
    fn seed_stays_stable_within_epoch() {
        let chain = ChainId::from("iroha:test:epoch_seed_stable");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(10, 3, 6);

        let reveal = [0xAA; 32];
        let commit: [u8; 32] = iroha_crypto::Hash::new(reveal).into();
        assert_eq!(
            em.try_note_commit_at_height(
                2,
                VrfCommit {
                    epoch: 0,
                    commitment: commit,
                    signer: 0,
                },
            ),
            VrfNoteResult::Accepted
        );
        let seed_before = em.seed();
        assert_eq!(
            em.try_note_reveal_at_height(
                5,
                VrfReveal {
                    epoch: 0,
                    reveal,
                    signer: 0,
                },
            ),
            VrfNoteResult::Accepted
        );
        assert_eq!(em.seed(), seed_before);

        em.on_block_commit(10);
        assert_ne!(em.seed(), seed_before);
    }

    #[test]
    fn vrf_note_rejects_height_zero() {
        let chain = ChainId::from("iroha:test:epoch_height_zero");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(10, 3, 6);

        let commit = VrfCommit {
            epoch: 0,
            commitment: [0x11; 32],
            signer: 0,
        };
        let reveal = VrfReveal {
            epoch: 0,
            reveal: [0x22; 32],
            signer: 0,
        };

        assert_eq!(
            em.try_note_commit_at_height(0, commit),
            VrfNoteResult::RejectedOutOfWindow
        );
        assert_eq!(
            em.try_note_reveal_at_height(0, reveal),
            VrfNoteResult::RejectedOutOfWindow
        );
    }

    #[test]
    fn set_epoch_seed_overrides_initial_seed() {
        let chain = ChainId::from("iroha:test:epoch_seed_override");
        let mut em = EpochManager::new_from_chain(&chain);
        let seed = [0x22; 32];
        em.set_epoch_seed(seed);
        assert_eq!(em.seed(), seed);
    }

    #[test]
    fn restore_from_record_updates_epoch_params() {
        let chain = ChainId::from("iroha:test:epoch_restore_params");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(10, 3, 6);

        let record = VrfEpochRecord {
            epoch: 2,
            seed: [0x11; 32],
            epoch_length: 12,
            commit_deadline_offset: 4,
            reveal_deadline_offset: 9,
            roster_len: 0,
            finalized: false,
            updated_at_height: 0,
            participants: Vec::new(),
            late_reveals: Vec::new(),
            committed_no_reveal: Vec::new(),
            no_participation: Vec::new(),
            penalties_applied: false,
            penalties_applied_at_height: None,
            validator_election: None,
        };

        em.restore_from_record(&record);
        assert_eq!(em.epoch_length_blocks(), 12);
        assert_eq!(em.commit_window_end(), 4);
        assert_eq!(em.reveal_window_end(), 9);
    }

    #[test]
    fn restore_from_finalized_record_advances_epoch_and_seed() {
        let chain = ChainId::from("iroha:test:epoch_restore_finalized");
        let record_seed = [0x11; 32];
        let reveal_a = [0x22; 32];
        let reveal_b = [0x33; 32];

        let participants = vec![
            VrfParticipantRecord {
                signer: 0,
                commitment: Some([0x44; 32]),
                reveal: Some(reveal_a),
                last_updated_height: 9,
            },
            VrfParticipantRecord {
                signer: 2,
                commitment: Some([0x55; 32]),
                reveal: Some(reveal_b),
                last_updated_height: 9,
            },
        ];

        let record = VrfEpochRecord {
            epoch: 7,
            seed: record_seed,
            epoch_length: 12,
            commit_deadline_offset: 4,
            reveal_deadline_offset: 9,
            roster_len: 4,
            finalized: true,
            updated_at_height: 12,
            participants,
            late_reveals: vec![VrfLateRevealRecord {
                signer: 1,
                reveal: [0x66; 32],
                noted_at_height: 10,
            }],
            committed_no_reveal: vec![2],
            no_participation: vec![3],
            penalties_applied: false,
            penalties_applied_at_height: None,
            validator_election: None,
        };

        let mut expected = EpochManager::new_from_chain(&chain);
        expected.set_epoch_seed(record_seed);
        expected.reveals.insert(0, reveal_a);
        expected.reveals.insert(2, reveal_b);
        let expected_seed = expected.current_entropy();

        let mut em = EpochManager::new_from_chain(&chain);
        em.restore_from_record(&record);

        assert_eq!(em.epoch(), record.epoch + 1);
        assert_eq!(em.seed(), expected_seed);
        let snapshot = em.snapshot_current_epoch(0, 0);
        assert!(snapshot.commits.is_empty());
        assert!(snapshot.reveals.is_empty());
        assert!(snapshot.late_reveals.is_empty());
        assert_eq!(em.test_current_roster_len(), None);
    }

    #[test]
    fn penalties_use_validator_roster_snapshot() {
        let chain = ChainId::from("iroha:test:epoch4");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(8, 3, 6);

        // Roster contains three validators {0,1,2}
        em.set_validator_roster_indices([0, 1, 2]);

        // Valid commit + reveal for signer 0
        let reveal0 = [11u8; 32];
        let commit0: [u8; 32] = iroha_crypto::Hash::new(reveal0).into();
        assert_eq!(
            em.try_note_commit_at_height(
                2,
                VrfCommit {
                    epoch: 0,
                    commitment: commit0,
                    signer: 0,
                },
            ),
            VrfNoteResult::Accepted
        );
        assert_eq!(
            em.try_note_reveal_at_height(
                5,
                VrfReveal {
                    epoch: 0,
                    reveal: reveal0,
                    signer: 0,
                },
            ),
            VrfNoteResult::Accepted
        );

        // Commit without reveal for signer 1 (penalty target)
        assert_eq!(
            em.try_note_commit_at_height(
                3,
                VrfCommit {
                    epoch: 0,
                    commitment: [0xAA; 32],
                    signer: 1,
                },
            ),
            VrfNoteResult::Accepted
        );

        // Signer 2 neither commits nor reveals → should appear in no-participation list

        // Trigger epoch rollover at height 8
        em.on_block_commit(8);

        let penalties = em
            .take_last_penalties_detailed()
            .expect("penalties should be produced");
        assert_eq!(penalties.0, 0);
        assert_eq!(penalties.1, vec![1]); // signer 1 committed but failed to reveal
        assert_eq!(penalties.2, vec![2]); // signer 2 did not participate at all
        assert_eq!(penalties.3, 3); // roster size propagated
    }

    #[test]
    fn zero_participation_epoch_marks_all_validators() {
        let chain = ChainId::from("iroha:test:epoch_zero_participation");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(6, 2, 4);

        em.set_validator_roster_indices(0..4);

        em.on_block_commit(6);

        let (_, committed_no_reveal, no_participation, roster_len) = em
            .take_last_penalties_detailed()
            .expect("penalties should be recorded");
        assert!(
            committed_no_reveal.is_empty(),
            "no commits recorded, penalties should be empty"
        );
        assert_eq!(
            no_participation,
            vec![0, 1, 2, 3],
            "every validator should appear in no-participation list"
        );
        assert_eq!(roster_len, 4);
    }

    #[test]
    fn empty_roster_without_participation_skips_penalties() {
        let chain = ChainId::from("iroha:test:epoch_empty_roster");
        let mut em = EpochManager::new_from_chain(&chain);
        em.set_params(4, 1, 2);

        em.on_block_commit(4);

        let (_, committed_no_reveal, no_participation, roster_len) = em
            .take_last_penalties_detailed()
            .expect("penalties should be recorded");
        assert!(committed_no_reveal.is_empty());
        assert!(no_participation.is_empty());
        assert_eq!(roster_len, 0);
    }
}
