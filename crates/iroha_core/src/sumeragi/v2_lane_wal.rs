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

/// Complete authenticated replay and its exact native witnesses.
///
/// This is an immutable recovery result, not a second safety-state owner. Only
/// the shared reducer interprets locks, votes and views. Native envelopes retain
/// the manifest, immutable value and signing preimages required by its effects.
/// No partial result escapes a failed physical/native/logical replay.
pub(crate) struct RecoveredLaneWal {
    reducer: reducer::Reducer,
    native_records: Vec<LaneWalEnvelopeV1>,
}

impl RecoveredLaneWal {
    /// Exact canonical records in increasing physical/persistence order.
    pub(crate) fn native_records(&self) -> &[LaneWalEnvelopeV1] {
        &self.native_records
    }

    /// Transfer the sole reducer and its witnesses to the process-lived driver.
    /// Fresh current-instance gates and body custody remain separate obligations.
    pub(crate) fn into_parts(self) -> (reducer::Reducer, Vec<LaneWalEnvelopeV1>) {
        (self.reducer, self.native_records)
    }
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

    /// Open the sole fixed native input store adjacent to this exact lane WAL.
    /// This does not resume the reducer or authorize any body completion.
    pub(crate) fn open_body_store(
        &self,
    ) -> Result<super::v2_lane_body_store::LaneBodyStore, LaneWalError> {
        if self.append_failed {
            return Err(bad("failed append requires a physical reopen"));
        }
        let authority = self
            .storage
            .mint_native_body_store_authority()
            .map_err(bad)?;
        super::v2_lane_body_store::LaneBodyStore::open(
            authority,
            self.verified.clone(),
            self.signer,
        )
        .map_err(bad)
    }
    /// Recover the sole shared reducer, still gated by `ResumeAfterReplay`.
    /// Physical checksums never replace native signatures or logical replay.
    pub(crate) fn recover(
        &self,
        generation: reducer::Generation,
    ) -> Result<reducer::Reducer, LaneWalError> {
        self.recover_with_native(generation)
            .map(|recovered| recovered.into_parts().0)
    }

    /// Authenticate one replay, retaining complete native witnesses for restart.
    /// The returned reducer remains gated by `ResumeAfterReplay`; returning the
    /// native records grants neither a signing lease nor body readiness.
    pub(crate) fn recover_with_native(
        &self,
        generation: reducer::Generation,
    ) -> Result<RecoveredLaneWal, LaneWalError> {
        if self.append_failed {
            return Err(bad("failed append requires a physical reopen"));
        }
        let auth = LaneAuthenticator::new(&self.verified);
        let mut native_records = Vec::with_capacity(self.storage.recovered_records().len());
        let mut entries = Vec::with_capacity(self.storage.recovered_records().len());
        for frame in self.storage.recovered_records() {
            let (native, entry) = auth.decode_storage_wal_with_envelope(frame).map_err(bad)?;
            native_records.push(native);
            entries.push(entry);
        }
        let context = self.verified.reducer_context();
        let reducer = reducer::Reducer::recover(
            context.clone(),
            Some(context.roster()[self.signer as usize].id()),
            generation,
            entries,
        )
        .map_err(bad)?;
        Ok(RecoveredLaneWal {
            reducer,
            native_records,
        })
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
