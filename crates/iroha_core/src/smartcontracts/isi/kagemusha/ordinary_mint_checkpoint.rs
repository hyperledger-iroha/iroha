//! Governed ordinary Mint checkpoint lifecycle driven by authentic Node originals.
//!
//! Only signed genesis can bootstrap. Later epochs require the actual incumbent-certified
//! boundary, including Retain and cancellation dispositions; neither a finalized source nor
//! an offered roster can substitute for a recursive authority checkpoint. Each step is
//! fsynced and read back under the same Kura owner, then independently verified again.
use super::ordinary_mint_publication::KagemushaAuthenticatedOrdinaryNodeFinalizedMintSourceV1;
use super::*;
use crate::state::StateReadOnly as _;
use iroha_data_model::sumeragi_finality::genesis_epoch;
use std::num::NonZeroUsize;

/// At most one real bootstrap or certified epoch edge is proved by a call.
/// A background owner may resume additional calls; this is never an HTTP-time proof loan.
pub(super) struct KagemushaOrdinaryMintCheckpointOwnerV1<'source, 'state> {
    source: &'source KagemushaAuthenticatedOrdinaryNodeFinalizedMintSourceV1<'state>,
    checkpoint: KagemushaMintAuthorityCheckpointV1,
    current: KagemushaMintFinalityEpochAuthorizationV1,
    target: KagemushaMintFinalityEpochAuthorizationV1,
    frozen: bool,
}
impl<'source, 'state> KagemushaOrdinaryMintCheckpointOwnerV1<'source, 'state> {
    pub(super) fn open(
        source: &'source KagemushaAuthenticatedOrdinaryNodeFinalizedMintSourceV1<'state>,
    ) -> Result<Self, String> {
        source.recheck_retained_custody()?;
        let runtime = source.runtime()?;
        let release = source.record()?.release_id;
        let view = source.view();
        let genesis = view
            .canonical_block_by_height(NonZeroUsize::new(1).expect("one is nonzero"))
            .map_err(|e| e.to_string())?;
        let epoch = genesis_epoch(&genesis).map_err(|error| error.to_string())?;
        if epoch.network_id != *view.network_id()
            || epoch.authorization.decision != KagemushaMintFinalityEpochDecisionV1::Genesis
        {
            return Err("ordinary Mint checkpoint bootstrap is not actual signed genesis".into());
        }
        let head = epoch
            .authorization
            .authorization_id()
            .map_err(|e| e.to_string())?;
        // Recursive verifier's independently installed genesis ID is checked by both
        // the genuine bootstrap producer and loaded-checkpoint verifier below.
        let checkpoint = match view
            .kura()
            .kagemusha_mint_authority_checkpoint_v1(release, head)
            .map_err(|e| e.to_string())?
        {
            Some(checkpoint) => checkpoint,
            None => {
                let proved = runtime.prove_mint_authority_bootstrap(
                    release,
                    &epoch.authority,
                    &epoch.authorization,
                )?;
                runtime.verify_mint_authority_checkpoint(release, &epoch.authorization, &proved)?;
                view.kura()
                    .store_kagemusha_mint_authority_checkpoint_v1(&proved)
                    .map_err(|e| e.to_string())?;
                let persisted = view
                    .kura()
                    .kagemusha_mint_authority_checkpoint_v1(release, head)
                    .map_err(|e| e.to_string())?
                    .ok_or("ordinary bootstrap missing after durable publication")?;
                if persisted != proved {
                    return Err("ordinary bootstrap durable original changed".into());
                }
                persisted
            }
        };
        runtime.verify_mint_authority_checkpoint(release, &epoch.authorization, &checkpoint)?;
        let (bundle, _) = crate::sumeragi::attestation::verify_native_mint_finality_bundle(
            &source.finalized()?.finality.finality_proof,
            source.trust_anchor()?,
        )?;
        let target = bundle.message.epoch_authorization;
        if target.network_id != epoch.network_id || target.epoch < epoch.authorization.epoch {
            return Err("ordinary source epoch precedes its authentic signed genesis".into());
        }
        let mut checkpoint = checkpoint;
        let mut current = epoch.authorization;
        if let Some(hint) = view
            .kura()
            .ordinary_mint_checkpoint_progress_v1(release)
            .map_err(|e| e.to_string())?
        {
            if hint.network_id != epoch.network_id {
                return Err("ordinary checkpoint progress network differs".into());
            }
            // An older target may already own its exact immutable checkpoint; never use a later
            // progress hint as authority for that target or jump over an absent historical row.
            let selected = if hint.epoch > target.epoch {
                target
            } else {
                hint
            };
            let actual =
                crate::sumeragi::certified_chain::committed_block(view, selected.first_height)
                    .map_err(|e| e.to_string())?;
            if actual.commitment().schedule.current.authorization != selected {
                return Err(
                    "ordinary checkpoint progress differs from actual native schedule".into(),
                );
            }
            let selected_head = selected.authorization_id().map_err(|e| e.to_string())?;
            if let Some(loaded) = view
                .kura()
                .kagemusha_mint_authority_checkpoint_v1(release, selected_head)
                .map_err(|e| e.to_string())?
            {
                runtime.verify_mint_authority_checkpoint(release, &selected, &loaded)?;
                checkpoint = loaded;
                current = selected;
            } else if selected == hint {
                return Err(
                    "ordinary checkpoint progress omits its actual immutable checkpoint".into(),
                );
            }
        }
        source.recheck_retained_custody()?;
        Ok(Self {
            source,
            checkpoint,
            current,
            target,
            frozen: false,
        })
    }
    /// A closed retained checkpoint only after the exact source epoch is reached.
    pub(super) fn ready(&self) -> Result<Option<&KagemushaMintAuthorityCheckpointV1>, String> {
        self.recheck()?;
        Ok((self.current == self.target).then_some(&self.checkpoint))
    }
    /// Advance one genuine scheduled boundary, or return false when already ready.
    /// On uncertain write/readback the owner is permanently closed; restart reads the same
    /// immutable paths and verifies every cached predecessor, without reissuing proof bytes.
    pub(super) fn advance_one(&mut self) -> Result<bool, String> {
        self.recheck()?;
        if self.current == self.target {
            return Ok(false);
        }
        if self.current.epoch >= self.target.epoch
            || self.current.last_height >= self.source.finalized()?.finality.finality_proof.height()
        {
            return Err(
                "ordinary checkpoint cannot reach source by the authentic next epoch".into(),
            );
        }
        let view = self.source.view();
        let boundary = crate::query::native_receipts::kagemusha_finality_source(
            view,
            self.current.last_height,
        )?;
        let anchor = KagemushaFinalityTrustAnchorV1 {
            network_id: *view.network_id(),
            checkpoint: crate::sumeragi::finality::build_checkpoint(view, self.current.last_height)
                .map_err(|e| e.to_string())?,
        };
        let (bundle, _) = crate::sumeragi::attestation::verify_native_mint_finality_bundle(
            boundary.finality_proof(),
            &anchor,
        )?;
        if bundle.message.epoch_authorization != self.current {
            return Err("ordinary checkpoint boundary changes its actual incumbent".into());
        }
        let next = bundle
            .message
            .next_epoch_authorization
            .ok_or("ordinary checkpoint authentic next epoch absent")?;
        require_adjacent_epoch(&self.current, &next, &self.target)?;
        let runtime = self.source.runtime()?;
        let release = self.source.record()?.release_id;
        let head = next.authorization_id().map_err(|e| e.to_string())?;
        let persisted = match view
            .kura()
            .kagemusha_mint_authority_checkpoint_v1(release, head)
            .map_err(|e| e.to_string())?
        {
            Some(checkpoint) => checkpoint,
            None => {
                let proved = runtime.prove_mint_authority_rotation(
                    release,
                    boundary.finality_proof(),
                    &anchor,
                    boundary.first_top_up_membership()?,
                    &self.checkpoint,
                )?;
                runtime.verify_mint_authority_checkpoint(release, &next, &proved)?;
                // From here any uncertainty closes this process owner. A durable row is
                // admitted on cold restart only through exact immutable readback+proof.
                self.frozen = true;
                view.kura()
                    .store_kagemusha_mint_authority_checkpoint_v1(&proved)
                    .map_err(|e| e.to_string())?;
                let loaded = view
                    .kura()
                    .kagemusha_mint_authority_checkpoint_v1(release, head)
                    .map_err(|e| e.to_string())?
                    .ok_or("ordinary epoch checkpoint absent after durable publication")?;
                if loaded != proved {
                    return Err("ordinary epoch checkpoint durable original changed".into());
                }
                loaded
            }
        };
        runtime.verify_mint_authority_checkpoint(release, &next, &persisted)?;
        // The cursor is independently reauthenticated recovery data, never checkpoint authority.
        // Any uncertainty here closes this owner too; no subsequent new proof is dispatched.
        self.frozen = true;
        view.kura()
            .store_ordinary_mint_checkpoint_progress_v1(release, &next)
            .map_err(|e| e.to_string())?;
        // Memory follows both durable originals before a subsequent custody check can fail.
        self.current = next;
        self.checkpoint = persisted;
        self.frozen = false;
        self.recheck()?;
        Ok(true)
    }
    fn recheck(&self) -> Result<(), String> {
        if self.frozen {
            return Err("ordinary checkpoint publication outcome is uncertain".into());
        }
        self.source.recheck_retained_custody()?;
        self.source.runtime()?.verify_mint_authority_checkpoint(
            self.source.record()?.release_id,
            &self.current,
            &self.checkpoint,
        )
    }
}
fn require_adjacent_epoch(
    current: &KagemushaMintFinalityEpochAuthorizationV1,
    next: &KagemushaMintFinalityEpochAuthorizationV1,
    target: &KagemushaMintFinalityEpochAuthorizationV1,
) -> Result<(), String> {
    next.validate_successor(current)
        .map_err(|e| e.to_string())?;
    target.validate().map_err(|e| e.to_string())?;
    if next.epoch > target.epoch || (next.epoch == target.epoch && next != target) {
        return Err("ordinary checkpoint cannot substitute the authentic target epoch".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::isi::kagemusha_v1::{
        BeaconEpochBindingV1, InstalledBeaconEpochBindingV1,
    };
    fn bodies() -> (
        KagemushaMintFinalityEpochAuthorizationV1,
        KagemushaMintFinalityEpochAuthorizationV1,
    ) {
        let f = iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture::new();
        let epoch = genesis_epoch(f.genesis()).unwrap();
        let genesis =
            KagemushaMintFinalityEpochAuthorizationV1::genesis(&epoch.authority, 10).unwrap();
        // Public shape-only successor; it is never wrapped in a checkpoint or source capability.
        let mut next = genesis;
        next.epoch = 1;
        next.first_height = 11;
        next.last_height = 20;
        next.previous_authorization_id = genesis.authorization_id().unwrap();
        next.beacon = BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
            session_id: [8; 32],
            transcript_hash: [9; 32],
        });
        next.decision = KagemushaMintFinalityEpochDecisionV1::Retain;
        (genesis, next)
    }
    #[test]
    fn ordinary_checkpoint_retention_requires_exact_consecutive_incumbent_and_target() {
        let (current, next) = bodies();
        require_adjacent_epoch(&current, &next, &next).unwrap();
        for mutation in 0..5 {
            let mut offered = next;
            match mutation {
                0 => offered.epoch += 1,
                1 => offered.first_height += 1,
                2 => offered.previous_authorization_id = [7; 32],
                3 => offered.authority_id = [6; 32],
                _ => offered.beacon = BeaconEpochBindingV1::Bootstrap,
            }
            assert!(require_adjacent_epoch(&current, &offered, &next).is_err());
        }
        let mut wrong_target = next;
        wrong_target.last_height += 1;
        assert!(require_adjacent_epoch(&current, &next, &wrong_target).is_err());
    }
    #[test]
    fn ordinary_checkpoint_cancellation_is_preserved_and_never_relabelled_genesis() {
        let (current, mut next) = bodies();
        next.decision = KagemushaMintFinalityEpochDecisionV1::RetainAndCancel;
        next.transition_id = [10; 32];
        require_adjacent_epoch(&current, &next, &next).unwrap();
        let mut offered = next;
        offered.decision = KagemushaMintFinalityEpochDecisionV1::Genesis;
        assert!(require_adjacent_epoch(&current, &offered, &next).is_err());
        offered = next;
        offered.authority_generation += 1;
        assert!(require_adjacent_epoch(&current, &offered, &next).is_err());
    }
}
