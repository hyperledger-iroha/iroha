//! Owned public originals for an ordinary bootstrap State consumer.
//!
//! All proof/protocol/fold material is copied once before lending. It carries no financial
//! secret, platform key, clock or verifier callback. The Native State prover reconstructs
//! the financial relation and verifies the complete actual result under its signed release.

use super::super::super::ordinary_guard_verifier::OrdinaryGuardProofWireV1;
use super::production_ordinary_state::{
    KagemushaOrdinaryBootstrapAuxiliaryConsumerV1, KagemushaOrdinaryBootstrapAuxiliaryProofSourceV1,
};
use super::*;
use crate::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1;

struct IncomingEq {
    instances: Vec<Vec<Fp>>,
    proof: Vec<u8>,
    history: KagemushaEqAccumulatorV1,
    history_fold_proof: KagemushaEqFoldProofV1,
    merge_fold_proof: KagemushaEqFoldProofV1,
}
impl IncomingEq {
    fn retain(w: KagemushaRecursiveIncomingEqGenerationWitnessV1<'_>) -> Self {
        Self {
            instances: w.instances.to_vec(),
            proof: w.proof.to_vec(),
            history: w.history.clone(),
            history_fold_proof: w.history_fold_proof.clone(),
            merge_fold_proof: w.merge_fold_proof.clone(),
        }
    }
    fn borrow_eq(&self) -> KagemushaRecursiveIncomingEqGenerationWitnessV1<'_> {
        KagemushaRecursiveIncomingEqGenerationWitnessV1 {
            instances: &self.instances,
            proof: &self.proof,
            history: &self.history,
            history_fold_proof: &self.history_fold_proof,
            merge_fold_proof: &self.merge_fold_proof,
        }
    }
}
struct IncomingEp {
    instances: Vec<Vec<Fq>>,
    proof: Vec<u8>,
    history: KagemushaEpAccumulatorV1,
    history_fold_proof: KagemushaEpFoldProofV1,
    merge_fold_proof: KagemushaEpFoldProofV1,
}
impl IncomingEp {
    fn retain(w: KagemushaRecursiveIncomingEpGenerationWitnessV1<'_>) -> Self {
        Self {
            instances: w.instances.to_vec(),
            proof: w.proof.to_vec(),
            history: w.history.clone(),
            history_fold_proof: w.history_fold_proof.clone(),
            merge_fold_proof: w.merge_fold_proof.clone(),
        }
    }
    fn borrow_ep(&self) -> KagemushaRecursiveIncomingEpGenerationWitnessV1<'_> {
        KagemushaRecursiveIncomingEpGenerationWitnessV1 {
            instances: &self.instances,
            proof: &self.proof,
            history: &self.history,
            history_fold_proof: &self.history_fold_proof,
            merge_fold_proof: &self.merge_fold_proof,
        }
    }
}

/// Immutable public bootstrap proof originals, retained by the actual Native installer.
/// Construction grants no proof admission or financial authority. Callers obtain the opaque
/// SHA artifacts from the authenticated release loader and supply original valid padding proofs;
/// all actual protocols and proofs are consumed by the Native circuit, never a callback verifier.
pub struct KagemushaRetainedOrdinaryBootstrapAuxiliariesV1 {
    paired_guard_original: Vec<u8>,
    eq_hash: KagemushaLoadedEqMintHashArtifactsV1,
    ep_hash: KagemushaLoadedEpMintHashArtifactsV1,
    eq_incoming: IncomingEq,
    ep_incoming: IncomingEp,
    state: KagemushaStateRelationWitnessV1,
    mint_authorization: KagemushaMintAuthorizationV1,
    mint_credit: KagemushaMintCreditV1,
    guard_relation: KagemushaGuardBundleRelationWitnessV1,
    eq_parent_protocol: PlonkProtocol<EqAffine>,
    ep_parent_protocol: PlonkProtocol<EpAffine>,
    eq_parent_instances: Vec<Vec<Fp>>,
    ep_parent_instances: Vec<Vec<Fq>>,
    eq_parent_proof: Vec<u8>,
    ep_parent_proof: Vec<u8>,
    eq_predecessor_history: KagemushaEqAccumulatorV1,
    ep_predecessor_history: KagemushaEpAccumulatorV1,
    eq_parent_fold_proof: KagemushaEqFoldProofV1,
    ep_parent_fold_proof: KagemushaEpFoldProofV1,
    eq_incoming_protocol: PlonkProtocol<EqAffine>,
    ep_incoming_protocol: PlonkProtocol<EpAffine>,
    eq_successor_history: KagemushaEqAccumulatorV1,
    ep_successor_history: KagemushaEpAccumulatorV1,
    eq_guard_protocol: PlonkProtocol<EqAffine>,
    ep_guard_protocol: PlonkProtocol<EpAffine>,
    eq_guard_proof: Vec<u8>,
    ep_guard_proof: Vec<u8>,
    eq_guard_history: KagemushaEqAccumulatorV1,
    ep_guard_history: KagemushaEpAccumulatorV1,
    eq_guard_history_fold_proof: KagemushaEqFoldProofV1,
    ep_guard_history_fold_proof: KagemushaEpFoldProofV1,
    eq_guard_merge_fold_proof: KagemushaEqFoldProofV1,
    ep_guard_merge_fold_proof: KagemushaEpFoldProofV1,
    eq_mint_authorization_protocol: PlonkProtocol<EqAffine>,
    ep_mint_authorization_protocol: PlonkProtocol<EpAffine>,
    eq_mint_authorization_instances: Vec<Vec<Fp>>,
    ep_mint_authorization_instances: Vec<Vec<Fq>>,
    eq_mint_authorization_proof: Vec<u8>,
    ep_mint_authorization_proof: Vec<u8>,
    eq_mint_authorization_history: KagemushaEqAccumulatorV1,
    ep_mint_authorization_history: KagemushaEpAccumulatorV1,
    eq_mint_authorization_history_fold_proof: KagemushaEqFoldProofV1,
    ep_mint_authorization_history_fold_proof: KagemushaEpFoldProofV1,
    eq_mint_authorization_merge_fold_proof: KagemushaEqFoldProofV1,
    ep_mint_authorization_merge_fold_proof: KagemushaEpFoldProofV1,
    eq_mint_protocol: PlonkProtocol<EqAffine>,
    ep_mint_protocol: PlonkProtocol<EpAffine>,
    eq_mint_instances: Vec<Vec<Fp>>,
    ep_mint_instances: Vec<Vec<Fq>>,
    eq_mint_proof: Vec<u8>,
    ep_mint_proof: Vec<u8>,
    eq_mint_history: KagemushaEqAccumulatorV1,
    ep_mint_history: KagemushaEpAccumulatorV1,
    eq_mint_history_fold_proof: KagemushaEqFoldProofV1,
    ep_mint_history_fold_proof: KagemushaEpFoldProofV1,
    eq_mint_merge_fold_proof: KagemushaEqFoldProofV1,
    ep_mint_merge_fold_proof: KagemushaEpFoldProofV1,
}
impl KagemushaRetainedOrdinaryBootstrapAuxiliariesV1 {
    /// Retain the exact public bootstrap originals with zero financial placeholders.
    /// A caller cannot pass a private/OEM witness, financial seed, clock or approval callback.
    /// Genuine Native proving independently replaces and verifies the private financial relation.
    /// # Errors
    /// Rejects a non-bootstrap, private or already-hashed witness, or substituted Guard originals.
    pub fn retain_public_bootstrap_originals(
        witness: KagemushaRecursiveStateGenerationWitnessV1<'_>,
        paired_guard_original: Vec<u8>,
        eq_hash: KagemushaLoadedEqMintHashArtifactsV1,
        ep_hash: KagemushaLoadedEpMintHashArtifactsV1,
    ) -> Result<Self, String> {
        if witness.hash_claim.is_some()
            || witness.mint_fold_opening.is_some()
            || witness.hardware_selection.is_some()
            || witness.ordinary_selection.is_some()
            || witness.state.operation != KagemushaOperationV1::Bootstrap
            || witness.state.predecessor.is_some()
            || witness.guard_relation.predecessor_device_authority_secret != [0; 32]
            || witness.guard_relation.successor_device_authority_secret != [0; 32]
            || paired_guard_original.is_empty()
            || paired_guard_original.len()
                > crate::kagemusha_v1_state::KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1
        {
            return Err(
                "ordinary public auxiliary input contains a private or foreign operation".into(),
            );
        }
        let guard: OrdinaryGuardProofWireV1 =
            norito::decode_canonical(&paired_guard_original).map_err(|e| e.to_string())?;
        if norito::encode_canonical(&guard).map_err(|e| e.to_string())? != paired_guard_original
            || guard.eq_proof != witness.eq_guard_proof
            || guard.ep_proof != witness.ep_guard_proof
            || &guard.eq_history != witness.eq_guard_history.as_bytes()
            || &guard.ep_history != witness.ep_guard_history.as_bytes()
            || guard.eq_protocol_digest != witness.state.guard_eq_protocol_digest
            || guard.ep_protocol_digest != witness.state.guard_ep_protocol_digest
        {
            return Err("ordinary public auxiliary Guard original differs".into());
        }
        witness.state.validate()?;
        Ok(Self {
            paired_guard_original,
            eq_hash,
            ep_hash,
            eq_incoming: IncomingEq::retain(witness.eq_incoming_credits[0]),
            ep_incoming: IncomingEp::retain(witness.ep_incoming_credits[0]),
            state: witness.state.clone(),
            mint_authorization: witness.mint_authorization.clone(),
            mint_credit: witness.mint_credit.clone(),
            guard_relation: witness.guard_relation.clone(),
            eq_parent_protocol: witness.eq_parent_protocol.clone(),
            ep_parent_protocol: witness.ep_parent_protocol.clone(),
            eq_parent_instances: witness.eq_parent_instances.to_vec(),
            ep_parent_instances: witness.ep_parent_instances.to_vec(),
            eq_parent_proof: witness.eq_parent_proof.to_vec(),
            ep_parent_proof: witness.ep_parent_proof.to_vec(),
            eq_predecessor_history: witness.eq_predecessor_history.clone(),
            ep_predecessor_history: witness.ep_predecessor_history.clone(),
            eq_parent_fold_proof: witness.eq_parent_fold_proof.clone(),
            ep_parent_fold_proof: witness.ep_parent_fold_proof.clone(),
            eq_incoming_protocol: witness.eq_incoming_protocol.clone(),
            ep_incoming_protocol: witness.ep_incoming_protocol.clone(),
            eq_successor_history: witness.eq_successor_history.clone(),
            ep_successor_history: witness.ep_successor_history.clone(),
            eq_guard_protocol: witness.eq_guard_protocol.clone(),
            ep_guard_protocol: witness.ep_guard_protocol.clone(),
            eq_guard_proof: witness.eq_guard_proof.to_vec(),
            ep_guard_proof: witness.ep_guard_proof.to_vec(),
            eq_guard_history: witness.eq_guard_history.clone(),
            ep_guard_history: witness.ep_guard_history.clone(),
            eq_guard_history_fold_proof: witness.eq_guard_history_fold_proof.clone(),
            ep_guard_history_fold_proof: witness.ep_guard_history_fold_proof.clone(),
            eq_guard_merge_fold_proof: witness.eq_guard_merge_fold_proof.clone(),
            ep_guard_merge_fold_proof: witness.ep_guard_merge_fold_proof.clone(),
            eq_mint_authorization_protocol: witness.eq_mint_authorization_protocol.clone(),
            ep_mint_authorization_protocol: witness.ep_mint_authorization_protocol.clone(),
            eq_mint_authorization_instances: witness.eq_mint_authorization_instances.to_vec(),
            ep_mint_authorization_instances: witness.ep_mint_authorization_instances.to_vec(),
            eq_mint_authorization_proof: witness.eq_mint_authorization_proof.to_vec(),
            ep_mint_authorization_proof: witness.ep_mint_authorization_proof.to_vec(),
            eq_mint_authorization_history: witness.eq_mint_authorization_history.clone(),
            ep_mint_authorization_history: witness.ep_mint_authorization_history.clone(),
            eq_mint_authorization_history_fold_proof: witness
                .eq_mint_authorization_history_fold_proof
                .clone(),
            ep_mint_authorization_history_fold_proof: witness
                .ep_mint_authorization_history_fold_proof
                .clone(),
            eq_mint_authorization_merge_fold_proof: witness
                .eq_mint_authorization_merge_fold_proof
                .clone(),
            ep_mint_authorization_merge_fold_proof: witness
                .ep_mint_authorization_merge_fold_proof
                .clone(),
            eq_mint_protocol: witness.eq_mint_protocol.clone(),
            ep_mint_protocol: witness.ep_mint_protocol.clone(),
            eq_mint_instances: witness.eq_mint_instances.to_vec(),
            ep_mint_instances: witness.ep_mint_instances.to_vec(),
            eq_mint_proof: witness.eq_mint_proof.to_vec(),
            ep_mint_proof: witness.ep_mint_proof.to_vec(),
            eq_mint_history: witness.eq_mint_history.clone(),
            ep_mint_history: witness.ep_mint_history.clone(),
            eq_mint_history_fold_proof: witness.eq_mint_history_fold_proof.clone(),
            ep_mint_history_fold_proof: witness.ep_mint_history_fold_proof.clone(),
            eq_mint_merge_fold_proof: witness.eq_mint_merge_fold_proof.clone(),
            ep_mint_merge_fold_proof: witness.ep_mint_merge_fold_proof.clone(),
        })
    }

    fn lend<'a>(
        &'a self,
        hash_witness: Option<KagemushaMintHashClaimGenerationWitnessV1<'a>>,
        eq_final_history: &'a KagemushaEqAccumulatorV1,
        ep_final_history: &'a KagemushaEpAccumulatorV1,
    ) -> KagemushaRecursiveStateGenerationWitnessV1<'a> {
        KagemushaRecursiveStateGenerationWitnessV1 {
            hash_claim: hash_witness,
            state: self.state.clone(),
            mint_fold_opening: None,
            mint_authorization: &self.mint_authorization,
            mint_credit: &self.mint_credit,
            guard_relation: self.guard_relation.clone(),
            hardware_selection: None,
            ordinary_selection: None,
            eq_parent_protocol: &self.eq_parent_protocol,
            ep_parent_protocol: &self.ep_parent_protocol,
            eq_parent_instances: &self.eq_parent_instances,
            ep_parent_instances: &self.ep_parent_instances,
            eq_parent_proof: &self.eq_parent_proof,
            ep_parent_proof: &self.ep_parent_proof,
            eq_predecessor_history: &self.eq_predecessor_history,
            ep_predecessor_history: &self.ep_predecessor_history,
            eq_parent_fold_proof: &self.eq_parent_fold_proof,
            ep_parent_fold_proof: &self.ep_parent_fold_proof,
            eq_incoming_protocol: &self.eq_incoming_protocol,
            ep_incoming_protocol: &self.ep_incoming_protocol,
            eq_incoming_credits: [self.eq_incoming.borrow_eq()],
            ep_incoming_credits: [self.ep_incoming.borrow_ep()],
            eq_successor_history: eq_final_history,
            ep_successor_history: ep_final_history,
            eq_guard_protocol: &self.eq_guard_protocol,
            ep_guard_protocol: &self.ep_guard_protocol,
            eq_guard_proof: &self.eq_guard_proof,
            ep_guard_proof: &self.ep_guard_proof,
            eq_guard_history: &self.eq_guard_history,
            ep_guard_history: &self.ep_guard_history,
            eq_guard_history_fold_proof: &self.eq_guard_history_fold_proof,
            ep_guard_history_fold_proof: &self.ep_guard_history_fold_proof,
            eq_guard_merge_fold_proof: &self.eq_guard_merge_fold_proof,
            ep_guard_merge_fold_proof: &self.ep_guard_merge_fold_proof,
            eq_mint_authorization_protocol: &self.eq_mint_authorization_protocol,
            ep_mint_authorization_protocol: &self.ep_mint_authorization_protocol,
            eq_mint_authorization_instances: &self.eq_mint_authorization_instances,
            ep_mint_authorization_instances: &self.ep_mint_authorization_instances,
            eq_mint_authorization_proof: &self.eq_mint_authorization_proof,
            ep_mint_authorization_proof: &self.ep_mint_authorization_proof,
            eq_mint_authorization_history: &self.eq_mint_authorization_history,
            ep_mint_authorization_history: &self.ep_mint_authorization_history,
            eq_mint_authorization_history_fold_proof: &self
                .eq_mint_authorization_history_fold_proof,
            ep_mint_authorization_history_fold_proof: &self
                .ep_mint_authorization_history_fold_proof,
            eq_mint_authorization_merge_fold_proof: &self.eq_mint_authorization_merge_fold_proof,
            ep_mint_authorization_merge_fold_proof: &self.ep_mint_authorization_merge_fold_proof,
            eq_mint_protocol: &self.eq_mint_protocol,
            ep_mint_protocol: &self.ep_mint_protocol,
            eq_mint_instances: &self.eq_mint_instances,
            ep_mint_instances: &self.ep_mint_instances,
            eq_mint_proof: &self.eq_mint_proof,
            ep_mint_proof: &self.ep_mint_proof,
            eq_mint_history: &self.eq_mint_history,
            ep_mint_history: &self.ep_mint_history,
            eq_mint_history_fold_proof: &self.eq_mint_history_fold_proof,
            ep_mint_history_fold_proof: &self.ep_mint_history_fold_proof,
            eq_mint_merge_fold_proof: &self.eq_mint_merge_fold_proof,
            ep_mint_merge_fold_proof: &self.ep_mint_merge_fold_proof,
        }
    }
}
impl KagemushaOrdinaryBootstrapAuxiliaryProofSourceV1
    for KagemushaRetainedOrdinaryBootstrapAuxiliariesV1
{
    fn with_borrowed_bootstrap_auxiliaries(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        paired_guard_original: &[u8],
        hash_claim: Option<&KagemushaGeneratedMintHashClaimV1>,
        consume: &mut KagemushaOrdinaryBootstrapAuxiliaryConsumerV1<'_>,
    ) -> Result<(), String> {
        if self.paired_guard_original != paired_guard_original
            || self.state.successor != selection.preview().map_err(|e| e.to_string())?.state
        {
            return Err(
                "retained ordinary bootstrap originals differ from Native selection".into(),
            );
        }
        let Some(claim) = hash_claim else {
            return consume(self.lend(
                None,
                &self.eq_successor_history,
                &self.ep_successor_history,
            ));
        };
        // These merge witnesses contain only public accumulator equations. A stable public
        // transcript seed reproduces the same original fold on recovery; it cannot open the
        // financial commitment or replace the private State seed retained by Native custody.
        let mut transcript = Sha256::new();
        transcript.update(b"iroha:kagemusha:v1:ordinary-bootstrap-public-history-fold\0");
        transcript.update(Sha256::digest(&self.paired_guard_original));
        transcript.update(Sha256::digest(&claim.eq_proof));
        transcript.update(Sha256::digest(&claim.ep_proof));
        transcript.update(self.eq_successor_history.as_bytes());
        transcript.update(self.ep_successor_history.as_bytes());
        transcript.update(claim.eq_complete_history.as_bytes());
        transcript.update(claim.ep_complete_history.as_bytes());
        let public_fold_seed = KagemushaRecoverySeedV1::from_unsealed(transcript.finalize().into())
            .map_err(|e| e.to_string())?;
        let eq_merge = fold_kagemusha_eq_accumulators_v1(
            &self.eq_hash.carrier_parameters,
            &self.eq_successor_history,
            &claim.eq_complete_history,
            &public_fold_seed,
        )
        .map_err(|e| e.to_string())?;
        let ep_merge = fold_kagemusha_ep_accumulators_v1(
            &self.ep_hash.carrier_parameters,
            &self.ep_successor_history,
            &claim.ep_complete_history,
            &public_fold_seed,
        )
        .map_err(|e| e.to_string())?;
        let hash = claim
            .consumer_witness(
                &self.eq_hash,
                &self.ep_hash,
                eq_merge.proof(),
                ep_merge.proof(),
            )
            .map_err(|e| e.to_string())?;
        consume(self.lend(Some(hash), eq_merge.successor(), ep_merge.successor()))
    }
}
