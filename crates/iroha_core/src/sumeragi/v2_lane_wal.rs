//! Physical persistence boundary for the shared native lane reducer.
//!
//! The caller owns the reducer and its exact outstanding `Persist` effect.
//! This owner retains no second lock, vote guard, retry policy or view counter.
//! It acknowledges only the matching native record after actual Kura-bound
//! append/fsync, and reauthenticates complete native evidence on every reopen.
//! TODO: wire this owner into the process-lived lane instance driver, retaining
//! its effects across global height rollover and draining them before closure.

use iroha_crypto::Hash;

use super::{
    safety_wal::SafetyWal,
    v2_core as reducer,
    v2_lane_wire::{LaneAuthenticator, LaneWalEnvelopeV1},
};
use crate::{kura::Kura, state::VerifiedLaneContext};

/// Physical lane persistence or authenticated replay failure.
#[derive(Debug, thiserror::Error)]
#[error("native lane safety WAL: {0}")]
pub(crate) struct LaneWalError(String);

fn bad(error: impl ToString) -> LaneWalError {
    LaneWalError(error.to_string())
}

/// One immutable instance and frozen local key's actual safety WAL.
pub(crate) struct LaneSafetyWal {
    storage: SafetyWal,
    verified: VerifiedLaneContext,
    signer: u32,
    append_failed: bool,
}

impl LaneSafetyWal {
    /// Open under Kura's descriptor-relative directory authority. Full instance
    /// and key identities distinguish cancellation/reopening at the same slot.
    pub(crate) fn open(
        kura: &Kura,
        verified: &VerifiedLaneContext,
        signer: u32,
    ) -> Result<Self, LaneWalError> {
        let identity = LaneAuthenticator::new(verified)
            .wal_identity(signer)
            .map_err(bad)?;
        let name = format!(
            "lane-{}-{}.wal",
            Hash::prehashed(*identity.context_id().as_bytes()),
            Hash::prehashed(identity.consensus_key_hash()),
        );
        let authority = kura.mint_safety_wal_directory_authority().map_err(bad)?;
        let storage =
            SafetyWal::open_with_kura_authority(kura, authority, name, identity).map_err(bad)?;
        Ok(Self {
            storage,
            verified: verified.clone(),
            signer,
            append_failed: false,
        })
    }

    /// Recover the sole shared reducer, still gated by `ResumeAfterReplay`.
    /// Physical checksums never replace native signatures or logical replay.
    pub(crate) fn recover(
        &self,
        generation: reducer::Generation,
    ) -> Result<reducer::Reducer, LaneWalError> {
        if self.append_failed {
            return Err(bad("failed append requires a physical reopen"));
        }
        let auth = LaneAuthenticator::new(&self.verified);
        let entries = self
            .storage
            .recovered_records()
            .iter()
            .map(|frame| auth.decode_storage_wal(frame).map_err(bad))
            .collect::<Result<Vec<_>, _>>()?;
        let context = self.verified.reducer_context();
        reducer::Reducer::recover(
            context.clone(),
            Some(context.roster()[self.signer as usize].id()),
            generation,
            entries,
        )
        .map_err(bad)
    }

    /// Persist the exact effect retained by the caller and return its fsync ack.
    /// No retry after a physical append failure is allowed without reopening;
    /// an uncertain complete frame must be resolved by normal WAL recovery.
    pub(crate) fn append_issued(
        &mut self,
        effect: &reducer::Effect,
        native: &LaneWalEnvelopeV1,
    ) -> Result<reducer::Event, LaneWalError> {
        if self.append_failed {
            return Err(bad("failed append requires a physical reopen"));
        }
        let reducer::Effect::Persist { tag, entry } = effect else {
            return Err(bad("only a reducer Persist effect can be acknowledged"));
        };
        let context = self.verified.reducer_context();
        if tag.height() != context.height() {
            return Err(bad("persistence effect belongs to another lane height"));
        }
        let local = context.roster()[self.signer as usize].id();
        let signing_identity = match entry.record() {
            reducer::WalRecord::ProposalIntent(proposal) => Some(proposal.proposer()),
            reducer::WalRecord::PrepareIntent(vote)
            | reducer::WalRecord::LockAndCommit { vote, .. } => Some(vote.signer()),
            reducer::WalRecord::TimeoutIntent(vote) => Some(vote.signer()),
            reducer::WalRecord::ObservePrepare(_)
            | reducer::WalRecord::InstallTimeout(_)
            | reducer::WalRecord::Decision(_) => None,
        };
        if signing_identity.is_some_and(|signer| signer != local) {
            return Err(bad("signing intent belongs to another frozen local key"));
        }
        let sequence = self.storage.recovered_records().len() as u64;
        if sequence.checked_add(1) != Some(entry.id().get()) {
            return Err(bad("effect is not the next physical persistence id"));
        }
        let payload = LaneAuthenticator::new(&self.verified)
            .encode_wal(native, entry)
            .map_err(bad)?;
        // From this point a failed operation can have changed disk state. Only
        // successful exact-receipt checks make this owner appendable again.
        self.append_failed = true;
        let receipt = self.storage.append(&payload).map_err(bad)?;
        let retained = self
            .storage
            .recovered_records()
            .last()
            .ok_or_else(|| bad("fsync receipt has no retained complete frame"))?;
        if receipt.sequence() != sequence
            || !retained.exactly_matches_receipt(receipt)
            || retained.payload() != payload
        {
            return Err(bad("fsync receipt differs from exact issued native frame"));
        }
        self.append_failed = false;
        Ok(reducer::Event::Persisted {
            tag: *tag,
            id: entry.id(),
        })
    }
}
