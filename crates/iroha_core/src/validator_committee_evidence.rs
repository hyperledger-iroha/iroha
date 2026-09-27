//! Offline verification of incumbent-authorized pending committee custody.
//!
//! The context pin is supplied independently. A lifecycle certificate authorizes the exact
//! transcript but does not prove transaction inclusion or activate its target committee.

use crate::{
    beacon::FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    state::{
        World,
        validator_committee::{verify_candidate, verify_progress},
        verify_threshold_key_lifecycle_certificate_v1,
    },
};
use iroha_data_model::{
    NetworkId,
    block::consensus_v2::HeightContextId,
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    consensus::GlobalThresholdBeaconKeySessionV1,
    isi::{
        consensus_keys::{ThresholdKeyLifecycleActionV1, ThresholdKeyLifecycleCertificateV1},
        kagemusha_v1::{
            BeaconEpochBindingV1, InstalledBeaconEpochBindingV1,
            KagemushaMintFinalityAuthorityGenerationV1,
        },
    },
    nexus::{
        ValidatorCandidateKeysV1, ValidatorCommitteePreparationV1, ValidatorCommitteeStatusV1,
        ValidatorCommitteeTransitionV1,
    },
};
use norito::{NoritoDeserialize, NoritoSerialize, codec::Encode as _};

/// Maximum encoded public evidence size, independent of caller transport limits.
pub const COMMITTEE_PROVISIONING_EVIDENCE_MAX_BYTES_V1: usize = 64 * 1024 * 1024;
/// Maximum contiguous finality artifacts admitted in one offline operation.
pub const COMMITTEE_PROVISIONING_FINALITY_MAX_COUNT_V1: usize = 65_536;

/// Finalized selection proof used before any target DKG transcript exists.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_core::validator_committee_evidence::ValidatorCommitteeSelectionEvidenceV1"
)]
#[norito(deny_unknown_fields)]
pub struct ValidatorCommitteeSelectionEvidenceV1 {
    /// Selection observation whose immutable preparation must match finality.
    pub status: ValidatorCommitteeStatusV1,
    /// Contiguous canonical finality from an independently pinned context.
    pub finality_chain: Vec<BridgeFinalityProof>,
}

/// Authority to run the exact selected DKG, without credential or activation authority.
pub struct VerifiedValidatorCommitteeSelectionV1 {
    preparation: ValidatorCommitteePreparationV1,
    incumbent_authority: KagemushaMintFinalityAuthorityGenerationV1,
    incumbent_beacon: InstalledBeaconEpochBindingV1,
    observed_height: u64,
}
impl VerifiedValidatorCommitteeSelectionV1 {
    /// Immutable target, generation and DKG session binding certified at selection.
    pub fn preparation(&self) -> &ValidatorCommitteePreparationV1 {
        &self.preparation
    }
    /// Current authority whose exact quorum owns the preparation epoch.
    pub fn incumbent_authority(&self) -> &KagemushaMintFinalityAuthorityGenerationV1 {
        &self.incumbent_authority
    }
    /// Current installed beacon session, never replaced by this selection proof.
    pub fn incumbent_beacon(&self) -> InstalledBeaconEpochBindingV1 {
        self.incumbent_beacon
    }
    /// Latest authenticated height before the preparation cutoff.
    pub fn observed_height(&self) -> u64 {
        self.observed_height
    }
}

/// Verify a frozen E+2 roster from independently anchored finality during E+1.
///
/// The result only authorizes private DKG preparation for the immutable target.
/// It neither authenticates unfinalized progress fields nor installs credentials.
///
/// # Errors
/// Rejects foreign networks or attempts, non-contiguous finality, changed
/// election inputs, invalid candidate BLS possession, and expired preparation.
pub fn verify_validator_committee_selection_evidence_v1(
    evidence: &ValidatorCommitteeSelectionEvidenceV1,
    network: NetworkId,
    trusted_context: HeightContextId,
    anchor_height: u64,
    target_epoch: u64,
    transition_id: [u8; 32],
) -> Result<VerifiedValidatorCommitteeSelectionV1, String> {
    let status = &evidence.status;
    let selected = status
        .selected
        .as_ref()
        .ok_or("selection evidence lacks a frozen committee")?;
    let preparation = &selected.transition.preparation;
    preparation.validate()?;
    if anchor_height == 0
        || transition_id == [0; 32]
        || status.network_id != network
        || preparation.network_id != network
        || status.target_epoch != target_epoch
        || preparation.target_epoch != target_epoch
        || preparation.transition_id()? != transition_id
        || evidence.finality_chain.is_empty()
        || evidence.finality_chain.len() > COMMITTEE_PROVISIONING_FINALITY_MAX_COUNT_V1
    {
        return Err("selection evidence differs from its exact network, attempt or bounds".into());
    }
    let mut size = status.encode().len();
    let mut verifier = BridgeFinalityVerifier::with_context(network, trusted_context);
    let mut selecting = None;
    for (index, proof) in evidence.finality_chain.iter().enumerate() {
        size = size
            .checked_add(proof.encode().len())
            .ok_or("selection evidence length overflows")?;
        let height = anchor_height
            .checked_add(u64::try_from(index).map_err(|_| "selection height overflows")?)
            .ok_or("selection height overflows")?;
        if size > COMMITTEE_PROVISIONING_EVIDENCE_MAX_BYTES_V1
            || proof.block_header.height().get() != height
        {
            return Err("selection finality is not bounded and contiguous from its anchor".into());
        }
        verifier.verify(proof).map_err(|error| error.to_string())?;
        if height == preparation.selection_height {
            selecting = Some(&proof.finality_artifact);
        }
    }
    let latest = &evidence
        .finality_chain
        .last()
        .ok_or("missing finality")?
        .finality_artifact;
    if latest != &status.latest_finality
        || selecting != Some(&selected.selecting_finality)
        || latest.height <= preparation.selection_height
        || latest.height >= preparation.first_height.saturating_sub(1)
        || latest.height_context.epoch != preparation.selection_epoch + 1
        || selected.selecting_finality.height_context.epoch != preparation.selection_epoch
        || selected.selecting_finality.subject.parent_block_hash
            != Some(preparation.selection_anchor)
    {
        return Err("selection observations differ from anchored E+1 finality".into());
    }
    let snapshot = selected
        .selecting_finality
        .height_context
        .next_epoch_snapshot
        .as_ref()
        .ok_or("selecting finality lacks its committee snapshot")?;
    if snapshot.committee_preparation.as_ref() != Some(preparation)
        || snapshot.epoch != preparation.selection_epoch + 1
        || snapshot.roster != latest.height_context.roster
        || snapshot.kagemusha_mint_finality_authority
            != latest.height_context.kagemusha_mint_finality_authority
        || snapshot.kagemusha_mint_finality_authorization
            != latest.height_context.kagemusha_mint_finality_authorization
    {
        return Err("selected committee or preparing authority changed after finality".into());
    }
    let incumbent = &latest.height_context.kagemusha_mint_finality_authority;
    if incumbent.validators.len() != latest.height_context.roster.len()
        || incumbent
            .validators
            .iter()
            .zip(&latest.height_context.roster)
            .any(|(keys, seat)| keys.validator != seat.validator)
    {
        return Err("preparing authority differs from the exact incumbent voter roster".into());
    }
    preparation.validate_against_preparing_authorization(
        &snapshot.kagemusha_mint_finality_authorization,
    )?;
    for (seat, pop) in preparation
        .roster
        .iter()
        .zip(&preparation.validator_set_pops)
    {
        iroha_crypto::bls_normal_pop_verify(seat.validator.public_key(), pop)
            .map_err(|_| "frozen target has invalid BLS key possession")?;
    }
    let BeaconEpochBindingV1::Installed(incumbent_beacon) = latest
        .height_context
        .kagemusha_mint_finality_authorization
        .beacon
    else {
        return Err("selection DKG requires an installed incumbent beacon".into());
    };
    Ok(VerifiedValidatorCommitteeSelectionV1 {
        preparation: preparation.clone(),
        incumbent_authority: incumbent.clone(),
        incumbent_beacon,
        observed_height: latest.height,
    })
}

/// Public custody evidence; its claimed context never supplies the verifier's trust root.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_core::validator_committee_evidence::ValidatorCommitteeProvisioningEvidenceV1"
)]
#[norito(deny_unknown_fields)]
pub struct ValidatorCommitteeProvisioningEvidenceV1 {
    /// Exact selected committee and cryptographically checkable progress observations.
    pub status: ValidatorCommitteeStatusV1,
    /// Contiguous canonical finality from the independent anchor through the observed tip.
    pub finality_chain: Vec<BridgeFinalityProof>,
    /// Exact incumbent quorum authorization for the pending public beacon transcript.
    pub beacon_finalization: ThresholdKeyLifecycleCertificateV1,
}

/// Verified authorization to prepare custody, with no committee activation authority.
pub struct VerifiedValidatorCommitteeProvisioningV1 {
    transition: ValidatorCommitteeTransitionV1,
    session: GlobalThresholdBeaconKeySessionV1,
    incumbent_authority: KagemushaMintFinalityAuthorityGenerationV1,
    incumbent_beacon: InstalledBeaconEpochBindingV1,
    observed_height: u64,
}
impl VerifiedValidatorCommitteeProvisioningV1 {
    /// Exact immutable attempt whose pending secrets may be provisioned.
    pub fn transition(&self) -> &ValidatorCommitteeTransitionV1 {
        &self.transition
    }
    /// Complete authorized target transcript for importing the private share.
    pub fn session(&self) -> &GlobalThresholdBeaconKeySessionV1 {
        &self.session
    }
    /// Current authenticated authority; incumbent members must retain their current share.
    pub fn incumbent_authority(&self) -> &KagemushaMintFinalityAuthorityGenerationV1 {
        &self.incumbent_authority
    }
    /// Exact active session which pending provisioning may never replace.
    pub fn incumbent_beacon(&self) -> InstalledBeaconEpochBindingV1 {
        self.incumbent_beacon
    }
    /// Latest height in the independently authenticated supplied chain.
    pub fn observed_height(&self) -> u64 {
        self.observed_height
    }
}

/// Verify a bounded public custody envelope from an independently supplied context pin.
///
/// # Errors
/// Rejects changed network/attempt/chain, expired preparation, substituted keys/transcript,
/// invalid actual-possession proofs, or missing exact incumbent quorum authorization.
pub fn verify_validator_committee_provisioning_evidence_v1(
    evidence: &ValidatorCommitteeProvisioningEvidenceV1,
    network: NetworkId,
    trusted_context: HeightContextId,
    anchor_height: u64,
    target_epoch: u64,
    transition_id: [u8; 32],
) -> Result<VerifiedValidatorCommitteeProvisioningV1, String> {
    let status = &evidence.status;
    let selected = status
        .selected
        .as_ref()
        .ok_or("custody evidence lacks a selected committee")?;
    let transition = &selected.transition;
    transition.validate()?;
    let preparation = &transition.preparation;
    if anchor_height == 0
        || transition_id == [0; 32]
        || status.network_id != network
        || preparation.network_id != network
        || status.target_epoch != target_epoch
        || preparation.target_epoch != target_epoch
        || preparation.transition_id()? != transition_id
        || transition.outcome.is_some()
        || transition.credentials.is_none()
        || evidence.finality_chain.is_empty()
        || evidence.finality_chain.len() > COMMITTEE_PROVISIONING_FINALITY_MAX_COUNT_V1
        || status.candidate_keys.len() != preparation.roster.len()
    {
        return Err(
            "custody evidence differs from its exact network, pending attempt or bounds".to_owned(),
        );
    }
    let mut size = status
        .encode()
        .len()
        .checked_add(evidence.beacon_finalization.encode().len())
        .ok_or("custody evidence length overflows")?;
    let mut verifier = BridgeFinalityVerifier::with_context(network, trusted_context);
    let mut selecting = None;
    let mut finalizing = None;
    for (index, proof) in evidence.finality_chain.iter().enumerate() {
        size = size
            .checked_add(proof.encode().len())
            .ok_or("custody evidence length overflows")?;
        let height = anchor_height
            .checked_add(u64::try_from(index).map_err(|_| "finality count overflows")?)
            .ok_or("finality height overflows")?;
        if size > COMMITTEE_PROVISIONING_EVIDENCE_MAX_BYTES_V1
            || proof.block_header.height().get() != height
        {
            return Err(
                "custody finality is not bounded and contiguous from its independent anchor"
                    .to_owned(),
            );
        }
        verifier.verify(proof).map_err(|error| error.to_string())?;
        if height == preparation.selection_height {
            selecting = Some(&proof.finality_artifact);
        }
        if height == evidence.beacon_finalization.effective_height {
            finalizing = Some(&proof.finality_artifact);
        }
    }
    let latest = &evidence
        .finality_chain
        .last()
        .ok_or("missing finality")?
        .finality_artifact;
    if latest != &status.latest_finality
        || selecting != Some(&selected.selecting_finality)
        || latest.height >= preparation.first_height - 1
        || selected.selecting_finality.subject.parent_block_hash
            != Some(preparation.selection_anchor)
    {
        return Err("custody observations differ from anchored pending finality".to_owned());
    }
    let selecting_snapshot = selected
        .selecting_finality
        .height_context
        .next_epoch_snapshot
        .as_ref()
        .ok_or("selecting finality lacks its committee snapshot")?;
    if selecting_snapshot.committee_preparation.as_ref() != Some(preparation) {
        return Err("selecting boundary did not certify this exact preparation".to_owned());
    }
    let (incumbent_authority, current) = match &latest.height_context.next_epoch_snapshot {
        Some(snapshot) => (
            &snapshot.kagemusha_mint_finality_authority,
            &snapshot.kagemusha_mint_finality_authorization,
        ),
        None => (
            &latest.height_context.kagemusha_mint_finality_authority,
            &latest.height_context.kagemusha_mint_finality_authorization,
        ),
    };
    preparation.validate_against_preparing_authorization(current)?;
    preparation.validate_against_preparing_authorization(
        &selecting_snapshot.kagemusha_mint_finality_authorization,
    )?;
    let BeaconEpochBindingV1::Installed(incumbent_beacon) = current.beacon else {
        return Err("pending custody requires an installed incumbent beacon".to_owned());
    };
    let finalizing = finalizing.ok_or("custody evidence lacks anchored finalization authority")?;
    let certificate = &evidence.beacon_finalization;
    if certificate.action != ThresholdKeyLifecycleActionV1::FinalizeGlobalBeaconKey
        || certificate.expected_active_session_id != Some(incumbent_beacon.session_id)
        || finalizing
            .height_context
            .kagemusha_mint_finality_authorization
            != *current
        || certificate.effective_height <= preparation.selection_height
        || certificate.effective_height >= preparation.first_height - 1
    {
        return Err("pending beacon certificate differs from its preparing authority".to_owned());
    }
    let peers = finalizing
        .height_context
        .roster
        .iter()
        .map(|seat| seat.validator.clone())
        .collect::<Vec<_>>();
    verify_threshold_key_lifecycle_certificate_v1(
        certificate,
        &network,
        certificate.effective_height,
        &peers,
    )
    .map_err(|error| error.to_string())?;
    let record: FinalizedGlobalThresholdBeaconKeySessionRecordV1 =
        norito::decode_canonical(&certificate.public_state)
            .map_err(|_| "pending beacon certificate public state is not canonical")?;
    record.validate().map_err(|error| error.to_string())?;
    if record.activated_at_height.is_some()
        || record.retired_at_height.is_some()
        || record.session.network_id != network
        || record.session.session_id != certificate.session_id
        || record.session.transcript_hash != certificate.transcript_hash
        || status.pending_beacon_session.as_ref() != Some(&record.session)
        || record.session.adaptive_dkg.finalized_at_height > certificate.effective_height
    {
        return Err("pending beacon transcript differs from its exact certificate".to_owned());
    }
    let mut world = World::new();
    for (candidate, seat) in status.candidate_keys.iter().zip(&preparation.roster) {
        if candidate.network_id != network
            || candidate.generation != preparation.authority_generation
            || candidate.keys.validator != seat.validator
        {
            return Err("candidate evidence differs from the frozen seat order".to_owned());
        }
        verify_candidate(candidate)?;
        world.validator_candidate_keys.insert(
            ValidatorCandidateKeysV1::key_id(
                network,
                candidate.generation,
                &candidate.keys.validator,
            ),
            candidate.clone(),
        );
    }
    let session = record.session.clone();
    world
        .global_beacon_key_sessions
        .insert(session.session_id, record);
    verify_progress(&world.view(), transition)?;
    Ok(VerifiedValidatorCommitteeProvisioningV1 {
        transition: transition.clone(),
        session,
        incumbent_authority: incumbent_authority.clone(),
        incumbent_beacon,
        observed_height: latest.height,
    })
}

#[cfg(test)]
#[path = "validator_committee_evidence/tests.rs"]
mod tests;
