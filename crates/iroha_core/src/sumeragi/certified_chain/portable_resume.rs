//! Closed portable-parent to original native-successor join after checkpoint bootstrap.
//!
//! Omitted bodies never become `CommittedBlock` or `CheckedFrameProjection`. The sealed
//! portable parent lends only successor predicates. A genuine full H+1 frame then passes
//! the unchanged native frame, quorum, availability and schedule verifier before becoming
//! the ordinary cursor. Query cursors retain the parent scalars with no invented wire fence.

use super::*;
use iroha_data_model::sumeragi_finality::{
    ScheduledConfig, ScheduledSlot, VerifiedPortableCheckpointV1,
};

/// Reverify the installed portable cut against the same original State publication.
/// This lends certified metadata only; callers cannot obtain native body custody at H.
pub(crate) fn portable_checkpoint_for_state(
    state: &(impl StateReadOnly + ?Sized),
) -> Result<Option<VerifiedPortableCheckpointV1>, ExecutionAttemptError<ChainReadError>> {
    let Some(height) = state.kura().checkpoint_prefix_height() else {
        return Ok(None);
    };
    let malformed = |reason: &str| ChainReadError::Malformed {
        height,
        reason: reason.into(),
    };
    let tip = state.native_execution_tip().ok_or_else(|| {
        malformed("portable authority requires the original published native execution tip")
    })?;
    if tip.height() != state.height() as u64
        || Some(tip.iroha_hash()) != state.latest_block_hash()
        || tip.height() < height
    {
        return Err(malformed("portable authority differs from the captured State cut").into());
    }
    let next = height
        .checked_add(1)
        .ok_or_else(|| malformed("checkpoint height overflows"))?;
    let sealed = ChainSource::State(state)
        .portable_checkpoint(next)?
        .ok_or_else(|| malformed("installed portable checkpoint disappeared"))?;
    let parent = sealed.checkpoint();
    if tip.root_scope() != parent.root_scope()
        || (tip.height() == height
            && (tip.iroha_hash() != parent.header().hash()
                || tip.core_hash() != parent.core_hash()
                || tip.result() != parent.result()))
    {
        return Err(malformed("portable checkpoint differs from native execution identity").into());
    }
    Ok(Some(sealed))
}

impl<V: StateReadOnly + ?Sized> ChainSource<'_, V> {
    fn portable_checkpoint(
        &self,
        target: u64,
    ) -> Result<Option<VerifiedPortableCheckpointV1>, ExecutionAttemptError<ChainReadError>> {
        let (kura, count) = match self {
            Self::State(view) => {
                let hashes = view.block_hashes();
                (view.kura(), hashes.len())
            }
            Self::Pinned { kura, hashes, .. } => {
                let count = match hashes {
                    PinnedHashes::Slice(values) => values.len(),
                    PinnedHashes::Captured(values) => values.len(),
                };
                (*kura, count)
            }
            Self::Frames { .. } => return Ok(None),
        };
        let hash_at = |index| match self {
            Self::State(view) => view.block_hashes().get(index).copied(),
            Self::Pinned { hashes, .. } => hashes.get(index).copied(),
            Self::Frames { .. } => None,
        };
        let Some(height) = kura.checkpoint_prefix_height() else {
            return Ok(None);
        };
        if target == GENESIS_HEIGHT {
            return Ok(None);
        }
        if target <= height {
            return Err(ChainReadError::NotInView { height: target }.into());
        }
        kura.walk_checkpoint_prefix_lookup(
            self.chain_id(),
            *self.network_id(),
            count,
            &hash_at,
            &mut |_| Ok(()),
        )
        .map_err(|error| {
            error.map_rejection(|error| ChainReadError::Malformed {
                height,
                reason: error.to_string(),
            })
        })
    }
}

/// Only a verified transferred parent can construct this transient predecessor owner.
struct PortableParent {
    tip: ParentProjection,
    schedule: schedule::ConsensusSchedule,
    authority: PrefixAuthority,
    validation: EpochValidationScope,
}

impl PortableParent {
    fn from_sealed(
        sealed: &VerifiedPortableCheckpointV1,
        instance: Hash32,
        scope: SumeragiRootScope,
        query: bool,
    ) -> Result<Self, ExecutionAttemptError<ChainReadError>> {
        let parent = sealed.checkpoint();
        let height = parent.header().height().get();
        let outcome = &parent.certified_result().commitment().schedule;
        if parent.root_scope() != scope || sealed.successor_header().instance != instance {
            return Err(ChainReadError::WrongInstance { height }.into());
        }
        let params = sealed.predecessor().map_or_else(
            || *outcome.next.params(),
            |previous| {
                *previous
                    .certified_result()
                    .commitment()
                    .schedule
                    .next
                    .params()
            },
        );
        // These exact three slots were authenticated by the portable cursor. Account for
        // each actual cloned epoch graph before reconstructing the native schedule window.
        norito::core::reserve_decode_allocation(3 * std::mem::size_of::<ScheduledSlot>())
            .map_err(|error| read_decode_error(height, error))?;
        query_reader::admit_epoch_clone(&outcome.current, height)?;
        for slot in [&outcome.next, &outcome.after_next] {
            if let ScheduledSlot::Ready(config) = slot {
                query_reader::admit_epoch_clone(&config.epoch, height)?;
            }
        }
        let schedule = schedule::ConsensusSchedule::from_owned_entries(vec![
            ScheduledSlot::Ready(ScheduledConfig {
                height,
                epoch: outcome.current.clone(),
                params,
            }),
            outcome.next.clone(),
            outcome.after_next.clone(),
        ])
        .map_err(|error| ChainReadError::Committee {
            height,
            reason: error.to_string(),
        })?;
        let mut validation = EpochValidationScope::new();
        query_reader::admit_epoch_clone(&outcome.current, height)?;
        norito::core::reserve_decode_arc_allocation::<VerifiedAuthority>()
            .map_err(|error| read_decode_error(height, error))?;
        let authority = if query {
            VerifiedAuthority::new_query(outcome.current.clone(), height, &mut validation)?
        } else {
            VerifiedAuthority::new(outcome.current.clone(), height, &mut validation)?
        };
        Ok(Self {
            tip: ParentProjection {
                height,
                iroha_hash: parent.header().hash(),
                core_hash: parent.core_hash(),
                result: parent.result(),
                root_scope: scope,
                beacon: parent.certified_result().commitment().beacon,
            },
            schedule,
            authority: PrefixAuthority::Transient(Arc::new(authority)),
            validation,
        })
    }

    fn borrowed(&mut self) -> PrefixRef<'_> {
        PrefixRef {
            tip: self.tip,
            schedule: &self.schedule,
            authority: &self.authority,
            validation: &mut self.validation,
            proof_source: None,
        }
    }
}

impl<V: StateReadOnly + ?Sized> CertifiedChain<'_, V> {
    pub(super) fn portable_native_prefix(
        &self,
        target: u64,
    ) -> Result<Option<VerifiedPrefix>, ExecutionAttemptError<ChainReadError>> {
        let Some(sealed) = self.source.portable_checkpoint(target)? else {
            return Ok(None);
        };
        let mut parent =
            PortableParent::from_sealed(&sealed, self.instance, self.root_scope, false)?;
        let height = sealed.successor_header().height;
        let original = read_frame_with_validation(
            self.source.block(height)?,
            height,
            self.root_scope,
            &mut parent.validation,
        )?;
        if original.header() != Some(sealed.successor_header())
            || original.certified_result() != sealed.successor_result()
        {
            return Err(ChainReadError::HeaderMismatch { height }.into());
        }
        let prepared = self.verification_context().prepare_advance_source(
            &mut parent.borrowed(),
            CertificateSource::Original(&original),
            None,
            (false, None),
            None,
            None,
        )?;
        Ok(Some(VerifiedPrefix {
            tip: original,
            schedule: prepared.schedule,
            authority: prepared.authority,
            validation: parent.validation,
            proof_source: None,
        }))
    }
}

impl QueryCertifiedChain<'_> {
    pub(super) fn portable_query_prefix(
        &self,
        target: u64,
    ) -> Result<Option<query_reader::QueryPrefix>, ExecutionAttemptError<ChainReadError>> {
        let Some(sealed) = self.source.portable_checkpoint(target)? else {
            return Ok(None);
        };
        let parent = PortableParent::from_sealed(&sealed, self.instance, self.root_scope, true)?;
        Ok(Some(query_reader::QueryPrefix {
            tip: parent.tip,
            wire: None,
            schedule: parent.schedule,
            authority: parent.authority,
            validation: parent.validation,
            proof_source: None,
            portable_successor: Some((
                sealed.successor_header().height,
                sealed.successor_core_hash(),
                sealed.successor_result_hash(),
            )),
        }))
    }
}

impl PrefixVerifierContext {
    /// Re-verify an actual frame for the same fully checked native decision after a resume join.
    pub(super) fn prepare_retained_native_certificate(
        &self,
        prefix: &mut VerifiedPrefix,
        offered: &CommittedBlock,
    ) -> Result<CertificateVerdict, ExecutionAttemptError<ChainReadError>> {
        let height = offered.height();
        if height != prefix.tip.height()
            || offered.block_hash() != prefix.tip.block_hash()
            || offered.core_hash != prefix.tip.core_hash
            || offered.result != prefix.tip.result
        {
            return Err(ChainReadError::NotInView { height }.into());
        }
        let config = prefix
            .schedule
            .ready(height)
            .and_then(|scheduled| scheduled.height_config_with_validation(&mut prefix.validation))
            .map_err(|error| ChainReadError::Committee {
                height,
                reason: error.to_string(),
            })?;
        self.prepare_certificate(offered, &prefix.authority, Some(&config), None, None)
    }
}

#[cfg(test)]
mod tests;
