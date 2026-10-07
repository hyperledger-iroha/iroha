//! Retained Bootstrap, Load and consuming originals for the installed native producers.
//!
//! These conversions do not authenticate a Q/A/W catalog or admit a wallet. Supplied
//! Q originals remain untrusted until the native plan verifies them; operation owners
//! then bind every signed tape and map to the same context before terminal admission.
//!
//! TODO: exercise these entry points through the installed wallet producer once its
//! complete authenticated catalog is available. The local path-conversion tests and
//! native producer proof tests establish separate component boundaries only.

use iroha_kagemusha_proof::{
    a_relation::native::{bootstrap, consuming, load},
    tree::IndexedInsert,
};

use super::*;

pub(super) fn no_map_openings(openings: &[Vec<u8>]) -> Result<(), Error> {
    if openings.is_empty() {
        Ok(())
    } else {
        Err(Error::Authority)
    }
}

pub(super) fn retained_insertion(
    openings: &[Vec<u8>],
) -> Result<KagemushaWalletIndexedInsertV1, Error> {
    let [low_original, empty_original] = openings else {
        return Err(Error::Authority);
    };
    let (Some(low), low_opening) = authority(KagemushaWalletIndexedOpeningV1::from_transcript(
        low_original,
    ))?
    else {
        return Err(Error::Authority);
    };
    let (None, slot_opening) = authority(KagemushaWalletIndexedOpeningV1::from_transcript(
        empty_original,
    ))?
    else {
        return Err(Error::Authority);
    };
    Ok(KagemushaWalletIndexedInsertV1 {
        low,
        low_opening,
        slot_opening,
    })
}

/// Decode only the retained paths for the statement's exact recovery-map effect.
/// Unload retains the low-leaf opening followed by the intermediate empty-slot
/// opening; Retiring has no recovery path and preserves the root.
fn consuming_recovery(
    effect: &KagemushaWalletEffectV1,
    openings: &[Vec<u8>],
    predecessor_root: &[u8; 32],
    successor_root: &[u8; 32],
) -> Result<Option<IndexedInsert<Fp>>, Error> {
    match effect {
        KagemushaWalletEffectV1::Unload {
            nullifier,
            redeem_ordinal,
            amount,
            online_charge,
            ..
        } => {
            let witness = retained_insertion(openings)?;
            let leaf = KagemushaWalletRedeemLeafV1 {
                ordinal: *redeem_ordinal,
                nullifier: *nullifier,
                amount: *amount,
                online_charge: *online_charge,
            };
            if authority(witness.verify(
                predecessor_root,
                &leaf.key(),
                &authority(leaf.leaf_value())?,
            ))? != *successor_root
            {
                return Err(Error::Authority);
            }
            super::monetary::insertion(&witness).map(Some)
        }
        KagemushaWalletEffectV1::Retiring => {
            no_map_openings(openings)?;
            if predecessor_root != successor_root {
                return Err(Error::Authority);
            }
            Ok(None)
        }
        _ => Err(Error::Authority),
    }
}

fn load_recovery(
    effect: &KagemushaWalletEffectV1,
    openings: &[Vec<u8>],
    predecessor_root: &[u8; 32],
    successor_root: &[u8; 32],
) -> Result<IndexedInsert<Fp>, Error> {
    let KagemushaWalletEffectV1::Load {
        receipt_digest,
        load_ordinal,
        amount,
        ..
    } = effect
    else {
        return Err(Error::Authority);
    };
    let leaf = KagemushaWalletLoadLeafV1 {
        ordinal: *load_ordinal,
        receipt_digest: *receipt_digest,
        amount: *amount,
    };
    let insertion = retained_insertion(openings)?;
    if authority(insertion.verify(
        predecessor_root,
        &leaf.key(),
        &authority(leaf.leaf_value())?,
    ))? != *successor_root
    {
        return Err(Error::Authority);
    }
    super::monetary::insertion(&insertion)
}

impl PreparationV1<'_> {
    /// Build complete native Load input from the retained ordinary receipt/finality
    /// and the capsule's authenticated low-leaf/empty-slot insertion. Original Qs
    /// and both predecessor claims are moved unchanged into the mandatory native
    /// plan, which verifies the installed finality source and all five A owners.
    /// This method grants no funding or catalog authority by itself.
    ///
    /// # Errors
    /// Changed receipt, anchor, source state, finality originals or recovery root;
    /// missing/reordered paths, occupied insertion slot, or another operation.
    #[allow(clippy::too_many_arguments)]
    pub fn load_native_inputs(
        &self,
        owner: &AuthenticatedCredentialV1,
        step: &ReleasedStep,
        predecessor: &FoldedStateV1,
        public: &KagemushaWalletLineagePublicV1,
        plan: &load::Plan,
        q: [load::QInput; 3],
        budget: MemoryBudget,
    ) -> Result<load::Inputs, Error> {
        let capsule = &step.frozen.capsule;
        let fields = self.load_fields(owner, step, predecessor, public, plan, budget)?;
        let insertion = load_recovery(
            &capsule.statement.effect,
            &capsule.map_openings,
            &predecessor.source_state.core.load_redeem_recovery_root,
            &capsule.successor_state.core.load_redeem_recovery_root,
        )?;
        Ok(load::Inputs {
            state: fields.state,
            sigma: fields.sigma,
            receipt: fields.receipt,
            objects: fields.objects,
            finality: fields.finality,
            insertion,
            q,
            predecessor: load::PredecessorInput {
                proof: predecessor.proof.clone(),
                pallas: predecessor.pallas,
                vesta: predecessor.vesta,
            },
        })
    }

    /// Build Bootstrap's native input from its authenticated released source and
    /// the exact two retained Q originals, without selecting any proving artifact.
    /// The installed native `bootstrap::Plan::prepare` and complete A/W chain must
    /// still verify the Q proofs and bind all three original signed tapes.
    ///
    /// # Errors
    /// Rejects another operation, extraneous map witnesses, invalid retained
    /// evidence, or a changed credential, state or installation.
    pub fn bootstrap_native_inputs(
        &self,
        owner: &AuthenticatedCredentialV1,
        step: &ReleasedStep,
        public: &KagemushaWalletLineagePublicV1,
        q: [bootstrap::QInput; 2],
        budget: MemoryBudget,
    ) -> Result<bootstrap::Inputs, Error> {
        no_map_openings(&step.frozen.capsule.map_openings)?;
        let fields = self.bootstrap_fields(owner, step, public, budget)?;
        Ok(bootstrap::Inputs {
            state: fields.state,
            sigma: fields.sigma,
            objects: fields.objects,
            q,
        })
    }

    /// Build Unload or Retiring's native input from the exact released source and
    /// fully verified predecessor. The recovery path is decoded from the capsule,
    /// never supplied separately, and Q originals are moved without alteration.
    /// The installed native `consuming::Plan::prepare` and complete A/W chain remain
    /// mandatory before accepting any result; this method grants no open authority.
    ///
    /// # Errors
    /// Rejects another operation, changed source/lineage/credential, wrong path
    /// count or order, a nonempty insertion slot, or either changed recovery root.
    pub fn consuming_native_inputs(
        &self,
        owner: &AuthenticatedCredentialV1,
        step: &ReleasedStep,
        predecessor: &FoldedStateV1,
        public: &KagemushaWalletLineagePublicV1,
        q: [consuming::QInput; 2],
        budget: MemoryBudget,
    ) -> Result<consuming::Inputs, Error> {
        let capsule = &step.frozen.capsule;
        let fields =
            self.consuming_fields(capsule.kind, owner, step, predecessor, public, budget)?;
        let recovery = consuming_recovery(
            &capsule.statement.effect,
            &capsule.map_openings,
            &predecessor.source_state.core.load_redeem_recovery_root,
            &capsule.successor_state.core.load_redeem_recovery_root,
        )?;
        Ok(consuming::Inputs {
            state: fields.state,
            sigma: fields.sigma,
            omega: predecessor.lineage.bytes(),
            objects: fields.objects,
            recovery,
            q,
            predecessor: consuming::PredecessorInput {
                proof: predecessor.proof.clone(),
                pallas: predecessor.pallas,
                vesta: predecessor.vesta,
            },
        })
    }
}

#[cfg(test)]
#[path = "native_inputs/tests.rs"]
mod tests;
