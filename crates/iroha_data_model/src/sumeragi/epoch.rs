//! Complete native validator epochs and the boundary results authorized by their incumbents.
//!
//! A generation owns signing keys; a scheduling authorization owns an epoch and its bounds.
//! These bodies contain no certificate and therefore cannot authenticate themselves.

use iroha_crypto::{Algorithm, Hash, HashOf};
use iroha_model_base::peer::PeerId;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize, NetworkId,
    block::BlockHeader,
    isi::kagemusha_v1::{
        KagemushaMintFinalityAuthorityGenerationV1, KagemushaMintFinalityEpochAuthorizationV1,
        KagemushaMintFinalityEpochDecisionV1,
    },
    nexus::ValidatorCommitteePreparationV1,
    parameter::system::ConsensusMode,
};

/// Largest first-release native voting committee.
pub const MAX_VALIDATORS: usize = 31;
const EPOCH_DOMAIN: &[u8] = b"iroha:native-validator-epoch:v1";
const SEAT_DOMAIN: &[u8] = b"iroha:validator-seat:v1";

/// One equal-vote BLS member and its original proof of possession.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi::epoch::ValidatorCommitteeMemberV1")]
pub struct ValidatorCommitteeMemberV1 {
    /// Canonical BLS-normal peer identity.
    pub validator: PeerId,
    /// Original 96-byte proof for this exact peer, retained across scheduling epochs.
    pub proof_of_possession: Vec<u8>,
}

/// Complete context whose identity every native signature binds.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi::epoch::ValidatorEpochContextV1")]
pub struct ValidatorEpochContextV1 {
    /// Sole first-release layout version, 1.
    pub version: u16,
    /// Exact genesis-derived network.
    pub network_id: NetworkId,
    /// Authenticated committee-selection policy.
    pub mode: ConsensusMode,
    /// Immutable generation of consensus and paired-Pasta keys.
    pub authority: KagemushaMintFinalityAuthorityGenerationV1,
    /// Exact scheduling epoch, bounds, predecessor, and installed beacon binding.
    pub authorization: KagemushaMintFinalityEpochAuthorizationV1,
    /// Canonically ordered, equal-vote committee and original BLS proofs.
    pub committee: Vec<ValidatorCommitteeMemberV1>,
    /// Fresh boundary-derived leader randomness; genesis uses its signed initial seed.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub leader_seed: [u8; 32],
}

impl ValidatorEpochContextV1 {
    /// Validate complete authority shape, real public keys, and every ordered BLS proof.
    ///
    /// This does not establish finality. The caller must authenticate signed genesis or the
    /// exact predecessor boundary's native certificate before accepting the context.
    ///
    /// # Errors
    /// Rejects identity, ordering, key, proof, geometry, seed, and epoch-bound mismatches.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.leader_seed == [0; 32] {
            return Err("invalid native epoch version or leader seed".into());
        }
        self.authorization
            .validate_against_authority(&self.authority)
            .map_err(|error| error.to_string())?;
        if self.network_id != self.authority.network_id {
            return Err("native epoch belongs to another network".into());
        }
        let length = self
            .authorization
            .last_height
            .checked_sub(self.authorization.first_height)
            .and_then(|length| length.checked_add(1))
            .ok_or("native epoch interval overflows")?;
        if self.mode == ConsensusMode::Npos && length < 3 {
            return Err("native NPoS epoch must leave a real preboundary pulse and parent".into());
        }
        validate_committee(&self.committee)?;
        if self.authority.validators.len() != self.committee.len() {
            return Err("native epoch consensus and Pasta rosters differ".into());
        }
        for (member, keys) in self.committee.iter().zip(&self.authority.validators) {
            if member.validator != keys.validator {
                return Err("native epoch consensus and Pasta roster order differs".into());
            }
            iroha_zkp_poseidon::pasta_keys::validate_paired_public_keys(
                &keys.eq_proof_public_key,
                &keys.ep_proof_public_key,
            )
            .map_err(str::to_owned)?;
        }
        Ok(())
    }

    /// Hash the complete canonical body, independently of any certificate or signer subset.
    ///
    /// # Errors
    /// Rejects an invalid context or a canonical encoding failure.
    pub fn context_id(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        let bytes = norito::encode_canonical(self).map_err(|error| error.to_string())?;
        Ok(Hash::new_from_chunks(&[EPOCH_DOMAIN, &[0], &bytes]).into())
    }

    /// Validate one contiguous successor, preserving exact original credentials on retention.
    ///
    /// # Errors
    /// Rejects epoch/height gaps, changed mode/network, or relabeled retained credentials.
    pub fn validate_successor(&self, previous: &Self) -> Result<(), String> {
        self.validate()?;
        previous.validate()?;
        self.authorization
            .validate_successor(&previous.authorization)
            .map_err(|error| error.to_string())?;
        if self.mode != previous.mode || self.network_id != previous.network_id {
            return Err("native epoch successor changes network or consensus policy".into());
        }
        if matches!(
            self.authorization.decision,
            KagemushaMintFinalityEpochDecisionV1::Retain
                | KagemushaMintFinalityEpochDecisionV1::RetainAndCancel
        ) && (self.authority != previous.authority || self.committee != previous.committee)
        {
            return Err("native epoch retention substitutes original credentials".into());
        }
        Ok(())
    }
}

/// An atomic boundary result certified by the previous exact native quorum.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi::epoch::ValidatorEpochBoundaryV1")]
pub struct ValidatorEpochBoundaryV1 {
    /// Sole first-release layout version, 1.
    pub version: u16,
    /// Final height B of the current epoch.
    pub height: u64,
    /// Complete context of the incumbent that authorizes this result.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub predecessor_context_id: [u8; 32],
    /// Exact committed B-1 block whose prestate supplies the selection and readiness cut.
    pub selection_anchor: HashOf<BlockHeader>,
    /// Activated or retained authority beginning exactly at B+1.
    pub next: ValidatorEpochContextV1,
    /// Immutable E+2 selection, first frozen by this boundary; absent for an insufficient pool.
    pub preparation: Option<ValidatorCommitteePreparationV1>,
}

impl ValidatorEpochBoundaryV1 {
    /// Validate this result's geometry and exact predecessor without treating it as a proof.
    ///
    /// # Errors
    /// Rejects early/late boundaries, substituted parent, noncontiguous authority or preparation.
    pub fn validate_against(&self, current: &ValidatorEpochContextV1) -> Result<(), String> {
        current.validate()?;
        if self.version != 1
            || self.height != current.authorization.last_height
            || self.predecessor_context_id != current.context_id()?
            || self.selection_anchor.as_ref() == &[0; 32]
            || current.mode != ConsensusMode::Npos
        {
            return Err("native boundary differs from its exact current epoch".into());
        }
        self.next.validate_successor(current)?;
        if let Some(preparation) = &self.preparation {
            preparation.validate_against_preparing_authorization(&self.next.authorization)?;
            if preparation.selection_epoch != current.authorization.epoch
                || preparation.selection_height != self.height
                || preparation.selection_anchor != self.selection_anchor
            {
                return Err("native boundary preparation changes its frozen input cut".into());
            }
        }
        Ok(())
    }
}

/// Validate the exact bounded equal-vote roster and every original proof.
///
/// # Errors
/// Rejects invalid committee size, non-BLS identities, duplicates/order, or invalid proofs.
pub fn validate_committee(members: &[ValidatorCommitteeMemberV1]) -> Result<(), String> {
    let n = members.len();
    if n < 4 || n > MAX_VALIDATORS || (n - 1) % 3 != 0 {
        return Err("native committee must contain exact 3f+1 seats in 4..31".into());
    }
    let mut previous: Option<&[u8]> = None;
    for member in members {
        let (algorithm, key) = member
            .validator
            .public_key()
            .try_to_bytes()
            .map_err(|error| error.to_string())?;
        if algorithm != Algorithm::BlsNormal
            || key.len() != 48
            || previous.is_some_and(|previous| previous >= key)
            || member.proof_of_possession.len() != 96
        {
            return Err("native committee key order or proof shape is invalid".into());
        }
        iroha_crypto::bls_normal_pop_verify(
            member.validator.public_key(),
            &member.proof_of_possession,
        )
        .map_err(|error| error.to_string())?;
        previous = Some(key);
    }
    Ok(())
}

/// Rank one already custody-eligible candidate for one exact E-to-E+2 election.
///
/// Stake gates eligibility and does not multiply votes. Rank collisions are broken by the
/// canonical peer identity by the selector; changing network, epoch, seed or peer changes the
/// domain-separated input. The caller authenticates all supplied inputs from the B prestate.
///
/// # Errors
/// Rejects a zero network/seed, invalid scheduling arithmetic or a non-BLS peer.
pub fn validator_seat_rank(
    network: NetworkId,
    selection_epoch: u64,
    target_epoch: u64,
    seed: [u8; 32],
    peer: &PeerId,
) -> Result<[u8; 32], String> {
    if network.as_bytes() == &[0; 32]
        || seed == [0; 32]
        || selection_epoch.checked_add(2) != Some(target_epoch)
        || peer.public_key().try_algorithm() != Ok(Algorithm::BlsNormal)
    {
        return Err("invalid native validator election rank context".into());
    }
    let identity = norito::encode_canonical(peer).map_err(|error| error.to_string())?;
    Ok(Hash::new_from_chunks(&[
        SEAT_DOMAIN,
        &[0],
        network.as_bytes(),
        &selection_epoch.to_le_bytes(),
        &target_epoch.to_le_bytes(),
        &seed,
        &identity,
    ])
    .into())
}

#[cfg(test)]
pub(crate) mod tests;
